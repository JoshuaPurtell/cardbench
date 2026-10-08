//! Full-game diagnostic authority for RAV Scion of the Wild.
//!
//! The pilot uses a legal shuffled 60-card RAV deck, casts Scion before any
//! other creature, and then casts Birds of Paradise. Its live P/T must update
//! from 1/1 to 2/2 as the controlled creature count changes.

use cardbench_magic_engine::{
    CardType, Color, ContinuousChange, DeckEntry, DeckList, Game, GameEvent, GameView, ManaCost,
    ObjectId, PlayerId, PolicyAction, Step, Zone,
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
use cardbench_magic_session::{project_events, EventFacts, ProjectedEvent};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Meta, PathArguments, Stmt, Type, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-SCION-OF-THE-WILD";
const SECOND_CREATURE_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/scion_of_the_wild.rs";
const MAX_TRACE_SEED: u64 = 128;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/scion_of_the_wild.rs"]
mod candidate_scion;

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
            eprintln!("Scion of the Wild PvP authority failed: {error}");
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
        return Err("Scion of the Wild shuffled full-game replay was not deterministic".to_owned());
    }
    let removed = run_match(seed, true)
        .and_then(|mutant| validate_trace(&mutant, seed).map(|_| mutant))
        .is_err();
    if !removed {
        return Err(
            "removing Scion's dynamic creature-count effect escaped the game trace".to_owned(),
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
        "card": {"collector_number": 182, "definition_id": CARD_ID, "name": "Scion of the Wild"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "105.2 and 202.1-202.2: colored mana cost and payment",
                "117: timing and priority",
                "208.1 and 613: dynamic power/toughness and continuous effects",
                "302.1-302.2: creature casting, resolution, and entry",
                "704: state-based actions",
            ],
            "source_contract": "RAV printed semantics for Scion of the Wild",
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
            "dynamic_creature_count_removed_mutation_detected_in_game": removed,
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
        return Err(format!(
            "RAV catalog has {} names / {} printings; expected {} / {}",
            catalog.len(),
            printings.len(),
            RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
            RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        ));
    }
    Ok(catalog)
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Scion source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Scion source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Scion source contains an unsupported module attribute".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "ContinuousChange"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "StaticContinuousEffectBinding"],
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
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let Some(paths) = flattened_use_paths(item) else {
                    return Err("Scion source contains an unsupported import".to_owned());
                };
                actual_imports.extend(paths);
            }
            Item::Fn(function) => {
                if !functions.insert(function.sig.ident.to_string()) {
                    return Err("Scion source contains duplicate functions".to_owned());
                }
                validate_safe_function(function)?;
            }
            _ => return Err("Scion source contains executable top-level code".to_owned()),
        }
    }
    if actual_imports != expected_imports
        || functions
            != BTreeSet::from([
                "definition".to_owned(),
                "static_continuous_effect".to_owned(),
            ])
    {
        return Err(
            "Scion imports or source function set differ from the task contract".to_owned(),
        );
    }
    Ok(())
}

fn validate_safe_function(function: &syn::ItemFn) -> Result<(), String> {
    let attrs_ok = function.attrs.iter().all(|attribute| {
        is_doc_attribute(attribute)
            || attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_))
    });
    let visibility_ok = matches!(
        &function.vis,
        Visibility::Restricted(visibility) if visibility.path.is_ident("crate")
    );
    let name = function.sig.ident.to_string();
    let return_type = if name == "definition" {
        &["CardDefinition"][..]
    } else if name == "static_continuous_effect" {
        &["StaticContinuousEffectBinding"][..]
    } else {
        return Err("Scion source has an unexpected function".to_owned());
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
            "Scion {name} must be a plain literal-returning function"
        ));
    }
    let Stmt::Expr(expression, None) = &function.block.stmts[0] else {
        return Err(format!("Scion {name} must return a single expression"));
    };
    if !safe_expr(expression) {
        return Err(format!(
            "Scion {name} contains executable code outside the literal contract"
        ));
    }
    Ok(())
}

fn safe_expr(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(literal) => literal.attrs.is_empty(),
        Expr::Path(path) => {
            path.attrs.is_empty()
                && path.qself.is_none()
                && [
                    &["None"][..],
                    &["Color", "Green"],
                    &["CardType", "Creature"],
                    &["ContinuousChange", "ControlledCreatureCountPowerToughness"],
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
                    &["StaticContinuousEffectBinding"][..],
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
            mac.attrs.is_empty() && mac.mac.path.is_ident("vec") && mac.mac.tokens.is_empty()
        }
        _ => false,
    }
}

