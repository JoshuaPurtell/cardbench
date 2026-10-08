//! Full-game RAV Stinkweed Imp authority for Dredge 5 and its exact combat-damage recipient.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CombatBlock, DeckEntry, DeckList, Effect, Game, GameEvent,
    GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step,
    TriggerCondition, Zone,
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
use std::fmt::Write as _;
use std::process::ExitCode;
use syn::{Expr, Item, ReturnType, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-STINKWEED-IMP";
const CARD_NAME: &str = "Stinkweed Imp";
const TARGET_ID: &str = "RAV-WATCHWOLF";
const TRIGGER_ID: &str = "destroy-combat-damaged-creature";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/stinkweed_imp.rs";
const SEED: u64 = 0;
const MAX_TURNS: u32 = 80;
const MAX_MOVES: u32 = 5_000;
const EXPECTED_CANDIDATE_SOURCE: &str = r#"//! Candidate-owned Stinkweed Imp definition and combat-damage trigger.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

/// RAV #107's black 1/2 Imp with Flying and Dredge 5.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-STINKWEED-IMP",
        name: "Stinkweed Imp",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Black]),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "dredge",
            "base-characteristics",
            "flying",
            "combat-damage-destroy-recipient",
        ],
        power: Some(1),
        toughness: Some(2),
        keywords: vec![Keyword::Flying, Keyword::Dredge(5)],
        effects: vec![],
    }
}

/// Destroys the exact creature Stinkweed Imp damaged in combat.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-STINKWEED-IMP",
        ability: TriggeredAbility {
            id: "destroy-combat-damaged-creature",
            condition: TriggerCondition::DealsCombatDamageToCreature,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::DestroyCombatDamagedCreature],
        },
    }
}
"#;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/stinkweed_imp.rs"]
mod candidate_stinkweed_imp;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
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
            eprintln!("Stinkweed Imp authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    // This literal-source gate is intentionally the first operation before any candidate call.
    validate_candidate_source()?;
    let malicious_side_effect_rejected = malicious_candidate_source_rejected()?;
    if !malicious_side_effect_rejected {
        return Err("compile-valid side-effect candidate escaped the pre-call source gate".into());
    }
    validate_candidate_definition()?;
    validate_candidate_binding()?;
    let catalog = unique_catalog()?;
    let captured = run_match(Mutation::None)?;
    let trace = validate_trace(&captured)?;
    let replay = run_match(Mutation::None)?;
    if captured != replay || project_events(&captured.events) != project_events(&replay.events) {
        return Err("Stinkweed Imp full-game replay was not deterministic".into());
    }

    let mutations = json!({
        "wrong_mana_cost_detected_in_game": mutation_detected(Mutation::WrongCost),
        "wrong_body_detected_in_game": mutation_detected(Mutation::WrongBody),
        "flying_removed_detected_in_game": mutation_detected(Mutation::NoFlying),
        "dredge_removed_detected_in_game": mutation_detected(Mutation::NoDredge),
        "wrong_dredge_count_detected_in_game": mutation_detected(Mutation::DredgeFour),
        "combat_trigger_removed_detected_in_game": mutation_detected(Mutation::NoTrigger),
        "combat_trigger_condition_changed_detected_in_game": mutation_detected(Mutation::WrongTriggerCondition),
        "combat_trigger_effect_changed_detected_in_game": mutation_detected(Mutation::WrongTriggerEffect),
        "compile_valid_candidate_side_effect_rejected_before_execution": malicious_side_effect_rejected,
        "sealed_mutation_authority": false
    });
    for key in [
        "wrong_mana_cost_detected_in_game",
        "wrong_body_detected_in_game",
        "flying_removed_detected_in_game",
        "dredge_removed_detected_in_game",
        "wrong_dredge_count_detected_in_game",
        "combat_trigger_removed_detected_in_game",
        "combat_trigger_condition_changed_detected_in_game",
        "combat_trigger_effect_changed_detected_in_game",
        "compile_valid_candidate_side_effect_rejected_before_execution",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Stinkweed Imp semantic mutation escaped the game oracle: {key}"
            ));
        }
    }
    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|row| row["trace_status"] == "passed")
        .count();
    if cards_passed != 1 {
        return Err("Stinkweed Imp trace must bind exactly one RAV catalog identity".into());
    }
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 107, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117: priority and draw replacement decisions",
                "202.1-202.2: mana cost and color",
                "302: creature characteristics",
                "510: combat damage",
                "603: triggered abilities and captured damage recipients",
                "608: resolving triggered abilities",
                "704: state-based actions",
                "702.9: Flying",
                "702.52: Dredge"
            ],
            "source_contract": "RAV Stinkweed Imp #107 is a black {2}{B} 1/2 Creature — Imp with Flying and Dredge 5; when it deals combat damage to a creature, destroy that creature. Imp is retained identity but its subtype is not represented by CardDefinition.",
            "oracle_reconciliation": "Current Oracle changes the trigger subject from the card name to this creature without changing the recipient or effect; Flying and Dredge 5 are unchanged.",
            "authority_cutoff": "2026-09-24",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": rav_main_set_catalog().len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "unreviewed_diagnostic",
        "deterministic_replay": true,
        "candidate_source_validation": {
            "candidate_ast_contract_matched": true,
            "compile_valid_injected_side_effect_candidate_rejected": malicious_side_effect_rejected,
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
    WrongCost,
    WrongBody,
    NoFlying,
    NoDredge,
    DredgeFour,
    NoTrigger,
    WrongTriggerCondition,
    WrongTriggerEffect,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    life_totals: [i64; 2],
    imp: Option<ObjectId>,
    target: Option<ObjectId>,
    imp_power: Option<i32>,
    imp_toughness: Option<i32>,
    imp_flying: bool,
    imp_dredge: Option<u8>,
    imp_zone: Option<Zone>,
    dredge_library_before: Option<Vec<ObjectId>>,
    dredge_library_after: Option<Vec<ObjectId>>,
}

