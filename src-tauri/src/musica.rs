//! Música en reproducción (Mac): Spotify y Apple Music, vía AppleScript (vía oficial de macOS),
//! y YouTube / YouTube Music en el navegador (ver navegador.rs).
//!
//! - Solo pregunta a una app si ya está abierta (nunca la abre por su cuenta).
//! - La primera vez, macOS pide permiso: "Sylvie quiere controlar Spotify/Música".
//! - Portada: Spotify da un enlace (se descarga la imagen); Apple Music entrega la imagen
//!   directamente. En ambos casos se convierte a "data URL" para la interfaz.
//! - Favorito: solo Apple Music lo permite por AppleScript.
//! - En Windows se hará en la fase 8 con la función oficial de Windows (SMTC).

use std::{process::Command, sync::Mutex, thread, time::Duration};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Serialize, PartialEq)]
pub struct Cancion {
    /// "Spotify" o "Apple Music"
    pub app: String,
    pub reproduciendo: bool,
    pub titulo: String,
    pub artista: String,
    /// Segundos reproducidos y duración total.
    pub posicion: f64,
    pub duracion: f64,
    /// Identifica la canción (para saber cuándo pedir otra portada).
    pub clave: String,
    /// false = se ve qué suena pero no se puede controlar (YouTube sin el permiso del navegador).
    pub control: bool,
}

/// Portadas ya descargadas: (clave de la canción, data URL). Guarda las últimas 20,
/// así volver a una canción anterior es instantáneo.
#[derive(Default)]
pub struct EstadoMusica(Mutex<Vec<(String, String)>>);
const MAX_PORTADAS: usize = 20;

/// Lista cerrada: la interfaz nunca manda un nombre de app libre a AppleScript.
fn nombre_app(etiqueta: &str) -> Result<&'static str, String> {
    match etiqueta {
        "Spotify" => Ok("Spotify"),
        "Apple Music" => Ok("Music"),
        _ => Err("App de música desconocida.".into()),
    }
}

/// AppleScript convierte los decimales según el idioma (1,5 en español): lo normalizamos.
fn numero(texto: &str) -> f64 {
    texto.trim().replace(',', ".").parse().unwrap_or(0.0)
}

#[cfg(target_os = "macos")]
pub(crate) fn osascript(script: &str) -> Option<String> {
    let salida = Command::new("osascript").args(["-e", script]).output().ok()?;
    if !salida.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&salida.stdout).trim_end_matches('\n').to_string())
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
    // Spotify da la duración en milisegundos; Música, en segundos.
    let duracion = if aplicacion == "Spotify" { "((duration of t) / 1000)" } else { "(duration of t)" };
    let script = format!(
        "tell application \"{aplicacion}\"\n\
         if player state is stopped then return \"\"\n\
         set t to current track\n\
         return (player state as text) & tab & (name of t) & tab & (artist of t) & tab & (player position as text) & tab & ({duracion} as text)\n\
         end tell"
    );
    let texto = osascript(&script)?;
    let mut partes = texto.split('\t');
    let estado = partes.next()?;
    let titulo = partes.next()?.trim().to_string();
    let artista = partes.next().unwrap_or("").trim().to_string();
    let posicion = numero(partes.next().unwrap_or("0"));
    let duracion = numero(partes.next().unwrap_or("0"));
    if titulo.is_empty() {
        return None;
    }
    Some(Cancion {
        app: etiqueta.into(),
        reproduciendo: estado.trim() == "playing",
        clave: format!("{etiqueta}|{titulo}|{artista}"),
        titulo,
        artista,
        posicion,
        duracion,
        control: true,
    })
}

