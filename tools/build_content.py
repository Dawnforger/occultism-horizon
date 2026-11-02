#!/usr/bin/env python3
"""Builds the Occultism content mod (content/) from an Occultism checkout.

    python3 tools/build_content.py /path/to/occultism [--out content]

What it does:
- Copies Occultism's textures, item models and lang strings for what this proof of concept uses
  (MIT; Kli Kli's permission covers the port).
- Copies Occultism's `occultism:crushing` recipes whose input and output exist in vanilla plus the
  dusts shipped here. Their `neoforge:conditions` (`tag_empty`, which Horizon doesn't know and
  would drop the file for) are removed: the dropped recipes are exactly the ones the conditions
  would have disabled, so the set is the same as Occultism's in a pack with no other mods.
- Copies Occultism's `c:` convention tags and adds the vanilla entries NeoForge's own tags supply.
- Writes the Demonic Altar block, the four crusher cores, the altar's menu type, its creative tab,
  loot table and crafting recipes.

Prints a summary and exits non-zero if anything it relies on is missing.
"""
import argparse
import json
import shutil
import struct
import sys
import zlib
from pathlib import Path

MOD = "occultism"

# Occultism dusts with textures, shipped as items.
DUSTS = [
    "iron", "gold", "copper", "netherite", "netherite_scrap", "obsidian",
    "emerald", "lapis", "amethyst", "echo", "silver", "iesnium",
]

# Chalks: colour name -> glyph colour (Occultism's client config defaults). The item's own tint
# comes from Occultism's item model definition.
CHALKS = {
    "white": 0xFFFFFF, "light_gray": 0x9D9D97, "gray": 0x474F52, "black": 0x1D1D21,
    "brown": 0x835432, "red": 0xCC0101, "orange": 0xF9801D, "yellow": 0xF0D700,
    "lime": 0x80C71F, "green": 0x5E7C16, "cyan": 0x169C9C, "light_blue": 0x80ABE7,
    "blue": 0x3C44AA, "purple": 0x9C0393, "magenta": 0xC74EBD, "pink": 0xF38BAA,
}
GLYPH_SIGNS = 14  # signs 1..14; 0 is Occultism's book preview

# Ritual circles and the rituals this port ships: Occultism's crusher summonings (binding into a
# spirit core instead of summoning a spirit) and the infused lenses. Pentacles come from
# Occultism's modonomicon multiblocks.
PENTACLES = ["summon_foliot", "summon_djinni", "summon_afrit", "summon_marid", "craft_foliot"]
RITUALS = {
    "summon_foliot_crusher": f"{MOD}:crusher_core_foliot",
    "summon_djinni_crusher": f"{MOD}:crusher_core_djinni",
    "summon_afrit_crusher": f"{MOD}:crusher_core_afrit",
    "summon_marid_crusher": f"{MOD}:crusher_core_marid",
    "craft_infused_lenses": None,  # Occultism's own result
}
CANDLES = ["minecraft:candle"] + [f"minecraft:{c}_candle" for c in CHALKS]
# Binding books (activation items), with the dye Occultism's own book recipe uses for the rank.
BOOK_DYES = {"foliot": "blue", "djinni": "purple", "afrit": "yellow", "marid": "green"}

# NeoForge's convention tags, for vanilla items (NeoForge `Tags.Items`, 26.x names).
VANILLA_C_TAGS = {
    "raw_materials/iron": ["minecraft:raw_iron"],
    "raw_materials/gold": ["minecraft:raw_gold"],
    "raw_materials/copper": ["minecraft:raw_copper"],
    "storage_blocks/raw_iron": ["minecraft:raw_iron_block"],
    "storage_blocks/raw_gold": ["minecraft:raw_gold_block"],
    "storage_blocks/raw_copper": ["minecraft:raw_copper_block"],
    "storage_blocks/diamond": ["minecraft:diamond_block"],
    "storage_blocks/emerald": ["minecraft:emerald_block"],
    "storage_blocks/netherite": ["minecraft:netherite_block"],
    "ingots/iron": ["minecraft:iron_ingot"],
    "ingots/gold": ["minecraft:gold_ingot"],
    "ingots/copper": ["minecraft:copper_ingot"],
    "ingots/netherite": ["minecraft:netherite_ingot"],
    "ores/iron": ["#minecraft:iron_ores"],
    "ores/gold": ["#minecraft:gold_ores"],
    "ores/copper": ["#minecraft:copper_ores"],
    "ores/coal": ["#minecraft:coal_ores"],
    "ores/diamond": ["#minecraft:diamond_ores"],
    "ores/emerald": ["#minecraft:emerald_ores"],
    "ores/lapis": ["#minecraft:lapis_ores"],
    "ores/redstone": ["#minecraft:redstone_ores"],
    "ores/quartz": ["minecraft:nether_quartz_ore"],
    "ores/netherite_scrap": ["minecraft:ancient_debris"],
    "gems/diamond": ["minecraft:diamond"],
    "gems/emerald": ["minecraft:emerald"],
    "gems/lapis": ["minecraft:lapis_lazuli"],
    "gems/quartz": ["minecraft:quartz"],
    "gems/amethyst": ["minecraft:amethyst_shard"],
    "obsidians": ["minecraft:obsidian", "minecraft:crying_obsidian"],
    "end_stones": ["minecraft:end_stone"],
    "ender_pearls": ["minecraft:ender_pearl"],
    "rods/blaze": ["minecraft:blaze_rod"],
    "bones": ["minecraft:bone"],
    "netherite_scraps": ["minecraft:netherite_scrap"],
    "dusts/redstone": ["minecraft:redstone"],
    "dusts/glowstone": ["minecraft:glowstone_dust"],
    "leathers": ["minecraft:leather"],
    "glass_panes": ["minecraft:glass_pane"],
    "slime_balls": ["minecraft:slime_ball"],
}

