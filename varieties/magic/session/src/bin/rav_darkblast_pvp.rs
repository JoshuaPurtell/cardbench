//! Full-game RAV Darkblast authority for its targeted -1/-1 effect and Dredge 3.

use cardbench_magic_engine::{
    CardType, Color, CombatBlock, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, Keyword,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};
use syn::parse::Parser;
use syn::{Expr, Item, ReturnType, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-DARKBLAST";
const CARD_NAME: &str = "Darkblast";
const TARGET_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/darkblast.rs";
const DECK_P0: &str = "rav_darkblast_black_green_white";
const DECK_P1: &str = "rav_watchwolf_green_white";
const MAX_TURNS: u32 = 80;
const MAX_MOVES: u32 = 5_000;
const EXPECTED_CANDIDATE_SOURCE: &str = r#"//! Candidate-owned RAV Darkblast instant definition.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, Keyword, ManaCost};
use std::collections::BTreeSet;

/// RAV #82 is a black instant with a temporary -1/-1 modifier and Dredge 3.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-DARKBLAST",
        name: "Darkblast",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["full-rules-fidelity", "targeted-layer-7-modifier", "dredge"],
        power: None,
        toughness: None,
        keywords: vec![Keyword::Dredge(3)],
        effects: vec![Effect::ModifyTargetPtUntilEndOfTurn {
            power: -1,
            toughness: -1,
        }],
    }
}
"#;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/darkblast.rs"]
mod candidate_darkblast;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Darkblast PvP result: {error}");
                    return ExitCode::FAILURE;
                }
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
            eprintln!("Darkblast PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    let malicious_candidate_rejected = malicious_candidate_source_rejected()?;
    validate_candidate_definition()?;
    validate_registry()?;
    let catalog = unique_catalog()?;
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured != replay {
        return Err("Darkblast full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "wrong_mana_cost_detected_in_game": mutation_detected(seed, Mutation::WrongCost),
        "wrong_modifier_detected_in_game": mutation_detected(seed, Mutation::WrongModifier),
        "dredge_removed_detected_in_game": mutation_detected(seed, Mutation::NoDredge),
        "wrong_dredge_count_detected_in_game": mutation_detected(seed, Mutation::DredgeFour),
        "compile_valid_candidate_side_effect_rejected_before_execution": malicious_candidate_rejected,
        "sealed_mutation_authority": false
    });
    for key in [
        "wrong_mana_cost_detected_in_game",
        "wrong_modifier_detected_in_game",
        "dredge_removed_detected_in_game",
        "wrong_dredge_count_detected_in_game",
        "compile_valid_candidate_side_effect_rejected_before_execution",
    ] {
        if mutations[key] != true {
            return Err(format!("Darkblast semantic mutation escaped: {key}"));
        }
    }
    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,
        "expansion":"RAV",
        "card":{"collector_number":82,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":[
                "105.2 and 202.1-202.2: black spell color and colored payment",
                "106.1 and 106.4: mana production and spell payment",
                "117: priority and instant-speed response windows",
                "109.5 and 115: target identity and legal creature target",
                "601.2 and 608.2b-608.3a: casting, target legality, and spell resolution",
                "613.4: power/toughness modification layer",
                "704.5f: creature with toughness zero or less is put into its owner's graveyard",
                "510: combat damage and terminal game state"
            ],
            "source_contract":"RAV Darkblast #82 is a black {B} instant: target creature gets -1/-1 until end of turn; Dredge 3.",
            "oracle_reconciliation":"The official RAV text explicitly describes Dredge replacement and fallback draw; current Oracle's reminder wording is semantically equivalent. The -1/-1 target effect and Dredge 3 are unchanged.",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true
        },
        "passed":true,
        "pilot_card_passed":true,
        "replay_deterministic":true,
        "cards_passed":cards_passed,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":"diagnostic_only; independent review pending",
        "mutation_sensitivity":mutations,
        "candidate_source_validation":{"candidate_ast_contract_matched":true,
            "compile_valid_injected_side_effect_candidate_rejected":malicious_candidate_rejected,
            "guard_ran_before_candidate_function_calls":true},
        "traces":[trace],
        "per_card_breakdown":breakdown
    }))
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

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
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
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(root.join("reward.txt"), "0.00000000\n").map_err(|e| e.to_string())
}
fn card_status(card: &CatalogCard) -> Value {
    let (engine_status, definition_id) = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => {
            ("executable_compatibility_slice", Some(definition_id))
        }
        CardSemanticStatus::CatalogOnly { .. } => ("catalog_only", None),
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"definition_id":definition_id,"name":card.name,"engine_status":engine_status,"qualification":if passed{"pilot_trace_passed; independent reviewer approval pending"}else{"not_evaluated"},"trace_status":if passed{"passed"}else{"not_qualified"},"credited":false})
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongCost,
    WrongModifier,
    NoDredge,
    DredgeFour,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ModifierObservation {
    source: ObjectId,
    target: ObjectId,
    power: Option<i32>,
    toughness: Option<i32>,
    zone: Option<Zone>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    life_totals: [i64; 2],
    darkblast: Option<ObjectId>,
    pilot_watchwolf: Option<ObjectId>,
    opponent_watchwolves: Vec<ObjectId>,
    darkblast_payments: Vec<Vec<Color>>,
    chosen_targets: Vec<(ObjectId, ObjectId)>,
    dredge_choices: Vec<ObjectId>,
    modifiers: Vec<ModifierObservation>,
}

