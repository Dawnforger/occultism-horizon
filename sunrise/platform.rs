// What Sunrise's mod API asks differently from Horizon's, for the code both share: compiled in
// mod/src/platform/horizon.rs's place in the Sunrise package (tools/make_sunrise.py). It lives
// outside mod/ so the Horizon package doesn't carry it. The same names as
// that file; keep the two in step.

use crate::vault::{gate_facing, GatePlacement};
use mod_api::*;
use std::sync::Arc;

/// Where a menu screen draws a slot: Sunrise takes its corner and an optional empty-slot icon.
pub type Slot = ScreenSlot;

pub fn slot(at: [f32; 2]) -> Slot {
    ScreenSlot { position: at, no_item_icon: None }
}

/// The stack a quick move is about: Sunrise's slot may be empty.
pub fn quick_stack(stack: &mut Option<ItemStack>) -> Option<&mut ItemStack> {
    stack.as_mut()
}

/// Whether an `ItemPlace::edit` changed the stack.
pub fn edited(changed: Result<(), ItemPlaceEditError>) -> bool {
    changed.is_ok()
}

/// An attachment snapshot.
pub fn batch(batch: Result<AttachmentBatch, AttachmentError>) -> Option<AttachmentBatch> {
    batch.ok()
}

/// A random draw below a bound (`next_int_bound`).
pub fn below(drawn: i32) -> i32 {
    drawn
}

/// The menu type a container opened or closed with.
pub fn opened_menu<'a>(event: &PlayerContainerOpenEvent<'a>) -> &'a str {
    event.menu_type
}

pub fn closed_menu<'a>(event: &PlayerContainerCloseEvent<'a>) -> &'a str {
    event.menu_type
}

/// Registrations only one of the two needs. Sunrise asks a menu's Part for its screen, and has
/// every attachment the mod uses declared, with whether it syncs to clients.
pub fn register(r: &mut dyn Registrar, s: &Arc<crate::State>) {
    let (work, look) = (&crate::workers::WORK, &crate::workers::LOOK);
    for (namespace, name, sync) in [(work.namespace, work.name, work.sync), (look.namespace, look.name, look.sync)] {
        let _ = r.register_attachment_type(&format!("{namespace}:{name}"), sync);
    }
    r.menu_part(crate::mineshaft::MENU, ShaftScreen);
    r.menu_part(crate::vault::MENU, StoreScreen(s.clone()));
}

#[mod_api::sunrise_part]
impl BlockPart for GatePlacement {
    fn state_for_placement(&self, level: LevelSide, event: &mut StateForPlacementEvent<'_>) -> Option<Option<BlockState>> {
        let (LevelSide::Server(level) | LevelSide::Client(level)) = level;
        level.state_with_property(event.state, "facing", gate_facing(event.context.face)).map(Some)
    }
}

/// The Demonic Altar's screen.
struct AltarScreenftScreen;

#[mod_api::sunrise_part]
impl MenuPart for ShaftScreen {
    fn screen(&self, _host: &dyn Host, _event: &mut ScreenEvent<'_>) -> Option<Screen> {
        Some(crate::mineshaft::screen())
    }

    fn screen_draw(&self, host: &dyn ClientHost, event: &mut ScreenDrawEvent<'_>) {
        crate::mineshaft::draw_screen(host, event.data, &mut *event.draw, event.origin);
    }
}

/// The storage terminal of an Actuator or a stable wormhole.
struct StoreScreen(Arc<crate::State>);

#[mod_api::sunrise_part]
impl MenuPart for StoreScreen {
    fn screen(&self, _host: &dyn Host, _event: &mut ScreenEvent<'_>) -> Option<Screen> {
        Some(self.0.ui.screen())
    }

    fn screen_draw(&self, host: &dyn ClientHost, event: &mut ScreenDrawEvent<'_>) {
        self.0.ui.draw(host, &mut *event.draw, event.origin, event.data);
    }

    fn screen_click(&self, _host: &dyn ClientHost, event: &mut ScreenClickEvent<'_>) -> Option<i32> {
        self.0.ui.click(event.at, event.right)
    }

    /// The terminal's tooltips, in Sunrise's lines: text and a colour.
    fn screen_tooltip(&self, host: &dyn ClientHost, event: &mut ScreenTooltipEvent<'_>) {
        let mut lines = Vec::new();
        self.0.ui.tooltip(host, event.at, event.data, &mut lines);
        event.lines.extend(lines.into_iter().map(|l| (Text::literal(&l), 0xFFFFFFFF)));
    }
}

/// The paste recipes' crafting kind (paste.rs). Sunrise hands each call the game's tags, which the
/// paste recipes don't need: their items are resolved when the mod is built.
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

    fn matches(&self, _context: &dyn LoopContext, value: &Self::Value, grid: CraftingGrid) -> bool {
        self.fits(value, &grid)
    }

    fn assemble(&self, _context: &dyn LoopContext, value: &Self::Value, _grid: CraftingGrid) -> Option<ItemStack> {
        self.result(value)
    }

    fn remaining_items(&self, _context: &dyn CraftingRemainders, _value: &Self::Value, grid: CraftingGrid) -> Option<Vec<Option<ItemStack>>> {
        Some(self.left(&grid))
    }
}
