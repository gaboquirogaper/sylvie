import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { Mascota, type EstadoMascota } from "./mascota";
import { ICONOS as ICONOS_APP, tileApp } from "./iconos";

// ══ Tipos ═══════════════════════════════════════════════════════
type Modo = "compacta" | "hover" | "abierta" | "toast" | "cancion" | "aturdida" | "comer";
type Pestana = "bienvenida" | "inicio" | "claude" | "bandeja" | "conexiones";
type Toast = null | "aviso" | "listo" | "cancion" | "aturdida" | "reunion" | "airdrop";
type Segmento = "notion" | "calendario" | "tareas" | "reservas";
type Reserva = { origen: "calendly" | "contentboard"; titulo: string; invitado: string; email: string; inicio: string; fin: string; enlace: string | null };
type Evento = { titulo: string; inicio: string; fin: string; todo_el_dia: boolean; enlace: string | null; lugar: string | null };
type Tarea = { app: "clickup" | "asana" | "trello" | "planner"; titulo: string; vence: string | null; lugar: string; url: string };
type Tareas = { tareas: Tarea[]; errores: string[] };
type EstadoApp = { id: string; conectada: boolean };
type Aviso = { clave: string; base: string; titulo: string; url: string; tipo: "nuevo" | "modificado"; editado: string };
type Entrada = { fecha: string; pedido: string; resultado: string; ok: boolean };
type Progreso = { tipo: "paso" | "texto"; texto: string };
type Fin = { ok: boolean; resultado: string; segundos: number };
type Cancion = {
  app: string;
  reproduciendo: boolean;
  titulo: string;
  artista: string;
  posicion: number;
  duracion: number;
  clave: string;
  /** false = YouTube sin el permiso del navegador: se ve, pero no se controla. */
  control: boolean;
} | null;

// ══ Medidas (la ventana mide 760 de ancho: tauri.conf.json) ═════
const ANCHO_VENTANA = 760;
// Mascota flotante: la ventana mide 760×460 (notch.rs) y la mascota ocupa 96×96 en una esquina.
const ALTO_FLOTANTE = 460;
const MARGEN_FLOTANTE = 12;
const TAM_MASCOTA = 96;
type Esquina = "abajo-der" | "abajo-izq" | "arriba-der" | "arriba-izq";
type Zona = { x: number; y: number; ancho: number; alto: number };
const TAMANOS: Record<Modo, { w: number; h: number; r: number }> = {
  compacta: { w: 304, h: 34, r: 14 },
  hover: { w: 348, h: 40, r: 18 },
  abierta: { w: 720, h: 284, r: 30 },
  toast: { w: 580, h: 128, r: 28 },
  cancion: { w: 600, h: 142, r: 28 },
  aturdida: { w: 580, h: 128, r: 28 },
  comer: { w: 520, h: 132, r: 28 },
};
const ESPERA_SALIDA_MS = 250;
const DURACION_TOAST_MS = 5000;
const DURACION_BIENVENIDA_MS = 4500;
const DURACION_FELIZ_MS = 6000;
const DORMIR_TRAS_SEG = 5 * 60; // 5 minutos sin tocar la Mac
const MAX_AVISOS = 30;
const GRACIA_BANDEJA_MS = 12_000; // con la Bandeja abierta: tiempo para ir al Finder y volver arrastrando
const AVISO_REUNION_MIN = 5; // avisar la reunión 5 minutos antes
const COLOR_RESERVA: Record<Reserva["origen"], string> = { calendly: "#4f8cff", contentboard: "#ff9f6e" };
const NOMBRE_RESERVA: Record<Reserva["origen"], string> = { calendly: "Calendly", contentboard: "contentBoard" };
const COLOR_TAREA: Record<Tarea["app"], string> = { clickup: "#c9a0ff", asana: "#ff9e9e", trello: "#5ab4f0", planner: "#b5e48c" };
const NOMBRE_TAREA: Record<Tarea["app"], string> = { clickup: "ClickUp", asana: "Asana", trello: "Trello", planner: "Planner" };

const PLATAFORMAS: Record<string, string> = { Spotify: "#1db954", "Apple Music": "#fa3d5a", YouTube: "#ff0033", "YouTube Music": "#ff0033" };
const NOTA_SVG = '<svg viewBox="0 0 24 24"><path d="M9 17.5a3 3 0 1 1-2-2.83V5l12-2v11.5a3 3 0 1 1-2-2.83V7.2L9 8.6z"></path></svg>';
const ICONOS = {
  favorito: '<svg viewBox="0 0 24 24"><path d="M12 20s-7.5-4.6-7.5-10.2A4.3 4.3 0 0 1 12 7a4.3 4.3 0 0 1 7.5 2.8C19.5 15.4 12 20 12 20z"></path></svg>',
  anterior: '<svg viewBox="0 0 24 24"><path d="M19 6v12l-9-6z"></path><rect x="5" y="6" width="2.5" height="12" rx="1"></rect></svg>',
  pausa: '<svg viewBox="0 0 24 24"><rect x="6" y="5" width="4" height="14" rx="1"></rect><rect x="14" y="5" width="4" height="14" rx="1"></rect></svg>',
  play: '<svg viewBox="0 0 24 24"><path d="M7 5v14l12-7z"></path></svg>',
  siguiente: '<svg viewBox="0 0 24 24"><path d="M5 6v12l9-6z"></path><rect x="16.5" y="6" width="2.5" height="12" rx="1"></rect></svg>',
};

const APPS = [
  { id: "notion", nombre: "Notion", letra: "N", color: "#e6e6ea" },
  { id: "claude", nombre: "Claude Code", letra: "C", color: "#f0a07a" },
  { id: "musica", nombre: "Música", letra: "♪", color: "#6fd6a0" },
  { id: "calendario", nombre: "Google Calendar", letra: "G", color: "#8fb8ff" },
  { id: "notion-calendar", nombre: "Notion Calendar", letra: "31", color: "#f2f2f5" },
  { id: "clickup", nombre: "ClickUp", letra: "U", color: "#c9a0ff" },
  { id: "asana", nombre: "Asana", letra: "A", color: "#ff9e9e" },
  { id: "trello", nombre: "Trello", letra: "T", color: "#5ab4f0" },
  { id: "planner", nombre: "Planner", letra: "P", color: "#b5e48c" },
  { id: "calendly", nombre: "Calendly", letra: "C", color: "#4f8cff" },
  { id: "contentboard", nombre: "contentBoard", letra: "cB", color: "#ff9f6e" },
  { id: "saas", nombre: "Seed Studio", letra: "S", color: "#f5b971" },
  { id: "mas", nombre: "Agregar", letra: "+", color: "#3a3b45" },
];

// ══ Elementos ══════════════════════════════════════════════════
const $ = <T extends HTMLElement>(sel: string) => document.querySelector<T>(sel)!;
const notch = $<HTMLDivElement>("#notch");
const capas: Record<Modo, HTMLElement> = {
  compacta: $("#capa-compacta"),
  hover: $("#capa-compacta"),
  abierta: $("#capa-panel"),
  toast: $("#capa-toast"),
  cancion: $("#capa-cancion"),
  aturdida: $("#capa-aturdida"),
  comer: $("#capa-comer"),
};
const vistas: Record<Pestana, HTMLElement> = {
  bienvenida: $("#v-bienvenida"),
  inicio: $("#v-inicio"),
  claude: $("#v-claude"),
  bandeja: $("#v-bandeja"),
  conexiones: $("#v-conexiones"),
};

// Una mascota por lugar donde aparece (todas muestran la misma cara).
const mascotas = {
  compacta: new Mascota($<HTMLCanvasElement>("#m-compacta")),
  panel: new Mascota($<HTMLCanvasElement>("#m-panel")),
  toast: new Mascota($<HTMLCanvasElement>("#m-toast")),
  claude: new Mascota($<HTMLCanvasElement>("#m-claude")),
  grande: new Mascota($<HTMLCanvasElement>("#m-grande")),
  aturdida: new Mascota($<HTMLCanvasElement>("#m-aturdida")),
  comer: new Mascota($<HTMLCanvasElement>("#m-comer")),
  zona: new Mascota($<HTMLCanvasElement>("#m-zona")),
  flotante: new Mascota($<HTMLCanvasElement>("#m-flotante")),
};
mascotas.aturdida.cambiar("aturdido");

// ══ Estado ═════════════════════════════════════════════════════
let modo: Modo = "compacta";
let apariencia: "notch" | "flotante" = "notch";
let esquina: Esquina = "abajo-der";
let zonaPendiente = true; // volver a mandar las zonas a Rust aunque el modo no cambie
let abierta = false;
let pestana: Pestana = "inicio";
let segmento: Segmento = "notion";
let toast: Toast = null;
let hover = false;
let mouseDentro = false;
let esperarSalida = false; // tras cerrar a mano, no reasomarse hasta que el mouse salga
let arrastrando = false;
// "encima" = el archivo está sobre el notch; "comiendo" = lo soltaste y Sylvie se lo come.
let comiendo: null | "encima" | "comiendo" = null;

let conexiones: Record<string, boolean> = {};
let eventos: Evento[] = [];
let errorCalendario = "";
let diaElegido = new Date();
let eventosVista: Evento[] = []; // los del rango que se ve (semana o mes); `eventos` = próximos 7 días (para avisos)
let vistaCal: "semana" | "mes" = "semana";
let inicioSemana = lunesDe(new Date());
let mesVisto = new Date(new Date().getFullYear(), new Date().getMonth(), 1);
let tareas: Tarea[] = [];
let erroresTareas: string[] = [];
const reunionesAvisadas = new Set<string>();
let reunionToast: Evento | null = null;
let reservas: Reserva[] = [];
type Llamada = { app: string; desde: number; puede_salir: boolean } | null;
let llamada: Llamada = null;
let erroresReservas: string[] = [];
let clavesReservas: Set<string> | null = null; // null = todavía no se cargaron (no avisar las que ya existían)
let avisos: Aviso[] = [];
let sinLeer = 0;
let cancion: Cancion = null;
let cambiosCancion: number[] = []; // para detectar el "modo DJ"
let bandeja: string[] = [];
let posicion = 0; // segundos (avanza sola entre lecturas)
let favorita: boolean | null = null; // null = la app no permite favoritos
let moviendoBarra = false;
let clavePortada = "";

