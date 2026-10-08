//! Full-game RAV Golgari Signet payment trace.

use cardbench_magic_engine::{
    CardType, Color, DeckEntry, DeckList, Game, GameEvent, GameView, Keyword,
    ManaAbilityActivation, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step,
    Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_triggered_ability_bindings, CardSemanticStatus, CatalogCard,
    RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
};
use cardbench_magic_session::{project_events, ProjectedEvent};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::parse::Parser;
use syn::{Expr, Item, Lit, Meta, Stmt, Type, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-GOLGARI-SIGNET";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/golgari_signet.rs";
const ROOFSTALKER_ID: &str = "RAV-ROOFSTALKER-WIGHT";
const P0_DECK: &str = "rav_golgari_signet_payment";
const P1_DECK: &str = "rav_lands_only";

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/golgari_signet.rs"]
mod candidate_golgari_signet;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Golgari Signet PvP result: {error}");
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
            eprintln!("Golgari Signet PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_implementation()?;
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<CatalogCard> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
    {
        return Err(format!(
            "RAV catalog has {} names / {} printings; expected {} / {}",
            catalog.len(),
            printings.len(),
            RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
            RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        ));
    }
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, false)?;
    if captured.events != replay.events
        || captured.terminal != replay.terminal
        || captured.winner != replay.winner
        || captured.signet != replay.signet
        || captured.roofstalker != replay.roofstalker
        || captured.signet_cast_payment != replay.signet_cast_payment
        || captured.roofstalker_cast_payment != replay.roofstalker_cast_payment
        || captured.roofstalker_pt != replay.roofstalker_pt
        || captured.roofstalker_keywords != replay.roofstalker_keywords
        || captured.roofstalker_zone != replay.roofstalker_zone
        || captured.life_totals != replay.life_totals
    {
        return Err("Golgari Signet full-game replay was not deterministic".to_owned());
    }
    let mutation_detected = binding_mutation_detected(seed)?;
    if !mutation_detected {
        return Err(
            "changing Golgari Signet's mana bundle escaped the live payment trace".to_owned(),
        );
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
        "card": {"collector_number": 262, "definition_id": CARD_ID, "name": "Golgari Signet"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "106.1 and 106.4: mana production and spell payment",
                "117: priority and spell resolution",
                "302.1-302.2: artifact permanents",
                "601.2: casting and paying spell costs",
                "605.1a and 605.3: mana abilities and their activation costs",
                "608.3a: resolving permanent spells"
            ],
            "source_contract": "RAV Golgari Signet definition and candidate-owned paid black-green mana ability",
            "no_provider_trial": true
        },
        "cards_passed": cards_passed,
        "pilot_card_passed": cards_passed == 1,
        "passed": cards_passed == 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "candidate_pilot_pass; independent reviewer approval remains",
        "mutation_sensitivity": {
            "wrong_signet_bundle_mutation_detected": mutation_detected,
            "sealed_mutation_authority": false
        },
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    signet: Option<ObjectId>,
    roofstalker: Option<ObjectId>,
    signet_cast_payment: Vec<Color>,
    roofstalker_cast_payment: Vec<Color>,
    roofstalker_pt: Option<(i32, i32)>,
    roofstalker_keywords: Vec<Keyword>,
    roofstalker_zone: Option<Zone>,
    life_totals: [i64; 2],
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    for seed in 0..128 {
        let captured = run_match(seed, false)?;
        match validate_trace(&captured, seed) {
            Ok(_) => return Ok((seed, captured)),
            Err(error) if error.contains("not drawn") || error.contains("did not resolve") => {}
            Err(error) => return Err(error),
        }
    }
    Err("no bounded seed produced a Golgari Signet payment trace".to_owned())
}

fn binding_mutation_detected(seed: u64) -> Result<bool, String> {
    let captured = run_match(seed, true)?;
    Ok(validate_trace(&captured, seed)
        .err()
        .is_some_and(|error| error.contains("did not produce exactly black and green")))
}

