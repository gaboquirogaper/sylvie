//! Chat con Claude Code: ejecuta `claude -p` en modo no interactivo, limitado a los
//! conectores que elegiste en Ajustes → Claude (por defecto, solo Notion),
//! y traduce su progreso (stream-json) a pasos en lenguaje simple.
//!
//! Usa tu sesión de Claude Code (plan Pro). No usa la API de Anthropic.

use std::{
    fs,
    path::PathBuf,
    process::Stdio,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, BufReader},
    process::Command,
    sync::Notify,
};

use crate::configuracion;

/// Las herramientas autorizadas salen de Ajustes → Claude (configuracion::Ajustes::claude_apps).
const PREFIJO_NOTION: &str = "mcp__claude_ai_Notion__";
/// Si un pedido tarda más que esto, se detiene.
const LIMITE: Duration = Duration::from_secs(5 * 60);
const ARCHIVO_HISTORIAL: &str = "historial.json";
const MAX_HISTORIAL: usize = 20;

#[derive(Default)]
pub struct EstadoClaude {
    ocupado: AtomicBool,
    cancelar: Notify,
}

/// Un paso del progreso: "paso" (acción) o "texto" (lo que Claude comenta).
#[derive(Clone, Serialize)]
struct Progreso {
    tipo: &'static str,
    texto: String,
}

#[derive(Clone, Serialize)]
struct Fin {
    ok: bool,
    resultado: String,
    segundos: u64,
}

/// Una entrada del historial de pedidos.
#[derive(Clone, Serialize, Deserialize)]
pub struct Entrada {
    pub fecha: String,
    pub pedido: String,
    pub resultado: String,
    pub ok: bool,
}

// ── Utilidades ──────────────────────────────────────────────────

/// Dónde está Claude Code: ~/.local/bin/claude (Mac) o claude.exe (Windows); si no, el PATH.
pub fn ruta_claude() -> PathBuf {
    let casa = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    if let Some(casa) = casa {
        for nombre in ["claude", "claude.exe"] {
            let ruta = casa.join(".local").join("bin").join(nombre);
            if ruta.exists() {
                return ruta;
            }
        }
    }
    PathBuf::from("claude")
}

/// Nombre legible de un conector: "mcp__claude_ai_Google_Calendar" → "Google Calendar".
pub fn nombre_conector(prefijo: &str) -> String {
    let corto = prefijo.trim_start_matches("mcp__");
    let corto = corto.strip_prefix("claude_ai_").unwrap_or(corto);
    corto.replace('_', " ")
}

/// Envuelve el pedido con instrucciones y la fecha de hoy (para entender "el viernes").
fn preparar_pedido(texto: &str, apps: &[String], adjuntos: &[String]) -> String {
    let ahora = Local::now();
    let nombres: Vec<String> = apps.iter().map(|p| nombre_conector(p)).collect();
    let archivos = if adjuntos.is_empty() {
        String::new()
    } else {
        format!(
            "\n\nArchivos adjuntos por el usuario (están en la carpeta adjuntos/ de tu directorio actual; \
léelos con la herramienta Read antes de responder): {}",
            adjuntos.iter().map(|n| format!("adjuntos/{n}")).collect::<Vec<_>>().join(", ")
        )
    };
    format!(
        "Eres Sylvie, una asistente que trabaja con las apps del usuario ({}). \
Reglas: usa solo las herramientas de esas apps; responde en español, en una o dos frases, \
sin tablas ni encabezados; si falta información importante o algo es ambiguo, dilo en vez de inventar. \
Fecha y hora actual: {} ({}).\n\nPedido: {}{}",
        nombres.join(", "),
        ahora.format("%Y-%m-%d %H:%M %:z"),
        ahora.format("%A"),
        texto.trim(),
        archivos
    )
}

