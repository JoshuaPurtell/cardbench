//! Full-game RAV Ethereal Usher target-Unblockable trace authority.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    AbilityActivation, CardDefinition, CardType, CastRequest, Color, DecisionKind,
    DecisionVisibility, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, Keyword, Layer,
    LibrarySearchDestination, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction,
    Step, Target, TargetRequirement, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::{ObjectRef, SeatId, ViewerScope};
use cardbench_magic_rav::{
    RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use syn::{Expr, Item, ReturnType, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-ETHEREAL-USHER";
const SUPPORT_ID: &str = "RAV-WATCHWOLF";
const BLOCKER_ID: &str = "RAV-WATCHWOLF";
const ABILITY_ID: &str = "tap-target-unblockable";
const TRANSMUTE_ID: &str = "transmute";
const MAX_TRACE_SEED: u64 = 256;
const MAX_TURNS: u32 = 48;
const MAX_MOVES: u32 = 8_000;
const EXPECTED_CANDIDATE_SOURCE: &str = r#"//! Candidate-owned RAV Ethereal Usher definition and activated ability.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, Keyword,
    ManaCost, TargetRequirement,
};

/// RAV #47 is a blue 2/3 Spirit with Transmute {1}{U}{U}.
/// `CardDefinition` has no subtype field, so Spirit remains source identity only.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-ETHEREAL-USHER",
        name: "Ethereal Usher",
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
            "activated-target-unblockable-until-end-of-turn",
            "transmute",
        ],
        power: Some(2),
        toughness: Some(3),
        keywords: vec![Keyword::Transmute(ManaCost::with_colors(
            1,
            [Color::Blue, Color::Blue],
        ))],
        effects: vec![],
    }
}

/// `{U}, {T}: Target creature can't be blocked this turn.`
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-ETHEREAL-USHER",
        ability: ActivatedAbility {
            id: "tap-target-unblockable",
            mana_cost: ManaCost::with_colors(0, [Color::Blue]),
            tap_cost: true,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![TargetRequirement::Creature],
            effects: vec![Effect::ModifyTargetKeywordUntilEndOfTurn {
                keyword: Keyword::Unblockable,
            }],
        },
    }
}
"#;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/ethereal_usher.rs"]
mod candidate_ethereal_usher;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Ethereal Usher PvP result: {error}");
                    return ExitCode::FAILURE;
                }
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
            eprintln!("Ethereal Usher PvP authority failed: {error}");
            ExitCode::FAILURE
        }
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
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(root.join("reward.txt"), "0.00000000\n").map_err(|error| error.to_string())
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

#[derive(Clone, Copy, Debug)]
enum Mutation {
    None,
    WrongEffect,
    WrongAbilityCost,
    WrongCastCost,
    WrongPower,
    WrongToughness,
    NoTransmute,
    WrongTransmuteCost,
}

fn candidate_values(
    mutation: Mutation,
) -> (CardDefinition, cardbench_magic_engine::ActivatedAbility) {
    let mut definition = candidate_ethereal_usher::definition();
    let mut ability = candidate_ethereal_usher::activated_ability().ability;
    match mutation {
        Mutation::None => {}
        Mutation::WrongEffect => {
            ability.effects = vec![Effect::ModifyTargetKeywordUntilEndOfTurn {
                keyword: Keyword::Vigilance,
            }]
        }
        Mutation::WrongAbilityCost => ability.mana_cost = ManaCost::new(0),
        Mutation::WrongCastCost => definition.mana_cost = ManaCost::with_colors(1, [Color::Blue]),
        Mutation::WrongPower => definition.power = Some(1),
        Mutation::WrongToughness => definition.toughness = Some(2),
        Mutation::NoTransmute => definition
            .keywords
            .retain(|keyword| !matches!(keyword, Keyword::Transmute(_))),
        Mutation::WrongTransmuteCost => {
            definition.keywords = vec![Keyword::Transmute(ManaCost::with_colors(
                2,
                [Color::Blue, Color::Blue],
            ))]
        }
    }
    (definition, ability)
}

fn validate_candidate_source() -> Result<Value, String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../sets/ravnica_city_of_guilds/src/cards/ethereal_usher.rs");
    let source = std::fs::read_to_string(&path)
        .map_err(|error| format!("could not read Ethereal Usher candidate module: {error}"))?;
    validate_candidate_source_text(&source)?;
    let injected = source.replacen(
        "pub(crate) fn definition() -> CardDefinition {",
        "pub(crate) fn definition() -> CardDefinition {\n    let _ = std::fs::write(\"/tmp/cardbench-ethereal-usher-candidate-executed\", b\"ran\");",
        1,
    );
    if injected == source || syn::parse_file(&injected).is_err() {
        return Err(
            "compile-valid Ethereal Usher side-effect fixture could not be constructed".into(),
        );
    }
    if validate_candidate_source_text(&injected).is_ok() {
        return Err("Ethereal Usher source guard accepted an injected side effect".into());
    }
    Ok(json!({
        "canonical_normalized_source_checked": true,
        "candidate_ast_contract_matched": true,
        "compile_valid_injected_side_effect_fixture_compiled_by_construction": true,
        "compile_valid_injected_side_effect_candidate_rejected": true,
        "injected_candidate_calls_before_rejection": 0,
        "guard_ran_before_candidate_function_calls": true,
        "rejection_stage": "normalized-source-and-declarative-literal-AST-before-candidate-function-invocation"
    }))
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let file = syn::parse_file(source)
        .map_err(|_| "Ethereal Usher module is not valid Rust source".to_owned())?;
    let expected = syn::parse_file(EXPECTED_CANDIDATE_SOURCE)
        .expect("pinned Ethereal Usher candidate source parses");
    let normalized = |text: &str| {
        text.chars()
            .filter(|character| !character.is_whitespace())
            .collect::<String>()
    };
    let functions: Vec<_> = file
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Fn(function) => Some(function),
            _ => None,
        })
        .collect();
    let shape_ok = file.attrs.iter().all(|attribute| attribute.path().is_ident("doc"))
        && file.items.len() == 4
        && functions.len() == 2
        && [
            ("definition", "CardDefinition"),
            ("activated_ability", "ActivatedAbilityBinding"),
        ].iter().all(|(name, return_type)| functions.iter().any(|function| {
            function.sig.ident == *name
                && matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                && function.sig.asyncness.is_none()
                && function.sig.unsafety.is_none()
                && function.sig.inputs.is_empty()
                && function.sig.generics.params.is_empty()
                && function.sig.generics.where_clause.is_none()
                && function.sig.variadic.is_none()
                && matches!(&function.sig.output, ReturnType::Type(_, ty) if matches!(&**ty, syn::Type::Path(path) if path.qself.is_none() && path.path.is_ident(return_type)))
                && function.block.stmts.len() == 1
                && matches!(function.block.stmts.first(), Some(Stmt::Expr(Expr::Struct(_), None)))
        }));
    if !shape_ok
        || normalized(source) != normalized(EXPECTED_CANDIDATE_SOURCE)
        || format!("{file:?}") != format!("{expected:?}")
    {
        return Err(
            "Ethereal Usher candidate AST differs from the pinned RAV source contract".into(),
        );
    }
    Ok(())
}

