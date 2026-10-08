//! Full-game RAV Frenzied Goblin optional attack-trigger trace.

use cardbench_magic_engine::{
    BasicLandManaAbilityActivation, CardType, Color, CombatBlock, DeckEntry, DeckList, Effect,
    Game, GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId,
    PolicyAction, ResolutionPaymentManaAbility, Step, Target, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
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
use syn::{Expr, Item, Meta, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-FRENZIED-GOBLIN";
const BLOCKER_ID: &str = "RAV-WATCHWOLF";
const CHAR_ID: &str = "RAV-CHAR";
const ABILITY_ID: &str = "attack-cannot-block";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/frenzied_goblin.rs";
const DECK_P0: &str = "rav_frenzied_goblin_red_pilot";
const DECK_P1: &str = "rav_frenzied_goblin_watchwolf_defense";
const MAX_TRACE_SEED: u64 = 96;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/frenzied_goblin.rs"]
mod candidate_frenzied_goblin;

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
            eprintln!("Frenzied Goblin PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
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
        return Err("RAV catalog denominator differs from the source contract".to_owned());
    }
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.terminal != replay.terminal
        || captured.winner != replay.winner
        || captured.goblin != replay.goblin
        || captured.blocker != replay.blocker
        || captured.goblin_payment != replay.goblin_payment
        || captured.blocker_payment != replay.blocker_payment
        || captured.chosen_targets != replay.chosen_targets
        || captured.source_incarnations != replay.source_incarnations
        || captured.blocker_can_block_at_declaration != replay.blocker_can_block_at_declaration
        || captured.goblin_damage != replay.goblin_damage
    {
        return Err("Frenzied Goblin full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "cannot_block_effect_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongKeyword)?,
        "attack_trigger_removal_detected_in_game": mutation_detected(seed, Mutation::RemoveTrigger)?,
        "sealed_mutation_authority": false
    });
    if mutations["cannot_block_effect_mutation_detected_in_game"] != true
        || mutations["attack_trigger_removal_detected_in_game"] != true
    {
        return Err(format!(
            "a Frenzied Goblin attack-trigger mutation escaped the combat trace: mutations={mutations}"
        ));
    }
    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1", "task_id":TASK_ID, "expansion":"RAV",
        "card":{"collector_number":125,"definition_id":CARD_ID,"name":"Frenzied Goblin"},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID}, "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":["105.2 Color", "106.1-106.4 Mana", "117 Timing and Priority", "302.6 Summoning Sickness", "508.1 Declare Attackers", "509.1 Declare Blockers", "603.2 Trigger Events", "603.3 Trigger Placement", "608.2 Resolve Abilities", "611.2c Until-end-of-turn Effects", "510 Combat Damage"],
            "source_contract":"RAV printed Frenzied Goblin #125 identity, {R} red 1/1 creature, and a candidate-owned optional {R} attack trigger that makes a targeted creature unable to block this turn",
            "authority_cutoff":"2026-09-23", "no_provider_trial":true
        },
        "passed":true,"pilot_card_passed":true,"replay_deterministic":true,
        "cards_passed":cards_passed,"cards_credited":0,"cards_total":catalog.len(),"printings_total":RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete":false,"full_reward":false,"harbor_reward":0.0,"pilot_reward":0.0,
        "qualification_status":"candidate_pilot_pass; independent reviewer approval remains",
        "mutation_sensitivity":mutations,"traces":[trace],"per_card_breakdown":breakdown
    }))
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}

fn card_status(card: &CatalogCard) -> Value {
    let (status, definition_id) = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => {
            ("executable_compatibility_slice", Some(definition_id))
        }
        CardSemanticStatus::CatalogOnly { .. } => ("catalog_only", None),
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"definition_id":definition_id,"name":card.name,"engine_status":status,
        "qualification":if passed{"pilot_trace_passed; independent reviewer approval pending"}else{"not_evaluated"},
        "trace_status":if passed{"passed"}else{"not_qualified"},"credited":false})
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongKeyword,
    RemoveTrigger,
}

#[derive(Clone)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    goblin: Option<ObjectId>,
    blocker: Option<ObjectId>,
    finisher_casts: u32,
    goblin_payment: Vec<Color>,
    blocker_payment: Vec<Color>,
    char_payments: Vec<Vec<Color>>,
    optional_choice_seen: bool,
    optional_can_pay: bool,
    optional_target_was_legal: bool,
    chosen_targets: Vec<ObjectId>,
    source_incarnations: Vec<(u64, u64)>,
    blocker_can_block_at_declaration: Option<bool>,
    blocker_has_cannot_block_at_declaration: Option<bool>,
    blockers_were_declared_empty: Option<bool>,
    goblin_damage: Vec<i32>,
    life_totals: [i64; 2],
}

