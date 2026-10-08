//! Full-game RAV Bathe in Light authority for targeted Radiance protection.

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DeckEntry, DeckList, Effect, Game, GameEvent,
    GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step,
    Target, TargetRequirement, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_basic_land_type_bindings,
    rav_main_set_catalog, rav_mana_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-BATHE-IN-LIGHT";
const CARD_NAME: &str = "Bathe in Light";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const SNAPPING_DRAKE_ID: &str = "RAV-SNAPPING-DRAKE";
const GLASS_GOLEM_ID: &str = "RAV-GLASS-GOLEM";
const HELIX_ID: &str = "RAV-LIGHTNING-HELIX";
const FOREST_ID: &str = "RAV-FOREST";
const PLAINS_ID: &str = "RAV-PLAINS";
const ISLAND_ID: &str = "RAV-ISLAND";
const MOUNTAIN_ID: &str = "RAV-MOUNTAIN";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/bathe_in_light.rs";
const MAX_TRACE_SEED: u64 = 32;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/bathe_in_light.rs"]
mod candidate_bathe_in_light;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Bathe in Light PvP result: {error}");
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
            eprintln!("Bathe in Light PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_registry()?;
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<CatalogCard> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || catalog.len() != 291
        || printings.len() != 306
    {
        return Err("RAV catalog denominator differs from the source contract".to_owned());
    }
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.winner != replay.winner
        || captured.observation != replay.observation
        || captured.resolution != replay.resolution
        || captured.protected_target_rejected != replay.protected_target_rejected
        || captured.nonmatching_target_accepted != replay.nonmatching_target_accepted
    {
        return Err("Bathe in Light deterministic full-game replay differed".to_owned());
    }
    let mutations = json!({
        "wrong_exact_mana_cost_detected": mutation_detected(seed, Mutation::WrongCost),
        "wrong_card_color_detected": mutation_detected(seed, Mutation::WrongColor),
        "wrong_target_requirement_detected": mutation_detected(seed, Mutation::WrongTarget),
        "controller_only_protection_mutation_detected": mutation_detected(seed, Mutation::ControllerOnly),
        "sealed_mutation_authority": false
    });
    for key in [
        "wrong_exact_mana_cost_detected",
        "wrong_card_color_detected",
        "wrong_target_requirement_detected",
        "controller_only_protection_mutation_detected",
    ] {
        if mutations[key] != true {
            return Err(format!("Bathe in Light semantic mutation escaped: {key}"));
        }
    }
    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,
        "expansion":"RAV",
        "card":{"collector_number":2,"printings":1,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}", ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":[
                "105.2 and 202.1-202.2: white spell identity and chosen card color",
                "115.1 and 601.2: creature target selection and casting",
                "608.2b-608.2c: target legality and resolution",
                "609.1 and 611.2: continuous effects and end-of-turn duration",
                "702.16: protection and its targeting restrictions",
                "702.50: Radiance shared-color recipients across controllers",
                "510: combat damage and terminal game state"
            ],
            "source_contract":"RAV Bathe in Light #2, {1}{W} white instant: target creature and each other creature that shares a color with it gain protection from the chosen color until end of turn.",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true
        },
        "passed":cards_passed == 1,
        "pilot_card_passed":cards_passed == 1,
        "replay_deterministic":true,
        "cards_passed":cards_passed,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":printings.len(),
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":if candidate_kind() == "candidate" { "candidate_pilot_pass; independent reviewer approval remains" } else { "reference_pilot_pass; independent review and candidate trial remain" },
        "mutation_sensitivity":mutations,
        "traces":[trace],
        "per_card_breakdown":breakdown
    }))
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn output_root() -> Option<std::path::PathBuf> {
    std::env::args_os()
        .collect::<Vec<_>>()
        .windows(2)
        .find(|pair| pair[0] == "--output-root")
        .map(|pair| pair[1].clone().into())
}

