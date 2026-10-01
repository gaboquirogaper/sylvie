//! YouTube y YouTube Music en el navegador (Mac): Chrome, Brave, Edge y Safari, vía AppleScript.
//!
//! - Leer QUÉ pestaña tiene YouTube (título y dirección) solo necesita el permiso de
//!   Automatización que macOS pide la primera vez ("Sylvie quiere controlar Google Chrome").
//! - Para saber si está sonando, la posición, la portada y para los botones, Sylvie ejecuta un
//!   pequeño código dentro de esa pestaña. Eso requiere activar en el navegador:
//!     Chrome/Brave/Edge: Ver → Opciones para desarrolladores → Permitir JavaScript desde eventos de Apple
//!     Safari: Desarrollo → Permitir JavaScript desde eventos de Apple
//!   Sin eso, Sylvie muestra el título pero no puede controlar la reproducción (control = false).
//! - Solo mira pestañas de youtube.com/watch, youtube.com/shorts y music.youtube.com.

// Varias partes solo se usan en Mac (en Windows llegarán en la fase 8).
#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};

use crate::musica::Cancion;

/// Se puede apagar en Ajustes (Conexiones → Música).
static ACTIVO: AtomicBool = AtomicBool::new(true);
/// Última pestaña donde se encontró YouTube (para los botones y la portada).
static FUENTE: Mutex<Option<Fuente>> = Mutex::new(None);

#[derive(Clone)]
struct Fuente {
    navegador: &'static str,
    ventana: u32,
    pestana: u32,
    url: String,
}

pub fn fijar_activo(activo: bool) {
    ACTIVO.store(activo, Ordering::Relaxed);
    if !activo {
        *FUENTE.lock().unwrap() = None;
    }
}

pub fn activo() -> bool {
    ACTIVO.load(Ordering::Relaxed)
}

/// Navegadores con AppleScript: (nombre de la app, nombre del proceso, ¿es Safari?).
const NAVEGADORES: [(&str, &str, bool); 4] = [
    ("Google Chrome", "Google Chrome", false),
    ("Brave Browser", "Brave Browser", false),
    ("Microsoft Edge", "Microsoft Edge", false),
    ("Safari", "Safari", true),
];

/// Hosts de donde se descargan portadas de YouTube.
const HOSTS_PORTADA: [&str; 4] = ["i.ytimg.com", "i9.ytimg.com", "lh3.googleusercontent.com", "yt3.ggpht.com"];

// El código que se ejecuta en la pestaña: sin comillas dobles ni barras invertidas,
// para poder meterlo tal cual dentro del texto de AppleScript.
const JS_LEER: &str = "(function(){var v=document.querySelector('video');if(!v)return '';var m=navigator.mediaSession&&navigator.mediaSession.metadata;var a='';if(m&&m.artwork&&m.artwork.length){a=m.artwork[m.artwork.length-1].src;}return JSON.stringify({t:m&&m.title?m.title:document.title,r:m&&m.artist?m.artist:'',p:!v.paused&&!v.ended,c:v.currentTime||0,d:isFinite(v.duration)?v.duration:0,a:a});})()";
const JS_ALTERNAR: &str = "(function(){var v=document.querySelector('video');if(v){if(v.paused){v.play();}else{v.pause();}}return 'ok';})()";
const JS_SIGUIENTE: &str = "(function(){var b=document.querySelector('ytmusic-player-bar .next-button')||document.querySelector('.ytp-next-button');if(b){b.click();}return 'ok';})()";
const JS_ANTERIOR: &str = "(function(){var v=document.querySelector('video');var b=document.querySelector('ytmusic-player-bar .previous-button')||document.querySelector('.ytp-prev-button');if(v&&v.currentTime>3){v.currentTime=0;}else if(b){b.click();}return 'ok';})()";

#[cfg(target_os = "macos")]
fn osascript(script: &str) -> Option<String> {
    crate::musica::osascript(script)
}

#[cfg(target_os = "macos")]
fn abierto(proceso: &str) -> bool {
    std::process::Command::new("pgrep")
        .args(["-x", proceso])
        .output()
        .map(|s| s.status.success())
        .unwrap_or(false)
}

