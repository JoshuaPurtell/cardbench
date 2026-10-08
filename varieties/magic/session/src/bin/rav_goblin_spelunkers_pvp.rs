//! Full-game RAV Goblin Spelunkers Mountainwalk trace authority.

use cardbench_magic_engine::{
    BasicLandType, CardDefinition, CardType, Color, CombatBlock, DeckEntry, DeckList, Game,
    GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, PlayerId, PolicyAction, Step,
    Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_basic_land_type_bindings,
    rav_main_set_catalog,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Lit, Meta, PathArguments, Stmt, Type, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/goblin_spelunkers.rs";
const DECK_P0: &str = "rav_goblin_spelunkers_mountainwalk_attack";
const DECK_P1: &str = "rav_goblin_spelunkers_mountainwalk_defense";
const MAX_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/goblin_spelunkers.rs"]
mod candidate_goblin_spelunkers;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Goblin Spelunkers result: {error}");
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
            eprintln!("Goblin Spelunkers PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_definition()?;
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<CatalogCard> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
    {
        return Err(format!(
            "RAV catalog has {} names / {} printings; expected {} / {}",
            catalog.len(),
            printings.len(),
            RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
            RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        ));
    }
    let (seed, game) = find_trace()?;
    let trace = validate_trace(&game, seed)?;
    let replay = run_match(candidate_goblin_spelunkers::definition(), seed, false)?;
    if game.events != replay.events
        || game.terminal != replay.terminal
        || game.winner != replay.winner
        || game.attacker != replay.attacker
        || game.blocker != replay.blocker
        || game.block_attempt_rejected != replay.block_attempt_rejected
        || game.mountain_controlled_at_attempt != replay.mountain_controlled_at_attempt
    {
        return Err("Goblin Spelunkers Mountainwalk PvP replay was not deterministic".to_owned());
    }
    let mutation_detected = mutation_reaches_trace_and_fails(seed)?;
    if !mutation_detected {
        return Err("removing Goblin Spelunkers Mountainwalk escaped the PvP trace".to_owned());
    }
    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 128, "definition_id": "RAV-GOBLIN-SPELUNKERS", "name": "Goblin Spelunkers"},
        "candidate": {"kind": candidate_kind(), "card_definition": "RAV-GOBLIN-SPELUNKERS"},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "105.2 and 202.1-202.2: red identity and colored payment",
                "106.1 and 106.4: mana pool and spell payment",
                "117.1b and 117.3c: priority and blocker declaration",
                "302.1-302.2 and 508: creature entry and combat",
                "509.1: declaring blockers and evasion restrictions",
                "601.2 and 608.3a: spell casting and resolution",
                "702.14: landwalk and Mountainwalk"
            ],
            "source_contract": "RAV printing identity and Mountainwalk interaction",
            "authority_cutoff": "2026-09-23"
        },
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": qualification_status(),
        "mutation_sensitivity": {
            "mountainwalk_removed_mutation_detected": mutation_detected,
            "sealed_mutation_authority": false
        },
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

/// Candidate Rust must contain one declarative definition literal and no executable helper.
fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Goblin Spelunkers source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Goblin Spelunkers source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Goblin Spelunkers module attributes differ from the task contract".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaCost"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut definition = None;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let Some(paths) = flattened_use_paths(item) else {
                    return Err("Goblin Spelunkers source has an unsupported import".to_owned());
                };
                imports.extend(paths);
            }
            Item::Fn(item) if item.sig.ident == "definition" && definition.is_none() => {
                definition = Some(item)
            }
            _ => {
                return Err(
                    "Goblin Spelunkers source contains executable top-level code".to_owned(),
                );
            }
        }
    }
    if imports != expected_imports || file.items.len() != 3 {
        return Err(
            "Goblin Spelunkers imports or item structure differ from the task contract".to_owned(),
        );
    }
    let function =
        definition.ok_or_else(|| "Goblin Spelunkers definition() is missing".to_owned())?;
    validate_definition_function(function)
}

