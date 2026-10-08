//! Full-game RAV Concerted Effort authority.

use cardbench_magic_engine::{
    CardType, Color, DeckEntry, DeckList, Game, GameEvent, GameView, Keyword, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, SharedKeywordFamily, Step, Target,
    TriggerCondition, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings, rav_basic_land_type_bindings,
    rav_main_set_catalog, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-CONCERTED-EFFORT";
const CARD_NAME: &str = "Concerted Effort";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/concerted_effort.rs";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const PILOT_DECK: &str = "rav_concerted_effort_keyword_pilot";
const OPPONENT_DECK: &str = "rav_concerted_effort_lands_opponent";
const TRIGGER_ID: &str = "upkeep-share-controller-creature-keywords";
const MAX_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/concerted_effort.rs"]
mod candidate_concerted_effort;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Concerted Effort report: {error}");
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
            eprintln!("Concerted Effort PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_binding()?;
    let catalog = unique_catalog()?;
    let (seed, capture, trace) = find_trace()?;
    let replay = run_match(seed, Mutation::None)?;
    if capture != replay {
        return Err("Concerted Effort full-game replay was not deterministic".to_owned());
    }
    let no_trigger = mutation_detected(seed, Mutation::NoTrigger)?;
    let no_flying = mutation_detected(seed, Mutation::NoFlying)?;
    let wrong_families = mutation_detected(seed, Mutation::WrongFamily)?;
    if !(no_trigger && no_flying && wrong_families) {
        return Err(
            "a Concerted Effort trigger or family mutation escaped the gameplay oracle".to_owned(),
        );
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,
        "expansion":"RAV",
        "card":{"collector_number":8,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "authority_cutoff":"2026-09-24",
            "checked_sections":["117 Priority", "302.6 summoning sickness", "603.2-603.3 upkeep triggered abilities", "608.2 resolving abilities", "611 temporary continuous effects", "613 ability layers", "702 keyword abilities"],
            "source_contract":"RAV #8 is a white {2}{W}{W} enchantment. At each upkeep, every creature its controller controls gains until end of turn each exact Flying, First Strike, Double Strike, Landwalk, Protection, or Trample keyword instance held by another creature that controller controls."
        },
        "passed":true,
        "pilot_card_passed":true,
        "deterministic_replay":true,
        "cards_passed":1,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":"unreviewed_diagnostic",
        "mutation_sensitivity":{
            "upkeep_trigger_removed_mutation_detected_in_game":no_trigger,
            "shared_flying_family_removed_mutation_detected_in_game":no_flying,
            "shared_family_scope_mutation_detected_in_game":wrong_families,
            "sealed_mutation_authority":false
        },
        "traces":[trace],
        "per_card_breakdown":breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoTrigger,
    NoFlying,
    WrongFamily,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Capture {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    seed: u64,
    accepted: u32,
    rejected: u32,
    enchantment: Option<ObjectId>,
    watchwolf: Option<ObjectId>,
    observed_keywords: Vec<String>,
    attached_trample_aura: bool,
    battlefield_definitions: Vec<String>,
    player_life: [i64; 2],
}

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let cards: Vec<_> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if cards.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
    {
        return Err("RAV catalog denominator differs from the card source contract".to_owned());
    }
    Ok(cards)
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read candidate source: {error}"))?;
    if source.len() > 64 * 1024 || syn::parse_file(&source).is_err() {
        return Err("Concerted Effort source must be bounded, valid Rust".to_owned());
    }
    Ok(())
}

fn validate_candidate_binding() -> Result<(), String> {
    let definition = candidate_concerted_effort::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Concerted Effort registry definition missing")?;
    let trigger = candidate_concerted_effort::upkeep_trigger();
    let registered_trigger = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Concerted Effort upkeep trigger missing")?;
    let expected_families = vec![
        SharedKeywordFamily::Flying,
        SharedKeywordFamily::FirstStrike,
        SharedKeywordFamily::DoubleStrike,
        SharedKeywordFamily::Landwalk,
        SharedKeywordFamily::Protection,
        SharedKeywordFamily::Trample,
    ];
    if definition != registered
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(2, [Color::White, Color::White])
        || definition.colors != BTreeSet::from([Color::White])
        || definition.card_types != BTreeSet::from([CardType::Enchantment])
        || trigger != registered_trigger
        || trigger.ability.condition != TriggerCondition::BeginningOfUpkeep
        || trigger.ability.id != TRIGGER_ID
        || trigger.ability.effects
            != vec![
                cardbench_magic_engine::Effect::ShareControllerCreatureKeywordsUntilEndOfTurn {
                    families: expected_families,
                },
            ]
    {
        return Err(
            "Concerted Effort identity, cost, upkeep timing, or six-family binding differs"
                .to_owned(),
        );
    }
    Ok(())
}

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let definition = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Concerted Effort registry definition missing")?;
    *definition = candidate_concerted_effort::definition();
    let mut triggers = rav_triggered_ability_bindings();
    let index = triggers
        .iter()
        .position(|binding| binding.card_definition == CARD_ID)
        .ok_or("Concerted Effort trigger missing from registry")?;
    triggers[index] = candidate_concerted_effort::upkeep_trigger();
    match mutation {
        Mutation::NoTrigger => {
            triggers.remove(index);
        }
        Mutation::NoFlying => {
            if let Some(
                cardbench_magic_engine::Effect::ShareControllerCreatureKeywordsUntilEndOfTurn {
                    families,
                },
            ) = triggers[index].ability.effects.first_mut()
            {
                families.retain(|family| *family != SharedKeywordFamily::Flying);
            }
        }
        Mutation::WrongFamily => {
            if let Some(
                cardbench_magic_engine::Effect::ShareControllerCreatureKeywordsUntilEndOfTurn {
                    families,
                },
            ) = triggers[index].ability.effects.first_mut()
            {
                families.retain(|family| *family != SharedKeywordFamily::Trample);
                families.push(SharedKeywordFamily::Flying);
            }
        }
        Mutation::None => {}
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
    game.register_attachment_bindings(rav_attachment_bindings())
        .map_err(|error| error.to_string())?;
    Ok(game)
}

fn find_trace() -> Result<(u64, Capture, Value), String> {
    if let Ok(value) = std::env::var("CARDBENCH_CONCERTED_EFFORT_SEED") {
        let seed = value.parse::<u64>().map_err(|_| {
            "CARDBENCH_CONCERTED_EFFORT_SEED must be an unsigned integer".to_owned()
        })?;
        let capture = run_match(seed, Mutation::None)?;
        let trace = validate_trace(&capture)?;
        return Ok((seed, capture, trace));
    }
    let mut last_error = "no seeds attempted".to_owned();
    for seed in 0..=MAX_SEED {
        match run_match(seed, Mutation::None)
            .and_then(|capture| validate_trace(&capture).map(|trace| (capture, trace)))
        {
            Ok((capture, trace)) => return Ok((seed, capture, trace)),
            Err(error) => last_error = format!("seed {seed}: {error}"),
        }
    }
    Err(format!(
        "no bounded Concerted Effort full-game trace succeeded: {last_error}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Capture, String> {
    let mut game = new_game(mutation)?;
    let pilot = DeckList {
        mainboard: vec![
            entry(CARD_ID, 4),
            entry(WATCHWOLF_ID, 4),
            entry("RAV-GRAYSCALED-GHARIAL", 4),
            entry("RAV-SCREECHING-GRIFFIN", 4),
            entry("RAV-NIGHTGUARD-PATROL", 4),
            entry("RAV-BOROS-SWIFTBLADE", 4),
            entry("RAV-FISTS-OF-IRONWOOD", 4),
            entry("RAV-PLAINS", 8),
            entry("RAV-FOREST", 8),
            entry("RAV-ISLAND", 8),
            entry("RAV-MOUNTAIN", 8),
        ],
        sideboard: vec![],
    };
    let opponent = DeckList {
        mainboard: vec![entry("RAV-PLAINS", 60)],
        sideboard: vec![],
    };
    if pilot
        .mainboard
        .iter()
        .map(|e| usize::from(e.count))
        .sum::<usize>()
        != 60
        || opponent
            .mainboard
            .iter()
            .map(|e| usize::from(e.count))
            .sum::<usize>()
            != 60
    {
        return Err("constructed decks must each contain exactly sixty cards".to_owned());
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 80,
        max_policy_moves: 12_000,
        ..DeckMatchConfig::default()
    };
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
    let mut policies: [Box<dyn CodePolicy>; 2] = [Box::new(ConcertedPolicy), Box::new(PassPolicy)];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut enchantment = None;
    let mut watchwolf = None;
    let mut observed_keywords = Vec::new();
    let mut attached_trample_aura = false;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Concerted Effort match exceeded its bounds".to_owned());
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
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        for event in &game.event_log[before..] {
            match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    enchantment = Some(*card)
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == WATCHWOLF_ID) =>
                {
                    watchwolf = Some(*card)
                }
                GameEvent::AbilityResolved {
                    source, ability, ..
                } if *ability == TRIGGER_ID
                    && game
                        .card_definition(*source)
                        .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    if let Some(wolf) = watchwolf
                        && let Ok(characteristics) = game.characteristics(wolf)
                    {
                        for keyword in characteristics.keywords {
                            let rendered = format!("{keyword:?}");
                            if !observed_keywords.contains(&rendered) {
                                observed_keywords.push(rendered);
                            }
                        }
                    }
                }
                GameEvent::AuraAttached { aura, target }
                    if game
                        .card_definition(*aura)
                        .is_ok_and(|definition| definition.id == "RAV-FISTS-OF-IRONWOOD")
                        && Some(*target) != watchwolf =>
                {
                    attached_trample_aura = true
                }
                _ => {}
            }
        }
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
    }
    let winner = game.winner().map(|seat| seat.0);
    let terminal = winner.map_or_else(
        || "Draw".to_owned(),
        |seat| format!("Winner(PlayerId({seat}))"),
    );
    let player_life = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    let battlefield_definitions = game
        .view_for_player(PlayerId(0))
        .map_err(|error| error.to_string())?
        .own_battlefield
        .iter()
        .filter_map(|card| card.definition.map(str::to_owned))
        .collect();
    Ok(Capture {
        events: game.event_log.clone(),
        terminal,
        winner,
        turns: game.turn,
        seed,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        enchantment,
        watchwolf,
        observed_keywords,
        attached_trample_aura,
        battlefield_definitions,
        player_life,
    })
}

