//! Full-game RAV Golgari Thug authority for Dredge 4 and its targeted death trigger.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CombatBlock, DeckEntry, DeckList, Effect, Game, GameEvent,
    GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step,
    Target, TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};
use cardbench_magic_policies::{
    CatalogPolicyProfile, CodePolicy, DeckMatchConfig, RavCatalogPolicy,
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
use std::collections::BTreeSet;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use syn::{Expr, Item, ReturnType, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-GOLGARI-THUG";
const CARD_NAME: &str = "Golgari Thug";
const OPPONENT_ID: &str = "RAV-WATCHWOLF";
const TRIGGER_ID: &str = "dies-target-creature-card-owner-library-top";
const SETUP_ID: &str = "RAV-GOLGARI-BROWNSCALE";
const MAX_TRACE_SEED: u64 = 256;
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/golgari_thug.rs";
const DEFAULT_TRACE_SEED: u64 = 12;
const EXPECTED_CANDIDATE_SOURCE: &str = r#"//! Candidate-owned Golgari Thug definition and graveyard death trigger.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TargetRequirement,
    TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

/// RAV #87's black 1/1 with Dredge 4.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GOLGARI-THUG",
        name: "Golgari Thug",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "dredge",
            "base-characteristics",
            "dies-target-creature-card-owner-library-top",
        ],
        power: Some(1),
        toughness: Some(1),
        keywords: vec![Keyword::Dredge(4)],
        effects: vec![],
    }
}

