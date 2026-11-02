// Occultism's ritual circles for Horizon: the glue between the game's hooks and the ritual rules
// (ritual.rs). Chalks draw glyphs (each chalk is its glyph's block item: Horizon's native API
// can't place a block by name, so vanilla's placement draws it and `state_for_placement` picks
// its sign and facing); sacrificial bowls hold one item each; a golden bowl checks its pentacle
// and ingredients, then runs the ritual and leaves its result in the bowl. Players with the
// Third Eye (or the goggles) find raw iesnium in the silver ore they mine.

use crate::ritual::{self, Accept, Pentacle, Ritual, RitualState, Step};
use horizon_client::mod_api::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const BOWL: &str = "occultism:sacrificial_bowl";
pub const GOLDEN_BOWL: &str = "occultism:golden_sacrificial_bowl";
pub const GLYPH_PREFIX: &str = "occultism:chalk_glyph_";
pub const RITUAL_TYPE: &str = "occultism:ritual";
pub const PENTACLES: &str = "data/occultism/pentacles.json";
pub const THIRD_EYE: &str = "occultism:third_eye";
pub const GOGGLES: &str = "occultism:otherworld_goggles";
pub const RAW_SILVER: &str = "occultism:raw_silver";
pub const RAW_IESNIUM: &str = "occultism:raw_iesnium";
pub const SILVER_ORE_TABLES: [&str; 2] = ["occultism:blocks/silver_ore", "occultism:blocks/silver_ore_deepslate"];
/// The armour slot the goggles are worn in (`Inventory`'s numbers: 103 is the head).
pub const HEAD_SLOT: i32 = 103;
/// How long after its last tick a player still counts as having the Third Eye or the goggles on.
const SEEN_FOR: Duration = Duration::from_secs(2);
/// Calls for the same player and bowl closer than this are one click (one per hand).
const ONE_CLICK: Duration = Duration::from_millis(150);

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
    third_eye: Mutex<HashMap<u128, Instant>>,
    goggles: Mutex<HashMap<u128, Instant>>,
    /// The last bowl each player used: Horizon asks `right_click_block` once per hand, so the
    /// off hand's call right after the main hand's is the same click.
    last_click: Mutex<HashMap<u128, ([i32; 3], Instant)>>,
    rng: AtomicU64,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn at(pos: [i32; 3], off: [i32; 3]) -> [i32; 3] {
    [pos[0] + off[0], pos[1] + off[1], pos[2] + off[2]]
}

fn tell(host: &mut dyn Host, uuid: u128, key: &str) {
    host.tell(uuid, Text::translate(&format!("ritual.occultism.{key}"), &[]));
}

fn one(item: &str) -> ItemStack {
    ItemStack { item: item.into(), count: 1, damage: 0, extra: None }
}

