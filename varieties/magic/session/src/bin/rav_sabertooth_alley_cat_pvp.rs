//! Full-game RAV Sabertooth Alley Cat authority for the Mountain block gate.

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, CombatBlock, DeckEntry, DeckList, Game,
    GameEvent, GameView, Keyword, ManaAbilityActivation, ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, Step, Zone,
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

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-SABERTOOTH-ALLEY-CAT";
const CARD_NAME: &str = "Sabertooth Alley Cat";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/sabertooth_alley_cat.rs";
const BIRD_ID: &str = "RAV-BIRDS-OF-PARADISE";
const SIGNET_ID: &str = "RAV-BOROS-SIGNET";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const ATTACKER_ID: &str = "RAV-WAR-TORCH-GOBLIN";
const REFERENCE_SEED: u64 = 19;
const SET_CODE: &str = "RAV";

fn colors<const N: usize>(values: [Color; N]) -> BTreeSet<Color> {
    values.into_iter().collect()
}
fn types<const N: usize>(values: [CardType; N]) -> BTreeSet<CardType> {
    values.into_iter().collect()
}

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/sabertooth_alley_cat.rs"]
mod candidate_sabertooth_alley_cat;

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
            eprintln!("Sabertooth Alley Cat PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_source()?;
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
        || catalog.len() != 291
        || printings.len() != 306
    {
        return Err("RAV catalog denominator differs from the source contract".to_owned());
    }
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.winner != replay.winner
        || captured.cat != replay.cat
        || captured.cat_payment != replay.cat_payment
        || captured.cat_cast_without_mountain != replay.cat_cast_without_mountain
        || captured.failed_block_atomic != replay.failed_block_atomic
        || captured.failed_block_position != replay.failed_block_position
        || captured.legal_block_with_mountain != replay.legal_block_with_mountain
        || captured.cat_zone != replay.cat_zone
        || captured.cat_blocked_attacker != replay.cat_blocked_attacker
        || captured.cat_blocked_attacker_zone != replay.cat_blocked_attacker_zone
    {
        return Err("Sabertooth Alley Cat shuffled full-game replay differed".to_owned());
    }
    let mutations = json!({
        "mountain_block_restriction_removal_detected": mutation_detected(seed, Mutation::NoRestriction),
        "wrong_exact_red_cost_detected": mutation_detected(seed, Mutation::WrongCost),
        "wrong_power_detected": mutation_detected(seed, Mutation::WrongPower),
        "sealed_mutation_authority": false
    });
    for key in [
        "mountain_block_restriction_removal_detected",
        "wrong_exact_red_cost_detected",
        "wrong_power_detected",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Sabertooth Alley Cat gameplay mutation escaped: {key}"
            ));
        }
    }
    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let passed_count = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,
        "expansion":"RAV",
        "card":{"collector_number":140,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}", ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":["117 Priority", "302.6 Creature blocking restrictions", "305.6 Basic land types and subtype identity", "509.1 Blocker declaration", "510.1-510.2 Combat damage", "601.2 Casting and exact colored costs", "608.3a Permanent spell resolution"],
            "source_contract":"RAV Sabertooth Alley Cat #140, {1}{R}{R} red 2/1 creature with the condition that it cannot block unless its controller controls a Mountain.",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true
        },
        "passed":passed_count == 1,
        "pilot_card_passed":passed_count == 1,
        "cards_passed":passed_count,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":printings.len(),
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":"unreviewed_diagnostic",
        "replay_deterministic":true,
        "mutation_sensitivity":mutations,
        "traces":[trace],
        "per_card_breakdown":breakdown
    }))
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn validate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Sabertooth Alley Cat module: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Sabertooth Alley Cat source is not valid Rust".to_owned())?;
    if file
        .items
        .iter()
        .filter(|item| matches!(item, syn::Item::Fn(_)))
        .count()
        != 1
        || file
            .items
            .iter()
            .any(|item| !matches!(item, syn::Item::Use(_) | syn::Item::Fn(_)))
    {
        return Err(
            "Sabertooth Alley Cat source must contain only private imports and one literal factory"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_definition() -> Result<(), String> {
    let definition = candidate_sabertooth_alley_cat::definition();
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != SET_CODE
        || definition.mana_cost != ManaCost::with_colors(1, [Color::Red, Color::Red])
        || definition.colors != BTreeSet::from([Color::Red])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.power != Some(2)
        || definition.toughness != Some(1)
        || definition.keywords != [Keyword::CannotBlockUnlessControlsMountain]
        || !definition.effects.is_empty()
        || definition.supported_rules
            != [
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "mountain-required-to-block",
            ]
    {
        return Err(
            "Sabertooth Alley Cat typed definition differs from its feature contract".to_owned(),
        );
    }
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Sabertooth Alley Cat missing from the RAV registry")?;
    if registered != definition {
        return Err(
            "Sabertooth Alley Cat registry is not delegated to its candidate module".to_owned(),
        );
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoRestriction,
    WrongCost,
    WrongPower,
}

#[derive(Clone, Debug)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    terminal: String,
    turns: u32,
    accepted: u32,
    rejected: u32,
    cat: ObjectId,
    cat_payment: Vec<Color>,
    cat_cast_without_mountain: bool,
    failed_block_atomic: bool,
    failed_block_position: Option<usize>,
    legal_block_with_mountain: bool,
    cat_zone: Option<Zone>,
    cat_blocked_attacker: Option<ObjectId>,
    cat_blocked_attacker_zone: Option<Zone>,
}

fn candidate_value(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_sabertooth_alley_cat::definition();
    match mutation {
        Mutation::None => {}
        Mutation::NoRestriction => definition.keywords.clear(),
        Mutation::WrongCost => definition.mana_cost = ManaCost::with_colors(1, [Color::Red]),
        Mutation::WrongPower => definition.power = Some(0),
    }
    definition
}

fn find_trace() -> Result<(u64, Captured), String> {
    let captured = run_match(REFERENCE_SEED, Mutation::None)?;
    validate_trace(&captured, REFERENCE_SEED)?;
    Ok((REFERENCE_SEED, captured))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation) {
        Err(_) => true,
        Ok(captured) => validate_trace(&captured, seed).is_err(),
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot = deck(&[
        (CARD_ID, 4),
        (BIRD_ID, 4),
        (SIGNET_ID, 4),
        (WATCHWOLF_ID, 4),
        ("RAV-PLAINS", 18),
        ("RAV-FOREST", 18),
        ("RAV-MOUNTAIN", 8),
    ]);
    let opponent = deck(&[(ATTACKER_ID, 4), ("RAV-MOUNTAIN", 56)]);
    if pilot
        .mainboard
        .iter()
        .map(|entry| usize::from(entry.count))
        .sum::<usize>()
        != 60
        || opponent
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
    {
        return Err("Sabertooth Alley Cat fixtures must use shuffled 60-card decks".to_owned());
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 60,
        max_policy_moves: 2_000,
        ..DeckMatchConfig::default()
    };
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Sabertooth Alley Cat missing from registry")? = candidate_value(mutation);
    let mut game = Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut p0 = AlleyCatPolicy::new(PlayerId(0));
    let mut p1 = AlleyCatPolicy::new(PlayerId(1));
    let mut cat = None;
    let mut cat_payment = Vec::new();
    let mut cat_cast_without_mountain = false;
    let mut failed_block_atomic = false;
    let mut failed_block_position = None;
    let mut legal_block_with_mountain = false;
    let mut cat_blocked_attacker = None;
    let mut accepted = 0u32;
    let mut attempted = 0u32;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            let view = game.view_for_player(PlayerId(0)).ok();
            return Err(format!(
                "Sabertooth Alley Cat match exceeded bounded game limits (turn={}, p0 signet_cast={} cat_submitted={} p0_hand={:?} p0_board={:?})",
                game.turn,
                p0.signet_cast,
                p0.cat_spell_submitted,
                view.as_ref().map(|view| view
                    .hand
                    .iter()
                    .map(|card| card.definition)
                    .collect::<Vec<_>>()),
                view.as_ref().map(|view| view
                    .own_battlefield
                    .iter()
                    .map(|card| (card.definition, card.tapped, card.summoning_sick))
                    .collect::<Vec<_>>())
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        if player == PlayerId(0)
            && !failed_block_atomic
            && view.step == Step::DeclareBlockers
            && view.active_player != player
            && !view.combat_attackers.is_empty()
        {
            if let Some(cat_view) = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            {
                let owns_mountain = view
                    .own_battlefield
                    .iter()
                    .any(|card| card.definition == Some("RAV-MOUNTAIN"));
                if !owns_mountain {
                    if let Some(attacker) = view
                        .combat_attackers
                        .iter()
                        .find(|card| !card.keywords.contains(&Keyword::Mountainwalk))
                    {
                        let before = game.event_log.clone();
                        let result = game.declare_blockers(
                            PlayerId(0),
                            &[CombatBlock {
                                attacker: attacker.id,
                                blocker: cat_view.id,
                            }],
                        );
                        if result.is_ok() || game.event_log != before {
                            return Err("Sabertooth Alley Cat block without a Mountain was not rejected atomically".to_owned());
                        }
                        failed_block_atomic = true;
                        failed_block_position = Some(game.event_log.len());
                        p0.offline_probe_done = true;
                    }
                }
            }
        }
        let action = if player == PlayerId(0) {
            policy_action(&mut p0, &view)
        } else {
            policy_action(&mut p1, &view)
        };
        attempted += 1;
        game.submit_policy_move(
            player,
            if player == PlayerId(0) {
                p0.id()
            } else {
                p1.id()
            },
            action,
        )
        .map_err(|error| {
            format!(
                "policy move rejected at turn {} {:?}: {error}",
                view.turn, view.step
            )
        })?;
        accepted += 1;
        if let Some(card) = p0.cat {
            if !cat_cast_without_mountain && game.event_log.iter().any(|event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card: spell } if *spell == card)) {
                cat_cast_without_mountain = !view.own_battlefield.iter().any(|permanent| permanent.definition == Some("RAV-MOUNTAIN"));
                cat = Some(card);
            }
        }
        if let Some(card) = cat {
            if cat_payment.is_empty() {
                cat_payment = game
                    .event_log
                    .iter()
                    .find_map(|event| match event {
                        GameEvent::SpellManaPaid {
                            player: PlayerId(0),
                            card: spell,
                            colors,
                        } if *spell == card => Some(colors.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
            }
        }
        if failed_block_atomic && !legal_block_with_mountain {
            let mountain_exists = view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some("RAV-MOUNTAIN"));
            if mountain_exists
                && game
                    .event_log
                    .iter()
                    .find_map(|event| match event {
                        GameEvent::BlockersDeclared {
                            player: PlayerId(0),
                            assignments,
                        } => cat.and_then(|cat| {
                            assignments
                                .iter()
                                .find(|(_, blocker)| *blocker == cat)
                                .map(|(attacker, _)| *attacker)
                        }),
                        _ => None,
                    })
                    .is_some_and(|attacker| {
                        cat_blocked_attacker = Some(attacker);
                        true
                    })
            {
                legal_block_with_mountain = true;
            }
        }
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
    }
    let cat = cat.ok_or_else(|| {
        let view = game.view_for_player(PlayerId(0)).ok();
        format!(
            "Sabertooth Alley Cat was not cast (turn={}, signet_cast={}, cat_submitted={}, hand={:?}, battlefield={:?})",
            game.turn,
            p0.signet_cast,
            p0.cat_spell_submitted,
            view.as_ref().map(|view| view.hand.iter().map(|card| card.definition).collect::<Vec<_>>()),
            view.as_ref().map(|view| view.own_battlefield.iter().map(|card| (card.definition, card.tapped)).collect::<Vec<_>>())
        )
    })?;
    let winner = game.winner().map(|seat| seat.0);
    Ok(Captured {
        events: game.event_log.clone(),
        winner,
        terminal: winner.map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({seat}))"),
        ),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        cat,
        cat_payment,
        cat_cast_without_mountain,
        failed_block_atomic,
        failed_block_position,
        legal_block_with_mountain,
        cat_zone: game.zone_of(cat),
        cat_blocked_attacker_zone: cat_blocked_attacker.and_then(|attacker| {
            game.event_log
                .iter()
                .any(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == attacker))
                .then_some(Zone::Graveyard)
        }),
        cat_blocked_attacker,
    })
}

