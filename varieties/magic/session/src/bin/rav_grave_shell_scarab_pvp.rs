//! Isolated RAV Grave-Shell Scarab full-game authority.

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, CardDefinition, CardType, Color, DecisionKind,
    DecisionSelection, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, Keyword, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Zone,
};
use cardbench_magic_policies::{
    CatalogPolicyProfile, CodePolicy, DeckMatchConfig, RavCatalogPolicy,
};
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
use syn::{Expr, Item, Stmt};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-GRAVE-SHELL-SCARAB";
const CARD_NAME: &str = "Grave-Shell Scarab";
const ABILITY_ID: &str = "sacrifice-source-draw";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/grave_shell_scarab.rs";
const DEFAULT_SEED: u64 = 12;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/grave_shell_scarab.rs"]
mod candidate_grave_shell_scarab;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Grave-Shell Scarab result: {error}");
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
            eprintln!("Grave-Shell Scarab PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_definition()?;
    let catalog = unique_catalog()?;
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured != replay || project_events(&captured.events) != project_events(&replay.events) {
        return Err("Grave-Shell Scarab replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "missing_dredge_mutation_detected": mutation_detected(seed, Mutation::MissingDredge),
        "wrong_dredge_count_mutation_detected": mutation_detected(seed, Mutation::DredgeTwo),
        "wrong_mana_cost_mutation_detected": mutation_detected(seed, Mutation::WrongCost),
        "wrong_creature_body_mutation_detected": mutation_detected(seed, Mutation::WrongBody),
        "missing_activated_ability_mutation_detected": mutation_detected(seed, Mutation::RemoveAbility),
        "wrong_activation_mana_cost_mutation_detected": mutation_detected(seed, Mutation::WrongAbilityCost),
        "missing_source_sacrifice_mutation_detected": mutation_detected(seed, Mutation::NoSourceSacrifice),
        "wrong_activation_effect_mutation_detected": mutation_detected(seed, Mutation::WrongAbilityEffect),
        "sealed_mutation_authority": false
    });
    for key in [
        "missing_dredge_mutation_detected",
        "wrong_dredge_count_mutation_detected",
        "wrong_mana_cost_mutation_detected",
        "wrong_creature_body_mutation_detected",
        "missing_activated_ability_mutation_detected",
        "wrong_activation_mana_cost_mutation_detected",
        "missing_source_sacrifice_mutation_detected",
        "wrong_activation_effect_mutation_detected",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Grave-Shell Scarab semantic mutation escaped: {key}"
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
        "card": {"collector_number": 211, "printings": 1, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117: priority and stack resolution",
                "202.1-202.2: mana cost and color",
                "302.1-302.2: creature characteristics",
                "602.1: activated ability costs and stack use",
                "702.52: Dredge replaces a draw",
                "704: state-based actions"
            ],
            "source_contract": "Original RAV #211 is a black-green {2}{B}{G}{G} 4/4 creature with Dredge 1 and '{1}, Sacrifice this creature: Draw a card.'",
            "rules_ruling": "The 2005-10-01 ruling states that the Scarab may be sacrificed to its first ability and then returned by replacing that draw with its own Dredge ability.",
            "authority_cutoff": "2026-09-24",
            "no_provider_trial": true
        },
        "passed": cards_passed == 1,
        "pilot_card_passed": cards_passed == 1,
        "replay_deterministic": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "diagnostic_only; external qualification pending",
        "candidate_source_validation": {
            "direct_literal_ast_checked_before_candidate_calls": true,
            "guard_ran_before_candidate_function_calls": true
        },
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    MissingDredge,
    DredgeTwo,
    WrongCost,
    WrongBody,
    RemoveAbility,
    WrongAbilityCost,
    NoSourceSacrifice,
    WrongAbilityEffect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    life_totals: [i64; 2],
    first_scarab: Option<ObjectId>,
    battlefield_scarabs: Vec<ObjectId>,
    battlefield_scarab_pt: Vec<(i32, i32)>,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    let requested_seed = std::env::var("CARDBENCH_GRAVE_SHELL_SCARAB_SEED")
        .ok()
        .and_then(|value| value.parse().ok());
    let seeds =
        requested_seed.map_or_else(|| (DEFAULT_SEED..=DEFAULT_SEED + 64).collect(), |s| vec![s]);
    let mut last_error = "no seed attempted".to_owned();
    for seed in seeds {
        match run_match(seed, Mutation::None) {
            Ok(captured) => match validate_trace(&captured, seed) {
                Ok(trace) => return Ok((seed, captured)),
                Err(error) => last_error = format!("seed {seed}: {error}"),
            },
            Err(error) => last_error = format!("seed {seed}: {error}"),
        }
    }
    Err(format!(
        "no deterministic Grave-Shell Scarab PvP trace found: {last_error}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation) {
        Ok(captured) => validate_trace(&captured, seed).is_err(),
        Err(_) => true,
    }
}

fn definition_for(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_grave_shell_scarab::definition();
    match mutation {
        Mutation::None
        | Mutation::RemoveAbility
        | Mutation::WrongAbilityCost
        | Mutation::NoSourceSacrifice
        | Mutation::WrongAbilityEffect => {}
        Mutation::MissingDredge => definition.keywords.clear(),
        Mutation::DredgeTwo => definition.keywords = vec![Keyword::Dredge(2)],
        Mutation::WrongCost => {
            definition.mana_cost = ManaCost::with_colors(2, [Color::Black, Color::Green]);
        }
        Mutation::WrongBody => {
            definition.power = Some(3);
            definition.toughness = Some(4);
        }
    }
    definition
}

fn ability_for(mutation: Mutation) -> ActivatedAbility {
    let mut ability = candidate_grave_shell_scarab::activated_ability();
    match mutation {
        Mutation::WrongAbilityCost => ability.mana_cost = ManaCost::new(2),
        Mutation::NoSourceSacrifice => ability.sacrifice_source = false,
        Mutation::WrongAbilityEffect => {
            ability.effects = vec![Effect::GainLifeController { amount: 1 }]
        }
        _ => {}
    }
    ability
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let pilot_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-SWAMP".to_owned(),
                count: 28,
            },
        ],
        sideboard: vec![],
    };
    let opponent_deck = DeckList {
        mainboard: vec![DeckEntry {
            card: "RAV-FOREST".to_owned(),
            count: 60,
        }],
        sideboard: vec![],
    };
    for (label, deck) in [("pilot", &pilot_deck), ("opponent", &opponent_deck)] {
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
        .ok_or("Grave-Shell Scarab missing from the RAV registry")?;
    *registered = definition_for(mutation);
    let scarab_cost = registered.mana_cost.clone();
    let mut abilities = rav_activated_ability_bindings();
    if mutation == Mutation::RemoveAbility {
        abilities.retain(|binding| {
            binding.card_definition != CARD_ID || binding.ability.id != ABILITY_ID
        });
    } else if let Some(binding) = abilities
        .iter_mut()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
    {
        binding.ability = ability_for(mutation);
    } else {
        return Err(
            "Grave-Shell Scarab activated ability missing from the RAV binding table".to_owned(),
        );
    }
    let ability_cost = abilities
        .iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .map(|binding| binding.ability.mana_cost.clone())
        .unwrap_or_else(|| ManaCost::new(1));
    let mut game = Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
        rav_triggered_ability_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
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
        Box::new(ScarabPolicy::new(
            PlayerId(0),
            true,
            scarab_cost,
            ability_cost,
        )),
        Box::new(ScarabPolicy::new(
            PlayerId(1),
            false,
            ManaCost::new(0),
            ManaCost::new(0),
        )),
    ];
    let mut accepted_moves = 0_u32;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted_moves >= config.max_policy_moves {
            return Err("Grave-Shell Scarab match exceeded its development bound".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("policy seat missing")?;
        let action = if let Some(action) = policy.propose_pending_decision(&view) {
            action
        } else if view.draw_replacement_pending {
            policy.propose_draw_replacement(&view)
        } else if view.optional_triggered_ability_choice.is_some() {
            policy.propose_optional_triggered_ability(&view)
        } else if view.private_library_choice.is_some() {
            policy.propose_private_library_choice(&view)
        } else if view.private_opponent_library_choice.is_some() {
            policy.propose_private_opponent_library_choice(&view)
        } else if view.library_search_choice.is_some() {
            policy.propose_library_search_choice(&view)
        } else {
            policy.propose_move(&view)
        };
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "policy move rejected at turn {} / {:?} for {:?}: {error}",
                    view.turn, view.step, player
                )
            })?;
        accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
    }
    let winner = game.winner().map(|player| player.0);
    let first_scarab = game.event_log.iter().find_map(|event| match event {
        GameEvent::AbilityActivated {
            player: PlayerId(0),
            source,
            definition: CARD_ID,
            ability: ABILITY_ID,
            ..
        } => Some(*source),
        _ => None,
    });
    let view = game
        .view_for_player(PlayerId(0))
        .map_err(|error| error.to_string())?;
    let battlefield_scarabs = view
        .own_battlefield
        .iter()
        .filter(|card| card.definition == Some(CARD_ID))
        .map(|card| card.id)
        .collect();
    let battlefield_scarab_pt = view
        .own_battlefield
        .iter()
        .filter(|card| card.definition == Some(CARD_ID))
        .filter_map(|card| {
            game.characteristics(card.id)
                .ok()
                .and_then(|value| Some((value.power?, value.toughness?)))
        })
        .collect();
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
        winner,
        turns: game.turn,
        accepted_moves,
        life_totals,
        first_scarab,
        battlefield_scarabs,
        battlefield_scarab_pt,
    })
}

