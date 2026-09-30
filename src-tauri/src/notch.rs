//! Ventana del notch: posición, ajustes nativos de macOS y vigilancia del cursor.

use std::{sync::Mutex, thread, time::Duration};

use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow};

/// Nombre interno (label) de la ventana del notch en tauri.conf.json.
pub const ETIQUETA: &str = "notch";

// La ventana es grande (520×220) y casi toda transparente. Para no bloquear
// lo que hay debajo, ignora el mouse salvo dentro de la "zona interactiva"
// (el rectángulo que ocupa la píldora según su estado). La interfaz le dice
// a Rust cuál es esa zona, y Rust vigila el cursor.

/// Rectángulo en píxeles lógicos, relativo a la esquina superior izquierda de la ventana.
#[derive(Clone, Copy, Deserialize)]
pub struct Zona {
    x: f64,
    y: f64,
    ancho: f64,
    alto: f64,
}

impl Zona {
    fn contiene(&self, x: f64, y: f64) -> bool {
        // Margen de 6 px hacia arriba: el cursor pegado al borde superior también cuenta.
        x >= self.x && x <= self.x + self.ancho && y >= self.y - 6.0 && y <= self.y + self.alto
    }
}

struct EstadoCursor {
    zona: Zona,
    /// Último valor avisado a la interfaz (None = hay que volver a avisar).
    ultimo: Option<bool>,
}

pub struct Compartido(Mutex<EstadoCursor>);

impl Compartido {
    pub fn nuevo() -> Self {
        Compartido(Mutex::new(EstadoCursor {
            // Zona inicial = estado "escondida" (se corrige apenas carga la interfaz).
            zona: Zona { x: 160.0, y: 0.0, ancho: 200.0, alto: 34.0 },
            ultimo: None,
        }))
    }
}

/// La interfaz llama a esto cada vez que la píldora cambia de estado.
#[tauri::command]
pub fn fijar_zona(zona: Zona, compartido: tauri::State<Compartido>) {
    let mut estado = compartido.0.lock().unwrap();
    estado.zona = zona;
    estado.ultimo = None;
}

/// Da el foco del teclado a Sylvie (al expandirse, para poder escribir).
#[tauri::command]
pub fn enfocar(ventana: WebviewWindow) {
    let _ = ventana.set_focus();
}

/// Hilo que mira dónde está el cursor ~25 veces por segundo y avisa con "cursor-notch".
pub fn vigilar_cursor(app: AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(40));

        let Some(ventana) = app.get_webview_window(ETIQUETA) else {
            continue;
        };
        let (Ok(cursor), Ok(pos), Ok(escala)) = (
            ventana.cursor_position(),
            ventana.outer_position(),
            ventana.scale_factor(),
        ) else {
            continue;
        };

        let x = (cursor.x - pos.x as f64) / escala;
        let y = (cursor.y - pos.y as f64) / escala;

        let cambio = {
            let compartido = app.state::<Compartido>();
            let mut estado = compartido.0.lock().unwrap();
            let dentro = estado.zona.contiene(x, y);
            if estado.ultimo != Some(dentro) {
                estado.ultimo = Some(dentro);
                Some(dentro)
            } else {
                None
            }
        };

        if let Some(dentro) = cambio {
            let _ = ventana.set_ignore_cursor_events(!dentro);
            let _ = app.emit("cursor-notch", dentro);
        }
    });
}

/// Centra la ventana en el borde superior de la pantalla principal (= el notch en una MacBook).
fn ubicar(ventana: &WebviewWindow) -> tauri::Result<()> {
    let Some(monitor) = ventana.primary_monitor()? else {
        return Ok(());
    };
    let tam_ventana = ventana.outer_size()?;
    let pos_monitor = monitor.position();
    let tam_monitor = monitor.size();
    let x = pos_monitor.x + (tam_monitor.width as i32 - tam_ventana.width as i32) / 2;
    ventana.set_position(PhysicalPosition::new(x, pos_monitor.y))?;
    Ok(())
}

/// Ajustes nativos de macOS que Tauri no expone directamente.
#[cfg(target_os = "macos")]
fn ajustes_macos(ventana: &WebviewWindow) -> tauri::Result<()> {
    use objc2::{msg_send, runtime::AnyObject};

    let puntero = ventana.ns_window()? as *mut AnyObject;
    if puntero.is_null() {
        return Ok(());
    }
    unsafe {
        let ns_window: &AnyObject = &*puntero;
        // Nivel 25 = por encima de la barra de menú (nivel 24).
        let _: () = msg_send![ns_window, setLevel: 25isize];
        // Todos los escritorios + quieta en Mission Control + fuera de Cmd+` + junto a pantalla completa.
        let comportamiento: usize = (1 << 0) | (1 << 4) | (1 << 6) | (1 << 8);
        let _: () = msg_send![ns_window, setCollectionBehavior: comportamiento];
        // Mostrarla sin activar la app ni robar el foco.
        let _: () = msg_send![ns_window, orderFrontRegardless];
    }
    Ok(())
}

/// Prepara y muestra la ventana del notch al arrancar.
pub fn iniciar(app: &AppHandle) -> tauri::Result<()> {
    let ventana = app
        .get_webview_window(ETIQUETA)
        .expect("Falta la ventana 'notch' en tauri.conf.json");

    ubicar(&ventana)?;
    // Al inicio, los clics atraviesan la ventana; el vigilante los activa sobre la píldora.
    ventana.set_ignore_cursor_events(true)?;

    #[cfg(target_os = "macos")]
    ajustes_macos(&ventana)?;

    #[cfg(not(target_os = "macos"))]
    ventana.show()?;

    vigilar_cursor(app.clone());
    Ok(())
}
