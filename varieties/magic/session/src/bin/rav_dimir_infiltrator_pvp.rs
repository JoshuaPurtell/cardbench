//! Isolated full-game RAV #203 Dimir Infiltrator authority.

use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DecisionKind, DecisionVisibility, DeckEntry,
    DeckList, Game, GameEvent, GameView, Keyword, LibrarySearchDestination, ManaCost,
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

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-DIMIR-INFILTRATOR";
const CARD_NAME: &str = "Dimir Infiltrator";
const SIGNET_ID: &str = "RAV-DIMIR-SIGNET";
const BLOCKER_ID: &str = "RAV-CARVEN-CARYATID";
const TRANSMUTE_ID: &str = "transmute";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/dimir_infiltrator.rs";
const MAX_TURNS: u32 = 80;
const MAX_MOVES: u32 = 8_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/dimir_infiltrator.rs"]
mod candidate_dimir_infiltrator;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Dimir Infiltrator PvP result: {error}");
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
            eprintln!("Dimir Infiltrator PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongManaCost,
    WrongPower,
    WrongToughness,
    RemoveUnblockable,
    RemoveTransmute,
    WrongTransmuteCost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    life: [i64; 2],
    source: Option<ObjectId>,
    source_incarnation: Option<u64>,
    resolved_source_incarnation: Option<u64>,
    source_mana_value: Option<u8>,
    source_zone_is_graveyard: bool,
    activation_cost: Option<ManaCost>,
    activation_step: Option<Step>,
    transmute_resolved: bool,
    selection: Option<ObjectId>,
    found: Option<ObjectId>,
    found_definition: Option<&'static str>,
    found_mana_value: Option<u8>,
    found_in_hand: bool,
    private_search_seen: bool,
    hidden_from_opponent: bool,
    library_search_resolved: bool,
    post_search_shuffle: bool,
    infiltrators_cast: Vec<ObjectId>,
    live_infiltrator_count: usize,
    live_characteristics_valid: bool,
    blocker_present_for_attack: bool,
    blocker_assignments_to_infiltrator: usize,
    attacker_declarations: usize,
    combat_damage_to_opponent: i32,
    damage_sources: BTreeSet<ObjectId>,
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate()?;
    let catalog = unique_catalog()?;
    let (seed, capture) = find_trace()?;
    let trace = validate_trace(&capture, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if capture != replay || project_events(&capture.events) != project_events(&replay.events) {
        return Err("Dimir Infiltrator shuffled PvP replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "wrong_mana_cost_detected": mutation_detected(seed, Mutation::WrongManaCost)?,
        "wrong_power_detected": mutation_detected(seed, Mutation::WrongPower)?,
        "wrong_toughness_detected": mutation_detected(seed, Mutation::WrongToughness)?,
        "unblockable_removed_detected": mutation_detected(seed, Mutation::RemoveUnblockable)?,
        "transmute_removed_detected": mutation_detected(seed, Mutation::RemoveTransmute)?,
        "transmute_cost_changed_detected": mutation_detected(seed, Mutation::WrongTransmuteCost)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "wrong_mana_cost_detected",
        "wrong_power_detected",
        "wrong_toughness_detected",
        "unblockable_removed_detected",
        "transmute_removed_detected",
        "transmute_cost_changed_detected",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Dimir Infiltrator semantic mutation escaped: {key}"
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
        "card": {"collector_number": 203, "printings": 1, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117: priority and sorcery timing",
                "202.1-202.2 and 208.1: color, mana cost, and characteristics",
                "302.1-302.6: creature characteristics",
                "508-509: attacker and blocker declarations",
                "702.53: Transmute",
                "701.19: library searching and shuffle"
            ],
            "source_contract": "RAV #203 Dimir Infiltrator is a blue-black {U}{B} 1/3 Spirit with unblockable and Transmute {1}{U}{B}",
            "transmute_contract": "The RAV Transmute rules require the hand-zone source to be discarded, a card with the same mana value to be found and revealed, then put into hand before the library is shuffled",
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
            "normalized_literal_source_checked_before_candidate_calls": true,
            "direct_ast_checked_before_candidate_calls": true
        },
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn candidate_definition(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_dimir_infiltrator::definition();
    match mutation {
        Mutation::None => {}
        Mutation::WrongManaCost => {
            definition.mana_cost = ManaCost::with_colors(1, [Color::Blue, Color::Black]);
        }
        Mutation::WrongPower => definition.power = Some(2),
        Mutation::WrongToughness => definition.toughness = Some(4),
        Mutation::RemoveUnblockable => definition
            .keywords
            .retain(|keyword| !matches!(keyword, Keyword::Unblockable)),
        Mutation::RemoveTransmute => definition
            .keywords
            .retain(|keyword| !matches!(keyword, Keyword::Transmute(_))),
        Mutation::WrongTransmuteCost => {
            definition.keywords = vec![
                Keyword::Unblockable,
                Keyword::Transmute(ManaCost::with_colors(1, [Color::Blue, Color::Blue])),
            ];
        }
    }
    definition
}

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Dimir Infiltrator is missing from the RAV definition registry")? =
        candidate_definition(mutation);
    Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .map_err(|error| error.to_string())
}

fn find_trace() -> Result<(u64, Captured), String> {
    for seed in 0..=128 {
        if let Ok(capture) = run_match(seed, Mutation::None)
            && validate_trace(&capture, seed).is_ok()
        {
            return Ok((seed, capture));
        }
    }
    Err("no complete Dimir Infiltrator PvP trace found for seeds 0..=128".to_owned())
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Err(_) => Ok(true),
        Ok(capture) => Ok(validate_trace(&capture, seed).is_err()),
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot = deck(&[
        (CARD_ID, 4),
        (SIGNET_ID, 4),
        ("RAV-ISLAND", 26),
        ("RAV-SWAMP", 26),
    ]);
    let opponent = deck(&[(BLOCKER_ID, 4), ("RAV-FOREST", 56)]);
    for (label, list) in [("pilot", &pilot), ("opponent", &opponent)] {
        if list
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
        {
            return Err(format!(
                "Dimir Infiltrator {label} deck is not a legal 60-card deck"
            ));
        }
    }
    let mut game = new_game(mutation)?;
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_MOVES,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    for (player, list) in [(PlayerId(0), &pilot), (PlayerId(1), &opponent)] {
        game.load_deck_into_library(player, list)
            .map_err(|error| error.to_string())?;
        game.draw_opening_hand(player, config.opening_hand_size)
            .map_err(|error| error.to_string())?;
    }
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(InfiltratorPolicy::new(PlayerId(0), true, mutation)),
        Box::new(InfiltratorPolicy::new(PlayerId(1), false, Mutation::None)),
    ];
    let mut capture = Captured {
        events: vec![],
        winner: None,
        turns: 0,
        accepted_moves: 0,
        rejected_moves: 0,
        life: [20, 20],
        source: None,
        source_incarnation: None,
        resolved_source_incarnation: None,
        source_mana_value: None,
        source_zone_is_graveyard: false,
        activation_cost: None,
        activation_step: None,
        transmute_resolved: false,
        selection: None,
        found: None,
        found_definition: None,
        found_mana_value: None,
        found_in_hand: false,
        private_search_seen: false,
        hidden_from_opponent: true,
        library_search_resolved: false,
        post_search_shuffle: false,
        infiltrators_cast: vec![],
        live_infiltrator_count: 0,
        live_characteristics_valid: false,
        blocker_present_for_attack: false,
        blocker_assignments_to_infiltrator: 0,
        attacker_declarations: 0,
        combat_damage_to_opponent: 0,
        damage_sources: BTreeSet::new(),
    };
    while !game.is_game_over() {
        if game.turn > config.max_turns || capture.accepted_moves >= config.max_policy_moves {
            return Err(format!(
                "Dimir Infiltrator duel exceeded its bound at turn {} with life {:?}",
                game.turn,
                game.players
                    .iter()
                    .map(|player| player.life)
                    .collect::<Vec<_>>()
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Dimir Infiltrator policy seat missing")?;
        let action = if let Some(search) = &view.library_search_choice {
            if player == PlayerId(0) && Some(search.source) == capture.source {
                capture.private_search_seen = true;
                let opponent_view = game
                    .view_for_player(PlayerId(1))
                    .map_err(|error| error.to_string())?;
                capture.hidden_from_opponent &= opponent_view.library_search_choice.is_none();
            }
            policy.propose_library_search_choice(&view)
        } else if let Some(action) = policy.propose_pending_decision(&view) {
            action
        } else if view.optional_triggered_ability_choice.is_some() {
            policy.propose_optional_triggered_ability(&view)
        } else if view.draw_replacement_pending {
            policy.propose_draw_replacement(&view)
        } else if view.private_library_choice.is_some() {
            policy.propose_private_library_choice(&view)
        } else if view.private_opponent_library_choice.is_some() {
            policy.propose_private_opponent_library_choice(&view)
        } else {
            policy.propose_move(&view)
        };
        if let PolicyAction::Transmute { card } = &action
            && player == PlayerId(0)
            && capture.source.is_none()
        {
            capture.source = Some(*card);
            capture.source_incarnation = game.object(*card).ok().map(|object| object.incarnation);
            capture.source_mana_value = game
                .card_definition(*card)
                .ok()
                .map(|definition| definition.mana_cost.mana_value());
            capture.activation_step = Some(view.step);
        }
        if let PolicyAction::ChooseLibrarySearchCard {
            source, selected, ..
        } = &action
            && Some(*source) == capture.source
        {
            capture.selection = *selected;
        }
        let cursor = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Dimir Infiltrator policy move rejected on turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        capture.accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        observe(&game, &game.event_log[cursor..], &mut capture)?;
    }
    capture.events = game.event_log.clone();
    capture.winner = game.winner().map(|player| player.0);
    capture.turns = game.turn;
    capture.life = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    let pilot_view = game
        .view_for_player(PlayerId(0))
        .map_err(|error| error.to_string())?;
    let live = pilot_view
        .own_battlefield
        .iter()
        .filter(|card| card.definition == Some(CARD_ID))
        .collect::<Vec<_>>();
    capture.live_infiltrator_count = live.len();
    capture.live_characteristics_valid = !live.is_empty()
        && live.iter().all(|card| {
            let Ok(characteristics) = game.characteristics(card.id) else {
                return false;
            };
            characteristics.colors == BTreeSet::from([Color::Blue, Color::Black])
                && characteristics.card_types.contains(&CardType::Creature)
                && characteristics.power == Some(1)
                && characteristics.toughness == Some(3)
                && characteristics.keywords.contains(&Keyword::Unblockable)
                && characteristics.keywords.iter().any(|keyword| {
                    matches!(keyword, Keyword::Transmute(cost) if *cost == ManaCost::with_colors(1, [Color::Blue, Color::Black]))
                })
        });
    Ok(capture)
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

fn observe(game: &Game, events: &[GameEvent], capture: &mut Captured) -> Result<(), String> {
    for event in events {
        match event {
            GameEvent::AbilityManaPaid {
                player: PlayerId(0),
                source,
                ability: TRANSMUTE_ID,
                mana_cost,
            } if Some(*source) == capture.source => {
                capture.activation_cost = Some(mana_cost.clone())
            }
            GameEvent::AbilityActivated {
                player: PlayerId(0),
                source,
                source_incarnation,
                definition,
                ability: TRANSMUTE_ID,
            } if Some(*source) == capture.source && *definition == CARD_ID => {
                capture.source_incarnation = Some(*source_incarnation);
            }
            GameEvent::AbilityResolved {
                source,
                source_incarnation,
                ability: TRANSMUTE_ID,
            } if Some(*source) == capture.source => {
                capture.transmute_resolved = true;
                capture.resolved_source_incarnation = Some(*source_incarnation);
            }
            GameEvent::CardRevealed {
                player: PlayerId(0),
                card,
                definition,
            } if Some(*card) == capture.selection => capture.found_definition = Some(definition),
            GameEvent::LibrarySearchResolved {
                player: PlayerId(0),
                source,
                found,
                destination: LibrarySearchDestination::Hand,
            } if Some(*source) == capture.source => {
                capture.library_search_resolved = true;
                capture.found = *found;
            }
            GameEvent::Transmuted {
                player: PlayerId(0),
                discarded,
                found,
            } if Some(*discarded) == capture.source => capture.found = *found,
            GameEvent::CardMoved {
                card,
                to: Zone::Hand,
            } if Some(*card) == capture.selection => {
                capture.found = Some(*card);
                capture.found_in_hand = true;
                capture.found_mana_value = game
                    .card_definition(*card)
                    .ok()
                    .map(|definition| definition.mana_cost.mana_value());
            }
            GameEvent::CardMoved {
                card,
                to: Zone::Graveyard,
            } if Some(*card) == capture.source => capture.source_zone_is_graveyard = true,
            GameEvent::LibraryShuffled {
                player: PlayerId(0),
                ..
            } if capture.library_search_resolved => capture.post_search_shuffle = true,
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                capture.infiltrators_cast.push(*card);
            }
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } if attackers.iter().any(|attacker| {
                game.card_definition(*attacker)
                    .is_ok_and(|definition| definition.id == CARD_ID)
            }) =>
            {
                capture.attacker_declarations += 1;
                let view = game
                    .view_for_player(PlayerId(0))
                    .map_err(|error| error.to_string())?;
                capture.blocker_present_for_attack |= view
                    .opponent_battlefield
                    .iter()
                    .any(|card| card.definition == Some(BLOCKER_ID) && card.can_block);
            }
            GameEvent::BlockersDeclared {
                player: PlayerId(1),
                assignments,
            } => {
                capture.blocker_assignments_to_infiltrator += assignments
                    .iter()
                    .filter(|(attacker, _)| {
                        game.card_definition(*attacker)
                            .is_ok_and(|definition| definition.id == CARD_ID)
                    })
                    .count();
            }
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if game
                .card_definition(*source)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                capture.combat_damage_to_opponent += *amount;
                capture.damage_sources.insert(*source);
            }
            _ => {}
        }
    }
    Ok(())
}

struct InfiltratorPolicy {
    player: PlayerId,
    pilot: bool,
    mutation: Mutation,
    inner: RavCatalogPolicy,
    transmute_started: bool,
    found_signet: Option<ObjectId>,
}

impl InfiltratorPolicy {
    fn new(player: PlayerId, pilot: bool, mutation: Mutation) -> Self {
        Self {
            player,
            pilot,
            mutation,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
            transmute_started: false,
            found_signet: None,
        }
    }

    fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let mut available = Color::MANA_ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect::<Vec<_>>();
        for color in &cost.colored {
            let amount = &mut available.iter_mut().find(|(known, _)| known == color)?.1;
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
        let mut required = BTreeMap::<Color, u8>::new();
        for color in &cost.colored {
            *required.entry(*color).or_default() += 1;
        }
        for (color, amount) in required {
            if view.mana_pool.amount(color) < amount {
                let land = view.own_battlefield.iter().find(|card| {
                    card.card_types.contains(&CardType::Land)
                        && !card.tapped
                        && card.mana_colors.contains(&color)
                })?;
                return Some(PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color,
                });
            }
        }
        if view.mana_pool.total() < u16::from(cost.mana_value()) {
            let land = view.own_battlefield.iter().find(|card| {
                card.card_types.contains(&CardType::Land)
                    && !card.tapped
                    && !card.mana_colors.is_empty()
            })?;
            let color = if land.mana_colors.contains(&Color::Blue) {
                Color::Blue
            } else if land.mana_colors.contains(&Color::Black) {
                Color::Black
            } else {
                *land.mana_colors.iter().next()?
            };
            return Some(PolicyAction::ActivateManaAbility {
                land: land.id,
                color,
            });
        }
        None
    }

    fn play_land(&mut self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        if self.pilot {
            let blue = view
                .own_battlefield
                .iter()
                .filter(|card| {
                    card.card_types.contains(&CardType::Land)
                        && card.mana_colors.contains(&Color::Blue)
                })
                .count();
            let black = view
                .own_battlefield
                .iter()
                .filter(|card| {
                    card.card_types.contains(&CardType::Land)
                        && card.mana_colors.contains(&Color::Black)
                })
                .count();
            let preferred = if blue <= black {
                "RAV-ISLAND"
            } else {
                "RAV-SWAMP"
            };
            return view
                .hand
                .iter()
                .find(|card| card.definition == Some(preferred))
                .or_else(|| {
                    view.hand
                        .iter()
                        .find(|card| card.card_types.contains(&CardType::Land))
                })
                .map(|card| PolicyAction::PlayLand { card: card.id });
        }
        view.hand
            .iter()
            .find(|card| card.definition == Some("RAV-FOREST"))
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            })
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }

    fn cast_creature(
        &self,
        view: &GameView,
        card_id: &'static str,
        cost: &ManaCost,
    ) -> Option<PolicyAction> {
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(card_id))?;
        if let Some(mana_selection) = Self::payment(view, cost) {
            Some(PolicyAction::CastWithPayment {
                request: CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection,
            })
        } else {
            Self::make_mana(view, cost)
        }
    }
}

