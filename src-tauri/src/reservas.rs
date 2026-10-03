//! Llamadas reservadas (pestaña «Reservas» del notch).
//!
//! - Calendly: API oficial con un token personal (en el Llavero). Solo lectura.
//! - contentBoard: no tiene API pública, pero sí un conector en tu cuenta de Claude.
//!   Sylvie le pide a Claude Code que use SOLO la herramienta «reuniones» de ese conector
//!   y lee directamente lo que devuelve la herramienta (no el texto de Claude).
//!   Como cada consulta usa un poquito de tu plan de Claude, se hace como mucho cada 30 minutos
//!   (o cuando pulsas «Actualizar»), con el modelo más liviano.

use std::{
    process::Stdio,
    sync::Mutex,
    time::{Duration, Instant},
};

use chrono::{Duration as Dias, Utc};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter};

use crate::{calendario, claude, configuracion, integraciones, secretos};

pub const CUENTA_CALENDLY: &str = "calendly-token";
const API_CALENDLY: &str = "https://api.calendly.com";
const DIAS: i64 = 14;
/// Cada cuánto se vuelve a preguntar a contentBoard (vía Claude) como máximo.
const CADA_CONTENTBOARD: Duration = Duration::from_secs(30 * 60);

#[derive(Clone, Serialize)]
pub struct Reserva {
    /// "calendly" o "contentboard"
    origen: &'static str,
    titulo: String,
    invitado: String,
    email: String,
    /// RFC 3339
    inicio: String,
    fin: String,
    /// Enlace de Meet / Zoom / Teams, si lo tiene.
    enlace: Option<String>,
}

#[derive(Serialize)]
pub struct Reservas {
    reservas: Vec<Reserva>,
    errores: Vec<String>,
}

/// Lo último que respondió contentBoard (para no gastar tu plan de Claude en cada vistazo).
static CACHE_CONTENTBOARD: Mutex<Option<(Instant, Vec<Reserva>)>> = Mutex::new(None);

fn cliente() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())
}

// ── Calendly ────────────────────────────────────────────────────

async fn calendly_get(token: &str, url: &str, consulta: &[(&str, String)]) -> Result<Value, String> {
    let r = cliente()?
        .get(url)
        .bearer_auth(token)
        .query(consulta)
        .send()
        .await
        .map_err(|e| format!("No pude conectarme a Calendly. ¿Hay internet? ({e})"))?;
    match r.status().as_u16() {
        200 => r.json().await.map_err(|e| format!("Calendly respondió algo raro: {e}")),
        401 | 403 => Err("Calendly rechazó el token: es incorrecto o fue revocado.".into()),
        429 => Err("Calendly pide esperar un poco.".into()),
        c => Err(format!("Calendly respondió con un error ({c}).")),
    }
}

/// Verifica el token y devuelve "Calendly de <nombre>".
pub async fn verificar_calendly(token: &str) -> Result<String, String> {
    let yo = calendly_get(token, &format!("{API_CALENDLY}/users/me"), &[]).await?;
    Ok(format!("Calendly de {}", yo["resource"]["name"].as_str().unwrap_or("tu cuenta")))
}

async fn reservas_calendly(token: &str) -> Result<Vec<Reserva>, String> {
    let yo = calendly_get(token, &format!("{API_CALENDLY}/users/me"), &[]).await?;
    let usuario = yo["resource"]["uri"].as_str().ok_or("Calendly no devolvió tu usuario.")?.to_string();
    let ahora = Utc::now();
    let eventos = calendly_get(
        token,
        &format!("{API_CALENDLY}/scheduled_events"),
        &[
            ("user", usuario),
            ("status", "active".into()),
            ("min_start_time", (ahora - Dias::hours(2)).to_rfc3339()),
            ("max_start_time", (ahora + Dias::days(DIAS)).to_rfc3339()),
            ("sort", "start_time:asc".into()),
            ("count", "50".into()),
        ],
    )
    .await?;

    let mut salida = Vec::new();
    for e in eventos["collection"].as_array().into_iter().flatten().take(25) {
        let Some(uri) = e["uri"].as_str().filter(|u| u.starts_with(&format!("{API_CALENDLY}/scheduled_events/"))) else {
            continue;
        };
        // Quién reservó (el primer invitado).
        let invitados = calendly_get(token, &format!("{uri}/invitees"), &[("count", "5".into())]).await.ok();
        let primero = invitados.as_ref().and_then(|i| i["collection"].as_array()).and_then(|a| a.first());
        let enlace = e["location"]["join_url"]
            .as_str()
            .or(e["location"]["location"].as_str())
            .filter(|u| {
                reqwest::Url::parse(u)
                    .ok()
                    .and_then(|x| x.host_str().map(|h| calendario::host_permitido(h, &calendario::HOSTS_REUNION)))
                    .unwrap_or(false)
            })
            .map(String::from);
        salida.push(Reserva {
            origen: "calendly",
            titulo: e["name"].as_str().unwrap_or("Reunión").to_string(),
            invitado: primero.and_then(|p| p["name"].as_str()).unwrap_or("").to_string(),
            email: primero.and_then(|p| p["email"].as_str()).unwrap_or("").to_string(),
            inicio: e["start_time"].as_str().unwrap_or("").to_string(),
            fin: e["end_time"].as_str().unwrap_or("").to_string(),
            enlace,
        });
    }
    Ok(salida)
}

