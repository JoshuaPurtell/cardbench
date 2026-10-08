//! Red regression for Trophy-Hunter-shaped source-aware damage/death triggers.
//!
//! The observer must retain both battlefield incarnations from positive
//! committed damage. Prevention, a different damage source, a returned source
//! or recipient, and a turn boundary must not manufacture a later trigger.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, Color, CounterKind, Effect, Game, GameEvent, ManaCost, PlayerId, Target,
    TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const HUNTER: &str = "TST-SOURCE-DAMAGE-DEATH-HUNTER";
const OTHER_SOURCE: &str = "TST-SOURCE-DAMAGE-DEATH-OTHER";
const CREATURE: &str = "TST-SOURCE-DAMAGE-DEATH-CREATURE";
const DAMAGE_SHIELD: &str = "TST-SOURCE-DAMAGE-DEATH-SHIELD";
const DESTROY: &str = "TST-SOURCE-DAMAGE-DEATH-DESTROY";
const BOUNCE: &str = "TST-SOURCE-DAMAGE-DEATH-BOUNCE";
const REANIMATE: &str = "TST-SOURCE-DAMAGE-DEATH-REANIMATE";
const DAMAGE_ABILITY: &str = "deal-one-to-creature";
const DEATH_TRIGGER: &str = "damaged-creature-dies-add-counter";

fn definition(
    id: &'static str,
    card_types: impl IntoIterator<Item = CardType>,
    power: Option<i16>,
    toughness: Option<i16>,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: if matches!(id, HUNTER | OTHER_SOURCE | CREATURE) {
            BTreeSet::from([Color::Green])
        } else {
            BTreeSet::new()
        },
        mana_colors: BTreeSet::new(),
        card_types: card_types.into_iter().collect(),
        is_basic_land: false,
        supported_rules: &["source-damaged-creature-dies-trigger-red"],
        power,
        toughness,
        keywords: vec![],
        effects,
    }
}

fn fixture() -> Game {
    let damage_ability = || ActivatedAbility {
        id: DAMAGE_ABILITY,
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
    };

    Game::new_with_all_bindings_and_triggers(
        [
            definition(HUNTER, [CardType::Creature], Some(2), Some(3), vec![]),
            definition(OTHER_SOURCE, [CardType::Creature], Some(1), Some(2), vec![]),
            definition(CREATURE, [CardType::Creature], Some(2), Some(2), vec![]),
            definition(
                DAMAGE_SHIELD,
                [CardType::Instant],
                None,
                None,
                vec![Effect::AddTargetDamageShieldUntilEndOfTurn { amount: 1 }],
            ),
            definition(
                DESTROY,
                [CardType::Instant],
                None,
                None,
                vec![Effect::DestroyTargetNonblackCreature],
            ),
            definition(
                BOUNCE,
                [CardType::Instant],
                None,
                None,
                vec![Effect::ReturnTargetPermanentToHandAndLoseControllerLife { amount: 1 }],
            ),
            definition(
                REANIMATE,
                [CardType::Instant],
                None,
                None,
                vec![
                    Effect::ReturnTargetCreatureCardToBattlefieldWithCounterIfManaColorSpent {
                        color: Color::Green,
                    },
                ],
            ),
        ],
        2,
        [],
        [],
        [],
        [
            ActivatedAbilityBinding {
                card_definition: HUNTER,
                ability: damage_ability(),
            },
            ActivatedAbilityBinding {
                card_definition: OTHER_SOURCE,
                ability: damage_ability(),
            },
        ],
        [TriggeredAbilityBinding {
            card_definition: HUNTER,
            ability: TriggeredAbility {
                id: DEATH_TRIGGER,
                condition: TriggerCondition::DamagedCreatureBySourceDiesThisTurn,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::AddPlusOneCounterToSource],
            },
        }],
    )
    .expect("source-damage/death fixture constructs")
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first)
        .expect("first player passes priority");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second player resolves the stack top");
}

