//! Full-game RAV #251 authority for Privileged Position's opponent-only protection.

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, ContinuousChange, DeckEntry, DeckList, Game,
    GameEvent, GameView, HybridManaSymbol, Keyword, ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, StaticContinuousEffectBinding, Step, Target, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings, rav_static_continuous_effect_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-PRIVILEGED-POSITION";
const CARD_NAME: &str = "Privileged Position";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const PUTREFY_ID: &str = "RAV-PUTREFY";
const GATHER_COURAGE_ID: &str = "RAV-GATHER-COURAGE";
const FOREST_ID: &str = "RAV-FOREST";
const PLAINS_ID: &str = "RAV-PLAINS";
const SWAMP_ID: &str = "RAV-SWAMP";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/privileged_position.rs";
const MAX_SEED: u64 = 256;
const MAX_TURNS: u32 = 50;
const MAX_MOVES: u32 = 5_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/privileged_position.rs"]
mod candidate_privileged_position;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Privileged Position result: {error}");
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
            eprintln!("Privileged Position PvP authority failed: {error}");
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
    validate_candidate_source()?;
    let source_guard = validate_compile_valid_side_effect_candidate()?;
    validate_candidate_registry()?;
    let catalog = unique_catalog()?;
    let (seed, captured) = find_reference()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if !same_match(&captured, &replay) {
        return Err("Privileged Position shuffled full-game replay differed".to_owned());
    }

    let mutations = json!({
        "static_binding_removal_detected": mutation_detected(seed, Mutation::RemoveBinding)?,
        "wrong_static_source_scope_detected": mutation_detected(seed, Mutation::WrongSourceScope)?,
        "shroud_broadening_detected_by_controller_target": mutation_detected(seed, Mutation::Shroud)?,
        "wrong_static_keyword_detected": mutation_detected(seed, Mutation::Flying)?,
        "hybrid_cost_mutation_detected": mutation_detected(seed, Mutation::WrongCost)?,
        "card_type_mutation_detected": mutation_detected(seed, Mutation::WrongType)?,
        "compile_valid_side_effect_candidate_rejected_before_candidate_calls": source_guard["compile_valid_side_effect_candidate_rejected_before_candidate_calls"] == true,
        "sealed_mutation_authority": false
    });
    for key in [
        "static_binding_removal_detected",
        "wrong_static_source_scope_detected",
        "shroud_broadening_detected_by_controller_target",
        "wrong_static_keyword_detected",
        "hybrid_cost_mutation_detected",
        "card_type_mutation_detected",
        "compile_valid_side_effect_candidate_rejected_before_candidate_calls",
    ] {
        if mutations[key] != true {
            return Err(format!("Privileged Position mutation escaped: {key}"));
        }
    }

    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 251, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "authority_cutoff": "2026-09-24",
            "source_contract": "RAV #251 costs {2}{G/W}{G/W}{G/W} and says other permanents you control can't be the targets of spells or abilities your opponents control. Oracle's Hexproof wording preserves this opponent-only restriction; Shroud is broader and is not equivalent.",
            "checked_sections": ["601.2 Casting spells and choosing targets", "702.11 Hexproof", "702.18 Shroud", "113.6 Static abilities", "611 Continuous effects", "704 State-based actions"]
        },
        "candidate_source_validation": source_guard,
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": rav_main_set_catalog().len(),
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    RemoveBinding,
    WrongSourceScope,
    Shroud,
    Flying,
    WrongCost,
    WrongType,
}

fn candidate_definition(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_privileged_position::definition();
    match mutation {
        Mutation::WrongCost => definition.mana_cost = ManaCost::new(5),
        Mutation::WrongType => definition.card_types = BTreeSet::from([CardType::Creature]),
        _ => {}
    }
    definition
}

