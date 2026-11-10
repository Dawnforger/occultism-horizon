// The storage multiblock for Horizon: the glue between the game's hooks and the store's rules
// (storage.rs). See docs/storage-design.md. The Core keeps a store in a file of its own in the
// world's folder, the Actuator on top is its interface (hoppers and a menu), plates around them
// raise its capacity. A store is a lasting network (network.rs): its Core is only its anchor, and
// as an item the Core carries the store's id, so it moves without its items ever moving.
//
// This is the adapter for items: `item_key` and `stack_of` are the only code that knows how
// a stack becomes a key, so a change in Horizon's item API stops here.

use crate::network::{self, Anchor, Placing, Record};
use crate::storage::{self, Change, Limits, Store};
use crate::terminal::{self, Serials};
use horizon_client::mod_api::*;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

pub const CORE: &str = "occultism:storage_core";
pub const ACTUATOR: &str = "occultism:storage_actuator";
pub const PLATE_PREFIX: &str = "occultism:storage_plate_tier";
pub const MENU: &str = "occultism:storage_actuator";
/// The component a Core item carries its store in: `{id, name, bond}`.
pub const STORE_COMPONENT: &str = "occultism:store";
/// The Actuator's slots: what comes in, then what was asked for.
pub const INPUTS: std::ops::Range<usize> = 0..9;
pub const OUTPUTS: std::ops::Range<usize> = 9..18;
pub const SLOT_COUNT: usize = 18;
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
    /// The numbers its viewers know its keys by.
    serials: Serials,
}

/// A player looking at a store through an Actuator, and what is still to be sent to them.
struct Viewer {
    store: String,
    queue: VecDeque<Vec<u8>>,
}

#[derive(Default)]
pub struct Vault {
    /// Stores in use, by id.
    stores: Mutex<HashMap<String, Open>>,
    /// Which store each loaded Core holds, and its timer (in memory: an idle Core changes nothing
    /// in the world, so it never marks its chunk for saving).
    cores: Mutex<HashMap<Place, (String, u32)>>,
    /// Stores' records, by id, read from their files the first time they're asked for.
    records: Mutex<HashMap<String, Record>>,
    /// Cores just placed: the store their item carried and who placed them, until their first tick.
    placed: Mutex<HashMap<Place, (Option<(String, u32)>, Option<u128>)>>,
    viewers: Mutex<HashMap<u128, Viewer>>,
    next_id: AtomicU64,
}

/// What a player is told about a store, by its spirit's name.
pub struct Notice {
    pub player: u128,
    pub key: &'static str,
    pub name: String,
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

/// The store a Core item carries: its id, spirit's name and bond.
pub fn carried(stack: &ItemStack) -> Option<(String, String, u32)> {
    let extra = stack.extra.as_ref()?;
    let (_, Tag::Compound(c)) = extra.other.iter().find(|(k, _)| k == STORE_COMPONENT)? else { return None };
    let bond = match c.get(&"bond".into()) {
        Some(Tag::Int(b)) => *b as u32,
        _ => 0,
    };
    Some((text(c, "id")?, text(c, "name").unwrap_or_default(), bond))
}

/// A Core item for store `id`, named `name`, of bond `bond`.
pub fn core_for(id: &str, name: &str, bond: u32) -> ItemStack {
    let mut core = ItemStack { item: CORE.into(), count: 1, damage: 0, extra: None };
    carry(&mut core, id, name, bond);
    core
}

/// `stack` carrying store `id`, named `name`, of bond `bond`.
pub fn carry(stack: &mut ItemStack, id: &str, name: &str, bond: u32) {
    let mut c = Compound::new();
    c.insert("id".into(), Tag::String(id.into()));
    c.insert("name".into(), Tag::String(name.into()));
    c.insert("bond".into(), Tag::Int(bond as i32));
    let extra = stack.extra.get_or_insert_with(Default::default);
    extra.other.retain(|(k, _)| k != STORE_COMPONENT);
    extra.other.push((STORE_COMPONENT.to_string(), Tag::Compound(c)));
}

fn plate_tier(block: &str) -> Option<u8> {
    block.strip_prefix(PLATE_PREFIX)?.parse().ok()
}

impl Vault {
    fn path(host: &dyn Host, id: &str, extension: &str) -> Option<PathBuf> {
        Some(host.world_dir()?.join("occultism").join("storage").join(format!("{id}.{extension}")))
    }

