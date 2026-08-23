//! Event-log contract for Spectral Searchlight's recipient-owned mana choice.

use cardbench_magic_engine::{
    AbilityActivation, Color, DecisionKind, DecisionSelection, Game, GameEvent, PlayerId, Target,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn game() -> Game {
    Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds with Searchlight binding")
}

#[test]
#[allow(clippy::too_many_lines)] // Event ordering and atomic rejected choices are the regression contract.
fn searchlight_keeps_its_ability_on_stack_until_the_target_chooses_a_color() {
    let mut game = game();
    let searchlight = game
        .put_on_battlefield(PlayerId(0), "RAV-SPECTRAL-SEARCHLIGHT")
        .expect("Searchlight enters before the measured game");
    game.begin_game().expect("game starts");

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: searchlight,
            ability_id: "tap-target-player-chosen-color-mana",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Player(PlayerId(1))],
        },
    )
    .expect("Searchlight activation uses the stack");
    game.pass_priority(PlayerId(0))
        .expect("controller passes to the target");
    game.pass_priority(PlayerId(1))
        .expect("target passes into resolution");

    let decision = game
        .view_for_player(PlayerId(1))
        .expect("target view")
        .pending_decision
        .expect("target must choose a colored mana output");
    assert_eq!(decision.kind, DecisionKind::TargetPlayerManaColor);
    assert_eq!(decision.color_candidates, Color::ALL);
    assert!(
        game.stack
            .last()
            .is_some_and(|item| item.card == searchlight)
    );
    assert_eq!(game.players[1].mana_pool.amount(Color::Green), 0);

    assert!(
        game.submit_decision(
            PlayerId(0),
            decision.id,
            DecisionSelection::Color(Color::Red),
        )
        .is_err(),
        "the artifact controller cannot answer the target's choice"
    );
    assert!(
        game.submit_decision(
            PlayerId(1),
            decision.id,
            DecisionSelection::Color(Color::Colorless),
        )
        .is_err(),
        "colorless is not one of the five card colors"
    );
    assert_eq!(game.players[1].mana_pool.amount(Color::Green), 0);

    game.submit_decision(
        PlayerId(1),
        decision.id,
        DecisionSelection::Color(Color::Green),
    )
    .expect("target chooses green mana");

    assert!(game.object(searchlight).expect("source remains").tapped);
    assert!(game.stack.is_empty());
    assert_eq!(game.players[0].mana_pool.amount(Color::Green), 0);
    assert_eq!(game.players[1].mana_pool.amount(Color::Green), 1);
    let opened = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::DecisionOpened {
                    kind: DecisionKind::TargetPlayerManaColor,
                    player: PlayerId(1),
                    ..
                }
            )
        })
        .expect("decision receipt");
    let completed = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::DecisionCompleted {
                    kind: DecisionKind::TargetPlayerManaColor,
                    player: PlayerId(1),
                    ..
                }
            )
        })
        .expect("completion receipt");
    let added = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::ManaAdded {
                    player: PlayerId(1),
                    color: Color::Green,
                    amount: 1,
                }
            )
        })
        .expect("target receives chosen mana");
    let resolved = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::AbilityResolved {
                    source,
                    ability: "tap-target-player-chosen-color-mana",
                    ..
                } if *source == searchlight
            )
        })
        .expect("ordinary ability terminal receipt");
    assert!(opened < completed && completed < added && added < resolved);
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-SPECTRAL-SEARCHLIGHT"));
    eprintln!(
        "spectral_searchlight_trace={:?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("Searchlight retains the target-player mana decision invariant");
}