fn run_match(mutation: Mutation) -> Result<CapturedMatch, String> {
    let pilot_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: TARGET_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-SWAMP".into(),
                count: 26,
            },
            DeckEntry {
                card: "RAV-FOREST".into(),
                count: 26,
            },
        ],
        sideboard: vec![],
    };
    let opponent_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: TARGET_ID.into(),
                count: 1,
            },
            DeckEntry {
                card: "RAV-FOREST".into(),
                count: 29,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 30,
            },
        ],
        sideboard: vec![],
    };
    for (name, deck) in [("pilot", &pilot_deck), ("opponent", &opponent_deck)] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{name} deck has {count} cards; expected 60"));
        }
    }

    let definition = mutated_definition(mutation);
    let cost = definition.mana_cost.clone();
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Stinkweed Imp is absent from the RAV registry")?;
    *registered = definition;
    let mut triggers = rav_triggered_ability_bindings();
    let trigger_index = triggers
        .iter()
        .position(|binding| binding.card_definition == CARD_ID && binding.ability.id == TRIGGER_ID);
    match mutation {
        Mutation::NoTrigger => {
            if let Some(index) = trigger_index {
                triggers.remove(index);
            }
        }
        Mutation::WrongTriggerCondition | Mutation::WrongTriggerEffect => {
            let index = trigger_index.ok_or("Stinkweed Imp combat binding is absent")?;
            triggers[index] = mutated_binding(mutation);
        }
        _ => {}
    }
    if !matches!(
        mutation,
        Mutation::NoTrigger | Mutation::WrongTriggerCondition | Mutation::WrongTriggerEffect
    ) {
        let index = trigger_index.ok_or("Stinkweed Imp combat binding is absent")?;
        triggers[index] = candidate_stinkweed_imp::triggered_ability();
    }
    let mut game = Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers,
    )
    .map_err(|error| error.to_string())?;
    let config = DeckMatchConfig {
        shuffle_seed: SEED,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_MOVES,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(SEED)
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
        Box::new(ImpPolicy::new(PlayerId(0), true, cost.clone())),
        Box::new(ImpPolicy::new(PlayerId(1), false, cost)),
    ];
    let mut captured = CapturedMatch {
        events: vec![],
        winner: None,
        turns: 0,
        accepted: 0,
        rejected: 0,
        life_totals: [20, 20],
        imp: None,
        target: None,
        imp_power: None,
        imp_toughness: None,
        imp_flying: false,
        imp_dredge: None,
        imp_zone: None,
        dredge_library_before: None,
        dredge_library_after: None,
    };
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || captured.accepted >= MAX_MOVES {
            return Err(format!(
                "Stinkweed Imp match exceeded bounds at turn {} {:?}",
                game.turn, game.step
            ));
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
        let draws_with_imp = matches!(&action, PolicyAction::Draw { dredge: Some(card), .. }
            if game.card_definition(*card).is_ok_and(|definition| definition.id == CARD_ID));
        let library_before = draws_with_imp.then(|| game.players[0].library.clone());
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "policy move rejected at turn {} {:?} for {:?}: {error}",
                    view.turn, view.step, player
                )
            })?;
        captured.accepted += 1;
        if let Some(before) = library_before {
            captured.dredge_library_before = Some(before);
            captured.dredge_library_after = Some(game.players[0].library.clone());
        }
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
    }
    captured.events = game.event_log.clone();
    captured.winner = game.winner().map(|player| player.0);
    captured.turns = game.turn;
    captured.imp = captured.events.iter().find_map(|event| match event {
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
    captured.target = captured.events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(1),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|definition| definition.id == TARGET_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    if let Some(imp) = captured.imp {
        let characteristics = game
            .characteristics(imp)
            .map_err(|error| error.to_string())?;
        captured.imp_power = characteristics.power;
        captured.imp_toughness = characteristics.toughness;
        captured.imp_flying = characteristics.keywords.contains(&Keyword::Flying);
        captured.imp_dredge = characteristics
            .keywords
            .iter()
            .find_map(|keyword| match keyword {
                Keyword::Dredge(amount) => Some(*amount),
                _ => None,
            });
        captured.imp_zone = game.zone_of(imp);
    }
    captured.life_totals = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    Ok(captured)
}

struct ImpPolicy {
    player: PlayerId,
    pilot: bool,
    inner: RavCatalogPolicy,
    imp_cost: ManaCost,
    initial_imp: Option<ObjectId>,
    blocked_imp: Option<ObjectId>,
    dredged_imp: Option<ObjectId>,
    initial_cast: bool,
    recast: bool,
}

impl ImpPolicy {
    fn new(player: PlayerId, pilot: bool, imp_cost: ManaCost) -> Self {
        Self {
            player,
            pilot,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
            imp_cost,
            initial_imp: None,
            blocked_imp: None,
            dredged_imp: None,
            initial_cast: false,
            recast: false,
        }
    }

    fn candidate_id_to_cast(&self) -> Option<ObjectId> {
        if self.pilot && !self.initial_cast {
            return None;
        }
        self.dredged_imp.filter(|_| !self.recast)
    }

    fn take_land_or_mana_action(&mut self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played == 0 {
            let wanted = if view.mana_pool.amount(Color::Black) == 0 {
                "RAV-SWAMP"
            } else {
                "RAV-FOREST"
            };
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(wanted))
            {
                return Some(PolicyAction::PlayLand { card: land.id });
            }
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
            {
                return Some(PolicyAction::PlayLand { card: land.id });
            }
        }
        let need_black = view.mana_pool.amount(Color::Black) == 0;
        let land = view
            .own_battlefield
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land)
                    && !card.tapped
                    && (!need_black || card.definition == Some("RAV-SWAMP"))
            })
            .or_else(|| {
                view.own_battlefield
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land) && !card.tapped)
            })?;
        let color = match land.definition {
            Some("RAV-SWAMP") => Color::Black,
            Some("RAV-FOREST") => Color::Green,
            Some("RAV-PLAINS") => Color::White,
            _ => return None,
        };
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }

    fn try_cast_imp(&mut self, view: &GameView, wanted: Option<ObjectId>) -> Option<PolicyAction> {
        if view.player != self.player
            || view.decision_player != self.player
            || view.priority != self.player
            || view.active_player != self.player
            || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
            || view.stack_depth != 0
        {
            return None;
        }
        let card = match wanted {
            Some(id) => view
                .hand
                .iter()
                .find(|card| card.id == id && card.definition == Some(CARD_ID)),
            None => view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID)),
        }?;
        if let Some(selection) = payment_selection(view, &self.imp_cost) {
            if wanted.is_some() {
                self.recast = true;
            } else {
                self.initial_cast = true;
                self.initial_imp = Some(card.id);
            }
            Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection: selection,
            })
        } else {
            None
        }
    }
}

