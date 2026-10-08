//! Full-game RAV Telling Time authority for its private library partition.

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DecisionKind, DecisionSelection,
    DecisionVisibility, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_basic_land_type_bindings,
    rav_main_set_catalog, rav_mana_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-TELLING-TIME";
const CARD_NAME: &str = "Telling Time";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const ISLAND_ID: &str = "RAV-ISLAND";
const FOREST_ID: &str = "RAV-FOREST";
const PLAINS_ID: &str = "RAV-PLAINS";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/telling_time.rs";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/telling_time.rs"]
mod candidate_telling_time;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Telling Time PvP result: {error}");
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
            eprintln!("Telling Time PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_registry()?;
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<CatalogCard> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || catalog.len() != 291
        || printings.len() != 306
    {
        return Err("RAV catalog denominator differs from the source contract".to_owned());
    }
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.winner != replay.winner
        || captured.partition != replay.partition
        || captured.library_after_partition != replay.library_after_partition
        || captured.opponent_saw_decision != replay.opponent_saw_decision
    {
        return Err("Telling Time deterministic full-game replay differed".to_owned());
    }
    let pristine = candidate_value(Mutation::None);
    let mutations = json!({
        "wrong_exact_mana_cost_detected": mutation_detected(seed, Mutation::WrongCost),
        "wrong_card_color_detected": mutation_detected(seed, Mutation::WrongColor),
        "fixed_one_card_draw_detected": mutation_detected(seed, Mutation::DrawOne),
        "pristine_definition_accepted": definition_matches_contract(&pristine),
        "sealed_mutation_authority": false
    });
    for key in [
        "wrong_exact_mana_cost_detected",
        "wrong_card_color_detected",
        "fixed_one_card_draw_detected",
        "pristine_definition_accepted",
    ] {
        if mutations[key] != true {
            return Err(format!("Telling Time semantic mutation gate failed: {key}"));
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
        "card":{"collector_number":69,"printings":1,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}", ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":[
                "105.2 and 202.1-202.2: blue spell identity and colored payment",
                "117: priority and instant-speed casting windows",
                "401.1-401.2: library order and top of library",
                "601.2: casting and payment",
                "608.2: resolving a spell and applying its instructions in order",
                "701.20: searching/looking at cards without revealing them",
                "701: private library partition and zone changes",
                "510: combat damage and terminal game state"
            ],
            "source_contract":"RAV Telling Time #69, {1}{U} blue instant: look at the top three cards of your library; put one into your hand, one on top, and one on the bottom.",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true
        },
        "passed":cards_passed == 1,
        "pilot_card_passed":cards_passed == 1,
        "replay_deterministic":true,
        "cards_passed":cards_passed,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":printings.len(),
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":if candidate_kind() == "candidate" { "candidate_pilot_pass; independent reviewer approval remains" } else { "reference_pilot_pass; independent review and candidate trial remain" },
        "mutation_sensitivity":mutations,
        "traces":[trace],
        "per_card_breakdown":breakdown
    }))
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongCost,
    WrongColor,
    DrawOne,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PartitionCapture {
    candidates: Vec<ObjectId>,
    library_before: Vec<ObjectId>,
    hand: ObjectId,
    top: Option<ObjectId>,
    bottom: Vec<ObjectId>,
    visibility: DecisionVisibility,
    min_selections: u8,
    max_selections: u8,
    opponent_saw_decision: bool,
    opponent_saw_candidate: bool,
    library_after: Vec<ObjectId>,
    zones_after: Vec<Option<Zone>>,
}

#[derive(Clone, Debug)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    telling_time: Option<ObjectId>,
    telling_time_definition: Option<CardDefinition>,
    telling_time_zone: Option<Zone>,
    telling_time_payment: Vec<Color>,
    watchwolf: Option<ObjectId>,
    life: [i64; 2],
    partition: Option<PartitionCapture>,
    library_after_partition: Vec<ObjectId>,
    opponent_saw_decision: bool,
}

fn candidate_value(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_telling_time::definition();
    match mutation {
        Mutation::None => {}
        Mutation::WrongCost => definition.mana_cost = ManaCost::with_colors(2, [Color::Blue]),
        Mutation::WrongColor => definition.colors = BTreeSet::from([Color::White]),
        Mutation::DrawOne => definition.effects = vec![Effect::DrawController],
    }
    definition
}

