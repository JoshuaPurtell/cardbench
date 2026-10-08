//! Full-game RAV Loxodon Hierarch PvP authority.

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbilityBinding, CardType, CastRequest, Color, DeckEntry, DeckList,
    Effect, Game, GameEvent, GameView, ManaCost, ManaPaymentSelection, ObjectId, PlayerId,
    PolicyAction, Step, TriggeredAbilityBinding, Zone,
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
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::parse::Parser;
use syn::{Expr, Item, Meta, ReturnType, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-LOXODON-HIERARCH";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const CHAR_ID: &str = "RAV-CHAR";
const TRIGGER_ID: &str = "etb-gain-four-life";
const ABILITY_ID: &str = "sacrifice-source-regenerate-controller-creatures";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/loxodon_hierarch.rs";
const DECK_P0: &str = "rav_loxodon_hierarch_watchwolf_team";
const DECK_P1: &str = "rav_char_watchwolf_opponent";

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/loxodon_hierarch.rs"]
mod candidate_loxodon_hierarch;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Loxodon Hierarch PvP result: {error}");
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
            eprintln!("Loxodon Hierarch PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    // The untrusted module is parsed and shape-checked before any candidate function call.
    validate_candidate_source()?;
    let side_effect_guard = validate_side_effect_source_mutation()?;
    validate_candidate_definition()?;
    validate_registry_delegation()?;
    let catalog = unique_catalog()?;
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if !same_match(&captured, &replay) {
        return Err("Loxodon Hierarch full-game replay was not deterministic".to_owned());
    }
    let mutations = mutation_sensitivity(seed, &side_effect_guard)?;
    for key in [
        "entry_trigger_removal_detected_in_game",
        "wrong_life_gain_mutation_detected_in_game",
        "wrong_activation_cost_mutation_detected_in_game",
        "wrong_regeneration_scope_mutation_detected_in_game",
        "activation_binding_removal_detected_in_game",
        "parse_valid_side_effect_source_rejected_before_candidate_calls",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Loxodon Hierarch mutation escaped authority: {key}"
            ));
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
        "card":{"collector_number":214,"definition_id":CARD_ID,"name":"Loxodon Hierarch"},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":[
                "105 and 202: player and card characteristics",
                "106.1 and 106.4: mana production and payment",
                "117: priority and timing",
                "601.2 and 608.3a: casting and resolving a creature spell",
                "603.6: enters-the-battlefield triggers",
                "608.2: resolving triggered and activated abilities",
                "701.19 and 614.8: regeneration replacement shields",
                "704.5g: lethal damage state-based action"
            ],
            "source_contract":"RAV #214; {2}{G}{W} 4/4 white-green Creature — Elephant Cleric; mandatory entry gain of 4 life; {G}{W}, sacrifice this source to regenerate each creature you control",
            "oracle_reconciliation":"The RAV 'comes into play' and named-source phrasing is represented by equivalent current 'enters' and 'this creature' wording. Printed cost/effects are the target.",
            "subtype_model_limit":"Elephant and Cleric are retained in task source metadata; CardDefinition currently cannot assert creature subtypes.",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true
        },
        "source_guard":side_effect_guard,
        "passed":true,
        "pilot_card_passed":true,
        "replay_deterministic":true,
        "cards_passed":cards_passed,
        "cards_credited":0,
        "cards_total":RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
        "printings_total":RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":"unreviewed_diagnostic; independent reviewer approval and candidate trial remain separate",
        "mutation_sensitivity":mutations,
        "traces":[trace],
        "per_card_breakdown":breakdown
    }))
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "candidate"
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
        return Err("RAV catalog identity or printing denominator changed".to_owned());
    }
    Ok(catalog)
}

fn card_status(card: &CatalogCard) -> Value {
    let (engine_status, definition_id) = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => {
            ("executable_compatibility_slice", Some(definition_id))
        }
        CardSemanticStatus::CatalogOnly { .. } => ("catalog_only", None),
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"definition_id":definition_id,"name":card.name,
        "engine_status":engine_status,"qualification":if passed{"local_trace_passed; independent reviewer approval pending"}else{"not_evaluated"},
        "trace_status":if passed{"passed"}else{"not_qualified"},"credited":false})
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    OmitTrigger,
    WrongLifeGain,
    WrongActivationCost,
    WrongRegenerationScope,
    RemoveActivation,
}

#[derive(Clone)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    terminal: String,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    hierarch: Option<ObjectId>,
    watchwolves: Vec<ObjectId>,
    opponent_watchwolf: Option<ObjectId>,
    char_spell: Option<ObjectId>,
    hierarch_cast_payment: Vec<Color>,
    entry_life_events: usize,
    life_after_entry: Option<i64>,
    trigger_stacks: usize,
    trigger_resolutions: usize,
    activation_mana_sources: Vec<Color>,
    activation_payment_cost: Option<ManaCost>,
    activation_count: usize,
    activation_resolutions: usize,
    hierarch_sacrificed: bool,
    creatures_controlled_at_resolution: Vec<ObjectId>,
    opponents_controlled_at_resolution: Vec<ObjectId>,
    shield_created_targets: Vec<ObjectId>,
    shield_used_targets: Vec<ObjectId>,
    char_damage_target: Option<ObjectId>,
    target_state_after_char: Option<(Zone, bool, i32)>,
    life_totals: [i64; 2],
}

