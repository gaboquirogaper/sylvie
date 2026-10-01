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
        let app2 = app.clone();
        app.run_on_main_thread(move || compartir_por_airdrop(app2, &rutas))
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

// Cómo terminó el envío: macOS avisa al "delegado" cuando AirDrop termina o se cancela.
// (macOS no informa el porcentaje: la barrita del notch muestra "enviando" hasta que termina.)
#[allow(dead_code)]
#[derive(Clone, serde::Serialize)]
struct EstadoAirDrop {
    /// "enviado", "cancelado" o "error"
    estado: &'static str,
    mensaje: String,
}

#[cfg(target_os = "macos")]
mod delegado {
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObject, NSObjectProtocol};
    use objc2::{define_class, msg_send, AllocAnyThread, DefinedClass};
    use tauri::{AppHandle, Emitter};

    use super::EstadoAirDrop;

    pub struct Datos {
        app: AppHandle,
    }

    define_class!(
        // SAFETY: NSObject no tiene requisitos para heredar de él y no implementamos Drop.
        #[unsafe(super(NSObject))]
        #[name = "SylvieDelegadoAirDrop"]
        #[ivars = Datos]
        pub struct Delegado;

        unsafe impl NSObjectProtocol for Delegado {}

        impl Delegado {
            #[unsafe(method(sharingService:didShareItems:))]
            fn compartido(&self, _servicio: *mut AnyObject, _items: *mut AnyObject) {
                let _ = self.ivars().app.emit(
                    "airdrop",
                    EstadoAirDrop { estado: "enviado", mensaje: "¡Enviado por AirDrop!".into() },
                );
            }

            #[unsafe(method(sharingService:didFailToShareItems:error:))]
            fn fallo(&self, _servicio: *mut AnyObject, _items: *mut AnyObject, error: *mut AnyObject) {
                // 3072 = NSUserCancelledError (cerraste la ventana de AirDrop).
                let codigo: isize = if error.is_null() { 0 } else { unsafe { msg_send![error, code] } };
                let estado = if codigo == 3072 {
                    EstadoAirDrop { estado: "cancelado", mensaje: "Envío cancelado.".into() }
                } else {
                    EstadoAirDrop { estado: "error", mensaje: "AirDrop no pudo enviar los archivos.".into() }
                };
                let _ = self.ivars().app.emit("airdrop", estado);
            }
        }
    );

    impl Delegado {
        pub fn nuevo(app: AppHandle) -> Retained<Self> {
            let este = Self::alloc().set_ivars(Datos { app });
            unsafe { msg_send![super(este), init] }
        }
    }
}

#[cfg(target_os = "macos")]
fn compartir_por_airdrop(app: AppHandle, rutas: &[String]) {
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
            use tauri::Emitter;
            let _ = app.emit(
                "airdrop",
                EstadoAirDrop { estado: "error", mensaje: "AirDrop no está disponible en esta Mac.".into() },
            );
            return;
        }

        // Traer Sylvie al frente para que la ventana de AirDrop no quede escondida.
        let aplicacion: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        let _: () = msg_send![aplicacion, activateIgnoringOtherApps: true];

        // El delegado avisa cuándo termina. AppKit no lo retiene: lo dejamos vivo a propósito
        // (es un objeto diminuto por envío).
        let avisador = delegado::Delegado::nuevo(app);
        let _: () = msg_send![servicio, setDelegate: &*avisador];
        std::mem::forget(avisador);

        let _: () = msg_send![servicio, performWithItems: lista];
    }
}
