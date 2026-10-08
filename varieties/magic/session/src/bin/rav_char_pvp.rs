//! Full-game RAV Char trace authority used by the first Magic set-PvP pilot.
//!
//! The artifact is deliberately a judge-scope slice: it contains only Char's
//! public cast, resolution, damage and graveyard events plus the terminal
//! result. It does not serialize the engine `Game` or unrelated hidden-zone
//! identities.

use cardbench_magic_engine::{
    CardType, Color, GameEvent, GameView, ManaCost, ObjectId, PlayerId, PolicyAction, Target, Zone,
};
use cardbench_magic_policies::{
    BorosCharControlPolicy, CodePolicy, DeckMatchConfig, SelesnyaConvokePolicy,
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

const TASK_ID: &str = "cardbench/magic/set_pvp";
const P0_DECK: &str = "rav_boros_char_control";
const P1_DECK: &str = "rav_selesnya_convoke";
const PLAYER_TRACE_SEED: u64 = 0;
const MAX_CREATURE_TRACE_SEED: u64 = 128;
const CHAR_SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/char.rs";

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Char PvP result: {error}");
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
            eprintln!("Char PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_char_definition()?;
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

    let player = deterministic_pair(PLAYER_TRACE_SEED, false)?;
    if !player.result.is_clean_completion() {
        return Err("player-target reference game did not complete cleanly".to_owned());
    }
    let player_trace = validate_trace(&player, TargetBranch::Player)?;
    let player_replay = deterministic_pair(PLAYER_TRACE_SEED, false)?;
    if player.result.digest != player_replay.result.digest
        || project_relevant(&player_replay.events, player.char_object)
            != project_relevant(&player.events, player.char_object)
    {
        return Err("player-target replay was not deterministic".to_owned());
    }

    let (creature_seed, creature) = find_creature_target_trace()?;
    if !creature.result.is_clean_completion() {
        return Err("creature-target reference game did not complete cleanly".to_owned());
    }
    let creature_trace = validate_trace(&creature, TargetBranch::Creature)?;
    let creature_replay = deterministic_pair(creature_seed, true)?;
    if creature.result.digest != creature_replay.result.digest
        || project_relevant(&creature_replay.events, creature.char_object)
            != project_relevant(&creature.events, creature.char_object)
    {
        return Err("creature-target replay was not deterministic".to_owned());
    }

    // Mutate the observed semantic packet, then require the same trace oracle
    // to reject it. This is a checker-sensitivity control, not a claim that a
    // sealed mutation suite or an independent candidate implementation exists.
    let mutation_detected = detects_damage_mutation(&player, TargetBranch::Player)
        && detects_damage_mutation(&creature, TargetBranch::Creature);
    if !mutation_detected {
        return Err("targeted 4-to-3 damage mutation escaped the trace oracle".to_owned());
    }

    let char_passed = true;
    let breakdown: Vec<Value> = catalog
        .iter()
        .map(|card| card_status(card, char_passed))
        .collect();
    let passed_count = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();

    Ok(json!({
        "schema_version": "cardbench.magic.set-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "candidate": {"kind": candidate_kind(), "card_definition": "RAV-CHAR"},
        "candidate_kind": candidate_kind(),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "source_contract": "RAV printed semantics; later Oracle target-domain errata are excluded from this historical set compatibility task",
            "authority_cutoff": "2026-09-23"
        },
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "passed": true,
        "pilot_card_passed": char_passed,
        "cards_passed": passed_count,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": qualification_status(),
        "mutation_sensitivity": {
            "public_trace_mutation": "Char first-target damage 4 -> 3",
            "player_target_detected": detects_damage_mutation(&player, TargetBranch::Player),
            "creature_target_detected": detects_damage_mutation(&creature, TargetBranch::Creature),
            "sealed_mutation_authority": false
        },
        "traces": [player_trace, creature_trace],
        "per_card_breakdown": breakdown
    }))
}

