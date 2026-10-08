//! Full-game diagnostic authority for RAV Terraformer's chosen-land-type ability.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, BasicLandType, CardDefinition,
    CardType, CastRequest, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, Layer,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target,
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
const CARD_ID: &str = "RAV-TERRAFORMER";
const MOSS_DOG_ID: &str = "RAV-GREATER-MOSSDOG";
const ISLAND_ID: &str = "RAV-ISLAND";
const OPPONENT_ISLAND_ID: &str = "RAV-ISLAND";
const OPPONENT_PLAINS_ID: &str = "RAV-PLAINS";
const ABILITY_ID: &str = "choose-controller-land-basic-type";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/terraformer.rs";
const MAX_TURNS: u32 = 60;
const MAX_MOVES: u32 = 6_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/terraformer.rs"]
mod candidate_terraformer;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Terraformer result: {error}");
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
            eprintln!("Terraformer PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoAbility,
    WrongAbilityCost,
    TapSource,
    SorcerySpeed,
    WrongEffect,
    WrongBody,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LandState {
    object: ObjectId,
    definition: Option<&'static str>,
    land_type: Option<BasicLandType>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Activation {
    turn: u32,
    active_player: PlayerId,
    step: Step,
    choice: BasicLandType,
    source: ObjectId,
    source_incarnation: Option<u64>,
    resolved_incarnation: Option<u64>,
    cost: Option<ManaCost>,
    activated: bool,
    resolved: bool,
    controller_before: Vec<LandState>,
    opponent_before: Vec<LandState>,
    effect_targets: Vec<ObjectId>,
    expiry_targets: Vec<ObjectId>,
    controller_after: Vec<LandState>,
    opponent_after: Vec<LandState>,
    controller_restored: Vec<LandState>,
    opponent_restored: Vec<LandState>,
    green_from_converted_island: Option<ObjectId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    terraformer: Option<ObjectId>,
    terraformer_incarnation: Option<u64>,
    terraformer_payment: Vec<Color>,
    terraformer_power_toughness: Option<(i32, i32)>,
    activations: Vec<Activation>,
    mossdog: Option<ObjectId>,
    mossdog_payment: Vec<Color>,
    mossdog_resolved: bool,
    mossdog_attacks: Vec<u32>,
    mossdog_damage: i32,
    terraformer_attacks: Vec<u32>,
    terraformer_damage: i32,
    life: [i64; 2],
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_registry()?;
    let seed = std::env::var("CARDBENCH_TERRAFORMER_SEED")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(12);
    let capture = run_match(seed, Mutation::None)?;
    let trace = validate_trace(&capture, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if capture != replay || project_events(&capture.events) != project_events(&replay.events) {
        return Err("Terraformer shuffled duel replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "ability_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::NoAbility),
        "generic_activation_cost_changed_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongAbilityCost),
        "source_tap_cost_added_mutation_detected_in_game": mutation_detected(seed, Mutation::TapSource),
        "instant_speed_activation_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::SorcerySpeed),
        "chosen_land_type_effect_changed_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongEffect),
        "printed_body_changed_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongBody),
        "sealed_mutation_authority": false
    });
    for key in [
        "ability_removed_mutation_detected_in_game",
        "generic_activation_cost_changed_mutation_detected_in_game",
        "source_tap_cost_added_mutation_detected_in_game",
        "instant_speed_activation_removed_mutation_detected_in_game",
        "chosen_land_type_effect_changed_mutation_detected_in_game",
        "printed_body_changed_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!("Terraformer gameplay mutant escaped: {key}"));
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
        "schema_version": "cardbench.magic.card-pvp.check.v1", "task_id": TASK_ID, "expansion": "RAV",
        "card": {"collector_number": 70, "printings": 1, "definition_id": CARD_ID, "name": "Terraformer"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID}, "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {"comprehensive_rules": "2026-08-07", "checked_sections": ["117: instant-speed priority", "302.1 and 601.2: creature spells and casting", "602: activated abilities", "613.1 and 305.6: type-changing effects and basic land types", "605: intrinsic basic-land mana abilities", "514: cleanup and end-of-turn expiration"], "source_contract": "RAV Terraformer #70 is a blue 2/2 Creature costing {2}{U}; {1}: each land you control becomes a basic land type of your choice until end of turn.", "authority_cutoff": "2026-09-24", "no_provider_trial": true},
        "passed": true, "pilot_card_passed": true, "deterministic_replay": true,
        "cards_passed": 1, "cards_credited": 0, "cards_total": catalog.len(), "printings_total": printings.len(),
        "coverage_complete": false, "full_reward": false, "harbor_reward": 0.0, "pilot_reward": 0.0,
        "qualification_status": "diagnostic_only; independent review and candidate trial pending",
        "mutation_sensitivity": mutations, "traces": [trace], "per_card_breakdown": breakdown
    }))
}

fn candidate_definition(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_terraformer::definition();
    if mutation == Mutation::WrongBody {
        definition.power = Some(1);
    }
    definition
}

