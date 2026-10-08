//! Full-game RAV Selesnya Sagittars Reach and tap-damage authority.

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, CardDefinition, CardType, CastRequest, Color, CombatBlock,
    DeckEntry, DeckList, Effect, Game, GameEvent, GameView, Keyword, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, TargetRequirement, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_activated_ability_cost_modifier_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings,
    rav_attachment_triggered_ability_bindings as attachment_triggers, rav_basic_land_type_bindings,
    rav_cost_reduction_bindings, rav_damage_replacement_effect_bindings,
    rav_entry_coin_flip_bindings, rav_entry_copy_bindings,
    rav_generalized_activated_ability_cost_bindings, rav_land_entry_bindings,
    rav_legendary_permanent_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_mana_ability_cost_bindings, rav_replacement_effect_bindings,
    rav_static_attack_restriction_bindings, rav_static_continuous_effect_bindings,
    rav_static_creature_spell_cost_modifier_bindings, rav_static_entry_restriction_bindings,
    rav_static_library_top_reveal_bindings, rav_triggered_ability_bindings, CardSemanticStatus,
    CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
};
use cardbench_magic_session::{project_events, ProjectedEvent};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::parse::Parser;
use syn::{Expr, Item, Meta, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-SELESNYA-SAGITTARS";
const WATCHWOLF: &str = "RAV-WATCHWOLF";
const HAWK: &str = "RAV-COURIER-HAWK";
const ELF: &str = "RAV-ELVES-OF-DEEP-SHADOW";
const ABILITY_ID: &str = "tap-damage-attacker-or-blocker";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/selesnya_sagittars.rs";
const MAX_TRACE_SEED: u64 = 128;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/selesnya_sagittars.rs"]
mod candidate_selesnya_sagittars;

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
            eprintln!("Selesnya Sagittars PvP authority failed: {error}");
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
        return Err("RAV catalog denominator differs from the source contract".to_owned());
    }
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.winner != replay.winner
        || captured.sagittars != replay.sagittars
        || captured.elf != replay.elf
        || captured.hawk != replay.hawk
        || captured.watchwolf != replay.watchwolf
    {
        return Err(
            "Selesnya Sagittars shuffled full-game replay was not deterministic".to_owned(),
        );
    }
    let mutations = json!({
        "damage_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::NoDamage)?,
        "tap_cost_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::NoTapCost)?,
        "reach_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::NoReach)?,
        "target_boundary_broadened_mutation_detected_by_probe": target_boundary_mutation_detected()?,
        "sealed_mutation_authority": false
    });
    for key in [
        "damage_removed_mutation_detected_in_game",
        "tap_cost_removed_mutation_detected_in_game",
        "reach_removed_mutation_detected_in_game",
        "target_boundary_broadened_mutation_detected_by_probe",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Selesnya Sagittars gameplay mutant escaped the oracle: {key}"
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
        "card": {"collector_number": 229, "definition_id": CARD_ID, "name": "Selesnya Sagittars"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": ["105.2, 202.1-202.2 colored identity and mana cost", "117 priority", "302.6 summoning sickness and tap costs", "508 attackers", "509 blockers and Reach", "601.2 casting", "602.1-602.2 activated abilities and costs", "608.2 resolving abilities", "120.3 damage", "704 lethal damage"],
            "source_contract": "RAV #229 identity and characteristics, Reach, and {T}: deal 1 damage to target attacking or blocking creature",
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
        "qualification_status": "unreviewed_diagnostic",
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy)]
enum Mutation {
    None,
    NoDamage,
    NoTapCost,
    NoReach,
    BroadTarget,
}

fn candidate_values(mutation: Mutation) -> (CardDefinition, ActivatedAbility) {
    let mut definition = candidate_selesnya_sagittars::definition();
    let mut ability = candidate_selesnya_sagittars::tap_damage_ability();
    match mutation {
        Mutation::None => {}
        Mutation::NoDamage => {
            ability.effects = vec![Effect::DealDamage {
                amount: 0,
                target: TargetRequirement::AttackingOrBlockingCreature,
            }]
        }
        Mutation::NoTapCost => ability.tap_cost = false,
        Mutation::NoReach => definition.keywords.clear(),
        Mutation::BroadTarget => {
            ability.targets = vec![TargetRequirement::Creature];
            ability.effects = vec![Effect::DealDamage {
                amount: 1,
                target: TargetRequirement::Creature,
            }];
        }
    }
    (definition, ability)
}

