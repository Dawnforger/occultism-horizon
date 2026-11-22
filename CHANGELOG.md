# Changelog

All notable changes to the Occultism port for Horizon. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html): while the major version is 0, a minor
version adds features and a patch version fixes them. The version lives in `VERSION`;
`python3 tools/set_version.py X.Y.Z` writes it into every manifest.

## [Unreleased]

## [0.11.1-alpha.2] - 2026-11-21

0.11.1-alpha.1 under the Horizon registry's id for the port. The game itself is unchanged.

### Changed
- On Horizon the port's id is `occultism_horizon`, the one it is listed under in the launcher's
  registry (the registry's `occultism` is AdLf0208's separate port). Its blocks, items and files keep
  the `occultism` namespace, so worlds carry over. A world saved with the old id asks once, as it
  opens, about the missing `occultism` mod: open it, with a backup if you like, and everything is
  there. The two ports can't be turned on together. On Sunrise the id stays `occultism`.

## [0.11.1-alpha.1] - 2026-11-21

Two security fixes from the Horizon registry's review, and three drawing fixes.

### Security
- Store ids and soul-gem work areas read from items and worlds are checked, as in 0.9.0-alpha.4: a
  forged store id could make a save write outside the world, and a forged area size could stall the
  server.

### Fixed
- The block under (and beside) a dimensional mineshaft, storage Actuator or stable wormhole showed
  through: they were declared as full blocks, hiding their neighbours' faces, though their models
  don't fill the block. The Dictionary of Spirits' ritual pages no longer draw their last ingredient
  over the line saying what the ritual makes.
- The dimensional matrix over a storage Actuator drew most of its cubes' faces see-through or as bare
  outlines: each face now shows the texture's framed square, as Occultism's model maps it.

## [0.11.0-alpha.1] - 2026-11-20

Occultism's guide book, its circle previews and ritual satchels, and its smelter and crystallizer spirits.

### Added
- **The Dictionary of Spirits, Occultism's guide book**, on a screen of the port's own (Horizon has
  no Modonomicon). Its nine categories as tabs, each a map of its entries linked as Occultism's, in
  the advancement screen's frames; an entry opens as a book, two pages to a spread, with its text and
  links, item spotlights, Occultism's pictures, creatures, and its crafting, spirit fire, smelting
  and ritual recipes, whose ingredients cycle through what they take. Different from Occultism for
  now: every entry is open from the start (no unlocking as you go), and a page whose recipe or item
  this port doesn't have yet says so (231 of 946 pages).
- **Pentacle previews in the world.** A pentacle page's "Show in the world" shows the circle where
  you look, turned the way you face; a right-click places it, and what is missing stays drawn until
  the circle is complete.
- **Occultism's ritual satchels.** Right-click a placed preview with the apprentice ritual satchel
  to build the place you clicked, or with the second satchel to build the whole circle: glyphs drawn
  with your chalk (a use each), candles, bowls and the rest placed. Different from Occultism for now:
  the satchels build from your inventory, not their own, and don't erase glyphs.