fn find_trace() -> Result<(u64, Captured), String> {
    let mut last = "no seed checked".to_owned();
    for seed in 0..512 {
        let captured = match run_match(seed, Mutation::None) {
            Ok(captured) => captured,
            Err(error) => {
                last = error;
                continue;
            }
        };
        match validate_trace(&captured, seed) {
            Ok(_) => return Ok((seed, captured)),
            Err(error) => last = error,
        }
    }
    Err(format!(
        "no bounded seed produced the required Loxodon Hierarch full-game trace: {last}"
    ))
}

fn mutation_sensitivity(seed: u64, source_guard: &Value) -> Result<Value, String> {
    let cases = [
        (
            "entry_trigger_removal_detected_in_game",
            Mutation::OmitTrigger,
        ),
        (
            "wrong_life_gain_mutation_detected_in_game",
            Mutation::WrongLifeGain,
        ),
        (
            "wrong_activation_cost_mutation_detected_in_game",
            Mutation::WrongActivationCost,
        ),
        (
            "wrong_regeneration_scope_mutation_detected_in_game",
            Mutation::WrongRegenerationScope,
        ),
        (
            "activation_binding_removal_detected_in_game",
            Mutation::RemoveActivation,
        ),
    ];
    let mut outcomes = serde_json::Map::new();
    for (key, mutation) in cases {
        let detected = match run_match(seed, mutation) {
            Ok(captured) => validate_trace(&captured, seed).is_err(),
            Err(_) => true,
        };
        outcomes.insert(key.to_owned(), Value::Bool(detected));
    }
    outcomes.insert(
        "parse_valid_side_effect_source_rejected_before_candidate_calls".to_owned(),
        Value::Bool(
            source_guard["side_effect_mutation_rejected"].as_bool() == Some(true)
                && source_guard["candidate_calls_before_guard"].as_u64() == Some(0),
        ),
    );
    outcomes.insert("sealed_mutation_authority".to_owned(), Value::Bool(false));
    Ok(Value::Object(outcomes))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: WATCHWOLF_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 26,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 26,
            },
        ],
        sideboard: vec![],
    };
    let opponent = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CHAR_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: WATCHWOLF_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 26,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 13,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 13,
            },
        ],
        sideboard: vec![],
    };
    for (name, deck) in [(DECK_P0, &pilot), (DECK_P1, &opponent)] {
        let size: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if size != 60 {
            return Err(format!("{name} deck has {size} cards; expected 60"));
        }
    }
    let mut definitions = card_definitions();
    let definition = definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Loxodon Hierarch definition is absent from RAV registry")?;
    *definition = candidate_loxodon_hierarch::definition();

    let mut triggers = rav_triggered_ability_bindings();
    triggers.retain(|binding| binding.card_definition != CARD_ID);
    if mutation != Mutation::OmitTrigger {
        let mut trigger = candidate_loxodon_hierarch::triggered_ability();
        if mutation == Mutation::WrongLifeGain {
            trigger.ability.effects = vec![Effect::GainLifeController { amount: 3 }];
        }
        triggers.push(trigger);
    }
    let mut abilities = rav_activated_ability_bindings();
    abilities.retain(|binding| binding.card_definition != CARD_ID);
    if mutation != Mutation::RemoveActivation {
        let mut ability = candidate_loxodon_hierarch::activated_ability();
        match mutation {
            Mutation::WrongActivationCost => {
                ability.ability.mana_cost = ManaCost::with_colors(0, [Color::Green])
            }
            Mutation::WrongRegenerationScope => {
                ability.ability.effects = vec![Effect::RegenerateSource]
            }
            _ => {}
        }
        abilities.push(ability);
    }
    let mut game = Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
        triggers,
    )
    .map_err(|e| e.to_string())?;
    game.set_shuffle_seed(seed).map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot)
        .map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent)
        .map_err(|e| e.to_string())?;
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 48,
        ..DeckMatchConfig::default()
    };
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|e| e.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|e| e.to_string())?;
    game.begin_game().map_err(|e| e.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(HierarchPolicy::new(PlayerId(0))),
        Box::new(HierarchPolicy::new(PlayerId(1))),
    ];
    let (mut accepted, mut attempted) = (0_u32, 0_u32);
    let mut creatures_at_resolution = Vec::new();
    let mut opponents_at_resolution = Vec::new();
    let mut life_after_entry = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns.min(48) || accepted >= config.max_policy_moves {
            return Err("Loxodon Hierarch match exceeded bounded turn/move limit".to_owned());
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("missing seat policy")?;
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
        attempted += 1;
        let cursor = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action).map_err(|e| format!(
            "policy move rejected for seat {} on turn {} at {:?} (active {}, decision {}, priority {}): {e}",
            player.0, view.turn, view.step, view.active_player.0, view.decision_player.0, view.priority.0
        ))?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|e| format!("post-move invariant failure: {e}"))?;
        let committed = &game.event_log[cursor..];
        if committed.iter().any(|event| {
            matches!(event,
            GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. }
                if game.card_definition(*source).is_ok_and(|d| d.id == CARD_ID))
        }) {
            let resolved = game
                .view_for_player(PlayerId(0))
                .map_err(|e| e.to_string())?;
            creatures_at_resolution = resolved
                .own_battlefield
                .iter()
                .filter(|card| card.card_types.contains(&CardType::Creature))
                .map(|card| card.id)
                .collect();
            opponents_at_resolution = resolved
                .opponent_battlefield
                .iter()
                .filter(|card| card.card_types.contains(&CardType::Creature))
                .map(|card| card.id)
                .collect();
        }
        if committed.iter().any(|event| {
            matches!(
                event,
                GameEvent::LifeGained {
                    player: PlayerId(0),
                    amount: 4
                }
            )
        }) {
            life_after_entry = Some(game.player(PlayerId(0)).map_err(|e| e.to_string())?.life);
        }
    }
    let events = game.event_log.clone();
    let hierarch = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game.card_definition(*card).is_ok_and(|d| d.id == CARD_ID) => Some(*card),
        _ => None,
    });
    let watchwolves: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|d| d.id == WATCHWOLF_ID) =>
            {
                Some(*card)
            }
            _ => None,
        })
        .collect();
    let opponent_watchwolf = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(1),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|d| d.id == WATCHWOLF_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let char_spell = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(1),
            card,
        } if game.card_definition(*card).is_ok_and(|d| d.id == CHAR_ID) => Some(*card),
        _ => None,
    });
    let hierarch_cast_payment = hierarch
        .and_then(|source| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if *card == source => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    let entry_life_events = hierarch.map_or(0, |source| events.iter().filter(|event| matches!(event,
        GameEvent::LifeGained { player: PlayerId(0), amount: 4 }
    ) && events[..].iter().any(|earlier| matches!(earlier, GameEvent::TriggeredAbilityStacked { source: trigger_source, ability: TRIGGER_ID, .. } if *trigger_source == source))).count());
    let trigger_stacks = hierarch.map_or(0, |source| events.iter().filter(|event| matches!(event,
        GameEvent::TriggeredAbilityStacked { source: trigger_source, ability: TRIGGER_ID, .. } if *trigger_source == source
    )).count());
    let trigger_resolutions = hierarch.map_or(0, |source| events.iter().filter(|event| matches!(event,
        GameEvent::AbilityResolved { source: resolved_source, ability: TRIGGER_ID, .. } if *resolved_source == source
    )).count());
    let char_cast_index = char_spell.and_then(|card| {
        event_index(&events, |event| {
            matches!(event,
                GameEvent::SpellCast { player: PlayerId(1), card: cast } if *cast == card
            )
        })
    });
    let ability_payment_index = hierarch.and_then(|source| event_index(&events, |event| matches!(event,
        GameEvent::AbilityManaPaid { player: PlayerId(0), source: paid_source, ability: ABILITY_ID, .. } if *paid_source == source
    )));
    let activation_mana_sources = match (char_cast_index, ability_payment_index) {
        (Some(start), Some(end)) if start < end => events[start + 1..end]
            .iter()
            .filter_map(|event| match event {
                GameEvent::ManaAbilityActivated {
                    player: PlayerId(0),
                    color,
                    ..
                } => Some(*color),
                _ => None,
            })
            .collect(),
        _ => vec![],
    };
    let activation_payment_cost = hierarch.and_then(|source| {
        events.iter().find_map(|event| match event {
            GameEvent::AbilityManaPaid {
                player: PlayerId(0),
                source: paid_source,
                ability: ABILITY_ID,
                mana_cost,
            } if *paid_source == source => Some(mana_cost.clone()),
            _ => None,
        })
    });
    let activation_count = hierarch.map_or(0, |source| events.iter().filter(|event| matches!(event,
        GameEvent::AbilityActivated { player: PlayerId(0), source: paid_source, ability: ABILITY_ID, .. } if *paid_source == source
    )).count());
    let activation_resolutions = hierarch.map_or(0, |source| events.iter().filter(|event| matches!(event,
        GameEvent::AbilityResolved { source: resolved_source, ability: ABILITY_ID, .. } if *resolved_source == source
    )).count());
    let hierarch_sacrificed = hierarch.is_some_and(|source| {
        events.iter().any(|event| {
            matches!(event,
                GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == source
            )
        })
    });
    let shield_created_targets = hierarch.map_or_else(Vec::new, |source| {
        events
            .iter()
            .filter_map(|event| match event {
                GameEvent::RegenerationShieldCreated {
                    source: shield_source,
                    target,
                } if *shield_source == source => Some(*target),
                _ => None,
            })
            .collect()
    });
    let shield_used_targets = hierarch.map_or_else(Vec::new, |source| {
        events
            .iter()
            .filter_map(|event| match event {
                GameEvent::RegenerationShieldUsed {
                    source: shield_source,
                    target,
                } if *shield_source == source => Some(*target),
                _ => None,
            })
            .collect()
    });
    let char_damage_target = char_spell.and_then(|source| {
        events.iter().find_map(|event| match event {
            GameEvent::DamageDealtToPermanent {
                source: damage_source,
                permanent,
                amount: 4,
            } if *damage_source == source => Some(*permanent),
            _ => None,
        })
    });
    let target_state_after_char = char_damage_target.and_then(|target| {
        Some((
            game.zone_of(target)?,
            game.object(target).ok()?.tapped,
            game.object(target).ok()?.damage,
        ))
    });
    Ok(Captured {
        events,
        winner: game.winner().map(|p| p.0),
        terminal: game.winner().map_or_else(
            || "Draw".to_owned(),
            |p| format!("Winner(PlayerId({}))", p.0),
        ),
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempted.saturating_sub(accepted),
        hierarch,
        watchwolves,
        opponent_watchwolf,
        char_spell,
        hierarch_cast_payment,
        entry_life_events,
        life_after_entry,
        trigger_stacks,
        trigger_resolutions,
        activation_mana_sources,
        activation_payment_cost,
        activation_count,
        activation_resolutions,
        hierarch_sacrificed,
        creatures_controlled_at_resolution: creatures_at_resolution,
        opponents_controlled_at_resolution: opponents_at_resolution,
        shield_created_targets,
        shield_used_targets,
        char_damage_target,
        target_state_after_char,
        life_totals: [
            game.player(PlayerId(0)).map_err(|e| e.to_string())?.life,
            game.player(PlayerId(1)).map_err(|e| e.to_string())?.life,
        ],
    })
}

