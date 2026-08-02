//! Red regression for Woebringer Demon's each-upkeep sacrifice trigger.
//!
//! The creature is controlled by player zero, but the effect belongs to the
//! player whose upkeep began.  The trigger must therefore be placed on the
//! ordinary stack before that upkeep's first priority instead of being
//! approximated as a controller-only static ability.

use cardbench_magic_engine::{Game, GameEvent, PlayerId};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

fn game() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV fixture builds")
}

#[test]
fn woebringer_demon_stacks_an_active_player_sacrifice_on_its_controllers_upkeep() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-WOEBRINGER-DEMON")
        .expect("Woebringer Demon definition exists");
    let mut game = game();
    let demon = game
        .put_on_battlefield(PlayerId(0), definition.id)
        .expect("demon starts on the battlefield");
    game.put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("active player has a legal creature to sacrifice");
    game.begin_game().expect("fixture enters first upkeep");

    println!("Woebringer red trace: {:#?}", game.canonical_event_log());
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == demon && *ability == "each-upkeep-active-player-sacrifice-creature"
    )));
    assert!(
        definition
            .supported_rules
            .contains(&"each-upkeep-active-player-sacrifice-creature"),
        "the card definition must not claim that a controller-only sacrifice is enough"
    );
}
