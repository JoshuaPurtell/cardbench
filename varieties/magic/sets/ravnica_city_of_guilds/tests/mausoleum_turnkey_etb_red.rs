//! Regression coverage for Mausoleum Turnkey's conditional graveyard-return ETB.

use cardbench_magic_engine::{
    CastRequest, Color, Game, GameEvent, PlayerId, PolicyAction, Target, Zone,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_land_entry_bindings, rav_mana_ability_bindings,
    rav_static_continuous_effect_bindings, rav_triggered_ability_bindings,
};

#[test]
fn mausoleum_turnkey_declares_its_conditional_graveyard_return_slice() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-MAUSOLEUM-TURNKEY")
        .expect("Mausoleum Turnkey definition exists");
    assert!(
        definition
            .supported_rules
            .contains(&"enter-battlefield-conditional-graveyard-return-to-hand")
    );
    assert!(rav_triggered_ability_bindings().iter().any(|binding| {
        binding.card_definition == "RAV-MAUSOLEUM-TURNKEY"
            && binding.ability.id == "conditional-graveyard-return"
    }));
}

fn trigger_game() -> Game {
    Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
        rav_static_continuous_effect_bindings(),
        rav_land_entry_bindings(),
    )
    .expect("RAV trigger fixture constructs")
}

fn cast_and_resolve(
    game: &mut Game,
    player: PlayerId,
    card: cardbench_magic_engine::ObjectId,
    targets: Vec<Target>,
) {
    game.cast_spell(
        player,
        CastRequest {
            card,
            targets,
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("spell casts");
    game.pass_priority(player).expect("controller passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");
}

#[test]
#[allow(clippy::too_many_lines)] // The full ETB trigger/target-resolution trace is one causal RAV contract.
fn mausoleum_turnkey_stacks_and_resolves_when_another_creature_remains() {
    let mut game = trigger_game();
    let turnkey = game
        .add_card(PlayerId(0), "RAV-MAUSOLEUM-TURNKEY", Zone::Hand)
        .expect("Turnkey setup");
    let target = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Graveyard)
        .expect("target creature setup");
    let other = game
        .add_card(PlayerId(0), "RAV-BOROS-RECRUIT", Zone::Graveyard)
        .expect("other creature setup");
    game.grant_mana(PlayerId(0), Color::Black, 4)
        .expect("setup mana");

    cast_and_resolve(&mut game, PlayerId(0), turnkey, vec![]);
    let target_choice = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .triggered_ability_target_choice
        .expect("ETB target choice opens before the trigger is stacked");
    assert_eq!(target_choice.source, turnkey);
    assert_eq!(target_choice.ability, "conditional-graveyard-return");
    assert!(target_choice.target_options[0].contains(&Target::Permanent(target)));
    assert!(target_choice.target_options[0].contains(&Target::Permanent(other)));
    assert!(
        game.view_for_player(PlayerId(1))
            .expect("opponent view")
            .triggered_ability_target_choice
            .is_none(),
        "only the controller sees the ETB target decision"
    );
    game.submit_policy_move(
        PlayerId(0),
        "test.mausoleum-turnkey-target.v1",
        PolicyAction::ChooseTriggeredAbilityTargets {
            source: turnkey,
            ability: "conditional-graveyard-return",
            targets: vec![Target::Permanent(other)],
        },
    )
    .expect("controller chooses the second creature card");
    assert_eq!(game.stack.len(), 1, "chosen ETB trigger is stacked");
    assert_eq!(game.stack[0].targets, vec![Target::Permanent(other)]);
    game.pass_priority(PlayerId(0))
        .expect("controller passes ETB");
    game.pass_priority(PlayerId(1))
        .expect("opponent reaches the optional ETB decision");
    println!(
        "Mausoleum Turnkey trace before optional decision: {:#?}",
        game.canonical_event_log()
    );
    let optional_choice = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .optional_triggered_ability_choice
        .expect("ETB may decision opens after priority passes");
    assert_eq!(optional_choice.source, turnkey);
    assert_eq!(optional_choice.ability, "conditional-graveyard-return");
    assert!(
        optional_choice.can_pay,
        "zero-mana optional trigger is acceptable"
    );
    assert!(optional_choice.conditional_targets.is_empty());
    game.submit_policy_move(
        PlayerId(0),
        "test.mausoleum-turnkey-accept.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            decision: optional_choice.decision,
            source: turnkey,
            ability: "conditional-graveyard-return",
            pay: true,
            target: None,
        },
    )
    .expect("controller accepts the ETB return");
    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(other), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::AbilityResolved { source, ability, .. }
                if *source == turnkey && *ability == "conditional-graveyard-return"
        )
    }));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DecisionOpened {
            player: PlayerId(0),
            ..
        }
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DecisionCompleted {
            player: PlayerId(0),
            ..
        }
    )));
    println!(
        "Mausoleum Turnkey accepted ETB trace: {:#?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("valid conditional return");
}

#[test]
fn mausoleum_turnkey_controller_may_decline_the_selected_return() {
    let mut game = trigger_game();
    let turnkey = game
        .add_card(PlayerId(0), "RAV-MAUSOLEUM-TURNKEY", Zone::Hand)
        .expect("Turnkey setup");
    let target = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Graveyard)
        .expect("target creature setup");
    let other = game
        .add_card(PlayerId(0), "RAV-BOROS-RECRUIT", Zone::Graveyard)
        .expect("other creature setup");
    game.grant_mana(PlayerId(0), Color::Black, 4)
        .expect("setup mana");

    cast_and_resolve(&mut game, PlayerId(0), turnkey, vec![]);
    game.submit_policy_move(
        PlayerId(0),
        "test.mausoleum-turnkey-decline-target.v1",
        PolicyAction::ChooseTriggeredAbilityTargets {
            source: turnkey,
            ability: "conditional-graveyard-return",
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("controller chooses a legal card before deciding whether to return it");
    game.pass_priority(PlayerId(0))
        .expect("controller passes selected ETB");
    game.pass_priority(PlayerId(1))
        .expect("opponent reaches optional ETB decision");
    game.submit_policy_move(
        PlayerId(0),
        "test.mausoleum-turnkey-decline.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            decision: game
                .view_for_player(PlayerId(0))
                .expect("controller view")
                .optional_triggered_ability_choice
                .expect("optional trigger choice")
                .decision,
            source: turnkey,
            ability: "conditional-graveyard-return",
            pay: false,
            target: None,
        },
    )
    .expect("controller declines the selected return");

    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(other), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == turnkey && *ability == "conditional-graveyard-return"
    )));
    println!(
        "Mausoleum Turnkey declined ETB trace: {:#?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("declined conditional return preserves invariants");
}

#[test]
fn mausoleum_turnkey_does_not_stack_without_another_creature_card() {
    let mut game = trigger_game();
    let turnkey = game
        .add_card(PlayerId(0), "RAV-MAUSOLEUM-TURNKEY", Zone::Hand)
        .expect("Turnkey setup");
    let only = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Graveyard)
        .expect("only creature setup");
    game.grant_mana(PlayerId(0), Color::Black, 4)
        .expect("setup mana");

    cast_and_resolve(&mut game, PlayerId(0), turnkey, vec![]);
    assert!(
        game.stack.is_empty(),
        "intervening condition prevents stacking"
    );
    assert_eq!(game.zone_of(only), Some(Zone::Graveyard));
    game.validate_invariants()
        .expect("valid no-trigger boundary");
}
