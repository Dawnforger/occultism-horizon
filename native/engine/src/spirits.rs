// Occultism's unbound spirits as this port's mob types (content: `data/occultism/horizon/entity_type`,
// each a zombie with Occultism's attributes): what a zombie does that they don't, and what they do
// that a zombie doesn't (`AfritUnboundEntity`, `MaridUnboundEntity`).

use horizon_client::mod_api::*;

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
