// Soul gems, as Occultism's `SoulGemItem`: right-click a creature with an empty gem and it goes
// into the gem whole, as its entity data (`minecraft:entity_data`, the component vanilla's spawn
// eggs carry); right-click a block with the filled gem and it comes out there, facing the player.
// The fragile gem breaks when it lets its creature out. Players, and what the gem's deny list
// names (the bosses), stay out.

use crate::game::{self, id, tell};
use mod_api::*;

pub const FRAGILE: &str = "occultism:fragile_soul_gem";
pub const SOUL_GEM: &str = "occultism:soul_gem";
pub const GEMS: [&str; 2] = [FRAGILE, SOUL_GEM];
/// The captured creature: its data, with its type as `id`.
const ENTITY_DATA: &str = "minecraft:entity_data";
const ITEM_NAME: &str = "minecraft:item_name";

/// Creatures just let out, with their data: given it again at the end of the server tick, after
/// the game's spawn setup (a sheep's random colour, a mob's random equipment) has run over it.
static RELEASED: std::sync::Mutex<Vec<(EntityRef, Compound)>> = std::sync::Mutex::new(Vec::new());

/// The end of a server tick: each creature let out this tick takes its own data back.
pub fn settle(host: &mut dyn ServerHost) {
    let released = std::mem::take(&mut *game::lock(&RELEASED));
    for (creature, data) in released {
        creature.merge_nbt(host, data);
    }
}

/// What a gem holds: the creature's type and the rest of its data.
fn held(stack: &ItemStack) -> Option<(String, Compound)> {
    let Tag::Compound(mut data) = stack.get(&game::Nbt(ENTITY_DATA))? else { return None };
    let kind = game::text(&data, "id")?;
    data.remove(&"id".into());
    Some((kind, data))
}

/// `entity` (a creature the player right-clicked with `stack`, a soul gem, in `hand`) into the
/// gem, on the server. The answer that stops the click there, or None to let it go on.
pub fn capture(host: &mut dyn ServerHost, hand: Hand, entity: i32, kind: &str, stack: &ItemStack) -> Option<InteractionResult> {
    let gem = id(&*host, stack);
    if !GEMS.contains(&gem) {
        return None;
    }
    // Only the main hand, as Occultism's. A full gem holds one creature already: the click does
    // nothing (a villager's trades don't open either).
    if hand != Hand::Main {
        return None;
    }
    if held(stack).is_some() {
        return (kind != "minecraft:player").then_some(InteractionResult::Fail);
    }
    let player = host.players().acting_player()?.uuid();
    let creature = host.entity(entity as u32)?;
    // Living creatures only, never a player.
    if kind == "minecraft:player" || host.living(creature).and_then(|l| l.alive(&*host)) != Some(true) {
        return None;
    }
    let deny = format!("occultism:deny_list_{}", gem.trim_start_matches("occultism:"));
    if host.entity_type_in_tag(kind, &deny) {
        tell(host, player, game::say(&format!("item.{}.message.entity_type_denied", gem.replace(':', ".")), &[]));
        return Some(InteractionResult::Fail);
    }
    let mut data = creature.nbt(host)?;
    data.insert("id".into(), Tag::String(kind.into()));
    // A working spirit takes its work along.
    if let Some(work) = crate::workers::carried(&*host, creature) {
        data.insert(crate::workers::CARRIED.into(), Tag::Compound(work));
    }
    let mut filled = stack.clone();
    filled.set(&game::Nbt(ENTITY_DATA), Tag::Compound(data)).ok()?;
    let mut name = Compound::new();
    name.insert("translate".into(), Tag::String(format!("item.{}.filled", gem.replace(':', ".")).as_str().into()));
    let _ = filled.set(&game::Nbt(ITEM_NAME), Tag::Compound(name));
    // The gem takes the creature before it leaves the world, and gives it back if it can't leave.
    let living = game::living(&*host, player)?;
    if !living.set_hand_item(host, hand, Some(filled)).unwrap_or(false) {
        return Some(InteractionResult::Fail);
    }
    if !creature.discard(host) {
        let _ = living.set_hand_item(host, hand, Some(stack.clone()));
        return Some(InteractionResult::Fail);
    }
    Some(InteractionResult::Success)
}

