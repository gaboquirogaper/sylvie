//! Cliente mínimo de la API de Notion. La única conexión a internet de Sylvie.

use serde::Deserialize;

const API: &str = "https://api.notion.com/v1";
/// Versión de la API de Notion. Se revisará en la fase 4 (bases de datos).
const VERSION_NOTION: &str = "2022-06-28";

#[derive(Deserialize)]
struct Usuario {
    name: Option<String>,
    bot: Option<Bot>,
}

#[derive(Deserialize)]
struct Bot {
    workspace_name: Option<String>,
}

/// Comprueba el token pidiendo a Notion "¿quién soy?".
/// Devuelve una descripción legible, p. ej. "Sylvie · espacio «Gabo»".
pub async fn verificar_token(token: &str) -> Result<String, String> {
    let respuesta = reqwest::Client::new()
        .get(format!("{API}/users/me"))
        .bearer_auth(token)
        .header("Notion-Version", VERSION_NOTION)
        .send()
        .await
        .map_err(|e| format!("No pude conectarme a Notion. ¿Hay internet? ({e})"))?;

    match respuesta.status().as_u16() {
        200 => {
            let usuario: Usuario = respuesta
                .json()
                .await
                .map_err(|e| format!("Notion respondió algo inesperado: {e}"))?;
            let integracion = usuario.name.unwrap_or_else(|| "tu integración".into());
            Ok(match usuario.bot.and_then(|b| b.workspace_name) {
                Some(espacio) => format!("{integracion} · espacio «{espacio}»"),
                None => integracion,
            })
        }
        401 => Err("Notion rechazó el token: es incorrecto o fue revocado.".into()),
        429 => Err("Notion pide esperar (demasiadas solicitudes). Intenta en un minuto.".into()),
        codigo => Err(format!("Notion respondió con un error ({codigo}).")),
    }
}
