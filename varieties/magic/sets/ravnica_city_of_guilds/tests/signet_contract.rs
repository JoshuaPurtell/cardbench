//! Public full-fidelity contract for the four RAV Signets.
//!
//! Their artifact casting, paid/tapped fixed mana bundles, and the legal
//! payment-context activation window are all represented by typed engine data.

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
    payment_scenario_id: &'static str,
}

const SIGNETS: [Signet; 4] = [
    Signet {
        collector_number: 255,
        definition_id: "RAV-BOROS-SIGNET",
        name: "Boros Signet",
        ability_id: "boros-signet-wr",
        colors: [Color::White, Color::Red],
        scenario_id: "rav_boros_signet_paid_bundle",
        payment_scenario_id: "rav_boros_signet_cast_payment",
    },
    Signet {
        collector_number: 260,
        definition_id: "RAV-DIMIR-SIGNET",
        name: "Dimir Signet",
        ability_id: "dimir-signet-ub",
        colors: [Color::Blue, Color::Black],
        scenario_id: "rav_dimir_signet_paid_bundle",
        payment_scenario_id: "rav_dimir_signet_cast_payment",
    },
    Signet {
        collector_number: 262,
        definition_id: "RAV-GOLGARI-SIGNET",
        name: "Golgari Signet",
        ability_id: "golgari-signet-bg",
        colors: [Color::Black, Color::Green],
        scenario_id: "rav_golgari_signet_paid_bundle",
        payment_scenario_id: "rav_golgari_signet_cast_payment",
    },
    Signet {
        collector_number: 270,
        definition_id: "RAV-SELESNYA-SIGNET",
        name: "Selesnya Signet",
        ability_id: "selesnya-signet-gw",
        colors: [Color::White, Color::Green],
        scenario_id: "rav_selesnya_signet_paid_bundle",
        payment_scenario_id: "rav_selesnya_signet_cast_payment",
    },
];

#[test]
fn rav_signets_have_complete_artifact_and_paid_bundle_bindings() {
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
            [
                "full-rules-fidelity",
                "artifact-casting",
                "paid-fixed-two-color-mana-ability",
                "cast-payment-mana-activation",
            ]
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
fn rav_signets_are_promoted_only_with_the_payment_context_activation_window() {
    for signet in SIGNETS {
        assert!(
            RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&signet.definition_id),
            "{} is ability-complete only because the engine provides its typed payment-context mana-activation window",
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
                payment_mana_abilities: vec![],
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
fn rav_signet_direct_activation_scenarios_emit_paid_bundle_receipts_without_stack_or_priority_events()
 {
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

#[test]
fn rav_signet_cast_payment_scenarios_preserve_causal_event_order() {
    let results = run_all_scenarios().expect("public RAV scenarios run");
    for signet in SIGNETS {
        let result = results
            .iter()
            .find(|result| result.id == signet.payment_scenario_id)
            .expect("every Signet has a payment-context scenario");
        let event_index = |marker: &str| {
            result
                .event_log
                .iter()
                .position(|event| event.contains(marker))
                .unwrap_or_else(|| panic!("{} lacks {marker}", signet.name))
        };
        let contextual_activation = event_index("CastPaymentManaAbilityActivated");
        let bundle_activation = event_index("BoundManaAbilityBundleActivated");
        let activation_payment = event_index("ManaAbilityManaPaid");
        let spell_cast = event_index("SpellCast");
        let spell_resolved = event_index("SpellResolved");
        assert!(contextual_activation < bundle_activation, "{}", signet.name);
        assert!(bundle_activation < activation_payment, "{}", signet.name);
        for color in signet.colors {
            let output = event_index(&format!(
                "ManaAdded {{ player: PlayerId(0), color: {color:?}, amount: 1 }}"
            ));
            assert!(
                activation_payment < output && output < spell_cast,
                "{}",
                signet.name
            );
        }
        assert!(spell_cast < spell_resolved, "{}", signet.name);
        assert_eq!(
            result
                .event_log
                .iter()
                .filter(|event| event.contains("SpellCast"))
                .count(),
            1,
            "only the cast spell, never a mana ability, uses the stack"
        );
    }
}