struct HierarchPolicy {
    player: PlayerId,
    hierarch_cast: bool,
    watchwolves_cast: u8,
    char_cast: bool,
    regeneration_activated: bool,
}
impl HierarchPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            hierarch_cast: false,
            watchwolves_cast: 0,
            char_cast: false,
            regeneration_activated: false,
        }
    }
}
impl CodePolicy for HierarchPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.loxodon-hierarch-team-shields.v1"
        } else {
            "rav.char-watchwolf-opponent.v1"
        }
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.can_attack)
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
        if self.player == PlayerId(0) && !self.regeneration_activated && has_char_on_stack(view) {
            if let Some(source) = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            {
                if view.mana_pool.amount(Color::Green) == 0 {
                    return activate_color(view, Color::Green);
                }
                if view.mana_pool.amount(Color::White) == 0 {
                    return activate_color(view, Color::White);
                }
                self.regeneration_activated = true;
                return PolicyAction::ActivateAbility {
                    activation: AbilityActivation {
                        source: source.id,
                        ability_id: ABILITY_ID,
                        sacrifice_sources: vec![source.id],
                        additional_tap_creatures: vec![],
                        discard_cards: vec![],
                        targets: vec![],
                    },
                };
            }
        }
        if view.decision_player != self.player || view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if view.active_player != self.player
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
        let lands = view
            .own_battlefield
            .iter()
            .filter(|card| card.card_types.contains(&CardType::Land))
            .count();
        if self.player == PlayerId(0) {
            if !self.hierarch_cast {
                let wolves = view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(WATCHWOLF_ID))
                    .count();
                if wolves < 2 {
                    if let Some(card) = view
                        .hand
                        .iter()
                        .find(|card| card.definition == Some(WATCHWOLF_ID))
                    {
                        let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
                        if let Some(selection) = payment(view, &cost) {
                            self.watchwolves_cast = self.watchwolves_cast.saturating_add(1);
                            return cast(card.id, selection, vec![]);
                        }
                        return make_mana(view, &cost);
                    }
                }
                if wolves >= 2 && lands >= 6 {
                    if let Some(card) = view
                        .hand
                        .iter()
                        .find(|card| card.definition == Some(CARD_ID))
                    {
                        if let Some(selection) = payment_preserving_green_white(view) {
                            self.hierarch_cast = true;
                            return cast(card.id, selection, vec![]);
                        }
                        return make_hierarch_cast_mana(view);
                    }
                }
            }
        } else {
            let own_wolf = view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(WATCHWOLF_ID));
            let enemy_has_hierarch = view
                .opponent_battlefield
                .iter()
                .any(|card| card.definition == Some(CARD_ID));
            if !own_wolf {
                if let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(WATCHWOLF_ID))
                {
                    let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
                    if let Some(selection) = payment(view, &cost) {
                        return cast(card.id, selection, vec![]);
                    }
                    return make_mana(view, &cost);
                }
            }
            if !self.char_cast && enemy_has_hierarch && own_wolf {
                if let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(CHAR_ID))
                {
                    let target = view
                        .opponent_battlefield
                        .iter()
                        .find(|creature| creature.definition == Some(WATCHWOLF_ID));
                    if let Some(target) = target {
                        let cost = ManaCost::with_colors(2, [Color::Red, Color::Red]);
                        if let Some(selection) = payment(view, &cost) {
                            self.char_cast = true;
                            return cast(
                                card.id,
                                selection,
                                vec![cardbench_magic_engine::Target::Permanent(target.id)],
                            );
                        }
                        return make_mana(view, &cost);
                    }
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn has_char_on_stack(view: &GameView) -> bool {
    view.stack_spells
        .iter()
        .any(|spell| spell.definition == Some(CHAR_ID))
}
fn cast(
    card: ObjectId,
    mana_selection: ManaPaymentSelection,
    targets: Vec<cardbench_magic_engine::Target>,
) -> PolicyAction {
    PolicyAction::CastWithPayment {
        request: CastRequest {
            card,
            targets,
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        chosen_x: None,
        mana_selection,
    }
}
fn activate_color(view: &GameView, color: Color) -> PolicyAction {
    let land = view.own_battlefield.iter().find(|card| {
        card.card_types.contains(&CardType::Land)
            && !card.tapped
            && card.mana_colors.contains(&color)
    });
    if let Some(land) = land {
        PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        }
    } else {
        PolicyAction::PassPriority
    }
}
fn make_hierarch_cast_mana(view: &GameView) -> PolicyAction {
    let green = view.mana_pool.amount(Color::Green);
    let desired = if green < 2 {
        Color::Green
    } else {
        Color::White
    };
    let same_color_untapped = view
        .own_battlefield
        .iter()
        .filter(|card| {
            card.card_types.contains(&CardType::Land)
                && !card.tapped
                && card.mana_colors.contains(&desired)
        })
        .count();
    let reserved_color = view.own_battlefield.iter().any(|card| {
        card.card_types.contains(&CardType::Land)
            && !card.tapped
            && card.mana_colors.contains(&if desired == Color::Green {
                Color::White
            } else {
                Color::Green
            })
    });
    if same_color_untapped > 1 && reserved_color {
        if let Some(land) = view.own_battlefield.iter().find(|card| {
            card.card_types.contains(&CardType::Land)
                && !card.tapped
                && card.mana_colors.contains(&desired)
        }) {
            return PolicyAction::ActivateManaAbility {
                land: land.id,
                color: desired,
            };
        }
    }
    PolicyAction::PassPriority
}
fn payment_preserving_green_white(view: &GameView) -> Option<ManaPaymentSelection> {
    if view.mana_pool.amount(Color::Green) < 2
        || view.mana_pool.amount(Color::White) < 2
        || view.mana_pool.total() < 4
    {
        return None;
    }
    let mut available = [
        (Color::Green, view.mana_pool.amount(Color::Green)),
        (Color::White, view.mana_pool.amount(Color::White)),
    ];
    available[0].1 -= 1;
    available[1].1 -= 1;
    let mut generic = Vec::new();
    for _ in 0..2 {
        let index = if available[0].1 > available[1].1 {
            0
        } else if available[1].1 > available[0].1 {
            1
        } else {
            generic.len() % 2
        };
        if available[index].1 == 0 {
            return None;
        }
        available[index].1 -= 1;
        generic.push(available[index].0);
    }
    Some(ManaPaymentSelection {
        generic,
        hybrid: vec![],
    })
}
fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<_> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let (_, amount) = available
            .iter_mut()
            .find(|(available_color, _)| available_color == color)?;
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
fn make_mana(view: &GameView, cost: &ManaCost) -> PolicyAction {
    let need =
        cost.colored
            .iter()
            .copied()
            .find(|color| view.mana_pool.amount(*color) == 0)
            .or_else(|| {
                (view.mana_pool.total() < cost.generic.saturating_add(cost.colored.len() as u8))
                    .then(|| {
                        view.own_battlefield
                            .iter()
                            .find(|card| {
                                card.card_types.contains(&CardType::Land)
                                    && !card.tapped
                                    && !card.mana_colors.is_empty()
                            })
                            .and_then(|card| {
                                Color::MANA_ALL
                                    .into_iter()
                                    .find(|color| card.mana_colors.contains(color))
                            })
                            .unwrap_or(Color::Green)
                    })
            });
    if let Some(color) = need {
        return activate_color(view, color);
    }
    PolicyAction::PassPriority
}

fn same_match(left: &Captured, right: &Captured) -> bool {
    left.events == right.events
        && left.winner == right.winner
        && left.terminal == right.terminal
        && left.hierarch == right.hierarch
        && left.watchwolves == right.watchwolves
        && left.opponent_watchwolf == right.opponent_watchwolf
        && left.char_spell == right.char_spell
        && left.hierarch_cast_payment == right.hierarch_cast_payment
        && left.entry_life_events == right.entry_life_events
        && left.life_after_entry == right.life_after_entry
        && left.trigger_stacks == right.trigger_stacks
        && left.trigger_resolutions == right.trigger_resolutions
        && left.activation_mana_sources == right.activation_mana_sources
        && left.activation_payment_cost == right.activation_payment_cost
        && left.activation_count == right.activation_count
        && left.activation_resolutions == right.activation_resolutions
        && left.hierarch_sacrificed == right.hierarch_sacrificed
        && left.creatures_controlled_at_resolution == right.creatures_controlled_at_resolution
        && left.opponents_controlled_at_resolution == right.opponents_controlled_at_resolution
        && left.shield_created_targets == right.shield_created_targets
        && left.shield_used_targets == right.shield_used_targets
        && left.char_damage_target == right.char_damage_target
        && left.target_state_after_char == right.target_state_after_char
}

fn validate_trace(c: &Captured, seed: u64) -> Result<Value, String> {
    let hierarch = c.hierarch.ok_or("Loxodon Hierarch was not cast")?;
    let char_spell = c.char_spell.ok_or("opponent Char was not cast")?;
    let opponent_wolf = c
        .opponent_watchwolf
        .ok_or("opponent Watchwolf was not cast")?;
    let target = c
        .char_damage_target
        .ok_or("Char damage target was not captured")?;
    if c.terminal != "Winner(PlayerId(0))" || c.winner != Some(0) {
        return Err("full-game terminal result is not a Player 0 win".to_owned());
    }
    if c.rejected_moves != 0 || c.watchwolves.len() < 2 {
        return Err(
            "legal PvP trace requires two controller Watchwolves and no rejected moves".to_owned(),
        );
    }
    if c.hierarch_cast_payment.len() != 4
        || c.hierarch_cast_payment
            .iter()
            .filter(|c| **c == Color::Green)
            .count()
            != 2
        || c.hierarch_cast_payment
            .iter()
            .filter(|c| **c == Color::White)
            .count()
            != 2
    {
        return Err("Loxodon Hierarch must pay exactly {2}{G}{W}".to_owned());
    }
    if c.entry_life_events != 1
        || c.life_after_entry != Some(24)
        || c.trigger_stacks != 1
        || c.trigger_resolutions != 1
    {
        return Err(
            "Loxodon Hierarch must resolve its mandatory one-time four-life entry trigger"
                .to_owned(),
        );
    }
    if c.activation_payment_cost.as_ref()
        != Some(&ManaCost::with_colors(0, [Color::Green, Color::White]))
        || c.activation_mana_sources
            .iter()
            .filter(|c| **c == Color::Green)
            .count()
            != 1
        || c.activation_mana_sources
            .iter()
            .filter(|c| **c == Color::White)
            .count()
            != 1
        || c.activation_count != 1
        || c.activation_resolutions != 1
        || !c.hierarch_sacrificed
    {
        return Err(
            "Loxodon Hierarch must pay {G}{W}, sacrifice itself, and resolve once".to_owned(),
        );
    }
    let expected = c.creatures_controlled_at_resolution.clone();
    let mut expected_sorted = expected.clone();
    expected_sorted.sort();
    let mut created_sorted = c.shield_created_targets.clone();
    created_sorted.sort();
    if expected.is_empty()
        || expected_sorted != created_sorted
        || expected.contains(&opponent_wolf)
        || c.opponents_controlled_at_resolution.is_empty()
        || c.shield_created_targets.contains(&opponent_wolf)
    {
        return Err(
            "regeneration shields must match exactly the controller's creatures at resolution"
                .to_owned(),
        );
    }
    if target != c.watchwolves[0] && !c.watchwolves.contains(&target) {
        return Err("Char must target one controlled Watchwolf".to_owned());
    }
    if !c.shield_used_targets.contains(&target)
        || c.target_state_after_char != Some((Zone::Battlefield, true, 0))
    {
        return Err("lethal Char damage must consume the target's shield and leave it tapped with damage cleared".to_owned());
    }
    let projected = project_events(&c.events);
    let events: Vec<Value> = projected.iter().map(|event| json!({"sequence":event.sequence,"turn":event.turn,"kind":event.kind,"facts":event.facts,"canonical":event.canonical})).collect();
    let required = [
        "SpellManaPaid",
        "SpellCast",
        "TriggeredAbilityStacked",
        "LifeGained",
        "AbilityResolved",
        "SpellCast",
        "ManaAbilityActivated",
        "AbilityManaPaid",
        "CardMoved",
        "AbilityActivated",
        "RegenerationShieldCreated",
        "AbilityResolved",
        "DamageDealtToPermanent",
        "RegenerationShieldUsed",
        "GameEnded",
    ];
    let ordered = ordered_event_indices(&projected, &required).ok_or(
        "required cast/trigger/payment/shield/damage/terminal events are absent or out of order",
    )?;
    let game_end = c
        .events
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::GameEnded {
                    winner: Some(PlayerId(0))
                }
            )
        })
        .ok_or("GameEnded(PlayerId(0)) event missing")?;
    let char_damage = event_index(&c.events, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 4 } if *source == char_spell && *permanent == target)).ok_or("Char did not deal 4 damage to shielded Watchwolf")?;
    let shield_used = event_index(&c.events, |event| matches!(event, GameEvent::RegenerationShieldUsed { source, target: shield_target } if *source == hierarch && *shield_target == target)).ok_or("Hierarch regeneration shield was not used")?;
    if char_damage >= shield_used || shield_used >= game_end {
        return Err(
            "Char damage, regeneration shield use, and terminal receipts are not ordered"
                .to_owned(),
        );
    }
    Ok(json!({
        "id":"rav-loxodon-hierarch-sacrifice-team-regeneration-full-game",
        "passed":true,"seed":seed,"terminal":c.terminal,"winner_seat":c.winner,"turns":c.turns,
        "accepted_moves":c.accepted_moves,"rejected_moves":c.rejected_moves,
        "loxodon_hierarch":{"object":hierarch,"definition_id":CARD_ID,"mana_cost":"{2}{G}{W}","cast_mana_paid":c.hierarch_cast_payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"power":4,"toughness":4,"life_gained":4,"life_after_entry":c.life_after_entry,"trigger_stacks":c.trigger_stacks,"trigger_resolutions":c.trigger_resolutions},
        "activation":{"ability":ABILITY_ID,"mana_cost":"{G}{W}","mana_abilities_after_char":c.activation_mana_sources.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"cost_paid_colors":c.activation_payment_cost.as_ref().map(|cost|cost.colored.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>()),"activated":c.activation_count,"resolved":c.activation_resolutions,"source_sacrificed":c.hierarch_sacrificed,"creatures_at_resolution":c.creatures_controlled_at_resolution,"shields_created":c.shield_created_targets,"opponent_creatures_at_resolution":c.opponents_controlled_at_resolution,"opponent_creature_shielded":false},
        "opponent":{"watchwolf":opponent_wolf,"char":char_spell,"char_mana_cost":"{2}{R}{R}"},
        "target":{"object":target,"definition_id":WATCHWOLF_ID,"char_damage":4,"shield_used":c.shield_used_targets.contains(&target),"post_damage_zone":format!("{:?}",c.target_state_after_char.map(|state|state.0)),"post_damage_tapped":c.target_state_after_char.is_some_and(|state|state.1),"post_damage_marked_damage":c.target_state_after_char.map(|state|state.2)},
        "life_totals":c.life_totals,"events":events,"ordered_event_sequence":ordered
    }))
}