fn validate_candidate_runtime() -> Result<(), String> {
    let (definition, ability) = candidate_values(Mutation::None);
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Ethereal Usher definition is missing from the RAV registry")?;
    let binding = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("Ethereal Usher activated ability is missing from the RAV registry")?;
    if registered != definition || binding.ability != ability {
        return Err(
            "Ethereal Usher definition and ability are not delegated to its candidate module"
                .into(),
        );
    }
    if definition.id != CARD_ID
        || definition.name != "Ethereal Usher"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(5, [Color::Blue])
        || definition.colors != BTreeSet::from([Color::Blue])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.power != Some(2)
        || definition.toughness != Some(3)
        || definition.keywords
            != [Keyword::Transmute(ManaCost::with_colors(
                1,
                [Color::Blue, Color::Blue],
            ))]
        || !definition.effects.is_empty()
        || ability.id != ABILITY_ID
        || ability.mana_cost != ManaCost::with_colors(0, [Color::Blue])
        || !ability.tap_cost
        || ability.sorcery_speed
        || ability.additional_tap_creatures != 0
        || ability.sacrifice_source
        || ability.sacrifice_creatures != 0
        || ability.sacrifice_lands != 0
        || ability.discard_cards != 0
        || ability.targets != [TargetRequirement::Creature]
        || ability.effects
            != [Effect::ModifyTargetKeywordUntilEndOfTurn {
                keyword: Keyword::Unblockable,
            }]
    {
        return Err(
            "Ethereal Usher identity or activated Unblockable feature differs from RAV #47".into(),
        );
    }
    Ok(())
}

fn run_authority() -> Result<Value, String> {
    let source_validation = validate_candidate_source()?;
    validate_candidate_runtime()?;
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
        return Err("RAV catalog denominator differs from its source contract".into());
    }
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.terminal != replay.terminal
        || captured.winner != replay.winner
        || captured.usher != replay.usher
        || captured.target != replay.target
        || captured.target_keywords != replay.target_keywords
        || captured.illegal_block_rejection != replay.illegal_block_rejection
        || captured.transmute_source != replay.transmute_source
        || captured.transmute_selected != replay.transmute_selected
        || captured.transmute_found != replay.transmute_found
        || captured.transmute_cost != replay.transmute_cost
        || captured.transmute_resolved != replay.transmute_resolved
        || captured.transmute_source_incarnation != replay.transmute_source_incarnation
        || captured.transmute_resolved_incarnation != replay.transmute_resolved_incarnation
        || captured.transmute_found_definition != replay.transmute_found_definition
        || captured.transmute_found_in_hand != replay.transmute_found_in_hand
        || captured.transmute_private_choice != replay.transmute_private_choice
        || captured.transmute_hidden_from_opponent != replay.transmute_hidden_from_opponent
        || captured.transmute_source_zone != replay.transmute_source_zone
        || captured.transmute_shuffled != replay.transmute_shuffled
        || captured.transmute_timing_valid != replay.transmute_timing_valid
        || captured.cast_payment != replay.cast_payment
        || captured.ability_source_tapped != replay.ability_source_tapped
        || captured.unblockable_at_attack != replay.unblockable_at_attack
        || captured.expiry_observed != replay.expiry_observed
        || captured.target_damage != replay.target_damage
        || captured.life != replay.life
    {
        return Err("Ethereal Usher shuffled-game replay was not deterministic".into());
    }
    let mutations = json!({
        "unblockable_effect_mutation_detected_in_game":mutation_detected(seed,Mutation::WrongEffect)?,
        "activated_ability_cost_mutation_detected_in_game":mutation_detected(seed,Mutation::WrongAbilityCost)?,
        "casting_cost_mutation_detected_in_game":mutation_detected(seed,Mutation::WrongCastCost)?,
        "base_power_mutation_detected_in_game":mutation_detected(seed,Mutation::WrongPower)?,
        "base_toughness_mutation_detected_in_game":mutation_detected(seed,Mutation::WrongToughness)?,
        "transmute_removed_mutation_detected_in_game":mutation_detected(seed,Mutation::NoTransmute)?,
        "transmute_cost_mutation_detected_in_game":mutation_detected(seed,Mutation::WrongTransmuteCost)?,
        "compile_valid_candidate_side_effect_rejected_before_execution":source_validation["compile_valid_injected_side_effect_candidate_rejected"],
        "sealed_mutation_authority":false
    });
    for key in [
        "unblockable_effect_mutation_detected_in_game",
        "activated_ability_cost_mutation_detected_in_game",
        "casting_cost_mutation_detected_in_game",
        "base_power_mutation_detected_in_game",
        "base_toughness_mutation_detected_in_game",
        "transmute_removed_mutation_detected_in_game",
        "transmute_cost_mutation_detected_in_game",
        "compile_valid_candidate_side_effect_rejected_before_execution",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Ethereal Usher semantic mutation escaped the full-game trace: {key}"
            ));
        }
    }
    let breakdown:Vec<_>=catalog.iter().map(|card|{let id=card_definitions().into_iter().find(|definition|definition.name==card.name).map(|definition|definition.id).unwrap_or("");json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,"engine_status":if card.semantic_status.is_executable(){"executable_compatibility_slice"}else{"catalog_only"},"trace_status":if id==CARD_ID{"passed"}else{"not_qualified"},"qualification":if id==CARD_ID{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})}).collect();
    Ok(
        json!({"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":TASK_ID,"expansion":"RAV","card":{"collector_number":47,"definition_id":CARD_ID,"name":"Ethereal Usher"},"candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},"candidate_kind":candidate_kind(),"viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{"comprehensive_rules":"2026-08-07","checked_sections":["117 Priority and timing","302.6 Summoning sickness","508.1 Attacking","509.1 Blocking and unblockability","510 Combat damage","601.2 Casting","602.1-602.2 Activated abilities","608.2 Ability resolution","611.2 Until-end-of-turn effects","701.19 Search and shuffle","702.53 Transmute"],"source_contract":"RAV Ethereal Usher #47 is a blue {5}{U} 2/3 Creature — Spirit; {U}, {T}: target creature is unblockable this turn; Transmute {1}{U}{U}","oracle_reconciliation":"Oracle's can't-be-blocked and mana-value phrases preserve the printed RAV behavior; Spirit is retained identity only because CardDefinition has no subtype field","authority_cutoff":"2026-09-24","no_provider_trial":true},
        "candidate_source_validation":source_validation,
        "passed":true,"pilot_card_passed":true,"replay_deterministic":true,"cards_passed":1,"cards_credited":0,"cards_total":catalog.len(),"printings_total":printings.len(),"coverage_complete":false,"full_reward":false,"harbor_reward":0.0,"pilot_reward":0.0,"qualification_status":"diagnostic_only; independent review and candidate trial pending","mutation_sensitivity":mutations,"traces":[trace],"per_card_breakdown":breakdown}),
    )
}

