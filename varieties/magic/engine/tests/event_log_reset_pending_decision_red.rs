//! Red regression: clearing the measured event log must not corrupt a live
//! id-bearing decision continuation.
//!
//! `clear_event_log` is a public scenario-boundary helper.  Before this
//! regression, calling it after a resolving spell opened a typed decision
//! erased `DecisionOpened` while leaving the private continuation live.  The
//! next invariant audit then rejected the public game state, and submitting
//! the otherwise legal answer could never recover it.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DecisionKind, DecisionSelection, Effect, Game,
    GameEvent, ManaCost, PlayerId, Zone,
};

const REORDER: &str = "EVENT-LOG-RESET-PENDING-DECISION-REORDER";
const FILLER: &str = "EVENT-LOG-RESET-PENDING-DECISION-FILLER";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["event-log-reset-pending-decision-probe"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

#[test]
fn event_log_reset_preserves_a_live_reorder_decision_boundary() {
    let first = PlayerId(0);
    let second = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                REORDER,
                CardType::Instant,
                vec![Effect::RevealTopLibraryCardsAndReorder { count: 1 }],
            ),
            definition(FILLER, CardType::Artifact, vec![]),
        ],
        2,
    )
    .expect("fixture game initializes");
    let spell = game
        .add_card(first, REORDER, Zone::Hand)
        .expect("spell enters the hand");
    game.add_card(first, FILLER, Zone::Library)
        .expect("library contains a visible top card");

    game.cast_spell(
        first,
        CastRequest {
            card: spell,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("spell is cast");
    game.pass_priority(first)
        .expect("caster passes to the opponent");
    game.pass_priority(second)
        .expect("opponent pass opens the reorder decision");
    let decision = game
        .view_for_player(first)
        .expect("controller view exists")
        .pending_decision
        .expect("reorder decision is visible");
    assert_eq!(decision.kind, DecisionKind::LibraryReorder);
    let selected = decision
        .candidates
        .first()
        .expect("top library card is visible")
        .id;

    game.clear_event_log();
    assert!(
        game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::DecisionOpened {
                kind: DecisionKind::LibraryReorder,
                ..
            }
        )),
        "reset must defer rather than erase the opening receipt of a live decision"
    );
    let audit = game.validate_invariants();
    eprintln!(
        "event-log reset while reorder decision pending: audit={audit:?}; events={:?}",
        game.canonical_event_log(),
    );
    assert!(
        audit.is_ok(),
        "a public event-log reset must not strand a live decision continuation"
    );
    game.submit_decision(
        first,
        decision.id,
        DecisionSelection::Objects(vec![selected]),
    )
    .expect("the preserved decision receipt permits its legal completion");
    game.validate_invariants()
        .expect("completed decision remains state-machine valid");
}
