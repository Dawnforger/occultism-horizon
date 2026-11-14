// Occultism's rituals without Horizon types, so they test anywhere: the pentacle a ritual needs
// around its golden bowl, the ingredients on the sacrificial bowls, the ritual's clock and when
// it takes each ingredient, the chalk glyphs' signs, and iesnium from silver ore.
//
// Rules from Occultism's `Ritual`, `GoldenSacrificialBowlBlockEntity`, `ChalkGlyphBlock` and
// modonomicon's dense multiblocks.

/// What a pentacle accepts at one place: anything (a display-only key, `_`), or one of a list
/// of blocks (a glyph colour, candles, a skull).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Accept {
    Any,
    OneOf(Vec<String>),
}

/// A pentacle as modonomicon's dense multiblock writes it: layers from the top, each a list of
/// rows (z) of keys (x); the key `0` is the golden bowl the ritual is cast on, and ` ` is air.
#[derive(Clone, Debug)]
pub struct Pentacle {
    /// Every place that isn't `Any`: its offset from the bowl (unrotated) and what it accepts.
    cells: Vec<([i32; 3], Accept)>,
    /// How far the pattern reaches from the bowl: horizontally, below and above.
    pub reach: i32,
    pub below: i32,
    pub above: i32,
}

pub const AIR: [&str; 3] = ["minecraft:air", "minecraft:cave_air", "minecraft:void_air"];

impl Pentacle {
    /// `keys` maps each key to what it accepts; `_` and unknown display keys are `Any`.
    pub fn new(pattern: &[Vec<String>], keys: &[(char, Accept)]) -> Result<Pentacle, String> {
        let mut center = None;
        for (l, layer) in pattern.iter().enumerate() {
            for (z, row) in layer.iter().enumerate() {
                for (x, ch) in row.chars().enumerate() {
                    if ch == '0' {
                        center = Some((l as i32, z as i32, x as i32));
                    }
                }
            }
        }
        let (cl, cz, cx) = center.ok_or("the pattern has no center (`0`)")?;
        let mut cells = Vec::new();
        let (mut reach, mut below, mut above) = (0, 0, 0);
        for (l, layer) in pattern.iter().enumerate() {
            for (z, row) in layer.iter().enumerate() {
                for (x, ch) in row.chars().enumerate() {
                    let offset = [x as i32 - cx, cl - l as i32, z as i32 - cz];
                    let accept = match ch {
                        ' ' => Accept::OneOf(AIR.iter().map(|s| s.to_string()).collect()),
                        _ => keys
                            .iter()
                            .find(|(k, _)| *k == ch)
                            .map(|(_, a)| a.clone())
                            .ok_or_else(|| format!("key {ch:?} has no mapping"))?,
                    };
                    if accept == Accept::Any {
                        continue;
                    }
                    reach = reach.max(offset[0].abs()).max(offset[2].abs());
                    below = below.max(-offset[1]);
                    above = above.max(offset[1]);
                    cells.push((offset, accept));
                }
            }
        }
        Ok(Pentacle { cells, reach, below, above })
    }

    /// Whether the blocks around the bowl at `[0, 0, 0]` (`block_at` names the block at an offset
    /// from it) make this pentacle in one of its four rotations, as modonomicon's `validate`.
    pub fn matches(&self, block_at: &dyn Fn([i32; 3]) -> String) -> bool {
        (0..4).any(|r| self.mismatches(r, block_at, 1).is_empty())
    }

    /// The offsets that don't fit, in the rotation that fits best: what's missing from a circle
    /// someone is building (Occultism's pentacle help).
    pub fn closest(&self, block_at: &dyn Fn([i32; 3]) -> String) -> Vec<([i32; 3], Accept)> {
        (0..4)
            .map(|r| self.mismatches(r, block_at, usize::MAX))
            .min_by_key(Vec::len)
            .unwrap_or_default()
    }

    fn mismatches(&self, rotation: u8, block_at: &dyn Fn([i32; 3]) -> String, stop_at: usize) -> Vec<([i32; 3], Accept)> {
        let mut out = Vec::new();
        for (offset, accept) in &self.cells {
            let at = rotate(*offset, rotation);
            let ok = match accept {
                Accept::Any => true,
                Accept::OneOf(blocks) => {
                    let block = block_at(at);
                    blocks.iter().any(|b| *b == block)
                }
            };
            if !ok {
                out.push((at, accept.clone()));
                if out.len() >= stop_at {
                    break;
                }
            }
        }
        out
    }
}

