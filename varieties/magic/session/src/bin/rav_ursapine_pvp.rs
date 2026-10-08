//! Full-game RAV Ursapine authority for its {G} target-creature +1/+1 ability.

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbilityBinding, CardType, Color, DeckEntry, DeckList, Effect, Game,
    GameEvent, GameView, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step,
    Target, TargetRequirement, Zone,
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
const CARD_ID: &str = "RAV-URSAPINE";
const CREATURE_ID: &str = "RAV-WATCHWOLF";
const ABILITY_ID: &str = "ursapine-target-pump";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/ursapine.rs";
const DECK_P0: &str = "rav_ursapine_watchwolf_pump";
const DECK_P1: &str = "rav_basic_plains_opponent";

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/ursapine.rs"]
mod candidate_ursapine;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Ursapine PvP result: {error}");
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
            eprintln!("Ursapine PvP authority failed: {error}");
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
        return Err("Ursapine full-game replay was not deterministic".to_owned());
    }
    let mutations = mutation_sensitivity(seed)?;
    for key in [
        "zero_power_mutation_detected_in_game",
        "zero_toughness_mutation_detected_in_game",
        "wrong_ability_cost_mutation_detected_in_game",
        "ability_binding_removal_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!("Ursapine mutation escaped the PvP trace: {key}"));
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
        "card":{"collector_number":188,"definition_id":CARD_ID,"name":"Ursapine"},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":[
                "105.2 and 202.1-202.2: green color and colored mana payment",
                "106.1 and 106.4: mana production and activated-ability payment",
                "117: timing and priority",
                "602.2: activating an activated ability",
                "601.2 and 608.3a: casting and resolving a creature spell",
                "608.2: resolving an activated ability with a target",
                "613.4: power/toughness modification layer",
                "514: cleanup and expiration of until-end-of-turn effects",
                "508.1 and 510: combat and combat damage"
            ],
            "source_contract":"RAV Ursapine #188, {3}{G}{G} 3/3 green creature; {G}: target creature gets +1/+1 until end of turn",
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
    json!({"collector_number":card.collector_number,"definition_id":definition_id,"name":card.name,
        "engine_status":engine_status,"qualification":if passed{"pilot_trace_passed; independent reviewer approval pending"}else{"not_evaluated"},
        "trace_status":if passed{"passed"}else{"not_qualified"},"credited":false})
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    ZeroPower,
    ZeroToughness,
    WrongCost,
    RemoveBinding,
}

#[derive(Clone)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    ursapine: Option<ObjectId>,
    watchwolf: Option<ObjectId>,
    cast_payment: Vec<Color>,
    activation_payments: usize,
    resolutions: usize,
    live_pt: Option<(i32, i32)>,
    cleanup_pt: Option<(i32, i32)>,
    powered_damage: usize,
    life_totals: [i64; 2],
}

fn find_trace() -> Result<(u64, Captured), String> {
    for seed in 0..256 {
        let captured = run_match(seed, Mutation::None)?;
        match validate_trace(&captured, seed) {
            Ok(_) => return Ok((seed, captured)),
            Err(error) if error.contains("was not cast") => continue,
            Err(error) => return Err(error),
        }
    }
    Err("no bounded seed produced a full-game Ursapine pump trace".to_owned())
}

