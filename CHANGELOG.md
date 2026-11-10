# Changelog

All notable changes to the Occultism port for Horizon. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html): while the major version is 0, a minor
version adds features and a patch version fixes them. The version lives in `VERSION`;
`python3 tools/set_version.py X.Y.Z` writes it into every manifest.

## [Unreleased]

### Added
- **Storage prototype** (`docs/storage-design.md`), built so a late-game store stays fast: the Storage
  Core and Dimensional Storage Actuator form a multiblock, and plates (tiers 1 to 4, Occultism's
  capacities) in the sixteen places around them raise its limits. Items are kept by exact identity
  (damage, enchantments, names), found by hash, and saved to a file of their own by appending only
  what changed. Prototype only: creative items, a simple in/out menu, no recipes.
- **Portable Storage Cores.** A store is a lasting network named for a spirit (Occultism's names): a
  broken Core drops as an item bound to it, however much it holds, and the store sleeps until the
  Core is placed again, anywhere, with nothing to rebind. A copy of a placed Core can't take its
  store over. The Core always drops, resists explosions, and its item doesn't burn.
- **The storage terminal.** The Actuator's screen shows the whole store as a grid under its spirit's
  name: search (`@mod` for a mod), sort by count, name or mod, scroll, compact counts with the exact
  one in the tooltip. Clicking an item gives you a stack (right-click: one). The server sends one
  snapshot when you open it, then only what changed; searching and sorting happen on your client.
  The Actuator's output row and Take button are gone.
- **Recall ritual** (Craft Foliot circle, Foliot book; ender pearl, spirit-attuned gem, silver,
  amethyst; 30 s): every store of yours whose Core is lost (not placed, and not in your inventory)
  gets a new Core, dropped on the golden bowl. Older Cores of a recalled store bind a new spirit
  instead of reaching it. Stores from before owners belong to whoever next places their Core.
- **Store ownership.** Only a store's owner and players they trust (`/occultism trust`,
  `untrust`, `trusted`) open its terminal, break its Core or wake it with a Core; operators may
  break any Core. A Core placed by a stranger does nothing, and the store stays dormant for its
  owner to recall.

### Changed
- The terminal has no visible input row: shift-click items from your inventory into the store
  (hoppers still feed it).
- Stores and their records are saved with the world, never between saves: a crash can no longer
  leave items both in a store and back in a player's inventory.

### Fixed
- Taking the last of a type into a full inventory from a full store could lose what didn't fit.

### Changed
- Pinned to Horizon Nightly 2026-11-04 (`6475f62`), which needs launcher 0.2.41 or later. The mod
  reads `Ingredient`'s named `order` field (it was a tuple at `0f0fa62`).

### Added
- The port is MIT licensed, like Occultism (`LICENSE`; Occultism's notice is in `NOTICE`).
- The content mod ships Occultism's `about_isabella.txt`, the CC BY 3.0 credit its third-party
  notices point to for textures based on Bonemouse's Isabella pack.

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

[Unreleased]: v0.2.0...HEAD
[0.2.0]: v0.1.3...v0.2.0
[0.1.3]: v0.1.2...v0.1.3
[0.1.2]: v0.1.1...v0.1.2
[0.1.1]: v0.1.0...v0.1.1
[0.1.0]: v0.1.0
