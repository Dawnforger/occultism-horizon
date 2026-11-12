// Spirit fire and Demon's Dream for Horizon: the glue between the game's hooks and the rules
// (fire.rs). Datura seeds drop from grass; flint and steel used by dropped Demon's Dream fruit
// lights spirit fire, using the fruit up; an item thrown into the fire turns by Occultism's
// `occultism:spirit_fire` recipes, read from the content mod's data.
//
// Horizon has no "items in this box" query, so a lighting is remembered for a few ticks while the
// item entities' own ticks say where they are; nor a block by name for a mod, so the fire is set by
// `/setblock`. The block-inside hook has no game to hand, so what touched the fire is handled at the
// end of the server tick. The crop is the seeds' own block, on wheat, but the seeds plant wheat as
// their template does: a sprout they planted is made datura at the end of the tick.

use crate::fire::{self, FireRecipe};
use horizon_client::mod_api::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// How many server ticks a lighting waits for the fruit's item ticks.
const LIGHTING_TICKS: u32 = 3;

/// A flint and steel used on a block, waiting to learn whether Demon's Dream lies around it.
struct Lighting {
    dimension: String,
    clicked: [i32; 3],
    face: [i32; 3],
    ticks: u32,
    fruit: Vec<u32>,
}

#[derive(Default)]
pub struct SpiritFire {
    lightings: Mutex<Vec<Lighting>>,
    /// Where datura seeds were just planted: the level and the sprout's position.
    plantings: Mutex<Vec<(String, [i32; 3])>>,
    /// Item entities that touched a mod block this tick: their level, id and the block.
    touched: Mutex<Vec<(String, u32, [i32; 3])>>,
    /// The recipes, resolved once per recipe table (as the crushing ones are).
    recipes: Mutex<(Vec<usize>, Arc<[FireRecipe]>)>,
    starters: Mutex<Option<Vec<String>>>,
    rng: AtomicU64,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// An item entity's item id, from its data (`Item: {id, count}`).
fn item_of(host: &mut dyn Host, id: u32) -> Option<(String, u32)> {
    let entity = host.entity(id)?;
    let nbt = host.entity_nbt(entity)?;
    let Some(Tag::Compound(item)) = nbt.get(&"Item".into()) else { return None };
    let Some(Tag::String(name)) = item.get(&"id".into()) else { return None };
    let count = match item.get(&"count".into()) {
        Some(Tag::Int(n)) => (*n).max(1) as u32,
        Some(Tag::Byte(n)) => i32::from(*n).max(1) as u32,
        _ => 1,
    };
    Some((name.to_string_lossy(), count))
}

impl SpiritFire {
    fn roll(&self) -> u64 {
        // xorshift, seeded from the clock once.
        let mut x = self.rng.load(Ordering::Relaxed);
        if x == 0 {
            x = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64) | 1;
        }
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng.store(x, Ordering::Relaxed);
        x
    }

    fn recipes(&self, host: &dyn Host) -> Arc<[FireRecipe]> {
        let table = host.recipes_of_type(fire::RECIPE_TYPE);
        let key: Vec<usize> = table.iter().map(|r| Arc::as_ptr(&r.json) as usize).collect();
        let mut cache = lock(&self.recipes);
        if cache.0 != key {
            let parsed: Vec<FireRecipe> = table
                .iter()
                .filter_map(|r| {
                    let inputs = host.recipe_ingredient(&r.json["ingredient"])?.order;
                    let result = r.json["result"]["id"].as_str()?.to_string();
                    Some(FireRecipe { inputs, result })
                })
                .collect();
            *cache = (key, parsed.into());
        }
        cache.1.clone()
    }

    fn is_starter(&self, host: &dyn Host, item: &str) -> bool {
        let mut starters = lock(&self.starters);
        let list = starters.get_or_insert_with(|| {
            host.recipe_ingredient(&serde_json::json!(fire::STARTERS)).map(|i| i.order).unwrap_or_default()
        });
        list.iter().any(|s| s == item)
    }

    /// A datura seed in grass's drops, now and then.
    pub fn drops(&self, context: &NativeLootDrops<'_>, drops: &mut Vec<ItemStack>) {
        if context.block.is_none() {
            return;
        }
        let sheared = context.tool.is_some_and(|t| &*t.item == "minecraft:shears");
        if fire::drops_seed(context.table, sheared, self.roll()) {
            drops.push(ItemStack { item: fire::DATURA_SEEDS.into(), count: 1, damage: 0, extra: None });
        }
    }

