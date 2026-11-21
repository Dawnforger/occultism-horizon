// Storage: the glue between the game and the store's rules
// (storage.rs). See docs/storage-design.md. One block, the Dimensional Storage Actuator, holds a
// store kept in a file of its own in the world's folder, and is its terminal and hopper face;
// stabilizer plates in its upgrade slots raise its capacity and are the store's, not the world's.
// A store is a lasting network (network.rs): the Actuator is only its anchor ("Core" below), and
// as an item it carries the store's id, so it moves without its items or plates ever moving.
//
// This is the adapter for items: `item_key` and `stack_of` are the only code that knows how
// a stack becomes a key, so a change in Horizon's item API stops here.

use crate::game::{self, id, lock, say, tell};
use crate::network::{self, Anchor, Placing, Record, UPGRADE_SLOTS};
use crate::storage::{self, valid_id, Change, Limits, Store};
use crate::terminal::{self, Serials};
use mod_api::prelude::*;
use mod_api::*;
use std::collections::{HashMap, HashSet, VecDeque};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub const ACTUATOR: &str = "occultism:storage_actuator";
/// The store's anchor and key: the Actuator itself.
pub const CORE: &str = ACTUATOR;
pub const PLATE_PREFIX: &str = "occultism:storage_plate_tier";
pub const MENU: &str = "occultism:storage_actuator";
/// A stable wormhole and the Storage Accessor: gates to a store, linked to it by its id.
pub const WORMHOLE: &str = "occultism:stable_wormhole";
pub const ACCESSOR: &str = "occultism:storage_remote";
/// The component a gate item carries its store's link in: `{id, name}`.
pub const LINK_COMPONENT: &str = "occultism:link";
/// The component a Core item carries its store in: `{id, name, bond}`.
pub const STORE_COMPONENT: &str = "occultism:store";
/// The Actuator's slots: what comes in, then what was asked for.
pub const INPUTS: std::ops::Range<usize> = 0..9;
pub const OUTPUTS: std::ops::Range<usize> = 9..18;
pub const SLOT_COUNT: usize = 18;
/// The terminal's crafting grid, as Occultism's: 3x3 container slots the block keeps, then the
/// slot the menu shows their result in.
pub const GRID: std::ops::Range<usize> = 18..27;
pub const RESULT: usize = 27;
/// Every slot of the Actuator's and a wormhole's container.
pub const CONTAINER_SLOTS: usize = 28;
/// The terminal's button that puts what its crafting grid holds back into the store.
pub const CLEAR_GRID_BUTTON: i32 = 7;

/// Data slots (16-bit): types used and allowed, thousands of items used and allowed, then each
/// upgrade slot's plate tier.
pub const DATA_COUNT: usize = 4 + UPGRADE_SLOTS;
/// The operator level that may break any Core (`/op` gives 4; single player's owner has it).
const OPERATOR: u8 = 2;
/// The sprite of the projection's matrix cubes.
const MATRIX_SPRITE: &str = "occultism:block/dimensional_matrix";

type Place = (String, [i32; 3]);

pub struct Open {
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
    /// Watching through a Storage Accessor: no block, so the server takes its requests by id.
    remote: bool,
    /// The Actuator or wormhole whose terminal is open: where its crafting grid is.
    at: Option<Place>,
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
    /// Records changed since the world last saved: they are written with it, never between.
    unsaved: Mutex<HashSet<String>>,
    /// Placed stable wormholes and the store each is linked to.
    gates: Mutex<HashMap<Place, String>>,
    /// Wormholes just placed, and the link their item carried, until their first tick.
    gates_placed: Mutex<HashMap<Place, Option<(String, String)>>>,
    /// The store each player last opened, for `/occultism trust`.
    last_opened: Mutex<HashMap<u128, String>>,
    /// The Core or wormhole item each player last used on a block, for the block it places.
    placing: Mutex<HashMap<u128, ItemStack>>,
    /// Actuators and wormholes a hopper below pulls from, and when it last tried.
    pulled: Mutex<HashMap<Place, std::time::Instant>>,
    /// Shift-clicks from a terminal's inventory, moved after the player's next tick: the player,
    /// the store's block, the inventory slot it should be in (0..36) and the stack clicked.
    moves: Mutex<Vec<(u128, [i32; 3], usize, ItemResource)>>,
    /// Crafting grids a player just crafted from, with the grid as it was: their emptied cells
    /// refill from the store on the block's next tick, as Occultism's `StorageControllerSlot` does.
    refills: Mutex<HashMap<Place, Vec<Vec<Option<ItemStack>>>>>,
    /// Crafting grid cells a player shift-clicked, for the block's next tick to empty into the
    /// store: the hook can't empty a slot, and a stack stored from it would stay there too.
    grid_moves: Mutex<HashMap<Place, Vec<(u128, usize)>>>,
    next_id: AtomicU64,
}

/// How long after a hopper's last try an Actuator keeps an item ready for it: a hopper tries every
/// 8 ticks, or each tick while it finds nothing.
const PULL_FOR: std::time::Duration = std::time::Duration::from_secs(2);

/// What a player is told about a store, by its spirit's name.
pub struct Notice {
    pub player: u128,
    pub key: &'static str,
    pub name: String,
}

/// A stack's item, exact identity and count. A plain stack's key is its item id; anything else
/// (damage, enchantments, a name...) is its NBT, one encoding per distinct stack.
pub fn item_key(host: &dyn Host, stack: &ItemStack) -> (String, Vec<u8>, u64) {
    let item = id(&*host, stack).to_string();
    let count = u64::from(stack.count());
    let one = ItemResource::of(stack);
    let nbt = (!game::plain(stack)).then(|| one.as_stack().to_nbt(host)).flatten();
    let Some(nbt) = nbt else {
        let mut key = vec![0u8];
        key.extend_from_slice(item.as_bytes());
        return (item, key, count);
    };
    let mut key = vec![1u8];
    write_tag(&Tag::Compound(nbt), &mut key);
    (item, key, count)
}

