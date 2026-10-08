//! Full-game RAV #246 authority for Gaze of the Gorgon's linked combat effects.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CombatBlock, DELAYED_COMBAT_HISTORY_DESTRUCTION_ABILITY_ID,
    DeckEntry, DeckList, Effect, Game, GameEvent, GameView, HybridManaSymbol, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-GAZE-OF-THE-GORGON";
const CARD_NAME: &str = "Gaze of the Gorgon";
const THUG_ID: &str = "RAV-GOLGARI-THUG";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const FOREST_ID: &str = "RAV-FOREST";
const PLAINS_ID: &str = "RAV-PLAINS";
const SWAMP_ID: &str = "RAV-SWAMP";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/gaze_of_the_gorgon.rs";
const MAX_SEED: u64 = 512;
const MAX_TURNS: u32 = 40;
const MAX_MOVES: u32 = 5_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/gaze_of_the_gorgon.rs"]
mod candidate_gaze_of_the_gorgon;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Gaze of the Gorgon result: {error}");
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
            eprintln!("Gaze of the Gorgon PvP authority failed: {error}");
            ExitCode::FAILURE
        }
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

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn run_authority() -> Result<Value, String> {
    // Fail closed before calling any function from the candidate card module.
    validate_candidate_source()?;
    let source_guard = validate_compile_valid_side_effect_candidate()?;
    validate_candidate_registry()?;

    let catalog = unique_catalog()?;
    let (seed, captured) = find_reference()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured != replay {
        return Err("Gaze of the Gorgon shuffled full-game replay differed".to_owned());
    }
    let mutations = json!({
        "combined_effect_removal_detected": mutation_detected(seed, Mutation::RemoveEffect)?,
        "hybrid_cost_mutation_detected": mutation_detected(seed, Mutation::RemoveHybridSymbol)?,
        "compile_valid_side_effect_candidate_rejected_before_candidate_calls": source_guard["compile_valid_injected_side_effect_candidate_rejected_before_candidate_calls"] == true,
        "sealed_mutation_authority": false
    });
    for key in [
        "combined_effect_removal_detected",
        "hybrid_cost_mutation_detected",
        "compile_valid_side_effect_candidate_rejected_before_candidate_calls",
    ] {
        if mutations[key] != true {
            return Err(format!("Gaze of the Gorgon mutation escaped: {key}"));
        }
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
        "card": {"collector_number": 246, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "authority_cutoff": "2026-09-24",
            "source_contract": "RAV #246 identity, {3}{B/G} instant cost, targeted regeneration, and end-of-combat destruction using creatures recorded in that target's block history. Current Oracle's 'this turn's next end of combat' wording is interpretive; the RAV printing controls.",
            "checked_sections": ["117 Priority", "400.7 Object identity", "511 End of Combat Step", "701.15 Regenerate", "704 State-Based Actions"]
        },
        "candidate_source_validation": source_guard,
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "unreviewed_diagnostic",
        "replay_deterministic": true,
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn validate_candidate_registry() -> Result<(), String> {
    let candidate = candidate_gaze_of_the_gorgon::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Gaze of the Gorgon definition is missing from the RAV registry")?;
    let expected_cost = ManaCost::with_hybrid(
        3,
        [],
        [HybridManaSymbol {
            first: Color::Black,
            second: Color::Green,
        }],
    );
    if candidate != registered
        || candidate.id != CARD_ID
        || candidate.name != CARD_NAME
        || candidate.set_code != "RAV"
        || candidate.mana_cost != expected_cost
        || candidate.colors != BTreeSet::from([Color::Black, Color::Green])
        || !candidate.mana_colors.is_empty()
        || candidate.card_types != BTreeSet::from([CardType::Instant])
        || candidate.is_basic_land
        || candidate.power.is_some()
        || candidate.toughness.is_some()
        || !candidate.keywords.is_empty()
        || candidate.effects
            != [Effect::RegenerateTargetCreatureAndScheduleCombatHistoryDestruction]
        || candidate.supported_rules
            != [
                "full-rules-fidelity",
                "targeted-regeneration-shield",
                "delayed-end-of-combat-block-history-destruction",
            ]
    {
        return Err(
            "Gaze of the Gorgon RAV identity, hybrid cost, or combined effect differs".to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Gaze of the Gorgon candidate source: {error}"))?;
    validate_candidate_source_text(&source)
}

const CANONICAL_CANDIDATE_SOURCE: &str = r#"//! Candidate-owned RAV #246 Gaze of the Gorgon definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, HybridManaSymbol, ManaCost};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GAZE-OF-THE-GORGON",
        name: "Gaze of the Gorgon",
        set_code: "RAV",
        mana_cost: ManaCost::with_hybrid(
            3,
            [],
            [HybridManaSymbol {
                first: Color::Black,
                second: Color::Green,
            }],
        ),
        colors: BTreeSet::from([Color::Black, Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "targeted-regeneration-shield",
            "delayed-end-of-combat-block-history-destruction",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::RegenerateTargetCreatureAndScheduleCombatHistoryDestruction],
    }
}"#;

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let file = syn::parse_file(source)
        .map_err(|_| "Gaze of the Gorgon candidate is not valid Rust source".to_owned())?;
    if normalized_source(source)? != normalized_source(CANONICAL_CANDIDATE_SOURCE)? {
        return Err(
            "Gaze of the Gorgon candidate differs from canonical normalized source".to_owned(),
        );
    }
    if !candidate_ast_has_literal_shape(&file) {
        return Err(
            "Gaze of the Gorgon candidate AST differs from its literal constructor contract"
                .to_owned(),
        );
    }
    Ok(())
}

fn normalized_source(source: &str) -> Result<String, String> {
    let chars: Vec<char> = source.chars().collect();
    let mut normalized = String::with_capacity(source.len());
    let mut index = 0;
    let mut block_comment_depth = 0_usize;
    let mut line_comment = false;
    let mut string = false;
    let mut character = false;
    while index < chars.len() {
        let current = chars[index];
        if line_comment {
            if current == '\n' {
                line_comment = false;
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
        if string || character {
            normalized.push(current);
            if current == '\\' {
                if let Some(escaped) = chars.get(index + 1) {
                    normalized.push(*escaped);
                    index += 2;
                    continue;
                }
            } else if (string && current == '"') || (character && current == '\'') {
                string = false;
                character = false;
            }
            index += 1;
            continue;
        }
        if current == '/' && chars.get(index + 1) == Some(&'/') {
            line_comment = true;
            index += 2;
        } else if current == '/' && chars.get(index + 1) == Some(&'*') {
            block_comment_depth = 1;
            index += 2;
        } else if current == '"' {
            string = true;
            normalized.push(current);
            index += 1;
        } else if current == '\'' {
            character = true;
            normalized.push(current);
            index += 1;
        } else if !current.is_whitespace() {
            normalized.push(current);
            index += 1;
        } else {
            index += 1;
        }
    }
    if string || character || block_comment_depth != 0 {
        return Err("Gaze candidate has an unterminated literal or comment".to_owned());
    }
    Ok(normalized)
}

fn candidate_ast_has_literal_shape(file: &syn::File) -> bool {
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
        || file.items.len() != 3
    {
        return false;
    }
    let mut uses = 0;
    let mut functions = 0;
    for item in &file.items {
        match item {
            Item::Use(import)
                if import.attrs.is_empty() && matches!(&import.vis, Visibility::Inherited) =>
            {
                uses += 1
            }
            Item::Fn(function) => {
                if function.sig.ident != "definition"
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || function.sig.inputs.len() != 0
                    || function.sig.constness.is_some()
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || !function.sig.generics.params.is_empty()
                    || function.block.stmts.len() != 1
                    || !function.attrs.iter().all(|attribute| {
                        attribute.path().is_ident("must_use") || attribute.path().is_ident("doc")
                    })
                {
                    return false;
                }
                let Stmt::Expr(Expr::Struct(literal), None) = &function.block.stmts[0] else {
                    return false;
                };
                if !literal.path.is_ident("CardDefinition") || literal.rest.is_some() {
                    return false;
                }
                functions += 1;
            }
            _ => return false,
        }
    }
    uses == 2 && functions == 1
}

fn validate_compile_valid_side_effect_candidate() -> Result<Value, String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Gaze side-effect fixture source: {error}"))?;
    let malicious = format!(
        "{source}\nfn injected_side_effect_candidate() {{ let _ = std::fs::write(std::env::temp_dir().join(\"cardbench-gaze-side-effect\"), b\"mutation\"); }}\n"
    );
    syn::parse_file(&malicious)
        .map_err(|_| "compile-valid side-effect fixture is not valid Rust syntax".to_owned())?;
    if validate_candidate_source_text(&malicious).is_ok() {
        return Err("Gaze source guard accepted an injected side-effect function".to_owned());
    }
    Ok(json!({
        "normalized_literal_source_and_ast_checked_before_candidate_calls": true,
        "compile_valid_side_effect_candidate_by_construction": true,
        "compile_valid_injected_side_effect_candidate_rejected_before_candidate_calls": true,
        "candidate_functions_called_before_source_validation": false,
        "side_effect_fixture_executed": false
    }))
}

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let unique: Vec<_> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || unique.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != 306
        || unique.len() != 291
        || !printings
            .iter()
            .any(|card| card.collector_number == 246 && card.name == CARD_NAME)
    {
        return Err("RAV catalog denominator or Gaze of the Gorgon identity differs".to_owned());
    }
    Ok(unique)
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
        "engine_status": match card.semantic_status {
            CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice",
            CardSemanticStatus::CatalogOnly { .. } => "catalog_only"
        },
        "trace_status": if passed {"passed"} else {"not_qualified"},
        "credited": false,
        "qualification": if passed {"pilot_trace_passed; independent review pending"} else {"not_evaluated"}
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    gaze: Option<ObjectId>,
    gaze_colors: Vec<Color>,
    gaze_hybrid: Vec<Color>,
    gaze_generic: Vec<Color>,
    target: Option<ObjectId>,
    blocker: Option<ObjectId>,
    opponent_watchwolf: Option<ObjectId>,
    pilot_watchwolf: Option<ObjectId>,
    gaze_zone: Option<Zone>,
    target_zone: Option<Zone>,
    blocker_zone: Option<Zone>,
    life: [i64; 2],
}

impl Captured {
    fn empty() -> Self {
        Self {
            events: Vec::new(),
            winner: None,
            turns: 0,
            accepted: 0,
            rejected: 0,
            gaze: None,
            gaze_colors: Vec::new(),
            gaze_hybrid: Vec::new(),
            gaze_generic: Vec::new(),
            target: None,
            blocker: None,
            opponent_watchwolf: None,
            pilot_watchwolf: None,
            gaze_zone: None,
            target_zone: None,
            blocker_zone: None,
            life: [0, 0],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    RemoveEffect,
    RemoveHybridSymbol,
}

fn find_reference() -> Result<(u64, Captured), String> {
    let mut last_error = String::new();
    for seed in 0..=MAX_SEED {
        let captured = run_match(seed, Mutation::None)?;
        match validate_trace(&captured, seed) {
            Ok(_) => return Ok((seed, captured)),
            Err(error) if error.contains("seed-dependent") => last_error = error,
            Err(error) => return Err(error),
        }
    }
    Err(format!(
        "no bounded seed produced the Gaze full-game combat trace: {last_error}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    let captured = run_match(seed, mutation)?;
    Ok(validate_trace(&captured, seed).is_err())
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut definitions = card_definitions();
    let mut submitted = candidate_gaze_of_the_gorgon::definition();
    match mutation {
        Mutation::None => {}
        Mutation::RemoveEffect => submitted.effects.clear(),
        Mutation::RemoveHybridSymbol => submitted.mana_cost = ManaCost::new(4),
    }
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Gaze of the Gorgon definition is absent from RAV registry")? = submitted;
    let mut game = Game::new_with_all_bindings(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .map_err(|error| error.to_string())?;
    let pilot = deck(&[
        (CARD_ID, 4),
        (THUG_ID, 4),
        (WATCHWOLF_ID, 4),
        (SWAMP_ID, 16),
        (FOREST_ID, 16),
        (PLAINS_ID, 16),
    ]);
    let opponent = deck(&[(WATCHWOLF_ID, 1), (FOREST_ID, 30), (PLAINS_ID, 29)]);
    for (label, list) in [("Gaze pilot", &pilot), ("Gaze opponent", &opponent)] {
        let size: usize = list
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if size != 60 {
            return Err(format!("{label} deck has {size} cards, expected 60"));
        }
    }
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    for (seat, list) in [(PlayerId(0), &pilot), (PlayerId(1), &opponent)] {
        game.load_deck_into_library(seat, list)
            .map_err(|error| error.to_string())?;
        game.draw_opening_hand(seat, 7)
            .map_err(|error| error.to_string())?;
    }
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(GazePilot::new(PlayerId(0))),
        Box::new(GazePilot::new(PlayerId(1))),
    ];
    let mut captured = Captured::empty();
    let mut attempted = 0_u32;
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || captured.accepted >= MAX_MOVES {
            return Err("Gaze PvP game exceeded bounded turn or move count".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Gaze policy seat missing")?;
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
        attempted = attempted.saturating_add(1);
        if let PolicyAction::CastWithPayment {
            request,
            mana_selection,
            ..
        } = &action
            && player == PlayerId(0)
            && game
                .card_definition(request.card)
                .is_ok_and(|definition| definition.id == CARD_ID)
        {
            captured.gaze_hybrid = mana_selection.hybrid.clone();
            captured.gaze_generic = mana_selection.generic.clone();
        }
        let start = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Gaze policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        captured.accepted = captured.accepted.saturating_add(1);
        for event in &game.event_log[start..] {
            match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    captured.gaze = Some(*card);
                    captured.gaze_colors = colors.clone();
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    captured.gaze = Some(*card)
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == WATCHWOLF_ID) =>
                {
                    captured.pilot_watchwolf.get_or_insert(*card);
                }
                GameEvent::SpellCast {
                    player: PlayerId(1),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == WATCHWOLF_ID) =>
                {
                    captured.opponent_watchwolf = Some(*card)
                }
                _ => {}
            }
        }
        game.validate_invariants()
            .map_err(|error| format!("Gaze invariant failed: {error}"))?;
    }
    captured.events = game.event_log.clone();
    captured.winner = game.winner().map(|player| player.0);
    captured.turns = game.turn;
    captured.rejected = attempted.saturating_sub(captured.accepted);
    captured.gaze_zone = captured.gaze.and_then(|object| game.zone_of(object));
    captured.target = captured.events.iter().find_map(|event| match event {
        GameEvent::AttackersDeclared {
            player: PlayerId(0),
            attackers,
        } => attackers
            .iter()
            .find(|object| {
                game.card_definition(**object)
                    .is_ok_and(|definition| definition.id == THUG_ID)
            })
            .copied(),
        _ => None,
    });
    captured.blocker = captured.events.iter().find_map(|event| match event {
        GameEvent::BlockersDeclared {
            player: PlayerId(1),
            assignments,
        } => assignments.iter().find_map(|(attacker, blocker)| {
            (Some(*attacker) == captured.target).then_some(*blocker)
        }),
        _ => None,
    });
    captured.target_zone = captured.target.and_then(|object| game.zone_of(object));
    captured.blocker_zone = captured.blocker.and_then(|object| game.zone_of(object));
    captured.life = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    Ok(captured)
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

struct GazePilot {
    player: PlayerId,
}

impl GazePilot {
    fn new(player: PlayerId) -> Self {
        Self { player }
    }
}

impl CodePolicy for GazePilot {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.gaze-of-the-gorgon-combat-pilot.v1"
        } else {
            "rav.gaze-of-the-gorgon-blocker.v1"
        }
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        let gaze_cost = candidate_gaze_of_the_gorgon::definition().mana_cost;
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            if self.player == PlayerId(0) {
                let opponent_wolf = view
                    .opponent_battlefield
                    .iter()
                    .any(|card| card.definition == Some(WATCHWOLF_ID));
                if opponent_wolf {
                    let thug = view
                        .own_battlefield
                        .iter()
                        .find(|card| card.definition == Some(THUG_ID) && card.can_attack);
                    let gaze_ready = view
                        .hand
                        .iter()
                        .any(|card| card.definition == Some(CARD_ID))
                        && can_generate_cost(view, &gaze_cost);
                    let attackers = if gaze_ready {
                        thug.map(|card| vec![card.id]).unwrap_or_default()
                    } else {
                        vec![]
                    };
                    return PolicyAction::DeclareAttackers { attackers };
                }
                let attackers = view
                    .own_battlefield
                    .iter()
                    .filter(|card| {
                        (card.definition == Some(WATCHWOLF_ID) || card.definition == Some(THUG_ID))
                            && card.can_attack
                    })
                    .map(|card| card.id)
                    .collect();
                return PolicyAction::DeclareAttackers { attackers };
            }
            return PolicyAction::DeclareAttackers { attackers: vec![] };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            if self.player == PlayerId(1) {
                let attacker = view
                    .opponent_battlefield
                    .iter()
                    .find(|card| card.definition == Some(THUG_ID));
                let blocker = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.definition == Some(WATCHWOLF_ID) && card.can_block);
                if let (Some(attacker), Some(blocker)) = (attacker, blocker) {
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
        if self.player == PlayerId(0)
            && view.step == Step::DeclareBlockers
            && view.active_player == self.player
        {
            let thug = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(THUG_ID));
            let was_blocked = thug.is_some_and(|target| {
                view.submitted_blocks.iter().any(|(seat, assignments)| {
                    *seat == PlayerId(1)
                        && assignments.iter().any(|block| block.attacker == target.id)
                })
            });
            if was_blocked
                && let (Some(target), Some(spell)) = (
                    thug,
                    view.hand
                        .iter()
                        .find(|card| card.definition == Some(CARD_ID)),
                )
            {
                if let Some(selection) = select_payment(view, &gaze_cost) {
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: spell.id,
                            targets: vec![Target::Permanent(target.id)],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: selection,
                    };
                }
                if let Some(action) = make_mana(view, &gaze_cost) {
                    return action;
                }
            }
        }
        if view.active_player == self.player
            && view.step == Step::PrecombatMain
            && view.stack_depth == 0
        {
            if view.lands_played == 0
                && let Some(land) = view
                    .hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
            if self.player == PlayerId(0) {
                let has_target_blocker = view
                    .opponent_battlefield
                    .iter()
                    .any(|card| card.definition == Some(WATCHWOLF_ID));
                if !has_target_blocker
                    && !view
                        .own_battlefield
                        .iter()
                        .any(|card| card.definition == Some(WATCHWOLF_ID))
                    && let Some(wolf) = view
                        .hand
                        .iter()
                        .find(|card| card.definition == Some(WATCHWOLF_ID))
                {
                    let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
                    if let Some(selection) = select_payment(view, &cost) {
                        return cast(wolf.id, vec![], selection);
                    }
                    if let Some(action) = make_mana(view, &cost) {
                        return action;
                    }
                }
                if !view
                    .own_battlefield
                    .iter()
                    .any(|card| card.definition == Some(THUG_ID))
                    && let Some(thug) = view
                        .hand
                        .iter()
                        .find(|card| card.definition == Some(THUG_ID))
                {
                    let cost = ManaCost::with_colors(1, [Color::Black]);
                    if let Some(selection) = select_payment(view, &cost) {
                        return cast(thug.id, vec![], selection);
                    }
                    if let Some(action) = make_mana(view, &cost) {
                        return action;
                    }
                }
            } else if let Some(wolf) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(WATCHWOLF_ID))
            {
                let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
                if let Some(selection) = select_payment(view, &cost) {
                    return cast(wolf.id, vec![], selection);
                }
                if let Some(action) = make_mana(view, &cost) {
                    return action;
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn cast(
    card: ObjectId,
    targets: Vec<Target>,
    mana_selection: ManaPaymentSelection,
) -> PolicyAction {
    PolicyAction::CastWithPayment {
        request: cardbench_magic_engine::CastRequest {
            card,
            targets,
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        chosen_x: None,
        mana_selection,
    }
}

fn select_payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: BTreeMap<Color, u8> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let amount = available.get_mut(color)?;
        if *amount == 0 {
            return None;
        }
        *amount -= 1;
    }
    let mut hybrid = Vec::new();
    for symbol in &cost.hybrid {
        let choice = if available.get(&symbol.first).copied().unwrap_or(0) > 0 {
            symbol.first
        } else if available.get(&symbol.second).copied().unwrap_or(0) > 0 {
            symbol.second
        } else {
            return None;
        };
        *available.get_mut(&choice)? -= 1;
        hybrid.push(choice);
    }
    let mut generic = Vec::new();
    for _ in 0..cost.generic {
        let color = available
            .iter()
            .filter(|(_, amount)| **amount > 0)
            .max_by_key(|(_, amount)| **amount)
            .map(|(color, _)| *color)?;
        *available.get_mut(&color)? -= 1;
        generic.push(color);
    }
    Some(ManaPaymentSelection { generic, hybrid })
}

fn can_generate_cost(view: &GameView, cost: &ManaCost) -> bool {
    let mut available: BTreeMap<Color, u16> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, u16::from(view.mana_pool.amount(color))))
        .collect();
    for land in view
        .own_battlefield
        .iter()
        .filter(|card| !card.tapped && card.card_types.contains(&CardType::Land))
    {
        if let Some(color) = land.mana_colors.iter().next() {
            *available.entry(*color).or_default() += 1;
        }
    }
    for color in &cost.colored {
        let Some(amount) = available.get_mut(color) else {
            return false;
        };
        if *amount == 0 {
            return false;
        }
        *amount -= 1;
    }
    for symbol in &cost.hybrid {
        let choice = if available.get(&symbol.first).copied().unwrap_or(0) > 0 {
            symbol.first
        } else if available.get(&symbol.second).copied().unwrap_or(0) > 0 {
            symbol.second
        } else {
            return false;
        };
        let Some(amount) = available.get_mut(&choice) else {
            return false;
        };
        *amount -= 1;
    }
    available
        .values()
        .map(|amount| *amount as usize)
        .sum::<usize>()
        >= usize::from(cost.generic)
}

fn make_mana(view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
    let available: BTreeMap<Color, u8> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    let mut desired = cost
        .colored
        .iter()
        .find(|color| available.get(color).copied().unwrap_or(0) == 0)
        .copied();
    if desired.is_none() {
        desired = cost
            .hybrid
            .iter()
            .find(|symbol| {
                available.get(&symbol.first).copied().unwrap_or(0) == 0
                    && available.get(&symbol.second).copied().unwrap_or(0) == 0
            })
            .map(|symbol| symbol.first);
    }
    if desired.is_none()
        && u16::from(view.mana_pool.total())
            < u16::from(cost.generic) + cost.colored.len() as u16 + cost.hybrid.len() as u16
    {
        desired = cost
            .colored
            .first()
            .copied()
            .or_else(|| cost.hybrid.first().map(|symbol| symbol.first))
            .or_else(|| {
                view.own_battlefield
                    .iter()
                    .flat_map(|card| card.mana_colors.iter().copied())
                    .next()
            });
    }
    let land = view
        .own_battlefield
        .iter()
        .filter(|card| !card.tapped && card.card_types.contains(&CardType::Land))
        .find(|card| desired.is_some_and(|color| card.mana_colors.contains(&color)))
        .or_else(|| {
            view.own_battlefield.iter().find(|card| {
                !card.tapped
                    && card.card_types.contains(&CardType::Land)
                    && !card.mana_colors.is_empty()
            })
        })?;
    let color = desired
        .filter(|color| land.mana_colors.contains(color))
        .or_else(|| land.mana_colors.iter().next().copied())?;
    Some(PolicyAction::ActivateManaAbility {
        land: land.id,
        color,
    })
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    let gaze = captured
        .gaze
        .ok_or("seed-dependent: Gaze was not drawn and cast")?;
    let target = captured
        .target
        .ok_or("seed-dependent: Golgari Thug did not attack")?;
    let blocker = captured
        .blocker
        .ok_or("seed-dependent: opposing Watchwolf did not block Golgari Thug")?;
    let opponent_wolf = captured
        .opponent_watchwolf
        .ok_or("seed-dependent: opposing Watchwolf was not cast")?;
    let pilot_wolf = captured
        .pilot_watchwolf
        .ok_or("seed-dependent: pilot Watchwolf was not cast")?;
    if target == blocker || blocker != opponent_wolf {
        return Err(
            "Gaze trace did not retain the exact attacking and blocking object identities"
                .to_owned(),
        );
    }
    if captured.gaze_colors.len() != 4
        || captured.gaze_generic.len() != 3
        || captured.gaze_hybrid.len() != 1
        || ![Color::Black, Color::Green].contains(&captured.gaze_hybrid[0])
    {
        return Err(format!(
            "Gaze did not pay {{3}}{{B/G}} with three generic plus one legal hybrid mana: {:?} / {:?} / {:?}",
            captured.gaze_generic, captured.gaze_hybrid, captured.gaze_colors
        ));
    }
    let mut paid = captured.gaze_generic.clone();
    paid.extend(captured.gaze_hybrid.clone());
    let mut event_paid = captured.gaze_colors.clone();
    paid.sort();
    event_paid.sort();
    if paid != event_paid {
        return Err(
            "Gaze payment selection differs from the engine's actual payment receipt".to_owned(),
        );
    }
    if captured.gaze_zone != Some(Zone::Graveyard)
        || captured.target_zone != Some(Zone::Battlefield)
        || captured.blocker_zone != Some(Zone::Graveyard)
    {
        return Err("Gaze did not resolve to the graveyard, preserve its target, and destroy the recorded blocker".to_owned());
    }
    if captured.winner != Some(0)
        || captured.life[0] <= 0
        || captured.life[1] > 0
        || captured.rejected != 0
    {
        return Err("Gaze full-game PvP did not reach a clean seat-zero terminal win".to_owned());
    }
    let events = &captured.events;
    let opponent_cast = event_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == opponent_wolf)).ok_or("opponent Watchwolf cast is missing")?;
    let opponent_paid = event_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(1), card, colors } if *card == opponent_wolf && colors.contains(&Color::Green) && colors.contains(&Color::White))).ok_or("opponent Watchwolf colored payment is missing")?;
    let target_attack = event_index_after(events, opponent_cast, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&target))).ok_or("target attack event is missing")?;
    let block = event_index_after(events, target_attack, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if assignments.contains(&(target, blocker)))).ok_or("exact blocker assignment is missing")?;
    let gaze_paid = event_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == gaze && colors.len() == 4)).ok_or("Gaze mana payment event is missing")?;
    let gaze_cast = event_index_after(events, gaze_paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == gaze)).ok_or("Gaze cast event is missing")?;
    let gaze_resolved = event_index_after(
        events,
        gaze_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == gaze),
    )
    .ok_or("Gaze resolution event is missing")?;
    let shield = event_index_after(events, gaze_resolved, |event| matches!(event, GameEvent::RegenerationShieldCreated { source, target: protected } if *source == gaze && *protected == target)).ok_or("Gaze did not create a shield on its target")?;
    let scheduled = event_index_after(events, shield, |event| matches!(event, GameEvent::DelayedCombatDestructionScheduled { source, target: scheduled_target, due_turn, .. } if *source == gaze && *scheduled_target == target && *due_turn > 0)).ok_or("Gaze did not schedule its target-bound end-of-combat action")?;
    let blocker_damage = event_index_after(events, block, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 3 } if *source == blocker && *permanent == target)).ok_or("blocked attacker did not receive the Watchwolf's three combat damage")?;
    let shield_used = event_index_after(events, blocker_damage, |event| matches!(event, GameEvent::RegenerationShieldUsed { source, target: protected } if *source == gaze && *protected == target)).ok_or("regeneration shield did not replace the target's combat destruction")?;
    let delayed_stack = event_index_after(events, shield_used, |event| matches!(event, GameEvent::DelayedCombatDestructionStacked { source, target: scheduled_target, participants, .. } if *source == gaze && *scheduled_target == target && participants.len() == 1 && participants[0].permanent == blocker && participants[0].incarnation > 0)).ok_or("delayed action did not capture the exact blocker incarnation")?;
    let destroyed = event_index_after(events, delayed_stack, |event| matches!(event, GameEvent::CardDestroyed { source, card } if *source == gaze && *card == blocker)).ok_or("delayed action did not destroy the recorded blocker")?;
    let movement = event_index_after(events, destroyed, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard, .. } if *card == blocker)).ok_or("destroyed blocker did not move to its owner's graveyard")?;
    let resolved_action = event_index_after(events, movement, |event| matches!(event, GameEvent::AbilityResolved { source, ability, .. } if *source == gaze && *ability == DELAYED_COMBAT_HISTORY_DESTRUCTION_ABILITY_ID)).ok_or("ordinary delayed ability resolution receipt is missing")?;
    let pilot_attack = event_index_after(events, resolved_action, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&pilot_wolf))).ok_or("pilot Watchwolf did not attack after the Gaze exchange")?;
    let pilot_damage = event_index_after(events, pilot_attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 3 } if *source == pilot_wolf)).ok_or("pilot Watchwolf did not deal combat damage to the opponent")?;
    let terminal = event_index_after(events, pilot_damage, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("normal terminal seat-zero game event is missing")?;
    if !(opponent_paid < opponent_cast
        && opponent_cast < target_attack
        && target_attack < block
        && block < gaze_paid
        && gaze_paid < gaze_cast
        && gaze_cast < gaze_resolved
        && gaze_resolved < shield
        && shield < scheduled
        && scheduled < blocker_damage
        && blocker_damage < shield_used
        && shield_used < delayed_stack
        && delayed_stack < destroyed
        && destroyed < movement
        && movement < resolved_action
        && resolved_action < pilot_attack
        && pilot_attack < pilot_damage
        && pilot_damage < terminal)
    {
        return Err("Gaze payment, target, combat history, delayed destruction, and terminal event order differs".to_owned());
    }
    let projected = project_events(events);
    let event_values: Vec<Value> = projected.iter().map(|event| json!({"sequence": event.sequence, "turn": event.turn, "kind": &event.kind, "canonical": &event.canonical})).collect();
    Ok(json!({
        "id": "rav-gaze-of-the-gorgon-regeneration-and-block-history",
        "passed": true,
        "seed": seed,
        "decks": ["rav_gaze_of_the_gorgon_pilot", "rav_gaze_of_the_gorgon_opponent"],
        "policies": ["rav.gaze-of-the-gorgon-combat-pilot.v1", "rav.gaze-of-the-gorgon-blocker.v1"],
        "terminal": format!("Winner(PlayerId({}))", captured.winner.unwrap_or(usize::MAX)),
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted,
        "rejected_moves": captured.rejected,
        "gaze_object": gaze.0,
        "gaze_mana_cost": "{3}{B/G}",
        "gaze_payment": {"colors": captured.gaze_colors.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "generic": captured.gaze_generic.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "hybrid": captured.gaze_hybrid.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>()},
        "target_object": target.0,
        "target_definition": THUG_ID,
        "target_zone_after_game": format!("{:?}", captured.target_zone),
        "blocker_object": blocker.0,
        "blocker_definition": WATCHWOLF_ID,
        "blocker_zone_after_game": format!("{:?}", captured.blocker_zone),
        "opponent_watchwolf_object": opponent_wolf.0,
        "pilot_watchwolf_object": pilot_wolf.0,
        "gaze_zone_after_game": format!("{:?}", captured.gaze_zone),
        "life_at_match_end": captured.life,
        "ordered_event_sequence": [opponent_paid, opponent_cast, target_attack, block, gaze_paid, gaze_cast, gaze_resolved, shield, scheduled, blocker_damage, shield_used, delayed_stack, destroyed, movement, resolved_action, pilot_attack, pilot_damage, terminal],
        "events": event_values
    }))
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
