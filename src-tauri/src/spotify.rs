//! Spotify: el corazón ("Tus me gusta"). El AppleScript de Spotify no permite guardar canciones,
//! así que se usa la API web oficial de Spotify con tu permiso.
//!
//! Inicio de sesión (OAuth con PKCE, sin contraseñas en Sylvie):
//! 1. Registras una app gratis en developer.spotify.com con la dirección de regreso
//!    http://127.0.0.1:43517/callback y pegas su Client ID en Ajustes.
//! 2. Sylvie abre el navegador en la página de Spotify; aceptas.
//! 3. Spotify vuelve a 127.0.0.1:43517 (tu propia Mac) con un código que Sylvie cambia por el permiso.
//! Se guarda en el Llavero: Client ID + refresh token. Permisos pedidos: leer y modificar "Tus me gusta".

use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};

use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::secretos;

const CUENTA: &str = "spotify-token";
const PUERTO: u16 = 43517;
const REGRESO: &str = "http://127.0.0.1:43517/callback";
const ALCANCE: &str = "user-library-read user-library-modify";
const CUENTAS: &str = "https://accounts.spotify.com";
const API: &str = "https://api.spotify.com/v1";
/// Tiempo máximo para aceptar en el navegador.
const ESPERA_LOGIN: Duration = Duration::from_secs(5 * 60);

/// Cada inicio de sesión tiene un número; uno nuevo cancela el anterior.
#[derive(Default)]
pub struct EstadoSpotify(AtomicU64);

/// Token de acceso en memoria (dura ~1 hora), para no pedir uno nuevo en cada toque.
static ACCESO: Mutex<Option<(String, Instant)>> = Mutex::new(None);

#[derive(Serialize, Deserialize)]
struct Guardado {
    cliente: String,
    refresh: String,
}

#[derive(Clone, Serialize)]
struct FinLogin {
    ok: bool,
    mensaje: String,
}

fn cliente_http() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())
}

fn aleatorio(bytes: usize) -> Result<String, String> {
    let mut buf = vec![0u8; bytes];
    getrandom::getrandom(&mut buf).map_err(|e| format!("No pude generar un código seguro: {e}"))?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(buf))
}

fn explicar(v: &Value) -> String {
    let desc = v["error_description"].as_str().or(v["error"]["message"].as_str()).unwrap_or("");
    let codigo = v["error"].as_str().unwrap_or("");
    if codigo == "invalid_client" {
        "Spotify no reconoce ese Client ID.".into()
    } else if desc.contains("redirect") || desc.contains("INVALID_CLIENT: Invalid redirect URI") {
        format!("En tu app de Spotify falta la dirección de regreso exacta: {REGRESO}")
    } else if codigo == "invalid_grant" {
        "La sesión de Spotify venció o fue cerrada: vuelve a conectar Spotify.".into()
    } else if codigo == "access_denied" {
        "Rechazaste el permiso en Spotify.".into()
    } else {
        let corto: String = format!("{codigo} {desc}").trim().chars().take(160).collect();
        format!("Spotify respondió: {corto}")
    }
}

async fn pedir_token(formulario: &[(&str, &str)]) -> Result<Value, String> {
    let r = cliente_http()?
        .post(format!("{CUENTAS}/api/token"))
        .form(formulario)
        .send()
        .await
        .map_err(|e| format!("No pude conectarme a Spotify. ¿Hay internet? ({e})"))?;
    let ok = r.status().is_success();
    let v: Value = r.json().await.unwrap_or(Value::Null);
    if ok {
        Ok(v)
    } else {
        Err(explicar(&v))
    }
}

/// Respuesta que ve el navegador al volver de Spotify.
fn pagina(texto: &str) -> String {
    let cuerpo = format!(
        "<!doctype html><meta charset=utf-8><title>Sylvie</title>\
         <body style=\"font-family:-apple-system,sans-serif;background:#0e0e12;color:#f4f4f6;display:grid;place-items:center;height:100vh;margin:0\">\
         <p style=\"font-size:18px\">{texto}</p></body>"
    );
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cuerpo}",
        cuerpo.len()
    )
}

