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
import math
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
PENTACLES = ["summon_foliot", "summon_djinni", "summon_afrit", "summon_marid", "craft_foliot", "craft_djinni", "craft_afrit",
             "contact_wild_spirit", "contact_eldritch_spirit"]
RITUALS = {
    "summon_foliot_crusher": f"{MOD}:crusher_core_foliot",
    "summon_djinni_crusher": f"{MOD}:crusher_core_djinni",
    "summon_afrit_crusher": f"{MOD}:crusher_core_afrit",
    "summon_marid_crusher": f"{MOD}:crusher_core_marid",
    "craft_infused_lenses": None,  # Occultism's own result
    "craft_miner_foliot_unspecialized": None,
    "craft_miner_djinni_ores": None,
    # Storage, as Occultism makes it (its stabilizers are this port's plates: ALIASES).
    "craft_storage_controller_base": None,
    "craft_dimensional_matrix": None,
    "craft_stabilizer_tier1": None,
    "craft_stabilizer_tier2": None,
    "craft_stable_wormhole": None,
    "craft_storage_remote": None,
    "craft_dimensional_mineshaft": None,
    # Repairs: the worn item back whole (the native mod's `occultism:repair`).
    "repair_chalks": None,
    "repair_miners": None,
    "repair_tools": None,
    "repair_armors": None,
    # Sacrifices: the ritual waits for a player to kill one of its animals near the bowl.
    "misc_bee_nest": None,
    "misc_bell": None,
    "misc_budding_amethyst": None,
    "misc_copper_horse_armor": None,
    "misc_iron_horse_armor": None,
    "misc_golden_horse_armor": None,
    "misc_diamond_horse_armor": None,
    "misc_copper_nautilus_armor": None,
    "misc_iron_nautilus_armor": None,
    "misc_golden_nautilus_armor": None,
    "misc_diamond_nautilus_armor": None,
    "misc_reinforced_deepslate": None,
    "misc_wild_trim": None,
}
# This port's storage is one block with its upgrades as item plates (docs/storage-design.md):
# Occultism's stabilizers are its plates and Occultism's storage controller its Actuator, in the
# recipes and rituals copied from Occultism.
ALIASES = {f"{MOD}:storage_stabilizer_tier{t}": f"{MOD}:storage_plate_tier{t}" for t in range(6)}
ALIASES[f"{MOD}:storage_controller"] = f"{MOD}:storage_actuator"


def alias(recipe):
    """`recipe` (an Occultism recipe's JSON) naming this port's items for Occultism's."""
    text = json.dumps(recipe)
    for old, new in ALIASES.items():
        text = text.replace(f'"{old}"', f'"{new}"')
    return json.loads(text)
CANDLES = ["minecraft:candle"] + [f"minecraft:{c}_candle" for c in CHALKS]
# Vanilla's, as 26.3 has it: the contact pentacles' rods.
LIGHTNING_RODS = [f"minecraft:{w}{o}lightning_rod" for w in ("", "waxed_")
                  for o in ("", "exposed_", "weathered_", "oxidized_")]
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
    "storage_blocks/copper": ["minecraft:copper_block"],
    # What Occultism's repair rituals take (NeoForge's `c:tools` and `c:armors`, by vanilla's own tags).
    "tools": ["#minecraft:pickaxes", "#minecraft:axes", "#minecraft:shovels", "#minecraft:hoes", "#minecraft:swords",
              "minecraft:shears", "minecraft:fishing_rod", "minecraft:flint_and_steel", "minecraft:bow",
              "minecraft:crossbow", "minecraft:trident", "minecraft:mace", "minecraft:brush", "minecraft:shield"],
    "armors": ["#minecraft:head_armor", "#minecraft:chest_armor", "#minecraft:leg_armor", "#minecraft:foot_armor"],
    "glass_panes": ["minecraft:glass_pane", "minecraft:white_stained_glass_pane", "minecraft:orange_stained_glass_pane", "minecraft:magenta_stained_glass_pane", "minecraft:light_blue_stained_glass_pane", "minecraft:yellow_stained_glass_pane", "minecraft:lime_stained_glass_pane", "minecraft:pink_stained_glass_pane", "minecraft:gray_stained_glass_pane", "minecraft:light_gray_stained_glass_pane", "minecraft:cyan_stained_glass_pane", "minecraft:purple_stained_glass_pane", "minecraft:blue_stained_glass_pane", "minecraft:brown_stained_glass_pane", "minecraft:green_stained_glass_pane", "minecraft:red_stained_glass_pane", "minecraft:black_stained_glass_pane"],
    "nuggets/gold": ["minecraft:gold_nugget"],
    "stones": ["minecraft:stone", "minecraft:andesite", "minecraft:diorite", "minecraft:granite", "minecraft:tuff",
               "minecraft:deepslate"],
    "foods/raw_fish": ["minecraft:cod", "minecraft:salmon", "minecraft:tropical_fish"],
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
    "rods/wooden": ["minecraft:stick"],
    "storage_blocks/iron": ["minecraft:iron_block"],
    "storage_blocks/gold": ["minecraft:gold_block"],
    "feathers": ["minecraft:feather"],
    "dyes/black": ["minecraft:black_dye"],
    "dyes/blue": ["minecraft:blue_dye"],
    "dyes/purple": ["minecraft:purple_dye"],
    "dyes/yellow": ["minecraft:yellow_dye"],
    "dyes/green": ["minecraft:green_dye"],
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
    "minecraft:coals": ["minecraft:coal", "minecraft:charcoal"],
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


