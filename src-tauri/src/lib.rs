use std::{sync::Mutex, thread, time::Duration};

use serde::Deserialize;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, PhysicalPosition, WebviewWindow,
};

// ─────────────────────────────────────────────────────────────
// Zona interactiva
// ─────────────────────────────────────────────────────────────
// La ventana es grande (520×220) y casi toda transparente. Para no bloquear
// lo que hay debajo, ignora el mouse salvo dentro de la "zona interactiva",
// que es el rectángulo que ocupa la píldora según su estado
// (escondida, asomada o expandida). La interfaz le dice a Rust cuál es
// esa zona, y Rust vigila el cursor.

/// Rectángulo en píxeles lógicos, relativo a la esquina superior izquierda de la ventana.
#[derive(Clone, Copy, Deserialize)]
struct Zona {
    x: f64,
    y: f64,
    ancho: f64,
    alto: f64,
}

impl Zona {
    fn contiene(&self, x: f64, y: f64) -> bool {
        // Margen de 6 px hacia arriba: el cursor pegado al borde superior de la pantalla también cuenta.
        x >= self.x && x <= self.x + self.ancho && y >= self.y - 6.0 && y <= self.y + self.alto
    }
}

struct EstadoCursor {
    zona: Zona,
    /// Último valor avisado a la interfaz (None = hay que volver a avisar).
    ultimo: Option<bool>,
}

struct Compartido(Mutex<EstadoCursor>);

/// La interfaz llama a esto cada vez que la píldora cambia de estado.
#[tauri::command]
fn fijar_zona(zona: Zona, compartido: tauri::State<Compartido>) {
    let mut estado = compartido.0.lock().unwrap();
    estado.zona = zona;
    estado.ultimo = None; // fuerza a reevaluar y avisar en la próxima vuelta
}

/// Da el foco del teclado a Sylvie (al expandirse, para poder escribir).
#[tauri::command]
fn enfocar(ventana: WebviewWindow) {
    let _ = ventana.set_focus();
}

/// Hilo que mira dónde está el cursor ~25 veces por segundo.
/// Cuando entra o sale de la zona, activa/desactiva los clics en la ventana
/// y avisa a la interfaz con el evento "cursor-notch" (true = dentro).
fn vigilar_cursor(app: AppHandle) {
    thread::spawn(move || loop {
        thread::sleep(Duration::from_millis(40));

        let Some(ventana) = app.get_webview_window("notch") else {
            continue;
        };
        let (Ok(cursor), Ok(pos), Ok(escala)) = (
            ventana.cursor_position(),
            ventana.outer_position(),
            ventana.scale_factor(),
        ) else {
            continue;
        };

        // Posición del cursor relativa a la ventana, en píxeles lógicos (los mismos del CSS).
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

// ─────────────────────────────────────────────────────────────
// Ícono en la barra de menú
// ─────────────────────────────────────────────────────────────

/// Ícono provisional: una carita redonda con dos ojos, dibujada por código (36×36).
/// En macOS se marca como "plantilla" y el sistema la pinta según el tema claro/oscuro.
fn icono_bandeja() -> Image<'static> {
    const TAM: u32 = 36;
    let mut rgba = vec![0u8; (TAM * TAM * 4) as usize];
    for y in 0..TAM {
        for x in 0..TAM {
            let fx = x as f32 + 0.5;
            let fy = y as f32 + 0.5;
            let dentro = |cx: f32, cy: f32, r: f32| ((fx - cx).powi(2) + (fy - cy).powi(2)).sqrt() <= r;
            let cuerpo = dentro(18.0, 19.0, 13.0);
            let ojos = dentro(13.0, 17.0, 2.5) || dentro(23.0, 17.0, 2.5);
            if cuerpo && !ojos {
                let i = ((y * TAM + x) * 4) as usize;
                rgba[i + 3] = 255;
            }
        }
    }
    Image::new_owned(rgba, TAM, TAM)
}

fn crear_bandeja(app: &AppHandle) -> tauri::Result<()> {
    let configuracion = MenuItem::with_id(app, "configuracion", "Configuración…", true, None::<&str>)?;
    let separador = PredefinedMenuItem::separator(app)?;
    let salir = MenuItem::with_id(app, "salir", "Salir de Sylvie", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&configuracion, &separador, &salir])?;

    TrayIconBuilder::with_id("sylvie")
        .icon(icono_bandeja())
        .icon_as_template(true)
        .tooltip("Sylvie")
        .menu(&menu)
        .on_menu_event(|app, evento| match evento.id().as_ref() {
            "configuracion" => {
                // La ventana de Configuración llega en la fase 3.
                let _ = app.emit("abrir-configuracion", ());
            }
            "salir" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────
// Posición y ajustes de la ventana
// ─────────────────────────────────────────────────────────────

/// Centra la ventana en el borde superior de la pantalla principal (= el notch en una MacBook).
fn ubicar_en_notch(ventana: &WebviewWindow) -> tauri::Result<()> {
    let Some(monitor) = ventana.primary_monitor()? else {
        return Ok(());
    };
    let tam_ventana = ventana.outer_size()?;
    let pos_monitor = monitor.position();
    let tam_monitor = monitor.size();
    let x = pos_monitor.x + (tam_monitor.width as i32 - tam_ventana.width as i32) / 2;
    let y = pos_monitor.y;
    ventana.set_position(PhysicalPosition::new(x, y))?;
    Ok(())
}

/// Ajustes nativos de macOS que Tauri no expone directamente.
#[cfg(target_os = "macos")]
fn configurar_ventana_macos(ventana: &WebviewWindow) -> tauri::Result<()> {
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(Compartido(Mutex::new(EstadoCursor {
            // Zona inicial = estado "escondida" (se corrige apenas carga la interfaz).
            zona: Zona { x: 160.0, y: 0.0, ancho: 200.0, alto: 34.0 },
            ultimo: None,
        })))
        .invoke_handler(tauri::generate_handler![fijar_zona, enfocar])
        .setup(|app| {
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            crear_bandeja(app.handle())?;

            let ventana = app
                .get_webview_window("notch")
                .expect("Falta la ventana 'notch' en tauri.conf.json");

            ubicar_en_notch(&ventana)?;
            // Al inicio, los clics atraviesan la ventana; el vigilante los activa sobre la píldora.
            ventana.set_ignore_cursor_events(true)?;

            #[cfg(target_os = "macos")]
            configurar_ventana_macos(&ventana)?;

            #[cfg(not(target_os = "macos"))]
            ventana.show()?;

            vigilar_cursor(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Error al iniciar Sylvie");
}
