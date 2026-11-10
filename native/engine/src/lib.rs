// Occultism's Demonic Altar for Horizon: the glue between the game's hooks and the altar's rules
// (logic.rs). A trait mod (docs/native-mods.md, "Trait mods"): the launcher links this package
// into the game and calls `hooks()`.
//
// Server side: the altar block gets a block entity with six slots (core, input, four outputs),
// a menu type of its own and per-face hopper rules; each tick runs the bound spirit's job over
// Occultism's `occultism:crushing` recipes, read from the content mod's data like any data pack.
// Client side: the altar's screen, the core floating over the altar while it works, and the
// cores' tooltips.
//
// The ritual circles (chalk glyphs, sacrificial bowls, the golden bowl's rituals) and iesnium
// from silver ore are in rites.rs, with their rules in ritual.rs.

pub mod logic;
pub mod network;
pub mod ritual;
mod rites;
pub mod storage;
mod vault;

use horizon_client::mod_api::*;
use logic::{AltarState, Recipe, Spirit, SpiritComponent, Stack, Tick};
use rites::{BOWL, GOLDEN_BOWL};
use vault::{ACTUATOR, CORE};
use std::sync::{Arc, Mutex};

pub const ALTAR: &str = "occultism:demonic_altar";
pub const MENU: &str = "occultism:demonic_altar";
pub const RECIPE_TYPE: &str = "occultism:crushing";
pub const SPIRIT_COMPONENT: &str = "occultism:spirit";

pub fn hooks() -> Box<dyn Hooks> {
    Box::new(DemonicAltar::default())
}

#[derive(Default)]
pub struct DemonicAltar {
    recipes: Mutex<RecipeCache>,
    rites: rites::Rites,
    vault: vault::Vault,
}

/// The crushing recipes, resolved to item ids once per recipe table: a `/reload` brings new
/// `CustomRecipe` values, and their addresses tell the cache it is stale.
#[derive(Default)]
struct RecipeCache {
    key: Vec<usize>,
    recipes: Arc<[Recipe]>,
}

impl DemonicAltar {
    fn recipes(&self, host: &dyn Host) -> Arc<[Recipe]> {
        let table = host.recipes_of_type(RECIPE_TYPE);
        let key: Vec<usize> = table.iter().map(|r| Arc::as_ptr(&r.json) as usize).collect();
        let mut cache = self.recipes.lock().unwrap_or_else(|e| e.into_inner());
        if cache.key != key {
            let parsed: Vec<Recipe> = table.iter().filter_map(|r| parse_recipe(host, r)).collect();
            cache.recipes = parsed.into();
            cache.key = key;
        }
        cache.recipes.clone()
    }
}

/// An `occultism:crushing` file as `CrushingRecipe.CODEC` reads it. A recipe whose ingredient or
/// result names nothing the game has is left out, as NeoForge's `tag_empty` conditions on the
/// original files leave it out.
fn parse_recipe(host: &dyn Host, r: &CustomRecipe) -> Option<Recipe> {
    let j = &*r.json;
    let inputs = host.recipe_ingredient(&j["ingredient"])?.order;
    let result = &j["result"];
    let item = match result["type"].as_str() {
        Some("occultism:tag") => {
            let tag = result["tag"].as_str()?;
            let tag = serde_json::Value::String(format!("#{tag}"));
            host.recipe_ingredient(&tag)?.order.into_iter().next()?
        }
        _ => host.recipe_result(&serde_json::json!({"id": result["id"], "count": 1}))?.item.to_string(),
    };
    let int = |key: &str, default: i64| j[key].as_i64().unwrap_or(default);
    Some(Recipe {
        id: r.id.clone(),
        inputs,
        result: item,
        count: result["count"].as_u64().unwrap_or(1).clamp(1, 99) as u32,
        time: int("crushing_time", logic::DEFAULT_CRUSHING_TIME as i64).max(1) as u32,
        min_tier: int("min_tier", -1) as i32,
        max_tier: int("max_tier", -1) as i32,
        ignore_multiplier: j["ignore_crushing_multiplier"].as_bool().unwrap_or(false),
    })
}

