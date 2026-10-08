//! Full-game diagnostic authority for RAV Votary of the Conclave.

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
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
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Meta, Stmt, Type, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-VOTARY-OF-THE-CONCLAVE";
const BRAIN_SPOIL_ID: &str = "RAV-BRAINSPOIL";
const REGEN_ABILITY: &str = "regenerate-target-creature";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/votary_of_the_conclave.rs";
const MAX_TRACE_SEED: u64 = 128;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/votary_of_the_conclave.rs"]
mod candidate_votary;

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
            eprintln!("Votary of the Conclave PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_bindings()?;
    let catalog = unique_catalog()?;
    let seed = find_trace_seed()?;
    let capture = run_match(seed, false)?;
    let trace = validate_trace(&capture, seed)?;
    let replay = run_match(seed, false)?;
    if !same_match(&capture, &replay) {
        return Err("Votary shuffled full-game replay was not deterministic".to_owned());
    }
    let wrong_cost_detected = run_match(seed, true)
        .and_then(|mutant| validate_trace(&mutant, seed).map(|_| mutant))
        .is_err();
    if !wrong_cost_detected {
        return Err(
            "Votary's compiled wrong-color regeneration mutant escaped the game trace".to_owned(),
        );
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
        "card": {"collector_number": 35, "definition_id": CARD_ID, "name": "Votary of the Conclave"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "105.2 and 202.1-202.2: colored mana costs and payment",
                "117: timing and priority",
                "302.1 and 601.2: creature casting and resolution",
                "608.2: resolving a targeted spell and activated ability",
                "701.15: regeneration",
                "614.8: a regeneration shield replaces destruction"
            ],
            "source_contract": "RAV Votary of the Conclave is a white 1/1 creature costing {W}; {G}: regenerate target creature",
            "authority_cutoff": "2026-09-23"
        },
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
        "sealed_authority": false,
        "qualification_status": "diagnostic_only; external qualification pending",
        "mutation_sensitivity": {
            "wrong_regeneration_color_mutation_detected_in_game": wrong_cost_detected,
            "sealed_mutation_authority": false
        },
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
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
        return Err("RAV catalog identity or printing denominator changed".to_owned());
    }
    Ok(catalog)
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Votary source: {error}"))?;
    let file =
        syn::parse_file(&source).map_err(|_| "Votary source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
    {
        return Err("Votary source contains an unsupported module attribute".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["cardbench_magic_engine", "ActivatedAbility"],
        vec!["cardbench_magic_engine", "ActivatedAbilityBinding"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TargetRequirement"],
        vec!["std", "collections", "BTreeSet"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut actual_imports = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(item.vis, Visibility::Inherited) =>
            {
                flatten_use_paths(&mut Vec::new(), &item.tree, &mut actual_imports)?;
            }
            Item::Fn(function) => {
                if !functions.insert(function.sig.ident.to_string()) {
                    return Err("Votary source contains duplicate functions".to_owned());
                }
                validate_safe_function(function)?;
            }
            _ => return Err("Votary source contains an unsupported top-level item".to_owned()),
        }
    }
    if actual_imports != expected_imports
        || functions != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()])
    {
        return Err(
            "Votary imports or source function set differ from the task contract".to_owned(),
        );
    }
    Ok(())
}

fn flatten_use_paths(
    prefix: &mut Vec<String>,
    tree: &UseTree,
    output: &mut BTreeSet<Vec<String>>,
) -> Result<(), String> {
    match tree {
        UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            flatten_use_paths(prefix, &path.tree, output)?;
            prefix.pop();
        }
        UseTree::Name(name) => {
            prefix.push(name.ident.to_string());
            output.insert(prefix.clone());
            prefix.pop();
        }
        UseTree::Group(group) => {
            for item in &group.items {
                flatten_use_paths(prefix, item, output)?;
            }
        }
        UseTree::Rename(_) | UseTree::Glob(_) => {
            return Err("Votary source imports may not rename or glob".to_owned());
        }
    }
    Ok(())
}

