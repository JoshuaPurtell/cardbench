//! Red regression for expansion-neutral copy semantics.
//!
//! This describes a copy effect in terms of rules state rather than a named
//! RAV card.  A copied permanent takes only its source's copiable values;
//! counters, marked damage, and temporary effects are not values to copy.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, Color, ContinuousChange, CounterKind, Duration, Effect, Game, GameEvent, Keyword,
    ManaCost, ObjectId, PlayerId, Target, TargetRequirement, TokenSpec, Zone,
};

const SOURCE: &str = "TST-COPY-SOURCE";
const TARGET: &str = "TST-COPY-TARGET";
const COUNTER: &str = "TST-COPY-COUNTER";
const DAMAGE: &str = "TST-COPY-DAMAGE";
const DESTROY: &str = "TST-COPY-DESTROY";
const TOKEN_SPELL: &str = "TST-COPY-TOKEN";

fn definition(
    id: &'static str,
    colors: BTreeSet<Color>,
    card_types: BTreeSet<CardType>,
    power: Option<i16>,
    toughness: Option<i16>,
    keywords: Vec<Keyword>,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors,
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["copy-semantics-probe"],
        power,
        toughness,
        keywords,
        effects,
    }
}

fn game() -> Game {
    Game::new_with_all_bindings(
        vec![
            definition(
                SOURCE,
                BTreeSet::from([Color::Red]),
                BTreeSet::from([CardType::Creature]),
                Some(3),
                Some(4),
                vec![Keyword::Flying],
                vec![],
            ),
            definition(
                TARGET,
                BTreeSet::from([Color::Blue]),
                BTreeSet::from([CardType::Artifact, CardType::Creature]),
                Some(1),
                Some(1),
                vec![],
                vec![],
            ),
            definition(
                COUNTER,
                BTreeSet::from([Color::Green]),
                BTreeSet::from([CardType::Instant]),
                None,
                None,
                vec![],
                vec![Effect::AddCountersToTarget {
                    counter: CounterKind::PlusOnePlusOne,
                    amount: 2,
                }],
            ),
            definition(
                DAMAGE,
                BTreeSet::from([Color::Red]),
                BTreeSet::from([CardType::Instant]),
                None,
                None,
                vec![],
                vec![Effect::DealDamage {
                    amount: 1,
                    target: TargetRequirement::Creature,
                }],
            ),
            definition(
                DESTROY,
                BTreeSet::from([Color::Black]),
                BTreeSet::from([CardType::Instant]),
                None,
                None,
                vec![],
                vec![Effect::DestroyTargetNonblackCreature],
            ),
            definition(
                TOKEN_SPELL,
                BTreeSet::from([Color::Green]),
                BTreeSet::from([CardType::Instant]),
                None,
                None,
                vec![],
                vec![Effect::CreateToken {
                    token: TokenSpec::white_spirit(),
                    count: 1,
                }],
            ),
        ],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: SOURCE,
            ability: ActivatedAbility {
                id: "copied-ping",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::DealDamageController { amount: 1 }],
            },
        }],
    )
    .expect("copy-semantics fixture initializes")
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player resolves");
}