fn ordered_event_indices(
    events: &[cardbench_magic_session::ProjectedEvent],
    required: &[&str],
) -> Option<Vec<u64>> {
    let mut cursor = 0_usize;
    let mut indices = Vec::new();
    for expected in required {
        let index = events
            .iter()
            .enumerate()
            .skip(cursor)
            .find_map(|(index, event)| (event.kind == *expected).then_some(index))?;
        indices.push(events[index].sequence);
        cursor = index + 1;
    }
    Some(indices)
}
fn event_index(
    events: &[GameEvent],
    mut predicate: impl FnMut(&GameEvent) -> bool,
) -> Option<usize> {
    events.iter().position(|event| predicate(event))
}

fn validate_candidate_definition() -> Result<(), String> {
    let card = candidate_loxodon_hierarch::definition();
    if card.id != CARD_ID
        || card.name != "Loxodon Hierarch"
        || card.set_code != "RAV"
        || card.mana_cost != ManaCost::with_colors(2, [Color::Green, Color::White])
        || card.colors != BTreeSet::from([Color::Green, Color::White])
        || card.card_types != BTreeSet::from([CardType::Creature])
        || card.power != Some(4)
        || card.toughness != Some(4)
        || card.is_basic_land
        || !card.mana_colors.is_empty()
        || !card.keywords.is_empty()
    {
        return Err(
            "Loxodon Hierarch printed identity, {2}{G}{W} cost, or 4/4 characteristics differ"
                .to_owned(),
        );
    }
    let trigger = candidate_loxodon_hierarch::triggered_ability();
    if trigger.card_definition != CARD_ID
        || trigger.ability.id != TRIGGER_ID
        || trigger.ability.condition != cardbench_magic_engine::TriggerCondition::EntersBattlefield
        || trigger.ability.mana_cost != ManaCost::new(0)
        || trigger.ability.optional
        || !trigger.ability.targets.is_empty()
        || trigger.ability.effects != [Effect::GainLifeController { amount: 4 }]
    {
        return Err(
            "Loxodon Hierarch entry trigger must be mandatory and gain exactly four life"
                .to_owned(),
        );
    }
    let binding = candidate_loxodon_hierarch::activated_ability();
    let ability = binding.ability;
    if binding.card_definition != CARD_ID
        || ability.id != ABILITY_ID
        || ability.mana_cost != ManaCost::with_colors(0, [Color::Green, Color::White])
        || ability.tap_cost
        || ability.sorcery_speed
        || ability.additional_tap_creatures != 0
        || !ability.sacrifice_source
        || ability.sacrifice_creatures != 0
        || ability.sacrifice_lands != 0
        || ability.discard_cards != 0
        || !ability.targets.is_empty()
        || ability.effects != [Effect::RegenerateControllerCreatures]
    {
        return Err("Loxodon Hierarch must pay {G}{W}, sacrifice itself, and regenerate each creature its controller controls".to_owned());
    }
    Ok(())
}