impl CodePolicy for ImpPolicy {
    fn id(&self) -> &'static str {
        if self.pilot {
            "rav.stinkweed-imp-dredge-pilot.v1"
        } else {
            "rav.stinkweed-imp-watchwolf-opponent.v1"
        }
    }

    fn propose_draw_replacement(&mut self, view: &GameView) -> PolicyAction {
        if self.pilot && self.dredged_imp.is_none() {
            if let Some(card) = self
                .blocked_imp
                .and_then(|id| view.dredge_candidates.iter().find(|card| card.id == id))
            {
                self.dredged_imp = Some(card.id);
                return PolicyAction::Draw {
                    decision: view
                        .draw_replacement_decision
                        .expect("pending draw has decision identity"),
                    dredge: Some(card.id),
                };
            }
        }
        PolicyAction::Draw {
            decision: view
                .draw_replacement_decision
                .expect("pending draw has decision identity"),
            dredge: None,
        }
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = if self.pilot && !self.recast {
                Vec::new()
            } else {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.card_types.contains(&CardType::Creature) && card.can_attack)
                    .map(|card| card.id)
                    .collect()
            };
            return PolicyAction::DeclareAttackers { attackers };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            if self.pilot {
                let attacker = view
                    .combat_attackers
                    .iter()
                    .find(|card| card.definition == Some(TARGET_ID))
                    .map(|card| card.id);
                let imp = self.initial_imp.or(self.dredged_imp).and_then(|source| {
                    view.own_battlefield
                        .iter()
                        .find(|card| card.id == source && card.can_block)
                        .map(|card| card.id)
                });
                if let (Some(attacker), Some(blocker)) = (attacker, imp) {
                    self.blocked_imp.get_or_insert(blocker);
                    return PolicyAction::DeclareBlockers {
                        assignments: vec![CombatBlock { attacker, blocker }],
                    };
                }
            }
            return self.inner.propose_move(view);
        }

        if self.pilot {
            let wanted = self.candidate_id_to_cast();
            if let Some(action) = self.try_cast_imp(view, wanted) {
                return action;
            }
            if !self.initial_cast
                && view.player == self.player
                && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
                && view.active_player == self.player
                && view.stack_depth == 0
            {
                if let Some(action) = self.take_land_or_mana_action(view) {
                    return action;
                }
                return PolicyAction::PassPriority;
            }
            if wanted.is_some()
                && view.player == self.player
                && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
                && view.active_player == self.player
                && view.stack_depth == 0
            {
                if let Some(action) = self.take_land_or_mana_action(view) {
                    return action;
                }
            }
        }
        self.inner.propose_move(view)
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

