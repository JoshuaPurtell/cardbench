//! Full-game diagnostic authority for the RAV Three Dreams task.

use cardbench_magic_engine::{
    CardType, Color, DecisionKind, DecisionSelection, DecisionVisibility, DeckEntry, DeckList,
    Effect, Game, GameEvent, GameView, LibrarySearchCardinality, LibrarySearchDestination,
    LibrarySearchRequirement, LibrarySearchSelection, ManaCost, ManaPaymentSelection, PlayerId,
    PolicyAction, Step, Zone,
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
    rav_static_library_top_reveal_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-THREE-DREAMS";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/three_dreams.rs";
const MAX_TRACE_SEED: u64 = 1;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/three_dreams.rs"]
mod candidate_three_dreams;

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
            eprintln!("Three Dreams PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_definition()?;
    let catalog = unique_catalog()?;
    let seed = find_trace_seed()?;
    let three = run_match(seed, SelectionMode::ThreeDistinct, Mutation::None)?;
    let three_replay = run_match(seed, SelectionMode::ThreeDistinct, Mutation::None)?;
    if three != three_replay {
        return Err("Three Dreams three-Aura full-game replay was not deterministic".to_owned());
    }
    validate_trace(SelectionMode::ThreeDistinct, &three)?;
    let zero = run_match(seed, SelectionMode::Zero, Mutation::None)?;
    let zero_replay = run_match(seed, SelectionMode::Zero, Mutation::None)?;
    if zero != zero_replay {
        return Err("Three Dreams zero-choice full-game replay was not deterministic".to_owned());
    }
    validate_trace(SelectionMode::Zero, &zero)?;
    let mutations = json!({
        "maximum_lowered_mutation_detected_in_game": mutation_detected(seed, SelectionMode::ThreeDistinct, Mutation::LowerMaximum),
        "duplicate_name_allowed_mutation_detected_in_game": mutation_detected(seed, SelectionMode::ThreeDistinct, Mutation::AllowDuplicateNames),
        "selected_reveals_suppressed_mutation_detected_in_game": mutation_detected(seed, SelectionMode::ThreeDistinct, Mutation::SuppressReveals),
        "sealed_mutation_authority": false
    });
    if mutations["maximum_lowered_mutation_detected_in_game"] != true
        || mutations["duplicate_name_allowed_mutation_detected_in_game"] != true
        || mutations["selected_reveals_suppressed_mutation_detected_in_game"] != true
    {
        return Err(format!(
            "a Three Dreams semantic mutation escaped the gameplay oracle: {mutations}"
        ));
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 32, "definition_id": CARD_ID, "name": "Three Dreams"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": ["601.2: casting spells", "608.2: resolving spells and abilities", "701.16: revealing cards", "701.19: searching a library", "400.2: public and hidden zones"],
            "source_contract": "RAV Three Dreams is a white Sorcery for {4}{W}; its controller may search their library for up to three Aura cards with different names, reveal them, put them into their hand, and shuffle.",
            "authority_cutoff": "2026-09-24"
        },
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": rav_main_set_catalog().len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "unreviewed_diagnostic",
        "mutation_sensitivity": mutations,
        "traces": [
            trace_json(&three, SelectionMode::ThreeDistinct, true),
            trace_json(&zero, SelectionMode::Zero, true)
        ],
        "per_card_breakdown": breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Three Dreams source: {error}"))?;
    let file =
        syn::parse_file(&source).map_err(|_| "Three Dreams source is not valid Rust".to_owned())?;
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "LibrarySearchCardinality"],
        vec!["cardbench_magic_engine", "LibrarySearchDestination"],
        vec!["cardbench_magic_engine", "LibrarySearchRequirement"],
        vec!["cardbench_magic_engine", "LibrarySearchSelection"],
        vec!["cardbench_magic_engine", "ManaCost"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut definition_count = 0;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let mut paths = Vec::new();
                if item.leading_colon.is_some() || !flatten_use_tree(&[], &item.tree, &mut paths) {
                    return Err("Three Dreams source has unsupported imports".to_owned());
                }
                imports.extend(paths);
            }
            Item::Fn(item)
                if item.sig.ident == "definition"
                    && item.sig.inputs.is_empty()
                    && item.sig.generics.params.is_empty()
                    && matches!(&item.sig.output, syn::ReturnType::Type(_, ty) if type_is(ty, "CardDefinition"))
                    && matches!(&item.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    && item.block.stmts.len() == 1
                    && matches!(&item.block.stmts[0], Stmt::Expr(Expr::Struct(value), None) if value.rest.is_none() && path_is_type_path(&value.path, "CardDefinition")) =>
            {
                definition_count += 1
            }
            _ => {
                return Err(
                    "Three Dreams module structure differs from its source contract".to_owned(),
                );
            }
        }
    }
    if imports != expected_imports || definition_count != 1 || file.items.len() != 3 {
        return Err("Three Dreams module structure differs from its source contract".to_owned());
    }
    Ok(())
}

