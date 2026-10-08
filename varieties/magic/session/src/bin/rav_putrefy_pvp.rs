//! Full-game diagnostic authority for RAV Putrefy's no-regeneration removal.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    AbilityActivation, CardType, CastRequest, Color, DeckEntry, DeckList, Effect, Game, GameEvent,
    GameView, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use syn::{Expr, Item, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-PUTREFY";
const TARGET_ID: &str = "RAV-GLASS-GOLEM";
const VOTARY_ID: &str = "RAV-VOTARY-OF-THE-CONCLAVE";
const ABILITY_ID: &str = "regenerate-target-creature";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/putrefy.rs";
const DECK_P0: &str = "rav_putrefy_golgari_midrange";
const DECK_P1: &str = "rav_glass_golem_colorless";

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/putrefy.rs"]
mod candidate_putrefy;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Putrefy result: {error}");
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
            eprintln!("Putrefy PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    Removed,
    Regenerable,
    WrongCost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    putrefy: Option<ObjectId>,
    target: Option<ObjectId>,
    votary: Option<ObjectId>,
    payment: Vec<Color>,
    zone: Option<Zone>,
    spell_zone: Option<Zone>,
    life: [i64; 2],
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_definition()?;
    let printings = rav_main_set_catalog();
    let mut seen = BTreeSet::new();
    let catalog: Vec<CatalogCard> = printings
        .iter()
        .copied()
        .filter(|card| seen.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
    {
        return Err("RAV catalog identity or printing denominator changed".to_owned());
    }
    let (seed, reference) = find_trace()?;
    let trace = validate_trace(&reference, seed)?;
    if reference != run_match(seed, Mutation::None)? {
        return Err("Putrefy full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "destruction_instruction_removed_detected": mutation_detected(seed, Mutation::Removed)?,
        "regeneration_bypass_removed_detected": mutation_detected(seed, Mutation::Regenerable)?,
        "printed_cost_changed_detected": mutation_detected(seed, Mutation::WrongCost)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "destruction_instruction_removed_detected",
        "regeneration_bypass_removed_detected",
        "printed_cost_changed_detected",
    ] {
        if mutations[key] != true {
            return Err(format!("Putrefy gameplay mutation escaped: {key}"));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,"expansion":"RAV",
        "card":{"collector_number":221,"definition_id":CARD_ID,"name":"Putrefy"},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},"candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{"comprehensive_rules":"2026-08-07","checked_sections":["105 and 202: color and characteristics","106 and 601.2: mana and casting","109.5 and 115: target identity and target legality","117: priority and instant-speed actions","608.2: resolution","701.7: destroy","701.15: regeneration replacement","704: state-based actions","510: combat damage"],"source_contract":"CardBench-authored semantic summary: RAV Putrefy is a {1}{B}{G} black-green instant that destroys target artifact or creature, and that destruction cannot be regenerated.","authority_cutoff":"2026-09-24","no_provider_trial":true},
        "passed":true,"pilot_card_passed":true,"replay_deterministic":true,
        "cards_passed":cards_passed,"cards_credited":0,"cards_total":catalog.len(),"printings_total":printings.len(),
        "coverage_complete":false,"full_reward":false,"harbor_reward":0.0,"pilot_reward":0.0,
        "qualification_status":"unreviewed_diagnostic","mutation_sensitivity":mutations,
        "traces":[trace],"per_card_breakdown":breakdown
    }))
}

fn validate_definition() -> Result<(), String> {
    let candidate = candidate_putrefy::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Putrefy missing from RAV registry")?;
    if candidate != registered
        || candidate.id != CARD_ID
        || candidate.name != "Putrefy"
        || candidate.set_code != "RAV"
        || candidate.mana_cost != ManaCost::with_colors(1, [Color::Black, Color::Green])
        || candidate.colors != BTreeSet::from([Color::Black, Color::Green])
        || candidate.card_types != BTreeSet::from([CardType::Instant])
        || candidate.is_basic_land
        || candidate.power.is_some()
        || candidate.toughness.is_some()
        || !candidate.mana_colors.is_empty()
        || !candidate.keywords.is_empty()
        || candidate.effects != [Effect::DestroyTargetArtifactOrCreatureNoRegeneration]
        || candidate.effects[0].target_requirement()
            != Some(cardbench_magic_engine::TargetRequirement::ArtifactOrCreature)
    {
        return Err(
            "Putrefy candidate or registry differs from the exact RAV #221 definition".to_owned(),
        );
    }
    Ok(())
}

fn find_trace() -> Result<(u64, Captured), String> {
    let selected_seed = std::env::var("CARDBENCH_PUTREFY_SEED")
        .ok()
        .and_then(|raw| raw.parse::<u64>().ok());
    let seeds: Box<dyn Iterator<Item = u64>> = if let Some(seed) = selected_seed {
        Box::new(std::iter::once(seed))
    } else {
        Box::new(0..512)
    };
    for seed in seeds {
        if selected_seed.is_none() {
            let Ok(scout) = run_match_until(seed, Mutation::None, 24) else {
                continue;
            };
            let observed_cast = scout.putrefy.is_some();
            let observed_shield = scout.target.is_some_and(|target| scout.events.iter().any(|event| matches!(event, GameEvent::RegenerationShieldCreated { target: protected, .. } if *protected == target)));
            if !observed_cast || !observed_shield {
                continue;
            }
        }
        let Ok(captured) = run_match(seed, Mutation::None) else {
            continue;
        };
        match validate_trace(&captured, seed) {
            Ok(_) => return Ok((seed, captured)),
            Err(error) => {
                if selected_seed.is_some() {
                    return Err(format!(
                        "selected seed {seed} failed: {error}; terminal={}; events={:?}",
                        captured.terminal,
                        captured
                            .events
                            .iter()
                            .filter(|event| matches!(
                                event,
                                GameEvent::SpellCast { .. }
                                    | GameEvent::SpellManaPaid { .. }
                                    | GameEvent::CardDestroyed { .. }
                                    | GameEvent::RegenerationShieldCreated { .. }
                                    | GameEvent::RegenerationShieldUsed { .. }
                                    | GameEvent::AttackersDeclared { .. }
                                    | GameEvent::DamageDealtToPlayer { .. }
                            ))
                            .collect::<Vec<_>>()
                    ));
                }
            }
        }
    }
    Err("no bounded seed produced a full-game Putrefy destruction trace".to_owned())
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Ok(captured) => Ok(validate_trace(&captured, seed).is_err()),
        Err(error) if error.contains("front-face effect is unsupported") => Ok(true),
        Err(error) => Err(error),
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    run_match_until(seed, mutation, 72)
}

fn run_match_until(seed: u64, mutation: Mutation, stop_turn: u32) -> Result<Captured, String> {
    let pilot = DeckList {
        mainboard: vec![
            entry(CARD_ID, 4),
            entry(VOTARY_ID, 4),
            entry("RAV-FOREST", 17),
            entry("RAV-SWAMP", 17),
            entry("RAV-PLAINS", 18),
        ],
        sideboard: vec![],
    };
    let opponent = DeckList {
        mainboard: vec![
            entry(TARGET_ID, 4),
            entry("RAV-FOREST", 28),
            entry("RAV-PLAINS", 28),
        ],
        sideboard: vec![],
    };
    for (name, deck) in [(DECK_P0, &pilot), (DECK_P1, &opponent)] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|item| usize::from(item.count))
            .sum();
        if count != 60 {
            return Err(format!("{name} deck has {count} cards, expected 60"));
        }
    }
    let mut definitions = card_definitions();
    let mut submitted = candidate_putrefy::definition();
    match mutation {
        Mutation::None => {}
        Mutation::Removed => submitted.effects.clear(),
        Mutation::Regenerable => submitted.effects = vec![Effect::DestroyDistinctTargetCreature],
        Mutation::WrongCost => {
            submitted.mana_cost = ManaCost::with_colors(2, [Color::Black, Color::Green])
        }
    }
    *definitions
        .iter_mut()
        .find(|item| item.id == CARD_ID)
        .ok_or("Putrefy definition absent from registry")? = submitted;
    let mut game = Game::new_with_all_bindings(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
    )
    .map_err(|e| e.to_string())?;
    game.set_shuffle_seed(seed).map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot)
        .map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent)
        .map_err(|e| e.to_string())?;
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 72,
        ..DeckMatchConfig::default()
    };
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|e| e.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|e| e.to_string())?;
    game.begin_game().map_err(|e| e.to_string())?;
    let pilot_cost = if mutation == Mutation::WrongCost {
        ManaCost::with_colors(2, [Color::Black, Color::Green])
    } else {
        ManaCost::with_colors(1, [Color::Black, Color::Green])
    };
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(PutrefyPolicy::new(PlayerId(0), pilot_cost)),
        Box::new(PutrefyPolicy::new(
            PlayerId(1),
            ManaCost::with_colors(1, [Color::Black, Color::Green]),
        )),
    ];
    let mut accepted = 0u32;
    let mut attempted = 0u32;
    while !game.is_game_over()
        && game.turn <= config.max_turns.min(stop_turn)
        && accepted < config.max_policy_moves
    {
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("missing seat policy")?;
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
        attempted += 1;
        let action_debug = format!("{action:?}");
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|e| {
                format!(
                    "policy action rejected at turn {} {:?} ({action_debug}): {e}",
                    view.turn, view.step,
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|e| format!("post-action invariant failure: {e}"))?;
    }
    let events = game.event_log.clone();
    let find_cast = |player, id: &'static str| {
        events.iter().find_map(|event| match event {
            GameEvent::SpellCast {
                player: owner,
                card,
            } if *owner == player && game.card_definition(*card).is_ok_and(|def| def.id == id) => {
                Some(*card)
            }
            _ => None,
        })
    };
    let putrefy = find_cast(PlayerId(0), CARD_ID);
    let target = find_cast(PlayerId(1), TARGET_ID);
    let votary = events.iter().find_map(|event| match event {
        GameEvent::CardMoved {
            card,
            to: Zone::Battlefield,
            ..
        } if game
            .card_definition(*card)
            .is_ok_and(|def| def.id == VOTARY_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let payment = putrefy
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
        .unwrap_or_default();
    let zone = target.and_then(|card| {
        events.iter().rev().find_map(|event| match event {
            GameEvent::CardMoved { card: moved, to } if *moved == card => Some(*to),
            _ => None,
        })
    });
    let spell_zone = putrefy.and_then(|card| {
        events.iter().rev().find_map(|event| match event {
            GameEvent::CardMoved { card: moved, to } if *moved == card => Some(*to),
            _ => None,
        })
    });
    Ok(Captured {
        events,
        terminal: game.winner().map_or_else(
            || "Draw".to_owned(),
            |p| format!("Winner(PlayerId({}))", p.0),
        ),
        winner: game.winner().map(|p| p.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        putrefy,
        target,
        votary,
        payment,
        zone,
        spell_zone,
        life: [
            game.player(PlayerId(0)).map_err(|e| e.to_string())?.life,
            game.player(PlayerId(1)).map_err(|e| e.to_string())?.life,
        ],
    })
}

fn entry(card: &str, count: u8) -> DeckEntry {
    DeckEntry {
        card: card.to_owned(),
        count,
    }
}

struct PutrefyPolicy {
    player: PlayerId,
    shield_requested: bool,
    target_offered: bool,
    putrefy_cost: ManaCost,
}
impl PutrefyPolicy {
    fn new(player: PlayerId, putrefy_cost: ManaCost) -> Self {
        Self {
            player,
            shield_requested: false,
            target_offered: false,
            putrefy_cost,
        }
    }
}
impl CodePolicy for PutrefyPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.putrefy-removal-win.v1"
        } else {
            "rav.glass-golem-target.v1"
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
            let opponent_golem = view
                .opponent_battlefield
                .iter()
                .any(|card| card.definition == Some(TARGET_ID));
            let attackers = if self.player == PlayerId(0) && opponent_golem {
                vec![]
            } else {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(VOTARY_ID) && card.can_attack)
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
        }
        if self.player == PlayerId(0) {
            let target = view
                .opponent_battlefield
                .iter()
                .find(|card| card.definition == Some(TARGET_ID));
            if let Some(target) = target {
                if !self.shield_requested {
                    if let Some(votary) = view
                        .own_battlefield
                        .iter()
                        .find(|card| card.definition == Some(VOTARY_ID))
                    {
                        let cost = ManaCost::with_colors(0, [Color::Green]);
                        if payment(view, &cost).is_some() {
                            self.shield_requested = true;
                            return PolicyAction::ActivateAbility {
                                activation: AbilityActivation {
                                    source: votary.id,
                                    ability_id: ABILITY_ID,
                                    sacrifice_sources: vec![],
                                    additional_tap_creatures: vec![],
                                    discard_cards: vec![],
                                    targets: vec![Target::Permanent(target.id)],
                                },
                            };
                        }
                        if let Some(action) = make_mana(view, &cost) {
                            return action;
                        }
                    }
                } else if view.stack_depth == 0 {
                    if let Some(card) = view
                        .hand
                        .iter()
                        .find(|card| card.definition == Some(CARD_ID))
                    {
                        let cost = self.putrefy_cost.clone();
                        if let Some(selection) = payment(view, &cost) {
                            return PolicyAction::CastWithPayment {
                                request: CastRequest {
                                    card: card.id,
                                    targets: vec![Target::Permanent(target.id)],
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
            }
            if view.active_player == self.player
                && view.step == Step::PrecombatMain
                && view.stack_depth == 0
            {
                if view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(VOTARY_ID))
                    .count()
                    < 4
                {
                    if let Some(card) = view
                        .hand
                        .iter()
                        .find(|card| card.definition == Some(VOTARY_ID))
                    {
                        let cost = ManaCost::with_colors(0, [Color::White]);
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
                }
            }
        } else if view.active_player == self.player
            && view.step == Step::PrecombatMain
            && view.stack_depth == 0
        {
            if !self.target_offered {
                if let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(TARGET_ID))
                {
                    let cost = ManaCost::new(5);
                    if let Some(selection) = payment(view, &cost) {
                        self.target_offered = true;
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
        }
        // Continue playing lands in main phase after creatures are deployed.
        if view.active_player == self.player
            && view.step == Step::PrecombatMain
            && view.stack_depth == 0
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
}

fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut pool: Vec<_> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let (_, amount) = pool.iter_mut().find(|(found, _)| found == color)?;
        if *amount == 0 {
            return None;
        }
        *amount -= 1;
    }
    let mut generic = Vec::new();
    for _ in 0..cost.generic {
        let (color, amount) = pool.iter_mut().max_by_key(|(_, amount)| *amount)?;
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
    let mut need = None;
    for color in &cost.colored {
        let count = cost.colored.iter().filter(|item| *item == color).count() as u8;
        if view.mana_pool.amount(*color) < count {
            need = Some(*color);
            break;
        }
    }
    if need.is_none()
        && view.mana_pool.total() < cost.generic.saturating_add(cost.colored.len() as u8)
    {
        need = Some(Color::Green);
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
    let spell = c.putrefy.ok_or("Putrefy was not cast")?;
    let target = c.target.ok_or("opposing Glass Golem was not cast")?;
    let votary = c.votary.ok_or("Votary was not cast")?;
    if c.zone != Some(Zone::Graveyard) {
        return Err("Putrefy did not destroy its artifact-creature target".to_owned());
    }
    if c.spell_zone != Some(Zone::Graveyard) {
        return Err("resolved Putrefy did not reach its owner's graveyard".to_owned());
    }
    if c.payment.len() != 3
        || !c.payment.contains(&Color::Black)
        || !c.payment.contains(&Color::Green)
    {
        return Err(format!(
            "Putrefy did not pay exact {{1}}{{B}}{{G}}: {:?}",
            c.payment
        ));
    }
    if !c.events.iter().any(|event| matches!(event, GameEvent::RegenerationShieldCreated { target: protected, .. } if *protected == target)) { return Err("Votary did not create a regeneration shield on the target".to_owned()); }
    if c.events.iter().any(|event| matches!(event, GameEvent::RegenerationShieldUsed { target: protected, .. } if *protected == target)) { return Err("Putrefy consumed a regeneration shield instead of bypassing it".to_owned()); }
    let payment_i = find_index(&c.events, |e| matches!(e, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == spell && colors.len() == 3 && colors.contains(&Color::Black) && colors.contains(&Color::Green))).ok_or("exact Putrefy payment event is missing")?;
    let cast_i = find_after(
        &c.events,
        payment_i,
        |e| matches!(e, GameEvent::SpellCast { player: PlayerId(0), card } if *card == spell),
    )
    .ok_or("Putrefy cast receipt is missing")?;
    let resolved_i = find_index(
        &c.events,
        |e| matches!(e, GameEvent::SpellResolved { card } if *card == spell),
    )
    .ok_or("Putrefy did not resolve")?;
    let shield_i = find_index(&c.events, |e| matches!(e, GameEvent::RegenerationShieldCreated { target: protected, .. } if *protected == target)).ok_or("target regeneration shield receipt is missing")?;
    let destroyed_i = find_after(&c.events, cast_i, |e| matches!(e, GameEvent::CardDestroyed { source, card } if *source == spell && *card == target)).ok_or("Putrefy destruction receipt is missing")?;
    let moved_i = find_after(
        &c.events,
        destroyed_i,
        |e| matches!(e, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == target),
    )
    .ok_or("destroyed Glass Golem did not move to its owner's graveyard")?;
    let attacker = votary;
    let damage = 1;
    let attack_i = find_index(&c.events, |e| matches!(e, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&attacker))).ok_or("seat-zero creature did not attack")?;
    let damage_i = find_after(&c.events, attack_i, |e| matches!(e, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount } if *source == attacker && *amount == damage)).ok_or("seat-zero creature did not deal expected combat damage")?;
    let ended_i = find_after(&c.events, damage_i, |e| {
        matches!(
            e,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal win receipt is missing")?;
    if !(shield_i < payment_i
        && payment_i < cast_i
        && cast_i < destroyed_i
        && destroyed_i < moved_i
        && cast_i < resolved_i
        && resolved_i < attack_i
        && moved_i < attack_i
        && attack_i < damage_i
        && damage_i < ended_i)
    {
        return Err(
            "Putrefy shield, cast, destruction, combat, and terminal receipts are out of order"
                .to_owned(),
        );
    }
    if c.terminal != "Winner(PlayerId(0))"
        || c.winner != Some(0)
        || c.life[0] <= 0
        || c.life[1] > 0
        || c.rejected != 0
    {
        return Err(format!(
            "Putrefy duel did not reach a clean seat-zero win: {:?}",
            c.life
        ));
    }
    let projected = project_events(&c.events);
    let event_values: Vec<_> = projected.iter().map(|event| json!({"sequence":event.sequence,"turn":event.turn,"kind":&event.kind,"canonical":&event.canonical})).collect();
    Ok(
        json!({"id":"rav-putrefy-artifact-creature-no-regeneration","passed":true,"seed":seed,"decks":[DECK_P0,DECK_P1],"terminal":c.terminal,"winner_seat":c.winner,"turns":c.turns,"accepted_moves":c.accepted,"rejected_moves":c.rejected,"putrefy_object":spell.0,"target_glass_golem_object":target.0,"votary_object":votary.0,"terminal_attacker_object":attacker.0,"putrefy_payment":{"colors":c.payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"black_paid":c.payment.contains(&Color::Black),"green_paid":c.payment.contains(&Color::Green),"total_symbols_paid":c.payment.len()},"target_zone":format!("{:?}",c.zone),"putrefy_zone":format!("{:?}",c.spell_zone),"regeneration_shield_created":true,"regeneration_shield_used":false,"life_at_match_end":c.life,"ordered_event_sequence":[shield_i,payment_i,cast_i,destroyed_i,moved_i,resolved_i,attack_i,damage_i,ended_i],"events":event_values}),
    )
}
fn find_index(
    events: &[GameEvent],
    mut predicate: impl FnMut(&GameEvent) -> bool,
) -> Option<usize> {
    events.iter().position(|event| predicate(event))
}
fn find_after(
    events: &[GameEvent],
    after: usize,
    mut predicate: impl FnMut(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(after + 1)
        .find_map(|(i, event)| predicate(event).then_some(i))
}
fn card_status(card: &CatalogCard) -> Value {
    let (status, id) = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => {
            ("executable_compatibility_slice", Some(definition_id))
        }
        CardSemanticStatus::CatalogOnly { .. } => ("catalog_only", None),
    };
    let passed = id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"definition_id":id,"name":card.name,"engine_status":status,"qualification":if passed {"unreviewed_diagnostic"} else {"not_evaluated"},"trace_status":if passed {"passed"} else {"not_qualified"},"credited":false})
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
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(root.join("reward.txt"), "0.00000000\n").map_err(|e| e.to_string())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|e| format!("could not read Putrefy candidate source: {e}"))?;
    let file =
        syn::parse_file(&source).map_err(|_| "Putrefy source is not valid Rust".to_owned())?;
    if !file.attrs.iter().all(|attr| attr.path().is_ident("doc")) {
        return Err("Putrefy module has an unsupported module attribute".to_owned());
    }
    let mut imports = BTreeSet::new();
    let mut definitions = 0;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(item.vis, Visibility::Inherited) =>
            {
                if item.leading_colon.is_some() || !flatten_use(&[], &item.tree, &mut imports) {
                    return Err("Putrefy import shape differs from task source contract".to_owned());
                }
            }
            Item::Fn(item)
                if item.sig.ident == "definition"
                    && matches!(&item.vis, Visibility::Restricted(v) if v.path.is_ident("crate"))
                    && item.sig.inputs.is_empty()
                    && item.sig.generics.params.is_empty()
                    && item.block.stmts.len() == 1 =>
            {
                let Stmt::Expr(Expr::Struct(literal), None) = &item.block.stmts[0] else {
                    return Err("Putrefy must return a direct card definition literal".to_owned());
                };
                if literal.rest.is_some() {
                    return Err("Putrefy definition cannot use struct update syntax".to_owned());
                }
                definitions += 1;
            }
            _ => {
                return Err(
                    "Putrefy module allows only private imports and definition()".to_owned(),
                );
            }
        }
    }
    let expected: BTreeSet<String> = [
        "std::collections::BTreeSet",
        "cardbench_magic_engine::CardDefinition",
        "cardbench_magic_engine::CardType",
        "cardbench_magic_engine::Color",
        "cardbench_magic_engine::Effect",
        "cardbench_magic_engine::ManaCost",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    if imports != expected || definitions != 1 || file.items.len() != 3 {
        return Err("Putrefy source structure differs from its direct-literal contract".to_owned());
    }
    Ok(())
}
fn flatten_use(prefix: &[String], tree: &UseTree, out: &mut BTreeSet<String>) -> bool {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix.to_vec();
            next.push(path.ident.to_string());
            flatten_use(&next, &path.tree, out)
        }
        UseTree::Name(name) => {
            let mut next = prefix.to_vec();
            next.push(name.ident.to_string());
            out.insert(next.join("::"));
            true
        }
        UseTree::Group(group) => group
            .items
            .iter()
            .all(|item| flatten_use(prefix, item, out)),
        _ => false,
    }
}