fn binding_with_mutation(mutation: Mutation) -> StaticContinuousEffectBinding {
    let mut binding = candidate_privileged_position::static_continuous_effect();
    match mutation {
        Mutation::WrongSourceScope => binding.card_definition = WATCHWOLF_ID,
        Mutation::Shroud => {
            binding.change = ContinuousChange::OtherControlledPermanentsAddKeyword(Keyword::Shroud)
        }
        Mutation::Flying => {
            binding.change = ContinuousChange::OtherControlledPermanentsAddKeyword(Keyword::Flying)
        }
        _ => {}
    }
    binding
}

fn validate_candidate_registry() -> Result<(), String> {
    let definition = candidate_privileged_position::definition();
    let registered_definition = card_definitions()
        .into_iter()
        .find(|item| item.id == CARD_ID)
        .ok_or("Privileged Position definition is missing from the RAV registry")?;
    let binding = candidate_privileged_position::static_continuous_effect();
    let registered_bindings: Vec<_> = rav_static_continuous_effect_bindings()
        .into_iter()
        .filter(|item| item.card_definition == CARD_ID)
        .collect();
    if registered_definition != definition || registered_bindings != [binding.clone()] {
        return Err("Privileged Position registry does not delegate its definition and Hexproof effect to the candidate module".to_owned());
    }
    let expected_cost = ManaCost::with_hybrid(
        2,
        [],
        [
            HybridManaSymbol {
                first: Color::Green,
                second: Color::White,
            },
            HybridManaSymbol {
                first: Color::Green,
                second: Color::White,
            },
            HybridManaSymbol {
                first: Color::Green,
                second: Color::White,
            },
        ],
    );
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != expected_cost
        || definition.colors != BTreeSet::from([Color::Green, Color::White])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Enchantment])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
        || definition.supported_rules
            != [
                "full-rules-fidelity",
                "hybrid-cost-casting",
                "other-controlled-permanents-have-hexproof",
            ]
        || binding.card_definition != CARD_ID
        || binding.change
            != ContinuousChange::OtherControlledPermanentsAddKeyword(Keyword::Hexproof)
    {
        return Err(
            "Privileged Position identity or opponent-only Hexproof binding differs from RAV #251"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Privileged Position source: {error}"))?;
    validate_candidate_source_text(&source)
}

const CANONICAL_CANDIDATE_SOURCE: &str = r#"//! Candidate-owned RAV #251 Privileged Position identity and hexproof effect.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, ContinuousChange, HybridManaSymbol, Keyword, ManaCost,
    StaticContinuousEffectBinding,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-PRIVILEGED-POSITION",
        name: "Privileged Position",
        set_code: "RAV",
        mana_cost: ManaCost::with_hybrid(
            2,
            [],
            [
                HybridManaSymbol {
                    first: Color::Green,
                    second: Color::White,
                },
                HybridManaSymbol {
                    first: Color::Green,
                    second: Color::White,
                },
                HybridManaSymbol {
                    first: Color::Green,
                    second: Color::White,
                },
            ],
        ),
        colors: BTreeSet::from([Color::Green, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "hybrid-cost-casting",
            "other-controlled-permanents-have-hexproof",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn static_continuous_effect() -> StaticContinuousEffectBinding {
    StaticContinuousEffectBinding {
        card_definition: "RAV-PRIVILEGED-POSITION",
        change: ContinuousChange::OtherControlledPermanentsAddKeyword(Keyword::Hexproof),
    }
}"#;

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let file = syn::parse_file(source)
        .map_err(|_| "Privileged Position candidate is not valid Rust source".to_owned())?;
    if normalized_source(source)? != normalized_source(CANONICAL_CANDIDATE_SOURCE)? {
        return Err(
            "Privileged Position candidate differs from the canonical normalized source".to_owned(),
        );
    }
    if !candidate_ast_has_literal_shape(&file) {
        return Err(
            "Privileged Position candidate AST differs from the literal constructor contract"
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
        return Err(
            "Privileged Position candidate has an unterminated literal or comment".to_owned(),
        );
    }
    Ok(normalized)
}

fn candidate_ast_has_literal_shape(file: &syn::File) -> bool {
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
        || file.items.len() != 4
    {
        return false;
    }
    let mut constructors = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(import)
                if import.attrs.is_empty() && matches!(&import.vis, Visibility::Inherited) => {}
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                let expected_type = match name.as_str() {
                    "definition" => "CardDefinition",
                    "static_continuous_effect" => "StaticContinuousEffectBinding",
                    _ => return false,
                };
                if !constructors.insert(name)
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !function.sig.inputs.is_empty()
                    || function.sig.constness.is_some()
                    || function.sig.asyncness.is_some()
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
                if !literal.path.is_ident(expected_type) || literal.rest.is_some() {
                    return false;
                }
            }
            _ => return false,
        }
    }
    constructors
        == BTreeSet::from([
            "definition".to_owned(),
            "static_continuous_effect".to_owned(),
        ])
}