fn candidate_ability(mutation: Mutation) -> ActivatedAbilityBinding {
    let mut binding = candidate_terraformer::activated_ability();
    match mutation {
        Mutation::WrongAbilityCost => binding.ability.mana_cost = ManaCost::new(2),
        Mutation::TapSource => binding.ability.tap_cost = true,
        Mutation::SorcerySpeed => binding.ability.sorcery_speed = true,
        Mutation::WrongEffect => {
            binding.ability.effects =
                vec![Effect::ReplaceControllerLandsBasicLandTypeUntilEndOfTurn {
                    land_type: BasicLandType::Mountain,
                }]
        }
        _ => {}
    }
    binding
}

fn validate_candidate_registry() -> Result<(), String> {
    let definition = candidate_terraformer::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Terraformer definition is missing from the RAV registry")?;
    if registered != definition || definition != expected_definition() {
        return Err(
            "Terraformer candidate definition differs from the RAV source contract".to_owned(),
        );
    }
    let binding = candidate_terraformer::activated_ability();
    let registered = rav_activated_ability_bindings()
        .into_iter()
        .find(|ability| ability.card_definition == CARD_ID)
        .ok_or("Terraformer activation is missing from the RAV registry")?;
    if binding != registered || binding != expected_ability() {
        return Err(
            "Terraformer candidate activation differs from the RAV source contract".to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Terraformer source: {error}"))?;
    let file =
        syn::parse_file(&source).map_err(|_| "Terraformer source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
        || file.items.len() != 4
    {
        return Err(
            "Terraformer module must contain safe private imports and its two direct card literals"
                .to_owned(),
        );
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
                    return Err("Terraformer contains an unsupported import".to_owned());
                }
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                let return_type = if name == "definition" {
                    "CardDefinition"
                } else if name == "activated_ability" {
                    "ActivatedAbilityBinding"
                } else {
                    return Err(format!(
                        "Terraformer source has unsupported function `{name}`"
                    ));
                };
                let signature_ok = !functions.contains(&name)
                    && function.attrs.iter().all(|attribute| {
                        attribute.path().is_ident("must_use") || attribute.path().is_ident("doc")
                    })
                    && matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    && function.sig.inputs.is_empty()
                    && function.sig.asyncness.is_none()
                    && function.sig.unsafety.is_none()
                    && function.sig.generics.params.is_empty()
                    && matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if matches!(&**ty, syn::Type::Path(path) if path.path.is_ident(return_type)));
                let body_ok = function.block.stmts.len() == 1
                    && matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(value), None) if value.rest.is_none() && value.fields.iter().all(|field| safe_expression(&field.expr)));
                if !signature_ok || !body_ok {
                    return Err(format!(
                        "Terraformer function `{name}` is not a direct declarative card literal"
                    ));
                }
                functions.insert(name);
            }
            _ => {
                return Err(
                    "Terraformer module contains executable or unsupported top-level code"
                        .to_owned(),
                );
            }
        }
    }
    if imports != 2
        || functions != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()])
    {
        return Err(
            "Terraformer imports or function identity differs from the source contract".to_owned(),
        );
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
                | "ActivatedAbility"
                | "ActivatedAbilityBinding"
                | "CardDefinition"
                | "CardType"
                | "Color"
                | "Effect"
                | "ManaCost"
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
            [
                vec!["None".to_owned()],
                vec!["Color".to_owned(), "Blue".to_owned()],
                vec!["CardType".to_owned(), "Creature".to_owned()],
                vec![
                    "Effect".to_owned(),
                    "ReplaceControllerLandsWithChosenBasicLandTypeUntilEndOfTurn".to_owned(),
                ],
            ]
            .contains(&segments)
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
            ([
                vec!["ManaCost".to_owned(), "with_colors".to_owned()],
                vec!["ManaCost".to_owned(), "new".to_owned()],
                vec!["BTreeSet".to_owned(), "from".to_owned()],
                vec!["BTreeSet".to_owned(), "new".to_owned()],
                vec!["Some".to_owned()],
            ]
            .contains(&segments))
                && call.args.iter().all(safe_expression)
        }
        Expr::Struct(value) => {
            let segments: Vec<_> = value
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            ([
                vec!["CardDefinition".to_owned()],
                vec!["ActivatedAbilityBinding".to_owned()],
                vec!["ActivatedAbility".to_owned()],
            ]
            .contains(&segments))
                && value.rest.is_none()
                && value
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

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: "Terraformer",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "controller-lands-chosen-basic-land-type-until-end-of-turn",
        ],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

