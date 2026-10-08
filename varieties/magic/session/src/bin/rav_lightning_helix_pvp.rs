//! Full-game RAV Lightning Helix trace authority for per-card set-PvP work.
//!
//! Each trace comes from a legal, shuffled 60-card match that reaches a clean
//! terminal result. The emitted card trace is limited to Helix's cast, its
//! target damage, controller life gain, resolution, and graveyard move.

use cardbench_magic_engine::{
    CardType, Color, GameEvent, GameView, ManaCost, ObjectId, PlayerId, PolicyAction, Target, Zone,
};
use cardbench_magic_policies::{
    BorosConvokeBurnPolicy, CodePolicy, DeckMatchConfig, SelesnyaConvokePolicy,
    run_deck_matchup_capturing,
};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_main_set_catalog,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::parse::Parser;
use syn::{Expr, Item, Lit, Meta, PathArguments, Stmt, Token, Type, UseTree, Visibility};

const P0_DECK: &str = "rav_boros_convoke_burn";
const P1_DECK: &str = "rav_selesnya_convoke";
const TASK_ID: &str = "cardbench/magic/card";
const HELIX_SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/lightning_helix.rs";
const MAX_PLAYER_TRACE_SEED: u64 = 256;
const MAX_CREATURE_TRACE_SEED: u64 = 256;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Lightning Helix PvP result: {error}");
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
            eprintln!("Lightning Helix PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_helix_identity()?;
    let printings = rav_main_set_catalog();
    let mut seen_names = BTreeSet::new();
    let catalog: Vec<CatalogCard> = printings
        .iter()
        .copied()
        .filter(|card| seen_names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
    {
        return Err(format!(
            "RAV catalog has {} names / {} printings; expected {} / {}",
            catalog.len(),
            printings.len(),
            RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
            RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        ));
    }

    let (player_seed, player) = find_target_trace(TargetBranch::Player)?;
    let player_trace = validate_trace(&player, TargetBranch::Player)?;
    let player_positions = player_trace_positions(&player, TargetBranch::Player)?;
    let player_selected = player_positions.ordered();
    let player_replay = deterministic_pair(player_seed, false)?;
    let player_replay_positions = player_trace_positions(&player_replay, TargetBranch::Player)?;
    let player_replay_selected = player_replay_positions.ordered();
    if player.result.digest != player_replay.result.digest
        || project_relevant(&player_replay.events, &player_replay_selected)
            != project_relevant(&player.events, &player_selected)
    {
        return Err("player-target replay was not deterministic".to_owned());
    }

    let (creature_seed, creature) = find_target_trace(TargetBranch::Creature)?;
    let creature_trace = validate_trace(&creature, TargetBranch::Creature)?;
    let creature_positions = player_trace_positions(&creature, TargetBranch::Creature)?;
    let creature_selected = creature_positions.ordered();
    let creature_replay = deterministic_pair(creature_seed, true)?;
    let creature_replay_positions =
        player_trace_positions(&creature_replay, TargetBranch::Creature)?;
    let creature_replay_selected = creature_replay_positions.ordered();
    if creature.result.digest != creature_replay.result.digest
        || project_relevant(&creature_replay.events, &creature_replay_selected)
            != project_relevant(&creature.events, &creature_selected)
    {
        return Err("creature-target replay was not deterministic".to_owned());
    }

    let mutation_sensitivity = json!({
        "player_damage_3_to_2_detected": detects_damage_mutation(&player, TargetBranch::Player),
        "creature_damage_3_to_2_detected": detects_damage_mutation(&creature, TargetBranch::Creature),
        "life_gain_3_to_2_detected": detects_life_gain_mutation(&player, TargetBranch::Player),
        "damage_and_gain_order_swap_detected": detects_order_mutation(&player, TargetBranch::Player),
        "sealed_mutation_authority": false
    });
    if mutation_sensitivity["player_damage_3_to_2_detected"] != true
        || mutation_sensitivity["creature_damage_3_to_2_detected"] != true
        || mutation_sensitivity["life_gain_3_to_2_detected"] != true
        || mutation_sensitivity["damage_and_gain_order_swap_detected"] != true
    {
        return Err("a Lightning Helix semantic mutation escaped the trace oracle".to_owned());
    }

    let breakdown: Vec<Value> = catalog
        .iter()
        .map(|card| card_status(card, card_definition_passed(card)))
        .collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();

    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 213, "definition_id": "RAV-LIGHTNING-HELIX", "name": "Lightning Helix"},
        "candidate": {"kind": candidate_kind(), "card_definition": "RAV-LIGHTNING-HELIX"},
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "source_contract": "RAV printed semantics; later Oracle target-domain errata are excluded from this historical set compatibility task",
            "authority_cutoff": "2026-09-23"
        },
        "candidate_kind": candidate_kind(),
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
        "mutation_sensitivity": mutation_sensitivity,
        "traces": [player_trace, creature_trace],
        "per_card_breakdown": breakdown
    }))
}

