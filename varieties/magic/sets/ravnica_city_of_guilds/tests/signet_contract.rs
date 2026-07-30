//! Public contract for the bounded RAV Signet compatibility slice.
//!
//! The fixed paid-bundle substrate is represented exactly, but the engine has
//! no payment-context mana-activation window. The Signets therefore must not
//! be promoted to the positive full-fidelity manifest yet.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, Game, GameEvent, ManaAbilityOutput, ManaBundle, ManaCost,
    PlayerId, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_mana_ability_bindings, run_all_scenarios,
};

#[derive(Clone, Copy)]
struct Signet {
    collector_number: u16,
    definition_id: &'static str,
    name: &'static str,
    ability_id: &'static str,
    colors: [Color; 2],
    scenario_id: &'static str,
}

const SIGNETS: [Signet; 4] = [
    Signet {
        collector_number: 255,
        definition_id: "RAV-BOROS-SIGNET",
        name: "Boros Signet",
        ability_id: "boros-signet-wr",
        colors: [Color::White, Color::Red],
        scenario_id: "rav_boros_signet_paid_bundle",
    },
    Signet {
        collector_number: 260,
        definition_id: "RAV-DIMIR-SIGNET",
        name: "Dimir Signet",
        ability_id: "dimir-signet-ub",
        colors: [Color::Blue, Color::Black],
        scenario_id: "rav_dimir_signet_paid_bundle",
    },
    Signet {
        collector_number: 262,
        definition_id: "RAV-GOLGARI-SIGNET",
        name: "Golgari Signet",
        ability_id: "golgari-signet-bg",
        colors: [Color::Black, Color::Green],
        scenario_id: "rav_golgari_signet_paid_bundle",
    },
    Signet {
        collector_number: 270,
        definition_id: "RAV-SELESNYA-SIGNET",
        name: "Selesnya Signet",
        ability_id: "selesnya-signet-gw",
        colors: [Color::White, Color::Green],
        scenario_id: "rav_selesnya_signet_paid_bundle",
    },
];

#[test]
fn rav_signets_have_exact_artifact_and_paid_bundle_compatibility_bindings() {
    let definitions = card_definitions();
    let bindings = rav_mana_ability_bindings();
    for signet in SIGNETS {
        assert_eq!(
            executable_definition_id_for_collector(signet.collector_number),
            Ok(signet.definition_id),
            "the public catalog maps {} at its verified collector number",
            signet.name
        );
        let definition = definitions
            .iter()
            .find(|definition| definition.id == signet.definition_id)
            .expect("every named Signet has a public compatibility definition");
        assert_eq!(definition.name, signet.name);
        assert_eq!(definition.mana_cost, ManaCost::new(2));
        assert!(definition.colors.is_empty());
        assert!(definition.mana_colors.is_empty());
        assert_eq!(definition.card_types.len(), 1);
        assert!(definition.card_types.contains(&CardType::Artifact));
        assert_eq!(
            definition.supported_rules,
            ["artifact-casting", "paid-fixed-two-color-mana-ability"]
        );

        let binding = bindings
            .iter()
            .find(|binding| binding.card_definition == signet.definition_id)
            .expect("each Signet has one definition-bound mana ability");
        assert_eq!(binding.ability.id, signet.ability_id);
        assert!(binding.ability.tap_cost);
        assert_eq!(binding.ability.amount, 0);
        assert_eq!(binding.ability.life_payment, None);
        assert_eq!(binding.ability.controller_damage, None);
        assert_eq!(
            binding.ability.output,
            ManaAbilityOutput::PaidBundle {
                mana_cost: ManaCost::new(1),
                bundle: ManaBundle::new(signet.colors.into_iter().map(|color| (color, 1))),
            },
            "{} has the exact paid fixed mana output",
            signet.name
        );
    }
}

#[test]
fn rav_signets_remain_outside_full_fidelity_until_payment_window_activation_exists() {
    for signet in SIGNETS {
        assert!(
            !RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&signet.definition_id),
            "{} must remain bounded: the engine currently only activates mana abilities with priority, not while paying a mana cost",
            signet.name
        );
    }
}

#[test]
fn rav_signets_cast_as_two_mana_artifacts_to_the_battlefield() {
    let definitions = card_definitions();
    for signet in SIGNETS {
        let mut game =
            Game::new_with_mana_abilities(definitions.clone(), 2, rav_mana_ability_bindings())
                .expect("RAV Signet bindings initialize");
        let signet_card = game
            .add_card(PlayerId(0), signet.definition_id, Zone::Hand)
            .expect("fixture puts Signet in hand");
        game.grant_mana(PlayerId(0), Color::Blue, 2)
            .expect("fixture grants the generic casting cost");
        game.clear_event_log();

        game.cast_spell(
            PlayerId(0),
            CastRequest {
                card: signet_card,
                targets: vec![],
                convoke: vec![],
            },
        )
        .expect("two generic mana casts each Signet artifact");
        game.pass_priority(PlayerId(0))
            .expect("caster passes to resolve the artifact");
        game.pass_priority(PlayerId(1))
            .expect("both players pass and the artifact resolves");

        assert_eq!(game.zone_of(signet_card), Some(Zone::Battlefield));
        assert!(
            game.event_log.iter().any(
                |event| matches!(event, GameEvent::SpellResolved { card, .. } if *card == signet_card)
            ),
            "{} has a canonical artifact-resolution event",
            signet.name
        );
        game.validate_invariants()
            .expect("Signet artifact casting preserves engine invariants");
    }
}

#[test]
fn rav_signet_public_scenarios_emit_paid_bundle_receipts_without_stack_or_priority_events() {
    let results = run_all_scenarios().expect("public RAV scenarios run");
    for signet in SIGNETS {
        let result = results
            .iter()
            .find(|result| result.id == signet.scenario_id)
            .expect("every Signet has a public scenario");
        assert_eq!(
            result
                .event_log
                .iter()
                .filter(|event| event.contains("BoundManaAbilityBundleActivated"))
                .count(),
            1,
            "{} emits exactly one paid-bundle activation receipt",
            signet.name
        );
        assert_eq!(
            result
                .event_log
                .iter()
                .filter(|event| event.contains("ManaAbilityManaPaid"))
                .count(),
            1,
            "{} emits exactly one activation-payment receipt",
            signet.name
        );
        for color in signet.colors {
            assert!(
                result.event_log.iter().any(|event| event.contains(&format!(
                    "ManaAdded {{ player: PlayerId(0), color: {color:?}, amount: 1 }}"
                ))),
                "{} emits a precise {color:?} mana receipt",
                signet.name
            );
        }
        assert!(
            result
                .event_log
                .iter()
                .all(|event| !event.contains("SpellCast")
                    && !event.contains("SpellResolved")
                    && !event.contains("PriorityPassed")),
            "{} activation uses neither the stack nor a priority pass",
            signet.name
        );
    }
}