fn validate_candidate_bindings() -> Result<(), String> {
    let submitted = candidate_scion::definition();
    let runtime = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or_else(|| "Scion is missing from the executable card registry".to_owned())?;
    if submitted != runtime
        || submitted.name != "Scion of the Wild"
        || submitted.set_code != "RAV"
        || submitted.mana_cost != ManaCost::with_colors(1, [Color::Green, Color::Green])
        || submitted.colors != BTreeSet::from([Color::Green])
        || !submitted.mana_colors.is_empty()
        || submitted.card_types != BTreeSet::from([CardType::Creature])
        || submitted.is_basic_land
        || submitted.power != Some(0)
        || submitted.toughness != Some(0)
        || !submitted.keywords.is_empty()
        || !submitted.effects.is_empty()
    {
        return Err("Scion identity, cost, or printed characteristics differ".to_owned());
    }
    let binding = candidate_scion::static_continuous_effect();
    if binding.card_definition != CARD_ID
        || binding.change != ContinuousChange::ControlledCreatureCountPowerToughness
        || !rav_static_continuous_effect_bindings().contains(&binding)
    {
        return Err("Scion controlled-creature-count static binding differs".to_owned());
    }
    Ok(())
}

fn new_rav_game(remove_dynamic_effect: bool) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let candidate = candidate_scion::definition();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or_else(|| "Scion is missing from the executable card registry".to_owned())?;
    *registered = candidate;
    let mut continuous = rav_static_continuous_effect_bindings();
    if remove_dynamic_effect {
        continuous.retain(|binding| binding.card_definition != CARD_ID);
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
        continuous,
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

#[derive(Clone)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    scion: Option<ObjectId>,
    second: Option<ObjectId>,
    before: Option<(i32, i32, usize)>,
    after: Option<(i32, i32, usize)>,
    scion_payment: Vec<Color>,
    second_payment: Vec<Color>,
    first_scion_damage: Option<i32>,
}

#[derive(Clone, Copy)]
struct Observed {
    scion: Option<ObjectId>,
    second: Option<ObjectId>,
    before: Option<(i32, i32, usize)>,
    after: Option<(i32, i32, usize)>,
    first_scion_damage: Option<i32>,
}

fn find_trace_seed() -> Result<u64, String> {
    if let Ok(value) = std::env::var("CARDBENCH_SCION_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_SCION_SEED must be an unsigned integer".to_owned())?;
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
        "no full Scion trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn run_match(seed: u64, remove_dynamic_effect: bool) -> Result<CapturedMatch, String> {
    let mut game = new_rav_game(remove_dynamic_effect)?;
    let forests = 51_u8;
    let deck0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: SECOND_CREATURE_ID.to_owned(),
                count: 1,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: forests,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 4,
            },
        ],
        sideboard: vec![],
    };
    let deck1 = DeckList {
        mainboard: vec![DeckEntry {
            card: "RAV-FOREST".to_owned(),
            count: 60,
        }],
        sideboard: vec![],
    };
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 80,
        max_policy_moves: 12_000,
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
        Box::new(ScionPolicy::new(PlayerId(0))),
        Box::new(PassPolicy),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut observed = Observed {
        scion: None,
        second: None,
        before: None,
        after: None,
        first_scion_damage: None,
    };
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(format!(
                "Scion duel exceeded its bound at turn {}",
                game.turn
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
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        observe(&game, &mut observed)?;
    }
    let winner = game.winner().map(|player| player.0);
    let terminal = winner.map_or_else(
        || "Draw".to_owned(),
        |seat| format!("Winner(PlayerId({seat}))"),
    );
    let events = game.event_log.clone();
    let scion = observed.scion;
    let second = observed.second;
    let scion_payment = payment_colors(&events, scion);
    let second_payment = payment_colors(&events, second);
    let first_scion_damage = events.iter().find_map(|event| match event {
        GameEvent::DamageDealtToPlayer {
            source,
            player: PlayerId(1),
            amount,
        } if Some(*source) == scion => Some(*amount),
        _ => None,
    });
    Ok(CapturedMatch {
        events,
        terminal,
        winner,
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempted.saturating_sub(accepted),
        scion,
        second,
        before: observed
            .before
            .map(|(power, toughness, count)| (power, toughness, count)),
        after: observed
            .after
            .map(|(power, toughness, count)| (power, toughness, count)),
        scion_payment,
        second_payment,
        first_scion_damage,
    })
}