fn flatten_use_tree(prefix: &[String], tree: &UseTree, output: &mut Vec<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix.to_vec();
            next.push(path.ident.to_string());
            flatten_use_tree(&next, &path.tree, output)
        }
        UseTree::Name(name) => {
            let mut path = prefix.to_vec();
            path.push(name.ident.to_string());
            output.push(path);
            true
        }
        UseTree::Group(group) => group
            .items
            .iter()
            .all(|item| flatten_use_tree(prefix, item, output)),
        _ => false,
    }
}

fn type_is(ty: &syn::Type, name: &str) -> bool {
    matches!(ty, syn::Type::Path(path) if path.qself.is_none() && path.path.segments.len() == 1 && path.path.segments[0].ident == name)
}

fn path_is_type_path(path: &syn::Path, name: &str) -> bool {
    path.leading_colon.is_none() && path.segments.len() == 1 && path.segments[0].ident == name
}

fn validate_candidate_definition() -> Result<(), String> {
    let definition = candidate_three_dreams::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Three Dreams definition is missing from the RAV registry")?;
    if definition != registered
        || definition.id != CARD_ID
        || definition.name != "Three Dreams"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(4, [Color::White])
        || definition.colors != BTreeSet::from([Color::White])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Sorcery])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
        || definition.effects != expected_effects(Mutation::None)
    {
        return Err("Three Dreams literal is outside the task source contract".to_owned());
    }
    Ok(())
}

fn expected_effects(mutation: Mutation) -> Vec<Effect> {
    let cardinality = match mutation {
        Mutation::LowerMaximum => LibrarySearchCardinality::ZeroOrMoreDistinctNames { maximum: 2 },
        Mutation::AllowDuplicateNames => LibrarySearchCardinality::ZeroOrMore { maximum: 3 },
        Mutation::None | Mutation::SuppressReveals => {
            LibrarySearchCardinality::ZeroOrMoreDistinctNames { maximum: 3 }
        }
    };
    vec![Effect::SearchControllerLibraryMany {
        requirement: LibrarySearchRequirement::Aura,
        destination: LibrarySearchDestination::Hand,
        cardinality,
        selection: LibrarySearchSelection::PolicySubmitted {
            may_fail_to_find: false,
        },
        reveal_selected: mutation != Mutation::SuppressReveals,
    }]
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
        return Err("RAV catalog denominator differs from the source contract".to_owned());
    }
    Ok(catalog)
}

