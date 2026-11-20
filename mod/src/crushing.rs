// Occultism's crushing recipes (`occultism:crushing`), read from the mod's data like any data pack,
// for the crusher spirits (workers.rs), and the readers of a recipe file's ingredients and results
// the rituals and the mineshaft share.

use crate::game::{id, lock};
use crate::logic::{self, CrushingRecipe};
use mod_api::*;
use std::sync::{Arc, Mutex};

pub const RECIPE_TYPE: &str = "occultism:crushing";

/// The crushing recipes, resolved to item ids once per recipe table: a `/reload` brings new
/// `Recipe` values, and their addresses tell the cache it is stale.
#[derive(Default)]
pub struct Crushing {
    recipes: Mutex<(Vec<usize>, Arc<[CrushingRecipe]>)>,
}

impl Crushing {
    pub fn recipes(&self, host: &dyn Host) -> Arc<[CrushingRecipe]> {
        let table = host.recipes_of_type(RECIPE_TYPE);
        let key: Vec<usize> = table.iter().map(|r| Arc::as_ptr(&r.json) as usize).collect();
        let mut cache = lock(&self.recipes);
        if cache.0 != key {
            let parsed: Vec<CrushingRecipe> = table.iter().filter_map(|r| parse_recipe(host, r)).collect();
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

/// An `occultism:crushing` file as `CrushingRecipe.CODEC` reads it. A recipe whose ingredient or
/// result names nothing the game has is left out, as NeoForge's `tag_empty` conditions on the
/// original files leave it out.
fn parse_recipe(host: &dyn Host, r: &mod_api::Recipe) -> Option<CrushingRecipe> {
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
        time: int("crushing_time", logic::DEFAULT_CRUSHING_TIME as i64).max(1) as u32,
        min_tier: int("min_tier", -1) as i32,
        max_tier: int("max_tier", -1) as i32,
        ignore_multiplier: j["ignore_crushing_multiplier"].as_bool().unwrap_or(false),
    })
}
