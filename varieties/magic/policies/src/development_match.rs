use cardbench_magic_engine::{Color, Game, GameEvent, PlayerId, RulesError, Zone};
use cardbench_magic_rav::{card_definitions, event_digest, load_reference_decks};

use crate::{BorosTempoPolicy, CodePolicy, SelesnyaConvokePolicy};

const REQUIRED_EVENT_MARKERS: [&str; 8] = [
    "PolicyMoveSubmitted",
    "SpellCast",
    "ConvokeUsed",
    "TokenCreated",
    "PriorityPassed",
    "SpellResolved",
    "DamageDealtToPlayer",
    "LifeGained",
];
const EXPECTED_EVENT_DIGEST: &str = "fnv1a64:0fc08a289eba7208";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PolicyMatchResult {
    pub id: &'static str,
    pub event_log: Vec<String>,
    pub digest: String,
    pub life: [i64; 2],
    pub token_count: usize,
    pub policy_moves: usize,
}

/// Runs the two public reference decks through their matching Rust code policies.
///
/// This is a deliberately scripted development opening, not a complete shuffled
/// game. It proves that policy-produced moves are submitted through the same engine
/// rules path used by scenarios and provides an event-log contract for iteration.
pub fn run_rav_reference_match() -> Result<PolicyMatchResult, String> {
    validate_reference_match_fixture()?;
    let decks = load_reference_decks().map_err(|error| error.to_string())?;
    let boros = decks.iter().find(|deck| deck.id == "rav_boros_helix");
    let selesnya = decks.iter().find(|deck| deck.id == "rav_selesnya_convoke");
    if boros.is_none() || selesnya.is_none() {
        return Err("reference policy match requires the two named RAV deck fixtures".to_owned());
    }
    let mut game = Game::new(card_definitions(), 2).map_err(rules_error)?;
    game.set_shuffle_seed(73).map_err(rules_error)?;
    game.add_card(PlayerId(0), "RAV-LIGHTNING-HELIX", Zone::Hand)
        .map_err(rules_error)?;
    game.add_card(PlayerId(1), "RAV-SCATTER-THE-SEEDS", Zone::Hand)
        .map_err(rules_error)?;
    for _ in 0..3 {
        game.put_on_battlefield(PlayerId(1), "RAV-GOLGARI-BROWNSCALE")
            .map_err(rules_error)?;
    }
    game.grant_mana(PlayerId(0), Color::White, 1)
        .map_err(rules_error)?;
    game.grant_mana(PlayerId(0), Color::Red, 1)
        .map_err(rules_error)?;
    game.grant_mana(PlayerId(1), Color::Red, 2)
        .map_err(rules_error)?;
    game.clear_event_log();

    let mut boros = BorosTempoPolicy::new(PlayerId(0));
    let mut selesnya = SelesnyaConvokePolicy::new(PlayerId(1));
    for move_number in 1..=8 {
        let player = game.next_policy_player();
        let result = if player == PlayerId(0) {
            submit_policy_move(&mut game, &mut boros, player)
        } else {
            submit_policy_move(&mut game, &mut selesnya, player)
        };
        result.map_err(|error| format!("scripted opening move {move_number}: {error}"))?;
    }

    let event_log = game.canonical_event_log();
    for marker in REQUIRED_EVENT_MARKERS {
        if !event_log.iter().any(|event| event.contains(marker)) {
            return Err(format!("reference policy match has no `{marker}` event"));
        }
    }
    let policy_moves = game
        .event_log
        .iter()
        .filter(|event| matches!(event, GameEvent::PolicyMoveSubmitted { .. }))
        .count();
    let token_count = game
        .players
        .iter()
        .flat_map(|player| player.battlefield.iter())
        .filter(|card| {
            game.object(**card)
                .is_ok_and(|object| object.token.is_some())
        })
        .count();
    let result = PolicyMatchResult {
        id: "rav_boros_vs_selesnya_development",
        digest: event_digest(&event_log),
        event_log,
        life: [game.players[0].life, game.players[1].life],
        token_count,
        policy_moves,
    };
    if result.life != [23, 17] || result.token_count != 3 || result.policy_moves != 8 {
        return Err(format!(
            "reference policy match postcondition failed: {result:?}"
        ));
    }
    if result.digest != EXPECTED_EVENT_DIGEST {
        return Err(format!(
            "reference policy match digest mismatch: expected {EXPECTED_EVENT_DIGEST}, got {}",
            result.digest
        ));
    }
    Ok(result)
}

/// Ensures the checked-in public fixture still names the executable contract that
/// this lightweight reference runner enforces. The exact event sequence is then
/// frozen by the verified digest above.
fn validate_reference_match_fixture() -> Result<(), String> {
    let fixture = include_str!("../reference_match.toml");
    for expected in [
        "schema_version = \"cardbench.magic.policy-match.v1\"",
        "id = \"rav_boros_vs_selesnya_development\"",
        "deck_p0 = \"rav_boros_helix\"",
        "deck_p1 = \"rav_selesnya_convoke\"",
        "policy_p0 = \"rav.boros-tempo.v1\"",
        "policy_p1 = \"rav.selesnya-convoke.v1\"",
        "life = [23, 17]",
        "token_count = 3",
        "policy_moves = 8",
        "digest = \"fnv1a64:0fc08a289eba7208\"",
    ] {
        if !fixture.contains(expected) {
            return Err(format!("reference_match.toml is missing `{expected}`"));
        }
    }
    for marker in REQUIRED_EVENT_MARKERS {
        let quoted = format!("\"{marker}\"");
        if !fixture.contains(&quoted) {
            return Err(format!("reference_match.toml is missing event `{marker}`"));
        }
    }
    Ok(())
}

fn submit_policy_move<P: CodePolicy>(
    game: &mut Game,
    policy: &mut P,
    player: PlayerId,
) -> Result<(), String> {
    let view = game.view_for_player(player).map_err(rules_error)?;
    let action = if let Some(action) = policy.propose_pending_decision(&view) {
        action
    } else if view.draw_replacement_pending {
        policy.propose_draw_replacement(&view)
    } else if view.private_library_choice.is_some() {
        policy.propose_private_library_choice(&view)
    } else if view.private_opponent_library_choice.is_some() {
        policy.propose_private_opponent_library_choice(&view)
    } else {
        policy.propose_move(&view)
    };
    game.submit_policy_move(player, policy.id(), action)
        .map_err(|error| {
            format!(
                "{} move for player {} at {:?} (priority {}, decision {}): {error}",
                policy.id(),
                player.0,
                view.step,
                view.priority.0,
                view.decision_player.0,
            )
        })
}

#[allow(clippy::needless_pass_by_value)] // `Result::map_err` provides an owned error.
fn rules_error(error: RulesError) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_policies_submit_expected_engine_moves() {
        let first = run_rav_reference_match().expect("reference policy development match");
        let second = run_rav_reference_match().expect("deterministic replay");
        assert_eq!(first, second);
        assert_eq!(first.life, [23, 17]);
        assert_eq!(first.token_count, 3);
        assert_eq!(first.policy_moves, 8);
    }
}
