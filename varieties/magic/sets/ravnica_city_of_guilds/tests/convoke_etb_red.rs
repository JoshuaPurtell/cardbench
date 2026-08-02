//! Red regression for the Convoke cards whose enter-the-battlefield rules
//! require cast-time provenance rather than a static creature chassis.

use cardbench_magic_engine::{
    CastRequest, Color, ConvokeContribution, ConvokePayment, Game, GameEvent, PlayerId, Step,
    Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
    RAV_FULL_FIDELITY_DEFINITION_IDS,
};

fn game() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV trigger-enabled game builds")
}

fn advance_to_precombat_main(game: &mut Game) {
    game.begin_game().expect("fixture begins");
    for _ in 0..2 {
        game.pass_priority(PlayerId(0)).expect("active player passes");
        game.pass_priority(PlayerId(1)).expect("opponent passes");
    }
    assert_eq!(game.step, Step::PrecombatMain);
    assert_eq!(game.priority, PlayerId(0));
}

#[test]
fn conclave_phalanx_counts_only_its_controllers_white_creatures_at_etb_resolution() {
    let caster = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = game();
    let phalanx = game
        .add_card(caster, "RAV-CONCLAVE-PHALANX", Zone::Hand)
        .expect("Phalanx exists");
    game.put_on_battlefield(caster, "RAV-WATCHWOLF")
        .expect("white-green controller creature");
    game.put_on_battlefield(caster, "RAV-COURIER-HAWK")
        .expect("white controller creature");
    game.put_on_battlefield(opponent, "RAV-COURIER-HAWK")
        .expect("opponent white creature");
    advance_to_precombat_main(&mut game);
    game.add_mana_from_action(caster, Color::White, 5)
        .expect("five white mana is available");
    game.clear_event_log();

    game.cast_spell(
        caster,
        CastRequest {
            card: phalanx,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Phalanx casts");
    game.pass_priority(caster).expect("caster passes spell");
    game.pass_priority(opponent)
        .expect("opponent passes spell and stacks ETB");
    game.pass_priority(caster).expect("caster passes ETB");
    game.pass_priority(opponent).expect("opponent resolves ETB");

    println!("Conclave Phalanx red trace: {:?}", game.event_log);
    assert_eq!(game.player(caster).expect("caster exists").life, 23);
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::LifeGained { player, amount } if *player == caster && *amount == 3)
    }));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-CONCLAVE-PHALANX"));
    game.validate_invariants()
        .expect("Conclave Phalanx ETB stays invariant-valid");
}

#[test]
fn root_kin_ally_counters_exact_convoke_contributors_but_not_other_creatures() {
    let caster = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = game();
    let ally = game
        .add_card(caster, "RAV-ROOT-KIN-ALLY", Zone::Hand)
        .expect("Root-Kin Ally exists");
    let first = game
        .put_on_battlefield(caster, "RAV-GOLGARI-BROWNSCALE")
        .expect("first green contributor");
    let second = game
        .put_on_battlefield(caster, "RAV-GOLGARI-BROWNSCALE")
        .expect("second green contributor");
    let bystander = game
        .put_on_battlefield(caster, "RAV-WATCHWOLF")
        .expect("non-convoking bystander");
    advance_to_precombat_main(&mut game);
    game.add_mana_from_action(caster, Color::Green, 2)
        .expect("colored Root-Kin mana is available");
    game.clear_event_log();

    game.cast_spell(
        caster,
        CastRequest {
            card: ally,
            targets: vec![],
            convoke: vec![
                ConvokePayment {
                    creature: first,
                    contribution: ConvokeContribution::Generic,
                },
                ConvokePayment {
                    creature: second,
                    contribution: ConvokeContribution::Generic,
                },
            ],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Root-Kin Ally casts through two contributors");
    game.pass_priority(caster).expect("caster passes spell");
    game.pass_priority(opponent)
        .expect("opponent passes spell and stacks ETB");
    game.pass_priority(caster).expect("caster passes ETB");
    game.pass_priority(opponent).expect("opponent resolves ETB");

    println!("Root-Kin Ally red trace: {:?}", game.event_log);
    assert_eq!(
        game.object(first)
            .expect("first contributor remains")
            .counters
            .get(&cardbench_magic_engine::CounterKind::PlusOnePlusOne),
        Some(&1)
    );
    assert_eq!(
        game.object(second)
            .expect("second contributor remains")
            .counters
            .get(&cardbench_magic_engine::CounterKind::PlusOnePlusOne),
        Some(&1)
    );
    assert!(
        game.object(bystander)
            .expect("bystander remains")
            .counters
            .is_empty(),
        "only creatures that actually convoked Root-Kin Ally receive counters"
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-ROOT-KIN-ALLY"));
    game.validate_invariants()
        .expect("Root-Kin Ally ETB stays invariant-valid");
}
