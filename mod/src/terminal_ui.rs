// The storage terminal on the client: the Actuator's screen, drawn and driven by the player's own
// copy of the store (terminal.rs's `View`), which the server keeps up to date over `occultism:store`.
// Searching, sorting and scrolling never leave the client; a click on the grid is a menu button
// naming what was clicked (`terminal::button_for`), and the server hands it to the player.
//
// The same terminal opens with no block behind it for a Storage Accessor: a screen of the mod's own
// (`mod_screen_open`), drawn over the world with no inventory, its grid clicks sent to the server
// as `terminal::Request`s from the next client tick (the screen's hooks can't send).

use crate::network::UPGRADE_SLOTS;
use crate::terminal::{self, Entry, Message, View, COLUMNS, MAX_ROWS};
use crate::game::{id, lock};
use crate::vault;
use mod_api::prelude::*;
use mod_api::*;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The screen, laid out as Occultism's terminal (`StorageTerminalLayouts`, `StorageMenuLayouts`),
/// 260 wide: a top bar with the spirit's name, the search field and the buttons; the item area,
/// 11 columns by the rows chosen, in a slate panel; under it the crafting grid, its arrow and
/// result; then the player's inventory in a panel of its own. The upgrade column (this port's
/// plates) stands to the right of the item area, and the store's capacity under the result.
/// Everything is drawn here (`own_background`), in Occultism's colours: its GUI library's panel and
/// slot sprites tinted by its storage style, with its own item-area and search-field sprites.
pub const WIDTH: f32 = 260.0;
const TOP_BAR: [f32; 4] = [21.0, 0.0, 218.0, 33.0];
const SEARCH: [f32; 4] = [30.0, 17.0, 96.0, 14.0];
/// The top bar's buttons, 12 square, 15 apart: clear the search, sort type, sort direction, rows.
const BUTTONS: [f32; 2] = [131.0, 18.0];
const BUTTON_COUNT: usize = 4;
const PANEL_X: f32 = 24.0;
const PANEL_W: f32 = 212.0;
const PANEL_Y: f32 = 24.0;
const GRID: [f32; 2] = [32.0, 36.0];
/// The upgrade column: one slot per plate, beside the item area.
const UPGRADES: [f32; 2] = [240.0, 36.0];
/// Menu buttons 1 to 6: take the plate out of that upgrade slot.
const UPGRADE_BUTTON: i32 = 1;
/// Below the item area (`menu_top`): the crafting grid, its result and arrow, the clear button.
const CRAFT_GRID: [f32; 2] = [85.0, 4.0];
const CRAFT_RESULT: [f32; 2] = [181.0, 22.0];
const CRAFT_ARROW: [f32; 2] = [146.0, 23.0];
const CLEAR_GRID: [f32; 4] = [141.0, 3.0, 12.0, 12.0];
/// The capacity labels under the result, and the area their tooltip answers for.
const LABELS_Y: f32 = 44.0;
const LABELS: [f32; 4] = [150.0, 43.0, 80.0, 19.0];
const INVENTORY: [f32; 2] = [51.0, 71.0];
/// The inventory's panel, and the screen's bottom, below `menu_top`.
const INVENTORY_PANEL: [f32; 4] = [44.0, 64.0, 176.0, 92.0];
const BOTTOM: f32 = 156.0;

/// Occultism's storage style (`OccultismGuiStyles`): panels, top bar, slots, buttons, fields.
const PANEL_TINT: u32 = 0xFF4B5563;
const TOP_BAR_TINT: u32 = 0xFF9C0393;
const SLOT_TINT: u32 = 0xFF697586;
const BUTTON_TINT: u32 = 0xFF5D6878;
const BUTTON_HOVER_TINT: u32 = 0xFF707C8D;
const FIELD_TINT: u32 = 0xFF596474;
const TEXT: u32 = 0xFFE6E8EC;
const TEXT_DIM: u32 = 0xFFB4BAC4;
const ITEM_AREA_SPRITE: &str = "occultism:storage_controller/storage_controller_item_area_bg";
const SEARCH_SPRITE: &str = "occultism:storage_controller/search_field";

/// The top of the area below the item area, for `rows` rows.
fn menu_top(rows: usize) -> f32 {
    GRID[1] + 18.0 * rows as f32
}

