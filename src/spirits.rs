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

/// Occultism's possessed creatures this port has, and its wild rituals' (`horde/`). Each is the
/// vanilla creature it possesses (a mod creature here copies only a few vanilla ones, none of
/// these), with Occultism's attributes (`createAttributes`, but for attack speed, which these
/// creatures don't have) and its own drops.
const POSSESSED: [(&str, &str, &[(&str, f64)]); 36] = [
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
    (
        "occultism:possessed_blaze",
        "minecraft:blaze",
        &[("minecraft:movement_speed", 0.4), ("minecraft:max_health", 66.6), ("minecraft:attack_damage", 12.0), ("minecraft:safe_fall_distance", 2147483647.0)],
    ),
    ("occultism:possessed_elder_guardian", "minecraft:elder_guardian", &[("minecraft:max_health", 160.0)]),
    ("occultism:possessed_enderman", "minecraft:enderman", &[]),
    ("occultism:possessed_endermite", "minecraft:endermite", &[]),
    ("occultism:possessed_ghast", "minecraft:ghast", &[]),
    ("occultism:possessed_guardian", "minecraft:guardian", &[("minecraft:max_health", 60.0), ("minecraft:armor", 6.0)]),
    ("occultism:possessed_hoglin", "minecraft:hoglin", &[("minecraft:attack_damage", 12.0), ("minecraft:max_health", 66.0)]),
    ("occultism:possessed_phantom", "minecraft:phantom", &[]),
    ("occultism:possessed_shulker", "minecraft:shulker", &[("minecraft:max_health", 40.0)]),
    ("occultism:possessed_skeleton", "minecraft:skeleton", &[("minecraft:attack_damage", 4.0), ("minecraft:max_health", 30.0)]),
    (
        "occultism:possessed_warden",
        "minecraft:warden",
        &[("minecraft:attack_damage", 15.0), ("minecraft:max_health", 250.0), ("minecraft:movement_speed", 0.6), ("minecraft:attack_knockback", 0.5)],
    ),
    ("occultism:possessed_weak_shulker", "minecraft:shulker", &[("minecraft:max_health", 10.0)]),
    ("occultism:possessed_witch", "minecraft:witch", &[("minecraft:movement_speed", 0.3), ("minecraft:max_health", 35.0)]),
    (WILD_CREEPER, "minecraft:creeper", &[("minecraft:armor", 20.0), ("minecraft:max_health", 5.0), ("minecraft:knockback_resistance", 1.0)]),
    (
        "occultism:wild_horde_drowned",
        "minecraft:drowned",
        &[
            ("minecraft:attack_damage", 4.0),
            ("minecraft:armor", 10.0),
            ("minecraft:max_health", 30.0),
            ("minecraft:knockback_resistance", 1.0),
            ("minecraft:movement_speed", 0.8),
        ],
    ),
    (
        "occultism:wild_horde_husk",
        "minecraft:husk",
        &[("minecraft:attack_damage", 4.0), ("minecraft:armor", 10.0), ("minecraft:max_health", 30.0), ("minecraft:knockback_resistance", 1.0)],
    ),
    (
        "occultism:wild_horde_parched",
        "minecraft:parched",
        &[("minecraft:attack_damage", 4.0), ("minecraft:armor", 10.0), ("minecraft:max_health", 30.0), ("minecraft:knockback_resistance", 1.0)],
    ),
    ("occultism:wild_horde_silverfish", "minecraft:silverfish", &[("minecraft:attack_damage", 9.0)]),
    (WILD_HUNT_LEADER, "minecraft:wither_skeleton", &[("minecraft:attack_damage", 6.0), ("minecraft:max_health", 60.0)]),
    (WILD_HUNT_SKELETON, "minecraft:skeleton", &[("minecraft:attack_damage", 4.0), ("minecraft:max_health", 20.0)]),
    (EVOKER, "minecraft:evoker", &[("minecraft:max_health", 42.0), ("minecraft:armor", 7.0)]),
    (WEAK_BREEZE, "minecraft:breeze", &[]),
    (BREEZE, "minecraft:breeze", &[("minecraft:max_health", 150.0)]),
    (STRONG_BREEZE, "minecraft:breeze", &[("minecraft:max_health", 300.0)]),
    // The trial breezes' followers (`horde/Wild*Entity`).
    ("occultism:wild_zombie", "minecraft:zombie", TRIAL_WEAK),
    ("occultism:wild_skeleton", "minecraft:skeleton", TRIAL_WEAK),
    ("occultism:wild_silverfish", "minecraft:silverfish", TRIAL_WEAK),
    ("occultism:wild_spider", "minecraft:spider", TRIAL),
    ("occultism:wild_stray", "minecraft:stray", TRIAL),
    ("occultism:wild_cave_spider", "minecraft:cave_spider", TRIAL),
    ("occultism:wild_husk", "minecraft:husk", TRIAL_STRONG),
    ("occultism:wild_bogged", "minecraft:bogged", TRIAL_STRONG),
    (WILD_SLIME, "minecraft:slime", &[]),
];

