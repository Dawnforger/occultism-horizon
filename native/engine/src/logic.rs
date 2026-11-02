// The Demonic Altar's rules, with no Horizon types in them, so they can be tested on their own
// (tests/logic compiles this file by path). lib.rs turns the game's stacks, recipes and saved
// compound into these types and back.
//
// The altar is a hollow machine: what it does is decided by the spirit core in its core slot.
// A crusher core makes it Occultism's crusher spirit (CrusherJob), with the same recipes, tier
// filter, time multiplier, output multiplier and operation count, except that the input comes
// from the altar's input slot instead of the spirit's hand, and the result goes to the output
// buffer instead of the ground. A full buffer holds the finished operation until there is room.

/// Slot 0 holds the spirit core; hoppers never reach it.
pub const SLOT_CORE: usize = 0;
/// The input buffer: hoppers and pipes put ores here from the top and the sides.
pub const SLOT_INPUT: usize = 1;
/// The output buffer: hoppers take from here through the bottom; players only take.
pub const SLOTS_OUTPUT: [usize; 4] = [2, 3, 4, 5];
pub const SLOT_COUNT: usize = 6;

/// The data slots the altar's screen reads (vanilla's `ContainerData`).
pub const DATA_PROGRESS: usize = 0;
pub const DATA_TOTAL: usize = 1;
pub const DATA_TIER: usize = 2;
pub const DATA_STATE: usize = 3;
pub const DATA_COUNT: usize = 4;

/// What a core makes the altar do. Only the crusher exists in this proof of concept; the
/// others in Occultism (smelter, crystallizer...) are the same shape and slot in here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Job {
    Crusher,
}

impl Job {
    pub fn from_name(name: &str) -> Option<Job> {
        match name {
            "crusher" => Some(Job::Crusher),
            _ => None,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Job::Crusher => "crusher",
        }
    }
}

/// Occultism's four spirit ranks, with the crusher's defaults from its server config
/// (`OccultismServerConfig.SpiritJobSettings`: crusher_tier1..4).
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
    /// The multiplier on each recipe's crushing time (`timeMultiplier`).
    pub fn time_multiplier(self) -> f32 {
        match self {
            Tier::Foliot => 2.0,
            Tier::Djinni => 1.0,
            Tier::Afrit => 0.5,
            Tier::Marid => 0.3,
        }
    }
    /// The multiplier on each result's count (`outputMultiplier`).
    pub fn output_multiplier(self) -> f32 {
        match self {
            Tier::Foliot => 1.0,
            Tier::Djinni => 1.5,
            Tier::Afrit => 2.0,
            Tier::Marid => 3.0,
        }
    }
    /// Recipes done per operation (`operationCount`): 1 for every rank by default.
    pub fn operation_count(self) -> u32 {
        1
    }
}

/// A bound spirit: the job its core gives the altar and its rank.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spirit {
    pub job: Job,
    pub tier: Tier,
}

/// The `occultism:spirit` data component a core carries: `{job: "crusher", tier: 2}`.
/// Any item carrying it binds that spirit, so new jobs and ranks are data, not new code paths.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpiritComponent {
    pub job: Option<String>,
    pub tier: Option<i32>,
}

/// The four crusher cores, by item id. A core's `occultism:spirit` component decides first; the
/// id is the fallback for a stack whose component is still its item's default (Horizon keeps a
/// stack's components only where they differ from the item's).
pub const CORES: [(&str, Job, Tier); 4] = [
    ("occultism:crusher_core_foliot", Job::Crusher, Tier::Foliot),
    ("occultism:crusher_core_djinni", Job::Crusher, Tier::Djinni),
    ("occultism:crusher_core_afrit", Job::Crusher, Tier::Afrit),
    ("occultism:crusher_core_marid", Job::Crusher, Tier::Marid),
];

pub fn spirit_of(item: &str, component: Option<&SpiritComponent>) -> Option<Spirit> {
    if let Some(c) = component {
        let job = c.job.as_deref().and_then(Job::from_name);
        let tier = c.tier.and_then(Tier::from_level);
        if let (Some(job), Some(tier)) = (job, tier) {
            return Some(Spirit { job, tier });
        }
    }
    CORES
        .iter()
        .find(|(id, _, _)| *id == item)
        .map(|&(_, job, tier)| Spirit { job, tier })
}

/// A stack in one of the altar's slots. `plain` is false for a stack with components of its own
/// (a name, enchantments...): the altar never merges a result into it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stack {
    pub item: String,
    pub count: u32,
    pub plain: bool,
}

impl Stack {
    pub fn plain(item: &str, count: u32) -> Stack {
        Stack {
            item: item.to_string(),
            count,
            plain: true,
        }
    }
}