/// The submitted source is linked into this process, so accept only the small
/// declarative Rust subset needed to describe a card. This prevents a card
/// definition from printing a forged report, exiting the verifier, or reading
/// its mounted build authority at runtime.
fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(HELIX_SOURCE_PATH)
        .map_err(|error| format!("could not read Lightning Helix source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Lightning Helix source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Lightning Helix source has a non-documentation module attribute".to_owned());
    }

    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TargetRequirement"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut actual_imports = BTreeSet::new();
    let mut definition = None;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let Some(paths) = flattened_use_paths(item) else {
                    return Err("Lightning Helix source contains an unsupported import".to_owned());
                };
                for path in paths {
                    actual_imports.insert(path);
                }
            }
            Item::Fn(item) if definition.is_none() => definition = Some(item),
            _ => return Err("Lightning Helix source contains executable top-level code".to_owned()),
        }
    }
    if actual_imports != expected_imports || file.items.len() != 3 {
        return Err(
            "Lightning Helix source imports or item structure differ from the task boundary"
                .to_owned(),
        );
    }
    let definition =
        definition.ok_or_else(|| "Lightning Helix definition() is missing".to_owned())?;
    validate_definition_function(definition)
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
        UseTree::Glob(_) | UseTree::Rename(_) => false,
    }
}

fn validate_definition_function(function: &syn::ItemFn) -> Result<(), String> {
    let signature = &function.sig;
    let visibility_is_crate = matches!(
        &function.vis,
        Visibility::Restricted(visibility) if visibility.path.is_ident("crate")
    );
    let returns_card_definition = matches!(
        &signature.output,
        syn::ReturnType::Type(_, ty)
            if matches!(&**ty, Type::Path(ty) if ty.qself.is_none() && path_is(&ty.path, &["CardDefinition"]))
    );
    let attributes_are_safe = function.attrs.iter().all(|attribute| {
        is_doc_attribute(attribute)
            || attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_))
    });
    if signature.ident != "definition"
        || !visibility_is_crate
        || !signature.inputs.is_empty()
        || !signature.generics.params.is_empty()
        || signature.generics.where_clause.is_some()
        || signature.constness.is_some()
        || signature.asyncness.is_some()
        || signature.unsafety.is_some()
        || signature.abi.is_some()
        || signature.variadic.is_some()
        || !returns_card_definition
        || !attributes_are_safe
    {
        return Err(
            "Lightning Helix function must remain a plain definition() -> CardDefinition"
                .to_owned(),
        );
    }
    if function.block.stmts.len() != 1 {
        return Err("Lightning Helix definition must be a single card literal".to_owned());
    }
    let Stmt::Expr(expression, None) = &function.block.stmts[0] else {
        return Err("Lightning Helix definition contains executable statements".to_owned());
    };
    validate_card_literal(expression)
}

fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc")
        && matches!(
            &attribute.meta,
            Meta::NameValue(value)
                if matches!(&value.value, Expr::Lit(literal) if matches!(&literal.lit, Lit::Str(_)))
        )
}

