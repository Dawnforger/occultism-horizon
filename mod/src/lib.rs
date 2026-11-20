// Occultism for Horizon (the arismoko modding fork): a trait mod. The launcher links this package
// into the game and calls the `hooks()` that `#[mod_api::horizon_mod]` makes.
//
// What happens to one block, item or creature is a Part attached to it in `register`: the
// sacrificial bowls and chalks (rites.rs), the storage Actuator, wormholes and Accessor (vault.rs),
// the dimensional mineshaft (mineshaft.rs), spirit fire and Demon's Dream (spirit_fire.rs), the
// working spirits and their books of calling (workers.rs). What isn't one thing's stays a global hook here: the server tick, the
// network, the commands, loot.
//
// The rules themselves (logic.rs, ritual.rs, mining.rs, storage.rs, network.rs, terminal.rs,
// fire.rs) know nothing of the game and are tested on their own (tests/logic).

pub mod fire;
pub mod logic;
pub mod mining;
pub mod network;
pub mod ritual;
pub mod spirit_model;
pub mod spirits;
pub mod storage;
pub mod terminal;

mod crushing;
mod game;
mod mineshaft;
mod paste;
mod paste_items;
mod rites;
mod soul_gem;
mod spirit_fire;
mod spirit_geo;
mod terminal_ui;
mod vault;
mod workers;

// What Horizon's and Sunrise's mod APIs ask differently (tools/make_sunrise.py swaps the file).
#[path = "platform/horizon.rs"]
mod platform;

use game::{say, tell};
use mineshaft::MINESHAFT;
use mod_api::*;
use rites::{BOWL, GOLDEN_BOWL};
use std::sync::Arc;
use vault::{ACCESSOR, ACTUATOR, CORE, WORMHOLE};

/// Everything the mod keeps between hooks, shared with its Parts.
#[derive(Default)]
pub struct State {
    crushing: crushing::Crushing,
    rites: rites::Rites,
    vault: vault::Vault,
    /// The storage terminal on the client.
    ui: terminal_ui::Ui,
    shafts: mineshaft::Mineshafts,
    spirit_fire: spirit_fire::SpiritFire,
    /// The spirits' models on the client, baked on their first frame.
    spirit_models: std::sync::Mutex<std::collections::HashMap<&'static str, spirit_model::Posed>>,
    /// The blazes in the world, by entity id: an afrit's allies, whose blows don't hurt it (the
    /// invulnerability check can't look the attacker up).
    blazes: std::sync::Mutex<std::collections::HashSet<i32>>,
    /// The working spirits.
    workers: workers::Workers,
    /// The jobs of the working spirits this client sees, for their looks, as of its last tick.
    seen_jobs: std::sync::Mutex<std::collections::HashMap<u32, String>>,
}

#[derive(Default)]
struct Occultism {
    s: Arc<State>,
}

impl State {
    /// The recall ritual finished on the golden bowl at `pos`: a new Core for each of the caster's
    /// lost stores, dropped on the bowl.
    fn recall(&self, host: &mut dyn ServerHost, pos: [i32; 3], caster: u128) {
        let cores = self.vault.recall(&*host, caster);
        if cores.is_empty() {
            tell(host, caster, say("storage.occultism.recall_none", &[]));
            return;
        }
        let at = [f64::from(pos[0]) + 0.5, f64::from(pos[1]) + 1.0, f64::from(pos[2]) + 0.5];
        let dimension = host.dimension();
        for (core, name) in cores {
            let dropped = core.to_nbt(&*host).and_then(|item| {
                let mut nbt = Compound::new();
                nbt.insert("Item".into(), Tag::Compound(item));
                nbt.insert("PickupDelay".into(), Tag::Short(10));
                host.summon(dimension, "minecraft:item", at, nbt)
            });
            if dropped.is_none() {
                // No item entity (an unloaded spot): the Core goes straight to the caster.
                let _ = game::give(host, caster, core);
            }
            tell(host, caster, say("storage.occultism.recalled", &[&name]));
        }
    }
}

