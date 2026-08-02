//! Red regression for affected-player ordering of concurrent quantity replacements.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, CounterKind, DecisionKind, DecisionSelection, Effect,
    Game, GameEvent, ManaCost, PlayerId, PolicyAction, ReplacementChoice, ReplacementEffect,
    ReplacementEffectBinding, TokenSpec, Zone,
};

const DOUBLER: &str = "TST-CHAIN-DOUBLER";
const TRIPLER: &str = "TST-CHAIN-TRIPLER";
const QUADRUPLER: &str = "TST-CHAIN-QUADRUPLER";
const TOKEN_SPELL: &str = "TST-CHAIN-TOKEN";
const COUNTER_SPELL: &str = "TST-CHAIN-COUNTER";
const TARGET: &str = "TST-CHAIN-TARGET";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type.clone()]),
        is_basic_land: false,
        supported_rules: &["replacement-chain-red"],
        power: (card_type == CardType::Creature).then_some(2),
        toughness: (card_type == CardType::Creature).then_some(2),
        keywords: vec![],
        effects,
    }
}

fn fixture() -> (Game, PlayerId, cardbench_magic_engine::ObjectId) {
    let player = PlayerId(0);
    let mut game = Game::new(
        [
            definition(DOUBLER, CardType::Enchantment, vec![]),
            definition(TRIPLER, CardType::Enchantment, vec![]),
            definition(QUADRUPLER, CardType::Enchantment, vec![]),
            definition(
                TOKEN_SPELL,
                CardType::Instant,
                vec![Effect::CreateToken {
                    token: TokenSpec::saproling(),
                    count: 1,
                }],
            ),
            definition(
                COUNTER_SPELL,
                CardType::Instant,
                vec![Effect::AddCountersToTarget {
                    counter: CounterKind::Charge,
                    amount: 1,
                }],
            ),
            definition(TARGET, CardType::Creature, vec![]),
        ],
        2,
    )
    .expect("fixture constructs");
    game.register_replacement_effect_bindings([
        ReplacementEffectBinding {
            source_definition: DOUBLER,
            effect: ReplacementEffect::MultiplyTokenCreation { multiplier: 2 },
        },
        ReplacementEffectBinding {
            source_definition: TRIPLER,
            effect: ReplacementEffect::MultiplyTokenCreation { multiplier: 3 },
        },
        ReplacementEffectBinding {
            source_definition: QUADRUPLER,
            effect: ReplacementEffect::MultiplyTokenCreation { multiplier: 4 },
        },
        ReplacementEffectBinding {
            source_definition: DOUBLER,
            effect: ReplacementEffect::MultiplyCounterPlacement { multiplier: 2 },
        },
        ReplacementEffectBinding {
            source_definition: TRIPLER,
            effect: ReplacementEffect::MultiplyCounterPlacement { multiplier: 3 },
        },
    ])
    .expect("immutable replacement bindings register");
    for source in [DOUBLER, TRIPLER, QUADRUPLER] {
        game.put_on_battlefield(player, source)
            .expect("replacement source enters before the game");
    }
    let target = game
        .put_on_battlefield(player, TARGET)
        .expect("counter target enters before the game");
    game.add_card(player, TOKEN_SPELL, Zone::Hand)
        .expect("token spell enters before the game");
    game.add_card(player, COUNTER_SPELL, Zone::Hand)
        .expect("counter spell enters before the game");
    game.begin_game().expect("fixture game starts");
    (game, player, target)
}

