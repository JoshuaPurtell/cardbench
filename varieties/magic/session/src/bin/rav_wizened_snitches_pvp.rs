//! Full-game RAV Wizened Snitches authority for Flying and global library-top reveal.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DeckEntry, DeckList, Game, GameEvent, GameView,
    Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction,
    StaticLibraryTopRevealBinding, StaticLibraryTopRevealScope, Zone,
};
use cardbench_magic_policies::{CatalogPolicyProfile, CodePolicy, RavCatalogPolicy};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    card_definitions, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_static_library_top_reveal_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-WIZENED-SNITCHES";
const REMOVE_ID: &str = "RAV-LAST-GASP";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/wizened_snitches.rs";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/wizened_snitches.rs"]
mod candidate_wizened_snitches;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoFlying,
    NoReveal,
    ControllerOnly,
    HigherCost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VisibleTops {
    viewer: PlayerId,
    tops: Vec<(PlayerId, ObjectId, Option<&'static str>)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    snitches: Option<ObjectId>,
    removal: Option<ObjectId>,
    definition_cost: ManaCost,
    mana_paid: Vec<Color>,
    power_toughness: Option<(i32, i32)>,
    keywords: Vec<Keyword>,
    tops_while_active: Vec<VisibleTops>,
    tops_after_departure: Vec<VisibleTops>,
}

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
            eprintln!("Wizened Snitches PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_registry()?;
    validate_candidate_definition()?;
    let catalog = unique_catalog()?;
    let (seed, reference) = find_trace()?;
    validate_trace(&reference)?;
    let replay = run_match(seed, Mutation::None)?;
    if reference != replay {
        return Err("Wizened Snitches shuffled full-game replay was not deterministic".to_owned());
    }

    let mutations = json!({
        "flying_removed_detected_in_game": mutation_detected(seed, Mutation::NoFlying),
        "top_reveal_removed_detected_in_game": mutation_detected(seed, Mutation::NoReveal),
        "reveal_scope_narrowed_detected_in_game": mutation_detected(seed, Mutation::ControllerOnly),
        "printed_cost_changed_detected_in_game": mutation_detected(seed, Mutation::HigherCost),
        "sealed_mutation_authority": false
    });
    for key in [
        "flying_removed_detected_in_game",
        "top_reveal_removed_detected_in_game",
        "reveal_scope_narrowed_detected_in_game",
        "printed_cost_changed_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!("Wizened Snitches gameplay mutation escaped: {key}"));
        }
    }

    let trace = trace_report(&reference, seed)?;
    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 75, "printings": 1, "definition_id": CARD_ID, "name": "Wizened Snitches"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117.3: priority after spells resolve",
                "302: creature characteristics",
                "508: declaring attackers",
                "601.2: casting spells and paying costs",
                "608.2: resolving Last Gasp",
                "702.9: Flying",
                "701.16: revealing information",
                "zone-change visibility and continuous static effects"
            ],
            "source_contract": "RAV #75 is a {3}{U} 1/3 Faerie Rogue with Flying; while it is on the battlefield, each player plays with the top card of each library revealed",
            "authority_cutoff": "2026-09-24"
        },
        "passed": true,
        "pilot_card_passed": true,
        "deterministic_replay": true,
        "cards_passed": 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": rav_main_set_catalog().len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "diagnostic_only; independent review and candidate trial pending",
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect::<Vec<_>>();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
    {
        return Err("RAV catalog denominator differs from the source contract".to_owned());
    }
    Ok(catalog)
}

fn validate_candidate_registry() -> Result<(), String> {
    let candidate = candidate_wizened_snitches::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Wizened Snitches is absent from the RAV card registry")?;
    let binding = candidate_wizened_snitches::static_library_top_reveal_binding();
    let registered_binding = rav_static_library_top_reveal_bindings()
        .into_iter()
        .find(|entry| entry.card_definition == CARD_ID)
        .ok_or("Wizened Snitches static reveal binding is absent from the RAV registry")?;
    if candidate != registered || binding != registered_binding {
        return Err(
            "Wizened Snitches registry does not delegate its full behavior to the candidate module"
                .to_owned(),
        );
    }
    Ok(())
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: "Wizened Snitches",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "flying",
            "global-static-top-library-visibility",
        ],
        power: Some(1),
        toughness: Some(3),
        keywords: vec![Keyword::Flying],
        effects: vec![],
    }
}