/// The storage terminal's channel: the Accessor's requests up, the store's listing down.
fn store_channel() -> PayloadId {
    PayloadId(terminal::CHANNEL.into())
}

/// A request from a player's Accessor screen.
struct StoreRequest(Arc<State>);

impl IPayloadHandler for StoreRequest {
    fn handle(&self, bytes: PayloadBytes, context: &mut dyn IPayloadContext) -> Result<(), PayloadHandleError> {
        let failed = |e| PayloadHandleError::Failed(format!("{e:?}"));
        let player = context.player().map_err(failed)?.uuid();
        let host = context.server_host().map_err(failed)?;
        if let Some(request) = terminal::Request::decode(&bytes.0) {
            self.0.vault.remote_request(host, player, request);
        }
        Ok(())
    }
}

/// The store's listing for this client's terminal.
struct StoreListing(Arc<State>);

impl IPayloadHandler for StoreListing {
    fn handle(&self, bytes: PayloadBytes, context: &mut dyn IPayloadContext) -> Result<(), PayloadHandleError> {
        let host = context.client_host().map_err(|e| PayloadHandleError::Failed(format!("{e:?}")))?;
        if self.0.ui.receive(&*host, &bytes.0) {
            host.mod_screen_open(terminal_ui::MOD_ID);
        }
        Ok(())
    }
}

/// Every item the given ids name, for attaching one Part to each.
fn items(r: &mut dyn Registrar, ids: &[&str]) -> Vec<ItemKey> {
    ids.iter().map(|id| r.item_key(id)).collect()
}

#[mod_api::horizon_mod]
impl Hooks for Occultism {
    fn register(&self, r: &mut dyn Registrar, _cx: &RegistrationContext) {
        let s = &self.s;
        // Occultism's own recipe files, read whole through recipes_of_type: each type is its own
        // raw serializer, as the files name it. An unregistered one refuses every recipe.
        for kind in [crushing::RECIPE_TYPE, fire::RECIPE_TYPE, mineshaft::RECIPE_TYPE, rites::RITUAL_TYPE] {
            r.register_recipe_type(kind);
            r.register_raw_recipe_serializer(kind, kind);
        }
        r.payload_handlers().registrar("1").play_bidirectional(
            store_channel(),
            Arc::new(StoreRequest(s.clone())),
            Some(Arc::new(StoreListing(s.clone()))),
        );
        let types: [(&str, &[&str]); 4] = [
            ("sacrificial_bowl", &[BOWL, GOLDEN_BOWL]),
            ("storage_actuator", &[ACTUATOR]),
            ("stable_wormhole", &[WORMHOLE]),
            ("dimensional_mineshaft", &[MINESHAFT]),
        ];
        let mut keys = Vec::new();
        for (name, blocks) in types {
            keys.push(r.block_entity_type(name).blocks(blocks).build());
        }
        let _ = r.block_entity_part(&keys[0], rites::BowlPart(s.clone()));
        let _ = r.block_entity_part(&keys[1], vault::StorePart(s.clone()));
        let _ = r.block_entity_part(&keys[2], vault::StorePart(s.clone()));
        let _ = r.block_entity_part(&keys[3], mineshaft::ShaftPart(s.clone()));
        // Each block's slots and menu, for hoppers and the game's container code.
        let containers: [(&[&str], fn() -> ModContainer); 3] = [
            (&[BOWL, GOLDEN_BOWL], rites::container),
            (&[MINESHAFT], mineshaft::container),
            (&[ACTUATOR, WORMHOLE], vault::container),
        ];
        for (blocks, container) in containers {
            for block in blocks {
                let key = r.block_key(block);
                r.block_container(&key, container());
            }
        }
        r.menu_part(mineshaft::MENU, mineshaft::ShaftMenu);

        let pastes = paste::PasteKind::new(r);
        r.register_crafting_recipe_serializer(paste::KIND, pastes);

        let fire = r.block_key(fire::SPIRIT_FIRE);
        r.block_part(fire, spirit_fire::FirePart(s.clone()));
        let wormhole = r.block_key(WORMHOLE);
        r.block_part(wormhole, vault::GatePlacement);

        for chalk in rites::COLOURS.map(|c| format!("occultism:chalk_{c}")) {
            let key = r.item_key(&chalk);
            r.item_part(key, rites::ChalkPart(s.clone()));
        }
        for key in items(r, &[WORMHOLE, ACCESSOR]) {
            r.item_part(key, vault::GateItem(s.clone()));
        }
        let core = r.item_key(CORE);
        r.item_part(core, vault::CoreItem(s.clone()));
        let seeds = r.item_key(fire::DATURA_SEEDS);
        r.item_part(seeds, spirit_fire::SeedsPart);
        for key in items(r, &soul_gem::GEMS) {
            r.item_part(key, soul_gem::GemPart);
        }
        for key in items(r, &rites::BOUND_BOOKS) {
            r.item_part(key, rites::BookPart(s.clone()));
        }
        for key in items(r, &workers::BOOKS) {
            r.item_part(key, workers::CallingPart(s.clone()));
        }
        for (kind, _) in workers::KINDS {
            let _ = r.entity_part_with_state(kind, workers::WorkerPart(s.clone()));
        }
        platform::register(r, s);
    }

