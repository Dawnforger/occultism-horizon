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

Note a failing row's number and what you saw; `.work/build.log` and the game's log help.
