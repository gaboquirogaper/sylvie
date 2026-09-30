import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

// ── Estados de la píldora ─────────────────────────────────────
// escondida → (mouse encima o aviso nuevo) → asomada → (clic) → expandida
// asomada   → (mouse se va / termina el anuncio) → escondida
// expandida → (Esc, clic en la cabecera o clic fuera) → escondida
type Estado = "escondida" | "asomada" | "expandida";

type Aviso = {
  clave: string;
  base: string;
  titulo: string;
  url: string;
  tipo: "nuevo" | "modificado";
  editado: string;
};

// Debe coincidir con "width" de la ventana en tauri.conf.json
const ANCHO_VENTANA = 520;

// Tamaño de la zona que acepta el mouse en cada estado (igual que en styles.css)
const ZONAS: Record<Estado, { ancho: number; alto: number }> = {
  escondida: { ancho: 200, alto: 34 },
  asomada: { ancho: 320, alto: 37 },
  expandida: { ancho: 480, alto: 272 },
};

const ESPERA_SALIDA_MS = 250; // antes de esconderse al sacar el mouse
const DURACION_ANUNCIO_MS = 5000; // cuánto se asoma sola al llegar un aviso
const MAX_AVISOS = 30;

const cabecera = document.querySelector<HTMLElement>(".cabecera")!;
const mascota = document.querySelector<HTMLDivElement>("#mascota")!;
const textoEstado = document.querySelector<HTMLSpanElement>("#estado")!;
const listaAvisos = document.querySelector<HTMLUListElement>("#lista-avisos")!;
const sinAvisos = document.querySelector<HTMLParagraphElement>("#sin-avisos")!;
const pedido = document.querySelector<HTMLInputElement>("#pedido")!;

let estado: Estado = "escondida";
let temporizadorSalida: number | undefined;
let temporizadorAnuncio: number | undefined;
// Tras cerrar el panel a mano, no reasomarse hasta que el mouse salga una vez.
let esperarSalida = false;
let mouseDentro = false;
// Mientras se anuncia un aviso, la píldora no se esconde al no haber mouse.
let anunciando = false;

let hayToken = false;
let avisos: Aviso[] = [];
let sinLeer = 0;

// ── Estados ───────────────────────────────────────────────────

function aplicarEstado(nuevo: Estado) {
  estado = nuevo;
  document.body.dataset.estado = nuevo;

  const zona = ZONAS[nuevo];
  invoke("fijar_zona", {
    zona: { x: (ANCHO_VENTANA - zona.ancho) / 2, y: 0, ancho: zona.ancho, alto: zona.alto },
  });

  if (nuevo === "expandida") {
    marcarLeidos();
    invoke("enfocar");
    setTimeout(() => pedido.focus(), 150);
  } else {
    pedido.blur();
  }
}

function cambiarEstado(nuevo: Estado) {
  if (nuevo !== estado) aplicarEstado(nuevo);
}

/** Cierre manual desde el panel: se esconde y no se reasoma hasta que el mouse salga. */
function cerrarPanel() {
  esperarSalida = true;
  cambiarEstado("escondida");
}

/** Se asoma sola unos segundos para avisar, sin robar el foco. */
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

// ── Texto de la derecha y lista de avisos ─────────────────────

function actualizarTexto() {
  if (sinLeer > 0) {
    textoEstado.textContent = sinLeer === 1 ? "1 aviso" : `${sinLeer} avisos`;
  } else {
    textoEstado.textContent = hayToken ? "hola" : "sin Notion";
  }
  mascota.classList.toggle("con-avisos", sinLeer > 0);
}

function marcarLeidos() {
  sinLeer = 0;
  actualizarTexto();
}

function haceCuanto(iso: string): string {
  const minutos = Math.round((Date.now() - new Date(iso).getTime()) / 60000);
  if (minutos < 1) return "ahora";
  if (minutos < 60) return `hace ${minutos} min`;
  const horas = Math.round(minutos / 60);
  if (horas < 24) return `hace ${horas} h`;
  return new Date(iso).toLocaleDateString();
}

function dibujarAvisos() {
  const filas = avisos.map((aviso) => {
    const li = document.createElement("li");
    li.className = `aviso ${aviso.tipo}`;
    li.title = "Abrir en Notion";

    const punto = document.createElement("span");
    punto.className = "punto";

    const texto = document.createElement("div");
    texto.className = "aviso-texto";
    const titulo = document.createElement("div");
    titulo.className = "aviso-titulo";
    titulo.textContent = aviso.titulo;
    const detalle = document.createElement("div");
    detalle.className = "aviso-detalle";
    detalle.textContent = `${aviso.base} · ${aviso.tipo} · ${haceCuanto(aviso.editado)}`;
    texto.append(titulo, detalle);

    li.append(punto, texto);
    li.addEventListener("click", () => {
      invoke("abrir_en_notion", { url: aviso.url }).catch((e) => console.error(e));
    });
    return li;
  });
  listaAvisos.replaceChildren(...filas);
  sinAvisos.hidden = avisos.length > 0;
}

async function actualizarToken() {
  try {
    hayToken = await invoke<boolean>("hay_token");
  } catch {
    hayToken = false;
  }
  actualizarTexto();
}

// ── Eventos desde Rust ────────────────────────────────────────

// El cursor entra (true) o sale (false) de la zona de la píldora.
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

// Llegaron avisos nuevos de Notion.
listen<Aviso[]>("avisos-nuevos", ({ payload: nuevos }) => {
  avisos = [...nuevos, ...avisos].slice(0, MAX_AVISOS);
  dibujarAvisos();
  if (estado === "expandida") return;
  sinLeer += nuevos.length;
  actualizarTexto();
  anunciar();
});

listen("ajustes-cambiados", actualizarToken);

// ── Interacción ───────────────────────────────────────────────

cabecera.addEventListener("click", () => {
  if (estado === "asomada") cambiarEstado("expandida");
  else if (estado === "expandida") cerrarPanel();
});

document.querySelector<HTMLButtonElement>("#abrir-configuracion")!.addEventListener("click", () => {
  invoke("abrir_configuracion");
});

window.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && estado === "expandida") cerrarPanel();
});

getCurrentWindow().onFocusChanged(({ payload: enfocada }) => {
  if (!enfocada && estado === "expandida") cambiarEstado("escondida");
});

// ── Arranque ──────────────────────────────────────────────────

async function iniciar() {
  aplicarEstado("escondida");
  await actualizarToken();
  avisos = await invoke<Aviso[]>("avisos_recientes");
  dibujarAvisos();
  // Refrescar "hace X min" cada minuto.
  window.setInterval(dibujarAvisos, 60_000);
}

iniciar();
