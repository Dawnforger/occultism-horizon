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

/// The paste recipes' crafting kind (paste.rs).
impl CraftingRecipeKind for crate::paste::PasteKind {
    type Value = crate::paste::PasteRecipe;

    fn codec(&self) -> Option<CraftingRecipeCodec<Self::Value>> {
        Some(CraftingRecipeCodec::wire())
    }

    fn decode(&self, json: &str) -> Result<Self::Value, String> {
        self.read(json)
    }

    fn write(&self, value: &Self::Value) -> Result<String, String> {
        self.json(value)
    }

    fn matches(&self, value: &Self::Value, grid: CraftingGrid) -> bool {
        self.fits(value, &grid)
    }

    fn assemble(&self, value: &Self::Value, _grid: CraftingGrid) -> Option<ItemStack> {
        self.result(value)
    }

    fn remaining_items(&self, _value: &Self::Value, grid: CraftingGrid) -> Option<Vec<Option<ItemStack>>> {
        Some(self.left(&grid))
    }
}

/// What `living_incoming_damage` is given to act on the level (the APIs name it differently).
pub type DamageHost<'a> = dyn LevelRandomHost + 'a;

/// A sound at `at` in `level` for the players near it, made by no one.
pub fn play_sound(host: &mut dyn ServerHost, level: LevelRef, sound: &str, at: [f64; 3], source: SoundSource, volume: f32, pitch: f32) -> bool {
    level.play_sound(host, sound, at, source, volume, pitch)
}
