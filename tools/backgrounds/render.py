"""Render FluxCode's animated backgrounds into looping pixel videos (animated WebP).

Each scene is a field of brightness and color, sampled on a grid of tiny 4 x 5 pixel
cells and written with a pixel "character ramp" (3 x 4 glyphs from a dot to a full
block), the way an image-to-ASCII generator works, only so small that it reads as a
detailed pixel video. Every motion uses whole turns of one phase, so the last frame
leads straight back into the first.

The app only plays these files (src/scene.rs); nothing is drawn per frame there.

    python -I tools/backgrounds/render.py            # all scenes -> assets/backgrounds/
    python -I tools/backgrounds/render.py ocean      # one scene

Needs numpy and Pillow (with WebP).
"""

import sys
from pathlib import Path

import numpy as np
from PIL import Image

W, H = 960, 540
CW, CH = 4, 5  # cell size in pixels: a 3 x 4 glyph and a 1 px gap
FPS = 10
FRAMES = 50  # 5 s loop
COLS, ROWS = W // CW, H // CH

# Pixel glyphs, darkest to brightest (rows of a 3 x 4 bitmap).
GLYPHS = [
    ["...", "...", "...", "..."],
    ["...", "...", ".#.", "..."],
    ["...", ".#.", "...", ".#."],
    ["...", "#.#", "...", "#.#"],
    [".#.", "###", ".#.", "..."],
    ["#.#", ".#.", "#.#", "..."],
    ["#.#", "###", "#.#", ".#."],
    ["###", "#.#", "###", "#.#"],
    ["###", "###", "#.#", "###"],
    ["###", "###", "###", "###"],
]
MASKS = np.array([[[c == "#" for c in row] for row in g] for g in GLYPHS], dtype=np.float32)  # (L, 4, 3)
LEVELS = len(GLYPHS)

u = (np.arange(COLS) + 0.5)[None, :] / COLS
v = (np.arange(ROWS) + 0.5)[:, None] / ROWS
U, V = np.broadcast_arrays(u, v)
IX, IY = np.meshgrid(np.arange(COLS), np.arange(ROWS))
ASPECT = W / H


def hexc(h):
    return np.array([(h >> 16) & 255, (h >> 8) & 255, h & 255], dtype=np.float32) / 255.0


def mix(a, b, f):
    f = np.clip(f, 0, 1)[..., None]
    return a * (1 - f) + b * f


def hash2(x, y, k=0.0):
    return np.modf(np.abs(np.sin(x * 127.1 + y * 311.7 + k * 74.7) * 43758.5453))[0]


def noise(x, y, k=0.0):
    xi, yi = np.floor(x), np.floor(y)
    fx, fy = x - xi, y - yi
    sx, sy = fx * fx * (3 - 2 * fx), fy * fy * (3 - 2 * fy)
    a = hash2(xi, yi, k) + (hash2(xi + 1, yi, k) - hash2(xi, yi, k)) * sx
    b = hash2(xi, yi + 1, k) + (hash2(xi + 1, yi + 1, k) - hash2(xi, yi + 1, k)) * sx
    return a + (b - a) * sy


def fbm(x, y, k=0.0):
    return 0.5 * noise(x, y, k) + 0.3 * noise(x * 2.03, y * 2.03, k + 1) + 0.2 * noise(x * 4.1, y * 4.1, k + 2)


def ridge(x, seed):
    return 0.55 * np.abs(np.sin(x * 5.4 + seed)) + 0.3 * np.abs(np.sin(x * 12.6 + seed * 1.7)) + 0.15 * np.abs(np.sin(x * 28.2 + seed * 2.9))


def dist(cu, cv):
    return np.sqrt(((U - cu) * ASPECT) ** 2 + (V - cv) ** 2)


def stars(ph, density, k=0.0):
    h = hash2(IX * 1.0, IY * 1.0, 11 + k)
    on = h > 1 - density
    tw = 0.5 + 0.5 * np.abs(np.sin(ph * np.floor(1 + h * 4) + h * 60))
    return np.where(on, tw, 0.0)