/// The candidate module is linked into this process, so accept only a small
/// declarative Rust subset. This prevents source code from printing forged
/// reports, exiting the verifier, or reading the trusted authority mounted by
/// Harbor after submission.
fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(CHAR_SOURCE_PATH)
        .map_err(|error| format!("could not read Char source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Char source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Char source has a non-documentation module attribute".to_owned());
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
                    return Err("Char source contains an unsupported import".to_owned());
                };
                for path in paths {
                    actual_imports.insert(path);
                }
            }
            Item::Fn(item) if definition.is_none() => definition = Some(item),
            _ => return Err("Char source contains executable top-level code".to_owned()),
        }
    }
    if actual_imports != expected_imports || file.items.len() != 3 {
        return Err(
            "Char source imports or item structure differ from the task boundary".to_owned(),
        );
    }
    let definition = definition.ok_or_else(|| "Char definition() is missing".to_owned())?;
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
        return Err("Char function must remain a plain definition() -> CardDefinition".to_owned());
    }
    if function.block.stmts.len() != 1 {
        return Err("Char definition must be a single card literal".to_owned());
    }
    let Stmt::Expr(expression, None) = &function.block.stmts[0] else {
        return Err("Char definition contains executable statements".to_owned());
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
        return Err("Char definition must return a CardDefinition literal".to_owned());
    };
    if !card.attrs.is_empty()
        || card.qself.is_some()
        || !path_is(&card.path, &["CardDefinition"])
        || card.rest.is_some()
    {
        return Err("Char definition must be a direct CardDefinition literal".to_owned());
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
            return Err("Char definition contains a non-named field".to_owned());
        };
        if !field.attrs.is_empty()
            || !actual_fields.insert(name.to_string())
            || !is_safe_card_expression(&field.expr)
        {
            return Err("Char definition contains a computed or duplicate field".to_owned());
        }
    }
    if actual_fields != expected_fields {
        return Err("Char definition fields differ from CardDefinition".to_owned());
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
            && matches!(&call.args[1], Expr::Array(array) if array.elems.iter().all(|element| path_is_expr(element, &["Color", "Red"])));
    }
    false
}

fn is_safe_effect(effect: &syn::ExprStruct) -> bool {
    if !effect.attrs.is_empty() || effect.qself.is_some() || effect.rest.is_some() {
        return false;
    }
    let expected_fields: &[&str] = if path_is(&effect.path, &["Effect", "DealDamage"]) {
        &["amount", "target"]
    } else if path_is(&effect.path, &["Effect", "DealDamageController"]) {
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

fn validate_char_definition() -> Result<(), String> {
    let definitions = card_definitions();
    let definition = definitions
        .iter()
        .find(|definition| definition.id == "RAV-CHAR")
        .ok_or_else(|| "RAV-CHAR definition is missing from the executable set".to_owned())?;
    if definition.name != "Char"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(2, [Color::Red])
        || definition.colors != BTreeSet::from([Color::Red])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Instant])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
    {
        return Err(
            "RAV-CHAR identity, cost, color, or type differs from the feature contract".to_owned(),
        );
    }
    Ok(())
}

struct CapturedMatch {
    result: cardbench_magic_policies::DeckMatchResult,
    events: Vec<GameEvent>,
    identities: Vec<(ObjectId, PlayerId, &'static str, bool)>,
    char_object: ObjectId,
}

fn deterministic_pair(seed: u64, prefer_creature: bool) -> Result<CapturedMatch, String> {
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    let boros: Box<dyn CodePolicy> = if prefer_creature {
        Box::new(CharCreatureTargetPolicy::new(PlayerId(0)))
    } else {
        Box::new(BorosCharControlPolicy::new(PlayerId(0)))
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
    let char_object = events
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if identities.iter().any(|identity| {
                identity.object == *card
                    && identity.owner == PlayerId(0)
                    && identity.definition == "RAV-CHAR"
            }) =>
            {
                Some(*card)
            }
            _ => None,
        })
        .ok_or_else(|| {
            let char_objects: Vec<_> = identities
                .iter()
                .filter(|identity| {
                    identity.definition == "RAV-CHAR" && identity.owner == PlayerId(0)
                })
                .map(|identity| identity.object)
                .collect();
            let seat_zero_casts: Vec<_> = events
                .iter()
                .filter_map(|event| match event {
                    GameEvent::SpellCast {
                        player: PlayerId(0),
                        card,
                    } => Some(*card),
                    _ => None,
                })
                .collect();
            format!(
                "RAV-CHAR not cast by seat zero; matching objects={char_objects:?}; seat-zero spells={seat_zero_casts:?}; termination={:?}",
                result.termination
            )
        })?;
    Ok(CapturedMatch {
        result,
        events,
        identities: identities
            .into_iter()
            .map(|identity| {
                (
                    identity.object,
                    identity.owner,
                    identity.definition,
                    creature_definitions.contains(identity.definition),
                )
            })
            .collect(),
        char_object,
    })
}

