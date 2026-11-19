// Occultism's ritual circles: the glue between the game and the ritual rules (ritual.rs). Chalks
// draw glyphs and wear by one each, as Occultism's (256 uses); sacrificial bowls hold one item
// each; a golden bowl checks its pentacle and ingredients, then runs the ritual and leaves its
// result in the bowl. Players with the Third Eye (or the goggles) find raw iesnium in the silver
// ore they mine, and otherworld logs among the oaks they chop.

use crate::game::{self, id, lock};
use crate::ritual::{self, Accept, Pentacle, Ritual, RitualState, Step};
use mod_api::*;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

pub const BOWL: &str = "occultism:sacrificial_bowl";
pub const GOLDEN_BOWL: &str = "occultism:golden_sacrificial_bowl";
pub const GLYPH_PREFIX: &str = "occultism:chalk_glyph_";
pub const RITUAL_TYPE: &str = "occultism:ritual";
pub const PENTACLES: &str = "occultism:pentacles.json";
pub const THIRD_EYE: &str = "occultism:third_eye";
pub const GOGGLES: &str = "occultism:otherworld_goggles";
pub const RAW_SILVER: &str = "occultism:raw_silver";
pub const RAW_IESNIUM: &str = "occultism:raw_iesnium";
pub const SILVER_ORE_TABLES: [&str; 2] = ["occultism:blocks/silver_ore", "occultism:blocks/silver_ore_deepslate"];
/// Occultism's otherworld trees are oaks to the unaided eye: here an oak log chopped with the
/// Third Eye or the goggles is, by iesnium's chances, an otherworld log.
pub const OAK_LOG_TABLE: &str = "minecraft:blocks/oak_log";
pub const OAK_LOG: &str = "minecraft:oak_log";
pub const OTHERWORLD_LOG: &str = "occultism:otherworld_log";
/// The chalks' and glyphs' colours.
pub const COLOURS: [&str; 16] = [
    "white", "light_gray", "gray", "black", "brown", "red", "orange", "yellow", "lime", "green", "cyan", "light_blue", "blue", "purple",
    "magenta", "pink",
];

type Place = (String, [i32; 3]);

#[derive(Default)]
pub struct Rites {
    /// The ritual recipes, resolved once per recipe table (as the crushing ones).
    rituals: Mutex<(Vec<usize>, Arc<[Ritual]>)>,
    pentacles: Mutex<Option<Arc<HashMap<String, Pentacle>>>>,
    /// What each loaded bowl holds, as its last tick saw it, by dimension and position.
    bowls: Mutex<HashMap<Place, Option<String>>>,
    /// Rituals a player started on a golden bowl, for its next tick: the ritual and the caster.
    pending: Mutex<HashMap<Place, (String, u128)>>,
    /// Golden bowls running a ritual.
    running: Mutex<HashMap<Place, String>>,
    /// Golden bowls whose ritual's sacrifice was just made, for their next tick.
    sacrifices: Mutex<HashSet<Place>>,
    /// Running bowls whose item a player just used nearby, until their next tick.
    item_uses: Mutex<HashSet<Place>>,
    /// Items players offered to bowls, put in at the end of the server tick: what a click's hook
    /// moves itself doesn't stay (Nightly 2026-11-18).
    offers: Mutex<Vec<Offer>>,
    /// Spirits a ritual summoned to work, for the glue to set to it.
    called: Mutex<Vec<Called>>,
    rng: AtomicU64,
}

/// A spirit a `SUMMON_JOB` ritual summoned: its entity, job, caster and name, the book of calling
/// it is bound to, and where the bowl is.
pub struct Called {
    pub spirit: u32,
    pub job: String,
    pub caster: Option<u128>,
    pub name: Option<String>,
    pub book: String,
    pub at: [f64; 3],
}

/// A player's click offering what they hold to a bowl: one of it goes in, and for a golden
/// bowl's activation item, its ritual waits for the bowl's next tick.
struct Offer {
    place: Place,
    player: u128,
    hand: Hand,
    item: String,
    /// The stack offered, as it was: damage, name and all.
    resource: ItemResource,
    ritual: Option<String>,
}

/// What a chalk may draw over: air, a glyph, low plants (`canBeReplaced`, as far as this port meets it).
fn replaceable(block: &str) -> bool {
    ritual::AIR.contains(&block)
        || block.starts_with(GLYPH_PREFIX)
        || matches!(block, "minecraft:short_grass" | "minecraft:fern" | "minecraft:short_dry_grass" | "minecraft:snow")
}

fn at(pos: [i32; 3], off: [i32; 3]) -> [i32; 3] {
    [pos[0] + off[0], pos[1] + off[1], pos[2] + off[2]]
}

