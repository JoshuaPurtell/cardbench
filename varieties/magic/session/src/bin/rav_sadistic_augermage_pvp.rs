//! Isolated PvP authority for RAV #103 Sadistic Augermage.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ContinuousChange, DecisionKind, DecisionVisibility, Duration,
    Effect, Game, GameEvent, ManaCost, PlayerId, PolicyAction, TriggerCondition,
    TriggeredAbilityBinding, Zone,
};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-SADISTIC-AUGERMAGE";
const CARD_NAME: &str = "Sadistic Augermage";
const ABILITY_ID: &str = "source-dies-each-player-puts-card-on-library-top";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/sadistic_augermage.rs";
const EXPECTED_SOURCE: &str = r#"//! Candidate-owned RAV #103 Sadistic Augermage identity and death trigger.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, ManaCost, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SADISTIC-AUGERMAGE",
        name: "Sadistic Augermage",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "source-dies-each-player-puts-card-on-library-top",
            "policy-private-each-player-hand-selection",
        ],
        power: Some(3),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-SADISTIC-AUGERMAGE",
        ability: TriggeredAbility {
            id: "source-dies-each-player-puts-card-on-library-top",
            condition: TriggerCondition::Dies,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::PutOneHandCardOnLibraryTopEachPlayer],
        },
    }
}
"#;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/sadistic_augermage.rs"]
mod candidate_sadistic_augermage;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            println!(
                "{}",
                serde_json::to_string(&report).expect("report serializes")
            );
            if report["passed"].as_bool() == Some(true) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("Sadistic Augermage PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongManaCost,
    WrongPower,
    WrongToughness,
    RemoveTrigger,
    WrongTriggerCondition,
    OptionalTrigger,
    RemoveEffect,
}

#[derive(Clone, Debug)]
struct Trace {
    source: cardbench_magic_engine::ObjectId,
    p0_choice: cardbench_magic_engine::ObjectId,
    p0_keep: cardbench_magic_engine::ObjectId,
    p1_choice: cardbench_magic_engine::ObjectId,
    p1_keep: cardbench_magic_engine::ObjectId,
    p0_library_sentinel: cardbench_magic_engine::ObjectId,
    p1_library_sentinel: cardbench_magic_engine::ObjectId,
    p0_candidates: usize,
    p1_candidates: usize,
    p0_private: bool,
    p1_private: bool,
    hidden_from_p1_before_choice: bool,
    hidden_from_p0_after_choice: bool,
    p0_owner_scope: bool,
    p1_owner_scope: bool,
    chosen_cards_remained_in_hand_after_first_choice: bool,
    both_cards_became_each_owner_library_top: bool,
    unchosen_cards_remained_in_hand: bool,
    sentinels_remained_below_new_tops: bool,
    no_discard_receipts: bool,
    body_valid: bool,
    cost_valid: bool,
    events: Vec<GameEvent>,
    stacked_index: usize,
    p0_opened_index: usize,
    p0_completed_index: usize,
    p1_opened_index: usize,
    p1_completed_index: usize,
    p0_move_index: usize,
    p1_move_index: usize,
    resolved_index: usize,
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate()?;
    let catalog = unique_catalog()?;
    let trace = run_trace(Mutation::None)?;
    validate_trace(&trace)?;
    let replay = run_trace(Mutation::None)?;
    if !same_trace(&trace, &replay) {
        return Err("Sadistic Augermage private-choice fixture is not deterministic".to_owned());
    }
    let mutations = json!({
        "wrong_mana_cost_detected": mutation_detected(Mutation::WrongManaCost)?,
        "wrong_power_detected": mutation_detected(Mutation::WrongPower)?,
        "wrong_toughness_detected": mutation_detected(Mutation::WrongToughness)?,
        "trigger_removal_detected": mutation_detected(Mutation::RemoveTrigger)?,
        "wrong_trigger_condition_detected": mutation_detected(Mutation::WrongTriggerCondition)?,
        "optional_trigger_detected": mutation_detected(Mutation::OptionalTrigger)?,
        "effect_removal_detected": mutation_detected(Mutation::RemoveEffect)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "wrong_mana_cost_detected",
        "wrong_power_detected",
        "wrong_toughness_detected",
        "trigger_removal_detected",
        "wrong_trigger_condition_detected",
        "optional_trigger_detected",
        "effect_removal_detected",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Sadistic Augermage semantic mutation escaped: {key}"
            ));
        }
    }

    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    let trace_json = validate_trace(&trace)?;
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 103, "printings": 1, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "603.2: source-death event triggers",
                "603.3: mandatory triggered ability is put on the stack",
                "608.2: resolving a triggered ability",
                "400.2 and 400.7: hand/library zones and new zone-change objects",
                "401.2: library top is the last card in the modeled library order"
            ],
            "source_contract": "RAV #103 is a black {2}{B} 3/1 Human Wizard with a mandatory source-death trigger that privately collects one hand choice from each player and places each selection on its owner's library top.",
            "authority_cutoff": "2026-09-24",
            "public_identity_record": "https://avascry.com/printing/sadistic-augermage-rav",
            "no_provider_trial": true
        },
        "passed": cards_passed == 1,
        "pilot_card_passed": cards_passed == 1,
        "replay_deterministic": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "diagnostic_only; external qualification pending",
        "candidate_source_validation": {
            "direct_constructor_ast_checked_before_candidate_calls": true,
            "candidate_api_contract_checked_before_game_setup": true
        },
        "mutation_sensitivity": mutations,
        "traces": [trace_json],
        "per_card_breakdown": breakdown
    }))
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn candidate_definition(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_sadistic_augermage::definition();
    match mutation {
        Mutation::None
        | Mutation::RemoveTrigger
        | Mutation::WrongTriggerCondition
        | Mutation::OptionalTrigger
        | Mutation::RemoveEffect => {}
        Mutation::WrongManaCost => {
            definition.mana_cost = ManaCost::with_colors(3, [Color::Black]);
        }
        Mutation::WrongPower => definition.power = Some(4),
        Mutation::WrongToughness => definition.toughness = Some(2),
    }
    definition
}

