// The storage multiblock for Horizon: the glue between the game's hooks and the store's rules
// (storage.rs). See docs/storage-design.md. The Core keeps a store in a file of its own in the
// world's folder, the Actuator on top is its interface (hoppers and a menu), plates around them
// raise its capacity.
//
// This is the adapter for items: `item_key` and `stack_of` are the only code that knows how
// a stack becomes a key, so a change in Horizon's item API stops here.

use crate::storage::{self, Change, Limits, Store};
use horizon_client::mod_api::*;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

pub const CORE: &str = "occultism:storage_core";
pub const ACTUATOR: &str = "occultism:storage_actuator";
pub const PLATE_PREFIX: &str = "occultism:storage_plate_tier";
pub const MENU: &str = "occultism:storage_actuator";
/// The Actuator's slots: what comes in, then what was asked for.
pub const INPUTS: std::ops::Range<usize> = 0..9;
pub const OUTPUTS: std::ops::Range<usize> = 9..18;
pub const SLOT_COUNT: usize = 18;
/// Menu button: take something out into the output slots.
pub const BUTTON_TAKE: i32 = 0;
/// Data slots (16-bit): types used and allowed, thousands of items used and allowed.
pub const DATA_COUNT: usize = 4;
/// Ticks between plate scans and between saves of what changed.
const SCAN_EVERY: u32 = 40;
const SAVE_EVERY: u32 = 20;

type Place = (String, [i32; 3]);

struct Open {
    store: Store,
    path: Option<PathBuf>,
    log_len: usize,
}

