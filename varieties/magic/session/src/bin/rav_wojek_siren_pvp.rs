//! Full-game RAV Wojek Siren authority: apply a Radiance pump and win through combat.

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DeckEntry, DeckList, Effect, Game, GameEvent,
    GameView, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_activated_ability_cost_modifier_bindings, rav_additional_spell_cost_bindings,
    rav_attachment_bindings, rav_attachment_triggered_ability_bindings as attachment_triggers,
    rav_basic_land_type_bindings, rav_cost_reduction_bindings,
    rav_damage_replacement_effect_bindings, rav_entry_coin_flip_bindings, rav_entry_copy_bindings,
    rav_generalized_activated_ability_cost_bindings, rav_land_entry_bindings,
    rav_legendary_permanent_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_mana_ability_cost_bindings, rav_replacement_effect_bindings,
    rav_static_attack_restriction_bindings, rav_static_continuous_effect_bindings,
    rav_static_creature_spell_cost_modifier_bindings, rav_static_entry_restriction_bindings,
    rav_static_library_top_reveal_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Expr, Item, Lit, Meta, Stmt, Token, Type, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-WOJEK-SIREN";
const TARGET_ID: &str = "RAV-WATCHWOLF";
const ALLY_ID: &str = "RAV-BOROS-RECRUIT";
const LAND_ID: &str = "RAV-PLAINS";
const FOREST_ID: &str = "RAV-FOREST";
const OPPONENT_CREATURE_ID: &str = "RAV-GOBLIN-SPELUNKERS";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/wojek_siren.rs";
const MAX_TRACE_SEED: u64 = 32;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/wojek_siren.rs"]
mod candidate_wojek_siren;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Wojek Siren PvP result: {error}");
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
            eprintln!("Wojek Siren PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_identity()?;
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
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if !same_match(&captured, &replay) {
        return Err("Wojek Siren full-game replay was not deterministic".to_owned());
    }

    let mutations = mutation_sensitivity(seed)?;
    for name in [
        "wrong_radiance_scope_detected_in_game",
        "wrong_pump_detected_in_game",
        "wrong_cost_detected_in_game",
        "wrong_color_detected_in_game",
    ] {
        if mutations[name] != true {
            return Err(format!(
                "Wojek Siren semantic mutation escaped the full-game trace: {name}"
            ));
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
        "card": {"collector_number": 37, "printings": 1, "definition_id": CARD_ID, "name": "Wojek Siren"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "601.2: casting an instant and announcing its creature target",
                "608.2: target legality on resolution",
                "207.2c: Radiance and the shared-color recipient set",
                "611.2: continuous effects with an end-of-turn duration",
                "613.4: layer 7 power and toughness modification",
                "510: combat damage",
            ],
            "source_contract": "RAV Wojek Siren #37, {W} instant: target creature and each other creature sharing a color with it get +1/+1 until end of turn",
            "authority_cutoff": "2026-09-24"
        },
        "passed": cards_passed == 1,
        "pilot_card_passed": cards_passed == 1,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": if candidate_kind() == "candidate" { "candidate_pilot_pass; independent reviewer approval remains" } else { "reference_pilot_pass; independent review and candidate trial remain" },
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongRadianceScope,
    WrongPump,
    WrongCost,
    WrongColor,
}

fn mutation_sensitivity(seed: u64) -> Result<Value, String> {
    let mut outcomes = serde_json::Map::new();
    for (name, mutation) in [
        (
            "wrong_radiance_scope_detected_in_game",
            Mutation::WrongRadianceScope,
        ),
        ("wrong_pump_detected_in_game", Mutation::WrongPump),
        ("wrong_cost_detected_in_game", Mutation::WrongCost),
        ("wrong_color_detected_in_game", Mutation::WrongColor),
    ] {
        let detected = match run_match(seed, mutation) {
            Ok(captured) => validate_trace(&captured, seed).is_err(),
            Err(_) => true,
        };
        outcomes.insert(name.to_owned(), Value::Bool(detected));
    }
    outcomes.insert("sealed_mutation_authority".to_owned(), Value::Bool(false));
    Ok(Value::Object(outcomes))
}

fn same_match(left: &CapturedMatch, right: &CapturedMatch) -> bool {
    left.events == right.events
        && left.terminal == right.terminal
        && left.winner == right.winner
        && left.turns == right.turns
        && left.siren == right.siren
        && left.target == right.target
        && left.ally == right.ally
        && left.shared_watchwolves == right.shared_watchwolves
        && left.shared_recruits == right.shared_recruits
        && left.unshared == right.unshared
        && left.radiance_recipients == right.radiance_recipients
        && left.snapshots == right.snapshots
        && left.payment == right.payment
        && left.stack_target_verified == right.stack_target_verified
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Wojek Siren source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Wojek Siren source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) || file.items.len() != 3 {
        return Err("Wojek Siren source has an unsupported module shape".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "ManaCost"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut actual_imports = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                if !collect_use_paths(&mut Vec::new(), &item.tree, &mut actual_imports) {
                    return Err("Wojek Siren source has an unsupported import".to_owned());
                }
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !functions.insert(name.clone()) || !definition_function_is_literal(function) {
                    return Err(format!(
                        "Wojek Siren function `{name}` must return a crate-private literal definition"
                    ));
                }
            }
            _ => return Err("Wojek Siren source contains executable top-level code".to_owned()),
        }
    }
    if actual_imports != expected_imports || functions != BTreeSet::from(["definition".to_owned()])
    {
        return Err("Wojek Siren source is missing its declarative definition".to_owned());
    }
    Ok(())
}

