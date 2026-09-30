import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type BaseDatos = { id: string; titulo: string };
type Ajustes = { intervalo_minutos: number; bases: BaseDatos[] };

const $ = <T extends HTMLElement>(selector: string) => document.querySelector<T>(selector)!;

// Notion
const conToken = $<HTMLDivElement>("#con-token");
const formToken = $<HTMLFormElement>("#form-token");
const campoToken = $<HTMLInputElement>("#token");
const botonProbar = $<HTMLButtonElement>("#probar");
const botonCambiar = $<HTMLButtonElement>("#cambiar");
const botonBorrar = $<HTMLButtonElement>("#borrar");
const botonCancelar = $<HTMLButtonElement>("#cancelar");
const mensajeNotion = $<HTMLParagraphElement>("#mensaje-notion");
// Bases
const botonBuscarBases = $<HTMLButtonElement>("#buscar-bases");
const listaBases = $<HTMLUListElement>("#lista-bases");
const mensajeBases = $<HTMLParagraphElement>("#mensaje-bases");
// Avisos
const campoIntervalo = $<HTMLInputElement>("#intervalo");
const botonRevisar = $<HTMLButtonElement>("#revisar-ahora");
const mensajeIntervalo = $<HTMLParagraphElement>("#mensaje-intervalo");

let ajustes: Ajustes = { intervalo_minutos: 5, bases: [] };
/** Bases que la integración ve ahora mismo (null = todavía no se buscó). */
let basesVisibles: BaseDatos[] | null = null;

function mostrar(elemento: HTMLParagraphElement, texto: string, tipo: "exito" | "error" | "" = "") {
  elemento.textContent = texto;
  elemento.className = `mensaje ${tipo}`.trim();
}

async function ocupado<T>(botones: HTMLButtonElement[], tarea: () => Promise<T>): Promise<T> {
  botones.forEach((b) => (b.disabled = true));
  try {
    return await tarea();
  } finally {
    botones.forEach((b) => (b.disabled = false));
  }
}

async function guardarAjustes(mensaje: HTMLParagraphElement) {
  try {
    ajustes = await invoke<Ajustes>("guardar_ajustes", { ajustes });
    campoIntervalo.value = String(ajustes.intervalo_minutos);
    mostrar(mensaje, "Guardado.", "exito");
    setTimeout(() => mostrar(mensaje, ""), 2000);
  } catch (error) {
    mostrar(mensaje, String(error), "error");
  }
}

// ── Token de Notion ───────────────────────────────────────────

function vistaToken(hayToken: boolean, puedeCancelar = false) {
  conToken.hidden = !hayToken;
  formToken.hidden = hayToken;
  botonCancelar.hidden = !puedeCancelar;
  campoToken.value = "";
}

formToken.addEventListener("submit", async (e) => {
  e.preventDefault();
  const boton = formToken.querySelector<HTMLButtonElement>('button[type="submit"]')!;
  mostrar(mensajeNotion, "Verificando con Notion…");
  try {
    const descripcion = await ocupado([boton], () =>
      invoke<string>("guardar_token", { token: campoToken.value }),
    );
    vistaToken(true);
    mostrar(mensajeNotion, `Conectado: ${descripcion}`, "exito");
    buscarBases();
  } catch (error) {
    mostrar(mensajeNotion, String(error), "error");
  }
});

botonProbar.addEventListener("click", async () => {
  mostrar(mensajeNotion, "Probando…");
  try {
    const descripcion = await ocupado([botonProbar], () => invoke<string>("probar_conexion"));
    mostrar(mensajeNotion, `Conectado: ${descripcion}`, "exito");
  } catch (error) {
    mostrar(mensajeNotion, String(error), "error");
  }
});

botonCambiar.addEventListener("click", () => {
  vistaToken(false, true);
  mostrar(mensajeNotion, "");
  campoToken.focus();
});

botonCancelar.addEventListener("click", () => {
  vistaToken(true);
  mostrar(mensajeNotion, "");
});

botonBorrar.addEventListener("click", async () => {
  try {
    await invoke("borrar_token");
    vistaToken(false);
    mostrar(mensajeNotion, "Token borrado del Llavero.");
  } catch (error) {
    mostrar(mensajeNotion, String(error), "error");
  }
});

// ── Bases de datos ────────────────────────────────────────────