let estadoPedido: "ninguno" | "trabajando" | "listo" | "error" = "ninguno";
let pasos: string[] = [];
let ultimoResultado = "";

let felizHasta = 0;
let djHasta = 0;
let dormida = false;

let tSalida: number | undefined;
let tToast: number | undefined;
let tBienvenida: number | undefined;
let tCuenta: number | undefined;

// ══ Utilidades ═════════════════════════════════════════════════
function el<K extends keyof HTMLElementTagNameMap>(tag: K, clase = "", texto = "") {
  const e = document.createElement(tag);
  if (clase) e.className = clase;
  if (texto) e.textContent = texto;
  return e;
}

function haceCuanto(iso: string): string {
  const min = Math.round((Date.now() - new Date(iso).getTime()) / 60000);
  if (min < 1) return "ahora";
  if (min < 60) return `hace ${min} min`;
  const h = Math.round(min / 60);
  return h < 24 ? `hace ${h} h` : new Date(iso).toLocaleDateString();
}

// ══ Modo del notch ═════════════════════════════════════════════
function calcularModo(): Modo {
  if (comiendo && !abierta) return "comer";
  if (abierta) return "abierta";
  if (toast === "aturdida") return "aturdida";
  if (toast === "cancion") return "cancion";
  if (toast) return "toast";
  return hover ? "hover" : "compacta";
}

function actualizar() {
  const nuevo = calcularModo();
  const cambio = nuevo !== modo;
  modo = nuevo;
  document.body.dataset.modo = modo;

  const t =
    modo === "compacta" && estadoAirDrop
      ? { w: 304, h: 44, r: 16 }
      : modo === "compacta" && llamada
        ? { w: 352, h: 34, r: 14 } // un poco más ancho para el cronómetro de la llamada
        : TAMANOS[modo];
  notch.style.width = `${t.w}px`;
  notch.style.height = `${t.h}px`;
  notch.style.borderRadius = apariencia === "flotante" ? "24px" : `0 0 ${t.r}px ${t.r}px`;

  // Mostrar solo la capa del modo actual.
  const visible = capas[modo];
  new Set(Object.values(capas)).forEach((c) => (c.hidden = c !== visible));

  // Vistas del panel
  (Object.keys(vistas) as Pestana[]).forEach((p) => (vistas[p].hidden = p !== pestana));
  document.querySelectorAll<HTMLButtonElement>(".pastilla[data-pestana]").forEach((b) => {
    b.classList.toggle("activa", abierta && b.dataset.pestana === pestana);
  });

  // Zona que acepta el mouse (el resto de la ventana deja pasar los clics).
  if (cambio || zonaPendiente) {
    zonaPendiente = false;
    invoke("fijar_zonas", { zonas: zonasActuales(t) });
  }

  // Pausar las mascotas que no se ven (ahorra batería).
  mascotas.compacta.pausar(apariencia === "flotante" || (modo !== "compacta" && modo !== "hover"));
  mascotas.flotante.pausar(apariencia !== "flotante");
  mascotas.panel.pausar(modo !== "abierta");
  mascotas.claude.pausar(!(modo === "abierta" && pestana === "claude"));
  mascotas.grande.pausar(!(modo === "abierta" && pestana === "bienvenida"));
  mascotas.toast.pausar(modo !== "toast");
  mascotas.aturdida.pausar(modo !== "aturdida");
  mascotas.comer.pausar(modo !== "comer");
  mascotas.zona.pausar(!(modo === "abierta" && comiendo));
  document.body.classList.toggle("comiendo", !!comiendo && abierta);
  $("#zona-comer").hidden = !(comiendo && abierta);

  actualizarCara();
  actualizarCaritas();
}

/** Rectángulos donde el mouse interactúa (el resto de la ventana deja pasar los clics). */
function zonasActuales(t: { w: number; h: number }): Zona[] {
  if (apariencia === "notch") return [{ x: (ANCHO_VENTANA - t.w) / 2, y: 0, ancho: t.w, alto: t.h }];
  const der = esquina.endsWith("der");
  const abajo = esquina.startsWith("abajo");
  const M = MARGEN_FLOTANTE;
  const P = TAM_MASCOTA;
  const px = der ? ANCHO_VENTANA - M - P : M;
  const py = abajo ? ALTO_FLOTANTE - M - P : M;
  const zonas: Zona[] = [{ x: px, y: py, ancho: P, alto: P }];
  // Al asomarse aparecen las caritas al lado de la mascota.
  if (modo === "hover") zonas.push({ x: der ? px - 8 - 60 : px + P + 8, y: py + P / 2 - 26, ancho: 60, alto: 52 });
  // Globo o panel: encima de la mascota (si está abajo) o debajo (si está arriba).
  if (modo !== "compacta" && modo !== "hover") {
    zonas.push({ x: der ? ANCHO_VENTANA - M - t.w : M, y: abajo ? py - 12 - t.h : py + P + 12, ancho: t.w, alto: t.h });
  }
  return zonas;
}

function fijarApariencia(nueva: string, nuevaEsquina: string) {
  apariencia = nueva === "flotante" ? "flotante" : "notch";
  esquina = (["abajo-der", "abajo-izq", "arriba-der", "arriba-izq"].includes(nuevaEsquina) ? nuevaEsquina : "abajo-der") as Esquina;
  document.body.classList.toggle("flotante", apariencia === "flotante");
  document.body.dataset.esquina = esquina;
  $("#flotante").hidden = apariencia !== "flotante";
  zonaPendiente = true;
  actualizar();
}

function abrir(p?: Pestana, enfocar = true) {
  abierta = true;
  toast = null;
  if (p) pestana = p;
  if (pestana === "inicio" && segmento === "notion") sinLeer = 0;
  actualizar();
  if (enfocar) invoke("enfocar");
  if (pestana === "claude" && enfocar) setTimeout(() => $<HTMLTextAreaElement>("#pedido").focus(), 180);
}

function cerrar(manual = false) {
  if (!abierta) return;
  abierta = false;
  if (pestana === "bienvenida") pestana = "inicio";
  if (manual) esperarSalida = true;
  actualizar();
}

function irA(p: Pestana) {
  pestana = p;
  if (p === "inicio" && segmento === "notion") sinLeer = 0;
  actualizar();
  if (p === "claude") setTimeout(() => $<HTMLTextAreaElement>("#pedido").focus(), 120);
}

function mostrarToast(tipo: Exclude<Toast, null>, ms = DURACION_TOAST_MS) {
  if (abierta && tipo !== "aturdida") return;
  toast = tipo;
  actualizar();
  window.clearTimeout(tToast);
  if (tipo === "aturdida") return;
  const vencer = () => {
    if (toast !== tipo) return;
    if (mouseDentro) tToast = window.setTimeout(vencer, 1500); // no se va mientras lo usas
    else {
      toast = null;
      actualizar();
    }
  };
  tToast = window.setTimeout(vencer, ms);
}

function bienvenida() {
  if (abierta && pestana !== "bienvenida") return; // no interrumpir si estás usando el panel
  abrir("bienvenida", false);
  window.clearTimeout(tBienvenida);
  tBienvenida = window.setTimeout(() => {
    if (abierta && pestana === "bienvenida" && !mouseDentro) cerrar();
  }, DURACION_BIENVENIDA_MS);
}

// ══ Cara de la mascota (por prioridad) ═════════════════════════
function caraActual(): EstadoMascota {
  const ahora = Date.now();
  if (toast === "aturdida") return "aturdido";
  if (comiendo === "comiendo") return "comer";
  if (comiendo === "encima" || arrastrando) return "boca";
  if (estadoPedido === "trabajando") return "pensando";
  if (djHasta > ahora) return "dj";
  if (felizHasta > ahora) return "feliz";
  if (abierta && pestana === "inicio" && segmento !== "notion") return "lupa";
  if (estadoPedido === "error" || sinLeer > 0) return "alerta";
  if (dormida) return "dormir";
  return "reposo";
}

function actualizarCara() {
  const cara = caraActual();
  mascotas.compacta.cambiar(cara);
  mascotas.flotante.cambiar(cara);
  mascotas.panel.cambiar(cara);
  mascotas.toast.cambiar(cara);
  mascotas.claude.cambiar(cara);
  mascotas.comer.cambiar(cara);
  mascotas.zona.cambiar(cara);
}

function actualizarCaritas() {
  const estados: Record<string, { on: boolean; viva: boolean }> = {
    notion: { on: sinLeer > 0, viva: sinLeer > 0 },
    claude: { on: estadoPedido === "trabajando" || felizHasta > Date.now(), viva: estadoPedido === "trabajando" },
    musica: { on: !!cancion?.reproduciendo, viva: !!cancion?.reproduciendo },
    bandeja: { on: bandeja.length > 0, viva: false },
  };
  document.querySelectorAll<HTMLSpanElement>(".carita").forEach((c) => {
    const e = estados[c.dataset.id!];
    c.classList.toggle("on", e.on);
    c.classList.toggle("viva", e.viva);
  });
}

// ══ Tres toques = aturdida ═════════════════════════════════════
let toques = 0;
let tToques: number | undefined;

function registrarToque(alUnToque?: () => void) {
  toques += 1;
  window.clearTimeout(tToques);
  if (toques >= 3) {
    toques = 0;
    aturdirse();
    return;
  }
  tToques = window.setTimeout(() => {
    const n = toques;
    toques = 0;
    if (n === 1 && alUnToque) alUnToque();
  }, 380);
}

function aturdirse() {
  abierta = false;
  mostrarToast("aturdida");
  let cuenta = 3;
  $("#cuenta").textContent = String(cuenta);
  window.clearInterval(tCuenta);
  tCuenta = window.setInterval(() => {
    cuenta -= 1;
    if (cuenta <= 0) {
      window.clearInterval(tCuenta);
      toast = null;
      actualizar();
    } else {
      $("#cuenta").textContent = String(cuenta);
    }
  }, 1000);
}