fn validate_definition_function(function: &syn::ItemFn) -> Result<(), String> {
    let attrs_ok = function.attrs.iter().all(|attribute| {
        is_doc_attribute(attribute)
            || attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_))
    });
    let crate_visibility =
        matches!(&function.vis, Visibility::Restricted(value) if value.path.is_ident("crate"));
    if !attrs_ok
        || !crate_visibility
        || function.sig.constness.is_some()
        || function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
        || function.sig.abi.is_some()
        || !function.sig.generics.params.is_empty()
        || function.sig.generics.where_clause.is_some()
        || !function.sig.inputs.is_empty()
        || function.sig.variadic.is_some()
        || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if type_is(ty, &["CardDefinition"]))
        || function.block.stmts.len() != 1
    {
        return Err("Goblin Spelunkers definition() has an unsupported signature".to_owned());
    }
    let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
        return Err("Goblin Spelunkers definition() must return one direct literal".to_owned());
    };
    if !path_is(&value.path, &["CardDefinition"]) || value.rest.is_some() || !value.attrs.is_empty()
    {
        return Err("Goblin Spelunkers definition literal type differs".to_owned());
    }
    let fields = literal_fields(value)?;
    if fields.len() != 13 {
        return Err("Goblin Spelunkers definition fields differ from CardDefinition".to_owned());
    }
    check_string(fields["id"], "RAV-GOBLIN-SPELUNKERS")?;
    check_string(fields["name"], "Goblin Spelunkers")?;
    check_string(fields["set_code"], "RAV")?;
    check_cost(fields["mana_cost"])?;
    check_single_set_path(fields["colors"], &["Color", "Red"])?;
    check_empty_set(fields["mana_colors"])?;
    check_single_set_path(fields["card_types"], &["CardType", "Creature"])?;
    check_bool(fields["is_basic_land"], false)?;
    check_rules(fields["supported_rules"])?;
    check_some_integer(fields["power"], 2)?;
    check_some_integer(fields["toughness"], 2)?;
    check_keyword_or_empty(fields["keywords"])?;
    check_empty_vec(fields["effects"])?;
    Ok(())
}

fn validate_candidate_definition() -> Result<(), String> {
    let submitted = candidate_goblin_spelunkers::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == "RAV-GOBLIN-SPELUNKERS")
        .ok_or_else(|| {
            "RAV-GOBLIN-SPELUNKERS is missing from the executable registry".to_owned()
        })?;
    if submitted.id != registered.id
        || submitted.name != "Goblin Spelunkers"
        || submitted.set_code != "RAV"
        || submitted.mana_cost != ManaCost::with_colors(2, [Color::Red])
        || submitted.colors != BTreeSet::from([Color::Red])
        || !submitted.mana_colors.is_empty()
        || submitted.card_types != BTreeSet::from([CardType::Creature])
        || submitted.is_basic_land
        || submitted.power != Some(2)
        || submitted.toughness != Some(2)
        || !submitted.effects.is_empty()
        || submitted.keywords != [Keyword::Mountainwalk] && !submitted.keywords.is_empty()
    {
        return Err("RAV-GOBLIN-SPELUNKERS identity or printed characteristics differ".to_owned());
    }
    Ok(())
}

fn new_game(definition: CardDefinition) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let target = definitions
        .iter_mut()
        .find(|card| card.id == "RAV-GOBLIN-SPELUNKERS")
        .ok_or_else(|| {
            "RAV-GOBLIN-SPELUNKERS is missing from the executable registry".to_owned()
        })?;
    *target = definition;
    Game::new_with_basic_land_types(definitions, 2, rav_basic_land_type_bindings())
        .map_err(|error| error.to_string())
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_GOBLIN_SPELUNKERS_SEED") {
        let seed = value.parse::<u64>().map_err(|_| {
            "CARDBENCH_GOBLIN_SPELUNKERS_SEED must be an unsigned integer".to_owned()
        })?;
        return Ok((
            seed,
            run_match(candidate_goblin_spelunkers::definition(), seed, false)?,
        ));
    }
    for seed in 0..=MAX_SEED {
        if let Ok(preview) = run_match(candidate_goblin_spelunkers::definition(), seed, true)
            && preview.block_attempt_seen
            && preview.attacker.is_some()
            && preview.blocker.is_some()
        {
            return Ok((
                seed,
                run_match(candidate_goblin_spelunkers::definition(), seed, false)?,
            ));
        }
    }
    Err(format!(
        "no Mountainwalk block challenge found for seeds 0..={MAX_SEED}"
    ))
}

