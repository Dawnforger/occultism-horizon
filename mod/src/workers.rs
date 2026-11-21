// Occultism's working spirits: a foliot summoned with a job, bound to a book of calling. The
// janitor (`CleanerJob`) picks up the items lying in its work area (`PickupItemsGoal`), carries them
// to the container the book set (`DepositItemsGoal`) and goes back to its base
// (`ReturnToWorkAreaGoal`). The transporter (`TransportItemsJob`) takes a stack at a time from one
// container (`ExtractItemsGoal`) and carries it to another. The lumberjack (`LumberjackJob`) fells
// the trees in its work area (`FellTreeBehaviour`), gathers their wood, leaves, saplings, fruit and
// sticks, takes them to its container and replants where it felled (`ReplantSaplingBehaviour`).
// The farmer (`FarmerJob`) harvests the ripe crops on the farmland in its work area, replanting
// each (`HarvestCropBehaviour`), and takes the seeds and crops to its container. The crushers,
// crystallizers and smelters (`CrusherJob`, `CrystallizerJob`, `SmelterJob`: a foliot, djinni, afrit
// or marid, each at its rank) pick up what their recipes take, work it in their hands and drop the
// result at their feet.
//
// Each spirit is a mod mob copying a wolf (a creature, so it stays on Peaceful), stripped of the
// wolf's own goals and sounds and drawn as Occultism's model. Its job, base, work area and deposit
// are its Part state, saved with it; its one inventory slot is its main hand, as Occultism's is.
// Goals only read the world: what they do to it (an item picked up, a stack put in a chest) waits
// for the end of the server tick.

use crate::game::{self, id, lock, tell};
use crate::logic::{self, Job, Step, Tier};
use mod_api::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub const FOLIOT: &str = "occultism:foliot";
pub const DJINNI: &str = "occultism:djinni";
pub const AFRIT: &str = "occultism:afrit";
pub const MARID: &str = "occultism:marid";
/// The working spirits, with each one's health (`createAttributes`).
pub const KINDS: [(&str, f64); 4] = [(FOLIOT, 20.0), (DJINNI, 100.0), (AFRIT, 200.0), (MARID, 300.0)];

/// Whether `kind` is one of the working spirits.
pub fn is_worker(kind: &str) -> bool {
    KINDS.iter().any(|(k, _)| *k == kind)
}

/// The processing jobs, each at its spirit's rank (`crush_tier1` to `crush_tier4`, `crystal_tier1`
/// to 4, `smelt_tier1` to 4).
const PROCESSING: [(&str, Job, Tier); 12] = [
    ("occultism:crush_tier1", Job::Crusher, Tier::Foliot),
    ("occultism:crush_tier2", Job::Crusher, Tier::Djinni),
    ("occultism:crush_tier3", Job::Crusher, Tier::Afrit),
    ("occultism:crush_tier4", Job::Crusher, Tier::Marid),
    ("occultism:crystal_tier1", Job::Crystallizer, Tier::Foliot),
    ("occultism:crystal_tier2", Job::Crystallizer, Tier::Djinni),
    ("occultism:crystal_tier3", Job::Crystallizer, Tier::Afrit),
    ("occultism:crystal_tier4", Job::Crystallizer, Tier::Marid),
    ("occultism:smelt_tier1", Job::Smelter, Tier::Foliot),
    ("occultism:smelt_tier2", Job::Smelter, Tier::Djinni),
    ("occultism:smelt_tier3", Job::Smelter, Tier::Afrit),
    ("occultism:smelt_tier4", Job::Smelter, Tier::Marid),
];

fn processing(job: &str) -> Option<(Job, Tier)> {
    PROCESSING.iter().find(|(j, _, _)| *j == job).map(|(_, kind, tier)| (*kind, *tier))
}

/// How long a crusher leaves what it dropped before picking it up again (`crusherResultPickupDelay`),
/// in ticks.
const CRUSHER_PICKUP_DELAY: u64 = 60;
/// How long anyone waits to pick up what a crusher drops (`spawnAtLocation`'s pickup delay), in ticks.
const DROP_PICKUP_DELAY: i16 = 10;
pub const CLEANER: &str = "occultism:cleaner";
pub const CLEANER_BOOK: &str = "occultism:book_of_calling_foliot_cleaner";
pub const TRANSPORTER: &str = "occultism:transport_items";
pub const TRANSPORTER_BOOK: &str = "occultism:book_of_calling_foliot_transport_items";
pub const LUMBERJACK: &str = "occultism:lumberjack";
pub const LUMBERJACK_BOOK: &str = "occultism:book_of_calling_foliot_lumberjack";
pub const FARMER: &str = "occultism:farmer";
pub const FARMER_BOOK: &str = "occultism:book_of_calling_foliot_farmer";
pub const BOOKS: [&str; 4] = [CLEANER_BOOK, TRANSPORTER_BOOK, LUMBERJACK_BOOK, FARMER_BOOK];
/// What a farmer picks up (`FarmerJob.onInit`): seeds and crops.
const HARVEST: [&str; 2] = ["c:seeds", "c:crops"];
/// How near a crop a farmer harvests (`HARVEST_CROP_RANGE_SQUARE`).
const HARVEST_RANGE: f64 = 2.5;
/// How often a farmer looks for ripe crops, and how long it waits after finding none
/// (`NearestCropSensor`), in ticks.
const CROP_SCAN: u64 = 100;
const NO_CROP_WAIT: u64 = 400;
/// The port's datura crop, a crop as vanilla's are.
const DATURA_CROP: &str = "occultism:datura_seeds";
/// What a lumberjack picks up (`LumberjackJob.onInit`): item tags, and sticks.
const LUMBER: [&str; 4] = ["minecraft:logs", "minecraft:leaves", "minecraft:saplings", "c:fruits"];
/// How near a tree a lumberjack fells it, and a stump it replants (`FELL_TREE_RANGE_SQUARE`).
const FELL_RANGE: f64 = 3.5;
/// How long a lumberjack chops before the tree falls, in ticks.
const FELL_TICKS: u32 = 160;
/// How many logs one felling takes, and how far from the stump (`maxFelledBlocks`,
/// `maxFellHorizontalDistance`, Occultism's defaults).
const MAX_FELLED: usize = 512;
const MAX_FELL_DISTANCE: i32 = 24;
/// How often a lumberjack looks for a tree, and how long it waits after finding none
/// (`NearestTreeSensor`), in ticks.
const TREE_SCAN: u64 = 100;
const NO_TREE_WAIT: u64 = 600;
/// A book of calling's bound spirit, mode, work area size and the places it set.
pub const CALLING_COMPONENT: &str = "occultism:calling";

/// Occultism's `WorkAreaSize`: the half-width of the box a spirit works in, and its name.
pub const AREA_SIZES: [(i32, &str); 3] = [(16, "small"), (32, "medium"), (64, "large")];
/// The book of calling modes each book has (`getItemModes`), as Occultism names them.
fn modes(book: &str) -> &'static [&'static str; 2] {
    if book == TRANSPORTER_BOOK { &["set_extract", "set_deposit"] } else { &["set_base", "set_deposit"] }
}
/// How near an item a spirit picks it up (`pickupRange`).
const PICKUP_RANGE: f64 = 2.0;
/// How near the container a spirit puts its load in (`accessDistance`).
const ACCESS_DISTANCE: f64 = 2.2;
/// How near the container a spirit takes from it (`ExtractItemsGoal`'s `accessDistance`).
const EXTRACT_DISTANCE: f64 = 1.86;
/// How near the container a spirit that has stopped walking still reaches it: its wolf's path
/// ends a little short of where Occultism's would.
const STOPPED_REACH: f64 = 3.0;
/// How long a spirit waits when its container is full (`pause(2000)`), in ticks.
const FULL_PAUSE: u64 = 40;
/// The goals a spirit keeps of its wolf's: floating, and looking about.
const KEEP_GOALS: [&str; 3] = ["float", "look_at_player", "random_look_around"];

/// What a working spirit does, and where: its Part state.
#[derive(Clone, Debug, Default, PartialEq, mod_api::Wire)]
pub struct Work {
    /// Its job (`occultism:cleaner`); empty for none.
    pub job: String,
    /// Its base (`workAreaPosition`); none for wherever it is.
    pub base: Option<[i32; 3]>,
    /// Its work area's half-width, one of `AREA_SIZES`.
    pub size: i32,
    /// The container it deposits in, and the face it puts things in through (0 down to 5 east,
    /// as vanilla numbers directions).
    pub deposit: Option<([i32; 3], u8)>,
    /// The container it takes from, and the face it takes through.
    pub extract: Option<([i32; 3], u8)>,
}

