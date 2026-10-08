//! Full-game diagnostic authority for RAV Sewerdreg.

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, CombatBlock, DeckEntry, DeckList, Effect, Game, GameEvent,
    GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step,
    Target, Zone,
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
const CARD_ID: &str = "RAV-SEWERDREG";
const CHAR_ID: &str = "RAV-CHAR";
const ELVES_ID: &str = "RAV-ELVES-OF-DEEP-SHADOW";
const REGEN_ABILITY: &str = "self-regeneration";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/sewerdreg.rs";
const MAX_TRACE_SEED: u64 = 128;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/sewerdreg.rs"]
mod candidate_sewerdreg;

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
            eprintln!("Sewerdreg PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_bindings()?;
    let catalog = unique_catalog()?;
    let seed = find_trace_seed()?;
    let capture = run_match(seed, false, false)?;
    let trace = validate_trace(&capture, seed)?;
    let replay = run_match(seed, false, false)?;
    if !same_match(&capture, &replay) {
        return Err("Sewerdreg full-game replay was not deterministic".to_owned());
    }
    let wrong_color_detected = run_match(seed, true, false)
        .and_then(|mutant| validate_trace(&mutant, seed).map(|_| mutant))
        .is_err();
    let no_fear_detected = run_match(seed, false, true).is_err();
    if !wrong_color_detected || !no_fear_detected {
        return Err("a compiled Sewerdreg semantic mutant escaped its game trace".to_owned());
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
        "card": {"collector_number": 104, "definition_id": CARD_ID, "name": "Sewerdreg"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "105.2 and 202.1-202.2: colored mana costs and payment",
                "117: priority and timing",
                "302.1 and 601.2: creature casting and resolution",
                "508-509: Fear attacker and blocker legality",
                "608.2: resolving Char and the no-target activated ability",
                "701.15: regeneration",
                "614.8: regeneration replaces destruction"
            ],
            "source_contract": "RAV Sewerdreg is a black 3/3 creature costing {3}{B}{B} with Fear and {B}: regenerate Sewerdreg",
            "authority_cutoff": "2026-09-23"
        },
        "passed": true,
        "pilot_card_passed": true,
        "deterministic_replay": true,
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
            "wrong_regeneration_color_mutation_detected_in_game": wrong_color_detected,
            "fear_removed_mutation_detected_by_combat_probe": no_fear_detected,
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
        .map_err(|error| format!("could not read Sewerdreg source: {error}"))?;
    let file =
        syn::parse_file(&source).map_err(|_| "Sewerdreg source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
    {
        return Err("Sewerdreg source contains an unsupported module attribute".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["cardbench_magic_engine", "ActivatedAbility"],
        vec!["cardbench_magic_engine", "ActivatedAbilityBinding"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["std", "collections", "BTreeSet"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(item.vis, Visibility::Inherited) =>
            {
                flatten_use_paths(&mut Vec::new(), &item.tree, &mut imports)?;
            }
            Item::Fn(function) => {
                if !functions.insert(function.sig.ident.to_string()) {
                    return Err("Sewerdreg source contains duplicate functions".to_owned());
                }
                validate_safe_function(function)?;
            }
            _ => {
                return Err("Sewerdreg source contains an unsupported top-level item".to_owned());
            }
        }
    }
    if imports != expected_imports
        || functions != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()])
    {
        return Err(
            "Sewerdreg imports or source function set differ from the task contract".to_owned(),
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
            return Err("Sewerdreg source imports may not rename or glob".to_owned());
        }
    }
    Ok(())
}

fn validate_safe_function(function: &syn::ItemFn) -> Result<(), String> {
    let attrs_ok = function.attrs.iter().all(|attribute| {
        attribute.path().is_ident("doc")
            || attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_))
    });
    let visible = matches!(&function.vis, Visibility::Restricted(visibility) if visibility.path.is_ident("crate"));
    let name = function.sig.ident.to_string();
    let output = match name.as_str() {
        "definition" => "CardDefinition",
        "activated_ability" => "ActivatedAbilityBinding",
        _ => return Err("Sewerdreg source has an unexpected function".to_owned()),
    };
    if !attrs_ok
        || !visible
        || function.sig.constness.is_some()
        || function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
        || function.sig.abi.is_some()
        || !function.sig.generics.params.is_empty()
        || function.sig.generics.where_clause.is_some()
        || !function.sig.inputs.is_empty()
        || function.sig.variadic.is_some()
        || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if matches!(ty.as_ref(), Type::Path(path) if path.path.is_ident(output)))
        || function.block.stmts.len() != 1
    {
        return Err(format!(
            "Sewerdreg {name} must be a plain literal-returning function"
        ));
    }
    let Stmt::Expr(expression, None) = &function.block.stmts[0] else {
        return Err(format!("Sewerdreg {name} must return a single expression"));
    };
    if !safe_expr(expression) {
        return Err(format!(
            "Sewerdreg {name} contains executable code outside the literal contract"
        ));
    }
    Ok(())
}

