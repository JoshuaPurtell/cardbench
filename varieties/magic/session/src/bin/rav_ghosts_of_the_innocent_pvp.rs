//! Full-game RAV Ghosts of the Innocent damage-replacement authority.

use std::collections::{BTreeSet, HashMap};
use std::process::ExitCode;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, CombatBlock, DamageReplacementEffect,
    DamageReplacementEffectBinding, DeckEntry, DeckList, Game, GameEvent, GameView, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    card_definitions, rav_basic_land_type_bindings, rav_damage_replacement_effect_bindings,
    rav_main_set_catalog, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};
use serde_json::{Value, json};
use syn::UseTree;

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-GHOSTS-OF-THE-INNOCENT";
const CHAR_ID: &str = "RAV-CHAR";
const WOLF_ID: &str = "RAV-WATCHWOLF";
const MAX_SEED: u64 = 64;
const MAX_TURNS: u32 = 55;
const MAX_POLICY_MOVES: u32 = 8_000;

// The only executable content accepted from a candidate module is this two-
// constructor declarative record. Comparing normalized source prevents field
// expressions from gaining side effects inside an otherwise direct literal.
const SOURCE_LITERAL_CONTRACT: &str = r#"
use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DamageReplacementEffect, DamageReplacementEffectBinding,
    ManaCost,
};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GHOSTS-OF-THE-INNOCENT",
        name: "Ghosts of the Innocent",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(5, [Color::White, Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "static-global-damage-amount-halving",
            "affected-player-damage-replacement-ordering",
        ],
        power: Some(4),
        toughness: Some(5),
        keywords: vec![],
        effects: vec![],
    }
}