fn observe(game: &Game, observed: &mut Observed) -> Result<(), String> {
    for event in &game.event_log {
        match event {
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } => {
                let id = game
                    .card_definition(*card)
                    .map_err(|error| error.to_string())?
                    .id;
                if id == CARD_ID {
                    observed.scion = Some(*card);
                }
                if id == SECOND_CREATURE_ID && observed.second.is_none() {
                    observed.second = Some(*card);
                }
            }
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if Some(*source) == observed.scion && observed.first_scion_damage.is_none() => {
                observed.first_scion_damage = Some(*amount);
            }
            _ => {}
        }
    }
    let Some(scion) = observed.scion else {
        return Ok(());
    };
    if game.zone_of(scion) != Some(Zone::Battlefield) {
        return Ok(());
    }
    let view = game
        .view_for_player(PlayerId(0))
        .map_err(|error| error.to_string())?;
    let creature_count = view
        .own_battlefield
        .iter()
        .filter(|card| card.card_types.contains(&CardType::Creature))
        .count();
    let characteristics = game
        .characteristics(scion)
        .map_err(|error| error.to_string())?;
    let power = characteristics.power.ok_or("Scion has no live power")?;
    let toughness = characteristics
        .toughness
        .ok_or("Scion has no live toughness")?;
    if observed
        .second
        .is_some_and(|second| game.zone_of(second) == Some(Zone::Battlefield))
    {
        if observed.after.is_none() {
            observed.after = Some((power, toughness, creature_count));
        }
    } else if observed.before.is_none() {
        observed.before = Some((power, toughness, creature_count));
    }
    Ok(())
}

fn payment_colors(events: &[GameEvent], object: Option<ObjectId>) -> Vec<Color> {
    let Some(object) = object else { return vec![] };
    events
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if *card == object => Some(colors.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

struct PassPolicy;
impl CodePolicy for PassPolicy {
    fn id(&self) -> &'static str {
        "rav.scion-pass-opponent.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        match view.step {
            Step::DeclareAttackers
                if view.active_player == view.player && !view.attackers_declared =>
            {
                PolicyAction::DeclareAttackers { attackers: vec![] }
            }
            Step::DeclareBlockers
                if view.active_player != view.player && !view.blockers_declared =>
            {
                PolicyAction::DeclareBlockers {
                    assignments: vec![],
                }
            }
            _ => PolicyAction::PassPriority,
        }
    }
}

