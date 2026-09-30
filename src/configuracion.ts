import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Mascota } from "./mascota";

// ══ Tipos ═══════════════════════════════════════════════════════
type Seccion = "general" | "conexiones" | "notion" | "claude";
type BaseDatos = { id: string; titulo: string };
type Ajustes = {
  intervalo_minutos: number;
  bases: BaseDatos[];
  claude_apps: string[];
  apariencia: "notch" | "flotante";
  esquina: string;
};
type EstadoApp = { id: string; conectada: boolean };
type Conector = { nombre: string; prefijo: string; estado: string; conectado: boolean; permitido: boolean };

type AppConexion = {
  id: string;
  nombre: string;
  letra: string;
  color: string;
  detalle: string;
  /** Si se configura en otra sección (Notion). */
  seccion?: Seccion;
  proximamente?: boolean;
  /** Pasos para conectar (HTML escrito aquí mismo, no viene de afuera). */
  pasos?: string[];
  enlace?: { url: string; texto: string };
  /** Datos que se piden. Si son varios (Trello), se envían juntos separados por "|". */
  campos?: { etiqueta: string; ejemplo: string; secreto: boolean }[];
  /** Si la app depende de otra conexión (Notion Calendar usa la de Google Calendar). */
  usa?: string;
  /** Si no tiene llave: funciona sola en la Mac (Música). */
  automatica?: boolean;
  /** Botón «Abrir» (id para abrir_app). */
  abrir?: string;
};

const APPS: AppConexion[] = [
  {
    id: "notion",
    nombre: "Notion",
    letra: "N",
    color: "#e6e6ea",
    detalle: "Avisos de tus bases de datos.",
    seccion: "notion",
  },
  {
    id: "calendario",
    nombre: "Google Calendar",
    letra: "G",
    color: "#8fb8ff",
    detalle: "Tus reuniones de la semana, aviso 5 minutos antes y botón «Unirse».",
    enlace: { url: "https://calendar.google.com/calendar/r/settings", texto: "Abrir configuración de Google Calendar ↗" },
    pasos: [
      "Abre la configuración de Google Calendar en la web.",
      "A la izquierda, en <b>Configuración de mis calendarios</b>, elige tu calendario y baja hasta <b>Integrar el calendario</b>.",
      "Copia la <b>Dirección secreta en formato iCal</b> (termina en <code>basic.ics</code>) y pégala aquí:",
    ],
    campos: [{ etiqueta: "Dirección secreta iCal", ejemplo: "https://calendar.google.com/calendar/ical/…/basic.ics", secreto: true }],
  },
  {
    id: "notion-calendar",
    nombre: "Notion Calendar",
    letra: "31",
    color: "#f2f2f5",
    detalle: "Notion Calendar no tiene conexión propia: muestra tus calendarios de Google. Conecta Google Calendar y sus eventos aparecen en Sylvie.",
    usa: "calendario",
    abrir: "notion-calendar",
  },
  {
    id: "clickup",
    nombre: "ClickUp",
    letra: "U",
    color: "#c9a0ff",
    detalle: "Tareas asignadas a ti, ordenadas por fecha de entrega.",
    enlace: { url: "https://app.clickup.com/settings/apps", texto: "Abrir ClickUp → Apps ↗" },
    pasos: [
      "En ClickUp, entra a tu avatar → <b>Configuración</b> → <b>Apps</b>.",
      "En <b>API Token</b>, pulsa <b>Generar</b> (o <b>Copiar</b> si ya tienes uno).",
      "Pega el token aquí (empieza con <code>pk_</code>):",
    ],
    campos: [{ etiqueta: "Token personal", ejemplo: "pk_…", secreto: true }],
  },
  {
    id: "asana",
    nombre: "Asana",
    letra: "A",
    color: "#ff9e9e",
    detalle: "Tareas pendientes asignadas a ti en tus espacios.",
    enlace: { url: "https://app.asana.com/0/my-apps", texto: "Abrir la consola de Asana ↗" },
    pasos: [
      "Abre la consola de desarrolladores de Asana.",
      "Pulsa <b>Crear token de acceso personal</b>, llámalo «Sylvie» y acepta.",
      "Copia el token (solo se muestra una vez) y pégalo aquí:",
    ],
    campos: [{ etiqueta: "Token de acceso personal", ejemplo: "2/1234…", secreto: true }],
  },
  {
    id: "trello",
    nombre: "Trello",
    letra: "T",
    color: "#5ab4f0",
    detalle: "Tarjetas abiertas donde eres miembro, con su fecha de entrega.",
    enlace: { url: "https://trello.com/power-ups/admin", texto: "Abrir Trello → Power-Ups (admin) ↗" },
    pasos: [
      "En Trello, abre el panel de administración de Power-Ups y pulsa <b>Nuevo</b> (<i>New</i>). Llámalo «Sylvie» y elige tu espacio de trabajo (la URL puede quedar vacía).",
      "Entra a la pestaña <b>Clave de API</b> (<i>API key</i>) → <b>Generar una nueva clave de API</b> (<i>Generate a new API key</i>) y copia la <b>Clave de API</b>.",
      "Al lado de la clave, pulsa el enlace <b>Token</b> → <b>Permitir</b> (<i>Allow</i>), y copia el token que aparece. Pega ambos aquí:",
    ],
    campos: [
      { etiqueta: "Clave de API", ejemplo: "Clave de API (32 caracteres)", secreto: false },
      { etiqueta: "Token", ejemplo: "Token (ATTA…)", secreto: true },
    ],
  },
  {
    id: "planner",
    nombre: "Microsoft Planner",
    letra: "P",
    color: "#b5e48c",
    detalle: "Tareas de Microsoft 365. Necesita iniciar sesión con Microsoft (no usa tokens): llegará en una próxima versión.",
    proximamente: true,
  },
  {
    id: "musica",
    nombre: "Música",
    letra: "♪",
    color: "#6fd6a0",
    detalle: "Spotify y Apple Music se detectan solos en tu Mac. YouTube y YouTube Music: próximamente.",
    automatica: true,
    abrir: "musica",
  },
  {
    id: "saas",
    nombre: "Seed Studio",
    letra: "S",
    color: "#f5b971",
    detalle: "Citas nuevas y su asistente de IA.",
    proximamente: true,
  },
];

