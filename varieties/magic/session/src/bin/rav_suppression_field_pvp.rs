//! Full-game RAV Suppression Field cost-modifier trace authority.

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbilityCostModifier, ActivatedAbilityCostModifierBinding,
    ActivatedAbilityKind, CardType, CastRequest, Color, DeckEntry, DeckList, Game, GameEvent,
    GameView, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::{
    CatalogPolicyProfile, CodePolicy, DeckMatchConfig, RavCatalogPolicy,
};
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
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use syn::{Expr, Item, Stmt, Type, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-SUPPRESSION-FIELD";
const CARD_NAME: &str = "Suppression Field";
const GUILDMAGE_ID: &str = "RAV-DIMIR-GUILDMAGE";
const GUILDMAGE_ABILITY: &str = "target-player-draw";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/suppression_field.rs";
const TRACE_ID: &str = "rav-suppression-field-global-ability-tax-pvp";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/suppression_field.rs"]
mod candidate_suppression_field;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
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
            eprintln!("Suppression Field PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_runtime()?;
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
        return Err("RAV catalog denominator differs from the pinned source contract".to_owned());
    }
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.terminal != replay.terminal
        || captured.winner != replay.winner
        || captured.field != replay.field
        || captured.guildmage != replay.guildmage
        || captured.drawn_card != replay.drawn_card
    {
        return Err("Suppression Field full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "missing_live_source_detected_in_game": mutation_detected(seed, Mutation::RemoveTax),
        "incorrect_tax_amount_detected_in_game": mutation_detected(seed, Mutation::WrongAmount),
        "sealed_mutation_authority": false
    });
    for key in [
        "missing_live_source_detected_in_game",
        "incorrect_tax_amount_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Suppression Field gameplay mutation escaped the trace oracle: {key}"
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
        "card": {"collector_number": 31, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": ["106 Mana", "117 Priority", "601.2 Casting spells", "602 Activated abilities and costs", "605 Mana abilities", "608 Resolving spells and abilities"],
            "source_contract": "RAV Suppression Field is a white enchantment whose live static ability increases each nonmana activated ability's generic activation cost by {2}; mana abilities remain unaffected.",
            "authority_cutoff": "2026-09-24"
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
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Suppression Field source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Suppression Field source is not valid declarative Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
    {
        return Err("Suppression Field has an unsupported module attribute".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "ActivatedAbilityCostModifier"],
        vec![
            "cardbench_magic_engine",
            "ActivatedAbilityCostModifierBinding",
        ],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "ManaCost"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let mut paths = Vec::new();
                if item.leading_colon.is_some() || !flatten_use_tree(&[], &item.tree, &mut paths) {
                    return Err(
                        "Suppression Field imports differ from the task contract".to_owned()
                    );
                }
                imports.extend(paths);
            }
            Item::Fn(function) => {
                let output = match function.sig.ident.to_string().as_str() {
                    "definition" => "CardDefinition",
                    "cost_modifier_binding" => "ActivatedAbilityCostModifierBinding",
                    _ => return Err("Suppression Field has an unsupported function".to_owned()),
                };
                if !matches!(&function.vis, Visibility::Restricted(visibility) if visibility.path.is_ident("crate"))
                    || !function.sig.inputs.is_empty()
                    || function.sig.constness.is_some()
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || function.sig.abi.is_some()
                    || !function.sig.generics.params.is_empty()
                    || function.sig.generics.where_clause.is_some()
                    || function.sig.variadic.is_some()
                    || function.block.stmts.len() != 1
                    || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if type_is(ty, output))
                    || !function.attrs.iter().all(|attribute| {
                        attribute.path().is_ident("doc")
                            || attribute.path().is_ident("must_use")
                                && matches!(attribute.meta, syn::Meta::Path(_))
                    })
                {
                    return Err("Suppression Field functions must be simple crate-visible literal factories".to_owned());
                }
                let Stmt::Expr(Expr::Struct(literal), None) = &function.block.stmts[0] else {
                    return Err(
                        "Suppression Field functions must return direct struct literals".to_owned(),
                    );
                };
                if !type_is(
                    &Type::Path(syn::TypePath {
                        qself: None,
                        path: literal.path.clone(),
                    }),
                    output,
                ) || literal.rest.is_some()
                {
                    return Err(
                        "Suppression Field literal type differs from its return type".to_owned(),
                    );
                }
                if !literal_only(&function.block.stmts[0]) {
                    return Err(
                        "Suppression Field candidate factories may contain only literal data"
                            .to_owned(),
                    );
                }
                functions.insert(function.sig.ident.to_string());
            }
            _ => return Err("Suppression Field contains executable top-level code".to_owned()),
        }
    }
    if imports != expected_imports
        || functions
            != BTreeSet::from(["definition".to_owned(), "cost_modifier_binding".to_owned()])
        || file.items.len() != 4
    {
        return Err("Suppression Field module structure differs from the task contract".to_owned());
    }
    Ok(())
}