/// The stack `count` of a stored key make.
pub fn stack_of(host: &dyn Host, key: &[u8], count: u32) -> Option<ItemStack> {
    match key.split_first()? {
        (0, item) => game::stack(&*host, std::str::from_utf8(item).ok()?, count),
        (1, bytes) => {
            let (Tag::Compound(nbt), _) = read_tag(bytes)? else { return None };
            let mut stack = ItemStack::from_nbt(host, &nbt)?;
            stack.set_count(count).ok()?;
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

use game::text;

/// The store a Core item carries: its id, spirit's name and bond.
pub fn carried(stack: &ItemStack) -> Option<(String, String, u32)> {
    let c = game::component(stack, STORE_COMPONENT)?;
    let bond = game::int(&c, "bond").unwrap_or(0) as u32;
    Some((text(&c, "id").filter(|id| valid_id(id))?, text(&c, "name").unwrap_or_default(), bond))
}

/// A Core item for store `id`, named `name`, of bond `bond`.
pub fn core_for(host: &dyn Host, id: &str, name: &str, bond: u32) -> Option<ItemStack> {
    let mut core = game::stack(&*host, CORE, 1)?;
    carry(&mut core, id, name, bond);
    Some(core)
}

/// The store a gate item (a wormhole, an Accessor) is linked to: its id and spirit's name.
pub fn linked(stack: &ItemStack) -> Option<(String, String)> {
    let c = game::component(stack, LINK_COMPONENT)?;
    Some((text(&c, "id").filter(|id| valid_id(id))?, text(&c, "name").unwrap_or_default()))
}

/// `stack` linked to store `id`, named `name` (a gate item).
pub fn set_link(stack: &mut ItemStack, id: &str, name: &str) {
    let mut c = Compound::new();
    c.insert("id".into(), Tag::String(id.into()));
    c.insert("name".into(), Tag::String(name.into()));
    game::set_component(stack, LINK_COMPONENT, c);
}

/// `stack` carrying store `id`, named `name`, of bond `bond`.
pub fn carry(stack: &mut ItemStack, id: &str, name: &str, bond: u32) {
    let mut c = Compound::new();
    c.insert("id".into(), Tag::String(id.into()));
    c.insert("name".into(), Tag::String(name.into()));
    c.insert("bond".into(), Tag::Int(bond as i32));
    game::set_component(stack, STORE_COMPONENT, c);
}

/// A stabilizer plate item's tier.
fn plate_tier(item: &str) -> Option<u8> {
    item.strip_prefix(PLATE_PREFIX)?.parse().ok().filter(|t| (1..=4).contains(t))
}

impl Vault {
    /// The world's folder of store files.
    fn dir(host: &dyn ServerHost) -> Option<PathBuf> {
        Some(host.world_dir()?.join("occultism").join("storage"))
    }

    /// Store `id`'s file of `extension`; None for an id `new_id` couldn't have made.
    fn path(host: &dyn ServerHost, id: &str, extension: &str) -> Option<PathBuf> {
        valid_id(id).then(|| Self::dir(host)).flatten().map(|d| d.join(format!("{id}.{extension}")))
    }

    /// The store `id`, loaded from its file the first time it's asked for.
    fn open<'a>(&self, host: &dyn ServerHost, stores: &'a mut HashMap<String, Open>, id: &str) -> &'a mut Open {
        stores.entry(id.to_string()).or_insert_with(|| {
            let path = Self::path(host, id, "bin");
            let log = path.as_ref().and_then(|p| fs::read(p).ok()).unwrap_or_default();
            let limits = self.record(host, id).map_or_else(|| Limits::with_plates(&[]), |r| r.limits());
            let store = Store::load(limits, &log);
            Open { store, path, log_len: log.len(), serials: Serials::default() }
        })
    }

    /// Store `id`'s record, if it has one.
    fn record(&self, host: &dyn ServerHost, id: &str) -> Option<Record> {
        let mut records = lock(&self.records);
        if let Some(r) = records.get(id) {
            return Some(r.clone());
        }
        let text = fs::read_to_string(Self::path(host, id, "txt")?).ok()?;
        let record = Record::decode(&text)?;
        records.insert(id.to_string(), record.clone());
        Some(record)
    }

    /// Keeps store `id`'s record, written to disk when the world next saves.
    fn keep(&self, _host: &dyn ServerHost, id: &str, record: Record) {
        lock(&self.records).insert(id.to_string(), record);
        lock(&self.unsaved).insert(id.to_string());
    }

    /// The world saved: so do the stores and their records, in the same moment. A store's file
    /// then never runs ahead of the world: after a crash, both come back as they were at the last
    /// save, so items put in since can't be both in the store and back in a player's hands.
    pub fn saved(&self, host: &dyn ServerHost) {
        lock(&self.stores).values_mut().for_each(Self::flush);
        let ids: Vec<String> = lock(&self.unsaved).drain().collect();
        let records = lock(&self.records);
        for id in ids {
            let (Some(record), Some(path)) = (records.get(&id), Self::path(host, &id, "txt")) else { continue };
            if let Some(dir) = path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let temp = path.with_extension("txt.tmp");
            let _ = fs::write(&temp, record.encode()).and_then(|()| fs::rename(&temp, &path));
        }
    }

    /// Whether `player` may use store `id`.
    fn may_use(&self, host: &dyn ServerHost, id: &str, player: u128) -> bool {
        self.record(host, id).is_none_or(|r| r.may_use(player))
    }

    /// The store of the Core at `pos` in `dimension`, if one is anchored there, or of the wormhole
    /// there.
    fn id_at(&self, dimension: &str, pos: [i32; 3]) -> Option<String> {
        let place = (dimension.to_string(), pos);
        lock(&self.cores).get(&place).map(|c| c.0.clone()).or_else(|| lock(&self.gates).get(&place).cloned())
    }

    /// Whether the wormhole at `pos` in `dimension` reaches its store: refused with the spirit's
    /// name while the store is dormant.
    pub fn gate_open(&self, host: &dyn ServerHost, dimension: &str, pos: [i32; 3]) -> Result<(), String> {
        let Some(id) = lock(&self.gates).get(&(dimension.to_string(), pos)).cloned() else { return Ok(()) };
        match self.record(host, &id) {
            Some(r) if !r.awake() => Err(r.name),
            _ => Ok(()),
        }
    }

    /// Whether `player` may open the Actuator at `pos`: any Actuator not on a store, or one whose
    /// store trusts them.
    pub fn may_open(&self, host: &dyn ServerHost, dimension: &str, pos: [i32; 3], player: u128) -> Result<(), String> {
        let Some(id) = self.id_at(dimension, pos) else { return Ok(()) };
        self.refusal(host, &id, player)
    }

    /// Whether `player` may break the Core at `pos`: one their store trusts them with, or any as
    /// an operator. A stranger can't make someone's store dormant or carry its Core away.
    pub fn may_break(&self, host: &dyn ServerHost, dimension: &str, pos: [i32; 3], player: u128, permission: u8) -> Result<(), String> {
        let Some(id) = self.id_at(dimension, pos) else { return Ok(()) };
        // A wormhole too: someone's gate isn't a stranger's to carry off.
        if permission >= OPERATOR {
            return Ok(());
        }
        self.refusal(host, &id, player)
    }

    /// None when `player` may use store `id`, else its spirit's name for the refusal.
    fn refusal(&self, host: &dyn ServerHost, id: &str, player: u128) -> Result<(), String> {
        match self.record(host, id) {
            Some(r) if !r.may_use(player) => Err(r.name),
            _ => Ok(()),
        }
    }

    /// `/occultism trust` and `untrust`: the owner of the store `owner` last opened lets `player`
    /// use it, or no longer. The store's name, or why not.
    pub fn trust(&self, host: &dyn ServerHost, owner: u128, player: u128, trusted: bool) -> Result<String, &'static str> {
        let id = lock(&self.last_opened).get(&owner).cloned().ok_or("storage.occultism.trust.none")?;
        let mut record = self.record(host, &id).ok_or("storage.occultism.trust.none")?;
        if record.owner != owner {
            return Err("storage.occultism.trust.not_owner");
        }
        record.trusted.retain(|p| *p != player);
        if trusted && player != owner {
            record.trusted.push(player);
        }
        let name = record.name.clone();
        self.keep(host, &id, record);
        if !trusted {
            // Someone no longer trusted stops watching at once.
            lock(&self.viewers).retain(|p, v| !(*p == player && v.store == id));
        }
        Ok(name)
    }

    /// Who the store `owner` last opened trusts, by UUID, with its name.
    pub fn trusted(&self, host: &dyn ServerHost, owner: u128) -> Option<(String, Vec<u128>)> {
        let id = lock(&self.last_opened).get(&owner).cloned()?;
        let record = self.record(host, &id).filter(|r| r.owner == owner)?;
        Some((record.name, record.trusted))
    }

    fn new_id(&self) -> String {
        let n = self.next_id.fetch_add(1, Ordering::Relaxed);
        let time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        format!("{time:x}-{n:x}")
    }

    /// Whether store `id` is awake: its Core is placed. Gates attuned to a dormant store stop.
    pub fn awake(&self, host: &dyn ServerHost, id: &str) -> bool {
        self.record(host, id).is_some_and(|r| r.awake())
    }

    /// A player is using a Core or wormhole item on a block: what it places takes its data.
    pub fn placing(&self, player: u128, stack: &ItemStack) {
        lock(&self.placing).insert(player, stack.clone());
    }

    /// `player` placed `block` at `pos`: a Core's first tick decides which store it anchors, a
    /// wormhole's which store it reaches, from the item they used.
    pub fn placed_by(&self, host: &dyn Host, dimension: &str, pos: [i32; 3], block: &str, player: u128) {
        let Some(stack) = lock(&self.placing).remove(&player).filter(|s| id(&*host, s) == block) else { return };
        let place = (dimension.to_string(), pos);
        match block {
            CORE => {
                lock(&self.placed).insert(place, (carried(&stack).map(|c| (c.0, c.2)), Some(player)));
            }
            WORMHOLE => {
                lock(&self.gates_placed).insert(place, linked(&stack));
            }
            _ => {}
        }
    }

    /// The Core at `pos` is leaving the world as an item: its store saves and goes dormant. The
    /// Core item that carries it.
    pub fn unanchor(&self, host: &dyn ServerHost, dimension: &str, pos: [i32; 3]) -> Option<(ItemStack, String)> {
        let (id, _) = lock(&self.cores).remove(&(dimension.to_string(), pos))?;
        let mut record = self.record(host, &id)?;
        if record.anchor.as_ref().is_some_and(|a| a.dimension == dimension && a.pos == pos) {
            record.anchor = None;
            self.keep(host, &id, record.clone());
        }
        Some((core_for(host, &id, &record.name, record.bond)?, record.name))
    }

    /// Every store's record in this world, from their files.
    fn all_records(&self, host: &dyn ServerHost) -> Vec<(String, Record)> {
        let Some(dir) = Self::dir(host) else { return Vec::new() };
        // On disk, and new since the last save.
        let mut ids: HashSet<String> = lock(&self.records).keys().cloned().collect();
        if let Ok(files) = fs::read_dir(dir) {
            ids.extend(files.filter_map(|f| Some(f.ok()?.file_name().into_string().ok()?.strip_suffix(".txt")?.to_string())).filter(|id| valid_id(id)));
        }
        ids.into_iter().filter_map(|id| Some((id.clone(), self.record(host, &id)?))).collect()
    }

    /// A recall ritual by `caster`: a new Core for each of their stores whose Core is lost (see
    /// `network::lost`), each bound anew so the lost item no longer reaches the store.
    pub fn recall(&self, host: &dyn ServerHost, caster: u128) -> Vec<(ItemStack, String)> {
        let records = self.all_records(host);
        let carried: Vec<(String, u32)> = host
            .player(caster)
            .and_then(|p| p.inventory(host))
            .unwrap_or_default()
            .iter()
            .filter(|s| id(&*host, s) == CORE)
            .filter_map(|s| carried(s).map(|c| (c.0, c.2)))
            .collect();
        let stores = records.iter().map(|(id, r)| (id.as_str(), r, r.awake() && Self::gone(host, r.anchor.as_ref())));
        let lost: Vec<String> = network::lost(stores, caster, &carried).into_iter().map(str::to_string).collect();
        lost.iter()
            .filter_map(|id| {
                let (_, r) = records.iter().find(|(i, _)| i == id)?;
                let r = Record { anchor: None, bond: r.bond + 1, ..r.clone() };
                self.keep(host, id, r.clone());
                Some((core_for(host, id, &r.name, r.bond)?, r.name))
            })
            .collect()
    }

    /// Which store a Core anchors, on its first tick: the one it was placed with (woken, or a new
    /// one for a copy of a Core already placed), or a new store for a crafted Core. A Core loaded
    /// from the world keeps its own, unless a copy of it took that store elsewhere.
    fn anchor(&self, host: &dyn ServerHost, pos: [i32; 3], entity: &mut ModBlockEntityView<'_>) -> (Option<String>, Option<Notice>) {
        let here = Anchor { dimension: host.dimension().to_string(), pos };
        let place = (here.dimension.clone(), pos);
        // A store's spirit is named from its id, so a lost record gets its name back.
        let named = |id: &str| {
            network::spirit_name(id.bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100_0000_01b3)))
        };
        let fresh = |vault: &Vault, owner: Option<u128>| {
            let id = vault.new_id();
            let record = Record::new(named(&id), owner.unwrap_or(0), here.clone());
            vault.keep(host, &id, record.clone());
            (id, record)
        };
        // A Core its store refused stays a dead stone until it's broken.
        if text(&entity.nbt, "refused").is_some() {
            return (None, None);
        }
        let (id, notice) = match (text(&entity.nbt, "store").filter(|id| valid_id(id)), lock(&self.placed).remove(&place)) {
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
                    let record = Record::new(named(&id), 0, here);
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
                match network::placing(carried.as_ref().map(|(id, bond)| (id.as_str(), *bond)), seen.as_ref(), player.unwrap_or(0)) {
                    Placing::Refused(id) => {
                        entity.nbt.insert("refused".into(), Tag::String(id.as_str().into()));
                        let name = record.map(|r| r.name).unwrap_or_default();
                        return (None, player.map(|p| Notice { player: p, key: "storage.occultism.refused", name }));
                    }
                    Placing::Wake(id) => {
                        let record = record.unwrap_or_else(|| Record::new(named(&id), player.unwrap_or(0), here.clone()));
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
    fn gone(host: &dyn ServerHost, anchor: Option<&Anchor>) -> bool {
        anchor.is_some_and(|a| {
            host.level(&a.dimension)
                .is_some_and(|l| l.is_loaded(host, a.pos) && game::block_name(&*host, l.block(host, a.pos)) != CORE)
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

    /// Lets go of everything: the world is closing. `saved` says its final save went through, so
    /// the stores save with it; after a failed one they stay as the world's last save left them.
    pub fn close(&self, host: &dyn ServerHost, saved: bool) {
        if saved {
            self.saved(host);
        }
        lock(&self.stores).clear();
        lock(&self.cores).clear();
        lock(&self.records).clear();
        lock(&self.placed).clear();
        lock(&self.unsaved).clear();
        lock(&self.viewers).clear();
        lock(&self.last_opened).clear();
        lock(&self.gates).clear();
        lock(&self.gates_placed).clear();
        lock(&self.placing).clear();
    }

    /// An Actuator's tick: it anchors its store once; then what hoppers put in its input slots goes
    /// into the store, and its screen's numbers and its projection follow the store. With nothing
    /// waiting it costs a look at a few slots, whatever the store holds. Whether clients must
    /// redraw it (its upgrades changed).
    pub fn tick(&self, host: &dyn ServerHost, pos: [i32; 3], entity: &mut ModBlockEntityView<'_>) -> (Option<Notice>, bool) {
        let place = (host.dimension().to_string(), pos);
        let known = lock(&self.cores).get(&place).map(|c| c.0.clone());
        let (id, notice) = match known {
            Some(id) => (id, None),
            None => match self.anchor(host, pos, entity) {
                (Some(id), notice) => {
                    lock(&self.cores).insert(place.clone(), (id.clone(), 0));
                    (id, notice)
                }
                (None, notice) => {
                    if entity.data.iter().any(|d| *d != 0) || entity.data.len() != DATA_COUNT {
                        entity.data = vec![0; DATA_COUNT];
                    }
                    return (notice, false);
                }
            },
        };
        let plates = self.record(host, &id).map_or([0; UPGRADE_SLOTS], |r| r.plates);
        let pulled = self.pulled(&place);
        self.serve(host, &id, plates, pulled, entity);
        // Clients draw the projection from the plates: they hear when those change.
        let shown = Tag::ByteArray(plates.to_vec());
        let sync = entity.nbt.get(&"plates".into()) != Some(&shown);
        if sync {
            entity.nbt.insert("plates".into(), shown);
        }
        (notice, sync)
    }

    /// A hopper at `pos` is about to move items: if an Actuator or a wormhole is above it, it is
    /// pulled from, and keeps an item ready in its bottom slot while that goes on.
    pub fn hopper_try(&self, host: &dyn ServerHost, pos: [i32; 3]) {
        let above = [pos[0], pos[1] + 1, pos[2]];
        if matches!(block_here(host, above), ACTUATOR | WORMHOLE) {
            lock(&self.pulled).insert((host.dimension().to_string(), above), std::time::Instant::now());
        }
    }

    fn pulled(&self, place: &Place) -> bool {
        let mut pulled = lock(&self.pulled);
        match pulled.get(place) {
            Some(at) if at.elapsed() < PULL_FOR => true,
            Some(_) => {
                pulled.remove(place);
                false
            }
            None => false,
        }
    }

    /// What an Actuator or a wormhole does each tick for store `id`: what hoppers left in its input
    /// slots goes in, and its screen's numbers follow the store. While a hopper below pulls from
    /// it, one item waits in its first output slot for the hopper to take (Occultism's storage
    /// controller is an item handler a hopper empties); once the hopper stops, it goes back. With
    /// nothing waiting it costs a look at a few slots, whatever the store holds.
    fn serve(&self, host: &dyn ServerHost, id: &str, plates: [u8; UPGRADE_SLOTS], pulled: bool, entity: &mut ModBlockEntityView<'_>) {
        let mut stores = lock(&self.stores);
        let open = self.open(host, &mut stores, id);
        if pulled && entity.items.get(OUTPUTS.start).is_none_or(Option::is_none) {
            if let Some(key) = open.store.any_key()
                && stack_of(host, &key, 1).is_some()
                && open.store.extract(&key, 1) == 1
            {
                if entity.items.len() < SLOT_COUNT {
                    entity.items.resize(SLOT_COUNT, None);
                }
                entity.items[OUTPUTS.start] = stack_of(host, &key, 1);
            }
        }
        let waiting = (0..SLOT_COUNT).any(|i| entity.items.get(i).is_some_and(Option::is_some) && !(pulled && OUTPUTS.contains(&i)));
        if waiting {
            for i in 0..SLOT_COUNT.min(entity.items.len()) {
                if pulled && OUTPUTS.contains(&i) {
                    continue;
                }
                let Some(stack) = entity.items.get(i).cloned().flatten() else { continue };
                let (item, key, count) = item_key(host, &stack);
                let took = open.store.insert(&item, &key, count) as u32;
                if took > 0 {
                    entity.items[i] = stack.shrink(took);
                }
            }
        }
        let limits = open.store.limits();
        let thousands = |n: u64| (n / 1000).min(32_767) as i32;
        let mut data = vec![
            open.store.types().min(32_767) as i32,
            limits.types.min(32_767) as i32,
            thousands(open.store.total()),
            thousands(limits.total),
        ];
        data.extend(plates.iter().map(|p| i32::from(*p)));
        if entity.data != data {
            entity.data = data;
        }
    }

    /// The store reached at `pos`, in the ticking level: anchored there (an Actuator), or the one
    /// a wormhole there is linked to while that store is awake.
    fn id_under(&self, host: &dyn ServerHost, pos: [i32; 3]) -> Option<String> {
        let place = (host.dimension().to_string(), pos);
        let block = host.level(host.dimension()).map_or("", |l| game::block_name(&*host, l.block(host, pos)));
        match block {
            ACTUATOR => lock(&self.cores).get(&place).map(|c| c.0.clone()),
            WORMHOLE => lock(&self.gates).get(&place).cloned().filter(|id| self.awake(host, id)),
            _ => None,
        }
    }

    /// A gate item used on the Actuator at `pos`: the store it links to and its spirit's name, if
    /// the player may use it.
    pub fn link(&self, host: &dyn ServerHost, player: u128, pos: [i32; 3]) -> Option<(String, String)> {
        if host.level(host.dimension()).is_none_or(|l| game::block_name(&*host, l.block(host, pos)) != ACTUATOR) {
            return None;
        }
        let id = lock(&self.cores).get(&(host.dimension().to_string(), pos)).map(|c| c.0.clone())?;
        let record = self.record(host, &id).filter(|r| r.may_use(player))?;
        Some((id, record.name))
    }

    /// A wormhole leaving the world: the link for the item it drops.
    pub fn gate_removed(&self, host: &dyn ServerHost, dimension: &str, pos: [i32; 3]) -> Option<(String, String)> {
        let id = lock(&self.gates).remove(&(dimension.to_string(), pos))?;
        let name = self.record(host, &id).map(|r| r.name).unwrap_or_default();
        Some((id, name))
    }

    /// A stable wormhole's tick: it links once (from its item, kept in its own data after), then
    /// does what an Actuator does for hoppers and its screen, for its store, while that is awake.
    pub fn gate_tick(&self, host: &dyn ServerHost, pos: [i32; 3], entity: &mut ModBlockEntityView<'_>) -> bool {
        let place = (host.dimension().to_string(), pos);
        if let Some(link) = lock(&self.gates_placed).remove(&place).flatten() {
            entity.nbt.insert("link".into(), Tag::String(link.0.as_str().into()));
        }
        let Some(id) = text(&entity.nbt, "link") else { return false };
        lock(&self.gates).insert(place.clone(), id.clone());
        let awake = self.awake(host, &id);
        if !awake {
            if entity.data.iter().any(|d| *d != 0) || entity.data.len() != DATA_COUNT {
                entity.data = vec![0; DATA_COUNT];
            }
            return false;
        }
        let plates = self.record(host, &id).map_or([0; UPGRADE_SLOTS], |r| r.plates);
        let pulled = self.pulled(&place);
        self.serve(host, &id, plates, pulled, entity);
        false
    }

    /// A shift-clicked stabilizer plate goes into the store's first empty upgrade slot. Whether it
    /// did (the player keeps it otherwise).
    pub fn upgrade(&self, host: &dyn ServerHost, player: u128, pos: [i32; 3], stack: &ItemStack) -> bool {
        let (Some(id), Some(tier)) = (self.id_under(host, pos), plate_tier(id(&*host, stack))) else { return false };
        let Some(mut record) = self.record(host, &id).filter(|r| r.may_use(player)) else { return false };
        if !record.add_plate(tier) {
            return false;
        }
        self.set_plates(host, &id, record);
        true
    }

    /// A click on upgrade slot `slot`: its plate goes back to the player. What doesn't fit stays.
    pub fn remove_plate(&self, host: &mut dyn ServerHost, player: u128, pos: [i32; 3], slot: usize) {
        let Some(id) = self.id_under(&*host, pos) else { return };
        let Some(mut record) = self.record(&*host, &id).filter(|r| r.may_use(player)) else { return };
        let Some(tier) = record.take_plate(slot) else { return };
        let Some(plate) = game::stack(&*host, &format!("{PLATE_PREFIX}{tier}"), 1) else { return };
        if game::give(host, player, plate).is_some() {
            return; // a full inventory: the plate stays where it was
        }
        self.set_plates(&*host, &id, record);
    }

    /// Keeps `record`'s new plates, and the store's capacity with them.
    fn set_plates(&self, host: &dyn ServerHost, id: &str, record: Record) {
        let limits = record.limits();
        let stats = terminal::stats(limits.types, limits.total, &record.plates);
        self.keep(host, id, record);
        if let Some(open) = lock(&self.stores).get_mut(id) {
            open.store.set_limits(limits);
        }
        for viewer in lock(&self.viewers).values_mut().filter(|v| v.store == id) {
            viewer.queue.push_back(stats.clone());
        }
    }

    /// A player opened the Actuator at `pos`: they get the store's spirit and everything in it,
    /// over the next ticks, then only what changes.
    pub fn watch(&self, player: u128, dimension: &str, pos: [i32; 3], host: &dyn ServerHost) {
        let Some(id) = self.id_at(dimension, pos) else { return };
        // The right-click refuses strangers first; a screen opened past it still shows them nothing.
        if self.gate_open(host, dimension, pos).is_err() || !self.may_use(host, &id, player) {
            return;
        }
        self.watch_store(host, player, &id, Some((dimension.to_string(), pos)));
    }

    /// A player used a Storage Accessor: its store, wherever it is, opens on their screen, if it
    /// is awake and trusts them. Why not, for the player, otherwise.
    pub fn watch_remote(&self, host: &dyn ServerHost, player: u128, stack: &ItemStack) -> Result<(), (&'static str, String)> {
        let (id, name) = linked(stack).ok_or(("storage.occultism.unlinked", String::new()))?;
        let record = self.record(host, &id).ok_or(("storage.occultism.unlinked", String::new()))?;
        if !record.may_use(player) {
            return Err(("storage.occultism.refused", record.name));
        }
        if !record.awake() {
            return Err(("storage.occultism.gate_dormant", name));
        }
        self.watch_store(host, player, &id, None);
        Ok(())
    }

    /// `player` starts watching store `id`: its spirit, its capacity and everything in it, over the
    /// next ticks, then only what changes.
    /// `at` is the block whose terminal it is; None for an Accessor's.
    fn watch_store(&self, host: &dyn ServerHost, player: u128, id: &str, at: Option<Place>) {
        let id = id.to_string();
        let remote = at.is_none();
        // Opening is refused before the menu opens (`may_open`); this keeps a menu opened some
        // other way from showing a store to someone it doesn't trust.
        if !self.may_use(host, &id, player) {
            return;
        }
        lock(&self.last_opened).insert(player, id.clone());
        // Viewers already watching hear what changed so far, so the snapshot starts clean.
        self.broadcast(host, &id);
        let record = self.record(host, &id);
        let name = record.as_ref().map(|r| r.name.clone()).unwrap_or_default();
        let mut queue = VecDeque::new();
        if remote {
            queue.push_back(terminal::open());
        }
        queue.push_back(terminal::begin(&name));
        if let Some(r) = &record {
            let limits = r.limits();
            queue.push_back(terminal::stats(limits.types, limits.total, &r.plates));
        }
        {
            let mut stores = lock(&self.stores);
            let open = self.open(host, &mut stores, &id);
            let snapshot = open.store.snapshot();
            let serials = &mut open.serials;
            let records: Vec<(u32, u64, &[u8])> = snapshot.iter().map(|c| (serials.of(&c.key), c.count, &*c.key)).collect();
            queue.extend(terminal::records(records));
        }
        lock(&self.viewers).insert(player, Viewer { store: id, queue, remote, at });
    }

    /// What a player watching through an Accessor asked for (`terminal::Request`), checked here.
    pub fn remote_request(&self, host: &mut dyn ServerHost, player: u128, request: terminal::Request) {
        let id = match lock(&self.viewers).get(&player) {
            Some(v) if v.remote => v.store.clone(),
            _ => return,
        };
        match request {
            terminal::Request::Take { serial, right } => {
                if self.awake(&*host, &id) {
                    self.take_from(host, player, &id, serial, right);
                }
            }
            terminal::Request::Close => self.unwatch(player),
        }
    }

    pub fn unwatch(&self, player: u128) {
        lock(&self.viewers).remove(&player);
    }

    /// What changed in store `id` since it was last asked, queued for everyone watching it.
    fn broadcast(&self, host: &dyn ServerHost, id: &str) {
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
    pub fn pump(&self, host: &mut dyn ServerHost) {
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
            if let Some(player) = host.player(player) {
                let _ = player.send_payload(host, &PayloadId(terminal::CHANNEL.into()), PayloadBytes(m));
            }
        }
    }

    /// A player clicked the terminal's grid: a stack of what they clicked (one with the right
    /// button) goes into their inventory; what doesn't fit stays in the store.
    /// Checked here on the server, whatever the client sent: the serial must name a key of this
    /// store, the player must be trusted, and no more comes out than is stored.
    pub fn take_to(&self, host: &mut dyn ServerHost, player: u128, pos: [i32; 3], serial: u32, right: bool) {
        let Some(id) = self.id_under(&*host, pos) else { return };
        self.take_from(host, player, &id, serial, right);
    }

    fn take_from(&self, host: &mut dyn ServerHost, player: u128, id: &str, serial: u32, right: bool) {
        let id = id.to_string();
        if !self.may_use(&*host, &id, player) {
            return;
        }
        let (key, item, stack) = {
            let mut stores = lock(&self.stores);
            let open = self.open(&*host, &mut stores, &id);
            let Some(key) = open.serials.key(serial).cloned() else { return };
            let Some(probe) = stack_of(&*host, &key, 1) else { return };
            let want = if right { 1 } else { host.max_stack_size(probe.item()).unwrap_or(64) };
            let took = open.store.extract(&key, u64::from(want)) as u32;
            if took == 0 {
                return;
            }
            let stack = stack_of(&*host, &key, took);
            (key, game::id(&*host, &probe).to_string(), stack)
        };
        let Some(stack) = stack else { return };
        // What didn't fit goes back under the same key, past the limits if it must.
        if let Some(left) = game::give(host, player, stack) {
            let mut stores = lock(&self.stores);
            self.open(&*host, &mut stores, &id).store.put_back(&item, &key, u64::from(left.count()));
        }
    }

    /// A shift-click from the player's inventory on the terminal: as much of `stack` as the store
    /// takes goes straight in. What it took.
    /// After `player`'s tick, while their inventory can be read: each stack they shift-clicked
    /// goes from it into the store, a stabilizer plate into an upgrade slot, one at a time. What
    /// doesn't fit stays.
    pub fn take_moves(&self, host: &mut dyn ServerHost, player: u128) {
        let moves: Vec<_> = {
            let mut all = lock(&self.moves);
            let (mine, others) = std::mem::take(&mut *all).into_iter().partition(|m| m.0 == player);
            *all = others;
            mine
        };
        for (player, pos, slot, resource) in moves {
            let Some(entity) = host.player(player).map(PlayerRef::entity) else { continue };
            let Some(items) = entity.capability(host, &ENTITY_ITEMS, ()) else { continue };
            // The clicked stack, where the menu's numbering puts it, else the first one like it.
            let size = items.size(host).min(36);
            let Some(slot) = std::iter::once(slot).chain(0..size).find(|&i| items.resource(host, i).as_ref() == Some(&resource)) else { continue };
            let Some(item) = resource.to_stack(1).map(|s| id(&*host, &s).to_string()) else { continue };
            // Taken first, so nothing is stored twice; what the store doesn't take goes back.
            let count = items.amount(host, slot);
            let came = host.transaction(|host, tx| Ok::<u32, ()>(items.extract(host, tx, slot, &resource, count))).unwrap_or(0);
            let Some(stack) = resource.to_stack(came) else { continue };
            let took = if plate_tier(&item).is_some() {
                let one = resource.to_stack(1);
                (0..came).take_while(|_| one.as_ref().is_some_and(|s| self.upgrade(&*host, player, pos, s))).count() as u32
            } else {
                self.deposit(&*host, player, pos, &stack)
            };
            if let Some(left) = resource.to_stack(came - took.min(came)) {
                let back = host.transaction(|host, tx| Ok::<u32, ()>(items.insert(host, tx, slot, &resource, left.count()))).unwrap_or(0);
                if let Some(rest) = resource.to_stack(left.count() - back.min(left.count())) {
                    let _ = game::give(host, player, rest);
                }
            }
        }
    }

    /// `player` took a crafting result, the grid having been `inputs`: if it was their terminal's,
    /// its emptied cells refill from the store on the block's next tick. Not at once: the game
    /// uses the grid up after this hook, and writes its own copy back over a cell changed in it.
    pub fn crafted(&self, player: u128, inputs: &[Option<ItemStack>]) {
        let Some(at) = lock(&self.viewers).get(&player).and_then(|v| v.at.clone()) else { return };
        lock(&self.refills).entry(at).or_default().push(inputs.to_vec());
    }

    /// The block's tick: cells of its crafting grid a craft emptied take one more of what they
    /// held, if the store has it. Whether the grid changed.
    fn refill_grid(&self, host: &dyn ServerHost, pos: [i32; 3], entity: &mut ModBlockEntityView<'_>) -> bool {
        let Some(crafts) = lock(&self.refills).remove(&(host.dimension().to_string(), pos)) else { return false };
        let Some(id) = self.id_under(host, pos) else { return false };
        if entity.items.len() < CONTAINER_SLOTS {
            entity.items.resize(CONTAINER_SLOTS, None);
        }
        let mut stores = lock(&self.stores);
        let open = self.open(host, &mut stores, &id);
        let mut changed = false;
        for inputs in crafts {
            for (i, was) in inputs.iter().take(GRID.len()).enumerate() {
                let slot = GRID.start + i;
                let Some(was) = was.as_ref().filter(|_| entity.items[slot].is_none()) else { continue };
                let Some(one) = ItemResource::of(was).to_stack(1) else { continue };
                let (_, key, _) = item_key(host, &one);
                if open.store.extract(&key, 1) == 1 {
                    entity.items[slot] = Some(one);
                    changed = true;
                }
            }
        }
        changed
    }

    /// The clear button: what the crafting grid holds goes into the store (past its limits if it
    /// must: nothing is lost).
    pub fn clear_grid(&self, host: &dyn ServerHost, player: u128, pos: [i32; 3], entity: &mut ModBlockEntityView<'_>) {
        let Some(id) = self.id_under(host, pos) else { return };
        if !self.may_use(host, &id, player) {
            return;
        }
        for slot in GRID {
            if let Some(stack) = entity.items.get_mut(slot).and_then(Option::take) {
                self.store_back(host, &id, &stack);
            }
        }
    }

    /// A shift-click on a crafting grid cell: its stack goes into the store, as far as it takes it,
    /// on the block's next tick.
    pub fn grid_to_store(&self, host: &dyn ServerHost, player: u128, pos: [i32; 3], slot: usize) {
        lock(&self.grid_moves).entry((host.dimension().to_string(), pos)).or_default().push((player, slot));
    }

    /// The block's tick: the grid cells shift-clicked go into the store. Whether the grid changed.
    fn empty_grid_cells(&self, host: &dyn ServerHost, pos: [i32; 3], entity: &mut ModBlockEntityView<'_>) -> bool {
        let Some(moves) = lock(&self.grid_moves).remove(&(host.dimension().to_string(), pos)) else { return false };
        let mut changed = false;
        for (player, slot) in moves {
            let Some(stack) = entity.items.get(slot).cloned().flatten() else { continue };
            let took = self.deposit(host, player, pos, &stack);
            if took > 0 {
                entity.items[slot] = stack.shrink(took);
                changed = true;
            }
        }
        changed
    }

    /// `stack` back into store `id`, past its limits if it must.
    fn store_back(&self, host: &dyn ServerHost, id: &str, stack: &ItemStack) {
        let (item, key, count) = item_key(host, stack);
        let mut stores = lock(&self.stores);
        self.open(host, &mut stores, id).store.put_back(&item, &key, count);
    }

    pub fn deposit(&self, host: &dyn ServerHost, player: u128, pos: [i32; 3], stack: &ItemStack) -> u32 {
        let Some(id) = self.id_under(host, pos) else { return 0 };
        if !self.may_use(host, &id, player) {
            return 0;
        }
        let (item, key, count) = item_key(host, stack);
        let mut stores = lock(&self.stores);
        self.open(host, &mut stores, &id).store.insert(&item, &key, count) as u32
    }
}

/// The Actuator's and a wormhole's container: hoppers in from the top and sides. The terminal
/// hands items to players, so the bottom face's slots stay empty (logistics go through wormholes).
pub fn container() -> ModContainer {
    let (inputs, outputs): (Vec<usize>, Vec<usize>) = (INPUTS.collect(), OUTPUTS.collect());
    let mut faces: [Vec<usize>; 6] = std::array::from_fn(|_| inputs.clone());
    faces[0] = outputs.clone();
    ModContainer { slots: CONTAINER_SLOTS, faces, inputs, outputs, menu: Some(MENU.to_string()), ..Default::default() }
}

/// The block at `pos` in the level in place, by id.
fn block_here(host: &dyn Host, pos: [i32; 3]) -> &'static str {
    host.level(host.dimension()).map_or("", |l| game::block_name(&*host, l.block(host, pos)))
}

/// The projection over a Storage Actuator, after Occultism's `StorageControllerGeoRenderer`: its
/// dimensional matrix (27 small cubes) turning a block and a quarter up, in colours cycling as
/// Occultism's do, and one chalk glyph per plate circling it (`network::glyph_orbit`), in its
/// tier's colour, always facing the viewer as a particle does. It glows: drawn at full light.
fn draw_projection(host: &dyn ClientHost, draw: &mut dyn BlockEntityDrawContext, plates: &[u8]) {
    let t = draw.game_time() as f32 + draw.partial();
    let light = draw.light();
    draw.set_light(0xF0F0);
    // The matrix's centre: Occultism's model is 9 texels tall from 5 above its pose's origin, set
    // a little higher than Occultism's so the lowest orbits clear the block.
    let rise = 1.25 + 0.2;
    let centre = [0.5, rise + 9.0 / 16.0, 0.5];

    // The glyphs: flat squares turned to the camera. The camera's right and up, from its yaw and
    // pitch (Minecraft's: yaw 0 looks south, pitch down is positive).
    if let Some(camera) = host.camera() {
        let (yaw, pitch) = (camera.yaw.to_radians(), camera.pitch.to_radians());
        let forward = [-yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos()];
        let right = [-yaw.cos(), 0.0, -yaw.sin()];
        let up = [
            right[1] * forward[2] - right[2] * forward[1],
            right[2] * forward[0] - right[0] * forward[2],
            right[0] * forward[1] - right[1] * forward[0],
        ];
        // A square of half-size `size` at `c`, turned to the camera, pushed `back` away from it so
        // what is drawn at one place layers: trail, then glow, then the glyph in front.
        let billboard = |draw: &mut dyn BlockEntityDrawContext, sprite: &str, c: [f32; 3], size: f32, back: f32, tint: u32| {
            let corner = |r: f32, u: f32| [0, 1, 2].map(|i| c[i] + (right[i] * r + up[i] * u) * size + forward[i] * back);
            let quad = [corner(-1.0, 1.0), corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0)];
            draw.quad(sprite, quad, [0.0, 0.0, 16.0, 16.0], tint);
        };
        let at_time = |slot: usize, tier: u8, when: f32| {
            let at = network::glyph_orbit(slot, tier, when);
            [centre[0] + at[0], centre[1] + at[1], centre[2] + at[2]]
        };
        for (slot, tier) in plates.iter().copied().enumerate() {
            let (Some(colour), Some(glow)) = (network::glyph_colour(tier), network::glyph_glow(tier)) else { continue };
            // A trail of motes along the orbit behind it, smaller and fainter the further back.
            for k in 1..=network::TRAIL_MOTES {
                let mote = at_time(slot, tier, t - k as f32 * network::TRAIL_STEP);
                let fade = k as f32 / network::TRAIL_MOTES as f32;
                let strength = ((fade * 4.0) as usize).min(3);
                let breathe = 1.0 + 0.15 * (t * 0.3 + k as f32).sin();
                billboard(draw, &format!("occultism:block/storage_glow_{strength}"), mote, 0.05 * (1.0 - 0.5 * fade) * breathe, 0.02, glow);
            }
            let c = at_time(slot, tier, t);
            // A soft glow behind it, breathing slowly.
            let pulse = 1.0 + 0.08 * (t * 0.12 + slot as f32 * 1.3).sin();
            billboard(draw, "occultism:block/storage_glow_0", c, 0.24 * pulse, 0.01, glow);
            // The glyph; each slot shows a sign of its own.
            billboard(draw, &format!("occultism:block/chalk_glyph/{colour}/{}", 1 + slot * 5 % 14), c, 0.16, 0.0, 0xFFFFFF);
        }
    }

    // The matrix: 3 x 3 x 3 cubes of 2 texels, 3 apart, turning slowly (Occultism: 22.5 degrees a
    // second), tinted by its hue cycle.
    let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis()) as i64;
    let scale = 100 - ((millis / 16 / 2) % 160 - 80).abs();
    let saturation = ((millis as f64 / 8.0).to_radians().sin() * 0.5 + 0.5) as f32;
    let tint = hsb(0.01 * scale as f32, saturation, 0.01 * scale as f32);
    draw.push();
    draw.translate([0.5, rise, 0.5]);
    draw.rotate_about([0.0, 1.0, 0.0], (millis / 16 % 360) as f32, [0.0; 3]);
    let face = Some(CubeFace::new(MATRIX_SPRITE));
    for x in [-4.0, -1.0, 2.0] {
        for y in [5.0, 8.0, 11.0] {
            for z in [-4.0, -1.0, 2.0] {
                draw.cube([x, y, z], [x + 2.0, y + 2.0, z + 2.0], [face; 6], tint);
            }
        }
    }
    draw.pop();
    draw.set_light(light);
}

/// Java's `Color.HSBtoRGB`, as 0xRRGGBB.
fn hsb(hue: f32, saturation: f32, brightness: f32) -> u32 {
    let channel = |v: f32| ((v * 255.0 + 0.5) as u32).min(255);
    if saturation == 0.0 {
        let v = channel(brightness);
        return v << 16 | v << 8 | v;
    }
    let h = (hue - hue.floor()) * 6.0;
    let f = h - h.floor();
    let p = brightness * (1.0 - saturation);
    let q = brightness * (1.0 - saturation * f);
    let u = brightness * (1.0 - saturation * (1.0 - f));
    let (r, g, b) = match h as u32 {
        0 => (brightness, u, p),
        1 => (q, brightness, p),
        2 => (p, brightness, u),
        3 => (p, q, brightness),
        4 => (u, p, brightness),
        _ => (brightness, p, q),
    };
    channel(r) << 16 | channel(g) << 8 | channel(b)
}

/// The Actuator's and a stable wormhole's block entity: the store's terminal and hopper face.
pub struct StorePart(pub Arc<crate::State>);

#[mod_api::horizon_part]
impl BlockPart for StorePart {}

/// A stable wormhole faces away from the block it was placed against (Occultism's clicked face).
pub struct GatePlacement;

/// The facing a wormhole placed against `face` takes.
pub fn gate_facing(face: Direction) -> &'static str {
    match face {
        Direction::Down => "down",
        Direction::Up => "up",
        Direction::North => "north",
        Direction::South => "south",
        Direction::West => "west",
        Direction::East => "east",
    }
}

// Its BlockPart is in src/platform/: the placement hook differs between Horizon and Sunrise.

#[mod_api::horizon_part]
impl BlockEntityPart for StorePart {
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let vault = &self.0.vault;
        let emptied = vault.empty_grid_cells(&*host, event.pos, &mut event.entity);
        if vault.refill_grid(&*host, event.pos, &mut event.entity) || emptied {
            host.block_entity_sync(event.pos);
        }
        if block_here(&*host, event.pos) == WORMHOLE {
            vault.gate_tick(&*host, event.pos, &mut event.entity);
            return;
        }
        let (notice, sync) = vault.tick(&*host, event.pos, &mut event.entity);
        if let Some(notice) = notice {
            tell(host, notice.player, say(notice.key, &[&notice.name]));
        }
        if sync {
            host.block_entity_sync(event.pos);
        }
    }

    fn block_entity_update_tag(&self, event: &mut BlockEntityUpdateTagEvent<'_>) -> Option<Compound> {
        let mut tag = Compound::new();
        if let Some(plates) = event.entity.nbt.get(&"plates".into()) {
            tag.insert("plates".into(), plates.clone());
        }
        Some(tag)
    }

    fn block_entity_submit(&self, host: &dyn ClientHost, event: &mut BlockEntitySubmitEvent<'_>) {
        if event.block != ACTUATOR {
            return;
        }
        let plates = match event.entity.nbt.get(&"plates".into()) {
            Some(Tag::ByteArray(p)) => p.clone(),
            _ => Vec::new(),
        };
        draw_projection(host, &mut *event.draw, &plates);
    }

    fn block_entity_render_bounding_box(&self, _host: &dyn ClientHost, event: &mut BlockEntityRenderBoundingBoxEvent<'_>) {
        if event.block == ACTUATOR {
            // The glyphs circle under a block from the matrix's centre.
            let b = &mut *event.bounds;
            (b[0], b[2], b[3], b[5]) = (b[0].min(-0.5), b[2].min(-0.5), b[3].max(1.5), b[5].max(1.5));
            b[4] = b[4].max(2.8);
        }
    }

    // #horizon-begin: Sunrise asks the menu's Part for its screen (sunrise/platform.rs).
    fn screen(&self, _host: &dyn Host, _event: &mut ScreenEvent<'_>) -> Option<Screen> {
        Some(self.0.ui.screen())
    }

    fn screen_draw(&self, host: &dyn ClientHost, event: &mut ScreenDrawEvent<'_>) {
        self.0.ui.draw(host, &mut *event.draw, event.origin, event.data);
    }

    fn screen_click(&self, _host: &dyn ClientHost, event: &mut ScreenClickEvent<'_>) -> Option<i32> {
        self.0.ui.click(event.at, event.right)
    }

    fn screen_tooltip(&self, host: &dyn ClientHost, event: &mut ScreenTooltipEvent<'_>) {
        self.0.ui.tooltip(host, event.at, event.data, event.lines);
    }
    // #horizon-end

    fn menu_button(&self, host: &mut dyn ServerHost, event: &mut MenuButtonEvent<'_>) {
        let (player, pos, button) = (event.context.player, event.context.pos, event.context.id);
        if let Some((serial, right)) = terminal::grid_click(button) {
            self.0.vault.take_to(host, player, pos, serial, right);
        } else if let Some(slot) = crate::terminal_ui::upgrade_click(button) {
            self.0.vault.remove_plate(host, player, pos, slot);
        } else if button == CLEAR_GRID_BUTTON {
            self.0.vault.clear_grid(&*host, player, pos, &mut event.entity);
        }
    }

    fn menu_quick_move(&self, host: &mut dyn ServerHost, event: &mut MenuQuickMoveEvent<'_>) -> Flow {
        // A shift-click from the inventory goes straight into the store: the terminal has no input
        // slots to show. A stabilizer plate goes into an upgrade slot, one at a time.
        let Some(player) = host.players().acting_player().map(PlayerRef::uuid) else { return Flow::Cancel };
        // A crafting grid cell goes into the store (the game crafts from the result itself).
        if GRID.contains(&event.slot) {
            self.0.vault.grid_to_store(&*host, player, event.pos, event.slot);
            return Flow::Cancel;
        }
        if event.slot < event.container_slots {
            return Flow::Continue;
        }
        // The hook can't empty the clicked slot (a stack holds at least one): the whole stack
        // moves from the inventory after the player's tick. Menu slots run main inventory, hotbar.
        let i = event.slot - event.container_slots;
        let slot = if i < 27 { 9 + i } else { i - 27 };
        let Some(stack) = crate::platform::quick_stack(&mut *event.stack) else { return Flow::Continue };
        lock(&self.0.vault.moves).push((player, event.pos, slot, ItemResource::of(stack)));
        Flow::Cancel
    }
}

