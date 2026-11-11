// Tests of the altar's rules (native/engine/src/logic.rs), compiled by path so they run without
// Horizon. Expected values come from Occultism's own numbers (its config defaults and recipe
// files), worked out by hand, not from the code under test.
#[path = "../../native/engine/src/logic.rs"]
pub mod logic;

#[cfg(test)]
mod tests {
    use super::logic::*;

    fn stack(item: &str, count: u32) -> Option<Stack> {
        Some(Stack::plain(item, count))
    }

    fn sixty_four(_: &str) -> u32 {
        64
    }

    /// occultism:crushing/iron_dust_from_raw: raw iron -> 2 iron dust, default time.
    fn raw_iron() -> Recipe {
        Recipe {
            id: "occultism:crushing/iron_dust_from_raw".into(),
            inputs: vec!["minecraft:raw_iron".into()],
            result: "occultism:iron_dust".into(),
            count: 2,
            time: DEFAULT_CRUSHING_TIME,
            min_tier: -1,
            max_tier: -1,
            ignore_multiplier: false,
        }
    }

    /// occultism:crushing/iron_dust_from_ingot: ingot -> 1 dust, multiplier ignored.
    fn iron_ingot() -> Recipe {
        Recipe {
            id: "occultism:crushing/iron_dust_from_ingot".into(),
            inputs: vec!["minecraft:iron_ingot".into()],
            result: "occultism:iron_dust".into(),
            count: 1,
            time: DEFAULT_CRUSHING_TIME,
            min_tier: -1,
            max_tier: -1,
            ignore_multiplier: true,
        }
    }

    fn altar(core: Option<&str>, input: Option<Stack>) -> Vec<Option<Stack>> {
        let mut slots = vec![None; SLOT_COUNT];
        slots[SLOT_CORE] = core.map(|c| Stack::plain(c, 1));
        slots[SLOT_INPUT] = input;
        slots
    }

    fn spirit(slots: &[Option<Stack>]) -> Option<Spirit> {
        slots[SLOT_CORE].as_ref().and_then(|s| spirit_of(&s.item, None))
    }

    /// Runs ticks until one finishes an operation; returns how many it took.
    fn ticks_to_finish(slots: &mut [Option<Stack>], state: &mut AltarState, recipes: &[Recipe]) -> u32 {
        for n in 1..=10_000 {
            let s = spirit(slots);
            if let Tick::Done { .. } = tick(state, slots, recipes, &sixty_four, s) {
                return n;
            }
        }
        panic!("never finished");
    }

    // Occultism's crusher: Foliot time x2.0 out x1.0, Djinni x1.0/x1.5, Afrit x0.5/x2.0,
    // Marid x0.3/x3.0, on raw iron's 200 ticks and 2 dust.
    #[test]
    fn each_tier_takes_occultisms_time_and_gives_its_yield() {
        // (core, ticks, dust): 200*2.0=400; 200*1.0=200; 200*0.5=100; 200*0.3f is 60.000004 in
        // Java's float math, so the counter has to reach 61 before it passes.
        let expected = [
            ("occultism:crusher_core_foliot", 400, 2),
            ("occultism:crusher_core_djinni", 200, 3),
            ("occultism:crusher_core_afrit", 100, 4),
            ("occultism:crusher_core_marid", 61, 6),
        ];
        for (core, ticks, dust) in expected {
            let mut slots = altar(Some(core), stack("minecraft:raw_iron", 5));
            let mut state = AltarState::default();
            assert_eq!(ticks_to_finish(&mut slots, &mut state, &[raw_iron()]), ticks, "{core}");
            assert_eq!(slots[SLOT_INPUT], stack("minecraft:raw_iron", 4), "{core}");
            assert_eq!(slots[2], stack("occultism:iron_dust", dust), "{core}");
        }
    }

    #[test]
    fn ingot_recipes_ignore_the_output_multiplier() {
        let mut slots = altar(Some("occultism:crusher_core_marid"), stack("minecraft:iron_ingot", 1));
        let mut state = AltarState::default();
        ticks_to_finish(&mut slots, &mut state, &[iron_ingot()]);
        assert_eq!(slots[2], stack("occultism:iron_dust", 1));
        assert_eq!(slots[SLOT_INPUT], None, "the last ingot is used up");
    }

    #[test]
    fn an_empty_core_slot_does_nothing_and_forgets_progress() {
        let mut slots = altar(Some("occultism:crusher_core_djinni"), stack("minecraft:raw_iron", 3));
        let mut state = AltarState::default();
        for _ in 0..50 {
            let s = spirit(&slots);
            tick(&mut state, &mut slots, &[raw_iron()], &sixty_four, s);
        }
        assert_eq!(state.progress, 50);
        slots[SLOT_CORE] = None;
        let r = tick(&mut state, &mut slots, &[raw_iron()], &sixty_four, None);
        assert_eq!(r, Tick::NoCore);
        assert_eq!(state, AltarState::default());
        assert_eq!(slots[SLOT_INPUT], stack("minecraft:raw_iron", 3), "nothing consumed");
    }

    #[test]
    fn a_non_core_item_in_the_core_slot_is_no_spirit() {
        let slots = altar(Some("minecraft:diamond"), stack("minecraft:raw_iron", 3));
        assert_eq!(spirit(&slots), None);
    }

    #[test]
    fn the_spirit_component_decides_over_the_item_id() {
        // A Foliot-looking gem whose component binds a Marid is a Marid: the data is the spirit.
        let c = SpiritComponent {
            job: Some("crusher".into()),
            tier: Some(4),
        };
        let s = spirit_of("occultism:crusher_core_foliot", Some(&c)).unwrap();
        assert_eq!(s.tier, Tier::Marid);
        // Any item can carry a spirit.
        assert!(spirit_of("occultism:soul_gem", Some(&c)).is_some());
        // An unknown job or rank falls back to the item's own spirit.
        let bad = SpiritComponent {
            job: Some("baker".into()),
            tier: Some(4),
        };
        assert_eq!(spirit_of("occultism:crusher_core_afrit", Some(&bad)).unwrap().tier, Tier::Afrit);
        assert_eq!(spirit_of("occultism:soul_gem", Some(&bad)), None);
    }