fn validate_candidate_definition() -> Result<(), String> {
    let (definition, ability) = candidate_values(Mutation::None);
    let cost = ManaCost::with_colors(3, [Color::Green, Color::White]);
    if definition.id != CARD_ID
        || definition.name != "Selesnya Sagittars"
        || definition.set_code != "RAV"
        || definition.mana_cost != cost
        || definition.colors != BTreeSet::from([Color::Green, Color::White])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.power != Some(2)
        || definition.toughness != Some(5)
        || definition.keywords != [Keyword::Reach]
        || !definition.effects.is_empty()
        || ability.id != ABILITY_ID
        || ability.mana_cost != ManaCost::new(0)
        || !ability.tap_cost
        || ability.sorcery_speed
        || ability.additional_tap_creatures != 0
        || ability.sacrifice_source
        || ability.sacrifice_creatures != 0
        || ability.sacrifice_lands != 0
        || ability.discard_cards != 0
        || ability.targets != [TargetRequirement::AttackingOrBlockingCreature]
        || ability.effects
            != [Effect::DealDamage {
                amount: 1,
                target: TargetRequirement::AttackingOrBlockingCreature,
            }]
    {
        return Err("Selesnya Sagittars identity, Reach, or activated ability differs from its feature contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Selesnya Sagittars source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Selesnya Sagittars module is not valid Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) || file.items.len() != 4 {
        return Err("Selesnya Sagittars module has unsupported items".to_owned());
    }
    let expected: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "ActivatedAbility"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TargetRequirement"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut functions = BTreeMap::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                if item.leading_colon.is_some() || !flatten_use(&[], &item.tree, &mut imports) {
                    return Err(
                        "Selesnya Sagittars imports are outside the source contract".to_owned()
                    );
                }
            }
            Item::Fn(item) => {
                if functions.insert(item.sig.ident.to_string(), item).is_some()
                    || !safe_function(item)
                {
                    return Err("Selesnya Sagittars has duplicate or executable helpers".to_owned());
                }
            }
            _ => {
                return Err(
                    "Selesnya Sagittars module contains executable top-level code".to_owned(),
                )
            }
        }
    }
    if imports != expected
        || functions.len() != 2
        || !functions.contains_key("definition")
        || !functions.contains_key("tap_damage_ability")
    {
        return Err(
            "Selesnya Sagittars module does not export only its literal definition and ability"
                .to_owned(),
        );
    }
    Ok(())
}

fn safe_function(function: &syn::ItemFn) -> bool {
    let attrs = function.attrs.iter().all(|attribute| {
        is_doc_attribute(attribute)
            || attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_))
    });
    let crate_visibility =
        matches!(&function.vis, Visibility::Restricted(value) if value.path.is_ident("crate"));
    attrs
        && crate_visibility
        && function.sig.constness.is_none()
        && function.sig.asyncness.is_none()
        && function.sig.unsafety.is_none()
        && function.sig.abi.is_none()
        && function.sig.generics.params.is_empty()
        && function.sig.generics.where_clause.is_none()
        && function.sig.inputs.is_empty()
        && function.sig.variadic.is_none()
        && function.block.stmts.len() == 1
        && matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(value), None) if value.rest.is_none() && safe_path(&value.path) && value.fields.iter().all(|field| safe_expr(&field.expr)))
}

fn safe_path(path: &syn::Path) -> bool {
    path.is_ident("CardDefinition") || path.is_ident("ActivatedAbility")
}