/// When this creature dies, its controller targets a creature card in their
/// graveyard and puts that card on top of its owner's library.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-GOLGARI-THUG",
        ability: TriggeredAbility {
            id: "dies-target-creature-card-owner-library-top",
            condition: TriggerCondition::Dies,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::CreatureCardInControllerGraveyard],
            effects: vec![Effect::PutTargetCreatureCardInControllerGraveyardOnOwnersLibraryTop],
        },
    }
}"#;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/golgari_thug.rs"]
mod candidate_golgari_thug;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Golgari Thug PvP result: {error}");
                return ExitCode::FAILURE;
            }
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
            eprintln!("Golgari Thug PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    let malicious_side_effect_rejected = malicious_candidate_source_rejected()?;
    if !malicious_side_effect_rejected {
        return Err("compile-valid side-effect candidate escaped the pre-call source gate".into());
    }
    validate_candidate_definition()?;
    validate_candidate_binding()?;
    validate_registry()?;
    let catalog = unique_catalog()?;
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured != replay || project_events(&captured.events) != project_events(&replay.events) {
        return Err("Golgari Thug shuffled-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "missing_dredge_mutation_detected": mutation_detected(seed, Mutation::MissingDredge),
        "wrong_dredge_count_mutation_detected": mutation_detected(seed, Mutation::DredgeThree),
        "wrong_identity_mutation_detected": mutation_detected(seed, Mutation::WrongIdentity),
        "wrong_mana_cost_mutation_detected": mutation_detected(seed, Mutation::WrongCost),
        "wrong_creature_body_mutation_detected": mutation_detected(seed, Mutation::WrongBody),
        "graveyard_trigger_removed_mutation_detected": mutation_detected(seed, Mutation::NoTrigger),
        "graveyard_trigger_condition_changed_mutation_detected": mutation_detected(seed, Mutation::WrongTriggerCondition),
        "graveyard_trigger_target_changed_mutation_detected": mutation_detected(seed, Mutation::WrongTriggerTarget),
        "graveyard_trigger_effect_changed_mutation_detected": mutation_detected(seed, Mutation::WrongTriggerEffect),
        "compile_valid_candidate_side_effect_rejected_before_execution": malicious_side_effect_rejected,
        "sealed_mutation_authority": false
    });
    for key in [
        "missing_dredge_mutation_detected",
        "wrong_dredge_count_mutation_detected",
        "wrong_identity_mutation_detected",
        "wrong_mana_cost_mutation_detected",
        "wrong_creature_body_mutation_detected",
        "graveyard_trigger_removed_mutation_detected",
        "graveyard_trigger_condition_changed_mutation_detected",
        "graveyard_trigger_target_changed_mutation_detected",
        "graveyard_trigger_effect_changed_mutation_detected",
        "compile_valid_candidate_side_effect_rejected_before_execution",
    ] {
        if mutations[key] != true {
            return Err(format!("Golgari Thug semantic mutation escaped: {key}"));
        }
    }
    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": TASK_ID, "expansion": "RAV",
        "card": {"collector_number": 87, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(), "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": ["117: priority and draw decisions", "202.1-202.2: colored mana cost and color",
                "302: creature characteristics and combat", "601.2: casting and payment",
                "603: dies trigger and target choice", "608: targeted trigger resolution",
                "510: combat damage and deaths", "704: state-based actions", "702.52: Dredge"],
            "source_contract": "RAV Golgari Thug #87 is a black {1}{B} 1/1 Creature — Human Warrior with Dredge 4. When put into a graveyard from play, its controller targets a creature card in their graveyard and puts it on top of its owner's library. Human Warrior is retained but not represented by CardDefinition.",
            "oracle_reconciliation": "Oracle's dies wording denotes the printed battlefield-to-graveyard event; target restriction, controller graveyard, library-top destination, and Dredge 4 are preserved.",
            "authority_cutoff": "2026-09-24", "no_provider_trial": true
        },
        "passed": true, "pilot_card_passed": true, "replay_deterministic": true,
        "cards_passed": cards_passed, "cards_credited": 0, "cards_total": catalog.len(),
        "printings_total": RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, "coverage_complete": false,
        "full_reward": false, "harbor_reward": 0.0, "pilot_reward": 0.0,
        "qualification_status": "diagnostic_only; independent review pending",
        "mutation_sensitivity": mutations,
        "candidate_source_validation": {"candidate_ast_contract_matched": true,
            "compile_valid_injected_side_effect_candidate_rejected": malicious_side_effect_rejected,
            "guard_ran_before_candidate_function_calls": true},
        "traces": [trace], "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    MissingDredge,
    DredgeThree,
    WrongIdentity,
    WrongCost,
    WrongBody,
    NoTrigger,
    WrongTriggerCondition,
    WrongTriggerTarget,
    WrongTriggerEffect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    life_totals: [i64; 2],
    brownscale: Option<ObjectId>,
    thug: Option<ObjectId>,
    thug_pt: Option<(i32, i32)>,
    thug_zone: Option<Zone>,
    thug_cost: Option<ManaCost>,
    thug_payments: Vec<Vec<Color>>,
    thug_graveyard_incarnation: Option<u64>,
    chosen_target: Option<(PlayerId, ObjectId, ObjectId)>,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_GOLGARI_THUG_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_GOLGARI_THUG_SEED must be an unsigned integer".to_owned())?;
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    let mut last_error = String::new();
    for seed in DEFAULT_TRACE_SEED..=MAX_TRACE_SEED {
        match run_match(seed, Mutation::None)
            .and_then(|captured| validate_trace(&captured, seed).map(|_| captured))
        {
            Ok(captured) => return Ok((seed, captured)),
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "no Golgari Thug full-game trace found for seeds {DEFAULT_TRACE_SEED}..={MAX_TRACE_SEED}: {last_error}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation) {
        Ok(captured) => validate_trace(&captured, seed).is_err(),
        Err(_) => true,
    }
}

fn definition_for(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_golgari_thug::definition();
    match mutation {
        Mutation::None => {}
        Mutation::MissingDredge => definition.keywords.clear(),
        Mutation::DredgeThree => definition.keywords = vec![Keyword::Dredge(3)],
        Mutation::WrongIdentity => definition.id = "RAV-GOLGARI-THUG-WRONG",
        Mutation::WrongCost => definition.mana_cost = ManaCost::with_colors(2, [Color::Black]),
        Mutation::WrongBody => {
            definition.power = Some(2);
            definition.toughness = Some(1);
        }
        Mutation::NoTrigger
        | Mutation::WrongTriggerCondition
        | Mutation::WrongTriggerTarget
        | Mutation::WrongTriggerEffect => {}
    }
    definition
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let pilot_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: SETUP_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: OPPONENT_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 24,
            },
            DeckEntry {
                card: "RAV-SWAMP".to_owned(),
                count: 12,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 12,
            },
        ],
        sideboard: vec![],
    };
    let opponent_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: OPPONENT_ID.to_owned(),
                count: 1,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 29,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 30,
            },
        ],
        sideboard: vec![],
    };
    for (label, deck) in [("pilot", &pilot_deck), ("opponent", &opponent_deck)] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{label} deck has {count} cards; expected 60"));
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 120,
        ..DeckMatchConfig::default()
    };
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Golgari Thug missing from the RAV registry")?;
    *registered = definition_for(mutation);
    let thug_cost = registered.mana_cost.clone();
    let brownscale_cost = definitions
        .iter()
        .find(|card| card.id == SETUP_ID)
        .map(|card| card.mana_cost.clone())
        .ok_or("Golgari Brownscale missing from the RAV registry")?;
    let watchwolf_cost = definitions
        .iter()
        .find(|card| card.id == OPPONENT_ID)
        .map(|card| card.mana_cost.clone())
        .ok_or("Watchwolf missing from the RAV registry")?;
    let triggers: Vec<TriggeredAbilityBinding> = rav_triggered_ability_bindings()
        .into_iter()
        .filter(|binding| binding.card_definition != CARD_ID)
        .chain(match mutation {
            Mutation::NoTrigger => None,
            _ => Some(mutated_binding(mutation)),
        })
        .collect();
    let mut game = Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers,
    )
    .map_err(|error| error.to_string())?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let chosen_target = Arc::new(Mutex::new(None));
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(ThugPolicy::new(
            PlayerId(0),
            true,
            thug_cost.clone(),
            brownscale_cost.clone(),
            watchwolf_cost.clone(),
            Arc::clone(&chosen_target),
        )),
        Box::new(ThugPolicy::new(
            PlayerId(1),
            false,
            thug_cost.clone(),
            brownscale_cost,
            watchwolf_cost,
            Arc::clone(&chosen_target),
        )),
    ];
    let mut accepted_moves = 0_u32;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted_moves >= config.max_policy_moves {
            return Err("Golgari Thug PvP match exceeded its development bound".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("policy seat missing")?;
        let action = if let Some(action) = policy.propose_pending_decision(&view) {
            action
        } else if view.draw_replacement_pending {
            policy.propose_draw_replacement(&view)
        } else if view.optional_triggered_ability_choice.is_some() {
            policy.propose_optional_triggered_ability(&view)
        } else if view.private_library_choice.is_some() {
            policy.propose_private_library_choice(&view)
        } else if view.private_opponent_library_choice.is_some() {
            policy.propose_private_opponent_library_choice(&view)
        } else if view.library_search_choice.is_some() {
            policy.propose_library_search_choice(&view)
        } else {
            policy.propose_move(&view)
        };
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "policy move rejected at turn {} / {:?} for {:?}: {error}",
                    view.turn, view.step, player
                )
            })?;
        accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
    }
    let winner = game.winner().map(|player| player.0);
    let find_cast = |definition_id: &str| {
        game.event_log.iter().find_map(|event| match event {
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == definition_id) =>
            {
                Some(*card)
            }
            _ => None,
        })
    };
    let brownscale = find_cast(SETUP_ID);
    let thug = find_cast(CARD_ID);
    let thug_pt = thug
        .and_then(|card| game.characteristics(card).ok())
        .and_then(|characteristics| Some((characteristics.power?, characteristics.toughness?)));
    let thug_zone = thug.and_then(|card| game.zone_of(card));
    let thug_payments = game
        .event_log
        .iter()
        .filter_map(|event| match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if Some(*card) == thug => Some(colors.clone()),
            _ => None,
        })
        .collect();
    let thug_graveyard_incarnation = game.event_log.iter().find_map(|event| match event {
        GameEvent::TriggeredAbilityStacked {
            source,
            source_incarnation,
            ability,
        } if Some(*source) == thug && *ability == TRIGGER_ID => Some(*source_incarnation),
        _ => None,
    });
    let life_totals = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    let chosen_target = *chosen_target
        .lock()
        .map_err(|_| "target-choice lock poisoned")?;
    Ok(CapturedMatch {
        events: game.event_log.clone(),
        winner,
        turns: game.turn,
        accepted_moves,
        life_totals,
        brownscale,
        thug,
        thug_pt,
        thug_zone,
        thug_cost: Some(thug_cost),
        thug_payments,
        thug_graveyard_incarnation,
        chosen_target,
    })
}