/// A stable wormhole or a Storage Accessor in hand: sneak-used on an Actuator, linked to its store,
/// as Occultism's. A wormhole placed carries its link to the block; an Accessor opens its store's
/// terminal wherever the player is.
pub struct GateItem(pub Arc<crate::State>);

#[mod_api::horizon_part]
impl ItemPart for GateItem {
    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let player = event.player;
        let HostSide::Server(host) = host else {
            // The client answers a link too, so the Accessor's own use doesn't open its terminal.
            let HostSide::Client(host) = host else { return None };
            let sneaking = host.player(player).and_then(|p| p.entity().is_shift_key_down(&*host)) == Some(true);
            return (sneaking && block_here(&*host, event.pos) == ACTUATOR).then_some(InteractionResult::Success);
        };
        let sneaking = host.player(player).and_then(|p| p.entity().is_shift_key_down(&*host)) == Some(true);
        if !(sneaking && block_here(&*host, event.pos) == ACTUATOR) {
            if id(&*host, event.stack) == WORMHOLE {
                self.0.vault.placing(player, event.stack);
            }
            return None;
        }
        match self.0.vault.link(&*host, player, event.pos) {
            Some((store, name)) => {
                let mut linked = event.stack.clone();
                set_link(&mut linked, &store, &name);
                if let Some(living) = game::living(&*host, player) {
                    let _ = living.set_hand_item(host, event.hand, Some(linked));
                }
                tell(host, player, say("storage.occultism.linked", &[&name]));
            }
            None => {
                let dimension = host.dimension();
                if let Err(name) = self.0.vault.may_open(&*host, dimension, event.pos, player) {
                    tell(host, player, say("storage.occultism.refused", &[&name]));
                }
            }
        }
        Some(InteractionResult::SuccessServer)
    }

    fn use_item(&self, host: HostSide, event: &mut UseItemEvent<'_>) -> Option<InteractionResult> {
        let accessor = match &host {
            HostSide::Server(h) => id(&**h, event.stack) == ACCESSOR,
            HostSide::Client(h) => id(&**h, event.stack) == ACCESSOR,
        };
        if !accessor {
            return None;
        }
        let HostSide::Server(host) = host else { return Some(InteractionResult::Success) };
        let player = host.players().acting_player()?.uuid();
        if let Err((key, name)) = self.0.vault.watch_remote(&*host, player, event.stack) {
            tell(host, player, say(key, &[&name]));
        }
        Some(InteractionResult::SuccessServer)
    }

    fn tooltip(&self, _host: &dyn ClientHost, event: &mut TooltipEvent<'_>) {
        if let Some((_, name)) = linked(event.stack) {
            event.lines.push((say("storage.occultism.linked_to", &[&name]), 0xFF8E6CC0));
        }
    }
}

/// The Actuator as an item: it carries its store, and the block it places takes it.
pub struct CoreItem(pub Arc<crate::State>);

#[mod_api::horizon_part]
impl ItemPart for CoreItem {
    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        if matches!(host, HostSide::Server(_)) {
            self.0.vault.placing(event.player, event.stack);
        }
        None
    }

    fn tooltip(&self, _host: &dyn ClientHost, event: &mut TooltipEvent<'_>) {
        if let Some((_, name, _)) = carried(event.stack) {
            event.lines.push((say("storage.occultism.bound", &[&name]), 0xFF8E6CC0));
        }
    }
}