/// Whether the client should answer a click on a creature with `stack` itself: a gem on anything
/// but a player, so a villager's trades don't open before the server captures it (or refuses).
pub fn takes_click(host: &dyn Host, hand: Hand, kind: &str, stack: &ItemStack) -> bool {
    hand == Hand::Main && GEMS.contains(&id(host, stack)) && kind != "minecraft:player"
}

/// A soul gem: its creature let out where it is used, and what it says it holds.
pub struct GemPart;

#[mod_api::horizon_part]
impl ItemPart for GemPart {
    fn use_on(&self, host: HostSide<'_>, event: &mut UseOnEvent<'_>) -> Option<InteractionResult> {
        if event.hand != Hand::Main {
            return None;
        }
        let HostSide::Server(host) = host else {
            return held(event.stack).map(|_| InteractionResult::Success);
        };
        let (kind, mut data) = held(event.stack)?;
        let player = event.player;
        let level = host.level(host.dimension())?;
        // Into the clicked block's space when it's open (grass, snow), else beside the face clicked.
        let here = game::block_name(&*host, level.block(&*host, event.pos));
        let open = crate::ritual::AIR.contains(&here) || matches!(here, "minecraft:short_grass" | "minecraft:fern" | "minecraft:snow");
        let at = if open { event.pos } else { [event.pos[0] + event.face[0], event.pos[1] + event.face[1], event.pos[2] + event.face[2]] };
        // Where it was, its motion and its identity stay behind: it is a new creature here.
        for key in ["Pos", "Motion", "UUID"] {
            data.remove(&key.into());
        }
        let work = match data.remove(&crate::workers::CARRIED.into()) {
            Some(Tag::Compound(work)) => Some(work),
            _ => None,
        };
        let Some(creature) = level.spawn_mob(host, &kind, [f64::from(at[0]) + 0.5, f64::from(at[1]), f64::from(at[2]) + 0.5], SpawnReason::MobSummoned) else {
            tell(host, player, game::say("item.occultism.soul_gem.message.no_room", &[]));
            return Some(InteractionResult::Fail);
        };
        creature.merge_nbt(host, data.clone());
        if let Some(work) = &work {
            crate::workers::restore(host, creature, work);
        }
        game::lock(&RELEASED).push((creature, data));
        let facing = host.player(player).and_then(|p| p.entity().head_yaw(&*host)).unwrap_or(0.0) + 180.0;
        creature.set_rotation(host, facing, 0.0);
        // The gem is empty again, or gone if it was fragile; if the hand can't change, the creature
        // goes back in.
        let left = if id(&*host, event.stack) == FRAGILE {
            None
        } else {
            let mut empty = event.stack.clone();
            empty.remove(&game::Nbt(ENTITY_DATA));
            empty.remove(&game::Nbt(ITEM_NAME));
            Some(empty)
        };
        let Some(living) = game::living(&*host, player) else { return Some(InteractionResult::Fail) };
        if !living.set_hand_item(host, event.hand, left).unwrap_or(false) {
            creature.discard(host);
            return Some(InteractionResult::Fail);
        }
        Some(InteractionResult::Success)
    }

    fn tooltip(&self, host: &dyn ClientHost, event: &mut TooltipEvent<'_>) {
        let gem = id(&*host, event.stack).replace(':', ".");
        let text = match held(event.stack) {
            Some((kind, _)) => {
                let (ns, path) = kind.split_once(':').unwrap_or(("minecraft", &kind));
                let name = host.translate(&format!("entity.{ns}.{path}")).unwrap_or_else(|| path.replace('_', " "));
                host.translate(&format!("item.{gem}.tooltip_filled")).map(|t| t.replacen("%s", &name, 1))
            }
            None => host.translate(&format!("item.{gem}.tooltip_empty")),
        };
        // Occultism's lines, colour codes and all: grey, red after §c, dark grey after §7.
        for line in text.unwrap_or_default().split('\n') {
            let (colour, line) = match line {
                l if l.starts_with("§c") => (0xFFFF5555, &l[3..]),
                l if l.starts_with("§7") => (0xFFAAAAAA, &l[3..]),
                l => (0xFFAAAAAA, l),
            };
            event.lines.push((Text::literal(line), colour));
        }
    }
}