fn validate_card_literal(expression: &Expr) -> Result<(), String> {
    let Expr::Struct(card) = expression else {
        return Err("Lightning Helix definition must return a CardDefinition literal".to_owned());
    };
    if !card.attrs.is_empty()
        || card.qself.is_some()
        || !path_is(&card.path, &["CardDefinition"])
        || card.rest.is_some()
    {
        return Err(
            "Lightning Helix definition must be a direct CardDefinition literal".to_owned(),
        );
    }
    let expected_fields: BTreeSet<String> = [
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
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let mut actual_fields = BTreeSet::new();
    for field in &card.fields {
        let syn::Member::Named(name) = &field.member else {
            return Err("Lightning Helix definition contains a non-named field".to_owned());
        };
        if !field.attrs.is_empty()
            || !actual_fields.insert(name.to_string())
            || !is_safe_card_expression(&field.expr)
        {
            return Err(
                "Lightning Helix definition contains a computed or duplicate field".to_owned(),
            );
        }
    }
    if actual_fields != expected_fields {
        return Err("Lightning Helix definition fields differ from CardDefinition".to_owned());
    }
    Ok(())
}

fn is_safe_card_expression(expression: &Expr) -> bool {
    if !expression_attrs_are_empty(expression) {
        return false;
    }
    match expression {
        Expr::Lit(literal) => matches!(&literal.lit, Lit::Str(_) | Lit::Int(_) | Lit::Bool(_)),
        Expr::Path(path) => {
            path.qself.is_none()
                && [
                    &["None"][..],
                    &["Color", "White"],
                    &["Color", "Red"],
                    &["CardType", "Instant"],
                    &["TargetRequirement", "PlayerOrCreature"],
                ]
                .iter()
                .any(|expected| path_is(&path.path, expected))
        }
        Expr::Array(array) => array.elems.iter().all(is_safe_card_expression),
        Expr::Reference(reference) => {
            reference.mutability.is_none() && is_safe_card_expression(&reference.expr)
        }
        Expr::Call(call) => is_safe_card_call(call),
        Expr::Macro(mac) => {
            if !path_is(&mac.mac.path, &["vec"])
                || !matches!(&mac.mac.delimiter, syn::MacroDelimiter::Bracket(_))
            {
                return false;
            }
            syn::punctuated::Punctuated::<Expr, Token![,]>::parse_terminated
                .parse2(mac.mac.tokens.clone())
                .is_ok_and(|items| items.iter().all(is_safe_card_expression))
        }
        Expr::Struct(structure) => is_safe_effect(structure),
        Expr::Paren(paren) => is_safe_card_expression(&paren.expr),
        Expr::Group(group) => is_safe_card_expression(&group.expr),
        _ => false,
    }
}

fn is_safe_card_call(call: &syn::ExprCall) -> bool {
    if !call.attrs.is_empty() {
        return false;
    }
    let Expr::Path(function) = &*call.func else {
        return false;
    };
    if function.qself.is_some() {
        return false;
    }
    if path_is(&function.path, &["BTreeSet", "new"]) {
        return call.args.is_empty();
    }
    if path_is(&function.path, &["BTreeSet", "from"]) {
        return call.args.len() == 1
            && matches!(&call.args[0], Expr::Array(array) if array.elems.iter().all(is_safe_card_expression));
    }
    if path_is(&function.path, &["ManaCost", "with_colors"]) {
        return call.args.len() == 2
            && matches!(&call.args[0], Expr::Lit(literal) if matches!(&literal.lit, Lit::Int(_)))
            && matches!(&call.args[1], Expr::Array(array) if array.elems.iter().all(|element| path_is_expr(element, &["Color", "White"]) || path_is_expr(element, &["Color", "Red"])));
    }
    false
}

fn is_safe_effect(effect: &syn::ExprStruct) -> bool {
    if !effect.attrs.is_empty() || effect.qself.is_some() || effect.rest.is_some() {
        return false;
    }
    let expected_fields: &[&str] = if path_is(&effect.path, &["Effect", "DealDamage"]) {
        &["amount", "target"]
    } else if path_is(&effect.path, &["Effect", "GainLifeController"]) {
        &["amount"]
    } else {
        return false;
    };
    let mut fields = BTreeSet::new();
    for field in &effect.fields {
        let syn::Member::Named(name) = &field.member else {
            return false;
        };
        if !field.attrs.is_empty() {
            return false;
        }
        let name = name.to_string();
        if !fields.insert(name.clone()) {
            return false;
        }
        let valid = match name.as_str() {
            "amount" => {
                matches!(&field.expr, Expr::Lit(literal) if matches!(&literal.lit, Lit::Int(_)))
            }
            "target" => path_is_expr(&field.expr, &["TargetRequirement", "PlayerOrCreature"]),
            _ => false,
        };
        if !valid {
            return false;
        }
    }
    fields
        == expected_fields
            .iter()
            .map(|field| (*field).to_owned())
            .collect()
}

fn path_is_expr(expression: &Expr, expected: &[&str]) -> bool {
    matches!(expression, Expr::Path(path) if path.qself.is_none() && path_is(&path.path, expected))
}

fn expression_attrs_are_empty(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(expression) => expression.attrs.is_empty(),
        Expr::Path(expression) => expression.attrs.is_empty(),
        Expr::Array(expression) => expression.attrs.is_empty(),
        Expr::Reference(expression) => expression.attrs.is_empty(),
        Expr::Call(expression) => expression.attrs.is_empty(),
        Expr::Macro(expression) => expression.attrs.is_empty(),
        Expr::Struct(expression) => expression.attrs.is_empty(),
        Expr::Paren(expression) => expression.attrs.is_empty(),
        Expr::Group(expression) => expression.attrs.is_empty(),
        _ => false,
    }
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

fn card_definition_passed(card: &CatalogCard) -> bool {
    matches!(
        card.semantic_status,
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id }
            if definition_id == "RAV-LIGHTNING-HELIX"
    )
}

