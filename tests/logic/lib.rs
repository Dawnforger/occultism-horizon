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