fn safe_expr(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(value) => value.attrs.is_empty(),
        Expr::Path(value) => {
            value.attrs.is_empty()
                && value.qself.is_none()
                && [
                    "Color::Black",
                    "CardType::Creature",
                    "Keyword::Fear",
                    "Effect::RegenerateSource",
                ]
                .iter()
                .any(|path| value.path.to_token_stream_string() == *path)
        }
        Expr::Array(value) => value.attrs.is_empty() && value.elems.iter().all(safe_expr),
        Expr::Reference(value) => {
            value.attrs.is_empty() && value.mutability.is_none() && safe_expr(&value.expr)
        }
        Expr::Call(value) => {
            if !value.attrs.is_empty() || !value.args.iter().all(safe_expr) {
                return false;
            }
            let Expr::Path(path) = &*value.func else {
                return false;
            };
            [
                "BTreeSet::from",
                "BTreeSet::new",
                "ManaCost::with_colors",
                "ManaCost::new",
                "Some",
            ]
            .iter()
            .any(|expected| path.path.to_token_stream_string() == *expected)
        }
        Expr::Struct(value) => {
            if !value.attrs.is_empty()
                || value.rest.is_some()
                || ![
                    "CardDefinition",
                    "ActivatedAbilityBinding",
                    "ActivatedAbility",
                ]
                .iter()
                .any(|expected| value.path.to_token_stream_string() == *expected)
            {
                return false;
            }
            value.fields.iter().all(|field| {
                field.attrs.is_empty()
                    && matches!(field.member, syn::Member::Named(_))
                    && field.colon_token.is_some()
                    && safe_expr(&field.expr)
            })
        }
        Expr::Macro(value) => {
            value.attrs.is_empty()
                && value.mac.path.is_ident("vec")
                && (value.mac.tokens.is_empty()
                    || value.mac.tokens.to_string() == "Keyword :: Fear"
                    || value.mac.tokens.to_string() == "Effect :: RegenerateSource")
        }
        _ => false,
    }
}

trait TokenPathString {
    fn to_token_stream_string(&self) -> String;
}
impl TokenPathString for syn::Path {
    fn to_token_stream_string(&self) -> String {
        self.segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect::<Vec<_>>()
            .join("::")
    }
}

fn validate_candidate_bindings() -> Result<(), String> {
    let submitted = candidate_sewerdreg::definition();
    let runtime = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or_else(|| "Sewerdreg is missing from the executable RAV registry".to_owned())?;
    if submitted != runtime
        || submitted.name != "Sewerdreg"
        || submitted.set_code != "RAV"
        || submitted.mana_cost != ManaCost::with_colors(3, [Color::Black, Color::Black])
        || submitted.colors != BTreeSet::from([Color::Black])
        || !submitted.mana_colors.is_empty()
        || submitted.card_types != BTreeSet::from([CardType::Creature])
        || submitted.is_basic_land
        || submitted.power != Some(3)
        || submitted.toughness != Some(3)
        || submitted.keywords != [Keyword::Fear]
        || !submitted.effects.is_empty()
    {
        return Err("Sewerdreg identity, cost, or printed characteristics differ".to_owned());
    }
    let submitted_ability = candidate_sewerdreg::activated_ability();
    let runtime_ability = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == REGEN_ABILITY)
        .ok_or_else(|| {
            "Sewerdreg regeneration binding is missing from the RAV registry".to_owned()
        })?;
    if submitted_ability != runtime_ability
        || submitted_ability.ability.mana_cost != ManaCost::with_colors(0, [Color::Black])
        || !submitted_ability.ability.targets.is_empty()
        || submitted_ability.ability.effects != [Effect::RegenerateSource]
        || submitted_ability.ability.tap_cost
        || submitted_ability.ability.sorcery_speed
        || submitted_ability.ability.sacrifice_source
    {
        return Err("Sewerdreg self-regeneration binding differs".to_owned());
    }
    Ok(())
}