/// Traduce el nombre técnico de la herramienta a una frase simple.
fn describir_herramienta(nombre: &str, entrada: &Value) -> String {
    let Some(corto) = nombre.strip_prefix(PREFIJO_NOTION) else {
        // Otros conectores: "mcp__<servidor>__<herramienta>" → "Usando Gmail (search threads)…"
        if let Some(resto) = nombre.strip_prefix("mcp__") {
            if let Some((servidor, herramienta)) = resto.split_once("__") {
                let app = nombre_conector(&format!("mcp__{servidor}"));
                let accion = herramienta.replace(['_', '-'], " ");
                return format!("Usando {app} ({accion})…");
            }
        }
        return match nombre {
            "ToolSearch" => "Preparando las herramientas…".into(),
            "Read" => match entrada["file_path"].as_str().and_then(|r| r.rsplit('/').next()) {
                Some(archivo) => format!("Leyendo «{archivo}»…"),
                None => "Leyendo el archivo adjunto…".into(),
            },
            _ => "Pensando…".into(),
        };
    };
    let busqueda = entrada["query"].as_str().filter(|q| !q.trim().is_empty());
    match corto {
        "notion-search" => match busqueda {
            Some(q) => format!("Buscando «{q}» en Notion…"),
            None => "Buscando en Notion…".into(),
        },
        "notion-fetch" => "Leyendo una página de Notion…".into(),
        "notion-create-pages" => "Creando en Notion…".into(),
        "notion-update-page" => "Actualizando una página…".into(),
        "notion-move-pages" => "Moviendo páginas…".into(),
        "notion-duplicate-page" => "Duplicando una página…".into(),
        "notion-create-database" => "Creando una base de datos…".into(),
        "notion-create-comment" => "Agregando un comentario…".into(),
        "notion-get-comments" => "Leyendo comentarios…".into(),
        "notion-get-users" | "notion-get-teams" => "Consultando personas del espacio…".into(),
        c if c.contains("query") => "Consultando una base de datos…".into(),
        c if c.contains("update") => "Modificando algo en Notion…".into(),
        c => format!("Usando Notion ({c})…"),
    }
}

/// Quita markdown simple (encabezados, negritas) para mostrar texto limpio en el notch.
fn limpiar(texto: &str) -> String {
    texto
        .lines()
        .map(|l| l.trim_start_matches('#').trim().replace("**", ""))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn progreso(app: &AppHandle, tipo: &'static str, texto: &str) {
    let _ = app.emit(
        "claude-progreso",
        Progreso {
            tipo,
            texto: texto.to_string(),
        },
    );
}

/// Explica en simple por qué Claude Code falló, a partir de su salida de errores.
fn explicar_fallo(errores: &str) -> String {
    let e = errores.to_lowercase();
    if e.contains("log in") || e.contains("login") || e.contains("authenticat") {
        "Claude Code no tiene la sesión iniciada. Abre la terminal, ejecuta «claude» e inicia sesión.".into()
    } else if e.contains("limit") {
        "Llegaste al límite de uso de tu plan de Claude por ahora. Intenta más tarde.".into()
    } else if errores.trim().is_empty() {
        "Claude Code se cerró sin responder.".into()
    } else {
        let corto: String = errores.trim().chars().take(300).collect();
        format!("Claude Code falló: {corto}")
    }
}

/// Lee una línea del stream-json. Devuelve Some(...) solo con el resultado final.
fn interpretar(app: &AppHandle, linea: &str, ultimo_texto: &mut String) -> Option<Result<String, String>> {
    let v: Value = serde_json::from_str(linea).ok()?;
    match v["type"].as_str()? {
        "system" if v["subtype"] == "init" => {
            progreso(app, "paso", "Conectando con tus apps…");
            None
        }
        "assistant" => {
            for parte in v["message"]["content"].as_array().into_iter().flatten() {
                match parte["type"].as_str() {
                    Some("text") => {
                        let texto = limpiar(parte["text"].as_str().unwrap_or(""));
                        if !texto.is_empty() {
                            *ultimo_texto = texto.clone();
                            progreso(app, "texto", &texto);
                        }
                    }
                    Some("tool_use") => {
                        let nombre = parte["name"].as_str().unwrap_or("");
                        progreso(app, "paso", &describir_herramienta(nombre, &parte["input"]));
                    }
                    _ => {}
                }
            }
            None
        }
        "user" => {
            let fallo = v["message"]["content"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|p| p["type"] == "tool_result" && p["is_error"] == true);
            if fallo {
                progreso(app, "paso", "Un paso falló; Claude lo intenta de otra forma…");
            }
            None
        }
        "result" => {
            let texto = v["result"]
                .as_str()
                .map(limpiar)
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| ultimo_texto.clone());
            let denegados: Vec<String> = v["permission_denials"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|d| d["tool_name"].as_str().map(String::from))
                .collect();

            if v["is_error"] == true || v["subtype"] != "success" {
                let explicacion = match v["subtype"].as_str() {
                    Some("error_max_turns") => {
                        "El pedido necesitó demasiados pasos y Claude se detuvo. Intenta dividirlo.".to_string()
                    }
                    _ if !texto.is_empty() => texto,
                    Some(otro) => format!("Claude Code terminó con un error ({otro})."),
                    None => "Claude Code terminó con un error.".into(),
                };
                Some(Err(explicacion))
            } else if !denegados.is_empty() {
                Some(Ok(format!(
                    "{texto}\n(Claude quiso usar {} y Sylvie lo bloqueó: actívalo en Ajustes → Claude si quieres permitirlo.)",
                    denegados.join(", ")
                )))
            } else {
                Some(Ok(texto))
            }
        }
        _ => None,
    }
}

