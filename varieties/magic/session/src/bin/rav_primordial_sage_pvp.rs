//! Isolated full-game RAV #177 Primordial Sage authority.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DeckEntry, DeckList, Effect, Game, GameEvent,
    GameView, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step,
    TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
};
use cardbench_magic_policies::{
    CatalogPolicyProfile, CodePolicy, DeckMatchConfig, RavCatalogPolicy,
};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-PRIMORDIAL-SAGE";
const CARD_NAME: &str = "Primordial Sage";
const WOLF_ID: &str = "RAV-WATCHWOLF";
const ABILITY_ID: &str = "controller-creature-spell-cast-may-draw";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/primordial_sage.rs";
const MAX_TURNS: u32 = 80;
const MAX_MOVES: u32 = 8_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/primordial_sage.rs"]
mod candidate_primordial_sage;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Primordial Sage PvP result: {error}");
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
            eprintln!("Primordial Sage PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongManaCost,
    WrongPower,
    WrongToughness,
    RemoveTrigger,
    RemoveDraw,
    MandatoryTrigger,
    WrongTriggerCondition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ChoiceMode {
    AcceptAll,
    DeclineAll,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ChoiceObservation {
    source: ObjectId,
    accepted: bool,
    library_top_before: Option<ObjectId>,
    library_top_after: Option<ObjectId>,
    hand_size_before: usize,
    hand_size_after: usize,
    moved_top_to_hand: bool,
    draw_move_index: Option<usize>,
    action_event_index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    life: [i64; 2],
    sage: Option<ObjectId>,
    sage_cast_index: Option<usize>,
    sage_resolved_index: Option<usize>,
    sage_payment: Vec<Color>,
    sage_characteristics_valid: bool,
    controller_creature_casts_after_sage: Vec<(ObjectId, usize)>,
    opponent_creature_casts_after_sage: Vec<(ObjectId, usize)>,
    trigger_stacks: Vec<(ObjectId, usize)>,
    trigger_resolutions: Vec<(ObjectId, usize)>,
    choices: Vec<ChoiceObservation>,
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate()?;
    let catalog = unique_catalog()?;
    let (seed, accepted, declined) = find_traces()?;
    let accepted_trace = validate_trace(&accepted, seed, ChoiceMode::AcceptAll)?;
    let declined_trace = validate_trace(&declined, seed, ChoiceMode::DeclineAll)?;
    for (capture, mode) in [
        (&accepted, ChoiceMode::AcceptAll),
        (&declined, ChoiceMode::DeclineAll),
    ] {
        let replay = run_match(seed, Mutation::None, mode)?;
        if capture != &replay || project_events(&capture.events) != project_events(&replay.events) {
            return Err("Primordial Sage shuffled PvP replay was not deterministic".to_owned());
        }
    }
    let mutations = json!({
        "wrong_mana_cost_detected": mutation_detected(seed, Mutation::WrongManaCost, ChoiceMode::AcceptAll)?,
        "wrong_power_detected": mutation_detected(seed, Mutation::WrongPower, ChoiceMode::AcceptAll)?,
        "wrong_toughness_detected": mutation_detected(seed, Mutation::WrongToughness, ChoiceMode::AcceptAll)?,
        "trigger_removal_detected": mutation_detected(seed, Mutation::RemoveTrigger, ChoiceMode::AcceptAll)?,
        "draw_effect_removal_detected": mutation_detected(seed, Mutation::RemoveDraw, ChoiceMode::AcceptAll)?,
        "mandatory_trigger_detected": mutation_detected(seed, Mutation::MandatoryTrigger, ChoiceMode::AcceptAll)?,
        "wrong_trigger_condition_detected": mutation_detected(seed, Mutation::WrongTriggerCondition, ChoiceMode::AcceptAll)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "wrong_mana_cost_detected",
        "wrong_power_detected",
        "wrong_toughness_detected",
        "trigger_removal_detected",
        "draw_effect_removal_detected",
        "mandatory_trigger_detected",
        "wrong_trigger_condition_detected",
    ] {
        if mutations[key] != true {
            return Err(format!("Primordial Sage semantic mutation escaped: {key}"));
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
        "card": {"collector_number": 177, "printings": 1, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "601.2: casting a creature spell",
                "603.2: triggered ability event conditions",
                "603.3: putting triggered abilities on the stack",
                "603.5: optional triggered abilities",
                "608.2: resolving a triggered ability",
                "120.2: drawing cards"
            ],
            "source_contract": "RAV #177 is a green {4}{G}{G} 4/5 Spirit; its controller's creature casts create a controller-owned optional draw branch that resolves before the cast creature spell.",
            "authority_cutoff": "2026-09-24",
            "no_provider_trial": true
        },
        "passed": cards_passed == 1,
        "pilot_card_passed": cards_passed == 1,
        "replay_deterministic": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "diagnostic_only; external qualification pending",
        "candidate_source_validation": {
            "normalized_literal_source_checked_before_candidate_calls": true,
            "direct_ast_checked_before_candidate_calls": true
        },
        "mutation_sensitivity": mutations,
        "traces": [accepted_trace, declined_trace],
        "per_card_breakdown": breakdown
    }))
}

fn candidate_definition(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_primordial_sage::definition();
    match mutation {
        Mutation::None
        | Mutation::RemoveTrigger
        | Mutation::RemoveDraw
        | Mutation::MandatoryTrigger
        | Mutation::WrongTriggerCondition => {}
        Mutation::WrongManaCost => {
            definition.mana_cost = ManaCost::with_colors(5, [Color::Green, Color::Green])
        }
        Mutation::WrongPower => definition.power = Some(5),
        Mutation::WrongToughness => definition.toughness = Some(4),
    }
    definition
}

fn candidate_trigger(mutation: Mutation) -> Option<TriggeredAbilityBinding> {
    if mutation == Mutation::RemoveTrigger {
        return None;
    }
    let mut binding = candidate_primordial_sage::triggered_ability();
    match mutation {
        Mutation::None
        | Mutation::WrongManaCost
        | Mutation::WrongPower
        | Mutation::WrongToughness => {}
        Mutation::RemoveTrigger => unreachable!(),
        Mutation::RemoveDraw => binding.ability.effects.clear(),
        Mutation::MandatoryTrigger => binding.ability.optional = false,
        Mutation::WrongTriggerCondition => {
            binding.ability.condition = TriggerCondition::CastsNoncreatureSpell
        }
    }
    Some(binding)
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: CARD_NAME,
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(4, [Color::Green, Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "controller-casts-creature-spell-may-draw",
        ],
        power: Some(4),
        toughness: Some(5),
        keywords: vec![],
        effects: vec![],
    }
}

fn expected_trigger() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: CARD_ID,
        ability: TriggeredAbility {
            id: ABILITY_ID,
            condition: TriggerCondition::CastsCreatureSpell,
            mana_cost: ManaCost::new(0),
            optional: true,
            targets: vec![],
            effects: vec![Effect::DrawController],
        },
    }
}

