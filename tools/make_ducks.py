"""Duck sprite sheet from the hand-drawn templates in duck_art.py.

Writes assets/ducks.png (row = breed*2 + baby, columns = frames) and
src/sprite_meta.rs. Optional argv[1]: a 6x preview.

Animation is derived from each template: blink closes the eyes, the waddle
lifts alternate feet and bobs the body, swimming sinks the duck to a
waterline, pecking / sleeping / quacking move the head.
"""
import sys
from PIL import Image
import os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from duck_art import ADULT, BABY, EAT, SLEEP

W, H = 20, 18          # the duckling template size
OUT_W, OUT_H = 30, 27  # sheet cell = the grown-up template size

BREEDS = {
    "pekin":        dict(body="#FFF8EC", bsh="#E6D6C0", head="#FFFBF2", wing="#F4E8D8", wsh="#D8C4AA",
                         beak="#FFA62B", ksh="#E07A12", feet="#FF9A30", cheek="#FFB0B6"),
    "mallard_drake": dict(body="#DCD6CC", bsh="#B4AA9C", head="#2F9460", hsh="#1D6E44", wing="#A89C8C", wsh="#857A6C",
                          beak="#F3D04A", ksh="#C9A52A", feet="#FF8C30", cheek="#6FBF8A", chest="#8E553A",
                          collar="#FFFFFF", spec="#3E63D8", tail="#33343C"),
    "mallard_hen":  dict(body="#C79D6E", bsh="#9C7550", head="#BC946A", wing="#A8825A", wsh="#7F6040",
                         beak="#E8913C", ksh="#A2602A", feet="#F28C3A", cheek="#F2A6A0", speck="#8A6440",
                         stripe="#6B4E33", spec="#3E63D8"),
    "wood_duck":    dict(crest="#3F9A70", eyec="#E0353F", body="#DCC490", bsh="#B49E6C", head="#4B3592", hsh="#2C7D5C", wing="#3D4A6E", wsh="#2A3450",
                         beak="#F08A50", ksh="#C8663A", feet="#F0A040", cheek="#F6A0B4", chest="#A65A50",
                         stripe="#FFFFFF", eyering="#E4553C"),
    "mandarin":     dict(body="#E0904C", bsh="#B86E34", head="#F4ECDF", hsh="#3E7E5E", wing="#6C4B8C", wsh="#4E336A",
                         beak="#E8443A", ksh="#B02A22", feet="#F09A40", cheek="#F28C28", chest="#6E3C7E",
                         sail="#F59A2E", stripe="#F28C28"),
    "cayuga":       dict(body="#2E3644", bsh="#1F2530", head="#30413E", hsh="#3F7A66", wing="#283040", wsh="#1A2030",
                         beak="#3A3E3C", ksh="#262A28", feet="#3A3A3A", cheek="#B66E80", sheen="#4E8A76"),
    "blue_swede":   dict(body="#7088AC", bsh="#566A8E", head="#6880A4", wing="#627AA0", wsh="#4A5E80",
                         beak="#6E8C5E", ksh="#526C44", feet="#4A4A52", cheek="#E3A0B4", chest="#F4F4F2"),
    "lavender":     dict(body="#C2B0DE", bsh="#9F8CC2", head="#CBBBE6", wing="#B3A0D4", wsh="#8E7AB4",
                         beak="#F4A85C", ksh="#D07E36", feet="#F49A52", cheek="#FF9EC0"),
    "choc_runner":  dict(body="#7C5438", bsh="#5C3D28", head="#86603F", wing="#6E4A30", wsh="#523622",
                         beak="#45463C", ksh="#2E2F28", feet="#3C3830", cheek="#D8908A", tall=True),
}

