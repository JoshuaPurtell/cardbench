//! Full-game diagnostic authority for RAV Junktroller #264.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, Keyword, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, TargetRequirement, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_attachment_bindings, rav_attachment_triggered_ability_bindings as attachment_triggers,
    rav_basic_land_type_bindings, rav_land_entry_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings, rav_static_continuous_effect_bindings,
    rav_triggered_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-JUNKTROLLER";
const ABILITY_ID: &str = "tap-target-graveyard-card-to-owners-library-bottom";
const DIZZY_ID: &str = "RAV-DIZZY-SPELL";
const MOROII_ID: &str = "RAV-MOROII";
const MAX_TURNS: u32 = 60;
const MAX_POLICY_MOVES: u32 = 8_000;
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/junktroller.rs"]
mod candidate_junktroller;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Junktroller result: {error}");
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
            eprintln!("Junktroller PvP authority failed: {error}");
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
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongActivationCost,
    NoTapCost,
    WrongTargetScope,
    WrongEffect,
    WrongCastCost,
    NoDefender,
    WrongToughness,
}

fn candidate_values(mutation: Mutation) -> (CardDefinition, ActivatedAbilityBinding) {
    let mut definition = candidate_junktroller::definition();
    let mut binding = candidate_junktroller::activated_ability();
    match mutation {
        Mutation::None => {}
        Mutation::WrongActivationCost => binding.ability.mana_cost = ManaCost::new(1),
        Mutation::NoTapCost => binding.ability.tap_cost = false,
        Mutation::WrongTargetScope => {
            binding.ability.targets = vec![TargetRequirement::OwnGraveyardCard]
        }
        Mutation::WrongEffect => {
            binding.ability.effects = vec![Effect::ExileUpToTargetGraveyardCards { maximum: 1 }]
        }
        Mutation::WrongCastCost => definition.mana_cost = ManaCost::new(3),
        Mutation::NoDefender => definition.keywords.clear(),
        Mutation::WrongToughness => definition.toughness = Some(5),
    }
    (definition, binding)
}

fn validate_candidate_source() -> Result<(), String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../sets/ravnica_city_of_guilds/src/cards/junktroller.rs");
    let source = std::fs::read_to_string(&path)
        .map_err(|error| format!("could not read Junktroller candidate module: {error}"))?;
    validate_candidate_source_text(&source)
}

// Whitespace and comments are normalized away, while the bytes in every literal remain part of
// this pinned source contract. Keep this independent of the candidate file so changing that file
// cannot silently redefine the authority's expected implementation.
const CANONICAL_CANDIDATE_SOURCE: &str = "usestd::collections::BTreeSet;usecardbench_magic_engine::{ActivatedAbility,ActivatedAbilityBinding,CardDefinition,CardType,Effect,Keyword,ManaCost,};#[must_use]pub(crate)fndefinition()->CardDefinition{CardDefinition{id:\"RAV-JUNKTROLLER\",name:\"Junktroller\",set_code:\"RAV\",mana_cost:ManaCost::new(4),colors:BTreeSet::new(),mana_colors:BTreeSet::new(),card_types:BTreeSet::from([CardType::Artifact,CardType::Creature]),is_basic_land:false,supported_rules:&[\"full-rules-fidelity\",\"colorless-artifact-creature-casting\",\"base-characteristics\",\"defender\",\"printed-tap-graveyard-card-to-owner-library-bottom\",],power:Some(0),toughness:Some(6),keywords:vec![Keyword::Defender],effects:vec![],}}#[must_use]pub(crate)fnactivated_ability()->ActivatedAbilityBinding{ActivatedAbilityBinding{card_definition:\"RAV-JUNKTROLLER\",ability:ActivatedAbility{id:\"tap-target-graveyard-card-to-owners-library-bottom\",mana_cost:ManaCost::new(0),tap_cost:true,sorcery_speed:false,additional_tap_creatures:0,sacrifice_source:false,sacrifice_creatures:0,sacrifice_lands:0,discard_cards:0,targets:vec![cardbench_magic_engine::TargetRequirement::GraveyardCard],effects:vec![Effect::PutTargetGraveyardCardOnOwnersLibraryBottom],},}}";
const INJECTED_SIDE_EFFECT: &str =
    "ifstd::process::id()>0{println!(\"injected candidate side effect\");std::process::exit(0);}";

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let file = syn::parse_file(source)
        .map_err(|_| "Junktroller candidate module is not valid Rust source".to_owned())?;
    if normalize_rust_source(source)? != CANONICAL_CANDIDATE_SOURCE {
        return Err(
            "Junktroller candidate differs from its canonical normalized source".to_owned(),
        );
    }
    if !candidate_ast_has_literal_shape(&file) {
        return Err("Junktroller candidate AST is outside the literal-only shape".to_owned());
    }
    Ok(())
}

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
        // Candidate data uses ordinary quoted strings. Preserve a quoted char literal too; an
        // apostrophe with no closing quote on this line is a Rust lifetime, not a literal.
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
        return Err("Junktroller candidate has an unterminated literal or comment".to_owned());
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