impl Rites {
    /// A random number for glyph signs and iesnium (xorshift; seeded from the clock once).
    fn roll(&self) -> u64 {
        let mut x = self.rng.load(Ordering::Relaxed);
        if x == 0 {
            x = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0x9E37_79B9_7F4A_7C15, |d| d.as_nanos() as u64)
                | 1;
        }
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng.store(x, Ordering::Relaxed);
        x
    }

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
        let parsed = Arc::new(host.data(PENTACLES).map(|b| parse_pentacles(&b)).unwrap_or_default());
        *cache = Some(parsed.clone());
        parsed
    }

    /// The sacrificial bowls around a golden bowl that count for `pentacle`'s rituals, and what
    /// each holds (`Ritual.getSacrificialBowls`), nearest first.
    fn bowls_near(&self, host: &dyn Host, dim: &str, pos: [i32; 3], pentacle: &Pentacle) -> Vec<([i32; 3], Option<String>)> {
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
        out.retain(|(b, _)| host.block(*b) == BOWL);
        out.sort_by_key(|(b, _)| (b[0] - pos[0]).abs() + (b[1] - pos[1]).abs() + (b[2] - pos[2]).abs());
        out
    }

    /// A right click on a chalk glyph, a sacrificial bowl or a golden bowl.
    pub fn right_click(&self, host: &mut dyn Host, pos: [i32; 3]) -> Flow {
        let block = host.block(pos);
        if block.starts_with(GLYPH_PREFIX) {
            // The same chalk over its own glyph draws the next sign, and isn't used up.
            if host.held().is_some_and(|(item, _, _)| item == block) {
                let current = host.block_property(pos, "sign").and_then(|s| s.parse().ok());
                host.set_block_property(pos, "sign", &ritual::glyph_sign(0, current).to_string());
                return Flow::Cancel;
            }
            return Flow::Continue;
        }
        if block != BOWL && block != GOLDEN_BOWL {
            return Flow::Continue;
        }
        let Some(player) = host.acting_player() else { return Flow::Continue };
        {
            let mut last = lock(&self.last_click);
            if last.get(&player.uuid).is_some_and(|(p, t)| *p == pos && t.elapsed() < ONE_CLICK) {
                return Flow::Cancel;
            }
            last.insert(player.uuid, (pos, Instant::now()));
        }
        let key = (player.dimension.clone(), pos);
        if block == GOLDEN_BOWL && lock(&self.running).contains_key(&key) {
            tell(host, player.uuid, "busy");
            return Flow::Cancel;
        }
        let content = lock(&self.bowls).get(&key).cloned().flatten();
        if content.is_some() {
            // An empty or a full hand takes the bowl's item back.
            if let Some(stack) = host.extract_item(&player.dimension, pos, Direction::Up, 0, 64) {
                if let Some(left) = host.give_stack(player.uuid, stack) {
                    host.insert_item(pos, Direction::Up, left);
                }
            }
            lock(&self.bowls).insert(key, None);
            return Flow::Cancel;
        }
        let Some((item, _, plain)) = host.held() else { return Flow::Continue };
        if !plain {
            return Flow::Continue;
        }
        if block == GOLDEN_BOWL {
            let rituals = self.rituals(&*host);
            let candidates: Vec<&Ritual> = rituals.iter().filter(|r| r.activation.contains(&item)).collect();
            if !candidates.is_empty() {
                self.start(host, pos, &player, &item, &candidates);
                return Flow::Cancel;
            }
        }
        if host.take_items(&item, 1) == 1 {
            match host.insert_item(pos, Direction::Up, one(&item)) {
                Some(back) => {
                    host.give_stack(player.uuid, back);
                }
                None => {
                    lock(&self.bowls).insert(key, Some(item));
                }
            }
        }
        Flow::Cancel
    }

    /// The caster offers an activation item to a golden bowl: the first ritual whose pentacle is
    /// drawn and whose ingredients are in the bowls starts on the bowl's next tick.
    fn start(&self, host: &mut dyn Host, pos: [i32; 3], player: &NativeServerPlayer, item: &str, candidates: &[&Ritual]) {
        let pentacles = self.pentacles(&*host);
        let circled: Vec<&&Ritual> = {
            let read = |off: [i32; 3]| host.block(at(pos, off));
            candidates
                .iter()
                .filter(|r| pentacles.get(&r.pentacle).is_some_and(|p| p.matches(&read)))
                .collect()
        };
        if circled.is_empty() {
            tell(host, player.uuid, "no_pentacle");
            return;
        }
        let ready = circled.into_iter().find(|r| {
            let p = &pentacles[&r.pentacle];
            let items: Vec<String> = self.bowls_near(&*host, &player.dimension, pos, p).into_iter().filter_map(|(_, i)| i).collect();
            ritual::ingredients_present(&r.ingredients, &items)
        });
        let Some(ritual) = ready else {
            tell(host, player.uuid, "no_ingredients");
            return;
        };
        if host.take_items(item, 1) != 1 {
            return;
        }
        if let Some(back) = host.insert_item(pos, Direction::Up, one(item)) {
            host.give_stack(player.uuid, back);
            return;
        }
        let key = (player.dimension.clone(), pos);
        lock(&self.bowls).insert(key.clone(), Some(item.to_string()));
        lock(&self.pending).insert(key, (ritual.id.clone(), player.uuid));
        tell(host, player.uuid, "started");
    }

    /// A sacrificial bowl's tick: remember what it holds.
    pub fn bowl_tick(&self, host: &dyn Host, pos: [i32; 3], entity: &mut ModBlockEntity) {
        let item = entity.items.first().cloned().flatten().map(|s| s.item.to_string());
        let key = (host.dimension().to_string(), pos);
        let mut bowls = lock(&self.bowls);
        if bowls.get(&key) != Some(&item) {
            bowls.insert(key, item);
        }
    }

    /// A golden bowl's tick: start a pending ritual, or run the one under way.
    pub fn golden_tick(&self, host: &mut dyn Host, pos: [i32; 3], entity: &mut ModBlockEntity) {
        let dim = host.dimension().to_string();
        let key = (dim.clone(), pos);
        let item = entity.items.first().cloned().flatten().map(|s| s.item.to_string());
        lock(&self.bowls).insert(key.clone(), item.clone());
        let (mut state, caster) = read_ritual(&entity.nbt);
        let before = (state.clone(), caster);
        let mut caster = caster;
        if state.ritual.is_none() {
            if let Some((id, who)) = lock(&self.pending).remove(&key) {
                state = RitualState { ritual: Some(id), ..Default::default() };
                caster = Some(who);
            }
        }
        let rituals = self.rituals(&*host);
        let current = state.ritual.as_ref().and_then(|id| rituals.iter().find(|r| &r.id == id));
        let mut interrupted = state.ritual.is_some() && current.is_none();
        let mut finished = false;
        if let Some(r) = current {
            let pentacles = self.pentacles(&*host);
            if !item.as_ref().is_some_and(|i| r.activation.contains(i)) {
                interrupted = true; // the activation item was taken out
            } else if state.ticks % ritual::TICKS_PER_SECOND == 0 && {
                let read = |off: [i32; 3]| host.block(at(pos, off));
                !pentacles.get(&r.pentacle).is_some_and(|p| p.matches(&read))
            } {
                interrupted = true; // the circle was broken
            } else {
                match ritual::advance(&mut state, r) {
                    Step::Wait => {}
                    Step::Take(i) => {
                        let bowls = pentacles
                            .get(&r.pentacle)
                            .map(|p| self.bowls_near(&*host, &dim, pos, p))
                            .unwrap_or_default();
                        let contents: Vec<Option<String>> = bowls.iter().map(|(_, i)| i.clone()).collect();
                        let took = ritual::bowl_for(&r.ingredients[i], &contents)
                            .and_then(|b| host.extract_item(&dim, bowls[b].0, Direction::Up, 0, 1).map(|_| bowls[b].0));
                        match took {
                            Some(b) => {
                                ritual::taken(&mut state);
                                lock(&self.bowls).insert((dim.clone(), b), None);
                            }
                            None => interrupted = true,
                        }
                    }
                    Step::Done => finished = true,
                }
            }
        }
        if finished {
            let r = current.expect("a finished ritual is known");
            if entity.items.is_empty() {
                entity.items.resize(1, None);
            }
            entity.items[0] = Some(ItemStack { item: r.result.as_str().into(), count: r.count, damage: 0, extra: None });
            if let Some(who) = caster {
                tell(host, who, "finished");
            }
        }
        if interrupted {
            if let Some(who) = caster {
                tell(host, who, "interrupted");
            }
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
    }

    /// The sign and facing of a glyph a chalk is drawing.
    pub fn glyph_state(&self, level: &dyn LevelHost, state: BlockStateId, context: &PlacementContext) -> Option<BlockStateId> {
        if !context.item.starts_with(GLYPH_PREFIX) {
            return None;
        }
        let sign = ritual::glyph_sign((self.roll() % 1000) as u32, None);
        let state = level.state_with_property(state, "sign", &sign.to_string())?;
        level.state_with_property(state, "facing", ritual::glyph_facing(context.yaw))
    }

    pub fn saw_third_eye(&self, uuid: u128) {
        lock(&self.third_eye).insert(uuid, Instant::now());
    }

    pub fn saw_goggles(&self, uuid: u128) {
        lock(&self.goggles).insert(uuid, Instant::now());
    }

    /// Raw iesnium in a player's silver ore drops, by chance, with the Third Eye or the goggles.
    pub fn silver_ore_drops(&self, context: &NativeLootDrops<'_>, drops: &mut Vec<ItemStack>) {
        if !SILVER_ORE_TABLES.contains(&context.table) || context.used || context.explosion.is_some() {
            return;
        }
        // Silk touch takes the ore whole: iesnium only shows when it breaks to raw silver.
        if !drops.iter().any(|s| &*s.item == RAW_SILVER) {
            return;
        }
        let Some(uuid) = context.player else { return };
        let seen = |m: &Mutex<HashMap<u128, Instant>>| lock(m).get(&uuid).is_some_and(|t| t.elapsed() < SEEN_FOR);
        let chance = ritual::iesnium_chance(seen(&self.third_eye), seen(&self.goggles));
        if chance > 0.0 && (self.roll() % 1_000_000) as f64 / 1_000_000.0 < chance {
            drops.push(one(RAW_IESNIUM));
        }
    }
}