    /// A player used `held` on the block at `clicked`: an igniter starts a lighting. The vanilla
    /// fire it lights is what spirit fire replaces.
    pub fn right_click(&self, dimension: &str, clicked: [i32; 3], face: [i32; 3], held: &str) {
        if held == fire::DATURA_SEEDS {
            lock(&self.plantings).push((dimension.to_string(), fire::beside(clicked, face)));
            return;
        }
        if !fire::IGNITERS.contains(&held) {
            return;
        }
        let mut lightings = lock(&self.lightings);
        if !lightings.iter().any(|l| l.dimension == dimension && l.clicked == clicked) {
            lightings.push(Lighting { dimension: dimension.to_string(), clicked, face, ticks: LIGHTING_TICKS, fruit: Vec::new() });
        }
    }

    /// Whether item ticks are wanted: only while a lighting waits.
    pub fn listening(&self) -> bool {
        !lock(&self.lightings).is_empty()
    }

    /// An item entity ticks: Demon's Dream near a waiting lighting is taken note of.
    pub fn item_tick(&self, host: &mut dyn Host, dimension: &str, id: u32) {
        let Some(entity) = host.entity(id) else { return };
        let Some(at) = host.entity_position(entity) else { return };
        let waiting: Vec<usize> = lock(&self.lightings)
            .iter()
            .enumerate()
            .filter(|(_, l)| l.dimension == dimension && fire::near(l.clicked, at))
            .map(|(i, _)| i)
            .collect();
        if waiting.is_empty() {
            return;
        }
        let Some((item, _)) = item_of(host, id) else { return };
        if !self.is_starter(&*host, &item) {
            return;
        }
        let mut lightings = lock(&self.lightings);
        for i in waiting {
            if let Some(l) = lightings.get_mut(i)
                && !l.fruit.contains(&id)
            {
                l.fruit.push(id);
            }
        }
    }

    /// An entity touched a mod block: an item may be in spirit fire.
    pub fn inside(&self, dimension: &str, entity: i32, kind: &str, position: [i32; 3]) {
        if kind == "minecraft:item"
            && let Ok(id) = u32::try_from(entity)
        {
            let mut touched = lock(&self.touched);
            if !touched.iter().any(|(d, e, _)| *e == id && d == dimension) {
                touched.push((dimension.to_string(), id, position));
            }
        }
    }

    /// The end of a server tick: lightings that found their fruit light, the rest wait or lapse;
    /// items in spirit fire turn.
    pub fn tick(&self, host: &mut dyn Host) {
        let done: Vec<Lighting> = {
            let mut lightings = lock(&self.lightings);
            for l in lightings.iter_mut() {
                l.ticks = l.ticks.saturating_sub(1);
            }
            let (done, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut *lightings).into_iter().partition(|l| !l.fruit.is_empty() || l.ticks == 0);
            *lightings = waiting;
            done
        };
        for l in done.into_iter().filter(|l| !l.fruit.is_empty()) {
            self.light(host, &l);
        }
        for (dimension, [x, y, z]) in std::mem::take(&mut *lock(&self.plantings)) {
            let Some(level) = host.level(&dimension) else { continue };
            if host.level_block_state(level, [x, y, z]).is_some_and(|s| fire::is_sprout(&s)) {
                host.run_command(&format!("execute in {dimension} run setblock {x} {y} {z} {} replace", fire::DATURA_SEEDS));
            }
        }
        let touched = std::mem::take(&mut *lock(&self.touched));
        if touched.is_empty() {
            return;
        }
        let recipes = self.recipes(&*host);
        for (dimension, id, pos) in touched {
            let Some(level) = host.level(&dimension) else { continue };
            if !host.level_block_state(level, pos).is_some_and(|s| s.starts_with(fire::SPIRIT_FIRE)) {
                continue;
            }
            let Some((item, count)) = item_of(host, id) else { continue };
            if let Some(result) = fire::convert(&recipes, &item) {
                let stack = ItemStack { item: result.into(), count, damage: 0, extra: None };
                host.set_item_entity(&dimension, id, stack);
            }
        }
    }

    /// Spirit fire beside the clicked face, the fruit around it used up (all of it, as Occultism's).
    fn light(&self, host: &mut dyn Host, l: &Lighting) {
        let at = fire::beside(l.clicked, l.face);
        let Some(level) = host.level(&l.dimension) else { return };
        if !host.level_block_state(level, at).is_some_and(|s| fire::may_light(&s)) {
            return;
        }
        for id in &l.fruit {
            host.set_item_entity(&l.dimension, *id, ItemStack { item: fire::DATURA_SEEDS.into(), count: 0, damage: 0, extra: None });
        }
        let [x, y, z] = at;
        host.run_command(&format!("execute in {} run setblock {x} {y} {z} {} replace", l.dimension, fire::SPIRIT_FIRE));
    }
}
