//! Isolated full-game authority for RAV #220 Psychic Drain.

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DeckEntry, DeckList, DeckRules, Effect, Game,
    GameEvent, GameView, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step,
    Target, Zone,
};
use cardbench_magic_policies::{CatalogPolicyProfile, CodePolicy, DeckMatchConfig, RavCatalogPolicy};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-PSYCHIC-DRAIN";
const CARD_NAME: &str = "Psychic Drain";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/psychic_drain.rs";
const MAX_TRACE_SEED: u64 = 96;
const MAX_TURNS: u32 = 80;
const MAX_MOVES: u32 = 8_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/psychic_drain.rs"]
mod candidate_psychic_drain;

const PINNED_SOURCE: &str = r##"//! Candidate-owned RAV Psychic Drain identity and chosen-X spell effect.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};
use std::collections::BTreeSet;

/// RAV #220: a blue-black sorcery with X in its mana cost. It mills the
/// targeted player for X cards and its controller gains X life.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-PSYCHIC-DRAIN",
        name: "Psychic Drain",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Blue, Color::Black]),
        colors: BTreeSet::from([Color::Blue, Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Sorcery]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "chosen-x-targeted-mill-and-controller-life-gain",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::MillTargetPlayerAndGainLifeControllerEqualToChosenX],
    }
}
"##;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongManaCost,
    FixedMillAndOneLife,
    WrongFixedMill,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    drain: Option<ObjectId>,
    drain_cast_index: Option<usize>,
    drain_resolved_index: Option<usize>,
    drain_graveyard_index: Option<usize>,
    cast_payment: Vec<Color>,
    chosen_x_paid: u8,
    milled_objects: Vec<ObjectId>,
    library_before: Option<usize>,
    library_after: Option<usize>,
    life_before: Option<i64>,
    life_after: Option<i64>,
    drain_zone: Option<Zone>,
}

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Psychic Drain PvP result: {error}");
                return ExitCode::FAILURE;
            }
            println!("{}", serde_json::to_string(&report).expect("report serializes"));
            if report["passed"].as_bool() == Some(true) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("Psychic Drain PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    let source_validation = validate_candidate_source()?;
    validate_candidate_definition()?;
    let catalog = unique_catalog()?;
    let (seed, reference) = find_trace()?;
    let trace = validate_trace(&reference, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if reference != replay || project_events(&reference.events) != project_events(&replay.events) {
        return Err("Psychic Drain full-game replay was not deterministic".to_owned());
    }

    let mutations = json!({
        "wrong_mana_cost_detected_in_game": mutation_detected(seed, Mutation::WrongManaCost),
        "fixed_mill_and_one_life_detected_in_game": mutation_detected(seed, Mutation::FixedMillAndOneLife),
        "wrong_fixed_mill_count_detected_in_game": mutation_detected(seed, Mutation::WrongFixedMill),
        "sealed_mutation_authority": false
    });
    for key in [
        "wrong_mana_cost_detected_in_game",
        "fixed_mill_and_one_life_detected_in_game",
        "wrong_fixed_mill_count_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!("Psychic Drain semantic mutation escaped: {key}"));
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
        "card":{"collector_number":220,"printings":1,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":["106: mana and X costs", "117: priority", "202.1-202.2: mana cost and color", "601.2: casting and choosing X", "608.2: resolving a spell", "701.13: milling", "119.1 and 119.5: life totals and life gain"],
            "source_contract":"RAV #220 is a blue-black sorcery with X in its mana cost; target player mills X cards and its controller gains X life, even when fewer than X cards are milled.",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true
        },
        "passed":cards_passed==1,
        "pilot_card_passed":cards_passed==1,
        "replay_deterministic":true,
        "cards_passed":cards_passed,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":"diagnostic_only; independent review and candidate trial pending",
        "candidate_source_validation":source_validation,
        "mutation_sensitivity":mutations,
        "traces":[trace],
        "per_card_breakdown":breakdown
    }))
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(seed) = std::env::var("CARDBENCH_PSYCHIC_DRAIN_SEED") {
        let seed = seed
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_PSYCHIC_DRAIN_SEED must be an unsigned integer".to_owned())?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    let mut last = "no seed attempted".to_owned();
    for seed in 0..=MAX_TRACE_SEED {
        match run_match(seed, Mutation::None)
            .and_then(|capture| validate_trace(&capture, seed).map(|_| capture))
        {
            Ok(capture) => return Ok((seed, capture)),
            Err(error) => last = format!("seed {seed}: {error}"),
        }
    }
    Err(format!("no Psychic Drain full-game trace found: {last}"))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation) {
        Err(_) => true,
        Ok(capture) => validate_trace(&capture, seed).is_err(),
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definition = candidate_psychic_drain::definition();
    match mutation {
        Mutation::None => {}
        Mutation::WrongManaCost => definition.mana_cost.generic += 1,
        Mutation::FixedMillAndOneLife => {
            definition.effects = vec![
                Effect::MillTargetPlayer { count: 2 },
                Effect::GainLifeController { amount: 1 },
            ];
        }
        Mutation::WrongFixedMill => {
            definition.effects = vec![
                Effect::MillTargetPlayer { count: 1 },
                Effect::GainLifeController { amount: 2 },
            ];
        }
    }
    let mut definitions = card_definitions();
    let definition_index = definitions
        .iter()
        .position(|card| card.id == CARD_ID)
        .ok_or("Psychic Drain is missing from the RAV registry")?;
    definitions[definition_index] = definition.clone();

    let mut game = Game::new_with_all_bindings_and_triggers(
        definitions.clone(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .map_err(|error| error.to_string())?;
    let (pilot_deck, opponent_deck) = decks();
    let deck_catalog: BTreeMap<_, _> = definitions
        .iter()
        .cloned()
        .map(|card| (card.id, card))
        .collect();
    let deck_rules = DeckRules {
        minimum_mainboard_size: 60,
        maximum_copies: 4,
        maximum_sideboard_size: 15,
    };
    for (label, deck) in [("pilot", &pilot_deck), ("opponent", &opponent_deck)] {
        deck.validate(&deck_catalog, deck_rules)
            .map_err(|error| format!("{label} constructed deck is illegal: {error}"))?;
        if deck.mainboard.iter().map(|entry| usize::from(entry.count)).sum::<usize>() != 60 {
            return Err(format!("{label} constructed deck is not exactly sixty cards"));
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_MOVES,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed).map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(PsychicDrainPolicy::new(PlayerId(0), definition.mana_cost, requires_x(&definition))),
        Box::new(PsychicDrainPolicy::new(PlayerId(1), definition.mana_cost, false)),
    ];
    let mut capture = CapturedMatch {
        events: Vec::new(),
        terminal: String::new(),
        winner: None,
        turns: 0,
        accepted_moves: 0,
        rejected_moves: 0,
        drain: None,
        drain_cast_index: None,
        drain_resolved_index: None,
        drain_graveyard_index: None,
        cast_payment: Vec::new(),
        chosen_x_paid: 0,
        milled_objects: Vec::new(),
        library_before: None,
        library_after: None,
        life_before: None,
        life_after: None,
        drain_zone: None,
    };
    while !game.is_game_over() {
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|error| error.to_string())?;
        if game.turn > MAX_TURNS || capture.accepted_moves >= MAX_MOVES {
            return Err(format!(
                "Psychic Drain match exceeded its run bound at turn {} / {:?}",
                game.turn, view.step
            ));
        }
        let policy = policies.get_mut(player.0).ok_or("policy seat missing")?;
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
        let is_drain_cast = matches!(&action,
            PolicyAction::CastWithPayment { request, .. }
                if game.card_definition(request.card).is_ok_and(|card| card.id == CARD_ID)
        );
        if is_drain_cast && capture.library_before.is_none() {
            capture.library_before = Some(
                game.player(PlayerId(1))
                    .map_err(|error| error.to_string())?
                    .library
                    .len(),
            );
            capture.life_before = Some(
                game.player(PlayerId(0))
                    .map_err(|error| error.to_string())?
                    .life,
            );
        }
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Psychic Drain policy action rejected at turn {} / {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        capture.accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        if capture.library_after.is_none()
            && game.event_log.iter().any(|event| {
                matches!(event, GameEvent::SpellResolved { card } if game.card_definition(*card).is_ok_and(|d| d.id == CARD_ID))
            })
        {
            capture.library_after = Some(
                game.player(PlayerId(1))
                    .map_err(|error| error.to_string())?
                    .library
                    .len(),
            );
            capture.life_after = Some(
                game.player(PlayerId(0))
                    .map_err(|error| error.to_string())?
                    .life,
            );
        }
    }

    capture.events = game.event_log.clone();
    capture.winner = game.winner().map(|player| player.0);
    capture.terminal = capture.winner.map_or_else(
        || "Draw".to_owned(),
        |seat| format!("Winner(PlayerId({seat}))"),
    );
    capture.turns = game.turn;
    capture.drain = capture.events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game.card_definition(*card).is_ok_and(|d| d.id == CARD_ID) => Some(*card),
        _ => None,
    });
    if let Some(drain) = capture.drain {
        capture.drain_cast_index = capture.events.iter().position(|event| {
            matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == drain)
        });
        capture.drain_resolved_index = capture.events.iter().position(|event| {
            matches!(event, GameEvent::SpellResolved { card } if *card == drain)
        });
        capture.drain_graveyard_index = capture.events.iter().position(|event| {
            matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == drain)
        });
        capture.cast_payment = capture.events.iter().find_map(|event| match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if *card == drain => Some(colors.clone()),
            _ => None,
        }).unwrap_or_default();
        let cast = capture.drain_cast_index.unwrap_or(0);
        let resolved = capture.drain_resolved_index.unwrap_or(cast);
        capture.milled_objects = capture.events[cast + 1..resolved]
            .iter()
            .filter_map(|event| match event {
                GameEvent::CardMoved { card, to: Zone::Graveyard }
                    if game.object(*card).is_ok_and(|object| object.owner == PlayerId(1)) => Some(*card),
                _ => None,
            })
            .collect();
        capture.chosen_x_paid = capture.milled_objects.len().min(u8::MAX as usize) as u8;
        capture.drain_zone = game.zone_of(drain);
    }
    Ok(capture)
}

