//! Full-game diagnostic authority for the RAV Farseek task.

use cardbench_magic_engine::{
    BasicLandType, CardType, Color, DecisionKind, DecisionVisibility, DeckEntry, DeckList, Effect,
    Game, GameEvent, GameView, LibrarySearchDestination, LibrarySearchRequirement,
    LibrarySearchSelection, ManaCost, ManaPaymentSelection, PlayerId, PolicyAction, Step, Zone,
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
const CARD_ID: &str = "RAV-FARSEEK";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/farseek.rs";
const MAX_TRACE_SEED: u64 = 1;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/farseek.rs"]
mod candidate_farseek;

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
            eprintln!("Farseek PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_definition()?;
    let catalog = unique_catalog()?;
    let seed = find_trace_seed()?;
    let reference = run_match(seed, Mutation::None)?;
    validate_trace(&reference)?;
    let replay = run_match(seed, Mutation::None)?;
    if !same_match(&reference, &replay) {
        return Err("Farseek full-game replay was not deterministic".to_owned());
    }
    let mutation_sensitivity = json!({
        "forest_allowed_by_search_mutation_detected_in_game": mutation_detected(seed, Mutation::AllowForest),
        "tapped_destination_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::UntappedDestination),
        "private_selection_reveal_mutation_detected_in_game": mutation_detected(seed, Mutation::RevealSelected),
        "sealed_mutation_authority": false
    });
    if mutation_sensitivity["forest_allowed_by_search_mutation_detected_in_game"] != true
        || mutation_sensitivity["tapped_destination_removed_mutation_detected_in_game"] != true
        || mutation_sensitivity["private_selection_reveal_mutation_detected_in_game"] != true
    {
        return Err(format!(
            "a Farseek semantic mutation escaped the gameplay oracle: {mutation_sensitivity}"
        ));
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 163, "definition_id": CARD_ID, "name": "Farseek"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": ["305: lands", "601.2: casting spells", "608.2: resolving spells and abilities", "701.19: searching a library", "701.16: revealing cards", "400.2: public and hidden zones"],
            "source_contract": "RAV Farseek is a green Sorcery for {1}{G}; its controller may search their library for a Plains, Island, Swamp, or Mountain card, put it onto the battlefield tapped, and shuffle.",
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
        "qualification_status": "unreviewed_diagnostic",
        "mutation_sensitivity": mutation_sensitivity,
        "traces": [trace_json(&reference)],
        "per_card_breakdown": breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Farseek source: {error}"))?;
    let file =
        syn::parse_file(&source).map_err(|_| "Farseek source is not valid Rust".to_owned())?;
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "BasicLandType"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
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
                    return Err("Farseek source has unsupported imports".to_owned());
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
            _ => return Err("Farseek module structure differs from its source contract".to_owned()),
        }
    }
    if imports != expected_imports || definition_count != 1 || file.items.len() != 3 {
        return Err("Farseek module structure differs from its source contract".to_owned());
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
    let definition = candidate_farseek::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Farseek definition is missing from the RAV registry")?;
    if definition != registered
        || definition.id != CARD_ID
        || definition.name != "Farseek"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(1, [Color::Green])
        || definition.colors != BTreeSet::from([Color::Green])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Sorcery])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
        || definition.effects != expected_effects(Mutation::None)
    {
        return Err("Farseek definition is outside its literal source contract".to_owned());
    }
    Ok(())
}