fn run_match(seed: u64, mutate_bundle: bool) -> Result<CapturedMatch, String> {
    let pilot_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: ROOFSTALKER_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 26,
            },
            DeckEntry {
                card: "RAV-SWAMP".to_owned(),
                count: 26,
            },
        ],
        sideboard: vec![],
    };
    let passive_deck = DeckList {
        mainboard: vec![DeckEntry {
            card: "RAV-PLAINS".to_owned(),
            count: 60,
        }],
        sideboard: vec![],
    };
    for (label, deck) in [(P0_DECK, &pilot_deck), (P1_DECK, &passive_deck)] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{label} deck has {count} cards; expected 60"));
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Golgari Signet is missing from the RAV definition registry")?;
    *registered = candidate_golgari_signet::definition();
    let mut mana_bindings = rav_mana_ability_bindings();
    mana_bindings.retain(|binding| binding.card_definition != CARD_ID);
    let mut mana_binding = candidate_golgari_signet::mana_ability();
    if mutate_bundle {
        mana_binding.ability.output = cardbench_magic_engine::ManaAbilityOutput::PaidBundle {
            mana_cost: ManaCost::new(1),
            bundle: cardbench_magic_engine::ManaBundle::new([(Color::Red, 1), (Color::Black, 1)]),
        };
    }
    mana_bindings.push(mana_binding);
    let mut game = Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        mana_bindings,
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &passive_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(GolgariSignetPolicy::new(PlayerId(0), true)),
        Box::new(GolgariSignetPolicy::new(PlayerId(1), false)),
    ];
    let mut accepted_moves = 0_u32;
    let mut attempted_moves = 0_u32;
    let mut signet = None;
    let mut roofstalker = None;
    let mut signet_cast_payment = Vec::new();
    let mut roofstalker_cast_payment = Vec::new();
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted_moves >= config.max_policy_moves {
            return Err("Golgari Signet PvP match exceeded its development bound".to_owned());
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
        let before = game.event_log.len();
        attempted_moves += 1;
        match game.submit_policy_move(player, policy.id(), action) {
            Ok(()) => accepted_moves += 1,
            Err(error) => {
                return Err(format!(
                    "policy move rejected on turn {} at {:?}: {error}",
                    view.turn, view.step
                ));
            }
        }
        for event in &game.event_log[before..] {
            if let GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } = event
            {
                let definition = game
                    .card_definition(*card)
                    .map_err(|error| error.to_string())?;
                if definition.id == CARD_ID {
                    signet = Some(*card);
                    signet_cast_payment = colors.clone();
                }
                if definition.id == ROOFSTALKER_ID {
                    roofstalker = Some(*card);
                    roofstalker_cast_payment = colors.clone();
                }
            }
            if let GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } = event
            {
                let definition = game
                    .card_definition(*card)
                    .map_err(|error| error.to_string())?;
                if definition.id == CARD_ID {
                    signet = Some(*card);
                }
                if definition.id == ROOFSTALKER_ID {
                    roofstalker = Some(*card);
                }
            }
        }
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
    }
    let winner = game.winner().map(|player| player.0);
    let roofstalker_characteristics = roofstalker.and_then(|card| game.characteristics(card).ok());
    let roofstalker_pt = roofstalker_characteristics
        .as_ref()
        .and_then(|characteristics| Some((characteristics.power?, characteristics.toughness?)));
    let roofstalker_keywords = roofstalker_characteristics
        .map_or_else(Vec::new, |characteristics| characteristics.keywords);
    let roofstalker_zone = roofstalker.and_then(|card| game.zone_of(card));
    let life_totals = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    Ok(CapturedMatch {
        events: game.event_log.clone(),
        terminal: winner.map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({seat}))"),
        ),
        winner,
        turns: game.turn,
        accepted_moves,
        rejected_moves: attempted_moves.saturating_sub(accepted_moves),
        signet,
        roofstalker,
        signet_cast_payment,
        roofstalker_cast_payment,
        roofstalker_pt,
        roofstalker_keywords,
        roofstalker_zone,
        life_totals,
    })
}

struct GolgariSignetPolicy {
    player: PlayerId,
    pilot: bool,
    signet_cast: bool,
    signet_activated: bool,
    roofstalker_cast: bool,
}

impl GolgariSignetPolicy {
    fn new(player: PlayerId, pilot: bool) -> Self {
        Self {
            player,
            pilot,
            signet_cast: false,
            signet_activated: false,
            roofstalker_cast: false,
        }
    }

    fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let mut available: Vec<(Color, u8)> = Color::MANA_ALL
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
        let mut generic = Vec::with_capacity(usize::from(cost.generic));
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

