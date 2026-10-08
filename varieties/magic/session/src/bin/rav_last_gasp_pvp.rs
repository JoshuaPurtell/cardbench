//! Full-game RAV Last Gasp authority for its targeted -3/-3 effect.

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
    rav_mana_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::parse::Parser;
use syn::{Expr, Item, Meta, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-LAST-GASP";
const TARGET_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/last_gasp.rs";
const DECK_P0: &str = "rav_last_gasp_black_white_green";
const DECK_P1: &str = "rav_watchwolf_green_white";

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/last_gasp.rs"]
mod candidate_last_gasp;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Last Gasp PvP result: {error}");
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
            eprintln!("Last Gasp PvP authority failed: {error}");
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
    {
        return Err("Last Gasp full-game replay was not deterministic".to_owned());
    }
    let mutation_detected = mutation_reaches_trace_and_fails(seed)?;
    if !mutation_detected {
        return Err(
            "changing Last Gasp's targeted toughness modifier escaped its PvP trace".to_owned(),
        );
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
        "card":{"collector_number":93,"definition_id":CARD_ID,"name":"Last Gasp"},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":[
                "105.2 and 202.1-202.2: black spell color and colored payment",
                "106.1 and 106.4: mana production and spell payment",
                "117: priority and instant-speed response windows",
                "109.5 and 115: target identity and legal creature target",
                "601.2 and 608.2b-608.3a: casting, target legality, and spell resolution",
                "613.4: power/toughness modification layer",
                "704.5f: creature with toughness zero or less is put into its owner's graveyard",
                "510: combat damage and terminal game state"
            ],
            "source_contract":"RAV Last Gasp identity, {1}{B} instant cost, and candidate-owned targeted -3/-3 until-end-of-turn effect",
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
        "mutation_sensitivity":{"wrong_toughness_modifier_mutation_detected":mutation_detected,"sealed_mutation_authority":false},
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
    MinusTwo,
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
    Err("no bounded seed produced a full-game Last Gasp removal trace".to_owned())
}

