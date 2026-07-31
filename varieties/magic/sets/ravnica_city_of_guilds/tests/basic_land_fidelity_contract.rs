//! Public typed-basic-land boundary contract for the five RAV basic lands.
//!
//! This stores only `CardBench` semantic identifiers and public rules-state
//! assertions; it does not reproduce card prose, artwork, or upstream data.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    BasicLandType, CardType, Color, Game, GameEvent, ManaCost, PlayerId, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, executable_definition_id_for_collector,
    rav_basic_land_type_bindings, rav_mana_ability_bindings, run_all_scenarios,
};

#[derive(Clone, Copy)]
struct BasicLand {
    definition_id: &'static str,
    name: &'static str,
    land_type: BasicLandType,
    color: Color,
    first_collector_number: u16,
}

const BASIC_LANDS: [BasicLand; 5] = [
    BasicLand {
        definition_id: "RAV-PLAINS",
        name: "Plains",
        land_type: BasicLandType::Plains,
        color: Color::White,
        first_collector_number: 287,
    },
    BasicLand {
        definition_id: "RAV-ISLAND",
        name: "Island",
        land_type: BasicLandType::Island,
        color: Color::Blue,
        first_collector_number: 291,
    },
    BasicLand {
        definition_id: "RAV-SWAMP",
        name: "Swamp",
        land_type: BasicLandType::Swamp,
        color: Color::Black,
        first_collector_number: 295,
    },
    BasicLand {
        definition_id: "RAV-MOUNTAIN",
        name: "Mountain",
        land_type: BasicLandType::Mountain,
        color: Color::Red,
        first_collector_number: 299,
    },
    BasicLand {
        definition_id: "RAV-FOREST",
        name: "Forest",
        land_type: BasicLandType::Forest,
        color: Color::Green,
        first_collector_number: 303,
    },
];

fn typed_rav_game() -> Game {
    Game::new_with_mana_abilities_and_basic_land_types(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
    )
    .expect("RAV typed basic-land bindings initialize")
}

#[test]
fn rav_basic_land_definitions_and_type_bindings_are_exact_and_ability_complete() {
    let definitions = card_definitions();
    let bindings = rav_basic_land_type_bindings();
    assert_eq!(bindings.len(), BASIC_LANDS.len());
    for basic in BASIC_LANDS {
        let definition = definitions
            .iter()
            .find(|definition| definition.id == basic.definition_id)
            .expect("every RAV basic land has a definition");
        assert_eq!(definition.name, basic.name);
        assert_eq!(definition.mana_cost, ManaCost::new(0));
        assert!(definition.colors.is_empty());
        assert_eq!(definition.mana_colors, BTreeSet::from([basic.color]));
        assert_eq!(definition.card_types, BTreeSet::from([CardType::Land]));
        assert!(definition.is_basic_land);
        assert_eq!(
            definition.supported_rules,
            [
                "full-rules-fidelity",
                "basic-land-type-line",
                "intrinsic-single-color-mana-ability",
                "basic-land-deck-construction",
            ]
        );
        assert!(
            RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&basic.definition_id),
            "{} is positive-manifest only after the typed cast-payment path is covered",
            basic.name
        );
        assert_eq!(basic.land_type.intrinsic_mana_color(), basic.color);
        assert!(bindings.iter().any(|binding| {
            binding.card_definition == basic.definition_id && binding.land_type == basic.land_type
        }));
        for collector_number in basic.first_collector_number..basic.first_collector_number + 4 {
            assert_eq!(
                executable_definition_id_for_collector(collector_number),
                Ok(basic.definition_id),
                "each retained {} printing resolves to its typed definition",
                basic.name
            );
        }
    }
}

