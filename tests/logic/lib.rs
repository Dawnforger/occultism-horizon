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
}