fn safe_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(value) => value.attrs.is_empty(),
        Expr::Path(value) => {
            if !value.attrs.is_empty() || value.qself.is_some() {
                return false;
            }
            let names: Vec<String> = value
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            matches!(names.as_slice(), [name] if name == "true" || name == "false")
                || matches!(names.as_slice(), [ty, variant] if matches!(ty.as_str(), "Color"|"CardType"|"Keyword"|"TargetRequirement") && !variant.is_empty())
        }
        Expr::Call(value) => {
            if !value.attrs.is_empty() {
                return false;
            }
            let allowed = match &*value.func {
                Expr::Path(path) if path.qself.is_none() => {
                    let names: Vec<String> = path
                        .path
                        .segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect();
                    matches!(names.as_slice(), [a,b] if (a == "ManaCost" && (b == "new" || b == "with_colors")) || (a == "BTreeSet" && (b == "new" || b == "from")))
                        || matches!(names.as_slice(), [name] if name == "Some")
                }
                _ => false,
            };
            allowed && value.args.iter().all(safe_expr)
        }
        Expr::Struct(value) => {
            let allowed = value.path.is_ident("CardDefinition")
                || value.path.is_ident("ActivatedAbility")
                || value
                    .path
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect::<Vec<_>>()
                    .as_slice()
                    == ["Effect", "DealDamage"];
            allowed
                && value.attrs.is_empty()
                && value.rest.is_none()
                && value.fields.iter().all(|field| safe_expr(&field.expr))
        }
        Expr::Array(value) => value.attrs.is_empty() && value.elems.iter().all(safe_expr),
        Expr::Reference(value) => {
            value.attrs.is_empty() && value.mutability.is_none() && safe_expr(&value.expr)
        }
        Expr::Macro(value) => {
            if !value.mac.path.is_ident("vec") || !value.attrs.is_empty() {
                return false;
            }
            let Ok(expressions) =
                syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
                    .parse2(value.mac.tokens.clone())
            else {
                return false;
            };
            expressions.iter().all(safe_expr)
        }
        Expr::Paren(value) => safe_expr(&value.expr),
        Expr::Group(value) => safe_expr(&value.expr),
        _ => false,
    }
}

fn flatten_use(prefix: &[String], tree: &UseTree, imports: &mut BTreeSet<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix.to_vec();
            next.push(path.ident.to_string());
            flatten_use(&next, &path.tree, imports)
        }
        UseTree::Name(name) => {
            let mut path = prefix.to_vec();
            if name.ident != "self" {
                path.push(name.ident.to_string());
            }
            imports.insert(path);
            true
        }
        UseTree::Group(group) => group
            .items
            .iter()
            .all(|item| flatten_use(prefix, item, imports)),
        UseTree::Glob(_) | UseTree::Rename(_) => false,
    }
}

fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc") && matches!(attribute.meta, Meta::NameValue(_))
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Some(value) = std::env::var("CARDBENCH_SELESNYA_SAGITTARS_SEED").ok() {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "seed must be an unsigned integer".to_owned())?;
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let Ok(captured) = run_match(seed, Mutation::None) {
            if validate_trace(&captured, seed).is_ok() {
                return Ok((seed, captured));
            }
        }
    }
    Err(format!(
        "no legal Selesnya Sagittars full-game trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    if matches!(mutation, Mutation::BroadTarget) {
        return target_boundary_mutation_detected();
    }
    match run_match(seed, mutation) {
        Ok(captured) => Ok(validate_trace(&captured, seed).is_err()),
        Err(_) => Ok(true),
    }
}

#[derive(Clone, Debug)]
struct TargetProbe {
    accepted: bool,
    rejection: String,
    unchanged: bool,
}

fn run_target_probe(mutation: Mutation) -> Result<TargetProbe, String> {
    let (definition, ability) = candidate_values(mutation);
    let (mut game, _) = new_game(definition, ability)?;
    let source = game
        .put_on_battlefield(PlayerId(0), CARD_ID)
        .map_err(|e| e.to_string())?;
    let unrelated = game
        .put_on_battlefield(PlayerId(1), WATCHWOLF)
        .map_err(|e| e.to_string())?;
    game.set_entered_turn_for_setup(source, 0)
        .map_err(|e| e.to_string())?;
    game.set_entered_turn_for_setup(unrelated, 0)
        .map_err(|e| e.to_string())?;
    game.begin_game().map_err(|e| e.to_string())?;
    for _ in 0..24 {
        if game.step == Step::PrecombatMain && game.priority == PlayerId(0) {
            break;
        }
        let player = game.priority;
        game.pass_priority(player).map_err(|e| e.to_string())?;
    }
    if game.step != Step::PrecombatMain || game.priority != PlayerId(0) {
        return Err("target probe could not reach seat-zero main phase".to_owned());
    }
    let events_before = game.event_log.clone();
    let before_source = game.object(source).map_err(|e| e.to_string())?.tapped;
    let before_damage = game.object(unrelated).map_err(|e| e.to_string())?.damage;
    let action = AbilityActivation {
        source,
        ability_id: ABILITY_ID,
        sacrifice_sources: vec![],
        additional_tap_creatures: vec![],
        discard_cards: vec![],
        targets: vec![Target::Permanent(unrelated)],
    };
    let result = game.activate_ability(PlayerId(0), action);
    let accepted = result.is_ok();
    let rejection = result
        .err()
        .map_or_else(String::new, |error| format!("{error:?}"));
    let after_view = game
        .view_for_player(PlayerId(0))
        .map_err(|e| e.to_string())?;
    let unchanged = events_before == game.event_log
        && before_source == game.object(source).map_err(|e| e.to_string())?.tapped
        && before_damage == game.object(unrelated).map_err(|e| e.to_string())?.damage
        && after_view.stack_depth == 0;
    Ok(TargetProbe {
        accepted,
        rejection,
        unchanged,
    })
}