fn collect_use_paths(
    prefix: &mut Vec<String>,
    tree: &UseTree,
    output: &mut BTreeSet<Vec<String>>,
) -> bool {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            let valid = collect_use_paths(prefix, &path.tree, output);
            prefix.pop();
            valid
        }
        UseTree::Name(name) => {
            prefix.push(name.ident.to_string());
            output.insert(prefix.clone());
            prefix.pop();
            true
        }
        UseTree::Group(group) => group
            .items
            .iter()
            .all(|item| collect_use_paths(prefix, item, output)),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}

fn definition_function_is_literal(function: &syn::ItemFn) -> bool {
    let attrs_ok = function.attrs.iter().all(|attribute| {
        is_doc_attribute(attribute)
            || attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_))
    });
    let crate_private = matches!(&function.vis, Visibility::Restricted(visibility) if visibility.path.is_ident("crate"));
    if !attrs_ok
        || !crate_private
        || function.sig.ident != "definition"
        || function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
        || !function.sig.inputs.is_empty()
        || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if type_name(ty).as_deref() == Some("CardDefinition"))
        || function.block.stmts.len() != 1
    {
        return false;
    }
    let Stmt::Expr(Expr::Struct(literal), None) = &function.block.stmts[0] else {
        return false;
    };
    literal
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "CardDefinition")
        && literal.rest.is_none()
        && literal.fields.iter().all(|field| {
            matches!(&field.member, syn::Member::Named(_))
                && field.colon_token.is_some()
                && field.attrs.is_empty()
                && safe_expression(&field.expr)
        })
}

fn safe_expression(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(literal) => {
            literal.attrs.is_empty()
                && matches!(literal.lit, Lit::Str(_) | Lit::Int(_) | Lit::Bool(_))
        }
        Expr::Path(path) => {
            let segments: Vec<String> = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            match segments.as_slice() {
                [single] => single == "None",
                [kind, variant] if kind == "Color" => {
                    ["White", "Blue", "Black", "Red", "Green", "Colorless"]
                        .contains(&variant.as_str())
                }
                [kind, variant] if kind == "CardType" => variant == "Instant",
                [kind, variant] if kind == "Effect" => false,
                _ => false,
            }
        }
        Expr::Struct(structure) => {
            let path: Vec<String> = structure
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            let field_names: BTreeSet<String> = structure
                .fields
                .iter()
                .filter_map(|field| match &field.member {
                    syn::Member::Named(name) => Some(name.to_string()),
                    syn::Member::Unnamed(_) => None,
                })
                .collect();
            (path == ["Effect", "RadianceModifyPtUntilEndOfTurn"]
                || path == ["Effect", "ModifyTargetPtUntilEndOfTurn"])
                && structure.rest.is_none()
                && structure.fields.len() == 2
                && field_names == BTreeSet::from(["power".to_owned(), "toughness".to_owned()])
                && structure.fields.iter().all(|field| {
                    let syn::Member::Named(name) = &field.member else { return false; };
                    match name.to_string().as_str() {
                        "power" | "toughness" => matches!(&field.expr, Expr::Lit(literal) if matches!(&literal.lit, Lit::Int(value) if value.base10_parse::<i32>().is_ok())),
                        _ => false,
                    }
                })
        }
        Expr::Call(call) => {
            let Expr::Path(path) = &*call.func else {
                return false;
            };
            let segments: Vec<String> = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            matches!(segments.as_slice(), [ty, method] if (ty == "ManaCost" && method == "with_colors") || (ty == "BTreeSet" && (method == "from" || method == "new")))
                && call.args.iter().all(safe_expression)
        }
        Expr::Array(array) => array.elems.iter().all(safe_expression),
        Expr::Reference(reference) => safe_expression(&reference.expr),
        Expr::Macro(mac) if mac.mac.path.is_ident("vec") => {
            let parser = Punctuated::<Expr, Token![,]>::parse_terminated;
            parser
                .parse2(mac.mac.tokens.clone())
                .is_ok_and(|values| values.iter().all(safe_expression))
        }
        _ => false,
    }
}

fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc")
        && matches!(&attribute.meta, Meta::NameValue(value) if matches!(&value.value, Expr::Lit(literal) if matches!(literal.lit, Lit::Str(_))))
}

fn type_name(ty: &Type) -> Option<String> {
    let Type::Path(path) = ty else {
        return None;
    };
    path.path
        .segments
        .last()
        .map(|segment| segment.ident.to_string())
}

fn validate_candidate_identity() -> Result<(), String> {
    let candidate = candidate_wojek_siren::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Wojek Siren is absent from the RAV definition registry")?;
    if candidate != registered || !definition_identity_valid(&candidate) {
        return Err("candidate Wojek Siren definition differs from its RAV identity".to_owned());
    }
    Ok(())
}

fn definition_identity_valid(definition: &CardDefinition) -> bool {
    definition.id == CARD_ID
        && definition.name == "Wojek Siren"
        && definition.set_code == "RAV"
        && definition.mana_cost == ManaCost::with_colors(0, [Color::White])
        && definition.colors == BTreeSet::from([Color::White])
        && definition.mana_colors.is_empty()
        && definition.card_types == BTreeSet::from([CardType::Instant])
        && !definition.is_basic_land
        && definition.supported_rules == ["full-rules-fidelity", "radiance", "layer-7-modifier"]
        && definition.power.is_none()
        && definition.toughness.is_none()
        && definition.keywords.is_empty()
        && definition.effects
            == [Effect::RadianceModifyPtUntilEndOfTurn {
                power: 1,
                toughness: 1,
            }]
}