const TRIAL_WEAK: &[(&str, f64)] = &[("minecraft:max_health", 30.0), ("minecraft:armor", 5.0), ("minecraft:knockback_resistance", 0.25)];
const TRIAL: &[(&str, f64)] = &[("minecraft:max_health", 45.0), ("minecraft:armor", 10.0), ("minecraft:knockback_resistance", 0.5)];
const TRIAL_STRONG: &[(&str, f64)] = &[("minecraft:max_health", 60.0), ("minecraft:armor", 15.0), ("minecraft:knockback_resistance", 0.75)];
const WEAK_BREEZE: &str = "occultism:possessed_weak_breeze";
const BREEZE: &str = "occultism:possessed_breeze";
const STRONG_BREEZE: &str = "occultism:possessed_strong_breeze";
pub const WILD_SLIME: &str = "occultism:wild_slime";

const WILD_CREEPER: &str = "occultism:wild_horde_creeper";
const WILD_HUNT_LEADER: &str = "occultism:wild_hunt_wither_skeleton";
pub const WILD_HUNT_SKELETON: &str = "occultism:wild_hunt_skeleton";
const EVOKER: &str = "occultism:possessed_evoker";

/// `kind`, if one of Occultism's possessed or wild creatures this port summons as a vanilla one.
pub fn possessed_id(kind: &str) -> Option<&'static str> {
    POSSESSED.iter().map(|(k, _, _)| *k).find(|k| *k == kind)
}

/// Whether a creature of the vanilla `kind` may be one of those.
pub fn may_be_possessed(kind: &str) -> bool {
    POSSESSED.iter().any(|(_, v, _)| *v == kind)
}

/// The tag a creature standing for Occultism's `kind` carries: `occultism.wild_horde_creeper`, which
/// a command's selector can name (`@e[tag=occultism.wild_horde_creeper]`).
fn tag_of(kind: &str) -> String {
    kind.replacen(':', ".", 1)
}

/// The Occultism creature a vanilla one stands for, from its tags (`Entity.getTags`), where its
/// summoning put it so that it is known after a reload.
pub fn possessed_kind(tags: &[String]) -> Option<&'static str> {
    POSSESSED.iter().map(|(k, _, _)| *k).find(|k| tags.iter().any(|t| *t == tag_of(k)))
}

/// The wild creatures that don't hurt each other (`isInvulnerableTo` with `#occultism:wild_hunt`,
/// `#occultism:wild_desert`).
fn horde(kind: &str) -> Option<&'static str> {
    match kind {
        WILD_HUNT_LEADER | WILD_HUNT_SKELETON => Some("wild_hunt"),
        "occultism:wild_horde_husk" | "occultism:wild_horde_parched" => Some("wild_desert"),
        // `#occultism:wild_trial`: the breezes and their followers.
        k if k.contains("breeze") || TRIAL_FOLLOWERS.contains(&k) => Some("wild_trial"),
        _ => None,
    }
}

