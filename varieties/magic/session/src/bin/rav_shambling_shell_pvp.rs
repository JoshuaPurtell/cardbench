//! Full-game RAV Shambling Shell authority for Dredge 3 and its exact return/cast.

use cardbench_magic_engine::{
    AbilityActivation, CardDefinition, CardType, Color, CounterKind, DeckEntry, DeckList, Effect,
    Game, GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId,
    PolicyAction, Step, Target, Zone,
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
const CARD_ID: &str = "RAV-SHAMBLING-SHELL";
const CARD_NAME: &str = "Shambling Shell";
const OPPONENT_ID: &str = "RAV-WATCHWOLF";
const TARGET_ID: &str = "RAV-WATCHWOLF";
const ABILITY_ID: &str = "shambling-shell-sacrifice-counter";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/shambling_shell.rs";

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/shambling_shell.rs"]
mod candidate_shambling_shell;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Shambling Shell PvP result: {error}");
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
            eprintln!("Shambling Shell PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    let source_guard = validate_compile_valid_side_effect_candidate()?;
    validate_candidate_definition()?;
    validate_registry()?;
    let catalog = unique_catalog()?;
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured != replay || project_events(&captured.events) != project_events(&replay.events) {
        return Err("Shambling Shell shuffled-game replay was not deterministic".to_owned());
    }

    let mutations = json!({
        "missing_dredge_mutation_detected": mutation_detected(seed, Mutation::MissingDredge),
        "wrong_dredge_count_mutation_detected": mutation_detected(seed, Mutation::DredgeTwo),
        "wrong_mana_cost_mutation_detected": mutation_detected(seed, Mutation::WrongCost),
        "wrong_creature_body_mutation_detected": mutation_detected(seed, Mutation::WrongBody),
        "missing_sacrifice_counter_binding_mutation_detected": mutation_detected(seed, Mutation::RemoveActivation),
        "missing_source_sacrifice_cost_mutation_detected": mutation_detected(seed, Mutation::NoSourceSacrifice),
        "wrong_sacrifice_counter_effect_mutation_detected": mutation_detected(seed, Mutation::WrongActivationEffect),
        "compile_valid_side_effect_candidate_rejected_before_candidate_calls": source_guard["compile_valid_side_effect_candidate_rejected_before_candidate_calls"] == true,
        "sealed_mutation_authority": false
    });
    for key in [
        "missing_dredge_mutation_detected",
        "wrong_dredge_count_mutation_detected",
        "wrong_mana_cost_mutation_detected",
        "wrong_creature_body_mutation_detected",
        "missing_sacrifice_counter_binding_mutation_detected",
        "missing_source_sacrifice_cost_mutation_detected",
        "wrong_sacrifice_counter_effect_mutation_detected",
        "compile_valid_side_effect_candidate_rejected_before_candidate_calls",
    ] {
        if mutations[key] != true {
            return Err(format!("Shambling Shell semantic mutation escaped: {key}"));
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
        "card": {"collector_number": 230, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117: priority and turn-based draw decisions",
                "202.1-202.2: colored mana cost and color identity",
                "302.1-302.2: creature characteristics and combat",
                "601.2: casting and paying for the creature spell",
                "702.52: Dredge replacement of a draw",
                "510: combat damage and creature deaths",
                "704: state-based actions after combat damage"
            ],
            "source_contract": "Original RAV #230 printing: Shambling Shell costs {1}{B}{G}, is a black-green 3/1 Creature — Plant Zombie, has 'Sacrifice Shambling Shell: Put a +1/+1 counter on target creature,' and has Dredge 3. Creature subtype is retained identity but not asserted by CardDefinition.",
            "oracle_reconciliation": "Current Oracle says 'this creature' and 'mill three cards' in the Dredge reminder; those terms preserve the printed source sacrifice, target-counter effect, and three-card draw replacement.",
            "authority_cutoff": "2026-09-24",
            "no_provider_trial": true
        },
        "source_guard": source_guard,
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
    RemoveActivation,
    NoSourceSacrifice,
    WrongActivationEffect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    life_totals: [i64; 2],
    shell: Option<ObjectId>,
    target: Option<ObjectId>,
    shell_pt: Option<(i32, i32)>,
    shell_zone: Option<Zone>,
    target_counter_count: i16,
    target_pt: Option<(i32, i32)>,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    let requested_seed = std::env::var("CARDBENCH_SHAMBLING_SHELL_SEED")
        .ok()
        .and_then(|value| value.parse().ok());
    let seeds: Vec<u64> = requested_seed.map_or_else(|| (0..=64).collect(), |seed| vec![seed]);
    let mut last_error = "no seed attempted".to_owned();
    for seed in seeds {
        match run_match(seed, Mutation::None) {
            Ok(captured) => match validate_trace(&captured, seed) {
                Ok(_) => return Ok((seed, captured)),
                Err(error) => last_error = format!("seed {seed}: {error}"),
            },
            Err(error) => last_error = format!("seed {seed}: {error}"),
        }
    }
    Err(format!(
        "no deterministic Shambling Shell PvP trace found: {last_error}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation) {
        Ok(captured) => validate_trace(&captured, seed).is_err(),
        Err(_) => true,
    }
}

fn definition_for(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_shambling_shell::definition();
    match mutation {
        Mutation::None => {}
        Mutation::MissingDredge => definition.keywords.clear(),
        Mutation::DredgeTwo => definition.keywords = vec![Keyword::Dredge(2)],
        Mutation::WrongCost => definition.mana_cost = ManaCost::with_colors(2, [Color::Green]),
        Mutation::WrongBody => {
            definition.power = Some(3);
            definition.toughness = Some(2);
        }
        Mutation::RemoveActivation
        | Mutation::NoSourceSacrifice
        | Mutation::WrongActivationEffect => {}
    }
    definition
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let pilot_deck = DeckList {
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
                card: "RAV-FOREST".to_owned(),
                count: 26,
            },
            DeckEntry {
                card: "RAV-SWAMP".to_owned(),
                count: 13,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 13,
            },
        ],
        sideboard: vec![],
    };
    let opponent_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 30,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 30,
            },
        ],
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
        .ok_or("Shambling Shell missing from the RAV registry")?;
    *registered = definition_for(mutation);
    let shell_cost = registered.mana_cost.clone();
    let watchwolf_cost = definitions
        .iter()
        .find(|card| card.id == TARGET_ID)
        .map(|card| card.mana_cost.clone())
        .ok_or("Watchwolf missing from the RAV registry")?;
    let mut activated_abilities = rav_activated_ability_bindings();
    match mutation {
        Mutation::RemoveActivation => activated_abilities.retain(|binding| {
            binding.card_definition != CARD_ID || binding.ability.id != ABILITY_ID
        }),
        Mutation::WrongActivationEffect => {
            if let Some(binding) = activated_abilities.iter_mut().find(|binding| {
                binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID
            }) {
                binding.ability.effects = vec![Effect::DrawController];
            }
        }
        Mutation::NoSourceSacrifice => {
            if let Some(binding) = activated_abilities.iter_mut().find(|binding| {
                binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID
            }) {
                binding.ability.sacrifice_source = false;
            }
        }
        _ => {}
    }
    let triggers = rav_triggered_ability_bindings();
    let mut game = Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        activated_abilities,
        triggers,
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
        Box::new(ShellPolicy::new(
            PlayerId(0),
            true,
            shell_cost.clone(),
            watchwolf_cost.clone(),
        )),
        Box::new(ShellPolicy::new(
            PlayerId(1),
            false,
            shell_cost,
            watchwolf_cost,
        )),
    ];
    let mut accepted_moves = 0_u32;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted_moves >= config.max_policy_moves {
            return Err("Shambling Shell PvP match exceeded its development bound".to_owned());
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
    let shell = game.event_log.iter().find_map(|event| match event {
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
    let target = game.event_log.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|definition| definition.id == TARGET_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let shell_pt = shell
        .and_then(|card| game.characteristics(card).ok())
        .and_then(|characteristics| Some((characteristics.power?, characteristics.toughness?)));
    let shell_zone = shell.and_then(|card| game.zone_of(card));
    let target_counter_count = target
        .and_then(|card| game.object(card).ok())
        .and_then(|object| object.counters.get(&CounterKind::PlusOnePlusOne).copied())
        .unwrap_or(0);
    let target_pt = target
        .and_then(|card| game.characteristics(card).ok())
        .and_then(|characteristics| Some((characteristics.power?, characteristics.toughness?)));
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
        shell,
        target,
        shell_pt,
        shell_zone,
        target_counter_count,
        target_pt,
    })
}