# Duckling fluff for every breed: soft, no adult markings, a cap / bib hint
# where the real ducklings have one. White Pekins hatch bright yellow.
BABY_PAL = {
    "pekin":         dict(body="#FFDF40", bsh="#F2B820", head="#FFE760", wing="#FFD436", wsh="#E8A818",
                          beak="#FF8F3A", ksh="#DC6A1A", feet="#FF8A2A", cheek="#FF8EA4"),
    "mallard_drake": dict(body="#F4D35E", bsh="#C99E3E", head="#F8DE74", wing="#8E6A3C", wsh="#6E5030",
                          beak="#6E5A3A", ksh="#4E4028", feet="#C8A040", cheek="#FF9CA8", cap="#7A5A32"),
    "mallard_hen":   dict(body="#EED070", bsh="#C49A44", head="#F4DA80", wing="#9A7444", wsh="#765834",
                          beak="#7A6040", ksh="#58442C", feet="#CFA048", cheek="#FFA0A8", cap="#8A6438"),
    "wood_duck":     dict(body="#B8A486", bsh="#8E7C62", head="#E8DCC0", wing="#7E6C56", wsh="#5E5040",
                          beak="#6A5E50", ksh="#4A4038", feet="#9A8C6A", cheek="#FFA6B0", cap="#6E5E4C"),
    "mandarin":      dict(body="#C4AC86", bsh="#9C8664", head="#F0E6CE", wing="#86704E", wsh="#665438",
                          beak="#E08A60", ksh="#B86A44", feet="#C8A070", cheek="#FF9AA0", cap="#7C6648"),
    "cayuga":        dict(body="#3A3A40", bsh="#26262C", head="#44444C", wing="#30303A", wsh="#1E1E26",
                          beak="#2E2E30", ksh="#1C1C1E", feet="#34343A", cheek="#E08AA0", bib="#F6D658"),
    "blue_swede":    dict(body="#5E6878", bsh="#46505E", head="#6A7484", wing="#525C6C", wsh="#3C4452",
                          beak="#4A5048", ksh="#343A34", feet="#3E3E44", cheek="#F0A0B8", bib="#F8E27A"),
    "lavender":      dict(body="#E6DAF6", bsh="#C8B6E6", head="#EEE4FA", wing="#DACCF0", wsh="#B8A4DA",
                          beak="#FFB878", ksh="#E89456", feet="#FFAE6A", cheek="#FF9CC8"),
    "choc_runner":   dict(body="#8C6448", bsh="#6A4A34", head="#E8D2B0", wing="#7A563C", wsh="#5A3E2A",
                          beak="#4E4A40", ksh="#34322A", feet="#5A5044", cheek="#FFA0A0", cap="#7A563C"),
}


PRETTY = {
    "pekin": "White (Pekin)", "wood_duck": "Wood duck",
    "mallard_drake": "Mallard", "mallard_hen": "Mallard hen", "mandarin": "Mandarin",
    "cayuga": "Cayuga", "blue_swede": "Blue Swedish", "lavender": "Lavender",
    "choc_runner": "Chocolate runner",
}
# Side frames then 4 views x 8 frames, same layout the app expects.
SIDE = ["idle", "blink", "walk1", "walk2", "walk3", "walk4", "swim1", "swim2",
        "peck1", "peck2", "sleep", "quack", "dabble", "sleep2", "sleepw", "sleepw2",
        "munch", "munch2", "munchw", "munchw2", "dabble2", "flap", "squash"]
VIEWS = ["s", "se", "ne", "n"]
VIEW_FR = ["idle", "blink", "walk1", "walk2", "walk3", "walk4", "swim1", "swim2", "flap", "squash"]

HEAD = set("HceEPYy")


def hexc(h, a=255):
    h = h.lstrip("#")
    return (int(h[0:2], 16), int(h[2:4], 16), int(h[4:6], 16), a)


def shade(c, f):
    return tuple(max(0, min(255, int(v * f))) for v in c[:3]) + (c[3],)


def outline_of(c):
    r, g, b, _ = c
    return (int(r * 0.38 + 12), int(g * 0.34 + 10), int(b * 0.42 + 28), 255)


def grid(rows):
    return [list(r) for r in rows]


def blank():
    return [["."] * W for _ in range(H)]


def move(g, letters, dx, dy):
    H, W = len(g), len(g[0])
    out = [row[:] for row in g]
    moved = []
    for y in range(H):
        for x in range(W):
            if g[y][x] in letters:
                moved.append((x + dx, y + dy, g[y][x]))
                out[y][x] = "."
    for x, y, c in moved:
        if 0 <= x < W and 0 <= y < H:
            out[y][x] = c
    return out


