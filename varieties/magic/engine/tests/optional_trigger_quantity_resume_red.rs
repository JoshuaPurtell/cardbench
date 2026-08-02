//! Red regression: an accepted optional trigger must not reopen its payment
//! boundary after a later instruction pauses for a quantity replacement.
//!
//! The synthetic upkeep trigger first gains life, then creates a token through
//! two concurrent multipliers, then gains more life.  The replacement choice
//! is deliberately in the middle of the trigger's instruction list.  Once
//! the controller has accepted the optional trigger, resuming that suffix must
//! retain the already-paid decision rather than asking the controller again.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DecisionKind, DecisionSelection, Effect, Game, GameEvent,
    ManaCost, PlayerId, PolicyAction, ReplacementChoice, ReplacementEffect,
    ReplacementEffectBinding, TokenSpec, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};

const OPTIONAL_TRIGGER_SOURCE: &str = "TST-OPTIONAL-TRIGGER-QUANTITY-SOURCE";
const DOUBLER: &str = "TST-OPTIONAL-TRIGGER-QUANTITY-DOUBLER";
const TRIPLER: &str = "TST-OPTIONAL-TRIGGER-QUANTITY-TRIPLER";
const ABILITY: &str = "optional-upkeep-token-and-life";

fn permanent(id: &'static str, card_type: CardType) -> CardDefinition {
    let is_creature = card_type == CardType::Creature;
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["optional-trigger-quantity-resume-red"],
        power: is_creature.then_some(1),
        toughness: is_creature.then_some(1),
        keywords: vec![],
        effects: vec![],
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first)
        .expect("first player passes priority");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second player passes priority");
}

#[test]
#[allow(clippy::too_many_lines)] // This is the public stack/decision transcript under audit.
fn accepted_optional_trigger_does_not_reopen_after_middle_quantity_replacement() {
    let controller = PlayerId(0);
    let binding = TriggeredAbilityBinding {
        card_definition: OPTIONAL_TRIGGER_SOURCE,
        ability: TriggeredAbility {
            id: ABILITY,
            condition: TriggerCondition::BeginningOfUpkeep,
            mana_cost: ManaCost::new(0),
            optional: true,
            targets: vec![],
            effects: vec![
                Effect::GainLifeController { amount: 1 },
                Effect::CreateToken {
                    token: TokenSpec::saproling(),
                    count: 1,
                },
                Effect::GainLifeController { amount: 2 },
            ],
        },
    };
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            permanent(OPTIONAL_TRIGGER_SOURCE, CardType::Creature),
            permanent(DOUBLER, CardType::Enchantment),
            permanent(TRIPLER, CardType::Enchantment),
        ],
        2,
        [],
        [],
        [],
        [],
        [binding],
    )
    .expect("fixture initializes");
    game.register_replacement_effect_bindings([
        ReplacementEffectBinding {
            source_definition: DOUBLER,
            effect: ReplacementEffect::MultiplyTokenCreation { multiplier: 2 },
        },
        ReplacementEffectBinding {
            source_definition: TRIPLER,
            effect: ReplacementEffect::MultiplyTokenCreation { multiplier: 3 },
        },
    ])
    .expect("multiplier bindings register before the game");
    let source = game
        .put_on_battlefield(controller, OPTIONAL_TRIGGER_SOURCE)
        .expect("optional trigger source begins on the battlefield");
    game.put_on_battlefield(controller, DOUBLER)
        .expect("doubler begins on the battlefield");
    game.put_on_battlefield(controller, TRIPLER)
        .expect("tripler begins on the battlefield");
    game.begin_game().expect("the game begins at upkeep");

    pass_pair(&mut game);
    let optional = game
        .view_for_player(controller)
        .expect("controller receives a view")
        .optional_triggered_ability_choice
        .expect("optional trigger reaches its one payment decision");
    assert_eq!(optional.source, source);
    assert_eq!(optional.ability, ABILITY);
    assert!(optional.can_pay, "zero-cost optional trigger can be accepted");

    game.submit_policy_move(
        controller,
        "optional-trigger-quantity-resume-red.accept.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            source,
            ability: ABILITY,
            pay: true,
            target: None,
        },
    )
    .expect("accepting optional trigger reaches its middle replacement choice");
    let decision = game
        .view_for_player(controller)
        .expect("controller receives replacement view")
        .pending_decision
        .expect("middle token instruction opens a replacement decision");
    assert_eq!(decision.kind, DecisionKind::Replacement);
    let tripler = decision
        .replacement_candidates
        .iter()
        .copied()
        .find(|choice| {
            matches!(
                choice,
                ReplacementChoice::Quantity {
                    effect: ReplacementEffect::MultiplyTokenCreation { multiplier: 3 },
                    ..
                }
            )
        })
        .expect("tripler is one legal first replacement");

    game.submit_policy_move(
        controller,
        "optional-trigger-quantity-resume-red.replace.v1",
        PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::Replacements(vec![tripler]),
        },
    )
    .expect("replacement choice resumes the already accepted optional trigger");

    let view = game
        .view_for_player(controller)
        .expect("controller receives final view");
    eprintln!(
        "optional-trigger quantity resume trace: stack={:?}; pending={:?}; optional={:?}; events={:?}",
        game.stack,
        view.pending_decision,
        view.optional_triggered_ability_choice,
        game.canonical_event_log(),
    );
    assert!(
        view.optional_triggered_ability_choice.is_none(),
        "an accepted optional trigger must not reopen its payment choice after a later replacement pause"
    );
    assert!(view.pending_decision.is_none());
    assert!(game.stack.is_empty(), "the resumed trigger resolves exactly once");
    assert_eq!(game.player(controller).expect("controller exists").life, 23);
    assert_eq!(
        game.player(controller)
            .expect("controller exists")
            .battlefield
            .len(),
        9,
        "source plus two multipliers plus six replaced tokens remain"
    );
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    GameEvent::AbilityResolved { source: resolved, ability, .. }
                        if *resolved == source && *ability == ABILITY
                )
            })
            .count(),
        1,
        "the trigger has one terminal resolution receipt"
    );
    game.validate_invariants()
        .expect("completed optional trigger remains state-machine valid");
}
