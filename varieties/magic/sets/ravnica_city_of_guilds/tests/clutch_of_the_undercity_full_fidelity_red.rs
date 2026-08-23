//! Red discovery regression for Clutch of the Undercity's full-card boundary.

use cardbench_magic_engine::{
    Color, DecisionSelection, Effect, Game, GameEvent, Keyword, ManaCost, PlayerId, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

fn clutch() -> cardbench_magic_engine::CardDefinition {
    card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-CLUTCH-OF-THE-UNDERCITY")
        .expect("Clutch of the Undercity definition exists")
}

#[test]
fn clutch_of_the_undercity_uses_its_exact_front_face_mana_value() {
    let definition = clutch();
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(1, [Color::Blue, Color::Blue, Color::Black]),
        "the front face must retain its exact mana value for casting and Transmute searches"
    );
    assert_eq!(
        definition.keywords,
        [Keyword::Transmute(ManaCost::with_colors(
            1,
            [Color::Blue, Color::Blue]
        ))]
    );
    assert_eq!(
        definition.effects,
        [Effect::ReturnTargetPermanentToHandAndLoseControllerLife { amount: 3 }]
    );
}

#[test]
fn clutch_of_the_undercity_combines_its_front_face_with_stack_backed_transmute() {
    let definition = clutch();
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "Clutch cannot be full fidelity while stack-backed Transmute remains unclaimed"
    );
    assert_eq!(
        definition.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "targeted-permanent-bounce",
            "controller-life-loss",
            "stack-backed-private-transmute",
        ]
    );

    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let clutch = game
        .add_card(PlayerId(0), definition.id, Zone::Hand)
        .expect("Clutch starts in hand");
    let found = game
        .add_card(PlayerId(0), definition.id, Zone::Library)
        .expect("matching mana-value card starts in library");
    game.grant_mana(PlayerId(0), Color::Blue, 2)
        .expect("blue symbols are available");
    game.grant_mana(PlayerId(0), Color::Green, 1)
        .expect("one generic payment is available");
    game.clear_event_log();

    game.activate_transmute(PlayerId(0), clutch)
        .expect("Transmute pays its cost and enters the stack");
    assert_eq!(game.zone_of(clutch), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(found), Some(Zone::Library));
    assert_eq!(game.stack.len(), 1, "Transmute waits for responses");
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
        "the opponent cannot inspect controller-library candidates"
    );
    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Objects(vec![found]),
    )
    .expect("controller chooses the matching card while ability resolves");

    println!(
        "Clutch full-fidelity trace: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(found), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::Transmuted { player, discarded, found: Some(selected) }
            if *player == PlayerId(0) && *discarded == clutch && *selected == found
    )));
    game.validate_invariants()
        .expect("Clutch Transmute preserves stack, privacy, and zone invariants");
}
