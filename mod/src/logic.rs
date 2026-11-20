// The processing spirits' rules, with no Horizon types in them, so they can be tested on their own
// (tests/logic compiles this file by path). workers.rs turns the spirit's hand and the game's
// recipes into these types and back.
//
// A crusher (`CrusherJob`), crystallizer (`CrystallizerJob`) or smelter (`SmelterJob`) holds what it
// picked up in its main hand and works it there over its recipes (Occultism's `occultism:crushing`
// and `occultism:crystallize`, vanilla's cooking recipes): the recipe's time times its job's time
// multiplier at its rank per operation, the result times its output multiplier (unless the recipe
// ignores it), and the result dropped at its feet.

/// Occultism's four spirit ranks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tier {
    Foliot,
    Djinni,
    Afrit,
    Marid,
}

impl Tier {
    pub const ALL: [Tier; 4] = [Tier::Foliot, Tier::Djinni, Tier::Afrit, Tier::Marid];

    /// The tier number recipes filter on (`min_tier`, `max_tier`).
    pub fn level(self) -> i32 {
        match self {
            Tier::Foliot => 1,
            Tier::Djinni => 2,
            Tier::Afrit => 3,
            Tier::Marid => 4,
        }
    }
    pub fn from_level(level: i32) -> Option<Tier> {
        Tier::ALL.into_iter().find(|t| t.level() == level)
    }
    pub fn name(self) -> &'static str {
        match self {
            Tier::Foliot => "foliot",
            Tier::Djinni => "djinni",
            Tier::Afrit => "afrit",
            Tier::Marid => "marid",
        }
    }
    pub fn from_name(name: &str) -> Option<Tier> {
        Tier::ALL.into_iter().find(|t| t.name() == name)
    }
    /// Recipes done per operation (`operationCount`): 1 for every job and rank by default.
    pub fn operation_count(self) -> u32 {
        1
    }
}

/// A processing job, with its defaults from Occultism's server config
/// (`OccultismServerConfig.SpiritJobSettings`: crusher_tier1..4, crystal_tier1..4, smelter_tier1..4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Job {
    Crusher,
    Crystallizer,
    Smelter,
}

impl Job {
    /// The multiplier on each recipe's time at `tier` (`timeMultiplier`).
    pub fn time_multiplier(self, tier: Tier) -> f32 {
        let by_rank = match self {
            Job::Crusher => [2.0, 1.0, 0.5, 0.3],
            Job::Crystallizer => [1.0, 0.5, 0.3, 0.1],
            Job::Smelter => [1.0, 0.5, 0.1, 0.01],
        };
        by_rank[tier.level() as usize - 1]
    }
    /// The multiplier on each result's count at `tier` (`outputMultiplier`); the smelter has none.
    pub fn output_multiplier(self, tier: Tier) -> f32 {
        match self {
            Job::Crusher | Job::Crystallizer => [1.0, 1.5, 2.0, 3.0][tier.level() as usize - 1],
            Job::Smelter => 1.0,
        }
    }
}

/// A processing recipe (`occultism:crushing`, `occultism:crystallize`, or a vanilla cooking recipe),
/// its ingredient already resolved to item ids and its result (an item, or a tag's first item as
/// Occultism's `TagUtil.getItemForTag`) to one item.
#[derive(Clone, Debug, PartialEq)]
pub struct CrushingRecipe {
    pub id: String,
    pub inputs: Vec<String>,
    pub result: String,
    pub count: u32,
    /// `crushing_time` or `crystallize_time`, 200 when the file leaves it out; a cooking recipe's
    /// `cookingtime`.
    pub time: u32,
    /// `min_tier` and `max_tier`; -1 means no bound.
    pub min_tier: i32,
    pub max_tier: i32,
    /// `ignore_crushing_multiplier`: the tier's output multiplier doesn't apply (ingot to dust
    /// recipes, so crushing can't multiply metal in a loop).
    pub ignore_multiplier: bool,
}

