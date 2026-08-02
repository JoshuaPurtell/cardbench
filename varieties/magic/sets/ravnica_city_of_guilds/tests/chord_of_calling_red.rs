//! Red-to-green regression for Chord of Calling's policy-submitted search.

use cardbench_magic_engine::{
    CastRequest, Color, ConvokeContribution, ConvokePayment, DecisionKind, DecisionSelection,
    DecisionVisibility, Game, GameEvent, LibrarySearchDestination, ManaPaymentSelection, PlayerId,
    PolicyAction, PolicyMoveKind, Zone,
};
use cardbench_magic_rav::{
    card_definitions, executable_definition_id_for_collector, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

#[test]
fn chord_of_calling_requires_an_executable_policy_submitted_search_definition() {
    let collector_resolution = executable_definition_id_for_collector(156);
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CHORD-OF-CALLING");
    println!(
        "Chord discovery: collector_resolution={collector_resolution:?}, definition_present={}",
        definition.is_some()
    );

    assert_eq!(collector_resolution, Ok("RAV-CHORD-OF-CALLING"));
    assert!(
        definition.is_some(),
        "Chord of Calling must have an executable policy-submitted search definition"
    );
}

#[test]
#[allow(clippy::too_many_lines)] // The public selected-X/search trace is intentionally linear.
fn chord_uses_convoke_and_chosen_x_then_waits_for_a_private_exact_search_selection() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let chord = game
        .add_card(PlayerId(0), "RAV-CHORD-OF-CALLING", Zone::Hand)
        .expect("Chord setup");
    let candidates = [
        game.add_card(PlayerId(0), "RAV-BOROS-RECRUIT", Zone::Library)
            .expect("small candidate setup"),
        game.add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Library)
            .expect("selected candidate setup"),
    ];
    let over_bound = game
        .add_card(PlayerId(0), "RAV-GOLIATH-SPIDER", Zone::Library)
        .expect("over-bound candidate setup");
    let opponent_hidden = game
        .add_card(PlayerId(1), "RAV-WATCHWOLF", Zone::Library)
        .expect("opponent hidden card setup");
    let convokers = (0..8)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE")
                .expect("green convoker setup")
        })
        .collect::<Vec<_>>();

    game.cast_spell_with_x(
        PlayerId(0),
        CastRequest {
            card: chord,
            targets: vec![],
            convoke: convokers
                .iter()
                .enumerate()
                .map(|(index, creature)| ConvokePayment {
                    creature: *creature,
                    contribution: if index < 3 {
                        ConvokeContribution::Color(Color::Green)
                    } else {
                        ConvokeContribution::Generic
                    },
                })
                .collect(),
            payment_mana_abilities: vec![],
        },
        2,
        ManaPaymentSelection {
            generic: vec![],
            hybrid: vec![],
        },
    )
    .expect("Chord pays all eight symbols through Convoke");
    game.pass_priority(PlayerId(0))
        .expect("caster passes to the opponent");
    game.pass_priority(PlayerId(1))
        .expect("second pass opens the search decision");

    let controller_view = game
        .view_for_player(PlayerId(0))
        .expect("controller view while Chord resolves");
    let choice = controller_view
        .library_search_choice
        .expect("controller receives private search candidates");
    let decision = controller_view
        .pending_decision
        .expect("controller receives the generic private search decision");
    assert_eq!(choice.source, chord);
    assert_eq!(decision.kind, DecisionKind::LibrarySearch);
    assert_eq!(decision.visibility, DecisionVisibility::Private);
    assert_eq!(decision.min_selections, 0);
    assert_eq!(decision.max_selections, 1);
    assert_eq!(choice.destination, LibrarySearchDestination::Battlefield);
    assert!(choice.may_fail_to_find);
    assert_eq!(
        choice.cards.iter().map(|card| card.id).collect::<Vec<_>>(),
        candidates
    );
    assert!(
        game.view_for_player(PlayerId(1))
            .expect("opponent view")
            .library_search_choice
            .is_none(),
        "opponents never receive the controller's hidden-library candidates"
    );
    assert!(
        game.view_for_player(PlayerId(1))
            .expect("opponent generic view")
            .pending_decision
            .is_none(),
        "opponents never receive the generic hidden-library decision candidates"
    );
    assert!(
        game.event_log
            .iter()
            .all(|event| !matches!(event, GameEvent::LibrarySearchResolved { .. })),
        "opening the private decision does not reveal a selected card"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DecisionOpened {
            decision: event_id,
            player,
            kind: DecisionKind::LibrarySearch,
            visibility: DecisionVisibility::Private,
            ..
        } if *event_id == decision.id && *player == PlayerId(0)
    )));
    assert_eq!(game.zone_of(opponent_hidden), Some(Zone::Library));

    let events_before_rejection = game.event_log.clone();
    assert!(
        game.submit_policy_move(
            PlayerId(1),
            "test.chord.v1",
            PolicyAction::SubmitDecision {
                decision: decision.id,
                selection: DecisionSelection::Objects(vec![candidates[1]]),
            },
        )
        .is_err()
    );
    assert!(
        game.pass_priority(PlayerId(0)).is_err(),
        "priority cannot interleave with the pending search"
    );
    assert!(
        game.submit_policy_move(
            PlayerId(0),
            "test.chord.v1",
            PolicyAction::SubmitDecision {
                decision: decision.id,
                selection: DecisionSelection::Objects(vec![over_bound]),
            },
        )
        .is_err()
    );
    assert_eq!(game.event_log, events_before_rejection);
    assert_eq!(game.zone_of(chord), None);
    assert_eq!(game.zone_of(candidates[1]), Some(Zone::Library));

    game.submit_policy_move(
        PlayerId(0),
        "test.chord.v1",
        PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::Objects(vec![candidates[1]]),
        },
    )
    .expect("exact legal candidate completes Chord");

    assert_eq!(game.zone_of(candidates[1]), Some(Zone::Battlefield));
    assert!(
        !game
            .object(candidates[1])
            .expect("selected card persists")
            .tapped
    );
    assert_eq!(game.zone_of(candidates[0]), Some(Zone::Library));
    assert_eq!(game.zone_of(over_bound), Some(Zone::Library));
    assert_eq!(game.zone_of(chord), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LibrarySearchResolved {
            player,
            source,
            found: Some(card),
            destination: LibrarySearchDestination::Battlefield,
        } if *player == PlayerId(0) && *source == chord && *card == candidates[1]
    )));
    assert!(matches!(
        game.event_log.last(),
        Some(GameEvent::PolicyMoveSubmitted {
            player,
            kind: PolicyMoveKind::SubmitDecision,
            ..
        }) if *player == PlayerId(0)
    ));
    println!(
        "Chord selected-search trace: {:?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("Chord's selected search preserves state invariants");
}

