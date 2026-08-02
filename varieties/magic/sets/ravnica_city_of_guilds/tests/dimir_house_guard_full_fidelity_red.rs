//! Red discovery regression for Dimir House Guard's omitted regeneration activation.

use cardbench_magic_engine::{
    AbilityActivation, Effect, Game, GameEvent, ManaCost, PlayerId, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
};

fn game_with_rav_bindings() -> Game {
    Game::new_with_all_bindings(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .expect("RAV game builds")
}

#[test]
fn dimir_house_guard_sacrifices_a_controlled_creature_for_a_regeneration_shield() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-DIMIR-HOUSE-GUARD")
        .expect("Dimir House Guard definition exists");
    assert!(
        RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id),
        "Dimir House Guard cannot be full fidelity until its creature-sacrifice regeneration is bound"
    );
    assert_eq!(
        definition.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "fear",
            "immediate-hand-zone-transmute-compatibility",
            "sacrifice-creature-regenerate",
        ]
    );

    let binding = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == definition.id)
        .expect("Dimir House Guard regeneration binding exists");
    assert_eq!(binding.ability.id, "sacrifice-creature-regenerate");
    assert_eq!(binding.ability.mana_cost, ManaCost::new(0));
    assert!(!binding.ability.tap_cost);
    assert_eq!(binding.ability.sacrifice_creatures, 1);
    assert!(binding.ability.targets.is_empty());
    assert_eq!(binding.ability.effects, [Effect::RegenerateSource]);

    let mut game = game_with_rav_bindings();
    let guard = game
        .put_on_battlefield(PlayerId(0), definition.id)
        .expect("House Guard begins on battlefield");
    let offering = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .expect("controlled offering begins on battlefield");
    game.begin_game().expect("fixture begins game");
    game.clear_event_log();

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: guard,
            ability_id: "sacrifice-creature-regenerate",
            sacrifice_sources: vec![offering],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("House Guard accepts one selected controlled creature sacrifice");

    assert_eq!(game.zone_of(offering), Some(Zone::Graveyard));
    let sacrifice_index = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::SacrificedAsAbilityCost { source, permanent, .. }
                    if *source == guard && *permanent == offering
            )
        })
        .expect("sacrifice-cost receipt exists");
    let activation_index = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::AbilityActivated { source, ability, .. }
                    if *source == guard && *ability == "sacrifice-creature-regenerate"
            )
        })
        .expect("activation receipt exists");
    assert!(
        sacrifice_index < activation_index,
        "the creature cost must be paid before the ability is stacked"
    );

    let first = game.priority;
    game.pass_priority(first).expect("controller passes");
    let second = game.priority;
    game.pass_priority(second)
        .expect("opponent resolves ability");
    println!(
        "Dimir House Guard full-fidelity trace: {:?}",
        game.canonical_event_log()
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::RegenerationShieldCreated { source, target }
            if *source == guard && *target == guard
    )));
    game.validate_invariants()
        .expect("House Guard sacrifice and regeneration preserve invariants");
}