#[derive(Default)]
pub struct Vault {
    /// Stores in use, by id.
    stores: Mutex<HashMap<String, Open>>,
    /// Which store each loaded Core holds, and its timer (in memory: an idle Core changes nothing
    /// in the world, so it never marks its chunk for saving).
    cores: Mutex<HashMap<Place, (String, u32)>>,
    next_id: AtomicU64,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// A stack's item, exact identity and count. A plain stack's key is its item id; anything else
/// (damage, enchantments, a name...) is its NBT, one encoding per distinct stack.
pub fn item_key(stack: &ItemStack) -> (String, Vec<u8>, u64) {
    let item = stack.item.to_string();
    let count = u64::from(stack.count);
    if stack.extra.is_none() && stack.damage == 0 {
        let mut key = vec![0u8];
        key.extend_from_slice(item.as_bytes());
        return (item, key, count);
    }
    let mut one = stack.clone();
    one.count = 1;
    let mut key = vec![1u8];
    write_tag(&horizon_client::save::item_to_nbt(&one), &mut key);
    (item, key, count)
}

/// The stack `count` of a stored key make.
pub fn stack_of(key: &[u8], count: u32) -> Option<ItemStack> {
    match key.split_first()? {
        (0, id) => Some(ItemStack { item: std::str::from_utf8(id).ok()?.into(), count, damage: 0, extra: None }),
        (1, bytes) => {
            let (tag, _) = read_tag(bytes)?;
            let mut stack = horizon_client::save::item_from_nbt(&tag)?;
            stack.count = count;
            Some(stack)
        }
        _ => None,
    }
}

/// NBT as bytes, compounds in key order, so equal stacks are equal keys.
fn write_tag(tag: &Tag, out: &mut Vec<u8>) {
    fn len(n: usize, out: &mut Vec<u8>) {
        out.extend_from_slice(&(n as u32).to_le_bytes());
    }
    match tag {
        Tag::End => out.push(0),
        Tag::Byte(v) => {
            out.push(1);
            out.push(*v as u8);
        }
        Tag::Short(v) => {
            out.push(2);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Tag::Int(v) => {
            out.push(3);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Tag::Long(v) => {
            out.push(4);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Tag::Float(v) => {
            out.push(5);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Tag::Double(v) => {
            out.push(6);
            out.extend_from_slice(&v.to_le_bytes());
        }
        Tag::ByteArray(v) => {
            out.push(7);
            len(v.len(), out);
            out.extend_from_slice(v);
        }
        Tag::String(s) => {
            out.push(8);
            len(s.0.len(), out);
            s.0.iter().for_each(|c| out.extend_from_slice(&c.to_le_bytes()));
        }
        Tag::List(v) => {
            out.push(9);
            len(v.len(), out);
            v.iter().for_each(|t| write_tag(t, out));
        }
        Tag::Compound(c) => {
            out.push(10);
            len(c.len(), out);
            for (k, v) in c {
                len(k.0.len(), out);
                k.0.iter().for_each(|ch| out.extend_from_slice(&ch.to_le_bytes()));
                write_tag(v, out);
            }
        }
        Tag::IntArray(v) => {
            out.push(11);
            len(v.len(), out);
            v.iter().for_each(|i| out.extend_from_slice(&i.to_le_bytes()));
        }
        Tag::LongArray(v) => {
            out.push(12);
            len(v.len(), out);
            v.iter().for_each(|i| out.extend_from_slice(&i.to_le_bytes()));
        }
    }
}

/// A string tag from its UTF-16 units, kept exactly (Java strings needn't be valid Unicode).
fn java_string(units: Vec<u16>) -> Tag {
    let mut tag = Tag::String("".into());
    if let Tag::String(s) = &mut tag {
        s.0 = units;
    }
    tag
}

fn read_tag(b: &[u8]) -> Option<(Tag, &[u8])> {
    fn take<const N: usize>(b: &[u8]) -> Option<([u8; N], &[u8])> {
        Some((b.get(..N)?.try_into().ok()?, b.get(N..)?))
    }
    fn len(b: &[u8]) -> Option<(usize, &[u8])> {
        let (n, b) = take::<4>(b)?;
        Some((u32::from_le_bytes(n) as usize, b))
    }
    fn units(b: &[u8]) -> Option<(Vec<u16>, &[u8])> {
        let (n, mut b) = len(b)?;
        let mut out = Vec::with_capacity(n.min(b.len() / 2));
        for _ in 0..n {
            let (c, rest) = take::<2>(b)?;
            out.push(u16::from_le_bytes(c));
            b = rest;
        }
        Some((out, b))
    }
    let (kind, b) = b.split_first()?;
    Some(match kind {
        0 => (Tag::End, b),
        1 => {
            let (v, b) = take::<1>(b)?;
            (Tag::Byte(v[0] as i8), b)
        }
        2 => {
            let (v, b) = take::<2>(b)?;
            (Tag::Short(i16::from_le_bytes(v)), b)
        }
        3 => {
            let (v, b) = take::<4>(b)?;
            (Tag::Int(i32::from_le_bytes(v)), b)
        }
        4 => {
            let (v, b) = take::<8>(b)?;
            (Tag::Long(i64::from_le_bytes(v)), b)
        }
        5 => {
            let (v, b) = take::<4>(b)?;
            (Tag::Float(u32::from_le_bytes(v)), b)
        }
        6 => {
            let (v, b) = take::<8>(b)?;
            (Tag::Double(u64::from_le_bytes(v)), b)
        }
        7 => {
            let (n, b) = len(b)?;
            (Tag::ByteArray(b.get(..n)?.to_vec()), b.get(n..)?)
        }
        8 => {
            let (s, b) = units(b)?;
            (java_string(s), b)
        }
        9 => {
            let (n, mut b) = len(b)?;
            let mut list = Vec::with_capacity(n.min(b.len()));
            for _ in 0..n {
                let (t, rest) = read_tag(b)?;
                list.push(t);
                b = rest;
            }
            (Tag::List(list), b)
        }
        10 => {
            let (n, mut b) = len(b)?;
            let mut map = Compound::new();
            for _ in 0..n {
                let (k, rest) = units(b)?;
                let (t, rest) = read_tag(rest)?;
                let Tag::String(k) = java_string(k) else { return None };
                map.insert(k, t);
                b = rest;
            }
            (Tag::Compound(map), b)
        }
        11 => {
            let (n, mut b) = len(b)?;
            let mut list = Vec::with_capacity(n.min(b.len() / 4));
            for _ in 0..n {
                let (v, rest) = take::<4>(b)?;
                list.push(i32::from_le_bytes(v));
                b = rest;
            }
            (Tag::IntArray(list), b)
        }
        12 => {
            let (n, mut b) = len(b)?;
            let mut list = Vec::with_capacity(n.min(b.len() / 8));
            for _ in 0..n {
                let (v, rest) = take::<8>(b)?;
                list.push(i64::from_le_bytes(v));
                b = rest;
            }
            (Tag::LongArray(list), b)
        }
        _ => return None,
    })
}

fn text(nbt: &Compound, key: &str) -> Option<String> {
    match nbt.get(&key.into()) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    }
}

fn plate_tier(block: &str) -> Option<u8> {
    block.strip_prefix(PLATE_PREFIX)?.parse().ok()
}

impl Vault {
    fn path(host: &dyn Host, id: &str) -> Option<PathBuf> {
        Some(host.world_dir()?.join("occultism").join("storage").join(format!("{id}.bin")))
    }

    /// The store `id`, loaded from its file the first time it's asked for.
    fn open<'a>(&self, host: &dyn Host, stores: &'a mut HashMap<String, Open>, id: &str) -> &'a mut Open {
        stores.entry(id.to_string()).or_insert_with(|| {
            let path = Self::path(host, id);
            let log = path.as_ref().and_then(|p| fs::read(p).ok()).unwrap_or_default();
            let store = Store::load(Limits::with_plates(&[]), &log);
            Open { store, path, log_len: log.len() }
        })
    }

    /// Appends what changed to the store's file, compacting it when it has grown past the live data.
    fn flush(open: &mut Open) {
        if !open.store.dirty() {
            return;
        }
        let Some(path) = open.path.clone() else { return };
        let changes: Vec<Change> = open.store.drain_saves();
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }
        let mut bytes = Vec::new();
        storage::write_records(&changes, &mut bytes);
        let appended = fs::OpenOptions::new().create(true).append(true).open(&path).and_then(|mut f| f.write_all(&bytes));
        if appended.is_err() {
            return;
        }
        open.log_len += bytes.len();
        // Only when the log has outgrown the live data does a flush touch the whole store.
        if storage::should_compact(open.log_len, open.store.live_bytes()) {
            let mut live = Vec::with_capacity(open.store.live_bytes());
            storage::write_records(&open.store.snapshot(), &mut live);
            let temp = path.with_extension("tmp");
            if fs::write(&temp, &live).and_then(|()| fs::rename(&temp, &path)).is_ok() {
                open.log_len = live.len();
            }
        }
    }

    /// Saves everything now and lets go of it: the world is closing.
    pub fn close(&self) {
        let mut stores = lock(&self.stores);
        stores.values_mut().for_each(Self::flush);
        stores.clear();
        lock(&self.cores).clear();
    }

    /// A Core's tick: it names its store once, then only scans plates and saves on a timer.
    pub fn core_tick(&self, host: &dyn Host, pos: [i32; 3], entity: &mut ModBlockEntity) {
        let id = match text(&entity.nbt, "store") {
            Some(id) => id,
            None => {
                let n = self.next_id.fetch_add(1, Ordering::Relaxed);
                let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
                let id = format!("{time:x}-{n:x}");
                entity.nbt.insert("store".into(), Tag::String(id.as_str().into()));
                id
            }
        };
        let place = (host.dimension().to_string(), pos);
        let (first, next) = {
            let mut cores = lock(&self.cores);
            let first = !cores.contains_key(&place);
            let core = cores.entry(place).or_insert_with(|| (id.clone(), 0));
            core.1 = core.1.wrapping_add(1);
            (first, core.1)
        };
        let scan = first || next % SCAN_EVERY == 0;
        let save = next % SAVE_EVERY == 0;
        if scan || save {
            let mut stores = lock(&self.stores);
            let open = self.open(host, &mut stores, &id);
            if scan {
                let tiers: Vec<u8> = storage::plate_places()
                    .into_iter()
                    .filter_map(|o| plate_tier(&host.block([pos[0] + o[0], pos[1] + o[1], pos[2] + o[2]])))
                    .collect();
                let limits = Limits::with_plates(&tiers);
                if open.store.limits() != limits {
                    open.store.set_limits(limits);
                }
            }
            if save {
                Self::flush(open);
            }
        }
    }

    /// The store of the Core under an Actuator at `pos`.
    fn id_under(&self, host: &dyn Host, pos: [i32; 3]) -> Option<String> {
        let below = [pos[0], pos[1] - 1, pos[2]];
        if host.block(below) != CORE {
            return None;
        }
        lock(&self.cores).get(&(host.dimension().to_string(), below)).map(|c| c.0.clone())
    }

    /// An Actuator's tick: what's in its input slots goes into the store. With nothing there it
    /// costs a look at nine slots, whatever the store holds.
    pub fn actuator_tick(&self, host: &dyn Host, pos: [i32; 3], entity: &mut ModBlockEntity) {
        let Some(id) = self.id_under(host, pos) else {
            if entity.data.iter().any(|d| *d != 0) || entity.data.len() != DATA_COUNT {
                entity.data = vec![0; DATA_COUNT];
            }
            return;
        };
        let mut stores = lock(&self.stores);
        let open = self.open(host, &mut stores, &id);
        let waiting = INPUTS.clone().any(|i| entity.items.get(i).is_some_and(Option::is_some));
        if waiting {
            for i in INPUTS {
                let Some(stack) = entity.items.get(i).cloned().flatten() else { continue };
                let (item, key, count) = item_key(&stack);
                let took = open.store.insert(&item, &key, count) as u32;
                if took > 0 {
                    let left = stack.count - took;
                    entity.items[i] = (left > 0).then(|| ItemStack { count: left, ..stack });
                }
            }
        }
        let limits = open.store.limits();
        let thousands = |n: u64| (n / 1000).min(32_767) as i32;
        let data = vec![
            open.store.types().min(32_767) as i32,
            limits.types.min(32_767) as i32,
            thousands(open.store.total()),
            thousands(limits.total),
        ];
        if entity.data != data {
            entity.data = data;
        }
    }

    /// Fills an empty output slot with something from the store.
    pub fn take(&self, host: &dyn Host, pos: [i32; 3], entity: &mut ModBlockEntity) {
        let Some(id) = self.id_under(host, pos) else { return };
        if entity.items.len() < SLOT_COUNT {
            entity.items.resize(SLOT_COUNT, None);
        }
        let Some(slot) = OUTPUTS.into_iter().find(|i| entity.items[*i].is_none()) else { return };
        let mut stores = lock(&self.stores);
        let open = self.open(host, &mut stores, &id);
        let Some(key) = open.store.any_key() else { return };
        let Some(probe) = stack_of(&key, 1) else { return };
        let max = host.max_stack_size(&probe.item).unwrap_or(64);
        let took = open.store.extract(&key, u64::from(max)) as u32;
        if took > 0 {
            entity.items[slot] = stack_of(&key, took);
        }
    }

    /// Whether the Core at `pos` still holds items, so breaking it would strand them.
    pub fn holds_items(&self, host: &dyn Host, dimension: &str, pos: [i32; 3]) -> bool {
        let Some(id) = lock(&self.cores).get(&(dimension.to_string(), pos)).map(|c| c.0.clone()) else { return false };
        let mut stores = lock(&self.stores);
        self.open(host, &mut stores, &id).store.total() > 0
    }
}
