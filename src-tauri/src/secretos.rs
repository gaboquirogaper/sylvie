//! Secretos (tokens y enlaces privados) en el Llavero de macOS / Administrador de credenciales de Windows.
//! Nunca se escriben en archivos ni se envían a la interfaz.
//!
//! Cuentas usadas (todas bajo el servicio "com.gabo.sylvie"):
//! - notion-token      → token de la integración de Notion
//! - calendario-ical   → dirección secreta iCal de Google Calendar
//! - clickup-token     → token personal de ClickUp (pk_…)
//! - asana-token       → token de acceso personal de Asana

use keyring::Entry;

const SERVICIO: &str = "com.gabo.sylvie";
const CUENTA_NOTION: &str = "notion-token";

fn entrada(cuenta: &str) -> Result<Entry, String> {
    Entry::new(SERVICIO, cuenta).map_err(|e| format!("No pude acceder al Llavero: {e}"))
}

pub fn guardar(cuenta: &str, valor: &str) -> Result<(), String> {
    entrada(cuenta)?
        .set_password(valor)
        .map_err(|e| format!("No pude guardar en el Llavero: {e}"))
}

/// Devuelve None si todavía no hay nada guardado en esa cuenta.
pub fn leer(cuenta: &str) -> Result<Option<String>, String> {
    match entrada(cuenta)?.get_password() {
        Ok(valor) => Ok(Some(valor)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("No pude leer el Llavero: {e}")),
    }
}

pub fn borrar(cuenta: &str) -> Result<(), String> {
    match entrada(cuenta)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("No pude borrar del Llavero: {e}")),
    }
}

// ── Atajos para Notion (los usa el resto de la app) ─────────────

pub fn guardar_token_notion(token: &str) -> Result<(), String> {
    guardar(CUENTA_NOTION, token)
}

pub fn leer_token_notion() -> Result<Option<String>, String> {
    leer(CUENTA_NOTION)
}

pub fn borrar_token_notion() -> Result<(), String> {
    borrar(CUENTA_NOTION)
}