    fn make_mana(&self, view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
        let missing = cost
            .colored
            .iter()
            .copied()
            .find(|color| view.mana_pool.amount(*color) == 0);
        let required = u16::from(cost.generic) + u16::try_from(cost.colored.len()).ok()?;
        let needed = missing.or_else(|| {
            (u16::from(view.mana_pool.total()) < required)
                .then(|| cost.colored.first().copied().unwrap_or(Color::Green))
        })?;
        let land = view.own_battlefield.iter().find(|card| {
            !card.tapped
                && card.card_types.contains(&CardType::Land)
                && matches!(
                    (card.basic_land_type, needed),
                    (
                        Some(cardbench_magic_engine::BasicLandType::Swamp),
                        Color::Black
                    ) | (
                        Some(cardbench_magic_engine::BasicLandType::Forest),
                        Color::Green
                    )
                )
        })?;
        let color = match land.basic_land_type? {
            cardbench_magic_engine::BasicLandType::Swamp => Color::Black,
            cardbench_magic_engine::BasicLandType::Forest => Color::Green,
            _ => return None,
        };
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        view.hand
            .iter()
            .find(|card| card.card_types.contains(&CardType::Land))
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }
}

impl CodePolicy for GolgariSignetPolicy {
    fn id(&self) -> &'static str {
        if self.pilot {
            "rav.golgari-signet-roofstalker.v1"
        } else {
            "rav.land-only-opponent.v1"
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
            let attackers = if self.pilot {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(ROOFSTALKER_ID) && card.can_attack)
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
        if view.active_player != self.player
            || view.priority != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.play_land(view) {
            return action;
        }
        if !self.pilot {
            return PolicyAction::PassPriority;
        }
        if !self.signet_cast {
            let cost = ManaCost::new(2);
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            {
                if let Some(mana_selection) = Self::payment(view, &cost) {
                    self.signet_cast = true;
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection,
                    };
                }
                if let Some(action) = self.make_mana(view, &cost) {
                    return action;
                }
            }
        }
        if !self.signet_cast {
            return PolicyAction::PassPriority;
        }
        let signet = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some(CARD_ID));
        let roofstalker_in_hand = view
            .hand
            .iter()
            .any(|card| card.definition == Some(ROOFSTALKER_ID));
        if !self.signet_activated && roofstalker_in_hand {
            if let Some(signet) = signet.filter(|card| !card.tapped) {
                let ability_cost = ManaCost::new(1);
                if view.mana_pool.total() < 1 {
                    if let Some(action) = self.make_mana(view, &ability_cost) {
                        return action;
                    }
                }
                self.signet_activated = true;
                return PolicyAction::ActivateBoundManaAbility {
                    activation: ManaAbilityActivation {
                        source: signet.id,
                        ability_id: "golgari-signet-bg",
                        chosen_color: None,
                    },
                };
            }
        }
        if !self.roofstalker_cast {
            let cost = ManaCost::with_colors(1, [Color::Black]);
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(ROOFSTALKER_ID))
            {
                if let Some(mana_selection) = Self::payment(view, &cost) {
                    self.roofstalker_cast = true;
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection,
                    };
                }
                if let Some(action) = self.make_mana(view, &cost) {
                    return action;
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    let signet = captured.signet.ok_or("Golgari Signet was not drawn")?;
    let roofstalker = captured
        .roofstalker
        .ok_or("Roofstalker Wight was not drawn")?;
    if captured.terminal != "Winner(PlayerId(0))" || captured.winner != Some(0) {
        return Err("Golgari Signet trace did not reach a terminal player-zero win".to_owned());
    }
    if captured.roofstalker_pt != Some((2, 1))
        || captured.roofstalker_zone != Some(Zone::Battlefield)
        || !captured.roofstalker_keywords.is_empty()
    {
        return Err(format!(
            "Roofstalker Wight did not resolve as a live vanilla 2/1: {:?}, {:?}, {:?}",
            captured.roofstalker_pt, captured.roofstalker_zone, captured.roofstalker_keywords
        ));
    }
    let cast_signet = event_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == signet)).ok_or("Golgari Signet cast receipt missing")?;
    let resolved_signet = event_index_after(
        &captured.events,
        cast_signet,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == signet),
    )
    .ok_or("Golgari Signet did not resolve")?;
    let entered_signet = event_index_after(&captured.events, resolved_signet, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == signet)).ok_or("Golgari Signet did not enter the battlefield")?;
    let bundle = event_index_after(&captured.events, entered_signet, |event| matches!(event, GameEvent::BoundManaAbilityBundleActivated { player: PlayerId(0), source, ability: "golgari-signet-bg", mana_cost, bundle, tapped: true, .. } if *source == signet && *mana_cost == ManaCost::new(1) && bundle.amount(Color::Green) == 1 && bundle.amount(Color::Black) == 1 && bundle.iter().count() == 2)).ok_or_else(|| format!("Golgari Signet did not produce exactly black and green with its tap cost paid: {:?}", captured.events.iter().filter(|event| matches!(event, GameEvent::ManaAbilityManaPaid { .. } | GameEvent::BoundManaAbilityBundleActivated { .. } | GameEvent::BoundManaAbilityActivated { .. } | GameEvent::ManaAdded { .. })).collect::<Vec<_>>()))?;
    if captured.signet_cast_payment.len() != 2
        || captured.roofstalker_cast_payment.len() != 2
        || !captured.roofstalker_cast_payment.contains(&Color::Green)
        || !captured.roofstalker_cast_payment.contains(&Color::Black)
    {
        return Err(format!(
            "Golgari Signet/Roofstalker Wight cast payments are wrong: {:?}/{:?}",
            captured.signet_cast_payment, captured.roofstalker_cast_payment
        ));
    }
    let ability_cost = event_index_after(&captured.events, bundle, |event| matches!(event, GameEvent::ManaAbilityManaPaid { player: PlayerId(0), mana_cost } if *mana_cost == ManaCost::new(1))).ok_or("Golgari Signet did not pay one generic mana for its activation")?;
    if captured.events.iter().any(
        |event| matches!(event, GameEvent::AbilityActivated { source, .. } if *source == signet),
    ) {
        return Err("Golgari Signet mana ability incorrectly used the stack".to_owned());
    }
    let cast_roofstalker = event_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == roofstalker)).ok_or("Roofstalker Wight cast receipt missing")?;
    let resolved_roofstalker = event_index_after(
        &captured.events,
        cast_roofstalker,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == roofstalker),
    )
    .ok_or("Roofstalker Wight did not resolve")?;
    let entered_roofstalker = event_index_after(&captured.events, resolved_roofstalker, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == roofstalker)).ok_or("Roofstalker Wight did not enter the battlefield")?;
    let damage = event_index(&captured.events, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 2 } if *source == roofstalker)).ok_or("Roofstalker Wight did not attack for combat damage")?;
    let end = event_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal player-zero game event missing")?;
    let ability_order = cast_signet < resolved_signet
        && resolved_signet < entered_signet
        && entered_signet < bundle
        && bundle < ability_cost
        && ability_cost < cast_roofstalker
        && cast_roofstalker < resolved_roofstalker
        && resolved_roofstalker < entered_roofstalker
        && entered_roofstalker < damage
        && damage < end;
    if !ability_order || captured.life_totals[1] > 0 {
        return Err(format!(
            "Golgari Signet paid-bundle/cast/combat/terminal event order differs (signet={cast_signet}/{resolved_signet}/{entered_signet}, activation={ability_cost}/{bundle}, roofstalker={cast_roofstalker}/{resolved_roofstalker}/{entered_roofstalker}, damage={damage}, end={end}, opponent_life={})",
            captured.life_totals[1]
        ));
    }
    let paid = captured
        .events
        .iter()
        .filter_map(|event| match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if *card == roofstalker => Some(colors.clone()),
            _ => None,
        })
        .next()
        .ok_or("Roofstalker Wight payment event missing")?;
    if paid.len() != 2 || !paid.contains(&Color::Green) || !paid.contains(&Color::Black) {
        return Err(
            "Roofstalker Wight did not actually spend the Signet's black-green colors".to_owned(),
        );
    }
    let projected = project_events(&captured.events);
    let cast_turn = projected
        .iter()
        .find(|event| {
            event.kind == "SpellCast"
                && event
                    .facts
                    .objects
                    .iter()
                    .any(|object| object.0 == roofstalker.0)
        })
        .map(|event| event.turn)
        .ok_or("Roofstalker Wight projected cast event missing")?;
    let attack_turns: Vec<u32> = projected
        .iter()
        .filter(|event| {
            event.kind == "AttackersDeclared"
                && event
                    .facts
                    .objects
                    .iter()
                    .any(|object| object.0 == roofstalker.0)
        })
        .map(|event| event.turn)
        .collect();
    if attack_turns.is_empty() || attack_turns[0] <= cast_turn {
        return Err(
            "Roofstalker Wight attacked before its summoning-sickness restriction expired"
                .to_owned(),
        );
    }
    let events = relevant_events(&captured.events, signet, roofstalker);
    let sequence = vec![
        cast_signet,
        resolved_signet,
        entered_signet,
        bundle,
        ability_cost,
        cast_roofstalker,
        resolved_roofstalker,
        entered_roofstalker,
        damage,
        end,
    ];
    Ok(json!({
        "id": "rav-golgari-signet-casts-roofstalker",
        "seed": seed,
        "decks": [P0_DECK, P1_DECK],
        "policies": ["rav.golgari-signet-roofstalker.v1", "rav.land-only-opponent.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "life_at_match_end": captured.life_totals,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "digest": event_digest(&captured.events),
        "golgari_signet_object": signet.0,
        "roofstalker_object": roofstalker.0,
        "roofstalker_power_toughness": {"power": captured.roofstalker_pt.map(|pt| pt.0), "toughness": captured.roofstalker_pt.map(|pt| pt.1)},
        "roofstalker_keywords": captured.roofstalker_keywords.iter().map(|keyword| format!("{keyword:?}")).collect::<Vec<_>>(),
        "roofstalker_zone": format!("{:?}", captured.roofstalker_zone),
        "cast_payments": {
            "golgari_signet": {"required_generic_symbols": 2, "paid_total": captured.signet_cast_payment.len()},
            "roofstalker": {"required_generic_symbols": 1, "green_generic_symbol": true, "black_symbol": true, "spent": paid.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "total": paid.len()}
        },
        "signet_activation": {"ability": "golgari-signet-bg", "generic_cost_paid": 1, "tapped": true, "mana_output": {"green": 1, "black": 1}, "non_stack": true},
        "roofstalker_cast_turn": cast_turn,
        "roofstalker_attack_turns": attack_turns,
        "ordered_event_sequence": sequence,
        "event_positions": {"signet_cast": cast_signet, "signet_resolved": resolved_signet, "signet_entered_battlefield": entered_signet, "signet_cost_paid": ability_cost, "signet_bundle_created": bundle, "roofstalker_cast": cast_roofstalker, "roofstalker_resolved": resolved_roofstalker, "roofstalker_entered_battlefield": entered_roofstalker, "roofstalker_combat_damage": damage, "terminal": end},
        "events": events,
        "passed": true
    }))
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn event_index_after(
    events: &[GameEvent],
    after: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(after + 1)
        .find_map(|(index, event)| predicate(event).then_some(index))
}

fn relevant_events(
    events: &[GameEvent],
    signet: ObjectId,
    roofstalker: ObjectId,
) -> Vec<ProjectedEvent> {
    let signet_ref = format!("ObjectId({})", signet.0);
    project_events(events)
        .into_iter()
        .filter(|event| match event.kind.as_str() {
            "SpellManaPaid"
            | "SpellCast"
            | "SpellResolved"
            | "CardMoved"
            | "DamageDealtToPlayer"
            | "AttackersDeclared"
            | "AbilityActivated" => event
                .facts
                .objects
                .iter()
                .any(|object| object.0 == signet.0 || object.0 == roofstalker.0),
            "BoundManaAbilityBundleActivated" => event.canonical.contains(&signet_ref),
            "ManaAbilityManaPaid" => event.canonical.contains("player: PlayerId(0)"),
            "GameEnded" => true,
            _ => false,
        })
        .collect()
}

fn event_digest(events: &[GameEvent]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for event in events {
        for byte in format!("{event:?}\n").bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    format!("fnv1a64:{hash:016x}")
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => definition_id,
        _ => "",
    };
    let passed = definition_id == CARD_ID;
    json!({"collector_number":card.collector_number,"definition_id":definition_id,"name":card.name,"engine_status":"executable_compatibility_slice","qualification":if passed{"pilot_trace_passed; independent reviewer approval pending"}else{"not_evaluated"},"trace_status":if passed{"passed"}else{"not_qualified"},"credited":false})
}

fn validate_candidate_implementation() -> Result<(), String> {
    let candidate = candidate_golgari_signet::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Golgari Signet is missing from the executable RAV registry")?;
    if candidate != registered {
        return Err(
            "candidate Golgari Signet definition differs from its candidate-owned registry binding"
                .to_owned(),
        );
    }
    let candidate_ability = candidate_golgari_signet::mana_ability();
    let registered_ability = rav_mana_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Golgari Signet paid mana ability is missing from the RAV registry")?;
    if candidate_ability != registered_ability {
        return Err(
            "candidate Golgari Signet mana ability differs from its candidate-owned registry binding"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Golgari Signet source: {error}"))?;
    if source.len() > 64 * 1024 {
        return Err("Golgari Signet source exceeds 64 KiB".to_owned());
    }
    let file = syn::parse_file(&source)
        .map_err(|_| "Golgari Signet source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Golgari Signet source has a non-documentation module attribute".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "ActivatedManaAbility"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "ManaAbilityBinding"],
        vec!["cardbench_magic_engine", "ManaAbilityOutput"],
        vec!["cardbench_magic_engine", "ManaBundle"],
        vec!["cardbench_magic_engine", "ManaCost"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let Some(paths) = flattened_use_paths(item) else {
                    return Err("Golgari Signet source contains an unsupported import".to_owned());
                };
                imports.extend(paths);
            }
            Item::Fn(function) if function.attrs.iter().all(is_safe_function_attribute) => {
                let name = function.sig.ident.to_string();
                if !functions.insert(name.clone()) {
                    return Err("Golgari Signet source has duplicate functions".to_owned());
                }
                let expected_type = match name.as_str() {
                    "definition" => "CardDefinition",
                    "mana_ability" => "ManaAbilityBinding",
                    _ => {
                        return Err(
                            "Golgari Signet source contains an unexpected function".to_owned()
                        );
                    }
                };
                validate_candidate_function(function, expected_type)?;
                if function.block.stmts.len() != 1 {
                    return Err(
                        "Golgari Signet functions must be direct declarative literals".to_owned(),
                    );
                }
                let Stmt::Expr(expression, None) = &function.block.stmts[0] else {
                    return Err("Golgari Signet functions cannot contain statements".to_owned());
                };
                validate_literal_shape(expression, expected_type)?;
                if !safe_candidate_expression(expression) {
                    return Err(
                        "Golgari Signet source contains an executable expression".to_owned()
                    );
                }
            }
            _ => {
                return Err(
                    "Golgari Signet source contains an unsupported top-level item".to_owned(),
                );
            }
        }
    }
    if imports != expected_imports
        || functions != BTreeSet::from(["definition".to_owned(), "mana_ability".to_owned()])
        || file.items.len() != 4
    {
        return Err(
            "Golgari Signet imports or module structure differ from the task boundary".to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_function(function: &syn::ItemFn, return_type: &str) -> Result<(), String> {
    let signature = &function.sig;
    let visibility = matches!(&function.vis, Visibility::Restricted(visibility) if visibility.path.is_ident("crate"));
    let returns_expected = matches!(&signature.output, syn::ReturnType::Type(_, ty) if matches!(&**ty, Type::Path(path) if path.qself.is_none() && path.path.is_ident(return_type)));
    if !visibility
        || !signature.inputs.is_empty()
        || !signature.generics.params.is_empty()
        || signature.generics.where_clause.is_some()
        || signature.constness.is_some()
        || signature.asyncness.is_some()
        || signature.unsafety.is_some()
        || signature.abi.is_some()
        || signature.variadic.is_some()
        || !returns_expected
    {
        return Err(format!(
            "Golgari Signet function must be a plain {}() -> {}",
            signature.ident, return_type
        ));
    }
    Ok(())
}

fn validate_literal_shape(expression: &Expr, expected: &str) -> Result<(), String> {
    let Expr::Struct(structure) = expression else {
        return Err(format!(
            "Golgari Signet {expected} must return a struct literal"
        ));
    };
    if structure
        .path
        .segments
        .last()
        .is_none_or(|segment| segment.ident != expected)
        || structure.rest.is_some()
    {
        return Err(format!("Golgari Signet literal must be {expected}"));
    }
    let expected_fields: BTreeSet<&str> = match expected {
        "CardDefinition" => [
            "id",
            "name",
            "set_code",
            "mana_cost",
            "colors",
            "mana_colors",
            "card_types",
            "is_basic_land",
            "supported_rules",
            "power",
            "toughness",
            "keywords",
            "effects",
        ]
        .into_iter()
        .collect(),
        "ManaAbilityBinding" => ["card_definition", "ability"].into_iter().collect(),
        "ActivatedManaAbility" => [
            "id",
            "tap_cost",
            "output",
            "amount",
            "life_payment",
            "controller_damage",
        ]
        .into_iter()
        .collect(),
        "PaidBundle" => ["mana_cost", "bundle"].into_iter().collect(),
        _ => return Err("unexpected candidate literal type".to_owned()),
    };
    let actual: BTreeSet<String> = structure
        .fields
        .iter()
        .filter_map(|field| match &field.member {
            syn::Member::Named(name) => Some(name.to_string()),
            _ => None,
        })
        .collect();
    if actual.len() != structure.fields.len()
        || actual
            != expected_fields
                .iter()
                .map(|field| (*field).to_owned())
                .collect()
    {
        return Err(format!(
            "Golgari Signet {expected} fields differ from the declared type"
        ));
    }
    Ok(())
}

fn safe_candidate_expression(expression: &Expr) -> bool {
    match expression {
        Expr::Path(path) => path.attrs.is_empty() && path.qself.is_none(),
        Expr::Lit(literal) => {
            literal.attrs.is_empty()
                && matches!(literal.lit, Lit::Str(_) | Lit::Int(_) | Lit::Bool(_))
        }
        Expr::Array(array) => {
            array.attrs.is_empty() && array.elems.iter().all(safe_candidate_expression)
        }
        Expr::Tuple(tuple) => {
            tuple.attrs.is_empty() && tuple.elems.iter().all(safe_candidate_expression)
        }
        Expr::Reference(reference) => {
            reference.attrs.is_empty() && safe_candidate_expression(&reference.expr)
        }
        Expr::Paren(paren) => paren.attrs.is_empty() && safe_candidate_expression(&paren.expr),
        Expr::Group(group) => group.attrs.is_empty() && safe_candidate_expression(&group.expr),
        Expr::Call(call) => {
            if !call.attrs.is_empty() {
                return false;
            }
            let Expr::Path(function) = &*call.func else {
                return false;
            };
            let path = function
                .path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            matches!(
                path.as_str(),
                "BTreeSet::from" | "BTreeSet::new" | "ManaCost::new" | "ManaBundle::new"
            ) && call.args.iter().all(safe_candidate_expression)
        }
        Expr::Struct(structure) => {
            if !structure.attrs.is_empty() {
                return false;
            }
            let name = structure
                .path
                .segments
                .last()
                .map(|segment| segment.ident.to_string());
            matches!(
                name.as_deref(),
                Some(
                    "CardDefinition" | "ManaAbilityBinding" | "ActivatedManaAbility" | "PaidBundle"
                )
            ) && structure.rest.is_none()
                && structure
                    .fields
                    .iter()
                    .all(|field| field.attrs.is_empty() && safe_candidate_expression(&field.expr))
        }
        Expr::Macro(mac) => {
            if !mac.attrs.is_empty() || !mac.mac.path.is_ident("vec") {
                return false;
            }
            syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
                .parse2(mac.mac.tokens.clone())
                .is_ok_and(|expressions| expressions.iter().all(safe_candidate_expression))
        }
        _ => false,
    }
}

fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc")
        && matches!(&attribute.meta, Meta::NameValue(value) if matches!(&value.value, Expr::Lit(literal) if matches!(&literal.lit, Lit::Str(_))))
}

fn is_safe_function_attribute(attribute: &syn::Attribute) -> bool {
    is_doc_attribute(attribute)
        || attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_))
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
        UseTree::Glob(_) | UseTree::Rename(_) => false,
    }
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn output_root() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        if argument == "--output-root" {
            return args.next().map(Into::into);
        }
    }
    None
}

fn write_outputs(root: &std::path::Path, report: &Value) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(root.join("reward.txt"), b"0.0\n").map_err(|error| error.to_string())?;
    if let Some(trace) = report["traces"]
        .as_array()
        .and_then(|traces| traces.first())
    {
        std::fs::write(
            root.join("trace.json"),
            serde_json::to_vec_pretty(trace).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(())
}