/// Pestañas con YouTube de un navegador: (ventana, pestaña, url, título).
#[cfg(target_os = "macos")]
fn pestanas(navegador: &str, safari: bool) -> Vec<(u32, u32, String, String)> {
    let titulo = if safari { "name" } else { "title" };
    let script = format!(
        "tell application \"{navegador}\"\n\
         set sep to character id 9\n\
         set salida to \"\"\n\
         repeat with w from 1 to (count of windows)\n\
           try\n\
             repeat with t from 1 to (count of tabs of window w)\n\
               set u to URL of tab t of window w\n\
               if u contains \"youtube.com/watch\" or u contains \"music.youtube.com\" or u contains \"youtube.com/shorts\" then\n\
                 set salida to salida & (w as text) & sep & (t as text) & sep & u & sep & ({titulo} of tab t of window w) & linefeed\n\
               end if\n\
             end repeat\n\
           end try\n\
         end repeat\n\
         return salida\n\
         end tell"
    );
    let Some(texto) = osascript(&script) else { return Vec::new() };
    texto
        .lines()
        .filter_map(|l| {
            let mut p = l.splitn(4, '\t');
            Some((p.next()?.trim().parse::<u32>().ok()?, p.next()?.trim().parse::<u32>().ok()?, p.next()?.to_string(), p.next().unwrap_or("").to_string()))
        })
        .take(4)
        .collect()
}

#[cfg(target_os = "macos")]
fn ejecutar_en(fuente: &Fuente, js: &str) -> Option<String> {
    let (n, w, t) = (fuente.navegador, fuente.ventana, fuente.pestana);
    let script = if fuente.navegador == "Safari" {
        format!("tell application \"Safari\" to do JavaScript \"{js}\" in tab {t} of window {w}")
    } else {
        format!("tell application \"{n}\" to execute tab {t} of window {w} javascript \"{js}\"")
    };
    osascript(&script)
}

/// Título de la pestaña sin "(3) " al inicio ni " - YouTube" al final.
fn limpiar_titulo(titulo: &str) -> String {
    let mut t = titulo.trim();
    if t.starts_with('(') {
        if let Some(fin) = t.find(") ") {
            if t[1..fin].chars().all(|c| c.is_ascii_digit()) {
                t = &t[fin + 2..];
            }
        }
    }
    for final_ in [" - YouTube Music", " - YouTube"] {
        if let Some(r) = t.strip_suffix(final_) {
            t = r;
        }
    }
    t.trim().to_string()
}

fn cancion_de(fuente: &Fuente, titulo_pestana: &str, datos: Option<serde_json::Value>) -> Option<Cancion> {
    let app = if fuente.url.contains("music.youtube.com") { "YouTube Music" } else { "YouTube" };
    let (titulo, artista, reproduciendo, posicion, duracion, control) = match datos {
        Some(d) => (
            limpiar_titulo(d["t"].as_str().unwrap_or(titulo_pestana)),
            d["r"].as_str().unwrap_or("").trim().to_string(),
            d["p"].as_bool().unwrap_or(false),
            d["c"].as_f64().unwrap_or(0.0),
            d["d"].as_f64().unwrap_or(0.0),
            true,
        ),
        None => (limpiar_titulo(titulo_pestana), String::new(), false, 0.0, 0.0, false),
    };
    if titulo.is_empty() {
        return None;
    }
    Some(Cancion {
        app: app.into(),
        reproduciendo,
        clave: format!("{app}|{titulo}|{artista}"),
        titulo,
        artista,
        posicion,
        duracion,
        control,
    })
}