fn candidate_trigger(mutation: Mutation) -> Option<TriggeredAbilityBinding> {
    if mutation == Mutation::RemoveTrigger {
        return None;
    }
    let mut binding = candidate_sadistic_augermage::triggered_ability();
    match mutation {
        Mutation::None
        | Mutation::WrongManaCost
        | Mutation::WrongPower
        | Mutation::WrongToughness => {}
        Mutation::RemoveTrigger => unreachable!(),
        Mutation::WrongTriggerCondition => {
            binding.ability.condition = TriggerCondition::EntersBattlefield;
        }
        Mutation::OptionalTrigger => binding.ability.optional = true,
        Mutation::RemoveEffect => binding.ability.effects.clear(),
    }
    Some(binding)
}

fn validate_candidate() -> Result<(), String> {
    let definition = candidate_sadistic_augermage::definition();
    let expected_cost = ManaCost::with_colors(2, [Color::Black]);
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != expected_cost
        || definition.colors != BTreeSet::from([Color::Black])
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.power != Some(3)
        || definition.toughness != Some(1)
    {
        return Err("Sadistic Augermage identity, cost, color, type, or body differs".to_owned());
    }
    let binding = candidate_sadistic_augermage::triggered_ability();
    if binding.card_definition != CARD_ID
        || binding.ability.id != ABILITY_ID
        || binding.ability.condition != TriggerCondition::Dies
        || binding.ability.optional
        || binding.ability.mana_cost != ManaCost::new(0)
        || !binding.ability.targets.is_empty()
        || binding.ability.effects != [Effect::PutOneHandCardOnLibraryTopEachPlayer]
    {
        return Err("Sadistic Augermage mandatory source-death effect differs".to_owned());
    }
    Ok(())
}

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Sadistic Augermage is missing from the RAV catalog")? =
        candidate_definition(mutation);
    let mut triggers = rav_triggered_ability_bindings();
    triggers.retain(|binding| binding.card_definition != CARD_ID);
    if let Some(binding) = candidate_trigger(mutation) {
        triggers.push(binding);
    }
    Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers,
    )
    .map_err(|error| error.to_string())
}