fn cast_targeted(
    game: &mut Game,
    player: PlayerId,
    card: cardbench_magic_engine::ObjectId,
    target: Target,
) {
    game.cast_spell(
        player,
        CastRequest {
            card,
            targets: vec![target],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("targeted fixture spell casts");
    pass_pair(game);
}

fn cast_untargeted(game: &mut Game, player: PlayerId, card: cardbench_magic_engine::ObjectId) {
    game.cast_spell(
        player,
        CastRequest {
            card,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("untargeted fixture spell casts");
    pass_pair(game);
}

fn activate_damage(
    game: &mut Game,
    source: cardbench_magic_engine::ObjectId,
    target: cardbench_magic_engine::ObjectId,
) {
    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source,
            ability_id: DAMAGE_ABILITY,
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Permanent(target)],
        },
    )
    .expect("damage ability activates");
    pass_pair(game);
}

fn battlefield_pair(
    game: &mut Game,
) -> (
    cardbench_magic_engine::ObjectId,
    cardbench_magic_engine::ObjectId,
) {
    let hunter = game
        .put_on_battlefield(PlayerId(0), HUNTER)
        .expect("observer enters before game start");
    let creature = game
        .put_on_battlefield(PlayerId(1), CREATURE)
        .expect("recipient enters before game start");
    game.begin_game().expect("fixture game begins");
    (hunter, creature)
}

fn assert_no_hunter_death_trigger(game: &Game, hunter: cardbench_magic_engine::ObjectId) {
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::TriggeredAbilityStacked {
                source,
                ability: DEATH_TRIGGER,
                ..
            } if *source == hunter
        )),
        "only the exact Trophy-Hunter-shaped source/damage/creature pair may trigger"
    );
}

fn destroy_card(
    game: &mut Game,
    card: cardbench_magic_engine::ObjectId,
    target: cardbench_magic_engine::ObjectId,
) {
    cast_targeted(game, PlayerId(0), card, Target::Permanent(target));
}

#[test]
fn positive_committed_damage_from_this_hunter_triggers_once_and_adds_a_counter() {
    let mut game = fixture();
    let (hunter, creature) = battlefield_pair(&mut game);
    let destroy = game
        .add_card(PlayerId(0), DESTROY, Zone::Hand)
        .expect("destroy spell enters hand");
    game.clear_event_log();

    activate_damage(&mut game, hunter, creature);
    assert_eq!(
        game.object(creature).expect("recipient remains").damage,
        1,
        "the activation commits one damage without killing the 2/2"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPermanent { source, permanent, amount: 1 }
            if *source == hunter && *permanent == creature
    )));

    destroy_card(&mut game, destroy, creature);
    assert_eq!(game.zone_of(creature), Some(Zone::Graveyard));
    let stacked = game
        .event_log
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::TriggeredAbilityStacked { source, ability: DEATH_TRIGGER, .. }
                    if *source == hunter
            )
        })
        .expect("the exact source's damaged-creature death trigger stacks");
    assert_eq!(
        game.object(hunter)
            .expect("hunter remains on the battlefield")
            .counters
            .get(&CounterKind::PlusOnePlusOne),
        None,
        "the counter waits for the trigger to resolve"
    );

    pass_pair(&mut game);
    assert_eq!(
        game.object(hunter)
            .expect("hunter remains on the battlefield")
            .counters
            .get(&CounterKind::PlusOnePlusOne),
        Some(&1)
    );
    assert!(game.event_log[stacked..].iter().any(|event| matches!(
        event,
        GameEvent::CounterPlaced {
            source,
            card,
            counter: CounterKind::PlusOnePlusOne,
            amount: 1,
        } if *source == hunter && *card == hunter
    )));
    game.validate_invariants()
        .expect("source-damage death trigger trace remains valid");
}