fn decks() -> (DeckList, DeckList) {
    (deck(&[
        (CARD_ID, 4),
        ("RAV-DIMIR-INFILTRATOR", 4),
        ("RAV-SNAPPING-DRAKE", 4),
        ("RAV-SURVEILLING-SPRITE", 4),
        ("RAV-ISLAND", 24),
        ("RAV-SWAMP", 20),
    ]), deck(&[
        ("RAV-WATCHWOLF", 4),
        ("RAV-FOREST", 28),
        ("RAV-PLAINS", 28),
    ]))
}

fn deck(entries: &[(&str, u8)]) -> DeckList {
    DeckList {
        mainboard: entries
            .iter()
            .map(|(card, count)| DeckEntry {
                card: (*card).to_owned(),
                count: *count,
            })
            .collect(),
        sideboard: vec![],
    }
}

struct PsychicDrainPolicy {
    player: PlayerId,
    inner: RavCatalogPolicy,
    cost: ManaCost,
    requires_x: bool,
    drain_cast: bool,
}

impl PsychicDrainPolicy {
    fn new(player: PlayerId, cost: ManaCost, requires_x: bool) -> Self {
        Self {
            player,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
            cost,
            requires_x,
            drain_cast: false,
        }
    }
}

impl CodePolicy for PsychicDrainPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.psychic-drain-pilot.v1"
        } else {
            "rav.psychic-drain-opponent.v1"
        }
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareBlockers && !view.blockers_declared {
            return PolicyAction::DeclareBlockers { assignments: vec![] };
        }
        if view.step == Step::DeclareAttackers && !view.attackers_declared {
            if self.player == PlayerId(0) {
                return PolicyAction::DeclareAttackers {
                    attackers: view
                        .own_battlefield
                        .iter()
                        .filter(|card| card.can_attack)
                        .map(|card| card.id)
                        .collect(),
                };
            }
            return PolicyAction::DeclareAttackers { attackers: vec![] };
        }
        if self.player == PlayerId(0)
            && view.active_player == self.player
            && view.priority == self.player
            && view.step.is_main()
            && view.stack_depth == 0
            && !self.drain_cast
        {
            if let Some(card) = view.hand.iter().find(|card| card.definition == Some(CARD_ID)) {
                let chosen_x = self.requires_x.then_some(2);
                let mut paid_cost = self.cost.clone();
                if let Some(x) = chosen_x {
                    paid_cost.generic = paid_cost.generic.saturating_add(x);
                }
                if let Some(mana_selection) = payment(view, &paid_cost) {
                    self.drain_cast = true;
                    return PolicyAction::CastWithPayment {
                        request: CastRequest {
                            card: card.id,
                            targets: vec![Target::Player(PlayerId(1))],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x,
                        mana_selection,
                    };
                }
                if let Some(action) = make_mana(view, &paid_cost) {
                    return action;
                }
            }
        }
        self.inner.propose_move(view)
    }
}

fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<_> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let amount = &mut available.iter_mut().find(|(candidate, _)| candidate == color)?.1;
        if *amount == 0 {
            return None;
        }
        *amount -= 1;
    }
    if !cost.hybrid.is_empty() {
        return None;
    }
    let mut generic = Vec::new();
    for _ in 0..cost.generic {
        let index = available.iter().position(|(_, amount)| *amount > 0)?;
        let (color, amount) = &mut available[index];
        *amount -= 1;
        generic.push(*color);
    }
    Some(ManaPaymentSelection { generic, hybrid: vec![] })
}

fn make_mana(view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
    let mut required = cost.colored.clone();
    required.extend(cost.hybrid.iter().map(|symbol| symbol.first));
    if let Some(color) = required.iter().find(|color| {
        view.mana_pool.amount(**color)
            < required.iter().filter(|other| *other == *color).count() as u8
    }) {
        return land_for(view, Some(*color));
    }
    if view.mana_pool.total() < cost.generic + required.len() as u8 {
        return land_for(view, None);
    }
    None
}

fn land_for(view: &GameView, desired: Option<Color>) -> Option<PolicyAction> {
    let land = view
        .own_battlefield
        .iter()
        .find(|card| {
            card.card_types.contains(&CardType::Land)
                && !card.tapped
                && desired.is_none_or(|color| card.mana_colors.contains(color))
        })
        .or_else(|| {
            view.own_battlefield
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land) && !card.tapped)
        })?;
    let color = desired
        .filter(|color| land.mana_colors.contains(color))
        .or_else(|| land.mana_colors.iter().next().copied())?;
    Some(PolicyAction::ActivateManaAbility { land: land.id, color })
}