// ── Ejecución ───────────────────────────────────────────────────

async fn ejecutar(app: &AppHandle, texto: &str, adjuntos: &[String]) -> Result<String, String> {
    // Carpeta vacía propia: así Claude Code no lee proyectos ni archivos de otros lados.
    let carpeta = configuracion::ruta(app, "espacio-claude")?;
    fs::create_dir_all(&carpeta).map_err(|e| format!("No pude preparar la carpeta de trabajo: {e}"))?;

    let apps = configuracion::leer(app).claude_apps;
    if apps.is_empty() {
        return Err("Claude no tiene permiso para usar ninguna app. Actívalas en Ajustes → Claude.".into());
    }

    progreso(app, "paso", "Despertando a Claude Code…");

    // --allowedTools recibe una lista: cada conector va como un valor aparte (al final del comando).
    let mut hijo = Command::new(ruta_claude())
        .arg("-p")
        .arg(preparar_pedido(texto, &apps, adjuntos))
        .args(["--output-format", "stream-json", "--verbose", "--allowedTools"])
        .args(&apps)
        .current_dir(&carpeta)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "No encontré Claude Code en ~/.local/bin/claude. ¿Está instalado?".to_string()
            } else {
                format!("No pude iniciar Claude Code: {e}")
            }
        })?;

    let salida = hijo.stdout.take().ok_or("No pude leer la salida de Claude Code.")?;
    let mut errores = hijo.stderr.take().ok_or("No pude leer los errores de Claude Code.")?;

    // Leer los errores en paralelo (si no, Claude Code podría quedarse trabado escribiéndolos).
    let lector_errores = tauri::async_runtime::spawn(async move {
        let mut texto = String::new();
        let _ = errores.read_to_string(&mut texto).await;
        texto
    });

    let mut lineas = BufReader::new(salida).lines();
    let estado = app.state::<EstadoClaude>();
    let cancelado = estado.cancelar.notified();
    tokio::pin!(cancelado);
    let limite = tokio::time::sleep(LIMITE);
    tokio::pin!(limite);

    let mut resultado: Option<Result<String, String>> = None;
    let mut ultimo_texto = String::new();

    loop {
        tokio::select! {
            linea = lineas.next_line() => match linea {
                Ok(Some(l)) => {
                    if let Some(r) = interpretar(app, &l, &mut ultimo_texto) {
                        resultado = Some(r);
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    resultado = Some(Err(format!("Error leyendo a Claude Code: {e}")));
                    break;
                }
            },
            _ = &mut cancelado => {
                let _ = hijo.kill().await;
                return Err("Cancelaste el pedido.".into());
            }
            _ = &mut limite => {
                let _ = hijo.kill().await;
                return Err("Claude Code tardó más de 5 minutos y lo detuve.".into());
            }
        }
    }

    let _ = hijo.wait().await;
    let texto_errores = lector_errores.await.unwrap_or_default();
    match resultado {
        Some(r) => r,
        None => Err(explicar_fallo(&texto_errores)),
    }
}

// ── Historial ───────────────────────────────────────────────────

fn leer_historial(app: &AppHandle) -> Vec<Entrada> {
    configuracion::ruta(app, ARCHIVO_HISTORIAL)
        .ok()
        .and_then(|r| fs::read_to_string(r).ok())
        .and_then(|texto| serde_json::from_str(&texto).ok())
        .unwrap_or_default()
}

fn agregar_al_historial(app: &AppHandle, entrada: Entrada) {
    let mut historial = leer_historial(app);
    historial.insert(0, entrada);
    historial.truncate(MAX_HISTORIAL);
    if let (Ok(ruta), Ok(texto)) = (
        configuracion::ruta(app, ARCHIVO_HISTORIAL),
        serde_json::to_string_pretty(&historial),
    ) {
        let _ = fs::write(ruta, texto);
    }
}

// ── Comandos que usa la interfaz ────────────────────────────────