fn validate_safe_function(function: &syn::ItemFn) -> Result<(), String> {
    let attrs_ok = function.attrs.iter().all(|attribute| {
        attribute.path().is_ident("doc")
            || attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_))
    });
    let visibility_ok = matches!(
        &function.vis,
        Visibility::Restricted(visibility) if visibility.path.is_ident("crate")
    );
    let name = function.sig.ident.to_string();
    let return_type = if name == "definition" {
        "CardDefinition"
    } else if name == "activated_ability" {
        "ActivatedAbilityBinding"
    } else {
        return Err("Votary source has an unexpected function".to_owned());
    };
    if !attrs_ok
        || !visibility_ok
        || function.sig.constness.is_some()
        || function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
        || function.sig.abi.is_some()
        || !function.sig.generics.params.is_empty()
        || function.sig.generics.where_clause.is_some()
        || !function.sig.inputs.is_empty()
        || function.sig.variadic.is_some()
        || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if type_is(ty, return_type))
        || function.block.stmts.len() != 1
    {
        return Err(format!(
            "Votary {name} must be a plain literal-returning function"
        ));
    }
    let Stmt::Expr(expression, None) = &function.block.stmts[0] else {
        return Err(format!("Votary {name} must return a single expression"));
    };
    if !safe_expr(expression) {
        return Err(format!(
            "Votary {name} contains executable code outside the literal contract"
        ));
    }
    Ok(())
}

fn type_is(ty: &Type, expected: &str) -> bool {
    matches!(ty, Type::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 && path.path.segments[0].ident == expected && matches!(path.path.segments[0].arguments, syn::PathArguments::None))
}

fn safe_expr(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(literal) => literal.attrs.is_empty(),
        Expr::Path(path) => {
            path.attrs.is_empty()
                && path.qself.is_none()
                && [
                    &["Color", "White"][..],
                    &["Color", "Green"],
                    &["Color", "Black"],
                    &["CardType", "Creature"],
                    &["TargetRequirement", "Creature"],
                    &["Effect", "RegenerateTargetCreature"],
                ]
                .iter()
                .any(|expected| path_is(&path.path, expected))
        }
        Expr::Array(array) => array.attrs.is_empty() && array.elems.iter().all(safe_expr),
        Expr::Reference(reference) => {
            reference.attrs.is_empty()
                && reference.mutability.is_none()
                && safe_expr(&reference.expr)
        }
        Expr::Call(call) => {
            if !call.attrs.is_empty() || !call.args.iter().all(safe_expr) {
                return false;
            }
            let Expr::Path(path) = &*call.func else {
                return false;
            };
            [
                &["BTreeSet", "from"][..],
                &["BTreeSet", "new"],
                &["ManaCost", "with_colors"],
                &["ManaCost", "new"],
                &["Some"],
            ]
            .iter()
            .any(|expected| path_is(&path.path, expected))
        }
        Expr::Struct(structure) => {
            if !structure.attrs.is_empty()
                || structure.rest.is_some()
                || ![
                    &["CardDefinition"][..],
                    &["ActivatedAbilityBinding"],
                    &["ActivatedAbility"],
                ]
                .iter()
                .any(|expected| path_is(&structure.path, expected))
            {
                return false;
            }
            structure.fields.iter().all(|field| {
                field.attrs.is_empty()
                    && matches!(field.member, syn::Member::Named(_))
                    && field.colon_token.is_some()
                    && safe_expr(&field.expr)
            })
        }
        Expr::Macro(mac) => {
            mac.attrs.is_empty()
                && mac.mac.path.is_ident("vec")
                && (mac.mac.tokens.is_empty()
                    || mac.mac.tokens.to_string() == "Effect :: RegenerateTargetCreature"
                    || mac.mac.tokens.to_string() == "TargetRequirement :: Creature")
        }
        _ => false,
    }
}

fn path_is(path: &syn::Path, expected: &[&str]) -> bool {
    path.segments.len() == expected.len()
        && path
            .segments
            .iter()
            .zip(expected)
            .all(|(segment, expected)| {
                segment.ident == *expected && matches!(segment.arguments, syn::PathArguments::None)
            })
}

