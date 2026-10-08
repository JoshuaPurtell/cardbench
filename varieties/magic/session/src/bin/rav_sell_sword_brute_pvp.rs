//! Full-game RAV Sell-Sword Brute dies-trigger authority.

use cardbench_magic_engine::{
    CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, Zone,
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
use syn::{Expr, Item, Meta, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-SELL-SWORD-BRUTE";
const TARGET_ID: &str = CARD_ID;
const GASP_ID: &str = "RAV-LAST-GASP";
const WIN_CONDITION_ID: &str = "RAV-WATCHWOLF";
const ABILITY_ID: &str = "dies-deal-two-to-controller";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/sell_sword_brute.rs";
const DECK_P0: &str = "rav_sell_sword_brute_red_death_test";
const DECK_P1: &str = "rav_sell_sword_brute_red_pilot";

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/sell_sword_brute.rs"]
mod candidate_sell_sword_brute;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Sell-Sword Brute PvP result: {error}");
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
            eprintln!("Sell-Sword Brute PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    let catalog = unique_catalog()?;
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.terminal != replay.terminal
        || captured.winner != replay.winner
        || captured.gasp != replay.gasp
        || captured.opponent_target != replay.opponent_target
        || captured.gasp_payment != replay.gasp_payment
        || captured.target_payment != replay.target_payment
        || captured.target_zone != replay.target_zone
        || captured.life_totals != replay.life_totals
        || captured.pilot_attack_turns != replay.pilot_attack_turns
        || captured.trigger_source_incarnation != replay.trigger_source_incarnation
        || captured.resolved_source_incarnation != replay.resolved_source_incarnation
        || captured.damage_to_controller != replay.damage_to_controller
    {
        return Err("Sell-Sword Brute full-game replay was not deterministic".to_owned());
    }
    let mutation_sensitivity = json!({
        "dies_trigger_removal_detected_in_game": mutation_reaches_trace_and_fails(seed, Mutation::RemoveTrigger)?,
        "one_damage_mutation_detected_in_game": mutation_reaches_trace_and_fails(seed, Mutation::OneDamage)?,
        "sealed_mutation_authority": false
    });
    if mutation_sensitivity["dies_trigger_removal_detected_in_game"] != true
        || mutation_sensitivity["one_damage_mutation_detected_in_game"] != true
    {
        return Err("a Sell-Sword Brute dies-trigger mutation escaped its PvP trace".to_owned());
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
        "card":{"collector_number":142,"definition_id":CARD_ID,"name":"Sell-Sword Brute"},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":[
                "105.2 and 202.1-202.2: red creature color and colored payment",
                "106.1 and 106.4: mana production and spell payment",
                "117: timing and priority",
                "302.1 and 601.2: creature spell casting and resolution",
                "603.2 and 603.6c: dies trigger event and trigger condition",
                "608.2: resolving a triggered ability",
                "120.2: a resolving source-damage effect deals damage",
                "510: combat damage and terminal game state"
            ],
            "source_contract":"RAV printed Sell-Sword Brute #142 identity, {1}{R} 2/2 red creature, and candidate-owned mandatory dies trigger that deals two damage to its controller",
            "authority_cutoff":"2026-09-23",
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
        "mutation_sensitivity":mutation_sensitivity,
        "traces":[trace],
        "per_card_breakdown":breakdown
    }))
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
    RemoveTrigger,
    OneDamage,
}

#[derive(Clone)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    gasp: Option<ObjectId>,
    opponent_target: Option<ObjectId>,
    pilot_watchwolf: Option<ObjectId>,
    gasp_payment: Vec<Color>,
    target_payment: Vec<Color>,
    gasp_zone: Option<Zone>,
    target_zone: Option<Zone>,
    life_totals: [i64; 2],
    pilot_attack_turns: Vec<u32>,
    trigger_source_incarnation: Option<u64>,
    resolved_source_incarnation: Option<u64>,
    damage_to_controller: Option<i32>,
}

fn find_trace() -> Result<(u64, Captured), String> {
    for seed in 0..512 {
        let captured = run_match(seed, Mutation::None)?;
        match validate_trace(&captured, seed) {
            Ok(_) => return Ok((seed, captured)),
            Err(error) if error.contains("was not drawn") || error.contains("was not cast") => {}
            Err(error) => return Err(error),
        }
    }
    Err("no bounded seed produced a Sell-Sword Brute death-trigger PvP trace".to_owned())
}