/// An `occultism:ritual` recipe, its items resolved by the game.
fn parse_ritual(host: &dyn Host, r: &CustomRecipe) -> Option<Ritual> {
    let j = &*r.json;
    let activation = host.recipe_ingredient(&j["activation_item"])?.1;
    let ingredients = j["ingredients"]
        .as_array()?
        .iter()
        .map(|i| host.recipe_ingredient(i).map(|ing| ing.1))
        .collect::<Option<Vec<_>>>()?;
    Some(Ritual {
        id: r.id.clone(),
        activation,
        duration: j["duration"].as_u64()?.max(1) as u32,
        ingredients,
        pentacle: j["pentacle_id"].as_str()?.to_string(),
        result: host.recipe_result(&serde_json::json!({"id": j["result"]["id"], "count": 1}))?.item.to_string(),
        count: j["result"]["count"].as_u64().unwrap_or(1).clamp(1, 64) as u32,
    })
}

/// The content mod's `pentacles.json`: `{id: {pattern, keys: {key: "any" | [blocks]}}}`.
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
                    serde_json::Value::Array(list) => {
                        Accept::OneOf(list.iter().filter_map(|b| b.as_str().map(str::to_string)).collect())
                    }
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
    let int = |k: &str| match nbt.get(&k.into()) {
        Some(Tag::Int(v)) => (*v).max(0) as u32,
        _ => 0,
    };
    let text = |k: &str| match nbt.get(&k.into()) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    };
    let state = RitualState { ritual: text("ritual"), ticks: int("ticks"), consumed: int("consumed") };
    let caster = text("caster").and_then(|s| u128::from_str_radix(&s, 16).ok());
    (state, caster)
}

fn write_ritual(nbt: &mut Compound, state: &RitualState, caster: Option<u128>) {
    match &state.ritual {
        Some(id) => {
            nbt.insert("ritual".into(), Tag::String(id.as_str().into()));
            nbt.insert("ticks".into(), Tag::Int(state.ticks as i32));
            nbt.insert("consumed".into(), Tag::Int(state.consumed as i32));
        }
        None => {
            for k in ["ritual", "ticks", "consumed"] {
                nbt.remove(&k.into());
            }
        }
    }
    match caster {
        Some(uuid) => nbt.insert("caster".into(), Tag::String(format!("{uuid:032x}").as_str().into())),
        None => nbt.remove(&"caster".into()),
    };
}
