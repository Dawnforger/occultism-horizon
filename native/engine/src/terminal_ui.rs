// The storage terminal on the client: the Actuator's screen, drawn and driven by the player's own
// copy of the store (terminal.rs's `View`), which the server keeps up to date over `occultism:store`.
// Searching, sorting and scrolling never leave the client; a click on the grid is a menu button
// naming what was clicked (`terminal::button_for`), and the server hands it to the player.

use crate::network::UPGRADE_SLOTS;
use crate::terminal::{self, Entry, Message, View, COLUMNS, ROWS};
use crate::vault::{self, ACTUATOR};
use horizon_client::mod_api::*;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// The screen: the spirit's name, a search box and the sort button, the grid and its scroll bar,
/// the column of upgrade slots on the right, the store's use of its capacity, the player's
/// inventory. Items go in by shift-click (or hoppers): the Actuator's input slots stay behind the
/// scenes. The upgrade slots are drawn, not menu slots: their plates live in the store's record.
pub const WIDTH: f32 = 200.0;
pub const HEIGHT: f32 = 229.0;
const SEARCH: [f32; 4] = [8.0, 17.0, 112.0, 11.0];
const SORT: [f32; 4] = [124.0, 17.0, 44.0, 11.0];
const GRID: [f32; 2] = [8.0, 31.0];
const BAR: [f32; 4] = [171.0, 31.0, 4.0, 18.0 * ROWS as f32];
const STATS_Y: f32 = 124.0;
/// The upgrade column: one slot per grid row.
const UPGRADES: [f32; 2] = [180.0, 31.0];
/// Menu buttons 1 to 5: take the plate out of that upgrade slot.
const UPGRADE_BUTTON: i32 = 1;

/// The upgrade slot a menu button names.
pub fn upgrade_click(button: i32) -> Option<usize> {
    let slot = button.checked_sub(UPGRADE_BUTTON)?;
    (0..UPGRADE_SLOTS as i32).contains(&slot).then_some(slot as usize)
}

fn upgrade_at(at: [f32; 2]) -> Option<usize> {
    let (x, y) = (at[0] - UPGRADES[0], at[1] - UPGRADES[1]);
    (x >= 0.0 && x < 18.0 && y >= 0.0).then(|| (y / 18.0) as usize).filter(|s| *s < UPGRADE_SLOTS)
}

/// A plate's tier as it gives capacity: types, items.
fn plate_gives(tier: i32) -> Option<(u32, u64)> {
    let base = crate::storage::Limits::with_plates(&[]);
    let with = crate::storage::Limits::with_plates(&[tier as u8]);
    (with != base).then(|| (with.types - base.types, with.total - base.total))
}
const INVENTORY_LABEL_Y: f32 = 136.0;
pub const INVENTORY_Y: f32 = 147.0;
/// How often a changing store re-sorts the grid: a big store's sort is a few milliseconds.
const RESORT: Duration = Duration::from_millis(200);

#[derive(Default)]
pub struct Ui {
    state: Mutex<State>,
}

#[derive(Default)]
struct State {
    view: View<ItemStack>,
    typing: bool,
    sorted: Option<Instant>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn inside(at: [f32; 2], r: [f32; 4]) -> bool {
    (r[0]..r[0] + r[2]).contains(&at[0]) && (r[1]..r[1] + r[3]).contains(&at[1])
}

/// The grid cell at `at` (GUI pixels from the screen's top left).
fn cell(at: [f32; 2]) -> Option<usize> {
    let (x, y) = (at[0] - GRID[0], at[1] - GRID[1]);
    let (c, r) = ((x / 18.0).floor(), (y / 18.0).floor());
    (x >= 0.0 && y >= 0.0 && (c as usize) < COLUMNS && (r as usize) < ROWS).then(|| r as usize * COLUMNS + c as usize)
}

/// An item's name as the player reads it: its custom name isn't read yet, so its item's.
fn name_of(id: &str) -> String {
    let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
    for kind in ["item", "block"] {
        let key = format!("{kind}.{ns}.{path}");
        let name = horizon_client::lang::get(&key);
        if name != key {
            return name.to_string();
        }
    }
    path.replace('_', " ")
}

/// The default font's width of `text`, for right-aligning counts.
fn width(text: &str) -> f32 {
    text.chars().map(|c| if matches!(c, '.' | ',' | '!' | 'i' | 'l') { 2.0 } else { 6.0 }).sum()
}

fn ours(screen: &NativeScreenView<'_>) -> bool {
    screen.menu == Some(vault::MENU)
}

impl Ui {
    pub fn screen() -> NativeScreen {
        NativeScreen {
            width: WIDTH,
            height: HEIGHT,
            slots: vec![None; vault::SLOT_COUNT],
            inventory_y: INVENTORY_Y,
            inventory_x: 8.0,
            own_labels: true,
            own_background: false,
        }
    }

