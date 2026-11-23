#!/usr/bin/env python3
"""Builds the Occultism content mod (content/) from an Occultism checkout.

    python3 tools/build_content.py /path/to/occultism [--out mod]

What it does:
- Copies Occultism's textures, item models and lang strings for what this proof of concept uses
  (MIT; KliKli's permission covers the port: https://github.com/klikli-dev).
- Copies Occultism's `occultism:crushing` recipes whose input and output exist in vanilla plus the
  dusts shipped here. Their `neoforge:conditions` (`tag_empty`, which Horizon doesn't know and
  would drop the file for) are removed: the dropped recipes are exactly the ones the conditions
  would have disabled, so the set is the same as Occultism's in a pack with no other mods.
- Copies Occultism's `c:` convention tags and adds the vanilla entries NeoForge's own tags supply.
- Writes the port's blocks, items, creatures, creative tab, loot tables and recipes.

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
# Occultism's other crushed and ground items, shipped as items like the dusts: what its coloured
# impure chalks are made of, from crushing and from rituals.
MATERIALS = [
    "crushed_ice", "crushed_packed_ice", "crushed_blue_ice", "crushed_calcite", "crushed_end_stone",
    "crushed_blackstone", "witherite_dust", "research_fragment_dust", "dragonyst_dust", "gray_paste",
    "nature_paste", "flaming_paste",
]
# Occultism's possessed creatures this port has (spirits.rs summons each as its vanilla creature,
# with Occultism's attributes), and the items they drop.
POSSESSED = ["possessed_bee", "mercy_goat", "possessed_zombified_piglin"] + [f"possessed_{m}" for m in (
    "blaze", "elder_guardian", "enderman", "endermite", "ghast", "guardian", "hoglin", "phantom", "shulker", "skeleton",
    "warden", "weak_shulker", "witch")]
# The wild rituals' creatures (`SummonWildRitual`), summoned as the possessed ones are, with the
# helpers some bring: the wild hunt's skeletons, the illager horde.
WILD = [f"wild_horde_{m}" for m in ("creeper", "drowned", "husk", "parched", "silverfish")] + [
    "wild_hunt_wither_skeleton", "wild_hunt_skeleton", "possessed_evoker",
    "possessed_weak_breeze", "possessed_breeze", "possessed_strong_breeze"]
# The trial breezes' followers, which drop nothing (Occultism has no loot table for them).
WILD_FOLLOWERS = [f"wild_{m}" for m in ("zombie", "skeleton", "silverfish", "spider", "stray", "cave_spider", "husk", "bogged", "slime")]
POSSESSED_DROPS = ["cursed_honey", "cruelty_essence", "demonic_meat", "tallow"]
# What Occultism's items are besides an icon (`OccultismItems`, `OccultismFoods`): foods with their
# effects, rarities, fire resistance, the pastes' 64 uses. A food's saturation is its nutrition times
# its modifier, twice, as vanilla's FoodProperties builder computes it.
FIRE_PROOF = {"minecraft:damage_resistant": {"types": "#minecraft:is_fire"}}
def food(nutrition, modifier, *effects):
    return {"minecraft:food": {"nutrition": nutrition, "saturation": round(nutrition * modifier * 2, 4)},
            "minecraft:consumable": {"on_consume_effects": [
                {"type": "minecraft:apply_effects", "effects": [e], "probability": p} for e, p in effects]}}
ITEM_TRAITS = {
    "cursed_honey": ({}, food(2, 1.0, ({'id': 'minecraft:regeneration', 'duration': 100, 'amplifier': 1, 'ambient': False, 'show_particles': True}, 1.0))),
    "demonic_meat": ({"rarity": "uncommon"}, {**food(11, 0.1, ({'id': 'minecraft:fire_resistance', 'duration': 3600, 'amplifier': 0, 'ambient': False, 'show_particles': True}, 1.0)), **FIRE_PROOF}),
    "cruelty_essence": ({"rarity": "rare"}, FIRE_PROOF),
    "gray_paste": ({"max_stack_size": 1}, {"minecraft:max_damage": 64, **food(2, 0.2, ({'id': 'minecraft:resistance', 'duration': 3000, 'amplifier': 3, 'ambient': True, 'show_particles': True}, 1.0), ({'id': 'minecraft:slowness', 'duration': 600, 'amplifier': 1, 'ambient': True, 'show_particles': True}, 1.0))}),
    "nature_paste": ({"max_stack_size": 1}, {"minecraft:max_damage": 64, **food(33, 0.3, ({'id': 'minecraft:haste', 'duration': 3600, 'amplifier': 0, 'ambient': True, 'show_particles': True}, 1.0),
                     ({'id': 'minecraft:oozing', 'duration': 3600, 'amplifier': 0, 'ambient': True, 'show_particles': True}, 0.333), ({'id': 'minecraft:nausea', 'duration': 3600, 'amplifier': 0, 'ambient': True, 'show_particles': True}, 0.333), ({'id': 'minecraft:poison', 'duration': 3600, 'amplifier': 0, 'ambient': True, 'show_particles': True}, 0.333))}),
}
# Flaming paste lights fires as flint and steel does (`FlamingPasteItem`, `DEFAULT_FLINT_ACTIONS`):
# the native mod gives it the rest (flaming_paste.rs).
ITEM_TRAITS["flaming_paste"] = (
    {"template": "minecraft:flint_and_steel", "max_stack_size": 1, "rarity": "uncommon"},
    {"minecraft:max_damage": 256, **FIRE_PROOF, **food(15, 1.0,
        ({'id': 'minecraft:fire_resistance', 'duration': 12000, 'amplifier': 0, 'ambient': True, 'show_particles': True}, 1.0),
        ({'id': 'minecraft:strength', 'duration': 12000, 'amplifier': 1, 'ambient': True, 'show_particles': True}, 1.0))})
# Occultism's pastes (`DamageInCraftingItem`): a craft takes one of their uses and leaves them in the grid.
PASTES = [f"{MOD}:gray_paste", f"{MOD}:nature_paste", f"{MOD}:flaming_paste"]

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
             "contact_wild_spirit", "contact_eldritch_spirit", "summon_unbound_afrit", "summon_unbound_marid", "craft_marid",
             "possess_djinni", "possess_marid", "possess_unbound_afrit", "possess_foliot", "possess_afrit", "resurrect_spirit"]
# The jobs whose spirit changes the time or weather and goes (`ChangeTimeJob`, `ChangeWeatherJob`).
SKY_JOBS = [f"{MOD}:{j}" for j in ("day_time", "night_time", "clear_weather", "rain_weather", "thunder_weather")]
# The traders' jobs (`TraderJob`), and their trades (`occultism:spirit_trade`).
TRADER_JOBS = [f"{MOD}:{j}" for j in ("trader_otherstone", "trader_otherrock", "gambler")]
TRADES = ["stone_to_otherstone", "stone_to_otherrock"] + [f"gambler_{g}" for g in (
    "amethyst", "diamond", "emerald", "gold", "iesnium", "iron", "lazuli", "prismarine", "quartz", "silver", "spirit")]
RITUALS = {
    # The crusher spirits come to work as Occultism's do, with no book of calling.
    "summon_foliot_crusher": None,
    "summon_djinni_crusher": None,
    "summon_afrit_crusher": None,
    "summon_marid_crusher": None,
    # The ritual satchels, which build a circle from the book's preview (pentacle_preview.rs).
    "craft_ritual_satchel_t1": None,
    "craft_ritual_satchel_t2": None,
    # The smelters and crystallizers, as the crushers.
    "summon_foliot_smelter": None,
    "summon_djinni_smelter": None,
    "summon_afrit_smelter": None,
    "summon_marid_smelter": None,
    "summon_foliot_crystallizer": None,
    "summon_djinni_crystallizer": None,
    "summon_afrit_crystallizer": None,
    "summon_marid_crystallizer": None,
    "craft_infused_lenses": None,  # Occultism's own result
    # Weather and time magic: the spirit casts at the bowl, changes the sky, and is gone (no book).
    "summon_djinni_day_time": None,
    "summon_djinni_night_time": None,
    "summon_djinni_clear_weather": None,
    "summon_afrit_rain_weather": None,
    "summon_afrit_thunder_weather": None,
    # Traders: the spirit trades what it picks up for a while, then goes (no book). Occultism's
    # sapling trader isn't here: it trades the hidden otherworld sapling for the plantable one, and
    # this port has only the plantable one.
    "summon_foliot_otherstone_trader": None,
    "summon_foliot_otherrock_trader": None,
    "summon_djinni_gambler": None,
    # The chalks' ritual-made materials.
    "craft_witherite_dust": None,
    "craft_research_fragment_dust": None,
    "craft_dragonyst_dust": None,
    "craft_gray_paste": None,
    "craft_nature_paste": None,
    "craft_flaming_paste": None,
    # Equipment: silver made infused keeps its enchantments and wear (`occultism:upgrade`); the
    # iesnium knife is bound to the book's spirit; horse and nautilus armour forged on leather.
    **{f"craft_infused_{t}": None for t in ("pickaxe", "axe", "shovel", "hoe", "sword", "spear", "helmet", "chestplate", "leggings", "boots")},
    "craft_iesnium_butcher_knife": None,
    "craft_iesnium_sacrificial_bowl": None,
    "craft_dark_iesnium_sacrificial_bowl": None,
    **{f"misc_{m}_{a}_armor": None for m in ("silver", "infused") for a in ("horse", "nautilus")},
    "craft_miner_foliot_unspecialized": None,
    "craft_miner_djinni_ores": None,
    # What the unbound spirits' essences make.
    "craft_miner_afrit_deeps": None,
    "craft_miner_marid_master": None,
    # Storage, as Occultism makes it (its stabilizers are this port's plates: ALIASES).
    "craft_storage_controller_base": None,
    "craft_dimensional_matrix": None,
    "craft_stabilizer_tier1": None,
    "craft_stabilizer_tier2": None,
    "craft_stabilizer_tier3": None,
    "craft_stabilizer_tier4": None,
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
    # Soul gems: a creature captured whole, released where the gem is used (the native mod).
    "craft_fragile_soul_gem": None,
    "craft_soul_gem": None,
    # Unbound spirits: the ritual summons one to fight; it drops its essence (`occultism:summon`).
    "summon_unbound_afrit": None,
    "summon_unbound_marid": None,
    # Working spirits: the foliot comes to work, and the caster gets its book of calling (the
    # native mod's `occultism:summon_spirit_with_job`).
    "summon_foliot_cleaner": None,
    "summon_foliot_transport_items": None,
    "summon_foliot_lumberjack": None,
    "summon_foliot_farmer": None,
    # Possessions: the creature is the result, and its drops are Occultism's.
    "possess_bee": None,
    "possess_goat": None,
    "possess_zombified_piglin": None,
    "possess_blaze": None,
    "possess_elder_guardian": None,
    "possess_enderman": None,
    "possess_endermite": None,
    "possess_ghast": None,
    "possess_guardian": None,
    "possess_hoglin": None,
    "possess_phantom": None,
    "possess_shulker": None,
    "possess_skeleton": None,
    "possess_warden": None,
    "possess_weak_shulker": None,
    "possess_witch": None,
    # A random animal of a kind, possessed (`entity_tag_to_summon`).
    **{f"possess_random_animal_{k}": None for k in ("common", "rideable", "small", "special", "water")},
    "possess_villager": None,
    # The wild rituals: several creatures, each named, none bound to the caster (`occultism:summon_wild`).
    "wild_creeper": None,
    "wild_drowned": None,
    "wild_husk": None,
    "wild_parched": None,
    "wild_silverfish": None,
    "wild_hunt": None,
    "wild_horde_illager": None,
    "wild_weak_breeze": None,
    "wild_breeze": None,
    "wild_strong_breeze": None,
    **{f"wild_random_animal_{k}": None for k in ("common", "rideable", "small", "special", "water")},
    "wild_villager": None,
    # Resurrections: the vanilla creature a sacrifice comes back as.
    "resurrect_allay": None,
    "resurrect_skeleton_horse": None,
    "resurrect_zombie_horse": None,
    "resurrect_camel_husk": None,
    "resurrect_zombie_nautilus": None,
}
# What this port's rituals take for Occultism ingredients it doesn't have.
RITUAL_STAND_INS = {}
# Occultism's unbound spirits, as mob types (data/<id>/horizon/entity_type): each copies a zombie,
# the template that fights as Occultism's Monster does, with Occultism's attributes
# (`createAttributes`). The native mod gives them the rest: fire immunity and the like, their helpers,
# their models.
SPIRIT_MOBS = {
    "afrit_unbound": {"minecraft:max_health": 100.0, "minecraft:attack_damage": 8.0, "minecraft:movement_speed": 0.4,
                      "minecraft:armor": 8.0, "minecraft:armor_toughness": 50.0, "minecraft:follow_range": 50.0},
    "marid_unbound": {"minecraft:max_health": 300.0, "minecraft:attack_damage": 14.0, "minecraft:movement_speed": 0.41,
                      "minecraft:armor": 16.0, "minecraft:armor_toughness": 50.0, "minecraft:follow_range": 50.0,
                      "minecraft:knockback_resistance": 1.0},
}
# This port's storage is one block with its upgrades as item plates (docs/storage-design.md):
# Occultism's stabilizers are its plates and Occultism's storage controller its Actuator, in the
# recipes and rituals copied from Occultism.
ALIASES = {f"{MOD}:storage_stabilizer_tier{t}": f"{MOD}:storage_plate_tier{t}" for t in range(6)}
ALIASES[f"{MOD}:storage_controller"] = f"{MOD}:storage_actuator"
# Occultism's natural otherworld tree is hidden (an oak to those without the Third Eye); this port's
# grows in the open, from the one sapling.
ALIASES[f"{MOD}:otherworld_sapling_natural"] = f"{MOD}:otherworld_sapling"
# Occultism's hidden ("natural") glass and flower look like vanilla's to the unaided eye; this port
# has the plain ones.
ALIASES[f"{MOD}:otherglass_natural"] = f"{MOD}:otherglass"
ALIASES[f"{MOD}:otherflower_natural"] = f"{MOD}:otherflower"


def alias(recipe):
    """`recipe` (an Occultism recipe's JSON) naming this port's items for Occultism's."""
    text = json.dumps(recipe)
    for old, new in ALIASES.items():
        text = text.replace(f'"{old}"', f'"{new}"')
    return json.loads(text)
# Occultism's large candles (`LargeCandleBlock`), plain and in each colour: candles to its pentacles.
LARGE_CANDLES = [f"{MOD}:large_candle"] + [f"{MOD}:large_candle_{c}" for c in CHALKS]
CANDLES = ["minecraft:candle"] + [f"minecraft:{c}_candle" for c in CHALKS] + LARGE_CANDLES
# Occultism's otherrock building blocks, each with the vanilla block it behaves as.
OTHERROCK_BLOCKS = {
    "otherrock_bricks": "minecraft:stone_bricks", "chiseled_otherrock_bricks": "minecraft:chiseled_stone_bricks",
    "cracked_otherrock_bricks": "minecraft:cracked_stone_bricks",
    "otherrock_slab": "minecraft:stone_slab", "otherrock_stairs": "minecraft:stone_stairs", "otherrock_wall": "minecraft:cobblestone_wall",
    "otherrock_button": "minecraft:stone_button", "otherrock_pressure_plate": "minecraft:stone_pressure_plate",
    "otherrock_bricks_slab": "minecraft:stone_brick_slab", "otherrock_bricks_stairs": "minecraft:stone_brick_stairs",
    "otherrock_bricks_wall": "minecraft:stone_brick_wall",
    "polished_otherrock_slab": "minecraft:polished_andesite_slab", "polished_otherrock_stairs": "minecraft:polished_andesite_stairs",
    "polished_otherrock_wall": "minecraft:andesite_wall",
}
# Vanilla's, as 26.3 has it: the contact pentacles' rods.
LIGHTNING_RODS = [f"minecraft:{w}{o}lightning_rod" for w in ("", "waxed_")
                  for o in ("", "exposed_", "weathered_", "oxidized_")]
# Binding books (activation items), with the dye Occultism's own book recipe uses for the rank.
# Occultism's GeckoLib spirits this port draws (spirit_model.rs): the unbound afrit and marid, the
# foliot in each job's look, and the djinni, afrit and marid at work.
SPIRIT_LOOKS = ("afrit", "marid", "foliot_janitor", "foliot_transporter", "foliot_lumberjack", "foliot_farmer",
                "foliot_crusher", "foliot_worker", "djinni_worker", "afrit_worker", "marid_worker")
# The working spirits besides the foliot, on the foliot's wolf (`createAttributes` of each, but
# for the afrit's and marid's attack speed: a wolf has no such attribute).
WORKERS = {
    "djinni": {"minecraft:max_health": 100.0, "minecraft:attack_damage": 3.0, "minecraft:movement_speed": 0.30000001192092896,
               "minecraft:armor": 4.0, "minecraft:armor_toughness": 5.0, "minecraft:follow_range": 50.0},
    "afrit": {"minecraft:max_health": 200.0, "minecraft:attack_damage": 8.0,
              "minecraft:movement_speed": 0.40000001192092896, "minecraft:armor": 8.0, "minecraft:armor_toughness": 50.0},
    "marid": {"minecraft:max_health": 300.0, "minecraft:attack_damage": 16.0,
              "minecraft:movement_speed": 0.40000001192092896, "minecraft:armor": 16.0, "minecraft:armor_toughness": 100.0},
}
BOOK_DYES = {"foliot": "blue", "djinni": "purple", "afrit": "yellow", "marid": "green"}

# NeoForge's convention tags, for vanilla items (NeoForge `Tags.Items`, 26.x names).
VANILLA_C_TAGS = {
    # NeoForge's, for the soul gems' rituals and the janitor's summoning.
    "chests": ["minecraft:chest", "minecraft:trapped_chest", "minecraft:ender_chest"],
    # NeoForge's, for the farmer's harvest.
    "crops": ["minecraft:beetroot", "minecraft:cactus", "minecraft:carrot", "minecraft:cocoa_beans", "minecraft:melon_slice",
              "minecraft:nether_wart", "minecraft:potato", "minecraft:pumpkin", "minecraft:sugar_cane", "minecraft:wheat"],
    "seeds": ["minecraft:beetroot_seeds", "minecraft:melon_seeds", "minecraft:pumpkin_seeds", "minecraft:torchflower_seeds",
              "minecraft:wheat_seeds", "minecraft:pitcher_pod"],
    # NeoForge's, for Occultism's brush.
    "strings": ["minecraft:string"],
    "eggs": ["minecraft:egg", "minecraft:blue_egg", "minecraft:brown_egg"],
    "glass_blocks": ["minecraft:glass", "minecraft:tinted_glass"] + [f"minecraft:{c}_stained_glass" for c in CHALKS],
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
    # NeoForge's, for the smelters' and crystallizers' summonings.
    "tools/igniter": ["minecraft:flint_and_steel"],
    "buckets/lava": ["minecraft:lava_bucket"],
    "storage_blocks/coal": ["minecraft:coal_block"],
    "storage_blocks/wheat": ["minecraft:hay_block"],
    "rods/breeze": ["minecraft:breeze_rod"],
    "clumps/resin": ["minecraft:resin_clump"],
    "crops/cactus": ["minecraft:cactus"],
    "bricks": ["minecraft:brick", "minecraft:nether_brick"],
    "foods/pie": ["minecraft:pumpkin_pie"],
    # NeoForge's, for the weather and time rituals.
    # NeoForge's, for the gambler's trades; and its c:gems, which Occultism's own file only adds the
    # spirit-attuned gem to.
    "nuggets/iron": ["minecraft:iron_nugget"],
    "gems": ["#c:gems/diamond", "#c:gems/emerald", "#c:gems/amethyst", "#c:gems/lapis", "#c:gems/prismarine", "#c:gems/quartz"],
    "crops/beetroot": ["minecraft:beetroot"],
    "crops/carrot": ["minecraft:carrot"],
    "crops/potato": ["minecraft:potato"],
    "crops/wheat": ["minecraft:wheat"],
    "gunpowders": ["minecraft:gunpowder"],
    "bones": ["minecraft:bone"],
    "sands": ["minecraft:sand", "minecraft:red_sand"],
    # NeoForge's, for the possessions' rituals.
    "buckets/water": ["minecraft:water_bucket"],
    "storage_blocks/lapis": ["minecraft:lapis_block"],
    # NeoForge's, one per colour (the gray paste's ritual among others).
    **{f"dyes/{c}": [f"minecraft:{c}_dye"] for c in CHALKS},
    # What the unbound spirits' summonings take.
    "netherracks": ["minecraft:netherrack"],
    "gunpowders": ["minecraft:gunpowder"],
    "gems/prismarine": ["minecraft:prismarine_crystals"],
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
    """A non-interlaced PNG as (width, height, rows of RGBA bytes): 8-bit RGBA, RGB, gray or gray with
    alpha, or a palette of 1, 2, 4 or 8 bits (with its tRNS alpha)."""
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        die(f"not a PNG: {path}")
    pos, idat, head, palette, trns = 8, b"", None, b"", b""
    while pos + 8 <= len(data):
        length, kind = struct.unpack(">I4s", data[pos:pos + 8])
        if kind == b"IEND":
            break
        body = data[pos + 8:pos + 8 + length]
        if kind == b"IHDR":
            head = struct.unpack(">IIBBBBB", body)
        elif kind == b"PLTE":
            palette = body
        elif kind == b"tRNS":
            trns = body
        elif kind == b"IDAT":
            idat += body
        pos += 12 + length
    width, height, depth, colour, _, _, interlace = head
    channels = {0: 1, 2: 3, 3: 1, 4: 2, 6: 4}.get(colour)
    if interlace or channels is None or (colour != 3 and depth != 8) or depth not in (1, 2, 4, 8):
        die(f"{path}: only non-interlaced 8-bit or palette PNGs are read here")
    bits = depth * channels
    bpp, stride = max(1, bits // 8), (width * bits + 7) // 8
    raw, rows, prev = zlib.decompress(idat), [], bytearray(stride)
    for y in range(height):
        kind, line = raw[y * (stride + 1)], bytearray(raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)])
        for i in range(stride):
            a = line[i - bpp] if i >= bpp else 0
            b = prev[i]
            c = prev[i - bpp] if i >= bpp else 0
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
        prev = line
        if colour == 6:
            rows.append(line)
            continue
        rgba = bytearray()
        for x in range(width):
            if colour == 3:
                per = 8 // depth
                index = (line[x // per] >> (8 - depth * (x % per + 1))) & ((1 << depth) - 1)
                rgba += palette[index * 3:index * 3 + 3] + bytes([trns[index] if index < len(trns) else 255])
            elif colour == 2:
                rgba += line[x * 3:x * 3 + 3] + b"\xff"
            elif colour == 0:
                rgba += bytes([line[x]] * 3) + b"\xff"
            else:
                rgba += bytes([line[x * 2]] * 3) + bytes([line[x * 2 + 1]])
        rows.append(rgba)
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


def loot_26(node):
    """One of Occultism's loot tables (1.21's form) in 26.3's: one `condition` (several as `all_of`),
    a condition's kind in `type`, `modifier` for `functions` (each kind in `type`), and vanilla's named
    tool checks for shears and silk touch. 26.3 reads 1.21's keys as unknown and skips them."""
    if isinstance(node, list):
        return [loot_26(n) for n in node]
    if not isinstance(node, dict):
        return node
    out = {}
    for key, value in node.items():
        if key == "conditions":
            conditions = [loot_condition_26(c) for c in value]
            out["condition"] = conditions[0] if len(conditions) == 1 else {"type": "minecraft:all_of", "terms": conditions}
        elif key == "functions":
            out["modifier"] = [loot_26({("type" if k == "function" else k): v for k, v in f.items()}) for f in value]
        elif key == "bonus_rolls" and not value:
            continue
        else:
            out[key] = loot_26(value)
    return out


def loot_condition_26(condition):
    kind = condition.get("condition")
    if kind == "minecraft:match_tool":
        predicate = condition.get("predicate", {})
        if predicate.get("items") in ("minecraft:shears", ["minecraft:shears"]):
            return "minecraft:tool/can_shear"
        if "minecraft:silk_touch" in json.dumps(predicate):
            return "minecraft:tool/can_silk_touch"
    # 1.21's block_state_property is 26.3's match_block (a double slab's two, a door's lower half):
    # Sunrise refuses the old name, and Horizon may let it through as no condition at all.
    if kind == "minecraft:block_state_property":
        return {"type": "minecraft:match_block", "blocks": condition["block"], "state": condition.get("properties", {})}
    out = {"type": kind}
    for key, value in condition.items():
        if key == "term":
            out[key] = loot_condition_26(value)
        elif key == "terms":
            out[key] = [loot_condition_26(c) for c in value]
        elif key != "condition":
            out[key] = loot_26(value)
    return out


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


# The Horizon build (Nightly 2026-11-18, the first with a signed SDK) and horizon-mixins commit
# the mod is pinned to.
HORIZON = "0b14f8acfd97fa2c750e42795249dcc981178ca4"
MIXINS = "e59da9094f0c2fdda6c627fe9eff22dec6a906f0"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("occultism", type=Path)
    ap.add_argument("--out", type=Path, default=Path(__file__).resolve().parent.parent / "mod")
    args = ap.parse_args()

    occ = args.occultism
    gen = occ / "src/generated/resources"
    main_res = occ / "src/main/resources"
    occ_data = gen / "data"
    occ_assets = main_res / "assets" / MOD
    gen_assets = gen / "assets" / MOD
    if not (occ_data / MOD / "recipe" / "crushing").is_dir():
        die(f"{occ} doesn't look like an Occultism checkout (no generated crushing recipes)")

    # The mod folder also holds the Rust package (Cargo.toml, src/): only what this writes is replaced.
    out = args.out
    for generated in ("assets", "data"):
        if (out / generated).exists():
            shutil.rmtree(out / generated)
    assets = out / "assets" / MOD
    data = out / "data" / MOD

    out.mkdir(parents=True, exist_ok=True)
    (out / "horizon-mod.toml").write_text(
        f'''id = "{MOD}_horizon"
name = "Occultism"
version = "0.12.1-alpha.1"
description = "An unofficial alpha port to Horizon of Occultism by KliKli (https://github.com/klikli-dev), with their permission: ritual circles, spirits and their jobs, possessed creatures, the guide book, dimensional storage and the mineshaft."
authors = ["KliKli (https://github.com/klikli-dev), Occultism's creator", "Dawnforger (Horizon port)"]
api = "0.2"
side = "both"
capabilities = ["registries"]
# Its files and in-game ids keep Occultism's own namespace, so saved worlds carry over.
namespaces = ["{MOD}"]

# The Horizon build this is made for (Nightly 2026-11-18, whose signed SDK it compiles against)
# and the horizon-mixins commit its launcher pins. Move both forward deliberately, to a Nightly.
[native]
game = "26.3"
horizon = "{HORIZON}"
mixins = "{MIXINS}"

# AdLf0208's port, the Horizon registry's `occultism`, uses the same namespace: only one can be loaded.
[incompatible]
{MOD} = "*"
'''
    )
    # The notices go in the package: it takes only root files named README*, LICENSE*, LICENCE*,
    # COPYING* or NOTICE*.
    copy(occ / "LICENSE", out / "LICENSE-occultism.txt")
    copy(occ / "THIRD_PARTY_NOTICES.md", out / "NOTICE-occultism-third-party.txt")
    copy(Path(__file__).resolve().parent.parent / "NOTICE", out / "NOTICE.txt")
    # The notices point here for the textures based on Bonemouse's Isabella pack (CC BY 3.0).
    copy(occ_assets / "textures" / "about_isabella.txt", assets / "textures" / "about_isabella.txt")
    # The unbound spirits' GeckoLib models and animations, which the native code turns into model
    # layers: as Rust strings, since only Rust sources go into a native build.
    geo = occ_assets / "geckolib"
    consts = []
    for spirit in SPIRIT_LOOKS:
        for name, src in ((f"{spirit}_geo", geo / "models" / "entity" / f"{spirit}.geo.json"),
                          (f"{spirit}_animation", geo / "animations" / "entity" / f"{spirit}.animation.json")):
            text = src.read_text()
            if '"#' in text:
                die(f"{src} holds '\"#', which would end its raw string")
            consts.append(f'/// Occultism\'s `{src.relative_to(main_res)}`.\npub const {name.upper()}: &str = r#"{text}"#;\n')
    (out / "src" / "spirit_geo.rs").write_text(
        "// Generated by tools/build_content.py from Occultism's GeckoLib files (MIT): don't edit.\n\n" + "\n".join(consts))

    lang_src = json.loads((gen_assets / "lang" / "en_us.json").read_text())
    lang = {}

    # --- Dusts and the chalks' materials: items, models, textures, names.
    items = set()
    for name in [f"{d}_dust" for d in DUSTS] + MATERIALS + POSSESSED_DROPS:
        ident = f"{MOD}:{name}"
        items.add(ident)
        decl, components = ITEM_TRAITS.get(name, ({}, {}))
        write_json(data / "horizon" / "item" / f"{name}.json",
                   {"template": "minecraft:glowstone_dust", **decl, **({"components": components} if components else {})})
        copy(occ_assets / "textures" / "item" / f"{name}.png", assets / "textures" / "item" / f"{name}.png")
        write_json(assets / "models" / "item" / f"{name}.json",
                   {"parent": "minecraft:item/generated", "textures": {"layer0": f"{MOD}:item/{name}"}})
        write_json(assets / "items" / f"{name}.json",
                   {"model": {"type": "minecraft:model", "model": f"{MOD}:item/{name}"}})
        key = f"item.{MOD}.{name}"
        if key not in lang_src:
            die(f"no lang string {key}")
        lang[key] = lang_src[key]

    # --- The soul gem's art (soul_gem.rs).
    copy(occ_assets / "textures" / "item" / "soul_gem.png", assets / "textures" / "item" / "soul_gem.png")
    copy(occ_assets / "textures" / "item" / "soul_gem.png.mcmeta", assets / "textures" / "item" / "soul_gem.png.mcmeta")

    # --- Otherstone textures the mineshaft and Occultism's stone blocks share.
    for tex in ["chiseled_otherstone_bricks", "dimensional_mineshaft_top", "polished_otherstone"]:
        copy(occ_assets / "textures" / "block" / f"{tex}.png", assets / "textures" / "block" / f"{tex}.png")
        mcmeta = occ_assets / "textures" / "block" / f"{tex}.png.mcmeta"
        if mcmeta.exists():
            copy(mcmeta, assets / "textures" / "block" / f"{tex}.png.mcmeta")
    pickaxe = []  # data/minecraft/tags/block/mineable/pickaxe.json, written last

    # --- Tags: Occultism's c: tags as shipped, plus the vanilla entries (merged by the game).
    for f in sorted((occ_data / "c" / "tags" / "item").rglob("*.json")):
        rel = f.relative_to(occ_data / "c" / "tags" / "item")
        copy(f, out / "data" / "c" / "tags" / "item" / rel)
    for name, values in VANILLA_C_TAGS.items():
        path = out / "data" / "c" / "tags" / "item" / f"{name}.json"
        existing = json.loads(path.read_text())["values"] if path.exists() else []
        write_json(path, {"values": values + existing})
    tags = Tags(occ_data, items)

    tab = []

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
        """A block drawn with one of Occultism's own models (its textures copied), and its item. None
        of those models fills its block, so it hides none of its neighbours' faces (a barrel template
        would, and the floor under a mineshaft would show through)."""
        ident = f"{MOD}:{name}"
        items.add(ident)
        decl = {"template": template, "item": True, "properties": {**props, "occludes": False}}
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
    # Chalks are Occultism's: impure chalk purified in spirit fire (below, with the impure chalks).

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
    # 26.3 keeps configured features in worldgen/feature/ (Occultism's 1.21 files: configured_feature/).
    worldgen = data / "worldgen"
    write_json(worldgen / "feature" / "ore_silver.json", {
        "type": "minecraft:ore", "discard_chance_on_air_exposure": 0.0, "size": 7, "targets": [
            {"state": f"{MOD}:silver_ore",
             "target": {"predicate_type": "minecraft:tag_match", "tag": "minecraft:stone_ore_replaceables"}}]})
    write_json(worldgen / "feature" / "ore_silver_deepslate.json", {
        "type": "minecraft:ore", "discard_chance_on_air_exposure": 0.0, "size": 7, "targets": [
            {"state": f"{MOD}:silver_ore_deepslate",
             "target": {"predicate_type": "minecraft:tag_match", "tag": "minecraft:deepslate_ore_replaceables"}}]})
    occ_size = {n: json.loads((occ_data / MOD / "worldgen" / "configured_feature" / f"{n}.json").read_text())["config"]["size"]
                for n in ("ore_silver", "ore_silver_deepslate")}
    for n, size in occ_size.items():  # Occultism's vein sizes, in 26.3's flattened format
        path = worldgen / "feature" / f"{n}.json"
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
        "shapes": {"default": [[0, 0, 0, 1, 0.0625, 1]]}, "collision_shapes": {"default": []},
        # Its colour, set by a chalk's click (`SpiritFireBlock.COLOR`, the native mod).
        "state_properties": {"color": {"type": "enum", "values": list(CHALKS)}}, "default_state": {"color": "white"}})
    fire_model = json.loads((occ_assets / "models" / "block" / "spirit_fire.json").read_text())
    for element in fire_model["elements"]:
        for face in element["faces"].values():
            face.pop("tintindex", None)  # Occultism's white spirit fire: its texture as it is
    write_json(assets / "models" / "block" / "spirit_fire.json", fire_model)
    copy_texture(occ_assets / "textures" / "block" / "spirit_fire_0.png", assets / "textures" / "block" / "spirit_fire_0.png")
    # Coloured spirit fire (Occultism's `spirit_fire_color` model): its flames in the chalk's colour,
    # their inner layer the colour mixed 60% towards white (`SpiritFireBlock.getColor`), baked into
    # textures of their own, each with its animation.
    color_model = json.loads((occ_assets / "models" / "block" / "spirit_fire_color.json").read_text())
    for element in color_model["elements"]:
        for face in element["faces"].values():
            face.pop("tintindex", None)
    variants = {"color=white": {"model": f"{MOD}:block/spirit_fire"}}
    for colour, rgb in CHALKS.items():
        if colour == "white":
            continue
        light = sum(int(((rgb >> s) & 0xFF) + (0xFF - ((rgb >> s) & 0xFF)) * 0.6) << s for s in (16, 8, 0))
        for src, name, tint in (("spirit_fire_0", f"spirit_fire_{colour}", rgb), ("spirit_fire_3", f"spirit_fire_{colour}_light", light)):
            tint_png(occ_assets / "textures" / "block" / f"{src}.png", assets / "textures" / "block" / f"{name}.png", tint)
            copy(occ_assets / "textures" / "block" / f"{src}.png.mcmeta", assets / "textures" / "block" / f"{name}.png.mcmeta")
        model = dict(color_model, textures={"fire": f"{MOD}:block/spirit_fire_{colour}", "fireC": f"{MOD}:block/spirit_fire_{colour}_light",
                                            "particle": f"{MOD}:block/spirit_fire_{colour}"})
        write_json(assets / "models" / "block" / f"spirit_fire_{colour}.json", model)
        variants[f"color={colour}"] = {"model": f"{MOD}:block/spirit_fire_{colour}"}
    write_json(assets / "blockstates" / "spirit_fire.json", {"variants": variants})
    # The rituals' particles (particles.rs): the waiting smoke on vanilla's generic sprites (its motion
    # and shade in the native mod), and the spirit-fire flame as vanilla's flame moves.
    write_json(assets / "particles" / "ritual_waiting.json",
               {"textures": [f"minecraft:generic_{i}" for i in range(8)], "horizon:behaviour": {"lifetime": 45, "animation": "random"}})
    copy_texture(occ_assets / "textures" / "particle" / "spirit_fire_flame.png", assets / "textures" / "particle" / "spirit_fire_flame.png")
    write_json(assets / "particles" / "spirit_fire_flame.json",
               {"textures": [f"{MOD}:spirit_fire_flame"], "horizon:behaviour": {"lifetime": 16, "friction": 0.96, "size": [0.1, 0.05]}})
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

    # Essences of the unbound spirits, dropped when one is killed (their loot tables, below).
    for name in ("afrit_essence", "marid_essence"):
        flat_item(name, "minecraft:blaze_powder")

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
    # 9 slots hoppers fill and 9 a hopper below pulls from (the screen shows none), then the
    # terminal's crafting grid, as Occultism's: 3x3 from slot 18, its result in 27 (the block keeps
    # the grid). 4 numbers, then 6 upgrade slots' plate tiers.
    write_json(data / "horizon" / "menu" / "storage_actuator.json",
               {"slots": 28, "data_slots": 10, "results": list(range(9, 18)),
                "crafting": {"grid": 18, "size": 3, "result": 27}})
    # The terminal's own sprites: Occultism's item area (nine-sliced) and search field.
    for sprite in ("storage_controller_item_area_bg", "search_field"):
        src = occ_assets / "textures" / "gui" / "sprites" / "storage_controller" / f"{sprite}.png"
        dst = assets / "textures" / "gui" / "sprites" / "storage_controller" / f"{sprite}.png"
        copy_texture(src, dst)
        if src.with_name(src.name + ".mcmeta").exists():
            copy(src.with_name(src.name + ".mcmeta"), dst.with_name(dst.name + ".mcmeta"))
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
    # Occultism's own words for its terminal's buttons and capacity.
    for key, text in lang_src.items():
        if key.startswith(f"gui.{MOD}.storage_controller."):
            lang[key] = text
    lang[f"gui.{MOD}.storage.rows_next_open"] = "Takes effect the next time the terminal opens."
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

    # --- Occultism's equipment: silver and spirit-attuned (infused) tools and armour, the butcher
    # knives, and silver and infused horse and nautilus armour. Each is the vanilla item of its kind
    # with Occultism's material as a component patch (`OccultismTiers`, vanilla's ToolMaterial and
    # ArmorMaterial arithmetic), its equipment asset and Occultism's models.
    def item_def(name):
        """Occultism's `items/<name>.json` and every model it names, copied."""
        for root in (gen_assets, occ_assets):
            f = root / "items" / f"{name}.json"
            if f.exists():
                break
        else:
            die(f"no Occultism item definition {name}")
        d = json.loads(f.read_text())
        # NeoForge's trim overlay isn't a model type the game knows: its plain armour model instead
        # (a trim doesn't show on the icon).
        if d["model"].get("type") == "neoforge:trimmed_armor":
            d["model"] = d["model"]["base_model"]
        def models(node):
            if isinstance(node, dict):
                if node.get("type") == "minecraft:model" and node.get("model", "").startswith(f"{MOD}:item/"):
                    occ_model("item", node["model"].split("/", 1)[1])
                for v in node.values():
                    models(v)
            elif isinstance(node, list):
                for v in node:
                    models(v)
        models(d)
        write_json(assets / "items" / f"{name}.json", d)

    def gear(name, template, components, rarity=None):
        ident = f"{MOD}:{name}"
        items.add(ident)
        decl = {"template": template, "components": components}
        if rarity:
            decl["rarity"] = rarity
        write_json(data / "horizon" / "item" / f"{name}.json", decl)
        item_def(name)
        lang_of(f"item.{MOD}.{name}")
        tab.append(ident)

    def attack(damage, speed):
        return [{"type": "minecraft:attack_damage", "amount": damage, "id": "minecraft:base_attack_damage", "operation": "add_value", "slot": "mainhand"},
                {"type": "minecraft:attack_speed", "amount": speed, "id": "minecraft:base_attack_speed", "operation": "add_value", "slot": "mainhand"}]

    # Tool materials: (incorrect blocks, durability, speed, attack bonus, enchantability, repair tag).
    TOOL_MATERIALS = {
        "silver": ("#minecraft:incorrect_for_iron_tool", 200, 10.0, 1.0, 20, "#c:ingots/silver"),
        "infused": ("#minecraft:incorrect_for_diamond_tool", 588, 9.0, 3.0, 22, "#c:gems/spirit_attuned"),
        "iesnium": ("#minecraft:incorrect_for_diamond_tool", 1561, 10.0, 4.0, 24, "#c:ingots/iesnium"),
        "iron": ("#minecraft:incorrect_for_iron_tool", 250, 6.0, 2.0, 14, "#minecraft:iron_tool_materials"),
    }
    def tool(name, material, kind, damage, speed, rarity=None):
        incorrect, durability, mining, bonus, enchant, repair = TOOL_MATERIALS[material]
        components = {"minecraft:max_damage": durability, "minecraft:enchantable": {"value": enchant},
                      "minecraft:repairable": {"items": repair}, "minecraft:attribute_modifiers": attack(damage + bonus, speed)}
        if kind != "sword":  # a sword's rules don't depend on its material
            components["minecraft:tool"] = {"rules": [{"blocks": incorrect, "correct_for_drops": False},
                                                      {"blocks": f"#minecraft:mineable/{kind}", "correct_for_drops": True, "speed": mining}]}
        gear(name, f"minecraft:diamond_{kind}", components, rarity)

    def spear(name, material, duration, multiplier, delay, dismount, dismount_speed, knockback, knockback_speed, hurt, hurt_speed):
        """`Item.Properties.spear`: its times in seconds, as vanilla's own spears give them."""
        _, durability, _, bonus, enchant, repair = TOOL_MATERIALS[material]
        ticks = lambda t: round(t * 20)
        gear(name, "minecraft:iron_spear", {
            "minecraft:max_damage": durability, "minecraft:enchantable": {"value": enchant}, "minecraft:repairable": {"items": repair},
            "minecraft:attribute_modifiers": attack(bonus, 1.0 / duration - 4.0),
            "minecraft:attack_animation": {"type": "stab", "duration": ticks(duration)},
            "minecraft:kinetic_weapon": {
                "damage_conditions": {"max_duration_ticks": ticks(hurt), "min_relative_speed": hurt_speed}, "damage_multiplier": multiplier,
                "delay_ticks": ticks(delay), "dismount_conditions": {"max_duration_ticks": ticks(dismount), "min_speed": dismount_speed},
                "forward_movement": 0.38, "hit_sound": "minecraft:item.spear.hit",
                "knockback_conditions": {"max_duration_ticks": ticks(knockback), "min_speed": knockback_speed}, "sound": "minecraft:item.spear.use"}})

    tool("silver_sword", "silver", "sword", 3.0, -2.4)
    tool("silver_shovel", "silver", "shovel", 1.5, -3.0)
    tool("silver_pickaxe", "silver", "pickaxe", 1.0, -2.8)
    tool("silver_axe", "silver", "axe", 7.5, -3.1)
    tool("silver_hoe", "silver", "hoe", -1.0, -1.0)
    spear("silver_spear", "silver", 0.95, 0.95, 0.6, 2.5, 11.0, 6.75, 5.1, 11.25, 4.6)
    tool("infused_sword", "infused", "sword", 3.0, -1.9)
    tool("infused_shovel", "infused", "shovel", 1.5, -2.5)
    tool("infused_pickaxe", "infused", "pickaxe", 1.0, -2.3)
    tool("infused_axe", "infused", "axe", 5.0, -2.5)
    tool("infused_hoe", "infused", "hoe", -3.0, 0.5)
    spear("infused_spear", "infused", 0.8, 1.1, 0.45, 2.75, 9.5, 6.0, 5.1, 9.5, 4.6)
    # The butcher knife is an iron sword in all but looks; the iesnium one, Occultism's own.
    tool("butcher_knife", "iron", "sword", 3.0, -2.4)
    tool("iesnium_butcher_knife", "iesnium", "sword", 10.0, -1.8, rarity="uncommon")

    # Armour materials: (durability multiplier, defence by slot, enchantability, equip sound,
    # toughness, knockback resistance, repair tag, equipment asset).
    ARMOR_MATERIALS = {
        "silver": (13, {"feet": 1, "legs": 4, "chest": 5, "head": 2, "body": 4}, 20, "minecraft:item.armor.equip_iron", 0.0, 0.0, "#c:ingots/silver", "silver"),
        "infused": (24, {"feet": 3, "legs": 6, "chest": 8, "head": 3, "body": 13}, 22, "minecraft:item.armor.equip_diamond", 3.0, 0.1, "#c:gems/spirit_attuned", "attuned_crystal"),
    }
    ARMOR_PIECES = {"helmet": ("head", 11), "chestplate": ("chest", 16), "leggings": ("legs", 15), "boots": ("feet", 13)}
    def armour_modifiers(material, slot, piece_id):
        _, defence, _, _, toughness, knockback, _, _ = ARMOR_MATERIALS[material]
        out = [{"type": "minecraft:armor", "amount": float(defence[slot]), "id": f"minecraft:armor.{piece_id}", "operation": "add_value", "slot": slot},
               {"type": "minecraft:armor_toughness", "amount": toughness, "id": f"minecraft:armor.{piece_id}", "operation": "add_value", "slot": slot}]
        if knockback > 0:
            out.append({"type": "minecraft:knockback_resistance", "amount": knockback, "id": f"minecraft:armor.{piece_id}", "operation": "add_value", "slot": slot})
        return out
    for material, (multiplier, _, enchant, sound, _, _, repair, asset) in ARMOR_MATERIALS.items():
        for piece, (slot, base) in ARMOR_PIECES.items():
            piece_id = {"head": "helmet", "chest": "chestplate", "legs": "leggings", "feet": "boots"}[slot]
            gear(f"{material}_{piece}", f"minecraft:iron_{piece}", {
                "minecraft:max_damage": base * multiplier, "minecraft:enchantable": {"value": enchant}, "minecraft:repairable": {"items": repair},
                "minecraft:attribute_modifiers": armour_modifiers(material, slot, piece_id),
                "minecraft:equippable": {"asset_id": f"{MOD}:{asset}", "equip_sound": sound, "slot": slot}})
        body = armour_modifiers(material, "body", "body")
        gear(f"{material}_horse_armor", "minecraft:iron_horse_armor", {"minecraft:attribute_modifiers": body, "minecraft:equippable": {
            "allowed_entities": "#minecraft:can_wear_horse_armor", "asset_id": f"{MOD}:{asset}", "can_be_sheared": True, "damage_on_hurt": False,
            "equip_sound": "minecraft:entity.horse.armor", "shearing_sound": "minecraft:item.horse_armor.unequip", "slot": "body"}})
        gear(f"{material}_nautilus_armor", "minecraft:iron_nautilus_armor", {"minecraft:attribute_modifiers": body, "minecraft:equippable": {
            "allowed_entities": "#minecraft:can_wear_nautilus_armor", "asset_id": f"{MOD}:{asset}", "can_be_sheared": True, "damage_on_hurt": False,
            "equip_on_interact": True, "equip_sound": "minecraft:item.armor.equip_nautilus", "shearing_sound": "minecraft:item.armor.unequip_nautilus", "slot": "body"}})
        # The armour as worn: Occultism's equipment asset and its layers.
        copy(occ_assets / "equipment" / f"{asset}.json", assets / "equipment" / f"{asset}.json")
        for layer in ("humanoid", "humanoid_baby", "humanoid_leggings", "horse_body", "nautilus_body"):
            copy(occ_assets / "textures" / "entity" / "equipment" / layer / f"{asset}.png",
                 assets / "textures" / "entity" / "equipment" / layer / f"{asset}.png")
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
    # --- Soul gems, as Occultism's: a creature captured in the gem as its entity data
    # (`minecraft:entity_data`), released where the gem is next used; the fragile one breaks then.
    # Empty, a gem shows its empty model and is named "Empty Soul Gem"; the native mod names a
    # filled one with `item.occultism.<gem>.filled`. The trinity gem waits for the eldritch rituals.
    for gem in ("fragile_soul_gem", "soul_gem"):
        ident = f"{MOD}:{gem}"
        items.add(ident)
        write_json(data / "horizon" / "item" / f"{gem}.json", {"template": "minecraft:paper", "max_stack_size": 1})
        empty, filled = occ_model("item", f"{gem}_empty"), occ_model("item", f"{gem}_filled")
        write_json(assets / "items" / f"{gem}.json", {"model": {
            "type": "minecraft:condition", "property": "minecraft:has_component", "component": "minecraft:entity_data",
            "on_true": {"type": "minecraft:model", "model": filled},
            "on_false": {"type": "minecraft:model", "model": empty}}})
        lang[f"item.{MOD}.{gem}"] = lang_src[f"item.{MOD}.{gem}_empty"]
        lang[f"item.{MOD}.{gem}.filled"] = lang_src[f"item.{MOD}.{gem}"]
        for key in ("message.entity_type_denied", "tooltip_empty", "tooltip_filled"):
            lang[f"item.{MOD}.{gem}.{key}"] = lang_src[f"item.{MOD}.{gem}.{key}"]
        tab.append(ident)
        # What it refuses: Occultism's list, without its iesnium golem (not in this port): the
        # bosses (NeoForge's c:bosses) and what any mod marks as not to be captured.
        write_json(data / "tags" / "entity_type" / f"deny_list_{gem}.json", {"values": [
            "minecraft:ender_dragon", "minecraft:wither", {"id": "#c:capturing_not_supported", "required": False}]})
    lang[f"item.{MOD}.soul_gem.message.no_room"] = "There is no room to release it here."

    # --- Working spirits: Occultism's brush (a summoning's ingredient) and the janitor's book of
    # calling, which the native mod binds to its spirit and uses to set its base, deposit and work
    # area. Occultism's book opens a screen for its mode and work area; this port's turns through
    # them when used in the air (sneaking for the work area), and says them in its tooltip.
    flat_item("brush", "minecraft:paper", extra={"max_stack_size": 1})
    flat_item("book_of_calling_foliot_cleaner", "minecraft:paper", texture="book_of_calling_cleaner", extra={"max_stack_size": 1})
    flat_item("book_of_calling_foliot_farmer", "minecraft:paper", texture="book_of_calling_farmer", extra={"max_stack_size": 1})
    flat_item("book_of_calling_foliot_lumberjack", "minecraft:paper", texture="book_of_calling_lumberjack",
              extra={"max_stack_size": 1})
    flat_item("book_of_calling_foliot_transport_items", "minecraft:paper", texture="book_of_calling_transport_items",
              extra={"max_stack_size": 1})
    for key in ["item.occultism.book_of_calling_foliot.tooltip", "gui.occultism.book_of_calling.mode",
                "gui.occultism.book_of_calling.work_area", "enum.occultism.book_of_calling.item_mode.set_base",
                "enum.occultism.book_of_calling.item_mode.set_deposit", "enum.occultism.book_of_calling.item_mode.set_extract",
                "item.occultism.book_of_calling_foliot.tooltip.extract", "item.occultism.book_of_calling_foliot.tooltip.deposit"] + [
                f"enum.occultism.work_area_size.{s}" for s in ("small", "medium", "large")] + [
                f"item.occultism.book_of_calling.{m}" for m in ("message_set_base", "message_set_deposit", "message_set_extract",
                "message_set_work_area_size", "message_spirit_not_found", "message_target_cannot_link",
                "message_target_linked")]:
        lang[key] = lang_src[key]
    lang["gui.occultism.book_of_calling.mode_set"] = "Mode: %s"
    lang["gui.occultism.book_of_calling.work_area_set"] = "Work area: %s"

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

    # --- Otherworld wood: Occultism's otherworld tree (its sapling from an oak sapling in spirit
    # fire, grown into `occultism:tree_otherworld` by the native mod), its leaves that drop pitaya;
    # and as before, an oak log a player with the Third Eye chops is now and then an otherworld log
    # (the native mod, at iesnium's rates). Its planks, and its ashes from spirit fire: with burnt
    # otherstone or otherrock, the base of chalk.
    items.update({f"{MOD}:otherworld_leaves", f"{MOD}:otherworld_sapling", f"{MOD}:pitaya"})
    for name, template, model in (
        ("otherworld_leaves", "minecraft:oak_leaves", {"parent": "minecraft:block/leaves", "textures": {"all": f"{MOD}:block/otherworld_leaves"}}),
        ("otherworld_sapling", "minecraft:oak_sapling", {"parent": "minecraft:block/cross", "textures": {"cross": f"{MOD}:block/otherworld_sapling"}}),
    ):
        write_json(data / "horizon" / "block" / f"{name}.json", {"template": template, "item": True})
        copy_texture(occ_assets / "textures" / "block" / f"{name}.png", assets / "textures" / "block" / f"{name}.png")
        write_json(assets / "models" / "block" / f"{name}.json", model)
        write_json(assets / "blockstates" / f"{name}.json", {"variants": {"": {"model": f"{MOD}:block/{name}"}}})
        write_json(data / "loot_table" / "blocks" / f"{name}.json",
                   loot_26(json.loads((occ_data / MOD / "loot_table" / "blocks" / f"{name}.json").read_text())))
        lang_of(f"block.{MOD}.{name}")
        tab.append(f"{MOD}:{name}")
    write_json(assets / "items" / "otherworld_leaves.json", {"model": {"type": "minecraft:model", "model": f"{MOD}:block/otherworld_leaves"}})
    write_json(assets / "models" / "item" / "otherworld_sapling.json",
               {"parent": "minecraft:item/generated", "textures": {"layer0": f"{MOD}:block/otherworld_sapling"}})
    write_json(assets / "items" / "otherworld_sapling.json", {"model": {"type": "minecraft:model", "model": f"{MOD}:item/otherworld_sapling"}})
    # In worldgen/feature/ and 26.3's tree format (Occultism's file is 1.21's): Occultism's tree has
    # the oak's shape (its trunk, blob foliage and sizes), so it is vanilla's oak with its blocks.
    tree = json.loads((occ_data / MOD / "worldgen" / "configured_feature" / "tree_otherworld.json").read_text())["config"]
    write_json(data / "worldgen" / "feature" / "tree_otherworld.json", {
        "type": "minecraft:tree", "below_trunk_provider": "minecraft:soil_beneath_tree", "decorators": tree["decorators"],
        "foliage_placer": tree["foliage_placer"],
        "foliage_provider": {"id": tree["foliage_provider"]["state"]["Name"], "properties": tree["foliage_provider"]["state"]["Properties"]},
        "ignore_vines": tree["ignore_vines"], "minimum_size": {"type": tree["minimum_size"]["type"]},
        "trunk_placer": tree["trunk_placer"],
        "trunk_provider": {"id": tree["trunk_provider"]["state"]["Name"], "properties": tree["trunk_provider"]["state"]["Properties"]}})
    # As vanilla's own: the lumberjack takes and plants saplings and gathers leaves by these tags.
    for kind, tag, values in (("item", "saplings", ["otherworld_sapling"]), ("block", "saplings", ["otherworld_sapling"]),
                              ("item", "leaves", ["otherworld_leaves"]), ("block", "leaves", ["otherworld_leaves"]),
                              ("item", "logs", ["otherworld_log"]), ("block", "logs", ["otherworld_log"])):
        write_json(out / "data" / "minecraft" / "tags" / kind / f"{tag}.json", {"values": [f"{MOD}:{v}" for v in values]})
    # Occultism's spirit campfire (`CampfireBlock(false, 0)`, a campfire that doesn't burn), on vanilla's
    # campfire, which the marid smelter's summoning takes; its recipe is Occultism's, copied below.
    items.add(f"{MOD}:spirit_campfire")
    write_json(data / "horizon" / "block" / "spirit_campfire.json", {"template": "minecraft:campfire", "item": True})
    for tex in ("spirit_campfire_fire", "spirit_campfire_log_lit"):
        copy_texture(occ_assets / "textures" / "block" / f"{tex}.png", assets / "textures" / "block" / f"{tex}.png")
        copy(occ_assets / "textures" / "block" / f"{tex}.png.mcmeta", assets / "textures" / "block" / f"{tex}.png.mcmeta")
    copy_texture(occ_assets / "textures" / "item" / "spirit_campfire.png", assets / "textures" / "item" / "spirit_campfire.png")
    write_json(assets / "models" / "block" / "spirit_campfire.json", {"parent": "minecraft:block/template_campfire", "textures": {
        "fire": f"{MOD}:block/spirit_campfire_fire", "lit_log": f"{MOD}:block/spirit_campfire_log_lit"}})
    copy(occ_assets / "blockstates" / "spirit_campfire.json", assets / "blockstates" / "spirit_campfire.json")
    write_json(assets / "models" / "item" / "spirit_campfire.json",
               {"parent": "minecraft:item/generated", "textures": {"layer0": f"{MOD}:item/spirit_campfire"}})
    write_json(assets / "items" / "spirit_campfire.json", {"model": {"type": "minecraft:model", "model": f"{MOD}:item/spirit_campfire"}})
    write_json(data / "loot_table" / "blocks" / "spirit_campfire.json",
               loot_26(json.loads((occ_data / MOD / "loot_table" / "blocks" / "spirit_campfire.json").read_text())))
    lang_of(f"block.{MOD}.spirit_campfire")
    tab.append(f"{MOD}:spirit_campfire")
    # The ritual satchels (`SingleBlockRitualSatchelItem`, `MultiBlockRitualSatchelItem`): one block
    # of the previewed circle at a time, or all of it.
    flat_item("ritual_satchel_t1", "minecraft:paper", extra={"max_stack_size": 1})
    flat_item("ritual_satchel_t2", "minecraft:paper", extra={"max_stack_size": 1, "rarity": "uncommon",
                                                            "components": FIRE_PROOF})
    for key in ("ritual_satchel_t1", "ritual_satchel_t2"):
        lang_of(f"item.{MOD}.{key}.auto_tooltip")
    for key in ("block_at_position_not_air", "glyph_cannot_survive", "no_valid_item_in_satchel"):
        lang_of(f"item.{MOD}.ritual_satchel.{key}")
    for key, text in (("not_in_port", "Not in this port yet"), ("back", "Back"), ("visualize", "Show in the world"),
                      ("open_category", "Open the category")):
        lang[f"book.{MOD}.{key}"] = text
    # Pitaya, the otherworld fruit (`OccultismFoods.PITAYA`): the Third Eye, and a twentieth's chance
    # of each of eight boons.
    flat_item("pitaya", "minecraft:apple", extra={"components": food(6, 3.0, (
        {"id": f"{MOD}:third_eye", "duration": 30 * 20, "amplifier": 0, "ambient": False, "show_particles": True}, 1.0),
        *[({"id": f"minecraft:{e}", "duration": 15 * 20, "amplifier": 1, "ambient": False, "show_particles": True}, 0.05)
          for e in ("speed", "haste", "strength", "jump_boost", "regeneration", "resistance", "fire_resistance", "water_breathing")])})
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
        else:
            # Occultism's own needs what this port doesn't have yet (a possessed creature's drop, or
            # the otherworld trees' fruit): dyed from white chalk until then.
            write_json(data / "recipe" / f"chalk_{colour}.json", {
                "type": "minecraft:crafting_shapeless", "category": "misc",
                "ingredients": [f"{MOD}:chalk_white", f"minecraft:{colour}_dye"],
                "result": {"id": f"{MOD}:chalk_{colour}", "count": 1}})

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

    # --- Furnishings: the golden pitayas, the large candles, copper and silver bowls, otherglass, the
    # otherflower and the otherrock building blocks. Their recipes are Occultism's, copied below.
    def always_edible(components):
        components["minecraft:food"]["can_always_eat"] = True
        return components
    third_eye = lambda s: ({"id": f"{MOD}:third_eye", "duration": s * 20, "amplifier": 0, "ambient": False, "show_particles": True}, 1.0)
    effect = lambda e, s, a: ({"id": f"minecraft:{e}", "duration": s * 20, "amplifier": a, "ambient": False, "show_particles": True}, 1.0)
    flat_item("golden_pitaya", "minecraft:golden_apple", extra={"components": always_edible(food(6, 9.0, third_eye(10), effect("health_boost", 60, 4)))})
    flat_item("enchanted_golden_pitaya", "minecraft:golden_apple", extra={"rarity": "rare", "components": always_edible(food(
        6, 13.0, third_eye(3), effect("health_boost", 180, 4), effect("resistance", 180, 2), effect("fire_resistance", 180, 0),
        effect("water_breathing", 180, 0), effect("wind_charged", 180, 2)))})

    def copied_block(name, template, props=None, loot=None):
        """A block drawn as Occultism draws it: its blockstate, the models those name (and theirs) and
        their textures copied as they are, with its item."""
        ident = f"{MOD}:{name}"
        items.add(ident)
        decl = {"template": template, "item": True}
        if props:
            decl["properties"] = props
        write_json(data / "horizon" / "block" / f"{name}.json", decl)
        def model_src(ref):
            kind, rel = ref.split(":", 1)[1].split("/", 1)
            for root in (gen_assets, main_res / "assets" / MOD):
                f = root / "models" / kind / f"{rel}.json"
                if f.exists():
                    return f
            return None
        def take_model(ref, seen):
            if not ref.startswith(f"{MOD}:") or ref in seen:
                return
            seen.add(ref)
            f = model_src(ref)
            if f is None:
                die(f"block {name}: no model {ref}")
            m = json.loads(f.read_text())
            kind, rel = ref.split(":", 1)[1].split("/", 1)
            write_json(assets / "models" / kind / f"{rel}.json", m)
            for tex in m.get("textures", {}).values():
                if tex.startswith(f"{MOD}:block/") or tex.startswith(f"{MOD}:item/"):
                    tkind, trel = tex.split(":", 1)[1].split("/", 1)
                    copy_texture(occ_assets / "textures" / tkind / f"{trel}.png", assets / "textures" / tkind / f"{trel}.png")
            if "parent" in m:
                take_model(m["parent"], seen)
        def asset(kind, file, fallback=None):
            for root in (gen_assets, main_res / "assets" / MOD):
                if (root / kind / file).exists():
                    return json.loads((root / kind / file).read_text())
            if fallback is not None:
                return fallback
            die(f"block {name}: no {kind}/{file}")
        states = asset("blockstates", f"{name}.json")
        seen = set()
        for variant in list(states.get("variants", {}).values()) + [part["apply"] for part in states.get("multipart", [])]:
            for v in variant if isinstance(variant, list) else [variant]:
                take_model(v["model"], seen)
        write_json(assets / "blockstates" / f"{name}.json", states)
        item_def = asset("items", f"{name}.json", {"model": {"type": "minecraft:model", "model": f"{MOD}:block/{name}"}})
        take_model(item_def["model"]["model"], seen)
        write_json(assets / "items" / f"{name}.json", item_def)
        src_loot = occ_data / MOD / "loot_table" / "blocks" / f"{name}.json"
        write_json(data / "loot_table" / "blocks" / f"{name}.json",
                   loot or (alias(loot_26(json.loads(src_loot.read_text()))) if src_loot.exists() else loot_self(ident)))
        lang_of(f"block.{MOD}.{name}")
        pickaxe.append(ident)
        tab.append(ident)

    for candle in LARGE_CANDLES:
        # A candle as vanilla's: lit with flint and steel, put out with a click (light 3, not 15).
        name = candle.split(":")[1]
        copied_block(name, "minecraft:candle", {"hardness": 0.1, "sounds": "minecraft:candle", "occludes": False})
        # Occultism tints a coloured candle with its chalk's colour (`ColorEventHandler`): baked into
        # a texture of its own here, as the glyphs' are.
        colour = name.removeprefix("large_candle").removeprefix("_")
        if colour:
            model = json.loads((assets / "models" / "block" / f"{name}.json").read_text())
            for element in model["elements"]:
                for face in element["faces"].values():
                    face.pop("tintindex", None)
            tint_png(occ_assets / "textures" / "block" / "large_candle.png", assets / "textures" / "block" / f"{name}.png", CHALKS[colour])
            model["textures"] = {k: f"{MOD}:block/{name}" for k in model["textures"]}
            write_json(assets / "models" / "block" / f"{name}.json", model)
    for metal in ("copper", "silver", "dark", "dark_copper", "dark_silver"):
        # Sacrificial bowls as the plain one (rites.rs's BOWLS), always upright: Occultism's turn
        # with the face they are put on, which the glass template has no state for.
        name = f"{metal}_sacrificial_bowl"
        copied_block(name, "minecraft:glass", {**bowl_props, "occludes": False})
        write_json(assets / "blockstates" / f"{name}.json", {"variants": {"": {"model": f"{MOD}:block/{name}"}}})
    # The spirit lantern and torch, as soul ones with Occultism's light (10). The wall torch is
    # declared for worlds that have one; the torch's item doesn't put it on walls at this SDK pin
    # (spirit_fire.rs).
    copied_block("spirit_lantern", "minecraft:soul_lantern", {"hardness": 3.5, "light": 10, "requires_tool": True})
    copied_block("spirit_torch", "minecraft:soul_torch", {"light": 10})
    pickaxe.remove(f"{MOD}:spirit_torch")
    lang_src.setdefault(f"block.{MOD}.spirit_wall_torch", lang_src[f"block.{MOD}.spirit_torch"])  # named as the torch
    copied_block("spirit_wall_torch", "minecraft:soul_wall_torch", {"light": 10}, loot=loot_self(f"{MOD}:spirit_torch"))
    pickaxe.remove(f"{MOD}:spirit_wall_torch")
    items.discard(f"{MOD}:spirit_wall_torch")
    tab.remove(f"{MOD}:spirit_wall_torch")
    (assets / "items" / "spirit_wall_torch.json").unlink()
    wall = json.loads((data / "horizon" / "block" / "spirit_wall_torch.json").read_text())
    write_json(data / "horizon" / "block" / "spirit_wall_torch.json", {**wall, "item": False, "pick_item": f"{MOD}:spirit_torch"})
    # Golden bowls of their own (rites.rs's GOLDEN_BOWLS): the dark golden bowl, and the iesnium
    # ones, which run a ritual four times as fast.
    for name in ("dark_golden_sacrificial_bowl", "iesnium_sacrificial_bowl", "dark_iesnium_sacrificial_bowl"):
        copied_block(name, "minecraft:glass", {**bowl_props, "occludes": False})
    copied_block("otherglass", "minecraft:glass", {"hardness": 0.3, "sounds": "minecraft:glass", "occludes": False, "requires_tool": False},
                 loot=loot_self(f"{MOD}:otherglass"))
    copied_block("otherflower", "minecraft:poppy", {"hardness": 0.0, "sounds": "minecraft:short_grass", "occludes": False, "requires_tool": False},
                 loot=loot_self(f"{MOD}:otherflower"))
    pickaxe.remove(f"{MOD}:otherflower")
    for name, template in OTHERROCK_BLOCKS.items():
        copied_block(name, template, {"hardness": 1.5, "explosion_resistance": 6.0, "requires_tool": True})
    # Silver's building blocks, each the vanilla block Occultism copies (`OccultismBlocks`): copper's
    # waxed forms, which don't weather, and iron's bars and chain.
    for name, template in {
        "chiseled_silver": "minecraft:waxed_chiseled_copper", "silver_grate": "minecraft:waxed_copper_grate",
        "cut_silver": "minecraft:waxed_cut_copper", "cut_silver_stairs": "minecraft:waxed_cut_copper_stairs",
        "cut_silver_slab": "minecraft:waxed_cut_copper_slab", "silver_bars": "minecraft:iron_bars",
        "silver_chain": "minecraft:iron_chain", "silver_door": "minecraft:waxed_copper_door",
        "silver_trapdoor": "minecraft:waxed_copper_trapdoor", "silver_bulb": "minecraft:waxed_copper_bulb",
    }.items():
        copied_block(name, template)

    # Occultism's own crafting, smelting and blasting recipes for what exists now (silver and
    # iesnium storage, nuggets, smelting, the golden bowl, crystal, lenses, goggles...).

    ours = {p.stem for p in (data / "recipe").glob("*.json")}
    copied = []
    for kind in ("crafting", "smelting", "blasting", "stonecutting"):
        for f in sorted((occ_data / MOD / "recipe" / kind).glob("*.json")):
            j = alias(json.loads(f.read_text()))
            j.pop("neoforge:conditions", None)
            result = j.get("result", {})
            rid = result.get("id") if isinstance(result, dict) else None
            if j.get("type") not in ("minecraft:crafting_shaped", "minecraft:crafting_shapeless",
                                     "minecraft:smelting", "minecraft:blasting", "minecraft:stonecutting"):
                continue
            # Recipes for vanilla items are left out, but for the otherflower's dyes.
            if (rid not in items and not f.stem.startswith("otherflower_to_")) or rid.split(":")[1] in ours \
                    or rid.startswith(f"{MOD}:book_of_binding_bound"):
                continue
            ins = list(ingredient_ids(list((j.get("key") or {}).values()))) + list(ingredient_ids(j.get("ingredients"))) + list(ingredient_ids(j.get("ingredient")))
            if any(i in PASTES for i in ins):
                continue  # written below, as paste recipes
            if ins and all(resolves(i) for i in ins):
                write_json(data / "recipe" / f"{kind}_{f.name}", j)
                copied.append(f"{kind}/{f.stem}")

    # --- Occultism's paste recipes: every crafting recipe that takes a gray or nature paste, as this
    # port's `occultism:paste_shapeless` (paste.rs), which leaves the paste in the grid a use worn.
    # Their ingredients are resolved to items here (the native mod can't read item tags), and every
    # item they name is listed in src/paste_items.rs for the mod to look up at startup.
    crafting = occ_data / MOD / "recipe" / "crafting"
    pasted = [crafting / f"chalk_{c}_impure.json" for c in ("gray", "green")]
    pasted += sorted((crafting / "gray_paste").glob("*.json")) + sorted((crafting / "nature_paste").glob("*.json")) + sorted((crafting / "flaming_paste").glob("*.json"))
    paste_items, paste_recipes = [], []
    for f in pasted:
        j = json.loads(f.read_text())
        if j["type"] != "minecraft:crafting_shapeless":
            die(f"paste recipe {f.stem}: {j['type']} isn't read")
        ingredients = []
        for i in j["ingredients"]:
            i = i if isinstance(i, str) else (i.get("item") or "#" + i["tag"])
            ingredients.append(tags.resolve(i[1:]) if i.startswith("#") else ([i] if tags.known(i) else []))
        result = j["result"]["id"]
        if not all(ingredients) or not tags.known(result):
            continue
        for ident in [x for ingredient in ingredients for x in ingredient] + [result]:
            if ident not in paste_items:
                paste_items.append(ident)
        name = f.stem if f.parent == crafting else f"{f.parent.name}_{f.stem}"
        write_json(data / "recipe" / f"paste_{name}.json", {"type": f"{MOD}:paste_shapeless", "ingredients": ingredients,
                                                          "result": {"id": result, "count": j["result"].get("count", 1)}})
        paste_recipes.append(name)
    (out / "src" / "paste_items.rs").write_text(
        "// Generated by tools/build_content.py: every item the paste recipes name (paste.rs). Don't edit.\n\n"
        "pub const ITEMS: &[&str] = &[\n" + "".join(f'    "{i}",\n' for i in paste_items) + "];\n")

    # --- Spirit fire's recipes (read by the native mod), for what this port has; the item tags they
    # and the crafting recipes name, with the entries this port has.
    fire_recipes = []
    for f in sorted((occ_data / MOD / "recipe" / "spirit_fire").glob("*.json")):
        j = alias(json.loads(f.read_text()))
        j.pop("neoforge:conditions", None)
        if resolves(j["ingredient"]) and tags.known(j["result"]["id"]):
            write_json(data / "recipe" / "spirit_fire" / f.name, j)
            fire_recipes.append(f.stem)
    for name in ("datura", "start_spiritfire", "books/books_for_empty", "otherworld_logs", "dusts/chalk_base",
                 "tools/chalk", "miners"):
        values = [v for v in tags.files[f"{MOD}:{name}"] if resolves(v)]
        write_json(data / "tags" / "item" / f"{name}.json", {"values": values})

    # --- Crushing and crystallizing recipes that work with vanilla plus this port's items.
    kept, dropped = [], []
    for f in sorted((occ_data / MOD / "recipe" / "crushing").glob("*.json")) + sorted((occ_data / MOD / "recipe" / "crystallize").glob("*.json")):
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
            write_json(data / "recipe" / f.parent.name / f.name, j)
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

    # --- The traders' trades (`occultism:spirit_trade`, read by the native mod): what a trader
    # takes, and the results it picks among by weight. A result named by a tag (the gambler's) is
    # the tag's first item, as Occultism's `TagUtil.getItemForTag` takes it.
    for name in TRADES:
        j = json.loads((occ_data / MOD / "recipe" / "spirit_trade" / f"{name}.json").read_text())
        r = j["result"]
        if r["type"] == f"{MOD}:weighted_tag":
            found = tags.resolve(r["tag"])
            if not found:
                die(f"trade {name}: nothing this port has is in #{r['tag']}")
            item, count = found[0], r.get("count", 1)
        else:
            item, count = r["stack"]["id"], r["stack"].get("count", 1)
        if not resolves(j["ingredient"]) or not tags.known(item):
            die(f"trade {name}: {j['ingredient']} or {item} isn't in this port")
        write_json(data / "recipe" / "spirit_trade" / f"{name}.json",
                   {"type": f"{MOD}:spirit_trade", "ingredient": j["ingredient"], "result": {"id": item, "count": count},
                    "weight": r.get("weight", 1), "trader": j["trader_id"]})

    # --- Ritual recipes (`occultism:ritual`, read by the native mod as the crushing ones are).
    for name, result in RITUALS.items():
        j = alias(json.loads((occ_data / MOD / "recipe" / "ritual" / f"{name}.json").read_text()))
        # Occultism's otherworld trees aren't in this port: its lumberjack summoning takes a
        # jungle sapling for their sapling.
        j["ingredients"] = [RITUAL_STAND_INS.get(i, i) if isinstance(i, str) else i for i in j["ingredients"]]
        j.pop("neoforge:conditions", None)
        if j["pentacle_id"].split(":")[1] not in PENTACLES:
            die(f"ritual {name}: pentacle {j['pentacle_id']} isn't shipped")
        # A ritual without a duration takes Occultism's DEFAULT_DURATION, 30 seconds.
        ritual = {"type": f"{MOD}:ritual", "activation_item": j["activation_item"], "duration": j.get("duration", 30),
                  "ingredients": j["ingredients"], "pentacle_id": j["pentacle_id"],
                  "result": {"id": result or j["result"]["id"], "count": j["result"].get("count", 1)}}
        if j.get("ritual_type") == f"{MOD}:summon_spirit_with_job" and result is None:
            # The spirit comes to do its job; the result is its book of calling. (The crushers
            # bind into cores instead: their result is this port's.)
            ritual["ritual_type"] = j["ritual_type"]
            ritual["summon"] = j["entity_to_summon"]
            ritual["job"] = j["spirit_job_type"]
            lang_of(f"entity.{MOD}.{j['entity_to_summon'].split(':')[1]}")
            lang_of(f"job.{MOD}.{j['spirit_job_type'].split(':')[1]}")
            # A spirit that goes after a while (`spirit_max_age`, in seconds): the traders.
            if "spirit_max_age" in j:
                ritual["max_age"] = j["spirit_max_age"]
            # A crusher, crystallizer or smelter comes with no book: Occultism's result only names the
            # ritual (a spawn egg).
            if j["spirit_job_type"].split(":")[1].startswith(("crush_tier", "crystal_tier", "smelt_tier")) or j["spirit_job_type"] in SKY_JOBS + TRADER_JOBS:
                del ritual["result"]
        if j.get("ritual_type") in (f"{MOD}:summon", f"{MOD}:summon_wild"):
            # The creature is the result; Occultism's `result` only names the ritual for its book (a
            # spawn egg this port may not have). A ritual of a tag summons one of its kinds, chosen
            # each time: the native mod can't read entity tags.
            ritual["ritual_type"] = j["ritual_type"]
            if "entity_tag_to_summon" in j:
                ritual["summon"] = entity_tag(occ_data, j["entity_tag_to_summon"])
                if not ritual["summon"]:
                    die(f"ritual {name}: nothing this port has is in #{j['entity_tag_to_summon']}")
            else:
                ritual["summon"] = j["entity_to_summon"]
                if not j["entity_to_summon"].startswith("minecraft:"):
                    lang_of(f"entity.{MOD}.{j['entity_to_summon'].split(':')[1]}")
            if j["ritual_type"] == f"{MOD}:summon_wild":
                ritual["summon_number"] = j.get("summon_number", 1)
            if not resolves(ritual["result"]["id"]):
                del ritual["result"]
        if "item_to_use" in j:
            # A player near the bowl must use this item before the ritual's time runs.
            ritual["item_to_use"] = j["item_to_use"]
            lang_of(f"ritual.{MOD}.use_item")
        if j.get("ritual_type") in (f"{MOD}:repair", f"{MOD}:craft_with_spirit_name", f"{MOD}:upgrade"):
            ritual["ritual_type"] = j["ritual_type"]
        if j.get("ritual_type") == f"{MOD}:craft_with_spirit_name":
            # The book's spirit goes into the result (`CraftWithSpiritNameRitual`): both say its name.
            for ident in (ritual["activation_item"], ritual["result"]["id"]):
                key = f"item.{MOD}.{ident.split(':')[1]}.tooltip"
                said = lang_src.get(key, "")
                lang[key] = said if "%s" in said else "Bound to the spirit %s."
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
        results = [ritual["result"]["id"]] if "result" in ritual else []
        for ident in [ritual["activation_item"]] + results + list(ingredient_ids(ritual["ingredients"])):
            if not resolves(ident):
                die(f"ritual {name}: {ident} isn't in this port")
        write_json(data / "recipe" / "ritual" / f"{name}.json", ritual)
    # Spirit mobs: their types, loot (their essence) and textures (drawn by the native mod).
    for name, attributes in SPIRIT_MOBS.items():
        write_json(data / "horizon" / "entity_type" / f"{name}.json", {"template": "minecraft:zombie", "attributes": attributes})
        loot = occ_data / MOD / "loot_table" / "entities" / f"{name}.json"
        write_json(data / "loot_table" / "entities" / f"{name}.json", loot_26(json.loads(loot.read_text())))
        lang_of(f"entity.{MOD}.{name}")
    # The native mod draws them from Occultism's GeckoLib models and animations (src/spirit_geo.rs,
    # written above), in their own textures.
    for spirit in SPIRIT_LOOKS:
        copy(occ_assets / "textures" / "entity" / f"{spirit}.png", assets / "textures" / "entity" / f"{spirit}.png")
    # The foliot, Occultism's working spirit: a wolf underneath (a creature that stays on Peaceful),
    # with the foliot's attributes (`FoliotEntity.createAttributes`); the native mod gives it its job,
    # its goals and its look.
    write_json(data / "horizon" / "entity_type" / "foliot.json", {"template": "minecraft:wolf", "attributes": {
        "minecraft:max_health": 20.0, "minecraft:attack_damage": 1.0, "minecraft:movement_speed": 0.30000001192092896,
        "minecraft:armor": 2.0, "minecraft:armor_toughness": 1.0, "minecraft:follow_range": 50.0}})
    # Possessed creatures: their drops and names (the native mod summons the vanilla creature, with
    # these drops: spirits.rs).
    for name in POSSESSED + WILD:
        write_json(data / "loot_table" / "entities" / f"{name}.json",
                   loot_26(json.loads((occ_data / MOD / "loot_table" / "entities" / f"{name}.json").read_text())))
        lang_of(f"entity.{MOD}.{name}")
    for name in WILD_FOLLOWERS:
        write_json(data / "loot_table" / "entities" / f"{name}.json", {"type": "minecraft:entity", "pools": []})
        lang_of(f"entity.{MOD}.{name}")
    # The butcher knives' extra drops (Occultism's `add_item` loot modifiers: tallow from animals,
    # heads and echo dust with the iesnium knife), for the native mod (butcher.rs): the knives, the
    # kinds killed (Occultism's possessed ones by name), the chance and the item.
    knife_drops = []
    for f in sorted((occ_data / MOD / "loot_modifiers").glob("*.json")):
        j = json.loads(f.read_text())
        knives, kinds, chance = None, None, 1.0
        for c in j.get("conditions", []):
            if c["condition"] == "minecraft:random_chance":
                chance = c["chance"]
            elif c["condition"] == "minecraft:entity_properties" and c["entity"] == "attacker":
                held = c["predicate"]["equipment"]["mainhand"]["items"]
                knives = tags.resolve(held[1:]) if held.startswith("#") else [held]
            elif c["condition"] == "minecraft:entity_properties" and c["entity"] == "this":
                kind = c["predicate"]["type"]
                kinds = entity_tag(occ_data, kind[1:]) if kind.startswith("#") else [kind]
            else:
                knives = None  # a block's drops (datura seeds: fire.rs), or a condition not read here
                break
        if j["type"] != f"{MOD}:add_item" or not knives or not kinds:
            continue
        kinds = [k for k in kinds if k.startswith("minecraft:") or k.split(":")[1] in POSSESSED + WILD]
        if kinds and tags.known(j["item"]):
            knife_drops.append({"knives": knives, "kinds": kinds, "chance": chance, "item": j["item"], "count": j.get("count", 1)})
    write_json(data / "knife_drops.json", knife_drops)
    for name, attributes in WORKERS.items():
        write_json(data / "horizon" / "entity_type" / f"{name}.json", {"template": "minecraft:wolf", "attributes": attributes})
        lang_of(f"entity.{MOD}.{name}")
    # Every book of binding carries its spirit's name, and says it.
    for key in sorted(lang_src):
        book = key.removeprefix(f"item.{MOD}.").removesuffix(".tooltip")
        if book.startswith("book_of_binding_bound_") and key.endswith(".tooltip") and resolves(f"{MOD}:{book}"):
            lang[key] = lang_src[key]
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
    # Occultism's entries in vanilla's item and block tags (the tool kinds enchantments go by, armour
    # slots, slabs, walls, horse food, beacon payment...), for what this port has, merged into any
    # this port writes itself.
    blocks = {f"{MOD}:{f.stem}" for f in (data / "horizon" / "block").glob("*.json")}
    for kind, have in (("item", items), ("block", blocks)):
        src_root = occ_data / "minecraft" / "tags" / kind
        for f in sorted(src_root.rglob("*.json")):
            def ours(entry):
                ident = entry["id"] if isinstance(entry, dict) else entry
                if not ident.startswith(("#" + MOD + ":", MOD + ":")):
                    return False
                if ident.startswith("#"):
                    return (data / "tags" / kind / f"{ident.split(':', 1)[1]}.json").exists()
                return ident in have
            # (Not trim materials: this port has no trim material data for them.)
            if f.stem == "trim_materials":
                continue
            wanted = [e for e in json.loads(f.read_text())["values"] if ours(e)]
            if not wanted:
                continue
            dest = out / "data" / "minecraft" / "tags" / kind / f.relative_to(src_root)
            values = json.loads(dest.read_text())["values"] if dest.exists() else []
            values += [e for e in wanted if e not in values]
            write_json(dest, {"values": values})
    write_json(data / "horizon" / "creative_tab" / "occultism.json", {
        "title": f"itemGroup.{MOD}.occultism", "icon": f"{MOD}:golden_sacrificial_bowl",
        "items": tab + [f"{MOD}:{d}_dust" for d in DUSTS] + [f"{MOD}:{m}" for m in MATERIALS + POSSESSED_DROPS]})
    lang[f"itemGroup.{MOD}.occultism"] = "Occultism"

    write_json(assets / "lang" / "en_us.json", dict(sorted(lang.items())))

    print(f"content mod written to {out}")
    # The Dictionary of Spirits, from the content just written (tools/build_book.py). Found by this
    # file's folder, since `python3 -I` (build.sh) leaves the script's folder off the path.
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    import build_book
    build_book.main(occ)
    print(f"  items: {len(items)} ({len(DUSTS)} dusts, {len(MATERIALS)} chalk materials, {len(CHALKS)} chalks and glyphs, ...)")
    print(f"  crushing recipes: {len(kept)} kept, {len(dropped)} need other mods' items")
    print(f"  Occultism recipes copied: {len(copied)}: " + ", ".join(copied))
    print(f"  spirit fire recipes: {len(fire_recipes)}: " + ", ".join(fire_recipes))
    print(f"  pentacles: {len(pentacles)}, rituals: {len(RITUALS)}")
    print("  kept: " + ", ".join(kept))


if __name__ == "__main__":
    main()
