//! Señales del sistema para la mascota:
//! - cuánto tiempo llevas sin tocar teclado/mouse (para que Sylvie se duerma),
//! - cuándo desbloqueas la pantalla (para que te salude).

use tauri::AppHandle;
#[cfg(target_os = "macos")]
use tauri::Emitter;

/// Segundos desde la última vez que usaste el teclado o el mouse.
#[tauri::command]
pub async fn segundos_inactivo() -> u64 {
    tauri::async_runtime::spawn_blocking(leer_inactividad)
        .await
        .unwrap_or(0)
}

/// macOS publica el dato en el registro de hardware como "HIDIdleTime" (nanosegundos).
#[cfg(target_os = "macos")]
fn leer_inactividad() -> u64 {
    let Ok(salida) = std::process::Command::new("ioreg")
        .args(["-c", "IOHIDSystem", "-d", "4"])
        .output()
    else {
        return 0;
    };
    let texto = String::from_utf8_lossy(&salida.stdout);
    for linea in texto.lines() {
        if let Some(pos) = linea.find("\"HIDIdleTime\" = ") {
            if let Ok(nanos) = linea[pos + 16..].trim().parse::<u64>() {
                return nanos / 1_000_000_000;
            }
        }
    }
    0
}

#[cfg(not(target_os = "macos"))]
fn leer_inactividad() -> u64 {
    0 // Windows: se hará en la fase 8 (GetLastInputInfo).
}

/// Avisa a la interfaz con el evento "desbloqueo" cada vez que desbloqueas la Mac.
#[cfg(target_os = "macos")]
pub fn escuchar_desbloqueo(app: AppHandle) {
    use std::ptr::NonNull;

    use block2::RcBlock;
    use objc2::{class, msg_send, runtime::AnyObject};

    let Ok(nombre_c) = std::ffi::CString::new("com.apple.screenIsUnlocked") else {
        return;
    };
    unsafe {
        let nombre: *mut AnyObject = msg_send![class!(NSString), stringWithUTF8String: nombre_c.as_ptr()];
        let centro: *mut AnyObject = msg_send![class!(NSDistributedNotificationCenter), defaultCenter];
        let bloque = RcBlock::new(move |_aviso: NonNull<AnyObject>| {
            let _ = app.emit("desbloqueo", ());
        });
        let _observador: *mut AnyObject = msg_send![
            centro,
            addObserverForName: nombre,
            object: std::ptr::null_mut::<AnyObject>(),
            queue: std::ptr::null_mut::<AnyObject>(),
            usingBlock: &*bloque
        ];
        // El observador vive mientras viva la app.
        std::mem::forget(bloque);
    }
}

#[cfg(not(target_os = "macos"))]
pub fn escuchar_desbloqueo(_app: AppHandle) {
    // Windows: se hará en la fase 8 (WTS_SESSION_UNLOCK).
}
