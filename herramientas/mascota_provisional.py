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


LENGUA = (255, 120, 140, 255)


def bocaza(img, dx=0, dy=0, abierta=3):
    """Boca abierta de `abierta` px de alto (0 = cerrada, masticando)."""
    x0, y0 = 13 + dx, 23 + dy
    if abierta <= 0:
        for x in range(x0, x0 + 6):
            pintar(img, x, y0 + 1, OJO)
        return
    for y in range(abierta):
        ancho = 6 if 0 < y < abierta - 1 else 4
        inicio = x0 + (6 - ancho) // 2
        for x in range(inicio, inicio + ancho):
            pintar(img, x, y0 + y, OJO)
    for x in range(x0 + 2, x0 + 4):
        pintar(img, x, y0 + abierta - 1, LENGUA)


def boca_abierta():
    """Esperando el archivo: boca bien abierta, se estira hacia arriba."""
    cuadros = []
    for i, (dy, abierta) in enumerate([(0, 4), (-1, 5), (-2, 5), (-1, 5), (0, 4), (0, 4)]):
        img = lienzo()
        cuerpo(img, dy=dy)
        hoja(img, dy=dy, inclinacion=[1, 0, -1, 0, 1, 0][i])
        ojos(img, dy=dy - 1, forma="grande")
        mejillas(img, dy=dy)
        bocaza(img, dy=dy, abierta=abierta)
        cuadros.append(img)
    return cuadros


def comer():
    """Traga: se aplasta, cierra la boca, mastica y queda feliz."""
    cuadros = []
    pasos = [(1, 1, 5, "grande"), (2, 1, 0, "cerrado"), (1, 0, 0, "cerrado"),
             (2, 1, 0, "cerrado"), (0, 0, 0, "feliz"), (-2, 0, 0, "feliz")]
    for dy, aplastar, abierta, forma in pasos:
        img = lienzo()
        cuerpo(img, dy=dy, aplastar=aplastar)
        hoja(img, dy=dy, inclinacion=1)
        ojos(img, dy=dy, forma=forma)
        mejillas(img, dy=dy)
        if abierta:
            bocaza(img, dy=dy, abierta=abierta)
        else:
            boca(img, dy=dy, forma="sonrisa")
        cuadros.append(img)
    return cuadros


GRIS = (200, 204, 214, 255)
GRIS_OSC = (120, 124, 138, 255)
CRISTAL = (190, 225, 255, 255)
AURICULAR = (60, 64, 80, 255)
ZETA = (190, 200, 255, 255)
DISCO = (225, 228, 238, 255)
DISCO_BRILLO = (170, 210, 255, 255)


def aro(img, cx, cy, r, color, grosor=1.0):
    for y in range(TAM):
        for x in range(TAM):
            d = math.hypot(x + 0.5 - cx, y + 0.5 - cy)
            if r - grosor <= d <= r:
                pintar(img, x, y, color)


def zeta(img, x, y):
    for i in range(4):
        pintar(img, x + i, y, ZETA)
        pintar(img, x + i, y + 3, ZETA)
    pintar(img, x + 2, y + 1, ZETA)
    pintar(img, x + 1, y + 2, ZETA)


def dormir():
    """Sin usarla un rato: ojos cerrados, respira lento y le salen zetas."""
    cuadros = []
    for i in range(6):
        img = lienzo()
        aplastar = 1 if i in (2, 3) else 0
        cuerpo(img, dy=1, aplastar=aplastar)
        hoja(img, dy=1, inclinacion=-1)
        ojos(img, dy=1, forma="cerrado")
        mejillas(img, dy=1)
        boca(img, dy=1, forma="sonrisa")
        # Zetas que suben y se alejan
        zeta(img, 23, 10 - i)
        if i >= 2:
            zeta(img, 27, 13 - (i - 2) * 2)
        cuadros.append(img)
    return cuadros


def disco(img, cx, cy):
    for y in range(TAM):
        for x in range(TAM):
            d = math.hypot(x + 0.5 - cx, y + 0.5 - cy)
            if d <= 2.6:
                pintar(img, x, y, DISCO if d > 0.9 else OJO)
    pintar(img, int(cx), int(cy) - 2, DISCO_BRILLO)