#[derive(Clone)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    siren: Option<ObjectId>,
    target: Option<ObjectId>,
    ally: Option<ObjectId>,
    shared_watchwolves: Vec<ObjectId>,
    shared_recruits: Vec<ObjectId>,
    unshared: Option<ObjectId>,
    stack_target_verified: bool,
    radiance_recipients: Vec<ObjectId>,
    snapshots: BTreeMap<ObjectId, (i32, i32)>,
    payment: Vec<Color>,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_WOJEK_SIREN_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_WOJEK_SIREN_SEED must be an unsigned integer".to_owned())?;
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    let mut last_error = String::new();
    for seed in 0..=MAX_TRACE_SEED {
        match run_match(seed, Mutation::None)
            .and_then(|captured| validate_trace(&captured, seed).map(|_| captured))
        {
            Ok(captured) => return Ok((seed, captured)),
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "no clean Wojek Siren target/Radiance combat trace found for seeds 0..={MAX_TRACE_SEED}: {last_error}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definition = candidate_wojek_siren::definition();
    match mutation {
        Mutation::None => {}
        Mutation::WrongRadianceScope => {
            definition.effects = vec![Effect::ModifyTargetPtUntilEndOfTurn {
                power: 1,
                toughness: 1,
            }]
        }
        Mutation::WrongPump => {
            definition.effects = vec![Effect::RadianceModifyPtUntilEndOfTurn {
                power: 1,
                toughness: 0,
            }]
        }
        Mutation::WrongCost => definition.mana_cost = ManaCost::new(2),
        Mutation::WrongColor => {
            definition.mana_cost = ManaCost::with_colors(0, [Color::Red]);
            definition.colors = BTreeSet::from([Color::Red]);
        }
    }
    let pilot_deck = DeckList {
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
                card: ALLY_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: FOREST_ID.to_owned(),
                count: 24,
            },
            DeckEntry {
                card: LAND_ID.to_owned(),
                count: 24,
            },
        ],
        sideboard: vec![],
    };
    let opponent_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: OPPONENT_CREATURE_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    for (name, deck) in [
        ("Wojek Siren pilot", &pilot_deck),
        ("red creature opponent", &opponent_deck),
    ] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{name} deck has {count} cards; expected 60"));
        }
    }
    let mut game = new_rav_game(definition)?;
    game.set_shuffle_seed(seed).map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot_deck)
        .map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent_deck)
        .map_err(|e| e.to_string())?;
    game.draw_opening_hand(PlayerId(0), 7)
        .map_err(|e| e.to_string())?;
    game.draw_opening_hand(PlayerId(1), 7)
        .map_err(|e| e.to_string())?;
    game.begin_game().map_err(|e| e.to_string())?;

    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(WojekSirenPolicy {
            player: PlayerId(0),
            siren_cast: false,
        }),
        Box::new(RedCreaturePolicy {
            player: PlayerId(1),
            creature_cast: false,
        }),
    ];
    let mut accepted_moves = 0_u32;
    let mut attempted_moves = 0_u32;
    let mut siren = None;
    let mut target = None;
    let mut unshared = None;
    let mut stack_target_verified = false;
    let mut radiance_recipients = Vec::new();
    let mut snapshots = BTreeMap::new();
    while !game.is_game_over() {
        if game.turn > 100 || accepted_moves >= 8000 {
            return Err(format!(
                "Wojek Siren game exceeded its action bound at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Wojek Siren policy seat missing")?;
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
        let siren_cast = match &action {
            PolicyAction::CastWithPayment { request, .. }
                if game
                    .card_definition(request.card)
                    .is_ok_and(|card| card.id == CARD_ID)
                    && request.targets.len() == 1 =>
            {
                match request.targets[0] {
                    Target::Permanent(target) => Some((request.card, target)),
                    _ => None,
                }
            }
            _ => None,
        };
        attempted_moves += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Wojek Siren policy move rejected on turn {} / {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        if let Some((spell, selected_target)) = siren_cast {
            let stack = game
                .stack
                .last()
                .ok_or("Wojek Siren did not enter the stack")?;
            if stack.card != spell || stack.targets != [Target::Permanent(selected_target)] {
                return Err(
                    "Wojek Siren selected target differs from its live stack object".to_owned(),
                );
            }
            siren = Some(spell);
            target = Some(selected_target);
            stack_target_verified = true;
        }
        if let Some(spell) = siren
            && snapshots.is_empty()
            && game
                .event_log
                .iter()
                .any(|event| matches!(event, GameEvent::SpellResolved { card } if *card == spell))
        {
            let mut recipients: Vec<ObjectId> = game
                .event_log
                .iter()
                .filter_map(|event| match event {
                    GameEvent::ContinuousEffectCreated { source, target, .. }
                        if *source == spell =>
                    {
                        Some(*target)
                    }
                    _ => None,
                })
                .collect();
            recipients.sort();
            recipients.dedup();
            radiance_recipients = recipients;
            let shared_watchwolves: Vec<ObjectId> = game.players[0]
                .battlefield
                .iter()
                .copied()
                .filter(|object| {
                    game.card_definition(*object)
                        .is_ok_and(|card| card.id == TARGET_ID)
                })
                .collect();
            let shared_recruits: Vec<ObjectId> = game.players[0]
                .battlefield
                .iter()
                .copied()
                .filter(|object| {
                    game.card_definition(*object)
                        .is_ok_and(|card| card.id == ALLY_ID)
                })
                .collect();
            if shared_watchwolves.is_empty() || shared_recruits.is_empty() {
                return Err(format!(
                    "Wojek Siren pilot needs at least one Watchwolf and one Boros Recruit before resolution; found {}/{}",
                    shared_watchwolves.len(),
                    shared_recruits.len()
                ));
            }
            let unshared_object = game.players[1]
                .battlefield
                .iter()
                .copied()
                .find(|object| {
                    game.card_definition(*object)
                        .is_ok_and(|card| card.id == OPPONENT_CREATURE_ID)
                })
                .ok_or("red control creature is not on the battlefield at Siren resolution")?;
            unshared = Some(unshared_object);
            for object in shared_watchwolves
                .iter()
                .chain(&shared_recruits)
                .copied()
                .chain([unshared_object])
            {
                let c = game.characteristics(object).map_err(|e| e.to_string())?;
                snapshots.insert(
                    object,
                    (
                        c.power.ok_or("Wojek Siren recipient has no power")?,
                        c.toughness
                            .ok_or("Wojek Siren recipient has no toughness")?,
                    ),
                );
            }
        }
    }
    let siren = siren.ok_or_else(|| {
        let spells: Vec<&str> = game
            .event_log
            .iter()
            .filter_map(|event| match event {
                GameEvent::SpellCast { card, .. } => game
                    .card_definition(*card)
                    .ok()
                    .map(|definition| definition.id),
                _ => None,
            })
            .collect();
        format!(
            "Wojek Siren was not cast in the full game (turn {}, winner {:?}, spell casts {:?}, pilot battlefield {:?})",
            game.turn, game.winner(), spells, game.players[0].battlefield
        )
    })?;
    let target = target.ok_or("Wojek Siren did not announce a target")?;
    let ally = game.players[0]
        .battlefield
        .iter()
        .copied()
        .find(|object| {
            game.card_definition(*object)
                .is_ok_and(|card| card.id == ALLY_ID)
        })
        .ok_or("Boros Recruit did not survive on the battlefield")?;
    let shared_watchwolves: Vec<ObjectId> = game.players[0]
        .battlefield
        .iter()
        .copied()
        .filter(|object| {
            game.card_definition(*object)
                .is_ok_and(|card| card.id == TARGET_ID)
        })
        .collect();
    let shared_recruits: Vec<ObjectId> = game.players[0]
        .battlefield
        .iter()
        .copied()
        .filter(|object| {
            game.card_definition(*object)
                .is_ok_and(|card| card.id == ALLY_ID)
        })
        .collect();
    let unshared = unshared.ok_or("red control creature was not captured at Siren resolution")?;
    let payment = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if *card == siren => Some(colors.clone()),
            _ => None,
        })
        .ok_or("Wojek Siren has no mana payment receipt")?;
    let winner = game.winner().map(|p| p.0);
    Ok(CapturedMatch {
        events: game.event_log.clone(),
        terminal: winner.map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({seat}))"),
        ),
        winner,
        turns: game.turn,
        accepted_moves,
        rejected_moves: attempted_moves.saturating_sub(accepted_moves),
        siren: Some(siren),
        target: Some(target),
        ally: Some(ally),
        shared_watchwolves,
        shared_recruits,
        unshared: Some(unshared),
        stack_target_verified,
        radiance_recipients,
        snapshots,
        payment,
    })
}

