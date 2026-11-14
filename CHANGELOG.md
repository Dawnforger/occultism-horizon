# Changelog

All notable changes to the Occultism port for Horizon. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html): while the major version is 0, a minor
version adds features and a patch version fixes them. The version lives in `VERSION`;
`python3 tools/set_version.py X.Y.Z` writes it into every manifest.

## [Unreleased]

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

[Unreleased]: v0.5.0-alpha.2...HEAD
[0.5.0-alpha.2]: v0.5.0-alpha.1...v0.5.0-alpha.2
[0.5.0-alpha.1]: v0.4.0-alpha.1...v0.5.0-alpha.1
[0.4.0-alpha.1]: v0.3.0-alpha.1...v0.4.0-alpha.1
[0.3.0-alpha.1]: v0.2.0...v0.3.0-alpha.1
[0.2.0]: v0.1.3...v0.2.0
[0.1.3]: v0.1.2...v0.1.3
[0.1.2]: v0.1.1...v0.1.2
[0.1.1]: v0.1.0...v0.1.1
[0.1.0]: v0.1.0