fn validate_compile_valid_side_effect_candidate() -> Result<Value, String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Privileged Position source fixture: {error}"))?;
    let malicious = format!(
        "{source}\nfn injected_side_effect() {{ let _ = std::fs::write(std::env::temp_dir().join(\"cardbench-privileged-position-side-effect\"), b\"mutation\"); }}\n"
    );
    syn::parse_file(&malicious).map_err(|_| {
        "compile-valid side-effect fixture is not syntactically valid Rust".to_owned()
    })?;
    if validate_candidate_source_text(&malicious).is_ok() {
        return Err(
            "Privileged Position source guard accepted an injected side-effect function".to_owned(),
        );
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
            .any(|card| card.collector_number == 251 && card.name == CARD_NAME)
    {
        return Err("RAV catalog denominator or Privileged Position identity differs".to_owned());
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
        "qualification": if passed {"pilot_trace_passed; independent reviewer approval pending"} else {"not_evaluated"}
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    position: Option<ObjectId>,
    position_incarnation: Option<u64>,
    position_is_enchantment: bool,
    position_payment: Vec<Color>,
    position_hybrid_payment: Vec<Color>,
    position_generic_payment: Vec<Color>,
    wolf: Option<ObjectId>,
    putrefy: Option<ObjectId>,
    rejected_target: Option<ObjectId>,
    rejected_by: Option<PlayerId>,
    rejection_error: Option<String>,
    rejection_atomic: bool,
    target_was_hexproof: bool,
    gather: Option<ObjectId>,
    gather_target: Option<ObjectId>,
    gather_payment: Vec<Color>,
    gather_resolved: bool,
    gather_target_power: Option<i32>,
    gather_target_toughness: Option<i32>,
    gather_target_hexproof: bool,
    attack_sequence: Option<usize>,
    combat_damage_sequence: Option<usize>,
    terminal_sequence: Option<usize>,
    life: [i64; 2],
}

impl Captured {
    fn empty() -> Self {
        Self {
            events: Vec::new(),
            winner: None,
            turns: 0,
            accepted: 0,
            position: None,
            position_incarnation: None,
            position_is_enchantment: false,
            position_payment: Vec::new(),
            position_hybrid_payment: Vec::new(),
            position_generic_payment: Vec::new(),
            wolf: None,
            putrefy: None,
            rejected_target: None,
            rejected_by: None,
            rejection_error: None,
            rejection_atomic: false,
            target_was_hexproof: false,
            gather: None,
            gather_target: None,
            gather_payment: Vec::new(),
            gather_resolved: false,
            gather_target_power: None,
            gather_target_toughness: None,
            gather_target_hexproof: false,
            attack_sequence: None,
            combat_damage_sequence: None,
            terminal_sequence: None,
            life: [0, 0],
        }
    }
}

fn find_reference() -> Result<(u64, Captured), String> {
    if let Ok(raw) = std::env::var("CARDBENCH_PRIVILEGED_POSITION_SEED") {
        let seed = raw.parse::<u64>().map_err(|_| {
            "CARDBENCH_PRIVILEGED_POSITION_SEED must be an unsigned integer".to_owned()
        })?;
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    let mut last_error = String::new();
    for seed in 0..=MAX_SEED {
        match run_match(seed, Mutation::None)
            .and_then(|captured| validate_trace(&captured, seed).map(|_| captured))
        {
            Ok(captured) => return Ok((seed, captured)),
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "no bounded seed produced a complete Privileged Position trace: {last_error}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    Ok(match run_match(seed, mutation) {
        Err(_) => true,
        Ok(captured) => validate_trace(&captured, seed).is_err(),
    })
}

fn game_for(mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|item| item.id == CARD_ID)
        .ok_or("Privileged Position definition missing while constructing game")? =
        candidate_definition(mutation);
    let mut bindings: Vec<_> = rav_static_continuous_effect_bindings()
        .into_iter()
        .filter(|item| item.card_definition != CARD_ID)
        .collect();
    if mutation != Mutation::RemoveBinding {
        bindings.push(binding_with_mutation(mutation));
    }
    Game::new_with_all_bindings_and_static_continuous_effects(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        bindings,
    )
    .map_err(|error| error.to_string())
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut game = game_for(mutation)?;
    let pilot = deck(&[
        (CARD_ID, 4),
        (WATCHWOLF_ID, 4),
        (GATHER_COURAGE_ID, 4),
        (FOREST_ID, 24),
        (PLAINS_ID, 24),
    ]);
    let opponent = deck(&[(PUTREFY_ID, 4), (SWAMP_ID, 28), (FOREST_ID, 28)]);
    for (name, list) in [
        ("Privileged Position pilot", &pilot),
        ("Putrefy opponent", &opponent),
    ] {
        if list
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
        {
            return Err(format!("{name} deck must contain exactly 60 cards"));
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
        Box::new(PositionPilot::new(PlayerId(0))),
        Box::new(PositionOpponent::new()),
    ];
    let mut captured = Captured::empty();
    let mut challenge_complete = false;
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || captured.accepted > MAX_MOVES {
            return Err(format!(
                "Privileged Position game exceeded bounds at turn {} with winner {:?}",
                game.turn,
                game.winner().map(|player| player.0)
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Privileged Position policy seat missing")?;
        let mut action = if let Some(action) = policy.propose_pending_decision(&view) {
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

        if !challenge_complete
            && player == PlayerId(1)
            && let PolicyAction::CastWithPayment { request, .. } = &action
            && game
                .card_definition(request.card)
                .is_ok_and(|definition| definition.id == PUTREFY_ID)
        {
            let target = request
                .targets
                .iter()
                .find_map(|target| match target {
                    Target::Permanent(id) => Some(*id),
                    _ => None,
                })
                .ok_or("opponent Putrefy action omitted its protected creature target")?;
            if !view
                .opponent_battlefield
                .iter()
                .any(|card| card.id == target && card.definition == Some(WATCHWOLF_ID))
            {
                return Err("opponent Putrefy did not target the pilot's Watchwolf".to_owned());
            }
            let before_events = game.event_log.clone();
            let before_stack = game.stack.clone();
            let before_hand = game
                .player(player)
                .map_err(|error| error.to_string())?
                .hand
                .clone();
            let before_pool = game
                .player(player)
                .map_err(|error| error.to_string())?
                .mana_pool
                .clone();
            let before_zone = game.zone_of(request.card);
            let before_keyword = game
                .characteristics(target)
                .map_err(|error| error.to_string())?
                .keywords
                .contains(&Keyword::Hexproof);
            let rejection_error = match game.cast_spell(player, request.clone()) {
                Err(
                    error @ cardbench_magic_engine::RulesError::IllegalTarget(Target::Permanent(
                        rejected,
                    )),
                ) if rejected == target => format!("{error:?}"),
                Err(error) => {
                    return Err(format!(
                        "Putrefy was rejected for the wrong reason: {error}"
                    ));
                }
                Ok(()) => {
                    return Err(
                        "Privileged Position failed to reject an opponent's Putrefy target"
                            .to_owned(),
                    );
                }
            };
            captured.rejection_atomic = game.event_log == before_events
                && game.stack == before_stack
                && game
                    .player(player)
                    .is_ok_and(|state| state.hand == before_hand && state.mana_pool == before_pool)
                && game.zone_of(request.card) == before_zone;
            captured.rejected_target = Some(target);
            captured.rejected_by = Some(player);
            captured.putrefy = Some(request.card);
            captured.rejection_error = Some(rejection_error);
            captured.target_was_hexproof = before_keyword;
            if !captured.rejection_atomic || !before_keyword {
                return Err("rejected Putrefy mutated state or target lacked Hexproof".to_owned());
            }
            action = PolicyAction::PassPriority;
            challenge_complete = true;
        }

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
            captured.position_hybrid_payment = mana_selection.hybrid.clone();
            captured.position_generic_payment = mana_selection.generic.clone();
        }
        if let PolicyAction::CastWithPayment { request, .. } = &action
            && player == PlayerId(0)
            && game
                .card_definition(request.card)
                .is_ok_and(|definition| definition.id == GATHER_COURAGE_ID)
        {
            captured.gather = Some(request.card);
            captured.gather_target = request.targets.iter().find_map(|target| match target {
                Target::Permanent(id) => Some(*id),
                _ => None,
            });
        }
        let start = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Privileged Position policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        captured.accepted += 1;
        for (offset, event) in game.event_log[start..].iter().enumerate() {
            let index = start + offset;
            match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    captured.position = Some(*card);
                    captured.position_payment = colors.clone();
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == GATHER_COURAGE_ID) =>
                {
                    captured.gather = Some(*card);
                    captured.gather_payment = colors.clone();
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    captured.position = Some(*card);
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == WATCHWOLF_ID) =>
                {
                    captured.wolf = Some(*card)
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == GATHER_COURAGE_ID) =>
                {
                    captured.gather = Some(*card)
                }
                GameEvent::SpellResolved { card } if Some(*card) == captured.gather => {
                    captured.gather_resolved = true;
                    if let Some(target) = captured.gather_target {
                        let characteristics = game
                            .characteristics(target)
                            .map_err(|error| error.to_string())?;
                        captured.gather_target_power = characteristics.power;
                        captured.gather_target_toughness = characteristics.toughness;
                        captured.gather_target_hexproof =
                            characteristics.keywords.contains(&Keyword::Hexproof);
                    }
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Battlefield,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    captured.position = Some(*card);
                    captured.position_incarnation =
                        game.object(*card).ok().map(|object| object.incarnation);
                    captured.position_is_enchantment =
                        game.card_definition(*card).is_ok_and(|definition| {
                            definition.card_types.contains(&CardType::Enchantment)
                        });
                }
                GameEvent::AttackersDeclared {
                    player: PlayerId(0),
                    attackers,
                } if captured.attack_sequence.is_none()
                    && captured.wolf.is_some_and(|wolf| attackers.contains(&wolf)) =>
                {
                    captured.attack_sequence = Some(index)
                }
                GameEvent::DamageDealtToPlayer {
                    source,
                    player: PlayerId(1),
                    ..
                } if captured.wolf == Some(*source)
                    && captured.combat_damage_sequence.is_none() =>
                {
                    captured.combat_damage_sequence = Some(index)
                }
                GameEvent::GameEnded { .. } => captured.terminal_sequence = Some(index),
                _ => {}
            }
        }
        game.validate_invariants()
            .map_err(|error| format!("Privileged Position invariant failed: {error}"))?;
    }
    captured.events = game.event_log.clone();
    captured.winner = game.winner().map(|player| player.0);
    captured.turns = game.turn;
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

fn same_match(left: &Captured, right: &Captured) -> bool {
    left.events == right.events
        && left.winner == right.winner
        && left.turns == right.turns
        && left.accepted == right.accepted
        && left.position == right.position
        && left.position_incarnation == right.position_incarnation
        && left.position_is_enchantment == right.position_is_enchantment
        && left.position_payment == right.position_payment
        && left.position_hybrid_payment == right.position_hybrid_payment
        && left.position_generic_payment == right.position_generic_payment
        && left.wolf == right.wolf
        && left.putrefy == right.putrefy
        && left.rejected_target == right.rejected_target
        && left.rejected_by == right.rejected_by
        && left.rejection_error == right.rejection_error
        && left.rejection_atomic == right.rejection_atomic
        && left.target_was_hexproof == right.target_was_hexproof
        && left.gather == right.gather
        && left.gather_target == right.gather_target
        && left.gather_payment == right.gather_payment
        && left.gather_resolved == right.gather_resolved
        && left.gather_target_power == right.gather_target_power
        && left.gather_target_toughness == right.gather_target_toughness
        && left.gather_target_hexproof == right.gather_target_hexproof
        && left.attack_sequence == right.attack_sequence
        && left.combat_damage_sequence == right.combat_damage_sequence
        && left.terminal_sequence == right.terminal_sequence
        && left.life == right.life
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.winner != Some(0) {
        return Err("Privileged Position duel did not end in a normal seat-zero win".to_owned());
    }
    let position = captured
        .position
        .ok_or("pilot did not cast Privileged Position")?;
    let incarnation = captured
        .position_incarnation
        .ok_or("Privileged Position battlefield incarnation missing")?;
    let wolf = captured.wolf.ok_or("pilot did not cast Watchwolf")?;
    let putrefy = captured
        .putrefy
        .ok_or("opponent did not attempt the Putrefy challenge")?;
    let gather = captured.gather.ok_or("pilot did not cast Gather Courage")?;
    let target = captured
        .rejected_target
        .ok_or("rejected Putrefy target missing")?;
    if target != wolf
        || captured.gather_target != Some(wolf)
        || captured.rejected_by != Some(PlayerId(1))
    {
        return Err(
            "opponent and controller target actions did not name the same protected Watchwolf"
                .to_owned(),
        );
    }
    if !captured.rejection_atomic || !captured.target_was_hexproof {
        return Err(
            "opponent target rejection was not atomic or lacked Hexproof evidence".to_owned(),
        );
    }
    if !captured.gather_resolved
        || captured.gather_target_power != Some(5)
        || captured.gather_target_toughness != Some(5)
        || !captured.gather_target_hexproof
    {
        return Err("controller's Gather Courage target did not legally resolve on the protected 3/3 Watchwolf".to_owned());
    }
    let mut selected_position_payment = captured.position_hybrid_payment.clone();
    selected_position_payment.extend(captured.position_generic_payment.iter().copied());
    selected_position_payment.sort();
    let mut paid_position_colors = captured.position_payment.clone();
    paid_position_colors.sort();
    if !captured.position_is_enchantment
        || captured.position_payment.len() != 5
        || captured
            .position_payment
            .iter()
            .any(|color| !matches!(*color, Color::Green | Color::White))
        || captured.position_hybrid_payment.len() != 3
        || captured
            .position_hybrid_payment
            .iter()
            .any(|color| !matches!(*color, Color::Green | Color::White))
        || captured.position_generic_payment.len() != 2
        || selected_position_payment != paid_position_colors
        || captured.gather_payment != [Color::Green]
    {
        return Err(
            "Privileged Position or Gather Courage mana payment differs from its printed cost"
                .to_owned(),
        );
    }
    let terminal = captured
        .terminal_sequence
        .ok_or("full game has no terminal event")?;
    let attack = captured
        .attack_sequence
        .ok_or("Watchwolf was not declared as an attacker")?;
    let combat_damage = captured
        .combat_damage_sequence
        .ok_or("Watchwolf dealt no combat damage to the opponent")?;
    if !(attack < combat_damage && combat_damage < terminal) {
        return Err("combat finish does not precede the terminal game event".to_owned());
    }
    let events = project_events(&captured.events);
    let index = |kind: &str, card: ObjectId, player: usize| -> Option<usize> {
        captured.events.iter().position(|event| match event {
            GameEvent::SpellCast {
                player: actor,
                card: object,
            } => actor.0 == player && *object == card && kind == "SpellCast",
            GameEvent::SpellManaPaid {
                player: actor,
                card: object,
                ..
            } => actor.0 == player && *object == card && kind == "SpellManaPaid",
            GameEvent::SpellResolved { card: object } => *object == card && kind == "SpellResolved",
            GameEvent::CardMoved {
                card: object,
                to: Zone::Battlefield,
            } => *object == card && kind == "CardMoved",
            _ => false,
        })
    };
    let position_cast =
        index("SpellCast", position, 0).ok_or("Privileged Position cast event missing")?;
    let position_paid = index("SpellManaPaid", position, 0)
        .ok_or("Privileged Position mana payment event missing")?;
    let position_resolved = index("SpellResolved", position, 0)
        .ok_or("Privileged Position resolution event missing")?;
    let position_entered =
        index("CardMoved", position, 0).ok_or("Privileged Position battlefield move missing")?;
    let wolf_cast = index("SpellCast", wolf, 0).ok_or("Watchwolf cast event missing")?;
    let wolf_entered = index("CardMoved", wolf, 0).ok_or("Watchwolf battlefield move missing")?;
    let gather_cast = index("SpellCast", gather, 0).ok_or("Gather Courage cast event missing")?;
    let gather_paid =
        index("SpellManaPaid", gather, 0).ok_or("Gather Courage mana payment event missing")?;
    let gather_resolved =
        index("SpellResolved", gather, 0).ok_or("Gather Courage resolution event missing")?;
    if !(wolf_cast < wolf_entered
        && wolf_entered < position_cast
        && wolf_entered < position_paid
        && position_paid < position_cast
        && position_cast < position_resolved
        && position_resolved < position_entered
        && position_entered < gather_paid
        && gather_paid < gather_cast
        && gather_cast < gather_resolved)
    {
        return Err(
            "Watchwolf, Privileged Position, and controller target spell event order differs"
                .to_owned(),
        );
    }
    let projected: Vec<Value> = serde_json::to_value(events)
        .map_err(|error| error.to_string())?
        .as_array()
        .cloned()
        .unwrap_or_default();
    Ok(json!({
        "id": "rav-privileged-position-opponent-only-hexproof",
        "seed": seed,
        "passed": true,
        "winner_seat": captured.winner,
        "terminal": "Winner(PlayerId(0))",
        "terminal_sequence": terminal,
        "turns": captured.turns,
        "accepted_moves": captured.accepted,
        "rejected_moves": 0,
        "policies": ["rav.privileged-position-hexproof-pilot.v1", "rav.privileged-position-putrefy-opponent.v1"],
        "decks": ["rav_privileged_position_pilot", "rav_putrefy_opponent"],
        "mechanics": {
            "privileged_position": {"object": position.0, "source_incarnation": incarnation, "is_enchantment": captured.position_is_enchantment, "mana_cost": "{2}{G/W}{G/W}{G/W}", "payment": captured.position_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "hybrid_payment": captured.position_hybrid_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "generic_payment": captured.position_generic_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>()},
            "protected_watchwolf": {"object": wolf.0, "controller_seat": 0, "base_power": 3, "base_toughness": 3, "hexproof_from_position": captured.target_was_hexproof},
            "opponent_putrefy_attempt": {"actor_seat": 1, "object": putrefy.0, "mana_cost": "{1}{B}{G}", "target": target.0, "error": captured.rejection_error, "rejected_before_payment": captured.rejection_atomic, "hand_stack_mana_and_event_state_unchanged": captured.rejection_atomic},
            "controller_gather_courage": {"actor_seat": 0, "object": gather.0, "mana_cost": "{G}", "payment": captured.gather_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "target": captured.gather_target.map(|object| object.0), "resolved": captured.gather_resolved, "target_power_after_resolution": captured.gather_target_power, "target_toughness_after_resolution": captured.gather_target_toughness, "target_still_has_hexproof": captured.gather_target_hexproof},
            "full_game": {"winner_seat": captured.winner, "watchwolf_attack_sequence": attack, "watchwolf_combat_damage_sequence": combat_damage, "life_totals": captured.life}
        },
        "event_indices": {"watchwolf_cast": wolf_cast, "position_payment": position_paid, "position_cast": position_cast, "position_resolved": position_resolved, "position_entered": position_entered, "gather_payment": gather_paid, "gather_cast": gather_cast, "gather_resolved": gather_resolved},
        "events": projected
    }))
}

struct PositionPilot {
    player: PlayerId,
    courage_offered: bool,
}
impl PositionPilot {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            courage_offered: false,
        }
    }
}

impl PositionPilot {
    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        view.hand
            .iter()
            .find(|card| card.definition == Some(FOREST_ID))
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some(PLAINS_ID))
            })
            .map(|land| PolicyAction::PlayLand { card: land.id })
    }

    fn cast_or_mana(
        &mut self,
        view: &GameView,
        definition: &'static str,
        cost: &ManaCost,
        target: Option<Target>,
    ) -> Option<PolicyAction> {
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(definition))?;
        if let Some(mana_selection) = select_payment(view, cost) {
            if definition == GATHER_COURAGE_ID {
                self.courage_offered = true;
            }
            return Some(PolicyAction::CastWithPayment {
                request: CastRequest {
                    card: card.id,
                    targets: target.into_iter().collect(),
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection,
            });
        }
        make_mana(view, cost)
    }
}

