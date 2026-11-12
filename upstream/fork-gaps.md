# Occultism on the fork's mod API: what's still missing (draft, not posted)

Found while porting to arismoko/horizon Nightly 2026-11-11 (`514ecf9`). Each item is checked
against the source at that commit; posting needs Dawnforger's OK.

## missing

- [ ] **`menu_quick_move` can't move a whole stack.** `event.stack` is a `mod_api::ItemStack`,
  whose count is `NonZeroU32`, and `settle` (`crates/horizon-client/src/game/modding/native_quick_move.rs`)
  writes what is left of it back to the clicked slot when the hook is handled. So a mod that moves
  all of the clicked stack (one spirit core into the altar's core slot, a whole stack into a
  storage block) has no way to say the slot is now empty; `set_count(0)` is refused with
  `StackError::Empty`. Java's `quickMoveStack` empties the slot with `ItemStack.EMPTY`.
  Suggestion: `stack: &mut Option<ItemStack>`, or a `taken: u32` field.
- [ ] **No stack on placement.** `entity_place` (`BlockEvent.EntityPlaceEvent`) has the placer and
  state but not the placed item, as NeoForge's doesn't; vanilla's moment with it is
  `Block.setPlacedBy(level, pos, state, placer, stack)`. A block that takes data from its item (our
  storage Core carries its store's id; a wormhole its link) has to remember the stack from the
  item's `use_on` and match it at `entity_place`. Suggestion: a `BlockPart::set_placed_by` with the
  stack.