/// An offset turned `rotation` quarter turns about the bowl (y up).
pub fn rotate([x, y, z]: [i32; 3], rotation: u8) -> [i32; 3] {
    match rotation % 4 {
        0 => [x, y, z],
        1 => [-z, y, x],
        2 => [-x, y, -z],
        _ => [z, y, -x],
    }
}

/// A ritual recipe (`occultism:ritual`), its ingredients as the items each accepts.
#[derive(Clone, Debug, PartialEq)]
pub struct Ritual {
    pub id: String,
    pub activation: Vec<String>,
    /// Seconds, as Occultism counts them on a golden bowl.
    pub duration: u32,
    pub ingredients: Vec<Vec<String>>,
    pub pentacle: String,
    pub result: String,
    pub count: u32,
    /// A ritual that acts instead of giving its result (the result is what a recipe viewer shows):
    /// `RECALL_STORAGE` calls lost Storage Cores back; `REPAIR` gives the activation item back whole.
    pub effect: Option<String>,
    /// The entity kinds a player must kill near the bowl before the ritual's time runs
    /// (`entity_to_sacrifice`); empty for none.
    pub sacrifice: Vec<String>,
    /// What the caster is told to sacrifice, a lang key (`display_name`).
    pub sacrifice_name: Option<String>,
}

/// The recall ritual's `ritual_type`.
pub const RECALL_STORAGE: &str = "occultism:recall_storage";
/// Gives the activation item back whole (Occultism's `RepairRitual`).
pub const REPAIR: &str = "occultism:repair";

/// The golden bowl's ticks per second of ritual time (`ritualDurationMultiplier` 1).
pub const TICKS_PER_SECOND: u32 = 20;
/// How far around the golden bowl sacrificial bowls count, horizontally (`SACRIFICIAL_BOWL_RANGE`).
pub const BOWL_RANGE: i32 = 8;
/// How far from the golden bowl a sacrifice counts (`SACRIFICE_DETECTION_RANGE`).
pub const SACRIFICE_RANGE: i32 = 8;

/// The damage types of a player's own blow, a shot or a throw: a kill with one of these and no
/// attacker named is a player's (Horizon's API says it names a mob, not a player, though it names
/// the player too).
const PLAYER_BLOWS: [&str; 8] = [
    "minecraft:player_attack",
    "minecraft:arrow",
    "minecraft:trident",
    "minecraft:thrown",
    "minecraft:fireworks",
    "minecraft:player_explosion",
    "minecraft:mace_smash",
    "minecraft:spear",
];

/// Whether a player killed: the attacker's kind, or with none named, the blow's.
pub fn killed_by_player(attacker_kind: Option<&str>, damage_type: &str) -> bool {
    match attacker_kind {
        Some(kind) => kind == "minecraft:player",
        None => PLAYER_BLOWS.contains(&damage_type),
    }
}

/// Whether a death counts as a sacrifice for a ritual needing `kinds`: one of them, killed by a
/// player (`onLivingDeath`: the source's entity is a Player), within range of the bowl
/// (`BlockPos.distSqr`).
pub fn is_sacrifice(kinds: &[String], kind: &str, by_player: bool, bowl: [i32; 3], at: [i32; 3]) -> bool {
    let d2: i64 = (0..3).map(|i| i64::from(at[i] - bowl[i]).pow(2)).sum();
    kinds.iter().any(|k| k == kind) && by_player && d2 <= i64::from(SACRIFICE_RANGE * SACRIFICE_RANGE)
}

impl Ritual {
    /// Seconds between ingredients: the duration over one more than their number, so the last is
    /// taken before the end (`RitualRecipe.durationPerIngredient`).
    pub fn seconds_per_ingredient(&self) -> f32 {
        self.duration as f32 / (self.ingredients.len() + 1) as f32
    }

    /// How many ingredients should have been taken by `time` seconds.
    pub fn due(&self, time: u32) -> usize {
        ((time as f32 / self.seconds_per_ingredient()).floor() as usize).min(self.ingredients.len())
    }
}

/// Whether `items` (one per bowl) hold every ingredient, each bowl counted once
/// (`Ritual.matchesAdditionalIngredients`; extra items don't matter, as by default).
pub fn ingredients_present(ingredients: &[Vec<String>], items: &[String]) -> bool {
    let mut left: Vec<&String> = items.iter().collect();
    ingredients.iter().all(|accepted| match left.iter().position(|item| accepted.contains(item)) {
        Some(i) => {
            left.remove(i);
            true
        }
        None => false,
    })
}

/// The first bowl (by its index in `bowls`) holding an item `accepted` takes.
pub fn bowl_for(accepted: &[String], bowls: &[Option<String>]) -> Option<usize> {
    bowls.iter().position(|b| b.as_ref().is_some_and(|item| accepted.contains(item)))
}

