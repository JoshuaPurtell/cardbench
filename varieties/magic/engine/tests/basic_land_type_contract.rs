//! Expansion-neutral basic-land type-line contracts.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    BasicLandType, BasicLandTypeBinding, CardDefinition, CardType, Color, Game, GameEvent,
    ManaCost, PlayerId, RulesError, Zone,
};

fn basic_land(id: &'static str, color: Color) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::from([color]),
        card_types: BTreeSet::from([CardType::Land]),
        is_basic_land: true,
        supported_rules: &["basic-land"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

fn nonbasic_land(id: &'static str, color: Color) -> CardDefinition {
    let mut definition = basic_land(id, color);
    definition.is_basic_land = false;
    definition
}

#[test]
fn typed_basic_land_is_visible_to_policies_and_activates_its_intrinsic_mana_ability() {
    let player = PlayerId(0);
    let mut game = Game::new_with_basic_land_types(
        [basic_land("TST-PLAINS", Color::White)],
        2,
        [BasicLandTypeBinding {
            card_definition: "TST-PLAINS",
            land_type: BasicLandType::Plains,
        }],
    )
    .expect("a correctly typed basic land initializes");
    let plains = game
        .add_card(player, "TST-PLAINS", Zone::Battlefield)
        .expect("Plains starts on the battlefield");
    game.clear_event_log();

    let view = game
        .view_for_player(player)
        .expect("controller view exists");
    let plains_view = view
        .own_battlefield
        .iter()
        .find(|card| card.id == plains)
        .expect("typed land is visible to its controller");
    assert_eq!(plains_view.basic_land_type, Some(BasicLandType::Plains));
    assert_eq!(plains_view.mana_colors, BTreeSet::from([Color::White]));

    game.activate_mana_ability(player, plains, Color::White)
        .expect("Plains produces its type-linked intrinsic color");

    assert!(game.object(plains).expect("Plains exists").tapped);
    assert_eq!(
        game.player(player)
            .expect("player exists")
            .mana_pool
            .amount(Color::White),
        1
    );
    assert!(
        game.stack.is_empty(),
        "intrinsic mana ability uses no stack"
    );
    assert_eq!(game.priority, player, "activator retains priority");
    assert_eq!(
        game.event_log,
        vec![
            GameEvent::ManaAbilityActivated {
                player,
                land: plains,
                color: Color::White,
            },
            GameEvent::ManaAdded {
                player,
                color: Color::White,
                amount: 1,
            },
        ]
    );
    game.validate_invariants()
        .expect("typed-land state and receipts preserve invariants");
}

#[test]
fn basic_land_type_binding_rejects_nonbasic_duplicate_and_color_mismatches() {
    let nonbasic = Game::new_with_basic_land_types(
        [nonbasic_land("TST-NONBASIC", Color::White)],
        2,
        [BasicLandTypeBinding {
            card_definition: "TST-NONBASIC",
            land_type: BasicLandType::Plains,
        }],
    );
    assert_eq!(
        nonbasic.expect_err("only basic lands have this type-line binding"),
        RulesError::IllegalAction("a basic land type binding requires a basic land definition")
    );

    let duplicate = Game::new_with_basic_land_types(
        [basic_land("TST-PLAINS", Color::White)],
        2,
        [
            BasicLandTypeBinding {
                card_definition: "TST-PLAINS",
                land_type: BasicLandType::Plains,
            },
            BasicLandTypeBinding {
                card_definition: "TST-PLAINS",
                land_type: BasicLandType::Plains,
            },
        ],
    );
    assert_eq!(
        duplicate.expect_err("one definition has one type line"),
        RulesError::IllegalAction("duplicate basic land type binding for card definition")
    );

    let mismatch = Game::new_with_basic_land_types(
        [basic_land("TST-SWAMP", Color::Black)],
        2,
        [BasicLandTypeBinding {
            card_definition: "TST-SWAMP",
            land_type: BasicLandType::Plains,
        }],
    );
    assert_eq!(
        mismatch.expect_err("type must link to its actual intrinsic color"),
        RulesError::IllegalAction("a basic land type must match its intrinsic mana color")
    );
}