// ── contentBoard (a través del conector de Claude) ─────────────

/// ¿El nombre del conector es el de contentBoard? ("Conted Board", "contentBoard"…)
fn es_contentboard(nombre: &str) -> bool {
    let n = nombre.to_lowercase().replace([' ', '_', '-'], "");
    n.contains("board") && (n.contains("cont") || n.contains("conted"))
}

/// Saca el texto que devolvió una herramienta (tool_result) en el stream de Claude Code.
fn resultado_de_herramienta(salida: &str, herramienta: &str) -> Option<String> {
    let mut id_llamada: Option<String> = None;
    for linea in salida.lines() {
        let Ok(v) = serde_json::from_str::<Value>(linea) else { continue };
        for parte in v["message"]["content"].as_array().into_iter().flatten() {
            match parte["type"].as_str() {
                Some("tool_use") if parte["name"].as_str() == Some(herramienta) => {
                    id_llamada = parte["id"].as_str().map(String::from);
                }
                Some("tool_result") if id_llamada.is_some() && parte["tool_use_id"].as_str() == id_llamada.as_deref() => {
                    if parte["is_error"] == true {
                        return None;
                    }
                    return Some(match &parte["content"] {
                        Value::String(t) => t.clone(),
                        Value::Array(a) => a.iter().filter_map(|x| x["text"].as_str()).collect::<Vec<_>>().join(""),
                        _ => String::new(),
                    });
                }
                _ => {}
            }
        }
    }
    None
}

async fn ejecutar_claude(app: &AppHandle, herramienta: &str, con_modelo: bool) -> Result<String, String> {
    let carpeta = configuracion::ruta(app, "espacio-claude")?;
    std::fs::create_dir_all(&carpeta).map_err(|e| e.to_string())?;
    let mut comando = tokio::process::Command::new(claude::ruta_claude());
    comando
        .arg("-p")
        .arg(format!(
            "Llama una sola vez a la herramienta {herramienta} con dias={DIAS}. No hagas nada más y no escribas ningún texto."
        ))
        .args(["--output-format", "stream-json", "--verbose", "--max-turns", "3"]);
    if con_modelo {
        comando.args(["--model", "haiku"]); // el modelo más liviano: gasta menos de tu plan
    }
    comando
        .args(["--allowedTools", herramienta])
        .current_dir(&carpeta)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let salida = tokio::time::timeout(Duration::from_secs(120), comando.output())
        .await
        .map_err(|_| "Claude Code tardó demasiado en consultar contentBoard.".to_string())?
        .map_err(|e| format!("No pude ejecutar Claude Code: {e}"))?;
    Ok(String::from_utf8_lossy(&salida.stdout).to_string())
}

