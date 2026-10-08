//! Full-game RAV Recollect own-graveyard return authority.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, Zone,
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
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::parse::Parser;
use syn::{Expr, Item, Lit, Meta, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-RECOLLECT";
const GATHER_ID: &str = "RAV-GATHER-COURAGE";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/recollect.rs";
const DECK_P0: &str = "rav_recollect_graveyard_return_60";
const DECK_P1: &str = "rav_gather_opponent_graveyard_60";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/recollect.rs"]
mod candidate_recollect;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Recollect result: {error}");
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
            eprintln!("Recollect PvP authority failed: {error}");
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
        return Err("Recollect shuffled-game replay was not deterministic".to_owned());
    }
    let mutations = mutation_sensitivity(seed)?;
    for key in [
        "wrong_target_mutation_detected_in_game",
        "creature_only_target_mutation_detected_in_game",
        "wrong_cost_mutation_detected_in_game",
        "wrong_color_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Recollect semantic mutation escaped the PvP trace: {key}"
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
        "card":{"collector_number":178,"printings":1,"definition_id":CARD_ID,"name":"Recollect"},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":[
                "105.2 and 202.1-202.2: green color and mana cost",
                "117: priority and spell resolution timing",
                "304.1: sorcery timing",
                "400.7: zone changes create new game objects",
                "601.2: casting and announcing a graveyard target",
                "608.2: resolving the targeted return instruction",
                "zone ownership and controller-scoped own-graveyard targets"
            ],
            "source_contract":"RAV Recollect #178, {2}{G} sorcery: return target card from your graveyard to your hand",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true
        },
        "passed":true,
        "pilot_card_passed":true,
        "replay_deterministic":true,
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
    WrongTarget,
    CreatureOnlyTarget,
    WrongCost,
    WrongColor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    recollect: Option<ObjectId>,
    gather: Option<ObjectId>,
    watchwolf: Option<ObjectId>,
    target: Option<Target>,
    target_zone_at_cast: Option<Zone>,
    target_zone_at_end: Option<Zone>,
    recollect_payment: Vec<Color>,
    gather_payment: Vec<Color>,
    source_colors: Vec<Color>,
    attacks: Vec<u32>,
    combat_damage: i32,
    life: [i64; 2],
}

fn same_match(left: &Captured, right: &Captured) -> bool {
    left.events == right.events
        && left.terminal == right.terminal
        && left.winner == right.winner
        && left.turns == right.turns
        && left.recollect == right.recollect
        && left.gather == right.gather
        && left.watchwolf == right.watchwolf
        && left.target == right.target
        && left.target_zone_at_cast == right.target_zone_at_cast
        && left.target_zone_at_end == right.target_zone_at_end
        && left.recollect_payment == right.recollect_payment
        && left.gather_payment == right.gather_payment
        && left.source_colors == right.source_colors
        && left.attacks == right.attacks
        && left.combat_damage == right.combat_damage
        && left.life == right.life
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_RECOLLECT_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_RECOLLECT_SEED must be an unsigned integer")?;
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
        "no bounded seed produced the Recollect own-graveyard return trace: {last_error}"
    ))
}

