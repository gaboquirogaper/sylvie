//! Música en reproducción (Mac): Spotify y Apple Music, vía AppleScript (vía oficial de macOS).
//!
//! - Solo pregunta a una app si ya está abierta (nunca la abre por su cuenta).
//! - La primera vez, macOS pide permiso: "Sylvie quiere controlar Spotify/Música".
//! - En Windows se hará en la fase 8 con la función oficial de Windows (SMTC).

use std::{process::Command, thread, time::Duration};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

#[derive(Clone, Serialize, PartialEq)]
pub struct Cancion {
    /// "Spotify" o "Apple Music"
    pub app: String,
    pub reproduciendo: bool,
    pub titulo: String,
    pub artista: String,
}

/// ¿El proceso está abierto? (sin abrirlo)
#[cfg(target_os = "macos")]
fn abierta(proceso: &str) -> bool {
    Command::new("pgrep")
        .args(["-x", proceso])
        .output()
        .map(|s| s.status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
fn consultar(aplicacion: &str, etiqueta: &str) -> Option<Cancion> {
    let script = format!(
        "tell application \"{aplicacion}\"\n\
         if player state is stopped then return \"\"\n\
         return (player state as text) & tab & (name of current track) & tab & (artist of current track)\n\
         end tell"
    );
    let salida = Command::new("osascript").args(["-e", &script]).output().ok()?;
    if !salida.status.success() {
        return None;
    }
    let texto = String::from_utf8_lossy(&salida.stdout).trim_end_matches('\n').to_string();
    let mut partes = texto.split('\t');
    let estado = partes.next()?;
    let titulo = partes.next()?.trim().to_string();
    let artista = partes.next().unwrap_or("").trim().to_string();
    if titulo.is_empty() {
        return None;
    }
    Some(Cancion {
        app: etiqueta.into(),
        reproduciendo: estado.trim() == "playing",
        titulo,
        artista,
    })
}

/// Lo que suena ahora. Si hay dos apps, gana la que está reproduciendo.
#[cfg(target_os = "macos")]
fn actual() -> Option<Cancion> {
    let spotify = if abierta("Spotify") { consultar("Spotify", "Spotify") } else { None };
    let musica = if abierta("Music") { consultar("Music", "Apple Music") } else { None };
    match (spotify, musica) {
        (Some(s), _) if s.reproduciendo => Some(s),
        (_, Some(m)) if m.reproduciendo => Some(m),
        (s, m) => s.or(m),
    }
}

#[cfg(not(target_os = "macos"))]
fn actual() -> Option<Cancion> {
    None
}

/// Revisa cada 3 segundos y avisa a la interfaz solo cuando algo cambia.
pub fn iniciar(app: AppHandle) {
    thread::spawn(move || {
        let mut anterior: Option<Cancion> = None;
        let mut primera = true;
        loop {
            let ahora = actual();
            if primera || ahora != anterior {
                let _ = app.emit("musica", &ahora);
                anterior = ahora;
                primera = false;
            }
            thread::sleep(Duration::from_secs(3));
        }
    });
}

// ── Comandos ────────────────────────────────────────────────────

#[tauri::command]
pub async fn musica_actual() -> Option<Cancion> {
    tauri::async_runtime::spawn_blocking(actual).await.ok().flatten()
}

/// accion: "alternar" (play/pausa), "siguiente" o "anterior".
#[tauri::command]
pub async fn controlar_musica(app_musica: String, accion: String) -> Result<(), String> {
    // Listas cerradas: la interfaz no puede mandar un AppleScript arbitrario.
    let aplicacion = match app_musica.as_str() {
        "Spotify" => "Spotify",
        "Apple Music" => "Music",
        _ => return Err("App de música desconocida.".into()),
    };
    let orden = match accion.as_str() {
        "alternar" => "playpause",
        "siguiente" => "next track",
        "anterior" => "previous track",
        _ => return Err("Acción desconocida.".into()),
    };

    #[cfg(target_os = "macos")]
    {
        let script = format!("tell application \"{aplicacion}\" to {orden}");
        let salida = tauri::async_runtime::spawn_blocking(move || {
            Command::new("osascript").args(["-e", &script]).output()
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
        if !salida.status.success() {
            return Err(format!(
                "No pude controlar {app_musica}. Revisa el permiso en Configuración del Sistema → \
                 Privacidad y seguridad → Automatización → Sylvie."
            ));
        }
        Ok(())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = (aplicacion, orden);
        Err("Por ahora, el control de música solo funciona en Mac.".into())
    }
}