// ══ Avisos de Notion ═══════════════════════════════════════════
function dibujarAvisos() {
  const lista = $<HTMLUListElement>("#lista-avisos");
  lista.replaceChildren(
    ...avisos.map((a) => {
      const li = el("li");
      const b = el("button");
      b.type = "button";
      b.title = "Abrir en Notion";
      const punto = el("span", "punto");
      punto.style.background = a.tipo === "nuevo" ? "var(--acento)" : "#8fb8ff";
      const col = el("span", "t-col");
      col.append(el("span", "t-titulo", a.titulo), el("span", "t-mini", `${a.base} · ${a.tipo} · ${haceCuanto(a.editado)}`));
      b.append(punto, col);
      b.addEventListener("click", () => invoke("abrir_en_notion", { url: a.url }).catch(console.error));
      li.append(b);
      return li;
    }),
  );
  $("#sin-avisos").hidden = avisos.length > 0;
  dibujarResumen();
}

function dibujarResumen() {
  const hoy = eventosDelDia(new Date(), eventos).length;
  $("#resumen-derecha").textContent =
    segmento === "notion"
      ? `${avisos.length} recientes`
      : segmento === "calendario"
        ? `${hoy} hoy`
        : segmento === "tareas"
          ? `${tareas.length} pendientes`
          : `${reservas.length} próximas`;
}

// ══ Calendario ════════════════════════════════════════════════
const inicioDelDia = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate());
const mismoDia = (a: Date, b: Date) => a.toDateString() === b.toDateString();
const hora = (d: Date) => d.toLocaleTimeString("es", { hour: "2-digit", minute: "2-digit" });

