//! Full-game RAV Leashling authority with resolution and response traces.

use cardbench_magic_engine::{
    AbilityActivation, AbilityCostPayment, CardType, CastRequest, Color, DeckEntry, DeckList, Game,
    GameEvent, GameView, GeneralizedAbilityActivation, ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, Step, Zone,
};
use cardbench_magic_policies::{CatalogPolicyProfile, CodePolicy, RavCatalogPolicy};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings,
    rav_generalized_activated_ability_cost_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-LEASHLING";
const CARD_NAME: &str = "Leashling";
const ABILITY_ID: &str = "hand-card-library-top-return-source";
const LAST_GASP_ID: &str = "RAV-LAST-GASP";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/leashling.rs";
const MAX_TRACE_SEED: u64 = 32;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/leashling.rs"]
mod candidate_leashling;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoHandCardCost,
    NoReturnEffect,
    WrongPower,
}

#[derive(Clone, Debug)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    source: Option<ObjectId>,
    payment_card: Option<ObjectId>,
    mana_paid: Vec<Color>,
    power: Option<i32>,
    attacks: u32,
    combat_damage: i64,
    cost_paid: bool,
    payment_card_on_top: bool,
    ability_resolved: bool,
    source_returned: bool,
    response_cast: bool,
    response_spell: Option<ObjectId>,
    response_resolved: bool,
    source_in_graveyard_after_response: bool,
    life: [i64; 2],
}

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Leashling result: {error}");
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
            eprintln!("Leashling PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_definition()?;
    let catalog = unique_catalog()?;
    let (seed, resolution) = find_trace(false)?;
    let (response_seed, response) = find_trace(true)?;
    let resolution_trace = validate_trace(&resolution, seed, false)?;
    let response_trace = validate_trace(&response, response_seed, true)?;
    let replay = run_match(seed, false, Mutation::None)?;
    let response_replay = run_match(response_seed, true, Mutation::None)?;
    let replay_deterministic =
        same_match(&resolution, &replay) && same_match(&response, &response_replay);
    if !replay_deterministic {
        return Err("Leashling shuffled-game replay was not deterministic".to_owned());
    }
    let mutation_sensitivity = json!({
        "hand_card_cost_removed_mutation_detected": mutation_detected(seed, Mutation::NoHandCardCost),
        "source_return_removed_mutation_detected": mutation_detected(seed, Mutation::NoReturnEffect),
        "creature_power_changed_mutation_detected": mutation_detected(seed, Mutation::WrongPower),
        "sealed_mutation_authority": false
    });
    for key in [
        "hand_card_cost_removed_mutation_detected",
        "source_return_removed_mutation_detected",
        "creature_power_changed_mutation_detected",
    ] {
        if mutation_sensitivity[key] != true {
            return Err(format!(
                "Leashling mutation escaped the full-game oracle: {key}"
            ));
        }
    }
    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 265, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "601.2 Casting spells and paying costs",
                "602.1-602.2 Activated abilities, costs, and priority",
                "608.2 Resolving abilities and source-relative effects",
                "701.17 Putting a card on top of its owner's library",
                "701.26 Returning a permanent to its owner's hand",
                "117 Priority and response windows",
                "510 Combat damage and terminal game state"
            ],
            "source_contract": "RAV Leashling #265: colorless 6-mana artifact creature, 3/3; one owned hand card is put on top of its owner's library as an activation cost, then the source returns to its owner's hand on resolution",
            "authority_cutoff": "2026-09-23",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "replay_deterministic": replay_deterministic,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "reference_pilot_pass; independent review and candidate trial remain",
        "mutation_sensitivity": mutation_sensitivity,
        "traces": [resolution_trace, response_trace],
        "per_card_breakdown": breakdown
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

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Leashling candidate source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Leashling candidate source is not valid Rust".to_owned())?;
    let mut functions = BTreeSet::new();
    for item in file.items {
        let Item::Fn(function) = item else {
            if matches!(item, Item::Use(_)) {
                continue;
            }
            return Err(
                "Leashling source contains unsupported executable top-level code".to_owned(),
            );
        };
        let name = function.sig.ident.to_string();
        if !functions.insert(name.clone())
            || !matches!(function.vis, Visibility::Restricted(ref visibility) if visibility.path.is_ident("crate"))
            || !function.sig.inputs.is_empty()
            || function.block.stmts.len() != 1
            || !matches!(
                function.block.stmts.first(),
                Some(Stmt::Expr(Expr::Struct(_), None))
            )
        {
            return Err(
                "Leashling functions must return direct, crate-visible card-data literals"
                    .to_owned(),
            );
        }
    }
    if functions
        != BTreeSet::from([
            "definition".to_owned(),
            "activated_ability".to_owned(),
            "activated_ability_cost".to_owned(),
        ])
    {
        return Err(
            "Leashling source is missing a candidate-owned definition, ability, or cost".to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_definition() -> Result<(), String> {
    let definition = candidate_leashling::definition();
    let ability = candidate_leashling::activated_ability();
    let cost = candidate_leashling::activated_ability_cost();
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.mana_cost != ManaCost::new(6)
        || definition.card_types != BTreeSet::from([CardType::Artifact, CardType::Creature])
        || (definition.power, definition.toughness) != (Some(3), Some(3))
        || ability.card_definition != CARD_ID
        || ability.ability.id != ABILITY_ID
        || ability.ability.effects.len() != 1
        || cost.card_definition != CARD_ID
        || cost.ability_id != ABILITY_ID
        || cost.cost.put_hand_cards_on_library_top != 1
    {
        return Err(
            "Leashling definition, hand-card cost, or owner-hand return behavior differs"
                .to_owned(),
        );
    }
    Ok(())
}

fn find_trace(response: bool) -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_LEASHLING_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_LEASHLING_SEED must be unsigned".to_owned())?;
        let capture = run_match(seed, response, Mutation::None)?;
        validate_trace(&capture, seed, response)?;
        return Ok((seed, capture));
    }
    let mut last_failure = String::from("no seeds attempted");
    for seed in 0..=MAX_TRACE_SEED {
        match run_match(seed, response, Mutation::None) {
            Ok(capture) => match validate_trace(&capture, seed, response) {
                Ok(_) => return Ok((seed, capture)),
                Err(error) => last_failure = format!("seed {seed}: {error}"),
            },
            Err(error) => last_failure = format!("seed {seed}: {error}"),
        }
    }
    Err(format!(
        "no Leashling {} trace found for seeds 0..={MAX_TRACE_SEED}; last failure: {last_failure}",
        if response { "response" } else { "resolution" },
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, false, mutation) {
        Ok(capture) => validate_trace(&capture, seed, false).is_err(),
        Err(_) => true,
    }
}

fn same_match(left: &CapturedMatch, right: &CapturedMatch) -> bool {
    left.events == right.events
        && left.winner == right.winner
        && left.source == right.source
        && left.payment_card == right.payment_card
        && left.mana_paid == right.mana_paid
        && left.life == right.life
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

fn run_match(seed: u64, response: bool, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut leashling = candidate_leashling::definition();
    let mut ability = candidate_leashling::activated_ability();
    let mut cost = candidate_leashling::activated_ability_cost();
    match mutation {
        Mutation::None => {}
        Mutation::NoHandCardCost => cost.cost.put_hand_cards_on_library_top = 0,
        Mutation::NoReturnEffect => ability.ability.effects.clear(),
        Mutation::WrongPower => leashling.power = Some(2),
    }
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Leashling registry entry missing")? = leashling;
    let mut abilities = rav_activated_ability_bindings();
    abilities.retain(|binding| binding.card_definition != CARD_ID);
    abilities.push(ability);
    let mut costs = rav_generalized_activated_ability_cost_bindings();
    costs.retain(|binding| binding.card_definition != CARD_ID);
    costs.push(cost);
    let mut game = Game::new_with_all_bindings(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
    )
    .map_err(|error| error.to_string())?;
    game.register_generalized_activated_ability_cost_bindings(costs)
        .map_err(|error| error.to_string())?;

    let pilot_deck = deck(&[
        (CARD_ID, 4),
        ("RAV-GOBLIN-FIRE-FIEND", 4),
        ("RAV-MOUNTAIN", 52),
    ]);
    let opponent_deck = if response {
        deck(&[(LAST_GASP_ID, 4), ("RAV-SWAMP", 56)])
    } else {
        deck(&[("RAV-PLAINS", 60)])
    };
    let card_count = |deck: &DeckList| {
        deck.mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
    };
    if card_count(&pilot_deck) != 60 || card_count(&opponent_deck) != 60 {
        return Err("Leashling trace deck is not a legal 60-card mainboard".to_owned());
    }
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), 7)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), 7)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(LeashlingPolicy::new(PlayerId(0))),
        if response {
            Box::new(LeashlingResponsePolicy::new(PlayerId(1)))
        } else {
            Box::new(RavCatalogPolicy::new(
                PlayerId(1),
                CatalogPolicyProfile::Curve,
            ))
        },
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut source = None;
    let mut payment_card = None;
    let mut mana_paid = Vec::new();
    let mut power = None;
    let mut attacks = 0_u32;
    let mut combat_damage = 0_i64;
    let mut cost_paid = false;
    let mut payment_card_on_top = false;
    let mut ability_resolved = false;
    let mut activation_seen = false;
    let mut source_returned = false;
    let mut response_cast = false;
    let mut response_spell = None;
    let mut response_resolved = false;
    let mut source_in_graveyard_after_response = false;
    while !game.is_game_over() {
        if game.turn > 60 || accepted >= 2500 {
            return Err("Leashling full game hit its bounded run limit".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Leashling policy seat missing")?;
        let action = if let Some(action) = policy.propose_pending_decision(&view) {
            action
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
        let submitted_source = match &action {
            PolicyAction::ActivateAbilityWithGeneralizedCosts { activation }
                if activation.activation.ability_id == ABILITY_ID =>
            {
                Some(activation.activation.source)
            }
            _ => None,
        };
        let submitted_payment = match &action {
            PolicyAction::ActivateAbilityWithGeneralizedCosts { activation }
                if activation.activation.ability_id == ABILITY_ID =>
            {
                activation
                    .cost_payment
                    .hand_cards_to_library_top
                    .first()
                    .copied()
            }
            _ => None,
        };
        let before = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Leashling policy action rejected at turn {} / {:?} for {:?}: {error}",
                    game.turn, view.step, player
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("Leashling invariant failure: {error}"))?;
        for event in &game.event_log[before..] {
            match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|known| known.id == CARD_ID) =>
                {
                    source.get_or_insert(*card);
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if Some(*card) == source && mana_paid.is_empty() => mana_paid = colors.clone(),
                GameEvent::SpellResolved { card } if Some(*card) == source && power.is_none() => {
                    power = game
                        .characteristics(*card)
                        .ok()
                        .and_then(|value| value.power);
                }
                GameEvent::AttackersDeclared {
                    player: PlayerId(0),
                    attackers,
                } if source.is_some_and(|source| attackers.contains(&source)) => attacks += 1,
                GameEvent::DamageDealtToPlayer {
                    source: attacker,
                    player: PlayerId(1),
                    amount,
                } if source == Some(*attacker) => combat_damage += i64::from(*amount),
                GameEvent::HandCardPutOnLibraryTopAsAbilityCost {
                    player: PlayerId(0),
                    source: ability_source,
                    card,
                } if Some(*ability_source) == submitted_source => {
                    source.get_or_insert(*ability_source);
                    payment_card = Some(*card);
                    cost_paid = true;
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Library,
                } if Some(*card) == submitted_payment => payment_card_on_top = true,
                GameEvent::AbilityResolved {
                    source: ability_source,
                    ability,
                    ..
                } if Some(*ability_source) == source && *ability == ABILITY_ID => {
                    ability_resolved = true
                }
                GameEvent::AbilityActivated {
                    source: ability_source,
                    ability,
                    ..
                } if Some(*ability_source) == source && *ability == ABILITY_ID => {
                    activation_seen = true
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Hand,
                } if Some(*card) == source && activation_seen => source_returned = true,
                GameEvent::SpellCast {
                    player: PlayerId(1),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|known| known.id == LAST_GASP_ID) =>
                {
                    response_cast = true;
                    response_spell = Some(*card);
                }
                GameEvent::SpellResolved { card }
                    if game
                        .card_definition(*card)
                        .is_ok_and(|known| known.id == LAST_GASP_ID) =>
                {
                    response_resolved = true
                }
                _ => {}
            }
        }
        if response
            && response_resolved
            && source.is_some_and(|source| game.zone_of(source) == Some(Zone::Graveyard))
        {
            source_in_graveyard_after_response = true;
        }
    }
    let players = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    let mana_paid = source
        .and_then(|source| {
            game.event_log.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if *card == source => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    Ok(CapturedMatch {
        events: game.event_log.clone(),
        winner: game.winner().map(|player| player.0),
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempted.saturating_sub(accepted),
        source,
        payment_card,
        mana_paid,
        power,
        attacks,
        combat_damage,
        cost_paid,
        payment_card_on_top,
        ability_resolved,
        source_returned,
        response_cast,
        response_spell,
        response_resolved,
        source_in_graveyard_after_response,
        life: players,
    })
}

struct LeashlingPolicy {
    player: PlayerId,
    activated: bool,
    inner: RavCatalogPolicy,
}
impl LeashlingPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            activated: false,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
        }
    }
}
impl CodePolicy for LeashlingPolicy {
    fn id(&self) -> &'static str {
        "rav.leashling-full-game.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.active_player == self.player
            && view.step == Step::PrecombatMain
            && view.stack_depth == 0
        {
            if view.lands_played == 0 {
                if let Some(land) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some("RAV-MOUNTAIN"))
                {
                    return PolicyAction::PlayLand { card: land.id };
                }
            }
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            {
                let cost = ManaCost::new(6);
                if let Some(selection) = payment_selection(view, &cost) {
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
                if let Some(action) = make_mana(view, &cost, Color::Red) {
                    return action;
                }
            }
        }
        if !self.activated
            && view.active_player == self.player
            && view.step == Step::PostcombatMain
            && view.stack_depth == 0
        {
            if let (Some(source), Some(payment)) = (
                view.own_battlefield
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID) && card.tapped),
                view.hand.first(),
            ) {
                self.activated = true;
                return PolicyAction::ActivateAbilityWithGeneralizedCosts {
                    activation: GeneralizedAbilityActivation {
                        activation: AbilityActivation {
                            source: source.id,
                            ability_id: ABILITY_ID,
                            sacrifice_sources: vec![],
                            additional_tap_creatures: vec![],
                            discard_cards: vec![],
                            targets: vec![],
                        },
                        cost_payment: AbilityCostPayment {
                            hand_cards_to_library_top: vec![payment.id],
                            ..Default::default()
                        },
                        mana_payment_selection: None,
                    },
                };
            }
        }
        self.inner.propose_move(view)
    }
}