/// The screen's height: Occultism's, with the inventory; an Accessor's ends under its capacity.
fn height(rows: usize, inventory: bool) -> f32 {
    menu_top(rows) + if inventory { BOTTOM } else { 34.0 }
}

/// The upgrade slot a menu button names.
pub fn upgrade_click(button: i32) -> Option<usize> {
    let slot = button.checked_sub(UPGRADE_BUTTON)?;
    (0..UPGRADE_SLOTS as i32).contains(&slot).then_some(slot as usize)
}

fn upgrade_at(at: [f32; 2]) -> Option<usize> {
    let (x, y) = (at[0] - UPGRADES[0], at[1] - UPGRADES[1]);
    (x >= 0.0 && x < 18.0 && y >= 0.0).then(|| (y / 18.0) as usize).filter(|s| *s < UPGRADE_SLOTS)
}

fn button_at(at: [f32; 2]) -> Option<usize> {
    (0..BUTTON_COUNT).find(|&i| inside(at, [BUTTONS[0] + 15.0 * i as f32, BUTTONS[1], 12.0, 12.0]))
}

/// A plate's tier as it gives capacity: types, items.
fn plate_gives(tier: i32) -> Option<(u32, u64)> {
    let base = crate::storage::Limits::with_plates(&[]);
    let with = crate::storage::Limits::with_plates(&[tier as u8]);
    (with != base).then(|| (with.types - base.types, with.total - base.total))
}
/// The native mod's id, which names its own screen.
pub const MOD_ID: &str = "occultism";
/// How often a changing store re-sorts the grid: a big store's sort is a few milliseconds.
const RESORT: Duration = Duration::from_millis(200);

/// A colour's channels scaled by `f` (a panel's light and dark edges from its tint).
fn shade(argb: u32, f: f32) -> u32 {
    let c = |shift: u32| ((((argb >> shift) & 0xFF) as f32 * f).round().clamp(0.0, 255.0) as u32) << shift;
    (argb & 0xFF00_0000) | c(16) | c(8) | c(0)
}

/// A raised panel in `tint`, as a GUI_BACKGROUND sprite tinted: a dark rim, light top and left
/// edges, dark bottom and right ones.
fn panel(batch: &mut dyn DrawContext, r: [f32; 4], tint: u32) {
    batch.fill([r[0] + 1.0, r[1], r[2] - 2.0, r[3]], 0xFF000000);
    batch.fill([r[0], r[1] + 1.0, r[2], r[3] - 2.0], 0xFF000000);
    batch.fill([r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0], shade(tint, 0.62));
    batch.fill([r[0] + 1.0, r[1] + 1.0, r[2] - 3.0, r[3] - 3.0], shade(tint, 1.45));
    batch.fill([r[0] + 2.0, r[1] + 2.0, r[2] - 4.0, r[3] - 4.0], tint);
}

/// A slot frame for the item at `at` (an 18-square cell, sunken: dark top and left edges).
fn slot(batch: &mut dyn DrawContext, at: [f32; 2], tint: u32) {
    let (x, y) = (at[0] - 1.0, at[1] - 1.0);
    batch.fill([x, y, 18.0, 18.0], shade(tint, 1.5));
    batch.fill([x, y, 17.0, 17.0], shade(tint, 0.55));
    batch.fill([x + 1.0, y + 1.0, 16.0, 16.0], tint);
}

/// A 12-square button with a one-character `label` in it, raised, lighter under the mouse.
fn button(batch: &mut dyn DrawContext, r: [f32; 4], label: &str, hovered: bool) {
    let tint = if hovered { BUTTON_HOVER_TINT } else { BUTTON_TINT };
    batch.fill(r, 0xFF000000);
    batch.fill([r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0], shade(tint, 1.4));
    batch.fill([r[0] + 2.0, r[1] + 2.0, r[2] - 3.0, r[3] - 3.0], shade(tint, 0.7));
    batch.fill([r[0] + 2.0, r[1] + 2.0, r[2] - 4.0, r[3] - 4.0], tint);
    let w = batch.text_width(label);
    batch.text(label, [r[0] + ((r[2] - w) / 2.0).floor() + 0.5, r[1] + 2.0], 0xFFFFFFFF, false);
}