fn find_trace_seed() -> Result<u64, String> {
    let mut last_problem = "unknown failure".to_owned();
    for seed in 0..MAX_TRACE_SEED {
        let three = match run_match(seed, SelectionMode::ThreeDistinct, Mutation::None) {
            Ok(capture) => capture,
            Err(error) => {
                last_problem = format!("three-card branch: {error}");
                continue;
            }
        };
        let zero = match run_match(seed, SelectionMode::Zero, Mutation::None) {
            Ok(capture) => capture,
            Err(error) => {
                last_problem = format!("zero-card branch: {error}");
                continue;
            }
        };
        match validate_trace(SelectionMode::ThreeDistinct, &three) {
            Ok(()) => {}
            Err(error) => {
                last_problem = format!("three-card validation: {error}");
                continue;
            }
        }
        match validate_trace(SelectionMode::Zero, &zero) {
            Ok(()) => {}
            Err(error) => {
                last_problem = format!("zero-card validation: {error}");
                continue;
            }
        }
        return Ok(seed);
    }
    Err(format!(
        "no bounded shuffle seed completed both Three Dreams Aura-search duels: {last_problem}"
    ))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SelectionMode {
    ThreeDistinct,
    Zero,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    LowerMaximum,
    AllowDuplicateNames,
    SuppressReveals,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    spell: Option<cardbench_magic_engine::ObjectId>,
    option_definitions: Vec<String>,
    selected: Vec<cardbench_magic_engine::ObjectId>,
    selected_names: Vec<String>,
    private_to_controller: bool,
    hidden_from_opponent: bool,
}

fn run_match(seed: u64, mode: SelectionMode, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definitions = card_definitions();
    let definition = definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Three Dreams definition missing from registry")?;
    let mut candidate_definition = candidate_three_dreams::definition();
    candidate_definition.effects = expected_effects(mutation);
    *definition = candidate_definition;
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_bindings(),
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

    let deck0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-WATCHWOLF".to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOLDERVINE-CLOAK".to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FLICKERFORM".to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FAITHS-FETTERS".to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 8,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 8,
            },
            DeckEntry {
                card: "RAV-ISLAND".to_owned(),
                count: 8,
            },
            DeckEntry {
                card: "RAV-SWAMP".to_owned(),
                count: 8,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 8,
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
        max_policy_moves: 8000,
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
        Box::new(ThreeDreamsPolicy::new(PlayerId(0))),
        Box::new(ThreeDreamsPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut spell = None;
    let mut option_definitions = Vec::new();
    let mut selected = Vec::new();
    let mut selected_names = Vec::new();
    let mut private_to_controller = false;
    let mut hidden_from_opponent = false;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(format!(
                "Three Dreams duel exceeded bounds at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let action = if let Some(choice) = view.library_search_choice.as_ref() {
            let prompt = view
                .pending_decision
                .as_ref()
                .ok_or("Aura search choice lacks typed decision")?;
            private_to_controller = prompt.kind == DecisionKind::LibrarySearch
                && prompt.visibility == DecisionVisibility::Private
                && prompt.candidates.len() == choice.cards.len();
            let opponent = game
                .view_for_player(PlayerId(1))
                .map_err(|error| error.to_string())?;
            hidden_from_opponent =
                opponent.library_search_choice.is_none() && opponent.pending_decision.is_none();
            option_definitions = choice
                .cards
                .iter()
                .filter_map(|card| card.definition.map(str::to_owned))
                .collect();
            let target_count = if mode == SelectionMode::Zero {
                0
            } else {
                usize::from(prompt.max_selections.min(3))
            };
            if mutation == Mutation::AllowDuplicateNames && mode == SelectionMode::ThreeDistinct {
                let duplicates: Vec<_> = choice
                    .cards
                    .iter()
                    .filter(|card| card.definition == Some("RAV-MOLDERVINE-CLOAK"))
                    .take(2)
                    .map(|card| (card.id, card.definition.unwrap_or_default().to_owned()))
                    .collect();
                if duplicates.len() == 2 {
                    selected.extend(duplicates.iter().map(|(id, _)| *id));
                    selected_names.extend(duplicates.into_iter().map(|(_, name)| name));
                    if let Some(card) = choice
                        .cards
                        .iter()
                        .find(|card| card.definition == Some("RAV-FLICKERFORM"))
                    {
                        if selected.len() < 3 {
                            selected.push(card.id);
                            selected_names.push(card.definition.unwrap_or_default().to_owned());
                        }
                    }
                }
            } else if target_count > 0 {
                for name in [
                    "RAV-MOLDERVINE-CLOAK",
                    "RAV-FLICKERFORM",
                    "RAV-FAITHS-FETTERS",
                ] {
                    if selected.len() >= target_count {
                        break;
                    }
                    if let Some(card) = choice
                        .cards
                        .iter()
                        .find(|card| card.definition == Some(name) && !selected.contains(&card.id))
                    {
                        selected.push(card.id);
                        selected_names.push(name.to_owned());
                    }
                }
            }
            PolicyAction::SubmitDecision {
                decision: choice.decision,
                selection: DecisionSelection::Objects(selected.clone()),
            }
        } else if let Some(action) = policies[player.0].propose_pending_decision(&view) {
            action
        } else if view.optional_triggered_ability_choice.is_some() {
            policies[player.0].propose_optional_triggered_ability(&view)
        } else if view.draw_replacement_pending {
            policies[player.0].propose_draw_replacement(&view)
        } else if view.private_library_choice.is_some() {
            policies[player.0].propose_private_library_choice(&view)
        } else if view.private_opponent_library_choice.is_some() {
            policies[player.0].propose_private_opponent_library_choice(&view)
        } else {
            policies[player.0].propose_move(&view)
        };
        attempted += 1;
        game.submit_policy_move(player, policies[player.0].id(), action)
            .map_err(|error| {
                format!(
                    "Three Dreams policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        if spell.is_none() {
            spell = game.event_log.iter().find_map(|event| match event {
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
        }
    }
    let winner = game.winner().map(|player| player.0);
    Ok(CapturedMatch {
        events: game.event_log.clone(),
        terminal: winner.map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({seat}))"),
        ),
        winner,
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempted.saturating_sub(accepted),
        spell,
        option_definitions,
        selected,
        selected_names,
        private_to_controller,
        hidden_from_opponent,
    })
}

fn rav_triggered_bindings() -> Vec<cardbench_magic_engine::TriggeredAbilityBinding> {
    let mut bindings = cardbench_magic_rav::rav_triggered_ability_bindings();
    bindings.extend(attachment_triggers());
    bindings
}

struct ThreeDreamsPolicy {
    player: PlayerId,
    has_cast_spell: bool,
}

impl ThreeDreamsPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            has_cast_spell: false,
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

    fn mana_action(view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
        let mut needed = BTreeSet::new();
        for color in &cost.colored {
            if view.mana_pool.amount(*color) == 0 {
                needed.insert(*color);
            }
        }
        let land = view
            .own_battlefield
            .iter()
            .find(|card| {
                !card.tapped
                    && card.card_types.contains(&CardType::Land)
                    && (needed.is_empty()
                        || card.mana_colors.iter().any(|color| needed.contains(color)))
            })
            .or_else(|| {
                view.own_battlefield
                    .iter()
                    .find(|card| !card.tapped && card.card_types.contains(&CardType::Land))
            })?;
        let color = needed
            .iter()
            .copied()
            .find(|color| land.mana_colors.contains(color))
            .or_else(|| land.mana_colors.iter().next().copied())?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }

    fn cast(
        view: &GameView,
        card: cardbench_magic_engine::ObjectId,
        cost: ManaCost,
    ) -> Option<PolicyAction> {
        let mana_selection = Self::payment(view, &cost)?;
        Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection,
        })
    }
}

impl CodePolicy for ThreeDreamsPolicy {
    fn id(&self) -> &'static str {
        "rav.three-dreams-full-game.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player {
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
                    .filter(|card| card.definition == Some("RAV-WATCHWOLF") && card.can_attack)
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
        if view.decision_player != self.player
            || view.priority != self.player
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
                .find(|card| card.definition == Some("RAV-FOREST"))
                .or_else(|| {
                    view.hand
                        .iter()
                        .find(|card| card.card_types.contains(&CardType::Land))
                })
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        let dreams_cost = ManaCost::with_colors(4, [Color::White]);
        if !self.has_cast_spell {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            {
                if let Some(action) = Self::cast(view, card.id, dreams_cost.clone()) {
                    self.has_cast_spell = true;
                    return action;
                }
                if let Some(action) = Self::mana_action(view, &dreams_cost) {
                    return action;
                }
            }
        }
        let watchwolf_cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
        if let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some("RAV-WATCHWOLF"))
        {
            if let Some(action) = Self::cast(view, card.id, watchwolf_cost.clone()) {
                return action;
            }
            if let Some(action) = Self::mana_action(view, &watchwolf_cost) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(mode: SelectionMode, capture: &CapturedMatch) -> Result<(), String> {
    if capture.terminal != "Winner(PlayerId(0))"
        || capture.winner != Some(0)
        || capture.rejected_moves != 0
    {
        return Err(
            "Three Dreams duel did not finish in a clean seat-zero full-game win".to_owned(),
        );
    }
    let source = capture.spell.ok_or("Three Dreams was not cast")?;
    let paid = capture
        .events
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if *card == source => Some(colors),
            _ => None,
        })
        .ok_or("Three Dreams payment receipt is missing")?;
    if paid.len() != 5 || paid.iter().filter(|color| **color == Color::White).count() != 1 {
        return Err("Three Dreams was not paid with exact {4}{W}".to_owned());
    }
    if !capture.private_to_controller
        || !capture.hidden_from_opponent
        || capture.option_definitions.len() < 3
    {
        return Err(
            "Three Dreams Aura identities were not private with at least three legal options"
                .to_owned(),
        );
    }
    if mode == SelectionMode::ThreeDistinct {
        if capture.selected.len() != 3
            || capture.selected_names.len() != 3
            || capture.selected_names.iter().collect::<BTreeSet<_>>().len() != 3
        {
            return Err("Three Dreams did not select three Auras with distinct names".to_owned());
        }
        if capture
            .selected_names
            .iter()
            .any(|name| !capture.option_definitions.contains(name))
        {
            return Err(
                "Three Dreams selected an Aura outside the legal private options".to_owned(),
            );
        }
    } else if !capture.selected.is_empty() || !capture.selected_names.is_empty() {
        return Err("Three Dreams zero-choice branch selected an Aura".to_owned());
    }
    let reveals: Vec<String> = capture
        .events
        .iter()
        .filter_map(|event| match event {
            GameEvent::CardRevealed {
                player: PlayerId(0),
                card,
                definition,
            } if capture.selected.contains(card) => Some((*definition).to_owned()),
            _ => None,
        })
        .collect();
    let moved: Vec<String> = capture
        .events
        .iter()
        .filter_map(|event| match event {
            GameEvent::CardMoved {
                card,
                to: Zone::Hand,
            } => capture
                .selected
                .iter()
                .position(|selected| selected == card)
                .and_then(|index| capture.selected_names.get(index).cloned()),
            _ => None,
        })
        .collect();
    if reveals != capture.selected_names || moved != capture.selected_names {
        return Err(
            "Three Dreams revealed and transferred cards do not equal the exact selection"
                .to_owned(),
        );
    }
    let search_index = capture.events.iter().position(|event| matches!(
        event, GameEvent::LibrarySearchBatchResolved { player: PlayerId(0), source: event_source, found, destination: LibrarySearchDestination::Hand }
            if *event_source == source && *found == capture.selected
    )).ok_or("Three Dreams batch search receipt differs from its submitted selection")?;
    let opened = capture
        .events
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::DecisionOpened {
                    player: PlayerId(0),
                    kind: DecisionKind::LibrarySearch,
                    visibility: DecisionVisibility::Private,
                    ..
                }
            )
        })
        .ok_or("Three Dreams private search decision was not opened")?;
    let completed = capture
        .events
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::DecisionCompleted {
                    player: PlayerId(0),
                    kind: DecisionKind::LibrarySearch,
                    ..
                }
            )
        })
        .ok_or("Three Dreams private search decision was not completed")?;
    let shuffle = capture
        .events
        .iter()
        .enumerate()
        .skip(search_index + 1)
        .find_map(|(index, event)| {
            matches!(
                event,
                GameEvent::LibraryShuffled {
                    player: PlayerId(0),
                    ..
                }
            )
            .then_some(index)
        })
        .ok_or("Three Dreams did not shuffle after its search")?;
    let resolved = capture
        .events
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == source))
        .ok_or("Three Dreams resolution receipt is missing")?;
    if !(opened < completed
        && completed < search_index
        && search_index < shuffle
        && shuffle < resolved)
    {
        return Err(
            "Three Dreams private-choice, search, shuffle, and resolution event order differs"
                .to_owned(),
        );
    }
    for card in &capture.selected {
        let reveal = capture.events.iter().position(|event| matches!(event, GameEvent::CardRevealed { card: revealed, .. } if revealed == card))
            .ok_or("Three Dreams selected Aura was not revealed")?;
        let move_to_hand = capture.events.iter().position(|event| matches!(event, GameEvent::CardMoved { card: moved, to: Zone::Hand } if moved == card))
            .ok_or("Three Dreams selected Aura was not moved to hand")?;
        if !(completed < reveal && reveal < move_to_hand && move_to_hand < search_index) {
            return Err(
                "Three Dreams reveal and hand transfer were not ordered before shuffle".to_owned(),
            );
        }
    }
    if !capture.events.iter().any(|event| {
        matches!(
            event,
            GameEvent::DamageDealtToPlayer {
                player: PlayerId(1),
                ..
            }
        )
    }) || !capture.events.iter().any(|event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    }) {
        return Err("Three Dreams duel did not finish through combat".to_owned());
    }
    Ok(())
}