    fn component_types(&self, _context: &RegistrationContext, event: &mut ComponentTypesEvent<'_>) {
        event.types.push(vault::STORE_COMPONENT.to_string());
        event.types.push(vault::LINK_COMPONENT.to_string());
        event.types.push(mineshaft::MINER_COMPONENT.to_string());
        event.types.push(ritual::SPIRIT_COMPONENT.to_string());
        event.types.push(workers::CALLING_COMPONENT.to_string());
    }

    fn server_tick(&self, host: &mut dyn ServerHost, event: &mut ServerTickEvent<'_>) {
        if matches!(event.phase, ServerTickPhase::ServerTickEnd { .. }) {
            self.s.vault.pump(host);
            self.s.spirit_fire.burn_fallen(host);
            soul_gem::settle(host);
            for c in self.s.rites.take_called() {
                self.s.workers.called(host, c.spirit, &c.job, c.caster, c.name.as_deref(), &c.book, c.at);
            }
            let recipes = self.s.crushing.recipes(&*host);
            self.s.workers.tick(host, &recipes);
        }
    }

    fn entity_interact(&self, host: HostSide, event: &mut EntityInteractEvent<'_>) -> Option<InteractionResult> {
        // A soul gem takes a creature before the creature's own use (a villager's trades) runs.
        let stack = event.stack?;
        // A book of calling, sneak-clicked on its spirit, binds to it.
        let spirit = spirits::kind(event.kind) == workers::FOLIOT;
        match host {
            HostSide::Server(host) => spirit
                .then(|| self.s.workers.bind(host, event.hand, event.entity, stack))
                .flatten()
                .or_else(|| soul_gem::capture(host, event.hand, event.entity, event.kind, stack)),
            HostSide::Client(host) => {
                let book = spirit && workers::BOOKS.contains(&game::id(&*host, stack));
                (book || soul_gem::takes_click(&*host, event.hand, event.kind, stack)).then_some(InteractionResult::Success)
            }
        }
    }

    fn item_crafted(&self, _host: &mut dyn ServerHost, event: &mut ItemCraftedEvent<'_>) {
        // A craft in a terminal's grid: its emptied cells refill from the store.
        self.s.vault.crafted(event.player, event.inputs);
    }

    fn player_tick_post(&self, host: HostSide, event: &mut PlayerTickPostEvent) {
        // A player's inventory reads only while their state is acting, as here: what they moved
        // into a store or offered to a bowl in a click.
        if let HostSide::Server(host) = host {
            self.s.vault.take_moves(host, event.player.uuid());
            self.s.rites.take_offers(host, event.player.uuid());
            self.s.workers.hand_over(host, event.player.uuid());
        }
    }

    fn client_tick_post(&self, host: &mut dyn ClientHost, _event: &mut ClientTickPostEvent) {
        *game::lock(&self.s.seen_jobs) = workers::seen_jobs(&*host);
        // The Accessor's requests, from the screen's hooks, which can't send.
        let mut waiting = self.s.ui.outbox().into_iter();
        while let Some(m) = waiting.next() {
            if host.send_payload_to_server(&store_channel(), PayloadBytes(m.clone())).is_err() {
                self.s.ui.requeue(std::iter::once(m).chain(waiting).collect());
                break;
            }
        }
    }