function lunesDe(d: Date) {
  const x = new Date(d.getFullYear(), d.getMonth(), d.getDate());
  x.setDate(x.getDate() - ((x.getDay() + 6) % 7));
  return x;
}
const sumarDias = (d: Date, n: number) => new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);
const fechaISO = (d: Date) => `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
const esBusy = (t: string) => /^(busy|ocupad[oa]|no disponible)$/i.test(t.trim());

function eventosDelDia(d: Date, fuente: Evento[] = eventosVista.length ? eventosVista : eventos) {
  const dia = inicioDelDia(d);
  return fuente.filter((e) => {
    const i = new Date(e.inicio);
    const f = new Date(e.fin);
    if (!e.todo_el_dia) return mismoDia(i, d);
    const fin = f > i ? f : new Date(i.getTime() + 86_400_000);
    return dia >= inicioDelDia(i) && dia < fin;
  });
}

/** Mensaje de lista vacía, con un botón opcional para ir a Ajustes. */
function vacio(p: HTMLElement, texto: string, conectar = false) {
  p.replaceChildren(texto);
  if (conectar) {
    const b = el("button", "boton-sec chico", "Conectar");
    b.type = "button";
    b.addEventListener("click", () => invoke("abrir_configuracion", { seccion: "conexiones" }));
    p.append(" ", b);
  }
}

const NOMBRES_DIA = ["Lu", "Ma", "Mi", "Ju", "Vi", "Sá", "Do"];

/** Filas (semanas) que se ven a la vez en el mes; la grilla gira como una rueda. */
const FILAS_MES = 5;
/** Lunes de la primera fila visible del mes. */
let semanaArriba = lunesDe(mesVisto);

/** El mes «en foco» es el de la fila del medio. */
function mesEnFoco() {
  const d = sumarDias(semanaArriba, 7 * Math.floor(FILAS_MES / 2) + 3);
  return new Date(d.getFullYear(), d.getMonth(), 1);
}

/** Rango de eventos a traer: lo que se ve y un poco antes y después (para girar sin huecos). */
function rangoVisto(): [Date, Date] {
  if (vistaCal === "semana") return [sumarDias(inicioSemana, -7), sumarDias(inicioSemana, 14)];
  return [sumarDias(semanaArriba, -14), sumarDias(semanaArriba, 7 * (FILAS_MES + 2))];
}

function dibujarCabeceraCal() {
  const base = vistaCal === "semana" ? sumarDias(inicioSemana, 3) : mesVisto;
  const mes = base.toLocaleDateString("es", { month: "long", year: "numeric" });
  $("#cal-titulo").replaceChildren(el("span", "", mes.charAt(0).toUpperCase() + mes.slice(1)), el("i", "", vistaCal === "semana" ? "▾" : "▴"));
  $("#cal-titulo").title = vistaCal === "semana" ? "Ver el mes" : "Volver a la semana";
  $("#cal-ant").title = vistaCal === "semana" ? "Semana anterior" : "Mes anterior";
  $("#cal-sig").title = vistaCal === "semana" ? "Semana siguiente" : "Mes siguiente";
}

/** Puntitos de color por evento (máximo 3) para la semana y el mes. */
function puntitos(d: Date) {
  const del = eventosDelDia(d);
  if (!del.length) return null;
  const caja = el("span", "puntos");
  del.slice(0, 3).forEach(() => caja.append(el("i")));
  return caja;
}

/** Dibuja la semana (3 semanas en una tira: anterior, actual, siguiente) o el mes (filas de más arriba y abajo). */
function dibujarSemana() {
  const hoy = new Date();
  if (vistaCal === "mes") mesVisto = mesEnFoco();
  dibujarCabeceraCal();
  $("#semana").hidden = vistaCal !== "semana";
  $("#mes").hidden = vistaCal !== "mes";
  $("#lista-eventos").hidden = vistaCal !== "semana";
  $("#sin-eventos").hidden = vistaCal !== "semana" || $("#sin-eventos").hidden;
  const pista = el("div", "pista");
  if (vistaCal === "semana") {
    for (let i = -7; i < 14; i++) {
      const d = sumarDias(inicioSemana, i);
      const clases = ["dia", mismoDia(d, hoy) ? "hoy" : "", mismoDia(d, diaElegido) ? "elegido" : ""].join(" ").trim();
      const caja = el("button", clases);
      caja.type = "button";
      caja.append(el("small", "", mismoDia(d, hoy) ? "Hoy" : NOMBRES_DIA[(i + 7) % 7]), el("b", "", String(d.getDate())));
      const p = puntitos(d);
      if (p) caja.append(p);
      caja.addEventListener("click", () => {
        if (i < 0 || i >= 7) return; // las de los costados solo se asoman al girar
        diaElegido = d;
        dibujarSemana();
        dibujarEventos();
      });
      pista.append(caja);
    }
    $("#semana").replaceChildren(pista);
  } else {
    const nombres = el("div", "nombres");
    nombres.append(...NOMBRES_DIA.map((n) => el("small", "nombre-dia", n)));
    for (let i = -7; i < 7 * (FILAS_MES + 1); i++) {
      const d = sumarDias(semanaArriba, i);
      const fuera = d.getMonth() !== mesVisto.getMonth();
      const clases = ["celda", fuera ? "fuera" : "", mismoDia(d, hoy) ? "hoy" : "", mismoDia(d, diaElegido) ? "elegido" : ""].join(" ").trim();
      const caja = el("button", clases);
      caja.type = "button";
      caja.append(el("b", "", String(d.getDate())));
      const p = puntitos(d);
      if (p) caja.append(p);
      const del = eventosDelDia(d);
      if (del.length) caja.title = del.map((e) => (e.todo_el_dia ? e.titulo : `${hora(new Date(e.inicio))} ${e.titulo}`)).join("\n");
      caja.addEventListener("click", () => {
        diaElegido = d;
        inicioSemana = lunesDe(d);
        vistaCal = "semana";
        desplazado = 0;
        dibujarSemana();
        dibujarEventos();
        cargarVista();
      });
      pista.append(caja);
    }
    const ventana = el("div", "ventana");
    ventana.append(pista);
    $("#mes").replaceChildren(nombres, ventana);
  }
  aplicarDesplazamiento();
}

// ── Girar como una rueda ─────────────────────────────────────
// La tira sigue al dedo píxel a píxel (y a la inercia del trackpad); al soltar, se acomoda sola
// en la semana (o fila) más cercana con un rebote suave.
let desplazado = 0; // px movidos desde la posición de reposo
let eje: "x" | "y" | null = null; // el gesto se decide al empezar: de lado o vertical
let ultimoMov = 0;
let ultimoDelta = 0;
let encajando = false;
let timerEncaje = 0;
let timerFin = 0;

/** Ancho de una semana o alto de una fila del mes, en px. */
function medida() {
  if (vistaCal === "semana") return $("#semana").clientWidth || 300;
  return document.querySelector<HTMLElement>("#mes .celda")?.offsetHeight || 19;
}

function aplicarDesplazamiento(animar = false) {
  const pista = document.querySelector<HTMLElement>(vistaCal === "semana" ? "#semana .pista" : "#mes .pista");
  if (!pista) return;
  pista.style.transition = animar ? "transform 0.38s cubic-bezier(0.2, 0.9, 0.3, 1)" : "none";
  pista.style.transform =
    vistaCal === "semana" ? `translateX(calc(-100% / 3 + ${desplazado}px))` : `translateY(${desplazado - medida()}px)`;
}

/** Corre la vista una semana (o una fila) sin animación. */
function correrUno(paso: number) {
  if (vistaCal === "semana") {
    inicioSemana = sumarDias(inicioSemana, 7 * paso);
    const hoy = new Date();
    diaElegido = mismoDia(lunesDe(hoy), inicioSemana) ? hoy : sumarDias(diaElegido, 7 * paso);
  } else {
    semanaArriba = sumarDias(semanaArriba, 7 * paso);
  }
  dibujarSemana();
  dibujarEventos();
}

function terminarEncaje() {
  clearTimeout(timerFin);
  encajando = false;
  const m = medida();
  if (desplazado <= -m / 2) {
    desplazado = 0;
    correrUno(1);
  } else if (desplazado >= m / 2) {
    desplazado = 0;
    correrUno(-1);
  } else {
    desplazado = 0;
    aplicarDesplazamiento();
  }
  cargarVista();
}

/** Al soltar: se acomoda en la semana/fila más cercana (con un empujoncito según la velocidad). */
function encajar() {
  const m = medida();
  const proyectado = desplazado - ultimoDelta * 3;
  const destino = proyectado <= -m / 2 ? -m : proyectado >= m / 2 ? m : 0;
  encajando = true;
  desplazado = destino;
  aplicarDesplazamiento(true);
  timerFin = window.setTimeout(terminarEncaje, 400);
}

/** Flechas: la semana gira animada; el mes salta al mes anterior/siguiente. */
function moverCal(paso: number) {
  if (encajando) terminarEncaje();
  if (vistaCal === "semana") {
    encajando = true;
    desplazado = -paso * medida();
    aplicarDesplazamiento(true);
    timerFin = window.setTimeout(terminarEncaje, 400);
    return;
  }
  const caja = $("#mes");
  caja.style.animation = "none";
  void caja.offsetWidth; // reinicia la animación de entrada
  caja.style.animation = "";
  semanaArriba = lunesDe(new Date(mesVisto.getFullYear(), mesVisto.getMonth() + paso, 1));
  desplazado = 0;
  dibujarSemana();
  dibujarEventos();
  cargarVista();
}

$("#seg-calendario").addEventListener(
  "wheel",
  (e) => {
    const ahora = Date.now();
    if (ahora - ultimoMov > 180) eje = null; // gesto nuevo
    if (!eje) eje = Math.abs(e.deltaX) > Math.abs(e.deltaY) ? "x" : "y";
    if (vistaCal === "semana" && eje === "y") return; // en la semana, arriba/abajo mueve la lista de eventos
    e.preventDefault();
    ultimoMov = ahora;
    if (encajando) terminarEncaje();
    // En el mes las filas son bajitas: se frena un poco para que no gire demasiado rápido.
    const freno = vistaCal === "mes" ? 0.55 : 1;
    const delta = (eje === "x" ? e.deltaX : e.deltaY) * (e.deltaMode === 1 ? 16 : 1) * freno;
    ultimoDelta = delta;
    desplazado -= delta;
    const m = medida();
    while (desplazado <= -m) {
      desplazado += m;
      correrUno(1);
    }
    while (desplazado >= m) {
      desplazado -= m;
      correrUno(-1);
    }
    aplicarDesplazamiento();
    clearTimeout(timerEncaje);
    timerEncaje = window.setTimeout(encajar, 140);
  },
  { passive: false },
);
$("#cal-ant").addEventListener("click", () => moverCal(-1));
$("#cal-sig").addEventListener("click", () => moverCal(1));
$("#cal-hoy").addEventListener("click", () => {
  diaElegido = new Date();
  inicioSemana = lunesDe(diaElegido);
  semanaArriba = lunesDe(new Date(diaElegido.getFullYear(), diaElegido.getMonth(), 1));
  vistaCal = "semana";
  desplazado = 0;
  dibujarSemana();
  dibujarEventos();
  cargarVista();
});
$("#cal-titulo").addEventListener("click", () => {
  if (vistaCal === "semana") {
    vistaCal = "mes";
    semanaArriba = lunesDe(new Date(diaElegido.getFullYear(), diaElegido.getMonth(), 1));
  } else {
    vistaCal = "semana";
    inicioSemana = lunesDe(diaElegido);
  }
  desplazado = 0;
  dibujarSemana();
  dibujarEventos();
  cargarVista();
});

/** Trae los eventos del rango que se ve (el calendario descargado se reutiliza 5 min en Rust). */
let cargaVista = 0;
async function cargarVista() {
  if (!conexiones.calendario) return;
  const mia = ++cargaVista;
  const [desde, hasta] = rangoVisto();
  try {
    const lista = await invoke<Evento[]>("eventos_calendario", { desde: fechaISO(desde), hasta: fechaISO(hasta) });
    if (mia !== cargaVista) return;
    eventosVista = lista;
    errorCalendario = "";
  } catch (error) {
    errorCalendario = String(error);
  }
  dibujarSemana();
  dibujarEventos();
}

function dibujarEventos() {
  const ahora = new Date();
  const delDia = eventosDelDia(diaElegido);
  $("#lista-eventos").replaceChildren(
    ...delDia.map((e) => {
      const i = new Date(e.inicio);
      const f = new Date(e.fin);
      const enCurso = !e.todo_el_dia && i <= ahora && f > ahora;
      const pronto = !e.todo_el_dia && i > ahora && i.getTime() - ahora.getTime() < 15 * 60_000;
      const li = el("li", f <= ahora && !e.todo_el_dia ? "evento pasado" : "evento");
      li.append(el("span", enCurso ? "barrita en-curso" : "barrita"));
      const col = el("span", "t-col");
      const cuando = e.todo_el_dia ? "Todo el día" : `${hora(i)} – ${hora(f)}`;
      col.append(el("span", "t-titulo", e.titulo), el("span", "t-mini", e.lugar ? `${cuando} · ${e.lugar}` : cuando));
      li.append(col);
      if (e.enlace) {
        const b = el("button", enCurso || pronto ? "unirse ya" : "unirse", enCurso || pronto ? "Unirse" : "Enlace");
        b.type = "button";
        b.title = e.enlace;
        b.addEventListener("click", () => invoke("abrir_enlace", { url: e.enlace }).catch(console.error));
        li.append(b);
      }
      return li;
    }),
  );
  const p = $("#sin-eventos");
  // Si todos dicen «Busy», es la dirección pública del calendario (sin nombres).
  const todos = eventosVista.length ? eventosVista : eventos;
  const soloBusy = todos.length > 0 && todos.every((e) => esBusy(e.titulo));
  p.hidden = (delDia.length > 0 && !soloBusy) || vistaCal !== "semana";
  if (soloBusy) {
    vacio(p, "Solo se ve «Busy» porque conectaste la dirección pública. Cámbiala por la «Dirección secreta en formato iCal».", true);
  } else if (!conexiones.calendario) vacio(p, "Conecta Google Calendar para ver tus reuniones aquí.", true);
  else if (errorCalendario) vacio(p, errorCalendario);
  else vacio(p, mismoDia(diaElegido, ahora) ? "Nada en tu agenda hoy." : "Día libre.");
  dibujarResumen();
}

async function cargarCalendario() {
  if (!conexiones.calendario) {
    eventos = [];
    errorCalendario = "";
  } else {
    try {
      eventos = await invoke<Evento[]>("eventos_calendario");
      errorCalendario = "";
    } catch (error) {
      errorCalendario = String(error);
    }
  }
  dibujarSemana();
  dibujarEventos();
  cargarVista();
}

/** Avisa con un toast unos minutos antes de cada reunión. */
function revisarReuniones() {
  const ahora = Date.now();
  for (const e of eventos) {
    if (e.todo_el_dia) continue;
    const falta = new Date(e.inicio).getTime() - ahora;
    const clave = `${e.titulo}|${e.inicio}`;
    if (falta > 0 && falta <= AVISO_REUNION_MIN * 60_000 && !reunionesAvisadas.has(clave)) {
      reunionesAvisadas.add(clave);
      reunionToast = e;
      const min = Math.max(1, Math.round(falta / 60_000));
      $("#toast-titulo").textContent = `Tu reunión empieza en ${min} min`;
      $("#toast-cuerpo").textContent = `${e.titulo} · ${hora(new Date(e.inicio))}`;
      $("#toast-accion").textContent = e.enlace ? "Unirse" : "Ver";
      felizHasta = ahora + 3000;
      mostrarToast("reunion", 12_000);
      return;
    }
  }
}

// ══ Tareas (ClickUp y Asana) ═══════════════════════════════════
function cuandoVence(iso: string) {
  const d = new Date(iso);
  const hoy = inicioDelDia(new Date());
  const dias = Math.round((inicioDelDia(d).getTime() - hoy.getTime()) / 86_400_000);
  if (d.getTime() < Date.now()) return { texto: "vencida", tarde: true };
  if (dias === 0) return { texto: "vence hoy", tarde: false };
  if (dias === 1) return { texto: "vence mañana", tarde: false };
  if (dias < 7) return { texto: `vence el ${d.toLocaleDateString("es", { weekday: "long" })}`, tarde: false };
  return { texto: `vence el ${d.toLocaleDateString("es", { day: "numeric", month: "short" })}`, tarde: false };
}

function dibujarTareas() {
  $("#lista-tareas").replaceChildren(
    ...tareas.map((t) => {
      const li = el("li");
      const b = el("button");
      b.type = "button";
      b.title = `Abrir en ${NOMBRE_TAREA[t.app]}`;
      const punto = el("span", "punto");
      punto.style.background = COLOR_TAREA[t.app];
      const col = el("span", "t-col");
      const vence = t.vence ? cuandoVence(t.vence) : null;
      const mini = el("span", vence?.tarde ? "t-mini tarde" : "t-mini", [t.lugar, vence?.texto].filter(Boolean).join(" · ") || "sin fecha");
      col.append(el("span", "t-titulo", t.titulo), mini);
      b.append(punto, col);
      b.addEventListener("click", () => invoke("abrir_enlace", { url: t.url }).catch(console.error));
      li.append(b);
      return li;
    }),
  );
  const p = $("#sin-tareas");
  p.hidden = tareas.length > 0 && erroresTareas.length === 0;
  if (!hayTareasConectadas()) vacio(p, "Conecta ClickUp, Asana, Trello o Planner para ver tus tareas pendientes.", true);
  else if (erroresTareas.length) vacio(p, erroresTareas.join(" "));
  else vacio(p, "No tienes tareas pendientes. ¡Bien!");
  dibujarResumen();
}

const hayTareasConectadas = () => !!(conexiones.clickup || conexiones.asana || conexiones.trello || conexiones.planner);

async function cargarTareas() {
  if (!hayTareasConectadas()) {
    tareas = [];
    erroresTareas = [];
  } else {
    try {
      const r = await invoke<Tareas>("tareas_pendientes");
      tareas = r.tareas;
      erroresTareas = r.errores;
    } catch (error) {
      erroresTareas = [String(error)];
    }
  }
  dibujarTareas();
}

// ══ Reservas (Calendly y contentBoard) ═════════════════════════
const hayReservasConectadas = () => !!(conexiones.calendly || conexiones.contentboard);
const claveReserva = (r: Reserva) => `${r.origen}|${r.inicio}|${r.email || r.invitado}`;

function cuandoReserva(d: Date) {
  const hoy = inicioDelDia(new Date());
  const dias = Math.round((inicioDelDia(d).getTime() - hoy.getTime()) / 86_400_000);
  const dia = dias === 0 ? "Hoy" : dias === 1 ? "Mañana" : d.toLocaleDateString("es", { weekday: "short", day: "numeric" });
  return `${dia} · ${hora(d)}`;
}

function dibujarReservas() {
  const ahora = new Date();
  $("#lista-reservas").replaceChildren(
    ...reservas.map((r) => {
      const i = new Date(r.inicio);
      const f = new Date(r.fin || r.inicio);
      const enCurso = i <= ahora && f > ahora;
      const pronto = i > ahora && i.getTime() - ahora.getTime() < 15 * 60_000;
      const li = el("li", f <= ahora ? "evento pasado" : "evento");
      const barra = el("span", enCurso ? "barrita en-curso" : "barrita");
      barra.style.background = enCurso ? "" : COLOR_RESERVA[r.origen];
      li.append(barra);
      const col = el("span", "t-col");
      col.append(
        el("span", "t-titulo", r.invitado || r.titulo),
        el("span", "t-mini", [cuandoReserva(i), r.invitado ? r.titulo : "", NOMBRE_RESERVA[r.origen]].filter(Boolean).join(" · ")),
      );
      col.title = r.email;
      li.append(col);
      if (r.enlace) {
        const b = el("button", enCurso || pronto ? "unirse ya" : "unirse", enCurso || pronto ? "Unirse" : "Enlace");
        b.type = "button";
        b.addEventListener("click", () => invoke("abrir_enlace", { url: r.enlace }).catch(console.error));
        li.append(b);
      }
      return li;
    }),
  );
  const p = $("#sin-reservas");
  p.hidden = reservas.length > 0 && erroresReservas.length === 0;
  if (!hayReservasConectadas()) vacio(p, "Conecta Calendly o contentBoard para ver tus llamadas reservadas.", true);
  else if (erroresReservas.length) vacio(p, erroresReservas.join(" "));
  else vacio(p, "No hay llamadas reservadas en los próximos 14 días.");
  dibujarResumen();
}

async function cargarReservas(forzar = false) {
  if (!hayReservasConectadas()) {
    reservas = [];
    erroresReservas = [];
    clavesReservas = null;
  } else {
    try {
      const r = await invoke<{ reservas: Reserva[]; errores: string[] }>("reservas", { forzar });
      reservas = r.reservas;
      erroresReservas = r.errores;
      avisarReservasNuevas();
    } catch (error) {
      erroresReservas = [String(error)];
    }
  }
  dibujarReservas();
}

/** Toast cuando alguien reserva una llamada nueva (desde la última revisión). */
function avisarReservasNuevas() {
  const claves = new Set(reservas.map(claveReserva));
  const antes = clavesReservas;
  clavesReservas = claves;
  if (!antes) return; // primera carga: no avisar lo que ya estaba
  const nuevas = reservas.filter((r) => !antes.has(claveReserva(r)) && new Date(r.inicio) > new Date());
  if (!nuevas.length) return;
  const r = nuevas[0];
  $("#toast-titulo").textContent = nuevas.length === 1 ? "Nueva llamada reservada" : `${nuevas.length} llamadas nuevas reservadas`;
  $("#toast-cuerpo").textContent = `${r.invitado || r.titulo} · ${cuandoReserva(new Date(r.inicio))}`;
  $("#toast-accion").textContent = "Ver";
  felizHasta = Date.now() + 4000;
  segmentoPendiente = "reservas";
  mostrarToast("aviso");
}
let segmentoPendiente: Segmento | null = null;
function elegirReservas() {
  irA("inicio");
  elegirSegmento("reservas");
}

function elegirSegmento(s: Segmento) {
  segmento = s;
  document.querySelectorAll<HTMLButtonElement>(".segmentos button").forEach((b) => b.classList.toggle("activo", b.dataset.seg === s));
  $("#seg-notion").hidden = s !== "notion";
  $("#seg-calendario").hidden = s !== "calendario";
  $("#seg-tareas").hidden = s !== "tareas";
  $("#seg-reservas").hidden = s !== "reservas";
  if (s === "reservas") dibujarReservas();
  if (s === "notion") sinLeer = 0;
  if (s === "calendario") {
    diaElegido = new Date();
    inicioSemana = lunesDe(diaElegido);
    vistaCal = "semana";
    desplazado = 0;
    dibujarSemana();
    dibujarEventos();
    cargarVista();
  }
  dibujarAvisos();
  actualizar();
}

// ══ Música ═════════════════════════════════════════════════════
function crearControles() {
  document.querySelectorAll<HTMLSpanElement>(".controles").forEach((grupo) => {
    (["favorito", "anterior", "alternar", "siguiente"] as const).forEach((accion) => {
      const b = el("button", accion === "alternar" || accion === "favorito" ? accion : "");
      b.type = "button";
      b.dataset.accion = accion;
      b.setAttribute("aria-label", { favorito: "Guardar en favoritos", anterior: "Anterior", alternar: "Reproducir o pausar", siguiente: "Siguiente" }[accion]);
      b.innerHTML = accion === "alternar" ? ICONOS.pausa : ICONOS[accion];
      b.addEventListener("click", async (e) => {
        e.stopPropagation();
        if (!cancion) return;
        if (accion === "favorito") {
          if (favorita === null && cancion.app === "Spotify") {
            invoke("abrir_configuracion", { seccion: "conexiones" }); // falta conectar Spotify
            return;
          }
          favorita = await invoke<boolean | null>("favorito_musica", { appMusica: cancion.app, cambiar: true }).catch(() => favorita);
          dibujarMusica();
          return;
        }
        try {
          await invoke("controlar_musica", { appMusica: cancion.app, accion });
          recibirCancion(await invoke<Cancion>("musica_actual"));
        } catch (error) {
          console.error(error);
        }
      });
      grupo.append(b);
    });
  });
}

function dibujarMusica() {
  const titulo = cancion?.titulo ?? "Nada sonando";
  const sinControl = !!cancion && cancion.control === false;
  const artista = !cancion
    ? "Abre Spotify, Música o YouTube"
    : sinControl
      ? `${cancion.app} · sin control (ver Ajustes → Música)`
      : [cancion.artista, cancion.app].filter(Boolean).join(" · ");
  $("#inicio-titulo").textContent = titulo;
  $("#inicio-artista").textContent = artista;
  $("#cancion-titulo").textContent = titulo;
  $("#cancion-artista").textContent = artista;
  document.querySelectorAll<HTMLSpanElement>(".insignia").forEach((i) => {
    if (cancion && PLATAFORMAS[cancion.app]) {
      i.style.background = PLATAFORMAS[cancion.app];
      i.title = cancion.app;
      i.innerHTML = NOTA_SVG;
    } else {
      i.innerHTML = "";
    }
  });
  document.querySelectorAll<HTMLButtonElement>(".controles .alternar").forEach((b) => {
    b.innerHTML = cancion?.reproduciendo ? ICONOS.pausa : ICONOS.play;
  });
  document.querySelectorAll<HTMLButtonElement>(".controles .favorito").forEach((b) => {
    b.classList.toggle("on", favorita === true);
    const spotifySinConectar = favorita === null && cancion?.app === "Spotify";
    b.disabled = favorita === null && !spotifySinConectar;
    b.classList.toggle("apagado", spotifySinConectar);
    b.title = spotifySinConectar
      ? "Conecta Spotify en Ajustes → Conexiones para guardar en «Tus me gusta»"
      : favorita === null
        ? "No disponible para esta app"
        : "Guardar en favoritos";
  });
  document.querySelectorAll<HTMLSpanElement>(".controles").forEach((g) => (g.style.visibility = cancion ? "visible" : "hidden"));
  document.querySelectorAll<HTMLButtonElement>(".controles button:not(.favorito)").forEach((b) => (b.disabled = sinControl));
  dibujarProgreso();
}

function reloj(seg: number) {
  const s = Math.max(0, Math.round(seg));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

function dibujarProgreso() {
  const dur = cancion?.duracion || 0;
  const pct = dur > 0 ? Math.min(100, (posicion / dur) * 100) : 0;
  for (const id of ["inicio", "cancion"]) {
    const barra = $<HTMLInputElement>(`#${id}-barra`);
    if (!moviendoBarra) barra.value = String(pct);
    const p = moviendoBarra ? Number(barra.value) : pct;
    barra.style.background = `linear-gradient(to right, #f4f4f6 ${p}%, #2a2b33 ${p}%)`;
    barra.disabled = !cancion || cancion.control === false;
    $(`#${id}-actual`).textContent = reloj(moviendoBarra ? (p / 100) * dur : posicion);
    $(`#${id}-total`).textContent = reloj(dur);
  }
}

