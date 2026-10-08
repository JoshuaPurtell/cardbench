//! Full-game RAV Dryad's Caress authority for battlefield-count life and conditional untapping.

use cardbench_magic_engine::{
    CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, ManaCost,
    ManaPaymentSelection, PlayerId, PolicyAction, Step,
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
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::parse::Parser;
use syn::{Expr, Item, Meta, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-DRYADS-CARESS";
const CREATURE_ID: &str = "RAV-WATCHWOLF";
const PRESSURE_ID: &str = "RAV-WAR-TORCH-GOBLIN";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/dryads_caress.rs";
const DECK_P0: &str = "rav_dryads_caress_watchwolf_wide";
const DECK_P1: &str = "rav_war_torch_goblin_opponent";

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/dryads_caress.rs"]
mod candidate_dryads_caress;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Dryad's Caress PvP result: {error}");
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
            eprintln!("Dryad's Caress PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_definition()?;
    validate_registry_delegation()?;
    let catalog = unique_catalog()?;
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if !same_match(&captured, &replay) {
        return Err("Dryad's Caress full-game replay was not deterministic".to_owned());
    }
    let mutations = mutation_sensitivity(seed)?;
    for key in [
        "life_gain_effect_removal_detected_in_game",
        "conditional_untap_removal_detected_in_game",
        "wrong_untap_color_mutation_detected_in_game",
        "wrong_cast_cost_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Dryad's Caress mutation escaped the PvP trace: {key}"
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
        "card":{"collector_number":160,"definition_id":CARD_ID,"name":"Dryad's Caress"},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":[
                "105.2 and 202.1-202.2: green spell color and mana payment",
                "106.1 and 106.4: mana production and colored mana spent toward generic costs",
                "117: priority and resolution timing",
                "119.1 and 119.4: life gain and life totals",
                "601.2: casting an instant spell",
                "608.2: resolving spell instructions in order",
                "701.21: untapping permanents",
                "508.1 and 510: attacking creatures and combat damage"
            ],
            "source_contract":"RAV Dryad's Caress #160, {4}{G}{G} green instant; gain one life per battlefield creature, and if White mana was spent to cast it, untap creatures its controller controls",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true
        },
        "passed":true,
        "pilot_card_passed":true,
        "replay_deterministic":true,
        "cards_passed":cards_passed,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":"candidate_pilot_pass; independent reviewer approval remains",
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
    json!({"collector_number":card.collector_number,"definition_id":definition_id,"name":card.name,"engine_status":engine_status,"qualification":if passed{"pilot_trace_passed; independent reviewer approval pending"}else{"not_evaluated"},"trace_status":if passed{"passed"}else{"not_qualified"},"credited":false})
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    RemoveLifeGain,
    RemoveUntap,
    WrongUntapColor,
    WrongCastCost,
}
#[derive(Clone)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    caress: Option<cardbench_magic_engine::ObjectId>,
    ours: Vec<cardbench_magic_engine::ObjectId>,
    opponent_creature: Option<cardbench_magic_engine::ObjectId>,
    cast_payment: Vec<Color>,
    creature_count: usize,
    life_gained: Option<i32>,
    untapped_ids: Vec<cardbench_magic_engine::ObjectId>,
    attacker_ids: Vec<cardbench_magic_engine::ObjectId>,
    own_attackers_untapped: usize,
    opponent_remained_tapped: bool,
    life_totals: [i64; 2],
}

