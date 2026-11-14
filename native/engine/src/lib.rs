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

pub mod fire;
pub mod logic;
pub mod mining;
mod mineshaft;
pub mod network;
pub mod ritual;
mod rites;
mod spirit_fire;
pub mod storage;
pub mod terminal;
mod terminal_ui;
mod vault;

use horizon_client::mod_api::*;
use logic::{AltarState, Recipe, Spirit, SpiritComponent, Stack, Tick};
use rites::{BOWL, GOLDEN_BOWL};
use mineshaft::MINESHAFT;
use vault::{ACCESSOR, ACTUATOR, CORE, WORMHOLE};
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
    /// The storage terminal on the client.
    ui: terminal_ui::Ui,
    shafts: mineshaft::Mineshafts,
    spirit_fire: spirit_fire::SpiritFire,
    /// When each player last linked a gate item: Horizon asks `right_click_block` once per hand.
    linked_at: Mutex<std::collections::HashMap<u128, std::time::Instant>>,
    /// Links waiting for the player's held gate item's next inventory tick: the store and its name.
    pending_links: Mutex<std::collections::HashMap<u128, (String, String)>>,
    /// The players holding their sneak key, as their clients last said (on the server).
    sneaking: Mutex<std::collections::HashSet<u128>>,
    /// The sneak state this client last told the server.
    sneak_sent: Mutex<Option<bool>>,
}

/// The crushing recipes, resolved to item ids once per recipe table: a `/reload` brings new
/// `CustomRecipe` values, and their addresses tell the cache it is stale.
#[derive(Default)]
struct RecipeCache {
    key: Vec<usize>,
    recipes: Arc<[Recipe]>,
}

impl DemonicAltar {
    /// The recall ritual finished on the golden bowl at `pos`: a new Core for each of the caster's
    /// lost stores, dropped on the bowl.
    fn recall(&self, host: &mut dyn Host, pos: [i32; 3], caster: u128) {
        let cores = self.vault.recall(&*host, caster);
        if cores.is_empty() {
            host.tell(caster, Text::translate("storage.occultism.recall_none", &[]));
            return;
        }
        let at = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 1.0, f64::from(pos[2]) + 0.5];
        let dimension = host.dimension();
        for (core, name) in cores {
            let mut nbt = Compound::new();
            nbt.insert("Item".into(), horizon_client::save::item_to_nbt(&core));
            nbt.insert("PickupDelay".into(), Tag::Short(10));
            if host.summon(dimension, "minecraft:item", at, nbt).is_none() {
                // No item entity (an unloaded spot): the Core goes straight to the caster.
                let _ = host.give_stack(caster, core);
            }
            host.tell(caster, Text::translate("storage.occultism.recalled", &[Text::literal(name)]));
        }
    }
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

