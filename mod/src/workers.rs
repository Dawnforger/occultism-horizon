// Occultism's working spirits, starting with the foliot janitor (`CleanerJob`): a foliot summoned
// with a job, bound to a book of calling. The janitor picks up the items lying in its work area
// (`PickupItemsGoal`), carries them to the container the book set (`DepositItemsGoal`) and goes
// back to its base (`ReturnToWorkAreaGoal`).
//
// The foliot is a mod mob copying a wolf (a creature, so it stays on Peaceful), stripped of the
// wolf's own goals and sounds and drawn as Occultism's model. Its job, base, work area and deposit
// are its Part state, saved with it; its one inventory slot is its main hand, as Occultism's is.
// Goals only read the world: what they do to it (an item picked up, a stack put in a chest) waits
// for the end of the server tick.

use crate::game::{self, id, lock, tell};
use mod_api::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

pub const FOLIOT: &str = "occultism:foliot";
pub const CLEANER: &str = "occultism:cleaner";
pub const CLEANER_BOOK: &str = "occultism:book_of_calling_foliot_cleaner";
pub const BOOKS: [&str; 1] = [CLEANER_BOOK];
/// A book of calling's bound spirit, mode and work area size.
pub const CALLING_COMPONENT: &str = "occultism:calling";

/// Occultism's `WorkAreaSize`: the half-width of the box a spirit works in, and its name.
pub const AREA_SIZES: [(i32, &str); 3] = [(16, "small"), (32, "medium"), (64, "large")];
/// The book of calling modes a janitor's book has (`BookOfCallingCleanerItem`), as Occultism names them.
pub const MODES: [&str; 2] = ["set_base", "set_deposit"];
/// How near an item a spirit picks it up (`pickupRange`).
const PICKUP_RANGE: f64 = 2.0;
/// How near the container a spirit puts its load in (`accessDistance`).
const ACCESS_DISTANCE: f64 = 2.2;
/// A foliot's health (`FoliotEntity.createAttributes`).
const FOLIOT_HEALTH: f64 = 20.0;
/// How near the container a spirit that has stopped walking still reaches it: its wolf's path
/// ends a little short of where Occultism's would.
const STOPPED_REACH: f64 = 3.0;
/// How long a spirit waits when its container is full (`pause(2000)`), in ticks.
const FULL_PAUSE: u64 = 40;
/// The goals a foliot keeps of its wolf's: floating, and looking about.
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
    Ok(c)
}