pub const DEFAULT_CRUSHING_TIME: u32 = 200;

impl CrushingRecipe {
    /// `CrushingRecipe.matches`: the item fits and the tier is within the bounds that are set.
    pub fn matches(&self, item: &str, tier: Tier) -> bool {
        let t = tier.level();
        let tier_ok = (self.min_tier < 0 || t >= self.min_tier) && (self.max_tier < 0 || t <= self.max_tier);
        tier_ok && self.inputs.iter().any(|i| i == item)
    }

    /// Ticks to finish one operation at this tier: the crusher compares its tick counter with
    /// `crushingTime * timeMultiplier` (an int against a float), so this is that product rounded
    /// up, and at least one tick.
    pub fn ticks(&self, job: Job, tier: Tier) -> u32 {
        let threshold = self.time as f32 * job.time_multiplier(tier);
        (threshold.ceil() as u32).max(1)
    }

    /// The result count of one operation over `operations` inputs, as `CrusherJob` computes it:
    /// `(int) (count * operations * multiplier)` in float arithmetic, truncated.
    pub fn result_count(&self, job: Job, tier: Tier, operations: u32) -> u32 {
        let multiplier = if self.ignore_multiplier {
            1.0
        } else {
            job.output_multiplier(tier)
        };
        ((self.count * operations) as f32 * multiplier) as u32
    }
}

/// Where a crusher spirit is in its work: `CrusherJob`'s `currentRecipe` and `crushingTimer`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Crushing {
    /// The recipe it is working, so a swapped input starts over.
    pub recipe: Option<String>,
    /// Ticks spent on the current operation.
    pub timer: u32,
}

/// What a crusher's tick came to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Nothing in hand, or what it held no longer matches its recipe.
    Idle,
    /// What it holds has no recipe at its rank: it drops it.
    NoRecipe,
    /// Working: `timer` of `total` ticks.
    Working { timer: u32, total: u32 },
    /// One operation done: `used` of what it holds became `produced` of `result`.
    Done { used: u32, result: String, produced: u32 },
}

/// Whether a crusher of `tier` takes `item`: some recipe at its rank crushes it
/// (`CrusherJob.onInit`'s `itemsToPickUp`).
pub fn crushable(recipes: &[CrushingRecipe], item: &str, tier: Tier) -> bool {
    recipes.iter().any(|r| r.matches(item, tier))
}

/// One server tick of a spirit doing `job` at `tier` holding `held` (its item and count), as
/// `CrusherJob.update` runs it (and the crystallizer's and smelter's, the same). Changes `state`; the caller takes `used` from the hand and drops
/// the result, or drops what it holds on `NoRecipe`.
pub fn crush_step(state: &mut Crushing, held: Option<(&str, u32)>, recipes: &[CrushingRecipe], job: Job, tier: Tier) -> Step {
    let held = held.filter(|(_, count)| *count > 0);
    if let (None, Some((item, _))) = (&state.recipe, held) {
        state.timer = 0;
        match recipes.iter().find(|r| r.matches(item, tier)) {
            Some(r) => state.recipe = Some(r.id.clone()),
            None => return Step::NoRecipe,
        }
    }
    let Some(id) = state.recipe.clone() else { return Step::Idle };
    let recipe = recipes.iter().find(|r| r.id == id);
    let (Some(recipe), Some((item, count))) = (recipe, held) else {
        state.recipe = None;
        return Step::Idle;
    };
    if !recipe.matches(item, tier) {
        state.recipe = None;
        return Step::Idle;
    }
    state.timer += 1;
    let total = recipe.ticks(job, tier);
    if state.timer < total {
        return Step::Working { timer: state.timer, total };
    }
    // The recipe stays cached for the next operation.
    state.timer = 0;
    let used = tier.operation_count().min(count);
    Step::Done { used, result: recipe.result.clone(), produced: recipe.result_count(job, tier, used) }
}