fn find_trace() -> Result<(u64, Captured), String> {
    for seed in 0..=MAX_TRACE_SEED {
        let captured = run_match(seed, Mutation::None)?;
        match validate_trace(&captured, seed) {
            Ok(_) => return Ok((seed, captured)),
            Err(error) if error.contains("was not cast") || error.contains("was not attacked") => {}
            Err(error) => return Err(error),
        }
    }
    Err(format!(
        "no bounded seed produced a full Frenzied Goblin combat trace through {MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    let capture = run_match(seed, mutation)?;
    let Some(error) = validate_trace(&capture, seed).err() else {
        return Ok(false);
    };
    Ok(match mutation {
        Mutation::WrongKeyword => {
            error.contains("did not prevent its chosen Watchwolf from blocking")
        }
        Mutation::RemoveTrigger => error.contains("did not pay for a legal conditional Watchwolf target"),
        Mutation::None => false,
    })
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let p0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: CHAR_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 52,
            },
        ],
        sideboard: vec![],
    };
    let p1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: BLOCKER_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 28,
            },
        ],
        sideboard: vec![],
    };
    for (name, deck) in [(DECK_P0, &p0), (DECK_P1, &p1)] {
        let size: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if size != 60 {
            return Err(format!("{name} has {size} cards; expected 60"));
        }
    }
    let mut definitions = card_definitions();
    let definition = candidate_frenzied_goblin::definition();
    *definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Frenzied Goblin definition missing from RAV registry")? = definition;
    let mut triggers = rav_triggered_ability_bindings();
    match mutation {
        Mutation::None => {}
        Mutation::RemoveTrigger => triggers.retain(|binding| {
            !(binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        }),
        Mutation::WrongKeyword => {
            let binding = triggers
                .iter_mut()
                .find(|binding| {
                    binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID
                })
                .ok_or("Frenzied Goblin trigger binding missing")?;
            binding.ability.effects = vec![Effect::ModifyTargetKeywordUntilEndOfTurn {
                keyword: Keyword::Flying,
            }];
        }
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
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &p0)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &p1)
        .map_err(|error| error.to_string())?;
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(FrenziedPolicy::new(PlayerId(0))),
        Box::new(FrenziedPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0u32;
    let mut attempted = 0u32;
    let mut goblin = None;
    let mut blocker = None;
    let mut finisher_casts = 0u32;
    let mut goblin_payment = Vec::new();
    let mut blocker_payment = Vec::new();
    let mut char_payments = Vec::new();
    let mut optional_choice_seen = false;
    let mut optional_can_pay = false;
    let mut optional_target_was_legal = false;
    let mut chosen_targets = Vec::new();
    let mut blocker_can_block_at_declaration = None;
    let mut blocker_has_cannot_block_at_declaration = None;
    let mut blockers_were_declared_empty = None;
    while !game.is_game_over() {
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        if game.turn > config.max_turns.min(32) || accepted >= config.max_policy_moves {
            let relevant_events: Vec<String> = game
                .event_log
                .iter()
                .filter(|event| {
                    matches!(
                        event,
                        GameEvent::AttackersDeclared { .. }
                            | GameEvent::TriggeredAbilityStacked { .. }
                            | GameEvent::AbilityManaPaid { .. }
                            | GameEvent::AbilityResolved { .. }
                            | GameEvent::BlockersDeclared { .. }
                            | GameEvent::DamageDealtToPlayer { .. }
                            | GameEvent::CardMoved { .. }
                    )
                })
                .take(80)
                .map(|event| format!("{event:?}"))
                .collect();
            return Err(format!(
                "Frenzied Goblin match exceeded its bounded game or move limit: turn={} accepted={} player={:?} step={:?} priority={:?} stack={} hand={:?} own={:?} opponent={:?} lands_played={} goblin={goblin:?} blocker={blocker:?} life=({}, {}) relevant={relevant_events:?}",
                game.turn,
                accepted,
                player,
                view.step,
                view.priority,
                view.stack_depth,
                view.hand
                    .iter()
                    .map(|card| card.definition)
                    .collect::<Vec<_>>(),
                view.own_battlefield
                    .iter()
                    .map(|card| card.definition)
                    .collect::<Vec<_>>(),
                view.opponent_battlefield
                    .iter()
                    .map(|card| card.definition)
                    .collect::<Vec<_>>(),
                view.lands_played,
                game.player(PlayerId(0))
                    .map_err(|error| error.to_string())?
                    .life,
                game.player(PlayerId(1))
                    .map_err(|error| error.to_string())?
                    .life
            ));
        }
        let policy = policies
            .get_mut(player.0)
            .ok_or("Frenzied Goblin policy seat missing")?;
        if let Some(choice) = &view.optional_triggered_ability_choice
            && choice.ability == ABILITY_ID
            && (Some(choice.source) == goblin
                || game
                    .card_definition(choice.source)
                    .is_ok_and(|card| card.id == CARD_ID))
        {
            optional_choice_seen = true;
            optional_can_pay |= choice.can_pay;
        }
        if let Some(choice) = &view.triggered_ability_target_choice
            && choice.ability == ABILITY_ID
            && (Some(choice.source) == goblin
                || game
                    .card_definition(choice.source)
                    .is_ok_and(|card| card.id == CARD_ID))
        {
            optional_target_was_legal |= blocker.is_some_and(|target| {
                choice
                    .target_options
                    .iter()
                    .any(|options| options.contains(&Target::Permanent(target)))
            });
        }
        if player == PlayerId(1)
            && view.step == Step::DeclareBlockers
            && !view.blockers_declared
            && view
                .combat_attackers
                .iter()
                .any(|card| card.definition == Some(CARD_ID))
        {
            if let Some(target) = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(BLOCKER_ID))
            {
                blocker_can_block_at_declaration.get_or_insert(target.can_block);
                blocker_has_cannot_block_at_declaration
                    .get_or_insert(target.keywords.contains(&Keyword::CannotBlock));
            }
        }
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
        match &action {
            PolicyAction::ResolveOptionalTriggeredAbility {
                source,
                ability,
                pay: true,
                target: Some(Target::Permanent(target)),
                ..
            }
            | PolicyAction::ResolveOptionalTriggeredAbilityWithManaPayment {
                source,
                ability,
                pay: true,
                target: Some(Target::Permanent(target)),
                ..
            } if *ability == ABILITY_ID
                && (Some(*source) == goblin
                    || game
                        .card_definition(*source)
                        .is_ok_and(|card| card.id == CARD_ID)) =>
            {
                optional_can_pay = true;
                chosen_targets.push(*target);
            }
            PolicyAction::ChooseTriggeredAbilityTargets {
                source,
                ability,
                targets,
                ..
            } if *ability == ABILITY_ID
                && (Some(*source) == goblin
                    || game
                        .card_definition(*source)
                        .is_ok_and(|card| card.id == CARD_ID)) =>
            {
                chosen_targets.extend(targets.iter().filter_map(|target| match target {
                    Target::Permanent(permanent) => Some(*permanent),
                    _ => None,
                }));
            }
            _ => {}
        }
        if player == PlayerId(1)
            && view.step == Step::DeclareBlockers
            && !view.blockers_declared
            && view
                .combat_attackers
                .iter()
                .any(|card| card.definition == Some(CARD_ID))
        {
            blockers_were_declared_empty = Some(
                matches!(&action, PolicyAction::DeclareBlockers { assignments } if assignments.is_empty()),
            );
        }
        let start = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Frenzied Goblin policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        for event in &game.event_log[start..] {
            match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|def| def.id == CARD_ID) =>
                {
                    goblin = Some(*card);
                    goblin_payment = colors.clone();
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|def| def.id == CARD_ID) =>
                {
                    goblin = Some(*card)
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(1),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|def| def.id == BLOCKER_ID) =>
                {
                    blocker = Some(*card);
                    blocker_payment = colors.clone();
                }
                GameEvent::SpellCast {
                    player: PlayerId(1),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|def| def.id == BLOCKER_ID) =>
                {
                    blocker = Some(*card)
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|def| def.id == CHAR_ID) =>
                {
                    finisher_casts += 1
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|def| def.id == CHAR_ID) =>
                {
                    char_payments.push(colors.clone())
                }
                _ => {}
            }
        }
        if mutation != Mutation::None
            && goblin.is_some_and(|card| game.zone_of(card) == Some(Zone::Graveyard))
        {
            break;
        }
    }
    let events = game.event_log.clone();
    let source_incarnations = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::TriggeredAbilityStacked {
                source,
                source_incarnation,
                ability: ABILITY_ID,
                ..
            } if Some(*source) == goblin => {
                let resolved = events.iter().find_map(|later| match later {
                    GameEvent::AbilityResolved {
                        source: resolved_source,
                        source_incarnation: resolved_incarnation,
                        ability: ABILITY_ID,
                    } if resolved_source == source
                        && resolved_incarnation == source_incarnation =>
                    {
                        Some(*resolved_incarnation)
                    }
                    _ => None,
                })?;
                Some((*source_incarnation, resolved))
            }
            _ => None,
        })
        .collect();
    let goblin_damage = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if Some(*source) == goblin => Some(*amount),
            _ => None,
        })
        .collect();
    Ok(Captured {
        events,
        terminal: game.winner().map_or_else(
            || {
                if game.is_game_over() {
                    "Draw".to_owned()
                } else {
                    "Running".to_owned()
                }
            },
            |winner| format!("Winner(PlayerId({}))", winner.0),
        ),
        winner: game.winner().map(|player| player.0),
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempted.saturating_sub(accepted),
        goblin,
        blocker,
        finisher_casts,
        goblin_payment,
        blocker_payment,
        char_payments,
        optional_choice_seen,
        optional_can_pay,
        optional_target_was_legal,
        chosen_targets,
        source_incarnations,
        blocker_can_block_at_declaration,
        blocker_has_cannot_block_at_declaration,
        blockers_were_declared_empty,
        goblin_damage,
        life_totals: [
            game.player(PlayerId(0)).map_err(|e| e.to_string())?.life,
            game.player(PlayerId(1)).map_err(|e| e.to_string())?.life,
        ],
    })
}