def drift(x, y, ph, r=0.35):
    """Sample position moving on a circle: cloud shapes morph and come back each loop."""
    return x + r * np.cos(ph), y + r * np.sin(ph)


def sky_layer(lum, rgb, ph, band, k, tint, amount=1.4, thresh=0.55):
    cx, cy = drift(U * 3.0, V * 9.0, ph)
    c = fbm(cx, cy, k)
    m = (V > band[0]) & (V < band[1]) & (c > thresh)
    lum = np.where(m, lum + (c - thresh) * amount, lum)
    rgb = np.where(m[..., None], mix(rgb, np.broadcast_to(tint, rgb.shape), np.full(lum.shape, 0.6)), rgb)
    return lum, rgb


def scene_ocean(ph, f):
    hz = 0.56
    sky = V < hz
    t = np.clip(V / hz, 0, 1)
    rgb = mix(hexc(0x0B1A3A)[None, None], hexc(0x2B5F8C)[None, None], t)
    lum = 0.08 + 0.16 * t
    d = dist(0.78, 0.22)
    lum = lum + 0.55 * np.exp(-np.maximum(d - 0.06, 0) * 9)
    lum, rgb = sky_layer(lum, rgb, ph, (0.06, 0.38), 5, hexc(0x9DB4D3))
    st = np.where(V < 0.45, stars(ph, 0.012), 0)
    rgb = np.where((st > lum)[..., None], hexc(0xFFFFFF), rgb)
    lum = np.maximum(lum, st)
    moon = d < 0.06
    lum = np.where(moon, 0.95 - 0.25 * fbm(U * 30, V * 30, 3), lum)
    rgb = np.where(moon[..., None], hexc(0xF4F1DE), rgb)
    # Sea.
    s = np.clip((V - hz) / (1 - hz), 0, 1)
    w1 = 0.5 + 0.5 * np.sin(U * 34 * (1 + s * 0.6) - ph * 2 - s * 26)
    w2 = 0.5 + 0.5 * np.sin(U * 11 + ph - (s * 14))
    wave = w1 * 0.6 + w2 * 0.4
    sl = 0.12 + 0.32 * wave * (1 - s * 0.4) + np.where(wave > 0.9, 0.3, 0)
    srgb = mix(hexc(0x2C6E91)[None, None], hexc(0x0A2A40)[None, None], s)
    col = np.abs(U - 0.78) < (0.015 + s * 0.09) * (0.55 + 0.45 * np.sin(V * 95 + ph * 3))
    sl = np.where(col, sl + 0.55 * (1 - s * 0.7), sl)
    srgb = np.where(col[..., None], mix(srgb, np.broadcast_to(hexc(0xF4F1DE), srgb.shape), np.full(s.shape, 0.7)), srgb)
    lum = np.where(sky, lum, sl)
    rgb = np.where(sky[..., None], rgb, srgb)
    return lum, rgb


