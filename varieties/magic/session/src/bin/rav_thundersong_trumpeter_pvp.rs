//! Trusted full-game RAV Thundersong Trumpeter diagnostic authority.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, Color, CombatBlock, DeckEntry, DeckList, Effect, Game, GameEvent, GameView,
    Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target,
    TargetRequirement, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use syn::parse::Parser;
use syn::{Expr, Item, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-THUNDERSONG-TRUMPETER";
const CARD_NAME: &str = "Thundersong Trumpeter";
const ABILITY_ID: &str = "tap-prevent-target-combat";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const TARGET_ID: &str = "RAV-BENEVOLENT-ANCESTOR";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/thundersong_trumpeter.rs";
const MAX_TRACE_SEED: u64 = 128;
const MAX_TURNS: u32 = 50;
const MAX_MOVES: u32 = 5_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/thundersong_trumpeter.rs"]
mod candidate_thundersong_trumpeter;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Thundersong Trumpeter result: {error}");
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
            eprintln!("Thundersong Trumpeter PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    RemoveCombatRestriction,
    TapTargetInstead,
    RemoveTapCost,
    AddWhiteManaCost,
    WrongTarget,
    WrongPower,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    trumpeter: Option<ObjectId>,
    trumpeter_payment: Vec<Color>,
    trumpeter_resolved: bool,
    trumpeter_entered: bool,
    trumpeter_power: Option<i32>,
    trumpeter_toughness: Option<i32>,
    watchwolf: Option<ObjectId>,
    watchwolf_entered: bool,
    target: Option<ObjectId>,
    target_payment: Vec<Color>,
    target_resolved: bool,
    target_entered: bool,
    target_can_block_before_activation: Option<bool>,
    activation_source: Option<ObjectId>,
    activation_target: Option<ObjectId>,
    activation_turn: Option<u32>,
    active_player_at_activation: Option<usize>,
    source_tapped_after_activation: bool,
    activation_count: u32,
    ability_mana_paid: bool,
    ability_resolved: bool,
    restriction_effect_created: bool,
    target_tapped_after_resolution: Option<bool>,
    target_has_combat_restriction_after_resolution: bool,
    target_can_block_at_declaration: Option<bool>,
    target_blocker_assignments: Vec<(ObjectId, ObjectId)>,
    damage_to_opponent: i64,
    life: [i64; 2],
}

impl Default for Captured {
    fn default() -> Self {
        Self {
            events: Vec::new(),
            terminal: String::new(),
            winner: None,
            turns: 0,
            accepted: 0,
            rejected: 0,
            trumpeter: None,
            trumpeter_payment: Vec::new(),
            trumpeter_resolved: false,
            trumpeter_entered: false,
            trumpeter_power: None,
            trumpeter_toughness: None,
            watchwolf: None,
            watchwolf_entered: false,
            target: None,
            target_payment: Vec::new(),
            target_resolved: false,
            target_entered: false,
            target_can_block_before_activation: None,
            activation_source: None,
            activation_target: None,
            activation_turn: None,
            active_player_at_activation: None,
            source_tapped_after_activation: false,
            activation_count: 0,
            ability_mana_paid: false,
            ability_resolved: false,
            restriction_effect_created: false,
            target_tapped_after_resolution: None,
            target_has_combat_restriction_after_resolution: false,
            target_can_block_at_declaration: None,
            target_blocker_assignments: Vec::new(),
            damage_to_opponent: 0,
            life: [20, 20],
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_registry()?;
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured != replay || project_events(&captured.events) != project_events(&replay.events) {
        return Err("Thundersong Trumpeter full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "combat_restriction_removed_detected": mutation_detected(seed, Mutation::RemoveCombatRestriction)?,
        "wrong_tap_effect_detected": mutation_detected(seed, Mutation::TapTargetInstead)?,
        "source_tap_cost_removed_detected": mutation_detected(seed, Mutation::RemoveTapCost)?,
        "wrong_ability_mana_cost_detected": mutation_detected(seed, Mutation::AddWhiteManaCost)?,
        "wrong_target_requirement_detected": mutation_detected(seed, Mutation::WrongTarget)?,
        "wrong_power_detected": mutation_detected(seed, Mutation::WrongPower)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "combat_restriction_removed_detected",
        "wrong_tap_effect_detected",
        "source_tap_cost_removed_detected",
        "wrong_ability_mana_cost_detected",
        "wrong_target_requirement_detected",
        "wrong_power_detected",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Thundersong Trumpeter gameplay mutation escaped: {key}"
            ));
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
        return Err("RAV catalog denominator differs from the source contract".to_owned());
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 235, "printings": 1, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117.1b and 117.3c: priority and activated-ability resolution",
                "302.6: tap-cost abilities and summoning sickness",
                "508.1: declaring attackers",
                "509.1: declaring blockers and legal blockers",
                "602.1-602.2: activated abilities, targets, and costs",
                "608.2: resolving a targeted ability",
                "611.2a and 613: until-end-of-turn continuous effects",
                "702.17: Defender",
                "704.5: state-based actions and terminal game result"
            ],
            "source_contract": "RAV #235 is a red-white 2/1 creature with a tap activation that installs a temporary restriction preventing a target creature from participating in combat.",
            "authority_cutoff": "2026-09-23",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "deterministic_replay": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "diagnostic_reference_trace_passed; independent review and candidate trial pending",
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
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
        "engine_status": if definition_id.is_some() { "executable_compatibility_slice" } else { "catalog_only" },
        "qualification": if passed { "diagnostic_trace_passed; independent review pending" } else { "not_evaluated" },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "credited": false
    })
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}