fn find_trace_seed() -> Result<u64, String> {
    if let Ok(value) = std::env::var("CARDBENCH_SEWERDREG_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_SEWERDREG_SEED must be an unsigned integer".to_owned())?;
        validate_trace(&run_match(seed, false, false)?, seed)?;
        return Ok(seed);
    }
    for seed in 0..=MAX_TRACE_SEED {
        if run_match(seed, false, false)
            .and_then(|capture| validate_trace(&capture, seed).map(|_| capture))
            .is_ok()
        {
            return Ok(seed);
        }
    }
    Err(format!(
        "no complete Sewerdreg trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

#[derive(Clone)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    sewerdreg: Option<ObjectId>,
    char: Option<ObjectId>,
    char_target: Option<ObjectId>,
    blocker: Option<ObjectId>,
    sewerdreg_payment: Vec<Color>,
    char_payment: Vec<Color>,
    ability_payment: Option<ManaCost>,
    ability_incarnation: Option<u64>,
    ability_resolved_incarnation: Option<u64>,
    shield_created: bool,
    shield_used: bool,
    tapped_after_shield: bool,
    alive_after_shield: bool,
    destroyed: bool,
    live_stats: Option<(i32, i32)>,
    live_fear: bool,
    fear_at_attack: bool,
    fear_at_block: bool,
    ground_block_rejected: bool,
    probe_error: Option<String>,
}

fn run_match(
    seed: u64,
    wrong_regeneration_color: bool,
    remove_fear: bool,
) -> Result<CapturedMatch, String> {
    let mut game = new_rav_game(wrong_regeneration_color, remove_fear)?;
    let deck0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-SWAMP".to_owned(),
                count: 8,
            },
            DeckEntry {
                card: "RAV-SWAMP".to_owned(),
                count: 48,
            },
        ],
        sideboard: vec![],
    };
    let deck1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CHAR_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: ELVES_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 48,
            },
        ],
        sideboard: vec![],
    };
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 100,
        max_policy_moves: 20_000,
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
        Box::new(SewerdregPolicy::new(PlayerId(0), wrong_regeneration_color)),
        Box::new(DefensePolicy::new(PlayerId(1))),
    ];
    let mut capture = CapturedMatch {
        events: vec![],
        terminal: String::new(),
        winner: None,
        turns: 0,
        accepted_moves: 0,
        sewerdreg: None,
        char: None,
        char_target: None,
        blocker: None,
        sewerdreg_payment: vec![],
        char_payment: vec![],
        ability_payment: None,
        ability_incarnation: None,
        ability_resolved_incarnation: None,
        shield_created: false,
        shield_used: false,
        tapped_after_shield: false,
        alive_after_shield: false,
        destroyed: false,
        live_stats: None,
        live_fear: false,
        fear_at_attack: false,
        fear_at_block: false,
        ground_block_rejected: false,
        probe_error: None,
    };
    let mut fear_probe_done = false;
    while !game.is_game_over() {
        if game.turn > config.max_turns || capture.accepted_moves >= config.max_policy_moves {
            let relevant: Vec<_> = game
                .event_log
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        GameEvent::SpellCast { .. }
                            | GameEvent::SpellManaPaid { .. }
                            | GameEvent::SpellResolved { .. }
                            | GameEvent::AttackersDeclared { .. }
                            | GameEvent::BlockersDeclared { .. }
                            | GameEvent::GameEnded { .. }
                            | GameEvent::AbilityActivated { .. }
                    )
                })
                .cloned()
                .collect();
            return Err(format!(
                "Sewerdreg trace hit its bound at turn {}: {:?}; candidate={:?}, Char={:?}, shield={}, events={relevant:?}",
                game.turn, game.step, capture.sewerdreg, capture.char, capture.shield_created
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        if player == PlayerId(0)
            && view.step == Step::DeclareAttackers
            && view.active_player == player
        {
            if let Some(sewerdreg) = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            {
                capture.fear_at_attack |= sewerdreg.keywords.contains(&Keyword::Fear);
            }
        }
        if player == PlayerId(1)
            && view.step == Step::DeclareBlockers
            && view.active_player != player
            && !view.blockers_declared
        {
            if let (Some(attacker), Some(ground)) = (
                view.combat_attackers
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID)),
                view.own_battlefield
                    .iter()
                    .find(|card| card.definition == Some(ELVES_ID) && card.can_block),
            ) {
                capture.fear_at_block |= attacker.keywords.contains(&Keyword::Fear);
                capture.blocker = Some(ground.id);
                if !fear_probe_done {
                    let before_events = game.event_log.len();
                    let before_view = view.clone();
                    let attempt = PolicyAction::DeclareBlockers {
                        assignments: vec![CombatBlock {
                            attacker: attacker.id,
                            blocker: ground.id,
                        }],
                    };
                    match game.submit_policy_move(
                        player,
                        "rav.sewerdreg-fear-legality-probe.v1",
                        attempt,
                    ) {
                        Ok(()) => {
                            if !remove_fear {
                                return Err(
                                    "a nonblack nonartifact Elf was allowed to block Sewerdreg with Fear".to_owned()
                                );
                            }
                            return Err(
                                "Fear-removed compiled mutant allowed a ground block".to_owned()
                            );
                        }
                        Err(error) => {
                            let message = error.to_string();
                            if !message.contains(
                                "fear attacker can be blocked only by black or artifact creatures",
                            ) {
                                return Err(format!(
                                    "unexpected Fear legality-probe rejection: {message}"
                                ));
                            }
                            let after_view =
                                game.view_for_player(player).map_err(|e| e.to_string())?;
                            if game.event_log.len() != before_events || after_view != before_view {
                                return Err("rejected Fear legality probe changed the live match"
                                    .to_owned());
                            }
                            capture.ground_block_rejected = true;
                            capture.probe_error = Some(message);
                            fear_probe_done = true;
                            continue;
                        }
                    }
                }
            }
        }
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
                    "Sewerdreg policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        capture.accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        observe(&game, &game.event_log[before..], &mut capture)?;
        if wrong_regeneration_color && capture.destroyed {
            return Err(
                "compiled wrong-color regeneration mutant let Char destroy Sewerdreg".to_owned(),
            );
        }
    }
    capture.events = game.event_log.clone();
    capture.winner = game.winner().map(|player| player.0);
    capture.terminal = capture.winner.map_or_else(
        || "Draw".to_owned(),
        |seat| format!("Winner(PlayerId({seat}))"),
    );
    capture.turns = game.turn;
    if let Some(sewerdreg) = capture.sewerdreg
        && game.zone_of(sewerdreg) == Some(Zone::Battlefield)
    {
        let characteristics = game
            .characteristics(sewerdreg)
            .map_err(|error| error.to_string())?;
        capture.live_stats = Some((
            characteristics.power.ok_or("Sewerdreg has no power")?,
            characteristics
                .toughness
                .ok_or("Sewerdreg has no toughness")?,
        ));
        capture.live_fear = characteristics.keywords.contains(&Keyword::Fear);
    }
    if !fear_probe_done {
        let relevant: Vec<_> = game
            .event_log
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    GameEvent::SpellCast { .. }
                        | GameEvent::SpellManaPaid { .. }
                        | GameEvent::SpellResolved { .. }
                        | GameEvent::AttackersDeclared { .. }
                        | GameEvent::BlockersDeclared { .. }
                        | GameEvent::GameEnded { .. }
                        | GameEvent::AbilityActivated { .. }
                )
            })
            .cloned()
            .collect();
        return Err(format!(
            "Sewerdreg full game never reached a ground-block legality probe; sewerdreg={:?}, Char={:?}, regenerated={}, events={relevant:?}",
            capture.sewerdreg, capture.char, capture.shield_created
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
                capture.sewerdreg = Some(*card);
                capture.sewerdreg_payment = colors.clone();
            }
            GameEvent::SpellManaPaid {
                player: PlayerId(1),
                card,
                colors,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CHAR_ID) =>
            {
                capture.char = Some(*card);
                capture.char_payment = colors.clone();
            }
            GameEvent::SpellCast {
                player: PlayerId(1),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CHAR_ID) =>
            {
                capture.char = Some(*card);
                capture.char_target =
                    game.stack
                        .last()
                        .and_then(|spell| match spell.targets.first() {
                            Some(Target::Permanent(target)) => Some(*target),
                            _ => None,
                        });
            }
            GameEvent::AbilityManaPaid {
                player: PlayerId(0),
                source,
                ability,
                mana_cost,
            } if Some(*source) == capture.sewerdreg && *ability == REGEN_ABILITY => {
                capture.ability_payment = Some(mana_cost.clone());
            }
            GameEvent::AbilityActivated {
                player: PlayerId(0),
                source,
                source_incarnation,
                ability,
                ..
            } if Some(*source) == capture.sewerdreg && *ability == REGEN_ABILITY => {
                capture.ability_incarnation = Some(*source_incarnation);
            }
            GameEvent::AbilityResolved {
                source,
                source_incarnation,
                ability,
            } if Some(*source) == capture.sewerdreg && *ability == REGEN_ABILITY => {
                capture.ability_resolved_incarnation = Some(*source_incarnation);
            }
            GameEvent::RegenerationShieldCreated { source, target }
                if Some(*source) == capture.sewerdreg && source == target =>
            {
                capture.shield_created = true;
            }
            GameEvent::RegenerationShieldUsed { target, .. }
                if Some(*target) == capture.sewerdreg =>
            {
                capture.shield_used = true;
                capture.tapped_after_shield =
                    game.object(*target).is_ok_and(|object| object.tapped);
                capture.alive_after_shield = game.zone_of(*target) == Some(Zone::Battlefield);
            }
            GameEvent::CardDestroyed { card, .. } if Some(*card) == capture.sewerdreg => {
                capture.destroyed = true
            }
            GameEvent::SpellResolved { card } if Some(*card) == capture.sewerdreg => {
                let stats = game
                    .characteristics(*card)
                    .map_err(|error| error.to_string())?;
                capture.live_stats = Some((
                    stats.power.ok_or("Sewerdreg has no live power")?,
                    stats.toughness.ok_or("Sewerdreg has no live toughness")?,
                ));
                capture.live_fear = stats.keywords.contains(&Keyword::Fear);
            }
            _ => {}
        }
    }
    Ok(())
}

