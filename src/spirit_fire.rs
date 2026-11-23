// Spirit fire and Demon's Dream: the glue between the game and the rules (fire.rs). Datura seeds
// drop from grass and plant the datura crop; flint and steel used by dropped Demon's Dream fruit
// lights spirit fire, using the fruit up; an item thrown into the fire turns by Occultism's
// `occultism:spirit_fire` recipes, read from the mod's data.

use crate::fire::{self, FireRecipe};
use crate::game::{self, id, lock};
use mod_api::*;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct SpiritFire {
    /// The recipes, resolved once per recipe table (as the crushing ones are).
    recipes: Mutex<(Vec<usize>, Arc<[FireRecipe]>)>,
    starters: Mutex<Option<Vec<String>>>,
    rng: AtomicU64,
    /// What fell in since the last server tick (dimension, entity id): `entity_inside` runs inside
    /// the entity's move and can't touch it, so the turning waits for the tick.
    fallen: Mutex<Vec<(String, i32)>>,
}

/// A dropped item's data for `/summon`-like spawning: `{Item: {id, count}}`.
fn item_nbt(item: &str, count: u32) -> Compound {
    let mut stack = Compound::new();
    stack.insert("id".into(), Tag::String(item.into()));
    stack.insert("count".into(), Tag::Int(count as i32));
    let mut nbt = Compound::new();
    nbt.insert("Item".into(), Tag::Compound(stack));
    nbt
}

impl SpiritFire {
    fn recipes(&self, host: &dyn Host) -> Arc<[FireRecipe]> {
        let table = host.recipes_of_type(fire::RECIPE_TYPE);
        let key: Vec<usize> = table.iter().map(|r| Arc::as_ptr(&r.json) as usize).collect();
        let mut cache = lock(&self.recipes);
        if cache.0 != key {
            let parsed: Vec<FireRecipe> = table
                .iter()
                .filter_map(|r| {
                    let inputs = crate::crushing::ingredient(host, &r.json["ingredient"])?;
                    let result = r.json["result"]["id"].as_str()?.to_string();
                    let count = r.json["result"]["count"].as_u64().unwrap_or(1).clamp(1, 64) as u32;
                    Some(FireRecipe { inputs, result, count })
                })
                .collect();
            *cache = (key, parsed.into());
        }
        cache.1.clone()
    }

    fn is_starter(&self, host: &dyn Host, item: &str) -> bool {
        let mut starters = lock(&self.starters);
        let list = starters.get_or_insert_with(|| crate::crushing::ingredient(host, &serde_json::json!(fire::STARTERS)).unwrap_or_default());
        list.iter().any(|s| s == item)
    }

    /// A datura seed in grass's drops, now and then.
    pub fn drops(&self, host: &dyn Host, context: &LootDrops<'_>, drops: &mut Vec<ItemStack>) {
        if context.block.is_none() {
            return;
        }
        let sheared = context.tool.is_some_and(|t| id(&*host, t) == "minecraft:shears");
        if fire::drops_seed(context.table, sheared, game::roll(&self.rng))
            && let Some(seed) = game::stack(&*host, fire::DATURA_SEEDS, 1)
        {
            drops.push(seed);
        }
    }

    /// An igniter used on the block at `clicked`: with Demon's Dream dropped around it, spirit
    /// fire beside the clicked face, the fruit used up (all of it, as Occultism's), the flint and
    /// steel worn or the fire charge spent. Whether it lit.
    pub fn light(&self, host: &mut dyn ServerHost, dimension: &str, clicked: [i32; 3], face: [i32; 3], player: u128, hand: Hand) -> bool {
        let Some(level) = host.level(dimension) else { return false };
        let centre = clicked.map(|c| f64::from(c) + 0.5);
        let entities = level.entities_in(&*host, centre.map(|c| c - 1.0), centre.map(|c| c + 1.0)).unwrap_or_default();
        let fruit: Vec<EntityRef> = entities
            .into_iter()
            .filter(|e| {
                host.item_entity(*e)
                    .and_then(|i| i.stack(&*host))
                    .is_some_and(|s| self.is_starter(&*host, id(&*host, &s)))
            })
            .collect();
        let at = fire::beside(clicked, face);
        if fruit.is_empty() || !level.block_state(&*host, at).and_then(|s| host.block_state_string(s)).is_some_and(|s| fire::may_light(&s)) {
            return false;
        }
        if !game::set_block(host, level, at, fire::SPIRIT_FIRE) {
            return false;
        }
        for e in fruit {
            e.discard(host);
        }
        let Some(living) = game::living(&*host, player) else { return true };
        let held = living.hand_item(&*host, hand);
        if held.as_ref().is_some_and(|s| id(&*host, s) == "minecraft:flint_and_steel") {
            let slot = if hand == Hand::Main { EquipmentSlot::MainHand } else { EquipmentSlot::OffHand };
            living.hurt_and_break(host, slot, 1);
        } else {
            game::use_one(host, player, hand);
        }
        true
    }