fn find_trace() -> Result<(u64, Captured), String> {
    let mut last_error = String::new();
    for seed in 0..=512 {
        match run_match(seed, Mutation::None)
            .and_then(|captured| validate_trace(&captured, seed).map(|_| captured))
        {
            Ok(captured) => return Ok((seed, captured)),
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "no bounded Darkblast terminal-game trace found for seeds 0..=512: {last_error}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    run_match(seed, mutation)
        .and_then(|captured| validate_trace(&captured, seed).map(|_| ()))
        .is_err()
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: TARGET_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-SWAMP".to_owned(),
                count: 20,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 16,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 16,
            },
        ],
        sideboard: vec![],
    };
    let opponent = DeckList {
        mainboard: vec![
            DeckEntry {
                card: TARGET_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 28,
            },
        ],
        sideboard: vec![],
    };
    for (label, deck) in [(DECK_P0, &pilot), (DECK_P1, &opponent)] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{label} deck has {count} cards; expected 60"));
        }
    }
    let mut definitions = card_definitions();
    let mut submitted = candidate_darkblast::definition();
    match mutation {
        Mutation::None => {}
        Mutation::WrongCost => submitted.mana_cost = ManaCost::with_colors(1, [Color::Black]),
        Mutation::WrongModifier => {
            submitted.effects = vec![Effect::ModifyTargetPtUntilEndOfTurn {
                power: -2,
                toughness: -2,
            }]
        }
        Mutation::NoDredge => submitted.keywords.clear(),
        Mutation::DredgeFour => submitted.keywords = vec![Keyword::Dredge(4)],
    }
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Darkblast definition is absent from the RAV registry")? = submitted;
    let mut game = Game::new_with_all_bindings(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent)
        .map_err(|error| error.to_string())?;
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        ..DeckMatchConfig::default()
    };
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let chosen_targets = Arc::new(Mutex::new(Vec::new()));
    let dredge_choices = Arc::new(Mutex::new(Vec::new()));
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(DarkblastPolicy::new(
            PlayerId(0),
            Arc::clone(&chosen_targets),
            Arc::clone(&dredge_choices),
        )),
        Box::new(DarkblastPolicy::new(
            PlayerId(1),
            Arc::clone(&chosen_targets),
            Arc::clone(&dredge_choices),
        )),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut modifiers = Vec::new();
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || accepted >= MAX_MOVES {
            return Err("Darkblast match exceeded the bounded turn or move limit".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("missing seat policy")?;
        let action = if view.draw_replacement_pending {
            policy.propose_draw_replacement(&view)
        } else if let Some(action) = policy.propose_pending_decision(&view) {
            action
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
        attempted += 1;
        let cursor = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "policy move rejected on turn {} at {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        for event in &game.event_log[cursor..] {
            if let GameEvent::ContinuousEffectCreated { source, target, .. } = event
                && game
                    .card_definition(*source)
                    .is_ok_and(|definition| definition.id == CARD_ID)
            {
                let characteristics = game.characteristics(*target).ok();
                modifiers.push(ModifierObservation {
                    source: *source,
                    target: *target,
                    power: characteristics.as_ref().and_then(|value| value.power),
                    toughness: characteristics.as_ref().and_then(|value| value.toughness),
                    zone: game.zone_of(*target),
                });
            }
        }
    }
    let events = game.event_log.clone();
    let darkblast = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|definition| definition.id == CARD_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let pilot_watchwolf = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|definition| definition.id == TARGET_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let opponent_watchwolves = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::SpellCast {
                player: PlayerId(1),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == TARGET_ID) =>
            {
                Some(*card)
            }
            _ => None,
        })
        .collect();
    let darkblast_payments = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if Some(*card) == darkblast => Some(colors.clone()),
            _ => None,
        })
        .collect();
    let chosen_targets = chosen_targets
        .lock()
        .map_err(|_| "target observation lock poisoned")?
        .clone();
    let dredge_choices = dredge_choices
        .lock()
        .map_err(|_| "Dredge observation lock poisoned")?
        .clone();
    Ok(Captured {
        events,
        terminal: game.winner().map_or_else(
            || "Draw".to_owned(),
            |player| format!("Winner(PlayerId({}))", player.0),
        ),
        winner: game.winner().map(|player| player.0),
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempted.saturating_sub(accepted),
        life_totals: [
            game.player(PlayerId(0))
                .map_err(|error| error.to_string())?
                .life,
            game.player(PlayerId(1))
                .map_err(|error| error.to_string())?
                .life,
        ],
        darkblast,
        pilot_watchwolf,
        opponent_watchwolves,
        darkblast_payments,
        chosen_targets,
        dredge_choices,
        modifiers,
    })
}

