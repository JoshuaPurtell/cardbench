//! Red discovery contract for Master Warcraft's controller-chosen combat.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, HybridManaSymbol, ManaCost};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
};

#[test]
fn master_warcraft_requires_precombat_controller_chosen_attackers_and_blockers() {
    assert_eq!(
        executable_definition_id_for_collector(250),
        Ok("RAV-MASTER-WARCRAFT")
    );
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-MASTER-WARCRAFT")
        .expect("Master Warcraft definition exists");

    assert_eq!(definition.name, "Master Warcraft");
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_hybrid(
            2,
            [],
            [
                HybridManaSymbol {
                    first: Color::Red,
                    second: Color::White,
                },
                HybridManaSymbol {
                    first: Color::Red,
                    second: Color::White,
                },
            ],
        )
    );
    assert_eq!(
        definition.colors,
        BTreeSet::from([Color::Red, Color::White])
    );
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Instant]));
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    for rule in [
        "hybrid-cost-casting",
        "cast-only-before-attackers-declared",
        "controller-chooses-all-attackers-this-turn",
        "controller-chooses-all-blockers-and-assignments-this-turn",
    ] {
        assert!(definition.supported_rules.contains(&rule), "missing {rule}");
    }
}