def scene_river(ph, f):
    t = np.clip(V / 0.6, 0, 1)
    rgb = mix(hexc(0x1B3B4B)[None, None], hexc(0xE8D9A8)[None, None], t)
    lum = 0.15 + 0.3 * t
    d = dist(0.24, 0.28)
    lum = lum + 0.4 * np.exp(-np.maximum(d - 0.05, 0) * 8)
    lum, rgb = sky_layer(lum, rgb, ph, (0.04, 0.4), 7, hexc(0xFFFFFF), 1.8, 0.58)
    sun = d < 0.05
    lum = np.where(sun, 0.95, lum)
    rgb = np.where(sun[..., None], hexc(0xFFF1C1), rgb)
    far = 0.5 - 0.13 * ridge(U, 1.2)
    near = 0.6 - 0.1 * ridge(U * 1.3, 3.4)
    m = V > far
    lum = np.where(m, 0.32 + 0.1 * fbm(U * 20, V * 20, 9), lum)
    rgb = np.where(m[..., None], hexc(0x4F7F78), rgb)
    m = V > near
    lum = np.where(m, 0.22 + 0.12 * fbm(U * 26, V * 26, 13), lum)
    rgb = np.where(m[..., None], hexc(0x2F5F55), rgb)
    k = np.clip((V - 0.6) / 0.4, 0, 1)
    low = V > 0.6
    half = 0.04 + k * 0.41
    water = low & (np.abs(U - 0.5) < half)
    ripple = (0.5 + 0.5 * np.sin(V * 70 - ph * 4 + np.sin(U * 18) * 1.4)) ** 6
    grass = low & ~water
    side = np.where(U < 0.5, U, 1 - U)
    lot = np.floor(side * 18)
    tx = side * 18 - lot
    top = 0.62 + 0.03 * hash2(lot, lot, 3)
    sway = np.sin(ph + lot * 2.0) * 0.03
    width = np.clip((V - top) / 0.14, 0, 1) * 0.45
    tree = grass & (V > top) & (np.abs(tx - 0.5 - sway * (1 - np.clip((V - top) / 0.14, 0, 1))) < width)
    lum = np.where(water, 0.28 + 0.55 * ripple * (0.4 + 0.6 * k), lum)
    rgb = np.where(water[..., None], mix(np.broadcast_to(hexc(0x6FA9C0), rgb.shape), np.broadcast_to(hexc(0xE9F6F8), rgb.shape), ripple), rgb)
    lum = np.where(grass, 0.25 + 0.15 * fbm(U * 30, V * 30, 19), lum)
    rgb = np.where(grass[..., None], hexc(0x3E6B45), rgb)
    lum = np.where(tree, 0.18 + 0.1 * fbm(U * 40, V * 40, 17), lum)
    rgb = np.where(tree[..., None], hexc(0x1F4A3A), rgb)
    return lum, rgb


def scene_beach(ph, f):
    hz = 0.55
    tide = 0.76 + 0.022 * np.sin(ph) + 0.008 * np.sin(U * 14 + ph * 2)
    t = np.clip(V / hz, 0, 1)
    rgb = mix(mix(hexc(0x3B2A6B)[None, None], hexc(0xF08A7E)[None, None], t * 1.3), np.broadcast_to(hexc(0xFFD29A), (ROWS, COLS, 3)), (t - 0.7) * 3)
    lum = 0.2 + 0.45 * t
    d = dist(0.5, hz)
    lum = lum + 0.4 * np.exp(-np.maximum(d - 0.14, 0) * 6)
    lum, rgb = sky_layer(lum, rgb, ph, (0.04, 0.42), 21, hexc(0xFFE3D3), 1.5, 0.56)
    sun = (V < hz) & (d < 0.14)
    gap = (V > hz - 0.07) & (np.sin(V * 160 - ph * 2) > 0.55)
    lum = np.where(sun, np.where(gap, 0.55, 1.0), lum)
    rgb = np.where(sun[..., None], np.where(gap[..., None], hexc(0xF08A7E), hexc(0xFFC46B)), rgb)
    sea = (V >= hz) & (V < tide - 0.012)
    s = np.clip((V - hz) / (tide - hz), 0, 1)
    wave = 0.5 + 0.5 * np.sin(U * 30 - ph * 2 - s * 20)
    sl = 0.2 + 0.3 * wave
    srgb = mix(hexc(0x2D5F8A)[None, None], hexc(0x6FB3C8)[None, None], s)
    glint = np.abs(U - 0.5) < (0.12 - s * 0.06) * (0.6 + 0.4 * np.sin(V * 120 + ph * 3))
    sl = np.where(glint, sl + 0.45, sl)
    srgb = np.where(glint[..., None], mix(srgb, np.broadcast_to(hexc(0xFFC46B), srgb.shape), np.full(s.shape, 0.7)), srgb)
    lum = np.where(sea, sl, lum)
    rgb = np.where(sea[..., None], srgb, rgb)
    foam = (V >= tide - 0.012) & (V < tide + 0.012)
    lum = np.where(foam, 0.92, lum)
    rgb = np.where(foam[..., None], hexc(0xF4F7F8), rgb)
    sand = V >= tide + 0.012
    lum = np.where(sand, 0.42 + 0.25 * fbm(U * 60, V * 60, 23), lum)
    rgb = np.where(sand[..., None], mix(np.broadcast_to(hexc(0xE9C79A), rgb.shape), np.broadcast_to(hexc(0xC99A6A), rgb.shape), (V - tide) * 3), rgb)
    return lum, rgb