fn mutation_reaches_trace_and_fails(seed: u64) -> Result<bool, String> {
    let mut mutated = candidate_goblin_spelunkers::definition();
    mutated
        .keywords
        .retain(|keyword| *keyword != Keyword::Mountainwalk);
    let captured = run_match(mutated, seed, false)?;
    Ok(validate_trace(&captured, seed).err().is_some_and(|error| {
        error.contains("Goblin Spelunkers Mountainwalk did not reject a block while defender controlled a Mountain")
    }))
}

#[derive(Clone)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    attacker: Option<cardbench_magic_engine::ObjectId>,
    blocker: Option<cardbench_magic_engine::ObjectId>,
    block_attempt_seen: bool,
    block_attempt_rejected: bool,
    block_rejection_reason: Option<String>,
    mountain_controlled_at_attempt: bool,
    attempt_event_count_before: Option<usize>,
    attempt_event_count_after: Option<usize>,
    live_mountainwalk: bool,
    live_power_toughness: Option<(i32, i32)>,
    blocker_live: bool,
    blocker_is_creature: bool,
    mana_paid: Vec<Color>,
}

fn run_match(
    definition: CardDefinition,
    seed: u64,
    preview: bool,
) -> Result<CapturedMatch, String> {
    let goblin_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: "RAV-GOBLIN-SPELUNKERS".to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    let count: usize = goblin_deck
        .mainboard
        .iter()
        .map(|entry| usize::from(entry.count))
        .sum();
    if count != 60 {
        return Err(format!(
            "Goblin Spelunkers deck has {count} cards, expected 60"
        ));
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    let mut game = new_game(definition.clone())?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &goblin_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &goblin_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let cost = definition.mana_cost;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(SpelunkersPolicy::new(PlayerId(0), cost.clone(), false)),
        Box::new(SpelunkersPolicy::new(PlayerId(1), cost, true)),
    ];
    let mut attempted_moves = 0_u32;
    let mut accepted_moves = 0_u32;
    let mut block_attempt_seen = false;
    let mut block_attempt_rejected = false;
    let mut block_rejection_reason = None;
    let mut mountain_controlled_at_attempt = false;
    let mut attempt_event_count_before = None;
    let mut attempt_event_count_after = None;
    let mut attacker = None;
    let mut blocker = None;
    let mut blocker_live_at_challenge = false;
    let mut blocker_is_creature_at_challenge = false;

    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted_moves >= config.max_policy_moves {
            return Err("Goblin Spelunkers PvP match exceeded its development bound".to_owned());
        }
        if preview && game.turn > 10 {
            break;
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("policy seat missing")?;
        let action = if let Some(action) = policy.propose_pending_decision(&view) {
            action
        } else if view.optional_triggered_ability_choice.is_some() {
            policy.propose_optional_triggered_ability(&view)
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
        let challenge = player == PlayerId(1)
            && matches!(&action, PolicyAction::DeclareBlockers { assignments } if assignments.len() == 1);
        if challenge {
            block_attempt_seen = true;
            attempt_event_count_before = Some(game.event_log.len());
            mountain_controlled_at_attempt = view
                .own_battlefield
                .iter()
                .any(|card| card.basic_land_type == Some(BasicLandType::Mountain));
            if let PolicyAction::DeclareBlockers { assignments } = &action {
                attacker = assignments.first().map(|block| block.attacker);
                blocker = assignments.first().map(|block| block.blocker);
                blocker_live_at_challenge =
                    blocker.is_some_and(|card| game.zone_of(card) == Some(Zone::Battlefield));
                blocker_is_creature_at_challenge = blocker.is_some_and(|card| {
                    blocker_live_at_challenge
                        && game
                            .characteristics(card)
                            .is_ok_and(|c| c.card_types.contains(&CardType::Creature))
                });
            }
        }
        attempted_moves += 1;
        match game.submit_policy_move(player, policy.id(), action) {
            Ok(()) => {
                accepted_moves += 1;
                if challenge {
                    if preview || !block_attempt_rejected {
                        break;
                    }
                }
            }
            Err(error) if challenge => {
                let diagnostic = error.to_string();
                if !diagnostic.contains(
                    "landwalk attacker cannot be blocked while defender controls its land type",
                ) {
                    return Err(format!(
                        "Mountainwalk challenge rejected for an unexpected reason: {diagnostic}"
                    ));
                }
                block_attempt_rejected = true;
                block_rejection_reason = Some(diagnostic);
                attempt_event_count_after = Some(game.event_log.len());
                if attempt_event_count_before != attempt_event_count_after {
                    return Err(
                        "rejected Mountainwalk block attempt changed the event log".to_owned()
                    );
                }
                if preview {
                    break;
                }
            }
            Err(error) => {
                return Err(format!(
                    "policy move rejected on turn {} at {:?}: {error}",
                    view.turn, view.step
                ));
            }
        }
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
    }
    let events = game.event_log.clone();
    let attacker = events
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|d| d.id == "RAV-GOBLIN-SPELUNKERS") =>
            {
                Some(*card)
            }
            _ => None,
        })
        .or(attacker);
    let live_mountainwalk = attacker.is_some_and(|card| {
        game.characteristics(card)
            .is_ok_and(|c| c.keywords.contains(&Keyword::Mountainwalk))
    });
    let live_power_toughness = attacker.and_then(|card| {
        let c = game.characteristics(card).ok()?;
        Some((c.power?, c.toughness?))
    });
    let mana_paid = attacker
        .and_then(|attacker| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if *card == attacker => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    let winner = game.winner().map(|player| player.0);
    Ok(CapturedMatch {
        events,
        terminal: if preview && !game.is_game_over() {
            "InProgress".to_owned()
        } else {
            winner.map_or_else(
                || "Draw".to_owned(),
                |seat| format!("Winner(PlayerId({seat}))"),
            )
        },
        winner,
        turns: game.turn,
        accepted_moves,
        rejected_moves: attempted_moves.saturating_sub(accepted_moves),
        attacker,
        blocker,
        block_attempt_seen,
        block_attempt_rejected,
        block_rejection_reason,
        mountain_controlled_at_attempt,
        attempt_event_count_before,
        attempt_event_count_after,
        live_mountainwalk,
        live_power_toughness,
        blocker_live: blocker_live_at_challenge,
        blocker_is_creature: blocker_is_creature_at_challenge,
        mana_paid,
    })
}