struct ThugPolicy {
    player: PlayerId,
    pilot: bool,
    inner: RavCatalogPolicy,
    thug_cost: ManaCost,
    brownscale_cost: ManaCost,
    watchwolf_cost: ManaCost,
    brownscale_source: Option<ObjectId>,
    brownscale_blocked: bool,
    thug_source: Option<ObjectId>,
    thug_blocked: bool,
    recovered_thug: Option<ObjectId>,
    thug_cast_count: usize,
    chosen_target: Arc<Mutex<Option<(PlayerId, ObjectId, ObjectId)>>>,
}

impl ThugPolicy {
    fn new(
        player: PlayerId,
        pilot: bool,
        thug_cost: ManaCost,
        brownscale_cost: ManaCost,
        watchwolf_cost: ManaCost,
        chosen_target: Arc<Mutex<Option<(PlayerId, ObjectId, ObjectId)>>>,
    ) -> Self {
        Self {
            player,
            pilot,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
            thug_cost,
            brownscale_cost,
            watchwolf_cost,
            brownscale_source: None,
            brownscale_blocked: false,
            thug_source: None,
            thug_blocked: false,
            recovered_thug: None,
            thug_cast_count: 0,
            chosen_target,
        }
    }

    fn target_decision(&self, view: &GameView) -> Option<PolicyAction> {
        let choice = view.triggered_ability_target_choice.as_ref()?;
        let target = self.brownscale_source?;
        if choice.ability != TRIGGER_ID
            || !choice
                .target_options
                .first()?
                .contains(&Target::Permanent(target))
        {
            return None;
        }
        if let Ok(mut selected) = self.chosen_target.lock() {
            *selected = Some((view.player, choice.source, target));
        }
        Some(PolicyAction::ChooseTriggeredAbilityTargets {
            decision: choice.decision,
            source: choice.source,
            ability: choice.ability,
            targets: vec![Target::Permanent(target)],
        })
    }

    fn cast_named(
        &self,
        view: &GameView,
        definition: &str,
        cost: &ManaCost,
    ) -> Option<(ObjectId, PolicyAction)> {
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(definition))?;
        let selection = payment_selection(view, cost)?;
        Some((
            card.id,
            PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection: selection,
            },
        ))
    }

    fn action_card(action: &PolicyAction) -> Option<ObjectId> {
        match action {
            PolicyAction::Cast(request)
            | PolicyAction::CastWithPayment { request, .. }
            | PolicyAction::CastWithMode { request, .. }
            | PolicyAction::CastWithModeAndColorChoice { request, .. }
            | PolicyAction::CastWithColorChoice { request, .. }
            | PolicyAction::CastWithCreatureSpellAdditionalMana { request, .. } => {
                Some(request.card)
            }
            _ => None,
        }
    }

    fn wrap_payment(&self, view: &GameView, action: PolicyAction) -> PolicyAction {
        let Some(source) = Self::action_card(&action) else {
            return action;
        };
        let Some(card) = view.hand.iter().find(|card| card.id == source) else {
            return action;
        };
        let cost = match card.definition {
            Some(CARD_ID) => Some(&self.thug_cost),
            Some(SETUP_ID) => Some(&self.brownscale_cost),
            Some(OPPONENT_ID) => Some(&self.watchwolf_cost),
            _ => None,
        };
        if let PolicyAction::Cast(request) = action {
            if let Some(selection) = cost.and_then(|cost| payment_selection(view, cost)) {
                return PolicyAction::CastWithPayment {
                    request,
                    chosen_x: None,
                    mana_selection: selection,
                };
            }
            PolicyAction::Cast(request)
        } else {
            action
        }
    }

    fn record_cast(&mut self, view: &GameView, action: &PolicyAction) {
        let Some(source) = Self::action_card(action) else {
            return;
        };
        let Some(card) = view.hand.iter().find(|card| card.id == source) else {
            return;
        };
        match card.definition {
            Some(SETUP_ID) if self.pilot => {
                self.brownscale_source.get_or_insert(source);
            }
            Some(CARD_ID) if self.pilot => {
                self.thug_source.get_or_insert(source);
                self.thug_cast_count += 1;
            }
            _ => {}
        }
    }
}