fn find_creature_target_trace() -> Result<(u64, CapturedMatch), String> {
    for seed in 0..=MAX_CREATURE_TRACE_SEED {
        let captured = deterministic_pair(seed, true)?;
        if captured.result.is_clean_completion()
            && validate_trace(&captured, TargetBranch::Creature).is_ok()
        {
            return Ok((seed, captured));
        }
    }
    Err(format!(
        "no clean creature-target trace found for seeds 0..={MAX_CREATURE_TRACE_SEED}"
    ))
}

#[derive(Clone, Copy)]
enum TargetBranch {
    Player,
    Creature,
}

fn validate_trace(captured: &CapturedMatch, branch: TargetBranch) -> Result<Value, String> {
    if !captured.result.is_clean_completion() {
        return Err("game did not reach a clean terminal state".to_owned());
    }
    if captured.result.attempted_policy_moves != captured.result.accepted_policy_moves {
        return Err("reference game contains rejected policy moves".to_owned());
    }

    let event_positions = char_event_positions(&captured.events, captured.char_object);
    let cast = event_positions
        .iter()
        .find(|(_, event)| matches!(event, GameEvent::SpellCast { card, .. } if *card == captured.char_object))
        .ok_or_else(|| "Char was not cast".to_owned())?;
    let resolved = event_positions
        .iter()
        .find(|(_, event)| matches!(event, GameEvent::SpellResolved { card } if *card == captured.char_object))
        .ok_or_else(|| "Char did not resolve".to_owned())?;
    let moved = event_positions
        .iter()
        .find(|(_, event)| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == captured.char_object))
        .ok_or_else(|| "resolved Char did not move to its graveyard".to_owned())?;

    let target_damage = event_positions
        .iter()
        .find(|(_, event)| match (branch, *event) {
            (
                TargetBranch::Player,
                GameEvent::DamageDealtToPlayer {
                    source,
                    player,
                    amount,
                },
            ) => *source == captured.char_object && *player == PlayerId(1) && *amount == 4,
            (
                TargetBranch::Creature,
                GameEvent::DamageDealtToPermanent {
                    source,
                    permanent,
                    amount,
                },
            ) => {
                *source == captured.char_object
                    && *amount == 4
                    && captured.identities.iter().any(|identity| {
                        identity.0 == *permanent && identity.1 == PlayerId(1) && identity.3
                    })
            }
            _ => false,
        })
        .ok_or_else(|| "Char's first target did not receive the required four damage".to_owned())?;

    let self_damage = event_positions
        .iter()
        .find(|(_, event)| {
            matches!(event,
            GameEvent::DamageDealtToPlayer { source, player, amount }
                if *source == captured.char_object && *player == PlayerId(0) && *amount == 2)
        })
        .ok_or_else(|| "Char's controller did not receive two damage".to_owned())?;

    if event_positions.len() != 5 {
        return Err(format!(
            "Char emitted {} relevant events; the trace requires exactly five",
            event_positions.len()
        ));
    }

    if !(cast.0 < target_damage.0
        && target_damage.0 < self_damage.0
        && self_damage.0 < resolved.0
        && resolved.0 < moved.0)
    {
        return Err("Char's ordered cast/effect/resolution/zone events differ".to_owned());
    }

    let target = match branch {
        TargetBranch::Player => json!({"kind": "player", "seat": 1}),
        TargetBranch::Creature => {
            json!({"kind": "opponent_creature", "object": target_object(target_damage.1)})
        }
    };
    let life_before_target = life_at(&captured.events, target_damage.0.saturating_sub(1));
    let checkpoint_life = life_at(&captured.events, moved.0);
    let expected_checkpoint_life = match branch {
        TargetBranch::Player => [life_before_target[0] - 2, life_before_target[1] - 4],
        TargetBranch::Creature => [life_before_target[0] - 2, life_before_target[1]],
    };
    if checkpoint_life != expected_checkpoint_life {
        return Err(format!(
            "Char post-resolution life state {:?} differs from expected {:?}",
            checkpoint_life, expected_checkpoint_life
        ));
    }
    let relevant = project_relevant(&captured.events, captured.char_object);

    Ok(json!({
        "id": match branch { TargetBranch::Player => "rav-char-player-target", TargetBranch::Creature => "rav-char-creature-target" },
        "seed": captured.result.config.shuffle_seed,
        "decks": captured.result.deck_ids,
        "policies": if matches!(branch, TargetBranch::Creature) {
            vec!["rav.boros-char-creature-target.v1", "rav.selesnya-convoke.v1"]
        } else {
            vec!["rav.boros-char-control.v1", "rav.selesnya-convoke.v1"]
        },
        "terminal": format!("{:?}", captured.result.termination),
        "winner_seat": captured.result.winner.map(|player| player.0),
        "match_end_life": captured.result.life,
        "life_before_char_target": life_before_target,
        "char_resolution_life": checkpoint_life,
        "turns": captured.result.turns,
        "accepted_moves": captured.result.accepted_policy_moves,
        "rejected_moves": captured.result.attempted_policy_moves.saturating_sub(captured.result.accepted_policy_moves),
        "digest": captured.result.digest,
        "char_object": captured.char_object.0,
        "target": target,
        "ordered_event_sequence": [cast.0, target_damage.0, self_damage.0, resolved.0, moved.0],
        "events": relevant,
        "passed": true
    }))
}

