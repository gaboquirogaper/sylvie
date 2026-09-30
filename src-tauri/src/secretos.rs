//! Secretos (token de Notion) en el Llavero de macOS / Administrador de credenciales de Windows.
//! Nunca se escriben en archivos ni se envían a la interfaz.

use keyring::Entry;

const SERVICIO: &str = "com.gabo.sylvie";
const CUENTA_NOTION: &str = "notion-token";

fn entrada_notion() -> Result<Entry, String> {
    Entry::new(SERVICIO, CUENTA_NOTION).map_err(|e| format!("No pude acceder al Llavero: {e}"))
}

pub fn guardar_token_notion(token: &str) -> Result<(), String> {
    entrada_notion()?
        .set_password(token)
        .map_err(|e| format!("No pude guardar el token en el Llavero: {e}"))
}

/// Devuelve None si todavía no hay token guardado.
pub fn leer_token_notion() -> Result<Option<String>, String> {
    match entrada_notion()?.get_password() {
        Ok(token) => Ok(Some(token)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(format!("No pude leer el Llavero: {e}")),
    }
}

pub fn borrar_token_notion() -> Result<(), String> {
    match entrada_notion()?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(format!("No pude borrar el token del Llavero: {e}")),
    }
}