/// The projection over a Storage Actuator, after Occultism's `StorageControllerGeoRenderer`: its
/// dimensional matrix (27 small cubes) turning a block and a quarter up, in colours cycling as
/// Occultism's do, and one chalk glyph per plate circling it (`network::glyph_orbit`), in its
/// tier's colour, always facing the viewer as a particle does. It glows: drawn at full light.
fn draw_projection(host: &dyn Host, draw: &mut BlockEntityDraw, plates: &[u8]) {
    let t = draw.game_time as f32 + draw.partial;
    let light = draw.light;
    draw.light = 0xF0F0;
    // The matrix's centre: Occultism's model is 9 texels tall from 5 above its pose's origin, set
    // a little higher than Occultism's so the lowest orbits clear the block.
    let rise = 1.25 + 0.2;
    let centre = [0.5, rise + 9.0 / 16.0, 0.5];

    // The glyphs: flat squares turned to the camera. The camera's right and up, from its yaw and
    // pitch (Minecraft's: yaw 0 looks south, pitch down is positive).
    if let Some(camera) = host.camera() {
        let (yaw, pitch) = (camera.yaw.to_radians(), camera.pitch.to_radians());
        let forward = [-yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos()];
        let right = [-yaw.cos(), 0.0, -yaw.sin()];
        let up = [
            right[1] * forward[2] - right[2] * forward[1],
            right[2] * forward[0] - right[0] * forward[2],
            right[0] * forward[1] - right[1] * forward[0],
        ];
        // A square of half-size `size` at `c`, turned to the camera, pushed `back` away from it so
        // what is drawn at one place layers: trail, then glow, then the glyph in front.
        let billboard = |draw: &mut BlockEntityDraw, sprite: &str, c: [f32; 3], size: f32, back: f32, tint: u32| {
            let corner = |r: f32, u: f32| [0, 1, 2].map(|i| c[i] + (right[i] * r + up[i] * u) * size + forward[i] * back);
            let quad = [corner(-1.0, 1.0), corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0)];
            host.draw_quad(draw, sprite, quad, [0.0, 0.0, 16.0, 16.0], tint);
        };
        let at_time = |slot: usize, tier: u8, when: f32| {
            let at = network::glyph_orbit(slot, tier, when);
            [centre[0] + at[0], centre[1] + at[1], centre[2] + at[2]]
        };
        for (slot, tier) in plates.iter().copied().enumerate() {
            let (Some(colour), Some(glow)) = (network::glyph_colour(tier), network::glyph_glow(tier)) else { continue };
            // A trail of motes along the orbit behind it, smaller and fainter the further back.
            for k in 1..=network::TRAIL_MOTES {
                let mote = at_time(slot, tier, t - k as f32 * network::TRAIL_STEP);
                let fade = k as f32 / network::TRAIL_MOTES as f32;
                let strength = ((fade * 4.0) as usize).min(3);
                let breathe = 1.0 + 0.15 * (t * 0.3 + k as f32).sin();
                billboard(draw, &format!("occultism:block/storage_glow_{strength}"), mote, 0.05 * (1.0 - 0.5 * fade) * breathe, 0.02, glow);
            }
            let c = at_time(slot, tier, t);
            // A soft glow behind it, breathing slowly.
            let pulse = 1.0 + 0.08 * (t * 0.12 + slot as f32 * 1.3).sin();
            billboard(draw, "occultism:block/storage_glow_0", c, 0.24 * pulse, 0.01, glow);
            // The glyph; each slot shows a sign of its own.
            billboard(draw, &format!("occultism:block/chalk_glyph/{colour}/{}", 1 + slot * 5 % 14), c, 0.16, 0.0, 0xFFFFFF);
        }
    }

    // The matrix: 3 x 3 x 3 cubes of 2 texels, 3 apart, turning slowly (Occultism: 22.5 degrees a
    // second), tinted by its hue cycle.
    let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis()) as i64;
    let scale = 100 - ((millis / 16 / 2) % 160 - 80).abs();
    let saturation = ((millis as f64 / 8.0).to_radians().sin() * 0.5 + 0.5) as f32;
    let tint = hsb(0.01 * scale as f32, saturation, 0.01 * scale as f32);
    draw.push();
    draw.translate([0.5, rise, 0.5]);
    draw.rotate_about([0.0, 1.0, 0.0], (millis / 16 % 360) as f32, [0.0; 3]);
    let face = Some(CubeFace { sprite: "occultism:block/dimensional_matrix", uv: None });
    for x in [-4.0, -1.0, 2.0] {
        for y in [5.0, 8.0, 11.0] {
            for z in [-4.0, -1.0, 2.0] {
                host.draw_cube(draw, [x, y, z], [x + 2.0, y + 2.0, z + 2.0], [face; 6], tint);
            }
        }
    }
    draw.pop();
    draw.light = light;
}