fn definition_matches_contract(definition: &CardDefinition) -> bool {
    definition.id == CARD_ID
        && definition.name == CARD_NAME
        && definition.set_code == "RAV"
        && definition.mana_cost == ManaCost::with_colors(1, [Color::Blue])
        && definition.colors == BTreeSet::from([Color::Blue])
        && definition.mana_colors.is_empty()
        && definition.card_types == BTreeSet::from([CardType::Instant])
        && !definition.is_basic_land
        && definition.supported_rules
            == [
                "full-rules-fidelity",
                "private-top-library-hand-top-bottom-partition",
            ]
        && definition.power.is_none()
        && definition.toughness.is_none()
        && definition.keywords.is_empty()
        && definition.effects
            == [Effect::LookAtTopCardsPutOneInHandOneOnTopRestOnBottom { count: 3 }]
}

fn validate_candidate_registry() -> Result<(), String> {
    let definition = candidate_telling_time::definition();
    if !definition_matches_contract(&definition) {
        return Err("candidate Telling Time definition differs from its RAV identity".to_owned());
    }
    let registered = card_definitions()
        .into_iter()
        .find(|item| item.id == CARD_ID)
        .ok_or("Telling Time is missing from the RAV definition registry")?;
    if registered != definition {
        return Err(
            "Telling Time candidate module and RAV registry differ from the exact RAV identity"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|e| format!("could not read Telling Time source: {e}"))?;
    let file =
        syn::parse_file(&source).map_err(|_| "Telling Time source is not valid Rust".to_owned())?;
    if !file.attrs.iter().all(|attr| attr.path().is_ident("doc")) || file.items.len() != 3 {
        return Err(
            "Telling Time source must contain two private imports and one direct-literal function"
                .to_owned(),
        );
    }
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(use_item)
                if use_item.attrs.is_empty() && matches!(&use_item.vis, Visibility::Inherited) => {}
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !functions.insert(name.clone())
                    || name != "definition"
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !function.sig.inputs.is_empty()
                    || function.sig.constness.is_some()
                    || function.sig.asyncness.is_some()
                    || function.block.stmts.len() != 1
                    || !matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(_), None))
                {
                    return Err(
                        "Telling Time module contains unsupported executable source".to_owned()
                    );
                }
            }
            _ => return Err(
                "Telling Time source permits only private imports and its declarative definition"
                    .to_owned(),
            ),
        }
    }
    if functions != BTreeSet::from(["definition".to_owned()]) {
        return Err("Telling Time candidate definition function is missing".to_owned());
    }
    Ok(())
}

