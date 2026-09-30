//! Google Calendar (u otro calendario) leído desde su «Dirección secreta en formato iCal».
//! Solo lectura. La dirección vive en el Llavero; aquí solo se descarga y se interpreta.
//!
//! Soporta: eventos sueltos, de todo el día, zonas horarias (TZID), repeticiones
//! diarias / semanales / mensuales / anuales (RRULE con INTERVAL, COUNT, UNTIL, BYDAY),
//! excepciones (EXDATE) y cambios a una sola repetición (RECURRENCE-ID).

use std::{collections::HashSet, time::Duration as Espera};

use chrono::{
    DateTime, Datelike, Duration, Local, NaiveDate, NaiveDateTime, TimeZone, Utc,
};
use chrono_tz::Tz;
use serde::Serialize;

/// Hosts de videollamadas para el botón «Unirse».
pub const HOSTS_REUNION: [&str; 4] = ["meet.google.com", "zoom.us", "teams.microsoft.com", "teams.live.com"];
/// Cuántos días hacia adelante se muestran.
const DIAS_VENTANA: i64 = 7;
const MAX_EVENTOS: usize = 60;

#[derive(Clone, Serialize)]
pub struct Evento {
    pub titulo: String,
    /// Inicio y fin en formato RFC 3339 (hora local).
    pub inicio: String,
    pub fin: String,
    pub todo_el_dia: bool,
    /// Enlace de Meet / Zoom / Teams, si lo tiene.
    pub enlace: Option<String>,
    pub lugar: Option<String>,
}

// ── Descarga y verificación ─────────────────────────────────────

/// Acepta https:// o webcal:// y devuelve siempre https://.
pub fn normalizar_url(valor: &str) -> Result<String, String> {
    let v = valor.trim();
    let v = match v.strip_prefix("webcal://") {
        Some(resto) => format!("https://{resto}"),
        None => v.to_string(),
    };
    let url = reqwest::Url::parse(&v).map_err(|_| "Eso no parece una dirección web.".to_string())?;
    if url.scheme() != "https" {
        return Err("La dirección debe empezar con https://".into());
    }
    Ok(v)
}

pub async fn descargar(url: &str) -> Result<String, String> {
    let cliente = reqwest::Client::builder()
        .timeout(Espera::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    let respuesta = cliente
        .get(url)
        .send()
        .await
        .map_err(|e| format!("No pude conectarme al calendario. ¿Hay internet? ({e})"))?;
    let codigo = respuesta.status().as_u16();
    if codigo == 404 || codigo == 401 || codigo == 403 {
        return Err(
            "El calendario no respondió a esa dirección. Copia de nuevo la «Dirección secreta en formato iCal» (no la pública)."
                .into(),
        );
    }
    if !respuesta.status().is_success() {
        return Err(format!("El calendario respondió con un error ({codigo})."));
    }
    let texto = respuesta
        .text()
        .await
        .map_err(|e| format!("No pude leer el calendario: {e}"))?;
    if !texto.contains("BEGIN:VCALENDAR") {
        return Err("Esa dirección no es un calendario iCal. Usa la que termina en «basic.ics».".into());
    }
    Ok(texto)
}

/// Nombre del calendario (X-WR-CALNAME), para confirmar que es el correcto.
pub fn nombre(texto: &str) -> String {
    lineas(texto)
        .iter()
        .filter_map(|l| propiedad(l))
        .find(|p| p.nombre == "X-WR-CALNAME")
        .map(|p| desescapar(&p.valor))
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "tu calendario".into())
}

// ── Lectura del formato iCal ────────────────────────────────────

struct Prop {
    nombre: String,
    params: Vec<(String, String)>,
    valor: String,
}

impl Prop {
    fn param(&self, nombre: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| k == nombre)
            .map(|(_, v)| v.as_str())
    }
}

/// Une las líneas "dobladas" (las que empiezan con espacio continúan la anterior).
fn lineas(texto: &str) -> Vec<String> {
    let mut salida: Vec<String> = Vec::new();
    for l in texto.split('\n') {
        let l = l.strip_suffix('\r').unwrap_or(l);
        if l.starts_with(' ') || l.starts_with('\t') {
            if let Some(anterior) = salida.last_mut() {
                anterior.push_str(&l[1..]);
                continue;
            }
        }
        salida.push(l.to_string());
    }
    salida
}

