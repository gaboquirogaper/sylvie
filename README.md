# Sylvie 🌱

Asistente de escritorio que vive en el **notch de la MacBook** (o como **mascota flotante** en
pantallas sin notch). Hecha con Tauri 2 (Rust + TypeScript). Versión **0.3.0**.

## Qué hace

- **Notion:** avisos cuando cambian tus bases de datos.
- **Claude:** le pides cosas en lenguaje normal y las hace en tus apps, usando **Claude Code**
  (tu plan de Claude, sin la API ni costos extra). Puedes adjuntar archivos o texto largo.
- **Música:** Spotify, Apple Music, YouTube y YouTube Music, con portada, barra y controles.
  Corazón en Apple Music y en Spotify (si lo conectas).
- **Agenda y tareas:** Google Calendar (y Notion Calendar), ClickUp, Asana, Trello y
  Microsoft Planner. Aviso 5 minutos antes de cada reunión con botón «Unirse».
- **Bandeja:** arrastra archivos a Sylvie (se los come 🍽️) y envíalos por AirDrop.
- **Mascota:** la semillita, en pixel art 32×32, con caras según lo que pasa (duerme, piensa,
  hace de DJ, se marea con tres toques…).

## Privacidad

- Sin telemetría. Sylvie solo habla con las apps que conectas, con Claude Code en tu Mac y con
  Spotify / Apple Music / YouTube para las portadas.
- Tokens y permisos se guardan en el **Llavero de macOS** (Administrador de credenciales en
  Windows), nunca en archivos.
- Las conexiones solo **leen**: Sylvie no cambia nada en esas apps (salvo el corazón de música).

## Desarrollo

```bash
npm install
npm run tauri dev      # probar
npm run tauri build    # app + .dmg en src-tauri/target/release/bundle/
```

Requisitos: Rust, Node, y Claude Code en `~/.local/bin/claude` con sesión iniciada.

### IDs públicos

Para que Spotify y Microsoft Planner se conecten con un solo botón, pega los IDs de tus apps
en `src-tauri/src/claves_publicas.rs` (no son secretos). Si quedan vacíos, Ajustes muestra los
pasos para que cada persona use su propia app.

### Estructura

- `src-tauri/src/`: Rust (ventana del notch, Notion, Claude Code, música, YouTube, calendario,
  integraciones, Planner, Spotify, AirDrop, Llavero…). Cada archivo explica qué hace al inicio.
- `src/main.ts`: el notch. `src/configuracion.ts`: la ventana de Ajustes.
- `src/assets/sylvie/`: sprites de la mascota (los genera `herramientas/mascota_provisional.py`).
- `herramientas/diagnostico-youtube.sh`: revisa qué ve Sylvie de YouTube en tus navegadores.

## Próximamente

Windows (fase 8), atajo de teclado global, aprobar permisos de Claude Code desde el notch,
mascota flotante en otros monitores y Seed Studio.
