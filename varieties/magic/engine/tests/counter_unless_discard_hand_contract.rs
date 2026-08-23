//! Green contracts for a stack-bound "counter unless discard hand" decision.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DecisionKind, DecisionSelection, Effect, Game,
    GameEvent, ManaCost, PlayerId, PolicyAction, RulesError, Target, Zone,
};

const TARGET: &str = "TST-DISCARD-UNLESS-TARGET";
const COUNTER: &str = "TST-DISCARD-UNLESS-COUNTER";
const HAND: &str = "TST-DISCARD-UNLESS-HAND";

fn definition(id: &'static str, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["counter-unless-discard-hand-contract"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

fn request(card: cardbench_magic_engine::ObjectId, targets: Vec<Target>) -> CastRequest {
    CastRequest {
        card,
        targets,
        convoke: vec![],
        payment_mana_abilities: vec![],
    }
}

fn pass_pair(game: &mut Game) -> Result<(), RulesError> {
    let first = game.priority;
    game.pass_priority(first)?;
    let second = game.priority;
    game.pass_priority(second)
}

fn fixture(
    hand_cards: usize,
) -> (
    Game,
    cardbench_magic_engine::ObjectId,
    cardbench_magic_engine::ObjectId,
    Vec<cardbench_magic_engine::ObjectId>,
) {
    let mut game = Game::new(
        vec![
            definition(TARGET, vec![Effect::DrawController]),
            definition(
                COUNTER,
                vec![Effect::CounterTargetSpellUnlessControllerDiscardsHand],
            ),
            definition(HAND, vec![]),
        ],
        2,
    )
    .expect("fixture constructs");
    let target = game
        .add_card(PlayerId(1), TARGET, Zone::Hand)
        .expect("target spell enters player one's hand");
    let counter = game
        .add_card(PlayerId(0), COUNTER, Zone::Hand)
        .expect("counterspell enters player zero's hand");
    let hand = (0..hand_cards)
        .map(|_| {
            game.add_card(PlayerId(1), HAND, Zone::Hand)
                .expect("discardable card enters player one's hand")
        })
        .collect();
    game.begin_game().expect("fixture game begins");

    // Player one casts first; player zero then responds after priority passes
    // back. This exercises the normal alternating-priority stack path.
    game.pass_priority(PlayerId(0))
        .expect("player zero passes priority to player one");
    game.cast_spell(PlayerId(1), request(target, vec![]))
        .expect("target spell casts");
    game.pass_priority(PlayerId(1))
        .expect("caster passes priority to player zero");
    game.cast_spell(PlayerId(0), request(counter, vec![Target::Spell(target)]))
        .expect("counterspell targets the lower physical spell");
    pass_pair(&mut game).expect("both players pass to open the no-priority decision");

    (game, target, counter, hand)
}

#[test]
fn discard_hand_branch_is_explicit_atomic_and_leaves_target_spell_on_stack() {
    let (mut game, target, counter, hand) = fixture(2);
    let decision = game
        .view_for_player(PlayerId(1))
        .expect("target controller sees a public decision")
        .pending_decision
        .expect("counterspell waits for the target controller's choice");
    assert_eq!(decision.kind, DecisionKind::CounterUnlessDiscardsHand);
    assert!(decision.candidates.is_empty());
    assert!(
        game.view_for_player(PlayerId(0))
            .expect("counter controller sees public decision")
            .pending_decision
            .is_some(),
        "public decision identity is observable to both players"
    );

    let before = game.canonical_event_log();
    assert!(
        game.submit_decision(
            PlayerId(0),
            decision.id,
            DecisionSelection::CounterUnlessDiscardsHand { discard: true },
        )
        .is_err(),
        "only the lower target spell's controller may answer"
    );
    assert_eq!(
        game.canonical_event_log(),
        before,
        "wrong-player answer is atomic"
    );

    game.submit_policy_move(
        PlayerId(1),
        "engine.counter-unless-discard-hand.accept.v1",
        PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::CounterUnlessDiscardsHand { discard: true },
        },
    )
    .expect("target controller explicitly discards their whole hand");

    eprintln!(
        "counter_unless_discard_hand_accept_events={:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(counter), Some(Zone::Graveyard));
    assert!(game.stack.iter().any(|item| item.card == target));
    for card in hand {
        assert_eq!(game.zone_of(card), Some(Zone::Graveyard));
    }
    assert!(game.event_log.windows(2).any(|events| matches!(
        events,
        [
            GameEvent::CounterUnlessDiscardHandChosen {
                player: PlayerId(1),
                source,
                target_spell,
                discarded: 2,
            },
            GameEvent::DecisionCompleted {
                decision: completed,
                player: PlayerId(1),
                kind: DecisionKind::CounterUnlessDiscardsHand,
            },
        ] if *source == counter && *target_spell == target && *completed == decision.id
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PolicyMoveSubmitted {
            player: PlayerId(1),
            ..
        }
    )));
    game.validate_invariants()
        .expect("discard-hand branch preserves stack and event state machines");
}

#[test]
fn empty_hand_may_explicitly_save_the_target_spell_or_decline_and_be_countered() {
    let (mut empty_hand_game, target, counter, _) = fixture(0);
    let decision = empty_hand_game
        .view_for_player(PlayerId(1))
        .expect("target controller view")
        .pending_decision
        .expect("empty hand still opens an explicit choice");
    empty_hand_game
        .submit_decision(
            PlayerId(1),
            decision.id,
            DecisionSelection::CounterUnlessDiscardsHand { discard: true },
        )
        .expect("empty hand is a legal discard-hand choice");
    assert_eq!(empty_hand_game.zone_of(counter), Some(Zone::Graveyard));
    assert!(empty_hand_game.stack.iter().any(|item| item.card == target));
    assert!(empty_hand_game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CounterUnlessDiscardHandChosen { discarded: 0, .. }
    )));
    empty_hand_game
        .validate_invariants()
        .expect("empty-hand accept keeps a valid state machine");

    let (mut decline_game, target, counter, hand) = fixture(1);
    let decision = decline_game
        .view_for_player(PlayerId(1))
        .expect("target controller view")
        .pending_decision
        .expect("counterspell waits for a choice");
    decline_game
        .submit_decision(
            PlayerId(1),
            decision.id,
            DecisionSelection::CounterUnlessDiscardsHand { discard: false },
        )
        .expect("target controller may decline the discard branch");
    eprintln!(
        "counter_unless_discard_hand_decline_events={:?}",
        decline_game.canonical_event_log()
    );
    assert_eq!(decline_game.zone_of(counter), Some(Zone::Graveyard));
    assert_eq!(decline_game.zone_of(target), Some(Zone::Graveyard));
    assert_eq!(decline_game.zone_of(hand[0]), Some(Zone::Hand));
    assert!(decline_game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCountered { card, source } if *card == target && *source == counter
    )));
    decline_game
        .validate_invariants()
        .expect("decline branch preserves stack and event state machines");
}