#[derive(Clone, Debug)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    usher: Option<ObjectId>,
    target: Option<ObjectId>,
    transmute_source: Option<ObjectId>,
    transmute_source_incarnation: Option<u64>,
    transmute_resolved_incarnation: Option<u64>,
    transmute_cost: Option<ManaCost>,
    transmute_selected: Option<ObjectId>,
    transmute_found: Option<ObjectId>,
    transmute_found_definition: Option<&'static str>,
    transmute_found_in_hand: bool,
    transmute_resolved: bool,
    transmute_shuffled: bool,
    transmute_private_choice: bool,
    transmute_hidden_from_opponent: bool,
    transmute_decision: Option<u64>,
    transmute_source_zone: Option<Zone>,
    transmute_turn: Option<u32>,
    transmute_timing_valid: bool,
    cast_cost: ManaCost,
    cast_payment: Vec<Color>,
    definition: CardDefinition,
    ability: cardbench_magic_engine::ActivatedAbility,
    target_keywords: Vec<String>,
    ability_source_tapped: bool,
    unblockable_at_attack: bool,
    target_zone: Option<Zone>,
    usher_zone: Option<Zone>,
    target_blocked: bool,
    illegal_block_rejection: Option<String>,
    expiry_observed: bool,
    target_damage: i64,
    life: [i64; 2],
    ability_turn: Option<u32>,
    attack_turn: Option<u32>,
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_ETHEREAL_USHER_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_ETHEREAL_USHER_SEED must be an unsigned integer")?;
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
        "no Ethereal Usher full-game Unblockable trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Ok(c) => Ok(validate_trace(&c, seed).is_err()),
        Err(_) => Ok(true),
    }
}