fn card_status(card: &CatalogCard, passed: bool) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    json!({
        "collector_number": card.collector_number,
        "name": card.name,
        "definition_id": definition_id,
        "engine_status": match card.semantic_status {
            CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice",
            CardSemanticStatus::CatalogOnly { .. } => "catalog_only",
        },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "credited": false,
        "qualification": if passed { "pilot_trace_passed; independent reviewer approval pending" } else { "not_evaluated" }
    })
}

/// Check printed identity and characteristics before play. Effect behavior is
/// intentionally checked only by the event oracle below, so semantic mutants
/// fail through their observed trace rather than this preflight.
fn validate_helix_identity() -> Result<(), String> {
    let definitions = card_definitions();
    let definition = definitions
        .iter()
        .find(|definition| definition.id == "RAV-LIGHTNING-HELIX")
        .ok_or_else(|| {
            "RAV-LIGHTNING-HELIX definition is missing from the executable set".to_owned()
        })?;
    if definition.name != "Lightning Helix"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(0, [Color::White, Color::Red])
        || definition.colors != BTreeSet::from([Color::White, Color::Red])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Instant])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
    {
        return Err(
            "RAV-LIGHTNING-HELIX identity, cost, color, or type differs from the feature contract"
                .to_owned(),
        );
    }
    Ok(())
}

struct CapturedMatch {
    result: cardbench_magic_policies::DeckMatchResult,
    events: Vec<GameEvent>,
    identities: Vec<(ObjectId, PlayerId, &'static str, bool)>,
    helix_object: ObjectId,
}

fn deterministic_pair(seed: u64, prefer_creature: bool) -> Result<CapturedMatch, String> {
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    let boros: Box<dyn CodePolicy> = if prefer_creature {
        Box::new(HelixCreatureTargetPolicy::new(PlayerId(0)))
    } else {
        Box::new(BorosConvokeBurnPolicy::new(PlayerId(0)))
    };
    let pilots: [Box<dyn CodePolicy>; 2] =
        [boros, Box::new(SelesnyaConvokePolicy::new(PlayerId(1)))];
    let (result, events, identities) =
        run_deck_matchup_capturing(config, P0_DECK, P1_DECK, pilots)?;
    let creature_definitions: BTreeSet<&'static str> = card_definitions()
        .into_iter()
        .filter(|definition| definition.card_types.contains(&CardType::Creature))
        .map(|definition| definition.id)
        .collect();
    let identities = identities
        .into_iter()
        .map(|identity| {
            (
                identity.object,
                identity.owner,
                identity.definition,
                creature_definitions.contains(identity.definition),
            )
        })
        .collect::<Vec<_>>();

    let helix_object = events
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if identities.iter().any(|identity| {
                identity.0 == *card
                    && identity.1 == PlayerId(0)
                    && identity.2 == "RAV-LIGHTNING-HELIX"
            }) => Some(*card),
            _ => None,
        })
        .ok_or_else(|| {
            let matching_objects: Vec<_> = identities
                .iter()
                .filter(|identity| {
                    identity.1 == PlayerId(0) && identity.2 == "RAV-LIGHTNING-HELIX"
                })
                .map(|identity| identity.0)
                .collect();
            format!(
                "RAV-LIGHTNING-HELIX was not cast by seat zero; matching objects={matching_objects:?}; termination={:?}",
                result.termination
            )
        })?;

    Ok(CapturedMatch {
        result,
        events,
        identities,
        helix_object,
    })
}