fn tell(host: &mut dyn ServerHost, uuid: u128, key: &str) {
    game::tell(host, uuid, game::say(&format!("ritual.occultism.{key}"), &[]));
}

/// The ritual has begun, and what it asks to be sacrificed or used (`ritual.occultism.sacrifice`).
fn announce(host: &mut dyn ServerHost, uuid: u128, ritual: &Ritual) {
    tell(host, uuid, "started");
    if let Some(name) = &ritual.sacrifice_name {
        tell(host, uuid, "sacrifice");
        game::tell(host, uuid, game::say(name, &[]));
    }
    if let Some(item) = ritual.item_use.first() {
        tell(host, uuid, "use_item");
        game::tell(host, uuid, game::say(&format!("item.{}", item.replace(':', ".")), &[]));
    }
}

/// The name of the spirit `stack` is bound to, if any (`ItemNBTUtil.getBoundSpiritName`).
pub fn spirit_name(stack: &ItemStack) -> Option<String> {
    match stack.get(&game::Nbt(ritual::SPIRIT_COMPONENT))? {
        Tag::String(name) => Some(name.to_string_lossy()),
        _ => None,
    }
}

/// `stack` bound to the spirit `name`.
pub fn set_spirit_name(stack: &mut ItemStack, name: &str) {
    let _ = stack.set(&game::Nbt(ritual::SPIRIT_COMPONENT), Tag::String(name.into()));
}

/// The block at `pos` in `level`, by id.
fn block(host: &dyn Host, level: LevelRef, pos: [i32; 3]) -> &'static str {
    game::block_name(&*host, level.block(host, pos))
}

/// Whether a player sees the otherworld: the Third Eye, the goggles worn.
#[derive(Clone, Copy, Default)]
pub struct Sight {
    third_eye: bool,
    goggles: bool,
}

pub fn sight(host: &dyn Host, player: u128) -> Sight {
    let Some(living) = game::living(host, player) else { return Sight::default() };
    Sight {
        third_eye: living.effects(host).unwrap_or_default().iter().any(|e| e.id.id(host) == THIRD_EYE),
        goggles: living.equipment(host, EquipmentSlot::Head).is_some_and(|s| id(&*host, &s) == GOGGLES),
    }
}

impl Rites {
    pub fn rituals(&self, host: &dyn Host) -> Arc<[Ritual]> {
        let table = host.recipes_of_type(RITUAL_TYPE);
        let key: Vec<usize> = table.iter().map(|r| Arc::as_ptr(&r.json) as usize).collect();
        let mut cache = lock(&self.rituals);
        if cache.0 != key {
            let parsed: Vec<Ritual> = table.iter().filter_map(|r| parse_ritual(host, r)).collect();
            *cache = (key, parsed.into());
        }
        cache.1.clone()
    }

    pub fn pentacles(&self, host: &dyn Host) -> Arc<HashMap<String, Pentacle>> {
        let mut cache = lock(&self.pentacles);
        if let Some(p) = cache.as_ref() {
            return p.clone();
        }
        let file = Identifier::parse(PENTACLES).ok().and_then(|id| host.resource(PackType::ServerData, &id));
        let parsed = Arc::new(file.map(|b| parse_pentacles(&b)).unwrap_or_default());
        *cache = Some(parsed.clone());
        parsed
    }

    /// The sacrificial bowls around a golden bowl that count for `pentacle`'s rituals, and what
    /// each holds (`Ritual.getSacrificialBowls`), nearest first.
    fn bowls_near(&self, host: &dyn Host, level: LevelRef, pos: [i32; 3], pentacle: &Pentacle) -> Vec<([i32; 3], Option<String>)> {
        let dim = level.dimension_id();
        let mut out: Vec<([i32; 3], Option<String>)> = lock(&self.bowls)
            .iter()
            .filter(|((d, b), _)| {
                d == dim
                    && (b[0] - pos[0]).abs() <= ritual::BOWL_RANGE
                    && (b[2] - pos[2]).abs() <= ritual::BOWL_RANGE
                    && b[1] - pos[1] >= -(pentacle.below + 1)
                    && b[1] - pos[1] <= pentacle.above + 1
            })
            .map(|((_, b), item)| (*b, item.clone()))
            .collect();
        out.retain(|(b, _)| block(host, level, *b) == BOWL);
        out.sort_by_key(|(b, _)| (b[0] - pos[0]).abs() + (b[1] - pos[1]).abs() + (b[2] - pos[2]).abs());
        out
    }