def dj():
    """Cambias de canción muy rápido: se pone audífonos y lanza discos por detrás."""
    cuadros = []
    trayectorias = [[(4, 16), (2, 10)], [(3, 12), (29, 15)], [(2, 8), (28, 10)],
                    [(29, 16), (4, 14)], [(28, 11), (3, 9)], [(27, 7), (2, 5)]]
    for i in range(6):
        img = lienzo()
        for cx, cy in trayectorias[i]:
            disco(img, cx + 0.5, cy + 0.5)          # discos detrás del cuerpo
        dy = [0, -1, 0, -1, 0, -1][i]
        cuerpo(img, dy=dy)
        hoja(img, dy=dy, inclinacion=[1, -1, 1, -1, 1, -1][i])
        ojos(img, dy=dy, forma="feliz")
        mejillas(img, dy=dy)
        boca(img, dy=dy, forma="abierta")
        # Audífonos: diadema y almohadillas
        aro(img, 15.5, 19.5 + dy, 11.5, AURICULAR, grosor=1.2)
        for y in range(TAM):                       # solo la mitad de arriba de la diadema
            for x in range(TAM):
                if img[y][x] == AURICULAR and y > 15 + dy:
                    img[y][x] = T
        for x0 in (4, 26):
            for yy in range(17 + dy, 23 + dy):
                for xx in range(x0, x0 + 3):
                    pintar(img, xx, yy, AURICULAR)
        cuadros.append(img)
    return cuadros


def pensando():
    """Claude trabajando: mira hacia arriba, pensativa, con una nube de ideas."""
    cuadros = []
    for i in range(6):
        img = lienzo()
        dy = 0 if i % 3 else 1
        cuerpo(img, dy=dy)
        hoja(img, dy=dy, inclinacion=1 if i < 3 else 0)
        # Ojos mirando arriba a la derecha
        for ox in (12, 20):
            for yy in range(2):
                pintar(img, ox, 18 + dy + yy, OJO)
                pintar(img, ox + 1, 18 + dy + yy, OJO)
            pintar(img, ox + 1, 18 + dy, BRILLO)
        mejillas(img, dy=dy)
        for x in (15, 16, 17):                     # boca de "mmm"
            pintar(img, x, 24 + dy, OJO)
        # Burbujas de pensamiento que van creciendo
        if i >= 1:
            pintar(img, 24, 11, BRILLO)
        if i >= 2:
            for dx, dyy in ((0, 0), (1, 0), (0, 1), (1, 1)):
                pintar(img, 26 + dx, 8 + dyy, BRILLO)
        if i >= 3:
            for y in range(1, 6):
                for x in range(23, 31):
                    if (x in (23, 30) and y in (1, 5)):
                        continue
                    pintar(img, x, y, BRILLO)
            puntos = min(3, i - 2)
            for k in range(puntos):
                pintar(img, 25 + k * 2, 3, OJO)
        cuadros.append(img)
    return cuadros


def lupa():
    """Viendo el calendario: revisa con una lupa, moviéndola de un lado a otro."""
    cuadros = []
    for i, mx in enumerate([0, 1, 2, 2, 1, 0]):
        img = lienzo()
        cuerpo(img)
        hoja(img, inclinacion=1)
        ojos(img)
        mejillas(img)
        boca(img, forma="o")
        cx, cy = 20.5 + mx - 1, 19.5
        # Cristal (agranda el ojo que hay detrás)
        for y in range(TAM):
            for x in range(TAM):
                if math.hypot(x + 0.5 - cx, y + 0.5 - cy) <= 3.6:
                    pintar(img, x, y, CRISTAL)
        for yy in range(3):
            for xx in range(3):
                pintar(img, int(cx) - 1 + xx, int(cy) - 1 + yy, OJO)
        pintar(img, int(cx) - 1, int(cy) - 1, BRILLO)
        aro(img, cx, cy, 5, GRIS, grosor=1.3)
        # Mango
        for k in range(4):
            pintar(img, int(cx) + 3 + k, int(cy) + 3 + k, GRIS_OSC)
            pintar(img, int(cx) + 4 + k, int(cy) + 3 + k, GRIS_OSC)
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
    guardar_tira("boca", boca_abierta())
    guardar_tira("comer", comer())
    guardar_tira("dormir", dormir())
    guardar_tira("dj", dj())
    guardar_tira("pensando", pensando())
    guardar_tira("lupa", lupa())