struct FrenziedPolicy {
    player: PlayerId,
    candidate_cast: bool,
    blocker_cast: bool,
    triggered: bool,
    inner: cardbench_magic_policies::RavCatalogPolicy,
}

impl FrenziedPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            candidate_cast: false,
            blocker_cast: false,
            triggered: false,
            inner: cardbench_magic_policies::RavCatalogPolicy::new(
                player,
                cardbench_magic_policies::CatalogPolicyProfile::Curve,
            ),
        }
    }

}

impl CodePolicy for FrenziedPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.frenzied-goblin-open-combat.v1"
        } else {
            "rav.frenzied-goblin-watchwolf-block.v1"
        }
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if self.player == PlayerId(0)
            && view.priority == self.player
            && view.stack_depth > 0
            && view.mana_pool.amount(Color::Red) == 0
            && view
                .combat_attackers
                .iter()
                .any(|card| card.definition == Some(CARD_ID))
            && view
                .opponent_battlefield
                .iter()
                .any(|card| card.definition == Some(BLOCKER_ID))
            && let Some(land) = view.own_battlefield.iter().find(|card| {
                card.card_types.contains(&CardType::Land)
                    && !card.tapped
                    && card.mana_colors.contains(&Color::Red)
            })
        {
            // The optional trigger choice needs red mana already available.
            // Float it during the attacker's priority window before declaring.
            return PolicyAction::ActivateManaAbility {
                land: land.id,
                color: Color::Red,
            };
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            if self.player == PlayerId(0) {
                let blocker_live = view
                    .opponent_battlefield
                    .iter()
                    .any(|card| card.definition == Some(BLOCKER_ID));
                let attackers: Vec<_> = if blocker_live {
                    view.own_battlefield
                        .iter()
                        .filter(|card| card.definition == Some(CARD_ID) && card.can_attack)
                        .map(|card| card.id)
                        .collect()
                } else {
                    Vec::new()
                };
                return PolicyAction::DeclareAttackers { attackers };
            }
            return PolicyAction::DeclareAttackers {
                attackers: Vec::new(),
            };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            if self.player == PlayerId(1) {
                let attacker = view
                    .combat_attackers
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID));
                let blocker = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.definition == Some(BLOCKER_ID) && card.can_block);
                if let (Some(attacker), Some(blocker)) = (attacker, blocker) {
                    return PolicyAction::DeclareBlockers {
                        assignments: vec![CombatBlock {
                            attacker: attacker.id,
                            blocker: blocker.id,
                        }],
                    };
                }
            }
            return PolicyAction::DeclareBlockers {
                assignments: Vec::new(),
            };
        }
        if view.active_player == self.player
            && view.step == Step::PrecombatMain
            && view.stack_depth == 0
        {
            if view.lands_played == 0 {
                if let Some(land) = view
                    .hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
                {
                    return PolicyAction::PlayLand { card: land.id };
                }
            }
            if self.player == PlayerId(0)
                && !self.candidate_cast
                && let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
            {
                let cost = candidate_frenzied_goblin::definition().mana_cost;
                if let Some(payment) = payment(view, &cost) {
                    self.candidate_cast = true;
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: payment,
                    };
                }
                if let Some(action) = make_mana(view, &cost) {
                    return action;
                }
            }
            if self.player == PlayerId(1)
                && !self.blocker_cast
                && let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(BLOCKER_ID))
            {
                let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
                if let Some(payment) = payment(view, &cost) {
                    self.blocker_cast = true;
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: payment,
                    };
                }
                if let Some(action) = make_mana(view, &cost) {
                    return action;
                }
            }
        }
        if self.player == PlayerId(0)
            && self.triggered
            && view.active_player == self.player
            && view.step == Step::PostcombatMain
            && view.stack_depth == 0
        {
            let cost = ManaCost::with_colors(2, [Color::Red, Color::Red]);
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CHAR_ID))
            {
                if let Some(payment) = payment(view, &cost) {
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![Target::Player(PlayerId(1))],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: payment,
                    };
                }
                if let Some(action) = make_mana(view, &cost) {
                    return action;
                }
            }
        }
        if view.active_player == self.player
            && view.step == Step::PrecombatMain
            && view.lands_played == 0
        {
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        PolicyAction::PassPriority
    }

    fn propose_optional_triggered_ability(&mut self, view: &GameView) -> PolicyAction {
        let choice = view
            .optional_triggered_ability_choice
            .as_ref()
            .expect("optional choice is visible");
        if choice.ability == ABILITY_ID {
            let mana_abilities = if choice.can_pay {
                Vec::new()
            } else if let Some(land) = view.own_battlefield.iter().find(|card| {
                    card.card_types.contains(&CardType::Land)
                        && !card.tapped
                        && card.mana_colors.contains(&Color::Red)
            }) {
                vec![ResolutionPaymentManaAbility::IntrinsicLand(
                    BasicLandManaAbilityActivation {
                        land: land.id,
                        color: Color::Red,
                    },
                )]
            } else {
                Vec::new()
            };
            if choice.can_pay || !mana_abilities.is_empty() {
                self.triggered = true;
                return PolicyAction::ResolveOptionalTriggeredAbilityWithManaPayment {
                    decision: choice.decision,
                    source: choice.source,
                    ability: choice.ability,
                    pay: true,
                    target: None,
                    mana_abilities,
                };
            }
        }
        PolicyAction::ResolveOptionalTriggeredAbility {
            decision: choice.decision,
            source: choice.source,
            ability: choice.ability,
            pay: false,
            target: None,
        }
    }

    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.player == PlayerId(0)
            && let Some(choice) = &view.triggered_ability_target_choice
            && choice.ability == ABILITY_ID
        {
            let blocker = view
                .opponent_battlefield
                .iter()
                .find(|card| card.definition == Some(BLOCKER_ID))?
                .id;
            let targets = choice
                .target_options
                .iter()
                .map(|options| {
                    if options.contains(&Target::Permanent(blocker)) {
                        Target::Permanent(blocker)
                    } else {
                        options.first().copied().unwrap_or(Target::Permanent(blocker))
                    }
                })
                .collect();
            return Some(PolicyAction::ChooseTriggeredAbilityTargets {
                decision: choice.decision,
                source: choice.source,
                ability: choice.ability,
                targets,
            });
        }
        self.inner.propose_pending_decision(view)
    }
    fn propose_draw_replacement(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_draw_replacement(view)
    }
    fn propose_private_library_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_private_library_choice(view)
    }
    fn propose_private_opponent_library_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_private_opponent_library_choice(view)
    }
    fn propose_library_search_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_library_search_choice(view)
    }
}

fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<_> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let amount = &mut available
            .iter_mut()
            .find(|(available_color, _)| available_color == color)?
            .1;
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
    let mut needed = cost.colored.iter().copied().find(|color| {
        view.mana_pool.amount(*color)
            < cost
                .colored
                .iter()
                .filter(|required| *required == color)
                .count() as u8
    });
    if needed.is_none() && view.mana_pool.total() < cost.generic + cost.colored.len() as u8 {
        needed = cost.colored.first().copied().or(Some(Color::Red));
    }
    let preferred = needed?;
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
    let goblin = c.goblin.ok_or("Frenzied Goblin was not cast")?;
    let blocker = c.blocker.ok_or("opposing Watchwolf was not cast")?;
    if c.goblin_payment.len() != 1 || c.goblin_payment != [Color::Red] {
        return Err("Frenzied Goblin did not pay actual {R}".to_owned());
    }
    if c.blocker_payment.len() != 2
        || !c.blocker_payment.contains(&Color::Green)
        || !c.blocker_payment.contains(&Color::White)
    {
        return Err("Watchwolf did not pay actual {G}{W}".to_owned());
    }
    if !c.optional_choice_seen
        || !c.optional_can_pay
        || !c.optional_target_was_legal
        || c.chosen_targets.is_empty()
        || c.chosen_targets.iter().any(|target| *target != blocker)
    {
        return Err(
            "Frenzied Goblin did not pay for a legal conditional Watchwolf target".to_owned(),
        );
    }
    if c.blocker_can_block_at_declaration != Some(false)
        || c.blocker_has_cannot_block_at_declaration != Some(true)
    {
        return Err(
            "Frenzied Goblin did not prevent its chosen Watchwolf from blocking".to_owned(),
        );
    }
    if c.blockers_were_declared_empty != Some(true) {
        return Err("Watchwolf was not kept out of the blocking assignment by the resolved Frenzied Goblin effect".to_owned());
    }
    if !c.goblin_damage.contains(&1) {
        return Err(
            "Frenzied Goblin did not deal unblocked combat damage to the opponent".to_owned(),
        );
    }
    if c.source_incarnations.is_empty()
        || c.source_incarnations
            .iter()
            .any(|(stacked, resolved)| stacked != resolved)
    {
        return Err(
            "Frenzied Goblin trigger source incarnation changed before resolution".to_owned(),
        );
    }
    if c.finisher_casts < 2 || c.char_payments.len() < 2 {
        return Err(format!(
            "the pilot did not cast two paid Char finishers: casts={} payments={} terminal={} life={:?}",
            c.finisher_casts, c.char_payments.len(), c.terminal, c.life_totals
        ));
    }
    if c.terminal != "Winner(PlayerId(0))"
        || c.winner != Some(0)
        || c.rejected_moves != 0
        || c.life_totals[0] <= 0
        || c.life_totals[1] > 0
    {
        return Err(format!(
            "Frenzied Goblin full PvP game did not reach a clean seat-zero terminal win: {:?}",
            c.life_totals
        ));
    }
    let candidate_payment = event_index(&c.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == goblin && colors.contains(&Color::Red))) .ok_or("Frenzied Goblin typed cast payment missing")?;
    let candidate_cast = event_index_after(&c.events, candidate_payment, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == goblin)).ok_or("Frenzied Goblin cast event missing")?;
    let candidate_resolved = event_index_after(
        &c.events,
        candidate_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == goblin),
    )
    .ok_or("Frenzied Goblin resolution missing")?;
    let candidate_entry = event_index_after(&c.events, candidate_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield, .. } if *card == goblin)).ok_or("Frenzied Goblin battlefield entry missing")?;
    let blocker_payment = event_index(&c.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(1), card, colors } if *card == blocker && colors.contains(&Color::Green) && colors.contains(&Color::White))) .ok_or("Watchwolf typed cast payment missing")?;
    let blocker_cast = event_index_after(&c.events, blocker_payment, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == blocker)).ok_or("Watchwolf cast event missing")?;
    let blocker_resolved = event_index_after(
        &c.events,
        blocker_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == blocker),
    )
    .ok_or("Watchwolf resolution missing")?;
    let blocker_entry = event_index_after(&c.events, blocker_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield, .. } if *card == blocker)).ok_or("Watchwolf battlefield entry missing")?;
    let attack = event_index_after(&c.events, candidate_entry, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&goblin))).ok_or("Frenzied Goblin was not attacked")?;
    let trigger = event_index_after(&c.events, attack, |event| matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, ability: ABILITY_ID, .. } if *source == goblin)).ok_or("Frenzied Goblin attack trigger did not reach the stack")?;
    let mana = event_index_after(&c.events, trigger, |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: ABILITY_ID, mana_cost } if *source == goblin && mana_cost == &ManaCost::with_colors(0, [Color::Red]))).ok_or("optional {R} trigger payment receipt missing")?;
    let ability = event_index_after(&c.events, mana, |event| matches!(event, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == goblin)).ok_or("Frenzied Goblin attack ability did not resolve")?;
    let blocks = event_index_after(&c.events, ability, |event| {
        matches!(
            event,
            GameEvent::BlockersDeclared {
                player: PlayerId(1),
                ..
            }
        )
    })
    .ok_or("opponent blocker declaration event missing")?;
    let damage = event_index_after(&c.events, blocks, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 1 } if *source == goblin)).ok_or("Frenzied Goblin did not get through after its effect")?;
    let ended = event_index_after(&c.events, damage, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal victory event missing")?;
    let projected = project_events(&c.events);
    let event_values: Vec<Value> = projected.iter().map(|event| json!({"sequence":event.sequence,"turn":event.turn,"kind":event.kind,"canonical":event.canonical,"facts":event.facts})).collect();
    let mut events = [
        candidate_payment,
        candidate_cast,
        candidate_resolved,
        candidate_entry,
        blocker_payment,
        blocker_cast,
        blocker_resolved,
        blocker_entry,
        attack,
        trigger,
        mana,
        ability,
        blocks,
        damage,
        ended,
    ];
    if !(candidate_payment < candidate_cast
        && candidate_cast < candidate_resolved
        && candidate_resolved < candidate_entry
        && candidate_entry < attack
        && attack < trigger
        && trigger < mana
        && mana < ability
        && ability < blocks
        && blocks < damage
        && damage < ended
        && blocker_payment < blocker_cast
        && blocker_cast < blocker_resolved
        && blocker_resolved < blocker_entry
        && blocker_entry < attack)
    {
        return Err("Frenzied Goblin's cast, target setup, attack, optional payment, combat, or terminal receipts are out of order".to_owned());
    }
    events.sort_unstable();
    if events.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err("Frenzied Goblin selected event receipts are duplicated".to_owned());
    }
    Ok(json!({
        "id":"rav-frenzied-goblin-optional-attack-cannot-block", "passed":true,"seed":seed,
        "decks":[DECK_P0,DECK_P1],"terminal":c.terminal,"winner_seat":c.winner,"turns":c.turns,
        "accepted_moves":c.accepted_moves,"rejected_moves":c.rejected_moves,
        "goblin_object":goblin.0,"watchwolf_object":blocker.0,
        "goblin_cast_payment":{"colors":c.goblin_payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>()},
        "watchwolf_cast_payment":{"colors":c.blocker_payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>()},
        "attack_trigger":{"ability":ABILITY_ID,"optional_choice_seen":c.optional_choice_seen,"can_pay":c.optional_can_pay,"conditional_target_legal":c.optional_target_was_legal,"chosen_target_objects":c.chosen_targets.iter().map(|target|target.0).collect::<Vec<_>>(),"source_incarnations":c.source_incarnations.iter().map(|(stacked,resolved)|json!({"stacked":stacked,"resolved":resolved})).collect::<Vec<_>>()},
        "blocker_state":{"can_block_at_declaration":c.blocker_can_block_at_declaration,"has_cannot_block_keyword":c.blocker_has_cannot_block_at_declaration,"assignments_empty":c.blockers_were_declared_empty},
        "goblin_combat_damage":c.goblin_damage,"char_finisher_count":c.finisher_casts,"char_cast_payments":c.char_payments.iter().map(|colors|colors.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>()).collect::<Vec<_>>(),
        "life_at_match_end":c.life_totals,"ordered_event_sequence":events,"events":event_values
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

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Frenzied Goblin source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Frenzied Goblin source is not valid Rust".to_owned())?;
    if !file.attrs.iter().all(|attribute| {
        attribute.path().is_ident("doc") && matches!(attribute.meta, Meta::NameValue(_))
    }) {
        return Err("Frenzied Goblin has an unsupported module attribute".to_owned());
    }
    let mut imports = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item) if item.attrs.is_empty() && matches!(item.vis, Visibility::Inherited) => imports.extend(flattened_use_paths(item).ok_or("unsupported Frenzied Goblin import")?),
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !matches!(name.as_str(),"definition"|"triggered_ability") || !matches!(&function.vis,Visibility::Restricted(v)if v.path.is_ident("crate"))
                    || function.sig.asyncness.is_some() || function.sig.unsafety.is_some() || !function.sig.inputs.is_empty() || !function.sig.generics.params.is_empty() || function.sig.variadic.is_some() || function.block.stmts.len()!=1
                    || !function.attrs.iter().all(|attr|(attr.path().is_ident("doc")&&matches!(attr.meta,Meta::NameValue(_)))||attr.path().is_ident("must_use"))
                { return Err("Frenzied Goblin functions must be safe, no-argument direct literals".to_owned()); }
                let Stmt::Expr(Expr::Struct(value),None)=&function.block.stmts[0] else {return Err("Frenzied Goblin candidate functions must return direct literals".to_owned());};
                if value.rest.is_some() || !declarative_expr(&Expr::Struct(value.clone())) || !functions.insert(name) { return Err("Frenzied Goblin candidate module has unsupported executable structure".to_owned()); }
            }
            _=>return Err("Frenzied Goblin module permits only private imports and its two candidate functions".to_owned()),
        }
    }
    let expected: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TargetRequirement"],
        vec!["cardbench_magic_engine", "TriggerCondition"],
        vec!["cardbench_magic_engine", "TriggeredAbility"],
        vec!["cardbench_magic_engine", "TriggeredAbilityBinding"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    if functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()])
        || imports != expected
    {
        return Err("Frenzied Goblin source imports or candidate functions differ".to_owned());
    }
    let card = candidate_frenzied_goblin::definition();
    if card.id != CARD_ID
        || card.name != "Frenzied Goblin"
        || card.set_code != "RAV"
        || card.mana_cost != ManaCost::with_colors(0, [Color::Red])
        || card.colors != BTreeSet::from([Color::Red])
        || card.card_types != BTreeSet::from([CardType::Creature])
        || card.power != Some(1)
        || card.toughness != Some(1)
        || card.is_basic_land
        || !card.mana_colors.is_empty()
        || !card.keywords.is_empty()
    {
        return Err(
            "Frenzied Goblin identity, {R} cost, or red 1/1 creature characteristics differ"
                .to_owned(),
        );
    }
    let ability = candidate_frenzied_goblin::triggered_ability();
    if ability.card_definition != CARD_ID
        || ability.ability.id != ABILITY_ID
        || ability.ability.condition != cardbench_magic_engine::TriggerCondition::Attacks
        || ability.ability.mana_cost != ManaCost::with_colors(0, [Color::Red])
        || !ability.ability.optional
        || ability.ability.targets != [cardbench_magic_engine::TargetRequirement::Creature]
    {
        return Err(
            "Frenzied Goblin must retain its optional red attack trigger and creature target"
                .to_owned(),
        );
    }
    if card_definitions()
        .into_iter()
        .find(|entry| entry.id == CARD_ID)
        .as_ref()
        != Some(&card)
    {
        return Err(
            "Frenzied Goblin candidate definition differs from the runtime registry".to_owned(),
        );
    }
    let binding = rav_triggered_ability_bindings()
        .into_iter()
        .find(|entry| entry.card_definition == CARD_ID && entry.ability.id == ABILITY_ID)
        .ok_or("Frenzied Goblin attack trigger is absent from the RAV registry")?;
    if binding != ability {
        return Err(
            "Frenzied Goblin candidate trigger differs from the runtime registry".to_owned(),
        );
    }
    Ok(())
}

