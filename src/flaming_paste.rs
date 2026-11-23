// Occultism's flaming paste (`FlamingPasteItem`): its template, flint and steel, lights fires,
// campfires, candles and TNT. This gives it the rest: a mob it touches or hits burns, sneaking it
// pours lava, and with gray paste in the other hand it throws a fireball. Eating any of the pastes
// costs two uses, not the paste (`DamageItemConsumeEffect`).

use crate::game;
use mod_api::*;

pub const FLAMING_PASTE: &str = "occultism:flaming_paste";
const GRAY_PASTE: &str = "occultism:gray_paste";
const PASTES: [&str; 3] = [GRAY_PASTE, "occultism:nature_paste", FLAMING_PASTE];
/// How long a mob it touches or hits burns, in ticks (30 seconds).
const BURN: i32 = 30 * 20;
/// The wait after setting a mob alight, and after a fireball, in ticks.
const TOUCH_COOLDOWN: i32 = 20;
const FIREBALL_COOLDOWN: i32 = 10;
/// The uses a pour of lava takes.
const LAVA_USES: i32 = 8;
/// The uses a meal of paste takes.
const MEAL_USES: u32 = 2;

fn slot(hand: Hand) -> EquipmentSlot {
    match hand {
        Hand::Main => EquipmentSlot::MainHand,
        Hand::Off => EquipmentSlot::OffHand,
    }
}

fn other(hand: Hand) -> Hand {
    match hand {
        Hand::Main => Hand::Off,
        Hand::Off => Hand::Main,
    }
}

pub struct PastePart;

#[mod_api::sunrise_part]
impl ItemPart for PastePart {
    /// A mob hit with it burns, and the paste wears (`postHurtEnemy`).
    fn hurt_enemy(&self, host: &mut dyn ServerHost, event: &mut HurtEnemyEvent<'_>) {
        if let Some(target) = host.entity(event.entity) {
            target.set_remaining_fire_ticks(host, BURN);
        }
        if let Some(player) = host.players().acting_player().and_then(|p| game::living(&*host, p.uuid())) {
            player.hurt_and_break(host, EquipmentSlot::MainHand, 1);
        }
    }

    /// With gray paste in the other hand, a small fireball where the player looks, each paste a use
    /// worn (`use`).
    fn use_item(&self, host: HostSide<'_>, event: &mut UseItemEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else { return None };
        let living = game::living(&*host, event.player)?;
        let off = living.hand_item(&*host, other(event.hand))?;
        if game::id(&*host, &off) != GRAY_PASTE {
            return None;
        }
        let player = host.player(event.player)?;
        if player.has_item_cooldown(&*host, FLAMING_PASTE) == Some(true) {
            return Some(InteractionResult::Fail);
        }
        let entity = host.entity(u32::try_from(event.player_entity_id).ok()?)?;
        let (eye, look) = (entity.eye_position(&*host)?, entity.view_vector(&*host)?);
        let level = host.level(host.dimension())?;
        let mut nbt = Compound::new();
        let motion = look.map(|v| Tag::Double((v * 1.5).to_bits()));
        nbt.insert("Motion".into(), Tag::List(motion.to_vec()));
        nbt.insert("Owner".into(), Tag::IntArray(uuid_ints(event.player).to_vec()));
        let at = [eye[0] + look[0], eye[1] + look[1], eye[2] + look[2]];
        host.summon(level.dimension_id(), "minecraft:small_fireball", at, nbt);
        crate::platform::play_sound(host, level, "minecraft:entity.firework_rocket.launch", eye, SoundSource::Players, 0.5, 0.45);
        living.hurt_and_break(host, slot(event.hand), 1);
        // The gray paste, set back a use worn (its `hurt_and_break` in the other hand doesn't take).
        let mut gray = off;
        let worn = gray.damage() + 1;
        let left = if worn >= 64 {
            None
        } else {
            gray.set_damage(worn);
            Some(gray)
        };
        let _ = living.set_hand_item(host, other(event.hand), left);
        player.set_item_cooldown(host, FLAMING_PASTE, FIREBALL_COOLDOWN);
        Some(InteractionResult::SuccessServer)
    }