fn resolve_to_replacement_choice(
    game: &mut Game,
    player: PlayerId,
    spell: &'static str,
    targets: Vec<cardbench_magic_engine::Target>,
) -> cardbench_magic_engine::PendingDecisionView {
    let card = game.players[player.0]
        .hand
        .iter()
        .copied()
        .find(|card| {
            game.card_definition(*card)
                .is_ok_and(|definition| definition.id == spell)
        })
        .expect("pre-game fixture has the requested spell in hand");
    game.cast_spell(
        player,
        CastRequest {
            card,
            targets,
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("zero-cost spell casts");
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second player reaches replacement boundary");
    game.view_for_player(player)
        .expect("affected player view")
        .pending_decision
        .expect("concurrent replacements open one typed choice")
}

fn quantity_choice(
    decision: &cardbench_magic_engine::PendingDecisionView,
    multiplier: u8,
) -> ReplacementChoice {
    decision
        .replacement_candidates
        .iter()
        .copied()
        .find(|choice| matches!(
            choice,
            ReplacementChoice::Quantity {
                effect: ReplacementEffect::MultiplyTokenCreation { multiplier: choice_multiplier }
                    | ReplacementEffect::MultiplyCounterPlacement { multiplier: choice_multiplier },
                ..
            } if *choice_multiplier == multiplier
        ))
        .expect("requested source is a current replacement option")
}

#[test]
fn concurrent_token_replacements_are_chosen_in_affected_player_order_and_apply_once() {
    let (mut game, player, _) = fixture();
    let first = resolve_to_replacement_choice(&mut game, player, TOKEN_SPELL, vec![]);
    assert_eq!(first.kind, DecisionKind::Replacement);
    assert_eq!(first.replacement_candidates.len(), 3);
    assert!(
        game.pass_priority(player).is_err(),
        "choice blocks priority"
    );

    let triplers_choice = quantity_choice(&first, 3);
    game.submit_policy_move(
        player,
        "replacement-chain-red",
        PolicyAction::SubmitDecision {
            decision: first.id,
            selection: DecisionSelection::Replacements(vec![triplers_choice]),
        },
    )
    .expect("affected player chooses the first replacement");
    let second = game
        .view_for_player(player)
        .expect("second choice view")
        .pending_decision
        .expect("remaining concurrent replacements require a new id");
    assert_ne!(second.id, first.id);
    assert_eq!(second.replacement_candidates.len(), 2);
    let doubler_choice = quantity_choice(&second, 2);
    game.submit_policy_move(
        player,
        "replacement-chain-red",
        PolicyAction::SubmitDecision {
            decision: second.id,
            selection: DecisionSelection::Replacements(vec![doubler_choice]),
        },
    )
    .expect("affected player chooses the second replacement");

    eprintln!(
        "token replacement-chain trace: {:?}",
        game.canonical_event_log()
    );

    assert_eq!(
        game.players[player.0].battlefield.len(),
        4 + 24,
        "the final forced source applies once after 1 -> 3 -> 6 -> 24"
    );
    let applied = game
        .event_log
        .iter()
        .filter(|event| matches!(event, GameEvent::ReplacementEffectApplied { .. }))
        .collect::<Vec<_>>();
    assert_eq!(applied.len(), 3);
    assert!(matches!(
        applied[0],
        GameEvent::ReplacementEffectApplied {
            original_amount: 1,
            replacement_amount: 3,
            ..
        }
    ));
    assert!(matches!(
        applied[1],
        GameEvent::ReplacementEffectApplied {
            original_amount: 3,
            replacement_amount: 6,
            ..
        }
    ));
    assert!(matches!(
        applied[2],
        GameEvent::ReplacementEffectApplied {
            original_amount: 6,
            replacement_amount: 24,
            ..
        }
    ));
    game.validate_invariants()
        .expect("quantity chain is invariant-safe");
}

#[test]
fn concurrent_counter_replacements_share_the_same_typed_chain() {
    let (mut game, player, target) = fixture();
    let first = resolve_to_replacement_choice(
        &mut game,
        player,
        COUNTER_SPELL,
        vec![cardbench_magic_engine::Target::Permanent(target)],
    );
    assert_eq!(first.kind, DecisionKind::Replacement);
    assert_eq!(first.replacement_candidates.len(), 2);
    let tripler_choice = quantity_choice(&first, 3);
    game.submit_policy_move(
        player,
        "replacement-chain-red",
        PolicyAction::SubmitDecision {
            decision: first.id,
            selection: DecisionSelection::Replacements(vec![tripler_choice]),
        },
    )
    .expect("choose first counter replacement");
    eprintln!(
        "counter replacement-chain trace: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(
        game.object(target)
            .expect("target lives")
            .counters
            .get(&CounterKind::Charge),
        Some(&6),
        "one remaining forced replacement follows the selected multiplier"
    );
    game.validate_invariants()
        .expect("counter chain is invariant-safe");
}
