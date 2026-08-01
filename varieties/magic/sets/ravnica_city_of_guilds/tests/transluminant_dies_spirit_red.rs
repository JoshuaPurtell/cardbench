//! Red discovery regression for Transluminant's unported dies trigger.

use cardbench_magic_engine::{
    ContinuousChange, Duration, Effect, Game, GameEvent, ManaCost, PlayerId,
};
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};

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
fn transluminant_requires_a_dies_trigger_that_creates_one_token() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-TRANSLUMINANT")
        .expect("Transluminant definition exists");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(1, [cardbench_magic_engine::Color::Green])
    );
    let binding = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == "RAV-TRANSLUMINANT"
                && binding.ability.id == "dies-create-flying-spirit"
        })
        .expect("Transluminant dies-token binding exists");
    assert_eq!(binding.ability.effects.len(), 1);
    assert!(matches!(
        binding.ability.effects.as_slice(),
        [Effect::CreateToken { count: 1, .. }]
    ));

    let mut game = game_with_rav_bindings();
    let transluminant = game
        .put_on_battlefield(PlayerId(0), "RAV-TRANSLUMINANT")
        .expect("Transluminant begins on the battlefield");
    game.begin_game().expect("fixture begins game");
    game.add_continuous_effect(
        transluminant,
        transluminant,
        ContinuousChange::ModifyPowerToughness {
            power: -2,
            toughness: -2,
        },
        Duration::EndOfTurn(1),
    )
    .expect("SBA makes Transluminant die");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == transluminant && *ability == "dies-create-flying-spirit"
    )));
    let first = game.priority;
    game.pass_priority(first).expect("first trigger pass");
    let second = game.priority;
    game.pass_priority(second).expect("second trigger pass");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TokenCreated { player, .. } if *player == PlayerId(0)
    )));
    game.validate_invariants()
        .expect("Transluminant trigger trace preserves invariants");
}
