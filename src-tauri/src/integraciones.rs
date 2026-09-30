//! Conexiones con otras apps:
//! - Google Calendar (dirección secreta iCal), ClickUp, Asana y Trello (tokens personales).
//!   Todo se guarda en el Llavero y solo se lee (Sylvie no modifica nada en esas apps).
//! - Qué conectores de Claude (los de claude.ai) puede usar Sylvie en los pedidos.
//! - Abrir apps y enlaces permitidos (botones del notch, «Unirse» a una reunión).

use std::{process::Stdio, time::Duration};

use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter};
use tauri_plugin_opener::OpenerExt;

use crate::{calendario, claude, configuracion, secretos};

const CUENTA_CALENDARIO: &str = "calendario-ical";
const CUENTA_CLICKUP: &str = "clickup-token";
const CUENTA_ASANA: &str = "asana-token";
/// Trello usa dos datos: clave de API + token. Se guardan juntos como "clave|token".
const CUENTA_TRELLO: &str = "trello-token";
const API_TRELLO: &str = "https://api.trello.com/1";
const API_CLICKUP: &str = "https://api.clickup.com/api/v2";
const API_ASANA: &str = "https://app.asana.com/api/1.0";
const MAX_TAREAS: usize = 40;

/// Sitios que Sylvie puede abrir en el navegador.
const HOSTS_PERMITIDOS: [&str; 9] = [
    "trello.com",
    "calendar.notion.so",
    "calendar.google.com",
    "app.clickup.com",
    "app.asana.com",
    "notion.so",
    "open.spotify.com",
    "music.apple.com",
    "claude.ai",
];

fn cuenta(id: &str) -> Result<&'static str, String> {
    match id {
        "calendario" => Ok(CUENTA_CALENDARIO),
        "clickup" => Ok(CUENTA_CLICKUP),
        "asana" => Ok(CUENTA_ASANA),
        "trello" => Ok(CUENTA_TRELLO),
        _ => Err("No conozco esa app.".into()),
    }
}

fn cliente() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())
}

// ── Estado ──────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct EstadoApp {
    id: &'static str,
    conectada: bool,
}

/// Qué apps tienen su llave guardada (nunca devuelve la llave).
#[tauri::command]
pub fn estado_conexiones() -> Result<Vec<EstadoApp>, String> {
    Ok(vec![
        EstadoApp { id: "notion", conectada: secretos::leer_token_notion()?.is_some() },
        EstadoApp { id: "calendario", conectada: secretos::leer(CUENTA_CALENDARIO)?.is_some() },
        EstadoApp { id: "clickup", conectada: secretos::leer(CUENTA_CLICKUP)?.is_some() },
        EstadoApp { id: "asana", conectada: secretos::leer(CUENTA_ASANA)?.is_some() },
        EstadoApp { id: "trello", conectada: secretos::leer(CUENTA_TRELLO)?.is_some() },
    ])
}

/// Verifica la llave con la app y, solo si funciona, la guarda en el Llavero.
#[tauri::command]
pub async fn conectar_app(app: AppHandle, id: String, valor: String) -> Result<String, String> {
    let valor = valor.trim().to_string();
    if valor.is_empty() {
        return Err("Pega el dato que te pide cada paso.".into());
    }
    let (guardar, descripcion) = match id.as_str() {
        "calendario" => {
            let url = calendario::normalizar_url(&valor)?;
            let texto = calendario::descargar(&url).await?;
            let eventos = calendario::eventos_semana(&texto).len();
            let nombre = calendario::nombre(&texto);
            (url, format!("{nombre} · {eventos} eventos esta semana"))
        }
        "clickup" => {
            let usuario = clickup(&valor, "/user").await?;
            let nombre = usuario["user"]["username"].as_str().unwrap_or("tu cuenta");
            (valor.clone(), format!("ClickUp de {nombre}"))
        }
        "asana" => {
            let yo = asana(&valor, "/users/me?opt_fields=name").await?;
            let nombre = yo["data"]["name"].as_str().unwrap_or("tu cuenta");
            (valor.clone(), format!("Asana de {nombre}"))
        }
        "trello" => {
            let (clave, token) = partes_trello(&valor)?;
            let yo = trello(&clave, &token, "/members/me", &[("fields", "fullName,username")]).await?;
            let nombre = yo["fullName"].as_str().or(yo["username"].as_str()).unwrap_or("tu cuenta");
            (format!("{clave}|{token}"), format!("Trello de {nombre}"))
        }
        _ => return Err("No conozco esa app.".into()),
    };
    secretos::guardar(cuenta(&id)?, &guardar)?;
    let _ = app.emit("conexiones-cambiadas", ());
    Ok(descripcion)
}