fn propiedad(linea: &str) -> Option<Prop> {
    let mut entre_comillas = false;
    let mut corte = None;
    for (i, c) in linea.char_indices() {
        match c {
            '"' => entre_comillas = !entre_comillas,
            ':' if !entre_comillas => {
                corte = Some(i);
                break;
            }
            _ => {}
        }
    }
    let corte = corte?;
    let (cabeza, valor) = (&linea[..corte], &linea[corte + 1..]);
    let mut partes = cabeza.split(';');
    let nombre = partes.next()?.trim().to_ascii_uppercase();
    let params = partes
        .filter_map(|p| {
            let (k, v) = p.split_once('=')?;
            Some((k.trim().to_ascii_uppercase(), v.trim_matches('"').to_string()))
        })
        .collect();
    Some(Prop {
        nombre,
        params,
        valor: valor.to_string(),
    })
}

fn desescapar(valor: &str) -> String {
    let mut s = String::with_capacity(valor.len());
    let mut letras = valor.chars();
    while let Some(c) = letras.next() {
        if c == '\\' {
            match letras.next() {
                Some('n') | Some('N') => s.push('\n'),
                Some(otra) => s.push(otra),
                None => {}
            }
        } else {
            s.push(c);
        }
    }
    s
}

// ── Fechas ──────────────────────────────────────────────────────

#[derive(Clone, Copy)]
enum Zona {
    Utc,
    Tz(Tz),
    Local,
}

#[derive(Clone, Copy)]
struct Momento {
    naive: NaiveDateTime,
    zona: Zona,
    dia: bool,
}

impl Momento {
    fn local(&self) -> Option<DateTime<Local>> {
        a_local(self.naive, self.zona)
    }
}

fn a_local(naive: NaiveDateTime, zona: Zona) -> Option<DateTime<Local>> {
    match zona {
        Zona::Utc => Some(Utc.from_utc_datetime(&naive).with_timezone(&Local)),
        Zona::Tz(tz) => tz
            .from_local_datetime(&naive)
            .earliest()
            .map(|d| d.with_timezone(&Local)),
        Zona::Local => Local.from_local_datetime(&naive).earliest(),
    }
}

fn leer_momento(valor: &str, solo_dia: bool, tzid: Option<&str>) -> Option<Momento> {
    let v = valor.trim();
    if solo_dia || v.len() == 8 {
        let d = NaiveDate::parse_from_str(v.get(..8)?, "%Y%m%d").ok()?;
        return Some(Momento {
            naive: d.and_hms_opt(0, 0, 0)?,
            zona: Zona::Local,
            dia: true,
        });
    }
    if let Some(sin_z) = v.strip_suffix('Z') {
        return Some(Momento {
            naive: NaiveDateTime::parse_from_str(sin_z, "%Y%m%dT%H%M%S").ok()?,
            zona: Zona::Utc,
            dia: false,
        });
    }
    let naive = NaiveDateTime::parse_from_str(v, "%Y%m%dT%H%M%S").ok()?;
    // Zonas con nombre IANA (America/La_Paz…). Si no se reconoce, se usa la hora de tu Mac.
    let zona = tzid
        .and_then(|t| t.parse::<Tz>().ok())
        .map(Zona::Tz)
        .unwrap_or(Zona::Local);
    Some(Momento {
        naive,
        zona,
        dia: false,
    })
}

fn momento(p: &Prop) -> Option<Momento> {
    leer_momento(&p.valor, p.param("VALUE") == Some("DATE"), p.param("TZID"))
}

