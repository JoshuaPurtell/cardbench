//! Red discovery contract for Farseek's controller-private library selection.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, DecisionKind, DecisionSelection, DecisionVisibility, Effect,
    Game, GameEvent, LibrarySearchDestination, LibrarySearchRequirement, LibrarySearchSelection,
    ManaCost, PlayerId, PolicyAction, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_basic_land_type_bindings,
};

#[test]
fn farseek_requires_a_private_controller_selected_nonforest_land_search() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-FARSEEK")
        .expect("Farseek definition exists");
    assert_eq!(definition.mana_cost, ManaCost::with_colors(1, [Color::Green]));
    assert_eq!(definition.card_types, [CardType::Sorcery].into());
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert!(definition.supported_rules.contains(&"private-library-selection"));
    assert_eq!(
        executable_definition_id_for_collector(163),
        Ok("RAV-FARSEEK")
    );
    assert_eq!(
        definition.effects,
        [Effect::SearchControllerLibrary {
            requirement: LibrarySearchRequirement::BasicLandTypes(
                [
                    cardbench_magic_engine::BasicLandType::Plains,
                    cardbench_magic_engine::BasicLandType::Island,
                    cardbench_magic_engine::BasicLandType::Swamp,
                    cardbench_magic_engine::BasicLandType::Mountain,
                ]
                .into(),
            ),
            destination: LibrarySearchDestination::BattlefieldTapped,
            selection: LibrarySearchSelection::PolicySubmitted {
                may_fail_to_find: true,
            },
        }]
    );

    let mut game = Game::new_with_basic_land_types(card_definitions(), 2, rav_basic_land_type_bindings())
        .expect("RAV game with basic land types builds");
    let farseek = game
        .add_card(PlayerId(0), definition.id, Zone::Hand)
        .expect("Farseek setup");
    let forest = game
        .add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .expect("ineligible Forest setup");
    let plains = game
        .add_card(PlayerId(0), "RAV-PLAINS", Zone::Library)
        .expect("eligible Plains setup");
    let island = game
        .add_card(PlayerId(0), "RAV-ISLAND", Zone::Library)
        .expect("eligible Island setup");
    game.grant_mana(PlayerId(0), Color::Green, 2)
        .expect("fixture mana");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: farseek,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Farseek casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes and opens choice");

    let view = game.view_for_player(PlayerId(0)).expect("controller view");
    let decision = view
        .pending_decision
        .expect("controller receives the hidden library decision");
    assert_eq!(decision.kind, DecisionKind::LibrarySearch);
    assert_eq!(decision.visibility, DecisionVisibility::Private);
    assert_eq!(decision.min_selections, 0);
    assert_eq!(decision.max_selections, 1);
    assert_eq!(
        view.library_search_choice
            .expect("controller receives matching hidden cards")
            .cards
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>(),
        vec![plains, island]
    );
    assert!(game
        .view_for_player(PlayerId(1))
        .expect("opponent view")
        .pending_decision
        .is_none());
    game.submit_policy_move(
        PlayerId(0),
        "test.farseek.private-search.v1",
        PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::Objects(vec![island]),
        },
    )
    .expect("controller selects Island rather than deterministic first match");

    assert_eq!(game.zone_of(forest), Some(Zone::Library));
    assert_eq!(game.zone_of(plains), Some(Zone::Library));
    assert_eq!(game.zone_of(island), Some(Zone::Battlefield));
    assert!(game.object(island).expect("Island remains live").tapped);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DecisionOpened {
            player,
            kind: DecisionKind::LibrarySearch,
            visibility: DecisionVisibility::Private,
            ..
        } if *player == PlayerId(0)
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LibrarySearchResolved { source, found: Some(card), .. }
            if *source == farseek && *card == island
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellResolved { card } if *card == farseek
    )));
    game.validate_invariants()
        .expect("Farseek private selection preserves all game invariants");
    println!("farseek_private_search_event_log={:#?}", game.canonical_event_log());
}
