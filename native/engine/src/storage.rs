// The storage's rules without Horizon types, so they test anywhere: the store itself, its save
// log, and the multiblock's capacity. See docs/storage-design.md: the cost of an operation depends
// on what changed, never on how much is stored.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// An item's exact identity: its encoded bytes (components, damage and all), count left out.
pub type Key = Arc<[u8]>;

/// The base capacity, and what each plate tier adds (Occultism's config defaults).
pub const BASE_TYPES: u32 = 128;
pub const BASE_TOTAL: u64 = 256_000;
pub const PLATE_TYPES: [u32; 4] = [64, 128, 256, 512];
pub const PLATE_TOTAL: [u64; 4] = [512_000, 1_024_000, 2_048_000, 4_096_000];

/// What a store may hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub types: u32,
    pub total: u64,
}

impl Limits {
    /// The capacity of a multiblock with these plates (`tier` 1 to 4 each).
    pub fn with_plates(tiers: &[u8]) -> Limits {
        let mut limits = Limits { types: BASE_TYPES, total: BASE_TOTAL };
        for &tier in tiers {
            if let Some(i) = (tier as usize).checked_sub(1).filter(|i| *i < PLATE_TYPES.len()) {
                limits.types += PLATE_TYPES[i];
                limits.total += PLATE_TOTAL[i];
            }
        }
        limits
    }
}

/// The places plates count, from the Core: the eight around it and the eight around the Actuator
/// on top of it.
pub fn plate_places() -> Vec<[i32; 3]> {
    let mut places = Vec::with_capacity(16);
    for y in 0..=1 {
        for x in -1..=1 {
            for z in -1..=1 {
                if x != 0 || z != 0 {
                    places.push([x, y, z]);
                }
            }
        }
    }
    places
}

#[derive(Clone, Debug)]
struct Entry {
    item: Arc<str>,
    key: Key,
    count: u64,
}

/// One item's new count, as the save log and viewers get it. A count of 0 is gone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub item: Arc<str>,
    pub key: Key,
    pub count: u64,
}

/// A store: exact items by key, each with its count.
#[derive(Debug)]
pub struct Store {
    limits: Limits,
    entries: HashMap<Key, Entry>,
    /// The keys of each item id, so "any of this item" looks at its variants only.
    by_item: HashMap<Arc<str>, HashSet<Key>>,
    total: u64,
    /// What a snapshot of the store would take in the save log, kept as items come and go.
    live_bytes: usize,
    /// Keys changed since the last `drain_saves` / `drain_viewers`.
    unsaved: HashSet<Key>,
    unseen: HashSet<Key>,
}

impl Store {
    pub fn new(limits: Limits) -> Store {
        Store {
            limits,
            entries: HashMap::new(),
            by_item: HashMap::new(),
            total: 0,
            live_bytes: 0,
            unsaved: HashSet::new(),
            unseen: HashSet::new(),
        }
    }

    pub fn limits(&self) -> Limits {
        self.limits
    }

    /// New limits apply to what comes in; what's stored stays, even past them.
    pub fn set_limits(&mut self, limits: Limits) {
        self.limits = limits;
    }

    pub fn types(&self) -> u32 {
        self.entries.len() as u32
    }

    pub fn total(&self) -> u64 {
        self.total
    }

    /// The size of a snapshot in the save log. O(1): it decides when to compact, on every flush.
    pub fn live_bytes(&self) -> usize {
        self.live_bytes
    }

    fn mark(&mut self, key: &Key) {
        self.unsaved.insert(key.clone());
        self.unseen.insert(key.clone());
    }

    /// Puts up to `count` of the item in; what it took. O(1).
    pub fn insert(&mut self, item: &str, key: &[u8], count: u64) -> u64 {
        let room = self.limits.total.saturating_sub(self.total);
        let take = count.min(room);
        if take == 0 {
            return 0;
        }
        if let Some(entry) = self.entries.get_mut(key) {
            entry.count += take;
            let key = entry.key.clone();
            self.total += take;
            self.mark(&key);
            return take;
        }
        if self.types() >= self.limits.types {
            return 0;
        }
        let key: Key = key.into();
        let item: Arc<str> = self.by_item.get_key_value(item).map_or_else(|| item.into(), |(k, _)| k.clone());
        self.by_item.entry(item.clone()).or_default().insert(key.clone());
        self.live_bytes += record_len(&item, &key);
        self.entries.insert(key.clone(), Entry { item, key: key.clone(), count: take });
        self.total += take;
        self.mark(&key);
        take
    }

    /// Takes up to `count` of exactly this item out; what it got. O(1).
    pub fn extract(&mut self, key: &[u8], count: u64) -> u64 {
        let Some(entry) = self.entries.get_mut(key) else { return 0 };
        let take = count.min(entry.count);
        entry.count -= take;
        let (left, key, item) = (entry.count, entry.key.clone(), entry.item.clone());
        self.total -= take;
        if left == 0 {
            self.entries.remove(&key);
            self.live_bytes -= record_len(&item, &key);
            if let Some(keys) = self.by_item.get_mut(&*item) {
                keys.remove(&key);
                if keys.is_empty() {
                    self.by_item.remove(&*item);
                }
            }
        }
        if take > 0 {
            self.unsaved.insert(key.clone());
            self.unseen.insert(key);
        }
        take
    }