/// DURATION del estilo "PT1H30M" o "P1D".
fn duracion(valor: &str) -> Option<Duration> {
    let v = valor.trim().trim_start_matches('+');
    let (negativo, v) = match v.strip_prefix('-') {
        Some(r) => (true, r),
        None => (false, v),
    };
    let v = v.strip_prefix('P')?;
    let mut total = 0i64;
    let mut numero = String::new();
    let mut en_hora = false;
    for c in v.chars() {
        match c {
            'T' => en_hora = true,
            '0'..='9' => numero.push(c),
            _ => {
                let n: i64 = numero.parse().ok()?;
                numero.clear();
                total += n * match (c, en_hora) {
                    ('W', _) => 7 * 86_400,
                    ('D', _) => 86_400,
                    ('H', true) => 3_600,
                    ('M', true) => 60,
                    ('S', true) => 1,
                    _ => return None,
                };
            }
        }
    }
    Some(Duration::seconds(if negativo { -total } else { total }))
}

// ── Repeticiones (RRULE) ────────────────────────────────────────

struct Regla {
    frecuencia: String,
    intervalo: i64,
    cuenta: Option<u32>,
    hasta: Option<DateTime<Local>>,
    /// (ordinal, día de la semana 0 = lunes). Ordinal 0 = todos.
    dias: Vec<(i32, u32)>,
}

fn dia_semana(codigo: &str) -> Option<u32> {
    ["MO", "TU", "WE", "TH", "FR", "SA", "SU"]
        .iter()
        .position(|d| *d == codigo)
        .map(|i| i as u32)
}

fn leer_regla(valor: &str) -> Option<Regla> {
    let mut regla = Regla {
        frecuencia: String::new(),
        intervalo: 1,
        cuenta: None,
        hasta: None,
        dias: Vec::new(),
    };
    for parte in valor.split(';') {
        let (k, v) = parte.split_once('=')?;
        match k.to_ascii_uppercase().as_str() {
            "FREQ" => regla.frecuencia = v.to_ascii_uppercase(),
            "INTERVAL" => regla.intervalo = v.parse::<i64>().ok()?.max(1),
            "COUNT" => regla.cuenta = v.parse().ok(),
            "UNTIL" => {
                let m = leer_momento(v, false, None)?;
                // Si es solo fecha, vale hasta el final de ese día.
                let fin = if m.dia { m.naive + Duration::seconds(86_399) } else { m.naive };
                regla.hasta = a_local(fin, m.zona);
            }
            "BYDAY" => {
                for d in v.split(',') {
                    let d = d.trim();
                    if d.len() < 2 {
                        continue;
                    }
                    let (ordinal, codigo) = d.split_at(d.len() - 2);
                    let ordinal: i32 = if ordinal.is_empty() { 0 } else { ordinal.parse().ok()? };
                    regla.dias.push((ordinal, dia_semana(codigo)?));
                }
            }
            _ => {}
        }
    }
    if regla.frecuencia.is_empty() {
        None
    } else {
        Some(regla)
    }
}

fn dias_del_mes(anio: i32, mes: u32) -> u32 {
    let (a, m) = if mes == 12 { (anio + 1, 1) } else { (anio, mes + 1) };
    NaiveDate::from_ymd_opt(a, m, 1)
        .and_then(|d| d.pred_opt())
        .map(|d| d.day())
        .unwrap_or(28)
}