impl CodePolicy for InfiltratorPolicy {
    fn id(&self) -> &'static str {
        if self.pilot {
            "rav.dimir-infiltrator-transmute-unblockable-pilot.v1"
        } else {
            "rav.dimir-infiltrator-caryatid-opponent.v1"
        }
    }

    fn propose_library_search_choice(&mut self, view: &GameView) -> PolicyAction {
        let choice = view
            .library_search_choice
            .as_ref()
            .expect("library search choice is visible to its controller");
        let selected = choice
            .cards
            .iter()
            .find(|card| card.definition == Some(SIGNET_ID))
            .map(|card| card.id);
        if self.pilot {
            self.found_signet = selected;
        }
        PolicyAction::ChooseLibrarySearchCard {
            decision: choice.decision,
            source: choice.source,
            selected,
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
            if !self.pilot || !self.transmute_started {
                return PolicyAction::DeclareAttackers { attackers: vec![] };
            }
            let has_ready_blocker = view
                .opponent_battlefield
                .iter()
                .any(|card| card.definition == Some(BLOCKER_ID));
            if !has_ready_blocker {
                return PolicyAction::DeclareAttackers { attackers: vec![] };
            }
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(CARD_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect(),
            };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            if self.pilot {
                return PolicyAction::DeclareBlockers {
                    assignments: vec![],
                };
            }
            let mut used = BTreeSet::new();
            let mut assignments = Vec::new();
            for attacker in view
                .combat_attackers
                .iter()
                .filter(|card| card.definition == Some(CARD_ID))
            {
                if attacker.keywords.contains(&Keyword::Unblockable) {
                    continue;
                }
                if let Some(blocker) = view.own_battlefield.iter().find(|card| {
                    card.definition == Some(BLOCKER_ID)
                        && card.can_block
                        && !card.tapped
                        && !used.contains(&card.id)
                }) {
                    used.insert(blocker.id);
                    assignments.push(cardbench_magic_engine::CombatBlock {
                        attacker: attacker.id,
                        blocker: blocker.id,
                    });
                }
            }
            return PolicyAction::DeclareBlockers { assignments };
        }
        if view.priority != self.player || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }
        if view.active_player == self.player && view.step == Step::PrecombatMain {
            if let Some(action) = self.play_land(view) {
                return action;
            }
            if self.pilot {
                if !self.transmute_started
                    && let Some(cost) = candidate_definition(self.mutation)
                        .transmute_cost()
                        .cloned()
                {
                    let copies_in_hand = view
                        .hand
                        .iter()
                        .filter(|card| card.definition == Some(CARD_ID))
                        .count();
                    if copies_in_hand >= 2 {
                        if Self::payment(view, &cost).is_some() {
                            self.transmute_started = true;
                            return PolicyAction::Transmute {
                                card: view
                                    .hand
                                    .iter()
                                    .find(|card| card.definition == Some(CARD_ID))
                                    .expect("source card was counted")
                                    .id,
                            };
                        }
                        if let Some(action) = Self::make_mana(view, &cost) {
                            return action;
                        }
                    }
                }
                let creature_cost = candidate_definition(self.mutation).mana_cost;
                if self.transmute_started
                    && let Some(action) = self.cast_creature(view, CARD_ID, &creature_cost)
                {
                    return action;
                }
                return PolicyAction::PassPriority;
            }
            if !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(BLOCKER_ID))
                && let Some(action) = self.cast_creature(
                    view,
                    BLOCKER_ID,
                    &ManaCost::with_colors(1, [Color::Green, Color::Green]),
                )
            {
                return action;
            }
        }
        self.inner.propose_move(view)
    }

    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        self.inner.propose_pending_decision(view)
    }
    fn propose_optional_triggered_ability(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_optional_triggered_ability(view)
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
}