struct SpelunkersPolicy {
    player: PlayerId,
    cost: ManaCost,
    defender: bool,
    cast_once: bool,
    challenge_once: bool,
}

impl SpelunkersPolicy {
    fn new(player: PlayerId, cost: ManaCost, defender: bool) -> Self {
        Self {
            player,
            cost,
            defender,
            cast_once: false,
            challenge_once: false,
        }
    }

    fn payment(&self, view: &GameView) -> Option<ManaPaymentSelection> {
        let mut available: Vec<(Color, u8)> = Color::MANA_ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        for color in &self.cost.colored {
            let (_, amount) = available
                .iter_mut()
                .find(|(available_color, _)| available_color == color)?;
            if *amount == 0 {
                return None;
            }
            *amount -= 1;
        }
        let mut generic = Vec::with_capacity(usize::from(self.cost.generic));
        for _ in 0..self.cost.generic {
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

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        view.hand
            .iter()
            .find(|card| card.card_types.contains(&CardType::Land))
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }

    fn add_mana(&self, view: &GameView) -> Option<PolicyAction> {
        let land = view
            .own_battlefield
            .iter()
            .find(|card| !card.tapped && card.basic_land_type == Some(BasicLandType::Mountain))?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color: Color::Red,
        })
    }
}

impl CodePolicy for SpelunkersPolicy {
    fn id(&self) -> &'static str {
        if self.defender {
            "rav.goblin-spelunkers-mountainwalk-defense.v1"
        } else {
            "rav.goblin-spelunkers-mountainwalk-attack.v1"
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
            if self.defender {
                return PolicyAction::DeclareAttackers { attackers: vec![] };
            }
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| {
                        card.definition == Some("RAV-GOBLIN-SPELUNKERS") && card.can_attack
                    })
                    .map(|card| card.id)
                    .collect(),
            };
        }
        if self.defender
            && view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            if !self.challenge_once {
                let attacker = view
                    .combat_attackers
                    .iter()
                    .find(|card| card.definition == Some("RAV-GOBLIN-SPELUNKERS"));
                let blocker = view.own_battlefield.iter().find(|card| {
                    card.definition == Some("RAV-GOBLIN-SPELUNKERS") && card.can_block
                });
                let has_mountain = view
                    .own_battlefield
                    .iter()
                    .any(|card| card.basic_land_type == Some(BasicLandType::Mountain));
                if let (Some(attacker), Some(blocker), true) = (attacker, blocker, has_mountain) {
                    self.challenge_once = true;
                    return PolicyAction::DeclareBlockers {
                        assignments: vec![CombatBlock {
                            attacker: attacker.id,
                            blocker: blocker.id,
                        }],
                    };
                }
            }
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.active_player != self.player
            || view.priority != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(land) = self.play_land(view) {
            return land;
        }
        if !self.cast_once
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-GOBLIN-SPELUNKERS"))
        {
            if let Some(mana_selection) = self.payment(view) {
                self.cast_once = true;
                return PolicyAction::CastWithPayment {
                    request: cardbench_magic_engine::CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection,
                };
            }
            if let Some(action) = self.add_mana(view) {
                return action;
            }
        }
        self.add_mana(view).unwrap_or(PolicyAction::PassPriority)
    }
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if !captured.block_attempt_seen || captured.attacker.is_none() || captured.blocker.is_none() {
        return Err(
            "Goblin Spelunkers trace did not stage a Mountainwalk block challenge".to_owned(),
        );
    }
    if !captured.mountain_controlled_at_attempt {
        return Err("Goblin Spelunkers block challenge lacked an opposing Mountain".to_owned());
    }
    if !captured.block_attempt_rejected {
        return Err("Goblin Spelunkers Mountainwalk did not reject a block while defender controlled a Mountain".to_owned());
    }
    if captured.attempt_event_count_before != captured.attempt_event_count_after {
        return Err("Goblin Spelunkers rejected block attempt was not event-atomic".to_owned());
    }
    if captured.terminal != "Winner(PlayerId(0))" || captured.winner != Some(0) {
        return Err(
            "Goblin Spelunkers game did not reach a clean seat-zero terminal win".to_owned(),
        );
    }
    if captured.rejected_moves != 1 {
        return Err(format!(
            "Goblin Spelunkers trace has {} rejected moves; one Mountainwalk challenge rejection is required",
            captured.rejected_moves
        ));
    }
    if !captured.live_mountainwalk {
        return Err("resolved Goblin Spelunkers is missing Mountainwalk".to_owned());
    }
    if captured.live_power_toughness != Some((2, 2)) {
        return Err(format!(
            "resolved Goblin Spelunkers is not a 2/2: {:?}",
            captured.live_power_toughness
        ));
    }
    if !captured.blocker_live || !captured.blocker_is_creature {
        return Err(
            "the challenged opposing creature did not remain live on the battlefield".to_owned(),
        );
    }
    let attacker = captured.attacker.expect("validated attacker");
    let blocker = captured.blocker.expect("validated blocker");
    if attacker == blocker {
        return Err("Goblin Spelunkers was challenged by the same object".to_owned());
    }
    let red_spent = captured
        .mana_paid
        .iter()
        .filter(|color| **color == Color::Red)
        .count();
    if red_spent < 1 || captured.mana_paid.len() != 3 {
        return Err(format!(
            "Goblin Spelunkers cast paid {:?}; red plus two generic mana is required",
            captured.mana_paid
        ));
    }
    let projected = project_events(&captured.events);
    let spell_paid = event_index(&captured.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, .. } if *card == attacker)).ok_or("Goblin Spelunkers SpellManaPaid missing")?;
    let spell_cast = event_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == attacker)).ok_or("Goblin Spelunkers SpellCast missing")?;
    let spell_resolved = event_index(
        &captured.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == attacker),
    )
    .ok_or("Goblin Spelunkers SpellResolved missing")?;
    let entered = event_index(&captured.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield, .. } if *card == attacker)).ok_or("Goblin Spelunkers battlefield entry missing")?;
    let attack = event_index(&captured.events, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&attacker))).ok_or("Goblin Spelunkers did not attack")?;
    let blockers = event_index(&captured.events, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if assignments.is_empty())).ok_or("empty legal blocker declaration after Mountainwalk rejection missing")?;
    let damage = event_index(&captured.events, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 2 } if *source == attacker)).ok_or("unblocked Goblin Spelunkers combat damage missing")?;
    let ended = event_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal seat-zero GameEnded missing")?;
    if !(spell_paid < spell_cast
        && spell_cast < spell_resolved
        && spell_resolved < entered
        && entered < attack
        && attack < blockers
        && blockers < damage
        && damage < ended)
    {
        return Err(
            "Goblin Spelunkers cast, combat, damage, and terminal event order differs".to_owned(),
        );
    }
    let relevant = project_relevant(&projected, attacker);
    let expected = [
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "AttackersDeclared",
        "BlockersDeclared",
        "DamageDealtToPlayer",
        "GameEnded",
    ];
    let kinds: Vec<_> = relevant.iter().map(|event| event.kind.as_str()).collect();
    if kinds != expected {
        return Err(format!(
            "Goblin Spelunkers ordered projected events differ: {kinds:?}"
        ));
    }
    Ok(json!({
        "id": "rav-goblin-spelunkers-mountainwalk-block-rejection",
        "seed": seed,
        "decks": [DECK_P0, DECK_P1],
        "policies": ["rav.goblin-spelunkers-mountainwalk-attack.v1", "rav.goblin-spelunkers-mountainwalk-defense.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "digest": event_digest(&captured.events),
        "goblin_spelunkers_object": attacker.0,
        "challenging_creature_object": blocker.0,
        "cast_payment": {
            "required": {"red_symbols": 1, "generic_symbols": 2, "total_symbols": 3},
            "spent": {"red": red_spent, "total": captured.mana_paid.len()}
        },
        "live_mountainwalk": captured.live_mountainwalk,
        "resolved_power_toughness": {"power": captured.live_power_toughness.unwrap().0, "toughness": captured.live_power_toughness.unwrap().1},
        "block_attempt": {
            "defender_controls_mountain": captured.mountain_controlled_at_attempt,
            "rejected": captured.block_attempt_rejected,
            "engine_reason": captured.block_rejection_reason,
            "event_count_before": captured.attempt_event_count_before,
            "event_count_after": captured.attempt_event_count_after,
            "blocker_remains_live": captured.blocker_live
        },
        "ordered_event_sequence": relevant.iter().map(|event| event.sequence).collect::<Vec<_>>(),
        "events": relevant,
        "passed": true
    }))
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}