    /// A chalk used on a block (`ChalkItem.useOn`): over its own glyph the next sign, else its
    /// glyph drawn and the chalk worn by one (not over a glyph of its own colour; a creative
    /// player's never). Whether the use was the chalk's.
    pub fn use_chalk(&self, host: &mut dyn ServerHost, event: &UseOnEvent<'_>) -> bool {
        let Some(glyph) = ritual::chalk_glyph(id(&*host, event.stack)) else { return false };
        let Some(level) = host.level(host.dimension()) else { return false };
        let clicked = event.pos;
        let here = block(&*host, level, clicked);
        if here == glyph {
            let current = level.block_property(&*host, clicked, "sign").and_then(|s| s.parse().ok());
            host.set_block_property(clicked, "sign", &ritual::glyph_sign(0, current).to_string());
            return true;
        }
        let sturdy = level.block_face_sturdy(&*host, clicked, BlockFace::Up, SupportType::Full) == Some(true);
        let into = replaceable(here) && !ritual::AIR.contains(&here);
        let Some(target) = ritual::glyph_target(clicked, event.face, into, sturdy) else { return true };
        let over = if target == clicked { here } else { block(&*host, level, target) };
        if !replaceable(over) {
            return true;
        }
        let Some(player) = host.player(event.player) else { return true };
        let yaw = player.entity().rotation(&*host).map_or(0.0, |r| r[0]);
        let sign = ritual::glyph_sign((game::roll(&self.rng) % 1000) as u32, None);
        let state = format!("{glyph}[facing={},segment_amount=1,sign={sign}]", ritual::glyph_facing(yaw));
        if game::set_block(host, level, target, &state)
            && over != glyph
            && let Some(living) = game::living(&*host, event.player)
        {
            let slot = if event.hand == Hand::Main { EquipmentSlot::MainHand } else { EquipmentSlot::OffHand };
            living.hurt_and_break(host, slot, 1);
        }
        true
    }

    /// A player used a sacrificial or golden bowl, with `held` in `hand` or nothing.
    pub fn use_bowl(&self, host: &mut dyn ServerHost, click: &BlockClick<'_>, golden: bool) -> Option<InteractionResult> {
        let level = host.level(click.dimension.key().id())?;
        let player = click.player;
        let key = (level.dimension_id().to_string(), click.pos);
        if golden && lock(&self.running).contains_key(&key) {
            tell(host, player, "busy");
            return Some(InteractionResult::Success);
        }
        let content = lock(&self.bowls).get(&key).cloned().flatten();
        if content.is_some() {
            // An empty or a full hand takes the bowl's item back.
            if let Some(stack) = game::extract(host, level, click.pos, Direction::Up, 0, 64)
                && let Some(left) = game::give(host, player, stack)
            {
                let _ = game::insert(host, level, click.pos, Direction::Up, left);
            }
            lock(&self.bowls).insert(key, None);
            return Some(InteractionResult::Success);
        }
        let held = click.stack?;
        let item = id(&*host, held);
        if golden {
            let rituals = self.rituals(&*host);
            let candidates: Vec<&Ritual> = rituals.iter().filter(|r| r.activation.iter().any(|a| a == item)).collect();
            if !candidates.is_empty() {
                self.start(host, level, click, held, &candidates);
                return Some(InteractionResult::Success);
            }
        }
        if !game::plain(held) {
            return None;
        }
        lock(&self.offers).push(Offer { place: key, player, hand: click.hand, item: item.to_string(), resource: ItemResource::of(held), ritual: None });
        Some(InteractionResult::Success)
    }

    /// The caster offers an activation item to a golden bowl: the first ritual whose pentacle is
    /// drawn and whose ingredients are in the bowls starts on the bowl's next tick.
    /// The stack itself goes in, one of it, damage and components and all: a repair's worn tool, a
    /// named book of binding.
    fn start(&self, host: &mut dyn ServerHost, level: LevelRef, click: &BlockClick<'_>, held: &ItemStack, candidates: &[&Ritual]) {
        let item = id(&*host, held);
        let pos = click.pos;
        let pentacles = self.pentacles(&*host);
        let circled: Vec<&&Ritual> = {
            let read = |off: [i32; 3]| block(&*host, level, at(pos, off)).to_string();
            candidates.iter().filter(|r| pentacles.get(&r.pentacle).is_some_and(|p| p.matches(&read))).collect()
        };
        if circled.is_empty() {
            tell(host, click.player, "no_pentacle");
            return;
        }
        let ready = circled.into_iter().find(|r| {
            let p = &pentacles[&r.pentacle];
            let items: Vec<String> = self.bowls_near(&*host, level, pos, p).into_iter().filter_map(|(_, i)| i).collect();
            ritual::ingredients_present(&r.ingredients, &items)
        });
        let Some(ritual) = ready else {
            tell(host, click.player, "no_ingredients");
            return;
        };
        let place = (level.dimension_id().to_string(), pos);
        lock(&self.offers).push(Offer { place, player: click.player, hand: click.hand, item: item.to_string(), resource: ItemResource::of(held), ritual: Some(ritual.id.clone()) });
    }