fn validate_candidate_definition() -> Result<(), String> {
    if candidate_wizened_snitches::definition() != expected_definition()
        || candidate_wizened_snitches::static_library_top_reveal_binding()
            != (StaticLibraryTopRevealBinding {
                card_definition: CARD_ID,
                scope: StaticLibraryTopRevealScope::EveryPlayer,
            })
    {
        return Err("Wizened Snitches candidate differs from RAV #75 printed characteristics or global reveal behavior".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Wizened Snitches source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Wizened Snitches source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
        || file.items.len() != 4
    {
        return Err("Wizened Snitches source must contain only private imports and two direct literal functions".to_owned());
    }
    let mut imported = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in file.items {
        match item {
            syn::Item::Use(item)
                if item.attrs.is_empty()
                    && matches!(item.vis, syn::Visibility::Inherited)
                    && item.leading_colon.is_none() =>
            {
                if !collect_use_paths(&[], &item.tree, &mut imported) {
                    return Err("Wizened Snitches source contains unsupported imports".to_owned());
                }
            }
            syn::Item::Fn(function) => {
                if !functions.insert(function.sig.ident.to_string())
                    || !direct_literal_function(&function)
                {
                    return Err(
                        "Wizened Snitches source contains executable or nonliteral card behavior"
                            .to_owned(),
                    );
                }
            }
            _ => {
                return Err(
                    "Wizened Snitches source contains unsupported executable items".to_owned(),
                );
            }
        }
    }
    let expected_imports = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "StaticLibraryTopRevealBinding"],
        vec!["cardbench_magic_engine", "StaticLibraryTopRevealScope"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect::<BTreeSet<Vec<String>>>();
    if imported != expected_imports
        || functions
            != BTreeSet::from([
                "definition".to_owned(),
                "static_library_top_reveal_binding".to_owned(),
            ])
    {
        return Err("Wizened Snitches source imports or function contract differs from the reviewed card module".to_owned());
    }
    Ok(())
}

fn collect_use_paths(
    prefix: &[String],
    tree: &syn::UseTree,
    output: &mut BTreeSet<Vec<String>>,
) -> bool {
    match tree {
        syn::UseTree::Path(path) => {
            let mut next = prefix.to_vec();
            next.push(path.ident.to_string());
            collect_use_paths(&next, &path.tree, output)
        }
        syn::UseTree::Name(name) => {
            let mut path = prefix.to_vec();
            path.push(name.ident.to_string());
            output.insert(path);
            true
        }
        syn::UseTree::Group(group) => group
            .items
            .iter()
            .all(|item| collect_use_paths(prefix, item, output)),
        syn::UseTree::Glob(_) | syn::UseTree::Rename(_) => false,
    }
}

fn direct_literal_function(function: &syn::ItemFn) -> bool {
    let wanted = match function.sig.ident.to_string().as_str() {
        "definition" => "CardDefinition",
        "static_library_top_reveal_binding" => "StaticLibraryTopRevealBinding",
        _ => return false,
    };
    let crate_private =
        matches!(&function.vis, syn::Visibility::Restricted(vis) if vis.path.is_ident("crate"));
    let attributes_are_docs = function.attrs.iter().all(|attribute| {
        attribute.path().is_ident("doc")
            || (attribute.path().is_ident("must_use")
                && matches!(attribute.meta, syn::Meta::Path(_)))
    });
    if !crate_private
        || !attributes_are_docs
        || !function.sig.inputs.is_empty()
        || function.sig.constness.is_some()
        || function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
        || !function.sig.generics.params.is_empty()
        || function.block.stmts.len() != 1
    {
        return false;
    }
    matches!(&function.block.stmts[0], syn::Stmt::Expr(syn::Expr::Struct(literal), None)
        if literal.path.segments.last().is_some_and(|segment| segment.ident == wanted)
            && literal.rest.is_none()
            && literal.fields.iter().all(|field| simple_expression(&field.expr)))
}

fn simple_expression(expression: &syn::Expr) -> bool {
    match expression {
        syn::Expr::Lit(literal) => matches!(
            literal.lit,
            syn::Lit::Str(_) | syn::Lit::Int(_) | syn::Lit::Bool(_)
        ),
        syn::Expr::Path(path) => path
            .path
            .segments
            .iter()
            .all(|segment| segment.arguments.is_empty()),
        syn::Expr::Call(call) => {
            matches!(&*call.func, syn::Expr::Path(_)) && call.args.iter().all(simple_expression)
        }
        syn::Expr::Array(array) => array.elems.iter().all(simple_expression),
        syn::Expr::Reference(reference) => simple_expression(&reference.expr),
        syn::Expr::Macro(mac) => mac.mac.path.is_ident("vec") || mac.mac.path.is_ident("BTreeSet"),
        _ => false,
    }
}

fn mutated_definition(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_wizened_snitches::definition();
    match mutation {
        Mutation::None | Mutation::NoReveal | Mutation::ControllerOnly => {}
        Mutation::NoFlying => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Flying),
        Mutation::HigherCost => definition.mana_cost = ManaCost::with_colors(4, [Color::Blue]),
    }
    definition
}

fn mutated_binding(mutation: Mutation) -> Option<StaticLibraryTopRevealBinding> {
    let mut binding = candidate_wizened_snitches::static_library_top_reveal_binding();
    match mutation {
        Mutation::None => Some(binding),
        Mutation::NoFlying | Mutation::HigherCost | Mutation::NoReveal => None,
        Mutation::ControllerOnly => {
            binding.scope = StaticLibraryTopRevealScope::SourceController;
            Some(binding)
        }
    }
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(seed) = std::env::var("CARDBENCH_WIZENED_SNITCHES_SEED") {
        let seed = seed.parse::<u64>().map_err(|_| {
            "CARDBENCH_WIZENED_SNITCHES_SEED must be an unsigned integer".to_owned()
        })?;
        return Ok((seed, run_match(seed, Mutation::None)?));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if !opponent_opens_with_removal(seed)? {
            continue;
        }
        match run_match(seed, Mutation::None) {
            Ok(captured) => match validate_trace(&captured) {
                Ok(()) => return Ok((seed, captured)),
                Err(_) => {}
            },
            Err(_) => {}
        }
    }
    Err(format!(
        "no clean Wizened Snitches full-game trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn opponent_opens_with_removal(seed: u64) -> Result<bool, String> {
    let mut game =
        Game::new_with_basic_land_types(card_definitions(), 2, rav_basic_land_type_bindings())
            .map_err(|error| error.to_string())?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(
        PlayerId(0),
        &deck(&[(CARD_ID, 4), ("RAV-WATCHWOLF", 4), ("RAV-ISLAND", 52)]),
    )
    .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &deck(&[(REMOVE_ID, 4), ("RAV-SWAMP", 56)]))
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), 7)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), 7)
        .map_err(|error| error.to_string())?;
    Ok(game
        .view_for_player(PlayerId(1))
        .map_err(|error| error.to_string())?
        .hand
        .iter()
        .any(|card| card.definition == Some(REMOVE_ID)))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation) {
        Ok(captured) => validate_trace(&captured).is_err(),
        Err(_) => true,
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot = deck(&[(CARD_ID, 4), ("RAV-WATCHWOLF", 4), ("RAV-ISLAND", 52)]);
    let opponent = deck(&[(REMOVE_ID, 4), ("RAV-SWAMP", 56)]);
    if deck_size(&pilot) != 60 || deck_size(&opponent) != 60 {
        return Err("Wizened Snitches fixtures must use shuffled 60-card decks".to_owned());
    }
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Wizened Snitches missing from RAV definitions")? = mutated_definition(mutation);
    let mut game = Game::new_with_basic_land_types(definitions, 2, rav_basic_land_type_bindings())
        .map_err(|error| error.to_string())?;
    if let Some(binding) = mutated_binding(mutation) {
        let mut bindings = rav_static_library_top_reveal_bindings()
            .into_iter()
            .filter(|entry| entry.card_definition != CARD_ID)
            .collect::<Vec<_>>();
        bindings.push(binding);
        game.register_static_library_top_reveal_bindings(bindings)
            .map_err(|error| error.to_string())?;
    } else {
        game.register_static_library_top_reveal_bindings(
            rav_static_library_top_reveal_bindings()
                .into_iter()
                .filter(|entry| entry.card_definition != CARD_ID),
        )
        .map_err(|error| error.to_string())?;
    }
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), 7)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), 7)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(SnitchesPilotPolicy::new()),
        Box::new(SnitchesRemovalPolicy::new()),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut snitches = None;
    let mut removal = None;
    let mut definition_cost = mutated_definition(mutation).mana_cost;
    let mut power_toughness = None;
    let mut keywords = Vec::new();
    let mut tops_while_active = Vec::new();
    let mut tops_after_departure = Vec::new();
    let mut saw_resolved = false;
    let mut saw_departure = false;
    while !game.is_game_over() {
        if game.turn > 60 || accepted >= 2_000 {
            return Err(format!(
                "Wizened Snitches match exceeded bounds at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Wizened Snitches policy seat is missing")?;
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
        let action_debug = format!("{action:?}");
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| format!("Wizened Snitches policy move rejected at turn {} {:?} for player {} ({action_debug}): {error}", view.turn, view.step, player.0))?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("Wizened Snitches post-move invariant failure: {error}"))?;

        if snitches.is_none() {
            snitches = game.event_log.iter().find_map(|event| match event {
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
            if let Some(card) = snitches {
                definition_cost = game
                    .card_definition(card)
                    .map_err(|error| error.to_string())?
                    .mana_cost
                    .clone();
            }
        }
        if let Some(card) = snitches
            && !saw_resolved
            && game.event_log.iter().any(|event| matches!(event, GameEvent::SpellResolved { card: resolved } if *resolved == card))
            && game.zone_of(card) == Some(Zone::Battlefield)
        {
            let characteristics = game.characteristics(card).map_err(|error| error.to_string())?;
            power_toughness = Some((characteristics.power.ok_or("Wizened Snitches has no live power")?, characteristics.toughness.ok_or("Wizened Snitches has no live toughness")?));
            keywords = characteristics.keywords.clone();
            tops_while_active = capture_tops(&game)?;
            saw_resolved = true;
        }
        if let Some(card) = snitches
            && saw_resolved
            && !saw_departure
            && game.zone_of(card) == Some(Zone::Graveyard)
        {
            removal = game.event_log.iter().find_map(|event| match event {
                GameEvent::SpellCast {
                    player: PlayerId(1),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == REMOVE_ID) =>
                {
                    Some(*card)
                }
                _ => None,
            });
            tops_after_departure = capture_tops(&game)?;
            saw_departure = true;
        }
    }
    let winner = game.winner().map(|player| player.0);
    if snitches.is_none() || !saw_resolved || !saw_departure {
        return Err(format!(
            "Wizened Snitches, its reveal window, or its departure were not exercised (turn {}, snitches={snitches:?}, resolved={saw_resolved}, departed={saw_departure})",
            game.turn
        ));
    }
    let mana_paid = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if Some(*card) == snitches => Some(colors.clone()),
            _ => None,
        })
        .ok_or("Wizened Snitches exact mana payment receipt is missing")?;
    Ok(Captured {
        events: game.event_log.clone(),
        winner,
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        snitches,
        removal,
        definition_cost,
        mana_paid,
        power_toughness,
        keywords,
        tops_while_active,
        tops_after_departure,
    })
}

