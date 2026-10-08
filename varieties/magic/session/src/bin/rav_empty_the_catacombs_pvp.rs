//! Full-game PvP authority for RAV Empty the Catacombs's symmetric creature return.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    CardType, CastRequest, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Zone,
};
use cardbench_magic_policies::CodePolicy;
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
use syn::{Expr, Item, Lit, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-EMPTY-THE-CATACOMBS";
const CARD_NAME: &str = "Empty the Catacombs";
const RAIN_ID: &str = "RAV-RAIN-OF-EMBERS";
const RECRUIT_ID: &str = "RAV-CAREGIVER";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/empty_the_catacombs.rs";
const MAX_TRACE_SEED: u64 = 128;
const MAX_TURNS: u32 = 32;
const MAX_MOVES: u32 = 8_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/empty_the_catacombs.rs"]
mod candidate_empty_the_catacombs;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Empty the Catacombs result: {error}");
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
            eprintln!("Empty the Catacombs PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_registry_delegation()?;
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
        return Err("RAV catalog denominator differs from its source contract".to_owned());
    }
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if !same_match(&captured, &replay) {
        return Err("Empty the Catacombs shuffled-game replay was not deterministic".to_owned());
    }
    let mutations = mutation_sensitivity(seed)?;
    for key in [
        "wrong-cost-mutation-detected-in-game",
        "wrong-color-mutation-detected-in-game",
        "missing-all-creature-return-mutation-detected-in-game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Empty the Catacombs semantic mutation escaped PvP trace: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,
        "expansion":"RAV",
        "card":{"collector_number":86,"printings":1,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":[
                "105.2 and 202.1-202.2: black color and mana cost",
                "117: sorcery-speed spell timing",
                "400.7: zone changes create new game objects",
                "601.2: casting Empty the Catacombs",
                "608.2: both graveyards are processed before the sorcery enters its owner's graveyard",
                "903.9: optional commander-zone replacement for a commander returning to hand"
            ],
            "source_contract":"RAV Empty the Catacombs #86, {3}{B} sorcery: each player returns all creature cards from their own graveyard to their own hand",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true
        },
        "passed":true,
        "pilot_card_passed":true,
        "replay_deterministic":true,
        "candidate_source_validation":{"direct_literal_ast_checked_before_candidate_calls":true,"guard_ran_before_candidate_function_calls":true},
        "cards_passed":cards_passed,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":printings.len(),
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":"unreviewed_diagnostic",
        "mutation_sensitivity":mutations,
        "traces":[trace],
        "per_card_breakdown":breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongCost,
    WrongColor,
    MissingEffect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    catacombs: Option<ObjectId>,
    rain_card: Option<ObjectId>,
    creature_cards_died: [Vec<ObjectId>; 2],
    graveyards_at_cast: [Vec<ObjectId>; 2],
    hand_moves_after_cast: Vec<ObjectId>,
    creature_cards_still_in_graveyard: [Vec<ObjectId>; 2],
    catacombs_resolved: bool,
    catacombs_in_owners_graveyard_at_end: bool,
    spell_payment: Vec<Color>,
    spell_cost: Option<ManaCost>,
    spell_colors: Vec<Color>,
    attacks: Vec<u32>,
    combat_damage_to_opponent: i32,
    life: [i64; 2],
}