fn policy_action(policy: &mut AlleyCatPolicy, view: &GameView) -> PolicyAction {
    if let Some(action) = policy.propose_pending_decision(view) {
        action
    } else if view.optional_triggered_ability_choice.is_some() {
        policy.propose_optional_triggered_ability(view)
    } else if view.draw_replacement_pending {
        policy.propose_draw_replacement(view)
    } else if view.private_library_choice.is_some() {
        policy.propose_private_library_choice(view)
    } else if view.private_opponent_library_choice.is_some() {
        policy.propose_private_opponent_library_choice(view)
    } else if view.library_search_choice.is_some() {
        policy.propose_library_search_choice(view)
    } else {
        policy.propose_move(view)
    }
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

struct AlleyCatPolicy {
    player: PlayerId,
    cat: Option<ObjectId>,
    offline_probe_done: bool,
    signet_cast: bool,
    cat_spell_submitted: bool,
    watchwolf_casts: u8,
}
impl AlleyCatPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            cat: None,
            offline_probe_done: false,
            signet_cast: false,
            cat_spell_submitted: false,
            watchwolf_casts: 0,
        }
    }
}

impl CodePolicy for AlleyCatPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.sabertooth-mountain-blocker.v1"
        } else {
            "rav.sabertooth-opponent.v1"
        }
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = if self.player == PlayerId(1) {
                if view
                    .opponent_battlefield
                    .iter()
                    .any(|card| card.definition == Some(CARD_ID))
                {
                    view.own_battlefield
                        .iter()
                        .filter(|card| {
                            (card.definition == Some(ATTACKER_ID)
                                || card.definition == Some(WATCHWOLF_ID))
                                && card.can_attack
                        })
                        .map(|card| card.id)
                        .collect()
                } else {
                    Vec::new()
                }
            } else if self.cat_spell_submitted {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.can_attack && card.definition != Some(CARD_ID))
                    .map(|card| card.id)
                    .collect()
            } else {
                Vec::new()
            };
            return PolicyAction::DeclareAttackers { attackers };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            let has_mountain = view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some("RAV-MOUNTAIN"));
            if self.player == PlayerId(0) && self.offline_probe_done && !has_mountain {
                return PolicyAction::DeclareBlockers {
                    assignments: vec![],
                };
            }
            let mut used = BTreeSet::new();
            let assignments = view
                .combat_attackers
                .iter()
                .filter(|attacker| !attacker.keywords.contains(&Keyword::Mountainwalk))
                .filter_map(|attacker| {
                    let flying_attacker = attacker.keywords.contains(&Keyword::Flying);
                    let blocker = view
                        .own_battlefield
                        .iter()
                        .find(|blocker| {
                            blocker.can_block
                                && (!flying_attacker
                                    || blocker.keywords.contains(&Keyword::Flying)
                                    || blocker.keywords.contains(&Keyword::Reach))
                                && !used.contains(&blocker.id)
                                && blocker.definition == Some(CARD_ID)
                                && has_mountain
                        })
                        .or_else(|| {
                            view.own_battlefield.iter().find(|blocker| {
                                blocker.can_block
                                    && (!flying_attacker
                                        || blocker.keywords.contains(&Keyword::Flying)
                                        || blocker.keywords.contains(&Keyword::Reach))
                                    && !used.contains(&blocker.id)
                                    && blocker.definition != Some(CARD_ID)
                            })
                        })?;
                    used.insert(blocker.id);
                    Some(CombatBlock {
                        attacker: attacker.id,
                        blocker: blocker.id,
                    })
                })
                .collect();
            return PolicyAction::DeclareBlockers { assignments };
        }
        if view.decision_player != self.player
            || view.priority != self.player
            || view.active_player != self.player
            || view.stack_depth != 0
            || view.step != Step::PrecombatMain
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            let land = if self.player == PlayerId(0) && !self.offline_probe_done {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some("RAV-FOREST"))
                    .or_else(|| {
                        view.hand
                            .iter()
                            .find(|card| card.definition == Some("RAV-PLAINS"))
                    })
            } else if self.player == PlayerId(0)
                && self.offline_probe_done
                && !view
                    .own_battlefield
                    .iter()
                    .any(|card| card.definition == Some("RAV-MOUNTAIN"))
            {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some("RAV-MOUNTAIN"))
                    .or_else(|| {
                        view.hand
                            .iter()
                            .find(|card| card.definition == Some("RAV-FOREST"))
                    })
                    .or_else(|| {
                        view.hand
                            .iter()
                            .find(|card| card.definition == Some("RAV-PLAINS"))
                    })
            } else {
                view.hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            };
            if let Some(land) = land {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if self.player == PlayerId(1) {
            let goblin = view
                .hand
                .iter()
                .find(|card| card.definition == Some(ATTACKER_ID))
                .cloned();
            let watchwolf = view
                .hand
                .iter()
                .find(|card| card.definition == Some(WATCHWOLF_ID))
                .cloned();
            if let Some(card) = goblin
                .as_ref()
                .filter(|_| view.mana_pool.amount(Color::Red) > 0)
            {
                return PolicyAction::Cast(CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                });
            }
            if let Some(card) = watchwolf.as_ref().filter(|_| {
                view.mana_pool.amount(Color::Green) > 0 && view.mana_pool.amount(Color::White) > 0
            }) {
                return PolicyAction::Cast(CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                });
            }
            if goblin.is_some() && view.mana_pool.amount(Color::Red) == 0 {
                if let Some(land) = view
                    .own_battlefield
                    .iter()
                    .find(|land| land.definition == Some("RAV-MOUNTAIN") && !land.tapped)
                {
                    return PolicyAction::ActivateManaAbility {
                        land: land.id,
                        color: Color::Red,
                    };
                }
            }
            if watchwolf.is_some() {
                if let Some(color) = [Color::Green, Color::White]
                    .into_iter()
                    .find(|color| view.mana_pool.amount(*color) == 0)
                {
                    let land_id = if color == Color::Green {
                        "RAV-FOREST"
                    } else {
                        "RAV-PLAINS"
                    };
                    if let Some(land) = view
                        .own_battlefield
                        .iter()
                        .find(|land| land.definition == Some(land_id) && !land.tapped)
                    {
                        return PolicyAction::ActivateManaAbility {
                            land: land.id,
                            color,
                        };
                    }
                }
            }
            return PolicyAction::PassPriority;
        }
        let bird_count = view
            .own_battlefield
            .iter()
            .filter(|card| card.definition == Some(BIRD_ID))
            .count();
        if bird_count < 2 {
            if let Some(bird) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(BIRD_ID))
            {
                if view.mana_pool.amount(Color::Green) > 0 {
                    return PolicyAction::Cast(CastRequest {
                        card: bird.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    });
                }
                if let Some(forest) = view
                    .own_battlefield
                    .iter()
                    .find(|land| land.definition == Some("RAV-FOREST") && !land.tapped)
                {
                    return PolicyAction::ActivateManaAbility {
                        land: forest.id,
                        color: Color::Green,
                    };
                }
            }
        }
        if !self.signet_cast {
            if let Some(signet) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(SIGNET_ID))
            {
                if view.mana_pool.total() >= 2 {
                    self.signet_cast = true;
                    return PolicyAction::Cast(CastRequest {
                        card: signet.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    });
                }
                if let Some(land) = view
                    .own_battlefield
                    .iter()
                    .find(|land| !land.tapped && land.card_types.contains(&CardType::Land))
                {
                    return PolicyAction::ActivateManaAbility {
                        land: land.id,
                        color: if land.definition == Some("RAV-FOREST") {
                            Color::Green
                        } else {
                            Color::White
                        },
                    };
                }
            }
        }
        if !self.cat_spell_submitted {
            let cat_hand = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID));
            let signet = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(SIGNET_ID) && !card.tapped);
            let bird = view.own_battlefield.iter().find(|card| {
                card.definition == Some(BIRD_ID) && !card.tapped && !card.summoning_sick
            });
            if let Some(cat) = cat_hand {
                if view.mana_pool.amount(Color::Red) >= 2
                    && view.mana_pool.amount(Color::White) >= 1
                {
                    self.cat_spell_submitted = true;
                    self.cat = Some(cat.id);
                    return PolicyAction::CastWithPayment {
                        request: CastRequest {
                            card: cat.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: ManaPaymentSelection {
                            generic: vec![Color::White],
                            hybrid: vec![],
                        },
                    };
                }
                if let Some(land) = view
                    .own_battlefield
                    .iter()
                    .find(|land| !land.tapped && land.card_types.contains(&CardType::Land))
                {
                    if signet.is_some() && view.mana_pool.total() == 0 {
                        return PolicyAction::ActivateManaAbility {
                            land: land.id,
                            color: if land.definition == Some("RAV-FOREST") {
                                Color::Green
                            } else {
                                Color::White
                            },
                        };
                    }
                }
                if let Some(signet) = signet {
                    if view.mana_pool.total() == 0 {
                        if let Some(land) = view
                            .own_battlefield
                            .iter()
                            .find(|land| !land.tapped && land.card_types.contains(&CardType::Land))
                        {
                            return PolicyAction::ActivateManaAbility {
                                land: land.id,
                                color: if land.definition == Some("RAV-FOREST") {
                                    Color::Green
                                } else {
                                    Color::White
                                },
                            };
                        }
                    } else {
                        return PolicyAction::ActivateBoundManaAbility {
                            activation: ManaAbilityActivation {
                                source: signet.id,
                                ability_id: "boros-signet-wr",
                                chosen_color: None,
                            },
                        };
                    }
                }
                if let Some(bird) = bird {
                    if view.mana_pool.amount(Color::Red) < 2 {
                        return PolicyAction::ActivateBoundManaAbility {
                            activation: ManaAbilityActivation {
                                source: bird.id,
                                ability_id: "produce-one-color",
                                chosen_color: Some(Color::Red),
                            },
                        };
                    }
                }
                if view.mana_pool.amount(Color::Red) >= 2
                    && view.mana_pool.amount(Color::White) == 0
                {
                    if let Some(land) = view
                        .own_battlefield
                        .iter()
                        .find(|land| land.definition == Some("RAV-PLAINS") && !land.tapped)
                    {
                        return PolicyAction::ActivateManaAbility {
                            land: land.id,
                            color: Color::White,
                        };
                    }
                }
            }
        }
        if self.cat_spell_submitted
            || !view
                .hand
                .iter()
                .any(|card| card.definition == Some(CARD_ID))
        {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(WATCHWOLF_ID))
            {
                if view.mana_pool.amount(Color::Green) > 0
                    && view.mana_pool.amount(Color::White) > 0
                {
                    self.watchwolf_casts = self.watchwolf_casts.saturating_add(1);
                    return PolicyAction::CastWithPayment {
                        request: CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: ManaPaymentSelection::default(),
                    };
                }
                if let Some(color) = [Color::Green, Color::White]
                    .into_iter()
                    .find(|color| view.mana_pool.amount(*color) == 0)
                {
                    if let Some(land) = view.own_battlefield.iter().find(|land| {
                        !land.tapped
                            && land.definition
                                == Some(if color == Color::Green {
                                    "RAV-FOREST"
                                } else {
                                    "RAV-PLAINS"
                                })
                    }) {
                        return PolicyAction::ActivateManaAbility {
                            land: land.id,
                            color,
                        };
                    }
                    if let Some(bird) = view.own_battlefield.iter().find(|bird| {
                        bird.definition == Some(BIRD_ID) && !bird.tapped && !bird.summoning_sick
                    }) {
                        return PolicyAction::ActivateBoundManaAbility {
                            activation: ManaAbilityActivation {
                                source: bird.id,
                                ability_id: "produce-one-color",
                                chosen_color: Some(color),
                            },
                        };
                    }
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.winner != Some(0) || captured.terminal != "Winner(PlayerId(0))" {
        return Err(format!(
            "Sabertooth Alley Cat game terminal was {}",
            captured.terminal
        ));
    }
    if captured.rejected != 0 {
        return Err("reference match has rejected policy moves".to_owned());
    }
    if !captured.cat_cast_without_mountain {
        return Err(
            "Sabertooth Alley Cat was not cast while its controller had no Mountain".to_owned(),
        );
    }
    if !captured.failed_block_atomic {
        return Err(
            "blocking without a Mountain was not rejected with no event-log changes".to_owned(),
        );
    }
    if !captured.legal_block_with_mountain {
        return Err(
            "Sabertooth Alley Cat did not block legally after its controller played a Mountain"
                .to_owned(),
        );
    }
    if captured.cat_zone != Some(Zone::Graveyard) {
        return Err(
            "Sabertooth Alley Cat did not complete its verified combat zone transition".to_owned(),
        );
    }
    if captured.cat_blocked_attacker.is_none()
        || captured.cat_blocked_attacker_zone != Some(Zone::Graveyard)
    {
        return Err(
            "the Sabertooth Alley Cat block did not deal lethal damage to its 1/1 attacker"
                .to_owned(),
        );
    }
    if captured.cat_payment.len() != 3
        || captured
            .cat_payment
            .iter()
            .filter(|color| **color == Color::Red)
            .count()
            != 2
        || captured
            .cat_payment
            .iter()
            .filter(|color| **color == Color::White)
            .count()
            != 1
    {
        return Err(format!(
            "Sabertooth Alley Cat payment did not include its exact {{1}}{{R}}{{R}} symbols: {:?}",
            captured.cat_payment
        ));
    }
    let cast = unique_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == captured.cat)).ok_or("Sabertooth Alley Cat cast receipt missing")?;
    let paid = unique_index(&captured.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == captured.cat && colors == &captured.cat_payment)).ok_or("Sabertooth Alley Cat exact payment receipt missing")?;
    let entered = unique_index(&captured.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == captured.cat)).ok_or("Sabertooth Alley Cat did not enter the battlefield")?;
    let failed = captured
        .failed_block_position
        .ok_or("atomic no-Mountain block probe position missing")?;
    let legal = unique_index(&captured.events, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(0), assignments } if assignments.iter().any(|(_, blocker)| *blocker == captured.cat))).ok_or("legal Mountain-enabled Cat block receipt missing")?;
    let moved = unique_index(&captured.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == captured.cat)).ok_or("Sabertooth Alley Cat did not move to its owner's graveyard")?;
    let attacker_moved = unique_index(&captured.events, |event| {
        matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if Some(*card) == captured.cat_blocked_attacker)
    })
    .ok_or("Cat-blocked attacker did not move to its owner's graveyard")?;
    let next_pilot_attack = captured
        .events
        .iter()
        .enumerate()
        .find_map(|(index, event)| {
            (index > legal
                && matches!(
                    event,
                    GameEvent::AttackersDeclared {
                        player: PlayerId(0),
                        ..
                    }
                ))
            .then_some(index)
        })
        .ok_or("pilot did not continue combat after the Cat block")?;
    let ended = unique_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal P0 win event missing")?;
    if !(paid < cast
        && cast < entered
        && entered < failed
        && failed <= legal
        && legal < moved
        && legal < attacker_moved
        && attacker_moved < next_pilot_attack
        && moved < ended)
    {
        return Err("Sabertooth cast, rejection, later legal block, combat move, and terminal ordering differs".to_owned());
    }
    let evidence: Vec<GameEvent> = captured
        .events
        .iter()
        .filter(|event| match event {
            GameEvent::SpellManaPaid { card, .. } | GameEvent::SpellCast { card, .. } => {
                *card == captured.cat
            }
            GameEvent::CardMoved { card, to } => {
                (*card == captured.cat && (*to == Zone::Battlefield || *to == Zone::Graveyard))
                    || (Some(*card) == captured.cat_blocked_attacker && *to == Zone::Graveyard)
            }
            GameEvent::BlockersDeclared {
                player: PlayerId(0),
                assignments,
            } => assignments
                .iter()
                .any(|(_, blocker)| *blocker == captured.cat),
            GameEvent::GameEnded {
                winner: Some(PlayerId(0)),
            } => true,
            _ => false,
        })
        .cloned()
        .collect();
    Ok(
        json!({"trace_id":"rav-sabertooth-alley-cat-mountain-block-condition-pvp","shuffle_seed":seed,"terminal":captured.terminal,"turns":captured.turns,"accepted_policy_moves":captured.accepted,"rejected_policy_moves":captured.rejected,"full_game_event_count":captured.events.len(),"mechanics":{"cast_from_shuffled_library":true,"spell":captured.cat.0,"mana_cost":"{1}{R}{R}","paid_colors":captured.cat_payment.iter().map(|color| format!("{:?}",color)).collect::<Vec<_>>(),"cat_cast_while_controller_lacked_mountain":captured.cat_cast_without_mountain,"no_mountain_block_attempt_rejected_atomically":captured.failed_block_atomic,"mountain_enabled_legal_block":captured.legal_block_with_mountain,"cat_final_zone":format!("{:?}",captured.cat_zone),"blocked_attacker":captured.cat_blocked_attacker.map(|id|id.0),"blocked_attacker_final_zone":format!("{:?}",captured.cat_blocked_attacker_zone),"controller_won":true},"events":project_events(&evidence)}),
    )
}

fn unique_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    let mut indexes = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let first = indexes.next()?;
    indexes.next().is_none().then_some(first)
}
fn card_status(card: &CatalogCard) -> Value {
    let id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 140;
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,"trace_status":if passed{"passed"}else if id.is_some(){"not_requested"}else{"catalog_only"},"credited":false})
}