fn validate_candidate_bindings() -> Result<(), String> {
    let submitted = candidate_votary::definition();
    let runtime = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or_else(|| "Votary is missing from the executable RAV registry".to_owned())?;
    if submitted != runtime
        || submitted.name != "Votary of the Conclave"
        || submitted.set_code != "RAV"
        || submitted.mana_cost != ManaCost::with_colors(0, [Color::White])
        || submitted.colors != BTreeSet::from([Color::White])
        || !submitted.mana_colors.is_empty()
        || submitted.card_types != BTreeSet::from([CardType::Creature])
        || submitted.is_basic_land
        || submitted.power != Some(1)
        || submitted.toughness != Some(1)
        || !submitted.keywords.is_empty()
        || !submitted.effects.is_empty()
    {
        return Err("Votary identity, cost, or printed characteristics differ".to_owned());
    }
    let submitted_ability = candidate_votary::activated_ability();
    let runtime_ability = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == REGEN_ABILITY)
        .ok_or_else(|| "Votary regeneration binding is missing from the RAV registry".to_owned())?;
    if submitted_ability != runtime_ability
        || submitted_ability.ability.mana_cost != ManaCost::with_colors(0, [Color::Green])
        || submitted_ability.ability.targets
            != vec![cardbench_magic_engine::TargetRequirement::Creature]
        || submitted_ability.ability.effects != vec![Effect::RegenerateTargetCreature]
        || submitted_ability.ability.tap_cost
        || submitted_ability.ability.sorcery_speed
        || submitted_ability.ability.sacrifice_source
    {
        return Err("Votary targeted green regeneration binding differs".to_owned());
    }
    Ok(())
}

fn find_trace_seed() -> Result<u64, String> {
    if let Ok(value) = std::env::var("CARDBENCH_VOTARY_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_VOTARY_SEED must be an unsigned integer".to_owned())?;
        validate_trace(&run_match(seed, false)?, seed)?;
        return Ok(seed);
    }
    for seed in 0..=MAX_TRACE_SEED {
        if run_match(seed, false)
            .and_then(|capture| validate_trace(&capture, seed).map(|_| capture))
            .is_ok()
        {
            return Ok(seed);
        }
    }
    Err(format!(
        "no complete Votary regeneration trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

#[derive(Clone)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    votary: Option<ObjectId>,
    brainspoil: Option<ObjectId>,
    brainspoil_target: Option<ObjectId>,
    votary_payment: Vec<Color>,
    brainspoil_payment: Vec<Color>,
    ability_payment: Option<ManaCost>,
    ability_incarnation: Option<u64>,
    ability_resolved_incarnation: Option<u64>,
    ability_target: Option<ObjectId>,
    shield_created: bool,
    shield_used: bool,
    tapped_after_shield: bool,
    on_battlefield_after_shield: bool,
    destroyed: bool,
    live_stats: Option<(i32, i32)>,
}

fn run_match(seed: u64, wrong_regeneration_color: bool) -> Result<CapturedMatch, String> {
    let mut game = new_rav_game(wrong_regeneration_color)?;
    let deck0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 12,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 44,
            },
        ],
        sideboard: vec![],
    };
    let deck1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: BRAIN_SPOIL_ID.to_owned(),
                count: 1,
            },
            DeckEntry {
                card: "RAV-SWAMP".to_owned(),
                count: 59,
            },
        ],
        sideboard: vec![],
    };
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 90,
        max_policy_moves: 16_000,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &deck0)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &deck1)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(VotaryPolicy::new(PlayerId(0), wrong_regeneration_color)),
        Box::new(BogusRemovalPolicy::new(PlayerId(1))),
    ];
    let mut capture = CapturedMatch {
        events: Vec::new(),
        terminal: String::new(),
        winner: None,
        turns: 0,
        accepted_moves: 0,
        rejected_moves: 0,
        votary: None,
        brainspoil: None,
        brainspoil_target: None,
        votary_payment: vec![],
        brainspoil_payment: vec![],
        ability_payment: None,
        ability_incarnation: None,
        ability_resolved_incarnation: None,
        ability_target: None,
        shield_created: false,
        shield_used: false,
        tapped_after_shield: false,
        on_battlefield_after_shield: false,
        destroyed: false,
        live_stats: None,
    };
    while !game.is_game_over() {
        if game.turn > config.max_turns || capture.accepted_moves >= config.max_policy_moves {
            return Err(format!(
                "Votary trace hit its bound at turn {}: {:?}",
                game.turn, game.step
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("policy seat unavailable")?;
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
        let before = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Votary policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        capture.accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        observe(&game, &game.event_log[before..], &mut capture)?;
    }
    capture.events = game.event_log.clone();
    capture.winner = game.winner().map(|player| player.0);
    capture.terminal = capture.winner.map_or_else(
        || "Draw".to_owned(),
        |seat| format!("Winner(PlayerId({seat}))"),
    );
    capture.turns = game.turn;
    if let Some(votary) = capture.votary
        && game.zone_of(votary) == Some(Zone::Battlefield)
    {
        let characteristics = game
            .characteristics(votary)
            .map_err(|error| error.to_string())?;
        capture.live_stats = Some((
            characteristics.power.ok_or("Votary has no power")?,
            characteristics.toughness.ok_or("Votary has no toughness")?,
        ));
    }
    Ok(capture)
}

fn observe(game: &Game, events: &[GameEvent], capture: &mut CapturedMatch) -> Result<(), String> {
    for event in events {
        match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                capture.votary = Some(*card);
                capture.votary_payment = colors.clone();
            }
            GameEvent::SpellManaPaid {
                player: PlayerId(1),
                card,
                colors,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == BRAIN_SPOIL_ID) =>
            {
                capture.brainspoil = Some(*card);
                capture.brainspoil_payment = colors.clone();
            }
            GameEvent::SpellCast {
                player: PlayerId(1),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == BRAIN_SPOIL_ID) =>
            {
                capture.brainspoil = Some(*card);
                capture.brainspoil_target =
                    game.stack
                        .last()
                        .and_then(|spell| match spell.targets.first() {
                            Some(Target::Permanent(target)) => Some(*target),
                            _ => None,
                        });
            }
            GameEvent::SpellResolved { card } if Some(*card) == capture.votary => {
                let stats = game
                    .characteristics(*card)
                    .map_err(|error| error.to_string())?;
                capture.live_stats = Some((
                    stats.power.ok_or("Votary has no live power")?,
                    stats.toughness.ok_or("Votary has no live toughness")?,
                ));
            }
            GameEvent::AbilityManaPaid {
                player: PlayerId(0),
                source,
                ability,
                mana_cost,
            } if Some(*source) == capture.votary && *ability == REGEN_ABILITY => {
                capture.ability_payment = Some(mana_cost.clone());
            }
            GameEvent::AbilityActivated {
                player: PlayerId(0),
                source,
                source_incarnation,
                ability,
                ..
            } if Some(*source) == capture.votary && *ability == REGEN_ABILITY => {
                capture.ability_incarnation = Some(*source_incarnation);
            }
            GameEvent::AbilityResolved {
                source,
                source_incarnation,
                ability,
            } if Some(*source) == capture.votary && *ability == REGEN_ABILITY => {
                capture.ability_resolved_incarnation = Some(*source_incarnation);
            }
            GameEvent::RegenerationShieldCreated { source, target }
                if Some(*source) == capture.votary =>
            {
                capture.shield_created = true;
                capture.ability_target = Some(*target);
            }
            GameEvent::RegenerationShieldUsed { target, .. } if Some(*target) == capture.votary => {
                capture.shield_used = true;
                capture.tapped_after_shield =
                    game.object(*target).is_ok_and(|object| object.tapped);
                capture.on_battlefield_after_shield =
                    game.zone_of(*target) == Some(Zone::Battlefield);
            }
            GameEvent::CardDestroyed { card, .. } if Some(*card) == capture.votary => {
                capture.destroyed = true
            }
            _ => {}
        }
    }
    Ok(())
}