    /// However a spirit comes (a ritual, `/summon`, a spawn egg), it gets its sun-proof head item.
    fn entity_join_level(&self, host: &mut dyn ServerHost, event: &mut EntityJoinLevelEvent<'_>) -> Flow {
        let kind = spirits::kind(event.kind);
        if kind == "minecraft:blaze" {
            game::lock(&self.s.blazes).insert(event.entity as i32);
        }
        if workers::is_worker(kind) {
            self.s.workers.joined(host, event.entity, kind);
        }
        if !event.loaded_from_disk
            && let Some(extra) = spirits::summon_nbt(kind)
            && let Some(entity) = host.entity(event.entity)
        {
            let mut nbt = Compound::new();
            for (k, v) in extra {
                nbt.insert(k.as_str().into(), v);
            }
            entity.merge_nbt(host, nbt);
        }
        Flow::Continue
    }

    /// A working spirit is no wolf: it isn't tamed with a bone.
    fn animal_tame(&self, _host: &mut dyn ServerHost, event: &mut AnimalTameEvent<'_>) -> Flow {
        if workers::is_worker(spirits::kind(event.kind)) { Flow::Cancel } else { Flow::Continue }
    }

    /// Nor does it pant, growl or whine as one.
    fn play_level_sound_at_position(&self, host: HostSide, event: &mut PlayLevelSoundAtPositionEvent<'_>) -> Flow {
        if matches!(host, HostSide::Server(_)) && self.s.workers.mutes(&event.sound.to_string(), event.position) { Flow::Cancel } else { Flow::Continue }
    }

    fn register_layer_definitions(&self, _context: &RegistrationContext, event: &mut RegisterLayerDefinitionsEvent<'_>) {
        event.layers.extend(spirit_model::layers());
    }

    /// A spirit is drawn as Occultism draws it, not as the zombie it is underneath.
    fn render_living_pre(&self, host: &dyn ClientHost, event: &mut RenderLivingPreEvent<'_>) -> Flow {
        let mut kind = spirits::kind(event.entity.kind);
        // A working spirit looks like its job.
        if workers::is_worker(kind) {
            kind = workers::look_of(kind, game::lock(&self.s.seen_jobs).get(&event.entity.id).map(String::as_str));
        }
        let Some((kind, look)) = spirit_model::LOOKS.iter().find(|(k, _)| *k == kind) else { return Flow::Continue };
        let mut models = game::lock(&self.s.spirit_models);
        if !models.contains_key(kind) {
            let Some(posed) = spirit_model::Posed::new(host, look) else { return Flow::Continue };
            models.insert(kind, posed);
        }
        let Some(posed) = models.get_mut(kind) else { return Flow::Continue };
        posed.pose(event.entity);
        // Into the model's frame, as `LivingEntityRenderer` turns into it: y down, front to -z.
        let draw = &mut *event.draw;
        draw.push();
        draw.translate([0.0, 1.501, 0.0]);
        draw.scale([1.0, -1.0, -1.0]);
        draw.model_part(&posed.root, look.sprite, 0xFFFFFFFF);
        draw.pop();
        Flow::Cancel
    }

    /// A spirit takes no damage of the kinds Occultism's doesn't (`isInvulnerableTo`).
    fn entity_invulnerability_check(&self, _level: LevelSide, event: &mut EntityInvulnerabilityCheckEvent<'_>) {
        if !spirits::is_spirit(event.kind) {
            return;
        }
        let ally = game::lock(&self.s.blazes).contains(&event.source.causing);
        let causing = ally.then_some("minecraft:blaze");
        if spirits::immune(event.kind, event.source.damage_type, causing) {
            event.invulnerable = true;
        }
    }