#[derive(Default)]
pub struct Ui {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    view: View<ItemStack>,
    typing: bool,
    sorted: Option<Instant>,
    /// An Accessor's terminal is open (or opening): a screen of the mod's own.
    remote: bool,
    /// Requests waiting for the next client tick to be sent.
    outbox: Vec<Vec<u8>>,
}

fn inside(at: [f32; 2], r: [f32; 4]) -> bool {
    (r[0]..r[0] + r[2]).contains(&at[0]) && (r[1]..r[1] + r[3]).contains(&at[1])
}

/// The grid cell at `at` (GUI pixels from the screen's top left), with `rows` rows shown.
fn cell(at: [f32; 2], rows: usize) -> Option<usize> {
    let (x, y) = (at[0] - GRID[0], at[1] - GRID[1]);
    let (c, r) = ((x / 18.0).floor(), (y / 18.0).floor());
    (x >= 0.0 && y >= 0.0 && (c as usize) < COLUMNS && (r as usize) < rows).then(|| r as usize * COLUMNS + c as usize)
}

/// A translated line, or its key when the game has none.
fn lang(host: &dyn ClientHost, key: &str) -> String {
    host.translate(key).unwrap_or_else(|| key.to_string())
}

/// An item's name as the player reads it: its custom name isn't read yet, so its item's.
fn name_of(host: &dyn ClientHost, id: &str) -> String {
    let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
    ["item", "block"].iter().find_map(|kind| host.translate(&format!("{kind}.{ns}.{path}"))).unwrap_or_else(|| path.replace('_', " "))
}

/// Whether `screen` is the terminal: an Actuator's or wormhole's menu, or an Accessor's own screen.
fn ours(screen: &ScreenView<'_>, remote: bool) -> bool {
    screen.menu == Some(vault::MENU) || (remote && screen.id == "mod")
}

/// A percentage of a capacity, as Occultism's labels show it.
fn percent(used: u64, of: u64) -> f64 {
    if of == 0 { 0.0 } else { used as f64 * 100.0 / of as f64 }
}

impl Ui {
    /// The Actuator's or wormhole's screen, its slots placed as Occultism's: the store's own slots
    /// stay hidden, the crafting grid and its result under the item area, then the inventory.
    pub fn screen(&self) -> Screen {
        let rows = lock(&self.state).view.rows();
        let top = menu_top(rows);
        let mut slots = vec![None; vault::CONTAINER_SLOTS];
        for (i, slot) in vault::GRID.enumerate() {
            slots[slot] = Some(crate::platform::slot([CRAFT_GRID[0] + 18.0 * (i % 3) as f32, top + CRAFT_GRID[1] + 18.0 * (i / 3) as f32]));
        }
        slots[vault::RESULT] = Some(crate::platform::slot([CRAFT_RESULT[0], top + CRAFT_RESULT[1]]));
        Screen {
            width: WIDTH,
            height: height(rows, true),
            slots,
            inventory_y: top + INVENTORY[1],
            inventory_x: INVENTORY[0],
            own_labels: true,
            own_background: true,
            recipe_book: None,
            recipe_book_toggle: None,
            ..Default::default()
        }
    }

    /// A message from the server about the store being looked at. True when it asks for the
    /// Accessor's screen to open.
    pub fn receive(&self, host: &dyn ClientHost, data: &[u8]) -> bool {
        let Some(message) = terminal::decode(data) else { return false };
        let mut state = lock(&self.state);
        if message == Message::Open {
            state.remote = true;
            return true;
        }
        if matches!(message, Message::Begin(_)) {
            state.sorted = None;
        }
        state.view.apply(message, |key, count| {
            let shown = vault::stack_of(host, key, 1)?;
            let id = id(&*host, &shown).to_string();
            Some(Entry { count, name: name_of(host, &id).to_lowercase(), id, shown })
        });
        false
    }