fn new_game(
    definition: CardDefinition,
    ability: cardbench_magic_engine::ActivatedAbility,
) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|item| item.id == CARD_ID)
        .ok_or("Ethereal Usher definition missing from registry")? = definition;
    let mut abilities = rav_activated_ability_bindings();
    let binding = abilities
        .iter_mut()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("Ethereal Usher ability missing from registry")?;
    binding.ability = ability;
    Game::new_with_all_bindings(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
    )
    .map_err(|error| error.to_string())
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let (definition, ability) = candidate_values(mutation);
    let cast_cost = definition.mana_cost.clone();
    let transmute_cost = definition.transmute_cost().cloned();
    let mut game = new_game(definition.clone(), ability.clone())?;
    let p0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: SUPPORT_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-ISLAND".into(),
                count: 20,
            },
            DeckEntry {
                card: "RAV-FOREST".into(),
                count: 16,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 16,
            },
        ],
        sideboard: vec![],
    };
    let p1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: BLOCKER_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".into(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 28,
            },
        ],
        sideboard: vec![],
    };
    if [p0.mainboard.as_slice(), p1.mainboard.as_slice()]
        .into_iter()
        .any(|board| {
            board
                .iter()
                .map(|entry| entry.count as usize)
                .sum::<usize>()
                != 60
        })
    {
        return Err("Ethereal Usher duel decks must each contain 60 cards".into());
    }
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &p0)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &p1)
        .map_err(|error| error.to_string())?;
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_MOVES,
        ..DeckMatchConfig::default()
    };
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let support_cost = card_definitions()
        .into_iter()
        .find(|card| card.id == SUPPORT_ID)
        .ok_or("Watchwolf definition missing")?
        .mana_cost;
    let blocker_cost = card_definitions()
        .into_iter()
        .find(|card| card.id == BLOCKER_ID)
        .ok_or("opposing Watchwolf definition missing")?
        .mana_cost;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(UsherPolicy::new(
            PlayerId(0),
            &cast_cost,
            &ability.mana_cost,
            &support_cost,
            transmute_cost.as_ref(),
        )),
        Box::new(UsherPolicy::new(
            PlayerId(1),
            &blocker_cost,
            &ManaCost::new(0),
            &support_cost,
            None,
        )),
    ];
    let (mut accepted, mut attempted) = (0u32, 0u32);
    let (mut usher, mut target) = (None, None);
    let (mut target_keywords, mut cast_payment) = (Vec::new(), Vec::new());
    let (mut unblockable_at_attack, mut expiry_observed) = (false, false);
    let mut ability_source_tapped = false;
    let mut illegal_block_rejection = None;
    let (mut ability_turn, mut attack_turn) = (None, None);
    let mut transmute_source = None;
    let mut transmute_source_incarnation = None;
    let mut transmute_resolved_incarnation = None;
    let mut transmute_cost = None;
    let mut transmute_selected = None;
    let mut transmute_found = None;
    let mut transmute_found_definition = None;
    let mut transmute_found_in_hand = false;
    let mut transmute_resolved = false;
    let mut transmute_shuffled = false;
    let mut transmute_private_choice = false;
    let mut transmute_hidden_from_opponent = true;
    let mut transmute_decision = None;
    let mut transmute_source_zone = None;
    let mut transmute_turn = None;
    let mut transmute_timing_valid = false;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Ethereal Usher match exceeded bounded move count".into());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        if !expiry_observed
            && player == PlayerId(0)
            && view.step == Step::PrecombatMain
            && ability_turn.is_some_and(|turn| view.turn > turn)
        {
            if let Some(target_id) = target {
                if let Some(target_view) = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.id == target_id)
                {
                    expiry_observed = !target_view.keywords.contains(&Keyword::Unblockable);
                }
            }
        }
        if player == PlayerId(0)
            && view.step == Step::DeclareAttackers
            && view.active_player == player
        {
            if let Some(target_id) = target {
                let carries = view.own_battlefield.iter().any(|card| {
                    card.id == target_id && card.keywords.contains(&Keyword::Unblockable)
                });
                if carries {
                    unblockable_at_attack = true;
                }
            }
        }
        let policy = policies.get_mut(player.0).ok_or("policy seat missing")?;
        if illegal_block_rejection.is_none()
            && player == PlayerId(1)
            && view.step == Step::DeclareBlockers
            && view.active_player == PlayerId(0)
            && !view.blockers_declared
        {
            if let Some(attacker) = view.combat_attackers.iter().find(|card| {
                Some(card.id) == target && card.keywords.contains(&Keyword::Unblockable)
            }) {
                if let Some(blocker) = view.own_battlefield.iter().find(|card| card.can_block) {
                    let mut probe = game.clone();
                    let block = PolicyAction::DeclareBlockers {
                        assignments: vec![cardbench_magic_engine::CombatBlock {
                            attacker: attacker.id,
                            blocker: blocker.id,
                        }],
                    };
                    if let Err(error) = probe.submit_policy_move(player, policy.id(), block) {
                        illegal_block_rejection = Some(error.to_string());
                    }
                }
            }
        }
        if let Some(choice) = &view.library_search_choice {
            if player == PlayerId(0) {
                transmute_private_choice = true;
                transmute_decision = Some(choice.decision.0);
                let opponent_view = game
                    .view_for_player(PlayerId(1))
                    .map_err(|error| error.to_string())?;
                transmute_hidden_from_opponent &= opponent_view.library_search_choice.is_none();
            }
        }
        let action = if view.library_search_choice.is_some() {
            policy.propose_library_search_choice(&view)
        } else if let Some(action) = policy.propose_pending_decision(&view) {
            action
        } else if view.optional_triggered_ability_choice.is_some() {
            policy.propose_optional_triggered_ability(&view)
        } else if view.draw_replacement_pending {
            policy.propose_draw_replacement(&view)
        } else if view.private_library_choice.is_some() {
            policy.propose_private_library_choice(&view)
        } else if view.private_opponent_library_choice.is_some() {
            policy.propose_private_opponent_library_choice(&view)
        } else {
            policy.propose_move(&view)
        };
        match &action {
            PolicyAction::Transmute { card }
                if player == PlayerId(0) && transmute_source.is_none() =>
            {
                transmute_source = Some(*card);
                transmute_source_incarnation =
                    game.object(*card).ok().map(|object| object.incarnation);
                transmute_turn = Some(view.turn);
                transmute_timing_valid = view.active_player == player
                    && view.priority == player
                    && view.step == Step::PrecombatMain
                    && view.stack_depth == 0;
            }
            PolicyAction::ChooseLibrarySearchCard {
                source, selected, ..
            } if Some(*source) == transmute_source => transmute_selected = *selected,
            _ => {}
        }
        let before = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Ethereal Usher policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("Ethereal Usher invariant failed after move: {error}"))?;
        let new_events = &game.event_log[before..];
        observe_transmute(
            &game,
            new_events,
            transmute_source,
            transmute_selected,
            &mut transmute_cost,
            &mut transmute_source_incarnation,
            &mut transmute_resolved_incarnation,
            &mut transmute_found,
            &mut transmute_found_definition,
            &mut transmute_found_in_hand,
            &mut transmute_resolved,
            &mut transmute_private_choice,
            &mut transmute_decision,
            &mut transmute_source_zone,
            &mut transmute_shuffled,
        )?;
        if usher.is_none() {
            usher = new_events.iter().find_map(|event| match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|card| card.id == CARD_ID) =>
                {
                    Some(*card)
                }
                _ => None,
            });
        }
        if let Some(source) = usher {
            if cast_payment.is_empty() {
                cast_payment = new_events
                    .iter()
                    .find_map(|event| match event {
                        GameEvent::SpellManaPaid {
                            player: PlayerId(0),
                            card,
                            colors,
                        } if *card == source => Some(colors.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
            }
            if ability_turn.is_none()&&new_events.iter().any(|event|matches!(event,GameEvent::AbilityResolved{source:resolved,ability:ABILITY_ID,..}if *resolved==source)) {
                ability_turn=Some(game.turn); let after=game.view_for_player(PlayerId(0)).map_err(|error|error.to_string())?;
                ability_source_tapped = after.own_battlefield.iter().any(|permanent| permanent.id == source && permanent.tapped);
                if let Some(chosen)=after.own_battlefield.iter().find(|permanent|permanent.definition==Some(SUPPORT_ID)&&permanent.keywords.contains(&Keyword::Unblockable)) {
                    target=Some(chosen.id); target_keywords=chosen.keywords.iter().map(|keyword|format!("{keyword:?}")).collect();
                }
            }
        }
        if let Some(target_id) = target {
            if attack_turn.is_none() && new_events.iter().any(|event|matches!(event,GameEvent::AttackersDeclared{player:PlayerId(0),attackers}if attackers.contains(&target_id))){attack_turn=Some(game.turn);}
        }
    }
    let events = game.event_log.clone();
    let target_blocked=target.is_some_and(|target_id|events.iter().any(|event|matches!(event,GameEvent::BlockersDeclared{assignments,..}if assignments.iter().any(|(attacker,_)|*attacker==target_id))));
    let target_damage = target.map_or(0, |target_id| {
        events
            .iter()
            .filter_map(|event| match event {
                GameEvent::DamageDealtToPlayer {
                    source,
                    player: PlayerId(1),
                    amount,
                } if *source == target_id => Some(i64::from(*amount)),
                _ => None,
            })
            .sum()
    });
    let grave_moved = |object: ObjectId| {
        events.iter().any(
            |event| matches!(event,GameEvent::CardMoved{card,to:Zone::Graveyard}if *card==object),
        )
    };
    let usher_zone = usher.and_then(|object| {
        grave_moved(object)
            .then_some(Zone::Graveyard)
            .or_else(|| game.zone_of(object))
    });
    let target_zone = target.and_then(|object| {
        grave_moved(object)
            .then_some(Zone::Graveyard)
            .or_else(|| game.zone_of(object))
    });
    let life = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    Ok(Captured {
        events,
        terminal: game.winner().map_or_else(
            || "Draw".into(),
            |seat| format!("Winner(PlayerId({}))", seat.0),
        ),
        winner: game.winner().map(|seat| seat.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        usher,
        target,
        transmute_source,
        transmute_source_incarnation,
        transmute_resolved_incarnation,
        transmute_cost,
        transmute_selected,
        transmute_found,
        transmute_found_definition,
        transmute_found_in_hand,
        transmute_resolved,
        transmute_shuffled,
        transmute_private_choice,
        transmute_hidden_from_opponent,
        transmute_decision,
        transmute_source_zone,
        transmute_turn,
        transmute_timing_valid,
        cast_cost,
        cast_payment,
        definition,
        ability,
        target_keywords,
        ability_source_tapped,
        unblockable_at_attack,
        target_zone,
        usher_zone,
        target_blocked,
        illegal_block_rejection,
        expiry_observed,
        target_damage,
        life,
        ability_turn,
        attack_turn,
    })
}

fn observe_transmute(
    _game: &Game,
    events: &[GameEvent],
    source: Option<ObjectId>,
    selected: Option<ObjectId>,
    mana_cost: &mut Option<ManaCost>,
    source_incarnation: &mut Option<u64>,
    resolved_incarnation: &mut Option<u64>,
    found: &mut Option<ObjectId>,
    found_definition: &mut Option<&'static str>,
    found_in_hand: &mut bool,
    resolved: &mut bool,
    private_choice: &mut bool,
    decision: &mut Option<u64>,
    source_zone: &mut Option<Zone>,
    shuffled: &mut bool,
) -> Result<(), String> {
    for event in events {
        match event {
            GameEvent::AbilityManaPaid {
                player: PlayerId(0),
                source: object,
                ability: TRANSMUTE_ID,
                mana_cost: cost,
            } if Some(*object) == source => *mana_cost = Some(cost.clone()),
            GameEvent::AbilityActivated {
                player: PlayerId(0),
                source: object,
                source_incarnation: incarnation,
                definition: CARD_ID,
                ability: TRANSMUTE_ID,
            } if Some(*object) == source => *source_incarnation = Some(*incarnation),
            GameEvent::AbilityResolved {
                source: object,
                source_incarnation: incarnation,
                ability: TRANSMUTE_ID,
            } if Some(*object) == source => {
                *resolved = true;
                *resolved_incarnation = Some(*incarnation);
            }
            GameEvent::DecisionOpened {
                decision: id,
                player: PlayerId(0),
                kind: DecisionKind::LibrarySearch,
                visibility: DecisionVisibility::Private,
                ..
            } => {
                *private_choice = true;
                *decision = Some(id.0);
            }
            GameEvent::DecisionCompleted {
                decision: id,
                player: PlayerId(0),
                kind: DecisionKind::LibrarySearch,
            } if *decision == Some(id.0) => *private_choice &= true,
            GameEvent::CardRevealed {
                player: PlayerId(0),
                card,
                definition,
            } if Some(*card) == selected => *found_definition = Some(definition),
            GameEvent::CardMoved {
                card,
                to: Zone::Hand,
            } if Some(*card) == selected => {
                *found = Some(*card);
                *found_in_hand = true;
            }
            GameEvent::LibrarySearchResolved {
                player: PlayerId(0),
                source: object,
                found: selected_card,
                destination: LibrarySearchDestination::Hand,
            } if Some(*object) == source => *found = *selected_card,
            GameEvent::Transmuted {
                player: PlayerId(0),
                discarded,
                found: selected_card,
            } if Some(*discarded) == source => *found = *selected_card,
            GameEvent::CardMoved {
                card,
                to: Zone::Graveyard,
            } if Some(*card) == source => *source_zone = Some(Zone::Graveyard),
            GameEvent::LibraryShuffled {
                player: PlayerId(0),
                ..
            } if source.is_some() => *shuffled = true,
            _ => {}
        }
    }
    Ok(())
}

struct UsherPolicy {
    player: PlayerId,
    cast_cost: ManaCost,
    activation_cost: ManaCost,
    support_cost: ManaCost,
    transmute_cost: Option<ManaCost>,
    transmute_started: bool,
    transmute_found_usher: Option<ObjectId>,
    activation_offered_turn: Option<u32>,
}

impl UsherPolicy {
    fn new(
        player: PlayerId,
        cast_cost: &ManaCost,
        activation_cost: &ManaCost,
        support_cost: &ManaCost,
        transmute_cost: Option<&ManaCost>,
    ) -> Self {
        Self {
            player,
            cast_cost: cast_cost.clone(),
            activation_cost: activation_cost.clone(),
            support_cost: support_cost.clone(),
            transmute_cost: transmute_cost.cloned(),
            transmute_started: false,
            transmute_found_usher: None,
            activation_offered_turn: None,
        }
    }
    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let land = view
            .hand
            .iter()
            .find(|card| card.card_types.contains(&CardType::Land))?;
        Some(PolicyAction::PlayLand { card: land.id })
    }
    fn tap_for(&self, view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
        let need = cost
            .colored
            .iter()
            .find(|color| view.mana_pool.amount(**color) == 0)
            .copied()
            .or_else(|| {
                (view.mana_pool.total() < cost.generic.saturating_add(cost.colored.len() as u8))
                    .then_some(Color::Blue)
            });
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
    fn make_cast(&self, view: &GameView, id: &str, cost: &ManaCost) -> Option<PolicyAction> {
        let card = self
            .transmute_found_usher
            .and_then(|found| view.hand.iter().find(|card| card.id == found))
            .filter(|card| card.definition == Some(id))
            .or_else(|| view.hand.iter().find(|card| card.definition == Some(id)))?;
        if let Some(mana_selection) = payment(view, cost) {
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
        } else {
            self.tap_for(view, cost)
        }
    }
    fn activate_usher(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.player != PlayerId(0)
            || view.active_player != self.player
            || view.priority != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
            || self.activation_offered_turn == Some(view.turn)
        {
            return None;
        }
        let source = view.own_battlefield.iter().find(|card| {
            card.definition == Some(CARD_ID) && !card.tapped && !card.summoning_sick
        })?;
        let target = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some(SUPPORT_ID) && card.can_attack)?;
        if payment(view, &self.activation_cost).is_some() {
            self.activation_offered_turn = Some(view.turn);
            Some(PolicyAction::ActivateAbility {
                activation: AbilityActivation {
                    source: source.id,
                    ability_id: ABILITY_ID,
                    sacrifice_sources: vec![],
                    additional_tap_creatures: vec![],
                    discard_cards: vec![],
                    targets: vec![Target::Permanent(target.id)],
                },
            })
        } else {
            self.tap_for(view, &self.activation_cost)
        }
    }
    fn declare_blocks(&self, view: &GameView) -> PolicyAction {
        let blockers: Vec<_> = view
            .own_battlefield
            .iter()
            .filter(|card| card.can_block)
            .collect();
        let assignments = view
            .combat_attackers
            .iter()
            .filter(|attacker| !attacker.keywords.contains(&Keyword::Unblockable))
            .zip(blockers)
            .map(|(attacker, blocker)| cardbench_magic_engine::CombatBlock {
                attacker: attacker.id,
                blocker: blocker.id,
            })
            .collect();
        PolicyAction::DeclareBlockers { assignments }
    }
}

impl CodePolicy for UsherPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.usher-dancer-unblockable.v1"
        } else {
            "rav.watchwolf-defender-block-policy.v1"
        }
    }
    fn propose_library_search_choice(&mut self, view: &GameView) -> PolicyAction {
        let choice = view
            .library_search_choice
            .as_ref()
            .expect("Ethereal Usher Transmute choice is visible to its controller");
        let selected = choice
            .cards
            .iter()
            .find(|card| card.definition == Some(CARD_ID))
            .map(|card| card.id);
        self.transmute_found_usher = selected;
        PolicyAction::ChooseLibrarySearchCard {
            decision: choice.decision,
            source: choice.source,
            selected,
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
                attackers: if self.player == PlayerId(0) {
                    view.own_battlefield
                        .iter()
                        .filter(|card| card.can_attack && card.definition == Some(SUPPORT_ID))
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
            return if self.player == PlayerId(1) {
                self.declare_blocks(view)
            } else {
                PolicyAction::DeclareBlockers {
                    assignments: vec![],
                }
            };
        }
        if view.priority != self.player
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.play_land(view) {
            return action;
        }
        if self.player == PlayerId(0) {
            if !self.transmute_started
                && let Some(transmute_cost) = &self.transmute_cost
                && let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
            {
                if payment(view, transmute_cost).is_some() {
                    self.transmute_started = true;
                    return PolicyAction::Transmute { card: card.id };
                }
                if let Some(action) = self.tap_for(view, transmute_cost) {
                    return action;
                }
            }
            if let Some(action) = self.activate_usher(view) {
                return action;
            }
            if !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(CARD_ID))
            {
                if let Some(action) = self.make_cast(view, CARD_ID, &self.cast_cost) {
                    return action;
                }
            }
            let number = view
                .own_battlefield
                .iter()
                .filter(|card| card.definition == Some(SUPPORT_ID))
                .count();
            if number < 1 {
                if let Some(action) = self.make_cast(view, SUPPORT_ID, &self.support_cost) {
                    return action;
                }
            }
        } else {
            let number = view
                .own_battlefield
                .iter()
                .filter(|card| card.definition == Some(BLOCKER_ID))
                .count();
            if number < 4 {
                if let Some(action) = self.make_cast(view, BLOCKER_ID, &self.cast_cost) {
                    return action;
                }
            }
        }
        PolicyAction::PassPriority
    }
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

