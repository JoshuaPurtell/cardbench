//! Full-fidelity contract for Civic Wayfinder's private optional land search.

use cardbench_magic_engine::{
    CastRequest, Color, DecisionKind, DecisionSelection, DecisionVisibility, Game, GameEvent,
    PlayerId, PolicyAction, Step, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

fn civic_wayfinder_search_fixture() -> (
    Game,
    cardbench_magic_engine::ObjectId,
    cardbench_magic_engine::ObjectId,
    cardbench_magic_engine::ObjectId,
) {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV fixture constructs");
    game.step = Step::PrecombatMain;
    let wayfinder = game
        .add_card(PlayerId(0), "RAV-CIVIC-WAYFINDER", Zone::Hand)
        .expect("Wayfinder setup");
    let forest = game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .expect("Forest library setup");
    let island = game
        .add_card(PlayerId(0), "RAV-ISLAND", Zone::Library)
        .expect("Island library setup");
    game.grant_mana(PlayerId(0), Color::Green, 3)
        .expect("Wayfinder mana");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: wayfinder,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Wayfinder casts");
    for player in [PlayerId(0), PlayerId(1), PlayerId(0), PlayerId(1)] {
        game.pass_priority(player)
            .expect("spell then ETB trigger priority passes");
    }
    (game, wayfinder, forest, island)
}

#[test]
fn civic_wayfinder_controller_selects_and_reveals_one_eligible_basic_land() {
    let (mut game, wayfinder, forest, island) = civic_wayfinder_search_fixture();
    let controller_view = game.view_for_player(PlayerId(0)).expect("controller view");
    let decision = controller_view
        .pending_decision
        .expect("controller-private library search opens");
    assert_eq!(decision.kind, DecisionKind::LibrarySearch);
    assert_eq!(decision.visibility, DecisionVisibility::Private);
    assert_eq!((decision.min_selections, decision.max_selections), (0, 1));
    assert_eq!(
        controller_view
            .library_search_choice
            .expect("controller sees only legal hidden candidates")
            .cards
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>(),
        vec![forest, island]
    );
    assert!(
        game.view_for_player(PlayerId(1))
            .expect("opponent view")
            .pending_decision
            .is_none(),
        "the opponent cannot inspect the controller's library selection"
    );

    game.submit_policy_move(
        PlayerId(0),
        "civic-wayfinder-full.v1",
        PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::Objects(vec![island]),
        },
    )
    .expect("controller selects Island rather than a deterministic first match");

    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-CIVIC-WAYFINDER"));
    assert_eq!(game.zone_of(forest), Some(Zone::Library));
    assert_eq!(game.zone_of(island), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardRevealed {
            player: PlayerId(0),
            card,
            definition: "RAV-ISLAND",
        } if *card == island
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LibrarySearchResolved { source, found: Some(card), .. }
            if *source == wayfinder && *card == island
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == wayfinder && *ability == "etb-search-basic-land-to-hand"
    )));
    game.validate_invariants()
        .expect("selected-and-revealed Civic Wayfinder search preserves invariants");
    eprintln!(
        "Civic Wayfinder selected trace={:?}",
        game.canonical_event_log()
    );
}

#[test]
fn civic_wayfinder_may_decline_without_revealing_a_library_card() {
    let (mut game, wayfinder, forest, island) = civic_wayfinder_search_fixture();
    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("optional library search opens");
    game.submit_policy_move(
        PlayerId(0),
        "civic-wayfinder-full.v1",
        PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::Objects(vec![]),
        },
    )
    .expect("controller declines to find");

    assert_eq!(game.zone_of(forest), Some(Zone::Library));
    assert_eq!(game.zone_of(island), Some(Zone::Library));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CardRevealed { card, .. } if *card == forest || *card == island
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LibrarySearchResolved { source, found: None, .. } if *source == wayfinder
    )));
    game.validate_invariants()
        .expect("declined Civic Wayfinder search preserves invariants");
    eprintln!(
        "Civic Wayfinder decline trace={:?}",
        game.canonical_event_log()
    );
}