fn capture_tops(game: &Game) -> Result<Vec<VisibleTops>, String> {
    [PlayerId(0), PlayerId(1)]
        .into_iter()
        .map(|viewer| {
            let view = game
                .view_for_player(viewer)
                .map_err(|error| error.to_string())?;
            let mut tops = view
                .revealed_library_tops
                .into_iter()
                .map(|top| (top.owner, top.card.id, top.card.definition))
                .collect::<Vec<_>>();
            tops.sort_by_key(|entry| entry.0);
            Ok(VisibleTops { viewer, tops })
        })
        .collect()
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

fn deck_size(deck: &DeckList) -> usize {
    deck.mainboard
        .iter()
        .map(|entry| usize::from(entry.count))
        .sum()
}

struct SnitchesPilotPolicy {
    inner: RavCatalogPolicy,
}

impl SnitchesPilotPolicy {
    fn new() -> Self {
        Self {
            inner: RavCatalogPolicy::new(PlayerId(0), CatalogPolicyProfile::Curve),
        }
    }
}

impl CodePolicy for SnitchesPilotPolicy {
    fn id(&self) -> &'static str {
        "rav.wizened-snitches-global-reveal-pilot.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player == PlayerId(0)
            && view.decision_player == PlayerId(0)
            && view.priority == PlayerId(0)
            && view.active_player == PlayerId(0)
            && view.step.is_main()
            && view.stack_depth == 0
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            && let Some(selection) = payment_selection(
                &view.mana_pool,
                &candidate_wizened_snitches::definition().mana_cost,
            )
        {
            return PolicyAction::CastWithPayment {
                request: CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection: selection,
            };
        }
        self.inner.propose_move(view)
    }
}