def scene_mountains(ph, f):
    rgb = mix(mix(hexc(0x2A2550)[None, None], hexc(0xB58AA8)[None, None], V * 1.8), np.broadcast_to(hexc(0xF6C9A0), (ROWS, COLS, 3)), (V - 0.5) * 2)
    lum = 0.12 + 0.4 * V
    d = dist(0.68, 0.34)
    lum = lum + 0.45 * np.exp(-np.maximum(d - 0.05, 0) * 7)
    st = np.where(V < 0.3, stars(ph, 0.01), 0)
    rgb = np.where((st > lum)[..., None], hexc(0xFFFFFF), rgb)
    lum = np.maximum(lum, st)
    sun = d < 0.05
    lum = np.where(sun, 0.95, lum)
    rgb = np.where(sun[..., None], hexc(0xFFE2B8), rgb)
    layers = [(0.56, 0.3, 0.7, 0x8C7AA6, 0.55), (0.68, 0.3, 2.2, 0x5E517F, 0.38), (0.82, 0.28, 4.1, 0x3A3158, 0.24), (0.98, 0.2, 5.6, 0x221C38, 0.12)]
    for i, (base, amp, seed, color, shade) in enumerate(layers):
        top = base - amp * ridge(U * 0.9, seed)
        m = V > top
        lum = np.where(m, shade + 0.1 * fbm(U * 24, V * 24, 29 + i), lum)
        rgb = np.where(m[..., None], hexc(color), rgb)
        if i < 2:
            snow = m & (V < top + 0.02)
            lum = np.where(snow, 0.85, lum)
            rgb = np.where(snow[..., None], hexc(0xF2E6F0), rgb)
        if i == 1:
            # Fog drifting through the valley behind the nearer ridges.
            fx, fy = drift(U * 2.5, V * 7.0, ph, 0.6)
            fog = fbm(fx, fy, 37)
            fm = (V > 0.55) & (V < 0.82) & (fog > 0.52)
            lum = np.where(fm, lum + (fog - 0.52) * 1.3, lum)
            rgb = np.where(fm[..., None], mix(rgb, np.broadcast_to(hexc(0xF2E6F0), rgb.shape), np.full(lum.shape, 0.5)), rgb)
    return lum, rgb