impl CodePolicy for ThugPolicy {
    fn id(&self) -> &'static str {
        if self.pilot {
            "rav.golgari-thug-full-game-pilot.v1"
        } else {
            "rav.golgari-thug-watchwolf-opponent.v1"
        }
    }
    fn propose_draw_replacement(&mut self, view: &GameView) -> PolicyAction {
        if self.pilot
            && let Some(source) = self.thug_source
            && view.dredge_candidates.iter().any(|card| card.id == source)
        {
            self.recovered_thug = Some(source);
            return PolicyAction::Draw {
                decision: view
                    .draw_replacement_decision
                    .expect("draw replacement has an id"),
                dredge: Some(source),
            };
        }
        PolicyAction::Draw {
            decision: view
                .draw_replacement_decision
                .expect("draw replacement has an id"),
            dredge: None,
        }
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = view
                .own_battlefield
                .iter()
                .filter(|card| {
                    card.can_attack && (self.pilot || card.definition == Some(OPPONENT_ID))
                })
                .map(|card| card.id)
                .collect();
            return PolicyAction::DeclareAttackers { attackers };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            let assignment = if self.pilot {
                let attacker = view
                    .combat_attackers
                    .iter()
                    .find(|card| card.definition == Some(OPPONENT_ID))
                    .map(|card| card.id);
                let blocker = if !self.brownscale_blocked {
                    self.brownscale_source.filter(|source| {
                        view.own_battlefield
                            .iter()
                            .any(|card| card.id == *source && card.definition == Some(SETUP_ID))
                    })
                } else if !self.thug_blocked {
                    self.thug_source.filter(|source| {
                        view.own_battlefield
                            .iter()
                            .any(|card| card.id == *source && card.definition == Some(CARD_ID))
                    })
                } else {
                    None
                };
                match (attacker, blocker) {
                    (Some(attacker), Some(blocker)) => {
                        if view
                            .own_battlefield
                            .iter()
                            .any(|card| card.id == blocker && card.definition == Some(SETUP_ID))
                        {
                            self.brownscale_blocked = true;
                        } else if view
                            .own_battlefield
                            .iter()
                            .any(|card| card.id == blocker && card.definition == Some(CARD_ID))
                        {
                            self.thug_blocked = true;
                        }
                        Some(CombatBlock { attacker, blocker })
                    }
                    _ => None,
                }
            } else {
                None
            };
            return PolicyAction::DeclareBlockers {
                assignments: assignment.into_iter().collect(),
            };
        }
        let inner_action = self.inner.propose_move(view);
        if self.pilot
            && view.priority == self.player
            && view.active_player == self.player
            && view.stack_depth == 0
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            if view.lands_played == 0
                && matches!(
                    inner_action,
                    PolicyAction::PlayLand { .. }
                        | PolicyAction::PlayLandWithEntryLifePayment { .. }
                )
            {
                return inner_action;
            }
            if self.brownscale_source.is_none()
                && let Some((source, action)) =
                    self.cast_named(view, SETUP_ID, &self.brownscale_cost)
            {
                self.brownscale_source.get_or_insert(source);
                return action;
            }
            if self.brownscale_blocked
                && self.thug_source.is_none()
                && let Some((source, action)) = self.cast_named(view, CARD_ID, &self.thug_cost)
            {
                self.thug_source.get_or_insert(source);
                self.thug_cast_count += 1;
                return action;
            }
            if let Some(source) = self.recovered_thug
                && self.thug_cast_count < 2
                && let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.id == source && card.definition == Some(CARD_ID))
                && let Some(selection) = payment_selection(view, &self.thug_cost)
            {
                self.thug_cast_count += 1;
                return PolicyAction::CastWithPayment {
                    request: cardbench_magic_engine::CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: selection,
                };
            }
        }
        let action = self.wrap_payment(view, inner_action);
        self.record_cast(view, &action);
        action
    }
    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        self.target_decision(view)
            .or_else(|| self.inner.propose_pending_decision(view))
    }
    fn propose_optional_triggered_ability(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_optional_triggered_ability(view)
    }
    fn propose_private_library_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_private_library_choice(view)
    }
    fn propose_private_opponent_library_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_private_opponent_library_choice(view)
    }
    fn propose_library_search_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_library_search_choice(view)
    }
}

