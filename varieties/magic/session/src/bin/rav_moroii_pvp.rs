//! Full-game diagnostic authority for RAV Moroii.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView,
    Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step,
    TriggerCondition, TriggeredAbilityBinding, Zone,
};
use cardbench_magic_policies::CodePolicy;
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
const CARD_ID: &str = "RAV-MOROII";
const CARD_NAME: &str = "Moroii";
const OPPONENT_CREATURE_ID: &str = "RAV-WAR-TORCH-GOBLIN";
const TRIGGER_ID: &str = "upkeep-lose-one-life";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/moroii.rs";
const REFERENCE_SEED: u64 = 37;
const MAX_TURNS: u32 = 40;
const MAX_MOVES: u32 = 4000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/moroii.rs"]
mod candidate_moroii;

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
            eprintln!("Moroii PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_definition_and_trigger()?;
    let catalog = unique_catalog()?;
    let seed = REFERENCE_SEED;
    let reference = run_match(seed, Mutation::None)?;
    let trace = validate_trace(&reference, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if reference != replay || project_events(&reference.events) != project_events(&replay.events) {
        return Err("Moroii full-game event log was not deterministic on replay".to_owned());
    }
    let mutations = json!({
        "printed_cost_changed_detected": mutation_detected(seed, Mutation::WrongCost)?,
        "base_power_or_toughness_changed_detected": mutation_detected(seed, Mutation::WrongStats)?,
        "flying_removed_detected": mutation_detected(seed, Mutation::NoFlying)?,
        "upkeep_trigger_removed_detected": mutation_detected(seed, Mutation::NoTrigger)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "printed_cost_changed_detected",
        "base_power_or_toughness_changed_detected",
        "flying_removed_detected",
        "upkeep_trigger_removed_detected",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Moroii gameplay mutation escaped the oracle: {key}"
            ));
        }
    }
    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,
        "expansion":"RAV",
        "card":{"collector_number":216,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":["202.1-202.2: colors and mana cost", "302.1 and 601.2: creature characteristics and casting", "508-509: flying attack and blocker legality", "117: upkeep priority", "603.2-603.3: triggered ability occurrence and stacking", "608: resolving triggered abilities", "119.3: life loss", "704: state-based actions"],
            "source_contract":"CardBench-authored semantic summary for RAV Moroii #216: a blue-black 4/4 creature with flying costing {2}{U}{B}; at the beginning of its controller's upkeep, that controller loses 1 life.",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true
        },
        "passed":cards_passed == 1,
        "pilot_card_passed":cards_passed == 1,
        "cards_passed":cards_passed,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
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
    {
        return Err("RAV catalog denominator differs from the task contract".to_owned());
    }
    Ok(unique)
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Moroii source: {error}"))?;
    let file =
        syn::parse_file(&source).map_err(|_| "Moroii source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
    {
        return Err("Moroii source has unsupported module-level attributes".to_owned());
    }
    let functions: Vec<_> = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Fn(function) => Some(function.sig.ident.to_string()),
            syn::Item::Use(_) => None,
            _ => Some("<unexpected item>".to_owned()),
        })
        .collect();
    if functions != ["definition", "triggered_ability"] {
        return Err(
            "Moroii source must contain only its definition and trigger factories".to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_definition_and_trigger() -> Result<(), String> {
    let candidate = candidate_moroii::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Moroii is absent from the RAV registry")?;
    let candidate_trigger = candidate_moroii::triggered_ability();
    let registered_trigger = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Moroii upkeep trigger is absent from the RAV registry")?;
    if candidate != registered
        || candidate.id != CARD_ID
        || candidate.name != CARD_NAME
        || candidate.set_code != "RAV"
        || candidate.mana_cost != ManaCost::with_colors(2, [Color::Blue, Color::Black])
        || candidate.colors != BTreeSet::from([Color::Blue, Color::Black])
        || !candidate.mana_colors.is_empty()
        || candidate.card_types != BTreeSet::from([CardType::Creature])
        || candidate.is_basic_land
        || candidate.power != Some(4)
        || candidate.toughness != Some(4)
        || candidate.keywords != [Keyword::Flying]
        || !candidate.effects.is_empty()
        || candidate.supported_rules
            != [
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "flying",
                "upkeep-controller-life-loss",
            ]
        || candidate_trigger != registered_trigger
        || candidate_trigger.card_definition != CARD_ID
        || candidate_trigger.ability.id != TRIGGER_ID
        || candidate_trigger.ability.condition != TriggerCondition::BeginningOfUpkeep
        || candidate_trigger.ability.mana_cost != ManaCost::new(0)
        || candidate_trigger.ability.optional
        || !candidate_trigger.ability.targets.is_empty()
        || candidate_trigger.ability.effects != [Effect::LoseLifeController { amount: 1 }]
    {
        return Err(
            "Moroii candidate or registry differs from its RAV #216 feature contract".to_owned(),
        );
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongCost,
    WrongStats,
    NoFlying,
    NoTrigger,
}

fn definitions_with_mutation(mutation: Mutation) -> Result<Vec<CardDefinition>, String> {
    let mut definitions = card_definitions();
    let candidate = definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Moroii is absent from the RAV registry")?;
    match mutation {
        Mutation::None | Mutation::NoTrigger => {}
        Mutation::WrongCost => {
            candidate.mana_cost = ManaCost::with_colors(3, [Color::Blue, Color::Black]);
        }
        Mutation::WrongStats => {
            candidate.power = Some(3);
            candidate.toughness = Some(4);
        }
        Mutation::NoFlying => candidate.keywords.clear(),
    }
    Ok(definitions)
}

fn triggers_with_mutation(mutation: Mutation) -> Vec<TriggeredAbilityBinding> {
    let mut bindings = rav_triggered_ability_bindings();
    if mutation == Mutation::NoTrigger {
        bindings.retain(|binding| binding.card_definition != CARD_ID);
    }
    bindings
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    moroii: Option<ObjectId>,
    payment: Vec<Color>,
    final_zone: Option<Zone>,
    power: Option<i32>,
    toughness: Option<i32>,
    keywords: Vec<Keyword>,
    life_totals: [i64; 2],
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Err(_) => Ok(true),
        Ok(captured) => Ok(validate_trace(&captured, seed).is_err()),
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-SWAMP".to_owned(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-ISLAND".to_owned(),
                count: 28,
            },
        ],
        sideboard: vec![],
    };
    let opponent_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: OPPONENT_CREATURE_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    for (label, deck) in [
        ("Moroii pilot", &pilot_deck),
        ("Goblin opponent", &opponent_deck),
    ] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{label} deck has {count} cards; expected 60"));
        }
    }
    let mut game = Game::new_with_all_bindings_and_triggers(
        definitions_with_mutation(mutation)?,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers_with_mutation(mutation),
    )
    .map_err(|error| error.to_string())?;
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
        Box::new(MoroiiPolicy {
            player: PlayerId(0),
            cast_once: false,
        }),
        Box::new(GoblinPolicy {
            player: PlayerId(1),
            cast_once: false,
        }),
    ];
    let mut captured = Captured {
        events: vec![],
        terminal: String::new(),
        winner: None,
        turns: 0,
        accepted_moves: 0,
        rejected_moves: 0,
        moroii: None,
        payment: vec![],
        final_zone: None,
        power: None,
        toughness: None,
        keywords: vec![],
        life_totals: [0, 0],
    };
    let mut attempts = 0u32;
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || captured.accepted_moves >= MAX_MOVES {
            return Err(format!(
                "Moroii duel exceeded its bound at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("policy seat is missing")?;
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
        let cursor = game.event_log.len();
        attempts += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "policy move rejected on turn {} at {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        captured.accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant check failed: {error}"))?;
        for event in &game.event_log[cursor..] {
            if let GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } = event
                && game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID)
            {
                captured.moroii = Some(*card);
                captured.payment = colors.clone();
            }
            if let GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } = event
                && game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID)
            {
                captured.moroii = Some(*card);
            }
        }
    }
    captured.events = game.event_log.clone();
    captured.terminal = game.winner().map_or_else(
        || "Draw".to_owned(),
        |winner| format!("Winner(PlayerId({}))", winner.0),
    );
    captured.winner = game.winner().map(|winner| winner.0);
    captured.turns = game.turn;
    captured.rejected_moves = attempts.saturating_sub(captured.accepted_moves);
    captured.final_zone = captured.moroii.and_then(|card| game.zone_of(card));
    if let Some(moroii) = captured.moroii {
        let characteristics = game
            .characteristics(moroii)
            .map_err(|error| error.to_string())?;
        captured.power = characteristics.power;
        captured.toughness = characteristics.toughness;
        captured.keywords = characteristics.keywords;
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

struct MoroiiPolicy {
    player: PlayerId,
    cast_once: bool,
}

impl CodePolicy for MoroiiPolicy {
    fn id(&self) -> &'static str {
        "rav.moroii-flying-upkeep-pilot.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && view.player == self.player
            && !view.attackers_declared
        {
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
            && view.player == self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.player != self.player
            || view.decision_player != self.player
            || view.priority != self.player
        {
            return PolicyAction::PassPriority;
        }
        if view.active_player == self.player
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
            && view.stack_depth == 0
        {
            if view.lands_played == 0
                && let Some(land) = preferred_land(view)
            {
                return PolicyAction::PlayLand { card: land.id };
            }
            if !self.cast_once
                && let Some(moroii) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
            {
                let black = view.mana_pool.amount(Color::Black);
                let blue = view.mana_pool.amount(Color::Blue);
                let generic = if black >= 3 && blue >= 1 {
                    Some(vec![Color::Black, Color::Black])
                } else if black >= 2 && blue >= 2 {
                    Some(vec![Color::Black, Color::Blue])
                } else if black >= 1 && blue >= 3 {
                    Some(vec![Color::Blue, Color::Blue])
                } else {
                    None
                };
                if let Some(generic) = generic {
                    self.cast_once = true;
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: moroii.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: ManaPaymentSelection {
                            generic,
                            hybrid: vec![],
                        },
                    };
                }
                if let Some(color) = next_mana_color(black, blue) {
                    if let Some(land) = view.own_battlefield.iter().find(|card| {
                        card.card_types.contains(&CardType::Land)
                            && !card.tapped
                            && card.mana_colors.contains(&color)
                    }) {
                        return PolicyAction::ActivateManaAbility {
                            land: land.id,
                            color,
                        };
                    }
                }
            }
        }
        PolicyAction::PassPriority
    }
}

