"""Build the lily-pad 'land' mask for assets/pond.png.

Pads in Monet's 1906 Water Lilies are the green / pale-green / pink-white
clusters; open water is blue-violet. Writes assets/land.png (white = land)
and a preview overlay to the path given as argv[1].
"""
import sys
import numpy as np
from PIL import Image
from scipy import ndimage

src = Image.open("assets/pond.png").convert("RGB")
a = np.asarray(src).astype(np.float32)
r, g, b = a[..., 0], a[..., 1], a[..., 2]

greenish = (g > b * 0.98) & (g > r * 1.05)            # pad leaves
bright = (r + g + b > 560) & (g > b * 0.9)             # pale pad highlights
blossom = (r > b * 1.05) & (r > g * 1.05) & (r > 120)   # pink / red flowers
raw = greenish | bright | blossom

# Clean up: close gaps inside clusters, drop specks, smooth edges.
m = ndimage.binary_closing(raw, structure=np.ones((3, 3)), iterations=2 if src.width > 400 else 1)
m = ndimage.binary_opening(m, structure=np.ones((2, 2)), iterations=1)
lab, n = ndimage.label(m)
sizes = ndimage.sum(m, lab, range(1, n + 1))
keep = np.isin(lab, [i + 1 for i, s in enumerate(sizes) if s >= 150 * (src.width / 640) ** 2])
keep = ndimage.binary_fill_holes(keep)

Image.fromarray((keep * 255).astype(np.uint8), "L").save("assets/land.png", optimize=True)
print("land fraction", keep.mean().round(3), "clusters", ndimage.label(keep)[1])

prev = a.copy()
prev[keep] = prev[keep] * 0.5 + np.array([255, 120, 0]) * 0.5
Image.fromarray(prev.astype(np.uint8)).resize((src.width * 2, src.height * 2), Image.NEAREST).save(sys.argv[1])