/// Lo que suena ahora. Gana la que está reproduciendo; si ninguna, Spotify > Música > YouTube.
#[cfg(target_os = "macos")]
fn actual() -> Option<Cancion> {
    let spotify = if abierta("Spotify") { consultar("Spotify", "Spotify") } else { None };
    let musica = if abierta("Music") { consultar("Music", "Apple Music") } else { None };
    // YouTube solo se busca si Spotify/Música no están sonando (ahorra trabajo).
    let suena_app = spotify.as_ref().is_some_and(|c| c.reproduciendo) || musica.as_ref().is_some_and(|c| c.reproduciendo);
    let web = if suena_app { None } else { crate::navegador::buscar() };
    // YouTube sin el permiso del navegador no sabe si está sonando: si Spotify/Música están
    // en pausa, se asume que lo que escuchas es YouTube.
    let web_sin_control = web.as_ref().is_some_and(|c| !c.control);
    let todas: Vec<Cancion> = if web_sin_control {
        [web, spotify, musica].into_iter().flatten().collect()
    } else {
        [spotify, musica, web].into_iter().flatten().collect()
    };
    todas.iter().find(|c| c.reproduciendo).cloned().or_else(|| todas.into_iter().next())
}

#[cfg(not(target_os = "macos"))]
fn actual() -> Option<Cancion> {
    None
}

/// Revisa cada 1,5 segundos (para enterarse rápido si pasas canciones) y avisa a la interfaz (la posición cambia, así que avisa seguido).
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
            thread::sleep(Duration::from_millis(1500));
        }
    });
}

// ── Portada ─────────────────────────────────────────────────────

