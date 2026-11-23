# Demonic Altar: in-game test pass

Run in the instance the native build was built for (`horizon-launcher native-build` on the default home builds
into the **Horizon** instance) with `occultism_horizon.hmod` in its `horizon-native-mods/` and turned on, in a new creative world.
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

Kit: `occultism:chalk_white` (white chalk), `minecraft:candle` x4, `occultism:golden_sacrificial_bowl`,
`occultism:sacrificial_bowl` x4, `occultism:book_of_binding_bound_foliot`, one each of iron, gold, copper and
`occultism:silver_ingot`.

| # | Do | Expect |
|---|---|---|
| 16 | Draw glyphs with white chalk, in survival | Each lands flat with a random sign (1-14), facing you; the floor shows through; the chalk loses one durability per glyph (256 in all), none in creative |
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

Kit, in creative: `occultism:storage_actuator`, `occultism:storage_plate_tier1` to `4`.
Put items in by shift-clicking them from the inventory on the Actuator's screen (or by hoppers).

| # | Do | Expect |
|---|---|---|
| 27 | Place the Actuator | Occultism's pedestal, with the dimensional matrix turning above it; "A spirit named <name> now keeps this store." |
| 28 | Open it, shift-click a tier 4 and a tier 1 plate | They go into the upgrade column on the right; "0/704 types  0/4M"; a red and a white glyph circle the matrix |
| 29 | Fill all six slots (tiers 1 to 4), walk around the Actuator, look from above; then click a plate | One glyph per plate in its tier's colour (white, yellow, purple, red), each always facing you, on crossing orbits, with a faint glow and a trail of motes in its colour (clearest at night); the clicked plate comes back to your inventory and its glyph goes |
| 30 | Put enchanted, damaged and named tools and cobblestone in; save and quit, reopen, click each tool | Each its own type, and each comes back with its damage, enchantments and name |
| 30a | Break the Actuator with items and plates in it, place it elsewhere | It drops alone, "Bound to <name>"; nothing else drops; placed, "<name> wakes." with every item and plate there |
| 30b | In creative, place the bound Actuator a second time (creative placement keeps the item) | "...already anchored elsewhere; this Actuator binds a new spirit." and an empty store |
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
| 38 | Make a store with plates, break its Actuator and destroy the item (`/kill @e[type=item]`); draw Craft Foliot with the four ingredients; offer the book | After 30 s, "<name> answers the call: its Actuator returns."; an Actuator bound to it on the golden bowl, and placed, its plates and items are all there |
| 39 | Hold every Actuator of your stores and run it again | "No lost spirit answers..."; nothing drops |
| 40 | With two lost stores (and one placed, one another player's), run it | Both lost Actuators drop, one message each; the others are left alone |
| 41 | Place an old Actuator of a recalled store (one kept in a chest) | "A recall moved <name> to another Actuator; this one binds a new spirit."; the recalled Actuator still wakes the store |

### Ownership and crashes

Needs a second player (LAN or a server) for 42-45; the host of a single-player world is an operator.

| # | Do | Expect |
|---|---|---|
| 42 | As a second player, open the first player's Actuator, then break its Actuator | "<name> does not answer you." both times; nothing opens or breaks |
| 43 | The owner opens the terminal and runs `/occultism trust <second>`, then `/occultism trusted` | "<second> may now use <name>."; the list names them; the second player now opens it and takes items |
| 44 | `/occultism untrust <second>` while they watch | Their grid stops updating; they can't take or reopen |
| 45 | Give the second player the owner's Actuator (break it as owner); they place it | "<name> does not answer you."; the store stays dormant; the owner's recall brings it back |
| 46 | Save, shift-click items into a store, kill the game process, start again | The items are back in your inventory and not in the store |
| 47 | Shift-click items in, save, kill the game, start again | The items are in the store and not in your inventory |

## Mineshaft, wormhole, Accessor

Kit, in creative: `occultism:dimensional_mineshaft`, `occultism:miner_foliot_unspecialized`, `occultism:miner_ancient_eldritch`,
`occultism:stable_wormhole`, `occultism:storage_remote`, a placed Actuator with items in it.

| # | Do | Expect |
|---|---|---|
| 48 | Place the mineshaft, shift-click the eldritch miner in | It mines every 16 ticks, 9 of each result (vanilla ores, silver); the arrow fills; the lamp's durability bar drops by one per operation |
| 49 | The Foliot miner instead, with a chest under the mineshaft | One result every 401 ticks, into the chest; the outputs stay empty |
| 50 | Fill the outputs (no chest), then put a chest under it | The arrow turns red, "Output full"; nothing is lost; with the chest, the outputs empty into it and it carries on |
| 50a | Run an Afrit, Marid or eldritch miner a long while (`/tick sprint`) | Now and then raw iesnium (about 1 roll in 450 with vanilla ores) |
| 51 | Power the mineshaft with redstone | It stops and starts over when the signal goes |
| 52 | Sneak-right-click the Actuator holding a wormhole, then an Accessor; then right-click it without sneaking | "Linked to <name>." each time; their tooltips say "Linked to <name>". Without sneaking the terminal opens |
| 52a | Sneak-place a wormhole against a chest and against another wormhole, on each face | It's placed, facing away from the clicked face; neither block opens |
| 53a | Stand a mineshaft with a miner on a linked wormhole | What it digs goes straight into the store; its outputs stay empty. With the store dormant, the wormhole's nine hidden slots fill, then the mineshaft's outputs, then it waits |
| 53 | Place the wormhole far away and open it | The store's terminal: take items, shift-click items in, upgrades; hoppers into it fill the store |
| 54 | Use the Accessor anywhere | The terminal over the world without your inventory: search, sort, scroll, a click takes a stack (right: one); Escape closes it |
| 55 | Pick up the Actuator, then open the wormhole and use the Accessor | "<name> is dormant: its Actuator isn't placed." for both; placed again, both work |
| 56 | Draw Craft Foliot with a magic lamp, iesnium pickaxe, copper and coal; offer the Foliot book. Then Craft Djinni with that miner, an iesnium pickaxe, an iron block, gold and lapis | A Foliot miner, then a Djinni ore miner, in the golden bowl |

## Demon's Dream and spirit fire

| # | Do | Expect |
|---|---|---|
| 57 | Break short grass, ferns or bushes by hand, a few hundred times; then with shears | Now and then (about 1 in 50) Demon's Dream Seeds; never with shears |
| 58 | Plant the seeds on farmland; bone meal one; wait | Occultism's crop, growing in its stages; bone meal speeds it |
| 59 | Break a ripe plant, and an unripe one | Ripe: Demon's Dream Fruit and 1 to 4 seeds. Unripe: one seed |
| 60 | Drop Demon's Dream Fruit on the ground, then use flint and steel on that block's top | Spirit fire instead of fire; every fruit within a block is used up. Without fruit: ordinary fire |
| 61 | Throw andesite, a diamond, 5 books, feathers and stone into the spirit fire | Otherstone, a spirit-attuned gem, 5 taboo books, awakened feathers; the stone stays stone |
| 62 | Craft a book of binding (taboo book, purified ink, awakened feather, the rank's dye), then it with the Dictionary of Spirits | A bound book of that rank; the Dictionary stays in the grid |
| 63 | Chop oak logs with the Third Eye (Demon's Dream Essence), then without | About 1 in 5 drops an otherworld log with it (2 in 5 with goggles); none without |
| 64 | Craft otherworld logs into planks; throw one into spirit fire | Four otherplanks; otherworld ashes |
| 65 | Smelt otherstone twice; craft three burnt otherstone and three otherworld ashes (two columns) | Polished, then burnt otherstone; a white impure chalk |
| 66 | Throw 2 white impure chalks into spirit fire | Two white chalks |
| 67 | Draw with a chalk on its last use (`/give @s occultism:chalk_red[minecraft:damage=255]`); draw into short grass | The glyph is drawn and the chalk breaks; a glyph takes the grass's place |
| 68 | Run the storage rituals (controller base, plates 1 and 2, Stable Wormhole in Craft Foliot; matrix, Storage Accessor, Dimensional Mineshaft in Craft Djinni) | Each finishes with its result in the golden bowl |
| 69 | Offer a worn chalk (`[minecraft:damage=100]`) to Craft Djinni's golden bowl with three otherworld essence and a spirit attuned gem in bowls | The ritual starts, the chalk leaves the hand; 10 s later it comes back whole |
| 70 | The same in Craft Afrit with a worn enchanted diamond pickaxe, then a worn iron chestplate | Both come back whole, the enchantment kept; the chestplate isn't put on (the server's count says one, unworn) |
| 71 | Contact Wild Spirit circle, four gold ingots in bowls, offer a leather horse armour; `/kill` an armadillo by the bowl, kill one 12 blocks away, a pig by it, one by a wolf, then one yourself by it | Told to sacrifice an armadillo; the ritual's time stays at 0 until your own kill nearby, then runs; golden horse armour after 206 s |
| 72 | The release's two files: `occultism-<version>.hmod` in `horizon-mods/`, `occultism_native-<version>.hnmod` unpacked and built with `native-build`; repeat 71 | Both load (`/occultism trusted` answers, Occultism's items give); the sacrifice ritual runs as in 71, its bowls taken about 41 s apart |
| 73 | Give yourself a bound djinni book; run Craft Dimensional Matrix with it | The book's tooltip names its djinni; the matrix that comes out names the same one |
| 74 | Put 32 cobblestone in an Actuator, a hopper under it into a chest; then a redstone block beside the hopper, then take it away | The chest gets all 32 and the store is empty; locked, the hopper takes nothing and nothing waits in the Actuator; unlocked, it pulls again |
| 75 | Summon Unbound Afrit circle: netherrack, iesnium ingot, flint and steel, gunpowder; offer a bound afrit book; kill a cow by the bowl | Told to sacrifice a cow; after 100 s an Unbound Afrit named for the book stands on the bowl (with blazes when `spawn_mobs` is on); at noon it doesn't burn; killing it drops afrit essence |
| 76 | `/damage` an unbound afrit and marid with in_fire, lava, drown, magic, explosion, then your own hit, a second apart | Afrit: no fire or lava damage, the rest hurt; marid: only your hit hurts |
| 77 | An unbound marid and a zombie in a water tank for a minute | The zombie becomes a drowned; the marid stays a marid |
| 78 | Summon Unbound Marid circle: conduit, prismarine crystals and shard, ghast tear; offer a bound marid book; use a trident nearby | Told to use a trident; its time runs only after the use; an Unbound Marid after 140 s |
| 79 | Craft Afrit circle: plate 2, a gold block, a totem of undying, a spirit attuned crystal, afrit essence; offer a bound afrit book | Stabilizer plate 3 in the bowl |
| 80 | `/summon` an unbound afrit and marid at noon, and watch one walk | Occultism's afrit (red and gold, a smoke tail) and marid (teal head, blue body), faces forward, not burning; arms and tails sway, faster when walking |

Note a failing row's number and what you saw; `.work/build.log` and the game's log help.

## Terminal and its crafting grid

Kit: an Actuator holding oak logs and a few other items, a linked wormhole and Accessor.

| # | Do | Expect |
|---|---|---|
| 81 | Open the Actuator | Occultism's layout and colours: a magenta top bar with the spirit's name, the search field and four buttons (x, #, v, 4), 11 columns by 4 rows, the crafting grid with its arrow and result, "N% filled" and "N% of types" under the result, the inventory in a panel of its own |
| 82 | Hover each button, the search field, the capacity labels, the clear button | Occultism's tooltips ("Clear search.", "Sort by amount.", "Sort ascending.", "Change rows quantity.", "Prefix @: Search mod id.", "Clear the crafting area."); the labels' tooltip has the numbers |
| 83 | Click the sort type, then the direction, then the rows button twice; close and reopen | Amount, name, mod; the order turns around; 6 rows once reopened, the choices kept |
| 84 | Put an oak log in the grid, take the planks | Four planks; the log is back in its cell from the store, which has one fewer |
| 85 | Shift-click the result | It crafts what the grid holds; the cell refills a tick later |
| 86 | Shift-click a grid cell, then press the clear button | The cell's stack goes into the store, then the rest of the grid; the store's count rises by each |
| 87 | Close and reopen with items in the grid | They are still there |
| 88 | Open a linked wormhole, then use the Accessor | The wormhole's terminal has the grid too; the Accessor's is the terminal without grid or inventory |
| 89 | Break the Actuator with something in its grid | The Actuator drops with its store, and what was in the grid drops beside it |

## Soul gems

| # | Do | Expect |
|---|---|---|
| 90 | Craft Foliot with an iron ingot, an egg and glass; then Craft Djinni with that gem, copper, silver, gold, a diamond and three soul sand | A fragile soul gem after 30 s, a soul gem after 120 s |
| 91 | Right-click a named, hurt cow, a lime sheep and a villager with a job with an empty soul gem; hover the gem | Each vanishes into it (a villager's trades don't open); "Soul Gem", "Contains a captured Cow." |
| 92 | Right-click a block with the full gem; check the creature | It comes out on the block, facing you, with its name, health, colour and job; the gem is empty ("Empty Soul Gem") |
| 93 | Click a villager with a full gem | Nothing happens, no trades |
| 94 | Capture with the fragile gem and release | The creature comes out and the gem breaks |
| 95 | Try a Wither, and another player (LAN) | "Soul gems cannot contain this type of being."; a player is never taken |
| 96 | Shear a sheep, use a lead or name tag on a creature | As in vanilla: only soul gems capture |
| 97 | Save and quit with a creature in a gem, reopen, release it | It comes out as it went in |

## Working spirits: the foliot janitor

Kit: a book of binding (Foliot), a brush, a chest, a dispenser, a hopper; the Summon Foliot circle.

| # | Do | Expect |
|---|---|---|
| 98 | Summon Foliot with the brush, a chest, a dispenser and a hopper; offer the bound book | After 40 s a foliot on the bowl, named for the book, in Occultism's janitor look; the caster gets "Book of Calling: Foliot Janitor" bound to it (dropped by the bowl if their inventory is full) |
| 99 | Use the book in the air; sneak-use it in the air | "Mode: Set Base Location" / "Mode: Set Deposit" in turn; "… will now monitor a work area of 32x32", then 64x64, then 16x16 |
| 100 | In Set Deposit, sneak-use the book on a chest | "… will now deposit into Chest from the side: …"; a plain use opens the chest |
| 101 | Drop several stacks in the work area, two of one kind | It picks up the nearest that fits its hand, gathers the same kind into one stack, puts each load in the chest, and comes back for the rest |
| 102 | In Set Base Location, sneak-use the book on a block away from it | "Set base for … to …"; it walks there and waits there between loads |
| 103 | Fill the chest, then drop more | It holds its load and tries again every 2 s; empty the chest and it goes on |
| 104 | Give it a bone; hurt it; listen | It isn't tamed, doesn't fight back and makes no wolf sounds; it has 20 health |
| 105 | Sneak-click it with a newly crafted book of calling (a bound foliot book and a brush) | "This spirit is now bound to this book."; the new book sets its base and deposit too |
| 106 | Save and quit, reopen | It keeps its base, deposit and work area, and what it held |
| 107 | Capture it in a soul gem and release it | It comes out with its name and its work; a book bound before must be bound again (it is a new creature) |

## Working spirits: the foliot transporter

Kit: a book of binding (Foliot), a minecart, a chest, a dispenser, a hopper; the Summon Foliot circle; two containers.

| # | Do | Expect |
|---|---|---|
| 108 | Summon Foliot with the minecart, a chest, a dispenser and a hopper; offer the bound book | A foliot in Occultism's transporter look (its bag); "Book of Calling: Foliot Transporter" in the caster's inventory |
| 109 | Use its book in the air | "Mode: Set Extract" / "Mode: Set Deposit" in turn |
| 110 | Sneak-use it on one container in Set Extract, another in Set Deposit; hover the book | "… will now extract from …" / "… will now deposit into …"; the tooltip says "Extracts from: x, y, z." and "Deposits to: x, y, z." |
| 111 | Fill the first with a few stacks, a tool among them | It carries them one stack at a time to the second, the tool too |
| 112 | Fill the second | It holds its stack and waits, as the janitor does |

## Working spirits: the foliot lumberjack

Kit: a book of binding (Foliot), a jungle, an oak, a birch and a spruce sapling, an axe; the Summon Foliot circle; a few trees on grass and a chest.

| # | Do | Expect |
|---|---|---|
| 113 | Summon Foliot with the four saplings and an axe (Occultism's otherworld sapling: a jungle sapling here); offer the bound book | A foliot in Occultism's lumberjack look; "Book of Calling: Foliot Lumberjack" in the caster's inventory |
| 114 | Sneak-use its book on the ground among the trees (Set Base Location), then on the chest (Set Deposit) | The base and deposit messages; it walks to the trees |
| 115 | Watch | Tree by tree, nearest first: it chops for 8 s, the whole trunk falls, it carries the logs to the chest |
| 116 | Wait for the leaves to decay | It gathers saplings, sticks and apples too, and plants a sapling where each tree stood |
| 117 | Leave an item where it can't walk (on the leaves) | It tries, gives up, and goes on with the trees |

## Working spirits: the foliot farmer

Kit: a book of binding (Foliot), datura (Demon's Dream fruit), wheat, a carrot, a potato, a hoe; the Summon Foliot circle; a watered farm with ripe and young crops, and a chest.

| # | Do | Expect |
|---|---|---|
| 118 | Summon Foliot with the datura, wheat, carrot, potato and a hoe; offer the bound book | A foliot in Occultism's farmer look (its straw hat); "Book of Calling: Foliot Farmer" in the caster's inventory |
| 119 | Sneak-use its book on the farm (Set Base Location), then on the chest (Set Deposit) | The base and deposit messages |
| 120 | Watch | It walks to the nearest ripe crop and harvests every ripe one within 2 blocks, wheat, carrots, potatoes and datura alike, planting each again young; young crops stay |
| 121 | Watch the chest | The seeds and crops go in, a stack at a time |

## Results: Nightly 2026-11-18 (`0b14f8a`), launcher 0.2.78

0.7.0-alpha.1 (before the version bump), agent-driven with `horizon-ctl` in a fresh world, 2026-11-18. Timings are
the game clock's, to within the 5 to 20 ticks the checks polled at.

| Rows | Result |
|---|---|
| 1-10, 13, 14 | Pass. Foliot 400 ticks / 2 dust, Djinni 204 / 3, Afrit 106 / 4, Marid 64 / 6; ingots 1 dust; echo dust only with Marid; output full holds; hoppers in and out, a fed core binds or passes through; wrong slots refused; progress survives a restart; the altar drops itself and its contents |
| 11 | Pass after a fix: the core was drawn black (no level light on the client) |
| 12 | Not checked: the test tool can't hover |
| 36 | Pass (seen in the 200,000-item check): its name, "4 stored", its id |
| 15 | Out of date: cores come from the summon rituals (row 26), not crafting |
| 16-24 | Pass. Glyph signs, facing, 1 durability per glyph; re-click free; no drop; incomplete circle and interrupt messages; book kept |
| 25 | Pass. 0 iesnium without the Third Eye; 20 in 100 with it; 115 in 260 with goggles (44%); silk touch the ore |
| 26 | Pass. Lenses 60 s, Djinni 79, Afrit 120, Marid 159, each with its result |
| 27-30b | Pass after a fix: a shift-click left a stack's last item behind (a single tool or plate couldn't go in). Plates, 0/704 types 0/4M, glyph orbits; tools keep damage, enchantments and names over a restart; dormant and wake messages; the second creative placement binds a new spirit |
| 31 | Pass. 200,000 unique items: 2.3 ms per tick idle, 2.4 ms with five hoppers feeding and one pulling, against 2.3 ms for an empty store (`docs/storage-design.md`) |
| 32-35, 37 | Pass. Sorted by count with stacks merged; stack and one taken; search, `@occultism`, E while typing; Count, Name, Mod; live counts under a hopper |
| 38-41 | Pass, single player. Recall 30 s, the Actuator dropped on the golden bowl; "No lost spirit answers"; two lost stores answer at once; an old Actuator binds a new spirit. The other player's store in 40 not checked |
| 42-45 | Not checked: need a second player |
| 46, 47 | Pass. Unsaved deposits come back to the inventory after a kill; saved ones stay in the store |
| 48-51 | Pass. Eldritch every 16 ticks, 9 each, 1 wear per operation; Foliot 408 ticks into a chest; output full waits without wear, a chest empties it; redstone stops it, it starts over |
| 50a | Pass. 9 raw iesnium in about 260 eldritch operations |
| 52 | Pass after a fix: sneak-linking an Accessor also opened its terminal |
| 52a, 53, 53a, 54, 55 | Pass. Sneak-placed against a chest and a wormhole, facing away; a hopper into a wormhole fills the store; a mineshaft on a wormhole fills the store, dormant fills the wormhole's slots then its own; the Accessor's terminal anywhere; dormant messages, both work again |
| 56, 68, 79 | Pass. All 13 Craft Foliot, Djinni, Afrit and Marid rituals on time with their results (miners, plates 1 to 4, wormhole, controller base, matrix, Accessor, mineshaft) |
| 57-62 | Pass. 35 datura seeds in 2,091 grass broken by hand (1.7%), none with shears; planting, bone meal, ripe and unripe drops; spirit fire over fruit, fire without; otherstone, gem, taboo books, awakened feathers, stone stays; book of binding and its binding, the Dictionary kept |
| 63 | Pass. 66 otherworld logs in 350 with the Third Eye (19%), 65 in 150 with goggles (43%), none without |
| 64-67 | Pass. Four otherplanks; an otherworld log to ashes; polished then burnt otherstone; impure chalk; two impure chalks to two chalks; a chalk's last use draws and breaks it; a glyph replaces grass |
| 69, 70 | Pass after a fix: a worn chestplate offered to the golden bowl was put on. Chalk, enchanted pickaxe and chestplate come back whole, the enchantment kept, the chestplate not worn |
| 71 | Pass. Told to sacrifice an armadillo; a kill 12 blocks away doesn't count, one by the bowl does; golden horse armour. The pig and wolf cases not checked |
| 72 | Out of date: one package now (`mod/`), installed into `horizon-native-mods/` and built with `native-build` |
| 73-78 | Pass. The matrix and Accessor carry the book's spirit name; the hopper pull, locked and unlocked; Unbound Afrit (cow sacrifice) named for its book, not burning at noon; immunities as Occultism's; the zombie drowns, the marid doesn't; Unbound Marid after the trident's use. Blazes not checked (`spawn_mobs` off) |
| 80 | Partly: both drawn from Occultism's models, standing on the floor; the arms' and tails' sway not checked |

Seen in passing: starting a ritual sometimes also says "A ritual is already under way here.", when
the click repeats after the ritual has begun.

Working spirits, the build after 0.8.0-alpha.2, agent-driven with `horizon-ctl` in the test world,
2026-11-18:

| Rows | Result |
|---|---|
| 98 | Pass. The brush crafts from three planks, two wool and a string. Named for the book, drawn from Occultism's janitor model; the caster's inventory was full, and the dropped book was the janitor's first pickup (dropped books now wait 2 s) |
| 99, 100, 102 | Pass. Modes and work area sizes as listed; the deposit set from the west face; the base on a smooth stone block 8 blocks off, walked to |
| 101 | Pass. 20 and 7 cobblestone as one load of 27, then logs, apples, bones and string, each into the chest |
| 104 | Pass after a fix: it had a wolf's 8 health. A bone neither tames it nor is eaten; its goals are float, look at player, look around and the three janitor goals. The sounds not checked by ear |
| 105, 106, 107 | Pass. Bound by sneak-click; deposit kept over a restart (it put in the apples it held); out of a soul gem with its name, it went back to its base and deposited |
| 103 | Pass. With the chest full it held 5 feathers; with one slot emptied they went in |
| 108 | Pass after a fix: the book of calling never reached the caster (a player's inventory takes items only in their own tick), and the janitor nearby picked it up. Now in the caster's inventory |
| 109-111 | Pass. 40 iron, 20 redstone and a diamond sword moved from a chest to a barrel in under 30 s |
| 110 (tooltip), 112 | Not checked |
| 113 | Pass; the "#minecraft:axes" ingredient an iron axe |
| 114 | Pass. The test world had a gap in the floor between the bowl and the trees, which it couldn't cross; filled |
| 118-121 | Pass after a fix: only the datura reached the chest (this port's `c:crops` and `c:seeds` had only Occultism's entries; NeoForge's vanilla ones are added now). 50 ripe crops harvested and replanted, the 8 young left; 17 wheat, 44 seeds, 58 potatoes, 54 carrots, 3 datura and 8 datura seeds in the chest. The test world's floor had holes around the chest; filled |
| 115-117 | Pass after a fix: an item it couldn't reach held it for good. Five oaks (one grown from its own sapling) felled, 25 logs, 3 apples, 4 saplings and 3 sticks in the chest; saplings planted on two stumps |

## Results: 0.9.0-alpha.2 on today's Nightlies (2026-11-20)

A short pass after the rename and the Sunrise platform move: the logic tests (70 pass), then the same
checks in game on each platform.

| Platform | Result |
|---|---|
| Horizon Nightly 2026-11-20 (`0a875da`), launcher 0.2.78 | Pass. `cargo horizon-mod fits`: made for `0b14f8a`, fits `5ef7734` and `0a875da`. Built in the instance on `0a875da`; Summon Foliot Crusher gave its crusher core in 40 s; Summon Foliot Janitor summoned one janitor and put its book of calling in the caster's inventory; the janitor picked up 5 cobblestone dropped near it |
| Sunrise Nightly 2026-11-20 (`9425651`), launcher 1.1.1 | Pass. Built with the Nightly's tool; `cargo sunrise-mod fits`: made for `9425651`. The same three checks as on Horizon; a foliot without a job left its stack alone. The first time rituals and a foliot's job ran on Sunrise |
| Both | Not rechecked this pass: the transporter, lumberjack and farmer, soul gems, storage, the altar's crushing and the screens (unchanged code, all checked on Horizon on 2026-11-18 and the screens on Sunrise on 2026-11-19) |

## Results: chalk crafting (2026-11-20, Horizon Nightly `0a875da`)

| Check | Result |
|---|---|
| Rituals: research fragment dust, gray paste, dragonyst dust, nature paste | Pass; each result in the golden bowl at the ritual's own length (60, 120, 240 and 60 s) |
| Ritual: witherite dust | Failed once while four rituals ran side by side (nothing after 241 s; cause not found), then passed alone: 3 witherite dust after 180 s |
| Crafting: black, light blue and gray impure chalk | Pass, at a crafting table from white impure chalk and Occultism's materials |
| Spirit fire: the three impure chalks | Pass; each became its chalk |
| Crushing with a djinni core: ice, calcite, end stone, blackstone | Pass; one crushed item each after 202 ticks |
| Not checked | the other eight impure chalks' crafting (the same kind of recipe, copied as is); packed and blue ice crushing |

## Results: crusher spirits (2026-11-20, Horizon Nightly `0a875da`)

| Check | Result |
|---|---|
| Summon Foliot Crusher, Summon Djinni Crusher | Pass; each summoned its spirit on the bowl, named, with no book; health 20 and 100 |
| Raw iron, 2 each, dropped beside them | Pass; both picked it up. The djinni crushed both in about 20 s (200 ticks each) into 6 iron dust (3 each); the foliot one at 400 ticks, then the second, 4 dust in all (2 each) |
| Dirt and ice beside the djinni | Pass; the ice crushed to crushed ice (the djinni's rank and up), the dirt left lying |
| Looks | The djinni drawn in Occultism's worker look, on its bowl; the foliot crusher's look not yet seen |
| Not checked | the afrit and marid crushers; reloading the world with a crusher at work; a crusher's own drops left lying for 3 s |

## Results: pastes and possessed creatures (2026-11-20, Horizon Nightly `0a875da`)

| Check | Result |
|---|---|
| Gray impure chalk from white impure chalk and gray paste | Pass; the paste stayed in the grid, 1 use worn |
| Obsidian from obsidian dust and the same paste | Pass; the paste at 2 uses worn |
| Mossy cobblestone from cobblestone and nature paste | Pass; the paste stayed |
| A gray paste on its 64th use | Pass; used up, the grid empty |
| Possess Zombified Piglin, a pig killed by the player by the bowl | Pass; a zombified piglin, named for the afrit's book, 80 health, Occultism's loot table; killed, it dropped demonic meat, tallow and rotten flesh |
| Possess Goat, a horse killed by the player | Pass; a goat of 1 health and no speed, named; one hit killed it, cruelty essence dropped |
| Possess Bee, a bee killed by the player | Pass; a bee of 40 health, named, with its loot table. Six bees with that table, killed, dropped six cursed honey |
| Open | The player's sword didn't kill the possessed bee (37 hits; it stayed at 3 health) and `/damage … by @s` left the sacrificed bee at 3.5 health, while `/damage` without a player killed it at once. The test world has a hive nearby, so the tool may have been hitting other bees; not yet understood |
| Not checked | eating cursed honey, demonic meat or a paste; the other 21 paste recipes |

## Results: the otherworld tree (2026-11-20, Horizon Nightly `0a875da`)

| Check | Result |
|---|---|
| `/place feature occultism:tree_otherworld` | Pass after a fix: 26.3's tree format (Occultism's file is 1.21's, which the game refused). Otherworld log trunk, otherworld leaves, drawn in Occultism's textures |
| An otherworld sapling on grass, bone meal | Pass; it grew the otherworld tree, not an oak |
| Its leaves broken 120 times by hand (`loot … mine`) | Pass after a fix: 26.3's loot form (the 1.21 form dropped nothing). 9 pitaya, 4 otherworld saplings, sticks |
| Magenta impure chalk from white impure chalk, dragonyst dust, pitaya, chorus fruit | Pass |
| Not checked | the oak sapling turning in spirit fire; eating pitaya; the lumberjack felling an otherworld tree |

## Results: more possessions (2026-11-20, Horizon Nightly `0a875da`)

| Check | Result |
|---|---|
| Possess Skeleton at noon, a chicken killed by the player | The skeleton came and killed the player (in survival), then burned in the sun, as a vanilla skeleton does |
| Possess Skeleton at midnight | Pass; 30 health, named; killed, it dropped arrows, a bone and a skeleton skull |
| Possess Endermite, an egg thrown by the bowl | Pass; the egg counted, the endermite came with its loot table |
| Not checked | the other eleven possessions (the same code, with their own attributes and tables) |

## Results: crushers again, and the whole of this round on Sunrise (2026-11-20)

| Platform | Check | Result |
|---|---|---|
| Horizon `0a875da` | Summon Afrit Crusher, Summon Marid Crusher side by side | The afrit came (200 health); the marid didn't, the bowl empty. Alone, the marid came (300 health). A ritual among several side by side has now twice given nothing (the witherite dust too): not yet understood |
| Horizon `0a875da` | One raw iron each | The afrit 4 iron dust, the marid 6, Occultism's yields |
| Sunrise `9425651` | `/place feature occultism:tree_otherworld`; an otherworld sapling and bone meal | Pass; both grew Occultism's tree |
| Sunrise `9425651` | Gray impure chalk with a gray paste | Pass; the paste stayed in the grid, a use worn |
| Sunrise `9425651` | Summon Djinni Crusher, one raw iron | Pass; the djinni (100 health) made 3 iron dust |
| Sunrise `9425651` | Possess Endermite, an egg thrown | Pass; the possessed endermite came with its loot table |

## Results: smelters and crystallizers (2026-11-20, Horizon Nightly `0a875da`)

| Check | Result |
|---|---|
| A spirit campfire placed | Pass; it stands lit, in Occultism's spirit flame, and the game runs on |
| Summon Djinni Smelter, two raw iron | Pass; 100 health; two iron ingots in under 14 s (100 ticks each). In the first try the player standing by took the iron, and an afrit crusher at the same bowl crushed it to dust: test spirits on their own |
| Summon Djinni Crystallizer, emerald dust | Pass; an emerald (the recipe ignores the rank's yield) |
| Summon Foliot Smelter | Pass; drawn as Occultism's foliot worker |
| Not checked | the afrit and marid smelters and crystallizers; the foliot crystallizer; the marid smelter's summoning with its spirit campfire |

## Results: the Dictionary of Spirits, previews and satchels (2026-11-20, Horizon Nightly `0a875da`)

| Check | Result |
|---|---|
| Right-click the Dictionary of Spirits | Pass after fixes: the book opens on Getting Started's map. Sprites are named by texture path (`gui/sprites/...`), and the book's pictures shrunk to 256 pixels, before they drew |
| The map | Pass; nine tabs with Occultism's category pictures, entries at Occultism's places with their links, frames gold or grey (not in the port) |
| An entry (First Ritual) | Pass; title, text with links, the Aviar's Circle drawn in chalk, candles and the bowl; pages turn; a ritual page shows its ingredients around the book that starts it, and what it summons with its time |
| Show in the world | Pass after a fix (the crosshair isn't read while the world draws: the view's ray is followed instead, 32 blocks): the circle follows the view; glyph marks and boxes for the candles and bowl |
| A right-click places it; the apprentice satchel on a place | Pass; one white glyph at the place clicked, the chalk worn a use |
| The second satchel | Pass; the whole circle: 24 white glyphs, 4 candles, the golden bowl; the chalk worn 24 uses; the preview gone once complete |
| Not checked | a ritual run on a satchel-built circle; turning the preview to other facings; entities on entity pages |
| Sunrise `9425651` | The book: map, tabs and pictures, an entry with a crafting page | Pass after a fix: Sunrise names GUI sprites by atlas id (no `gui/sprites/`), so the book tries both |
| Sunrise `9425651` | Show in the world, placed with a right-click, the second satchel | Pass; 24 white glyphs and the golden bowl |


## Results: 0.11.1-alpha.1 (2026-11-21)

The registry review's two security fixes (as in 0.9.0-alpha.4) and the three drawing fixes.

| Platform | Result |
|---|---|
| Logic tests | 64 pass (new: only well-formed store ids name store files) |
| Horizon Nightly 2026-11-21 (`8f83e5c`), launcher 0.2.78 | Pass. `cargo horizon-mod check`: `occultism 0.11.1-alpha.1`, native-mod; `fits`: fits `8f83e5c`. On a copy of the test world: an Actuator item carrying the forged store id `../../evil` bound a new store (`18ea1a3b39d0747e-0`), and no `evil` file was written; a hopper fed it 5 diamonds and the terminal showed them; the matrix drew whole cubes, and the floor under the Actuator and a mineshaft showed; the Dictionary of Spirits opened on its map; Craft Dimensional Matrix gave its matrix in 119.2 s (its first run said the circle wasn't complete: built just after a teleport, as a platform filled then didn't reach the client either; the second run passed) |
| Sunrise Nightly 2026-11-20 (`9425651`), launcher 1.1.1 | Pass. Built with the Nightly's tool; `cargo sunrise-mod fits`: made for `9425651`. An Actuator set with the forged block data `store:"../../evil"` took a new store id (`18ea1aa9f30e0e85-0`), whose files were saved in the world's `occultism/storage/`, and no `evil` file was written; a hopper fed it 5 diamonds; the matrix drew whole cubes. `sunrise-ctl interact` placed no block from the hand (not even oak planks), so the forged item itself wasn't placed on Sunrise |
| Both | Not tested in game: a forged soul-gem area size |

## Results: 0.11.1-alpha.2 (2026-11-21)

0.11.1-alpha.1 with the Horizon id `occultism_horizon` (Sunrise unchanged).

| Platform | Result |
|---|---|
| Logic tests | 64 pass |
| Horizon Nightly 2026-11-21 (`8f83e5c`), launcher 0.2.78 | Pass. `cargo horizon-mod check`: `occultism_horizon 0.11.1-alpha.2`, native-mod. The test world, last saved with `occultism` 0.11.0-alpha.1, asked about the missing `occultism` mod (code only; no blocks or content missing); opened with a backup, its Occultism blocks were all there; a new storage Actuator's terminal opened and drew as before, and its crafting grid passed `term_check.py`; Craft Dimensional Matrix gave its matrix in 119.0 s (its first run, just after a teleport, didn't start, as before) |
| Sunrise | Not rebuilt: `make_sunrise.py` writes the same `sunrise-mod.toml` as for 0.11.1-alpha.1 apart from the version |

## Results: 0.11.2-alpha.1 (2026-11-21)

The ritual satchel's server-side checks.

| Platform | Result |
|---|---|
| Logic tests | 65 pass (new: a satchel builds only within reach and inside the world border) |
| Horizon Nightly 2026-11-21 (`8f83e5c`), launcher 0.2.78 | Pass. Hedyrin's Lure shown in the world from the Dictionary of Spirits and placed; the second satchel clicked on it in adventure mode built nothing; the same click in survival built the whole circle (12 white and 4 yellow glyphs, 4 candles, the golden bowl). A forged far-away click wasn't sent from a real client: the reach rule is unit-tested |
| Sunrise | Packed and compiled with `cargo sunrise-mod build` for `9425651`; not played |

## Results: weather and time magic (2026-11-22, unreleased)

| Platform | Result |
|---|---|
| Logic tests | Pass, with a new one for the next dawn or nightfall and the clock's ramp |
| Horizon Nightly 2026-11-21 (`8f83e5c`) | Day Time cast at 6000 ticks: the clock went to 24000 (the next dawn) and the djinni was gone. Night Time from there: 37000 (the next nightfall). Clear Weather in rain: clear after, with a flash. Rain Weather, a cow killed by the player at the circle: raining about 70 s after the sacrifice, with a flash. Not run in game: Thunder Weather (the same code with thunder on) |
| Horizon `8f83e5c`: traders | Otherstone Trader: 6 stone dropped near it became 12 otherstone. Gambler: 10 diamonds became random gem trades (iesnium, silver, gold and iron nuggets, an amethyst shard), one picked up across a reload. Not seen in game: a trader going when its time is up (an hour or half an hour), the Otherrock Trader (the otherstone one's code) |
| Sunrise `9425651` | Compiles (`cargo check`); not played |
| Horizon `8f83e5c`: furnishings | Craft Dimensional Matrix on a fresh circle with red large candles for its candles and copper bowls for its ingredients: began, took the bowls' items, gave its matrix in 120 s. Every new block placed and drawn: the otherrock blocks, copper and silver bowls (upright), large candles in their colours, otherglass (nearly invisible, as its texture), the otherflower. A golden pitaya eaten at full hunger: the Third Eye and Health Boost V. Not checked: the recipes in a crafting table, lighting a large candle by hand |
| Horizon `8f83e5c`: particles, coloured fire | Summon Afrit: Rain on a fresh circle: gray smoke around the golden bowl while it waited for the cow; after the sacrifice, the purple portal swirl over the golden bowl and a spirit-fire flame over the bowl holding the next ingredient. Three spirit fires, two clicked with red and light blue chalk: `color=red`, `color=light_blue`, drawn in those colours beside the white one. Not seen: the flames over the golden bowl once all ingredients are taken |
| Horizon `8f83e5c`: wild rituals | Wild creeper horde, a pig sacrificed: 5 named, charged creepers with Occultism's armour and health; one ignited among the other four, which were all left whole. Wild Hunt, a villager sacrificed: a wither skeleton and 5 skeletons, four with stone swords and one with a bow; the leader took no damage from a skeleton's arrow and 10 from the player. Illager invasion: an evoker, 2 vindicators, 5 pillagers, an illusioner and a ravager. Wild random animals (an egg thrown): 7 named animals of the common kinds. Possess Random Animal (common): one more. Resurrect Allay, a vex sacrificed: an allay on the bowl. Across two reloads, two hunt skeletons were still spared each other's arrows and still hurt by the player. Not run in game: the drowned, husk, parched and silverfish hordes, the other animal kinds, villagers, the undead mounts |
| Sunrise `9425651` | Builds; not played |
| Horizon `8f83e5c`: trial breezes | The First Key (a snow golem sacrificed): a weak breeze with 3 zombies, 3 skeletons and 3 silverfish, all named. In the Chamber (a copper golem): a breeze of 150 health with 5 spiders, 5 strays and 5 cave spiders; 16 damage from the player took 1 with its 15 followers alive, the strays then glowing; killed, its 15 followers went at once. Glorious Vault (an iron golem): a strong breeze, 7 husks, 7 bogged and 7 slimes of sizes 1 to 7. The weak breeze and a zombie of its own were each invulnerable to the other's blows |
| Horizon `8f83e5c`: flaming paste | Used on stone: fire above it; on an unlit campfire: lit. A pig touched with it: burning (586 fire ticks after 10), a use worn (in rain its fire went out, as vanilla's does). Sneaking on stone: lava, 8 uses. With gray paste in the other hand: a small fireball, each paste worn. Eaten at no food: food 15, two uses worn, Fire Resistance and Strength II for 10 minutes. Not checked: its crafting recipes, TNT, a hit with it, the ritual |
| Sunrise `9425651` | Builds; not played |
| Horizon `8f83e5c`: equipment | Full silver armour: 12 armour, no toughness. Full infused: 20 armour, 12 toughness, 0.4 knockback resistance. Attack damage and speed held: silver sword 5 / 1.6, infused sword 7 / 2.1, butcher knife 6 / 1.6, iesnium knife 15 / 2.2, infused axe 9 / 1.5, silver spear 2 / 1.05, infused pickaxe 5 / 1.7. Infused armour drawn on the player, the spear in hand, every icon in the hotbar. Craft Infused Pickaxe from a silver pickaxe with Efficiency III, Unbreaking II, 50 damage and a custom name: an infused pickaxe with both enchantments and 50 damage, unnamed. Not checked: the crafting recipes in a table, the iesnium knife and armour rituals, a horse wearing the armour, mining speed |
| Sunrise `9425651` | Builds; not played |
| Logic tests | Pass, with a new one: an iesnium bowl finishes a ritual in a quarter of the ticks, taking every ingredient |
| Horizon `8f83e5c`: silver blocks and bowls | Every new silver block and bowl placed and drawn. The silver bulb lit on a redstone pulse and stayed lit when it ended. Craft Infused Pickaxe on an iesnium bowl, its ingredients in dark silver bowls: done in 609 ticks (a golden bowl's would be 2400), its pickaxe in the bowl. Not checked: the crafting and stonecutting recipes in game, the door and trapdoor opening, the iesnium bowl rituals |
| Sunrise `9425651` | Builds; not played |
| Horizon `8f83e5c`: knife drops, torches | A cow killed in two blows with the butcher knife: its leather and beef and 4 tallow. A spirit torch on the side of stone: a spirit wall torch facing south; on top of stone: a standing spirit torch; a spirit lantern on stone; two torches used of four. Drawn at night. The wall's stone removed: the wall torch dropped as a spirit torch. Not checked: the iesnium knife's heads and echo dust, the recipes in a crafting table, a torch's light level |
| Sunrise `9425651` | Builds; not played |