fn encode(work: &Work) -> Result<Compound, AttachmentError> {
    let mut c = Compound::new();
    c.insert("job".into(), Tag::String(work.job.as_str().into()));
    if let Some(b) = work.base {
        c.insert("base".into(), Tag::IntArray(b.to_vec()));
    }
    c.insert("size".into(), Tag::Int(work.size));
    if let Some((d, face)) = work.deposit {
        c.insert("deposit".into(), Tag::IntArray(d.to_vec()));
        c.insert("face".into(), Tag::Byte(face as i8));
    }
    if let Some((e, face)) = work.extract {
        c.insert("extract".into(), Tag::IntArray(e.to_vec()));
        c.insert("extract_face".into(), Tag::Byte(face as i8));
    }
    Ok(c)
}

fn decode(c: &Compound) -> Result<Work, AttachmentError> {
    let pos = |key: &str| match c.get(&key.into()) {
        Some(Tag::IntArray(v)) if v.len() == 3 => Some([v[0], v[1], v[2]]),
        _ => None,
    };
    let face = |key: &str| match c.get(&key.into()) {
        Some(Tag::Byte(f)) => (*f as u8).min(5),
        _ => 1,
    };
    Ok(Work {
        job: game::text(c, "job").unwrap_or_default(),
        base: pos("base"),
        // Only a size the soul gem's settings offer: the farmer and lumberjack scan about size^3
        // blocks, so a forged one would stall the server.
        size: match c.get(&"size".into()) {
            Some(Tag::Int(s)) if AREA_SIZES.iter().any(|(a, _)| a == s) => *s,
            _ => AREA_SIZES[0].0,
        },
        deposit: pos("deposit").map(|d| (d, face("face"))),
        extract: pos("extract").map(|e| (e, face("extract_face"))),
    })
}

pub static WORK: AttachmentType<Work> = AttachmentType {
    namespace: "occultism",
    name: "work",
    default: || Work { size: AREA_SIZES[0].0, ..Work::default() },
    codec: Some(AttachmentCodec { encode, decode }),
    copy_on_death: false,
    sync: false,
};

/// A foliot's job, sent to the clients that see it: what it looks like.
pub static LOOK: AttachmentType<String> = AttachmentType {
    namespace: "occultism",
    name: "look",
    default: String::new,
    codec: Some(AttachmentCodec {
        encode: |job| {
            let mut c = Compound::new();
            c.insert("job".into(), Tag::String(job.as_str().into()));
            Ok(c)
        },
        decode: |c| Ok(game::text(c, "job").unwrap_or_default()),
    }),
    copy_on_death: false,
    sync: true,
};

/// The jobs of the foliots this client sees, by entity id.
pub fn seen_jobs(host: &dyn Host) -> HashMap<u32, String> {
    let Some(batch) = crate::platform::batch(host.attachment_query(LOOK.namespace, LOOK.name)) else { return HashMap::new() };
    let Ok(query) = batch.query(&LOOK) else { return HashMap::new() };
    query
        .iter()
        .filter_map(|row| {
            let row = row.ok()?;
            let AttachmentHolder::Entity(entity) = row.holder else { return None };
            Some((entity.id(), row.value.clone()))
        })
        .collect()
}

/// The look a spirit of `kind` doing `job` has (`spirit_model::LOOKS`): a foliot's is its job's, a
/// djinni's, afrit's or marid's its worker model.
pub fn look_of(kind: &str, job: Option<&str>) -> &'static str {
    match (kind, job) {
        (DJINNI, _) => DJINNI,
        (AFRIT, _) => AFRIT,
        (MARID, _) => MARID,
        (_, Some(TRANSPORTER)) => "occultism:foliot/transport_items",
        (_, Some(LUMBERJACK)) => "occultism:foliot/lumberjack",
        (_, Some(FARMER)) => "occultism:foliot/farmer",
        (_, Some(job)) if matches!(processing(job), Some((Job::Crusher, _))) => "occultism:foliot/crusher",
        // Occultism has no foliot smelter or crystallizer model: they are its foliot worker.
        (_, Some(job)) if processing(job).is_some() => "occultism:foliot/worker",
        _ => FOLIOT,
    }
}

/// A direction by vanilla's number.
fn direction(face: u8) -> Direction {
    [Direction::Down, Direction::Up, Direction::North, Direction::South, Direction::West, Direction::East][usize::from(face.min(5))]
}

/// A clicked face's number, from its normal.
fn face_number(face: [i32; 3]) -> u8 {
    match face {
        [0, -1, 0] => 0,
        [0, 1, 0] => 1,
        [0, 0, -1] => 2,
        [0, 0, 1] => 3,
        [-1, 0, 0] => 4,
        _ => 5,
    }
}

const FACE_NAMES: [&str; 6] = ["down", "up", "north", "south", "west", "east"];

/// What a goal asks done to the world, at the end of the tick.
enum Act {
    /// The spirit takes what it can of the item entity into its hand.
    Pickup { spirit: u32, item: u32 },
    /// The spirit puts what it holds into its container.
    Deposit { spirit: u32 },
    /// The spirit takes a stack from the container it extracts from.
    Extract { spirit: u32 },
    /// The lumberjack's tree, from its stump, falls.
    Fell { spirit: u32, stump: [i32; 3] },
    /// The lumberjack plants a sapling it holds where it felled.
    Replant { spirit: u32 },
    /// The farmer harvests the ripe crops around `crop`.
    Harvest { spirit: u32, crop: [i32; 3] },
}

/// The working spirits the server knows: their work, by entity id (read by the goals, which can't
/// reach the Part state from inside their mob's tick), and what their goals asked.
#[derive(Default)]
pub struct Workers {
    work: std::sync::Mutex<HashMap<u32, Work>>,
    /// Where each spirit was at the end of the last tick, for muting its wolf's sounds.
    at: std::sync::Mutex<HashMap<u32, [f64; 3]>>,
    acts: std::sync::Mutex<Vec<Act>>,
    /// Foliots that joined this tick, to finish setting up.
    fresh: std::sync::Mutex<Vec<u32>>,
    /// Spirits whose container was full, and the tick they try again.
    paused: std::sync::Mutex<HashMap<u32, u64>>,
    ticks: AtomicU64,
    /// Books of calling waiting for their casters' ticks (a player's inventory takes items only
    /// then), and where to drop them if they don't fit.
    gifts: std::sync::Mutex<Vec<(u128, ItemStack, [f64; 3])>>,
    /// Where each lumberjack felled, to replant (`LAST_FELLED_TREE`).
    stumps: std::sync::Mutex<HashMap<u32, Vec<[i32; 3]>>>,
    /// Each spirit's kind, by entity id.
    kinds: std::sync::Mutex<HashMap<u32, &'static str>>,
    /// Each crusher's progress (`currentRecipe`, `crushingTimer`).
    crushing: std::sync::Mutex<HashMap<u32, logic::Crushing>>,
    /// What crushers dropped, by item entity id, and the tick they dropped it (`DROPPED_BY_CRUSHER`).
    dropped: std::sync::Mutex<HashMap<u32, u64>>,
}

impl Workers {
    fn work(&self, spirit: u32) -> Option<Work> {
        lock(&self.work).get(&spirit).cloned()
    }

    /// `spirit`'s work changed by `edit`: in its Part state, and for its goals.
    fn edit(&self, host: &mut dyn ServerHost, spirit: EntityRef, edit: impl FnOnce(&mut Work)) -> Option<Work> {
        let work = WorkerPart::edit_state(host, spirit, |_, w| {
            edit(w);
            w.clone()
        })
        .ok()?;
        lock(&self.work).insert(spirit.id(), work.clone());
        Some(work)
    }

    /// A spirit of `kind` came into the world: its work for its goals, and none of its wolf's ways.
    pub fn joined(&self, host: &mut dyn ServerHost, entity: u32, kind: &str) {
        let Some(spirit) = host.entity(entity) else { return };
        let kind = KINDS.iter().find(|(k, _)| *k == kind).map_or(FOLIOT, |(k, _)| *k);
        lock(&self.kinds).insert(entity, kind);
        let work = WorkerPart::state(&*host, spirit).unwrap_or_else(|_| (WORK.default)());
        lock(&self.work).insert(entity, work);
        if let Some(mob) = host.mob(spirit) {
            for goal in mob.goals(&*host).unwrap_or_default() {
                if !KEEP_GOALS.contains(&goal.id) && !OUR_GOALS.contains(&goal.id) {
                    mob.remove_goal(host, goal.id);
                }
            }
            for goal in mob.target_goals(&*host).unwrap_or_default() {
                if !OUR_GOALS.contains(&goal.id) {
                    mob.remove_target_goal(host, goal.id);
                }
            }
        }
        // What it carries falls when it dies, whole (`setGuaranteedDrop`).
        let mut chances = Compound::new();
        chances.insert("mainhand".into(), Tag::Float(2f32.to_bits()));
        let mut nbt = Compound::new();
        nbt.insert("drop_chances".into(), Tag::Compound(chances));
        spirit.merge_nbt(host, nbt);
        lock(&self.fresh).push(entity);
    }

