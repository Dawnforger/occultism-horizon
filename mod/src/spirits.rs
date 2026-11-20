// Occultism's unbound spirits as this port's mob types (content: `data/occultism/horizon/entity_type`,
// each a zombie with Occultism's attributes): what a zombie does that they don't, and what they do
// that a zombie doesn't (`AfritUnboundEntity`, `MaridUnboundEntity`).

use mod_api::*;

pub const AFRIT: &str = "occultism:afrit_unbound";
pub const MARID: &str = "occultism:marid_unbound";

/// A mob kind as a hook names it: some hooks put `minecraft:` before a mod's own kind
/// (`minecraft:occultism:marid_unbound`), which this takes back off.
pub fn kind(raw: &str) -> &str {
    raw.strip_prefix("minecraft:").filter(|rest| rest.contains(':')).unwrap_or(raw)
}

/// Whether `kind` is one of this port's spirits.
pub fn is_spirit(kind: &str) -> bool {
    let kind = self::kind(kind);
    kind == AFRIT || kind == MARID
}

/// What a spirit brings when a ritual summons it (`finalizeSpawn`'s blazes and guardians).
pub fn helper(kind: &str) -> Option<&'static str> {
    match kind {
        AFRIT => Some("minecraft:blaze"),
        MARID => Some("minecraft:guardian"),
        _ => None,
    }
}

/// Damage types in vanilla's `#is_fire`, `#is_drowning`, `#is_freezing` and `#is_explosion`.
const FIRE: [&str; 7] =
    ["minecraft:in_fire", "minecraft:campfire", "minecraft:on_fire", "minecraft:lava", "minecraft:hot_floor", "minecraft:fireball", "minecraft:unattributed_fireball"];
const DROWNING: [&str; 1] = ["minecraft:drown"];
const FREEZING: [&str; 1] = ["minecraft:freeze"];
const EXPLOSION: [&str; 4] = ["minecraft:explosion", "minecraft:player_explosion", "minecraft:fireworks", "minecraft:wind_charge"];
/// `#witch_resistant_to`.
const WITCH_RESISTANT: [&str; 4] = ["minecraft:magic", "minecraft:indirect_magic", "minecraft:sonic_boom", "minecraft:thorns"];

/// Whether a spirit of `kind` takes no damage of `damage_type` dealt by an entity of `causing`
/// (`isInvulnerableTo`): an afrit no fire, nor its blazes' hits (`#occultism:afrit_allies`); a
/// marid no fire, drowning, freezing, explosions or magic.
pub fn immune(kind: &str, damage_type: &str, causing: Option<&str>) -> bool {
    match self::kind(kind) {
        AFRIT => FIRE.contains(&damage_type) || causing == Some("minecraft:blaze"),
        MARID => [&FIRE[..], &DROWNING, &FREEZING, &EXPLOSION, &WITCH_RESISTANT].iter().any(|set| set.contains(&damage_type)),
        _ => false,
    }
}

/// What a summoned spirit is given besides its name and facing: a head item, which keeps a zombie
/// from burning in the sun (the native mod draws the spirit, and leaves armour out), never dropped.
pub fn summon_nbt(kind: &str) -> Option<Vec<(String, Tag)>> {
    if !is_spirit(kind) {
        return None;
    }
    let mut head = Compound::new();
    head.insert("id".into(), Tag::String("minecraft:carved_pumpkin".into()));
    head.insert("count".into(), Tag::Int(1));
    let mut equipment = Compound::new();
    equipment.insert("head".into(), Tag::Compound(head));
    let mut chances = Compound::new();
    chances.insert("head".into(), Tag::Float(0f32.to_bits()));
    Some(vec![("equipment".into(), Tag::Compound(equipment)), ("drop_chances".into(), Tag::Compound(chances))])
}

/// Occultism's possessed creatures this port has. Each is the vanilla creature it possesses (a mod
/// creature here copies only a few vanilla ones, none of these), with Occultism's attributes
/// (`createAttributes`, but for attack speed, which these creatures don't have) and its own drops.
const POSSESSED: [(&str, &str, &[(&str, f64)]); 3] = [
    (
        "occultism:possessed_bee",
        "minecraft:bee",
        &[("minecraft:max_health", 40.0), ("minecraft:movement_speed", 0.4), ("minecraft:attack_damage", 4.0), ("minecraft:follow_range", 48.0)],
    ),
    ("occultism:mercy_goat", "minecraft:goat", &[("minecraft:max_health", 1.0), ("minecraft:movement_speed", 0.0)]),
    (
        "occultism:possessed_zombified_piglin",
        "minecraft:zombified_piglin",
        &[
            ("minecraft:spawn_reinforcements", 0.0),
            ("minecraft:movement_speed", 0.26),
            ("minecraft:attack_damage", 6.0),
            ("minecraft:armor", 6.0),
            ("minecraft:max_health", 80.0),
            ("minecraft:follow_range", 35.0),
            ("minecraft:armor_toughness", 25.0),
        ],
    ),
];

/// The creature a ritual summons for Occultism's possessed `kind`, and what it is given: its
/// attributes, full health and its loot table (`occultism:entities/<name>`).
pub fn possessed(kind: &str) -> Option<(&'static str, Vec<(String, Tag)>)> {
    let (_, vanilla, attributes) = POSSESSED.iter().find(|(k, _, _)| *k == kind)?;
    let list = attributes
        .iter()
        .map(|(id, base)| {
            let mut a = Compound::new();
            a.insert("id".into(), Tag::String((*id).into()));
            a.insert("base".into(), Tag::Double(base.to_bits()));
            Tag::Compound(a)
        })
        .collect();
    let health = attributes.iter().find(|(id, _)| *id == "minecraft:max_health").map_or(1.0, |(_, h)| *h) as f32;
    let name = kind.split_once(':').map_or(kind, |(_, n)| n);
    Some((
        vanilla,
        vec![
            ("attributes".into(), Tag::List(list)),
            ("Health".into(), Tag::Float(health.to_bits())),
            ("DeathLootTable".into(), Tag::String(format!("occultism:entities/{name}").as_str().into())),
        ],
    ))
}