struct DarkblastPolicy {
    player: PlayerId,
    chosen_targets: Arc<Mutex<Vec<(ObjectId, ObjectId)>>>,
    dredge_choices: Arc<Mutex<Vec<ObjectId>>>,
    darkblast_source: Option<ObjectId>,
    darkblast_casts: usize,
    dredge_used: bool,
    watchwolf_casts: usize,
}

impl DarkblastPolicy {
    fn new(
        player: PlayerId,
        chosen_targets: Arc<Mutex<Vec<(ObjectId, ObjectId)>>>,
        dredge_choices: Arc<Mutex<Vec<ObjectId>>>,
    ) -> Self {
        Self {
            player,
            chosen_targets,
            dredge_choices,
            darkblast_source: None,
            darkblast_casts: 0,
            dredge_used: false,
            watchwolf_casts: 0,
        }
    }

    fn target_already_chosen(&self, target: ObjectId) -> bool {
        self.chosen_targets
            .lock()
            .is_ok_and(|targets| targets.iter().any(|(_, chosen)| *chosen == target))
    }

    fn cast(
        &self,
        view: &GameView,
        card: ObjectId,
        cost: &ManaCost,
        target: Option<ObjectId>,
    ) -> Option<PolicyAction> {
        let mana_selection = payment(view, cost)?;
        Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card,
                targets: target.into_iter().map(Target::Permanent).collect(),
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection,
        })
    }
}

