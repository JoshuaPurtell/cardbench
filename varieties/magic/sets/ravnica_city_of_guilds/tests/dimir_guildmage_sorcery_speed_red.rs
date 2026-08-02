//! Red regression for Dimir Guildmage's sorcery-speed discard restriction.

use cardbench_magic_engine::{AbilityActivation, Color, Game, PlayerId, RulesError, Target};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

fn guildmage_game() -> Game {
    Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV fixture constructs")
}

#[test]
fn dimir_guildmage_discard_rejects_upkeep_activation_without_paying_or_stacking() {
    let controller = PlayerId(0);
    let mut game = guildmage_game();
    let guildmage = game
        .put_on_battlefield(controller, "RAV-DIMIR-GUILDMAGE")
        .expect("Guildmage setup");
    let swamps = (0..4)
        .map(|_| {
            game.put_on_battlefield(controller, "RAV-SWAMP")
                .expect("Swamp setup")
        })
        .collect::<Vec<_>>();

    game.begin_game().expect("game begins at upkeep");
    for swamp in swamps {
        game.activate_mana_ability(controller, swamp, Color::Black)
            .expect("live black mana");
    }
    let before_events = game.event_log.clone();
    assert_eq!(
        game.activate_ability(
            controller,
            AbilityActivation {
                source: guildmage,
                ability_id: "target-player-discard",
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![Target::Player(PlayerId(1))],
            },
        ),
        Err(RulesError::IllegalAction(
            "this activated ability is allowed only during your main phase with an empty stack"
        ))
    );
    assert!(
        game.stack.is_empty(),
        "rejected activation cannot reach the stack"
    );
    assert_eq!(game.event_log, before_events, "rejection is atomic");
    assert_eq!(
        game.player(controller)
            .expect("controller exists")
            .mana_pool
            .amount(Color::Black),
        4,
        "rejected activation cannot consume mana"
    );
    game.validate_invariants()
        .expect("rejected activation preserves invariants");
}