#[test]
fn fully_prevented_damage_does_not_create_source_damage_history() {
    let mut game = fixture();
    let (hunter, creature) = battlefield_pair(&mut game);
    let shield = game
        .add_card(PlayerId(0), DAMAGE_SHIELD, Zone::Hand)
        .expect("shield spell enters hand");
    let destroy = game
        .add_card(PlayerId(0), DESTROY, Zone::Hand)
        .expect("destroy spell enters hand");

    cast_targeted(&mut game, PlayerId(0), shield, Target::Permanent(creature));
    activate_damage(&mut game, hunter, creature);
    assert_eq!(
        game.object(creature).expect("recipient remains").damage,
        0,
        "the only damage point is prevented"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamagePrevented { source, target: Target::Permanent(permanent), amount: 1 }
            if *source == hunter && *permanent == creature
    )));
    assert!(!game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPermanent { source, permanent, .. }
            if *source == hunter && *permanent == creature
    )));

    destroy_card(&mut game, destroy, creature);
    assert_no_hunter_death_trigger(&game, hunter);
    game.validate_invariants()
        .expect("fully prevented damage leaves valid trigger history");
}

#[test]
fn damage_from_another_creature_does_not_satisfy_this_hunters_trigger() {
    let mut game = fixture();
    let hunter = game
        .put_on_battlefield(PlayerId(0), HUNTER)
        .expect("hunter enters before game start");
    let other = game
        .put_on_battlefield(PlayerId(0), OTHER_SOURCE)
        .expect("unrelated damage source enters before game start");
    let creature = game
        .put_on_battlefield(PlayerId(1), CREATURE)
        .expect("recipient enters before game start");
    let destroy = game
        .add_card(PlayerId(0), DESTROY, Zone::Hand)
        .expect("destroy spell enters hand");
    game.begin_game().expect("fixture game begins");

    activate_damage(&mut game, other, creature);
    destroy_card(&mut game, destroy, creature);
    assert_no_hunter_death_trigger(&game, hunter);
    game.validate_invariants()
        .expect("unrelated-source damage history stays valid");
}

#[test]
fn returned_source_incarnation_cannot_inherit_its_old_damage_observation() {
    let mut game = fixture();
    let (hunter, creature) = battlefield_pair(&mut game);
    let original_hunter_incarnation = game.object(hunter).expect("hunter exists").incarnation;
    let destroy_hunter = game
        .add_card(PlayerId(0), DESTROY, Zone::Hand)
        .expect("first destroy spell enters hand");
    let reanimate = game
        .add_card(PlayerId(0), REANIMATE, Zone::Hand)
        .expect("reanimation spell enters hand");
    let destroy_creature = game
        .add_card(PlayerId(0), DESTROY, Zone::Hand)
        .expect("second destroy spell enters hand");

    activate_damage(&mut game, hunter, creature);
    destroy_card(&mut game, destroy_hunter, hunter);
    assert_eq!(game.zone_of(hunter), Some(Zone::Graveyard));
    cast_targeted(&mut game, PlayerId(0), reanimate, Target::Permanent(hunter));
    let returned_hunter_incarnation = game.object(hunter).expect("hunter returns").incarnation;
    assert!(returned_hunter_incarnation > original_hunter_incarnation);

    destroy_card(&mut game, destroy_creature, creature);
    assert_no_hunter_death_trigger(&game, hunter);
    game.validate_invariants()
        .expect("returned source incarnation does not inherit old damage history");
}

