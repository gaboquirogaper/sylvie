import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

// ── Tipos ─────────────────────────────────────────────────────
// Píldora: escondida → (mouse o aviso) → asomada → (clic) → expandida
type Estado = "escondida" | "asomada" | "expandida";
type Vista = "avisos" | "historial" | "pedido";
type EstadoPedido = "ninguno" | "trabajando" | "listo" | "error";

type Aviso = {
  clave: string;
  base: string;
  titulo: string;
  url: string;
  tipo: "nuevo" | "modificado";
  editado: string;
};
type Entrada = { fecha: string; pedido: string; resultado: string; ok: boolean };
type Progreso = { tipo: "paso" | "texto"; texto: string };
type Fin = { ok: boolean; resultado: string; segundos: number };

// ── Constantes (deben coincidir con tauri.conf.json y styles.css) ──
const ANCHO_VENTANA = 520;
const ZONAS: Record<Estado, { ancho: number; alto: number }> = {
  escondida: { ancho: 200, alto: 34 },
  asomada: { ancho: 320, alto: 37 },
  expandida: { ancho: 480, alto: 290 },
};
const ESPERA_SALIDA_MS = 250; // antes de esconderse al sacar el mouse
const DURACION_ANUNCIO_MS = 5000; // cuánto se asoma sola para avisar
const DURACION_FELIZ_MS = 6000; // cuánto dura la cara "feliz"
const MAX_AVISOS = 30;

// ── Elementos ─────────────────────────────────────────────────
const $ = <T extends HTMLElement>(selector: string) => document.querySelector<T>(selector)!;

const cabecera = $<HTMLElement>(".cabecera");
const mascota = $<HTMLDivElement>("#mascota");
const textoEstado = $<HTMLSpanElement>("#estado");
const pestanas = $<HTMLElement>(".pestanas");
const vistas: Record<Vista, HTMLDivElement> = {
  avisos: $<HTMLDivElement>("#vista-avisos"),
  historial: $<HTMLDivElement>("#vista-historial"),
  pedido: $<HTMLDivElement>("#vista-pedido"),
};
const listaAvisos = $<HTMLUListElement>("#lista-avisos");
const sinAvisos = $<HTMLParagraphElement>("#sin-avisos");
const listaHistorial = $<HTMLUListElement>("#lista-historial");
const sinHistorial = $<HTMLParagraphElement>("#sin-historial");
const pedidoTexto = $<HTMLParagraphElement>("#pedido-texto");
const pasos = $<HTMLOListElement>("#pasos");
const pedidoResultado = $<HTMLParagraphElement>("#pedido-resultado");
const botonCancelar = $<HTMLButtonElement>("#cancelar-pedido");
const botonListo = $<HTMLButtonElement>("#listo-pedido");
const formPedido = $<HTMLFormElement>("#form-pedido");
const campoPedido = $<HTMLInputElement>("#pedido");

// ── Estado ────────────────────────────────────────────────────
let estado: Estado = "escondida";
let vista: Vista = "avisos";
let vistaAnterior: Exclude<Vista, "pedido"> = "avisos";
let estadoPedido: EstadoPedido = "ninguno";

let temporizadorSalida: number | undefined;
let temporizadorAnuncio: number | undefined;
let temporizadorFeliz: number | undefined;
let esperarSalida = false; // tras cerrar a mano, no reasomarse hasta que el mouse salga
let mouseDentro = false;
let anunciando = false; // mientras se anuncia algo, no esconderse por falta de mouse

let hayToken = false;
let avisos: Aviso[] = [];
let sinLeer = 0;

// ── Utilidades ────────────────────────────────────────────────

function haceCuanto(iso: string): string {
  const minutos = Math.round((Date.now() - new Date(iso).getTime()) / 60000);
  if (minutos < 1) return "ahora";
  if (minutos < 60) return `hace ${minutos} min`;
  const horas = Math.round(minutos / 60);
  if (horas < 24) return `hace ${horas} h`;
  return new Date(iso).toLocaleDateString();
}

function elemento<K extends keyof HTMLElementTagNameMap>(etiqueta: K, clase: string, texto = "") {
  const el = document.createElement(etiqueta);
  el.className = clase;
  el.textContent = texto;
  return el;
}

// ── Estados de la píldora ─────────────────────────────────────

function aplicarEstado(nuevo: Estado) {
  estado = nuevo;
  document.body.dataset.estado = nuevo;

  const zona = ZONAS[nuevo];
  invoke("fijar_zona", {
    zona: { x: (ANCHO_VENTANA - zona.ancho) / 2, y: 0, ancho: zona.ancho, alto: zona.alto },
  });

  if (nuevo === "expandida") {
    if (vista === "avisos") sinLeer = 0;
    actualizarCabecera();
    invoke("enfocar");
    setTimeout(() => campoPedido.focus(), 150);
  } else {
    campoPedido.blur();
  }
}