fn mutation_sensitivity(seed: u64) -> Result<Value, String> {
    let mut outcomes = serde_json::Map::new();
    for (key, mutation) in [
        ("zero_power_mutation_detected_in_game", Mutation::ZeroPower),
        (
            "zero_toughness_mutation_detected_in_game",
            Mutation::ZeroToughness,
        ),
        (
            "wrong_ability_cost_mutation_detected_in_game",
            Mutation::WrongCost,
        ),
        (
            "ability_binding_removal_detected_in_game",
            Mutation::RemoveBinding,
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
        mainboard: vec![DeckEntry {
            card: "RAV-PLAINS".to_owned(),
            count: 60,
        }],
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
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Ursapine definition is absent from RAV registry")? =
        candidate_ursapine::definition();
    let mut bindings = rav_activated_ability_bindings();
    bindings.retain(|binding| binding.card_definition != CARD_ID);
    if mutation != Mutation::RemoveBinding {
        let mut ability = candidate_ursapine::activated_ability().ability;
        match mutation {
            Mutation::ZeroPower => {
                ability.effects = vec![Effect::ModifyTargetPtUntilEndOfTurn {
                    power: 0,
                    toughness: 1,
                }]
            }
            Mutation::ZeroToughness => {
                ability.effects = vec![Effect::ModifyTargetPtUntilEndOfTurn {
                    power: 1,
                    toughness: 0,
                }]
            }
            Mutation::WrongCost => ability.mana_cost = ManaCost::with_colors(1, [Color::Green]),
            _ => {}
        }
        bindings.push(ActivatedAbilityBinding {
            card_definition: CARD_ID,
            ability,
        });
    }
    let mut game = Game::new_with_all_bindings(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        bindings,
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
        Box::new(UrsapinePolicy::new(PlayerId(0))),
        Box::new(UrsapinePolicy::new(PlayerId(1))),
    ];
    let (mut accepted, mut attempted) = (0u32, 0u32);
    let mut live_pt = None;
    let mut cleanup_pt = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns.min(60) || accepted >= config.max_policy_moves {
            return Err("Ursapine match exceeded its bounded turn or move limit".to_owned());
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
        if live_pt.is_none() {
            if let Some(source) = game.event_log[cursor..]
                .iter()
                .find_map(|event| match event {
                    GameEvent::AbilityResolved {
                        source,
                        ability: ABILITY_ID,
                        ..
                    } => Some(*source),
                    _ => None,
                })
            {
                let _ = source;
                let target = game.event_log.iter().find_map(|event| match event {
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
                });
                if let Some(target) = target {
                    live_pt = game
                        .characteristics(target)
                        .ok()
                        .and_then(|c| Some((c.power?, c.toughness?)));
                }
            }
        }
        if cleanup_pt.is_none()
            && let Some(target) = game.event_log[cursor..]
                .iter()
                .find_map(|event| match event {
                    GameEvent::ContinuousEffectExpired { source, target, .. }
                        if game.card_definition(*source).is_ok_and(|d| d.id == CARD_ID) =>
                    {
                        Some(*target)
                    }
                    _ => None,
                })
        {
            cleanup_pt = game
                .characteristics(target)
                .ok()
                .and_then(|c| Some((c.power?, c.toughness?)));
        }
    }
    let events = game.event_log.clone();
    let ursapine = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|def| def.id == CARD_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let watchwolf = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|def| def.id == CREATURE_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let cast_payment = ursapine
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
    let activation_payments = ursapine.map_or(0, |source| events.iter().filter(|event| matches!(event,
        GameEvent::AbilityManaPaid { player: PlayerId(0), source: paid_source, ability: ABILITY_ID, mana_cost }
            if *paid_source == source && mana_cost == &ManaCost::with_colors(0, [Color::Green])
    )).count());
    let resolutions = ursapine.map_or(0, |source| events.iter().filter(|event| matches!(event,
        GameEvent::AbilityResolved { source: paid_source, ability: ABILITY_ID, .. } if *paid_source == source
    )).count());
    let powered_damage = watchwolf.map_or(0, |target| events.iter().filter(|event| matches!(event,
        GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 4 } if *source == target
    )).count());
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
        ursapine,
        watchwolf,
        cast_payment,
        activation_payments,
        resolutions,
        live_pt,
        cleanup_pt,
        powered_damage,
        life_totals: [
            game.player(PlayerId(0)).map_err(|e| e.to_string())?.life,
            game.player(PlayerId(1)).map_err(|e| e.to_string())?.life,
        ],
    })
}