fn new_rav_game(wrong_regeneration_color: bool, remove_fear: bool) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let sewerdreg = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Sewerdreg definition missing from RAV registry")?;
    *sewerdreg = candidate_sewerdreg::definition();
    if remove_fear {
        sewerdreg.keywords.clear();
    }
    let mut abilities = rav_activated_ability_bindings();
    abilities.retain(|binding| {
        !(binding.card_definition == CARD_ID && binding.ability.id == REGEN_ABILITY)
    });
    let mut regeneration = candidate_sewerdreg::activated_ability();
    if wrong_regeneration_color {
        regeneration.ability.mana_cost = ManaCost::with_colors(0, [Color::Green]);
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

struct SewerdregPolicy {
    player: PlayerId,
    wrong_color: bool,
    sewerdreg_cast: bool,
    regenerated: bool,
}
impl SewerdregPolicy {
    fn new(player: PlayerId, wrong_color: bool) -> Self {
        Self {
            player,
            wrong_color,
            sewerdreg_cast: false,
            regenerated: false,
        }
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
    fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let mut available: Vec<(Color, u8)> = Color::MANA_ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        for color in &cost.colored {
            let slot = available
                .iter_mut()
                .find(|(candidate, _)| candidate == color)?;
            if slot.1 == 0 {
                return None;
            }
            slot.1 -= 1;
        }
        let mut generic = Vec::new();
        for _ in 0..cost.generic {
            let index = available
                .iter()
                .position(|(color, count)| *count > 0 && *color != Color::Black)
                .or_else(|| available.iter().position(|(_, count)| *count > 0))?;
            let slot = &mut available[index];
            slot.1 -= 1;
            generic.push(slot.0);
        }
        Some(ManaPaymentSelection {
            generic,
            hybrid: vec![],
        })
    }
    fn mana_action(view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
        if let Some(color) = cost
            .colored
            .iter()
            .find(|color| view.mana_pool.amount(**color) == 0)
        {
            return Self::mana_source(view, *color);
        }
        let total: u8 = Color::MANA_ALL
            .into_iter()
            .map(|color| view.mana_pool.amount(color))
            .sum();
        if total < cost.generic + cost.colored.len() as u8 {
            return Color::MANA_ALL
                .into_iter()
                .find_map(|color| Self::mana_source(view, color));
        }
        None
    }
    fn cast(
        &mut self,
        view: &GameView,
        id: &str,
        cost: &ManaCost,
        targets: Vec<Target>,
    ) -> Option<PolicyAction> {
        if let Some(card) = view.hand.iter().find(|card| card.definition == Some(id)) {
            if let Some(mana_selection) = Self::payment(view, cost) {
                if id == CARD_ID {
                    self.sewerdreg_cast = true;
                }
                return Some(PolicyAction::CastWithPayment {
                    request: cardbench_magic_engine::CastRequest {
                        card: card.id,
                        targets,
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection,
                });
            }
            return Self::mana_action(view, cost);
        }
        None
    }
    fn regeneration(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.player != PlayerId(0)
            || self.regenerated
            || !view
                .stack_spells
                .iter()
                .any(|spell| spell.definition == Some(CHAR_ID))
        {
            return None;
        }
        let sewerdreg = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some(CARD_ID))?;
        let cost = ManaCost::with_colors(
            0,
            [if self.wrong_color {
                Color::Green
            } else {
                Color::Black
            }],
        );
        if view.mana_pool.amount(cost.colored[0]) == 0 {
            return Self::mana_source(view, cost.colored[0]);
        }
        self.regenerated = true;
        Some(PolicyAction::ActivateAbility {
            activation: AbilityActivation {
                source: sewerdreg.id,
                ability_id: REGEN_ABILITY,
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![],
            },
        })
    }
}
impl CodePolicy for SewerdregPolicy {
    fn id(&self) -> &'static str {
        "rav.sewerdreg-controller.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = if self.player == PlayerId(0) && self.regenerated {
                if view
                    .opponent_battlefield
                    .iter()
                    .any(|card| card.definition == Some(ELVES_ID) && card.can_block)
                {
                    view.own_battlefield
                        .iter()
                        .filter(|card| card.definition == Some(CARD_ID) && card.can_attack)
                        .map(|card| card.id)
                        .collect()
                } else {
                    vec![]
                }
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
                let land = if !view
                    .own_battlefield
                    .iter()
                    .any(|card| card.definition == Some("RAV-SWAMP"))
                {
                    "RAV-SWAMP"
                } else {
                    "RAV-SWAMP"
                };
                if let Some(card) = view.hand.iter().find(|card| card.definition == Some(land)) {
                    return PolicyAction::PlayLand { card: card.id };
                }
            }
            let black_sources = view
                .own_battlefield
                .iter()
                .filter(|card| card.card_types.contains(&CardType::Land))
                .count();
            if !self.sewerdreg_cast && black_sources >= 6 {
                if let Some(action) = self.cast(
                    view,
                    CARD_ID,
                    &ManaCost::with_colors(3, [Color::Black, Color::Black]),
                    vec![],
                ) {
                    return action;
                }
            }
        }
        if let Some(action) = self.regeneration(view) {
            return action;
        }
        PolicyAction::PassPriority
    }
}

