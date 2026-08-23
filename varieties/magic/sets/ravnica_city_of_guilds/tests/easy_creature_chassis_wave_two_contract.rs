//! Public contract for the former low-complexity RAV creature-chassis wave.
//!
//! Its only member has graduated to a full-fidelity card. Keep this explicit so
//! a future catalog reconciliation cannot silently reintroduce the incorrect
//! Blue chassis or discard the stack-backed activation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
#[allow(clippy::too_many_lines)] // Declarative public fact table stays together for auditability.
fn easy_creature_wave_two_promotes_roofstalker_wight_without_a_stale_blue_chassis() {
    let definitions = card_definitions();
    let wight = definitions
        .iter()
        .find(|definition| definition.id == "RAV-ROOFSTALKER-WIGHT")
        .expect("Roofstalker Wight definition exists");
    assert_eq!(wight.name, "Roofstalker Wight");
    assert_eq!(wight.mana_cost, ManaCost::with_colors(1, [Color::Black]));
    assert_eq!(wight.colors, BTreeSet::from([Color::Black]));
    assert_eq!(wight.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!(wight.power, Some(2));
    assert_eq!(wight.toughness, Some(1));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&wight.id));
    assert!(
        wight
            .supported_rules
            .contains(&"self-flying-until-end-of-turn")
    );
    assert!(wight.keywords.is_empty());
    assert!(wight.effects.is_empty());

    let commando = definitions
        .iter()
        .find(|definition| definition.id == "RAV-ORDRUUN-COMMANDO")
        .expect("Ordruun Commando definition exists");
    assert_eq!(
        commando.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "activated-prevent-one-damage-to-self"
        ]
    );
}

#[test]
fn public_easy_creature_wave_two_scenarios_cover_casting_stack_zones_and_base_pt() {
    let scenarios = run_all_scenarios()
        .expect("RAV public scenarios run")
        .into_iter()
        .map(|scenario| scenario.id)
        .collect::<BTreeSet<_>>();
    for id in [
        "rav_easy_white_blue_creatures_wave_two",
        "rav_easy_blue_black_creatures_wave_two",
        "rav_easy_red_creatures_wave_two",
        "rav_easy_green_creatures_wave_two",
        "rav_easy_blue_red_creatures_wave_two",
    ] {
        assert!(scenarios.contains(id), "missing public scenario {id}");
    }
}
