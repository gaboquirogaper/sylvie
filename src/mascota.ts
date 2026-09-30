// ── Mascota de Sylvie: animaciones en pixel art ───────────────
//
// Cada estado es una tira horizontal de cuadros de 32×32 px en src/assets/sylvie/.
// La cantidad de cuadros se calcula sola (ancho de la imagen ÷ 32), así que
// puedes reemplazar los PNG por tus dibujos sin tocar este archivo.
// Solo ajusta aquí la velocidad (cuadros por segundo) y si la animación se repite.

import urlReposo from "./assets/sylvie/reposo.png";
import urlTrabajando from "./assets/sylvie/trabajando.png";
import urlFeliz from "./assets/sylvie/feliz.png";
import urlAlerta from "./assets/sylvie/alerta.png";
import urlAturdido from "./assets/sylvie/aturdido.png";

export type EstadoMascota = "reposo" | "trabajando" | "feliz" | "alerta" | "aturdido";

const TAM_CUADRO = 32;

const ANIMACIONES: Record<EstadoMascota, { url: string; fps: number; repetir: boolean }> = {
  reposo: { url: urlReposo, fps: 3, repetir: true },
  trabajando: { url: urlTrabajando, fps: 8, repetir: true },
  feliz: { url: urlFeliz, fps: 10, repetir: true },
  alerta: { url: urlAlerta, fps: 6, repetir: true },
  aturdido: { url: urlAturdido, fps: 8, repetir: true },
};

export class Mascota {
  private contexto: CanvasRenderingContext2D;
  private imagenes = new Map<EstadoMascota, HTMLImageElement>();
  private estado: EstadoMascota = "reposo";
  private cuadro = 0;
  private temporizador: number | undefined;
  private pausada = false;
  private readonly sinMovimiento = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  constructor(private lienzo: HTMLCanvasElement) {
    lienzo.width = TAM_CUADRO;
    lienzo.height = TAM_CUADRO;
    this.contexto = lienzo.getContext("2d")!;
    this.contexto.imageSmoothingEnabled = false;

    for (const [estado, { url }] of Object.entries(ANIMACIONES) as [EstadoMascota, { url: string }][]) {
      const imagen = new Image();
      imagen.src = url;
      imagen.onload = () => {
        if (estado === this.estado) this.dibujar();
      };
      this.imagenes.set(estado, imagen);
    }
    this.programar();
  }

  /** Cambia de animación (no hace nada si ya está en ese estado). */
  cambiar(estado: EstadoMascota) {
    if (estado === this.estado) return;
    this.estado = estado;
    this.cuadro = 0;
    this.lienzo.dataset.estado = estado;
    this.dibujar();
    this.programar();
  }

  /** Pausa la animación mientras la píldora está escondida (ahorra energía). */
  pausar(pausada: boolean) {
    this.pausada = pausada;
  }

  private cuadrosDe(estado: EstadoMascota): number {
    const imagen = this.imagenes.get(estado);
    if (!imagen || !imagen.complete || imagen.naturalWidth === 0) return 1;
    return Math.max(1, Math.floor(imagen.naturalWidth / TAM_CUADRO));
  }

  private programar() {
    window.clearInterval(this.temporizador);
    if (this.sinMovimiento) return; // "Reducir movimiento": se queda en el primer cuadro
    const { fps } = ANIMACIONES[this.estado];
    this.temporizador = window.setInterval(() => this.avanzar(), 1000 / fps);
  }

  private avanzar() {
    if (this.pausada) return;
    const total = this.cuadrosDe(this.estado);
    const siguiente = this.cuadro + 1;
    if (siguiente >= total) {
      if (!ANIMACIONES[this.estado].repetir) return;
      this.cuadro = 0;
    } else {
      this.cuadro = siguiente;
    }
    this.dibujar();
  }

  private dibujar() {
    const imagen = this.imagenes.get(this.estado);
    this.contexto.clearRect(0, 0, TAM_CUADRO, TAM_CUADRO);
    if (!imagen || !imagen.complete || imagen.naturalWidth === 0) return;
    this.contexto.drawImage(
      imagen,
      this.cuadro * TAM_CUADRO, 0, TAM_CUADRO, TAM_CUADRO,
      0, 0, TAM_CUADRO, TAM_CUADRO,
    );
  }
}