    /// What a spirit that just came can't be given while it joins: its health.
    fn settle(&self, host: &mut dyn ServerHost) {
        for entity in std::mem::take(&mut *lock(&self.fresh)) {
            let Some(spirit) = host.entity(entity) else { continue };
            // Its work as it is now: a soul gem gives a released spirit its work after it joins.
            if let Ok(work) = WorkerPart::state(&*host, spirit) {
                self.show(host, spirit, &work.job);
                // A transporter goes through doors (`TransportItemsJob.onInit`).
                if work.job == TRANSPORTER
                    && let Some(mob) = host.mob(spirit)
                {
                    mob.set_can_open_doors(host, true);
                    mob.set_can_pass_doors(host, true);
                }
                lock(&self.work).insert(entity, work);
            }
            // A wolf that isn't tamed has 8 health, set again each time it loads; a foliot has 20
            // and a marid 300 (`createAttributes`). Its health is lost to the wolf's cap on
            // loading, so a spirit loads whole.
            let kind = lock(&self.kinds).get(&entity).copied().unwrap_or(FOLIOT);
            let health = KINDS.iter().find(|(k, _)| *k == kind).map_or(20.0, |(_, h)| *h);
            if let Some(living) = host.living(spirit)
                && living.attribute_base(&*host, "minecraft:max_health") != Some(health)
            {
                living.set_attribute_base(host, "minecraft:max_health", health);
                living.set_health(host, health as f32);
            }
        }
    }

    /// The clients that see `spirit` draw it for `job`.
    fn show(&self, host: &mut dyn ServerHost, spirit: EntityRef, job: &str) {
        let holder = Holder::Entity(spirit.id());
        if LOOK.get_existing(host.attachments(), holder).ok().flatten().as_deref() != Some(job) {
            let _ = LOOK.set(host.attachments(), holder, job.to_string());
        }
    }

    /// The end of a server tick: what the goals asked, done; the crushers' work; where the spirits
    /// are now.
    pub fn tick(&self, host: &mut dyn ServerHost, recipes: &crate::crushing::Crushing) {
        let now = self.ticks.fetch_add(1, Ordering::Relaxed);
        self.settle(host);
        let acts = std::mem::take(&mut *lock(&self.acts));
        for act in acts {
            match act {
                Act::Pickup { spirit, item } => self.pickup(host, spirit, item),
                Act::Deposit { spirit } => self.deposit(host, spirit, now),
                Act::Extract { spirit } => self.extract(host, spirit, now),
                Act::Fell { spirit, stump } => self.fell(host, spirit, stump),
                Act::Replant { spirit } => self.replant(host, spirit),
                Act::Harvest { spirit, crop } => self.harvest(host, spirit, crop),
            }
        }
        lock(&self.paused).retain(|_, until| *until > now);
        let workers: Vec<(u32, Job, Tier)> =
            lock(&self.work).iter().filter_map(|(id, w)| processing(&w.job).map(|(job, tier)| (*id, job, tier))).collect();
        for (spirit, job, tier) in workers {
            let recipes = recipes.recipes(&*host, job);
            self.crush(host, spirit, job, tier, &recipes, now);
        }
        lock(&self.dropped).retain(|_, at| now - *at < CRUSHER_PICKUP_DELAY);
        let ids: Vec<u32> = lock(&self.work).keys().copied().collect();
        let mut at = HashMap::new();
        for spirit in ids {
            match host.entity(spirit).filter(|e| host.living(*e).and_then(|l| l.alive(&*host)) == Some(true)) {
                Some(e) => {
                    if let Some(p) = e.position(&*host) {
                        at.insert(spirit, p);
                    }
                }
                None => {
                    lock(&self.work).remove(&spirit);
                    lock(&self.kinds).remove(&spirit);
                    lock(&self.crushing).remove(&spirit);
                }
            }
        }
        *lock(&self.at) = at;

    }

    /// A processing spirit's tick (`CrusherJob.update`, and the crystallizer's and smelter's): what it
    /// holds worked at its rank, the result dropped at its feet; what no recipe at its rank takes,
    /// dropped.
    fn crush(&self, host: &mut dyn ServerHost, spirit: u32, job: Job, tier: Tier, recipes: &[logic::CrushingRecipe], now: u64) {
        let Some(entity) = host.entity(spirit) else { return };
        let (Some(living), Some(level), Some(at)) = (host.living(entity), entity.level(&*host), entity.position(&*host)) else { return };
        let held = living.hand_item(&*host, Hand::Main);
        let item = held.as_ref().map(|h| id(&*host, h).to_string());
        let step = {
            let mut all = lock(&self.crushing);
            let state = all.entry(spirit).or_default();
            logic::crush_step(state, item.as_deref().zip(held.as_ref().map(ItemStack::count)), recipes, job, tier)
        };
        let dim = level.dimension_id().to_string();
        match step {
            Step::NoRecipe => {
                if let Some(held) = held {
                    let _ = living.set_hand_item(host, Hand::Main, None);
                    self.drop_crushed(host, &dim, held, at, now);
                }
            }
            Step::Done { used, result, produced } => {
                let left = held.and_then(|h| h.shrink(used));
                let _ = living.set_hand_item(host, Hand::Main, left);
                if produced > 0
                    && let Some(stack) = game::stack(&*host, &result, produced)
                {
                    self.drop_crushed(host, &dim, stack, at, now);
                }
            }
            Step::Working { .. } | Step::Idle => {}
        }
    }

    /// `stack` dropped at a crusher's feet, marked as its own.
    fn drop_crushed(&self, host: &mut dyn ServerHost, dim: &str, stack: ItemStack, at: [f64; 3], now: u64) {
        let Some(item) = stack.to_nbt(&*host) else { return };
        let mut nbt = Compound::new();
        nbt.insert("Item".into(), Tag::Compound(item));
        nbt.insert("PickupDelay".into(), Tag::Short(DROP_PICKUP_DELAY));
        if let Some(dropped) = host.summon(dim, "minecraft:item", [at[0], at[1] + 0.5, at[2]], nbt) {
            lock(&self.dropped).insert(dropped, now);
        }
    }

    /// Whether a crusher dropped `item` too lately to pick it up again.
    fn dropped_lately(&self, item: u32) -> bool {
        lock(&self.dropped).contains_key(&item)
    }

    fn pickup(&self, host: &mut dyn ServerHost, spirit: u32, item: u32) {
        let (Some(spirit), Some(item)) = (host.entity(spirit), host.entity(item)) else { return };
        let (Some(living), Some(dropped)) = (host.living(spirit), host.item_entity(item)) else { return };
        let Some(lying) = dropped.stack(&*host) else { return };
        let held = living.hand_item(&*host, Hand::Main);
        let max = game::item(&*host, id(&*host, &lying)).and_then(|k| host.max_stack_size(k)).unwrap_or(64);
        let (taken, now_held) = match held {
            None => (lying.count(), lying.clone()),
            Some(h) if ItemResource::of(&h) == ItemResource::of(&lying) && h.count() < max => {
                let n = lying.count().min(max - h.count());
                let mut h = h;
                let _ = h.set_count(h.count() + n);
                (n, h)
            }
            Some(_) => return,
        };
        if !living.set_hand_item(host, Hand::Main, Some(now_held)).unwrap_or(false) {
            return;
        }
        match lying.shrink(taken) {
            Some(left) => {
                let _ = dropped.set_stack(host, left);
            }
            None => {
                item.discard(host);
            }
        }
    }

    fn deposit(&self, host: &mut dyn ServerHost, spirit: u32, now: u64) {
        let Some(work) = self.work(spirit) else { return };
        let Some(entity) = host.entity(spirit) else { return };
        let (Some(living), Some(level)) = (host.living(entity), entity.level(&*host)) else { return };
        let Some(held) = living.hand_item(&*host, Hand::Main) else { return };
        let Some((pos, face)) = work.deposit else { return };
        // No container there any more: the spirit forgets it (`resetTarget`).
        if level.capability(host, &ITEMS, pos, Some(direction(face))).is_none() {
            self.edit(host, entity, |w| w.deposit = None);
            return;
        }
        let count = held.count();
        let left = game::insert(host, level, pos, direction(face), held);
        if left.as_ref().map(ItemStack::count) == Some(count) {
            lock(&self.paused).insert(spirit, now + FULL_PAUSE);
            return;
        }
        let full = left.is_some();
        let _ = living.set_hand_item(host, Hand::Main, left);
        if full {
            lock(&self.paused).insert(spirit, now + FULL_PAUSE);
        }
    }

