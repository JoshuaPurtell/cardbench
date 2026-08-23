//! Red discovery regression for Dimir Infiltrator's full-card boundary.

use cardbench_magic_engine::{
    Color, DecisionSelection, Game, GameEvent, Keyword, ManaCost, PlayerId, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

fn infiltrator() -> cardbench_magic_engine::CardDefinition {
    card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DIMIR-INFILTRATOR")
        .expect("Dimir Infiltrator definition exists")
}

#[test]
fn dimir_infiltrator_claims_its_complete_static_and_stack_transmute_rules() {
    let definition = infiltrator();
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(0, [Color::Blue, Color::Black])
    );
    assert_eq!(definition.power, Some(1));
    assert_eq!(definition.toughness, Some(3));
    assert_eq!(
        definition.keywords,
        [
            Keyword::Unblockable,
            Keyword::Transmute(ManaCost::with_colors(1, [Color::Blue, Color::Black])),
        ]
    );
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "Dimir Infiltrator cannot be full fidelity while stack-backed Transmute remains unclaimed"
    );
    assert_eq!(
        definition.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "unblockable",
            "stack-backed-private-transmute",
        ]
    );
}

#[test]
fn dimir_infiltrator_transmute_uses_private_stack_resolution() {
    let definition = infiltrator();
    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let infiltrator = game
        .add_card(PlayerId(0), definition.id, Zone::Hand)
        .expect("Infiltrator starts in hand");
    let found = game
        .add_card(PlayerId(0), definition.id, Zone::Library)
        .expect("matching mana-value card starts in library");
    game.grant_mana(PlayerId(0), Color::Blue, 1)
        .expect("blue symbol is available");
    game.grant_mana(PlayerId(0), Color::Black, 1)
        .expect("black symbol is available");
    game.grant_mana(PlayerId(0), Color::Green, 1)
        .expect("generic payment is available");
    game.clear_event_log();

    game.activate_transmute(PlayerId(0), infiltrator)
        .expect("Transmute pays its cost and enters the stack");
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
        "Dimir Infiltrator full-fidelity trace: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(found), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::Transmuted { player, discarded, found: Some(selected) }
            if *player == PlayerId(0) && *discarded == infiltrator && *selected == found
    )));
    game.validate_invariants()
        .expect("Dimir Infiltrator Transmute preserves stack, privacy, and zone invariants");
}
