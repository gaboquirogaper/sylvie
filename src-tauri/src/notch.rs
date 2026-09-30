//! Ventana del notch: posición, ajustes nativos de macOS y vigilancia del cursor.

use std::{sync::Mutex, thread, time::Duration};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, WebviewWindow};

use crate::configuracion;

/// Nombre interno (label) de la ventana del notch en tauri.conf.json.
pub const ETIQUETA: &str = "notch";

/// Tamaño de la ventana (transparente) en cada apariencia, en píxeles lógicos.
const TAM_NOTCH: (f64, f64) = (760.0, 360.0);
const TAM_FLOTANTE: (f64, f64) = (760.0, 460.0);
/// Distancia de la mascota flotante al borde de la pantalla.
const MARGEN_FLOTANTE: f64 = 8.0;

// La ventana es grande (760×360) y casi toda transparente. Para no bloquear
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
    /// Zonas donde el mouse sí interactúa (en la mascota flotante: mascota + globo/panel).
    zonas: Vec<Zona>,
    /// Último valor avisado a la interfaz (None = hay que volver a avisar).
    ultimo: Option<bool>,
}

pub struct Compartido(Mutex<EstadoCursor>);

impl Compartido {
    pub fn nuevo() -> Self {
        Compartido(Mutex::new(EstadoCursor {
            // Zona inicial = estado "escondida" (se corrige apenas carga la interfaz).
            zonas: vec![Zona { x: 228.0, y: 0.0, ancho: 304.0, alto: 34.0 }],
            ultimo: None,
        }))
    }
}

/// La interfaz llama a esto cada vez que la píldora (o la mascota) cambia de estado.
#[tauri::command]
pub fn fijar_zonas(zonas: Vec<Zona>, compartido: tauri::State<Compartido>) {
    let mut estado = compartido.0.lock().unwrap();
    estado.zonas = zonas.into_iter().take(4).collect();
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
            let dentro = estado.zonas.iter().any(|z| z.contiene(x, y));
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

/// Ubica la ventana según la apariencia elegida en Ajustes:
/// - notch: centrada en el borde superior de la pantalla principal (= el notch en una MacBook).
/// - flotante: en la esquina elegida, dentro del área útil (sin tapar el Dock ni la barra de menú).
fn ubicar(ventana: &WebviewWindow, ajustes: &configuracion::Ajustes) -> tauri::Result<()> {
    let Some(monitor) = ventana.primary_monitor()? else {
        return Ok(());
    };
    let flotante = ajustes.apariencia == "flotante";
    let (ancho, alto) = if flotante { TAM_FLOTANTE } else { TAM_NOTCH };
    ventana.set_size(LogicalSize::new(ancho, alto))?;

    let escala = monitor.scale_factor();
    let ancho_f = (ancho * escala) as i32;
    let alto_f = (alto * escala) as i32;

    if !flotante {
        let pos = monitor.position();
        let x = pos.x + (monitor.size().width as i32 - ancho_f) / 2;
        return ventana.set_position(PhysicalPosition::new(x, pos.y));
    }

    let area = monitor.work_area();
    let margen = (MARGEN_FLOTANTE * escala) as i32;
    let izquierda = area.position.x + margen;
    let derecha = area.position.x + area.size.width as i32 - ancho_f - margen;
    let arriba = area.position.y + margen;
    let abajo = area.position.y + area.size.height as i32 - alto_f - margen;
    let x = if ajustes.esquina.ends_with("der") { derecha } else { izquierda };
    let y = if ajustes.esquina.starts_with("abajo") { abajo } else { arriba };
    ventana.set_position(PhysicalPosition::new(x, y))
}

#[derive(Clone, Serialize)]
struct Apariencia {
    modo: String,
    esquina: String,
}

/// Reubica la ventana y avisa a la interfaz (al cambiar la apariencia o la esquina).
pub fn aplicar_apariencia(app: &AppHandle) {
    let ajustes = configuracion::leer(app);
    if let Some(ventana) = app.get_webview_window(ETIQUETA) {
        let _ = ubicar(&ventana, &ajustes);
    }
    let _ = app.emit(
        "apariencia",
        Apariencia {
            modo: ajustes.apariencia,
            esquina: ajustes.esquina,
        },
    );
}

/// Al terminar de arrastrar la mascota: la pega a la esquina más cercana y la recuerda.
#[tauri::command]
pub fn soltar_mascota(app: AppHandle, ventana: WebviewWindow) -> Result<(), String> {
    let (Ok(pos), Ok(tam), Ok(Some(monitor))) = (ventana.outer_position(), ventana.outer_size(), ventana.primary_monitor()) else {
        return Err("No pude ubicar la ventana.".into());
    };
    let area = monitor.work_area();
    let centro_x = pos.x + tam.width as i32 / 2;
    let centro_y = pos.y + tam.height as i32 / 2;
    let medio_x = area.position.x + area.size.width as i32 / 2;
    let medio_y = area.position.y + area.size.height as i32 / 2;
    let esquina = match (centro_y >= medio_y, centro_x >= medio_x) {
        (true, true) => "abajo-der",
        (true, false) => "abajo-izq",
        (false, true) => "arriba-der",
        (false, false) => "arriba-izq",
    };
    let mut ajustes = configuracion::leer(&app);
    ajustes.esquina = esquina.into();
    configuracion::guardar(&app, &ajustes)?;
    aplicar_apariencia(&app);
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

    ubicar(&ventana, &configuracion::leer(app))?;
    // Al inicio, los clics atraviesan la ventana; el vigilante los activa sobre la píldora.
    ventana.set_ignore_cursor_events(true)?;

    #[cfg(target_os = "macos")]
    ajustes_macos(&ventana)?;

    #[cfg(not(target_os = "macos"))]
    ventana.show()?;

    vigilar_cursor(app.clone());
    Ok(())
}