    #[test]
    fn input_without_a_recipe_waits() {
        let mut slots = altar(Some("occultism:crusher_core_marid"), stack("minecraft:dirt", 10));
        let mut state = AltarState::default();
        let s = spirit(&slots);
        assert_eq!(tick(&mut state, &mut slots, &[raw_iron()], &sixty_four, s), Tick::NoRecipe);
        assert_eq!(slots[SLOT_INPUT], stack("minecraft:dirt", 10));
    }

    #[test]
    fn tier_bounds_filter_recipes_as_crushing_recipe_matches() {
        let mut r = raw_iron();
        r.min_tier = 3;
        assert!(!r.matches("minecraft:raw_iron", Tier::Djinni));
        assert!(r.matches("minecraft:raw_iron", Tier::Afrit));
        r.max_tier = 3;
        assert!(!r.matches("minecraft:raw_iron", Tier::Marid));
        r.min_tier = -1;
        assert!(r.matches("minecraft:raw_iron", Tier::Foliot));
    }

    #[test]
    fn swapping_the_input_restarts_the_count() {
        let recipes = [raw_iron(), iron_ingot()];
        let mut slots = altar(Some("occultism:crusher_core_djinni"), stack("minecraft:raw_iron", 2));
        let mut state = AltarState::default();
        for _ in 0..150 {
            let s = spirit(&slots);
            tick(&mut state, &mut slots, &recipes, &sixty_four, s);
        }
        slots[SLOT_INPUT] = stack("minecraft:iron_ingot", 1);
        let s = spirit(&slots);
        assert_eq!(
            tick(&mut state, &mut slots, &recipes, &sixty_four, s),
            Tick::Working { progress: 1, total: 200 }
        );
    }

    #[test]
    fn a_full_buffer_holds_the_finished_operation_until_there_is_room() {
        let mut slots = altar(Some("occultism:crusher_core_marid"), stack("minecraft:raw_iron", 2));
        for slot in SLOTS_OUTPUT {
            slots[slot] = stack("minecraft:cobblestone", 64);
        }
        let mut state = AltarState::default();
        let mut last = Tick::NoCore;
        for _ in 0..200 {
            let s = spirit(&slots);
            last = tick(&mut state, &mut slots, &[raw_iron()], &sixty_four, s);
        }
        assert_eq!(last, Tick::OutputFull { total: 61 });
        assert_eq!(slots[SLOT_INPUT], stack("minecraft:raw_iron", 2), "nothing consumed while blocked");
        // Room for the whole result frees it on the next tick, with no second wait.
        slots[5] = None;
        let s = spirit(&slots);
        let done = tick(&mut state, &mut slots, &[raw_iron()], &sixty_four, s);
        assert_eq!(
            done,
            Tick::Done { consumed: 1, result: "occultism:iron_dust".into(), produced: 6 }
        );
        assert_eq!(slots[5], stack("occultism:iron_dust", 6));
    }

    #[test]
    fn results_top_up_matching_stacks_before_empty_slots_and_never_split_partially() {
        let mut slots = vec![None; SLOT_COUNT];
        slots[3] = stack("occultism:iron_dust", 62);
        slots[4] = Some(Stack { item: "occultism:iron_dust".into(), count: 1, plain: false });
        assert!(insert(&mut slots, "occultism:iron_dust", 6, &sixty_four, false));
        assert_eq!(slots[3], stack("occultism:iron_dust", 64), "topped up first");
        assert_eq!(slots[2], stack("occultism:iron_dust", 4), "rest in the first empty slot");
        assert_eq!(slots[4].as_ref().unwrap().count, 1, "a stack with its own components is left alone");

        let mut full = vec![None; SLOT_COUNT];
        for slot in SLOTS_OUTPUT {
            full[slot] = stack("occultism:iron_dust", 63);
        }
        let before = full.clone();
        assert!(!insert(&mut full, "occultism:iron_dust", 6, &sixty_four, false));
        assert_eq!(full, before, "all or nothing");
    }

    #[test]
    fn a_stack_with_components_is_not_crushed() {
        let mut slots = altar(Some("occultism:crusher_core_marid"), None);
        slots[SLOT_INPUT] = Some(Stack { item: "minecraft:raw_iron".into(), count: 3, plain: false });
        let mut state = AltarState::default();
        let s = spirit(&slots);
        assert_eq!(tick(&mut state, &mut slots, &[raw_iron()], &sixty_four, s), Tick::NoRecipe);
    }

    #[test]
    fn a_stack_runs_down_to_empty_over_repeated_operations() {
        let mut slots = altar(Some("occultism:crusher_core_afrit"), stack("minecraft:raw_iron", 3));
        let mut state = AltarState::default();
        let mut done = 0;
        for _ in 0..1000 {
            let s = spirit(&slots);
            if let Tick::Done { .. } = tick(&mut state, &mut slots, &[raw_iron()], &sixty_four, s) {
                done += 1;
            }
        }
        assert_eq!(done, 3);
        assert_eq!(slots[SLOT_INPUT], None);
        assert_eq!(slots[2], stack("occultism:iron_dust", 12));
        let s = spirit(&slots);
        assert_eq!(tick(&mut state, &mut slots, &[raw_iron()], &sixty_four, s), Tick::NoInput);
    }

    #[test]
    fn hoppers_fill_the_input_from_above_and_sides_and_drain_the_output_from_below() {
        let faces = face_slots();
        assert_eq!(faces[0], SLOTS_OUTPUT.to_vec(), "down");
        for face in &faces[1..] {
            assert_eq!(face, &vec![SLOT_INPUT]);
            assert!(!face.contains(&SLOT_CORE), "no face reaches the core");
        }
    }

    #[test]
    fn players_put_cores_only_in_the_core_slot_and_nothing_in_the_output() {
        assert_eq!(slot_rule(SLOT_CORE, "occultism:crusher_core_afrit", None), (true, 1));
        assert_eq!(slot_rule(SLOT_CORE, "minecraft:raw_iron", None).0, false);
        assert!(slot_rule(SLOT_INPUT, "minecraft:raw_iron", None).0);
        assert!(!slot_rule(SLOT_INPUT, "occultism:crusher_core_djinni", None).0);
        for slot in SLOTS_OUTPUT {
            assert!(!slot_rule(slot, "minecraft:raw_iron", None).0);
        }
        assert_eq!(quick_move_target("occultism:crusher_core_afrit", None, true), Some(SLOT_CORE));
        assert_eq!(quick_move_target("occultism:crusher_core_afrit", None, false), None);
        assert_eq!(quick_move_target("minecraft:raw_iron", None, true), Some(SLOT_INPUT));
    }

