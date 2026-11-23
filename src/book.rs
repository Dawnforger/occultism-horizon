// The Dictionary of Spirits: Occultism's guide book (its Modonomicon book, `book_data.rs`, written by
// tools/build_book.py), on a screen of the mod's own. As Modonomicon shows it: a category per tab,
// each a map of its entries, linked to the ones they follow; an entry opens as a book, two pages to
// a spread. Text pages, item spotlights, crafting, spirit fire, smelting and ritual recipes,
// pentacles (with a button that shows the circle in the world, pentacle_preview.rs), creatures and
// Occultism's pictures, drawn here; recipes cycle through the items their ingredients take.
// Different from Modonomicon: every entry is open from the start, and a page whose recipe or item
// this port doesn't have yet says so.

use crate::book_data::BOOK;
use crate::game::lock;
use mod_api::*;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

pub const DICTIONARY: &str = "occultism:dictionary_of_spirits";

/// The book, parsed once.
fn book() -> &'static Value {
    static PARSED: OnceLock<Value> = OnceLock::new();
    PARSED.get_or_init(|| serde_json::from_str(BOOK).unwrap_or(Value::Null))
}

fn categories() -> &'static [Value] {
    book()["categories"].as_array().map_or(&[], Vec::as_slice)
}

fn entries(cat: usize) -> &'static [Value] {
    categories().get(cat).and_then(|c| c["entries"].as_array()).map_or(&[], Vec::as_slice)
}

