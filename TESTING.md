# Demonic Altar: in-game test pass

Run in the instance the native build was built for (`horizon-launcher native-build` on the default home builds
into the **Horizon** instance) with `occultism.hmod` in its `horizon-mods/` and turned on, in a new creative world.
Each check says what Occultism itself does, so a pass means parity with the original crusher.

Give yourself the kit: `/give @s occultism:demonic_altar`, `/give @s occultism:crusher_core_foliot`
(and `_djinni`, `_afrit`, `_marid`), `/give @s minecraft:raw_iron 64`, `/give @s minecraft:hopper 4`,
`/give @s minecraft:chest 2`.

| # | Do | Expect |
|---|---|---|
| 1 | Place the altar, right-click it | A 176-wide screen: core slot left, input, an arrow, a 2x2 output, your inventory below; "Bind a spirit core" |
| 2 | Put raw iron in the input, no core | Nothing happens |
| 3 | Shift-click a Foliot core from your inventory | It lands in the core slot (one only); the label turns to "Crushing", "Tier: Foliot" |
| 4 | Watch a full cycle with Foliot | 400 ticks (20 s) per raw iron, 2 iron dust each |
| 5 | Swap in Djinni, Afrit, Marid | 200 ticks / 3 dust, 100 / 4, 61 / 6 |
| 6 | Put an iron ingot in with any core | 1 dust per ingot, whatever the tier (multiplier ignored) |
| 7 | Echo shard with Afrit, then Marid | Afrit does nothing; Marid makes echo dust (`min_tier` 4) |
| 8 | Fill the outputs by crushing a stack of raw iron with Marid (players can't put items in outputs) | When the next result no longer fits, the arrow fills and turns red ("Output full"); no raw iron is used; empty one slot and it finishes at once |
| 9 | Hopper into the top, chest under a hopper below; then a spare core into the top hopper | Ore goes into the input only, dust comes out the bottom; the bound core never leaves. A hopper-fed core binds if the core slot is empty, else passes to the output and out the bottom: it never jams the input |
| 10 | Try to put raw iron in the core slot, a core in the input, or anything in an output slot | Refused |
| 11 | Close the screen, look at the altar | The core floats over it, spinning fast while working, slow while idle |
| 12 | Hover a core in your inventory | "Job: Crusher", its tier, "Place in a Demonic Altar..." |
| 13 | Save and quit mid-crush, reopen | Same slots, progress continues |
| 14 | Break the altar | It drops itself, its core, input and outputs |
| 15 | Craft a Foliot core, then upgrade it to Djinni with iron, gold and copper dust | Both recipes appear and work |

## Rituals and iesnium

Kit: `occultism:chalk_glyph_white` (white chalk), `minecraft:candle` x4, `occultism:golden_sacrificial_bowl`,
`occultism:sacrificial_bowl` x4, `occultism:book_of_binding_bound_foliot`, one each of iron, gold, copper and
`occultism:silver_ingot`.

| # | Do | Expect |
|---|---|---|
| 16 | Draw glyphs with white chalk | Each lands flat with a random sign (1-14), facing you; the floor shows through; one chalk used per glyph |
| 17 | Right-click a glyph with the same chalk | Its sign changes; no chalk used |
| 18 | Break a glyph | Nothing drops |
| 19 | Draw the Summon Foliot circle (24 glyphs, 4 candles) around the golden bowl | |
| 20 | Right-click a sacrificial bowl with an item, then with an empty hand | One item goes in and shows in the bowl; it comes back out |
| 21 | Put the four ingots in four bowls within 8 blocks; right-click the golden bowl with the Foliot book | "The ritual has begun."; the book sits in the bowl, turning |
| 22 | Watch 40 s | An ingot vanishes every 8 s; at 40 s the golden bowl holds a Bound Foliot Crusher; "The ritual is complete." |
| 23 | Remove a glyph, then offer the book | "The circle around this bowl isn't complete."; you keep the book |
| 24 | Start again and break a glyph mid-ritual | "The ritual was interrupted."; the book stays in the golden bowl; no more ingots are taken |
| 25 | In survival, mine silver ore without and with `/effect give @s occultism:third_eye`, then wearing Otherworld Goggles | Raw silver only; about 1 in 5 also drops raw iesnium; about 2 in 5 with goggles. Silk touch gives the ore, no iesnium |
| 26 | Build the Summon Djinni, Afrit and Marid circles and Craft Foliot (purple chalk), with their ingredients; start each with its book (Foliot's for the lenses) | Each finishes on time (80, 120, 160, 60 s), its ingredients taken evenly, with its core (or Infused Lenses) in the golden bowl |

## Storage (prototype)

Kit, in creative: `occultism:storage_core`, `occultism:storage_actuator`, `occultism:storage_plate_tier1` to `4`.
Put items in through the Actuator's input row (or shift-click them from the inventory).

| # | Do | Expect |
|---|---|---|
| 27 | Place the Core, the Actuator on it, a tier 4 and a tier 1 plate beside them; open the Actuator | "0/704 types, 0k/4864k" |
| 28 | Put enchanted, damaged and named tools and a stack of cobblestone in the top row | They vanish into the store, each distinct tool its own type |
| 29 | Save and quit, reopen, click each tool on the grid | Each comes to your inventory with its damage, enchantments and name |
| 30 | Break the Core with items stored (survival, then creative) | It drops (creative: comes to you) as an item "Bound to <name>"; "<name> is dormant..."; the Actuator shows 0/0 |
| 30a | Place that Core elsewhere, with the Actuator on it | "<name> wakes."; every item is there, the plates around it set its capacity |
| 30b | In creative, place the bound Core a second time (creative placement keeps the item, so this is a copy) | "...already anchored elsewhere; this Core binds a new spirit." and an empty store; the original is untouched |
| 31 | With a very large store, run `/tick query` idle and while hoppers feed the Actuator | Time per tick doesn't grow with what's stored |

### Terminal

| # | Do | Expect |
|---|---|---|
| 32 | Open the Actuator of a store with a dozen kinds of items | Its spirit's name as the title; the grid sorted by count (equal stacks merged), counts as 999 / 1k / 239k, enchanted items glinting; "N/M types  total/limit" under it |
| 33 | Left-click an item on the grid, then right-click another | A full stack of the first comes to your inventory, one of the second; the grid follows at once |
| 34 | Click the search box and type `ingot`, then `@occultism` | Only ingots; then only the mod's items; the number found shows at the right. E doesn't close the screen while typing; Enter or a click elsewhere lets go |
| 35 | Click the sort button (Count, Name, Mod), scroll the grid with the wheel | The order changes; scrolling stops at the last row |
| 36 | Hover an item on the grid | Its name, the exact count ("239,481 stored"), its id |
| 37 | Hoppers feed the Actuator while you watch | Counts change in place, without the screen reopening |

### Recall ritual

Kit: purple chalk (the Craft Foliot circle), `occultism:golden_sacrificial_bowl`, `occultism:sacrificial_bowl` x4, `occultism:book_of_binding_bound_foliot`, an ender pearl, a spirit-attuned gem, a silver ingot, an amethyst shard.

| # | Do | Expect |
|---|---|---|
| 38 | Make a store, break its Core and destroy the item (`/kill @e[type=item]`); draw Craft Foliot with the four ingredients; offer the book | After 30 s, "<name> answers the call: its Core returns."; a Core bound to it on the golden bowl |
| 39 | Hold every Core of your stores and run it again | "No lost spirit answers..."; nothing drops |
| 40 | With two lost stores (and one placed, one another player's), run it | Both lost Cores drop, one message each; the others are left alone |
| 41 | Place an old Core of a recalled store (one kept in a chest) | "A recall moved <name> to another Core; this one binds a new spirit."; the recalled Core still wakes the store |

Note a failing row's number and what you saw; `.work/build.log` and the game's log help.
