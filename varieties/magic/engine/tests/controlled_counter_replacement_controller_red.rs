//! RED regression: the affected player for counter-placement replacement is
//! the live controller of the permanent receiving counters.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    ContinuousChange, CounterKind, Duration, Effect, Game, GameEvent, ManaCost, PlayerId,
    ReplacementEffect, ReplacementEffectBinding, ReplacementEventKind, Target,
    TargetRequirement,
};

const PLACER: &str = "TST-CONTROLLED-COUNTER-PLACER";
const TARGET: &str = "TST-CONTROLLED-COUNTER-TARGET";
const CONTROL_SOURCE: &str = "TST-CONTROLLED-COUNTER-CONTROL-SOURCE";
const DOUBLER: &str = "TST-CONTROLLED-COUNTER-DOUBLER";

fn definition(id: &'static str, card_type: CardType) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["synthetic-controlled-counter-replacement-contract"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

fn add_counter_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: "place-charge",
        mana_cost: ManaCost::new(0),
        tap_cost: false,
        sorcery_speed: false,
        additional_tap_creatures: 0,
        sacrifice_source: false,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets: vec![TargetRequirement::Permanent],
        effects: vec![Effect::AddCountersToTarget {
            counter: CounterKind::Charge,
            amount: 1,
        }],
    }
}

#[test]
fn counter_placement_uses_the_targets_live_controller_for_replacement() {
    let owner = PlayerId(0);
    let live_controller = PlayerId(1);
    let ability_controller = PlayerId(2);
    let mut game = Game::new_with_all_bindings(
        [
            definition(PLACER, CardType::Artifact),
            definition(TARGET, CardType::Artifact),
            definition(CONTROL_SOURCE, CardType::Artifact),
            definition(DOUBLER, CardType::Enchantment),
        ],
        3,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: PLACER,
            ability: add_counter_ability(),
        }],
    )
    .expect("three-player counter fixture initializes");
    game.register_replacement_effect_bindings([ReplacementEffectBinding {
        source_definition: DOUBLER,
        effect: ReplacementEffect::MultiplyCounterPlacement { multiplier: 2 },
    }])
    .expect("counter multiplier registers before the game");
    let target = game
        .put_on_battlefield(owner, TARGET)
        .expect("owner deploys counter target");
    let doubler = game
        .put_on_battlefield(live_controller, DOUBLER)
        .expect("future controller deploys multiplier");
    let control_source = game
        .put_on_battlefield(ability_controller, CONTROL_SOURCE)
        .expect("third player deploys control source");
    let placer = game
        .put_on_battlefield(ability_controller, PLACER)
        .expect("third player deploys counter ability source");

    game.begin_game().expect("game starts");
    game.add_continuous_effect(
        control_source,
        target,
        ContinuousChange::ChangeController(live_controller),
        Duration::Permanent,
    )
    .expect("target changes control through layer two");
    assert_eq!(game.controller_of(target), Ok(live_controller));

    for player in [owner, live_controller] {
        game.pass_priority(player)
            .expect("earlier player passes to ability controller");
    }
    game.activate_ability(
        ability_controller,
        AbilityActivation {
            source: placer,
            ability_id: "place-charge",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("counter ability activates");
    for player in [ability_controller, owner, live_controller] {
        game.pass_priority(player)
            .expect("all players pass the ability");
    }

    assert_eq!(
        game.object(target)
            .expect("target remains live")
            .counters
            .get(&CounterKind::Charge),
        Some(&2),
        "the multiplier controlled by the target's live controller doubles the counter"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ReplacementEffectApplied {
            source,
            affected_player,
            event: ReplacementEventKind::CounterPlacement { counter: CounterKind::Charge },
            original_amount: 1,
            replacement_amount: 2,
            ..
        } if *source == doubler && *affected_player == live_controller
    )));
    game.validate_invariants()
        .expect("controlled counter replacement preserves invariants");
}