#[must_use]
pub(crate) fn damage_replacement_binding() -> DamageReplacementEffectBinding {
    DamageReplacementEffectBinding {
        source_definition: "RAV-GHOSTS-OF-THE-INNOCENT",
        effect: DamageReplacementEffect::HalveDamage,
    }
}
"#;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/ghosts_of_the_innocent.rs"]
mod candidate_ghosts;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Ghosts of the Innocent result: {error}");
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
            eprintln!("Ghosts of the Innocent PvP authority failed: {error}");
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
    WrongCost,
    WrongToughness,
    MissingBinding,
    WrongReplacement,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Packet {
    source: ObjectId,
    target: Target,
    original: i32,
    replacement: i32,
    replacement_index: usize,
    committed_index: usize,
    committed_amount: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CastObservation {
    spell: ObjectId,
    target: Option<Target>,
    payment: Vec<Color>,
    paid_index: usize,
    cast_index: usize,
    resolved_index: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    terminal: String,
    turns: u32,
    accepted: u32,
    rejected: u32,
    life: [i64; 2],
    ghost: Option<ObjectId>,
    wolf: Option<ObjectId>,
    ghost_generic_cost: Option<u8>,
    ghost_colored_cost: Vec<Color>,
    ghost_power: Option<i32>,
    ghost_toughness: Option<i32>,
    ghost_cast: Option<CastObservation>,
    chars: Vec<CastObservation>,
    packets: Vec<Packet>,
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: "Ghosts of the Innocent",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(5, [Color::White, Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "static-global-damage-amount-halving",
            "affected-player-damage-replacement-ordering",
        ],
        power: Some(4),
        toughness: Some(5),
        keywords: vec![],
        effects: vec![],
    }
}

fn expected_binding() -> DamageReplacementEffectBinding {
    DamageReplacementEffectBinding {
        source_definition: CARD_ID,
        effect: DamageReplacementEffect::HalveDamage,
    }
}

fn validate_candidate_source() -> Result<(), String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../sets/ravnica_city_of_guilds/src/cards/ghosts_of_the_innocent.rs");
    let source = std::fs::read_to_string(path)
        .map_err(|error| format!("could not read Ghosts candidate source: {error}"))?;
    let uncommented = source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    let compact_actual: String = uncommented
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let compact_contract: String = SOURCE_LITERAL_CONTRACT
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    if compact_actual != compact_contract {
        return Err(
            "Ghosts of the Innocent candidate definition is outside its literal source contract"
                .into(),
        );
    }
    let file = syn::parse_file(&source)
        .map_err(|_| "Ghosts candidate source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
    {
        return Err("Ghosts candidate module has an executable module attribute".into());
    }
    let expected_imports: BTreeSet<String> = [
        "std::collections::BTreeSet",
        "cardbench_magic_engine::CardDefinition",
        "cardbench_magic_engine::CardType",
        "cardbench_magic_engine::Color",
        "cardbench_magic_engine::DamageReplacementEffect",
        "cardbench_magic_engine::DamageReplacementEffectBinding",
        "cardbench_magic_engine::ManaCost",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let mut actual_imports = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            syn::Item::Use(import)
                if import.attrs.is_empty() && matches!(&import.vis, syn::Visibility::Inherited) =>
            {
                let mut paths = Vec::new();
                if import.leading_colon.is_some()
                    || !flatten_use_tree(&[], &import.tree, &mut paths)
                {
                    return Err("Ghosts candidate module contains an unsupported import".into());
                }
                actual_imports.extend(paths.into_iter().map(|path| path.join("::")));
            }
            syn::Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                let (return_type, struct_type) = match name.as_str() {
                    "definition" => ("CardDefinition", "CardDefinition"),
                    "damage_replacement_binding" => (
                        "DamageReplacementEffectBinding",
                        "DamageReplacementEffectBinding",
                    ),
                    _ => return Err("Ghosts candidate module contains an extra function".into()),
                };
                let valid_attribute = function
                    .attrs
                    .iter()
                    .filter(|attribute| attribute.path().is_ident("must_use"))
                    .count()
                    == 1
                    && function.attrs.iter().all(|attribute| {
                        attribute.path().is_ident("doc")
                            || (attribute.path().is_ident("must_use")
                                && matches!(&attribute.meta, syn::Meta::Path(_)))
                    });
                let valid_return = matches!(&function.sig.output,
                    syn::ReturnType::Type(_, ty)
                        if matches!(ty.as_ref(), syn::Type::Path(path)
                            if path.qself.is_none() && path.path.is_ident(return_type)));
                let direct_constructor = function.block.stmts.len() == 1
                    && matches!(&function.block.stmts[0], syn::Stmt::Expr(syn::Expr::Struct(value), None)
                        if value.path.is_ident(struct_type) && value.rest.is_none());
                if !valid_attribute
                    || !matches!(&function.vis, syn::Visibility::Restricted(visibility) if visibility.path.is_ident("crate"))
                    || !function.sig.inputs.is_empty()
                    || function.sig.constness.is_some()
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || function.sig.abi.is_some()
                    || function.sig.variadic.is_some()
                    || !function.sig.generics.params.is_empty()
                    || function.sig.generics.where_clause.is_some()
                    || !valid_return
                    || !direct_constructor
                {
                    return Err(
                        "Ghosts candidate functions must be direct literal constructors".into(),
                    );
                }
                if !functions.insert(name) {
                    return Err("Ghosts candidate module repeats a constructor".into());
                }
            }
            _ => {
                return Err(
                    "Ghosts candidate module contains an unsupported top-level item".into(),
                );
            }
        }
    }
    let compact: String = source
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let required = [
        "id:\"RAV-GHOSTS-OF-THE-INNOCENT\"",
        "name:\"GhostsoftheInnocent\"",
        "mana_cost:ManaCost::with_colors(5,[Color::White,Color::White])",
        "card_types:BTreeSet::from([CardType::Creature])",
        "power:Some(4)",
        "toughness:Some(5)",
        "effect:DamageReplacementEffect::HalveDamage",
    ];
    if actual_imports != expected_imports
        || functions
            != BTreeSet::from([
                "definition".to_owned(),
                "damage_replacement_binding".to_owned(),
            ])
        || required.iter().any(|fragment| !compact.contains(fragment))
    {
        return Err(
            "Ghosts of the Innocent candidate definition is outside its literal source contract"
                .into(),
        );
    }
    Ok(())
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
        UseTree::Glob(_) | UseTree::Rename(_) => false,
    }
}

