#!/usr/bin/env python3
"""
Genera la mascota PROVISIONAL de Sylvie (un brote redondo con una hojita),
en tiras de sprites de 32×32 por cuadro, en src/assets/sylvie/.

Es solo un marcador hasta que dibujes la tuya. No necesita librerías extra.
Uso (desde la carpeta del proyecto):   python3 herramientas/mascota_provisional.py
"""
import math
import os
import struct
import zlib

TAM = 32
CARPETA = os.path.join(os.path.dirname(__file__), "..", "src", "assets", "sylvie")

# Paleta (R, G, B, A)
T = (0, 0, 0, 0)            # transparente
CONTORNO = (22, 58, 44, 255)
CUERPO = (126, 224, 181, 255)
LUZ = (190, 245, 218, 255)
SOMBRA = (86, 180, 140, 255)
OJO = (20, 32, 28, 255)
BRILLO = (255, 255, 255, 255)
MEJILLA = (255, 150, 160, 255)
HOJA = (70, 170, 90, 255)
HOJA_OSC = (40, 120, 60, 255)
ALERTA = (255, 105, 97, 255)
GOTA = (143, 184, 255, 255)


def lienzo():
    return [[T] * TAM for _ in range(TAM)]


def pintar(img, x, y, color):
    if 0 <= x < TAM and 0 <= y < TAM:
        img[y][x] = color


def cuerpo(img, dx=0, dy=0, aplastar=0):
    """Cuerpo redondo con contorno, luz y sombra. `aplastar` lo ensancha y achata."""
    cx, cy = 15.5 + dx, 20.5 + dy
    rx, ry = 10.5 + aplastar, 9.5 - aplastar
    for y in range(TAM):
        for x in range(TAM):
            d = ((x + 0.5 - cx) / rx) ** 2 + ((y + 0.5 - cy) / ry) ** 2
            if d <= 1.0:
                color = CUERPO
                if d > 0.72:
                    color = CONTORNO
                elif (y + 0.5) > cy + ry * 0.45:
                    color = SOMBRA
                elif (x + 0.5 - cx) < -rx * 0.35 and (y + 0.5) < cy - ry * 0.2:
                    color = LUZ
                img[y][x] = color


def hoja(img, dx=0, dy=0, inclinacion=0):
    """Tallo y hojita encima de la cabeza. `inclinacion`: -1, 0 o 1."""
    bx, by = 16 + dx, 11 + dy
    pintar(img, bx, by, HOJA_OSC)
    pintar(img, bx, by - 1, HOJA_OSC)
    lado = 1 if inclinacion >= 0 else -1
    base = bx + lado
    for i, ancho in enumerate([3, 4, 3, 1]):
        y = by - 2 - i + (1 if inclinacion == 0 else 0)
        for j in range(ancho):
            pintar(img, base + lado * j, y, HOJA)
    pintar(img, base + lado, by - 2, HOJA_OSC)


def ojos(img, dx=0, dy=0, forma="normal", mirar=0):
    for ox in (11, 19):
        x, y = ox + dx + mirar, 19 + dy
        if forma == "normal":
            for yy in range(3):
                pintar(img, x, y + yy, OJO)
                pintar(img, x + 1, y + yy, OJO)
            pintar(img, x, y, BRILLO)
        elif forma == "cerrado":
            pintar(img, x, y + 2, OJO)
            pintar(img, x + 1, y + 2, OJO)
        elif forma == "feliz":  # forma de ^
            pintar(img, x - 1 + 1, y + 1, OJO)
            pintar(img, x + 1, y, OJO)
            pintar(img, x + 2, y + 1, OJO)
            pintar(img, x, y + 1, OJO)
        elif forma == "x":  # aturdido
            for d in range(3):
                pintar(img, x - 1 + d, y + d, OJO)
                pintar(img, x + 1 - d, y + d, OJO)
        elif forma == "grande":
            for yy in range(4):
                for xx in range(3):
                    pintar(img, x - 1 + xx, y - 1 + yy, OJO)
            pintar(img, x - 1, y - 1, BRILLO)


def mejillas(img, dx=0, dy=0):
    for x in (9, 22):
        pintar(img, x + dx, 23 + dy, MEJILLA)
        pintar(img, x + 1 + dx, 23 + dy, MEJILLA)


def boca(img, dx=0, dy=0, forma="sonrisa"):
    x, y = 15 + dx, 24 + dy
    if forma == "sonrisa":
        pintar(img, x, y, OJO)
        pintar(img, x + 1, y, OJO)
    elif forma == "abierta":
        for xx in range(3):
            pintar(img, x - 1 + xx + 1, y, OJO)
        pintar(img, x + 1, y + 1, ALERTA)
    elif forma == "o":
        pintar(img, x, y, OJO)
        pintar(img, x + 1, y, OJO)
        pintar(img, x, y + 1, OJO)
        pintar(img, x + 1, y + 1, OJO)


def exclamacion(img, visible=True):
    if not visible:
        return
    for y in range(2, 7):
        pintar(img, 27, y, ALERTA)
        pintar(img, 28, y, ALERTA)
    pintar(img, 27, 8, ALERTA)
    pintar(img, 28, 8, ALERTA)