def close_eyes(g):
    H, W = len(g), len(g[0])
    out = [row[:] for row in g]
    for y in range(H):
        for x in range(W):
            if g[y][x] in "eE":
                below = y + 1 < H and g[y + 1][x] in "eE"
                out[y][x] = "H" if below else "L"
    return out


def feet_rows(g):
    H, W = len(g), len(g[0])
    return [y for y in range(H) if any(c in "fF" for c in g[y])]


def walk(g, k):
    """k: 0 stand, 1 lift first foot, 2 bob, 3 lift second foot, 4 bob."""
    H, W = len(g), len(g[0])
    legs = sorted({x for y in range(H) for x in range(W) if g[y][x] == "f"})
    if not legs:
        return g
    first, second = legs[0], legs[-1]
    out = [row[:] for row in g]
    if k in (2, 4):
        body = [row[:] for row in g]
        for y in range(H):
            for x in range(W):
                if body[y][x] in "fF":
                    body[y][x] = "."
        body = move(body, set("HceEPYyBbCTWwL"), 0, -1)
        for y in range(H):
            for x in range(W):
                if g[y][x] in "fF":
                    body[y][x] = g[y][x]
        # stretch legs to meet the lifted body
        top = min(y for y in range(H) for x in range(W) if g[y][x] == "f")
        for lx in (first, second):
            if body[top - 1][lx] == ".":
                body[top - 1][lx] = "f"
        return body
    if k in (1, 3):
        lx = first if k == 1 else second
        # the lifted foot: drop its bottom pixels, raise the foot one row
        fy = max(y for y in range(H) for x in range(W) if g[y][x] == "F")
        for x in range(W):
            if g[fy][x] == "F" and abs(x - lx) <= 1:
                out[fy][x] = "."
                out[fy - 1][x] = "F"
    return out


def body_bottom(g):
    H, W = len(g), len(g[0])
    return max(y for y in range(H) for x in range(W) if g[y][x] in "BbCWwT")


def swim(g, phase):
    H, W = len(g), len(g[0])
    wb = body_bottom(g)
    s = [[c if c not in "fF" else "." for c in row] for row in g]
    s = move(s, set("HceEPYyBbCTWwL"), 0, 2 + (1 if phase else 0) * 0)
    if phase:
        s = move(s, HEAD, 0, 1) if False else s
    for y in range(wb, H):
        for x in range(W):
            s[y][x] = "."
    return s, wb


def quack(g):
    H, W = len(g), len(g[0])
    out = [row[:] for row in g]
    ys = [y for y in range(H) for x in range(W) if g[y][x] == "Y"]
    if not ys:
        return out
    y0 = min(ys)
    xs = [x for x in range(W) if g[y0][x] == "Y"]
    for x in xs:
        out[y0][x] = "m" if x != xs[0] else "Y"
        if y0 - 1 >= 0 and g[y0 - 1][x] == ".":
            out[y0 - 1][x] = "Y"
    return out


def quack_open(g):
    """Grown-ups: the lower jaw drops a pixel, showing a dark mouth."""
    H, W = len(g), len(g[0])
    out = [row[:] for row in g]
    jaw = [(x, y) for y in range(H) for x in range(W) if g[y][x] == "y"]
    if not jaw:
        return quack(g)
    hinge = min(x for x, _ in jaw)
    for x, y in jaw:
        if x != hinge:
            out[y][x] = "m"
    for x, y in jaw:
        if y + 1 < H and out[y + 1][x] == ".":
            out[y + 1][x] = "y"
    return out


def breathe(g):
    """Breathing in: the whole loaf rises a pixel, the bottom stays put."""
    H, W = len(g), len(g[0])
    up = move(g, set("HceEPYyBbCTWwLm"), 0, -1)
    for x in range(W):
        if g[H - 1][x] != ".":
            up[H - 1][x] = g[H - 1][x]
    return up


