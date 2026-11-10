// The storage terminal's rules, without Horizon types (docs/storage-design.md, "Performance rules"
// 2 and 3): what the server sends a player looking at a store, and the player's own copy of it,
// searched, sorted and scrolled on the client. The server never sorts and sends only what changed.
//
// On the wire (`occultism:store`), each message is a kind byte, then:
//   0 Begin: the store's spirit name (u32 length, UTF-8): the client forgets what it had;
//   1 Records: (u32 serial, u64 count, u32 key length, key)*: count 0 is a type that left.
// A serial is the server's small number for a key while it runs, so a click names a key in 4 bytes.

use std::collections::HashMap;

pub const CHANNEL: &str = "occultism:store";
/// A message's most bytes (Horizon takes 30 000).
pub const MAX_MESSAGE: usize = 29_000;
/// Messages sent to one player a tick (Horizon takes 32), so a big store streams in over a few
/// seconds instead of one tick's spike.
pub const MESSAGES_PER_TICK: usize = 16;

const BEGIN: u8 = 0;
const RECORDS: u8 = 1;

/// The terminal's grid.
pub const COLUMNS: usize = 9;
pub const ROWS: usize = 6;
/// Menu buttons below this are the screen's own; a grid click is `GRID_BUTTONS + serial * 2 + right`.
pub const GRID_BUTTONS: i32 = 16;

pub fn button_for(serial: u32, right: bool) -> i32 {
    GRID_BUTTONS.saturating_add((serial as i32).saturating_mul(2)).saturating_add(right as i32)
}

/// The serial and mouse button a grid click names.
pub fn grid_click(button: i32) -> Option<(u32, bool)> {
    let n = button.checked_sub(GRID_BUTTONS)?;
    (n >= 0).then(|| ((n / 2) as u32, n % 2 == 1))
}

/// The server's serials: one per key a viewer was told of, never reused while the store is open.
#[derive(Default)]
pub struct Serials {
    by_key: HashMap<std::sync::Arc<[u8]>, u32>,
    keys: Vec<std::sync::Arc<[u8]>>,
}

impl Serials {
    pub fn of(&mut self, key: &std::sync::Arc<[u8]>) -> u32 {
        if let Some(n) = self.by_key.get(key) {
            return *n;
        }
        let n = self.keys.len() as u32;
        self.keys.push(key.clone());
        self.by_key.insert(key.clone(), n);
        n
    }

    pub fn key(&self, serial: u32) -> Option<&std::sync::Arc<[u8]>> {
        self.keys.get(serial as usize)
    }
}

pub fn begin(name: &str) -> Vec<u8> {
    let mut out = vec![BEGIN];
    out.extend_from_slice(&(name.len() as u32).to_le_bytes());
    out.extend_from_slice(name.as_bytes());
    out
}

/// `records` (serial, count, key) as messages of at most `MAX_MESSAGE` bytes.
pub fn records<'a>(records: impl IntoIterator<Item = (u32, u64, &'a [u8])>) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut message = vec![RECORDS];
    for (serial, count, key) in records {
        let len = 16 + key.len();
        if message.len() + len > MAX_MESSAGE && message.len() > 1 {
            out.push(std::mem::replace(&mut message, vec![RECORDS]));
        }
        message.extend_from_slice(&serial.to_le_bytes());
        message.extend_from_slice(&count.to_le_bytes());
        message.extend_from_slice(&(key.len() as u32).to_le_bytes());
        message.extend_from_slice(key);
    }
    if message.len() > 1 {
        out.push(message);
    }
    out
}

#[derive(Debug, PartialEq, Eq)]
pub enum Message {
    Begin(String),
    Records(Vec<(u32, u64, Vec<u8>)>),
}