fn write_outputs(root: &std::path::Path, report: &Value) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(root.join("reward.txt"), "0.00000000\n").map_err(|e| e.to_string())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongCost,
    WrongColor,
    WrongTarget,
    ControllerOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CreatureBefore {
    object: ObjectId,
    definition: Option<&'static str>,
    controller: PlayerId,
    colors: BTreeSet<Color>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BatheObservation {
    spell: ObjectId,
    target: ObjectId,
    chosen_color: Color,
    target_colors: BTreeSet<Color>,
    creatures: Vec<CreatureBefore>,
    expected_recipients: Vec<ObjectId>,
    excluded_creatures: Vec<ObjectId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CreatureAfter {
    object: ObjectId,
    controller: PlayerId,
    colors: BTreeSet<Color>,
    has_protection: bool,
    zone: Option<Zone>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BatheResolution {
    creatures: Vec<CreatureAfter>,
    created_recipients: Vec<ObjectId>,
}

#[derive(Clone, Debug)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    life: [i64; 2],
    bathe: Option<ObjectId>,
    bathe_definition: Option<CardDefinition>,
    bathe_zone: Option<Zone>,
    mana_paid: Vec<Color>,
    observation: Option<BatheObservation>,
    resolution: Option<BatheResolution>,
    protected_target_rejected: bool,
    rejection_atomic: bool,
    nonmatching_target_accepted: bool,
    helix: Option<ObjectId>,
    helix_target: Option<ObjectId>,
    watchwolf: Option<ObjectId>,
}

fn candidate_value(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_bathe_in_light::definition();
    match mutation {
        Mutation::None => {}
        Mutation::WrongCost => definition.mana_cost = ManaCost::with_colors(2, [Color::White]),
        Mutation::WrongColor => definition.colors = BTreeSet::from([Color::Blue]),
        Mutation::WrongTarget => definition.effects = vec![Effect::TargetedBundle {
            target: TargetRequirement::Player,
            effects: vec![Effect::RadianceAddChosenColorProtectionToTargetAndSharedColorCreaturesUntilEndOfTurn],
        }],
        Mutation::ControllerOnly => definition.effects = vec![Effect::TargetedBundle {
            target: TargetRequirement::Creature,
            effects: vec![Effect::AddChosenColorProtectionToControllerCreaturesUntilEndOfTurn],
        }],
    }
    definition
}

fn definition_matches_contract(definition: &CardDefinition) -> bool {
    definition.id == CARD_ID
        && definition.name == CARD_NAME
        && definition.set_code == "RAV"
        && definition.mana_cost == ManaCost::with_colors(1, [Color::White])
        && definition.colors == BTreeSet::from([Color::White])
        && definition.mana_colors.is_empty()
        && definition.card_types == BTreeSet::from([CardType::Instant])
        && !definition.is_basic_land
        && definition.supported_rules
            == [
                "full-rules-fidelity",
                "colored-cost-casting",
                "targeted-radiance-chosen-color-protection",
            ]
        && definition.power.is_none()
        && definition.toughness.is_none()
        && definition.keywords.is_empty()
        && definition.effects
            == [Effect::TargetedBundle {
                target: TargetRequirement::Creature,
                effects: vec![
            Effect::RadianceAddChosenColorProtectionToTargetAndSharedColorCreaturesUntilEndOfTurn,
        ],
            }]
}

fn validate_candidate_registry() -> Result<(), String> {
    let definition = candidate_bathe_in_light::definition();
    if !definition_matches_contract(&definition) {
        return Err("candidate Bathe in Light definition differs from its RAV identity".to_owned());
    }
    let registered = card_definitions()
        .into_iter()
        .find(|item| item.id == CARD_ID)
        .ok_or("Bathe in Light is missing from the RAV definition registry")?;
    if registered != definition {
        return Err(
            "Bathe in Light registry definition is not delegated to its candidate module"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|e| format!("could not read Bathe in Light source: {e}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Bathe in Light source is not valid Rust".to_owned())?;
    if !file.attrs.iter().all(|attr| attr.path().is_ident("doc")) || file.items.len() != 3 {
        return Err(
            "Bathe in Light module must contain private imports and one direct-literal function"
                .to_owned(),
        );
    }
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(use_item)
                if use_item.attrs.is_empty() && matches!(&use_item.vis, Visibility::Inherited) => {}
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !functions.insert(name.clone())
                    || name != "definition"
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !function.sig.inputs.is_empty()
                    || function.sig.constness.is_some()
                    || function.sig.asyncness.is_some()
                    || function.block.stmts.len() != 1
                    || !matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(_), None))
                {
                    return Err(
                        "Bathe in Light module contains unsupported executable source".to_owned(),
                    );
                }
            }
            _ => return Err(
                "Bathe in Light module permits only private imports and its declarative definition"
                    .to_owned(),
            ),
        }
    }
    if functions != BTreeSet::from(["definition".to_owned()]) {
        return Err("Bathe in Light candidate definition function is missing".to_owned());
    }
    Ok(())
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
        .ok_or("Bathe in Light definition missing during game construction")? =
        candidate_value(mutation);
    Game::new_with_mana_abilities_and_basic_land_types(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
    )
    .map_err(|e| e.to_string())
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_BATHE_IN_LIGHT_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_BATHE_IN_LIGHT_SEED must be an unsigned integer".to_owned())?;
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    let mut last_error = String::new();
    for seed in 0..=MAX_TRACE_SEED {
        match run_match(seed, Mutation::None)
            .and_then(|capture| validate_trace(&capture, seed).map(|_| capture))
        {
            Ok(captured) => return Ok((seed, captured)),
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "no bounded seed produced a full-game Bathe in Light Radiance trace: {last_error}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    run_match(seed, mutation)
        .and_then(|capture| validate_trace(&capture, seed).map(|_| ()))
        .is_err()
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut game = new_game(mutation)?;
    let p0 = deck(&[
        (CARD_ID, 4),
        (WATCHWOLF_ID, 8),
        (GLASS_GOLEM_ID, 4),
        (FOREST_ID, 22),
        (PLAINS_ID, 22),
    ]);
    let p1 = deck(&[
        (HELIX_ID, 16),
        (WATCHWOLF_ID, 8),
        (SNAPPING_DRAKE_ID, 8),
        (PLAINS_ID, 7),
        (FOREST_ID, 7),
        (ISLAND_ID, 7),
        (MOUNTAIN_ID, 7),
    ]);
    game.set_shuffle_seed(seed).map_err(|e| e.to_string())?;
    for (player, list) in [(PlayerId(0), &p0), (PlayerId(1), &p1)] {
        if list
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
        {
            return Err("Bathe in Light PvP decks must each contain exactly 60 cards".to_owned());
        }
        game.load_deck_into_library(player, list)
            .map_err(|e| e.to_string())?;
        game.draw_opening_hand(player, 7)
            .map_err(|e| e.to_string())?;
    }
    game.begin_game().map_err(|e| e.to_string())?;
    let mut policies = [BathePolicy::new(PlayerId(0)), BathePolicy::new(PlayerId(1))];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut observation: Option<BatheObservation> = None;
    let mut bathe_payment: Option<Vec<Color>> = None;
    let mut resolution: Option<BatheResolution> = None;
    let mut protected_target_rejected = false;
    let mut rejection_atomic = false;
    let mut nonmatching_target_accepted = false;
    let mut helix_target = None;
    while !game.is_game_over() {
        if game.turn > 50 || accepted >= 7_500 {
            return Err("Bathe in Light full-game trace exceeded its bound".to_owned());
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Bathe in Light policy seat missing")?;
        let mut action = if let Some(action) = policy.propose_pending_decision(&view) {
            action
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
        let new_observation = match &action {
            PolicyAction::CastWithColorChoice {
                request,
                color: Color::Red,
            } if game
                .card_definition(request.card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                let target = request
                    .targets
                    .iter()
                    .find_map(|target| match target {
                        Target::Permanent(id) => Some(*id),
                        _ => None,
                    })
                    .ok_or("Bathe in Light cast action omitted its creature target")?;
                Some(observation_from_view(
                    &view,
                    request.card,
                    target,
                    Color::Red,
                )?)
            }
            _ => None,
        };
        let mana_before_bathe = if new_observation.is_some() {
            Some(
                game.player(player)
                    .map_err(|e| e.to_string())?
                    .mana_pool
                    .clone(),
            )
        } else {
            None
        };
        if let PolicyAction::CastWithPayment { request, .. } = &action
            && player == PlayerId(1)
            && game
                .card_definition(request.card)
                .is_ok_and(|definition| definition.id == HELIX_ID)
            && !protected_target_rejected
        {
            let protected = request
                .targets
                .iter()
                .find_map(|target| match target {
                    Target::Permanent(id) => Some(*id),
                    _ => None,
                })
                .ok_or("Lightning Helix protection challenge omitted its protected target")?;
            let nonmatching = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(SNAPPING_DRAKE_ID))
                .map(|card| card.id)
                .ok_or(
                    "opponent has no nonmatching-color creature for the legal target challenge",
                )?;
            let before_events = game.event_log.clone();
            let before_stack = game.stack.clone();
            let before_hand = game.player(player).map_err(|e| e.to_string())?.hand.clone();
            let before_pool = game
                .player(player)
                .map_err(|e| e.to_string())?
                .mana_pool
                .clone();
            let invalid_request = request.clone();
            match game.cast_spell(player, invalid_request) {
                Err(cardbench_magic_engine::RulesError::IllegalTarget(Target::Permanent(
                    target,
                ))) if target == protected => {}
                Err(error) => {
                    return Err(format!(
                        "Lightning Helix protected target was rejected for an unexpected reason: {error}"
                    ));
                }
                Ok(()) => {
                    return Err("Protection from red did not reject Lightning Helix targeting the cross-controller Radiance recipient".to_owned());
                }
            }
            rejection_atomic = game.event_log == before_events
                && game.stack == before_stack
                && game
                    .player(player)
                    .is_ok_and(|state| state.hand == before_hand && state.mana_pool == before_pool);
            if !rejection_atomic {
                return Err(
                    "rejected protected target mutated Bathe/Lightning Helix game state".to_owned(),
                );
            }
            let mut corrected = action.clone();
            if let PolicyAction::CastWithPayment { request, .. } = &mut corrected {
                request.targets = vec![Target::Permanent(nonmatching)];
            }
            action = corrected;
            protected_target_rejected = true;
            nonmatching_target_accepted = true;
            helix_target = Some(nonmatching);
        }
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|e| {
                format!(
                    "Bathe in Light policy move rejected on turn {} {:?}: {e}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        if let (Some(obs), Some(before)) = (new_observation, mana_before_bathe) {
            let after = game
                .player(player)
                .map_err(|e| e.to_string())?
                .mana_pool
                .clone();
            let mut spent = Vec::new();
            for color in Color::MANA_ALL {
                let before_amount = before.amount(color);
                let after_amount = after.amount(color);
                if after_amount > before_amount {
                    return Err("Bathe in Light cast increased the mana pool".to_owned());
                }
                for _ in 0..before_amount - after_amount {
                    spent.push(color);
                }
            }
            bathe_payment = Some(spent);
            observation = Some(obs);
        }
        if resolution.is_none()
            && let Some(obs) = &observation
            && game.event_log.iter().any(
                |event| matches!(event, GameEvent::SpellResolved { card } if *card == obs.spell),
            )
        {
            resolution = Some(snapshot_resolution(&game, obs)?);
        }
        game.validate_invariants()
            .map_err(|e| format!("post-move invariant failure: {e}"))?;
    }
    let events = game.event_log.clone();
    let find_cast = |seat: PlayerId, card_id: &str| {
        events.iter().find_map(|event| match event {
            GameEvent::SpellCast { player, card }
                if *player == seat
                    && game
                        .card_definition(*card)
                        .is_ok_and(|definition| definition.id == card_id) =>
            {
                Some(*card)
            }
            _ => None,
        })
    };
    let bathe = find_cast(PlayerId(0), CARD_ID);
    let bathe_definition = bathe.and_then(|card| game.card_definition(card).ok().cloned());
    let helix = find_cast(PlayerId(1), HELIX_ID);
    let watchwolf = find_cast(PlayerId(0), WATCHWOLF_ID);
    let mana_paid = bathe_payment.unwrap_or_default();
    let end_of_game = events
        .iter()
        .position(|event| matches!(event, GameEvent::GameEnded { .. }))
        .unwrap_or(events.len());
    let bathe_zone = bathe.and_then(|card| {
        events[..end_of_game]
            .iter()
            .rev()
            .find_map(|event| match event {
                GameEvent::CardMoved {
                    card: moved, to, ..
                } if *moved == card => Some(*to),
                _ => None,
            })
    });
    Ok(Captured {
        events,
        winner: game.winner().map(|winner| winner.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        life: [
            game.player(PlayerId(0)).map_err(|e| e.to_string())?.life,
            game.player(PlayerId(1)).map_err(|e| e.to_string())?.life,
        ],
        bathe,
        bathe_definition,
        bathe_zone,
        mana_paid,
        observation,
        resolution,
        protected_target_rejected,
        rejection_atomic,
        nonmatching_target_accepted,
        helix,
        helix_target,
        watchwolf,
    })
}

fn observation_from_view(
    view: &GameView,
    spell: ObjectId,
    target: ObjectId,
    chosen_color: Color,
) -> Result<BatheObservation, String> {
    let creature_views = view
        .own_battlefield
        .iter()
        .chain(&view.opponent_battlefield)
        .filter(|card| card.card_types.contains(&CardType::Creature))
        .collect::<Vec<_>>();
    let target_view = creature_views
        .iter()
        .find(|card| card.id == target)
        .ok_or("Bathe target was not a creature in the battlefield snapshot")?;
    let target_colors = target_view.colors.clone();
    let creatures: Vec<CreatureBefore> = creature_views
        .iter()
        .map(|card| CreatureBefore {
            object: card.id,
            definition: card.definition,
            controller: card.controller,
            colors: card.colors.clone(),
        })
        .collect();
    let expected_recipients: Vec<_> = creatures
        .iter()
        .filter(|card| card.object == target || !card.colors.is_disjoint(&target_colors))
        .map(|card| card.object)
        .collect();
    let excluded_creatures: Vec<_> = creatures
        .iter()
        .filter(|card| card.object != target && card.colors.is_disjoint(&target_colors))
        .map(|card| card.object)
        .collect();
    Ok(BatheObservation {
        spell,
        target,
        chosen_color,
        target_colors,
        creatures,
        expected_recipients,
        excluded_creatures,
    })
}

fn snapshot_resolution(
    game: &Game,
    observation: &BatheObservation,
) -> Result<BatheResolution, String> {
    let creatures = observation
        .creatures
        .iter()
        .map(|before| {
            let characteristics = game
                .characteristics(before.object)
                .map_err(|e| e.to_string())?;
            Ok(CreatureAfter {
                object: before.object,
                controller: game
                    .controller_of(before.object)
                    .map_err(|e| e.to_string())?,
                colors: characteristics.colors,
                has_protection: characteristics
                    .keywords
                    .contains(&Keyword::Protection(observation.chosen_color)),
                zone: game.zone_of(before.object),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let created_recipients = game
        .event_log
        .iter()
        .filter_map(|event| match event {
            GameEvent::ContinuousEffectCreated { source, target, .. }
                if *source == observation.spell =>
            {
                Some(*target)
            }
            _ => None,
        })
        .collect();
    Ok(BatheResolution {
        creatures,
        created_recipients,
    })
}

struct BathePolicy {
    player: PlayerId,
    bathe_offered: bool,
    helix_challenge_offered: bool,
}
impl BathePolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            bathe_offered: false,
            helix_challenge_offered: false,
        }
    }
}

impl CodePolicy for BathePolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.bathe-in-light-targeted-radiance.v1"
        } else {
            "rav.radiance-target-legality-opponent.v1"
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
            let attackers = if self.player == PlayerId(0) && self.bathe_offered && view.turn >= 12 {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(WATCHWOLF_ID) && card.can_attack)
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
        if self.player == PlayerId(1)
            && view.priority == self.player
            && view.stack_depth == 0
            && !self.helix_challenge_offered
            && has_red_protection(view, WATCHWOLF_ID)
            && let Some(helix) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(HELIX_ID))
        {
            let cost = ManaCost::with_colors(0, [Color::Red, Color::White]);
            if let Some(mana_selection) = payment(view, &cost) {
                let protected = view.own_battlefield.iter().find(|card| {
                    card.definition == Some(WATCHWOLF_ID)
                        && card.keywords.contains(&Keyword::Protection(Color::Red))
                });
                if let Some(protected) = protected {
                    self.helix_challenge_offered = true;
                    return PolicyAction::CastWithPayment {
                        request: CastRequest {
                            card: helix.id,
                            targets: vec![Target::Permanent(protected.id)],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection,
                    };
                }
            }
            if let Some(action) = make_mana(view, &cost) {
                return action;
            }
        }
        if view.priority != self.player
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if self.player == PlayerId(1) {
            if !has_permanent(view, WATCHWOLF_ID)
                && let Some(action) = cast_permanent(
                    view,
                    WATCHWOLF_ID,
                    &ManaCost::with_colors(0, [Color::Green, Color::White]),
                )
            {
                return action;
            }
            if !has_permanent(view, SNAPPING_DRAKE_ID)
                && let Some(action) = cast_permanent(
                    view,
                    SNAPPING_DRAKE_ID,
                    &ManaCost::with_colors(3, [Color::Blue]),
                )
            {
                return action;
            }
            if has_red_protection(view, WATCHWOLF_ID)
                && let Some(protected) = view.own_battlefield.iter().find(|card| {
                    card.definition == Some(WATCHWOLF_ID)
                        && card.keywords.contains(&Keyword::Protection(Color::Red))
                })
                && view.own_battlefield.iter().any(|card| {
                    card.definition == Some(SNAPPING_DRAKE_ID)
                        && !card.keywords.contains(&Keyword::Protection(Color::Red))
                })
                && let Some(helix) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(HELIX_ID))
            {
                let cost = ManaCost::with_colors(0, [Color::Red, Color::White]);
                if let Some(mana_selection) = payment(view, &cost) {
                    return PolicyAction::CastWithPayment {
                        request: CastRequest {
                            card: helix.id,
                            targets: vec![Target::Permanent(protected.id)],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection,
                    };
                }
                if let Some(action) = make_mana(view, &cost) {
                    return action;
                }
            }
        } else {
            if !has_permanent(view, WATCHWOLF_ID)
                && let Some(action) = cast_permanent(
                    view,
                    WATCHWOLF_ID,
                    &ManaCost::with_colors(0, [Color::Green, Color::White]),
                )
            {
                return action;
            }
            if !has_permanent(view, GLASS_GOLEM_ID)
                && let Some(action) = cast_permanent(view, GLASS_GOLEM_ID, &ManaCost::new(5))
            {
                return action;
            }
            if !self.bathe_offered
                && view.turn >= 4
                && has_permanent(view, WATCHWOLF_ID)
                && has_permanent(view, GLASS_GOLEM_ID)
                && view
                    .opponent_battlefield
                    .iter()
                    .any(|card| card.definition == Some(WATCHWOLF_ID))
                && view
                    .opponent_battlefield
                    .iter()
                    .any(|card| card.definition == Some(SNAPPING_DRAKE_ID))
            {
                if let Some(spell) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
                {
                    let cost = ManaCost::with_colors(1, [Color::White]);
                    if payment(view, &cost).is_some() {
                        let Some(target) = view
                            .opponent_battlefield
                            .iter()
                            .find(|card| card.definition == Some(WATCHWOLF_ID))
                        else {
                            return PolicyAction::PassPriority;
                        };
                        self.bathe_offered = true;
                        return PolicyAction::CastWithColorChoice {
                            request: CastRequest {
                                card: spell.id,
                                targets: vec![Target::Permanent(target.id)],
                                convoke: vec![],
                                payment_mana_abilities: vec![],
                            },
                            color: Color::Red,
                        };
                    }
                    if let Some(action) = make_mana(view, &cost) {
                        return action;
                    }
                }
            }
        }
        if view.lands_played == 0 {
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        PolicyAction::PassPriority
    }
}

fn has_permanent(view: &GameView, id: &str) -> bool {
    view.own_battlefield
        .iter()
        .any(|card| card.definition == Some(id))
}
fn has_red_protection(view: &GameView, id: &str) -> bool {
    view.own_battlefield.iter().any(|card| {
        card.definition == Some(id) && card.keywords.contains(&Keyword::Protection(Color::Red))
    })
}

fn cast_permanent(view: &GameView, id: &str, cost: &ManaCost) -> Option<PolicyAction> {
    if let Some(card) = view.hand.iter().find(|card| card.definition == Some(id)) {
        if let Some(mana_selection) = payment(view, cost) {
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
        return make_mana(view, cost);
    }
    None
}

fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<_> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let (_, amount) = available
            .iter_mut()
            .find(|(candidate, _)| candidate == color)?;
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
    let mut need = None;
    for color in &cost.colored {
        let required = cost
            .colored
            .iter()
            .filter(|candidate| *candidate == color)
            .count() as u8;
        if view.mana_pool.amount(*color) < required {
            need = Some(*color);
            break;
        }
    }
    if need.is_none()
        && view.mana_pool.total() < cost.generic.saturating_add(cost.colored.len() as u8)
    {
        need = Some(Color::Blue);
    }
    let preferred = need?;
    let land = view
        .own_battlefield
        .iter()
        .find(|card| {
            card.card_types.contains(&CardType::Land)
                && !card.tapped
                && card.mana_colors.contains(&preferred)
        })
        .or_else(|| {
            view.own_battlefield.iter().find(|card| {
                card.card_types.contains(&CardType::Land)
                    && !card.tapped
                    && !card.mana_colors.is_empty()
            })
        })?;
    let color = if land.mana_colors.contains(&preferred) {
        preferred
    } else {
        *land.mana_colors.iter().next()?
    };
    Some(PolicyAction::ActivateManaAbility {
        land: land.id,
        color,
    })
}

fn validate_trace(c: &Captured, seed: u64) -> Result<Value, String> {
    if c.winner != Some(0) || c.rejected != 0 || c.life[0] <= 0 || c.life[1] > 0 {
        return Err(format!(
            "Bathe in Light match did not reach a clean seat-zero win: {:?} {:?}",
            c.winner, c.life
        ));
    }
    let spell = c.bathe.ok_or("seat zero did not cast Bathe in Light")?;
    let definition = c
        .bathe_definition
        .as_ref()
        .ok_or("Bathe in Light live definition missing")?;
    if !definition_matches_contract(definition) {
        return Err("live Bathe in Light definition differs from its RAV identity".to_owned());
    }
    if c.bathe_zone != Some(Zone::Graveyard) {
        return Err("resolved Bathe in Light did not move to its owner's graveyard".to_owned());
    }
    if c.mana_paid.len() != 2 || !c.mana_paid.contains(&Color::White) {
        return Err(format!(
            "Bathe in Light did not pay actual {{1}}{{W}}: {:?}",
            c.mana_paid
        ));
    }
    let observation = c
        .observation
        .as_ref()
        .ok_or("Bathe in Light cast-time target/color snapshot missing")?;
    let resolution = c
        .resolution
        .as_ref()
        .ok_or("Bathe in Light resolution-time creature snapshot missing")?;
    if observation.spell != spell || observation.chosen_color != Color::Red {
        return Err("Bathe in Light target or chosen-color provenance differs".to_owned());
    }
    let target = observation
        .creatures
        .iter()
        .find(|creature| creature.object == observation.target)
        .ok_or("Bathe target missing from cast-time creature snapshot")?;
    if target.definition != Some(WATCHWOLF_ID)
        || target.controller != PlayerId(1)
        || !target.colors.contains(&Color::Green)
        || !target.colors.contains(&Color::White)
    {
        return Err(
            "Bathe in Light did not target the intended opposing Green/White creature".to_owned(),
        );
    }
    let expected: BTreeSet<_> = observation.expected_recipients.iter().copied().collect();
    let created: BTreeSet<_> = resolution.created_recipients.iter().copied().collect();
    if expected.len() < 2
        || expected != created
        || resolution.created_recipients.len() != created.len()
    {
        return Err(format!(
            "Radiance recipients differ from the exact cross-controller shared-color set: expected={expected:?}, created={created:?}"
        ));
    }
    if !observation.expected_recipients.iter().any(|id| {
        observation
            .creatures
            .iter()
            .any(|creature| creature.object == *id && creature.controller == PlayerId(0))
    }) || !observation.expected_recipients.iter().any(|id| {
        observation
            .creatures
            .iter()
            .any(|creature| creature.object == *id && creature.controller == PlayerId(1))
    }) {
        return Err(
            "Radiance did not affect matching creatures controlled by both players".to_owned(),
        );
    }
    let excluded_definitions: BTreeSet<_> = observation
        .creatures
        .iter()
        .filter(|creature| observation.excluded_creatures.contains(&creature.object))
        .filter_map(|creature| creature.definition)
        .collect();
    if !excluded_definitions.contains(GLASS_GOLEM_ID)
        || !excluded_definitions.contains(SNAPPING_DRAKE_ID)
    {
        return Err(format!(
            "Radiance trace omitted colorless or cross-controller nonmatching-color exclusions: {excluded_definitions:?}"
        ));
    }
    for creature in &observation.creatures {
        let expected_protection = expected.contains(&creature.object);
        let actual = resolution
            .creatures
            .iter()
            .find(|after| after.object == creature.object)
            .ok_or("post-resolution creature snapshot omitted a battlefield creature")?;
        if actual.zone != Some(Zone::Battlefield)
            || actual.controller != creature.controller
            || actual.has_protection != expected_protection
        {
            return Err(format!(
                "wrong chosen-color protection result for {:?}: expected={expected_protection}, actual={actual:?}",
                creature.definition
            ));
        }
    }
    if !c.protected_target_rejected || !c.rejection_atomic || !c.nonmatching_target_accepted {
        return Err("protection did not reject its matching-color target atomically while allowing the nonmatching blue target".to_owned());
    }
    let helix = c.helix.ok_or(
        "opponent did not cast Lightning Helix for the protection target legality challenge",
    )?;
    let snap = c
        .helix_target
        .ok_or("Lightning Helix nonmatching-color target was not captured")?;
    let wolf = c.watchwolf.ok_or("seat zero did not cast Watchwolf")?;
    let events = &c.events;
    if c.mana_paid.len() != 2 || !c.mana_paid.contains(&Color::White) {
        return Err(
            "Bathe in Light mana-pool delta does not prove actual {1}{W} payment".to_owned(),
        );
    }
    let color = event(events, |event| matches!(event, GameEvent::SpellColorChosen { player: PlayerId(0), card, color: Color::Red } if *card == spell)).ok_or("chosen Red color was not retained as a policy-submitted cast choice")?;
    let cast = after(events, color, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == spell)).ok_or("Bathe in Light SpellCast receipt missing")?;
    let resolved = after(
        events,
        cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == spell),
    )
    .ok_or("Bathe in Light did not resolve")?;
    let graveyard = after(events, resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == spell)).ok_or("Bathe in Light did not move to its graveyard")?;
    let helix_cast = event(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == helix)).ok_or("Lightning Helix SpellCast receipt missing")?;
    let helix_damage = after(events, helix_cast, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 3 } if *source == helix && *permanent == snap)).ok_or("red Lightning Helix did not successfully target/deal three damage to the nonmatching blue Drake")?;
    let drake_died = after(events, helix_damage, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == snap)).ok_or("nonmatching Snapping Drake did not move to graveyard after Lightning Helix")?;
    let wolf_cast = event(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == wolf)).ok_or("Watchwolf SpellCast receipt missing")?;
    let wolf_resolved = after(
        events,
        wolf_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == wolf),
    )
    .ok_or("Watchwolf did not resolve")?;
    let wolf_entered = after(events, wolf_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == wolf)).ok_or("Watchwolf did not enter battlefield")?;
    let attack = after(events, wolf_entered, |event| {
        matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&wolf))
    }).ok_or("Watchwolf did not attack")?;
    let damage = after(events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 3 } if *source == wolf)).ok_or("Watchwolf did not deal combat damage")?;
    let terminal = after(events, damage, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal seat-zero win receipt missing")?;
    let total_damage: i64 = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if *source == wolf => Some(i64::from(*amount)),
            _ => None,
        })
        .sum();
    if total_damage < 20
        || !(color < cast
            && cast < resolved
            && resolved < graveyard
            && graveyard < helix_cast
            && helix_cast < helix_damage
            && helix_damage < drake_died
            && wolf_cast < wolf_resolved
            && wolf_resolved < wolf_entered
            && wolf_entered < attack
            && attack < damage
            && damage < terminal)
    {
        return Err(
            "Bathe, Radiance, protection challenge, or terminal combat receipts were out of order"
                .to_owned(),
        );
    }
    let mut selected = BTreeSet::from([
        color,
        cast,
        resolved,
        graveyard,
        helix_cast,
        helix_damage,
        drake_died,
        wolf_cast,
        wolf_resolved,
        wolf_entered,
        attack,
        damage,
        terminal,
    ]);
    selected.extend(events.iter().enumerate().filter_map(|(index, event)| {
        matches!(event, GameEvent::ContinuousEffectCreated { source, .. } if *source == spell)
            .then_some(index)
    }));
    selected.extend(events.iter().enumerate().filter_map(|(index, event)| {
        matches!(event, GameEvent::ContinuousEffectExpired { source, .. } if *source == spell)
            .then_some(index)
    }));
    let projected: Vec<ProjectedEvent> = project_events(events)
        .into_iter()
        .filter(|event| selected.contains(&(event.sequence as usize)))
        .collect();
    Ok(json!({
        "trace_id":"rav-bathe-in-light-targeted-radiance-cross-controller-full-game",
        "shuffle_seed":seed,
        "decks":["rav_bathe_watchwolf_golem_60","rav_helix_watchwolf_drake_60"],
        "policies":["rav.bathe-in-light-targeted-radiance.v1","rav.radiance-target-legality-opponent.v1"],
        "terminal":"Winner(PlayerId(0))","winner_seat":0,"turns":c.turns,"accepted_moves":c.accepted,"rejected_moves":c.rejected,
        "bathe_in_light":{"object":spell.0,"collector_number":2,"mana_cost":"{1}{W}","mana_paid":c.mana_paid.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"mana_payment_source":"before_after_mana_pool_delta","chosen_color":"Red","target_object":observation.target.0,"target_controller":target.controller.0,"target_colors":target.colors.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"zone_after_resolution":"Graveyard"},
        "radiance":{"expected_recipient_objects":expected.iter().map(|id|id.0).collect::<Vec<_>>(),"continuous_effect_recipient_objects":created.iter().map(|id|id.0).collect::<Vec<_>>(),"excluded_nonmatching_creatures":observation.creatures.iter().filter(|creature|observation.excluded_creatures.contains(&creature.object)).map(|creature|json!({"object":creature.object.0,"definition":creature.definition,"controller":creature.controller.0,"colors":creature.colors.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>()})).collect::<Vec<_>>(),"creature_results":resolution.creatures.iter().map(|creature|json!({"object":creature.object.0,"controller":creature.controller.0,"colors":creature.colors.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"has_protection_from_chosen_red":creature.has_protection,"zone":format!("{:?}",creature.zone)})).collect::<Vec<_>>(),"matching_recipients_cross_both_controllers":true,"cross_controller_nonmatching_blue_excluded":true,"colorless_creature_excluded":true},
        "protection_target_legality":{"protected_watchwolf_target_rejected":c.protected_target_rejected,"rejection_was_atomic":c.rejection_atomic,"nonmatching_blue_drake_target_accepted":c.nonmatching_target_accepted,"challenge_spell_object":helix.0,"legal_target":snap.0},
        "combat":{"watchwolf":wolf.0,"total_damage_to_opponent":total_damage,"terminal_life":c.life},
        "events":projected,"passed":true
    }))
}

fn event(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn after(
    events: &[GameEvent],
    index: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(index + 1)
        .find_map(|(i, event)| predicate(event).then_some(i))
}

fn card_status(card: &CatalogCard) -> Value {
    let id = card_definitions()
        .into_iter()
        .find(|definition| definition.name == card.name)
        .map(|definition| definition.id)
        .unwrap_or("");
    let engine_status = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice",
        CardSemanticStatus::CatalogOnly { .. } => "catalog_only",
    };
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,"engine_status":engine_status,"trace_status":if id==CARD_ID{"passed"}else{"not_qualified"},"qualification":if id==CARD_ID{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
}