struct ShellPolicy {
    player: PlayerId,
    pilot: bool,
    inner: RavCatalogPolicy,
    shell_cost: ManaCost,
    watchwolf_cost: ManaCost,
    sacrificed_source: Option<ObjectId>,
    recovered_source: Option<ObjectId>,
    activation_offered: bool,
    dredge_used: bool,
}

impl ShellPolicy {
    fn new(player: PlayerId, pilot: bool, shell_cost: ManaCost, watchwolf_cost: ManaCost) -> Self {
        Self {
            player,
            pilot,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
            shell_cost,
            watchwolf_cost,
            sacrificed_source: None,
            recovered_source: None,
            activation_offered: false,
            dredge_used: false,
        }
    }
}

impl CodePolicy for ShellPolicy {
    fn id(&self) -> &'static str {
        if self.pilot {
            "rav.shambling-shell-dredge-pilot.v1"
        } else {
            "rav.shambling-shell-watchwolf-opponent.v1"
        }
    }

    fn propose_draw_replacement(&mut self, view: &GameView) -> PolicyAction {
        if self.pilot {
            let candidate = (!self.dredge_used)
                .then_some(self.sacrificed_source)
                .flatten()
                .and_then(|source| view.dredge_candidates.iter().find(|card| card.id == source));
            if let Some(candidate) = candidate {
                self.recovered_source = Some(candidate.id);
                self.dredge_used = true;
                return PolicyAction::Draw {
                    decision: view
                        .draw_replacement_decision
                        .expect("live draw replacement has an identity"),
                    dredge: Some(candidate.id),
                };
            }
        }
        self.inner.propose_draw_replacement(view)
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = if self.pilot {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.can_attack)
                    .map(|card| card.id)
                    .collect()
            } else {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(OPPONENT_ID) && card.can_attack)
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
        if self.pilot
            && !self.activation_offered
            && view.active_player == self.player
            && view.step == Step::PrecombatMain
            && view.stack_depth == 0
        {
            let source = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
                .map(|card| card.id);
            let target = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(TARGET_ID))
                .map(|card| card.id);
            if let (Some(source), Some(target)) = (source, target) {
                self.activation_offered = true;
                self.sacrificed_source = Some(source);
                return PolicyAction::ActivateAbility {
                    activation: AbilityActivation {
                        source,
                        ability_id: ABILITY_ID,
                        sacrifice_sources: vec![source],
                        additional_tap_creatures: vec![],
                        discard_cards: vec![],
                        targets: vec![Target::Permanent(target)],
                    },
                };
            }
        }

        let action = self.inner.propose_move(view);
        match action {
            PolicyAction::Cast(request) => {
                let cost = view
                    .hand
                    .iter()
                    .find(|card| card.id == request.card)
                    .and_then(|card| match card.definition {
                        Some(CARD_ID) => Some(&self.shell_cost),
                        Some(TARGET_ID) => Some(&self.watchwolf_cost),
                        _ => None,
                    });
                if let Some(selection) = cost.and_then(|cost| payment_selection(view, cost)) {
                    PolicyAction::CastWithPayment {
                        request,
                        chosen_x: None,
                        mana_selection: selection,
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
    let shell = captured.shell.ok_or("pilot never cast Shambling Shell")?;
    let target = captured
        .target
        .ok_or("pilot never cast its Watchwolf counter target")?;
    let shell_casts: Vec<usize> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == shell)
                .then_some(index)
        })
        .collect();
    let target_cast = events
        .iter()
        .position(|event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == target))
        .ok_or("Watchwolf target was not cast")?;
    let shell_resolutions: Vec<usize> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            matches!(event, GameEvent::SpellResolved { card } if *card == shell).then_some(index)
        })
        .collect();
    let target_resolved = events
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == target))
        .ok_or("Watchwolf target spell did not resolve")?;
    if shell_casts.len() < 2 || shell_resolutions.len() < 2 {
        return Err("the exact sacrificed Shell returned by Dredge was not recast".to_owned());
    }
    let shell_payments: Vec<Vec<Color>> = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if *card == shell => Some(colors.clone()),
            _ => None,
        })
        .collect();
    let target_payments: Vec<Vec<Color>> = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if *card == target => Some(colors.clone()),
            _ => None,
        })
        .collect();
    let pays_shell = |payment: &[Color]| {
        payment.len() == 3
            && payment
                .iter()
                .filter(|color| **color == Color::Black)
                .count()
                >= 1
            && payment
                .iter()
                .filter(|color| **color == Color::Green)
                .count()
                >= 1
    };
    let pays_target = |payment: &[Color]| {
        payment.len() == 3
            && payment
                .iter()
                .filter(|color| **color == Color::Green)
                .count()
                >= 1
            && payment
                .iter()
                .filter(|color| **color == Color::White)
                .count()
                >= 1
    };
    if shell_payments.len() < 2
        || !shell_payments
            .iter()
            .take(2)
            .all(|payment| pays_shell(payment))
        || !target_payments.iter().any(|payment| pays_target(payment))
    {
        return Err(format!(
            "colored cast payments do not show Shell {{1}}{{B}}{{G}} twice and Watchwolf {{1}}{{G}}{{W}} once: shell={shell_payments:?}, target={target_payments:?}"
        ));
    }
    let sacrificed = events
        .iter()
        .position(|event| matches!(event, GameEvent::SacrificedAsAbilityCost { player: PlayerId(0), source, permanent } if *source == shell && *permanent == shell))
        .ok_or("Shell was not sacrificed as the activated ability's exact cost")?;
    let source_moved = events
        .iter()
        .enumerate()
        .skip(sacrificed + 1)
        .find_map(|(index, event)| {
            matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == shell)
                .then_some(index)
        })
        .ok_or("sacrificed Shell did not move to its owner's graveyard")?;
    let activated = events
        .iter()
        .enumerate()
        .skip(source_moved + 1)
        .find_map(|(index, event)| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source, definition: CARD_ID, ability: ABILITY_ID, .. } if *source == shell).then_some(index))
        .ok_or("Shell sacrifice ability did not enter the stack")?;
    let resolved = events
        .iter()
        .enumerate()
        .skip(activated + 1)
        .find_map(|(index, event)| matches!(event, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == shell).then_some(index))
        .ok_or("Shell's counter ability did not resolve")?;
    let counter = events
        .iter()
        .enumerate()
        .skip(activated + 1)
        .find_map(|(index, event)| matches!(event, GameEvent::CounterPlaced { source, card, counter: CounterKind::PlusOnePlusOne, amount: 1 } if *source == shell && *card == target).then_some(index))
        .ok_or("Shell did not put one +1/+1 counter on the exact Watchwolf target")?;
    let dredges: Vec<(usize, u8)> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::Dredged {
                player: PlayerId(0),
                card,
                count,
            } if *card == shell => Some((index, *count)),
            _ => None,
        })
        .collect();
    if dredges.len() != 1 || dredges[0].1 != 3 {
        return Err(format!(
            "expected exactly one Shambling Shell Dredge 3: {dredges:?}"
        ));
    }
    let dredge = dredges[0].0;
    let mills = validate_dredge_suffix(events, dredge, shell)?;
    if !(shell_casts[0] < shell_resolutions[0]
        && target_cast < target_resolved
        && target_resolved < sacrificed
        && sacrificed < source_moved
        && source_moved < activated
        && activated < counter
        && counter < resolved
        && counter < dredge
        && dredge < shell_casts[1]
        && shell_casts[1] < shell_resolutions[1])
    {
        return Err("cast, source-sacrifice, counter resolution, Dredge, and same-object recast order differs".to_owned());
    }
    if captured.target_counter_count != 1 || captured.target_pt != Some((4, 4)) {
        return Err(format!(
            "Watchwolf did not retain exactly one +1/+1 counter: count={}, pt={:?}",
            captured.target_counter_count, captured.target_pt
        ));
    }
    if captured.shell_pt != Some((3, 1)) || captured.shell_zone != Some(Zone::Battlefield) {
        return Err(format!(
            "terminal Shell characteristics/zone differ: pt={:?}, zone={:?}",
            captured.shell_pt, captured.shell_zone
        ));
    }
    let target_damage = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if *source == target => Some(*amount),
            _ => None,
        })
        .sum::<i32>();
    if target_damage < 20 {
        return Err(format!(
            "countered Watchwolf did not deal lethal player damage: {target_damage}"
        ));
    }
    if captured.winner != Some(0) || captured.turns > 80 || captured.accepted_moves > 5_000 {
        return Err(format!(
            "duel did not reach bounded pilot win: winner={:?}, turns={}, moves={}",
            captured.winner, captured.turns, captured.accepted_moves
        ));
    }
    if captured.life_totals[1] > 0 {
        return Err(format!(
            "opponent life did not reach zero: {:?}",
            captured.life_totals
        ));
    }
    let relevant: Vec<String> = events
        .iter()
        .enumerate()
        .filter(|(index, event)| {
            [shell_casts[0], shell_casts[1], target_cast, sacrificed, source_moved, activated, resolved, counter, dredge].contains(index)
                || matches!(event, GameEvent::SpellResolved { card } if *card == shell || *card == target)
                || matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), .. } if *source == target)
                || matches!(event, GameEvent::GameEnded { .. })
        })
        .map(|(index, event)| format!("{index}: {event:?}"))
        .collect();
    Ok(json!({
        "trace_id": "rav-shambling-shell-sacrifice-counter-dredge-full-game",
        "deck_p0": "4 Shambling Shell + 4 Watchwolf + 26 Forest + 13 Swamp + 13 Plains (60 shuffled cards)",
        "deck_p1": "30 Forest + 30 Plains (60 shuffled cards)",
        "shuffle_seed": seed,
        "turns": captured.turns,
        "accepted_policy_moves": captured.accepted_moves,
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": 0,
        "rejected_moves": 0,
        "life_totals": captured.life_totals,
        "shell_object": format!("{shell:?}"),
        "watchwolf_target_object": format!("{target:?}"),
        "shell_cast_payments": shell_payments.iter().take(2).map(|colors| colors.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>()).collect::<Vec<_>>(),
        "watchwolf_cast_payments": target_payments.iter().map(|colors| colors.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>()).collect::<Vec<_>>(),
        "sacrifice_as_cost_event_index": sacrificed,
        "ability_activated_event_index": activated,
        "ability_resolved_event_index": resolved,
        "counter_placed_event_index": counter,
        "dredge_event_index": dredge,
        "target_counter_count": captured.target_counter_count,
        "target_power": captured.target_pt.map(|pt| pt.0),
        "target_toughness": captured.target_pt.map(|pt| pt.1),
        "target_combat_damage": target_damage,
        "dredge_count": 3,
        "shell_power": captured.shell_pt.map(|pt| pt.0),
        "shell_toughness": captured.shell_pt.map(|pt| pt.1),
        "shell_zone": format!("{:?}", captured.shell_zone),
        "mill_object_ids": mills.iter().map(|id| format!("{id:?}")).collect::<Vec<_>>(),
        "event_count": events.len(),
        "event_digest": format!("fnv1a64:{:016x}", event_digest(events)),
        "events": relevant
    }))
}

