import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

// ── Estados de la píldora ─────────────────────────────────────
// escondida → (mouse encima) → asomada → (clic) → expandida
// asomada   → (mouse se va)  → escondida
// expandida → (Esc, clic en la cabecera o clic fuera) → escondida
type Estado = "escondida" | "asomada" | "expandida";

// Debe coincidir con "width" de la ventana en tauri.conf.json
const ANCHO_VENTANA = 520;

// Tamaño de la zona que acepta el mouse en cada estado (igual que en styles.css)
const ZONAS: Record<Estado, { ancho: number; alto: number }> = {
  escondida: { ancho: 200, alto: 34 },
  asomada: { ancho: 320, alto: 37 },
  expandida: { ancho: 480, alto: 200 },
};

// Espera antes de esconderse al sacar el mouse (evita parpadeos)
const ESPERA_SALIDA_MS = 250;

const cabecera = document.querySelector<HTMLElement>(".cabecera")!;
const textoEstado = document.querySelector<HTMLSpanElement>("#estado")!;
const pedido = document.querySelector<HTMLInputElement>("#pedido")!;

let estado: Estado = "escondida";
let temporizadorSalida: number | undefined;
// Tras cerrar el panel a mano (Esc o clic), el cursor suele seguir sobre el notch.
// Sin esto, la píldora se volvería a asomar al instante. Esperamos a que el mouse
// salga una vez antes de volver a reaccionar.
let esperarSalida = false;

function aplicarEstado(nuevo: Estado) {
  estado = nuevo;
  document.body.dataset.estado = nuevo;

  const zona = ZONAS[nuevo];
  invoke("fijar_zona", {
    zona: { x: (ANCHO_VENTANA - zona.ancho) / 2, y: 0, ancho: zona.ancho, alto: zona.alto },
  });

  if (nuevo === "expandida") {
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

// Rust avisa cuando el cursor entra (true) o sale (false) de la zona.
listen<boolean>("cursor-notch", ({ payload: dentro }) => {
  window.clearTimeout(temporizadorSalida);
  if (!dentro) esperarSalida = false;
  if (esperarSalida) return;
  if (dentro) {
    if (estado === "escondida") cambiarEstado("asomada");
  } else if (estado === "asomada") {
    temporizadorSalida = window.setTimeout(() => cambiarEstado("escondida"), ESPERA_SALIDA_MS);
  }
});

// Clic en la cabecera: asomada → expandida, expandida → escondida.
cabecera.addEventListener("click", () => {
  if (estado === "asomada") cambiarEstado("expandida");
  else if (estado === "expandida") cerrarPanel();
});

// Botón "Configuración" del panel (alternativa al menú de la barra).
document.querySelector<HTMLButtonElement>("#abrir-configuracion")!.addEventListener("click", () => {
  invoke("abrir_configuracion");
});

// Esc cierra el panel.
window.addEventListener("keydown", (e) => {
  if (e.key === "Escape" && estado === "expandida") cerrarPanel();
});

// Clic en otra app (la ventana pierde el foco) cierra el panel.
getCurrentWindow().onFocusChanged(({ payload: enfocada }) => {
  if (!enfocada && estado === "expandida") cambiarEstado("escondida");
});

// Texto de la derecha: "hola" si Notion está conectado; si no, un recordatorio.
async function actualizarTexto() {
  try {
    const hayToken = await invoke<boolean>("hay_token");
    textoEstado.textContent = hayToken ? "hola" : "sin Notion";
  } catch {
    textoEstado.textContent = "sin Notion";
  }
}
listen("ajustes-cambiados", actualizarTexto);
actualizarTexto();

// Arranque: dejar a Rust sincronizado con el estado inicial.
aplicarEstado("escondida");
