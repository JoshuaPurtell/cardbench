//! Red regression: a code policy must be able to submit every represented
//! spell-cost choice, rather than only the legacy deterministic cast path.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, GameEvent, ManaCost,
    ManaPaymentSelection, PlayerId, PolicyAction, PolicyMoveKind, Target, Zone,
};

const X_REMOVAL: &str = "POLICY-X-REMOVAL";
const PAID_CONDITIONAL: &str = "POLICY-PAID-CONDITIONAL";
const TARGET: &str = "POLICY-COST-TARGET";
const DRAWN: &str = "POLICY-COST-DRAWN";

fn definition(
    id: &'static str,
    mana_cost: ManaCost,
    card_types: BTreeSet<CardType>,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost,
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["policy-cast-payment-red"],
        power: (id == TARGET).then_some(2),
        toughness: (id == TARGET).then_some(2),
        keywords: vec![],
        effects,
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.submit_policy_move(first, "test.policy-cost.v1", PolicyAction::PassPriority)
        .expect("first priority pass");
    let second = game.priority;
    game.submit_policy_move(second, "test.policy-cost.v1", PolicyAction::PassPriority)
        .expect("second priority pass");
}

#[test]
fn a_policy_submits_chosen_x_and_explicit_generic_mana_for_one_spell_cast() {
    let caster = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                X_REMOVAL,
                ManaCost::with_colors(0, [Color::Black]),
                BTreeSet::from([CardType::Instant]),
                vec![Effect::DestroyTargetCreatureWithManaValueAtMostChosenX],
            ),
            definition(
                TARGET,
                ManaCost::with_colors(1, [Color::Green]),
                BTreeSet::from([CardType::Creature]),
                vec![],
            ),
        ],
        2,
    )
    .expect("fixture game initializes");
    let spell = game
        .add_card(caster, X_REMOVAL, Zone::Hand)
        .expect("spell setup");
    let target = game
        .put_on_battlefield(opponent, TARGET)
        .expect("target setup");
    game.grant_mana(caster, Color::Black, 3)
        .expect("fixture mana fits");
    game.begin_game().expect("fixture game begins");

    game.submit_policy_move(
        caster,
        "test.policy-cost.v1",
        PolicyAction::CastWithPayment {
            request: CastRequest {
                card: spell,
                targets: vec![Target::Permanent(target)],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: Some(2),
            mana_selection: ManaPaymentSelection {
                generic: vec![Color::Black, Color::Black],
                hybrid: vec![],
            },
        },
    )
    .expect("policy-provided chosen X and mana selection cast the spell");
    assert!(matches!(
        game.event_log.first(),
        Some(GameEvent::SpellManaPaid { player, card, colors })
            if *player == caster
                && *card == spell
                && colors == &vec![Color::Black, Color::Black, Color::Black]
    ));
    assert!(matches!(
        game.event_log.last(),
        Some(GameEvent::PolicyMoveSubmitted {
            player,
            kind: PolicyMoveKind::Cast,
            ..
        }) if *player == caster
    ));

    pass_pair(&mut game);
    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    game.validate_invariants()
        .expect("policy-selected X remains stack provenance");
}

#[test]
fn a_policy_submits_explicit_spent_mana_for_a_conditional_spell() {
    let caster = PlayerId(0);
    let mut game = Game::new(
        [
            definition(
                PAID_CONDITIONAL,
                ManaCost::with_colors(1, [Color::Black]),
                BTreeSet::from([CardType::Instant]),
                vec![Effect::DrawControllerIfManaColorSpent {
                    color: Color::Blue,
                }],
            ),
            definition(
                DRAWN,
                ManaCost::new(0),
                BTreeSet::from([CardType::Artifact]),
                vec![],
            ),
        ],
        2,
    )
    .expect("fixture game initializes");
    let spell = game
        .add_card(caster, PAID_CONDITIONAL, Zone::Hand)
        .expect("spell setup");
    let drawn = game
        .add_card(caster, DRAWN, Zone::Library)
        .expect("library setup");
    game.grant_mana(caster, Color::Black, 1)
        .expect("black fixture mana fits");
    game.grant_mana(caster, Color::Blue, 1)
        .expect("blue fixture mana fits");
    game.begin_game().expect("fixture game begins");

    game.submit_policy_move(
        caster,
        "test.policy-cost.v1",
        PolicyAction::CastWithPayment {
            request: CastRequest {
                card: spell,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection {
                generic: vec![Color::Blue],
                hybrid: vec![],
            },
        },
    )
    .expect("policy-provided mana selection casts the conditional spell");

    pass_pair(&mut game);
    assert_eq!(game.zone_of(drawn), Some(Zone::Hand));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellManaPaid { player, card, colors }
            if *player == caster
                && *card == spell
                && colors == &vec![Color::Black, Color::Blue]
    )));
    game.validate_invariants()
        .expect("policy-selected spent mana remains stack provenance");
}
