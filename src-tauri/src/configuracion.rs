//! Ventana de Configuración, ajustes guardados en disco y comandos para la interfaz.
//!
//! - Ajustes normales (intervalo, bases vigiladas) → ajustes.json en la carpeta de la app
//!   (~/Library/Application Support/com.gabo.sylvie/ en Mac).
//! - Token de Notion → Llavero (ver secretos.rs). Nunca en el JSON.

use std::{fs, path::PathBuf, sync::Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::{avisos::EstadoAvisos, notion, secretos};

pub const ETIQUETA: &str = "configuracion";
const ARCHIVO_AJUSTES: &str = "ajustes.json";

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Ajustes {
    /// Cada cuántos minutos revisar Notion (1 a 120).
    pub intervalo_minutos: u32,
    /// Bases de datos de Notion que Sylvie vigila.
    pub bases: Vec<notion::BaseDatos>,
    /// Conectores de Claude que Sylvie deja usar en los pedidos (prefijos "mcp__…").
    pub claude_apps: Vec<String>,
}

impl Default for Ajustes {
    fn default() -> Self {
        Ajustes {
            intervalo_minutos: 5,
            bases: Vec::new(),
            claude_apps: vec!["mcp__claude_ai_Notion".into()],
        }
    }
}

/// Sección que la ventana de Configuración debe mostrar al abrirse.
#[derive(Default)]
pub struct SeccionPendiente(pub Mutex<Option<String>>);

const SECCIONES: [&str; 4] = ["general", "conexiones", "notion", "claude"];

/// Ruta de un archivo dentro de la carpeta de datos de Sylvie (la crea si no existe).
pub fn ruta(app: &AppHandle, archivo: &str) -> Result<PathBuf, String> {
    let carpeta = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("No encontré la carpeta de configuración: {e}"))?;
    fs::create_dir_all(&carpeta)
        .map_err(|e| format!("No pude crear la carpeta de configuración: {e}"))?;
    Ok(carpeta.join(archivo))
}

pub fn leer(app: &AppHandle) -> Ajustes {
    ruta(app, ARCHIVO_AJUSTES)
        .ok()
        .and_then(|r| fs::read_to_string(r).ok())
        .and_then(|texto| serde_json::from_str(&texto).ok())
        .unwrap_or_default()
}

pub fn guardar(app: &AppHandle, ajustes: &Ajustes) -> Result<(), String> {
    let texto = serde_json::to_string_pretty(ajustes).map_err(|e| e.to_string())?;
    fs::write(ruta(app, ARCHIVO_AJUSTES)?, texto)
        .map_err(|e| format!("No pude guardar los ajustes: {e}"))
}

/// Abre la ventana de Configuración (o la trae al frente si ya está abierta).
pub fn abrir_ventana(app: &AppHandle) -> tauri::Result<()> {
    if let Some(ventana) = app.get_webview_window(ETIQUETA) {
        ventana.show()?;
        ventana.set_focus()?;
        return Ok(());
    }
    let ventana = WebviewWindowBuilder::new(app, ETIQUETA, WebviewUrl::App("configuracion.html".into()))
        .title("Ajustes de Sylvie")
        .inner_size(780.0, 580.0)
        .min_inner_size(640.0, 460.0)
        .resizable(true)
        .center()
        .build()?;
    ventana.set_focus()?;
    Ok(())
}

// ── Comandos que usa la interfaz ────────────────────────────────

/// Abre la Configuración desde el panel del notch.
/// Es `async` a propósito: en Windows, crear ventanas desde un comando síncrono puede bloquear la app.
/// `seccion` (opcional): "general", "conexiones", "notion" o "claude".
#[tauri::command]
pub async fn abrir_configuracion(app: AppHandle, seccion: Option<String>) -> Result<(), String> {
    let seccion = seccion.filter(|s| SECCIONES.contains(&s.as_str()));
    if let Some(s) = &seccion {
        if app.get_webview_window(ETIQUETA).is_some() {
            let _ = app.emit_to(ETIQUETA, "ir-a-seccion", s.clone());
        } else if let Ok(mut pendiente) = app.state::<SeccionPendiente>().0.lock() {
            *pendiente = Some(s.clone());
        }
    }
    abrir_ventana(&app).map_err(|e| format!("No pude abrir los Ajustes: {e}"))
}