fn mutated_definition(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_stinkweed_imp::definition();
    match mutation {
        Mutation::WrongCost => {
            definition.mana_cost = ManaCost::with_colors(2, [Color::Black, Color::Black])
        }
        Mutation::WrongBody => {
            definition.power = Some(2);
            definition.toughness = Some(1);
        }
        Mutation::NoFlying => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Flying),
        Mutation::NoDredge => definition
            .keywords
            .retain(|keyword| !matches!(keyword, Keyword::Dredge(_))),
        Mutation::DredgeFour => definition.keywords = vec![Keyword::Flying, Keyword::Dredge(4)],
        _ => {}
    }
    definition
}

fn mutated_binding(mutation: Mutation) -> cardbench_magic_engine::TriggeredAbilityBinding {
    let mut binding = candidate_stinkweed_imp::triggered_ability();
    match mutation {
        Mutation::WrongTriggerCondition => {
            binding.ability.condition = TriggerCondition::DealsCombatDamageToPlayer
        }
        Mutation::WrongTriggerEffect => {
            binding.ability.effects = vec![Effect::ReturnSourceFromOwnersGraveyardToHand]
        }
        _ => {}
    }
    binding
}

fn mutation_detected(mutation: Mutation) -> bool {
    run_match(mutation)
        .and_then(|capture| validate_trace(&capture))
        .is_err()
}