# Vanilla's ore tags (minecraft:*_ores), so the generator can tell which inputs resolve.
VANILLA_ORE_TAGS = {
    "minecraft:iron_ores": ["minecraft:iron_ore", "minecraft:deepslate_iron_ore"],
    "minecraft:gold_ores": ["minecraft:gold_ore", "minecraft:deepslate_gold_ore", "minecraft:nether_gold_ore"],
    "minecraft:copper_ores": ["minecraft:copper_ore", "minecraft:deepslate_copper_ore"],
    "minecraft:coal_ores": ["minecraft:coal_ore", "minecraft:deepslate_coal_ore"],
    "minecraft:diamond_ores": ["minecraft:diamond_ore", "minecraft:deepslate_diamond_ore"],
    "minecraft:emerald_ores": ["minecraft:emerald_ore", "minecraft:deepslate_emerald_ore"],
    "minecraft:lapis_ores": ["minecraft:lapis_ore", "minecraft:deepslate_lapis_ore"],
    "minecraft:redstone_ores": ["minecraft:redstone_ore", "minecraft:deepslate_redstone_ore"],
}

# The four cores: (tier name, level, rarity, tint).
TIERS = [
    ("foliot", 1, "common", 0xB8E986),
    ("djinni", 2, "uncommon", 0x5FD3C6),
    ("afrit", 3, "rare", 0xF0623A),
    ("marid", 4, "epic", 0x6C7BFF),
]


def die(msg):
    print(f"error: {msg}", file=sys.stderr)
    sys.exit(1)


def write_json(path: Path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + "\n")


def copy(src: Path, dst: Path):
    if not src.is_file():
        die(f"missing in the Occultism checkout: {src}")
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(src, dst)


def copy_texture(src: Path, dst: Path):
    """A texture and its animation file, when it has one."""
    copy(src, dst)
    if src.with_name(src.name + ".mcmeta").exists():
        copy(src.with_name(src.name + ".mcmeta"), dst.with_name(dst.name + ".mcmeta"))


def read_png(path: Path):
    """An 8-bit RGBA, non-interlaced PNG as (width, height, rows of RGBA bytes)."""
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        die(f"not a PNG: {path}")
    pos, idat, head = 8, b"", None
    while pos + 8 <= len(data):
        length, kind = struct.unpack(">I4s", data[pos:pos + 8])
        if kind == b"IEND":
            break
        body = data[pos + 8:pos + 8 + length]
        if kind == b"IHDR":
            head = struct.unpack(">IIBBBBB", body)
        elif kind == b"IDAT":
            idat += body
        pos += 12 + length
    width, height, depth, colour, _, _, interlace = head
    if (depth, colour, interlace) != (8, 6, 0):
        die(f"{path}: only 8-bit RGBA, non-interlaced PNGs are read here")
    raw, stride, rows, prev = zlib.decompress(idat), width * 4, [], bytearray(width * 4)
    for y in range(height):
        kind, line = raw[y * (stride + 1)], bytearray(raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)])
        for i in range(stride):
            a = line[i - 4] if i >= 4 else 0
            b = prev[i]
            c = prev[i - 4] if i >= 4 else 0
            if kind == 1:
                line[i] = (line[i] + a) & 0xFF
            elif kind == 2:
                line[i] = (line[i] + b) & 0xFF
            elif kind == 3:
                line[i] = (line[i] + (a + b) // 2) & 0xFF
            elif kind == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 0xFF
        rows.append(line)
        prev = line
    return width, height, rows


def write_png(path: Path, width: int, height: int, rows):
    def chunk(kind, body):
        return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))
    raw = b"".join(b"\x00" + bytes(r) for r in rows)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
                     + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))


def tint_png(src: Path, dst: Path, rgb: int):
    """`src` multiplied by `rgb`, as a model's tintindex would draw it (Horizon has no block tints)."""
    width, height, rows = read_png(src)
    tint = ((rgb >> 16) & 0xFF, (rgb >> 8) & 0xFF, rgb & 0xFF)
    out = []
    for line in rows:
        line = bytearray(line)
        for i in range(0, len(line), 4):
            for k in range(3):
                line[i + k] = line[i + k] * tint[k] // 255
        out.append(line)
    write_png(dst, width, height, out)


def loot_self(block: str) -> dict:
    """26.3's loot table for a block that drops itself (vanilla's iron_block)."""
    return {"type": "minecraft:block",
            "pools": [{"rolls": 1, "condition": {"type": "minecraft:survives_explosion"},
                       "entries": [{"type": "minecraft:item", "name": block}]}],
            "random_sequence": block.replace(":", ":blocks/", 1)}


def loot_ore(block: str, raw: str) -> dict:
    """26.3's loot table for an ore (vanilla's iron_ore): itself with silk touch, else raw ore."""
    return {"type": "minecraft:block",
            "pools": [{"rolls": 1, "entries": [{"type": "minecraft:alternatives", "children": [
                {"type": "minecraft:item", "condition": "minecraft:tool/can_silk_touch", "name": block},
                {"type": "minecraft:item", "name": raw, "modifier": [
                    {"type": "minecraft:apply_bonus", "enchantment": "minecraft:fortune", "formula": "minecraft:ore_drops"},
                    {"type": "minecraft:explosion_decay"}]}]}]}],
            "random_sequence": block.replace(":", ":blocks/", 1)}