/// Whether Occultism's possessed `kind` takes no damage of `damage_type` from creatures of
/// `sources` (the one causing it and the one dealing it, as Occultism's possessed kinds where they
/// are): a wild creeper no explosions; the wild hunt and the desert horde nothing from their own.
pub fn possessed_immune(kind: &str, damage_type: &str, sources: [Option<&str>; 2]) -> bool {
    if kind == WILD_CREEPER && EXPLOSION.contains(&damage_type) {
        return true;
    }
    horde(kind).is_some_and(|h| sources.iter().flatten().any(|s| horde(s) == Some(h)))
}

const TRIAL_FOLLOWERS: [&str; 9] = [
    "occultism:wild_zombie",
    "occultism:wild_skeleton",
    "occultism:wild_silverfish",
    "occultism:wild_spider",
    "occultism:wild_stray",
    "occultism:wild_cave_spider",
    "occultism:wild_husk",
    "occultism:wild_bogged",
    WILD_SLIME,
];

/// How many followers it takes to stop all of a leader's damage: each living one takes off its
/// share (`actuallyHurt`'s `1 - minions / n`), and the followers glow. None for a creature whose
/// followers don't shield it.
pub fn shielded_by(kind: &str) -> Option<f32> {
    match kind {
        WILD_HUNT_LEADER | WEAK_BREEZE => Some(10.0),
        BREEZE => Some(16.0),
        STRONG_BREEZE => Some(24.0),
        _ => None,
    }
}

/// Whether Occultism's possessed `kind` keeps from turning into another creature: the desert
/// horde's husks don't drown into zombies (`convertsInWater`).
pub fn keeps_form(kind: &str) -> bool {
    horde(kind) == Some("wild_desert")
}

/// The creatures a wild one brings when it is summoned (`finalizeSpawn`), when mobs spawn: the wild
/// hunt's three to eight skeletons, the evoker's illagers. `roll` is any random number. The last
/// of each is the health it has, if not its own.
pub fn followers(kind: &str, roll: u64) -> Vec<(&'static str, u32, Option<f32>)> {
    match kind {
        WILD_HUNT_LEADER => vec![(WILD_HUNT_SKELETON, 3 + (roll % 6) as u32, None)],
        EVOKER => vec![
            ("minecraft:vindicator", 2, None),
            ("minecraft:pillager", 5, None),
            ("minecraft:illusioner", 1, None),
            ("minecraft:ravager", 1, Some(10.0)),
        ],
        WEAK_BREEZE => vec![("occultism:wild_zombie", 3, None), ("occultism:wild_skeleton", 3, None), ("occultism:wild_silverfish", 3, None)],
        BREEZE => vec![("occultism:wild_spider", 5, None), ("occultism:wild_stray", 5, None), ("occultism:wild_cave_spider", 5, None)],
        // The strong breeze's slimes are of each size from 1 to 7 (`setSize(i, true)`).
        STRONG_BREEZE => vec![("occultism:wild_husk", 7, None), ("occultism:wild_bogged", 7, None), (WILD_SLIME, 7, None)],
        _ => Vec::new(),
    }
}

/// The creature a ritual summons for Occultism's possessed `kind`, and what it is given: its
/// attributes, full health and its loot table (`occultism:entities/<name>`). A possessed hoglin is
/// grown, so that it drops (`PossessedHoglinEntity.finalizeSpawn`).
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
    let name = kind.split_once(':').map_or(kind, |(_, n)| n);
    let mut nbt = vec![
        ("attributes".into(), Tag::List(list)),
        ("DeathLootTable".into(), Tag::String(format!("occultism:entities/{name}").as_str().into())),
    ];
    // Full health of its new maximum; a creature whose health isn't Occultism's keeps its own.
    if let Some((_, health)) = attributes.iter().find(|(id, _)| *id == "minecraft:max_health") {
        nbt.push(("Health".into(), Tag::Float((*health as f32).to_bits())));
    }
    if *vanilla == "minecraft:hoglin" {
        nbt.push(("Age".into(), Tag::Int(0)));
    }
    // A wild creeper is charged (`isPowered`).
    if kind == WILD_CREEPER {
        nbt.push(("powered".into(), Tag::Byte(1)));
    }
    // What it stands for, kept in its tags.
    nbt.push(("Tags".into(), Tag::List(vec![Tag::String(tag_of(kind).as_str().into())])));
    Some((vanilla, nbt))
}
