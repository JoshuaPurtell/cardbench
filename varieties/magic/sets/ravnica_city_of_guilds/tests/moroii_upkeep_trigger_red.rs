//! Red regression for a beginning-of-upkeep triggered ability.

use cardbench_magic_engine::{Game, GameEvent, PlayerId, Step, Zone};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

#[test]
fn moroii_stacks_and_resolves_its_upkeep_life_loss() {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV game builds");
    let moroii = game
        .add_card(PlayerId(0), "RAV-MOROII", Zone::Battlefield)
        .expect("Moroii begins on the battlefield");
    game.set_entered_turn_for_setup(moroii, 0)
        .expect("Moroii predates the measured turn");

    game.begin_game().expect("game starts");

    assert_eq!(game.step, Step::Upkeep);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == moroii && *ability == "upkeep-lose-one-life"
    )));
    assert_eq!(game.stack.len(), 1, "upkeep trigger should be on the stack");

    game.pass_priority(PlayerId(0))
        .expect("controller passes upkeep trigger");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes upkeep trigger");

    assert_eq!(
        game.player(PlayerId(0)).expect("controller exists").life,
        19
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LifeLost { source, player, amount }
            if *source == moroii && *player == PlayerId(0) && *amount == 1
    )));
}