    /// A stack, the first in the container's slots, into the spirit's empty hand.
    fn extract(&self, host: &mut dyn ServerHost, spirit: u32, now: u64) {
        let Some(work) = self.work(spirit) else { return };
        let Some(entity) = host.entity(spirit) else { return };
        let (Some(living), Some(level)) = (host.living(entity), entity.level(&*host)) else { return };
        if living.hand_item(&*host, Hand::Main).is_some() {
            return;
        }
        let Some((pos, face)) = work.extract else { return };
        let Some(items) = level.capability(host, &ITEMS, pos, Some(direction(face))) else {
            self.edit(host, entity, |w| w.extract = None);
            return;
        };
        let size = items.size(host);
        let Some(slot) = (0..size).find(|&i| items.amount(host, i) > 0) else {
            // Nothing to take: it looks again in a while.
            lock(&self.paused).insert(spirit, now + FULL_PAUSE);
            return;
        };
        if let Some(taken) = game::extract(host, level, pos, direction(face), slot, u32::MAX)
            && let Err(_) | Ok(false) = living.set_hand_item(host, Hand::Main, Some(taken.clone()))
        {
            // The hand wouldn't take it: back where it was.
            let _ = game::insert(host, level, pos, direction(face), taken);
        }
    }

    /// The tree standing on `stump` falls, its logs dropped (`FellTreeBehaviour.fellTree`); the
    /// lumberjack remembers the stump, and the stumps beside it, to replant.
    fn fell(&self, host: &mut dyn ServerHost, spirit: u32, stump: [i32; 3]) {
        let Some(level) = host.entity(spirit).and_then(|e| e.level(&*host)) else { return };
        if !is_log(&*host, level, stump) {
            return;
        }
        // A wide tree's other stumps (`addAllStump`): logs beside it, on dirt.
        let mut stumps = vec![stump];
        let mut look = vec![stump];
        while let Some(at) = look.pop() {
            for [dx, dz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
                let p = [at[0] + dx, at[1], at[2] + dz];
                if !stumps.contains(&p) && within_fell(stump, p) && stumps.len() < 4 && is_log(&*host, level, p) && is_dirt(&*host, level, [p[0], p[1] - 1, p[2]]) {
                    stumps.push(p);
                    look.push(p);
                }
            }
        }
        let mut queue = std::collections::VecDeque::from([stump]);
        let mut seen = std::collections::HashSet::new();
        let mut felled = 0;
        while let Some(p) = queue.pop_front() {
            if felled >= MAX_FELLED {
                break;
            }
            if !seen.insert(p) || !within_fell(stump, p) || !is_log(&*host, level, p) {
                continue;
            }
            for [dx, dz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
                queue.push_back([p[0] + dx, p[1], p[2] + dz]);
            }
            for dx in -1..=1 {
                for dz in -1..=1 {
                    queue.push_back([p[0] + dx, p[1] + 1, p[2] + dz]);
                }
            }
            let _ = level.destroy_block(host, p, true, None, false);
            felled += 1;
        }
        lock(&self.stumps).entry(spirit).or_default().extend(stumps);
    }

    /// A sapling the lumberjack holds into its first stump's place, if that is open, on dirt.
    fn replant(&self, host: &mut dyn ServerHost, spirit: u32) {
        let Some(stump) = lock(&self.stumps).get_mut(&spirit).and_then(|s| (!s.is_empty()).then(|| s.remove(0))) else { return };
        let Some(entity) = host.entity(spirit) else { return };
        let (Some(living), Some(level)) = (host.living(entity), entity.level(&*host)) else { return };
        let Some(held) = living.hand_item(&*host, Hand::Main) else { return };
        if !item_in_tag(&*host, &held, "minecraft:saplings") || !is_air(&*host, level, stump) || !is_dirt(&*host, level, [stump[0], stump[1] - 1, stump[2]]) {
            return;
        }
        let sapling = id(&*host, &held);
        if game::set_block(host, level, stump, sapling) {
            let _ = living.set_hand_item(host, Hand::Main, held.shrink(1));
        }
    }

    /// The ripe crops within 2 of `crop` broken, their drops falling, and each planted again
    /// young (`HarvestCropBehaviour.tick`).
    fn harvest(&self, host: &mut dyn ServerHost, spirit: u32, crop: [i32; 3]) {
        let Some(level) = host.entity(spirit).and_then(|e| e.level(&*host)) else { return };
        for dx in -2..=2 {
            for dz in -2..=2 {
                let p = [crop[0] + dx, crop[1], crop[2] + dz];
                if !is_ripe(&*host, level, p) {
                    continue;
                }
                let Some(young) = block_at(&*host, level, p).and_then(|b| host.block_state_default(b)) else { continue };
                if level.destroy_block(host, p, true, None, false).is_ok() {
                    let _ = level.set_block(host, p, young, BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS);
                }
            }
        }
    }

    /// Sneak-clicking a spirit binds the book to it, if the spirit does the book's job.
    pub fn bind(&self, host: &mut dyn ServerHost, hand: Hand, entity: i32, stack: &ItemStack) -> Option<InteractionResult> {
        let player = host.players().acting_player()?.uuid();
        if !BOOKS.contains(&id(&*host, stack)) || !sneaking(&*host, player) {
            return None;
        }
        let spirit = host.entity(entity as u32)?;
        let job = self.work(spirit.id()).map(|w| w.job);
        if job.as_deref() != book_job(id(&*host, stack)) {
            tell(host, player, game::say("item.occultism.book_of_calling.message_target_cannot_link", &[]));
            return Some(InteractionResult::Fail);
        }
        let uuid = spirit.uuid(host)?;
        let name = spirit.nbt(host).and_then(|n| game::text(&n, "CustomName"));
        let mut book = stack.clone();
        if let Some(name) = &name {
            crate::rites::set_spirit_name(&mut book, name);
        }
        CallingPart::rewrite(host, player, hand, &book, |c| c.spirit = Some(uuid));
        tell(host, player, game::say("item.occultism.book_of_calling.message_target_linked", &[]));
        Some(InteractionResult::Success)
    }

    fn ask(&self, act: Act) {
        lock(&self.acts).push(act);
    }

    fn paused(&self, spirit: u32) -> bool {
        lock(&self.paused).contains_key(&spirit)
    }

    /// Whether a sound at `at` is a spirit's wolf's own.
    pub fn mutes(&self, sound: &str, at: [f64; 3]) -> bool {
        sound.starts_with("minecraft:entity.wolf")
            && lock(&self.at).values().any(|p| (0..3).map(|i| (p[i] - at[i]).powi(2)).sum::<f64>() < 1.0)
    }

    /// A ritual summoned `spirit` with `job` for `caster`: it takes up the job, and the caster gets
    /// the book of calling `book`, bound to it (`SummonRitual.finishBookOfCallingSetup`). A crusher
    /// has no book (`book` is empty): it works where it was summoned.
    pub fn called(&self, host: &mut dyn ServerHost, spirit: u32, job: &str, caster: Option<u128>, name: Option<&str>, book: &str, at: [f64; 3]) {
        let Some(entity) = host.entity(spirit) else { return };
        let kind = lock(&self.kinds).get(&spirit).copied().unwrap_or(FOLIOT);
        self.joined(host, spirit, kind);
        let home = processing(job).map(|_| at.map(|c| c.floor() as i32));
        self.edit(host, entity, |w| {
            w.job = job.to_string();
            if home.is_some() {
                w.base = home;
            }
        });
        self.show(host, entity, job);
        let Some(uuid) = entity.uuid(host) else { return };
        let Some(mut stack) = game::stack(&*host, book, 1) else { return };
        set_calling(&mut stack, &Calling { spirit: Some(uuid), ..Calling::default() });
        if let Some(name) = name {
            crate::rites::set_spirit_name(&mut stack, name);
        }
        match caster {
            Some(who) => lock(&self.gifts).push((who, stack, at)),
            None => drop_at(host, stack, at),
        }
    }

    /// `player`'s tick: the books of calling waiting for them, into their inventory or dropped
    /// by the bowl.
    pub fn hand_over(&self, host: &mut dyn ServerHost, player: u128) {
        let mine: Vec<_> = {
            let mut gifts = lock(&self.gifts);
            let (mine, rest) = std::mem::take(&mut *gifts).into_iter().partition(|(who, _, _)| *who == player);
            *gifts = rest;
            mine
        };
        for (_, stack, at) in mine {
            if let Some(left) = game::give(host, player, stack) {
                drop_at(host, left, at);
            }
        }
    }
}

/// `stack` dropped at `at`, as a ritual drops its result.
fn drop_at(host: &mut dyn ServerHost, stack: ItemStack, at: [f64; 3]) {
    if let Some(item) = stack.to_nbt(&*host) {
        let mut nbt = Compound::new();
        nbt.insert("Item".into(), Tag::Compound(item));
        nbt.insert("PickupDelay".into(), Tag::Short(40));
        host.summon(host.dimension(), "minecraft:item", at, nbt);
    }
}

