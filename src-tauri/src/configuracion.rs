//! Ventana de Configuración, ajustes guardados en disco y comandos para la interfaz.
//!
//! - Ajustes normales (intervalo, y en la fase 4 las bases de datos) → archivo JSON
//!   en la carpeta de datos de la app (~/Library/Application Support/com.gabo.sylvie/).
//! - Token de Notion → Llavero (ver secretos.rs). Nunca en el JSON.

use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};

use crate::{notion, secretos};

pub const ETIQUETA: &str = "configuracion";

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Ajustes {
    /// Cada cuántos minutos revisar Notion (1 a 120).
    pub intervalo_minutos: u32,
}

impl Default for Ajustes {
    fn default() -> Self {
        Ajustes { intervalo_minutos: 5 }
    }
}

fn ruta_archivo(app: &AppHandle) -> Result<PathBuf, String> {
    let carpeta = app
        .path()
        .app_config_dir()
        .map_err(|e| format!("No encontré la carpeta de configuración: {e}"))?;
    fs::create_dir_all(&carpeta).map_err(|e| format!("No pude crear la carpeta de configuración: {e}"))?;
    Ok(carpeta.join("ajustes.json"))
}

pub fn leer(app: &AppHandle) -> Ajustes {
    ruta_archivo(app)
        .ok()
        .and_then(|ruta| fs::read_to_string(ruta).ok())
        .and_then(|texto| serde_json::from_str(&texto).ok())
        .unwrap_or_default()
}

fn guardar(app: &AppHandle, ajustes: &Ajustes) -> Result<(), String> {
    let texto = serde_json::to_string_pretty(ajustes).map_err(|e| e.to_string())?;
    fs::write(ruta_archivo(app)?, texto).map_err(|e| format!("No pude guardar los ajustes: {e}"))
}

/// Abre la ventana de Configuración (o la trae al frente si ya está abierta).
pub fn abrir_ventana(app: &AppHandle) -> tauri::Result<()> {
    if let Some(ventana) = app.get_webview_window(ETIQUETA) {
        ventana.show()?;
        ventana.set_focus()?;
        return Ok(());
    }
    let ventana = WebviewWindowBuilder::new(app, ETIQUETA, WebviewUrl::App("configuracion.html".into()))
        .title("Configuración de Sylvie")
        .inner_size(460.0, 540.0)
        .resizable(false)
        .center()
        .build()?;
    ventana.set_focus()?;
    Ok(())
}

// ── Comandos que usa la interfaz ────────────────────────────────

/// Abre la Configuración desde el panel del notch.
/// Es `async` a propósito: en Windows, crear ventanas desde un comando síncrono puede bloquear la app.
#[tauri::command]
pub async fn abrir_configuracion(app: AppHandle) -> Result<(), String> {
    abrir_ventana(&app).map_err(|e| format!("No pude abrir la Configuración: {e}"))
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

#[tauri::command]
pub fn leer_ajustes(app: AppHandle) -> Ajustes {
    leer(&app)
}

#[tauri::command]
pub fn guardar_ajustes(app: AppHandle, ajustes: Ajustes) -> Result<Ajustes, String> {
    let ajustes = Ajustes {
        intervalo_minutos: ajustes.intervalo_minutos.clamp(1, 120),
    };
    guardar(&app, &ajustes)?;
    let _ = app.emit("ajustes-cambiados", ());
    Ok(ajustes)
}