function cambiarEstado(nuevo: Estado) {
  if (nuevo !== estado) aplicarEstado(nuevo);
}

function cerrarPanel() {
  esperarSalida = true;
  cambiarEstado("escondida");
}

/** Se asoma sola unos segundos, sin robar el foco. */
function anunciar() {
  if (estado !== "escondida") return;
  anunciando = true;
  cambiarEstado("asomada");
  window.clearTimeout(temporizadorAnuncio);
  temporizadorAnuncio = window.setTimeout(() => {
    anunciando = false;
    if (estado === "asomada" && !mouseDentro) cambiarEstado("escondida");
  }, DURACION_ANUNCIO_MS);
}

// ── Cabecera: mascota + texto de la derecha ───────────────────
// Prioridad: trabajando > resultado del pedido > avisos sin leer > normal.

function actualizarCabecera() {
  let texto = hayToken ? "hola" : "sin Notion";
  let cara = "";
  if (estadoPedido === "trabajando") {
    texto = "pensando";
    cara = "trabajando";
  } else if (estadoPedido === "listo") {
    texto = "¡listo!";
    cara = "feliz";
  } else if (estadoPedido === "error") {
    texto = "error";
    cara = "alerta";
  } else if (sinLeer > 0) {
    texto = sinLeer === 1 ? "1 aviso" : `${sinLeer} avisos`;
    cara = "con-avisos";
  }
  textoEstado.textContent = texto;
  mascota.className = cara;
}

// ── Vistas del panel ──────────────────────────────────────────

function mostrarVista(nueva: Vista) {
  vista = nueva;
  if (nueva !== "pedido") vistaAnterior = nueva;
  (Object.keys(vistas) as Vista[]).forEach((v) => (vistas[v].hidden = v !== nueva));
  pestanas.hidden = nueva === "pedido";
  pestanas.querySelectorAll<HTMLButtonElement>(".pestana").forEach((b) => {
    b.classList.toggle("activa", b.dataset.vista === nueva);
  });
  if (nueva === "avisos") {
    sinLeer = 0;
    actualizarCabecera();
  }
  if (nueva === "historial") cargarHistorial();
}

pestanas.querySelectorAll<HTMLButtonElement>(".pestana").forEach((boton) => {
  boton.addEventListener("click", () => mostrarVista(boton.dataset.vista as Vista));
});

// ── Avisos ────────────────────────────────────────────────────

function dibujarAvisos() {
  const filas = avisos.map((aviso) => {
    const li = elemento("li", `aviso ${aviso.tipo}`);
    li.title = "Abrir en Notion";
    const texto = elemento("div", "aviso-texto");
    texto.append(
      elemento("div", "aviso-titulo", aviso.titulo),
      elemento("div", "aviso-detalle", `${aviso.base} · ${aviso.tipo} · ${haceCuanto(aviso.editado)}`),
    );
    li.append(elemento("span", "punto"), texto);
    li.addEventListener("click", () => {
      invoke("abrir_en_notion", { url: aviso.url }).catch((e) => console.error(e));
    });
    return li;
  });
  listaAvisos.replaceChildren(...filas);
  sinAvisos.hidden = avisos.length > 0;
}

// ── Historial ─────────────────────────────────────────────────

async function cargarHistorial() {
  const entradas = await invoke<Entrada[]>("historial");
  const filas = entradas.map((entrada) => {
    const li = elemento("li", `aviso entrada${entrada.ok ? "" : " fallida"}`);
    li.title = "Clic para ver la respuesta completa";
    const texto = elemento("div", "aviso-texto");
    texto.append(
      elemento("div", "aviso-titulo", entrada.pedido),
      elemento("div", "aviso-detalle", `${haceCuanto(entrada.fecha)} · ${entrada.resultado}`),
    );
    li.append(elemento("span", "marca", entrada.ok ? "✓" : "✕"), texto);
    li.addEventListener("click", () => li.classList.toggle("abierta"));
    return li;
  });
  listaHistorial.replaceChildren(...filas);
  sinHistorial.hidden = entradas.length > 0;
}

// ── Pedidos a Claude Code ─────────────────────────────────────

function agregarPaso(tipo: Progreso["tipo"], texto: string) {
  pasos.append(elemento("li", tipo, texto));
  vistas.pedido.scrollTop = vistas.pedido.scrollHeight;
}

