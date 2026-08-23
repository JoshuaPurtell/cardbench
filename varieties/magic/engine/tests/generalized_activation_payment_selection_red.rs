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
                effects: vec![Effect::DrawControllerIfManaColorSpent { color: Color::Blue }],
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

fn started_fixture() -> (Game, cardbench_magic_engine::ObjectId) {
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
    (game, source)
}

fn activate_selected(game: &mut Game, source: cardbench_magic_engine::ObjectId) {
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
}

#[test]
fn generalized_activation_composes_nonmana_costs_with_selected_hybrid_payment() {
    let (mut game, source) = started_fixture();
    activate_selected(&mut game, source);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        cardbench_magic_engine::GameEvent::ActivatedAbilityCostCalculated { context }
            if context.source == source
                && context.source_incarnation == 1
                && context.ability_id == "pay-selected-generalized-cost"
                && context.payment_selection == Some(ManaPaymentSelection {
                    hybrid: vec![Color::Blue],
                    generic: vec![Color::Red],
                })
    )));
    assert_eq!(
        game.stack
            .last()
            .expect("ability is on the stack")
            .mana_spent,
        Some(vec![Color::Blue, Color::Red])
    );
    game.validate_invariants()
        .expect("live selected payment retains source-bound provenance");
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second player resolves ability");

    eprintln!(
        "generalized_selected_payment_events={:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.player(PlayerId(0)).expect("player exists").life, 18);
    assert_eq!(
        game.player(PlayerId(0)).expect("player exists").hand.len(),
        1
    );
    game.validate_invariants()
        .expect("selected payment and generalized costs retain provenance");
}

#[test]
fn selected_generalized_payment_provenance_rejects_a_tampered_live_allocation() {
    let (mut game, source) = started_fixture();
    activate_selected(&mut game, source);
    let context = game
        .event_log
        .iter_mut()
        .find_map(|event| match event {
            cardbench_magic_engine::GameEvent::ActivatedAbilityCostCalculated { context } => {
                Some(context)
            }
            _ => None,
        })
        .expect("explicit payment records a cost context");
    context.payment_selection = Some(ManaPaymentSelection {
        hybrid: vec![Color::Blue],
        generic: vec![Color::Blue],
    });

    let result = game.validate_invariants();
    assert!(result.is_err(), "{result:?}");
}

#[test]
fn malformed_selected_payment_rolls_back_the_whole_generalized_cost_transaction() {
    let (mut game, source) = started_fixture();
    let events_before = game.canonical_event_log();
    let mana_before = game
        .player(PlayerId(0))
        .expect("player exists")
        .mana_pool
        .clone();

    let result = game.activate_ability_with_generalized_costs(
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
                generic: vec![Color::Blue],
            }),
        },
    );

    assert!(result.is_err(), "{result:?}");
    assert_eq!(game.player(PlayerId(0)).expect("player exists").life, 20);
    assert_eq!(
        game.player(PlayerId(0)).expect("player exists").mana_pool,
        mana_before
    );
    assert!(game.stack.is_empty());
    assert_eq!(game.canonical_event_log(), events_before);
    game.validate_invariants()
        .expect("rejected payment leaves no state-machine residue");
}
