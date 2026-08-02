//! Full-fidelity contracts for Vitu-Ghazi's typed mana and Saproling creation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, CreatureSubtype, Effect, Game, GameEvent,
    ManaAbilityActivation, ManaCost, PlayerId,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
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
fn vitu_ghazi_has_exact_definition_and_typed_bindings() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-VITU-GHAZI")
        .expect("Vitu-Ghazi definition");
    assert_eq!(
        executable_definition_id_for_collector(285),
        Ok("RAV-VITU-GHAZI")
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Land]));
    assert_eq!(definition.colors, BTreeSet::new());
    assert_eq!(definition.mana_colors, BTreeSet::from([Color::Colorless]));
    assert_eq!(
        definition.supported_rules,
        [
            "full-rules-fidelity",
            "colorless-mana-ability",
            "activated-green-saproling-token",
        ]
    );

    let mana = rav_mana_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == definition.id)
        .expect("colorless mana binding");
    assert_eq!(mana.ability.id, "produce-colorless");
    assert!(mana.ability.tap_cost);
    assert_eq!(mana.ability.amount, 1);

    let token = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| {
            binding.card_definition == definition.id
                && binding.ability.id == "create-green-saproling"
        })
        .expect("green Saproling binding");
    assert_eq!(
        token.ability.mana_cost,
        ManaCost::with_colors(2, [Color::Green, Color::White])
    );
    assert!(token.ability.tap_cost);
    assert_eq!(
        token.ability.effects,
        [Effect::CreateToken {
            token: cardbench_magic_engine::TokenSpec::saproling(),
            count: 1,
        }]
    );
}

#[test]
#[allow(clippy::too_many_lines)] // The complete activation trace is intentionally inspected in one regression.
fn vitu_ghazi_mana_pays_its_stack_backed_saproling_activation() {
    let mut game = game_with_rav_bindings();
    let mana_land = game
        .put_on_battlefield(PlayerId(0), "RAV-VITU-GHAZI")
        .expect("mana Vitu-Ghazi starts on battlefield");
    let token_land = game
        .put_on_battlefield(PlayerId(0), "RAV-VITU-GHAZI")
        .expect("token Vitu-Ghazi starts on battlefield");
    let first_forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("first Forest starts on battlefield");
    let second_forest = game
        .put_on_battlefield(PlayerId(0), "RAV-FOREST")
        .expect("second Forest starts on battlefield");
    let plains = game
        .put_on_battlefield(PlayerId(0), "RAV-PLAINS")
        .expect("Plains starts on battlefield");
    game.begin_game().expect("game starts");
    game.clear_event_log();

    game.activate_bound_mana_ability(
        PlayerId(0),
        ManaAbilityActivation {
            source: mana_land,
            ability_id: "produce-colorless",
            chosen_color: None,
        },
    )
    .expect("Vitu-Ghazi produces colorless mana");
    for forest in [first_forest, second_forest] {
        game.activate_mana_ability(PlayerId(0), forest, Color::Green)
            .expect("Forest produces green mana");
    }
    game.activate_mana_ability(PlayerId(0), plains, Color::White)
        .expect("Plains produces white mana");
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: token_land,
            ability_id: "create-green-saproling",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Saproling activation enters the stack");
    game.pass_priority(PlayerId(0))
        .expect("activator passes to resolve Vitu-Ghazi");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes to resolve Vitu-Ghazi");

    let tokens = game
        .player(PlayerId(0))
        .expect("controller exists")
        .battlefield
        .iter()
        .copied()
        .filter(|object| {
            game.object(*object)
                .is_ok_and(|object| object.token.is_some())
        })
        .collect::<Vec<_>>();
    assert_eq!(tokens.len(), 1);
    let characteristics = game
        .characteristics(tokens[0])
        .expect("Saproling characteristics exist");
    assert_eq!(characteristics.colors, BTreeSet::from([Color::Green]));
    assert_eq!(
        characteristics.card_types,
        BTreeSet::from([CardType::Creature])
    );
    assert_eq!(
        characteristics.creature_subtypes,
        BTreeSet::from([CreatureSubtype::Saproling])
    );
    assert_eq!(
        (characteristics.power, characteristics.toughness),
        (Some(1), Some(1))
    );
    assert!(game.object(mana_land).expect("mana land remains").tapped);
    assert!(game.object(token_land).expect("token land remains").tapped);
    assert_eq!(
        game.player(PlayerId(0))
            .expect("controller exists")
            .mana_pool
            .amount(Color::Colorless),
        0
    );
    println!(
        "Vitu-Ghazi activation trace: {:#?}",
        game.canonical_event_log()
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::BoundManaAbilityActivated { source, ability, color, amount, .. }
            if *source == mana_land && *ability == "produce-colorless" && *color == Color::Colorless && *amount == 1
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == token_land && *ability == "create-green-saproling"
    )));
    game.validate_invariants()
        .expect("Vitu-Ghazi activation preserves invariants");
}