fn find_target_trace(branch: TargetBranch) -> Result<(u64, CapturedMatch), String> {
    let (prefer_creature, max_seed, branch_label) = match branch {
        TargetBranch::Player => (false, MAX_PLAYER_TRACE_SEED, "player"),
        TargetBranch::Creature => (true, MAX_CREATURE_TRACE_SEED, "opponent-creature"),
    };
    let mut first_semantic_error = None;
    for seed in 0..=max_seed {
        let Ok(captured) = deterministic_pair(seed, prefer_creature) else {
            continue;
        };
        if captured.result.is_clean_completion() {
            match validate_trace(&captured, branch) {
                Ok(_) => return Ok((seed, captured)),
                Err(error)
                    if error
                        .starts_with("first target did not receive the required three damage") =>
                {
                    return Err(error);
                }
                Err(error) if first_semantic_error.is_none() => {
                    first_semantic_error = Some(error);
                }
                Err(_) => {}
            }
        }
    }
    Err(first_semantic_error.unwrap_or_else(|| {
        format!("no clean {branch_label}-target trace found for seeds 0..={max_seed}")
    }))
}

#[derive(Clone, Copy)]
enum TargetBranch {
    Player,
    Creature,
}

#[derive(Clone, Copy)]
struct TracePositions {
    cast: usize,
    target_damage: usize,
    controller_gain: usize,
    resolved: usize,
    graveyard: usize,
}

impl TracePositions {
    fn ordered(self) -> [usize; 5] {
        [
            self.cast,
            self.target_damage,
            self.controller_gain,
            self.resolved,
            self.graveyard,
        ]
    }
}

fn player_trace_positions(
    captured: &CapturedMatch,
    branch: TargetBranch,
) -> Result<TracePositions, String> {
    trace_positions(
        &captured.events,
        captured.helix_object,
        &captured.identities,
        branch,
    )
}

fn trace_positions(
    events: &[GameEvent],
    helix_object: ObjectId,
    identities: &[(ObjectId, PlayerId, &'static str, bool)],
    branch: TargetBranch,
) -> Result<TracePositions, String> {
    let cast = single_index(events, |event| {
        matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == helix_object)
    })
    .ok_or_else(|| "Lightning Helix was not cast by seat zero exactly once".to_owned())?;
    let resolved = single_index(
        events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == helix_object),
    )
    .ok_or_else(|| "Lightning Helix did not resolve exactly once".to_owned())?;
    let graveyard = single_index(events, |event| {
        matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == helix_object)
    })
    .ok_or_else(|| "Lightning Helix did not move to its owner's graveyard exactly once".to_owned())?;

    let damage_events: Vec<_> = events
        .iter()
        .enumerate()
        .filter(|(_, event)| match event {
            GameEvent::DamageDealtToPlayer { source, .. }
            | GameEvent::DamageDealtToPermanent { source, .. } => *source == helix_object,
            _ => false,
        })
        .collect();
    if damage_events.len() != 1 {
        return Err(format!(
            "Lightning Helix emitted {} source damage events; expected exactly one",
            damage_events.len()
        ));
    }
    let (target_damage, target_event) = damage_events[0];
    let valid_target_and_amount = match (branch, target_event) {
        (
            TargetBranch::Player,
            GameEvent::DamageDealtToPlayer {
                source,
                player,
                amount,
            },
        ) => *source == helix_object && *player == PlayerId(1) && *amount == 3,
        (
            TargetBranch::Creature,
            GameEvent::DamageDealtToPermanent {
                source,
                permanent,
                amount,
            },
        ) => {
            *source == helix_object
                && *amount == 3
                && identities.iter().any(|identity| {
                    identity.0 == *permanent && identity.1 == PlayerId(1) && identity.3
                })
        }
        _ => false,
    };
    if !valid_target_and_amount {
        return Err("first target did not receive the required three damage".to_owned());
    }

    let controller_gains: Vec<_> = events
        .iter()
        .enumerate()
        .filter(|(index, event)| {
            *index > target_damage
                && *index < resolved
                && matches!(
                    event,
                    GameEvent::LifeGained {
                        player: PlayerId(0),
                        ..
                    }
                )
        })
        .collect();
    if controller_gains.len() != 1 {
        return Err(format!(
            "Lightning Helix resolution contains {} controller life-gain events between damage and resolution; expected exactly one",
            controller_gains.len()
        ));
    }
    let (controller_gain, gain_event) = controller_gains[0];
    if !matches!(
        gain_event,
        GameEvent::LifeGained {
            player: PlayerId(0),
            amount: 3
        }
    ) {
        return Err("Lightning Helix controller did not gain exactly three life".to_owned());
    }

    let source_casts = events
        .iter()
        .filter(|event| matches!(event, GameEvent::SpellCast { card, .. } if *card == helix_object))
        .count();
    let source_resolutions = events
        .iter()
        .filter(|event| matches!(event, GameEvent::SpellResolved { card } if *card == helix_object))
        .count();
    let source_graveyard_moves = events
        .iter()
        .filter(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == helix_object))
        .count();
    if source_casts != 1 || source_resolutions != 1 || source_graveyard_moves != 1 {
        return Err(
            "Lightning Helix cast, resolution, or graveyard transition was duplicated".to_owned(),
        );
    }

    if !(cast < target_damage
        && target_damage < controller_gain
        && controller_gain < resolved
        && resolved < graveyard)
    {
        return Err(
            "Lightning Helix ordered cast/damage/life-gain/resolution/zone events differ"
                .to_owned(),
        );
    }

    Ok(TracePositions {
        cast,
        target_damage,
        controller_gain,
        resolved,
        graveyard,
    })
}

