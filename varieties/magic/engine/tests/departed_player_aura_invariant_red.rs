//! CR 800.4a regression: a departed player's Aura must not break the
//! attachment invariant audit.
//!
//! When a player leaves the game, every object they own is removed from the
//! game rather than moved to a zone. The historical `AuraAttached` receipt for
//! such an object stays in the canonical event log forever, and the attachment
//! invariant replays that log on every subsequent transition. Resolving the
//! receipt's Aura through the live object map therefore fails with
//! `UnknownCard`, and because the audit runs inside the atomic transition, an
//! ordinary later action is rejected and rolled back.
//!
//! The land-entry life-payment audit already handles exactly this case by
//! falling back to retained departed-card provenance; the attachment audit
//! does not. This is the same defect class as the ledger's
//! `policy-cross-deck-pass-priority-stale-object`.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, ContinuousChange, Effect, Game, Keyword, ManaCost,
    PlayerId, Target, TargetRequirement, Zone,
};

const BEAR: &str = "DEPARTED-AURA-BEAR";
const SHACKLE: &str = "DEPARTED-AURA-SHACKLE";
const FINISHER: &str = "DEPARTED-AURA-FINISHER";

fn creature(id: &'static str) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["departed-player-attachment-invariant-probe"],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

/// An Aura that restricts the creature it lands on, so its binding carries a
/// nonempty change set and the invariant actually inspects it.
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
        supported_rules: &["departed-player-attachment-invariant-probe"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::AttachSourceToTarget {
            target: TargetRequirement::Creature,
            changes: vec![ContinuousChange::AddKeyword(Keyword::CannotAttackOrBlock)],
        }],
    }
}

fn finisher() -> CardDefinition {
    CardDefinition {
        id: FINISHER,
        name: FINISHER,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["departed-player-attachment-invariant-probe"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::DealDamage {
            amount: 20,
            target: TargetRequirement::Player,
        }],
    }
}

/// Three seats, so the game continues after one player is eliminated and the
/// invariant audit keeps running against the surviving state.
#[test]
fn a_departed_players_aura_receipt_does_not_reject_a_later_legal_action() {
    let doomed = PlayerId(0);
    let survivor = PlayerId(1);
    let bystander = PlayerId(2);

    let mut game = Game::new([creature(BEAR), shackle(), finisher()], 3)
        .expect("three-player fixture initializes");

    // The bystander's creature is the Aura's target, so the Aura and its
    // target are owned by different players and only the Aura departs.
    let target = game
        .add_card(bystander, BEAR, Zone::Battlefield)
        .expect("bystander's creature enters the battlefield");
    let aura = game
        .add_card(doomed, SHACKLE, Zone::Hand)
        .expect("doomed player's aura enters hand");
    let bolt = game
        .add_card(survivor, FINISHER, Zone::Hand)
        .expect("survivor's finisher enters hand");

    // Libraries, so an ordinary draw step does not end the game by decking
    // before the fixture reaches its actual subject.
    for seat in [doomed, survivor, bystander] {
        for _ in 0..8 {
            game.add_card(seat, BEAR, Zone::Library)
                .expect("library filler enters");
        }
    }

    game.begin_game().expect("game begins");
    advance_to_main(&mut game);

    game.submit_policy_move(
        doomed,
        "fixture",
        cardbench_magic_engine::PolicyAction::Cast(CastRequest {
            card: aura,
            targets: vec![Target::Permanent(target)],
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
        "the fixture must actually produce the historical attachment receipt"
    );

    // Eliminate the Aura's owner. CR 800.4a removes the Aura from the game.
    for _ in 0..8 {
        if game.priority == survivor {
            break;
        }
        let actor = game.priority;
        game.submit_policy_move(
            actor,
            "fixture",
            cardbench_magic_engine::PolicyAction::PassPriority,
        )
        .expect("passing priority toward the survivor");
    }
    game.submit_policy_move(
        survivor,
        "fixture",
        cardbench_magic_engine::PolicyAction::Cast(CastRequest {
            card: bolt,
            targets: vec![Target::Player(doomed)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        }),
    )
    .expect("the finisher is cast");

    // Pass until the lethal spell resolves. The final pass is the subject: it
    // resolves 20 damage, state-based actions remove the doomed player, CR
    // 800.4a removes their Aura from the game, and the attachment invariant
    // then replays the historical `AuraAttached` receipt against a live object
    // map that no longer contains it.
    let mut rejection = None;
    for _ in 0..8 {
        if game.stack.is_empty() {
            break;
        }
        let actor = game.next_policy_player();
        if let Err(error) = game.submit_policy_move(
            actor,
            "fixture",
            cardbench_magic_engine::PolicyAction::PassPriority,
        ) {
            rejection = Some(error);
            break;
        }
    }

    assert!(
        rejection.is_none(),
        "resolving a lethal spell must not be rejected by the attachment \
         invariant after CR 800.4a removes the departed player's aura: {rejection:?}"
    );
    assert!(
        game.players[doomed.0].lost,
        "the aura's owner must have left the game"
    );
    game.validate_invariants().unwrap_or_else(|error| {
        panic!("a departed player's aura receipt must not break the invariant audit: {error:?}")
    });

    // The audit runs inside every transition, so an ordinary later action must
    // also still be accepted.
    let actor = game.next_policy_player();
    game.submit_policy_move(
        actor,
        "fixture",
        cardbench_magic_engine::PolicyAction::PassPriority,
    )
    .unwrap_or_else(|error| {
        panic!("an ordinary pass after the departure must be accepted: {error:?}")
    });
}

/// Advances to the active player's precombat main phase, where a sorcery-speed
/// spell is legal.
fn advance_to_main(game: &mut Game) {
    for _ in 0..32 {
        if game.step == cardbench_magic_engine::Step::PrecombatMain {
            return;
        }
        let actor = game.next_policy_player();
        let pending_draw = game
            .view_for_player(actor)
            .ok()
            .and_then(|view| view.draw_replacement_decision);
        let action = pending_draw.map_or(
            cardbench_magic_engine::PolicyAction::PassPriority,
            |decision| cardbench_magic_engine::PolicyAction::Draw {
                decision,
                dredge: None,
            },
        );
        if game.submit_policy_move(actor, "fixture", action).is_err() {
            return;
        }
    }
}

fn resolve_stack(game: &mut Game) {
    for _ in 0..24 {
        if game.stack.is_empty() || game.is_game_over() {
            return;
        }
        let actor = game.next_policy_player();
        let pending_draw = game
            .view_for_player(actor)
            .ok()
            .and_then(|view| view.draw_replacement_decision);
        let action = pending_draw.map_or(
            cardbench_magic_engine::PolicyAction::PassPriority,
            |decision| cardbench_magic_engine::PolicyAction::Draw {
                decision,
                dredge: None,
            },
        );
        if game.submit_policy_move(actor, "fixture", action).is_err() {
            return;
        }
    }
}