fn same_match(left: &Captured, right: &Captured) -> bool {
    left.events == right.events
        && left.terminal == right.terminal
        && left.winner == right.winner
        && left.turns == right.turns
        && left.catacombs == right.catacombs
        && left.rain_card == right.rain_card
        && left.creature_cards_died == right.creature_cards_died
        && left.graveyards_at_cast == right.graveyards_at_cast
        && left.hand_moves_after_cast == right.hand_moves_after_cast
        && left.creature_cards_still_in_graveyard == right.creature_cards_still_in_graveyard
        && left.catacombs_resolved == right.catacombs_resolved
        && left.catacombs_in_owners_graveyard_at_end == right.catacombs_in_owners_graveyard_at_end
        && left.spell_payment == right.spell_payment
        && left.spell_cost == right.spell_cost
        && left.spell_colors == right.spell_colors
        && left.attacks == right.attacks
        && left.combat_damage_to_opponent == right.combat_damage_to_opponent
        && left.life == right.life
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_EMPTY_THE_CATACOMBS_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_EMPTY_THE_CATACOMBS_SEED must be an unsigned integer")?;
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
        "no bounded seed completed the Empty the Catacombs two-graveyard PvP trace: {last_error}"
    ))
}

fn mutation_sensitivity(seed: u64) -> Result<Value, String> {
    let mut outcomes = serde_json::Map::new();
    for (name, mutation) in [
        ("wrong-cost-mutation-detected-in-game", Mutation::WrongCost),
        (
            "wrong-color-mutation-detected-in-game",
            Mutation::WrongColor,
        ),
        (
            "missing-all-creature-return-mutation-detected-in-game",
            Mutation::MissingEffect,
        ),
    ] {
        let detected = match run_match(seed, mutation) {
            Ok(captured) => validate_trace(&captured, seed).is_err(),
            Err(_) => true,
        };
        outcomes.insert(name.to_owned(), Value::Bool(detected));
    }
    outcomes.insert("sealed-mutation-authority".to_owned(), Value::Bool(false));
    Ok(Value::Object(outcomes))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot = deck(&[
        (CARD_ID, 4),
        (RAIN_ID, 4),
        (RECRUIT_ID, 4),
        (WATCHWOLF_ID, 4),
        ("RAV-SWAMP", 12),
        ("RAV-FOREST", 10),
        ("RAV-PLAINS", 12),
        ("RAV-MOUNTAIN", 8),
        ("RAV-ISLAND", 2),
    ]);
    let opponent = deck(&[
        (RAIN_ID, 4),
        (RECRUIT_ID, 4),
        ("RAV-FOREST", 14),
        ("RAV-PLAINS", 14),
        ("RAV-MOUNTAIN", 12),
        ("RAV-SWAMP", 12),
    ]);
    for (label, deck) in [("pilot", &pilot), ("opponent", &opponent)] {
        let count: u16 = deck
            .mainboard
            .iter()
            .map(|entry| u16::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{label} deck contains {count} cards, not 60"));
        }
    }
    let mut definitions = card_definitions();
    let mut candidate = candidate_empty_the_catacombs::definition();
    match mutation {
        Mutation::None => {}
        Mutation::WrongCost => candidate.mana_cost = ManaCost::with_colors(2, [Color::Black]),
        Mutation::WrongColor => candidate.colors = BTreeSet::from([Color::Red]),
        Mutation::MissingEffect => candidate.effects.clear(),
    }
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Empty the Catacombs is absent from the RAV registry")? = candidate;
    let mut game = Game::new_with_all_bindings(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), 7)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), 7)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut pilot_policy = CatacombsPolicy::new(PlayerId(0));
    let mut opponent_policy = CatacombsPolicy::new(PlayerId(1));
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut catacombs = None;
    let mut rain_card = None;
    let mut creature_cards_died: [Vec<ObjectId>; 2] = [vec![], vec![]];
    let mut graveyards_at_cast: [Vec<ObjectId>; 2] = [vec![], vec![]];
    let mut cast_event_index = None;
    let mut hand_moves_after_cast = Vec::new();
    let mut catacombs_resolved = false;
    let mut spell_payment = Vec::new();
    let mut spell_cost = None;
    let mut spell_colors = Vec::new();
    let mut attacks = Vec::new();
    let mut combat_damage_to_opponent = 0_i32;

    while !game.is_game_over() {
        if game.turn > MAX_TURNS || accepted >= MAX_MOVES {
            return Err(format!(
                "Empty the Catacombs match exceeded bounds at turn {} after {accepted} actions",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let action = if player == PlayerId(0) {
            propose_action(&mut pilot_policy, &view)
        } else {
            propose_action(&mut opponent_policy, &view)
        };
        attempted += 1;
        let cursor = game.event_log.len();
        let policy_id = if player == PlayerId(0) {
            pilot_policy.id()
        } else {
            opponent_policy.id()
        };
        game.submit_policy_move(player, policy_id, action)
            .map_err(|error| {
                format!(
                    "policy move rejected for seat {} at turn {} {:?}: {error}",
                    player.0, view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        for (offset, event) in game.event_log[cursor..].iter().enumerate() {
            let event_index = cursor + offset;
            match event {
                GameEvent::SpellCast {
                    player: controller,
                    card,
                } if *controller == PlayerId(0)
                    && game
                        .card_definition(*card)
                        .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    catacombs = Some(*card);
                    cast_event_index = Some(event_index);
                    let definition = game
                        .card_definition(*card)
                        .map_err(|error| error.to_string())?;
                    spell_cost = Some(definition.mana_cost.clone());
                    spell_colors = definition.colors.iter().copied().collect();
                    for owner in 0..2 {
                        graveyards_at_cast[owner] = game
                            .player(PlayerId(owner))
                            .map_err(|error| error.to_string())?
                            .graveyard
                            .clone();
                    }
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    spell_payment = colors.clone()
                }
                GameEvent::SpellCast {
                    player: caster,
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == RECRUIT_ID) =>
                {
                    pilot_policy.creatures_cast[caster.0] += 1;
                    opponent_policy.creatures_cast[caster.0] += 1;
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == RAIN_ID) =>
                {
                    rain_card = Some(*card);
                    pilot_policy.rain_submitted = true;
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Graveyard,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == RECRUIT_ID) =>
                {
                    let owner = game
                        .object(*card)
                        .map_err(|error| error.to_string())?
                        .owner
                        .0;
                    creature_cards_died[owner].push(*card);
                    pilot_policy.creature_deaths[owner] += 1;
                    opponent_policy.creature_deaths[owner] += 1;
                }
                GameEvent::SpellResolved { card } if Some(*card) == catacombs => {
                    catacombs_resolved = true;
                    pilot_policy.catacombs_resolved = true;
                    opponent_policy.catacombs_resolved = true;
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Hand,
                } if cast_event_index.is_some_and(|cast| event_index > cast)
                    && !catacombs_resolved =>
                {
                    hand_moves_after_cast.push(*card)
                }
                GameEvent::AttackersDeclared {
                    player: PlayerId(0),
                    attackers,
                } if !attackers.is_empty() => attacks.push(game.turn),
                GameEvent::DamageDealtToPlayer {
                    source,
                    player: PlayerId(1),
                    amount,
                } if game
                    .card_definition(*source)
                    .is_ok_and(|definition| definition.id == WATCHWOLF_ID) =>
                {
                    combat_damage_to_opponent += *amount
                }
                _ => {}
            }
        }
    }
    let events = game.event_log.clone();
    let terminal = game.winner().map_or_else(
        || "Draw".to_owned(),
        |winner| format!("Winner(PlayerId({}))", winner.0),
    );
    let catacombs_in_owners_graveyard_at_end = catacombs.is_some_and(|card| {
        game.player(PlayerId(0))
            .is_ok_and(|owner| owner.graveyard.contains(&card))
    });
    let creature_cards_still_in_graveyard = std::array::from_fn(|owner| {
        game.player(PlayerId(owner))
            .expect("two configured players remain addressable")
            .graveyard
            .iter()
            .copied()
            .filter(|card| {
                game.card_definition(*card)
                    .is_ok_and(|definition| definition.id == RECRUIT_ID)
            })
            .collect()
    });
    Ok(Captured {
        events,
        terminal,
        winner: game.winner().map(|winner| winner.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        catacombs,
        rain_card,
        creature_cards_died,
        graveyards_at_cast,
        hand_moves_after_cast,
        creature_cards_still_in_graveyard,
        catacombs_resolved,
        catacombs_in_owners_graveyard_at_end,
        spell_payment,
        spell_cost,
        spell_colors,
        attacks,
        combat_damage_to_opponent,
        life: [
            game.player(PlayerId(0))
                .map_err(|error| error.to_string())?
                .life,
            game.player(PlayerId(1))
                .map_err(|error| error.to_string())?
                .life,
        ],
    })
}

fn deck(cards: &[(&str, u8)]) -> DeckList {
    DeckList {
        mainboard: cards
            .iter()
            .map(|(card, count)| DeckEntry {
                card: (*card).to_owned(),
                count: *count,
            })
            .collect(),
        sideboard: vec![],
    }
}

fn propose_action(policy: &mut CatacombsPolicy, view: &GameView) -> PolicyAction {
    if view.decision_player != policy.player
        || view.priority != policy.player
        || view.active_player != policy.player
        || view.stack_depth != 0
        || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
    {
        if view.step == Step::DeclareAttackers
            && view.active_player == policy.player
            && !view.attackers_declared
        {
            let attackers = view
                .own_battlefield
                .iter()
                .filter(|card| card.definition == Some(WATCHWOLF_ID) && card.can_attack)
                .map(|card| card.id)
                .collect();
            return PolicyAction::DeclareAttackers { attackers };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != policy.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        return PolicyAction::PassPriority;
    }
    if view.lands_played == 0
        && let Some(land) = view
            .hand
            .iter()
            .find(|card| card.card_types.contains(&CardType::Land))
    {
        return PolicyAction::PlayLand { card: land.id };
    }
    if policy.creatures_cast[policy.player.0] < 2
        && let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some(RECRUIT_ID))
    {
        let cost = ManaCost::with_colors(0, [Color::White]);
        if let Some(selection) = payment(view, &cost) {
            return PolicyAction::CastWithPayment {
                request: CastRequest {
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
    if policy.player == PlayerId(0)
        && policy.creatures_cast[0] >= 2
        && policy.creatures_cast[1] >= 2
        && !policy.rain_submitted
        && let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some(RAIN_ID))
    {
        let cost = ManaCost::with_colors(1, [Color::Red]);
        if let Some(selection) = payment(view, &cost) {
            policy.rain_submitted = true;
            return PolicyAction::CastWithPayment {
                request: CastRequest {
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
    if policy.player == PlayerId(0)
        && policy.creature_deaths[0] >= 2
        && policy.creature_deaths[1] >= 2
        && !policy.catacombs_submitted
        && let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some(CARD_ID))
    {
        let cost = ManaCost::with_colors(3, [Color::Black]);
        if let Some(selection) = payment(view, &cost) {
            policy.catacombs_submitted = true;
            return PolicyAction::CastWithPayment {
                request: CastRequest {
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
    if policy.player == PlayerId(0)
        && policy.catacombs_resolved
        && let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some(WATCHWOLF_ID))
    {
        let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
        if let Some(selection) = payment(view, &cost) {
            return PolicyAction::CastWithPayment {
                request: CastRequest {
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
    PolicyAction::PassPriority
}

struct CatacombsPolicy {
    player: PlayerId,
    rain_submitted: bool,
    catacombs_submitted: bool,
    creatures_cast: [u8; 2],
    creature_deaths: [u8; 2],
    catacombs_resolved: bool,
}
impl CatacombsPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            rain_submitted: false,
            catacombs_submitted: false,
            creatures_cast: [0, 0],
            creature_deaths: [0, 0],
            catacombs_resolved: false,
        }
    }
}
impl CodePolicy for CatacombsPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.empty-the-catacombs-pilot.v1"
        } else {
            "rav.empty-the-catacombs-opponent.v1"
        }
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        propose_action(self, view)
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
        need = cost.colored.first().copied().or(Some(Color::Green));
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
    if c.terminal != "Winner(PlayerId(0))" || c.winner != Some(0) || c.rejected != 0 {
        return Err(format!(
            "Empty the Catacombs duel did not end in a clean pilot win: terminal={} rejected={}",
            c.terminal, c.rejected
        ));
    }
    let catacombs = c.catacombs.ok_or("Empty the Catacombs was not cast")?;
    if !c.catacombs_resolved || !c.catacombs_in_owners_graveyard_at_end {
        return Err(
            "Empty the Catacombs did not resolve before entering its owner's graveyard".to_owned(),
        );
    }
    if c.rain_card.is_none() || c.creature_cards_died.iter().any(|cards| cards.len() < 2) {
        return Err("each player must have at least two real creature cards die before the sorcery resolves".to_owned());
    }
    if c.creature_cards_died
        .iter()
        .enumerate()
        .any(|(owner, cards)| {
            cards
                .iter()
                .any(|card| !c.graveyards_at_cast[owner].contains(card))
        })
    {
        return Err(
            "the full creature-return setup was not present in each owner's graveyard at casting"
                .to_owned(),
        );
    }
    if c.creature_cards_still_in_graveyard
        .iter()
        .any(|cards| !cards.is_empty())
    {
        return Err(
            "at least one eligible creature card remained in a graveyard after resolution"
                .to_owned(),
        );
    }
    if c.hand_moves_after_cast
        != c.creature_cards_died
            .iter()
            .flatten()
            .copied()
            .collect::<Vec<_>>()
    {
        return Err(format!(
            "the effect must return every eligible creature and no other graveyard card: returned={:?} eligible={:?}",
            c.hand_moves_after_cast, c.creature_cards_died
        ));
    }
    if c.hand_moves_after_cast.contains(&catacombs)
        || c.rain_card
            .is_some_and(|rain| c.hand_moves_after_cast.contains(&rain))
        || c.spell_payment.len() != 4
        || c.spell_payment
            .iter()
            .filter(|color| **color == Color::Black)
            .count()
            != 1
        || c.spell_cost != Some(ManaCost::with_colors(3, [Color::Black]))
        || c.spell_colors != [Color::Black]
    {
        return Err(
            "the spell's black mana, sorcery identity, or exclusion of noncreatures is incorrect"
                .to_owned(),
        );
    }
    if c.events.iter().any(|event| {
        matches!(
            event,
            GameEvent::DecisionOpened {
                kind: cardbench_magic_engine::DecisionKind::PublicGraveyardCreatureReturn,
                ..
            }
        )
    }) {
        return Err(
            "the all-creature effect incorrectly requested a player card-selection choice"
                .to_owned(),
        );
    }
    if c.attacks.is_empty() || c.combat_damage_to_opponent <= 0 || c.turns == 0 {
        return Err(
            "the card trace did not continue through meaningful full-game PvP combat".to_owned(),
        );
    }
    let projected = project_events(&c.events);
    let ordered = ordered_receipts(&c.events, catacombs, &c.creature_cards_died, c.rain_card)?;
    Ok(json!({
        "trace_id":"rav-empty-the-catacombs-return-all-creatures-from-each-graveyard-full-game",
        "passed":true,
        "seed":seed,
        "terminal":c.terminal,
        "winner_seat":c.winner,
        "turns":c.turns,
        "accepted_moves":c.accepted,
        "rejected_moves":c.rejected,
        "catacombs":{"object":catacombs.0,"mana_cost":"{3}{B}","mana_paid":c.spell_payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"card_colors":c.spell_colors.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"resolved_before_owner_graveyard":c.catacombs_resolved,"ends_in_owner_graveyard":c.catacombs_in_owners_graveyard_at_end},
        "both_players":{"graveyards_at_cast":c.graveyards_at_cast.iter().map(|yard|yard.iter().map(|card|card.0).collect::<Vec<_>>()).collect::<Vec<_>>(),"creatures_died":c.creature_cards_died.iter().map(|yard|yard.iter().map(|card|card.0).collect::<Vec<_>>()).collect::<Vec<_>>(),"returned_to_hand":c.hand_moves_after_cast.iter().map(|card|card.0).collect::<Vec<_>>(),"creatures_left_in_graveyards":c.creature_cards_still_in_graveyard.iter().map(|yard|yard.iter().map(|card|card.0).collect::<Vec<_>>()).collect::<Vec<_>>(),"rain_spell":c.rain_card.map(|card|card.0)},
        "public_creature_choice_opened":false,
        "combat":{"attacking_turns":c.attacks,"damage_to_opponent":c.combat_damage_to_opponent},
        "life_totals":c.life,
        "ordered_event_receipts":ordered,
        "event_count":c.events.len(),
        "projected_event_count":projected.len(),
        "zone_move_count":c.events.iter().filter(|event| matches!(event, GameEvent::CardMoved { .. })).count()
    }))
}

fn ordered_receipts(
    events: &[GameEvent],
    catacombs: ObjectId,
    creatures: &[Vec<ObjectId>; 2],
    rain: Option<ObjectId>,
) -> Result<Value, String> {
    let cast = events
        .iter()
        .position(|event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == catacombs))
        .ok_or("Empty the Catacombs cast event missing")?;
    let resolved = events
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == catacombs))
        .ok_or("Empty the Catacombs SpellResolved event missing")?;
    let terminal_zone = events
        .iter()
        .position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == catacombs))
        .ok_or("Empty the Catacombs terminal graveyard move missing")?;
    let hand_moves = |cards: &[ObjectId]| -> Result<Vec<usize>, String> {
        cards.iter().map(|card| {
            events.iter().enumerate().position(|(index, event)| index > cast && matches!(event, GameEvent::CardMoved { card: moved, to: Zone::Hand } if moved == card)).ok_or_else(|| format!("Empty the Catacombs did not return creature {card:?} to its owner's hand"))
        }).collect()
    };
    let owner_moves = [hand_moves(&creatures[0])?, hand_moves(&creatures[1])?];
    if owner_moves[0].iter().any(|index| *index >= resolved)
        || owner_moves[1].iter().any(|index| *index >= resolved)
        || owner_moves[0].iter().any(|index| owner_moves[1].iter().any(|other| index >= other))
        || !(cast < resolved && resolved < terminal_zone)
        || rain.is_some_and(|rain_card| events.iter().enumerate().any(|(index, event)| {
            index > cast && index < resolved && matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == rain_card)
        }))
    {
        return Err("all creature returns, spell resolution, and the spell's terminal graveyard move are out of order or include a noncreature".to_owned());
    }
    Ok(json!({
        "cast":cast,
        "seat_zero_creature_returns":owner_moves[0],
        "seat_one_creature_returns":owner_moves[1],
        "spell_resolved":resolved,
        "spell_to_owners_graveyard":terminal_zone
    }))
}

fn card_status(card: &CatalogCard) -> Value {
    let (engine_status, definition_id) = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => {
            ("executable_compatibility_slice", Some(definition_id))
        }
        CardSemanticStatus::CatalogOnly { .. } => ("catalog_only", None),
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"definition_id":definition_id,"name":card.name,"engine_status":engine_status,"trace_status":if passed {"passed"} else {"not_qualified"},"credited":false})
}
fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}
fn output_root() -> Option<std::path::PathBuf> {
    std::env::var_os("CARDBENCH_TASK_OUTPUT").map(std::path::PathBuf::from)
}
fn write_outputs(root: &std::path::Path, report: &Value) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    std::fs::write(root.join("result.json"), serde_json::to_vec_pretty(report)?)?;
    Ok(())
}

fn validate_registry_delegation() -> Result<(), String> {
    let candidate = candidate_empty_the_catacombs::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Empty the Catacombs is missing from the RAV registry")?;
    if registered != candidate {
        return Err(
            "Empty the Catacombs registry definition does not delegate to its candidate module"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Empty the Catacombs source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Empty the Catacombs source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Empty the Catacombs module has an unsupported module attribute".to_owned());
    }
    let mut imports = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if matches!(item.vis, Visibility::Inherited) && item.attrs.is_empty() =>
            {
                imports.extend(
                    flattened_use_paths(&item.tree)
                        .ok_or("Empty the Catacombs import shape differs")?,
                )
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if name != "definition"
                    || !functions.insert(name)
                    || !matches!(&function.vis, Visibility::Restricted(visibility) if visibility.path.is_ident("crate"))
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || !function.sig.inputs.is_empty()
                    || !function.sig.generics.params.is_empty()
                    || function.sig.variadic.is_some()
                    || function.block.stmts.len() != 1
                    || !function.attrs.iter().all(|attribute| {
                        is_doc_attribute(attribute) || attribute.path().is_ident("must_use")
                    })
                {
                    return Err("Empty the Catacombs module must expose only one safe no-argument candidate function".to_owned());
                }
                let Stmt::Expr(Expr::Struct(literal), None) = &function.block.stmts[0] else {
                    return Err(
                        "Empty the Catacombs definition must be a direct struct literal".to_owned(),
                    );
                };
                if literal.rest.is_some() || !declarative_expr(&Expr::Struct(literal.clone())) {
                    return Err(
                        "Empty the Catacombs source contains unsupported executable code"
                            .to_owned(),
                    );
                }
            }
            _ => {
                return Err(
                    "Empty the Catacombs module permits only private imports and its definition"
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
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    if imports != expected || functions != BTreeSet::from(["definition".to_owned()]) {
        return Err("Empty the Catacombs definition imports or function set differs".to_owned());
    }
    let definition = candidate_empty_the_catacombs::definition();
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(3, [Color::Black])
        || definition.colors != BTreeSet::from([Color::Black])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Sorcery])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
        || definition.effects != [Effect::ReturnAllCreatureCardsFromEachGraveyardToHand]
    {
        return Err("Empty the Catacombs identity, {3}{B} cost, sorcery type, or all-creature return effect differs".to_owned());
    }
    Ok(())
}
fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc")
        && matches!(&attribute.meta, syn::Meta::NameValue(value) if matches!(&value.value, Expr::Lit(literal) if matches!(literal.lit, Lit::Str(_))))
}
fn flattened_use_paths(tree: &UseTree) -> Option<Vec<Vec<String>>> {
    let mut output = Vec::new();
    flatten_use_tree(&[], tree, &mut output).then_some(output)
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
fn declarative_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(literal) => matches!(literal.lit, Lit::Str(_) | Lit::Int(_) | Lit::Bool(_)),
        Expr::Path(_) => true,
        Expr::Unary(unary) => {
            matches!(unary.op, syn::UnOp::Neg(_)) && matches!(&*unary.expr, Expr::Lit(_))
        }
        Expr::Array(array) => array.elems.iter().all(declarative_expr),
        Expr::Reference(reference) => declarative_expr(&reference.expr),
        Expr::Struct(value) => {
            value.attrs.is_empty()
                && value.fields.iter().all(|field| {
                    matches!(field.member, syn::Member::Named(_)) && declarative_expr(&field.expr)
                })
                && value.rest.as_deref().is_none_or(declarative_expr)
        }
        Expr::Call(call) => {
            let Expr::Path(path) = &*call.func else {
                return false;
            };
            let names: Vec<_> = path
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect();
            (matches!(names.as_slice(), [ty, method] if (ty == "ManaCost" && method == "with_colors") || (ty == "BTreeSet" && (method == "from" || method == "new")))
                || matches!(names.as_slice(), [one] if one == "Some"))
                && call.args.iter().all(declarative_expr)
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
