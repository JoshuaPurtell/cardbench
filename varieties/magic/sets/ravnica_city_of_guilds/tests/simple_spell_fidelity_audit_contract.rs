//! Public fail-closed audit for simple executable RAV instants and sorceries.
//!
//! The tests use `CardBench` semantic identifiers and receipt names, never card
//! rules text, images, or upstream card records.

use cardbench_magic_engine::{Effect, TokenSpec};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

fn definition(id: &str) -> cardbench_magic_engine::CardDefinition {
    card_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
        .unwrap_or_else(|| panic!("missing RAV definition {id}"))
}

#[test]
fn audited_simple_spells_keep_their_precise_compatibility_boundaries() {
    let expected = [
        ("RAV-DRYADS-CARESS", &["controller-life-gain"] as &[_]),
        (
            "RAV-MUDDLE-THE-MIXTURE",
            &[
                "full-rules-fidelity",
                "counter-target-instant-or-sorcery-spell",
                "transmute",
            ] as &[_],
        ),
        (
            "RAV-DIZZY-SPELL",
            &[
                "full-rules-fidelity",
                "targeted-layer-7-modifier",
                "transmute",
            ] as &[_],
        ),
    ];
    for (id, supported_rules) in expected {
        let spell = definition(id);
        assert_eq!(spell.supported_rules, supported_rules, "{id}");
        if !matches!(id, "RAV-DIZZY-SPELL" | "RAV-MUDDLE-THE-MIXTURE") {
            assert!(
                !RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&id),
                "{id} has an explicit unsupported functional rule and must not be promoted"
            );
        }
    }

    assert_eq!(
        definition("RAV-SCATTER-THE-SEEDS").effects,
        vec![Effect::CreateToken {
            token: TokenSpec::saproling(),
            count: 3,
        }]
    );
    assert_eq!(
        definition("RAV-DIZZY-SPELL").effects,
        vec![Effect::ModifyTargetPtUntilEndOfTurn {
            power: -3,
            toughness: 0,
        }]
    );
    assert_eq!(
        definition("RAV-MUDDLE-THE-MIXTURE").effects,
        vec![Effect::CounterTargetInstantOrSorcerySpell]
    );
}

#[test]
fn audited_spell_scenarios_retain_only_their_supported_public_receipts() {
    let results = run_all_scenarios().expect("public RAV scenarios run");
    for (id, receipts) in [
        (
            "rav_convoke_scatter_the_seeds",
            ["ConvokeUsed", "TokenCreated"].as_slice(),
        ),
        ("rav_dryads_caress_life", ["LifeGained"].as_slice()),
        (
            "rav_fiery_conclusion_sacrifice_damage",
            ["DamageDealtToPermanent", "StateBasedAction"].as_slice(),
        ),
        (
            "rav_ribbons_of_night_damage_life_slice",
            ["DamageDealtToPermanent", "LifeGained"].as_slice(),
        ),
        ("rav_muddle_counterspell", ["SpellCountered"].as_slice()),
        (
            "rav_transmute_search",
            ["CardRevealed", "LibraryShuffled", "Transmuted"].as_slice(),
        ),
        (
            "rav_dizzy_spell_modifier",
            ["ContinuousEffectCreated"].as_slice(),
        ),
        (
            "rav_dizzy_spell_transmute",
            ["CardRevealed", "LibraryShuffled", "Transmuted"].as_slice(),
        ),
    ] {
        let result = results
            .iter()
            .find(|result| result.id == id)
            .unwrap_or_else(|| panic!("missing public scenario {id}"));
        for receipt in receipts {
            assert!(
                result.event_log.iter().any(|event| event.contains(receipt)),
                "{id} is missing its supported `{receipt}` receipt"
            );
        }
    }
}