/// Where a soul gem keeps a captured spirit's work, beside its entity data.
pub const CARRIED: &str = "occultism:work";

/// A spirit's work, for a soul gem capturing it.
pub fn carried(host: &dyn Host, spirit: EntityRef) -> Option<Compound> {
    encode(&WorkerPart::state_existing(host, spirit).ok()??).ok()
}

/// A spirit let out of a soul gem takes its work back up.
pub fn restore(host: &mut dyn ServerHost, spirit: EntityRef, carried: &Compound) {
    if let Ok(work) = decode(carried) {
        let _ = WorkerPart::edit_state(host, spirit, |_, w| *w = work);
    }
}

/// A book of calling's settings: the spirit it calls, its mode and the work area size it gives.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Calling {
    pub spirit: Option<u128>,
    pub mode: usize,
    pub size: usize,
    /// Where it last set its spirit to take from and to put into, for its tooltip
    /// (`ItemNBTUtil.updateItemNBTFromEntity`).
    pub extract: Option<[i32; 3]>,
    pub deposit: Option<[i32; 3]>,
}

pub fn calling(stack: &ItemStack) -> Calling {
    let Some(Tag::Compound(c)) = stack.get(&game::Nbt(CALLING_COMPONENT)) else { return Calling::default() };
    let spirit = match c.get(&"spirit".into()) {
        Some(Tag::IntArray(v)) if v.len() == 4 => Some(v.iter().fold(0u128, |acc, part| (acc << 32) | u128::from(*part as u32))),
        _ => None,
    };
    let small = |key: &str, n: usize| match c.get(&key.into()) {
        Some(Tag::Byte(b)) => (*b as usize).min(n - 1),
        _ => 0,
    };
    let pos = |key: &str| match c.get(&key.into()) {
        Some(Tag::IntArray(v)) if v.len() == 3 => Some([v[0], v[1], v[2]]),
        _ => None,
    };
    Calling { spirit, mode: small("mode", 2), size: small("size", AREA_SIZES.len()), extract: pos("extract"), deposit: pos("deposit") }
}

fn set_calling(stack: &mut ItemStack, calling: &Calling) {
    let mut c = Compound::new();
    if let Some(u) = calling.spirit {
        c.insert("spirit".into(), Tag::IntArray((0..4).rev().map(|i| (u >> (32 * i)) as u32 as i32).collect()));
    }
    c.insert("mode".into(), Tag::Byte(calling.mode as i8));
    c.insert("size".into(), Tag::Byte(calling.size as i8));
    for (key, place) in [("extract", calling.extract), ("deposit", calling.deposit)] {
        if let Some(p) = place {
            c.insert(key.into(), Tag::IntArray(p.to_vec()));
        }
    }
    let _ = stack.set(&game::Nbt(CALLING_COMPONENT), Tag::Compound(c));
}

/// The book's job, by its id.
fn book_job(book: &str) -> Option<&'static str> {
    match book {
        CLEANER_BOOK => Some(CLEANER),
        TRANSPORTER_BOOK => Some(TRANSPORTER),
        LUMBERJACK_BOOK => Some(LUMBERJACK),
        FARMER_BOOK => Some(FARMER),
        _ => None,
    }
}

/// The spirit a book calls, if it is in this level.
fn bound(host: &dyn ServerHost, stack: &ItemStack) -> Option<EntityRef> {
    host.entity_by_uuid(EntityUuid(calling(stack).spirit?))
}

/// The spirit's name, as the book says it (and the spirit carries it).
fn spirit_name(stack: &ItemStack) -> String {
    crate::rites::spirit_name(stack).unwrap_or_else(|| "Foliot".into())
}

fn translated(key: &str, args: Vec<Text>) -> Text {
    Text::translate(key, args)
}

/// A block's name, translated.
fn block_text(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Text {
    let name = game::block_name(host, level.block(host, pos)).replacen(':', ".", 1);
    translated(&format!("block.{name}"), Vec::new())
}

/// A working spirit: its Part state, and the goals of every job.
pub struct WorkerPart(pub Arc<crate::State>);

/// The names of this port's goals, as a mob's goal list gives them.
const OUR_GOALS: [&str; 7] = ["HarvestCrops", "PickupItems", "ReplantSapling", "DepositItems", "ExtractItems", "FellTree", "ReturnToWorkArea"];

impl EntityPartState for WorkerPart {
    type Data = Work;
    const DATA: &'static AttachmentType<Work> = &WORK;
}

#[mod_api::horizon_part]
impl EntityPart for WorkerPart {
    fn register_goals(&self, goals: &mut GoalRegistrar<'_>) {
        // Occultism's jobs add their goals when they start; here every foliot has them all, and
        // each goal works only for its jobs. The janitor's return and pickup are in Occultism's
        // target selector, beside the move goals; here they all drive the navigation, the pickup
        // first, then the transporter's deposit and extraction (`TransportItemsJob`'s 3 and 4).
        // The lumberjack's order is Occultism's: replant, deposit, pick up, fell.
        // The farmer's: harvest, then pick up, then deposit.
        goals.add_goal(0, HarvestCrops { s: self.0.clone(), crop: None, next_scan: 0 });
        goals.add_goal(1, PickupItems { s: self.0.clone(), target: None, unreachable: Vec::new() });
        goals.add_goal(2, ReplantSapling { s: self.0.clone() });
        goals.add_goal(3, DepositItems { s: self.0.clone() });
        goals.add_goal(4, ExtractItems { s: self.0.clone() });
        goals.add_goal(5, FellTree { s: self.0.clone(), tree: None, chopped: 0, next_scan: 0 });
        goals.add_goal(6, ReturnToWorkArea { s: self.0.clone() });
    }
}

/// The world the goal's mob is in.
fn level_of(cx: &GoalContext<'_>) -> Option<LevelRef> {
    let host = cx.host();
    cx.mob().entity().level(host)
}

fn position(cx: &GoalContext<'_>) -> Option<[f64; 3]> {
    cx.mob().entity().position(cx.host())
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

fn centre(p: [i32; 3]) -> [f64; 3] {
    [f64::from(p[0]) + 0.5, f64::from(p[1]) + 0.5, f64::from(p[2]) + 0.5]
}

/// What the goal's mob holds in its main hand.
fn held(cx: &GoalContext<'_>) -> Option<ItemStack> {
    let host = cx.host();
    host.living(cx.mob().entity())?.hand_item(host, Hand::Main)
}

/// Whether a goal looks this tick: one try in ten, and every tenth tick (`executionChance`).
fn tries(cx: &mut GoalContext<'_>, s: &crate::State) -> bool {
    s.workers.ticks.load(Ordering::Relaxed) % 10 == 0 || crate::platform::below(cx.random().next_int_bound(10)) == 0
}

/// `PickupItemsGoal`: to the nearest item in the work area that fits in the spirit's hand.
struct PickupItems {
    s: Arc<crate::State>,
    target: Option<EntityRef>,
    /// Items it couldn't reach, and the tick it tries them again.
    unreachable: Vec<(u32, u64)>,
}

impl PickupItems {
    /// The nearest item lying in the work area that the hand takes.
    fn find(&self, cx: &GoalContext<'_>) -> Option<EntityRef> {
        let id = cx.mob().entity().id();
        let work = self.s.workers.work(id)?;
        // The janitor takes anything; the lumberjack and the farmer what their jobs yield; a
        // crusher, crystallizer or smelter what a recipe of its job takes at its rank, but not what
        // it just dropped.
        let crusher = processing(&work.job);
        let wanted_tags: &[&str] = match work.job.as_str() {
            CLEANER => &[],
            LUMBERJACK => &LUMBER,
            FARMER => &HARVEST,
            _ if crusher.is_some() => &[],
            _ => return None,
        };
        let lumberjack = work.job == LUMBERJACK;
        let host = cx.host();
        let recipes = crusher.map(|(job, _)| self.s.crushing.recipes(host, job));
        let here = position(cx)?;
        let centre_at = work.base.map(centre).unwrap_or(here);
        let size = f64::from(work.size);
        let min = [centre_at[0] - size, centre_at[1] - size / 2.0, centre_at[2] - size];
        let max = [centre_at[0] + size, centre_at[1] + size / 2.0, centre_at[2] + size];
        let held = held(cx);
        let fits = |stack: &ItemStack| match &held {
            None => true,
            Some(h) => {
                ItemResource::of(h) == ItemResource::of(stack)
                    && game::item(host, game::id(host, h)).and_then(|k| host.max_stack_size(k)).is_some_and(|m| h.count() < m)
            }
        };
        level_of(cx)?
            .entities_in(host, min, max)?
            .into_iter()
            .filter_map(|e| {
                let item = host.item_entity(e)?;
                let stack = item.stack(host)?;
                let wanted = match (crusher, &recipes) {
                    (Some((_, tier)), Some(recipes)) => {
                        logic::crushable(recipes, game::id(host, &stack), tier) && !self.s.workers.dropped_lately(e.id())
                    }
                    _ => {
                        wanted_tags.is_empty()
                            || (lumberjack && game::id(host, &stack) == "minecraft:stick")
                            || wanted_tags.iter().any(|t| item_in_tag(host, &stack, t))
                    }
                };
                let skipped = self.unreachable.iter().any(|(i, _)| *i == e.id());
                (item.pickup_delay(host).unwrap_or(0) <= 0 && wanted && !skipped && fits(&stack)).then_some((e, e.position(host)?))
            })
            .min_by(|a, b| distance(a.1, here).total_cmp(&distance(b.1, here)))
            .map(|(e, _)| e)
    }
}

impl Goal for PickupItems {
    fn flags(&self) -> GoalFlags {
        GoalFlags::MOVE
    }
    fn name(&self) -> &'static str {
        "PickupItems"
    }
    fn can_use(&mut self, cx: &mut GoalContext<'_>) -> bool {
        if !tries(cx, &self.s) {
            return false;
        }
        let now = self.s.workers.ticks.load(Ordering::Relaxed);
        self.unreachable.retain(|(_, until)| *until > now);
        self.target = self.find(cx);
        self.target.is_some()
    }
    /// While it walks (`canContinueToUse`: the navigation isn't done). An item its path ended
    /// short of is out of reach: it leaves it be for a while.
    fn can_continue(&mut self, cx: &mut GoalContext<'_>) -> bool {
        let Some(target) = self.target else { return false };
        if cx.host().item_entity(target).and_then(|i| i.stack(cx.host())).is_none() {
            return false;
        }
        if cx.mob().is_done(cx.host()) == Some(true) {
            // Its path ends a little short, as its wolf's does: near enough still takes it.
            let near = position(cx).zip(target.position(cx.host())).is_some_and(|(a, b)| distance(a, b) < STOPPED_REACH);
            if near {
                self.s.workers.ask(Act::Pickup { spirit: cx.mob().entity().id(), item: target.id() });
            } else {
                let now = self.s.workers.ticks.load(Ordering::Relaxed);
                self.unreachable.push((target.id(), now + NO_TREE_WAIT));
            }
            return false;
        }
        true
    }
    fn start(&mut self, cx: &mut GoalContext<'_>) {
        if let Some(at) = self.target.and_then(|t| t.position(cx.host())) {
            cx.move_to(at, 1.0);
        }
    }
    fn stop(&mut self, cx: &mut GoalContext<'_>) {
        self.target = None;
        cx.stop_navigation();
    }
    fn tick(&mut self, cx: &mut GoalContext<'_>) {
        let (Some(target), Some(here)) = (self.target, position(cx)) else { return };
        let Some(at) = target.position(cx.host()) else {
            self.target = None;
            return;
        };
        if distance(here, at) < PICKUP_RANGE {
            cx.stop_navigation();
            self.s.workers.ask(Act::Pickup { spirit: cx.mob().entity().id(), item: target.id() });
            self.target = None;
        } else {
            cx.move_to(at, 1.0);
        }
    }
}

/// `DepositItemsGoal`: what the spirit holds, to its container.
struct DepositItems {
    s: Arc<crate::State>,
}

impl DepositItems {
    fn container(&self, cx: &GoalContext<'_>) -> Option<[i32; 3]> {
        let id = cx.mob().entity().id();
        if self.s.workers.paused(id) || held(cx).is_none() {
            return None;
        }
        Some(self.s.workers.work(id)?.deposit?.0)
    }
}

impl Goal for DepositItems {
    fn flags(&self) -> GoalFlags {
        GoalFlags::MOVE
    }
    fn name(&self) -> &'static str {
        "DepositItems"
    }
    fn can_use(&mut self, cx: &mut GoalContext<'_>) -> bool {
        self.container(cx).is_some()
    }
    fn stop(&mut self, cx: &mut GoalContext<'_>) {
        cx.stop_navigation();
    }
    fn tick(&mut self, cx: &mut GoalContext<'_>) {
        let (Some(pos), Some(here)) = (self.container(cx), position(cx)) else { return };
        let at = centre(pos);
        let near = distance(here, at);
        let stopped = cx.mob().is_done(cx.host()) == Some(true);
        if near < ACCESS_DISTANCE || (stopped && near < STOPPED_REACH) {
            cx.stop_navigation();
            self.s.workers.ask(Act::Deposit { spirit: cx.mob().entity().id() });
        } else {
            cx.move_to(beside(here, pos), 1.0);
        }
    }
}

