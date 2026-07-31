use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, CombatBlock, Game, GameEvent, PlayerId, Step};
use cardbench_magic_rav::card_definitions;

#[test]
fn sabertooth_alley_cat_is_executable_with_its_mountain_block_condition() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SABERTOOTH-ALLEY-CAT")
        .expect("Sabertooth Alley Cat exists");
    assert_eq!(definition.name, "Sabertooth Alley Cat");
    assert_eq!(definition.colors, BTreeSet::from([Color::Red]));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(2), Some(1)));
    assert!(
        definition
            .supported_rules
            .contains(&"mountain-required-to-block")
    );
}

#[test]
fn sabertooth_alley_cat_rejects_blocking_without_a_controlled_mountain() {
    let mut game = Game::new(card_definitions(), 2).expect("RAV game builds");
    let attacker = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("attacker enters");
    let cat = game
        .put_on_battlefield(PlayerId(1), "RAV-SABERTOOTH-ALLEY-CAT")
        .expect("Cat enters");
    game.set_entered_turn_for_setup(attacker, 0)
        .expect("old attacker");
    game.set_entered_turn_for_setup(cat, 0).expect("old Cat");
    game.begin_game().expect("game starts");
    while game.step != Step::DeclareAttackers {
        let priority = game.priority;
        game.pass_priority(priority).expect("advance to combat");
        game.pass_priority(PlayerId(1 - priority.0))
            .expect("pass turn");
    }
    game.declare_attackers(PlayerId(0), &[attacker])
        .expect("attacker declares");
    game.pass_priority(PlayerId(0)).expect("attacker passes");
    game.pass_priority(PlayerId(1))
        .expect("advance to blockers");
    let before = game.event_log.clone();
    assert!(
        game.declare_blockers(
            PlayerId(1),
            &[CombatBlock {
                attacker,
                blocker: cat,
            }],
        )
        .is_err()
    );
    assert_eq!(game.event_log, before);
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::BlockersDeclared { .. }))
    );
}
