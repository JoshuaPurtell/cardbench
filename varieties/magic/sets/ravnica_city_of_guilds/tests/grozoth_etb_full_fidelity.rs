//! Full event-log coverage for Grozoth's optional entry search.

use cardbench_magic_engine::{
    CastRequest, Color, DecisionKind, DecisionSelection, Game, GameEvent, PlayerId, PolicyAction,
    Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

fn game_with_triggers() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV trigger fixture builds")
}

fn cast_and_resolve_creature(game: &mut Game, card: cardbench_magic_engine::ObjectId) {
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Grozoth casts");
    game.pass_priority(PlayerId(0))
        .expect("controller passes on Grozoth");
    game.pass_priority(PlayerId(1))
        .expect("Grozoth resolves and its ETB trigger stacks");
}

fn open_optional_search(game: &mut Game, grozoth: cardbench_magic_engine::ObjectId) {
    assert_eq!(game.stack.len(), 1, "Grozoth ETB trigger is on the stack");
    game.pass_priority(PlayerId(0))
        .expect("controller passes on Grozoth ETB");
    game.pass_priority(PlayerId(1))
        .expect("opponent pass opens the optional ETB choice");
    let choice = game
        .view_for_player(PlayerId(0))
        .expect("controller view is available")
        .optional_triggered_ability_choice
        .expect("optional Grozoth ETB choice opens");
    assert_eq!(choice.source, grozoth);
    assert_eq!(choice.ability, "etb-search-mana-value-nine");
    assert!(choice.can_pay, "the zero-mana ETB can be accepted");
    assert!(
        game.view_for_player(PlayerId(1))
            .expect("opponent view is available")
            .optional_triggered_ability_choice
            .is_none(),
        "only the controller receives the may decision"
    );
}

#[test]
fn grozoth_etb_privately_reveals_any_chosen_mana_value_nine_cards_then_shuffles() {
    let mut game = game_with_triggers();
    let grozoth = game
        .add_card(PlayerId(0), "RAV-GROZOTH", Zone::Hand)
        .expect("Grozoth setup");
    let first = game
        .add_card(PlayerId(0), "RAV-GROZOTH", Zone::Library)
        .expect("first mana-value-nine answer setup");
    let second = game
        .add_card(PlayerId(0), "RAV-GROZOTH", Zone::Library)
        .expect("second mana-value-nine answer setup");
    let off_value = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Library)
        .expect("off-value card setup");
    game.grant_mana(PlayerId(0), Color::Blue, 9)
        .expect("fixture supplies Grozoth's cost");

    cast_and_resolve_creature(&mut game, grozoth);
    open_optional_search(&mut game, grozoth);
    game.submit_policy_move(
        PlayerId(0),
        "test.grozoth-etb-accept.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            decision: game
                .view_for_player(PlayerId(0))
                .expect("controller view")
                .optional_triggered_ability_choice
                .expect("optional trigger choice")
                .decision,
            source: grozoth,
            ability: "etb-search-mana-value-nine",
            pay: true,
            target: None,
        },
    )
    .expect("controller accepts Grozoth ETB search");

    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view is available")
        .pending_decision
        .expect("private library search opens after accepting");
    assert_eq!(decision.kind, DecisionKind::LibrarySearch);
    assert_eq!(decision.min_selections, 0);
    assert_eq!(
        decision.max_selections, 2,
        "any number is bounded by the two legal candidates actually present"
    );
    assert_eq!(decision.candidates.len(), 2);
    assert!(
        game.view_for_player(PlayerId(1))
            .expect("opponent view is available")
            .pending_decision
            .is_none(),
        "opponent must not see the controller's library candidates"
    );
    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Objects(vec![second, first]),
    )
    .expect("controller selects any number of matching cards");

    assert_eq!(game.zone_of(first), Some(Zone::Hand));
    assert_eq!(game.zone_of(second), Some(Zone::Hand));
    assert_eq!(game.zone_of(off_value), Some(Zone::Library));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LibrarySearchBatchResolved { source, found, .. }
            if *source == grozoth && found == &vec![second, first]
    )));
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::CardRevealed { card, .. } if *card == first || *card == second))
            .count(),
        2,
        "each selected card is publicly revealed"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == grozoth && *ability == "etb-search-mana-value-nine"
    )));
    println!(
        "Grozoth accepted ETB trace: {:#?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("Grozoth accepted ETB preserves engine invariants");
}

#[test]
fn grozoth_controller_may_decline_the_entry_search_without_opening_library_choice() {
    let mut game = game_with_triggers();
    let grozoth = game
        .add_card(PlayerId(0), "RAV-GROZOTH", Zone::Hand)
        .expect("Grozoth setup");
    let library_card = game
        .add_card(PlayerId(0), "RAV-GROZOTH", Zone::Library)
        .expect("library card setup");
    game.grant_mana(PlayerId(0), Color::Blue, 9)
        .expect("fixture supplies Grozoth's cost");

    cast_and_resolve_creature(&mut game, grozoth);
    open_optional_search(&mut game, grozoth);
    game.submit_policy_move(
        PlayerId(0),
        "test.grozoth-etb-decline.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            decision: game
                .view_for_player(PlayerId(0))
                .expect("controller view")
                .optional_triggered_ability_choice
                .expect("optional trigger choice")
                .decision,
            source: grozoth,
            ability: "etb-search-mana-value-nine",
            pay: false,
            target: None,
        },
    )
    .expect("controller declines Grozoth ETB search");

    assert_eq!(game.zone_of(library_card), Some(Zone::Library));
    assert!(
        game.view_for_player(PlayerId(0))
            .expect("controller view is available")
            .pending_decision
            .is_none(),
        "declining must not expose a library decision"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == grozoth && *ability == "etb-search-mana-value-nine"
    )));
    println!(
        "Grozoth declined ETB trace: {:#?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("declined Grozoth ETB preserves engine invariants");
}