    /// The terminal's screen closed: forget the store (an Accessor's server stops sending it), but
    /// keep how the player likes it: search, sort, direction, rows.
    pub fn closed(&self, screen: &ScreenView<'_>) {
        let mut state = lock(&self.state);
        if !ours(screen, state.remote) {
            return;
        }
        let (search, sort, reversed, rows, remote) =
            (state.view.search().to_string(), state.view.sort(), state.view.reversed(), state.view.rows(), state.remote);
        let mut outbox = std::mem::take(&mut state.outbox);
        if remote {
            outbox.push(terminal::Request::Close.encode());
        }
        *state = State { outbox, ..State::default() };
        state.view.set_search(search);
        state.view.set_sort(sort);
        state.view.set_reversed(reversed);
        state.view.set_rows(rows);
    }

    /// What the client tick sends the server: the Accessor's requests.
    pub fn outbox(&self) -> Vec<Vec<u8>> {
        std::mem::take(&mut lock(&self.state).outbox)
    }

    /// Requests a tick couldn't send, back at the front of the queue.
    pub fn requeue(&self, mut waiting: Vec<Vec<u8>>) {
        let mut state = lock(&self.state);
        waiting.append(&mut state.outbox);
        state.outbox = waiting;
    }

    /// An Accessor's screen, drawn over the world centred in the window: the terminal without the
    /// crafting grid and inventory, its numbers from the server's `Stats` instead of data slots.
    pub fn draw_remote(&self, host: &dyn ClientHost, batch: &mut dyn DrawContext, screen: &ScreenView<'_>) {
        let (remote, data) = {
            let state = lock(&self.state);
            let data = state.view.stats.as_ref().map(|(types, total, plates)| {
                let mut d = vec![0, (*types).min(32_767) as i32, 0, (total / 1000).min(32_767) as i32];
                d.extend(plates.iter().map(|p| i32::from(*p)));
                d
            });
            (state.remote, data.unwrap_or_default())
        };
        if !remote || screen.id != "mod" || host.mod_screen() != Some(MOD_ID) {
            return;
        }
        let origin = self.remote_origin(screen);
        self.draw_terminal(host, batch, origin, &data, false, screen.mouse);
        // No tooltips over a screen of the mod's own: the hovered item's name and count on a line.
        let local = [screen.mouse[0] - origin[0], screen.mouse[1] - origin[1]];
        let state = lock(&self.state);
        if let Some((_, e)) = cell(local, state.view.rows()).and_then(|c| state.view.at(c)) {
            let line = format!("{}  {}", name_of(host, &e.id), terminal::grouped(e.count));
            let w = batch.text_width(&line) + 6.0;
            let at = [screen.mouse[0] + 10.0, screen.mouse[1] - 12.0];
            batch.fill([at[0], at[1], w, 12.0], 0xF0100010);
            batch.text(&line, [at[0] + 3.0, at[1] + 2.0], 0xFFFFFFFF, false);
        }
    }

    fn remote_origin(&self, screen: &ScreenView<'_>) -> [f32; 2] {
        let r = screen.rect;
        let h = height(lock(&self.state).view.rows(), false);
        [(r[0] + (r[2] - WIDTH) / 2.0).floor(), (r[1] + (r[3] - h) / 2.0).floor()]
    }

    /// A mouse press on an Accessor's screen: the search field and buttons as a menu's, a grid
    /// click queued for the server.
    pub fn remote_press(&self, screen: &ScreenView<'_>, button: i32) -> Flow {
        if !(lock(&self.state).remote && screen.id == "mod") {
            return Flow::Continue;
        }
        let origin = self.remote_origin(screen);
        let local = [screen.mouse[0] - origin[0], screen.mouse[1] - origin[1]];
        if upgrade_at(local).is_some() {
            return Flow::Cancel; // plates are changed at the Actuator
        }
        let take = self.click(local, button == 1);
        if let Some((serial, right)) = take.and_then(terminal::grid_click) {
            lock(&self.state).outbox.push(terminal::Request::Take { serial, right }.encode());
        }
        Flow::Cancel
    }

    pub fn draw(&self, host: &dyn ClientHost, batch: &mut dyn DrawContext, origin: [f32; 2], data: &[i32]) {
        let mouse = host.cursor();
        self.draw_terminal(host, batch, origin, data, true, mouse);
    }

