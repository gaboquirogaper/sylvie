//! Ícono en la barra de menú (Mac) / bandeja del sistema (Windows) y su menú.

use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
    AppHandle,
};

use crate::configuracion;

/// Ícono provisional: una carita redonda con dos ojos, dibujada por código (36×36).
/// En macOS se marca como "plantilla" y el sistema la pinta según el tema claro/oscuro.
fn icono() -> Image<'static> {
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

pub fn crear(app: &AppHandle) -> tauri::Result<()> {
    let ajustes = MenuItem::with_id(app, "configuracion", "Configuración…", true, None::<&str>)?;
    let separador = PredefinedMenuItem::separator(app)?;
    let salir = MenuItem::with_id(app, "salir", "Salir de Sylvie", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&ajustes, &separador, &salir])?;

    TrayIconBuilder::with_id("sylvie")
        .icon(icono())
        .icon_as_template(true)
        .tooltip("Sylvie")
        .menu(&menu)
        .on_menu_event(|app, evento| match evento.id().as_ref() {
            "configuracion" => {
                if let Err(e) = configuracion::abrir_ventana(app) {
                    eprintln!("No pude abrir la Configuración: {e}");
                }
            }
            "salir" => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}
