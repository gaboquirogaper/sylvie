//! Indicador «en llamada»: detecta si estás en una videollamada de Zoom, Microsoft Teams
//! o Google Meet, para mostrar en el notch un puntito rojo con el tiempo que llevas.
//!
//! Cómo se detecta (Mac), sin conectar cuentas:
//! - Zoom: durante una reunión corre un proceso propio de Zoom ("CptHost").
//! - Teams: la app de Teams está abierta Y el micrófono está en uso.
//! - Google Meet: el micrófono está en uso Y hay una pestaña meet.google.com/abc-defg-hij.
//! El micrófono se consulta a macOS (CoreAudio): solo se pregunta «¿alguien lo está usando?»,
//! nunca se escucha nada.

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use std::{
    sync::Mutex,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::navegador::PestanaLlamada;

#[derive(Clone, PartialEq)]
enum Origen {
    Zoom,
    Teams,
    Meet(PestanaLlamada),
}

#[derive(Clone, Serialize, PartialEq)]
pub struct Llamada {
    /// "Zoom", "Teams" o "Google Meet"
    app: &'static str,
    /// Cuándo empezó (milisegundos desde 1970), para el cronómetro.
    desde: u64,
    /// ¿Se puede salir desde Sylvie? (Zoom y Meet sí; Teams no)
    puede_salir: bool,
}

static ACTUAL: Mutex<Option<(Origen, Llamada)>> = Mutex::new(None);

fn ahora_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

// ── Mac: proceso abierto y micrófono en uso ─────────────────────

#[cfg(target_os = "macos")]
fn corriendo(proceso: &str) -> bool {
    std::process::Command::new("pgrep")
        .args(["-x", proceso])
        .output()
        .map(|s| s.status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
mod audio {
    //! Pregunta a CoreAudio si el micrófono predeterminado está siendo usado por alguna app.
    use std::ffi::c_void;

    #[repr(C)]
    struct Direccion {
        selector: u32,
        alcance: u32,
        elemento: u32,
    }

    #[link(name = "CoreAudio", kind = "framework")]
    extern "C" {
        fn AudioObjectGetPropertyData(
            objeto: u32,
            direccion: *const Direccion,
            tam_calificador: u32,
            calificador: *const c_void,
            tam_dato: *mut u32,
            dato: *mut c_void,
        ) -> i32;
    }

    const fn cuatro(c: &[u8; 4]) -> u32 {
        ((c[0] as u32) << 24) | ((c[1] as u32) << 16) | ((c[2] as u32) << 8) | (c[3] as u32)
    }
    const SISTEMA: u32 = 1; // kAudioObjectSystemObject
    const ENTRADA_PREDETERMINADA: u32 = cuatro(b"dIn "); // kAudioHardwarePropertyDefaultInputDevice
    const EN_USO_EN_ALGUN_LADO: u32 = cuatro(b"gone"); // kAudioDevicePropertyDeviceIsRunningSomewhere
    const GLOBAL: u32 = cuatro(b"glob"); // kAudioObjectPropertyScopeGlobal

    pub fn microfono_en_uso() -> bool {
        unsafe {
            let mut dispositivo: u32 = 0;
            let mut tam = std::mem::size_of::<u32>() as u32;
            let dir = Direccion { selector: ENTRADA_PREDETERMINADA, alcance: GLOBAL, elemento: 0 };
            if AudioObjectGetPropertyData(SISTEMA, &dir, 0, std::ptr::null(), &mut tam, &mut dispositivo as *mut u32 as *mut c_void) != 0
                || dispositivo == 0
            {
                return false;
            }
            let mut en_uso: u32 = 0;
            let mut tam = std::mem::size_of::<u32>() as u32;
            let dir = Direccion { selector: EN_USO_EN_ALGUN_LADO, alcance: GLOBAL, elemento: 0 };
            AudioObjectGetPropertyData(dispositivo, &dir, 0, std::ptr::null(), &mut tam, &mut en_uso as *mut u32 as *mut c_void) == 0
                && en_uso != 0
        }
    }
}

#[cfg(target_os = "macos")]
fn detectar() -> Option<Origen> {
    if corriendo("CptHost") {
        return Some(Origen::Zoom);
    }
    if !audio::microfono_en_uso() {
        return None;
    }
    if corriendo("MSTeams") || corriendo("Microsoft Teams") {
        return Some(Origen::Teams);
    }
    crate::navegador::pestana_meet().map(Origen::Meet)
}

#[cfg(not(target_os = "macos"))]
fn detectar() -> Option<Origen> {
    None
}

fn nombre(o: &Origen) -> &'static str {
    match o {
        Origen::Zoom => "Zoom",
        Origen::Teams => "Teams",
        Origen::Meet(_) => "Google Meet",
    }
}

/// Revisa cada 3 segundos y avisa con "llamada" (Llamada o null) cuando cambia.
pub fn iniciar(app: AppHandle) {
    thread::spawn(move || loop {
        let detectada = detectar();
        let cambio = {
            let mut actual = ACTUAL.lock().unwrap();
            let mismo_tipo = match (&*actual, &detectada) {
                (Some((a, _)), Some(b)) => std::mem::discriminant(a) == std::mem::discriminant(b),
                (None, None) => true,
                _ => false,
            };
            if mismo_tipo {
                // Misma llamada: actualiza la pestaña de Meet por si se movió, sin reiniciar el tiempo.
                if let (Some((o, _)), Some(d)) = (actual.as_mut(), detectada) {
                    *o = d;
                }
                None
            } else {
                *actual = detectada.map(|o| {
                    let ll = Llamada { app: nombre(&o), desde: ahora_ms(), puede_salir: !matches!(o, Origen::Teams) };
                    (o, ll)
                });
                Some(actual.as_ref().map(|(_, l)| l.clone()))
            }
        };
        if let Some(estado) = cambio {
            let _ = app.emit("llamada", estado);
        }
        thread::sleep(Duration::from_secs(3));
    });
}

/// La llamada en curso (al abrir Sylvie).
#[tauri::command]
pub fn llamada_actual() -> Option<Llamada> {
    ACTUAL.lock().unwrap().as_ref().map(|(_, l)| l.clone())
}

#[cfg(target_os = "macos")]
fn abrir_mac(id: &str) -> bool {
    std::process::Command::new("open").args(["-b", id]).status().map(|s| s.success()).unwrap_or(false)
}

/// Trae la llamada al frente.
#[tauri::command]
pub fn llamada_volver() -> Result<(), String> {
    let origen = ACTUAL.lock().unwrap().as_ref().map(|(o, _)| o.clone()).ok_or("Ya no estás en una llamada.")?;
    #[cfg(target_os = "macos")]
    {
        let ok = match &origen {
            Origen::Zoom => abrir_mac("us.zoom.xos"),
            Origen::Teams => abrir_mac("com.microsoft.teams2") || abrir_mac("com.microsoft.teams"),
            Origen::Meet(p) => crate::navegador::mostrar_pestana(p),
        };
        if ok {
            return Ok(());
        }
    }
    let _ = origen;
    Err("No pude volver a la llamada.".into())
}

/// Sale de la llamada: en Meet cierra la pestaña; en Zoom cierra Zoom. (Teams no se puede.)
#[tauri::command]
pub fn llamada_salir() -> Result<(), String> {
    let origen = ACTUAL.lock().unwrap().as_ref().map(|(o, _)| o.clone()).ok_or("Ya no estás en una llamada.")?;
    #[cfg(target_os = "macos")]
    {
        let ok = match &origen {
            Origen::Zoom => crate::musica::osascript("tell application \"zoom.us\" to quit").is_some(),
            Origen::Meet(p) => crate::navegador::cerrar_pestana(p),
            Origen::Teams => return Err("En Teams sal desde la ventana de la llamada.".into()),
        };
        if ok {
            return Ok(());
        }
    }
    let _ = origen;
    Err("No pude salir de la llamada.".into())
}