struct LeashlingResponsePolicy {
    player: PlayerId,
    responded: bool,
}
impl LeashlingResponsePolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            responded: false,
        }
    }
}
impl CodePolicy for LeashlingResponsePolicy {
    fn id(&self) -> &'static str {
        "rav.leashling-last-gasp-response.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if !self.responded {
            if let Some(stack_ability) = view.stack_activated_abilities.iter().rev().find(|entry| {
                entry.ability == ABILITY_ID && entry.source.definition == Some(CARD_ID)
            }) {
                let target = stack_ability.source.id;
                if let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(LAST_GASP_ID))
                {
                    let cost = ManaCost::with_colors(1, [Color::Black]);
                    if let Some(selection) = payment_selection(view, &cost) {
                        self.responded = true;
                        return PolicyAction::CastWithPayment {
                            request: CastRequest {
                                card: card.id,
                                targets: vec![cardbench_magic_engine::Target::Permanent(target)],
                                convoke: vec![],
                                payment_mana_abilities: vec![],
                            },
                            chosen_x: None,
                            mana_selection: selection,
                        };
                    }
                    if let Some(action) = make_mana(view, &cost, Color::Black) {
                        return action;
                    }
                }
            }
        }
        if view.active_player == self.player
            && view.step == Step::PrecombatMain
            && view.stack_depth == 0
            && view.lands_played == 0
        {
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-SWAMP"))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if !view.attackers_declared
            && view.active_player == self.player
            && view.step == Step::DeclareAttackers
        {
            return PolicyAction::DeclareAttackers { attackers: vec![] };
        }
        if !view.blockers_declared
            && view.active_player != self.player
            && view.step == Step::DeclareBlockers
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        PolicyAction::PassPriority
    }
}
fn payment_selection(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<_> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let (_, count) = available.iter_mut().find(|(known, _)| known == color)?;
        if *count == 0 {
            return None;
        }
        *count -= 1;
    }
    let mut generic = Vec::new();
    for _ in 0..cost.generic {
        let (color, count) = available.iter_mut().max_by_key(|(_, count)| *count)?;
        if *count == 0 {
            return None;
        }
        *count -= 1;
        generic.push(*color);
    }
    Some(ManaPaymentSelection {
        generic,
        hybrid: vec![],
    })
}
fn make_mana(view: &GameView, cost: &ManaCost, fallback_color: Color) -> Option<PolicyAction> {
    let mut need = cost
        .colored
        .iter()
        .copied()
        .find(|color| view.mana_pool.amount(*color) == 0);
    if need.is_none()
        && view.mana_pool.total() < cost.generic.saturating_add(cost.colored.len() as u8)
    {
        need = Some(fallback_color);
    }
    let color = need?;
    let land_definition = match color {
        Color::Red => "RAV-MOUNTAIN",
        _ => "RAV-SWAMP",
    };
    view.own_battlefield
        .iter()
        .find(|card| card.definition == Some(land_definition) && !card.tapped)
        .map(|land| PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
}

fn validate_trace(capture: &CapturedMatch, seed: u64, response: bool) -> Result<Value, String> {
    let source = capture.source.ok_or("Leashling was not cast")?;
    let payment = capture
        .payment_card
        .ok_or("Leashling hand-card payment identity missing")?;
    if capture.winner != Some(0)
        || capture.rejected_moves != 0
        || capture.life[0] <= 0
        || capture.life[1] > 0
    {
        return Err("Leashling game did not reach a clean seat-zero terminal win".to_owned());
    }
    if capture.mana_paid.len() != 6
        || capture.power != Some(3)
        || capture.attacks == 0
        || capture.combat_damage < 3
    {
        return Err(format!(
            "Leashling actual generic payment, 3/3 body, attack, or damage differs: mana={:?}, power={:?}, attacks={}, damage={}",
            capture.mana_paid, capture.power, capture.attacks, capture.combat_damage
        ));
    }
    if !capture.cost_paid || !capture.payment_card_on_top || !capture.ability_resolved {
        return Err(
            "Leashling did not pay the selected hand card onto its owner's library top and resolve"
                .to_owned(),
        );
    }
    if response
        && (!capture.response_cast
            || !capture.response_resolved
            || !capture.source_in_graveyard_after_response
            || capture.source_returned)
    {
        return Err(
            "Last Gasp response did not remove the exact source before the old ability resolved"
                .to_owned(),
        );
    }
    if !response && !capture.source_returned {
        return Err("Leashling did not return to its owner's hand on resolution".to_owned());
    }
    let events = &capture.events;
    let paid = events
        .iter()
        .position(|event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == source && colors.len() == 6))
        .ok_or_else(|| format!("actual Leashling six-mana spell payment missing for {source:?}; captured {:?}", events.iter().filter(|event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), .. })).collect::<Vec<_>>()))?;
    let cast = first_after(events, paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == source)).ok_or("Leashling cast event missing")?;
    let resolved = first_after(
        events,
        cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == source),
    )
    .ok_or("Leashling spell resolution missing")?;
    let entry = first_after(events, resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == source)).ok_or("Leashling battlefield entry missing")?;
    let attack = first_after(events, entry, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&source))).ok_or("Leashling attack receipt missing")?;
    let damage = first_after(events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source: attacker, player: PlayerId(1), amount } if *attacker == source && *amount == 3)).ok_or("Leashling combat damage receipt missing")?;
    let hand_cost = first_after(events, damage, |event| matches!(event, GameEvent::HandCardPutOnLibraryTopAsAbilityCost { player: PlayerId(0), source: ability_source, card } if *ability_source == source && *card == payment)).ok_or("Leashling hand-card cost provenance missing")?;
    let payment_move = first_after(events, hand_cost, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Library } if *card == payment)).ok_or("hand payment did not reach its owner's library")?;
    let activation = first_after(events, payment_move, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source: ability_source, ability: ABILITY_ID, .. } if *ability_source == source)).ok_or("Leashling activation did not reach the stack")?;
    let resolution = first_after(events, activation, |event| matches!(event, GameEvent::AbilityResolved { source: ability_source, ability: ABILITY_ID, .. } if *ability_source == source)).ok_or("Leashling activated ability did not resolve")?;
    let terminal = unique_index(events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("seat-zero terminal event missing")?;
    if !(paid < cast
        && cast < resolved
        && resolved < entry
        && entry < attack
        && attack < damage
        && damage < hand_cost
        && hand_cost < payment_move
        && payment_move < activation
        && activation < resolution
        && resolution < terminal)
    {
        return Err(
            "Leashling payment, combat, hand cost, resolution, and terminal event order differs"
                .to_owned(),
        );
    }
    let witness_indices = [
        paid,
        cast,
        resolved,
        entry,
        attack,
        damage,
        hand_cost,
        payment_move,
        activation,
        resolution,
        terminal,
    ];
    let projection = project_events(events);
    let projected: Vec<ProjectedEvent> = witness_indices
        .iter()
        .map(|index| projection[*index].clone())
        .collect();
    let expected = [
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "AttackersDeclared",
        "DamageDealtToPlayer",
        "HandCardPutOnLibraryTopAsAbilityCost",
        "CardMoved",
        "AbilityActivated",
        "AbilityResolved",
        "GameEnded",
    ];
    if projected
        .iter()
        .map(|event| event.kind.as_str())
        .collect::<Vec<_>>()
        != expected
    {
        return Err("typed Leashling projected event sequence differs".to_owned());
    }
    if response {
        let response_spell = capture
            .response_spell
            .ok_or("response Last Gasp object identity missing")?;
        let response_cast = unique_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == response_spell)).ok_or("response Last Gasp cast event missing")?;
        let response_resolved = first_after(
            events,
            response_cast,
            |event| matches!(event, GameEvent::SpellResolved { card } if *card == response_spell),
        )
        .ok_or("response Last Gasp resolution event missing")?;
        if !(activation < response_cast
            && response_cast < response_resolved
            && response_resolved < resolution)
        {
            return Err("Last Gasp did not resolve in the response window before Leashling's historical ability".to_owned());
        }
    }
    Ok(json!({
        "id": if response { "rav-leashling-hand-cost-source-departure-response" } else { "rav-leashling-hand-cost-return-full-game" },
        "seed": seed, "passed": true, "terminal": "Winner(PlayerId(0))", "winner_seat": 0,
        "turns": capture.turns, "accepted_moves": capture.accepted_moves, "rejected_moves": capture.rejected_moves,
        "leashling_object": source.0, "collector_number": 265, "mana_paid": capture.mana_paid.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
        "power": capture.power, "attacks": capture.attacks, "combat_damage": capture.combat_damage,
        "hand_payment": {"object": payment.0, "owner": 0, "zone_after_cost": "Library", "top_of_library": capture.payment_card_on_top},
        "activation": {"ability": ABILITY_ID, "source": source.0, "response_trace": response,
            "resolved": capture.ability_resolved, "source_returned_to_hand": capture.source_returned,
            "response_cast": capture.response_cast, "response_resolved": capture.response_resolved,
            "source_in_graveyard_after_response": capture.source_in_graveyard_after_response},
        "life_at_match_end": capture.life, "events": projected,
        "ordered_event_sequence": projected.iter().map(|event| event.sequence).collect::<Vec<_>>()
    }))
}

fn unique_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    let mut matches = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}
fn first_after(
    events: &[GameEvent],
    index: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(index + 1)
        .find_map(|(index, event)| predicate(event).then_some(index))
}
fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"trace_status":if passed{"passed"}else{"not_qualified"},"credited":false})
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
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(root.join("reward.txt"), "0.00000000\n").map_err(|error| error.to_string())
}