fn candidate_ast_has_literal_shape(file: &syn::File) -> bool {
    if file
        .attrs
        .iter()
        .any(|attribute| !attribute.path().is_ident("doc"))
        || file.items.len() != 4
        || !matches!(file.items[0], syn::Item::Use(_))
        || !matches!(file.items[1], syn::Item::Use(_))
    {
        return false;
    }
    let (Some(syn::Item::Fn(definition)), Some(syn::Item::Fn(ability))) =
        (file.items.get(2), file.items.get(3))
    else {
        return false;
    };
    function_has_literal_shape(
        definition,
        "definition",
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
    ) && function_has_literal_shape(
        ability,
        "activated_ability",
        "ActivatedAbilityBinding",
        &["card_definition", "ability"],
    ) && function_tail_struct(ability).is_some_and(|binding| {
        struct_field(binding, "ability").is_some_and(|ability| {
            let syn::Expr::Struct(ability) = ability else {
                return false;
            };
            struct_has_shape(
                ability,
                "ActivatedAbility",
                &[
                    "id",
                    "mana_cost",
                    "tap_cost",
                    "sorcery_speed",
                    "additional_tap_creatures",
                    "sacrifice_source",
                    "sacrifice_creatures",
                    "sacrifice_lands",
                    "discard_cards",
                    "targets",
                    "effects",
                ],
            )
        })
    })
}

fn function_has_literal_shape(
    function: &syn::ItemFn,
    name: &str,
    return_type: &str,
    fields: &[&str],
) -> bool {
    let visibility_is_pub_crate = match &function.vis {
        syn::Visibility::Restricted(visibility) => {
            visibility.path.is_ident("crate") && visibility.in_token.is_none()
        }
        _ => false,
    };
    let signature = &function.sig;
    visibility_is_pub_crate
        && signature.ident == name
        && signature.inputs.is_empty()
        && signature.generics.params.is_empty()
        && signature.generics.where_clause.is_none()
        && signature.constness.is_none()
        && signature.asyncness.is_none()
        && signature.unsafety.is_none()
        && signature.abi.is_none()
        && signature.variadic.is_none()
        && return_type_is(&signature.output, return_type)
        && function.attrs.len() == 1
        && function.attrs[0].path().is_ident("must_use")
        && function_tail_struct(function)
            .is_some_and(|expression| struct_has_shape(expression, return_type, fields))
}

fn return_type_is(output: &syn::ReturnType, expected: &str) -> bool {
    match output {
        syn::ReturnType::Type(_, ty) => match &**ty {
            syn::Type::Path(path) if path.qself.is_none() => path
                .path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == expected),
            _ => false,
        },
        syn::ReturnType::Default => false,
    }
}

fn function_tail_struct(function: &syn::ItemFn) -> Option<&syn::ExprStruct> {
    if function.block.stmts.len() != 1 {
        return None;
    }
    match function.block.stmts.first()? {
        syn::Stmt::Expr(syn::Expr::Struct(expression), None) => Some(expression),
        _ => None,
    }
}

fn struct_field<'a>(expression: &'a syn::ExprStruct, name: &str) -> Option<&'a syn::Expr> {
    expression
        .fields
        .iter()
        .find_map(|field| match &field.member {
            syn::Member::Named(ident) if ident == name => Some(&field.expr),
            _ => None,
        })
}

fn struct_has_shape(expression: &syn::ExprStruct, name: &str, fields: &[&str]) -> bool {
    let actual: Vec<_> = expression
        .fields
        .iter()
        .map(|field| match &field.member {
            syn::Member::Named(ident) => Some(ident.to_string()),
            syn::Member::Unnamed(_) => None,
        })
        .collect();
    let expected: Vec<_> = fields.iter().map(|field| (*field).to_owned()).collect();
    expression
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == name)
        && actual == expected.into_iter().map(Some).collect::<Vec<_>>()
        && expression.rest.is_none()
        && expression
            .fields
            .iter()
            .all(|field| literal_expression(&field.expr))
}