fn expected_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: CARD_ID,
        ability: ActivatedAbility {
            id: ABILITY_ID,
            mana_cost: ManaCost::new(1),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![],
            effects: vec![Effect::ReplaceControllerLandsWithChosenBasicLandTypeUntilEndOfTurn],
        },
    }
}

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Terraformer definition is missing from the RAV registry")? =
        candidate_definition(mutation);
    let mut activated = rav_activated_ability_bindings();
    let index = activated
        .iter()
        .position(|binding| binding.card_definition == CARD_ID)
        .ok_or("Terraformer activation is missing from the RAV registry")?;
    if mutation == Mutation::NoAbility {
        activated.remove(index);
    } else {
        activated[index] = candidate_ability(mutation);
    }
    Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        activated,
        rav_triggered_ability_bindings(),
    )
    .map_err(|error| error.to_string())
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

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot = deck(&[(CARD_ID, 4), (MOSS_DOG_ID, 4), (ISLAND_ID, 52)]);
    let opponent = deck(&[(OPPONENT_ISLAND_ID, 30), (OPPONENT_PLAINS_ID, 30)]);
    for (name, list) in [("pilot", &pilot), ("opponent", &opponent)] {
        if list
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
        {
            return Err(format!("Terraformer {name} deck is not 60 cards"));
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
    let mut policies = [
        TerraformerPolicy::new(PlayerId(0), mutation),
        TerraformerPolicy::new(PlayerId(1), Mutation::None),
    ];
    let mut capture = Captured {
        events: vec![],
        winner: None,
        turns: 0,
        accepted: 0,
        rejected: 0,
        terraformer: None,
        terraformer_incarnation: None,
        terraformer_payment: vec![],
        terraformer_power_toughness: None,
        activations: vec![],
        mossdog: None,
        mossdog_payment: vec![],
        mossdog_resolved: false,
        mossdog_attacks: vec![],
        mossdog_damage: 0,
        terraformer_attacks: vec![],
        terraformer_damage: 0,
        life: [20, 20],
    };
    while !game.is_game_over() {
        if game.turn > config.max_turns || capture.accepted >= config.max_policy_moves {
            return Err(format!(
                "Terraformer duel exceeded bounds at turn {} (Terra={:?}, activations={:?}, Mossdog={:?}, life={:?})",
                game.turn,
                capture.terraformer,
                capture
                    .activations
                    .iter()
                    .map(|activation| (
                        activation.turn,
                        activation.active_player,
                        activation.step,
                        activation.choice
                    ))
                    .collect::<Vec<_>>(),
                capture.mossdog,
                [
                    game.player(PlayerId(0))
                        .map_err(|error| error.to_string())?
                        .life,
                    game.player(PlayerId(1))
                        .map_err(|error| error.to_string())?
                        .life
                ]
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Terraformer policy seat missing")?;
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
            if game
                .card_definition(request.card)
                .is_ok_and(|definition| definition.id == CARD_ID)
            {
                capture.terraformer.get_or_insert(request.card);
            }
            if game
                .card_definition(request.card)
                .is_ok_and(|definition| definition.id == MOSS_DOG_ID)
            {
                capture.mossdog.get_or_insert(request.card);
            }
        }
        if let PolicyAction::ActivateAbility { activation } = &action
            && activation.ability_id == ABILITY_ID
        {
            let source = activation.source;
            let controller_before = view
                .own_battlefield
                .iter()
                .filter(|card| card.card_types.contains(&CardType::Land))
                .map(|card| land_state(&game, card.id))
                .collect::<Result<Vec<_>, _>>()?;
            let opponent_before = view
                .opponent_battlefield
                .iter()
                .filter(|card| card.card_types.contains(&CardType::Land))
                .map(|card| land_state(&game, card.id))
                .collect::<Result<Vec<_>, _>>()?;
            let choice = activation
                .targets
                .iter()
                .find_map(|target| {
                    if let Target::BasicLandType(kind) = target {
                        Some(*kind)
                    } else {
                        None
                    }
                })
                .unwrap_or(BasicLandType::Forest);
            capture.activations.push(Activation {
                turn: game.turn,
                active_player: game.active_player,
                step: view.step,
                choice,
                source,
                source_incarnation: game.object(source).ok().map(|object| object.incarnation),
                resolved_incarnation: None,
                cost: None,
                activated: false,
                resolved: false,
                controller_before,
                opponent_before,
                effect_targets: vec![],
                expiry_targets: vec![],
                controller_after: vec![],
                opponent_after: vec![],
                controller_restored: vec![],
                opponent_restored: vec![],
                green_from_converted_island: None,
            });
        }
        let cursor = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Terraformer move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        capture.accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        observe(&game, &game.event_log[cursor..], &mut capture)?;
        policies[0].first_activation_resolved = capture
            .activations
            .first()
            .is_some_and(|activation| activation.resolved);
        policies[0].second_activation_resolved = capture
            .activations
            .get(1)
            .is_some_and(|activation| activation.resolved);
    }
    capture.events = game.event_log.clone();
    capture.winner = game.winner().map(|player| player.0);
    capture.turns = game.turn;
    capture.life = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    Ok(capture)
}

fn land_state(game: &Game, object: ObjectId) -> Result<LandState, String> {
    Ok(LandState {
        object,
        definition: game
            .card_definition(object)
            .ok()
            .map(|definition| definition.id),
        land_type: game
            .basic_land_type(object)
            .map_err(|error| error.to_string())?,
    })
}

fn observe(game: &Game, events: &[GameEvent], capture: &mut Captured) -> Result<(), String> {
    for event in events {
        match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if Some(*card) == capture.terraformer => capture.terraformer_payment = colors.clone(),
            GameEvent::SpellResolved { card } if Some(*card) == capture.terraformer => {
                let characteristics = game
                    .characteristics(*card)
                    .map_err(|error| error.to_string())?;
                capture.terraformer_power_toughness = Some((
                    characteristics.power.ok_or("Terraformer power missing")?,
                    characteristics
                        .toughness
                        .ok_or("Terraformer toughness missing")?,
                ));
            }
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if Some(*card) == capture.mossdog => capture.mossdog_payment = colors.clone(),
            GameEvent::SpellResolved { card } if Some(*card) == capture.mossdog => {
                capture.mossdog_resolved = true
            }
            GameEvent::AbilityManaPaid {
                player: PlayerId(0),
                source,
                ability: ABILITY_ID,
                mana_cost,
            } if Some(*source) == capture.terraformer => {
                if let Some(activation) =
                    capture.activations.iter_mut().rev().find(|activation| {
                        activation.turn == game.turn && activation.source == *source
                    })
                {
                    activation.cost = Some(mana_cost.clone());
                }
            }
            GameEvent::AbilityActivated {
                player: PlayerId(0),
                source,
                source_incarnation,
                definition: CARD_ID,
                ability: ABILITY_ID,
            } if Some(*source) == capture.terraformer => {
                if let Some(activation) =
                    capture.activations.iter_mut().rev().find(|activation| {
                        activation.turn == game.turn && activation.source == *source
                    })
                {
                    activation.activated = true;
                    activation.source_incarnation = Some(*source_incarnation);
                }
                capture.terraformer_incarnation = Some(*source_incarnation);
            }
            GameEvent::AbilityResolved {
                source,
                source_incarnation,
                ability: ABILITY_ID,
            } if Some(*source) == capture.terraformer => {
                if let Some(activation) =
                    capture.activations.iter_mut().rev().find(|activation| {
                        activation.turn == game.turn && activation.source == *source
                    })
                {
                    activation.resolved = true;
                    activation.resolved_incarnation = Some(*source_incarnation);
                    activation.controller_after = activation
                        .controller_before
                        .iter()
                        .map(|land| land_state(game, land.object))
                        .collect::<Result<Vec<_>, _>>()?;
                    activation.opponent_after = activation
                        .opponent_before
                        .iter()
                        .map(|land| land_state(game, land.object))
                        .collect::<Result<Vec<_>, _>>()?;
                }
            }
            GameEvent::ContinuousEffectCreated {
                source,
                target,
                layer: Layer::Type,
            } if Some(*source) == capture.terraformer => {
                if let Some(activation) =
                    capture.activations.iter_mut().rev().find(|activation| {
                        activation.turn == game.turn && activation.source == *source
                    })
                {
                    activation.effect_targets.push(*target);
                }
            }
            GameEvent::ContinuousEffectExpired {
                source,
                target,
                layer: Layer::Type,
            } if Some(*source) == capture.terraformer => {
                if let Some(activation) = capture.activations.iter_mut().rev().find(|activation| {
                    activation.source == *source
                        && activation.effect_targets.contains(target)
                        && !activation.expiry_targets.contains(target)
                }) {
                    activation.expiry_targets.push(*target);
                }
            }
            GameEvent::ManaAbilityActivated {
                player: PlayerId(0),
                land,
                color: Color::Green,
            } => {
                if let Some(activation) = capture.activations.iter_mut().rev().find(|activation| {
                    activation.turn == game.turn && activation.effect_targets.contains(land)
                }) {
                    if activation
                        .controller_before
                        .iter()
                        .any(|known| known.object == *land && known.definition == Some(ISLAND_ID))
                    {
                        activation.green_from_converted_island.get_or_insert(*land);
                    }
                }
            }
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } => {
                for attacker in attackers {
                    match game
                        .card_definition(*attacker)
                        .ok()
                        .map(|definition| definition.id)
                    {
                        Some(CARD_ID) => capture.terraformer_attacks.push(game.turn),
                        Some(MOSS_DOG_ID) => capture.mossdog_attacks.push(game.turn),
                        _ => {}
                    }
                }
            }
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if Some(*source) == capture.terraformer => capture.terraformer_damage += *amount,
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if Some(*source) == capture.mossdog => capture.mossdog_damage += *amount,
            _ => {}
        }
    }
    for activation in &mut capture.activations {
        if !activation.effect_targets.is_empty()
            && activation.expiry_targets.len() == activation.effect_targets.len()
            && activation.controller_restored.is_empty()
        {
            activation.controller_restored = activation
                .controller_before
                .iter()
                .map(|land| land_state(game, land.object))
                .collect::<Result<Vec<_>, _>>()?;
            activation.opponent_restored = activation
                .opponent_before
                .iter()
                .map(|land| land_state(game, land.object))
                .collect::<Result<Vec<_>, _>>()?;
        }
    }
    Ok(())
}

struct TerraformerPolicy {
    player: PlayerId,
    mutation: Mutation,
    terraformer_cast: bool,
    first_activation: bool,
    second_activation: bool,
    first_activation_resolved: bool,
    second_activation_resolved: bool,
    green_from_transformed_land: bool,
    second_green_from_transformed_land: bool,
    mossdog_cast: bool,
}

impl TerraformerPolicy {
    fn new(player: PlayerId, mutation: Mutation) -> Self {
        Self {
            player,
            mutation,
            terraformer_cast: false,
            first_activation: false,
            second_activation: false,
            first_activation_resolved: false,
            second_activation_resolved: false,
            green_from_transformed_land: false,
            second_green_from_transformed_land: false,
            mossdog_cast: false,
        }
    }
    fn tap_land(view: &GameView, color: Color, prefer_island: bool) -> Option<PolicyAction> {
        let land = view
            .own_battlefield
            .iter()
            .filter(|card| {
                card.card_types.contains(&CardType::Land)
                    && !card.tapped
                    && card.mana_colors.contains(&color)
            })
            .min_by_key(|card| {
                if prefer_island && card.definition == Some(ISLAND_ID) {
                    0
                } else {
                    1
                }
            })?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }
    fn ensure_cost(view: &GameView, cost: &ManaCost, prefer_island: bool) -> Option<PolicyAction> {
        for color in &cost.colored {
            if view.mana_pool.amount(*color)
                < cost.colored.iter().filter(|same| *same == color).count() as u8
            {
                return Self::tap_land(view, *color, prefer_island);
            }
        }
        if view.mana_pool.total() < cost.mana_value() {
            return Self::tap_land(view, Color::Green, prefer_island)
                .or_else(|| Self::tap_land(view, Color::Blue, prefer_island));
        }
        None
    }
    fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let mut available: Vec<_> = Color::MANA_ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        for color in &cost.colored {
            let amount = &mut available.iter_mut().find(|(known, _)| known == color)?.1;
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
    fn activate_terraformer(&mut self, view: &GameView, source: ObjectId) -> Option<PolicyAction> {
        let cost = candidate_ability(self.mutation).ability.mana_cost;
        if view.mana_pool.total() < cost.mana_value() {
            return Self::tap_land(view, Color::Blue, false)
                .or_else(|| Self::tap_land(view, Color::Green, false));
        }
        Some(PolicyAction::ActivateAbility {
            activation: AbilityActivation {
                source,
                ability_id: ABILITY_ID,
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![Target::BasicLandType(BasicLandType::Forest)],
            },
        })
    }
    fn cast_creature(&mut self, view: &GameView) -> Option<PolicyAction> {
        let cost = ManaCost::with_colors(3, [Color::Green]);
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(MOSS_DOG_ID))?;
        // GameView reports printed Island mana colors even while the live type
        // layer has changed those same permanents to Forests. Use the known
        // resolution flag to request Green from a former Island for every
        // colored and generic pip; the engine validates the live type layer.
        if view.mana_pool.amount(Color::Green) < 1 || view.mana_pool.total() < cost.mana_value() {
            let island = view
                .own_battlefield
                .iter()
                .find(|candidate| candidate.definition == Some(ISLAND_ID) && !candidate.tapped)?;
            return Some(PolicyAction::ActivateManaAbility {
                land: island.id,
                color: Color::Green,
            });
        }
        let mana_selection = Self::payment(view, &cost)?;
        self.mossdog_cast = true;
        Some(PolicyAction::CastWithPayment {
            request: CastRequest {
                card: card.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection,
        })
    }
}

impl CodePolicy for TerraformerPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.terraformer-controller-land-type.v1"
        } else {
            "rav.terraformer-patient-opponent.v1"
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
            // Hold the 2/2 Terraformer back so this fixture cannot end before the
            // colored-mana conversion and Greater Mossdog branch are exercised.
            let attackers = view
                .own_battlefield
                .iter()
                .filter(|card| card.can_attack && card.definition == Some(MOSS_DOG_ID))
                .map(|card| card.id)
                .collect();
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
        if view.priority != self.player || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }
        if view.active_player == self.player && view.step == Step::PrecombatMain {
            if view.lands_played == 0
                && let Some(land) = view
                    .hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if self.player == PlayerId(0) && view.step == Step::PrecombatMain {
            if view.active_player == PlayerId(0) && !self.terraformer_cast {
                let land_count = view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.card_types.contains(&CardType::Land))
                    .count();
                if land_count >= 5
                    && let Some(card) = view
                        .hand
                        .iter()
                        .find(|card| card.definition == Some(CARD_ID))
                {
                    let cost = candidate_definition(self.mutation).mana_cost;
                    if let Some(action) = Self::ensure_cost(view, &cost, false) {
                        return action;
                    }
                    if let Some(mana_selection) = Self::payment(view, &cost) {
                        self.terraformer_cast = true;
                        return PolicyAction::CastWithPayment {
                            request: CastRequest {
                                card: card.id,
                                targets: vec![],
                                convoke: vec![],
                                payment_mana_abilities: vec![],
                            },
                            chosen_x: None,
                            mana_selection,
                        };
                    }
                }
            }
            let source = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID) && !card.tapped)
                .map(|card| card.id);
            if let Some(source) = source {
                if view.active_player == PlayerId(1) && !self.first_activation {
                    if let Some(action) = self.activate_terraformer(view, source) {
                        if matches!(action, PolicyAction::ActivateAbility { .. }) {
                            self.first_activation = true;
                        }
                        return action;
                    }
                }
                if view.active_player == PlayerId(1)
                    && self.first_activation_resolved
                    && !self.green_from_transformed_land
                {
                    if let Some(island) = view
                        .own_battlefield
                        .iter()
                        .find(|card| card.definition == Some(ISLAND_ID) && !card.tapped)
                    {
                        self.green_from_transformed_land = true;
                        return PolicyAction::ActivateManaAbility {
                            land: island.id,
                            color: Color::Green,
                        };
                    }
                }
                if view.active_player == PlayerId(0)
                    && self.second_activation_resolved
                    && !self.second_green_from_transformed_land
                {
                    if let Some(island) = view
                        .own_battlefield
                        .iter()
                        .find(|card| card.definition == Some(ISLAND_ID) && !card.tapped)
                    {
                        self.second_green_from_transformed_land = true;
                        return PolicyAction::ActivateManaAbility {
                            land: island.id,
                            color: Color::Green,
                        };
                    }
                }
                if view.active_player == PlayerId(0)
                    && !self.second_activation
                    && self.first_activation
                    && view
                        .hand
                        .iter()
                        .any(|card| card.definition == Some(MOSS_DOG_ID))
                {
                    if let Some(action) = self.activate_terraformer(view, source) {
                        if matches!(action, PolicyAction::ActivateAbility { .. }) {
                            self.second_activation = true;
                        }
                        return action;
                    }
                }
                if view.active_player == PlayerId(0) && self.second_activation && !self.mossdog_cast
                {
                    if self.second_green_from_transformed_land
                        && let Some(action) = self.cast_creature(view)
                    {
                        return action;
                    }
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(capture: &Captured, seed: u64) -> Result<Value, String> {
    if capture.winner != Some(0) || capture.rejected != 0 {
        return Err(format!(
            "Terraformer duel did not reach a clean P0 win: winner={:?}, turns={}, life={:?}, activations={}, Terra attacks={}, Mossdog attacks={}",
            capture.winner,
            capture.turns,
            capture.life,
            capture.activations.len(),
            capture.terraformer_attacks.len(),
            capture.mossdog_attacks.len()
        ));
    }
    let terraformer = capture.terraformer.ok_or("Terraformer was not cast")?;
    let mossdog = capture.mossdog.ok_or_else(|| format!("Greater Mossdog was not cast (turns={}, winner={:?}, activations={}, Terra={:?}, life={:?}, Terra damage={}, attacks={:?})", capture.turns, capture.winner, capture.activations.len(), capture.terraformer, capture.life, capture.terraformer_damage, capture.terraformer_attacks))?;
    if capture.terraformer_incarnation.is_none()
        || capture
            .terraformer_payment
            .iter()
            .filter(|color| **color == Color::Blue)
            .count()
            < 1
        || capture.terraformer_payment.len() != 3
        || capture.terraformer_power_toughness != Some((2, 2))
        || !capture.mossdog_resolved
        || capture.mossdog_payment.len() != 4
        || capture
            .mossdog_payment
            .iter()
            .filter(|color| **color == Color::Green)
            .count()
            < 1
        || capture.terraformer_damage + capture.mossdog_damage < 20
        || capture.terraformer_attacks.len() + capture.mossdog_attacks.len() < 5
        || capture.life[0] <= 0
        || capture.life[1] > 0
    {
        return Err(format!(
            "Terraformer/G.Mossdog spell, body, mana payment, combat, or terminal state differs: Terra pay={:?} PT={:?}; Mossdog={mossdog:?} pay={:?} resolved={} damage={} attacks={}; Terra damage={} attacks={}; life={:?}, winner={:?}, turns={}",
            capture.terraformer_payment,
            capture.terraformer_power_toughness,
            capture.mossdog_payment,
            capture.mossdog_resolved,
            capture.mossdog_damage,
            capture.mossdog_attacks.len(),
            capture.terraformer_damage,
            capture.terraformer_attacks.len(),
            capture.life,
            capture.winner,
            capture.turns
        ));
    }
    if capture.activations.len() != 2 {
        return Err(format!(
            "expected two resolved Terraformer activations, observed {}",
            capture.activations.len()
        ));
    }
    for (index, activation) in capture.activations.iter().enumerate() {
        if activation.source != terraformer
            || activation.source_incarnation.is_none()
            || activation.source_incarnation != activation.resolved_incarnation
            || !activation.activated
            || !activation.resolved
            || activation.choice != BasicLandType::Forest
            || activation.cost.as_ref() != Some(&ManaCost::new(1))
        {
            return Err(format!(
                "Terraformer activation {index} source, mana cost, choice, or resolution identity differs"
            ));
        }
        let mut controlled_ids: Vec<_> = activation
            .controller_before
            .iter()
            .map(|land| land.object)
            .collect();
        let mut effect_ids = activation.effect_targets.clone();
        controlled_ids.sort();
        effect_ids.sort();
        if controlled_ids.is_empty()
            || controlled_ids != effect_ids
            || activation
                .controller_before
                .iter()
                .any(|land| land.land_type != Some(BasicLandType::Island))
            || activation
                .controller_after
                .iter()
                .any(|land| land.land_type != Some(BasicLandType::Forest))
            || activation.opponent_before != activation.opponent_after
            || activation.opponent_before.iter().any(|land| {
                land.land_type != Some(BasicLandType::Island)
                    && land.land_type != Some(BasicLandType::Plains)
            })
        {
            return Err(format!(
                "Terraformer activation {index} did not change every controller land to Forest while preserving opponent lands"
            ));
        }
        let mut expiry_ids = activation.expiry_targets.clone();
        expiry_ids.sort();
        if expiry_ids != controlled_ids
            || activation.controller_restored != activation.controller_before
            || activation.opponent_restored != activation.opponent_before
        {
            return Err(format!(
                "Terraformer activation {index} failed to expire at end of turn and restore original land types: controlled={controlled_ids:?} expired={expiry_ids:?} before={:?} restored={:?} opponent_before={:?} opponent_restored={:?}",
                activation.controller_before,
                activation.controller_restored,
                activation.opponent_before,
                activation.opponent_restored
            ));
        }
        if activation.green_from_converted_island.is_none() {
            return Err(format!(
                "Terraformer activation {index} did not produce intrinsic Green from a former Island"
            ));
        }
    }
    if capture.activations[0].active_player != PlayerId(1)
        || capture.activations[0].step != Step::PrecombatMain
        || capture.activations[1].active_player != PlayerId(0)
        || capture.activations[1].step != Step::PrecombatMain
        || capture.activations[0].turn >= capture.activations[1].turn
    {
        return Err("Terraformer did not demonstrate instant-speed opponent-turn activation followed by a main-phase activation".to_owned());
    }

    let source = terraformer;
    let paid: Vec<_> = capture.events.iter().enumerate().filter_map(|(index, event)| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source: known, ability: ABILITY_ID, mana_cost } if *known == source && *mana_cost == ManaCost::new(1)).then_some(index)).collect();
    let activated: Vec<_> = capture.events.iter().enumerate().filter_map(|(index, event)| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source: known, definition: CARD_ID, ability: ABILITY_ID, .. } if *known == source).then_some(index)).collect();
    let resolved: Vec<_> = capture.events.iter().enumerate().filter_map(|(index, event)| matches!(event, GameEvent::AbilityResolved { source: known, ability: ABILITY_ID, .. } if *known == source).then_some(index)).collect();
    if paid.len() != 2 || activated.len() != 2 || resolved.len() != 2 {
        return Err(
            "Terraformer exact ability payment/activation/resolution receipts are missing"
                .to_owned(),
        );
    }

    let mut selected = Vec::new();
    let mut activation_json = Vec::new();
    for (ordinal, activation) in capture.activations.iter().enumerate() {
        let created: Vec<_> = capture.events.iter().enumerate().filter_map(|(index, event)| matches!(event, GameEvent::ContinuousEffectCreated { source: known, target, layer: Layer::Type } if *known == source && activation.effect_targets.contains(target) && index > activated[ordinal] && index < resolved[ordinal]).then_some(index)).collect();
        let expired: Vec<_> = capture.events.iter().enumerate().filter_map(|(index, event)| matches!(event, GameEvent::ContinuousEffectExpired { source: known, target, layer: Layer::Type } if *known == source && activation.expiry_targets.contains(target) && index > resolved[ordinal] && (ordinal == 1 || index < activated[ordinal + 1])).then_some(index)).collect();
        if created.len() != activation.effect_targets.len()
            || expired.len() != activation.expiry_targets.len()
        {
            return Err(format!(
                "Terraformer activation {ordinal} lacks exact per-land type-effect or expiry receipts"
            ));
        }
        let green = capture.events.iter().enumerate().find_map(|(index, event)| matches!(event, GameEvent::ManaAbilityActivated { player: PlayerId(0), land, color: Color::Green } if Some(*land) == activation.green_from_converted_island && index > resolved[ordinal] && (ordinal == 1 || index < activated[ordinal + 1])).then_some(index)).ok_or("converted Island Green mana activation receipt missing")?;
        let mana_cost = paid[ordinal];
        selected.extend([mana_cost, activated[ordinal]]);
        selected.extend(created);
        selected.push(resolved[ordinal]);
        selected.push(green);
        selected.extend(expired);
        activation_json.push(json!({
            "turn": activation.turn, "active_player": activation.active_player.0,
            "step": format!("{:?}", activation.step), "ability_id": ABILITY_ID,
            "source": activation.source.0, "source_incarnation": activation.source_incarnation,
            "resolved_source_incarnation": activation.resolved_incarnation,
            "mana_cost": "{1}", "chosen_type": format!("{:?}", activation.choice),
            "controller_lands_before": activation.controller_before.iter().map(land_json).collect::<Vec<_>>(),
            "controller_lands_after": activation.controller_after.iter().map(land_json).collect::<Vec<_>>(),
            "opponent_lands_before": activation.opponent_before.iter().map(land_json).collect::<Vec<_>>(),
            "opponent_lands_after": activation.opponent_after.iter().map(land_json).collect::<Vec<_>>(),
            "effect_targets": activation.effect_targets.iter().map(|object| object.0).collect::<Vec<_>>(),
            "expiry_targets": activation.expiry_targets.iter().map(|object| object.0).collect::<Vec<_>>(),
            "controller_lands_restored": activation.controller_restored.iter().map(land_json).collect::<Vec<_>>(),
            "opponent_lands_restored": activation.opponent_restored.iter().map(land_json).collect::<Vec<_>>(),
            "former_island_that_made_green": activation.green_from_converted_island.map(|object| object.0)
        }));
    }
    let terraformer_paid = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == source && colors == &capture.terraformer_payment)).ok_or("Terraformer spell payment receipt missing")?;
    let terraformer_cast = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == source)).ok_or("Terraformer cast receipt missing")?;
    let terraformer_resolved = unique_index(
        &capture.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == source),
    )
    .ok_or("Terraformer spell resolution receipt missing")?;
    let mossdog_paid = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == mossdog && colors == &capture.mossdog_payment)).ok_or("Greater Mossdog spell payment receipt missing")?;
    let mossdog_cast = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == mossdog)).ok_or("Greater Mossdog cast receipt missing")?;
    let mossdog_resolved = unique_index(
        &capture.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == mossdog),
    )
    .ok_or("Greater Mossdog resolution receipt missing")?;
    let end = unique_index(&capture.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal P0 GameEnded receipt missing")?;
    selected.extend([
        terraformer_paid,
        terraformer_cast,
        terraformer_resolved,
        mossdog_paid,
        mossdog_cast,
        mossdog_resolved,
        end,
    ]);
    selected.sort_unstable();
    selected.dedup();
    for pair in [
        terraformer_paid,
        terraformer_cast,
        terraformer_resolved,
        paid[0],
        activated[0],
        resolved[0],
        activated[1],
        resolved[1],
        mossdog_paid,
        mossdog_cast,
        mossdog_resolved,
        end,
    ]
    .windows(2)
    {
        if pair[0] >= pair[1] {
            return Err("Terraformer casting, activation, expiration, converted-mana, Mossdog, and terminal event order differs".to_owned());
        }
    }
    let projected = project_events(&capture.events);
    let events: Vec<_> = selected
        .iter()
        .map(|index| serde_json::to_value(&projected[*index]).map_err(|error| error.to_string()))
        .collect::<Result<_, _>>()?;
    let ordered_event_sequence: Vec<_> = events
        .iter()
        .filter_map(|event| event.get("sequence").and_then(Value::as_u64))
        .collect();
    Ok(json!({
        "trace_id": "rav-terraformer-chosen-basic-type-intrinsic-mana-full-game", "seed": seed,
        "terminal": "Winner(PlayerId(0))", "winner_seat": 0, "turns": capture.turns,
        "accepted_moves": capture.accepted, "rejected_moves": capture.rejected,
        "terraformer": {"collector_number": 70, "object": source.0, "mana_cost": "{2}{U}", "mana_paid": capture.terraformer_payment.iter().map(|color| format!("{:?}", color)).collect::<Vec<_>>(), "power": capture.terraformer_power_toughness.map(|pt| pt.0), "toughness": capture.terraformer_power_toughness.map(|pt| pt.1), "activated_ability_id": ABILITY_ID, "activation_count": capture.activations.len()},
        "activations": activation_json,
        "intrinsic_mana_proof": {"source_land_definition": ISLAND_ID, "chosen_color": "Green", "during_each_activation": true},
        "followup_creature": {"definition": MOSS_DOG_ID, "object": mossdog.0, "mana_cost": "{3}{G}", "mana_paid": capture.mossdog_payment.iter().map(|color| format!("{:?}", color)).collect::<Vec<_>>(), "resolved": capture.mossdog_resolved},
        "combat": {"attacks_by_terraformer": capture.terraformer_attacks, "terraformer_damage": capture.terraformer_damage, "attacks_by_mossdog": capture.mossdog_attacks, "mossdog_damage": capture.mossdog_damage, "damage_to_opponent": capture.terraformer_damage + capture.mossdog_damage, "life_at_terminal": capture.life},
        "ordered_event_sequence": ordered_event_sequence, "events": events, "passed": true
    }))
}

fn land_json(land: &LandState) -> Value {
    json!({"object": land.object.0, "definition": land.definition, "basic_land_type": land.land_type.map(|kind| format!("{:?}", kind))})
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation) {
        Ok(capture) => validate_trace(&capture, seed).is_err(),
        Err(_) => true,
    }
}

fn unique_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    let mut found = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let first = found.next()?;
    found.next().is_none().then_some(first)
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number": card.collector_number, "name": card.name, "definition_id": definition_id, "engine_status": match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" }, "trace_status": if passed { "passed" } else { "not_qualified" }, "qualification": if passed { "diagnostic_pilot_trace_passed; independent review pending" } else { "not_evaluated" }, "credited": false})
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "agent"
    } else {
        "reference"
    }
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