/// Busca YouTube en los navegadores abiertos. Prefiere la pestaña que está sonando.
#[cfg(target_os = "macos")]
pub fn buscar() -> Option<Cancion> {
    if !activo() {
        return None;
    }
    let mut mejor: Option<(Fuente, Cancion)> = None;
    for (navegador, proceso, safari) in NAVEGADORES {
        if !abierto(proceso) {
            continue;
        }
        for (ventana, pestana, url, titulo) in pestanas(navegador, safari) {
            let fuente = Fuente { navegador, ventana, pestana, url };
            let datos = ejecutar_en(&fuente, JS_LEER)
                .filter(|t| !t.trim().is_empty() && t.trim() != "missing value")
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok());
            let Some(cancion) = cancion_de(&fuente, &titulo, datos) else { continue };
            let suena = cancion.reproduciendo;
            if mejor.is_none() || suena {
                mejor = Some((fuente, cancion));
            }
            if suena {
                break;
            }
        }
        if mejor.as_ref().is_some_and(|(_, c)| c.reproduciendo) {
            break;
        }
    }
    let (fuente, cancion) = mejor?;
    *FUENTE.lock().unwrap() = Some(fuente);
    Some(cancion)
}

#[cfg(not(target_os = "macos"))]
pub fn buscar() -> Option<Cancion> {
    None
}

/// Botones del reproductor: "alternar", "siguiente", "anterior".
#[cfg(target_os = "macos")]
pub fn controlar(accion: &str) -> Result<(), String> {
    let js = match accion {
        "alternar" => JS_ALTERNAR,
        "siguiente" => JS_SIGUIENTE,
        "anterior" => JS_ANTERIOR,
        _ => return Err("Acción desconocida.".into()),
    };
    ejecutar_con_aviso(js)
}

#[cfg(target_os = "macos")]
pub fn mover(segundos: f64) -> Result<(), String> {
    let js = format!(
        "(function(){{var v=document.querySelector('video');if(v){{v.currentTime={};}}return 'ok';}})()",
        segundos.max(0.0).round() as u64
    );
    ejecutar_con_aviso(&js)
}

#[cfg(target_os = "macos")]
fn ejecutar_con_aviso(js: &str) -> Result<(), String> {
    let fuente = FUENTE.lock().unwrap().clone().ok_or("No encontré YouTube abierto.")?;
    ejecutar_en(&fuente, js).map(|_| ()).ok_or_else(|| {
        if fuente.navegador == "Safari" {
            "Para controlar YouTube en Safari: Desarrollo → Permitir JavaScript desde eventos de Apple.".into()
        } else {
            format!(
                "Para controlar YouTube en {}: Ver → Opciones para desarrolladores → Permitir JavaScript desde eventos de Apple.",
                fuente.navegador
            )
        }
    })
}

#[cfg(not(target_os = "macos"))]
pub fn controlar(_accion: &str) -> Result<(), String> {
    Err("Por ahora, YouTube solo se controla en Mac.".into())
}

#[cfg(not(target_os = "macos"))]
pub fn mover(_segundos: f64) -> Result<(), String> {
    Err("Por ahora, YouTube solo se controla en Mac.".into())
}

/// Dirección de la portada de la canción/video `titulo` (si sigue siendo la que suena).
#[cfg(target_os = "macos")]
pub fn url_portada(titulo: &str) -> Option<String> {
    let fuente = FUENTE.lock().unwrap().clone()?;
    // Con el permiso de JavaScript: la portada que da la página (y se confirma el título).
    let datos: Option<serde_json::Value> = ejecutar_en(&fuente, JS_LEER).and_then(|t| serde_json::from_str(&t).ok());
    if let Some(d) = &datos {
        if limpiar_titulo(d["t"].as_str().unwrap_or("")) != titulo {
            return None;
        }
    }
    let desde_pagina = datos
        .as_ref()
        .and_then(|d| d["a"].as_str().map(String::from))
        .filter(|u| host_ok(u));
    // Sin permiso (o sin portada): la miniatura del video, sacada de la dirección (watch?v=ID).
    desde_pagina.or_else(|| {
        let url = reqwest::Url::parse(&fuente.url).ok()?;
        let id = url.query_pairs().find(|(k, _)| k == "v")?.1.to_string();
        id.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            .then(|| format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg"))
    })
}

#[cfg(not(target_os = "macos"))]
pub fn url_portada(_titulo: &str) -> Option<String> {
    None
}

pub fn host_ok(url: &str) -> bool {
    reqwest::Url::parse(url)
        .ok()
        .filter(|u| u.scheme() == "https")
        .and_then(|u| u.host_str().map(|h| HOSTS_PORTADA.contains(&h)))
        .unwrap_or(false)
}
