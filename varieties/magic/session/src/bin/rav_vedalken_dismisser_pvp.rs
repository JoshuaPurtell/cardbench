//! Full-game diagnostic authority for RAV Vedalken Dismisser.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DeckEntry, DeckList, Effect, Game, GameEvent,
    GameView, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target,
    TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use syn::parse::Parser;
use syn::{Expr, Item, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-VEDALKEN-DISMISSER";
const TRIGGER_ID: &str = "etb-target-creature-owner-library-top";
const PILOT_FINISHER_ID: &str = "RAV-WATCHWOLF";
const OPPONENT_WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const OPPONENT_BIRD_ID: &str = "RAV-BIRDS-OF-PARADISE";
const OPPONENT_HAWK_ID: &str = "RAV-COURIER-HAWK";
const ISLAND_ID: &str = "RAV-ISLAND";
const FOREST_ID: &str = "RAV-FOREST";
const PLAINS_ID: &str = "RAV-PLAINS";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/vedalken_dismisser.rs";
const MAX_TRACE_SEED: u64 = 32;
const MAX_TURNS: u32 = 40;
const MAX_MOVES: u32 = 5_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/vedalken_dismisser.rs"]
mod candidate_vedalken_dismisser;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Vedalken Dismisser result: {error}");
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
            eprintln!("Vedalken Dismisser PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    TriggerRemoved,
    OptionalTrigger,
    WrongDestination,
    WrongBody,
    WrongCost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    source: Option<ObjectId>,
    spell_payment: Vec<Color>,
    spell_resolved: bool,
    source_entered: bool,
    source_power: Option<i32>,
    source_toughness: Option<i32>,
    selected_target: Option<Target>,
    target_controller: Option<PlayerId>,
    target_owner: Option<PlayerId>,
    target_incarnation_before: Option<u64>,
    target_incarnation_after: Option<u64>,
    target_zone_after: Option<Zone>,
    target_library_before: Vec<ObjectId>,
    target_library_after: Vec<ObjectId>,
    target_is_library_top: bool,
    target_decision_player: Option<PlayerId>,
    trigger_stacked: bool,
    trigger_source_incarnation: Option<u64>,
    trigger_resolved_incarnation: Option<u64>,
    finisher: Option<ObjectId>,
    finisher_attack_turns: Vec<u32>,
    finisher_damage: i32,
    life: [i64; 2],
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_registry()?;
    let (seed, capture) = find_trace()?;
    let trace = validate_trace(&capture, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if capture != replay || project_events(&capture.events) != project_events(&replay.events) {
        return Err("Vedalken Dismisser full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "etb_trigger_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::TriggerRemoved)?,
        "trigger_made_optional_mutation_detected_in_game": mutation_detected(seed, Mutation::OptionalTrigger)?,
        "target_returned_to_hand_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongDestination)?,
        "printed_body_changed_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongBody)?,
        "printed_cost_changed_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongCost)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "etb_trigger_removed_mutation_detected_in_game",
        "trigger_made_optional_mutation_detected_in_game",
        "target_returned_to_hand_mutation_detected_in_game",
        "printed_body_changed_mutation_detected_in_game",
        "printed_cost_changed_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!("Vedalken Dismisser gameplay mutant escaped: {key}"));
        }
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
        || catalog.len() != 291
        || printings.len() != 306
    {
        return Err("RAV catalog denominator differs from the shared source contract".to_owned());
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 73, "printings": 1, "definition_id": CARD_ID, "name": "Vedalken Dismisser"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "601.2: casting the creature spell",
                "603.3: putting the triggered ability on the stack",
                "603.3d: choosing a target for a triggered ability",
                "608.2: resolving the targeted trigger",
                "701.13: moving a permanent to its owner's library"
            ],
            "source_contract": "RAV Vedalken Dismisser #73 is a blue 2/2 Creature costing {5}{U}; when it enters, target creature is put on top of its owner's library.",
            "authority_cutoff": "2026-09-24",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "deterministic_replay": true,
        "cards_passed": 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
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

fn candidate_definition(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_vedalken_dismisser::definition();
    match mutation {
        Mutation::WrongBody => {
            definition.power = Some(3);
            definition.toughness = Some(2);
        }
        Mutation::WrongCost => definition.mana_cost = ManaCost::with_colors(4, [Color::Blue]),
        _ => {}
    }
    definition
}

fn candidate_trigger(mutation: Mutation) -> TriggeredAbilityBinding {
    let mut binding = candidate_vedalken_dismisser::triggered_ability();
    match mutation {
        Mutation::OptionalTrigger => binding.ability.optional = true,
        Mutation::WrongDestination => {
            binding.ability.targets = vec![TargetRequirement::OpponentCreature];
            binding.ability.effects = vec![Effect::ReturnOpponentCreatureToHand];
        }
        _ => {}
    }
    binding
}

fn validate_candidate_registry() -> Result<(), String> {
    let definition = candidate_vedalken_dismisser::definition();
    let ability = candidate_vedalken_dismisser::triggered_ability();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Vedalken Dismisser is missing from the RAV definition registry")?;
    let bindings = rav_triggered_ability_bindings();
    let candidates: Vec<_> = bindings
        .iter()
        .filter(|binding| binding.card_definition == CARD_ID)
        .collect();
    if registered != definition
        || candidates.len() != 1
        || *candidates[0] != ability
        || definition != expected_definition()
        || ability != expected_trigger()
    {
        return Err("Vedalken Dismisser candidate definition or mandatory ETB trigger differs from the RAV source contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Vedalken Dismisser source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Vedalken Dismisser source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
        || file.items.len() != 4
    {
        return Err("Vedalken Dismisser module must contain private imports and two direct-literal functions".to_owned());
    }
    let mut imports = 0;
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                imports += 1;
                if !safe_use_tree(&item.tree) {
                    return Err("Vedalken Dismisser contains an unsupported import".to_owned());
                }
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !functions.insert(name.clone())
                    || !function.attrs.iter().all(|attribute| {
                        attribute.path().is_ident("must_use") || attribute.path().is_ident("doc")
                    })
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !function.sig.inputs.is_empty()
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || !function.sig.generics.params.is_empty()
                    || function.block.stmts.len() != 1
                    || !matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(literal), None) if literal.rest.is_none() && literal.fields.iter().all(|field| safe_expression(&field.expr)))
                {
                    return Err(format!(
                        "Vedalken Dismisser function `{name}` is not a direct declarative literal"
                    ));
                }
            }
            _ => {
                return Err(
                    "Vedalken Dismisser source contains an executable or unsupported item"
                        .to_owned(),
                );
            }
        }
    }
    if imports != 2
        || functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()])
    {
        return Err("Vedalken Dismisser source imports or function identity differs from the source contract".to_owned());
    }
    Ok(())
}