fn cast_and_resolve(game: &mut Game, player: PlayerId, card: ObjectId, target: ObjectId) {
    game.cast_spell(
        player,
        CastRequest {
            card,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("fixture spell casts");
    resolve_top(game);
}

fn give_priority_to(game: &mut Game, player: PlayerId) {
    if game.priority != player {
        let current = game.priority;
        game.pass_priority(current)
            .expect("current priority holder yields to the other player");
    }
    assert_eq!(game.priority, player, "requested player receives priority");
}

#[test]
#[allow(clippy::too_many_lines)] // The regression asserts one complete copy/lifecycle receipt trace.
fn copied_card_uses_source_copiable_values_not_runtime_state_or_old_incarnations() {
    let mut game = game();
    let source = game
        .put_on_battlefield(PlayerId(0), SOURCE)
        .expect("source enters battlefield");
    let target = game
        .put_on_battlefield(PlayerId(0), TARGET)
        .expect("target enters battlefield");
    let counter = game
        .add_card(PlayerId(0), COUNTER, Zone::Hand)
        .expect("counter spell enters hand");
    let damage = game
        .add_card(PlayerId(0), DAMAGE, Zone::Hand)
        .expect("damage spell enters hand");
    let destroy = game
        .add_card(PlayerId(1), DESTROY, Zone::Hand)
        .expect("destroy spell enters hand");
    let target_destroy = game
        .add_card(PlayerId(1), DESTROY, Zone::Hand)
        .expect("second destroy spell enters hand");
    game.begin_game().expect("game begins");

    cast_and_resolve(&mut game, PlayerId(0), counter, source);
    cast_and_resolve(&mut game, PlayerId(0), damage, source);
    game.add_continuous_effect(
        source,
        source,
        ContinuousChange::ModifyPowerToughness {
            power: 2,
            toughness: -1,
        },
        Duration::EndOfTurn(game.turn),
    )
    .expect("source receives a temporary modifier");
    assert_eq!(
        (
            game.characteristics(source).expect("source exists").power,
            game.characteristics(source)
                .expect("source exists")
                .toughness
        ),
        (Some(7), Some(5)),
        "the source now proves counters and temporary effects are live runtime state"
    );
    assert_eq!(game.object(source).expect("source exists").damage, 1);

    game.clear_event_log();
    game.copy_permanent(target, source)
        .expect("copying a battlefield permanent succeeds");

    let copied = game.characteristics(target).expect("copied target exists");
    assert_eq!(copied.colors, BTreeSet::from([Color::Red]));
    assert_eq!(copied.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((copied.power, copied.toughness), (Some(3), Some(4)));
    assert_eq!(copied.keywords, vec![Keyword::Flying]);
    assert!(
        game.object(target)
            .expect("target exists")
            .counters
            .is_empty()
    );
    assert_eq!(game.object(target).expect("target exists").damage, 0);
    assert_eq!(
        game.card_definition(target)
            .expect("effective definition")
            .id,
        SOURCE
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PermanentCopied { source: copied_source, target: copied_target, .. }
            if *copied_source == source && *copied_target == target
    )));

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: target,
            ability_id: "copied-ping",
            targets: vec![],
            additional_tap_creatures: vec![],
            sacrifice_sources: vec![],
            discard_cards: vec![],
        },
    )
    .expect("copied definition exposes its definition-bound ability");
    resolve_top(&mut game);
    assert_eq!(game.players[0].life, 19);

    give_priority_to(&mut game, PlayerId(1));
    cast_and_resolve(&mut game, PlayerId(1), destroy, source);
    assert_eq!(game.zone_of(source), Some(Zone::Graveyard));
    assert_eq!(
        game.card_definition(target)
            .expect("copy survives source departure")
            .id,
        SOURCE
    );

    give_priority_to(&mut game, PlayerId(1));
    cast_and_resolve(&mut game, PlayerId(1), target_destroy, target);
    assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
    let reverted = game
        .characteristics(target)
        .expect("card remains inspectable in graveyard");
    assert_eq!(reverted.colors, BTreeSet::from([Color::Blue]));
    assert_eq!(
        reverted.card_types,
        BTreeSet::from([CardType::Artifact, CardType::Creature])
    );
    assert_eq!((reverted.power, reverted.toughness), (Some(1), Some(1)));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::PermanentCopyExpired { target: expired, .. } if *expired == target
    )));
    println!(
        "copy lifecycle trace: {:?}",
        game.event_log
            .iter()
            .filter(|event| matches!(
                event,
                GameEvent::PermanentCopied { .. }
                    | GameEvent::AbilityActivated { .. }
                    | GameEvent::DamageDealtToPlayer { .. }
                    | GameEvent::CardDestroyed { .. }
                    | GameEvent::PermanentCopyExpired { .. }
            ))
            .collect::<Vec<_>>()
    );
    game.validate_invariants()
        .expect("copy lifecycle remains valid");
}

#[test]
fn a_card_can_copy_a_token_without_becoming_a_token() {
    let mut game = game();
    let target = game
        .put_on_battlefield(PlayerId(0), TARGET)
        .expect("target enters battlefield");
    let token_spell = game
        .add_card(PlayerId(0), TOKEN_SPELL, Zone::Hand)
        .expect("token spell enters hand");
    game.begin_game().expect("game begins");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: token_spell,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("token spell casts");
    resolve_top(&mut game);
    let token = *game.players[0]
        .battlefield
        .iter()
        .find(|card| {
            game.object(**card)
                .expect("battlefield object")
                .token
                .is_some()
        })
        .expect("spirit token exists");

    game.copy_permanent(target, token)
        .expect("a card copies the token's copiable values");
    let copied = game.characteristics(target).expect("target exists");
    assert_eq!(copied.colors, BTreeSet::from([Color::White]));
    assert_eq!(copied.card_types, BTreeSet::from([CardType::Creature]));
    assert_eq!((copied.power, copied.toughness), (Some(1), Some(1)));
    assert_eq!(copied.keywords, vec![Keyword::Flying]);
    assert!(game.object(target).expect("target exists").token.is_none());
    assert!(
        game.card_definition(target).is_err(),
        "a card copying token values must not retain its physical card's definition-bound abilities"
    );
    game.validate_invariants()
        .expect("token-copy state remains valid");
}