fn validate_dredge_suffix(
    events: &[GameEvent],
    dredge_index: usize,
    shell: ObjectId,
) -> Result<Vec<ObjectId>, String> {
    if dredge_index < 8 {
        return Err("Dredge event has too little preceding zone provenance".to_owned());
    }
    let suffix = &events[dredge_index - 8..=dredge_index];
    let mut mills = Vec::new();
    for pair in suffix[..6].chunks_exact(2) {
        match pair {
            [
                GameEvent::CardMoved {
                    card,
                    to: Zone::Graveyard,
                },
                GameEvent::ObjectIncarnationAdvanced {
                    object,
                    incarnation: _,
                },
            ] if card == object => mills.push(*card),
            _ => {
                return Err(format!(
                    "Dredge did not mill three exact objects in order: {pair:?}"
                ));
            }
        }
    }
    match &suffix[6..] {
        [
            GameEvent::CardMoved {
                card,
                to: Zone::Hand,
            },
            GameEvent::ObjectIncarnationAdvanced {
                object,
                incarnation: _,
            },
            GameEvent::Dredged {
                player: PlayerId(0),
                card: dredged,
                count: 3,
            },
        ] if *card == shell && *object == shell && *dredged == shell => {}
        other => {
            return Err(format!(
                "Dredge did not return the same object to hand: {other:?}"
            ));
        }
    }
    if mills.len() != 3
        || mills.contains(&shell)
        || mills.iter().copied().collect::<BTreeSet<_>>().len() != 3
    {
        return Err(format!(
            "Dredge mill objects are not three distinct cards: {mills:?}"
        ));
    }
    Ok(mills)
}