fn requires_x(definition: &CardDefinition) -> bool {
    definition.effects.iter().any(Effect::requires_chosen_x)
}

fn validate_trace(capture: &CapturedMatch, seed: u64) -> Result<Value, String> {
    let drain = capture.drain.ok_or("Psychic Drain was never cast")?;
    let cast = capture.drain_cast_index.ok_or("Psychic Drain cast receipt missing")?;
    let resolved = capture
        .drain_resolved_index
        .ok_or("Psychic Drain resolve receipt missing")?;
    let spell_graveyard = capture
        .drain_graveyard_index
        .ok_or("Psychic Drain spell did not reach its terminal graveyard zone")?;
    let before = capture.library_before.ok_or("target library before cast missing")?;
    let after = capture.library_after.ok_or("target library after resolution missing")?;
    let life_before = capture.life_before.ok_or("controller life before cast missing")?;
    let life_after = capture.life_after.ok_or("controller life after resolution missing")?;
    if capture.terminal != "Winner(PlayerId(0))"
        || capture.winner != Some(0)
        || capture.accepted_moves == 0
        || capture.rejected_moves != 0
        || capture.chosen_x_paid != 2
        || capture.milled_objects.len() != 2
        || before.saturating_sub(after) != 2
        || life_after - life_before != 2
        || capture.drain_zone != Some(Zone::Graveyard)
        || capture.cast_payment.len() != 4
        || !capture.cast_payment.contains(&Color::Blue)
        || !capture.cast_payment.contains(&Color::Black)
        || !(cast < resolved && resolved < spell_graveyard)
    {
        return Err(format!(
            "Psychic Drain X=2 trace differs: winner={:?} x={} milled={} library={before}->{after} life={life_before}->{life_after} payment={:?}",
            capture.winner,
            capture.chosen_x_paid,
            capture.milled_objects.len(),
            capture.cast_payment
        ));
    }
    let mana_paid = event_index(&capture.events, |event| {
        matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, .. } if *card == drain)
    })?;
    let mill_indices = capture
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if capture.milled_objects.contains(card))
                .then_some(index)
        })
        .collect::<Vec<_>>();
    let life_gain = event_index_after(&capture.events, cast, |event| {
        matches!(event, GameEvent::LifeGained { player: PlayerId(0), amount: 2 })
    })?;
    let game_end = event_index(&capture.events, |event| {
        matches!(event, GameEvent::GameEnded { winner: Some(PlayerId(0)) })
    })?;
    if !(mana_paid < cast
        && cast < mill_indices[0]
        && mill_indices[0] < mill_indices[1]
        && mill_indices[1] < life_gain
        && life_gain < resolved
        && resolved < spell_graveyard
        && spell_graveyard < game_end)
    {
        return Err("Psychic Drain ordered cast, mill, life, resolve, and terminal receipts differ".to_owned());
    }
    let projected = project_events(&capture.events);
    let event_kinds = projected
        .iter()
        .map(|event| format!("{:?}", event.kind))
        .collect::<Vec<_>>();
    Ok(json!({
        "id":"rav-psychic-drain-chosen-x-mill-life-gain-full-game",
        "seed":seed,
        "passed":true,
        "terminal":capture.terminal,
        "winner_seat":capture.winner,
        "turns":capture.turns,
        "accepted_moves":capture.accepted_moves,
        "rejected_moves":capture.rejected_moves,
        "mechanics":{
            "definition_id":CARD_ID,
            "chosen_x":2,
            "mana_cost":"{X}{U}{B}",
            "paid_mana_colors":capture.cast_payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),
            "target_player_seat":1,
            "target_library_size_before":before,
            "target_library_size_after":after,
            "milled_card_count":capture.milled_objects.len(),
            "controller_life_before":life_before,
            "controller_life_after":life_after,
            "spell_zone_after_resolution":format!("{:?}",capture.drain_zone),
            "event_order_indices":{"mana_paid":mana_paid,"cast":cast,"mill_first":mill_indices[0],"mill_second":mill_indices[1],"life_gain":life_gain,"spell_resolved":resolved,"spell_to_graveyard":spell_graveyard,"game_end":game_end},
            "projected_event_kinds":event_kinds,
            "terminal_pilot_win":capture.winner==Some(0)
        },
        "ordered_event_sequence":[mana_paid,cast,mill_indices[0],mill_indices[1],life_gain,resolved,spell_graveyard,game_end],
        "events":projected
    }))
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Result<usize, String> {
    events.iter().position(predicate).ok_or_else(|| "required Psychic Drain event is absent".to_owned())
}