    /// A message from the server about the store being looked at.
    pub fn receive(&self, data: &[u8]) {
        let Some(message) = terminal::decode(data) else { return };
        let mut state = lock(&self.state);
        if matches!(message, Message::Begin(_)) {
            state.sorted = None;
        }
        state.view.apply(message, |key, count| {
            let shown = vault::stack_of(key, 1)?;
            let id = shown.item.to_string();
            Some(Entry { count, name: name_of(&id).to_lowercase(), id, shown })
        });
    }

    pub fn closed(&self) {
        let mut state = lock(&self.state);
        let (search, sort) = (state.view.search().to_string(), state.view.sort());
        *state = State::default();
        state.view.set_search(search);
        state.view.set_sort(sort);
    }

    pub fn draw(&self, host: &dyn Host, batch: &mut HudBatch, origin: [f32; 2], data: &[i32]) {
        let mut state = lock(&self.state);
        if state.view.stale() && state.sorted.is_none_or(|t| t.elapsed() >= RESORT) {
            state.view.rebuild();
            state.sorted = Some(Instant::now());
        }
        let [ox, oy] = origin;
        let at = |r: [f32; 4]| [ox + r[0], oy + r[1], r[2], r[3]];
        let mouse = host.cursor();
        let local = [mouse[0] - ox, mouse[1] - oy];

        let title = if state.view.name.is_empty() { horizon_client::lang::get("container.occultism.storage_actuator").to_string() } else { state.view.name.clone() };
        host.text(batch, &title, [ox + 8.0, oy + 6.0], 0x6A3D9A, false);
        host.text(batch, &horizon_client::lang::get("container.inventory"), [ox + 8.0, oy + INVENTORY_LABEL_Y], 0x404040, false);

        // The search box, and the sort button.
        host.fill(batch, at(SEARCH), if state.typing { 0xFFFFFFFF } else { 0xFFA0A0A0 });
        host.fill(batch, at([SEARCH[0] + 1.0, SEARCH[1] + 1.0, SEARCH[2] - 2.0, SEARCH[3] - 2.0]), 0xFF000000);
        let search = state.view.search();
        let caret = if state.typing && std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() / 400) % 2 == 0 { "_" } else { "" };
        if search.is_empty() && !state.typing {
            host.text(batch, &horizon_client::lang::get("gui.occultism.storage.search"), [ox + SEARCH[0] + 3.0, oy + SEARCH[1] + 2.0], 0x808080, false);
        } else {
            // The end of a long search stays in view.
            let room = ((SEARCH[2] - 10.0) / 6.0) as usize;
            let tail: String = search.chars().rev().take(room).collect::<Vec<_>>().into_iter().rev().collect();
            host.text(batch, &format!("{tail}{caret}"), [ox + SEARCH[0] + 3.0, oy + SEARCH[1] + 2.0], 0xE0E0E0, false);
        }
        host.fill(batch, at(SORT), 0xFF6A3D9A);
        let sort = horizon_client::lang::get(&format!("gui.occultism.storage.sort.{}", state.view.sort().name()));
        host.text(batch, &sort, [ox + SORT[0] + (SORT[2] - width(&sort)) / 2.0, oy + SORT[1] + 2.0], 0xFFFFFF, false);

        // The grid: slot frames as vanilla's, the items, their counts.
        let hovered = cell(local);
        for i in 0..COLUMNS * ROWS {
            let (x, y) = (ox + GRID[0] + 18.0 * (i % COLUMNS) as f32, oy + GRID[1] + 18.0 * (i / COLUMNS) as f32);
            host.fill(batch, [x, y, 18.0, 18.0], 0xFF373737);
            host.fill(batch, [x + 1.0, y + 1.0, 17.0, 17.0], 0xFFFFFFFF);
            host.fill(batch, [x + 1.0, y + 1.0, 16.0, 16.0], 0xFF8B8B8B);
            if let Some((_, e)) = state.view.at(i) {
                host.item_stack(batch, &e.shown, [x + 1.0, y + 1.0], 16.0);
                if e.count > 1 {
                    let n = terminal::compact(e.count);
                    host.text(batch, &n, [x + 17.0 - width(&n), y + 9.0], 0xFFFFFF, true);
                }
            }
            if hovered == Some(i) {
                host.fill(batch, [x + 1.0, y + 1.0, 16.0, 16.0], 0x80FFFFFF);
            }
        }

        // The upgrade column: a slot per plate the store keeps, its plate drawn in it.
        for slot in 0..UPGRADE_SLOTS {
            let (x, y) = (ox + UPGRADES[0], oy + UPGRADES[1] + 18.0 * slot as f32);
            host.fill(batch, [x, y, 18.0, 18.0], 0xFF373737);
            host.fill(batch, [x + 1.0, y + 1.0, 17.0, 17.0], 0xFFFFFFFF);
            host.fill(batch, [x + 1.0, y + 1.0, 16.0, 16.0], 0xFF6A5A7A);
            let tier = data.get(4 + slot).copied().unwrap_or(0);
            if tier > 0 {
                let plate = ItemStack { item: format!("{}{tier}", vault::PLATE_PREFIX).as_str().into(), count: 1, damage: 0, extra: None };
                host.item_stack(batch, &plate, [x + 1.0, y + 1.0], 16.0);
            }
            if upgrade_at(local) == Some(slot) {
                host.fill(batch, [x + 1.0, y + 1.0, 16.0, 16.0], 0x80FFFFFF);
            }
        }

