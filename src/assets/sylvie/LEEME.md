# Mascota de Sylvie

Aquí viven las animaciones de la mascota. Hay **un PNG por estado**:

| Archivo          | Cuándo se ve                                       |
|------------------|----------------------------------------------------|
| `reposo.png`     | Normal, sin nada pendiente                         |
| `trabajando.png` | Mientras Claude Code hace un pedido                |
| `feliz.png`      | Un pedido terminó bien (unos segundos)             |
| `alerta.png`     | Hay avisos sin leer o un pedido falló              |
| `aturdido.png`   | Le das 3 toques seguidos a la mascota (secreto)    |
| `boca.png`       | Arrastras un archivo sobre el notch (lo espera)    |
| `comer.png`      | Sueltas el archivo: se lo "come" a la bandeja      |

## Formato

- Cada cuadro mide **32 × 32 píxeles**.
- Los cuadros van **uno al lado del otro, en horizontal** (una "tira").
  - 6 cuadros → imagen de 192 × 32; 4 cuadros → 128 × 32, etc.
- Fondo **transparente**. PNG.
- La cantidad de cuadros se detecta sola. La velocidad se ajusta en `src/mascota.ts` (`fps`).

## Cómo reemplazar la mascota provisional por la tuya

1. Dibuja tus animaciones. Programas gratuitos: **Piskel** (piskelapp.com, en el navegador)
   o **LibreSprite**. De pago: **Aseprite**.
2. Exporta cada animación como "sprite sheet" horizontal, sin espacios entre cuadros.
3. Guarda los PNG aquí con **los mismos nombres**, reemplazando los actuales.
4. Con `npm run tauri dev` corriendo, se actualiza sola.

La mascota provisional se genera con `python3 herramientas/mascota_provisional.py`
(si la vuelves a ejecutar, **sobrescribe** estos PNG).

### Animaciones extra (v0.3)

| Archivo          | Cuándo se ve                                               |
|------------------|------------------------------------------------------------|
| `dormir.png`     | Llevas un rato sin usar la Mac: duerme y le salen zetas     |
| `dj.png`         | Cambias de canción muy rápido: audífonos y discos volando  |
| `pensando.png`   | Claude está trabajando en tu pedido                        |
| `lupa.png`       | Estás mirando el calendario                                |
