// Occultism's flaming paste (`FlamingPasteItem`): it lights the campfire or candle it is used on,
// and a mob it touches or hits burns. Eating any of the pastes costs two uses, not the paste
// (`DamageItemConsumeEffect`).
//
// Not here at this SDK pin (Horizon registry review, horizon-mods#1742): setting fire in the block
// beside the one clicked and pouring lava there, which write a block the game hasn't checked the
// player may change (spawn protection, the world border: `Level.mayInteract`, which the SDK can't
// ask); the fireball thrown with gray paste, whose impact sets fire the same way; and priming TNT,
// which would pass over the server's own priming checks (its cancellation hook, `tnt_explodes`).
// The clicked block itself the game checks before the item is used.

use crate::game;
use mod_api::*;

pub const FLAMING_PASTE: &str = "occultism:flaming_paste";
const GRAY_PASTE: &str = "occultism:gray_paste";
const PASTES: [&str; 3] = [GRAY_PASTE, "occultism:nature_paste", FLAMING_PASTE];
/// How long a mob it touches or hits burns, in ticks (30 seconds).
const BURN: i32 = 30 * 20;
/// The wait after setting a mob alight, in ticks.
const TOUCH_COOLDOWN: i32 = 20;
/// The uses a meal of paste takes.
const MEAL_USES: u32 = 2;

fn slot(hand: Hand) -> EquipmentSlot {
    match hand {
        Hand::Main => EquipmentSlot::MainHand,
        Hand::Off => EquipmentSlot::OffHand,
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

    /// On a campfire, candle or candle cake it lights it (`FlintAndSteelItem.useOn`): the clicked
    /// block only, which the game checked before this use.
    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        let HostSide::Server(host) = host else { return None };
        let level = host.level(host.dimension())?;
        let may_build = host.player(event.player).and_then(|p| p.may_build(&*host)) == Some(true);
        if !may_build || !light(host, level, event.pos) {
            return Some(InteractionResult::Fail);
        }
        if let Some(living) = game::living(&*host, event.player) {
            living.hurt_and_break(host, slot(event.hand), 1);
        }
        Some(InteractionResult::SuccessServer)
    }
}

/// What flint and steel does to the block it is used on (`FlintAndSteelItem.useOn`): lights the
/// campfire, candle or candle cake `clicked`. Whether it did.
fn light(host: &mut dyn ServerHost, level: LevelRef, clicked: [i32; 3]) -> bool {
    let block = game::block_name(&*host, level.block(&*host, clicked));
    let centre = clicked.map(|c| f64::from(c) + 0.5);
    let lightable = block.ends_with("campfire") || block.ends_with("candle") || block.ends_with("candle_cake");
    if lightable
        && level.block_property(&*host, clicked, "lit").as_deref() == Some("false")
        && level.block_property(&*host, clicked, "waterlogged").as_deref() != Some("true")
    {
        host.set_block_property(clicked, "lit", "true");
        crate::platform::play_sound(host, level, "minecraft:item.flintandsteel.use", centre, SoundSource::Blocks, 0.8, 1.0);
        return true;
    }
    false
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