fn a_data_url(bytes: &[u8]) -> Option<String> {
    use base64::Engine;
    if bytes.len() < 16 {
        return None;
    }
    let tipo = if bytes.starts_with(&[0x89, b'P', b'N', b'G']) { "image/png" } else { "image/jpeg" };
    Some(format!(
        "data:{tipo};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[cfg(target_os = "macos")]
async fn descargar_portada(app: &AppHandle, cancion: &Cancion) -> Option<String> {
    if cancion.app.starts_with("YouTube") {
        let titulo = cancion.titulo.clone();
        let url = tauri::async_runtime::spawn_blocking(move || crate::navegador::url_portada(&titulo))
            .await
            .ok()
            .flatten()?;
        if !crate::navegador::host_ok(&url) {
            return None;
        }
        let bytes = reqwest::get(&url).await.ok()?.bytes().await.ok()?;
        return a_data_url(&bytes);
    }
    if cancion.app == "Spotify" {
        // Nombre y portada en la MISMA consulta: así la imagen es de esta canción y no de la anterior.
        let texto = osascript(
            "tell application \"Spotify\" to return (name of current track) & tab & (artwork url of current track)",
        )?;
        let (nombre, url) = texto.split_once('\t')?;
        if nombre.trim() != cancion.titulo {
            return None; // cambió de canción mientras tanto
        }
        let url = url.trim().to_string();
        // Solo imágenes de los servidores de Spotify.
        if !url.starts_with("https://i.scdn.co/") {
            return None;
        }
        let bytes = reqwest::get(&url).await.ok()?.bytes().await.ok()?;
        a_data_url(&bytes)
    } else {
        // Apple Music entrega la imagen: la escribimos en la carpeta de caché y la leemos.
        let carpeta = app.path().app_cache_dir().ok()?;
        std::fs::create_dir_all(&carpeta).ok()?;
        let ruta = carpeta.join("portada.bin");
        let ruta_texto = ruta.to_string_lossy().replace('"', "");
        let script = format!(
            "tell application \"Music\" to set d to raw data of artwork 1 of current track\n\
             set f to open for access (POSIX file \"{ruta_texto}\") with write permission\n\
             set eof f to 0\n\
             write d to f\n\
             close access f"
        );
        osascript(&script)?;
        let bytes = std::fs::read(&ruta).ok()?;
        a_data_url(&bytes)
    }
}

#[cfg(not(target_os = "macos"))]
async fn descargar_portada(_app: &AppHandle, _cancion: &Cancion) -> Option<String> {
    None
}

// ── Comandos ────────────────────────────────────────────────────

#[tauri::command]
pub async fn musica_actual() -> Option<Cancion> {
    tauri::async_runtime::spawn_blocking(actual).await.ok().flatten()
}

/// Portada de la canción `clave` como data URL. Devuelve None si ya no es la que suena
/// (así nunca se muestra la portada de otra canción).
#[tauri::command]
pub async fn portada_musica(app: AppHandle, clave: String) -> Option<String> {
    {
        let estado = app.state::<EstadoMusica>();
        let guardadas = estado.0.lock().unwrap();
        if let Some((_, url)) = guardadas.iter().find(|(c, _)| *c == clave) {
            return Some(url.clone());
        }
    }
    let cancion = tauri::async_runtime::spawn_blocking(actual).await.ok().flatten()?;
    if cancion.clave != clave {
        return None;
    }
    let url = descargar_portada(&app, &cancion).await?;
    let estado = app.state::<EstadoMusica>();
    let mut guardadas = estado.0.lock().unwrap();
    guardadas.retain(|(c, _)| *c != clave);
    guardadas.push((clave, url.clone()));
    if guardadas.len() > MAX_PORTADAS {
        guardadas.remove(0);
    }
    Some(url)
}

/// accion: "alternar" (play/pausa), "siguiente" o "anterior".
#[tauri::command]
pub async fn controlar_musica(app_musica: String, accion: String) -> Result<(), String> {
    if app_musica.starts_with("YouTube") {
        return tauri::async_runtime::spawn_blocking(move || crate::navegador::controlar(&accion))
            .await
            .map_err(|e| e.to_string())?;
    }
    let aplicacion = nombre_app(&app_musica)?;
    let orden = match accion.as_str() {
        "alternar" => "playpause",
        "siguiente" => "next track",
        "anterior" => "previous track",
        _ => return Err("Acción desconocida.".into()),
    };
    ejecutar(format!("tell application \"{aplicacion}\" to {orden}"), &app_musica).await
}

/// Mueve la canción a un segundo concreto (la barra de progreso).
#[tauri::command]
pub async fn mover_musica(app_musica: String, segundos: f64) -> Result<(), String> {
    if app_musica.starts_with("YouTube") {
        return tauri::async_runtime::spawn_blocking(move || crate::navegador::mover(segundos))
            .await
            .map_err(|e| e.to_string())?;
    }
    let aplicacion = nombre_app(&app_musica)?;
    let segundos = segundos.max(0.0).round() as u64;
    ejecutar(format!("tell application \"{aplicacion}\" to set player position to {segundos}"), &app_musica).await
}

/// ¿La canción actual está en favoritos? Con `cambiar`, la agrega o la quita.
/// None = no disponible (YouTube, o Spotify sin conectar en Ajustes).
#[tauri::command]
pub async fn favorito_musica(app_musica: String, cambiar: bool) -> Result<Option<bool>, String> {
    if app_musica == "Spotify" {
        #[cfg(target_os = "macos")]
        {
            if !crate::spotify::conectado()? {
                return Ok(None);
            }
            // "spotify:track:ID" (las canciones locales no tienen ID de Spotify).
            let uri = tauri::async_runtime::spawn_blocking(|| osascript("tell application \"Spotify\" to return id of current track"))
                .await
                .map_err(|e| e.to_string())?;
            let Some(id) = uri.as_deref().and_then(|u| u.trim().strip_prefix("spotify:track:")).map(String::from) else {
                return Ok(None);
            };
            return crate::spotify::favorito(&id, cambiar).await;
        }
        #[cfg(not(target_os = "macos"))]
        return Ok(None);
    }
    if app_musica != "Apple Music" {
        return Ok(None);
    }
    #[cfg(target_os = "macos")]
    {
        let script = if cambiar {
            "tell application \"Music\"\nset favorited of current track to (not (favorited of current track))\nreturn favorited of current track as text\nend tell"
        } else {
            "tell application \"Music\" to return favorited of current track as text"
        };
        let texto = tauri::async_runtime::spawn_blocking(move || osascript(script))
            .await
            .map_err(|e| e.to_string())?;
        Ok(texto.map(|t| t.trim() == "true"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = cambiar;
        Ok(None)
    }
}

async fn ejecutar(script: String, app_musica: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let ok = tauri::async_runtime::spawn_blocking(move || osascript(&script).is_some())
            .await
            .map_err(|e| e.to_string())?;
        if !ok {
            return Err(format!(
                "No pude controlar {app_musica}. Revisa el permiso en Configuración del Sistema → \
                 Privacidad y seguridad → Automatización → Sylvie."
            ));
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (script, app_musica);
        Err("Por ahora, el control de música solo funciona en Mac.".into())
    }
}