function dibujarBases() {
  // Mostrar las visibles + las ya elegidas que Notion no devolvió (p. ej. dejaron de compartirse).
  const visibles = basesVisibles ?? [];
  const idsVisibles = new Set(visibles.map((b) => b.id));
  const ocultas = ajustes.bases.filter((b) => !idsVisibles.has(b.id));
  const elegidas = new Set(ajustes.bases.map((b) => b.id));

  const filas = [...visibles, ...ocultas].map((base) => {
    const li = document.createElement("li");
    const label = document.createElement("label");
    const casilla = document.createElement("input");
    casilla.type = "checkbox";
    casilla.checked = elegidas.has(base.id);
    casilla.addEventListener("change", () => {
      ajustes.bases = casilla.checked
        ? [...ajustes.bases, base]
        : ajustes.bases.filter((b) => b.id !== base.id);
      guardarAjustes(mensajeBases);
    });
    const nombre = document.createElement("span");
    nombre.textContent = base.titulo;
    label.append(casilla, nombre);
    if (basesVisibles !== null && !idsVisibles.has(base.id)) {
      const aviso = document.createElement("span");
      aviso.className = "no-visible";
      aviso.textContent = "(ya no está compartida)";
      label.append(aviso);
    }
    li.append(label);
    return li;
  });
  listaBases.replaceChildren(...filas);
}

async function buscarBases() {
  mostrar(mensajeBases, "Buscando bases en Notion…");
  try {
    basesVisibles = await ocupado([botonBuscarBases], () => invoke<BaseDatos[]>("listar_bases"));
    dibujarBases();
    if (basesVisibles.length === 0) {
      mostrar(
        mensajeBases,
        "Tu integración todavía no ve ninguna base de datos. Compártelas desde Notion: en cada base, ⋯ → Conexiones → tu integración. Luego pulsa «Buscar bases».",
        "error",
      );
    } else {
      const n = basesVisibles.length;
      mostrar(mensajeBases, `${n} base${n === 1 ? "" : "s"} encontrada${n === 1 ? "" : "s"}. Marca las que quieres vigilar.`);
    }
  } catch (error) {
    mostrar(mensajeBases, String(error), "error");
  }
}

botonBuscarBases.addEventListener("click", buscarBases);

// ── Avisos ────────────────────────────────────────────────────

campoIntervalo.addEventListener("change", () => {
  ajustes.intervalo_minutos = Math.round(Number(campoIntervalo.value)) || 5;
  guardarAjustes(mensajeIntervalo);
});

botonRevisar.addEventListener("click", () => {
  invoke("revisar_ahora");
  mostrar(mensajeIntervalo, "Revisando…");
});

// Resultado de cada revisión (null = todo bien; texto = error explicado).
listen<string | null>("avisos-estado", ({ payload: error }) => {
  if (error) {
    mostrar(mensajeIntervalo, error, "error");
  } else if (ajustes.bases.length === 0) {
    mostrar(mensajeIntervalo, "Elige al menos una base para empezar a recibir avisos.");
  } else {
    const hora = new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
    mostrar(mensajeIntervalo, `Última revisión: ${hora} ✓`, "exito");
  }
});

// ── General: inicio automático ────────────────────────────────

const casillaInicio = $<HTMLInputElement>("#inicio-automatico");
const mensajeGeneral = $<HTMLParagraphElement>("#mensaje-general");
let esDesarrollo = false;

casillaInicio.addEventListener("change", async () => {
  try {
    casillaInicio.checked = await invoke<boolean>("fijar_inicio_automatico", {
      activo: casillaInicio.checked,
    });
    if (esDesarrollo && casillaInicio.checked) {
      mostrar(
        mensajeGeneral,
        "Ojo: estás en modo desarrollo. Actívalo desde la app instalada, no desde la terminal.",
        "error",
      );
    } else {
      mostrar(mensajeGeneral, "Guardado.", "exito");
      setTimeout(() => mostrar(mensajeGeneral, ""), 2000);
    }
  } catch (error) {
    mostrar(mensajeGeneral, String(error), "error");
  }
});

async function iniciarGeneral() {
  try {
    esDesarrollo = await invoke<boolean>("modo_desarrollo");
    casillaInicio.checked = await invoke<boolean>("inicio_automatico");
  } catch (error) {
    mostrar(mensajeGeneral, String(error), "error");
  }
}

// ── Arranque ──────────────────────────────────────────────────

async function iniciar() {
  ajustes = await invoke<Ajustes>("leer_ajustes");
  campoIntervalo.value = String(ajustes.intervalo_minutos);
  dibujarBases();

  let hayToken = false;
  try {
    hayToken = await invoke<boolean>("hay_token");
  } catch (error) {
    mostrar(mensajeNotion, String(error), "error");
  }
  vistaToken(hayToken);
  if (hayToken) buscarBases();
  iniciarGeneral();
}

iniciar();