const CANONICAL_CANDIDATE_SOURCE: &str = r#"usecardbench_magic_engine::{ActivatedAbility,ActivatedAbilityBinding,CardDefinition,CardType,Color,Effect,Keyword,ManaCost,TargetRequirement,};usestd::collections::BTreeSet;#[must_use]pub(crate)fndefinition()->CardDefinition{CardDefinition{id:"RAV-SHAMBLING-SHELL",name:"Shambling Shell",set_code:"RAV",mana_cost:ManaCost::with_colors(1,[Color::Black,Color::Green]),colors:BTreeSet::from([Color::Black,Color::Green]),mana_colors:BTreeSet::new(),card_types:BTreeSet::from([CardType::Creature]),is_basic_land:false,supported_rules:&["full-rules-fidelity","dredge","base-characteristics","sacrifice-source-target-counter",],power:Some(3),toughness:Some(1),keywords:vec![Keyword::Dredge(3)],effects:vec![],}}#[must_use]pub(crate)fnactivated_ability()->ActivatedAbilityBinding{ActivatedAbilityBinding{card_definition:"RAV-SHAMBLING-SHELL",ability:ActivatedAbility{id:"shambling-shell-sacrifice-counter",mana_cost:ManaCost::new(0),tap_cost:false,sorcery_speed:false,additional_tap_creatures:0,sacrifice_source:true,sacrifice_creatures:0,sacrifice_lands:0,discard_cards:0,targets:vec![TargetRequirement::Creature],effects:vec![Effect::AddPlusOneCounterToTarget],},}}"#;

