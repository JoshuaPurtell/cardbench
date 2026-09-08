//! Full-fidelity Perplex contract: its target controller owns the stack-bound
//! counter-or-discard choice, rather than the engine making it automatically.

use cardbench_magic_engine::{
    CastRequest, Color, DecisionKind, DecisionSelection, Game, GameEvent, PlayerId, PolicyAction,
    PolicyMoveKind, Target, Zone,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions};

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

fn move_to_first_main(game: &mut Game) {
    for _ in 0..2 {
        pass_pair(game);
    }
}

#[test]
#[allow(clippy::too_many_lines)] // The cast/priority/decision transcript is the card contract.
fn perplex_controller_may_save_the_target_spell_by_discarding_its_complete_hand() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV catalog builds");
    let perplex = game
        .add_card(PlayerId(0), "RAV-PERPLEX", Zone::Hand)
        .expect("Perplex enters player zero's hand");
    let last_gasp = game
        .add_card(PlayerId(1), "RAV-LAST-GASP", Zone::Hand)
        .expect("target instant enters player one's hand");
    let discarded = game
        .add_card(PlayerId(1), "RAV-PLAINS", Zone::Hand)
        .expect("discardable card enters player one's hand");
    let watchwolf = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("legal Last Gasp target enters battlefield");
    let island = game
        .put_on_battlefield(PlayerId(0), "RAV-ISLAND")
        .expect("blue source enters for setup");
    let swamps = (0..2)
        .map(|_| {
            game.put_on_battlefield(PlayerId(0), "RAV-SWAMP")
                .expect("black source enters for setup")
        })
        .collect::<Vec<_>>();
    let target_swamp = game
        .put_on_battlefield(PlayerId(1), "RAV-SWAMP")
        .expect("target caster's black source enters");
    let printed_cost_generic = game.put_on_battlefield(PlayerId(1), "RAV-SWAMP").unwrap();
    game.begin_game().expect("game begins");
    move_to_first_main(&mut game);

    game.pass_priority(PlayerId(0))
        .expect("sorcery-speed player passes to instant-speed opponent");
    game.activate_mana_ability(PlayerId(1), target_swamp, Color::Black)
        .expect("target caster produces black mana");
    game.activate_mana_ability(PlayerId(1), printed_cost_generic, Color::Black).unwrap();
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: last_gasp,
            targets: vec![Target::Permanent(watchwolf)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Last Gasp casts as the lower target spell");
    game.pass_priority(PlayerId(1))
        .expect("target caster passes priority to Perplex controller");
    game.activate_mana_ability(PlayerId(0), island, Color::Blue)
        .expect("Perplex controller produces blue mana");
    for swamp in swamps {
        game.activate_mana_ability(PlayerId(0), swamp, Color::Black)
            .expect("Perplex controller produces black mana");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: perplex,
            targets: vec![Target::Spell(last_gasp)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("Perplex targets the lower instant");
    pass_pair(&mut game);

    let decision = game
        .view_for_player(PlayerId(1))
        .expect("target controller view")
        .pending_decision
        .expect("Perplex opens one no-priority counter-or-discard choice");
    assert_eq!(decision.kind, DecisionKind::CounterUnlessDiscardsHand);
    assert!(decision.candidates.is_empty());
    game.submit_policy_move(
        PlayerId(1),
        "rav-perplex.counter-unless-discard-hand.v1",
        PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::CounterUnlessDiscardsHand { discard: true },
        },
    )
    .expect("target controller chooses the printed complete-hand discard branch");

    println!("Perplex event log: {:?}", game.canonical_event_log());
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-PERPLEX"));
    assert_eq!(game.zone_of(perplex), Some(Zone::Graveyard));
    assert!(game.stack.iter().any(|item| item.card == last_gasp));
    assert_eq!(game.zone_of(discarded), Some(Zone::Graveyard));
    assert!(game.event_log.windows(2).any(|events| matches!(
        events,
        [
            GameEvent::CounterUnlessDiscardHandChosen {
                player: PlayerId(1),
                source,
                target_spell,
                discarded: 1,
            },
            GameEvent::DecisionCompleted {
                decision: completed,
                player: PlayerId(1),
                kind: DecisionKind::CounterUnlessDiscardsHand,
            },
        ] if *source == perplex && *target_spell == last_gasp && *completed == decision.id
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PolicyMoveSubmitted {
            player: PlayerId(1),
            policy,
            kind: PolicyMoveKind::SubmitDecision,
        } if policy == "rav-perplex.counter-unless-discard-hand.v1"
    )));
    game.validate_invariants()
        .expect("Perplex preserves the stack and event state machines");
}
