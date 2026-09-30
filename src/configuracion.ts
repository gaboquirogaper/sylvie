import { invoke } from "@tauri-apps/api/core";

type Ajustes = { intervalo_minutos: number };

const $ = <T extends HTMLElement>(selector: string) => document.querySelector<T>(selector)!;

const conToken = $<HTMLDivElement>("#con-token");
const formToken = $<HTMLFormElement>("#form-token");
const campoToken = $<HTMLInputElement>("#token");
const botonProbar = $<HTMLButtonElement>("#probar");
const botonCambiar = $<HTMLButtonElement>("#cambiar");
const botonBorrar = $<HTMLButtonElement>("#borrar");
const botonCancelar = $<HTMLButtonElement>("#cancelar");
const mensajeNotion = $<HTMLParagraphElement>("#mensaje-notion");
const campoIntervalo = $<HTMLInputElement>("#intervalo");
const mensajeIntervalo = $<HTMLParagraphElement>("#mensaje-intervalo");

function mostrar(elemento: HTMLParagraphElement, texto: string, tipo: "exito" | "error" | "" = "") {
  elemento.textContent = texto;
  elemento.className = `mensaje ${tipo}`.trim();
}

/** Muestra el formulario (sin token) o el resumen (con token). */
function vistaToken(hayToken: boolean, puedeCancelar = false) {
  conToken.hidden = !hayToken;
  formToken.hidden = hayToken;
  botonCancelar.hidden = !puedeCancelar;
  campoToken.value = "";
}

async function ocupado<T>(botones: HTMLButtonElement[], tarea: () => Promise<T>): Promise<T> {
  botones.forEach((b) => (b.disabled = true));
  try {
    return await tarea();
  } finally {
    botones.forEach((b) => (b.disabled = false));
  }
}

// ── Token de Notion ───────────────────────────────────────────

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

// ── Intervalo ─────────────────────────────────────────────────

campoIntervalo.addEventListener("change", async () => {
  const minutos = Math.round(Number(campoIntervalo.value));
  try {
    const guardados = await invoke<Ajustes>("guardar_ajustes", {
      ajustes: { intervalo_minutos: minutos },
    });
    campoIntervalo.value = String(guardados.intervalo_minutos);
    mostrar(mensajeIntervalo, "Guardado.", "exito");
    setTimeout(() => mostrar(mensajeIntervalo, ""), 2000);
  } catch (error) {
    mostrar(mensajeIntervalo, String(error), "error");
  }
});

// ── Arranque ──────────────────────────────────────────────────

async function iniciar() {
  try {
    vistaToken(await invoke<boolean>("hay_token"));
  } catch (error) {
    vistaToken(false);
    mostrar(mensajeNotion, String(error), "error");
  }
  const ajustes = await invoke<Ajustes>("leer_ajustes");
  campoIntervalo.value = String(ajustes.intervalo_minutos);
}

iniciar();