fn decode(c: &Compound) -> Result<Work, AttachmentError> {
    let pos = |key: &str| match c.get(&key.into()) {
        Some(Tag::IntArray(v)) if v.len() == 3 => Some([v[0], v[1], v[2]]),
        _ => None,
    };
    let face = match c.get(&"face".into()) {
        Some(Tag::Byte(f)) => (*f as u8).min(5),
        _ => 1,
    };
    Ok(Work {
        job: game::text(c, "job").unwrap_or_default(),
        base: pos("base"),
        size: match c.get(&"size".into()) {
            Some(Tag::Int(s)) => *s,
            _ => AREA_SIZES[0].0,
        },
        deposit: pos("deposit").map(|d| (d, face)),
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
}

impl Workers {
    fn work(&self, spirit: u32) -> Option<Work> {
        lock(&self.work).get(&spirit).cloned()
    }

    /// `spirit`'s work changed by `edit`: in its Part state, and for its goals.
    fn edit(&self, host: &mut dyn ServerHost, spirit: EntityRef, edit: impl FnOnce(&mut Work)) -> Option<Work> {
        let work = FoliotPart::edit_state(host, spirit, |_, w| {
            edit(w);
            w.clone()
        })
        .ok()?;
        lock(&self.work).insert(spirit.id(), work.clone());
        Some(work)
    }

    /// A foliot came into the world: its work for its goals, and none of its wolf's ways.
    pub fn joined(&self, host: &mut dyn ServerHost, entity: u32) {
        let Some(spirit) = host.entity(entity) else { return };
        let work = FoliotPart::state(&*host, spirit).unwrap_or_else(|_| (WORK.default)());
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

    /// What a foliot that just came can't be given while it joins: its health.
    fn settle(&self, host: &mut dyn ServerHost) {
        for entity in std::mem::take(&mut *lock(&self.fresh)) {
            let Some(spirit) = host.entity(entity) else { continue };
            // Its work as it is now: a soul gem gives a released spirit its work after it joins.
            if let Ok(work) = FoliotPart::state(&*host, spirit) {
                lock(&self.work).insert(entity, work);
            }
            // A wolf that isn't tamed has 8 health, set again each time it loads; a foliot has 20
            // (`FoliotEntity.createAttributes`). Its health is lost to the wolf's cap on loading,
            // so a foliot loads whole.
            if let Some(living) = host.living(spirit)
                && living.attribute_base(&*host, "minecraft:max_health") != Some(FOLIOT_HEALTH)
            {
                living.set_attribute_base(host, "minecraft:max_health", FOLIOT_HEALTH);
                living.set_health(host, FOLIOT_HEALTH as f32);
            }
        }
    }

    /// The end of a server tick: what the goals asked, done; where the spirits are now.
    pub fn tick(&self, host: &mut dyn ServerHost) {
        let now = self.ticks.fetch_add(1, Ordering::Relaxed);
        self.settle(host);
        let acts = std::mem::take(&mut *lock(&self.acts));
        for act in acts {
            match act {
                Act::Pickup { spirit, item } => self.pickup(host, spirit, item),
                Act::Deposit { spirit } => self.deposit(host, spirit, now),
            }
        }
        lock(&self.paused).retain(|_, until| *until > now);
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
                }
            }
        }
        *lock(&self.at) = at;
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

    /// Whether a sound at `at` is a foliot's wolf's own.
    pub fn mutes(&self, sound: &str, at: [f64; 3]) -> bool {
        sound.starts_with("minecraft:entity.wolf")
            && lock(&self.at).values().any(|p| (0..3).map(|i| (p[i] - at[i]).powi(2)).sum::<f64>() < 1.0)
    }

    /// A ritual summoned `spirit` with `job` for `caster`: it takes up the job, and the caster gets
    /// the book of calling `book`, bound to it (`SummonRitual.finishBookOfCallingSetup`).
    pub fn called(&self, host: &mut dyn ServerHost, spirit: u32, job: &str, caster: Option<u128>, name: Option<&str>, book: &str, at: [f64; 3]) {
        let Some(entity) = host.entity(spirit) else { return };
        self.joined(host, spirit);
        self.edit(host, entity, |w| w.job = job.to_string());
        let Some(uuid) = entity.uuid(host) else { return };
        let Some(mut stack) = game::stack(&*host, book, 1) else { return };
        set_calling(&mut stack, &Calling { spirit: Some(uuid), ..Calling::default() });
        if let Some(name) = name {
            crate::rites::set_spirit_name(&mut stack, name);
        }
        let left = match caster {
            Some(who) => game::give(host, who, stack),
            None => Some(stack),
        };
        if let Some(left) = left.and_then(|s| s.to_nbt(&*host)) {
            let mut nbt = Compound::new();
            nbt.insert("Item".into(), Tag::Compound(left));
            nbt.insert("PickupDelay".into(), Tag::Short(40));
            host.summon(host.dimension(), "minecraft:item", at, nbt);
        }
    }
}

/// Where a soul gem keeps a captured spirit's work, beside its entity data.
pub const CARRIED: &str = "occultism:work";

/// A spirit's work, for a soul gem capturing it.
pub fn carried(host: &dyn Host, spirit: EntityRef) -> Option<Compound> {
    encode(&FoliotPart::state_existing(host, spirit).ok()??).ok()
}

/// A spirit let out of a soul gem takes its work back up.
pub fn restore(host: &mut dyn ServerHost, spirit: EntityRef, carried: &Compound) {
    if let Ok(work) = decode(carried) {
        let _ = FoliotPart::edit_state(host, spirit, |_, w| *w = work);
    }
}

/// A book of calling's settings: the spirit it calls, its mode and the work area size it gives.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Calling {
    pub spirit: Option<u128>,
    pub mode: usize,
    pub size: usize,
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
    Calling { spirit, mode: small("mode", MODES.len()), size: small("size", AREA_SIZES.len()) }
}

fn set_calling(stack: &mut ItemStack, calling: &Calling) {
    let mut c = Compound::new();
    if let Some(u) = calling.spirit {
        c.insert("spirit".into(), Tag::IntArray((0..4).rev().map(|i| (u >> (32 * i)) as u32 as i32).collect()));
    }
    c.insert("mode".into(), Tag::Byte(calling.mode as i8));
    c.insert("size".into(), Tag::Byte(calling.size as i8));
    let _ = stack.set(&game::Nbt(CALLING_COMPONENT), Tag::Compound(c));
}