    #[test]
    fn the_screen_sees_progress_total_rank_and_state() {
        let mut slots = altar(Some("occultism:crusher_core_afrit"), stack("minecraft:raw_iron", 1));
        let mut state = AltarState::default();
        let s = spirit(&slots);
        let t = tick(&mut state, &mut slots, &[raw_iron()], &sixty_four, s);
        assert_eq!(data_slots(&t, &state, s), [1, 100, 3, 1]);
    }
    #[test]
    fn a_hopper_fed_core_binds_or_passes_through_instead_of_jamming_the_input() {
        let core = || stack("occultism:crusher_core_foliot", 1);
        let mut slots = vec![None, core(), None, None, None, None];
        assert_eq!(stray_core_target(&slots, true), Some(SLOT_CORE));
        slots[SLOT_CORE] = stack("occultism:crusher_core_marid", 1);
        slots[2] = stack("occultism:iron_dust", 64);
        assert_eq!(stray_core_target(&slots, true), Some(3));
        for slot in SLOTS_OUTPUT {
            slots[slot] = stack("occultism:iron_dust", 64);
        }
        assert_eq!(stray_core_target(&slots, true), None);
        slots[SLOT_INPUT] = stack("minecraft:raw_iron", 4);
        slots[4] = None;
        assert_eq!(stray_core_target(&slots, false), None);
    }
}

// Tests of the ritual rules (native/engine/src/ritual.rs). Expected values come from Occultism's
// recipes and pentacles (durations, ingredient counts, the summon_foliot circle), worked by hand.
#[path = "../../native/engine/src/ritual.rs"]
pub mod ritual;

#[cfg(test)]
mod ritual_tests {
    use super::ritual::*;
    use std::collections::HashMap;

    fn ids(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    /// Occultism's summon_foliot pentacle, its glyph layer and the display-only floor below.
    fn summon_foliot() -> Pentacle {
        let pattern = vec![
            ids(&[
                "_________", "___WWW___", "__W_W_W__", "_W_1W1_W_", "_WWW0WWW_",
                "_W_1W1_W_", "__W_W_W__", "___WWW___", "_________",
            ]),
            ids(&["*+*+*+*+*"; 9]),
        ];
        let keys = vec![
            ('_', Accept::Any),
            ('*', Accept::Any),
            ('+', Accept::Any),
            ('0', Accept::OneOf(ids(&["occultism:golden_sacrificial_bowl"]))),
            ('1', Accept::OneOf(ids(&["minecraft:candle", "minecraft:red_candle"]))),
            ('W', Accept::OneOf(ids(&["occultism:chalk_glyph_white", "occultism:chalk_glyph_gray"]))),
        ];
        Pentacle::new(&pattern, &keys).unwrap()
    }

    /// A world holding `summon_foliot` drawn around a bowl at the origin, turned `rotation` times.
    fn world_with_circle(rotation: u8) -> HashMap<[i32; 3], String> {
        let rows = [
            "_________", "___WWW___", "__W_W_W__", "_W_1W1_W_", "_WWW0WWW_",
            "_W_1W1_W_", "__W_W_W__", "___WWW___", "_________",
        ];
        let mut world = HashMap::new();
        for (z, row) in rows.iter().enumerate() {
            for (x, ch) in row.chars().enumerate() {
                let block = match ch {
                    '0' => "occultism:golden_sacrificial_bowl",
                    '1' => "minecraft:candle",
                    'W' => "occultism:chalk_glyph_white",
                    _ => continue,
                };
                world.insert(rotate([x as i32 - 4, 0, z as i32 - 4], rotation), block.to_string());
            }
        }
        world
    }

    fn reader(world: &HashMap<[i32; 3], String>) -> impl Fn([i32; 3]) -> String + '_ {
        move |p| world.get(&p).cloned().unwrap_or_else(|| "minecraft:stone".into())
    }

    #[test]
    fn a_complete_circle_matches_in_every_rotation() {
        let p = summon_foliot();
        for r in 0..4 {
            assert!(p.matches(&reader(&world_with_circle(r))), "rotation {r}");
        }
        // The outermost glyphs are 3 from the bowl; the floor layer is display-only, so nothing below.
        assert_eq!((p.reach, p.below, p.above), (3, 0, 0));
    }

    #[test]
    fn a_missing_glyph_or_candle_breaks_the_circle_and_is_named() {
        let p = summon_foliot();
        let mut world = world_with_circle(0);
        world.remove(&[0, 0, -3]); // the top glyph of the circle
        world.insert([-1, 0, -1], "minecraft:torch".into()); // a candle's place
        assert!(!p.matches(&reader(&world)));
        let missing: Vec<[i32; 3]> = p.closest(&reader(&world)).into_iter().map(|(at, _)| at).collect();
        assert_eq!(missing.len(), 2);
        assert!(missing.contains(&[0, 0, -3]) && missing.contains(&[-1, 0, -1]));
    }

    #[test]
    fn any_glyph_in_the_key_counts() {
        let p = summon_foliot();
        let mut world = world_with_circle(1);
        world.insert(rotate([-3, 0, 0], 1), "occultism:chalk_glyph_gray".into());
        assert!(p.matches(&reader(&world)));
        world.insert(rotate([-3, 0, 0], 1), "occultism:chalk_glyph_red".into());
        assert!(!p.matches(&reader(&world)));
    }

    fn foliot_crusher() -> Ritual {
        Ritual {
            id: "occultism:ritual/summon_foliot_crusher".into(),
            activation: ids(&["occultism:book_of_binding_bound_foliot"]),
            duration: 40,
            ingredients: vec![
                ids(&["minecraft:iron_ingot"]),
                ids(&["minecraft:gold_ingot"]),
                ids(&["minecraft:copper_ingot"]),
                ids(&["occultism:silver_ingot"]),
            ],
            pentacle: "occultism:summon_foliot".into(),
            result: "occultism:crusher_core_foliot".into(),
            count: 1,
            effect: None,
        }
    }

