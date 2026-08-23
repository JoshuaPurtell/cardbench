//! Red regression for Dimir House Guard's stack-Transmute fidelity contract.

use cardbench_magic_engine::{Color, DecisionSelection, Game, GameEvent, PlayerId, Zone};
use cardbench_magic_rav::card_definitions;

fn house_guard() -> cardbench_magic_engine::CardDefinition {
    card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DIMIR-HOUSE-GUARD")
        .expect("Dimir House Guard definition exists")
}

#[test]
fn dimir_house_guard_full_manifest_claims_stack_backed_transmute() {
    let definition = house_guard();
    assert_eq!(
        definition.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "fear",
            "stack-backed-private-transmute",
            "sacrifice-creature-regenerate",
        ]
    );
}

#[test]
fn dimir_house_guard_transmute_uses_private_stack_resolution() {
    let definition = house_guard();
    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let guard = game
        .add_card(PlayerId(0), definition.id, Zone::Hand)
        .expect("House Guard starts in hand");
    let found = game
        .add_card(PlayerId(0), "RAV-CLUTCH-OF-THE-UNDERCITY", Zone::Library)
        .expect("matching mana-value card starts in library");
    game.grant_mana(PlayerId(0), Color::Black, 2)
        .expect("black symbols are available");
    game.grant_mana(PlayerId(0), Color::Green, 1)
        .expect("generic payment is available");
    game.clear_event_log();

    game.activate_transmute(PlayerId(0), guard)
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
        "Dimir House Guard stack-Transmute trace: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(found), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::Transmuted { player, discarded, found: Some(selected) }
            if *player == PlayerId(0) && *discarded == guard && *selected == found
    )));
    game.validate_invariants()
        .expect("House Guard Transmute preserves stack, privacy, and zone invariants");
}