fn output_root() -> Option<std::path::PathBuf> {
    let mut args = std::env::args_os().skip(1);
    while let Some(argument) = args.next() {
        if argument == "--output-root" {
            return args.next().map(Into::into);
        }
    }
    None
}

fn write_outputs(root: &std::path::Path, report: &Value) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(root.join("reward.txt"), "0.0\n").map_err(|error| error.to_string())
}

fn candidate_values(mutation: Mutation) -> (CardDefinition, ActivatedAbilityBinding) {
    let mut definition = candidate_thundersong_trumpeter::definition();
    let mut binding = candidate_thundersong_trumpeter::activated_ability();
    match mutation {
        Mutation::None
        | Mutation::RemoveCombatRestriction
        | Mutation::TapTargetInstead
        | Mutation::RemoveTapCost
        | Mutation::AddWhiteManaCost
        | Mutation::WrongTarget => {}
        Mutation::WrongPower => definition.power = Some(3),
    }
    match mutation {
        Mutation::None | Mutation::WrongPower => {}
        Mutation::RemoveCombatRestriction => binding.ability.effects.clear(),
        Mutation::TapTargetInstead => binding.ability.effects = vec![Effect::TapTargetCreature],
        Mutation::RemoveTapCost => binding.ability.tap_cost = false,
        Mutation::AddWhiteManaCost => {
            binding.ability.mana_cost = ManaCost::with_colors(0, [Color::White])
        }
        Mutation::WrongTarget => binding.ability.targets = vec![TargetRequirement::Player],
    }
    (definition, binding)
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: CARD_NAME,
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Red, Color::White]),
        colors: BTreeSet::from([Color::Red, Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "tap-prevent-target-combat",
        ],
        power: Some(2),
        toughness: Some(1),
        keywords: vec![],
        effects: vec![],
    }
}

fn expected_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: CARD_ID,
        ability: ActivatedAbility {
            id: ABILITY_ID,
            mana_cost: ManaCost::new(0),
            tap_cost: true,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![TargetRequirement::Creature],
            effects: vec![Effect::ModifyTargetKeywordUntilEndOfTurn {
                keyword: Keyword::CannotAttackOrBlock,
            }],
        },
    }
}

fn validate_candidate_registry() -> Result<(), String> {
    let definition = candidate_thundersong_trumpeter::definition();
    let binding = candidate_thundersong_trumpeter::activated_ability();
    let registered_definition = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Thundersong Trumpeter is missing from the RAV registry")?;
    let registered_bindings: Vec<_> = rav_activated_ability_bindings()
        .into_iter()
        .filter(|candidate| candidate.card_definition == CARD_ID)
        .collect();
    if definition != expected_definition()
        || binding != expected_ability()
        || registered_definition != definition
        || registered_bindings != [binding]
    {
        return Err(
            "Thundersong Trumpeter registry must delegate its card definition and combat ability to the candidate module".to_owned(),
        );
    }
    Ok(())
}

/// Check the source grammar before evaluating either candidate function.
fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read candidate source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Thundersong Trumpeter source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
    {
        return Err("candidate module attributes are not allowed".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "ActivatedAbility"],
        vec!["cardbench_magic_engine", "ActivatedAbilityBinding"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TargetRequirement"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                imports.extend(
                    flatten_use_tree(&item.tree, Vec::new())
                        .ok_or("candidate module contains an unsupported import")?,
                );
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                let result_type = match name.as_str() {
                    "definition" => "CardDefinition",
                    "activated_ability" => "ActivatedAbilityBinding",
                    _ => return Err(format!("unsupported candidate function `{name}`")),
                };
                if !functions.insert(name.clone())
                    || !matches!(&function.vis, Visibility::Restricted(visibility) if visibility.path.is_ident("crate"))
                    || !function.sig.inputs.is_empty()
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || !function.sig.generics.params.is_empty()
                    || function.sig.generics.where_clause.is_some()
                    || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if type_is(ty, result_type))
                    || function.block.stmts.len() != 1
                    || !function.attrs.iter().all(|attribute| {
                        attribute.path().is_ident("doc") || attribute.path().is_ident("must_use")
                    })
                    || !matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(literal), None)
                        if literal.rest.is_none()
                            && literal.path.segments.len() == 1
                            && literal.path.segments[0].ident == result_type
                            && literal.fields.iter().all(|field| safe_expression(&field.expr)))
                {
                    return Err(format!(
                        "candidate function `{name}` is not a direct literal"
                    ));
                }
            }
            _ => {
                return Err(
                    "candidate contains executable or unsupported top-level code".to_owned(),
                );
            }
        }
    }
    if file.items.len() != 4
        || imports != expected_imports
        || functions != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()])
    {
        return Err("candidate source items or imports differ from the task contract".to_owned());
    }
    Ok(())
}