fn mutation_sensitivity(seed: u64) -> Result<Value, String> {
    let mut outcomes = serde_json::Map::new();
    for (name, mutation) in [
        (
            "wrong_target_mutation_detected_in_game",
            Mutation::WrongTarget,
        ),
        (
            "creature_only_target_mutation_detected_in_game",
            Mutation::CreatureOnlyTarget,
        ),
        ("wrong_cost_mutation_detected_in_game", Mutation::WrongCost),
        (
            "wrong_color_mutation_detected_in_game",
            Mutation::WrongColor,
        ),
    ] {
        let detected = match run_match(seed, mutation) {
            Ok(captured) => validate_trace(&captured, seed).is_err(),
            Err(_) => true,
        };
        outcomes.insert(name.to_owned(), Value::Bool(detected));
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
                card: GATHER_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: WATCHWOLF_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 24,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 24,
            },
        ],
        sideboard: vec![],
    };
    let opponent = DeckList {
        mainboard: vec![
            DeckEntry {
                card: GATHER_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: WATCHWOLF_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 24,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 28,
            },
        ],
        sideboard: vec![],
    };
    for (label, deck) in [(DECK_P0, &pilot), (DECK_P1, &opponent)] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{label} deck has {count} cards; expected 60"));
        }
    }

    let mut definitions = card_definitions();
    let mut candidate = candidate_recollect::definition();
    match mutation {
        Mutation::None | Mutation::WrongTarget => {}
        Mutation::CreatureOnlyTarget => {
            candidate.effects = vec![Effect::ReturnTargetCreatureCardToHand]
        }
        Mutation::WrongCost => candidate.mana_cost = ManaCost::with_colors(3, [Color::Green]),
        Mutation::WrongColor => candidate.colors = BTreeSet::from([Color::Blue]),
    }
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Recollect is absent from the RAV registry")? = candidate;
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

    let mut pilot_policy = RecollectPolicy::new(PlayerId(0), mutation);
    let mut opponent_policy = RecollectPolicy::new(PlayerId(1), Mutation::None);
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut recollect = None;
    let mut gather = None;
    let mut watchwolf = None;
    let mut recollect_target = None;
    let mut target_zone_at_cast = None;
    let mut recollect_payment = Vec::new();
    let mut gather_payment = Vec::new();
    let mut source_colors = Vec::new();
    let mut attacks = Vec::new();
    let mut combat_damage = 0_i32;

    while !game.is_game_over() {
        if game.turn > 24 || accepted >= 4000 {
            let p0 = game
                .player(PlayerId(0))
                .map_err(|error| error.to_string())?;
            let p1 = game
                .player(PlayerId(1))
                .map_err(|error| error.to_string())?;
            let visible_names = |cards: &[ObjectId]| {
                cards
                    .iter()
                    .filter_map(|card| {
                        game.card_definition(*card)
                            .ok()
                            .map(|definition| definition.id)
                    })
                    .collect::<Vec<_>>()
            };
            return Err(format!(
                "Recollect match exceeded its turn/action bound at turn {}; seat0 life={} hand={:?} battlefield={:?} own_gather={:?} attacks={} seat1 life={} hand={:?} battlefield={:?}",
                game.turn,
                p0.life,
                visible_names(&p0.hand),
                visible_names(&p0.battlefield),
                pilot_policy.own_gather,
                attacks.len(),
                p1.life,
                visible_names(&p1.hand),
                visible_names(&p1.battlefield)
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
        let new_events = &game.event_log[cursor..];
        for event in new_events {
            match event {
                GameEvent::SpellCast {
                    player: controller,
                    card,
                } if *controller == player => {
                    let id = game
                        .card_definition(*card)
                        .map_err(|error| error.to_string())?
                        .id;
                    if id == GATHER_ID {
                        if let Some(policy) = if player == PlayerId(0) {
                            Some(&mut pilot_policy)
                        } else {
                            Some(&mut opponent_policy)
                        } {
                            policy.gather_submitted = true;
                        }
                    }
                    if *controller == PlayerId(0) && id == WATCHWOLF_ID && watchwolf.is_none() {
                        watchwolf = Some(*card);
                    }
                    if *controller == PlayerId(0) && id == CARD_ID {
                        recollect = Some(*card);
                        if let Some(stack) = game.stack.iter().find(|stack| stack.card == *card) {
                            recollect_target = stack.targets.first().copied();
                            if let Some(Target::Permanent(target)) = recollect_target {
                                target_zone_at_cast = game.zone_of(target);
                            }
                            source_colors = stack.source_colors.iter().copied().collect();
                        }
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
                    recollect_payment = colors.clone()
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == GATHER_ID) =>
                {
                    gather_payment = colors.clone()
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Graveyard,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == GATHER_ID) =>
                {
                    if game
                        .player(PlayerId(0))
                        .is_ok_and(|seat| seat.graveyard.contains(card))
                    {
                        gather = Some(*card);
                        pilot_policy.own_gather = Some(*card);
                    }
                    if game
                        .player(PlayerId(1))
                        .is_ok_and(|seat| seat.graveyard.contains(card))
                    {
                        opponent_policy.own_gather = Some(*card);
                        pilot_policy.opponent_gather = Some(*card);
                    }
                }
                GameEvent::AttackersDeclared {
                    player: PlayerId(0),
                    attackers,
                } => {
                    if attackers.iter().any(|attacker| {
                        game.card_definition(*attacker)
                            .is_ok_and(|definition| definition.id == WATCHWOLF_ID)
                    }) {
                        attacks.push(game.turn);
                    }
                }
                GameEvent::DamageDealtToPlayer {
                    source,
                    player: PlayerId(1),
                    amount,
                } if game
                    .card_definition(*source)
                    .is_ok_and(|definition| definition.id == WATCHWOLF_ID) =>
                {
                    combat_damage += *amount
                }
                _ => {}
            }
        }
    }
    let events = game.event_log.clone();
    let target_zone_at_end = recollect_target.and_then(|target| match target {
        Target::Permanent(object) => game.zone_of(object),
        _ => None,
    });
    let recollect = events
        .iter()
        .find_map(|event| match event {
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
        .or(recollect);
    Ok(Captured {
        events,
        terminal: game.winner().map_or_else(
            || "Draw".to_owned(),
            |winner| format!("Winner(PlayerId({}))", winner.0),
        ),
        winner: game.winner().map(|winner| winner.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        recollect,
        gather,
        watchwolf,
        target: recollect_target,
        target_zone_at_cast,
        target_zone_at_end,
        recollect_payment,
        gather_payment,
        source_colors,
        attacks,
        combat_damage,
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

fn propose_action(policy: &mut RecollectPolicy, view: &GameView) -> PolicyAction {
    if let Some(action) = policy.propose_pending_decision(view) {
        return action;
    }
    if view.optional_triggered_ability_choice.is_some() {
        return policy.propose_optional_triggered_ability(view);
    }
    if view.draw_replacement_pending {
        return policy.propose_draw_replacement(view);
    }
    if view.private_library_choice.is_some() {
        return policy.propose_private_library_choice(view);
    }
    if view.private_opponent_library_choice.is_some() {
        return policy.propose_private_opponent_library_choice(view);
    }
    if view.library_search_choice.is_some() {
        return policy.propose_library_search_choice(view);
    }
    if view.step == Step::DeclareAttackers
        && view.active_player == policy.player
        && !view.attackers_declared
    {
        let attackers = if policy.player == PlayerId(0) {
            view.own_battlefield
                .iter()
                .filter(|card| card.definition == Some(WATCHWOLF_ID) && card.can_attack)
                .map(|card| card.id)
                .collect()
        } else {
            vec![]
        };
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
    if view.decision_player != policy.player
        || view.priority != policy.player
        || view.active_player != policy.player
        || view.stack_depth != 0
        || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
    {
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

    if let Some(card) = view
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

    if !policy.gather_submitted
        && let Some(wolf) = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some(WATCHWOLF_ID))
        && let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some(GATHER_ID))
    {
        let cost = ManaCost::with_colors(0, [Color::Green]);
        if let Some(selection) = payment(view, &cost) {
            policy.gather_submitted = true;
            return PolicyAction::CastWithPayment {
                request: CastRequest {
                    card: card.id,
                    targets: vec![Target::Permanent(wolf.id)],
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
        && !policy.recollect_submitted
        && let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some(CARD_ID))
    {
        let own_target = policy.own_gather;
        let target = if policy.mutation == Mutation::WrongTarget {
            policy.opponent_gather
        } else {
            own_target
        };
        if let Some(target) = target {
            let cost = ManaCost::with_colors(2, [Color::Green]);
            if let Some(selection) = payment(view, &cost) {
                policy.recollect_submitted = true;
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: card.id,
                        targets: vec![Target::Permanent(target)],
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

struct RecollectPolicy {
    player: PlayerId,
    mutation: Mutation,
    gather_submitted: bool,
    recollect_submitted: bool,
    own_gather: Option<ObjectId>,
    opponent_gather: Option<ObjectId>,
}

impl RecollectPolicy {
    fn new(player: PlayerId, mutation: Mutation) -> Self {
        Self {
            player,
            mutation,
            gather_submitted: false,
            recollect_submitted: false,
            own_gather: None,
            opponent_gather: None,
        }
    }
}

impl CodePolicy for RecollectPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.recollect-own-graveyard.v1"
        } else {
            "rav.graveyard-target-fixture.v1"
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
    let recollect = c.recollect.ok_or("Recollect was not cast")?;
    let gather = c
        .gather
        .ok_or("Gather Courage did not reach the pilot's graveyard")?;
    let target = match c.target {
        Some(Target::Permanent(target)) => target,
        _ => return Err("Recollect did not announce a card target".to_owned()),
    };
    if target != gather {
        return Err(format!(
            "Recollect selected {target:?}; expected the pilot-owned Gather Courage {gather:?}"
        ));
    }
    if c.target_zone_at_cast != Some(Zone::Graveyard) || c.target_zone_at_end != Some(Zone::Hand) {
        return Err(format!(
            "Recollect target did not move from its own graveyard into the pilot's hand: {:?} -> {:?}",
            c.target_zone_at_cast, c.target_zone_at_end
        ));
    }
    if c.recollect_payment.len() != 3 || !c.recollect_payment.contains(&Color::Green) {
        return Err(format!(
            "Recollect did not pay {{2}}{{G}}: {:?}",
            c.recollect_payment
        ));
    }
    if c.gather_payment != [Color::Green] {
        return Err(format!(
            "support Gather Courage did not pay {{G}}: {:?}",
            c.gather_payment
        ));
    }
    if c.source_colors != [Color::Green] {
        return Err(format!(
            "Recollect color identity differs: {:?}",
            c.source_colors
        ));
    }
    if c.terminal != "Winner(PlayerId(0))"
        || c.winner != Some(0)
        || c.life[0] <= 0
        || c.life[1] > 0
        || c.rejected != 0
    {
        return Err(format!(
            "Recollect PvP match did not finish cleanly for seat zero: {:?}",
            c.life
        ));
    }
    if c.attacks.len() < 3 || c.combat_damage < 9 {
        return Err("Watchwolf did not complete repeated combat in the full game".to_owned());
    }

    let gather_cast = event_index(&c.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == gather)).ok_or("Gather Courage cast receipt missing")?;
    let gather_resolved = event_index_after(
        &c.events,
        gather_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == gather),
    )
    .ok_or("Gather Courage resolution receipt missing")?;
    let gather_graveyard = event_index_after(&c.events, gather_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == gather)).ok_or("Gather Courage did not move into the pilot's graveyard")?;
    let recollect_paid = event_index(&c.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == recollect && colors.len() == 3 && colors.contains(&Color::Green))).ok_or("Recollect exact colored payment receipt missing")?;
    let recollect_cast = event_index_after(&c.events, recollect_paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == recollect)).ok_or("Recollect cast receipt missing")?;
    let returned = event_index_after(
        &c.events,
        recollect_cast,
        |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == gather),
    )
    .ok_or("Recollect target did not move to the pilot's hand")?;
    let recollect_resolved = event_index_after(
        &c.events,
        returned,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == recollect),
    )
    .ok_or("Recollect resolution receipt missing")?;
    let recollect_graveyard = event_index_after(&c.events, recollect_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == recollect)).ok_or("resolved Recollect did not move to its owner's graveyard")?;
    let watchwolf = c.watchwolf.ok_or("pilot Watchwolf was not cast")?;
    let wolf_cast = event_index(&c.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == watchwolf)).ok_or("Watchwolf cast receipt missing")?;
    let attack = event_index_after(&c.events, recollect_graveyard, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&watchwolf))).ok_or("Watchwolf did not attack after Recollect resolved")?;
    let damage = event_index_after(&c.events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount } if *source == watchwolf && *amount > 0)).ok_or("Watchwolf did not deal combat damage")?;
    let ended = event_index_after(&c.events, damage, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("seat-zero terminal event missing")?;
    if !(gather_cast < gather_resolved
        && gather_resolved < gather_graveyard
        && gather_graveyard < recollect_paid
        && recollect_paid < recollect_cast
        && recollect_cast < returned
        && returned < recollect_resolved
        && recollect_resolved < recollect_graveyard
        && recollect_graveyard < attack
        && attack < damage
        && damage < ended)
    {
        return Err(format!(
            "Recollect graveyard targeting, return, and full-game event order is invalid: wolf={wolf_cast} gather={gather_cast}/{gather_resolved}/{gather_graveyard} recollect={recollect_paid}/{recollect_cast}/{returned}/{recollect_resolved}/{recollect_graveyard} combat={attack}/{damage}/{ended}"
        ));
    }
    let projected = project_events(&c.events);
    let events: Vec<Value> = projected.iter().map(|event| json!({"sequence":event.sequence,"turn":event.turn,"kind":&event.kind,"canonical":&event.canonical,"facts":&event.facts})).collect();
    Ok(json!({
        "trace_id":"rav-recollect-own-graveyard-return-full-game",
        "passed":true,
        "shuffle_seed":seed,
        "decks":[DECK_P0,DECK_P1],
        "terminal":c.terminal,
        "winner_seat":c.winner,
        "turns":c.turns,
        "accepted_moves":c.accepted,
        "rejected_moves":c.rejected,
        "recollect":{"object":recollect.0,"target_object":target.0,"target_definition":GATHER_ID,"target_zone_at_cast":"Graveyard","target_zone_after_resolution":"Hand","mana_cost":"{2}{G}","mana_paid":c.recollect_payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"source_colors":c.source_colors.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>()},
        "support":{"card":"Gather Courage","object":gather.0,"mana_cost":"{G}","mana_paid":c.gather_payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>()},
        "combat":{"card":"Watchwolf","object":watchwolf.0,"attack_turns":c.attacks,"damage_to_opponent":c.combat_damage,"life_at_terminal":c.life},
        "ordered_event_sequence":[wolf_cast,gather_cast,gather_resolved,gather_graveyard,recollect_paid,recollect_cast,returned,recollect_resolved,recollect_graveyard,attack,damage,ended],
        "events":events
    }))
}

fn event_index(
    events: &[GameEvent],
    mut predicate: impl FnMut(&GameEvent) -> bool,
) -> Option<usize> {
    events.iter().position(|event| predicate(event))
}
fn event_index_after(
    events: &[GameEvent],
    after: usize,
    mut predicate: impl FnMut(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(after + 1)
        .find_map(|(index, event)| predicate(event).then_some(index))
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

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}
fn output_root() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--output-root" {
            return args.next().map(std::path::PathBuf::from);
        }
    }
    None
}
fn write_outputs(root: &std::path::Path, report: &Value) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).expect("report serializes"),
    )?;
    std::fs::write(root.join("reward.txt"), "0.0\n")
}

