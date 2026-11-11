// The dimensional mineshaft for Horizon: the glue between the game's hooks and the mineshaft's
// rules (mining.rs). A block entity with a lamp slot and nine outputs; its miner spirit mines on a
// timer from Occultism's `occultism:miner` recipes, read from the content mod's data, their tags
// resolved by the game so a modpack's ores join in. Results go into the container under it when
// there is one, else into its outputs; a redstone signal stops it, as Occultism's.

use crate::mining::{self, Miner, Shaft, Step, OUTPUTS, SLOT_COUNT, SLOT_LAMP};
use horizon_client::mod_api::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub const MINESHAFT: &str = "occultism:dimensional_mineshaft";
pub const MENU: &str = "occultism:dimensional_mineshaft";
const RECIPE_TYPE: &str = "occultism:miner";
const MINER_COMPONENT: &str = "occultism:miner";
/// Data slots: ticks done, ticks per operation, 1 when the outputs are full.
pub const DATA_COUNT: usize = 3;

/// The five miners and their durability (Occultism's config defaults, also in the content mod).
const DURABILITY: [(&str, u32); 5] = [
    ("occultism:miner_foliot_unspecialized", 511),
    ("occultism:miner_djinni_ores", 1023),
    ("occultism:miner_afrit_deeps", 2047),
    ("occultism:miner_marid_master", 4095),
    ("occultism:miner_ancient_eldritch", 8191),
];

/// One miner recipe, resolved: the lamps it is for, what it yields and its weight.
struct Dig {
    miners: Vec<String>,
    result: String,
    count: u32,
    weight: u32,
}

#[derive(Default)]
pub struct Mineshafts {
    /// The recipes, resolved once per recipe table (as the crushing ones are).
    recipes: Mutex<(Vec<usize>, Arc<[Dig]>)>,
    rng: AtomicU64,
}

fn number(nbt: &Compound, key: &str) -> u32 {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => (*v).max(0) as u32,
        _ => 0,
    }
}

/// The miner a lamp holds: its `occultism:miner` component, or Occultism's defaults for a lamp of
/// one of the five miners without one.
pub fn miner_of(stack: &ItemStack) -> Option<Miner> {
    let component = stack.extra.as_ref().and_then(|e| e.other.iter().find(|(k, _)| k == MINER_COMPONENT));
    if let Some((_, Tag::Compound(c))) = component {
        let n = |k: &str| match c.get(&k.into()) {
            Some(Tag::Int(v)) => Some((*v).max(1) as u32),
            _ => None,
        };
        return Some(Miner {
            ticks: n("ticks").unwrap_or(mining::DEFAULT_MINER.ticks),
            rolls: n("rolls").unwrap_or(1),
            multiplier: n("multiplier").unwrap_or(1),
        });
    }
    DURABILITY.iter().any(|(id, _)| **id == *stack.item).then(|| miner_defaults(&stack.item))
}

/// A miner's numbers when its lamp carries none: Occultism's per-miner defaults.
fn miner_defaults(item: &str) -> Miner {
    let ticks = match item {
        "occultism:miner_djinni_ores" => 300,
        "occultism:miner_afrit_deeps" => 200,
        "occultism:miner_marid_master" => 100,
        "occultism:miner_ancient_eldritch" => 15,
        _ => 400,
    };
    Miner { ticks, rolls: 1, multiplier: if item == "occultism:miner_ancient_eldritch" { 9 } else { 1 } }
}

pub fn is_miner(stack: &ItemStack) -> bool {
    miner_of(stack).is_some()
}

impl Mineshafts {
    fn recipes(&self, host: &dyn Host) -> Arc<[Dig]> {
        let table = host.recipes_of_type(RECIPE_TYPE);
        let key: Vec<usize> = table.iter().map(|r| Arc::as_ptr(&r.json) as usize).collect();
        let mut cache = self.recipes.lock().unwrap_or_else(|e| e.into_inner());
        if cache.0 != key {
            let parsed: Vec<Dig> = table.iter().filter_map(|r| parse(host, &r.json)).collect();
            *cache = (key, parsed.into());
        }
        cache.1.clone()
    }

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