fn single_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    let mut matches = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn validate_trace(captured: &CapturedMatch, branch: TargetBranch) -> Result<Value, String> {
    if !captured.result.is_clean_completion() {
        return Err("game did not reach a clean terminal state".to_owned());
    }
    if captured.result.attempted_policy_moves != captured.result.accepted_policy_moves {
        return Err("reference game contains rejected policy moves".to_owned());
    }
    let positions = player_trace_positions(captured, branch)?;
    let life_before_target = life_at(&captured.events, positions.target_damage.saturating_sub(1));
    let resolution_life = life_at(&captured.events, positions.graveyard);
    let expected_resolution_life = match branch {
        TargetBranch::Player => [life_before_target[0] + 3, life_before_target[1] - 3],
        TargetBranch::Creature => [life_before_target[0] + 3, life_before_target[1]],
    };
    if resolution_life != expected_resolution_life {
        return Err(format!(
            "Lightning Helix post-resolution life {:?} differs from expected {:?}",
            resolution_life, expected_resolution_life
        ));
    }

    let selected = positions.ordered();
    let target = match branch {
        TargetBranch::Player => json!({"kind": "player", "seat": 1}),
        TargetBranch::Creature => json!({
            "kind": "opponent_creature",
            "object": target_object(&captured.events[positions.target_damage])
        }),
    };

    Ok(json!({
        "id": match branch {
            TargetBranch::Player => "rav-lightning-helix-player-target",
            TargetBranch::Creature => "rav-lightning-helix-creature-target"
        },
        "seed": captured.result.config.shuffle_seed,
        "decks": captured.result.deck_ids,
        "policies": if matches!(branch, TargetBranch::Creature) {
            vec!["rav.boros-convoke-burn-creature-target.v1", "rav.selesnya-convoke.v1"]
        } else {
            vec!["rav.boros-convoke-burn.v1", "rav.selesnya-convoke.v1"]
        },
        "terminal": format!("{:?}", captured.result.termination),
        "winner_seat": captured.result.winner.map(|player| player.0),
        "match_end_life": captured.result.life,
        "life_before_target_damage": life_before_target,
        "resolution_life": resolution_life,
        "turns": captured.result.turns,
        "accepted_moves": captured.result.accepted_policy_moves,
        "rejected_moves": captured.result.attempted_policy_moves.saturating_sub(captured.result.accepted_policy_moves),
        "digest": captured.result.digest,
        "helix_object": captured.helix_object.0,
        "target": target,
        "ordered_event_sequence": selected,
        "events": project_relevant(&captured.events, &selected),
        "passed": true
    }))
}

fn project_relevant(events: &[GameEvent], selected: &[usize]) -> Vec<ProjectedEvent> {
    project_events(events)
        .into_iter()
        .filter(|event| selected.contains(&(event.sequence as usize)) || event.kind == "GameEnded")
        .collect()
}