    fn living_conversion_pre(&self, _host: &mut dyn ServerHost, event: &mut LivingConversionPreEvent<'_>) -> Flow {
        // A spirit is a zombie underneath, which would drown into a drowned.
        if spirits::is_spirit(event.kind) { Flow::Cancel } else { Flow::Continue }
    }

    /// A death may be a running ritual's sacrifice.
    fn living_death(&self, host: &mut dyn ServerHost, event: &mut LivingDeathEvent<'_>) -> Flow {
        game::lock(&self.s.blazes).remove(&(event.entity as i32));
        let Some(entity) = host.entity(event.entity) else { return Flow::Continue };
        let (Some(at), Some(level)) = (entity.block_position(&*host), entity.level(&*host)) else { return Flow::Continue };
        let causing = u32::try_from(event.source.causing).ok().filter(|c| *c != 0).and_then(|c| host.entity(c));
        let causing = causing.and_then(|c| c.kind(&*host)).map(|k| k.id(&*host));
        let by_player = ritual::killed_by_player(causing.as_deref(), event.source.damage_type);
        let dimension = level.dimension_id().to_string();
        self.s.rites.death(&*host, &dimension, at, spirits::kind(event.kind), by_player);
        Flow::Continue
    }

    /// A ritual nearby may wait for this item's use (the marid's trident); the use goes on.
    fn right_click_item(&self, host: HostSide, event: &mut RightClickItemEvent<'_>) -> Flow {
        if let HostSide::Server(host) = host {
            let click = event.click;
            let item = game::id(&*host, click.stack);
            self.s.rites.used_item(&*host, click.dimension.key().id(), click.pos, item);
        }
        Flow::Continue
    }

    fn prepare_hopper_transfer(&self, host: &mut dyn ServerHost, event: &mut PrepareHopperTransferEvent) {
        self.s.vault.hopper_try(&*host, event.pos);
    }

    /// A book of binding, or what a ritual bound to its spirit, names the spirit.
    fn item_tooltip(&self, host: &dyn ClientHost, event: &mut ItemTooltipEvent<'_>) {
        if let Some(name) = rites::spirit_name(event.stack) {
            let key = format!("item.{}.tooltip", game::id(host, event.stack).replace(':', "."));
            event.lines.push((game::say(&key, &[&name]), 0xFFFFAA00));
        }
    }

    fn player_container_open(&self, host: &mut dyn ServerHost, event: &mut PlayerContainerOpenEvent<'_>) {
        if crate::platform::opened_menu(event) == vault::MENU
            && let Some(pos) = event.pos
        {
            self.s.vault.watch(event.player, event.dimension, pos, &*host);
        }
    }

    fn player_container_close(&self, _host: &mut dyn ServerHost, event: &mut PlayerContainerCloseEvent<'_>) {
        if crate::platform::closed_menu(event) == vault::MENU {
            self.s.vault.unwatch(event.player);
        }
    }

    fn player_logged_out(&self, _host: &mut dyn ServerHost, event: &mut PlayerLoggedOutEvent<'_>) {
        self.s.vault.unwatch(event.player);
    }