impl CodePolicy for DarkblastPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.darkblast-dredge-pilot.v1"
        } else {
            "rav.watchwolf-target-and-block-policy.v1"
        }
    }

    fn propose_draw_replacement(&mut self, view: &GameView) -> PolicyAction {
        if self.player == PlayerId(0) && !self.dredge_used {
            if let Some(source) = self
                .darkblast_source
                .filter(|source| view.dredge_candidates.iter().any(|card| card.id == *source))
            {
                self.dredge_used = true;
                if let Ok(mut choices) = self.dredge_choices.lock() {
                    choices.push(source);
                }
                return PolicyAction::Draw {
                    decision: view
                        .draw_replacement_decision
                        .expect("draw replacement has decision id"),
                    dredge: Some(source),
                };
            }
        }
        PolicyAction::Draw {
            decision: view
                .draw_replacement_decision
                .expect("draw replacement has decision id"),
            dredge: None,
        }
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = if self.player == PlayerId(0) {
                let marked_target_alive = view.opponent_battlefield.iter().any(|card| {
                    card.definition == Some(TARGET_ID) && self.target_already_chosen(card.id)
                });
                let no_opposing_watchwolf = !view
                    .opponent_battlefield
                    .iter()
                    .any(|card| card.definition == Some(TARGET_ID));
                if marked_target_alive || no_opposing_watchwolf {
                    view.own_battlefield
                        .iter()
                        .filter(|card| card.definition == Some(TARGET_ID) && card.can_attack)
                        .map(|card| card.id)
                        .collect()
                } else {
                    Vec::new()
                }
            } else {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(TARGET_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect()
            };
            return PolicyAction::DeclareAttackers { attackers };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            let assignments = if self.player == PlayerId(1) {
                let attacker = view
                    .combat_attackers
                    .iter()
                    .find(|card| card.definition == Some(TARGET_ID))
                    .map(|card| card.id);
                let blocker = view
                    .own_battlefield
                    .iter()
                    .find(|card| {
                        card.definition == Some(TARGET_ID)
                            && card.can_block
                            && self.target_already_chosen(card.id)
                    })
                    .map(|card| card.id);
                attacker
                    .zip(blocker)
                    .map(|(attacker, blocker)| vec![CombatBlock { attacker, blocker }])
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            return PolicyAction::DeclareBlockers { assignments };
        }
        if view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if self.player == PlayerId(0) {
            let target = view.opponent_battlefield.iter().find(|card| {
                card.definition == Some(TARGET_ID) && !self.target_already_chosen(card.id)
            });
            let candidate = if self.darkblast_casts == 0 {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
                    .map(|card| card.id)
            } else if self.darkblast_casts == 1 && self.dredge_used {
                self.darkblast_source.filter(|source| {
                    view.hand
                        .iter()
                        .any(|card| card.id == *source && card.definition == Some(CARD_ID))
                })
            } else {
                None
            };
            if let (Some(target), Some(card)) = (target, candidate) {
                let cost = ManaCost::with_colors(0, [Color::Black]);
                if let Some(action) = self.cast(view, card, &cost, Some(target.id)) {
                    if self.darkblast_source.is_none() {
                        self.darkblast_source = Some(card);
                    }
                    self.darkblast_casts += 1;
                    if let Ok(mut choices) = self.chosen_targets.lock() {
                        choices.push((card, target.id));
                    }
                    return action;
                }
                if let Some(action) = make_mana(view, &cost) {
                    return action;
                }
            }
            if self.watchwolf_casts == 0
                && !view
                    .own_battlefield
                    .iter()
                    .any(|card| card.definition == Some(TARGET_ID))
            {
                if let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(TARGET_ID))
                {
                    let cost = ManaCost::with_colors(1, [Color::Green, Color::White]);
                    if let Some(action) = self.cast(view, card.id, &cost, None) {
                        self.watchwolf_casts += 1;
                        return action;
                    }
                    if let Some(action) = make_mana(view, &cost) {
                        return action;
                    }
                }
            }
        } else if self.watchwolf_casts < 2 {
            let current = view
                .own_battlefield
                .iter()
                .filter(|card| card.definition == Some(TARGET_ID))
                .count();
            if current < 2 {
                if let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(TARGET_ID))
                {
                    let cost = ManaCost::with_colors(1, [Color::Green, Color::White]);
                    if let Some(action) = self.cast(view, card.id, &cost, None) {
                        self.watchwolf_casts += 1;
                        return action;
                    }
                    if let Some(action) = make_mana(view, &cost) {
                        return action;
                    }
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<_> = Color::MANA_ALL
        .into_iter()
        .map(|c| (c, view.mana_pool.amount(c)))
        .collect();
    for color in &cost.colored {
        let (_, amount) = available.iter_mut().find(|(c, _)| c == color)?;
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
fn make_mana(view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
    let mut need = None;
    for color in &cost.colored {
        let required = cost.colored.iter().filter(|c| *c == color).count() as u8;
        if view.mana_pool.amount(*color) < required {
            need = Some(*color);
            break;
        }
    }
    if need.is_none()
        && view.mana_pool.total() < cost.generic.saturating_add(cost.colored.len() as u8)
    {
        need = Some(Color::Black);
    }
    let preferred = need?;
    let land = view
        .own_battlefield
        .iter()
        .find(|card| {
            card.card_types.contains(&CardType::Land)
                && !card.tapped
                && card.mana_colors.contains(&preferred)
        })
        .or_else(|| {
            view.own_battlefield.iter().find(|card| {
                card.card_types.contains(&CardType::Land)
                    && !card.tapped
                    && !card.mana_colors.is_empty()
            })
        })?;
    let color = if land.mana_colors.contains(&preferred) {
        preferred
    } else {
        *land.mana_colors.iter().next()?
    };
    Some(PolicyAction::ActivateManaAbility {
        land: land.id,
        color,
    })
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    let events = &captured.events;
    let darkblast = captured.darkblast.ok_or("Darkblast was not cast")?;
    let pilot_watchwolf = captured
        .pilot_watchwolf
        .ok_or("pilot Watchwolf was not cast")?;
    if captured.opponent_watchwolves.len() != 2
        || captured.opponent_watchwolves[0] == captured.opponent_watchwolves[1]
    {
        return Err(format!(
            "expected two distinct opposing Watchwolves: {:?}",
            captured.opponent_watchwolves
        ));
    }
    let casts: Vec<usize> = events.iter().enumerate().filter_map(|(index, event)| {
        matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == darkblast).then_some(index)
    }).collect();
    let resolves: Vec<usize> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            matches!(event, GameEvent::SpellResolved { card } if *card == darkblast)
                .then_some(index)
        })
        .collect();
    if casts.len() != 2 || resolves.len() != 2 || casts[0] >= resolves[0] || casts[1] >= resolves[1]
    {
        return Err(format!(
            "same Darkblast object was not cast and resolved twice: cast={casts:?}, resolved={resolves:?}"
        ));
    }
    if captured.darkblast_payments.len() != 2
        || captured
            .darkblast_payments
            .iter()
            .any(|colors| colors.len() != 1 || !colors.contains(&Color::Black))
    {
        return Err(format!(
            "both Darkblast casts must pay actual {{B}}: {:?}",
            captured.darkblast_payments
        ));
    }
    if captured.chosen_targets.len() != 2
        || captured
            .chosen_targets
            .iter()
            .any(|(source, _)| *source != darkblast)
    {
        return Err(format!(
            "the exact same Darkblast source must choose two targets: {:?}",
            captured.chosen_targets
        ));
    }
    let target_ids: BTreeSet<_> = captured
        .chosen_targets
        .iter()
        .map(|(_, target)| *target)
        .collect();
    let opponent_ids: BTreeSet<_> = captured.opponent_watchwolves.iter().copied().collect();
    if target_ids != opponent_ids || captured.modifiers.len() != 2 {
        return Err(format!(
            "Darkblast choices/modifiers did not match the two opposing Watchwolves: targets={target_ids:?}, modifiers={:?}",
            captured.modifiers
        ));
    }
    for observation in &captured.modifiers {
        if observation.source != darkblast
            || !opponent_ids.contains(&observation.target)
            || (observation.power, observation.toughness) != (Some(2), Some(2))
            || observation.zone != Some(Zone::Battlefield)
        {
            return Err(format!(
                "Darkblast did not apply exactly -1/-1 to a surviving 3/3 Watchwolf: {observation:?}"
            ));
        }
    }
    let dredge_events: Vec<(usize, u8)> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::Dredged {
                player: PlayerId(0),
                card,
                count,
            } if *card == darkblast => Some((index, *count)),
            _ => None,
        })
        .collect();
    if dredge_events.len() != 1 || dredge_events[0].1 != 3 || captured.dredge_choices != [darkblast]
    {
        return Err(format!(
            "the same Darkblast must replace one draw with Dredge 3: {dredge_events:?}; choices={:?}",
            captured.dredge_choices
        ));
    }
    let dredge_position = dredge_events[0].0;
    let returned = events
        .iter()
        .enumerate()
        .take(dredge_position)
        .rev()
        .find_map(|(index, event)| {
            matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == darkblast)
                .then_some(index)
        })
        .ok_or("Dredge did not return the same Darkblast object to hand")?;
    let mills = validate_dredge_suffix(events, dredge_position, darkblast)?;
    let first_graveyard = event_index_after(events, resolves[0], |event| {
        matches!(event,
        GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == darkblast)
    })
    .ok_or("first resolved Darkblast did not reach its graveyard before Dredge")?;
    if !(casts[0] < resolves[0]
        && resolves[0] < first_graveyard
        && first_graveyard < returned
        && returned < dredge_position
        && dredge_position < casts[1]
        && casts[1] < resolves[1])
    {
        return Err(
            "first resolution, graveyard-to-hand Dredge 3, and same-object recast order differs"
                .into(),
        );
    }
    let mut target_event_positions = Vec::new();
    for target in &captured.opponent_watchwolves {
        let target_cast = event_index(events, |event| {
            matches!(event,
            GameEvent::SpellCast { player: PlayerId(1), card } if *card == *target)
        })
        .ok_or("opponent Watchwolf cast receipt missing")?;
        let target_resolved = event_index_after(events, target_cast, |event| {
            matches!(event,
            GameEvent::SpellResolved { card } if *card == *target)
        })
        .ok_or("opponent Watchwolf resolution missing")?;
        let modifier = events.iter().enumerate().find_map(|(index, event)| matches!(event,
            GameEvent::ContinuousEffectCreated { source, target: affected, .. } if *source == darkblast && *affected == *target).then_some(index)).ok_or("Darkblast modifier source/target event missing")?;
        let block = event_index(events, |event| matches!(event,
            GameEvent::BlockersDeclared { player: PlayerId(1), assignments }
                if assignments.iter().any(|assignment| assignment.attacker == pilot_watchwolf && assignment.blocker == *target))).ok_or("modified Watchwolf did not block the pilot's Watchwolf")?;
        let pilot_damage = event_index_after(events, block, |event| matches!(event,
            GameEvent::DamageDealtToPermanent { source, permanent, amount: 3 } if *source == pilot_watchwolf && *permanent == *target)).ok_or("pilot Watchwolf did not deal three combat damage to the exact modified target")?;
        let target_damage = event_index_after(events, block, |event| matches!(event,
            GameEvent::DamageDealtToPermanent { source, permanent, amount: 2 } if *source == *target && *permanent == pilot_watchwolf)).ok_or("modified Watchwolf did not deal two combat damage back")?;
        let sba = event_index_after(events, pilot_damage.max(target_damage), |event| matches!(event,
            GameEvent::StateBasedAction { card, reason: "creature has lethal damage" } if *card == *target)).ok_or("modified Watchwolf did not die from combat damage")?;
        let graveyard = event_index_after(events, sba, |event| {
            matches!(event,
            GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == *target)
        })
        .ok_or("modified Watchwolf did not move to the graveyard after combat")?;
        if !(target_cast < target_resolved
            && target_resolved < modifier
            && modifier < block
            && block < pilot_damage
            && block < target_damage
            && pilot_damage < sba
            && target_damage < sba
            && sba < graveyard)
        {
            return Err(
                "Watchwolf cast, Darkblast modifier, combat, and lethal-damage SBA order differs"
                    .into(),
            );
        }
        target_event_positions.push((
            target_cast,
            target_resolved,
            modifier,
            block,
            pilot_damage,
            target_damage,
            sba,
            graveyard,
        ));
    }
    let terminal = event_index(events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("normal seat-zero terminal game event is missing")?;
    let final_attack = events.iter().enumerate().find_map(|(index, event)| matches!(event,
        GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&pilot_watchwolf)
            && index > target_event_positions.iter().map(|positions| positions.7).max().unwrap_or(0)).then_some(index))
        .ok_or("pilot Watchwolf did not attack after both targeted Watchwolves died")?;
    let final_damage = event_index_after(events, final_attack, |event| matches!(event,
        GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 3 } if *source == pilot_watchwolf))
        .ok_or("pilot Watchwolf did not deal terminal combat damage")?;
    if final_damage >= terminal
        || captured.winner != Some(0)
        || captured.terminal != "Winner(PlayerId(0))"
        || captured.turns > MAX_TURNS
        || captured.accepted_moves > MAX_MOVES
        || captured.rejected_moves != 0
        || captured.life_totals[0] <= 0
        || captured.life_totals[1] > 0
    {
        return Err(format!(
            "bounded legal duel did not reach a clean pilot win: winner={:?}, life={:?}",
            captured.winner, captured.life_totals
        ));
    }
    let projected = project_events(events);
    let event_values: Vec<Value> = projected.iter().map(|event| json!({"sequence":event.sequence,"turn":event.turn,"kind":event.kind,"canonical":event.canonical})).collect();
    let positions = json!({
        "first_darkblast_cast":casts[0], "first_darkblast_resolved":resolves[0], "darkblast_to_hand":returned,
        "dredged":dredge_position, "darkblast_recast":casts[1], "darkblast_recast_resolved":resolves[1],
        "first_target_cast":target_event_positions[0].0, "first_target_resolved":target_event_positions[0].1,
        "first_modifier":target_event_positions[0].2, "first_block":target_event_positions[0].3,
        "first_target_sba":target_event_positions[0].6, "first_target_graveyard":target_event_positions[0].7,
        "second_target_cast":target_event_positions[1].0, "second_target_resolved":target_event_positions[1].1,
        "second_modifier":target_event_positions[1].2, "second_block":target_event_positions[1].3,
        "second_target_sba":target_event_positions[1].6, "second_target_graveyard":target_event_positions[1].7,
        "pilot_final_attack":final_attack, "pilot_final_damage":final_damage, "terminal":terminal
    });
    Ok(json!({
        "id":"rav-darkblast-minus-one-dredge-three-terminal-pvp", "passed":true, "seed":seed,
        "terminal":captured.terminal, "winner_seat":captured.winner, "turns":captured.turns,
        "accepted_moves":captured.accepted_moves, "rejected_moves":captured.rejected_moves,
        "darkblast_object":darkblast.0, "pilot_watchwolf_object":pilot_watchwolf.0,
        "opponent_watchwolf_objects":captured.opponent_watchwolves.iter().map(|id|id.0).collect::<Vec<_>>(),
        "decks":[DECK_P0,DECK_P1],
        "darkblast":{"mana_cost":"{B}","paid_colors":captured.darkblast_payments[0].iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"cast_payments":captured.darkblast_payments.iter().map(|colors|colors.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>()).collect::<Vec<_>>(),"dredge":3},
        "chosen_targets":captured.chosen_targets.iter().map(|(source,target)|json!({"source":source.0,"target":target.0})).collect::<Vec<_>>(),
        "modifier_observations":captured.modifiers.iter().map(|item|json!({"source":item.source.0,"target":item.target.0,"power":item.power,"toughness":item.toughness,"zone":item.zone})).collect::<Vec<_>>(),
        "dredge":{"card_object":darkblast.0,"count":3,"mill_objects":mills.iter().map(|id|id.0).collect::<Vec<_>>(),"same_object_returned_to_hand":true,"same_object_recast":true},
        "event_positions":positions,"ordered_event_sequence":projected.iter().map(|event|event.sequence).collect::<Vec<_>>(),
        "life_totals":captured.life_totals,"deterministic_replay":true,"events":event_values
    }))
}