fn safe_use_tree(tree: &UseTree) -> bool {
    match tree {
        UseTree::Path(path) => {
            matches!(
                path.ident.to_string().as_str(),
                "std" | "collections" | "cardbench_magic_engine"
            ) && safe_use_tree(&path.tree)
        }
        UseTree::Name(name) => matches!(
            name.ident.to_string().as_str(),
            "BTreeSet"
                | "CardDefinition"
                | "CardType"
                | "Color"
                | "Effect"
                | "ManaCost"
                | "TargetRequirement"
                | "TriggerCondition"
                | "TriggeredAbility"
                | "TriggeredAbilityBinding"
        ),
        UseTree::Group(group) => group.items.iter().all(safe_use_tree),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}

fn safe_expression(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(literal) => matches!(
            literal.lit,
            syn::Lit::Str(_) | syn::Lit::Int(_) | syn::Lit::Bool(_)
        ),
        Expr::Path(path) => {
            let segments: Vec<_> = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            path_is(&segments, &["None"])
                || path_is(&segments, &["Color", "Blue"])
                || path_is(&segments, &["CardType", "Creature"])
                || path_is(&segments, &["TriggerCondition", "EntersBattlefield"])
                || path_is(&segments, &["TargetRequirement", "Creature"])
                || path_is(
                    &segments,
                    &["Effect", "PutTargetCreatureOnOwnersLibraryTop"],
                )
        }
        Expr::Call(call) => {
            let Expr::Path(path) = &*call.func else {
                return false;
            };
            let segments: Vec<_> = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            (path_is(&segments, &["ManaCost", "with_colors"])
                || path_is(&segments, &["ManaCost", "new"])
                || path_is(&segments, &["BTreeSet", "from"])
                || path_is(&segments, &["BTreeSet", "new"])
                || path_is(&segments, &["Some"]))
                && call.args.iter().all(safe_expression)
        }
        Expr::Struct(literal) => {
            let segments: Vec<_> = literal
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            (path_is(&segments, &["CardDefinition"])
                || path_is(&segments, &["TriggeredAbility"])
                || path_is(&segments, &["TriggeredAbilityBinding"])
                || path_is(
                    &segments,
                    &["Effect", "PutTargetCreatureOnOwnersLibraryTop"],
                ))
                && literal.rest.is_none()
                && literal
                    .fields
                    .iter()
                    .all(|field| safe_expression(&field.expr))
        }
        Expr::Array(array) => array.elems.iter().all(safe_expression),
        Expr::Reference(reference) => safe_expression(&reference.expr),
        Expr::Macro(mac) if mac.mac.path.is_ident("vec") => {
            syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
                .parse2(mac.mac.tokens.clone())
                .is_ok_and(|values| values.iter().all(safe_expression))
        }
        _ => false,
    }
}