pub fn decode(data: &[u8]) -> Option<Message> {
    fn take<const N: usize>(b: &mut &[u8]) -> Option<[u8; N]> {
        let (head, rest) = b.split_at_checked(N)?;
        *b = rest;
        head.try_into().ok()
    }
    let (kind, mut b) = data.split_first()?;
    match *kind {
        BEGIN => {
            let n = u32::from_le_bytes(take(&mut b)?) as usize;
            Some(Message::Begin(String::from_utf8(b.get(..n)?.to_vec()).ok()?))
        }
        RECORDS => {
            let mut out = Vec::new();
            while !b.is_empty() {
                let serial = u32::from_le_bytes(take(&mut b)?);
                let count = u64::from_le_bytes(take(&mut b)?);
                let n = u32::from_le_bytes(take(&mut b)?) as usize;
                let (key, rest) = b.split_at_checked(n)?;
                out.push((serial, count, key.to_vec()));
                b = rest;
            }
            Some(Message::Records(out))
        }
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    /// Most first, as Occultism's default.
    #[default]
    Count,
    Name,
    /// By mod, then name: everything of one mod together.
    Mod,
}

impl Sort {
    pub fn next(self) -> Sort {
        match self {
            Sort::Count => Sort::Name,
            Sort::Name => Sort::Mod,
            Sort::Mod => Sort::Count,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Sort::Count => "count",
            Sort::Name => "name",
            Sort::Mod => "mod",
        }
    }
}

/// One stored type as the player's terminal holds it: `T` is how the client draws it.
pub struct Entry<T> {
    pub count: u64,
    /// The item's id, `minecraft:diamond_sword`.
    pub id: String,
    /// What the player sees, lower-cased once for searching.
    pub name: String,
    pub shown: T,
}

/// The player's copy of a store: kept up to date by messages, searched and sorted here.
pub struct View<T> {
    pub name: String,
    entries: HashMap<u32, Entry<T>>,
    /// Every item in the store, kept as records come in.
    total: u64,
    search: String,
    sort: Sort,
    /// The serials shown, in order; rebuilt when something changed.
    order: Vec<u32>,
    stale: bool,
    /// The first row shown.
    pub row: usize,
}

impl<T> Default for View<T> {
    fn default() -> Self {
        View { name: String::new(), entries: HashMap::new(), total: 0, search: String::new(), sort: Sort::default(), order: Vec::new(), stale: false, row: 0 }
    }
}

impl<T> View<T> {
    /// A message from the server; `make` turns a new key into its entry (or None for a key this
    /// client can't show, such as an item it doesn't have).
    pub fn apply(&mut self, message: Message, mut make: impl FnMut(&[u8], u64) -> Option<Entry<T>>) {
        match message {
            Message::Begin(name) => {
                *self = View { name, search: std::mem::take(&mut self.search), sort: self.sort, ..View::default() };
            }
            Message::Records(records) => {
                for (serial, count, key) in records {
                    if count == 0 {
                        if let Some(e) = self.entries.remove(&serial) {
                            self.total -= e.count;
                        }
                    } else if let Some(e) = self.entries.get_mut(&serial) {
                        self.total = self.total - e.count + count;
                        e.count = count;
                    } else if let Some(e) = make(&key, count) {
                        self.total += count;
                        self.entries.insert(serial, e);
                    }
                }
                self.stale = true;
            }
        }
    }

    pub fn search(&self) -> &str {
        &self.search
    }

    pub fn set_search(&mut self, text: String) {
        if text != self.search {
            self.search = text;
            self.row = 0;
            self.stale = true;
        }
    }

    pub fn sort(&self) -> Sort {
        self.sort
    }

    pub fn set_sort(&mut self, sort: Sort) {
        self.sort = sort;
        self.stale = true;
    }

    /// Whether the order must be rebuilt before it is read.
    pub fn stale(&self) -> bool {
        self.stale
    }

    /// Rebuilds what's shown: every word of the search in the name or id (`@mod` matches the mod),
    /// in the sort's order. O(types log types), so the client calls it only when stale.
    pub fn rebuild(&mut self) {
        let words: Vec<String> = self.search.to_lowercase().split_whitespace().map(str::to_string).collect();
        let matches = |e: &Entry<T>| {
            words.iter().all(|w| match w.strip_prefix('@') {
                Some(m) => e.id.split(':').next().is_some_and(|ns| ns.contains(m)),
                None => e.name.contains(w.as_str()) || e.id.contains(w.as_str()),
            })
        };
        let mut order: Vec<u32> = self.entries.iter().filter(|(_, e)| matches(e)).map(|(s, _)| *s).collect();
        let e = |s: &u32| &self.entries[s];
        match self.sort {
            Sort::Count => order.sort_by(|a, b| e(b).count.cmp(&e(a).count).then_with(|| e(a).name.cmp(&e(b).name)).then(a.cmp(b))),
            Sort::Name => order.sort_by(|a, b| e(a).name.cmp(&e(b).name).then(a.cmp(b))),
            Sort::Mod => order.sort_by(|a, b| {
                let ns = |s: &u32| e(s).id.split(':').next().unwrap_or("").to_string();
                ns(a).cmp(&ns(b)).then_with(|| e(a).name.cmp(&e(b).name)).then(a.cmp(b))
            }),
        }
        self.order = order;
        self.row = self.row.min(self.max_row());
        self.stale = false;
    }

    /// How many types the search shows.
    pub fn shown(&self) -> usize {
        self.order.len()
    }

    pub fn max_row(&self) -> usize {
        self.order.len().div_ceil(COLUMNS).saturating_sub(ROWS)
    }

    pub fn scroll(&mut self, rows: isize) {
        self.row = self.row.saturating_add_signed(rows).min(self.max_row());
    }

    /// The entry in grid cell `cell` (row by row from the top left), with its serial.
    pub fn at(&self, cell: usize) -> Option<(u32, &Entry<T>)> {
        let serial = *self.order.get(self.row * COLUMNS + cell)?;
        Some((serial, self.entries.get(&serial)?))
    }

    pub fn types(&self) -> usize {
        self.entries.len()
    }

    pub fn total(&self) -> u64 {
        self.total
    }
}

/// A count in at most four characters: 999, 1k, 239k, 4M... (rounded down; the tooltip has the
/// exact count). Below 100k it fits a slot's width in the game's font.
pub fn compact(n: u64) -> String {
    match n {
        0..1_000 => n.to_string(),
        1_000..1_000_000 => format!("{}k", n / 1_000),
        1_000_000..1_000_000_000 => format!("{}M", n / 1_000_000),
        _ => format!("{}G", (n / 1_000_000_000).min(99)),
    }
}

/// A count with its thousands apart, for the tooltip: 239,481.
pub fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}
