//! Red discovery contract for RAV's three remaining typed dual shock lands.

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

struct ShockLandContract {
    id: &'static str,
    name: &'static str,
    collector: u16,
    colors: [Color; 2],
    ability: &'static str,
}

const SHOCK_LANDS: [ShockLandContract; 3] = [
    ShockLandContract {
        id: "RAV-SACRED-FOUNDRY",
        name: "Sacred Foundry",
        collector: 280,
        colors: [Color::White, Color::Red],
        ability: "produce-white-or-red",
    },
    ShockLandContract {
        id: "RAV-TEMPLE-GARDEN",
        name: "Temple Garden",
        collector: 284,
        colors: [Color::Green, Color::White],
        ability: "produce-green-or-white",
    },
    ShockLandContract {
        id: "RAV-WATERY-GRAVE",
        name: "Watery Grave",
        collector: 286,
        colors: [Color::Blue, Color::Black],
        ability: "produce-blue-or-black",
    },
];

fn shock_game() -> Game {
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
fn rav_shock_lands_share_overgrown_tombs_explicit_two_life_entry_contract() {
    let definitions = card_definitions();
    for land in SHOCK_LANDS {
        let definition = definitions
            .iter()
            .find(|definition| definition.id == land.id)
            .unwrap_or_else(|| panic!("{} definition exists", land.name));
        assert_eq!(definition.name, land.name);
        assert_eq!(definition.card_types, BTreeSet::from([CardType::Land]));
        assert_eq!(definition.colors, BTreeSet::<Color>::new());
        assert_eq!(definition.mana_colors, BTreeSet::from(land.colors));
        assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&land.id));
        assert_eq!(
            executable_definition_id_for_collector(land.collector),
            Ok(land.id)
        );
        assert!(rav_land_entry_bindings().iter().any(|binding| {
            binding.card_definition == land.id
                && !binding.enters_tapped
                && binding.optional_life_payment == Some(2)
        }));
        assert!(rav_mana_ability_bindings().iter().any(|binding| {
            binding.card_definition == land.id
                && binding.ability.id == land.ability
                && binding.ability.tap_cost
        }));
    }
}

#[test]
fn rav_shock_lands_pay_two_life_to_enter_untapped_or_decline_and_enter_tapped() {
    for land in SHOCK_LANDS {
        let mut paid_game = shock_game();
        let paid_land = paid_game
            .add_card(PlayerId(0), land.id, Zone::Hand)
            .unwrap_or_else(|_| panic!("{} hand fixture", land.name));
        paid_game
            .play_land_with_entry_life_payment(PlayerId(0), paid_land, true)
            .unwrap_or_else(|_| panic!("controller pays for {} entry", land.name));
        assert_eq!(paid_game.player(PlayerId(0)).expect("player").life, 18);
        assert!(!paid_game.object(paid_land).expect("land persists").tapped);
        assert!(paid_game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::LandEntryLifePaid { player, card, amount }
                if *player == PlayerId(0) && *card == paid_land && *amount == 2
        )));
        paid_game
            .activate_bound_mana_ability(
                PlayerId(0),
                ManaAbilityActivation {
                    source: paid_land,
                    ability_id: land.ability,
                    chosen_color: Some(land.colors[0]),
                },
            )
            .unwrap_or_else(|_| panic!("{} produces selected mana", land.name));
        assert_eq!(
            paid_game
                .player(PlayerId(0))
                .expect("player")
                .mana_pool
                .amount(land.colors[0]),
            1
        );
        paid_game
            .validate_invariants()
            .unwrap_or_else(|_| panic!("paid {} entry is auditable", land.name));

        let mut declined_game = shock_game();
        let declined_land = declined_game
            .add_card(PlayerId(0), land.id, Zone::Hand)
            .unwrap_or_else(|_| panic!("{} hand fixture", land.name));
        declined_game
            .play_land_with_entry_life_payment(PlayerId(0), declined_land, false)
            .unwrap_or_else(|_| panic!("controller declines {} entry payment", land.name));
        assert_eq!(declined_game.player(PlayerId(0)).expect("player").life, 20);
        assert!(declined_game.object(declined_land).expect("land persists").tapped);
        assert!(!declined_game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::LandEntryLifePaid { card, .. } if *card == declined_land
        )));
        declined_game
            .validate_invariants()
            .unwrap_or_else(|_| panic!("declined {} entry is auditable", land.name));
    }
}