    fn draw_terminal(&self, host: &dyn ClientHost, batch: &mut dyn DrawContext, origin: [f32; 2], data: &[i32], inventory: bool, mouse: [f32; 2]) {
        let mut state = lock(&self.state);
        if state.view.stale() && state.sorted.is_none_or(|t| t.elapsed() >= RESORT) {
            state.view.rebuild();
            state.sorted = Some(Instant::now());
        }
        let [ox, oy] = origin;
        let at = |r: [f32; 4]| [ox + r[0], oy + r[1], r[2], r[3]];
        let local = [mouse[0] - ox, mouse[1] - oy];
        let rows = state.view.rows();
        let top = menu_top(rows);

        // The main panel under the item area and the crafting grid, and the top bar over it.
        let panel_h = if inventory { top + 62.0 - PANEL_Y } else { top + 30.0 - PANEL_Y };
        panel(batch, [ox + PANEL_X, oy + PANEL_Y, PANEL_W, panel_h], PANEL_TINT);
        panel(batch, at(TOP_BAR), TOP_BAR_TINT);
        let title = if state.view.name.is_empty() { lang(host, "container.occultism.storage_actuator") } else { state.view.name.clone() };
        let w = batch.text_width(&title);
        batch.text(&title, [ox + (WIDTH - w) / 2.0, oy + 5.0], 0xFFFFFFFF, true);

        // The search field, Occultism's sprite, and the buttons beside it.
        if !batch.sprite(SEARCH_SPRITE, [0.0, 0.0, 96.0, 14.0], at(SEARCH), None) {
            batch.fill(at(SEARCH), 0xFF000000);
            batch.fill(at([SEARCH[0] + 1.0, SEARCH[1] + 1.0, SEARCH[2] - 2.0, SEARCH[3] - 2.0]), FIELD_TINT);
        }
        let search = state.view.search();
        let caret = if state.typing && std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() / 400) % 2 == 0 { "_" } else { "" };
        let text_at = [ox + SEARCH[0] + 4.0, oy + SEARCH[1] + 3.0];
        if search.is_empty() && !state.typing {
            batch.text(&lang(host, "gui.occultism.storage.search"), text_at, TEXT_DIM, false);
        } else {
            // The end of a long search stays in view.
            let room = ((SEARCH[2] - 10.0) / 6.0) as usize;
            let tail: String = search.chars().rev().take(room).collect::<Vec<_>>().into_iter().rev().collect();
            batch.text(&format!("{tail}{caret}"), text_at, 0xFFFFFFFF, false);
        }
        let hovered_button = button_at(local);
        let sort_label = match state.view.sort() {
            terminal::Sort::Count => "#",
            terminal::Sort::Name => "A",
            terminal::Sort::Mod => "@",
        };
        let rows_label = rows.to_string();
        let labels = ["x", sort_label, if state.view.reversed() { "^" } else { "v" }, rows_label.as_str()];
        for (i, label) in labels.iter().enumerate() {
            button(batch, [ox + BUTTONS[0] + 15.0 * i as f32, oy + BUTTONS[1], 12.0, 12.0], label, hovered_button == Some(i));
        }

        // The item area: Occultism's sprite behind its slots, then the items and their counts.
        let area = [ox + GRID[0] - 1.0, oy + GRID[1] - 1.0, 18.0 * COLUMNS as f32 + 2.0, 18.0 * rows as f32 + 2.0];
        batch.gui_nine_slice(ITEM_AREA_SPRITE, area, 0xFFFFFFFF);
        let hovered = cell(local, rows);
        for i in 0..COLUMNS * rows {
            let (x, y) = (ox + GRID[0] + 18.0 * (i % COLUMNS) as f32, oy + GRID[1] + 18.0 * (i / COLUMNS) as f32);
            slot(batch, [x + 1.0, y + 1.0], SLOT_TINT);
            if let Some((_, e)) = state.view.at(i) {
                batch.item_stack(&e.shown, [x + 1.0, y + 1.0], 16.0);
                if e.count > 1 {
                    let n = terminal::compact(e.count);
                    let w = batch.text_width(&n);
                    batch.text(&n, [x + 17.0 - w, y + 9.0], 0xFFFFFFFF, true);
                }
            }
            if hovered == Some(i) {
                batch.fill([x + 1.0, y + 1.0, 16.0, 16.0], 0x80FFFFFF);
            }
        }