struct SnitchesRemovalPolicy {
    inner: RavCatalogPolicy,
    used_last_gasp: bool,
}

impl SnitchesRemovalPolicy {
    fn new() -> Self {
        Self {
            inner: RavCatalogPolicy::new(PlayerId(1), CatalogPolicyProfile::Patient),
            used_last_gasp: false,
        }
    }
}

impl CodePolicy for SnitchesRemovalPolicy {
    fn id(&self) -> &'static str {
        "rav.wizened-snitches-reveal-removal-opponent.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if !self.used_last_gasp
            && view.player == PlayerId(1)
            && view.decision_player == PlayerId(1)
            && view.priority == PlayerId(1)
            && view.stack_depth == 0
            && let Some(target) = view
                .opponent_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            && let Some(spell) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(REMOVE_ID))
        {
            let removal_cost = ManaCost::with_colors(1, [Color::Black]);
            if can_pay(&view.mana_pool, &removal_cost) {
                self.used_last_gasp = true;
                return PolicyAction::Cast(CastRequest {
                    card: spell.id,
                    targets: vec![cardbench_magic_engine::Target::Permanent(target.id)],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                });
            }
            if let Some(swamp) = view
                .own_battlefield
                .iter()
                .find(|card| !card.tapped && card.mana_colors.contains(&Color::Black))
            {
                return PolicyAction::ActivateManaAbility {
                    land: swamp.id,
                    color: Color::Black,
                };
            }
        }
        self.inner.propose_move(view)
    }
}