fn payment_selection(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<(Color, u8)> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let (_, amount) = available.iter_mut().find(|(known, _)| known == color)?;
        if *amount == 0 {
            return None;
        }
        *amount -= 1;
    }
    let mut generic = Vec::new();
    for _ in 0..cost.generic {
        let (color, amount) = available.iter_mut().max_by_key(|(_, amount)| *amount)?;
        if *amount == 0 {
            return None;
        }
        *amount -= 1;
        generic.push(*color);
    }
    Some(ManaPaymentSelection {
        generic,
        hybrid: vec![],
    })
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    let events = &captured.events;
    let brownscale = captured
        .brownscale
        .ok_or("pilot never cast setup Brownscale")?;
    let thug = captured.thug.ok_or("pilot never cast Golgari Thug")?;
    let watchwolf = events
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCast {
                player: PlayerId(1),
                card,
            } => Some(*card),
            _ => None,
        })
        .ok_or("opponent never cast a Watchwolf")?;
    let thug_casts: Vec<usize> = events
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            matches!(e, GameEvent::SpellCast { player: PlayerId(0), card } if *card == thug)
                .then_some(i)
        })
        .collect();
    let thug_resolves: Vec<usize> = events
        .iter()
        .enumerate()
        .filter_map(|(i, e)| {
            matches!(e, GameEvent::SpellResolved { card } if *card == thug).then_some(i)
        })
        .collect();
    let payments: Vec<Vec<Color>> = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if *card == thug => Some(colors.clone()),
            _ => None,
        })
        .collect();
    if thug_casts.len() < 2
        || thug_resolves.len() < 2
        || payments.len() < 2
        || payments
            .iter()
            .take(2)
            .any(|spent| spent.len() != 2 || !spent.contains(&Color::Black))
    {
        return Err(format!(
            "Thug was not cast twice with {{1}}{{B}} payment: {payments:?}"
        ));
    }
    let brownscale_cast = events.iter().position(|event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == brownscale)).ok_or("setup Brownscale was not cast")?;
    let brownscale_resolved = events
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == brownscale))
        .ok_or("setup Brownscale did not resolve")?;
    let watchwolf_attacks: Vec<usize> = events
        .iter()
        .enumerate()
        .filter_map(|(i, event)| match event {
            GameEvent::AttackersDeclared {
                player: PlayerId(1),
                attackers,
            } if attackers.contains(&watchwolf) => Some(i),
            _ => None,
        })
        .collect();
    if watchwolf_attacks.len() < 2 {
        return Err("Watchwolf did not attack on both creature turns".into());
    }
    let brownscale_block = events.iter().position(|event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(0), assignments } if assignments.iter().any(|b| b.attacker == watchwolf && b.blocker == brownscale))).ok_or("Brownscale did not block Watchwolf")?;
    let thug_block = events.iter().position(|event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(0), assignments } if assignments.iter().any(|b| b.attacker == watchwolf && b.blocker == thug))).ok_or("Thug did not block Watchwolf")?;
    let damage = |source, permanent| {
        events
            .iter()
            .enumerate()
            .find_map(|(i, event)| match event {
                GameEvent::DamageDealtToPermanent {
                    source: s,
                    permanent: p,
                    amount,
                } if *s == source && *p == permanent => Some((i, *amount)),
                _ => None,
            })
    };
    let brownscale_damage =
        damage(watchwolf, brownscale).ok_or("Watchwolf did not damage Brownscale")?;
    let brownscale_return_damage =
        damage(brownscale, watchwolf).ok_or("Brownscale did not damage Watchwolf")?;
    let thug_damage = damage(watchwolf, thug).ok_or("Watchwolf did not damage Thug")?;
    let thug_return_damage = damage(thug, watchwolf).ok_or("Thug did not damage Watchwolf")?;
    if (
        brownscale_damage.1,
        brownscale_return_damage.1,
        thug_damage.1,
        thug_return_damage.1,
    ) != (3, 2, 3, 1)
    {
        return Err(format!(
            "combat damage differs: {brownscale_damage:?}/{brownscale_return_damage:?}, {thug_damage:?}/{thug_return_damage:?}"
        ));
    }
    let brownscale_death = events
        .iter()
        .enumerate()
        .skip(brownscale_block + 1)
        .find_map(|(i, e)| {
            matches!(e, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == brownscale)
                .then_some(i)
        })
        .ok_or("Brownscale did not enter the graveyard")?;
    let thug_death = events
        .iter()
        .enumerate()
        .skip(thug_block + 1)
        .find_map(|(i, e)| {
            matches!(e, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == thug)
                .then_some(i)
        })
        .ok_or("Thug did not enter the graveyard")?;
    let thug_incarnation = events
        .get(thug_death + 1)
        .and_then(|e| match e {
            GameEvent::ObjectIncarnationAdvanced {
                object,
                incarnation,
            } if *object == thug => Some(*incarnation),
            _ => None,
        })
        .ok_or("Thug's graveyard incarnation receipt is missing")?;
    let stacked = events
        .iter()
        .enumerate()
        .find_map(|(i, e)| match e {
            GameEvent::TriggeredAbilityStacked {
                source,
                source_incarnation,
                ability,
            } if *source == thug && *ability == TRIGGER_ID => Some((i, *source_incarnation)),
            _ => None,
        })
        .ok_or("Thug Dies trigger did not stack")?;
    let resolved = events
        .iter()
        .enumerate()
        .find_map(|(i, e)| match e {
            GameEvent::AbilityResolved {
                source,
                source_incarnation,
                ability,
            } if *source == thug && *ability == TRIGGER_ID => Some((i, *source_incarnation)),
            _ => None,
        })
        .ok_or("Thug Dies trigger did not resolve")?;
    let target_to_library = events
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            matches!(e, GameEvent::CardMoved { card, to: Zone::Library } if *card == brownscale)
                .then_some(i)
        })
        .ok_or("exact target Brownscale did not move to its owner's library top")?;
    if captured.chosen_target != Some((PlayerId(0), thug, brownscale)) {
        return Err(format!(
            "wrong target chooser/source/object: {:?}",
            captured.chosen_target
        ));
    }
    if stacked.1 != thug_incarnation
        || resolved.1 != thug_incarnation
        || captured.thug_graveyard_incarnation != Some(thug_incarnation)
    {
        return Err("Thug trigger lost source-incarnation provenance".into());
    }
    let dredges: Vec<(usize, u8)> = events
        .iter()
        .enumerate()
        .filter_map(|(i, e)| match e {
            GameEvent::Dredged {
                player: PlayerId(0),
                card,
                count,
            } if *card == thug => Some((i, *count)),
            _ => None,
        })
        .collect();
    if dredges.len() != 1 || dredges[0].1 != 4 {
        return Err(format!(
            "expected exactly one Dredge 4 on Thug: {dredges:?}"
        ));
    }
    let mills = validate_dredge_suffix(events, dredges[0].0, thug, brownscale)?;
    let returned = events
        .iter()
        .enumerate()
        .find_map(|(i, e)| {
            matches!(e, GameEvent::CardMoved { card, to: Zone::Hand } if *card == thug).then_some(i)
        })
        .ok_or("same Thug object did not return to hand")?;
    if !(brownscale_cast < brownscale_resolved
        && brownscale_resolved < watchwolf_attacks[0]
        && watchwolf_attacks[0] < brownscale_block
        && brownscale_block < brownscale_damage.0
        && brownscale_damage.0 < brownscale_death
        && brownscale_death < thug_casts[0]
        && thug_casts[0] < thug_resolves[0]
        && thug_resolves[0] < watchwolf_attacks[1]
        && watchwolf_attacks[1] < thug_block
        && thug_block < thug_damage.0
        && thug_damage.0 < thug_death
        && thug_death < stacked.0
        && stacked.0 < resolved.0
        && resolved.0 < target_to_library
        && target_to_library < returned
        && returned < dredges[0].0
        && returned < thug_casts[1]
        && dredges[0].0 < thug_casts[1]
        && thug_casts[1] < thug_resolves[1])
    {
        return Err("combat, trigger target, Dredge 4, return, and recast order differs".into());
    }
    let terminal = events
        .iter()
        .rposition(|event| matches!(event, GameEvent::GameEnded { .. }))
        .ok_or("terminal game event is missing")?;
    if thug_resolves[1] >= terminal {
        return Err("recast did not resolve before terminal result".into());
    }
    if captured.thug_pt != Some((1, 1))
        || captured.thug_cost.as_ref() != Some(&ManaCost::with_colors(1, [Color::Black]))
    {
        return Err(format!(
            "Thug's body or mana cost differs: {:?}/{:?}",
            captured.thug_pt, captured.thug_cost
        ));
    }
    if captured.winner != Some(0)
        || captured.turns > 120
        || captured.accepted_moves > 8_000
        || captured.life_totals[1] > 0
    {
        return Err(format!(
            "duel failed to reach bounded pilot win: winner={:?}, turns={}, life={:?}",
            captured.winner, captured.turns, captured.life_totals
        ));
    }
    let projected = project_events(events);
    let projected_events: Vec<Value> = projected.iter().map(|item| json!({"sequence":item.sequence,"turn":item.turn,"kind":item.kind,"facts":item.facts,"canonical":item.canonical})).collect();
    let sequence = |i: usize| {
        projected
            .get(i)
            .map(|item| json!(item.sequence))
            .unwrap_or(Value::Null)
    };
    let ordered_event_sequence: Vec<Value> =
        projected.iter().map(|item| json!(item.sequence)).collect();
    let winner = captured.winner.ok_or("terminal game has no winner")?;
    Ok(json!({
        "id":"rav-golgari-thug-dies-target-dredge-four-terminal-game","seed":seed,
        "digest":format!("fnv1a64:{:016x}",event_digest(events)),"ordered_event_sequence":ordered_event_sequence,
        "event_positions":{"brownscale_cast":sequence(brownscale_cast),"brownscale_resolved":sequence(brownscale_resolved),
            "brownscale_watchwolf_attack":sequence(watchwolf_attacks[0]),"brownscale_block":sequence(brownscale_block),
            "brownscale_damage":sequence(brownscale_damage.0),"brownscale_death":sequence(brownscale_death),
            "thug_cast":sequence(thug_casts[0]),"thug_resolved":sequence(thug_resolves[0]),
            "watchwolf_attack":sequence(watchwolf_attacks[1]),"thug_block":sequence(thug_block),
            "thug_damage":sequence(thug_damage.0),"thug_death":sequence(thug_death),
            "trigger_stacked":sequence(stacked.0),"trigger_resolved":sequence(resolved.0),
            "target_to_library_top":sequence(target_to_library),"dredged":sequence(dredges[0].0),
            "thug_returned_to_hand":sequence(returned),"thug_recast":sequence(thug_casts[1]),
            "thug_recast_resolved":sequence(thug_resolves[1]),"terminal":sequence(terminal)},
        "passed":true,"terminal":format!("Winner(PlayerId({winner}))"),"winner_seat":winner,
        "turns":captured.turns,"accepted_moves":captured.accepted_moves,"rejected_moves":0,
        "brownscale_object":brownscale.0,"thug_object":thug.0,"watchwolf_object":watchwolf.0,
        "thug":{"mana_cost":"{1}{B}","paid_colors":payments[0].iter().map(|c|format!("{c:?}")).collect::<Vec<_>>(),
            "power":captured.thug_pt.map(|pt|pt.0),"toughness":captured.thug_pt.map(|pt|pt.1),
            "printed_subtypes":["Human","Warrior"],"subtypes_asserted":false,"dredge":4},
        "combat":{"setup_object":brownscale.0,"source_object":thug.0,"source_incarnation":thug_incarnation,
            "setup_watchwolf_damage":brownscale_damage.1,"setup_brownscale_damage":brownscale_return_damage.1,
            "brownscale_died":true,"watchwolf_damage":thug_damage.1,"thug_damage":thug_return_damage.1,"thug_died":true},
        "trigger":{"actor_seat":captured.chosen_target.map(|c|c.0.0),"source_object":thug.0,"target_object":brownscale.0,
            "source_incarnation":stacked.1,"target_moved_to_library_top":brownscale.0},
        "dredge":{"card_object":thug.0,"count":4,"mill_objects":mills.iter().map(|id|id.0).collect::<Vec<_>>(),
            "top_target_milled_first":mills.first().copied()==Some(brownscale),"same_object_returned_to_hand":true,"same_object_recast":true},
        "deterministic_replay":true,"life_totals":captured.life_totals,"events":projected_events
    }))
}