def happy(g):
    """^ ^ eyes: a little arch where the open eye was."""
    H, W = len(g), len(g[0])
    out = [row[:] for row in g]
    eyes = [(x, y) for y in range(H) for x in range(W) if g[y][x] in "eE"]
    if not eyes:
        return out
    x0, y0 = min(x for x, _ in eyes), min(y for _, y in eyes)
    for x, y in eyes:
        out[y][x] = "H"
    for x, y in ((x0 - 1, y0 + 1), (x0, y0), (x0 + 1, y0), (x0 + 2, y0 + 1)):
        if 0 <= x < W and out[y][x] in "Hc":
            out[y][x] = "L"
    return out


def dip_to_water(g):
    """Lower the head until the bill tip just touches the waterline that
    swim() will cut at (body bottom, after its 2 px sink)."""
    H, W = len(g), len(g[0])
    tip = max(y for y in range(H) for x in range(W) if g[y][x] in "Yy")
    dy = (body_bottom(g) - 3) - tip
    return move(g, HEAD, 0, dy) if dy > 0 else g


def side_frames(t, sleep_t, eat):
    g = grid(t)
    big = len(g) > 18
    f = {}
    f["idle"] = g
    f["blink"] = close_eyes(g)
    for k in range(1, 5):
        f[f"walk{k}"] = walk(g, k)
    f["swim1"] = swim(g, 0)
    f["swim2"] = swim(move(g, HEAD, 0, 1), 1)
    # eating: dip, bill on the bread (eyes shut in concentration), then a
    # happy munch with ^ ^ eyes
    half, down = grid(eat["half"]), grid(eat["down"])
    f["peck1"] = half
    f["peck2"] = close_eyes(down)
    dip = close_eyes(dip_to_water(half))
    f["dabble"] = swim(dip, 0)
    f["dabble2"] = swim(move(dip, HEAD, 0, -1), 0)
    munch = happy(g)
    munch2 = quack_open(munch) if big else quack(munch)
    f["munch"], f["munch2"] = munch, munch2
    f["munchw"], f["munchw2"] = swim(munch, 0), swim(munch2, 0)
    sl = grid(sleep_t)
    f["sleep"] = sl
    f["sleep2"] = breathe(sl)
    f["sleepw"] = swim(sl, 0)
    f["sleepw2"] = swim(breathe(sl), 0)
    f["quack"] = quack_open(g) if big else quack(g)
    f.update(hop_frames(g, False))
    return f


def bill_tip(cell):
    """Where the bill meets the bread: (x, y) in the cell. On land the
    lowest, most forward bill pixel; on water the waterline under it."""
    g, water = (cell if isinstance(cell, tuple) else (cell, None))
    H, W = len(g), len(g[0])
    pts = [(y, x) for y in range(H) for x in range(W) if g[y][x] in "Yym"]
    y, x = max(pts)
    return (x, water if water is not None else y)


def raise_wings(g, flare):
    """Wings up for a hop: lifted over the back (and out to the sides when
    seen from the front or back). The body fills in underneath."""
    H, W = len(g), len(g[0])
    lift = 4 if H > 18 else 2
    spread = 2 if H > 18 else 1
    wing = [(x, y, g[y][x]) for y in range(H) for x in range(W) if g[y][x] in "Ww"]
    if not wing:
        return [row[:] for row in g]
    out = [row[:] for row in g]
    for x, y, _ in wing:
        out[y][x] = "B"
    mid = (min(x for x, _, _ in wing) + max(x for x, _, _ in wing)) / 2
    for x, y, c in wing:
        dx = (-spread if x < mid else spread) if flare else 0
        nx, ny = x + dx, y - lift
        if 0 <= nx < W and 0 <= ny < H and out[ny][nx] not in "HceEPYyLm":
            out[ny][nx] = c
    return out


def squash(g):
    """Crouched: everything above the feet sinks a pixel."""
    return move(g, set("HceEPYyBbCTWwLm"), 0, 1)


def hop_frames(g, flare):
    return {"flap": happy(raise_wings(g, flare)), "squash": squash(g)}


def view_frames(t, flare=False):
    g = grid(t)
    f = {"idle": g, "blink": close_eyes(g)}
    for k in range(1, 5):
        f[f"walk{k}"] = walk(g, k)
    f["swim1"] = swim(g, 0)
    f["swim2"] = swim(move(g, HEAD, 0, 1), 1)
    f.update(hop_frames(g, flare))
    return f


