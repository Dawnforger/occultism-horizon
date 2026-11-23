// What the glue asks of the game, in the words it uses: items by id, this mod's own components,
// and items moved through the game's handlers (`ITEMS`, `ENTITY_ITEMS`) in transactions, so a move
// that can't finish puts everything back.

use mod_api::*;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, OnceLock};

pub fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// The game's item and block names, by key and by id, read once from its registries: keys keep
/// their identity until the game restarts.
pub struct Names {
    items: HashMap<ItemKey, &'static str>,
    item_keys: HashMap<&'static str, ItemKey>,
    blocks: HashMap<BlockKey, &'static str>,
}

static NAMES: OnceLock<Names> = OnceLock::new();

fn names(host: &dyn Host) -> &'static Names {
    NAMES.get_or_init(|| {
        let leak = |s: String| -> &'static str { Box::leak(s.into_boxed_str()) };
        let items: HashMap<ItemKey, &'static str> = host.item_keys().into_iter().map(|k| (k, leak(k.id(host)))).collect();
        let item_keys = items.iter().map(|(k, n)| (*n, *k)).collect();
        let blocks = host.block_keys().into_iter().map(|k| (k, leak(k.id(host)))).collect();
        Names { items, item_keys, blocks }
    })
}

/// An item's id (`minecraft:stone`), or "" for a key the game doesn't have.
pub fn item_name(host: &dyn Host, item: ItemKey) -> &'static str {
    names(host).items.get(&item).copied().unwrap_or("")
}

/// An item's id where no host is given (a menu's slot rules), once a hook with one has read the
/// names; "" before.
pub fn known(item: ItemKey) -> &'static str {
    NAMES.get().and_then(|n| n.items.get(&item).copied()).unwrap_or("")
}

/// A stack's item id.
pub fn id(host: &dyn Host, stack: &ItemStack) -> &'static str {
    item_name(host, stack.item())
}

/// The item named `item`.
pub fn item(host: &dyn Host, item: &str) -> Option<ItemKey> {
    names(host).item_keys.get(item).copied()
}

/// A block's id, or "" for none.
pub fn block_name(host: &dyn Host, block: Option<BlockKey>) -> &'static str {
    block.and_then(|b| names(host).blocks.get(&b).copied()).unwrap_or("")
}

/// `count` of `item`, plain; none for an unknown item or a count of 0.
pub fn stack(host: &dyn Host, item_id: &str, count: u32) -> Option<ItemStack> {
    ItemStack::new(item(host, item_id)?, count)
}

/// Whether a stack is its item and nothing more: no damage, no components of its own.
pub fn plain(stack: &ItemStack) -> bool {
    stack.components().is_none_or(|c| c.as_nbt().is_empty()) && stack.damage() == 0
}

/// One of this mod's own data components, read and written whole as its NBT.
pub struct Nbt(pub &'static str);

impl Component for Nbt {
    type Value = Tag;
    fn id(&self) -> &str {
        self.0
    }
    fn decode(&self, value: &Tag) -> Option<Tag> {
        Some(value.clone())
    }
    fn encode(&self, value: Tag) -> Tag {
        value
    }
}

/// The compound a stack carries as component `name`.
pub fn component(stack: &ItemStack, name: &'static str) -> Option<Compound> {
    match stack.get(&Nbt(name))? {
        Tag::Compound(c) => Some(c),
        _ => None,
    }
}

/// `stack` with component `name` set to `value`.
pub fn set_component(stack: &mut ItemStack, name: &'static str, value: Compound) {
    let _ = stack.set(&Nbt(name), Tag::Compound(value));
}

pub fn text(nbt: &Compound, key: &str) -> Option<String> {
    match nbt.get(&key.into()) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    }
}

pub fn int(nbt: &Compound, key: &str) -> Option<i32> {
    match nbt.get(&key.into()) {
        Some(Tag::Int(v)) => Some(*v),
        Some(Tag::Short(v)) => Some(i32::from(*v)),
        Some(Tag::Byte(v)) => Some(i32::from(*v)),
        _ => None,
    }
}