fn validate_registry_delegation() -> Result<(), String> {
    let candidate = candidate_loxodon_hierarch::definition();
    if card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .as_ref()
        != Some(&candidate)
    {
        return Err(
            "Loxodon Hierarch registry definition does not delegate to its candidate module"
                .to_owned(),
        );
    }
    let expected_trigger = candidate_loxodon_hierarch::triggered_ability();
    if rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == TRIGGER_ID)
        .as_ref()
        != Some(&expected_trigger)
    {
        return Err(
            "Loxodon Hierarch registry trigger does not delegate to its candidate module"
                .to_owned(),
        );
    }
    let expected_ability = candidate_loxodon_hierarch::activated_ability();
    if rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .as_ref()
        != Some(&expected_ability)
    {
        return Err(
            "Loxodon Hierarch registry activation does not delegate to its candidate module"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|e| format!("could not read Loxodon Hierarch source: {e}"))?;
    validate_source_shape(&source)
}

fn validate_source_shape(source: &str) -> Result<(), String> {
    let file =
        syn::parse_file(source).map_err(|_| "Loxodon Hierarch source is not valid Rust syntax")?;
    if !file
        .attrs
        .iter()
        .all(|attr| attr.path().is_ident("doc") && matches!(attr.meta, Meta::NameValue(_)))
    {
        return Err("Loxodon Hierarch module has an unsupported module attribute".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "ActivatedAbility"],
        vec!["cardbench_magic_engine", "ActivatedAbilityBinding"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
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
                if matches!(item.vis, Visibility::Inherited) && item.attrs.is_empty() =>
            {
                imports.extend(
                    flattened_use_paths(item)
                        .ok_or("Loxodon Hierarch imports are outside the source contract")?,
                );
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                let expected_return = match name.as_str() {
                    "definition" => "CardDefinition",
                    "triggered_ability" => "TriggeredAbilityBinding",
                    "activated_ability" => "ActivatedAbilityBinding",
                    _ => {
                        return Err(
                            "Loxodon Hierarch may expose only three candidate functions".to_owned()
                        );
                    }
                };
                if !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || !function.sig.inputs.is_empty()
                    || !function.sig.generics.params.is_empty()
                    || function.sig.variadic.is_some()
                    || !matches!(&function.sig.output, ReturnType::Type(_, output) if matches!(&**output, syn::Type::Path(path) if path.path.is_ident(expected_return)))
                    || function.block.stmts.len() != 1
                    || !function.attrs.iter().all(|attr| {
                        (attr.path().is_ident("doc") && matches!(attr.meta, Meta::NameValue(_)))
                            || attr.path().is_ident("must_use")
                    })
                {
                    return Err("Loxodon Hierarch candidate functions must have safe no-argument signatures and one literal return".to_owned());
                }
                let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
                    return Err(
                        "Loxodon Hierarch candidate functions must return direct struct literals"
                            .to_owned(),
                    );
                };
                let expected_type = expected_return;
                if value
                    .path
                    .segments
                    .last()
                    .is_none_or(|segment| segment.ident != expected_type)
                    || value.rest.is_some()
                    || !declarative_expr(&Expr::Struct(value.clone()))
                {
                    return Err(
                        "Loxodon Hierarch candidate source contains an unsupported expression"
                            .to_owned(),
                    );
                }
                let expected_fields: BTreeSet<&str> = match name.as_str() {
                    "definition" => [
                        "id",
                        "name",
                        "set_code",
                        "mana_cost",
                        "colors",
                        "mana_colors",
                        "card_types",
                        "is_basic_land",
                        "supported_rules",
                        "power",
                        "toughness",
                        "keywords",
                        "effects",
                    ]
                    .into_iter()
                    .collect(),
                    "triggered_ability" => ["card_definition", "ability"].into_iter().collect(),
                    "activated_ability" => ["card_definition", "ability"].into_iter().collect(),
                    _ => unreachable!(),
                };
                let fields: BTreeSet<String> = value
                    .fields
                    .iter()
                    .filter_map(|field| match &field.member {
                        syn::Member::Named(name) => Some(name.to_string()),
                        _ => None,
                    })
                    .collect();
                if fields.len() != value.fields.len()
                    || fields.iter().map(String::as_str).collect::<BTreeSet<_>>() != expected_fields
                {
                    return Err(
                        "Loxodon Hierarch candidate struct literal field shape differs".to_owned(),
                    );
                }
                functions.insert(name);
            }
            _ => {
                return Err(
                    "Loxodon Hierarch source permits only imports and candidate functions"
                        .to_owned(),
                );
            }
        }
    }
    if imports != expected_imports
        || functions
            != BTreeSet::from([
                "definition".to_owned(),
                "triggered_ability".to_owned(),
                "activated_ability".to_owned(),
            ])
    {
        return Err(
            "Loxodon Hierarch normalized source shape differs from the pinned module schema"
                .to_owned(),
        );
    }
    Ok(())
}