fn normalized_source(source: &str) -> String {
    let mut normalized = String::new();
    let mut in_string = false;
    let mut escaped = false;
    for character in source.chars() {
        if in_string {
            normalized.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
        } else if character == '"' {
            in_string = true;
            normalized.push(character);
        } else if !character.is_whitespace() {
            normalized.push(character);
        }
    }
    normalized
}

fn function_returns_literal(item: &Item, name: &str, struct_name: &str) -> bool {
    matches!(item, Item::Fn(function)
        if function.sig.ident == name
            && function.sig.inputs.is_empty()
            && matches!(&function.vis, syn::Visibility::Restricted(vis) if vis.path.is_ident("crate"))
            && function.block.stmts.len() == 1
            && matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(value), None) if value.path.is_ident(struct_name) && value.rest.is_none()))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Shambling Shell candidate source: {error}"))?;
    validate_candidate_source_text(&source)
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    if source.len() > 32 * 1024 {
        return Err("Shambling Shell source exceeds the candidate source boundary".to_owned());
    }
    let file = syn::parse_file(source)
        .map_err(|_| "Shambling Shell candidate source is not valid Rust".to_owned())?;
    if file.attrs.len() != 0 || file.items.len() != 4 {
        return Err(
            "Shambling Shell module must contain only its imports and two literal factories"
                .to_owned(),
        );
    }
    let [Item::Use(_), Item::Use(_), definition, activation] = file.items.as_slice() else {
        return Err("Shambling Shell module top-level shape changed".to_owned());
    };
    if !function_returns_literal(definition, "definition", "CardDefinition") {
        return Err(
            "Shambling Shell definition must be a direct crate-visible CardDefinition literal"
                .to_owned(),
        );
    }
    if !function_returns_literal(activation, "activated_ability", "ActivatedAbilityBinding") {
        return Err("Shambling Shell activation must be a direct crate-visible ActivatedAbilityBinding literal".to_owned());
    }
    let Item::Fn(activation) = activation else {
        unreachable!()
    };
    let Stmt::Expr(Expr::Struct(binding), None) = &activation.block.stmts[0] else {
        unreachable!()
    };
    let ability = binding
        .fields
        .iter()
        .find(|field| matches!(&field.member, syn::Member::Named(name) if name == "ability"))
        .ok_or("Shambling Shell binding literal has no ability field")?;
    if !matches!(&ability.expr, Expr::Struct(value) if value.path.is_ident("ActivatedAbility") && value.rest.is_none())
    {
        return Err("Shambling Shell activated ability must be a direct literal".to_owned());
    }
    if normalized_source(source) != CANONICAL_CANDIDATE_SOURCE {
        return Err(
            "Shambling Shell normalized source differs from its exact literal contract".to_owned(),
        );
    }
    Ok(())
}

