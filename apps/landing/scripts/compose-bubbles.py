# One-off: composite the two speech bubbles (exact text) onto the generated
# textless base scene -> slides/public/assets/003-004-merged.png
import math
import os
from PIL import Image, ImageDraw, ImageFont

REPO = os.path.abspath(os.path.join(os.path.dirname(__file__), "../../.."))
BASE = f"{REPO}/slides/public/assets/003-004-merged-base.png"
OUT = f"{REPO}/slides/public/assets/003-004-merged.png"
FONT = "/System/Library/Fonts/Supplemental/Comic Sans MS Bold.ttf"

im = Image.open(BASE).convert("RGB")
W, H = im.size  # 1344 x 768
d = ImageDraw.Draw(im)


def wrap(text, font, max_w):
    words, lines, cur = text.split(), [], ""
    for w_ in words:
        t = (cur + " " + w_).strip()
        if d.textlength(t, font=font) <= max_w:
            cur = t
        else:
            lines.append(cur)
            cur = w_
    lines.append(cur)
    return lines


def draw_text_block(cx, cy, lines, font, fill="black", spacing=1.18):
    asc, desc = font.getmetrics()
    lh = (asc + desc) * spacing
    total = lh * len(lines)
    y = cy - total / 2
    for ln in lines:
        lw = d.textlength(ln, font=font)
        d.text((cx - lw / 2, y), ln, font=font, fill=fill)
        y += lh


def starburst(cx, cy, rx, ry, spikes=24, inner=0.80, rot=0.0):
    pts = []
    for i in range(spikes * 2):
        a = rot + math.pi * i / spikes
        r = 1.0 if i % 2 == 0 else inner
        pts.append((cx + math.cos(a) * rx * r, cy + math.sin(a) * ry * r))
    return pts


# ---- Bubble 1: jagged, top-right, from the wall screen --------------------
b1cx, b1cy, b1rx, b1ry = W * 0.775, H * 0.225, W * 0.175, H * 0.205
pts = starburst(b1cx, b1cy, b1rx, b1ry, spikes=22, inner=0.80, rot=0.35)
d.polygon(pts, fill="white", outline="black")
d.line(pts + [pts[0]], fill="black", width=4)
# tail toward the screen (down-left)
d.polygon(
    [(b1cx - b1rx * 0.55, b1cy + b1ry * 0.72), (b1cx - b1rx * 0.95, b1cy + b1ry * 1.45), (b1cx - b1rx * 0.25, b1cy + b1ry * 0.86)],
    fill="white", outline="black",
)
f1 = ImageFont.truetype(FONT, 22)
t1 = "API COSTS SPIKED, AND MERCURY CAN'T PAY — CASH RAN OUT AFTER YESTERDAY'S CEO LAMBORGHINI BUY."
draw_text_block(b1cx, b1cy, wrap(t1, f1, b1rx * 1.30), f1)

# ---- Bubble 2: rounded chat bubble, lower-left, the agent -----------------
x0, y0, x1, y1 = W * 0.025, H * 0.615, W * 0.385, H * 0.955
d.rounded_rectangle([x0, y0, x1, y1], radius=26, fill="white", outline="black", width=4)
# tail pointing up toward the screen
d.polygon([(x1 - 60, y0 + 2), (x1 + 40, y0 - 52), (x1 - 130, y0 + 2)], fill="white")
d.line([(x1 - 60, y0), (x1 + 40, y0 - 52), (x1 - 130, y0)], fill="black", width=4)

fh = ImageFont.truetype(FONT, 26)
f2 = ImageFont.truetype(FONT, 24)
# robot chip + AGENT header
chip_cx, chip_cy, chip_r = x0 + 38, y0 + 36, 16
d.ellipse([chip_cx - chip_r, chip_cy - chip_r, chip_cx + chip_r, chip_cy + chip_r], fill="#1e293b")
d.ellipse([chip_cx - 8, chip_cy - 5, chip_cx - 2, chip_cy + 1], fill="white")
d.ellipse([chip_cx + 2, chip_cy - 5, chip_cx + 8, chip_cy + 1], fill="white")
d.text((chip_cx + chip_r + 10, chip_cy - fh.getmetrics()[0] * 0.62), "AGENT", font=fh, fill="#15803d")

t2 = "To ensure we have enough to pay the bill, shutting down the CLOUD and our building power NOW"
body_cx = (x0 + x1) / 2
body_cy = (y0 + y1) / 2 + 26
draw_text_block(body_cx, body_cy, wrap(t2, f2, (x1 - x0) * 0.88), f2)

im.save(OUT)
print("saved", OUT, im.size)