// ══ Utilidades ══════════════════════════════════════════════════
const $ = <T extends HTMLElement>(selector: string) => document.querySelector<T>(selector)!;

function el<K extends keyof HTMLElementTagNameMap>(tag: K, clase = "", texto = "") {
  const e = document.createElement(tag);
  if (clase) e.className = clase;
  if (texto) e.textContent = texto;
  return e;
}

function boton(texto: string, clase = "boton sec", alHacerClic?: () => void) {
  const b = el("button", clase, texto);
  b.type = "button";
  if (alHacerClic) b.addEventListener("click", alHacerClic);
  return b;
}

function mostrar(elemento: HTMLElement, texto: string, tipo: "exito" | "error" | "" = "") {
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

// ══ Estado ══════════════════════════════════════════════════════
let ajustes: Ajustes = { intervalo_minutos: 5, bases: [], claude_apps: [], apariencia: "notch", esquina: "abajo-der" };
let conexiones: Record<string, boolean> = {};
let appAbierta: string | null = null; // tarjeta de conexión desplegada
let basesVisibles: BaseDatos[] | null = null;
let conectores: Conector[] | null = null;
let seccionActual: Seccion = "conexiones";

// ══ Navegación ══════════════════════════════════════════════════
function ir(seccion: Seccion) {
  seccionActual = seccion;
  document.querySelectorAll<HTMLButtonElement>(".item[data-seccion]").forEach((b) => {
    b.classList.toggle("activo", b.dataset.seccion === seccion);
  });
  document.querySelectorAll<HTMLElement>(".seccion").forEach((s) => (s.hidden = s.id !== `s-${seccion}`));
  if (seccion === "claude" && conectores === null) buscarConectores();
  if (seccion === "notion" && conexiones.notion && basesVisibles === null) buscarBases();
}

document.querySelectorAll<HTMLButtonElement>(".item[data-seccion]").forEach((b) => {
  b.addEventListener("click", () => ir(b.dataset.seccion as Seccion));
});

// Enlaces de ayuda (solo los que Rust permite abrir).
document.addEventListener("click", (e) => {
  const enlace = (e.target as HTMLElement).closest<HTMLElement>("[data-url]");
  if (enlace) invoke("abrir_enlace", { url: enlace.dataset.url }).catch(console.error);
});

function dibujarResumenes() {
  const conectables = APPS.filter((a) => !a.proximamente && !a.usa && !a.automatica);
  const n = conectables.filter((a) => conexiones[a.id]).length;
  $("#resumen-conexiones").textContent = `${n}/${conectables.length}`;
  $("#resumen-notion").textContent = conexiones.notion ? "✓" : "";
  $("#resumen-claude").textContent = ajustes.claude_apps.length ? String(ajustes.claude_apps.length) : "";
}

// ══ General ═════════════════════════════════════════════════════
const mensajeGeneral = $("#mensaje-general");
const campoIntervalo = $<HTMLInputElement>("#intervalo");
const casillaInicio = $<HTMLInputElement>("#inicio-automatico");
let esDesarrollo = false;

async function guardarAjustes(mensaje: HTMLElement) {
  try {
    ajustes = await invoke<Ajustes>("guardar_ajustes", { ajustes });
    campoIntervalo.value = String(ajustes.intervalo_minutos);
    mostrar(mensaje, "Guardado.", "exito");
    setTimeout(() => mostrar(mensaje, ""), 2000);
  } catch (error) {
    mostrar(mensaje, String(error), "error");
  }
}

campoIntervalo.addEventListener("change", () => {
  ajustes.intervalo_minutos = Math.round(Number(campoIntervalo.value)) || 5;
  guardarAjustes(mensajeGeneral);
});

$("#revisar-ahora").addEventListener("click", () => {
  invoke("revisar_ahora");
  mostrar(mensajeGeneral, "Revisando Notion…");
});

listen<string | null>("avisos-estado", ({ payload: error }) => {
  if (error) mostrar(mensajeGeneral, error, "error");
  else if (ajustes.bases.length === 0) mostrar(mensajeGeneral, "Elige al menos una base en la sección Notion para recibir avisos.");
  else mostrar(mensajeGeneral, `Última revisión: ${new Date().toLocaleTimeString("es", { hour: "2-digit", minute: "2-digit" })} ✓`, "exito");
});

// Apariencia: notch o mascota flotante (y en qué esquina)
function dibujarApariencia() {
  document.querySelectorAll<HTMLButtonElement>("[data-apariencia]").forEach((b) => {
    const activa = b.dataset.apariencia === ajustes.apariencia;
    b.classList.toggle("activa", activa);
    b.setAttribute("aria-checked", String(activa));
  });
  document.querySelectorAll<HTMLButtonElement>(".esquinas [data-esquina]").forEach((b) => {
    const activa = b.dataset.esquina === ajustes.esquina;
    b.classList.toggle("activa", activa);
    b.setAttribute("aria-checked", String(activa));
  });
  $("#fila-esquina").hidden = ajustes.apariencia !== "flotante";
}

document.querySelectorAll<HTMLButtonElement>("[data-apariencia]").forEach((b) => {
  b.addEventListener("click", () => {
    ajustes.apariencia = b.dataset.apariencia as Ajustes["apariencia"];
    dibujarApariencia();
    guardarAjustes(mensajeGeneral);
  });
});
document.querySelectorAll<HTMLButtonElement>(".esquinas [data-esquina]").forEach((b) => {
  b.addEventListener("click", () => {
    ajustes.esquina = b.dataset.esquina!;
    dibujarApariencia();
    guardarAjustes(mensajeGeneral);
  });
});

casillaInicio.addEventListener("change", async () => {
  try {
    casillaInicio.checked = await invoke<boolean>("fijar_inicio_automatico", { activo: casillaInicio.checked });
    if (esDesarrollo && casillaInicio.checked) {
      mostrar(mensajeGeneral, "Ojo: estás en modo desarrollo. Actívalo desde la app instalada, no desde la terminal.", "error");
    } else {
      mostrar(mensajeGeneral, "Guardado.", "exito");
      setTimeout(() => mostrar(mensajeGeneral, ""), 2000);
    }
  } catch (error) {
    mostrar(mensajeGeneral, String(error), "error");
  }
});

// ══ Conexiones ══════════════════════════════════════════════════
function tarjetaApp(app: AppConexion) {
  const conectada = app.automatica || !!conexiones[app.usa ?? app.id];
  const abierta = appAbierta === app.id;
  const tarjeta = el("div", `app${abierta ? " abierta" : ""}${app.proximamente ? " apagada" : ""}`);

  const cabeza = el("div", "cabeza");
  const letra = el("span", app.letra.length > 1 ? "letra doble" : "letra", app.letra);
  letra.style.background = app.color;
  const textos = el("div", "textos");
  const titulo = el("div", "titulo");
  titulo.append(el("b", "", app.nombre));
  if (app.proximamente) titulo.append(el("span", "chip", "Próximamente"));
  else if (app.automatica) titulo.append(el("span", "chip ok", "Automática"));
  else if (conectada) titulo.append(el("span", "chip ok", "Conectada"));
  textos.append(titulo, el("small", "", app.detalle));
  const acciones = el("div", "acciones");

  const botonAbrir = () => boton("Abrir", "boton sec", () => invoke("abrir_app", { cual: app.abrir ?? app.id }).catch(console.error));
  if (app.proximamente) {
    // nada que hacer todavía
  } else if (app.automatica) {
    acciones.append(botonAbrir());
  } else if (app.usa) {
    acciones.append(conectada ? botonAbrir() : boton("Conectar Google Calendar", "boton", () => desplegar(app.usa!)));
  } else if (app.seccion) {
    acciones.append(boton(conectada ? "Ajustar" : "Conectar", conectada ? "boton sec" : "boton", () => ir(app.seccion!)));
  } else if (conectada && !abierta) {
    acciones.append(
      botonAbrir(),
      boton("Cambiar", "boton sec", () => desplegar(app.id)),
      boton("Quitar", "boton peligro", () => desconectar(app)),
    );
  } else if (!abierta) {
    acciones.append(boton("Conectar", "boton", () => desplegar(app.id)));
  }
  cabeza.append(letra, textos, acciones);
  tarjeta.append(cabeza);

  if (abierta && app.pasos && app.campos) tarjeta.append(formularioApp(app));
  return tarjeta;
}

function formularioApp(app: AppConexion) {
  const form = el("form", "pasos-form");
  const pasos = el("ol", "pasos");
  app.pasos!.forEach((texto, i) => {
    const li = el("li");
    li.innerHTML = texto; // texto fijo escrito arriba, no viene de internet
    if (i === 0 && app.enlace) {
      const b = boton(app.enlace.texto, "enlace");
      b.dataset.url = app.enlace.url;
      li.append(b);
    }
    pasos.append(li);
  });

  const fila = el("div", "campo-fila");
  const campos = app.campos!.map((c) => {
    const campo = el("input");
    campo.type = c.secreto ? "password" : "text";
    campo.placeholder = c.ejemplo;
    campo.autocomplete = "off";
    campo.spellcheck = false;
    campo.setAttribute("aria-label", c.etiqueta);
    return campo;
  });
  const guardar = el("button", "boton", "Verificar y guardar");
  guardar.type = "submit";
  if (campos.length > 1) {
    fila.classList.add("varios");
    const columna = el("div", "campos");
    columna.append(...campos);
    fila.append(columna);
  } else fila.append(...campos);
  fila.append(guardar, boton("Cancelar", "boton sec", () => desplegar(null)));

  const mensaje = el("p", "mensaje");
  mensaje.setAttribute("role", "status");
  const nota = el("p", "tenue chico", "Se guarda en el Llavero de tu Mac. Sylvie nunca lo muestra ni lo envía a otro lado.");

  form.append(pasos, fila, mensaje, nota);
  form.addEventListener("submit", async (e) => {
    e.preventDefault();
    mostrar(mensaje, "Verificando…");
    try {
      const valor = campos.map((c) => c.value.trim()).join("|");
      const descripcion = await ocupado([guardar], () => invoke<string>("conectar_app", { id: app.id, valor }));
      campos.forEach((c) => (c.value = ""));
      appAbierta = null;
      await cargarConexiones();
      mostrarGlobal(`✓ Conectado: ${descripcion}`, "exito");
    } catch (error) {
      mostrar(mensaje, String(error), "error");
    }
  });
  setTimeout(() => campos[0].focus(), 50);
  return form;
}

function mostrarGlobal(texto: string, tipo: "exito" | "error") {
  const aviso = document.querySelector<HTMLElement>("#aviso-conexiones");
  if (!aviso) return;
  mostrar(aviso, texto, tipo);
  setTimeout(() => mostrar(aviso, ""), 5000);
}

function desplegar(id: string | null) {
  appAbierta = id;
  dibujarApps();
}

async function desconectar(app: AppConexion) {
  try {
    await invoke("desconectar_app", { id: app.id });
    await cargarConexiones();
    mostrarGlobal(`${app.nombre} desconectado y borrado del Llavero.`, "exito");
  } catch (error) {
    mostrarGlobal(String(error), "error");
  }
}

function dibujarApps() {
  const aviso = el("p", "mensaje");
  aviso.id = "aviso-conexiones";
  aviso.setAttribute("role", "status");
  const anterior = document.querySelector("#aviso-conexiones");
  if (anterior) {
    aviso.textContent = anterior.textContent;
    aviso.className = anterior.className;
  }
  $("#lista-apps").replaceChildren(...APPS.map(tarjetaApp), aviso);
}

async function cargarConexiones() {
  try {
    const lista = await invoke<EstadoApp[]>("estado_conexiones");
    conexiones = Object.fromEntries(lista.map((a) => [a.id, a.conectada]));
  } catch (error) {
    mostrarGlobal(String(error), "error");
  }
  dibujarApps();
  dibujarNotion();
  dibujarResumenes();
}

// ══ Notion ══════════════════════════════════════════════════════
const conToken = $("#con-token");
const formToken = $<HTMLFormElement>("#form-token");
const campoToken = $<HTMLInputElement>("#token");
const mensajeNotion = $("#mensaje-notion");
const botonCancelar = $<HTMLButtonElement>("#cancelar");
const botonBuscarBases = $<HTMLButtonElement>("#buscar-bases");
const mensajeBases = $("#mensaje-bases");
let cambiandoToken = false;

function dibujarNotion() {
  const hay = !!conexiones.notion;
  conToken.hidden = !hay || cambiandoToken;
  formToken.hidden = hay && !cambiandoToken;
  botonCancelar.hidden = !cambiandoToken;
  $("#estado-notion").textContent = hay ? "✓ Guardado en el Llavero" : "Sin conectar";
  $("#estado-notion").className = hay ? "ok" : "";
  botonBuscarBases.disabled = !hay;
}

formToken.addEventListener("submit", async (e) => {
  e.preventDefault();
  const guardar = formToken.querySelector<HTMLButtonElement>('button[type="submit"]')!;
  mostrar(mensajeNotion, "Verificando con Notion…");
  try {
    const descripcion = await ocupado([guardar], () => invoke<string>("guardar_token", { token: campoToken.value }));
    campoToken.value = "";
    cambiandoToken = false;
    await cargarConexiones();
    mostrar(mensajeNotion, `Conectado: ${descripcion}`, "exito");
    buscarBases();
  } catch (error) {
    mostrar(mensajeNotion, String(error), "error");
  }
});

$("#probar").addEventListener("click", async (e) => {
  mostrar(mensajeNotion, "Probando…");
  try {
    const descripcion = await ocupado([e.currentTarget as HTMLButtonElement], () => invoke<string>("probar_conexion"));
    mostrar(mensajeNotion, `Conectado: ${descripcion}`, "exito");
  } catch (error) {
    mostrar(mensajeNotion, String(error), "error");
  }
});

$("#cambiar").addEventListener("click", () => {
  cambiandoToken = true;
  dibujarNotion();
  mostrar(mensajeNotion, "");
  campoToken.focus();
});

botonCancelar.addEventListener("click", () => {
  cambiandoToken = false;
  campoToken.value = "";
  dibujarNotion();
  mostrar(mensajeNotion, "");
});

$("#borrar").addEventListener("click", async () => {
  try {
    await invoke("borrar_token");
    basesVisibles = null;
    await cargarConexiones();
    dibujarBases();
    mostrar(mensajeNotion, "Token borrado del Llavero.");
  } catch (error) {
    mostrar(mensajeNotion, String(error), "error");
  }
});

function dibujarBases() {
  // Las visibles + las ya elegidas que Notion no devolvió (p. ej. dejaron de compartirse).
  const visibles = basesVisibles ?? [];
  const idsVisibles = new Set(visibles.map((b) => b.id));
  const ocultas = ajustes.bases.filter((b) => !idsVisibles.has(b.id));
  const elegidas = new Set(ajustes.bases.map((b) => b.id));

  $("#lista-bases").replaceChildren(
    ...[...visibles, ...ocultas].map((base) => {
      const li = el("li");
      const nombre = el("span", "nombre", base.titulo);
      if (basesVisibles !== null && !idsVisibles.has(base.id)) nombre.append(el("small", "tenue", " (ya no está compartida)"));
      const interruptor = el("label", "interruptor");
      const casilla = el("input");
      casilla.type = "checkbox";
      casilla.checked = elegidas.has(base.id);
      casilla.addEventListener("change", () => {
        ajustes.bases = casilla.checked ? [...ajustes.bases, base] : ajustes.bases.filter((b) => b.id !== base.id);
        guardarAjustes(mensajeBases);
      });
      interruptor.append(casilla, el("span"));
      li.append(nombre, interruptor);
      return li;
    }),
  );
}

async function buscarBases() {
  if (!conexiones.notion) return;
  mostrar(mensajeBases, "Buscando bases en Notion…");
  try {
    basesVisibles = await ocupado([botonBuscarBases], () => invoke<BaseDatos[]>("listar_bases"));
    dibujarBases();
    const n = basesVisibles.length;
    if (n === 0) {
      mostrar(mensajeBases, "Tu integración todavía no ve ninguna base. En Notion, abre cada base → ⋯ → Conexiones → Sylvie. Luego pulsa «Buscar bases».", "error");
    } else {
      mostrar(mensajeBases, `${n} base${n === 1 ? "" : "s"} encontrada${n === 1 ? "" : "s"}. Activa las que quieres vigilar.`);
    }
  } catch (error) {
    mostrar(mensajeBases, String(error), "error");
  }
}

botonBuscarBases.addEventListener("click", buscarBases);

// ══ Claude: qué conectores puede usar ═══════════════════════════
const botonBuscarConectores = $<HTMLButtonElement>("#buscar-conectores");
const mensajeClaude = $("#mensaje-claude");

function estadoConector(c: Conector) {
  if (c.conectado) return { texto: "Conectado", clase: "chip ok" };
  if (/auth/i.test(c.estado)) return { texto: "Falta iniciar sesión", clase: "chip aviso" };
  if (/no aparece/i.test(c.estado)) return { texto: "No encontrado", clase: "chip" };
  return { texto: "Sin conexión", clase: "chip aviso" };
}

function dibujarConectores() {
  const lista = conectores ?? [];
  $("#lista-conectores").replaceChildren(
    ...lista.map((c) => {
      const li = el("li");
      const nombre = el("span", "nombre", c.nombre);
      const estado = estadoConector(c);
      const chip = el("span", estado.clase, estado.texto);
      chip.title = c.estado;
      const interruptor = el("label", "interruptor");
      const casilla = el("input");
      casilla.type = "checkbox";
      casilla.checked = c.permitido;
      casilla.setAttribute("aria-label", `Permitir ${c.nombre}`);
      casilla.addEventListener("change", async () => {
        try {
          ajustes.claude_apps = await invoke<string[]>("permitir_conector", { prefijo: c.prefijo, permitido: casilla.checked });
          c.permitido = casilla.checked;
          dibujarResumenes();
          mostrar(mensajeClaude, casilla.checked ? `Claude ya puede usar ${c.nombre}.` : `Claude ya no usará ${c.nombre}.`, "exito");
          if (ajustes.claude_apps.length === 0) mostrar(mensajeClaude, "Ojo: sin ninguna app activada, Claude no podrá hacer pedidos.", "error");
        } catch (error) {
          casilla.checked = !casilla.checked;
          mostrar(mensajeClaude, String(error), "error");
        }
      });
      interruptor.append(casilla, el("span"));
      li.append(nombre, chip, interruptor);
      return li;
    }),
  );
}

async function buscarConectores() {
  $("#estado-conectores").textContent = "Preguntándole a Claude Code qué conectores tienes…";
  try {
    conectores = await ocupado([botonBuscarConectores], () => invoke<Conector[]>("conectores_claude"));
    const n = conectores.length;
    $("#estado-conectores").textContent = n
      ? `${n} conector${n === 1 ? "" : "es"} en tu cuenta. Activa los que Sylvie puede usar.`
      : "No encontré conectores. Agrégalos en claude.ai y vuelve a buscar.";
    dibujarConectores();
  } catch (error) {
    conectores = [];
    $("#estado-conectores").textContent = "No pude listar los conectores.";
    mostrar(mensajeClaude, String(error), "error");
  }
}

botonBuscarConectores.addEventListener("click", buscarConectores);

// ══ Arranque ════════════════════════════════════════════════════
async function iniciar() {
  const mascota = new Mascota($<HTMLCanvasElement>("#mascota"));
  mascota.cambiar("reposo");

  ajustes = await invoke<Ajustes>("leer_ajustes");
  campoIntervalo.value = String(ajustes.intervalo_minutos);
  dibujarApariencia();
  dibujarBases();
  await cargarConexiones();

  try {
    esDesarrollo = await invoke<boolean>("modo_desarrollo");
    casillaInicio.checked = await invoke<boolean>("inicio_automatico");
  } catch (error) {
    mostrar(mensajeGeneral, String(error), "error");
  }

  const pedida = await invoke<Seccion | null>("tomar_seccion").catch(() => null);
  ir(pedida ?? seccionActual);
}

listen<Seccion>("ir-a-seccion", ({ payload }) => ir(payload));
listen("conexiones-cambiadas", cargarConexiones);
// Si arrastras la mascota a otra esquina, se refleja aquí.
listen<{ modo: string; esquina: string }>("apariencia", ({ payload }) => {
  ajustes.apariencia = payload.modo === "flotante" ? "flotante" : "notch";
  ajustes.esquina = payload.esquina;
  dibujarApariencia();
});

iniciar();
