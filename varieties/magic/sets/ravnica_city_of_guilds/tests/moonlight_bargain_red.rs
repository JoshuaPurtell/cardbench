//! Regression contract for Moonlight Bargain's private resolution choice.

use cardbench_magic_engine::{
    CastRequest, Color, Game, GameEvent, PlayerId, PolicyAction, PolicyMoveKind, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
#[allow(clippy::too_many_lines)] // The lifecycle and information-boundary audit is intentionally one trace.
fn moonlight_bargain_uses_a_private_no_priority_resolution_choice() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV catalog constructs");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-MOONLIGHT-BARGAIN"));
    let bargain = game
        .add_card(PlayerId(0), "RAV-MOONLIGHT-BARGAIN", Zone::Hand)
        .expect("Moonlight Bargain definition exists");
    let library = [
        game.add_card(PlayerId(0), "RAV-PLAINS", Zone::Library)
            .expect("first private library card"),
        game.add_card(PlayerId(0), "RAV-ISLAND", Zone::Library)
            .expect("second private library card"),
        game.add_card(PlayerId(0), "RAV-SWAMP", Zone::Library)
            .expect("third private library card"),
        game.add_card(PlayerId(0), "RAV-MOUNTAIN", Zone::Library)
            .expect("fourth private library card"),
        game.add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
            .expect("top private library card"),
    ];
    game.grant_mana(PlayerId(0), Color::Black, 5)
        .expect("Moonlight mana is available during setup");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: bargain,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Moonlight casts");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1))
        .expect("opponent causes suspended resolution");

    let controller_view = game.view_for_player(PlayerId(0)).expect("controller view");
    let choice = controller_view
        .private_library_choice
        .expect("only controller sees the pending private choice");
    assert_eq!(choice.spell, bargain);
    assert_eq!(
        choice.cards.iter().map(|card| card.id).collect::<Vec<_>>(),
        vec![library[4], library[3], library[2], library[1], library[0]],
        "the controller sees the top five in top-first order"
    );
    assert_eq!(choice.life_per_card, 2);
    assert!(
        game.view_for_player(PlayerId(1))
            .expect("opponent view")
            .private_library_choice
            .is_none(),
        "opponent cannot inspect the pending hidden-zone choice"
    );
    assert!(
        game.pass_priority(PlayerId(0)).is_err(),
        "a player cannot pass while this spell is resolving"
    );
    assert!(
        game.cast_spell(
            PlayerId(0),
            CastRequest {
                card: bargain,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
        )
        .is_err(),
        "a priority action cannot interleave with the private choice"
    );

    game.submit_policy_move(
        PlayerId(0),
        "moonlight-private-choice-contract",
        PolicyAction::ChoosePrivateLibraryCards {
            spell: bargain,
            selected: vec![library[4], library[1]],
        },
    )
    .expect("controller completes the atomic private choice");

    println!(
        "Moonlight Bargain event log: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.player(PlayerId(0)).expect("controller").life, 16);
    assert_eq!(game.zone_of(library[4]), Some(Zone::Hand));
    assert_eq!(game.zone_of(library[1]), Some(Zone::Hand));
    for card in [library[3], library[2], library[0]] {
        assert_eq!(game.zone_of(card), Some(Zone::Graveyard));
    }
    assert_eq!(game.zone_of(bargain), Some(Zone::Graveyard));
    let looked_at = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::CardsLookedAt { viewer, source, count, .. } if *viewer == PlayerId(0) && *source == bargain && *count == 5))
        .expect("private inspection receipt");
    let life_paid = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::LifePaid { source, player, amount } if *source == bargain && *player == PlayerId(0) && *amount == 4))
        .expect("per-card life receipt");
    let resolved = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == bargain))
        .expect("spell resolution receipt");
    assert!(looked_at < life_paid && life_paid < resolved);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PolicyMoveSubmitted {
            kind: PolicyMoveKind::ChoosePrivateLibraryCards,
            ..
        }
    )));
    game.validate_invariants()
        .expect("private resolution boundary preserves invariants");
}
