//! Red regression: a generalized activation must retain the controller's
//! explicit generic/hybrid mana allocation in the same atomic cost action.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, AbilityCostPayment, ActivatedAbility, ActivatedAbilityBinding,
    ActivatedAbilityCostBinding, CardDefinition, CardType, Color, Effect, Game,
    GeneralizedAbilityActivation, GeneralizedActivatedAbilityCost, HybridManaSymbol, ManaCost,
    ManaPaymentSelection, PlayerId,
};

const SOURCE: &str = "TST-GENERALIZED-SELECTED-PAYMENT";
const DRAW: &str = "TST-GENERALIZED-SELECTED-PAYMENT-DRAW";

fn creature(id: &'static str) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["generalized-activation-payment-selection-red"],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

fn fixture() -> Game {
    let mut game = Game::new_with_all_bindings(
        [creature(SOURCE), creature(DRAW)],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: SOURCE,
            ability: ActivatedAbility {
                id: "pay-selected-generalized-cost",
                mana_cost: ManaCost::with_hybrid(
                    1,
                    [],
                    [HybridManaSymbol {
                        first: Color::Blue,
                        second: Color::Red,
                    }],
                ),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::DrawControllerIfManaColorSpent {
                    color: Color::Blue,
                }],
            },
        }],
    )
    .expect("fixture constructs");
    game.register_generalized_activated_ability_cost_bindings([ActivatedAbilityCostBinding {
        card_definition: SOURCE,
        ability_id: "pay-selected-generalized-cost",
        cost: GeneralizedActivatedAbilityCost {
            life_payment: 2,
            ..GeneralizedActivatedAbilityCost::default()
        },
    }])
    .expect("generalized cost registers before the game begins");
    game
}

#[test]
fn generalized_activation_composes_nonmana_costs_with_selected_hybrid_payment() {
    let mut game = fixture();
    let source = game
        .put_on_battlefield(PlayerId(0), SOURCE)
        .expect("source starts on battlefield");
    game.add_card(PlayerId(0), DRAW, cardbench_magic_engine::Zone::Library)
        .expect("draw card enters library before the game begins");
    game.begin_game().expect("fixture begins");
    game.add_mana_from_action(PlayerId(0), Color::Blue, 1)
        .expect("blue mana is available");
    game.add_mana_from_action(PlayerId(0), Color::Red, 1)
        .expect("red mana is available");

    game.activate_ability_with_generalized_costs(
        PlayerId(0),
        GeneralizedAbilityActivation {
            activation: AbilityActivation {
                source,
                ability_id: "pay-selected-generalized-cost",
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![],
            },
            cost_payment: AbilityCostPayment::default(),
            mana_payment_selection: Some(ManaPaymentSelection {
                hybrid: vec![Color::Blue],
                generic: vec![Color::Red],
            }),
        },
    )
    .expect("one generalized activation accepts its selected payment");
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player resolves ability");

    assert_eq!(game.player(PlayerId(0)).expect("player exists").life, 18);
    assert_eq!(game.player(PlayerId(0)).expect("player exists").hand.len(), 1);
    game.validate_invariants()
        .expect("selected payment and generalized costs retain provenance");
}