    /// After `player`'s tick, while their inventory can be read: what they offered to bowls goes
    /// in, from their hand or wherever the click left it (armour the click put on), if the bowl
    /// still has room. An activation item's ritual is then announced, for the bowl's next tick.
    pub fn take_offers(&self, host: &mut dyn ServerHost, player: u128) {
        let offers: Vec<Offer> = {
            let mut all = lock(&self.offers);
            let (mine, others) = std::mem::take(&mut *all).into_iter().partition(|o| o.player == player);
            *all = others;
            mine
        };
        for offer in offers {
            let (dimension, pos) = &offer.place;
            let Some(level) = host.level(dimension) else { continue };
            // The activation item goes in as it is (a repair's worn tool, a named book); others plain.
            let Some(one) = offer.resource.to_stack(1) else { continue };
            let in_hand = game::living(&*host, player).and_then(|l| l.hand_item(&*host, offer.hand)).is_some_and(|h| ItemResource::of(&h) == offer.resource);
            let slot = if in_hand {
                None
            } else {
                let Some(entity) = host.player(player).map(PlayerRef::entity) else { continue };
                let Some(items) = entity.capability(host, &ENTITY_ITEMS, ()) else { continue };
                let size = items.size(host);
                match (0..size).rev().find(|&i| items.resource(host, i).as_ref() == Some(&offer.resource)) {
                    Some(i) => Some((items, i)),
                    None => continue,
                }
            };
            if game::insert(host, level, *pos, Direction::Up, one).is_some() {
                continue;
            }
            match slot {
                None => {
                    game::use_one(host, player, offer.hand);
                }
                Some((items, i)) => {
                    if host.player(player).and_then(|p| p.is_creative(&*host)) != Some(true) {
                        let _ = host.transaction(|host, tx| Ok::<u32, ()>(items.extract(host, tx, i, &offer.resource, 1)));
                    }
                }
            }
            lock(&self.bowls).insert(offer.place.clone(), Some(offer.item.clone()));
            if let Some(id) = offer.ritual {
                let rituals = self.rituals(&*host);
                lock(&self.pending).insert(offer.place, (id.clone(), player));
                if let Some(ritual) = rituals.iter().find(|r| r.id == id) {
                    announce(host, player, ritual);
                }
            }
        }
    }

    /// A player used `item` (right-clicked with it) at `at`: the item use of each ritual running
    /// within 16 blocks that asks for it.
    pub fn used_item(&self, host: &dyn Host, dimension: &str, at: [i32; 3], item: &str) {
        let rituals = self.rituals(host);
        let made: Vec<Place> = lock(&self.running)
            .iter()
            .filter(|((dim, pos), id)| {
                dim == dimension && rituals.iter().find(|r| &r.id == *id).is_some_and(|r| ritual::is_item_use(&r.item_use, item, *pos, at))
            })
            .map(|(place, _)| place.clone())
            .collect();
        lock(&self.item_uses).extend(made);
    }

    /// A mob of `kind` died at `at` in `dimension`: the sacrifice of each ritual running near it that
    /// asks for its kind, if a player killed it.
    pub fn death(&self, host: &dyn Host, dimension: &str, at: [i32; 3], kind: &str, by_player: bool) {
        let rituals = self.rituals(host);
        let made: Vec<Place> = lock(&self.running)
            .iter()
            .filter(|((dim, pos), id)| {
                dim == dimension
                    && rituals.iter().find(|r| &r.id == *id).is_some_and(|r| ritual::is_sacrifice(&r.sacrifice, kind, by_player, *pos, at))
            })
            .map(|(place, _)| place.clone())
            .collect();
        lock(&self.sacrifices).extend(made);
    }