/// A pentacle's parts, from its page (for pentacle_preview.rs): each place's offset from the bowl
/// and the blocks it accepts.
pub fn parts(pentacle: &str) -> Vec<([i32; 3], Vec<String>)> {
    let page = categories()
        .iter()
        .flat_map(|c| c["entries"].as_array().into_iter().flatten())
        .flat_map(|e| e["pages"].as_array().into_iter().flatten())
        .find(|p| p["t"] == "multiblock" && p["pentacle"] == pentacle);
    page.and_then(|p| p["parts"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|part| {
            let n = |i: usize| part.get(i).and_then(Value::as_i64).map(|v| v as i32);
            let accepts = part.get(3)?.as_array()?.iter().filter_map(|b| b.as_str().map(str::to_string)).collect();
            Some(([n(0)?, n(1)?, n(2)?], accepts))
        })
        .collect()
}

fn str_of(v: &Value) -> &str {
    v.as_str().unwrap_or("")
}

const PARCHMENT: u32 = 0xFFF2E6C9;
const PARCHMENT_EDGE: u32 = 0xFF8A6A3A;
const INK: u32 = 0xFF3A2A1A;
const INK_DIM: u32 = 0xFF8A7A6A;
const LINK: u32 = 0xFF2A4FA8;
const SLATE: u32 = 0xFF26262E;
const SLATE_EDGE: u32 = 0xFF0E0E12;
const LINE: u32 = 0xFFB8A27A;
/// A book spread: two pages, as Modonomicon's double page.
const PAGE_W: f32 = 146.0;
const PAGE_H: f32 = 182.0;
const MARGIN: f32 = 12.0;
const TEXT_W: f32 = PAGE_W - 2.0 * MARGIN;
const LINE_H: f32 = 9.0;
/// The map's grid: Modonomicon's entry positions are in these steps.
const NODE_STEP: f32 = 30.0;

/// What a click on something drawn does.
#[derive(Clone, Debug, PartialEq)]
enum Hit {
    Tab(usize),
    Entry(usize, usize),
    Link(String),
    Turn(i32),
    Back,
    Visualize(String),
}

#[derive(Clone, Debug, PartialEq)]
enum View {
    Map,
    Entry { entry: usize, spread: usize },
}

/// One drawn page of an entry: a page of the book's (its header drawn), or the rest of a page's text
/// that didn't fit.
#[derive(Clone)]
struct Sheet {
    page: Option<usize>,
    title: bool,
    lines: Vec<Line>,
}

#[derive(Clone, Default)]
struct Line {
    words: Vec<Word>,
}

#[derive(Clone)]
struct Word {
    text: String,
    x: f32,
    colour: u32,
    bold: bool,
    link: Option<String>,
}

struct State {
    open: bool,
    cat: usize,
    view: View,
    /// Where the map is scrolled to, per category.
    pan: HashMap<usize, [f32; 2]>,
    history: Vec<(usize, View)>,
    hits: Vec<([f32; 4], Hit)>,
    sheets: HashMap<(usize, usize), Vec<Sheet>>,
    ingredients: HashMap<String, Vec<String>>,
    /// A pentacle the reader asked to see in the world, for pentacle_preview.rs.
    visualize: Option<String>,
}

#[derive(Default)]
pub struct Book {
    state: Mutex<Option<State>>,
}

/// Seconds since the book was first drawn: what a recipe slot cycles its items by.
fn seconds() -> u64 {
    static START: OnceLock<std::time::Instant> = OnceLock::new();
    START.get_or_init(std::time::Instant::now).elapsed().as_secs()
}

/// A GUI sprite, by its texture path (`gui/sprites/...`, Horizon's name for it), or else by its
/// atlas id (the path without `gui/sprites/`, Sunrise's).
fn sprite(draw: &mut dyn DrawContext, name: &str, src: [f32; 4], dst: [f32; 4]) -> bool {
    if draw.sprite(name, src, dst, None) {
        return true;
    }
    let atlas = name.replacen("gui/sprites/", "", 1);
    atlas != name && draw.sprite(&atlas, src, dst, None)
}

fn inside(r: [f32; 4], p: [f32; 2]) -> bool {
    p[0] >= r[0] && p[1] >= r[1] && p[0] < r[0] + r[2] && p[1] < r[1] + r[3]
}

fn lang(host: &dyn ClientHost, key: &str, fallback: &str) -> String {
    host.translate(key).unwrap_or_else(|| fallback.to_string())
}

fn item_name(host: &dyn ClientHost, id: &str) -> String {
    let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
    ["item", "block", "entity"].iter().find_map(|kind| host.translate(&format!("{kind}.{ns}.{path}"))).unwrap_or_else(|| path.replace('_', " "))
}

impl Book {
    pub fn open(&self) {
        let mut guard = lock(&self.state);
        let state = guard.get_or_insert_with(|| State {
            open: false,
            cat: 0,
            view: View::Map,
            pan: HashMap::new(),
            history: Vec::new(),
            hits: Vec::new(),
            sheets: HashMap::new(),
            ingredients: HashMap::new(),
            visualize: None,
        });
        state.open = true;
    }

    pub fn is_open(&self) -> bool {
        lock(&self.state).as_ref().is_some_and(|s| s.open)
    }

    fn ours(&self, host: &dyn ClientHost, screen: &ScreenView<'_>) -> bool {
        self.is_open() && screen.id == "mod" && host.mod_screen() == Some(crate::terminal_ui::MOD_ID)
    }

    pub fn closed(&self, screen: &ScreenView<'_>) {
        if screen.id == "mod"
            && let Some(state) = lock(&self.state).as_mut()
        {
            state.open = false;
        }
    }

    /// The pentacle the reader asked to see, once.
    pub fn take_visualize(&self) -> Option<String> {
        lock(&self.state).as_mut().and_then(|s| s.visualize.take())
    }

    pub fn draw(&self, host: &dyn ClientHost, draw: &mut dyn DrawContext, screen: &ScreenView<'_>) {
        if !self.ours(host, screen) {
            return;
        }
        let mut guard = lock(&self.state);
        let Some(state) = guard.as_mut() else { return };
        state.hits.clear();
        let mut tip = None;
        match state.view.clone() {
            View::Map => draw_map(host, draw, screen, state, &mut tip),
            View::Entry { entry, spread } => draw_entry(host, draw, screen, state, entry, spread, &mut tip),
        }
        if let Some(lines) = tip {
            tooltip(draw, &lines, screen.mouse);
        }
    }

    pub fn press(&self, host: &dyn ClientHost, screen: &ScreenView<'_>, button: i32) -> Flow {
        if !self.ours(host, screen) {
            return Flow::Continue;
        }
        let mut guard = lock(&self.state);
        let Some(state) = guard.as_mut() else { return Flow::Continue };
        if button == 1 {
            back(state);
            return Flow::Cancel;
        }
        let hit = state.hits.iter().rev().find(|(r, _)| inside(*r, screen.mouse)).map(|(_, h)| h.clone());
        match hit {
            Some(Hit::Tab(cat)) => {
                state.cat = cat;
                state.view = View::Map;
                state.history.clear();
            }
            Some(Hit::Entry(cat, entry)) => open_entry(state, cat, entry),
            Some(Hit::Link(target)) => follow(state, &target),
            Some(Hit::Turn(by)) => {
                if let View::Entry { spread, .. } = &mut state.view {
                    *spread = (*spread as i32 + by).max(0) as usize;
                }
            }
            Some(Hit::Back) => back(state),
            Some(Hit::Visualize(pentacle)) => {
                state.visualize = Some(pentacle);
                drop(guard);
                return Flow::Cancel;
            }
            None => {}
        }
        Flow::Cancel
    }

    pub fn scroll(&self, host: &dyn ClientHost, screen: &ScreenView<'_>, amount: [f64; 2]) -> Flow {
        if !self.ours(host, screen) {
            return Flow::Continue;
        }
        let mut guard = lock(&self.state);
        let Some(state) = guard.as_mut() else { return Flow::Continue };
        match &mut state.view {
            View::Map => {
                let pan = state.pan.entry(state.cat).or_insert([0.0, 0.0]);
                pan[0] += amount[0] as f32 * 20.0;
                pan[1] += amount[1] as f32 * 20.0;
            }
            View::Entry { spread, .. } => {
                *spread = (*spread as i32 - amount[1].signum() as i32).max(0) as usize;
            }
        }
        Flow::Cancel
    }

    pub fn key(&self, host: &dyn ClientHost, screen: &ScreenView<'_>, key: &str) -> Flow {
        if !self.ours(host, screen) {
            return Flow::Continue;
        }
        let mut guard = lock(&self.state);
        let Some(state) = guard.as_mut() else { return Flow::Continue };
        let step = |state: &mut State, dx: f32, dy: f32| {
            let pan = state.pan.entry(state.cat).or_insert([0.0, 0.0]);
            pan[0] += dx;
            pan[1] += dy;
        };
        match (key, &mut state.view) {
            ("key.keyboard.backspace", _) => back(state),
            ("key.keyboard.left" | "key.keyboard.a", View::Entry { spread, .. }) => *spread = spread.saturating_sub(1),
            ("key.keyboard.right" | "key.keyboard.d", View::Entry { spread, .. }) => *spread += 1,
            ("key.keyboard.left" | "key.keyboard.a", View::Map) => step(state, 20.0, 0.0),
            ("key.keyboard.right" | "key.keyboard.d", View::Map) => step(state, -20.0, 0.0),
            ("key.keyboard.up" | "key.keyboard.w", View::Map) => step(state, 0.0, 20.0),
            ("key.keyboard.down" | "key.keyboard.s", View::Map) => step(state, 0.0, -20.0),
            _ => return Flow::Continue,
        }
        Flow::Cancel
    }
}

fn open_entry(state: &mut State, cat: usize, entry: usize) {
    state.history.push((state.cat, state.view.clone()));
    state.cat = cat;
    state.view = View::Entry { entry, spread: 0 };
}

fn back(state: &mut State) {
    match state.history.pop() {
        Some((cat, view)) => {
            state.cat = cat;
            state.view = view;
        }
        None => state.view = View::Map,
    }
}

/// A link to an entry (`category/entry`) or a category (`category`).
fn follow(state: &mut State, target: &str) {
    let (cat_id, entry_id) = target.split_once('/').map_or((target, None), |(c, e)| (c, Some(e)));
    let cat_id = cat_id.split(':').next_back().unwrap_or(cat_id);
    let Some(cat) = categories().iter().position(|c| str_of(&c["id"]) == cat_id) else { return };
    match entry_id.and_then(|e| entries(cat).iter().position(|x| str_of(&x["id"]) == format!("{cat_id}/{e}"))) {
        Some(entry) => open_entry(state, cat, entry),
        None => {
            state.history.push((state.cat, state.view.clone()));
            state.cat = cat;
            state.view = View::Map;
        }
    }
}

fn tooltip(draw: &mut dyn DrawContext, lines: &[String], at: [f32; 2]) {
    let w = lines.iter().map(|l| draw.text_width(l)).fold(0.0, f32::max) + 8.0;
    let h = lines.len() as f32 * 10.0 + 6.0;
    let p = [at[0] + 10.0, at[1] - 12.0];
    draw.fill([p[0], p[1], w, h], 0xF0100010);
    draw.fill([p[0], p[1], w, 1.0], 0xFF5000A0);
    for (n, l) in lines.iter().enumerate() {
        draw.text(l, [p[0] + 4.0, p[1] + 4.0 + n as f32 * 10.0], if n == 0 { 0xFFFFFFFF } else { 0xFFAAAAAA }, true);
    }
}

fn icon(host: &dyn ClientHost, draw: &mut dyn DrawContext, icon: &Value, at: [f32; 2], size: f32) {
    if let Some(name) = icon["sprite"]["id"].as_str() {
        let (w, h) = (icon["sprite"]["w"].as_f64().unwrap_or(16.0) as f32, icon["sprite"]["h"].as_f64().unwrap_or(16.0) as f32);
        sprite(draw, name, [0.0, 0.0, w, h], [at[0], at[1], size, size]);
    } else if let Some(stack) = crate::game::stack(host, str_of(&icon["item"]), 1) {
        draw.item_stack(&stack, at, size);
    }
}

/// The tabs, one per category, down the left of the window, as the advancements screen's.
fn draw_tabs(host: &dyn ClientHost, draw: &mut dyn DrawContext, state: &mut State, window: [f32; 4]) {
    let count = categories().len();
    for (n, c) in categories().iter().enumerate() {
        let place = if n == 0 { "top" } else if n + 1 == count { "bottom" } else { "middle" };
        let selected = if n == state.cat { "_selected" } else { "" };
        let r = [window[0] - 28.0, window[1] + 4.0 + n as f32 * 28.0, 32.0, 28.0];
        if !sprite(draw, &format!("gui/sprites/advancements/tab_left_{place}{selected}"), [0.0, 0.0, 32.0, 28.0], r) {
            draw.fill(r, if n == state.cat { SLATE } else { 0xFF404048 });
        }
        icon(host, draw, &c["icon"], [r[0] + 10.0, r[1] + 6.0], 16.0);
        state.hits.push((r, Hit::Tab(n)));
    }
}

fn frame(draw: &mut dyn DrawContext, r: [f32; 4], fill: u32, edge: u32) {
    draw.fill([r[0] - 2.0, r[1] - 2.0, r[2] + 4.0, r[3] + 4.0], edge);
    draw.fill(r, fill);
}

fn draw_map(host: &dyn ClientHost, draw: &mut dyn DrawContext, screen: &ScreenView<'_>, state: &mut State, tip: &mut Option<Vec<String>>) {
    let s = screen.rect;
    let w = (s[2] - 80.0).clamp(200.0, 420.0);
    let h = (s[3] - 40.0).clamp(150.0, 280.0);
    let window = [(s[0] + (s[2] - w) / 2.0 + 14.0).floor(), (s[1] + (s[3] - h) / 2.0).floor(), w, h];
    draw_tabs(host, draw, state, window);
    frame(draw, window, SLATE, SLATE_EDGE);
    let cat = state.cat;
    let Some(c) = categories().get(cat) else { return };
    let name = format!("{} - {}", str_of(&book()["name"]), str_of(&c["name"]));
    draw.text(&name, [window[0] + 6.0, window[1] + 6.0], 0xFFE0D0B0, true);
    let list = entries(cat);
    // Centred on the entries' middle until scrolled.
    let (min, max) = list.iter().fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), e| {
        let p = [e["x"].as_f64().unwrap_or(0.0) as f32, e["y"].as_f64().unwrap_or(0.0) as f32];
        ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])])
    });
    let pan = *state.pan.entry(cat).or_insert([0.0, 0.0]);
    let origin = [
        window[0] + w / 2.0 - (min[0] + max[0]) / 2.0 * NODE_STEP + pan[0],
        window[1] + h / 2.0 + 6.0 - (min[1] + max[1]) / 2.0 * NODE_STEP + pan[1],
    ];
    let at = |e: &Value| [origin[0] + e["x"].as_f64().unwrap_or(0.0) as f32 * NODE_STEP, origin[1] + e["y"].as_f64().unwrap_or(0.0) as f32 * NODE_STEP];
    let clip = [window[0], window[1] + 16.0, w, h - 16.0];
    let clipped = draw.push_clip(clip).is_ok();
    // Lines first, from each entry to those it follows.
    for e in list {
        let to = at(e);
        for p in e["parents"].as_array().into_iter().flatten() {
            if let Some(parent) = list.iter().find(|x| x["id"] == *p) {
                let from = at(parent);
                let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
                let len = (dx * dx + dy * dy).sqrt().max(0.001);
                let (nx, ny) = (-dy / len, dx / len);
                draw.quad([[from[0] + nx, from[1] + ny], [to[0] + nx, to[1] + ny], [to[0] - nx, to[1] - ny], [from[0] - nx, from[1] - ny]], LINE);
            }
        }
    }
    for (n, e) in list.iter().enumerate() {
        let c = at(e);
        let r = [c[0] - 13.0, c[1] - 13.0, 26.0, 26.0];
        let missing = e["missing"].as_bool().unwrap_or(false);
        let frame_sprite = if missing { "gui/sprites/advancements/task_frame_unobtained" } else { "gui/sprites/advancements/task_frame_obtained" };
        if !sprite(draw, frame_sprite, [0.0, 0.0, 26.0, 26.0], r) {
            frame(draw, r, 0xFF6A5A3A, 0xFF1A1A1A);
        }
        icon(host, draw, &e["icon"], [c[0] - 8.0, c[1] - 8.0], 16.0);
        if missing {
            draw.fill(r, 0x80000000);
        }
        if inside(clip, screen.mouse) && inside(r, screen.mouse) {
            let mut lines = vec![str_of(&e["name"]).to_string()];
            if !str_of(&e["desc"]).is_empty() {
                lines.push(str_of(&e["desc"]).to_string());
            }
            if missing {
                lines.push(lang(host, "book.occultism.not_in_port", "Not in this port yet"));
            }
            *tip = Some(lines);
        }
        if inside(clip, [c[0], c[1]]) {
            state.hits.push((r, Hit::Entry(cat, n)));
        }
    }
    if clipped {
        let _ = draw.pop_clip();
    }
}