fn new_rav_game(wrong_regeneration_color: bool) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Votary definition missing from RAV registry")? = candidate_votary::definition();
    let mut abilities = rav_activated_ability_bindings();
    abilities.retain(|binding| {
        !(binding.card_definition == CARD_ID && binding.ability.id == REGEN_ABILITY)
    });
    let mut regeneration = candidate_votary::activated_ability();
    if wrong_regeneration_color {
        regeneration.ability.mana_cost = ManaCost::with_colors(0, [Color::Black]);
    }
    abilities.push(regeneration);
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
    game.register_static_library_top_reveal_bindings(rav_static_library_top_reveal_bindings())
        .map_err(|error| error.to_string())?;
    game.register_static_attack_restrictions(rav_static_attack_restriction_bindings())
        .map_err(|error| error.to_string())?;
    game.register_static_creature_spell_cost_modifiers(
        rav_static_creature_spell_cost_modifier_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_attachment_bindings(rav_attachment_bindings())
        .map_err(|error| error.to_string())?;
    game.register_entry_copy_bindings(rav_entry_copy_bindings())
        .map_err(|error| error.to_string())?;
    game.register_entry_coin_flip_bindings(rav_entry_coin_flip_bindings())
        .map_err(|error| error.to_string())?;
    game.register_legendary_permanent_bindings(rav_legendary_permanent_bindings())
        .map_err(|error| error.to_string())?;
    game.register_static_entry_restriction_bindings(rav_static_entry_restriction_bindings())
        .map_err(|error| error.to_string())?;
    game.register_mana_ability_cost_bindings(rav_mana_ability_cost_bindings())
        .map_err(|error| error.to_string())?;
    game.register_cost_reduction_bindings(rav_cost_reduction_bindings())
        .map_err(|error| error.to_string())?;
    game.register_activated_ability_cost_modifier_bindings(
        rav_activated_ability_cost_modifier_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_generalized_activated_ability_cost_bindings(
        rav_generalized_activated_ability_cost_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_damage_replacement_effect_bindings(rav_damage_replacement_effect_bindings())
        .map_err(|error| error.to_string())?;
    game.register_replacement_effect_bindings(rav_replacement_effect_bindings())
        .map_err(|error| error.to_string())?;
    game.validate_invariants()
        .map_err(|error| error.to_string())?;
    Ok(game)
}

struct VotaryPolicy {
    player: PlayerId,
    wrong_regeneration_color: bool,
    votary_cast: bool,
    regeneration_activated: bool,
}

impl VotaryPolicy {
    fn new(player: PlayerId, wrong_regeneration_color: bool) -> Self {
        Self {
            player,
            wrong_regeneration_color,
            votary_cast: false,
            regeneration_activated: false,
        }
    }

    fn land(view: &GameView, preferred: &[&str]) -> Option<PolicyAction> {
        preferred.iter().find_map(|id| {
            view.hand
                .iter()
                .find(|card| card.definition == Some(*id))
                .map(|card| PolicyAction::PlayLand { card: card.id })
        })
    }

    fn mana_source(view: &GameView, color: Color) -> Option<PolicyAction> {
        view.own_battlefield
            .iter()
            .find(|land| {
                !land.tapped
                    && land.card_types.contains(&CardType::Land)
                    && land.mana_colors.contains(&color)
            })
            .map(|land| PolicyAction::ActivateManaAbility {
                land: land.id,
                color,
            })
    }

    fn cast_votary(card: ObjectId) -> PolicyAction {
        PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection {
                generic: vec![],
                hybrid: vec![],
            },
        }
    }

    fn regenerate(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.player != PlayerId(0)
            || self.regeneration_activated
            || !view
                .stack_spells
                .iter()
                .any(|spell| spell.definition == Some(BRAIN_SPOIL_ID))
        {
            return None;
        }
        let votary = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some(CARD_ID))?;
        let color = if self.wrong_regeneration_color {
            Color::Black
        } else {
            Color::Green
        };
        if view.mana_pool.amount(color) == 0 {
            return Self::mana_source(view, color);
        }
        self.regeneration_activated = true;
        Some(PolicyAction::ActivateAbility {
            activation: AbilityActivation {
                source: votary.id,
                ability_id: REGEN_ABILITY,
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![Target::Permanent(votary.id)],
            },
        })
    }
}

