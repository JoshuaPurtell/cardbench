//! Red regression for Perilous Forays' controller-private library choice.

use cardbench_magic_engine::{
    AbilityActivation, Color, DecisionKind, DecisionSelection, Game, PlayerId, PolicyAction, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn perilous_forays_waits_for_its_controllers_private_basic_land_choice() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV fixture builds");
    let forays = game
        .add_card(PlayerId(0), "RAV-PERILOUS-FORAYS", Zone::Battlefield)
        .expect("Perilous Forays setup");
    let victim = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Battlefield)
        .expect("sacrifice cost setup");
    let forest = game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .expect("Forest setup");
    let plains = game
        .add_card(PlayerId(0), "RAV-PLAINS", Zone::Library)
        .expect("Plains setup");
    game.grant_mana(PlayerId(0), Color::Green, 1)
        .expect("ability mana");

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: forays,
            ability_id: "sacrifice-creature-search-basic-land",
            sacrifice_sources: vec![victim],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Perilous Forays activation");
    game.pass_priority(PlayerId(0)).expect("controller passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");

    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("Perilous Forays must suspend for its controller's private library choice");
    assert_eq!(decision.kind, DecisionKind::LibrarySearch);
    assert!(
        game.view_for_player(PlayerId(1))
            .expect("opponent view")
            .pending_decision
            .is_none(),
        "the opponent must not receive hidden library candidates"
    );
    game.submit_policy_move(
        PlayerId(0),
        "perilous-forays-policy-search-test.v1",
        PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::Objects(vec![plains]),
        },
    )
    .expect("controller selects the non-first basic land");
    assert_eq!(game.zone_of(forest), Some(Zone::Library));
    assert_eq!(game.zone_of(plains), Some(Zone::Battlefield));
    assert!(game.object(plains).expect("Plains persists").tapped);
    println!(
        "Perilous Forays policy-search trace: {:?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("private search choice preserves stack and hidden-zone invariants");
}