fn validate_candidate_runtime() -> Result<(), String> {
    let candidate = candidate_ghosts::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Ghosts of the Innocent is missing from the RAV definition registry")?;
    let registered_bindings = rav_damage_replacement_effect_bindings();
    if candidate != expected_definition()
        || candidate != registered
        || candidate_ghosts::damage_replacement_binding() != expected_binding()
        || registered_bindings
            .iter()
            .filter(|binding| binding.source_definition == CARD_ID)
            .count()
            != 1
        || registered_bindings
            .iter()
            .find(|binding| binding.source_definition == CARD_ID)
            != Some(&candidate_ghosts::damage_replacement_binding())
    {
        return Err("Ghosts candidate definition or damage binding differs from the exact RAV card contract".into());
    }
    Ok(())
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
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
        || !catalog
            .iter()
            .any(|card| card.collector_number == 20 && card.name == "Ghosts of the Innocent")
    {
        return Err("RAV catalog denominator or Ghosts of the Innocent identity differs".into());
    }

    let (seed, capture) = find_trace()?;
    let trace = validate_trace(&capture, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if capture.events != replay.events || capture.winner != replay.winner {
        return Err("Ghosts of the Innocent full-game replay differs".into());
    }
    let mutations = json!({
        "printed_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongCost),
        "printed_toughness_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongToughness),
        "missing_binding_mutation_detected_in_game": mutation_detected(seed, Mutation::MissingBinding),
        "wrong_replacement_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongReplacement),
        "sealed_mutation_authority": false
    });
    for key in [
        "printed_cost_mutation_detected_in_game",
        "printed_toughness_mutation_detected_in_game",
        "missing_binding_mutation_detected_in_game",
        "wrong_replacement_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!("Ghosts semantic mutation escaped: {key}"));
        }
    }

    let definitions = card_definitions();
    let breakdown: Vec<_> = catalog.iter().map(|card| {
        let id = definitions.iter().find(|definition| definition.name == card.name).map(|definition| definition.id).unwrap_or("");
        json!({
            "collector_number": card.collector_number,
            "name": card.name,
            "definition_id": id,
            "engine_status": if card.semantic_status.is_executable() { "executable_compatibility_slice" } else { "catalog_only" },
            "trace_status": if id == CARD_ID { "passed" } else { "not_qualified" },
            "qualification": if id == CARD_ID { "unreviewed_diagnostic" } else { "not_evaluated" },
            "credited": false
        })
    }).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 20, "definition_id": CARD_ID, "name": "Ghosts of the Innocent"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "authority_cutoff": "2026-09-24",
            "source_contract": "RAV Ghosts of the Innocent #20 costs {5}{W}{W}; it is a 4/5 white creature; each source's damage to a permanent or player is halved, rounded down.",
            "checked_sections": ["120 Damage", "400 Zones", "601 Casting spells", "608 Resolving spells", "614 Replacement effects", "510 Combat damage", "704 State-based actions"],
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "unreviewed_diagnostic",
        "deterministic_replay": true,
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(raw) = std::env::var("CARDBENCH_GHOSTS_OF_THE_INNOCENT_SEED") {
        let seed = raw
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_GHOSTS_OF_THE_INNOCENT_SEED must be an unsigned integer")?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    for seed in 0..=MAX_SEED {
        if let Ok(capture) = run_match(seed, Mutation::None)
            && validate_trace(&capture, seed).is_ok()
        {
            return Ok((seed, capture));
        }
    }
    Err(format!(
        "no clean Ghosts of the Innocent PvP trace found for seeds 0..={MAX_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation) {
        Ok(capture) => validate_trace(&capture, seed).is_err(),
        Err(_) => true,
    }
}

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let target = definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Ghosts of the Innocent is missing from the RAV registry")?;
    match mutation {
        Mutation::None | Mutation::MissingBinding | Mutation::WrongReplacement => {}
        Mutation::WrongCost => {
            target.mana_cost = ManaCost::with_colors(4, [Color::White, Color::White])
        }
        Mutation::WrongToughness => target.toughness = Some(4),
    }
    let mut bindings = rav_damage_replacement_effect_bindings();
    match mutation {
        Mutation::MissingBinding => bindings.retain(|binding| binding.source_definition != CARD_ID),
        Mutation::WrongReplacement => {
            if let Some(binding) = bindings
                .iter_mut()
                .find(|binding| binding.source_definition == CARD_ID)
            {
                binding.effect =
                    DamageReplacementEffect::ReplaceCombatDamageToPlayerWithMillAndCounters;
            }
        }
        _ => {}
    }
    let mut game = Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        vec![],
        vec![],
        rav_triggered_ability_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_damage_replacement_effect_bindings(bindings)
        .map_err(|error| error.to_string())?;
    Ok(game)
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut game = new_game(mutation)?;
    let p0_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: WOLF_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 32,
            },
            DeckEntry {
                card: "RAV-FOREST".into(),
                count: 20,
            },
        ],
        sideboard: vec![],
    };
    let p1_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CHAR_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".into(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 28,
            },
        ],
        sideboard: vec![],
    };
    if [p0_deck.mainboard.as_slice(), p1_deck.mainboard.as_slice()]
        .into_iter()
        .any(|board| {
            board
                .iter()
                .map(|entry| usize::from(entry.count))
                .sum::<usize>()
                != 60
        })
    {
        return Err("Ghosts PvP decks must each contain exactly 60 cards".into());
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_POLICY_MOVES,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &p0_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &p1_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies = [GhostPolicy::new(PlayerId(0)), GhostPolicy::new(PlayerId(1))];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut payments = HashMap::<ObjectId, (Vec<Color>, usize)>::new();
    let mut ghost = None;
    let mut wolf = None;
    let mut ghost_generic_cost = None;
    let mut ghost_colored_cost = Vec::new();
    let mut ghost_power = None;
    let mut ghost_toughness = None;
    let mut ghost_cast = None;
    let mut chars = Vec::<CastObservation>::new();
    let mut packets = Vec::<Packet>::new();
    let mut open_replacements = HashMap::<(ObjectId, Target, i32), (usize, ObjectId, i32)>::new();
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(format!("Ghosts duel exceeded bounds at turn {}", game.turn));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Ghosts policy seat missing")?;
        let action = if view.pending_decision.is_some() {
            policy
                .propose_pending_decision(&view)
                .ok_or("Ghosts policy could not answer a mandatory decision")?
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
        let action_debug = format!("{action:?}");
        let event_start = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Ghosts policy move rejected at turn {} {:?}, action={action_debug}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("Ghosts game invariant failed: {error}"))?;
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
                    payments.insert(*card, (colors.clone(), index));
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(1),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CHAR_ID) =>
                {
                    payments.insert(*card, (colors.clone(), index));
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    let (payment, paid_index) = payments
                        .get(card)
                        .cloned()
                        .ok_or("Ghosts payment receipt missing")?;
                    let stack = game
                        .stack
                        .iter()
                        .find(|item| item.card == *card)
                        .ok_or("Ghosts spell has no stack object")?;
                    let target = stack.targets.first().copied();
                    let printed_cost = &game
                        .card_definition(*card)
                        .map_err(|error| error.to_string())?
                        .mana_cost;
                    ghost_generic_cost = Some(printed_cost.generic);
                    ghost_colored_cost = printed_cost.colored.clone();
                    ghost_cast = Some(CastObservation {
                        spell: *card,
                        target,
                        payment,
                        paid_index,
                        cast_index: index,
                        resolved_index: None,
                    });
                }
                GameEvent::SpellCast {
                    player: PlayerId(1),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CHAR_ID) =>
                {
                    let (payment, paid_index) = payments
                        .get(card)
                        .cloned()
                        .ok_or("Char payment receipt missing")?;
                    let stack = game
                        .stack
                        .iter()
                        .find(|item| item.card == *card)
                        .ok_or("Char spell has no stack object")?;
                    let target = Some(stack.targets.first().copied().ok_or("Char has no target")?);
                    chars.push(CastObservation {
                        spell: *card,
                        target,
                        payment,
                        paid_index,
                        cast_index: index,
                        resolved_index: None,
                    });
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Battlefield,
                } => {
                    if game
                        .card_definition(*card)
                        .is_ok_and(|definition| definition.id == CARD_ID)
                    {
                        ghost.get_or_insert(*card);
                        if let Ok(characteristics) = game.characteristics(*card) {
                            ghost_power = characteristics.power;
                            ghost_toughness = characteristics.toughness;
                        }
                    }
                    if game
                        .card_definition(*card)
                        .is_ok_and(|definition| definition.id == WOLF_ID)
                        && game
                            .controller_of(*card)
                            .is_ok_and(|controller| controller == PlayerId(0))
                    {
                        wolf.get_or_insert(*card);
                    }
                }
                GameEvent::DamageAmountReplaced {
                    source,
                    target,
                    replacement_source,
                    original_amount,
                    replacement_amount,
                    ..
                } if Some(*replacement_source) == ghost
                    || game
                        .card_definition(*replacement_source)
                        .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    open_replacements.insert(
                        (*source, *target, *replacement_amount),
                        (index, *replacement_source, *original_amount),
                    );
                }
                GameEvent::DamageDealtToPermanent {
                    source,
                    permanent,
                    amount,
                } => {
                    let target = Target::Permanent(*permanent);
                    if let Some(key) = open_replacements
                        .keys()
                        .find(|(candidate, target_key, replacement)| {
                            *candidate == *source
                                && *target_key == target
                                && *replacement == *amount
                        })
                        .copied()
                    {
                        let (replacement_index, replacement_source, original) = open_replacements
                            .remove(&key)
                            .expect("matched replacement exists");
                        if Some(replacement_source) == ghost {
                            packets.push(Packet {
                                source: *source,
                                target,
                                original,
                                replacement: key.2,
                                replacement_index,
                                committed_index: index,
                                committed_amount: *amount,
                            });
                        }
                    }
                }
                GameEvent::DamageDealtToPlayer {
                    source,
                    player: target_player,
                    amount,
                } => {
                    let target = Target::Player(*target_player);
                    if let Some(key) = open_replacements
                        .keys()
                        .find(|(candidate, target_key, replacement)| {
                            *candidate == *source
                                && *target_key == target
                                && *replacement == *amount
                        })
                        .copied()
                    {
                        let (replacement_index, replacement_source, original) = open_replacements
                            .remove(&key)
                            .expect("matched replacement exists");
                        if Some(replacement_source) == ghost {
                            packets.push(Packet {
                                source: *source,
                                target,
                                original,
                                replacement: key.2,
                                replacement_index,
                                committed_index: index,
                                committed_amount: *amount,
                            });
                        }
                    }
                }
                GameEvent::SpellResolved { card } => {
                    if let Some(observation) = ghost_cast.as_mut().filter(|observation| {
                        observation.spell == *card && observation.resolved_index.is_none()
                    }) {
                        observation.resolved_index = Some(index);
                    }
                    if let Some(observation) = chars.iter_mut().find(|observation| {
                        observation.spell == *card && observation.resolved_index.is_none()
                    }) {
                        observation.resolved_index = Some(index);
                    }
                }
                _ => {}
            }
        }
        for observer in &mut policies {
            observer.observe_events(&game.event_log[event_start..], &game)?;
        }
    }
    Ok(Captured {
        events: game.event_log.clone(),
        winner: game.winner().map(|seat| seat.0),
        terminal: game.winner().map_or_else(
            || "Draw".into(),
            |seat| format!("Winner(PlayerId({}))", seat.0),
        ),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        life: [
            game.player(PlayerId(0))
                .map_err(|error| error.to_string())?
                .life,
            game.player(PlayerId(1))
                .map_err(|error| error.to_string())?
                .life,
        ],
        ghost,
        wolf,
        ghost_generic_cost,
        ghost_colored_cost,
        ghost_power,
        ghost_toughness,
        ghost_cast,
        chars,
        packets,
    })
}