/// Espera (en un hilo aparte) a que Spotify vuelva a 127.0.0.1:43517 con el código.
fn esperar_regreso(app: &AppHandle, turno: u64, estado: &str) -> Result<String, String> {
    let mut oyente = None;
    for _ in 0..10 {
        match TcpListener::bind(("127.0.0.1", PUERTO)) {
            Ok(o) => {
                oyente = Some(o);
                break;
            }
            Err(_) => std::thread::sleep(Duration::from_millis(300)),
        }
    }
    let oyente = oyente.ok_or("El puerto 43517 está ocupado. Cierra otras ventanas de inicio de sesión y prueba otra vez.")?;
    oyente.set_nonblocking(true).map_err(|e| e.to_string())?;
    let inicio = Instant::now();
    loop {
        if app.state::<EstadoSpotify>().0.load(Ordering::SeqCst) != turno {
            return Err("Inicio de sesión cancelado.".into());
        }
        if inicio.elapsed() > ESPERA_LOGIN {
            return Err("Pasaron 5 minutos sin respuesta de Spotify. Prueba otra vez.".into());
        }
        let (mut conexion, _) = match oyente.accept() {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
            Err(e) => return Err(e.to_string()),
        };
        let _ = conexion.set_nonblocking(false);
        let _ = conexion.set_read_timeout(Some(Duration::from_secs(5)));
        let mut buf = [0u8; 4096];
        let n = conexion.read(&mut buf).unwrap_or(0);
        let pedido = String::from_utf8_lossy(&buf[..n]);
        // Primera línea: "GET /callback?code=...&state=... HTTP/1.1"
        let ruta = pedido.lines().next().and_then(|l| l.split_whitespace().nth(1)).unwrap_or("");
        if !ruta.starts_with("/callback") {
            let _ = conexion.write_all(pagina("…").as_bytes());
            continue; // p. ej. el navegador pidiendo /favicon.ico
        }
        let url = reqwest::Url::parse(&format!("http://127.0.0.1{ruta}")).map_err(|e| e.to_string())?;
        let valor = |clave: &str| url.query_pairs().find(|(k, _)| k == clave).map(|(_, v)| v.to_string());
        if valor("state").as_deref() != Some(estado) {
            let _ = conexion.write_all(pagina("Algo no coincide. Vuelve a Sylvie y prueba otra vez.").as_bytes());
            return Err("La respuesta de Spotify no coincide. Prueba otra vez.".into());
        }
        if let Some(error) = valor("error") {
            let _ = conexion.write_all(pagina("No se conectó Spotify. Puedes cerrar esta pestaña.").as_bytes());
            return Err(explicar(&serde_json::json!({ "error": error })));
        }
        let codigo = valor("code").ok_or("Spotify no envió el código.")?;
        let _ = conexion.write_all(pagina("¡Listo! Spotify quedó conectado a Sylvie. Ya puedes cerrar esta pestaña.").as_bytes());
        return Ok(codigo);
    }
}

/// Abre el navegador para que aceptes y espera el regreso. Avisa con "spotify-login".
#[tauri::command]
pub fn spotify_iniciar(app: AppHandle, cliente: Option<String>) -> Result<(), String> {
    // Sin ID propio: se usa el de Sylvie (claves_publicas.rs).
    let cliente = cliente
        .map(|c| c.trim().to_string())
        .filter(|c| !c.is_empty())
        .unwrap_or_else(|| crate::claves_publicas::SPOTIFY_CLIENT_ID.to_string());
    if cliente.is_empty() {
        return Err("Esta versión de Sylvie no trae su ID de Spotify: usa «Usar mi propia app».".into());
    }
    if cliente.len() != 32 || !cliente.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("El Client ID de Spotify tiene 32 letras y números (está en Settings de tu app).".into());
    }
    let verificador = aleatorio(48)?;
    let desafio = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(Sha256::digest(verificador.as_bytes()));
    let estado = aleatorio(16)?;
    let mut url = reqwest::Url::parse(&format!("{CUENTAS}/authorize")).map_err(|e| e.to_string())?;
    url.query_pairs_mut()
        .append_pair("client_id", &cliente)
        .append_pair("response_type", "code")
        .append_pair("redirect_uri", REGRESO)
        .append_pair("scope", ALCANCE)
        .append_pair("code_challenge_method", "S256")
        .append_pair("code_challenge", &desafio)
        .append_pair("state", &estado);

    let turno = app.state::<EstadoSpotify>().0.fetch_add(1, Ordering::SeqCst) + 1;
    let app2 = app.clone();
    std::thread::spawn(move || {
        let fin = |ok: bool, mensaje: String| {
            let _ = app2.emit("spotify-login", FinLogin { ok, mensaje });
        };
        let codigo = match esperar_regreso(&app2, turno, &estado) {
            Ok(c) => c,
            Err(e) => return fin(false, e),
        };
        let resultado = tauri::async_runtime::block_on(pedir_token(&[
            ("grant_type", "authorization_code"),
            ("code", codigo.as_str()),
            ("redirect_uri", REGRESO),
            ("client_id", cliente.as_str()),
            ("code_verifier", verificador.as_str()),
        ]));
        match resultado {
            Ok(v) => {
                let Some(refresh) = v["refresh_token"].as_str() else {
                    return fin(false, "Spotify no devolvió una sesión duradera.".into());
                };
                let guardado = Guardado { cliente: cliente.clone(), refresh: refresh.to_string() };
                if let Err(e) = secretos::guardar(CUENTA, &serde_json::to_string(&guardado).unwrap_or_default()) {
                    return fin(false, e);
                }
                if let Some(acceso) = v["access_token"].as_str() {
                    *ACCESO.lock().unwrap() = Some((acceso.to_string(), Instant::now() + Duration::from_secs(3000)));
                }
                let _ = app2.emit("conexiones-cambiadas", ());
                fin(true, "Spotify: el corazón ya guarda en «Tus me gusta»".into());
            }
            Err(e) => fin(false, e),
        }
    });

    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|e| format!("No pude abrir el navegador: {e}"))
}

