"""Regenerate the original app icons; requires Pillow."""

from pathlib import Path
from PIL import Image, ImageDraw

output = Path(__file__).resolve().parent.parent / "web" / "assets"
output.mkdir(parents=True, exist_ok=True)
image = Image.new("RGB", (1024, 1024), "#1f2327")
draw = ImageDraw.Draw(image)
draw.rounded_rectangle((235, 260, 675, 810), 36, fill="#da7774")
draw.rounded_rectangle((300, 220, 740, 770), 36, fill="#86cbb3")
draw.rounded_rectangle((365, 180, 805, 730), 36, fill="#f1d879")
draw.polygon([(448, 300), (718, 300), (718, 374), (564, 624), (473, 624), (627, 380), (448, 380)], fill="#25352f")
for size in (192, 512):
    image.resize((size, size), Image.Resampling.LANCZOS).save(output / f"icon-{size}.png")