fn flatten_use_tree(tree: &UseTree, prefix: Vec<String>) -> Option<Vec<Vec<String>>> {
    match tree {
        UseTree::Path(path) => {
            let mut prefix = prefix;
            prefix.push(path.ident.to_string());
            flatten_use_tree(&path.tree, prefix)
        }
        UseTree::Name(name) => {
            let mut prefix = prefix;
            prefix.push(name.ident.to_string());
            Some(vec![prefix])
        }
        UseTree::Group(group) => group
            .items
            .iter()
            .map(|tree| flatten_use_tree(tree, prefix.clone()))
            .collect::<Option<Vec<_>>>()
            .map(|nested| nested.into_iter().flatten().collect()),
        UseTree::Rename(_) | UseTree::Glob(_) => None,
    }
}

fn type_is(ty: &syn::Type, expected: &str) -> bool {
    matches!(ty, syn::Type::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 && path.path.segments[0].ident == expected)
}

fn safe_expression(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(literal) => matches!(
            literal.lit,
            syn::Lit::Str(_) | syn::Lit::Int(_) | syn::Lit::Bool(_)
        ),
        Expr::Path(path) => {
            let parts: Vec<_> = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            matches!(parts.as_slice(),
                [a, b] if (a == "Color" && (b == "Red" || b == "White"))
                    || (a == "CardType" && b == "Creature")
                    || (a == "TargetRequirement" && b == "Creature")
                    || (a == "Keyword" && b == "CannotAttackOrBlock"))
                || parts.as_slice() == ["None"]
        }
        Expr::Call(call) => {
            let Expr::Path(path) = &*call.func else {
                return false;
            };
            let parts: Vec<_> = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            (matches!(parts.as_slice(), [a, b] if (a == "ManaCost" && (b == "new" || b == "with_colors")) || (a == "BTreeSet" && (b == "new" || b == "from")))
                || parts.as_slice() == ["Some"])
                && call.args.iter().all(safe_expression)
        }
        Expr::Struct(literal) => {
            (literal.path.segments.len() == 1
                && matches!(
                    literal.path.segments[0].ident.to_string().as_str(),
                    "CardDefinition" | "ActivatedAbilityBinding" | "ActivatedAbility"
                )
                || literal.path.segments.len() == 2
                    && literal.path.segments[0].ident == "Effect"
                    && literal.path.segments[1].ident == "ModifyTargetKeywordUntilEndOfTurn")
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

fn deck(entries: &[(&str, u8)]) -> DeckList {
    DeckList {
        mainboard: entries
            .iter()
            .map(|(card, count)| DeckEntry {
                card: (*card).to_owned(),
                count: *count,
            })
            .collect(),
        sideboard: Vec::new(),
    }
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_THUNDERSONG_TRUMPETER_SEED") {
        let seed = value.parse::<u64>().map_err(|_| {
            "CARDBENCH_THUNDERSONG_TRUMPETER_SEED must be an unsigned integer".to_owned()
        })?;
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    let mut last_error = String::new();
    for seed in 0..=MAX_TRACE_SEED {
        match run_match(seed, Mutation::None)
            .and_then(|captured| validate_trace(&captured, seed).map(|_| captured))
        {
            Ok(captured) => return Ok((seed, captured)),
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "no complete Thundersong Trumpeter trace found for seeds 0..={MAX_TRACE_SEED}: {last_error}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot = deck(&[
        (CARD_ID, 4),
        (WATCHWOLF_ID, 4),
        ("RAV-PLAINS", 18),
        ("RAV-MOUNTAIN", 17),
        ("RAV-FOREST", 17),
    ]);
    let defender = deck(&[(TARGET_ID, 4), ("RAV-PLAINS", 56)]);
    for (name, list) in [("pilot", &pilot), ("defender", &defender)] {
        let count: usize = list
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!(
                "{name} constructed deck has {count} cards; expected 60"
            ));
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_MOVES,
        ..DeckMatchConfig::default()
    };
    let (definition, binding) = candidate_values(mutation);
    let mut game = new_game(definition.clone(), binding)?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    for (player, list) in [(PlayerId(0), &pilot), (PlayerId(1), &defender)] {
        game.load_deck_into_library(player, list)
            .map_err(|error| error.to_string())?;
        game.draw_opening_hand(player, config.opening_hand_size)
            .map_err(|error| error.to_string())?;
    }
    game.begin_game().map_err(|error| error.to_string())?;
    let watchwolf_cost = card_definitions()
        .into_iter()
        .find(|card| card.id == WATCHWOLF_ID)
        .ok_or("Watchwolf is missing from the RAV catalog")?
        .mana_cost;
    let target_cost = card_definitions()
        .into_iter()
        .find(|card| card.id == TARGET_ID)
        .ok_or("Benevolent Ancestor is missing from the RAV catalog")?
        .mana_cost;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(TrumpeterPolicy::new(
            PlayerId(0),
            definition.mana_cost,
            watchwolf_cost,
        )),
        Box::new(TrumpeterPolicy::new(
            PlayerId(1),
            target_cost,
            ManaCost::new(0),
        )),
    ];
    let mut captured = Captured::default();
    while !game.is_game_over() {
        if game.turn > config.max_turns || captured.accepted >= config.max_policy_moves {
            return Err(format!(
                "Thundersong Trumpeter game exceeded its run bound on turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Thundersong policy seat missing")?;
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

        let activation = if let PolicyAction::ActivateAbility { activation } = &action
            && activation.ability_id == ABILITY_ID
        {
            let target = activation
                .targets
                .first()
                .and_then(|target| match target {
                    Target::Permanent(object) => Some(*object),
                    _ => None,
                })
                .ok_or("Thundersong Trumpeter activation did not target a permanent")?;
            captured.activation_source = Some(activation.source);
            captured.activation_target = Some(target);
            captured.activation_turn = Some(view.turn);
            captured
                .active_player_at_activation
                .get_or_insert(view.active_player.0);
            captured
                .target_can_block_before_activation
                .get_or_insert_with(|| {
                    view.opponent_battlefield
                        .iter()
                        .find(|card| card.id == target)
                        .is_some_and(|card| card.can_block)
                });
            Some((activation.source, target))
        } else {
            None
        };

        if view.step == Step::DeclareBlockers
            && view.active_player == PlayerId(0)
            && player == PlayerId(1)
            && !view.blockers_declared
            && captured.target_can_block_at_declaration.is_none()
            && captured.ability_resolved
        {
            if let Some(target) = captured.activation_target {
                captured.target_can_block_at_declaration = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.id == target)
                    .map(|card| card.can_block);
            }
        }

        let event_cursor = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Thundersong policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        captured.accepted += 1;
        if let Some((source, target)) = activation {
            captured.activation_count += 1;
            captured.source_tapped_after_activation =
                game.object(source).is_ok_and(|object| object.tapped);
            captured.target = Some(target);
        }
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        observe(&game, &game.event_log[event_cursor..], &mut captured)?;
    }
    captured.events = game.event_log.clone();
    captured.winner = game.winner().map(|player| player.0);
    captured.terminal = captured.winner.map_or_else(
        || "Draw".to_owned(),
        |seat| format!("Winner(PlayerId({seat}))"),
    );
    captured.turns = game.turn;
    captured.life = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    Ok(captured)
}

fn new_game(definition: CardDefinition, binding: ActivatedAbilityBinding) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Thundersong Trumpeter is missing from the RAV registry")?;
    *registered = definition;
    let mut abilities = rav_activated_ability_bindings();
    let matches: Vec<_> = abilities
        .iter_mut()
        .filter(|known| known.card_definition == CARD_ID && known.ability.id == ABILITY_ID)
        .collect();
    if matches.len() != 1 {
        return Err(format!(
            "expected exactly one Thundersong Trumpeter ability binding, found {}",
            matches.len()
        ));
    }
    matches
        .into_iter()
        .next()
        .expect("one binding")
        .clone_from(&binding);
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

fn observe(game: &Game, events: &[GameEvent], captured: &mut Captured) -> Result<(), String> {
    for event in events {
        match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if game
                .card_definition(*card)
                .is_ok_and(|known| known.id == CARD_ID) =>
            {
                captured.trumpeter_payment = colors.clone();
            }
            GameEvent::SpellManaPaid {
                player: PlayerId(1),
                card,
                colors,
            } if game
                .card_definition(*card)
                .is_ok_and(|known| known.id == TARGET_ID) =>
            {
                captured.target_payment = colors.clone();
            }
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|known| known.id == CARD_ID) =>
            {
                captured.trumpeter = Some(*card);
            }
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|known| known.id == WATCHWOLF_ID) =>
            {
                captured.watchwolf = Some(*card);
            }
            GameEvent::SpellCast {
                player: PlayerId(1),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|known| known.id == TARGET_ID) =>
            {
                captured.target = Some(*card)
            }
            GameEvent::SpellResolved { card } if Some(*card) == captured.trumpeter => {
                captured.trumpeter_resolved = true;
            }
            GameEvent::SpellResolved { card } if Some(*card) == captured.watchwolf => {}
            GameEvent::SpellResolved { card } if Some(*card) == captured.target => {
                captured.target_resolved = true;
            }
            GameEvent::CardMoved {
                card,
                to: Zone::Battlefield,
            } if Some(*card) == captured.trumpeter => {
                captured.trumpeter_entered = true;
                if let Ok(characteristics) = game.characteristics(*card) {
                    captured.trumpeter_power = characteristics.power;
                    captured.trumpeter_toughness = characteristics.toughness;
                }
            }
            GameEvent::CardMoved {
                card,
                to: Zone::Battlefield,
            } if Some(*card) == captured.target => captured.target_entered = true,
            GameEvent::CardMoved {
                card,
                to: Zone::Battlefield,
            } if Some(*card) == captured.watchwolf => captured.watchwolf_entered = true,
            GameEvent::AbilityActivated {
                player: PlayerId(0),
                source,
                definition,
                ability,
                ..
            } if *definition == CARD_ID && *ability == ABILITY_ID => {
                captured.activation_source.get_or_insert(*source);
            }
            GameEvent::AbilityManaPaid {
                player: PlayerId(0),
                source,
                ability,
                ..
            } if Some(*source) == captured.activation_source && *ability == ABILITY_ID => {
                captured.ability_mana_paid = true;
            }
            GameEvent::ContinuousEffectCreated { source, target, .. }
                if Some(*source) == captured.activation_source
                    && Some(*target) == captured.activation_target =>
            {
                captured.restriction_effect_created = true;
            }
            GameEvent::AbilityResolved {
                source, ability, ..
            } if Some(*source) == captured.activation_source && *ability == ABILITY_ID => {
                captured.ability_resolved = true;
                if let Some(target) = captured.activation_target {
                    captured.target_tapped_after_resolution =
                        game.object(target).ok().map(|object| object.tapped);
                    captured.target_has_combat_restriction_after_resolution =
                        game.characteristics(target).is_ok_and(|characteristics| {
                            characteristics
                                .keywords
                                .contains(&Keyword::CannotAttackOrBlock)
                        });
                }
            }
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                ..
            } => {}
            GameEvent::BlockersDeclared {
                player: PlayerId(1),
                assignments,
            } => {
                captured.target_blocker_assignments.extend(
                    assignments
                        .iter()
                        .filter(|(_, blocker)| Some(*blocker) == captured.activation_target)
                        .map(|(attacker, blocker)| (*attacker, *blocker)),
                );
            }
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if game
                .card_definition(*source)
                .is_ok_and(|definition| definition.id == WATCHWOLF_ID) =>
            {
                captured.damage_to_opponent += i64::from(*amount);
            }
            GameEvent::GameEnded {
                winner: Some(PlayerId(0)),
            } => captured.winner = Some(0),
            _ => {}
        }
    }
    Ok(())
}