fn type_is(ty: &Type, expected: &str) -> bool {
    matches!(ty, Type::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 && path.path.segments[0].ident == expected)
}

fn literal_only(statement: &Stmt) -> bool {
    let Stmt::Expr(Expr::Struct(value), None) = statement else {
        return false;
    };
    value.fields.iter().all(|field| literal_expr(&field.expr))
}

fn literal_expr(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(value) => matches!(
            value.lit,
            syn::Lit::Str(_) | syn::Lit::Int(_) | syn::Lit::Bool(_)
        ),
        Expr::Path(path) => {
            let segments: Vec<_> = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            matches!(segments.as_slice(), [name] if matches!(name.as_str(), "None" | "White" | "Enchantment"))
                || matches!(segments.as_slice(), [owner, member] if matches!((owner.as_str(), member.as_str()), ("Color", "White") | ("CardType", "Enchantment") | ("BTreeSet", "new") | ("ManaCost", "with_colors") | ("ActivatedAbilityCostModifier", "IncreaseGeneric")))
        }
        Expr::Array(array) => array.elems.iter().all(literal_expr),
        Expr::Reference(reference) => literal_expr(&reference.expr),
        Expr::Call(call) => {
            let Expr::Path(function) = call.func.as_ref() else {
                return false;
            };
            let segments: Vec<_> = function
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            matches!(segments.as_slice(), [owner, member] if matches!((owner.as_str(), member.as_str()), ("ManaCost", "with_colors") | ("BTreeSet", "from") | ("BTreeSet", "new")))
                && call.args.iter().all(literal_expr)
        }
        Expr::Struct(value) => {
            let segments: Vec<_> = value
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            (matches!(segments.as_slice(), [name] if matches!(name.as_str(), "CardDefinition" | "ActivatedAbilityCostModifierBinding"))
                || matches!(segments.as_slice(), [owner, member] if owner == "ActivatedAbilityCostModifier" && member == "IncreaseGeneric"))
                && value.rest.is_none()
                && value.fields.iter().all(|field| literal_expr(&field.expr))
        }
        Expr::Macro(mac) => mac.mac.path.is_ident("vec") && mac.mac.tokens.is_empty(),
        Expr::Paren(value) => literal_expr(&value.expr),
        Expr::Group(value) => literal_expr(&value.expr),
        _ => false,
    }
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

