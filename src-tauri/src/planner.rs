//! Microsoft Planner (Microsoft 365): tareas asignadas a ti. Solo lectura.
//!
//! Microsoft no da tokens personales: hay que iniciar sesión. Se usa el "inicio de sesión con
//! código" (device code): Sylvie muestra un código, tú lo escribes en microsoft.com/devicelogin
//! e inicias sesión en tu navegador. Sylvie nunca ve tu contraseña.
//! Necesita el "ID de aplicación (cliente)" de una app que registras gratis en Microsoft Entra.
//! Lo que se guarda en el Llavero: ese ID + el "refresh token" (para no pedir el código otra vez).

use std::{
    collections::HashMap,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

use crate::{integraciones::Tarea, secretos};

const CUENTA: &str = "planner-token";
const LOGIN: &str = "https://login.microsoftonline.com/organizations/oauth2/v2.0";
const GRAPH: &str = "https://graph.microsoft.com/v1.0";
const ALCANCE: &str = "Tasks.Read User.Read offline_access";

/// Cada inicio de sesión tiene un número; si empiezas otro, el anterior deja de esperar.
#[derive(Default)]
pub struct EstadoPlanner(AtomicU64);

#[derive(Serialize, Deserialize)]
struct Guardado {
    cliente: String,
    refresh: String,
}

#[derive(Serialize)]
pub struct CodigoLogin {
    codigo: String,
    url: String,
    /// Minutos que dura el código.
    minutos: u64,
}

#[derive(Clone, Serialize)]
struct FinLogin {
    ok: bool,
    mensaje: String,
}

fn cliente_http() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())
}

/// Traduce los errores de Microsoft (AADSTS…) a algo entendible.
fn explicar(v: &Value) -> String {
    let desc = v["error_description"].as_str().unwrap_or("");
    let codigo = v["error"].as_str().unwrap_or("");
    if desc.contains("AADSTS700016") || desc.contains("AADSTS90002") {
        "Microsoft no encuentra esa app: revisa el «ID de aplicación (cliente)».".into()
    } else if desc.contains("AADSTS7000218") || codigo == "invalid_client" {
        "En tu app de Entra falta activar «Permitir flujos de clientes públicos» (Autenticación → Sí).".into()
    } else if desc.contains("AADSTS65001") || desc.contains("consent") {
        "Tu organización pide que un administrador apruebe la app antes de usarla.".into()
    } else if codigo == "invalid_grant" {
        "La sesión de Microsoft venció o fue cerrada: vuelve a conectar Planner.".into()
    } else if codigo == "authorization_declined" {
        "Rechazaste el permiso en Microsoft.".into()
    } else if codigo == "expired_token" {
        "El código venció. Pulsa «Iniciar sesión» otra vez.".into()
    } else {
        let corto: String = desc.lines().next().unwrap_or(codigo).chars().take(160).collect();
        format!("Microsoft respondió: {corto}")
    }
}

fn id_valido(cliente: &str) -> bool {
    cliente.len() == 36 && cliente.chars().all(|c| c.is_ascii_hexdigit() || c == '-')
}

async fn pedir_token(formulario: &[(&str, &str)]) -> Result<Value, Value> {
    let r = cliente_http()
        .map_err(|e| serde_json::json!({ "error_description": e }))?
        .post(format!("{LOGIN}/token"))
        .form(formulario)
        .send()
        .await
        .map_err(|e| serde_json::json!({ "error_description": format!("No pude conectarme a Microsoft ({e})") }))?;
    let ok = r.status().is_success();
    let v: Value = r.json().await.unwrap_or(Value::Null);
    if ok {
        Ok(v)
    } else {
        Err(v)
    }
}

