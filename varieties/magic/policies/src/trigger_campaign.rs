//! Policy-submitted trigger coverage campaign.
//!
//! The large ordered deck matrix is intentionally broad, but its public
//! policies do not reliably draw and cast a card with a triggered ability.
//! This probe supplies a tiny deterministic game that still uses the normal
//! policy-submit ABI, then reviews the resulting canonical transcript.

use cardbench_magic_engine::{
    CastRequest, Color, DecisionSelection, Game, GameEvent, ObjectId, PlayerId, PolicyAction, Step,
    Zone,
};
use cardbench_magic_rav::{
    card_definitions, event_digest, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

/// Stable id for the targeted trigger-coverage campaign.
pub const RAV_TRIGGER_PROBE_ID: &str = "rav_trigger_probe_flame_kin_zealot";
/// Stable id for the optional-trigger policy fixture.
pub const RAV_OPTIONAL_TRIGGER_PROBE_ID: &str = "rav_trigger_probe_twilight_drover";
/// Stable id for the simultaneous trigger-order policy fixture.
pub const RAV_TRIGGER_ORDER_PROBE_ID: &str = "rav_trigger_probe_twilight_drover_order";

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

/// Runs a policy-submitted spell whose resolution makes another creature leave
/// the battlefield, then accepts Twilight Drover's optional trigger through
/// the ordinary id-bearing policy decision. This complements the mandatory ETB
/// probe with an explicit optional-payment boundary.
pub fn run_rav_optional_trigger_probe() -> Result<TriggerProbeResult, String> {
    let (mut game, drovers, exiter, clutch) = setup_drover_bounce_game(1)?;
    let drover = drovers[0];
    game.submit_policy_move(
        PlayerId(0),
        "probe.optional.v1",
        PolicyAction::Cast(CastRequest {
            card: clutch,
            targets: vec![cardbench_magic_engine::Target::Permanent(exiter)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        }),
    )
    .map_err(|error| format!("cast bounce source: {error}"))?;
    game.submit_policy_move(PlayerId(0), "probe.optional.v1", PolicyAction::PassPriority)
        .map_err(|error| format!("pass after cast: {error}"))?;
    game.submit_policy_move(PlayerId(1), "probe.optional.v1", PolicyAction::PassPriority)
        .map_err(|error| format!("resolve bounce source: {error}"))?;
    game.submit_policy_move(PlayerId(0), "probe.optional.v1", PolicyAction::PassPriority)
        .map_err(|error| format!("pass optional trigger: {error}"))?;
    game.submit_policy_move(PlayerId(1), "probe.optional.v1", PolicyAction::PassPriority)
        .map_err(|error| format!("open optional trigger choice: {error}"))?;
    let choice = game
        .view_for_player(PlayerId(0))
        .map_err(|error| error.to_string())?
        .optional_triggered_ability_choice
        .ok_or_else(|| "Twilight Drover optional trigger choice was not opened".to_owned())?;
    game.submit_policy_move(
        PlayerId(0),
        "probe.optional.v1",
        PolicyAction::ResolveOptionalTriggeredAbility {
            decision: choice.decision,
            source: drover,
            ability: "another-creature-leaves-plus-one-counter",
            pay: true,
            target: None,
        },
    )
    .map_err(|error| format!("accept optional trigger: {error}"))?;
    finish_trigger_probe(RAV_OPTIONAL_TRIGGER_PROBE_ID, &game)
}

/// Runs the same departure with two Twilight Drovers, requiring the policy to
/// submit an APNAP trigger-order decision before either optional trigger is
/// placed on the stack. Both triggers are then declined after their response
/// windows, proving the order and optional-choice continuations compose.
pub fn run_rav_trigger_order_probe() -> Result<TriggerProbeResult, String> {
    let (mut game, drovers, exiter, clutch) = setup_drover_bounce_game(2)?;
    game.submit_policy_move(
        PlayerId(0),
        "probe.order.v1",
        PolicyAction::Cast(CastRequest {
            card: clutch,
            targets: vec![cardbench_magic_engine::Target::Permanent(exiter)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        }),
    )
    .map_err(|error| format!("cast bounce source: {error}"))?;
    game.submit_policy_move(PlayerId(0), "probe.order.v1", PolicyAction::PassPriority)
        .map_err(|error| format!("pass after cast: {error}"))?;
    game.submit_policy_move(PlayerId(1), "probe.order.v1", PolicyAction::PassPriority)
        .map_err(|error| format!("resolve bounce source: {error}"))?;
    let decision = game
        .view_for_player(PlayerId(0))
        .map_err(|error| error.to_string())?
        .pending_decision
        .ok_or_else(|| "Twilight Drover trigger-order decision was not opened".to_owned())?;
    let order = decision.trigger_candidates.clone();
    if order.len() != drovers.len() {
        return Err(format!(
            "expected {} ordered Drover triggers, got {}",
            drovers.len(),
            order.len()
        ));
    }
    game.submit_policy_move(
        PlayerId(0),
        "probe.order.v1",
        PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::TriggerOrder(order),
        },
    )
    .map_err(|error| format!("order Drover triggers: {error}"))?;
    for _ in drovers {
        game.submit_policy_move(PlayerId(0), "probe.order.v1", PolicyAction::PassPriority)
            .map_err(|error| format!("pass ordered trigger: {error}"))?;
        game.submit_policy_move(PlayerId(1), "probe.order.v1", PolicyAction::PassPriority)
            .map_err(|error| format!("resolve ordered trigger: {error}"))?;
        let choice = game
            .view_for_player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .optional_triggered_ability_choice
            .ok_or_else(|| "ordered Drover optional choice was not opened".to_owned())?;
        game.submit_policy_move(
            PlayerId(0),
            "probe.order.v1",
            PolicyAction::ResolveOptionalTriggeredAbility {
                decision: choice.decision,
                source: choice.source,
                ability: "another-creature-leaves-plus-one-counter",
                pay: false,
                target: None,
            },
        )
        .map_err(|error| format!("decline ordered trigger: {error}"))?;
    }
    finish_trigger_probe(RAV_TRIGGER_ORDER_PROBE_ID, &game)
}

fn setup_drover_bounce_game(
    drover_count: usize,
) -> Result<(Game, Vec<ObjectId>, ObjectId, ObjectId), String> {
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
    let mut drovers = Vec::with_capacity(drover_count);
    for _ in 0..drover_count {
        drovers.push(
            game.put_on_battlefield(PlayerId(0), "RAV-TWILIGHT-DROVER")
                .map_err(|error| error.to_string())?,
        );
    }
    let exiter = game
        .put_on_battlefield(PlayerId(0), "RAV-WATCHWOLF")
        .map_err(|error| error.to_string())?;
    let clutch = game
        .add_card(PlayerId(0), "RAV-CLUTCH-OF-THE-UNDERCITY", Zone::Hand)
        .map_err(|error| error.to_string())?;
    let lands = [
        ("RAV-PLAINS", Color::White),
        ("RAV-PLAINS", Color::White),
        ("RAV-PLAINS", Color::White),
        ("RAV-ISLAND", Color::Blue),
        ("RAV-ISLAND", Color::Blue),
        ("RAV-SWAMP", Color::Black),
        ("RAV-SWAMP", Color::Black),
    ];
    let mut land_ids = Vec::with_capacity(lands.len());
    for (definition, color) in lands {
        let land = game
            .put_on_battlefield(PlayerId(0), definition)
            .map_err(|error| error.to_string())?;
        land_ids.push((land, color));
    }
    game.begin_game().map_err(|error| error.to_string())?;
    while game.step != Step::PrecombatMain {
        let player = game.next_policy_player();
        game.submit_policy_move(player, "probe.optional.v1", PolicyAction::PassPriority)
            .map_err(|error| format!("advance to main: {error}"))?;
    }
    for (land, color) in land_ids {
        game.activate_mana_ability(PlayerId(0), land, color)
            .map_err(|error| format!("activate land: {error}"))?;
    }
    Ok((game, drovers, exiter, clutch))
}

fn finish_trigger_probe(id: &'static str, game: &Game) -> Result<TriggerProbeResult, String> {
    game.validate_invariants()
        .map_err(|error| error.to_string())?;
    let event_log = game.canonical_event_log();
    let triggered_ability_count = game
        .event_log
        .iter()
        .filter(|event| matches!(event, GameEvent::TriggeredAbilityStacked { .. }))
        .count();
    Ok(TriggerProbeResult {
        id,
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

    #[test]
    fn optional_trigger_probe_is_policy_submitted_and_fail_closed() {
        let result = run_rav_optional_trigger_probe().expect("optional trigger probe runs");
        assert!(result.passed(), "{result:#?}");
        assert_eq!(result.triggered_ability_count, 1);
        assert!(
            result
                .event_log
                .iter()
                .any(|event| event.contains("CounterPlaced"))
        );
        assert!(
            result
                .event_log
                .iter()
                .any(|event| event.contains("AbilityResolved"))
        );
    }

    #[test]
    fn trigger_order_probe_is_policy_submitted_and_fail_closed() {
        let result = run_rav_trigger_order_probe().expect("trigger-order probe runs");
        assert!(result.passed(), "{result:#?}");
        assert_eq!(result.triggered_ability_count, 2);
        assert!(
            result
                .event_log
                .iter()
                .any(|event| event.contains("TriggeredAbilityOrderChosen"))
        );
    }
}