/// La ventana de Configuración pregunta al arrancar qué sección mostrar.
#[tauri::command]
pub fn tomar_seccion(app: AppHandle) -> Option<String> {
    let estado = app.state::<SeccionPendiente>();
    let seccion = estado.0.lock().ok().and_then(|mut s| s.take());
    seccion
}

/// ¿Hay un token guardado? (Nunca devuelve el token en sí.)
#[tauri::command]
pub fn hay_token() -> Result<bool, String> {
    Ok(secretos::leer_token_notion()?.is_some())
}

/// Verifica el token con Notion y, solo si funciona, lo guarda en el Llavero.
#[tauri::command]
pub async fn guardar_token(app: AppHandle, token: String) -> Result<String, String> {
    let token = token.trim().to_string();
    if token.is_empty() {
        return Err("Pega tu token de Notion.".into());
    }
    let descripcion = notion::verificar_token(&token).await?;
    secretos::guardar_token_notion(&token)?;
    let _ = app.emit("ajustes-cambiados", ());
    app.state::<EstadoAvisos>().revisar_ya();
    Ok(descripcion)
}

/// Prueba la conexión con el token ya guardado.
#[tauri::command]
pub async fn probar_conexion() -> Result<String, String> {
    let Some(token) = secretos::leer_token_notion()? else {
        return Err("Todavía no guardaste un token.".into());
    };
    notion::verificar_token(&token).await
}

#[tauri::command]
pub fn borrar_token(app: AppHandle) -> Result<(), String> {
    secretos::borrar_token_notion()?;
    let _ = app.emit("ajustes-cambiados", ());
    Ok(())
}

/// Bases de datos que la integración puede ver (las compartidas con ella en Notion).
#[tauri::command]
pub async fn listar_bases() -> Result<Vec<notion::BaseDatos>, String> {
    let Some(token) = secretos::leer_token_notion()? else {
        return Err("Primero guarda tu token de Notion.".into());
    };
    notion::listar_bases(&token).await
}

#[tauri::command]
pub fn leer_ajustes(app: AppHandle) -> Ajustes {
    leer(&app)
}

#[tauri::command]
pub fn guardar_ajustes(app: AppHandle, ajustes: Ajustes) -> Result<Ajustes, String> {
    let ajustes = Ajustes {
        intervalo_minutos: ajustes.intervalo_minutos.clamp(1, 120),
        bases: ajustes.bases,
        claude_apps: ajustes
            .claude_apps
            .into_iter()
            .filter(|p| prefijo_valido(p))
            .collect(),
    };
    guardar(&app, &ajustes)?;
    let _ = app.emit("ajustes-cambiados", ());
    // Revisar enseguida con la configuración nueva (y fijar el punto de partida de bases nuevas).
    app.state::<EstadoAvisos>().revisar_ya();
    Ok(ajustes)
}

// ── Inicio automático ───────────────────────────────────────────

/// ¿Sylvie se abre sola al iniciar sesión?
#[tauri::command]
pub fn inicio_automatico(app: AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch()
        .is_enabled()
        .map_err(|e| format!("No pude consultar el inicio automático: {e}"))
}

#[tauri::command]
pub fn fijar_inicio_automatico(app: AppHandle, activo: bool) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    let lanzador = app.autolaunch();
    let resultado = if activo { lanzador.enable() } else { lanzador.disable() };
    resultado.map_err(|e| format!("No pude cambiar el inicio automático: {e}"))?;
    lanzador
        .is_enabled()
        .map_err(|e| format!("No pude consultar el inicio automático: {e}"))
}

/// true cuando corre con `npm run tauri dev` (no la app instalada).
#[tauri::command]
pub fn modo_desarrollo() -> bool {
    cfg!(debug_assertions)
}

/// Un prefijo de conector válido: "mcp__" + letras, números, "_" o "-".
pub fn prefijo_valido(prefijo: &str) -> bool {
    prefijo.len() > 5
        && prefijo.len() < 80
        && prefijo.starts_with("mcp__")
        && prefijo.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}