def scene_anime(ph, f):
    rgb = mix(mix(hexc(0x2B1055)[None, None], hexc(0xD5426B)[None, None], V * 1.8), np.broadcast_to(hexc(0xFFB36B), (ROWS, COLS, 3)), (V - 0.55) * 2.5)
    lum = 0.12 + 0.45 * V
    st = np.where(V < 0.5, stars(ph, 0.015), 0)
    rgb = np.where((st > lum)[..., None], hexc(0xFFFFFF), rgb)
    lum = np.maximum(lum, st)
    d = dist(0.5, 0.6)
    lum = lum + 0.35 * np.exp(-np.maximum(d - 0.22, 0) * 5)
    sun = d < 0.22
    gap = (V > 0.6) & (np.sin(V * 90 - ph * 2) > 0.35 - (V - 0.6) * 4)
    lum = np.where(sun, np.where(gap, 0.3, 0.95), lum)
    sun_rgb = mix(np.broadcast_to(hexc(0xFFE08A), rgb.shape), np.broadcast_to(hexc(0xFF7A6B), rgb.shape), (V - 0.38) * 2)
    rgb = np.where(sun[..., None], np.where(gap[..., None], hexc(0xD5426B), sun_rgb), rgb)
    hill = V > 0.74 - 0.08 * ridge(U * 1.2, 1.9)
    lum = np.where(hill, 0.22, lum)
    rgb = np.where(hill[..., None], hexc(0x5A1E5C), rgb)
    lot = np.floor(U * 22)
    tall = 0.62 + 0.18 * hash2(lot, lot * 0.5, 2)
    bld = V > tall
    lit = bld & (IX % 3 == 1) & (IY % 2 == 0) & (hash2(IX * 1.0, IY * 1.0, 41) > 0.55)
    on = 0.5 + 0.5 * np.sin(ph * np.floor(1 + hash2(IX * 1.0, IY * 1.0, 47) * 2) + hash2(IX * 1.0, IY * 1.0, 43) * 6.283)
    lum = np.where(bld, np.where(lit, 0.45 + 0.5 * on, 0.1), lum)
    rgb = np.where(bld[..., None], np.where(lit[..., None], hexc(0xFFD27A), hexc(0x2A0E3A)), rgb)
    # Petals: each falls once (or twice) per loop, swaying.
    rng = np.random.default_rng(7)
    n = 70
    x0, y0, sp, sw = rng.random(n), rng.random(n), rng.integers(1, 3, n), rng.random(n) * 6.283
    py = ((y0 + sp * f) % 1.0) * ROWS
    px_ = ((x0 + 0.03 * np.sin(ph * sp + sw)) % 1.0) * COLS
    for x, y in zip(px_.astype(int), py.astype(int)):
        if 0 <= y < ROWS and 0 <= x < COLS:
            lum[y, x] = 0.9
            rgb[y, x] = hexc(0xFFC7DD)
    return lum, rgb


SCENES = {"ocean": scene_ocean, "river": scene_river, "beach": scene_beach, "mountains": scene_mountains, "anime": scene_anime}
PAPER = {"ocean": 0x050C1C, "river": 0x0B1B20, "beach": 0x1A1230, "mountains": 0x120F24, "anime": 0x150826}


def frame(name, i):
    f = i / FRAMES
    ph = f * 2 * np.pi
    lum, rgb = SCENES[name](ph, f)
    lum = np.clip(lum, 0, 1)
    level = np.clip(np.round(lum ** 0.9 * (LEVELS - 1)), 0, LEVELS - 1).astype(int)
    ink = np.clip(rgb * (0.55 + 0.7 * lum)[..., None], 0, 1)
    # Glyph masks for every cell, laid out as pixels: (ROWS, 4, COLS, 3) -> (ROWS*4, COLS*3).
    m = MASKS[level]  # (ROWS, COLS, 4, 3)
    cell = np.zeros((ROWS, COLS, CH, CW), dtype=np.float32)
    cell[:, :, : 4, : 3] = m
    pix = cell.transpose(0, 2, 1, 3).reshape(ROWS * CH, COLS * CW)
    color = np.repeat(np.repeat(ink, CH, axis=0), CW, axis=1)
    paper = hexc(PAPER[name])
    # The gaps between glyphs carry a dim wash of the cell's color, so the picture
    # reads as one image rather than letters on black.
    img = color * pix[..., None] + (paper * 0.6 + color * 0.18) * (1 - pix[..., None])
    out = np.zeros((H, W, 3), dtype=np.float32)
    out[: img.shape[0], : img.shape[1]] = img
    return Image.fromarray((out * 255).astype(np.uint8), "RGB")


def render(name, out_dir):
    frames = [frame(name, i) for i in range(FRAMES)]
    path = out_dir / f"{name}.webp"
    # Lossless with frame deltas: pixel art compresses far better this way than lossy
    # (a few hundred KB a scene), and the app only decodes the parts that change.
    frames[0].save(path, save_all=True, append_images=frames[1:], duration=int(1000 / FPS), loop=0, lossless=True, quality=60, method=4, minimize_size=True)
    print(f"{path}: {path.stat().st_size / 1024:.0f} KiB")


def main():
    out_dir = Path(__file__).resolve().parents[2] / "assets" / "backgrounds"
    out_dir.mkdir(parents=True, exist_ok=True)
    names = sys.argv[1:] or list(SCENES)
    for n in names:
        render(n, out_dir)


if __name__ == "__main__":
    main()
