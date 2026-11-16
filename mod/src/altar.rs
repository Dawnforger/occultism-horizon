// Occultism's Demonic Altar: the glue between the game and the altar's rules (logic.rs). A block
// entity Part: six slots (core, input, four outputs), a menu of its own and per-face hopper rules;
// each tick runs the bound spirit's job over Occultism's `occultism:crushing` recipes, read from the
// mod's data like any data pack. On the client: the altar's screen, the core floating over the
// altar while it works, and the cores' tooltips.

use crate::game::{self, id, lock};
use crate::logic::{self, AltarState, Recipe, Spirit, SpiritComponent, Stack, Tick};
use mod_api::prelude::*;
use mod_api::*;
use std::sync::{Arc, Mutex};

pub const ALTAR: &str = "occultism:demonic_altar";
pub const MENU: &str = "occultism:demonic_altar";
pub const RECIPE_TYPE: &str = "occultism:crushing";
pub const SPIRIT_COMPONENT: &str = "occultism:spirit";

/// The crushing recipes, resolved to item ids once per recipe table: a `/reload` brings new
/// `Recipe` values, and their addresses tell the cache it is stale.
#[derive(Default)]
pub struct Altar {
    recipes: Mutex<(Vec<usize>, Arc<[Recipe]>)>,
}

impl Altar {
    fn recipes(&self, host: &dyn Host) -> Arc<[Recipe]> {
        let table = host.recipes_of_type(RECIPE_TYPE);
        let key: Vec<usize> = table.iter().map(|r| Arc::as_ptr(&r.json) as usize).collect();
        let mut cache = lock(&self.recipes);
        if cache.0 != key {
            let parsed: Vec<Recipe> = table.iter().filter_map(|r| parse_recipe(host, r)).collect();
            *cache = (key, parsed.into());
        }
        cache.1.clone()
    }
}

/// An ingredient's items, in the game's order, from a recipe file's JSON.
pub fn ingredient(host: &dyn Host, json: &serde_json::Value) -> Option<Vec<String>> {
    Some(host.recipe_ingredient(&json.to_string()).ok()??.order)
}

/// A result's item, from a recipe file's JSON (`{"id": ...}`).
pub fn result_item(host: &dyn Host, json: &serde_json::Value) -> Option<String> {
    let one = serde_json::json!({"id": json["id"], "count": 1});
    Some(id(&*host, &host.recipe_result(&one.to_string()).ok()??).to_string())
}