struct DefensePolicy {
    player: PlayerId,
    elf_cast: bool,
    char_cast: bool,
}
impl DefensePolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            elf_cast: false,
            char_cast: false,
        }
    }
}
impl CodePolicy for DefensePolicy {
    fn id(&self) -> &'static str {
        "rav.sewerdreg-char-opponent.v1"
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
            if view.lands_played == 0 {
                let need_forest = !self.elf_cast
                    && view
                        .hand
                        .iter()
                        .any(|card| card.definition == Some(ELVES_ID))
                    && !view
                        .own_battlefield
                        .iter()
                        .any(|card| card.definition == Some("RAV-FOREST"));
                let land_id = if need_forest {
                    "RAV-FOREST"
                } else {
                    "RAV-MOUNTAIN"
                };
                if let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(land_id))
                {
                    return PolicyAction::PlayLand { card: card.id };
                }
            }
            if !self.elf_cast
                && let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(ELVES_ID))
            {
                let cost = ManaCost::with_colors(0, [Color::Green]);
                if let Some(selection) = SewerdregPolicy::payment(view, &cost) {
                    self.elf_cast = true;
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: selection,
                    };
                }
                if let Some(action) = SewerdregPolicy::mana_action(view, &cost) {
                    return action;
                }
            }
            if !self.char_cast
                && let (Some(sewerdreg), Some(spell)) = (
                    view.opponent_battlefield
                        .iter()
                        .find(|card| card.definition == Some(CARD_ID)),
                    view.hand
                        .iter()
                        .find(|card| card.definition == Some(CHAR_ID)),
                )
            {
                let cost = ManaCost::with_colors(2, [Color::Red]);
                if let Some(selection) = SewerdregPolicy::payment(view, &cost) {
                    self.char_cast = true;
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: spell.id,
                            targets: vec![Target::Permanent(sewerdreg.id)],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: selection,
                    };
                }
                if let Some(action) = SewerdregPolicy::mana_action(view, &cost) {
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
        || capture.sewerdreg.is_none()
        || capture.char.is_none()
    {
        return Err(
            "Sewerdreg full game did not finish with both spells resolved and a clean P0 win"
                .to_owned(),
        );
    }
    if capture.sewerdreg_payment.len() != 5
        || !capture.sewerdreg_payment.contains(&Color::Black)
        || capture
            .sewerdreg_payment
            .iter()
            .filter(|color| **color == Color::Black)
            .count()
            != 5
    {
        return Err(format!(
            "Sewerdreg {{3}}{{B}}{{B}} payment was not exactly five black mana: {:?}",
            capture.sewerdreg_payment
        ));
    }
    if capture.char_payment.len() != 3
        || capture
            .char_payment
            .iter()
            .all(|color| *color != Color::Red)
    {
        return Err(format!(
            "Char payment did not include one red and three total mana: {:?}",
            capture.char_payment
        ));
    }
    if capture.live_stats != Some((3, 3))
        || !capture.live_fear
        || !capture.fear_at_attack
        || !capture.fear_at_block
    {
        return Err(
            "live Sewerdreg characteristics did not retain 3/3 Fear through cast and combat"
                .to_owned(),
        );
    }
    if !capture.ground_block_rejected
        || capture.probe_error.as_deref().is_none_or(|message| {
            !message.contains("fear attacker can be blocked only by black or artifact creatures")
        })
    {
        return Err(
            "a legal ground creature was not rejected as a blocker by live Fear".to_owned(),
        );
    }
    if capture
        .ability_payment
        .as_ref()
        .is_none_or(|cost| cost != &ManaCost::with_colors(0, [Color::Black]))
        || capture.ability_incarnation.is_none()
        || capture.ability_incarnation != capture.ability_resolved_incarnation
        || !capture.shield_created
        || !capture.shield_used
        || !capture.tapped_after_shield
        || !capture.alive_after_shield
        || capture.destroyed
    {
        return Err(
            "Sewerdreg did not pay {B}, resolve self-regeneration, and survive Char damage"
                .to_owned(),
        );
    }
    let events = selected_events(capture)?;
    let sequences: Vec<_> = events.iter().map(|event| event.sequence).collect();
    if !sequences.windows(2).all(|window| window[0] < window[1]) {
        return Err("Sewerdreg typed event sequence is not strictly increasing".to_owned());
    }
    let kinds: Vec<_> = events.iter().map(|event| event.kind.as_str()).collect();
    let expected = [
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "SpellManaPaid",
        "SpellCast",
        "AbilityManaPaid",
        "AbilityActivated",
        "RegenerationShieldCreated",
        "AbilityResolved",
        "SpellResolved",
        "RegenerationShieldUsed",
        "AttackersDeclared",
        "BlockersDeclared",
        "DamageDealtToPlayer",
        "GameEnded",
    ];
    if kinds != expected {
        return Err(format!(
            "Sewerdreg typed gameplay event order differs: {kinds:?}"
        ));
    }
    Ok(json!({
        "trace_id": "rav-sewerdreg-fear-self-regeneration",
        "seed": seed,
        "decks": ["rav-sewerdreg-swamp-60", "rav-char-forest-mountain-60"],
        "policies": ["rav.sewerdreg-controller.v1", "rav.sewerdreg-char-opponent.v1"],
        "terminal": capture.terminal, "winner_seat": capture.winner, "turns": capture.turns,
        "accepted_policy_moves": capture.accepted_moves,
        "sewerdreg_object": capture.sewerdreg.map(|object| object.0), "char_object": capture.char.map(|object| object.0), "char_target_object": capture.char_target.map(|object| object.0),
        "sewerdreg_mana_paid": capture.sewerdreg_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
        "char_mana_paid": capture.char_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
        "sewerdreg_characteristics": {"power": 3, "toughness": 3, "fear_at_resolution": capture.live_fear, "fear_at_attack": capture.fear_at_attack, "fear_at_block": capture.fear_at_block},
        "fear_legality_probe": {"ground_block_rejected": capture.ground_block_rejected, "blocker_object": capture.blocker.map(|object| object.0), "error": capture.probe_error},
        "regeneration": {"mana_cost": {"generic": capture.ability_payment.as_ref().map(|cost| cost.generic), "colored": capture.ability_payment.as_ref().map(|cost| cost.colored.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>())}, "source_incarnation": capture.ability_incarnation, "resolved_incarnation": capture.ability_resolved_incarnation, "shield_created": capture.shield_created, "shield_used": capture.shield_used, "tapped_after_shield": capture.tapped_after_shield, "alive_after_shield": capture.alive_after_shield, "destroyed": capture.destroyed},
        "events": events, "passed": true
    }))
}