fn run_trace(mutation: Mutation) -> Result<Trace, String> {
    let mut game = new_game(mutation)?;
    let source = game
        .put_on_battlefield(PlayerId(0), CARD_ID)
        .map_err(|error| error.to_string())?;
    game.set_entered_turn_for_setup(source, 0)
        .map_err(|error| error.to_string())?;
    let p0_library_sentinel = game
        .add_card(PlayerId(0), "RAV-ISLAND", Zone::Library)
        .map_err(|error| error.to_string())?;
    let p1_library_sentinel = game
        .add_card(PlayerId(1), "RAV-FOREST", Zone::Library)
        .map_err(|error| error.to_string())?;
    let p0_choice = game
        .add_card(PlayerId(0), "RAV-SWAMP", Zone::Hand)
        .map_err(|error| error.to_string())?;
    let p0_keep = game
        .add_card(PlayerId(0), "RAV-WATCHWOLF", Zone::Hand)
        .map_err(|error| error.to_string())?;
    let p1_choice = game
        .add_card(PlayerId(1), "RAV-PLAINS", Zone::Hand)
        .map_err(|error| error.to_string())?;
    let p1_keep = game
        .add_card(PlayerId(1), "RAV-ISLAND", Zone::Hand)
        .map_err(|error| error.to_string())?;

    game.begin_game().map_err(|error| error.to_string())?;
    game.add_continuous_effect(
        source,
        source,
        ContinuousChange::ModifyPowerToughness {
            power: 0,
            toughness: -1,
        },
        Duration::EndOfTurn(1),
    )
    .map_err(|error| error.to_string())?;
    if game.zone_of(source) != Some(Zone::Graveyard) {
        return Err("Sadistic Augermage did not die during state-based actions".to_owned());
    }
    if game
        .view_for_player(PlayerId(0))
        .map_err(|error| error.to_string())?
        .optional_triggered_ability_choice
        .is_some()
    {
        return Err("Sadistic Augermage's trigger was incorrectly optional".to_owned());
    }
    game.pass_priority(PlayerId(0))
        .map_err(|error| error.to_string())?;
    game.pass_priority(PlayerId(1))
        .map_err(|error| error.to_string())?;

    let p0_view = game
        .view_for_player(PlayerId(0))
        .map_err(|error| error.to_string())?;
    let p0_decision = p0_view
        .pending_decision
        .as_ref()
        .ok_or("controller did not receive a typed hand-card decision")?;
    let p0_choice_view = p0_view
        .triggered_ability_effect_object_choice
        .as_ref()
        .ok_or("controller did not receive the triggered-effect object choice")?;
    if p0_decision.kind != DecisionKind::TriggeredEffectObject
        || p0_decision.visibility != DecisionVisibility::Private
        || p0_choice_view.source != source
        || p0_choice_view.ability != ABILITY_ID
    {
        return Err("controller choice was not private or did not match this trigger".to_owned());
    }
    let p1_before = game
        .view_for_player(PlayerId(1))
        .map_err(|error| error.to_string())?;
    let hidden_from_p1_before_choice = p1_before.pending_decision.is_none()
        && p1_before.triggered_ability_effect_object_choice.is_none();
    let p0_candidates = p0_choice_view.candidates.len();
    let p0_owner_scope = p0_candidates == 2
        && p0_choice_view.candidates.iter().all(|candidate| {
            game.object(candidate.id)
                .is_ok_and(|object| object.owner == PlayerId(0))
                && game.zone_of(candidate.id) == Some(Zone::Hand)
        })
        && [p0_choice, p0_keep].iter().all(|card| {
            p0_choice_view
                .candidates
                .iter()
                .any(|candidate| candidate.id == *card)
        });
    if !p0_choice_view
        .candidates
        .iter()
        .any(|candidate| candidate.id == p0_choice)
    {
        return Err("controller's own selected card was not eligible".to_owned());
    }
    let p0_decision_id = p0_choice_view.decision;
    game.submit_policy_move(
        PlayerId(0),
        "rav.sadistic-augermage-private-choice-p0.v1",
        PolicyAction::ChooseTriggeredAbilityEffectObject {
            decision: p0_decision_id,
            source,
            ability: ABILITY_ID,
            selected: Some(p0_choice),
        },
    )
    .map_err(|error| format!("controller's private choice was rejected: {error}"))?;
    let chosen_cards_remained_in_hand_after_first_choice = game.zone_of(p0_choice)
        == Some(Zone::Hand)
        && game.zone_of(p1_choice) == Some(Zone::Hand)
        && game.players[0].library.last() == Some(&p0_library_sentinel)
        && game.players[1].library.last() == Some(&p1_library_sentinel);

    let p1_view = game
        .view_for_player(PlayerId(1))
        .map_err(|error| error.to_string())?;
    let p1_decision = p1_view
        .pending_decision
        .as_ref()
        .ok_or("opponent did not receive their own typed hand-card decision")?;
    let p1_choice_view = p1_view
        .triggered_ability_effect_object_choice
        .as_ref()
        .ok_or("opponent did not receive their own triggered-effect object choice")?;
    if p1_decision.kind != DecisionKind::TriggeredEffectObject
        || p1_decision.visibility != DecisionVisibility::Private
        || p1_choice_view.source != source
        || p1_choice_view.ability != ABILITY_ID
    {
        return Err("opponent choice was not private or did not match this trigger".to_owned());
    }
    let p0_after_choice = game
        .view_for_player(PlayerId(0))
        .map_err(|error| error.to_string())?;
    let hidden_from_p0_after_choice = p0_after_choice.pending_decision.is_none()
        && p0_after_choice
            .triggered_ability_effect_object_choice
            .is_none();
    let p1_candidates = p1_choice_view.candidates.len();
    let p1_owner_scope = p1_candidates == 2
        && p1_choice_view.candidates.iter().all(|candidate| {
            game.object(candidate.id)
                .is_ok_and(|object| object.owner == PlayerId(1))
                && game.zone_of(candidate.id) == Some(Zone::Hand)
        })
        && [p1_choice, p1_keep].iter().all(|card| {
            p1_choice_view
                .candidates
                .iter()
                .any(|candidate| candidate.id == *card)
        });
    let p1_decision_id = p1_choice_view.decision;
    game.submit_policy_move(
        PlayerId(1),
        "rav.sadistic-augermage-private-choice-p1.v1",
        PolicyAction::ChooseTriggeredAbilityEffectObject {
            decision: p1_decision_id,
            source,
            ability: ABILITY_ID,
            selected: Some(p1_choice),
        },
    )
    .map_err(|error| format!("opponent's private choice was rejected: {error}"))?;
    game.validate_invariants()
        .map_err(|error| format!("post-resolution invariant failure: {error}"))?;
    let events = game.event_log.clone();

    let both_cards_became_each_owner_library_top = game.zone_of(p0_choice) == Some(Zone::Library)
        && game.zone_of(p1_choice) == Some(Zone::Library)
        && game.players[0].library.last() == Some(&p0_choice)
        && game.players[1].library.last() == Some(&p1_choice);
    let unchosen_cards_remained_in_hand =
        game.zone_of(p0_keep) == Some(Zone::Hand) && game.zone_of(p1_keep) == Some(Zone::Hand);
    let sentinels_remained_below_new_tops = game.players[0].library.contains(&p0_library_sentinel)
        && game.players[1].library.contains(&p1_library_sentinel)
        && game.players[0].library.last() != Some(&p0_library_sentinel)
        && game.players[1].library.last() != Some(&p1_library_sentinel);
    let no_discard_receipts = !events
        .iter()
        .any(|event| matches!(event, GameEvent::CardDiscarded { .. }));
    let definition = game
        .card_definition(source)
        .map_err(|error| error.to_string())?;
    let body_valid = definition.colors == BTreeSet::from([Color::Black])
        && definition.card_types.contains(&CardType::Creature)
        && game
            .characteristics(source)
            .is_ok_and(|value| value.power == Some(3) && value.toughness == Some(1));
    let cost_valid = definition.mana_cost == ManaCost::with_colors(2, [Color::Black]);

    let stacked_index = find_event_index(&events, |event| {
        matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source: found, ability, .. } if *found == source && *ability == ABILITY_ID)
    })
    .ok_or("Sadistic Augermage source-death trigger did not stack")?;
    let p0_opened_index = find_event_index(&events, |event| {
        matches!(
            event,
            GameEvent::DecisionOpened {
                player: PlayerId(0),
                kind: DecisionKind::TriggeredEffectObject,
                visibility: DecisionVisibility::Private,
                ..
            }
        )
    })
    .ok_or("controller private decision-opened receipt is missing")?;
    let p0_completed_index = find_event_index(&events, |event| {
        matches!(
            event,
            GameEvent::DecisionCompleted {
                player: PlayerId(0),
                kind: DecisionKind::TriggeredEffectObject,
                ..
            }
        )
    })
    .ok_or("controller private decision-completed receipt is missing")?;
    let p1_opened_index = find_event_index(&events, |event| {
        matches!(
            event,
            GameEvent::DecisionOpened {
                player: PlayerId(1),
                kind: DecisionKind::TriggeredEffectObject,
                visibility: DecisionVisibility::Private,
                ..
            }
        )
    })
    .ok_or("opponent private decision-opened receipt is missing")?;
    let p1_completed_index = find_event_index(&events, |event| {
        matches!(
            event,
            GameEvent::DecisionCompleted {
                player: PlayerId(1),
                kind: DecisionKind::TriggeredEffectObject,
                ..
            }
        )
    })
    .ok_or("opponent private decision-completed receipt is missing")?;
    let p0_move_index = find_event_index(&events, |event| {
        matches!(event, GameEvent::CardMoved { card, to: Zone::Library, .. } if *card == p0_choice)
    })
    .ok_or("controller's chosen card did not move to their library")?;
    let p1_move_index = find_event_index(&events, |event| {
        matches!(event, GameEvent::CardMoved { card, to: Zone::Library, .. } if *card == p1_choice)
    })
    .ok_or("opponent's chosen card did not move to their library")?;
    let resolved_index = find_event_index(&events, |event| {
        matches!(event, GameEvent::AbilityResolved { source: found, ability, .. } if *found == source && *ability == ABILITY_ID)
    })
    .ok_or("Sadistic Augermage trigger did not resolve")?;

    Ok(Trace {
        source,
        p0_choice,
        p0_keep,
        p1_choice,
        p1_keep,
        p0_library_sentinel,
        p1_library_sentinel,
        p0_candidates,
        p1_candidates,
        p0_private: true,
        p1_private: true,
        hidden_from_p1_before_choice,
        hidden_from_p0_after_choice,
        p0_owner_scope,
        p1_owner_scope,
        chosen_cards_remained_in_hand_after_first_choice,
        both_cards_became_each_owner_library_top,
        unchosen_cards_remained_in_hand,
        sentinels_remained_below_new_tops,
        no_discard_receipts,
        body_valid,
        cost_valid,
        events,
        stacked_index,
        p0_opened_index,
        p0_completed_index,
        p1_opened_index,
        p1_completed_index,
        p0_move_index,
        p1_move_index,
        resolved_index,
    })
}