const dormir = (ms: number) => new Promise<void>((r) => window.setTimeout(r, ms));

/** Espera a que el navegador tenga la imagen lista (así no parpadea al cambiarla). */
function precargar(url: string) {
  return new Promise<void>((listo) => {
    const img = new Image();
    img.onload = img.onerror = () => listo();
    img.src = url;
  });
}

let ultimaClaveConPortada = ""; // de qué canción es la portada que se ve ahora
let cargaPortada = 0; // cada carga tiene su número; solo la última puede tocar la portada

/**
 * Pide la portada de la canción `clave`. Mientras llega, la anterior se oscurece un poco
 * (nunca se transparenta ni deja ver la carátula de Sylvie). Si Sylvie todavía no se enteró
 * de que cambiaste de canción (pasaste varias muy rápido), pregunta qué suena y empieza de nuevo.
 */
async function cargarPortada(clave: string) {
  const mia = ++cargaPortada;
  const vigente = () => mia === cargaPortada && clave === clavePortada;
  const imgs = [$<HTMLImageElement>("#inicio-img"), $<HTMLImageElement>("#cancion-img")];
  const terminar = (url: string | null) => {
    if (mia !== cargaPortada) return;
    document.body.classList.remove("portada-cargando");
    imgs.forEach((i) => {
      if (url) i.src = url;
      i.hidden = !url; // sin portada (p. ej. un podcast): se ve la carátula de Sylvie
    });
    ultimaClaveConPortada = url ? clave : "";
  };
  if (!clave) return terminar(null);

  // Si cambió de app (p. ej. de Spotify a YouTube), la portada anterior no sirve ni atenuada.
  const appDe = (c: string) => c.split("|")[0];
  if (appDe(clave) !== appDe(ultimaClaveConPortada)) imgs.forEach((i) => (i.hidden = true));
  document.body.classList.add("portada-cargando");
  for (const espera of [0, 700, 1500, 3000]) {
    if (espera) await dormir(espera);
    if (!vigente()) return;
    const url = await invoke<string | null>("portada_musica", { clave }).catch(() => null);
    if (!vigente()) return;
    if (url) {
      await precargar(url);
      if (vigente()) terminar(url);
      return;
    }
    // ¿Sigue sonando la misma? Si no, esto dispara una carga nueva con la canción real.
    const ahora = await invoke<Cancion>("musica_actual").catch(() => null);
    if (!vigente()) return;
    if ((ahora?.clave || "") !== clave) return recibirCancion(ahora);
  }
  terminar(null);
}

