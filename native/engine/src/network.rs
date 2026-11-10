// A store as a lasting network (docs/storage-design.md, "Resilient pairing"), without Horizon
// types: its record (name, owner, capacity, where its Core is), what placing a Core does, and the
// spirit's name players know it by. Gates bind to a store's id, never to a position.

use crate::storage::Limits;

/// Where a store's Core sits.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Anchor {
    pub dimension: String,
    pub pos: [i32; 3],
}

/// What a store keeps beside its items: a few lines, rewritten only when one of them changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    /// The spirit's name players see ("bound to Azuriel").
    pub name: String,
    /// Who created it: only they recall its Core or trust others. 0 for a store from before
    /// owners, claimed by whoever next places its Core.
    pub owner: u128,
    /// Players the owner trusts with it: they open its terminal, move its Core and wake it.
    pub trusted: Vec<u128>,
    /// Its capacity when its plates were last seen, so it holds while their chunk is unloaded.
    pub limits: Limits,
    /// Where its Core is; none while the Core is an item, and the store is dormant.
    pub anchor: Option<Anchor>,
    /// Which Core the spirit is bound to: a recall binds it to a new one, and Core items from
    /// before (`bond` lower) no longer reach it.
    pub bond: u32,
}

impl Record {
    pub fn new(name: String, owner: u128, limits: Limits, anchor: Anchor) -> Record {
        Record { name, owner, limits, anchor: Some(anchor), bond: 0, trusted: Vec::new() }
    }

    /// A store is awake while its Core is placed; gates attuned to a dormant store stop.
    pub fn awake(&self) -> bool {
        self.anchor.is_some()
    }

    /// Whether `player` may use the store: its terminal, its Core, waking it.
    pub fn may_use(&self, player: u128) -> bool {
        self.owner == 0 || player == self.owner || self.trusted.contains(&player)
    }

    pub fn encode(&self) -> String {
        let anchor = match &self.anchor {
            Some(a) => format!("{} {} {} {}", a.dimension, a.pos[0], a.pos[1], a.pos[2]),
            None => String::new(),
        };
        let trusted: Vec<String> = self.trusted.iter().map(|p| format!("{p:032x}")).collect();
        format!(
            "name={}\nowner={:032x}\ntypes={}\ntotal={}\nanchor={}\nbond={}\ntrusted={}\n",
            self.name,
            self.owner,
            self.limits.types,
            self.limits.total,
            anchor,
            self.bond,
            trusted.join(",")
        )
    }

    /// A record from its file; None for one that doesn't read (it would be rebuilt).
    pub fn decode(text: &str) -> Option<Record> {
        let field = |key: &str| {
            text.lines().find_map(|l| l.strip_prefix(key).and_then(|r| r.strip_prefix('=')).map(str::trim))
        };
        let anchor = match field("anchor")? {
            "" => None,
            a => {
                let parts: Vec<&str> = a.split_whitespace().collect();
                let [dimension, x, y, z] = parts.as_slice() else { return None };
                Some(Anchor { dimension: dimension.to_string(), pos: [x.parse().ok()?, y.parse().ok()?, z.parse().ok()?] })
            }
        };
        Some(Record {
            name: field("name")?.to_string(),
            owner: u128::from_str_radix(field("owner")?, 16).ok()?,
            limits: Limits { types: field("types")?.parse().ok()?, total: field("total")?.parse().ok()? },
            anchor,
            // Records from before recalls have none.
            bond: field("bond").map_or(Some(0), |b| b.parse().ok())?,
            trusted: match field("trusted") {
                None | Some("") => Vec::new(),
                Some(list) => list.split(',').map(|p| u128::from_str_radix(p, 16).ok()).collect::<Option<_>>()?,
            },
        })
    }
}

/// What placing a Core does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Placing {
    /// A Core from crafting (no store on the item): a new store, here.
    New,
    /// The Core of store `id`, dormant until now: it wakes here.
    Wake(String),
    /// A Core naming a store whose Core is already placed elsewhere (a copy): it can't take the
    /// store over, so it starts a new one.
    Copy(String),
    /// A Core whose bond a recall broke (the spirit went to a new Core): a new store too.
    Stale(String),
    /// A dormant store's Core placed by someone it doesn't trust (taken from its owner): the
    /// store stays dormant, so its owner can still recall it, and the Core does nothing.
    Refused(String),
}

/// What a Core placed by `player` with `carried` (the store its item names and its bond, if any)
/// does, given that store's record.
pub fn placing(carried: Option<(&str, u32)>, record: Option<&Record>, player: u128) -> Placing {
    match (carried, record) {
        (None, _) => Placing::New,
        (Some((id, bond)), Some(r)) if bond < r.bond => Placing::Stale(id.to_string()),
        (Some((id, _)), Some(r)) if r.awake() => Placing::Copy(id.to_string()),
        (Some((id, _)), Some(r)) if !r.may_use(player) => Placing::Refused(id.to_string()),
        (Some((id, _)), _) => Placing::Wake(id.to_string()),
    }
}