struct GhostPolicy {
    player: PlayerId,
    chars_cast: usize,
}

impl GhostPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            chars_cast: 0,
        }
    }

    fn observe_events(&mut self, events: &[GameEvent], game: &Game) -> Result<(), String> {
        for event in events {
            if let GameEvent::SpellCast { player, card } = event {
                if *player == PlayerId(1)
                    && game
                        .card_definition(*card)
                        .is_ok_and(|definition| definition.id == CHAR_ID)
                {
                    self.chars_cast += 1;
                }
            }
        }
        Ok(())
    }

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let count_sources = |color| {
            view.own_battlefield
                .iter()
                .filter(|field| {
                    field.card_types.contains(&CardType::Land) && field.mana_colors.contains(&color)
                })
                .count()
        };
        let preferred = if self.player == PlayerId(0) {
            if count_sources(Color::White) < 2 {
                Color::White
            } else if count_sources(Color::Green) < 1 {
                Color::Green
            } else {
                Color::White
            }
        } else if count_sources(Color::Red) < 4 {
            Color::Red
        } else {
            Color::Red
        };
        view.hand
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&preferred)
            })
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            })
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }

    fn make_mana(&self, view: &GameView, preferred: Color) -> Option<PolicyAction> {
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

    fn cast(
        &self,
        view: &GameView,
        definition: &str,
        cost: &ManaCost,
        target: Option<Target>,
        generic_color: Color,
    ) -> Option<PolicyAction> {
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(definition))?;
        let selection = ManaPaymentSelection {
            generic: vec![generic_color; usize::from(cost.generic)],
            hybrid: vec![],
        };
        if let Some(missing) = missing_payment_color(view, cost, &selection) {
            return self.make_mana(view, missing);
        }
        Some(PolicyAction::CastWithPayment {
            request: CastRequest {
                card: card.id,
                targets: target.into_iter().collect(),
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: selection,
        })
    }
}