/// Beside the container at `pos`, on the spirit's side of it (`getMoveTarget`).
fn beside(here: [f64; 3], pos: [i32; 3]) -> [f64; 3] {
    let at = centre(pos);
    let (dx, dz) = (here[0] - at[0], here[2] - at[2]);
    let side = if dx.abs() > dz.abs() { [dx.signum(), 0.0] } else { [0.0, dz.signum()] };
    [at[0] + side[0], f64::from(pos[1]), at[2] + side[1]]
}

/// `ExtractItemsGoal`: a transporter with an empty hand, to the container it takes from.
struct ExtractItems {
    s: Arc<crate::State>,
}

impl ExtractItems {
    fn container(&self, cx: &GoalContext<'_>) -> Option<[i32; 3]> {
        let id = cx.mob().entity().id();
        if self.s.workers.paused(id) || held(cx).is_some() {
            return None;
        }
        let work = self.s.workers.work(id)?;
        if work.job != TRANSPORTER {
            return None;
        }
        Some(work.extract?.0)
    }
}

impl Goal for ExtractItems {
    fn flags(&self) -> GoalFlags {
        GoalFlags::MOVE
    }
    fn name(&self) -> &'static str {
        "ExtractItems"
    }
    fn can_use(&mut self, cx: &mut GoalContext<'_>) -> bool {
        self.container(cx).is_some()
    }
    fn stop(&mut self, cx: &mut GoalContext<'_>) {
        cx.stop_navigation();
    }
    fn tick(&mut self, cx: &mut GoalContext<'_>) {
        let (Some(pos), Some(here)) = (self.container(cx), position(cx)) else { return };
        let near = distance(here, centre(pos));
        let stopped = cx.mob().is_done(cx.host()) == Some(true);
        if near < EXTRACT_DISTANCE || (stopped && near < STOPPED_REACH) {
            cx.stop_navigation();
            self.s.workers.ask(Act::Extract { spirit: cx.mob().entity().id() });
        } else {
            cx.move_to(beside(here, pos), 1.0);
        }
    }
}