// La barra de progreso se puede arrastrar: al soltar, mueve la canción.
for (const id of ["inicio", "cancion"]) {
  const barra = $<HTMLInputElement>(`#${id}-barra`);
  barra.addEventListener("input", () => {
    moviendoBarra = true;
    dibujarProgreso();
  });
  barra.addEventListener("change", async () => {
    if (!cancion) return;
    posicion = (Number(barra.value) / 100) * cancion.duracion;
    moviendoBarra = false;
    dibujarProgreso();
    await invoke("mover_musica", { appMusica: cancion.app, segundos: posicion }).catch(console.error);
  });
  barra.addEventListener("click", (e) => e.stopPropagation());
}

function recibirCancion(nueva: Cancion) {
  const anterior = cancion;
  cancion = nueva;
  if (nueva) posicion = nueva.posicion;
  let portadaLista: Promise<void> = Promise.resolve();
  if ((nueva?.clave || "") !== clavePortada) {
    clavePortada = nueva?.clave || "";
    favorita = null;
    portadaLista = cargarPortada(clavePortada);
    if (nueva) {
      invoke<boolean | null>("favorito_musica", { appMusica: nueva.app, cambiar: false })
        .then((f) => {
          favorita = f;
          dibujarMusica();
        })
        .catch(() => {});
    }
  }
  dibujarMusica();
  if (nueva && anterior && anterior.titulo !== nueva.titulo) {
    // Modo DJ: 3 cambios de canción en menos de 20 segundos.
    const ahora = Date.now();
    cambiosCancion = [...cambiosCancion.filter((t) => ahora - t < 20000), ahora];
    if (cambiosCancion.length >= 3) djHasta = ahora + 8000;
    // El aviso de canción espera la portada nueva (máximo 1,5 s) para no mostrar la anterior.
    if (nueva.reproduciendo && !abierta) {
      Promise.race([portadaLista, dormir(1500)]).then(() => {
        if (cancion?.clave === nueva.clave && !abierta) mostrarToast("cancion");
      });
    }
  }
  actualizar();
}

// ══ Pedidos a Claude ═══════════════════════════════════════════
function dibujarPasos() {
  const trabajando = estadoPedido === "trabajando";
  $("#pasos").replaceChildren(
    ...pasos.map((texto, i) => {
      const ultimo = i === pasos.length - 1;
      const clase = trabajando && ultimo ? "activo" : "hecho";
      const li = el("li", clase);
      li.append(el("span", "ico"), el("span", "", texto.replace(/…$/, "")));
      return li;
    }),
  );
}

async function cargarHistorial() {
  const entradas = await invoke<Entrada[]>("historial");
  $("#historial").replaceChildren(
    ...entradas.slice(0, 4).map((e) => {
      const li = el("li", e.ok ? "" : "fallo");
      li.title = e.resultado;
      li.append(el("b", "", e.ok ? "✓" : "✕"), el("span", "", `${e.pedido} · ${haceCuanto(e.fecha)}`));
      return li;
    }),
  );
}

function prepararConversacion(pedido: string) {
  const burbuja = $("#pedido-burbuja");
  burbuja.textContent = pedido;
  burbuja.hidden = false;
  $("#comentario").hidden = true;
  $("#resultado").hidden = true;
  $("#bienvenida-chat").hidden = true;
}

// ══ Adjuntos para Claude (archivos arrastrados o de la bandeja, y texto largo pegado) ══
const MAX_ADJUNTOS = 5;
const LARGO_PEGADO = 1500; // al pegar más que esto, se adjunta como archivo de texto
let adjuntos: string[] = [];
let pegado: string | null = null;

function agregarAdjuntos(rutas: string[]) {
  const nuevos = rutas.filter((r) => !adjuntos.includes(r));
  adjuntos = [...adjuntos, ...nuevos].slice(0, MAX_ADJUNTOS);
  if (adjuntos.length + nuevos.length > MAX_ADJUNTOS) mostrarNota(`Máximo ${MAX_ADJUNTOS} archivos por pedido.`);
  dibujarAdjuntos();
}

function mostrarNota(texto: string) {
  const c = $("#comentario");
  c.textContent = texto;
  c.hidden = false;
}

function chip(texto: string, quitar: () => void) {
  const c = el("span", "chip-adjunto");
  c.append(el("span", "", texto));
  const x = el("button", "", "×");
  x.type = "button";
  x.setAttribute("aria-label", `Quitar ${texto}`);
  x.addEventListener("click", () => {
    quitar();
    dibujarAdjuntos();
  });
  c.append(x);
  return c;
}

function dibujarAdjuntos() {
  const caja = $("#adjuntos");
  const chips = adjuntos.map((r) => chip(`📎 ${nombreArchivo(r)}`, () => (adjuntos = adjuntos.filter((x) => x !== r))));
  if (pegado) chips.push(chip(`📝 Texto pegado (${pegado.length.toLocaleString("es")} caracteres)`, () => (pegado = null)));
  caja.replaceChildren(...chips);
  caja.hidden = chips.length === 0;
  $("#adjuntar").classList.toggle("activo", chips.length > 0);
}

function dibujarMenuAdjuntar() {
  const menu = $("#menu-adjuntar");
  if (!bandeja.length) {
    menu.replaceChildren(el("span", "t-mini", "Arrastra archivos a esta pestaña o a la Bandeja para adjuntarlos."));
    return;
  }
  menu.replaceChildren(
    el("span", "t-mini", "De la bandeja:"),
    ...bandeja.map((r) => {
      const b = el("button", adjuntos.includes(r) ? "elegido" : "", nombreArchivo(r));
      b.type = "button";
      b.addEventListener("click", () => {
        if (adjuntos.includes(r)) adjuntos = adjuntos.filter((x) => x !== r);
        else agregarAdjuntos([r]);
        dibujarAdjuntos();
        dibujarMenuAdjuntar();
      });
      return b;
    }),
  );
}

$("#adjuntar").addEventListener("click", () => {
  const menu = $("#menu-adjuntar");
  menu.hidden = !menu.hidden;
  if (!menu.hidden) dibujarMenuAdjuntar();
});

$<HTMLTextAreaElement>("#pedido").addEventListener("paste", (e) => {
  const texto = e.clipboardData?.getData("text/plain") ?? "";
  if (texto.length <= LARGO_PEGADO) return;
  e.preventDefault(); // un texto muy largo no entra cómodo en la cajita: va como adjunto
  pegado = texto;
  dibujarAdjuntos();
});

async function enviarPedido() {
  const campo = $<HTMLTextAreaElement>("#pedido");
  const texto = campo.value.trim();
  const hayAdjuntos = adjuntos.length > 0 || !!pegado;
  if ((!texto && !hayAdjuntos) || estadoPedido === "trabajando") return;
  const conAdjuntos = { adjuntos: [...adjuntos], pegado };
  adjuntos = [];
  pegado = null;
  $("#menu-adjuntar").hidden = true;
  dibujarAdjuntos();
  estadoPedido = "trabajando";
  pasos = [];
  campo.value = "";
  campo.disabled = true;
  $<HTMLButtonElement>("#enviar-pedido").disabled = true;
  $("#cancelar-pedido").hidden = false;
  const n = conAdjuntos.adjuntos.length + (conAdjuntos.pegado ? 1 : 0);
  prepararConversacion([texto || "Revisa lo que te adjunto.", n ? `📎 ${n} adjunto${n === 1 ? "" : "s"}` : ""].filter(Boolean).join("\n"));
  dibujarPasos();
  actualizar();
  try {
    await invoke("enviar_pedido", { texto, ...conAdjuntos });
  } catch (error) {
    terminarPedido(false, String(error));
  }
}

function terminarPedido(ok: boolean, resultado: string) {
  estadoPedido = ok ? "listo" : "error";
  ultimoResultado = resultado;
  if (ok) felizHasta = Date.now() + DURACION_FELIZ_MS;
  const r = $("#resultado");
  r.textContent = resultado;
  r.classList.toggle("error", !ok);
  r.hidden = false;
  $("#comentario").hidden = true;
  const campo = $<HTMLTextAreaElement>("#pedido");
  campo.disabled = false;
  $<HTMLButtonElement>("#enviar-pedido").disabled = false;
  $("#cancelar-pedido").hidden = true;
  dibujarPasos();
  cargarHistorial();
  if (!abierta) mostrarToast("listo", 6000);
  actualizar();
}

// ══ Bandeja y AirDrop ══════════════════════════════════════════
function nombreArchivo(ruta: string) {
  return ruta.split(/[\\/]/).pop() || ruta;
}

function dibujarBandeja() {
  $("#lista-bandeja").replaceChildren(
    ...bandeja.map((ruta) => {
      const caja = el("div", "archivo");
      const nombre = nombreArchivo(ruta);
      const icono = el("button", "icono-archivo", (nombre.split(".").pop() || "").toUpperCase().slice(0, 4));
      icono.title = "Mostrar en Finder";
      icono.addEventListener("click", () => invoke("mostrar_en_finder", { ruta }).catch((e) => ($("#mensaje-bandeja").textContent = String(e))));
      const quitar = el("button", "quitar", "×");
      quitar.title = "Quitar de la bandeja (no borra el archivo)";
      quitar.addEventListener("click", () => {
        bandeja = bandeja.filter((r) => r !== ruta);
        dibujarBandeja();
      });
      caja.append(icono, el("span", "nombre", nombre), quitar);
      return caja;
    }),
  );
  $("#sin-bandeja").hidden = bandeja.length > 0;
  $("#acciones-bandeja").hidden = bandeja.length === 0;
  const contador = $("#contador-bandeja");
  contador.textContent = String(bandeja.length);
  contador.hidden = bandeja.length === 0;
  $("#texto-airdrop").textContent = bandeja.length ? `Enviar ${bandeja.length} ${bandeja.length === 1 ? "archivo" : "archivos"}` : "Sin archivos";
  actualizarCaritas();
}

