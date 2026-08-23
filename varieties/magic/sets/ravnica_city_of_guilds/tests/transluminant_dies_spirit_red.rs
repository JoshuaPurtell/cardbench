//! Red discovery regression for Transluminant's unported dies trigger.

use cardbench_magic_engine::{
    Color, ContinuousChange, CreatureSubtype, Duration, Effect, Game, GameEvent, Keyword, ManaCost,
    PlayerId,
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
        ManaCost::with_colors(1, [Color::Green])
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
    let spirit = game
        .player(PlayerId(0))
        .expect("controller exists")
        .battlefield
        .iter()
        .copied()
        .find(|card| {
            game.object(*card)
                .expect("battlefield object exists")
                .token
                .is_some()
        })
        .expect("dies trigger creates a Spirit token");
    let spirit_characteristics = game.characteristics(spirit).expect("Spirit is live");
    assert_eq!(
        spirit_characteristics.colors,
        [Color::White].into_iter().collect()
    );
    assert_eq!(
        spirit_characteristics.creature_subtypes,
        [CreatureSubtype::Spirit].into_iter().collect()
    );
    assert_eq!(
        (
            spirit_characteristics.power,
            spirit_characteristics.toughness
        ),
        (Some(1), Some(1))
    );
    assert!(spirit_characteristics.keywords.contains(&Keyword::Flying));
    println!("transluminant_event_log={:?}", game.canonical_event_log());
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TokenCreated { player, .. } if *player == PlayerId(0)
    )));
    game.validate_invariants()
        .expect("Transluminant trigger trace preserves invariants");
}