fn target_boundary_mutation_detected() -> Result<bool, String> {
    let reference = run_target_probe(Mutation::None)?;
    let broadened = run_target_probe(Mutation::BroadTarget)?;
    Ok(!reference.accepted
        && reference.unchanged
        && reference.rejection.contains("IllegalTarget")
        && broadened.accepted)
}

#[derive(Clone, Debug)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    sagittars: Option<ObjectId>,
    watchwolf: Option<ObjectId>,
    hawk: Option<ObjectId>,
    elf: Option<ObjectId>,
    sag_payment: Vec<Color>,
    activation_target: Option<ObjectId>,
    activation_turn: Option<u32>,
    sag_tapped_on_activation: bool,
    sag_zone: Option<Zone>,
    sag_pt: Option<(i32, i32)>,
    hawk_zone: Option<Zone>,
    elf_zone: Option<Zone>,
    p0_life: i64,
    p1_life: i64,
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let (definition, ability) = candidate_values(mutation);
    let (mut game, _definitions) = new_game(definition, ability)?;
    let p0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: WATCHWOLF.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 24,
            },
        ],
        sideboard: vec![],
    };
    let p1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: ELF.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: HAWK.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 24,
            },
        ],
        sideboard: vec![],
    };
    if p0
        .mainboard
        .iter()
        .map(|entry| entry.count as usize)
        .sum::<usize>()
        != 60
        || p1
            .mainboard
            .iter()
            .map(|entry| entry.count as usize)
            .sum::<usize>()
            != 60
    {
        return Err("Sagittars games require shuffled 60-card decks".to_owned());
    }
    game.set_shuffle_seed(seed).map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(0), &p0)
        .map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(1), &p1)
        .map_err(|e| e.to_string())?;
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|e| e.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|e| e.to_string())?;
    game.begin_game().map_err(|e| e.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(SagittarsPolicy::new(PlayerId(0))),
        Box::new(SagittarsPolicy::new(PlayerId(1))),
    ];
    let mut attempted = 0u32;
    let mut accepted = 0u32;
    let mut activation_target = None;
    let mut activation_turn = None;
    let mut sag_tapped = false;
    let mut sagittars = None;
    let mut watchwolf = None;
    let mut hawk = None;
    let mut elf = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(
                "Selesnya Sagittars game exceeded bounded turn or action budget".to_owned(),
            );
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
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
        if let PolicyAction::CastWithPayment { request, .. } = &action {
            if let Some(card) = view.hand.iter().find(|card| card.id == request.card) {
                match (player, card.definition) {
                    (PlayerId(0), Some(CARD_ID)) => sagittars = Some(request.card),
                    (PlayerId(0), Some(WATCHWOLF)) => watchwolf = Some(request.card),
                    (PlayerId(1), Some(HAWK)) => hawk = Some(request.card),
                    (PlayerId(1), Some(ELF)) => elf = Some(request.card),
                    _ => {}
                }
            }
        }
        let activation = match &action {
            PolicyAction::ActivateAbility { activation } if activation.ability_id == ABILITY_ID => {
                Some((
                    activation.source,
                    activation.targets.iter().find_map(|target| match target {
                        Target::Permanent(object) => Some(*object),
                        _ => None,
                    }),
                ))
            }
            _ => None,
        };
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Sagittars policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("Sagittars invariant failed: {error}"))?;
        if let Some((source, target)) = activation {
            if activation_turn.is_none() {
                sag_tapped = game.object(source).is_ok_and(|object| object.tapped);
                activation_target = target;
                activation_turn = Some(view.turn);
            }
        }
    }
    let events = game.event_log.clone();
    let sag_payment = sagittars
        .and_then(|object| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if *card == object => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    let sag_zone = sagittars.and_then(|object| final_zone(&events, object));
    let sag_pt = sagittars.and_then(|object| {
        let characteristics = game.characteristics(object).ok()?;
        Some((characteristics.power?, characteristics.toughness?))
    });
    let hawk_zone = hawk.and_then(|object| final_zone(&events, object));
    let elf_zone = elf.and_then(|object| final_zone(&events, object));
    let p0_life = game.player(PlayerId(0)).map_err(|e| e.to_string())?.life;
    let p1_life = game.player(PlayerId(1)).map_err(|e| e.to_string())?.life;
    Ok(Captured {
        events,
        winner: game.winner().map(|player| player.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        sagittars,
        watchwolf,
        hawk,
        elf,
        sag_payment,
        activation_target,
        activation_turn,
        sag_tapped_on_activation: sag_tapped,
        sag_zone,
        sag_pt,
        hawk_zone,
        elf_zone,
        p0_life,
        p1_life,
    })
}

fn final_zone(events: &[GameEvent], object: ObjectId) -> Option<Zone> {
    events.iter().rev().find_map(|event| match event {
        GameEvent::CardMoved { card, to } if *card == object => Some(*to),
        _ => None,
    })
}

fn new_game(
    definition: CardDefinition,
    ability: ActivatedAbility,
) -> Result<(Game, Vec<CardDefinition>), String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|item| item.id == CARD_ID)
        .ok_or("Sagittars definition missing from RAV registry")? = definition;
    let mut activated = rav_activated_ability_bindings();
    activated
        .iter_mut()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("Sagittars ability binding missing from RAV registry")?
        .ability = ability;
    let mut triggers = rav_triggered_ability_bindings();
    triggers.extend(attachment_triggers());
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions.clone(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        activated,
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
    game.register_replacement_effect_bindings(rav_replacement_effect_bindings())
        .map_err(|e| e.to_string())?;
    game.register_damage_replacement_effect_bindings(rav_damage_replacement_effect_bindings())
        .map_err(|e| e.to_string())?;
    Ok((game, definitions))
}

