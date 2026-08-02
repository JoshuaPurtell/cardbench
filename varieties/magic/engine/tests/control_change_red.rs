//! RED: control must be a timestamped continuous effect, not a zone move.
//!
//! This synthetic contract deliberately exercises only expansion-neutral
//! infrastructure.  It does not promote a Ravnica card: the test uses an
//! ordinary permanent source, an Aura-shaped attachment, and a temporary
//! control spell to pin the rules boundaries needed by later cards.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, ContinuousChange, Duration, Effect, Game,
    GameEvent, ManaCost, PlayerId, Target, Zone,
};

const CONTROL_SOURCE: &str = "TST-CONTROL-SOURCE";
const CREATURE: &str = "TST-CONTROLLED-CREATURE";
const TEMPORARY_CONTROL: &str = "TST-TEMPORARY-CONTROL";
const AURA: &str = "TST-CONTROLLED-CREATURE-AURA";

fn types(types: impl IntoIterator<Item = CardType>) -> BTreeSet<CardType> {
    types.into_iter().collect()
}

fn definition(
    id: &'static str,
    card_types: impl IntoIterator<Item = CardType>,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: types(card_types),
        is_basic_land: false,
        supported_rules: &["synthetic-control-change-contract"],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
        effects,
    }
}

fn game() -> Game {
    Game::new(
        vec![
            definition(CONTROL_SOURCE, [CardType::Creature], vec![]),
            definition(CREATURE, [CardType::Creature], vec![]),
            definition(
                TEMPORARY_CONTROL,
                [CardType::Instant],
                vec![Effect::GainControlTargetUntilEndOfTurn],
            ),
            definition(
                AURA,
                [CardType::Enchantment],
                vec![Effect::AttachSourceToTarget {
                    target: cardbench_magic_engine::TargetRequirement::ControlledCreature,
                    changes: vec![],
                }],
            ),
        ],
        2,
    )
    .expect("synthetic control-change game initializes")
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first pass");
    let second = game.priority;
    game.pass_priority(second).expect("second pass resolves");
}

#[test]
fn control_uses_layer_two_timestamp_and_is_visible_to_the_new_controller() {
    let mut game = game();
    let source = game
        .put_on_battlefield(PlayerId(1), CONTROL_SOURCE)
        .expect("source enters");
    let target = game
        .put_on_battlefield(PlayerId(0), CREATURE)
        .expect("opponent creature enters");
    game.begin_game().expect("game begins");
    game.clear_event_log();

    game.add_continuous_effect(
        source,
        target,
        ContinuousChange::ChangeController(PlayerId(1)),
        Duration::Permanent,
    )
    .expect("control effect installs");

    assert_eq!(game.controller_of(target).expect("derived controller"), PlayerId(1));
    assert!(game
        .view_for_player(PlayerId(1))
        .expect("new controller view")
        .own_battlefield
        .iter()
        .any(|card| card.id == target));
    assert!(game
        .view_for_player(PlayerId(0))
        .expect("former controller view")
        .opponent_battlefield
        .iter()
        .any(|card| card.id == target));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ControllerChanged { source: event_source, target: event_target, from, to }
            if *event_source == source && *event_target == target && *from == PlayerId(0) && *to == PlayerId(1)
    )));
    game.validate_invariants()
        .expect("derived control leaves a valid owner-indexed battlefield");
}

#[test]
fn temporary_control_expires_without_moving_the_permanent_or_losing_ownership() {
    let mut game = game();
    let spell = game
        .add_card(PlayerId(0), TEMPORARY_CONTROL, Zone::Hand)
        .expect("temporary control enters hand");
    let target = game
        .put_on_battlefield(PlayerId(1), CREATURE)
        .expect("target enters");
    game.begin_game().expect("game begins");
    game.clear_event_log();

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("temporary control spell casts");
    resolve_top(&mut game);

    assert_eq!(game.controller_of(target).expect("temporary controller"), PlayerId(0));
    assert_eq!(game.object(target).expect("target exists").owner, PlayerId(1));
    assert_eq!(game.zone_of(target), Some(Zone::Battlefield));
    game.validate_invariants()
        .expect("temporary control resolves into a valid game");
}

#[test]
fn control_change_rechecks_controller_relative_aura_legality() {
    let mut game = game();
    let source = game
        .put_on_battlefield(PlayerId(1), CONTROL_SOURCE)
        .expect("control source enters");
    let target = game
        .put_on_battlefield(PlayerId(0), CREATURE)
        .expect("creature enters");
    let aura = game
        .add_card(PlayerId(0), AURA, Zone::Hand)
        .expect("aura enters hand");
    game.begin_game().expect("game begins");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: aura,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("controlled-creature aura casts before control changes");
    resolve_top(&mut game);
    assert_eq!(game.zone_of(aura), Some(Zone::Battlefield));
    game.clear_event_log();

    game.add_continuous_effect(
        source,
        target,
        ContinuousChange::ChangeController(PlayerId(1)),
        Duration::Permanent,
    )
    .expect("control changes after the attachment exists");

    assert_eq!(game.zone_of(aura), Some(Zone::Graveyard));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::StateBasedAction { card, reason: "Aura is not attached to a legal battlefield permanent" }
            if *card == aura
    )));
    game.validate_invariants()
        .expect("illegal controller-relative attachment is cleaned up");
}