    /// Sneaking, it pours lava against the face clicked, as a lava bucket would (`placeLava`).
    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else { return None };
        let entity = host.entity(u32::try_from(event.player_entity_id).ok()?)?;
        let level = host.level(host.dimension())?;
        let at = [event.pos[0] + event.face[0], event.pos[1] + event.face[1], event.pos[2] + event.face[2]];
        let there = level.block_state(&*host, at).and_then(|s| host.block_state_string(s)).unwrap_or_default();
        let may_build = host.player(event.player).and_then(|p| p.may_build(&*host)) == Some(true);
        if entity.is_shift_key_down(&*host) != Some(true) {
            return Some(if may_build && light(host, level, event.pos, at, &there) {
                if let Some(living) = game::living(&*host, event.player) {
                    living.hurt_and_break(host, slot(event.hand), 1);
                }
                InteractionResult::SuccessServer
            } else {
                InteractionResult::Fail
            });
        }
        if !may_build || !crate::fire::may_light(&there) || !game::set_block(host, level, at, "minecraft:lava") {
            return Some(InteractionResult::Fail);
        }
        let centre = at.map(|c| f64::from(c) + 0.5);
        crate::platform::play_sound(host, level, "minecraft:item.bucket.empty_lava", centre, SoundSource::Blocks, 0.8, 1.0);
        if let Some(living) = game::living(&*host, event.player) {
            living.hurt_and_break(host, slot(event.hand), LAVA_USES);
        }
        Some(InteractionResult::SuccessServer)
    }
}

/// What flint and steel does where it's used (`FlintAndSteelItem.useOn`, `TntBlock.onCaughtFire`):
/// lights the campfire, candle or candle cake `clicked`, primes TNT, or sets fire at `at`, the place
/// against the face clicked (soul fire on soul sand or soil). Whether it did.
fn light(host: &mut dyn ServerHost, level: LevelRef, clicked: [i32; 3], at: [i32; 3], there: &str) -> bool {
    let block = game::block_name(&*host, level.block(&*host, clicked));
    let centre = |p: [i32; 3]| p.map(|c| f64::from(c) + 0.5);
    let lightable = block.ends_with("campfire") || block.ends_with("candle") || block.ends_with("candle_cake");
    if lightable
        && level.block_property(&*host, clicked, "lit").as_deref() == Some("false")
        && level.block_property(&*host, clicked, "waterlogged").as_deref() != Some("true")
    {
        host.set_block_property(clicked, "lit", "true");
        crate::platform::play_sound(host, level, "minecraft:item.flintandsteel.use", centre(clicked), SoundSource::Blocks, 0.8, 1.0);
        return true;
    }
    if block == "minecraft:tnt" {
        game::set_block(host, level, clicked, "minecraft:air");
        let mut nbt = Compound::new();
        nbt.insert("fuse".into(), Tag::Short(80));
        let at = centre(clicked);
        host.summon(level.dimension_id(), "minecraft:tnt", [at[0], at[1] - 0.5, at[2]], nbt);
        return true;
    }
    if !crate::fire::may_light(there) || there.starts_with("minecraft:fire") {
        return false;
    }
    let below = game::block_name(&*host, level.block(&*host, [at[0], at[1] - 1, at[2]]));
    let fire = if matches!(below, "minecraft:soul_sand" | "minecraft:soul_soil") { "minecraft:soul_fire" } else { "minecraft:fire" };
    if !game::set_block(host, level, at, fire) {
        return false;
    }
    crate::platform::play_sound(host, level, "minecraft:item.firecharge.use", centre(at), SoundSource::Blocks, 0.3, 1.0);
    true
}

/// A mob a player touches with flaming paste burns, the paste a use worn (`interactLivingEntity`):
/// whether it did.
pub fn touch(host: &mut dyn ServerHost, player: u128, hand: Hand, target: u32) -> bool {
    let (Some(living), Some(who)) = (game::living(&*host, player), host.player(player)) else { return false };
    if who.has_item_cooldown(&*host, FLAMING_PASTE) == Some(true) {
        return false;
    }
    let Some(target) = host.entity(target) else { return false };
    if host.living(target).and_then(|l| l.alive(&*host)) != Some(true) {
        return false;
    }
    target.set_remaining_fire_ticks(host, BURN);
    if let (Some(at), Some(level)) = (target.position(&*host), host.level(host.dimension())) {
        crate::platform::play_sound(host, level, "minecraft:item.firecharge.use", at, SoundSource::Players, 0.3, 1.0);
    }
    living.hurt_and_break(host, slot(hand), 1);
    who.set_item_cooldown(host, FLAMING_PASTE, TOUCH_COOLDOWN);
    true
}

/// What eating a paste leaves in the hand: the paste two uses worn, or nothing once they're spent.
pub fn after_meal(host: &dyn Host, eaten: &ItemStack) -> Option<Option<ItemStack>> {
    let id = game::id(host, eaten);
    if !PASTES.contains(&id) {
        return None;
    }
    let uses = if id == FLAMING_PASTE { 256 } else { 64 };
    let worn = eaten.damage() + MEAL_USES;
    if worn >= uses {
        return Some(None);
    }
    let mut left = eaten.clone();
    let _ = left.set_count(1);
    left.set_damage(worn);
    Some(Some(left))
}

/// A UUID as the four ints an entity's `Owner` takes.
fn uuid_ints(uuid: u128) -> [i32; 4] {
    [(uuid >> 96) as i32, (uuid >> 64) as i32, (uuid >> 32) as i32, uuid as i32]
}