def ore_feature(stone: str, deepslate: str, size: int) -> dict:
    """26.3's ore feature (vanilla's ore_copper without its height-specific rule)."""
    return {"type": "minecraft:ore", "discard_chance_on_air_exposure": 0.0, "size": size, "targets": [
        {"state": stone, "target": {"predicate_type": "minecraft:tag_match", "tag": "minecraft:stone_ore_replaceables"}},
        {"state": deepslate, "target": {"predicate_type": "minecraft:tag_match", "tag": "minecraft:deepslate_ore_replaceables"}},
    ]}


class Tags:
    """Item tags as the game will see them: Occultism's c: tags, the vanilla entries above."""

    def __init__(self, occ_data: Path, items: set):
        self.files = {}  # "c:dusts/iron" -> list of entries
        for f in sorted((occ_data / "c" / "tags" / "item").rglob("*.json")):
            name = "c:" + f.relative_to(occ_data / "c" / "tags" / "item").with_suffix("").as_posix()
            self.files[name] = json.loads(f.read_text()).get("values", [])
        for name, values in VANILLA_C_TAGS.items():
            self.files.setdefault("c:" + name, [])
            self.files["c:" + name] = values + self.files["c:" + name]
        for name, values in VANILLA_ORE_TAGS.items():
            self.files[name] = values
        self.items = items

    def resolve(self, tag: str, seen=None) -> list:
        seen = seen or set()
        if tag in seen:
            return []
        seen.add(tag)
        out = []
        for entry in self.files.get(tag, []):
            ident = entry["id"] if isinstance(entry, dict) else entry
            if ident.startswith("#"):
                out += self.resolve(ident[1:], seen)
            elif self.known(ident) and ident not in out:
                out.append(ident)
        return out

    def known(self, ident: str) -> bool:
        return ident.startswith("minecraft:") or ident in self.items


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("occultism", type=Path)
    ap.add_argument("--out", type=Path, default=Path(__file__).resolve().parent.parent / "content")
    args = ap.parse_args()

    occ = args.occultism
    gen = occ / "src/generated/resources"
    main_res = occ / "src/main/resources"
    occ_data = gen / "data"
    occ_assets = main_res / "assets" / MOD
    gen_assets = gen / "assets" / MOD
    if not (occ_data / MOD / "recipe" / "crushing").is_dir():
        die(f"{occ} doesn't look like an Occultism checkout (no generated crushing recipes)")

    out = args.out
    if out.exists():
        shutil.rmtree(out)
    assets = out / "assets" / MOD
    data = out / "data" / MOD

    (out).mkdir(parents=True)
    (out / "horizon-mod.toml").write_text(
        f'''id = "{MOD}"
name = "Occultism: Demonic Altar (proof of concept)"
version = "0.2.0"
description = "Occultism's crusher spirits bound into spirit cores that power a Demonic Altar. Port by permission of Kli Kli; needs the occultism native mod."
authors = ["klikli-dev (Occultism)", "Horizon port"]
api = "0.2"
side = "both"
capabilities = ["registries"]
'''
    )
    copy(occ / "LICENSE", out / "LICENSE-occultism.txt")
    copy(occ / "THIRD_PARTY_NOTICES.md", out / "THIRD_PARTY_NOTICES-occultism.md")

    lang_src = json.loads((gen_assets / "lang" / "en_us.json").read_text())
    lang = {}

    # --- Dusts: items, models, textures, names.
    items = set()
    for d in DUSTS:
        ident = f"{MOD}:{d}_dust"
        items.add(ident)
        write_json(data / "horizon" / "item" / f"{d}_dust.json", {"template": "minecraft:glowstone_dust"})
        copy(occ_assets / "textures" / "item" / f"{d}_dust.png", assets / "textures" / "item" / f"{d}_dust.png")
        write_json(assets / "models" / "item" / f"{d}_dust.json",
                   {"parent": "minecraft:item/generated", "textures": {"layer0": f"{MOD}:item/{d}_dust"}})
        write_json(assets / "items" / f"{d}_dust.json",
                   {"model": {"type": "minecraft:model", "model": f"{MOD}:item/{d}_dust"}})
        key = f"item.{MOD}.{d}_dust"
        if key not in lang_src:
            die(f"no lang string {key}")
        lang[key] = lang_src[key]

    # --- Spirit cores: one item per rank, each carrying its spirit as data.
    copy(occ_assets / "textures" / "item" / "soul_gem.png", assets / "textures" / "item" / "soul_gem.png")
    copy(occ_assets / "textures" / "item" / "soul_gem.png.mcmeta", assets / "textures" / "item" / "soul_gem.png.mcmeta")
    write_json(assets / "models" / "item" / "spirit_core.json",
               {"parent": "minecraft:item/generated", "textures": {"layer0": f"{MOD}:item/soul_gem"}})
    for name, level, rarity, tint in TIERS:
        core = f"crusher_core_{name}"
        items.add(f"{MOD}:{core}")
        write_json(data / "horizon" / "item" / f"{core}.json", {
            "template": "minecraft:echo_shard",
            "max_stack_size": 1,
            "rarity": rarity,
            "components": {f"{MOD}:spirit": {"job": "crusher", "tier": level}},
        })
        write_json(assets / "items" / f"{core}.json", {
            "model": {"type": "minecraft:model", "model": f"{MOD}:item/spirit_core",
                      "tints": [{"type": "minecraft:constant", "value": tint}]}
        })
        title = name.capitalize()
        lang[f"item.{MOD}.{core}"] = f"Bound {title} Crusher"
        lang[f"tooltip.{MOD}.spirit.{name}"] = lang_src.get(f"item.{MOD}.ritual_dummy.summon_{name}_crusher.auto_tooltip", f"Tier: {title}")
    lang[f"tooltip.{MOD}.spirit.crusher"] = "Job: Crusher"
    lang[f"tooltip.{MOD}.spirit.place"] = "Place in a Demonic Altar to put this spirit to work."

    # --- The Demonic Altar.
    for tex in ["chiseled_otherstone_bricks", "dimensional_mineshaft_top", "polished_otherstone"]:
        copy(occ_assets / "textures" / "block" / f"{tex}.png", assets / "textures" / "block" / f"{tex}.png")
        mcmeta = occ_assets / "textures" / "block" / f"{tex}.png.mcmeta"
        if mcmeta.exists():
            copy(mcmeta, assets / "textures" / "block" / f"{tex}.png.mcmeta")
    write_json(data / "horizon" / "block" / "demonic_altar.json", {
        "template": "minecraft:barrel",
        "properties": {"hardness": 3.5, "explosion_resistance": 1200.0, "light": 4,
                       "requires_tool": True, "sounds": "minecraft:deepslate_bricks"},
        "item": True,
        "components": {"minecraft:rarity": "uncommon"},
    })
    write_json(assets / "models" / "block" / "demonic_altar.json", {
        "parent": "minecraft:block/cube_bottom_top",
        "textures": {"top": f"{MOD}:block/dimensional_mineshaft_top",
                     "side": f"{MOD}:block/chiseled_otherstone_bricks",
                     "bottom": f"{MOD}:block/polished_otherstone"},
    })
    # The barrel template's states (facing x open) all show the one altar model.
    variants = {}
    for facing in ["down", "up", "north", "south", "west", "east"]:
        for open_ in ["false", "true"]:
            variants[f"facing={facing},open={open_}"] = {"model": f"{MOD}:block/demonic_altar"}
    write_json(assets / "blockstates" / "demonic_altar.json", {"variants": variants})
    write_json(assets / "items" / "demonic_altar.json",
               {"model": {"type": "minecraft:model", "model": f"{MOD}:block/demonic_altar"}})
    lang[f"block.{MOD}.demonic_altar"] = "Demonic Altar"
    lang[f"container.{MOD}.demonic_altar"] = "Demonic Altar"
    lang[f"gui.{MOD}.demonic_altar.no_core"] = "Bind a spirit core"
    lang[f"gui.{MOD}.demonic_altar.idle"] = "Waiting for ore"
    lang[f"gui.{MOD}.demonic_altar.working"] = "Crushing"
    lang[f"gui.{MOD}.demonic_altar.full"] = "Output full"
    write_json(data / "horizon" / "menu" / "demonic_altar.json", {"slots": 6, "data_slots": 4, "results": [2, 3, 4, 5]})
    # 26.3's loot format, as vanilla's own barrel table has it: a condition names its kind in
    # `type` (1.21's `{"condition": ...}` reads as an unknown kind, and the pool never rolls).
    write_json(data / "loot_table" / "blocks" / "demonic_altar.json", {
        "type": "minecraft:block",
        "pools": [{"rolls": 1, "entries": [{"type": "minecraft:item", "name": f"{MOD}:demonic_altar"}],
                   "condition": {"type": "minecraft:survives_explosion"}}],
        "random_sequence": f"{MOD}:blocks/demonic_altar",
    })
    pickaxe = [f"{MOD}:demonic_altar"]  # data/minecraft/tags/block/mineable/pickaxe.json, written last

    # --- Tags: Occultism's c: tags as shipped, plus the vanilla entries (merged by the game).
    for f in sorted((occ_data / "c" / "tags" / "item").rglob("*.json")):
        rel = f.relative_to(occ_data / "c" / "tags" / "item")
        copy(f, out / "data" / "c" / "tags" / "item" / rel)
    for name, values in VANILLA_C_TAGS.items():
        path = out / "data" / "c" / "tags" / "item" / f"{name}.json"
        existing = json.loads(path.read_text())["values"] if path.exists() else []
        write_json(path, {"values": values + existing})
    tags = Tags(occ_data, items)

    # --- Crafting: the altar.
    write_json(data / "recipe" / "demonic_altar.json", {
        "type": "minecraft:crafting_shaped", "category": "misc",
        "pattern": ["GAG", "OCO", "OOO"],
        "key": {"G": "minecraft:gold_ingot", "A": "minecraft:amethyst_shard",
                "O": "minecraft:obsidian", "C": "minecraft:crying_obsidian"},
        "result": {"id": f"{MOD}:demonic_altar", "count": 1},
    })
    # The cores are bound in rituals now (below), as Occultism summons its crusher spirits.
    tab = [f"{MOD}:demonic_altar"] + [f"{MOD}:crusher_core_{t[0]}" for t in TIERS]

    def lang_of(key, fallback=None):
        if key in lang_src:
            lang[key] = lang_src[key]
        elif fallback is not None:
            lang[key] = fallback
        else:
            die(f"no lang string {key}")

    def flat_item(name, template, texture=None, extra=None, block=None):
        """An item with a flat icon from Occultism's textures (or a block's own model)."""
        ident = f"{MOD}:{name}"
        items.add(ident)
        decl = {"template": template}
        decl.update(extra or {})
        write_json(data / "horizon" / "item" / f"{name}.json", decl)
        if block is None:
            copy_texture(occ_assets / "textures" / "item" / f"{texture or name}.png",
                         assets / "textures" / "item" / f"{texture or name}.png")
            write_json(assets / "models" / "item" / f"{name}.json",
                       {"parent": "minecraft:item/generated", "textures": {"layer0": f"{MOD}:item/{texture or name}"}})
        write_json(assets / "items" / f"{name}.json",
                   {"model": {"type": "minecraft:model", "model": block or f"{MOD}:item/{name}"}})
        lang_of(f"item.{MOD}.{name}")
        tab.append(ident)

    def cube_block(name, template, texture=None, loot=None, props=None, tool=True):
        """A full block with one texture on all sides, and its item."""
        ident = f"{MOD}:{name}"
        items.add(ident)
        decl = {"template": template, "item": True}
        if props:
            decl["properties"] = props
        write_json(data / "horizon" / "block" / f"{name}.json", decl)
        copy_texture(occ_assets / "textures" / "block" / f"{texture or name}.png",
                     assets / "textures" / "block" / f"{texture or name}.png")
        write_json(assets / "models" / "block" / f"{name}.json",
                   {"parent": "minecraft:block/cube_all", "textures": {"all": f"{MOD}:block/{texture or name}"}})
        write_json(assets / "blockstates" / f"{name}.json", {"variants": {"": {"model": f"{MOD}:block/{name}"}}})
        write_json(assets / "items" / f"{name}.json", {"model": {"type": "minecraft:model", "model": f"{MOD}:block/{name}"}})
        write_json(data / "loot_table" / "blocks" / f"{name}.json", loot or loot_self(ident))
        lang_of(f"block.{MOD}.{name}")
        if tool:
            pickaxe.append(ident)
        tab.append(ident)

    def model_block(name, template, model, props, shapes=None):
        """A block drawn with one of Occultism's own models (its textures copied), and its item."""
        ident = f"{MOD}:{name}"
        items.add(ident)
        decl = {"template": template, "item": True, "properties": props}
        write_json(data / "horizon" / "block" / f"{name}.json", decl)
        src = json.loads((main_res / "assets" / MOD / "models" / "block" / f"{model}.json").read_text())
        parent = src.get("parent", "")
        if parent.startswith(f"{MOD}:"):  # a model built on another of Occultism's (the bowls)
            base = parent.split("/", 1)[1]
            write_json(assets / "models" / "block" / f"{base}.json",
                       json.loads((main_res / "assets" / MOD / "models" / "block" / f"{base}.json").read_text()))
        for tex in src.get("textures", {}).values():
            if tex.startswith(f"{MOD}:block/"):
                rel = tex.split("/", 1)[1]
                copy_texture(occ_assets / "textures" / "block" / f"{rel}.png", assets / "textures" / "block" / f"{rel}.png")
        write_json(assets / "models" / "block" / f"{name}.json", src)
        write_json(assets / "blockstates" / f"{name}.json", {"variants": {"": {"model": f"{MOD}:block/{name}"}}})
        write_json(assets / "items" / f"{name}.json", {"model": {"type": "minecraft:model", "model": f"{MOD}:block/{name}"}})
        write_json(data / "loot_table" / "blocks" / f"{name}.json", loot_self(ident))
        lang_of(f"block.{MOD}.{name}")
        pickaxe.append(ident)
        tab.append(ident)

    # --- Chalks and their glyphs: a chalk places its glyph (the native mod's use hook), a glyph
    # is a flat carpet with a sign (1..14, Occultism's 15 typefaces less the preview) and a facing.
    glyphs = set()  # blocks without an item, for the pentacles' checks
    glyph_model = json.loads((main_res / "assets" / MOD / "models" / "block" / "chalk_glyph" / "chalk_glyph.json").read_text())
    for face in glyph_model["elements"][0]["faces"].values():
        face.pop("tintindex", None)  # baked into each colour's textures below
    write_json(assets / "models" / "block" / "chalk_glyph" / "chalk_glyph.json", glyph_model)
    white_states = json.loads((gen_assets / "blockstates" / "chalk_glyph_white.json").read_text())["variants"]
    copy_texture(occ_assets / "textures" / "item" / "chalk_base.png", assets / "textures" / "item" / "chalk_base.png")
    write_json(assets / "models" / "item" / "chalk.json",
               {"parent": "minecraft:item/generated", "textures": {"layer0": f"{MOD}:item/chalk_base"}})
    for colour, rgb in CHALKS.items():
        # Horizon's native API can't place a block by name, so a chalk is its glyph's own block
        # item: vanilla's placement draws the glyph (the native mod picks its sign and facing)
        # and uses up one chalk, where Occultism's chalk loses durability.
        glyph = f"chalk_glyph_{colour}"
        items.add(f"{MOD}:{glyph}")
        glyphs.add(f"{MOD}:{glyph}")
        # Leaf litter: flat, see-through (drawn cut-out), no collision, a horizontal facing, and it
        # needs a sturdy face below, as Occultism's glyph does. Its segment_amount stays 1.
        write_json(data / "horizon" / "block" / f"{glyph}.json", {
            "template": "minecraft:leaf_litter", "item": True,
            "properties": {"hardness": 0.0, "sounds": "minecraft:stone"},
            "state_properties": {"sign": {"type": "enum", "values": [str(i) for i in range(GLYPH_SIGNS + 1)]}},
            "default_state": {"sign": "1"},
            "components": {"!minecraft:compostable": {}, "!minecraft:cooking_fuel": {}}})
        # A glyph drops nothing, as Occultism's (without a table it would drop its template's leaf litter).
        write_json(data / "loot_table" / "blocks" / f"{glyph}.json",
                   {"type": "minecraft:block", "pools": [], "random_sequence": f"{MOD}:blocks/{glyph}"})
        item_def = json.loads((gen_assets / "items" / f"chalk_{colour}.json").read_text())
        item_def["model"]["model"] = f"{MOD}:item/chalk"
        write_json(assets / "items" / f"{glyph}.json", item_def)
        lang[f"block.{MOD}.{glyph}"] = lang_src[f"item.{MOD}.chalk_{colour}"]
        tab.append(f"{MOD}:{glyph}")
        for sign in range(1, GLYPH_SIGNS + 1):
            tint_png(occ_assets / "textures" / "block" / "chalk_glyph" / f"{sign}.png",
                     assets / "textures" / "block" / "chalk_glyph" / colour / f"{sign}.png", rgb)
            write_json(assets / "models" / "block" / "chalk_glyph" / colour / f"{sign}.json", {
                "parent": f"{MOD}:block/chalk_glyph/chalk_glyph",
                "textures": {"texture": f"{MOD}:block/chalk_glyph/{colour}/{sign}"}})
        variants = {}
        for key, variant in white_states.items():
            props = dict(kv.split("=") for kv in key.split(","))
            sign = props["sign"]
            variant = dict(variant)
            variant["model"] = f"{MOD}:block/chalk_glyph/{colour}/{sign if sign != '0' else '1'}"
            for segments in range(1, 5):
                variants[f"facing={props['facing']},segment_amount={segments},sign={sign}"] = variant
        write_json(assets / "blockstates" / f"{glyph}.json", {"variants": variants})
    write_json(data / "recipe" / "chalk_white.json", {
        "type": "minecraft:crafting_shapeless", "category": "misc",
        "ingredients": ["minecraft:calcite", "minecraft:bone_meal", "minecraft:clay_ball"],
        "result": {"id": f"{MOD}:chalk_glyph_white", "count": 16}})
    for colour in CHALKS:
        if colour != "white":
            write_json(data / "recipe" / f"chalk_{colour}.json", {
                "type": "minecraft:crafting_shaped", "category": "misc",
                "pattern": ["ccc", "cdc", "ccc"],
                "key": {"c": f"{MOD}:chalk_glyph_white", "d": f"minecraft:{colour}_dye"},
                "result": {"id": f"{MOD}:chalk_glyph_{colour}", "count": 8}})

    # --- Sacrificial bowls: glass templates (they don't hide their neighbours' faces), with
    # Occultism's models. The native mod gives each a one-slot block entity.
    bowl_props = {"hardness": 1.5, "explosion_resistance": 6.0, "sounds": "minecraft:stone", "requires_tool": False}
    model_block("sacrificial_bowl", "minecraft:glass", "sacrificial_bowl", bowl_props)
    model_block("golden_sacrificial_bowl", "minecraft:glass", "golden_sacrificial_bowl", bowl_props)
    write_json(data / "recipe" / "sacrificial_bowl.json", {
        "type": "minecraft:crafting_shaped", "category": "building",
        "pattern": ["o o", "ooo"], "key": {"o": "minecraft:polished_deepslate"},
        "result": {"id": f"{MOD}:sacrificial_bowl", "count": 1}})

    # --- Silver and iesnium. Silver ore generates as in Occultism; iesnium has no ore: the native
    # mod drops raw iesnium from mined silver ore for a player with the Third Eye.
    cube_block("silver_ore", "minecraft:iron_ore", loot=loot_ore(f"{MOD}:silver_ore", f"{MOD}:raw_silver"),
               props={"requires_tool": True})
    cube_block("silver_ore_deepslate", "minecraft:deepslate_iron_ore",
               loot=loot_ore(f"{MOD}:silver_ore_deepslate", f"{MOD}:raw_silver"), props={"requires_tool": True})
    for name, template in [("silver_block", "minecraft:iron_block"), ("raw_silver_block", "minecraft:raw_iron_block"),
                           ("iesnium_block", "minecraft:netherite_block"), ("raw_iesnium_block", "minecraft:raw_iron_block")]:
        cube_block(name, template, props={"requires_tool": True})
    for name, template in [("raw_silver", "minecraft:raw_iron"), ("silver_ingot", "minecraft:iron_ingot"),
                           ("silver_nugget", "minecraft:iron_nugget"), ("raw_iesnium", "minecraft:raw_iron"),
                           ("iesnium_ingot", "minecraft:netherite_ingot"), ("iesnium_nugget", "minecraft:iron_nugget")]:
        flat_item(name, template)
    worldgen = data / "worldgen"
    write_json(worldgen / "configured_feature" / "ore_silver.json", {
        "type": "minecraft:ore", "discard_chance_on_air_exposure": 0.0, "size": 7, "targets": [
            {"state": f"{MOD}:silver_ore",
             "target": {"predicate_type": "minecraft:tag_match", "tag": "minecraft:stone_ore_replaceables"}}]})
    write_json(worldgen / "configured_feature" / "ore_silver_deepslate.json", {
        "type": "minecraft:ore", "discard_chance_on_air_exposure": 0.0, "size": 7, "targets": [
            {"state": f"{MOD}:silver_ore_deepslate",
             "target": {"predicate_type": "minecraft:tag_match", "tag": "minecraft:deepslate_ore_replaceables"}}]})
    occ_size = {n: json.loads((occ_data / MOD / "worldgen" / "configured_feature" / f"{n}.json").read_text())["config"]["size"]
                for n in ("ore_silver", "ore_silver_deepslate")}
    for n, size in occ_size.items():  # Occultism's vein sizes, in 26.3's flattened format
        path = worldgen / "configured_feature" / f"{n}.json"
        feature = json.loads(path.read_text())
        feature["size"] = size
        write_json(path, feature)
        copy(occ_data / MOD / "worldgen" / "placed_feature" / f"{n}.json", worldgen / "placed_feature" / f"{n}.json")
        copy(occ_data / MOD / "neoforge" / "biome_modifier" / f"add_{n}.json",
             data / "neoforge" / "biome_modifier" / f"add_{n}.json")

    # --- The Third Eye: a mob effect (the native mod reads who has it) and its sources.
    write_json(data / "horizon" / "mob_effect" / "third_eye.json", {"category": "beneficial", "color": 0x6A3D9A})
    copy_texture(occ_assets / "textures" / "mob_effect" / "third_eye.png", assets / "textures" / "mob_effect" / "third_eye.png")
    lang_of(f"effect.{MOD}.third_eye")
    effect = lambda ident, seconds, amp=0: {"id": ident, "duration": seconds * 20, "amplifier": amp, "show_icon": True}
    flat_item("demons_dream_essence", "minecraft:honey_bottle", extra={"components": {
        "minecraft:food": {"can_always_eat": True, "nutrition": 0, "saturation": 0.0},
        "minecraft:consumable": {"animation": "drink", "consume_seconds": 2.0, "has_consume_particles": False,
                                 "sound": "minecraft:item.honey_bottle.drink", "on_consume_effects": [
            {"type": "minecraft:apply_effects", "effects": [effect(f"{MOD}:third_eye", 60)]},
            {"type": "minecraft:apply_effects", "effects": [effect("minecraft:hunger", 15, 1)]},
            *[{"type": "minecraft:apply_effects", "probability": 0.2, "effects": [effect(e, s, 1)]}
              for e, s in [("minecraft:darkness", 15), ("minecraft:glowing", 15), ("minecraft:luck", 300),
                           ("minecraft:unluck", 300), ("minecraft:nausea", 15)]]]}}})
    write_json(data / "recipe" / "demons_dream_essence.json", {
        "type": "minecraft:crafting_shapeless", "category": "misc",
        "ingredients": ["minecraft:honey_bottle", "minecraft:nether_wart", "minecraft:glow_berries", "minecraft:fermented_spider_eye"],
        "result": {"id": f"{MOD}:demons_dream_essence", "count": 1}})

    # --- Spirit-attuned gems and crystal, lenses and the goggles.
    flat_item("spirit_attuned_gem", "minecraft:diamond")
    write_json(data / "recipe" / "spirit_attuned_gem.json", {
        "type": "minecraft:crafting_shapeless", "category": "misc",
        "ingredients": ["minecraft:diamond", "minecraft:amethyst_shard", "minecraft:lapis_lazuli", "minecraft:glowstone_dust"],
        "result": {"id": f"{MOD}:spirit_attuned_gem", "count": 1}})
    model_block("spirit_attuned_crystal", "minecraft:glass", "spirit_attuned_crystal",
                {"hardness": 1.5, "light": 7, "sounds": "minecraft:amethyst_block", "requires_tool": False})
    for name in ("lenses", "infused_lenses", "lens_frame"):
        flat_item(name, "minecraft:paper")
    write_json(data / "recipe" / "lens_frame.json", {  # Occultism's, polished deepslate for otherstone
        "type": "minecraft:crafting_shaped", "category": "misc",
        "pattern": ["ooo", "s s", "ooo"], "key": {"o": "minecraft:polished_deepslate", "s": "#c:ingots/silver"},
        "result": {"id": f"{MOD}:lens_frame", "count": 1}})
    flat_item("otherworld_goggles", "minecraft:leather_helmet", extra={"components": {
        "minecraft:equippable": {"slot": "head", "equip_sound": "minecraft:item.armor.equip_leather"}}})

    # --- Binding books: the rituals' activation items, one per rank (Occultism binds a book to a
    # spirit's name with the Dictionary of Spirits; here it is crafted bound).
    for rank, dye in BOOK_DYES.items():
        name = f"book_of_binding_bound_{rank}"
        flat_item(name, "minecraft:book", extra={"max_stack_size": 1, "components": {"!minecraft:enchantable": {}}})
        write_json(data / "recipe" / f"{name}.json", {
            "type": "minecraft:crafting_shapeless", "category": "misc",
            "ingredients": ["minecraft:book", "minecraft:ink_sac", "minecraft:feather", f"minecraft:{dye}_dye"],
            "result": {"id": f"{MOD}:{name}", "count": 1}})

    # Occultism's own crafting, smelting and blasting recipes for what exists now (silver and
    # iesnium storage, nuggets, smelting, the golden bowl, crystal, lenses, goggles...).
    def ingredient_ids(value):
        if isinstance(value, str):
            yield value
        elif isinstance(value, list):
            for v in value:
                yield from ingredient_ids(v)
        elif isinstance(value, dict):
            for k in ("item", "tag", "id"):
                if k in value:
                    yield ("#" + value[k]) if k == "tag" and not value[k].startswith("#") else value[k]

    def resolves(ident):
        return bool(tags.resolve(ident[1:])) if ident.startswith("#") else tags.known(ident)

    ours = {p.stem for p in (data / "recipe").glob("*.json")}
    copied = []
    for kind in ("crafting", "smelting", "blasting"):
        for f in sorted((occ_data / MOD / "recipe" / kind).glob("*.json")):
            j = json.loads(f.read_text())
            j.pop("neoforge:conditions", None)
            result = j.get("result", {})
            rid = result.get("id") if isinstance(result, dict) else None
            if j.get("type") not in ("minecraft:crafting_shaped", "minecraft:crafting_shapeless",
                                     "minecraft:smelting", "minecraft:blasting"):
                continue
            if rid not in items or rid.split(":")[1] in ours or rid.startswith(f"{MOD}:book_of_binding"):
                continue
            ins = list(ingredient_ids(list((j.get("key") or {}).values()))) + list(ingredient_ids(j.get("ingredients"))) + list(ingredient_ids(j.get("ingredient")))
            if ins and all(resolves(i) for i in ins):
                write_json(data / "recipe" / f"{kind}_{f.name}", j)
                copied.append(f"{kind}/{f.stem}")

    # --- Crushing recipes that work with vanilla plus this port's items.
    kept, dropped = [], []
    for f in sorted((occ_data / MOD / "recipe" / "crushing").glob("*.json")):
        j = json.loads(f.read_text())
        j.pop("neoforge:conditions", None)
        ing = j["ingredient"]
        ing_items = []
        for part in ing if isinstance(ing, list) else [ing]:
            part = part.get("tag", part.get("item")) if isinstance(part, dict) else part
            ing_items += tags.resolve(part[1:]) if part.startswith("#") else ([part] if tags.known(part) else [])
        res = j["result"]
        if res.get("type") == f"{MOD}:tag":
            outs = tags.resolve(res["tag"])
        else:
            outs = [res["id"]] if tags.known(res["id"]) else []
        if ing_items and outs:
            kept.append(f.stem)
            write_json(data / "recipe" / "crushing" / f.name, j)
        else:
            dropped.append(f.stem)

    # --- Pentacles, from Occultism's multiblocks: each key as the blocks it accepts ("any" for
    # Occultism's display-only floor and `_`), the native mod checks them around the golden bowl.
    block_tags = {}
    for f in sorted((occ_data / MOD / "tags" / "block").glob("*.json")):
        block_tags[f"{MOD}:{f.stem}"] = json.loads(f.read_text())["values"]
    block_tags["minecraft:candles"] = CANDLES
    pentacles = {}
    for name in PENTACLES:
        mb = json.loads((occ_data / MOD / "modonomicon" / "multiblocks" / f"{name}.json").read_text())
        if mb["type"] != "modonomicon:dense":
            die(f"pentacle {name}: {mb['type']} isn't read")
        keys = {"_": "any"}
        for ch, m in mb["mapping"].items():
            if m["type"] == "modonomicon:display":
                keys[ch] = "any"
            elif m["type"] == "modonomicon:block":
                keys[ch] = [m["block"]]
            elif m["type"] == "modonomicon:tag":
                accepted = [b for b in block_tags.get(m["tag"].lstrip("#"), [])
                            if b.startswith("minecraft:") or b in items or b in glyphs]
                if not accepted:
                    die(f"pentacle {name}: nothing this port has is in {m['tag']}")
                keys[ch] = accepted
            else:
                die(f"pentacle {name}: mapping type {m['type']} isn't read")
        for ch in "".join("".join(layer) for layer in mb["pattern"]):
            if ch not in keys and ch != " ":
                die(f"pentacle {name}: key {ch!r} has no mapping")
        for accepted in keys.values():
            if accepted != "any":
                for b in accepted:
                    if not (b.startswith("minecraft:") or b in items or b in glyphs):
                        die(f"pentacle {name}: {b} isn't in this port")
        pentacles[f"{MOD}:{name}"] = {"pattern": mb["pattern"], "keys": keys}
        lang_of(f"multiblock.{MOD}.{name}", name.replace("_", " ").title())
    write_json(data / "pentacles.json", pentacles)

    # --- Ritual recipes (`occultism:ritual`, read by the native mod as the crushing ones are).
    for name, result in RITUALS.items():
        j = json.loads((occ_data / MOD / "recipe" / "ritual" / f"{name}.json").read_text())
        j.pop("neoforge:conditions", None)
        if j["pentacle_id"].split(":")[1] not in PENTACLES:
            die(f"ritual {name}: pentacle {j['pentacle_id']} isn't shipped")
        ritual = {"type": f"{MOD}:ritual", "activation_item": j["activation_item"], "duration": j["duration"],
                  "ingredients": j["ingredients"], "pentacle_id": j["pentacle_id"],
                  "result": {"id": result or j["result"]["id"], "count": j["result"].get("count", 1)}}
        for ident in [ritual["activation_item"], ritual["result"]["id"]] + list(ingredient_ids(ritual["ingredients"])):
            if not resolves(ident):
                die(f"ritual {name}: {ident} isn't in this port")
        write_json(data / "recipe" / "ritual" / f"{name}.json", ritual)
    lang[f"ritual.{MOD}.started"] = "The ritual has begun."
    lang[f"ritual.{MOD}.finished"] = "The ritual is complete."
    lang[f"ritual.{MOD}.interrupted"] = "The ritual was interrupted."
    lang[f"ritual.{MOD}.no_pentacle"] = "The circle around this bowl isn't complete."
    lang[f"ritual.{MOD}.no_ingredients"] = "The sacrificial bowls don't hold what this ritual needs."
    lang[f"ritual.{MOD}.busy"] = "A ritual is already under way here."

    write_json(out / "data" / "minecraft" / "tags" / "block" / "mineable" / "pickaxe.json", {"values": pickaxe})
    write_json(data / "horizon" / "creative_tab" / "occultism.json", {
        "title": f"itemGroup.{MOD}.occultism", "icon": f"{MOD}:demonic_altar",
        "items": tab + [f"{MOD}:{d}_dust" for d in DUSTS]})
    lang[f"itemGroup.{MOD}.occultism"] = "Occultism"

    write_json(assets / "lang" / "en_us.json", dict(sorted(lang.items())))

    print(f"content mod written to {out}")
    print(f"  items: {len(items)} ({len(DUSTS)} dusts, {len(TIERS)} cores, {len(CHALKS)} chalks and glyphs, ...) + the altar")
    print(f"  crushing recipes: {len(kept)} kept, {len(dropped)} need other mods' items")
    print(f"  Occultism recipes copied: {len(copied)}: " + ", ".join(copied))
    print(f"  pentacles: {len(pentacles)}, rituals: {len(RITUALS)}")
    print("  kept: " + ", ".join(kept))


if __name__ == "__main__":
    main()