fn path_is(actual: &[String], expected: &[&str]) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(actual, expected)| actual == expected)
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: "Vedalken Dismisser",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(5, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "etb-target-creature-owner-library-top",
            "policy-submitted-trigger-target",
        ],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

fn expected_trigger() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: CARD_ID,
        ability: TriggeredAbility {
            id: TRIGGER_ID,
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::Creature],
            effects: vec![Effect::PutTargetCreatureOnOwnersLibraryTop],
        },
    }
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "agent"
    } else {
        "reference"
    }
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

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Vedalken Dismisser definition is missing from the RAV registry")? =
        candidate_definition(mutation);
    let mut triggers = rav_triggered_ability_bindings();
    let matching: Vec<_> = triggers
        .iter()
        .enumerate()
        .filter(|(_, binding)| binding.card_definition == CARD_ID)
        .map(|(index, _)| index)
        .collect();
    match mutation {
        Mutation::TriggerRemoved => {
            triggers.retain(|binding| binding.card_definition != CARD_ID);
        }
        _ => {
            if matching.len() != 1 {
                return Err(format!(
                    "expected one Dismisser trigger binding, found {}",
                    matching.len()
                ));
            }
            triggers[matching[0]] = candidate_trigger(mutation);
        }
    }
    Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers,
    )
    .map_err(|error| error.to_string())
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_VEDALKEN_DISMISSER_SEED") {
        let seed = value.parse::<u64>().map_err(|_| {
            "CARDBENCH_VEDALKEN_DISMISSER_SEED must be an unsigned integer".to_owned()
        })?;
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
        "no Vedalken Dismisser full-game trace found for seeds 0..={MAX_TRACE_SEED}: {last_error}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot = deck(&[
        (CARD_ID, 4),
        (PILOT_FINISHER_ID, 4),
        (ISLAND_ID, 24),
        (FOREST_ID, 14),
        (PLAINS_ID, 14),
    ]);
    let opponent = deck(&[
        (OPPONENT_BIRD_ID, 4),
        (OPPONENT_HAWK_ID, 4),
        (OPPONENT_WATCHWOLF_ID, 4),
        (FOREST_ID, 24),
        (PLAINS_ID, 24),
    ]);
    for list in [&pilot, &opponent] {
        if list
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
        {
            return Err("Vedalken Dismisser fixtures must be two legal 60-card decks".to_owned());
        }
    }
    let mut game = new_game(mutation)?;
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_MOVES,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    for (player, list) in [(PlayerId(0), &pilot), (PlayerId(1), &opponent)] {
        game.load_deck_into_library(player, list)
            .map_err(|error| error.to_string())?;
        game.draw_opening_hand(player, config.opening_hand_size)
            .map_err(|error| error.to_string())?;
    }
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(DismisserPolicy::new(PlayerId(0), mutation)),
        Box::new(TargetCreaturePolicy::new(PlayerId(1))),
    ];
    let mut capture = Captured {
        events: vec![],
        winner: None,
        turns: 0,
        accepted: 0,
        rejected: 0,
        source: None,
        spell_payment: vec![],
        spell_resolved: false,
        source_entered: false,
        source_power: None,
        source_toughness: None,
        selected_target: None,
        target_controller: None,
        target_owner: None,
        target_incarnation_before: None,
        target_incarnation_after: None,
        target_zone_after: None,
        target_library_before: vec![],
        target_library_after: vec![],
        target_is_library_top: false,
        target_decision_player: None,
        trigger_stacked: false,
        trigger_source_incarnation: None,
        trigger_resolved_incarnation: None,
        finisher: None,
        finisher_attack_turns: vec![],
        finisher_damage: 0,
        life: [20, 20],
    };
    while !game.is_game_over() {
        if game.turn > config.max_turns || capture.accepted >= config.max_policy_moves {
            return Err(format!(
                "Vedalken Dismisser duel exceeded its action bound at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Vedalken Dismisser policy seat missing")?;
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
        let chosen_target = if let PolicyAction::ChooseTriggeredAbilityTargets {
            source,
            ability,
            targets,
            ..
        } = &action
            && *ability == TRIGGER_ID
            && capture.source == Some(*source)
        {
            captured_decision_target(&view, &mut capture);
            targets.first().cloned()
        } else {
            None
        };
        if let PolicyAction::CastWithPayment { request, .. } = &action
            && game
                .card_definition(request.card)
                .is_ok_and(|definition| definition.id == CARD_ID)
        {
            capture.source = Some(request.card);
        }
        if let PolicyAction::CastWithPayment { request, .. } = &action
            && game
                .card_definition(request.card)
                .is_ok_and(|definition| definition.id == PILOT_FINISHER_ID)
        {
            capture.finisher = Some(request.card);
        }
        let cursor = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Vedalken Dismisser move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        capture.accepted += 1;
        if let Some(target) = chosen_target {
            capture.selected_target = Some(target.clone());
            if let Target::Permanent(object) = target {
                let target_object = game.object(object).map_err(|error| error.to_string())?;
                capture.target_controller = Some(target_object.controller);
                capture.target_owner = Some(target_object.owner);
                capture.target_incarnation_before = Some(target_object.incarnation);
                capture.target_library_before = game
                    .player(target_object.owner)
                    .map_err(|error| error.to_string())?
                    .library
                    .clone();
            }
        }
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        observe(&game, &game.event_log[cursor..], &mut capture)?;
    }
    capture.life = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    capture.events = game.event_log.clone();
    capture.winner = game.winner().map(|player| player.0);
    capture.turns = game.turn;
    Ok(capture)
}

fn captured_decision_target(view: &GameView, capture: &mut Captured) {
    if let Some(choice) = &view.triggered_ability_target_choice {
        capture.target_decision_player = Some(view.player);
        if choice.ability == TRIGGER_ID && view.player == PlayerId(0) {
            // The submitted action below remains the authority for the exact chosen object.
        }
    }
}

fn observe(game: &Game, events: &[GameEvent], capture: &mut Captured) -> Result<(), String> {
    for event in events {
        match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if Some(*card) == capture.source => capture.spell_payment = colors.clone(),
            GameEvent::SpellResolved { card } if Some(*card) == capture.source => {
                capture.spell_resolved = true
            }
            GameEvent::CardMoved {
                card,
                to: Zone::Battlefield,
            } if Some(*card) == capture.source => {
                capture.source_entered = true;
                let characteristics = game
                    .characteristics(*card)
                    .map_err(|error| error.to_string())?;
                capture.source_power = characteristics.power;
                capture.source_toughness = characteristics.toughness;
            }
            GameEvent::DecisionOpened {
                player,
                kind: cardbench_magic_engine::DecisionKind::TriggeredAbilityTargets,
                ..
            } if *player == PlayerId(0) => capture.target_decision_player = Some(*player),
            GameEvent::TriggeredAbilityStacked {
                source,
                source_incarnation,
                ability,
                controller,
            } if Some(*source) == capture.source
                && *ability == TRIGGER_ID
                && *controller == PlayerId(0) =>
            {
                capture.trigger_stacked = true;
                capture.trigger_source_incarnation = Some(*source_incarnation);
            }
            GameEvent::CardMoved {
                card,
                to: Zone::Library,
            } if capture.selected_target == Some(Target::Permanent(*card)) => {
                capture.target_zone_after = Some(Zone::Library);
                capture.target_incarnation_after =
                    game.object(*card).ok().map(|object| object.incarnation);
                if let Some(owner) = capture.target_owner {
                    capture.target_library_after = game
                        .player(owner)
                        .map_err(|error| error.to_string())?
                        .library
                        .clone();
                    capture.target_is_library_top =
                        capture.target_library_after.last() == Some(card);
                }
            }
            GameEvent::AbilityResolved {
                source,
                source_incarnation,
                ability,
            } if Some(*source) == capture.source && *ability == TRIGGER_ID => {
                capture.trigger_resolved_incarnation = Some(*source_incarnation)
            }
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == PILOT_FINISHER_ID) =>
            {
                capture.finisher = Some(*card)
            }
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } if capture
                .finisher
                .is_some_and(|wolf| attackers.contains(&wolf)) =>
            {
                capture.finisher_attack_turns.push(game.turn)
            }
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if capture.finisher == Some(*source) => capture.finisher_damage += *amount,
            _ => {}
        }
    }
    Ok(())
}

