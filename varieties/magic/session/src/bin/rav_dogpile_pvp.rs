//! Full-game RAV Dogpile authority for its attacking-creature-count damage.

use cardbench_magic_engine::{
    CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, ManaCost,
    ManaPaymentSelection, PlayerId, PolicyAction, Step, Target, TargetRequirement,
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
const CARD_ID: &str = "RAV-DOGPILE";
const CREATURE_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/dogpile.rs";
const DECK_P0: &str = "rav_dogpile_watchwolf_wide";
const DECK_P1: &str = "rav_basic_plains_opponent";

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/dogpile.rs"]
mod candidate_dogpile;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Dogpile PvP result: {error}");
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
            eprintln!("Dogpile PvP authority failed: {error}");
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
        return Err("Dogpile full-game replay was not deterministic".to_owned());
    }
    let mutations = mutation_sensitivity(seed)?;
    for key in [
        "damage_effect_removal_detected_in_game",
        "creature_only_target_mutation_detected_in_game",
        "wrong_cast_cost_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!("Dogpile mutation escaped the PvP trace: {key}"));
        }
    }
    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1","task_id":TASK_ID,"expansion":"RAV",
        "card":{"collector_number":120,"definition_id":CARD_ID,"name":"Dogpile"},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},"candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":["105.2 and 202.1-202.2: red spell color and payment","106.1 and 106.4: mana production and spell payment","117: priority and resolution timing","601.2: casting a targeted instant","608.2: target legality and effect resolution","508.1: declaring attackers","510: attacking-creature state and combat damage"],
            "source_contract":"RAV Dogpile #120, {3}{R} red instant; it deals damage to any target equal to the number of attacking creatures as it resolves",
            "authority_cutoff":"2026-09-24","no_provider_trial":true
        },
        "passed":true,"pilot_card_passed":true,"replay_deterministic":true,"cards_passed":cards_passed,
        "cards_credited":0,"cards_total":catalog.len(),"printings_total":RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete":false,"full_reward":false,"harbor_reward":0.0,"pilot_reward":0.0,
        "qualification_status":"candidate_pilot_pass; independent reviewer approval remains",
        "mutation_sensitivity":mutations,"traces":[trace],"per_card_breakdown":breakdown
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
    RemoveEffect,
    CreatureOnlyTarget,
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
    dogpile: Option<cardbench_magic_engine::ObjectId>,
    cast_payment: Vec<Color>,
    attacking_count: usize,
    damage_amount: Option<i32>,
    dogpile_damage_events: usize,
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
    Err("no bounded seed produced a full-game Dogpile combat-count trace".to_owned())
}
fn mutation_sensitivity(seed: u64) -> Result<Value, String> {
    let mut outcomes = serde_json::Map::new();
    for (key, mutation) in [
        (
            "damage_effect_removal_detected_in_game",
            Mutation::RemoveEffect,
        ),
        (
            "creature_only_target_mutation_detected_in_game",
            Mutation::CreatureOnlyTarget,
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
                count: 20,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 20,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 12,
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
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{name} deck has {count} cards; expected 60"));
        }
    }
    let mut definitions = card_definitions();
    let dogpile_definition = definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Dogpile definition is absent from RAV registry")?;
    *dogpile_definition = candidate_dogpile::definition();
    if mutation == Mutation::RemoveEffect {
        dogpile_definition.effects.clear();
    }
    if mutation == Mutation::CreatureOnlyTarget {
        dogpile_definition.effects = vec![Effect::DealDamageEqualToAttackingCreatures {
            target: TargetRequirement::Creature,
        }];
    }
    if mutation == Mutation::WrongCastCost {
        dogpile_definition.mana_cost = ManaCost::with_colors(2, [Color::Red]);
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
        Box::new(DogpilePolicy::new(PlayerId(0))),
        Box::new(DogpilePolicy::new(PlayerId(1))),
    ];
    let (mut accepted, mut attempted) = (0u32, 0u32);
    while !game.is_game_over() {
        if game.turn > config.max_turns.min(60) || accepted >= config.max_policy_moves {
            return Err("Dogpile match exceeded its bounded turn or move limit".to_owned());
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
        let policy_id = policy.id();
        game.submit_policy_move(player, policy_id, action)
            .map_err(|e| {
                format!(
                    "policy move rejected for seat {} on turn {} at {:?}: {e}",
                    player.0, view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|e| format!("post-move invariant failure: {e}"))?;
    }
    let events = game.event_log.clone();
    let dogpile = events.iter().find_map(|event| match event {
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
    let cast_payment = dogpile
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
    let damage_event = dogpile.and_then(|source| {
        events.iter().find_map(|event| match event {
            GameEvent::DamageDealtToPlayer {
                source: damage_source,
                player: PlayerId(1),
                amount,
            } if *damage_source == source => Some(*amount),
            _ => None,
        })
    });
    let dogpile_damage_events = dogpile.map_or(0, |source| events.iter().filter(|event| matches!(event, GameEvent::DamageDealtToPlayer { source: damage_source, player: PlayerId(1), .. } if *damage_source == source)).count());
    let cast_index = dogpile.and_then(|source| {
        events.iter().position(|event| {
            matches!(event,
                GameEvent::SpellCast { player: PlayerId(0), card } if *card == source
            )
        })
    });
    let attacking_count = cast_index
        .and_then(|cast_index| {
            events[..cast_index]
                .iter()
                .rev()
                .find_map(|event| match event {
                    GameEvent::AttackersDeclared {
                        player: PlayerId(0),
                        attackers,
                    } if attackers.len() >= 2 => Some(attackers.len()),
                    _ => None,
                })
        })
        .unwrap_or(0);
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
        dogpile,
        cast_payment,
        attacking_count,
        damage_amount: damage_event,
        dogpile_damage_events,
        life_totals: [
            game.player(PlayerId(0)).map_err(|e| e.to_string())?.life,
            game.player(PlayerId(1)).map_err(|e| e.to_string())?.life,
        ],
    })
}

struct DogpilePolicy {
    player: PlayerId,
    cast: bool,
}
impl DogpilePolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            cast: false,
        }
    }
}
impl CodePolicy for DogpilePolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.dogpile-wide-attack.v1"
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
        if view.active_player != self.player {
            return PolicyAction::PassPriority;
        }
        if self.player == PlayerId(1) {
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
            let creatures = view
                .own_battlefield
                .iter()
                .filter(|card| card.definition == Some(CREATURE_ID))
                .count();
            if creatures < 2 {
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
            }
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.attackers_declared
            && !self.cast
            && view.stack_depth == 0
        {
            let attackers = view.combat_attackers.len();
            if attackers >= 2
                && let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
            {
                let cost = ManaCost::with_colors(3, [Color::Red]);
                if let Some(selection) = payment(view, &cost) {
                    self.cast = true;
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![Target::Player(PlayerId(1))],
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
        need = Some(Color::Red);
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
        && a.dogpile == b.dogpile
        && a.cast_payment == b.cast_payment
        && a.attacking_count == b.attacking_count
        && a.damage_amount == b.damage_amount
        && a.dogpile_damage_events == b.dogpile_damage_events
}
fn validate_trace(c: &Captured, seed: u64) -> Result<Value, String> {
    let dogpile = c.dogpile.ok_or("Dogpile was not cast")?;
    if c.cast_payment.len() != 4 || !c.cast_payment.contains(&Color::Red) {
        return Err(format!(
            "Dogpile did not pay {{3}}{{R}}: {:?}",
            c.cast_payment
        ));
    }
    if c.attacking_count < 2
        || c.damage_amount != Some(c.attacking_count as i32)
        || c.dogpile_damage_events != 1
    {
        return Err(format!(
            "Dogpile damage did not equal the {n} attackers present at cast",
            n = c.attacking_count
        ));
    }
    if c.terminal != "Winner(PlayerId(0))"
        || c.winner != Some(0)
        || c.life_totals[0] <= 0
        || c.life_totals[1] > 0
        || c.rejected_moves != 0
    {
        return Err(format!(
            "Dogpile PvP did not reach a clean Player 0 win: {:?}",
            c.life_totals
        ));
    }
    let attack = event_index(
        &c.events,
        |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.len() == c.attacking_count),
    );
    let attack = attack.ok_or("Dogpile combat attackers receipt missing")?;
    let paid = event_index_after(&c.events, attack, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == dogpile && colors.len() == 4 && colors.contains(&Color::Red))).ok_or("Dogpile exact {3}{R} payment receipt missing")?;
    let cast = event_index_after(&c.events, paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == dogpile)).ok_or("Dogpile cast receipt missing")?;
    let damage = event_index_after(&c.events, cast, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount } if *source == dogpile && *amount == c.attacking_count as i32)).ok_or("Dogpile did not deal its attacking-creature count in damage")?;
    let resolved = event_index_after(
        &c.events,
        damage,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == dogpile),
    )
    .ok_or("Dogpile spell did not resolve")?;
    let ended = event_index_after(&c.events, resolved, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal Player 0 win receipt missing")?;
    if !(attack < paid && paid < cast && cast < damage && damage < resolved && resolved < ended) {
        return Err(
            "Dogpile attack, payment, cast, dynamic damage, and terminal event order is invalid"
                .to_owned(),
        );
    }
    let projected = project_events(&c.events);
    let events: Vec<Value> = projected.iter().map(|event| json!({"sequence":event.sequence,"turn":event.turn,"kind":&event.kind,"canonical":&event.canonical,"facts":&event.facts})).collect();
    Ok(
        json!({"id":"rav-dogpile-attacking-creature-count-full-game","passed":true,"shuffle_seed":seed,"decks":[DECK_P0,DECK_P1],"terminal":c.terminal,"winner_seat":c.winner,"turns":c.turns,"accepted_moves":c.accepted_moves,"rejected_moves":c.rejected_moves,
        "dogpile":{"object":dogpile.0,"mana_cost":"{3}{R}","cast_mana_paid":c.cast_payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>()},
        "target":{"kind":"player","player":"PlayerId(1)"},"combat":{"attacking_creature_count_at_resolution":c.attacking_count,"damage_amount":c.damage_amount,"dogpile_damage_events":c.dogpile_damage_events,"life_at_terminal":c.life_totals},
        "ordered_event_sequence":[attack,paid,cast,damage,resolved,ended],"events":events}),
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
    let card = candidate_dogpile::definition();
    if card.id != CARD_ID
        || card.name != "Dogpile"
        || card.set_code != "RAV"
        || card.mana_cost != ManaCost::with_colors(3, [Color::Red])
        || card.colors != BTreeSet::from([Color::Red])
        || card.card_types != BTreeSet::from([CardType::Instant])
        || card.is_basic_land
        || !card.mana_colors.is_empty()
        || card.power.is_some()
        || card.toughness.is_some()
        || !card.keywords.is_empty()
        || card.effects
            != [Effect::DealDamageEqualToAttackingCreatures {
                target: TargetRequirement::PlayerOrCreature,
            }]
    {
        return Err("Dogpile must be a red {3}{R} instant targeting a player or creature for attacking-creature-count damage".to_owned());
    }
    Ok(())
}
fn validate_registry_delegation() -> Result<(), String> {
    let candidate = candidate_dogpile::definition();
    if card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .as_ref()
        != Some(&candidate)
    {
        return Err(
            "Dogpile registry definition does not delegate to its candidate module".to_owned(),
        );
    }
    Ok(())
}
fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|e| format!("could not read Dogpile source: {e}"))?;
    let file =
        syn::parse_file(&source).map_err(|_| "Dogpile source is not valid declarative Rust")?;
    if !file
        .attrs
        .iter()
        .all(|attr| attr.path().is_ident("doc") && matches!(attr.meta, Meta::NameValue(_)))
    {
        return Err("Dogpile module has unsupported module attributes".to_owned());
    }
    let expected: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TargetRequirement"],
    ]
    .into_iter()
    .map(|p| p.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut found = false;
    for item in &file.items {
        match item {
            Item::Use(item)
                if matches!(item.vis, Visibility::Inherited) && item.attrs.is_empty() =>
            {
                imports.extend(flattened_use_paths(item).ok_or("Dogpile import structure differs")?)
            }
            Item::Fn(function) if !found => {
                found = true;
                if function.sig.ident != "definition"
                    || !matches!(&function.vis,Visibility::Restricted(vis) if vis.path.is_ident("crate"))
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
                        "Dogpile may expose only a safe no-argument definition()".to_owned()
                    );
                }
                let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
                    return Err("Dogpile definition must return a direct struct literal".to_owned());
                };
                if value.rest.is_some() || !declarative_expr(&Expr::Struct(value.clone())) {
                    return Err(
                        "Dogpile source contains an unsupported executable expression".to_owned(),
                    );
                }
            }
            _ => return Err("Dogpile source permits only imports and definition()".to_owned()),
        }
    }
    if !found || imports != expected {
        return Err("Dogpile source imports or candidate function set differs".to_owned());
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
                    matches!(names.as_slice(),[a,b] if (a=="BTreeSet"&&(b=="from"||b=="new"))||(a=="ManaCost"&&(b=="new"||b=="with_colors")))
                        || matches!(names.as_slice(),[name] if name=="Some")
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