fn can_pay(pool: &cardbench_magic_engine::ManaPool, cost: &ManaCost) -> bool {
    let colored = cost.colored.iter().all(|color| pool.amount(*color) > 0);
    colored && pool.total_exact() >= u16::from(cost.generic) + cost.colored.len() as u16
}

fn payment_selection(
    pool: &cardbench_magic_engine::ManaPool,
    cost: &ManaCost,
) -> Option<ManaPaymentSelection> {
    let mut available: Vec<_> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let (_, amount) = available.iter_mut().find(|(known, _)| known == color)?;
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

fn validate_trace(captured: &Captured) -> Result<(), String> {
    let snitches = captured
        .snitches
        .ok_or("Wizened Snitches was not cast from the shuffled deck")?;
    if captured.winner != Some(0) || captured.rejected != 0 || captured.turns > 60 {
        return Err("Wizened Snitches full-game duel did not end in a clean pilot win".to_owned());
    }
    if captured.definition_cost != ManaCost::with_colors(3, [Color::Blue])
        || captured.power_toughness != Some((1, 3))
        || !captured.keywords.contains(&Keyword::Flying)
        || captured.mana_paid.len() != 4
        || !captured.mana_paid.contains(&Color::Blue)
    {
        return Err(format!(
            "Wizened Snitches cost, 1/3 characteristics, Flying, or payment differs from RAV #75: cost={:?}, paid={:?}, power_toughness={:?}, keywords={:?}",
            captured.definition_cost,
            captured.mana_paid,
            captured.power_toughness,
            captured.keywords
        ));
    }
    let active = &captured.tops_while_active;
    if active.len() != 2
        || active.iter().any(|viewer| viewer.tops.len() != 2)
        || active[0].tops != active[1].tops
        || active
            .iter()
            .any(|viewer| viewer.tops.iter().any(|top| top.1.0 == 0))
    {
        return Err(
            "Wizened Snitches did not reveal both current library tops to both players".to_owned(),
        );
    }
    if captured.removal.is_none()
        || captured.tops_after_departure.len() != 2
        || captured
            .tops_after_departure
            .iter()
            .any(|viewer| !viewer.tops.is_empty())
    {
        return Err(
            "removing Wizened Snitches did not revoke its public library-top visibility".to_owned(),
        );
    }
    let paid = captured.events.iter().position(|event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == snitches && colors == &captured.mana_paid)).ok_or("Wizened Snitches payment event is missing")?;
    let cast = captured.events.iter().position(|event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == snitches)).ok_or("Wizened Snitches cast event is missing")?;
    let resolved = captured
        .events
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == snitches))
        .ok_or("Wizened Snitches resolution event is missing")?;
    let entered = captured.events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == snitches)).ok_or("Wizened Snitches battlefield entry event is missing")?;
    let removal = captured.events.iter().position(|event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if Some(*card) == captured.removal)).ok_or("Last Gasp cast event is missing")?;
    let removal_resolved = captured.events.iter().position(|event| matches!(event, GameEvent::SpellResolved { card } if Some(*card) == captured.removal)).ok_or("Last Gasp resolution event is missing")?;
    let departed = captured.events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == snitches)).ok_or("Wizened Snitches departure event is missing")?;
    let ended = captured
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
        .ok_or("Wizened Snitches terminal win event is missing")?;
    if !(paid < cast
        && cast < resolved
        && resolved < entered
        && entered < removal
        && removal < removal_resolved
        && removal_resolved < departed
        && departed < ended)
    {
        return Err("Wizened Snitches ordered cast, battlefield, removal, or terminal events are incomplete".to_owned());
    }
    Ok(())
}