/// The `occultism:spirit` component a stack carries, when it differs from its item's default
/// (Horizon keeps the rest in the item's own defaults, which `logic::spirit_of` falls back to).
fn spirit_component(stack: &ItemStack) -> Option<SpiritComponent> {
    let extra = stack.extra.as_ref()?;
    let (_, tag) = extra.other.iter().find(|(k, _)| k == SPIRIT_COMPONENT)?;
    let Tag::Compound(c) = tag else { return None };
    let job = match c.get(&"job".into()) {
        Some(Tag::String(s)) => Some(s.to_string_lossy()),
        _ => None,
    };
    let tier = match c.get(&"tier".into()) {
        Some(Tag::Int(i)) => Some(*i),
        Some(Tag::Byte(i)) => Some(*i as i32),
        Some(Tag::Short(i)) => Some(*i as i32),
        _ => None,
    };
    Some(SpiritComponent { job, tier })
}

fn spirit_in(stack: Option<&ItemStack>) -> Option<Spirit> {
    let stack = stack?;
    logic::spirit_of(&stack.item, spirit_component(stack).as_ref())
}

fn to_logic(stack: &Option<ItemStack>) -> Option<Stack> {
    stack.as_ref().filter(|s| s.count > 0).map(|s| Stack {
        item: s.item.to_string(),
        count: s.count,
        plain: s.extra.is_none() && s.damage == 0,
    })
}

fn read_state(nbt: &Compound) -> AltarState {
    AltarState {
        progress: match nbt.get(&"progress".into()) {
            Some(Tag::Int(p)) => (*p).max(0) as u32,
            _ => 0,
        },
        recipe: match nbt.get(&"recipe".into()) {
            Some(Tag::String(s)) => Some(s.to_string_lossy()),
            _ => None,
        },
    }
}

fn write_state(nbt: &mut Compound, state: &AltarState) {
    nbt.insert("progress".into(), Tag::Int(state.progress as i32));
    match &state.recipe {
        Some(r) => nbt.insert("recipe".into(), Tag::String(r.as_str().into())),
        None => nbt.remove(&"recipe".into()),
    };
}

/// What clients draw: the core's item id (empty without one) and whether the altar is working.
fn shown(core: Option<&ItemStack>, active: bool) -> Compound {
    let mut tag = Compound::new();
    let core = core.map_or(String::new(), |s| s.item.to_string());
    tag.insert("core".into(), Tag::String(core.as_str().into()));
    tag.insert("active".into(), Tag::Byte(active as i8));
    tag
}

impl Hooks for DemonicAltar {
    fn component_types(&self, event: &mut ComponentTypesEvent<'_>) {
        event.types.push(SPIRIT_COMPONENT.to_string());
        event.types.push(vault::STORE_COMPONENT.to_string());
    }