struct GoblinPolicy {
    player: PlayerId,
    cast_once: bool,
}

impl CodePolicy for GoblinPolicy {
    fn id(&self) -> &'static str {
        "rav.war-torch-goblin-ground-blocker.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && view.player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(OPPONENT_CREATURE_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect(),
            };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && view.player == self.player
            && !view.blockers_declared
        {
            let attacker = view
                .combat_attackers
                .iter()
                .find(|card| card.definition == Some(CARD_ID));
            let goblin = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(OPPONENT_CREATURE_ID) && card.can_block);
            if let (Some(attacker), Some(goblin)) = (attacker, goblin)
                && !attacker.keywords.contains(&Keyword::Flying)
            {
                return PolicyAction::DeclareBlockers {
                    assignments: vec![cardbench_magic_engine::CombatBlock {
                        attacker: attacker.id,
                        blocker: goblin.id,
                    }],
                };
            }
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.player != self.player
            || view.decision_player != self.player
            || view.priority != self.player
        {
            return PolicyAction::PassPriority;
        }
        if view.active_player == self.player
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
            && view.stack_depth == 0
        {
            if view.lands_played == 0
                && let Some(land) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some("RAV-MOUNTAIN"))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
            if !self.cast_once
                && let Some(goblin) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(OPPONENT_CREATURE_ID))
            {
                if view.mana_pool.amount(Color::Red) > 0 {
                    self.cast_once = true;
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: goblin.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: ManaPaymentSelection::default(),
                    };
                }
                if let Some(mountain) = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.definition == Some("RAV-MOUNTAIN") && !card.tapped)
                {
                    return PolicyAction::ActivateManaAbility {
                        land: mountain.id,
                        color: Color::Red,
                    };
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn preferred_land(view: &GameView) -> Option<&cardbench_magic_engine::CardView> {
    let swamp_count = view
        .own_battlefield
        .iter()
        .filter(|card| card.definition == Some("RAV-SWAMP"))
        .count();
    let island_count = view
        .own_battlefield
        .iter()
        .filter(|card| card.definition == Some("RAV-ISLAND"))
        .count();
    let preferred = if swamp_count <= island_count {
        "RAV-SWAMP"
    } else {
        "RAV-ISLAND"
    };
    view.hand
        .iter()
        .find(|card| card.definition == Some(preferred))
        .or_else(|| {
            view.hand
                .iter()
                .find(|card| matches!(card.definition, Some("RAV-SWAMP" | "RAV-ISLAND")))
        })
}

fn next_mana_color(black: u8, blue: u8) -> Option<Color> {
    let total = usize::from(black) + usize::from(blue);
    if black == 0 {
        Some(Color::Black)
    } else if blue == 0 {
        Some(Color::Blue)
    } else if total < 4 {
        Some(if black <= blue {
            Color::Black
        } else {
            Color::Blue
        })
    } else if black < 2 {
        Some(Color::Black)
    } else if blue < 2 {
        Some(Color::Blue)
    } else {
        None
    }
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))" || captured.winner != Some(0) {
        return Err(format!(
            "Moroii duel did not reach the pilot's terminal win: {}",
            captured.terminal
        ));
    }
    if captured.rejected_moves != 0 || captured.life_totals[0] <= 0 || captured.life_totals[1] > 0 {
        return Err(format!(
            "Moroii duel ended with invalid life/move state: life={:?}",
            captured.life_totals
        ));
    }
    let moroii = captured.moroii.ok_or("Moroii was not cast")?;
    if captured.payment.len() != 4
        || !captured.payment.contains(&Color::Black)
        || !captured.payment.contains(&Color::Blue)
    {
        return Err(format!(
            "Moroii did not pay exact {{2}}{{U}}{{B}} cost: {:?}",
            captured.payment
        ));
    }
    if captured.final_zone != Some(Zone::Battlefield)
        || captured.power != Some(4)
        || captured.toughness != Some(4)
        || captured.keywords != [Keyword::Flying]
    {
        return Err(format!(
            "Moroii live characteristics differ from 4/4 Flying: zone={:?} P/T={:?}/{:?} keywords={:?}",
            captured.final_zone, captured.power, captured.toughness, captured.keywords
        ));
    }
    let paid = event_index(&captured.events, |event| {
        matches!(event,
            GameEvent::SpellManaPaid { player: PlayerId(0), card, colors }
            if *card == moroii && colors.len() == 4
                && colors.contains(&Color::Black)
                && colors.contains(&Color::Blue)
        )
    })
    .ok_or("exact four-mana {2}{U}{B} payment receipt is missing")?;
    let cast = event_index_after(&captured.events, paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == moroii)).ok_or("Moroii cast receipt is missing")?;
    let resolved = event_index_after(
        &captured.events,
        cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == moroii),
    )
    .ok_or("Moroii did not resolve")?;
    let entered = event_index_after(&captured.events, resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == moroii)).ok_or("Moroii did not enter the battlefield")?;
    let opponent_upkeep = event_index_after(&captured.events, entered, |event| {
        matches!(
            event,
            GameEvent::StepBegan {
                active_player: PlayerId(1),
                step: Step::Upkeep,
                ..
            }
        )
    })
    .ok_or("opponent upkeep after Moroii resolved is missing")?;
    let pilot_upkeep = event_index_after(&captured.events, opponent_upkeep, |event| {
        matches!(
            event,
            GameEvent::StepBegan {
                active_player: PlayerId(0),
                step: Step::Upkeep,
                ..
            }
        )
    })
    .ok_or("Moroii controller's upkeep is missing")?;
    if captured.events[opponent_upkeep + 1..pilot_upkeep].iter().any(|event| matches!(event, GameEvent::TriggeredAbilityStacked { source, ability: TRIGGER_ID, .. } if *source == moroii)) {
        return Err("Moroii trigger was incorrectly stacked during the opponent's upkeep".to_owned());
    }
    let stacked = event_index_after(&captured.events, pilot_upkeep, |event| matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, ability: TRIGGER_ID, .. } if *source == moroii)).ok_or("Moroii controller-upkeep trigger provenance is missing")?;
    let source_incarnation = match &captured.events[stacked] {
        GameEvent::TriggeredAbilityStacked {
            source_incarnation, ..
        } => *source_incarnation,
        _ => unreachable!(),
    };
    let loss = event_index_after(&captured.events, stacked, |event| matches!(event, GameEvent::LifeLost { source, player: PlayerId(0), amount: 1 } if *source == moroii)).ok_or("Moroii did not make its controller lose exactly one life")?;
    let ability_resolved = event_index_after(&captured.events, loss, |event| matches!(event, GameEvent::AbilityResolved { source, source_incarnation: incarnation, ability: TRIGGER_ID } if *source == moroii && *incarnation == source_incarnation)).ok_or("Moroii upkeep ability did not resolve from the stacked incarnation")?;
    let attacker = event_index_after(&captured.events, entered, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&moroii))).ok_or("Moroii did not attack")?;
    let blockers = event_index_after(&captured.events, attacker, |event| {
        matches!(
            event,
            GameEvent::BlockersDeclared {
                player: PlayerId(1),
                ..
            }
        )
    })
    .ok_or("opponent blocker declaration for Moroii's first attack is missing")?;
    if !matches!(&captured.events[blockers], GameEvent::BlockersDeclared { assignments, .. } if assignments.is_empty())
    {
        return Err("a ground creature blocked Moroii despite Flying".to_owned());
    }
    let damage = event_index_after(&captured.events, blockers, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 4 } if *source == moroii)).ok_or("flying Moroii did not deal its four combat damage to the opponent")?;
    let terminal = event_index_after(&captured.events, damage, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal pilot victory receipt missing")?;
    let order = [
        paid,
        cast,
        resolved,
        entered,
        opponent_upkeep,
        pilot_upkeep,
        stacked,
        loss,
        ability_resolved,
        attacker,
        blockers,
        damage,
        terminal,
    ];
    if order.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(format!("Moroii event sequence is out of order: {order:?}"));
    }
    let selected: BTreeSet<usize> = order.into_iter().collect();
    let events = project_events(&captured.events)
        .into_iter()
        .filter(|event| selected.contains(&(event.sequence as usize)))
        .collect::<Vec<_>>();
    Ok(json!({
        "trace_id":"rav-moroii-colored-flying-upkeep-life-loss-pvp",
        "seed":seed,
        "decks":["rav-moroii-dimir-flyers-60","rav-war-torch-goblin-opponent-60"],
        "policies":["rav.moroii-flying-upkeep-pilot.v1","rav.war-torch-goblin-ground-blocker.v1"],
        "terminal":captured.terminal,
        "winner_seat":captured.winner,
        "turns":captured.turns,
        "accepted_moves":captured.accepted_moves,
        "rejected_moves":captured.rejected_moves,
        "mechanics":{
            "mana_cost":"{2}{U}{B}",
            "paid_colors":captured.payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),
            "power":captured.power,"toughness":captured.toughness,
            "keywords":captured.keywords.iter().map(|keyword|format!("{keyword:?}")).collect::<Vec<_>>(),
            "moroii":moroii.0,"controller":0,"upkeep_trigger":TRIGGER_ID,
            "opponent_upkeep_triggered":false,"pilot_upkeep_triggered":true,
            "trigger_source_incarnation":source_incarnation,
            "life_loss_amount":1,"life_loss_controller":0,
            "flying_block_rejected_by_empty_blockers":true,"combat_damage_to_opponent":4,
            "controller_won":captured.winner == Some(0)
        },
        "ordered_event_sequence":order,
        "events":events,
        "passed":true
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

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"engine_status":match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" },"trace_status":if passed { "passed" } else { "not_qualified" },"credited":false,"qualification":if passed { "pilot_trace_passed; independent reviewer approval pending" } else { "not_evaluated"}})
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}