    #[test]
    fn ingredients_are_taken_evenly_and_the_ritual_ends_on_time() {
        // 40 s over 4 ingredients + 1: one every 8 s, at 8, 16, 24 and 32 s; done at 40 s.
        let r = foliot_crusher();
        assert_eq!(r.seconds_per_ingredient(), 8.0);
        let mut state = RitualState { ritual: Some(r.id.clone()), ..Default::default() };
        let mut takes = Vec::new();
        let mut done_at = None;
        for _ in 0..(45 * TICKS_PER_SECOND) {
            match advance(&mut state, &r) {
                Step::Take(i) => {
                    takes.push((i, state.ticks));
                    taken(&mut state);
                }
                Step::Done => {
                    done_at = Some(state.ticks);
                    break;
                }
                Step::Wait => {}
            }
        }
        assert_eq!(takes, vec![(0, 160), (1, 320), (2, 480), (3, 640)]);
        assert_eq!(done_at, Some(800));
    }

    #[test]
    fn a_ritual_waits_while_its_ingredient_cannot_be_taken() {
        let r = foliot_crusher();
        let mut state = RitualState::default();
        for _ in 0..160 {
            advance(&mut state, &r);
        }
        // Not taken: the next tick asks again, and the ritual never finishes without it.
        assert_eq!(advance(&mut state, &r), Step::Take(0));
        state.ticks = 2000;
        assert_eq!(advance(&mut state, &r), Step::Take(0));
    }

    #[test]
    fn each_bowl_counts_for_one_ingredient() {
        let r = foliot_crusher();
        let full = ids(&["minecraft:iron_ingot", "minecraft:gold_ingot", "minecraft:copper_ingot", "occultism:silver_ingot"]);
        assert!(ingredients_present(&r.ingredients, &full));
        let mut extra = full.clone();
        extra.push("minecraft:dirt".into());
        assert!(ingredients_present(&r.ingredients, &extra));
        let two_iron = vec![
            ids(&["minecraft:iron_ingot"]),
            ids(&["minecraft:iron_ingot"]),
        ];
        assert!(!ingredients_present(&two_iron, &ids(&["minecraft:iron_ingot"])));
        assert!(ingredients_present(&two_iron, &ids(&["minecraft:iron_ingot", "minecraft:iron_ingot"])));
        let bowls = vec![None, Some("minecraft:gold_ingot".to_string()), Some("minecraft:iron_ingot".to_string())];
        assert_eq!(bowl_for(&r.ingredients[0], &bowls), Some(2));
        assert_eq!(bowl_for(&r.ingredients[2], &bowls), None);
    }

    #[test]
    fn glyphs_get_a_drawn_sign_and_cycle_when_drawn_over() {
        assert_eq!(glyph_sign(0, None), 1); // the preview sign 0 is never drawn
        assert_eq!(glyph_sign(7, None), 7);
        assert_eq!(glyph_sign(14 + 15, None), 14);
        assert_eq!(glyph_sign(3, Some(5)), 6);
        assert_eq!(glyph_sign(3, Some(14)), 1); // wraps past 14, skipping 0
    }

    #[test]
    fn a_glyph_faces_the_one_drawing_it() {
        // yaw 0 looks south, 90 west, 180 north, -90 east.
        assert_eq!(glyph_facing(0.0), "north");
        assert_eq!(glyph_facing(90.0), "east");
        assert_eq!(glyph_facing(180.0), "south");
        assert_eq!(glyph_facing(-90.0), "west");
        assert_eq!(glyph_facing(44.0), "north");
        assert_eq!(glyph_facing(46.0), "east");
    }

    #[test]
    fn iesnium_shows_in_silver_ore_only_to_the_third_eye_and_more_with_goggles() {
        assert_eq!(iesnium_chance(false, false), 0.0);
        assert_eq!(iesnium_chance(true, false), 0.20);
        assert_eq!(iesnium_chance(false, true), 0.40);
        assert_eq!(iesnium_chance(true, true), 0.40);
    }
}

// Tests of the storage rules (native/engine/src/storage.rs). The scale tests pin the design's rule:
// the cost of an operation depends on what changed, never on how much is stored.
#[path = "../../native/engine/src/storage.rs"]
pub mod storage;

#[cfg(test)]
mod storage_tests {
    use super::storage::*;
    use std::time::{Duration, Instant};

    const ROOMY: Limits = Limits { types: u32::MAX, total: u64::MAX };

    /// A worn sword as a store sees it: its own bytes, like a real tool's encoded components.
    fn sword(n: u32) -> Vec<u8> {
        let mut key = b"minecraft:diamond_sword{damage:".to_vec();
        key.extend_from_slice(&n.to_le_bytes());
        key.extend_from_slice(&[7u8; 96]);
        key
    }

    #[test]
    fn capacity_is_occultisms_base_plus_each_plate() {
        assert_eq!(Limits::with_plates(&[]), Limits { types: 128, total: 256_000 });
        assert_eq!(Limits::with_plates(&[1]), Limits { types: 192, total: 768_000 });
        assert_eq!(Limits::with_plates(&[1, 4, 4]), Limits { types: 128 + 64 + 1024, total: 256_000 + 512_000 + 8_192_000 });
        assert_eq!(Limits::with_plates(&[9, 0]), Limits::with_plates(&[]));
    }

    #[test]
    fn exact_items_stay_apart_and_counts_add_up() {
        let mut s = Store::new(ROOMY);
        assert_eq!(s.insert("minecraft:diamond_sword", &sword(1), 1), 1);
        assert_eq!(s.insert("minecraft:diamond_sword", &sword(2), 1), 1);
        assert_eq!(s.insert("minecraft:cobblestone", b"cobble", 64), 64);
        assert_eq!(s.insert("minecraft:cobblestone", b"cobble", 36), 36);
        assert_eq!((s.types(), s.total()), (3, 102));
        assert_eq!(s.count_of(b"cobble"), 100);
        assert_eq!(s.count_item("minecraft:diamond_sword"), 2);
        assert_eq!(s.extract(&sword(1), 5), 1);
        assert_eq!(s.count_of(&sword(1)), 0);
        assert_eq!(s.count_item("minecraft:diamond_sword"), 1);
        assert_eq!(s.extract(b"nothing", 5), 0);
    }

