use cardbench_magic_engine::{CardType, ManaCost};
use cardbench_magic_rav::{card_definitions, rav_mana_ability_bindings, rav_triggered_ability_bindings};

const BOUNCE_LANDS: [(&str, &str, &str); 4] = [
    ("RAV-BOROS-GARRISON", "Boros Garrison", "boros-garrison-rw"),
    ("RAV-DIMIR-AQUEDUCT", "Dimir Aqueduct", "dimir-aqueduct-ub"),
    ("RAV-GOLGARI-ROT-FARM", "Golgari Rot Farm", "golgari-rot-farm-bg"),
    (
        "RAV-SELESNYA-SANCTUARY",
        "Selesnya Sanctuary",
        "selesnya-sanctuary-gw",
    ),
];

#[test]
fn guild_bounce_lands_require_tapped_entry_return_trigger_and_two_mana_binding() {
    let definitions = card_definitions();
    let mana = rav_mana_ability_bindings();
    let triggers = rav_triggered_ability_bindings();
    for (id, name, mana_ability) in BOUNCE_LANDS {
        let definition = definitions
            .iter()
            .find(|definition| definition.id == id)
            .unwrap_or_else(|| panic!("{name} definition exists"));
        assert_eq!(definition.mana_cost, ManaCost::new(0), "{name}");
        assert_eq!(definition.card_types, [CardType::Land].into(), "{name}");
        assert!(
            mana.iter().any(|binding| {
                binding.card_definition == id && binding.ability.id == mana_ability
            }),
            "{name} has its two-mana activation"
        );
        assert!(
            triggers.iter().any(|binding| {
                binding.card_definition == id && binding.ability.id == "return-controlled-land"
            }),
            "{name} has a stack-backed return trigger"
        );
    }
}
