//! Red discovery contract for Overgrown Tomb's optional entry-life replacement.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, Color, Game, GameEvent, ManaAbilityActivation, PlayerId, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_land_entry_bindings, rav_mana_ability_bindings,
    rav_static_continuous_effect_bindings, rav_triggered_ability_bindings,
};

fn tomb_game() -> Game {
    Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
        rav_static_continuous_effect_bindings(),
        rav_land_entry_bindings(),
    )
    .expect("RAV fixture constructs")
}

#[test]
fn overgrown_tomb_requires_its_exact_dual_land_entry_and_mana_contract() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-OVERGROWN-TOMB")
        .expect("Overgrown Tomb definition exists");
    assert_eq!(definition.name, "Overgrown Tomb");
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Land]));
    assert_eq!(definition.colors, BTreeSet::<Color>::new());
    assert_eq!(
        definition.mana_colors,
        BTreeSet::from([Color::Black, Color::Green])
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert_eq!(
        executable_definition_id_for_collector(279),
        Ok("RAV-OVERGROWN-TOMB")
    );
    assert!(rav_land_entry_bindings().iter().any(|binding| {
        binding.card_definition == definition.id
            && binding.enters_tapped == false
            && binding.optional_life_payment == Some(2)
    }));
    assert!(rav_mana_ability_bindings().iter().any(|binding| {
        binding.card_definition == definition.id
            && binding.ability.id == "produce-black-or-green"
            && binding.ability.tap_cost
    }));
}

#[test]
fn overgrown_tomb_explicitly_pays_two_life_to_enter_untapped_or_declines_and_enters_tapped() {
    let mut paid_game = tomb_game();
    let paid_tomb = paid_game
        .add_card(PlayerId(0), "RAV-OVERGROWN-TOMB", Zone::Hand)
        .expect("Overgrown Tomb hand fixture");
    paid_game
        .play_land_with_entry_life_payment(PlayerId(0), paid_tomb, true)
        .expect("controller may pay two life as Tomb enters");
    assert_eq!(paid_game.player(PlayerId(0)).expect("player").life, 18);
    assert!(!paid_game.object(paid_tomb).expect("Tomb persists").tapped);
    assert!(paid_game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LandEntryLifePaid { player, card, amount }
            if *player == PlayerId(0) && *card == paid_tomb && *amount == 2
    )));
    paid_game
        .activate_bound_mana_ability(
            PlayerId(0),
            ManaAbilityActivation {
                source: paid_tomb,
                ability_id: "produce-black-or-green",
                chosen_color: Some(Color::Green),
            },
        )
        .expect("untapped Tomb produces selected Green mana");
    assert_eq!(
        paid_game
            .player(PlayerId(0))
            .expect("player")
            .mana_pool
            .amount(Color::Green),
        1
    );
    paid_game
        .validate_invariants()
        .expect("paid Tomb entry is auditable");

    let mut declined_game = tomb_game();
    let declined_tomb = declined_game
        .add_card(PlayerId(0), "RAV-OVERGROWN-TOMB", Zone::Hand)
        .expect("Overgrown Tomb hand fixture");
    declined_game
        .play_land_with_entry_life_payment(PlayerId(0), declined_tomb, false)
        .expect("controller may decline Tomb life payment");
    assert_eq!(declined_game.player(PlayerId(0)).expect("player").life, 20);
    assert!(
        declined_game
            .object(declined_tomb)
            .expect("Tomb persists")
            .tapped
    );
    assert!(!declined_game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LandEntryLifePaid { card, .. } if *card == declined_tomb
    )));
    declined_game
        .validate_invariants()
        .expect("declined Tomb entry is auditable");
}