    #[test]
    fn any_variant_of_an_item_can_be_taken() {
        let mut s = Store::new(ROOMY);
        s.insert("minecraft:diamond_sword", &sword(1), 1);
        s.insert("minecraft:diamond_sword", &sword(2), 1);
        s.insert("minecraft:stone", b"stone", 5);
        let (key, took) = s.extract_any("minecraft:diamond_sword", 1).unwrap();
        assert_eq!(took, 1);
        assert!(*key == *sword(1) || *key == *sword(2));
        assert_eq!(s.count_item("minecraft:diamond_sword"), 1);
        assert!(s.extract_any("minecraft:dirt", 1).is_none());
    }

    #[test]
    fn what_was_taken_goes_back_even_into_a_full_store() {
        let mut store = Store::new(Limits { types: 1, total: 10 });
        assert_eq!(store.insert("minecraft:stone", b"stone", 10), 10);
        // The last of a type comes out, can't be handed over, and goes back past the limits.
        assert_eq!(store.extract(b"stone", 10), 10);
        assert_eq!(store.insert("minecraft:dirt", b"dirt", 10), 10);
        store.put_back("minecraft:stone", b"stone", 10);
        assert_eq!((store.count_of(b"stone"), store.total()), (10, 20));
    }

    #[test]
    fn limits_stop_new_types_and_total_but_never_lose_items() {
        let mut s = Store::new(Limits { types: 2, total: 100 });
        assert_eq!(s.insert("a", b"a", 60), 60);
        assert_eq!(s.insert("b", b"b", 60), 40); // only 40 of room
        assert_eq!(s.insert("c", b"c", 1), 0);
        s.extract(b"a", 10);
        assert_eq!(s.insert("c", b"c", 1), 0); // two types already
        assert_eq!(s.insert("a", b"a", 5), 5); // more of a known type fits
        s.set_limits(Limits { types: 1, total: 10 }); // plates removed
        assert_eq!((s.types(), s.total()), (2, 95)); // what's stored stays
        assert_eq!(s.insert("a", b"a", 1), 0);
    }

    #[test]
    fn a_burst_of_changes_to_one_item_is_one_record() {
        let mut s = Store::new(ROOMY);
        for _ in 0..1000 {
            s.insert("minecraft:stone", b"stone", 1);
        }
        s.extract(b"stone", 400);
        let saves = s.drain_saves();
        assert_eq!(saves.len(), 1);
        assert_eq!(saves[0].count, 600);
        assert!(s.drain_saves().is_empty());
        assert_eq!(s.drain_viewers().len(), 1); // viewers keep their own place
    }

    #[test]
    fn a_save_log_comes_back_exactly_even_cut_off() {
        let mut s = Store::new(ROOMY);
        s.insert("minecraft:diamond_sword", &sword(1), 1);
        s.insert("minecraft:stone", b"stone", 500);
        let mut log = Vec::new();
        write_records(&s.drain_saves(), &mut log);
        s.extract(&sword(1), 1); // gone
        s.extract(b"stone", 100);
        s.insert("minecraft:dirt", b"dirt", 7);
        write_records(&s.drain_saves(), &mut log);

        let back = Store::load(ROOMY, &log);
        assert_eq!((back.types(), back.total()), (2, 407));
        assert_eq!(back.count_of(b"stone"), 400);
        assert_eq!(back.count_of(&sword(1)), 0);
        assert_eq!(back.count_item("minecraft:dirt"), 7);

        // A crash mid-write loses only the record being written.
        let cut = Store::load(ROOMY, &log[..log.len() - 3]);
        assert!(cut.total() > 0);
        assert!(!should_compact(log.len(), log.len()));
    }

    #[test]
    fn the_live_size_is_known_without_looking_at_the_store() {
        let mut s = Store::new(ROOMY);
        for n in 0..50 {
            s.insert("minecraft:diamond_sword", &sword(n), 1);
        }
        s.insert("minecraft:stone", b"stone", 500);
        s.extract(&sword(3), 1);
        s.extract(b"stone", 100);
        let mut live = Vec::new();
        write_records(&s.snapshot(), &mut live);
        assert_eq!(s.live_bytes(), live.len());
        let mut log = Vec::new();
        write_records(&s.drain_saves(), &mut log);
        assert_eq!(Store::load(ROOMY, &live).live_bytes(), live.len());
        // A log four times past the live data is worth rewriting; a small one never is.
        assert!(should_compact(5 << 20, 1 << 20));
        assert!(!should_compact(3 << 20, 1 << 20));
        assert!(!should_compact(1000, 10));
    }

    /// 300,000 unique tools: a store far past anything Occultism's survives.
    fn huge() -> Store {
        let mut s = Store::new(ROOMY);
        for n in 0..300_000 {
            s.insert("minecraft:diamond_sword", &sword(n), 1);
        }
        s.drain_saves();
        s.drain_viewers();
        s
    }

    #[test]
    fn exact_operations_dont_slow_down_with_size() {
        let mut s = huge();
        let start = Instant::now();
        for n in 0..10_000 {
            s.extract(&sword(n), 1);
            s.insert("minecraft:diamond_sword", &sword(n), 1);
            s.count_of(&sword(n + 1));
        }
        // 30,000 operations on 300,000 unique items; a slot scan would take minutes.
        assert!(start.elapsed() < Duration::from_millis(500), "{:?}", start.elapsed());
    }

    #[test]
    fn a_change_saves_and_syncs_only_itself() {
        let mut s = huge();
        s.insert("minecraft:stone", b"stone", 1);
        let start = Instant::now();
        let saves = s.drain_saves();
        let seen = s.drain_viewers();
        assert_eq!((saves.len(), seen.len()), (1, 1));
        let mut log = Vec::new();
        write_records(&saves, &mut log);
        assert!(log.len() < 64, "{} bytes", log.len());
        assert!(start.elapsed() < Duration::from_millis(5), "{:?}", start.elapsed());
    }