/// Text runs laid out in lines `width` wide.
fn layout(host: &dyn ClientHost, draw: &dyn DrawContext, runs: &Value, width: f32) -> Vec<Line> {
    let mut lines = vec![Line::default()];
    let mut x = 0.0;
    for run in runs.as_array().into_iter().flatten() {
        let text = match run["it"].as_str() {
            Some(item) if run["s"].is_null() => item_name(host, item),
            _ => str_of(&run["s"]).to_string(),
        };
        let colour = run["c"].as_u64().map_or(INK, |c| 0xFF000000 | c as u32);
        let bold = run["b"].is_number();
        let link = run["l"].as_str().map(str::to_string);
        let colour = if link.is_some() { LINK } else { colour };
        for (n, piece) in text.split('\n').enumerate() {
            if n > 0 {
                lines.push(Line::default());
                x = 0.0;
            }
            for word in piece.split_inclusive(' ') {
                let w = draw.text_width(word) + if bold { 1.0 } else { 0.0 };
                if x + w > width && x > 0.0 && !word.trim().is_empty() {
                    lines.push(Line::default());
                    x = 0.0;
                }
                let word = if x == 0.0 { word.trim_start() } else { word };
                if word.is_empty() {
                    continue;
                }
                let w = draw.text_width(word) + if bold { 1.0 } else { 0.0 };
                if let Some(line) = lines.last_mut() {
                    line.words.push(Word { text: word.to_string(), x, colour, bold, link: link.clone() });
                }
                x += w;
            }
        }
    }
    lines
}