fn validate_trace(c: &CapturedMatch) -> Result<Value, String> {
    let imp = c.imp.ok_or("pilot did not cast Stinkweed Imp")?;
    let target = c
        .target
        .ok_or("opponent did not cast the Watchwolf target")?;
    if c.winner != Some(0)
        || c.rejected != 0
        || c.turns == 0
        || c.turns > MAX_TURNS
        || c.accepted > MAX_MOVES
    {
        return Err(format!(
            "duel did not reach a bounded terminal pilot win: winner={:?}, turns={}, rejected={}",
            c.winner, c.turns, c.rejected
        ));
    }
    if c.life_totals[1] > 0 {
        return Err(format!(
            "opponent life remained positive at terminal game: {:?}",
            c.life_totals
        ));
    }
    if c.imp_power != Some(1)
        || c.imp_toughness != Some(2)
        || !c.imp_flying
        || c.imp_dredge != Some(5)
    {
        return Err(format!(
            "Stinkweed Imp characteristics differ: {}/{}, flying={}, dredge={:?}",
            c.imp_power.unwrap_or_default(),
            c.imp_toughness.unwrap_or_default(),
            c.imp_flying,
            c.imp_dredge
        ));
    }
    let events = &c.events;
    let imp_casts: Vec<usize> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == imp)
                .then_some(index)
        })
        .collect();
    if imp_casts.len() < 2 {
        return Err("same Stinkweed Imp was not recast after Dredge".into());
    }
    let first_resolved = event_after(
        events,
        imp_casts[0],
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == imp),
    )
    .ok_or("initial Stinkweed Imp resolution missing")?;
    let (target_attack, blocks) = events.iter().enumerate().find_map(|(attack_index, event)| {
        let is_target_attack = matches!(event, GameEvent::AttackersDeclared { player: PlayerId(1), attackers } if attackers.contains(&target));
        if !is_target_attack { return None; }
        let block_index = event_after(events, attack_index, |later| matches!(later,
            GameEvent::BlockersDeclared { player: PlayerId(0), assignments }
                if assignments.iter().any(|(attacker, blocker)| *attacker == target && *blocker == imp)));
        block_index.map(|block_index| (attack_index, block_index))
    }).ok_or("Stinkweed Imp did not block an attacking target Watchwolf")?;
    let damage = event_after(events, blocks, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 1 } if *source == imp && *permanent == target)).ok_or("Stinkweed Imp did not deal exactly 1 combat damage to the target Watchwolf")?;
    let imp_damage = event_after(events, blocks, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 3 } if *source == target && *permanent == imp)).ok_or("Watchwolf did not deal its combat damage to Stinkweed Imp")?;
    let imp_died = event_after(
        events,
        imp_damage,
        |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == imp),
    )
    .ok_or("combat-dead Stinkweed Imp did not move to its graveyard")?;
    let stacked = event_after(events, damage, |event| matches!(event, GameEvent::TriggeredAbilityStacked { source, ability: TRIGGER_ID, .. } if *source == imp)).ok_or("Stinkweed Imp combat-damage trigger did not stack")?;
    let resolved = event_after(events, stacked, |event| matches!(event, GameEvent::AbilityResolved { source, ability: TRIGGER_ID, .. } if *source == imp)).ok_or("Stinkweed Imp combat-damage trigger did not resolve")?;
    let stacked_incarnation = match &events[stacked] {
        GameEvent::TriggeredAbilityStacked {
            source_incarnation, ..
        } => *source_incarnation,
        _ => return Err("Stinkweed Imp trigger stack event changed after validation".into()),
    };
    let resolved_incarnation = match &events[resolved] {
        GameEvent::AbilityResolved {
            source_incarnation, ..
        } => *source_incarnation,
        _ => return Err("Stinkweed Imp trigger resolution event changed after validation".into()),
    };
    if stacked_incarnation != resolved_incarnation {
        return Err(
            "Stinkweed Imp damage-trigger source incarnation changed before resolution".into(),
        );
    }
    let destroyed = event_after(events, resolved, |event| matches!(event, GameEvent::CardDestroyed { source, card } if *source == imp && *card == target)).ok_or("trigger did not destroy the exact Watchwolf damaged by Stinkweed Imp")?;
    let target_died = event_after(events, destroyed, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == target)).ok_or("destroyed Watchwolf did not reach the graveyard")?;
    let dredge = event_after(events, target_died, |event| matches!(event, GameEvent::Dredged { player: PlayerId(0), card, count: 5 } if *card == imp)).ok_or("Stinkweed Imp did not replace a draw with Dredge 5")?;
    let returned = events
        .iter()
        .enumerate()
        .take(dredge)
        .rev()
        .find_map(|(index, event)| {
            matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == imp)
                .then_some(index)
        })
        .ok_or("Dredge did not return the same Stinkweed Imp object to hand")?;
    let second_cast = imp_casts[1];
    let second_resolved = event_after(
        events,
        second_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == imp),
    )
    .ok_or("Dredged Stinkweed Imp was not resolved after its recast")?;
    let terminal = events
        .iter()
        .rposition(|event| {
            matches!(
                event,
                GameEvent::GameEnded {
                    winner: Some(PlayerId(0))
                }
            )
        })
        .ok_or("terminal P0 win event missing")?;
    if !(imp_casts[0] < first_resolved
        && first_resolved < blocks
        && target_attack < blocks
        && blocks < damage
        && blocks < imp_damage
        && damage < stacked
        && imp_damage < imp_died
        && imp_died < stacked
        && stacked < resolved
        && resolved < destroyed
        && destroyed < target_died
        && target_died < returned
        && returned < dredge
        && dredge < second_cast
        && second_cast < second_resolved
        && second_resolved < terminal)
    {
        return Err(
            "Stinkweed Imp cast/combat/destroy/Dredge/recast/terminal event order differs".into(),
        );
    }
    let payment = events
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if *card == imp => Some(colors.clone()),
            _ => None,
        })
        .ok_or("Stinkweed Imp mana-payment receipt missing")?;
    if payment.len() != 3 || !payment.contains(&Color::Black) {
        return Err(format!(
            "Stinkweed Imp payment did not meet {{2}}{{B}}: {payment:?}"
        ));
    }
    let mill_ids = validate_dredge_sequence(events, dredge, imp, c)?;
    let destroyed_count = events.iter().filter(|event| matches!(event, GameEvent::CardDestroyed { source, card } if *source == imp && *card == target)).count();
    if destroyed_count != 1 {
        return Err(format!(
            "expected exactly one destruction of target object, got {destroyed_count}"
        ));
    }
    if c.imp_zone != Some(Zone::Battlefield) {
        return Err(format!(
            "recast Imp did not remain on battlefield: {:?}",
            c.imp_zone
        ));
    }
    let target_damage_count = events.iter().filter(|event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, .. } if *source == imp && *permanent == target)).count();
    if target_damage_count != 1 {
        return Err(format!(
            "expected one exact Stinkweed Imp damage event to target, got {target_damage_count}"
        ));
    }
    Ok(trace_json(
        c,
        imp,
        target,
        stacked_incarnation,
        &payment,
        &mill_ids,
        [
            imp_casts[0],
            first_resolved,
            target_attack,
            blocks,
            damage,
            imp_damage,
            imp_died,
            stacked,
            resolved,
            destroyed,
            target_died,
            returned,
            dredge,
            second_cast,
            second_resolved,
            terminal,
        ],
    ))
}