fn new_rav_game(definition: CardDefinition) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Wojek Siren missing from the RAV definition registry")?;
    *registered = definition;
    let mut triggers = rav_triggered_ability_bindings();
    triggers.extend(attachment_triggers());
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers,
        rav_static_continuous_effect_bindings(),
        rav_land_entry_bindings(),
    )
    .map_err(|e| e.to_string())?;
    game.register_static_library_top_reveal_bindings(rav_static_library_top_reveal_bindings())
        .map_err(|e| e.to_string())?;
    game.register_static_attack_restrictions(rav_static_attack_restriction_bindings())
        .map_err(|e| e.to_string())?;
    game.register_static_creature_spell_cost_modifiers(
        rav_static_creature_spell_cost_modifier_bindings(),
    )
    .map_err(|e| e.to_string())?;
    game.register_attachment_bindings(rav_attachment_bindings())
        .map_err(|e| e.to_string())?;
    game.register_entry_copy_bindings(rav_entry_copy_bindings())
        .map_err(|e| e.to_string())?;
    game.register_entry_coin_flip_bindings(rav_entry_coin_flip_bindings())
        .map_err(|e| e.to_string())?;
    game.register_legendary_permanent_bindings(rav_legendary_permanent_bindings())
        .map_err(|e| e.to_string())?;
    game.register_static_entry_restriction_bindings(rav_static_entry_restriction_bindings())
        .map_err(|e| e.to_string())?;
    game.register_mana_ability_cost_bindings(rav_mana_ability_cost_bindings())
        .map_err(|e| e.to_string())?;
    game.register_cost_reduction_bindings(rav_cost_reduction_bindings())
        .map_err(|e| e.to_string())?;
    game.register_activated_ability_cost_modifier_bindings(
        rav_activated_ability_cost_modifier_bindings(),
    )
    .map_err(|e| e.to_string())?;
    game.register_generalized_activated_ability_cost_bindings(
        rav_generalized_activated_ability_cost_bindings(),
    )
    .map_err(|e| e.to_string())?;
    game.register_damage_replacement_effect_bindings(rav_damage_replacement_effect_bindings())
        .map_err(|e| e.to_string())?;
    game.register_replacement_effect_bindings(rav_replacement_effect_bindings())
        .map_err(|e| e.to_string())?;
    game.validate_invariants().map_err(|e| e.to_string())?;
    Ok(game)
}