fn char_event_positions(events: &[GameEvent], char_object: ObjectId) -> Vec<(usize, &GameEvent)> {
    events
        .iter()
        .enumerate()
        .filter(|(_, event)| match event {
            GameEvent::SpellCast { card, .. } | GameEvent::SpellResolved { card } => {
                *card == char_object
            }
            GameEvent::CardMoved {
                card,
                to: Zone::Graveyard,
            } => *card == char_object,
            GameEvent::DamageDealtToPlayer { source, .. }
            | GameEvent::DamageDealtToPermanent { source, .. } => *source == char_object,
            _ => false,
        })
        .collect()
}

fn project_relevant(events: &[GameEvent], char_object: ObjectId) -> Vec<ProjectedEvent> {
    project_events(events)
        .into_iter()
        .filter(|event| {
            event.kind == "GameEnded"
                || matches!(event.kind.as_str(), "SpellCast" | "SpellResolved")
                    && event
                        .facts
                        .objects
                        .first()
                        .is_some_and(|object| object.0 == char_object.0)
                || matches!(
                    event.kind.as_str(),
                    "DamageDealtToPlayer" | "DamageDealtToPermanent"
                ) && event
                    .facts
                    .objects
                    .first()
                    .is_some_and(|object| object.0 == char_object.0)
                || event.kind == "CardMoved"
                    && event
                        .facts
                        .objects
                        .first()
                        .is_some_and(|object| object.0 == char_object.0)
                    && event.facts.zone.as_deref() == Some("Graveyard")
        })
        .collect()
}

fn target_object(event: &GameEvent) -> u64 {
    match event {
        GameEvent::DamageDealtToPermanent { permanent, .. } => permanent.0,
        _ => 0,
    }
}

