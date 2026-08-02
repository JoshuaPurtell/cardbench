//! Red regression for Blood Funnel's controller-selected sacrifice branch.

use cardbench_magic_engine::{
    CastRequest, Color, DecisionKind, DecisionSelection, Game, GameEvent, PlayerId, Target, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_cost_reduction_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

fn game_with_funnel_rules() -> Game {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV catalog constructs");
    game.register_cost_reduction_bindings(rav_cost_reduction_bindings())
        .expect("Blood Funnel reduction registers");
    game
}

#[test]
fn blood_funnel_controller_chooses_which_creature_to_sacrifice() {
    let mut game = game_with_funnel_rules();
    game.add_card(PlayerId(0), "RAV-BLOOD-FUNNEL", Zone::Battlefield)
        .expect("Blood Funnel setup");
    let first = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("first creature setup");
    let chosen = game
        .add_card(PlayerId(0), "RAV-GLASS-GOLEM", Zone::Battlefield)
        .expect("second creature setup");
    let target = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("Aura target setup");
    let spell = game
        .add_card(PlayerId(0), "RAV-FISTS-OF-IRONWOOD", Zone::Hand)
        .expect("noncreature spell setup");
    game.grant_mana(PlayerId(0), Color::Green, 1)
        .expect("reduced spell mana");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Blood Funnel reduces and triggers for the noncreature spell");
    game.pass_priority(PlayerId(0))
        .expect("caster passes trigger");
    game.pass_priority(PlayerId(1))
        .expect("trigger reaches its resolution choice");

    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("Blood Funnel must not choose a creature by battlefield order");
    assert_eq!(decision.kind, DecisionKind::TriggeredEffectObject);
    assert_eq!(decision.min_selections, 1);
    assert_eq!(decision.max_selections, 1);
    assert_eq!(
        decision
            .candidates
            .iter()
            .map(|candidate| candidate.id)
            .collect::<Vec<_>>(),
        vec![first, chosen, target],
        "all and only controller creatures are selectable"
    );
    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Objects(vec![chosen]),
    )
    .expect("controller chooses the second creature rather than the first");

    println!(
        "Blood Funnel selected-sacrifice trace: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(first), Some(Zone::Battlefield));
    assert_eq!(game.zone_of(chosen), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(target), Some(Zone::Battlefield));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SacrificedByEffect { player, permanent, .. }
            if *player == PlayerId(0) && *permanent == chosen
    )));
    game.validate_invariants()
        .expect("Blood Funnel selected sacrifice preserves invariants");
}