fn declarative_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(_) | Expr::Path(_) => true,
        Expr::Unary(unary) => {
            matches!(unary.op, syn::UnOp::Neg(_)) && matches!(&*unary.expr, Expr::Lit(_))
        }
        Expr::Reference(reference) => declarative_expr(&reference.expr),
        Expr::Array(array) => array.elems.iter().all(declarative_expr),
        Expr::Struct(value) => {
            value.attrs.is_empty()
                && value.fields.iter().all(|field| {
                    matches!(field.member, syn::Member::Named(_)) && declarative_expr(&field.expr)
                })
                && value.rest.as_deref().is_none_or(declarative_expr)
        }
        Expr::Call(call) => {
            let allowed = match &*call.func {
                Expr::Path(path) if path.qself.is_none() => {
                    let names: Vec<_> = path
                        .path
                        .segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect();
                    matches!(names.as_slice(), [a, b] if (a == "BTreeSet" && (b == "from" || b == "new")) || (a == "ManaCost" && (b == "new" || b == "with_colors")))
                        || matches!(names.as_slice(), [name] if name == "Some")
                }
                _ => false,
            };
            allowed && call.args.iter().all(declarative_expr)
        }
        Expr::Macro(mac) => {
            let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
            mac.mac.path.is_ident("vec")
                && parser
                    .parse2(mac.mac.tokens.clone())
                    .is_ok_and(|items| items.iter().all(declarative_expr))
        }
        _ => false,
    }
}

