//! Complete public RAV contract for Votary of the Conclave.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn votary_represents_its_complete_vigilance_creature_behavior() {
    let votary = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-VOTARY-OF-THE-CONCLAVE")
        .expect("Votary of the Conclave definition");

    assert_eq!(votary.name, "Votary of the Conclave");
    assert_eq!(votary.mana_cost, ManaCost::with_colors(0, [Color::White]));
    assert_eq!(votary.colors, BTreeSet::from([Color::White]));
    assert_eq!(votary.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((votary.power, votary.toughness), (Some(1), Some(1)));
    assert_eq!(votary.keywords, [Keyword::Vigilance]);
    assert!(votary.effects.is_empty());
    assert_eq!(
        votary.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "vigilance",
        ]
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&votary.id));
}

#[test]
fn votary_public_trace_preserves_its_untapped_state_when_attacking() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_votary_of_the_conclave_vigilance_attack")
        .expect("Votary of the Conclave public trace");

    println!("Votary vigilance trace: {:?}", trace.event_log);
    assert_eq!(trace.digest, "fnv1a64:2b40464c0df717b5");
    assert!(
        trace
            .event_log
            .iter()
            .any(|event| event.contains("AttackersDeclared"))
    );
    assert!(
        !trace
            .event_log
            .iter()
            .any(|event| event.contains("attacker: ObjectId(1), tapped: true")),
        "Vigilance must preserve Votary's untapped state during declaration"
    );
}