fn validate_dredge_sequence(
    events: &[GameEvent],
    dredge_index: usize,
    imp: ObjectId,
    captured: &CapturedMatch,
) -> Result<Vec<ObjectId>, String> {
    let before = captured
        .dredge_library_before
        .as_ref()
        .ok_or("missing private authority library snapshot before Dredge")?;
    let after = captured
        .dredge_library_after
        .as_ref()
        .ok_or("missing private authority library snapshot after Dredge")?;
    if before.len() < 5 || after.len() + 5 != before.len() || after != &before[..before.len() - 5] {
        return Err("Dredge 5 did not remove exactly the five top library cards".into());
    }
    if dredge_index < 12 {
        return Err("Dredge event lacks exact move/incarnation provenance".into());
    }
    let suffix = &events[dredge_index - 12..=dredge_index];
    let expected: Vec<ObjectId> = before.iter().rev().take(5).copied().collect();
    let mut milled = Vec::new();
    for (pair, wanted) in suffix[..10].chunks_exact(2).zip(expected.iter()) {
        match pair {
            [
                GameEvent::CardMoved {
                    card,
                    to: Zone::Graveyard,
                },
                GameEvent::ObjectIncarnationAdvanced { object, .. },
            ] if card == wanted && object == wanted => milled.push(*card),
            other => {
                return Err(format!(
                    "Dredge did not mill exact library-top card {wanted:?}: {other:?}"
                ));
            }
        }
    }
    match &suffix[10..] {
        [
            GameEvent::CardMoved {
                card,
                to: Zone::Hand,
            },
            GameEvent::ObjectIncarnationAdvanced { object, .. },
            GameEvent::Dredged {
                player: PlayerId(0),
                card: dredged,
                count: 5,
            },
        ] if *card == imp && *object == imp && *dredged == imp => {}
        other => {
            return Err(format!(
                "Dredge did not return the same Stinkweed Imp object: {other:?}"
            ));
        }
    }
    if milled.len() != 5
        || milled.contains(&imp)
        || milled.iter().copied().collect::<BTreeSet<_>>().len() != 5
    {
        return Err(format!(
            "Dredge mill identities are not five distinct library objects: {milled:?}"
        ));
    }
    Ok(milled)
}

