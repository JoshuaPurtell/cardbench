//! Full-game RAV Golgari Brownscale authority for Dredge 2 and its exact return/cast.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CombatBlock, DeckEntry, DeckList, Effect, Game, GameEvent,
    GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step,
    TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
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
use syn::{Expr, Item, ReturnType, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-GOLGARI-BROWNSCALE";
const CARD_NAME: &str = "Golgari Brownscale";
const OPPONENT_ID: &str = "RAV-WATCHWOLF";
const TRIGGER_ID: &str = "graveyard-to-hand-gain-life";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/golgari_brownscale.rs";
const DEFAULT_TRACE_SEED: u64 = 12;
const EXPECTED_CANDIDATE_SOURCE: &str = r#"//! Candidate-owned Golgari Brownscale definition and graveyard-to-hand trigger.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

/// RAV #166's green 2/3 with Dredge 2 and a two-life graveyard return trigger.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GOLGARI-BROWNSCALE",
        name: "Golgari Brownscale",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Green, Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "dredge",
            "base-characteristics",
            "graveyard-to-hand-gain-life",
        ],
        power: Some(2),
        toughness: Some(3),
        keywords: vec![Keyword::Dredge(2)],
        effects: vec![],
    }
}

/// Gains two life whenever this object returns from its owner's graveyard to hand.
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-GOLGARI-BROWNSCALE",
        ability: TriggeredAbility {
            id: "graveyard-to-hand-gain-life",
            condition: TriggerCondition::GraveyardToHand,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::GainLifeController { amount: 2 }],
        },
    }
}
"#;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/golgari_brownscale.rs"]
mod candidate_golgari_brownscale;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Golgari Brownscale PvP result: {error}");
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
            eprintln!("Golgari Brownscale PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    let malicious_side_effect_rejected = malicious_candidate_source_rejected()?;
    if !malicious_side_effect_rejected {
        return Err("compile-valid side-effect candidate escaped the pre-call source gate".into());
    }
    validate_candidate_definition()?;
    validate_candidate_binding()?;
    validate_registry()?;
    let catalog = unique_catalog()?;
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured != replay || project_events(&captured.events) != project_events(&replay.events) {
        return Err("Golgari Brownscale shuffled-game replay was not deterministic".to_owned());
    }

    let mutations = json!({
        "missing_dredge_mutation_detected": mutation_detected(seed, Mutation::MissingDredge),
        "wrong_dredge_count_mutation_detected": mutation_detected(seed, Mutation::DredgeThree),
        "wrong_identity_mutation_detected": mutation_detected(seed, Mutation::WrongIdentity),
        "wrong_mana_cost_mutation_detected": mutation_detected(seed, Mutation::WrongCost),
        "wrong_creature_body_mutation_detected": mutation_detected(seed, Mutation::WrongBody),
        "graveyard_trigger_removed_mutation_detected": mutation_detected(seed, Mutation::NoTrigger),
        "graveyard_trigger_condition_changed_mutation_detected": mutation_detected(seed, Mutation::WrongTriggerCondition),
        "graveyard_trigger_effect_changed_mutation_detected": mutation_detected(seed, Mutation::WrongTriggerEffect),
        "graveyard_trigger_amount_changed_mutation_detected": mutation_detected(seed, Mutation::GainOneLife),
        "compile_valid_candidate_side_effect_rejected_before_execution": malicious_side_effect_rejected,
        "sealed_mutation_authority": false
    });
    for key in [
        "missing_dredge_mutation_detected",
        "wrong_dredge_count_mutation_detected",
        "wrong_identity_mutation_detected",
        "wrong_mana_cost_mutation_detected",
        "wrong_creature_body_mutation_detected",
        "graveyard_trigger_removed_mutation_detected",
        "graveyard_trigger_condition_changed_mutation_detected",
        "graveyard_trigger_effect_changed_mutation_detected",
        "graveyard_trigger_amount_changed_mutation_detected",
        "compile_valid_candidate_side_effect_rejected_before_execution",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Golgari Brownscale semantic mutation escaped: {key}"
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
        "card": {"collector_number": 166, "definition_id": CARD_ID, "name": CARD_NAME},
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
                "603: graveyard-to-hand triggered ability",
                "608: trigger resolution and life gain",
                "510: combat damage and creature deaths",
                "704: state-based actions after combat damage"
            ],
            "source_contract": "RAV Golgari Brownscale #166 is a green {1}{G}{G} 2/3 Creature — Lizard with Dredge 2; whenever it is put into its owner's hand from their graveyard, its controller gains 2 life. Lizard is retained identity but not represented by CardDefinition.",
            "oracle_reconciliation": "Current Oracle preserves the printed two-life graveyard-to-hand ability and Dredge 2; the RAV source contract controls identity and mechanics.",
            "authority_cutoff": "2026-09-24",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
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
        "candidate_source_validation": {
            "candidate_ast_contract_matched": true,
            "compile_valid_injected_side_effect_candidate_rejected": malicious_side_effect_rejected,
            "guard_ran_before_candidate_function_calls": true
        },
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    MissingDredge,
    DredgeThree,
    WrongIdentity,
    WrongCost,
    WrongBody,
    NoTrigger,
    WrongTriggerCondition,
    WrongTriggerEffect,
    GainOneLife,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    life_totals: [i64; 2],
    brownscale: Option<ObjectId>,
    brownscale_pt: Option<(i32, i32)>,
    brownscale_zone: Option<Zone>,
    brownscale_life_gained: i32,
    brownscale_graveyard_incarnation: Option<u64>,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    let seed = std::env::var("CARDBENCH_GOLGARI_BROWNSCALE_SEED")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_TRACE_SEED);
    match run_match(seed, Mutation::None) {
        Ok(captured) => {
            validate_trace(&captured, seed)?;
            Ok((seed, captured))
        }
        Err(error) => Err(error),
    }
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation) {
        Ok(captured) => validate_trace(&captured, seed).is_err(),
        Err(_) => true,
    }
}

