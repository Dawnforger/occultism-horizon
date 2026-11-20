// The processing spirits' recipes, read from the game's data like any data pack: Occultism's
// crushing (`occultism:crushing`) and crystallizing (`occultism:crystallize`) recipes, and vanilla's
// cooking recipes for the smelters; and the readers of a recipe file's ingredients and results the
// rituals and the mineshaft share.

use crate::game::{id, lock};
use crate::logic::{self, CrushingRecipe, Job};
use mod_api::*;
use std::sync::{Arc, Mutex};

pub const RECIPE_TYPE: &str = "occultism:crushing";
pub const CRYSTALLIZE_TYPE: &str = "occultism:crystallize";
/// What a smelter cooks by, in the order `SmelterJob` looks: a smelting recipe first, then blasting,
/// smoking and the campfire's.
const COOKING: [&str; 4] = ["minecraft:smelting", "minecraft:blasting", "minecraft:smoking", "minecraft:campfire_cooking"];

type Cache = Mutex<(Vec<usize>, Arc<[CrushingRecipe]>)>;

/// Each job's recipes, resolved to item ids once per recipe table: a `/reload` brings new `Recipe`
/// values, and their addresses tell a cache it is stale.
#[derive(Default)]
pub struct Crushing {
    crushing: Cache,
    crystallizing: Cache,
    cooking: Cache,
}

impl Crushing {
    pub fn recipes(&self, host: &dyn Host, job: Job) -> Arc<[CrushingRecipe]> {
        let (cache, tables) = match job {
            Job::Crusher => (&self.crushing, vec![host.recipes_of_type(RECIPE_TYPE)]),
            Job::Crystallizer => (&self.crystallizing, vec![host.recipes_of_type(CRYSTALLIZE_TYPE)]),
            Job::Smelter => (&self.cooking, COOKING.iter().map(|kind| host.recipes_of_type(kind)).collect()),
        };
        let key: Vec<usize> = tables.iter().flat_map(|t| t.iter()).map(|r| Arc::as_ptr(&r.json) as usize).collect();
        let mut cache = lock(cache);
        if cache.0 != key {
            let parsed: Vec<CrushingRecipe> = tables
                .iter()
                .flat_map(|t| t.iter())
                .filter_map(|r| match job {
                    Job::Crusher => parse_recipe(host, r, "crushing_time", "ignore_crushing_multiplier"),
                    Job::Crystallizer => parse_recipe(host, r, "crystallize_time", "ignore_crystallize_multiplier"),
                    Job::Smelter => cooking(host, r),
                })
                .collect();
            *cache = (key, parsed.into());
        }
        cache.1.clone()
    }
}

/// An ingredient's items, in the game's order, from a recipe file's JSON.
pub fn ingredient(host: &dyn Host, json: &serde_json::Value) -> Option<Vec<String>> {
    Some(host.recipe_ingredient(&json.to_string()).ok()??.order)
}

/// A result's item, from a recipe file's JSON (`{"id": ...}`).
pub fn result_item(host: &dyn Host, json: &serde_json::Value) -> Option<String> {
    let one = serde_json::json!({"id": json["id"], "count": 1});
    Some(id(&*host, &host.recipe_result(&one.to_string()).ok()??).to_string())
}

/// An `occultism:crushing` or `occultism:crystallize` file as Occultism's codecs read it, by its
/// time and multiplier fields' names. A recipe whose ingredient or result names nothing the game has
/// is left out, as NeoForge's `tag_empty` conditions on the original files leave it out.
fn parse_recipe(host: &dyn Host, r: &mod_api::Recipe, time: &str, ignore: &str) -> Option<CrushingRecipe> {
    let j = &*r.json;
    let inputs = ingredient(host, &j["ingredient"])?;
    let result = &j["result"];
    let item = match result["type"].as_str() {
        Some("occultism:tag") => {
            let tag = result["tag"].as_str()?;
            ingredient(host, &serde_json::Value::String(format!("#{tag}")))?.into_iter().next()?
        }
        _ => result_item(host, result)?,
    };
    let int = |key: &str, default: i64| j[key].as_i64().unwrap_or(default);
    Some(CrushingRecipe {
        id: r.id.clone(),
        inputs,
        result: item,
        count: result["count"].as_u64().unwrap_or(1).clamp(1, 99) as u32,
        time: int(time, logic::DEFAULT_CRUSHING_TIME as i64).max(1) as u32,
        min_tier: int("min_tier", -1) as i32,
        max_tier: int("max_tier", -1) as i32,
        ignore_multiplier: j[ignore].as_bool().unwrap_or(false),
    })
}

/// A vanilla cooking recipe, as the game typed it: its one ingredient, its result and its time.
fn cooking(host: &dyn Host, r: &mod_api::Recipe) -> Option<CrushingRecipe> {
    let result = r.results.first()?;
    Some(CrushingRecipe {
        id: r.id.clone(),
        inputs: r.ingredients.first()?.order.clone(),
        result: id(host, result).to_string(),
        count: result.count(),
        time: r.cooking_time?.max(1),
        min_tier: -1,
        max_tier: -1,
        ignore_multiplier: true,
    })
}
