//! Red regression for Consult the Necrosages' recipient-private discard.
//!
//! This intentionally requires the targeted player, rather than the caster
//! or a deterministic resolver fallback, to select the two cards discarded
//! by Consult's discard mode.

use cardbench_magic_engine::{
    CastRequest, Color, DecisionId, DecisionKind, DecisionSelection, Game, PlayerId, PolicyAction,
    RulesError, Step, Target, Zone,
};
use cardbench_magic_rav::card_definitions;

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

fn advance_to_precombat_main(game: &mut Game) {
    game.begin_game().expect("fixture begins");
    while game.step != Step::PrecombatMain {
        if game
            .view_for_player(game.active_player)
            .expect("active player view")
            .draw_replacement_pending
        {
            game.resolve_pending_draw(game.active_player, None)
                .expect("ordinary draw resolves");
        } else {
            pass_pair(game);
        }
    }
}

fn cast_request(card: cardbench_magic_engine::ObjectId) -> CastRequest {
    CastRequest {
        card,
        targets: vec![Target::Player(PlayerId(1))],
        convoke: vec![],
        payment_mana_abilities: vec![],
    }
}

fn give_consult_mana(game: &mut Game) {
    for color in [Color::Blue, Color::Black, Color::Colorless] {
        game.add_mana_from_action(PlayerId(0), color, 1)
            .expect("Consult payment mana");
    }
}

#[test]
fn consult_target_privately_chooses_non_oldest_hand_cards_to_discard() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let consult = game
        .add_card(PlayerId(0), "RAV-CONSULT-THE-NECROSAGES", Zone::Hand)
        .expect("Consult begins in hand");
    game.add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
        .expect("opening draw card exists");
    let oldest = game
        .add_card(PlayerId(1), "RAV-PLAINS", Zone::Hand)
        .expect("oldest target hand card");
    let selected_one = game
        .add_card(PlayerId(1), "RAV-ISLAND", Zone::Hand)
        .expect("first chosen target hand card");
    let selected_two = game
        .add_card(PlayerId(1), "RAV-SWAMP", Zone::Hand)
        .expect("second chosen target hand card");
    advance_to_precombat_main(&mut game);
    give_consult_mana(&mut game);
    game.submit_policy_move(
        PlayerId(0),
        "test.consult-necrosages.recipient-private-discard.v1",
        PolicyAction::CastWithMode {
            request: cast_request(consult),
            mode: 1,
        },
    )
    .expect("caster selects Consult discard mode");
    pass_pair(&mut game);

    let choice = game
        .view_for_player(PlayerId(1))
        .expect("target view")
        .pending_decision
        .unwrap_or_else(|| {
            panic!(
                "Consult must open a recipient-private discard decision instead of discarding oldest cards; trace={:#?}",
                game.canonical_event_log()
            )
        });
    assert_eq!(choice.kind, DecisionKind::ConditionalPrivateDiscard);
    assert!(
        game.view_for_player(PlayerId(0))
            .expect("caster view")
            .pending_decision
            .is_none(),
        "the caster must not see the recipient's private hand candidates"
    );
    let before_events = game.event_log.clone();
    let before_hand = game.players[1].hand.clone();
    assert_eq!(
        game.submit_decision(
            PlayerId(0),
            choice.id,
            DecisionSelection::Objects(vec![selected_one, selected_two]),
        ),
        Err(RulesError::IllegalAction(
            "only the decision player may submit this decision"
        ))
    );
    assert_eq!(game.event_log, before_events);
    assert_eq!(game.players[1].hand, before_hand);
    assert_eq!(
        game.submit_decision(
            PlayerId(1),
            DecisionId(choice.id.0 + 1),
            DecisionSelection::Objects(vec![selected_one, selected_two]),
        ),
        Err(RulesError::IllegalAction("stale or unknown decision id"))
    );
    assert_eq!(game.event_log, before_events);
    assert_eq!(game.players[1].hand, before_hand);
    game.submit_decision(
        PlayerId(1),
        choice.id,
        DecisionSelection::Objects(vec![selected_one, selected_two]),
    )
    .expect("target selects two non-oldest hand cards");

    assert_eq!(game.zone_of(oldest), Some(Zone::Hand));
    assert_eq!(game.zone_of(selected_one), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(selected_two), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(consult), Some(Zone::Graveyard));
    game.validate_invariants()
        .expect("recipient-private discard preserves state-machine invariants");
}