        // The scroll bar: its thumb's share of the rows shown.
        host.fill(batch, at(BAR), 0xFF373737);
        let rows = state.view.max_row() + ROWS;
        let thumb = (BAR[3] * ROWS as f32 / rows as f32).max(6.0);
        let top = if state.view.max_row() == 0 { 0.0 } else { (BAR[3] - thumb) * state.view.row as f32 / state.view.max_row() as f32 };
        host.fill(batch, [ox + BAR[0], oy + BAR[1] + top, BAR[2], thumb], 0xFFC6C6C6);

        // Its use of capacity: what it holds from the client's copy, its limits from the data slots.
        let get = |i: usize| data.get(i).copied().unwrap_or(0).max(0) as u64;
        let line = format!(
            "{}/{} types  {}/{}",
            terminal::compact(state.view.types() as u64),
            terminal::compact(get(1)),
            terminal::compact(state.view.total()),
            terminal::compact(get(3) * 1000)
        );
        host.text(batch, &line, [ox + 8.0, oy + STATS_Y], 0x404040, false);
        if state.view.shown() != state.view.types() {
            let found = format!("{}", terminal::compact(state.view.shown() as u64));
            host.text(batch, &found, [ox + 168.0 - width(&found), oy + STATS_Y], 0x6A3D9A, false);
        }
    }

    /// A press on the screen over no slot: a menu button when it was on the grid.
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
        if inside(at, SORT) {
            let next = if right { state.view.sort().next().next() } else { state.view.sort().next() };
            state.view.set_sort(next);
            return None;
        }
        if let Some(slot) = upgrade_at(at) {
            return Some(UPGRADE_BUTTON + slot as i32);
        }
        let (serial, _) = state.view.at(cell(at)?)?;
        Some(terminal::button_for(serial, right))
    }

    pub fn scroll(&self, screen: &NativeScreenView<'_>, amount: f64) -> Flow {
        if !ours(screen) {
            return Flow::Continue;
        }
        lock(&self.state).view.scroll(if amount > 0.0 { -1 } else { 1 });
        Flow::Cancel
    }

    /// While the search box has the keys, they type into it instead of acting (E doesn't close
    /// the screen); Escape and Enter let go of it.
    pub fn key(&self, screen: &NativeScreenView<'_>, key: &str) -> Flow {
        if !ours(screen) {
            return Flow::Continue;
        }
        let mut state = lock(&self.state);
        if !state.typing {
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

    pub fn typed(&self, screen: &NativeScreenView<'_>, c: char) -> Flow {
        if !ours(screen) {
            return Flow::Continue;
        }
        let mut state = lock(&self.state);
        if !state.typing || c.is_control() {
            return Flow::Continue;
        }
        let text = format!("{}{c}", state.view.search());
        if text.chars().count() <= 48 {
            state.view.set_search(text);
        }
        Flow::Cancel
    }

    /// The tooltip over a grid cell: the item's name, how many are stored, its id. Over an upgrade
    /// slot: its plate and what it gives, or how to fill it.
    pub fn tooltip(&self, at: [f32; 2], data: &[i32], lines: &mut Vec<String>) {
        if let Some(slot) = upgrade_at(at) {
            let tier = data.get(4 + slot).copied().unwrap_or(0);
            match plate_gives(tier) {
                Some((types, items)) => {
                    lines.push(name_of(&format!("{}{tier}", vault::PLATE_PREFIX)));
                    let gives = horizon_client::lang::get("gui.occultism.storage.upgrade_gives")
                        .replacen("%s", &terminal::grouped(u64::from(types)), 1)
                        .replacen("%s", &terminal::grouped(items), 1);
                    lines.push(format!("§7{gives}"));
                    lines.push(format!("§8{}", horizon_client::lang::get("gui.occultism.storage.upgrade_take")));
                }
                None => {
                    lines.push(horizon_client::lang::get("gui.occultism.storage.upgrade").to_string());
                    lines.push(format!("§7{}", horizon_client::lang::get("gui.occultism.storage.upgrade_empty")));
                }
            }
            return;
        }
        let state = lock(&self.state);
        let Some((_, e)) = cell(at).and_then(|c| state.view.at(c)) else { return };
        lines.push(name_of(&e.id));
        lines.push(format!("§7{} {}", terminal::grouped(e.count), horizon_client::lang::get("gui.occultism.storage.stored")));
        lines.push(format!("§8{}", e.id));
    }
}

/// Whether `block`'s screen is the terminal.
pub fn is_terminal(block: &str) -> bool {
    block == ACTUATOR
}
