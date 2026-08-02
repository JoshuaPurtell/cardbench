//! RED: control must be a timestamped continuous effect, not a zone move.
//!
//! This synthetic contract deliberately exercises only expansion-neutral
//! infrastructure.  It does not promote a Ravnica card: the test uses an
//! ordinary permanent source, an Aura-shaped attachment, and a temporary
//! control spell to pin the rules boundaries needed by later cards.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, ContinuousChange, Duration, Effect, Game,
    GameEvent, Keyword, ManaCost, PlayerId, Target, Zone,
};

const CONTROL_SOURCE: &str = "TST-CONTROL-SOURCE";
const CREATURE: &str = "TST-CONTROLLED-CREATURE";
const TEMPORARY_CONTROL: &str = "TST-TEMPORARY-CONTROL";
const AURA: &str = "TST-CONTROLLED-CREATURE-AURA";
const DESTROY: &str = "TST-DESTROY-CONTROL-SOURCE";
const RETURN: &str = "TST-RETURN-CONTROLLED-PERMANENT";

fn types(types: impl IntoIterator<Item = CardType>) -> BTreeSet<CardType> {
    types.into_iter().collect()
}

fn definition(
    id: &'static str,
    card_types: impl IntoIterator<Item = CardType>,
    effects: Vec<Effect>,
) -> CardDefinition {
    let card_types = types(card_types);
    let creature = card_types.contains(&CardType::Creature);
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["synthetic-control-change-contract"],
        power: creature.then_some(2),
        toughness: creature.then_some(2),
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
                    changes: vec![ContinuousChange::AddKeyword(Keyword::Haste)],
                }],
            ),
            definition(
                DESTROY,
                [CardType::Instant],
                vec![Effect::DealDamage {
                    amount: 2,
                    target: cardbench_magic_engine::TargetRequirement::Creature,
                }],
            ),
            definition(
                RETURN,
                [CardType::Instant],
                vec![Effect::ReturnTargetPermanentToHandAndLoseControllerLife { amount: 1 }],
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

fn advance_to_precombat_main(game: &mut Game) {
    for _ in 0..3 {
        if game.step == cardbench_magic_engine::Step::PrecombatMain {
            return;
        }
        let first = game.priority;
        game.pass_priority(first).expect("step first pass");
        let second = game.priority;
        game.pass_priority(second).expect("step second pass");
    }
    assert_eq!(game.step, cardbench_magic_engine::Step::PrecombatMain);
}

fn advance_through_turn_one_cleanup(game: &mut Game) {
    while game.turn == 1 {
        match game.step {
            cardbench_magic_engine::Step::DeclareAttackers
                if !game
                    .view_for_player(game.active_player)
                    .expect("attacker view")
                    .attackers_declared =>
            {
                game.declare_attackers(game.active_player, &[])
                    .expect("empty attacker declaration advances combat");
            }
            cardbench_magic_engine::Step::DeclareBlockers
                if !game
                    .view_for_player(game.next_policy_player())
                    .expect("blocker view")
                    .blockers_declared =>
            {
                game.declare_blockers(game.next_policy_player(), &[])
                    .expect("empty blocker declaration advances combat");
            }
            _ => {
                let first = game.priority;
                game.pass_priority(first).expect("step first pass");
                if game.turn == 1 && game.step != cardbench_magic_engine::Step::DeclareAttackers {
                    let second = game.priority;
                    game.pass_priority(second).expect("step second pass");
                }
            }
        }
    }
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

    assert_eq!(
        game.controller_of(target).expect("derived controller"),
        PlayerId(1)
    );
    assert!(
        game.view_for_player(PlayerId(1))
            .expect("new controller view")
            .own_battlefield
            .iter()
            .any(|card| card.id == target)
    );
    assert!(
        game.view_for_player(PlayerId(0))
            .expect("former controller view")
            .opponent_battlefield
            .iter()
            .any(|card| card.id == target)
    );
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

    assert_eq!(
        game.controller_of(target).expect("temporary controller"),
        PlayerId(0)
    );
    assert_eq!(
        game.object(target).expect("target exists").owner,
        PlayerId(1)
    );
    assert_eq!(game.zone_of(target), Some(Zone::Battlefield));
    game.validate_invariants()
        .expect("temporary control resolves into a valid game");

    advance_through_turn_one_cleanup(&mut game);

    assert_eq!(
        game.controller_of(target).expect("reverted controller"),
        PlayerId(1)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectExpired { source, target: event_target, .. }
            if *source == spell && *event_target == target
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ControllerChanged { source, target: event_target, from, to }
            if *source == spell && *event_target == target && *from == PlayerId(0) && *to == PlayerId(1)
    )));
    game.validate_invariants()
        .expect("cleanup reverts temporary control through an audited transition");
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
    advance_to_precombat_main(&mut game);
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