/// The stores a recall ritual by `owner` calls back: theirs, with no Core placed (dormant, or its
/// place `gone`), and none of whose Cores (of the current bond) the caster carries. `stores` is
/// each store's id, record and whether its anchor is gone; `carried` the (id, bond) of each Core
/// in the caster's inventory.
pub fn lost<'a>(stores: impl IntoIterator<Item = (&'a str, &'a Record, bool)>, owner: u128, carried: &[(String, u32)]) -> Vec<&'a str> {
    let mut out: Vec<&str> = stores
        .into_iter()
        .filter(|(id, r, gone)| {
            r.owner == owner && (!r.awake() || *gone) && !carried.iter().any(|(c, bond)| c == id && *bond >= r.bond)
        })
        .map(|(id, _, _)| id)
        .collect();
    out.sort_unstable();
    out
}

// Occultism's spirit names (`TextUtil.generateName`): three syllables, or one of its community's
// easter eggs one time in twenty.
const EASTER_EGGS: [&str; 16] = [
    "KliKli", "Xalmas", "Toastbroat", "Najlitarvan", "TheBoo", "Ridanisaurus", "Legiaseth", "Vallen", "Vemerion",
    "EqisEdu", "Feellian", "Ginkrepeerc", "Kat", "Chonky", "Ciege", "Chanticleer",
];
const SYLLABLE1: [&str; 59] = [
    "Kr", "Ca", "Ra", "Mrok", "Cru", "Ray", "Bre", "Zed", "Drak", "Mor", "Jag", "Mer", "Jar", "Mjol", "Zork", "Mad",
    "Cry", "Zur", "Creo", "Azak", "Azur", "Rei", "Cro", "Mar", "Luk", "Bar", "Gor", "Rak", "Thr", "Nar", "Vor", "Fir",
    "Trin", "Drog", "Karn", "Gar", "Ulf", "Hroth", "Ald", "Yng", "Styr", "Eir", "Ein", "Sig", "Ket", "Erl", "Haf",
    "Bryn", "Nid", "Grim", "Hol", "Fen", "Sigr", "Geir", "Hyr", "Val", "Har", "Kol", "Eyr",
];
const SYLLABLE2: [&str; 64] = [
    "air", "ir", "mi", "sor", "mee", "clo", "red", "cra", "ark", "arc", "miri", "lori", "cres", "mur", "zer", "marac",
    "zoir", "slamar", "salmar", "urak", "tim", "jor", "vyr", "dor", "thor", "kyl", "lyn", "wyn", "wynn", "lond", "rond",
    "vond", "dorn", "korn", "morn", "gorn", "thorn", "worn", "norn", "rinn", "dell", "bell", "vell", "fell", "kell",
    "zell", "nir", "fir", "mir", "tir", "sir", "vir", "zir", "lir", "jyr", "ryl", "rym", "lym", "lon", "ron", "myr",
    "myl", "myn", "ryn",
];
const SYLLABLE3: [&str; 59] = [
    "d", "ed", "ark", "arc", "es", "er", "der", "tron", "med", "ure", "zur", "cred", "mur", "aeus", "th", "vyr", "dor",
    "morn", "born", "thork", "fyr", "lyr", "ryth", "ryn", "drin", "dryn", "kyr", "kyn", "lynd", "lind", "lyne", "line",
    "ryne", "rine", "thyr", "thyre", "vyn", "vin", "vyne", "vine", "rynne", "rinne", "syr", "syrn", "zirn", "zirne",
    "kyl", "kylle", "dyr", "dorne", "lor", "lorne", "mon", "morne", "thorn", "thorne", "vyrn", "vyrne", "wyrm",
];

/// A spirit's name from `seed` (any random number), as Occultism names its spirits.
pub fn spirit_name(seed: u64) -> String {
    let pick = |n: u64, len: usize| (n % len as u64) as usize;
    let mix = |n: u64| n.wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(29);
    let (a, b, c, d) = (seed, mix(seed), mix(mix(seed)), mix(mix(mix(seed))));
    if a % 20 == 0 {
        return EASTER_EGGS[pick(b, EASTER_EGGS.len())].to_string();
    }
    format!("{}{}{}", SYLLABLE1[pick(b, SYLLABLE1.len())], SYLLABLE2[pick(c, SYLLABLE2.len())], SYLLABLE3[pick(d, SYLLABLE3.len())])
}