fn literal_expression(expression: &syn::Expr) -> bool {
    match expression {
        syn::Expr::Lit(_) | syn::Expr::Path(_) => true,
        syn::Expr::Call(call) => {
            matches!(&*call.func, syn::Expr::Path(_)) && call.args.iter().all(literal_expression)
        }
        syn::Expr::Array(array) => array.elems.iter().all(literal_expression),
        syn::Expr::Reference(reference) => {
            reference.mutability.is_none() && literal_expression(&reference.expr)
        }
        syn::Expr::Struct(structure) => {
            structure.rest.is_none()
                && structure
                    .fields
                    .iter()
                    .all(|field| literal_expression(&field.expr))
        }
        syn::Expr::Macro(mac) => mac.mac.path.is_ident("vec"),
        _ => false,
    }
}

fn injected_side_effect_candidate_rejected() -> bool {
    let signature = "#[must_use]pub(crate)fndefinition()->CardDefinition{";
    if CANONICAL_CANDIDATE_SOURCE.matches(signature).count() != 1 {
        return false;
    }
    let injected = CANONICAL_CANDIDATE_SOURCE.replacen(
        signature,
        &format!("{signature}{INJECTED_SIDE_EFFECT}"),
        1,
    );
    let Ok(parsed) = syn::parse_file(&injected) else {
        return false;
    };
    normalize_rust_source(&injected)
        .is_ok_and(|normalized| normalized != CANONICAL_CANDIDATE_SOURCE)
        && !candidate_ast_has_literal_shape(&parsed)
        && validate_candidate_source_text(&injected).is_err()
}

// This dead-code fixture type-checks the exact injected control-flow statement used by the
// attestation. It is never called; the validator must reject the corresponding candidate source
// before the candidate module's functions can execute.
#[allow(dead_code)]
fn compile_valid_injected_side_effect_candidate_fixture() -> CardDefinition {
    if std::process::id() > 0 {
        println!("injected candidate side effect");
        std::process::exit(0);
    }
    expected_definition()
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: "Junktroller",
        set_code: "RAV",
        mana_cost: ManaCost::new(4),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact, CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colorless-artifact-creature-casting",
            "base-characteristics",
            "defender",
            "printed-tap-graveyard-card-to-owner-library-bottom",
        ],
        power: Some(0),
        toughness: Some(6),
        keywords: vec![Keyword::Defender],
        effects: vec![],
    }
}