def colourise(cell, pal, baby):
    g, water = (cell if isinstance(cell, tuple) else (cell, None))
    H, W = len(g), len(g[0])
    body = pal["body"]
    head = pal["head"]
    col = {
        "H": head,
        "c": pal.get("cap", pal.get("crest", head)),
        "e": (255, 255, 255, 255),
        "E": pal.get("eyec", (30, 20, 40, 255)),
        "L": shade(head, 0.45),
        "P": pal["cheek"],
        "Y": pal["beak"],
        "y": pal["ksh"],
        "m": shade(pal["ksh"], 0.5),
        "B": body,
        "b": pal["bsh"],
        "C": pal.get("chest", body),
        "T": pal.get("tail", pal["bsh"]),
        "W": pal["wing"],
        "w": pal["wsh"],
        "f": shade(pal["feet"], 0.85),
        "F": pal["feet"],
    }
    if baby and "bib" in pal:
        col["C"] = pal["bib"]
    img = [[(0, 0, 0, 0)] * W for _ in range(H)]
    for y in range(H):
        for x in range(W):
            k = g[y][x]
            if k == ".":
                continue
            c = col[k]
            # soft line where the body tucks under the head
            if k in "BbCWw" and y > 0 and g[y - 1][x] in "HPEe":
                c = shade(c, 0.82)
            img[y][x] = c
    filled = [[img[y][x][3] > 0 for x in range(W)] for y in range(H)]
    out = [row[:] for row in img]
    for y in range(H):
        for x in range(W):
            if filled[y][x] or (water is not None and y >= water):
                continue
            nb = [img[ny][nx] for nx, ny in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1))
                  if 0 <= nx < W and 0 <= ny < H and filled[ny][nx]]
            if nb:
                out[y][x] = outline_of(min(nb, key=lambda c: c[0] + c[1] + c[2]))
    return out