fn selected_events(capture: &CapturedMatch) -> Result<Vec<ProjectedEvent>, String> {
    let sewerdreg = capture.sewerdreg.ok_or("Sewerdreg identity missing")?;
    let char = capture.char.ok_or("Char identity missing")?;
    let mut indices = Vec::new();
    macro_rules! add {
        ($predicate:expr, $label:literal) => {
            indices.push(event_index(&capture.events, $predicate).ok_or($label)?);
        };
    }
    add!(
        |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == sewerdreg && colors.len() == 5 && colors.iter().filter(|color| **color == Color::Black).count() == 5),
        "Sewerdreg mana payment event missing"
    );
    add!(
        |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == sewerdreg),
        "Sewerdreg cast event missing"
    );
    add!(
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == sewerdreg),
        "Sewerdreg resolution event missing"
    );
    add!(
        |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == sewerdreg),
        "Sewerdreg entry event missing"
    );
    add!(
        |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(1), card, colors } if *card == char && colors.len() == 3 && colors.contains(&Color::Red)),
        "Char mana payment event missing"
    );
    add!(
        |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == char),
        "Char cast event missing"
    );
    add!(
        |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: REGEN_ABILITY, mana_cost } if *source == sewerdreg && *mana_cost == ManaCost::with_colors(0, [Color::Black])),
        "regeneration payment event missing"
    );
    add!(
        |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source, ability: REGEN_ABILITY, .. } if *source == sewerdreg),
        "regeneration activation event missing"
    );
    add!(
        |event| matches!(event, GameEvent::RegenerationShieldCreated { source, target } if *source == sewerdreg && *target == sewerdreg),
        "regeneration shield creation event missing"
    );
    add!(
        |event| matches!(event, GameEvent::AbilityResolved { source, ability: REGEN_ABILITY, .. } if *source == sewerdreg),
        "regeneration ability resolution event missing"
    );
    add!(
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == char),
        "Char resolution event missing"
    );
    add!(
        |event| matches!(event, GameEvent::RegenerationShieldUsed { target, .. } if *target == sewerdreg),
        "regeneration shield use event missing"
    );
    add!(
        |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&sewerdreg)),
        "Sewerdreg attack event missing"
    );
    add!(
        |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if assignments.is_empty()),
        "empty legal blockers event missing"
    );
    add!(
        |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 3 } if *source == sewerdreg),
        "Sewerdreg combat damage event missing"
    );
    add!(
        |event| matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        ),
        "terminal P0 win event missing"
    );
    if !indices.windows(2).all(|window| window[0] < window[1]) {
        return Err("cast, Char/response, combat and terminal events are out of order".to_owned());
    }
    let projected = project_events(&capture.events);
    Ok(indices
        .into_iter()
        .map(|index| {
            projected
                .iter()
                .find(|event| event.sequence == index as u64)
                .cloned()
                .ok_or_else(|| "projected typed event sequence missing".to_owned())
        })
        .collect::<Result<_, _>>()?)
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn same_match(left: &CapturedMatch, right: &CapturedMatch) -> bool {
    left.events == right.events
        && left.terminal == right.terminal
        && left.winner == right.winner
        && left.turns == right.turns
        && left.accepted_moves == right.accepted_moves
        && left.sewerdreg == right.sewerdreg
        && left.char == right.char
        && left.char_target == right.char_target
        && left.blocker == right.blocker
        && left.sewerdreg_payment == right.sewerdreg_payment
        && left.char_payment == right.char_payment
        && left.ability_payment == right.ability_payment
        && left.ability_incarnation == right.ability_incarnation
        && left.ability_resolved_incarnation == right.ability_resolved_incarnation
        && left.shield_created == right.shield_created
        && left.shield_used == right.shield_used
        && left.tapped_after_shield == right.tapped_after_shield
        && left.alive_after_shield == right.alive_after_shield
        && left.destroyed == right.destroyed
        && left.live_stats == right.live_stats
        && left.live_fear == right.live_fear
        && left.fear_at_attack == right.fear_at_attack
        && left.fear_at_block == right.fear_at_block
        && left.ground_block_rejected == right.ground_block_rejected
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