        // The scroll bar, in the panel's right margin.
        let bar = [ox + GRID[0] + 18.0 * COLUMNS as f32 + 2.0, oy + GRID[1], 3.0, 18.0 * rows as f32];
        batch.fill(bar, shade(PANEL_TINT, 0.6));
        let all = state.view.max_row() + rows;
        let thumb = (bar[3] * rows as f32 / all as f32).max(6.0);
        let thumb_y = if state.view.max_row() == 0 { 0.0 } else { (bar[3] - thumb) * state.view.row as f32 / state.view.max_row() as f32 };
        batch.fill([bar[0], bar[1] + thumb_y, bar[2], thumb], shade(PANEL_TINT, 1.7));

        // The upgrade column: a panel beside the item area, a slot per plate the store keeps.
        panel(batch, [ox + UPGRADES[0] - 3.0, oy + UPGRADES[1] - 3.0, 24.0, 18.0 * UPGRADE_SLOTS as f32 + 6.0], PANEL_TINT);
        for s in 0..UPGRADE_SLOTS {
            let (x, y) = (ox + UPGRADES[0], oy + UPGRADES[1] + 18.0 * s as f32);
            slot(batch, [x + 1.0, y + 1.0], SLOT_TINT);
            let tier = data.get(4 + s).copied().unwrap_or(0);
            if tier > 0 && let Some(plate) = crate::game::stack(&*host, &format!("{}{tier}", vault::PLATE_PREFIX), 1) {
                batch.item_stack(&plate, [x + 1.0, y + 1.0], 16.0);
            }
            if upgrade_at(local) == Some(s) {
                batch.fill([x + 1.0, y + 1.0, 16.0, 16.0], 0x80FFFFFF);
            }
        }

        // Its use of capacity, as Occultism's labels: what it holds from the client's copy, its
        // limits from the data slots; and how many types the search shows.
        let get = |i: usize| data.get(i).copied().unwrap_or(0).max(0) as u64;
        let (types, total) = (state.view.types() as u64, state.view.total());
        let filled = lang(host, "gui.occultism.storage_controller.space_info_label_new").replacen("%s", &format!("{:.0}", percent(total, get(3) * 1000)), 1).replace("%%", "%");
        let kinds = lang(host, "gui.occultism.storage_controller.space_info_label_types").replacen("%s", &format!("{:.0}", percent(types, get(1))), 1).replace("%%", "%");
        let numbers = format!("{}/{}  {}/{}", terminal::compact(types), terminal::compact(get(1)), terminal::compact(total), terminal::compact(get(3) * 1000));
        if inventory {
            // Centred under the result, as Occultism's (`storageTypesLabelX`); the numbers are in
            // their tooltip.
            let centre = ox + CRAFT_RESULT[0] + 9.0;
            for (line, y) in [(&filled, LABELS_Y), (&kinds, LABELS_Y + 9.0)] {
                let w = batch.text_width(line);
                batch.text(line, [centre - w / 2.0, oy + top + y], TEXT, false);
            }
            let _ = numbers;
        } else {
            batch.text(&format!("{kinds}  {filled}"), [ox + GRID[0], oy + top + 4.0], TEXT, false);
            batch.text(&numbers, [ox + GRID[0], oy + top + 14.0], TEXT_DIM, false);
        }
        if state.view.shown() != state.view.types() {
            let found = terminal::compact(state.view.shown() as u64);
            let w = batch.text_width(&found);
            batch.text(&found, [ox + SEARCH[0] + SEARCH[2] - w - 4.0, oy + SEARCH[1] + 3.0], 0xFFFFD27F, false);
        }
        if !inventory {
            return;
        }