    /// `kind` summoned on the golden bowl at `pos` (`SummonRitual.prepareLivingEntityForSpawn`):
    /// facing anywhere, named `name`, kept from despawning. A spirit's helpers come with it, as its
    /// `finalizeSpawn` brings them: a difficulty's worth and up to five more, when mobs spawn.
    fn summon(&self, host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], kind: &str, name: Option<&str>) -> Option<u32> {
        let dim = level.dimension_id().to_string();
        let yaw = (game::roll(&self.rng) % 360) as f32;
        let mut nbt = Compound::new();
        nbt.insert("Rotation".into(), Tag::List(vec![Tag::Float(yaw.to_bits()), Tag::Float(0f32.to_bits())]));
        nbt.insert("PersistenceRequired".into(), Tag::Byte(1));
        if let Some(name) = name {
            nbt.insert("CustomName".into(), Tag::String(name.into()));
        }
        // A zombie with something on its head doesn't burn in the sun, and its model is this
        // mod's (drawn instead of the vanilla one, which hides its armour): Occultism's spirits
        // don't burn.
        if let Some(extra) = crate::spirits::summon_nbt(kind) {
            for (k, v) in extra {
                nbt.insert(k.as_str().into(), v);
            }
        }
        let at = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + ritual::BOWL_TOP, f64::from(pos[2]) + 0.5];
        let summoned = host.summon(&dim, kind, at, nbt)?;
        let Some(helper) = crate::spirits::helper(kind) else { return Some(summoned) };
        // `spawn_mobs` (26.1's name), else `doMobSpawning`.
        let rule = |name: &str| level.game_rule(&*host, name).ok().flatten();
        if matches!(rule("minecraft:spawn_mobs").or_else(|| rule("doMobSpawning")), Some(GameRuleValue::Bool(false))) {
            return Some(summoned);
        }
        let difficulty = match level.difficulty(&*host) {
            Some("peaceful") => 0,
            Some("easy") => 1,
            Some("hard") => 3,
            _ => 2,
        };
        for _ in 0..difficulty + (game::roll(&self.rng) % 6) as u32 {
            let spread = |r: u64| ((r % 2000) as f64 / 1000.0 - 1.0) * (1 + r % 4) as f64;
            let p = [f64::from(pos[0]) + spread(game::roll(&self.rng)), f64::from(pos[1]) + 1.5, f64::from(pos[2]) + spread(game::roll(&self.rng))];
            let mut nbt = Compound::new();
            nbt.insert("Rotation".into(), Tag::List(vec![Tag::Float(((game::roll(&self.rng) % 360) as f32).to_bits()), Tag::Float(0f32.to_bits())]));
            host.summon(&dim, helper, p, nbt);
        }
        Some(summoned)
    }

    /// The spirits rituals summoned to work since the last call.
    pub fn take_called(&self) -> Vec<Called> {
        std::mem::take(&mut *lock(&self.called))
    }

    /// A book of binding a player carries in `slot` gets its spirit's name, as Occultism names it
    /// when it is crafted or first carried (`generateBoundSpiritNameIfNone`).
    pub fn name_book(&self, host: &mut dyn ServerHost, stack: &ItemStack, slot: i32) {
        if !id(&*host, stack).starts_with(ritual::BOUND_BOOK) || spirit_name(stack).is_some() {
            return;
        }
        let Some(player) = host.players().acting_player().map(PlayerRef::uuid) else { return };
        let name = crate::network::spirit_name(game::roll(&self.rng));
        let place = ItemPlace::PlayerSlot { player, slot };
        let _ = host.transaction(|host, tx| if place.edit(host, tx, |s| set_spirit_name(s, &name)) { Ok(()) } else { Err(()) });
    }

    /// A sacrificial bowl's tick: remember what it holds.
    fn bowl_tick(&self, host: &dyn Host, pos: [i32; 3], items: &[Option<ItemStack>]) {
        let item = items.first().cloned().flatten().map(|s| id(&*host, &s).to_string());
        let key = (host.dimension().to_string(), pos);
        let mut bowls = lock(&self.bowls);
        if bowls.get(&key) != Some(&item) {
            bowls.insert(key, item);
        }
    }

    /// A golden bowl's tick: start a pending ritual, or run the one under way. A ritual with an
    /// effect that finished: its effect and caster, for the glue that does it.
    fn golden_tick(&self, host: &mut dyn ServerHost, pos: [i32; 3], entity: &mut ModBlockEntityView<'_>) -> Option<(String, u128)> {
        let level = host.level(host.dimension())?;
        let key = (level.dimension_id().to_string(), pos);
        let item = entity.items.first().cloned().flatten().map(|s| id(&*host, &s).to_string());
        lock(&self.bowls).insert(key.clone(), item.clone());
        let (mut state, caster) = read_ritual(&entity.nbt);
        let before = (state.clone(), caster);
        let mut caster = caster;
        if state.ritual.is_none()
            && item.is_some()
            && let Some((id, who)) = lock(&self.pending).remove(&key)
        {
            state = RitualState { ritual: Some(id), ..Default::default() };
            caster = Some(who);
        }
        let rituals = self.rituals(&*host);
        let current = state.ritual.as_ref().and_then(|id| rituals.iter().find(|r| &r.id == id));
        if lock(&self.sacrifices).remove(&key) && current.is_some() {
            state.sacrificed = true;
        }
        if lock(&self.item_uses).remove(&key) && current.is_some() {
            state.item_used = true;
        }
        let mut interrupted = state.ritual.is_some() && current.is_none();
        let mut finished = false;
        if let Some(r) = current {
            let pentacles = self.pentacles(&*host);
            if !item.as_ref().is_some_and(|i| r.activation.contains(i)) {
                interrupted = true; // the activation item was taken out
            } else if state.ticks % ritual::TICKS_PER_SECOND == 0 && {
                let read = |off: [i32; 3]| block(&*host, level, at(pos, off)).to_string();
                !pentacles.get(&r.pentacle).is_some_and(|p| p.matches(&read))
            } {
                interrupted = true; // the circle was broken
            } else {
                match ritual::advance(&mut state, r) {
                    Step::Wait => {}
                    Step::Take(i) => {
                        let bowls = pentacles.get(&r.pentacle).map(|p| self.bowls_near(&*host, level, pos, p)).unwrap_or_default();
                        let contents: Vec<Option<String>> = bowls.iter().map(|(_, i)| i.clone()).collect();
                        let took = ritual::bowl_for(&r.ingredients[i], &contents)
                            .and_then(|b| game::extract(host, level, bowls[b].0, Direction::Up, 0, 1).map(|_| bowls[b].0));
                        match took {
                            Some(b) => {
                                ritual::taken(&mut state);
                                lock(&self.bowls).insert((key.0.clone(), b), None);
                            }
                            None => interrupted = true,
                        }
                    }
                    Step::Done => finished = true,
                }
            }
        }
        let mut effect = None;
        if finished {
            let r = current.expect("a finished ritual is known");
            if entity.items.is_empty() {
                entity.items.resize(1, None);
            }
            let book = entity.items[0].as_ref().and_then(spirit_name);
            match (&r.effect, caster) {
                // The item comes back whole, with its components (`RepairRitual`).
                (Some(e), _) if e == ritual::REPAIR => {
                    if let Some(stack) = entity.items[0].as_mut() {
                        stack.set_damage(0);
                    }
                    if let Some(who) = caster {
                        tell(host, who, "finished");
                    }
                }
                // The result is bound to the book's spirit; the book is spent.
                (Some(e), _) if e == ritual::SPIRIT_NAME => {
                    let mut result = game::stack(&*host, &r.result, r.count);
                    if let (Some(result), Some(name)) = (result.as_mut(), &book) {
                        set_spirit_name(result, name);
                    }
                    entity.items[0] = result;
                    if let Some(who) = caster {
                        tell(host, who, "finished");
                    }
                }
                // The spirit comes on the bowl, named for the book's; the book is spent.
                (Some(e), _) if e == ritual::SUMMON || e == ritual::SUMMON_JOB => {
                    entity.items[0] = None;
                    let summoned = r.summon.as_ref().and_then(|kind| self.summon(host, level, pos, kind, book.as_deref()));
                    // A spirit summoned to work takes up its job, and the caster gets its book.
                    if let (Some(spirit), Some(job)) = (summoned, &r.job) {
                        let at = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.5, f64::from(pos[2]) + 0.5];
                        let called = Called { spirit, job: job.clone(), caster, name: book.clone(), book: r.result.clone(), at };
                        lock(&self.called).push(called);
                    }
                    if let Some(who) = caster {
                        tell(host, who, "finished");
                    }
                }
                // The book is spent; what the ritual does is told by the glue.
                (Some(e), Some(who)) => {
                    entity.items[0] = None;
                    effect = Some((e.clone(), who));
                }
                _ => {
                    entity.items[0] = game::stack(&*host, &r.result, r.count);
                    if let Some(who) = caster {
                        tell(host, who, "finished");
                    }
                }
            }
        }
        if interrupted && let Some(who) = caster {
            tell(host, who, "interrupted");
        }
        if finished || interrupted {
            state = RitualState::default();
            caster = None;
        }
        match &state.ritual {
            Some(id) => lock(&self.running).insert(key, id.clone()),
            None => lock(&self.running).remove(&key),
        };
        if (state.clone(), caster) != before {
            write_ritual(&mut entity.nbt, &state, caster);
        }
        effect
    }

    /// Whether a roll at `chance` comes up.
    fn chance(&self, chance: f64) -> bool {
        chance > 0.0 && (game::roll(&self.rng) % 1_000_000) as f64 / 1_000_000.0 < chance
    }

    /// Raw iesnium in a player's silver ore drops, by chance, with the Third Eye or the goggles.
    pub fn silver_ore_drops(&self, host: &dyn Host, context: &LootDrops<'_>, sight: Sight, drops: &mut Vec<ItemStack>) {
        if !SILVER_ORE_TABLES.contains(&context.table) || context.used || context.explosion.is_some() {
            return;
        }
        // Silk touch takes the ore whole: iesnium only shows when it breaks to raw silver.
        if !drops.iter().any(|s| id(&*host, s) == RAW_SILVER) || context.player.is_none() {
            return;
        }
        if self.chance(ritual::iesnium_chance(sight.third_eye, sight.goggles))
            && let Some(iesnium) = game::stack(&*host, RAW_IESNIUM, 1)
        {
            drops.push(iesnium);
        }
    }

    /// An oak log a player chops is now and then an otherworld log, with the Third Eye or the goggles.
    pub fn oak_log_drops(&self, host: &dyn Host, context: &LootDrops<'_>, sight: Sight, drops: &mut [ItemStack]) {
        if context.table != OAK_LOG_TABLE || context.used || context.explosion.is_some() || context.player.is_none() {
            return;
        }
        let chance = ritual::iesnium_chance(sight.third_eye, sight.goggles);
        for stack in drops.iter_mut().filter(|s| id(&*host, s) == OAK_LOG) {
            if self.chance(chance)
                && let Some(log) = game::stack(&*host, OTHERWORLD_LOG, stack.count())
            {
                *stack = log;
            }
        }
    }
}

