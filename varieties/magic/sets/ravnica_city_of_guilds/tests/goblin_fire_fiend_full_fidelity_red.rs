//! Red milestone for the remaining Goblin Fire Fiend rules.

use cardbench_magic_engine::{AbilityActivation, Color, Game, GameEvent, PlayerId};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

#[test]
fn goblin_fire_fiend_requires_its_omitted_behaviors_for_full_fidelity() {
    let fiend = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GOBLIN-FIRE-FIEND")
        .expect("Goblin Fire Fiend definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&fiend.id));
}

#[test]
fn goblin_fire_fiend_declares_must_block_and_pump_rules() {
    let fiend = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GOBLIN-FIRE-FIEND")
        .expect("Goblin Fire Fiend definition exists");
    assert!(fiend.supported_rules.contains(&"must-block-if-able"));
    assert!(fiend.supported_rules.contains(&"activated-plus-one-power"));
}

#[test]
fn goblin_fire_fiend_activated_pump_uses_stack_and_expires() {
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    let fiend = game
        .put_on_battlefield(PlayerId(0), "RAV-GOBLIN-FIRE-FIEND")
        .expect("fiend enters");
    game.set_entered_turn_for_setup(fiend, 0)
        .expect("old fixture entry");
    game.grant_mana(PlayerId(0), Color::Red, 1)
        .expect("red mana");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: fiend,
            ability_id: "pump-plus-one-power",
            sacrifice_sources: vec![],
            targets: vec![],
        },
    )
    .expect("ability activation");
    assert_eq!(game.priority, PlayerId(0));
    game.pass_priority(PlayerId(0)).expect("activator passes");
    game.pass_priority(PlayerId(1)).expect("ability resolves");
    assert_eq!(
        game.characteristics(fiend).expect("fiend chars").power,
        Some(2)
    );
    assert!(
        game.event_log
            .iter()
            .any(|event| matches!(event, GameEvent::AbilityActivated { .. }))
    );
    let starting_turn = game.turn;
    while game.turn == starting_turn {
        match game.step {
            cardbench_magic_engine::Step::DeclareAttackers => {
                if game
                    .view_for_player(PlayerId(0))
                    .expect("combat view")
                    .attackers_declared
                {
                    let priority = game.priority;
                    game.pass_priority(priority)
                        .expect("advance after attackers");
                } else {
                    game.declare_attackers(PlayerId(0), &[])
                        .expect("empty attackers");
                }
            }
            cardbench_magic_engine::Step::DeclareBlockers => {
                game.declare_blockers(PlayerId(1), &[])
                    .expect("empty blockers");
            }
            _ => {
                let priority = game.priority;
                game.pass_priority(priority).expect("advance priority");
            }
        }
    }
    assert_eq!(
        game.characteristics(fiend).expect("fiend chars").power,
        Some(1)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectExpired { target, .. } if *target == fiend
    )));
    game.validate_invariants().expect("ability trace is valid");
}