function terminarPedido(ok: boolean, resultado: string) {
  estadoPedido = ok ? "listo" : "error";
  vistas.pedido.classList.remove("trabajando-ahora");
  pedidoResultado.textContent = resultado;
  pedidoResultado.classList.toggle("error", !ok);
  pedidoResultado.hidden = false;
  botonCancelar.hidden = true;
  botonListo.hidden = false;
  campoPedido.disabled = false;
  campoPedido.placeholder = "Pídele algo a Sylvie… (Enter para enviar)";
  vistas.pedido.scrollTop = vistas.pedido.scrollHeight;
  actualizarCabecera();

  window.clearTimeout(temporizadorFeliz);
  if (ok) {
    // La cara feliz dura unos segundos; el resultado queda en pantalla hasta pulsar "Listo".
    temporizadorFeliz = window.setTimeout(() => {
      if (estadoPedido === "listo") {
        mascota.className = "";
      }
    }, DURACION_FELIZ_MS);
  }
  if (estado !== "expandida") anunciar();
}

formPedido.addEventListener("submit", async (e) => {
  e.preventDefault();
  const texto = campoPedido.value.trim();
  if (!texto || estadoPedido === "trabajando") return;

  estadoPedido = "trabajando";
  pedidoTexto.textContent = texto;
  pasos.replaceChildren();
  pedidoResultado.hidden = true;
  botonCancelar.hidden = false;
  botonListo.hidden = true;
  campoPedido.value = "";
  campoPedido.disabled = true;
  campoPedido.placeholder = "Sylvie está trabajando…";
  vistas.pedido.classList.add("trabajando-ahora");
  mostrarVista("pedido");
  actualizarCabecera();

  try {
    await invoke("enviar_pedido", { texto });
  } catch (error) {
    terminarPedido(false, String(error));
  }
});

botonCancelar.addEventListener("click", () => {
  invoke("cancelar_pedido");
  botonCancelar.disabled = true;
  setTimeout(() => (botonCancelar.disabled = false), 1500);
});

botonListo.addEventListener("click", () => {
  estadoPedido = "ninguno";
  window.clearTimeout(temporizadorFeliz);
  mostrarVista(vistaAnterior);
  actualizarCabecera();
  campoPedido.focus();
});

// ── Eventos desde Rust ────────────────────────────────────────

listen<boolean>("cursor-notch", ({ payload: dentro }) => {
  mouseDentro = dentro;
  window.clearTimeout(temporizadorSalida);
  if (!dentro) esperarSalida = false;
  if (esperarSalida) return;
  if (dentro) {
    if (estado === "escondida") cambiarEstado("asomada");
  } else if (estado === "asomada" && !anunciando) {
    temporizadorSalida = window.setTimeout(() => cambiarEstado("escondida"), ESPERA_SALIDA_MS);
  }
});

listen<Aviso[]>("avisos-nuevos", ({ payload: nuevos }) => {
  avisos = [...nuevos, ...avisos].slice(0, MAX_AVISOS);
  dibujarAvisos();
  if (estado === "expandida" && vista === "avisos") return;
  sinLeer += nuevos.length;
  actualizarCabecera();
  anunciar();
});

listen<Progreso>("claude-progreso", ({ payload }) => {
  if (estadoPedido === "trabajando") agregarPaso(payload.tipo, payload.texto);
});

listen<Fin>("claude-fin", ({ payload }) => {
  terminarPedido(payload.ok, payload.resultado);
  if (vista === "historial") cargarHistorial();
});

listen("ajustes-cambiados", async () => {
  await actualizarToken();
});

// ── Interacción general ───────────────────────────────────────

cabecera.addEventListener("click", () => {
  if (estado === "asomada") cambiarEstado("expandida");
  else if (estado === "expandida") cerrarPanel();
});

$<HTMLButtonElement>("#abrir-configuracion").addEventListener("click", () => {
  invoke("abrir_configuracion");
});

window.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && estado === "expandida") cerrarPanel();
});

getCurrentWindow().onFocusChanged(({ payload: enfocada }) => {
  if (!enfocada && estado === "expandida") cambiarEstado("escondida");
});

// ── Arranque ──────────────────────────────────────────────────

async function actualizarToken() {
  try {
    hayToken = await invoke<boolean>("hay_token");
  } catch {
    hayToken = false;
  }
  actualizarCabecera();
}

async function iniciar() {
  aplicarEstado("escondida");
  mostrarVista("avisos");
  await actualizarToken();
  avisos = await invoke<Aviso[]>("avisos_recientes");
  dibujarAvisos();
  window.setInterval(dibujarAvisos, 60_000); // refrescar "hace X min"
}

iniciar();