fn validate_side_effect_source_mutation() -> Result<Value, String> {
    let source = std::fs::read_to_string(SOURCE_PATH).map_err(|e| e.to_string())?;
    let marker = "pub(crate) fn definition() -> CardDefinition {\n    CardDefinition {";
    let injected = source.replace(marker, "pub(crate) fn definition() -> CardDefinition {\n    let _ = std::fs::write(\"/tmp/cardbench-loxodon-source-guard-executed\", b\"candidate ran\");\n    CardDefinition {");
    if injected == source {
        return Err("could not construct compile-valid source-side-effect mutation".to_owned());
    }
    syn::parse_file(&injected).map_err(|_| "side-effect mutation must remain parse-valid Rust")?;
    let rejected = validate_source_shape(&injected).is_err();
    if !rejected {
        return Err("source guard accepted an executable candidate mutation".to_owned());
    }
    Ok(
        json!({"normalized_ast_schema_checked_before_candidate_calls":true,"side_effect_mutation_parse_valid":true,"side_effect_mutation_rejected":true,"candidate_calls_before_guard":0,"rejection_stage":"source-shape-and-declarative-ast-before-candidate-function-invocation"}),
    )
}

fn flattened_use_paths(item: &syn::ItemUse) -> Option<Vec<Vec<String>>> {
    if item.leading_colon.is_some() {
        return None;
    }
    let mut paths = Vec::new();
    flatten_use_tree(&[], &item.tree, &mut paths).then_some(paths)
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
        _ => false,
    }
}
