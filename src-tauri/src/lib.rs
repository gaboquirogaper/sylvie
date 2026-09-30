//! Sylvie: asistente en el notch.
//!
//! Módulos:
//! - notch          → ventana del notch (posición, cursor, estados)
//! - bandeja        → ícono en la barra de menú y su menú
//! - configuracion  → ventana de Configuración y ajustes guardados
//! - secretos       → Llavero / Administrador de credenciales
//! - notion         → API de Notion
//! - avisos         → revisión periódica de Notion y avisos al notch
//! - claude         → chat con Claude Code (modo no interactivo, solo Notion) e historial
//! - musica         → lo que suena en Spotify / Apple Music y sus controles
//! - archivos       → bandeja de archivos del notch y AirDrop
//! - sistema        → inactividad (para dormir) y desbloqueo de pantalla (para saludar)
//! - calendario     → lee Google Calendar desde su dirección secreta iCal
//! - integraciones  → conectar Calendar / ClickUp / Asana, conectores de Claude, abrir apps

mod archivos;
mod avisos;
mod bandeja;
mod calendario;
mod claude;
mod configuracion;
mod integraciones;
mod musica;
mod notch;
mod notion;
mod secretos;
mod sistema;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // Abrir al iniciar sesión (Mac: LaunchAgent; Windows: registro de inicio).
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .manage(notch::Compartido::nuevo())
        .manage(avisos::EstadoAvisos::default())
        .manage(claude::EstadoClaude::default())
        .manage(musica::EstadoMusica::default())
        .manage(configuracion::SeccionPendiente::default())
        .invoke_handler(tauri::generate_handler![
            notch::fijar_zonas,
            notch::soltar_mascota,
            notch::enfocar,
            configuracion::abrir_configuracion,
            configuracion::tomar_seccion,
            configuracion::hay_token,
            configuracion::guardar_token,
            configuracion::probar_conexion,
            configuracion::borrar_token,
            configuracion::listar_bases,
            configuracion::leer_ajustes,
            configuracion::guardar_ajustes,
            configuracion::inicio_automatico,
            configuracion::fijar_inicio_automatico,
            configuracion::modo_desarrollo,
            avisos::avisos_recientes,
            avisos::revisar_ahora,
            avisos::abrir_en_notion,
            claude::enviar_pedido,
            claude::cancelar_pedido,
            claude::historial,
            musica::musica_actual,
            musica::controlar_musica,
            musica::mover_musica,
            musica::favorito_musica,
            musica::portada_musica,
            archivos::enviar_por_airdrop,
            archivos::mostrar_en_finder,
            sistema::segundos_inactivo,
            integraciones::estado_conexiones,
            integraciones::conectar_app,
            integraciones::desconectar_app,
            integraciones::eventos_calendario,
            integraciones::tareas_pendientes,
            integraciones::conectores_claude,
            integraciones::permitir_conector,
            integraciones::abrir_enlace,
            integraciones::abrir_app,
        ])
        .setup(|app| {
            // En Mac: sin ícono en el Dock; Sylvie vive en la barra de menú.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            bandeja::crear(app.handle())?;
            notch::iniciar(app.handle())?;
            avisos::iniciar(app.handle().clone());
            musica::iniciar(app.handle().clone());
            sistema::escuchar_desbloqueo(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Error al iniciar Sylvie");
}