fn entry(card: &str, count: u8) -> DeckEntry {
    DeckEntry {
        card: card.to_owned(),
        count,
    }
}

struct ConcertedPolicy;
struct PassPolicy;

impl CodePolicy for PassPolicy {
    fn id(&self) -> &'static str {
        "rav.concerted-effort-pass.v1"
    }
    fn propose_move(&mut self, _view: &GameView) -> PolicyAction {
        if _view.step == Step::DeclareAttackers
            && _view.active_player == _view.decision_player
            && !_view.attackers_declared
        {
            PolicyAction::DeclareAttackers { attackers: vec![] }
        } else if _view.step == Step::DeclareBlockers
            && _view.active_player != _view.decision_player
            && !_view.blockers_declared
        {
            PolicyAction::DeclareBlockers {
                assignments: vec![],
            }
        } else {
            PolicyAction::PassPriority
        }
    }
}

impl CodePolicy for ConcertedPolicy {
    fn id(&self) -> &'static str {
        "rav.concerted-effort-keyword-sharing.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == PlayerId(0)
            && !view.attackers_declared
        {
            let all_present = [
                CARD_ID,
                WATCHWOLF_ID,
                "RAV-GRAYSCALED-GHARIAL",
                "RAV-SCREECHING-GRIFFIN",
                "RAV-NIGHTGUARD-PATROL",
                "RAV-BOROS-SWIFTBLADE",
                "RAV-FISTS-OF-IRONWOOD",
            ]
            .iter()
            .all(|id| {
                view.own_battlefield
                    .iter()
                    .any(|card| card.definition == Some(id))
            });
            let shared_snapshot_ready = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(WATCHWOLF_ID))
                .is_some_and(|wolf| {
                    [
                        Keyword::Flying,
                        Keyword::FirstStrike,
                        Keyword::DoubleStrike,
                        Keyword::Landwalk(cardbench_magic_engine::BasicLandType::Island),
                        Keyword::Trample,
                    ]
                    .iter()
                    .all(|required| wolf.keywords.contains(required))
                });
            let attackers = if all_present && shared_snapshot_ready {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.can_attack)
                    .map(|card| card.id)
                    .collect()
            } else {
                vec![]
            };
            return PolicyAction::DeclareAttackers { attackers };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player == PlayerId(1)
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == view.decision_player
            && view.active_player != PlayerId(0)
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers { attackers: vec![] };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != view.decision_player
            && view.active_player == PlayerId(0)
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.decision_player != PlayerId(0)
            || view.priority != PlayerId(0)
            || view.active_player != PlayerId(0)
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0
            && let Some(land) = view
                .hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
        {
            return PolicyAction::PlayLand { card: land.id };
        }
        let targets = [
            CARD_ID,
            WATCHWOLF_ID,
            "RAV-GRAYSCALED-GHARIAL",
            "RAV-SCREECHING-GRIFFIN",
            "RAV-NIGHTGUARD-PATROL",
            "RAV-BOROS-SWIFTBLADE",
            "RAV-FISTS-OF-IRONWOOD",
        ];
        for id in targets {
            if view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(id))
            {
                continue;
            }
            let Some(card) = view.hand.iter().find(|card| card.definition == Some(id)) else {
                continue;
            };
            let cost = match id {
                CARD_ID => ManaCost::with_colors(2, [Color::White, Color::White]),
                WATCHWOLF_ID => ManaCost::with_colors(0, [Color::Green, Color::White]),
                "RAV-GRAYSCALED-GHARIAL" => ManaCost::with_colors(0, [Color::Blue]),
                "RAV-SCREECHING-GRIFFIN" => ManaCost::with_colors(3, [Color::White]),
                "RAV-NIGHTGUARD-PATROL" => ManaCost::with_colors(2, [Color::White]),
                "RAV-BOROS-SWIFTBLADE" => ManaCost::with_colors(0, [Color::Red, Color::White]),
                "RAV-FISTS-OF-IRONWOOD" => ManaCost::with_colors(1, [Color::Green]),
                _ => unreachable!(),
            };
            if id == "RAV-FISTS-OF-IRONWOOD" {
                let Some(target) = view
                    .own_battlefield
                    .iter()
                    .find(|permanent| permanent.definition == Some("RAV-BOROS-SWIFTBLADE"))
                else {
                    continue;
                };
                if let Some(payment) = payment(view, &cost) {
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![Target::Permanent(target.id)],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: payment,
                    };
                }
            } else if let Some(payment) = payment(view, &cost) {
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
        PolicyAction::PassPriority
    }
}

fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<_> = Color::MANA_ALL
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
    let missing = cost.colored.iter().copied().find(|color| {
        let required = cost
            .colored
            .iter()
            .filter(|required_color| **required_color == *color)
            .count();
        usize::from(view.mana_pool.amount(*color)) < required
    });
    let total = u16::from(cost.generic) + u16::try_from(cost.colored.len()).ok()?;
    let needed = missing.or_else(|| {
        (u16::from(view.mana_pool.total()) < total)
            .then(|| cost.colored.first().copied().unwrap_or(Color::White))
    })?;
    let land = view
        .own_battlefield
        .iter()
        .find(|card| {
            !card.tapped
                && card.card_types.contains(&CardType::Land)
                && match (card.basic_land_type, needed) {
                    (Some(cardbench_magic_engine::BasicLandType::Plains), Color::White)
                    | (Some(cardbench_magic_engine::BasicLandType::Forest), Color::Green)
                    | (Some(cardbench_magic_engine::BasicLandType::Island), Color::Blue)
                    | (Some(cardbench_magic_engine::BasicLandType::Mountain), Color::Red) => true,
                    _ => false,
                }
        })
        .or_else(|| {
            view.own_battlefield
                .iter()
                .find(|card| !card.tapped && card.card_types.contains(&CardType::Land))
        })?;
    let color = match land.basic_land_type? {
        cardbench_magic_engine::BasicLandType::Plains => Color::White,
        cardbench_magic_engine::BasicLandType::Forest => Color::Green,
        cardbench_magic_engine::BasicLandType::Island => Color::Blue,
        cardbench_magic_engine::BasicLandType::Mountain => Color::Red,
        _ => return None,
    };
    Some(PolicyAction::ActivateManaAbility {
        land: land.id,
        color,
    })
}