fn validate_dredge_suffix(
    events: &[GameEvent],
    dredged: usize,
    darkblast: ObjectId,
) -> Result<Vec<ObjectId>, String> {
    if dredged < 8 {
        return Err("Dredge event lacks the required preceding movement receipts".into());
    }
    let suffix = &events[dredged - 8..=dredged];
    let mut mills = Vec::new();
    for pair in suffix[..6].chunks_exact(2) {
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
                    "Dredge did not mill three distinct cards in order: {pair:?}"
                ));
            }
        }
    }
    match &suffix[6..] {
        [
            GameEvent::CardMoved {
                card,
                to: Zone::Hand,
            },
            GameEvent::ObjectIncarnationAdvanced { object, .. },
            GameEvent::Dredged {
                player: PlayerId(0),
                card: dredged_card,
                count: 3,
            },
        ] if *card == darkblast && *object == darkblast && *dredged_card == darkblast => {}
        other => {
            return Err(format!(
                "Dredge did not return the same Darkblast object after exactly three mills: {other:?}"
            ));
        }
    }
    if mills.len() != 3
        || mills.contains(&darkblast)
        || mills.iter().copied().collect::<BTreeSet<_>>().len() != 3
    {
        return Err(format!("Dredge mill count/uniqueness differs: {mills:?}"));
    }
    Ok(mills)
}