fn project_relevant(
    events: &[ProjectedEvent],
    attacker: cardbench_magic_engine::ObjectId,
) -> Vec<ProjectedEvent> {
    let mut attack_selected = false;
    let mut blockers_selected = false;
    let mut damage_selected = false;
    events
        .iter()
        .filter(|event| match event.kind.as_str() {
            "SpellManaPaid" | "SpellCast" | "SpellResolved" | "CardMoved" => {
                event
                    .canonical
                    .contains(&format!("card: ObjectId({})", attacker.0))
                    && (event.kind != "CardMoved" || event.canonical.contains("to: Battlefield"))
            }
            "AttackersDeclared" if !attack_selected => {
                let matches = event
                    .canonical
                    .contains(&format!("ObjectId({})", attacker.0));
                attack_selected = matches;
                matches
            }
            "BlockersDeclared" if attack_selected && !blockers_selected => {
                let matches = event.canonical.contains("player: PlayerId(1)")
                    && event.canonical.contains("assignments: []");
                blockers_selected = matches;
                matches
            }
            "DamageDealtToPlayer" if blockers_selected && !damage_selected => {
                let matches = event
                    .canonical
                    .contains(&format!("source: ObjectId({})", attacker.0))
                    && event.canonical.contains("player: PlayerId(1)");
                damage_selected = matches;
                matches
            }
            "GameEnded" => true,
            _ => false,
        })
        .cloned()
        .collect()
}