fn mutation_reaches_trace_and_fails(seed: u64) -> Result<bool, String> {
    let captured = run_match(seed, Mutation::MinusTwo)?;
    Ok(validate_trace(&captured, seed).err().is_some_and(|error| {
        error.contains(
            "Last Gasp did not remove the 3/3 Watchwolf through the toughness-based state action",
        )
    }))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: TARGET_ID.to_owned(),
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
                count: 1,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 30,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 29,
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
    let mut submitted = candidate_last_gasp::definition();
    if mutation == Mutation::MinusTwo {
        submitted.effects = vec![Effect::ModifyTargetPtUntilEndOfTurn {
            power: -2,
            toughness: -2,
        }];
    }
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Last Gasp definition is absent from RAV registry")? = submitted;
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
        Box::new(PilotPolicy::new(PlayerId(0))),
        Box::new(PilotPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0u32;
    let mut attempted = 0u32;
    let mut attack_turns = Vec::new();
    while !game.is_game_over() {
        if game.turn > config.max_turns.min(40) || accepted >= config.max_policy_moves {
            return Err("Last Gasp match exceeded the bounded turn or move limit".to_owned());
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
        if game.event_log[cursor..].iter().any(|event|matches!(event,GameEvent::AttackersDeclared{player:PlayerId(0),attackers} if attackers.iter().any(|id|game.card_definition(*id).is_ok_and(|card|card.id==TARGET_ID)))) { attack_turns.push(game.turn); }
    }
    let events = game.event_log.clone();
    let gasp = events.iter().find_map(|event| match event {
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
            .is_ok_and(|def| def.id == TARGET_ID) =>
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
    })
}

struct PilotPolicy {
    player: PlayerId,
}
impl PilotPolicy {
    fn new(player: PlayerId) -> Self {
        Self { player }
    }
}
impl CodePolicy for PilotPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.last-gasp-removal-and-win.v1"
        } else {
            "rav.watchwolf-target-policy.v1"
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
                    .filter(|card| card.definition == Some(TARGET_ID) && card.can_attack)
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
                    .find(|card| card.definition == Some(CARD_ID)),
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
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(TARGET_ID))
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
    let gasp = c.gasp.ok_or("Last Gasp was not cast")?;
    let target = c.opponent_target.ok_or("opposing Watchwolf was not cast")?;
    if c.target_zone != Some(Zone::Graveyard) {
        return Err(
            "Last Gasp did not remove the 3/3 Watchwolf through the toughness-based state action"
                .to_owned(),
        );
    }
    if c.gasp_zone != Some(Zone::Graveyard) {
        return Err("resolved Last Gasp did not reach its owner's graveyard".to_owned());
    }
    if c.gasp_payment.len() != 2 || !c.gasp_payment.contains(&Color::Black) {
        return Err(format!(
            "Last Gasp did not pay actual {{1}}{{B}} including black: {:?}",
            c.gasp_payment
        ));
    }
    if c.target_payment.len() != 2
        || !c.target_payment.contains(&Color::Green)
        || !c.target_payment.contains(&Color::White)
    {
        return Err(format!(
            "opposing Watchwolf did not pay actual {{G}}{{W}}: {:?}",
            c.target_payment
        ));
    }
    let sba=event_index(&c.events,|e|matches!(e,GameEvent::StateBasedAction{card,reason:"creature has toughness zero or less"} if *card==target)).ok_or("Last Gasp did not cause the toughness-zero state-based action")?;
    let target_move = event_index_after(
        &c.events,
        sba,
        |e| matches!(e,GameEvent::CardMoved{card,to:Zone::Graveyard,..} if *card==target),
    )
    .ok_or("Last Gasp's 3/3 target did not move to its owner's graveyard")?;
    let target_cast_payment=event_index(&c.events,|e|matches!(e,GameEvent::SpellManaPaid{player:PlayerId(1),card,colors} if *card==target&&colors.contains(&Color::Green)&&colors.contains(&Color::White))).ok_or("opposing Watchwolf colored payment receipt is missing")?;
    let target_cast = event_index_after(
        &c.events,
        target_cast_payment,
        |e| matches!(e,GameEvent::SpellCast{player:PlayerId(1),card} if *card==target),
    )
    .ok_or("opposing Watchwolf cast receipt is missing")?;
    let target_resolved = event_index_after(
        &c.events,
        target_cast,
        |e| matches!(e,GameEvent::SpellResolved{card} if *card==target),
    )
    .ok_or("opposing Watchwolf did not resolve")?;
    let target_entered = event_index_after(
        &c.events,
        target_resolved,
        |e| matches!(e,GameEvent::CardMoved{card,to:Zone::Battlefield,..} if *card==target),
    )
    .ok_or("opposing Watchwolf did not enter the battlefield")?;
    let gasp_payment=event_index(&c.events,|e|matches!(e,GameEvent::SpellManaPaid{player:PlayerId(0),card,colors} if *card==gasp&&colors.contains(&Color::Black)&&colors.len()==2)).ok_or("Last Gasp's black payment receipt is missing")?;
    let gasp_cast = event_index_after(
        &c.events,
        gasp_payment,
        |e| matches!(e,GameEvent::SpellCast{player:PlayerId(0),card} if *card==gasp),
    )
    .ok_or("Last Gasp cast receipt is missing")?;
    let gasp_resolved = event_index_after(
        &c.events,
        gasp_cast,
        |e| matches!(e,GameEvent::SpellResolved{card} if *card==gasp),
    )
    .ok_or("Last Gasp did not resolve")?;
    if !(target_cast < target_resolved
        && target_resolved < target_entered
        && target_entered < gasp_payment
        && gasp_payment < gasp_cast
        && gasp_cast < gasp_resolved
        && gasp_resolved < sba
        && sba < target_move)
    {
        return Err("Last Gasp cast/target/resolve/state-action event order is invalid".to_owned());
    }
    if c.terminal != "Winner(PlayerId(0))"
        || c.winner != Some(0)
        || c.life_totals[0] <= 0
        || c.life_totals[1] > 0
        || c.rejected_moves != 0
    {
        return Err(format!(
            "Last Gasp PvP game did not reach a clean seat-zero terminal win: {:?}",
            c.life_totals
        ));
    }
    let pilot_watchwolf = c
        .pilot_watchwolf
        .ok_or("seat zero Watchwolf was not cast as the terminal win condition")?;
    let attack=event_index(&c.events,|e|matches!(e,GameEvent::AttackersDeclared{player:PlayerId(0),attackers} if attackers.contains(&pilot_watchwolf))).ok_or("seat-zero Watchwolf did not attack")?;
    let damage=event_index_after(&c.events,attack,|e|matches!(e,GameEvent::DamageDealtToPlayer{source,player:PlayerId(1),amount:3} if *source==pilot_watchwolf)).ok_or("seat-zero Watchwolf did not deal combat damage")?;
    let ended = event_index_after(&c.events, damage, |e| {
        matches!(
            e,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal game event is missing")?;
    let projected = project_events(&c.events);
    let event_values:Vec<Value>=projected.iter().map(|e|json!({"sequence":e.sequence,"turn":e.turn,"kind":&e.kind,"canonical":&e.canonical})).collect();
    Ok(
        json!({"id":"rav-last-gasp-targeted-minus-three-minus-three","passed":true,"seed":seed,"decks":[DECK_P0,DECK_P1],"terminal":c.terminal,"winner_seat":c.winner,"turns":c.turns,"accepted_moves":c.accepted_moves,"rejected_moves":c.rejected_moves,"last_gasp_object":gasp.0,"opponent_watchwolf_object":target.0,"pilot_watchwolf_object":pilot_watchwolf.0,"last_gasp_cast_payment":{"colors":c.gasp_payment.iter().map(|x|format!("{x:?}")).collect::<Vec<_>>(),"black_paid":c.gasp_payment.contains(&Color::Black)},"opposing_watchwolf_cast_payment":{"colors":c.target_payment.iter().map(|x|format!("{x:?}")).collect::<Vec<_>>(),"green_paid":c.target_payment.contains(&Color::Green),"white_paid":c.target_payment.contains(&Color::White)},"removed_target_zone":format!("{:?}",c.target_zone),"last_gasp_zone":format!("{:?}",c.gasp_zone),"life_at_match_end":c.life_totals,"pilot_attack_turns":c.pilot_attack_turns,"ordered_event_sequence":[target_cast_payment,target_cast,target_resolved,target_entered,gasp_payment,gasp_cast,gasp_resolved,sba,target_move,attack,damage,ended],"events":event_values}),
    )
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
        .map_err(|e| format!("could not read Last Gasp candidate source: {e}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Last Gasp source is not valid declarative Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|a| a.path().is_ident("doc") && matches!(a.meta, Meta::NameValue(_)))
    {
        return Err("Last Gasp module has an unsupported module attribute".to_owned());
    }
    let mut imports = BTreeSet::new();
    let mut definitions = 0;
    for item in &file.items {
        match item {
            Item::Use(item) => {
                if !matches!(item.vis, Visibility::Inherited) || !item.attrs.is_empty() {
                    return Err("Last Gasp imports must be private".to_owned());
                }
                imports.extend(flattened_use_paths(item).ok_or("Last Gasp import shape differs")?);
            }
            Item::Fn(function) => {
                if function.sig.ident != "definition"
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
                        "Last Gasp source must expose one safe no-argument definition function"
                            .to_owned(),
                    );
                }
                let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
                    return Err("Last Gasp definition must be a direct card literal".to_owned());
                };
                if value.rest.is_some() || !declarative_expr(&Expr::Struct(value.clone())) {
                    return Err(
                        "Last Gasp source contains an unsupported executable expression".to_owned(),
                    );
                }
                definitions += 1;
            }
            _ => {
                return Err(
                    "Last Gasp module permits only imports and the definition function".to_owned(),
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
    ]
    .into_iter()
    .map(|p| p.into_iter().map(str::to_owned).collect())
    .collect();
    if definitions != 1 || imports != expected {
        return Err("Last Gasp source imports or definition count differ".to_owned());
    }
    let card = candidate_last_gasp::definition();
    if card.id != CARD_ID
        || card.name != "Last Gasp"
        || card.set_code != "RAV"
        || card.mana_cost != ManaCost::with_colors(1, [Color::Black])
        || card.colors != BTreeSet::from([Color::Black])
        || card.card_types != BTreeSet::from([CardType::Instant])
        || card.power.is_some()
        || card.toughness.is_some()
        || card.is_basic_land
        || !card.mana_colors.is_empty()
        || !card.keywords.is_empty()
    {
        return Err(
            "Last Gasp identity, {1}{B} cost, or instant characteristics differ".to_owned(),
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