fn validate_dredge_suffix(
    events: &[GameEvent],
    dredge_index: usize,
    thug: ObjectId,
    top_target: ObjectId,
) -> Result<Vec<ObjectId>, String> {
    if dredge_index < 10 {
        return Err("Dredge event has too little preceding zone provenance".into());
    }
    let suffix = &events[dredge_index - 10..=dredge_index];
    let mut mills = Vec::new();
    for pair in suffix[..8].chunks_exact(2) {
        match pair {
            [
                GameEvent::CardMoved {
                    card,
                    to: Zone::Graveyard,
                },
                GameEvent::ObjectIncarnationAdvanced { object, .. },
            ] if card == object => mills.push(*card),
            _ => {
                return Err(format!(
                    "Dredge did not mill four exact objects in order: {pair:?}"
                ));
            }
        }
    }
    match &suffix[8..] {
        [
            GameEvent::CardMoved {
                card,
                to: Zone::Hand,
            },
            GameEvent::ObjectIncarnationAdvanced { object, .. },
            GameEvent::Dredged {
                player: PlayerId(0),
                card: dredged,
                count: 4,
            },
        ] if *card == thug && *object == thug && *dredged == thug => {}
        other => {
            return Err(format!(
                "Dredge did not return the same Thug object to hand: {other:?}"
            ));
        }
    }
    if mills.len() != 4
        || mills.first().copied() != Some(top_target)
        || mills.contains(&thug)
        || mills.iter().copied().collect::<BTreeSet<_>>().len() != 4
    {
        return Err(format!("Dredge mill order/count differs: {mills:?}"));
    }
    Ok(mills)
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Golgari Thug source: {error}"))?;
    validate_candidate_source_text(&source)
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let file = syn::parse_file(source)
        .map_err(|_| "Golgari Thug candidate source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
        || file.items.len() != 4
    {
        return Err("Golgari Thug candidate source AST differs from RAV literal contract".into());
    }
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) => {}
            Item::Fn(function)
                if function.attrs.iter().all(|attribute| {
                    attribute.path().is_ident("must_use") || attribute.path().is_ident("doc")
                }) && matches!(&function.vis, Visibility::Restricted(_))
                    && function.sig.inputs.is_empty()
                    && function.sig.generics.params.is_empty()
                    && function.sig.generics.where_clause.is_none() =>
            {
                let (name, result) =
                    match function.sig.ident.to_string().as_str() {
                        "definition" => ("definition", "CardDefinition"),
                        "triggered_ability" => ("triggered_ability", "TriggeredAbilityBinding"),
                        _ => return Err(
                            "Golgari Thug candidate source AST differs from RAV literal contract"
                                .into(),
                        ),
                    };
                let [Stmt::Expr(Expr::Struct(literal), None)] = function.block.stmts.as_slice()
                else {
                    return Err(
                        "Golgari Thug candidate source AST differs from RAV literal contract"
                            .into(),
                    );
                };
                if !return_type_is(&function.sig.output, result)
                    || !path_is_type(&literal.path, result)
                    || literal.rest.is_some()
                    || !functions.insert(name.to_owned())
                {
                    return Err(
                        "Golgari Thug candidate source AST differs from RAV literal contract"
                            .into(),
                    );
                }
            }
            _ => {
                return Err(
                    "Golgari Thug candidate source AST differs from RAV literal contract".into(),
                );
            }
        }
    }
    let expected = syn::parse_file(EXPECTED_CANDIDATE_SOURCE)
        .expect("trusted Golgari Thug source contract parses");
    if functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()])
        || format!("{file:?}") != format!("{expected:?}")
    {
        return Err("Golgari Thug candidate source AST differs from RAV literal contract".into());
    }
    Ok(())
}