#[test]
fn rav_basic_land_types_are_visible_in_state_and_produce_exact_nonstack_receipts() {
    let player = PlayerId(0);
    let mut game = typed_rav_game();
    let lands = BASIC_LANDS.map(|basic| {
        (
            basic,
            game.add_card(player, basic.definition_id, Zone::Battlefield)
                .expect("basic land begins on the battlefield"),
        )
    });
    game.clear_event_log();

    let view = game
        .view_for_player(player)
        .expect("controller view exists");
    for (basic, land) in lands {
        assert_eq!(game.basic_land_type(land), Ok(Some(basic.land_type)));
        let land_view = view
            .own_battlefield
            .iter()
            .find(|card| card.id == land)
            .expect("each land is visible to its controller");
        assert_eq!(land_view.basic_land_type, Some(basic.land_type));
        game.activate_mana_ability(player, land, basic.color)
            .expect("typed land produces exactly its linked color");
    }

    assert!(game.stack.is_empty());
    assert_eq!(game.priority, player);
    for basic in BASIC_LANDS {
        assert_eq!(
            game.player(player)
                .expect("player exists")
                .mana_pool
                .amount(basic.color),
            1,
            "{} produces exactly one of its linked color",
            basic.name
        );
    }
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::ManaAbilityActivated { .. }))
            .count(),
        5
    );
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::ManaAdded { .. }))
            .count(),
        5
    );
    game.validate_invariants()
        .expect("all typed basic-land transitions preserve invariants");
}

#[test]
fn rav_basic_land_public_scenario_has_one_receipt_per_type_and_no_stack_events() {
    let scenario = run_all_scenarios()
        .expect("public RAV scenarios run")
        .into_iter()
        .find(|result| result.id == "rav_basic_land_type_lines")
        .expect("typed basic-land scenario exists");
    assert_eq!(scenario.digest, "fnv1a64:2ebe6d7e14855cd8");
    assert_eq!(
        scenario
            .event_log
            .iter()
            .filter(|event| event.contains("ManaAbilityActivated"))
            .count(),
        5
    );
    for basic in BASIC_LANDS {
        assert!(
            scenario
                .event_log
                .iter()
                .any(|event| event.contains(&format!(
                    "ManaAdded {{ player: PlayerId(0), color: {:?}, amount: 1 }}",
                    basic.color
                ))),
            "{} has a precise mana receipt",
            basic.name
        );
    }
    assert!(
        scenario
            .event_log
            .iter()
            .all(|event| !event.contains("SpellCast")
                && !event.contains("SpellResolved")
                && !event.contains("PriorityPassed")),
        "typed basic-land abilities never use the stack or a priority pass"
    );
}

#[test]
fn rav_basic_land_cast_payment_scenario_has_ordered_intrinsic_receipts() {
    let scenario = run_all_scenarios()
        .expect("public RAV scenarios run")
        .into_iter()
        .find(|result| result.id == "rav_basic_land_cast_payment")
        .expect("typed basic-land cast-payment scenario exists");
    assert_eq!(scenario.digest, "fnv1a64:092e92750db81bd3");
    assert_eq!(
        scenario.event_log[..7],
        [
            "CastPaymentBasicLandManaAbilityActivated { player: PlayerId(0), card: ObjectId(3), land: ObjectId(1), color: Green }",
            "ManaAbilityActivated { player: PlayerId(0), land: ObjectId(1), color: Green }",
            "ManaAdded { player: PlayerId(0), color: Green, amount: 1 }",
            "CastPaymentBasicLandManaAbilityActivated { player: PlayerId(0), card: ObjectId(3), land: ObjectId(2), color: White }",
            "ManaAbilityActivated { player: PlayerId(0), land: ObjectId(2), color: White }",
            "ManaAdded { player: PlayerId(0), color: White, amount: 1 }",
            "SpellCast { player: PlayerId(0), card: ObjectId(3) }",
        ],
        "each explicit typed source is recorded before its intrinsic receipt and the enclosing spell receipt"
    );
    assert!(
        scenario
            .event_log
            .iter()
            .all(|event| !event.contains("BoundManaAbilityActivated")
                && !event.contains("BoundManaAbilityBundleActivated")),
        "basic-land payment uses the intrinsic substrate rather than a catalog-bound substitute"
    );
}