// ══ Conexiones ═════════════════════════════════════════════════
function dibujarConexiones() {
  $("#apps").replaceChildren(
    ...APPS.map((a) => {
      const tarjeta = el("div", "app");
      const cabeza = el("div", "cabeza");
      const letra = tileApp(a.id, a.color);
      cabeza.append(letra, el("span", "nombre", a.nombre));
      const boton = el("button");
      boton.type = "button";
      const ajustes = (seccion: string) => () => invoke("abrir_configuracion", { seccion });
      const marcar = (ok: boolean, alConectado: () => void, seccion = "conexiones") => {
        boton.textContent = ok ? "Conectada ✓" : "Conectar";
        boton.className = ok ? "conectada" : "conectar";
        boton.addEventListener("click", ok ? alConectado : ajustes(seccion));
      };
      if (a.id === "notion") marcar(!!conexiones.notion, ajustes("notion"), "notion");
      else if (a.id === "notion-calendar") {
        // Notion Calendar no tiene API: usa tus calendarios de Google (la conexión de Google Calendar).
        marcar(!!conexiones.calendario, () => invoke("abrir_app", { cual: "notion-calendar" }));
        tarjeta.title = "Muestra los eventos de tu Google Calendar; el botón abre Notion Calendar";
      } else if (a.id === "contentboard") {
        marcar(!!conexiones.contentboard, () => elegirReservas());
      } else if (a.id in conexiones) {
        marcar(!!conexiones[a.id], () => invoke("abrir_app", { cual: a.id }));
      } else if (a.id === "claude") {
        boton.textContent = "Elegir apps";
        boton.className = "conectada";
        boton.addEventListener("click", ajustes("claude"));
      } else if (a.id === "musica") {
        boton.textContent = "En tu Mac ✓";
        boton.className = "conectada";
        boton.addEventListener("click", () => invoke("abrir_app", { cual: "musica" }));
        tarjeta.title = "Spotify, Apple Music, YouTube y YouTube Music";
      } else if (a.id === "mas") {
        boton.textContent = "Ver";
        boton.className = "conectada";
        boton.addEventListener("click", ajustes("claude"));
      } else {
        boton.textContent = "Pronto";
        boton.disabled = true;
      }
      tarjeta.append(cabeza, boton);
      return tarjeta;
    }),
  );
}

async function actualizarConexiones() {
  try {
    const lista = await invoke<EstadoApp[]>("estado_conexiones");
    conexiones = Object.fromEntries(lista.map((a) => [a.id, a.conectada]));
  } catch {
    conexiones = {};
  }
  dibujarConexiones();
  cargarCalendario();
  cargarTareas();
  cargarReservas();
  // Si se acaba de conectar Spotify, el corazón ya sirve.
  if (cancion) {
    invoke<boolean | null>("favorito_musica", { appMusica: cancion.app, cambiar: false })
      .then((f) => {
        favorita = f;
        dibujarMusica();
      })
      .catch(() => {});
  }
}

// ══ Caritas-botón del notch compacto ═══════════════════════════
function accionCarita(id: string) {
  if (id === "notion") invoke("abrir_app", { cual: "notion" }).catch(console.error);
  else if (id === "musica") invoke("abrir_app", { cual: "musica" }).catch(console.error);
  else if (id === "claude") abrir("claude");
  else if (id === "bandeja") abrir("bandeja");
}

// ══ Eventos de la interfaz ═════════════════════════════════════
$("#capa-compacta").addEventListener("click", () => registrarToque(() => abrir()));
document.querySelectorAll<HTMLButtonElement>(".carita").forEach((c) => {
  c.innerHTML = ICONOS_APP[c.dataset.id!] ?? "";
  c.addEventListener("click", (e) => {
    e.stopPropagation(); // no cuenta como toque a Sylvie
    accionCarita(c.dataset.id!);
  });
});
$("#m-panel-btn").addEventListener("click", () => registrarToque());
$("#cerrar").addEventListener("click", () => cerrar(true));
$("#capa-toast").addEventListener("click", () => {
  if (toast === "listo") abrir("claude");
  else if (toast === "airdrop") abrir("bandeja");
  else if (toast === "reunion" && reunionToast?.enlace) {
    invoke("abrir_enlace", { url: reunionToast.enlace }).catch(console.error);
    toast = null;
    actualizar();
  } else if (toast === "reunion") {
    abrir("inicio");
    elegirSegmento("calendario");
  } else {
    abrir("inicio");
    elegirSegmento(segmentoPendiente ?? "notion"); // el aviso de Notion o el de una reserva nueva
    segmentoPendiente = null;
  }
});
$("#cancion-portada").addEventListener("click", () => abrir("inicio"));

document.querySelectorAll<HTMLButtonElement>(".pastilla[data-pestana]").forEach((b) => {
  b.addEventListener("click", () => irA(b.dataset.pestana as Pestana));
});
document.querySelectorAll<HTMLButtonElement>(".segmentos button").forEach((b) => {
  b.addEventListener("click", () => {
    const s = b.dataset.seg as Segmento;
    // Tocar «Reservas» estando ya en Reservas = actualizar ahora (también contentBoard).
    if (s === "reservas" && segmento === "reservas") {
      $("#sin-reservas").hidden = false;
      vacio($("#sin-reservas"), "Actualizando…");
      cargarReservas(true);
    }
    elegirSegmento(s);
  });
});

$("#form-pedido").addEventListener("submit", (e) => {
  e.preventDefault();
  enviarPedido();
});
$<HTMLTextAreaElement>("#pedido").addEventListener("keydown", (e) => {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    enviarPedido();
  }
});
$("#cancelar-pedido").addEventListener("click", () => invoke("cancelar_pedido"));

// ══ En llamada (Zoom, Teams, Google Meet) ══════════════════════
function fijarLlamada(nueva: Llamada) {
  const cambia = !!nueva !== !!llamada;
  llamada = nueva;
  document.body.classList.toggle("en-llamada-activa", !!llamada);
  document.querySelectorAll<HTMLElement>(".llamada-app").forEach((e) => (e.textContent = llamada?.app ?? ""));
  document.querySelectorAll<HTMLElement>(".en-llamada .salir").forEach((e) => (e.hidden = !llamada?.puede_salir));
  document.querySelectorAll<HTMLElement>(".en-llamada").forEach((e) => (e.title = llamada ? `Volver a ${llamada.app}` : ""));
  dibujarTiempoLlamada();
  if (cambia) {
    zonaPendiente = true;
    actualizar();
  }
}

function dibujarTiempoLlamada() {
  if (!llamada) return;
  const seg = Math.max(0, Math.floor((Date.now() - llamada.desde) / 1000));
  const h = Math.floor(seg / 3600);
  const texto = h ? `${h}:${String(Math.floor((seg % 3600) / 60)).padStart(2, "0")}:${String(seg % 60).padStart(2, "0")}` : reloj(seg);
  document.querySelectorAll<HTMLElement>(".llamada-tiempo").forEach((e) => (e.textContent = texto));
}

document.querySelectorAll<HTMLElement>(".en-llamada").forEach((b) => {
  b.addEventListener("click", (e) => {
    e.stopPropagation();
    const salir = (e.target as HTMLElement).closest("[data-llamada='salir']");
    invoke(salir ? "llamada_salir" : "llamada_volver").catch((error) => {
      $("#toast-titulo").textContent = "Llamada";
      $("#toast-cuerpo").textContent = String(error);
      $("#toast-accion").textContent = "";
      mostrarToast("aviso", 3500);
    });
  });
});
listen<Llamada>("llamada", ({ payload }) => fijarLlamada(payload));

// ══ AirDrop: barrita delgada en el notch mientras se envía ═════
let estadoAirDrop: null | "enviando" | "enviado" = null;
let tAirDrop: number | undefined;

function fijarAirDrop(estado: typeof estadoAirDrop) {
  estadoAirDrop = estado;
  document.body.classList.toggle("airdrop-enviando", estado === "enviando");
  document.body.classList.toggle("airdrop-enviado", estado === "enviado");
  window.clearTimeout(tAirDrop);
  if (estado === "enviado") tAirDrop = window.setTimeout(() => fijarAirDrop(null), 2500);
  zonaPendiente = true; // el notch cambia de alto
  actualizar();
}

listen<{ estado: "enviado" | "cancelado" | "error"; mensaje: string }>("airdrop", ({ payload }) => {
  $("#mensaje-bandeja").textContent = payload.mensaje;
  if (payload.estado === "cancelado") return fijarAirDrop(null);
  fijarAirDrop(payload.estado === "enviado" ? "enviado" : null);
  if (payload.estado === "enviado") felizHasta = Date.now() + 4000;
  $("#toast-titulo").textContent = payload.estado === "enviado" ? "¡Listo!" : "AirDrop";
  $("#toast-cuerpo").textContent = payload.mensaje;
  $("#toast-accion").textContent = "Ver";
  mostrarToast("airdrop");
});

$("#enviar-airdrop").addEventListener("click", async () => {
  if (!bandeja.length) return;
  try {
    await invoke("enviar_por_airdrop", { rutas: bandeja });
    $("#mensaje-bandeja").textContent = "Elige a quién enviarlo en la ventana de AirDrop…";
    fijarAirDrop("enviando");
    cerrar(); // se achica y queda la barrita mientras se envía
  } catch (error) {
    $("#mensaje-bandeja").textContent = String(error);
  }
});
$("#vaciar-bandeja").addEventListener("click", () => {
  bandeja = [];
  $("#mensaje-bandeja").textContent = "";
  dibujarBandeja();
});
$("#abrir-ajustes").addEventListener("click", () => invoke("abrir_configuracion", { seccion: "conexiones" }));

