//! Red regression for a cast trigger that observes a virtual spell copy.
//!
//! The observed copy can be countered while its cast trigger is on the stack.
//! The trigger's "exile it" instruction must then resolve as a no-op rather
//! than rejecting the legal stack history.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, Color, Effect, Game, GameEvent, ManaCost, ObjectId, PlayerId, Target,
    TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const EYE: &str = "TST-EXILED-VIRTUAL-EYE";
const PING: &str = "TST-EXILED-VIRTUAL-PING";
const COUNTERER: &str = "TST-EXILED-VIRTUAL-COUNTERER";
const EYE_ABILITY: &str = "exile-and-copy";
const COUNTER_ABILITY: &str = "counter-target-spell";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
    effects: Vec<Effect>,
) -> CardDefinition {
    let is_creature = card_types.contains(&CardType::Creature);
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["exiled-virtual-copy-departure-trigger-red"],
        power: is_creature.then_some(1),
        toughness: is_creature.then_some(1),
        keywords: vec![],
        effects,
    }
}

fn request(card: ObjectId, targets: Vec<Target>) -> CastRequest {
    CastRequest {
        card,
        targets,
        convoke: vec![],
        payment_mana_abilities: vec![],
    }
}

fn pass_pair(game: &mut Game) -> Result<(), cardbench_magic_engine::RulesError> {
    let first = game.priority;
    game.pass_priority(first)?;
    let second = game.priority;
    game.pass_priority(second)
}

fn decline_serial_copy(game: &mut Game, caster: PlayerId) {
    let decision = game
        .view_for_player(caster)
        .expect("caster view")
        .pending_decision
        .expect("Eye-style trigger opens serial copy decision");
    game.submit_decision(
        caster,
        decision.id,
        cardbench_magic_engine::DecisionSelection::ExiledSpellCopyCast {
            card: None,
            targets: vec![],
            mode: None,
            color: None,
        },
    )
    .expect("declining an Eye-style serial choice is legal");
}

#[test]
#[allow(clippy::too_many_lines)] // The legal response sequence is one stack-history contract.
fn departed_virtual_copy_makes_its_observing_exile_trigger_a_no_op() {
    let caster = PlayerId(0);
    let observer_controller = PlayerId(1);
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(EYE, BTreeSet::from([CardType::Enchantment]), vec![]),
            definition(
                PING,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::DealDamage {
                    amount: 1,
                    target: TargetRequirement::Player,
                }],
            ),
            definition(COUNTERER, BTreeSet::from([CardType::Creature]), vec![]),
        ],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: COUNTERER,
            ability: ActivatedAbility {
                id: COUNTER_ABILITY,
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::Spell],
                effects: vec![Effect::CounterTargetSpell],
            },
        }],
        [TriggeredAbilityBinding {
            card_definition: EYE,
            ability: TriggeredAbility {
                id: EYE_ABILITY,
                condition: TriggerCondition::AnyPlayerCastsInstantOrSorcerySpell,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::ExileCastInstantOrSorceryThenCopyExiledCards],
            },
        }],
    )
    .expect("fixture initializes");
    game.put_on_battlefield(observer_controller, EYE)
        .expect("cast observer begins on battlefield");
    let counterer = game
        .put_on_battlefield(caster, COUNTERER)
        .expect("counter ability source begins on battlefield");
    let ping = game
        .add_card(caster, PING, Zone::Hand)
        .expect("physical spell begins in hand");
    game.begin_game().expect("fixture begins");
    pass_pair(&mut game).expect("first step advances");
    pass_pair(&mut game).expect("main phase begins");

    game.cast_spell(
        caster,
        request(ping, vec![Target::Player(observer_controller)]),
    )
    .expect("physical spell casts");
    pass_pair(&mut game).expect("cast trigger resolves to its serial choice");
    let first_choice = game
        .view_for_player(caster)
        .expect("caster view")
        .pending_decision
        .expect("physical spell is offered as a template");
    game.submit_decision(
        caster,
        first_choice.id,
        cardbench_magic_engine::DecisionSelection::ExiledSpellCopyCast {
            card: Some(ping),
            targets: vec![Target::Player(observer_controller)],
            mode: None,
            color: None,
        },
    )
    .expect("casts a virtual copy");
    let virtual_copy = game
        .event_log
        .iter()
        .rev()
        .find_map(|event| match event {
            GameEvent::SpellCopied { copy, original, .. } if *original == ping => Some(*copy),
            _ => None,
        })
        .expect("copy receipt identifies the virtual spell");
    decline_serial_copy(&mut game, caster);
    assert!(
        game.stack.iter().any(|item| item.card == virtual_copy),
        "the virtual spell remains below its observed cast trigger"
    );

    game.activate_ability(
        caster,
        AbilityActivation {
            source: counterer,
            ability_id: COUNTER_ABILITY,
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Spell(virtual_copy)],
        },
    )
    .expect("activated ability can counter the observed copy in response");
    pass_pair(&mut game).expect("counter ability resolves first");
    assert!(
        !game.stack.iter().any(|item| item.card == virtual_copy),
        "the observed virtual spell copy is gone before its trigger resolves"
    );

    let result = pass_pair(&mut game);
    eprintln!(
        "departed virtual-copy trigger result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    result.expect("a departed observed virtual copy makes the trigger a no-op");
    decline_serial_copy(&mut game, caster);
    assert!(game.stack.is_empty(), "the trigger terminates normally");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCopyCountered { copy, .. } if *copy == virtual_copy
    )));
    game.validate_invariants()
        .expect("the no-op trigger preserves stack provenance");
}