struct SagittarsPolicy {
    player: PlayerId,
    sagittars_cast: bool,
    watchwolf_cast: bool,
    elf_cast: bool,
    hawk_cast: bool,
    activation_offered: bool,
}

impl SagittarsPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            sagittars_cast: false,
            watchwolf_cast: false,
            elf_cast: false,
            hawk_cast: false,
            activation_offered: false,
        }
    }

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let forest_count = view
            .own_battlefield
            .iter()
            .filter(|card| {
                card.basic_land_type == Some(cardbench_magic_engine::BasicLandType::Forest)
            })
            .count();
        let plains_count = view
            .own_battlefield
            .iter()
            .filter(|card| {
                card.basic_land_type == Some(cardbench_magic_engine::BasicLandType::Plains)
            })
            .count();
        let prefer = if forest_count <= plains_count {
            Color::Green
        } else {
            Color::White
        };
        view.hand
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&prefer)
            })
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            })
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }

    fn tap_land(&self, view: &GameView, color: Color) -> Option<PolicyAction> {
        let land = view.own_battlefield.iter().find(|card| {
            !card.tapped
                && card.card_types.contains(&CardType::Land)
                && card.mana_colors.contains(&color)
        })?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }

    fn payment_selection(&self, view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let mut available: BTreeMap<Color, u8> = Color::ALL
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
        if !cost.hybrid.is_empty() {
            return None;
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
        Some(ManaPaymentSelection {
            generic,
            hybrid: vec![],
        })
    }

    fn cast_or_make_mana(
        &mut self,
        view: &GameView,
        definition: &str,
        cost: &ManaCost,
    ) -> Option<PolicyAction> {
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(definition))?;
        if let Some(mana_selection) = self.payment_selection(view, cost) {
            match definition {
                CARD_ID => self.sagittars_cast = true,
                WATCHWOLF => self.watchwolf_cast = true,
                ELF => self.elf_cast = true,
                HAWK => self.hawk_cast = true,
                _ => {}
            }
            return Some(PolicyAction::CastWithPayment {
                request: CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection,
            });
        }
        let missing_color = cost
            .colored
            .iter()
            .find(|color| view.mana_pool.amount(**color) == 0)
            .copied();
        let total = u16::from(cost.generic) + cost.colored.len() as u16 + cost.hybrid.len() as u16;
        if let Some(color) = missing_color.or_else(|| {
            (u16::from(view.mana_pool.total()) < total)
                .then(|| cost.colored.first().copied().unwrap_or(Color::Green))
        }) {
            return self.tap_land(view, color).or_else(|| {
                let land = view
                    .own_battlefield
                    .iter()
                    .find(|card| !card.tapped && card.card_types.contains(&CardType::Land))?;
                let color = *land.mana_colors.iter().next()?;
                Some(PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color,
                })
            });
        }
        None
    }

    fn sagittars<'a>(&self, view: &'a GameView) -> Option<&'a cardbench_magic_engine::CardView> {
        view.own_battlefield
            .iter()
            .find(|card| card.definition == Some(CARD_ID))
    }
}