impl CodePolicy for PositionPilot {
    fn id(&self) -> &'static str {
        "rav.privileged-position-hexproof-pilot.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(WATCHWOLF_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect(),
            };
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
            || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(land) = self.play_land(view) {
            return land;
        }
        let wolf = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some(WATCHWOLF_ID))
            .map(|card| card.id);
        let position = view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some(CARD_ID));
        if wolf.is_none() {
            let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
            if let Some(action) = self.cast_or_mana(view, WATCHWOLF_ID, &cost, None) {
                return action;
            }
        }
        if wolf.is_some() && !position {
            let cost = position_cost();
            if let Some(action) = self.cast_or_mana(view, CARD_ID, &cost, None) {
                return action;
            }
        }
        if position
            && !self.courage_offered
            && let Some(target) = wolf
        {
            let cost = ManaCost::with_colors(0, [Color::Green]);
            if let Some(action) = self.cast_or_mana(
                view,
                GATHER_COURAGE_ID,
                &cost,
                Some(Target::Permanent(target)),
            ) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

struct PositionOpponent {
    challenge_offered: bool,
}
impl PositionOpponent {
    fn new() -> Self {
        Self {
            challenge_offered: false,
        }
    }
}

impl CodePolicy for PositionOpponent {
    fn id(&self) -> &'static str {
        "rav.privileged-position-putrefy-opponent.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != PlayerId(1) || view.decision_player != PlayerId(1) {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == PlayerId(1)
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers { attackers: vec![] };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != PlayerId(1)
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.priority != PlayerId(1) || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0
            && view.active_player == PlayerId(1)
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(SWAMP_ID))
                .or_else(|| {
                    view.hand
                        .iter()
                        .find(|card| card.definition == Some(FOREST_ID))
                })
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        let position_present = view
            .opponent_battlefield
            .iter()
            .any(|card| card.definition == Some(CARD_ID));
        let target = view
            .opponent_battlefield
            .iter()
            .find(|card| card.definition == Some(WATCHWOLF_ID))
            .map(|card| card.id);
        if !self.challenge_offered
            && position_present
            && let Some(target) = target
        {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(PUTREFY_ID))
            {
                let cost = ManaCost::with_colors(1, [Color::Black, Color::Green]);
                if let Some(mana_selection) = select_payment(view, &cost) {
                    self.challenge_offered = true;
                    return PolicyAction::CastWithPayment {
                        request: CastRequest {
                            card: card.id,
                            targets: vec![Target::Permanent(target)],
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
        if view.active_player != PlayerId(1)
            || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            return PolicyAction::PassPriority;
        }
        PolicyAction::PassPriority
    }
}

fn position_cost() -> ManaCost {
    ManaCost::with_hybrid(
        2,
        [],
        [
            HybridManaSymbol {
                first: Color::Green,
                second: Color::White,
            },
            HybridManaSymbol {
                first: Color::Green,
                second: Color::White,
            },
            HybridManaSymbol {
                first: Color::Green,
                second: Color::White,
            },
        ],
    )
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
