// ── Iconos propios de Sylvie (dibujados aquí, no son logos de las apps) ──────
// Trazos redondeados de 24×24; el color lo pone el CSS (currentColor).

const T = (d: string) => `<svg viewBox="0 0 24 24" aria-hidden="true">${d}</svg>`;

export const ICONOS: Record<string, string> = {
  // Notion → una hoja con renglones
  notion: T('<path d="M7 3.5h7l4 4V20a.5.5 0 0 1-.5.5h-10A.5.5 0 0 1 7 20z"/><path d="M14 3.5v4h4"/><path d="M10 12h5M10 15.5h5"/>'),
  // Claude → un destello
  claude: T('<path class="relleno" d="M12 3.5c.7 4.3 2.9 6.6 7.2 7.3-4.3.7-6.5 3-7.2 7.3-.7-4.3-2.9-6.6-7.2-7.3 4.3-.7 6.5-3 7.2-7.3z"/>'),
  // Música → una nota
  musica: T('<path d="M9.5 17.5V6.5l9-2v11"/><circle cx="7" cy="17.5" r="2.5"/><circle cx="16" cy="15.5" r="2.5"/>'),
  // Bandeja → una bandeja
  bandeja: T('<path d="M4 13.5 6.6 6h10.8l2.6 7.5V19a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1z"/><path d="M4 13.5h4.5l1.5 2.5h4l1.5-2.5H20"/>'),
  // Google Calendar → calendario con un día marcado
  calendario: T('<rect x="4" y="5" width="16" height="15" rx="3"/><path d="M4 10h16M9 3v4M15 3v4"/><rect class="relleno" x="13" y="13" width="3.5" height="3.5" rx="1"/>'),
  // Notion Calendar → calendario con renglones
  "notion-calendar": T('<rect x="4" y="5" width="16" height="15" rx="3"/><path d="M4 10h16M9 3v4M15 3v4M8 14h8M8 17h5"/>'),
  // ClickUp → círculo con check
  clickup: T('<circle cx="12" cy="12" r="8"/><path d="m8.5 12.3 2.4 2.4 4.6-4.9"/>'),
  // Asana → lista con checks
  asana: T('<path d="m4.5 7 1.6 1.6L8.8 6M4.5 13l1.6 1.6 2.7-2.6M12 7.4h7.5M12 13.4h7.5M5 18.8h14.5"/>'),
  // Trello → tarjetas apiladas
  trello: T('<rect x="4" y="7.5" width="12.5" height="12" rx="2.5"/><path d="M8 4.5h9.5a2.5 2.5 0 0 1 2.5 2.5v9"/>'),
  // Microsoft Planner → portapapeles con check
  planner: T('<rect x="5.5" y="5" width="13" height="15.5" rx="2.5"/><path d="M9 5V3.5h6V5"/><path d="m9 12.6 2.1 2.1 4-4.2"/>'),
  // Calendly → reloj
  calendly: T('<circle cx="12" cy="12" r="8"/><path d="M12 7.5V12l3 2"/>'),
  // contentBoard → tablero de bloques
  contentboard: T('<rect x="4" y="4" width="7" height="7" rx="2"/><rect x="13" y="4" width="7" height="7" rx="2"/><rect x="4" y="13" width="7" height="7" rx="2"/><path d="M16.5 13.5v6M13.5 16.5h6"/>'),
  // Seed Studio → un brote
  saas: T('<path d="M12 20.5v-8"/><path d="M12 12.5c0-4.2 2.9-6.6 7-6.6 0 4.2-2.9 6.6-7 6.6zM12 15c0-3.2-2.4-5.3-6-5.3 0 3.2 2.4 5.3 6 5.3z"/>'),
  // Spotify (corazón) → corazón
  spotify: T('<path d="M12 19.5s-7-4.3-7-9.5a4 4 0 0 1 7-2.6A4 4 0 0 1 19 10c0 5.2-7 9.5-7 9.5z"/>'),
  // Videollamada
  llamada: T('<rect x="3.5" y="7" width="12" height="10" rx="2.5"/><path d="m15.5 11 5-3v8l-5-3"/>'),
  // Agregar → más
  mas: T('<path d="M12 6v12M6 12h12"/>'),
};

/** Ícono de una app dentro de su tilecito de color (Conexiones y Ajustes). */
export function tileApp(id: string, color: string, clase = "tile") {
  const span = document.createElement("span");
  span.className = clase;
  span.style.setProperty("--c", color);
  span.innerHTML = ICONOS[id] ?? ICONOS.mas;
  return span;
}