fn return_type_is(output: &ReturnType, expected: &str) -> bool {
    matches!(output, ReturnType::Type(_, ty) if matches!(&**ty, syn::Type::Path(path) if path.qself.is_none() && path_is_type(&path.path, expected)))
}

fn path_is_type(path: &syn::Path, expected: &str) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == 1
        && path.segments[0].ident == expected
        && matches!(path.segments[0].arguments, syn::PathArguments::None)
}

fn malicious_candidate_source_rejected() -> Result<bool, String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Golgari Thug source: {error}"))?;
    let signature = "pub(crate) fn definition() -> CardDefinition {";
    let injected = "pub(crate) fn definition() -> CardDefinition {\n    std::process::exit(0);";
    let malicious = source.replacen(signature, injected, 1);
    if malicious == source || syn::parse_file(&malicious).is_err() {
        return Err(
            "compile-valid Golgari Thug side-effect candidate could not be constructed".into(),
        );
    }
    Ok(validate_candidate_source_text(&malicious).is_err())
}

fn validate_candidate_definition() -> Result<(), String> {
    let definition = candidate_golgari_thug::definition();
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(1, [Color::Black])
        || definition.colors != BTreeSet::from([Color::Black])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || (definition.power, definition.toughness) != (Some(1), Some(1))
        || definition.keywords != [Keyword::Dredge(4)]
        || !definition.effects.is_empty()
        || definition.supported_rules
            != [
                "full-rules-fidelity",
                "dredge",
                "base-characteristics",
                TRIGGER_ID,
            ]
    {
        return Err("Golgari Thug definition differs from RAV #87 identity".into());
    }
    Ok(())
}