struct WojekSirenPolicy {
    player: PlayerId,
    siren_cast: bool,
}
struct RedCreaturePolicy {
    player: PlayerId,
    creature_cast: bool,
}

impl CodePolicy for WojekSirenPolicy {
    fn id(&self) -> &'static str {
        "rav.wojek-siren-radiance-pump.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && view.player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: if self.siren_cast {
                    view.own_battlefield
                        .iter()
                        .filter(|card| card.can_attack)
                        .map(|card| card.id)
                        .collect()
                } else {
                    vec![]
                },
            };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && view.player == self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.player != self.player
            || view.decision_player != self.player
            || view.priority != self.player
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        let main = view.active_player == self.player
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain);
        if !main {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            let forests = view
                .own_battlefield
                .iter()
                .filter(|card| card.definition == Some(FOREST_ID))
                .count();
            let plains = view
                .own_battlefield
                .iter()
                .filter(|card| card.definition == Some(LAND_ID))
                .count();
            let preferred = if forests <= plains {
                FOREST_ID
            } else {
                LAND_ID
            };
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(preferred))
                .or_else(|| {
                    view.hand.iter().find(|card| {
                        card.definition == Some(FOREST_ID) || card.definition == Some(LAND_ID)
                    })
                })
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        let target_count = view
            .own_battlefield
            .iter()
            .filter(|card| card.definition == Some(TARGET_ID))
            .count();
        let ally_count = view
            .own_battlefield
            .iter()
            .filter(|card| card.definition == Some(ALLY_ID))
            .count();
        if target_count < 4
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(TARGET_ID))
        {
            if view.mana_pool.amount(Color::Green) >= 1 && view.mana_pool.amount(Color::White) >= 1
            {
                return PolicyAction::Cast(CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                });
            }
            let missing = if view.mana_pool.amount(Color::Green) == 0 {
                FOREST_ID
            } else {
                LAND_ID
            };
            if let Some(land) = view
                .own_battlefield
                .iter()
                .find(|land| land.definition == Some(missing) && !land.tapped)
            {
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: if missing == FOREST_ID {
                        Color::Green
                    } else {
                        Color::White
                    },
                };
            }
        }
        if target_count >= 1
            && ally_count == 0
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(ALLY_ID))
        {
            if view.mana_pool.amount(Color::White) >= 1 {
                return PolicyAction::Cast(CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                });
            }
            if let Some(land) = view
                .own_battlefield
                .iter()
                .find(|land| land.definition == Some(LAND_ID) && !land.tapped)
            {
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: Color::White,
                };
            }
        }
        let ready_target_count = view
            .own_battlefield
            .iter()
            .filter(|card| card.definition == Some(TARGET_ID) && card.can_attack)
            .count();
        let ready_ally_count = view
            .own_battlefield
            .iter()
            .filter(|card| card.definition == Some(ALLY_ID) && card.can_attack)
            .count();
        if target_count >= 1
            && ally_count >= 1
            && !self.siren_cast
            && view.step == Step::PrecombatMain
            && ready_target_count >= 1
            && ready_ally_count >= 1
            && view
                .opponent_battlefield
                .iter()
                .any(|card| card.definition == Some(OPPONENT_CREATURE_ID))
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
        {
            if view.mana_pool.amount(Color::White) >= 1 {
                self.siren_cast = true;
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: card.id,
                        targets: vec![Target::Permanent(
                            view.own_battlefield
                                .iter()
                                .find(|c| c.definition == Some(TARGET_ID))
                                .expect("target checked")
                                .id,
                        )],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: ManaPaymentSelection::default(),
                };
            }
            if let Some(land) = view
                .own_battlefield
                .iter()
                .find(|land| land.definition == Some(LAND_ID) && !land.tapped)
            {
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: Color::White,
                };
            }
        }
        PolicyAction::PassPriority
    }
}

