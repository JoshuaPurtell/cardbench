//! Full-fidelity contracts for Sunhome's typed mana and Double Strike ability.

use cardbench_magic_engine::{
    AbilityActivation, Color, Game, GameEvent, Keyword, ManaAbilityActivation, PlayerId, Target,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

fn game_with_rav_bindings() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds")
}

#[test]
fn sunhome_has_a_complete_typed_definition_and_bindings() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SUNHOME-FORTRESS")
        .expect("Sunhome definition");
    assert_eq!(
        executable_definition_id_for_collector(282),
        Ok("RAV-SUNHOME-FORTRESS")
    );
    assert_eq!(
        definition.supported_rules,
        [
            "full-rules-fidelity",
            "colorless-mana-ability",
            "activated-double-strike-grant",
        ]
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(rav_mana_ability_bindings().iter().any(|binding| {
        binding.card_definition == definition.id && binding.ability.id == "produce-colorless"
    }));
    assert!(rav_activated_ability_bindings().iter().any(|binding| {
        binding.card_definition == definition.id
            && binding.ability.id == "grant-target-double-strike"
    }));
}

#[test]
fn sunhome_colorless_mana_is_not_a_colored_choice() {
    let mut game = game_with_rav_bindings();
    let land = game
        .put_on_battlefield(PlayerId(0), "RAV-SUNHOME-FORTRESS")
        .expect("Sunhome starts on the battlefield");
    game.begin_game().expect("game starts");
    game.clear_event_log();

    game.activate_bound_mana_ability(
        PlayerId(0),
        ManaAbilityActivation {
            source: land,
            ability_id: "produce-colorless",
            chosen_color: None,
        },
    )
    .expect("Sunhome produces colorless mana");

    assert_eq!(
        game.player(PlayerId(0))
            .expect("player exists")
            .mana_pool
            .amount(Color::Colorless),
        1
    );
    assert_eq!(
        game.player(PlayerId(0))
            .expect("player exists")
            .mana_pool
            .amount(Color::Red),
        0
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::BoundManaAbilityActivated { source, ability, color, amount, .. }
            if *source == land && *ability == "produce-colorless" && *color == Color::Colorless && *amount == 1
    )));
    game.validate_invariants()
        .expect("Sunhome mana activation is valid");
}

#[test]
fn sunhome_grants_double_strike_through_a_typed_stack_activation() {
    let mut game = game_with_rav_bindings();
    let land = game
        .put_on_battlefield(PlayerId(0), "RAV-SUNHOME-FORTRESS")
        .expect("Sunhome starts on the battlefield");
    let creature = game
        .put_on_battlefield(PlayerId(0), "RAV-BOROS-RECRUIT")
        .expect("target creature starts on the battlefield");
    let mountains = (0..4)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
                .expect("Mountain starts on battlefield")
        })
        .collect::<Vec<_>>();
    let plains = game
        .put_on_battlefield(PlayerId(0), "RAV-PLAINS")
        .expect("Plains starts on battlefield");
    game.begin_game().expect("game starts");
    for mountain in mountains {
        game.activate_mana_ability(PlayerId(0), mountain, Color::Red)
            .expect("Mountain produces red mana");
    }
    game.activate_mana_ability(PlayerId(0), plains, Color::White)
        .expect("Plains produces white mana");

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: land,
            ability_id: "grant-target-double-strike",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(creature)],
        },
    )
    .expect("Sunhome activation enters the stack");
    game.pass_priority(PlayerId(0))
        .expect("activator passes to resolve Sunhome");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes to resolve Sunhome");

    println!(
        "Sunhome activation trace: {:#?}",
        game.canonical_event_log()
    );
    assert!(
        game.characteristics(creature)
            .expect("target survives")
            .keywords
            .contains(&Keyword::DoubleStrike)
    );
    assert!(game.object(land).expect("Sunhome remains").tapped);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectCreated { source, target, .. }
            if *source == land && *target == creature
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability }
            if *source == land && *ability == "grant-target-double-strike"
    )));
    game.validate_invariants()
        .expect("Sunhome activation preserves invariants");
}