    fn right_click_block(&self, host: HostSide, event: &mut RightClickBlockEvent<'_>) -> Flow {
        let click = event.click;
        // A bowl takes what is offered to it: the item's own use (armour put on, food eaten) never
        // runs, on the client as on the server, or the client would do it before the bowl answers.
        let bowl = |h: &dyn Host| {
            let level = h.level(click.dimension.key().id()).or_else(|| h.level(h.dimension()));
            let block = game::block_name(h, level.and_then(|l| l.block(h, click.pos)));
            block == BOWL || block == GOLDEN_BOWL
        };
        let on_bowl = match &host {
            HostSide::Server(h) => bowl(&**h),
            HostSide::Client(h) => bowl(&**h),
        };
        if on_bowl {
            *event.use_item = TriState::False;
        }
        let HostSide::Server(host) = host else { return Flow::Continue };
        // Flint and steel over dropped Demon's Dream lights spirit fire, not fire.
        if click.stack.is_some_and(|s| fire::IGNITERS.contains(&game::id(&*host, s)))
            && self.s.spirit_fire.light(host, click.dimension.key().id(), click.pos, click.hit.face, click.player, click.hand)
        {
            return Flow::Cancel;
        }
        // A wormhole opens only while its store is awake; a store's blocks only for those it trusts.
        let Some(level) = host.level(click.dimension.key().id()) else { return Flow::Continue };
        let block = game::block_name(&*host, level.block(&*host, click.pos));
        let sneaking = host.player(click.player).and_then(|p| p.entity().is_shift_key_down(&*host)) == Some(true);
        if sneaking && click.stack.is_some() || !(block == ACTUATOR || block == WORMHOLE) {
            return Flow::Continue;
        }
        let dimension = click.dimension.key().id();
        let refusal = if block == WORMHOLE {
            self.s.vault.gate_open(&*host, dimension, click.pos).map_err(|name| ("storage.occultism.gate_dormant", name))
        } else {
            Ok(())
        };
        let refusal = refusal.and_then(|()| {
            self.s.vault.may_open(&*host, dimension, click.pos, click.player).map_err(|name| ("storage.occultism.refused", name))
        });
        match refusal {
            Ok(()) => Flow::Continue,
            Err((key, name)) => {
                if click.hand == Hand::Main {
                    tell(host, click.player, say(key, &[&name]));
                }
                *event.cancellation_result = InteractionResult::Fail;
                Flow::Cancel
            }
        }
    }

    fn break_block(&self, host: &mut dyn ServerHost, event: &mut BreakBlockEvent<'_>) -> Flow {
        let Some(player) = host.players().acting_player() else { return Flow::Continue };
        let dimension = host.dimension();
        let core = host.level(dimension).is_some_and(|l| game::block_name(&*host, l.block(&*host, event.pos)) == CORE);
        if !core {
            return Flow::Continue;
        }
        // Only those a store trusts (and operators) break its Core.
        let operator = player.permission_level(&*host).unwrap_or(0);
        if let Err(name) = self.s.vault.may_break(&*host, dimension, event.pos, player.uuid(), operator) {
            tell(host, player.uuid(), say("storage.occultism.refused", &[&name]));
            return Flow::Cancel;
        }
        // A creative break drops nothing, so the Core comes to the breaker's inventory carrying its
        // store (as vanilla's pick-block with its data would); `loot_drops` does it otherwise.
        if player.is_creative(&*host) == Some(true)
            && let Some((core, name)) = self.s.vault.unanchor(&*host, dimension, event.pos)
        {
            let _ = game::give(host, player.uuid(), core);
            tell(host, player.uuid(), say("storage.occultism.dormant", &[&name]));
        }
        Flow::Continue
    }

    fn entity_place(&self, host: &mut dyn ServerHost, event: &mut EntityPlaceEvent) -> Flow {
        // A Core or wormhole takes its store or link from the item that placed it, which its use
        // left for this moment (the event has no stack: upstream/fork-gaps.md).
        let Some(placer) = event.placer else { return Flow::Continue };
        let Some(uuid) = host.players().server_players().into_iter().find(|p| p.entity() == placer).map(PlayerRef::uuid) else {
            return Flow::Continue;
        };
        let block = game::block_name(&*host, Some(event.state.block()));
        let pos = [event.pos.x, event.pos.y, event.pos.z];
        self.s.vault.placed_by(&*host, host.dimension(), pos, block, uuid);
        Flow::Continue
    }

    fn world_lifecycle(&self, host: &mut dyn ServerHost, event: &mut WorldLifecycleEvent<'_>) {
        match event.phase {
            WorldPhase::WorldCloseFinal => self.s.vault.close(&*host, true),
            WorldPhase::WorldCloseFailed => self.s.vault.close(&*host, false),
            _ => {}
        }
    }

    fn world_saved(&self, host: &mut dyn ServerHost, _event: &mut WorldSavedEvent) {
        self.s.vault.saved(&*host);
    }

    fn register_commands(&self, _host: &mut dyn ServerHost, event: &mut RegisterCommandsEvent<'_>) {
        event.commands.push(Command::new("occultism trust <player:player>"));
        event.commands.push(Command::new("occultism untrust <player:player>"));
        event.commands.push(Command::new("occultism trusted"));
    }

