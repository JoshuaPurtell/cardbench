//! Red regression: a code policy must be able to submit every represented
//! spell-cost choice, rather than only the legacy deterministic cast path.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    BasicLandManaAbilityActivation, BasicLandType, BasicLandTypeBinding, CardDefinition, CardType,
    CastPaymentManaAbility, CastRequest, Color, Effect, Game, GameEvent, ManaCost,
    ManaPaymentSelection, PlayerId, PolicyAction, PolicyMoveKind, Target, Zone,
};

const X_REMOVAL: &str = "POLICY-X-REMOVAL";
const PAID_CONDITIONAL: &str = "POLICY-PAID-CONDITIONAL";
const TARGET: &str = "POLICY-COST-TARGET";
const DRAWN: &str = "POLICY-COST-DRAWN";
const SWAMP: &str = "POLICY-COST-SWAMP";
const ISLAND: &str = "POLICY-COST-ISLAND";

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
        supported_rules: &["policy-cast-payment-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

fn basic_payment(land: cardbench_magic_engine::ObjectId, color: Color) -> CastPaymentManaAbility {
    CastPaymentManaAbility::BasicLand(BasicLandManaAbilityActivation { land, color })
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
#[allow(clippy::too_many_lines)] // Full policy payment regression keeps rollback assertions together.
fn a_policy_submits_chosen_x_and_explicit_generic_mana_for_one_spell_cast() {
    let caster = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new_with_basic_land_types(
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
            basic_land(SWAMP, Color::Black),
        ],
        2,
        [BasicLandTypeBinding {
            card_definition: SWAMP,
            land_type: BasicLandType::Swamp,
        }],
    )
    .expect("fixture game initializes");
    let spell = game
        .add_card(caster, X_REMOVAL, Zone::Hand)
        .expect("spell setup");
    let target = game
        .put_on_battlefield(opponent, TARGET)
        .expect("target setup");
    let swamps = (0..3)
        .map(|_| game.put_on_battlefield(caster, SWAMP).expect("Swamp setup"))
        .collect::<Vec<_>>();
    game.begin_game().expect("fixture game begins");
    let payment_mana_abilities = swamps
        .iter()
        .map(|land| basic_payment(*land, Color::Black))
        .collect::<Vec<_>>();

    let events_before_rejection = game.event_log.clone();
    let rejected = game.submit_policy_move(
        caster,
        "test.policy-cost.v1",
        PolicyAction::CastWithPayment {
            request: CastRequest {
                card: spell,
                targets: vec![Target::Permanent(target)],
                convoke: vec![],
                payment_mana_abilities: payment_mana_abilities.clone(),
            },
            chosen_x: Some(2),
            mana_selection: ManaPaymentSelection {
                generic: vec![Color::Black],
                hybrid: vec![],
            },
        },
    );
    assert!(
        rejected.is_err(),
        "an incomplete policy payment must reject"
    );
    assert_eq!(game.zone_of(spell), Some(Zone::Hand));
    assert_eq!(game.zone_of(target), Some(Zone::Battlefield));
    assert_eq!(game.players[caster.0].mana_pool.amount(Color::Black), 0);
    assert!(
        swamps
            .iter()
            .all(|land| { !game.object(*land).expect("fixture land persists").tapped })
    );
    assert!(game.stack.is_empty());
    assert_eq!(game.event_log, events_before_rejection);

    game.submit_policy_move(
        caster,
        "test.policy-cost.v1",
        PolicyAction::CastWithPayment {
            request: CastRequest {
                card: spell,
                targets: vec![Target::Permanent(target)],
                convoke: vec![],
                payment_mana_abilities,
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
        game.event_log.iter().find(|event| matches!(event, GameEvent::SpellManaPaid { .. })),
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
    let mut game = Game::new_with_basic_land_types(
        [
            definition(
                PAID_CONDITIONAL,
                ManaCost::with_colors(1, [Color::Black]),
                BTreeSet::from([CardType::Instant]),
                vec![Effect::DrawControllerIfManaColorSpent { color: Color::Blue }],
            ),
            definition(
                DRAWN,
                ManaCost::new(0),
                BTreeSet::from([CardType::Artifact]),
                vec![],
            ),
            basic_land(SWAMP, Color::Black),
            basic_land(ISLAND, Color::Blue),
        ],
        2,
        [
            BasicLandTypeBinding {
                card_definition: SWAMP,
                land_type: BasicLandType::Swamp,
            },
            BasicLandTypeBinding {
                card_definition: ISLAND,
                land_type: BasicLandType::Island,
            },
        ],
    )
    .expect("fixture game initializes");
    let spell = game
        .add_card(caster, PAID_CONDITIONAL, Zone::Hand)
        .expect("spell setup");
    let drawn = game
        .add_card(caster, DRAWN, Zone::Library)
        .expect("library setup");
    let swamp = game.put_on_battlefield(caster, SWAMP).expect("Swamp setup");
    let island = game
        .put_on_battlefield(caster, ISLAND)
        .expect("Island setup");
    game.begin_game().expect("fixture game begins");

    game.submit_policy_move(
        caster,
        "test.policy-cost.v1",
        PolicyAction::CastWithPayment {
            request: CastRequest {
                card: spell,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![
                    basic_payment(swamp, Color::Black),
                    basic_payment(island, Color::Blue),
                ],
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