/// An `occultism:crushing` recipe, its ingredient already resolved to item ids and its result
/// (an item, or a tag's first item as Occultism's `TagUtil.getItemForTag`) to one item.
#[derive(Clone, Debug, PartialEq)]
pub struct Recipe {
    pub id: String,
    pub inputs: Vec<String>,
    pub result: String,
    pub count: u32,
    /// `crushing_time`, 200 when the file leaves it out.
    pub time: u32,
    /// `min_tier` and `max_tier`; -1 means no bound.
    pub min_tier: i32,
    pub max_tier: i32,
    /// `ignore_crushing_multiplier`: the tier's output multiplier doesn't apply (ingot to dust
    /// recipes, so crushing can't multiply metal in a loop).
    pub ignore_multiplier: bool,
}

pub const DEFAULT_CRUSHING_TIME: u32 = 200;

impl Recipe {
    /// `CrushingRecipe.matches`: the item fits and the tier is within the bounds that are set.
    pub fn matches(&self, item: &str, tier: Tier) -> bool {
        let t = tier.level();
        let tier_ok = (self.min_tier < 0 || t >= self.min_tier) && (self.max_tier < 0 || t <= self.max_tier);
        tier_ok && self.inputs.iter().any(|i| i == item)
    }

    /// Ticks to finish one operation at this tier: the crusher compares its tick counter with
    /// `crushingTime * timeMultiplier` (an int against a float), so this is that product rounded
    /// up, and at least one tick.
    pub fn ticks(&self, tier: Tier) -> u32 {
        let threshold = self.time as f32 * tier.time_multiplier();
        (threshold.ceil() as u32).max(1)
    }

    /// The result count of one operation over `operations` inputs, as `CrusherJob` computes it:
    /// `(int) (count * operations * multiplier)` in float arithmetic, truncated.
    pub fn result_count(&self, tier: Tier, operations: u32) -> u32 {
        let multiplier = if self.ignore_multiplier {
            1.0
        } else {
            tier.output_multiplier()
        };
        ((self.count * operations) as f32 * multiplier) as u32
    }
}

/// What the altar keeps in its saved compound between ticks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AltarState {
    /// Ticks spent on the current operation.
    pub progress: u32,
    /// The recipe being worked, so a swapped input restarts the count as Occultism's does.
    pub recipe: Option<String>,
}

/// Why the altar did or didn't work this tick (shown on its screen and in tests).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Tick {
    /// No core, or a core whose spirit the altar can't use.
    NoCore,
    /// A core, but nothing in the input buffer.
    NoInput,
    /// The input has no recipe at this spirit's rank.
    NoRecipe,
    /// Working: `progress` of `total` ticks.
    Working { progress: u32, total: u32 },
    /// Finished, but the output buffer can't take the whole result: held until it can.
    OutputFull { total: u32 },
    /// One operation done: `consumed` inputs became `produced` of `result`.
    Done { consumed: u32, result: String, produced: u32 },
}

impl Tick {
    /// The state data slot: 0 idle, 1 working, 2 blocked by a full buffer, 3 just finished.
    pub fn code(&self) -> i32 {
        match self {
            Tick::NoCore | Tick::NoInput | Tick::NoRecipe => 0,
            Tick::Working { .. } => 1,
            Tick::OutputFull { .. } => 2,
            Tick::Done { .. } => 3,
        }
    }
}

/// One server tick of an altar. `slots` holds `SLOT_COUNT` slots; `max_stack` gives an item's
/// stack size. Changes `state` and `slots` in place.
pub fn tick(
    state: &mut AltarState,
    slots: &mut [Option<Stack>],
    recipes: &[Recipe],
    max_stack: &dyn Fn(&str) -> u32,
    spirit: Option<Spirit>,
) -> Tick {
    let Some(spirit) = spirit else {
        *state = AltarState::default();
        return Tick::NoCore;
    };
    match spirit.job {
        Job::Crusher => crush(state, slots, recipes, max_stack, spirit.tier),
    }
}

