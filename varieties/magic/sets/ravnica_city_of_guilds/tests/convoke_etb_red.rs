//! Red regression for the Convoke cards whose enter-the-battlefield rules
//! require cast-time provenance rather than a static creature chassis.

use cardbench_magic_engine::{
    CastRequest, Color, ConvokeContribution, ConvokePayment, Game, GameEvent, PlayerId, Step, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
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
        game.pass_priority(PlayerId(0))
            .expect("active player passes");
        game.pass_priority(PlayerId(1)).expect("opponent passes");
    }
    assert_eq!(game.step, Step::PrecombatMain);
    assert_eq!(game.priority, PlayerId(0));
}

#[test]
fn conclave_phalanx_counts_its_controllers_creatures_of_every_color_at_etb_resolution() {
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
    game.put_on_battlefield(caster, "RAV-GOLGARI-BROWNSCALE").unwrap();
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
    assert_eq!(game.player(caster).expect("caster exists").life, 24);
    assert!(game.event_log.iter().any(|event| {
        matches!(event, GameEvent::LifeGained { player, amount } if *player == caster && *amount == 4)
    }));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-CONCLAVE-PHALANX"));
    game.validate_invariants()
        .expect("Conclave Phalanx ETB stays invariant-valid");
}

#[test]
fn root_kin_convoke_does_not_invent_enter_the_battlefield_counters() {
    let mut game = game();
    let ally = game.add_card(PlayerId(0), "RAV-ROOT-KIN-ALLY", Zone::Hand).unwrap();
    let first = game.put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE").unwrap();
    let second = game.put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE").unwrap();
    advance_to_precombat_main(&mut game);
    game.add_mana_from_action(PlayerId(0), Color::Green, 4).unwrap();
    game.cast_spell(PlayerId(0), CastRequest { card: ally, targets: vec![],
        convoke: [first, second].into_iter().map(|creature| ConvokePayment {
            creature, contribution: ConvokeContribution::Generic }).collect(),
        payment_mana_abilities: vec![] }).unwrap();
    game.pass_priority(PlayerId(0)).unwrap();
    game.pass_priority(PlayerId(1)).unwrap();
    assert!(game.stack.is_empty());
    for card in [first, second, ally] { assert!(game.object(card).unwrap().counters.is_empty()); }
    game.validate_invariants().unwrap();
}

#[test]
fn root_kin_taps_two_controlled_creatures_for_temporary_plus_two() {
    use cardbench_magic_engine::AbilityActivation;
    let mut game = game();
    let ally = game.put_on_battlefield(PlayerId(0), "RAV-ROOT-KIN-ALLY").unwrap();
    let first = game.put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE").unwrap();
    let second = game.put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF").unwrap();
    advance_to_precombat_main(&mut game);
    game.activate_ability(PlayerId(0), AbilityActivation { source: ally,
        ability_id: "tap-two-creatures-pump-self", sacrifice_sources: vec![],
        additional_tap_creatures: vec![first, second], discard_cards: vec![], targets: vec![] }).unwrap();
    assert!(game.object(first).unwrap().tapped && game.object(second).unwrap().tapped);
    assert_eq!(game.characteristics(ally).unwrap().power, Some(3));
    game.pass_priority(PlayerId(0)).unwrap();
    game.pass_priority(PlayerId(1)).unwrap();
    assert_eq!(game.characteristics(ally).unwrap().power, Some(5));
    assert!(game.object(ally).unwrap().counters.is_empty());
    game.validate_invariants().unwrap();
}