struct ScarabPolicy {
    player: PlayerId,
    pilot: bool,
    inner: RavCatalogPolicy,
    scarab_cost: ManaCost,
    ability_cost: ManaCost,
    activation_count: u8,
    dredge_decisions: u8,
    source: Option<ObjectId>,
    recovered: Option<ObjectId>,
    recovered_recast: bool,
}

impl ScarabPolicy {
    fn new(player: PlayerId, pilot: bool, scarab_cost: ManaCost, ability_cost: ManaCost) -> Self {
        Self {
            player,
            pilot,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
            scarab_cost,
            ability_cost,
            activation_count: 0,
            dredge_decisions: 0,
            source: None,
            recovered: None,
            recovered_recast: false,
        }
    }
}

impl CodePolicy for ScarabPolicy {
    fn id(&self) -> &'static str {
        if self.pilot {
            "rav.grave-shell-scarab-dredge-pilot.v1"
        } else {
            "rav.grave-shell-scarab-no-creature-opponent.v1"
        }
    }

    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        let decision = view.pending_decision.as_ref()?;
        if self.pilot
            && view.player == self.player
            && decision.kind == DecisionKind::DredgeReplacement
        {
            let chosen = if self.dredge_decisions == 0 {
                self.source.and_then(|source| {
                    decision
                        .candidates
                        .iter()
                        .find(|card| card.id == source)
                        .map(|card| card.id)
                })
            } else {
                None
            };
            if chosen.is_some() {
                self.recovered = chosen;
            }
            self.dredge_decisions = self.dredge_decisions.saturating_add(1);
            return Some(PolicyAction::SubmitDecision {
                decision: decision.id,
                selection: DecisionSelection::Objects(chosen.into_iter().collect()),
            });
        }
        self.inner.propose_pending_decision(view)
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if self.pilot
            && view.step == Step::PrecombatMain
            && view.active_player == self.player
            && view.priority == self.player
            && view.stack_depth == 0
        {
            if let Some(source) = self.recovered.filter(|source| {
                view.hand
                    .iter()
                    .any(|card| card.id == *source && card.definition == Some(CARD_ID))
            }) && let Some(mana_selection) = payment_selection(view, &self.scarab_cost)
            {
                self.recovered_recast = true;
                self.recovered = None;
                return PolicyAction::CastWithPayment {
                    request: cardbench_magic_engine::CastRequest {
                        card: source,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection,
                };
            }
            if self.activation_count < 2
                && view.mana_pool.total_exact() >= u16::from(self.ability_cost.mana_value())
            {
                if let Some(source) = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
                    .map(|card| card.id)
                {
                    self.activation_count += 1;
                    self.source = Some(source);
                    return PolicyAction::ActivateAbility {
                        activation: AbilityActivation {
                            source,
                            ability_id: ABILITY_ID,
                            sacrifice_sources: vec![source],
                            additional_tap_creatures: vec![],
                            discard_cards: vec![],
                            targets: vec![],
                        },
                    };
                }
            }
        }
        let action = self.inner.propose_move(view);
        match action {
            PolicyAction::Cast(request) => {
                let is_scarab = view
                    .hand
                    .iter()
                    .any(|card| card.id == request.card && card.definition == Some(CARD_ID));
                if self.pilot && is_scarab && !self.recovered_recast && self.recovered.is_some() {
                    return PolicyAction::PassPriority;
                }
                if is_scarab
                    && let Some(mana_selection) = payment_selection(view, &self.scarab_cost)
                {
                    PolicyAction::CastWithPayment {
                        request,
                        chosen_x: None,
                        mana_selection,
                    }
                } else {
                    PolicyAction::Cast(request)
                }
            }
            action => action,
        }
    }
}

