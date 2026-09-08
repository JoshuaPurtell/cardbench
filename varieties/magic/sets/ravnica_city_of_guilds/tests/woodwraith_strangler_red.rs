//! Red discovery contract for Woodwraith Strangler's graveyard-exile regeneration.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, AbilityCostPayment, CardType, Color, Game, GameEvent,
    GeneralizedAbilityActivation, ManaCost, PlayerId, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings,
    rav_generalized_activated_ability_cost_bindings, rav_mana_ability_bindings,
};

#[test]
fn woodwraith_strangler_requires_graveyard_exile_regeneration() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-WOODWRAITH-STRANGLER")
        .expect("Woodwraith Strangler definition exists");

    assert_eq!(definition.name, "Woodwraith Strangler");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(2, [Color::Black, Color::Green])
    );
    assert_eq!(
        definition.colors,
        BTreeSet::from([Color::Black, Color::Green])
    );
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(2), Some(2)));
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "Woodwraith Strangler cannot be complete while its graveyard-exile regeneration is absent"
    );
    assert!(
        definition
            .supported_rules
            .contains(&"exile-controller-graveyard-creature-card-regenerate-source"),
        "Woodwraith Strangler must expose its graveyard-exile regeneration cost"
    );
}

#[test]
fn woodwraith_strangler_exiles_the_selected_creature_card_before_regenerating() {
    let controller = PlayerId(0);
    let mut game = Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds");
    game.register_generalized_activated_ability_cost_bindings(
        rav_generalized_activated_ability_cost_bindings(),
    )
    .expect("graveyard-exile cost binding registers");
    let strangler = game
        .put_on_battlefield(controller, "RAV-WOODWRAITH-STRANGLER")
        .expect("Strangler begins on the battlefield");
    let creature_cost = game
        .add_card(controller, "RAV-WATCHWOLF", Zone::Graveyard)
        .expect("creature cost begins in controller graveyard");
    game.begin_game().expect("game begins");

    game.activate_ability_with_generalized_costs(
        controller,
        GeneralizedAbilityActivation {
            activation: AbilityActivation {
                source: strangler,
                ability_id: "exile-creature-card-regenerate-source",
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![],
            },
            cost_payment: AbilityCostPayment {
                graveyard_cards_to_exile: vec![creature_cost],
                ..AbilityCostPayment::default()
            },
            mana_payment_selection: None,
        },
    )
    .expect("selected creature card pays the activation cost");
    assert_eq!(game.zone_of(creature_cost), Some(Zone::Exile));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ExiledFromGraveyardAsAbilityCost { player, source, card }
            if *player == controller && *source == strangler && *card == creature_cost
    )));
    game.pass_priority(controller).expect("controller passes");
    game.pass_priority(PlayerId(1))
        .expect("opponent passes and regeneration resolves");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::RegenerationShieldCreated { source, target }
            if *source == strangler && *target == strangler
    )));
    eprintln!(
        "Woodwraith Strangler trace={:?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("graveyard-exile regeneration preserves invariants");
}
