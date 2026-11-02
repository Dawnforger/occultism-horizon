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
import sys
from pathlib import Path

MOD = "occultism"

# Occultism dusts with textures, shipped as items.
DUSTS = [
    "iron", "gold", "copper", "netherite", "netherite_scrap", "obsidian",
    "emerald", "lapis", "amethyst", "echo",
]

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
version = "0.1.2"
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
    write_json(data / "loot_table" / "blocks" / "demonic_altar.json", {
        "type": "minecraft:block",
        "pools": [{"rolls": 1, "entries": [{"type": "minecraft:item", "name": f"{MOD}:demonic_altar"}],
                   "conditions": [{"condition": "minecraft:survives_explosion"}]}],
    })
    write_json(out / "data" / "minecraft" / "tags" / "block" / "mineable" / "pickaxe.json",
               {"values": [f"{MOD}:demonic_altar"]})

    write_json(data / "horizon" / "creative_tab" / "occultism.json", {
        "title": f"itemGroup.{MOD}.occultism",
        "icon": f"{MOD}:demonic_altar",
        "items": [f"{MOD}:demonic_altar"] + [f"{MOD}:crusher_core_{t[0]}" for t in TIERS] + [f"{MOD}:{d}_dust" for d in DUSTS],
    })
    lang[f"itemGroup.{MOD}.occultism"] = "Occultism"

    # --- Tags: Occultism's c: tags as shipped, plus the vanilla entries (merged by the game).
    for f in sorted((occ_data / "c" / "tags" / "item").rglob("*.json")):
        rel = f.relative_to(occ_data / "c" / "tags" / "item")
        copy(f, out / "data" / "c" / "tags" / "item" / rel)
    for name, values in VANILLA_C_TAGS.items():
        path = out / "data" / "c" / "tags" / "item" / f"{name}.json"
        existing = json.loads(path.read_text())["values"] if path.exists() else []
        write_json(path, {"values": values + existing})
    tags = Tags(occ_data, items)

    # --- Crushing recipes that work with vanilla plus the dusts above.
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

    # --- Crafting: the altar, and the cores (a stand-in for Occultism's binding rituals, with
    # each ritual's own ingredients where vanilla has them).
    write_json(data / "recipe" / "demonic_altar.json", {
        "type": "minecraft:crafting_shaped", "category": "misc",
        "pattern": ["GAG", "OCO", "OOO"],
        "key": {"G": "minecraft:gold_ingot", "A": "minecraft:amethyst_shard",
                "O": "minecraft:obsidian", "C": "minecraft:crying_obsidian"},
        "result": {"id": f"{MOD}:demonic_altar", "count": 1},
    })
    core_recipes = {
        "foliot": ["minecraft:iron_ingot", "minecraft:gold_ingot", "minecraft:copper_ingot", "minecraft:amethyst_shard", "minecraft:echo_shard"],
        "djinni": [f"{MOD}:crusher_core_foliot", f"{MOD}:iron_dust", f"{MOD}:gold_dust", f"{MOD}:copper_dust"],
        "afrit": [f"{MOD}:crusher_core_djinni", f"{MOD}:emerald_dust", f"{MOD}:lapis_dust", f"{MOD}:amethyst_dust", f"{MOD}:obsidian_dust"],
        "marid": [f"{MOD}:crusher_core_afrit", "minecraft:diamond_block", "minecraft:emerald_block", "minecraft:netherite_block", "minecraft:ghast_tear"],
    }
    for name, ingredients in core_recipes.items():
        write_json(data / "recipe" / f"crusher_core_{name}.json", {
            "type": "minecraft:crafting_shapeless", "category": "misc",
            "ingredients": ingredients,
            "result": {"id": f"{MOD}:crusher_core_{name}", "count": 1},
        })
        for i in ingredients:
            if not tags.known(i):
                die(f"core recipe {name} uses unknown item {i}")

    write_json(assets / "lang" / "en_us.json", dict(sorted(lang.items())))

    print(f"content mod written to {out}")
    print(f"  items: {len(items)} ({len(DUSTS)} dusts, {len(TIERS)} cores) + the altar")
    print(f"  crushing recipes: {len(kept)} kept, {len(dropped)} need other mods' items")
    print("  kept: " + ", ".join(kept))


if __name__ == "__main__":
    main()