#[test]
fn returned_creature_incarnation_does_not_inherit_damage_from_its_old_object() {
    let mut game = fixture();
    let hunter = game
        .put_on_battlefield(PlayerId(0), HUNTER)
        .expect("hunter enters before game start");
    let creature = game
        .put_on_battlefield(PlayerId(0), CREATURE)
        .expect("owner's recipient enters before game start");
    let bounce = game
        .add_card(PlayerId(0), BOUNCE, Zone::Hand)
        .expect("bounce spell enters hand");
    let destroy = game
        .add_card(PlayerId(0), DESTROY, Zone::Hand)
        .expect("destroy spell enters hand");
    game.begin_game().expect("fixture game begins");
    let original_creature_incarnation = game.object(creature).expect("creature exists").incarnation;

    activate_damage(&mut game, hunter, creature);
    cast_targeted(&mut game, PlayerId(0), bounce, Target::Permanent(creature));
    assert_eq!(game.zone_of(creature), Some(Zone::Hand));
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: creature,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("returned creature is cast again");
    pass_pair(&mut game);
    let returned_creature_incarnation =
        game.object(creature).expect("creature returns").incarnation;
    assert!(returned_creature_incarnation > original_creature_incarnation);

    destroy_card(&mut game, destroy, creature);
    assert_no_hunter_death_trigger(&game, hunter);
    game.validate_invariants()
        .expect("returned recipient incarnation does not inherit old damage history");
}

#[test]
fn source_damage_observation_expires_at_the_next_turn_boundary() {
    let mut game = fixture();
    let (hunter, creature) = battlefield_pair(&mut game);
    let destroy = game
        .add_card(PlayerId(0), DESTROY, Zone::Hand)
        .expect("destroy spell enters hand");

    activate_damage(&mut game, hunter, creature);
    let mut transitions = 0;
    while game.turn == 1 {
        let priority = game.priority;
        game.pass_priority(priority)
            .expect("empty turn advances without an action");
        transitions += 1;
        assert!(
            transitions < 64,
            "turn boundary is reached in bounded passes"
        );
    }
    assert!(game.turn > 1, "the fixture advanced into a later turn");
    while game.priority != PlayerId(0) {
        let priority = game.priority;
        game.pass_priority(priority)
            .expect("priority advances to the destroy spell's controller");
    }

    destroy_card(&mut game, destroy, creature);
    assert_no_hunter_death_trigger(&game, hunter);
    game.validate_invariants()
        .expect("turn-scoped damage history expires cleanly");
}

#[test]
fn simultaneous_source_and_recipient_departure_uses_last_known_source() {
    const SWEEP: &str = "TST-SOURCE-DAMAGE-DEATH-SWEEP";
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(HUNTER, [CardType::Creature], Some(2), Some(3), vec![]),
            definition(CREATURE, [CardType::Creature], Some(2), Some(2), vec![]),
            definition(
                SWEEP,
                [CardType::Sorcery],
                None,
                None,
                vec![Effect::DealDamageToEachCreatureAndPlayer { amount: 3 }],
            ),
        ],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: HUNTER,
            ability: ActivatedAbility {
                id: DAMAGE_ABILITY,
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
        [TriggeredAbilityBinding {
            card_definition: HUNTER,
            ability: TriggeredAbility {
                id: DEATH_TRIGGER,
                condition: TriggerCondition::DamagedCreatureBySourceDiesThisTurn,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::AddPlusOneCounterToSource],
            },
        }],
    )
    .expect("simultaneous-departure fixture constructs");
    let hunter = game
        .put_on_battlefield(PlayerId(0), HUNTER)
        .expect("hunter enters before game start");
    let creature = game
        .put_on_battlefield(PlayerId(1), CREATURE)
        .expect("recipient enters before game start");
    let sweep = game
        .add_card(PlayerId(0), SWEEP, Zone::Hand)
        .expect("sweep enters hand");
    game.begin_game().expect("fixture game begins");

    activate_damage(&mut game, hunter, creature);
    cast_untargeted(&mut game, PlayerId(0), sweep);
    assert_eq!(game.zone_of(hunter), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(creature), Some(Zone::Graveyard));
    assert!(
        game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::TriggeredAbilityStacked {
                source,
                source_incarnation,
                ability: DEATH_TRIGGER,
                ..
            } if *source == hunter && *source_incarnation == 1
        )),
        "simultaneous departure observes the still-present source's LKI"
    );
    pass_pair(&mut game);
    game.validate_invariants()
        .expect("departed observer's trigger resolves through last-known identity");
}