    fn run_mod_command(&self, host: &mut dyn ServerHost, event: &mut RunModCommandEvent<'_>) -> Flow {
        let words: Vec<&str> = event.command.literals.iter().map(String::as_str).collect();
        if words.first() != Some(&"occultism") {
            return Flow::Continue;
        }
        let CommandSource::Player { player: owner, .. } = event.source else { return Flow::Cancel };
        let owner = owner.uuid();
        let line = match words.as_slice() {
            ["occultism", verb @ ("trust" | "untrust")] => {
                let Some(CommandArgumentValue::Player(player)) = event.command.argument("player") else { return Flow::Cancel };
                let name = player.name(&*host).unwrap_or_default();
                match self.s.vault.trust(&*host, owner, player.uuid(), *verb == "trust") {
                    Ok(store) => say(&format!("storage.occultism.{verb}ed"), &[&name, &store]),
                    Err(key) => say(key, &[]),
                }
            }
            ["occultism", "trusted"] => match self.s.vault.trusted(&*host, owner) {
                None => say("storage.occultism.trust.none", &[]),
                Some((store, list)) => {
                    let names: Vec<String> = list
                        .iter()
                        .map(|u| host.player(*u).and_then(|p| p.name(&*host)).unwrap_or_else(|| format!("{u:032x}")))
                        .collect();
                    let names = if names.is_empty() { "-".to_string() } else { names.join(", ") };
                    say("storage.occultism.trust.list", &[&store, &names])
                }
            },
            _ => return Flow::Continue,
        };
        let _ = host.commands().feedback(line);
        Flow::Cancel
    }

    fn loot_drops(&self, host: &mut dyn ServerHost, event: &mut LootDropsEvent<'_>) {
        let context = event.context;
        let sight = context.player.map(|p| rites::sight(&*host, p)).unwrap_or_default();
        self.s.rites.silver_ore_drops(&*host, context, sight, event.drops);
        self.s.rites.oak_log_drops(&*host, context, sight, event.drops);
        self.s.spirit_fire.drops(&*host, context, event.drops);
        let Some((pos, _)) = context.block else { return };
        // A Core leaving the world carries its store; the store sleeps until it's placed again.
        if let Some(core) = event.drops.iter_mut().find(|s| game::id(&*host, s) == CORE)
            && let Some((carrying, name)) = self.s.vault.unanchor(&*host, context.dimension, pos)
        {
            *core = carrying;
            if let Some(player) = context.player {
                tell(host, player, say("storage.occultism.dormant", &[&name]));
            }
        }
        // A wormhole leaving the world keeps its link.
        if let Some(gate) = event.drops.iter_mut().find(|s| game::id(&*host, s) == WORMHOLE)
            && let Some((id, name)) = self.s.vault.gate_removed(&*host, context.dimension, pos)
        {
            vault::set_link(gate, &id, &name);
        }
    }

    fn screen_mouse_scrolled_pre(&self, _host: &dyn ClientHost, event: &mut ScreenMouseScrolledPreEvent<'_>) -> Flow {
        self.s.ui.scroll(event.screen, event.scroll[1])
    }

    fn screen_key_pressed_pre(&self, _host: &dyn ClientHost, event: &mut ScreenKeyPressedPreEvent<'_>) -> Flow {
        self.s.ui.key(event.screen, event.key)
    }

    fn screen_char_typed_pre(&self, _host: &dyn ClientHost, event: &mut ScreenCharTypedPreEvent<'_>) -> Flow {
        self.s.ui.typed(event.screen, event.c)
    }

    fn screen_closing(&self, _host: &dyn ClientHost, event: &mut ScreenClosingEvent<'_>) {
        self.s.ui.closed(event.screen);
    }

    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        self.s.ui.draw_remote(host, &mut *event.draw, event.screen);
    }

    fn screen_mouse_button_pressed_pre(&self, _host: &dyn ClientHost, event: &mut ScreenMouseButtonPressedPreEvent<'_>) -> Flow {
        self.s.ui.remote_press(event.screen, event.button)
    }
}
