// Occultism's pastes (`DamageInCraftingItem`): gray and nature paste last 64 crafts. A recipe that
// takes one is this port's `occultism:paste_shapeless`, written by tools/build_content.py with its
// ingredients resolved to items: a shapeless recipe that leaves the paste in the grid a use worn,
// until its last use. The platform files make it a crafting kind (their signatures differ).

use crate::paste_items::ITEMS;
use mod_api::*;

pub const KIND: &str = "occultism:paste_shapeless";
const PASTES: [&str; 2] = ["occultism:gray_paste", "occultism:nature_paste"];
/// A paste's uses (`durability(64)`).
const USES: u32 = 64;

/// A paste recipe: the items each ingredient takes and its result, as indices into `ITEMS`.
#[derive(Clone, Debug, PartialEq, mod_api::Wire)]
pub struct PasteRecipe {
    pub ingredients: Vec<Vec<u16>>,
    pub result: u16,
    pub count: u32,
}

/// The items paste recipes name, looked up at startup.
pub struct PasteKind {
    keys: Vec<ItemKey>,
    pastes: Vec<ItemKey>,
}

fn index(id: &str) -> Result<u16, String> {
    ITEMS.iter().position(|i| *i == id).map(|i| i as u16).ok_or_else(|| format!("{id} isn't in paste_items.rs"))
}

impl PasteKind {
    pub fn new(r: &mut dyn Registrar) -> Self {
        PasteKind { keys: ITEMS.iter().map(|id| r.item_key(id)).collect(), pastes: PASTES.iter().map(|id| r.item_key(id)).collect() }
    }

    /// A recipe file, with its type removed.
    pub fn read(&self, json: &str) -> Result<PasteRecipe, String> {
        let j: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let ids = |v: &serde_json::Value| -> Result<Vec<u16>, String> {
            v.as_array().ok_or("an ingredient isn't a list")?.iter().map(|i| index(i.as_str().unwrap_or_default())).collect()
        };
        let ingredients = j["ingredients"].as_array().ok_or("no ingredients")?.iter().map(ids).collect::<Result<Vec<_>, _>>()?;
        let result = index(j["result"]["id"].as_str().unwrap_or_default())?;
        let count = j["result"]["count"].as_u64().unwrap_or(1).clamp(1, 99) as u32;
        Ok(PasteRecipe { ingredients, result, count })
    }

    /// The recipe as its file has it, for the clients.
    pub fn json(&self, recipe: &PasteRecipe) -> Result<String, String> {
        let id = |i: &u16| ITEMS.get(usize::from(*i)).copied().ok_or_else(|| "an item outside paste_items.rs".to_string());
        let ingredients = recipe.ingredients.iter().map(|ing| ing.iter().map(id).collect::<Result<Vec<_>, _>>()).collect::<Result<Vec<_>, _>>()?;
        Ok(serde_json::json!({"ingredients": ingredients, "result": {"id": id(&recipe.result)?, "count": recipe.count}}).to_string())
    }

    /// Whether the grid holds exactly the recipe's ingredients, one item to a cell, in any order.
    pub fn fits(&self, recipe: &PasteRecipe, grid: &CraftingGrid) -> bool {
        let cells: Vec<ItemKey> = grid.cells.iter().flatten().map(ItemStack::item).collect();
        if cells.len() != recipe.ingredients.len() {
            return false;
        }
        let takes = |ingredient: &[u16], key: ItemKey| ingredient.iter().any(|i| self.keys.get(usize::from(*i)) == Some(&key));
        // Each cell to a different ingredient (there are at most nine): try each in turn.
        fn assign(cells: &[ItemKey], used: &mut Vec<bool>, recipe: &PasteRecipe, takes: &dyn Fn(&[u16], ItemKey) -> bool) -> bool {
            let Some((&first, rest)) = cells.split_first() else { return true };
            for (n, ingredient) in recipe.ingredients.iter().enumerate() {
                if !used[n] && takes(ingredient, first) {
                    used[n] = true;
                    if assign(rest, used, recipe, takes) {
                        return true;
                    }
                    used[n] = false;
                }
            }
            false
        }
        assign(&cells, &mut vec![false; recipe.ingredients.len()], recipe, &takes)
    }

    pub fn result(&self, recipe: &PasteRecipe) -> Option<ItemStack> {
        ItemStack::new(*self.keys.get(usize::from(recipe.result))?, recipe.count)
    }

    /// What stays in each cell: a paste one use more worn, none once its last use is spent.
    pub fn left(&self, grid: &CraftingGrid) -> Vec<Option<ItemStack>> {
        grid.cells
            .iter()
            .map(|cell| {
                let cell = cell.as_ref().filter(|c| self.pastes.contains(&c.item()))?;
                let worn = cell.damage() + 1;
                if worn >= USES {
                    return None;
                }
                let mut paste = cell.clone();
                paste.set_count(1).ok()?;
                paste.set_damage(worn);
                Some(paste)
            })
            .collect()
    }
}
