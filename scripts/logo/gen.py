import sys, os
sys.path.insert(0, os.path.dirname(__file__))
from glyphs import Font, path_data, bbox, fmt

FONT = sys.argv[1]
OUT = sys.argv[2]
os.makedirs(OUT, exist_ok=True)
f = Font(FONT)
INK_LIGHT, INK_DARK, ACCENT = "#1d2a2a", "#dde6e2", "#198f7b"
PAPER, PAPER_DARK, RULE = "#f3f5f2", "#172221", "#cbd4cf"


def layout(text, base, tight):
    """(char, x) in font units, as CSS letter-spacing lays it out: `base`
    after the word's letters, `tight` after each ';' (the .s span)."""
    out, x = [], 0
    for ch in text:
        out.append((ch, x))
        x += f.advance(f.cmap[ch]) + (tight if ch == ";" else base)
    return out


def group(items, ox, oy, scale):
    """Paths per colour role and the ink bounding box."""
    word, semi, boxes = [], [], []
    for ch, x in items:
        c = f.contours(f.cmap[ch])
        d = path_data(c, ox + x * scale, oy, scale)
        (semi if ch == ";" else word).append(d)
        boxes.append(bbox(c, ox + x * scale, oy, scale))
    b = (min(b[0] for b in boxes), min(b[1] for b in boxes), max(b[2] for b in boxes), max(b[3] for b in boxes))
    return "".join(word), "".join(semi), b


def write(name, svg):
    open(os.path.join(OUT, name), "w").write(svg)
    print(name, len(svg), "bytes")


# Horizontal logo: tight viewBox around the ink, transparent background.
items = layout("fung;;", -40, -320)
_, _, b = group(items, 0, 0, 1)
w, h = b[2] - b[0], b[3] - b[1]
word, semi, _ = group(items, -b[0], -b[1], 1)
for variant, ink in (("light", INK_LIGHT), ("dark", INK_DARK)):
    write(f"logo-{variant}.svg",
          f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {fmt(w)} {fmt(h)}" role="img" aria-label="fung;;">'
          f'<title>fung;;</title><path fill="{ink}" d="{word}"/><path fill="{ACCENT}" d="{semi}"/></svg>\n')

# Square icon: "f;;" centred by its ink in a rounded tile.
S = 512
scale = 0.62 * S / 1000
items = layout("f;;", -60, -320)
_, _, b = group(items, 0, 0, scale)
ox = (S - (b[2] - b[0])) / 2 - b[0]
oy = (S - (b[3] - b[1])) / 2 - b[1]
word, semi, nb = group(items, ox, oy, scale)
print("icon ink: %.0f%% of the tile wide, %.0f%% tall" % (100 * (nb[2] - nb[0]) / S, 100 * (nb[3] - nb[1]) / S))
r = 0.22 * S
for variant, bg, ink, edge in (("light", PAPER, INK_LIGHT, True), ("dark", PAPER_DARK, INK_DARK, False)):
    border = f'<rect x="4" y="4" width="{S-8}" height="{S-8}" rx="{fmt(r-4)}" fill="none" stroke="{RULE}" stroke-width="8"/>' if edge else ""
    write(f"icon-{variant}.svg",
          f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {S} {S}" role="img" aria-label="fung">'
          f'<title>fung</title><rect width="{S}" height="{S}" rx="{fmt(r)}" fill="{bg}"/>{border}'
          f'<path fill="{ink}" d="{word}"/><path fill="{ACCENT}" d="{semi}"/></svg>\n')
