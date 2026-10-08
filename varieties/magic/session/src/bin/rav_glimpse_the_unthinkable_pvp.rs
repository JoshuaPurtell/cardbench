//! Full-game RAV Glimpse the Unthinkable targeted-mill authority.

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
const CARD_ID: &str = "RAV-GLIMPSE-THE-UNTHINKABLE";
const MOROII_ID: &str = "RAV-MOROII";
const ABILITY_TARGET: PlayerId = PlayerId(1);
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/glimpse_the_unthinkable.rs";
const DECK_P0: &str = "rav_glimpse_dimir_mill_pilot_60";
const DECK_P1: &str = "rav_basic_plains_opponent_60";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/glimpse_the_unthinkable.rs"]
mod candidate_glimpse_the_unthinkable;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Glimpse the Unthinkable result: {error}");
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
            eprintln!("Glimpse the Unthinkable PvP authority failed: {error}");
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
        return Err(
            "Glimpse the Unthinkable shuffled-game replay was not deterministic".to_owned(),
        );
    }
    let mutations = mutation_sensitivity(seed)?;
    for key in [
        "nine_card_mill_mutation_detected_in_game",
        "eleven_card_mill_mutation_detected_in_game",
        "wrong_target_mutation_detected_in_game",
        "wrong_cast_cost_mutation_detected_in_game",
        "wrong_color_identity_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Glimpse semantic mutation escaped the PvP trace: {key}"
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
        "card":{"collector_number":208,"printings":1,"definition_id":CARD_ID,"name":"Glimpse the Unthinkable"},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":[
                "105.2 and 202.1-202.2: blue-black color identity and mana cost",
                "117: priority and spell resolution timing",
                "301 and 304: artifacts and sorceries as appropriate to support objects",
                "601.2: casting a sorcery and paying its costs",
                "608.2: resolving the targeted spell instruction",
                "701.13: mill ten cards from a player's library into their graveyard",
                "400.7: cards that change zones become new game objects"
            ],
            "source_contract":"RAV Glimpse the Unthinkable #208, {U}{B} sorcery: target player mills ten cards",
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
    MillNine,
    MillEleven,
    WrongTarget,
    WrongCastCost,
    WrongColorIdentity,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    glimpse: Option<ObjectId>,
    moroii: Option<ObjectId>,
    target: Option<Target>,
    spell_payment: Vec<Color>,
    source_colors: Vec<Color>,
    mill_objects: Vec<ObjectId>,
    mill_library_before: Option<usize>,
    mill_library_after: Option<usize>,
    moroii_attacks: Vec<u32>,
    moroii_damage: i32,
    life: [i64; 2],
}

fn same_match(left: &Captured, right: &Captured) -> bool {
    left.events == right.events
        && left.terminal == right.terminal
        && left.winner == right.winner
        && left.turns == right.turns
        && left.glimpse == right.glimpse
        && left.moroii == right.moroii
        && left.target == right.target
        && left.spell_payment == right.spell_payment
        && left.source_colors == right.source_colors
        && left.mill_objects == right.mill_objects
        && left.mill_library_before == right.mill_library_before
        && left.mill_library_after == right.mill_library_after
        && left.moroii_attacks == right.moroii_attacks
        && left.moroii_damage == right.moroii_damage
        && left.life == right.life
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_GLIMPSE_THE_UNTHINKABLE_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_GLIMPSE_THE_UNTHINKABLE_SEED must be an unsigned integer")?;
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
        "no bounded seed produced the Glimpse targeted-mill full-game trace: {last_error}"
    ))
}

