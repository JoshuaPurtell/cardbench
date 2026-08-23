//! Red discovery contract for Agrus Kos's attack-triggered color-specific
//! combat modifiers.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, Color, Game, GameEvent, Layer, ManaCost, ObjectId, PlayerId, Step,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings, run_public_scenario,
};

fn agrus_game() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV Agrus fixture builds")
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

fn advance_one_policy_action(game: &mut Game) {
    let active = game.active_player;
    if game.step == Step::DeclareAttackers
        && !game
            .view_for_player(active)
            .expect("attacker view")
            .attackers_declared
    {
        game.declare_attackers(active, &[])
            .expect("empty attacker declaration");
        return;
    }
    if game.step == Step::DeclareBlockers
        && !game
            .view_for_player(game.next_policy_player())
            .expect("blocker view")
            .blockers_declared
    {
        let defender = game.next_policy_player();
        game.declare_blockers(defender, &[])
            .expect("empty blocker declaration");
        return;
    }
    game.pass_priority(game.priority)
        .expect("priority holder advances turn state");
}

fn advance_to(game: &mut Game, turn: u32, step: Step) {
    for _ in 0..64 {
        if game.turn == turn && game.step == step {
            return;
        }
        advance_one_policy_action(game);
        game.validate_invariants()
            .expect("ordinary turn transition preserves invariants");
    }
    panic!(
        "turn machine did not reach turn {turn} step {step:?}; reached turn {} step {:?}",
        game.turn, game.step
    );
}

fn power_toughness(game: &Game, card: ObjectId) -> (Option<i32>, Option<i32>) {
    let characteristics = game.characteristics(card).expect("creature remains live");
    (characteristics.power, characteristics.toughness)
}

#[test]
fn agrus_kos_requires_its_attack_triggered_color_specific_combat_modifiers() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-AGRUS-KOS-WOJEK-VETERAN")
        .expect("Agrus Kos definition exists");

    assert_eq!(definition.name, "Agrus Kos, Wojek Veteran");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(3, [Color::Red, Color::White])
    );
    assert_eq!(
        definition.colors,
        BTreeSet::from([Color::Red, Color::White])
    );
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(3), Some(3)));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(
        definition
            .supported_rules
            .contains(&"attack-triggered-color-specific-combat-modifiers")
    );
}

#[test]
#[allow(clippy::too_many_lines)] // The public combat trace verifies recipient snapshot and cleanup expiry.
fn agrus_kos_snapshots_only_current_attackers_and_modifies_each_matching_color() {
    let mut game = agrus_game();
    let agrus = game
        .put_on_battlefield(PlayerId(0), "RAV-AGRUS-KOS-WOJEK-VETERAN")
        .expect("Agrus setup");
    let red_white = game
        .put_on_battlefield(PlayerId(0), "RAV-BOROS-RECRUIT")
        .expect("red/white attacker setup");
    let red = game
        .put_on_battlefield(PlayerId(0), "RAV-GOBLIN-FIRE-FIEND")
        .expect("red attacker setup");
    let white = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("white attacker setup");
    let nonattacking_red = game
        .put_on_battlefield(PlayerId(0), "RAV-GOBLIN-FIRE-FIEND")
        .expect("same-color nonattacker setup");
    for creature in [agrus, red_white, red, white, nonattacking_red] {
        game.set_entered_turn_for_setup(creature, 0)
            .expect("fixture creatures are not summoning sick");
    }

    game.begin_game().expect("game begins");
    advance_to(&mut game, 1, Step::DeclareAttackers);
    game.declare_attackers(PlayerId(0), &[agrus, red_white, red, white])
        .expect("legal attackers declare");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == agrus && *ability == "attack-color-specific-combat-modifiers"
    )));

    pass_pair(&mut game);
    assert_eq!(power_toughness(&game, agrus), (Some(5), Some(5)));
    assert_eq!(power_toughness(&game, red_white), (Some(3), Some(3)));
    assert_eq!(power_toughness(&game, red), (Some(3), Some(1)));
    assert_eq!(power_toughness(&game, white), (Some(3), Some(5)));
    assert_eq!(
        power_toughness(&game, nonattacking_red),
        (Some(1), Some(1)),
        "same-colored nonattackers are outside the trigger's combat snapshot"
    );
    let created = game
        .event_log
        .iter()
        .filter(|event| {
            matches!(
                event,
                GameEvent::ContinuousEffectCreated {
                    source,
                    layer: Layer::PowerToughness,
                    ..
                } if *source == agrus
            )
        })
        .count();
    assert_eq!(
        created, 6,
        "each matching attacker gets one modifier per color"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == agrus && *ability == "attack-color-specific-combat-modifiers"
    )));
    game.validate_invariants()
        .expect("attack-trigger recipient snapshot preserves invariants");

    advance_to(&mut game, 2, Step::Upkeep);
    for creature in [agrus, red_white, red, white, nonattacking_red] {
        let _ = game
            .characteristics(creature)
            .expect("creature remains live after cleanup");
    }
    assert_eq!(power_toughness(&game, agrus), (Some(3), Some(3)));
    assert_eq!(power_toughness(&game, red_white), (Some(1), Some(1)));
    assert_eq!(power_toughness(&game, red), (Some(1), Some(1)));
    assert_eq!(power_toughness(&game, white), (Some(3), Some(3)));
    let expired = game
        .event_log
        .iter()
        .filter(|event| {
            matches!(
                event,
                GameEvent::ContinuousEffectExpired {
                    source,
                    layer: Layer::PowerToughness,
                    ..
                } if *source == agrus
            )
        })
        .count();
    assert_eq!(
        expired, 6,
        "every attack-trigger modifier expires at cleanup"
    );
    eprintln!("Agrus Kos trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("cleanup expiration preserves combat-modifier provenance");
}

#[test]
fn agrus_kos_public_scenario_preserves_the_reviewable_stack_trace() {
    let result = run_public_scenario("rav_agrus_kos_attack_color_modifiers")
        .expect("public Agrus Kos fixture executes");
    assert_eq!(result.digest, "fnv1a64:29b052fe6ec09e66");
    assert!(
        result
            .event_log
            .iter()
            .any(|event| event.contains("TriggeredAbilityStacked"))
    );
    assert_eq!(
        result
            .event_log
            .iter()
            .filter(|event| event.contains("ContinuousEffectCreated"))
            .count(),
        6
    );
    eprintln!("Agrus Kos public scenario trace={:?}", result.event_log);
}
