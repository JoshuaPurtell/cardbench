//! Direct, ability-complete contract for RAV Guardian of Vitu-Ghazi.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardType, Color, Keyword, ManaCost};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
fn guardian_of_vitu_ghazi_has_complete_convoke_characteristics_and_vigilance_contract() {
    let definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == "RAV-GUARDIAN-OF-VITU-GHAZI")
        .expect("Guardian of Vitu-Ghazi definition exists");

    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&definition.id));
    assert_eq!(
        definition.supported_rules,
        [
            "full-rules-fidelity",
            "convoke",
            "base-characteristics",
            "vigilance",
        ]
    );
    assert_eq!(
        definition.mana_cost,
        ManaCost::with_colors(6, [Color::Green, Color::White])
    );
    assert_eq!(
        definition.colors,
        BTreeSet::from([Color::Green, Color::White])
    );
    assert_eq!(definition.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((definition.power, definition.toughness), (Some(4), Some(7)));
    assert_eq!(definition.keywords, [Keyword::Convoke, Keyword::Vigilance]);
    assert!(definition.effects.is_empty());
}

#[test]
fn guardian_public_traces_cover_convoke_then_vigilance_declaration() {
    let scenarios = run_all_scenarios().expect("public RAV scenarios run");
    let find = |id| {
        scenarios
            .iter()
            .find(|scenario| scenario.id == id)
            .unwrap_or_else(|| panic!("missing public scenario {id}"))
    };

    let convoke = find("rav_guardian_of_vitu_ghazi_convoke_slice");
    assert_eq!(convoke.digest, "fnv1a64:a68cdb671a037d94");
    assert_eq!(
        convoke
            .event_log
            .iter()
            .filter(|event| event.contains("ConvokeUsed"))
            .count(),
        6,
        "the exact generic portion of Guardian's cost receives six convoke receipts"
    );

    let vigilance = find("rav_guardian_of_vitu_ghazi_vigilance_attack");
    assert_eq!(vigilance.digest, "fnv1a64:2b40464c0df717b5");
    let declaration = vigilance
        .event_log
        .iter()
        .position(|event| {
            event == "AttackersDeclared { player: PlayerId(0), attackers: [ObjectId(1)] }"
        })
        .expect("vigilance trace records Guardian's attack declaration");
    assert_eq!(
        vigilance
            .event_log
            .get(declaration.checked_sub(1).expect("not first event")),
        Some(
            &"StepBegan { turn: 1, active_player: PlayerId(0), step: DeclareAttackers }".to_owned()
        ),
        "the declaration begins only after the turn machine enters its dedicated step"
    );
    assert_eq!(
        vigilance
            .event_log
            .get(declaration.checked_sub(2).expect("not second event")),
        Some(&"PriorityPassed { player: PlayerId(1) }".to_owned()),
        "both players passed through beginning of combat before the declaration step"
    );
    assert_eq!(declaration + 1, vigilance.event_log.len());
    assert!(
        !vigilance
            .event_log
            .iter()
            .any(|event| event.contains("PermanentsUntapped")),
        "vigilance keeps the attacker untapped at declaration rather than untapping it later"
    );
}