impl CodePolicy for GhostPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.ghosts-of-the-innocent-pilot.v1"
        } else {
            "rav.ghosts-of-the-innocent-opponent.v1"
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
            let attackers = if self.player == PlayerId(0) {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.can_attack)
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
            let assignments = if self.player == PlayerId(1) {
                view.own_battlefield
                    .iter()
                    .find(|card| card.definition == Some(WOLF_ID) && card.can_block)
                    .and_then(|blocker| {
                        view.combat_attackers
                            .iter()
                            .find(|attacker| attacker.definition == Some(WOLF_ID))
                            .map(|attacker| CombatBlock {
                                attacker: attacker.id,
                                blocker: blocker.id,
                            })
                    })
                    .into_iter()
                    .collect()
            } else {
                vec![]
            };
            return PolicyAction::DeclareBlockers { assignments };
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
            let wolf_live = view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(WOLF_ID));
            let ghost_live = view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(CARD_ID));
            if !wolf_live {
                let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
                if let Some(action) = self.cast(view, WOLF_ID, &cost, None, Color::White) {
                    return action;
                }
            }
            if !ghost_live {
                let cost = ManaCost::with_colors(5, [Color::White, Color::White]);
                if let Some(action) = self.cast(view, CARD_ID, &cost, None, Color::White) {
                    return action;
                }
            }
        } else {
            let ghost_live = view
                .opponent_battlefield
                .iter()
                .any(|card| card.definition == Some(CARD_ID));
            let wolf = view
                .opponent_battlefield
                .iter()
                .find(|card| card.definition == Some(WOLF_ID));
            if ghost_live
                && let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(CHAR_ID))
            {
                let cost = ManaCost::with_colors(2, [Color::Red]);
                let target = if self.chars_cast == 0 {
                    wolf.map(|wolf| Target::Permanent(wolf.id))
                } else {
                    Some(Target::Player(PlayerId(0)))
                };
                if let Some(target) = target {
                    if let Some(action) = self.cast(view, CHAR_ID, &cost, Some(target), Color::Red)
                    {
                        return action;
                    }
                } else {
                    let _ = card;
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn missing_payment_color(
    view: &GameView,
    cost: &ManaCost,
    selection: &ManaPaymentSelection,
) -> Option<Color> {
    let mut required = std::collections::BTreeMap::<Color, u8>::new();
    for color in &cost.colored {
        *required.entry(*color).or_default() += 1;
    }
    for color in &selection.generic {
        *required.entry(*color).or_default() += 1;
    }
    Color::MANA_ALL
        .into_iter()
        .find(|color| view.mana_pool.amount(*color) < required.get(color).copied().unwrap_or(0))
}

fn validate_trace(capture: &Captured, seed: u64) -> Result<Value, String> {
    if capture.winner != Some(0)
        || capture.terminal != "Winner(PlayerId(0))"
        || capture.rejected != 0
    {
        return Err("Ghosts duel did not reach a clean seat-zero terminal win".into());
    }
    let ghost = capture
        .ghost
        .ok_or("Ghosts never resolved onto the battlefield")?;
    let wolf = capture
        .wolf
        .ok_or("seat zero never controlled a Watchwolf")?;
    let cast = capture
        .ghost_cast
        .as_ref()
        .ok_or("Ghosts cast receipt missing")?;
    let resolved = cast.resolved_index.ok_or("Ghosts did not resolve")?;
    if cast.target.is_some()
        || capture.ghost_generic_cost != Some(5)
        || capture.ghost_colored_cost != [Color::White, Color::White]
        || capture.ghost_power != Some(4)
        || capture.ghost_toughness != Some(5)
        || cast.payment.len() != 7
        || cast
            .payment
            .iter()
            .filter(|color| **color == Color::White)
            .count()
            != 7
        || cast.paid_index >= cast.cast_index
        || cast.cast_index >= resolved
    {
        return Err("Ghosts did not use its printed {5}{W}{W} cost and resolve in order".into());
    }
    let ghost_enter = event_index(&capture.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == ghost)).ok_or("Ghosts battlefield entry receipt missing")?;
    if !(cast.cast_index < resolved && resolved < ghost_enter) {
        return Err(format!(
            "Ghosts cast, resolution, and entry receipts are out of order: cast={}, resolution={resolved}, entry={ghost_enter}",
            cast.cast_index
        ));
    }
    let first_char = capture.chars.first().ok_or("opposing Char did not cast")?;
    let second_char = capture
        .chars
        .get(1)
        .ok_or("opposing Char player-target control did not cast")?;
    if first_char.target != Some(Target::Permanent(wolf))
        || second_char.target != Some(Target::Player(PlayerId(0)))
    {
        return Err("Char did not exercise creature and player target domains".into());
    }
    let expected = [
        (first_char.spell, Target::Permanent(wolf), 4, 2),
        (first_char.spell, Target::Player(PlayerId(1)), 2, 1),
        (second_char.spell, Target::Player(PlayerId(0)), 4, 2),
        (second_char.spell, Target::Player(PlayerId(1)), 2, 1),
        (wolf, Target::Player(PlayerId(1)), 3, 1),
    ];
    let mut selected = BTreeSet::from([ghost_enter, cast.paid_index, cast.cast_index, resolved]);
    let mut packet_json = Vec::new();
    for (source, target, original, replacement) in expected {
        let packet = capture.packets.iter().find(|packet| packet.source == source && packet.target == target && packet.original == original && packet.replacement == replacement)
            .ok_or_else(|| format!("required Ghosts damage packet {source:?} {target:?} {original}->{replacement} missing"))?;
        if packet.replacement_index >= packet.committed_index
            || packet.committed_amount != replacement
        {
            return Err("damage replacement and committed packet ordering differs".into());
        }
        selected.insert(packet.replacement_index);
        selected.insert(packet.committed_index);
        packet_json.push(json!({
            "source_object": packet.source.0,
            "target": format!("{:?}", packet.target),
            "original_amount": packet.original,
            "replacement_amount": packet.replacement,
            "committed_amount": packet.committed_amount,
            "replacement_index": packet.replacement_index,
            "committed_index": packet.committed_index
        }));
    }
    let terminal_index = event_index(&capture.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("Ghosts game has no terminal win receipt")?;
    if capture.life[0] <= 0 || capture.life[1] > 0 || terminal_index <= resolved {
        return Err("Ghosts terminal state or event order is inconsistent".into());
    }
    selected.insert(terminal_index);
    let projected: Vec<_> = cardbench_magic_session::project_events(&capture.events)
        .into_iter()
        .enumerate()
        .filter_map(|(index, event)| selected.contains(&index).then_some(event))
        .collect();
    Ok(json!({
        "id": "rav-ghosts-of-the-innocent-global-damage-halving",
        "seed": seed,
        "decks": ["rav_ghosts_white_midrange", "rav_char_red_control"],
        "policies": ["rav.ghosts-of-the-innocent-pilot.v1", "rav.ghosts-of-the-innocent-opponent.v1"],
        "passed": true,
        "terminal": capture.terminal,
        "winner_seat": capture.winner,
        "turns": capture.turns,
        "accepted_moves": capture.accepted,
        "rejected_moves": capture.rejected,
        "life_at_terminal": capture.life,
        "mechanics": {
            "ghost_object": ghost.0,
            "ghost_mana_cost": "{5}{W}{W}",
            "ghost_payment": cast.payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "ghost_power": capture.ghost_power,
            "ghost_toughness": capture.ghost_toughness,
            "replacement_source": CARD_ID,
            "replacement_packets": packet_json,
            "creature_packet_halved": true,
            "player_packet_halved": true,
            "self_packet_halved": true,
            "combat_packet_halved": true
        },
        "event_indices": {"ghost_entry": ghost_enter, "payment": cast.paid_index, "cast": cast.cast_index, "resolution": resolved, "terminal": terminal_index},
        "events": projected
    }))
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