fn crush(
    state: &mut AltarState,
    slots: &mut [Option<Stack>],
    recipes: &[Recipe],
    max_stack: &dyn Fn(&str) -> u32,
    tier: Tier,
) -> Tick {
    let Some(input) = slots[SLOT_INPUT].clone().filter(|s| s.count > 0) else {
        *state = AltarState::default();
        return Tick::NoInput;
    };
    // A stack with components of its own (a renamed ore) isn't an ingredient match in Occultism
    // either way, but crushing it would throw its components away: leave it alone.
    let found = input
        .plain
        .then(|| recipes.iter().find(|r| r.matches(&input.item, tier)))
        .flatten();
    let Some(recipe) = found else {
        *state = AltarState::default();
        return Tick::NoRecipe;
    };
    if state.recipe.as_deref() != Some(recipe.id.as_str()) {
        // A new recipe starts from zero (CrusherJob resets crushingTimer when it finds one).
        state.recipe = Some(recipe.id.clone());
        state.progress = 0;
    }
    let total = recipe.ticks(tier);
    if state.progress < total {
        state.progress += 1;
    }
    if state.progress < total {
        return Tick::Working {
            progress: state.progress,
            total,
        };
    }
    let operations = tier.operation_count().min(input.count);
    let produced = recipe.result_count(tier, operations);
    if produced > 0 && !insert(slots, &recipe.result, produced, max_stack, true) {
        return Tick::OutputFull { total };
    }
    if produced > 0 {
        insert(slots, &recipe.result, produced, max_stack, false);
    }
    let left = input.count - operations;
    slots[SLOT_INPUT] = (left > 0).then(|| Stack { count: left, ..input });
    state.progress = 0;
    Tick::Done {
        consumed: operations,
        result: recipe.result.clone(),
        produced,
    }
}

/// Puts `count` of `item` into the output buffer, onto plain stacks of the same item first and
/// then into empty slots, as `ItemHandlerHelper.insertItemStacked` does. With `simulate` it only
/// says whether all of it fits; the altar never puts in part of a result.
pub fn insert(
    slots: &mut [Option<Stack>],
    item: &str,
    count: u32,
    max_stack: &dyn Fn(&str) -> u32,
    simulate: bool,
) -> bool {
    let max = max_stack(item).max(1);
    let mut left = count;
    let mut plan: Vec<(usize, u32)> = Vec::new();
    for &slot in &SLOTS_OUTPUT {
        if left == 0 {
            break;
        }
        if let Some(s) = &slots[slot] {
            if s.plain && s.item == item && s.count < max {
                let take = (max - s.count).min(left);
                plan.push((slot, take));
                left -= take;
            }
        }
    }
    for &slot in &SLOTS_OUTPUT {
        if left == 0 {
            break;
        }
        if slots[slot].is_none() {
            let take = max.min(left);
            plan.push((slot, take));
            left -= take;
        }
    }
    if left > 0 {
        return false;
    }
    if !simulate {
        for (slot, add) in plan {
            match &mut slots[slot] {
                Some(s) => s.count += add,
                empty => *empty = Some(Stack::plain(item, add)),
            }
        }
    }
    true
}

/// The values the screen's data slots carry for this tick.
pub fn data_slots(tick: &Tick, state: &AltarState, spirit: Option<Spirit>) -> [i32; DATA_COUNT] {
    let mut data = [0; DATA_COUNT];
    data[DATA_TIER] = spirit.map_or(0, |s| s.tier.level());
    data[DATA_STATE] = tick.code();
    match tick {
        Tick::Working { progress, total } => {
            data[DATA_PROGRESS] = *progress as i32;
            data[DATA_TOTAL] = *total as i32;
        }
        Tick::OutputFull { total } => {
            data[DATA_PROGRESS] = *total as i32;
            data[DATA_TOTAL] = *total as i32;
        }
        _ => {
            data[DATA_PROGRESS] = state.progress as i32;
        }
    }
    data
}

/// What a hopper or pipe may do on each face (`WorldlyContainer`), by Direction's order: down,
/// up, north, south, west, east. Down reaches the output buffer, every other face the input.
pub fn face_slots() -> [Vec<usize>; 6] {
    let input = vec![SLOT_INPUT];
    let output = SLOTS_OUTPUT.to_vec();
    [
        output,
        input.clone(),
        input.clone(),
        input.clone(),
        input.clone(),
        input,
    ]
}

/// Whether a player may put `item` into `slot` of the altar's menu (`Slot.mayPlace`), and how
/// many it holds there. The core slot takes one core; the output buffer takes nothing.
pub fn slot_rule(slot: usize, item: &str, component: Option<&SpiritComponent>) -> (bool, u32) {
    match slot {
        SLOT_CORE => (spirit_of(item, component).is_some(), 1),
        SLOT_INPUT => (true, 99),
        _ => (false, 99),
    }
}

/// Where a shift-click on stack `item` from the player's inventory goes (`quickMoveStack`): a
/// core to the core slot when it is empty, anything else to the input buffer.
pub fn quick_move_target(item: &str, component: Option<&SpiritComponent>, core_empty: bool) -> Option<usize> {
    if spirit_of(item, component).is_some() {
        return core_empty.then_some(SLOT_CORE);
    }
    Some(SLOT_INPUT)
}
