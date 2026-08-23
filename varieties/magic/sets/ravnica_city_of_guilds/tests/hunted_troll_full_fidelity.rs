//! Complete public contract for Hunted Troll's token and regeneration rules.

use std::collections::BTreeSet;

use cardbench_magic_engine::{Color, CreatureSubtype, Effect, Keyword, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_triggered_ability_bindings, run_all_scenarios,
};

#[test]
fn hunted_troll_has_exact_typed_faerie_and_regeneration_bindings() {
    let troll = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-HUNTED-TROLL")
        .expect("Hunted Troll definition exists");
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&troll.id));
    assert_eq!(
        troll.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "etb-targeted-opponent-flying-faerie-tokens",
            "regeneration",
        ]
    );

    let trigger = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == troll.id)
        .expect("Hunted Troll entry trigger exists");
    assert_eq!(trigger.ability.id, "etb-opponent-faeries");
    let [Effect::CreateTokenForTargetPlayer { token, count }] = trigger.ability.effects.as_slice()
    else {
        panic!("Hunted Troll ETB must create one typed token batch");
    };
    assert_eq!(*count, 4);
    assert_eq!(token.name, "Faerie");
    assert_eq!(token.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(
        token.creature_subtypes,
        BTreeSet::from([CreatureSubtype::Faerie])
    );
    assert_eq!(token.keywords, [Keyword::Flying]);
    assert_eq!((token.power, token.toughness), (1, 1));

    let regeneration = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == troll.id)
        .expect("Hunted Troll regeneration exists");
    assert_eq!(regeneration.ability.id, "self-regeneration");
    assert_eq!(
        regeneration.ability.mana_cost,
        ManaCost::with_colors(0, [Color::Green])
    );
    assert_eq!(regeneration.ability.effects, [Effect::RegenerateSource]);
}

#[test]
fn hunted_troll_public_scenario_resolves_four_faeries_and_a_regeneration_shield() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_hunted_troll_faeries_and_regeneration")
        .expect("Hunted Troll public scenario exists");
    println!("Hunted Troll full trace: {:?}", trace.event_log);
    assert_eq!(trace.digest, "fnv1a64:ba8b1fd744f163a2");
    assert_eq!(
        trace
            .event_log
            .iter()
            .filter(|event| event.contains("TokenCreated"))
            .count(),
        4
    );
    assert!(
        trace
            .event_log
            .iter()
            .any(|event| event.contains("RegenerationShieldCreated"))
    );
    assert!(
        trace
            .event_log
            .iter()
            .any(|event| event.contains("AbilityResolved"))
    );
}