fn deck(entries: &[(&str, u8)]) -> DeckList {
    DeckList {
        mainboard: entries
            .iter()
            .map(|(card, count)| DeckEntry {
                card: (*card).to_owned(),
                count: *count,
            })
            .collect(),
        sideboard: vec![],
    }
}

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Telling Time definition missing during game construction")? =
        candidate_value(mutation);
    Game::new_with_mana_abilities_and_basic_land_types(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
    )
    .map_err(|e| e.to_string())
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_TELLING_TIME_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_TELLING_TIME_SEED must be an unsigned integer".to_owned())?;
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    let mut last_error = String::new();
    for seed in 0..=MAX_TRACE_SEED {
        match run_match(seed, Mutation::None)
            .and_then(|capture| validate_trace(&capture, seed).map(|_| capture))
        {
            Ok(captured) => return Ok((seed, captured)),
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "no bounded seed produced a full-game Telling Time trace: {last_error}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    run_match(seed, mutation)
        .and_then(|capture| validate_trace(&capture, seed).map(|_| ()))
        .is_err()
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut game = new_game(mutation)?;
    let p0 = deck(&[
        (CARD_ID, 4),
        (WATCHWOLF_ID, 12),
        (ISLAND_ID, 28),
        (FOREST_ID, 8),
        (PLAINS_ID, 8),
    ]);
    let p1 = deck(&[(ISLAND_ID, 60)]);
    game.set_shuffle_seed(seed).map_err(|e| e.to_string())?;
    for (player, list) in [(PlayerId(0), &p0), (PlayerId(1), &p1)] {
        if list
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
        {
            return Err("Telling Time PvP decks must each contain exactly 60 cards".to_owned());
        }
        game.load_deck_into_library(player, list)
            .map_err(|e| e.to_string())?;
        game.draw_opening_hand(player, 7)
            .map_err(|e| e.to_string())?;
    }
    game.begin_game().map_err(|e| e.to_string())?;
    let mut policies: [TellingPolicy; 2] = [
        TellingPolicy::new(PlayerId(0)),
        TellingPolicy::new(PlayerId(1)),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut partition = None;
    let mut library_after_partition = Vec::new();
    let mut opponent_saw_decision = false;
    while !game.is_game_over() {
        if game.turn > 48 || accepted >= 7_500 {
            return Err("Telling Time full-game trace exceeded its bounded match".to_owned());
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Telling Time policy seat missing")?;
        let action = if let Some(action) = policy.propose_pending_decision(&view) {
            action
        } else if view.draw_replacement_pending {
            policy.propose_draw_replacement(&view)
        } else if view.private_library_choice.is_some() {
            policy.propose_private_library_choice(&view)
        } else if view.private_opponent_library_choice.is_some() {
            policy.propose_private_opponent_library_choice(&view)
        } else if view.library_search_choice.is_some() {
            policy.propose_library_search_choice(&view)
        } else {
            policy.propose_move(&view)
        };
        let pending_partition = match (&action, view.pending_decision.as_ref()) {
            (
                PolicyAction::SubmitDecision {
                    decision,
                    selection: DecisionSelection::LibraryTopPartition { hand, top, bottom },
                },
                Some(pending),
            ) if pending.id == *decision && pending.kind == DecisionKind::LibraryTopPartition => {
                let ids = pending
                    .candidates
                    .iter()
                    .map(|candidate| candidate.id)
                    .collect::<Vec<_>>();
                let controller_library =
                    &game.player(PlayerId(0)).map_err(|e| e.to_string())?.library;
                let library_before = controller_library
                    .iter()
                    .rev()
                    .take(3)
                    .copied()
                    .collect::<Vec<_>>();
                let opponent = game
                    .view_for_player(PlayerId(1))
                    .map_err(|e| e.to_string())?;
                let visible = visible_object_ids(&opponent);
                Some(PartitionCapture {
                    candidates: ids.clone(),
                    library_before,
                    hand: *hand,
                    top: *top,
                    bottom: bottom.clone(),
                    visibility: pending.visibility.clone(),
                    min_selections: pending.min_selections,
                    max_selections: pending.max_selections,
                    opponent_saw_decision: opponent.pending_decision.is_some(),
                    opponent_saw_candidate: ids.iter().any(|id| visible.contains(id)),
                    library_after: Vec::new(),
                    zones_after: Vec::new(),
                })
            }
            _ => None,
        };
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|e| {
                format!(
                    "Telling Time policy move rejected on turn {} {:?}: {e}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        if let Some(mut capture) = pending_partition {
            let state = game.player(PlayerId(0)).map_err(|e| e.to_string())?;
            capture.library_after = state.library.clone();
            capture.zones_after = std::iter::once(capture.hand)
                .chain(capture.top)
                .chain(capture.bottom.iter().copied())
                .map(|id| game.zone_of(id))
                .collect();
            opponent_saw_decision |= capture.opponent_saw_decision;
            if capture.opponent_saw_candidate {
                return Err(
                    "opponent policy view exposed a hidden Telling Time library candidate"
                        .to_owned(),
                );
            }
            library_after_partition = capture.library_after.clone();
            partition = Some(capture);
        }
        game.validate_invariants()
            .map_err(|e| format!("post-move invariant failure: {e}"))?;
    }
    let events = game.event_log.clone();
    let find_cast = |seat: PlayerId, definition: &str| {
        events.iter().find_map(|event| match event {
            GameEvent::SpellCast { player, card }
                if *player == seat
                    && game
                        .card_definition(*card)
                        .is_ok_and(|known| known.id == definition) =>
            {
                Some(*card)
            }
            _ => None,
        })
    };
    let telling_time = find_cast(PlayerId(0), CARD_ID);
    let telling_time_definition =
        telling_time.and_then(|card| game.card_definition(card).ok().cloned());
    let watchwolf = find_cast(PlayerId(0), WATCHWOLF_ID);
    let telling_time_payment = telling_time
        .and_then(|card| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card: paid,
                    colors,
                } if *paid == card => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    let end_of_game = events
        .iter()
        .position(|event| matches!(event, GameEvent::GameEnded { .. }))
        .unwrap_or(events.len());
    let telling_time_zone = telling_time.and_then(|card| {
        events[..end_of_game]
            .iter()
            .rev()
            .find_map(|event| match event {
                GameEvent::CardMoved {
                    card: moved, to, ..
                } if *moved == card => Some(*to),
                _ => None,
            })
    });
    Ok(Captured {
        events,
        winner: game.winner().map(|p| p.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        telling_time,
        telling_time_definition,
        telling_time_zone,
        telling_time_payment,
        watchwolf,
        life: [
            game.player(PlayerId(0)).map_err(|e| e.to_string())?.life,
            game.player(PlayerId(1)).map_err(|e| e.to_string())?.life,
        ],
        partition,
        library_after_partition,
        opponent_saw_decision,
    })
}

fn visible_object_ids(view: &GameView) -> BTreeSet<ObjectId> {
    view.hand
        .iter()
        .chain(&view.command_zone)
        .chain(&view.own_battlefield)
        .chain(&view.opponent_battlefield)
        .chain(&view.teammate_battlefield)
        .chain(&view.combat_attackers)
        .chain(&view.stack_spells)
        .map(|card| card.id)
        .chain(
            view.stack_activated_abilities
                .iter()
                .map(|ability| ability.source.id),
        )
        .chain(view.revealed_library_tops.iter().map(|top| top.card.id))
        .chain(
            view.pending_decision
                .iter()
                .flat_map(|decision| decision.candidates.iter().map(|candidate| candidate.id)),
        )
        .collect()
}

struct TellingPolicy {
    player: PlayerId,
    telling_time_offered: bool,
    watchwolf_offered: bool,
}
impl TellingPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            telling_time_offered: false,
            watchwolf_offered: false,
        }
    }
}

impl CodePolicy for TellingPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.telling-time-partition-and-win.v1"
        } else {
            "rav.island-control-opponent.v1"
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
            let attackers = if self.player == PlayerId(0) {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(WATCHWOLF_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect()
            } else {
                vec![]
            };
            return PolicyAction::DeclareAttackers { attackers };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.priority != self.player
            || view.active_player != self.player
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
        if self.player == PlayerId(0) && !self.telling_time_offered && view.turn >= 3 {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            {
                let cost = ManaCost::with_colors(1, [Color::Blue]);
                if let Some(mana_selection) = payment(view, &cost) {
                    self.telling_time_offered = true;
                    return PolicyAction::CastWithPayment {
                        request: CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection,
                    };
                }
                if let Some(action) = make_mana(view, &cost) {
                    return action;
                }
            }
        }
        if self.player == PlayerId(0) && !self.watchwolf_offered && view.turn >= 4 {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(WATCHWOLF_ID))
            {
                let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
                if let Some(mana_selection) = payment(view, &cost) {
                    self.watchwolf_offered = true;
                    return PolicyAction::CastWithPayment {
                        request: CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection,
                    };
                }
                if let Some(action) = make_mana(view, &cost) {
                    return action;
                }
            }
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
        PolicyAction::PassPriority
    }

    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        let decision = view.pending_decision.as_ref()?;
        if decision.kind != DecisionKind::LibraryTopPartition || self.player != PlayerId(0) {
            return None;
        }
        let candidates = decision
            .candidates
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>();
        if candidates.len() < 1 {
            return None;
        }
        let hand = candidates[0];
        let top = candidates.get(1).copied();
        let bottom = candidates.iter().skip(2).rev().copied().collect();
        Some(PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::LibraryTopPartition { hand, top, bottom },
        })
    }
}

fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<_> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let (_, amount) = available
            .iter_mut()
            .find(|(candidate, _)| candidate == color)?;
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
        let required = cost
            .colored
            .iter()
            .filter(|candidate| *candidate == color)
            .count() as u8;
        if view.mana_pool.amount(*color) < required {
            need = Some(*color);
            break;
        }
    }
    if need.is_none()
        && view.mana_pool.total() < cost.generic.saturating_add(cost.colored.len() as u8)
    {
        need = Some(Color::Blue);
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

fn validate_trace(c: &Captured, seed: u64) -> Result<Value, String> {
    if c.winner != Some(0) || c.rejected != 0 || c.life[0] <= 0 || c.life[1] > 0 {
        return Err(format!(
            "Telling Time match did not reach a clean seat-zero win: {:?} {:?}",
            c.winner, c.life
        ));
    }
    let spell = c
        .telling_time
        .ok_or("seat zero did not cast Telling Time")?;
    let definition = c
        .telling_time_definition
        .as_ref()
        .ok_or("Telling Time live definition missing")?;
    if !definition_matches_contract(definition) {
        return Err("live Telling Time definition differs from its exact RAV identity".to_owned());
    }
    let wolf = c.watchwolf.ok_or("seat zero did not cast Watchwolf")?;
    if c.telling_time_zone != Some(Zone::Graveyard) {
        return Err("resolved Telling Time did not move to its owner's graveyard".to_owned());
    }
    if c.telling_time_payment.len() != 2 || !c.telling_time_payment.contains(&Color::Blue) {
        return Err(format!(
            "Telling Time did not pay actual {{1}}{{U}}: {:?}",
            c.telling_time_payment
        ));
    }
    let p = c
        .partition
        .as_ref()
        .ok_or("controller did not submit the private library partition")?;
    let candidate_set: BTreeSet<_> = p.candidates.iter().copied().collect();
    let selected_set: BTreeSet<_> = std::iter::once(p.hand)
        .chain(p.top)
        .chain(p.bottom.iter().copied())
        .collect();
    if p.visibility != DecisionVisibility::Private
        || p.min_selections != 3
        || p.max_selections != 3
        || p.candidates.len() != 3
        || candidate_set.len() != 3
        || p.library_before != p.candidates
        || selected_set != candidate_set
        || p.hand == p.top.unwrap_or(p.hand)
        || p.bottom.len() != 1
        || p.bottom[0] == p.hand
        || Some(p.bottom[0]) == p.top
    {
        return Err("Telling Time did not expose and exhaustively partition the exact private top-three snapshot".to_owned());
    }
    if p.opponent_saw_decision || p.opponent_saw_candidate || c.opponent_saw_decision {
        return Err("opponent view exposed Telling Time private decision or candidate".to_owned());
    }
    if p.zones_after != [Some(Zone::Hand), Some(Zone::Library), Some(Zone::Library)] {
        return Err(format!(
            "private partition zones were incorrect: {:?}",
            p.zones_after
        ));
    }
    // PlayerState stores a library bottom-to-top, while the policy decision
    // presents candidates in top-to-bottom order.
    if p.library_after.first().copied() != Some(p.bottom[0])
        || p.library_after.last().copied() != p.top
    {
        return Err(
            "top and bottom choices were not restored to their selected library positions"
                .to_owned(),
        );
    }
    let events = &c.events;
    let cast = event(
        events,
        |e| matches!(e, GameEvent::SpellCast { player: PlayerId(0), card } if *card == spell),
    )
    .ok_or("Telling Time SpellCast receipt missing")?;
    let resolved = after(
        events,
        cast,
        |e| matches!(e, GameEvent::SpellResolved { card } if *card == spell),
    )
    .ok_or("Telling Time SpellResolved receipt missing")?;
    let partition_resolved = event(events, |e| matches!(e, GameEvent::PrivateLibraryTopPartitionResolved { player: PlayerId(0), source, inspected: 3 } if *source == spell)).filter(|index| *index > cast && *index < resolved).ok_or("private three-card partition resolution receipt missing")?;
    let spell_moved = after(
        events,
        resolved,
        |e| matches!(e, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == spell),
    )
    .ok_or("Telling Time did not reach its owner's graveyard")?;
    let hand_move = after(
        events,
        cast,
        |e| matches!(e, GameEvent::CardMoved { card, to: Zone::Hand } if *card == p.hand),
    )
    .ok_or("chosen private card did not move to hand")?;
    let wolf_cast = event(
        events,
        |e| matches!(e, GameEvent::SpellCast { player: PlayerId(0), card } if *card == wolf),
    )
    .ok_or("Watchwolf SpellCast receipt missing")?;
    let wolf_resolved = after(
        events,
        wolf_cast,
        |e| matches!(e, GameEvent::SpellResolved { card } if *card == wolf),
    )
    .ok_or("Watchwolf did not resolve")?;
    let wolf_entered = after(
        events,
        wolf_resolved,
        |e| matches!(e, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == wolf),
    )
    .ok_or("Watchwolf did not enter battlefield")?;
    let attack = after(events, wolf_entered, |e| matches!(e, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&wolf))).ok_or("Watchwolf did not attack")?;
    let damage = after(events, attack, |e| matches!(e, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 3 } if *source == wolf)).ok_or("Watchwolf did not deal combat damage")?;
    let terminal = after(events, damage, |e| {
        matches!(
            e,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal seat-zero win receipt missing")?;
    let total_damage: i64 = events
        .iter()
        .filter_map(|e| match e {
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if *source == wolf => Some(i64::from(*amount)),
            _ => None,
        })
        .sum();
    if total_damage < 20
        || !(cast < partition_resolved
            && partition_resolved < resolved
            && resolved < spell_moved
            && spell_moved < wolf_cast
            && wolf_cast < wolf_resolved
            && wolf_resolved < wolf_entered
            && wolf_entered < attack
            && attack < damage
            && damage < terminal)
    {
        return Err(format!(
            "Telling Time resolution, private partition, or terminal combat receipts were out of order: positions={cast},{resolved},{partition_resolved},{spell_moved},{wolf_cast},{wolf_resolved},{wolf_entered},{attack},{damage},{terminal}; damage={total_damage}"
        ));
    }
    let mut selected = BTreeSet::from([
        cast,
        resolved,
        partition_resolved,
        spell_moved,
        hand_move,
        wolf_cast,
        wolf_resolved,
        wolf_entered,
        attack,
        damage,
        terminal,
    ]);
    selected.extend(events.iter().enumerate().filter_map(|(index, event)| {
        matches!(
            event,
            GameEvent::DecisionOpened {
                player: PlayerId(0),
                kind: DecisionKind::LibraryTopPartition,
                ..
            } | GameEvent::DecisionCompleted {
                player: PlayerId(0),
                kind: DecisionKind::LibraryTopPartition,
                ..
            }
        )
        .then_some(index)
    }));
    let projected: Vec<ProjectedEvent> = project_events(events)
        .into_iter()
        .filter(|event| selected.contains(&(event.sequence as usize)))
        .collect();
    let card_ids = [p.hand.0, p.top.map_or(0, |id| id.0), p.bottom[0].0];
    Ok(json!({
        "trace_id":"rav-telling-time-private-top-three-partition-full-game",
        "shuffle_seed":seed,
        "decks":["rav_telling_time_watchwolf_60","rav_island_opponent_60"],
        "policies":["rav.telling-time-partition-and-win.v1","rav.island-control-opponent.v1"],
        "terminal":"Winner(PlayerId(0))","winner_seat":0,"turns":c.turns,"accepted_moves":c.accepted,"rejected_moves":c.rejected,
        "telling_time":{"object":spell.0,"collector_number":69,"mana_cost":"{1}{U}","mana_paid":c.telling_time_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),"color":"Blue","card_type":"Instant","zone_after_resolution":"Graveyard"},
        "private_partition":{"decision_visibility":format!("{:?}",p.visibility),"minimum_selections":p.min_selections,"maximum_selections":p.max_selections,"controller_candidate_count":p.candidates.len(),"candidate_objects":p.candidates.iter().map(|id|id.0).collect::<Vec<_>>(),"library_top_to_bottom_before":p.library_before.iter().map(|id|id.0).collect::<Vec<_>>(),"opponent_saw_decision":p.opponent_saw_decision,"opponent_saw_any_candidate":p.opponent_saw_candidate,"hand_object":p.hand.0,"top_object":p.top.map(|id|id.0),"bottom_objects":p.bottom.iter().map(|id|id.0).collect::<Vec<_>>(),"zones_after":p.zones_after.iter().map(|zone|format!("{zone:?}")).collect::<Vec<_>>(),"library_top_after":p.library_after.last().map(|id|id.0),"library_bottom_after":p.library_after.first().map(|id|id.0),"distinct_partition_objects":BTreeSet::from(card_ids).len()==3},
        "combat":{"watchwolf":wolf.0,"power":3,"total_damage_to_opponent":total_damage,"terminal_life":c.life},
        "events":projected,"passed":true
    }))
}

fn event(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn after(
    events: &[GameEvent],
    index: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(index + 1)
        .find_map(|(i, event)| predicate(event).then_some(i))
}

fn card_status(card: &CatalogCard) -> Value {
    let id = card_definitions()
        .into_iter()
        .find(|definition| definition.name == card.name)
        .map(|definition| definition.id)
        .unwrap_or("");
    let engine_status = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice",
        CardSemanticStatus::CatalogOnly { .. } => "catalog_only",
    };
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,"engine_status":engine_status,"trace_status":if id==CARD_ID{"passed"}else{"not_qualified"},"qualification":if id==CARD_ID{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
}