    #[test]
    fn an_idle_store_has_nothing_to_do() {
        let mut s = huge();
        assert!(!s.dirty());
        assert!(s.drain_saves().is_empty() && s.drain_viewers().is_empty());
    }

    #[test]
    fn checking_for_an_item_looks_only_at_its_variants() {
        let mut s = huge();
        s.insert("minecraft:stone", b"stone", 5);
        let start = Instant::now();
        for _ in 0..100_000 {
            assert_eq!(s.count_item("minecraft:stone"), 5);
        }
        assert!(start.elapsed() < Duration::from_millis(500), "{:?}", start.elapsed());
    }

    #[test]
    fn a_huge_store_loads_quickly() {
        let s = huge();
        let mut log = Vec::new();
        write_records(&s.snapshot(), &mut log);
        let start = Instant::now();
        let back = Store::load(ROOMY, &log);
        assert_eq!(back.total(), 300_000);
        eprintln!("{} MB loads in {:?}", log.len() >> 20, start.elapsed());
        assert!(start.elapsed() < Duration::from_secs(5), "{:?}", start.elapsed());
    }
}

// Tests of a store as a lasting network (native/engine/src/network.rs): its record, what placing a
// Core does, and its spirit's name.
#[path = "../../native/engine/src/network.rs"]
pub mod network;

#[cfg(test)]
mod network_tests {
    use super::network::*;
    use super::storage::Limits;

    fn here() -> Anchor {
        Anchor { dimension: "minecraft:the_nether".into(), pos: [-120, 64, 3_000_017] }
    }

    #[test]
    fn a_record_comes_back_exactly_from_its_file() {
        let awake = Record::new("Azurmirkyr".into(), 0x00ab_cdef_u128 << 64 | 7, here());
        let mut awake = awake;
        awake.plates = [4, 0, 1, 0, 0, 2];
        assert_eq!(Record::decode(&awake.encode()), Some(awake.clone()));
        let dormant = Record { anchor: None, ..awake };
        assert!(!dormant.awake());
        assert_eq!(Record::decode(&dormant.encode()), Some(dormant));
        assert_eq!(Record::decode("name=Broken\n"), None);
    }

    #[test]
    fn a_crafted_core_starts_a_store_and_a_carried_one_wakes_its_own() {
        assert_eq!(placing(None, None, 1), Placing::New);
        let dormant = Record { anchor: None, ..Record::new("Kat".into(), 1, here()) };
        assert_eq!(placing(Some(("s1", 0)), Some(&dormant), 1), Placing::Wake("s1".into()));
        // A record lost to a crash doesn't lose the items: the store's file is still found by id.
        assert_eq!(placing(Some(("s1", 0)), None, 1), Placing::Wake("s1".into()));
    }

    #[test]
    fn a_copy_of_a_placed_core_cant_take_its_store() {
        let awake = Record::new("Kat".into(), 1, here());
        assert_eq!(placing(Some(("s1", 0)), Some(&awake), 1), Placing::Copy("s1".into()));
    }

    #[test]
    fn only_its_owner_and_those_they_trust_wake_a_store() {
        let mut dormant = Record { anchor: None, ..Record::new("Kat".into(), 1, here()) };
        // A Core taken from its owner does nothing for the thief, and the store stays dormant, so
        // the owner can still recall it.
        assert_eq!(placing(Some(("s1", 0)), Some(&dormant), 2), Placing::Refused("s1".into()));
        dormant.trusted.push(2);
        assert_eq!(placing(Some(("s1", 0)), Some(&dormant), 2), Placing::Wake("s1".into()));
        assert!(dormant.may_use(1) && dormant.may_use(2) && !dormant.may_use(3));
        assert_eq!(Record::decode(&dormant.encode()), Some(dormant.clone()));
        // A store from before owners is anyone's until someone places its Core.
        let unowned = Record { owner: 0, trusted: Vec::new(), ..dormant };
        assert_eq!(placing(Some(("s1", 0)), Some(&unowned), 3), Placing::Wake("s1".into()));
    }

    #[test]
    fn plates_are_the_stores_and_set_its_capacity() {
        let mut r = Record::new("Kat".into(), 1, here());
        assert_eq!(r.limits(), Limits::with_plates(&[]));
        assert!(r.add_plate(4) && r.add_plate(1));
        assert_eq!((r.plates, r.power()), ([4, 1, 0, 0, 0, 0], 5));
        assert_eq!(r.limits(), Limits::with_plates(&[4, 1]));
        assert!(!r.add_plate(9));
        assert_eq!((r.take_plate(0), r.take_plate(0), r.plates), (Some(4), None, [0, 1, 0, 0, 0, 0]));
        // The freed slot is the next one filled; six at most.
        assert!((0..5).all(|_| r.add_plate(2)) && !r.add_plate(2));
        assert_eq!(r.plates, [2, 1, 2, 2, 2, 2]);
        // A record from before upgrade slots reads with none; one from when there were five, with
        // the sixth empty.
        let old = r.encode().replace("plates=2,1,2,2,2,2\n", "types=704\ntotal=4864000\n");
        assert_eq!(Record::decode(&old).map(|r| r.plates), Some([0; 6]));
        let five = r.encode().replace("plates=2,1,2,2,2,2\n", "plates=4,4,3,0,1\n");
        assert_eq!(Record::decode(&five).map(|r| r.plates), Some([4, 4, 3, 0, 1, 0]));
    }

