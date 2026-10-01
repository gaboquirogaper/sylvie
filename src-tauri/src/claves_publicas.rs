//! IDs PÚBLICOS de las apps de Sylvie en Spotify y en Microsoft.
//!
//! No son secretos (por eso pueden ir en el código, como hacen todas las apps de escritorio):
//! con PKCE / inicio con código no hay "client secret". Gracias a esto, quien usa Sylvie solo
//! pulsa «Conectar» y acepta en el navegador, sin crear nada.
//!
//! Los registra una sola vez quien publica Sylvie (Gabo):
//! - Spotify: developer.spotify.com/dashboard → Create app → Redirect URI
//!   http://127.0.0.1:43517/callback → Web API → copiar el Client ID.
//! - Microsoft: entra.microsoft.com → Registros de aplicaciones → Nuevo registro →
//!   "Cuentas en cualquier directorio organizativo" → Autenticación: "Permitir flujos de
//!   clientes públicos" = Sí → copiar el Id. de aplicación (cliente).
//!
//! Si quedan vacíos, Ajustes muestra la opción de usar una app propia.

pub const SPOTIFY_CLIENT_ID: &str = "";
pub const MICROSOFT_CLIENT_ID: &str = "";