    /// The store `id`, loaded from its file the first time it's asked for.
    fn open<'a>(&self, host: &dyn Host, stores: &'a mut HashMap<String, Open>, id: &str) -> &'a mut Open {
        stores.entry(id.to_string()).or_insert_with(|| {
            let path = Self::path(host, id, "bin");
            let log = path.as_ref().and_then(|p| fs::read(p).ok()).unwrap_or_default();
            let limits = self.record(host, id).map_or_else(|| Limits::with_plates(&[]), |r| r.limits);
            let store = Store::load(limits, &log);
            Open { store, path, log_len: log.len(), serials: Serials::default() }
        })
    }

    /// Store `id`'s record, if it has one.
    fn record(&self, host: &dyn Host, id: &str) -> Option<Record> {
        let mut records = lock(&self.records);
        if let Some(r) = records.get(id) {
            return Some(r.clone());
        }
        let text = fs::read_to_string(Self::path(host, id, "txt")?).ok()?;
        let record = Record::decode(&text)?;
        records.insert(id.to_string(), record.clone());
        Some(record)
    }

    /// Keeps store `id`'s record, on disk at once: it changes only when a Core moves or a plate does.
    fn keep(&self, host: &dyn Host, id: &str, record: Record) {
        if let Some(path) = Self::path(host, id, "txt") {
            if let Some(dir) = path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let temp = path.with_extension("txt.tmp");
            let _ = fs::write(&temp, record.encode()).and_then(|()| fs::rename(&temp, &path));
        }
        lock(&self.records).insert(id.to_string(), record);
    }

    fn new_id(&self) -> String {
        let n = self.next_id.fetch_add(1, Ordering::Relaxed);
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        format!("{time:x}-{n:x}")
    }

    /// Whether store `id` is awake: its Core is placed. Gates attuned to a dormant store stop.
    pub fn awake(&self, host: &dyn Host, id: &str) -> bool {
        self.record(host, id).is_some_and(|r| r.awake())
    }

    /// A player placed a Core from `stack`: its first tick decides which store it anchors.
    pub fn placed(&self, dimension: &str, pos: [i32; 3], stack: &ItemStack, player: Option<u128>) {
        lock(&self.placed).insert((dimension.to_string(), pos), (carried(stack).map(|c| (c.0, c.2)), player));
    }

    /// The Core at `pos` is leaving the world as an item: its store saves and goes dormant. The
    /// Core item that carries it.
    pub fn unanchor(&self, host: &dyn Host, dimension: &str, pos: [i32; 3]) -> Option<(ItemStack, String)> {
        let (id, _) = lock(&self.cores).remove(&(dimension.to_string(), pos))?;
        if let Some(open) = lock(&self.stores).get_mut(&id) {
            Self::flush(open);
        }
        let mut record = self.record(host, &id)?;
        if record.anchor.as_ref().is_some_and(|a| a.dimension == dimension && a.pos == pos) {
            record.anchor = None;
            self.keep(host, &id, record.clone());
        }
        Some((core_for(&id, &record.name, record.bond), record.name))
    }

    /// Every store's record in this world, from their files.
    fn all_records(&self, host: &dyn Host) -> Vec<(String, Record)> {
        let Some(dir) = Self::path(host, "x", "txt").and_then(|p| p.parent().map(PathBuf::from)) else { return Vec::new() };
        let Ok(files) = fs::read_dir(dir) else { return Vec::new() };
        files
            .filter_map(|f| {
                let name = f.ok()?.file_name().into_string().ok()?;
                let id = name.strip_suffix(".txt")?;
                Some((id.to_string(), self.record(host, id)?))
            })
            .collect()
    }

    /// A recall ritual by `caster`: a new Core for each of their stores whose Core is lost (see
    /// `network::lost`), each bound anew so the lost item no longer reaches the store.
    pub fn recall(&self, host: &dyn Host, caster: u128) -> Vec<(ItemStack, String)> {
        let records = self.all_records(host);
        let carried: Vec<(String, u32)> = host
            .player_stacks(caster)
            .unwrap_or_default()
            .iter()
            .flatten()
            .filter(|s| &*s.item == CORE)
            .filter_map(|s| carried(s).map(|c| (c.0, c.2)))
            .collect();
        let stores = records.iter().map(|(id, r)| (id.as_str(), r, r.awake() && Self::gone(host, r.anchor.as_ref())));
        let lost: Vec<String> = network::lost(stores, caster, &carried).into_iter().map(str::to_string).collect();
        lost.iter()
            .filter_map(|id| {
                let (_, r) = records.iter().find(|(i, _)| i == id)?;
                let r = Record { anchor: None, bond: r.bond + 1, ..r.clone() };
                self.keep(host, id, r.clone());
                Some((core_for(id, &r.name, r.bond), r.name))
            })
            .collect()
    }

    /// Which store a Core anchors, on its first tick: the one it was placed with (woken, or a new
    /// one for a copy of a Core already placed), or a new store for a crafted Core. A Core loaded
    /// from the world keeps its own, unless a copy of it took that store elsewhere.
    fn anchor(&self, host: &dyn Host, pos: [i32; 3], entity: &mut ModBlockEntity) -> (Option<String>, Option<Notice>) {
        let here = Anchor { dimension: host.dimension().to_string(), pos };
        let place = (here.dimension.clone(), pos);
        // A store's spirit is named from its id, so a lost record gets its name back.
        let named = |id: &str| {
            network::spirit_name(id.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)))
        };
        let fresh = |vault: &Vault, owner: Option<u128>| {
            let id = vault.new_id();
            let record = Record::new(named(&id), owner.unwrap_or(0), Limits::with_plates(&[]), here.clone());
            vault.keep(host, &id, record.clone());
            (id, record)
        };
        let (id, notice) = match (text(&entity.nbt, "store"), lock(&self.placed).remove(&place)) {
            (Some(id), _) => match self.record(host, &id) {
                Some(r) if r.anchor.as_ref() == Some(&here) => (id, None),
                // Anchored elsewhere: this one is a stray copy (a world edit, a backup) and stays
                // inert while the store's own Core is placed.
                Some(r) if r.awake() && !Self::gone(host, r.anchor.as_ref()) => return (None, None),
                Some(r) => {
                    self.keep(host, &id, Record { anchor: Some(here), ..r });
                    (id, None)
                }
                // A store from before records: it gets its spirit now.
                None => {
                    let record = Record::new(named(&id), 0, Limits::with_plates(&[]), here);
                    self.keep(host, &id, record);
                    (id, None)
                }
            },
            (None, placed) => {
                let (carried, player) = placed.unwrap_or((None, None));
                let record = carried.as_ref().and_then(|(id, _)| self.record(host, id));
                // A store whose Core vanished (a command) counts as dormant here.
                let seen = record.clone().map(|r| if Self::gone(host, r.anchor.as_ref()) { Record { anchor: None, ..r } } else { r });
                let live = record.as_ref().filter(|r| r.awake() && !Self::gone(host, r.anchor.as_ref()));
                match network::placing(carried.as_ref().map(|(id, bond)| (id.as_str(), *bond)), seen.as_ref()) {
                    Placing::Wake(id) => {
                        let record = record.unwrap_or_else(|| Record::new(named(&id), player.unwrap_or(0), Limits::with_plates(&[]), here.clone()));
                        let name = record.name.clone();
                        // A store from before owners is its placer's from now on.
                        let owner = if record.owner == 0 { player.unwrap_or(0) } else { record.owner };
                        self.keep(host, &id, Record { anchor: Some(here), owner, ..record });
                        (id, player.map(|p| Notice { player: p, key: "storage.occultism.woke", name }))
                    }
                    Placing::Copy(_) => {
                        let (id, _) = fresh(self, player);
                        let name = live.map(|r| r.name.clone()).unwrap_or_default();
                        (id, player.map(|p| Notice { player: p, key: "storage.occultism.copy", name }))
                    }
                    Placing::Stale(_) => {
                        let (id, _) = fresh(self, player);
                        let name = record.map(|r| r.name).unwrap_or_default();
                        (id, player.map(|p| Notice { player: p, key: "storage.occultism.stale", name }))
                    }
                    Placing::New => {
                        let (id, record) = fresh(self, player);
                        (id, player.map(|p| Notice { player: p, key: "storage.occultism.named", name: record.name }))
                    }
                }
            }
        };
        entity.nbt.insert("store".into(), Tag::String(id.as_str().into()));
        (Some(id), notice)
    }

    /// Whether `anchor` no longer holds a Core (broken without dropping it, by a command). Only a
    /// loaded place can be seen; an unloaded one is taken to still hold it.
    fn gone(host: &dyn Host, anchor: Option<&Anchor>) -> bool {
        anchor.is_some_and(|a| {
            host.level(&a.dimension).is_some_and(|l| {
                host.level_is_loaded(l, a.pos) && host.level_block(l, a.pos).as_deref() != Some(CORE)
            })
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
        lock(&self.records).clear();
        lock(&self.placed).clear();
    }

    /// A Core's tick: it anchors its store once, then only scans plates and saves on a timer.
    pub fn core_tick(&self, host: &dyn Host, pos: [i32; 3], entity: &mut ModBlockEntity) -> Option<Notice> {
        let place = (host.dimension().to_string(), pos);
        let known = lock(&self.cores).get(&place).map(|c| c.0.clone());
        let (id, notice) = match known {
            Some(id) => (id, None),
            None => match self.anchor(host, pos, entity) {
                (Some(id), notice) => (id, notice),
                (None, _) => return None,
            },
        };
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
                    drop(stores);
                    if let Some(record) = self.record(host, &id) {
                        self.keep(host, &id, Record { limits, ..record });
                    }
                    stores = lock(&self.stores);
                }
            }
            if save {
                Self::flush(self.open(host, &mut stores, &id));
            }
        }
        notice
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
        // The terminal hands items straight to the player: anything left in the old output row
        // goes back in with the inputs.
        let waiting = (0..SLOT_COUNT).any(|i| entity.items.get(i).is_some_and(Option::is_some));
        if waiting {
            for i in 0..SLOT_COUNT.min(entity.items.len()) {
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

    /// A player opened the Actuator at `pos`: they get the store's spirit and everything in it,
    /// over the next ticks, then only what changes.
    pub fn watch(&self, player: u128, dimension: &str, pos: [i32; 3], host: &dyn Host) {
        let below = [pos[0], pos[1] - 1, pos[2]];
        let Some(id) = lock(&self.cores).get(&(dimension.to_string(), below)).map(|c| c.0.clone()) else { return };
        // Viewers already watching hear what changed so far, so the snapshot starts clean.
        self.broadcast(host, &id);
        let name = self.record(host, &id).map(|r| r.name).unwrap_or_default();
        let mut queue = VecDeque::from([terminal::begin(&name)]);
        {
            let mut stores = lock(&self.stores);
            let open = self.open(host, &mut stores, &id);
            let snapshot = open.store.snapshot();
            let serials = &mut open.serials;
            let records: Vec<(u32, u64, &[u8])> = snapshot.iter().map(|c| (serials.of(&c.key), c.count, &*c.key)).collect();
            queue.extend(terminal::records(records));
        }
        lock(&self.viewers).insert(player, Viewer { store: id, queue });
    }

    pub fn unwatch(&self, player: u128) {
        lock(&self.viewers).remove(&player);
    }

    /// What changed in store `id` since it was last asked, queued for everyone watching it.
    fn broadcast(&self, host: &dyn Host, id: &str) {
        let messages = {
            let mut stores = lock(&self.stores);
            let Some(open) = stores.get_mut(id) else { return };
            let changes = open.store.drain_viewers();
            if changes.is_empty() {
                return;
            }
            let serials = &mut open.serials;
            terminal::records(changes.iter().map(|c| (serials.of(&c.key), c.count, &*c.key)).collect::<Vec<_>>())
        };
        let _ = host;
        for viewer in lock(&self.viewers).values_mut().filter(|v| v.store == id) {
            viewer.queue.extend(messages.iter().cloned());
        }
    }

    /// Once a server tick: changes to watched stores go out, and each viewer gets the next few
    /// messages. With nobody watching, one look at an empty map.
    pub fn pump(&self, host: &mut dyn Host) {
        let watched: Vec<String> = {
            let viewers = lock(&self.viewers);
            if viewers.is_empty() {
                return;
            }
            let mut ids: Vec<String> = viewers.values().map(|v| v.store.clone()).collect();
            ids.sort_unstable();
            ids.dedup();
            ids
        };
        for id in &watched {
            self.broadcast(&*host, id);
        }
        let mut sends: Vec<(u128, Vec<u8>)> = Vec::new();
        for (player, viewer) in lock(&self.viewers).iter_mut() {
            for _ in 0..terminal::MESSAGES_PER_TICK {
                let Some(m) = viewer.queue.pop_front() else { break };
                sends.push((*player, m));
            }
        }
        for (player, m) in sends {
            let _ = host.net_send_to_player(player, terminal::CHANNEL, &m);
        }
    }

    /// A player clicked the terminal's grid: a stack of what they clicked (one with the right
    /// button) goes into their inventory; what doesn't fit stays in the store.
    pub fn take_to(&self, host: &mut dyn Host, player: u128, pos: [i32; 3], serial: u32, right: bool) {
        let Some(id) = self.id_under(&*host, pos) else { return };
        let stack = {
            let mut stores = lock(&self.stores);
            let open = self.open(&*host, &mut stores, &id);
            let Some(key) = open.serials.key(serial).cloned() else { return };
            let Some(probe) = stack_of(&key, 1) else { return };
            let want = if right { 1 } else { host.max_stack_size(&probe.item).unwrap_or(64) };
            let took = open.store.extract(&key, u64::from(want)) as u32;
            if took == 0 {
                return;
            }
            stack_of(&key, took)
        };
        let Some(stack) = stack else { return };
        if let Some(left) = host.give_stack(player, stack) {
            let (item, key, count) = item_key(&left);
            let mut stores = lock(&self.stores);
            self.open(&*host, &mut stores, &id).store.insert(&item, &key, count);
        }
    }
}