struct DismisserPolicy {
    player: PlayerId,
    mutation: Mutation,
    dismisser_cast: bool,
    finisher_cast: bool,
}
impl DismisserPolicy {
    fn new(player: PlayerId, mutation: Mutation) -> Self {
        Self {
            player,
            mutation,
            dismisser_cast: false,
            finisher_cast: false,
        }
    }
    fn target(&self, view: &GameView) -> Option<PolicyAction> {
        let choice = view.triggered_ability_target_choice.as_ref()?;
        if choice.ability != TRIGGER_ID || view.player != self.player {
            return None;
        }
        let options = choice.target_options.first()?;
        let target = view
            .opponent_battlefield
            .iter()
            .find(|card| card.card_types.contains(&CardType::Creature))
            .map(|card| Target::Permanent(card.id))
            .filter(|target| options.contains(target))
            .or_else(|| options.first().copied())?;
        Some(PolicyAction::ChooseTriggeredAbilityTargets {
            decision: choice.decision,
            source: choice.source,
            ability: choice.ability,
            targets: vec![target],
        })
    }
    fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let mut available: Vec<_> = Color::MANA_ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
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
    fn make_mana(view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
        let missing = cost
            .colored
            .iter()
            .find(|color| view.mana_pool.amount(**color) == 0)
            .copied()
            .or_else(|| {
                (view.mana_pool.total() < cost.generic.saturating_add(cost.colored.len() as u8))
                    .then(|| cost.colored.first().copied())
                    .flatten()
            })
            .unwrap_or(Color::Blue);
        let land = view.own_battlefield.iter().find(|card| {
            card.card_types.contains(&CardType::Land)
                && !card.tapped
                && card.mana_colors.contains(&missing)
        })?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color: missing,
        })
    }
    fn play_land(view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let have_blue = view.own_battlefield.iter().any(|card| {
            card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&Color::Blue)
        });
        let have_green = view.own_battlefield.iter().any(|card| {
            card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&Color::Green)
        });
        let have_white = view.own_battlefield.iter().any(|card| {
            card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&Color::White)
        });
        let first = if !have_blue {
            ISLAND_ID
        } else if !have_green {
            FOREST_ID
        } else if !have_white {
            PLAINS_ID
        } else {
            ISLAND_ID
        };
        view.hand
            .iter()
            .find(|card| card.definition == Some(first))
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some(ISLAND_ID))
            })
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some(FOREST_ID))
            })
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some(PLAINS_ID))
            })
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }
    fn cast(&mut self, view: &GameView, id: &'static str, cost: &ManaCost) -> Option<PolicyAction> {
        if let Some(mana_selection) = Self::payment(view, cost) {
            let card = view.hand.iter().find(|card| card.definition == Some(id))?;
            if id == CARD_ID {
                self.dismisser_cast = true;
            }
            if id == PILOT_FINISHER_ID {
                self.finisher_cast = true;
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
        Self::make_mana(view, cost)
    }
}
impl CodePolicy for DismisserPolicy {
    fn id(&self) -> &'static str {
        "rav.vedalken-dismisser-full-game-pilot.v1"
    }
    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        self.target(view)
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
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(PILOT_FINISHER_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect(),
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
        if view.priority != self.player || view.stack_depth != 0 || self.player != PlayerId(0) {
            return PolicyAction::PassPriority;
        }
        if view.active_player == self.player && view.step.is_main() {
            if view.lands_played == 0
                && let Some(action) = Self::play_land(view)
            {
                return action;
            }
            let has_target = view
                .opponent_battlefield
                .iter()
                .any(|card| card.card_types.contains(&CardType::Creature));
            if !self.dismisser_cast
                && has_target
                && view
                    .hand
                    .iter()
                    .any(|card| card.definition == Some(CARD_ID))
            {
                let cost = candidate_definition(self.mutation).mana_cost;
                if let Some(action) = self.cast(view, CARD_ID, &cost) {
                    return action;
                }
            }
            if self.dismisser_cast
                && !self.finisher_cast
                && view
                    .hand
                    .iter()
                    .any(|card| card.definition == Some(PILOT_FINISHER_ID))
            {
                let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
                if let Some(action) = self.cast(view, PILOT_FINISHER_ID, &cost) {
                    return action;
                }
            }
        }
        PolicyAction::PassPriority
    }
}

