//! Full-game diagnostic authority for RAV Hour of Reckoning.

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, ConvokeContribution, ConvokePayment, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings,
    rav_attachment_triggered_ability_bindings as attachment_triggers, rav_basic_land_type_bindings,
    rav_land_entry_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-HOUR-OF-RECKONING";
const CARD_NAME: &str = "Hour of Reckoning";
const BIRD_ID: &str = "RAV-BIRDS-OF-PARADISE";
const DRAGON_ID: &str = "RAV-HUNTED-DRAGON";
const DRAGON_TRIGGER_ID: &str = "etb-opponent-knights";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/hour_of_reckoning.rs";
const MAX_TURNS: u32 = 60;
const MAX_MOVES: u32 = 8_000;
const MAX_SEED: u64 = 64;
const EXPECTED_CANDIDATE_SOURCE: &str = r#"//! Candidate-owned RAV Hour of Reckoning definition.

use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, Keyword, ManaCost};
use std::collections::BTreeSet;

/// Printed white sorcery with Convoke that destroys all nontoken creatures.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-HOUR-OF-RECKONING",
        name: "Hour of Reckoning",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(4, [Color::White, Color::White, Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Sorcery]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "convoke",
            "destroy-all-nontoken-creatures",
        ],
        power: None,
        toughness: None,
        keywords: vec![Keyword::Convoke],
        effects: vec![Effect::DestroyAllNonTokenCreatures],
    }
}
"#;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/hour_of_reckoning.rs"]
mod candidate_hour_of_reckoning;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Hour of Reckoning result: {error}");
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
            eprintln!("Hour of Reckoning PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    let side_effect_rejected = injected_side_effect_rejected()?;
    validate_candidate_definition()?;
    let catalog = unique_catalog()?;
    let (seed, reference) = find_trace()?;
    let trace = validate_trace(&reference, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if reference != replay || project_events(&reference.events) != project_events(&replay.events) {
        return Err("Hour of Reckoning full-game replay differed".into());
    }
    let mutations = json!({
        "convoke_removed_detected_in_game": mutation_detected(seed, Mutation::NoConvoke),
        "mana_cost_changed_detected_in_game": mutation_detected(seed, Mutation::WrongCost),
        "nontoken_destruction_removed_detected_in_game": mutation_detected(seed, Mutation::NoDestroy),
        "syntactic_filesystem_side_effect_rejected_before_candidate_call": side_effect_rejected,
        "sealed_mutation_authority": false
    });
    for key in [
        "convoke_removed_detected_in_game",
        "mana_cost_changed_detected_in_game",
        "nontoken_destruction_removed_detected_in_game",
        "syntactic_filesystem_side_effect_rejected_before_candidate_call",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Hour of Reckoning semantic mutation escaped: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,
        "expansion":"RAV",
        "card":{"collector_number":21,"printings":1,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":["601.2 Casting Spells and Paying Costs","608.2 Resolving Spells","701.7 Destroy","702.50 Convoke","111 Tokens","508.1 Declaring Attackers","510 Combat Damage"],
            "source_contract":"RAV #21 Hour of Reckoning is a white {4}{W}{W}{W} Sorcery with Convoke that destroys all nontoken creatures.",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true
        },
        "passed":true,
        "pilot_card_passed":true,
        "deterministic_replay":true,
        "cards_passed":1,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":rav_main_set_catalog().len(),
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":"unreviewed_diagnostic",
        "mutation_sensitivity":mutations,
        "traces":[trace],
        "per_card_breakdown":breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoConvoke,
    WrongCost,
    NoDestroy,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TokenFact {
    id: ObjectId,
    controller: PlayerId,
    zone: Option<Zone>,
    colors: BTreeSet<Color>,
    card_types: BTreeSet<CardType>,
    power: Option<i32>,
    toughness: Option<i32>,
    first_strike: bool,
    is_token: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    hour: Option<ObjectId>,
    bird: Option<ObjectId>,
    dragon: Option<ObjectId>,
    mana_paid: Vec<Color>,
    tokens: Vec<TokenFact>,
    spell_zone: Option<Zone>,
}

fn mutated_definition(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_hour_of_reckoning::definition();
    match mutation {
        Mutation::None => {}
        Mutation::NoConvoke => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Convoke),
        Mutation::WrongCost => {
            definition.mana_cost = ManaCost::with_colors(4, [Color::White, Color::White])
        }
        Mutation::NoDestroy => definition.effects.clear(),
    }
    definition
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Hour of Reckoning source: {error}"))?;
    validate_candidate_source_text(&source)
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let file = syn::parse_file(source)
        .map_err(|_| "Hour of Reckoning source is not valid Rust".to_owned())?;
    if source != EXPECTED_CANDIDATE_SOURCE
        || !file.attrs.iter().all(|attr| attr.path().is_ident("doc"))
        || file.items.len() != 3
    {
        return Err("Hour of Reckoning source differs from the task contract".into());
    }
    let mut found_definition = false;
    for item in &file.items {
        match item {
            Item::Use(item) if matches!(item.vis, Visibility::Inherited) => {}
            Item::Fn(function)
                if function.sig.ident == "definition"
                    && matches!(function.vis, Visibility::Restricted(_))
                    && function.sig.inputs.is_empty()
                    && function.sig.generics.params.is_empty()
                    && function.block.stmts.len() == 1 =>
            {
                let [Stmt::Expr(Expr::Struct(literal), None)] = function.block.stmts.as_slice()
                else {
                    return Err("Hour of Reckoning must return one literal definition".into());
                };
                if literal.path.is_ident("CardDefinition") && literal.rest.is_none() {
                    found_definition = true;
                }
            }
            _ => return Err("Hour of Reckoning source contains an unsupported item".into()),
        }
    }
    if !found_definition {
        return Err("Hour of Reckoning CardDefinition literal was not found".into());
    }
    Ok(())
}

fn injected_side_effect_rejected() -> Result<bool, String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Hour of Reckoning source: {error}"))?;
    let signature = "pub(crate) fn definition() -> CardDefinition {";
    let injected = "pub(crate) fn definition() -> CardDefinition {\n    let _ = std::fs::write(\"/tmp/cardbench-hour-of-reckoning-candidate-executed\", b\"executed\");";
    let malicious = source.replacen(signature, injected, 1);
    if malicious == source || syn::parse_file(&malicious).is_err() {
        return Err("could not construct syntactically valid side-effect injection".into());
    }
    Ok(validate_candidate_source_text(&malicious).is_err())
}

fn validate_candidate_definition() -> Result<(), String> {
    let definition = candidate_hour_of_reckoning::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Hour of Reckoning is missing from the RAV registry")?;
    if definition != registered
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost
            != ManaCost::with_colors(4, [Color::White, Color::White, Color::White])
        || definition.colors != BTreeSet::from([Color::White])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Sorcery])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || definition.keywords != [Keyword::Convoke]
        || definition.effects != [Effect::DestroyAllNonTokenCreatures]
    {
        return Err("Hour of Reckoning definition differs from exact RAV #21 cost, Convoke, or destruction contract".into());
    }
    Ok(())
}

