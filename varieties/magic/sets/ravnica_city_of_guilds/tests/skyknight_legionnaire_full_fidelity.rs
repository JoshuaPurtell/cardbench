//! Complete public RAV contract for Skyknight Legionnaire.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn skyknight_legionnaire_represents_its_complete_keyword_creature_behavior() {
    let legionnaire = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-SKYKNIGHT-LEGIONNAIRE")
        .expect("Skyknight Legionnaire definition");
    assert_eq!(legionnaire.name, "Skyknight Legionnaire");
    assert_eq!(
        legionnaire.mana_cost,
        ManaCost::with_colors(1, [Color::Red, Color::White])
    );
    assert_eq!(
        legionnaire.colors,
        BTreeSet::from([Color::Red, Color::White])
    );
    assert_eq!(legionnaire.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!(
        (legionnaire.power, legionnaire.toughness),
        (Some(2), Some(2))
    );
    assert_eq!(legionnaire.keywords, [Keyword::Flying, Keyword::Haste]);
    assert!(legionnaire.effects.is_empty());
    assert_eq!(
        legionnaire.supported_rules,
        [
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "haste",
        ]
    );
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&legionnaire.id));
}

#[test]
fn skyknight_legionnaire_public_trace_covers_cast_flying_haste_attack() {
    let trace = run_all_scenarios()
        .expect("shown RAV scenarios run")
        .into_iter()
        .find(|scenario| scenario.id == "rav_skyknight_legionnaire_flying_haste_attack")
        .expect("Skyknight Legionnaire public trace");
    println!("Skyknight Legionnaire trace: {:?}", trace.event_log);
    for marker in [
        "CastPaymentBasicLandManaAbilityActivated",
        "SpellCast",
        "SpellResolved",
        "AttackersDeclared",
    ] {
        assert!(
            trace.event_log.iter().any(|event| event.contains(marker)),
            "Skyknight trace lacks {marker}: {:?}",
            trace.event_log
        );
    }
    assert!(
        trace
            .event_log
            .iter()
            .any(|event| event
                == "AttackersDeclared { player: PlayerId(0), attackers: [ObjectId(4)] }"),
        "the cast creature must be the same-turn declared attacker: {:?}",
        trace.event_log
    );
}