/// A sacrificial bowl or golden bowl's block entity: one slot, any face.
pub struct BowlPart(pub Arc<crate::State>);

pub fn container() -> ModContainer {
    // One item, no screen: players use the bowl by hand, hoppers through any face.
    ModContainer { slots: 1, faces: std::array::from_fn(|_| vec![0]), inputs: vec![0], outputs: vec![0], menu: None }
}

#[mod_api::horizon_part]
impl BlockPart for BowlPart {
    fn use_item_on(&self, host: &mut dyn ServerHost, event: &mut UseItemOnEvent<'_>) -> Option<InteractionResult> {
        let golden = game::block_name(&*host, Some(event.state.block())) == GOLDEN_BOWL;
        self.0.rites.use_bowl(host, event.click, golden)
    }

    fn use_without_item(&self, host: &mut dyn ServerHost, event: &mut UseWithoutItemEvent<'_>) -> Option<InteractionResult> {
        let golden = game::block_name(&*host, Some(event.state.block())) == GOLDEN_BOWL;
        self.0.rites.use_bowl(host, event.click, golden)
    }
}

#[mod_api::horizon_part]
impl BlockEntityPart for BowlPart {
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let pos = event.pos;
        // Both bowls share one block entity type: the block tells them apart.
        let golden = host.level(host.dimension()).is_some_and(|l| block(&*host, l, pos) == GOLDEN_BOWL);
        if golden {
            if let Some((effect, caster)) = self.0.rites.golden_tick(host, pos, &mut event.entity)
                && effect == ritual::RECALL_STORAGE
            {
                self.0.recall(host, pos, caster);
            }
        } else {
            self.0.rites.bowl_tick(&*host, pos, &event.entity.items);
        }
        // Clients draw the bowl's item, turning faster while a ritual runs.
        let entity = &mut *event.entity;
        let active = entity.nbt.get(&"ritual".into()).is_some();
        let now = crate::altar::shown(&*host, entity.items.first().and_then(Option::as_ref), active);
        crate::altar::show(host, pos, &mut entity.nbt, now);
    }

    fn block_entity_update_tag(&self, event: &mut BlockEntityUpdateTagEvent<'_>) -> Option<Compound> {
        Some(crate::altar::shown_tag(event.entity))
    }

    fn block_entity_submit(&self, host: &dyn ClientHost, event: &mut BlockEntitySubmitEvent<'_>) {
        crate::altar::draw_shown(host, event.pos, event.entity, &mut *event.draw, true);
    }
}