/// Java's `Color.HSBtoRGB`, as 0xRRGGBB.
fn hsb(hue: f32, saturation: f32, brightness: f32) -> u32 {
    let channel = |v: f32| ((v * 255.0 + 0.5) as u32).min(255);
    if saturation == 0.0 {
        let v = channel(brightness);
        return v << 16 | v << 8 | v;
    }
    let h = (hue - hue.floor()) * 6.0;
    let f = h - h.floor();
    let p = brightness * (1.0 - saturation);
    let q = brightness * (1.0 - saturation * f);
    let u = brightness * (1.0 - saturation * (1.0 - f));
    let (r, g, b) = match h as u32 {
        0 => (brightness, u, p),
        1 => (q, brightness, p),
        2 => (p, brightness, u),
        3 => (p, q, brightness),
        4 => (u, p, brightness),
        _ => (brightness, p, q),
    };
    channel(r) << 16 | channel(g) << 8 | channel(b)
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
        event.types.push(vault::LINK_COMPONENT.to_string());
        event.types.push("occultism:miner".to_string());
    }

    fn block_entity(&self, _host: &mut dyn Host, event: &mut BlockEntityEvent<'_>) -> Option<(String, Compound)> {
        [ALTAR, BOWL, GOLDEN_BOWL, ACTUATOR, WORMHOLE, MINESHAFT]
            .contains(&event.block)
            .then(|| (event.block.to_string(), Compound::new()))
    }

    fn block_entity_ticks(&self, _host: &dyn Host, event: &mut BlockEntityTicksEvent<'_>) {
        if [ALTAR, BOWL, GOLDEN_BOWL, ACTUATOR, WORMHOLE, MINESHAFT].contains(&event.block) {
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
        if event.block == MINESHAFT {
            // A lamp in from the top and sides, results out from the bottom.
            let outputs: Vec<usize> = mining::OUTPUTS.collect();
            let mut faces: [Vec<usize>; 6] = std::array::from_fn(|_| vec![mining::SLOT_LAMP]);
            faces[0] = outputs.clone();
            return Some(ModContainer { slots: mining::SLOT_COUNT, faces, inputs: vec![mining::SLOT_LAMP], outputs, menu: Some(mineshaft::MENU.to_string()) });
        }
        if event.block == ACTUATOR || event.block == WORMHOLE {
            // Hoppers in from the top and sides. The terminal hands items to players, so the bottom
            // face's slots stay empty (logistics go through wormholes).
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
        if event.entity.id == MINESHAFT {
            self.shafts.tick(host, event.pos, event.entity);
            return;
        }
        if event.entity.id == WORMHOLE {
            self.vault.gate_tick(&*host, event.pos, event.entity);
            return;
        }
        if event.entity.id == ACTUATOR {
            let (notice, sync) = self.vault.tick(&*host, event.pos, event.entity);
            if let Some(notice) = notice {
                host.tell(notice.player, Text::translate(notice.key, &[Text::literal(notice.name)]));
            }
            if sync {
                host.block_entity_sync(event.pos);
            }
            return;
        }
        if event.entity.id == BOWL || event.entity.id == GOLDEN_BOWL {
            let pos = event.pos;
            if event.entity.id == BOWL {
                self.rites.bowl_tick(&*host, pos, event.entity);
            } else {
                if let Some((effect, caster)) = self.rites.golden_tick(host, pos, event.entity)
                    && effect == ritual::RECALL_STORAGE
                {
                    self.recall(host, pos, caster);
                }
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
        if event.entity.id == ACTUATOR {
            let mut tag = Compound::new();
            if let Some(plates) = event.entity.nbt.get(&"plates".into()) {
                tag.insert("plates".into(), plates.clone());
            }
            return Some(tag);
        }
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
        if event.entity.id == ACTUATOR {
            let plates = match event.entity.nbt.get(&"plates".into()) {
                Some(Tag::ByteArray(p)) => p.clone(),
                _ => Vec::new(),
            };
            draw_projection(host, event.draw, &plates);
            return;
        }
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
        if event.block == ACTUATOR {
            // The glyphs circle under a block from the matrix's centre.
            let b = &mut event.bounds;
            (b[0], b[2], b[3], b[5]) = (b[0].min(-0.5), b[2].min(-0.5), b[3].max(1.5), b[5].max(1.5));
            b[4] = b[4].max(2.8);
        }
    }

    fn screen(&self, _host: &dyn Host, event: &mut ScreenEvent<'_>) -> Option<NativeScreen> {
        if terminal_ui::is_terminal(event.block) {
            return Some(terminal_ui::Ui::screen());
        }
        if event.block == MINESHAFT {
            // 176 x 166: the lamp on the left, the progress arrow, a 3 x 3 output.
            let mut slots = vec![None; mining::SLOT_COUNT];
            slots[mining::SLOT_LAMP] = Some([44.0, 35.0]);
            for (n, i) in mining::OUTPUTS.enumerate() {
                slots[i] = Some([98.0 + 18.0 * (n % 3) as f32, 17.0 + 18.0 * (n / 3) as f32]);
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
        if terminal_ui::is_terminal(event.block) {
            return self.ui.click(event.at, event.right);
        }
        None
    }

    fn screen_tooltip(&self, _host: &dyn Host, event: &mut ScreenTooltipEvent<'_>) {
        if terminal_ui::is_terminal(event.block) {
            self.ui.tooltip(event.at, event.data, event.lines);
        }
    }

    fn screen_mouse_scrolled_pre(&self, _host: &dyn Host, event: &mut ScreenMouseScrolledPreEvent<'_>) -> Flow {
        self.ui.scroll(event.screen, event.scroll[1])
    }

    fn screen_key_pressed_pre(&self, _host: &dyn Host, event: &mut ScreenKeyPressedPreEvent<'_>) -> Flow {
        self.ui.key(event.screen, event.key)
    }

    fn screen_char_typed_pre(&self, _host: &dyn Host, event: &mut ScreenCharTypedPreEvent<'_>) -> Flow {
        self.ui.typed(event.screen, event.c)
    }

    fn screen_closing(&self, _host: &dyn Host, event: &mut ScreenClosingEvent<'_>) {
        self.ui.closed(event.screen);
    }

    fn screen_render_post(&self, host: &dyn Host, event: &mut ScreenRenderPostEvent<'_>) {
        self.ui.draw_remote(host, event.batch, event.screen);
    }

    fn screen_mouse_button_pressed_pre(&self, _host: &dyn Host, event: &mut ScreenMouseButtonPressedPreEvent<'_>) -> Flow {
        self.ui.remote_press(event.screen, event.button)
    }

    fn tick(&self, host: &mut dyn Host, _event: &mut TickEvent) {
        // The Accessor's requests go from the window's own tick: on the server thread sending
        // fails, and what didn't go waits for the next tick that can.
        let mut waiting = self.ui.outbox().into_iter();
        while let Some(m) = waiting.next() {
            if host.net_send_to_server(terminal::CHANNEL, &m).is_err() {
                self.ui.requeue(std::iter::once(m).chain(waiting).collect());
                break;
            }
        }
    }

    fn net_from_client(&self, host: &mut dyn Host, event: &mut NetFromClientEvent<'_>) {
        if event.channel == terminal::CHANNEL
            && let Some(request) = terminal::Request::decode(event.data)
        {
            if let terminal::Request::Sneak(down) = request {
                let mut sneaking = self.sneaking.lock().unwrap_or_else(|e| e.into_inner());
                if down {
                    sneaking.insert(event.player);
                } else {
                    sneaking.remove(&event.player);
                }
                return;
            }
            self.vault.remote_request(host, event.player, request);
        }
    }

    fn movement_input(&self, _host: &dyn Host, event: &mut MovementInputEvent) {
        // The server's right-click hook doesn't say whether the player sneaks: tell it.
        let mut sent = self.sneak_sent.lock().unwrap_or_else(|e| e.into_inner());
        if *sent != Some(event.sneak) {
            *sent = Some(event.sneak);
            self.ui.send(terminal::Request::Sneak(event.sneak).encode());
        }
    }

    fn use_item(&self, host: &mut dyn Host, event: &mut UseItemEvent<'_>) -> Flow {
        if host.acting_player().is_some_and(|p| self.rites.offering(p.uuid)) {
            return Flow::Cancel;
        }
        // The Storage Accessor: its store's terminal, wherever the player is.
        if &*event.stack.item != ACCESSOR {
            return Flow::Continue;
        }
        let Some(player) = host.acting_player() else { return Flow::Cancel };
        if let Err((key, name)) = self.vault.watch_remote(&*host, player.uuid, event.stack) {
            host.tell(player.uuid, Text::translate(key, &[Text::literal(name)]));
        }
        Flow::Cancel
    }

    fn net_from_server(&self, host: &mut dyn Host, event: &mut NetFromServerEvent<'_>) {
        if event.channel == terminal::CHANNEL && self.ui.receive(event.data) {
            host.mod_screen_open(terminal_ui::MOD_ID);
        }
    }

    fn player_container_open(&self, host: &mut dyn Host, event: &mut PlayerContainerOpenEvent<'_>) {
        if event.menu == vault::MENU
            && let Some(pos) = event.pos
        {
            self.vault.watch(event.player, event.dimension, pos, &*host);
        }
    }

    fn player_container_close(&self, _host: &mut dyn Host, event: &mut PlayerContainerCloseEvent<'_>) {
        if event.menu == vault::MENU {
            self.vault.unwatch(event.player);
        }
    }

    fn player_logged_out(&self, _host: &mut dyn Host, event: &mut PlayerLoggedOutEvent<'_>) {
        self.sneaking.lock().unwrap_or_else(|e| e.into_inner()).remove(&event.player);
        self.vault.unwatch(event.player);
    }

    fn server_tick(&self, host: &mut dyn Host, event: &mut ServerTickEvent<'_>) {
        if matches!(event.phase, ServerTickPhase::ServerTickEnd { .. }) {
            self.vault.pump(host);
            self.spirit_fire.tick(host);
            self.rites.draw(host);
        }
    }

    fn use_item_on(&self, host: &mut dyn Host, event: &mut UseItemOnEvent<'_>) -> Flow {
        self.rites.use_chalk(host, event.stack, event.pos, event.face)
    }

    fn entity_tick_listened(&self, _host: &mut dyn Host, event: &mut EntityTickListenedEvent) {
        // Item ticks only while a flint and steel waits to find Demon's Dream around it.
        if self.spirit_fire.listening() {
            event.listened = true;
        }
    }

    fn entity_tick_pre(&self, host: &mut dyn Host, event: &mut EntityTickPreEvent<'_>) -> Flow {
        if event.kind == "minecraft:item" {
            self.spirit_fire.item_tick(host, event.dimension, event.entity);
        }
        Flow::Continue
    }

    fn block_entity_inside(&self, event: &mut BlockEntityInsideEvent<'_>) {
        self.spirit_fire.inside(event.dimension, event.entity, event.kind, event.position);
    }

    fn menu_button(&self, host: &mut dyn Host, event: &mut MenuButtonEvent<'_>) {
        if event.entity.id != ACTUATOR && event.entity.id != WORMHOLE {
            return;
        }
        let Some(player) = host.acting_player() else { return };
        if let Some((serial, right)) = terminal::grid_click(event.id) {
            self.vault.take_to(host, player.uuid, event.pos, serial, right);
        } else if let Some(slot) = terminal_ui::upgrade_click(event.id) {
            self.vault.remove_plate(host, player.uuid, event.pos, slot);
        }
    }

    fn destroy_block(&self, host: &mut dyn Host, event: &mut DestroyBlockEvent) -> Flow {
        // Only those a store trusts (and operators) break its Core.
        if host.block(event.pos) == CORE
            && let Some(player) = host.acting_player()
            && let Err(name) = self.vault.may_break(&*host, &player.dimension, event.pos, &player)
        {
            host.tell(player.uuid, Text::translate("storage.occultism.refused", &[Text::literal(name)]));
            return Flow::Cancel;
        }
        // A creative break drops nothing, so the Core comes to the breaker's inventory carrying its
        // store (as vanilla's pick-block with its data would); `loot_drops` does it otherwise.
        if host.block(event.pos) == CORE
            && let Some(player) = host.acting_player()
            && player.game_mode == 1
            && let Some((core, name)) = self.vault.unanchor(&*host, &player.dimension, event.pos)
        {
            let _ = host.give_stack(player.uuid, core);
            host.tell(player.uuid, Text::translate("storage.occultism.dormant", &[Text::literal(name)]));
        }
        Flow::Continue
    }

    fn place_block(&self, host: &mut dyn Host, event: &mut PlaceBlockEvent<'_>) {
        if &*event.stack.item == CORE {
            self.vault.placed(host.dimension(), event.pos, event.stack, host.acting_player().map(|p| p.uuid));
        }
        if &*event.stack.item == WORMHOLE {
            self.vault.gate_placed(host.dimension(), event.pos, event.stack);
        }
    }

    fn world_lifecycle(&self, host: &mut dyn Host, event: &mut WorldLifecycleEvent<'_>) {
        match event.phase {
            WorldPhase::WorldCloseFinal => self.vault.close(&*host, true),
            WorldPhase::WorldCloseFailed => self.vault.close(&*host, false),
            _ => {}
        }
    }

    fn world_saved(&self, host: &mut dyn Host, _event: &mut WorldSavedEvent) {
        self.vault.saved(&*host);
    }

    fn register_commands(&self, _host: &mut dyn Host, event: &mut RegisterCommandsEvent<'_>) {
        event.commands.push(NativeCommand::new("occultism trust <player:player>"));
        event.commands.push(NativeCommand::new("occultism untrust <player:player>"));
        event.commands.push(NativeCommand::new("occultism trusted"));
    }

    fn perform_command(&self, host: &mut dyn Host, event: &mut PerformCommandEvent<'_>) -> Flow {
        let words: Vec<&str> = event.command.split_whitespace().collect();
        let owner = event.player.uuid;
        match words.as_slice() {
            ["occultism", verb @ ("trust" | "untrust"), name] => {
                let Some(player) = host.server_players().into_iter().find(|p| p.name.eq_ignore_ascii_case(name)) else {
                    host.feedback(Text::translate("storage.occultism.trust.unknown", &[Text::literal(*name)]));
                    return Flow::Cancel;
                };
                let line = match self.vault.trust(&*host, owner, player.uuid, *verb == "trust") {
                    Ok(store) => Text::translate(&format!("storage.occultism.{verb}ed"), &[Text::literal(player.name), Text::literal(store)]),
                    Err(key) => Text::translate(key, &[]),
                };
                host.feedback(line);
                Flow::Cancel
            }
            ["occultism", "trusted"] => {
                let line = match self.vault.trusted(&*host, owner) {
                    None => Text::translate("storage.occultism.trust.none", &[]),
                    Some((store, list)) => {
                        let players = host.server_players();
                        let names: Vec<String> = list
                            .iter()
                            .map(|u| players.iter().find(|p| p.uuid == *u).map_or_else(|| format!("{u:032x}"), |p| p.name.clone()))
                            .collect();
                        let names = if names.is_empty() { "-".to_string() } else { names.join(", ") };
                        Text::translate("storage.occultism.trust.list", &[Text::literal(store), Text::literal(names)])
                    }
                };
                host.feedback(line);
                Flow::Cancel
            }
            _ => Flow::Continue,
        }
    }

    fn screen_draw(&self, host: &dyn Host, event: &mut ScreenDrawEvent<'_>) {
        if terminal_ui::is_terminal(event.block) {
            self.ui.draw(host, event.batch, event.origin, event.data);
            return;
        }
        if event.block == MINESHAFT {
            // The arrow from lamp to outputs fills as the miner works; red while the outputs are full.
            let get = |i: usize| event.data.get(i).copied().unwrap_or(0);
            let [ox, oy] = event.origin;
            let track = [ox + 66.0, oy + 38.0, 26.0, 10.0];
            host.fill(event.batch, track, 0xFF3B2A4A);
            let (done, of, full) = (get(0), get(1), get(2) != 0);
            if of > 0 {
                let w = 26.0 * (done.clamp(0, of) as f32 / of as f32);
                host.fill(event.batch, [track[0], track[1], w, track[3]], if full { 0xFFB23A3A } else { 0xFFB06CFF });
            }
            let status = match (of > 0, full) {
                (false, _) => "no_miner",
                (true, true) => "full",
                (true, false) => "mining",
            };
            let label = horizon_client::lang::get(&format!("gui.occultism.mineshaft.{status}"));
            host.text(event.batch, &label, [ox + 8.0, oy + 63.0], 0x404040, false);
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
        if event.menu == mineshaft::MENU {
            // The lamp slot takes one miner; the outputs take nothing from players.
            let lamp = event.slot == mining::SLOT_LAMP;
            event.may_place = lamp && mineshaft::is_miner(event.stack);
            if lamp {
                event.max_stack = 1;
            }
            return;
        }
        if event.menu != MENU {
            return;
        }
        let component = spirit_component(event.stack);
        let (may_place, max) = logic::slot_rule(event.slot, &event.stack.item, component.as_ref());
        event.may_place = may_place;
        event.max_stack = event.max_stack.min(max);
    }

    fn menu_quick_move(&self, host: &mut dyn Host, event: &mut MenuQuickMoveEvent<'_>) -> Flow {
        // A shift-click from the inventory on the terminal goes straight into the store: it has no
        // input slots to show.
        if event.entity.id == MINESHAFT && event.slot >= event.container_slots {
            // A miner shift-clicked from the inventory goes into an empty lamp slot.
            if mineshaft::is_miner(event.stack) && event.entity.items.first().is_none_or(Option::is_none) {
                if event.entity.items.is_empty() {
                    event.entity.items.resize(mining::SLOT_COUNT, None);
                }
                event.entity.items[mining::SLOT_LAMP] = Some(ItemStack { count: 1, ..event.stack.clone() });
                event.stack.count -= 1;
            }
            return Flow::Cancel;
        }
        if (event.entity.id == ACTUATOR || event.entity.id == WORMHOLE) && event.slot >= event.container_slots {
            if let Some(player) = host.acting_player() {
                // A stabilizer plate goes into an upgrade slot, one at a time; anything else (and
                // a plate with every slot taken) into the store.
                if self.vault.upgrade(&*host, player.uuid, event.pos, event.stack) {
                    event.stack.count -= 1;
                } else {
                    let took = self.vault.deposit(&*host, player.uuid, event.pos, event.stack);
                    event.stack.count -= took.min(event.stack.count);
                }
            }
            return Flow::Cancel;
        }
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
        let block = host.block(event.pos);
        let Some(player) = host.acting_player() else { return self.rites.right_click(host, event.pos) };
        let held = host.held();
        let sneaking = self.sneaking.lock().unwrap_or_else(|e| e.into_inner()).contains(&player.uuid);
        if let Some((item, _, _)) = &held {
            self.spirit_fire.right_click(&player.dimension, event.pos, event.face, item);
        }
        // A wormhole or an Accessor in hand, sneak-used on an Actuator: linked to its store, as
        // Occultism's. The held stack is rewritten on its next inventory tick, where a mod may
        // change it.
        if block == ACTUATOR
            && sneaking
            && let Some((held, _, _)) = &held
            && (held == WORMHOLE || held == ACCESSOR)
        {
            let mut last = self.linked_at.lock().unwrap_or_else(|e| e.into_inner());
            let again = last.get(&player.uuid).is_some_and(|t| t.elapsed() < std::time::Duration::from_millis(150));
            last.insert(player.uuid, std::time::Instant::now());
            drop(last);
            if !again {
                match self.vault.link(&*host, player.uuid, event.pos) {
                    Some((id, name)) => {
                        self.pending_links.lock().unwrap_or_else(|e| e.into_inner()).insert(player.uuid, (id, name.clone()));
                        host.tell(player.uuid, Text::translate("storage.occultism.linked", &[Text::literal(name)]));
                    }
                    None => {
                        if let Err(name) = self.vault.may_open(&*host, &player.dimension, event.pos, player.uuid) {
                            host.tell(player.uuid, Text::translate("storage.occultism.refused", &[Text::literal(name)]));
                        }
                    }
                }
            }
            return Flow::Cancel;
        }
        // Sneaking with something in hand, the block doesn't open: the item is used on it (a
        // wormhole placed against another), so the refusals below would only get in the way.
        if sneaking && held.is_some() {
            return self.rites.right_click(host, event.pos);
        }
        // A wormhole opens only while its store is awake.
        if block == WORMHOLE
            && let Err(name) = self.vault.gate_open(&*host, &player.dimension, event.pos)
        {
            host.tell(player.uuid, Text::translate("storage.occultism.gate_dormant", &[Text::literal(name)]));
            return Flow::Cancel;
        }
        // A store's terminal opens only for those it trusts.
        if (block == ACTUATOR || block == WORMHOLE)
            && let Err(name) = self.vault.may_open(&*host, &player.dimension, event.pos, player.uuid)
        {
            host.tell(player.uuid, Text::translate("storage.occultism.refused", &[Text::literal(name)]));
            return Flow::Cancel;
        }
        self.rites.right_click(host, event.pos)
    }

    fn state_for_placement(&self, level: &dyn LevelHost, event: &mut StateForPlacementEvent<'_>) -> Option<Option<BlockStateId>> {
        // A wormhole faces away from the block it was placed against (Occultism's clicked face).
        if event.context.item == WORMHOLE {
            let facing = match event.context.face {
                Direction::Down => "down",
                Direction::Up => "up",
                Direction::North => "north",
                Direction::South => "south",
                Direction::West => "west",
                Direction::East => "east",
            };
            return level.state_with_property(event.state, "facing", facing).map(Some);
        }
        None
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
        if event.selected
            && let Some(player) = host.acting_player()
        {
            self.rites.wear_chalk(host, player.uuid, event.slot);
            self.rites.hand_over(host, player.uuid, event.slot);
        }
        if event.selected
            && let Some(player) = host.acting_player()
            && let Some((id, name)) = self.pending_links.lock().unwrap_or_else(|e| e.into_inner()).remove(&player.uuid)
            && let Some(held) = host.carried_stack(event.slot)
            && (&*held.item == WORMHOLE || &*held.item == ACCESSOR)
        {
            vault::set_link(held, &id, &name);
        }
        if event.slot == rites::HEAD_SLOT
            && host.carried_stack(rites::HEAD_SLOT).is_some_and(|s| &*s.item == rites::GOGGLES)
            && let Some(player) = host.acting_player()
        {
            self.rites.saw_goggles(player.uuid);
        }
    }

    fn loot_drops(&self, host: &mut dyn Host, event: &mut LootDropsEvent<'_>) {
        self.rites.silver_ore_drops(event.context, event.drops);
        self.rites.oak_log_drops(event.context, event.drops);
        self.spirit_fire.drops(event.context, event.drops);
        // A Core leaving the world carries its store; the store sleeps until it's placed again.
        if let Some((pos, _)) = event.context.block
            && let Some(core) = event.drops.iter_mut().find(|s| &*s.item == CORE)
            && let Some((carrying, name)) = self.vault.unanchor(&*host, event.context.dimension, pos)
        {
            *core = carrying;
            if let Some(player) = event.context.player {
                host.tell(player, Text::translate("storage.occultism.dormant", &[Text::literal(name)]));
            }
        }
        // A wormhole leaving the world keeps its link.
        if let Some((pos, _)) = event.context.block
            && let Some(gate) = event.drops.iter_mut().find(|s| &*s.item == WORMHOLE)
            && let Some((id, name)) = self.vault.gate_removed(&*host, event.context.dimension, pos)
        {
            vault::set_link(gate, &id, &name);
        }
    }

    fn tooltip(&self, _host: &dyn Host, event: &mut TooltipEvent<'_>) {
        if let Some((_, name, _)) = vault::carried(event.stack) {
            event.lines.push((Text::translate("storage.occultism.bound", &[Text::literal(name)]), 0xFF8E6CC0));
        }
        if let Some((_, name)) = vault::linked(event.stack) {
            event.lines.push((Text::translate("storage.occultism.linked_to", &[Text::literal(name)]), 0xFF8E6CC0));
        }
        let Some(spirit) = spirit_in(Some(event.stack)) else { return };
        let grey = 0xFFAAAAAA;
        event.lines.push((Text::translate(&format!("tooltip.occultism.spirit.{}", spirit.job.name()), &[]), grey));
        event.lines.push((Text::translate(&format!("tooltip.occultism.spirit.{}", spirit.tier.name()), &[]), grey));
        event.lines.push((Text::translate("tooltip.occultism.spirit.place", &[]), 0xFF8E6CC0));
    }
}
