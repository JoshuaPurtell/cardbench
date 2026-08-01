//! Red regression: a target that leaves and re-enters is a new object.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    BasicLandManaAbilityActivation, BasicLandType, BasicLandTypeBinding, CardDefinition, CardType,
    CastPaymentManaAbility, CastRequest, Color, Effect, Game, ManaCost, ManaPaymentSelection,
    PlayerId, Target, Zone,
};

const TARGET: &str = "STACK-INCARNATION-TARGET";
const GROWTH: &str = "STACK-INCARNATION-GROWTH";
const DESTROY: &str = "STACK-INCARNATION-DESTROY";
const RETURN: &str = "STACK-INCARNATION-RETURN";
const FOREST: &str = "STACK-INCARNATION-FOREST";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
    mana_cost: ManaCost,
    power: Option<i16>,
    toughness: Option<i16>,
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
        supported_rules: &["stack-target-incarnation-probe"],
        power,
        toughness,
        keywords: vec![],
        effects,
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first)
        .expect("first priority pass succeeds");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second priority pass succeeds");
}

#[test]
#[allow(clippy::too_many_lines)]
fn a_reentered_target_is_not_legal_for_the_original_stack_object() {
    let caster = PlayerId(0);
    let responder = PlayerId(1);
    let mut game = Game::new_with_mana_abilities_basic_land_types_and_additional_spell_costs(
        [
            definition(
                TARGET,
                BTreeSet::from([CardType::Creature]),
                ManaCost::new(0),
                Some(1),
                Some(1),
                vec![],
            ),
            definition(
                GROWTH,
                BTreeSet::from([CardType::Instant]),
                ManaCost::new(0),
                None,
                None,
                vec![Effect::ModifyTargetPtUntilEndOfTurn {
                    power: 1,
                    toughness: 1,
                }],
            ),
            definition(
                DESTROY,
                BTreeSet::from([CardType::Instant]),
                ManaCost::new(0),
                None,
                None,
                vec![Effect::DestroyTargetNonblackCreature],
            ),
            definition(
                RETURN,
                BTreeSet::from([CardType::Instant]),
                ManaCost::new(1),
                None,
                None,
                vec![
                    Effect::ReturnTargetCreatureCardToBattlefieldWithCounterIfManaColorSpent {
                        color: Color::Red,
                    },
                ],
            ),
            CardDefinition {
                id: FOREST,
                name: FOREST,
                set_code: "TST",
                mana_cost: ManaCost::new(0),
                colors: BTreeSet::new(),
                mana_colors: BTreeSet::from([Color::Green]),
                card_types: BTreeSet::from([CardType::Land]),
                is_basic_land: true,
                supported_rules: &["stack-target-incarnation-probe"],
                power: None,
                toughness: None,
                keywords: vec![],
                effects: vec![],
            },
        ],
        2,
        [],
        [BasicLandTypeBinding {
            card_definition: FOREST,
            land_type: BasicLandType::Forest,
        }],
        [],
    )
    .expect("fixture game initializes");
    let target = game
        .put_on_battlefield(caster, TARGET)
        .expect("target enters the battlefield");
    let growth = game
        .add_card(caster, GROWTH, Zone::Hand)
        .expect("growth enters the caster's hand");
    let destroy = game
        .add_card(responder, DESTROY, Zone::Hand)
        .expect("destroy enters the responder's hand");
    let return_spell = game
        .add_card(caster, RETURN, Zone::Hand)
        .expect("return spell enters the caster's hand");
    let forest = game
        .put_on_battlefield(caster, FOREST)
        .expect("a typed basic land enters the battlefield");
    game.begin_game().expect("fixture game begins");

    game.cast_spell(
        caster,
        CastRequest {
            card: growth,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("growth targets the original incarnation");
    game.pass_priority(caster)
        .expect("caster passes to the responder");
    game.cast_spell(
        responder,
        CastRequest {
            card: destroy,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("destroy targets the original incarnation");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));

    game.cast_spell_with_mana_spend(
        caster,
        CastRequest {
            card: return_spell,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![CastPaymentManaAbility::BasicLand(
                BasicLandManaAbilityActivation {
                    land: forest,
                    color: Color::Green,
                },
            )],
        },
        ManaPaymentSelection {
            generic: vec![Color::Green],
            hybrid: vec![],
        },
    )
    .expect("return spell targets the graveyard incarnation");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(target), Some(Zone::Battlefield));

    pass_pair(&mut game);
    eprintln!(
        "target incarnation red result: target={target:?}; characteristics={:?}; events={:?}",
        game.characteristics(target),
        game.canonical_event_log(),
    );
    assert_eq!(
        game.characteristics(target)
            .expect("reentered target has characteristics")
            .power,
        Some(1),
        "the original growth target must be illegal after the card leaves and re-enters"
    );
    assert!(
        !game
            .canonical_event_log()
            .iter()
            .any(|event| event.contains("ContinuousEffectCreated")),
        "a stale target must not create a modifier on the new incarnation"
    );
    game.validate_invariants()
        .expect("incarnation regression leaves a valid game state");
}
