//! Sylvie: asistente en el notch.
//!
//! Módulos:
//! - notch          → ventana del notch (posición, cursor, estados)
//! - bandeja        → ícono en la barra de menú y su menú
//! - configuracion  → ventana de Configuración y ajustes guardados
//! - secretos       → Llavero / Administrador de credenciales
//! - notion         → API de Notion
//! - avisos         → revisión periódica de Notion y avisos al notch
//!
//! Más adelante: claude (fase 5) y fuentes futuras (calendario, hooks,
//! música, archivos…), cada una en su propio módulo.

mod avisos;
mod bandeja;
mod configuracion;
mod notch;
mod notion;
mod secretos;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(notch::Compartido::nuevo())
        .manage(avisos::EstadoAvisos::default())
        .invoke_handler(tauri::generate_handler![
            notch::fijar_zona,
            notch::enfocar,
            configuracion::abrir_configuracion,
            configuracion::hay_token,
            configuracion::guardar_token,
            configuracion::probar_conexion,
            configuracion::borrar_token,
            configuracion::listar_bases,
            configuracion::leer_ajustes,
            configuracion::guardar_ajustes,
            avisos::avisos_recientes,
            avisos::revisar_ahora,
            avisos::abrir_en_notion,
        ])
        .setup(|app| {
            // En Mac: sin ícono en el Dock; Sylvie vive en la barra de menú.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            bandeja::crear(app.handle())?;
            notch::iniciar(app.handle())?;
            avisos::iniciar(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Error al iniciar Sylvie");
}
