# Occultism for Horizon

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
  Goggles (`ritual.rs`), vein mining included.
- **The spirit is data.** A core's `occultism:spirit` component (`{job, tier}`) decides the job,
  so new jobs and ranks are new data and a new arm in `logic.rs`, not a new block.

## Layout

| Path | What |
|---|---|
| `native/` | The native mod: `horizon-mixin.toml` and `engine/` (a trait mod) |
| `native/engine/src/logic.rs` | The altar's rules, no Horizon types |
| `native/engine/src/lib.rs` | Glue to Horizon's hooks: block entity, menu, screen, hoppers, drawing, tooltips |
| `native/engine/src/ritual.rs` | The rituals' rules, no Horizon types: pentacles, ingredients, timing, glyphs, iesnium |
| `native/engine/src/rites.rs` | Glue for chalk glyphs, sacrificial bowls, the golden bowl's rituals, iesnium |
| `native/engine/src/storage.rs` | The storage's rules, no Horizon types: the store, its save log, capacity |
| `native/engine/src/vault.rs` | Glue for the storage multiblock, and the one place that turns stacks into keys |
| `docs/storage-design.md` | Why Occultism's storage lags, the performance rules, the structure, the Acolyte roadmap |
| `tests/logic/` | 37 unit tests of the rules, including scale tests, runnable anywhere (`cargo test`) |
| `tools/build_content.py` | Builds `content/` (the `.hmod`) from an Occultism checkout |
| `content/` | Generated content mod: altar, cores, chalks and glyphs, bowls, silver and iesnium, rituals and pentacles, recipes, tags, Occultism's art |
| `build.sh` | Logic tests, content mod, `.hmod` check, native build, in that order |
| `TESTING.md` | The in-game pass (altar, rituals, iesnium) |
| `CHANGELOG.md`, `VERSION` | What changed in each version; the current version (`tools/set_version.py` writes it everywhere) |

## Status

| Part | State |
|---|---|
| Rules (`logic.rs`, `ritual.rs`, `storage.rs`) | 37/37 tests pass; tier timings match a Java oracle |
| Content mod | Generated and self-checked; `cargo horizon-mod check` runs in `build.sh` |
| Glue (`lib.rs`, `rites.rs`) | Built and run on Horizon Nightly 2026-11-04 (`6475f62`, NATIVE_HOOKS_API 359) with launcher 0.2.41; earlier launchers can't build it |
| In-game | 0.2.0, agent-driven on Horizon `0f0fa62`: the altar's 15 rows; the Foliot crusher ritual end to end (circle, bowls, timing, result, interrupt, messages); glyph signs, facing, cycling and drops; iesnium rates (19.9% with the Third Eye, 41% with goggles, none without); silver ore generation; the Djinni, Afrit, Marid and infused lenses rituals run side by side, each finishing on time with its result Rechecked on Nightly 2026-11-04: content load, altar, Foliot ritual, chalk, bowls, iesnium (18% of 60 ores with the Third Eye) |

Not in this port yet: spirit fire, otherworld materials and the Dictionary of Spirits (chalk,
binding books, spirit-attuned gems and Demon's Dream Essence are crafted from vanilla items: see
`CHANGELOG.md`), other spirit jobs, ritual sacrifices and item-use requirements, sounds and
particles, JEI-style recipe display.

Horizon's native API can't place a block by name, so a chalk is its glyph's block item: drawing a
glyph uses up one chalk (Occultism's chalk loses durability instead). Occultism's code is MIT; its third-party asset notices ship in `content/`.

## License

MIT, like Occultism: see [LICENSE](LICENSE). [NOTICE](NOTICE) carries Occultism's own MIT notice
(this port is based on its code, data and art, with klikli-dev's blessing). Some of Occultism's art is
under Creative Commons Attribution 3.0; its credits ship with the content mod
(`content/THIRD_PARTY_NOTICES-occultism.md`, `content/assets/occultism/textures/about_isabella.txt`).