def entity_tag(occ_data: Path, tag: str, seen=None) -> list:
    """The vanilla entity types in an entity type tag of Occultism's, nested tags followed."""
    seen = seen or set()
    if tag in seen:
        return []
    seen.add(tag)
    ns, path = tag.split(":")
    f = occ_data / ns / "tags" / "entity_type" / f"{path}.json"
    out = []
    for entry in json.loads(f.read_text())["values"] if f.exists() else []:
        ident = entry["id"] if isinstance(entry, dict) else entry
        for kind in entity_tag(occ_data, ident[1:], seen) if ident.startswith("#") else [ident]:
            if kind.startswith("minecraft:") and kind not in out:
                out.append(kind)
    return out


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
        for f in sorted((occ_data / MOD / "tags" / "item").rglob("*.json")):
            name = f"{MOD}:" + f.relative_to(occ_data / MOD / "tags" / "item").with_suffix("").as_posix()
            self.files[name] = json.loads(f.read_text()).get("values", [])
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

    def matches_any(self, tag: str) -> bool:
        """Whether the game will find an item in a tag: vanilla's own tags (#minecraft:...) count,
        though this script doesn't list their items."""
        return tag.startswith("minecraft:") or bool(self.resolve(tag)) or any(
            (e["id"] if isinstance(e, dict) else e).startswith("#minecraft:") for e in self.files.get(tag, []))


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
version = "0.5.0-alpha.2"
description = "Occultism's crusher spirits bound into spirit cores that power a Demonic Altar. Port by permission of Kli Kli; needs the occultism_native native mod."
authors = ["klikli-dev (Occultism)", "Horizon port"]
api = "0.2"
side = "both"
capabilities = ["registries"]
'''
    )
    copy(occ / "LICENSE", out / "LICENSE-occultism.txt")
    copy(occ / "THIRD_PARTY_NOTICES.md", out / "THIRD_PARTY_NOTICES-occultism.md")
    # The notices point here for the textures based on Bonemouse's Isabella pack (CC BY 3.0).
    copy(occ_assets / "textures" / "about_isabella.txt", assets / "textures" / "about_isabella.txt")

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
        elif key not in lang:  # named by this port already
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

    def model_block(name, template, model, props, shapes=None, lang_key=None, components=None, loot=None):
        """A block drawn with one of Occultism's own models (its textures copied), and its item."""
        ident = f"{MOD}:{name}"
        items.add(ident)
        decl = {"template": template, "item": True, "properties": props}
        if components:
            decl["components"] = components
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
        write_json(data / "loot_table" / "blocks" / f"{name}.json", loot or loot_self(ident))
        lang[f"block.{MOD}.{name}"] = lang_src[f"block.{MOD}.{lang_key or name}"]
        pickaxe.append(ident)
        tab.append(ident)

    # --- Chalks and their glyphs: a chalk draws its glyph and wears by one (the native mod's use
    # hook), 256 times, as Occultism's; a glyph is a flat carpet with a sign (1..14, Occultism's 15
    # typefaces less the preview) and a facing, and has no item.
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
        glyph = f"chalk_glyph_{colour}"
        glyphs.add(f"{MOD}:{glyph}")
        chalk = f"chalk_{colour}"
        items.add(f"{MOD}:{chalk}")
        write_json(data / "horizon" / "item" / f"{chalk}.json", {
            "template": "minecraft:paper", "max_stack_size": 1,
            "components": {"minecraft:max_damage": 256, "minecraft:damage": 0}})
        # Leaf litter: flat, see-through (drawn cut-out), no collision, a horizontal facing, and it
        # needs a sturdy face below, as Occultism's glyph does. Its segment_amount stays 1.
        write_json(data / "horizon" / "block" / f"{glyph}.json", {
            "template": "minecraft:leaf_litter", "item": False,
            "properties": {"hardness": 0.0, "sounds": "minecraft:stone"},
            "state_properties": {"sign": {"type": "enum", "values": [str(i) for i in range(GLYPH_SIGNS + 1)]}},
            "default_state": {"sign": "1"}})
        # A glyph drops nothing, as Occultism's (without a table it would drop its template's leaf litter).
        write_json(data / "loot_table" / "blocks" / f"{glyph}.json",
                   {"type": "minecraft:block", "pools": [], "random_sequence": f"{MOD}:blocks/{glyph}"})
        item_def = json.loads((gen_assets / "items" / f"{chalk}.json").read_text())
        item_def["model"]["model"] = f"{MOD}:item/chalk"
        write_json(assets / "items" / f"{chalk}.json", item_def)
        lang[f"item.{MOD}.{chalk}"] = lang_src[f"item.{MOD}.{chalk}"]
        lang[f"block.{MOD}.{glyph}"] = lang_src[f"item.{MOD}.{chalk}"]
        tab.append(f"{MOD}:{chalk}")
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
    # White chalk is Occultism's: impure chalk purified in spirit fire (below). The other colours are
    # dyed from it here too, where Occultism's own needs what this port doesn't have yet.
    for colour in CHALKS:
        if colour != "white":
            write_json(data / "recipe" / f"chalk_{colour}.json", {
                "type": "minecraft:crafting_shapeless", "category": "misc",
                "ingredients": [f"{MOD}:chalk_white", f"minecraft:{colour}_dye"],
                "result": {"id": f"{MOD}:chalk_{colour}", "count": 1}})

    # --- Sacrificial bowls: glass templates (they don't hide their neighbours' faces), with
    # Occultism's models. The native mod gives each a one-slot block entity.
    bowl_props = {"hardness": 1.5, "explosion_resistance": 6.0, "sounds": "minecraft:stone", "requires_tool": False}
    model_block("sacrificial_bowl", "minecraft:glass", "sacrificial_bowl", bowl_props)
    model_block("golden_sacrificial_bowl", "minecraft:glass", "golden_sacrificial_bowl", bowl_props)

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

    # --- Datura (Demon's Dream): seeds from grass and ferns (2%, not shears: the native mod's loot
    # hook), a crop on farmland, a fruit that lights spirit fire and makes the essence. Horizon's
    # items can't place a block of another name, so the crop is the seeds' own block,
    # `datura_seeds` (Occultism's is `datura`).
    items.update({f"{MOD}:datura_seeds", f"{MOD}:datura"})
    write_json(data / "horizon" / "block" / "datura_seeds.json", {
        "template": "minecraft:wheat", "item": True, "components": {"!minecraft:compostable": {}}})
    datura_states = json.loads((occ_assets / "blockstates" / "datura.json").read_text())["variants"]
    copy(occ_assets / "models" / "block" / "cross_crop.json", assets / "models" / "block" / "cross_crop.json")
    for stage in sorted({v["model"].split(":block/", 1)[1] for v in datura_states.values()}):
        copy(occ_assets / "models" / "block" / f"{stage}.json", assets / "models" / "block" / f"{stage}.json")
        copy_texture(occ_assets / "textures" / "block" / f"{stage}.png", assets / "textures" / "block" / f"{stage}.png")
    write_json(assets / "blockstates" / "datura_seeds.json", {"variants": datura_states})
    copy_texture(occ_assets / "textures" / "item" / "datura_seeds.png", assets / "textures" / "item" / "datura_seeds.png")
    write_json(assets / "models" / "item" / "datura_seeds.json",
               {"parent": "minecraft:item/generated", "textures": {"layer0": f"{MOD}:item/datura_seeds"}})
    write_json(assets / "items" / "datura_seeds.json", {"model": {"type": "minecraft:model", "model": f"{MOD}:item/datura_seeds"}})
    lang[f"block.{MOD}.datura_seeds"] = lang_src[f"tag.item.c.seeds.datura"]  # "Demon's Dream Seeds"
    tab.append(f"{MOD}:datura_seeds")
    # Occultism's datura table (the fruit when ripe, else a seed; ripe, more seeds by fortune) is
    # vanilla's wheat one, written here in 26.3's format.
    ripe = {"type": "minecraft:match_block", "blocks": f"{MOD}:datura_seeds", "state": {"age": "7"}}
    write_json(data / "loot_table" / "blocks" / "datura_seeds.json", {
        "type": "minecraft:block", "modifier": {"type": "minecraft:explosion_decay"},
        "pools": [{"rolls": 1, "entries": [{"type": "minecraft:alternatives", "children": [
                      {"type": "minecraft:item", "condition": ripe, "name": f"{MOD}:datura"},
                      {"type": "minecraft:item", "name": f"{MOD}:datura_seeds"}]}]},
                  {"rolls": 1, "condition": ripe, "entries": [{"type": "minecraft:item", "name": f"{MOD}:datura_seeds", "modifier": {
                      "type": "minecraft:apply_bonus", "enchantment": "minecraft:fortune",
                      "formula": "minecraft:binomial_with_bonus_count", "parameters": {"extra": 3, "probability": 0.5714286}}}]}],
        "random_sequence": f"{MOD}:blocks/datura_seeds"})
    flat_item("datura", "minecraft:carrot", extra={"components": {
        "minecraft:food": {"can_always_eat": True, "nutrition": 0, "saturation": 0.0},
        "minecraft:consumable": {"on_consume_effects": [
            {"type": "minecraft:apply_effects", "probability": 0.7, "effects": [effect(f"{MOD}:third_eye", 15)]},
            {"type": "minecraft:apply_effects", "effects": [effect("minecraft:hunger", 15, 1)]},
            {"type": "minecraft:apply_effects", "probability": 0.3, "effects": [effect("minecraft:nausea", 15, 1)]}]}}})
    lang[f"item.{MOD}.dictionary_of_spirits"] = "Dictionary of Spirits"  # Occultism's book names it
    flat_item("dictionary_of_spirits", "minecraft:book",
              extra={"max_stack_size": 1, "crafting_remainder": f"{MOD}:dictionary_of_spirits",
                     "components": {"!minecraft:enchantable": {}}})

    # --- Spirit fire: lit by flint and steel by Demon's Dream fruit (the native mod), it turns what
    # is thrown in by Occultism's `occultism:spirit_fire` recipes. On glass, not fire: a fire
    # template would burn the items, spread and go out. No collision, breaks at a touch, light 12.
    items.add(f"{MOD}:spirit_fire")
    write_json(data / "horizon" / "block" / "spirit_fire.json", {
        "template": "minecraft:glass", "item": True,
        "properties": {"hardness": 0.0, "explosion_resistance": 0.0, "light": 12,
                       "sounds": "minecraft:white_wool", "requires_tool": False},
        "shapes": {"default": [[0, 0, 0, 1, 0.0625, 1]]}, "collision_shapes": {"default": []}})
    fire_model = json.loads((occ_assets / "models" / "block" / "spirit_fire.json").read_text())
    for element in fire_model["elements"]:
        for face in element["faces"].values():
            face.pop("tintindex", None)  # Occultism's white spirit fire: its texture as it is
    write_json(assets / "models" / "block" / "spirit_fire.json", fire_model)
    copy_texture(occ_assets / "textures" / "block" / "spirit_fire_0.png", assets / "textures" / "block" / "spirit_fire_0.png")
    write_json(assets / "blockstates" / "spirit_fire.json", {"variants": {"": {"model": f"{MOD}:block/spirit_fire"}}})
    write_json(assets / "models" / "item" / "spirit_fire.json",
               {"parent": "minecraft:item/generated", "textures": {"layer0": f"{MOD}:block/spirit_fire_0"}})
    write_json(assets / "items" / "spirit_fire.json", {"model": {"type": "minecraft:model", "model": f"{MOD}:item/spirit_fire"}})
    write_json(data / "loot_table" / "blocks" / "spirit_fire.json",
               {"type": "minecraft:block", "pools": [], "random_sequence": f"{MOD}:blocks/spirit_fire"})
    lang_of(f"block.{MOD}.spirit_fire")
    tab.append(f"{MOD}:spirit_fire")
    # What spirit fire makes, as Occultism's items (their recipes are Occultism's, copied below).
    cube_block("otherstone", "minecraft:stone", props={"hardness": 1.5, "explosion_resistance": 30.0, "requires_tool": True})
    cube_block("otherrock", "minecraft:stone", props={"hardness": 1.5, "explosion_resistance": 30.0, "requires_tool": True})
    cube_block("spirit_attuned_block", "minecraft:diamond_block", props={"requires_tool": True})
    for name in ("taboo_book", "purified_ink", "awakened_feather"):
        flat_item(name, "minecraft:paper")
    flat_item("otherworld_essence", "minecraft:honey_bottle", extra={"components": {
        "minecraft:food": {"can_always_eat": True, "nutrition": 0, "saturation": 0.0},
        "minecraft:consumable": {"animation": "drink", "consume_seconds": 2.0, "has_consume_particles": False,
                                 "sound": "minecraft:item.honey_bottle.drink", "on_consume_effects": [
            {"type": "minecraft:apply_effects", "effects": [effect(f"{MOD}:third_eye", 60)]},
            {"type": "minecraft:apply_effects", "effects": [effect("minecraft:luck", 300, 1)]}]}}})

    # --- Spirit-attuned gems and crystal, lenses and the goggles.
    flat_item("spirit_attuned_gem", "minecraft:diamond")  # a diamond in spirit fire
    model_block("spirit_attuned_crystal", "minecraft:glass", "spirit_attuned_crystal",
                {"hardness": 1.5, "light": 7, "sounds": "minecraft:amethyst_block", "requires_tool": False})
    for name in ("lenses", "infused_lenses", "lens_frame"):
        flat_item(name, "minecraft:paper")
    flat_item("otherworld_goggles", "minecraft:leather_helmet", extra={"components": {
        "minecraft:equippable": {"slot": "head", "equip_sound": "minecraft:item.armor.equip_leather"}}})

    # --- Binding books: the rituals' activation items, one per rank, as Occultism's: a taboo book,
    # purified ink and an awakened feather (spirit fire's) make a book of binding, dyed for its rank
    # (Occultism's recipes, copied below). Occultism then binds it to a spirit's name with the
    # Dictionary of Spirits; here the book and the Dictionary make the bound book and the
    # Dictionary stays in the grid (its crafting remainder). The spirit's name isn't written yet.
    flat_item("book_of_binding_empty", "minecraft:book", extra={"max_stack_size": 16, "components": {"!minecraft:enchantable": {}}})
    for rank in BOOK_DYES:
        flat_item(f"book_of_binding_{rank}", "minecraft:book",
                  extra={"max_stack_size": 16, "components": {"!minecraft:enchantable": {}}})
        name = f"book_of_binding_bound_{rank}"
        flat_item(name, "minecraft:book", extra={"max_stack_size": 1, "components": {"!minecraft:enchantable": {}}})
        write_json(data / "recipe" / f"{name}.json", {
            "type": "minecraft:crafting_shapeless", "category": "misc",
            "ingredients": [f"{MOD}:book_of_binding_{rank}", f"{MOD}:dictionary_of_spirits"],
            "result": {"id": f"{MOD}:{name}", "count": 1}})

    # --- Storage (docs/storage-design.md): one block, Occultism's Dimensional Storage Actuator, holds
    # a store and is its terminal; Occultism's dimensional matrix floats over it, a chalk glyph
    # circling it for each upgrade (drawn by the native mod). Prototype: no recipes yet.
    plate_props = {"hardness": 1.5, "explosion_resistance": 6.0, "sounds": "minecraft:stone", "requires_tool": False}
    # The Actuator is its store's anchor: as an item it carries the store (the native mod writes
    # `occultism:store` on it), so it always drops, even in an explosion, and doesn't burn.
    actuator_loot = loot_self(f"{MOD}:storage_actuator")
    del actuator_loot["pools"][0]["condition"]
    model_block("storage_actuator", "minecraft:barrel", "storage_controller",
                {**plate_props, "explosion_resistance": 1200.0}, lang_key="storage_controller",
                components={"minecraft:damage_resistant": {"types": "#minecraft:is_fire"}}, loot=actuator_loot)
    variants = {f"facing={f},open={o}": {"model": f"{MOD}:block/storage_actuator"}
                for f in ["down", "up", "north", "south", "west", "east"] for o in ["false", "true"]}
    write_json(assets / "blockstates" / "storage_actuator.json", {"variants": variants})
    # 9 slots hoppers fill (the screen shows none), 4 numbers, then 6 upgrade slots' plate tiers.
    write_json(data / "horizon" / "menu" / "storage_actuator.json",
               {"slots": 18, "data_slots": 10, "results": list(range(9, 18))})
    lang[f"container.{MOD}.storage_actuator"] = lang_src[f"block.{MOD}.storage_controller"]
    # The projection: Occultism's matrix texture (its glyphs are the chalks' own textures).
    copy_texture(occ_assets / "textures" / "block" / "dimensional_matrix.png", assets / "textures" / "block" / "dimensional_matrix.png")
    # The glyphs' glow: a soft white disc, tinted by the native mod, at a few strengths so a trail
    # can fade out (a quad's colour can't carry transparency; its texture's can). The game drops
    # pixels under about a tenth opaque, so where the glow is fainter than that it is dithered:
    # fewer and fewer pixels at that tenth towards the rim, a haze of motes rather than a disc.
    floor = 0.11
    bayer = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]]
    for n, strength in enumerate([0.34, 0.26, 0.2, 0.15]):
        size, rows = 32, []
        for y in range(size):
            row = []
            for x in range(size):
                r = math.hypot((x + 0.5) / size * 2 - 1, (y + 0.5) / size * 2 - 1)
                want = strength * max(0.0, 1 - r) ** 2
                if want < floor:
                    want = floor if want / floor > (bayer[y % 4][x % 4] + 0.5) / 16 else 0
                row += [255, 255, 255, int(255 * want)]
            rows.append(row)
        write_png(assets / "textures" / "block" / f"storage_glow_{n}.png", size, size, rows)
    # Stabilizer plates are upgrades kept by the store, not blocks: items drawn with Occultism's
    # stabilizer models.
    for tier in range(1, 5):
        name = f"storage_plate_tier{tier}"
        src = json.loads((main_res / "assets" / MOD / "models" / "block" / f"storage_stabilizer_tier{tier}.json").read_text())
        for tex in src.get("textures", {}).values():
            if tex.startswith(f"{MOD}:block/"):
                rel = tex.split("/", 1)[1]
                copy_texture(occ_assets / "textures" / "block" / f"{rel}.png", assets / "textures" / "block" / f"{rel}.png")
        parent = src.get("parent", "")
        if parent.startswith(f"{MOD}:block/"):  # tiers 2 to 4 build on tier 1's model
            base = parent.split("/", 1)[1]
            base_src = json.loads((main_res / "assets" / MOD / "models" / "block" / f"{base}.json").read_text())
            for tex in base_src.get("textures", {}).values():
                if tex.startswith(f"{MOD}:block/"):
                    rel = tex.split("/", 1)[1]
                    copy_texture(occ_assets / "textures" / "block" / f"{rel}.png", assets / "textures" / "block" / f"{rel}.png")
            write_json(assets / "models" / "block" / f"{base}.json", base_src)
        write_json(assets / "models" / "block" / f"{name}.json", src)
        lang_src[f"item.{MOD}.{name}"] = lang_src[f"block.{MOD}.storage_stabilizer_tier{tier}"]
        flat_item(name, "minecraft:amethyst_shard", block=f"{MOD}:block/{name}", extra={"max_stack_size": 16})
    lang[f"gui.{MOD}.storage.upgrade"] = "Upgrade slot"
    lang[f"gui.{MOD}.storage.upgrade_empty"] = "Shift-click a storage stabilizer to put it here"
    lang[f"gui.{MOD}.storage.upgrade_gives"] = "+%s types, +%s items"
    lang[f"gui.{MOD}.storage.upgrade_take"] = "Click to take it out"
    lang[f"storage.{MOD}.bound"] = "Bound to %s"
    lang[f"storage.{MOD}.dormant"] = "%s is dormant until its Actuator is placed again."
    lang[f"storage.{MOD}.woke"] = "%s wakes."
    lang[f"storage.{MOD}.named"] = "A spirit named %s now keeps this store."
    lang[f"storage.{MOD}.copy"] = "%s is already anchored elsewhere; this Actuator binds a new spirit."
    lang[f"storage.{MOD}.refused"] = "%s does not answer you."
    lang[f"storage.{MOD}.trusted"] = "%s may now use %s."
    lang[f"storage.{MOD}.untrusted"] = "%s may no longer use %s."
    lang[f"storage.{MOD}.trust.none"] = "Open a store's terminal you own first."
    lang[f"storage.{MOD}.trust.not_owner"] = "Only the store's owner can choose who it trusts."
    lang[f"storage.{MOD}.trust.unknown"] = "No player named %s is here."
    lang[f"storage.{MOD}.trust.list"] = "%s trusts: %s"
    lang[f"storage.{MOD}.stale"] = "A recall moved %s to another Actuator; this one binds a new spirit."
    lang[f"gui.{MOD}.storage.search"] = "Search..."
    lang[f"gui.{MOD}.storage.stored"] = "stored"
    lang[f"gui.{MOD}.storage.sort.count"] = "Count"
    lang[f"gui.{MOD}.storage.sort.name"] = "Name"
    lang[f"gui.{MOD}.storage.sort.mod"] = "Mod"

    # --- Item models straight from Occultism (and the models and textures they build on).
    def occ_model(kind, name, dest=None):
        """Copies Occultism's `models/<kind>/<name>.json` as `dest` (or the same name), with every
        model of its own it builds on and every texture it names; the model's id."""
        def find(k, n):
            for root in (gen_assets, occ_assets):
                f = root / "models" / k / f"{n}.json"
                if f.exists():
                    return f
            die(f"no Occultism model {k}/{n}")
        def take(k, n, out_name):
            src = json.loads(find(k, n).read_text())
            parent = src.get("parent", "")
            if parent.startswith(f"{MOD}:"):
                pk, pn = parent.split(":", 1)[1].split("/", 1)
                take(pk, pn, pn)
            for tex in src.get("textures", {}).values():
                if tex.startswith(f"{MOD}:"):
                    rel = tex.split(":", 1)[1]
                    copy_texture(occ_assets / "textures" / f"{rel}.png", assets / "textures" / f"{rel}.png")
            write_json(assets / "models" / k / f"{out_name}.json", src)
        take(kind, name, dest or name)
        return f"{MOD}:{kind}/{dest or name}"

    # --- Magic lamps and miner spirits (Occultism's dimensional mineshaft). A miner is a spirit in a
    # lamp: its mining time, rolls and output multiplier ride on the item (`occultism:miner`, read by
    # the native mod), its durability is Occultism's config default.
    flat_item("magic_lamp_empty", "minecraft:paper", block=occ_model("item", "magic_lamp_empty"))
    flat_item("iesnium_pickaxe", "minecraft:diamond_pickaxe", block=occ_model("item", "iesnium_pickaxe"))
    MINERS = {  # name: (ticks per operation, rolls, output multiplier, durability, rarity)
        "miner_foliot_unspecialized": (400, 1, 1, 511, "common"),
        "miner_djinni_ores": (300, 1, 1, 1023, "common"),
        "miner_afrit_deeps": (200, 1, 1, 2047, "uncommon"),
        "miner_marid_master": (100, 1, 1, 4095, "rare"),
        "miner_ancient_eldritch": (15, 1, 9, 8191, "epic"),
    }
    for name, (ticks, rolls, multiplier, durability, rarity) in MINERS.items():
        flat_item(name, "minecraft:echo_shard", block=occ_model("item", name), extra={
            "max_stack_size": 1, "rarity": rarity, "components": {
                "minecraft:max_damage": durability, "minecraft:damage": 0,
                f"{MOD}:miner": {"ticks": ticks, "rolls": rolls, "multiplier": multiplier}}})
    # Which miner digs what: Occultism's own tags, and its miner recipes as they are. A recipe whose
    # result tag is empty here (another mod's ore) finds nothing until that mod is installed.
    for f in sorted((occ_data / MOD / "tags" / "item" / "miners").glob("*.json")):
        values = [v for v in json.loads(f.read_text())["values"] if v in items and "debug" not in v]
        write_json(data / "tags" / "item" / "miners" / f.name, {"values": values})
    miner_recipes = 0
    for f in sorted((occ_data / MOD / "recipe" / "miners").rglob("*.json")):
        if "debug" in f.parts:
            continue
        j = json.loads(f.read_text())
        j.pop("neoforge:conditions", None)
        # This port has no iesnium ore (it comes from silver ore with the Third Eye), so the deep
        # miners Occultism has dig iesnium ore dig raw iesnium instead, as often.
        if j["result"].get("tag") == "c:ores/iesnium":
            j["result"] = {"type": f"{MOD}:weighted_item", "stack": {"id": f"{MOD}:raw_iesnium", "count": 1},
                           "weight": j["result"]["weight"]}
        write_json(data / "recipe" / "miners" / f.relative_to(occ_data / MOD / "recipe" / "miners"), j)
        miner_recipes += 1
    print(f"  miner recipes: {miner_recipes}")
    # The mineshaft: a lamp in, nine outputs (or the container under it). Occultism turns it through
    # four models as it works; a block here keeps the first.
    model_block("dimensional_mineshaft", "minecraft:barrel", "dimensional_mineshaft",
                {"hardness": 1.5, "explosion_resistance": 6.0, "sounds": "minecraft:stone", "requires_tool": False})
    write_json(assets / "blockstates" / "dimensional_mineshaft.json", {"variants": {
        f"facing={f},open={o}": {"model": f"{MOD}:block/dimensional_mineshaft"}
        for f in ["down", "up", "north", "south", "west", "east"] for o in ["false", "true"]}})
    write_json(data / "horizon" / "menu" / "dimensional_mineshaft.json", {"slots": 10, "data_slots": 3, "results": list(range(1, 10))})
    lang[f"container.{MOD}.dimensional_mineshaft"] = lang_src[f"block.{MOD}.dimensional_mineshaft"]
    lang[f"gui.{MOD}.mineshaft.no_miner"] = "Put a miner spirit's lamp in"
    lang[f"gui.{MOD}.mineshaft.mining"] = "Mining"
    lang[f"gui.{MOD}.mineshaft.full"] = "Output full"

    # --- Stable wormhole: a gate to a store anywhere, its terminal and hopper face. Linked by using it
    # on an Actuator while sneaking (the native mod writes `occultism:link` on it); Occultism's facing models,
    # facing the clicked face.
    wormhole_states = json.loads((gen_assets / "blockstates" / "stable_wormhole.json").read_text())["variants"]
    model_block("stable_wormhole", "minecraft:barrel", "stable_wormhole",
                {"hardness": 1.5, "explosion_resistance": 6.0, "sounds": "minecraft:stone", "requires_tool": False})
    occ_model("block", "stable_wormhole")
    variants = {}
    for key, v in wormhole_states.items():
        facing = dict(p.split("=") for p in key.split(","))
        if facing.get("linked") == "true":
            for o in ["false", "true"]:
                variants[f"facing={facing['facing']},open={o}"] = {**v, "model": f"{MOD}:block/stable_wormhole"}
    write_json(assets / "blockstates" / "stable_wormhole.json", {"variants": variants})
    # --- Storage Accessor: Occultism's storage remote, a terminal in the hand.
    flat_item("storage_remote", "minecraft:paper", block=occ_model("item", "storage_remote_linked"), extra={"max_stack_size": 1})
    lang[f"storage.{MOD}.linked"] = "Linked to %s."
    lang[f"storage.{MOD}.unlinked"] = "Sneak-use it on a Dimensional Storage Actuator first to link it."
    lang[f"storage.{MOD}.gate_dormant"] = "%s is dormant: its Actuator isn't placed."
    lang[f"storage.{MOD}.linked_to"] = "Linked to %s"

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
        return tags.matches_any(ident[1:]) if ident.startswith("#") else tags.known(ident)

    # --- Otherworld wood, without Occultism's hidden trees: an oak log a player with the Third Eye
    # chops is now and then an otherworld log (the native mod, at iesnium's rates). Its planks, and
    # its ashes from spirit fire: with burnt otherstone or otherrock, the base of chalk.
    items.update({f"{MOD}:otherworld_log", f"{MOD}:otherplanks"})
    write_json(data / "horizon" / "block" / "otherworld_log.json", {"template": "minecraft:oak_log", "item": True})
    for tex in ("otherworld_log", "otherworld_log_top"):
        copy_texture(occ_assets / "textures" / "block" / f"{tex}.png", assets / "textures" / "block" / f"{tex}.png")
    write_json(assets / "models" / "block" / "otherworld_log.json", {"parent": "minecraft:block/cube_column", "textures": {
        "end": f"{MOD}:block/otherworld_log_top", "side": f"{MOD}:block/otherworld_log"}})
    copy(occ_assets / "blockstates" / "otherworld_log.json", assets / "blockstates" / "otherworld_log.json")
    write_json(assets / "items" / "otherworld_log.json", {"model": {"type": "minecraft:model", "model": f"{MOD}:block/otherworld_log"}})
    write_json(data / "loot_table" / "blocks" / "otherworld_log.json", loot_self(f"{MOD}:otherworld_log"))
    lang_of(f"block.{MOD}.otherworld_log")
    tab.append(f"{MOD}:otherworld_log")
    write_json(data / "horizon" / "block" / "otherplanks.json", {"template": "minecraft:oak_planks", "item": True})
    copy_texture(occ_assets / "textures" / "block" / "otherplanks.png", assets / "textures" / "block" / "otherplanks.png")
    write_json(assets / "models" / "block" / "otherplanks.json",
               {"parent": "minecraft:block/cube_all", "textures": {"all": f"{MOD}:block/otherplanks"}})
    write_json(assets / "blockstates" / "otherplanks.json", {"variants": {"": {"model": f"{MOD}:block/otherplanks"}}})
    write_json(assets / "items" / "otherplanks.json", {"model": {"type": "minecraft:model", "model": f"{MOD}:block/otherplanks"}})
    write_json(data / "loot_table" / "blocks" / "otherplanks.json", loot_self(f"{MOD}:otherplanks"))
    lang_of(f"block.{MOD}.otherplanks")
    tab.append(f"{MOD}:otherplanks")
    write_json(out / "data" / "minecraft" / "tags" / "block" / "mineable" / "axe.json",
               {"values": [f"{MOD}:otherworld_log", f"{MOD}:otherplanks"]})
    flat_item("otherworld_ashes", "minecraft:gunpowder")
    for stone in ("otherstone", "otherrock"):
        cube_block(f"polished_{stone}", "minecraft:stone", props={"hardness": 1.5, "explosion_resistance": 30.0, "requires_tool": True})
        flat_item(f"burnt_{stone}", "minecraft:brick")
    # Impure chalk: Occultism's white (burnt otherstone or otherrock and otherworld ashes), and each
    # colour whose own recipe needs only what this port has. Spirit fire purifies it into a chalk;
    # Occultism's lasts 256 glyphs, this port's is used up per glyph, so one gives a stack of 64.
    impure = []
    for colour in CHALKS:
        recipe = json.loads((occ_data / MOD / "recipe" / "crafting" / f"chalk_{colour}_impure.json").read_text())
        needs = [i for i in list(ingredient_ids(list((recipe.get("key") or {}).values()))) + list(ingredient_ids(recipe.get("ingredients")))
                 if i != f"{MOD}:chalk_white_impure"]
        if colour == "white" or all(resolves(i) for i in needs):
            impure.append(colour)
            flat_item(f"chalk_{colour}_impure", "minecraft:paper", block=occ_model("item", f"chalk_{colour}_impure"))
            item_def = json.loads((gen_assets / "items" / f"chalk_{colour}_impure.json").read_text())
            write_json(assets / "items" / f"chalk_{colour}_impure.json", item_def)

    # --- Storage's parts, as Occultism crafts them: otherstone slabs, and the pedestal and frame made
    # from them; the base stabilizer the pedestal turns into (this port's plate 0, which no store
    # takes); the controller base and the dimensional matrix, which make the Actuator as they make
    # Occultism's controller; the otherworldly tablet and the inert Accessor. Their recipes and
    # rituals are Occultism's (copied below and in RITUALS). The parts are items here, not blocks:
    # they are ingredients, and the pedestal's decoration waits for Occultism's other stone blocks.
    slab = "otherstone_slab"
    write_json(data / "horizon" / "block" / f"{slab}.json", {
        "template": "minecraft:stone_slab", "item": True,
        "properties": {"hardness": 1.5, "explosion_resistance": 30.0, "requires_tool": True}})
    for model in (slab, f"{slab}_top"):
        occ_model("block", model)
    write_json(assets / "blockstates" / f"{slab}.json", json.loads((gen_assets / "blockstates" / f"{slab}.json").read_text()))
    write_json(assets / "items" / f"{slab}.json", {"model": {"type": "minecraft:model", "model": f"{MOD}:block/{slab}"}})
    # Vanilla's slab table, in 26.3's format: two from a double slab.
    write_json(data / "loot_table" / "blocks" / f"{slab}.json", {
        "type": "minecraft:block", "random_sequence": f"{MOD}:blocks/{slab}",
        "pools": [{"rolls": 1, "entries": [{"type": "minecraft:item", "name": f"{MOD}:{slab}", "modifier": [
            {"type": "minecraft:set_count", "count": 2, "condition": {
                "type": "minecraft:match_block", "blocks": f"{MOD}:{slab}", "state": {"type": "double"}}},
            {"type": "minecraft:explosion_decay"}]}]}]})
    items.add(f"{MOD}:{slab}")
    pickaxe.append(f"{MOD}:{slab}")
    tab.append(f"{MOD}:{slab}")
    lang_of(f"block.{MOD}.{slab}")

    def occ_item(name, model=None, lang_key=None):
        """An item drawn with Occultism's own item model (or block model `model`), named as there."""
        ident = f"{MOD}:{name}"
        items.add(ident)
        write_json(data / "horizon" / "item" / f"{name}.json", {"template": "minecraft:paper"})
        model_id = occ_model("block", model, dest=name) if model else occ_model("item", name)
        write_json(assets / "items" / f"{name}.json", {"model": {"type": "minecraft:model", "model": model_id}})
        key = lang_key or next(k for k in (f"item.{MOD}.{name}", f"block.{MOD}.{name}") if k in lang_src)
        lang[f"item.{MOD}.{name}"] = lang_src[key]
        tab.append(ident)

    for name in ("otherstone_frame", "dimensional_matrix", "otherworldly_tablet", "storage_remote_inert"):
        occ_item(name)
    for name in ("otherstone_pedestal", "storage_controller_base"):
        occ_item(name, model=name)
    occ_item("storage_plate_tier0", model="storage_stabilizer_tier0", lang_key=f"block.{MOD}.storage_stabilizer_tier0")
    # What a repair ritual shows as its result (the item itself comes back); kept out of the tab.
    flat_item("repair_icon", "minecraft:paper")
    tab.remove(f"{MOD}:repair_icon")

    # Occultism's own crafting, smelting and blasting recipes for what exists now (silver and
    # iesnium storage, nuggets, smelting, the golden bowl, crystal, lenses, goggles...).

    ours = {p.stem for p in (data / "recipe").glob("*.json")}
    copied = []
    for kind in ("crafting", "smelting", "blasting"):
        for f in sorted((occ_data / MOD / "recipe" / kind).glob("*.json")):
            j = alias(json.loads(f.read_text()))
            j.pop("neoforge:conditions", None)
            result = j.get("result", {})
            rid = result.get("id") if isinstance(result, dict) else None
            if j.get("type") not in ("minecraft:crafting_shaped", "minecraft:crafting_shapeless",
                                     "minecraft:smelting", "minecraft:blasting"):
                continue
            if rid not in items or rid.split(":")[1] in ours or rid.startswith(f"{MOD}:book_of_binding_bound"):
                continue
            ins = list(ingredient_ids(list((j.get("key") or {}).values()))) + list(ingredient_ids(j.get("ingredients"))) + list(ingredient_ids(j.get("ingredient")))
            if ins and all(resolves(i) for i in ins):
                write_json(data / "recipe" / f"{kind}_{f.name}", j)
                copied.append(f"{kind}/{f.stem}")

    # --- Spirit fire's recipes (read by the native mod), for what this port has; the item tags they
    # and the crafting recipes name, with the entries this port has.
    fire_recipes = []
    for f in sorted((occ_data / MOD / "recipe" / "spirit_fire").glob("*.json")):
        j = json.loads(f.read_text())
        j.pop("neoforge:conditions", None)
        if resolves(j["ingredient"]) and tags.known(j["result"]["id"]):
            write_json(data / "recipe" / "spirit_fire" / f.name, j)
            fire_recipes.append(f.stem)
    for name in ("datura", "start_spiritfire", "books/books_for_empty", "otherworld_logs", "dusts/chalk_base",
                 "tools/chalk", "miners"):
        values = [v for v in tags.files[f"{MOD}:{name}"] if resolves(v)]
        write_json(data / "tags" / "item" / f"{name}.json", {"values": values})

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
    for ns in (MOD, "c"):
        for f in sorted((occ_data / ns / "tags" / "block").rglob("*.json")):
            name = f"{ns}:" + f.relative_to(occ_data / ns / "tags" / "block").with_suffix("").as_posix()
            block_tags[name] = [e["id"] if isinstance(e, dict) else e for e in json.loads(f.read_text())["values"]]
    block_tags["minecraft:candles"] = CANDLES
    block_tags["minecraft:lightning_rods"] = LIGHTNING_RODS
    block_tags["c:glass_panes"] = VANILLA_C_TAGS["glass_panes"]  # NeoForge's, which Occultism doesn't ship
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
        j = alias(json.loads((occ_data / MOD / "recipe" / "ritual" / f"{name}.json").read_text()))
        j.pop("neoforge:conditions", None)
        if j["pentacle_id"].split(":")[1] not in PENTACLES:
            die(f"ritual {name}: pentacle {j['pentacle_id']} isn't shipped")
        ritual = {"type": f"{MOD}:ritual", "activation_item": j["activation_item"], "duration": j["duration"],
                  "ingredients": j["ingredients"], "pentacle_id": j["pentacle_id"],
                  "result": {"id": result or j["result"]["id"], "count": j["result"].get("count", 1)}}
        if j.get("ritual_type") == f"{MOD}:repair":
            ritual["ritual_type"] = j["ritual_type"]
        if "entity_to_sacrifice" in j:
            # The kinds a player must kill near the bowl, resolved here: the native mod can't read
            # entity tags. Occultism's own mobs aren't in this port.
            sacrifice = j["entity_to_sacrifice"]
            kinds = entity_tag(occ_data, sacrifice["tag"])
            if not kinds:
                die(f"ritual {name}: nothing this port has is in #{sacrifice['tag']}")
            ritual["sacrifice"] = {"kinds": kinds, "display_name": sacrifice["display_name"]}
            lang_of(sacrifice["display_name"])
            lang_of(f"ritual.{MOD}.sacrifice")
        for ident in [ritual["activation_item"], ritual["result"]["id"]] + list(ingredient_ids(ritual["ingredients"])):
            if not resolves(ident):
                die(f"ritual {name}: {ident} isn't in this port")
        write_json(data / "recipe" / "ritual" / f"{name}.json", ritual)
    # The port's own: recall lost Storage Cores. In the circle Occultism crafts its storage in, with
    # an ender pearl to call across distance; the native mod does it (`ritual_type`), the result is
    # what a recipe viewer shows.
    recall = {"type": f"{MOD}:ritual", "ritual_type": f"{MOD}:recall_storage",
              "activation_item": f"{MOD}:book_of_binding_bound_foliot", "duration": 30,
              "ingredients": ["minecraft:ender_pearl", f"{MOD}:spirit_attuned_gem", "#c:ingots/silver", "minecraft:amethyst_shard"],
              "pentacle_id": f"{MOD}:craft_foliot", "result": {"id": f"{MOD}:storage_actuator", "count": 1}}
    for ident in [recall["activation_item"], recall["result"]["id"]] + list(ingredient_ids(recall["ingredients"])):
        if not resolves(ident):
            die(f"ritual recall_storage_core: {ident} isn't in this port")
    write_json(data / "recipe" / "ritual" / "recall_storage_core.json", recall)
    lang[f"storage.{MOD}.recalled"] = "%s answers the call: its Actuator returns."
    lang[f"storage.{MOD}.recall_none"] = "No lost spirit answers: every store of yours has its Actuator."
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
    print(f"  spirit fire recipes: {len(fire_recipes)}: " + ", ".join(fire_recipes))
    print(f"  pentacles: {len(pentacles)}, rituals: {len(RITUALS)}")
    print("  kept: " + ", ".join(kept))


if __name__ == "__main__":
    main()