def gota(img, paso):
    y = 9 + paso
    pintar(img, 25, y, GOTA)
    pintar(img, 25, y + 1, GOTA)
    pintar(img, 24, y + 1, GOTA)
    pintar(img, 26, y + 1, GOTA)
    pintar(img, 25, y + 2, GOTA)


# ── Animaciones ──────────────────────────────────────────────
def reposo():
    cuadros = []
    for i, (dy, forma) in enumerate([(0, "normal"), (0, "normal"), (1, "normal"), (1, "cerrado"),
                                     (1, "normal"), (0, "normal")]):
        img = lienzo()
        cuerpo(img, dy=dy)
        hoja(img, dy=dy, inclinacion=0 if i % 3 else 1)
        ojos(img, dy=dy, forma=forma)
        mejillas(img, dy=dy)
        boca(img, dy=dy)
        cuadros.append(img)
    return cuadros


def trabajando():
    cuadros = []
    for i, mirar in enumerate([-1, -1, 0, 1, 1, 0]):
        img = lienzo()
        dy = i % 2
        cuerpo(img, dy=dy)
        hoja(img, dy=dy, inclinacion=[-1, 0, 1, 0, -1, 0][i])
        ojos(img, dy=dy, mirar=mirar)
        boca(img, dy=dy, forma="o")
        gota(img, i % 3)
        cuadros.append(img)
    return cuadros


def feliz():
    cuadros = []
    for dy, aplastar in [(1, 1), (0, 0), (-3, 0), (-5, 0), (-3, 0), (0, 0)]:
        img = lienzo()
        cuerpo(img, dy=dy, aplastar=aplastar)
        hoja(img, dy=dy, inclinacion=1 if dy < 0 else 0)
        ojos(img, dy=dy, forma="feliz")
        mejillas(img, dy=dy)
        boca(img, dy=dy, forma="abierta")
        cuadros.append(img)
    return cuadros


def alerta():
    cuadros = []
    for i, dx in enumerate([0, -1, 1, -1, 1, 0]):
        img = lienzo()
        cuerpo(img, dx=dx)
        hoja(img, dx=dx, inclinacion=1)
        ojos(img, dx=dx, forma="grande")
        boca(img, dx=dx, forma="o")
        exclamacion(img, visible=i % 2 == 0 or i == 5)
        cuadros.append(img)
    return cuadros


ESTRELLA = (255, 214, 102, 255)


def estrellita(img, x, y):
    pintar(img, x, y, ESTRELLA)
    pintar(img, x - 1, y, ESTRELLA)
    pintar(img, x + 1, y, ESTRELLA)
    pintar(img, x, y - 1, ESTRELLA)
    pintar(img, x, y + 1, ESTRELLA)


def aturdido():
    """Ojos en X, se tambalea y le giran estrellitas alrededor de la cabeza."""
    cuadros = []
    for i in range(6):
        img = lienzo()
        dx = [0, 1, 1, 0, -1, -1][i]
        cuerpo(img, dx=dx, dy=1)
        hoja(img, dx=dx, dy=1, inclinacion=[1, 1, 0, -1, -1, 0][i])
        ojos(img, dx=dx, dy=1, forma="x")
        boca(img, dx=dx, dy=1, forma="o")
        for k in range(3):
            angulo = (i / 6 + k / 3) * 2 * math.pi
            estrellita(img, round(16 + 11 * math.cos(angulo)), round(7 + 3 * math.sin(angulo)))
        cuadros.append(img)
    return cuadros


# ── PNG sin librerías ────────────────────────────────────────
def guardar_tira(nombre, cuadros):
    ancho, alto = TAM * len(cuadros), TAM
    filas = []
    for y in range(alto):
        fila = bytearray([0])  # filtro "ninguno"
        for cuadro in cuadros:
            for x in range(TAM):
                fila.extend(cuadro[y][x])
        filas.append(bytes(fila))
    datos = zlib.compress(b"".join(filas), 9)

    def bloque(tipo, contenido):
        return (struct.pack(">I", len(contenido)) + tipo + contenido
                + struct.pack(">I", zlib.crc32(tipo + contenido) & 0xFFFFFFFF))

    png = (b"\x89PNG\r\n\x1a\n"
           + bloque(b"IHDR", struct.pack(">IIBBBBB", ancho, alto, 8, 6, 0, 0, 0))
           + bloque(b"IDAT", datos)
           + bloque(b"IEND", b""))
    ruta = os.path.join(CARPETA, f"{nombre}.png")
    with open(ruta, "wb") as f:
        f.write(png)
    print(f"  {nombre}.png  ({len(cuadros)} cuadros)")


if __name__ == "__main__":
    os.makedirs(CARPETA, exist_ok=True)
    print("Generando mascota provisional en src/assets/sylvie/:")
    guardar_tira("reposo", reposo())
    guardar_tira("trabajando", trabajando())
    guardar_tira("feliz", feliz())
    guardar_tira("alerta", alerta())
    guardar_tira("aturdido", aturdido())