/// The books of binding that carry a spirit's name.
pub const BOUND_BOOKS: [&str; 4] = [
    "occultism:book_of_binding_bound_foliot",
    "occultism:book_of_binding_bound_djinni",
    "occultism:book_of_binding_bound_afrit",
    "occultism:book_of_binding_bound_marid",
];

/// A bound book of binding: named for its spirit once carried.
pub struct BookPart(pub Arc<crate::State>);

#[mod_api::horizon_part]
impl ItemPart for BookPart {
    fn inventory_tick(&self, host: &mut dyn ServerHost, event: &mut InventoryTickEvent<'_>) {
        self.0.rites.name_book(host, event.stack, event.slot);
    }
}

/// A chalk: it draws its glyph where it is used.
pub struct ChalkPart(pub Arc<crate::State>);

#[mod_api::horizon_part]
impl ItemPart for ChalkPart {
    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else { return None };
        self.0.rites.use_chalk(host, event).then_some(InteractionResult::SuccessServer)
    }
}

/// An `occultism:ritual` recipe, its items resolved by the game.
fn parse_ritual(host: &dyn Host, r: &mod_api::Recipe) -> Option<Ritual> {
    let j = &*r.json;
    let activation = crate::altar::ingredient(host, &j["activation_item"])?;
    let ingredients = j["ingredients"].as_array()?.iter().map(|i| crate::altar::ingredient(host, i)).collect::<Option<Vec<_>>>()?;
    Some(Ritual {
        id: r.id.clone(),
        activation,
        duration: j["duration"].as_u64()?.max(1) as u32,
        ingredients,
        pentacle: j["pentacle_id"].as_str()?.to_string(),
        result: crate::altar::result_item(host, &j["result"])?,
        count: j["result"]["count"].as_u64().unwrap_or(1).clamp(1, 64) as u32,
        effect: j["ritual_type"]
            .as_str()
            .filter(|t| [ritual::RECALL_STORAGE, ritual::REPAIR, ritual::SPIRIT_NAME, ritual::SUMMON, ritual::SUMMON_JOB].contains(t))
            .map(str::to_string),
        sacrifice: j["sacrifice"]["kinds"].as_array().into_iter().flatten().filter_map(|k| k.as_str().map(str::to_string)).collect(),
        sacrifice_name: j["sacrifice"]["display_name"].as_str().map(str::to_string),
        summon: j["summon"].as_str().map(str::to_string),
        job: j["job"].as_str().map(str::to_string),
        item_use: if j["item_to_use"].is_null() { Vec::new() } else { crate::altar::ingredient(host, &j["item_to_use"])? },
    })
}