fn expected_binding() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: CARD_ID,
        ability: ActivatedAbility {
            id: ABILITY_ID,
            mana_cost: ManaCost::new(0),
            tap_cost: true,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![TargetRequirement::GraveyardCard],
            effects: vec![Effect::PutTargetGraveyardCardOnOwnersLibraryBottom],
        },
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    junktroller: Option<ObjectId>,
    finisher: Option<ObjectId>,
    target: Option<ObjectId>,
    target_owner: Option<PlayerId>,
    target_zone_at_activation: Option<Zone>,
    target_zone_after_resolution: Option<Zone>,
    activation_targets: Vec<Target>,
    source_tapped_at_activation: bool,
    activation_payment: Option<ManaCost>,
    cast_payment_count: usize,
    source_incarnation_at_activation: Option<u64>,
    source_entered_turn: Option<u32>,
    activation_turn: Option<u32>,
    owner_library_before: Vec<ObjectId>,
    owner_library_after: Vec<ObjectId>,
    controller_library_before: Vec<ObjectId>,
    controller_library_after: Vec<ObjectId>,
    library_sizes: [usize; 2],
    junktroller_definition: CardDefinition,
    activation_binding: ActivatedAbilityBinding,
    attack_receipts: usize,
    life: [i64; 2],
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    let source_validation = json!({
        "normalized_literal_source_contract_matched": true,
        "ast_literal_shape_matched": true,
        "compile_valid_injected_side_effect_candidate_rejected": injected_side_effect_candidate_rejected()
    });
    if source_validation["compile_valid_injected_side_effect_candidate_rejected"] != true {
        return Err("Junktroller validator accepted an injected side-effect candidate".to_owned());
    }
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
        return Err("RAV catalog denominator differs from the source contract".to_owned());
    }
    let (seed, capture) = find_trace()?;
    let trace = validate_trace(&capture, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if capture != replay || project_events(&capture.events) != project_events(&replay.events) {
        return Err("Junktroller shuffled-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "activation_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongActivationCost)?,
        "tap_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::NoTapCost)?,
        "target_scope_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongTargetScope)?,
        "library_bottom_effect_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongEffect)?,
        "cast_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongCastCost)?,
        "defender_mutation_detected_in_game": mutation_detected(seed, Mutation::NoDefender)?,
        "toughness_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongToughness)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "activation_cost_mutation_detected_in_game",
        "tap_cost_mutation_detected_in_game",
        "target_scope_mutation_detected_in_game",
        "library_bottom_effect_mutation_detected_in_game",
        "cast_cost_mutation_detected_in_game",
        "defender_mutation_detected_in_game",
        "toughness_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Junktroller semantic mutation escaped the PvP trace: {key}"
            ));
        }
    }
    let definitions = card_definitions();
    let breakdown: Vec<_> = catalog
        .iter()
        .map(|card| {
            let id = definitions
                .iter()
                .find(|definition| definition.name == card.name)
                .map(|definition| definition.id)
                .unwrap_or("");
            json!({
                "collector_number":card.collector_number,
                "name":card.name,
                "definition_id":if card.name == "Junktroller" { CARD_ID } else { id },
                "engine_status":if card.semantic_status.is_executable() { "executable_compatibility_slice" } else { "catalog_only" },
                "trace_status":if card.name == "Junktroller" { "passed" } else { "not_qualified" },
                "qualification":if card.name == "Junktroller" { "unreviewed_diagnostic" } else { "not_evaluated" },
                "credited":false
            })
        })
        .collect();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,
        "expansion":"RAV",
        "card":{"collector_number":264,"definition_id":CARD_ID,"name":"Junktroller"},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":["117 Priority","302.6 Creature summoning sickness","400.7 Zone changes","401.2 Library order","601.2 Casting and payment","602.1-602.2 Activated abilities","608.2 Ability resolution"],
            "source_contract":"RAV Junktroller #264 is a 0/6 artifact creature with defender and a tap-only activated ability that moves target card from a graveyard to the bottom of its owner's library. The printed card scan contains no mana symbol before the tap symbol; the instruction contains no shuffle.",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true
        },
        "passed":true,
        "pilot_card_passed":true,
        "deterministic_replay":true,
        "cards_passed":1,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":printings.len(),
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":"unreviewed_diagnostic",
        "candidate_source_validation":source_validation,
        "mutation_sensitivity":mutations,
        "traces":[trace],
        "per_card_breakdown":breakdown
    }))
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_JUNKTROLLER_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_JUNKTROLLER_SEED must be an unsigned integer")?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    let mut last_error = String::new();
    for seed in 0..=MAX_TRACE_SEED {
        match run_match(seed, Mutation::None)
            .and_then(|capture| validate_trace(&capture, seed).map(|_| capture))
        {
            Ok(capture) => return Ok((seed, capture)),
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "no bounded seed produced a valid Junktroller full-game trace: {last_error}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Err(_) => Ok(true),
        Ok(capture) => Ok(validate_trace(&capture, seed).is_err()),
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let (definition, binding) = candidate_values(mutation);
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Junktroller definition is missing from the RAV registry")?;
    *registered = definition.clone();
    let mut abilities = rav_activated_ability_bindings();
    let ability_index = abilities
        .iter()
        .position(|candidate| {
            candidate.card_definition == CARD_ID && candidate.ability.id == ABILITY_ID
        })
        .ok_or("Junktroller activation is missing from the RAV registry")?;
    abilities[ability_index] = binding.clone();
    let mut triggers = rav_triggered_ability_bindings();
    triggers.extend(attachment_triggers());
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
        triggers,
        rav_static_continuous_effect_bindings(),
        rav_land_entry_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_attachment_bindings(rav_attachment_bindings())
        .map_err(|error| error.to_string())?;

    let p0_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: MOROII_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-ISLAND".into(),
                count: 26,
            },
            DeckEntry {
                card: "RAV-SWAMP".into(),
                count: 26,
            },
        ],
        sideboard: vec![],
    };
    let p1_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: DIZZY_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-ISLAND".into(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    for (label, deck) in [("Junktroller deck", &p0_deck), ("opponent deck", &p1_deck)] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{label} has {count} cards, expected sixty"));
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_POLICY_MOVES,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &p0_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &p1_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut policies = [
        JunktrollerPolicy::new(PlayerId(0)),
        JunktrollerPolicy::new(PlayerId(1)),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut junktroller = None;
    let mut finisher = None;
    let mut target = None;
    let mut target_owner = None;
    let mut target_zone_at_activation = None;
    let mut target_zone_after_resolution = None;
    let mut activation_targets = Vec::new();
    let mut source_tapped_at_activation = false;
    let mut activation_payment = None;
    let mut cast_payment_count = 0;
    let mut source_incarnation_at_activation = None;
    let mut source_entered_turn = None;
    let mut activation_turn = None;
    let mut owner_library_before = Vec::new();
    let mut owner_library_after = Vec::new();
    let mut controller_library_before = Vec::new();
    let mut controller_library_after = Vec::new();
    let mut library_sizes = [0; 2];
    let mut attack_receipts = 0;
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || accepted >= MAX_POLICY_MOVES {
            return Err(format!(
                "Junktroller duel exceeded its bound at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("policy seat is unavailable")?;
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
        let old_event_count = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Junktroller policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        for event in &game.event_log[old_event_count..] {
            match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|card| card.id == CARD_ID) =>
                {
                    junktroller = Some(*card)
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|card| card.id == MOROII_ID) =>
                {
                    finisher = Some(*card)
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    cast_payment_count = colors.len()
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Battlefield,
                } if Some(*card) == junktroller => {
                    policies[0].junktroller = Some(*card);
                    policies[0].junktroller_entered_turn = Some(game.turn);
                    source_entered_turn = Some(game.turn);
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Graveyard,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|card| card.id == DIZZY_ID) =>
                {
                    target = Some(*card);
                    target_owner = game.object(*card).ok().map(|object| object.owner);
                    policies[0].target = Some(*card);
                }
                GameEvent::AbilityManaPaid {
                    player: PlayerId(0),
                    source,
                    ability: ABILITY_ID,
                    mana_cost,
                } if Some(*source) == junktroller => activation_payment = Some(mana_cost.clone()),
                GameEvent::AbilityActivated {
                    player: PlayerId(0),
                    source,
                    definition: CARD_ID,
                    ability: ABILITY_ID,
                    source_incarnation,
                } if Some(*source) == junktroller => {
                    source_incarnation_at_activation = Some(*source_incarnation);
                    activation_turn = Some(game.turn);
                    activation_targets = game
                        .stack
                        .iter()
                        .rev()
                        .find(|stack| stack.card == *source && stack.ability_id == Some(ABILITY_ID))
                        .map(|stack| stack.targets.clone())
                        .unwrap_or_default();
                    target_zone_at_activation = target.and_then(|card| game.zone_of(card));
                    target_owner =
                        target.and_then(|card| game.object(card).ok().map(|object| object.owner));
                    source_tapped_at_activation =
                        game.object(*source).is_ok_and(|object| object.tapped);
                    if let Some(owner) = target_owner {
                        owner_library_before = game
                            .player(owner)
                            .map_err(|error| error.to_string())?
                            .library
                            .clone();
                        library_sizes[owner.0] = owner_library_before.len();
                    }
                    controller_library_before = game
                        .player(PlayerId(0))
                        .map_err(|error| error.to_string())?
                        .library
                        .clone();
                    library_sizes[0] = controller_library_before.len();
                }
                GameEvent::AbilityResolved {
                    source,
                    ability: ABILITY_ID,
                    ..
                } if Some(*source) == junktroller => {
                    target_zone_after_resolution = target.and_then(|card| game.zone_of(card));
                    if let Some(owner) = target_owner {
                        owner_library_after = game
                            .player(owner)
                            .map_err(|error| error.to_string())?
                            .library
                            .clone();
                    }
                    controller_library_after = game
                        .player(PlayerId(0))
                        .map_err(|error| error.to_string())?
                        .library
                        .clone();
                }
                GameEvent::AttackersDeclared { .. } => attack_receipts += 1,
                _ => {}
            }
        }
    }
    let life = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    Ok(Captured {
        events: game.event_log.clone(),
        terminal: game.winner().map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({}))", seat.0),
        ),
        winner: game.winner().map(|seat| seat.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        junktroller,
        finisher,
        target,
        target_owner,
        target_zone_at_activation,
        target_zone_after_resolution,
        activation_targets,
        source_tapped_at_activation,
        activation_payment,
        cast_payment_count,
        source_incarnation_at_activation,
        source_entered_turn,
        activation_turn,
        owner_library_before,
        owner_library_after,
        controller_library_before,
        controller_library_after,
        library_sizes,
        junktroller_definition: candidate_values(mutation).0,
        activation_binding: candidate_values(mutation).1,
        attack_receipts,
        life,
    })
}