/// How tall a page's own part is, above its text.
fn header_height(page: &Value) -> f32 {
    let title = if str_of(&page["title"]).is_empty() { 0.0 } else { 14.0 };
    title + match str_of(&page["t"]) {
        "spotlight" => 40.0,
        "crafting" => page["recipes"].as_array().map_or(1, Vec::len) as f32 * 64.0,
        "fire" | "smelting" => 30.0,
        "ritual" => 136.0,
        "multiblock" => 112.0,
        "entity" => 84.0,
        "image" => 100.0,
        "link" => 18.0,
        "missing" => 12.0,
        _ => 0.0,
    }
}

/// An entry's pages as sheets: each page, its text continued on sheets of its own when it doesn't fit.
fn sheets(host: &dyn ClientHost, draw: &dyn DrawContext, entry: &Value) -> Vec<Sheet> {
    let mut out = Vec::new();
    let room = PAGE_H - 2.0 * MARGIN;
    for (n, page) in entry["pages"].as_array().into_iter().flatten().enumerate() {
        let first = n == 0;
        let top = header_height(page) + if first { 18.0 } else { 0.0 };
        let mut lines = layout(host, draw, &page["text"], TEXT_W);
        while lines.last().is_some_and(|l| l.words.is_empty()) {
            lines.pop();
        }
        let fits = (((room - top) / LINE_H).floor().max(0.0)) as usize;
        let rest = if lines.len() > fits { lines.split_off(fits) } else { Vec::new() };
        out.push(Sheet { page: Some(n), title: first, lines });
        for chunk in rest.chunks(((room / LINE_H).floor() as usize).max(1)) {
            out.push(Sheet { page: None, title: false, lines: chunk.to_vec() });
        }
    }
    if out.is_empty() {
        out.push(Sheet { page: None, title: true, lines: Vec::new() });
    }
    out
}

