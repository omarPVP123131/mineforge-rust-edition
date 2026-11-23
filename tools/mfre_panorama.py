#!/usr/bin/env python3
"""MFRE (Fase 12): the fork's own title panorama, six faces plus the overlay.

The classic menu background ships from the player's Mojang assets; the fork
replaces it at prepare time with its own (TRAYECTORIA's brand notice: zero
Mojang assets in the fork's branding). These faces are a sober forge night:
a vertical gradient with an ember horizon, a few sparks, and the M mark as a
faint watermark on face 0. Every face is horizontally uniform apart from the
margin-kept sparks, so the cubemap is seamless as the title screen rotates.

Run from the repo root:  python3 tools/mfre_panorama.py
Writes crates/horizon-prepare/src/mfre_panorama/{panorama_0..5,panorama_overlay}.png
which `mfre.rs` embeds and `horizon-prepare` writes over the classic faces.
"""
import math
import os
import random

from PIL import Image, ImageDraw

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(REPO, "crates", "horizon-prepare", "src", "mfre_panorama")
SIZE = 1024

# Vertical gradient stops: (y fraction, rgb).
STOPS = [
    (0.00, (7, 8, 12)),
    (0.45, (16, 19, 26)),
    (0.58, (42, 22, 12)),
    (0.630, (124, 45, 18)),
    (0.648, (249, 115, 22)),   # the ember horizon, the logo's spark
    (0.660, (124, 45, 18)),
    (0.75, (26, 18, 16)),
    (1.00, (12, 13, 18)),
]

# The M mark on the 16 grid, as in docs/media/brand/icon.svg.
STEMS = [(3, 2, 2, 12), (11, 2, 2, 12), (5, 4, 2, 2), (9, 4, 2, 2)]
SPARK = (7, 6, 2, 3)


def ramp(y):
    for (y0, c0), (y1, c1) in zip(STOPS, STOPS[1:]):
        if y0 <= y <= y1:
            t = (y - y0) / (y1 - y0)
            return tuple(round(c0[i] + (c1[i] - c0[i]) * t) for i in range(3))
    return STOPS[-1][1]


def face(index):
    import numpy as np
    ys = np.linspace(0.0, 1.0, SIZE)
    cols = np.array([ramp(y) for y in ys], dtype=np.uint8)          # (SIZE,3)
    arr = np.repeat(cols[:, None, :], SIZE, axis=1)                 # (SIZE,SIZE,3)
    img = Image.fromarray(arr, "RGB")
    draw = ImageDraw.Draw(img, "RGBA")
    # Sparks: deterministic per face, kept off the edges so the cube is seamless.
    rnd = random.Random(1700 + index)
    for _ in range(42):
        x = rnd.randint(24, SIZE - 25)
        y = rnd.randint(int(SIZE * 0.30), int(SIZE * 0.62))
        r = rnd.choice([1, 1, 2])
        a = rnd.randint(50, 120)
        draw.ellipse([x - r, y - r, x + r, y + r], fill=(251, 146, 60, a))
    if index == 0:
        # The mark, faint, upper centre: it passes once per rotation.
        u = 32.0
        ox, oy = 256.0, 96.0
        for x, y, w, h in STEMS + [SPARK]:
            draw.rectangle(
                [ox + x * u, oy + y * u, ox + (x + w) * u, oy + (y + h) * u],
                fill=(236, 237, 243, 16),
            )
    return img


def overlay():
    import numpy as np
    g = np.arange(SIZE, dtype=np.float64)
    x, y = np.meshgrid(g, g)
    d = np.hypot(x - SIZE / 2, y - SIZE / 2) / (SIZE / 2 * math.sqrt(2))
    a = (np.clip((d - 0.35) / 0.65, 0.0, 1.0) * 150).astype(np.uint8)
    arr = np.empty((SIZE, SIZE, 4), dtype=np.uint8)
    arr[..., 0], arr[..., 1], arr[..., 2] = 4, 5, 10
    arr[..., 3] = a
    return Image.fromarray(arr, "RGBA")


def main():
    os.makedirs(OUT, exist_ok=True)
    for i in range(6):
        p = os.path.join(OUT, f"panorama_{i}.png")
        face(i).save(p, compress_level=9)
        print("wrote", os.path.relpath(p, REPO), os.path.getsize(p), "B")
    p = os.path.join(OUT, "panorama_overlay.png")
    overlay().save(p, compress_level=9)
    print("wrote", os.path.relpath(p, REPO), os.path.getsize(p), "B")


if __name__ == "__main__":
    main()