#[tauri::command]
pub fn desconectar_app(app: AppHandle, id: String) -> Result<(), String> {
    secretos::borrar(cuenta(&id)?)?;
    let _ = app.emit("conexiones-cambiadas", ());
    Ok(())
}

// ── Calendario ──────────────────────────────────────────────────

/// Eventos de hoy y los próximos 7 días. Lista vacía si no hay calendario conectado.
#[tauri::command]
pub async fn eventos_calendario() -> Result<Vec<calendario::Evento>, String> {
    let Some(url) = secretos::leer(CUENTA_CALENDARIO)? else {
        return Ok(Vec::new());
    };
    let texto = calendario::descargar(&url).await?;
    Ok(calendario::eventos_semana(&texto))
}

// ── Tareas (ClickUp y Asana) ────────────────────────────────────

#[derive(Serialize)]
pub struct Tarea {
    app: &'static str,
    titulo: String,
    /// Fecha de vencimiento (RFC 3339) si tiene.
    vence: Option<String>,
    /// Lista o proyecto donde está.
    lugar: String,
    url: String,
}

#[derive(Serialize)]
pub struct Tareas {
    tareas: Vec<Tarea>,
    /// Problemas por app (una app caída no esconde las otras).
    errores: Vec<String>,
}

fn explicar(app: &str, codigo: u16) -> String {
    match codigo {
        401 | 403 => format!("{app} rechazó el token: es incorrecto o fue revocado."),
        429 => format!("{app} pide esperar un poco (demasiadas consultas)."),
        c if c >= 500 => format!("{app} tiene problemas ahora mismo ({c})."),
        c => format!("{app} respondió con un error ({c})."),
    }
}

async fn clickup(token: &str, ruta: &str) -> Result<Value, String> {
    let r = cliente()?
        .get(format!("{API_CLICKUP}{ruta}"))
        .header("Authorization", token)
        .send()
        .await
        .map_err(|e| format!("No pude conectarme a ClickUp. ¿Hay internet? ({e})"))?;
    if !r.status().is_success() {
        return Err(explicar("ClickUp", r.status().as_u16()));
    }
    r.json().await.map_err(|e| format!("ClickUp respondió algo raro: {e}"))
}

