//! RED: Ghosts of the Innocent and Szadek share the affected player's
//! policy-controlled combat-damage replacement ordering boundary.
//!
//! Szadek's five combat damage to a player is simultaneously subject to its
//! own "mill and add counters" replacement and Ghosts' global halving
//! replacement.  CR 616 requires the affected player to select which applies
//! first: halving first mills two cards, while Szadek first mills five cards
//! and ends the event before Ghosts can apply.

use cardbench_magic_engine::{
    DamageReplacementChoice, DecisionKind, DecisionSelection, Game, GameEvent, PlayerId,
    PolicyAction, ReplacementChoice, Step, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_damage_replacement_effect_bindings,
};

const POLICY_ID: &str = "test.rav-replacement-ordering.v1";

fn game_with_competing_replacements() -> (Game, cardbench_magic_engine::ObjectId) {
    let attacker_controller = PlayerId(0);
    let affected_player = PlayerId(1);
    let mut game = Game::new_with_all_bindings(card_definitions(), 2, [], [], [], [])
        .expect("RAV fixture initializes");
    game.register_damage_replacement_effect_bindings(rav_damage_replacement_effect_bindings())
        .expect("RAV damage-replacement bindings register before the game begins");
    let szadek = game
        .put_on_battlefield(attacker_controller, "RAV-SZADEK")
        .expect("Szadek begins on the battlefield");
    game.put_on_battlefield(affected_player, "RAV-GHOSTS-OF-THE-INNOCENT")
        .expect("Ghosts begins on the battlefield");
    for _ in 0..5 {
        game.add_card(affected_player, "RAV-FOREST", Zone::Library)
            .expect("affected player has five cards available to mill");
    }
    game.set_entered_turn_for_setup(szadek, 0)
        .expect("Szadek predates this turn");
    game.begin_game().expect("fixture begins");
    while game.step != Step::DeclareAttackers {
        let player = game.priority;
        game.pass_priority(player)
            .expect("priority advances the deterministic combat fixture");
    }
    game.declare_attackers(attacker_controller, &[szadek])
        .expect("Szadek attacks");
    game.pass_priority(attacker_controller)
        .expect("attacker controller passes");
    game.pass_priority(affected_player)
        .expect("combat advances to blockers");
    game.declare_blockers(affected_player, &[])
        .expect("affected player declares no blockers");
    for _ in 0..2 {
        let player = game.priority;
        game.pass_priority(player)
            .expect("combat advances to the replacement-decision boundary");
    }
    (game, szadek)
}

fn pending_choice(
    game: &Game,
    affected_player: PlayerId,
    predicate: impl Fn(DamageReplacementChoice) -> bool,
) -> (cardbench_magic_engine::DecisionId, ReplacementChoice) {
    let decision = game
        .view_for_player(affected_player)
        .expect("affected player has a public game view")
        .pending_decision
        .expect("concurrent replacements open a public no-priority decision");
    assert_eq!(decision.kind, DecisionKind::Replacement);
    assert_eq!(game.next_policy_player(), affected_player);
    let choice = decision
        .replacement_candidates
        .iter()
        .copied()
        .find_map(|choice| match choice {
            ReplacementChoice::Damage(damage) if predicate(damage) => Some(choice),
            _ => None,
        })
        .expect("the requested live replacement is offered to the affected player");
    (decision.id, choice)
}

#[test]
fn affected_player_can_choose_ghosts_before_szadek_through_a_policy_move() {
    let affected_player = PlayerId(1);
    let (mut game, szadek) = game_with_competing_replacements();
    let (decision, halving) = pending_choice(&game, affected_player, |choice| {
        matches!(choice, DamageReplacementChoice::HalveDamage { .. })
    });

    game.submit_policy_move(
        affected_player,
        POLICY_ID,
        PolicyAction::SubmitDecision {
            decision,
            selection: DecisionSelection::Replacements(vec![halving]),
        },
    )
    .expect("affected player can choose Ghosts before Szadek");

    eprintln!("Ghosts-then-Szadek trace={:?}", game.canonical_event_log());
    assert_eq!(
        game.player(affected_player)
            .expect("affected player remains live")
            .library
            .len(),
        3,
        "Ghosts halves five damage to two before Szadek mills",
    );
    assert_eq!(
        game.characteristics(szadek)
            .expect("Szadek remains live")
            .power,
        Some(7),
        "Szadek receives two +1/+1 counters after the selected halving replacement",
    );
    let applied = game
        .event_log
        .iter()
        .filter_map(|event| match event {
            GameEvent::DamageReplacementApplied { replacement, .. } => Some(*replacement),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        applied.as_slice(),
        [
            DamageReplacementChoice::HalveDamage { .. },
            DamageReplacementChoice::CombatDamageMillAndCounters { source, .. },
        ] if *source == szadek
    ));
    game.validate_invariants()
        .expect("Ghosts-then-Szadek replacement lifecycle is auditable");
}

#[test]
fn affected_player_can_choose_szadek_before_ghosts_through_a_policy_move() {
    let affected_player = PlayerId(1);
    let (mut game, szadek) = game_with_competing_replacements();
    let (decision, szadek_replacement) = pending_choice(&game, affected_player, |choice| {
        matches!(
            choice,
            DamageReplacementChoice::CombatDamageMillAndCounters { .. }
        )
    });

    game.submit_policy_move(
        affected_player,
        POLICY_ID,
        PolicyAction::SubmitDecision {
            decision,
            selection: DecisionSelection::Replacements(vec![szadek_replacement]),
        },
    )
    .expect("affected player can choose Szadek before Ghosts");

    eprintln!("Szadek-then-Ghosts trace={:?}", game.canonical_event_log());
    assert!(
        game.player(affected_player)
            .expect("affected player remains live")
            .library
            .is_empty(),
        "Szadek consumes the five-damage event before Ghosts can halve it",
    );
    assert_eq!(
        game.characteristics(szadek)
            .expect("Szadek remains live")
            .power,
        Some(10),
        "Szadek receives five +1/+1 counters from the unhalved packet",
    );
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::DamageReplacementApplied {
                replacement: DamageReplacementChoice::HalveDamage { .. },
                ..
            }
        )),
        "a replacement whose event was fully replaced cannot apply afterward",
    );
    game.validate_invariants()
        .expect("Szadek-then-Ghosts replacement lifecycle is auditable");
}

#[test]
fn ghosts_and_szadek_declare_the_now_complete_replacement_ordering_contract() {
    for definition in ["RAV-GHOSTS-OF-THE-INNOCENT", "RAV-SZADEK"] {
        assert!(
            RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition),
            "{definition} has all printed behavior represented by the policy-controlled replacement substrate",
        );
        let card = card_definitions()
            .into_iter()
            .find(|card| card.id == definition)
            .expect("RAV card exists");
        assert!(
            card.supported_rules.contains(&"full-rules-fidelity"),
            "{definition} must not retain a stale bounded-replacement label",
        );
    }
}
