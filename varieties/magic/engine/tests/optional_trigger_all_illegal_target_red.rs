//! Red regression: an all-illegal optional triggered ability never prompts.
//!
//! Target legality is checked before resolving an optional instruction. If a
//! response removes the sole target, CR 608.2b counters the ability; its
//! controller must not receive a later optional-payment decision.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, ManaCost, PlayerId,
    PolicyAction, Target, TargetRequirement, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding, Zone,
};

const SOURCE: &str = "TST-OPTIONAL-ILLEGAL-TARGET-SOURCE";
const TARGET: &str = "TST-OPTIONAL-ILLEGAL-TARGET-CREATURE";
const DESTROY: &str = "TST-OPTIONAL-ILLEGAL-TARGET-DESTROY";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    let creature = card_type == CardType::Creature;
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["optional-trigger-all-illegal-target-red"],
        power: creature.then_some(2),
        toughness: creature.then_some(2),
        keywords: vec![],
        effects,
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

#[test]
#[allow(clippy::too_many_lines)] // The target decision and response stack transcript are the regression.
fn all_illegal_optional_trigger_is_countered_before_optional_payment_choice() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(SOURCE, CardType::Creature, vec![]),
            definition(TARGET, CardType::Creature, vec![]),
            definition(
                DESTROY,
                CardType::Instant,
                vec![Effect::ExileTargetCreature],
            ),
        ],
        2,
        [],
        [],
        [],
        [],
        [TriggeredAbilityBinding {
            card_definition: SOURCE,
            ability: TriggeredAbility {
                id: "optional-upkeep-damage",
                condition: TriggerCondition::BeginningOfUpkeep,
                mana_cost: ManaCost::new(0),
                optional: true,
                targets: vec![TargetRequirement::Creature],
                effects: vec![Effect::DealDamage {
                    amount: 1,
                    target: TargetRequirement::Creature,
                }],
            },
        }],
    )
    .expect("fixture initializes");
    let source = game
        .put_on_battlefield(controller, SOURCE)
        .expect("trigger source setup");
    let target = game
        .put_on_battlefield(opponent, TARGET)
        .expect("target creature setup");
    let destroy = game
        .add_card(opponent, DESTROY, Zone::Hand)
        .expect("response spell setup");
    game.begin_game().expect("game begins at controller upkeep");

    let target_choice = game
        .view_for_player(controller)
        .expect("controller view")
        .triggered_ability_target_choice
        .expect("upkeep trigger needs its creature target");
    game.submit_policy_move(
        controller,
        "optional-trigger-all-illegal-target.choose.v1",
        PolicyAction::ChooseTriggeredAbilityTargets {
            decision: target_choice.decision,
            source,
            ability: "optional-upkeep-damage",
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("controller chooses the legal creature target");

    game.pass_priority(controller)
        .expect("controller passes to opponent");
    game.cast_spell(
        opponent,
        CastRequest {
            card: destroy,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent responds by exiling the target");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(target), Some(Zone::Exile));

    pass_pair(&mut game);
    eprintln!(
        "optional all-illegal target trace={:?}; optional={:?}",
        game.canonical_event_log(),
        game.view_for_player(controller)
            .expect("controller view after counter")
            .optional_triggered_ability_choice
    );
    assert!(
        game.view_for_player(controller)
            .expect("controller view after counter")
            .optional_triggered_ability_choice
            .is_none(),
        "an all-illegal targeted ability must not open an optional payment choice"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityCounteredByRules { source: actual, ability, .. }
            if *actual == source && *ability == "optional-upkeep-damage"
    )));
    game.validate_invariants()
        .expect("all-illegal optional trigger remains invariant-valid");
}
