//! Red regression: an activated ability's nonmana selections and variable
//! payment must form one atomic, policy-submitted cost transaction.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, AbilityCostPayment, ActivatedAbility, ActivatedAbilityBinding,
    ActivatedAbilityCostBinding, ActivatedCounterCost, ActivatedCounterCostTarget, CardDefinition,
    CardType, Color, CounterKind, Effect, Game, GameEvent, GeneralizedAbilityActivation,
    GeneralizedActivatedAbilityCost, ManaCost, ObjectId, PlayerId, Target, Zone,
};

const SOURCE: &str = "TST-GENERALIZED-COST-SOURCE";
const COUNTER_BEARER: &str = "TST-GENERALIZED-COST-COUNTER-BEARER";
const RETURNED: &str = "TST-GENERALIZED-COST-RETURNED";

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
        supported_rules: &["generalized-activated-cost-red"],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

fn ability(
    id: &'static str,
    targets: Vec<cardbench_magic_engine::TargetRequirement>,
    effects: Vec<Effect>,
) -> ActivatedAbility {
    ActivatedAbility {
        id,
        mana_cost: ManaCost::new(1),
        tap_cost: false,
        sorcery_speed: false,
        additional_tap_creatures: 0,
        sacrifice_source: false,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets,
        effects,
    }
}

fn fixture() -> Game {
    let mut game = Game::new_with_all_bindings(
        [
            creature(SOURCE),
            creature(COUNTER_BEARER),
            creature(RETURNED),
        ],
        2,
        [],
        [],
        [],
        [
            ActivatedAbilityBinding {
                card_definition: SOURCE,
                ability: ActivatedAbility {
                    id: "seed-charge",
                    mana_cost: ManaCost::new(0),
                    tap_cost: false,
                    sorcery_speed: false,
                    additional_tap_creatures: 0,
                    sacrifice_source: false,
                    sacrifice_creatures: 0,
                    sacrifice_lands: 0,
                    discard_cards: 0,
                    targets: vec![cardbench_magic_engine::TargetRequirement::Permanent],
                    effects: vec![Effect::AddCountersToTarget {
                        counter: CounterKind::Charge,
                        amount: 2,
                    }],
                },
            },
            ActivatedAbilityBinding {
                card_definition: SOURCE,
                ability: ability(
                    "pay-everything",
                    vec![],
                    vec![Effect::GainLifeController { amount: 1 }],
                ),
            },
        ],
    )
    .expect("fixture constructs");
    game.register_generalized_activated_ability_cost_bindings([ActivatedAbilityCostBinding {
        card_definition: SOURCE,
        ability_id: "pay-everything",
        cost: GeneralizedActivatedAbilityCost {
            life_payment: 2,
            counter_removals: vec![ActivatedCounterCost {
                target: ActivatedCounterCostTarget::SelectedControlledPermanent,
                counter: CounterKind::Charge,
                amount: 2,
            }],
            return_source_to_hand: false,
            return_controlled_permanents: 1,
            put_hand_cards_on_library_top: 0,
            sacrifice_land_basic_type: None,
            has_x_cost: true,
        },
    }])
    .expect("generalized cost binding registers before the game begins");
    game
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first)
        .expect("first priority pass succeeds");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass resolves the stack");
}

fn activate_seed(game: &mut Game, source: ObjectId, target: ObjectId) {
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source,
            ability_id: "seed-charge",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("seed ability stacks");
    pass_pair(game);
}