fn definition_for(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_golgari_brownscale::definition();
    match mutation {
        Mutation::None => {}
        Mutation::MissingDredge => definition.keywords.clear(),
        Mutation::DredgeThree => definition.keywords = vec![Keyword::Dredge(3)],
        Mutation::WrongIdentity => definition.id = "RAV-BROWN-SCALE-WRONG",
        Mutation::WrongCost => {
            definition.mana_cost = ManaCost::with_colors(2, [Color::Green, Color::Green])
        }
        Mutation::WrongBody => {
            definition.power = Some(3);
            definition.toughness = Some(3);
        }
        Mutation::NoTrigger
        | Mutation::WrongTriggerCondition
        | Mutation::WrongTriggerEffect
        | Mutation::GainOneLife => {}
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
                card: OPPONENT_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 52,
            },
        ],
        sideboard: vec![],
    };
    let opponent_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: OPPONENT_ID.to_owned(),
                count: 1,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 29,
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
        .ok_or("Golgari Brownscale missing from the RAV registry")?;
    *registered = definition_for(mutation);
    let brownscale_cost = registered.mana_cost.clone();
    let watchwolf_cost = definitions
        .iter()
        .find(|card| card.id == OPPONENT_ID)
        .map(|card| card.mana_cost.clone())
        .ok_or("Watchwolf missing from the RAV registry")?;
    let triggers: Vec<TriggeredAbilityBinding> = rav_triggered_ability_bindings()
        .into_iter()
        .filter(|binding| binding.card_definition != CARD_ID)
        .chain(match mutation {
            Mutation::NoTrigger => None,
            _ => Some(mutated_binding(mutation)),
        })
        .collect();
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
        Box::new(BrownscalePolicy::new(
            PlayerId(0),
            true,
            brownscale_cost.clone(),
            watchwolf_cost.clone(),
        )),
        Box::new(BrownscalePolicy::new(
            PlayerId(1),
            false,
            brownscale_cost,
            watchwolf_cost,
        )),
    ];
    let mut accepted_moves = 0_u32;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted_moves >= config.max_policy_moves {
            return Err("Golgari Brownscale PvP match exceeded its development bound".to_owned());
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
    let brownscale = game.event_log.iter().find_map(|event| match event {
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
    let brownscale_pt = brownscale
        .and_then(|card| game.characteristics(card).ok())
        .and_then(|characteristics| Some((characteristics.power?, characteristics.toughness?)));
    let brownscale_zone = brownscale.and_then(|card| game.zone_of(card));
    let life_totals = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    let brownscale_life_gained: i32 = game
        .event_log
        .iter()
        .filter_map(|event| match event {
            GameEvent::LifeGained {
                player: PlayerId(0),
                amount,
            } => Some(*amount),
            _ => None,
        })
        .sum();
    let brownscale_graveyard_incarnation = game.event_log.iter().find_map(|event| match event {
        GameEvent::TriggeredAbilityStacked {
            source,
            source_incarnation,
            ability,
        } if Some(*source) == brownscale && *ability == TRIGGER_ID => Some(*source_incarnation),
        _ => None,
    });
    Ok(CapturedMatch {
        events: game.event_log.clone(),
        winner,
        turns: game.turn,
        accepted_moves,
        life_totals,
        brownscale,
        brownscale_pt,
        brownscale_zone,
        brownscale_life_gained,
        brownscale_graveyard_incarnation,
    })
}

struct BrownscalePolicy {
    player: PlayerId,
    pilot: bool,
    inner: RavCatalogPolicy,
    brownscale_cost: ManaCost,
    watchwolf_cost: ManaCost,
    initial_source: Option<ObjectId>,
    blocked_source: Option<ObjectId>,
    recovered_source: Option<ObjectId>,
    has_blocked: bool,
}

impl BrownscalePolicy {
    fn new(
        player: PlayerId,
        pilot: bool,
        brownscale_cost: ManaCost,
        watchwolf_cost: ManaCost,
    ) -> Self {
        Self {
            player,
            pilot,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
            brownscale_cost,
            watchwolf_cost,
            initial_source: None,
            blocked_source: None,
            recovered_source: None,
            has_blocked: false,
        }
    }
}

impl CodePolicy for BrownscalePolicy {
    fn id(&self) -> &'static str {
        if self.pilot {
            "rav.greater-brownscale-dredge-pilot.v1"
        } else {
            "rav.greater-brownscale-watchwolf-opponent.v1"
        }
    }

    fn propose_draw_replacement(&mut self, view: &GameView) -> PolicyAction {
        if self.pilot {
            let candidate = self
                .blocked_source
                .and_then(|source| view.dredge_candidates.iter().find(|card| card.id == source))
                .or_else(|| {
                    view.dredge_candidates
                        .iter()
                        .find(|card| card.definition == Some(CARD_ID))
                });
            if let Some(candidate) = candidate {
                self.recovered_source = Some(candidate.id);
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
                    .filter(|card| {
                        card.can_attack
                            && (card.definition == Some(OPPONENT_ID)
                                || self.recovered_source == Some(card.id))
                    })
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
            let assignments = if self.pilot && !self.has_blocked && self.recovered_source.is_none()
            {
                let attacker = view
                    .combat_attackers
                    .iter()
                    .find(|card| card.definition == Some(OPPONENT_ID))
                    .map(|card| card.id);
                let blocker = self
                    .initial_source
                    .and_then(|source| {
                        view.own_battlefield
                            .iter()
                            .any(|card| card.id == source && card.definition == Some(CARD_ID))
                            .then_some(source)
                    })
                    .or_else(|| {
                        view.own_battlefield
                            .iter()
                            .find(|card| card.definition == Some(CARD_ID))
                            .map(|card| card.id)
                    });
                match (attacker, blocker) {
                    (Some(attacker), Some(blocker)) => {
                        self.blocked_source = Some(blocker);
                        self.has_blocked = true;
                        vec![CombatBlock { attacker, blocker }]
                    }
                    _ => Vec::new(),
                }
            } else {
                Vec::new()
            };
            return PolicyAction::DeclareBlockers { assignments };
        }

        let action = self.inner.propose_move(view);
        let action = match action {
            PolicyAction::Cast(request) => {
                let cost = view
                    .hand
                    .iter()
                    .find(|card| card.id == request.card)
                    .and_then(|card| match card.definition {
                        Some(CARD_ID) => Some(&self.brownscale_cost),
                        Some(OPPONENT_ID) => Some(&self.watchwolf_cost),
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
        };
        if self.pilot {
            let cast_source = match &action {
                PolicyAction::Cast(request)
                | PolicyAction::CastWithPayment { request, .. }
                | PolicyAction::CastWithMode { request, .. }
                | PolicyAction::CastWithModeAndColorChoice { request, .. }
                | PolicyAction::CastWithColorChoice { request, .. }
                | PolicyAction::CastWithCreatureSpellAdditionalMana { request, .. } => {
                    Some(request.card)
                }
                _ => None,
            };
            if let Some(source) = cast_source
                && view
                    .hand
                    .iter()
                    .any(|card| card.id == source && card.definition == Some(CARD_ID))
            {
                self.initial_source.get_or_insert(source);
            }
        }
        action
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
    let brownscale = captured
        .brownscale
        .ok_or("pilot never cast Golgari Brownscale")?;
    let watchwolf = events
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCast {
                player: PlayerId(1),
                card,
            } => Some(*card),
            _ => None,
        })
        .ok_or("opponent never cast the 3/3 Watchwolf combatant")?;
    let casts: Vec<usize> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == brownscale)
                .then_some(index)
        })
        .collect();
    if casts.len() < 2 {
        return Err("the exact Brownscale returned by Dredge was not recast".to_owned());
    }
    let payments: Vec<Vec<Color>> = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if *card == brownscale => Some(colors.clone()),
            _ => None,
        })
        .collect();
    if payments.len() < 2
        || payments
            .iter()
            .take(2)
            .any(|payment| payment.len() != 3 || payment.iter().any(|color| *color != Color::Green))
    {
        return Err(format!(
            "Brownscale spell payments do not show {{1}}{{G}}{{G}}: {payments:?}"
        ));
    }
    let resolved_casts: Vec<usize> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            matches!(event, GameEvent::SpellResolved { card } if *card == brownscale)
                .then_some(index)
        })
        .collect();
    if resolved_casts.len() < 2 {
        return Err("the returned Brownscale was not resolved after recast".to_owned());
    }
    let dredges: Vec<(usize, u8)> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::Dredged {
                player: PlayerId(0),
                card,
                count,
            } if *card == brownscale => Some((index, *count)),
            _ => None,
        })
        .collect();
    if dredges.len() != 1 || dredges[0].1 != 2 {
        return Err(format!(
            "expected exactly one Dredge 2 of Brownscale: {dredges:?}"
        ));
    }
    let dredge_index = dredges[0].0;
    let mills = validate_dredge_suffix(events, dredge_index, brownscale)?;
    let block_index = events
        .iter()
        .position(|event| matches!(event,
            GameEvent::BlockersDeclared { player: PlayerId(0), assignments }
                if assignments.iter().any(|(attacker, blocker)| *attacker == watchwolf && *blocker == brownscale)))
        .ok_or("Golgari Brownscale did not block Watchwolf")?;
    let watchwolf_attack_index = events.iter().position(|event| matches!(event,
        GameEvent::AttackersDeclared { player: PlayerId(1), attackers } if attackers.contains(&watchwolf)))
        .ok_or("opponent Watchwolf never attacked")?;
    let brownscale_damage = events
        .iter()
        .enumerate()
        .find_map(|(index, event)| match event {
            GameEvent::DamageDealtToPermanent {
                source,
                permanent,
                amount,
            } if *source == brownscale && *permanent == watchwolf => Some((index, *amount)),
            _ => None,
        })
        .ok_or("Brownscale combat damage to the exact Watchwolf is missing")?;
    let watchwolf_damage = events
        .iter()
        .enumerate()
        .find_map(|(index, event)| match event {
            GameEvent::DamageDealtToPermanent {
                source,
                permanent,
                amount,
            } if *source == watchwolf && *permanent == brownscale => Some((index, *amount)),
            _ => None,
        })
        .ok_or("Watchwolf combat damage to the exact Brownscale is missing")?;
    if brownscale_damage.1 != 2 || watchwolf_damage.1 != 3 {
        return Err(format!(
            "combat damage amounts differ: Brownscale={:?}, Watchwolf={:?}",
            brownscale_damage, watchwolf_damage
        ));
    }
    let died_index = events.iter().enumerate().skip(block_index + 1).find_map(|(index, event)| {
        matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == brownscale)
            .then_some(index)
    }).ok_or("blocked Brownscale did not move to its owner's graveyard")?;
    let died_incarnation = events
        .get(died_index + 1)
        .and_then(|event| match event {
            GameEvent::ObjectIncarnationAdvanced {
                object,
                incarnation,
            } if *object == brownscale => Some(*incarnation),
            _ => None,
        })
        .ok_or("Brownscale's graveyard incarnation advance is missing")?;
    let returned = events
        .iter()
        .enumerate()
        .skip(died_index + 1)
        .find_map(|(index, event)| match event {
            GameEvent::CardMoved {
                card,
                to: Zone::Hand,
            } if *card == brownscale => Some(index),
            _ => None,
        })
        .ok_or("the exact Brownscale did not return to hand")?;
    let stacked = events
        .iter()
        .enumerate()
        .find_map(|(index, event)| match event {
            GameEvent::TriggeredAbilityStacked {
                source,
                source_incarnation,
                ability,
            } if *source == brownscale && *ability == TRIGGER_ID => {
                Some((index, *source_incarnation))
            }
            _ => None,
        })
        .ok_or("Brownscale graveyard-to-hand ability did not stack")?;
    let resolved = events
        .iter()
        .enumerate()
        .find_map(|(index, event)| match event {
            GameEvent::AbilityResolved {
                source,
                source_incarnation,
                ability,
            } if *source == brownscale && *ability == TRIGGER_ID => {
                Some((index, *source_incarnation))
            }
            _ => None,
        })
        .ok_or("Brownscale graveyard-to-hand ability did not resolve")?;
    let gained = events
        .iter()
        .enumerate()
        .find_map(|(index, event)| match event {
            GameEvent::LifeGained {
                player: PlayerId(0),
                amount,
            } if index > resolved.0 => Some((index, *amount)),
            _ => None,
        })
        .ok_or("Brownscale trigger did not produce a typed life-gain event")?;
    let terminal = events
        .iter()
        .rposition(|event| matches!(event, GameEvent::GameEnded { .. }))
        .ok_or("full-game terminal event is missing")?;
    let combat_damage_window = [brownscale_damage.0, watchwolf_damage.0];
    let first_combat_damage = *combat_damage_window
        .iter()
        .min()
        .expect("two damage events");
    let last_combat_damage = *combat_damage_window
        .iter()
        .max()
        .expect("two damage events");
    if !(casts[0] < resolved_casts[0]
        && resolved_casts[0] < watchwolf_attack_index
        && watchwolf_attack_index < block_index
        && block_index < first_combat_damage
        && last_combat_damage < died_index
        && died_index < returned
        && returned < dredge_index
        && dredge_index < stacked.0
        && stacked.0 < resolved.0
        && resolved.0 < gained.0
        && gained.0 < casts[1]
        && casts[1] < resolved_casts[1]
        && resolved_casts[1] < terminal)
    {
        return Err(
            "combat death, Dredge, trigger, life gain, recast, and terminal order differs"
                .to_owned(),
        );
    }
    if stacked.1 != resolved.1
        || stacked.1 != died_incarnation
        || captured.brownscale_graveyard_incarnation != Some(stacked.1)
    {
        return Err("graveyard-to-hand trigger lost the source graveyard incarnation".to_owned());
    }
    if gained.1 != 2 || captured.brownscale_life_gained != 2 {
        return Err(format!(
            "Brownscale must gain exactly 2 life from the trigger: event={:?}, total={}",
            gained.1, captured.brownscale_life_gained
        ));
    }
    if captured.brownscale_pt != Some((2, 3)) {
        return Err(format!(
            "Brownscale characteristics differ: {:?}",
            captured.brownscale_pt
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
    let projected = project_events(events);
    let projected_events: Vec<Value> = projected
        .iter()
        .map(|item| {
            json!({
                "sequence": item.sequence, "turn": item.turn, "kind": item.kind,
                "facts": item.facts, "canonical": item.canonical
            })
        })
        .collect();
    let sequence = |index: usize| {
        projected
            .get(index)
            .map(|item| json!(item.sequence))
            .unwrap_or(Value::Null)
    };
    let ordered_event_sequence: Vec<Value> =
        projected.iter().map(|item| json!(item.sequence)).collect();
    let winner = captured.winner.ok_or("terminal game has no winner")?;
    Ok(json!({
        "id": "rav-golgari-brownscale-dredge-two-life-trigger-terminal-game",
        "seed": seed,
        "digest": format!("fnv1a64:{:016x}", event_digest(events)),
        "ordered_event_sequence": ordered_event_sequence,
        "event_positions": {
            "brownscale_cast": sequence(casts[0]), "brownscale_first_resolved": sequence(resolved_casts[0]),
            "watchwolf_attack": sequence(watchwolf_attack_index), "brownscale_block": sequence(block_index),
            "brownscale_damage_to_watchwolf": sequence(brownscale_damage.0),
            "watchwolf_damage_to_brownscale": sequence(watchwolf_damage.0),
            "brownscale_death": sequence(died_index), "brownscale_returned_to_hand": sequence(returned),
            "dredged": sequence(dredge_index), "trigger_stacked": sequence(stacked.0),
            "trigger_resolved": sequence(resolved.0), "life_gained": sequence(gained.0),
            "brownscale_recast": sequence(casts[1]), "brownscale_recast_resolved": sequence(resolved_casts[1]),
            "terminal": sequence(terminal)
        },
        "passed": true,
        "terminal": format!("Winner(PlayerId({winner}))"),
        "winner_seat": winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": 0,
        "brownscale_object": brownscale.0,
        "watchwolf_object": watchwolf.0,
        "brownscale": {
            "mana_cost": "{1}{G}{G}", "paid_colors": payments[0].iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "power": captured.brownscale_pt.map(|pt| pt.0), "toughness": captured.brownscale_pt.map(|pt| pt.1),
            "printed_subtype": "Lizard", "subtype_asserted": false, "dredge": 2,
            "terminal_zone": format!("{:?}", captured.brownscale_zone)
        },
        "combat": {
            "damaged_target_object": watchwolf.0, "brownscale_damage": brownscale_damage.1,
            "source_object": brownscale.0, "source_incarnation": died_incarnation,
            "watchwolf_damage": watchwolf_damage.1, "brownscale_died": true
        },
        "dredge": {
            "card_object": brownscale.0, "count": 2,
            "mill_objects": mills.iter().map(|id| id.0).collect::<Vec<_>>(),
            "same_object_returned_to_hand": true, "same_object_recast": true,
            "source_graveyard_incarnation": stacked.1, "life_gained": gained.1
        },
        "deterministic_replay": true,
        "life_totals": captured.life_totals,
        "events": projected_events
    }))
}

fn validate_dredge_suffix(
    events: &[GameEvent],
    dredge_index: usize,
    brownscale: ObjectId,
) -> Result<Vec<ObjectId>, String> {
    if dredge_index < 6 {
        return Err("Dredge event has too little preceding zone provenance".to_owned());
    }
    let suffix = &events[dredge_index - 6..=dredge_index];
    let mut mills = Vec::new();
    for pair in suffix[..4].chunks_exact(2) {
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
                    "Dredge did not mill two exact objects in order: {pair:?}"
                ));
            }
        }
    }
    match &suffix[4..] {
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
                count: 2,
            },
        ] if *card == brownscale && *object == brownscale && *dredged == brownscale => {}
        other => {
            return Err(format!(
                "Dredge did not return the same object to hand: {other:?}"
            ));
        }
    }
    if mills.len() != 2
        || mills.contains(&brownscale)
        || mills.iter().copied().collect::<BTreeSet<_>>().len() != 2
    {
        return Err(format!(
            "Dredge mill objects are not two distinct cards: {mills:?}"
        ));
    }
    Ok(mills)
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Golgari Brownscale source: {error}"))?;
    validate_candidate_source_text(&source)
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let file = syn::parse_file(source)
        .map_err(|_| "Golgari Brownscale candidate source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
        || file.items.len() != 4
    {
        return Err(
            "Golgari Brownscale candidate source AST differs from RAV literal contract".into(),
        );
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
                let (name, result) = match function.sig.ident.to_string().as_str() {
                    "definition" => ("definition", "CardDefinition"),
                    "triggered_ability" => ("triggered_ability", "TriggeredAbilityBinding"),
                    _ => return Err(
                        "Golgari Brownscale candidate source AST differs from RAV literal contract"
                            .into(),
                    ),
                };
                let [Stmt::Expr(Expr::Struct(literal), None)] = function.block.stmts.as_slice()
                else {
                    return Err(
                        "Golgari Brownscale candidate source AST differs from RAV literal contract"
                            .into(),
                    );
                };
                if !return_type_is(&function.sig.output, result)
                    || !path_is_type(&literal.path, result)
                    || literal.rest.is_some()
                    || !functions.insert(name.to_owned())
                {
                    return Err(
                        "Golgari Brownscale candidate source AST differs from RAV literal contract"
                            .into(),
                    );
                }
            }
            _ => {
                return Err(
                    "Golgari Brownscale candidate source AST differs from RAV literal contract"
                        .into(),
                );
            }
        }
    }
    let expected = syn::parse_file(EXPECTED_CANDIDATE_SOURCE)
        .expect("trusted Golgari Brownscale source contract parses");
    if functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()])
        || format!("{file:?}") != format!("{expected:?}")
    {
        return Err(
            "Golgari Brownscale candidate source AST differs from RAV literal contract".into(),
        );
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
        .map_err(|error| format!("could not read Golgari Brownscale source: {error}"))?;
    let signature = "pub(crate) fn definition() -> CardDefinition {";
    let injected = "pub(crate) fn definition() -> CardDefinition {\n    std::process::exit(0);";
    let malicious = source.replacen(signature, injected, 1);
    if malicious == source || syn::parse_file(&malicious).is_err() {
        return Err(
            "compile-valid Golgari Brownscale side-effect candidate could not be constructed"
                .into(),
        );
    }
    Ok(validate_candidate_source_text(&malicious).is_err())
}

