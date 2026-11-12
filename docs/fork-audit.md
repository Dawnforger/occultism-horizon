# Moving to the arismoko modding fork: phase 1 audit

Checked on 2026-11-12 against the fork's Nightly 2026-11-11 (`514ecf9`), read from its source
(`crates/horizon-mod-vocab`, the hook catalog the mod API is generated from) and its book
(https://arismoko.github.io/horizon/). Nothing here is compiled yet: names and declarations were
matched by reading, and phase 2 is where the compiler confirms them.

## Set up

- Fork launcher `launcher@0.2.10` (Linux tarball, checksum checked against the release's
  `SHA256SUMS`; the signature itself was not verified) in `.work/fork-launcher-0.2.10/`.
- Its data is its own: `~/.local/share/horizon-fork-arismoko/`. The official launcher, its instance
  and the 0.3.0-alpha.1 build are untouched.
- `nightly-2026-11-11-514ecf9` installed and prepared. Playing it needs a signed-in Microsoft
  account in the fork launcher (`horizon-fork-launcher account`), which is the player's to do.

## The package changes shape

Ours today is two things: a content mod (`content/`, `occultism.hmod`) and a trait mod in `native/`
(`horizon-mixin.toml` with `[compatibility] horizon_revision` and `[api] path = "engine"`, the
engine crate depending on `horizon-client` by path).

The fork's is one flat project (book, "Rules for agents"):

- `horizon-mod.toml`, `Cargo.toml`, `src/`, `assets/`, `data/` at the top; no `native/` folder.
- The manifest keeps `api = "0.2"`, `side`, `capabilities`, and adds `[native]` (`game`, `horizon`
  pins) in place of `horizon-mixin.toml`.
- The code implements `mod_api::Hooks` under `#[mod_api::horizon_mod]`, and attaches Parts in
  `register` (`r.block_part(r.block_key("occultism:..."), ...)`).
- `cargo horizon-mod new occultism --template native` writes the skeleton to start from.

Content declarations are the same API 0.2 format, with more: `occludes`, `random_ticks`,
`render_shape`, and state properties a block declares itself (the glyphs could stop borrowing
`leaf_litter`'s states).

## Hooks

We implement 47 hooks. 40 have a hook of the same name in the fork's catalog:

| Kind | Hooks |
|---|---|
| Block entity (scoped to the block or entity) | `block_entity`, `block_entity_container`, `block_entity_render_bounding_box`, `block_entity_submit`, `block_entity_tick`, `block_entity_ticks`, `block_entity_update_tag` |
| Menus and screens | `menu_button`, `menu_quick_move`, `menu_slot_rules`, `screen`, `screen_click`, `screen_draw`, `screen_tooltip`, `screen_closing`, `screen_render_post`, `screen_char_typed_pre`, `screen_key_pressed_pre`, `screen_mouse_button_pressed_pre`, `screen_mouse_scrolled_pre`, `player_container_open`, `player_container_close` |
| Items (scoped to the stack) | `inventory_tick`, `tooltip`, `use_item` |
| World and players | `server_tick`, `world_lifecycle`, `world_saved`, `player_logged_out`, `register_commands`, `component_types`, `loot_drops`, `right_click_block`, `entity_tick_pre`, `entity_tick_listened`, `movement_input`, `should_apply_effect_tick_this_tick` |
| Network | `net_from_client`, `net_from_server` |
| Block (scoped to the state) | `state_for_placement` |

A shared name doesn't promise the same arguments: the fork's clean break moved hooks to events
(`&mut` event, handles from the event, writes through `host`), so each is rewritten in phase 2.

The other 7:

| Ours | In the fork |
|---|---|
| `use_item_on` (chalk) | Item `use_on`, which now gets the `hand` and a `BlockHit`; the block's own is the block-scoped `use_item_on` |
| `block_entity_inside` (spirit fire) | `entity_inside`, scoped to the block |
| `apply_effect_tick` | Still there, declared in `on_effect_added.rs` |
| `place_block`, `destroy_block`, `perform_command`, `tick` | Not in the catalog by these names; to be matched in phase 2 (candidates: block Part hooks such as `remove_block`, `affect_neighbors_after_removal`, `break_block`, and the `command` hooks) |

## Host calls

We call 45 host methods. 33 have the same name in the fork. The other 12 have counterparts by a
new name, still to be confirmed by the compiler:

| Ours | The fork's |
|---|---|
| `set_item_entity` | `item_entity_set_stack` |
| `insert_item`, `extract_item`, `take_items`, `give_stack` | `container_insert` / `container_extract`, `item_container_*`, `give` |
| `tell` | `player_tell` |
| `block_property` | `level_block_property` |
| `neighbor_signal` | `level_has_neighbor_signal`, `level_signal` |
| `carried_stack` | `menu_carried` for the cursor; the held stack in `inventory_tick` comes from the event |
| `draw_cube`, `draw_quad`, `draw_item` | The new rendering API (#808): world lines and quads, model handles, `draw_model_instances`, `gui_draw_item_with` |

## Workarounds the fork makes unnecessary

| Workaround on official Horizon | On the fork |
|---|---|
| Glyphs, datura and spirit fire set with `/setblock` through `run_command`, deferred to the server tick | `LevelWriteHost::set_block(pos, state, flags)`: `Level.setBlock` with the block entity, updates and events |
| The client sends its sneak state (`Request::Sneak`) for linking and sneak-placing | `entity_is_shift_key_down` on the server |
| Chalk assumes the main hand and debounces per-hand clicks | Item `use_on` gets the `hand` |
| Chalk wear by rewriting the held stack's damage in `inventory_tick` | A `damage_item` hook exists; how an item is hurt is to be checked in phase 2 |
| Spirit fire learns where Demon's Dream lies by waiting for item ticks (`entity_tick_listened`) | `level_entities_in` asks for the items in a box |
| Datura seeds plant wheat and the sprout is swapped at the end of the tick | Block Parts for crops: `may_place_on`, `can_survive`, `random_tick`, `perform_bonemeal`, `is_valid_bonemeal_target`, and `item_place` |
| Glyph textures pre-tinted per colour | A `block_tint` block API |
| Item tags read by hand | Still to check |

## Gap report

Nothing confirmed missing yet. Every gap a port reports on the fork has to be a moment with no hook
or Part after phase 2's rewrite, in the shape of #725 (Farmer's Delight): missing, covered for
reference, not needed. It starts as `upstream/fork-gaps.md` and is posted only on the user's OK.

## Phase 2, next

1. Generate the skeleton with the fork's `cargo horizon-mod` and move `content/`, `assets/`,
   `data/` and `native/engine/src` into the flat layout on a branch.
2. Build with `horizon-fork-launcher native-build . --trust-native-code` against `514ecf9`, and
   let the compiler drive the hook and host rewrites above.
3. Replace the workarounds one by one, each retested in game against TESTING.md.