struct ScionPolicy {
    player: PlayerId,
    scion_cast: bool,
}
impl ScionPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            scion_cast: false,
        }
    }
    fn green_source(view: &GameView) -> Option<PolicyAction> {
        Self::mana_source(view, Color::Green)
    }
    fn mana_source(view: &GameView, color: Color) -> Option<PolicyAction> {
        view.own_battlefield
            .iter()
            .find(|card| {
                !card.tapped
                    && card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&color)
            })
            .map(|card| PolicyAction::ActivateManaAbility {
                land: card.id,
                color,
            })
    }
    fn cast(card: ObjectId, generic_green: bool) -> PolicyAction {
        PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: cardbench_magic_engine::ManaPaymentSelection {
                generic: if generic_green {
                    vec![Color::Green]
                } else {
                    vec![]
                },
                hybrid: vec![],
            },
        }
    }
}
impl CodePolicy for ScionPolicy {
    fn id(&self) -> &'static str {
        "rav.scion-live-creature-count.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let second_is_ready = view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(SECOND_CREATURE_ID));
            return PolicyAction::DeclareAttackers {
                attackers: if second_is_ready {
                    view.own_battlefield
                        .iter()
                        .filter(|card| {
                            card.can_attack && card.card_types.contains(&CardType::Creature)
                        })
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
        if view.decision_player != self.player
            || view.priority != self.player
            || view.active_player != self.player
            || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            let has_plains = view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some("RAV-PLAINS"));
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| !has_plains && card.definition == Some("RAV-PLAINS"))
                .or_else(|| {
                    view.hand
                        .iter()
                        .find(|card| card.definition == Some("RAV-FOREST"))
                })
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        let scion_live = view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some(CARD_ID));
        let second_live = view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some(SECOND_CREATURE_ID));
        if !self.scion_cast {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            {
                if view.mana_pool.amount(Color::Green) >= 3 {
                    self.scion_cast = true;
                    return Self::cast(card.id, true);
                }
                if let Some(action) = Self::green_source(view) {
                    return action;
                }
            }
            return PolicyAction::PassPriority;
        }
        if (scion_live || self.scion_cast) && !second_live {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(SECOND_CREATURE_ID))
            {
                if view.mana_pool.amount(Color::Green) >= 1
                    && view.mana_pool.amount(Color::White) >= 1
                {
                    return Self::cast(card.id, false);
                }
                let color = if view.mana_pool.amount(Color::White) == 0 {
                    Color::White
                } else {
                    Color::Green
                };
                if let Some(action) = Self::mana_source(view, color) {
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
        return Err("Scion full-game trace did not finish in a clean P0 win".to_owned());
    }
    let scion = capture.scion.ok_or("Scion was not cast")?;
    let second = capture
        .second
        .ok_or("second controlled creature was not cast")?;
    if capture.before != Some((1, 1, 1)) || capture.after != Some((2, 2, 2)) {
        return Err(format!(
            "Scion live power/toughness did not follow controlled creature count: {:?} -> {:?}",
            capture.before, capture.after
        ));
    }
    if capture.scion_payment != vec![Color::Green, Color::Green, Color::Green] {
        return Err(format!(
            "Scion {{1}}{{G}}{{G}} payment was not exactly three green mana: {:?}",
            capture.scion_payment
        ));
    }
    let mut second_colors = capture.second_payment.clone();
    second_colors.sort_by_key(|color| format!("{color:?}"));
    let mut expected_second_colors = vec![Color::Green, Color::White];
    expected_second_colors.sort_by_key(|color| format!("{color:?}"));
    if second_colors != expected_second_colors {
        return Err(format!(
            "Watchwolf colored payment differed: {:?}",
            capture.second_payment
        ));
    }
    if capture.first_scion_damage != Some(2) {
        return Err(format!("Scion did not deal combat damage as a live 2-power creature: {:?}; Scion {:?}, second {:?}, terminal {}", capture.first_scion_damage, capture.scion, capture.second, capture.terminal));
    }
    let projected = project_events(&capture.events);
    let mut relevant: Vec<(usize, &ProjectedEvent)> = Vec::new();
    let mut attack_selected = false;
    let mut damage_selected = false;
    for (index, event) in projected.iter().enumerate() {
        let include = if event.kind == "GameEnded" {
            true
        } else if event.kind == "AttackersDeclared" {
            let names_target = event.facts.objects.iter().any(|object| object.0 == scion.0)
                || event
                    .facts
                    .objects
                    .iter()
                    .any(|object| object.0 == second.0);
            if names_target && !attack_selected {
                attack_selected = true;
                true
            } else {
                false
            }
        } else if event.kind == "DamageDealtToPlayer" {
            let names_scion = event
                .facts
                .objects
                .first()
                .is_some_and(|object| object.0 == scion.0);
            if names_scion && !damage_selected {
                damage_selected = true;
                true
            } else {
                false
            }
        } else {
            matches!(
                event.kind.as_str(),
                "SpellManaPaid" | "SpellCast" | "SpellResolved" | "CardMoved"
            ) && event
                .facts
                .objects
                .first()
                .is_some_and(|object| object.0 == scion.0 || object.0 == second.0)
                && (event.kind != "CardMoved" || event.facts.zone.as_deref() == Some("Battlefield"))
        };
        if include {
            relevant.push((index, event));
        }
    }
    let kinds: Vec<&str> = relevant
        .iter()
        .map(|(_, event)| event.kind.as_str())
        .collect();
    let expected = [
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "AttackersDeclared",
        "DamageDealtToPlayer",
        "GameEnded",
    ];
    if kinds != expected {
        return Err(format!("typed Scion event order differs: {kinds:?}"));
    }
    let trace_events: Vec<Value> = relevant
        .iter()
        .map(|(_, event)| {
            json!({
                "sequence": event.sequence,
                "turn": event.turn,
                "kind": event.kind,
                "facts": event.facts,
                "canonical": event.canonical
            })
        })
        .collect();
    let facts: Vec<_> = relevant.iter().map(|(_, event)| &event.facts).collect();
    if !one_seat(facts[0], 0)
        || !one_object(facts[0], scion)
        || facts[0]
            .mana_colors
            .iter()
            .filter(|color| *color == "Green")
            .count()
            != 3
        || !one_seat(facts[1], 0)
        || !one_object(facts[1], scion)
        || !one_object(facts[2], scion)
        || !one_object(facts[3], scion)
        || facts[3].zone.as_deref() != Some("Battlefield")
        || !one_seat(facts[4], 0)
        || !one_object(facts[4], second)
        || facts[4]
            .mana_colors
            .iter()
            .filter(|color| *color == "Green")
            .count()
            != 1
        || facts[4]
            .mana_colors
            .iter()
            .filter(|color| *color == "White")
            .count()
            != 1
        || !one_seat(facts[5], 0)
        || !one_object(facts[5], second)
        || !one_object(facts[6], second)
        || !one_object(facts[7], second)
        || facts[7].zone.as_deref() != Some("Battlefield")
        || !contains_object(facts[8], scion)
        || !one_seat(facts[8], 0)
        || !one_object(facts[9], scion)
        || facts[9].amount != Some(2)
        || !one_seat(facts[9], 1)
        || !one_seat(facts[10], 0)
    {
        return Err(format!(
            "typed Scion payment/cast/resolution/entry/combat facts differ: {facts:?}"
        ));
    }
    let sequences: Vec<u64> = relevant.iter().map(|(_, event)| event.sequence).collect();
    if !sequences.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err("Scion projected event sequence is not strictly ordered".to_owned());
    }
    Ok(json!({
        "trace_id": "rav-scion-live-controlled-creature-count",
        "seed": seed,
        "decks": ["rav-scion-playset-curve-60", "rav-forest-pass-60"],
        "policies": ["rav.scion-live-creature-count.v1", "rav.scion-pass-opponent.v1"],
        "terminal": capture.terminal,
        "winner_seat": capture.winner,
        "turns": capture.turns,
        "accepted_policy_moves": capture.accepted_moves,
        "rejected_policy_moves": capture.rejected_moves,
        "scion": scion.0,
        "second_creature": second.0,
        "live_before_second_creature": {"power": capture.before.unwrap().0, "toughness": capture.before.unwrap().1, "controlled_creature_count": capture.before.unwrap().2},
        "live_after_second_creature": {"power": capture.after.unwrap().0, "toughness": capture.after.unwrap().1, "controlled_creature_count": capture.after.unwrap().2},
        "scion_mana_paid": capture.scion_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
        "second_creature_mana_paid": capture.second_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
        "scion_combat_damage": capture.first_scion_damage,
        "events": trace_events,
        "passed": true
    }))
}