window.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && abierta) cerrar(true);
});

// Al hacer clic fuera, el panel se cierra… salvo en la Bandeja y en Claude: ahí esperamos un rato,
// para que puedas ir al Finder, agarrar un archivo y volver arrastrándolo.
let tGracia: number | undefined;
function cerrarTrasGracia() {
  window.clearTimeout(tGracia);
  tGracia = window.setTimeout(() => {
    if (!abierta || (pestana !== "bandeja" && pestana !== "claude")) return;
    if (mouseDentro || arrastrando || comiendo) cerrarTrasGracia();
    else cerrar();
  }, GRACIA_BANDEJA_MS);
}

getCurrentWindow().onFocusChanged(({ payload: enfocada }) => {
  window.clearTimeout(tGracia);
  if (enfocada || !abierta || arrastrando || comiendo || pestana === "bienvenida") return;
  if (pestana === "bandeja" || pestana === "claude") cerrarTrasGracia();
  else cerrar();
});

// Arrastrar archivos sobre el notch → Sylvie abre la boca y se los "come" hacia la Bandeja.
function etiquetaArchivos(rutas: string[]) {
  if (rutas.length > 1) return String(rutas.length);
  return (nombreArchivo(rutas[0] || "").split(".").pop() || "").toUpperCase().slice(0, 4) || "DOC";
}

function textosComer(titulo: string, cuerpo: string) {
  $("#comer-titulo").textContent = titulo;
  $("#comer-cuerpo").textContent = cuerpo;
  $("#zona-titulo").textContent = titulo;
  $("#zona-cuerpo").textContent = cuerpo;
}

function claseVolador(clase: "tiembla" | "tragado", etiqueta?: string) {
  for (const v of [$("#volador"), $("#volador-zona")]) {
    if (etiqueta) v.textContent = etiqueta;
    v.classList.remove("tiembla", "tragado");
    void v.offsetWidth; // reinicia la animación
    v.classList.add(clase);
  }
}

/** ¿El archivo que se arrastra va a Claude (pestaña Claude abierta) en vez de a la bandeja? */
let arrastreParaClaude = false;

getCurrentWebview().onDragDropEvent(({ payload }) => {
  if (payload.type === "enter" && abierta && pestana === "claude" && estadoPedido !== "trabajando") {
    arrastreParaClaude = true;
    arrastrando = true;
    document.body.classList.add("arrastrando");
    actualizar();
    return;
  }
  if (arrastreParaClaude && (payload.type === "leave" || payload.type === "drop")) {
    arrastreParaClaude = false;
    arrastrando = false;
    document.body.classList.remove("arrastrando");
    if (payload.type === "drop") {
      agregarAdjuntos(payload.paths);
      felizHasta = Date.now() + 2500;
      $<HTMLTextAreaElement>("#pedido").focus();
    }
    actualizar();
    return;
  }
  if (payload.type === "enter") {
    arrastrando = true;
    comiendo = "encima";
    document.body.classList.add("arrastrando");
    claseVolador("tiembla", etiquetaArchivos(payload.paths));
    textosComer("¡Dámelo!", abierta ? "Suéltalo aquí y lo guardo en la bandeja." : "Suéltalo y lo guardo en la bandeja.");
    toast = null;
    if (abierta) pestana = "bandeja"; // con el panel abierto: no se cierra, salta a Bandeja
    actualizar();
  } else if (payload.type === "leave") {
    if (comiendo === "comiendo") return;
    arrastrando = false;
    comiendo = null;
    document.body.classList.remove("arrastrando");
    actualizar();
  } else if (payload.type === "drop") {
    arrastrando = false;
    document.body.classList.remove("arrastrando");
    const nuevos = payload.paths.filter((r) => !bandeja.includes(r));
    const quien = payload.paths.length === 1 ? `«${nombreArchivo(payload.paths[0])}»` : `${payload.paths.length} archivos`;
    comiendo = "comiendo";
    textosComer("¡Ñam, ñam, ñam!", `${quien} ya ${payload.paths.length === 1 ? "está" : "están"} en la bandeja.`);
    claseVolador("tragado");
    actualizar();
    window.setTimeout(() => {
      comiendo = null;
      bandeja = [...bandeja, ...nuevos];
      felizHasta = Date.now() + 3000;
      dibujarBandeja();
      abrir("bandeja"); // con foco: un clic fuera vuelve a cerrar
    }, 1500);
  }
});

// ══ Eventos desde Rust ═════════════════════════════════════════
listen<boolean>("cursor-notch", ({ payload: dentro }) => {
  mouseDentro = dentro;
  window.clearTimeout(tSalida);
  if (dentro) dormida = false;
  if (!dentro) esperarSalida = false;
  if (esperarSalida) return;
  if (dentro) {
    if (!hover) {
      hover = true;
      actualizar();
    }
  } else {
    tSalida = window.setTimeout(() => {
      hover = false;
      actualizar();
    }, ESPERA_SALIDA_MS);
  }
});

listen<Aviso[]>("avisos-nuevos", ({ payload: nuevos }) => {
  avisos = [...nuevos, ...avisos].slice(0, MAX_AVISOS);
  dibujarAvisos();
  if (abierta && pestana === "inicio" && segmento === "notion") return;
  sinLeer += nuevos.length;
  const primero = nuevos[0];
  segmentoPendiente = "notion";
  $("#toast-titulo").textContent = nuevos.length === 1 ? `Nuevo en ${primero.base}` : `${nuevos.length} novedades en Notion`;
  $("#toast-cuerpo").textContent = `«${primero.titulo}» · ${primero.tipo} · ahora`;
  $("#toast-accion").textContent = "Ver";
  mostrarToast("aviso");
});

listen<Progreso>("claude-progreso", ({ payload }) => {
  if (estadoPedido !== "trabajando") return;
  if (payload.tipo === "paso") {
    pasos.push(payload.texto);
    dibujarPasos();
  } else {
    const c = $("#comentario");
    c.textContent = payload.texto;
    c.hidden = false;
  }
});

listen<Fin>("claude-fin", ({ payload }) => {
  terminarPedido(payload.ok, payload.resultado);
  if (!abierta) {
    $("#toast-titulo").textContent = payload.ok ? "¡Listo!" : "Algo salió mal";
    $("#toast-cuerpo").textContent = ultimoResultado;
    $("#toast-accion").textContent = "Ver";
  }
});

listen<Cancion>("musica", ({ payload }) => recibirCancion(payload));
listen("ajustes-cambiados", actualizarConexiones);
listen("conexiones-cambiadas", actualizarConexiones);
// ══ Mascota flotante: clic = abrir/cerrar, arrastrar = moverla de esquina ══
let presion: { x: number; y: number } | null = null;
let moviendoMascota = false;
let tSoltar: number | undefined;
const botonMascota = $("#mascota-flotante");

botonMascota.addEventListener("mousedown", (e) => {
  if (e.button === 0) presion = { x: e.screenX, y: e.screenY };
});
window.addEventListener("mouseup", () => (presion = null));
window.addEventListener("mousemove", (e) => {
  if (!presion || moviendoMascota) return;
  if (Math.hypot(e.screenX - presion.x, e.screenY - presion.y) < 5) return;
  presion = null;
  moviendoMascota = true;
  if (abierta) cerrar();
  toast = null;
  actualizar();
  getCurrentWindow().startDragging().catch(() => (moviendoMascota = false));
});
getCurrentWindow().onMoved(() => {
  if (!moviendoMascota) return;
  window.clearTimeout(tSoltar);
  // Cuando deja de moverse un momento, se pega a la esquina más cercana.
  tSoltar = window.setTimeout(() => {
    moviendoMascota = false;
    invoke("soltar_mascota").catch(console.error);
  }, 400);
});
botonMascota.addEventListener("click", () => {
  if (moviendoMascota) return;
  registrarToque(() => (abierta ? cerrar(true) : abrir()));
});

listen<{ modo: string; esquina: string }>("apariencia", ({ payload }) => fijarApariencia(payload.modo, payload.esquina));

listen("desbloqueo", () => {
  dormida = false;
  bienvenida();
});

// ══ Dormir: revisa cada 15 s cuánto llevas sin usar la Mac ═════
async function revisarInactividad() {
  try {
    const seg = await invoke<number>("segundos_inactivo");
    const antes = dormida;
    dormida = seg >= DORMIR_TRAS_SEG;
    if (antes !== dormida) actualizarCara();
  } catch {
    /* sin dato: no pasa nada */
  }
}

// ══ Arranque ═══════════════════════════════════════════════════
async function iniciar() {
  invoke<Llamada>("llamada_actual").then(fijarLlamada).catch(() => {});
  try {
    const a = await invoke<{ apariencia?: string; esquina?: string }>("leer_ajustes");
    fijarApariencia(a.apariencia ?? "notch", a.esquina ?? "abajo-der");
  } catch {
    /* se queda en modo notch */
  }
  crearControles();
  elegirSegmento("notion");
  dibujarBandeja();
  await actualizarConexiones();
  avisos = await invoke<Aviso[]>("avisos_recientes");
  dibujarAvisos();
  cargarHistorial();
  recibirCancion(await invoke<Cancion>("musica_actual"));
  mascotas.grande.cambiar("reposo");
  actualizar();
  bienvenida();

  window.setInterval(() => {
    actualizarCara(); // expira "feliz" y "dj" a su tiempo
    dibujarTiempoLlamada();
    actualizarCaritas();
    if (cancion?.reproduciendo && !moviendoBarra) {
      posicion = Math.min(cancion.duracion, posicion + 1);
      dibujarProgreso();
    }
  }, 1000);
  window.setInterval(() => {
    dibujarAvisos();
    if (segmento === "calendario") dibujarEventos(); // "en curso" / "Unirse"
  }, 60_000);
  window.setInterval(revisarReuniones, 30_000);
  window.setInterval(cargarCalendario, 10 * 60_000);
  window.setInterval(cargarTareas, 5 * 60_000);
  window.setInterval(() => cargarReservas(), 5 * 60_000); // contentBoard se consulta como mucho cada 30 min (Rust)
  window.setInterval(revisarInactividad, 15_000);
}

iniciar();