- **Smelter and crystallizer spirits, as Occultism's**, in all four ranks: Summon Foliot, Djinni,
  Afrit or Marid Smelter and Crystallizer. Like the crushers, each picks up what its recipes take from
  the ground near it, works it in its hands and drops the result: a smelter cooks by vanilla's
  smelting, blasting, smoking and campfire recipes (in that order, as Occultism's), faster at each
  rank (1, 0.5, 0.1 and 0.01 times the recipe's time); a crystallizer by Occultism's crystallize
  recipes, at its rank's time and yield. A foliot smelter or crystallizer is drawn as Occultism's
  foliot worker.
- **Occultism's spirit campfire**, a campfire that doesn't burn, which the marid smelter's summoning
  takes, with Occultism's recipe and its spirit flame.

## [0.10.0-alpha.1] - 2026-11-20

Occultism's spirits as creatures, its possessions, its otherworld tree and all of its chalks.

### Added
- **Crusher spirits, as Occultism's.** Summon Foliot, Djinni, Afrit or Marid Crusher now summons the
  spirit itself, drawn in Occultism's look: it picks up what Occultism's crushing recipes take at its
  rank from the ground near it, crushes it in its hands at its rank's speed and yield, and drops the
  result at its feet, as Occultism's `CrusherJob` does. It stays by the bowl it was summoned on.
  Different from Occultism for now: no crunching sound or sparks, no item filter, and its progress
  on the stack in hand starts over when the world reloads.
- **Occultism's possessed creatures**, sixteen of them, from its possession rituals and their five
  circles (Possess Foliot, Djinni, Afrit, Marid and Unbound Afrit): the possessed bee, goat of mercy,
  zombified piglin, blaze, elder guardian, enderman, endermite, ghast, guardian, hoglin, phantom,
  shulker, skeleton, warden, weak shulker and witch. Each is the vanilla creature, possessed:
  Occultism's health, speed and strength, and its drops (cursed honey, cruelty essence, demonic meat
  and tallow among them, and Occultism's rich tables for the rest). Different from Occultism for now:
  the possessed bee calls no other bees to its aid, the goat dies without its lightning, the
  possessed piglin is neutral as a zombified piglin, not hostile, and the possessed blaze isn't immune
  to snowballs and water. Not yet: the possessed iesnium golem, villagers and random animals.
- **Occultism's pastes.** Gray paste and nature paste last 64 crafts: a recipe that takes one leaves
  it in the grid a use worn. With them, Occultism's 23 paste recipes (gray paste and a dust make the
  block or gem again; nature paste turns stone to mossy stone, clay to slime, a stick to bamboo).
  Different from Occultism for now: eating a paste uses it up instead of two of its uses.
- **Cursed honey and demonic meat** are Occultism's foods, with their effects.
- **Occultism's otherworld tree.** An oak sapling in spirit fire becomes an otherworld sapling, which
  grows Occultism's otherworld tree: otherworld logs and leaves. Its leaves drop otherworld saplings
  and pitaya, Occultism's otherworld fruit (the Third Eye when eaten, now and then a boon).
  Different from Occultism: the tree grows in the open, where Occultism's natural one looks like an
  oak to those without the Third Eye. The Summon Foliot Lumberjack ritual takes an otherworld
  sapling now, as Occultism's does, in place of the jungle sapling.
- **Occultism's chalk crafting, all sixteen colours.** Every chalk is made as in Occultism: impure
  chalk crafted from white impure chalk and the colour's own materials, then purified in spirit
  fire. New for this: crushed ice, packed ice, blue ice, calcite, end stone and
  blackstone (crushed by the crusher spirits), and witherite dust, research fragment dust, dragonyst
  dust, gray paste and nature paste (from Occultism's crafting rituals).

### Removed
- **The Demonic Altar and the four crusher cores**, the port's stand-in for crusher spirits before
  it could make working spirits. Altars and cores in existing worlds are gone with them: summon a
  crusher spirit instead.

### Changed
- The dye recipes that stood in for Occultism's chalks are gone.

### Fixed
- Loot tables copied from Occultism are written in 26.3's form: the unbound afrit's and marid's
  essence drops now follow Occultism's counts and Looting, which 26.3 had skipped in 1.21's form.

## [0.9.0-alpha.2] - 2026-11-20

The first release on both Horizon and Sunrise from the same source.

### Added
- **A Sunrise build.** The same port packs for [Sunrise](https://github.com/sunrise-org/sunrise), a fork
  of Horizon: `tools/make_sunrise.py` writes the Sunrise package from the Horizon one, and the few
  places their mod APIs differ (the menu screens, quick moves, containers, wormhole placement and
  a handful of small calls) go through `mod/src/platform/horizon.rs` and `sunrise/platform.rs`.
  Built for Sunrise Nightly 2026-11-20 (`9425651`). It was first released for Sunrise as
  `sunrise-v0.9.0-alpha.1` (0.9.0-alpha.1's features with the notices fix below).

### Changed
- The Horizon package no longer carries the Sunrise platform file (now `sunrise/platform.rs`, outside
  `mod/`), and the crushing recipes' own type is `CrushingRecipe`. The fit check read both as uses of
  the SDK, which kept the package to the one Nightly it was made for; it now fits Horizon's Nightlies
  of 2026-11-19 (`5ef7734`) and 2026-11-20 (`0a875da`) too.

### Fixed
- The package carries all of Occultism's notices: its third-party art credits
  (`NOTICE-occultism-third-party.txt`) and this port's `NOTICE.txt` beside Occultism's licence. The
  packager takes only root files named `LICENSE*`, `NOTICE*` and the like, so the `.md` notices
  were left out before.

## [0.9.0-alpha.1] - 2026-11-18

### Added
- **Soul gems, as Occultism's.** Right-click a creature with an empty soul gem and it goes into the
  gem whole, its name, health, colour, job and the rest with it; right-click a block and it comes
  out there, facing you. The fragile soul gem (Craft Foliot: iron, an egg, glass) breaks after
  letting its creature out; the soul gem (Craft Djinni: the fragile gem, copper, silver, gold, a
  diamond and three soul sand or soil) lasts. Players can't be captured, nor the Ender Dragon and
  the Wither (or what a mod tags `c:capturing_not_supported`); a full gem does nothing to a
  creature, so a villager's trades don't open. The trinity gem waits for the eldritch rituals.
- **The foliot janitor, Occultism's first working spirit.** Summon Foliot with a brush, a chest, a
  dispenser and a hopper, and the bound book's foliot comes to work, drawn in Occultism's janitor
  look; you get its Book of Calling. It picks up the items lying in its work area, gathers them a
  stack at a time and puts them in the container you choose; between loads it goes back to its base.
  Sneak-use the book on a block to set the base or the deposit (by its mode); use it in the air to
  change the mode, sneaking to change the work area (16, 32 or 64 blocks). Occultism opens a screen
  for these; this port's book says them in its tooltip. Sneak-click the spirit with another book to
  bind that one. Its work is saved with it and goes with it into a soul gem. Also new: Occultism's
  brush. Not yet: the janitor's item filter, and carrying a spirit in its book of calling.
- **The foliot transporter.** Summon Foliot with a minecart, a chest, a dispenser and a hopper: in
  Occultism's transporter look, it takes a stack at a time from one container and carries it to
  another. Its book's modes are Set Extract and Set Deposit; its tooltip says where each is. It
  doesn't open doors yet (Occultism's spirit does), nor take filters.

- **The foliot lumberjack.** Summon Foliot with four saplings and an axe (a jungle sapling stands
  in for Occultism's otherworld sapling, whose trees aren't in this port): in Occultism's lumberjack
  look, it fells the trees in its work area, nearest first, the whole trunk at once after 8 s of
  chopping; gathers their wood, saplings, sticks and fruit; takes them to its container; and plants
  a sapling where each tree stood. Its book sets its base and deposit, as the janitor's does.

- **The foliot farmer.** Summon Foliot with datura, wheat, a carrot, a potato and a hoe: in
  Occultism's farmer look, it walks to the nearest ripe crop on farmland in its work area,
  harvests every ripe crop within 2 blocks and plants each again, then takes the seeds and crops
  to its container. It works vanilla's crops and the datura.
- NeoForge's vanilla entries in the `c:crops` and `c:seeds` item tags (Horizon has no NeoForge
  to add them).

### Fixed
- A summoning's book of calling now reaches the caster's inventory; it was dropped by the bowl.
- A janitor or lumberjack no longer stops working for good over an item it can't reach: it leaves
  that item for 30 s, as Occultism's gives up when its path ends.

## [0.8.0-alpha.2] - 2026-11-18

### Changed
- The mod's description refers to Occultism's creator, KliKli, in neutral terms ("with their
  permission"). Nothing else changed.

## [0.8.0-alpha.1] - 2026-11-18

### Added
- **The terminal looks like Occultism's.** 260 wide, in Occultism's colours: a magenta top bar
  with the spirit's name, Occultism's search field sprite and its buttons (clear the search, sort
  by amount, name or mod, sort direction, rows shown: 1 to 9, 4 by default, taking effect when the
  terminal next opens); an item area of 11 columns on Occultism's own background; the store's
  capacity as Occultism's labels ("2% filled", "4% of types", the numbers in their tooltip). Its
  button tooltips are Occultism's own words.
- **A crafting grid in the terminal**, as Occultism's: a 3x3 grid under the item area of an
  Actuator's or a wormhole's terminal, kept in the block between openings. Taking the result
  refills each emptied cell from the store; a shift-click on a grid cell puts its stack in the
  store; the clear button puts the whole grid back in the store. A shift-click on the result crafts
  what the grid holds, which then refills, rather than all the store can make at once: Horizon
  crafts a mod grid's shift-click itself. An Accessor's terminal has no grid yet.

## [0.7.0-alpha.2] - 2026-11-18

### Changed
- The mod is named **Occultism**, and it credits Occultism's creator, KliKli
  (https://github.com/klikli-dev), in its description and authors, as the README and NOTICE do.
  0.7.0-alpha.1's file was named "Occultism for Horizon". Nothing else changed.

## [0.7.0-alpha.1] - 2026-11-18

### Changed
- **On Horizon's mod API.** The port is now built against Horizon's mod API (horizon-rust/horizon
  #17145), pinned to Nightly 2026-11-18 (`0b14f8a`), the first with a signed mod SDK, and
  horizon-mixins `e59da90`. It is one package again: `mod/` holds the manifest, the native code and
  the generated content, so the two files and the modpack of 0.5 and 0.6 go. Blocks and items are
  Parts (the altar, bowls, chalks, stores, wormholes, the mineshaft, spirit fire, the books of
  binding); the storage terminal talks over a registered payload (`occultism:store`); spirit fire
  turns what falls in on the next server tick; an unbound spirit's immunities answer Horizon's
  invulnerability check, as Occultism's `isInvulnerableTo` does. `mod/Cargo.toml` names the SDK by
  the path `cargo horizon-mod` fetches it to (`.horizon/sdk/`), the only form the launcher takes.
- Occultism's recipe types (`occultism:crushing`, `ritual`, `spirit_fire`, `miner`) are registered
  at startup, as Horizon now asks: it refuses every recipe while one file names an unknown type.
- What a player puts in a bowl by hand, and an offered activation item, go in after the player's
  tick: on Nightly 2026-11-18 what a block's click moves itself is undone, and a player's inventory
  reads only while their state is acting. An activation item goes into the golden bowl damage, name
  and all, instead of on the caster's next inventory tick.

### Fixed
- Silver ore generates again: 26.3 reads configured features from `worldgen/feature/`, not
  Occultism's 1.21 `worldgen/configured_feature/`.
- A shift-click in a terminal moves the whole stack into the store, its last item and a single tool
  or plate included (the hook can't empty a slot, so the stack moves after the player's tick).
- Armour offered to a golden bowl (the repair ritual) goes into the bowl instead of being put on:
  clicks on a bowl don't fall through to the item's own use, on the client as on the server.
- Sneak-using a Storage Accessor on an Actuator only links it; it no longer opens its terminal too.
- The core over a Demonic Altar is lit again: a client reads no level light on this Nightly, so it
  is drawn in full sky light with the altar's own.

## [0.6.0-alpha.1] - 2026-11-14

### Added
- **Spirit names, as Occultism's.** A bound book of binding gets its spirit's name when it is first
  carried, and says it ("Can be used to summon the djinni …"). The rituals that bind the book's
  spirit into what they make (the dimensional matrix, the Storage Accessor) pass the name on, and the
  result says it too (`CraftWithSpiritNameRitual`).
- **Hoppers pull from a store.** A hopper under an Actuator or a stable wormhole takes the store's
  items out one by one, as a hopper empties Occultism's storage controller. While it pulls, one item
  waits in the block's bottom slot; when the hopper stops (locked, full, gone), that item goes back.
- **Unbound spirits, as Occultism's.** The Summon Unbound Afrit ritual (sacrifice a cow) and the
  Summon Unbound Marid ritual (use a trident near the bowl) summon a hostile spirit on the golden
  bowl, named for its book, with Occultism's health, armour, damage and speed. When mobs spawn, a
  difficulty's worth of blazes (afrit) or guardians (marid) come with it. Killing one drops its
  essence: afrit essence and marid essence are new items. An afrit takes no fire damage, a marid no
  fire, drowning, freezing, explosion or magic damage; neither burns in the sun or drowns into
  anything else. Rituals can now wait for an item to be used near the bowl, as Occultism's do.
- **The spirits look like Occultism's.** The unbound afrit and marid are drawn from Occultism's own
  GeckoLib models and textures, turned into vanilla model layers when the game starts, and play its
  idle and walk animations; their heads follow where they look. However one comes (a ritual,
  `/summon`), it doesn't burn in the sun.
- **What the essences make, as Occultism's.** Stabilizer plates 3 (Craft Afrit circle) and 4 (Craft
  Marid circle, a new pentacle), the Afrit and Marid miners, and red and blue impure chalk, which
  spirit fire purifies into red and blue chalk. Every store plate and every miner now has a survival
  recipe.

### Known gaps
- Occultism's possessed mobs, wild hordes, familiars and working spirits aren't in yet. Horizon's mod
  mob types copy only a zombie, a villager or a wandering trader, so a possessed enderman or a
  familiar cat can't behave as Occultism's does yet; working spirits are a larger piece of their own.
- The afrit's glowing parts (Occultism's glow layer) aren't drawn, and the spirits have no attack
  animation; their hitbox is a zombie's (Occultism's spirits are 1 wide and 2.4 tall).
- A spirit's helpers (blazes, guardians) come only when `spawn_mobs` is on, as Occultism's.

## [0.5.0-alpha.2] - 2026-11-14

### Changed
- **Two files instead of one, for today's launchers.** Launcher 0.2.63 and 0.2.64 build a combined
  `.hmod` without its content (horizon-rust/horizon#15933), so this release ships the halves apart:
  `occultism-0.5.0-alpha.2.hmod`, the content only (id `occultism`), and
  `occultism_native-0.5.0-alpha.2.hnmod`, the native code only (id `occultism_native`: two registry
  entries can't share an id, and the content has to keep its namespace's). The modpack
  `occultism_pack-0.5.0-alpha.2.hpack` installs both, once the registry lists them.
  `python3 tools/pack_hmod.py --split` packs all three; without `--split` it still packs one
  combined file. Nothing in the game changed since 0.5.0-alpha.1.

## [0.5.0-alpha.1] - 2026-11-13

### Added
- **Storage is obtainable in survival, as Occultism makes it.** Otherstone slabs (a block), and the
  otherstone pedestal and frame crafted from them; the storage stabilizer base, crafted from a
  pedestal; the Storage Actuator crafted from a dimensional matrix over a storage controller base, as
  Occultism's controller is; the otherworldly tablet and the inert Storage Accessor. Occultism's
  rituals make the rest: the controller base, stabilizer plates 1 and 2 and the Stable Wormhole in the
  Craft Foliot circle, the dimensional matrix, the Storage Accessor and the Dimensional Mineshaft in
  the Craft Djinni circle. Occultism's stabilizers are this port's plates, and its storage controller
  this port's Actuator (`ALIASES` in `tools/build_content.py`). The parts are items, not placeable
  blocks.
- **Repair rituals, as Occultism's.** A worn chalk in the Craft Djinni circle, or a worn tool, armour
  piece or miner in the Craft Afrit circle, with three otherworld essence and a spirit attuned gem,
  comes back whole, its enchantments and name kept. The golden bowl takes the worn stack itself, so
  the ritual starts on the caster's next tick (Horizon's native API hands a mod the held stack only
  there). Offering armour to the bowl doesn't put it on.
- **Sacrifice rituals, as Occultism's.** Thirteen of Occultism's rituals that ask for a sacrifice
  and make something this port has: a bee nest, a bell, budding amethyst, copper, iron, golden and
  diamond horse armour and nautilus armour, reinforced deepslate and the wild armour trim, in the
  Contact Wild Spirit circle (and the Contact Eldritch Spirit pentacle). A ritual's time doesn't run
  until a player kills one of its animals within 8 blocks of the golden bowl; the caster is told
  which. The animals are Occultism's entity tags, resolved when the content is built.

### Fixed
- A golden bowl starts a ritual for any stack its activation item matches, as Occultism's: a dyed,
  named or worn one goes in itself (a leather horse armour, which Horizon gives components, started
  nothing).

### Known gaps
- Stabilizer plates 3 and 4 and the Afrit and Marid miners still have no survival recipe: Occultism's
  need afrit and marid essence, which come from summoning and killing an unbound Afrit or Marid.
- The rituals that write a spirit's name on their result (the matrix, the Accessor) don't name it yet.
- The rituals that summon a spirit, a familiar or a possessed mob, and those that ask for an item to
  be used, need entities this port doesn't have yet. A kill by a dispenser's arrow counts as a
  player's, since Horizon doesn't tell them apart.
- Offering armour to a golden bowl still shows it worn on the caster's screen until the inventory is
  next sent (rejoining, for one); the server never equips it.

## [0.4.0-alpha.1] - 2026-11-13

### Changed
- **One file.** The release is one `.hmod` with the content and the native code
  (`tools/pack_hmod.py`, Horizon's `docs/api/mod-files.md`), installable from the launcher's Mods page
  or from this repository's release with `horizon-launcher mods add-github`.
- Built on Horizon Nightly 2026-11-13 (`9f42064`) and launcher 0.2.63: the launchers since 0.2.62
  can't build native mods on the older Nightlies (horizon-rust/horizon#15764).

### Added
- **Demon's Dream (datura).** Its seeds drop from grass, ferns and bushes 2% of the time (not
  with shears), as Occultism's; they grow on farmland into Occultism's crop, which bone meal
  speeds, and give its fruit when ripe. Nine fruit make Demon's Dream Essence, and seeds and a
  book the Dictionary of Spirits, by Occultism's recipes. The crop is the seeds' own block
  (`occultism:datura_seeds`, Occultism's is `occultism:datura`): Horizon's content items can't
  place a block of another name.
- **Spirit fire.** Flint and steel (or a fire charge) used by dropped Demon's Dream fruit lights
  spirit fire and uses the fruit up, as in Occultism. An item thrown in turns, the whole stack, by
  Occultism's spirit fire recipes: andesite to otherstone, diorite to otherrock, a diamond to a
  spirit-attuned gem (a diamond block to its block), a book to a taboo book, black dye to purified
  ink, a feather to an awakened feather, a book of binding back to an empty one, Demon's Dream
  Essence to Otherworld Essence. It doesn't spread, burn items or go out.
- **Otherworld wood without Occultism's hidden trees.** An oak log chopped by a player with the
  Third Eye is an otherworld log 20% of the time (40% wearing Otherworld Goggles), as iesnium
  comes from silver ore; Occultism's otherworld trees look like oaks to the unaided eye. Otherworld
  logs make otherplanks (Occultism's recipe) and, in spirit fire, otherworld ashes.
- **Occultism's chalk.** Otherstone and otherrock smelt to their polished forms and again to burnt
  otherstone and otherrock; three of those and three otherworld ashes make impure chalk, which
  spirit fire purifies into a chalk. Yellow and cyan impure chalk come from Occultism's own recipes
  too; the other colours, whose recipes need spirit essences, coral or witherite, are dyed from a
  white chalk (the chalk and a dye).
- **Chalk lasts, as Occultism's.** A chalk is an item of its own again (`occultism:chalk_white` and
  the other colours, Occultism's ids) that draws 256 glyphs, one durability each, and breaks after
  the last; creative mode doesn't wear it, and drawing over a glyph of its own colour turns that
  glyph's sign for free. It draws on a sturdy top face, or into a replaceable block such as grass
  or another colour's glyph. Glyph blocks have no item any more. The click is claimed in
  `use_item_on`, the glyph set by `/setblock` at the end of the tick and the chalk worn on its next
  inventory tick: Horizon's native API can't place a block by name from a player's hook.
- Otherstone, otherrock, the spirit-attuned block, taboo book, purified ink, awakened feather,
  Otherworld Essence and the books of binding (empty and for each rank), with Occultism's recipes.

### Changed
- White chalk is no longer crafted from calcite, bone meal and clay: it is Occultism's, above.
- Chalk items from before (`occultism:chalk_glyph_<colour>`) are gone from inventories: the glyphs
  stay, and new chalks are crafted as above.
- Builds need launcher 0.2.61 or older: 0.2.62's generated game wrapper needs an allocator neither
  this pin nor the newest Nightly has (horizon-rust/horizon#15764).
- Drawing over a glyph with its own chalk turns its sign by one again (Horizon's second, off-hand
  right-click turned it twice).
- **Stand-ins replaced by Occultism's recipes:** the spirit-attuned gem comes from spirit fire;
  sacrificial bowls and the lens frame take otherstone again; Demon's Dream Essence is made from
  datura; a book of binding is crafted from a taboo book, purified ink and an awakened feather and
  dyed for its rank. Binding it takes the Dictionary of Spirits, which stays in the grid (Occultism
  binds through the Dictionary too; the spirit's name isn't written on the book yet).
- `build.sh` reinstalls `cargo-horizon-mod` when the Horizon pin moves: an older checker refuses
  newer content fields.

## [0.3.0-alpha.1] - 2026-11-11

A work-in-progress alpha: storage, the mineshaft and their gates are playable and tested in game,
but not yet balanced or finished. Pipes can feed a store but not pull from it yet.

### Added
- **Dimensional storage** (`docs/storage-design.md`), built so a late-game store stays fast:
  Occultism's Dimensional Storage Actuator, with its dimensional matrix turning above it, holds a
  store with Occultism's capacities. Items are kept by exact identity (damage, enchantments,
  names), found by hash, and saved to a file of their own by appending only what changed.
- **Portable stores.** A store is a lasting network named for a spirit (Occultism's names): a
  broken Actuator drops as an item bound to it, however much it holds, and the store sleeps until
  the Actuator is placed again, anywhere, with nothing to rebind. A copy of a placed Actuator can't
  take its store over. It always drops, resists explosions, and its item doesn't burn.
- **Stabilizer plates are upgrades kept by the store:** six upgrade slots on the terminal's right
  (shift-click a plate in, click to take it out). They move with the Actuator, never drop, and come
  back with a recall.
- **The projection shows the upgrades:** one chalk glyph per plate circles the matrix like an
  electron, in its tier's colour (white, yellow, purple, red), always facing the viewer, with a
  faint glow and a short trail of fading motes.
- **The storage terminal.** The Actuator's screen shows the whole store as a grid under its spirit's
  name: search (`@mod` for a mod), sort by count, name or mod, scroll, compact counts with the exact
  one in the tooltip. Clicking an item gives you a stack (right-click: one); shift-click items from
  your inventory in. The server sends one snapshot when you open it, then only what changed;
  searching and sorting happen on your client.
- **Recall ritual** (Craft Foliot circle, Foliot book; ender pearl, spirit-attuned gem, silver,
  amethyst; 30 s): every store of yours whose Actuator is lost (not placed, and not in your
  inventory) gets a new one, dropped on the golden bowl. Older copies of a recalled store bind a new
  spirit instead of reaching it.
- **Store ownership.** Only a store's owner and players they trust (`/occultism trust`,
  `untrust`, `trusted`) open its terminal, break its Actuator or wake it; operators may break any.
  An Actuator placed by a stranger does nothing, and the store stays dormant for its owner to recall.
- **Dimensional Mineshaft and miner spirits.** Occultism's five miners in their magic lamps
  (Foliot, Djinni, Afrit, Marid, Ancient Eldritch) with its mining times, rolls, multipliers and
  durability; its 89 miner recipes, their ore tags resolved by the game so a modpack's ores join
  in. The deep miners (Afrit and up) dig raw iesnium where Occultism's dig iesnium ore, as often
  (there is no iesnium ore here). Results go into a container below, else nine outputs, which
  empty into a container placed below later; redstone stops it. The magic lamp
  and iesnium pickaxe are crafted as in Occultism, and the Foliot and Djinni miners bound in
  their rituals (the Craft Djinni circle is new).
- **Stable Wormhole.** Linked to a store by sneak-using it on its Actuator, it is that store's
  terminal and hopper face anywhere, while the store is awake. Sneaking places it against any
  block, facing away from the clicked face, as Occultism's.
- **Storage Accessor.** Linked the same way, it opens the store's terminal from the hand,
  anywhere, as a screen of its own (no inventory: taking only).
- The port is MIT licensed, like Occultism (`LICENSE`; Occultism's notice is in `NOTICE`).
- The content mod ships Occultism's `about_isabella.txt`, the CC BY 3.0 credit its third-party
  notices point to for textures based on Bonemouse's Isabella pack.

### Changed
- Pinned to Horizon Nightly 2026-11-04 (`6475f62`), which needs launcher 0.2.41 or later. The mod
  reads `Ingredient`'s named `order` field (it was a tuple at `0f0fa62`).
- Storage is one block, not Occultism's Storage Controller multiblock with stabilizers around it:
  the plates are upgrades instead, so a store moves in one piece.

### Fixed
- Taking the last of a type into a full inventory from a full store could lose what didn't fit.
- Stores and their records are saved with the world, never between saves: a crash can no longer
  leave items both in a store and back in a player's inventory.

### Known gaps
- Pipes and hoppers can put items into an Actuator or wormhole but not take them out: Horizon has
  no item-handler hook for a whole store yet.
- Not tested in game yet: the miner rituals, a mineshaft stopped by redstone, two-player trust.

## [0.2.0] - 2026-11-02

### Added
- **Ritual circles.** Chalk in Occultism's 16 colours draws glyphs (a random sign of 14, facing the
  drawer; the same chalk over its own glyph draws the next sign). Sacrificial bowls hold one item,
  shown in the bowl. A golden sacrificial bowl starts a ritual when offered its binding book, if
  its pentacle is drawn around it (any rotation) and its ingredients are in bowls within 8 blocks;
  it takes one ingredient every duration / (ingredients + 1) seconds and leaves the result in the
  bowl. Breaking the circle or taking an ingredient away interrupts it (the book stays in the
  bowl). Messages for a missing circle, missing ingredients, start, end and interruption.
- **Rituals:** Occultism's crusher summonings (Foliot 40 s, Djinni 80 s, Afrit 120 s, Marid 160 s)
  bind spirit cores, with their own ingredients and pentacles; Occultism's infused lenses ritual.
- **Silver** (ore and deepslate ore with Occultism's generation, raw, ingot, nugget, dust, blocks)
  and **iesnium** (raw, ingot, nugget, dust, blocks) with Occultism's recipes and crushing.
- **Iesnium from silver ore:** there is no iesnium ore. A player with the Third Eye who mines silver
  ore (not silk-touched) gets raw iesnium 20% of the time, 40% wearing Otherworld Goggles; vein
  mining counts each block.
- **The Third Eye** effect; Demon's Dream Essence (drink: Third Eye 60 s, Hunger, and Occultism's
  chance effects); spirit-attuned gem and crystal; lenses, infused lenses, lens frame, Otherworld
  Goggles; binding books for the four ranks.

### Changed
- Spirit cores are bound in rituals; their crafting recipes are gone.
- Stand-ins for what this port doesn't have yet (spirit fire, otherstone, the Dictionary of
  Spirits): white chalk from calcite, bone meal and clay (16); coloured chalk from 8 white chalk and
  a dye (8); polished deepslate for otherstone in the sacrificial bowl and lens frame; a spirit-attuned
  gem from a diamond, an amethyst shard, lapis and glowstone; Demon's Dream Essence from a honey
  bottle, nether wart, glow berries and a fermented spider eye; a bound binding book from a book, an
  ink sac, a feather and the rank's dye.
- A chalk is its glyph's block item and is used up per glyph: Horizon's native API can't place a
  block by name, so vanilla's placement draws it.
- Versioning: `VERSION`, `tools/set_version.py` and this changelog; the earlier history rebuilt as
  one tagged commit per stage.

### Fixed (found while testing)
- Horizon asks the right-click hook once per hand: the off hand's call no longer takes back the item
  the main hand just put in a bowl.
- Glyphs are drawn cut-out (on a leaf-litter template), not on black squares, and drop nothing.
- Otherworld Goggles no longer remove a component leather helmets don't have (which kept the whole
  content mod from loading).

## [0.1.3] - 2026-11-02

### Fixed
- A harvested altar drops itself. Its loot table was in 1.21's format (`{"condition": ...}`),
  which 26.3 reads as an unknown condition, so the pool never rolled.

### Changed
- All 15 rows of `TESTING.md` pass in game (agent-driven run on Horizon `0f0fa62`).
- Horizon's `/loot ... mine` bug, found along the way, is reported upstream as
  horizon-rust/horizon#7424 (reproduced with Horizon's template mod only).

## [0.1.2] - 2026-11-02

### Fixed
- The rank label ("Tier: ...") no longer draws over the "Inventory" label.
- Players can no longer put a spirit core in the input slot.
- A core that a hopper pushes into the input binds into an empty core slot, or else passes to
  the output for the hopper below; it no longer jams the altar.
- The core floating over the altar is lit by the air above it instead of being drawn black.

### Changed
- `TESTING.md` rows 8-10 describe what a player can actually do (outputs are filled by
  crushing, cores are refused in the input).

## [0.1.1] - 2026-11-02

### Changed
- `build.sh` uses the Horizon launcher and its portable Rust when they aren't on `PATH`, and
  keeps Cargo's downloads in `.work/`.
- The native glue compiles cleanly against Horizon `0f0fa62` and builds into the game with the
  launcher's `native-build` (launcher 0.2.37 or later: 0.2.26 rejects the `[api]` table).

## [0.1.0] - 2026-11-02

### Added
- First proof of concept, from the cloud session: the Demonic Altar with Occultism's crusher
  spirits bound into four spirit cores (Foliot, Djinni, Afrit, Marid), Occultism's timings,
  tier gates and ingot rule; 15 unit tests of the rules; the content mod generated from an
  Occultism checkout; native glue (not yet compiled); the in-game test plan.

[Unreleased]: v0.11.1-alpha.2...HEAD
[0.11.1-alpha.2]: v0.11.1-alpha.1...v0.11.1-alpha.2
[0.11.1-alpha.1]: v0.11.0-alpha.1...v0.11.1-alpha.1
[0.11.0-alpha.1]: v0.10.0-alpha.1...v0.11.0-alpha.1
[0.10.0-alpha.1]: v0.9.0-alpha.2...v0.10.0-alpha.1
[0.9.0-alpha.2]: v0.9.0-alpha.1...v0.9.0-alpha.2
[0.9.0-alpha.1]: v0.8.0-alpha.2...v0.9.0-alpha.1
[0.8.0-alpha.2]: v0.8.0-alpha.1...v0.8.0-alpha.2
[0.8.0-alpha.1]: v0.7.0-alpha.2...v0.8.0-alpha.1
[0.7.0-alpha.2]: v0.7.0-alpha.1...v0.7.0-alpha.2
[0.7.0-alpha.1]: v0.6.0-alpha.1...v0.7.0-alpha.1
[0.6.0-alpha.1]: v0.5.0-alpha.2...v0.6.0-alpha.1
[0.5.0-alpha.2]: v0.5.0-alpha.1...v0.5.0-alpha.2
[0.5.0-alpha.1]: v0.4.0-alpha.1...v0.5.0-alpha.1
[0.4.0-alpha.1]: v0.3.0-alpha.1...v0.4.0-alpha.1
[0.3.0-alpha.1]: v0.2.0...v0.3.0-alpha.1
[0.2.0]: v0.1.3...v0.2.0
[0.1.3]: v0.1.2...v0.1.3
[0.1.2]: v0.1.1...v0.1.2
[0.1.1]: v0.1.0...v0.1.1
[0.1.0]: v0.1.0