    /// A mineshaft's tick: its timer, an operation's results when one finishes, its lamp's wear.
    pub fn tick(&self, host: &mut dyn Host, pos: [i32; 3], entity: &mut ModBlockEntity) {
        if entity.items.len() < SLOT_COUNT {
            entity.items.resize(SLOT_COUNT, None);
        }
        let mut shaft = Shaft { left: number(&entity.nbt, "left"), of: number(&entity.nbt, "of") };
        let before = shaft;
        let lamp = entity.items[SLOT_LAMP].clone();
        let powered = host.neighbor_signal(host.dimension(), pos) > 0;
        let miner = if powered { None } else { lamp.as_ref().and_then(miner_of) };
        let mut full = false;
        if step(&mut shaft, miner) == Step::Mine
            && let (Some(miner), Some(lamp)) = (miner, lamp)
        {
            let recipes = self.recipes(&*host);
            let digs: Vec<&Dig> = recipes.iter().filter(|d| d.miners.iter().any(|m| **m == *lamp.item)).collect();
            let weights: Vec<u32> = digs.iter().map(|d| d.weight).collect();
            let rolls: Vec<usize> = (0..miner.rolls).filter_map(|_| mining::pick(&weights, self.roll())).collect();
            let counts: Vec<u32> = digs.iter().map(|d| d.count).collect();
            let batch = mining::batch(&rolls, &counts, miner);
            // Results go to the container below first, so the outputs fill only once it is full (or
            // there is none). With no empty output for each result, wait, as a full altar does.
            if !self.room(entity, batch.len()) {
                shaft.left = 1;
                full = true;
            } else {
                for (i, count) in batch {
                    let stack = ItemStack { item: digs[i].result.as_str().into(), count, damage: 0, extra: None };
                    self.deliver(host, pos, entity, stack);
                }
                let max = DURABILITY.iter().find(|(id, _)| **id == *lamp.item).map_or(0, |(_, d)| *d);
                entity.items[SLOT_LAMP] = mining::wear(lamp.damage, max).map(|damage| ItemStack { damage, ..lamp });
            }
        }
        if shaft != before {
            entity.nbt.insert("left".into(), Tag::Int(shaft.left as i32));
            entity.nbt.insert("of".into(), Tag::Int(shaft.of as i32));
        }
        let done = if shaft.of == 0 { 0 } else { shaft.of - shaft.left };
        let data = vec![done.min(32_767) as i32, shaft.of.min(32_767) as i32, i32::from(full)];
        if entity.data != data {
            entity.data = data;
        }
    }

    /// Whether the outputs have room for `kinds` new stacks (an empty slot each).
    fn room(&self, entity: &ModBlockEntity, kinds: usize) -> bool {
        OUTPUTS.filter(|i| entity.items.get(*i).is_none_or(Option::is_none)).count() >= kinds
    }

    /// A result into the container under the mineshaft, else onto its outputs.
    fn deliver(&self, host: &mut dyn Host, pos: [i32; 3], entity: &mut ModBlockEntity, stack: ItemStack) {
        let below = [pos[0], pos[1] - 1, pos[2]];
        let Some(mut left) = host.insert_item(below, Direction::Up, stack) else { return };
        let max = host.max_stack_size(&left.item).unwrap_or(64);
        for i in OUTPUTS {
            match &mut entity.items[i] {
                Some(s) if s.item == left.item && s.extra.is_none() && s.count < max => {
                    let n = (max - s.count).min(left.count);
                    s.count += n;
                    left.count -= n;
                }
                slot @ None => {
                    let n = left.count.min(max);
                    *slot = Some(ItemStack { count: n, ..left.clone() });
                    left.count -= n;
                }
                _ => {}
            }
            if left.count == 0 {
                return;
            }
        }
    }
}

fn step(shaft: &mut Shaft, miner: Option<Miner>) -> Step {
    mining::step(shaft, miner)
}

/// An `occultism:miner` recipe: `ingredient` (a miner tag), `result` a weighted tag (its first
/// item, as Occultism picks) or a weighted item.
fn parse(host: &dyn Host, j: &serde_json::Value) -> Option<Dig> {
    let miners = host.recipe_ingredient(&j["ingredient"])?.order;
    let r = &j["result"];
    let weight = r["weight"].as_u64()?.min(u64::from(u32::MAX)) as u32;
    let (result, count) = match r["type"].as_str()? {
        "occultism:weighted_tag" => {
            let tag = r["tag"].as_str()?;
            let items = host.recipe_ingredient(&serde_json::json!(format!("#{tag}")))?.order;
            (items.into_iter().next()?, r["count"].as_u64().unwrap_or(1) as u32)
        }
        "occultism:weighted_item" => {
            let stack = &r["stack"];
            (stack["id"].as_str()?.to_string(), stack["count"].as_u64().unwrap_or(1) as u32)
        }
        _ => return None,
    };
    (!miners.is_empty()).then_some(Dig { miners, result, count: count.max(1), weight })
}