/// An `occultism:crushing` file as `CrushingRecipe.CODEC` reads it. A recipe whose ingredient or
/// result names nothing the game has is left out, as NeoForge's `tag_empty` conditions on the
/// original files leave it out.
fn parse_recipe(host: &dyn Host, r: &mod_api::Recipe) -> Option<Recipe> {
    let j = &*r.json;
    let inputs = ingredient(host, &j["ingredient"])?;
    let result = &j["result"];
    let item = match result["type"].as_str() {
        Some("occultism:tag") => {
            let tag = result["tag"].as_str()?;
            ingredient(host, &serde_json::Value::String(format!("#{tag}")))?.into_iter().next()?
        }
        _ => result_item(host, result)?,
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
/// (the rest is in the item's own defaults, which `logic::spirit_of` falls back to).
pub fn spirit_component(stack: &ItemStack) -> Option<SpiritComponent> {
    let c = game::component(stack, SPIRIT_COMPONENT)?;
    Some(SpiritComponent { job: game::text(&c, "job"), tier: game::int(&c, "tier") })
}

pub fn spirit_in(host: &dyn Host, stack: Option<&ItemStack>) -> Option<Spirit> {
    let stack = stack?;
    logic::spirit_of(id(&*host, stack), spirit_component(stack).as_ref())
}

fn to_logic(host: &dyn Host, stack: &Option<ItemStack>) -> Option<Stack> {
    stack.as_ref().map(|s| Stack { item: id(&*host, s).to_string(), count: s.count(), plain: game::plain(s) })
}

fn read_state(nbt: &Compound) -> AltarState {
    AltarState { progress: game::int(nbt, "progress").unwrap_or(0).max(0) as u32, recipe: game::text(nbt, "recipe") }
}

fn write_state(nbt: &mut Compound, state: &AltarState) {
    nbt.insert("progress".into(), Tag::Int(state.progress as i32));
    match &state.recipe {
        Some(r) => nbt.insert("recipe".into(), Tag::String(r.as_str().into())),
        None => nbt.remove(&"recipe".into()),
    };
}

/// What clients draw: the core's item id (empty without one) and whether the altar is working.
pub fn shown(host: &dyn Host, core: Option<&ItemStack>, active: bool) -> Compound {
    let mut tag = Compound::new();
    tag.insert("core".into(), Tag::String(core.map_or("", |c| id(host, c)).into()));
    tag.insert("active".into(), Tag::Byte(active as i8));
    tag
}

/// Keeps `shown` in the block entity's data, and tells clients when it changes.
pub fn show(host: &mut dyn ServerHost, pos: [i32; 3], nbt: &mut Compound, now: Compound) {
    if nbt.get(&"shown".into()) != Some(&Tag::Compound(now.clone())) {
        nbt.insert("shown".into(), Tag::Compound(now));
        host.block_entity_sync(pos);
    }
}

/// The `shown` field a client copy of an altar or bowl has, for its update tag.
pub fn shown_tag(entity: &ModBlockEntity) -> Compound {
    let mut tag = Compound::new();
    if let Some(shown) = entity.nbt.get(&"shown".into()) {
        tag.insert("shown".into(), shown.clone());
    }
    tag
}

/// The item floating over an altar or in a bowl, from its `shown` field: slow and low when idle,
/// fast and bobbing at work over an altar; turning slowly in a bowl, rising and faster in a ritual.
pub fn draw_shown(host: &dyn ClientHost, pos: [i32; 3], entity: &ModBlockEntity, draw: &mut dyn BlockEntityDrawContext, in_bowl: bool) {
    let Some(Tag::Compound(shown)) = entity.nbt.get(&"shown".into()) else { return };
    let Some(core) = game::text(shown, "core").filter(|c| !c.is_empty()) else { return };
    let Some(stack) = game::stack(&*host, &core, 1) else { return };
    let active = matches!(shown.get(&"active".into()), Some(Tag::Byte(1)));
    // The altar is opaque, so its own light is dark: light the core as the air it floats in.
    let above = [pos[0], pos[1] + 1, pos[2]];
    let level = host.level(host.dimension());
    let read = |layer| level.and_then(|l| l.brightness(host, layer, above)).unwrap_or(0).clamp(0, 15) as u32;
    let light = draw.light();
    draw.set_light(read(LightLayer::Sky) * 16 | (read(LightLayer::Block) * 16) << 8);
    let t = draw.game_time() as f32 + draw.partial();
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
    draw.item(&stack, ItemDisplayContext::Fixed, None, 0);
    draw.pop();
    draw.set_light(light);
}

/// The altar's block entity.
pub struct AltarPart(pub Arc<crate::State>);

#[mod_api::horizon_part]
impl BlockPart for AltarPart {}

#[mod_api::horizon_part]
impl BlockEntityPart for AltarPart {
    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        event.ticks = true;
    }

    fn block_entity_tick(&self, host: &mut dyn ServerHost, event: &mut BlockEntityTickEvent<'_>) {
        let recipes = self.0.altar.recipes(&*host);
        let pos = event.pos;
        let entity = &mut *event.entity;
        if entity.items.len() < logic::SLOT_COUNT {
            entity.items.resize(logic::SLOT_COUNT, None);
        }
        // A core a hopper put in the input binds or passes through, whole stack and components.
        let logic_slots: Vec<Option<Stack>> = entity.items.iter().map(|s| to_logic(&*host, s)).collect();
        let input_is_core = spirit_in(&*host, entity.items[logic::SLOT_INPUT].as_ref()).is_some();
        if let Some(to) = logic::stray_core_target(&logic_slots, input_is_core)
            && let Some(core) = entity.items[logic::SLOT_INPUT].take()
        {
            if to == logic::SLOT_CORE && core.count() > 1 {
                let mut one = core.clone();
                let _ = one.set_count(1);
                entity.items[logic::SLOT_CORE] = Some(one);
                entity.items[logic::SLOT_INPUT] = core.shrink(1);
            } else {
                entity.items[to] = Some(core);
            }
        }
        let before: Vec<Option<Stack>> = entity.items.iter().map(|s| to_logic(&*host, s)).collect();
        let mut slots = before.clone();
        let mut state = read_state(&entity.nbt);
        let spirit = spirit_in(&*host, entity.items[logic::SLOT_CORE].as_ref());
        let max_stack = |item: &str| game::item(&*host, item).and_then(|k| host.max_stack_size(k)).unwrap_or(64);
        let tick = logic::tick(&mut state, &mut slots, &recipes, &max_stack, spirit);

        // Write back only the slots the tick changed, so untouched stacks keep their components.
        for i in 0..logic::SLOT_COUNT {
            if slots[i] != before[i] {
                entity.items[i] = slots[i].as_ref().and_then(|s| match &entity.items[i] {
                    Some(old) if id(&*host, old) == s.item => {
                        let mut kept = old.clone();
                        kept.set_count(s.count).ok().map(|()| kept)
                    }
                    _ => game::stack(&*host, &s.item, s.count),
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
        let now = shown(&*host, entity.items[logic::SLOT_CORE].as_ref(), active);
        show(host, pos, &mut entity.nbt, now);
    }

    fn block_entity_update_tag(&self, event: &mut BlockEntityUpdateTagEvent<'_>) -> Option<Compound> {
        Some(shown_tag(event.entity))
    }

    fn block_entity_submit(&self, host: &dyn ClientHost, event: &mut BlockEntitySubmitEvent<'_>) {
        draw_shown(host, event.pos, event.entity, &mut *event.draw, false);
    }

    fn block_entity_render_bounding_box(&self, _host: &dyn ClientHost, event: &mut BlockEntityRenderBoundingBoxEvent<'_>) {
        event.bounds[4] = event.bounds[4].max(2.0);
    }

    fn screen(&self, _host: &dyn Host, _event: &mut ScreenEvent<'_>) -> Option<Screen> {
        // 176 x 166 like a furnace: core on the left, input, the progress arrow, a 2 x 2 output.
        let mut slots = vec![None; logic::SLOT_COUNT];
        slots[logic::SLOT_CORE] = Some([26.0, 35.0]);
        slots[logic::SLOT_INPUT] = Some([56.0, 35.0]);
        slots[2] = Some([116.0, 26.0]);
        slots[3] = Some([134.0, 26.0]);
        slots[4] = Some([116.0, 44.0]);
        slots[5] = Some([134.0, 44.0]);
        Some(Screen {
            width: 176.0,
            height: 166.0,
            slots,
            inventory_y: 84.0,
            inventory_x: 8.0,
            own_labels: false,
            own_background: false,
            recipe_book: None,
            recipe_book_toggle: None,
        })
    }

    fn screen_draw(&self, host: &dyn ClientHost, event: &mut ScreenDrawEvent<'_>) {
        let get = |i: usize| event.data.get(i).copied().unwrap_or(0);
        let [ox, oy] = event.origin;
        let (progress, total, tier, state) = (get(logic::DATA_PROGRESS), get(logic::DATA_TOTAL), get(logic::DATA_TIER), get(logic::DATA_STATE));
        let draw = &mut *event.draw;
        // The arrow from input to output: a dark track and a filled part.
        let track = [ox + 79.0, oy + 38.0, 26.0, 10.0];
        draw.fill(track, 0xFF3B2A4A);
        if total > 0 {
            let w = 26.0 * (progress.clamp(0, total) as f32 / total as f32);
            draw.fill([track[0], track[1], w, track[3]], if state == 2 { 0xFFB23A3A } else { 0xFFB06CFF });
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
        let lang = |key: &str| host.translate(key).unwrap_or_else(|| key.to_string());
        draw.text(&lang(&format!("gui.occultism.demonic_altar.{status}")), [ox + 8.0, oy + 63.0], 0xFF404040, false);
        if let Some(t) = logic::Tier::from_level(tier) {
            draw.text(&lang(&format!("tooltip.occultism.spirit.{}", t.name())), [ox + 116.0, oy + 63.0], 0xFF6A3D9A, false);
        }
    }

    fn menu_quick_move(&self, host: &mut dyn ServerHost, event: &mut MenuQuickMoveEvent<'_>) -> Flow {
        if event.slot < event.container_slots {
            // Shift-clicks out of the altar go to the inventory as a chest's do.
            return Flow::Continue;
        }
        let component = spirit_component(event.stack);
        let core_empty = event.entity.items.get(logic::SLOT_CORE).is_none_or(Option::is_none);
        match logic::quick_move_target(id(&*host, event.stack), component.as_ref(), core_empty) {
            // A single core: the game's own routing puts it in the core slot, its first that takes
            // it (the hook can't empty the clicked slot: upstream/fork-gaps.md).
            Some(logic::SLOT_CORE) if event.stack.count() == 1 => Flow::Continue,
            Some(logic::SLOT_CORE) => {
                let mut one = event.stack.clone();
                let _ = one.set_count(1);
                if event.entity.items.len() < logic::SLOT_COUNT {
                    event.entity.items.resize(logic::SLOT_COUNT, None);
                }
                event.entity.items[logic::SLOT_CORE] = Some(one);
                let _ = event.stack.set_count(event.stack.count() - 1);
                Flow::Cancel
            }
            Some(_) => Flow::Continue,
            // A core with the core slot taken: leave it where it is, never in the input.
            None => Flow::Cancel,
        }
    }
}

/// The altar menu's slots: what each takes.
pub struct AltarMenu;

#[mod_api::horizon_part]
impl MenuPart for AltarMenu {
    fn menu_slot_rules(&self, _host: &dyn Host, event: &mut MenuSlotRulesEvent<'_>) {
        let component = spirit_component(event.stack);
        let (may_place, max) = logic::slot_rule(event.slot, game::known(event.stack.item()), component.as_ref());
        event.may_place = may_place;
        event.max_stack = event.max_stack.min(max);
    }
}

/// The altar's container: core, input and outputs, by face.
pub fn container() -> ModContainer {
    ModContainer {
        slots: logic::SLOT_COUNT,
        faces: logic::face_slots(),
        inputs: vec![logic::SLOT_INPUT],
        outputs: logic::SLOTS_OUTPUT.to_vec(),
        menu: Some(MENU.to_string()),
    }
}

/// A spirit core's tooltip: its job and rank, and how to bind it.
pub struct CoreTooltip;

#[mod_api::horizon_part]
impl ItemPart for CoreTooltip {
    fn tooltip(&self, host: &dyn ClientHost, event: &mut TooltipEvent<'_>) {
        let Some(spirit) = spirit_in(host, Some(event.stack)) else { return };
        let grey = 0xFFAAAAAA;
        event.lines.push((Text::translate(format!("tooltip.occultism.spirit.{}", spirit.job.name()), Vec::new()), grey));
        event.lines.push((Text::translate(format!("tooltip.occultism.spirit.{}", spirit.tier.name()), Vec::new()), grey));
        event.lines.push((Text::translate("tooltip.occultism.spirit.place", Vec::new()), 0xFF8E6CC0));
    }
}