#[test]
fn generalized_costs_are_policy_selected_atomic_and_provenanced() {
    let mut game = fixture();
    let source = game
        .put_on_battlefield(PlayerId(0), SOURCE)
        .expect("source starts on battlefield");
    let bearer = game
        .put_on_battlefield(PlayerId(0), COUNTER_BEARER)
        .expect("counter bearer starts on battlefield");
    let returned = game
        .put_on_battlefield(PlayerId(0), RETURNED)
        .expect("returned permanent starts on battlefield");
    game.begin_game().expect("fixture begins");
    activate_seed(&mut game, source, bearer);
    game.add_mana_from_action(PlayerId(0), Color::Blue, 4)
        .expect("four mana funds {1}+X");
    game.clear_event_log();

    game.activate_ability_with_generalized_costs(
        PlayerId(0),
        GeneralizedAbilityActivation {
            activation: AbilityActivation {
                source,
                ability_id: "pay-everything",
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![],
            },
            cost_payment: AbilityCostPayment {
                counter_sources: vec![bearer],
                return_permanents: vec![returned],
                hand_cards_to_library_top: vec![],
                chosen_x: Some(3),
            },
            mana_payment_selection: None,
        },
    )
    .expect("one atomic generalized payment is legal");

    assert_eq!(game.player(PlayerId(0)).expect("player exists").life, 18);
    assert_eq!(game.zone_of(returned), Some(Zone::Hand));
    assert!(
        game.object(bearer)
            .expect("counter bearer remains live")
            .counters
            .is_empty()
    );
    eprintln!("generalized_cost_events={:?}", game.canonical_event_log());
    assert!(game.event_log.windows(9).any(|events| matches!(
        events,
        [
            GameEvent::AbilityManaPaid { player: PlayerId(0), source: paid_source, ability: "pay-everything", mana_cost },
            GameEvent::AbilityLifePaid { player: PlayerId(0), source: life_source, ability: "pay-everything", amount: 2 },
            GameEvent::CounterRemovedAsAbilityCost { player: PlayerId(0), source: counter_source, card, counter: CounterKind::Charge, amount: 2 },
            GameEvent::CounterRemoved { source: removal_source, card: removed_card, counter: CounterKind::Charge, amount: 2 },
            GameEvent::ReturnedAsAbilityCost { player: PlayerId(0), source: returned_source, permanent },
            GameEvent::CardMoved { card: moved_return, to: Zone::Hand },
            GameEvent::ObjectIncarnationAdvanced { object: returned_object, .. },
            GameEvent::AbilityXCostChosen { player: PlayerId(0), source: x_source, ability: "pay-everything", x: 3 },
            GameEvent::AbilityActivated { player: PlayerId(0), source: activated_source, ability: "pay-everything", .. },
        ] if *paid_source == source && *life_source == source && *counter_source == source
            && *removal_source == source && *card == bearer && *removed_card == bearer
            && *returned_source == source && *permanent == returned && *moved_return == returned
            && *returned_object == returned && *x_source == source && *activated_source == source
            && *mana_cost == ManaCost::new(4)
    )));
    assert_eq!(
        game.stack.last().expect("ability is stacked").chosen_x,
        Some(3)
    );
    game.validate_invariants()
        .expect("cost transaction preserves engine invariants");
}

#[test]
fn failed_generalized_cost_payment_rolls_back_every_prior_component() {
    let mut game = fixture();
    let source = game
        .put_on_battlefield(PlayerId(0), SOURCE)
        .expect("source starts on battlefield");
    let bearer = game
        .put_on_battlefield(PlayerId(0), COUNTER_BEARER)
        .expect("counter bearer starts on battlefield");
    let returned = game
        .put_on_battlefield(PlayerId(0), RETURNED)
        .expect("returned permanent starts on battlefield");
    game.begin_game().expect("fixture begins");
    activate_seed(&mut game, source, bearer);
    game.add_mana_from_action(PlayerId(0), Color::Blue, 4)
        .expect("setup mana is available");
    game.clear_event_log();

    let result = game.activate_ability_with_generalized_costs(
        PlayerId(0),
        GeneralizedAbilityActivation {
            activation: AbilityActivation {
                source,
                ability_id: "pay-everything",
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![],
            },
            cost_payment: AbilityCostPayment {
                counter_sources: vec![bearer],
                return_permanents: vec![returned],
                hand_cards_to_library_top: vec![],
                chosen_x: Some(4),
            },
            mana_payment_selection: None,
        },
    );
    assert!(result.is_err(), "{result:?}");
    assert_eq!(game.player(PlayerId(0)).expect("player exists").life, 20);
    assert_eq!(game.zone_of(returned), Some(Zone::Battlefield));
    assert_eq!(
        game.object(bearer)
            .expect("counter bearer remains live")
            .counters
            .get(&CounterKind::Charge),
        Some(&2)
    );
    assert!(game.stack.is_empty());
    assert!(game.event_log.is_empty());
    game.validate_invariants()
        .expect("rejected cost transaction leaves no state-machine residue");
}