    #[test]
    fn each_plate_is_a_glyph_of_its_tiers_colour_circling_the_matrix() {
        assert_eq!((1..=4).map(|t| glyph_colour(t).unwrap()).collect::<Vec<_>>(), ["white", "yellow", "purple", "red"]);
        assert_eq!((glyph_colour(0), glyph_colour(5)), (None, None));
        assert!((1..=4).all(|t| glyph_glow(t).is_some()) && glyph_glow(0).is_none() && glyph_glow(5).is_none());
        let length = |p: [f32; 3]| (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        for slot in 0..UPGRADE_SLOTS {
            for tier in 1..=4u8 {
                // On its circle at every moment, close enough to stay over its block.
                let r = 0.55 + 0.05 * f32::from(tier);
                assert!((0..400).all(|t| (length(glyph_orbit(slot, tier, t as f32)) - r).abs() < 1e-4));
                assert!(r < 0.8);
                assert_ne!(glyph_orbit(slot, tier, 0.0), glyph_orbit(slot, tier, 10.0), "it moves");
            }
        }
        // Six plates of a tier never sit on top of each other.
        let at: Vec<[f32; 3]> = (0..UPGRADE_SLOTS).map(|s| glyph_orbit(s, 4, 37.0)).collect();
        assert!(at.iter().enumerate().all(|(i, a)| at[i + 1..].iter().all(|b| length([a[0] - b[0], a[1] - b[1], a[2] - b[2]]) > 0.05)));
    }

    #[test]
    fn a_recall_breaks_the_old_cores_bond() {
        let recalled = Record { anchor: None, bond: 1, ..Record::new("Kat".into(), 1, here()) };
        assert_eq!(placing(Some(("s1", 0)), Some(&recalled), 1), Placing::Stale("s1".into()));
        assert_eq!(placing(Some(("s1", 1)), Some(&recalled), 1), Placing::Wake("s1".into()));
        assert_eq!(Record::decode(&recalled.encode()), Some(recalled.clone()));
        // A record from before bonds reads as bond 0.
        let old = recalled.encode().replace("bond=1\n", "");
        assert_eq!(Record::decode(&old).map(|r| r.bond), Some(0));
    }

    #[test]
    fn a_recall_calls_back_every_lost_store_of_its_caster_only() {
        let me = 7u128;
        let placed = Record::new("Placed".into(), me, here());
        let dormant = Record { anchor: None, ..Record::new("Dormant".into(), me, here()) };
        let carried = Record { anchor: None, ..Record::new("Carried".into(), me, here()) };
        let broken = Record::new("Broken".into(), me, here());
        let theirs = Record { anchor: None, ..Record::new("Theirs".into(), 8, here()) };
        let rebound = Record { anchor: None, bond: 2, ..Record::new("Rebound".into(), me, here()) };
        let stores = [
            ("placed", &placed, false),
            ("dormant", &dormant, false),
            ("carried", &carried, false),
            ("broken", &broken, true),
            ("theirs", &theirs, false),
            ("rebound", &rebound, false),
        ];
        // An old Core of "rebound" in the inventory doesn't count: its bond is broken.
        let inventory = [("carried".to_string(), 0), ("rebound".to_string(), 1)];
        assert_eq!(lost(stores, me, &inventory), ["broken", "dormant", "rebound"]);
        assert!(lost(stores, 99, &inventory).is_empty());
    }

    #[test]
    fn spirits_are_named_as_occultism_names_them() {
        let names: Vec<String> = (0..2000u64).map(|n| spirit_name(n.wrapping_mul(0x2545_F491_4F6C_DD1D))).collect();
        assert!(names.iter().all(|n| n.len() >= 3 && n.chars().next().unwrap().is_uppercase()));
        assert_eq!(spirit_name(42), spirit_name(42));
        let distinct: std::collections::HashSet<&String> = names.iter().collect();
        assert!(distinct.len() > 1500, "{}", distinct.len());
        // One in twenty is an easter egg; the rest are three syllables.
        let eggs = names.iter().filter(|n| ["KliKli", "Xalmas", "TheBoo", "Kat", "Chonky"].contains(&n.as_str())).count();
        assert!(eggs > 0);
    }
}

// Tests of the storage terminal (native/engine/src/terminal.rs): its messages, and the client's
// searched, sorted copy of a store.
#[path = "../../native/engine/src/terminal.rs"]
pub mod terminal;

#[cfg(test)]
mod terminal_tests {
    use super::terminal::*;

    fn entry(key: &[u8], count: u64) -> Option<Entry<()>> {
        let id = std::str::from_utf8(key).ok()?.to_string();
        let name = id.split(':').nth(1)?.replace('_', " ");
        Some(Entry { count, id, name, shown: () })
    }

    fn names(view: &View<()>) -> Vec<String> {
        (0..view.shown()).filter_map(|i| view.at(i).map(|(_, e)| e.name.clone())).collect()
    }