struct TrumpeterPolicy {
    player: PlayerId,
    primary_cost: ManaCost,
    secondary_cost: ManaCost,
    primary_cast: bool,
    secondary_cast: bool,
    ability_turn: Option<u32>,
}

impl TrumpeterPolicy {
    fn new(player: PlayerId, primary_cost: ManaCost, secondary_cost: ManaCost) -> Self {
        Self {
            player,
            primary_cost,
            secondary_cost,
            primary_cast: false,
            secondary_cast: false,
            ability_turn: None,
        }
    }

    fn player_is_pilot(&self) -> bool {
        self.player == PlayerId(0)
    }

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let need = if self.player_is_pilot() {
            let colors = [Color::Red, Color::White, Color::Green];
            colors.into_iter().find(|color| {
                view.mana_pool.amount(*color) == 0
                    && !view.own_battlefield.iter().any(|card| {
                        card.card_types.contains(&CardType::Land)
                            && card.mana_colors.contains(color)
                    })
            })
        } else {
            Some(Color::White)
        };
        let land = need
            .and_then(|color| {
                view.hand.iter().find(|card| {
                    card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&color)
                })
            })
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            })?;
        Some(PolicyAction::PlayLand { card: land.id })
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
            hybrid: Vec::new(),
        })
    }

    fn mana_action(view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
        let missing = cost
            .colored
            .iter()
            .find(|color| view.mana_pool.amount(**color) == 0)
            .copied();
        let land = view
            .own_battlefield
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land)
                    && !card.tapped
                    && missing.is_some_and(|color| card.mana_colors.contains(&color))
            })
            .or_else(|| {
                view.own_battlefield.iter().find(|card| {
                    card.card_types.contains(&CardType::Land)
                        && !card.tapped
                        && !card.mana_colors.is_empty()
                })
            })?;
        let color = missing
            .filter(|color| land.mana_colors.contains(color))
            .or_else(|| land.mana_colors.iter().next().copied())?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }

    fn cast(
        &mut self,
        view: &GameView,
        card_id: &str,
        cost: &ManaCost,
        primary: bool,
    ) -> Option<PolicyAction> {
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(card_id))?;
        let action = if let Some(mana_selection) = Self::payment(view, cost) {
            PolicyAction::CastWithPayment {
                request: CastRequest {
                    card: card.id,
                    targets: Vec::new(),
                    convoke: Vec::new(),
                    payment_mana_abilities: Vec::new(),
                },
                chosen_x: None,
                mana_selection,
            }
        } else {
            Self::mana_action(view, cost)?
        };
        if matches!(action, PolicyAction::CastWithPayment { .. }) {
            if primary {
                self.primary_cast = true;
            } else {
                self.secondary_cast = true;
            }
        }
        Some(action)
    }
}