/// A golden bowl's ritual, as its block entity keeps it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RitualState {
    pub ritual: Option<String>,
    pub ticks: u32,
    pub consumed: u32,
    /// The ritual's sacrifice was made.
    pub sacrificed: bool,
}

impl RitualState {
    /// Seconds of ritual time so far.
    pub fn time(&self) -> u32 {
        self.ticks / TICKS_PER_SECOND
    }
}

/// What one tick of a running ritual asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Nothing to do this tick.
    Wait,
    /// Take ingredient `index` from a bowl now (call `taken` when it is).
    Take(usize),
    /// The time is up and every ingredient is taken.
    Done,
}

/// One tick of a running ritual (the bowl's circle and activation item already checked).
pub fn advance(state: &mut RitualState, ritual: &Ritual) -> Step {
    // Until its sacrifice, a ritual's time doesn't run (`sacrificeFulfilled`).
    if !ritual.sacrifice.is_empty() && !state.sacrificed {
        return Step::Wait;
    }
    state.ticks += 1;
    let time = state.time();
    if (state.consumed as usize) < ritual.due(time) {
        return Step::Take(state.consumed as usize);
    }
    if time >= ritual.duration && state.consumed as usize >= ritual.ingredients.len() {
        return Step::Done;
    }
    Step::Wait
}

/// The ingredient was taken from its bowl.
pub fn taken(state: &mut RitualState) {
    state.consumed += 1;
}

/// The glyph signs (`ChalkGlyphBlock.SIGN`): 0 is the book's preview, 1 to 14 are drawn.
pub const MAX_SIGN: u8 = 14;

/// The sign a new glyph gets from a random `roll`, or the next one when the same chalk draws over
/// its own glyph (`ChalkGlyphBlock.getStateForPlacement`).
pub fn glyph_sign(roll: u32, current: Option<u8>) -> u8 {
    let sign = match current {
        Some(s) => (s + 1) % (MAX_SIGN + 1),
        None => (roll % (MAX_SIGN as u32 + 1)) as u8,
    };
    sign.max(1)
}

/// How many glyphs a chalk draws before it is used up (Occultism's chalk durability).
pub const CHALK_USES: u32 = 256;
const CHALK_PREFIX: &str = "occultism:chalk_";

/// The glyph block a chalk draws: `occultism:chalk_red` draws `occultism:chalk_glyph_red`. None for
/// what isn't a chalk (an impure chalk, a glyph's own id).
pub fn chalk_glyph(item: &str) -> Option<String> {
    let colour = item.strip_prefix(CHALK_PREFIX)?;
    (!colour.is_empty() && !colour.ends_with("_impure") && !colour.starts_with("glyph_"))
        .then(|| format!("{CHALK_PREFIX}glyph_{colour}"))
}

/// Where a chalk used on `clicked` draws (`ChalkItem.useOn`): into the clicked block when it can
/// be replaced (a glyph, grass), else on its top face when that is sturdy; elsewhere nowhere.
pub fn glyph_target(clicked: [i32; 3], face: [i32; 3], replaceable: bool, sturdy_top: bool) -> Option<[i32; 3]> {
    if replaceable {
        Some(clicked)
    } else if face == [0, 1, 0] && sturdy_top {
        Some([clicked[0], clicked[1] + 1, clicked[2]])
    } else {
        None
    }
}

/// The facing a glyph gets from its drawer's yaw: towards them (`getHorizontalDirection().getOpposite()`).
pub fn glyph_facing(yaw: f32) -> &'static str {
    // Direction.fromYRot: floor(yaw / 90 + 0.5) & 3 picks south, west, north, east.
    let looking = ["south", "west", "north", "east"][((yaw / 90.0 + 0.5).floor() as i32 & 3) as usize];
    match looking {
        "south" => "north",
        "west" => "east",
        "north" => "south",
        _ => "west",
    }
}

/// Raw iesnium's chance with each block of silver ore a player mines (this port's design: the
/// Third Eye shows iesnium in silver ore, so vein mining doesn't destroy it). Goggles see more.
pub const IESNIUM_CHANCE_THIRD_EYE: f64 = 0.20;
pub const IESNIUM_CHANCE_GOGGLES: f64 = 0.40;

pub fn iesnium_chance(third_eye: bool, goggles: bool) -> f64 {
    if goggles {
        IESNIUM_CHANCE_GOGGLES
    } else if third_eye {
        IESNIUM_CHANCE_THIRD_EYE
    } else {
        0.0
    }
}