fn trace_json(
    c: &CapturedMatch,
    imp: ObjectId,
    target: ObjectId,
    source_incarnation: u64,
    payment: &[Color],
    mills: &[ObjectId],
    positions: [usize; 16],
) -> Value {
    let projected = project_events(&c.events);
    let required_ids: BTreeSet<ObjectId> = std::iter::once(imp)
        .chain(std::iter::once(target))
        .chain(mills.iter().copied())
        .collect();
    let events: Vec<Value> = c.events.iter().enumerate().filter_map(|(index, event)| {
        let include = match event {
            GameEvent::SpellManaPaid { card, .. } | GameEvent::SpellCast { card, .. } | GameEvent::SpellResolved { card } => required_ids.contains(card),
            GameEvent::CardMoved { card, .. } | GameEvent::ObjectIncarnationAdvanced { object: card, .. } => required_ids.contains(card),
            GameEvent::Dredged { card, .. } => *card == imp,
            GameEvent::AttackersDeclared { attackers, .. } => attackers.contains(&target),
            GameEvent::BlockersDeclared { assignments, .. } => assignments.iter().any(|(attacker, blocker)| *attacker == target && *blocker == imp),
            GameEvent::DamageDealtToPermanent { source, permanent, .. } => *source == imp && *permanent == target || *source == target && *permanent == imp,
            GameEvent::TriggeredAbilityStacked { source, ability, .. } | GameEvent::AbilityResolved { source, ability, .. } => *source == imp && *ability == TRIGGER_ID,
            GameEvent::CardDestroyed { source, card } => *source == imp && *card == target,
            GameEvent::GameEnded { winner: Some(PlayerId(0)) } => true,
            _ => false,
        };
        if !include { return None; }
        let item = projected.get(index)?;
        Some(json!({"sequence":item.sequence,"turn":item.turn,"kind":item.kind,"facts":item.facts,"canonical":item.canonical}))
    }).collect();
    let sequence_positions: Vec<Value> = positions
        .iter()
        .map(|index| {
            projected
                .get(*index)
                .map(|event| json!(event.sequence))
                .unwrap_or(Value::Null)
        })
        .collect();
    let digest = event_digest(&c.events);
    json!({
        "id":"rav-stinkweed-imp-dredge-five-destroys-damaged-creature",
        "seed":SEED,
        "digest":format!("fnv1a64:{digest:016x}"),
        "ordered_event_sequence":events.iter().filter_map(|event|event.get("sequence").cloned()).collect::<Vec<_>>(),
        "event_positions":{
            "imp_cast":sequence_positions[0],"imp_first_resolved":sequence_positions[1],"watchwolf_attack":sequence_positions[2],"imp_block":sequence_positions[3],
            "imp_damage_to_watchwolf":sequence_positions[4],"watchwolf_damage_to_imp":sequence_positions[5],"imp_death":sequence_positions[6],"trigger_stacked":sequence_positions[7],
            "trigger_resolved":sequence_positions[8],"target_destroyed":sequence_positions[9],"target_graveyard":sequence_positions[10],
            "imp_returned_to_hand":sequence_positions[11],"dredged":sequence_positions[12],"imp_recast":sequence_positions[13],"imp_recast_resolved":sequence_positions[14],"terminal":sequence_positions[15]
        },
        "passed":true,"terminal":"Winner(PlayerId(0))","winner_seat":c.winner,"turns":c.turns,"accepted_moves":c.accepted,"rejected_moves":c.rejected,
        "imp_object":imp.0,"target_object":target.0,
        "imp":{"mana_cost":"{2}{B}","paid_colors":payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>(),"power":c.imp_power,"toughness":c.imp_toughness,"flying":c.imp_flying,"dredge":c.imp_dredge,"printed_subtype":"Imp","subtype_asserted":false,"terminal_zone":format!("{:?}",c.imp_zone)},
        "combat":{"source_object":imp.0,"source_incarnation":source_incarnation,"damaged_target_object":target.0,"damage_amount":1,"destroyed_target_object":target.0,"target_graveyard":true},
        "dredge":{"card_object":imp.0,"count":5,"mill_objects":mills.iter().map(|id|id.0).collect::<Vec<_>>(),"library_before":c.dredge_library_before.as_ref().map(|items|items.len()),"library_after":c.dredge_library_after.as_ref().map(|items|items.len()),"same_object_returned_to_hand":true,"same_object_recast":true},
        "deterministic_replay":true,"life_totals":c.life_totals,"events":events
    })
}