fn validate_registry_delegation() -> Result<(), String> {
    let candidate = candidate_recollect::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Recollect is missing from the RAV registry")?;
    if registered != candidate {
        return Err(
            "Recollect registry definition does not delegate to its candidate module".to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Recollect source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Recollect source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Recollect module has an unsupported module attribute".to_owned());
    }
    let mut imports = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if matches!(item.vis, Visibility::Inherited) && item.attrs.is_empty() =>
            {
                imports.extend(
                    flattened_use_paths(&item.tree).ok_or("Recollect import shape differs")?,
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
                    || function.sig.generics.params.len() != 0
                    || function.sig.variadic.is_some()
                    || function.block.stmts.len() != 1
                    || !function.attrs.iter().all(|attribute| {
                        is_doc_attribute(attribute) || attribute.path().is_ident("must_use")
                    })
                {
                    return Err(
                        "Recollect module must expose only one safe no-argument candidate function"
                            .to_owned(),
                    );
                }
                let Stmt::Expr(Expr::Struct(literal), None) = &function.block.stmts[0] else {
                    return Err("Recollect definition must be a direct struct literal".to_owned());
                };
                if literal.rest.is_some() || !declarative_expr(&Expr::Struct(literal.clone())) {
                    return Err("Recollect source contains unsupported executable code".to_owned());
                }
            }
            _ => {
                return Err(
                    "Recollect module permits only private imports and its definition".to_owned(),
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
        return Err("Recollect definition imports or function set differs".to_owned());
    }
    let definition = candidate_recollect::definition();
    if definition.id != CARD_ID
        || definition.name != "Recollect"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(2, [Color::Green])
        || definition.colors != BTreeSet::from([Color::Green])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Sorcery])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
        || definition.effects != [Effect::ReturnTargetCardToHand]
    {
        return Err("Recollect identity or own-graveyard return effect differs".to_owned());
    }
    Ok(())
}

fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc")
        && matches!(&attribute.meta,Meta::NameValue(value)if matches!(&value.value,Expr::Lit(literal)if matches!(literal.lit,Lit::Str(_))))
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
            (matches!(names.as_slice(),[ty,method] if(ty=="ManaCost"&&method=="with_colors")||(ty=="BTreeSet"&&(method=="from"||method=="new")))
                || matches!(names.as_slice(),[one]if one=="Some"))
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