struct JunktrollerPolicy {
    player: PlayerId,
    junktroller: Option<ObjectId>,
    junktroller_entered_turn: Option<u32>,
    target: Option<ObjectId>,
    cast_junktroller: bool,
    cast_moroii: bool,
    activated: bool,
    cast_dizzy: bool,
}

impl JunktrollerPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            junktroller: None,
            junktroller_entered_turn: None,
            target: None,
            cast_junktroller: false,
            cast_moroii: false,
            activated: false,
            cast_dizzy: false,
        }
    }

    fn total_mana(view: &GameView) -> u16 {
        Color::MANA_ALL
            .into_iter()
            .map(|color| u16::from(view.mana_pool.amount(color)))
            .sum()
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

    fn mana_source(view: &GameView, preferred: Option<Color>) -> Option<PolicyAction> {
        let land = view
            .own_battlefield
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land)
                    && !card.tapped
                    && preferred.is_none_or(|color| card.mana_colors.contains(&color))
            })
            .or_else(|| {
                view.own_battlefield
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land) && !card.tapped)
            })?;
        let color = preferred
            .filter(|color| land.mana_colors.contains(color))
            .or_else(|| land.mana_colors.iter().next().copied())?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }

    fn cast_or_mana(
        &mut self,
        view: &GameView,
        definition: &'static str,
        cost: ManaCost,
        targets: Vec<Target>,
    ) -> Option<PolicyAction> {
        if let Some(mana_selection) = Self::payment(view, &cost) {
            let card = view
                .hand
                .iter()
                .find(|card| card.definition == Some(definition))?;
            match definition {
                CARD_ID => self.cast_junktroller = true,
                MOROII_ID => self.cast_moroii = true,
                DIZZY_ID => self.cast_dizzy = true,
                _ => {}
            }
            return Some(PolicyAction::CastWithPayment {
                request: CastRequest {
                    card: card.id,
                    targets,
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection,
            });
        }
        let missing = cost
            .colored
            .iter()
            .find(|color| view.mana_pool.amount(**color) == 0)
            .copied()
            .or_else(|| {
                (Self::total_mana(view) < u16::from(cost.generic) + cost.colored.len() as u16)
                    .then_some(cost.colored.first().copied().unwrap_or(Color::Blue))
            });
        Self::mana_source(view, missing)
    }
}