fn event_digest(events: &[GameEvent]) -> String {
    use std::fmt::Write as _;
    let mut data = String::new();
    for event in events {
        let _ = writeln!(data, "{event:?}");
    }
    let hash = data
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
    format!("fnv1a64:{hash:016x}")
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some("RAV-GOBLIN-SPELUNKERS");
    json!({
        "collector_number": card.collector_number,
        "name": card.name,
        "definition_id": definition_id,
        "engine_status": match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "credited": false,
        "qualification": if passed { "pilot_trace_passed; independent reviewer approval pending" } else { "not_evaluated" }
    })
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}
fn qualification_status() -> &'static str {
    if candidate_kind() == "candidate" {
        "candidate_pilot_pass; independent reviewer approval remains"
    } else {
        "reference_pilot_pass; independent review and candidate trial remain"
    }
}
fn output_root() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--output-root" {
            return args.next().map(std::path::PathBuf::from);
        }
    }
    None
}
fn write_outputs(root: &std::path::Path, report: &Value) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).expect("report serializes"),
    )?;
    std::fs::write(root.join("reward.txt"), "0.0\n")
}

fn flattened_use_paths(item: &syn::ItemUse) -> Option<Vec<Vec<String>>> {
    if item.leading_colon.is_some() {
        return None;
    }
    let mut paths = Vec::new();
    flatten_use_tree(&[], &item.tree, &mut paths).then_some(paths)
}
fn flatten_use_tree(prefix: &[String], tree: &UseTree, paths: &mut Vec<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix.to_vec();
            next.push(path.ident.to_string());
            flatten_use_tree(&next, &path.tree, paths)
        }
        UseTree::Name(name) => {
            let mut path = prefix.to_vec();
            path.push(name.ident.to_string());
            paths.push(path);
            true
        }
        UseTree::Group(group) => group
            .items
            .iter()
            .all(|item| flatten_use_tree(prefix, item, paths)),
        _ => false,
    }
}
fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc") && matches!(attribute.meta, Meta::NameValue(_))
}
fn type_is(ty: &Type, expected: &[&str]) -> bool {
    matches!(ty, Type::Path(path) if path.qself.is_none() && path_is(&path.path, expected))
}
fn path_is(path: &syn::Path, expected: &[&str]) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == expected.len()
        && path
            .segments
            .iter()
            .zip(expected)
            .all(|(segment, expected)| {
                segment.ident == *expected && matches!(&segment.arguments, PathArguments::None)
            })
}
fn literal_fields(
    value: &syn::ExprStruct,
) -> Result<std::collections::BTreeMap<String, &Expr>, String> {
    let mut fields = std::collections::BTreeMap::new();
    for field in &value.fields {
        let syn::Member::Named(name) = &field.member else {
            return Err("Goblin Spelunkers fields must be named".to_owned());
        };
        if field.colon_token.is_none() || fields.insert(name.to_string(), &field.expr).is_some() {
            return Err("Goblin Spelunkers fields must be explicit and unique".to_owned());
        }
    }
    Ok(fields)
}
fn check_string(expr: &Expr, expected: &str) -> Result<(), String> {
    if matches!(expr, Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Str(lit) if lit.value() == expected))
    {
        Ok(())
    } else {
        Err(format!(
            "Goblin Spelunkers string field must be {expected:?}"
        ))
    }
}
fn check_bool(expr: &Expr, expected: bool) -> Result<(), String> {
    if matches!(expr, Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Bool(lit) if lit.value == expected))
    {
        Ok(())
    } else {
        Err("Goblin Spelunkers boolean field differs".to_owned())
    }
}
fn check_path(expr: &Expr, expected: &[&str]) -> bool {
    matches!(expr, Expr::Path(path) if path.attrs.is_empty() && path.qself.is_none() && path_is(&path.path, expected))
}
fn call_expr<'a>(expr: &'a Expr, path: &[&str]) -> Option<Vec<&'a Expr>> {
    match expr {
        Expr::Call(call) if call.attrs.is_empty() && check_path(&call.func, path) => {
            Some(call.args.iter().collect())
        }
        _ => None,
    }
}
fn array_items(expr: &Expr) -> Option<Vec<&Expr>> {
    match expr {
        Expr::Array(array) if array.attrs.is_empty() => Some(array.elems.iter().collect()),
        _ => None,
    }
}
fn check_single_set_path(expr: &Expr, expected: &[&str]) -> Result<(), String> {
    let args = call_expr(expr, &["BTreeSet", "from"])
        .ok_or_else(|| "Goblin Spelunkers set must use BTreeSet::from".to_owned())?;
    let items = if args.len() == 1 {
        array_items(args[0])
    } else {
        None
    };
    if items.is_some_and(|items| items.len() == 1 && check_path(items[0], expected)) {
        Ok(())
    } else {
        Err("Goblin Spelunkers set value differs".to_owned())
    }
}
fn check_empty_set(expr: &Expr) -> Result<(), String> {
    if call_expr(expr, &["BTreeSet", "new"]).is_some_and(|args| args.is_empty()) {
        Ok(())
    } else {
        Err("Goblin Spelunkers mana colors must be empty".to_owned())
    }
}
fn check_cost(expr: &Expr) -> Result<(), String> {
    let args = call_expr(expr, &["ManaCost", "with_colors"])
        .ok_or_else(|| "Goblin Spelunkers cost must use ManaCost::with_colors".to_owned())?;
    if args.len() != 2
        || !matches!(args[0], Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Int(lit) if lit.base10_parse::<u8>().ok() == Some(2)))
    {
        return Err("Goblin Spelunkers generic cost must be two".to_owned());
    }
    let colors = array_items(args[1]);
    if colors.is_some_and(|colors| colors.len() == 1 && check_path(colors[0], &["Color", "Red"])) {
        Ok(())
    } else {
        Err("Goblin Spelunkers colored cost must be red".to_owned())
    }
}
fn check_rules(expr: &Expr) -> Result<(), String> {
    let Expr::Reference(reference) = expr else {
        return Err("Goblin Spelunkers rules must be a static slice".to_owned());
    };
    let items = array_items(&reference.expr)
        .ok_or_else(|| "Goblin Spelunkers rules must be a string array".to_owned())?;
    let expected = [
        "full-rules-fidelity",
        "colored-cost-casting",
        "base-characteristics",
        "mountainwalk",
    ];
    if reference.attrs.is_empty()
        && reference.mutability.is_none()
        && items.len() == expected.len()
        && items
            .iter()
            .zip(expected)
            .all(|(item, value)| check_string(item, value).is_ok())
    {
        Ok(())
    } else {
        Err("Goblin Spelunkers supported rules differ".to_owned())
    }
}
fn check_some_integer(expr: &Expr, expected: i16) -> Result<(), String> {
    let args = call_expr(expr, &["Some"])
        .ok_or_else(|| "Goblin Spelunkers power and toughness must be literals".to_owned())?;
    if args.len() == 1
        && matches!(args[0], Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Int(lit) if lit.base10_parse::<i16>().ok() == Some(expected)))
    {
        Ok(())
    } else {
        Err("Goblin Spelunkers power or toughness differs".to_owned())
    }
}
fn check_keyword_or_empty(expr: &Expr) -> Result<(), String> {
    let Expr::Macro(mac) = expr else {
        return Err("Goblin Spelunkers keyword list must use a literal vec!".to_owned());
    };
    let tokens = mac.mac.tokens.to_string().replace(' ', "");
    if mac.attrs.is_empty()
        && mac.mac.path.is_ident("vec")
        && (tokens.is_empty() || tokens == "Keyword::Mountainwalk")
    {
        Ok(())
    } else {
        Err(
            "Goblin Spelunkers keyword must be Mountainwalk or omitted for semantic rejection"
                .to_owned(),
        )
    }
}
fn check_empty_vec(expr: &Expr) -> Result<(), String> {
    match expr {
        Expr::Macro(mac)
            if mac.attrs.is_empty()
                && mac.mac.path.is_ident("vec")
                && mac.mac.tokens.is_empty() =>
        {
            Ok(())
        }
        _ => Err("Goblin Spelunkers effects must be empty".to_owned()),
    }
}
