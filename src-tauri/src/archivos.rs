//! Bandeja de archivos del notch: enviar por AirDrop (Mac) y mostrar en Finder.
//! Los archivos NO se copian ni se mueven: la bandeja solo guarda sus rutas.

use std::path::Path;

use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
pub fn enviar_por_airdrop(app: AppHandle, rutas: Vec<String>) -> Result<(), String> {
    let rutas: Vec<String> = rutas.into_iter().filter(|r| Path::new(r).exists()).collect();
    if rutas.is_empty() {
        return Err("No hay archivos para enviar (¿se movieron o se borraron?).".into());
    }

    #[cfg(target_os = "macos")]
    {
        app.run_on_main_thread(move || compartir_por_airdrop(&rutas))
            .map_err(|e| format!("No pude abrir AirDrop: {e}"))
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Err("AirDrop solo existe en Mac.".into())
    }
}

#[tauri::command]
pub fn mostrar_en_finder(app: AppHandle, ruta: String) -> Result<(), String> {
    if !Path::new(&ruta).exists() {
        return Err("Ese archivo ya no está ahí.".into());
    }
    app.opener()
        .reveal_item_in_dir(&ruta)
        .map_err(|e| format!("No pude mostrar el archivo: {e}"))
}

// ── AirDrop con la función oficial de macOS (NSSharingService) ──

#[cfg(target_os = "macos")]
fn ns_string(texto: &str) -> *mut objc2::runtime::AnyObject {
    use objc2::{class, msg_send};
    let c = std::ffi::CString::new(texto).unwrap_or_default();
    unsafe { msg_send![class!(NSString), stringWithUTF8String: c.as_ptr()] }
}

#[cfg(target_os = "macos")]
fn compartir_por_airdrop(rutas: &[String]) {
    use objc2::{class, msg_send, runtime::AnyObject};

    unsafe {
        let lista: *mut AnyObject = msg_send![class!(NSMutableArray), array];
        for ruta in rutas {
            let url: *mut AnyObject = msg_send![class!(NSURL), fileURLWithPath: ns_string(ruta)];
            if !url.is_null() {
                let _: () = msg_send![lista, addObject: url];
            }
        }

        let nombre = ns_string("com.apple.share.AirDrop.send");
        let servicio: *mut AnyObject = msg_send![class!(NSSharingService), sharingServiceNamed: nombre];
        if servicio.is_null() {
            eprintln!("[archivos] AirDrop no está disponible en esta Mac");
            return;
        }

        // Traer Sylvie al frente para que la ventana de AirDrop no quede escondida.
        let aplicacion: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![aplicacion, activateIgnoringOtherApps: true];

        let _: () = msg_send![servicio, performWithItems: lista];
    }
}
