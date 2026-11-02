# Native mod API gaps: use-case comments, posted 2026-11-02 (arismoko/horizon#29 and #27)

Checked 2026-11-02 against Horizon `main` (`888d694`) and its trackers. Every gap this port hit is
already tracked, so these are use-case comments for the existing trackers, not new issues.
Native mod API work has moved to the fork `arismoko/horizon`; its trackers mirror upstream's:

| Gap | Tracker line | Status there | Our workaround |
|---|---|---|---|
| A block state by name, to `set_block` a mod's own block | #7387 / arismoko#29 "block states and properties": `block_state_default` | claimed (reservedbytes), stacked on #7454 | the marker block is its own block item, placed by vanilla; `state_for_placement` sets its properties |
| Reading another block entity's slots | #7387 / arismoko#29 "inventory and containers": `container_slots`, `container_contents` | paused (to become one capability API) | each container remembers its contents from its own tick |
| A player's effects and armour | #7387 / arismoko#29 "Players" | open | `apply_effect_tick` and `inventory_tick` (slot 103) record them per player |
| Mod block tint | #4953 / arismoko#27 "Block tint" (`BlockTintSources`) | not started | textures pre-tinted per colour at build time |
| Which hand used a block | #4953 / arismoko#27 "The hand and the hit point on block use" | claimed (reservedbytes) | a second call for the same player and block within 150 ms is ignored |

Nothing below names this project, its mod or its blocks. Horizon's agent guide asks every agent
comment to start with `🤖 Agent:`; drop it if you post them in your own words.

---

## 1. For arismoko/horizon#29 (Host services)

🤖 Agent: a use case for three lines here, from a native mod with machines and
multiblock structures, built against `0f0fa62`:

- **`block_state_default`** (block states and properties): the mod needs to place its own
  blocks (flat floor markers with a couple of properties) from a hook. `set_block` takes a
  `BlockStateId`, but a trait mod has no way to get one for a block it hasn't seen in the world.
  Our workaround is to make the marker its own block item, let vanilla's `BlockItem.place` draw
  it and set its properties in `state_for_placement`, which changes the gameplay (the placing
  item is used up). A `Host::block_state_default(name)` plus `state_with_property` would cover
  it.
- **`container_slots` / `container_contents`** (inventory and containers): a controller block
  entity needs to see what nearby one-slot block entities hold before taking from them.
  `extract_item` can take but not look, so each container records its contents into shared
  state on its own tick, which can be a tick stale. A read-only `container_slots(dimension, pos)`
  would be enough for us, ahead of the full capability API.
- **Players' effects and armour**: a loot change depends on whether the breaking player has a
  given mob effect or wears a given helmet. We track both from `apply_effect_tick` and
  `inventory_tick` (head slot 103); a read on `PlayerRef` would replace that bookkeeping.

## 2. For arismoko/horizon#27 (hook coverage)

🤖 Agent: two use cases from a native mod built against `0f0fa62`:

- **Block tint** (`RegisterColorHandlersEvent.BlockTintSources`): a content block drawn in 16
  colours from one greyscale texture with `tintindex`. Without a tint source the face draws
  untinted, so the build bakes 16 copies of each texture. A per-block constant tint would be
  enough here; a state-dependent one would save the copies for other mods too.
- **The hand on block use** (`RightClickBlock.getHand`): `right_click_block` is called once per
  hand, and returning `Flow::Cancel` for the main hand doesn't stop the off hand's call. A block
  that takes the held item on click (put an item in, take it back out) sees the second call with
  an empty main hand and undoes the first. We ignore a repeat call for the same player and block
  within 150 ms; with the hand on the event, the mod could simply act on the main hand only.
