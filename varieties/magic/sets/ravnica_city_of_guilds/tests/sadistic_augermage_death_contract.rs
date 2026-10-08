//! Stack and private-choice contract for Sadistic Augermage's source-death trigger.

use cardbench_magic_engine::{
    ContinuousChange, DecisionVisibility, Duration, Game, GameEvent, PlayerId, PolicyAction,
    PolicyMoveKind, Step, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

const ABILITY: &str = "source-dies-each-player-puts-card-on-library-top";

fn game_with_rav_bindings() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds")
}

#[test]
#[allow(clippy::too_many_lines)] // One trace covers the full mandatory private-choice boundary.
fn source_death_stacks_then_each_player_puts_a_hand_card_on_library_top() {
    let mut game = game_with_rav_bindings();
    let augermage = game
        .put_on_battlefield(PlayerId(0), "RAV-SADISTIC-AUGERMAGE")
        .expect("Augermage enters");
    game.set_entered_turn_for_setup(augermage, 0)
        .expect("Augermage predates measured turn");
    let p0_top = game
        .add_card(PlayerId(0), "RAV-SWAMP", Zone::Hand)
        .expect("controller choice enters hand");
    let p0_keep = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Hand)
        .expect("controller retains another card");
    let p1_top = game
        .add_card(PlayerId(1), "RAV-ISLAND", Zone::Hand)
        .expect("opponent choice enters hand");

    game.begin_game().expect("fixture begins game");
    assert_eq!(game.step, Step::Upkeep);
    game.add_continuous_effect(
        augermage,
        augermage,
        ContinuousChange::ModifyPowerToughness {
            power: 0,
            toughness: -1,
        },
        Duration::EndOfTurn(1),
    )
    .expect("the setup modifier makes Augermage die to an SBA");
    assert_eq!(game.zone_of(augermage), Some(Zone::Graveyard));
    assert_eq!(game.stack.len(), 1, "source-death trigger is queued");
    assert!(
        game.view_for_player(PlayerId(0))
            .expect("controller view")
            .optional_triggered_ability_choice
            .is_none(),
        "the printed trigger is mandatory"
    );

    game.pass_priority(PlayerId(0))
        .expect("active player passes");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes to resolve trigger");
    let controller_view = game.view_for_player(PlayerId(0)).expect("controller view");
    assert_eq!(
        controller_view
            .pending_decision
            .as_ref()
            .expect("controller has a pending private decision")
            .visibility,
        DecisionVisibility::Private
    );
    let controller_choice = controller_view
        .triggered_ability_effect_object_choice
        .expect("controller receives the first private choice");
    assert_eq!(controller_choice.candidates.len(), 2);
    game.submit_policy_move(
        PlayerId(0),
        "test.augermage-library-top-p0.v1",
        PolicyAction::ChooseTriggeredAbilityEffectObject {
            decision: controller_choice.decision,
            source: augermage,
            ability: ABILITY,
            selected: Some(p0_top),
        },
    )
    .expect("controller selects a card privately");

    let opponent_view = game.view_for_player(PlayerId(1)).expect("opponent view");
    assert_eq!(
        opponent_view
            .pending_decision
            .as_ref()
            .expect("opponent has a pending private decision")
            .visibility,
        DecisionVisibility::Private
    );
    let opponent_choice = opponent_view
        .triggered_ability_effect_object_choice
        .expect("opponent receives its own private choice");
    assert_eq!(opponent_choice.candidates.len(), 1);
    game.submit_policy_move(
        PlayerId(1),
        "test.augermage-library-top-p1.v1",
        PolicyAction::ChooseTriggeredAbilityEffectObject {
            decision: opponent_choice.decision,
            source: augermage,
            ability: ABILITY,
            selected: Some(p1_top),
        },
    )
    .expect("opponent selects a card privately");

    println!(
        "Sadistic Augermage trace: {:#?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(p0_top), Some(Zone::Library));
    assert_eq!(game.zone_of(p1_top), Some(Zone::Library));
    assert_eq!(game.zone_of(p0_keep), Some(Zone::Hand));
    assert_eq!(
        game.player(PlayerId(0))
            .expect("controller exists")
            .library
            .last(),
        Some(&p0_top),
        "the chosen card becomes the controller's library top"
    );
    assert_eq!(
        game.player(PlayerId(1))
            .expect("opponent exists")
            .library
            .last(),
        Some(&p1_top),
        "the opponent's chosen card becomes their library top"
    );
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::CardDiscarded { .. }))
    );
    for card in [p0_top, p1_top] {
        assert!(game.event_log.iter().any(|event| {
            matches!(event, GameEvent::CardMoved { card: moved, to: Zone::Library } if *moved == card)
        }));
    }
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == augermage && *ability == ABILITY
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == augermage && *ability == ABILITY
    )));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PolicyMoveSubmitted {
            kind: PolicyMoveKind::ResolveOptionalTriggeredAbility,
            ..
        }
    )));
    game.validate_invariants()
        .expect("private library-top choice trace preserves invariants");
}
