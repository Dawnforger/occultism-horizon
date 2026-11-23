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
/// Every bowl that holds a ritual's ingredient: the plain one and Occultism's copper and silver ones.
pub const BOWLS: [&str; 3] = [BOWL, "occultism:copper_sacrificial_bowl", "occultism:silver_sacrificial_bowl"];
pub const GOLDEN_BOWL: &str = "occultism:golden_sacrificial_bowl";
/// How many satchel builds a player may have waiting for their next tick.
const MAX_PENDING_BUILDS: usize = 4;
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
pub const OTHERWORLD_SAPLING: &str = "occultism:otherworld_sapling";
/// Occultism's otherworld tree (`worldgen/configured_feature`), which the sapling grows.
pub const OTHERWORLD_TREE: &str = "occultism:tree_otherworld";
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
    /// Ritual satchels' builds, waiting for their players' ticks (pentacle_preview.rs).
    builds: Mutex<Vec<(u128, crate::pentacle_preview::Build)>>,
    /// Spirits a ritual summoned to work, for the glue to set to it.
    called: Mutex<Vec<Called>>,
    /// The vanilla creatures in the world that are Occultism's possessed or wild ones, by entity id
    /// (the damage hooks can't read their tags).
    possessed: Mutex<HashMap<u32, &'static str>>,
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
    /// How long the spirit stays, in seconds; none for for good.
    pub max_age: Option<u32>,
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

/// A chalk's uses (`max_damage`, Occultism's 256).
const CHALK_USES: u32 = 256;

/// A ritual satchel's word (Occultism's `item.occultism.ritual_satchel.*`).
fn satchel_says(host: &mut dyn ServerHost, uuid: u128, key: &str) {
    game::tell(host, uuid, game::say(&format!("item.occultism.ritual_satchel.{key}"), &[]));
}

/// `kind` spawned at `at` as the game spawns a summoned mob (`finalizeMobSpawn` with
/// `MOB_SUMMONED`: its equipment, its variant), then given `nbt`: its id.
fn spawn(host: &mut dyn ServerHost, level: LevelRef, kind: &str, at: [f64; 3], nbt: Compound) -> Option<u32> {
    let mob = level.spawn_mob(host, kind, at, SpawnReason::MobSummoned)?;
    mob.merge_nbt(host, nbt);
    Some(mob.id())
}