#[tauri::command]
pub fn spotify_cancelar(app: AppHandle) {
    app.state::<EstadoSpotify>().0.fetch_add(1, Ordering::SeqCst);
}

pub fn conectado() -> Result<bool, String> {
    Ok(secretos::leer(CUENTA)?.is_some())
}

pub fn desconectar() -> Result<(), String> {
    *ACCESO.lock().unwrap() = None;
    secretos::borrar(CUENTA)
}

/// Token de acceso vigente (lo renueva con el refresh token si hace falta). None = no conectado.
async fn acceso() -> Result<Option<String>, String> {
    if let Some((token, vence)) = ACCESO.lock().unwrap().clone() {
        if Instant::now() < vence {
            return Ok(Some(token));
        }
    }
    let Some(texto) = secretos::leer(CUENTA)? else { return Ok(None) };
    let guardado: Guardado = serde_json::from_str(&texto).map_err(|_| "Vuelve a conectar Spotify.".to_string())?;
    let v = pedir_token(&[
        ("grant_type", "refresh_token"),
        ("refresh_token", guardado.refresh.as_str()),
        ("client_id", guardado.cliente.as_str()),
    ])
    .await?;
    if let Some(nuevo) = v["refresh_token"].as_str().filter(|n| *n != guardado.refresh) {
        let actualizado = Guardado { cliente: guardado.cliente, refresh: nuevo.to_string() };
        let _ = secretos::guardar(CUENTA, &serde_json::to_string(&actualizado).unwrap_or_default());
    }
    let token = v["access_token"].as_str().ok_or("Spotify no dio permiso.")?.to_string();
    let dura = v["expires_in"].as_u64().unwrap_or(3600).saturating_sub(120);
    *ACCESO.lock().unwrap() = Some((token.clone(), Instant::now() + Duration::from_secs(dura)));
    Ok(Some(token))
}

/// Llama a la API: primero la ruta nueva (/me/library, desde febrero de 2026) y, si no existe,
/// la clásica (/me/tracks).
async fn api(token: &str, metodo: reqwest::Method, rutas: [String; 2]) -> Result<Value, String> {
    let cliente = cliente_http()?;
    let mut ultimo = 0;
    for ruta in rutas {
        let r = cliente
            .request(metodo.clone(), format!("{API}{ruta}"))
            .bearer_auth(token)
            .header("Content-Length", "0")
            .send()
            .await
            .map_err(|e| format!("No pude conectarme a Spotify. ¿Hay internet? ({e})"))?;
        let codigo = r.status().as_u16();
        match codigo {
            200..=299 => return Ok(r.json().await.unwrap_or(Value::Null)),
            403 | 404 | 410 if ruta.starts_with("/me/library") => ultimo = codigo, // probar la ruta clásica
            404 | 410 => ultimo = codigo,
            401 => {
                *ACCESO.lock().unwrap() = None;
                return Err("Spotify pidió iniciar sesión otra vez: vuelve a conectar Spotify.".into());
            }
            403 => return Err("Spotify no dio permiso (¿tu app de Spotify tiene tu correo como usuario?).".into()),
            429 => return Err("Spotify pide esperar un poco.".into()),
            c => return Err(format!("Spotify respondió con un error ({c}).")),
        }
    }
    Err(format!("Spotify respondió con un error ({ultimo})."))
}

/// ¿La canción `id` está en "Tus me gusta"? Si `cambiar`, la agrega o la quita.
/// None = Spotify no está conectado en Sylvie.
pub async fn favorito(id: &str, cambiar: bool) -> Result<Option<bool>, String> {
    if id.len() > 40 || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Ok(None);
    }
    let Some(token) = acceso().await? else { return Ok(None) };
    let uri = format!("spotify%3Atrack%3A{id}");
    let respuesta = api(
        &token,
        reqwest::Method::GET,
        [format!("/me/library/contains?uris={uri}"), format!("/me/tracks/contains?ids={id}")],
    )
    .await?;
    let guardada = respuesta.as_array().and_then(|a| a.first()).and_then(|v| v.as_bool()).unwrap_or(false);
    if !cambiar {
        return Ok(Some(guardada));
    }
    let metodo = if guardada { reqwest::Method::DELETE } else { reqwest::Method::PUT };
    api(&token, metodo, [format!("/me/library?uris={uri}"), format!("/me/tracks?ids={id}")]).await?;
    Ok(Some(!guardada))
}