impl CodePolicy for RedCreaturePolicy {
    fn id(&self) -> &'static str {
        "rav.red-goblin-spelunkers-opponent.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && view.player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers { attackers: vec![] };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && view.player == self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.player != self.player
            || view.decision_player != self.player
            || view.active_player != self.player
            || view.priority != self.player
            || view.stack_depth != 0
            || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0
            && let Some(land) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-MOUNTAIN"))
        {
            return PolicyAction::PlayLand { card: land.id };
        }
        if !self.creature_cast
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(OPPONENT_CREATURE_ID))
        {
            if view.mana_pool.amount(Color::Red) >= 3 {
                self.creature_cast = true;
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: ManaPaymentSelection {
                        generic: vec![Color::Red; 2],
                        hybrid: vec![],
                    },
                };
            }
            if let Some(land) = view
                .own_battlefield
                .iter()
                .find(|land| land.definition == Some("RAV-MOUNTAIN") && !land.tapped)
            {
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: Color::Red,
                };
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if captured.winner != Some(0) || captured.terminal != "Winner(PlayerId(0))" {
        return Err("Wojek Siren game did not end in a seat-zero win".to_owned());
    }
    if captured.rejected_moves != 0 {
        return Err("Wojek Siren reference match contains rejected policy moves".to_owned());
    }
    let siren = captured.siren.ok_or("Wojek Siren was not cast")?;
    let target = captured.target.ok_or("Wojek Siren target missing")?;
    let ally = captured.ally.ok_or("Radiance peer creature missing")?;
    let unshared = captured
        .unshared
        .ok_or("nonsharing control creature missing")?;
    if !captured.stack_target_verified {
        return Err("Wojek Siren target was not verified on the live stack object".to_owned());
    }
    if captured.shared_watchwolves.is_empty() || captured.shared_recruits.is_empty() {
        return Err("Wojek Siren did not retain a target and a shared-color creature".to_owned());
    }
    let all_shared: BTreeSet<ObjectId> = captured
        .shared_watchwolves
        .iter()
        .chain(&captured.shared_recruits)
        .copied()
        .collect();
    if all_shared.len() != captured.shared_watchwolves.len() + captured.shared_recruits.len()
        || all_shared.len() < 2
        || !captured.shared_watchwolves.contains(&target)
        || !captured.shared_recruits.contains(&ally)
        || all_shared.contains(&unshared)
        || all_shared.contains(&siren)
    {
        return Err("Wojek Siren trace object identities are not distinct".to_owned());
    }
    if captured.payment != [Color::White] {
        return Err(format!(
            "Wojek Siren must pay exactly one white mana, received {:?}",
            captured.payment
        ));
    }
    let recipients: BTreeSet<ObjectId> = captured.radiance_recipients.iter().copied().collect();
    if recipients != all_shared {
        return Err(format!(
            "Wojek Siren Radiance recipients differ from all eight shared-white creatures: {recipients:?}"
        ));
    }
    if captured
        .shared_watchwolves
        .iter()
        .any(|object| captured.snapshots.get(object) != Some(&(4, 4)))
        || captured
            .shared_recruits
            .iter()
            .any(|object| captured.snapshots.get(object) != Some(&(2, 2)))
        || captured.snapshots.get(&unshared) != Some(&(2, 2))
    {
        return Err(format!(
            "Wojek Siren did not apply +1/+1 only to creatures sharing color with target: {:?}",
            captured.snapshots
        ));
    }
    let payment = captured.events.iter().position(|event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == siren && colors == &[Color::White])).ok_or("Wojek Siren white payment receipt missing")?;
    let cast = captured.events.iter().position(|event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == siren)).ok_or("Wojek Siren cast receipt missing")?;
    let recipient_effects: Vec<usize> = captured
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::ContinuousEffectCreated {
                source,
                target: recipient,
                layer: cardbench_magic_engine::Layer::PowerToughness,
            } if *source == siren && all_shared.contains(recipient) => Some(index),
            _ => None,
        })
        .collect();
    let effected: BTreeSet<ObjectId> = recipient_effects
        .iter()
        .filter_map(|index| match &captured.events[*index] {
            GameEvent::ContinuousEffectCreated { target, .. } => Some(*target),
            _ => None,
        })
        .collect();
    if effected != all_shared {
        return Err(
            "Wojek Siren did not create one layer-seven effect for every shared-color creature"
                .to_owned(),
        );
    }
    let target_effect = recipient_effects.iter().copied().find(|index| matches!(&captured.events[*index], GameEvent::ContinuousEffectCreated { target: recipient, .. } if *recipient == target)).ok_or("Wojek Siren target did not receive the layer-seven modification")?;
    let ally_effect = recipient_effects.iter().copied().find(|index| matches!(&captured.events[*index], GameEvent::ContinuousEffectCreated { target: recipient, .. } if *recipient == ally)).ok_or("Wojek Siren Radiance did not reach its shared-color ally")?;
    let unshared_effect = captured.events.iter().any(|event| matches!(event, GameEvent::ContinuousEffectCreated { source, target: recipient, .. } if *source == siren && *recipient == unshared));
    if unshared_effect {
        return Err(
            "Wojek Siren incorrectly modified a creature that shares no color with its target"
                .to_owned(),
        );
    }
    let resolved = captured
        .events
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == siren))
        .ok_or("Wojek Siren resolution receipt missing")?;
    let spell_graveyard = captured.events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == siren)).ok_or("Wojek Siren did not move from stack to graveyard")?;
    let attackers = captured.events.iter().position(|event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&target) && attackers.contains(&ally))).ok_or("both Radiance creatures did not attack together")?;
    let target_damage = captured.events.iter().position(|event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 4 } if *source == target)).ok_or("modified Watchwolf did not deal four combat damage")?;
    let ally_damage = captured.events.iter().position(|event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 2 } if *source == ally)).ok_or("modified Boros Recruit did not deal two combat damage")?;
    let terminal = captured
        .events
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::GameEnded {
                    winner: Some(PlayerId(0))
                }
            )
        })
        .ok_or("Wojek Siren match terminal event missing")?;
    if !(payment < cast
        && cast < target_effect
        && cast < ally_effect
        && recipient_effects
            .iter()
            .all(|index| cast < *index && *index < resolved)
        && resolved < spell_graveyard
        && spell_graveyard < attackers
        && attackers < target_damage
        && attackers < ally_damage
        && target_damage < terminal
        && ally_damage < terminal)
    {
        return Err(format!(
            "Wojek Siren cast, Radiance, resolution, and pumped combat order differs: {payment}/{cast}/{target_effect}/{ally_effect}/{resolved}/{spell_graveyard}/{attackers}/{target_damage}/{ally_damage}/{terminal}"
        ));
    }
    let combat_damage: Vec<(ObjectId, i32)> = captured
        .events
        .iter()
        .filter_map(|event| match event {
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if all_shared.contains(source) => Some((*source, *amount)),
            _ => None,
        })
        .collect();
    let damaged: BTreeSet<ObjectId> = combat_damage.iter().map(|(source, _)| *source).collect();
    let combat_damage_total: i32 = combat_damage.iter().map(|(_, amount)| *amount).sum();
    if !damaged.contains(&target) || !damaged.contains(&ally) || combat_damage_total < 20 {
        return Err(format!(
            "Wojek Siren Radiance creatures did not deal lethal full-game combat damage: {combat_damage:?}"
        ));
    }
    let mut order = vec![
        payment,
        cast,
        target_effect,
        ally_effect,
        resolved,
        spell_graveyard,
        attackers,
        target_damage,
        ally_damage,
        terminal,
    ];
    order.extend(recipient_effects.iter().copied());
    order.sort_unstable();
    order.dedup();
    let selected: BTreeSet<usize> = order.iter().copied().collect();
    let events: Vec<ProjectedEvent> = project_events(&captured.events)
        .into_iter()
        .filter(|event| selected.contains(&(event.sequence as usize)))
        .collect();
    let live_power_toughness: BTreeMap<String, (i32, i32)> = captured
        .snapshots
        .iter()
        .map(|(object, stats)| (object.0.to_string(), *stats))
        .collect();
    Ok(json!({
        "id": "rav-wojek-siren-radiance-shared-color-pump",
        "seed": seed,
        "decks": ["rav-wojek-siren-pilot-60", "rav-red-creature-opponent-60"],
        "policies": ["rav.wojek-siren-radiance-pump.v1", "rav.red-goblin-spelunkers-opponent.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "siren_object": siren.0,
        "target_object": target.0,
        "target_definition": TARGET_ID,
        "shared_color_ally_object": ally.0,
        "shared_watchwolf_objects": captured.shared_watchwolves.iter().map(|object| object.0).collect::<Vec<_>>(),
        "shared_recruit_objects": captured.shared_recruits.iter().map(|object| object.0).collect::<Vec<_>>(),
        "nonsharing_control_object": unshared.0,
        "live_power_toughness": live_power_toughness,
        "radiance_recipients": captured.radiance_recipients.iter().map(|object| object.0).collect::<Vec<_>>(),
        "target_bound_stack_object_verified": captured.stack_target_verified,
        "combat_damage": combat_damage.iter().map(|(source, amount)| json!({"source": source.0, "amount": amount})).collect::<Vec<_>>(),
        "combat_damage_total": combat_damage_total,
        "ordered_event_sequence": order,
        "events": events,
        "passed": true
    }))
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
        "qualification": if passed { "pilot_trace_passed; independent reviewer approval pending" } else { "not_evaluated" }
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