fn draw_entry(
    host: &dyn ClientHost,
    draw: &mut dyn DrawContext,
    screen: &ScreenView<'_>,
    state: &mut State,
    entry: usize,
    spread: usize,
    tip: &mut Option<Vec<String>>,
) {
    let cat = state.cat;
    let Some(e) = entries(cat).get(entry) else {
        state.view = View::Map;
        return;
    };
    if !state.sheets.contains_key(&(cat, entry)) {
        let laid = sheets(host, draw, e);
        state.sheets.insert((cat, entry), laid);
    }
    let all = state.sheets[&(cat, entry)].clone();
    let spreads = all.len().div_ceil(2);
    let spread = spread.min(spreads.saturating_sub(1));
    state.view = View::Entry { entry, spread };
    let s = screen.rect;
    let book = [(s[0] + (s[2] - 2.0 * PAGE_W) / 2.0).floor(), (s[1] + (s[3] - PAGE_H) / 2.0).floor(), 2.0 * PAGE_W, PAGE_H];
    frame(draw, book, PARCHMENT, PARCHMENT_EDGE);
    draw.fill([book[0] + PAGE_W - 1.0, book[1], 2.0, PAGE_H], 0xFFC8B48A);
    for side in 0..2 {
        if let Some(sheet) = all.get(spread * 2 + side) {
            let origin = [book[0] + side as f32 * PAGE_W + MARGIN, book[1] + MARGIN];
            draw_sheet(host, draw, screen, state, e, sheet, origin, tip);
        }
    }
    // Turning the page, and back to the map.
    let button = |draw: &mut dyn DrawContext, label: &str, r: [f32; 4], hit: Hit, state: &mut State| {
        let hover = inside(r, screen.mouse);
        draw.fill(r, if hover { 0xFFD8C8A0 } else { 0xFFE6D6B0 });
        let w = draw.text_width(label);
        draw.text(label, [r[0] + (r[2] - w) / 2.0, r[1] + 3.0], INK, false);
        state.hits.push((r, hit));
    };
    let y = book[1] + PAGE_H - 14.0;
    if spread > 0 {
        button(draw, "<", [book[0] + 4.0, y, 14.0, 12.0], Hit::Turn(-1), state);
    }
    if spread + 1 < spreads {
        button(draw, ">", [book[0] + 2.0 * PAGE_W - 18.0, y, 14.0, 12.0], Hit::Turn(1), state);
    }
    let back = lang(host, "book.occultism.back", "Back");
    let bw = draw.text_width(&back) + 8.0;
    button(draw, &back, [book[0] + PAGE_W - bw / 2.0, book[1] + PAGE_H + 4.0, bw, 12.0], Hit::Back, state);
    let n = format!("{}/{}", spread + 1, spreads.max(1));
    let nw = draw.text_width(&n);
    draw.text(&n, [book[0] + PAGE_W - nw / 2.0, y + 2.0], INK_DIM, false);
}