fn expected_effects(mutation: Mutation) -> Vec<Effect> {
    let mut requirement = BTreeSet::from([
        BasicLandType::Plains,
        BasicLandType::Island,
        BasicLandType::Swamp,
        BasicLandType::Mountain,
    ]);
    if mutation == Mutation::AllowForest {
        requirement.insert(BasicLandType::Forest);
    }
    vec![Effect::SearchControllerLibrary {
        requirement: LibrarySearchRequirement::BasicLandTypes(requirement),
        destination: if mutation == Mutation::UntappedDestination {
            LibrarySearchDestination::Battlefield
        } else {
            LibrarySearchDestination::BattlefieldTapped
        },
        selection: LibrarySearchSelection::PolicySubmitted {
            may_fail_to_find: true,
        },
        reveal_selected: mutation == Mutation::RevealSelected,
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
    for seed in 0..MAX_TRACE_SEED {
        if let Ok(capture) = run_match(seed, Mutation::None)
            && validate_trace(&capture).is_ok()
        {
            return Ok(seed);
        }
    }
    Err("no bounded shuffle seed completed the Farseek tapped Plains search game".to_owned())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    AllowForest,
    UntappedDestination,
    RevealSelected,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    farseek: Option<cardbench_magic_engine::ObjectId>,
    selected: Option<cardbench_magic_engine::ObjectId>,
    selected_definition: Option<String>,
    option_definitions: Vec<String>,
    private_to_controller: bool,
    hidden_from_opponent: bool,
    tapped_at_resolution: bool,
    selected_is_plains: bool,
    selected_zone_battlefield: bool,
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definitions = card_definitions();
    let definition = definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Farseek definition missing from registry")?;
    let mut candidate_definition = candidate_farseek::definition();
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
                count: 16,
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
        Box::new(FarseekPolicy::new(PlayerId(0))),
        Box::new(FarseekPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut farseek = None;
    let mut selected = None;
    let mut selected_definition = None;
    let mut option_definitions = Vec::new();
    let mut private_to_controller = false;
    let mut hidden_from_opponent = false;
    let mut tapped_at_resolution = false;
    let mut selected_is_plains = false;
    let mut selected_zone_battlefield = false;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(format!(
                "Farseek duel exceeded bounds at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let action = if let Some(choice_view) = view.library_search_choice.as_ref() {
            private_to_controller = choice_view.destination
                == LibrarySearchDestination::BattlefieldTapped
                && view
                    .pending_decision
                    .as_ref()
                    .is_some_and(|decision| decision.visibility == DecisionVisibility::Private);
            let opponent_view = game
                .view_for_player(PlayerId(1))
                .map_err(|error| error.to_string())?;
            hidden_from_opponent = opponent_view.library_search_choice.is_none()
                && opponent_view
                    .pending_decision
                    .as_ref()
                    .is_none_or(|decision| decision.visibility != DecisionVisibility::Private);
            option_definitions = choice_view
                .cards
                .iter()
                .filter_map(|card| card.definition.map(str::to_owned))
                .collect();
            let choice = choice_view
                .cards
                .iter()
                .find(|card| card.definition == Some("RAV-PLAINS"));
            selected = choice.map(|card| card.id);
            selected_definition = choice.and_then(|card| card.definition.map(str::to_owned));
            PolicyAction::ChooseLibrarySearchCard {
                decision: choice_view.decision,
                source: choice_view.source,
                selected,
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
        if matches!(action, PolicyAction::ChooseLibrarySearchCard { .. }) {
            if let Some(action_source) = farseek.or_else(|| {
                game.event_log.iter().find_map(|event| match event {
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
                })
            }) {
                farseek = Some(action_source);
            }
        }
        attempted += 1;
        game.submit_policy_move(player, policies[player.0].id(), action)
            .map_err(|error| {
                format!(
                    "Farseek policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        if farseek.is_none() {
            farseek = game.event_log.iter().find_map(|event| match event {
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
        if let Some(land) = selected {
            if !selected_zone_battlefield
                && selected_definition.as_deref() == Some("RAV-PLAINS")
                && game.zone_of(land) == Some(Zone::Battlefield)
            {
                selected_is_plains = game
                    .basic_land_type(land)
                    .map_err(|error| error.to_string())?
                    == Some(BasicLandType::Plains);
                selected_zone_battlefield = true;
                tapped_at_resolution = game.object(land).map_err(|error| error.to_string())?.tapped;
            }
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
        farseek,
        selected,
        selected_definition,
        option_definitions,
        private_to_controller,
        hidden_from_opponent,
        tapped_at_resolution,
        selected_is_plains,
        selected_zone_battlefield,
    })
}

fn rav_triggered_bindings() -> Vec<cardbench_magic_engine::TriggeredAbilityBinding> {
    let mut bindings = cardbench_magic_rav::rav_triggered_ability_bindings();
    bindings.extend(attachment_triggers());
    bindings
}

struct FarseekPolicy {
    player: PlayerId,
    has_cast_farseek: bool,
}

impl FarseekPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            has_cast_farseek: false,
        }
    }
}

impl FarseekPolicy {
    fn land_to_tap(
        &self,
        view: &GameView,
        color: Color,
    ) -> Option<cardbench_magic_engine::ObjectId> {
        view.own_battlefield
            .iter()
            .find(|card| {
                !card.tapped
                    && card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&color)
            })
            .map(|card| card.id)
    }
    fn mana_action(&self, view: &GameView) -> Option<PolicyAction> {
        let has_farseek = !self.has_cast_farseek
            && view
                .hand
                .iter()
                .any(|card| card.definition == Some(CARD_ID));
        if has_farseek {
            if view.mana_pool.amount(Color::Green) < 2 {
                return self.land_to_tap(view, Color::Green).map(|land| {
                    PolicyAction::ActivateManaAbility {
                        land,
                        color: Color::Green,
                    }
                });
            }
        } else if view
            .hand
            .iter()
            .any(|card| card.definition == Some("RAV-WATCHWOLF"))
        {
            for color in [Color::Green, Color::White] {
                if view.mana_pool.amount(color) == 0 {
                    if let Some(land) = self.land_to_tap(view, color) {
                        return Some(PolicyAction::ActivateManaAbility { land, color });
                    }
                }
            }
        }
        None
    }
}

impl CodePolicy for FarseekPolicy {
    fn id(&self) -> &'static str {
        "rav.farseek-full-game.v1"
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
        if !self.has_cast_farseek && view.mana_pool.amount(Color::Green) >= 2 {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            {
                self.has_cast_farseek = true;
                return PolicyAction::CastWithPayment {
                    request: cardbench_magic_engine::CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: ManaPaymentSelection {
                        generic: vec![Color::Green],
                        hybrid: vec![],
                    },
                };
            }
        }
        if let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some("RAV-WATCHWOLF"))
            && view.mana_pool.amount(Color::Green) > 0
            && view.mana_pool.amount(Color::White) > 0
        {
            return PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection: ManaPaymentSelection {
                    generic: vec![],
                    hybrid: vec![],
                },
            };
        }
        if let Some(action) = self.mana_action(view) {
            return action;
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(capture: &CapturedMatch) -> Result<(), String> {
    if capture.terminal != "Winner(PlayerId(0))" || capture.rejected_moves != 0 {
        return Err("Farseek trace did not end in a clean P0 full-game win".to_owned());
    }
    let source = capture.farseek.ok_or("Farseek was not cast")?;
    let land = capture
        .selected
        .ok_or("Farseek did not choose a legal Plains")?;
    if capture.selected_definition.as_deref() != Some("RAV-PLAINS")
        || !capture.selected_is_plains
        || !capture.selected_zone_battlefield
        || !capture.tapped_at_resolution
    {
        return Err(
            "Farseek did not put the selected Plains onto the battlefield tapped".to_owned(),
        );
    }
    if !capture.private_to_controller || !capture.hidden_from_opponent {
        return Err(
            "Farseek library candidate identities were not private to the searching player"
                .to_owned(),
        );
    }
    if capture.option_definitions.len() < 4
        || capture
            .option_definitions
            .iter()
            .any(|id| id == "RAV-FOREST")
        || !["RAV-PLAINS", "RAV-ISLAND", "RAV-SWAMP", "RAV-MOUNTAIN"]
            .iter()
            .all(|id| {
                capture
                    .option_definitions
                    .iter()
                    .any(|candidate| candidate == id)
            })
    {
        return Err("Farseek candidate options did not contain the four legal non-Forest basic land types exclusively".to_owned());
    }
    let mana_paid = capture.events.iter().any(|event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == source && colors.len() == 2 && colors.contains(&Color::Green)));
    let cast = capture.events.iter().any(|event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == source));
    let resolved = capture
        .events
        .iter()
        .any(|event| matches!(event, GameEvent::SpellResolved { card } if *card == source));
    let search = capture.events.iter().find_map(|event| match event {
        GameEvent::LibrarySearchResolved {
            player: PlayerId(0),
            source: event_source,
            found,
            destination: LibrarySearchDestination::BattlefieldTapped,
        } if *event_source == source => Some(*found),
        _ => None,
    });
    if !mana_paid || !cast || !resolved || search.flatten() != Some(land) {
        return Err(
            "Farseek payment, cast, resolution, or typed tapped-search receipt is missing"
                .to_owned(),
        );
    }
    if !capture.events.iter().any(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == land))
        || capture.events.iter().any(|event| matches!(event, GameEvent::CardRevealed { card, .. } if *card == land)) {
        return Err("Farseek selected Plains movement or no-reveal semantics differ".to_owned());
    }
    let search_index = capture.events.iter().position(|event| matches!(event, GameEvent::LibrarySearchResolved { source: event_source, .. } if *event_source == source)).ok_or("Farseek search receipt missing")?;
    let shuffle_index = capture
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
        });
    if shuffle_index.is_none() {
        return Err("Farseek did not shuffle its library after the search".to_owned());
    }
    if !capture.events.iter().any(|event| {
        matches!(
            event,
            GameEvent::DamageDealtToPlayer {
                player: PlayerId(1),
                ..
            }
        )
    }) || capture.winner != Some(0)
    {
        return Err(
            "Farseek PvP trace did not complete through combat to a terminal win".to_owned(),
        );
    }
    Ok(())
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    run_match(seed, mutation)
        .and_then(|capture| validate_trace(&capture))
        .is_err()
}

fn same_match(left: &CapturedMatch, right: &CapturedMatch) -> bool {
    left == right
}

fn trace_json(capture: &CapturedMatch) -> Value {
    let source = capture.farseek;
    let selected = capture.selected;
    let search_index = capture.events.iter().position(|event| matches!(event, GameEvent::LibrarySearchResolved { source: event_source, .. } if Some(*event_source) == source));
    let shuffle_after_search = search_index.is_some_and(|search_index| {
        capture
            .events
            .iter()
            .enumerate()
            .skip(search_index + 1)
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
    let relevant_indices: Vec<usize> = capture
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
            GameEvent::LibrarySearchResolved {
                source: event_source,
                ..
            } if Some(*event_source) == source => Some(index),
            GameEvent::CardMoved {
                card,
                to: Zone::Battlefield,
            } if Some(*card) == selected => Some(index),
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
    let events: Vec<Value> = relevant_indices.into_iter().filter_map(|index| {
        let event: &ProjectedEvent = projected.get(index)?;
        Some(json!({"sequence": event.sequence, "turn": event.turn, "kind": event.kind, "facts": event.facts, "canonical": event.canonical}))
    }).collect();
    json!({
        "trace_id": "rav-farseek-search-nonforest-basic-land-tapped",
        "terminal": capture.terminal,
        "winner": capture.winner,
        "turns": capture.turns,
        "accepted_policy_moves": capture.accepted_moves,
        "rejected_policy_moves": capture.rejected_moves,
        "mechanics": {
            "mana_cost": "{1}{G}",
            "paid_mana_colors": source.and_then(|source| capture.events.iter().find_map(|event| match event { GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == source => Some(colors.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>()), _ => None })),
            "private_to_controller": capture.private_to_controller,
            "hidden_from_opponent": capture.hidden_from_opponent,
            "legal_candidate_count": capture.option_definitions.len(),
            "candidate_definitions": capture.option_definitions,
            "forest_excluded": !capture.option_definitions.iter().any(|id| id == "RAV-FOREST"),
            "selected_definition": capture.selected_definition,
            "selected_is_plains": capture.selected_is_plains,
            "destination": "BattlefieldTapped",
            "selected_tapped_at_resolution": capture.tapped_at_resolution,
            "selected_land_battlefield": capture.selected_zone_battlefield,
            "selected_land_revealed": selected.is_some_and(|selected| capture.events.iter().any(|event| matches!(event, GameEvent::CardRevealed { card, .. } if *card == selected))),
            "library_shuffled_after_search": shuffle_after_search,
            "combat_to_terminal_win": capture.winner == Some(0) && capture.events.iter().any(|event| matches!(event, GameEvent::DamageDealtToPlayer { player: PlayerId(1), .. }))
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
    let passed = card.collector_number == 163;
    json!({"collector_number": card.collector_number, "name": card.name, "definition_id": if passed { definition_id } else { None }, "passed": passed, "credited": false, "reward": 0.0})
}
