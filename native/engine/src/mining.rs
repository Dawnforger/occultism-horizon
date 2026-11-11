// The dimensional mineshaft's rules, without Horizon types (Occultism's
// `DimensionalMineshaftBlockEntity`): a miner spirit in its lamp mines on a timer, each operation
// rolling its results by weight from the `occultism:miner` recipes its lamp's tag matches, and
// wears the lamp by one.

/// What a miner's lamp says about it (`occultism:miner` on the item): ticks per operation, results
/// rolled per operation, and how many of each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Miner {
    pub ticks: u32,
    pub rolls: u32,
    pub multiplier: u32,
}

/// Occultism's defaults when a lamp carries none (`DEFAULT_MAX_MINING_TIME` and the rest).
pub const DEFAULT_MINER: Miner = Miner { ticks: 400, rolls: 1, multiplier: 1 };

/// The mineshaft's slots: the lamp, then nine outputs.
pub const SLOT_LAMP: usize = 0;
pub const OUTPUTS: std::ops::Range<usize> = 1..10;
pub const SLOT_COUNT: usize = 10;

/// A mineshaft's progress: ticks left of the operation under way, 0 when none is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shaft {
    pub left: u32,
    pub of: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    /// No lamp: nothing happens.
    Idle,
    /// Working: so many ticks of so many left.
    Working,
    /// An operation finished: roll its results.
    Mine,
}

/// One tick of the mineshaft with `miner` in it (or none). A lamp taken out, or swapped, starts
/// over, as Occultism's does.
pub fn step(shaft: &mut Shaft, miner: Option<Miner>) -> Step {
    let Some(miner) = miner else {
        *shaft = Shaft::default();
        return Step::Idle;
    };
    if shaft.left == 0 || shaft.of != miner.ticks.max(1) {
        *shaft = Shaft { left: miner.ticks.max(1), of: miner.ticks.max(1) };
        return Step::Working;
    }
    shaft.left -= 1;
    if shaft.left == 0 {
        Step::Mine
    } else {
        Step::Working
    }
}

/// The result `roll` (any random number) picks from `weights`, each in proportion to its weight
/// (`WeightedRandom.getRandomItem`); none when nothing weighs anything.
pub fn pick(weights: &[u32], roll: u64) -> Option<usize> {
    let total: u64 = weights.iter().map(|w| u64::from(*w)).sum();
    if total == 0 {
        return None;
    }
    let mut at = roll % total;
    for (i, w) in weights.iter().enumerate() {
        if at < u64::from(*w) {
            return Some(i);
        }
        at -= u64::from(*w);
    }
    None
}

/// The batch an operation yields: each roll's result `count` times the miner's multiplier, equal
/// results merged (`batchedDrops`). `rolls` are the indices `pick` chose, `counts` each result's
/// count.
pub fn batch(rolls: &[usize], counts: &[u32], miner: Miner) -> Vec<(usize, u32)> {
    let mut out: Vec<(usize, u32)> = Vec::new();
    for &r in rolls {
        let n = counts.get(r).copied().unwrap_or(1) * miner.multiplier.max(1);
        match out.iter_mut().find(|(i, _)| *i == r) {
            Some((_, c)) => *c += n,
            None => out.push((r, n)),
        }
    }
    out
}

/// A lamp's wear after one operation: its new damage, or None when that wore it out (Occultism's
/// default drops nothing for a spent miner: `minerOutputBeforeBreak` is off).
pub fn wear(damage: u32, max_damage: u32) -> Option<u32> {
    (max_damage == 0 || damage + 1 < max_damage).then_some(damage + 1)
}