#[allow(clippy::too_many_arguments)]
fn draw_sheet(
    host: &dyn ClientHost,
    draw: &mut dyn DrawContext,
    screen: &ScreenView<'_>,
    state: &mut State,
    entry: &Value,
    sheet: &Sheet,
    origin: [f32; 2],
    tip: &mut Option<Vec<String>>,
) {
    let mut y = origin[1];
    if sheet.title {
        let name = str_of(&entry["name"]);
        let w = draw.text_width(name);
        draw.text(name, [origin[0] + (TEXT_W - w) / 2.0, y], INK, false);
        draw.text(name, [origin[0] + (TEXT_W - w) / 2.0 + 0.5, y], INK, false);
        draw.fill([origin[0] + 10.0, y + 11.0, TEXT_W - 20.0, 1.0], PARCHMENT_EDGE);
        y += 18.0;
    }
    if let Some(n) = sheet.page
        && let Some(page) = entry["pages"].get(n)
    {
        let title = str_of(&page["title"]);
        if !title.is_empty() {
            let w = draw.text_width(title);
            draw.text(title, [origin[0] + (TEXT_W - w) / 2.0, y], INK, false);
            draw.text(title, [origin[0] + (TEXT_W - w) / 2.0 + 0.5, y], INK, false);
            y += 14.0;
        }
        let top = y;
        draw_page_part(host, draw, screen, state, page, [origin[0], top], tip);
        y = top + header_height(page) - if title.is_empty() { 0.0 } else { 14.0 };
    }
    for line in &sheet.lines {
        for word in &line.words {
            let at = [origin[0] + word.x, y];
            draw.text(&word.text, at, word.colour, false);
            if word.bold {
                draw.text(&word.text, [at[0] + 0.5, at[1]], word.colour, false);
            }
            if let Some(link) = &word.link {
                let w = draw.text_width(&word.text);
                draw.fill([at[0], at[1] + 8.0, w, 1.0], LINK);
                state.hits.push(([at[0], at[1], w, LINE_H], Hit::Link(link.clone())));
            }
        }
        y += LINE_H;
    }
}

/// The items an ingredient takes, as the game resolves its JSON (tags included).
fn ingredient(host: &dyn ClientHost, state: &mut State, json: &Value) -> Vec<String> {
    let key = json.to_string();
    if let Some(items) = state.ingredients.get(&key) {
        return items.clone();
    }
    let items = host.recipe_ingredient(&key).ok().flatten().map(|i| i.order).unwrap_or_default();
    state.ingredients.insert(key, items.clone());
    items
}

/// A slot showing one of `items` (a new one each second), with its name on hover.
fn slot(host: &dyn ClientHost, draw: &mut dyn DrawContext, screen: &ScreenView<'_>, items: &[String], count: u32, at: [f32; 2], tip: &mut Option<Vec<String>>) {
    draw.fill([at[0], at[1], 18.0, 18.0], 0xFF8B8B8B);
    draw.fill([at[0] + 1.0, at[1] + 1.0, 16.0, 16.0], 0xFFC6C6C6);
    if items.is_empty() {
        return;
    }
    let item = &items[seconds() as usize % items.len()];
    if let Some(stack) = crate::game::stack(host, item, count.max(1)) {
        draw.item_stack(&stack, [at[0] + 1.0, at[1] + 1.0], 16.0);
        if count > 1 {
            let n = count.to_string();
            let w = draw.text_width(&n);
            draw.text(&n, [at[0] + 17.0 - w, at[1] + 10.0], 0xFFFFFFFF, true);
        }
    }
    if inside([at[0], at[1], 18.0, 18.0], screen.mouse) {
        *tip = Some(vec![item_name(host, item)]);
    }
}

fn arrow(draw: &mut dyn DrawContext, at: [f32; 2]) {
    draw.fill([at[0], at[1] + 7.0, 14.0, 4.0], INK_DIM);
    draw.quad([[at[0] + 12.0, at[1] + 3.0], [at[0] + 20.0, at[1] + 9.0], [at[0] + 12.0, at[1] + 15.0], [at[0] + 12.0, at[1] + 9.0]], INK_DIM);
}

fn small(draw: &mut dyn DrawContext, text: &str, at: [f32; 2], colour: u32) {
    draw.text(text, at, colour, false);
}