fn mutation_reaches_trace_and_fails(seed: u64, mutation: Mutation) -> Result<bool, String> {
    let captured = run_match(seed, mutation)?;
    let expected = match mutation {
        Mutation::RemoveTrigger | Mutation::OneDamage => {
            "Sell-Sword Brute did not deal exactly two damage to its controller"
        }
        Mutation::None => return Ok(false),
    };
    Ok(validate_trace(&captured, seed)
        .err()
        .is_some_and(|error| error.contains(expected)))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot = DeckList {
        mainboard: vec![
            DeckEntry {
                card: GASP_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: WIN_CONDITION_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-SWAMP".to_owned(),
                count: 17,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 17,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 18,
            },
        ],
        sideboard: vec![],
    };
    let opponent = DeckList {
        mainboard: vec![
            DeckEntry {
                card: TARGET_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 56,
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
    let submitted = candidate_sell_sword_brute::definition();
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Sell-Sword Brute definition is absent from RAV registry")? = submitted;
    let mut triggers = rav_triggered_ability_bindings();
    if mutation == Mutation::RemoveTrigger {
        triggers.retain(|binding| {
            !(binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        });
    }
    if mutation == Mutation::OneDamage {
        let binding = triggers
            .iter_mut()
            .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
            .ok_or("Sell-Sword Brute trigger binding is absent from RAV registry")?;
        binding.ability.effects = vec![Effect::DealDamageController { amount: 1 }];
    }
    let mut game = Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
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
        ..DeckMatchConfig::default()
    };
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|e| e.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|e| e.to_string())?;
    game.begin_game().map_err(|e| e.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(PilotPolicy::new(PlayerId(0))),
        Box::new(PilotPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0u32;
    let mut attempted = 0u32;
    let mut attack_turns = Vec::new();
    while !game.is_game_over() {
        if game.turn > config.max_turns.min(40) || accepted >= config.max_policy_moves {
            return Err(
                "Sell-Sword Brute match exceeded the bounded turn or move limit".to_owned(),
            );
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
                    "policy move rejected on turn {} at {:?}: {e}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|e| format!("post-move invariant failure: {e}"))?;
        if game.event_log[cursor..].iter().any(|event|matches!(event,GameEvent::AttackersDeclared{player:PlayerId(0),attackers} if attackers.iter().any(|id|game.card_definition(*id).is_ok_and(|card|card.id==WIN_CONDITION_ID)))) { attack_turns.push(game.turn); }
    }
    let events = game.event_log.clone();
    let gasp = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|def| def.id == GASP_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let opponent_target = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(1),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|def| def.id == TARGET_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let pilot_watchwolf = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|def| def.id == WIN_CONDITION_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let gasp_payment = gasp
        .and_then(|gasp| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if *card == gasp => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    let target_payment = opponent_target
        .and_then(|target| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(1),
                    card,
                    colors,
                } if *card == target => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    let target_zone = opponent_target.and_then(|target| {
        events.iter().rev().find_map(|event| match event {
            GameEvent::CardMoved { card, to } if *card == target => Some(*to),
            _ => None,
        })
    });
    let gasp_zone = gasp.and_then(|spell| {
        events.iter().rev().find_map(|event| match event {
            GameEvent::CardMoved { card, to } if *card == spell => Some(*to),
            _ => None,
        })
    });
    let trigger_source_incarnation = events.iter().find_map(|event| match event {
        GameEvent::TriggeredAbilityStacked {
            source,
            source_incarnation,
            ability,
            ..
        } if Some(*source) == opponent_target && *ability == ABILITY_ID => {
            Some(*source_incarnation)
        }
        _ => None,
    });
    let resolved_source_incarnation = events.iter().find_map(|event| match event {
        GameEvent::AbilityResolved {
            source,
            source_incarnation,
            ability,
        } if Some(*source) == opponent_target && *ability == ABILITY_ID => {
            Some(*source_incarnation)
        }
        _ => None,
    });
    let damage_to_controller = events.iter().find_map(|event| match event {
        GameEvent::DamageDealtToPlayer {
            source,
            player: PlayerId(1),
            amount,
        } if Some(*source) == opponent_target => Some(*amount),
        _ => None,
    });
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
        gasp,
        opponent_target,
        pilot_watchwolf,
        gasp_payment,
        target_payment,
        gasp_zone,
        target_zone,
        life_totals: [
            game.player(PlayerId(0)).map_err(|e| e.to_string())?.life,
            game.player(PlayerId(1)).map_err(|e| e.to_string())?.life,
        ],
        pilot_attack_turns: attack_turns,
        trigger_source_incarnation,
        resolved_source_incarnation,
        damage_to_controller,
    })
}

struct PilotPolicy {
    player: PlayerId,
    candidate_deployed: bool,
}
impl PilotPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            candidate_deployed: false,
        }
    }
}
impl CodePolicy for PilotPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.sell-sword-brute-removal-and-win.v1"
        } else {
            "rav.sell-sword-brute-red-cast.v1"
        }
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let target_live = view
                .opponent_battlefield
                .iter()
                .any(|card| card.definition == Some(TARGET_ID));
            let attackers = if self.player == PlayerId(0) && target_live {
                vec![]
            } else {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(WIN_CONDITION_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect()
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
        let target = view
            .opponent_battlefield
            .iter()
            .find(|card| card.definition == Some(TARGET_ID));
        if self.player == PlayerId(0) {
            if let (Some(card), Some(target)) = (
                view.hand
                    .iter()
                    .find(|card| card.definition == Some(GASP_ID)),
                target,
            ) {
                let cost = ManaCost::with_colors(1, [Color::Black]);
                if let Some(selection) = payment(view, &cost) {
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![Target::Permanent(target.id)],
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
        }
        if view.active_player == self.player
            && view.step == Step::PrecombatMain
            && view.stack_depth == 0
        {
            if view.lands_played == 0 {
                if let Some(land) = view
                    .hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
                {
                    return PolicyAction::PlayLand { card: land.id };
                }
            }
            let (spell_id, cost) = if self.player == PlayerId(1) {
                (CARD_ID, ManaCost::with_colors(1, [Color::Red]))
            } else {
                (
                    WIN_CONDITION_ID,
                    ManaCost::with_colors(0, [Color::Green, Color::White]),
                )
            };
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(spell_id))
                && !(self.player == PlayerId(1) && self.candidate_deployed)
            {
                if let Some(selection) = payment(view, &cost) {
                    if self.player == PlayerId(1) {
                        self.candidate_deployed = true;
                    }
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
        }
        if view.active_player == self.player
            && view.step == Step::PrecombatMain
            && view.lands_played == 0
        {
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        PolicyAction::PassPriority
    }
}

fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<_> = Color::MANA_ALL
        .into_iter()
        .map(|c| (c, view.mana_pool.amount(c)))
        .collect();
    for color in &cost.colored {
        let (_, amount) = available.iter_mut().find(|(c, _)| c == color)?;
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
    let mut need = None;
    for color in &cost.colored {
        let required = cost.colored.iter().filter(|c| *c == color).count() as u8;
        if view.mana_pool.amount(*color) < required {
            need = Some(*color);
            break;
        }
    }
    if need.is_none()
        && view.mana_pool.total() < cost.generic.saturating_add(cost.colored.len() as u8)
    {
        need = Some(Color::Black);
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

fn validate_trace(c: &Captured, seed: u64) -> Result<Value, String> {
    let target = c
        .opponent_target
        .ok_or("opposing Sell-Sword Brute was not cast")?;
    let gasp = c.gasp.ok_or("Last Gasp did not answer Sell-Sword Brute")?;
    if c.target_zone != Some(Zone::Graveyard) {
        return Err(
            "Last Gasp did not put the 2/2 Sell-Sword Brute into its owner's graveyard".to_owned(),
        );
    }
    if c.gasp_zone != Some(Zone::Graveyard) {
        return Err("resolved Last Gasp did not reach its owner's graveyard".to_owned());
    }
    if c.target_payment.len() != 2 || !c.target_payment.contains(&Color::Red) {
        return Err(format!(
            "Sell-Sword Brute did not pay actual {{1}}{{R}}: {:?}",
            c.target_payment
        ));
    }
    if c.gasp_payment.len() != 2 || !c.gasp_payment.contains(&Color::Black) {
        return Err(format!(
            "Last Gasp did not pay actual {{1}}{{B}}: {:?}",
            c.gasp_payment
        ));
    }
    if c.damage_to_controller != Some(2) {
        let receipts: Vec<_> = c.events.iter().filter(|event| matches!(event,
            GameEvent::TriggeredAbilityStacked { source, ability: ABILITY_ID, .. } if *source == target
        ) || matches!(event, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == target)
            || matches!(event, GameEvent::DamageDealtToPlayer { source, .. } if *source == target)
        ).collect();
        return Err(format!(
            "Sell-Sword Brute did not deal exactly two damage to its controller (recorded {:?}; receipts {receipts:?})",
            c.damage_to_controller
        ));
    }
    if c.trigger_source_incarnation.is_none()
        || c.trigger_source_incarnation != c.resolved_source_incarnation
    {
        return Err(
            "Sell-Sword Brute dies trigger changed source incarnation before resolution".to_owned(),
        );
    }
    let target_payment = event_index(&c.events, |e| matches!(e, GameEvent::SpellManaPaid { player: PlayerId(1), card, colors } if *card == target && colors.len() == 2 && colors.contains(&Color::Red)))
        .ok_or("Sell-Sword Brute red mana payment receipt is missing")?;
    let target_cast = event_index_after(
        &c.events,
        target_payment,
        |e| matches!(e, GameEvent::SpellCast { player: PlayerId(1), card } if *card == target),
    )
    .ok_or("Sell-Sword Brute cast receipt is missing")?;
    let target_resolved = event_index_after(
        &c.events,
        target_cast,
        |e| matches!(e, GameEvent::SpellResolved { card } if *card == target),
    )
    .ok_or("Sell-Sword Brute did not resolve")?;
    let target_entered = event_index_after(
        &c.events,
        target_resolved,
        |e| matches!(e, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == target),
    )
    .ok_or("Sell-Sword Brute did not enter the battlefield")?;
    let gasp_payment = event_index(&c.events, |e| matches!(e, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == gasp && colors.len() == 2 && colors.contains(&Color::Black)))
        .ok_or("Last Gasp black payment receipt is missing")?;
    let gasp_cast = event_index_after(
        &c.events,
        gasp_payment,
        |e| matches!(e, GameEvent::SpellCast { player: PlayerId(0), card } if *card == gasp),
    )
    .ok_or("Last Gasp cast receipt is missing")?;
    let gasp_resolved = event_index_after(
        &c.events,
        gasp_cast,
        |e| matches!(e, GameEvent::SpellResolved { card } if *card == gasp),
    )
    .ok_or("Last Gasp did not resolve")?;
    let sba = event_index_after(&c.events, gasp_resolved, |e| matches!(e, GameEvent::StateBasedAction { card, reason: "creature has toughness zero or less" } if *card == target))
        .ok_or("Last Gasp did not cause the Sell-Sword Brute toughness-zero state action")?;
    let target_move = event_index_after(
        &c.events,
        sba,
        |e| matches!(e, GameEvent::CardMoved { card, to: Zone::Graveyard, .. } if *card == target),
    )
    .ok_or("Sell-Sword Brute did not move to its owner's graveyard")?;
    let trigger = event_index_after(&c.events, target_move, |e| matches!(e, GameEvent::TriggeredAbilityStacked { controller: PlayerId(1), source, ability: ABILITY_ID, .. } if *source == target))
        .ok_or("Sell-Sword Brute mandatory dies trigger was not stacked after it died")?;
    let damage = event_index_after(&c.events, trigger, |e| matches!(e, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 2 } if *source == target))
        .ok_or("Sell-Sword Brute did not deal exactly two damage to its controller")?;
    let resolved = event_index_after(&c.events, damage, |e| matches!(e, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == target))
        .ok_or("Sell-Sword Brute dies trigger did not resolve")?;
    if !(target_payment < target_cast
        && target_cast < target_resolved
        && target_resolved < target_entered
        && target_entered < gasp_payment
        && gasp_payment < gasp_cast
        && gasp_cast < gasp_resolved
        && gasp_resolved < sba
        && sba < target_move
        && target_move < trigger
        && trigger < damage
        && damage < resolved)
    {
        return Err(
            "Sell-Sword Brute cast, death, trigger, or damage event order differs".to_owned(),
        );
    }
    if c.terminal != "Winner(PlayerId(0))"
        || c.winner != Some(0)
        || c.life_totals[0] <= 0
        || c.life_totals[1] > 0
        || c.rejected_moves != 0
    {
        return Err(format!(
            "Sell-Sword Brute PvP game did not reach a clean seat-zero terminal win: {:?}",
            c.life_totals
        ));
    }
    let finisher = c
        .pilot_watchwolf
        .ok_or("pilot Watchwolf was not cast as the terminal win condition")?;
    let attack = event_index(&c.events, |e| matches!(e, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&finisher)))
        .ok_or("pilot Watchwolf did not attack")?;
    let combat_damage = event_index_after(&c.events, attack, |e| matches!(e, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 3 } if *source == finisher))
        .ok_or("pilot Watchwolf did not deal combat damage")?;
    let ended = event_index_after(&c.events, combat_damage, |e| {
        matches!(
            e,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal PvP game event is missing")?;
    let projected = project_events(&c.events);
    let event_values: Vec<Value> = projected.iter().map(|e| json!({"sequence": e.sequence, "turn": e.turn, "kind": &e.kind, "canonical": &e.canonical, "facts": &e.facts})).collect();
    Ok(json!({
        "id":"rav-sell-sword-brute-dies-deals-two-to-controller", "passed":true, "seed":seed,
        "decks":[DECK_P0,DECK_P1], "terminal":c.terminal, "winner_seat":c.winner,
        "turns":c.turns, "accepted_moves":c.accepted_moves, "rejected_moves":c.rejected_moves,
        "sell_sword_brute_object":target.0, "last_gasp_object":gasp.0, "win_condition_object":finisher.0,
        "sell_sword_brute_cast_payment":{"colors":c.target_payment.iter().map(|x|format!("{x:?}")).collect::<Vec<_>>(),"red_paid":c.target_payment.contains(&Color::Red)},
        "last_gasp_cast_payment":{"colors":c.gasp_payment.iter().map(|x|format!("{x:?}")).collect::<Vec<_>>(),"black_paid":c.gasp_payment.contains(&Color::Black)},
        "dies_trigger":{"ability":ABILITY_ID,"source_incarnation":c.trigger_source_incarnation,"resolved_incarnation":c.resolved_source_incarnation,"damage_to_controller":c.damage_to_controller,"source_zone_before_trigger":"Graveyard"},
        "sell_sword_brute_zone":format!("{:?}",c.target_zone), "life_at_match_end":c.life_totals,
        "pilot_attack_turns":c.pilot_attack_turns,
        "ordered_event_sequence":[target_payment,target_cast,target_resolved,target_entered,gasp_payment,gasp_cast,gasp_resolved,sba,target_move,trigger,damage,resolved,attack,combat_damage,ended],
        "events":event_values
    }))
}

fn event_index(events: &[GameEvent], mut pred: impl FnMut(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(|e| pred(e))
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
        .find_map(|(i, e)| pred(e).then_some(i))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|e| format!("could not read Sell-Sword Brute candidate source: {e}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Sell-Sword Brute source is not valid declarative Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|a| a.path().is_ident("doc") && matches!(a.meta, Meta::NameValue(_)))
    {
        return Err("Sell-Sword Brute module has an unsupported module attribute".to_owned());
    }
    let mut imports = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item) => {
                if !matches!(item.vis, Visibility::Inherited) || !item.attrs.is_empty() {
                    return Err("Sell-Sword Brute imports must be private".to_owned());
                }
                imports.extend(
                    flattened_use_paths(item).ok_or("Sell-Sword Brute import shape differs")?,
                );
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !matches!(name.as_str(), "definition" | "triggered_ability")
                    || !matches!(&function.vis,Visibility::Restricted(v)if v.path.is_ident("crate"))
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || !function.sig.inputs.is_empty()
                    || !function.sig.generics.params.is_empty()
                    || function.sig.variadic.is_some()
                    || function.block.stmts.len() != 1
                    || !function.attrs.iter().all(|a| {
                        (a.path().is_ident("doc") && matches!(a.meta, Meta::NameValue(_)))
                            || a.path().is_ident("must_use")
                    })
                {
                    return Err(
                        "Sell-Sword Brute functions must be safe no-argument direct literals"
                            .to_owned(),
                    );
                }
                let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
                    return Err(
                        "Sell-Sword Brute definition and trigger must be direct literals"
                            .to_owned(),
                    );
                };
                if value.rest.is_some() || !declarative_expr(&Expr::Struct(value.clone())) {
                    return Err(
                        "Sell-Sword Brute source contains an unsupported executable expression"
                            .to_owned(),
                    );
                }
                if !functions.insert(name) {
                    return Err("Sell-Sword Brute has a duplicate candidate function".to_owned());
                }
            }
            _ => {
                return Err(
                    "Sell-Sword Brute module permits only imports and two candidate functions"
                        .to_owned(),
                );
            }
        }
    }
    let expected: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
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
    .map(|p| p.into_iter().map(str::to_owned).collect())
    .collect();
    if functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()])
        || imports != expected
    {
        return Err(
            "Sell-Sword Brute source imports or candidate function count differ".to_owned(),
        );
    }
    let card = candidate_sell_sword_brute::definition();
    if card.id != CARD_ID
        || card.name != "Sell-Sword Brute"
        || card.set_code != "RAV"
        || card.mana_cost != ManaCost::with_colors(1, [Color::Red])
        || card.colors != BTreeSet::from([Color::Red])
        || card.card_types != BTreeSet::from([CardType::Creature])
        || card.power != Some(2)
        || card.toughness != Some(2)
        || card.is_basic_land
        || !card.mana_colors.is_empty()
        || !card.keywords.is_empty()
    {
        return Err(
            "Sell-Sword Brute identity, {1}{R} cost, or 2/2 red creature characteristics differ"
                .to_owned(),
        );
    }
    let ability = candidate_sell_sword_brute::triggered_ability();
    if ability.card_definition != CARD_ID
        || ability.ability.id != ABILITY_ID
        || ability.ability.condition != cardbench_magic_engine::TriggerCondition::Dies
        || ability.ability.mana_cost != ManaCost::new(0)
        || ability.ability.optional
        || !ability.ability.targets.is_empty()
    {
        return Err(
            "Sell-Sword Brute must retain a mandatory, untargeted, zero-cost dies trigger"
                .to_owned(),
        );
    }
    let runtime_definition = card_definitions()
        .into_iter()
        .find(|entry| entry.id == CARD_ID)
        .ok_or("Sell-Sword Brute definition is not registered")?;
    if runtime_definition != card {
        return Err(
            "Sell-Sword Brute candidate definition differs from the runtime registry".to_owned(),
        );
    }
    let runtime_trigger = rav_triggered_ability_bindings()
        .into_iter()
        .find(|entry| entry.card_definition == CARD_ID && entry.ability.id == ABILITY_ID)
        .ok_or("Sell-Sword Brute candidate dies binding is not registered")?;
    if runtime_trigger != ability {
        return Err(
            "Sell-Sword Brute candidate trigger differs from the runtime registry".to_owned(),
        );
    }
    Ok(())
}
fn declarative_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(_) | Expr::Path(_) => true,
        Expr::Unary(u) => matches!(u.op, syn::UnOp::Neg(_)) && matches!(&*u.expr, Expr::Lit(_)),
        Expr::Reference(r) => declarative_expr(&r.expr),
        Expr::Array(a) => a.elems.iter().all(declarative_expr),
        Expr::Struct(s) => {
            s.attrs.is_empty()
                && s.fields
                    .iter()
                    .all(|f| matches!(f.member, syn::Member::Named(_)) && declarative_expr(&f.expr))
                && s.rest.as_deref().is_none_or(declarative_expr)
        }
        Expr::Call(c) => {
            let allowed = match &*c.func {
                Expr::Path(p) if p.qself.is_none() => {
                    let names: Vec<_> = p
                        .path
                        .segments
                        .iter()
                        .map(|s| s.ident.to_string())
                        .collect();
                    matches!(names.as_slice(),[a,b]if(a=="BTreeSet"&&(b=="from"||b=="new"))||(a=="ManaCost"&&(b=="new"||b=="with_colors")))
                        || matches!(names.as_slice(),[one]if one=="Some")
                }
                _ => false,
            };
            allowed && c.args.iter().all(declarative_expr)
        }
        Expr::Macro(m) => {
            let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
            m.mac.path.is_ident("vec")
                && parser
                    .parse2(m.mac.tokens.clone())
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
        UseTree::Path(p) => {
            let mut next = prefix.to_vec();
            next.push(p.ident.to_string());
            flatten_use_tree(&next, &p.tree, paths)
        }
        UseTree::Name(n) => {
            let mut path = prefix.to_vec();
            path.push(n.ident.to_string());
            paths.push(path);
            true
        }
        UseTree::Group(g) => g
            .items
            .iter()
            .all(|item| flatten_use_tree(prefix, item, paths)),
        _ => false,
    }
}