impl CodePolicy for VotaryPolicy {
    fn id(&self) -> &'static str {
        "rav.votary-controller.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = if self.player == PlayerId(0) && self.regeneration_activated {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(CARD_ID) && card.can_attack)
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
        if self.player == PlayerId(0)
            && view.player == self.player
            && view.priority == self.player
            && view.active_player == self.player
            && view.stack_depth == 0
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            if view.lands_played == 0 {
                let preferred: &[&str] = if self.votary_cast {
                    &["RAV-FOREST", "RAV-PLAINS"]
                } else {
                    &["RAV-PLAINS", "RAV-FOREST"]
                };
                if let Some(action) = Self::land(view, preferred) {
                    return action;
                }
            }
            if !self.votary_cast {
                if let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
                {
                    if view.mana_pool.amount(Color::White) > 0 {
                        self.votary_cast = true;
                        return Self::cast_votary(card.id);
                    }
                    if let Some(action) = Self::mana_source(view, Color::White) {
                        return action;
                    }
                }
            }
        }
        if let Some(action) = self.regenerate(view) {
            return action;
        }
        PolicyAction::PassPriority
    }
}

struct BogusRemovalPolicy {
    player: PlayerId,
    brainspoil_cast: bool,
}
impl BogusRemovalPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            brainspoil_cast: false,
        }
    }
}
impl CodePolicy for BogusRemovalPolicy {
    fn id(&self) -> &'static str {
        "rav.votary-brainspoil-opponent.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers { attackers: vec![] };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if self.player == PlayerId(1)
            && view.player == self.player
            && view.priority == self.player
            && view.active_player == self.player
            && view.stack_depth == 0
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            if view.lands_played == 0
                && let Some(land) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some("RAV-SWAMP"))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
            if !self.brainspoil_cast
                && let Some(target) = view
                    .opponent_battlefield
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
                && let Some(spell) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(BRAIN_SPOIL_ID))
                && view.mana_pool.amount(Color::Black) >= 5
            {
                self.brainspoil_cast = true;
                return PolicyAction::CastWithPayment {
                    request: cardbench_magic_engine::CastRequest {
                        card: spell.id,
                        targets: vec![Target::Permanent(target.id)],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: ManaPaymentSelection {
                        generic: vec![Color::Black; 3],
                        hybrid: vec![],
                    },
                };
            }
            if !self.brainspoil_cast
                && view
                    .hand
                    .iter()
                    .any(|card| card.definition == Some(BRAIN_SPOIL_ID))
                && view
                    .opponent_battlefield
                    .iter()
                    .any(|card| card.definition == Some(CARD_ID))
            {
                if let Some(action) = VotaryPolicy::mana_source(view, Color::Black) {
                    return action;
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(capture: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if capture.terminal != "Winner(PlayerId(0))"
        || capture.winner != Some(0)
        || capture.rejected_moves != 0
    {
        return Err("Votary full game did not finish in a clean P0 win".to_owned());
    }
    let votary = capture.votary.ok_or("Votary was not cast")?;
    let brainspoil = capture.brainspoil.ok_or("Brainspoil was not cast")?;
    if capture.votary_payment != [Color::White] {
        return Err(format!(
            "Votary {{W}} was not paid with exactly one white mana: {:?}",
            capture.votary_payment
        ));
    }
    if capture.brainspoil_payment.len() != 5
        || capture
            .brainspoil_payment
            .iter()
            .filter(|color| **color == Color::Black)
            .count()
            != 5
    {
        return Err(format!(
            "Brainspoil payment did not contain two black and three generic mana: {:?}",
            capture.brainspoil_payment
        ));
    }
    if capture.live_stats != Some((1, 1)) {
        return Err("resolved Votary is not a live 1/1 creature".to_owned());
    }
    if capture.brainspoil_target != Some(votary) {
        return Err("Brainspoil did not target the Votary".to_owned());
    }
    if capture
        .ability_payment
        .as_ref()
        .is_none_or(|cost| cost.generic != 0 || cost.colored != [Color::Green])
        || capture.ability_incarnation.is_none()
        || capture.ability_incarnation != capture.ability_resolved_incarnation
        || capture.ability_target != Some(votary)
        || !capture.shield_created
        || !capture.shield_used
        || !capture.tapped_after_shield
        || !capture.on_battlefield_after_shield
        || capture.destroyed
    {
        return Err(
            "Votary did not legally regenerate the targeted creature and survive destruction"
                .to_owned(),
        );
    }
    let events = select_events(&project_events(&capture.events), capture)?;
    let sequences: Vec<u64> = events.iter().map(|event| event.sequence).collect();
    if !sequences.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err("Votary typed trace event sequence is not strictly increasing".to_owned());
    }
    Ok(json!({
        "trace_id": "rav-votary-targeted-regeneration",
        "seed": seed,
        "decks": ["rav-votary-white-green-basics-60", "rav-brainspoil-swamp-60"],
        "policies": ["rav.votary-controller.v1", "rav.votary-brainspoil-opponent.v1"],
        "terminal": capture.terminal,
        "winner_seat": capture.winner,
        "turns": capture.turns,
        "accepted_policy_moves": capture.accepted_moves,
        "rejected_policy_moves": capture.rejected_moves,
        "votary_object": votary.0,
        "brainspoil_object": brainspoil.0,
        "brainspoil_target_object": capture.brainspoil_target.map(|object| object.0),
        "votary_characteristics": {"power": 1, "toughness": 1},
        "votary_mana_paid": capture.votary_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
        "brainspoil_mana_paid": capture.brainspoil_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
        "regeneration": {
            "mana_cost": {"generic": capture.ability_payment.as_ref().map(|cost| cost.generic), "colored": capture.ability_payment.as_ref().map(|cost| cost.colored.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>())},
            "source_incarnation": capture.ability_incarnation,
            "resolved_incarnation": capture.ability_resolved_incarnation,
            "target_object": capture.ability_target.map(|object| object.0),
            "shield_created": capture.shield_created,
            "shield_used": capture.shield_used,
            "permanent_tapped_after_shield": capture.tapped_after_shield,
            "still_on_battlefield_after_shield": capture.on_battlefield_after_shield,
            "destroyed": capture.destroyed
        },
        "events": events,
        "passed": true
    }))
}

fn select_events(
    projected: &[ProjectedEvent],
    capture: &CapturedMatch,
) -> Result<Vec<ProjectedEvent>, String> {
    let votary = capture.votary.ok_or("Votary identity missing")?;
    let brainspoil = capture.brainspoil.ok_or("Brainspoil identity missing")?;
    let mut selected = Vec::new();
    let mut take = |kind: &str,
                    object: ObjectId,
                    seat: Option<u16>,
                    ability: Option<&str>,
                    zone: Option<&str>,
                    second_object: Option<ObjectId>|
     -> Result<(), String> {
        let found = projected
            .iter()
            .find(|event| {
                event.kind == kind
                    && (kind == "GameEnded"
                        || event
                            .facts
                            .objects
                            .first()
                            .is_some_and(|candidate| candidate.0 == object.0))
                    && seat.is_none_or(|seat| event.facts.seats.iter().any(|value| value.0 == seat))
                    && ability.is_none_or(|ability| event.facts.ability.as_deref() == Some(ability))
                    && zone.is_none_or(|zone| event.facts.zone.as_deref() == Some(zone))
                    && second_object.is_none_or(|second| {
                        event
                            .facts
                            .objects
                            .get(1)
                            .is_some_and(|candidate| candidate.0 == second.0)
                    })
                    && selected
                        .last()
                        .is_none_or(|previous: &ProjectedEvent| previous.sequence < event.sequence)
            })
            .cloned()
            .ok_or_else(|| format!("ordered typed Votary event {kind} is missing"))?;
        selected.push(found);
        Ok(())
    };
    take("SpellManaPaid", votary, Some(0), None, None, None)?;
    take("SpellCast", votary, Some(0), None, None, None)?;
    take("SpellResolved", votary, None, None, None, None)?;
    take("CardMoved", votary, None, None, Some("Battlefield"), None)?;
    take("SpellManaPaid", brainspoil, Some(1), None, None, None)?;
    take("SpellCast", brainspoil, Some(1), None, None, None)?;
    take(
        "AbilityManaPaid",
        votary,
        Some(0),
        Some(REGEN_ABILITY),
        None,
        None,
    )?;
    take(
        "AbilityActivated",
        votary,
        Some(0),
        Some(REGEN_ABILITY),
        None,
        None,
    )?;
    take(
        "RegenerationShieldCreated",
        votary,
        None,
        None,
        None,
        Some(votary),
    )?;
    take(
        "AbilityResolved",
        votary,
        None,
        Some(REGEN_ABILITY),
        None,
        None,
    )?;
    take(
        "RegenerationShieldUsed",
        votary,
        None,
        None,
        None,
        Some(votary),
    )?;
    take("SpellResolved", brainspoil, None, None, None, None)?;
    take("GameEnded", votary, Some(0), None, None, None)?;
    Ok(selected)
}

fn same_match(left: &CapturedMatch, right: &CapturedMatch) -> bool {
    left.events == right.events
        && left.terminal == right.terminal
        && left.winner == right.winner
        && left.turns == right.turns
        && left.accepted_moves == right.accepted_moves
        && left.rejected_moves == right.rejected_moves
        && left.votary == right.votary
        && left.brainspoil == right.brainspoil
        && left.brainspoil_target == right.brainspoil_target
        && left.votary_payment == right.votary_payment
        && left.brainspoil_payment == right.brainspoil_payment
        && left.ability_payment == right.ability_payment
        && left.ability_incarnation == right.ability_incarnation
        && left.ability_resolved_incarnation == right.ability_resolved_incarnation
        && left.ability_target == right.ability_target
        && left.shield_created == right.shield_created
        && left.shield_used == right.shield_used
        && left.tapped_after_shield == right.tapped_after_shield
        && left.on_battlefield_after_shield == right.on_battlefield_after_shield
        && left.destroyed == right.destroyed
        && left.live_stats == right.live_stats
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