fn draw_page_part(
    host: &dyn ClientHost,
    draw: &mut dyn DrawContext,
    screen: &ScreenView<'_>,
    state: &mut State,
    page: &Value,
    at: [f32; 2],
    tip: &mut Option<Vec<String>>,
) {
    let mid = at[0] + TEXT_W / 2.0;
    match str_of(&page["t"]) {
        "spotlight" => {
            let item = str_of(&page["item"]).to_string();
            draw.fill([mid - 15.0, at[1] + 2.0, 30.0, 30.0], 0xFFD8C8A0);
            slot(host, draw, screen, &[item], 1, [mid - 9.0, at[1] + 8.0], tip);
        }
        "crafting" => {
            for (n, recipe) in page["recipes"].as_array().into_iter().flatten().enumerate() {
                let top = at[1] + n as f32 * 64.0 + 2.0;
                let left = mid - 50.0;
                for (row, cells) in recipe["grid"].as_array().into_iter().flatten().enumerate() {
                    for (col, cell) in cells.as_array().into_iter().flatten().enumerate() {
                        let items = if cell.is_null() {
                            Vec::new()
                        } else if let Some(list) = cell.as_array().filter(|l| l.iter().all(Value::is_string)) {
                            list.iter().map(|v| str_of(v).to_string()).collect()
                        } else {
                            ingredient(host, state, cell)
                        };
                        slot(host, draw, screen, &items, 1, [left + col as f32 * 18.0, top + row as f32 * 18.0], tip);
                    }
                }
                arrow(draw, [left + 58.0, top + 18.0]);
                let result = str_of(&recipe["result"]).to_string();
                slot(host, draw, screen, &[result], recipe["count"].as_u64().unwrap_or(1) as u32, [left + 82.0, top + 18.0], tip);
            }
        }
        "fire" | "smelting" => {
            let items = ingredient(host, state, &page["input"]);
            slot(host, draw, screen, &items, 1, [mid - 46.0, at[1] + 4.0], tip);
            let by = if str_of(&page["t"]) == "fire" { "occultism:spirit_fire" } else { "minecraft:furnace" };
            if let Some(stack) = crate::game::stack(host, by, 1) {
                draw.item_stack(&stack, [mid - 8.0, at[1] + 5.0], 16.0);
            }
            arrow(draw, [mid + 12.0, at[1] + 4.0]);
            let result = str_of(&page["result"]).to_string();
            slot(host, draw, screen, &[result], page["count"].as_u64().unwrap_or(1) as u32, [mid + 34.0, at[1] + 4.0], tip);
        }
        "ritual" => draw_ritual(host, draw, screen, state, page, at, tip),
        "multiblock" => draw_pentacle(host, draw, screen, state, page, at, tip),
        "entity" => {
            let kind = str_of(&page["entity"]);
            let r = [mid - 40.0, at[1], mid + 40.0, at[1] + 78.0];
            let view = GuiEntityView::new(r, 30.0 * page["scale"].as_f64().unwrap_or(1.0) as f32, page["offset"].as_f64().unwrap_or(0.0) as f32, GuiEntityLook::At(screen.mouse));
            let _ = draw.entity(&GuiEntity::kind(kind), view);
            let name = item_name(host, kind);
            let w = draw.text_width(&name);
            small(draw, &name, [mid - w / 2.0, at[1] + 76.0], INK_DIM);
        }
        "image" => {
            let images = page["images"].as_array().map_or(&[][..], Vec::as_slice);
            if let Some(image) = images.get((seconds() / 3) as usize % images.len().max(1)) {
                // The whole picture, as large as fits the page.
                let (w, h) = (image["w"].as_f64().unwrap_or(256.0) as f32, image["h"].as_f64().unwrap_or(256.0) as f32);
                let scale = (TEXT_W / w).min(96.0 / h);
                let (dw, dh) = (w * scale, h * scale);
                let r = [at[0] + (TEXT_W - dw) / 2.0, at[1] + (96.0 - dh) / 2.0, dw, dh];
                draw.fill([r[0] - 1.0, r[1] - 1.0, dw + 2.0, dh + 2.0], PARCHMENT_EDGE);
                sprite(draw, str_of(&image["id"]), [0.0, 0.0, w, h], r);
            }
        }
        "link" => {
            let label = lang(host, "book.occultism.open_category", "Open the category");
            let w = draw.text_width(&label) + 8.0;
            let r = [mid - w / 2.0, at[1] + 2.0, w, 12.0];
            draw.fill(r, 0xFFE6D6B0);
            draw.text(&label, [r[0] + 4.0, r[1] + 2.0], LINK, false);
            state.hits.push((r, Hit::Link(str_of(&page["category"]).to_string())));
        }
        "missing" => {
            small(draw, &lang(host, "book.occultism.not_in_port", "Not in this port yet"), [at[0], at[1]], INK_DIM);
        }
        _ => {}
    }
}