fn validate_trace(c: &Captured, seed: u64) -> Result<Value, String> {
    if c.terminal != "Winner(PlayerId(0))" || c.winner != Some(0) || c.rejected != 0 {
        return Err("Ethereal Usher match did not reach a clean seat-zero terminal win".into());
    }
    let transmute_source = c
        .transmute_source
        .ok_or("Ethereal Usher Transmute was not activated")?;
    let found = c
        .transmute_found
        .ok_or("Ethereal Usher Transmute did not find a card")?;
    let usher = c.usher.ok_or("seat zero did not cast Ethereal Usher")?;
    let target = c
        .target
        .ok_or("Ethereal Usher did not grant unblockability to a Watchwolf")?;
    let expected_transmute = ManaCost::with_colors(1, [Color::Blue, Color::Blue]);
    let expected_cast = ManaCost::with_colors(5, [Color::Blue]);
    let expected_activation = ManaCost::with_colors(0, [Color::Blue]);
    if transmute_source == found
        || c.transmute_selected != Some(found)
        || c.transmute_found_definition != Some(CARD_ID)
        || !c.transmute_found_in_hand
        || !c.transmute_resolved
        || !c.transmute_shuffled
        || !c.transmute_private_choice
        || !c.transmute_hidden_from_opponent
        || !c.transmute_timing_valid
        || c.transmute_turn.is_none()
        || c.transmute_source_zone != Some(Zone::Graveyard)
        || c.transmute_source_incarnation.is_none()
        || c.transmute_source_incarnation != c.transmute_resolved_incarnation
        || c.transmute_cost.as_ref() != Some(&expected_transmute)
        || usher != found
    {
        return Err(format!(
            "Ethereal Usher Transmute identity/timing/payment/search failed: source={transmute_source:?} found={found:?} selected={:?} def={:?} cost={:?} hand={} resolved={} shuffled={} private={} hidden={} timing={} zone={:?}",
            c.transmute_selected,
            c.transmute_found_definition,
            c.transmute_cost,
            c.transmute_found_in_hand,
            c.transmute_resolved,
            c.transmute_shuffled,
            c.transmute_private_choice,
            c.transmute_hidden_from_opponent,
            c.transmute_timing_valid,
            c.transmute_source_zone
        ));
    }
    if c.definition.id != CARD_ID
        || c.definition.name != "Ethereal Usher"
        || c.definition.mana_cost != expected_cast
        || c.definition.colors != BTreeSet::from([Color::Blue])
        || c.definition.power != Some(2)
        || c.definition.toughness != Some(3)
        || c.definition.keywords != [Keyword::Transmute(expected_transmute.clone())]
        || c.cast_cost != expected_cast
        || c.cast_payment.len() != 6
        || !c.cast_payment.contains(&Color::Blue)
    {
        return Err(format!(
            "Ethereal Usher printed cost/body/Transmute differs: def={:?} cast={} paid={:?}",
            c.definition, c.cast_cost, c.cast_payment
        ));
    }
    if c.ability.id != ABILITY_ID
        || c.ability.mana_cost != expected_activation
        || !c.ability.tap_cost
        || c.ability.sorcery_speed
        || c.ability.additional_tap_creatures != 0
        || c.ability.sacrifice_source
        || c.ability.sacrifice_creatures != 0
        || c.ability.sacrifice_lands != 0
        || c.ability.discard_cards != 0
        || c.ability.targets != [TargetRequirement::Creature]
        || c.ability.effects
            != [Effect::ModifyTargetKeywordUntilEndOfTurn {
                keyword: Keyword::Unblockable,
            }]
    {
        return Err(format!(
            "Ethereal Usher's exact {{U}}, tap-target unblockable activation is wrong: {:?}",
            c.ability
        ));
    }
    if c.ability_turn.is_none()
        || c.attack_turn != c.ability_turn
        || !c.ability_source_tapped
        || !c.unblockable_at_attack
        || !c
            .target_keywords
            .iter()
            .any(|keyword| keyword == "Unblockable")
        || c.illegal_block_rejection.as_deref() != Some("unblockable attacker cannot be blocked")
        || c.target_blocked
        || !c.expiry_observed
        || c.target_zone != Some(Zone::Battlefield)
        || c.usher_zone != Some(Zone::Battlefield)
        || c.target_damage < 20
        || c.life[1] > 0
        || c.life[0] <= 0
    {
        return Err(format!(
            "Ethereal Usher target/combat/duration trace failed: target={target:?} keywords={:?} attack={:?}/{:?} unblockable={} tapped={} block={:?} blocked={} expired={} damage={} life={:?}",
            c.target_keywords,
            c.ability_turn,
            c.attack_turn,
            c.unblockable_at_attack,
            c.ability_source_tapped,
            c.illegal_block_rejection,
            c.target_blocked,
            c.expiry_observed,
            c.target_damage,
            c.life
        ));
    }

    let events = &c.events;
    let transmute_paid = find_event(events, |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: TRANSMUTE_ID, mana_cost } if *source == transmute_source && *mana_cost == expected_transmute)).ok_or("Ethereal Usher exact Transmute mana receipt missing")?;
    let transmute_discarded = find_event_after(events, transmute_paid, |event| matches!(event, GameEvent::DiscardedAsAbilityCost { player: PlayerId(0), source, card } if *source == transmute_source && *card == transmute_source)).ok_or("Ethereal Usher Transmute discard cost receipt missing")?;
    let transmute_source_moved = find_event_after(events, transmute_discarded, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == transmute_source)).ok_or("Ethereal Usher Transmute source did not move to its graveyard")?;
    let transmute_activated = find_event_after(events, transmute_source_moved, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source, definition: CARD_ID, ability: TRANSMUTE_ID, .. } if *source == transmute_source)).ok_or("Ethereal Usher Transmute activation receipt missing")?;
    let decision_id = c
        .transmute_decision
        .ok_or("Ethereal Usher private search decision id missing")?;
    let decision_opened = find_event_after(events, transmute_activated, |event| matches!(event, GameEvent::DecisionOpened { decision, player: PlayerId(0), kind: DecisionKind::LibrarySearch, visibility: DecisionVisibility::Private, .. } if decision.0 == decision_id)).ok_or("Ethereal Usher private library search did not open")?;
    let decision_completed = find_event_after(events, decision_opened, |event| matches!(event, GameEvent::DecisionCompleted { decision, player: PlayerId(0), kind: DecisionKind::LibrarySearch } if decision.0 == decision_id)).ok_or("Ethereal Usher library-search decision did not complete")?;
    let revealed = find_event_after(events, decision_completed, |event| matches!(event, GameEvent::CardRevealed { player: PlayerId(0), card, definition: CARD_ID } if *card == found)).ok_or("Ethereal Usher searched card was not revealed")?;
    let moved_to_hand = find_event_after(
        events,
        revealed,
        |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == found),
    )
    .ok_or("Ethereal Usher searched object did not move to hand")?;
    let search_resolved = find_event_after(events, moved_to_hand, |event| matches!(event, GameEvent::LibrarySearchResolved { player: PlayerId(0), source, found: Some(selected), destination: LibrarySearchDestination::Hand } if *source == transmute_source && *selected == found)).ok_or("Ethereal Usher exact search result receipt missing")?;
    let shuffled = find_event_after(events, search_resolved, |event| {
        matches!(
            event,
            GameEvent::LibraryShuffled {
                player: PlayerId(0),
                ..
            }
        )
    })
    .ok_or("Ethereal Usher Transmute did not shuffle the library")?;
    let transmuted = find_event_after(events, shuffled, |event| matches!(event, GameEvent::Transmuted { player: PlayerId(0), discarded, found: Some(selected) } if *discarded == transmute_source && *selected == found)).ok_or("Ethereal Usher Transmuted receipt missing")?;
    let transmute_resolved = find_event_after(events, transmuted, |event| matches!(event, GameEvent::AbilityResolved { source, ability: TRANSMUTE_ID, .. } if *source == transmute_source)).ok_or("Ethereal Usher Transmute ability did not resolve")?;
    let cast_payment = find_event_after(events, transmute_resolved, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == usher && colors.len() == 6 && colors.contains(&Color::Blue))).ok_or("Ethereal Usher exact {5}{U} spell payment receipt missing")?;
    let cast = find_event_after(events, cast_payment, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == usher)).ok_or("Ethereal Usher cast receipt missing")?;
    let cast_resolved = find_event_after(
        events,
        cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == usher),
    )
    .ok_or("Ethereal Usher spell did not resolve")?;
    let entered = find_event_after(events, cast_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == usher)).ok_or("Ethereal Usher did not enter the battlefield")?;
    let ability_paid = find_event_after(events, entered, |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: ABILITY_ID, mana_cost } if *source == usher && *mana_cost == expected_activation)).ok_or("Ethereal Usher exact {U} ability payment receipt missing")?;
    let activated = find_event_after(events, ability_paid, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source, definition: CARD_ID, ability: ABILITY_ID, .. } if *source == usher)).ok_or("Ethereal Usher unblockable activation receipt missing")?;
    let modifier = find_event_after(events, activated, |event| matches!(event, GameEvent::ContinuousEffectCreated { source, target: affected, layer: Layer::Ability } if *source == usher && *affected == target)).ok_or("Ethereal Usher exact source-to-Watchwolf unblockable effect receipt missing")?;
    let ability_resolved = find_event_after(events, activated, |event| matches!(event, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == usher)).ok_or("Ethereal Usher unblockable ability did not resolve")?;
    let attack = find_event_after(events, ability_resolved, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&target))).ok_or("targeted Watchwolf did not attack after the ability resolved")?;
    let blockers = find_event_after(events, attack, |event| matches!(event, GameEvent::BlockersDeclared { assignments, .. } if !assignments.iter().any(|(attacker, _)| *attacker == target))).ok_or("unblockable Watchwolf was not omitted from legal blockers")?;
    let damage = find_event_after(events, blockers, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 3 } if *source == target)).ok_or("unblockable Watchwolf did not deal direct combat damage")?;
    let expired = find_event_after(events, damage, |event| matches!(event, GameEvent::ContinuousEffectExpired { source, target: affected, layer: Layer::Ability } if *source == usher && *affected == target)).ok_or("Ethereal Usher unblockable effect did not expire")?;
    let ended = find_event_after(events, damage, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal seat-zero victory receipt missing")?;
    let ordered = [
        transmute_paid,
        transmute_discarded,
        transmute_source_moved,
        transmute_activated,
        decision_opened,
        decision_completed,
        revealed,
        moved_to_hand,
        search_resolved,
        shuffled,
        transmuted,
        transmute_resolved,
        cast_payment,
        cast,
        cast_resolved,
        entered,
        ability_paid,
        activated,
        modifier,
        ability_resolved,
        attack,
        blockers,
        damage,
        expired,
        ended,
    ];
    if ordered.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(
            "Ethereal Usher Transmute/cast/activation/combat event order is invalid".into(),
        );
    }

    let projected_all = project_events(events);
    let projected_attack = projected_all
        .iter()
        .find(|event| {
            event.kind == "AttackersDeclared" && event.facts.objects.contains(&ObjectRef(target.0))
        })
        .ok_or("projected target attack missing")?;
    let projected_damage = projected_all
        .iter()
        .find(|event| {
            event.kind == "DamageDealtToPlayer"
                && event.facts.objects.contains(&ObjectRef(target.0))
                && event.facts.seats == [SeatId(1)]
                && event.facts.amount == Some(3)
        })
        .ok_or("projected direct damage receipt missing")?;
    let terminal_index = projected_all
        .iter()
        .rposition(|event| event.kind == "GameEnded" && event.facts.seats == [SeatId(0)])
        .ok_or("projected terminal event missing")?;
    if projected_attack.sequence >= projected_damage.sequence
        || terminal_index >= projected_all.len()
    {
        return Err("projected Ethereal Usher event sequence is invalid".into());
    }
    let selected_events = BTreeSet::from([
        transmute_paid,
        transmute_discarded,
        transmute_source_moved,
        transmute_activated,
        decision_opened,
        decision_completed,
        revealed,
        moved_to_hand,
        search_resolved,
        shuffled,
        transmuted,
        transmute_resolved,
        cast_payment,
        cast,
        cast_resolved,
        entered,
        ability_paid,
        activated,
        modifier,
        ability_resolved,
        attack,
        blockers,
        damage,
        expired,
        ended,
    ]);
    let projected: Vec<ProjectedEvent> = projected_all
        .iter()
        .enumerate()
        .filter_map(|(index, event)| selected_events.contains(&index).then_some(event.clone()))
        .collect();
    Ok(json!({
        "trace_id":"rav-ethereal-usher-transmute-unblockable-terminal-pvp",
        "shuffle_seed":seed,
        "decks":["rav_ethereal_usher_azorius_watchwolf","rav_watchwolf_defender_selesnya"],
        "policies":["rav.ethereal-usher-transmute-unblockable.v1","rav.watchwolf-defender-block-policy.v1"],
        "terminal":c.terminal,"winner_seat":c.winner,"turns":c.turns,"accepted_moves":c.accepted,"rejected_moves":c.rejected,
        "ethereal_usher":{"object":usher.0,"collector_number":47,"mana_cost":"{5}{U}","mana_paid":c.cast_payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"base_power":2,"base_toughness":3,"subtype_asserted":false,"zone_at_terminal":c.usher_zone.map(|zone|format!("{zone:?}")).unwrap_or_default()},
        "transmute":{"source":transmute_source.0,"source_definition":CARD_ID,"source_incarnation":c.transmute_source_incarnation,"mana_cost":"{1}{U}{U}","activation_turn":c.transmute_turn,"sorcery_timing":c.transmute_timing_valid,"selected":found.0,"found_definition":c.transmute_found_definition,"revealed":true,"destination":"Hand","source_zone_after_cost":format!("{:?}",c.transmute_source_zone),"private_choice":c.transmute_private_choice,"hidden_from_opponent":c.transmute_hidden_from_opponent,"library_shuffled":c.transmute_shuffled,"resolved":c.transmute_resolved,"same_found_object_cast":usher==found},
        "activation":{"ability":ABILITY_ID,"source":usher.0,"mana_cost":"{U}","tap_cost_paid":c.ability_source_tapped,"target":target.0,"target_definition":SUPPORT_ID,"target_keywords":c.target_keywords,"resolved_turn":c.ability_turn,"attack_turn":c.attack_turn,"unblockable_at_attack":c.unblockable_at_attack,"unblockable_expired_next_turn":c.expiry_observed,"illegal_block_diagnostic":c.illegal_block_rejection,"target_was_blocked":c.target_blocked},
        "combat":{"target_watchwolf":target.0,"direct_damage_total":c.target_damage,"target_zone_at_terminal":c.target_zone.map(|zone|format!("{zone:?}")).unwrap_or_default(),"opponent_life":c.life[1],"life_at_terminal":c.life},
        "ordered_event_indices":ordered,"events":projected,"passed":true
    }))
}

fn find_event(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn find_event_after(
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
