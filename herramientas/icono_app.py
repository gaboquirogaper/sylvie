#!/usr/bin/env python3
"""
Genera el ícono PROVISIONAL de la app (1024×1024) a partir de la mascota provisional:
un cuadrado redondeado oscuro con el brotecito en el centro.

Uso (desde la carpeta del proyecto):
    python3 herramientas/icono_app.py
    npm run tauri icon herramientas/icono-app.png     # crea todos los tamaños en src-tauri/icons
"""
import os
import struct
import zlib

from mascota_provisional import reposo, TAM

LADO = 1024
MARGEN = 100            # rejilla de íconos de macOS: el cuadrado ocupa 824×824
RADIO = 185
ESCALA = 20             # 32 px × 20 = 640 px (múltiplo entero: sigue nítido)
ARRIBA = (28, 48, 40)   # degradado del fondo
ABAJO = (12, 22, 18)


def dentro_cuadrado(x, y):
    a, b = MARGEN, LADO - MARGEN
    if not (a <= x < b and a <= y < b):
        return False
    cx = min(max(x, a + RADIO), b - RADIO)
    cy = min(max(y, a + RADIO), b - RADIO)
    return (x - cx) ** 2 + (y - cy) ** 2 <= RADIO ** 2


def main():
    cuadro = reposo()[0]
    inicio = (LADO - TAM * ESCALA) // 2
    filas = []
    for y in range(LADO):
        fila = bytearray([0])
        t = y / LADO
        fondo = tuple(round(ARRIBA[i] + (ABAJO[i] - ARRIBA[i]) * t) for i in range(3))
        for x in range(LADO):
            color = (0, 0, 0, 0)
            if dentro_cuadrado(x, y):
                color = (*fondo, 255)
                sx, sy = (x - inicio) // ESCALA, (y - inicio + 30) // ESCALA
                if 0 <= sx < TAM and 0 <= sy < TAM and x >= inicio and y >= inicio - 30:
                    pixel = cuadro[sy][sx]
                    if pixel[3] > 0:
                        color = pixel
            fila.extend(color)
        filas.append(bytes(fila))

    def bloque(tipo, contenido):
        return (struct.pack(">I", len(contenido)) + tipo + contenido
                + struct.pack(">I", zlib.crc32(tipo + contenido) & 0xFFFFFFFF))

    png = (b"\x89PNG\r\n\x1a\n"
           + bloque(b"IHDR", struct.pack(">IIBBBBB", LADO, LADO, 8, 6, 0, 0, 0))
           + bloque(b"IDAT", zlib.compress(b"".join(filas), 9))
           + bloque(b"IEND", b""))
    ruta = os.path.join(os.path.dirname(__file__), "icono-app.png")
    with open(ruta, "wb") as f:
        f.write(png)
    print("Ícono creado: herramientas/icono-app.png")


if __name__ == "__main__":
    main()
