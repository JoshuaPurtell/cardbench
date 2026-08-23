//! CR 800.4a / 704.5m: a departing player's permanents must not strand a
//! surviving player's Aura on the battlefield.
//!
//! When a player leaves the game, every object they own leaves with them. An
//! Aura controlled by a *survivor* that was attached to one of those
//! permanents is now attached to nothing, which CR 704.5m makes a state-based
//! action: the Aura goes to its owner's graveyard.
//!
//! The engine removes the objects but never performs that cleanup on this
//! path. `lose_player` is called directly from the empty-library draw
//! (`game.rs`, the deck-out branch) rather than from the state-based-action
//! fixed point, so no SBA pass follows the departure. The live attachment
//! invariant then finds an Aura whose target no longer exists and rejects the
//! transition -- which is the very draw that caused the departure. The losing
//! player never leaves and the game cannot continue.
//!
//! This is the live-battlefield counterpart to
//! `departed-player-aura-breaks-attachment-invariant`, which repaired the
//! historical-receipt audit for the same underlying event.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, ContinuousChange, Effect, Game, Keyword, ManaCost,
    PlayerId, PolicyAction, Step, Target, TargetRequirement, Zone,
};

const BEAR: &str = "ORPHAN-AURA-BEAR";
const SHACKLE: &str = "ORPHAN-AURA-SHACKLE";

fn bear() -> CardDefinition {
    CardDefinition {
        id: BEAR,
        name: BEAR,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["departed-player-orphan-aura-probe"],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

fn shackle() -> CardDefinition {
    CardDefinition {
        id: SHACKLE,
        name: SHACKLE,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &["departed-player-orphan-aura-probe"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::AttachSourceToTarget {
            target: TargetRequirement::Creature,
            changes: vec![ContinuousChange::AddKeyword(Keyword::CannotAttackOrBlock)],
        }],
    }
}

/// The survivor enchants the doomed player's creature, then the doomed player
/// decks out. The draw that kills them must be accepted.
#[test]
fn a_survivors_aura_on_a_departing_players_creature_does_not_reject_the_draw() {
    let survivor = PlayerId(0);
    let doomed = PlayerId(1);

    let mut game = Game::new([bear(), shackle()], 2).expect("two-player fixture initializes");

    let victim = game
        .add_card(doomed, BEAR, Zone::Battlefield)
        .expect("doomed player's creature enters the battlefield");
    let aura = game
        .add_card(survivor, SHACKLE, Zone::Hand)
        .expect("survivor's aura enters hand");
    // The survivor needs cards to draw; the doomed player gets none, so their
    // first draw step empties the library and CR 704.5b removes them.
    for _ in 0..30 {
        game.add_card(survivor, BEAR, Zone::Library)
            .expect("survivor library filler enters");
    }

    game.begin_game().expect("game begins");
    advance_to_main(&mut game);

    game.submit_policy_move(
        survivor,
        "fixture",
        PolicyAction::Cast(CastRequest {
            card: aura,
            targets: vec![Target::Permanent(victim)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        }),
    )
    .expect("the aura is cast");
    resolve_stack(&mut game);
    assert!(
        game.canonical_event_log()
            .iter()
            .any(|line| line.starts_with("AuraAttached")),
        "the fixture must actually attach the aura"
    );

    // Play on until the doomed player must draw from an empty library.
    let mut rejection = None;
    for _ in 0..400 {
        if game.is_game_over() {
            break;
        }
        if let Err(error) = step_once(&mut game) {
            rejection = Some(error);
            break;
        }
    }

    assert!(
        rejection.is_none(),
        "a survivor's aura on a departing player's creature must not reject the \
         draw that removes them: {rejection:?}"
    );
    assert!(
        game.players[doomed.0].lost,
        "the doomed player must have left the game"
    );
    game.validate_invariants()
        .unwrap_or_else(|error| panic!("invariants must hold after the departure: {error:?}"));
}

fn advance_to_main(game: &mut Game) {
    for _ in 0..32 {
        if game.step == Step::PrecombatMain {
            return;
        }
        if step_once(game).is_err() {
            return;
        }
    }
}

fn resolve_stack(game: &mut Game) {
    for _ in 0..24 {
        if game.stack.is_empty() || game.is_game_over() {
            return;
        }
        if step_once(game).is_err() {
            return;
        }
    }
}

fn step_once(game: &mut Game) -> Result<(), cardbench_magic_engine::RulesError> {
    let actor = game.next_policy_player();
    let view = game.view_for_player(actor).ok();
    let action = if let Some(decision) = view
        .as_ref()
        .and_then(|view| view.draw_replacement_decision)
    {
        PolicyAction::Draw {
            decision,
            dredge: None,
        }
    } else if game.step == Step::DeclareAttackers
        && game.active_player == actor
        && view.as_ref().is_some_and(|view| !view.attackers_declared)
    {
        PolicyAction::DeclareAttackers { attackers: vec![] }
    } else if game.step == Step::DeclareBlockers
        && game.active_player != actor
        && view.as_ref().is_some_and(|view| !view.blockers_declared)
    {
        PolicyAction::DeclareBlockers {
            assignments: vec![],
        }
    } else {
        PolicyAction::PassPriority
    };
    game.submit_policy_move(actor, "fixture", action)
}