fn validate_candidate_runtime() -> Result<(), String> {
    let candidate = candidate_suppression_field::definition();
    let registry = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Suppression Field is missing from the RAV registry")?;
    let expected_cost = ManaCost::with_colors(1, [Color::White]);
    if candidate.id != CARD_ID
        || candidate.name != CARD_NAME
        || candidate.set_code != "RAV"
        || candidate.mana_cost != expected_cost
        || candidate.colors != BTreeSet::from([Color::White])
        || !candidate.mana_colors.is_empty()
        || candidate.card_types != BTreeSet::from([CardType::Enchantment])
        || candidate.is_basic_land
        || candidate.power.is_some()
        || candidate.toughness.is_some()
        || !candidate.keywords.is_empty()
        || !candidate.effects.is_empty()
        || candidate.supported_rules
            != [
                "full-rules-fidelity",
                "colored-cost-casting",
                "static-nonmana-activated-ability-tax",
            ]
        || candidate.id != registry.id
        || candidate.name != registry.name
        || candidate.mana_cost != registry.mana_cost
        || candidate.colors != registry.colors
        || candidate.card_types != registry.card_types
    {
        return Err(
            "Suppression Field candidate identity or printed characteristics differ".to_owned(),
        );
    }
    let binding = candidate_suppression_field::cost_modifier_binding();
    if binding.source_definition != CARD_ID
        || binding.modifier
            != (ActivatedAbilityCostModifier::IncreaseGeneric {
                amount: 2,
                nonmana_only: true,
            })
    {
        return Err(
            "Suppression Field candidate cost modifier must add exactly {2} to nonmana abilities"
                .to_owned(),
        );
    }
    let registered = rav_activated_ability_cost_modifier_bindings();
    if !registered.contains(&binding) {
        return Err(
            "Suppression Field candidate modifier is not delegated through the RAV registry"
                .to_owned(),
        );
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    RemoveTax,
    WrongAmount,
}

fn candidate_binding(mutation: Mutation) -> Option<ActivatedAbilityCostModifierBinding> {
    let mut binding = candidate_suppression_field::cost_modifier_binding();
    match mutation {
        Mutation::None => Some(binding),
        Mutation::RemoveTax => None,
        Mutation::WrongAmount => {
            binding.modifier = ActivatedAbilityCostModifier::IncreaseGeneric {
                amount: 1,
                nonmana_only: true,
            };
            Some(binding)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    field: Option<ObjectId>,
    guildmage: Option<ObjectId>,
    ability_paid: Option<ManaCost>,
    drawn_card: Option<ObjectId>,
    draw_target_player: Option<usize>,
    p0_library_before: Option<usize>,
    p0_library_after: Option<usize>,
    mana_source_after_field: Option<ObjectId>,
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(raw) = std::env::var("CARDBENCH_SUPPRESSION_FIELD_SEED") {
        let seed = raw
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_SUPPRESSION_FIELD_SEED must be an unsigned integer")?;
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let Ok(captured) = run_match(seed, Mutation::None)
            && validate_trace(&captured, seed).is_ok()
        {
            return Ok((seed, captured));
        }
    }
    Err(format!(
        "no complete Suppression Field trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Suppression Field missing from RAV definitions")? =
        candidate_suppression_field::definition();
    let mut modifiers = rav_activated_ability_cost_modifier_bindings();
    modifiers.retain(|binding| binding.source_definition != CARD_ID);
    if let Some(binding) = candidate_binding(mutation) {
        modifiers.push(binding);
    }
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
    .map_err(|error| error.to_string())?;
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
    game.register_activated_ability_cost_modifier_bindings(modifiers)
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
    let deck0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: WATCHWOLF_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".into(),
                count: 26,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 26,
            },
        ],
        sideboard: vec![],
    };
    let deck1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: GUILDMAGE_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-ISLAND".into(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    for (seat, deck) in [(0, &deck0), (1, &deck1)] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("seat {seat} deck has {count} cards, expected 60"));
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 48,
        max_policy_moves: 6000,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed).map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(0), &deck0)
        .map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(1), &deck1)
        .map_err(|e| e.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|e| e.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|e| e.to_string())?;
    game.begin_game().map_err(|e| e.to_string())?;
    let shared = Arc::new(AtomicBool::new(false));
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(SuppressionPolicy::new(PlayerId(0), shared.clone())),
        Box::new(SuppressionPolicy::new(PlayerId(1), shared.clone())),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut target_player = None;
    let mut draw_snapshot: Option<(usize, Vec<ObjectId>)> = None;
    let mut library_after = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(format!(
                "Suppression Field game exceeded bounds at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Suppression Field policy seat missing")?;
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
        let ability_target = match &action {
            PolicyAction::ActivateAbility { activation }
                if game
                    .card_definition(activation.source)
                    .is_ok_and(|card| card.id == GUILDMAGE_ID) =>
            {
                activation.targets.first().and_then(|target| match target {
                    Target::Player(player) => Some(player.0),
                    _ => None,
                })
            }
            _ => None,
        };
        if ability_target.is_some() {
            target_player = ability_target;
            draw_snapshot = Some((
                game.players[0].library.len(),
                game.players[0].library.clone(),
            ));
        }
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Suppression Field move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|e| format!("post-move invariant failure: {e}"))?;
        if game.event_log.iter().any(|event|matches!(event,GameEvent::AbilityResolved{source,ability:GUILDMAGE_ABILITY,..} if game.card_definition(*source).is_ok_and(|definition|definition.id==GUILDMAGE_ID))) {
            if draw_snapshot.is_some() && library_after.is_none() { library_after=Some(game.players[0].library.len()); }
            shared.store(true,Ordering::Relaxed);
        }
    }
    let events = game.event_log.clone();
    let field = events.iter().find_map(|event| match event {
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
    let guildmage = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(1),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|definition| definition.id == GUILDMAGE_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let ability_resolved=guildmage.and_then(|source|events.iter().position(|event|matches!(event,GameEvent::AbilityResolved{source:actual,ability:GUILDMAGE_ABILITY,..} if *actual==source)));
    let mana_source_after_field = ability_resolved.and_then(|resolved| {
        events
            .iter()
            .enumerate()
            .skip(resolved + 1)
            .find_map(|(_, event)| match event {
                GameEvent::ManaAbilityActivated {
                    player: PlayerId(1),
                    land,
                    ..
                } if game
                    .card_definition(*land)
                    .is_ok_and(|definition| definition.id == "RAV-ISLAND") =>
                {
                    Some(*land)
                }
                _ => None,
            })
    });
    let ability_paid = guildmage.and_then(|source| {
        events.iter().find_map(|event| match event {
            GameEvent::AbilityManaPaid {
                player: PlayerId(1),
                source: actual,
                ability: GUILDMAGE_ABILITY,
                mana_cost,
            } if *actual == source => Some(mana_cost.clone()),
            _ => None,
        })
    });
    let ability_activated=guildmage.and_then(|source|events.iter().position(|event|matches!(event,GameEvent::AbilityActivated{player:PlayerId(1),source:actual,ability:GUILDMAGE_ABILITY,..} if *actual==source)));
    let drawn_card = if let (Some((_, library_cards)), Some(activation), Some(resolved)) =
        (draw_snapshot.as_ref(), ability_activated, ability_resolved)
    {
        events
            .iter()
            .enumerate()
            .skip(activation + 1)
            .take(resolved.saturating_sub(activation + 1))
            .find_map(|(_, event)| match event {
                GameEvent::CardMoved {
                    card,
                    to: Zone::Hand,
                } if library_cards.contains(card) => Some(*card),
                _ => None,
            })
    } else {
        None
    };
    let winner = game.winner().map(|player| player.0);
    Ok(Captured {
        events,
        terminal: winner.map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({seat}))"),
        ),
        winner,
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempted.saturating_sub(accepted),
        field,
        guildmage,
        ability_paid,
        drawn_card,
        draw_target_player: target_player,
        p0_library_before: draw_snapshot.as_ref().map(|snapshot| snapshot.0),
        p0_library_after: library_after,
        mana_source_after_field,
    })
}

