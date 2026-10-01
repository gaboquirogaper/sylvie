#!/bin/bash
# Diagnóstico de YouTube para Sylvie: hace lo mismo que Sylvie para encontrar YouTube
# en tus navegadores y muestra qué responde cada uno. No cambia nada.
# Uso:  bash herramientas/diagnostico-youtube.sh

for NAV in "Google Chrome" "Brave Browser" "Microsoft Edge" "Safari" "Arc"; do
  if ! pgrep -x "$NAV" >/dev/null; then
    echo "· $NAV: cerrado"
    continue
  fi
  echo "== $NAV: abierto"
  if [ "$NAV" = "Safari" ]; then TITULO="name"; else TITULO="title"; fi
  osascript <<APPLESCRIPT 2>&1 | sed 's/^/   /'
tell application "$NAV"
  set sep to character id 9
  set salida to ""
  repeat with w from 1 to (count of windows)
    try
      repeat with t from 1 to (count of tabs of window w)
        set u to URL of tab t of window w
        if u contains "youtube.com" then
          set salida to salida & "ventana " & (w as text) & sep & "pestaña " & (t as text) & sep & u & sep & ($TITULO of tab t of window w) & linefeed
        end if
      end repeat
    end try
  end repeat
  if salida is "" then return "(no encontré pestañas de YouTube)"
  return salida
end tell
APPLESCRIPT
  # Prueba del permiso de JavaScript en la primera ventana/pestaña con YouTube
  if [ "$NAV" = "Safari" ]; then
    echo "   Prueba de JavaScript (pestaña activa):"
    osascript -e 'tell application "Safari" to do JavaScript "document.title" in current tab of front window' 2>&1 | sed 's/^/     /'
  elif [ "$NAV" != "Arc" ]; then
    echo "   Prueba de JavaScript (pestaña activa):"
    osascript -e "tell application \"$NAV\" to execute active tab of front window javascript \"document.title\"" 2>&1 | sed 's/^/     /'
  fi
done