fn validate_trace(capture: &Captured, seed: u64) -> Result<Value, String> {
    if capture.winner != Some(0) || capture.rejected_moves != 0 {
        return Err(format!(
            "Dimir Infiltrator duel did not reach a clean P0 win: winner={:?}, turns={}, rejected={}, damage={}, life={:?}",
            capture.winner,
            capture.turns,
            capture.rejected_moves,
            capture.combat_damage_to_opponent,
            capture.life
        ));
    }
    let source = capture
        .source
        .ok_or("Dimir Infiltrator Transmute was not activated")?;
    let found = capture
        .found
        .ok_or("Dimir Infiltrator Transmute found no card")?;
    let expected_transmute = ManaCost::with_colors(1, [Color::Blue, Color::Black]);
    if source == found
        || capture.source_incarnation.is_none()
        || capture.source_incarnation != capture.resolved_source_incarnation
        || capture.source_mana_value != Some(2)
        || capture.found_mana_value != Some(2)
        || capture.activation_cost.as_ref() != Some(&expected_transmute)
        || !matches!(
            capture.activation_step,
            Some(Step::PrecombatMain | Step::PostcombatMain)
        )
        || !capture.source_zone_is_graveyard
        || !capture.transmute_resolved
        || capture.selection != Some(found)
        || capture.found_definition != Some(SIGNET_ID)
        || !capture.found_in_hand
        || !capture.private_search_seen
        || !capture.hidden_from_opponent
        || !capture.library_search_resolved
        || !capture.post_search_shuffle
    {
        return Err("Dimir Infiltrator Transmute timing, cost, private selection, mana value, or source lifecycle differs".to_owned());
    }
    if capture.infiltrators_cast.len() < 2
        || capture.live_infiltrator_count == 0
        || !capture.live_characteristics_valid
        || !capture.blocker_present_for_attack
        || capture.blocker_assignments_to_infiltrator != 0
        || capture.attacker_declarations == 0
        || capture.combat_damage_to_opponent < 20
        || capture.damage_sources.is_empty()
        || capture.life[0] <= 0
        || capture.life[1] > 0
    {
        return Err(format!(
            "Dimir Infiltrator unblockable-combat trace differs: cast={}, live={}, chars={}, blocker={}, block_assignments={}, attacks={}, damage={}, life={:?}",
            capture.infiltrators_cast.len(),
            capture.live_infiltrator_count,
            capture.live_characteristics_valid,
            capture.blocker_present_for_attack,
            capture.blocker_assignments_to_infiltrator,
            capture.attacker_declarations,
            capture.combat_damage_to_opponent,
            capture.life
        ));
    }
    let mana_paid = event_index(&capture.events, |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source: object, ability: TRANSMUTE_ID, mana_cost } if *object == source && *mana_cost == expected_transmute)).ok_or("Dimir Infiltrator Transmute mana payment event missing")?;
    let discarded_as_cost = event_index(&capture.events, |event| matches!(event, GameEvent::DiscardedAsAbilityCost { player: PlayerId(0), source: object, card } if *object == source && *card == source)).ok_or("Dimir Infiltrator Transmute discard-cost event missing")?;
    let source_discard = event_index(&capture.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == source)).ok_or("Dimir Infiltrator Transmute source discard event missing")?;
    let source_activation = event_index(&capture.events, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source: object, definition: CARD_ID, ability: TRANSMUTE_ID, .. } if *object == source)).ok_or("Dimir Infiltrator Transmute activation event missing")?;
    let decision_opened = event_index_after(&capture.events, source_activation, |event| {
        matches!(
            event,
            GameEvent::DecisionOpened {
                player: PlayerId(0),
                kind: DecisionKind::LibrarySearch,
                visibility: DecisionVisibility::Private,
                ..
            }
        )
    })
    .ok_or("private Dimir Infiltrator library-search decision missing")?;
    let decision_id = match &capture.events[decision_opened] {
        GameEvent::DecisionOpened { decision, .. } => *decision,
        _ => unreachable!(),
    };
    let decision_completed = event_index_after(&capture.events, decision_opened, |event| matches!(event, GameEvent::DecisionCompleted { decision, player: PlayerId(0), kind: DecisionKind::LibrarySearch } if *decision == decision_id)).ok_or("private Dimir Infiltrator library-search decision completion missing")?;
    let revealed = event_index_after(&capture.events, decision_completed, |event| matches!(event, GameEvent::CardRevealed { player: PlayerId(0), card, definition: SIGNET_ID } if *card == found)).ok_or("Dimir Signet reveal event missing")?;
    let moved = event_index_after(
        &capture.events,
        revealed,
        |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == found),
    )
    .ok_or("Dimir Signet hand movement event missing")?;
    let search = event_index_after(&capture.events, moved, |event| matches!(event, GameEvent::LibrarySearchResolved { player: PlayerId(0), source: object, found: Some(card), destination: LibrarySearchDestination::Hand } if *object == source && *card == found)).ok_or("Dimir Infiltrator same-mana-value library search event missing")?;
    let shuffle = event_index_after(&capture.events, moved, |event| {
        matches!(
            event,
            GameEvent::LibraryShuffled {
                player: PlayerId(0),
                ..
            }
        )
    })
    .ok_or("post-Transmute library shuffle event missing")?;
    let transmuted = event_index_after(&capture.events, shuffle, |event| matches!(event, GameEvent::Transmuted { player: PlayerId(0), discarded, found: Some(selected) } if *discarded == source && *selected == found)).ok_or("Dimir Infiltrator Transmuted receipt missing")?;
    let resolved = event_index_after(&capture.events, transmuted, |event| matches!(event, GameEvent::AbilityResolved { source: object, ability: TRANSMUTE_ID, .. } if *object == source)).ok_or("Dimir Infiltrator Transmute resolution receipt missing")?;
    if !(mana_paid < discarded_as_cost
        && discarded_as_cost < source_discard
        && source_discard < source_activation
        && source_activation < decision_opened
        && decision_opened < decision_completed
        && decision_completed < revealed
        && revealed < moved
        && moved < search
        && search < shuffle
        && shuffle < transmuted
        && transmuted < resolved)
    {
        return Err("Dimir Infiltrator Transmute event order differs".to_owned());
    }
    Ok(json!({
        "trace_id": "rav-dimir-infiltrator-transmute-unblockable-full-game",
        "passed": true,
        "seed": seed,
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": 0,
        "turns": capture.turns,
        "accepted_moves": capture.accepted_moves,
        "rejected_moves": capture.rejected_moves,
        "deck_p0": "4 Dimir Infiltrator + 4 Dimir Signet + 52 Islands/Swamps (60 shuffled cards)",
        "deck_p1": "4 Carven Caryatid + 56 Forests (60 shuffled cards)",
        "transmute": {
            "source": source.0,
            "source_incarnation": capture.source_incarnation,
            "mana_value": capture.source_mana_value,
            "cost": "{1}{U}{B}",
            "step": format!("{:?}", capture.activation_step),
            "discarded_to_graveyard": capture.source_zone_is_graveyard,
            "selected": found.0,
            "selected_definition": capture.found_definition,
            "selected_mana_value": capture.found_mana_value,
            "revealed": true,
            "put_into_hand": capture.found_in_hand,
            "private_to_controller": capture.hidden_from_opponent,
            "library_shuffled": capture.post_search_shuffle
        },
        "combat": {
            "infiltrators_cast": capture.infiltrators_cast.len(),
            "live_unblockable_infiltrators": capture.live_infiltrator_count,
            "available_carven_caryatid": capture.blocker_present_for_attack,
            "block_assignments_to_infiltrator": capture.blocker_assignments_to_infiltrator,
            "combat_damage_to_opponent": capture.combat_damage_to_opponent,
            "damage_source_count": capture.damage_sources.len()
        },
        "life_totals": capture.life,
        "events": project_events(&capture.events),
        "event_count": capture.events.len(),
        "ordered_event_sequence": [mana_paid, discarded_as_cost, source_discard, source_activation, decision_opened, decision_completed, revealed, moved, search, shuffle, transmuted, resolved],
        "mechanics": {
            "definition_id": CARD_ID,
            "mana_cost": "{U}{B}",
            "power": 1,
            "toughness": 3,
            "colors": ["Black", "Blue"],
            "unblockable": true,
            "transmute_cost": "{1}{U}{B}",
            "mana_value": 2
        }
    }))
}