fn find_trace() -> Result<(u64, Captured), String> {
    for seed in 0..256 {
        let captured = run_match(seed, Mutation::None)?;
        match validate_trace(&captured, seed) {
            Ok(_) => return Ok((seed, captured)),
            Err(error) if error.contains("was not cast") || error.contains("were not cast") => {
                continue;
            }
            Err(error) => return Err(error),
        }
    }
    Err("no bounded seed produced the Dryad's Caress battlefield-count and untap trace".to_owned())
}
fn mutation_sensitivity(seed: u64) -> Result<Value, String> {
    let mut outcomes = serde_json::Map::new();
    for (key, mutation) in [
        (
            "life_gain_effect_removal_detected_in_game",
            Mutation::RemoveLifeGain,
        ),
        (
            "conditional_untap_removal_detected_in_game",
            Mutation::RemoveUntap,
        ),
        (
            "wrong_untap_color_mutation_detected_in_game",
            Mutation::WrongUntapColor,
        ),
        (
            "wrong_cast_cost_mutation_detected_in_game",
            Mutation::WrongCastCost,
        ),
    ] {
        let detected = match run_match(seed, mutation) {
            Ok(captured) => validate_trace(&captured, seed).is_err(),
            Err(_) => true,
        };
        outcomes.insert(key.to_owned(), Value::Bool(detected));
    }
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
                card: CREATURE_ID.to_owned(),
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
                card: PRESSURE_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 26,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 30,
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
    let card = definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Dryad's Caress definition is absent from RAV registry")?;
    *card = candidate_dryads_caress::definition();
    match mutation {
        Mutation::RemoveLifeGain => card
            .effects
            .retain(|effect| !matches!(effect, Effect::GainLifeForEachCreature)),
        Mutation::RemoveUntap => card.effects.retain(|effect| {
            !matches!(
                effect,
                Effect::UntapControllerCreaturesIfManaColorSpent { .. }
            )
        }),
        Mutation::WrongUntapColor => {
            for effect in &mut card.effects {
                if let Effect::UntapControllerCreaturesIfManaColorSpent { color } = effect {
                    *color = Color::Red;
                }
            }
        }
        Mutation::WrongCastCost => {
            card.mana_cost = ManaCost::with_colors(3, [Color::Green, Color::Green])
        }
        Mutation::None => {}
    }
    let mut game = Game::new_with_all_bindings(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .map_err(|e| e.to_string())?;
    game.set_shuffle_seed(seed).map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot)
        .map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent)
        .map_err(|e| e.to_string())?;
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|e| e.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|e| e.to_string())?;
    game.begin_game().map_err(|e| e.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(CaressPolicy::new(PlayerId(0))),
        Box::new(CaressPolicy::new(PlayerId(1))),
    ];
    let (mut accepted, mut attempted) = (0u32, 0u32);
    let (mut creature_count, mut life_gained) = (0usize, None);
    let mut untapped_ids = Vec::new();
    let mut attacker_ids = Vec::new();
    let mut own_attackers_untapped = 0usize;
    let mut opponent_remained_tapped = false;
    while !game.is_game_over() {
        if game.turn > config.max_turns.min(60) || accepted >= config.max_policy_moves {
            return Err("Dryad's Caress match exceeded its bounded turn or move limit".to_owned());
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
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|e| {
                format!(
                    "policy move rejected for seat {} turn {} at {:?}: {e}",
                    player.0, view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|e| format!("post-move invariant failure: {e}"))?;
        if let Some(caress) = game.event_log[cursor..]
            .iter()
            .find_map(|event| match event {
                GameEvent::SpellResolved { card }
                    if game
                        .card_definition(*card)
                        .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    Some(*card)
                }
                _ => None,
            })
        {
            let resolved_view = game
                .view_for_player(PlayerId(0))
                .map_err(|e| e.to_string())?;
            creature_count = resolved_view
                .own_battlefield
                .iter()
                .chain(resolved_view.opponent_battlefield.iter())
                .filter(|card| card.card_types.contains(&CardType::Creature))
                .count();
            life_gained = game.event_log[cursor..]
                .iter()
                .find_map(|event| match event {
                    GameEvent::LifeGained {
                        player: PlayerId(0),
                        amount,
                    } => Some(*amount),
                    _ => None,
                });
            untapped_ids = game.event_log[cursor..]
                .iter()
                .filter_map(|event| match event {
                    GameEvent::PermanentUntapped { source, card } if *source == caress => {
                        Some(*card)
                    }
                    _ => None,
                })
                .collect();
            let ours: Vec<_> = game
                .event_log
                .iter()
                .filter_map(|event| match event {
                    GameEvent::SpellCast {
                        player: PlayerId(0),
                        card,
                    } if game
                        .card_definition(*card)
                        .is_ok_and(|definition| definition.id == CREATURE_ID) =>
                    {
                        Some(*card)
                    }
                    _ => None,
                })
                .collect();
            attacker_ids = game
                .event_log
                .iter()
                .find_map(|event| match event {
                    GameEvent::AttackersDeclared {
                        player: PlayerId(0),
                        attackers,
                    } if attackers.iter().any(|id| ours.contains(id)) => Some(attackers.clone()),
                    _ => None,
                })
                .unwrap_or_default();
            own_attackers_untapped = attacker_ids
                .iter()
                .filter(|id| {
                    ours.contains(id)
                        && untapped_ids.contains(id)
                        && game.object(**id).is_ok_and(|object| !object.tapped)
                })
                .count();
            let opponent_id = game.event_log.iter().find_map(|event| match event {
                GameEvent::SpellCast {
                    player: PlayerId(1),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == PRESSURE_ID) =>
                {
                    Some(*card)
                }
                _ => None,
            });
            opponent_remained_tapped =
                opponent_id.is_some_and(|id| game.object(id).is_ok_and(|object| object.tapped));
        }
    }
    let events = game.event_log.clone();
    let caress = events.iter().find_map(|event| match event {
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
    let ours = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CREATURE_ID) =>
            {
                Some(*card)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let opponent_creature = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(1),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|definition| definition.id == PRESSURE_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let cast_payment = caress
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
    Ok(Captured {
        events,
        terminal: game.winner().map_or_else(
            || "Draw".to_owned(),
            |p| format!("Winner(PlayerId({}))", p.0),
        ),
        winner: game.winner().map(|p| p.0),
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempted.saturating_sub(accepted),
        caress,
        ours,
        opponent_creature,
        cast_payment,
        creature_count,
        life_gained,
        untapped_ids,
        attacker_ids,
        own_attackers_untapped,
        opponent_remained_tapped,
        life_totals: [
            game.player(PlayerId(0)).map_err(|e| e.to_string())?.life,
            game.player(PlayerId(1)).map_err(|e| e.to_string())?.life,
        ],
    })
}

struct CaressPolicy {
    player: PlayerId,
    cast: bool,
}
impl CaressPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            cast: false,
        }
    }
}
impl CodePolicy for CaressPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.dryads-caress-combat-untap.v1"
        } else {
            "rav.watchwolf-pressure.v1"
        }
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let creature_id = if self.player == PlayerId(0) {
                CREATURE_ID
            } else {
                PRESSURE_ID
            };
            let own_creatures = view
                .own_battlefield
                .iter()
                .filter(|card| card.definition == Some(creature_id))
                .collect::<Vec<_>>();
            let ready_to_test = if self.player == PlayerId(0) {
                own_creatures.len() >= 2
                    && own_creatures.iter().filter(|card| card.can_attack).count() >= 2
                    && view
                        .hand
                        .iter()
                        .any(|card| card.definition == Some(CARD_ID))
                    && view
                        .own_battlefield
                        .iter()
                        .filter(|card| card.card_types.contains(&CardType::Land))
                        .count()
                        >= 6
            } else {
                own_creatures.len() >= 1
            };
            let attackers = if ready_to_test {
                own_creatures
                    .into_iter()
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
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.decision_player != self.player || view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if view.active_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::PrecombatMain && view.stack_depth == 0 {
            let lands = view
                .own_battlefield
                .iter()
                .filter(|card| card.card_types.contains(&CardType::Land))
                .count();
            if view.lands_played == 0 && lands < 8 {
                if let Some(land) = view
                    .hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
                {
                    return PolicyAction::PlayLand { card: land.id };
                }
            }
            let creature_id = if self.player == PlayerId(0) {
                CREATURE_ID
            } else {
                PRESSURE_ID
            };
            let own_count = view
                .own_battlefield
                .iter()
                .filter(|card| card.definition == Some(creature_id))
                .count();
            let max_count = if self.player == PlayerId(0) { 2 } else { 1 };
            if own_count < max_count {
                if let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(creature_id))
                {
                    let cost = if self.player == PlayerId(0) {
                        ManaCost::with_colors(0, [Color::Green, Color::White])
                    } else {
                        ManaCost::with_colors(0, [Color::Red])
                    };
                    if let Some(selection) = payment(view, &cost, None) {
                        return PolicyAction::CastWithPayment {
                            request: cardbench_magic_engine::CastRequest {
                                card: card.id,
                                targets: vec![],
                                convoke: vec![],
                                payment_mana_abilities: vec![],
                            },
                            chosen_x: None,
                            mana_selection: selection,
                        };
                    }
                    if let Some(action) = make_mana(view, &cost, None) {
                        return action;
                    }
                }
            }
            return PolicyAction::PassPriority;
        }
        if self.player == PlayerId(1) {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.attackers_declared
            && !self.cast
            && view.stack_depth == 0
        {
            let attack_count = view.combat_attackers.len();
            if attack_count >= 2
                && let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
            {
                let cost = ManaCost::with_colors(4, [Color::Green, Color::Green]);
                if let Some(selection) = payment(view, &cost, Some(Color::White)) {
                    self.cast = true;
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: selection,
                    };
                }
                if let Some(action) = make_mana(view, &cost, Some(Color::White)) {
                    return action;
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn payment(
    view: &GameView,
    cost: &ManaCost,
    generic_color: Option<Color>,
) -> Option<ManaPaymentSelection> {
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
    if let Some(color) = generic_color {
        let (_, amount) = available
            .iter_mut()
            .find(|(available_color, _)| *available_color == color)?;
        if *amount == 0 {
            return None;
        }
        *amount -= 1;
        generic.push(color);
    }
    for _ in generic.len()..cost.generic as usize {
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
fn make_mana(
    view: &GameView,
    cost: &ManaCost,
    generic_color: Option<Color>,
) -> Option<PolicyAction> {
    let mut need = cost
        .colored
        .iter()
        .copied()
        .find(|color| view.mana_pool.amount(*color) == 0);
    if need.is_none() && generic_color.is_some_and(|color| view.mana_pool.amount(color) == 0) {
        need = generic_color;
    }
    if need.is_none()
        && view.mana_pool.total() < cost.generic.saturating_add(cost.colored.len() as u8)
    {
        need = Some(Color::Green);
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

fn same_match(a: &Captured, b: &Captured) -> bool {
    a.events == b.events
        && a.terminal == b.terminal
        && a.winner == b.winner
        && a.caress == b.caress
        && a.ours == b.ours
        && a.opponent_creature == b.opponent_creature
        && a.cast_payment == b.cast_payment
        && a.creature_count == b.creature_count
        && a.life_gained == b.life_gained
        && a.untapped_ids == b.untapped_ids
        && a.attacker_ids == b.attacker_ids
        && a.own_attackers_untapped == b.own_attackers_untapped
        && a.opponent_remained_tapped == b.opponent_remained_tapped
        && a.life_totals == b.life_totals
}

fn validate_trace(c: &Captured, seed: u64) -> Result<Value, String> {
    let caress = c.caress.ok_or("Dryad's Caress was not cast")?;
    let opponent = c
        .opponent_creature
        .ok_or("opponent Watchwolf was not cast")?;
    if c.ours.len() < 2 {
        return Err("two controlled Watchwolves were not cast".to_owned());
    }
    if c.cast_payment.len() != 6
        || c.cast_payment
            .iter()
            .filter(|color| **color == Color::Green)
            .count()
            < 2
        || !c.cast_payment.contains(&Color::White)
    {
        return Err(format!(
            "Dryad's Caress did not pay {{4}}{{G}}{{G}} with White among generic mana: {:?}",
            c.cast_payment
        ));
    }
    if c.creature_count < 3 || c.life_gained != Some(c.creature_count as i32) {
        return Err(format!(
            "Dryad's Caress life gain {:?} did not equal {n} battlefield creatures",
            c.life_gained,
            n = c.creature_count
        ));
    }
    if c.own_attackers_untapped < 2 || !c.opponent_remained_tapped {
        return Err(format!(
            "conditional untap was not controller-only: our attackers {}, opponent stayed tapped {}, attack IDs {:?}, own IDs {:?}, untapped IDs {:?}",
            c.own_attackers_untapped,
            c.opponent_remained_tapped,
            c.attacker_ids,
            c.ours,
            c.untapped_ids
        ));
    }
    if c.terminal != "Winner(PlayerId(0))"
        || c.winner != Some(0)
        || c.life_totals[0] <= 0
        || c.life_totals[1] > 0
        || c.rejected_moves != 0
    {
        return Err(format!(
            "Dryad's Caress full game did not reach a clean Player 0 win: {:?}",
            c.life_totals
        ));
    }
    let attack = event_index(
        &c.events,
        |event| matches!(event,GameEvent::AttackersDeclared{player:PlayerId(0),attackers}if attackers.len()>=2),
    );
    let attack = attack.ok_or("two controlled attackers declaration missing")?;
    let paid=event_index_after(&c.events,attack,|event|matches!(event,GameEvent::SpellManaPaid{player:PlayerId(0),card,colors}if *card==caress&&colors.len()==6&&colors.contains(&Color::White)&&colors.iter().filter(|color|**color==Color::Green).count()>=2)).ok_or("Dryad's Caress exact {4}{G}{G} White-spent payment missing")?;
    let cast = event_index_after(
        &c.events,
        paid,
        |event| matches!(event,GameEvent::SpellCast{player:PlayerId(0),card}if *card==caress),
    )
    .ok_or("Dryad's Caress cast receipt missing")?;
    let gain=event_index_after(&c.events,cast,|event|matches!(event,GameEvent::LifeGained{player:PlayerId(0),amount}if *amount==c.creature_count as i32)).ok_or("Dryad's Caress battlefield-creature-count life event missing")?;
    let untap=event_index_after(&c.events,gain,|event|matches!(event,GameEvent::PermanentUntapped{source,card}if *source==caress&&c.ours.contains(card))).ok_or("Dryad's Caress did not untap its controller's creatures")?;
    let resolved = event_index_after(
        &c.events,
        untap,
        |event| matches!(event,GameEvent::SpellResolved{card}if *card==caress),
    )
    .ok_or("Dryad's Caress spell resolution receipt missing")?;
    let ended = event_index_after(&c.events, resolved, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal Player 0 win receipt missing")?;
    if !(attack < paid
        && paid < cast
        && cast < gain
        && gain < untap
        && untap < resolved
        && resolved < ended)
    {
        return Err("Dryad's Caress combat, payment, life, untap, resolution, and terminal ordering is invalid".to_owned());
    }
    let projected = project_events(&c.events);
    let events:Vec<Value>=projected.iter().map(|event|json!({"sequence":event.sequence,"turn":event.turn,"kind":&event.kind,"canonical":&event.canonical,"facts":&event.facts})).collect();
    Ok(
        json!({"id":"rav-dryads-caress-battlefield-count-and-conditional-untap-full-game","passed":true,"shuffle_seed":seed,"decks":[DECK_P0,DECK_P1],"terminal":c.terminal,"winner_seat":c.winner,"turns":c.turns,"accepted_moves":c.accepted_moves,"rejected_moves":c.rejected_moves,
        "card":{"object":caress.0,"mana_cost":"{4}{G}{G}","cast_mana_paid":c.cast_payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>()},
        "battlefield_creature_count":c.creature_count,"life_gained":c.life_gained,"white_mana_spent":true,
        "untap":{"controller_attackers_untapped":c.own_attackers_untapped,"opponent_creature_object":opponent.0,"opponent_remained_tapped":c.opponent_remained_tapped,"untapped_objects":c.untapped_ids.iter().map(|id|id.0).collect::<Vec<_>>(),"attacker_objects":c.attacker_ids.iter().map(|id|id.0).collect::<Vec<_>>()},
        "ordered_event_sequence":[attack,paid,cast,gain,untap,resolved,ended],"events":events}),
    )
}
fn event_index(events: &[GameEvent], mut pred: impl FnMut(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(|event| pred(event))
}
fn event_index_after(
    events: &[GameEvent],
    after: usize,
    mut pred: impl FnMut(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(after + 1)
        .find_map(|(index, event)| pred(event).then_some(index))
}

fn validate_candidate_definition() -> Result<(), String> {
    let card = candidate_dryads_caress::definition();
    if card.id != CARD_ID
        || card.name != "Dryad's Caress"
        || card.set_code != "RAV"
        || card.mana_cost != ManaCost::with_colors(4, [Color::Green, Color::Green])
        || card.colors != BTreeSet::from([Color::Green])
        || card.card_types != BTreeSet::from([CardType::Instant])
        || card.is_basic_land
        || !card.mana_colors.is_empty()
        || card.power.is_some()
        || card.toughness.is_some()
        || !card.keywords.is_empty()
        || card.effects
            != [
                Effect::GainLifeForEachCreature,
                Effect::UntapControllerCreaturesIfManaColorSpent {
                    color: Color::White,
                },
            ]
    {
        return Err("Dryad's Caress must gain life per battlefield creature and untap its controller's creatures if White mana was spent".to_owned());
    }
    Ok(())
}
fn validate_registry_delegation() -> Result<(), String> {
    let candidate = candidate_dryads_caress::definition();
    if card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .as_ref()
        != Some(&candidate)
    {
        return Err(
            "Dryad's Caress registry definition does not delegate to its candidate module"
                .to_owned(),
        );
    }
    Ok(())
}
fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|e| format!("could not read Dryad's Caress source: {e}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Dryad's Caress source is not valid declarative Rust")?;
    if !file
        .attrs
        .iter()
        .all(|attr| attr.path().is_ident("doc") && matches!(attr.meta, Meta::NameValue(_)))
    {
        return Err("Dryad's Caress module has unsupported attributes".to_owned());
    }
    let expected: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "ManaCost"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut found = false;
    for item in &file.items {
        match item {
            Item::Use(item)
                if matches!(item.vis, Visibility::Inherited) && item.attrs.is_empty() =>
            {
                imports.extend(
                    flattened_use_paths(item).ok_or("Dryad's Caress import structure differs")?,
                )
            }
            Item::Fn(function) if !found => {
                found = true;
                if function.sig.ident != "definition"
                    || !matches!(&function.vis,Visibility::Restricted(vis)if vis.path.is_ident("crate"))
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || !function.sig.inputs.is_empty()
                    || !function.sig.generics.params.is_empty()
                    || function.sig.variadic.is_some()
                    || function.block.stmts.len() != 1
                    || !function.attrs.iter().all(|attr| {
                        (attr.path().is_ident("doc") && matches!(attr.meta, Meta::NameValue(_)))
                            || attr.path().is_ident("must_use")
                    })
                {
                    return Err(
                        "Dryad's Caress may expose only a safe no-argument definition()".to_owned(),
                    );
                }
                let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
                    return Err(
                        "Dryad's Caress definition must return a direct struct literal".to_owned(),
                    );
                };
                if value.rest.is_some() || !declarative_expr(&Expr::Struct(value.clone())) {
                    return Err(
                        "Dryad's Caress source contains unsupported executable Rust".to_owned()
                    );
                }
            }
            _ => {
                return Err(
                    "Dryad's Caress source permits only imports and definition()".to_owned(),
                );
            }
        }
    }
    if !found || imports != expected {
        return Err("Dryad's Caress source imports or candidate function set differs".to_owned());
    }
    Ok(())
}
fn declarative_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(_) | Expr::Path(_) => true,
        Expr::Unary(x) => matches!(x.op, syn::UnOp::Neg(_)) && matches!(&*x.expr, Expr::Lit(_)),
        Expr::Reference(x) => declarative_expr(&x.expr),
        Expr::Array(x) => x.elems.iter().all(declarative_expr),
        Expr::Struct(x) => {
            x.attrs.is_empty()
                && x.fields
                    .iter()
                    .all(|f| matches!(f.member, syn::Member::Named(_)) && declarative_expr(&f.expr))
                && x.rest.as_deref().is_none_or(declarative_expr)
        }
        Expr::Call(x) => {
            let allowed = match &*x.func {
                Expr::Path(path) if path.qself.is_none() => {
                    let names: Vec<_> = path
                        .path
                        .segments
                        .iter()
                        .map(|s| s.ident.to_string())
                        .collect();
                    matches!(names.as_slice(),[a,b]if(a=="BTreeSet"&&(b=="from"||b=="new"))||(a=="ManaCost"&&(b=="new"||b=="with_colors")))
                        || matches!(names.as_slice(),[name]if name=="Some")
                }
                _ => false,
            };
            allowed && x.args.iter().all(declarative_expr)
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