        // The crafting grid, its arrow and result, and the clear button.
        for i in 0..9 {
            slot(batch, [ox + CRAFT_GRID[0] + 18.0 * (i % 3) as f32, oy + top + CRAFT_GRID[1] + 18.0 * (i / 3) as f32], SLOT_TINT);
        }
        let (rx, ry) = (ox + CRAFT_RESULT[0], oy + top + CRAFT_RESULT[1]);
        batch.fill([rx - 5.0, ry - 5.0, 26.0, 26.0], shade(BUTTON_TINT, 0.55));
        batch.fill([rx - 4.0, ry - 4.0, 25.0, 25.0], shade(BUTTON_TINT, 1.4));
        batch.fill([rx - 4.0, ry - 4.0, 24.0, 24.0], BUTTON_TINT);
        slot(batch, [rx, ry], SLOT_TINT);
        // The arrow: a shaft and a head, in the slots' light edge.
        let (ax, ay) = (ox + CRAFT_ARROW[0], oy + top + CRAFT_ARROW[1]);
        let arrow = shade(SLOT_TINT, 1.5);
        batch.fill([ax, ay + 6.0, 16.0, 4.0], arrow);
        for k in 0..8 {
            let k = k as f32;
            batch.fill([ax + 15.0 + k, ay + k, 1.0, 16.0 - 2.0 * k], arrow);
        }
        button(batch, at([CLEAR_GRID[0], top + CLEAR_GRID[1], CLEAR_GRID[2], CLEAR_GRID[3]]), "x", inside(local, [CLEAR_GRID[0], top + CLEAR_GRID[1], CLEAR_GRID[2], CLEAR_GRID[3]]));