/// Fechas candidatas de un período (en orden).
fn fechas_del_periodo(inicio: NaiveDate, regla: &Regla, k: i64) -> Vec<NaiveDate> {
    let paso = k * regla.intervalo;
    match regla.frecuencia.as_str() {
        "DAILY" => vec![inicio + Duration::days(paso)],
        "WEEKLY" => {
            let lunes = inicio - Duration::days(inicio.weekday().num_days_from_monday() as i64);
            let semana = lunes + Duration::days(7 * paso);
            let mut dias: Vec<u32> = if regla.dias.is_empty() {
                vec![inicio.weekday().num_days_from_monday()]
            } else {
                regla.dias.iter().map(|(_, d)| *d).collect()
            };
            dias.sort_unstable();
            dias.dedup();
            dias.into_iter().map(|d| semana + Duration::days(d as i64)).collect()
        }
        "MONTHLY" => {
            let meses = inicio.month0() as i64 + paso;
            let anio = inicio.year() + (meses / 12) as i32;
            let mes = (meses % 12) as u32 + 1;
            if regla.dias.is_empty() {
                NaiveDate::from_ymd_opt(anio, mes, inicio.day()).into_iter().collect()
            } else {
                let total = dias_del_mes(anio, mes);
                let mut fechas: Vec<NaiveDate> = Vec::new();
                for (ordinal, dia) in &regla.dias {
                    let del_dia: Vec<NaiveDate> = (1..=total)
                        .filter_map(|n| NaiveDate::from_ymd_opt(anio, mes, n))
                        .filter(|f| f.weekday().num_days_from_monday() == *dia)
                        .collect();
                    match *ordinal {
                        0 => fechas.extend(del_dia),
                        o if o > 0 => fechas.extend(del_dia.get(o as usize - 1)),
                        o => fechas.extend(del_dia.len().checked_sub(o.unsigned_abs() as usize).and_then(|i| del_dia.get(i))),
                    }
                }
                fechas.sort_unstable();
                fechas.dedup();
                fechas
            }
        }
        "YEARLY" => NaiveDate::from_ymd_opt(inicio.year() + paso as i32, inicio.month(), inicio.day())
            .into_iter()
            .collect(),
        _ => Vec::new(),
    }
}

/// Todas las repeticiones (hora de la zona del evento) hasta `fin_ventana`.
fn repeticiones(inicio: &Momento, regla: &Regla, fin_ventana: DateTime<Local>) -> Vec<NaiveDateTime> {
    let mut salida = Vec::new();
    let mut contadas = 0u32;
    let hora = inicio.naive.time();
    for k in 0..20_000i64 {
        let fechas = fechas_del_periodo(inicio.naive.date(), regla, k);
        if fechas.is_empty() && !matches!(regla.frecuencia.as_str(), "MONTHLY" | "YEARLY") {
            break; // frecuencia no soportada
        }
        for fecha in fechas {
            if fecha < inicio.naive.date() {
                continue;
            }
            let naive = fecha.and_time(hora);
            let Some(local) = a_local(naive, inicio.zona) else { continue };
            contadas += 1;
            if regla.cuenta.is_some_and(|c| contadas > c)
                || regla.hasta.is_some_and(|h| local > h)
                || local > fin_ventana
            {
                return salida;
            }
            salida.push(naive);
        }
    }
    salida
}

// ── Enlaces de videollamada ─────────────────────────────────────

pub fn host_permitido(host: &str, lista: &[&str]) -> bool {
    lista
        .iter()
        .any(|h| host == *h || host.ends_with(&format!(".{h}")))
}

fn buscar_enlace(texto: &str) -> Option<String> {
    for (i, _) in texto.match_indices("https://") {
        let resto = &texto[i..];
        let fin = resto
            .find(|c: char| c.is_whitespace() || "<>\"'()[]{}|\\^`,;".contains(c))
            .unwrap_or(resto.len());
        let url = &resto[..fin];
        if let Ok(u) = reqwest::Url::parse(url) {
            if u.host_str().is_some_and(|h| host_permitido(h, &HOSTS_REUNION)) {
                return Some(url.to_string());
            }
        }
    }
    None
}

// ── Eventos de la semana ────────────────────────────────────────

#[derive(Default)]
struct Crudo {
    uid: String,
    titulo: String,
    inicio: Option<Momento>,
    fin: Option<Momento>,
    duracion: Option<Duration>,
    regla: Option<String>,
    excepciones: Vec<i64>,
    cambio_de: Option<i64>,
    cancelado: bool,
    lugar: String,
    textos: String,
}

