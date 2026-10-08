//! Full-game diagnostic authority for the RAV Civic Wayfinder task.

use cardbench_magic_engine::{
    BasicLandType, CardType, Color, DecisionKind, DecisionVisibility, DeckEntry, DeckList, Effect,
    Game, GameEvent, GameView, LibrarySearchDestination, LibrarySearchRequirement,
    LibrarySearchSelection, ManaCost, PlayerId, PolicyAction, Step, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding, Zone,
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
    rav_static_library_top_reveal_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-CIVIC-WAYFINDER";
const TRIGGER_ID: &str = "etb-search-basic-land-to-hand";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/civic_wayfinder.rs";
const MAX_TRACE_SEED: u64 = 128;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/civic_wayfinder.rs"]
mod candidate_civic_wayfinder;

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
            eprintln!("Civic Wayfinder PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_bindings()?;
    let catalog = unique_catalog()?;
    let seed = find_trace_seed()?;

    let found = run_match(seed, SearchChoice::FindPlains, Mutation::None)?;
    validate_trace(&found, SearchChoice::FindPlains)?;
    let declined = run_match(seed, SearchChoice::Decline, Mutation::None)?;
    validate_trace(&declined, SearchChoice::Decline)?;
    for (capture, choice) in [
        (&found, SearchChoice::FindPlains),
        (&declined, SearchChoice::Decline),
    ] {
        let replay = run_match(seed, choice, Mutation::None)?;
        if !same_match(capture, &replay) {
            return Err(
                "Civic Wayfinder shuffled full-game replay was not deterministic".to_owned(),
            );
        }
    }

    let mutation_sensitivity = json!({
        "etb_trigger_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::NoTrigger, SearchChoice::FindPlains),
        "plains_omitted_from_search_mutation_detected_in_game": mutation_detected(seed, Mutation::ForestOnly, SearchChoice::FindPlains),
        "selected_land_reveal_removed_mutation_detected_in_game": mutation_detected(seed, Mutation::NoReveal, SearchChoice::FindPlains),
        "sealed_mutation_authority": false
    });
    if mutation_sensitivity["etb_trigger_removed_mutation_detected_in_game"] != true
        || mutation_sensitivity["plains_omitted_from_search_mutation_detected_in_game"] != true
        || mutation_sensitivity["selected_land_reveal_removed_mutation_detected_in_game"] != true
    {
        return Err(format!(
            "a Civic Wayfinder semantic mutation escaped the gameplay oracle: {mutation_sensitivity}"
        ));
    }

    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 157, "definition_id": CARD_ID, "name": "Civic Wayfinder"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117: timing and priority",
                "302.1 and 601.2: creature spells and casting",
                "603.2 and 603.3: triggered abilities",
                "608.2: resolving spells and abilities",
                "701.19: searching a library",
                "701.16: revealing cards"
            ],
            "source_contract": "RAV Civic Wayfinder is a green 2/2 Creature for {2}{G}; its mandatory ETB trigger lets its controller search for a basic land card, may fail to find, reveals a selected card, puts it into hand, and shuffles.",
            "authority_cutoff": "2026-09-23"
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
        "mutation_sensitivity": mutation_sensitivity,
        "traces": [trace_json(&found, SearchChoice::FindPlains), trace_json(&declined, SearchChoice::Decline)],
        "per_card_breakdown": breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Civic Wayfinder source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Civic Wayfinder source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
    {
        return Err("Civic Wayfinder source has an unsupported module attribute".to_owned());
    }
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
        vec!["cardbench_magic_engine", "TriggerCondition"],
        vec!["cardbench_magic_engine", "TriggeredAbility"],
        vec!["cardbench_magic_engine", "TriggeredAbilityBinding"],
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
                let mut paths = Vec::new();
                if item.leading_colon.is_some() || !flatten_use_tree(&[], &item.tree, &mut paths) {
                    return Err("Civic Wayfinder source has unsupported imports".to_owned());
                }
                imports.extend(paths);
            }
            Item::Fn(item)
                if item.attrs.iter().all(|attribute| {
                    attribute.path().is_ident("doc")
                        || attribute.path().is_ident("must_use")
                            && matches!(attribute.meta, syn::Meta::Path(_))
                }) && matches!(&item.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    && item.sig.inputs.is_empty()
                    && item.sig.generics.params.is_empty()
                    && item.sig.generics.where_clause.is_none()
                    && item.sig.constness.is_none()
                    && item.sig.asyncness.is_none()
                    && item.sig.unsafety.is_none()
                    && item.sig.abi.is_none()
                    && item.sig.variadic.is_none()
                    && item.block.stmts.len() == 1 =>
            {
                let expected_type = match item.sig.ident.to_string().as_str() {
                    "definition" => "CardDefinition",
                    "triggered_ability" => "TriggeredAbilityBinding",
                    _ => return Err("Civic Wayfinder has an unsupported function".to_owned()),
                };
                if !matches!(&item.sig.output, syn::ReturnType::Type(_, ty) if type_is(ty, expected_type))
                    || !matches!(&item.block.stmts[0], Stmt::Expr(Expr::Struct(value), None) if !value.attrs.is_empty() == false && value.rest.is_none() && path_is_type_path(&value.path, expected_type))
                {
                    return Err(
                        "Civic Wayfinder functions must return direct typed literals".to_owned(),
                    );
                }
                if !functions.insert(item.sig.ident.to_string()) {
                    return Err("Civic Wayfinder has duplicate functions".to_owned());
                }
            }
            _ => {
                return Err(
                    "Civic Wayfinder source contains an unsupported top-level item".to_owned(),
                );
            }
        }
    }
    if imports != expected_imports
        || functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()])
        || file.items.len() != 4
    {
        return Err("Civic Wayfinder module structure differs from the source contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_bindings() -> Result<(), String> {
    let definition = candidate_civic_wayfinder::definition();
    let expected_definition = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Civic Wayfinder definition is missing from the RAV registry")?;
    if definition != expected_definition
        || definition.name != "Civic Wayfinder"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(2, [Color::Green])
        || definition.colors != BTreeSet::from([Color::Green])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.power != Some(2)
        || definition.toughness != Some(2)
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
    {
        return Err("Civic Wayfinder literal is outside the task source contract".to_owned());
    }
    let binding = candidate_civic_wayfinder::triggered_ability();
    let registered = rav_triggered_ability_bindings()
        .into_iter()
        .find(|candidate| candidate.card_definition == CARD_ID)
        .ok_or("Civic Wayfinder ETB search trigger is missing from the RAV registry")?;
    if binding != registered || binding != expected_trigger() {
        return Err(
            "Civic Wayfinder trigger literal is outside the task source contract".to_owned(),
        );
    }
    Ok(())
}

fn expected_trigger() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: CARD_ID,
        ability: TriggeredAbility {
            id: TRIGGER_ID,
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::SearchControllerLibrary {
                requirement: LibrarySearchRequirement::BasicLandTypes(BTreeSet::from([
                    BasicLandType::Plains,
                    BasicLandType::Island,
                    BasicLandType::Swamp,
                    BasicLandType::Mountain,
                    BasicLandType::Forest,
                ])),
                destination: LibrarySearchDestination::Hand,
                selection: LibrarySearchSelection::PolicySubmitted {
                    may_fail_to_find: true,
                },
                reveal_selected: true,
            }],
        },
    }
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
        let capture = run_match(seed, SearchChoice::FindPlains, Mutation::None)?;
        if capture.winner == Some(0)
            && capture.candidate.is_some()
            && capture.found_definition.as_deref() == Some("RAV-PLAINS")
            && capture.search_options > 0
        {
            return Ok(seed);
        }
    }
    Err("no bounded shuffle seed completed the Civic Wayfinder Plains search game".to_owned())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SearchChoice {
    FindPlains,
    Decline,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoTrigger,
    ForestOnly,
    NoReveal,
}

struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    candidate: Option<cardbench_magic_engine::ObjectId>,
    found: Option<cardbench_magic_engine::ObjectId>,
    found_definition: Option<String>,
    search_options: usize,
    requested_selection: Option<cardbench_magic_engine::ObjectId>,
    live_power: Option<i32>,
    live_toughness: Option<i32>,
    live_green: bool,
}

fn run_match(seed: u64, choice: SearchChoice, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definitions = card_definitions();
    let definition = definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Civic Wayfinder definition missing from registry")?;
    let civic_definition = candidate_civic_wayfinder::definition();
    *definition = civic_definition;

    let mut triggers = rav_triggered_ability_bindings();
    let trigger_index = triggers
        .iter()
        .position(|binding| binding.card_definition == CARD_ID)
        .ok_or("Civic Wayfinder trigger missing from registry")?;
    triggers[trigger_index] = candidate_civic_wayfinder::triggered_ability();
    match mutation {
        Mutation::None => {}
        Mutation::NoTrigger => {
            triggers.remove(trigger_index);
        }
        Mutation::ForestOnly => {
            let Effect::SearchControllerLibrary { requirement, .. } =
                &mut triggers[trigger_index].ability.effects[0]
            else {
                return Err("Civic Wayfinder mutation target is not a library search".to_owned());
            };
            *requirement =
                LibrarySearchRequirement::BasicLandTypes(BTreeSet::from([BasicLandType::Forest]));
        }
        Mutation::NoReveal => {
            let Effect::SearchControllerLibrary {
                reveal_selected, ..
            } = &mut triggers[trigger_index].ability.effects[0]
            else {
                return Err("Civic Wayfinder mutation target is not a library search".to_owned());
            };
            *reveal_selected = false;
        }
    }
    triggers.extend(attachment_triggers());
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers,
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
                card: "RAV-FOREST".to_owned(),
                count: 40,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-ISLAND".to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-SWAMP".to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 4,
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
        max_policy_moves: 8_000,
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
        Box::new(WayfinderPolicy::new(PlayerId(0))),
        Box::new(WayfinderPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut candidate = None;
    let mut live = None;
    let mut search_options = 0;
    let mut requested_selection = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(format!(
                "Civic Wayfinder duel exceeded bounds at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("policy seat is unavailable")?;
        let action = if let Some(choice_view) = view.library_search_choice.as_ref() {
            search_options = choice_view.cards.len();
            let selected = if choice == SearchChoice::Decline {
                None
            } else {
                choice_view
                    .cards
                    .iter()
                    .find(|card| card.definition == Some("RAV-PLAINS"))
                    .or_else(|| choice_view.cards.first())
                    .map(|card| card.id)
            };
            requested_selection = selected;
            PolicyAction::ChooseLibrarySearchCard {
                decision: choice_view.decision,
                source: choice_view.source,
                selected,
            }
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
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;

        if candidate.is_none() {
            candidate = game.event_log.iter().find_map(|event| match event {
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
        if let Some(id) = candidate
            && live.is_none()
            && game.zone_of(id) == Some(Zone::Battlefield)
        {
            let characteristics = game
                .characteristics(id)
                .map_err(|error| error.to_string())?;
            live = Some((
                characteristics.power.unwrap_or_default(),
                characteristics.toughness.unwrap_or_default(),
                characteristics.colors.contains(&Color::Green),
            ));
        }
    }
    let found = candidate.and_then(|candidate| {
        game.event_log.iter().find_map(|event| match event {
            GameEvent::LibrarySearchResolved {
                source,
                found,
                destination: LibrarySearchDestination::Hand,
                ..
            } if *source == candidate => *found,
            _ => None,
        })
    });
    let found_definition = found
        .map(|card| {
            game.card_definition(card)
                .map(|definition| definition.id.to_owned())
        })
        .transpose()
        .map_err(|error| error.to_string())?;
    let winner = game.winner().map(|player| player.0);
    let (live_power, live_toughness, live_green) = live.unwrap_or_default();
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
        candidate,
        found,
        found_definition,
        search_options,
        requested_selection,
        live_power: candidate.map(|_| live_power),
        live_toughness: candidate.map(|_| live_toughness),
        live_green,
    })
}

struct WayfinderPolicy {
    player: PlayerId,
}

impl WayfinderPolicy {
    fn new(player: PlayerId) -> Self {
        Self { player }
    }

    fn make_green(&self, view: &GameView) -> Option<PolicyAction> {
        view.own_battlefield
            .iter()
            .find(|card| {
                !card.tapped
                    && card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&Color::Green)
            })
            .map(|card| PolicyAction::ActivateManaAbility {
                land: card.id,
                color: Color::Green,
            })
    }
}

impl CodePolicy for WayfinderPolicy {
    fn id(&self) -> &'static str {
        "rav.civic-wayfinder-full-game.v1"
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
                    .filter(|card| card.definition == Some(CARD_ID) && card.can_attack)
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
            let land = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-FOREST"))
                .or_else(|| {
                    view.hand
                        .iter()
                        .find(|card| card.card_types.contains(&CardType::Land))
                });
            if let Some(land) = land {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        let already_cast = view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some(CARD_ID))
            || view
                .opponent_battlefield
                .iter()
                .any(|card| card.definition == Some(CARD_ID));
        if !already_cast && self.player == PlayerId(0) {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            {
                if view.mana_pool.amount(Color::Green) < 3 {
                    if let Some(action) = self.make_green(view) {
                        return action;
                    }
                } else {
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: cardbench_magic_engine::ManaPaymentSelection {
                            generic: vec![Color::Green, Color::Green],
                            hybrid: vec![],
                        },
                    };
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(capture: &CapturedMatch, choice: SearchChoice) -> Result<(), String> {
    if capture.terminal != "Winner(PlayerId(0))" || capture.rejected_moves != 0 {
        return Err("Civic Wayfinder trace did not end in a clean P0 win".to_owned());
    }
    let candidate = capture.candidate.ok_or("Civic Wayfinder was not cast")?;
    if capture.live_power != Some(2) || capture.live_toughness != Some(2) || !capture.live_green {
        return Err(
            "Civic Wayfinder live characteristics differ from its printed 2/2 green body"
                .to_owned(),
        );
    }
    let trigger_stacked = capture.events.iter().any(|event| matches!(event,
        GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, ability: TRIGGER_ID, .. } if *source == candidate));
    let ability_resolved = capture.events.iter().any(|event| {
        matches!(event,
        GameEvent::AbilityResolved { source, ability: TRIGGER_ID, .. } if *source == candidate)
    });
    let found = capture.events.iter().find_map(|event| match event {
        GameEvent::LibrarySearchResolved {
            player: PlayerId(0),
            source,
            found,
            destination: LibrarySearchDestination::Hand,
        } if *source == candidate => Some(*found),
        _ => None,
    });
    if !trigger_stacked || !ability_resolved || capture.search_options == 0 {
        return Err(
            "Civic Wayfinder mandatory ETB search decision did not resolve with live candidates"
                .to_owned(),
        );
    }
    match choice {
        SearchChoice::FindPlains => {
            let land = found
                .flatten()
                .ok_or("Civic Wayfinder selected branch found no land")?;
            if capture.found_definition.as_deref() != Some("RAV-PLAINS")
                || capture.requested_selection != Some(land)
            {
                return Err(
                    "Civic Wayfinder did not select the requested Plains from its legal candidates"
                        .to_owned(),
                );
            }
            if !capture.events.iter().any(|event| matches!(event,
                GameEvent::CardRevealed { player: PlayerId(0), card, definition: "RAV-PLAINS" } if *card == land))
                || !capture.events.iter().any(|event| matches!(event,
                    GameEvent::CardMoved { card, to: Zone::Hand } if *card == land))
            {
                return Err("Civic Wayfinder did not reveal the selected Plains and move it to hand".to_owned());
            }
        }
        SearchChoice::Decline => {
            if found.flatten().is_some() || capture.requested_selection.is_some() {
                return Err(
                    "Civic Wayfinder may-fail-to-find branch unexpectedly selected a card"
                        .to_owned(),
                );
            }
            if capture.events.iter().any(|event| {
                matches!(
                    event,
                    GameEvent::CardRevealed {
                        player: PlayerId(0),
                        ..
                    }
                )
            }) {
                return Err("Civic Wayfinder declined branch revealed a library card".to_owned());
            }
        }
    }
    if !capture.events.iter().any(|event| {
        matches!(
            event,
            GameEvent::LibraryShuffled {
                player: PlayerId(0),
                ..
            }
        )
    }) {
        return Err("Civic Wayfinder search did not shuffle its controller's library".to_owned());
    }
    if !capture.events.iter().any(|event| matches!(event,
        GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&candidate)))
        || !capture.events.iter().any(|event| matches!(event,
            GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 2 } if *source == candidate))
    {
        return Err("Civic Wayfinder did not attack and deal live 2-power combat damage".to_owned());
    }
    Ok(())
}

fn mutation_detected(seed: u64, mutation: Mutation, choice: SearchChoice) -> bool {
    run_match(seed, choice, mutation)
        .and_then(|capture| validate_trace(&capture, choice).map(|()| capture))
        .is_err()
}

fn same_match(left: &CapturedMatch, right: &CapturedMatch) -> bool {
    left.events == right.events
        && left.terminal == right.terminal
        && left.winner == right.winner
        && left.turns == right.turns
        && left.accepted_moves == right.accepted_moves
        && left.rejected_moves == right.rejected_moves
        && left.candidate == right.candidate
        && left.found == right.found
        && left.found_definition == right.found_definition
        && left.search_options == right.search_options
        && left.requested_selection == right.requested_selection
        && left.live_power == right.live_power
        && left.live_toughness == right.live_toughness
        && left.live_green == right.live_green
}

fn trace_json(capture: &CapturedMatch, choice: SearchChoice) -> Value {
    let candidate = capture.candidate;
    let source_events = &capture.events;
    let search_index = source_events.iter().position(|event| {
        matches!(event, GameEvent::LibrarySearchResolved { source, .. } if candidate == Some(*source))
    });
    let attack_index = source_events.iter().position(|event| {
        matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if candidate.is_some_and(|card| attackers.contains(&card)))
    });
    let first_after = |after: usize, predicate: &dyn Fn(&GameEvent) -> bool| {
        source_events
            .iter()
            .enumerate()
            .skip(after + 1)
            .find_map(|(index, event)| predicate(event).then_some(index))
    };
    let mut relevant_indices = Vec::new();
    if let Some(candidate) = candidate {
        relevant_indices.extend(
            source_events
                .iter()
                .enumerate()
                .filter_map(|(index, event)| match event {
                    GameEvent::SpellManaPaid {
                        player: PlayerId(0),
                        card,
                        ..
                    } if *card == candidate => Some(index),
                    GameEvent::SpellCast {
                        player: PlayerId(0),
                        card,
                    }
                    | GameEvent::SpellResolved { card }
                        if *card == candidate =>
                    {
                        Some(index)
                    }
                    GameEvent::CardMoved {
                        card,
                        to: Zone::Battlefield,
                    } if *card == candidate => Some(index),
                    GameEvent::TriggeredAbilityStacked {
                        source,
                        ability: TRIGGER_ID,
                        ..
                    } if *source == candidate => Some(index),
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
                    GameEvent::LibrarySearchResolved { source, .. } if *source == candidate => {
                        Some(index)
                    }
                    GameEvent::AbilityResolved {
                        source,
                        ability: TRIGGER_ID,
                        ..
                    } if *source == candidate => Some(index),
                    GameEvent::GameEnded { .. } => Some(index),
                    _ => None,
                }),
        );
    }
    if let Some(land) = capture.found {
        relevant_indices.extend(
            source_events
                .iter()
                .enumerate()
                .filter_map(|(index, event)| match event {
                    GameEvent::CardRevealed {
                        player: PlayerId(0),
                        card,
                        definition: "RAV-PLAINS",
                    } if *card == land => Some(index),
                    GameEvent::CardMoved {
                        card,
                        to: Zone::Hand,
                    } if *card == land => Some(index),
                    _ => None,
                }),
        );
    }
    if let Some(search_index) = search_index {
        if let Some(shuffle_index) = first_after(search_index, &|event| {
            matches!(
                event,
                GameEvent::LibraryShuffled {
                    player: PlayerId(0),
                    ..
                }
            )
        }) {
            relevant_indices.push(shuffle_index);
        }
    }
    if let Some(attack_index) = attack_index {
        relevant_indices.push(attack_index);
        if let Some(block_index) = first_after(attack_index, &|event| {
            matches!(
                event,
                GameEvent::BlockersDeclared {
                    player: PlayerId(1),
                    ..
                }
            )
        }) {
            relevant_indices.push(block_index);
            if let Some(damage_index) = first_after(
                block_index,
                &|event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), .. } if candidate == Some(*source)),
            ) {
                relevant_indices.push(damage_index);
            }
        }
    }
    relevant_indices.sort_unstable();
    relevant_indices.dedup();
    let projected = project_events(&capture.events);
    let events: Vec<Value> = relevant_indices
        .into_iter()
        .filter_map(|index| {
            let event: &ProjectedEvent = projected.get(index)?;
            Some(json!({
                "sequence": event.sequence,
                "turn": event.turn,
                "kind": event.kind,
                "facts": event.facts,
                "canonical": event.canonical
            }))
        })
        .collect();
    json!({
        "trace_id": match choice { SearchChoice::FindPlains => "rav-civic-wayfinder-find-reveal-plains", SearchChoice::Decline => "rav-civic-wayfinder-may-fail-to-find" },
        "terminal": capture.terminal,
        "winner": capture.winner,
        "turns": capture.turns,
        "accepted_policy_moves": capture.accepted_moves,
        "rejected_policy_moves": capture.rejected_moves,
        "mechanics": {
            "cast_from_shuffled_library": true,
            "mana_cost": "{2}{G}",
            "paid_mana_colors": capture.events.iter().find_map(|event| match event { GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if Some(*card) == candidate => Some(colors.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>()), _ => None }),
            "live_power_toughness": [capture.live_power, capture.live_toughness],
            "live_green": capture.live_green,
            "search_choice": match choice { SearchChoice::FindPlains => "find", SearchChoice::Decline => "decline" },
            "search_option_count": capture.search_options,
            "selected_definition": capture.found_definition,
            "selected_land_revealed": capture.found.is_some_and(|land| capture.events.iter().any(|event| matches!(event, GameEvent::CardRevealed { card, definition: "RAV-PLAINS", .. } if *card == land))),
            "selected_land_moved_to_hand": capture.found.is_some_and(|land| capture.events.iter().any(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == land))),
            "library_shuffled": capture.events.iter().any(|event| matches!(event, GameEvent::LibraryShuffled { player: PlayerId(0), .. })),
            "combat_damage": capture.events.iter().filter_map(|event| match event { GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount } if Some(*source) == candidate => Some(*amount), _ => None }).sum::<i32>()
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
    let passed = card.collector_number == 157;
    json!({
        "collector_number": card.collector_number,
        "name": card.name,
        "definition_id": definition_id,
        "engine_status": if definition_id.is_some() { "executable_compatibility_slice" } else { "catalog_only" },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "qualification": if passed { "unreviewed_diagnostic" } else { "not_evaluated" },
        "credited": false
    })
}

fn flatten_use_tree(prefix: &[String], tree: &UseTree, paths: &mut Vec<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(item) => {
            let mut next = prefix.to_vec();
            next.push(item.ident.to_string());
            flatten_use_tree(&next, &item.tree, paths)
        }
        UseTree::Name(item) => {
            let mut path = prefix.to_vec();
            path.push(item.ident.to_string());
            paths.push(path);
            true
        }
        UseTree::Group(items) => items
            .items
            .iter()
            .all(|item| flatten_use_tree(prefix, item, paths)),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}

fn type_is(ty: &syn::Type, name: &str) -> bool {
    matches!(ty, syn::Type::Path(path) if path.qself.is_none() && path_is_type_path(&path.path, name))
}

fn path_is_type_path(path: &syn::Path, name: &str) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == 1
        && path.segments[0].ident == name
        && matches!(path.segments[0].arguments, syn::PathArguments::None)
}