impl CodePolicy for JunktrollerPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.junktroller-library-bottom.v1"
        } else {
            "rav.junktroller-dizzy-spell-opponent.v1"
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
                    .filter(|card| card.can_attack && card.card_types.contains(&CardType::Creature))
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
            || !view.step.is_main()
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
        if self.player == PlayerId(1) {
            if !self.cast_dizzy {
                if let Some(target) = view
                    .opponent_battlefield
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
                {
                    if view
                        .hand
                        .iter()
                        .any(|card| card.definition == Some(DIZZY_ID))
                    {
                        if let Some(action) = self.cast_or_mana(
                            view,
                            DIZZY_ID,
                            ManaCost::with_colors(0, [Color::Blue]),
                            vec![Target::Permanent(target.id)],
                        ) {
                            return action;
                        }
                    }
                }
            }
            return PolicyAction::PassPriority;
        }
        if !self.cast_junktroller
            && view
                .hand
                .iter()
                .any(|card| card.definition == Some(CARD_ID))
        {
            if let Some(action) = self.cast_or_mana(view, CARD_ID, ManaCost::new(4), vec![]) {
                return action;
            }
        }
        if let (Some(source), Some(target), Some(entered_turn)) =
            (self.junktroller, self.target, self.junktroller_entered_turn)
            && !self.activated
            && view.turn > entered_turn
            && view
                .own_battlefield
                .iter()
                .any(|card| card.id == source && !card.tapped)
        {
            let cost = ManaCost::new(0);
            if Self::payment(view, &cost).is_some() {
                self.activated = true;
                return PolicyAction::ActivateAbility {
                    activation: AbilityActivation {
                        source,
                        ability_id: ABILITY_ID,
                        sacrifice_sources: vec![],
                        additional_tap_creatures: vec![],
                        discard_cards: vec![],
                        targets: vec![Target::Permanent(target)],
                    },
                };
            }
            if let Some(action) = Self::mana_source(view, None) {
                return action;
            }
        }
        if !self.cast_moroii
            && view
                .hand
                .iter()
                .any(|card| card.definition == Some(MOROII_ID))
        {
            if let Some(action) = self.cast_or_mana(
                view,
                MOROII_ID,
                ManaCost::with_colors(2, [Color::Blue, Color::Black]),
                vec![],
            ) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(capture: &Captured, seed: u64) -> Result<Value, String> {
    if capture.terminal != "Winner(PlayerId(0))"
        || capture.winner != Some(0)
        || capture.rejected != 0
    {
        return Err("Junktroller duel did not reach a clean seat-zero win".to_owned());
    }
    if capture.junktroller_definition != expected_definition()
        || capture.activation_binding != expected_binding()
    {
        return Err(
            "Junktroller identity or activated ability differs from the printed source contract"
                .to_owned(),
        );
    }
    let source = capture.junktroller.ok_or("Junktroller was not cast")?;
    let target = capture
        .target
        .ok_or("opponent Dizzy Spell did not reach its graveyard")?;
    let target_moves: Vec<_> = capture
        .events
        .iter()
        .enumerate()
        .filter(|(_, event)| matches!(event, GameEvent::CardMoved { card, .. } if *card == target))
        .map(|(index, event)| format!("{index}:{event:?}"))
        .collect();
    if capture.target_owner != Some(PlayerId(1))
        || capture.target_zone_at_activation != Some(Zone::Graveyard)
        || capture.target_zone_after_resolution != Some(Zone::Library)
        || capture.activation_targets != [Target::Permanent(target)]
        || !capture.source_tapped_at_activation
        || capture.activation_payment.is_some()
        || capture.source_incarnation_at_activation.is_none()
        || capture.cast_payment_count != 4
        || capture.source_entered_turn.is_none_or(|entered| {
            capture
                .activation_turn
                .is_none_or(|activated| activated <= entered)
        })
        || capture.owner_library_after.len() != capture.owner_library_before.len() + 1
        || capture.owner_library_after.first() != Some(&target)
        || capture.owner_library_after[1..] != capture.owner_library_before[..]
        || capture.controller_library_after != capture.controller_library_before
        || capture.attack_receipts == 0
    {
        return Err(format!(
            "Junktroller did not complete the tap-only graveyard-to-library-bottom operation: target={target:?}, owner={:?}, zone={:?}->{:?}, activation={:?}, unexpected_mana_payment={:?}, tapped={} incarnation={:?} cast_colors={} entered_turn={:?} activation_turn={:?} attacks={} owner_library_before={} owner_library_after={} contains_target={} bottom_first={} owner_order_preserved={} before_head={:?} after_head={:?} controller_unchanged={} controller_library_before={} controller_library_after={} target_moves={target_moves:?}",
            capture.target_owner,
            capture.target_zone_at_activation,
            capture.target_zone_after_resolution,
            capture.activation_targets,
            capture.activation_payment,
            capture.source_tapped_at_activation,
            capture.source_incarnation_at_activation,
            capture.cast_payment_count,
            capture.source_entered_turn,
            capture.activation_turn,
            capture.attack_receipts,
            capture.owner_library_before.len(),
            capture.owner_library_after.len(),
            capture.owner_library_after.contains(&target),
            capture.owner_library_after.first() == Some(&target),
            capture.owner_library_after.get(1..) == Some(capture.owner_library_before.as_slice()),
            capture
                .owner_library_before
                .iter()
                .take(4)
                .collect::<Vec<_>>(),
            capture
                .owner_library_after
                .iter()
                .take(4)
                .collect::<Vec<_>>(),
            capture.controller_library_after == capture.controller_library_before,
            capture.controller_library_before.len(),
            capture.controller_library_after.len()
        ));
    }
    let events = &capture.events;
    let cast_paid = event_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == source && colors.len() == 4))
        .ok_or("Junktroller exact {4} cast payment receipt missing")?;
    let cast = event_index_after(events, cast_paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == source)).ok_or("Junktroller SpellCast receipt missing")?;
    let cast_resolved = event_index_after(
        events,
        cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == source),
    )
    .ok_or("Junktroller SpellResolved receipt missing")?;
    let entered = event_index_after(events, cast_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == source)).ok_or("Junktroller battlefield entry missing")?;
    let dizzy_cast = event_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == target)).ok_or("opponent Dizzy Spell cast receipt missing")?;
    let dizzy_resolved = event_index_after(
        events,
        dizzy_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == target),
    )
    .ok_or("opponent Dizzy Spell resolution missing")?;
    let target_yard = event_index_after(events, dizzy_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == target)).ok_or("opponent-owned target did not enter its graveyard")?;
    if events.iter().any(|event| matches!(event, GameEvent::AbilityManaPaid { source: object, ability: ABILITY_ID, .. } if *object == source)) {
        return Err("Junktroller tap-only ability unexpectedly generated an AbilityManaPaid receipt".to_owned());
    }
    let activated = event_index_after(events, target_yard, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source: object, definition: CARD_ID, ability: ABILITY_ID, .. } if *object == source)).ok_or("Junktroller tap-only activation receipt missing")?;
    let moved = event_index_after(events, activated, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Library } if *card == target)).ok_or("exact target did not move to a library")?;
    let resolved = event_index_after(events, moved, |event| matches!(event, GameEvent::AbilityResolved { source: object, ability: ABILITY_ID, .. } if *object == source)).ok_or("Junktroller ability did not resolve")?;
    let terminal = event_index_after(events, resolved, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal win did not follow Junktroller resolution")?;
    let finisher = capture.finisher.ok_or("Moroii finisher was not cast")?;
    let finisher_cast = event_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == finisher)).ok_or("Moroii finisher cast receipt missing")?;
    let finisher_resolved = event_index_after(
        events,
        finisher_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == finisher),
    )
    .ok_or("Moroii finisher did not resolve")?;
    let finisher_entered = event_index_after(events, finisher_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == finisher)).ok_or("Moroii finisher did not enter the battlefield")?;
    let finish_attack = event_index_after(events, resolved, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&finisher))).ok_or("Moroii did not attack after Junktroller resolved")?;
    let finish_blockers = event_index_after(events, finish_attack, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if assignments.is_empty())).ok_or("opponent did not declare the unblocked Moroii attack")?;
    let finish_damage = event_index_after(events, finish_blockers, |event| matches!(event, GameEvent::DamageDealtToPlayer { source: attacker, player: PlayerId(1), amount: 4 } if *attacker == finisher)).ok_or("Moroii did not deal combat damage after Junktroller resolved")?;
    if events.iter().any(|event| matches!(event, GameEvent::AttackersDeclared { attackers, .. } if attackers.contains(&source))) {
        return Err("Junktroller's Defender keyword did not prevent it from attacking".to_owned());
    }
    let bottom_shuffles = events.iter().enumerate().any(|(index, event)| {
        index > activated
            && index < resolved
            && matches!(
                event,
                GameEvent::LibraryShuffled {
                    player: PlayerId(1),
                    ..
                }
            )
    });
    if bottom_shuffles {
        return Err(
            "Junktroller ability unexpectedly shuffled the target owner's library".to_owned(),
        );
    }
    if !(cast_paid < cast
        && cast < cast_resolved
        && cast_resolved < entered
        && entered < target_yard
        && target_yard < activated
        && activated < moved
        && moved < resolved
        && finisher_cast < finisher_resolved
        && finisher_resolved < finisher_entered
        && finisher_entered < finish_attack
        && finish_attack < finish_blockers
        && finish_blockers < finish_damage
        && finish_damage < terminal)
    {
        return Err("Junktroller cast, target setup, activation, library move, resolution, and terminal events are out of order".to_owned());
    }
    let selected = BTreeSet::from([
        cast_paid,
        cast,
        cast_resolved,
        entered,
        dizzy_cast,
        dizzy_resolved,
        target_yard,
        activated,
        moved,
        resolved,
        finisher_cast,
        finisher_resolved,
        finisher_entered,
        finish_attack,
        finish_blockers,
        finish_damage,
        terminal,
    ]);
    let projected: Vec<_> = project_events(events)
        .into_iter()
        .enumerate()
        .filter_map(|(index, event)| selected.contains(&index).then_some(event))
        .collect();
    Ok(json!({
        "id":"rav-junktroller-graveyard-card-to-owner-library-bottom-full-game",
        "seed":seed,
        "terminal":capture.terminal,
        "winner_seat":capture.winner,
        "turns":capture.turns,
        "accepted_moves":capture.accepted,
        "rejected_moves":capture.rejected,
        "junktroller":{"object":source.0,"collector_number":264,"definition_id":CARD_ID,"mana_cost":"{4}","power":0,"toughness":6,"defender":true},
        "ability":{"id":ABILITY_ID,"mana_cost":"none","mana_payment_event":false,"tap_cost_paid":capture.source_tapped_at_activation,"target_object":target.0,"target_definition":DIZZY_ID,"target_owner_seat":1,"target_zone_at_activation":"Graveyard","result_zone":"Library","bottom_index":0,"owner_library_size_before":capture.owner_library_before.len(),"owner_library_size_after":capture.owner_library_after.len(),"other_owner_library_order_preserved":true,"controller_library_unchanged":true,"shuffle_after_activation":false,"source_incarnation_at_activation":capture.source_incarnation_at_activation},
        "setup":{"opponent_spell_cast_and_put_in_graveyard":true,"source_entered_turn":capture.source_entered_turn,"activation_turn":capture.activation_turn,"source_waited_until_a_later_turn":true,"legal_deck_sizes":[60,60],"library_sizes_at_activation":capture.library_sizes},
        "events":projected,
        "events_digest":event_digest(events),
        "opponent_life_at_terminal":capture.life[1],
        "passed":true
    }))
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn event_index_after(
    events: &[GameEvent],
    after: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(after + 1)
        .find_map(|(index, event)| predicate(event).then_some(index))
}
fn event_digest(events: &[GameEvent]) -> String {
    let bytes = events
        .iter()
        .map(|event| format!("{event:?}\n"))
        .collect::<String>();
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes.bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}