fn validate_trace(capture: &Capture) -> Result<Value, String> {
    if capture.terminal != "Winner(PlayerId(0))"
        || capture.winner != Some(0)
        || capture.rejected != 0
    {
        return Err("Concerted Effort duel did not end in a clean player-zero win".to_owned());
    }
    let enchantment = capture.enchantment.ok_or("Concerted Effort was not cast")?;
    let wolf = capture.watchwolf.ok_or("Watchwolf was not cast")?;
    for (label, predicate) in [
        ("Flying", "Flying"),
        ("FirstStrike", "FirstStrike"),
        ("DoubleStrike", "DoubleStrike"),
        ("Islandwalk", "Landwalk(Island)"),
        ("Trample", "Trample"),
    ] {
        if !capture
            .observed_keywords
            .iter()
            .any(|keyword| keyword.contains(predicate))
        {
            return Err(format!(
                "Watchwolf did not receive shared {label}: {:?}; battlefield={:?}",
                capture.observed_keywords, capture.battlefield_definitions
            ));
        }
    }
    if !capture.attached_trample_aura {
        return Err(
            "the RAV Aura Trample provider did not resolve onto another controlled creature"
                .to_owned(),
        );
    }
    let cast = index(&capture.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == enchantment)).ok_or("Concerted Effort cast receipt missing")?;
    let resolved = index_after(
        &capture.events,
        cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == enchantment),
    )
    .ok_or("Concerted Effort resolution receipt missing")?;
    let entered = index_after(&capture.events, resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == enchantment)).ok_or("Concerted Effort battlefield entry missing")?;
    let trigger = index_after(&capture.events, entered, |event| matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, ability, .. } if *source == enchantment && *ability == TRIGGER_ID)).ok_or("Concerted Effort upkeep trigger was not stacked")?;
    let trigger_resolved = index_after(&capture.events, trigger, |event| matches!(event, GameEvent::AbilityResolved { source, ability, .. } if *source == enchantment && *ability == TRIGGER_ID)).ok_or("Concerted Effort upkeep trigger did not resolve")?;
    let attack = index_after(&capture.events, trigger_resolved, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&wolf))).ok_or("shared-keyword recipient did not attack")?;
    let damage = index_after(&capture.events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount } if *source == wolf && *amount > 0)).ok_or("shared-keyword recipient did not deal combat damage")?;
    let ended = index_after(&capture.events, damage, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("full-game terminal result missing")?;
    let projected = project_events(&capture.events);
    Ok(json!({
        "id":"rav-concerted-effort-upkeep-keyword-sharing",
        "seed":capture.seed,
        "decks":[PILOT_DECK,OPPONENT_DECK],
        "policies":["rav.concerted-effort-keyword-sharing.v1","rav.concerted-effort-pass.v1"],
        "winner_seat":capture.winner,"terminal":capture.terminal,"turns":capture.turns,"accepted_moves":capture.accepted,"rejected_moves":capture.rejected,
        "concerted_effort":{"object":enchantment.0,"mana_cost":"{2}{W}{W}","trigger_id":TRIGGER_ID,"trigger_resolutions":capture.events.iter().filter(|event| matches!(event,GameEvent::AbilityResolved{source,ability,..} if *source==enchantment && *ability==TRIGGER_ID)).count()},
        "recipient":{"definition_id":WATCHWOLF_ID,"object":wolf.0,"shared_keywords":capture.observed_keywords,"received_trample_from_other_creature":capture.attached_trample_aura},
        "ordered_event_indices":{"spell_cast":cast,"spell_resolved":resolved,"battlefield_entry":entered,"upkeep_trigger_stacked":trigger,"upkeep_trigger_resolved":trigger_resolved,"recipient_attack":attack,"recipient_damage":damage,"terminal":ended},
        "events":projected.iter().filter(|event| ["SpellManaPaid","SpellCast","SpellResolved","TriggeredAbilityStacked","AbilityResolved","AuraAttached","AttackersDeclared","DamageDealtToPlayer","GameEnded"].contains(&event.kind.as_str())).collect::<Vec<_>>(),
        "passed":true
    }))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Err(_) => Ok(true),
        Ok(capture) => Ok(validate_trace(&capture).is_err()),
    }
}
fn index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn index_after(
    events: &[GameEvent],
    after: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(after + 1)
        .find_map(|(i, e)| predicate(e).then_some(i))
}
fn card_status(card: &CatalogCard) -> Value {
    let id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => definition_id,
        _ => "",
    };
    let passed = id == CARD_ID;
    json!({"collector_number":card.collector_number,"definition_id":id,"name":card.name,"engine_status":"executable_compatibility_slice","qualification":if passed{"pilot_trace_passed; independent review pending"}else{"not_evaluated"},"trace_status":if passed{"passed"}else{"not_qualified"},"credited":false})
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
    while let Some(arg) = args.next() {
        if arg == "--output-root" {
            return args.next().map(Into::into);
        }
    }
    None
}
fn write_outputs(root: &std::path::Path, report: &Value) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::write(root.join("reward.txt"), b"0.0\n").map_err(|e| e.to_string())?;
    if let Some(trace) = report["traces"]
        .as_array()
        .and_then(|traces| traces.first())
    {
        std::fs::write(
            root.join("trace.json"),
            serde_json::to_vec_pretty(trace).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