    fn block_entity(&self, _host: &mut dyn Host, event: &mut BlockEntityEvent<'_>) -> Option<(String, Compound)> {
        [ALTAR, BOWL, GOLDEN_BOWL, CORE, ACTUATOR]
            .contains(&event.block)
            .then(|| (event.block.to_string(), Compound::new()))
    }

    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if [ALTAR, BOWL, GOLDEN_BOWL, CORE, ACTUATOR].contains(&event.block) {
            event.ticks = true;
        }
    }

    fn block_entity_container(&self, _host: &dyn Host, event: &mut BlockEntityContainerEvent<'_>) -> Option<ModContainer> {
        if event.block == BOWL || event.block == GOLDEN_BOWL {
            // One item, no screen: players use the bowl by hand, hoppers through any face.
            return Some(ModContainer {
                slots: 1,
                faces: std::array::from_fn(|_| vec![0]),
                inputs: vec![0],
                outputs: vec![0],
                menu: None,
            });
        }
        if event.block == ACTUATOR {
            // Hoppers in from the top and sides, out from the bottom.
            let (inputs, outputs): (Vec<usize>, Vec<usize>) = (vault::INPUTS.collect(), vault::OUTPUTS.collect());
            let mut faces: [Vec<usize>; 6] = std::array::from_fn(|_| inputs.clone());
            faces[0] = outputs.clone();
            return Some(ModContainer { slots: vault::SLOT_COUNT, faces, inputs, outputs, menu: Some(vault::MENU.to_string()) });
        }
        (event.block == ALTAR).then(|| ModContainer {
            slots: logic::SLOT_COUNT,
            faces: logic::face_slots(),
            inputs: vec![logic::SLOT_INPUT],
            outputs: logic::SLOTS_OUTPUT.to_vec(),
            menu: Some(MENU.to_string()),
        })
    }

    fn block_entity_tick(&self, host: &mut dyn Host, event: &mut BlockEntityTickEvent<'_>) {
        if event.entity.id == CORE {
            if let Some(notice) = self.vault.core_tick(&*host, event.pos, event.entity) {
                host.tell(notice.player, Text::translate(notice.key, &[Text::literal(notice.name)]));
            }
            return;
        }
        if event.entity.id == ACTUATOR {
            self.vault.actuator_tick(&*host, event.pos, event.entity);
            return;
        }
        if event.entity.id == BOWL || event.entity.id == GOLDEN_BOWL {
            let pos = event.pos;
            if event.entity.id == BOWL {
                self.rites.bowl_tick(&*host, pos, event.entity);
            } else {
                self.rites.golden_tick(host, pos, event.entity);
            }
            // Clients draw the bowl's item, turning faster while a ritual runs.
            let entity = &mut *event.entity;
            let active = entity.nbt.get(&"ritual".into()).is_some();
            let now = shown(entity.items.first().and_then(Option::as_ref), active);
            if entity.nbt.get(&"shown".into()) != Some(&Tag::Compound(now.clone())) {
                entity.nbt.insert("shown".into(), Tag::Compound(now));
                host.block_entity_sync(pos);
            }
            return;
        }
        if event.entity.id != ALTAR {
            return;
        }
        let recipes = self.recipes(&*host);
        let pos = event.pos;
        let entity = &mut *event.entity;
        // A core a hopper put in the input binds or passes through, whole stack and components.
        let logic_slots: Vec<Option<Stack>> = (0..logic::SLOT_COUNT)
            .map(|i| entity.items.get(i).and_then(to_logic))
            .collect();
        let input_is_core = spirit_in(entity.items.get(logic::SLOT_INPUT).and_then(Option::as_ref)).is_some();
        if let Some(to) = logic::stray_core_target(&logic_slots, input_is_core) {
            if entity.items.len() < logic::SLOT_COUNT {
                entity.items.resize(logic::SLOT_COUNT, None);
            }
            let mut core = entity.items[logic::SLOT_INPUT].take();
            if to == logic::SLOT_CORE
                && let Some(stack) = core.as_mut()
                && stack.count > 1
            {
                let mut one = stack.clone();
                one.count = 1;
                stack.count -= 1;
                entity.items[logic::SLOT_CORE] = Some(one);
            } else {
                entity.items[to] = core.take();
            }
            entity.items[logic::SLOT_INPUT] = core;
        }
        let before: Vec<Option<Stack>> = (0..logic::SLOT_COUNT)
            .map(|i| entity.items.get(i).and_then(to_logic))
            .collect();
        let mut slots = before.clone();
        let mut state = read_state(&entity.nbt);
        let spirit = spirit_in(entity.items.get(logic::SLOT_CORE).and_then(Option::as_ref));
        let max_stack = |item: &str| host.max_stack_size(item).unwrap_or(64);
        let tick = logic::tick(&mut state, &mut slots, &recipes, &max_stack, spirit);

        // Write back only the slots the tick changed, so untouched stacks keep their components.
        if entity.items.len() < logic::SLOT_COUNT {
            entity.items.resize(logic::SLOT_COUNT, None);
        }
        for i in 0..logic::SLOT_COUNT {
            if slots[i] != before[i] {
                entity.items[i] = slots[i].as_ref().map(|s| match &entity.items[i] {
                    Some(old) if old.item.as_ref() == s.item => ItemStack { count: s.count, ..old.clone() },
                    _ => ItemStack { item: s.item.as_str().into(), count: s.count, damage: 0, extra: None },
                });
            }
        }
        if read_state(&entity.nbt) != state {
            write_state(&mut entity.nbt, &state);
        }
        let data = logic::data_slots(&tick, &state, spirit).to_vec();
        if entity.data != data {
            entity.data = data;
        }
        // Tell clients when what they draw changes: a core in or out, work starting or stopping.
        let active = matches!(tick, Tick::Working { .. } | Tick::Done { .. });
        let now = shown(entity.items.get(logic::SLOT_CORE).and_then(Option::as_ref), active);
        let was = entity.nbt.get(&"shown".into()).cloned();
        if was != Some(Tag::Compound(now.clone())) {
            entity.nbt.insert("shown".into(), Tag::Compound(now));
            host.block_entity_sync(pos);
        }
    }

    fn block_entity_update_tag(&self, event: &mut BlockEntityUpdateTagEvent<'_>) -> Option<Compound> {
        if ![ALTAR, BOWL, GOLDEN_BOWL].contains(&event.entity.id.as_str()) {
            return None;
        }
        let mut tag = Compound::new();
        if let Some(shown) = event.entity.nbt.get(&"shown".into()) {
            tag.insert("shown".into(), shown.clone());
        }
        Some(tag)
    }

    fn block_entity_submit(&self, host: &dyn Host, event: &mut BlockEntitySubmitEvent<'_>) {
        if ![ALTAR, BOWL, GOLDEN_BOWL].contains(&event.entity.id.as_str()) {
            return;
        }
        let in_bowl = event.entity.id != ALTAR;
        let Some(Tag::Compound(shown)) = event.entity.nbt.get(&"shown".into()) else { return };
        let core = match shown.get(&"core".into()) {
            Some(Tag::String(s)) => s.to_string_lossy(),
            _ => return,
        };
        if core.is_empty() {
            return;
        }
        let active = matches!(shown.get(&"active".into()), Some(Tag::Byte(1)));
        let pos = event.pos;
        let draw = &mut *event.draw;
        // The altar is opaque, so its own light is dark: light the core as the air it floats in.
        let (block, sky) = host.light([pos[0], pos[1] + 1, pos[2]]);
        let light = draw.light;
        draw.light = u32::from(sky) * 16 | (u32::from(block) * 16) << 8;
        let t = draw.game_time as f32 + draw.partial;
        // The bound spirit hovers over the altar: slow and low when idle, fast and bobbing at work.
        // A bowl's item sits in it, turning slowly, and rises and turns faster during a ritual.
        let (spin, bob) = if active { (6.0, 0.06) } else { (1.5, 0.02) };
        let (height, size) = match (in_bowl, active) {
            (false, _) => (1.25, 0.6),
            (true, false) => (0.3, 0.4),
            (true, true) => (0.55, 0.4),
        };
        draw.push();
        draw.translate([0.5, height + bob * (t * 0.15).sin(), 0.5]);
        draw.rotate_about([0.0, 1.0, 0.0], (t * spin) % 360.0, [0.0, 0.0, 0.0]);
        draw.scale([size, size, size]);
        let stack = ItemStack { item: core.as_str().into(), count: 1, damage: 0, extra: None };
        host.draw_item(draw, &stack, DisplayContext::Fixed, None, 0);
        draw.pop();
        draw.light = light;
    }

    fn block_entity_render_bounding_box(&self, _host: &dyn Host, event: &mut BlockEntityRenderBoundingBoxEvent<'_>) {
        if event.block == ALTAR {
            event.bounds[4] = event.bounds[4].max(2.0);
        }
    }

    fn screen(&self, _host: &dyn Host, event: &mut ScreenEvent<'_>) -> Option<NativeScreen> {
        if event.block == ACTUATOR {
            // Prototype: a row in, a row out, its counts and a take button. The terminal comes next.
            let mut slots = vec![None; vault::SLOT_COUNT];
            for i in 0..9 {
                slots[i] = Some([8.0 + 18.0 * i as f32, 18.0]);
                slots[i + 9] = Some([8.0 + 18.0 * i as f32, 60.0]);
            }
            return Some(NativeScreen {
                width: 176.0,
                height: 166.0,
                slots,
                inventory_y: 84.0,
                inventory_x: 8.0,
                own_labels: false,
                own_background: false,
            });
        }
        if event.block != ALTAR {
            return None;
        }
        // 176 x 166 like a furnace: core on the left, input, the progress arrow, a 2 x 2 output.
        let mut slots = vec![None; logic::SLOT_COUNT];
        slots[logic::SLOT_CORE] = Some([26.0, 35.0]);
        slots[logic::SLOT_INPUT] = Some([56.0, 35.0]);
        slots[2] = Some([116.0, 26.0]);
        slots[3] = Some([134.0, 26.0]);
        slots[4] = Some([116.0, 44.0]);
        slots[5] = Some([134.0, 44.0]);
        Some(NativeScreen {
            width: 176.0,
            height: 166.0,
            slots,
            inventory_y: 84.0,
            inventory_x: 8.0,
            own_labels: false,
            own_background: false,
        })
    }

    fn screen_click(&self, _host: &dyn Host, event: &mut ScreenClickEvent<'_>) -> Option<i32> {
        // The take button: the strip between the rows.
        (event.block == ACTUATOR && (40.0..58.0).contains(&event.at[1]) && (120.0..168.0).contains(&event.at[0]))
            .then_some(vault::BUTTON_TAKE)
    }

    fn menu_button(&self, host: &mut dyn Host, event: &mut MenuButtonEvent<'_>) {
        if event.entity.id == ACTUATOR && event.id == vault::BUTTON_TAKE {
            self.vault.take(&*host, event.pos, event.entity);
        }
    }

    fn destroy_block(&self, host: &mut dyn Host, event: &mut DestroyBlockEvent) -> Flow {
        // A creative break drops nothing, so the Core comes to the breaker's inventory carrying its
        // store (as vanilla's pick-block with its data would); `loot_drops` does it otherwise.
        if host.block(event.pos) == CORE
            && let Some(player) = host.acting_player()
            && player.game_mode == 1
            && let Some((id, name)) = self.vault.unanchor(&*host, &player.dimension, event.pos)
        {
            let mut core = ItemStack { item: CORE.into(), count: 1, damage: 0, extra: None };
            vault::carry(&mut core, &id, &name);
            let _ = host.give_stack(player.uuid, core);
            host.tell(player.uuid, Text::translate("storage.occultism.dormant", &[Text::literal(name)]));
        }
        Flow::Continue
    }

    fn place_block(&self, host: &mut dyn Host, event: &mut PlaceBlockEvent<'_>) {
        if &*event.stack.item == CORE {
            self.vault.placed(host.dimension(), event.pos, event.stack, host.acting_player().map(|p| p.uuid));
        }
    }

    fn world_lifecycle(&self, _host: &mut dyn Host, event: &mut WorldLifecycleEvent<'_>) {
        if matches!(event.phase, WorldPhase::WorldCloseFinal | WorldPhase::WorldCloseFailed) {
            self.vault.close();
        }
    }

    fn screen_draw(&self, host: &dyn Host, event: &mut ScreenDrawEvent<'_>) {
        if event.block == ACTUATOR {
            let get = |i: usize| event.data.get(i).copied().unwrap_or(0);
            let [ox, oy] = event.origin;
            let line = format!("{}/{} types, {}k/{}k", get(0), get(1), get(2), get(3));
            host.text(event.batch, &line, [ox + 8.0, oy + 44.0], 0x404040, false);
            host.fill(event.batch, [ox + 124.0, oy + 42.0, 44.0, 12.0], 0xFF6A3D9A);
            host.text(event.batch, "Take", [ox + 134.0, oy + 44.0], 0xFFFFFF, false);
            return;
        }
        if event.block != ALTAR {
            return;
        }
        let get = |i: usize| event.data.get(i).copied().unwrap_or(0);
        let [ox, oy] = event.origin;
        let (progress, total, tier, state) =
            (get(logic::DATA_PROGRESS), get(logic::DATA_TOTAL), get(logic::DATA_TIER), get(logic::DATA_STATE));
        // The arrow from input to output: a dark track and a filled part.
        let track = [ox + 79.0, oy + 38.0, 26.0, 10.0];
        host.fill(event.batch, track, 0xFF3B2A4A);
        if total > 0 {
            let w = 26.0 * (progress.clamp(0, total) as f32 / total as f32);
            let colour = if state == 2 { 0xFFB23A3A } else { 0xFFB06CFF };
            host.fill(event.batch, [track[0], track[1], w, track[3]], colour);
        }
        let status = if tier == 0 {
            "no_core"
        } else {
            match state {
                1 | 3 => "working",
                2 => "full",
                _ => "idle",
            }
        };
        // One line between the slots and the "Inventory" label (y 73): the status on the left, the
        // rank under the output grid.
        let label = horizon_client::lang::get(&format!("gui.occultism.demonic_altar.{status}"));
        host.text(event.batch, &label, [ox + 8.0, oy + 63.0], 0x404040, false);
        if let Some(t) = logic::Tier::from_level(tier) {
            let rank = horizon_client::lang::get(&format!("tooltip.occultism.spirit.{}", t.name()));
            host.text(event.batch, &rank, [ox + 116.0, oy + 63.0], 0x6A3D9A, false);
        }
    }

    fn menu_slot_rules(&self, event: &mut MenuSlotRulesEvent<'_>) {
        if event.menu != MENU {
            return;
        }
        let component = spirit_component(event.stack);
        let (may_place, max) = logic::slot_rule(event.slot, &event.stack.item, component.as_ref());
        event.may_place = may_place;
        event.max_stack = event.max_stack.min(max);
    }

    fn menu_quick_move(&self, host: &mut dyn Host, event: &mut MenuQuickMoveEvent<'_>) -> Flow {
        if event.entity.id != ALTAR || event.slot < event.container_slots {
            // Shift-clicks out of the altar go to the inventory as a chest's do.
            return Flow::Continue;
        }
        let component = spirit_component(event.stack);
        let core_empty = event.entity.items.get(logic::SLOT_CORE).is_none_or(Option::is_none);
        match logic::quick_move_target(&event.stack.item, component.as_ref(), core_empty) {
            Some(logic::SLOT_CORE) => {
                let mut one = event.stack.clone();
                one.count = 1;
                event.stack.count -= 1;
                event.entity.items[logic::SLOT_CORE] = Some(one);
                Flow::Cancel
            }
            Some(_) => Flow::Continue,
            None => {
                // A core with the core slot taken: leave it where it is, never in the input.
                let _ = host;
                Flow::Cancel
            }
        }
    }

    fn right_click_block(&self, host: &mut dyn Host, event: &mut RightClickBlockEvent) -> Flow {
        self.rites.right_click(host, event.pos)
    }

    fn state_for_placement(&self, level: &dyn LevelHost, event: &mut StateForPlacementEvent<'_>) -> Option<Option<BlockStateId>> {
        self.rites.glyph_state(level, event.state, event.context).map(Some)
    }

    fn should_apply_effect_tick_this_tick(&self, _host: &dyn Host, event: &mut ShouldApplyEffectTickThisTickEvent<'_>) {
        if event.effect == rites::THIRD_EYE {
            event.apply = true;
        }
    }

    fn apply_effect_tick(&self, host: &dyn Host, event: &mut ApplyEffectTickEvent<'_>) {
        if event.effect == rites::THIRD_EYE
            && event.entity.is_none()
            && let Some(player) = host.acting_player()
        {
            self.rites.saw_third_eye(player.uuid);
        }
    }

    fn inventory_tick(&self, host: &mut dyn Host, event: &mut InventoryTickEvent) {
        if event.slot == rites::HEAD_SLOT
            && host.carried_stack(rites::HEAD_SLOT).is_some_and(|s| &*s.item == rites::GOGGLES)
            && let Some(player) = host.acting_player()
        {
            self.rites.saw_goggles(player.uuid);
        }
    }

    fn loot_drops(&self, host: &mut dyn Host, event: &mut LootDropsEvent<'_>) {
        self.rites.silver_ore_drops(event.context, event.drops);
        // A Core leaving the world carries its store; the store sleeps until it's placed again.
        if let Some((pos, _)) = event.context.block
            && let Some(core) = event.drops.iter_mut().find(|s| &*s.item == CORE)
            && let Some((id, name)) = self.vault.unanchor(&*host, event.context.dimension, pos)
        {
            vault::carry(core, &id, &name);
            if let Some(player) = event.context.player {
                host.tell(player, Text::translate("storage.occultism.dormant", &[Text::literal(name)]));
            }
        }
    }

    fn tooltip(&self, _host: &dyn Host, event: &mut TooltipEvent<'_>) {
        if let Some((_, name)) = vault::carried(event.stack) {
            event.lines.push((Text::translate("storage.occultism.bound", &[Text::literal(name)]), 0xFF8E6CC0));
        }
        let Some(spirit) = spirit_in(Some(event.stack)) else { return };
        let grey = 0xFFAAAAAA;
        event.lines.push((Text::translate(&format!("tooltip.occultism.spirit.{}", spirit.job.name()), &[]), grey));
        event.lines.push((Text::translate(&format!("tooltip.occultism.spirit.{}", spirit.tier.name()), &[]), grey));
        event.lines.push((Text::translate("tooltip.occultism.spirit.place", &[]), 0xFF8E6CC0));
    }
}