fn mutation_detected(seed: u64, mode: SelectionMode, mutation: Mutation) -> bool {
    run_match(seed, mode, mutation)
        .and_then(|capture| validate_trace(mode, &capture))
        .is_err()
}

fn trace_json(capture: &CapturedMatch, mode: SelectionMode, replay_matches: bool) -> Value {
    let source = capture.spell;
    let selected_set: BTreeSet<_> = capture.selected.iter().copied().collect();
    let revealed: Vec<String> = capture
        .events
        .iter()
        .filter_map(|event| match event {
            GameEvent::CardRevealed {
                card, definition, ..
            } if selected_set.contains(card) => Some((*definition).to_owned()),
            _ => None,
        })
        .collect();
    let hand_moved: Vec<String> = capture
        .events
        .iter()
        .filter_map(|event| match event {
            GameEvent::CardMoved {
                card,
                to: Zone::Hand,
            } => capture
                .selected
                .iter()
                .position(|selected| selected == card)
                .and_then(|index| capture.selected_names.get(index).cloned()),
            _ => None,
        })
        .collect();
    let search_index = capture.events.iter().position(|event| matches!(event, GameEvent::LibrarySearchBatchResolved { source: event_source, .. } if Some(*event_source) == source));
    let shuffled = search_index.is_some_and(|index| {
        capture
            .events
            .iter()
            .enumerate()
            .skip(index + 1)
            .any(|(_, event)| {
                matches!(
                    event,
                    GameEvent::LibraryShuffled {
                        player: PlayerId(0),
                        ..
                    }
                )
            })
    });
    let terminal_combat_win = capture.winner == Some(0)
        && capture.events.iter().any(|event| {
            matches!(
                event,
                GameEvent::DamageDealtToPlayer {
                    player: PlayerId(1),
                    ..
                }
            )
        });
    let paid_colors = source.and_then(|source| {
        capture.events.iter().find_map(|event| match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if *card == source => Some(
                colors
                    .iter()
                    .map(|color| format!("{color:?}"))
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
    });
    let relevant: Vec<usize> = capture
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::SpellManaPaid { card, .. }
            | GameEvent::SpellCast { card, .. }
            | GameEvent::SpellResolved { card }
                if Some(*card) == source =>
            {
                Some(index)
            }
            GameEvent::DecisionOpened {
                player: PlayerId(0),
                kind: DecisionKind::LibrarySearch,
                visibility: DecisionVisibility::Private,
                ..
            }
            | GameEvent::DecisionCompleted {
                player: PlayerId(0),
                kind: DecisionKind::LibrarySearch,
                ..
            } => Some(index),
            GameEvent::CardRevealed { card, .. } if selected_set.contains(card) => Some(index),
            GameEvent::CardMoved {
                card,
                to: Zone::Hand,
            } if selected_set.contains(card) => Some(index),
            GameEvent::LibrarySearchBatchResolved {
                source: event_source,
                ..
            } if Some(*event_source) == source => Some(index),
            GameEvent::LibraryShuffled {
                player: PlayerId(0),
                ..
            } => Some(index),
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } if !attackers.is_empty() => Some(index),
            GameEvent::DamageDealtToPlayer {
                player: PlayerId(1),
                ..
            }
            | GameEvent::GameEnded { .. } => Some(index),
            _ => None,
        })
        .collect();
    let projected = project_events(&capture.events);
    let events: Vec<Value> = relevant.into_iter().filter_map(|index| {
        let event: &ProjectedEvent = projected.get(index)?;
        Some(json!({"sequence": event.sequence, "turn": event.turn, "kind": event.kind, "facts": event.facts, "canonical": event.canonical}))
    }).collect();
    json!({
        "trace_id": if mode == SelectionMode::ThreeDistinct { "rav-three-dreams-select-three-distinct-auras" } else { "rav-three-dreams-select-zero" },
        "terminal": capture.terminal, "winner": capture.winner, "turns": capture.turns,
        "accepted_policy_moves": capture.accepted_moves, "rejected_policy_moves": capture.rejected_moves,
        "mechanics": {
            "mana_cost": "{4}{W}", "paid_mana_colors": paid_colors,
            "legal_aura_option_count": capture.option_definitions.len(),
            "selected_count": capture.selected.len(), "selected_aura_names": capture.selected_names,
            "revealed_aura_names": revealed, "hand_aura_names": hand_moved,
            "private_to_controller": capture.private_to_controller,
            "hidden_from_opponent": capture.hidden_from_opponent,
            "library_shuffled_after_search": shuffled,
            "deterministic_replay": replay_matches, "terminal_combat_win": terminal_combat_win
        },
        "events": events
    })
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 32;
    json!({"collector_number": card.collector_number, "name": card.name, "definition_id": if passed { definition_id } else { None }, "passed": passed, "credited": false, "reward": 0.0})
}
