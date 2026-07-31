//! Red discovery contract for Screeching Griffin's activated evasion ability.
//!
//! This test intentionally starts as a failing milestone: the card is present
//! as a Flying-compatible chassis, but its `{R}` ability that prevents one
//! chosen creature from blocking this source is not yet bound to the stack.

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, CombatBlock, Game, GameEvent, Keyword, PlayerId, Target,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
};
use cardbench_magic_rav::{
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn screeching_griffin_requires_its_activated_ability_for_full_fidelity() {
    let griffin = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SCREECHING-GRIFFIN")
        .expect("Screeching Griffin definition exists");
    assert_eq!(griffin.colors, [Color::White].into_iter().collect());
    assert_eq!(
        griffin.card_types,
        [CardType::Creature].into_iter().collect()
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&griffin.id));
    assert!(
        griffin
            .supported_rules
            .contains(&"activated-prevent-target-blocking-source")
    );
    assert!(rav_activated_ability_bindings().iter().any(|binding| {
        binding.card_definition == "RAV-SCREECHING-GRIFFIN"
            && binding.ability.id == "prevent-target-blocking-griffin"
    }));
}

#[test]
fn screeching_griffin_activation_rejects_only_the_selected_creature_as_its_blocker() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    let griffin = game
        .put_on_battlefield(PlayerId(0), "RAV-SCREECHING-GRIFFIN")
        .expect("Griffin enters");
    let blocker = game
        .put_on_battlefield(PlayerId(1), "RAV-COURIER-HAWK")
        .expect("flying blocker enters");
    let mountain = game
        .put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN")
        .expect("Mountain enters");
    game.set_entered_turn_for_setup(griffin, 0)
        .expect("Griffin has prior-turn provenance");
    game.set_entered_turn_for_setup(blocker, 0)
        .expect("blocker has prior-turn provenance");
    game.set_entered_turn_for_setup(mountain, 0)
        .expect("Mountain has prior-turn provenance");
    game.begin_game().expect("game starts");
    for player in [PlayerId(0), PlayerId(1), PlayerId(0), PlayerId(1)] {
        game.pass_priority(player)
            .expect("advance to precombat main");
    }
    game.activate_mana_ability(PlayerId(0), mountain, Color::Red)
        .expect("activation mana");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: griffin,
            ability_id: "prevent-target-blocking-griffin",
            sacrifice_sources: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(blocker)],
        },
    )
    .expect("Griffin activation");
    game.pass_priority(PlayerId(0)).expect("activation pass");
    game.pass_priority(PlayerId(1))
        .expect("activation resolves");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectCreated { source, target, .. }
            if *source == griffin && *target == blocker
    )));
    assert!(
        !game
            .characteristics(blocker)
            .expect("blocker characteristics")
            .keywords
            .contains(&Keyword::CannotAttackOrBlock)
    );

    for player in [PlayerId(0), PlayerId(1), PlayerId(0), PlayerId(1)] {
        game.pass_priority(player)
            .expect("advance through beginning of combat");
    }
    game.declare_attackers(PlayerId(0), &[griffin])
        .expect("Griffin attacks");
    game.pass_priority(PlayerId(0)).expect("attack pass");
    game.pass_priority(PlayerId(1)).expect("enter blockers");
    let result = game.declare_blockers(
        PlayerId(1),
        &[CombatBlock {
            attacker: griffin,
            blocker,
        }],
    );
    assert!(result.is_err(), "selected creature must not block Griffin");
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::BlockersDeclared { .. }))
    );
}