    #[test]
    fn messages_come_back_exactly_and_stay_under_the_limit() {
        assert_eq!(decode(&begin("Gormylmur")), Some(Message::Begin("Gormylmur".into())));
        let keys: Vec<Vec<u8>> = (0..5000u32).map(|i| format!("minecraft:item_{i}_with_a_long_nbt_key").into_bytes()).collect();
        let messages = records(keys.iter().enumerate().map(|(i, k)| (i as u32, i as u64 * 7, k.as_slice())));
        assert!(messages.len() > 1 && messages.iter().all(|m| m.len() <= MAX_MESSAGE));
        let back: Vec<(u32, u64, Vec<u8>)> = messages
            .iter()
            .flat_map(|m| match decode(m) {
                Some(Message::Records(r)) => r,
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(back.len(), 5000);
        assert_eq!(back[4321], (4321, 4321 * 7, keys[4321].clone()));
        assert_eq!(decode(&[1, 0, 0]), None);
    }

    #[test]
    fn serials_name_keys_and_grid_clicks_name_serials() {
        let mut serials = Serials::default();
        let a: std::sync::Arc<[u8]> = b"a".as_slice().into();
        let b: std::sync::Arc<[u8]> = b"b".as_slice().into();
        assert_eq!((serials.of(&a), serials.of(&b), serials.of(&a)), (0, 1, 0));
        assert_eq!(serials.key(1), Some(&b));
        for (serial, right) in [(0, false), (0, true), (123_456, true)] {
            assert_eq!(grid_click(button_for(serial, right)), Some((serial, right)));
        }
        assert_eq!(grid_click(3), None);
    }

    #[test]
    fn the_view_searches_sorts_and_follows_changes() {
        let mut view = View::default();
        view.apply(Message::Begin("Kat".into()), entry);
        let recs = vec![
            (0, 5, b"minecraft:diamond_sword".to_vec()),
            (1, 900, b"minecraft:cobblestone".to_vec()),
            (2, 64, b"occultism:silver_ingot".to_vec()),
            (3, 64, b"minecraft:iron_ingot".to_vec()),
        ];
        view.apply(Message::Records(recs), entry);
        assert!(view.stale());
        view.rebuild();
        assert_eq!(names(&view), ["cobblestone", "iron ingot", "silver ingot", "diamond sword"]);
        view.set_search("ingot".into());
        view.rebuild();
        assert_eq!(names(&view), ["iron ingot", "silver ingot"]);
        view.set_search("@occult".into());
        view.rebuild();
        assert_eq!(names(&view), ["silver ingot"]);
        view.set_search(String::new());
        view.set_sort(Sort::Name);
        // Cobblestone runs out, a sword is taken: the changes, not the store, come over.
        view.apply(Message::Records(vec![(1, 0, Vec::new()), (0, 4, Vec::new())]), entry);
        view.rebuild();
        assert_eq!(names(&view), ["diamond sword", "iron ingot", "silver ingot"]);
        assert_eq!(view.at(0).map(|(s, e)| (s, e.count)), Some((0, 4)));
        assert_eq!(view.total(), 4 + 64 + 64);
        // A new store keeps the player's search and sort.
        view.apply(Message::Begin("Vallen".into()), entry);
        assert_eq!((view.types(), view.sort()), (0, Sort::Name));
    }

    #[test]
    fn scrolling_stops_at_the_last_row() {
        let mut view = View::default();
        let recs = (0..100u32).map(|i| (i, 1, format!("minecraft:item_{i:03}").into_bytes())).collect();
        view.apply(Message::Records(recs), entry);
        view.rebuild();
        // 100 types: 12 rows of 9, six shown at once.
        assert_eq!(view.max_row(), 6);
        view.scroll(50);
        assert_eq!(view.row, 6);
        // The last row holds the hundredth type alone.
        assert!(view.at(45).is_some() && view.at(46).is_none());
        view.scroll(-100);
        assert_eq!(view.row, 0);
    }

    #[test]
    fn counts_fit_a_slot() {
        let cases = [(0, "0"), (999, "999"), (1_000, "1k"), (1_999, "1k"), (239_481, "239k"), (4_864_000, "4M"), (12_000_000_000, "12G")];
        for (n, s) in cases {
            assert_eq!(compact(n), s);
        }
        assert!((0..2_000_000_000u64).step_by(999_983).chain([u64::MAX]).all(|n| compact(n).len() <= 4));
        assert_eq!((grouped(7), grouped(1_000), grouped(239_481), grouped(4_864_000)), ("7".into(), "1,000".into(), "239,481".into(), "4,864,000".into()));
    }
}

// Tests of the dimensional mineshaft (native/engine/src/mining.rs).
#[path = "../../native/engine/src/mining.rs"]
pub mod mining;

#[cfg(test)]
mod mining_tests {
    use super::mining::*;

    const FOLIOT: Miner = Miner { ticks: 400, rolls: 1, multiplier: 1 };

    #[test]
    fn a_miner_mines_once_per_its_time_and_starts_over_when_taken_out() {
        let mut shaft = Shaft::default();
        let mut mined = Vec::new();
        for t in 0..1201 {
            if step(&mut shaft, Some(FOLIOT)) == Step::Mine {
                mined.push(t);
            }
        }
        // Occultism's period is one more than the lamp's time: the tick after an operation sets the
        // next one going.
        assert_eq!(mined, [400, 801]);
        // Taken out halfway: the next lamp waits a whole operation again.
        for _ in 0..200 {
            step(&mut shaft, Some(FOLIOT));
        }
        assert_eq!(step(&mut shaft, None), Step::Idle);
        let eldritch = Miner { ticks: 15, rolls: 1, multiplier: 9 };
        assert_eq!((1..=16).filter(|_| step(&mut shaft, Some(eldritch)) == Step::Mine).count(), 1);
        assert_eq!(step(&mut shaft, Some(eldritch)), Step::Working);
        assert_eq!(shaft.left, 15, "a new operation begins");
    }

    #[test]
    fn results_are_picked_by_weight() {
        assert_eq!(pick(&[], 5), None);
        assert_eq!(pick(&[0, 0], 5), None);
        let weights = [25, 75];
        let picks: Vec<usize> = (0..10_000u64).filter_map(|r| pick(&weights, r.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 7)).collect();
        let first = picks.iter().filter(|i| **i == 0).count() as f64 / picks.len() as f64;
        assert!((first - 0.25).abs() < 0.02, "{first}");
        assert_eq!((pick(&[0, 3], 0), pick(&[0, 3], 2)), (Some(1), Some(1)));
    }

    #[test]
    fn a_batch_merges_equal_results_and_multiplies() {
        let eldritch = Miner { ticks: 15, rolls: 3, multiplier: 9 };
        assert_eq!(batch(&[0, 2, 0], &[1, 1, 2], eldritch), [(0, 18), (2, 18)]);
        assert_eq!(batch(&[1], &[1, 1], FOLIOT), [(1, 1)]);
    }

    #[test]
    fn a_lamp_wears_one_per_operation_and_is_spent_at_its_durability() {
        assert_eq!(wear(0, 511), Some(1));
        assert_eq!(wear(509, 511), Some(510));
        assert_eq!(wear(510, 511), None);
        assert_eq!(wear(7, 0), Some(8), "no durability, no wear-out");
    }
}

#[cfg(test)]
mod accessor_tests {
    use super::terminal::*;

    #[test]
    fn an_accessors_messages_and_requests_come_back_exactly() {
        assert_eq!(decode(&open()), Some(Message::Open));
        assert_eq!(decode(&stats(704, 4_864_000, &[4, 1, 0, 0, 0, 0])), Some(Message::Stats { types: 704, total: 4_864_000, plates: vec![4, 1, 0, 0, 0, 0] }));
        for r in [Request::Take { serial: 0, right: false }, Request::Take { serial: 123_456, right: true }, Request::Close] {
            assert_eq!(Request::decode(&r.encode()), Some(r));
        }
        assert_eq!(Request::decode(&[0, 1, 2]), None);
        assert_eq!(Request::decode(&[9]), None);
        // The view keeps what the server last said of the store's capacity.
        let mut view: View<()> = View::default();
        view.apply(Message::Stats { types: 128, total: 256_000, plates: vec![0; 6] }, |_, _| None);
        assert_eq!(view.stats, Some((128, 256_000, vec![0; 6])));
    }
}
