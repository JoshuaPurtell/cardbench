//! Public RAV spell-feature contracts for a focused expansion batch.
//!
//! Ribbons of Night deliberately excludes its
//! payment-color-conditioned draw because this engine slice does not preserve
//! the colors spent to cast a spell.

use cardbench_magic_engine::{Color, Effect, Keyword, ManaCost, TargetRequirement};
use cardbench_magic_rav::{
    card_definitions, executable_definition_id_for_collector, run_all_scenarios,
};

fn definition(id: &str) -> cardbench_magic_engine::CardDefinition {
    card_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
        .unwrap_or_else(|| panic!("missing RAV definition {id}"))
}

#[test]
fn batch_card_metadata_and_executable_semantics_are_explicit() {
    let ribbons = definition("RAV-RIBBONS-OF-NIGHT");
    assert_eq!(ribbons.mana_cost, ManaCost::with_colors(4, [Color::Black]));
    assert_eq!(
        ribbons.supported_rules,
        ["targeted-creature-damage", "life-gain"]
    );
    assert_eq!(
        ribbons.effects,
        vec![
            Effect::DealDamage {
                amount: 4,
                target: TargetRequirement::Creature,
            },
            Effect::GainLifeController { amount: 4 },
        ]
    );

    let dogpile = definition("RAV-DOGPILE");
    assert_eq!(dogpile.mana_cost, ManaCost::with_colors(3, [Color::Red]));
    assert_eq!(
        dogpile.supported_rules,
        [
            "full-rules-fidelity",
            "player-or-creature-targeting",
            "attacking-creature-count-damage",
        ]
    );
    assert_eq!(
        dogpile.effects,
        vec![Effect::DealDamageEqualToAttackingCreatures {
            target: TargetRequirement::PlayerOrCreature,
        }]
    );

    let overwhelm = definition("RAV-OVERWHELM");
    assert_eq!(
        overwhelm.mana_cost,
        ManaCost::with_colors(5, [Color::Green, Color::Green])
    );
    assert_eq!(
        overwhelm.supported_rules,
        [
            "full-rules-fidelity",
            "convoke",
            "controller-creature-layer-7-modifier",
        ]
    );
    assert_eq!(overwhelm.keywords, vec![Keyword::Convoke]);
    assert_eq!(
        overwhelm.effects,
        vec![Effect::ModifyControllerCreaturesPtUntilEndOfTurn {
            power: 3,
            toughness: 3,
        }]
    );
}

#[test]
fn catalog_maps_the_exact_public_spells() {
    assert_eq!(
        executable_definition_id_for_collector(101),
        Ok("RAV-RIBBONS-OF-NIGHT")
    );
    assert_eq!(
        executable_definition_id_for_collector(120),
        Ok("RAV-DOGPILE")
    );
    assert_eq!(
        executable_definition_id_for_collector(175),
        Ok("RAV-OVERWHELM")
    );
}

#[test]
fn public_scenarios_exercise_each_spells_complete_semantics() {
    let scenario_ids = run_all_scenarios()
        .expect("RAV public scenarios run")
        .into_iter()
        .map(|result| result.id)
        .collect::<std::collections::BTreeSet<_>>();
    for id in [
        "rav_ribbons_of_night_damage_life_slice",
        "rav_overwhelm_convoke_wide_modifier",
        "rav_dogpile_combat_count_damage",
        "rav_dogpile_rejects_noncreature_target",
    ] {
        assert!(scenario_ids.contains(id), "missing public scenario {id}");
    }
}

#[test]
fn public_event_logs_capture_the_new_spells_meaningful_resolution_receipts() {
    let scenarios = run_all_scenarios().expect("RAV public scenarios run");
    let find = |id| {
        scenarios
            .iter()
            .find(|scenario| scenario.id == id)
            .unwrap_or_else(|| panic!("missing public scenario {id}"))
    };

    let ribbons = find("rav_ribbons_of_night_damage_life_slice");
    assert!(ribbons.event_log.iter().any(|event| {
        event == "DamageDealtToPermanent { source: ObjectId(1), permanent: ObjectId(2), amount: 4 }"
    }));
    assert!(
        ribbons
            .event_log
            .iter()
            .any(|event| event == "LifeGained { player: PlayerId(0), amount: 4 }")
    );

    let overwhelm = find("rav_overwhelm_convoke_wide_modifier");
    assert_eq!(
        overwhelm
            .event_log
            .iter()
            .filter(|event| event.contains("ConvokeUsed"))
            .count(),
        5
    );
    assert_eq!(
        overwhelm
            .event_log
            .iter()
            .filter(|event| event.contains("ContinuousEffectCreated"))
            .count(),
        5
    );

    let dogpile = find("rav_dogpile_combat_count_damage");
    let attackers = dogpile
        .event_log
        .iter()
        .position(|event| {
            event
                == "AttackersDeclared { player: PlayerId(0), attackers: [ObjectId(2), ObjectId(3)] }"
        })
        .expect("Dogpile trace records both attackers");
    let cast = dogpile
        .event_log
        .iter()
        .position(|event| event == "SpellCast { player: PlayerId(0), card: ObjectId(1) }")
        .expect("Dogpile trace records its stack entry");
    let damage = dogpile
        .event_log
        .iter()
        .position(|event| {
            event == "DamageDealtToPermanent { source: ObjectId(1), permanent: ObjectId(8), amount: 2 }"
        })
        .expect("Dogpile trace records two damage to the selected creature");
    let resolved = dogpile
        .event_log
        .iter()
        .position(|event| event == "SpellResolved { card: ObjectId(1) }")
        .expect("Dogpile trace records resolution");
    let sba = dogpile
        .event_log
        .iter()
        .position(|event| {
            event
                == "StateBasedAction { card: ObjectId(8), reason: \"creature has lethal damage\" }"
        })
        .expect("Dogpile trace runs an SBA after the complete damage instruction");
    assert!(attackers < cast && cast < damage && damage < resolved && resolved < sba);
}