fn event_index_after(
    events: &[GameEvent],
    after: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Result<usize, String> {
    events.iter().enumerate().skip(after + 1).find_map(|(index, event)| predicate(event).then_some(index))
        .ok_or_else(|| "ordered Psychic Drain event is absent".to_owned())
}

fn validate_candidate_source() -> Result<Value, String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Psychic Drain candidate: {error}"))?;
    validate_candidate_source_text(&source)?;
    let injection = source.replacen(
        "pub(crate) fn definition() -> CardDefinition {",
        "pub(crate) fn definition() -> CardDefinition { let _ = std::fs::write(\"/tmp/psychic-drain-side-effect\", b\"executed\");",
        1,
    );
    if injection == source || syn::parse_file(&injection).is_err() {
        return Err("a compile-valid Psychic Drain side-effect fixture could not be constructed".to_owned());
    }
    let rejected = validate_candidate_source_text(&injection).is_err();
    if !rejected {
        return Err("Psychic Drain source guard accepted an injected executable statement".to_owned());
    }
    Ok(json!({
        "normalized_literal_source_checked_before_candidate_calls":true,
        "direct_definition_ast_checked_before_candidate_calls":true,
        "compile_valid_injected_side_effect_fixture_parsed":true,
        "compile_valid_injected_side_effect_rejected":rejected
    }))
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let file = syn::parse_file(source)
        .map_err(|_| "Psychic Drain source is not valid declarative Rust".to_owned())?;
    if normalize_source(source) != normalize_source(PINNED_SOURCE)
        || !file.attrs.iter().all(|attribute| attribute.path().is_ident("doc"))
        || file.items.len() != 3
    {
        return Err("Psychic Drain source differs from the pinned normalized module".to_owned());
    }
    let mut definition_seen = false;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(item.vis, Visibility::Inherited) => {}
            Item::Fn(function)
                if !definition_seen
                    && function.sig.ident == "definition"
                    && matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    && function.sig.inputs.is_empty()
                    && function.sig.constness.is_none()
                    && function.sig.asyncness.is_none()
                    && function.sig.unsafety.is_none()
                    && function.sig.generics.params.is_empty()
                    && function.sig.generics.where_clause.is_none()
                    && function.block.stmts.len() == 1
                    && matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(literal), None) if literal.path.is_ident("CardDefinition") && literal.rest.is_none()) =>
            {
                definition_seen = true;
            }
            _ => return Err("Psychic Drain source contains executable top-level or function code".to_owned()),
        }
    }
    if !definition_seen {
        return Err("Psychic Drain definition() direct literal is missing".to_owned());
    }
    Ok(())
}