fn event_index(
    events: &[GameEvent],
    mut predicate: impl FnMut(&GameEvent) -> bool,
) -> Option<usize> {
    events.iter().position(|event| predicate(event))
}
fn event_index_after(
    events: &[GameEvent],
    after: usize,
    mut predicate: impl FnMut(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(after + 1)
        .find_map(|(index, event)| predicate(event).then_some(index))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Darkblast candidate source: {error}"))?;
    validate_candidate_source_text(&source)
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let file =
        syn::parse_file(source).map_err(|_| "Darkblast source is not valid Rust".to_owned())?;
    if normalize_rust_source(source)? != normalize_rust_source(EXPECTED_CANDIDATE_SOURCE)? {
        return Err("Darkblast candidate differs from its canonical normalized source".into());
    }
    if !candidate_ast_has_literal_shape(&file) {
        return Err("Darkblast candidate AST is outside the direct-literal shape".into());
    }
    Ok(())
}

fn candidate_ast_has_literal_shape(file: &syn::File) -> bool {
    if file.shebang.is_some()
        || file.items.len() != 3
        || !file
            .attrs
            .iter()
            .all(|attribute| attribute.path().is_ident("doc"))
        || !matches!(file.items.first(), Some(Item::Use(_)))
        || !matches!(file.items.get(1), Some(Item::Use(_)))
    {
        return false;
    }
    for item in &file.items[..2] {
        let Item::Use(import) = item else {
            return false;
        };
        if !matches!(import.vis, Visibility::Inherited) || !import.attrs.is_empty() {
            return false;
        }
    }
    let Some(Item::Fn(function)) = file.items.get(2) else {
        return false;
    };
    if function.sig.ident != "definition"
        || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
        || function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
        || !function.sig.inputs.is_empty()
        || !function.sig.generics.params.is_empty()
        || function.sig.generics.where_clause.is_some()
        || function.sig.variadic.is_some()
        || !return_type_is_card_definition(&function.sig.output)
        || !function.attrs.iter().all(|attribute| {
            attribute.path().is_ident("must_use") || attribute.path().is_ident("doc")
        })
    {
        return false;
    }
    let [Stmt::Expr(Expr::Struct(card), None)] = function.block.stmts.as_slice() else {
        return false;
    };
    struct_has_exact_fields(
        card,
        "CardDefinition",
        &[
            "id",
            "name",
            "set_code",
            "mana_cost",
            "colors",
            "mana_colors",
            "card_types",
            "is_basic_land",
            "supported_rules",
            "power",
            "toughness",
            "keywords",
            "effects",
        ],
    ) && card
        .fields
        .iter()
        .all(|field| literal_expression(&field.expr))
}

fn return_type_is_card_definition(output: &ReturnType) -> bool {
    matches!(output, ReturnType::Type(_, ty) if matches!(ty.as_ref(), syn::Type::Path(path) if path.qself.is_none() && path.path.is_ident("CardDefinition")))
}

fn struct_has_exact_fields(
    expression: &syn::ExprStruct,
    type_name: &str,
    expected: &[&str],
) -> bool {
    expression.path.is_ident(type_name)
        && expression.rest.is_none()
        && expression
            .fields
            .iter()
            .map(|field| match &field.member {
                syn::Member::Named(name) => Some(name.to_string()),
                syn::Member::Unnamed(_) => None,
            })
            .collect::<Vec<_>>()
            == expected
                .iter()
                .map(|name| Some((*name).to_owned()))
                .collect::<Vec<_>>()
}

fn literal_expression(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(_) | Expr::Path(_) => true,
        Expr::Unary(unary) => {
            matches!(unary.op, syn::UnOp::Neg(_)) && matches!(&*unary.expr, Expr::Lit(_))
        }
        Expr::Array(array) => array.elems.iter().all(literal_expression),
        Expr::Reference(reference) => {
            reference.mutability.is_none() && literal_expression(&reference.expr)
        }
        Expr::Struct(structure) => {
            structure.attrs.is_empty()
                && structure.rest.is_none()
                && structure
                    .fields
                    .iter()
                    .all(|field| literal_expression(&field.expr))
        }
        Expr::Call(call) => {
            let valid_callee = match &*call.func {
                Expr::Path(path) if path.qself.is_none() => {
                    let segments: Vec<_> = path
                        .path
                        .segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect();
                    matches!(segments.as_slice(), [owner, method] if (owner == "BTreeSet" && (method == "from" || method == "new")) || (owner == "ManaCost" && method == "with_colors") || (owner == "Keyword" && method == "Dredge"))
                }
                _ => false,
            };
            valid_callee && call.args.iter().all(literal_expression)
        }
        Expr::Macro(mac) => {
            let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
            mac.mac.path.is_ident("vec")
                && parser
                    .parse2(mac.mac.tokens.clone())
                    .is_ok_and(|items| items.iter().all(literal_expression))
        }
        _ => false,
    }
}

