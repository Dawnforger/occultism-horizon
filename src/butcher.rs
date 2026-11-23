// The butcher knives' extra drops (Occultism's `add_item` loot modifiers, resolved by
// tools/build_content.py into `data/occultism/knife_drops.json`): tallow from animals killed with
// either knife, heads and echo dust with the iesnium one.

use crate::game::{self, lock};
use mod_api::*;
use std::sync::{Arc, Mutex};

pub const DROPS: &str = "occultism:knife_drops.json";

/// One extra drop: what a kill with one of `knives` of one of `kinds` adds, `chance` of the time.
#[derive(Clone, Debug, PartialEq)]
pub struct KnifeDrop {
    pub knives: Vec<String>,
    pub kinds: Vec<String>,
    pub chance: f64,
    pub item: String,
    pub count: u32,
}

#[derive(Default)]
pub struct Butcher {
    drops: Mutex<Option<Arc<Vec<KnifeDrop>>>>,
    rng: std::sync::atomic::AtomicU64,
}

impl Butcher {
    fn drops(&self, host: &dyn Host) -> Arc<Vec<KnifeDrop>> {
        let mut cache = lock(&self.drops);
        if let Some(d) = cache.as_ref() {
            return d.clone();
        }
        let file = Identifier::parse(DROPS).ok().and_then(|id| host.resource(PackType::ServerData, &id));
        let parsed = Arc::new(file.map(|b| parse(&b)).unwrap_or_default());
        *cache = Some(parsed.clone());
        parsed
    }

    /// What a creature of `kind` (Occultism's possessed kind where it is one) killed with `weapon`
    /// drops besides its own loot.
    pub fn extra(&self, host: &dyn Host, kind: &str, weapon: &ItemStack) -> Vec<ItemStack> {
        let knife = game::id(host, weapon);
        let mut out = Vec::new();
        for d in self.drops(host).iter() {
            if !d.knives.iter().any(|k| k == knife) || !d.kinds.iter().any(|k| k == kind) {
                continue;
            }
            let roll = (game::roll(&self.rng) % 1_000_000) as f64 / 1_000_000.0;
            if roll < d.chance
                && let Some(stack) = game::stack(host, &d.item, d.count)
            {
                out.push(stack);
            }
        }
        out
    }
}

fn parse(bytes: &[u8]) -> Vec<KnifeDrop> {
    let Ok(serde_json::Value::Array(all)) = serde_json::from_slice::<serde_json::Value>(bytes) else { return Vec::new() };
    let strings = |v: &serde_json::Value| v.as_array().into_iter().flatten().filter_map(|s| s.as_str().map(str::to_string)).collect::<Vec<_>>();
    all.iter()
        .filter_map(|d| {
            Some(KnifeDrop {
                knives: strings(&d["knives"]),
                kinds: strings(&d["kinds"]),
                chance: d["chance"].as_f64().unwrap_or(1.0),
                item: d["item"].as_str()?.to_string(),
                count: d["count"].as_u64().unwrap_or(1).clamp(1, 64) as u32,
            })
        })
        .collect()
}
