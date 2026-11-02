# Occultism for Horizon: Demonic Altar (proof of concept)

Occultism's worker spirits, redesigned for Horizon: instead of a summoned entity walking around,
a spirit is bound into a **spirit core** that you place in a generic **Demonic Altar**. The core
decides what the altar does. This proof of concept binds Occultism's crusher in all four ranks.

- **Same numbers as Occultism.** Recipes are Occultism's own `occultism:crushing` files; ranks use
  its config defaults (Foliot 400 ticks x2, Djinni 200 x3, Afrit 100 x4, Marid 61 x6 on raw iron),
  tier gates (`min_tier`/`max_tier`) and the ingot rule (`ignore_crushing_multiplier`).
  The timings were cross-checked against Java's float math.
- **A machine, not a mob.** Input and output buffers, hoppers in from the top and sides, out from
  the bottom; the core slot is unreachable by automation. A full output buffer holds the finished
  operation instead of dropping items on the ground.
- **The spirit is data.** A core's `occultism:spirit` component (`{job, tier}`) decides the job,
  so new jobs and ranks are new data and a new arm in `logic.rs`, not a new block.

## Layout

| Path | What |
|---|---|
| `native/` | The native mod: `horizon-mixin.toml` and `engine/` (a trait mod) |
| `native/engine/src/logic.rs` | The altar's rules, no Horizon types |
| `native/engine/src/lib.rs` | Glue to Horizon's hooks: block entity, menu, screen, hoppers, drawing, tooltips |
| `tests/logic/` | 15 unit tests of the rules, runnable anywhere (`cargo test`) |
| `tools/build_content.py` | Builds `content/` (the `.hmod`) from an Occultism checkout |
| `content/` | Generated content mod: altar, cores, dusts, 25 crushing recipes, tags, Occultism's art |
| `build.sh` | Logic tests, content mod, `.hmod` check, native build, in that order |
| `TESTING.md` | The 15-row in-game pass |

## Status

| Part | State |
|---|---|
| Rules (`logic.rs`) | 15/15 tests pass; tier timings match a Java oracle |
| Content mod | Generated and self-checked; `cargo horizon-mod check` runs in `build.sh` |
| Glue (`lib.rs`) | `cargo check` clean (no warnings) against Horizon `0f0fa62` (NATIVE_HOOKS_API 153); native build needs launcher ≥ the one at that commit (0.2.26 rejects `[api]`) |
| In-game | **Not yet run**: `TESTING.md` |

Not in this proof of concept: the binding rituals (cores are crafted, with each ritual's
ingredients where vanilla has them), other jobs, crushing sounds and particles, JEI-style recipe
display. Occultism's code is MIT; its third-party asset notices ship in `content/`.