fn new_game(definition: CardDefinition) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Hour of Reckoning is missing from the card registry")?;
    *registered = definition;
    let mut triggers = rav_triggered_ability_bindings();
    triggers.extend(attachment_triggers());
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers,
        vec![],
        rav_land_entry_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_attachment_bindings(rav_attachment_bindings())
        .map_err(|error| error.to_string())?;
    Ok(game)
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_HOUR_OF_RECKONING_SEED") {
        let seed = value.parse().map_err(|_| {
            "CARDBENCH_HOUR_OF_RECKONING_SEED must be an unsigned integer".to_owned()
        })?;
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    let mut last_error = String::new();
    for seed in 0..=MAX_SEED {
        if let Ok(captured) = run_match(seed, Mutation::None) {
            match validate_trace(&captured, seed) {
                Ok(_) => return Ok((seed, captured)),
                Err(error) => last_error = error,
            }
        }
    }
    Err(format!(
        "no terminal Hour of Reckoning trace in seeds 0..={MAX_SEED}: {last_error}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut game = new_game(mutated_definition(mutation))?;
    let pilot = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: BIRD_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 26,
            },
            DeckEntry {
                card: "RAV-FOREST".into(),
                count: 26,
            },
        ],
        sideboard: vec![],
    };
    let opponent = DeckList {
        mainboard: vec![
            DeckEntry {
                card: DRAGON_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".into(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    if deck_size(&pilot) != 60 || deck_size(&opponent) != 60 {
        return Err("Hour of Reckoning duel requires two exact 60-card decks".into());
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_MOVES,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent)
        .map_err(|error| error.to_string())?;
    for player in [PlayerId(0), PlayerId(1)] {
        game.draw_opening_hand(player, config.opening_hand_size)
            .map_err(|error| error.to_string())?;
    }
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(HourPilot {
            player: PlayerId(0),
            hour_cast: false,
            bird_cast: false,
        }),
        Box::new(DragonOpponent {
            player: PlayerId(1),
            cast_once: false,
        }),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || accepted >= MAX_MOVES {
            return Err(format!(
                "Hour of Reckoning match exceeded bounds at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Hour of Reckoning policy seat missing")?;
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
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Hour of Reckoning policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
    }
    let events = game.event_log.clone();
    let hour = events.iter().find_map(|event| match event {
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
    let bird = events.iter().find_map(|event| match event {
        GameEvent::CardMoved {
            card,
            to: Zone::Battlefield,
        } if game
            .card_definition(*card)
            .is_ok_and(|definition| definition.id == BIRD_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let dragon = events.iter().find_map(|event| match event {
        GameEvent::CardMoved {
            card,
            to: Zone::Battlefield,
        } if game
            .card_definition(*card)
            .is_ok_and(|definition| definition.id == DRAGON_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let mana_paid = hour
        .and_then(|hour| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if *card == hour => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    let tokens = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::TokenCreated {
                player: PlayerId(0),
                token,
            } => {
                let characteristics = game.characteristics(*token).ok()?;
                Some(TokenFact {
                    id: *token,
                    controller: game.controller_of(*token).ok()?,
                    zone: game.zone_of(*token),
                    colors: characteristics.colors,
                    card_types: characteristics.card_types,
                    power: characteristics.power,
                    toughness: characteristics.toughness,
                    first_strike: characteristics.keywords.contains(&Keyword::FirstStrike),
                    is_token: game
                        .object(*token)
                        .is_ok_and(|object| object.token.is_some()),
                })
            }
            _ => None,
        })
        .collect();
    Ok(Captured {
        events,
        winner: game.winner().map(|player| player.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        hour,
        bird,
        dragon,
        mana_paid,
        tokens,
        spell_zone: hour.and_then(|spell| game.zone_of(spell)),
    })
}

struct HourPilot {
    player: PlayerId,
    hour_cast: bool,
    bird_cast: bool,
}

impl CodePolicy for HourPilot {
    fn id(&self) -> &'static str {
        "rav.hour-of-reckoning-convoke-and-sweep.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = if self.hour_cast {
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
            && view.active_player != self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.player != self.player
            || view.decision_player != self.player
            || view.priority != self.player
            || view.active_player != self.player
            || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            if let Some(land) = land_to_play(view) {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        let bird = view
            .hand
            .iter()
            .find(|card| card.definition == Some(BIRD_ID));
        if !self.bird_cast
            && view
                .own_battlefield
                .iter()
                .all(|card| card.definition != Some(BIRD_ID))
        {
            if let Some(bird) = bird {
                if view.mana_pool.amount(Color::Green) >= 1 {
                    self.bird_cast = true;
                    return PolicyAction::Cast(CastRequest {
                        card: bird.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    });
                }
                if let Some(land) = view
                    .own_battlefield
                    .iter()
                    .find(|card| !card.tapped && card.definition == Some("RAV-FOREST"))
                {
                    return PolicyAction::ActivateManaAbility {
                        land: land.id,
                        color: Color::Green,
                    };
                }
            }
        }
        let Some(hour) = view
            .hand
            .iter()
            .find(|card| card.definition == Some(CARD_ID))
        else {
            return activate_any_land(view, self.player);
        };
        let has_dragon = view
            .opponent_battlefield
            .iter()
            .any(|card| card.definition == Some(DRAGON_ID));
        let birds_on_board = view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some(BIRD_ID));
        let mut tokens: Vec<_> = view
            .own_battlefield
            .iter()
            .filter(|card| {
                card.definition.is_none()
                    && card.card_types.contains(&CardType::Creature)
                    && card.colors.contains(&Color::White)
                    && !card.tapped
            })
            .map(|card| card.id)
            .collect();
        if !has_dragon || !birds_on_board || tokens.len() < 2 {
            return activate_any_land(view, self.player);
        }
        let white = view.mana_pool.amount(Color::White);
        let green = view.mana_pool.amount(Color::Green);
        if white < 2 {
            if let Some(land) = view
                .own_battlefield
                .iter()
                .find(|card| !card.tapped && card.definition == Some("RAV-PLAINS"))
            {
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: Color::White,
                };
            }
        }
        if green < 3 {
            if let Some(land) = view
                .own_battlefield
                .iter()
                .find(|card| !card.tapped && card.definition == Some("RAV-FOREST"))
            {
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: Color::Green,
                };
            }
        }
        if white >= 2 && green >= 3 {
            let generic_token = tokens.remove(0);
            let white_token = tokens.remove(0);
            self.hour_cast = true;
            return PolicyAction::CastWithPayment {
                request: CastRequest {
                    card: hour.id,
                    targets: vec![],
                    convoke: vec![
                        ConvokePayment {
                            creature: generic_token,
                            contribution: ConvokeContribution::Generic,
                        },
                        ConvokePayment {
                            creature: white_token,
                            contribution: ConvokeContribution::Color(Color::White),
                        },
                    ],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection: ManaPaymentSelection {
                    generic: vec![Color::Green; 3],
                    hybrid: vec![],
                },
            };
        }
        activate_any_land(view, self.player)
    }
}

struct DragonOpponent {
    player: PlayerId,
    cast_once: bool,
}

impl DragonOpponent {
    fn choose_target(&self, view: &GameView) -> Option<PolicyAction> {
        let choice = view.triggered_ability_target_choice.as_ref()?;
        if choice.ability != DRAGON_TRIGGER_ID
            || !choice
                .target_options
                .first()?
                .contains(&Target::Player(PlayerId(0)))
        {
            return None;
        }
        Some(PolicyAction::ChooseTriggeredAbilityTargets {
            decision: choice.decision,
            source: choice.source,
            ability: choice.ability,
            targets: vec![Target::Player(PlayerId(0))],
        })
    }
}

impl CodePolicy for DragonOpponent {
    fn id(&self) -> &'static str {
        "rav.hour-of-reckoning-hunted-dragon-support.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if let Some(action) = self.choose_target(view) {
            return action;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = view
                .own_battlefield
                .iter()
                .filter(|card| card.can_attack)
                .map(|card| card.id)
                .collect();
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
        if view.player != self.player
            || view.decision_player != self.player
            || view.priority != self.player
            || view.active_player != self.player
            || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-MOUNTAIN"))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if !self.cast_once
            && view
                .hand
                .iter()
                .any(|card| card.definition == Some(DRAGON_ID))
        {
            if view.mana_pool.amount(Color::Red) >= 5 {
                let dragon = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(DRAGON_ID))
                    .expect("checked");
                self.cast_once = true;
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: dragon.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: ManaPaymentSelection {
                        generic: vec![Color::Red; 3],
                        hybrid: vec![],
                    },
                };
            }
            if let Some(land) = view
                .own_battlefield
                .iter()
                .find(|card| !card.tapped && card.definition == Some("RAV-MOUNTAIN"))
            {
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: Color::Red,
                };
            }
        }
        PolicyAction::PassPriority
    }

    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        self.choose_target(view)
    }
}

fn land_to_play(view: &GameView) -> Option<&cardbench_magic_engine::CardView> {
    let plains = view
        .own_battlefield
        .iter()
        .filter(|card| card.definition == Some("RAV-PLAINS"))
        .count();
    let forests = view
        .own_battlefield
        .iter()
        .filter(|card| card.definition == Some("RAV-FOREST"))
        .count();
    let preferred = if plains < 2 {
        "RAV-PLAINS"
    } else if forests < 3 {
        "RAV-FOREST"
    } else if plains <= forests {
        "RAV-PLAINS"
    } else {
        "RAV-FOREST"
    };
    view.hand
        .iter()
        .find(|card| card.definition == Some(preferred))
        .or_else(|| {
            view.hand
                .iter()
                .find(|card| matches!(card.definition, Some("RAV-PLAINS" | "RAV-FOREST")))
        })
}

fn activate_any_land(view: &GameView, player: PlayerId) -> PolicyAction {
    if view.player == player && view.priority == player {
        if let Some(land) = view
            .own_battlefield
            .iter()
            .find(|card| !card.tapped && card.definition == Some("RAV-PLAINS"))
        {
            return PolicyAction::ActivateManaAbility {
                land: land.id,
                color: Color::White,
            };
        }
        if let Some(land) = view
            .own_battlefield
            .iter()
            .find(|card| !card.tapped && card.definition == Some("RAV-FOREST"))
        {
            return PolicyAction::ActivateManaAbility {
                land: land.id,
                color: Color::Green,
            };
        }
    }
    PolicyAction::PassPriority
}

fn validate_trace(c: &Captured, seed: u64) -> Result<Value, String> {
    if c.winner != Some(0) || c.rejected != 0 {
        return Err("Hour of Reckoning duel did not reach a clean terminal seat-zero win".into());
    }
    let hour = c.hour.ok_or("Hour of Reckoning was not cast")?;
    let bird = c
        .bird
        .ok_or("the pilot's nontoken creature was not established")?;
    let dragon = c
        .dragon
        .ok_or("opponent Hunted Dragon support creature was not established")?;
    if c.spell_zone != Some(Zone::Graveyard)
        || c.mana_paid
            .iter()
            .filter(|color| **color == Color::White)
            .count()
            != 2
        || c.mana_paid
            .iter()
            .filter(|color| **color == Color::Green)
            .count()
            != 3
    {
        return Err(format!(
            "Hour of Reckoning did not pay the expected 2 white and 3 green mana: {:?}",
            c.mana_paid
        ));
    }
    if c.tokens.len() != 3
        || c.tokens.iter().any(|token| {
            token.controller != PlayerId(0)
                || token.zone != Some(Zone::Battlefield)
                || !token.is_token
                || !token.colors.contains(&Color::White)
                || !token.card_types.contains(&CardType::Creature)
                || token.power != Some(2)
                || token.toughness != Some(2)
                || !token.first_strike
        })
    {
        return Err(format!(
            "the three white 2/2 First Strike Knight tokens did not survive: {:?}",
            c.tokens
        ));
    }
    let destructions: Vec<_> = c
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::CardDestroyed { source, card } if *source == hour => Some((index, *card)),
            _ => None,
        })
        .collect();
    if destructions
        .iter()
        .map(|(_, card)| *card)
        .collect::<BTreeSet<_>>()
        != BTreeSet::from([bird, dragon])
    {
        return Err(format!(
            "Hour did not destroy exactly both battlefield nontoken creatures: {:?}",
            destructions
        ));
    }
    let convokes: Vec<_> = c
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::ConvokeUsed {
                player: PlayerId(0),
                creature,
                contribution,
            } => Some((index, *creature, *contribution)),
            _ => None,
        })
        .collect();
    let generic = convokes
        .iter()
        .find(|(_, creature, contribution)| *contribution == None && *creature != bird)
        .ok_or("generic Convoke receipt missing")?;
    let white = convokes
        .iter()
        .find(|(_, creature, contribution)| *contribution == Some(Color::White))
        .ok_or("white Convoke receipt missing")?;
    let payment = event_index(&c.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, .. } if *card == hour)).ok_or("direct Hour mana payment receipt missing")?;
    let cast = event_index(&c.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == hour)).ok_or("Hour SpellCast receipt missing")?;
    let first_destroy = destructions
        .iter()
        .map(|(index, _)| *index)
        .min()
        .ok_or("Hour destruction receipts missing")?;
    let resolve = event_index(
        &c.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == hour),
    )
    .ok_or("Hour resolution receipt missing")?;
    let spell_move = event_index(&c.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == hour)).ok_or("Hour did not move to its owner's graveyard")?;
    let attack = event_index_after(&c.events, spell_move, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if c.tokens.iter().any(|token| attackers.contains(&token.id)))).ok_or("surviving tokens did not attack after Hour resolved")?;
    let damage = event_index_after(&c.events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { player: PlayerId(1), amount, .. } if *amount > 0)).ok_or("surviving token combat damage was not recorded")?;
    let ended = event_index(&c.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal win receipt missing")?;
    if !(generic.0 < payment
        && white.0 < payment
        && payment < cast
        && cast < first_destroy
        && first_destroy < resolve
        && resolve < spell_move
        && spell_move < attack
        && attack < damage
        && damage < ended)
    {
        return Err("Hour Convoke, mana payment, global destruction, resolution, and combat event order differs".into());
    }
    let selected: BTreeSet<_> = [
        generic.0,
        white.0,
        payment,
        cast,
        first_destroy,
        resolve,
        spell_move,
        attack,
        damage,
        ended,
    ]
    .into_iter()
    .collect();
    let projected: Vec<_> = project_events(&c.events)
        .into_iter()
        .enumerate()
        .filter_map(|(index, event)| selected.contains(&index).then_some(event))
        .collect();
    Ok(json!({
        "trace_id":"rav-hour-of-reckoning-convoke-token-safe-global-destruction-full-game",
        "shuffle_seed":seed,
        "decks":["rav_hour_of_reckoning_birds_two_colors_60","rav_hunted_dragon_mountains_60"],
        "policies":["rav.hour-of-reckoning-convoke-and-sweep.v1","rav.hour-of-reckoning-hunted-dragon-support.v1"],
        "terminal":"Winner(PlayerId(0))","winner_seat":0,"turns":c.turns,"accepted_moves":c.accepted,"rejected_moves":c.rejected,"passed":true,
        "hour_of_reckoning":{"object":hour.0,"collector_number":21,"mana_cost":"{4}{W}{W}{W}","direct_mana_paid":c.mana_paid,"convoke_generic_creature":convokes.iter().find(|(_, _, contribution)| *contribution == None).map(|(_, creature, _)| creature.0),"convoke_white_creature":white.1.0,"destroyed_nontoken_creatures":[bird.0,dragon.0],"surviving_white_knight_tokens":c.tokens.iter().map(|token| token.id.0).collect::<Vec<_>>(),"spell_zone_after_resolution":"Graveyard","terminal_combat_win":true},
        "ordered_event_indices":[generic.0,white.0,payment,cast,first_destroy,resolve,spell_move,attack,damage,ended],
        "events":projected
    }))
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
fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    run_match(seed, mutation)
        .and_then(|capture| validate_trace(&capture, seed).map(|_| ()))
        .is_err()
}
fn deck_size(deck: &DeckList) -> usize {
    deck.mainboard
        .iter()
        .map(|entry| usize::from(entry.count))
        .sum()
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
        || cards.len() != 291
        || printings.len() != 306
    {
        return Err("RAV catalog denominator differs from the source contract".into());
    }
    Ok(cards)
}
fn card_status(card: &CatalogCard) -> Value {
    let id = card_definitions()
        .into_iter()
        .find(|definition| definition.name == card.name)
        .map(|definition| definition.id);
    let passed = id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,"engine_status":match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" },"trace_status":if passed {"passed"} else {"not_qualified"},"credited":false,"qualification":if passed {"pilot_trace_passed; independent reviewer approval pending"} else {"not_evaluated"}})
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
    std::fs::write(root.join("reward.txt"), "0.00000000\n")
}