fn event_after(
    events: &[GameEvent],
    start: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, event)| predicate(event).then_some(index))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Stinkweed Imp source: {error}"))?;
    validate_candidate_source_text(&source)
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let file = syn::parse_file(source)
        .map_err(|_| "Stinkweed Imp candidate source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
        || file.items.len() != 4
    {
        return Err("Stinkweed Imp candidate source AST differs from RAV literal contract".into());
    }
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) => {}
            Item::Fn(function)
                if function.attrs.iter().all(|attribute| {
                    attribute.path().is_ident("must_use") || attribute.path().is_ident("doc")
                }) && matches!(&function.vis, Visibility::Restricted(_))
                    && function.sig.inputs.is_empty()
                    && function.sig.generics.params.is_empty()
                    && function.sig.generics.where_clause.is_none() =>
            {
                let (name, result) =
                    match function.sig.ident.to_string().as_str() {
                        "definition" => ("definition", "CardDefinition"),
                        "triggered_ability" => ("triggered_ability", "TriggeredAbilityBinding"),
                        _ => return Err(
                            "Stinkweed Imp candidate source AST differs from RAV literal contract"
                                .into(),
                        ),
                    };
                let [Stmt::Expr(Expr::Struct(literal), None)] = function.block.stmts.as_slice()
                else {
                    return Err(
                        "Stinkweed Imp candidate source AST differs from RAV literal contract"
                            .into(),
                    );
                };
                if !return_type_is(&function.sig.output, result)
                    || !path_is_type(&literal.path, result)
                    || literal.rest.is_some()
                    || !functions.insert(name.to_owned())
                {
                    return Err(
                        "Stinkweed Imp candidate source AST differs from RAV literal contract"
                            .into(),
                    );
                }
            }
            _ => {
                return Err(
                    "Stinkweed Imp candidate source AST differs from RAV literal contract".into(),
                );
            }
        }
    }
    let expected =
        syn::parse_file(EXPECTED_CANDIDATE_SOURCE).expect("trusted source contract parses");
    if functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()])
        || format!("{file:?}") != format!("{expected:?}")
    {
        return Err("Stinkweed Imp candidate source AST differs from RAV literal contract".into());
    }
    Ok(())
}

fn return_type_is(output: &ReturnType, expected: &str) -> bool {
    matches!(output, ReturnType::Type(_, ty) if matches!(&**ty, syn::Type::Path(path) if path.qself.is_none() && path_is_type(&path.path, expected)))
}

fn path_is_type(path: &syn::Path, expected: &str) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == 1
        && path.segments[0].ident == expected
        && matches!(path.segments[0].arguments, syn::PathArguments::None)
}

fn malicious_candidate_source_rejected() -> Result<bool, String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Stinkweed Imp source: {error}"))?;
    let signature = "pub(crate) fn definition() -> CardDefinition {";
    let injected = "pub(crate) fn definition() -> CardDefinition {\n    std::process::exit(0);";
    let malicious = source.replacen(signature, injected, 1);
    if malicious == source || syn::parse_file(&malicious).is_err() {
        return Err(
            "compile-valid Stinkweed Imp side-effect candidate could not be constructed".into(),
        );
    }
    Ok(validate_candidate_source_text(&malicious).is_err())
}

fn validate_candidate_definition() -> Result<(), String> {
    let definition = candidate_stinkweed_imp::definition();
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(2, [Color::Black])
        || definition.colors != BTreeSet::from([Color::Black])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.power != Some(1)
        || definition.toughness != Some(2)
        || definition.keywords != [Keyword::Flying, Keyword::Dredge(5)]
        || !definition.effects.is_empty()
        || definition.supported_rules
            != [
                "full-rules-fidelity",
                "dredge",
                "base-characteristics",
                "flying",
                "combat-damage-destroy-recipient",
            ]
    {
        return Err("Stinkweed Imp definition differs from the RAV literal contract".into());
    }
    Ok(())
}

fn validate_candidate_binding() -> Result<(), String> {
    let candidate = candidate_stinkweed_imp::triggered_ability();
    let registered = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == TRIGGER_ID)
        .ok_or("Stinkweed Imp combat-damage binding is absent from the RAV registry")?;
    if candidate != registered
        || candidate.ability.condition != TriggerCondition::DealsCombatDamageToCreature
        || candidate.ability.mana_cost != ManaCost::new(0)
        || candidate.ability.optional
        || !candidate.ability.targets.is_empty()
        || candidate.ability.effects != [Effect::DestroyCombatDamagedCreature]
        || rav_triggered_ability_bindings()
            .iter()
            .filter(|binding| binding.card_definition == CARD_ID)
            .count()
            != 1
    {
        return Err(
            "Stinkweed Imp binding differs from the RAV printed combat-damage contract".into(),
        );
    }
    Ok(())
}

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let unique: Vec<_> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if unique.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || unique
            .iter()
            .filter(|card| card.collector_number == 107 && card.name == CARD_NAME)
            .count()
            != 1
    {
        return Err(
            "RAV Stinkweed Imp catalog denominator/identity differs from the source contract"
                .into(),
        );
    }
    Ok(unique)
}

fn card_status(card: &CatalogCard) -> Value {
    let id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 107 && card.name == CARD_NAME && id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,
        "engine_status":if id.is_some(){"executable_compatibility_slice"}else{"catalog_only"},
        "trace_status":if passed{"passed"}else{"not_qualified"},
        "qualification":if passed{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}

fn event_digest(events: &[GameEvent]) -> u64 {
    let mut text = String::new();
    for event in events {
        let _ = writeln!(text, "{event:?}");
    }
    text.as_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        })
}