/// The book's job, by its id.
fn book_job(book: &str) -> Option<&'static str> {
    (book == CLEANER_BOOK).then_some(CLEANER)
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

/// A foliot: its Part state, and Occultism's janitor goals.
pub struct FoliotPart(pub Arc<crate::State>);

/// The names of this port's goals, as a mob's goal list gives them.
const OUR_GOALS: [&str; 3] = ["PickupItems", "DepositItems", "ReturnToWorkArea"];

impl EntityPartState for FoliotPart {
    type Data = Work;
    const DATA: &'static AttachmentType<Work> = &WORK;
}

#[mod_api::horizon_part]
impl EntityPart for FoliotPart {
    fn register_goals(&self, goals: &mut GoalRegistrar<'_>) {
        // Occultism's CleanerJob: the return and pickup in the target selector, which run beside
        // the move goals; here all three drive the navigation, the pickup first.
        goals.add_goal(1, PickupItems { s: self.0.clone(), target: None });
        goals.add_goal(4, DepositItems { s: self.0.clone() });
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
    s.workers.ticks.load(Ordering::Relaxed) % 10 == 0 || cx.random().next_int_bound(10).unwrap_or(0) == 0
}

/// `PickupItemsGoal`: to the nearest item in the work area that fits in the spirit's hand.
struct PickupItems {
    s: Arc<crate::State>,
    target: Option<EntityRef>,
}

impl PickupItems {
    /// The nearest item lying in the work area that the hand takes.
    fn find(&self, cx: &GoalContext<'_>) -> Option<EntityRef> {
        let id = cx.mob().entity().id();
        let work = self.s.workers.work(id)?;
        if work.job != CLEANER {
            return None;
        }
        let host = cx.host();
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
                (item.pickup_delay(host).unwrap_or(0) <= 0 && fits(&stack)).then_some((e, e.position(host)?))
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
        self.target = self.find(cx);
        self.target.is_some()
    }
    fn can_continue(&mut self, cx: &mut GoalContext<'_>) -> bool {
        self.target.is_some_and(|t| cx.host().item_entity(t).and_then(|i| i.stack(cx.host())).is_some())
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
            // Beside the container, on the spirit's side of it (`getMoveTarget`).
            let (dx, dz) = (here[0] - at[0], here[2] - at[2]);
            let side = if dx.abs() > dz.abs() { [dx.signum(), 0.0] } else { [0.0, dz.signum()] };
            cx.move_to([at[0] + side[0], f64::from(pos[1]), at[2] + side[1]], 1.0);
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
            let settings = Self::rewrite(host, player, hand, event.stack, |c| c.mode = (c.mode + 1) % MODES.len());
            let mode = translated(&format!("enum.occultism.book_of_calling.item_mode.{}", MODES[settings.mode]), Vec::new());
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
        match MODES[calling(event.stack).mode] {
            "set_base" => {
                self.0.workers.edit(host, spirit, |w| w.base = Some(pos));
                let line = translated("item.occultism.book_of_calling.message_set_base", vec![name, block_text(&*host, level, pos)]);
                tell(host, player, line);
            }
            _ => {
                if level.capability(host, &ITEMS, pos, Some(direction(face))).is_none() {
                    return Some(InteractionResult::Success);
                }
                self.0.workers.edit(host, spirit, |w| w.deposit = Some((pos, face)));
                let side = Text::literal(FACE_NAMES[usize::from(face)]);
                let line = translated("item.occultism.book_of_calling.message_set_deposit", vec![name, block_text(&*host, level, pos), side]);
                tell(host, player, line);
            }
        }
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
        let mode = say(&format!("enum.occultism.book_of_calling.item_mode.{}", MODES[settings.mode]));
        let area = say(&format!("enum.occultism.work_area_size.{}", AREA_SIZES[settings.size].1));
        event.lines.push((Text::literal(&format!("{}: {mode}", say("gui.occultism.book_of_calling.mode"))), 0xFFAAAAAA));
        event.lines.push((Text::literal(&format!("{}: {area}", say("gui.occultism.book_of_calling.work_area"))), 0xFFAAAAAA));
    }
}