/// A ritual: its pentacle (a link to it), the book that starts it in the golden bowl, its
/// ingredients in the sacrificial bowls around it, and what comes of it; its time, sacrifice and use.
fn draw_ritual(host: &dyn ClientHost, draw: &mut dyn DrawContext, screen: &ScreenView<'_>, state: &mut State, page: &Value, at: [f32; 2], tip: &mut Option<Vec<String>>) {
    let mid = at[0] + TEXT_W / 2.0;
    let name = str_of(&page["pentacle_name"]);
    let w = draw.text_width(name).min(TEXT_W);
    small(draw, name, [mid - w / 2.0, at[1]], LINK);
    let pentacle = str_of(&page["pentacle"]).split(':').next_back().unwrap_or("").to_string();
    state.hits.push(([mid - w / 2.0, at[1], w, LINE_H], Hit::Link(format!("pentacles/{pentacle}"))));
    // The ring of bowls, the book in the golden bowl at its middle; under it, what comes of it.
    let centre = [mid, at[1] + 54.0];
    let ingredients: Vec<Value> = page["ingredients"].as_array().cloned().unwrap_or_default();
    let n = ingredients.len().max(1);
    for (i, ing) in ingredients.iter().enumerate() {
        let angle = std::f32::consts::TAU * i as f32 / n as f32 - std::f32::consts::FRAC_PI_2;
        let p = [centre[0] + angle.cos() * 32.0 - 9.0, centre[1] + angle.sin() * 32.0 - 9.0];
        let items = ingredient(host, state, ing);
        slot(host, draw, screen, &items, 1, p, tip);
    }
    let activation = ingredient(host, state, &page["activation"]);
    slot(host, draw, screen, &activation, 1, [centre[0] - 9.0, centre[1] - 9.0], tip);
    let mut y = at[1] + 99.0;
    let mut line = format!("{} s", page["duration"].as_u64().unwrap_or(30));
    if let Some(result) = page["result"].as_str() {
        slot(host, draw, screen, &[result.to_string()], 1, [at[0], y], tip);
        small(draw, &format!("= {line}"), [at[0] + 22.0, y + 5.0], INK_DIM);
        y += 20.0;
    } else {
        if let Some(kind) = page["summon"].as_str() {
            line = format!("{}, {line}", item_name(host, kind));
        }
        small(draw, &line, [at[0], y + 2.0], INK_DIM);
        y += 11.0;
    }
    if let Some(sacrifice) = page["sacrifice"].as_str() {
        small(draw, &format!("+ {sacrifice}"), [at[0], y], INK_DIM);
        y += 9.0;
    }
    if page["item_use"].is_object() || page["item_use"].is_string() {
        let items = ingredient(host, state, &page["item_use"]);
        if let Some(item) = items.first() {
            small(draw, &format!("+ {}", item_name(host, item)), [at[0], y], INK_DIM);
        }
    }
}

/// A pentacle: its layout, a glyph or block to a cell, and the button that shows it in the world.
fn draw_pentacle(host: &dyn ClientHost, draw: &mut dyn DrawContext, screen: &ScreenView<'_>, state: &mut State, page: &Value, at: [f32; 2], tip: &mut Option<Vec<String>>) {
    let mid = at[0] + TEXT_W / 2.0;
    let id = str_of(&page["pentacle"]).to_string();
    let name = str_of(&page["name"]);
    let w = draw.text_width(name).min(TEXT_W);
    small(draw, name, [mid - w / 2.0, at[1]], INK);
    let size = page["size"].as_f64().unwrap_or(1.0).max(1.0) as f32;
    let cell = (88.0 / size).floor().clamp(4.0, 16.0);
    let left = mid - cell * size / 2.0;
    let top = at[1] + 11.0;
    draw.fill([left - 1.0, top - 1.0, cell * size + 2.0, cell * size + 2.0], 0xFFD8C8A0);
    for c in page["cells"].as_array().into_iter().flatten() {
        let (x, z, item) = (c[0].as_f64().unwrap_or(0.0) as f32, c[1].as_f64().unwrap_or(0.0) as f32, str_of(&c[2]));
        if let Some(stack) = crate::game::stack(host, item, 1) {
            let r = [left + x * cell, top + z * cell];
            draw.item_stack(&stack, r, cell);
            if inside([r[0], r[1], cell, cell], screen.mouse) {
                *tip = Some(vec![item_name(host, item)]);
            }
        }
    }
    let label = lang(host, "book.occultism.visualize", "Show in the world");
    let bw = draw.text_width(&label) + 8.0;
    let r = [mid - bw / 2.0, at[1] + 100.0, bw, 11.0];
    let hover = inside(r, screen.mouse);
    draw.fill(r, if hover { 0xFFD8C8A0 } else { 0xFFE6D6B0 });
    draw.text(&label, [r[0] + 4.0, r[1] + 2.0], LINK, false);
    state.hits.push((r, Hit::Visualize(id)));
}