fn validate_compile_valid_side_effect_candidate() -> Result<Value, String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Shambling Shell candidate source: {error}"))?;
    let injected = format!(
        "{source}\nfn injected_side_effect() {{ let _ = std::fs::write(std::env::temp_dir().join(\"cardbench-side-effect\"), b\"mutation\"); }}\n"
    );
    let compile_valid_by_construction = syn::parse_file(&injected).is_ok();
    let rejected = validate_candidate_source_text(&injected).is_err();
    if !compile_valid_by_construction || !rejected {
        return Err(
            "compile-valid side-effect candidate did not fail the pre-call source gate".to_owned(),
        );
    }
    Ok(json!({
        "compile_valid_side_effect_candidate_by_construction": true,
        "compile_valid_side_effect_candidate_rejected_before_candidate_calls": true,
        "candidate_factories_called_for_injected_source": false,
        "sealed_candidate_attestation": false
    }))
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: CARD_NAME,
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Black, Color::Green]),
        colors: BTreeSet::from([Color::Black, Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "dredge",
            "base-characteristics",
            "sacrifice-source-target-counter",
        ],
        power: Some(3),
        toughness: Some(1),
        keywords: vec![Keyword::Dredge(3)],
        effects: vec![],
    }
}

fn expected_activated_ability() -> cardbench_magic_engine::ActivatedAbilityBinding {
    cardbench_magic_engine::ActivatedAbilityBinding {
        card_definition: CARD_ID,
        ability: cardbench_magic_engine::ActivatedAbility {
            id: ABILITY_ID,
            mana_cost: ManaCost::new(0),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: true,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![cardbench_magic_engine::TargetRequirement::Creature],
            effects: vec![Effect::AddPlusOneCounterToTarget],
        },
    }
}