fn validate_candidate() -> Result<(), String> {
    let definition = candidate_primordial_sage::definition();
    let trigger = candidate_primordial_sage::triggered_ability();
    let registered = card_definitions()
        .into_iter()
        .find(|value| value.id == CARD_ID)
        .ok_or("Primordial Sage is missing from the RAV definition registry")?;
    if definition != expected_definition()
        || definition != registered
        || trigger != expected_trigger()
        || !rav_triggered_ability_bindings().contains(&trigger)
    {
        return Err(
            "Primordial Sage card module, registry, or optional creature-cast trigger differs"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Primordial Sage candidate source: {error}"))?;
    if source.len() > 32 * 1024
        || source.contains("include!")
        || source.contains("std::fs")
        || source.contains("std::process")
    {
        return Err("Primordial Sage source exceeds the declarative module boundary".to_owned());
    }
    let file = syn::parse_file(&source)
        .map_err(|_| "Primordial Sage source is not valid Rust".to_owned())?;
    let functions: BTreeSet<String> = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Fn(function) => Some(function.sig.ident.to_string()),
            _ => None,
        })
        .collect();
    let compact: String = source.chars().filter(|ch| !ch.is_whitespace()).collect();
    let exact_source = source.trim() == EXPECTED_SOURCE.trim();
    let direct_constructors = file.items.iter().filter(|item| matches!(item, syn::Item::Fn(function)
        if function.block.stmts.len() == 1
            && matches!(&function.block.stmts[0], syn::Stmt::Expr(syn::Expr::Struct(value), None)
                if value.rest.is_none()
                    && (value.path.is_ident("CardDefinition") || value.path.is_ident("TriggeredAbilityBinding"))))).count() == 2;
    if !exact_source
        || functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()])
        || !direct_constructors
        || !compact.contains("id:\"RAV-PRIMORDIAL-SAGE\"")
        || !compact.contains("condition:TriggerCondition::CastsCreatureSpell")
        || !compact.contains("optional:true")
        || !compact.contains("effects:vec![Effect::DrawController]")
    {
        return Err(
            "Primordial Sage source differs from the pinned declarative module contract".to_owned(),
        );
    }
    Ok(())
}

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|value| value.id == CARD_ID)
        .ok_or("Primordial Sage is missing from the RAV definition registry")? =
        candidate_definition(mutation);
    let mut triggers = rav_triggered_ability_bindings();
    triggers.retain(|binding| binding.card_definition != CARD_ID);
    if let Some(binding) = candidate_trigger(mutation) {
        triggers.push(binding);
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
        || !catalog
            .iter()
            .any(|card| card.collector_number == 177 && card.name == CARD_NAME)
    {
        return Err("RAV catalog identity or printing denominator changed".to_owned());
    }
    Ok(catalog)
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
        "qualification": if passed { "pilot trace passed; independent review pending" } else { "not_evaluated" }
    })
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

