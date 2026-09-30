//! Avisos de Notion: revisa las bases elegidas cada X minutos y avisa al notch.
//!
//! - La primera vez que se vigila una base solo se marca el punto de partida
//!   (no te llena de avisos con todo lo antiguo).
//! - Luego avisa de elementos nuevos o modificados desde la última revisión.
//! - "Revisar ahora" (o cambiar la configuración) despierta la revisión al instante.

use std::{
    collections::{HashMap, HashSet},
    fs,
    sync::Mutex,
    time::Duration,
};

use chrono::{DateTime, Duration as Lapso, Utc};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Notify;

use crate::{configuracion, notion, secretos};

const MAX_RECIENTES: usize = 30;
/// Notion redondea las horas de edición al minuto: revisamos con este margen y descartamos repetidos.
const MARGEN_MINUTOS: i64 = 2;
const ARCHIVO_MARCAS: &str = "avisos.json";

/// Un aviso que se muestra en el notch.
#[derive(Clone, Serialize)]
pub struct Aviso {
    /// Identificador único del aviso (página + momento de edición).
    pub clave: String,
    pub base: String,
    pub titulo: String,
    pub url: String,
    /// "nuevo" o "modificado"
    pub tipo: String,
    /// Fecha de edición en formato ISO.
    pub editado: String,
}

/// Hasta cuándo se revisó cada base (se guarda en disco para sobrevivir reinicios).
#[derive(Default, Serialize, Deserialize)]
struct Marcas {
    ultima_revision: HashMap<String, DateTime<Utc>>,
}

#[derive(Default)]
pub struct EstadoAvisos {
    recientes: Mutex<Vec<Aviso>>,
    vistos: Mutex<HashSet<String>>,
    despertar: Notify,
}

impl EstadoAvisos {
    /// Pide una revisión inmediata.
    pub fn revisar_ya(&self) {
        self.despertar.notify_one();
    }
}

fn leer_marcas(app: &AppHandle) -> Marcas {
    configuracion::ruta(app, ARCHIVO_MARCAS)
        .ok()
        .and_then(|ruta| fs::read_to_string(ruta).ok())
        .and_then(|texto| serde_json::from_str(&texto).ok())
        .unwrap_or_default()
}

fn guardar_marcas(app: &AppHandle, marcas: &Marcas) {
    if let (Ok(ruta), Ok(texto)) = (
        configuracion::ruta(app, ARCHIVO_MARCAS),
        serde_json::to_string_pretty(marcas),
    ) {
        let _ = fs::write(ruta, texto);
    }
}

/// Una vuelta de revisión por todas las bases vigiladas.
async fn revisar(app: &AppHandle) -> Result<(), String> {
    let ajustes = configuracion::leer(app);
    if ajustes.bases.is_empty() {
        return Ok(());
    }
    let Some(token) = secretos::leer_token_notion()? else {
        return Ok(());
    };

    let mut marcas = leer_marcas(app);
    // Olvidar bases que ya no se vigilan (si vuelves a marcarlas, empiezan de cero).
    marcas
        .ultima_revision
        .retain(|id, _| ajustes.bases.iter().any(|b| &b.id == id));

    let mut nuevos: Vec<Aviso> = Vec::new();
    let mut errores: Vec<String> = Vec::new();

    for base in &ajustes.bases {
        let ahora = Utc::now();
        let Some(desde) = marcas.ultima_revision.get(&base.id).copied() else {
            // Primera vez: solo marcar el punto de partida.
            marcas.ultima_revision.insert(base.id.clone(), ahora);
            continue;
        };
        let con_margen = desde - Lapso::minutes(MARGEN_MINUTOS);

        match notion::paginas_editadas_desde(&token, base, con_margen).await {
            Ok(paginas) => {
                let estado = app.state::<EstadoAvisos>();
                let mut vistos = estado.vistos.lock().unwrap();
                for p in paginas {
                    let clave = format!("{}@{}", p.id, p.editada.timestamp());
                    if !vistos.insert(clave.clone()) {
                        continue; // ya avisado
                    }
                    let tipo = if p.creada >= con_margen { "nuevo" } else { "modificado" };
                    nuevos.push(Aviso {
                        clave,
                        base: base.titulo.clone(),
                        titulo: p.titulo,
                        url: p.url,
                        tipo: tipo.into(),
                        editado: p.editada.to_rfc3339(),
                    });
                }
                if vistos.len() > 5000 {
                    vistos.clear();
                }
                marcas.ultima_revision.insert(base.id.clone(), ahora);
            }
            Err(e) => errores.push(e),
        }
    }

    guardar_marcas(app, &marcas);

    if !nuevos.is_empty() {
        {
            let estado = app.state::<EstadoAvisos>();
            let mut recientes = estado.recientes.lock().unwrap();
            for aviso in nuevos.iter().rev() {
                recientes.insert(0, aviso.clone());
            }
            recientes.truncate(MAX_RECIENTES);
        }
        let _ = app.emit("avisos-nuevos", &nuevos);
    }

    if errores.is_empty() {
        Ok(())
    } else {
        Err(errores.join("\n"))
    }
}

/// Arranca el ciclo de revisión en segundo plano.
pub fn iniciar(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        // Dar unos segundos a que la interfaz cargue.
        tokio::time::sleep(Duration::from_secs(3)).await;
        loop {
            match revisar(&app).await {
                Ok(()) => {
                    let _ = app.emit("avisos-estado", Option::<String>::None);
                }
                Err(e) => {
                    eprintln!("[avisos] {e}");
                    let _ = app.emit("avisos-estado", Some(e));
                }
            }

            let minutos = u64::from(configuracion::leer(&app).intervalo_minutos.max(1));
            let estado = app.state::<EstadoAvisos>();
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(minutos * 60)) => {}
                _ = estado.despertar.notified() => {}
            }
        }
    });
}

// ── Comandos que usa la interfaz ────────────────────────────────

/// Los últimos avisos (para cuando se recarga la interfaz del notch).
#[tauri::command]
pub fn avisos_recientes(estado: tauri::State<EstadoAvisos>) -> Vec<Aviso> {
    estado.recientes.lock().unwrap().clone()
}

#[tauri::command]
pub fn revisar_ahora(estado: tauri::State<EstadoAvisos>) {
    estado.revisar_ya();
}

/// Abre un elemento en Notion (navegador o app de Notion, según tu configuración de Notion).
#[tauri::command]
pub fn abrir_en_notion(app: AppHandle, url: String) -> Result<(), String> {
    // Solo enlaces de Notion: la interfaz no puede usar esto para abrir otra cosa.
    if !(url.starts_with("https://www.notion.so/") || url.starts_with("https://notion.so/")) {
        return Err("Ese enlace no es de Notion.".into());
    }
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| format!("No pude abrir Notion: {e}"))
}