        // The player's inventory, in a panel of its own, with its label.
        panel(batch, at([INVENTORY_PANEL[0], top + INVENTORY_PANEL[1], INVENTORY_PANEL[2], INVENTORY_PANEL[3]]), PANEL_TINT);
        for i in 0..36 {
            let (c, r) = (i % 9, i / 9);
            let y = if r < 3 { INVENTORY[1] + 18.0 * r as f32 } else { INVENTORY[1] + 58.0 };
            slot(batch, [ox + INVENTORY[0] + 18.0 * c as f32, oy + top + y], SLOT_TINT);
        }
    }

    /// A press on the screen over no slot: the search field, the buttons; a menu button when it was
    /// on the item area, an upgrade slot or the clear button.
    pub fn click(&self, at: [f32; 2], right: bool) -> Option<i32> {
        let mut state = lock(&self.state);
        state.typing = false;
        if inside(at, SEARCH) {
            state.typing = true;
            if right {
                state.view.set_search(String::new());
            }
            return None;
        }
        match button_at(at) {
            Some(0) => {
                state.view.set_search(String::new());
                return None;
            }
            Some(1) => {
                let next = if right { state.view.sort().next().next() } else { state.view.sort().next() };
                state.view.set_sort(next);
                return None;
            }
            Some(2) => {
                let reversed = !state.view.reversed();
                state.view.set_reversed(reversed);
                return None;
            }
            Some(_) => {
                // Rows, as Occultism's button: 1 to 9 and round again; the screen takes it on opening.
                let rows = state.view.rows();
                let next = if right { (rows + MAX_ROWS - 2) % MAX_ROWS + 1 } else { rows % MAX_ROWS + 1 };
                state.view.set_rows(next);
                return None;
            }
            None => {}
        }
        if let Some(slot) = upgrade_at(at) {
            return Some(UPGRADE_BUTTON + slot as i32);
        }
        let top = menu_top(state.view.rows());
        if inside(at, [CLEAR_GRID[0], top + CLEAR_GRID[1], CLEAR_GRID[2], CLEAR_GRID[3]]) {
            return Some(vault::CLEAR_GRID_BUTTON);
        }
        let (serial, _) = state.view.at(cell(at, state.view.rows())?)?;
        Some(terminal::button_for(serial, right))
    }

    pub fn scroll(&self, screen: &ScreenView<'_>, amount: f64) -> Flow {
        let mut state = lock(&self.state);
        if !ours(screen, state.remote) {
            return Flow::Continue;
        }
        state.view.scroll(if amount > 0.0 { -1 } else { 1 });
        Flow::Cancel
    }

    /// While the search field has the keys, they type into it instead of acting (E doesn't close
    /// the screen); Escape and Enter let go of it.
    pub fn key(&self, screen: &ScreenView<'_>, key: &str) -> Flow {
        let mut state = lock(&self.state);
        if !ours(screen, state.remote) || !state.typing {
            return Flow::Continue;
        }
        match key {
            "key.keyboard.escape" | "key.keyboard.enter" | "key.keyboard.keypad.enter" => state.typing = false,
            "key.keyboard.backspace" => {
                let mut text = state.view.search().to_string();
                text.pop();
                state.view.set_search(text);
            }
            _ => {}
        }
        Flow::Cancel
    }

    pub fn typed(&self, screen: &ScreenView<'_>, c: char) -> Flow {
        let mut state = lock(&self.state);
        if !ours(screen, state.remote) || !state.typing || c.is_control() {
            return Flow::Continue;
        }
        let text = format!("{}{c}", state.view.search());
        if text.chars().count() <= 48 {
            state.view.set_search(text);
        }
        Flow::Cancel
    }

    /// The tooltip over a grid cell (the item's name, how many are stored, its id), an upgrade slot
    /// (its plate and what it gives, or how to fill it), or a button (Occultism's words).
    pub fn tooltip(&self, host: &dyn ClientHost, at: [f32; 2], data: &[i32], lines: &mut Vec<String>) {
        let key = "gui.occultism.storage_controller";
        let state = lock(&self.state);
        if let Some(b) = button_at(at) {
            match b {
                0 => lines.push(lang(host, &format!("{key}.search.tooltip_clear"))),
                1 => {
                    let kind = match state.view.sort() {
                        terminal::Sort::Count => "amount",
                        terminal::Sort::Name => "name",
                        terminal::Sort::Mod => "mod",
                    };
                    lines.push(lang(host, &format!("{key}.search.tooltip_sort_type_{kind}")));
                }
                2 => lines.push(lang(host, &format!("{key}.search.tooltip_sort_direction_{}", if state.view.reversed() { "up" } else { "down" }))),
                _ => {
                    lines.push(lang(host, &format!("{key}.display.rows")));
                    lines.push(format!("§7{}", lang(host, "gui.occultism.storage.rows_next_open")));
                }
            }
            return;
        }
        if inside(at, SEARCH) {
            lines.push(lang(host, &format!("{key}.search.tooltip@")));
            lines.push(format!("§7{}", lang(host, &format!("{key}.search.tooltip_rightclick"))));
            return;
        }
        let top = menu_top(state.view.rows());
        if inside(at, [CLEAR_GRID[0], top + CLEAR_GRID[1], CLEAR_GRID[2], CLEAR_GRID[3]]) {
            lines.push(lang(host, &format!("{key}.crafting.tooltip_clear")));
            return;
        }
        if inside(at, [LABELS[0], top + LABELS[1], LABELS[2], LABELS[3]]) {
            let get = |i: usize| data.get(i).copied().unwrap_or(0).max(0) as u64;
            let label = lang(host, &format!("{key}.space_info_label"));
            let of = |used: u64, all: u64| label.replacen("%d", &terminal::grouped(used), 1).replacen("%d", &terminal::grouped(all), 1);
            lines.push(of(state.view.total(), get(3) * 1000));
            lines.push(format!("§7{}", of(state.view.types() as u64, get(1))));
            return;
        }
        if let Some(slot) = upgrade_at(at) {
            let tier = data.get(4 + slot).copied().unwrap_or(0);
            match plate_gives(tier) {
                Some((types, items)) => {
                    lines.push(name_of(host, &format!("{}{tier}", vault::PLATE_PREFIX)));
                    let gives = lang(host, "gui.occultism.storage.upgrade_gives")
                        .replacen("%s", &terminal::grouped(u64::from(types)), 1)
                        .replacen("%s", &terminal::grouped(items), 1);
                    lines.push(format!("§7{gives}"));
                    lines.push(format!("§8{}", lang(host, "gui.occultism.storage.upgrade_take")));
                }
                None => {
                    lines.push(lang(host, "gui.occultism.storage.upgrade"));
                    lines.push(format!("§7{}", lang(host, "gui.occultism.storage.upgrade_empty")));
                }
            }
            return;
        }
        let Some((_, e)) = cell(at, state.view.rows()).and_then(|c| state.view.at(c)) else { return };
        lines.push(name_of(host, &e.id));
        lines.push(format!("§7{} {}", terminal::grouped(e.count), lang(host, "gui.occultism.storage.stored")));
        lines.push(format!("§8{}", e.id));
    }
}