async fn asana(token: &str, ruta: &str) -> Result<Value, String> {
    let r = cliente()?
        .get(format!("{API_ASANA}{ruta}"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("No pude conectarme a Asana. ¿Hay internet? ({e})"))?;
    if !r.status().is_success() {
        return Err(explicar("Asana", r.status().as_u16()));
    }
    r.json().await.map_err(|e| format!("Asana respondió algo raro: {e}"))
}

async fn tareas_clickup(token: &str) -> Result<Vec<Tarea>, String> {
    let usuario = clickup(token, "/user").await?;
    let yo = usuario["user"]["id"].as_i64().ok_or("ClickUp no devolvió tu usuario.")?;
    let equipos = clickup(token, "/team").await?;
    let mut tareas = Vec::new();
    for equipo in equipos["teams"].as_array().into_iter().flatten().take(3) {
        let Some(id) = equipo["id"].as_str() else { continue };
        let ruta = format!("/team/{id}/task?assignees%5B%5D={yo}&subtasks=true&include_closed=false&order_by=due_date");
        let datos = clickup(token, &ruta).await?;
        for t in datos["tasks"].as_array().into_iter().flatten() {
            let vence = t["due_date"]
                .as_str()
                .and_then(|ms| ms.parse::<i64>().ok())
                .and_then(DateTime::<Utc>::from_timestamp_millis)
                .map(|d| d.to_rfc3339());
            tareas.push(Tarea {
                app: "clickup",
                titulo: t["name"].as_str().unwrap_or("(Sin nombre)").to_string(),
                vence,
                lugar: t["list"]["name"].as_str().unwrap_or("").to_string(),
                url: t["url"].as_str().unwrap_or("https://app.clickup.com").to_string(),
            });
        }
    }
    Ok(tareas)
}

async fn tareas_asana(token: &str) -> Result<Vec<Tarea>, String> {
    let yo = asana(token, "/users/me?opt_fields=workspaces.name").await?;
    let mut tareas = Vec::new();
    for espacio in yo["data"]["workspaces"].as_array().into_iter().flatten().take(3) {
        let Some(gid) = espacio["gid"].as_str() else { continue };
        let ruta = format!(
            "/tasks?assignee=me&workspace={gid}&completed_since=now&limit=50&opt_fields=name,due_on,due_at,permalink_url,projects.name"
        );
        let datos = asana(token, &ruta).await?;
        for t in datos["data"].as_array().into_iter().flatten() {
            // due_at = fecha y hora; due_on = solo fecha (se toma el final de ese día).
            let vence = t["due_at"].as_str().map(String::from).or_else(|| {
                let dia = NaiveDate::parse_from_str(t["due_on"].as_str()?, "%Y-%m-%d").ok()?;
                let fin = Local.from_local_datetime(&dia.and_hms_opt(23, 59, 0)?).earliest()?;
                Some(fin.to_rfc3339())
            });
            let lugar = t["projects"]
                .as_array()
                .and_then(|p| p.first())
                .and_then(|p| p["name"].as_str())
                .unwrap_or("")
                .to_string();
            tareas.push(Tarea {
                app: "asana",
                titulo: t["name"].as_str().unwrap_or("(Sin nombre)").to_string(),
                vence,
                lugar,
                url: t["permalink_url"].as_str().unwrap_or("https://app.asana.com").to_string(),
            });
        }
    }
    Ok(tareas)
}

fn partes_trello(valor: &str) -> Result<(String, String), String> {
    let (clave, token) = valor
        .split_once('|')
        .ok_or("Pega la clave de API y el token de Trello.")?;
    let (clave, token) = (clave.trim(), token.trim());
    if clave.is_empty() || token.is_empty() {
        return Err("Faltan la clave de API o el token de Trello.".into());
    }
    let valido = |t: &str| t.chars().all(|c| c.is_ascii_alphanumeric());
    if !valido(clave) || !valido(token) {
        return Err("La clave y el token de Trello solo tienen letras y números. Revisa que copiaste bien.".into());
    }
    Ok((clave.to_string(), token.to_string()))
}

async fn trello(clave: &str, token: &str, ruta: &str, extra: &[(&str, &str)]) -> Result<Value, String> {
    let r = cliente()?
        .get(format!("{API_TRELLO}{ruta}"))
        .query(&[("key", clave), ("token", token)])
        .query(extra)
        .send()
        .await
        .map_err(|e| format!("No pude conectarme a Trello. ¿Hay internet? ({e})"))?;
    if !r.status().is_success() {
        return Err(explicar("Trello", r.status().as_u16()));
    }
    r.json().await.map_err(|e| format!("Trello respondió algo raro: {e}"))
}

/// Tarjetas abiertas en las que estás como miembro (las completadas no aparecen).
async fn tareas_trello(guardado: &str) -> Result<Vec<Tarea>, String> {
    let (clave, token) = partes_trello(guardado)?;
    let tableros = trello(&clave, &token, "/members/me/boards", &[("fields", "name"), ("filter", "open")]).await?;
    let nombres: std::collections::HashMap<String, String> = tableros
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|b| Some((b["id"].as_str()?.to_string(), b["name"].as_str()?.to_string())))
        .collect();
    let tarjetas = trello(
        &clave,
        &token,
        "/members/me/cards",
        &[("filter", "open"), ("fields", "name,due,dueComplete,shortUrl,idBoard")],
    )
    .await?;
    Ok(tarjetas
        .as_array()
        .into_iter()
        .flatten()
        .filter(|t| t["dueComplete"] != true)
        .filter(|t| t["idBoard"].as_str().is_some_and(|id| nombres.contains_key(id)))
        .map(|t| Tarea {
            app: "trello",
            titulo: t["name"].as_str().unwrap_or("(Sin nombre)").to_string(),
            vence: t["due"].as_str().map(String::from),
            lugar: t["idBoard"].as_str().and_then(|id| nombres.get(id)).cloned().unwrap_or_default(),
            url: t["shortUrl"].as_str().unwrap_or("https://trello.com").to_string(),
        })
        .collect())
}