/// Whether mobs spawn in `level`: `spawn_mobs` (26.1's name), else `doMobSpawning`.
fn mobs_spawn(host: &dyn ServerHost, level: LevelRef) -> bool {
    let rule = |name: &str| level.game_rule(host, name).ok().flatten();
    !matches!(rule("minecraft:spawn_mobs").or_else(|| rule("doMobSpawning")), Some(GameRuleValue::Bool(false)))
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
        out.retain(|(b, _)| BOWLS.contains(&block(host, level, *b)));
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
        // Spirit fire takes the chalk's colour, the chalk unworn (`SpiritFireBlock.useItemOn`), for a
        // player who may build.
        if here == crate::fire::SPIRIT_FIRE {
            if host.player(event.player).and_then(|p| p.may_build(&*host)) == Some(true) {
                host.set_block_property(clicked, "color", glyph.trim_start_matches("occultism:chalk_glyph_"));
            }
            return true;
        }
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
        // A possessed creature is the vanilla one, with Occultism's attributes and drops.
        let original = kind;
        let (kind, possessed) = crate::spirits::possessed(kind).unwrap_or((kind, Vec::new()));
        for (k, v) in possessed {
            nbt.insert(k.as_str().into(), v);
        }
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
        if !crate::spirits::is_spirit(kind) {
            // Spawned as the game spawns a summoned mob, with its equipment and variant
            // (`finalizeMobSpawn` with `MOB_SUMMONED`), then given the rest.
            let summoned = spawn(host, level, kind, at, nbt)?;
            self.note(summoned, original);
            if mobs_spawn(host, level) {
                self.followers(host, level, pos, original);
            }
            return Some(summoned);
        }
        let summoned = host.summon(&dim, kind, at, nbt)?;
        let Some(helper) = crate::spirits::helper(kind) else { return Some(summoned) };
        if !mobs_spawn(host, level) {
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

    /// One of `kinds`, at random.
    fn pick<'k>(&self, kinds: &'k [String]) -> Option<&'k str> {
        if kinds.is_empty() {
            return None;
        }
        Some(&kinds[(game::roll(&self.rng) % kinds.len() as u64) as usize])
    }

    /// Somewhere around the golden bowl at `pos`, as Occultism places a wild ritual's creatures and
    /// their followers: a Gaussian times one to four blocks out each way, a block and a half up.
    fn around(&self, pos: [i32; 3]) -> [f64; 3] {
        let out = |c: i32| f64::from(c) + game::gaussian(&self.rng) * (1 + game::roll(&self.rng) % 4) as f64;
        [out(pos[0]), f64::from(pos[1]) + 1.5, out(pos[2])]
    }

    /// A wild ritual's creature (`SummonWildRitual.finish`): around the bowl, facing anywhere, with a
    /// spirit's name, spawned as a summoned mob, and its followers with it.
    fn summon_wild(&self, host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], kind: &str) {
        let (vanilla, mut nbt) = match crate::spirits::possessed(kind) {
            Some((vanilla, extra)) => (vanilla, extra.into_iter().map(|(k, v)| (k.as_str().into(), v)).collect()),
            None => (kind, Compound::new()),
        };
        self.named(&mut nbt);
        let Some(summoned) = spawn(host, level, vanilla, self.around(pos), nbt) else { return };
        self.note(summoned, kind);
        if mobs_spawn(host, level) {
            self.followers(host, level, pos, kind);
        }
    }

    /// A random facing and a spirit's name (`TextUtil.generateName`).
    fn named(&self, nbt: &mut Compound) {
        let yaw = (game::roll(&self.rng) % 360) as f32;
        nbt.insert("Rotation".into(), Tag::List(vec![Tag::Float(yaw.to_bits()), Tag::Float(0f32.to_bits())]));
        nbt.insert("CustomName".into(), Tag::String(crate::network::spirit_name(game::roll(&self.rng)).as_str().into()));
    }

    /// The creatures Occultism's `kind` brings (the wild hunt's skeletons, the evoker's illagers),
    /// each named, around the bowl at `pos`.
    fn followers(&self, host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], kind: &str) {
        for (follower, count, health) in crate::spirits::followers(kind, game::roll(&self.rng)) {
            for _ in 0..count {
                let (vanilla, mut nbt) = match crate::spirits::possessed(follower) {
                    Some((vanilla, extra)) => (vanilla, extra.into_iter().map(|(k, v)| (k.as_str().into(), v)).collect()),
                    None => (follower, Compound::new()),
                };
                self.named(&mut nbt);
                if let Some(health) = health {
                    nbt.insert("Health".into(), Tag::Float(health.to_bits()));
                }
                // A wild hunt skeleton has a stone sword seven times in ten
                // (`WildHuntSkeletonEntity.populateDefaultEquipmentSlots`).
                if follower == crate::spirits::WILD_HUNT_SKELETON && game::roll(&self.rng) % 10 < 7 {
                    let mut sword = Compound::new();
                    sword.insert("id".into(), Tag::String("minecraft:stone_sword".into()));
                    sword.insert("count".into(), Tag::Int(1));
                    let mut equipment = Compound::new();
                    equipment.insert("mainhand".into(), Tag::Compound(sword));
                    nbt.insert("equipment".into(), Tag::Compound(equipment));
                }
                if let Some(summoned) = spawn(host, level, vanilla, self.around(pos), nbt) {
                    self.note(summoned, follower);
                }
            }
        }
    }

    /// The entity `id` is Occultism's `kind`, if that is a possessed or wild creature.
    fn note(&self, id: u32, kind: &str) {
        if let Some(kind) = crate::spirits::possessed_id(kind) {
            lock(&self.possessed).insert(id, kind);
        }
    }

    /// A creature joined from disk, with its `tags`: one of Occultism's possessed or wild ones, if
    /// they say so.
    pub fn joined(&self, id: u32, tags: &[String]) {
        if let Some(kind) = crate::spirits::possessed_kind(tags) {
            lock(&self.possessed).insert(id, kind);
        }
    }

    pub fn left(&self, id: u32) {
        lock(&self.possessed).remove(&id);
    }

    /// The possessed or wild creature the entity `id` is, if one.
    pub fn possessed(&self, id: u32) -> Option<&'static str> {
        lock(&self.possessed).get(&id).copied()
    }

    /// A player's ritual satchel asked to build from their preview of a pentacle. A player has at
    /// most `MAX_PENDING_BUILDS` waiting: more are dropped, as a client could send any number.
    pub fn queue_build(&self, player: u128, build: crate::pentacle_preview::Build) {
        let mut builds = lock(&self.builds);
        if builds.iter().filter(|(p, _)| *p == player).count() < MAX_PENDING_BUILDS {
            builds.push((player, build));
        }
    }

    /// A player left: their satchel builds that never got a tick go.
    pub fn drop_builds(&self, player: u128) {
        lock(&self.builds).retain(|(p, _)| *p != player);
    }

    /// After `player`'s tick, while their inventory can be changed: their satchel builds what is
    /// missing of the previewed pentacle from what they carry (`RitualSatchelItem.tryPlaceBlockForMatcher`):
    /// the place they clicked, or with the second satchel all of it. A glyph is drawn with its chalk,
    /// worn a use as a drawn glyph wears it; any other block placed from its item.
    pub fn take_builds(&self, host: &mut dyn ServerHost, player: u128) {
        use crate::pentacle_preview::{SATCHEL_T1, SATCHEL_T2};
        let builds: Vec<_> = {
            let mut all = lock(&self.builds);
            let (mine, others) = std::mem::take(&mut *all).into_iter().partition(|(p, _)| *p == player);
            *all = others;
            mine
        };
        let pentacles = self.pentacles(&*host);
        for (_, build) in builds {
            let Some(living) = game::living(&*host, player) else { return };
            let held = living.hand_item(&*host, Hand::Main).map(|s| id(&*host, &s).to_string()).unwrap_or_default();
            if held != if build.all { SATCHEL_T2 } else { SATCHEL_T1 } {
                continue;
            }
            let Some(pentacle) = pentacles.get(&build.pentacle) else { continue };
            let Some(me) = host.player(player) else { return };
            // The client says where: the server checks it as it checks a placed block. Only a player
            // who may build (not in adventure or spectator mode), and a clicked place within reach
            // that is a part of the pentacle as previewed, so the whole circle is next to them.
            if me.is_spectator(&*host) != Some(false) || me.may_build(&*host) != Some(true) {
                continue;
            }
            let entity = me.entity();
            let Some(eye) = entity.eye_position(&*host) else { continue };
            let parts = || pentacle.parts().iter().map(|(offset, _)| at(build.anchor, ritual::rotate(*offset, build.rotation)));
            if !ritual::in_satchel_reach(eye, build.target) || !parts().any(|pos| pos == build.target) {
                continue;
            }
            let (Some(level), yaw) = (entity.level(&*host), entity.rotation(&*host).map_or(0.0, |r| r[0])) else { continue };
            let Some(border) = level.world_border(&*host) else { continue };
            let creative = host.player(player).and_then(|p| p.is_creative(&*host)) == Some(true);
            let mut placed = 0;
            let mut wanted = None;
            for (offset, accept) in pentacle.parts() {
                let ritual::Accept::OneOf(blocks) = accept else { continue };
                if blocks.iter().all(|b| ritual::AIR.contains(&b.as_str())) {
                    continue;
                }
                let pos = at(build.anchor, ritual::rotate(*offset, build.rotation));
                if (!build.all && pos != build.target) || !ritual::inside_border(border, pos) {
                    continue;
                }
                let here = block(&*host, level, pos);
                if blocks.iter().any(|b| b == here) {
                    continue;
                }
                if !replaceable(here) {
                    if !build.all {
                        satchel_says(host, player, "block_at_position_not_air");
                    }
                    continue;
                }
                // The first accepted block the player has the item for: a glyph's chalk, else the block's item.
                let item_for = |b: &str| b.strip_prefix(GLYPH_PREFIX).map_or(b.to_string(), |colour| format!("occultism:chalk_{colour}"));
                let Some(items) = entity.capability(host, &ENTITY_ITEMS, ()) else { return };
                let size = items.size(host);
                let found = blocks.iter().find_map(|b| {
                    let item = item_for(b);
                    (0..size).find(|&i| items.resource(host, i).is_some_and(|r| r.to_stack(1).is_some_and(|s| id(&*host, &s) == item))).map(|i| (b.clone(), i))
                });
                let Some((wanted_block, slot)) = found else {
                    wanted = Some(());
                    continue;
                };
                let glyph = wanted_block.starts_with(GLYPH_PREFIX);
                let state = if glyph {
                    let below = [pos[0], pos[1] - 1, pos[2]];
                    if level.block_face_sturdy(&*host, below, BlockFace::Up, SupportType::Full) != Some(true) {
                        if !build.all {
                            satchel_says(host, player, "glyph_cannot_survive");
                        }
                        continue;
                    }
                    let sign = ritual::glyph_sign((game::roll(&self.rng) % 1000) as u32, None);
                    format!("{wanted_block}[facing={},segment_amount=1,sign={sign}]", ritual::glyph_facing(yaw))
                } else {
                    wanted_block.clone()
                };
                if !game::set_block(host, level, pos, &state) {
                    continue;
                }
                placed += 1;
                if !creative
                    && let Some(resource) = items.resource(host, slot)
                {
                    let taken = host.transaction(|host, tx| Ok::<u32, ()>(items.extract(host, tx, slot, &resource, 1))).unwrap_or(0);
                    // A chalk goes back a use more worn, until its last.
                    if glyph
                        && taken == 1
                        && let Some(mut chalk) = resource.to_stack(1)
                    {
                        let worn = chalk.damage() + 1;
                        if worn < CHALK_USES {
                            chalk.set_damage(worn);
                            let _ = game::give(host, player, chalk);
                        }
                    }
                }
                if !build.all {
                    break;
                }
            }
            if placed == 0 && wanted.is_some() {
                satchel_says(host, player, "no_valid_item_in_satchel");
            }
        }
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
        let _ = host.transaction(|host, tx| if crate::platform::edited(place.edit(host, tx, |s| set_spirit_name(s, &name))) { Ok(()) } else { Err(()) });
    }

    /// A running ritual's particles (`GoldenSacrificialBowlBlockEntity.tick`): while it waits for its
    /// sacrifice or item use, gray smoke around the bowl now and then; else the portal's swirl over
    /// it now and then, and every quarter second a spirit-fire flame circling the bowl it takes from
    /// next (`Ritual.markNextIngredient`), or once all are taken, the golden bowl.
    fn particles(&self, host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], r: &Ritual, state: &RitualState, waiting: bool) {
        let Some(keys) = crate::particles::keys() else { return };
        let centre = |p: [i32; 3]| [f64::from(p[0]) + 0.5, f64::from(p[1]) + 0.5, f64::from(p[2]) + 0.5];
        let now_and_then = game::roll(&self.rng) % 16 == 0;
        if waiting {
            if now_and_then {
                let c = centre(pos);
                crate::particles::send(host, level, keys.waiting, [c[0], c[1], c[2]], [1.0, 0.0, 1.0], 0.0, 9);
            }
            return;
        }
        if now_and_then {
            crate::particles::send(host, level, keys.portal, centre(pos), [1.0 / 3.0, 0.0, 1.0 / 3.0], 0.0, 5);
        }
        let Some(time) = level.game_time(&*host) else { return };
        if time % 5 != 0 {
            return;
        }
        let next = r.ingredients.get(state.consumed as usize).and_then(|ingredient| {
            let pentacles = self.pentacles(&*host);
            let bowls = self.bowls_near(&*host, level, pos, pentacles.get(&r.pentacle)?);
            let contents: Vec<Option<String>> = bowls.iter().map(|(_, i)| i.clone()).collect();
            ritual::bowl_for(ingredient, &contents).map(|b| bowls[b].0)
        });
        let c = centre(next.unwrap_or(pos));
        let angle = time as f64 * 0.05;
        let (sin, cos) = (angle.sin() * 0.3, angle.cos() * 0.3);
        crate::particles::send(host, level, keys.flame, [c[0] + cos, c[1] + 0.2 + cos, c[2] + sin], [0.0; 3], 0.003, 1);
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
                let waiting = !r.sacrifice.is_empty() && !state.sacrificed || !r.item_use.is_empty() && !state.item_used;
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
                if !interrupted && !finished {
                    self.particles(host, level, pos, r, &state, waiting);
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
                    let summoned = self.pick(&r.summon).and_then(|kind| self.summon(host, level, pos, kind, book.as_deref()));
                    // A spirit summoned to work takes up its job, and the caster gets its book.
                    if let (Some(spirit), Some(job)) = (summoned, &r.job) {
                        let at = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 0.5, f64::from(pos[2]) + 0.5];
                        let called = Called { spirit, job: job.clone(), caster, name: book.clone(), book: r.result.clone(), at, max_age: r.max_age };
                        lock(&self.called).push(called);
                    }
                    if let Some(who) = caster {
                        tell(host, who, "finished");
                    }
                }
                // The wild creatures come around the bowl; the activation item is spent.
                (Some(e), _) if e == ritual::SUMMON_WILD => {
                    entity.items[0] = None;
                    for _ in 0..r.summon_number {
                        if let Some(kind) = self.pick(&r.summon) {
                            self.summon_wild(host, level, pos, kind);
                        }
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
    ModContainer { slots: 1, faces: std::array::from_fn(|_| vec![0]), inputs: vec![0], outputs: vec![0], menu: None, ..Default::default() }
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
        let now = shown(&*host, entity.items.first().and_then(Option::as_ref), active);
        show(host, pos, &mut entity.nbt, now);
    }

    fn block_entity_update_tag(&self, event: &mut BlockEntityUpdateTagEvent<'_>) -> Option<Compound> {
        Some(shown_tag(event.entity))
    }

    fn block_entity_submit(&self, host: &dyn ClientHost, event: &mut BlockEntitySubmitEvent<'_>) {
        draw_shown(host, event.pos, event.entity, &mut *event.draw);
    }
}

