//! Policy-submitted trigger coverage campaign.
//!
//! The large ordered deck matrix is intentionally broad, but its public
//! policies do not reliably draw and cast a card with a triggered ability.
//! This probe supplies a tiny deterministic game that still uses the normal
//! policy-submit ABI, then reviews the resulting canonical transcript.

use cardbench_magic_engine::{
    CastRequest, Color, Game, GameEvent, PlayerId, PolicyAction, Step, Zone,
};
use cardbench_magic_rav::{
    card_definitions, event_digest, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

/// Stable id for the targeted trigger-coverage campaign.
pub const RAV_TRIGGER_PROBE_ID: &str = "rav_trigger_probe_flame_kin_zealot";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TriggerProbeResult {
    pub id: &'static str,
    pub event_log: Vec<String>,
    pub digest: String,
    pub triggered_ability_count: usize,
}

impl TriggerProbeResult {
    #[must_use]
    pub fn passed(&self) -> bool {
        self.triggered_ability_count > 0
            && self
                .event_log
                .iter()
                .any(|event| event.contains("AbilityResolved"))
            && !self.event_log.iter().any(|event| {
                event.contains("PolicyMoveRejected")
                    || event.contains("EngineWeaknessRevealed")
                    || event.contains("InvariantViolation")
            })
    }
}

/// Runs a policy-submitted cast whose ETB trigger must be placed and resolved.
/// The four lands and target creature are fixture setup; the cast, priority
/// passes, and trigger resolution all cross `submit_policy_move`.
pub fn run_rav_trigger_probe() -> Result<TriggerProbeResult, String> {
    let mut game = Game::new_with_all_bindings_and_triggers(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .map_err(|error| error.to_string())?;
    let zealot = game
        .add_card(PlayerId(0), "RAV-FLAME-KIN-ZEALOT", Zone::Hand)
        .map_err(|error| error.to_string())?;
    game.put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .map_err(|error| error.to_string())?;
    let mountains = (0..3)
        .map(|_| game.put_on_battlefield(PlayerId(0), "RAV-MOUNTAIN"))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    let plains = game
        .put_on_battlefield(PlayerId(0), "RAV-PLAINS")
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    game.validate_invariants()
        .map_err(|error| format!("after begin: {error}"))?;
    while game.step != Step::PrecombatMain {
        let player = game.next_policy_player();
        game.submit_policy_move(player, "probe.trigger.v1", PolicyAction::PassPriority)
            .map_err(|error| format!("advance to main: {error}"))?;
    }
    for mountain in mountains {
        game.activate_mana_ability(PlayerId(0), mountain, Color::Red)
            .map_err(|error| format!("activate mountain: {error}"))?;
    }
    game.activate_mana_ability(PlayerId(0), plains, Color::White)
        .map_err(|error| format!("activate plains: {error}"))?;
    game.submit_policy_move(
        PlayerId(0),
        "probe.trigger.v1",
        PolicyAction::Cast(CastRequest {
            card: zealot,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        }),
    )
    .map_err(|error| format!("cast trigger source: {error}"))?;
    game.submit_policy_move(PlayerId(0), "probe.trigger.v1", PolicyAction::PassPriority)
        .map_err(|error| format!("pass after cast: {error}"))?;
    game.submit_policy_move(PlayerId(1), "probe.trigger.v1", PolicyAction::PassPriority)
        .map_err(|error| format!("resolve source spell: {error}"))?;
    game.submit_policy_move(PlayerId(0), "probe.trigger.v1", PolicyAction::PassPriority)
        .map_err(|error| format!("pass trigger priority: {error}"))?;
    game.submit_policy_move(PlayerId(1), "probe.trigger.v1", PolicyAction::PassPriority)
        .map_err(|error| format!("resolve trigger: {error}"))?;
    game.validate_invariants()
        .map_err(|error| error.to_string())?;
    let event_log = game.canonical_event_log();
    let triggered_ability_count = game
        .event_log
        .iter()
        .filter(|event| matches!(event, GameEvent::TriggeredAbilityStacked { .. }))
        .count();
    Ok(TriggerProbeResult {
        id: RAV_TRIGGER_PROBE_ID,
        digest: event_digest(&event_log),
        event_log,
        triggered_ability_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trigger_probe_is_policy_submitted_and_fail_closed() {
        let result = run_rav_trigger_probe().expect("trigger probe runs");
        assert!(result.passed(), "{result:#?}");
        assert_eq!(result.triggered_ability_count, 1);
        assert!(
            result
                .event_log
                .iter()
                .any(|event| event.contains("TriggeredAbilityStacked"))
        );
        assert!(
            result
                .event_log
                .iter()
                .any(|event| event.contains("AbilityResolved"))
        );
    }
}
