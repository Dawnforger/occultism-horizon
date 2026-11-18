# Occultism

An unofficial port to [Horizon](https://playhorizon.dev) of [Occultism](https://github.com/klikli-dev/occultism),
the mod by KliKli ([klikli-dev](https://github.com/klikli-dev)), made with her permission. Please report problems with
the port here, not to KliKli.

Occultism's worker spirits, redesigned for Horizon: instead of a summoned entity walking around,
a spirit is bound into a **spirit core** that you place in a generic **Demonic Altar**. The core
decides what the altar does. This port binds Occultism's crusher in all four ranks, bound in
Occultism's own **ritual circles**: chalk glyphs drawn around a golden sacrificial bowl, ingredients
on sacrificial bowls, a binding book to start the ritual.

- **Same numbers as Occultism.** Recipes are Occultism's own `occultism:crushing` files; ranks use
  its config defaults (Foliot 400 ticks x2, Djinni 200 x3, Afrit 100 x4, Marid 61 x6 on raw iron),
  tier gates (`min_tier`/`max_tier`) and the ingot rule (`ignore_crushing_multiplier`).
  The timings were cross-checked against Java's float math.
- **A machine, not a mob.** Input and output buffers, hoppers in from the top and sides, out from
  the bottom; the core slot is unreachable by automation. A full output buffer holds the finished
  operation instead of dropping items on the ground.
- **Rituals as in Occultism.** Its pentacles (`summon_foliot` to `summon_marid`, `craft_foliot`),
  checked in all four rotations; its ritual recipes, durations and ingredient timing (one every
  duration / (ingredients + 1) seconds from bowls within 8 blocks); an interrupted ritual keeps its
  book in the bowl. The result lands in the golden bowl.
- **Iesnium without the hidden ore.** There is no iesnium ore to vein-mine by mistake: a player
  with the Third Eye finds raw iesnium in 20% of the silver ore they mine, 40% wearing Otherworld
  Goggles (`ritual.rs`), vein mining included. Deep miner spirits dig raw iesnium too, as rarely as
  Occultism's dig its ore.
- **The spirit is data.** A core's `occultism:spirit` component (`{job, tier}`) decides the job,
  so new jobs and ranks are new data and a new arm in `logic.rs`, not a new block.

## Install (alpha)

0.7.0-alpha.1 is a work-in-progress alpha: expect rough edges and keep backups of your worlds.

It is one file, `occultism-0.7.0-alpha.1.hmod`: the native code and the content (blocks, items,
recipes, textures) together, on Horizon's mod API (horizon-rust/horizon#17145). It is built for
Horizon Nightly 2026-11-18 (`0b14f8a`), the first with a signed mod SDK, with launcher 0.2.78 or newer.
It replaces 0.6's two files and modpack: remove `occultism` and `occultism_native` 0.6 first.

- **From this repository:**
  `horizon-launcher mods add-github https://github.com/Dawnforger/occultism-horizon/releases/tag/v0.7.0-alpha.1 --trust-native-code`,
  or download `occultism-0.7.0-alpha.1.hmod` from that release and install it from the launcher's
  Mods page (Install from file). Build the instance. GitHub installs aren't reviewed by the Horizon
  registry.
- **From the launcher's registry:** not yet; the registry is moving to this format
  (horizon-rust/horizon-mods#1117). 0.6.0-alpha.1 is still the version submitted there, for Nightly
  2026-11-13 (`9f42064`).

The launcher asks you to trust the native code before it builds it: it runs with your account's
permissions. `./build.sh` in the source builds the same from an Occultism checkout, with the logic
tests.

## Layout

| Path | What |
|---|---|
| `mod/` | The mod, one flat package: `horizon-mod.toml`, `Cargo.toml`, `src/` (the native code, a trait mod with Parts) and the generated `assets/` and `data/` |
| `mod/src/logic.rs` | The altar's rules, no Horizon types |
| `mod/src/lib.rs` | Registration and the global hooks: Parts, containers, the store's payload, ticks, deaths, spirits, loot, commands |
| `mod/src/altar.rs` | The Demonic Altar's Parts: its block entity, menu, cores, drawing |
| `mod/src/ritual.rs` | The rituals' rules, no Horizon types: pentacles, ingredients, timing, sacrifices, item uses, glyphs, iesnium |
| `mod/src/rites.rs` | Glue for chalk glyphs, sacrificial bowls, the golden bowl's rituals, spirit names, summoning, iesnium |
| `mod/src/spirits.rs` | The unbound afrit and marid: their helpers, immunities and sun-proofing |
| `mod/src/spirit_model.rs` | The unbound spirits drawn from Occultism's GeckoLib models and animations (`spirit_geo.rs`, generated) |
| `mod/src/storage.rs` | The storage's rules, no Horizon types: the store, its save log, capacity |
| `mod/src/network.rs` | A store as a lasting network, no Horizon types: its record and upgrades, what placing an Actuator does, spirit names, the glyphs' orbits |
| `mod/src/terminal.rs` | The storage terminal's rules, no Horizon types: its messages, the client's searched and sorted copy of a store |
| `mod/src/terminal_ui.rs` | The terminal's screen on the client: grid, search, sort, scrolling, tooltips |
| `mod/src/fire.rs` | Spirit fire and Demon's Dream, no Horizon types: seed drops, where it lights, what it turns items into |
| `mod/src/spirit_fire.rs` | Glue for spirit fire and the datura crop: lighting by dropped fruit, items in the fire, planting |
| `mod/src/mining.rs` | The dimensional mineshaft's rules, no Horizon types: timing, weighted rolls, batching, wear |
| `mod/src/mineshaft.rs` | Glue for the mineshaft: miner recipes resolved by the game, outputs, the container below, its screen |
| `mod/src/vault.rs` | Glue for the storage multiblock, and the one place that turns stacks into keys |
| `mod/src/game.rs` | What the glue asks of the game: items and blocks by id, this mod's components, moves in transactions |
| `docs/storage-design.md` | Why Occultism's storage lags, the performance rules, the structure, the Acolyte roadmap |
| `tests/logic/` | 69 unit tests of the rules, including scale tests, runnable anywhere (`cargo test`) |
| `tools/build_content.py` | Generates `mod/assets`, `mod/data`, `mod/horizon-mod.toml` and `mod/src/spirit_geo.rs` from an Occultism checkout: altar, cores, chalks and glyphs, bowls, silver and iesnium, rituals and pentacles, recipes, tags, Occultism's art |
| `build.sh` | Logic tests, content, `.hmod` pack and check, native build, in that order |
| `TESTING.md` | The in-game pass (altar, rituals, iesnium) |
| `CHANGELOG.md`, `VERSION` | What changed in each version; the current version (`tools/set_version.py` writes it everywhere) |

## Status

| Part | State |
|---|---|
| Rules (`logic.rs`, `ritual.rs`, `storage.rs`, `network.rs`, `terminal.rs`, `mining.rs`, `fire.rs`) | 69/69 tests pass; tier timings match a Java oracle |
| Content mod | Generated and self-checked; `cargo horizon-mod check` runs in `build.sh` |
| Glue (`lib.rs`, `rites.rs` and the rest) | 0.7.0-alpha.1 built on Horizon Nightly 2026-11-18 (`0b14f8a`) with launcher 0.2.78; 0.6.0-alpha.1 on Nightly 2026-11-13 (`9f42064`) |
| In-game (0.7.0) | Full pass of `TESTING.md` on Nightly 2026-11-18 (`0b14f8a`), launcher 0.2.78: every row checked passes (results at its end), after the fixes in `CHANGELOG.md`; not covered: the four two-player rows, hover tooltips, the large-store tick timing, the animations' sway |
| In-game (0.4.0) | Smoke test on Nightly 2026-11-13 (`9f42064`), launcher 0.2.63: the altar crushing raw iron with a Foliot core; a white chalk drawing two glyphs for 2 durability; spirit fire lit over dropped fruit (used up) turning 3 andesite to otherstone; an Actuator placed by hand anchoring a new, named store and opening its terminal; the Eldritch miner digging ores and wearing its lamp |
| In-game (storage) | 0.3.0-alpha.1 on Nightly 2026-11-04: the Actuator, its terminal, upgrades and projection, portable stores and recall, the mineshaft and miners, wormholes (linking, sneak-placing, a mineshaft feeding one) and the Accessor, from `TESTING.md` rows 27 to 56; not yet the miner rituals, the redstone stop, hoppers into a store, or two-player trust |
| In-game (spirit fire) | Unreleased, on Nightly 2026-11-04: spirit fire lit by dropped fruit (all used up); andesite, a diamond, 5 books and feathers turned, stone left alone; planting, bone meal and a ripe harvest; a seed from grass broken by hand; a bound book crafted with the Dictionary kept; otherworld logs from oaks with the Third Eye (3 of 15) and none without, ashes, two impure chalks purified into two chalks, otherstone smelted to burnt otherstone; durable chalk: a glyph per use for one durability, into grass, none in creative, its own colour's glyph turned for free, broken after its last use (`TESTING.md` rows 16, 17 and 57 to 67; the 2% rate is unit-tested, not counted in game) |
| In-game | 0.2.0, agent-driven on Horizon `0f0fa62`: the altar's 15 rows; the Foliot crusher ritual end to end (circle, bowls, timing, result, interrupt, messages); glyph signs, facing, cycling and drops; iesnium rates (19.9% with the Third Eye, 41% with goggles, none without); silver ore generation; the Djinni, Afrit, Marid and infused lenses rituals run side by side, each finishing on time with its result. Rechecked on Nightly 2026-11-04: content load, altar, Foliot ritual, chalk, bowls, iesnium (18% of 60 ores with the Third Eye) |

Not in this port yet: the otherworld's own trees and the rest of its materials (otherworld logs
come from oaks chopped with the Third Eye; chalk colours other than white, yellow and cyan are
dyed from white chalk: see `CHANGELOG.md`), spirit fire's
sound and colours, the spirit's name on a bound book, other spirit jobs, rituals that summon
an entity or ask for an item to be used, sounds and particles, JEI-style recipe display.

A chalk draws 256 glyphs, as Occultism's. Horizon's native API can't place a block by name from a
player's hook, so the glyph appears at the end of the tick it is drawn in. Occultism's code is MIT; its third-party asset notices ship in `mod/`.

## License

MIT, like Occultism: see [LICENSE](LICENSE). [NOTICE](NOTICE) carries Occultism's own MIT notice
(this port is based on its code, data and art, with the blessing of Occultism's creator, KliKli ([klikli-dev](https://github.com/klikli-dev))). Some of Occultism's art is
under Creative Commons Attribution 3.0; its credits ship with the content mod
(`mod/THIRD_PARTY_NOTICES-occultism.md`, `mod/assets/occultism/textures/about_isabella.txt`).