struct UrsapinePolicy {
    player: PlayerId,
    activated: bool,
}
impl UrsapinePolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            activated: false,
        }
    }
}
impl CodePolicy for UrsapinePolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.ursapine-watchwolf-pump.v1"
        } else {
            "rav.passive-plains.v1"
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
        if view.decision_player != self.player || view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
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
        if self.player != PlayerId(0) {
            return PolicyAction::PassPriority;
        }
        let ursapine = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some(CARD_ID));
        if ursapine.is_none() {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            {
                let cost = ManaCost::with_colors(3, [Color::Green, Color::Green]);
                if let Some(selection) = payment(view, &cost) {
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
                if let Some(action) = make_mana(view, &cost) {
                    return action;
                }
            }
            return PolicyAction::PassPriority;
        }
        let watchwolf = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some(CREATURE_ID));
        if watchwolf.is_none() {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CREATURE_ID))
            {
                let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
                if let Some(selection) = payment(view, &cost) {
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
                if let Some(action) = make_mana(view, &cost) {
                    return action;
                }
            }
            return PolicyAction::PassPriority;
        }
        if !self.activated && watchwolf.is_some_and(|target| target.can_attack) {
            let source = ursapine.expect("checked above");
            let target = watchwolf.expect("checked above");
            let cost = ManaCost::with_colors(0, [Color::Green]);
            if payment(view, &cost).is_some() {
                self.activated = true;
                return PolicyAction::ActivateAbility {
                    activation: AbilityActivation {
                        source: source.id,
                        ability_id: ABILITY_ID,
                        sacrifice_sources: vec![],
                        additional_tap_creatures: vec![],
                        discard_cards: vec![],
                        targets: vec![Target::Permanent(target.id)],
                    },
                };
            }
            if let Some(action) = make_mana(view, &cost) {
                return action;
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

fn make_mana(view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
    let mut need = cost
        .colored
        .iter()
        .copied()
        .find(|color| view.mana_pool.amount(*color) == 0);
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

fn same_match(left: &Captured, right: &Captured) -> bool {
    left.events == right.events
        && left.terminal == right.terminal
        && left.winner == right.winner
        && left.ursapine == right.ursapine
        && left.watchwolf == right.watchwolf
        && left.cast_payment == right.cast_payment
        && left.activation_payments == right.activation_payments
        && left.resolutions == right.resolutions
        && left.live_pt == right.live_pt
        && left.cleanup_pt == right.cleanup_pt
        && left.powered_damage == right.powered_damage
}

fn validate_trace(c: &Captured, seed: u64) -> Result<Value, String> {
    let ursapine = c.ursapine.ok_or("Ursapine was not cast")?;
    let watchwolf = c
        .watchwolf
        .ok_or("Watchwolf target creature was not cast")?;
    if c.cast_payment.len() != 5
        || c.cast_payment
            .iter()
            .filter(|color| **color == Color::Green)
            .count()
            < 2
    {
        return Err(format!(
            "Ursapine did not pay {{3}}{{G}}{{G}}: {:?}",
            c.cast_payment
        ));
    }
    if c.activation_payments < 1
        || c.resolutions < 1
        || c.live_pt != Some((4, 4))
        || c.cleanup_pt != Some((3, 3))
    {
        return Err(format!(
            "Ursapine {{G}} ability did not resolve on Watchwolf and expire correctly: live {:?}, cleanup {:?}",
            c.live_pt, c.cleanup_pt
        ));
    }
    if c.powered_damage < 1 {
        return Err("the pumped 4/4 Watchwolf did not deal four combat damage".to_owned());
    }
    if c.terminal != "Winner(PlayerId(0))"
        || c.winner != Some(0)
        || c.life_totals[0] <= 0
        || c.life_totals[1] > 0
        || c.rejected_moves != 0
    {
        return Err(format!(
            "Ursapine full game did not reach a clean seat-zero win: {:?}",
            c.life_totals
        ));
    }
    let paid = event_index(&c.events, |e| matches!(e, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == ursapine && colors.len() == 5))
        .ok_or("Ursapine mana payment receipt missing")?;
    let cast = event_index_after(
        &c.events,
        paid,
        |e| matches!(e, GameEvent::SpellCast { player: PlayerId(0), card } if *card == ursapine),
    )
    .ok_or("Ursapine cast receipt missing")?;
    let resolved = event_index_after(
        &c.events,
        cast,
        |e| matches!(e, GameEvent::SpellResolved { card } if *card == ursapine),
    )
    .ok_or("Ursapine did not resolve")?;
    let entered = event_index_after(
        &c.events,
        resolved,
        |e| matches!(e, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == ursapine),
    )
    .ok_or("Ursapine did not enter the battlefield")?;
    let target_cast = event_index(
        &c.events,
        |e| matches!(e, GameEvent::SpellCast { player: PlayerId(0), card } if *card == watchwolf),
    )
    .ok_or("Watchwolf cast receipt missing")?;
    let ability_paid = event_index_after(&c.events, entered, |e| matches!(e, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: ABILITY_ID, mana_cost } if *source == ursapine && mana_cost == &ManaCost::with_colors(0, [Color::Green]))).ok_or("Ursapine exact {G} activation payment missing")?;
    let activation = event_index_after(&c.events, ability_paid, |e| matches!(e, GameEvent::AbilityActivated { player: PlayerId(0), source, ability: ABILITY_ID, .. } if *source == ursapine)).ok_or("Ursapine ability was not stacked")?;
    let ability_resolved = event_index_after(&c.events, activation, |e| matches!(e, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == ursapine)).ok_or("Ursapine ability did not resolve")?;
    let attack = event_index_after(&c.events, ability_resolved, |e| matches!(e, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&watchwolf))).ok_or("pumped Watchwolf did not attack")?;
    let damage = event_index_after(&c.events, attack, |e| matches!(e, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 4 } if *source == watchwolf)).ok_or("pumped Watchwolf did not deal four damage")?;
    let ended = event_index_after(&c.events, damage, |e| {
        matches!(
            e,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal Player 0 win receipt missing")?;
    if !(paid < cast
        && cast < resolved
        && resolved < entered
        && target_cast < ability_paid
        && ability_paid < activation
        && activation < ability_resolved
        && ability_resolved < attack
        && attack < damage
        && damage < ended)
    {
        return Err("Ursapine payment, cast, activation, target, combat, and terminal event order is invalid".to_owned());
    }
    let projected = project_events(&c.events);
    let events: Vec<Value> = projected.iter().map(|event| json!({"sequence":event.sequence,"turn":event.turn,"kind":&event.kind,"canonical":&event.canonical,"facts":&event.facts})).collect();
    Ok(json!({
        "id":"rav-ursapine-targeted-pump-full-game","passed":true,"shuffle_seed":seed,"decks":[DECK_P0,DECK_P1],
        "terminal":c.terminal,"winner_seat":c.winner,"turns":c.turns,"accepted_moves":c.accepted_moves,"rejected_moves":c.rejected_moves,
        "ursapine":{"object":ursapine.0,"mana_cost":"{3}{G}{G}","cast_mana_paid":c.cast_payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"zone":"Battlefield"},
        "target":{"definition_id":CREATURE_ID,"object":watchwolf.0,"base_power":3,"base_toughness":3,"live_power":c.live_pt.map(|pt|pt.0),"live_toughness":c.live_pt.map(|pt|pt.1),"post_cleanup_power":c.cleanup_pt.map(|pt|pt.0),"post_cleanup_toughness":c.cleanup_pt.map(|pt|pt.1)},
        "activation":{"ability":ABILITY_ID,"mana_cost":"{G}","paid_activations":c.activation_payments,"resolutions":c.resolutions,"target_object":watchwolf.0,"targeted_creature_gets_plus_one_plus_one":true,"until_end_of_turn_expired":true},
        "combat":{"pumped_target_four_damage_count":c.powered_damage,"life_at_terminal":c.life_totals},
        "ordered_event_sequence":[paid,cast,resolved,entered,target_cast,ability_paid,activation,ability_resolved,attack,damage,ended],"events":events
    }))
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
    let card = candidate_ursapine::definition();
    if card.id != CARD_ID
        || card.name != "Ursapine"
        || card.set_code != "RAV"
        || card.mana_cost != ManaCost::with_colors(3, [Color::Green, Color::Green])
        || card.colors != BTreeSet::from([Color::Green])
        || card.card_types != BTreeSet::from([CardType::Creature])
        || card.power != Some(3)
        || card.toughness != Some(3)
        || card.is_basic_land
        || !card.mana_colors.is_empty()
        || !card.keywords.is_empty()
    {
        return Err(
            "Ursapine printed identity, {3}{G}{G} cost, or base 3/3 characteristics differ"
                .to_owned(),
        );
    }
    let binding = candidate_ursapine::activated_ability();
    let ability = binding.ability;
    if binding.card_definition != CARD_ID
        || ability.id != ABILITY_ID
        || ability.mana_cost != ManaCost::with_colors(0, [Color::Green])
        || ability.tap_cost
        || ability.sorcery_speed
        || ability.additional_tap_creatures != 0
        || ability.sacrifice_source
        || ability.sacrifice_creatures != 0
        || ability.sacrifice_lands != 0
        || ability.discard_cards != 0
        || ability.targets != [TargetRequirement::Creature]
        || ability.effects
            != [Effect::ModifyTargetPtUntilEndOfTurn {
                power: 1,
                toughness: 1,
            }]
    {
        return Err(
            "Ursapine must bind {G}: target creature gets +1/+1 until end of turn".to_owned(),
        );
    }
    Ok(())
}

fn validate_registry_delegation() -> Result<(), String> {
    let candidate = candidate_ursapine::definition();
    if card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .as_ref()
        != Some(&candidate)
    {
        return Err(
            "Ursapine registry definition does not delegate to its candidate module".to_owned(),
        );
    }
    let expected = candidate_ursapine::activated_ability();
    if rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .as_ref()
        != Some(&expected)
    {
        return Err(
            "Ursapine registry ability does not delegate to its candidate module".to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|e| format!("could not read Ursapine source: {e}"))?;
    let file =
        syn::parse_file(&source).map_err(|_| "Ursapine source is not valid declarative Rust")?;
    if !file
        .attrs
        .iter()
        .all(|attr| attr.path().is_ident("doc") && matches!(attr.meta, Meta::NameValue(_)))
    {
        return Err("Ursapine module has an unsupported module attribute".to_owned());
    }
    let expected: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "ActivatedAbility"],
        vec!["cardbench_magic_engine", "ActivatedAbilityBinding"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TargetRequirement"],
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
                if matches!(item.vis, Visibility::Inherited) && item.attrs.is_empty() =>
            {
                imports
                    .extend(flattened_use_paths(item).ok_or("Ursapine import structure differs")?);
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !["definition", "activated_ability"].contains(&name.as_str())
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
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
                        "Ursapine may expose only safe no-argument candidate functions".to_owned(),
                    );
                }
                let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
                    return Err(
                        "Ursapine candidate functions must return direct struct literals"
                            .to_owned(),
                    );
                };
                if value.rest.is_some() || !declarative_expr(&Expr::Struct(value.clone())) {
                    return Err(
                        "Ursapine source contains an unsupported executable expression".to_owned(),
                    );
                }
                functions.insert(name);
            }
            _ => {
                return Err(
                    "Ursapine source permits only imports and candidate functions".to_owned(),
                );
            }
        }
    }
    if imports != expected
        || functions != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()])
    {
        return Err("Ursapine source imports or candidate function set differs".to_owned());
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
                        .map(|s| s.ident.to_string())
                        .collect();
                    matches!(names.as_slice(), [a,b] if (a == "BTreeSet" && (b == "from" || b == "new")) || (a == "ManaCost" && (b == "new" || b == "with_colors")))
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