fn mutation_sensitivity(seed: u64) -> Result<Value, String> {
    let mut outcomes = serde_json::Map::new();
    for (name, mutation) in [
        (
            "nine_card_mill_mutation_detected_in_game",
            Mutation::MillNine,
        ),
        (
            "eleven_card_mill_mutation_detected_in_game",
            Mutation::MillEleven,
        ),
        (
            "wrong_target_mutation_detected_in_game",
            Mutation::WrongTarget,
        ),
        (
            "wrong_cast_cost_mutation_detected_in_game",
            Mutation::WrongCastCost,
        ),
        (
            "wrong_color_identity_mutation_detected_in_game",
            Mutation::WrongColorIdentity,
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
                card: MOROII_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-ISLAND".to_owned(),
                count: 26,
            },
            DeckEntry {
                card: "RAV-SWAMP".to_owned(),
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
    let mut candidate = candidate_glimpse_the_unthinkable::definition();
    match mutation {
        Mutation::None => {}
        Mutation::MillNine => candidate.effects = vec![Effect::MillTargetPlayer { count: 9 }],
        Mutation::MillEleven => candidate.effects = vec![Effect::MillTargetPlayer { count: 11 }],
        Mutation::WrongTarget => {
            candidate.effects = vec![Effect::MillCapturedPlayer {
                player: PlayerId(0),
                count: 10,
            }]
        }
        Mutation::WrongCastCost => candidate.mana_cost = ManaCost::with_colors(1, [Color::Blue]),
        Mutation::WrongColorIdentity => candidate.colors = BTreeSet::from([Color::Blue]),
    }
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Glimpse the Unthinkable is absent from the RAV registry")? = candidate;
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

    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(GlimpsePolicy::new(PlayerId(0))),
        Box::new(GlimpsePolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut glimpse = None;
    let mut target = None;
    let mut spell_payment = Vec::new();
    let mut source_colors = Vec::new();
    let mut mill_objects = Vec::new();
    let mut mill_library_before = None;
    let mut mill_library_after = None;
    let mut moroii_attacks = Vec::new();
    let mut moroii_damage = 0_i32;

    while !game.is_game_over() {
        if game.turn > 45 || accepted >= 3500 {
            return Err(format!(
                "Glimpse match exceeded its turn/action bound at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
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
        let before_library: BTreeSet<ObjectId> = game
            .player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .library
            .iter()
            .copied()
            .collect();
        let before_library_count = before_library.len();
        let cursor = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
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
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    glimpse = Some(*card);
                    if let Some(stack) = game.stack.iter().find(|stack| stack.card == *card) {
                        target = stack.targets.first().copied();
                        source_colors = stack.source_colors.iter().copied().collect();
                    }
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if glimpse == Some(*card) => spell_payment = colors.clone(),
                GameEvent::AttackersDeclared {
                    player: PlayerId(0),
                    attackers,
                } => {
                    if attackers.iter().any(|attacker| {
                        game.card_definition(*attacker)
                            .is_ok_and(|definition| definition.id == MOROII_ID)
                    }) {
                        moroii_attacks.push(game.turn);
                    }
                }
                GameEvent::DamageDealtToPlayer {
                    source,
                    player: PlayerId(1),
                    amount,
                } if game
                    .card_definition(*source)
                    .is_ok_and(|definition| definition.id == MOROII_ID) =>
                {
                    moroii_damage += *amount
                }
                _ => {}
            }
        }
        if let Some(source) = new_events.iter().find_map(|event| match event {
            GameEvent::SpellResolved { card } if glimpse == Some(*card) => Some(*card),
            _ => None,
        }) {
            let _ = source;
            mill_library_before = Some(before_library_count);
            mill_library_after = Some(
                game.player(PlayerId(1))
                    .map_err(|error| error.to_string())?
                    .library
                    .len(),
            );
            mill_objects = new_events
                .iter()
                .filter_map(|event| match event {
                    GameEvent::CardMoved {
                        card,
                        to: Zone::Graveyard,
                    } if before_library.contains(card) => Some(*card),
                    _ => None,
                })
                .collect();
        }
    }

    let events = game.event_log.clone();
    let glimpse = events.iter().find_map(|event| match event {
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
    let moroii = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|definition| definition.id == MOROII_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let spell_payment = glimpse
        .and_then(|spell| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if *card == spell => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or(spell_payment);
    let source_colors = glimpse
        .and_then(|spell| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if *card == spell => game
                    .card_definition(*card)
                    .ok()
                    .map(|definition| definition.colors.iter().copied().collect()),
                _ => None,
            })
        })
        .unwrap_or(source_colors);
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
        glimpse,
        moroii,
        target,
        spell_payment,
        source_colors,
        mill_objects,
        mill_library_before,
        mill_library_after,
        moroii_attacks,
        moroii_damage,
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

struct GlimpsePolicy {
    player: PlayerId,
    glimpse_submitted: bool,
    moroii_submitted: bool,
}

impl GlimpsePolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            glimpse_submitted: false,
            moroii_submitted: false,
        }
    }
}

impl CodePolicy for GlimpsePolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.glimpse-targeted-mill.v1"
        } else {
            "rav.plains-pass.v1"
        }
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = view
                .own_battlefield
                .iter()
                .filter(|card| card.definition == Some(MOROII_ID) && card.can_attack)
                .map(|card| card.id)
                .collect();
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
        if view.decision_player != self.player
            || view.priority != self.player
            || view.active_player != self.player
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
        if self.player == PlayerId(1) {
            return PolicyAction::PassPriority;
        }

        if !self.glimpse_submitted {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            {
                let cost = ManaCost::with_colors(0, [Color::Blue, Color::Black]);
                if let Some(selection) = payment(view, &cost) {
                    self.glimpse_submitted = true;
                    return PolicyAction::CastWithPayment {
                        request: CastRequest {
                            card: card.id,
                            targets: vec![Target::Player(ABILITY_TARGET)],
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

        if !self.moroii_submitted
            && !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(MOROII_ID))
        {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(MOROII_ID))
            {
                let cost = ManaCost::with_colors(2, [Color::Blue, Color::Black]);
                if let Some(selection) = payment(view, &cost) {
                    self.moroii_submitted = true;
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
        need = cost.colored.first().copied().or(Some(Color::Blue));
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
    let glimpse = c.glimpse.ok_or("Glimpse the Unthinkable was not cast")?;
    let moroii = c.moroii.ok_or("the support Moroii was not cast")?;
    if c.target != Some(Target::Player(PlayerId(1))) {
        return Err("Glimpse did not retain opponent PlayerId(1) as its stack target".to_owned());
    }
    if c.spell_payment.len() != 2
        || !c.spell_payment.contains(&Color::Blue)
        || !c.spell_payment.contains(&Color::Black)
    {
        return Err(format!(
            "Glimpse did not pay exact {{U}}{{B}}: {:?}",
            c.spell_payment
        ));
    }
    if c.source_colors.iter().copied().collect::<BTreeSet<_>>()
        != BTreeSet::from([Color::Blue, Color::Black])
    {
        return Err(format!(
            "Glimpse color identity is wrong: {:?}",
            c.source_colors
        ));
    }
    if c.mill_objects.len() != 10 || c.mill_objects.iter().collect::<BTreeSet<_>>().len() != 10 {
        return Err(format!(
            "Glimpse moved {} unique opponent library cards; expected exactly ten",
            c.mill_objects.len()
        ));
    }
    let before = c
        .mill_library_before
        .ok_or("opponent library before Glimpse resolution was not recorded")?;
    let after = c
        .mill_library_after
        .ok_or("opponent library after Glimpse resolution was not recorded")?;
    if before.saturating_sub(after) != 10 {
        return Err(format!(
            "opponent library changed by {}; expected exactly ten",
            before.saturating_sub(after)
        ));
    }
    if c.terminal != "Winner(PlayerId(0))"
        || c.winner != Some(0)
        || c.life[0] <= 0
        || c.life[1] > 0
        || c.rejected != 0
    {
        return Err(format!(
            "Glimpse PvP match did not finish cleanly for seat zero: {:?}",
            c.life
        ));
    }
    if c.moroii_attacks.len() < 3 || c.moroii_damage < 12 {
        return Err("the full-game support creature did not complete repeated combat".to_owned());
    }

    let paid = event_index(&c.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == glimpse && colors.len() == 2 && colors.contains(&Color::Blue) && colors.contains(&Color::Black))).ok_or("Glimpse blue-black mana payment receipt missing")?;
    let cast = event_index_after(&c.events, paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == glimpse)).ok_or("Glimpse cast receipt missing")?;
    // The engine emits each effect's zone change before the terminal spell
    // receipt, then moves Glimpse from the stack to its owner's graveyard.
    let first_mill = event_index_after(&c.events, cast, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if c.mill_objects.contains(card))).ok_or("first targeted library-to-graveyard move missing")?;
    let last_mill = c.mill_objects.iter().filter_map(|object| event_index_after(&c.events, cast, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if card == object))).max().ok_or("last targeted library-to-graveyard move missing")?;
    let resolved = event_index_after(
        &c.events,
        last_mill,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == glimpse),
    )
    .ok_or("Glimpse resolution receipt missing after its mill instructions")?;
    let spell_graveyard = event_index_after(&c.events, resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == glimpse)).ok_or("resolved Glimpse did not reach its owner's graveyard after milling")?;
    let moroii_cast = event_index_after(&c.events, spell_graveyard, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == moroii)).ok_or("Moroii support spell was not cast after Glimpse resolved")?;
    let moroii_resolved = event_index_after(
        &c.events,
        moroii_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == moroii),
    )
    .ok_or("Moroii did not resolve")?;
    let attack = event_index_after(&c.events, moroii_resolved, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&moroii))).ok_or("Moroii did not attack")?;
    let damage = event_index_after(&c.events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 4 } if *source == moroii)).ok_or("Moroii did not deal combat damage to seat one")?;
    let ended = event_index_after(&c.events, damage, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("seat-zero terminal game event is missing")?;
    if !(paid < cast
        && cast < first_mill
        && first_mill <= last_mill
        && last_mill < resolved
        && resolved < spell_graveyard
        && spell_graveyard < moroii_cast
        && moroii_cast < moroii_resolved
        && moroii_resolved < attack
        && attack < damage
        && damage < ended)
    {
        return Err("Glimpse payment, target resolution, mill, support-cast, and combat event order is invalid".to_owned());
    }

    let projected = project_events(&c.events);
    let events: Vec<Value> = projected.iter().map(|event| json!({"sequence":event.sequence,"turn":event.turn,"kind":&event.kind,"canonical":&event.canonical,"facts":&event.facts})).collect();
    Ok(json!({
        "trace_id":"rav-glimpse-the-unthinkable-targeted-mill-full-game",
        "passed":true,
        "shuffle_seed":seed,
        "decks":[DECK_P0,DECK_P1],
        "terminal":c.terminal,
        "winner_seat":c.winner,
        "turns":c.turns,
        "accepted_moves":c.accepted,
        "rejected_moves":c.rejected,
        "glimpse":{"object":glimpse.0,"target":"PlayerId(1)","mana_cost":"{U}{B}","mana_paid":c.spell_payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"source_colors":c.source_colors.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"library_cards_milled":10,"milled_objects":c.mill_objects.iter().map(|object|object.0).collect::<Vec<_>>(),"opponent_library_before":before,"opponent_library_after":after},
        "combat":{"support_card":"RAV-MOROII","object":moroii.0,"attack_turns":c.moroii_attacks,"damage_to_opponent":c.moroii_damage,"life_at_terminal":c.life},
        "ordered_event_sequence":[paid,cast,first_mill,last_mill,resolved,spell_graveyard,moroii_cast,moroii_resolved,attack,damage,ended],
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
    let candidate = candidate_glimpse_the_unthinkable::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Glimpse the Unthinkable definition is missing from the RAV registry")?;
    if registered != candidate {
        return Err(
            "Glimpse the Unthinkable registry definition does not delegate to its candidate module"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Glimpse source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Glimpse source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Glimpse module has an unsupported module attribute".to_owned());
    }
    let mut imports = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if matches!(item.vis, Visibility::Inherited) && item.attrs.is_empty() =>
            {
                imports
                    .extend(flattened_use_paths(&item.tree).ok_or("Glimpse import shape differs")?);
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
                        "Glimpse module must expose only one safe no-argument candidate function"
                            .to_owned(),
                    );
                }
                let Stmt::Expr(Expr::Struct(literal), None) = &function.block.stmts[0] else {
                    return Err("Glimpse definition must be a direct struct literal".to_owned());
                };
                if literal.rest.is_some() || !declarative_expr(&Expr::Struct(literal.clone())) {
                    return Err("Glimpse source contains unsupported executable code".to_owned());
                }
            }
            _ => {
                return Err(
                    "Glimpse module permits only private imports and its definition".to_owned(),
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
        return Err("Glimpse definition imports or function set differs".to_owned());
    }
    let definition = candidate_glimpse_the_unthinkable::definition();
    if definition.id != CARD_ID
        || definition.name != "Glimpse the Unthinkable"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(0, [Color::Blue, Color::Black])
        || definition.colors != BTreeSet::from([Color::Blue, Color::Black])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Sorcery])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
        || definition.effects != [Effect::MillTargetPlayer { count: 10 }]
    {
        return Err("Glimpse identity or exact targeted-mill-ten definition differs".to_owned());
    }
    Ok(())
}

fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc")
        && matches!(&attribute.meta, Meta::NameValue(value) if matches!(&value.value, Expr::Lit(literal) if matches!(literal.lit, Lit::Str(_))))
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
            (matches!(names.as_slice(),[ty,method] if (ty=="ManaCost" && method=="with_colors") || (ty=="BTreeSet" && (method=="from" || method=="new")))
                || matches!(names.as_slice(),[one] if one=="Some"))
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