fn payment_selection(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<(Color, u8)> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let (_, amount) = available.iter_mut().find(|(known, _)| known == color)?;
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

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    let events = &captured.events;
    let source = captured
        .first_scarab
        .ok_or("pilot never activated Grave-Shell Scarab")?;
    let activations: Vec<usize> = events.iter().enumerate().filter_map(|(index, event)| matches!(event,
        GameEvent::AbilityActivated { player: PlayerId(0), source: card, definition: CARD_ID, ability: ABILITY_ID, .. } if *card == source
    ).then_some(index)).collect();
    if activations.len() != 2 {
        return Err(format!(
            "expected two activations of the same Scarab, observed {}",
            activations.len()
        ));
    }
    let sacrifices = events.iter().enumerate().filter_map(|(index, event)| matches!(event,
        GameEvent::SacrificedAsAbilityCost { player: PlayerId(0), source: card, permanent } if *card == source && *permanent == source
    ).then_some(index)).collect::<Vec<_>>();
    if sacrifices.len() != 2
        || sacrifices
            .iter()
            .zip(&activations)
            .any(|(sacrifice, activation)| sacrifice >= activation)
    {
        return Err(
            "the same Scarab was not sacrificed as the cost before each ability entered the stack"
                .to_owned(),
        );
    }
    if sacrifices
        .iter()
        .zip(&activations)
        .any(|(sacrifice, activation)| {
            !events[*sacrifice + 1..*activation].iter().any(|event| {
            matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == source)
        })
        })
    {
        return Err(
            "sacrificed Scarab did not enter its graveyard before the ability was stacked"
                .to_owned(),
        );
    }
    let paid = events.iter().filter(|event| matches!(event,
        GameEvent::AbilityManaPaid { player: PlayerId(0), source: card, ability: ABILITY_ID, mana_cost } if *card == source && *mana_cost == ManaCost::new(1)
    )).count();
    if paid != 2 {
        return Err(format!(
            "expected two exact {{1}} activation payments, observed {paid}"
        ));
    }
    let casts: Vec<ObjectId> = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } => Some(*card),
            _ => None,
        })
        .collect();
    let scarab_payments: Vec<&Vec<Color>> = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if casts.contains(card) => Some(colors),
            _ => None,
        })
        .collect();
    if scarab_payments.is_empty()
        || scarab_payments.iter().any(|payment| {
            payment.len() != 5
                || payment
                    .iter()
                    .filter(|color| **color == Color::Green)
                    .count()
                    < 2
                || payment
                    .iter()
                    .filter(|color| **color == Color::Black)
                    .count()
                    < 1
        })
    {
        return Err(format!(
            "Grave-Shell Scarab spells were not paid as {{2}}{{B}}{{G}}{{G}}: {scarab_payments:?}"
        ));
    }
    let opened: Vec<(usize, cardbench_magic_engine::DecisionId)> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::DecisionOpened {
                decision,
                player: PlayerId(0),
                kind: DecisionKind::DredgeReplacement,
                ..
            } => Some((index, *decision)),
            _ => None,
        })
        .collect();
    if opened.len() < 2 {
        return Err(format!(
            "expected Dredge choices for both ability draws, observed {}",
            opened.len()
        ));
    }
    let completed = |decision| {
        events.iter().enumerate().find_map(|(index, event)| matches!(event,
        GameEvent::DecisionCompleted { decision: done, player: PlayerId(0), kind: DecisionKind::DredgeReplacement } if *done == decision
    ).then_some(index))
    };
    let first_complete = completed(opened[0].1).ok_or("first Dredge decision did not complete")?;
    let first_dredge = events
        .iter()
        .enumerate()
        .skip(first_complete + 1)
        .find_map(|(index, event)| {
            matches!(event,
                GameEvent::Dredged { player: PlayerId(0), card, count: 1 } if *card == source
            )
            .then_some(index)
        })
        .ok_or("first ability draw did not Dredge the exact sacrificed Scarab")?;
    let milled = events
        .iter()
        .enumerate()
        .skip(first_complete + 1)
        .take(first_dredge - first_complete - 1)
        .filter(|(_, event)| {
            matches!(
                event,
                GameEvent::CardMoved {
                    to: Zone::Graveyard,
                    ..
                }
            )
        })
        .count();
    if milled != 1 {
        return Err(format!(
            "Dredge 1 moved {milled} cards to graveyard before returning Scarab"
        ));
    }
    let returned = events.iter().enumerate().skip(first_complete + 1).take(first_dredge - first_complete - 1).any(|(_, event)| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == source));
    if !returned {
        return Err("Dredge did not return the exact Scarab to its owner's hand".to_owned());
    }
    let recast = events
        .iter()
        .enumerate()
        .skip(first_dredge + 1)
        .find_map(|(index, event)| {
            matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == source)
                .then_some(index)
        })
        .ok_or("the exact Dredged Scarab was not recast")?;
    if recast >= activations[1] {
        return Err("the returned Scarab was not recast before its second sacrifice".to_owned());
    }
    let second_complete =
        completed(opened[1].1).ok_or("second Dredge decision did not complete")?;
    if second_complete <= activations[1] {
        return Err("second optional Dredge decision preceded the second activation".to_owned());
    }
    let second_resolved = events
        .iter()
        .enumerate()
        .skip(second_complete + 1)
        .find_map(|(index, event)| {
            matches!(event,
        GameEvent::AbilityResolved { source: card, ability: ABILITY_ID, .. } if *card == source
    ).then_some(index)
        })
        .ok_or("second sacrifice-to-draw ability did not finish")?;
    let drew_normally = events
        .iter()
        .enumerate()
        .skip(second_complete + 1)
        .take(second_resolved - second_complete - 1)
        .any(|(_, event)| {
            matches!(event,
                GameEvent::CardMoved { card, to: Zone::Hand } if *card != source
            )
        });
    if !drew_normally {
        return Err("declining Dredge did not complete an ordinary card draw".to_owned());
    }
    if captured.winner != Some(0)
        || captured.turns > 80
        || captured.accepted_moves > 5_000
        || captured.life_totals[1] > 0
    {
        return Err(format!(
            "the shuffled duel did not reach a bounded pilot win: winner={:?}, life={:?}, turns={}, moves={}",
            captured.winner, captured.life_totals, captured.turns, captured.accepted_moves
        ));
    }
    if captured.battlefield_scarabs.is_empty() {
        return Err(
            "the terminal pilot win did not retain a Grave-Shell Scarab on the battlefield"
                .to_owned(),
        );
    }
    if captured
        .battlefield_scarab_pt
        .iter()
        .any(|pt| *pt != (4, 4))
        || captured.battlefield_scarab_pt.len() != captured.battlefield_scarabs.len()
    {
        return Err(format!(
            "terminal Scarab characteristics differ from 4/4: {:?}",
            captured.battlefield_scarab_pt
        ));
    }
    let resolved_first = events
        .iter()
        .enumerate()
        .skip(first_dredge + 1)
        .find_map(|(index, event)| {
            matches!(event,
        GameEvent::AbilityResolved { source: card, ability: ABILITY_ID, .. } if *card == source
    ).then_some(index)
        })
        .ok_or("first sacrifice ability did not finish after its Dredge replacement")?;
    if resolved_first >= recast {
        return Err("first ability resolution and exact-object recast are out of order".to_owned());
    }
    let relevant = events
        .iter()
        .enumerate()
        .filter(|(_, event)| match event {
            GameEvent::AbilityManaPaid { source: card, .. } => *card == source,
            GameEvent::AbilityActivated { source: card, .. }
            | GameEvent::AbilityResolved { source: card, .. } => *card == source,
            GameEvent::SacrificedAsAbilityCost { permanent, .. } => *permanent == source,
            GameEvent::Dredged { card, .. } => *card == source,
            GameEvent::SpellCast { card, .. } | GameEvent::SpellResolved { card } => {
                *card == source
            }
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                ..
            } => true,
            GameEvent::CardMoved { .. } => true,
            GameEvent::DecisionOpened {
                kind: DecisionKind::DredgeReplacement,
                ..
            }
            | GameEvent::DecisionCompleted {
                kind: DecisionKind::DredgeReplacement,
                ..
            }
            | GameEvent::GameEnded { .. } => true,
            _ => false,
        })
        .map(|(index, event)| format!("{index}: {event:?}"))
        .collect::<Vec<_>>();
    Ok(json!({
        "trace_id": "rav-grave-shell-scarab-activated-draw-dredge-and-decline-full-game",
        "passed": true,
        "deck_p0": "4 Grave-Shell Scarab + 28 Forest + 28 Swamp (60 shuffled cards)",
        "deck_p1": "60 Forest (60 shuffled cards)",
        "shuffle_seed": seed,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": 0,
        "rejected_moves": 0,
        "life_totals": captured.life_totals,
        "scarab_object": format!("{source:?}"),
        "activated_ability": ABILITY_ID,
        "activation_mana_cost": "{1}",
        "dredge_count": 1,
        "dredge_decisions": opened.len(),
        "normal_draw_after_decline": true,
        "battlefield_scarabs": captured.battlefield_scarabs.iter().map(|card| format!("{card:?}")).collect::<Vec<_>>(),
        "event_count": events.len(),
        "event_digest": format!("fnv1a64:{:016x}", event_digest(events)),
        "events": relevant
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read candidate source: {error}"))?;
    if source.len() > 32 * 1024
        || source.contains("include!")
        || source.contains("std::fs")
        || source.contains("std::process")
    {
        return Err("candidate source exceeds the direct-literal module boundary".to_owned());
    }
    let file =
        syn::parse_file(&source).map_err(|_| "candidate source is not valid Rust".to_owned())?;
    let mut definitions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(_) => {}
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                let expected_type = match name.as_str() {
                    "definition" => "CardDefinition",
                    "activated_ability" => "ActivatedAbility",
                    _ => "",
                };
                if expected_type.is_empty()
                    || !function.sig.inputs.is_empty()
                    || !matches!(&function.vis, syn::Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !function
                        .attrs
                        .iter()
                        .all(|attribute| attribute.path().is_ident("must_use"))
                    || function.block.stmts.len() != 1
                    || !matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(structure), None) if structure.path.is_ident(expected_type))
                {
                    return Err(
                        "candidate module may contain only imports and direct literal builders"
                            .to_owned(),
                    );
                }
                definitions.insert(name);
            }
            _ => {
                return Err(
                    "candidate module may contain only imports and direct literal builders"
                        .to_owned(),
                );
            }
        }
    }
    if definitions != BTreeSet::from(["activated_ability".to_owned(), "definition".to_owned()]) {
        return Err(
            "candidate module must define exactly the card and activated-ability literals"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_definition() -> Result<(), String> {
    let definition = candidate_grave_shell_scarab::definition();
    let ability = candidate_grave_shell_scarab::activated_ability();
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost
            != ManaCost::with_colors(2, [Color::Black, Color::Green, Color::Green])
        || definition.colors != BTreeSet::from([Color::Black, Color::Green])
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || (definition.power, definition.toughness) != (Some(4), Some(4))
        || definition.keywords != [Keyword::Dredge(1)]
        || ability.id != ABILITY_ID
        || ability.mana_cost != ManaCost::new(1)
        || ability.tap_cost
        || ability.sorcery_speed
        || !ability.sacrifice_source
        || ability.sacrifice_creatures != 0
        || ability.sacrifice_lands != 0
        || ability.discard_cards != 0
        || !ability.targets.is_empty()
        || ability.effects != [Effect::DrawController]
    {
        return Err("candidate definition or activated ability differs from RAV #211".to_owned());
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
    json!({"collector_number": card.collector_number, "definition_id": definition_id, "name": card.name, "engine_status": engine_status, "qualification": if passed {"diagnostic_trace_passed; independent review pending"} else {"not_evaluated"}, "trace_status": if passed {"passed"} else {"not_qualified"}, "credited": false})
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
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

fn event_digest(events: &[GameEvent]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in format!("{events:?}").bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x00000100000001b3);
    }
    hash
}