    /// Turn what fell in since the last tick.
    pub fn burn_fallen(&self, host: &mut dyn ServerHost) {
        let fallen = std::mem::take(&mut *self.fallen.lock().unwrap_or_else(|e| e.into_inner()));
        let mut seen = std::collections::HashSet::new();
        for (dimension, id) in fallen {
            if seen.insert(id)
                && let Some(entity) = u32::try_from(id).ok().and_then(|id| host.entity(id))
            {
                self.burn(host, &dimension, entity);
            }
        }
    }

    /// An item in spirit fire turns, the whole stack, each into the recipe's count.
    fn burn(&self, host: &mut dyn ServerHost, dimension: &str, entity: EntityRef) {
        let Some(item) = host.item_entity(entity) else { return };
        let Some(stack) = item.stack(&*host) else { return };
        let recipes = self.recipes(&*host);
        let Some(recipe) = fire::recipe_for(&recipes, id(&*host, &stack)) else { return };
        let Some(max) = game::item(&*host, &recipe.result).and_then(|k| host.max_stack_size(k)) else { return };
        let stacks = fire::split(stack.count().saturating_mul(recipe.count), max);
        let at = entity.position(&*host);
        for (n, size) in stacks.into_iter().enumerate() {
            if n == 0 {
                if let Some(turned) = game::stack(&*host, &recipe.result, size) {
                    let _ = item.set_stack(host, turned);
                }
            } else if let Some(at) = at {
                host.summon(dimension, "minecraft:item", at, item_nbt(&recipe.result, size));
            }
        }
    }
}

/// The spirit fire block: what falls in turns.
pub struct FirePart(pub Arc<crate::State>);

#[mod_api::sunrise_part]
impl BlockPart for FirePart {
    fn entity_inside(&self, _host: &mut dyn LevelRandomHost, event: &mut EntityInsideEvent<'_>) {
        if event.kind == "minecraft:item" {
            let fallen = (event.dimension.to_string(), event.entity);
            self.0.spirit_fire.fallen.lock().unwrap_or_else(|e| e.into_inner()).push(fallen);
        }
    }
}

/// Datura seeds: planted on farmland as the datura crop (their template's item would plant wheat).
pub struct SeedsPart;

#[mod_api::sunrise_part]
impl ItemPart for SeedsPart {
    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        // The client doesn't predict the planting (its template would predict wheat).
        let HostSide::Server(host) = host else { return Some(InteractionResult::Consume) };
        let Some(level) = host.level(host.dimension()) else { return None };
        let above = fire::beside(event.pos, [0, 1, 0]);
        let farmland = game::block_name(&*host, level.block(&*host, event.pos)) == "minecraft:farmland";
        let empty = level.block_state(&*host, above).and_then(|s| host.block_state_string(s)).is_some_and(|s| fire::may_light(&s) && !s.starts_with("minecraft:fire"));
        if event.face != [0, 1, 0] || !farmland || !empty {
            return None;
        }
        if game::set_block(host, level, above, fire::DATURA_SEEDS) {
            game::use_one(host, event.player, event.hand);
        }
        Some(InteractionResult::SuccessServer)
    }
}

/// The spirit torch's item: it stands on a floor by its template's own placement, which the game
/// checks. On a wall it would be the spirit wall torch, but its template's item would put a vanilla
/// soul wall torch there, and placing the right one itself writes a block the game hasn't checked
/// the player may change (spawn protection, the world border), which the SDK can't ask at this pin
/// (horizon-mods#1742): on a wall, it isn't placed.
pub struct TorchPart;

pub const SPIRIT_TORCH: &str = "occultism:spirit_torch";

#[mod_api::sunrise_part]
impl ItemPart for TorchPart {
    fn use_on(&self, _host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        if event.face == [0, 1, 0] { None } else { Some(InteractionResult::Fail) }
    }
}
