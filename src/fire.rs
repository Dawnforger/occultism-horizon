// Spirit fire and Demon's Dream (datura), without Horizon types: where datura seeds drop, where
// flint and steel lights spirit fire over dropped datura, and what the fire turns a thrown item
// into (Occultism's `SpiritFireBlock`, `PlayerEventHandler.spiritFire` and its loot modifiers).

pub const SPIRIT_FIRE: &str = "occultism:spirit_fire";
pub const DATURA_SEEDS: &str = "occultism:datura_seeds";
/// The item tag of what lights spirit fire (Occultism's `start_spiritfire`: the fruit).
pub const STARTERS: &str = "#occultism:start_spiritfire";
pub const RECIPE_TYPE: &str = "occultism:spirit_fire";

/// The loot tables Occultism's `datura_seed_from_*` modifiers add a seed to, 2% of the time, when
/// the block isn't sheared.
pub const SEED_TABLES: [&str; 8] = [
    "minecraft:blocks/short_grass",
    "minecraft:blocks/tall_grass",
    "minecraft:blocks/fern",
    "minecraft:blocks/large_fern",
    "minecraft:blocks/bush",
    "minecraft:blocks/firefly_bush",
    "minecraft:blocks/short_dry_grass",
    "minecraft:blocks/tall_dry_grass",
];
/// The seed's chance, in millionths (0.02).
const SEED_CHANCE: u64 = 20_000;

/// What lights fire: flint and steel, a fire charge (Occultism's igniters, less mods' tools).
pub const IGNITERS: [&str; 2] = ["minecraft:flint_and_steel", "minecraft:fire_charge"];

/// Whether breaking a block that dropped `table` adds a datura seed; `roll` is any random number.
pub fn drops_seed(table: &str, sheared: bool, roll: u64) -> bool {
    !sheared && SEED_TABLES.contains(&table) && roll % 1_000_000 < SEED_CHANCE
}

/// Whether an item at `at` lies in the box Occultism searches for datura when `clicked` is lit: a
/// block each way from the clicked block's centre.
pub fn near(clicked: [i32; 3], at: [f64; 3]) -> bool {
    (0..3).all(|i| (at[i] - (f64::from(clicked[i]) + 0.5)).abs() <= 1.0)
}

/// The block beside the clicked face: where flint and steel lights fire, and seeds are planted.
pub fn beside(clicked: [i32; 3], face: [i32; 3]) -> [i32; 3] {
    [clicked[0] + face[0], clicked[1] + face[1], clicked[2] + face[2]]
}

/// Whether spirit fire may take the place of `state` (`minecraft:fire[age=0,...]` as `/setblock`
/// writes it): air, or the fire the flint and steel just lit.
pub fn may_light(state: &str) -> bool {
    let block = state.split('[').next().unwrap_or(state);
    matches!(block, "minecraft:air" | "minecraft:cave_air" | "minecraft:fire")
}

/// Whether `state` is the wheat sprout datura seeds plant (their template's), to be made datura.
pub fn is_sprout(state: &str) -> bool {
    state == "minecraft:wheat[age=0]"
}

/// A spirit fire recipe, its ingredient resolved to items.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FireRecipe {
    pub inputs: Vec<String>,
    pub result: String,
    /// Made of each item thrown in: 1 for Occultism's own, 64 for this port's chalks.
    pub count: u32,
}

/// The recipe the fire turns `item` by: the first that takes it. The whole stack turns
/// (`SpiritFireRecipe.assemble`), each item into the recipe's count.
pub fn recipe_for<'a>(recipes: &'a [FireRecipe], item: &str) -> Option<&'a FireRecipe> {
    recipes.iter().find(|r| r.inputs.iter().any(|i| i == item))
}

/// `total` items as stacks of at most `max`: the first takes the thrown stack's place, the rest
/// drop beside it.
pub fn split(total: u32, max: u32) -> Vec<u32> {
    let max = max.max(1);
    let mut left = total;
    let mut stacks = Vec::new();
    while left > 0 {
        stacks.push(left.min(max));
        left -= left.min(max);
    }
    stacks
}