fn validate_candidate_definition() -> Result<(), String> {
    let definition = candidate_shambling_shell::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Shambling Shell is absent from the RAV card registry")?;
    let ability = candidate_shambling_shell::activated_ability();
    let registered_ability = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("Shambling Shell activated ability is absent from the RAV registry")?;
    if definition != expected_definition() || definition != registered {
        return Err("Shambling Shell candidate definition differs from the exact RAV source contract or registry".to_owned());
    }
    if ability != expected_activated_ability() || ability != registered_ability {
        return Err("Shambling Shell candidate activated ability differs from the exact source contract or registry".to_owned());
    }
    Ok(())
}

fn validate_registry() -> Result<(), String> {
    if rav_triggered_ability_bindings()
        .iter()
        .any(|binding| binding.card_definition == CARD_ID)
    {
        return Err("Shambling Shell has an unexpected triggered-ability binding".to_owned());
    }
    Ok(())
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
        || !printings
            .iter()
            .any(|card| card.collector_number == 230 && card.name == CARD_NAME)
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
    json!({
        "collector_number": card.collector_number,
        "definition_id": definition_id,
        "name": card.name,
        "engine_status": engine_status,
        "qualification": if passed {"diagnostic_trace_passed; independent review pending"} else {"not_evaluated"},
        "trace_status": if passed {"passed"} else {"not_qualified"},
        "credited": false
    })
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
