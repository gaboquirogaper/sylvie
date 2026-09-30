//! Cliente mínimo de la API de Notion. La única conexión a internet de Sylvie.

use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const API: &str = "https://api.notion.com/v1";
/// Versión de la API de Notion (bases de datos "clásicas"): estable y soportada.
const VERSION_NOTION: &str = "2022-06-28";

/// Una base de datos de Notion que la integración puede ver.
#[derive(Clone, Serialize, Deserialize)]
pub struct BaseDatos {
    pub id: String,
    pub titulo: String,
}

/// Un elemento (página) de una base de datos, con sus fechas.
pub struct PaginaEditada {
    pub id: String,
    pub titulo: String,
    pub url: String,
    pub creada: DateTime<Utc>,
    pub editada: DateTime<Utc>,
}

/// Traduce los códigos de error de Notion a lenguaje simple.
fn explicar_error(codigo: u16, contexto: &str) -> String {
    match codigo {
        401 => "Notion rechazó el token: es incorrecto o fue revocado.".into(),
        403 => format!(
            "La integración no tiene permiso para leer {contexto}. En Notion, revisa que tenga la capacidad «Leer contenido»."
        ),
        404 => format!(
            "Notion no encuentra {contexto}: puede que ya no esté compartida con la integración o que se haya borrado."
        ),
        429 => "Notion pide esperar (demasiadas solicitudes). Se reintentará en la próxima revisión.".into(),
        c if c >= 500 => format!("Notion tiene problemas ahora mismo ({c}). Se reintentará solo."),
        c => format!("Notion respondió con un error ({c}) al consultar {contexto}."),
    }
}

async fn pedir(
    token: &str,
    metodo: reqwest::Method,
    ruta: &str,
    cuerpo: Option<Value>,
    contexto: &str,
) -> Result<Value, String> {
    let mut solicitud = reqwest::Client::new()
        .request(metodo, format!("{API}{ruta}"))
        .bearer_auth(token)
        .header("Notion-Version", VERSION_NOTION);
    if let Some(cuerpo) = cuerpo {
        solicitud = solicitud.json(&cuerpo);
    }
    let respuesta = solicitud
        .send()
        .await
        .map_err(|e| format!("No pude conectarme a Notion. ¿Hay internet? ({e})"))?;
    let codigo = respuesta.status().as_u16();
    if codigo != 200 {
        return Err(explicar_error(codigo, contexto));
    }
    respuesta
        .json::<Value>()
        .await
        .map_err(|e| format!("Notion respondió algo inesperado: {e}"))
}

/// Une los trozos de texto enriquecido de Notion en un texto simple.
fn texto_plano(partes: &Value) -> String {
    partes
        .as_array()
        .map(|partes| partes.iter().filter_map(|p| p["plain_text"].as_str()).collect::<String>())
        .unwrap_or_default()
}

fn fecha(valor: &Value) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(valor.as_str()?)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

/// Comprueba el token pidiendo a Notion "¿quién soy?".
/// Devuelve una descripción legible, p. ej. "Sylvie · espacio «Gabo»".
pub async fn verificar_token(token: &str) -> Result<String, String> {
    let yo = pedir(token, reqwest::Method::GET, "/users/me", None, "tu usuario").await?;
    let integracion = yo["name"].as_str().unwrap_or("tu integración").to_string();
    Ok(match yo["bot"]["workspace_name"].as_str() {
        Some(espacio) => format!("{integracion} · espacio «{espacio}»"),
        None => integracion,
    })
}

/// Lista las bases de datos que la integración puede ver (las compartidas con ella).
pub async fn listar_bases(token: &str) -> Result<Vec<BaseDatos>, String> {
    let mut bases = Vec::new();
    let mut cursor: Option<String> = None;

    for _ in 0..5 {
        let mut cuerpo = json!({
            "filter": { "property": "object", "value": "database" },
            "page_size": 100
        });
        if let Some(c) = &cursor {
            cuerpo["start_cursor"] = json!(c);
        }
        let r = pedir(token, reqwest::Method::POST, "/search", Some(cuerpo), "tus bases de datos").await?;

        for base in r["results"].as_array().into_iter().flatten() {
            let Some(id) = base["id"].as_str() else {
                continue;
            };
            let titulo = texto_plano(&base["title"]);
            bases.push(BaseDatos {
                id: id.to_string(),
                titulo: if titulo.trim().is_empty() { "Sin título".into() } else { titulo },
            });
        }

        match (r["has_more"].as_bool(), r["next_cursor"].as_str()) {
            (Some(true), Some(c)) => cursor = Some(c.to_string()),
            _ => break,
        }
    }

    bases.sort_by_key(|b| b.titulo.to_lowercase());
    Ok(bases)
}

/// Elementos de una base editados (o creados) desde `desde`, del más reciente al más antiguo.
pub async fn paginas_editadas_desde(
    token: &str,
    base: &BaseDatos,
    desde: DateTime<Utc>,
) -> Result<Vec<PaginaEditada>, String> {
    let cuerpo = json!({
        "filter": {
            "timestamp": "last_edited_time",
            "last_edited_time": { "on_or_after": desde.to_rfc3339_opts(SecondsFormat::Secs, true) }
        },
        "sorts": [{ "timestamp": "last_edited_time", "direction": "descending" }],
        "page_size": 50
    });
    let contexto = format!("la base «{}»", base.titulo);
    let r = pedir(
        token,
        reqwest::Method::POST,
        &format!("/databases/{}/query", base.id),
        Some(cuerpo),
        &contexto,
    )
    .await?;

    let mut paginas = Vec::new();
    for p in r["results"].as_array().into_iter().flatten() {
        let (Some(id), Some(url), Some(creada), Some(editada)) = (
            p["id"].as_str(),
            p["url"].as_str(),
            fecha(&p["created_time"]),
            fecha(&p["last_edited_time"]),
        ) else {
            continue;
        };
        // El título es la propiedad de tipo "title" (se llame como se llame en tu base).
        let titulo = p["properties"]
            .as_object()
            .and_then(|props| props.values().find(|v| v["type"] == "title"))
            .map(|v| texto_plano(&v["title"]))
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| "Sin título".into());

        paginas.push(PaginaEditada {
            id: id.into(),
            titulo,
            url: url.into(),
            creada,
            editada,
        });
    }
    Ok(paginas)
}