impl CodePolicy for TrumpeterPolicy {
    fn id(&self) -> &'static str {
        if self.player_is_pilot() {
            "rav.thundersong-trumpeter-pilot.v1"
        } else {
            "rav.thundersong-trumpeter-defender.v1"
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
            let attackers = if self.player_is_pilot() && self.ability_turn == Some(view.turn) {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(WATCHWOLF_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect()
            } else {
                Vec::new()
            };
            return PolicyAction::DeclareAttackers { attackers };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            let assignments = if self.player == PlayerId(1) {
                let blocker = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.definition == Some(TARGET_ID) && card.can_block);
                let attacker = view
                    .combat_attackers
                    .iter()
                    .find(|card| card.definition == Some(WATCHWOLF_ID));
                match (blocker, attacker) {
                    (Some(blocker), Some(attacker)) => vec![CombatBlock {
                        attacker: attacker.id,
                        blocker: blocker.id,
                    }],
                    _ => Vec::new(),
                }
            } else {
                Vec::new()
            };
            return PolicyAction::DeclareBlockers { assignments };
        }
        if view.active_player != self.player
            || view.priority != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.play_land(view) {
            return action;
        }
        if self.player_is_pilot() && !self.secondary_cast {
            if let Some(action) = self.cast(view, WATCHWOLF_ID, &self.secondary_cost.clone(), false)
            {
                return action;
            }
        }
        if !self.primary_cast {
            let primary_id = if self.player_is_pilot() {
                CARD_ID
            } else {
                TARGET_ID
            };
            if let Some(action) = self.cast(view, primary_id, &self.primary_cost.clone(), true) {
                return action;
            }
        }
        if self.player_is_pilot() && self.ability_turn != Some(view.turn) {
            let attack_is_ready = view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(WATCHWOLF_ID) && card.can_attack);
            let source = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID) && card.can_attack);
            let target = view
                .opponent_battlefield
                .iter()
                .find(|card| card.definition == Some(TARGET_ID) && card.can_block);
            if attack_is_ready && let (Some(source), Some(target)) = (source, target) {
                self.ability_turn = Some(view.turn);
                return PolicyAction::ActivateAbility {
                    activation: AbilityActivation {
                        source: source.id,
                        ability_id: ABILITY_ID,
                        sacrifice_sources: Vec::new(),
                        additional_tap_creatures: Vec::new(),
                        discard_cards: Vec::new(),
                        targets: vec![Target::Permanent(target.id)],
                    },
                };
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    let trumpeter = captured
        .trumpeter
        .ok_or("Thundersong Trumpeter was not cast")?;
    let watchwolf = captured.watchwolf.ok_or("pilot Watchwolf was not cast")?;
    let target = captured
        .target
        .ok_or("opposing Benevolent Ancestor was not cast")?;
    let source = captured
        .activation_source
        .ok_or("Thundersong Trumpeter activated ability was not used")?;
    let ability_target = captured
        .activation_target
        .ok_or("Thundersong Trumpeter ability target is missing")?;

    if captured.winner != Some(0)
        || captured.terminal != "Winner(PlayerId(0))"
        || captured.rejected != 0
        || captured.turns == 0
    {
        return Err("Thundersong Trumpeter game did not reach a clean P0 terminal win".to_owned());
    }
    if !captured.trumpeter_resolved
        || !captured.trumpeter_entered
        || captured.trumpeter_power != Some(2)
        || captured.trumpeter_toughness != Some(1)
        || captured.trumpeter_payment.len() != 2
        || !captured.trumpeter_payment.contains(&Color::Red)
        || !captured.trumpeter_payment.contains(&Color::White)
    {
        return Err(
            "Thundersong Trumpeter did not resolve as the red-white 2/1 with its exact cost"
                .to_owned(),
        );
    }
    if !captured.target_resolved || !captured.target_entered {
        return Err("opposing Benevolent Ancestor did not resolve onto the battlefield".to_owned());
    }
    if source != trumpeter
        || ability_target != target
        || captured.activation_count == 0
        || captured.active_player_at_activation != Some(0)
        || !captured.watchwolf_entered
        || captured.target_can_block_before_activation != Some(true)
        || !captured.source_tapped_after_activation
        || captured.ability_mana_paid
        || !captured.ability_resolved
        || !captured.restriction_effect_created
        || captured.target_tapped_after_resolution != Some(false)
        || !captured.target_has_combat_restriction_after_resolution
        || captured.target_can_block_at_declaration != Some(false)
        || !captured.target_blocker_assignments.is_empty()
    {
        return Err(format!(
            "Thundersong Trumpeter must tap itself, resolve the temporary combat restriction on the live opposing creature, and remove that creature's block choice: source={source:?}, target={ability_target:?}, count={}, active={:?}, before={:?}, source_tapped={}, mana_paid={}, resolved={}, effect_created={}, target_tapped={:?}, has_restriction={}, can_block={:?}, target_blocks={:?}",
            captured.activation_count,
            captured.active_player_at_activation,
            captured.target_can_block_before_activation,
            captured.source_tapped_after_activation,
            captured.ability_mana_paid,
            captured.ability_resolved,
            captured.restriction_effect_created,
            captured.target_tapped_after_resolution,
            captured.target_has_combat_restriction_after_resolution,
            captured.target_can_block_at_declaration,
            captured.target_blocker_assignments
        ));
    }
    if captured.damage_to_opponent < 20 || captured.life[0] <= 0 || captured.life[1] > 0 {
        return Err(format!(
            "the P0 pilot did not finish the full match after clearing the blocker: damage={}, life={:?}",
            captured.damage_to_opponent, captured.life
        ));
    }

    let paid = event_index(&captured.events, |event| {
        matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, .. } if *card == trumpeter)
    })
    .ok_or("Thundersong Trumpeter SpellManaPaid receipt missing")?;
    let cast = event_index(&captured.events, |event| {
        matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == trumpeter)
    })
    .ok_or("Thundersong Trumpeter SpellCast receipt missing")?;
    let resolved = event_index(
        &captured.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == trumpeter),
    )
    .ok_or("Thundersong Trumpeter SpellResolved receipt missing")?;
    let entered = event_index(&captured.events, |event| {
        matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == trumpeter)
    })
    .ok_or("Thundersong Trumpeter battlefield entry receipt missing")?;
    let target_paid = event_index(&captured.events, |event| {
        matches!(event, GameEvent::SpellManaPaid { player: PlayerId(1), card, .. } if *card == target)
    })
    .ok_or("Benevolent Ancestor SpellManaPaid receipt missing")?;
    let target_cast = event_index(&captured.events, |event| {
        matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == target)
    })
    .ok_or("Benevolent Ancestor SpellCast receipt missing")?;
    let target_resolved = event_index(
        &captured.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == target),
    )
    .ok_or("Benevolent Ancestor SpellResolved receipt missing")?;
    let target_entered = event_index(&captured.events, |event| {
        matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == target)
    })
    .ok_or("Benevolent Ancestor battlefield entry receipt missing")?;
    let activated = event_index_last(&captured.events, |event| {
        matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source: known, definition, ability, .. } if *known == trumpeter && *definition == CARD_ID && *ability == ABILITY_ID)
    })
    .ok_or("Thundersong Trumpeter AbilityActivated receipt missing")?;
    let effect_created = event_index_last(&captured.events, |event| {
        matches!(event, GameEvent::ContinuousEffectCreated { source: known, target: affected, .. } if *known == trumpeter && *affected == target)
    })
    .ok_or("Thundersong Trumpeter combat-restriction effect receipt missing")?;
    let ability_resolved = event_index_last(&captured.events, |event| {
        matches!(event, GameEvent::AbilityResolved { source: known, ability: ABILITY_ID, .. } if *known == trumpeter)
    })
    .ok_or("Thundersong Trumpeter AbilityResolved receipt missing")?;
    let attack = event_index_after(&captured.events, ability_resolved, |event| {
        matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&watchwolf))
    })
    .ok_or("pilot Watchwolf did not attack after the restriction resolved")?;
    let blockers = event_index_after(&captured.events, attack, |event| {
        matches!(event, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if assignments.iter().all(|(_, blocker)| *blocker != target))
    })
    .ok_or("no blocker declaration followed the pilot's attack")?;
    let damage = event_index_after(&captured.events, blockers, |event| {
        matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount } if *source == watchwolf && *amount > 0)
    })
    .ok_or("pilot Watchwolf dealt no damage after the combat restriction")?;
    let ended = event_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal P0 GameEnded receipt missing")?;
    if !(paid < cast
        && cast < resolved
        && resolved < entered
        && target_paid < target_cast
        && target_cast < target_resolved
        && target_resolved < target_entered
        && entered < activated
        && target_entered < activated
        && activated < effect_created
        && effect_created < ability_resolved
        && ability_resolved < attack
        && attack < blockers
        && blockers < damage
        && damage < ended)
    {
        return Err(format!(
            "Thundersong Trumpeter cast, target, ability, combat, and terminal event order differs: card=({paid},{cast},{resolved},{entered}), target=({target_paid},{target_cast},{target_resolved},{target_entered}), ability=({activated},{effect_created},{ability_resolved}), combat=({attack},{blockers},{damage},{ended})"
        ));
    }
    if target_paid < paid && target_entered >= paid {
        return Err("opposing target spell payment, cast, and entry are out of order".to_owned());
    }
    if paid < target_paid && entered >= target_paid {
        return Err("Thundersong spell payment, cast, and entry are out of order".to_owned());
    }
    let selected: BTreeSet<_> = [
        target_paid,
        target_cast,
        target_resolved,
        target_entered,
        paid,
        cast,
        resolved,
        entered,
        activated,
        effect_created,
        ability_resolved,
        attack,
        blockers,
        damage,
        ended,
    ]
    .into_iter()
    .collect();
    let projected = project_events(&captured.events);
    let activation_turn = captured
        .activation_turn
        .ok_or("Thundersong Trumpeter activation turn is missing")?;
    if projected[ability_resolved].turn != activation_turn
        || projected[attack].turn != activation_turn
        || projected[blockers].turn != activation_turn
    {
        return Err(format!(
            "Thundersong restriction and its combat declaration must occur in the same turn: activation={activation_turn}, resolved={}, attack={}, blockers={}",
            projected[ability_resolved].turn, projected[attack].turn, projected[blockers].turn
        ));
    }
    let events: Vec<_> = selected
        .iter()
        .map(|index| serde_json::to_value(&projected[*index]).map_err(|error| error.to_string()))
        .collect::<Result<_, _>>()?;
    let kinds: Vec<_> = events
        .iter()
        .filter_map(|event| event["kind"].as_str())
        .collect();
    let spell_group = ["SpellManaPaid", "SpellCast", "SpellResolved", "CardMoved"];
    let combat_group = [
        "AbilityActivated",
        "ContinuousEffectCreated",
        "AbilityResolved",
        "AttackersDeclared",
        "BlockersDeclared",
        "DamageDealtToPlayer",
        "GameEnded",
    ];
    let mut expected_kinds = Vec::new();
    expected_kinds.extend(spell_group);
    expected_kinds.extend(spell_group);
    expected_kinds.extend(combat_group);
    if kinds != expected_kinds {
        return Err(format!(
            "typed Thundersong event projection differs: {kinds:?}"
        ));
    }
    Ok(json!({
        "id": "rav-thundersong-trumpeter-restricts-benevolent-ancestor-block-full-game",
        "seed": seed,
        "decks": ["rav_thundersong_trumpeter_pilot_60", "rav_benevolent_ancestor_defense_60"],
        "policies": ["rav.thundersong-trumpeter-pilot.v1", "rav.thundersong-trumpeter-defender.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted,
        "rejected_moves": captured.rejected,
        "digest": event_digest(&captured.events),
        "thundersong_trumpeter": {
            "object": trumpeter.0,
            "collector_number": 235,
            "mana_cost": "{R}{W}",
            "mana_paid": captured.trumpeter_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "power": captured.trumpeter_power,
            "toughness": captured.trumpeter_toughness,
            "resolved": captured.trumpeter_resolved
        },
        "activation": {
            "source": source.0,
            "ability_id": ABILITY_ID,
            "mana_cost": "{0}",
            "tap_cost": true,
            "activation_count": captured.activation_count,
            "activation_turn": captured.activation_turn,
            "active_player": captured.active_player_at_activation,
            "source_tapped_after_activation": captured.source_tapped_after_activation,
            "target": target.0,
            "target_definition": TARGET_ID,
            "target_can_block_before": captured.target_can_block_before_activation,
            "target_tapped_after_resolution": captured.target_tapped_after_resolution,
            "cannot_attack_or_block_effect_created": captured.restriction_effect_created,
            "target_has_combat_restriction": captured.target_has_combat_restriction_after_resolution,
            "target_can_block_at_declaration": captured.target_can_block_at_declaration,
            "target_assigned_as_blocker": !captured.target_blocker_assignments.is_empty()
        },
        "combat": {
            "watchwolf": watchwolf.0,
            "target_card_payment": captured.target_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "target_resolved": captured.target_resolved,
            "watchwolf_damage_to_opponent": captured.damage_to_opponent,
            "life_at_terminal": captured.life
        },
        "ordered_event_sequence": selected.iter().copied().collect::<Vec<_>>(),
        "events": events,
        "passed": true
    }))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Err(_) => Ok(true),
        Ok(captured) => Ok(validate_trace(&captured, seed).is_err()),
    }
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}

fn event_index_last(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().rposition(predicate)
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

fn event_digest(events: &[GameEvent]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in serde_json::to_vec(&project_events(events)).expect("projected events serialize") {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}