struct SuppressionPolicy {
    player: PlayerId,
    inner: RavCatalogPolicy,
    shared: Arc<AtomicBool>,
    field_cast: bool,
    guildmage_cast: bool,
    activation_offered: bool,
    mana_demo_offered: bool,
}
impl SuppressionPolicy {
    fn new(player: PlayerId, shared: Arc<AtomicBool>) -> Self {
        Self {
            player,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
            shared,
            field_cast: false,
            guildmage_cast: false,
            activation_offered: false,
            mana_demo_offered: false,
        }
    }
}
impl CodePolicy for SuppressionPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.suppression-field-controller.v1"
        } else {
            "rav.suppression-field-opponent-guildmage.v1"
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
            return PolicyAction::DeclareAttackers {
                attackers: if self.player == PlayerId(0) && self.shared.load(Ordering::Relaxed) {
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
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.priority != self.player
            || view.active_player != self.player
            || view.stack_depth != 0
            || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            return self.inner.propose_move(view);
        }
        if view.lands_played == 0
            && view
                .hand
                .iter()
                .any(|card| card.card_types.contains(&CardType::Land))
        {
            let action = self.inner.propose_move(view);
            if matches!(
                action,
                PolicyAction::PlayLand { .. } | PolicyAction::PlayLandWithEntryLifePayment { .. }
            ) {
                return action;
            }
        }
        if self.player == PlayerId(0) && !self.field_cast {
            let cost = candidate_suppression_field::definition().mana_cost;
            if let Some(action) = cast_action(view, CARD_ID, &cost, &mut self.field_cast) {
                return action;
            }
        }
        if self.player == PlayerId(1) && !self.guildmage_cast {
            let cost = card_definitions()
                .into_iter()
                .find(|card| card.id == GUILDMAGE_ID)
                .map(|card| card.mana_cost)
                .unwrap_or_else(|| ManaCost::new(99));
            if let Some(action) = cast_action(view, GUILDMAGE_ID, &cost, &mut self.guildmage_cast) {
                return action;
            }
        }
        let field_live = view
            .own_battlefield
            .iter()
            .chain(view.opponent_battlefield.iter())
            .any(|card| card.definition == Some(CARD_ID));
        if self.player == PlayerId(1) && field_live && !self.activation_offered {
            if let Some(source) = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(GUILDMAGE_ID))
            {
                let effective = ManaCost::with_colors(5, [Color::Blue]);
                if view.mana_pool.amount(Color::Blue) > 0 && view.mana_pool.total() >= 6 {
                    self.activation_offered = true;
                    return PolicyAction::ActivateAbility {
                        activation: AbilityActivation {
                            source: source.id,
                            ability_id: GUILDMAGE_ABILITY,
                            sacrifice_sources: vec![],
                            additional_tap_creatures: vec![],
                            discard_cards: vec![],
                            targets: vec![Target::Player(PlayerId(0))],
                        },
                    };
                }
                if let Some(action) = make_mana_action(view, &effective) {
                    return action;
                }
            }
        }
        if self.player == PlayerId(1)
            && field_live
            && self.shared.load(Ordering::Relaxed)
            && !self.mana_demo_offered
        {
            if let Some(land) = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some("RAV-ISLAND") && !card.tapped)
            {
                self.mana_demo_offered = true;
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: Color::Blue,
                };
            }
        }
        let action = self.inner.propose_move(view);
        if self.player == PlayerId(1) && matches!(action, PolicyAction::ActivateAbility { .. }) {
            PolicyAction::PassPriority
        } else {
            action
        }
    }
}