#[test]
fn chord_may_legally_fail_to_find_after_seeing_matching_private_candidates() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV fixture builds");
    let chord = game
        .add_card(PlayerId(0), "RAV-CHORD-OF-CALLING", Zone::Hand)
        .expect("Chord setup");
    let candidate = game
        .add_card(PlayerId(0), "RAV-BOROS-RECRUIT", Zone::Library)
        .expect("matching candidate setup");
    let convokers = (0..7)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE")
                .expect("green convoker setup")
        })
        .collect::<Vec<_>>();

    game.cast_spell_with_x(
        PlayerId(0),
        CastRequest {
            card: chord,
            targets: vec![],
            convoke: convokers
                .iter()
                .enumerate()
                .map(|(index, creature)| ConvokePayment {
                    creature: *creature,
                    contribution: if index < 3 {
                        ConvokeContribution::Color(Color::Green)
                    } else {
                        ConvokeContribution::Generic
                    },
                })
                .collect(),
            payment_mana_abilities: vec![],
        },
        1,
        ManaPaymentSelection {
            generic: vec![],
            hybrid: vec![],
        },
    )
    .expect("Chord X=1 convoke payment succeeds");
    game.pass_priority(PlayerId(0)).expect("first pass");
    game.pass_priority(PlayerId(1)).expect("second pass");
    assert!(
        game.view_for_player(PlayerId(0))
            .expect("controller view")
            .library_search_choice
            .expect("private choice")
            .cards
            .iter()
            .any(|card| card.id == candidate),
        "the controller may decline even a matching hidden-zone candidate"
    );

    game.submit_policy_move(
        PlayerId(0),
        "test.chord.v1",
        PolicyAction::ChooseLibrarySearchCard {
            source: chord,
            selected: None,
        },
    )
    .expect("a hidden-zone search may fail to find");

    assert_eq!(game.zone_of(candidate), Some(Zone::Library));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LibrarySearchResolved {
            source,
            found: None,
            destination: LibrarySearchDestination::Battlefield,
            ..
        } if *source == chord
    )));
    game.validate_invariants()
        .expect("failed search preserves state invariants");
}

#[test]
fn chord_battlefield_entry_queues_the_selected_creatures_etb_after_its_terminal_lifecycle() {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("full RAV fixture builds");
    let chord = game
        .add_card(PlayerId(0), "RAV-CHORD-OF-CALLING", Zone::Hand)
        .expect("Chord setup");
    let caryatid = game
        .add_card(PlayerId(0), "RAV-CARVEN-CARYATID", Zone::Library)
        .expect("ETB creature setup");
    let drawn = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Library)
        .expect("draw card setup");
    let convokers = (0..9)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE")
                .expect("green convoker setup")
        })
        .collect::<Vec<_>>();

    game.cast_spell_with_x(
        PlayerId(0),
        CastRequest {
            card: chord,
            targets: vec![],
            convoke: convokers
                .iter()
                .enumerate()
                .map(|(index, creature)| ConvokePayment {
                    creature: *creature,
                    contribution: if index < 3 {
                        ConvokeContribution::Color(Color::Green)
                    } else {
                        ConvokeContribution::Generic
                    },
                })
                .collect(),
            payment_mana_abilities: vec![],
        },
        3,
        ManaPaymentSelection {
            generic: vec![],
            hybrid: vec![],
        },
    )
    .expect("Chord X=3 convoke payment succeeds");
    game.pass_priority(PlayerId(0)).expect("first pass");
    game.pass_priority(PlayerId(1)).expect("second pass");
    game.submit_policy_move(
        PlayerId(0),
        "test.chord.v1",
        PolicyAction::ChooseLibrarySearchCard {
            source: chord,
            selected: Some(caryatid),
        },
    )
    .expect("selected creature enters through Chord");

    let terminal_index = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == chord))
        .expect("Chord terminal receipt");
    let trigger_index = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::TriggeredAbilityStacked { source, ability, .. }
                    if *source == caryatid && *ability == "etb-draw-controller"
            )
        })
        .expect("selected creature ETB trigger");
    assert!(terminal_index < trigger_index);
    assert_eq!(game.zone_of(caryatid), Some(Zone::Battlefield));

    game.pass_priority(PlayerId(0)).expect("first ETB pass");
    game.pass_priority(PlayerId(1)).expect("second ETB pass");
    assert_eq!(game.zone_of(drawn), Some(Zone::Hand));
    game.validate_invariants()
        .expect("Chord ETB continuation preserves state invariants");
}