/// The mod's `pentacles.json`: `{id: {pattern, keys: {key: "any" | [blocks]}}}`.
fn parse_pentacles(bytes: &[u8]) -> HashMap<String, Pentacle> {
    let Ok(serde_json::Value::Object(all)) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    for (id, p) in all {
        let pattern: Vec<Vec<String>> = p["pattern"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|layer| layer.as_array().into_iter().flatten().filter_map(|row| row.as_str().map(str::to_string)).collect())
            .collect();
        let keys: Vec<(char, Accept)> = p["keys"]
            .as_object()
            .into_iter()
            .flatten()
            .filter_map(|(k, v)| {
                let ch = k.chars().next()?;
                let accept = match v {
                    serde_json::Value::Array(list) => Accept::OneOf(list.iter().filter_map(|b| b.as_str().map(str::to_string)).collect()),
                    _ => Accept::Any,
                };
                Some((ch, accept))
            })
            .collect();
        if let Ok(pentacle) = Pentacle::new(&pattern, &keys) {
            out.insert(id, pentacle);
        }
    }
    out
}

fn read_ritual(nbt: &Compound) -> (RitualState, Option<u128>) {
    let int = |k: &str| game::int(nbt, k).unwrap_or(0).max(0) as u32;
    let state = RitualState {
        ritual: game::text(nbt, "ritual"),
        ticks: int("ticks"),
        consumed: int("consumed"),
        sacrificed: int("sacrificed") != 0,
        item_used: int("item_used") != 0,
    };
    let caster = game::text(nbt, "caster").and_then(|s| u128::from_str_radix(&s, 16).ok());
    (state, caster)
}

fn write_ritual(nbt: &mut Compound, state: &RitualState, caster: Option<u128>) {
    match &state.ritual {
        Some(id) => {
            nbt.insert("ritual".into(), Tag::String(id.as_str().into()));
            nbt.insert("ticks".into(), Tag::Int(state.ticks as i32));
            nbt.insert("consumed".into(), Tag::Int(state.consumed as i32));
            nbt.insert("sacrificed".into(), Tag::Int(i32::from(state.sacrificed)));
            nbt.insert("item_used".into(), Tag::Int(i32::from(state.item_used)));
        }
        None => {
            for k in ["ritual", "ticks", "consumed", "sacrificed", "item_used"] {
                nbt.remove(&k.into());
            }
        }
    }
    match caster {
        Some(uuid) => nbt.insert("caster".into(), Tag::String(format!("{uuid:032x}").as_str().into())),
        None => nbt.remove(&"caster".into()),
    };
}