/// Tareas pendientes asignadas a ti en ClickUp, Asana y Trello (las que vencen antes, primero).
#[tauri::command]
pub async fn tareas_pendientes() -> Result<Tareas, String> {
    let mut tareas = Vec::new();
    let mut errores = Vec::new();
    if let Some(token) = secretos::leer(CUENTA_CLICKUP)? {
        match tareas_clickup(&token).await {
            Ok(t) => tareas.extend(t),
            Err(e) => errores.push(e),
        }
    }
    if let Some(guardado) = secretos::leer(CUENTA_TRELLO)? {
        match tareas_trello(&guardado).await {
            Ok(t) => tareas.extend(t),
            Err(e) => errores.push(e),
        }
    }
    if let Some(token) = secretos::leer(CUENTA_ASANA)? {
        match tareas_asana(&token).await {
            Ok(t) => tareas.extend(t),
            Err(e) => errores.push(e),
        }
    }
    // Primero las que tienen fecha (la más cercana arriba), luego las sin fecha.
    let momento = |v: &Option<String>| v.as_deref().and_then(|t| DateTime::parse_from_rfc3339(t).ok());
    tareas.sort_by(|a, b| match (momento(&a.vence), momento(&b.vence)) {
        (Some(x), Some(y)) => x.cmp(&y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.titulo.cmp(&b.titulo),
    });
    tareas.truncate(MAX_TAREAS);
    Ok(Tareas { tareas, errores })
}

// ── Conectores de Claude ────────────────────────────────────────

#[derive(Serialize)]
pub struct Conector {
    nombre: String,
    prefijo: String,
    estado: String,
    conectado: bool,
    permitido: bool,
}

/// Claude Code nombra las herramientas "mcp__<servidor>__<herramienta>", cambiando
/// por "_" todo lo que no sea letra, número, "_" o "-".
fn prefijo_de(nombre: &str) -> String {
    let limpio: String = nombre
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    format!("mcp__{limpio}")
}

/// Lee `claude mcp list`. Líneas del estilo:
/// "claude.ai Notion: https://mcp.notion.com/mcp - ✓ Connected"
fn leer_lista(salida: &str) -> Vec<(String, String, bool)> {
    salida
        .lines()
        .filter_map(|l| {
            let (nombre, resto) = l.split_once(": ")?;
            let (_, estado) = resto.rsplit_once(" - ")?;
            let nombre = nombre.trim().to_string();
            if nombre.is_empty() {
                return None;
            }
            let estado = estado.trim().to_string();
            let conectado = estado.contains("Connected") && !estado.contains("Not");
            Some((nombre, estado, conectado))
        })
        .collect()
}

/// Conectores que tiene tu Claude Code y si Sylvie los deja usar.
/// Tarda unos segundos: Claude Code revisa cada conector.
#[tauri::command]
pub async fn conectores_claude(app: AppHandle) -> Result<Vec<Conector>, String> {
    let carpeta = configuracion::ruta(&app, "espacio-claude")?;
    std::fs::create_dir_all(&carpeta).map_err(|e| e.to_string())?;
    let hijo = tokio::process::Command::new(claude::ruta_claude())
        .args(["mcp", "list"])
        .current_dir(&carpeta)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .output();
    let salida = tokio::time::timeout(Duration::from_secs(45), hijo)
        .await
        .map_err(|_| "Claude Code tardó demasiado en listar sus conectores.".to_string())?
        .map_err(|e| format!("No pude ejecutar Claude Code: {e}"))?;
    let texto = String::from_utf8_lossy(&salida.stdout);

    let permitidos = configuracion::leer(&app).claude_apps;
    let mut lista: Vec<Conector> = leer_lista(&texto)
        .into_iter()
        .map(|(nombre, estado, conectado)| {
            let prefijo = prefijo_de(&nombre);
            Conector {
                permitido: permitidos.contains(&prefijo),
                nombre: nombre.trim_start_matches("claude.ai ").to_string(),
                prefijo,
                estado,
                conectado,
            }
        })
        .collect();
    // Permitidos que Claude Code ya no muestra (para poder quitarlos).
    for p in &permitidos {
        if !lista.iter().any(|c| &c.prefijo == p) {
            lista.push(Conector {
                nombre: claude::nombre_conector(p),
                prefijo: p.clone(),
                estado: "No aparece en Claude Code".into(),
                conectado: false,
                permitido: true,
            });
        }
    }
    Ok(lista)
}

/// Deja (o no) que Claude use un conector en los pedidos de Sylvie.
#[tauri::command]
pub fn permitir_conector(app: AppHandle, prefijo: String, permitido: bool) -> Result<Vec<String>, String> {
    if !configuracion::prefijo_valido(&prefijo) {
        return Err("Ese conector no es válido.".into());
    }
    let mut ajustes = configuracion::leer(&app);
    ajustes.claude_apps.retain(|p| p != &prefijo);
    if permitido {
        ajustes.claude_apps.push(prefijo);
    }
    configuracion::guardar(&app, &ajustes)?;
    Ok(ajustes.claude_apps)
}

// ── Abrir enlaces y apps ────────────────────────────────────────

/// Abre un enlace solo si es de una reunión (Meet/Zoom/Teams) o de una app conectada.
#[tauri::command]
pub fn abrir_enlace(app: AppHandle, url: String) -> Result<(), String> {
    let u = reqwest::Url::parse(&url).map_err(|_| "Ese enlace no es válido.".to_string())?;
    let host = u.host_str().unwrap_or("");
    let permitido = u.scheme() == "https"
        && (calendario::host_permitido(host, &calendario::HOSTS_REUNION)
            || calendario::host_permitido(host, &HOSTS_PERMITIDOS));
    if !permitido {
        return Err("Sylvie no abre ese tipo de enlace.".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| format!("No pude abrir el enlace: {e}"))
}

#[cfg(target_os = "macos")]
fn abrir_en_mac(identificador: &str) -> bool {
    std::process::Command::new("open")
        .args(["-b", identificador])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn corriendo(proceso: &str) -> bool {
    std::process::Command::new("pgrep")
        .args(["-x", proceso])
        .stdout(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Botones del notch: abre la app de escritorio (o su web si no está instalada).
/// Solo acepta: "notion", "musica", "calendario", "notion-calendar", "clickup", "asana", "trello".
#[tauri::command]
pub fn abrir_app(app: AppHandle, cual: String) -> Result<(), String> {
    let web = match cual.as_str() {
        "notion" => "https://www.notion.so",
        "musica" => "https://open.spotify.com",
        "calendario" => "https://calendar.google.com",
        "clickup" => "https://app.clickup.com",
        "asana" => "https://app.asana.com",
        "trello" => "https://trello.com",
        "notion-calendar" => "https://calendar.notion.so",
        _ => return Err("No conozco esa app.".into()),
    };

    #[cfg(target_os = "macos")]
    {
        let abierta = match cual.as_str() {
            "notion" => abrir_en_mac("notion.id"),
            "clickup" => abrir_en_mac("com.clickup.desktop-app"),
            "asana" => abrir_en_mac("com.electron.asana"),
            "trello" => abrir_en_mac("com.atlassian.trello"),
            "notion-calendar" => abrir_en_mac("com.cron.electron"),
            // Música: la que está sonando; si ninguna, Spotify y si no, Música.
            "musica" => {
                if corriendo("Spotify") {
                    abrir_en_mac("com.spotify.client")
                } else if corriendo("Music") {
                    abrir_en_mac("com.apple.Music")
                } else {
                    abrir_en_mac("com.spotify.client") || abrir_en_mac("com.apple.Music")
                }
            }
            _ => false,
        };
        if abierta {
            return Ok(());
        }
    }

    app.opener()
        .open_url(web, None::<&str>)
        .map_err(|e| format!("No pude abrir la app: {e}"))
}