fn validate_candidate_definition() -> Result<(), String> {
    let definition = candidate_golgari_brownscale::definition();
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(1, [Color::Green, Color::Green])
        || definition.colors != BTreeSet::from([Color::Green])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || (definition.power, definition.toughness) != (Some(2), Some(3))
        || definition.keywords != [Keyword::Dredge(2)]
        || !definition.effects.is_empty()
        || definition.supported_rules
            != [
                "full-rules-fidelity",
                "dredge",
                "base-characteristics",
                "graveyard-to-hand-gain-life",
            ]
    {
        return Err("Golgari Brownscale definition differs from RAV #166 identity".into());
    }
    Ok(())
}

fn validate_candidate_binding() -> Result<(), String> {
    let candidate = candidate_golgari_brownscale::triggered_ability();
    let expected = TriggeredAbilityBinding {
        card_definition: CARD_ID,
        ability: TriggeredAbility {
            id: TRIGGER_ID,
            condition: TriggerCondition::GraveyardToHand,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::GainLifeController { amount: 2 }],
        },
    };
    if candidate != expected {
        return Err("Golgari Brownscale binding differs from RAV #166's two-life trigger".into());
    }
    Ok(())
}

fn validate_registry() -> Result<(), String> {
    let definitions: Vec<_> = card_definitions()
        .into_iter()
        .filter(|definition| definition.id == CARD_ID)
        .collect();
    let expected_definition = candidate_golgari_brownscale::definition();
    if definitions.len() != 1
        || definitions[0].name != expected_definition.name
        || definitions[0].mana_cost != expected_definition.mana_cost
        || definitions[0].colors != expected_definition.colors
        || definitions[0].card_types != expected_definition.card_types
        || (definitions[0].power, definitions[0].toughness)
            != (expected_definition.power, expected_definition.toughness)
        || definitions[0].keywords != expected_definition.keywords
    {
        return Err(
            "RAV card registry does not match the candidate-owned Brownscale definition".into(),
        );
    }
    let registered: Vec<_> = rav_triggered_ability_bindings()
        .into_iter()
        .filter(|binding| binding.card_definition == CARD_ID)
        .collect();
    if registered.len() != 1 || registered[0] != candidate_golgari_brownscale::triggered_ability() {
        return Err(
            "RAV registry does not delegate Brownscale's trigger to the candidate-owned module"
                .into(),
        );
    }
    Ok(())
}

fn mutated_binding(mutation: Mutation) -> TriggeredAbilityBinding {
    let mut binding = candidate_golgari_brownscale::triggered_ability();
    match mutation {
        Mutation::WrongTriggerCondition => binding.ability.condition = TriggerCondition::Dies,
        Mutation::WrongTriggerEffect => {
            binding.ability.effects = vec![Effect::GainLifeController { amount: 3 }]
        }
        Mutation::GainOneLife => {
            binding.ability.effects = vec![Effect::GainLifeController { amount: 1 }]
        }
        _ => {}
    }
    binding
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