fn normalize_source(source: &str) -> String {
    source.lines().map(str::trim).collect::<Vec<_>>().join("\n")
}

fn validate_candidate_definition() -> Result<(), String> {
    let definition = candidate_psychic_drain::definition();
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(0, [Color::Blue, Color::Black])
        || definition.colors != BTreeSet::from([Color::Blue, Color::Black])
        || definition.card_types != BTreeSet::from([CardType::Sorcery])
        || definition.supported_rules
            != ["full-rules-fidelity", "chosen-x-targeted-mill-and-controller-life-gain"]
        || definition.effects != [Effect::MillTargetPlayerAndGainLifeControllerEqualToChosenX]
        || definition.power.is_some()
        || definition.toughness.is_some()
    {
        return Err("Psychic Drain candidate definition differs from RAV #220 semantics".to_owned());
    }
    if candidate_kind() == "reference" {
        let registered = card_definitions()
            .into_iter()
            .find(|card| card.id == CARD_ID)
            .ok_or("Psychic Drain is missing from the set registry")?;
        if registered != definition {
            return Err("Psychic Drain registry definition does not delegate to the candidate module".to_owned());
        }
    }
    Ok(())
}

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect::<Vec<_>>();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
    {
        return Err("RAV identity or printing denominator differs from the source contract".to_owned());
    }
    Ok(catalog)
}

fn card_status(card: &CatalogCard) -> Value {
    let passed = card.name == CARD_NAME;
    json!({
        "collector_number":card.collector_number,
        "name":card.name,
        "definition_id":match card.semantic_status { cardbench_magic_rav::CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id), cardbench_magic_rav::CardSemanticStatus::CatalogOnly { .. } => None },
        "trace_status":if passed{"passed"}else{"not_traced_in_this_single_card_task"},
        "passed":passed,
        "credited":false
    })
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "candidate_card_module"
    } else {
        "reference_card_module"
    }
}

fn output_root() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        if argument == "--output-root" {
            return args.next().map(std::path::PathBuf::from);
        }
    }
    None
}

fn write_outputs(root: &std::path::Path, report: &Value) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    std::fs::write(root.join("result.json"), serde_json::to_vec_pretty(report).expect("report serializes"))?;
    std::fs::write(root.join("reward.txt"), "0.0\n")
}