async fn consultar_contentboard(app: &AppHandle) -> Result<Vec<Reserva>, String> {
    let prefijo = integraciones::buscar_conector(app, es_contentboard).await?.ok_or(
        "No encontré el conector de contentBoard en tu Claude (o necesita iniciar sesión). Revísalo en claude.ai → Configuración → Conectores.",
    )?;
    let herramienta = format!("{prefijo}__reuniones");
    let mut texto = ejecutar_claude(app, &herramienta, true).await?;
    let mut datos = resultado_de_herramienta(&texto, &herramienta);
    if datos.is_none() {
        // Por si tu versión de Claude Code no acepta «--model haiku»: otra vez con el modelo normal.
        texto = ejecutar_claude(app, &herramienta, false).await?;
        datos = resultado_de_herramienta(&texto, &herramienta);
    }
    let datos = datos.ok_or("contentBoard no respondió. Prueba «Actualizar» en un rato.")?;
    let lista: Value = serde_json::from_str(datos.trim()).map_err(|_| "contentBoard respondió algo que no entiendo.".to_string())?;
    Ok(lista
        .as_array()
        .into_iter()
        .flatten()
        .map(|r| Reserva {
            origen: "contentboard",
            titulo: r["titulo"].as_str().unwrap_or("Reunión").to_string(),
            invitado: r["invitado"].as_str().unwrap_or("").to_string(),
            email: r["email"].as_str().unwrap_or("").to_string(),
            inicio: r["inicio"].as_str().unwrap_or("").to_string(),
            fin: r["fin"].as_str().unwrap_or("").to_string(),
            enlace: r["enlace"]
                .as_str()
                .or(r["meet"].as_str())
                .filter(|u| {
                    reqwest::Url::parse(u)
                        .ok()
                        .and_then(|x| x.host_str().map(|h| calendario::host_permitido(h, &calendario::HOSTS_REUNION)))
                        .unwrap_or(false)
                })
                .map(String::from),
        })
        .filter(|r| !r.inicio.is_empty())
        .collect())
}

/// Activa contentBoard: comprueba que el conector exista y responda.
#[tauri::command]
pub async fn contentboard_conectar(app: AppHandle) -> Result<String, String> {
    let lista = consultar_contentboard(&app).await?;
    *CACHE_CONTENTBOARD.lock().unwrap() = Some((Instant::now(), lista.clone()));
    let mut ajustes = configuracion::leer(&app);
    ajustes.contentboard = true;
    configuracion::guardar(&app, &ajustes)?;
    let _ = app.emit("conexiones-cambiadas", ());
    let n = lista.len();
    Ok(format!("contentBoard · {n} reserva{} en los próximos {DIAS} días", if n == 1 { "" } else { "s" }))
}

#[tauri::command]
pub fn contentboard_desconectar(app: AppHandle) -> Result<(), String> {
    let mut ajustes = configuracion::leer(&app);
    ajustes.contentboard = false;
    configuracion::guardar(&app, &ajustes)?;
    *CACHE_CONTENTBOARD.lock().unwrap() = None;
    let _ = app.emit("conexiones-cambiadas", ());
    Ok(())
}

// ── Todas las reservas ──────────────────────────────────────────

/// Reservas de los próximos 14 días (Calendly + contentBoard), ordenadas.
/// `forzar` = volver a preguntar a contentBoard aunque no hayan pasado 30 minutos.
#[tauri::command]
pub async fn reservas(app: AppHandle, forzar: Option<bool>) -> Result<Reservas, String> {
    let mut lista = Vec::new();
    let mut errores = Vec::new();

    if let Some(token) = secretos::leer(CUENTA_CALENDLY)? {
        match reservas_calendly(&token).await {
            Ok(r) => lista.extend(r),
            Err(e) => errores.push(e),
        }
    }

    if configuracion::leer(&app).contentboard {
        let guardado = CACHE_CONTENTBOARD.lock().unwrap().clone();
        let vigente = guardado
            .as_ref()
            .filter(|(cuando, _)| !forzar.unwrap_or(false) && cuando.elapsed() < CADA_CONTENTBOARD)
            .map(|(_, r)| r.clone());
        match vigente {
            Some(r) => lista.extend(r),
            None => match consultar_contentboard(&app).await {
                Ok(r) => {
                    *CACHE_CONTENTBOARD.lock().unwrap() = Some((Instant::now(), r.clone()));
                    lista.extend(r);
                }
                Err(e) => {
                    // Si falla, mostrar lo último que sabíamos.
                    if let Some((_, r)) = guardado {
                        lista.extend(r);
                    }
                    errores.push(e);
                }
            },
        }
    }

    lista.sort_by(|a, b| a.inicio.cmp(&b.inicio));
    Ok(Reservas { reservas: lista, errores })
}
