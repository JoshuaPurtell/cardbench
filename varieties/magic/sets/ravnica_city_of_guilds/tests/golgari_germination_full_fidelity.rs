//! Event-log contract for Golgari Germination's controller-scoped dies trigger.

use cardbench_magic_engine::{
    CastRequest, Color, ContinuousChange, Duration, Game, GameEvent, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
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

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

#[test]
fn controlled_nontoken_death_creates_one_saproling_but_token_and_opponent_deaths_do_not() {
    let mut game = game_with_rav_bindings();
    let germination = game
        .put_on_battlefield(PlayerId(0), "RAV-GOLGARI-GERMINATION")
        .expect("Germination setup");
    let aura_target = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("Aura target setup");
    let controlled_victim = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("controlled victim setup");
    let opponent_victim = game
        .put_on_battlefield(PlayerId(1), "RAV-WATCHWOLF")
        .expect("opponent victim setup");
    let fists = game
        .add_card(PlayerId(0), "RAV-FISTS-OF-IRONWOOD", Zone::Hand)
        .expect("token-making fixture spell");
    game.grant_mana(PlayerId(0), Color::Green, 2)
        .expect("fixture mana");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: fists,
            targets: vec![Target::Permanent(aura_target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Fists casts");
    resolve_top(&mut game);
    resolve_top(&mut game);
    let token = game
        .player(PlayerId(0))
        .expect("controller exists")
        .battlefield
        .iter()
        .copied()
        .find(|card| game.object(*card).expect("live object").token.is_some())
        .expect("Fists created a token");

    game.add_continuous_effect(
        controlled_victim,
        token,
        ContinuousChange::ModifyPowerToughness {
            power: -2,
            toughness: -2,
        },
        Duration::EndOfTurn(1),
    )
    .expect("token dies through SBA");
    game.add_continuous_effect(
        controlled_victim,
        opponent_victim,
        ContinuousChange::ModifyPowerToughness {
            power: -4,
            toughness: -4,
        },
        Duration::EndOfTurn(1),
    )
    .expect("opponent creature dies through SBA");
    assert_eq!(
        game.stack.len(),
        0,
        "ineligible deaths do not trigger Germination"
    );

    game.add_continuous_effect(
        controlled_victim,
        controlled_victim,
        ContinuousChange::ModifyPowerToughness {
            power: -4,
            toughness: -4,
        },
        Duration::EndOfTurn(1),
    )
    .expect("controlled nontoken creature dies through SBA");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == germination
                && *ability == "controlled-nontoken-creature-dies-create-saproling"
    )));
    assert_eq!(game.stack.len(), 1, "one eligible death stacks one trigger");
    resolve_top(&mut game);

    let saprolings = game
        .player(PlayerId(0))
        .expect("controller exists")
        .battlefield
        .iter()
        .filter(|card| game.object(**card).expect("live object").token.is_some())
        .count();
    assert_eq!(
        saprolings, 2,
        "one token died and one eligible death replaced it"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == germination
                && *ability == "controlled-nontoken-creature-dies-create-saproling"
    )));
    println!(
        "golgari_germination_event_log={:#?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("Germination trigger trace preserves invariants");
}