fn validate_trace(trace: &Trace) -> Result<Value, String> {
    if trace.p0_candidates != 2
        || trace.p1_candidates != 2
        || !trace.p0_private
        || !trace.p1_private
        || !trace.hidden_from_p1_before_choice
        || !trace.hidden_from_p0_after_choice
        || !trace.p0_owner_scope
        || !trace.p1_owner_scope
        || !trace.chosen_cards_remained_in_hand_after_first_choice
        || !trace.both_cards_became_each_owner_library_top
        || !trace.unchosen_cards_remained_in_hand
        || !trace.sentinels_remained_below_new_tops
        || !trace.no_discard_receipts
        || !trace.body_valid
        || !trace.cost_valid
    {
        return Err(
            "Sadistic Augermage PvP identity, privacy, ownership, or zone result differs"
                .to_owned(),
        );
    }
    if !(trace.stacked_index < trace.p0_opened_index
        && trace.p0_opened_index < trace.p0_completed_index
        && trace.p0_completed_index < trace.p1_opened_index
        && trace.p1_opened_index < trace.p1_completed_index
        && trace.p1_completed_index < trace.p0_move_index
        && trace.p1_completed_index < trace.p1_move_index
        && trace.p0_move_index < trace.resolved_index
        && trace.p1_move_index < trace.resolved_index)
    {
        return Err(
            "Sadistic Augermage choices and library moves did not resolve in order".to_owned(),
        );
    }
    Ok(json!({
        "trace_id": "rav-sadistic-augermage-source-death-private-library-top",
        "passed": true,
        "terminal": "EffectResolved",
        "source": {"object": trace.source.0, "definition_id": CARD_ID, "mana_cost": "{2}{B}", "power": 3, "toughness": 1, "died": true},
        "trigger": {"ability": ABILITY_ID, "condition": "Dies", "mandatory": true, "players_chose_in_apnap_order": [0, 1]},
        "choices": {
            "private_choice_seats": [0, 1],
            "private_to_each_chooser": [trace.p0_private, trace.p1_private],
            "hidden_from_nonchooser": [trace.hidden_from_p1_before_choice, trace.hidden_from_p0_after_choice],
            "candidate_counts_by_chooser": [trace.p0_candidates, trace.p1_candidates],
            "candidates_belong_to_chooser": [trace.p0_owner_scope, trace.p1_owner_scope],
            "chosen_cards_remained_in_hand_until_both_choices": trace.chosen_cards_remained_in_hand_after_first_choice,
            "chosen_card_owners_match_seats": true,
            "chosen_cards_are_each_library_top": trace.both_cards_became_each_owner_library_top,
            "unchosen_cards_remained_in_hand": trace.unchosen_cards_remained_in_hand,
            "prior_library_cards_remained_below_new_top": trace.sentinels_remained_below_new_tops,
            "discard_receipts": 0
        },
        "ordered_event_indices": {
            "trigger_stacked": trace.stacked_index,
            "p0_private_choice_opened": trace.p0_opened_index,
            "p0_choice_completed": trace.p0_completed_index,
            "p1_private_choice_opened": trace.p1_opened_index,
            "p1_choice_completed": trace.p1_completed_index,
            "p0_card_moved_to_library": trace.p0_move_index,
            "p1_card_moved_to_library": trace.p1_move_index,
            "trigger_resolved": trace.resolved_index
        },
        "events": project_events(&trace.events),
        "event_count": trace.events.len()
    }))
}