fn target_object(event: &GameEvent) -> u64 {
    match event {
        GameEvent::DamageDealtToPermanent { permanent, .. } => permanent.0,
        _ => 0,
    }
}

fn life_at(events: &[GameEvent], end: usize) -> [i64; 2] {
    let mut life = [20_i64; 2];
    for event in events.iter().take(end.saturating_add(1)) {
        match event {
            GameEvent::DamageDealtToPlayer { player, amount, .. } => {
                life[player.0] -= i64::from(*amount);
            }
            GameEvent::LifeLost { player, amount, .. }
            | GameEvent::LifePaid { player, amount, .. } => {
                life[player.0] -= i64::from(*amount);
            }
            GameEvent::AbilityLifePaid { player, amount, .. }
            | GameEvent::ManaAbilityLifePaid { player, amount } => {
                life[player.0] -= i64::from(*amount);
            }
            GameEvent::LandEntryLifePaid { player, amount, .. } => {
                life[player.0] -= i64::from(*amount);
            }
            GameEvent::LifeGained { player, amount } => {
                life[player.0] += i64::from(*amount);
            }
            _ => {}
        }
    }
    life
}

fn detects_damage_mutation(captured: &CapturedMatch, branch: TargetBranch) -> bool {
    let Ok(positions) = player_trace_positions(captured, branch) else {
        return false;
    };
    let mut mutant = captured.events.clone();
    let changed = match &mut mutant[positions.target_damage] {
        GameEvent::DamageDealtToPlayer { amount, .. }
        | GameEvent::DamageDealtToPermanent { amount, .. }
            if *amount == 3 =>
        {
            *amount = 2;
            true
        }
        _ => false,
    };
    changed
        && trace_positions(&mutant, captured.helix_object, &captured.identities, branch).is_err()
}

fn detects_life_gain_mutation(captured: &CapturedMatch, branch: TargetBranch) -> bool {
    let Ok(positions) = player_trace_positions(captured, branch) else {
        return false;
    };
    let mut mutant = captured.events.clone();
    let changed = match &mut mutant[positions.controller_gain] {
        GameEvent::LifeGained {
            player: PlayerId(0),
            amount,
        } if *amount == 3 => {
            *amount = 2;
            true
        }
        _ => false,
    };
    changed
        && trace_positions(&mutant, captured.helix_object, &captured.identities, branch).is_err()
}

fn detects_order_mutation(captured: &CapturedMatch, branch: TargetBranch) -> bool {
    let Ok(positions) = player_trace_positions(captured, branch) else {
        return false;
    };
    let mut mutant = captured.events.clone();
    mutant.swap(positions.target_damage, positions.controller_gain);
    trace_positions(&mutant, captured.helix_object, &captured.identities, branch).is_err()
}

fn output_root() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        if argument == "--output-root" {
            return args.next().map(std::path::PathBuf::from);
        }
    }
    None
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
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

fn write_outputs(root: &std::path::Path, report: &Value) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).expect("report serializes"),
    )?;
    std::fs::write(root.join("reward.txt"), "0.0\n")
}

struct HelixCreatureTargetPolicy {
    inner: BorosConvokeBurnPolicy,
}

impl HelixCreatureTargetPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            inner: BorosConvokeBurnPolicy::new(player),
        }
    }
}

impl CodePolicy for HelixCreatureTargetPolicy {
    fn id(&self) -> &'static str {
        "rav.boros-convoke-burn-creature-target.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        let action = self.inner.propose_move(view);
        let PolicyAction::Cast(mut request) = action else {
            return action;
        };
        let is_helix = view
            .hand
            .iter()
            .any(|card| card.id == request.card && card.definition == Some("RAV-LIGHTNING-HELIX"));
        if !is_helix {
            return PolicyAction::Cast(request);
        }
        if let Some(creature) = view
            .opponent_battlefield
            .iter()
            .find(|card| card.card_types.contains(&CardType::Creature))
        {
            request.targets = vec![Target::Permanent(creature.id)];
            PolicyAction::Cast(request)
        } else {
            // Wait for a legal creature target so this match can supply the
            // creature-target contract rather than recording a player-target
            // spell under the wrong trace label.
            PolicyAction::PassPriority
        }
    }
}
