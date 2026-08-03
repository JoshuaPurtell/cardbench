//! Red regression: a returned source's new controller cannot rewrite the
//! controller-relative prevention facts of its older activated ability.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, BasicLandManaAbilityActivation,
    BasicLandType, BasicLandTypeBinding, CardDefinition, CardType, CastPaymentManaAbility,
    CastRequest, Color, ContinuousChange, Effect, Game, GameEvent, Keyword, ManaCost,
    ManaPaymentSelection, PlayerId, StaticContinuousEffectBinding, Target, TargetRequirement, Zone,
};

const LIGHT: &str = "TST-RETURNED-SOURCE-CONTROLLER-LIGHT";
const PINGER: &str = "TST-RETURNED-SOURCE-CONTROLLER-PINGER";
const TARGET: &str = "TST-RETURNED-SOURCE-CONTROLLER-TARGET";
const KILL: &str = "TST-RETURNED-SOURCE-CONTROLLER-KILL";
const RETURN: &str = "TST-RETURNED-SOURCE-CONTROLLER-RETURN";
const STEAL: &str = "TST-RETURNED-SOURCE-CONTROLLER-STEAL";
const FOREST: &str = "TST-RETURNED-SOURCE-CONTROLLER-FOREST";

fn definition(
    id: &'static str,
    card_type: CardType,
    colors: BTreeSet<Color>,
    power: Option<i16>,
    toughness: Option<i16>,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors,
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["returned-source-controller-lki-red"],
        power,
        toughness,
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
#[allow(clippy::too_many_lines)] // One leave-return-control response chain owns this provenance boundary.
fn returned_source_uses_its_old_controller_for_friendly_damage_prevention() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut forest = definition(FOREST, CardType::Land, BTreeSet::new(), None, None, vec![]);
    forest.is_basic_land = true;
    forest.mana_colors = BTreeSet::from([Color::Green]);
    let mut return_definition = definition(
        RETURN,
        CardType::Instant,
        BTreeSet::new(),
        None,
        None,
        vec![
            Effect::ReturnTargetCreatureCardToBattlefieldWithCounterIfManaColorSpent {
                color: Color::Red,
            },
        ],
    );
    return_definition.mana_cost = ManaCost::new(1);
    let mut game = Game::new_with_all_bindings_and_static_continuous_effects(
        [
            definition(
                LIGHT,
                CardType::Enchantment,
                BTreeSet::from([Color::White]),
                None,
                None,
                vec![],
            ),
            definition(
                PINGER,
                CardType::Creature,
                BTreeSet::new(),
                Some(1),
                Some(1),
                vec![],
            ),
            definition(
                TARGET,
                CardType::Creature,
                BTreeSet::new(),
                Some(1),
                Some(3),
                vec![],
            ),
            definition(
                KILL,
                CardType::Instant,
                BTreeSet::new(),
                None,
                None,
                vec![Effect::DestroyTargetNonblackCreature],
            ),
            return_definition,
            definition(
                STEAL,
                CardType::Instant,
                BTreeSet::from([Color::Blue]),
                None,
                None,
                vec![Effect::GainControlTargetUntilEndOfTurn],
            ),
            forest,
        ],
        2,
        [],
        [BasicLandTypeBinding {
            card_definition: FOREST,
            land_type: BasicLandType::Forest,
        }],
        [],
        [ActivatedAbilityBinding {
            card_definition: PINGER,
            ability: ActivatedAbility {
                id: "ping",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::Creature],
                effects: vec![Effect::DealDamage {
                    amount: 1,
                    target: TargetRequirement::Creature,
                }],
            },
        }],
        [StaticContinuousEffectBinding {
            card_definition: LIGHT,
            change: ContinuousChange::ControlledCreaturesAddKeyword(
                Keyword::PreventDamageFromControlledSources,
            ),
        }],
    )
    .expect("fixture initializes");
    game.put_on_battlefield(controller, LIGHT)
        .expect("prevention source enters");
    let pinger = game
        .put_on_battlefield(controller, PINGER)
        .expect("pinger enters");
    let target = game
        .put_on_battlefield(controller, TARGET)
        .expect("protected target enters");
    let forest = game
        .put_on_battlefield(controller, FOREST)
        .expect("forest enters");
    let kill = game
        .add_card(opponent, KILL, Zone::Hand)
        .expect("kill enters hand");
    let return_spell = game
        .add_card(controller, RETURN, Zone::Hand)
        .expect("return enters hand");
    let steal = game
        .add_card(opponent, STEAL, Zone::Hand)
        .expect("steal enters hand");
    game.begin_game().expect("game begins");
    let old_incarnation = game.object(pinger).expect("pinger exists").incarnation;

    game.activate_ability(
        controller,
        AbilityActivation {
            source: pinger,
            ability_id: "ping",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("old pinger activates");
    game.pass_priority(controller)
        .expect("controller passes to removal");
    game.cast_spell(
        opponent,
        CastRequest {
            card: kill,
            targets: vec![Target::Permanent(pinger)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent destroys old pinger");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(pinger), Some(Zone::Graveyard));

    game.cast_spell_with_mana_spend(
        controller,
        CastRequest {
            card: return_spell,
            targets: vec![Target::Permanent(pinger)],
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
    .expect("pinger returns before its old ability resolves");
    pass_pair(&mut game);
    let returned_incarnation = game.object(pinger).expect("pinger returns").incarnation;
    assert!(returned_incarnation > old_incarnation);

    game.pass_priority(controller)
        .expect("controller passes to control-change response");
    game.cast_spell(
        opponent,
        CastRequest {
            card: steal,
            targets: vec![Target::Permanent(pinger)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("opponent steals only the returned pinger incarnation");
    pass_pair(&mut game);
    assert_eq!(game.controller_of(pinger), Ok(opponent));

    pass_pair(&mut game);
    eprintln!(
        "returned source controller LKI red trace: old={old_incarnation}; returned={returned_incarnation}; target_damage={:?}; events={:?}",
        game.object(target).map(|object| object.damage),
        game.canonical_event_log(),
    );
    assert_eq!(
        game.object(target).expect("target remains").damage,
        0,
        "the old ability's source was controlled by the protected target's controller"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamagePrevented {
            source,
            target: Target::Permanent(permanent),
            amount: 1,
        } if *source == pinger && *permanent == target
    )));
    game.validate_invariants()
        .expect("old source controller provenance remains valid");
}
