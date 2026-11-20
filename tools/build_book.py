#!/usr/bin/env python3
"""Writes the Dictionary of Spirits for the port's book screen (mod/src/book.rs, book_ui.rs) from
Occultism's Modonomicon book: mod/src/book_data.rs (the book as one JSON string) and the book's own
pictures as GUI sprites (mod/assets/occultism/textures/gui/sprites/book/).

Occultism's book (MIT; KliKli's) is its categories, each a map of entries, each a run of pages.
Here every page's text is its English text, its markup (Modonomicon's: colours, bold, links between
entries, item names) turned into runs; a recipe page names the port's own recipe file, its
ingredients left as the file has them for the game to resolve; a page whose recipe this port doesn't
have yet says so. Entries and pages keep Occultism's order, positions and links.

    python3 tools/build_book.py <occultism checkout>     # tools/build_content.py runs it last
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "mod"
MOD = "occultism"
# The port's names for Occultism's storage parts (tools/build_content.py's ALIASES).
ALIASES = {f"{MOD}:storage_stabilizer_tier{t}": f"{MOD}:storage_plate_tier{t}" for t in range(6)}
ALIASES[f"{MOD}:storage_controller"] = f"{MOD}:storage_actuator"
ALIASES[f"{MOD}:otherworld_sapling_natural"] = f"{MOD}:otherworld_sapling"
# Occultism's possessed creatures, which this port summons as the vanilla creature (spirits.rs).
POSSESSED_AS = {f"{MOD}:possessed_{m}": f"minecraft:{m}" for m in (
    "bee", "zombified_piglin", "blaze", "elder_guardian", "enderman", "endermite", "ghast", "guardian", "hoglin",
    "phantom", "shulker", "skeleton", "warden", "witch")}
POSSESSED_AS[f"{MOD}:possessed_weak_shulker"] = "minecraft:shulker"
POSSESSED_AS[f"{MOD}:mercy_goat"] = "minecraft:goat"

COLOUR = re.compile(r"\[#\]\(([0-9a-fA-F]{6})\)")
TOKENS = re.compile(r"(\[#\]\([0-9a-fA-F]{0,6}\)|\*\*|\*|\[[^\]]*\]\([a-z]+://[^)]*\)|\$\(br\)|\n)")


def runs(text):
    """Modonomicon markup as runs: [{"s": text, "c": colour, "b": bold, "i": italic, "l": entry,
    "it": item}], newlines kept in the text."""
    out, colour, bold, italic = [], None, False, False

    def add(s, **extra):
        if not s:
            return
        run = {"s": s}
        if colour is not None:
            run["c"] = colour
        if bold:
            run["b"] = 1
        if italic:
            run["i"] = 1
        run.update(extra)
        out.append(run)

    # A backslash ending a line is Modonomicon's hard line break.
    for part in TOKENS.split(text.replace("\\n", "\n").replace("\\\n", "\n").rstrip("\\")):
        if not part:
            continue
        if part.startswith("[#]("):
            m = COLOUR.fullmatch(part)
            colour = int(m.group(1), 16) if m else None
        elif part == "**":
            bold = not bold
        elif part == "*":
            italic = not italic
        elif part in ("\n", "$(br)"):
            add("\n")
        elif part.startswith("[") and "](" in part:
            label, target = part[1:-1].split("](", 1)
            kind, _, ident = target.partition("://")
            if kind == "item":
                add(label, it=ALIASES.get(ident, ident)) if label else out.append({"it": ALIASES.get(ident, ident)})
            elif kind in ("entry", "category"):
                add(label or ident, l=ident.split("#")[0])
            else:
                add(label)
        else:
            add(part)
    return out


def main(occ):
    occ = Path(occ)
    gen = occ / "src" / "generated" / "resources"
    book_dir = gen / "data" / MOD / "modonomicon" / "books" / "dictionary_of_spirits"
    lang = json.loads((gen / "assets" / MOD / "lang" / "en_us.json").read_text())
    occ_tex = occ / "src" / "main" / "resources" / "assets" / MOD / "textures"
    data = OUT / "data" / MOD
    sprites = OUT / "assets" / MOD / "textures" / "gui" / "sprites" / "book"
    ours = {f"{MOD}:{p.stem}" for kind in ("item", "block") for p in (data / "horizon" / kind).glob("*.json")}
    entities = {f"{MOD}:{p.stem}" for p in (data / "horizon" / "entity_type").glob("*.json")}
    pentacles = json.loads((data / "pentacles.json").read_text())

    def tr(key):
        return lang.get(key, "") if key else ""

    def known(ident):
        ident = ALIASES.get(ident, ident)
        return ident.startswith("minecraft:") or ident in ours

    def sprite(texture):
        """A picture of Occultism's book, `occultism:textures/gui/book/x.png`, as the sprite `occultism:book/x`."""
        name = texture.split("/")[-1].removesuffix(".png")
        src = occ_tex / "gui" / "book" / f"{name}.png"
        if not src.exists():
            return None
        sprites.mkdir(parents=True, exist_ok=True)
        # At most 256 pixels a side: the GUI atlas leaves larger pictures out.
        from PIL import Image
        with Image.open(src) as img:
            img = img.convert("RGBA")
            img.thumbnail((256, 256), Image.LANCZOS)
            img.save(sprites / f"{name}.png")
            w, h = img.size
        return {"id": f"{MOD}:gui/sprites/book/{name}", "w": w, "h": h}

    def icon(i):
        if isinstance(i, str):
            i = {"id": i}
        if "texture" in i:
            return {"sprite": sprite(i["texture"])}
        return {"item": ALIASES.get(i["id"], i["id"])} if known(i["id"]) else {"item": "minecraft:book"}

    def top_down(pentacle):
        """A pentacle seen from above, for the page's picture: each cell's first accepted block, a
        glyph shown as its chalk, the layers from the ground up (Modonomicon writes them from the top)."""
        cells = {}
        size = 0
        for layer in reversed(pentacle["pattern"]):
            for z, row in enumerate(layer):
                size = max(size, len(row), len(layer))
                for x, key in enumerate(row):
                    accepted = pentacle["keys"].get(key)
                    if key == " " or not accepted or accepted == "any":
                        continue
                    block = accepted[0]
                    item = block.replace("chalk_glyph_", "chalk_") if "chalk_glyph_" in block else block
                    cells[(x, z)] = item
        # And in full, for the preview in the world (pentacle_preview.rs): each place's offset from
        # the bowl as ritual.rs's `Pentacle::new` takes it, and the blocks it accepts.
        layers = pentacle["pattern"]
        centre = next((l, z, x) for l, layer in enumerate(layers) for z, row in enumerate(layer) for x, k in enumerate(row) if k == "0")
        parts = []
        for l, layer in enumerate(layers):
            for z, row in enumerate(layer):
                for x, key in enumerate(row):
                    accepted = pentacle["keys"].get(key)
                    if key == " " or not accepted or accepted == "any":
                        continue
                    parts.append([x - centre[2], centre[0] - l, z - centre[1], accepted])
        return {"size": size, "cells": [[x, z, item] for (x, z), item in sorted(cells.items())], "parts": parts}

    def recipe(rid):
        """The port's file for Occultism's recipe `rid`, by where tools/build_content.py writes it."""
        if not rid:
            return None
        ns, path = rid.split(":", 1)
        root = (OUT / "data" / ns / "recipe")
        for candidate in (path, path.replace("/", "_"), "paste_" + path.replace("/", "_"), "paste_" + path.split("/")[-1]):
            f = root / f"{candidate}.json"
            if f.exists():
                return json.loads(f.read_text())
        return None

    def grid(r):
        """A crafting recipe as up to three by three cells of ingredient JSON, and its result."""
        kind = r["type"]
        if kind == "minecraft:crafting_shaped":
            pattern = r["pattern"]
            cells = [[r["key"].get(ch) if ch != " " else None for ch in row] for row in pattern]
        elif kind in ("minecraft:crafting_shapeless", f"{MOD}:paste_shapeless"):
            ings = r["ingredients"]
            cells = [ings[i:i + 3] for i in range(0, len(ings), 3)]
        else:
            return None
        res = r["result"]
        return {"grid": cells, "result": ALIASES.get(res["id"], res["id"]), "count": res.get("count", 1)}

    def page(p):
        kind = p["type"].split(":")[1]
        text = runs(tr(p.get("text")))
        title = tr(p.get("title"))
        base = {"title": title, "text": text}
        missing = {"t": "missing", **base}
        if kind in ("text",):
            return {"t": "text", **base}
        if kind == "spotlight":
            item = p["item"] if isinstance(p["item"], str) else p["item"].get("id", "")
            return {"t": "spotlight", "item": ALIASES.get(item, item), **base} if known(item) else missing
        if kind == "image":
            return {"t": "image", "images": [s for s in (sprite(i) for i in p.get("images", [])) if s], **base}
        if kind == "entity":
            e = POSSESSED_AS.get(p["entity_id"], p["entity_id"])
            ok = e.startswith("minecraft:") or e in entities
            return {"t": "entity", "entity": e, "scale": p.get("scale", 1.0), "offset": p.get("offset", 0.0), **base} if ok else missing
        if kind == "category_link":
            return {"t": "link", "category": p.get("category_id", ""), **base}
        if kind == "multiblock":
            pid = p["multiblock_id"]
            if pid not in pentacles:
                return missing
            return {"t": "multiblock", "pentacle": pid, "name": tr(f"multiblock.{MOD}.{pid.split(':')[1]}") or tr(p.get("multiblock_name")),
                    **top_down(pentacles[pid]), **base}
        recipes = [recipe(p.get(f"recipe_id_{n}")) for n in (1, 2)]
        recipes = [r for r in recipes if r]
        if not recipes:
            return missing
        if kind in ("crafting_recipe", "book_binding_recipe"):
            grids = [g for g in (grid(r) for r in recipes) if g]
            return {"t": "crafting", "recipes": grids, **base} if grids else missing
        if kind in ("spirit_fire_recipe", "smelting_recipe"):
            r = recipes[0]
            return {"t": "fire" if kind == "spirit_fire_recipe" else "smelting", "input": r["ingredient"],
                    "result": ALIASES.get(r["result"]["id"], r["result"]["id"]), "count": r["result"].get("count", 1), **base}
        if kind == "ritual_recipe":
            r = recipes[0]
            pentacle = r["pentacle_id"]
            out = {"t": "ritual", "pentacle": pentacle, "pentacle_name": tr(f"multiblock.{MOD}.{pentacle.split(':')[1]}"),
                   "activation": r["activation_item"], "ingredients": r["ingredients"], "duration": r["duration"], **base}
            if "result" in r:
                out["result"] = r["result"]["id"]
            if "summon" in r:
                out["summon"] = POSSESSED_AS.get(r["summon"], r["summon"])
            if "sacrifice" in r:
                out["sacrifice"] = tr(r["sacrifice"]["display_name"])
            if "item_to_use" in r:
                out["item_use"] = r["item_to_use"]
            return out
        return missing

    categories = []
    for cf in sorted((book_dir / "categories").glob("*.json"), key=lambda f: json.loads(f.read_text())["sort_number"]):
        c = json.loads(cf.read_text())
        entries = []
        for ef in sorted((book_dir / "entries" / cf.stem).glob("*.json")):
            e = json.loads(ef.read_text())
            pages = [json.loads(pf.read_text()) for pf in (book_dir / "entries" / cf.stem / ef.stem / "pages").glob("*.json")]
            pages = [page(p) for p in sorted(pages, key=lambda p: p.get("sort_number", 0))]
            ident = e["id"].split(":", 1)[1]
            entries.append({
                "id": ident, "name": tr(e["name"]), "desc": tr(e.get("description")), "icon": icon(e["icon"]),
                "x": e.get("x", 0), "y": e.get("y", 0), "parents": [p["entry"].split(":", 1)[1] for p in e.get("parents", [])],
                "pages": pages,
                # An entry about an item this port hasn't got yet.
                "missing": isinstance(e["icon"], dict) and "id" in e["icon"] and not known(e["icon"]["id"]),
            })
        categories.append({"id": cf.stem, "name": tr(c["name"]), "icon": icon(c["icon"]), "entries": entries})
    book = {"name": tr("book.occultism.dictionary_of_spirits.name"), "categories": categories}
    text = json.dumps(book, separators=(",", ":"), ensure_ascii=False)
    if '"#' in text:
        text = text.replace('"#', '"\\u0023')
    (OUT / "src" / "book_data.rs").write_text(
        "// Generated by tools/build_book.py from Occultism's Dictionary of Spirits (MIT): don't edit.\n\n"
        f'pub const BOOK: &str = r#"{text}"#;\n')
    pages = [p for c in categories for e in c["entries"] for p in e["pages"]]
    print(f"  book: {len(categories)} categories, {sum(len(c['entries']) for c in categories)} entries, "
          f"{len(pages)} pages ({sum(p['t'] == 'missing' for p in pages)} not in this port yet)")


if __name__ == "__main__":
    main(sys.argv[1])