fn cast_action(
    view: &GameView,
    card_id: &str,
    cost: &ManaCost,
    offered: &mut bool,
) -> Option<PolicyAction> {
    let card = view
        .hand
        .iter()
        .find(|card| card.definition == Some(card_id))?;
    if let Some(selection) = payment_selection(view, cost) {
        *offered = true;
        return Some(PolicyAction::CastWithPayment {
            request: CastRequest {
                card: card.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: selection,
        });
    }
    make_mana_action(view, cost)
}
fn payment_selection(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
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
        let color = if available.get(&symbol.first).copied().unwrap_or(0) > 0 {
            symbol.first
        } else if available.get(&symbol.second).copied().unwrap_or(0) > 0 {
            symbol.second
        } else {
            return None;
        };
        *available.get_mut(&color)? -= 1;
        hybrid.push(color);
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
fn make_mana_action(view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
    let missing = cost
        .colored
        .iter()
        .find(|color| view.mana_pool.amount(**color) == 0)
        .copied()
        .or_else(|| {
            cost.hybrid
                .iter()
                .find(|symbol| {
                    view.mana_pool.amount(symbol.first) == 0
                        && view.mana_pool.amount(symbol.second) == 0
                })
                .map(|symbol| symbol.first)
        });
    let short = u16::from(view.mana_pool.total())
        < u16::from(cost.generic) + cost.colored.len() as u16 + cost.hybrid.len() as u16;
    if let Some(land) = view
        .own_battlefield
        .iter()
        .find(|land| {
            !land.tapped
                && land.card_types.contains(&CardType::Land)
                && missing.map_or(!land.mana_colors.is_empty(), |color| {
                    land.mana_colors.contains(&color)
                })
        })
        .or_else(|| {
            short
                .then(|| {
                    view.own_battlefield
                        .iter()
                        .find(|land| !land.tapped && land.card_types.contains(&CardType::Land))
                })
                .flatten()
        })
    {
        let color = missing
            .filter(|color| land.mana_colors.contains(color))
            .or_else(|| land.mana_colors.iter().next().copied())?;
        return Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        });
    }
    None
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))"
        || captured.winner != Some(0)
        || captured.rejected_moves != 0
    {
        return Err(
            "Suppression Field match did not reach a clean seat-zero terminal win".to_owned(),
        );
    }
    let field = captured
        .field
        .ok_or("Suppression Field was not cast by seat zero")?;
    let guildmage = captured
        .guildmage
        .ok_or("seat one did not cast Dimir Guildmage")?;
    if captured.draw_target_player != Some(0) {
        return Err("Guildmage draw ability did not target seat zero".to_owned());
    }
    let base = ManaCost::with_colors(3, [Color::Blue]);
    let effective = ManaCost::with_colors(5, [Color::Blue]);
    if captured.ability_paid.as_ref() != Some(&effective) {
        return Err(format!(
            "taxed Guildmage ability paid {:?}, expected {{5}}{{U}}",
            captured.ability_paid
        ));
    }
    if captured
        .p0_library_before
        .zip(captured.p0_library_after)
        .is_none_or(|(before, after)| before != after + 1)
    {
        return Err(format!(
            "targeted player draw did not move exactly one library card to hand: {:?}->{:?}",
            captured.p0_library_before, captured.p0_library_after
        ));
    }
    let events = &captured.events;
    let field_paid=find_event(events,|event|matches!(event,GameEvent::SpellManaPaid{player:PlayerId(0),card,colors} if *card==field&&colors.len()==2&&colors.contains(&Color::White))).ok_or("Suppression Field payment receipt missing")?;
    let field_cast = find_event(
        events,
        |event| matches!(event,GameEvent::SpellCast{player:PlayerId(0),card} if *card==field),
    )
    .ok_or("Suppression Field SpellCast missing")?;
    let field_resolved = find_after(
        events,
        field_cast,
        |event| matches!(event,GameEvent::SpellResolved{card} if *card==field),
    )
    .ok_or("Suppression Field did not resolve")?;
    let field_entered = find_after(
        events,
        field_resolved,
        |event| matches!(event,GameEvent::CardMoved{card,to:Zone::Battlefield} if *card==field),
    )
    .ok_or("Suppression Field did not enter the battlefield")?;
    let guild_paid=find_event(events,|event|matches!(event,GameEvent::SpellManaPaid{player:PlayerId(1),card,..} if *card==guildmage)).ok_or("Dimir Guildmage payment receipt missing")?;
    let guild_cast = find_event(
        events,
        |event| matches!(event,GameEvent::SpellCast{player:PlayerId(1),card} if *card==guildmage),
    )
    .ok_or("Dimir Guildmage SpellCast missing")?;
    let guild_resolved = find_after(
        events,
        guild_cast,
        |event| matches!(event,GameEvent::SpellResolved{card} if *card==guildmage),
    )
    .ok_or("Dimir Guildmage did not resolve")?;
    let guild_entered = find_after(
        events,
        guild_resolved,
        |event| matches!(event,GameEvent::CardMoved{card,to:Zone::Battlefield} if *card==guildmage),
    )
    .ok_or("Dimir Guildmage battlefield entry missing")?;
    let cost_calculated=find_event(events,|event|matches!(event,GameEvent::ActivatedAbilityCostCalculated{context} if context.acting_player==PlayerId(1)&&context.source==guildmage&&context.ability_id==GUILDMAGE_ABILITY)).ok_or("taxed ability cost calculation receipt missing")?;
    let cost_context = match &events[cost_calculated] {
        GameEvent::ActivatedAbilityCostCalculated { context } => context,
        _ => unreachable!(),
    };
    if cost_context.kind != ActivatedAbilityKind::NonMana
        || cost_context.base_mana_cost != base
        || cost_context.effective_mana_cost != effective
        || cost_context.increases.len() != 1
        || cost_context.increases[0].source != field
        || cost_context.increases[0].generic_amount != 2
    {
        return Err("Suppression Field cost receipt does not show the live source adding exactly {2} to a nonmana ability".to_owned());
    }
    let ability_paid=find_after(events,cost_calculated,|event|matches!(event,GameEvent::AbilityManaPaid{player:PlayerId(1),source,ability:GUILDMAGE_ABILITY,mana_cost} if *source==guildmage&&*mana_cost==effective)).ok_or("Guildmage did not pay the effective {5}{U} activation cost")?;
    let activated=find_after(events,ability_paid,|event|matches!(event,GameEvent::AbilityActivated{player:PlayerId(1),source,definition:GUILDMAGE_ID,ability:GUILDMAGE_ABILITY,..} if *source==guildmage)).ok_or("Guildmage ability did not enter the stack")?;
    let resolved=find_after(events,activated,|event|matches!(event,GameEvent::AbilityResolved{source,ability:GUILDMAGE_ABILITY,..} if *source==guildmage)).ok_or("Guildmage draw ability did not resolve")?;
    let drawn = captured
        .drawn_card
        .ok_or("Guildmage ability did not move a card from seat zero's library to hand")?;
    let draw_move=(activated+1..resolved).find(|index|matches!(events[*index],GameEvent::CardMoved{card,to:Zone::Hand} if card==drawn)).ok_or("targeted card draw movement is missing")?;
    let island = captured
        .mana_source_after_field
        .ok_or("seat one did not use an Island mana ability while Suppression Field was live")?;
    let mana_ability=find_after(events,resolved,|event|matches!(event,GameEvent::ManaAbilityActivated{player:PlayerId(1),land,color:Color::Blue} if *land==island)).ok_or("untaxed Island mana ability receipt missing")?;
    let attack=find_after(events,resolved,|event|matches!(event,GameEvent::AttackersDeclared{player:PlayerId(0),attackers} if !attackers.is_empty())).ok_or("seat zero did not continue to attack after the ability resolved")?;
    let ended = find_after(events, attack, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("seat-zero terminal PvP win missing")?;
    let field_mana_paid = match &events[field_paid] {
        GameEvent::SpellManaPaid { colors, .. } => colors
            .iter()
            .map(|color| format!("{color:?}"))
            .collect::<Vec<_>>(),
        _ => unreachable!(),
    };
    let selected = BTreeSet::from([
        field_paid,
        field_cast,
        field_resolved,
        field_entered,
        guild_paid,
        guild_cast,
        guild_resolved,
        guild_entered,
        cost_calculated,
        ability_paid,
        activated,
        draw_move,
        resolved,
        mana_ability,
        attack,
        ended,
    ]);
    let mut projected: Vec<ProjectedEvent> = project_events(events)
        .into_iter()
        .enumerate()
        .filter_map(|(index, event)| selected.contains(&index).then_some(event))
        .collect();
    projected.sort_by_key(|event| event.sequence);
    let kinds: Vec<_> = projected.iter().map(|event| event.kind.as_str()).collect();
    let mut expected = Vec::new();
    let field_group = ["SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved"];
    let guildmage_group = ["SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved"];
    if guild_paid < field_paid {
        expected.extend(guildmage_group);
        expected.extend(field_group);
    } else {
        expected.extend(field_group);
        expected.extend(guildmage_group);
    }
    expected.extend([
        "ActivatedAbilityCostCalculated",
        "AbilityManaPaid",
        "AbilityActivated",
        "CardMoved",
        "AbilityResolved",
        "ManaAbilityActivated",
        "AttackersDeclared",
        "GameEnded",
    ]);
    if kinds != expected {
        return Err(format!(
            "Suppression Field projected event trace differs: {kinds:?}; expected {expected:?}"
        ));
    }
    let ordered: Vec<usize> = projected
        .iter()
        .map(|event| event.sequence as usize)
        .collect();
    Ok(json!({
        "id":TRACE_ID,"seed":seed,"decks":["rav-suppression-field-watchwolf-forest-plains","rav-dimir-guildmage-island"],"policies":["rav.suppression-field-controller.v1","rav.suppression-field-opponent-guildmage.v1"],
        "terminal":captured.terminal,"winner_seat":captured.winner,"turns":captured.turns,"accepted_moves":captured.accepted_moves,"rejected_moves":captured.rejected_moves,"digest":event_digest(events),
        "mechanics":{"field_object":field.0,"field_mana_cost":"{1}{W}","field_mana_paid":field_mana_paid,"field_controller_seat":0,"affected_ability_source_object":guildmage.0,"affected_ability_definition":GUILDMAGE_ID,"affected_ability":"target-player-draw","affected_ability_controller_seat":1,"base_ability_cost":"{3}{U}","effective_ability_cost":"{5}{U}","generic_increase":2,"cost_modifier_source_object":field.0,"ability_mana_paid":{"generic":5,"colors":["Blue"]},"ability_stacked":true,"ability_resolved":true,"draw_target_player":0,"draw_count":1,"drawn_card_object":drawn.0,"target_library_before":captured.p0_library_before,"target_library_after":captured.p0_library_after,"mana_ability_source_object":island.0,"mana_ability_source_definition":"RAV-ISLAND","mana_ability_activated_after_field":true,"ordered_event_sequence":ordered},
        "events":projected,"passed":true
    }))
}

fn find_event(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn find_after(
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
fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    run_match(seed, mutation)
        .and_then(|captured| validate_trace(&captured, seed).map(|_| ()))
        .is_err()
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
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"engine_status":match card.semantic_status{CardSemanticStatus::ExecutableCompatibilitySlice{..}=>"executable_compatibility_slice",CardSemanticStatus::CatalogOnly{..}=>"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"credited":false,"qualification":if passed{"pilot_trace_passed; independent reviewer approval pending"}else{"not_evaluated"}})
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