/// Envía un pedido. Responde enseguida; el progreso llega con "claude-progreso"
/// y el final con "claude-fin".
#[tauri::command]
pub async fn enviar_pedido(
    app: AppHandle,
    texto: String,
    adjuntos: Option<Vec<String>>,
    pegado: Option<String>,
) -> Result<(), String> {
    let texto = texto.trim().to_string();
    let adjuntos = adjuntos.unwrap_or_default();
    let pegado = pegado.filter(|p| !p.trim().is_empty());
    if texto.is_empty() && adjuntos.is_empty() && pegado.is_none() {
        return Err("Escribe un pedido.".into());
    }
    let texto = if texto.is_empty() { "Revisa lo que te adjunto y dime qué es, en pocas palabras.".to_string() } else { texto };
    if app.state::<EstadoClaude>().ocupado.swap(true, Ordering::SeqCst) {
        return Err("Ya estoy trabajando en otro pedido.".into());
    }
    let nombres = match preparar_adjuntos(&app, &adjuntos, pegado.as_deref()) {
        Ok(n) => n,
        Err(e) => {
            app.state::<EstadoClaude>().ocupado.store(false, Ordering::SeqCst);
            return Err(e);
        }
    };

    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        let inicio = Instant::now();
        let (ok, resultado) = match ejecutar(&app2, &texto, &nombres).await {
            Ok(r) => (true, r),
            Err(e) => (false, e),
        };
        app2.state::<EstadoClaude>().ocupado.store(false, Ordering::SeqCst);
        agregar_al_historial(
            &app2,
            Entrada {
                fecha: Local::now().to_rfc3339(),
                pedido: if nombres.is_empty() { texto } else { format!("{texto} (📎 {})", nombres.len()) },
                resultado: resultado.clone(),
                ok,
            },
        );
        let _ = app2.emit(
            "claude-fin",
            Fin {
                ok,
                resultado,
                segundos: inicio.elapsed().as_secs(),
            },
        );
    });
    Ok(())
}

// ── Adjuntos ────────────────────────────────────────────────────

const MAX_ADJUNTOS: usize = 5;
const MAX_BYTES_ADJUNTO: u64 = 25 * 1024 * 1024;
const MAX_PEGADO: usize = 200_000;

/// Copia los adjuntos a espacio-claude/adjuntos/ (la única carpeta que Claude puede leer).
/// Borra los del pedido anterior. Devuelve los nombres copiados.
fn preparar_adjuntos(app: &AppHandle, rutas: &[String], pegado: Option<&str>) -> Result<Vec<String>, String> {
    let carpeta = configuracion::ruta(app, "espacio-claude")?.join("adjuntos");
    let _ = fs::remove_dir_all(&carpeta);
    if rutas.is_empty() && pegado.is_none() {
        return Ok(Vec::new());
    }
    fs::create_dir_all(&carpeta).map_err(|e| format!("No pude preparar los adjuntos: {e}"))?;
    if rutas.len() > MAX_ADJUNTOS {
        return Err(format!("Puedes adjuntar hasta {MAX_ADJUNTOS} archivos por pedido."));
    }
    let mut nombres = Vec::new();
    for ruta in rutas {
        let origen = PathBuf::from(ruta);
        let datos = fs::metadata(&origen).map_err(|_| format!("No encontré «{ruta}».", ruta = nombre_de(&origen)))?;
        if !datos.is_file() {
            return Err(format!("«{}» es una carpeta; adjunta archivos sueltos.", nombre_de(&origen)));
        }
        if datos.len() > MAX_BYTES_ADJUNTO {
            return Err(format!("«{}» es muy grande (máximo 25 MB).", nombre_de(&origen)));
        }
        // Nombre seguro y sin repetir.
        let base: String = nombre_de(&origen)
            .chars()
            .map(|c| if c.is_alphanumeric() || ".-_ ".contains(c) { c } else { '_' })
            .collect();
        let mut nombre = base.clone();
        let mut n = 2;
        while nombres.contains(&nombre) {
            nombre = format!("{n}-{base}");
            n += 1;
        }
        fs::copy(&origen, carpeta.join(&nombre)).map_err(|e| format!("No pude copiar «{nombre}»: {e}"))?;
        nombres.push(nombre);
    }
    if let Some(texto) = pegado {
        let texto: String = texto.chars().take(MAX_PEGADO).collect();
        fs::write(carpeta.join("texto-pegado.txt"), texto).map_err(|e| format!("No pude guardar el texto pegado: {e}"))?;
        nombres.push("texto-pegado.txt".into());
    }
    Ok(nombres)
}

fn nombre_de(ruta: &std::path::Path) -> String {
    ruta.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "archivo".into())
}

#[tauri::command]
pub fn cancelar_pedido(estado: tauri::State<EstadoClaude>) {
    estado.cancelar.notify_waiters();
}

#[tauri::command]
pub fn historial(app: AppHandle) -> Vec<Entrada> {
    leer_historial(&app)
}