fn declarative_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(_) | Expr::Path(_) => true,
        Expr::Unary(unary) => {
            matches!(unary.op, syn::UnOp::Neg(_)) && matches!(&*unary.expr, Expr::Lit(_))
        }
        Expr::Reference(reference) => declarative_expr(&reference.expr),
        Expr::Array(array) => array.elems.iter().all(declarative_expr),
        Expr::Struct(value) => {
            value.attrs.is_empty()
                && value.fields.iter().all(|field| {
                    matches!(field.member, syn::Member::Named(_)) && declarative_expr(&field.expr)
                })
                && value.rest.as_deref().is_none_or(declarative_expr)
        }
        Expr::Call(call) => {
            let allowed = match &*call.func {
                Expr::Path(path) if path.qself.is_none() => {
                    let names: Vec<_> = path
                        .path
                        .segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect();
                    matches!(names.as_slice(),[a,b]if(a=="BTreeSet"&&(b=="from"||b=="new"))||(a=="ManaCost"&&(b=="new"||b=="with_colors")))
                        || matches!(names.as_slice(),[one]if one=="Some")
                }
                _ => false,
            };
            allowed && call.args.iter().all(declarative_expr)
        }
        Expr::Macro(mac) => {
            let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
            mac.mac.path.is_ident("vec")
                && syn::parse::Parser::parse2(&parser, mac.mac.tokens.clone())
                    .is_ok_and(|values| values.iter().all(declarative_expr))
        }
        _ => false,
    }
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
        _ => false,
    }
}