/// Paso 1: pide el código que el usuario escribe en microsoft.com/devicelogin.
/// Luego espera (en segundo plano) a que inicie sesión y avisa con "planner-login".
#[tauri::command]
pub async fn planner_iniciar(app: AppHandle, cliente: Option<String>) -> Result<CodigoLogin, String> {
    // Sin ID propio: se usa el de Sylvie (claves_publicas.rs).
    let cliente = cliente
        .map(|c| c.trim().to_lowercase())
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| crate::claves_publicas::MICROSOFT_CLIENT_ID.to_lowercase());
    if cliente.is_empty() {
        return Err("Esta versión de Sylvie no trae su ID de Microsoft: usa «Usar mi propia app».".into());
    }
    if !id_valido(&cliente) {
        return Err("El ID de aplicación tiene este formato: 1a2b3c4d-1111-2222-3333-444455556666.".into());
    }
    let r = cliente_http()?
        .post(format!("{LOGIN}/devicecode"))
        .form(&[("client_id", cliente.as_str()), ("scope", ALCANCE)])
        .send()
        .await
        .map_err(|e| format!("No pude conectarme a Microsoft. ¿Hay internet? ({e})"))?;
    let ok = r.status().is_success();
    let v: Value = r.json().await.map_err(|e| format!("Microsoft respondió algo raro: {e}"))?;
    if !ok {
        return Err(explicar(&v));
    }
    let codigo_dispositivo = v["device_code"].as_str().ok_or("Microsoft no dio el código.")?.to_string();
    let codigo = v["user_code"].as_str().unwrap_or("").to_string();
    let url = v["verification_uri"].as_str().unwrap_or("https://microsoft.com/devicelogin").to_string();
    let expira = v["expires_in"].as_u64().unwrap_or(900);
    let mut intervalo = v["interval"].as_u64().unwrap_or(5).max(2);

    let turno = app.state::<EstadoPlanner>().0.fetch_add(1, Ordering::SeqCst) + 1;
    let app2 = app.clone();
    let cliente2 = cliente.clone();
    tauri::async_runtime::spawn(async move {
        let fin = |ok: bool, mensaje: String| {
            let _ = app2.emit("planner-login", FinLogin { ok, mensaje });
        };
        let mut esperado = 0;
        while esperado < expira {
            tokio::time::sleep(Duration::from_secs(intervalo)).await;
            esperado += intervalo;
            if app2.state::<EstadoPlanner>().0.load(Ordering::SeqCst) != turno {
                return; // se canceló o empezó otro inicio de sesión
            }
            let respuesta = pedir_token(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", cliente2.as_str()),
                ("device_code", codigo_dispositivo.as_str()),
            ])
            .await;
            match respuesta {
                Ok(v) => {
                    let Some(refresh) = v["refresh_token"].as_str() else {
                        return fin(false, "Microsoft no devolvió una sesión duradera (falta offline_access).".into());
                    };
                    let guardado = Guardado { cliente: cliente2.clone(), refresh: refresh.to_string() };
                    let texto = serde_json::to_string(&guardado).unwrap_or_default();
                    if let Err(e) = secretos::guardar(CUENTA, &texto) {
                        return fin(false, e);
                    }
                    let nombre = match v["access_token"].as_str() {
                        Some(token) => graph(token, "/me?$select=displayName")
                            .await
                            .ok()
                            .and_then(|yo| yo["displayName"].as_str().map(String::from)),
                        None => None,
                    };
                    let _ = app2.emit("conexiones-cambiadas", ());
                    return fin(true, format!("Planner de {}", nombre.unwrap_or_else(|| "tu cuenta".into())));
                }
                Err(v) => match v["error"].as_str().unwrap_or("") {
                    "authorization_pending" => {}
                    "slow_down" => intervalo += 5,
                    _ => return fin(false, explicar(&v)),
                },
            }
        }
        fin(false, "El código venció. Pulsa «Iniciar sesión» otra vez.".into());
    });

    Ok(CodigoLogin { codigo, url, minutos: expira / 60 })
}

#[tauri::command]
pub fn planner_cancelar(app: AppHandle) {
    app.state::<EstadoPlanner>().0.fetch_add(1, Ordering::SeqCst);
}

pub fn conectado() -> Result<bool, String> {
    Ok(secretos::leer(CUENTA)?.is_some())
}

pub fn desconectar() -> Result<(), String> {
    secretos::borrar(CUENTA)
}

/// Pide un token de acceso nuevo con el refresh token (y guarda el refresh nuevo si cambió).
async fn token_de_acceso() -> Result<Option<String>, String> {
    let Some(texto) = secretos::leer(CUENTA)? else { return Ok(None) };
    let guardado: Guardado = serde_json::from_str(&texto).map_err(|_| "Vuelve a conectar Planner.".to_string())?;
    let v = pedir_token(&[
        ("grant_type", "refresh_token"),
        ("client_id", guardado.cliente.as_str()),
        ("refresh_token", guardado.refresh.as_str()),
        ("scope", ALCANCE),
    ])
    .await
    .map_err(|v| explicar(&v))?;
    if let Some(nuevo) = v["refresh_token"].as_str().filter(|n| *n != guardado.refresh) {
        let actualizado = Guardado { cliente: guardado.cliente, refresh: nuevo.to_string() };
        let _ = secretos::guardar(CUENTA, &serde_json::to_string(&actualizado).unwrap_or_default());
    }
    Ok(v["access_token"].as_str().map(String::from))
}

async fn graph(token: &str, ruta: &str) -> Result<Value, String> {
    let r = cliente_http()?
        .get(format!("{GRAPH}{ruta}"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| format!("No pude conectarme a Microsoft. ¿Hay internet? ({e})"))?;
    match r.status().as_u16() {
        200 => r.json().await.map_err(|e| format!("Microsoft respondió algo raro: {e}")),
        401 | 403 => Err("Microsoft no dio permiso para leer Planner (¿tu cuenta tiene Microsoft 365?).".into()),
        c => Err(format!("Planner respondió con un error ({c}).")),
    }
}

/// Tareas de Planner asignadas a ti que no están completas.
pub async fn tareas() -> Result<Vec<Tarea>, String> {
    let Some(token) = token_de_acceso().await? else { return Ok(Vec::new()) };
    let datos = graph(&token, "/me/planner/tasks").await?;
    let pendientes: Vec<&Value> = datos["value"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|t| t["percentComplete"].as_i64().unwrap_or(0) < 100)
        .collect();

    // Nombre de cada plan (máximo 10 consultas).
    let mut planes: HashMap<String, String> = HashMap::new();
    for t in &pendientes {
        let Some(id) = t["planId"].as_str() else { continue };
        if planes.contains_key(id) || planes.len() >= 10 {
            continue;
        }
        let seguro = id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        let nombre = if seguro {
            graph(&token, &format!("/planner/plans/{id}?$select=title"))
                .await
                .ok()
                .and_then(|p| p["title"].as_str().map(String::from))
        } else {
            None
        };
        planes.insert(id.to_string(), nombre.unwrap_or_default());
    }

    Ok(pendientes
        .into_iter()
        .map(|t| Tarea::nueva(
            "planner",
            t["title"].as_str().unwrap_or("(Sin nombre)"),
            t["dueDateTime"].as_str().map(String::from),
            t["planId"].as_str().and_then(|id| planes.get(id)).cloned().unwrap_or_default(),
            "https://planner.cloud.microsoft",
        ))
        .collect())
}