fn trace_report(captured: &Captured, seed: u64) -> Result<Value, String> {
    validate_trace(captured)?;
    let snitches = captured
        .snitches
        .ok_or("Wizened Snitches identity is missing")?;
    let projected = project_events(&captured.events);
    let event_values = serde_json::to_value(projected).map_err(|error| error.to_string())?;
    let tops = captured.tops_while_active.iter().map(|entry| json!({
        "viewer": entry.viewer.0,
        "tops": entry.tops.iter().map(|top| json!({"owner": top.0.0, "object": top.1.0, "definition": top.2})).collect::<Vec<_>>()
    })).collect::<Vec<_>>();
    let revoked = captured
        .tops_after_departure
        .iter()
        .map(|entry| json!({"viewer": entry.viewer.0, "count": entry.tops.len()}))
        .collect::<Vec<_>>();
    let mana_paid_sequence = captured
        .events
        .iter()
        .position(|event| {
            matches!(event,
                GameEvent::SpellManaPaid { player: PlayerId(0), card, colors }
                    if *card == snitches && colors == &captured.mana_paid
            )
        })
        .ok_or("Wizened Snitches payment sequence is missing")?;
    Ok(json!({
        "trace_id": "rav-wizened-snitches-global-reveal-terminal-duel",
        "seed": seed,
        "terminal": "Winner(PlayerId(0))",
        "turns": captured.turns,
        "accepted_policy_moves": captured.accepted,
        "rejected_policy_moves": captured.rejected,
        "passed": true,
        "mechanics": {
            "collector_number": 75,
            "cast_from_shuffled_library": true,
            "last_gasp_object": captured.removal.map(|object| object.0),
            "last_gasp_target_object": snitches.0,
            "object": snitches.0,
            "mana_cost": "{3}{U}",
            "mana_paid": captured.mana_paid.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "mana_paid_sequence": mana_paid_sequence,
            "power_toughness": [1, 3],
            "flying": captured.keywords.contains(&Keyword::Flying),
            "active_reveal_tops": tops,
            "removed_by_last_gasp": captured.removal.is_some(),
            "reveal_counts_after_departure": revoked,
            "terminal_winner": 0
        },
        "events": event_values
    }))
}

fn card_status(card: &CatalogCard) -> Value {
    let status = if card.semantic_status.is_executable() {
        "executable_not_per_card_credited"
    } else {
        "catalog_only"
    };
    json!({"collector_number":card.collector_number,"name":card.name,"semantic_status":status,"trace_status":if card.name == "Wizened Snitches" { "passed" } else { "not_run" },"credited":false,"reward":0.0})
}
