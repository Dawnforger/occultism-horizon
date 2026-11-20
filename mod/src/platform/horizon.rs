// What Horizon's mod API asks differently from Sunrise's, for the code both share. The Sunrise
// package (tools/make_sunrise.py) compiles sunrise/platform.rs (at the repository's root, outside
// the Horizon package) in this file's place: each has the
// same names, so the rest of the port is one source. Keep the two in step.

use crate::vault::{gate_facing, GatePlacement};
use mod_api::*;
use std::sync::Arc;

/// Where a menu screen draws a slot: Horizon takes its top-left corner.
pub type Slot = [f32; 2];

pub fn slot(at: [f32; 2]) -> Slot {
    at
}

/// The stack a quick move is about. On Horizon it is always there (a slot holds at least one).
pub fn quick_stack(stack: &mut ItemStack) -> Option<&mut ItemStack> {
    Some(stack)
}

/// Whether an `ItemPlace::edit` changed the stack.
pub fn edited(changed: bool) -> bool {
    changed
}

/// An attachment snapshot.
pub fn batch(batch: AttachmentBatch) -> Option<AttachmentBatch> {
    Some(batch)
}

/// A random draw below a bound (`next_int_bound`), 0 for a bound it refuses.
pub fn below<E>(drawn: Result<i32, E>) -> i32 {
    drawn.unwrap_or(0)
}

/// The menu type a container opened or closed with.
pub fn opened_menu<'a>(event: &PlayerContainerOpenEvent<'a>) -> &'a str {
    event.menu
}

pub fn closed_menu<'a>(event: &PlayerContainerCloseEvent<'a>) -> &'a str {
    event.menu
}

/// Registrations only one of the two needs. Horizon asks the block entities' Parts for their
/// screens, which they answer themselves.
pub fn register(_r: &mut dyn Registrar, _s: &Arc<crate::State>) {}

#[mod_api::horizon_part]
impl BlockPart for GatePlacement {
    fn state_for_placement(&self, level: &dyn LevelHost, event: &mut StateForPlacementEvent<'_>) -> Option<Option<BlockState>> {
        level.state_with_property(event.state, "facing", gate_facing(event.context.face)).map(Some)
    }
}
