//! Red discovery regression for Brainspoil's full card boundary.

use cardbench_magic_engine::{
    Color, DecisionSelection, Effect, Game, GameEvent, Keyword, ManaCost, PlayerId, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

#[test]
fn brainspoil_uses_typed_nonblack_destruction_and_stack_backed_transmute() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-BRAINSPOIL")
        .expect("Brainspoil definition exists");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "Brainspoil cannot be full fidelity while its Transmute remains classified as immediate"
    );
    assert_eq!(
        definition.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "destroy-target-nonblack-creature",
            "stack-backed-private-transmute",
        ]
    );
    assert_eq!(
        definition.keywords,
        [Keyword::Transmute(ManaCost::with_colors(1, [Color::Black, Color::Black]))]
    );
    assert_eq!(definition.effects, [Effect::DestroyTargetNonblackCreature]);

    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let brainspoil = game
        .add_card(PlayerId(0), definition.id, Zone::Hand)
        .expect("Brainspoil starts in hand");
    let found = game
        .add_card(PlayerId(0), definition.id, Zone::Library)
        .expect("matching mana-value card starts in library");
    game.grant_mana(PlayerId(0), Color::Black, 2)
        .expect("black symbols are available");
    game.grant_mana(PlayerId(0), Color::Green, 1)
        .expect("one generic payment is available");
    game.clear_event_log();

    game.activate_transmute(PlayerId(0), brainspoil)
        .expect("Transmute pays its cost and enters the stack");
    assert_eq!(game.zone_of(brainspoil), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(found), Some(Zone::Library));
    assert_eq!(game.stack.len(), 1, "Transmute must await responses");

    game.pass_priority(PlayerId(0))
        .expect("controller passes on Transmute");
    game.pass_priority(PlayerId(1))
        .expect("opponent pass opens the private search");
    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view is available")
        .pending_decision
        .expect("Transmute search decision is pending");
    assert_eq!(
        decision
            .candidates
            .iter()
            .map(|candidate| candidate.id)
            .collect::<Vec<_>>(),
        [found]
    );
    assert!(
        game.view_for_player(PlayerId(1))
            .expect("opponent view is available")
            .pending_decision
            .is_none(),
        "the opponent must not receive controller-library candidates"
    );
    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Objects(vec![found]),
    )
    .expect("controller chooses the matching library card while ability resolves");

    println!("Brainspoil full-fidelity trace: {:?}", game.canonical_event_log());
    assert_eq!(game.zone_of(found), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::Transmuted { player, discarded, found: Some(selected) }
            if *player == PlayerId(0) && *discarded == brainspoil && *selected == found
    )));
    game.validate_invariants()
        .expect("Brainspoil Transmute preserves stack, privacy, and zone invariants");
}