struct TargetCreaturePolicy {
    player: PlayerId,
    creature_cast: bool,
}
impl TargetCreaturePolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            creature_cast: false,
        }
    }
}
impl CodePolicy for TargetCreaturePolicy {
    fn id(&self) -> &'static str {
        "rav.vedalken-dismisser-passive-target-player.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
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
        if view.priority != self.player
            || view.stack_depth != 0
            || view.active_player != self.player
        {
            return PolicyAction::PassPriority;
        }
        if view.step.is_main() {
            if view.lands_played == 0
                && let Some(action) = DismisserPolicy::play_land(view)
            {
                return action;
            }
            let has_creature = view
                .own_battlefield
                .iter()
                .any(|card| card.card_types.contains(&CardType::Creature));
            if !self.creature_cast && !has_creature {
                for (id, cost) in [
                    (OPPONENT_BIRD_ID, ManaCost::with_colors(0, [Color::Green])),
                    (OPPONENT_HAWK_ID, ManaCost::with_colors(1, [Color::White])),
                    (
                        OPPONENT_WATCHWOLF_ID,
                        ManaCost::with_colors(0, [Color::Green, Color::White]),
                    ),
                ] {
                    if view.hand.iter().any(|card| card.definition == Some(id)) {
                        if let Some(action) = self.cast_creature(view, id, &cost) {
                            return action;
                        }
                    }
                }
            }
        }
        PolicyAction::PassPriority
    }
    fn propose_pending_decision(&mut self, _view: &GameView) -> Option<PolicyAction> {
        None
    }
}
impl TargetCreaturePolicy {
    fn cast_creature(
        &mut self,
        view: &GameView,
        id: &'static str,
        cost: &ManaCost,
    ) -> Option<PolicyAction> {
        if let Some(mana_selection) = DismisserPolicy::payment(view, cost) {
            let card = view.hand.iter().find(|card| card.definition == Some(id))?;
            self.creature_cast = true;
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
        DismisserPolicy::make_mana(view, cost)
    }
}

fn validate_trace(capture: &Captured, seed: u64) -> Result<Value, String> {
    if capture.winner != Some(0) || capture.rejected != 0 {
        return Err("Vedalken Dismisser duel did not reach a clean P0 terminal win".to_owned());
    }
    let source = capture.source.ok_or("Vedalken Dismisser was not cast")?;
    let target = match capture.selected_target {
        Some(Target::Permanent(target)) => target,
        _ => return Err("Dismisser trigger did not choose an exact permanent".to_owned()),
    };
    if !capture.spell_resolved
        || !capture.source_entered
        || capture.spell_payment.len() != 6
        || !capture.spell_payment.contains(&Color::Blue)
        || capture.source_power != Some(2)
        || capture.source_toughness != Some(2)
    {
        return Err(
            "Vedalken Dismisser cast, {5}{U} payment, or 2/2 characteristics differ".to_owned(),
        );
    }
    if capture.target_controller != Some(PlayerId(1))
        || capture.target_owner != Some(PlayerId(1))
        || capture.target_decision_player != Some(PlayerId(0))
        || !capture.trigger_stacked
        || capture.trigger_source_incarnation.is_none()
        || capture.trigger_source_incarnation != capture.trigger_resolved_incarnation
        || capture.target_zone_after != Some(Zone::Library)
        || capture.target_incarnation_before == capture.target_incarnation_after
        || !capture.target_is_library_top
    {
        return Err(format!(
            "Dismisser trigger did not select and move the exact opposing creature to its owner's library top: target={target:?}, owner={:?}, controller={:?}, decision_player={:?}, zone={:?}, before_inc={:?}, after_inc={:?}, top={}",
            capture.target_owner,
            capture.target_controller,
            capture.target_decision_player,
            capture.target_zone_after,
            capture.target_incarnation_before,
            capture.target_incarnation_after,
            capture.target_is_library_top
        ));
    }
    if capture.target_library_after.len() != capture.target_library_before.len() + 1
        || capture.target_library_after.last() != Some(&target)
    {
        return Err(
            "Dismisser target was not appended as the owner's exact library-top card".to_owned(),
        );
    }
    let finisher = capture
        .finisher
        .ok_or("pilot Watchwolf finisher was not cast")?;
    if capture.finisher_attack_turns.len() < 3
        || capture.finisher_damage < 9
        || capture.life[0] <= 0
        || capture.life[1] > 0
    {
        return Err("pilot Watchwolf did not continue through combat to a terminal win".to_owned());
    }
    let paid = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == source && colors.len() == 6 && colors.contains(&Color::Blue))).ok_or("Dismisser exact six-mana spell payment missing")?;
    let cast = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == source)).ok_or("Dismisser SpellCast receipt missing")?;
    let spell_resolved = unique_index(
        &capture.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == source),
    )
    .ok_or("Dismisser SpellResolved receipt missing")?;
    let entered = unique_index(&capture.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == source)).ok_or("Dismisser battlefield entry receipt missing")?;
    let opened = unique_index(&capture.events, |event| {
        matches!(
            event,
            GameEvent::DecisionOpened {
                player: PlayerId(0),
                kind: cardbench_magic_engine::DecisionKind::TriggeredAbilityTargets,
                ..
            }
        )
    })
    .ok_or("Dismisser controller target-decision receipt missing")?;
    let completed = unique_index(&capture.events, |event| {
        matches!(
            event,
            GameEvent::DecisionCompleted {
                player: PlayerId(0),
                kind: cardbench_magic_engine::DecisionKind::TriggeredAbilityTargets,
                ..
            }
        )
    })
    .ok_or("Dismisser target-decision completion receipt missing")?;
    let stacked = unique_index(&capture.events, |event| matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source: known, ability: TRIGGER_ID, .. } if *known == source)).ok_or("Dismisser TriggeredAbilityStacked receipt missing")?;
    let moved = unique_index(&capture.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Library } if *card == target)).ok_or("exact Dismisser target owner-library move receipt missing")?;
    let resolved = unique_index(&capture.events, |event| matches!(event, GameEvent::AbilityResolved { source: known, ability: TRIGGER_ID, .. } if *known == source)).ok_or("Dismisser AbilityResolved receipt missing")?;
    let wolf_cast = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == finisher)).ok_or("Watchwolf SpellCast receipt missing")?;
    let attack = event_index_after(&capture.events, resolved, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&finisher))).ok_or("Watchwolf did not attack after Dismisser resolution")?;
    let damage = event_index_after(&capture.events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source: known, player: PlayerId(1), amount } if *known == finisher && *amount > 0)).ok_or("Watchwolf dealt no combat damage after Dismisser resolution")?;
    let ended = unique_index(&capture.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal P0 GameEnded receipt missing")?;
    if !(paid < cast
        && cast < spell_resolved
        && spell_resolved < entered
        && entered < opened
        && opened < completed
        && completed < stacked
        && stacked < moved
        && moved < resolved
        && resolved < wolf_cast
        && wolf_cast < attack
        && attack < damage
        && damage < ended)
    {
        return Err(format!(
            "Dismisser spell/target/trigger/combat event order differs: spell=({paid},{cast},{spell_resolved},{entered}), trigger=({opened},{completed},{stacked},{moved},{resolved}), combat=({wolf_cast},{attack},{damage},{ended})"
        ));
    }
    let projected = project_events(&capture.events);
    let include: BTreeSet<_> = [
        paid,
        cast,
        spell_resolved,
        entered,
        opened,
        completed,
        stacked,
        moved,
        resolved,
        wolf_cast,
        attack,
        damage,
        ended,
    ]
    .into_iter()
    .collect();
    let events: Vec<_> = include
        .iter()
        .map(|index| serde_json::to_value(&projected[*index]).map_err(|error| error.to_string()))
        .collect::<Result<_, _>>()?;
    Ok(json!({
        "trace_id": "rav-vedalken-dismisser-etb-opposing-creature-owner-library-top-full-game",
        "seed": seed, "terminal": "Winner(PlayerId(0))", "winner_seat": 0,
        "turns": capture.turns, "accepted_moves": capture.accepted, "rejected_moves": capture.rejected,
        "dismisser": {"object": source.0, "collector_number": 73, "mana_cost": "{5}{U}", "mana_paid": capture.spell_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "power": capture.source_power, "toughness": capture.source_toughness, "resolved": capture.spell_resolved},
        "trigger": {"source_object": source.0, "ability_id": TRIGGER_ID, "mandatory": true, "controller": 0, "decision_player": capture.target_decision_player.map(|player| player.0), "source_incarnation": capture.trigger_source_incarnation, "resolved_source_incarnation": capture.trigger_resolved_incarnation, "target_object": target.0, "target_controller": capture.target_controller.map(|player| player.0), "target_owner": capture.target_owner.map(|player| player.0), "target_incarnation_before": capture.target_incarnation_before, "target_incarnation_after": capture.target_incarnation_after, "target_zone_after": format!("{:?}", capture.target_zone_after), "owner_library_size_before": capture.target_library_before.len(), "owner_library_size_after": capture.target_library_after.len(), "owner_library_top_object": capture.target_library_after.last().map(|object| object.0), "target_is_library_top": capture.target_is_library_top},
        "combat": {"card": "Watchwolf", "object": finisher.0, "attack_turns": capture.finisher_attack_turns, "damage_to_opponent": capture.finisher_damage, "life_at_terminal": capture.life},
        "ordered_event_sequence": [paid, cast, spell_resolved, entered, opened, completed, stacked, moved, resolved, wolf_cast, attack, damage, ended],
        "events": events,
        "passed": true
    }))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    Ok(run_match(seed, mutation)
        .and_then(|capture| validate_trace(&capture, seed).map(|_| capture))
        .is_err())
}

fn unique_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    let mut matches = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
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

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number": card.collector_number, "name": card.name, "definition_id": definition_id, "engine_status": match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" }, "trace_status": if passed { "passed" } else { "not_qualified" }, "qualification": if passed { "diagnostic_pilot_trace_passed; independent review pending" } else { "not_evaluated" }, "credited": false})
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

fn write_outputs(root: &std::path::Path, report: &Value) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).expect("report serializes"),
    )?;
    std::fs::write(root.join("reward.txt"), "0.0\n")
}