fn validate_candidate() -> Result<(), String> {
    let definition = candidate_dimir_infiltrator::definition();
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(0, [Color::Blue, Color::Black])
        || definition.colors != BTreeSet::from([Color::Blue, Color::Black])
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || (definition.power, definition.toughness) != (Some(1), Some(3))
        || !definition.keywords.contains(&Keyword::Unblockable)
        || !definition.keywords.iter().any(|keyword| matches!(keyword, Keyword::Transmute(cost) if *cost == ManaCost::with_colors(1, [Color::Blue, Color::Black])))
    {
        return Err("candidate definition differs from RAV #203".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Dimir Infiltrator candidate source: {error}"))?;
    if source.len() > 32 * 1024
        || source.contains("include!")
        || source.contains("std::fs")
        || source.contains("std::process")
    {
        return Err("Dimir Infiltrator source exceeds the declarative module boundary".to_owned());
    }
    syn::parse_file(&source)
        .map_err(|_| "Dimir Infiltrator source is not valid Rust".to_owned())?;
    if source.trim() != EXPECTED_SOURCE.trim() {
        return Err(
            "Dimir Infiltrator source differs from the pinned literal card module".to_owned(),
        );
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
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({
        "collector_number": card.collector_number,
        "name": card.name,
        "definition_id": definition_id,
        "engine_status": match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "credited": false,
        "qualification": if passed { "pilot trace passed; independent review pending" } else { "not_evaluated" }
    })
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}

fn event_index_after(
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

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "candidate"
    } else {
        "reference"
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
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).expect("report serializes"),
    )?;
    std::fs::write(root.join("reward.txt"), "0.0\n")
}

const EXPECTED_SOURCE: &str = r#"//! Candidate-owned RAV Dimir Infiltrator definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Keyword, ManaCost};

/// RAV #203 is an unblockable blue-black 1/3 Spirit with Transmute `{1}{U}{B}`.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-DIMIR-INFILTRATOR",
        name: "Dimir Infiltrator",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(0, [Color::Blue, Color::Black]),
        colors: BTreeSet::from([Color::Blue, Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "unblockable",
            "stack-backed-private-transmute",
        ],
        power: Some(1),
        toughness: Some(3),
        keywords: vec![
            Keyword::Unblockable,
            Keyword::Transmute(ManaCost::with_colors(1, [Color::Blue, Color::Black])),
        ],
        effects: vec![],
    }
}
"#;