#[test]
fn control_source_departure_reverts_and_owner_zone_destinations_are_preserved() {
    let mut game = game();
    let source = game
        .put_on_battlefield(PlayerId(1), CONTROL_SOURCE)
        .expect("control source enters");
    let target = game
        .put_on_battlefield(PlayerId(0), CREATURE)
        .expect("owned target enters");
    let destroy = game
        .add_card(PlayerId(0), DESTROY, Zone::Hand)
        .expect("destroy spell enters hand");
    game.begin_game().expect("game begins");
    game.add_continuous_effect(
        source,
        target,
        ContinuousChange::ChangeController(PlayerId(1)),
        Duration::Permanent,
    )
    .expect("persistent control installs");
    game.clear_event_log();

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: destroy,
            targets: vec![Target::Permanent(source)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("source-destroying instant casts");
    resolve_top(&mut game);

    assert_eq!(game.zone_of(source), Some(Zone::Graveyard));
    assert_eq!(
        game.controller_of(target).expect("reverted controller"),
        PlayerId(0)
    );
    assert_eq!(
        game.object(target).expect("target exists").owner,
        PlayerId(0)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::ContinuousEffectExpired { source: event_source, target: event_target, .. }
            if *event_source == source && *event_target == target
    )));
    game.validate_invariants()
        .expect("source departure revokes only the persistent control effect");
}

#[test]
fn controlled_permanent_returns_to_its_owners_hand_not_its_controllers_hand() {
    let mut game = game();
    let source = game
        .put_on_battlefield(PlayerId(1), CONTROL_SOURCE)
        .expect("control source enters");
    let target = game
        .put_on_battlefield(PlayerId(0), CREATURE)
        .expect("owned target enters");
    let return_spell = game
        .add_card(PlayerId(0), RETURN, Zone::Hand)
        .expect("return spell enters hand");
    game.begin_game().expect("game begins");
    game.add_continuous_effect(
        source,
        target,
        ContinuousChange::ChangeController(PlayerId(1)),
        Duration::Permanent,
    )
    .expect("persistent control installs");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: return_spell,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("return spell casts");
    resolve_top(&mut game);

    assert_eq!(game.zone_of(target), Some(Zone::Hand));
    assert!(
        game.player(PlayerId(0))
            .expect("owner exists")
            .hand
            .contains(&target)
    );
    assert!(
        !game
            .player(PlayerId(1))
            .expect("controller exists")
            .hand
            .contains(&target)
    );
    assert_eq!(
        game.controller_of(target)
            .expect("base controller outside battlefield"),
        PlayerId(0)
    );
    game.validate_invariants()
        .expect("owner destination remains valid after a control effect ends");
}

#[test]
fn a_creature_that_changed_controller_this_turn_is_summoning_sick_for_its_new_controller() {
    let mut game = game();
    let spell = game
        .add_card(PlayerId(0), TEMPORARY_CONTROL, Zone::Hand)
        .expect("temporary control enters hand");
    let target = game
        .put_on_battlefield(PlayerId(1), CREATURE)
        .expect("target enters before the game");
    game.set_entered_turn_for_setup(target, 0)
        .expect("fixture makes the target old enough for its owner");
    game.begin_game().expect("game begins");
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
    advance_to_precombat_main(&mut game);
    let first = game.priority;
    game.pass_priority(first).expect("main first pass");
    let second = game.priority;
    game.pass_priority(second).expect("main second pass");
    let first = game.priority;
    game.pass_priority(first)
        .expect("beginning combat first pass");
    let second = game.priority;
    game.pass_priority(second)
        .expect("beginning combat second pass");
    assert_eq!(game.step, cardbench_magic_engine::Step::DeclareAttackers);

    assert!(
        !game
            .view_for_player(PlayerId(0))
            .expect("new controller view")
            .own_battlefield
            .iter()
            .find(|card| card.id == target)
            .expect("stolen creature is visible")
            .can_attack
    );
    assert!(game.declare_attackers(PlayerId(0), &[target]).is_err());
    assert!(game.event_log.iter().all(|event| !matches!(
        event,
        GameEvent::AttackersDeclared { attackers, .. } if attackers.contains(&target)
    )));
    game.validate_invariants()
        .expect("rejected same-turn stolen attack is atomic");
}