impl CodePolicy for SagittarsPolicy {
    fn id(&self) -> &'static str {
        "rav.selesnya-sagittars-full-game.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = if self.player == PlayerId(0) {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.can_attack && card.definition == Some(WATCHWOLF))
                    .map(|card| card.id)
                    .collect()
            } else {
                let sagittars_ready = view
                    .opponent_battlefield
                    .iter()
                    .any(|card| card.definition == Some(CARD_ID) && !card.summoning_sick);
                let hawk_ready = view
                    .own_battlefield
                    .iter()
                    .any(|card| card.definition == Some(HAWK) && card.can_attack);
                let elf_ready = view
                    .own_battlefield
                    .iter()
                    .any(|card| card.definition == Some(ELF) && card.can_attack);
                if sagittars_ready && hawk_ready && elf_ready {
                    view.own_battlefield
                        .iter()
                        .filter(|card| {
                            card.can_attack
                                && (card.definition == Some(HAWK) || card.definition == Some(ELF))
                        })
                        .map(|card| card.id)
                        .collect()
                } else {
                    vec![]
                }
            };
            return PolicyAction::DeclareAttackers { attackers };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            let assignment = if self.player == PlayerId(0) {
                let attacker = view
                    .combat_attackers
                    .iter()
                    .find(|card| card.definition == Some(HAWK));
                let blocker = self.sagittars(view).filter(|card| card.can_block);
                attacker
                    .zip(blocker)
                    .map(|(attacker, blocker)| CombatBlock {
                        attacker: attacker.id,
                        blocker: blocker.id,
                    })
            } else {
                None
            };
            return PolicyAction::DeclareBlockers {
                assignments: assignment.into_iter().collect(),
            };
        }
        if view.decision_player != self.player || view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && view.blockers_declared
            && !self.activation_offered
        {
            if let Some(source) = self
                .sagittars(view)
                .filter(|card| !card.tapped && !card.summoning_sick)
                .map(|card| card.id)
            {
                if let Some(target) = view
                    .combat_attackers
                    .iter()
                    .find(|card| card.definition == Some(ELF))
                    .map(|card| card.id)
                {
                    self.activation_offered = true;
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
            }
        }
        if view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.play_land(view) {
            return action;
        }
        if self.player == PlayerId(0) {
            if !self.sagittars_cast {
                if let Some(action) = self.cast_or_make_mana(
                    view,
                    CARD_ID,
                    &ManaCost::with_colors(3, [Color::Green, Color::White]),
                ) {
                    return action;
                }
            }
            if !self.watchwolf_cast {
                if let Some(action) = self.cast_or_make_mana(
                    view,
                    WATCHWOLF,
                    &ManaCost::with_colors(0, [Color::Green, Color::White]),
                ) {
                    return action;
                }
            }
        } else {
            if !self.elf_cast {
                if let Some(action) =
                    self.cast_or_make_mana(view, ELF, &ManaCost::with_colors(0, [Color::Green]))
                {
                    return action;
                }
            }
            if !self.hawk_cast {
                if let Some(action) =
                    self.cast_or_make_mana(view, HAWK, &ManaCost::with_colors(1, [Color::White]))
                {
                    return action;
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.winner != Some(0) || captured.rejected != 0 {
        return Err(
            "Selesnya Sagittars full game did not end cleanly in seat zero's win".to_owned(),
        );
    }
    let sagittars = captured
        .sagittars
        .ok_or("seat zero did not cast Selesnya Sagittars")?;
    let watchwolf = captured
        .watchwolf
        .ok_or("seat zero did not cast a Watchwolf finisher")?;
    let hawk = captured.hawk.ok_or("seat one did not cast Courier Hawk")?;
    let elf = captured
        .elf
        .ok_or("seat one did not cast Elves of Deep Shadow")?;
    if captured.sag_payment.len() != 5
        || !captured.sag_payment.contains(&Color::Green)
        || !captured.sag_payment.contains(&Color::White)
    {
        return Err(
            "Selesnya Sagittars was not paid with its three generic, green, and white mana"
                .to_owned(),
        );
    }
    if captured.activation_target != Some(elf) || !captured.sag_tapped_on_activation {
        return Err("Sagittars did not tap to target the attacking Elf".to_owned());
    }
    if captured.activation_turn.is_none()
        || captured.sag_zone != Some(Zone::Battlefield)
        || captured.sag_pt != Some((2, 5))
        || captured.elf_zone != Some(Zone::Graveyard)
        || captured.hawk_zone != Some(Zone::Graveyard)
    {
        let casts: Vec<String> = captured
            .events
            .iter()
            .filter_map(|event| match event {
                GameEvent::SpellCast { player, card } => Some(format!("{player:?}:{card:?}")),
                _ => None,
            })
            .collect();
        let combat: Vec<String> = captured
            .events
            .iter()
            .filter_map(|event| match event {
                GameEvent::AttackersDeclared { player, attackers } => {
                    Some(format!("{player:?} atk {attackers:?}"))
                }
                GameEvent::BlockersDeclared {
                    player,
                    assignments,
                } => Some(format!("{player:?} blk {assignments:?}")),
                GameEvent::DamageDealtToPermanent {
                    source,
                    permanent,
                    amount,
                } => Some(format!("dmg {source:?}->{permanent:?} {amount}")),
                GameEvent::CardMoved { card, to } if *card == hawk || *card == elf => {
                    Some(format!("move {card:?} {to:?}"))
                }
                _ => None,
            })
            .collect();
        return Err(format!("Sagittars activation/combat mismatch: turn={:?} sag={:?}/{:?} elf={:?} hawk={:?} target={:?} casts={casts:?} combat={combat:?}",captured.activation_turn,captured.sag_zone,captured.sag_pt,captured.elf_zone,captured.hawk_zone,captured.activation_target));
    }
    if captured.p1_life > 0 || captured.p0_life <= 0 {
        return Err("the legal full game did not reach seat zero's ordinary win".to_owned());
    }
    let target_probe = run_target_probe(Mutation::None)?;
    if target_probe.accepted
        || !target_probe.unchanged
        || !target_probe.rejection.contains("IllegalTarget")
    {
        return Err("noncombat creature target probe was not rejected atomically".to_owned());
    }
    let events = &captured.events;
    let sag_paid = event_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == sagittars && colors.len() == 5 && colors.contains(&Color::Green) && colors.contains(&Color::White))).ok_or("Sagittars colored cast payment receipt is missing")?;
    let sag_cast = event_after(events, sag_paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == sagittars)).ok_or("Sagittars SpellCast receipt is missing")?;
    let sag_resolved = event_after(
        events,
        sag_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == sagittars),
    )
    .ok_or("Sagittars SpellResolved receipt is missing")?;
    let sag_entered = event_after(events, sag_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == sagittars)).ok_or("Sagittars battlefield entry is missing")?;
    let elf_cast = event_index(
        events,
        |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == elf),
    )
    .ok_or("Elf cast receipt is missing")?;
    let hawk_cast = event_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == hawk)).ok_or("Hawk cast receipt is missing")?;
    let attacks = event_after(events, sag_entered, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(1), attackers } if attackers.contains(&hawk) && attackers.contains(&elf))).ok_or("Hawk and Elf did not attack together after Sagittars entered")?;
    let blocks = event_after(events, attacks, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(0), assignments } if assignments.contains(&(hawk, sagittars)))).ok_or("Reach did not legally block the attacking flying Hawk")?;
    let activated = event_after(events, blocks, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source, definition: CARD_ID, ability: ABILITY_ID, .. } if *source == sagittars)).ok_or("Sagittars ability was not stacked")?;
    let ping = event_after(events, activated, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 1 } if *source == sagittars && *permanent == elf)).ok_or("Sagittars did not deal exactly one to the attacking Elf")?;
    let resolved = event_after(events, ping, |event| matches!(event, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == sagittars)).ok_or("Sagittars ability did not resolve")?;
    let elf_sba = event_after(events, ping, |event| matches!(event, GameEvent::StateBasedAction { card, reason: "creature has lethal damage" } if *card == elf)).ok_or("attacking Elf did not die from activation damage")?;
    let elf_died = event_after(
        events,
        elf_sba,
        |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == elf),
    )
    .ok_or("attacking Elf did not enter its graveyard")?;
    let hawk_damage = event_after(events, elf_died, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 1 } if *source == hawk && *permanent == sagittars)).ok_or("blocked Hawk did not deal its combat damage")?;
    let sag_damage = event_after(events, hawk_damage, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 2 } if *source == sagittars && *permanent == hawk)).ok_or("Reach blocker did not assign its combat damage")?;
    let hawk_sba = event_after(events, sag_damage, |event| matches!(event, GameEvent::StateBasedAction { card, reason: "creature has lethal damage" } if *card == hawk)).ok_or("blocked Hawk was not destroyed by combat")?;
    let hawk_died = event_after(events, hawk_sba, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == hawk)).ok_or("blocked Hawk did not enter graveyard")?;
    let later_attack = event_after(events, hawk_died, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&watchwolf))).ok_or("Watchwolf did not press the game after combat")?;
    let lethal = event_after(events, later_attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), .. } if *source == watchwolf)).ok_or("Watchwolf did not deal later combat damage")?;
    let ended = event_after(events, lethal, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal seat-zero win event is missing")?;
    if !(sag_paid < sag_cast
        && sag_cast < sag_resolved
        && sag_resolved < sag_entered
        && elf_cast < attacks
        && hawk_cast < attacks
        && attacks < blocks
        && blocks < activated
        && activated < ping
        && ping < resolved
        && resolved < elf_sba
        && elf_sba < elf_died
        && elf_died < hawk_damage
        && hawk_damage < sag_damage
        && sag_damage < hawk_sba
        && hawk_sba < hawk_died
        && hawk_died < later_attack
        && later_attack < lethal
        && lethal < ended)
    {
        return Err(
            "Sagittars typed event order is inconsistent with the legal combat trace".to_owned(),
        );
    }
    let selected = BTreeSet::from([
        sag_paid,
        sag_cast,
        sag_resolved,
        sag_entered,
        attacks,
        blocks,
        activated,
        ping,
        resolved,
        elf_sba,
        elf_died,
        hawk_damage,
        sag_damage,
        hawk_sba,
        hawk_died,
        later_attack,
        lethal,
        ended,
    ]);
    let projected: Vec<ProjectedEvent> = project_events(events)
        .into_iter()
        .enumerate()
        .filter_map(|(index, event)| selected.contains(&index).then_some(event))
        .collect();
    let kinds: Vec<&str> = projected.iter().map(|event| event.kind.as_str()).collect();
    let expected = [
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "AttackersDeclared",
        "BlockersDeclared",
        "AbilityActivated",
        "DamageDealtToPermanent",
        "AbilityResolved",
        "StateBasedAction",
        "CardMoved",
        "DamageDealtToPermanent",
        "DamageDealtToPermanent",
        "StateBasedAction",
        "CardMoved",
        "AttackersDeclared",
        "DamageDealtToPlayer",
        "GameEnded",
    ];
    if kinds != expected {
        return Err(format!(
            "Sagittars typed event projection differs: {kinds:?}"
        ));
    }
    Ok(json!({
        "trace_id":"rav-selesnya-sagittars-reach-and-tap-damage-pvp", "shuffle_seed":seed,
        "decks":["rav_selesnya_sagittars_watchwolf_forest_plains","rav_hawk_elf_forest_plains"],
        "policies":["rav.selesnya-sagittars-full-game.v1","rav.selesnya-sagittars-full-game.v1"],
        "terminal":format!("Winner(PlayerId({}))",captured.winner.unwrap_or(usize::MAX)),"winner_seat":captured.winner,
        "turns":captured.turns,"accepted_moves":captured.accepted,"rejected_moves":captured.rejected,
        "sagittars":{"object":sagittars.0,"mana_cost":"{3}{G}{W}","mana_paid":captured.sag_payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"zone":"Battlefield","power":captured.sag_pt.map(|pt|pt.0),"toughness":captured.sag_pt.map(|pt|pt.1)},
        "reach":{"attacking_flying_hawk":hawk.0,"legal_block":true,"blocker":sagittars.0,"hawk_zone_after_combat":"Graveyard"},
        "activation":{"ability":ABILITY_ID,"source":sagittars.0,"target":elf.0,"target_was_attacking":true,"damage":1,"tap_cost_paid":captured.sag_tapped_on_activation,"target_zone_after_resolution":"Graveyard"},
        "legality_probe":{"target_definition":WATCHWOLF,"target_was_attacking_or_blocking":false,"rejected_by_engine":true,"match_state_unchanged":target_probe.unchanged,"rejection":target_probe.rejection},
        "finisher":{"watchwolf":watchwolf.0,"p1_life":captured.p1_life,"p0_life":captured.p0_life},
        "events":projected,"passed":true
    }))
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn event_after(
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
fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}
fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"engine_status":if card.semantic_status.is_executable(){"executable_compatibility_slice"}else{"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"qualification":if passed{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
}