/// Tells `player` a line, if they are here.
pub fn tell(host: &mut dyn ServerHost, player: u128, line: Text) {
    if let Some(p) = host.player(player) {
        let _ = p.tell(host, line);
    }
}

/// A line of translated text with literal arguments.
pub fn say(key: &str, args: &[&str]) -> Text {
    Text::translate(key, args.iter().map(|a| Text::literal(*a)).collect())
}

/// `stack` into `player`'s inventory where `Inventory.add` puts it. What didn't fit.
pub fn give(host: &mut dyn ServerHost, player: u128, stack: ItemStack) -> Option<ItemStack> {
    let Some(entity) = host.player(player).map(PlayerRef::entity) else { return Some(stack) };
    let Some(items) = entity.capability(host, &ENTITY_ITEMS, ()) else { return Some(stack) };
    let resource = ItemResource::of(&stack);
    let count = stack.count();
    let went = host.transaction(|host, tx| Ok::<u32, ()>(items.insert_any(host, tx, &resource, count))).unwrap_or(0);
    resource.to_stack(count - went.min(count))
}

/// `stack` into the block at `pos` through its `side`, as a hopper would put it. What didn't go in.
pub fn insert(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], side: Direction, stack: ItemStack) -> Option<ItemStack> {
    let Some(items) = level.capability(host, &ITEMS, pos, Some(side)) else { return Some(stack) };
    let resource = ItemResource::of(&stack);
    let count = stack.count();
    let went = host.transaction(|host, tx| Ok::<u32, ()>(items.insert_any(host, tx, &resource, count))).unwrap_or(0);
    resource.to_stack(count - went.min(count))
}

/// Up to `count` of what slot `slot` of the block at `pos` holds, taken through `side`.
pub fn extract(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], side: Direction, slot: usize, count: u32) -> Option<ItemStack> {
    let items = level.capability(host, &ITEMS, pos, Some(side))?;
    let resource = items.resource(host, slot)?;
    let came = host.transaction(|host, tx| Ok::<u32, ()>(items.extract(host, tx, slot, &resource, count))).unwrap_or(0);
    resource.to_stack(came)
}

/// A player as a living entity: their hands, equipment and effects.
pub fn living(host: &dyn Host, player: u128) -> Option<LivingRef> {
    let entity = host.player(player)?.entity();
    host.living(entity)
}

/// One of what `player` holds in `hand` taken from them (none in creative). Whether they had it.
pub fn use_one(host: &mut dyn ServerHost, player: u128, hand: Hand) -> bool {
    let Some(living) = living(host, player) else { return false };
    let Some(held) = living.hand_item(host, hand) else { return false };
    if host.player(player).and_then(|p| p.is_creative(host)) == Some(true) {
        return true;
    }
    let left = held.shrink(1);
    living.set_hand_item(host, hand, left).is_ok_and(|changed| changed)
}

/// The block state `name` (`occultism:spirit_fire`, `mod:block[facing=north]`) set at `pos`.
pub fn set_block(host: &mut dyn ServerHost, level: LevelRef, pos: [i32; 3], name: &str) -> bool {
    let Some(state) = host.block_state_id(name) else { return false };
    level.set_block(host, pos, state, BlockUpdateFlags::NEIGHBORS | BlockUpdateFlags::CLIENTS).unwrap_or(false)
}

/// A random number (xorshift, seeded from the clock once), for what Occultism rolls.
/// A standard normal number from `state` (Box-Muller), as `RandomSource.nextGaussian` gives.
pub fn gaussian(state: &std::sync::atomic::AtomicU64) -> f64 {
    let unit = |r: u64| ((r >> 11) as f64 + 0.5) / (1u64 << 53) as f64;
    let (a, b) = (unit(roll(state)), unit(roll(state)));
    (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos()
}

pub fn roll(state: &std::sync::atomic::AtomicU64) -> u64 {
    use std::sync::atomic::Ordering;
    let mut x = state.load(Ordering::Relaxed);
    if x == 0 {
        x = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0x9E37_79B9_7F4A_7C15, |d| d.as_nanos() as u64) | 1;
    }
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    state.store(x, Ordering::Relaxed);
    x
}