fn find_traces() -> Result<(u64, Captured, Captured), String> {
    for seed in 0..=256 {
        if let (Ok(accepted), Ok(declined)) = (
            run_match(seed, Mutation::None, ChoiceMode::AcceptAll),
            run_match(seed, Mutation::None, ChoiceMode::DeclineAll),
        ) && validate_trace(&accepted, seed, ChoiceMode::AcceptAll).is_ok()
            && validate_trace(&declined, seed, ChoiceMode::DeclineAll).is_ok()
        {
            return Ok((seed, accepted, declined));
        }
    }
    Err(
        "no complete Primordial Sage accepted/declined PvP traces found for seeds 0..=256"
            .to_owned(),
    )
}

fn mutation_detected(seed: u64, mutation: Mutation, mode: ChoiceMode) -> Result<bool, String> {
    match run_match(seed, mutation, mode) {
        Err(_) => Ok(true),
        Ok(capture) => Ok(validate_trace(&capture, seed, mode).is_err()),
    }
}

fn run_match(seed: u64, mutation: Mutation, mode: ChoiceMode) -> Result<Captured, String> {
    let pilot = deck(&[
        (CARD_ID, 1),
        (WOLF_ID, 12),
        ("RAV-FOREST", 23),
        ("RAV-PLAINS", 24),
    ]);
    let opponent = deck(&[(WOLF_ID, 4), ("RAV-FOREST", 28), ("RAV-PLAINS", 28)]);
    for (label, list) in [("pilot", &pilot), ("opponent", &opponent)] {
        if list
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
        {
            return Err(format!(
                "Primordial Sage {label} deck is not a legal 60-card deck"
            ));
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
        Box::new(SagePolicy::new(PlayerId(0), true, mode, mutation)),
        Box::new(SagePolicy::new(PlayerId(1), false, mode, Mutation::None)),
    ];
    let mut capture = Captured {
        events: vec![],
        winner: None,
        turns: 0,
        accepted_moves: 0,
        rejected_moves: 0,
        life: [20, 20],
        sage: None,
        sage_cast_index: None,
        sage_resolved_index: None,
        sage_payment: vec![],
        sage_characteristics_valid: false,
        controller_creature_casts_after_sage: vec![],
        opponent_creature_casts_after_sage: vec![],
        trigger_stacks: vec![],
        trigger_resolutions: vec![],
        choices: vec![],
    };
    while !game.is_game_over() {
        if game.turn > config.max_turns || capture.accepted_moves >= config.max_policy_moves {
            return Err(format!(
                "Primordial Sage duel exceeded its bound at turn {} with life {:?}",
                game.turn,
                game.players
                    .iter()
                    .map(|player| player.life)
                    .collect::<Vec<_>>()
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Primordial Sage policy seat missing")?;
        let choice = view.optional_triggered_ability_choice.clone();
        let relevant_choice = choice
            .filter(|value| value.ability == ABILITY_ID && Some(value.source) == capture.sage);
        let library_top_before = relevant_choice
            .as_ref()
            .and_then(|_| game.players[player.0].library.last().copied());
        let hand_size_before = relevant_choice
            .as_ref()
            .map_or(0, |_| game.players[player.0].hand.len());
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
        let event_start = game.event_log.len();
        let resolved_choice = match (&relevant_choice, &action) {
            (
                Some(_),
                PolicyAction::ResolveOptionalTriggeredAbility {
                    source,
                    ability,
                    pay,
                    ..
                },
            ) if Some(*source) == capture.sage && *ability == ABILITY_ID => Some((*source, *pay)),
            _ => None,
        };
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Primordial Sage policy rejected a move at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        capture.accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        for (offset, event) in game.event_log[event_start..].iter().enumerate() {
            let index = event_start + offset;
            match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    capture.sage_payment = colors.clone()
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    capture.sage = Some(*card);
                    capture.sage_cast_index = Some(index);
                }
                GameEvent::SpellResolved { card } if Some(*card) == capture.sage => {
                    capture.sage_resolved_index = Some(index);
                    capture.sage_characteristics_valid =
                        game.characteristics(*card).is_ok_and(|value| {
                            value.colors == BTreeSet::from([Color::Green])
                                && value.card_types.contains(&CardType::Creature)
                                && value.power == Some(4)
                                && value.toughness == Some(5)
                        });
                }
                GameEvent::SpellCast { player, card }
                    if game.card_definition(*card).is_ok_and(|definition| {
                        definition.card_types.contains(&CardType::Creature)
                    }) =>
                {
                    if capture.sage_resolved_index.is_some() {
                        if *player == PlayerId(0) && Some(*card) != capture.sage {
                            capture
                                .controller_creature_casts_after_sage
                                .push((*card, index));
                        } else if *player == PlayerId(1) {
                            capture
                                .opponent_creature_casts_after_sage
                                .push((*card, index));
                        }
                    }
                }
                GameEvent::TriggeredAbilityStacked {
                    source, ability, ..
                } if Some(*source) == capture.sage && *ability == ABILITY_ID => {
                    capture.trigger_stacks.push((*source, index))
                }
                GameEvent::AbilityResolved {
                    source, ability, ..
                } if Some(*source) == capture.sage && *ability == ABILITY_ID => {
                    capture.trigger_resolutions.push((*source, index))
                }
                _ => {}
            }
        }
        if let Some((source, accepted)) = resolved_choice {
            let moved_top_to_hand = library_top_before.is_some_and(|card| {
                game.players[player.0].hand.contains(&card)
                    && !game.players[player.0].library.contains(&card)
            });
            let draw_move_index = library_top_before.and_then(|card| {
                game.event_log[event_start..]
                    .iter()
                    .position(|event| matches!(event, GameEvent::CardMoved { card: moved, to: cardbench_magic_engine::Zone::Hand } if *moved == card))
                    .map(|offset| event_start + offset)
            });
            capture.choices.push(ChoiceObservation {
                source,
                accepted,
                library_top_before,
                library_top_after: game.players[player.0].library.last().copied(),
                hand_size_before,
                hand_size_after: game.players[player.0].hand.len(),
                moved_top_to_hand,
                draw_move_index,
                action_event_index: event_start,
            });
        }
    }
    capture.events = game.event_log.clone();
    capture.winner = game.winner().map(|player| player.0);
    capture.turns = game.turn;
    capture.life = [game.players[0].life, game.players[1].life];
    Ok(capture)
}

fn validate_trace(capture: &Captured, seed: u64, mode: ChoiceMode) -> Result<Value, String> {
    if capture.winner != Some(0) || capture.rejected_moves != 0 {
        return Err(format!(
            "Primordial Sage duel did not reach a clean P0 win: winner={:?}, turns={}, rejected={}",
            capture.winner, capture.turns, capture.rejected_moves
        ));
    }
    let sage = capture.sage.ok_or("Primordial Sage was not cast")?;
    if capture.sage_payment != vec![Color::Green; 6]
        || capture.sage_resolved_index.is_none()
        || !capture.sage_characteristics_valid
        || capture.controller_creature_casts_after_sage.is_empty()
        || capture.opponent_creature_casts_after_sage.is_empty()
        || capture.trigger_stacks.len() != capture.controller_creature_casts_after_sage.len()
        || capture.trigger_resolutions.len() != capture.trigger_stacks.len()
        || capture.choices.len() != capture.trigger_stacks.len()
    {
        return Err(
            "Primordial Sage payment, live body, controller scope, or trigger count differs"
                .to_owned(),
        );
    }
    let accepted = capture
        .choices
        .iter()
        .filter(|choice| choice.accepted)
        .count();
    let declined = capture.choices.len().saturating_sub(accepted);
    if (mode == ChoiceMode::AcceptAll && (accepted == 0 || declined != 0))
        || (mode == ChoiceMode::DeclineAll && (declined == 0 || accepted != 0))
    {
        return Err(
            "Primordial Sage optional trigger did not follow the selected accept/decline branch"
                .to_owned(),
        );
    }
    for choice in &capture.choices {
        if choice.source != sage {
            return Err("Primordial Sage optional choice named the wrong source".to_owned());
        }
        if choice.accepted {
            if !choice.moved_top_to_hand || choice.hand_size_after != choice.hand_size_before + 1 {
                return Err(
                    "accepted Primordial Sage trigger did not draw the exact top card".to_owned(),
                );
            }
        } else if choice.library_top_before != choice.library_top_after
            || choice.hand_size_after != choice.hand_size_before
            || choice.moved_top_to_hand
        {
            return Err("declined Primordial Sage trigger changed the library or hand".to_owned());
        }
    }
    let sage_cast = capture
        .sage_cast_index
        .ok_or("Primordial Sage cast event missing")?;
    let sage_resolved = capture
        .sage_resolved_index
        .ok_or("Primordial Sage resolution event missing")?;
    let mut trigger_sequences = Vec::new();
    for (((source, stacked), (_, resolved)), choice) in capture
        .trigger_stacks
        .iter()
        .zip(&capture.trigger_resolutions)
        .zip(&capture.choices)
    {
        if *source != sage || stacked >= resolved {
            return Err("Primordial Sage trigger source or resolution order differs".to_owned());
        }
        let creature_cast = capture
            .controller_creature_casts_after_sage
            .iter()
            .filter(|(_, index)| index < stacked)
            .map(|(_, index)| *index)
            .max()
            .ok_or("Primordial Sage trigger has no preceding controller creature cast")?;
        let creature_resolved = capture.events.iter().enumerate().skip(*resolved + 1).find_map(|(index, event)|
            matches!(event, GameEvent::SpellResolved { card } if capture.controller_creature_casts_after_sage.iter().any(|(creature, cast)| *creature == *card && *cast == creature_cast)).then_some(index))
            .ok_or("creature spell below the Sage trigger did not resolve")?;
        if !(sage_cast < sage_resolved
            && creature_cast < *stacked
            && *stacked < *resolved
            && *stacked < choice.action_event_index
            && choice.action_event_index < *resolved
            && *resolved < creature_resolved)
        {
            return Err(
                "Primordial Sage trigger did not resolve above its creature spell".to_owned(),
            );
        }
        if choice.accepted && choice.draw_move_index.is_none()
            || !choice.accepted && choice.draw_move_index.is_some()
        {
            return Err(
                "Primordial Sage accepted/declined choice did not match its exact draw event"
                    .to_owned(),
            );
        }
        trigger_sequences.push(json!({"creature_cast": creature_cast, "trigger_stacked": stacked, "choice_action": choice.action_event_index, "draw_move": choice.draw_move_index, "trigger_resolved": resolved, "creature_resolved": creature_resolved}));
    }
    for (_, opponent_cast) in &capture.opponent_creature_casts_after_sage {
        let next_controller_cast = capture
            .controller_creature_casts_after_sage
            .iter()
            .map(|(_, index)| *index)
            .find(|index| index > opponent_cast)
            .unwrap_or(capture.events.len());
        if capture
            .trigger_stacks
            .iter()
            .any(|(_, trigger)| trigger > opponent_cast && *trigger < next_controller_cast)
        {
            return Err(
                "Primordial Sage incorrectly triggered for an opponent's creature spell".to_owned(),
            );
        }
    }
    Ok(json!({
        "trace_id": if mode == ChoiceMode::AcceptAll { "rav-primordial-sage-creature-cast-optional-draw-accepted" } else { "rav-primordial-sage-creature-cast-optional-draw-declined" },
        "passed": true,
        "seed": seed,
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": 0,
        "turns": capture.turns,
        "accepted_moves": capture.accepted_moves,
        "rejected_moves": capture.rejected_moves,
        "deck_p0": "1 Primordial Sage + 12 Watchwolf + 47 Forests/Plains (60 shuffled cards)",
        "deck_p1": "4 Watchwolf + 56 Forests/Plains (60 shuffled cards)",
        "sage": {"object": sage.0, "mana_cost": "{4}{G}{G}", "mana_payment": capture.sage_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "power": 4, "toughness": 5, "live_characteristics_valid": capture.sage_characteristics_valid},
        "trigger": {"ability": ABILITY_ID, "condition": "CastsCreatureSpell", "source": sage.0, "controller_creature_spells": capture.controller_creature_casts_after_sage.len(), "opponent_creature_spells": capture.opponent_creature_casts_after_sage.len(), "trigger_count": capture.trigger_stacks.len(), "accepted": accepted, "declined": declined, "draw_choice": if mode == ChoiceMode::AcceptAll { "accept" } else { "decline" }, "choice_card_zones_valid": true},
        "ordered_trigger_sequences": trigger_sequences,
        "life_totals": capture.life,
        "events": project_events(&capture.events),
        "event_count": capture.events.len()
    }))
}

struct SagePolicy {
    player: PlayerId,
    pilot: bool,
    mode: ChoiceMode,
    mutation: Mutation,
    inner: RavCatalogPolicy,
    sage_cast_started: bool,
}

impl SagePolicy {
    fn new(player: PlayerId, pilot: bool, mode: ChoiceMode, mutation: Mutation) -> Self {
        Self {
            player,
            pilot,
            mode,
            mutation,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
            sage_cast_started: false,
        }
    }

    fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let mut available = Color::MANA_ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect::<Vec<_>>();
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

    fn make_mana(view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
        let mut required = std::collections::BTreeMap::<Color, u8>::new();
        for color in &cost.colored {
            *required.entry(*color).or_default() += 1;
        }
        for (color, amount) in required {
            if view.mana_pool.amount(color) < amount {
                let land = view.own_battlefield.iter().find(|card| {
                    card.card_types.contains(&CardType::Land)
                        && !card.tapped
                        && card.mana_colors.contains(&color)
                })?;
                return Some(PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color,
                });
            }
        }
        if view.mana_pool.total() < u16::from(cost.mana_value()) {
            let land = view.own_battlefield.iter().find(|card| {
                card.card_types.contains(&CardType::Land)
                    && !card.tapped
                    && !card.mana_colors.is_empty()
            })?;
            let color = if land.mana_colors.contains(&Color::Green) {
                Color::Green
            } else {
                *land.mana_colors.iter().next()?
            };
            return Some(PolicyAction::ActivateManaAbility {
                land: land.id,
                color,
            });
        }
        None
    }
}

impl CodePolicy for SagePolicy {
    fn id(&self) -> &'static str {
        if self.pilot {
            "rav.primordial-sage-cast-trigger-pilot.v1"
        } else {
            "rav.primordial-sage-watchwolf-opponent.v1"
        }
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if self.pilot
            && !self.sage_cast_started
            && view.active_player == self.player
            && view.step == Step::PrecombatMain
            && view.priority == self.player
            && view.stack_depth == 0
        {
            if let Some(sage) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            {
                let cost = candidate_definition(self.mutation).mana_cost;
                if let Some(mana_selection) = Self::payment(view, &cost) {
                    self.sage_cast_started = true;
                    return PolicyAction::CastWithPayment {
                        request: CastRequest {
                            card: sage.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection,
                    };
                }
                if let Some(action) = Self::make_mana(view, &cost) {
                    return action;
                }
            }
        }
        self.inner.propose_move(view)
    }

    fn propose_optional_triggered_ability(&mut self, view: &GameView) -> PolicyAction {
        let Some(choice) = &view.optional_triggered_ability_choice else {
            return self.inner.propose_optional_triggered_ability(view);
        };
        if self.pilot && choice.ability == ABILITY_ID {
            PolicyAction::ResolveOptionalTriggeredAbility {
                decision: choice.decision,
                source: choice.source,
                ability: ABILITY_ID,
                pay: self.mode == ChoiceMode::AcceptAll,
                target: None,
            }
        } else {
            self.inner.propose_optional_triggered_ability(view)
        }
    }

    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        self.inner.propose_pending_decision(view)
    }
    fn propose_draw_replacement(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_draw_replacement(view)
    }
    fn propose_private_library_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_private_library_choice(view)
    }
    fn propose_private_opponent_library_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_private_opponent_library_choice(view)
    }
    fn propose_library_search_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_library_search_choice(view)
    }
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "candidate"
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

const EXPECTED_SOURCE: &str = r#"//! Candidate-owned RAV #177 Primordial Sage definition and cast trigger.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, ManaCost, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-PRIMORDIAL-SAGE",
        name: "Primordial Sage",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(4, [Color::Green, Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "controller-casts-creature-spell-may-draw",
        ],
        power: Some(4),
        toughness: Some(5),
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-PRIMORDIAL-SAGE",
        ability: TriggeredAbility {
            id: "controller-creature-spell-cast-may-draw",
            condition: TriggerCondition::CastsCreatureSpell,
            mana_cost: ManaCost::new(0),
            optional: true,
            targets: vec![],
            effects: vec![Effect::DrawController],
        },
    }
}
"#;