fn one_object(facts: &EventFacts, object: ObjectId) -> bool {
    facts.objects.len() == 1 && facts.objects[0].0 == object.0
}

fn contains_object(facts: &EventFacts, object: ObjectId) -> bool {
    facts
        .objects
        .iter()
        .any(|candidate| candidate.0 == object.0)
}

fn one_seat(facts: &EventFacts, seat: u16) -> bool {
    facts.seats.len() == 1 && facts.seats[0].0 == seat
}

fn same_match(left: &CapturedMatch, right: &CapturedMatch) -> bool {
    left.events == right.events
        && left.terminal == right.terminal
        && left.winner == right.winner
        && left.turns == right.turns
        && left.accepted_moves == right.accepted_moves
        && left.rejected_moves == right.rejected_moves
        && left.scion == right.scion
        && left.second == right.second
        && left.before == right.before
        && left.after == right.after
        && left.scion_payment == right.scion_payment
        && left.second_payment == right.second_payment
        && left.first_scion_damage == right.first_scion_damage
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
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "credited": false,
        "qualification": if passed { "diagnostic_pilot_pass; independent reviewer approval pending" } else { "not_evaluated" }
    })
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
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
        _ => false,
    }
}
fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc") && matches!(attribute.meta, Meta::NameValue(_))
}
fn type_is(ty: &Type, expected: &[&str]) -> bool {
    matches!(ty, Type::Path(path) if path.qself.is_none() && path_is(&path.path, expected))
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