fn block_at(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Option<BlockKey> {
    level.block(host, pos)
}

fn block_in(host: &dyn Host, level: LevelRef, pos: [i32; 3], tag: &str) -> bool {
    block_at(host, level, pos).and_then(|b| host.block_in_tag(b, tag)) == Some(true)
}

fn is_log(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> bool {
    block_in(host, level, pos, "minecraft:logs")
}

fn is_leaf(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> bool {
    block_in(host, level, pos, "minecraft:leaves")
}

fn is_dirt(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> bool {
    block_in(host, level, pos, "minecraft:dirt")
}

fn is_air(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> bool {
    crate::ritual::AIR.contains(&game::block_name(host, block_at(host, level, pos)))
}

fn within_fell(stump: [i32; 3], p: [i32; 3]) -> bool {
    let (dx, dz) = (p[0] - stump[0], p[2] - stump[2]);
    dx * dx + dz * dz <= MAX_FELL_DISTANCE * MAX_FELL_DISTANCE
}

/// Whether `stack` is in the item tag `tag`.
fn item_in_tag(host: &dyn Host, stack: &ItemStack, tag: &str) -> bool {
    let tags = host.tags();
    let (Some(key), Some(item)) = (Identifier::parse(tag).ok().and_then(|t| tags.key(&t)), game::item(host, id(host, stack))) else { return false };
    tags.is_in_tag(item, key) == Some(true)
}

/// Whether the log at `stump` carries a tree: up its trunk, sideways where it bends, to leaves
/// (`NearestTreeSensor.isTree`).
fn is_tree(host: &dyn Host, level: LevelRef, stump: [i32; 3]) -> bool {
    let mut top = stump;
    for _ in 0..64 {
        let above = [top[0], top[1] + 1, top[2]];
        if is_air(host, level, above) {
            break;
        }
        top = above;
        if is_air(host, level, [top[0], top[1] + 1, top[2]]) {
            for [dx, dz] in [[1, 0], [-1, 0], [0, 1], [0, -1]] {
                if !is_air(host, level, [top[0] + dx, top[1] + 1, top[2] + dz]) {
                    top = [top[0] + dx, top[1], top[2] + dz];
                    break;
                }
            }
        }
    }
    is_leaf(host, level, top)
}

/// Where to stand to fell or replant at `pos`: an open block beside it.
fn standing(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> Option<[f64; 3]> {
    [[1, 0], [-1, 0], [0, 1], [0, -1]]
        .into_iter()
        .map(|[dx, dz]| [pos[0] + dx, pos[1], pos[2] + dz])
        .find(|p| is_air(host, level, *p))
        .map(|p| [f64::from(p[0]) + 0.5, f64::from(p[1]), f64::from(p[2]) + 0.5])
}

/// Whether the block at `pos` is a crop at its last age (`CropBlock.isMaxAge`): vanilla's crops
/// (not the melon and pumpkin stems, which aren't `CropBlock`s) and the datura.
fn is_ripe(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> bool {
    let Some(block) = block_at(host, level, pos) else { return false };
    let name = game::block_name(host, Some(block));
    let crop = name == DATURA_CROP || (host.block_in_tag(block, "minecraft:crops") == Some(true) && !name.ends_with("_stem"));
    if !crop {
        return false;
    }
    let Some(max) = host.block_states().definition(block).and_then(|d| d.into_iter().find(|p| p.name == "age")).and_then(|p| p.values.last().cloned()) else { return false };
    let state = level.block_state(host, pos).and_then(|s| host.block_state_string(s)).unwrap_or_default();
    state.contains(&format!("age={max}]")) || state.contains(&format!("age={max},"))
}

/// `HarvestCropBehaviour` and `NearestCropSensor`: to the nearest ripe crop on farmland in the
/// work area, and its harvest.
struct HarvestCrops {
    s: Arc<crate::State>,
    crop: Option<[i32; 3]>,
    next_scan: u64,
}

impl HarvestCrops {
    fn find(&self, cx: &GoalContext<'_>, work: &Work) -> Option<[i32; 3]> {
        let host = cx.host();
        let level = level_of(cx)?;
        let here = position(cx)?;
        let centre_at = work.base.unwrap_or([here[0].floor() as i32, here[1].floor() as i32, here[2].floor() as i32]);
        let half = work.size / 2;
        let mut crops = Vec::new();
        for y in -half..=half {
            for x in -half..=half {
                for z in -half..=half {
                    let p = [centre_at[0] + x, centre_at[1] + y, centre_at[2] + z];
                    if is_ripe(host, level, p) && game::block_name(host, block_at(host, level, [p[0], p[1] - 1, p[2]])) == "minecraft:farmland" {
                        crops.push(p);
                    }
                }
            }
        }
        crops.into_iter().min_by(|a, b| distance(centre(*a), here).total_cmp(&distance(centre(*b), here)))
    }
}

impl Goal for HarvestCrops {
    fn flags(&self) -> GoalFlags {
        GoalFlags::MOVE.union(GoalFlags::LOOK)
    }
    fn name(&self) -> &'static str {
        "HarvestCrops"
    }
    fn can_use(&mut self, cx: &mut GoalContext<'_>) -> bool {
        let Some(work) = self.s.workers.work(cx.mob().entity().id()) else { return false };
        if work.job != FARMER {
            return false;
        }
        let now = self.s.workers.ticks.load(Ordering::Relaxed);
        if now < self.next_scan {
            return false;
        }
        self.crop = self.find(cx, &work);
        self.next_scan = now + if self.crop.is_some() { 0 } else { NO_CROP_WAIT };
        self.crop.is_some()
    }
    fn can_continue(&mut self, cx: &mut GoalContext<'_>) -> bool {
        let (Some(crop), Some(level)) = (self.crop, level_of(cx)) else { return false };
        if !is_ripe(cx.host(), level, crop) {
            return false;
        }
        // A crop its path can't reach waits for the next look (`HandleUnreachableCropBehaviour`).
        if cx.mob().is_done(cx.host()) == Some(true) && position(cx).is_some_and(|h| distance(h, centre(crop)) > STOPPED_REACH) {
            self.next_scan = self.s.workers.ticks.load(Ordering::Relaxed) + CROP_SCAN;
            return false;
        }
        true
    }
    fn start(&mut self, cx: &mut GoalContext<'_>) {
        if let Some(crop) = self.crop {
            cx.move_to([f64::from(crop[0]) + 0.5, f64::from(crop[1]), f64::from(crop[2]) + 0.5], 1.0);
        }
    }
    fn stop(&mut self, cx: &mut GoalContext<'_>) {
        self.crop = None;
        cx.stop_navigation();
    }
    fn tick(&mut self, cx: &mut GoalContext<'_>) {
        let (Some(crop), Some(here)) = (self.crop, position(cx)) else { return };
        let near = distance(here, centre(crop));
        if near <= HARVEST_RANGE || (cx.mob().is_done(cx.host()) == Some(true) && near < STOPPED_REACH) {
            cx.stop_navigation();
            cx.look_at(centre(crop));
            self.s.workers.ask(Act::Harvest { spirit: cx.mob().entity().id(), crop });
            self.crop = None;
        } else {
            cx.move_to([f64::from(crop[0]) + 0.5, f64::from(crop[1]), f64::from(crop[2]) + 0.5], 1.0);
        }
    }
}

/// `FellTreeBehaviour` and its sensor: to the nearest tree in the work area, chopped until it falls.
struct FellTree {
    s: Arc<crate::State>,
    tree: Option<[i32; 3]>,
    chopped: u32,
    /// The tick it looks for a tree again.
    next_scan: u64,
}

impl FellTree {
    /// The nearest stump in the work area: a log on dirt carrying leaves, open beside.
    fn find(&self, cx: &GoalContext<'_>, work: &Work) -> Option<[i32; 3]> {
        let host = cx.host();
        let level = level_of(cx)?;
        let here = position(cx)?;
        let centre_at = work.base.unwrap_or([here[0].floor() as i32, here[1].floor() as i32, here[2].floor() as i32]);
        let half = work.size / 2;
        let mut stumps = Vec::new();
        for y in -half..=half {
            for x in -half..=half {
                for z in -half..=half {
                    let p = [centre_at[0] + x, centre_at[1] + y, centre_at[2] + z];
                    if is_log(host, level, p) && is_dirt(host, level, [p[0], p[1] - 1, p[2]]) {
                        stumps.push(p);
                    }
                }
            }
        }
        stumps.sort_by(|a, b| distance(centre(*a), here).total_cmp(&distance(centre(*b), here)));
        stumps.into_iter().find(|p| is_tree(host, level, *p) && standing(host, level, *p).is_some())
    }
}

impl Goal for FellTree {
    fn flags(&self) -> GoalFlags {
        GoalFlags::MOVE.union(GoalFlags::LOOK)
    }
    fn name(&self) -> &'static str {
        "FellTree"
    }
    fn requires_update_every_tick(&self) -> bool {
        true
    }
    fn can_use(&mut self, cx: &mut GoalContext<'_>) -> bool {
        let id = cx.mob().entity().id();
        let Some(work) = self.s.workers.work(id) else { return false };
        if work.job != LUMBERJACK || held(cx).is_some() {
            return false;
        }
        let now = self.s.workers.ticks.load(Ordering::Relaxed);
        if now < self.next_scan {
            return false;
        }
        self.tree = self.find(cx, &work);
        self.next_scan = now + if self.tree.is_some() { TREE_SCAN } else { NO_TREE_WAIT };
        self.chopped = 0;
        self.tree.is_some()
    }
    fn can_continue(&mut self, cx: &mut GoalContext<'_>) -> bool {
        let (Some(tree), Some(level)) = (self.tree, level_of(cx)) else { return false };
        held(cx).is_none() && is_log(cx.host(), level, tree)
    }
    fn stop(&mut self, cx: &mut GoalContext<'_>) {
        self.tree = None;
        cx.stop_navigation();
    }
    fn tick(&mut self, cx: &mut GoalContext<'_>) {
        let (Some(tree), Some(here), Some(level)) = (self.tree, position(cx), level_of(cx)) else { return };
        if distance(here, centre(tree)) > FELL_RANGE {
            if let Some(at) = standing(cx.host(), level, tree) {
                cx.move_to(at, 1.0);
            }
            return;
        }
        cx.stop_navigation();
        cx.look_at(centre(tree));
        self.chopped += 1;
        if self.chopped >= FELL_TICKS {
            self.s.workers.ask(Act::Fell { spirit: cx.mob().entity().id(), stump: tree });
            self.tree = None;
            // Next tree once this one's wood is gathered.
            self.next_scan = self.s.workers.ticks.load(Ordering::Relaxed) + TREE_SCAN;
        }
    }
}

/// `ReplantSaplingBehaviour`: a lumberjack holding a sapling, to where it felled.
struct ReplantSapling {
    s: Arc<crate::State>,
}

impl ReplantSapling {
    fn stump(&self, cx: &GoalContext<'_>) -> Option<[i32; 3]> {
        let id = cx.mob().entity().id();
        if self.s.workers.work(id)?.job != LUMBERJACK || !held(cx).is_some_and(|h| item_in_tag(cx.host(), &h, "minecraft:saplings")) {
            return None;
        }
        lock(&self.s.workers.stumps).get(&id)?.first().copied()
    }
}

impl Goal for ReplantSapling {
    fn flags(&self) -> GoalFlags {
        GoalFlags::MOVE
    }
    fn name(&self) -> &'static str {
        "ReplantSapling"
    }
    fn can_use(&mut self, cx: &mut GoalContext<'_>) -> bool {
        self.stump(cx).is_some()
    }
    fn stop(&mut self, cx: &mut GoalContext<'_>) {
        cx.stop_navigation();
    }
    fn tick(&mut self, cx: &mut GoalContext<'_>) {
        let (Some(stump), Some(here), Some(level)) = (self.stump(cx), position(cx), level_of(cx)) else { return };
        if distance(here, centre(stump)) <= FELL_RANGE {
            cx.stop_navigation();
            self.s.workers.ask(Act::Replant { spirit: cx.mob().entity().id() });
        } else if let Some(at) = standing(cx.host(), level, stump).or(Some(centre(stump))) {
            cx.move_to(at, 1.0);
        }
    }
}

/// `ReturnToWorkAreaGoal`: back to the base when there's nothing else to do.
struct ReturnToWorkArea {
    s: Arc<crate::State>,
}

impl Goal for ReturnToWorkArea {
    fn flags(&self) -> GoalFlags {
        GoalFlags::MOVE
    }
    fn name(&self) -> &'static str {
        "ReturnToWorkArea"
    }
    fn can_use(&mut self, cx: &mut GoalContext<'_>) -> bool {
        if !tries(cx, &self.s) {
            return false;
        }
        let Some(base) = self.s.workers.work(cx.mob().entity().id()).and_then(|w| w.base) else { return false };
        position(cx).is_some_and(|here| distance(here, centre(base)) > 1.5)
    }
    fn can_continue(&mut self, cx: &mut GoalContext<'_>) -> bool {
        let host = cx.host();
        cx.mob().is_done(host) == Some(false)
    }
    fn start(&mut self, cx: &mut GoalContext<'_>) {
        if let Some(base) = self.s.workers.work(cx.mob().entity().id()).and_then(|w| w.base) {
            cx.move_to([f64::from(base[0]) + 0.5, f64::from(base[1]), f64::from(base[2]) + 0.5], 1.0);
        }
    }
}

/// A book of calling: it sets its spirit's base and deposit, and its work area size.
pub struct CallingPart(pub Arc<crate::State>);

fn sneaking(host: &dyn Host, player: u128) -> bool {
    host.player(player).and_then(|p| p.entity().is_shift_key_down(host)) == Some(true)
}

impl CallingPart {
    /// The book in `player`'s `hand` changed by `edit`.
    fn rewrite(host: &mut dyn ServerHost, player: u128, hand: Hand, stack: &ItemStack, edit: impl FnOnce(&mut Calling)) -> Calling {
        let mut settings = calling(stack);
        edit(&mut settings);
        let mut book = stack.clone();
        set_calling(&mut book, &settings);
        if let Some(living) = game::living(&*host, player) {
            let _ = living.set_hand_item(host, hand, Some(book));
        }
        settings
    }
}

#[mod_api::horizon_part]
impl ItemPart for CallingPart {
    /// Using the book in the air turns to its next mode; sneaking, to its next work area size,
    /// which its spirit takes at once. (Occultism opens a screen for both.)
    fn use_item(&self, host: HostSide<'_>, event: &mut UseItemEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else { return Some(InteractionResult::Success) };
        let (player, hand) = (event.player, event.hand);
        if sneaking(&*host, player) {
            let settings = Self::rewrite(host, player, hand, event.stack, |c| c.size = (c.size + 1) % AREA_SIZES.len());
            let (size, size_name) = AREA_SIZES[settings.size];
            let area = translated(&format!("enum.occultism.work_area_size.{size_name}"), Vec::new());
            if let Some(spirit) = bound(&*host, event.stack) {
                self.0.workers.edit(host, spirit, |w| w.size = size);
                let line = translated("item.occultism.book_of_calling.message_set_work_area_size", vec![Text::literal(spirit_name(event.stack)), area]);
                tell(host, player, line);
            } else {
                tell(host, player, translated("gui.occultism.book_of_calling.work_area_set", vec![area]));
            }
        } else {
            let modes = modes(id(&*host, event.stack));
            let settings = Self::rewrite(host, player, hand, event.stack, |c| c.mode = (c.mode + 1) % modes.len());
            let mode = translated(&format!("enum.occultism.book_of_calling.item_mode.{}", modes[settings.mode]), Vec::new());
            tell(host, player, translated("gui.occultism.book_of_calling.mode_set", vec![mode]));
        }
        Some(InteractionResult::Success)
    }

    /// Sneak-using the book on a block sets its spirit's base there, or its deposit, by its mode.
    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let player = event.player;
        let HostSide::Server(host) = host else {
            let HostSide::Client(host) = host else { return None };
            return sneaking(&*host, player).then_some(InteractionResult::Success);
        };
        if !sneaking(&*host, player) {
            return None;
        }
        let level = host.level(host.dimension())?;
        let Some(spirit) = bound(&*host, event.stack) else {
            tell(host, player, game::say("item.occultism.book_of_calling.message_spirit_not_found", &[]));
            return Some(InteractionResult::Fail);
        };
        let name = Text::literal(spirit_name(event.stack));
        let (pos, face) = (event.pos, face_number(event.face));
        let mode = modes(id(&*host, event.stack))[calling(event.stack).mode];
        if mode == "set_base" {
            self.0.workers.edit(host, spirit, |w| w.base = Some(pos));
            let line = translated("item.occultism.book_of_calling.message_set_base", vec![name, block_text(&*host, level, pos)]);
            tell(host, player, line);
            return Some(InteractionResult::Success);
        }
        // Extract and deposit: a container only (`DepositItemMode`, `ExtractItemMode`).
        if level.capability(host, &ITEMS, pos, Some(direction(face))).is_none() {
            return Some(InteractionResult::Success);
        }
        let extract = mode == "set_extract";
        self.0.workers.edit(host, spirit, |w| {
            if extract { w.extract = Some((pos, face)) } else { w.deposit = Some((pos, face)) }
        });
        Self::rewrite(host, player, event.hand, event.stack, |c| {
            if extract { c.extract = Some(pos) } else { c.deposit = Some(pos) }
        });
        let side = Text::literal(FACE_NAMES[usize::from(face)]);
        let key = if extract { "message_set_extract" } else { "message_set_deposit" };
        let line = translated(&format!("item.occultism.book_of_calling.{key}"), vec![name, block_text(&*host, level, pos), side]);
        tell(host, player, line);
        Some(InteractionResult::Success)
    }

    /// Whom the book calls, and its mode and work area size.
    fn tooltip(&self, host: &dyn ClientHost, event: &mut TooltipEvent<'_>) {
        let settings = calling(event.stack);
        let say = |key: &str| host.translate(key).unwrap_or_else(|| key.to_string());
        if settings.spirit.is_some() {
            let line = say("item.occultism.book_of_calling_foliot.tooltip").replacen("%s", &spirit_name(event.stack), 1);
            event.lines.push((Text::literal(&line), 0xFFAAAAAA));
        }
        let at = |p: [i32; 3]| format!("{}, {}, {}", p[0], p[1], p[2]);
        for (key, place) in [("extract", settings.extract), ("deposit", settings.deposit)] {
            if let Some(p) = place {
                let line = say(&format!("item.occultism.book_of_calling_foliot.tooltip.{key}")).replacen("%s", &at(p), 1);
                event.lines.push((Text::literal(&line), 0xFFAAAAAA));
            }
        }
        let mode = say(&format!("enum.occultism.book_of_calling.item_mode.{}", modes(id(host, event.stack))[settings.mode]));
        let area = say(&format!("enum.occultism.work_area_size.{}", AREA_SIZES[settings.size].1));
        event.lines.push((Text::literal(&format!("{}: {mode}", say("gui.occultism.book_of_calling.mode"))), 0xFFAAAAAA));
        event.lines.push((Text::literal(&format!("{}: {area}", say("gui.occultism.book_of_calling.work_area"))), 0xFFAAAAAA));
    }
}