// The normalizer removes comments/whitespace but preserves every quoted literal byte, so changing
// an identity, cost, support tag, Dredge count, or effect literal changes the authority contract.
fn normalize_rust_source(source: &str) -> Result<String, String> {
    let chars: Vec<char> = source.chars().collect();
    let mut normalized = String::with_capacity(source.len());
    let mut index = 0;
    let mut block_comment_depth = 0_usize;
    let mut in_line_comment = false;
    let mut in_string = false;
    let mut in_char = false;
    while index < chars.len() {
        let current = chars[index];
        if in_line_comment {
            if current == '\n' {
                in_line_comment = false;
            }
            index += 1;
            continue;
        }
        if block_comment_depth > 0 {
            if current == '/' && chars.get(index + 1) == Some(&'*') {
                block_comment_depth += 1;
                index += 2;
            } else if current == '*' && chars.get(index + 1) == Some(&'/') {
                block_comment_depth -= 1;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if in_string || in_char {
            normalized.push(current);
            if current == '\\' {
                if let Some(escaped) = chars.get(index + 1) {
                    normalized.push(*escaped);
                    index += 2;
                    continue;
                }
            } else if (in_string && current == '"') || (in_char && current == '\'') {
                in_string = false;
                in_char = false;
            }
            index += 1;
            continue;
        }
        if current == '/' && chars.get(index + 1) == Some(&'/') {
            in_line_comment = true;
            index += 2;
            continue;
        }
        if current == '/' && chars.get(index + 1) == Some(&'*') {
            block_comment_depth = 1;
            index += 2;
            continue;
        }
        if current == '"' {
            in_string = true;
            normalized.push(current);
            index += 1;
            continue;
        }
        if current == '\'' && has_closing_char_quote(&chars, index + 1) {
            in_char = true;
            normalized.push(current);
            index += 1;
            continue;
        }
        if !current.is_whitespace() {
            normalized.push(current);
        }
        index += 1;
    }
    if in_string || in_char || block_comment_depth > 0 {
        return Err("Darkblast candidate has an unterminated literal or comment".into());
    }
    Ok(normalized)
}

fn has_closing_char_quote(chars: &[char], mut index: usize) -> bool {
    while let Some(character) = chars.get(index) {
        if *character == '\n' {
            return false;
        }
        if *character == '\\' {
            index += 2;
            continue;
        }
        if *character == '\'' {
            return true;
        }
        index += 1;
    }
    false
}

fn malicious_candidate_source_rejected() -> Result<bool, String> {
    let source = std::fs::read_to_string(SOURCE_PATH).map_err(|error| error.to_string())?;
    let signature = "pub(crate) fn definition() -> CardDefinition {";
    if source.matches(signature).count() != 1 {
        return Err(
            "could not locate exactly one Darkblast definition for side-effect injection".into(),
        );
    }
    let malicious = source.replacen(
        signature,
        &format!("{signature}\n    let _ = std::fs::write(\"/tmp/cardbench-darkblast-candidate-executed\", b\"executed\");"),
        1,
    );
    if syn::parse_file(&malicious).is_err() {
        return Err("injected Darkblast side-effect fixture is not valid Rust".into());
    }
    Ok(validate_candidate_source_text(&malicious).is_err())
}

#[allow(dead_code, unreachable_code)]
fn compile_valid_injected_side_effect_candidate_fixture() -> cardbench_magic_engine::CardDefinition
{
    let _ = std::fs::write("/tmp/cardbench-darkblast-candidate-executed", b"executed");
    candidate_darkblast::definition()
}

fn validate_candidate_definition() -> Result<(), String> {
    let card = candidate_darkblast::definition();
    if card.id != CARD_ID
        || card.name != CARD_NAME
        || card.set_code != "RAV"
        || card.mana_cost != ManaCost::with_colors(0, [Color::Black])
        || card.colors != BTreeSet::from([Color::Black])
        || !card.mana_colors.is_empty()
        || card.card_types != BTreeSet::from([CardType::Instant])
        || card.is_basic_land
        || card.power.is_some()
        || card.toughness.is_some()
        || card.keywords != [Keyword::Dredge(3)]
        || card.effects
            != [Effect::ModifyTargetPtUntilEndOfTurn {
                power: -1,
                toughness: -1,
            }]
        || card.supported_rules != ["full-rules-fidelity", "targeted-layer-7-modifier", "dredge"]
    {
        return Err(
            "Darkblast candidate definition differs from RAV #82 printed identity and mechanics"
                .into(),
        );
    }
    Ok(())
}

fn validate_registry() -> Result<(), String> {
    let candidates: Vec<_> = card_definitions()
        .into_iter()
        .filter(|definition| definition.id == CARD_ID)
        .collect();
    let expected = candidate_darkblast::definition();
    if candidates.len() != 1
        || candidates[0].name != expected.name
        || candidates[0].mana_cost != expected.mana_cost
        || candidates[0].colors != expected.colors
        || candidates[0].card_types != expected.card_types
        || candidates[0].keywords != expected.keywords
        || candidates[0].effects != expected.effects
    {
        return Err("RAV registry does not contain exactly one Darkblast definition matching the candidate module".into());
    }
    Ok(())
}