fn validate_candidate_binding() -> Result<(), String> {
    let expected = TriggeredAbilityBinding {
        card_definition: CARD_ID,
        ability: TriggeredAbility {
            id: TRIGGER_ID,
            condition: TriggerCondition::Dies,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::CreatureCardInControllerGraveyard],
            effects: vec![Effect::PutTargetCreatureCardInControllerGraveyardOnOwnersLibraryTop],
        },
    };
    if candidate_golgari_thug::triggered_ability() != expected {
        return Err("Golgari Thug trigger differs from RAV #87's targeted Dies trigger".into());
    }
    Ok(())
}

fn validate_registry() -> Result<(), String> {
    let definitions: Vec<_> = card_definitions()
        .into_iter()
        .filter(|definition| definition.id == CARD_ID)
        .collect();
    let expected = candidate_golgari_thug::definition();
    if definitions.len() != 1
        || definitions[0].name != expected.name
        || definitions[0].mana_cost != expected.mana_cost
        || definitions[0].colors != expected.colors
        || definitions[0].card_types != expected.card_types
        || (definitions[0].power, definitions[0].toughness) != (expected.power, expected.toughness)
        || definitions[0].keywords != expected.keywords
    {
        return Err("RAV registry does not match candidate-owned Golgari Thug definition".into());
    }
    let bindings: Vec<_> = rav_triggered_ability_bindings()
        .into_iter()
        .filter(|binding| binding.card_definition == CARD_ID)
        .collect();
    if bindings.len() != 1 || bindings[0] != candidate_golgari_thug::triggered_ability() {
        return Err(
            "RAV registry does not delegate Golgari Thug's trigger to its candidate module".into(),
        );
    }
    Ok(())
}

fn mutated_binding(mutation: Mutation) -> TriggeredAbilityBinding {
    let mut binding = candidate_golgari_thug::triggered_ability();
    match mutation {
        Mutation::WrongTriggerCondition => {
            binding.ability.condition = TriggerCondition::GraveyardToHand
        }
        Mutation::WrongTriggerTarget => binding.ability.targets = vec![TargetRequirement::Opponent],
        Mutation::WrongTriggerEffect => {
            binding.ability.effects = vec![Effect::PutTargetCreatureOnOwnersLibraryTop]
        }
        _ => {}
    }
    binding
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
    {
        return Err("RAV catalog identity or printing denominator changed".to_owned());
    }
    Ok(catalog)
}

fn card_status(card: &CatalogCard) -> Value {
    let (engine_status, definition_id) = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => {
            ("executable_compatibility_slice", Some(definition_id))
        }
        CardSemanticStatus::CatalogOnly { .. } => ("catalog_only", None),
    };
    let passed = definition_id == Some(CARD_ID);
    json!({
        "collector_number": card.collector_number,
        "definition_id": definition_id,
        "name": card.name,
        "engine_status": engine_status,
        "qualification": if passed {"diagnostic_trace_passed; independent review pending"} else {"not_evaluated"},
        "trace_status": if passed {"passed"} else {"not_qualified"},
        "credited": false
    })
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}

fn output_root() -> Option<std::path::PathBuf> {
    std::env::args_os()
        .collect::<Vec<_>>()
        .windows(2)
        .find(|pair| pair[0] == "--output-root")
        .map(|pair| pair[1].clone().into())
}

fn write_outputs(root: &std::path::Path, report: &Value) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(root.join("reward.txt"), "0.00000000\n").map_err(|error| error.to_string())
}

fn event_digest(events: &[GameEvent]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in format!("{events:?}").bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x00000100000001b3);
    }
    hash
}