def anchors(cell):
    g = cell[0] if isinstance(cell, tuple) else cell
    H, W = len(g), len(g[0])
    hp = [(x, y) for y in range(H) for x in range(W) if g[y][x] in "Hc"]
    ep = [(x, y) for y in range(H) for x in range(W) if g[y][x] in "eEL"]
    bp = [(x, y) for y in range(H) for x in range(W) if g[y][x] in "Yy"]
    beak = max(bp) if bp else (15, 6)
    if not hp:
        return dict(head_top=(10, 4), eye=(12, 6), head=(10, 6), beak=beak)
    top = min(y for _, y in hp)
    tx = sorted(x for x, y in hp if y == top)
    cx = sum(x for x, _ in hp) // len(hp)
    cy = sum(y for _, y in hp) // len(hp)
    e = ep[0] if ep else (cx, cy)
    return dict(head_top=(tx[len(tx) // 2], top), eye=e, head=(cx, cy), beak=beak)


# --- sizing -----------------------------------------------------------------

def duckling(cell):
    """A 20x18 frame placed, unchanged, at the bottom-middle of a 30x27 cell."""
    g, water = (cell if isinstance(cell, tuple) else (cell, None))
    dx, dy = (OUT_W - W) // 2, OUT_H - H
    out = [["."] * OUT_W for _ in range(OUT_H)]
    for y in range(H):
        for x in range(W):
            out[y + dy][x + dx] = g[y][x]
    return (out, water + dy) if water is not None else out


def main():
    names = list(BREEDS)
    rows = [(b, baby) for b in names for baby in (False, True)]
    frames_by_set = {}
    for label, art in (("adult", ADULT), ("baby", BABY)):
        sf = side_frames(art["side"], SLEEP[label], EAT[label])
        cells = [sf[n] for n in SIDE]
        for v in VIEWS:
            vf = view_frames(art[v], flare=v in ("s", "n"))
            cells += [vf[n] for n in VIEW_FR]
        frames_by_set[label] = cells if label == "adult" else [duckling(c) for c in cells]
    ncols = len(frames_by_set["adult"])
    sheet = Image.new("RGBA", (OUT_W * ncols, OUT_H * len(rows)), (0, 0, 0, 0))
    metas = []
    for ri, (b, baby) in enumerate(rows):
        src = BABY_PAL[b] if baby else BREEDS[b]
        pal = {k: (hexc(v) if isinstance(v, str) else v) for k, v in src.items()}
        cells = frames_by_set["baby" if baby else "adult"]
        row_meta = []
        for ci, cell in enumerate(cells):
            img = colourise(cell, pal, baby)
            for y in range(OUT_H):
                for x in range(OUT_W):
                    sheet.putpixel((ci * OUT_W + x, ri * OUT_H + y), img[y][x])
            row_meta.append(anchors(cell))
        metas.append(row_meta)
    sheet.save("assets/ducks.png", optimize=True)

    with open("src/sprite_meta.rs", "w") as fh:
        fh.write("// @generated by tools/make_ducks.py - do not edit\n")
        fh.write(f"pub const CELL_W: u32 = {OUT_W};\npub const CELL_H: u32 = {OUT_H};\n")
        fh.write(f"pub const BREEDS: [&str; {len(names)}] = {names!r};\n".replace("'", '"'))
        pretty = [PRETTY[n] for n in names]
        fh.write(f"pub const BREED_NAMES: [&str; {len(names)}] = {pretty!r};\n".replace("'", '"'))
        fh.write("#[allow(dead_code)]\n#[derive(Clone, Copy, Debug, PartialEq, Eq)]\npub enum Frame {\n")
        for n in SIDE:
            fh.write(f"    {n.capitalize()},\n")
        fh.write("}\n")
        fh.write("#[derive(Clone, Copy)]\npub struct Anchor { pub head_top: (i32, i32), pub eye: (i32, i32), pub head: (i32, i32), pub beak: (i32, i32) }\n")
        fh.write(f"/// Directional views start at this column: VIEW_BASE + view * VIEW_FRAMES + frame.\n")
        fh.write(f"pub const VIEW_BASE: usize = {len(SIDE)};\npub const VIEW_FRAMES: usize = {len(VIEW_FR)};\n")
        fh.write("/// Sheet row for (breed, baby) is breed * 2 + baby.\n")
        fh.write(f"pub const ANCHORS: [[Anchor; {ncols}]; {len(rows)}] = [\n")
        for row in metas:
            fh.write("    [" + ", ".join(
                f"Anchor {{ head_top: {m['head_top']}, eye: {m['eye']}, head: {m['head']}, beak: {m['beak']} }}" for m in row) + "],\n")
        fh.write("];\n")
        # where each row's bill meets the bread: [on land, on water]
        tips = []
        for b, baby in rows:
            cells = frames_by_set["baby" if baby else "adult"]
            tips.append((bill_tip(cells[SIDE.index("peck2")]), bill_tip(cells[SIDE.index("dabble")])))
        fh.write("/// Bill tip, in cell pixels, while eating: [on land (Peck2), on water (Dabble)].\n")
        fh.write(f"pub const EAT_TIP: [[(i32, i32); 2]; {len(rows)}] = [\n")
        for land, water in tips:
            fh.write(f"    [{land}, {water}],\n")
        fh.write("];\n")

    if len(sys.argv) > 1:
        bg = Image.new("RGBA", sheet.size, (110, 150, 200, 255))
        bg.alpha_composite(sheet)
        bg.resize((sheet.width * 6, sheet.height * 6), Image.NEAREST).save(sys.argv[1])


if __name__ == "__main__":
    main()
# ---------------------------------------------------------------------------
# Hats: one per file kind. Grids use single-char keys; '.' is transparent.
# anchor: "top" = bottom-centre of the grid sits on the head top (overlap rows),
#         "eye" = grid centre on the eye, "head" = grid centre on the head.
HAT_PAL = {
    "o": "#2A2238", "R": "#D8404A", "r": "#A82C3A", "N": "#2E4A8A", "S": "#E8C878", "s": "#C49E50",
    "B": "#3E7BD8", "Y": "#F6D23C", "P": "#F28DB8", "p": "#D0608E", "G": "#5DBB63", "K": "#2B2B33",
    "W": "#FFFFFF", "L": "#9FD8F0", "M": "#C0C0CC", "V": "#8B5CC8",
}
HATS = {
    # folders: straw sun hat with a ribbon
    "straw": ("top", 1, [
        "...oooo...",
        "..oSSSSo..",
        "..oRRRRo..",
        "ooSSSSSSoo",
        "osssssssso",
        ".oooooooo.",
    ]),
    # images / screenshots: painter's beret (Monet would approve)
    "beret": ("top", 2, [
        "....o.....",
        "...oRo....",
        ".ooRRRoo..",
        "oRRRRRRRo.",
        "orrrrrrrro",
        ".oooooooo.",
    ]),
    "beret_navy": ("top", 2, [
        "....o.....",
        "...oNo....",
        ".ooNNNoo..",
        "oNNNNNNNo.",
        "oKNNNNNNKo",
        ".oooooooo.",
    ]),
    # pdf: mortarboard
    "mortar": ("top", 1, [
        "..oooooo..",
        "ooKKKKKKoo",
        "oKKKKKKKKo",
        ".ooKKKKoYo",
        "..oKKKKoY.",
        "..oooooo..",
    ]),
    # code: propeller beanie
    "propeller": ("top", 1, [
        ".RRR.BBB.",
        "....o....",
        "..ooYoo..",
        ".oRRYBBo.",
        "oRRRYBBBo",
        "ooooooooo",
    ]),
    # archives: party hat
    "party": ("top", 1, [
        "...Y...",
        "...o...",
        "..oPo..",
        "..PLPo.",
        ".oPPLo.",
        ".oLPPPo",
        "ooooooo",
    ]),
    # video / screen recordings: red cap, bill forward
    "cap": ("top", 1, [
        "..oooo....",
        ".oRRRRo...",
        "oRRWRRRo..",
        "oRRRRRRRRo",
        "ooooooorro",
    ]),
    # apps: tiny crown
    "crown": ("top", 1, [
        "o.o.o",
        "YoYoY",
        "YYRYY",
        "ooooo",
    ]),
    # anything else: a flower tucked on the head
    "flower": ("top", 2, [
        ".P.",
        "PYP",
        ".pG",
    ]),
    # shortcuts / aliases: a bow
    "bow": ("top", 2, [
        "oo.oo",
        "PPoPP",
        "PpPpP",
        "o...o",
    ]),
    # documents / notes: round reading glasses over the eye
    "glasses": ("eye", 0, [
        ".ooo.",
        "oL.Lo",
        "oL.Lo",
        ".ooo.",
    ]),
    # audio: headphones
    "headphones": ("head", 0, [
        "..ooooo..",
        ".o.....o.",
        "o.......o",
        "o.......o",
        "VV.....VV",
        "VV.....VV",
    ]),
}


def make_hats():
    names = list(HATS)
    cw, ch = 12, 8
    sheet = Image.new("RGBA", (cw * len(names), ch), (0, 0, 0, 0))
    meta = []
    for i, n in enumerate(names):
        anchor, overlap, rows = HATS[n]
        h, w = len(rows), max(len(r) for r in rows)
        for y, row in enumerate(rows):
            for x, k in enumerate(row):
                if k != ".":
                    sheet.putpixel((i * cw + x, y), hexc(HAT_PAL[k]))
        meta.append((n, anchor, overlap, w, h))
    sheet.save("assets/hats.png", optimize=True)
    with open("src/sprite_meta.rs", "a") as fh:
        fh.write("\n#[derive(Clone, Copy, PartialEq, Eq)]\npub enum HatAnchor { Top, Eye, Head }\n")
        fh.write("#[derive(Clone, Copy)]\npub struct HatInfo { pub name: &'static str, pub anchor: HatAnchor, pub overlap: i32, pub w: u32, pub h: u32 }\n")
        fh.write(f"pub const HAT_CELL_W: u32 = {cw};\n")
        fh.write(f"pub const HATS: [HatInfo; {len(names)}] = [\n")
        for n, a, o, w, h in meta:
            fh.write(f'    HatInfo {{ name: "{n}", anchor: HatAnchor::{a.capitalize()}, overlap: {o}, w: {w}, h: {h} }},\n')
        fh.write("];\n")



if __name__ == "__main__":
    make_hats()
