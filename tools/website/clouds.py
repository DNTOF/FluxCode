"""Generate the website's cloud layers (RGBA WebP) procedurally.

Cumulus: soft round puffs joined by a soft union, eroded by fBm noise, shaded by the puff
domes and by how much cloud lies above (light from above, a warm dusk light), cool violet in the shadows, fading out at the
bottom and sides so the layers blend into the sky and the page.
Stratus: thin stretched fBm streaks for the far sky.

Usage (needs numpy, scipy and Pillow with WebP):
    python tools/website/clouds.py website/img/sky
"""
import sys
import numpy as np
from PIL import Image

OUT = sys.argv[1]


def fbm(w, h, scale, octaves, seed, stretch=1.0, gain=0.5):
    rng = np.random.default_rng(seed)
    acc = np.zeros((h, w), np.float32)
    amp, tot = 1.0, 0.0
    for o in range(octaves):
        f = 2 ** o
        gw = max(2, int(w / scale * f / stretch))
        gh = max(2, int(h / scale * f))
        g = rng.random((gh, gw)).astype(np.float32)
        img = Image.fromarray(g, "F").resize((w, h), Image.BICUBIC)
        acc += amp * np.asarray(img, np.float32)
        tot += amp
        amp *= gain
    return acc / tot


def smooth(a, b, x):
    t = np.clip((x - a) / (b - a), 0, 1)
    return t * t * (3 - 2 * t)


def blur(a, r):
    from scipy.ndimage import gaussian_filter
    return gaussian_filter(a.astype(np.float32), r)


def save(rgb, alpha, name, q=74):
    a = np.clip(alpha, 0, 1)
    arr = np.dstack([np.clip(rgb, 0, 255), a * 255]).astype(np.uint8)
    Image.fromarray(arr, "RGBA").save(f"{OUT}/{name}", "WEBP", quality=q, method=6, alpha_quality=85)


LIT = np.array([255, 228, 190], np.float32)     # warm dusk light on the tops
SKY = np.array([168, 160, 205], np.float32)     # sky bounce on the sides
SHADOW = np.array([46, 50, 84], np.float32)     # cool underside


def puffs(w, h, seed, n, rmin, rmax, ybias, xrange=(0.0, 1.0)):
    """Sum of soft gaussian puffs; big ones sit low, small ones crown the top."""
    rng = np.random.default_rng(seed)
    field = np.zeros((h, w), np.float32)
    # A slow ridge profile so the bank rises into towers in places.
    knots = rng.random(9)
    prof = lambda x: np.interp(x / w * 8, np.arange(9), knots) ** 2
    for _ in range(n):
        r = rng.uniform(rmin, rmax)
        cx = rng.uniform(*xrange) * w
        t = (r - rmin) / (rmax - rmin + 1e-6)
        cy = h * ybias - prof(cx) * rmax * 1.8 * (1 - t) ** 0.5 + (1 - t) * rng.uniform(-0.9, -0.2) * rmax + t * rng.uniform(-0.1, 0.4) * rmax
        x0, x1 = int(max(0, cx - 3 * r)), int(min(w, cx + 3 * r))
        y0, y1 = int(max(0, cy - 3 * r)), int(min(h, cy + 3 * r))
        if x1 <= x0 or y1 <= y0:
            continue
        yy, xx = np.mgrid[y0:y1, x0:x1].astype(np.float32)
        d2 = ((xx - cx) ** 2 + (yy - cy) ** 2) / (r * r)
        field[y0:y1, x0:x1] += np.exp(-d2 * 1.2) ** 3
    return np.cbrt(field)


def cumulus(w, h, seed, n, rmin, rmax, ybias, name, xrange=(0.0, 1.0), fade=(0.06, 0.06), bottom=0.62, warm=1.0, absorb=3.0):
    f = puffs(w, h, seed, n, rmin, rmax, ybias, xrange)
    noise = fbm(w, h, rmax * 0.6, 6, seed + 1)
    fine = fbm(w, h, rmax * 0.15, 4, seed + 2)
    dens = f + (noise - 0.5) * 0.35 + (fine - 0.5) * 0.22
    a = smooth(0.30, 0.55, dens)
    # Dome shading: normals from the smooth puff field, light from above and a little right.
    hs = blur(f, rmax * 0.12) * rmax * 1.1
    gy, gx = np.gradient(hs)
    nz = np.ones_like(gx)
    norm = np.sqrt(gx * gx + gy * gy + nz * nz)
    lx, ly, lz = 0.35, -0.8, 0.5
    dome = np.clip((-gx * lx - gy * ly + nz * lz) / norm, 0, 1)
    # Light from above: the more cloud above a pixel, the darker it is (cheap volumetric).
    tau = np.cumsum(np.clip(blur(dens, rmax * 0.1), 0, 1.5), axis=0) / h
    light = np.exp(-tau * absorb) * (0.35 + 0.75 * dome)
    light = np.clip(light + (fine - 0.5) * 0.12 + (1 - a) * 0.3, 0, 1)
    lit = LIT * warm + SKY * (1 - warm)
    l3 = light[..., None]
    rgb = SHADOW * (1 - l3) + lit * l3
    mid = (np.clip(1 - np.abs(light - 0.5) * 2.0, 0, 1) * 0.3)[..., None]
    rgb = rgb * (1 - mid) + SKY * mid
    xs = np.arange(w, dtype=np.float32)[None, :]
    ys = np.arange(h, dtype=np.float32)[:, None]
    a = a * smooth(0, w * max(fade[0], 1e-3), xs) * smooth(w, w * (1 - fade[1]), xs)
    a *= smooth(h, h * bottom, ys) * smooth(0, h * 0.04, ys)
    save(rgb, a, name)


def streaks(w, h, seed, name):
    ys = np.arange(h, dtype=np.float32)[:, None]
    xs = np.arange(w, dtype=np.float32)[None, :]
    n = fbm(w, h, 300, 6, seed, stretch=5.0)
    n2 = fbm(w, h, 80, 4, seed + 7, stretch=3.0)
    band = smooth(0, h * 0.4, ys) * smooth(h, h * 0.6, ys) * smooth(0, w * 0.08, xs) * smooth(w, w * 0.92, xs)
    d = smooth(0.5, 0.8, n * 0.85 + n2 * 0.25) * band
    l = np.clip(0.55 + (d - blur(d, 6)) * 3 + (n2 - 0.5), 0, 1)
    rgb = SKY * (1 - l[..., None]) + LIT * l[..., None]
    save(rgb, d * 0.8, name)


if __name__ == "__main__":
    streaks(2000, 360, 11, "cloud-far.webp")
    cumulus(2400, 560, 23, 160, 46, 120, 0.7, "cloud-mid.webp", fade=(0.04, 0.04), bottom=0.62, warm=0.7)
    cumulus(1500, 820, 5, 110, 70, 200, 0.7, "cloud-front.webp", xrange=(0.0, 0.72), fade=(0.0, 0.28), bottom=0.78)