/// What clients draw: the bowl's item id (empty without one) and whether a ritual is running.
fn shown(host: &dyn Host, item: Option<&ItemStack>, active: bool) -> Compound {
    let mut tag = Compound::new();
    tag.insert("core".into(), Tag::String(item.map_or("", |c| id(host, c)).into()));
    tag.insert("active".into(), Tag::Byte(active as i8));
    tag
}

/// Keeps `shown` in the block entity's data, and tells clients when it changes.
fn show(host: &mut dyn ServerHost, pos: [i32; 3], nbt: &mut Compound, now: Compound) {
    if nbt.get(&"shown".into()) != Some(&Tag::Compound(now.clone())) {
        nbt.insert("shown".into(), Tag::Compound(now));
        host.block_entity_sync(pos);
    }
}

/// The `shown` field a client copy of a bowl has, for its update tag.
fn shown_tag(entity: &ModBlockEntity) -> Compound {
    let mut tag = Compound::new();
    if let Some(shown) = entity.nbt.get(&"shown".into()) {
        tag.insert("shown".into(), shown.clone());
    }
    tag
}

/// The item in a bowl, from its `shown` field: turning slowly, rising and faster in a ritual.
fn draw_shown(host: &dyn ClientHost, pos: [i32; 3], entity: &ModBlockEntity, draw: &mut dyn BlockEntityDrawContext) {
    let Some(Tag::Compound(shown)) = entity.nbt.get(&"shown".into()) else { return };
    let Some(core) = game::text(shown, "core").filter(|c| !c.is_empty()) else { return };
    let Some(stack) = game::stack(&*host, &core, 1) else { return };
    let active = matches!(shown.get(&"active".into()), Some(Tag::Byte(1)));
    // The bowl's own light is dark: light the item as the air above it.
    // A client reads no level light on Nightly 2026-11-18: then full sky and the block's own light.
    let above = [pos[0], pos[1] + 1, pos[2]];
    let level = host.level(host.dimension());
    let read = |layer| level.and_then(|l| l.brightness(host, layer, above)).map(|b| b.clamp(0, 15) as u32);
    let light = draw.light();
    let lit = match (read(LightLayer::Sky), read(LightLayer::Block)) {
        (None, None) => 15 * 16 | (light & 0xFF00),
        (sky, block) => sky.unwrap_or(0) * 16 | (block.unwrap_or(0) * 16) << 8,
    };
    draw.set_light(lit);
    let t = draw.game_time() as f32 + draw.partial();
    let (spin, bob) = if active { (6.0, 0.06) } else { (1.5, 0.02) };
    let (height, size) = if active { (0.55, 0.4) } else { (0.3, 0.4) };
    draw.push();
    draw.translate([0.5, height + bob * (t * 0.15).sin(), 0.5]);
    draw.rotate_about([0.0, 1.0, 0.0], (t * spin) % 360.0, [0.0, 0.0, 0.0]);
    draw.scale([size, size, size]);
    draw.item(&stack, ItemDisplayContext::Fixed, None, 0);
    draw.pop();
    draw.set_light(light);
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
    let activation = crate::crushing::ingredient(host, &j["activation_item"])?;
    let ingredients = j["ingredients"].as_array()?.iter().map(|i| crate::crushing::ingredient(host, i)).collect::<Option<Vec<_>>>()?;
    Some(Ritual {
        id: r.id.clone(),
        activation,
        duration: j["duration"].as_u64()?.max(1) as u32,
        ingredients,
        pentacle: j["pentacle_id"].as_str()?.to_string(),
        // A crusher's summoning has none: the spirit is all it gives.
        result: if j["result"].is_null() { String::new() } else { crate::crushing::result_item(host, &j["result"])? },
        count: j["result"]["count"].as_u64().unwrap_or(1).clamp(1, 64) as u32,
        effect: j["ritual_type"]
            .as_str()
            .filter(|t| [ritual::RECALL_STORAGE, ritual::REPAIR, ritual::SPIRIT_NAME, ritual::SUMMON, ritual::SUMMON_JOB, ritual::SUMMON_WILD].contains(t))
            .map(str::to_string),
        sacrifice: j["sacrifice"]["kinds"].as_array().into_iter().flatten().filter_map(|k| k.as_str().map(str::to_string)).collect(),
        sacrifice_name: j["sacrifice"]["display_name"].as_str().map(str::to_string),
        summon: match &j["summon"] {
            serde_json::Value::String(kind) => vec![kind.clone()],
            kinds => kinds.as_array().into_iter().flatten().filter_map(|k| k.as_str().map(str::to_string)).collect(),
        },
        summon_number: j["summon_number"].as_u64().unwrap_or(1).clamp(1, 64) as u32,
        job: j["job"].as_str().map(str::to_string),
        max_age: j["max_age"].as_u64().map(|s| s.min(u64::from(u32::MAX)) as u32),
        item_use: if j["item_to_use"].is_null() { Vec::new() } else { crate::crushing::ingredient(host, &j["item_to_use"])? },
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