fn crudos(texto: &str) -> Vec<Crudo> {
    let mut eventos = Vec::new();
    let mut actual: Option<Crudo> = None;
    for linea in lineas(texto) {
        let Some(p) = propiedad(&linea) else { continue };
        match (p.nombre.as_str(), p.valor.trim()) {
            ("BEGIN", "VEVENT") => actual = Some(Crudo::default()),
            ("END", "VEVENT") => eventos.extend(actual.take()),
            _ => {
                let Some(e) = actual.as_mut() else { continue };
                match p.nombre.as_str() {
                    "UID" => e.uid = p.valor.clone(),
                    "SUMMARY" => e.titulo = desescapar(&p.valor),
                    "DTSTART" => e.inicio = momento(&p),
                    "DTEND" => e.fin = momento(&p),
                    "DURATION" => e.duracion = duracion(&p.valor),
                    "RRULE" => e.regla = Some(p.valor.clone()),
                    "EXDATE" => {
                        let solo_dia = p.param("VALUE") == Some("DATE");
                        for v in p.valor.split(',') {
                            if let Some(l) = leer_momento(v, solo_dia, p.param("TZID")).and_then(|m| m.local()) {
                                e.excepciones.push(l.timestamp());
                            }
                        }
                    }
                    "RECURRENCE-ID" => e.cambio_de = momento(&p).and_then(|m| m.local()).map(|l| l.timestamp()),
                    "STATUS" => e.cancelado = p.valor.trim().eq_ignore_ascii_case("CANCELLED"),
                    "LOCATION" => {
                        e.lugar = desescapar(&p.valor);
                        e.textos.push_str(&e.lugar.clone());
                        e.textos.push(' ');
                    }
                    "DESCRIPTION" | "URL" | "X-GOOGLE-CONFERENCE" => {
                        e.textos.push_str(&desescapar(&p.valor));
                        e.textos.push(' ');
                    }
                    _ => {}
                }
            }
        }
    }
    eventos
}

/// Eventos desde hoy (00:00) hasta dentro de 7 días, ordenados.
pub fn eventos_semana(texto: &str) -> Vec<Evento> {
    let hoy = Local::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|n| Local.from_local_datetime(&n).earliest())
        .unwrap_or_else(Local::now);
    let fin_ventana = hoy + Duration::days(DIAS_VENTANA);

    let crudos = crudos(texto);
    // Repeticiones que fueron cambiadas o canceladas una sola vez.
    let cambiadas: HashSet<(String, i64)> = crudos
        .iter()
        .filter_map(|e| e.cambio_de.map(|t| (e.uid.clone(), t)))
        .collect();

    let mut salida: Vec<(DateTime<Local>, Evento)> = Vec::new();
    for e in &crudos {
        if e.cancelado {
            continue;
        }
        let Some(inicio) = e.inicio else { continue };
        let Some(inicio_local) = inicio.local() else { continue };
        let largo = match (e.fin.and_then(|f| f.local()), e.duracion) {
            (Some(f), _) if f > inicio_local => f - inicio_local,
            (None, Some(d)) => d,
            (None, None) if inicio.dia => Duration::days(1),
            _ => Duration::zero(),
        };
        let enlace = buscar_enlace(&e.textos);
        let titulo = if e.titulo.trim().is_empty() { "(Sin título)".to_string() } else { e.titulo.clone() };
        let lugar = Some(e.lugar.trim().to_string()).filter(|l| !l.is_empty() && !l.starts_with("https://"));

        let comienzos: Vec<DateTime<Local>> = match (e.regla.as_deref().and_then(leer_regla), e.cambio_de) {
            (Some(regla), None) => repeticiones(&inicio, &regla, fin_ventana)
                .into_iter()
                .filter_map(|n| a_local(n, inicio.zona))
                .filter(|l| !e.excepciones.contains(&l.timestamp()))
                .filter(|l| !cambiadas.contains(&(e.uid.clone(), l.timestamp())))
                .collect(),
            _ => vec![inicio_local],
        };

        for comienzo in comienzos {
            let termina = comienzo + largo;
            let dentro = if largo == Duration::zero() { comienzo >= hoy } else { termina > hoy };
            if dentro && comienzo < fin_ventana {
                salida.push((
                    comienzo,
                    Evento {
                        titulo: titulo.clone(),
                        inicio: comienzo.to_rfc3339(),
                        fin: termina.to_rfc3339(),
                        todo_el_dia: inicio.dia,
                        enlace: enlace.clone(),
                        lugar: lugar.clone(),
                    },
                ));
            }
        }
    }
    salida.sort_by_key(|(c, _)| *c);
    salida.into_iter().take(MAX_EVENTOS).map(|(_, e)| e).collect()
}