fn life_at(events: &[GameEvent], end: usize) -> [i64; 2] {
    let mut life = [20_i64, 20_i64];
    for event in events.iter().take(end.saturating_add(1)) {
        match event {
            GameEvent::DamageDealtToPlayer { player, amount, .. } => {
                life[player.0] -= i64::from(*amount);
            }
            GameEvent::LifeLost { player, amount, .. } => {
                life[player.0] -= i64::from(*amount);
            }
            GameEvent::LifeGained { player, amount } => {
                life[player.0] += i64::from(*amount);
            }
            GameEvent::LifePaid { player, amount, .. } => {
                life[player.0] -= i64::from(*amount);
            }
            GameEvent::AbilityLifePaid { player, amount, .. }
            | GameEvent::ManaAbilityLifePaid { player, amount } => {
                life[player.0] -= i64::from(*amount);
            }
            GameEvent::LandEntryLifePaid { player, amount, .. } => {
                life[player.0] -= i64::from(*amount);
            }
            _ => {}
        }
    }
    life
}

fn detects_damage_mutation(captured: &CapturedMatch, branch: TargetBranch) -> bool {
    let mut mutant = captured.events.clone();
    let mut changed = false;
    for event in &mut mutant {
        match (branch, event) {
            (
                TargetBranch::Player,
                GameEvent::DamageDealtToPlayer {
                    source,
                    player,
                    amount,
                },
            ) if *source == captured.char_object && *player == PlayerId(1) && *amount == 4 => {
                *amount = 3;
                changed = true;
                break;
            }
            (TargetBranch::Creature, GameEvent::DamageDealtToPermanent { source, amount, .. })
                if *source == captured.char_object && *amount == 4 =>
            {
                *amount = 3;
                changed = true;
                break;
            }
            _ => {}
        }
    }
    changed && validate_event_slice(&mutant, captured.char_object, branch).is_err()
}

fn validate_event_slice(
    events: &[GameEvent],
    char_object: ObjectId,
    branch: TargetBranch,
) -> Result<(), String> {
    let target_four = events.iter().any(|event| match (branch, event) {
        (
            TargetBranch::Player,
            GameEvent::DamageDealtToPlayer {
                source,
                player,
                amount,
            },
        ) => *source == char_object && *player == PlayerId(1) && *amount == 4,
        (TargetBranch::Creature, GameEvent::DamageDealtToPermanent { source, amount, .. }) => {
            *source == char_object && *amount == 4
        }
        _ => false,
    });
    let self_two = events.iter().any(|event| {
        matches!(event,
        GameEvent::DamageDealtToPlayer { source, player, amount }
            if *source == char_object && *player == PlayerId(0) && *amount == 2)
    });
    if target_four && self_two {
        Ok(())
    } else {
        Err("Char event semantics mismatch".to_owned())
    }
}

fn card_status(card: &CatalogCard, char_passed: bool) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = char_passed && definition_id == Some("RAV-CHAR");
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
    std::fs::write(root.join("reward.txt"), "0.0\n")?;
    Ok(())
}

struct CharCreatureTargetPolicy {
    inner: BorosCharControlPolicy,
}

impl CharCreatureTargetPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            inner: BorosCharControlPolicy::new(player),
        }
    }
}

impl CodePolicy for CharCreatureTargetPolicy {
    fn id(&self) -> &'static str {
        "rav.boros-char-creature-target.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        let action = self.inner.propose_move(view);
        let PolicyAction::Cast(mut request) = action else {
            return action;
        };
        let is_char = view
            .hand
            .iter()
            .any(|card| card.id == request.card && card.definition == Some("RAV-CHAR"));
        if is_char
            && let Some(creature) = view
                .opponent_battlefield
                .iter()
                .find(|card| card.card_types.contains(&CardType::Creature))
        {
            request.targets = vec![Target::Permanent(creature.id)];
        }
        PolicyAction::Cast(request)
    }
}