fn mutation_detected(mutation: Mutation) -> Result<bool, String> {
    match run_trace(mutation) {
        Err(_) => Ok(true),
        Ok(trace) => Ok(validate_trace(&trace).is_err()),
    }
}

fn same_trace(left: &Trace, right: &Trace) -> bool {
    left.source == right.source
        && left.p0_choice == right.p0_choice
        && left.p0_keep == right.p0_keep
        && left.p1_choice == right.p1_choice
        && left.p1_keep == right.p1_keep
        && left.p0_library_sentinel == right.p0_library_sentinel
        && left.p1_library_sentinel == right.p1_library_sentinel
        && left.events == right.events
}

fn find_event_index(
    events: &[GameEvent],
    mut predicate: impl FnMut(&GameEvent) -> bool,
) -> Option<usize> {
    events.iter().position(&mut predicate)
}

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<_> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || !catalog
            .iter()
            .any(|card| card.collector_number == 103 && card.name == CARD_NAME)
    {
        return Err("RAV catalog identity or denominator changed".to_owned());
    }
    Ok(catalog)
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({
        "collector_number": card.collector_number,
        "name": card.name,
        "definition_id": definition_id,
        "engine_status": match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "credited": false,
        "qualification": if passed { "pilot trace passed; independent review pending" } else { "not_evaluated" }
    })
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Sadistic Augermage source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Sadistic Augermage source is not valid Rust".to_owned())?;
    let compact = |text: &str| {
        text.chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>()
    };
    if compact(&source) != compact(EXPECTED_SOURCE) {
        return Err(
            "Sadistic Augermage source differs from the pinned declarative module contract"
                .to_owned(),
        );
    }
    if file
        .items
        .iter()
        .any(|item| !matches!(item, syn::Item::Use(_) | syn::Item::Fn(_)))
    {
        return Err("Sadistic Augermage source contains unsupported executable items".to_owned());
    }
    let mut functions = BTreeSet::new();
    for item in &file.items {
        let syn::Item::Fn(function) = item else {
            continue;
        };
        functions.insert(function.sig.ident.to_string());
        if function.block.stmts.len() != 1
            || !matches!(function.block.stmts.first(), Some(syn::Stmt::Expr(syn::Expr::Struct(value), None)) if value.rest.is_none())
        {
            return Err(
                "Sadistic Augermage functions must directly return declarative structs".to_owned(),
            );
        }
    }
    if functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()]) {
        return Err(
            "Sadistic Augermage module must expose only its two card constructors".to_owned(),
        );
    }
    Ok(())
}