    /// Takes up to `count` of one variant of `item` (any damage, any components); the variant and
    /// what it got. O(variants of this item).
    pub fn extract_any(&mut self, item: &str, count: u64) -> Option<(Key, u64)> {
        let key = self.by_item.get(item)?.iter().next()?.clone();
        let took = self.extract(&key, count);
        Some((key, took))
    }

    /// How many of exactly this item. O(1).
    pub fn count_of(&self, key: &[u8]) -> u64 {
        self.entries.get(key).map_or(0, |e| e.count)
    }

    /// How many of `item`, all variants. O(variants of this item).
    pub fn count_item(&self, item: &str) -> u64 {
        self.by_item.get(item).map_or(0, |keys| keys.iter().map(|k| self.count_of(k)).sum())
    }

    /// Any one stored key, for taking "something" out.
    pub fn any_key(&self) -> Option<Key> {
        self.entries.keys().next().cloned()
    }

    fn changes(&self, keys: HashSet<Key>) -> Vec<Change> {
        keys.into_iter()
            .map(|key| match self.entries.get(&key) {
                Some(e) => Change { item: e.item.clone(), key, count: e.count },
                None => Change { item: "".into(), key, count: 0 },
            })
            .collect()
    }

    /// What changed since the last call, one record per item however often it changed, for the
    /// save log. O(changed).
    pub fn drain_saves(&mut self) -> Vec<Change> {
        let keys = std::mem::take(&mut self.unsaved);
        self.changes(keys)
    }

    /// The same, for players with a terminal open.
    pub fn drain_viewers(&mut self) -> Vec<Change> {
        let keys = std::mem::take(&mut self.unseen);
        self.changes(keys)
    }

    /// Whether anything waits to be saved.
    pub fn dirty(&self) -> bool {
        !self.unsaved.is_empty()
    }

    /// Everything stored, as records: a viewer's first snapshot, or a compacted save.
    pub fn snapshot(&self) -> Vec<Change> {
        self.entries
            .values()
            .map(|e| Change { item: e.item.clone(), key: e.key.clone(), count: e.count })
            .collect()
    }

    /// One record of a save log, applied.
    fn apply(&mut self, change: Change) {
        if let Some(old) = self.entries.remove(&change.key) {
            self.total -= old.count;
            self.live_bytes -= record_len(&old.item, &change.key);
            if let Some(keys) = self.by_item.get_mut(&*old.item) {
                keys.remove(&change.key);
                if keys.is_empty() {
                    self.by_item.remove(&*old.item);
                }
            }
        }
        if change.count > 0 {
            self.total += change.count;
            self.live_bytes += record_len(&change.item, &change.key);
            self.by_item.entry(change.item.clone()).or_default().insert(change.key.clone());
            let entry = Entry { item: change.item, key: change.key.clone(), count: change.count };
            self.entries.insert(change.key, entry);
        }
    }

    /// A store from its save log; a cut-off last record (a crash mid-write) is left out.
    pub fn load(limits: Limits, log: &[u8]) -> Store {
        let mut store = Store::new(limits);
        let mut at = 0;
        while let Some((change, next)) = read_record(log, at) {
            store.apply(change);
            at = next;
        }
        store
    }
}

/// The size of one record in the save log.
fn record_len(item: &str, key: &[u8]) -> usize {
    4 + item.len() + 4 + key.len() + 8
}

/// A record of the save log: item length, item, key length, key, count.
pub fn write_records(changes: &[Change], out: &mut Vec<u8>) {
    for c in changes {
        out.extend_from_slice(&(c.item.len() as u32).to_le_bytes());
        out.extend_from_slice(c.item.as_bytes());
        out.extend_from_slice(&(c.key.len() as u32).to_le_bytes());
        out.extend_from_slice(&c.key);
        out.extend_from_slice(&c.count.to_le_bytes());
    }
}

fn read_record(log: &[u8], at: usize) -> Option<(Change, usize)> {
    let u32_at = |p: usize| -> Option<usize> {
        Some(u32::from_le_bytes(log.get(p..p + 4)?.try_into().ok()?) as usize)
    };
    let item_len = u32_at(at)?;
    let item = std::str::from_utf8(log.get(at + 4..at + 4 + item_len)?).ok()?;
    let key_at = at + 4 + item_len;
    let key_len = u32_at(key_at)?;
    let key = log.get(key_at + 4..key_at + 4 + key_len)?;
    let count_at = key_at + 4 + key_len;
    let count = u64::from_le_bytes(log.get(count_at..count_at + 8)?.try_into().ok()?);
    Some((Change { item: item.into(), key: key.into(), count }, count_at + 8))
}

/// Whether a log of `log_len` bytes is worth rewriting as a snapshot of `live_len` bytes.
pub fn should_compact(log_len: usize, live_len: usize) -> bool {
    log_len > 1 << 20 && log_len > live_len * 4
}
