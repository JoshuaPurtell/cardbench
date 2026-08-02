//! Red discovery contract for Necroplasm's two beginning-of-upkeep abilities.
//!
//! Necroplasm's controller orders its simultaneous counter and mana-value
//! sweep triggers.  The sweep therefore has to read its source's current
//! `+1/+1` counter quantity when it resolves, rather than being a
//! card-specific precomputed value.

use cardbench_magic_engine::{CounterKind, Effect, Game, ManaCost, PlayerId};
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
fn necroplasm_requires_both_upkeep_bindings_and_counter_sweep_contract() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-NECROPLASM")
        .expect("Necroplasm definition exists");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(
            1,
            [
                cardbench_magic_engine::Color::Black,
                cardbench_magic_engine::Color::Black
            ]
        )
    );
    assert!(
        definition
            .supported_rules
            .contains(&"upkeep-add-plus-one-counter"),
        "Necroplasm must expose its first beginning-of-upkeep ability"
    );
    assert!(
        definition
            .supported_rules
            .contains(&"upkeep-destroy-creatures-by-plus-one-counter-mana-value"),
        "Necroplasm must expose its second beginning-of-upkeep ability"
    );

    let bindings = rav_triggered_ability_bindings()
        .into_iter()
        .filter(|binding| binding.card_definition == definition.id)
        .map(|binding| binding.ability)
        .collect::<Vec<_>>();
    assert_eq!(bindings.len(), 2, "both simultaneous upkeep triggers bind");
    assert!(bindings.iter().any(|ability| {
        ability.id == "upkeep-add-plus-one-counter"
            && ability.effects
                == [Effect::AddCountersToSource {
                    counter: CounterKind::PlusOnePlusOne,
                    amount: 1,
                }]
    }));

    let mut game = game();
    game.put_on_battlefield(PlayerId(0), definition.id)
        .expect("Necroplasm starts on battlefield");
    game.begin_game()
        .expect("game reaches the first upkeep trigger boundary");
    assert!(
        game.view_for_player(PlayerId(0))
            .expect("controller view is available")
            .pending_decision
            .is_some(),
        "the controller must order both simultaneous upkeep triggers before priority"
    );
}
