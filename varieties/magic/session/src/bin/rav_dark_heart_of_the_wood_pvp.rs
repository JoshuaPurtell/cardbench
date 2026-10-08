//! Full-game RAV authority for Dark Heart of the Wood's typed Forest sacrifice.

use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, BasicLandType, CardDefinition, CardType, CastRequest,
    Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, GeneralizedActivatedAbilityCost,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings,
    rav_generalized_activated_ability_cost_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use syn::{Expr, Item, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-DARK-HEART-OF-THE-WOOD";
const CARD_NAME: &str = "Dark Heart of the Wood";
const ABILITY_ID: &str = "green-sacrifice-forest-gain-three-life";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const FOREST_ID: &str = "RAV-FOREST";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/dark_heart_of_the_wood.rs";
const MAX_REFERENCE_SEED: u64 = 64;
const MAX_TURNS: u32 = 60;
const MAX_POLICY_MOVES: u32 = 8_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/dark_heart_of_the_wood.rs"]
mod candidate_dark_heart;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Dark Heart of the Wood result: {error}");
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
            eprintln!("Dark Heart of the Wood PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_runtime()?;
    let catalog = unique_catalog()?;
    let (seed, reference) = find_trace()?;
    let trace = validate_trace(&reference, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if reference != replay {
        return Err("Dark Heart of the Wood full-game replay was not deterministic".to_owned());
    }
    let mutations = [
        (
            "printed_cost_mutation_detected_in_game",
            Mutation::WrongPrintedCost,
        ),
        (
            "activation_cost_mutation_detected_in_game",
            Mutation::WrongActivationCost,
        ),
        (
            "forest_type_mutation_detected_in_game",
            Mutation::WrongSacrificeType,
        ),
        ("life_gain_mutation_detected_in_game", Mutation::GainTwoLife),
    ];
    let mut sensitivity = serde_json::Map::new();
    for (key, mutation) in mutations {
        let detected = mutation_detected(seed, mutation)?;
        if !detected {
            return Err(format!("Dark Heart semantic mutation escaped: {key}"));
        }
        sensitivity.insert(key.to_owned(), Value::Bool(true));
    }
    sensitivity.insert("sealed_mutation_authority".to_owned(), Value::Bool(false));
    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 200, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "106.1 and 106.4 Mana",
                "117 Priority",
                "202.1-202.2 Mana cost and color",
                "303 Enchantments",
                "601.2 Casting and paying costs",
                "602.2 Activating abilities and paying costs",
                "608 Resolving abilities",
                "701.17 Sacrifice",
                "119.3 Gaining life",
                "510 Combat damage"
            ],
            "source_contract": "RAV #200 is a {B}{G} enchantment with {G}, sacrifice a Forest: gain 3 life",
            "authority_cutoff": "2026-09-24",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "unreviewed_diagnostic",
        "deterministic_replay": true,
        "mutation_sensitivity": sensitivity,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongPrintedCost,
    WrongActivationCost,
    WrongSacrificeType,
    GainTwoLife,
}

fn candidate_values(
    mutation: Mutation,
) -> (
    CardDefinition,
    ActivatedAbility,
    GeneralizedActivatedAbilityCost,
) {
    let mut definition = candidate_dark_heart::definition();
    let mut ability = candidate_dark_heart::activated_ability().ability;
    let mut cost = candidate_dark_heart::activated_ability_cost().cost;
    match mutation {
        Mutation::None => {}
        Mutation::WrongPrintedCost => {
            definition.mana_cost = ManaCost::with_colors(0, [Color::Black]);
        }
        Mutation::WrongActivationCost => {
            ability.mana_cost = ManaCost::with_colors(0, [Color::Black]);
        }
        Mutation::WrongSacrificeType => {
            cost.sacrifice_land_basic_type = Some(BasicLandType::Plains);
        }
        Mutation::GainTwoLife => {
            ability.effects = vec![Effect::GainLifeController { amount: 2 }];
        }
    }
    (definition, ability, cost)
}

fn validate_candidate_runtime() -> Result<(), String> {
    let (definition, ability, cost) = candidate_values(Mutation::None);
    let registered_definition = card_definitions()
        .into_iter()
        .find(|registered| registered.id == CARD_ID)
        .ok_or("Dark Heart of the Wood is missing from the RAV registry")?;
    let registered_ability = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("Dark Heart activated ability is missing from the RAV registry")?;
    let registered_cost = rav_generalized_activated_ability_cost_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability_id == ABILITY_ID)
        .ok_or("Dark Heart Forest-sacrifice cost is missing from the RAV registry")?;
    if definition != registered_definition
        || ability != registered_ability.ability
        || cost != registered_cost.cost
    {
        return Err(
            "Dark Heart candidate module is not the authoritative RAV registry binding".to_owned(),
        );
    }
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(0, [Color::Black, Color::Green])
        || definition.colors != BTreeSet::from([Color::Black, Color::Green])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Enchantment])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
        || ability.id != ABILITY_ID
        || ability.mana_cost != ManaCost::with_colors(0, [Color::Green])
        || ability.tap_cost
        || ability.sorcery_speed
        || ability.additional_tap_creatures != 0
        || ability.sacrifice_source
        || ability.sacrifice_creatures != 0
        || ability.sacrifice_lands != 1
        || ability.discard_cards != 0
        || !ability.targets.is_empty()
        || ability.effects != [Effect::GainLifeController { amount: 3 }]
        || cost.sacrifice_land_basic_type != Some(BasicLandType::Forest)
    {
        return Err("Dark Heart candidate differs from the RAV #200 feature contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Dark Heart source: {error}"))?;
    if source.len() > 64 * 1024 {
        return Err("Dark Heart source exceeds 64 KiB".to_owned());
    }
    let file = syn::parse_file(&source)
        .map_err(|_| "Dark Heart source is not valid declarative Rust".to_owned())?;
    let mut functions = BTreeSet::new();
    let mut uses = 0;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                uses += 1;
            }
            Item::Fn(function) => {
                if !function
                    .attrs
                    .iter()
                    .all(|attr| attr.path().is_ident("doc") || attr.path().is_ident("must_use"))
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !function.sig.inputs.is_empty()
                    || !function.sig.generics.params.is_empty()
                    || function.sig.generics.where_clause.is_some()
                    || function.sig.constness.is_some()
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || function.sig.abi.is_some()
                    || function.sig.variadic.is_some()
                    || function.block.stmts.len() != 1
                {
                    return Err(
                        "Dark Heart factories must be direct crate-visible literal factories"
                            .to_owned(),
                    );
                }
                let direct_literal = matches!(
                    &function.block.stmts[0],
                    Stmt::Expr(Expr::Struct(value), None) if value.rest.is_none()
                );
                if !direct_literal || !functions.insert(function.sig.ident.to_string()) {
                    return Err(
                        "Dark Heart factories must return direct struct literals".to_owned()
                    );
                }
            }
            _ => return Err("Dark Heart source contains an unsupported top-level item".to_owned()),
        }
    }
    if uses != 2
        || file.items.len() != 5
        || functions
            != BTreeSet::from([
                "definition".to_owned(),
                "activated_ability".to_owned(),
                "activated_ability_cost".to_owned(),
            ])
    {
        return Err("Dark Heart module structure differs from the task contract".to_owned());
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
    {
        return Err(
            "RAV catalog identity/printing denominator differs from the task contract".to_owned(),
        );
    }
    Ok(catalog)
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_DARK_HEART_OF_THE_WOOD_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "Dark Heart seed must be an unsigned integer".to_owned())?;
        if seed > MAX_REFERENCE_SEED {
            return Err(format!("Dark Heart seed {seed} exceeds the bounded range"));
        }
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    for seed in 0..=MAX_REFERENCE_SEED {
        if let Ok(captured) = run_match(seed, Mutation::None) {
            if validate_trace(&captured, seed).is_ok() {
                return Ok((seed, captured));
            }
        }
    }
    Err("no bounded seed produced the Dark Heart full-game trace".to_owned())
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Ok(captured) => Ok(validate_trace(&captured, seed).is_err()),
        Err(_) => Ok(true),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    heart: Option<ObjectId>,
    opponent_watchwolf: Option<ObjectId>,
    sacrificed_forest: Option<ObjectId>,
    green_mana_source: Option<ObjectId>,
    heart_payment: Vec<Color>,
    ability_payment: Option<ManaCost>,
    forest_type: Option<BasicLandType>,
    heart_zone: Option<Zone>,
    forest_zone: Option<Zone>,
    life_at_activation: Option<i64>,
    life_after_gain: Option<i64>,
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let (definition, ability, cost) = candidate_values(mutation);
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|registered| registered.id == CARD_ID)
        .ok_or("Dark Heart definition missing from the RAV registry")? = definition;
    let mut abilities = rav_activated_ability_bindings();
    let binding = abilities
        .iter_mut()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("Dark Heart activation missing from the RAV registry")?;
    binding.ability = ability;
    let mut costs = rav_generalized_activated_ability_cost_bindings();
    let cost_binding = costs
        .iter_mut()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability_id == ABILITY_ID)
        .ok_or("Dark Heart generalized cost missing from the RAV registry")?;
    cost_binding.cost = cost;
    let mut game = Game::new_with_all_bindings(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
    )
    .map_err(|error| error.to_string())?;
    game.register_generalized_activated_ability_cost_bindings(costs)
        .map_err(|error| error.to_string())?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    let pilot_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: WATCHWOLF_ID.to_owned(),
                count: 8,
            },
            DeckEntry {
                card: FOREST_ID.to_owned(),
                count: 16,
            },
            DeckEntry {
                card: "RAV-SWAMP".to_owned(),
                count: 16,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 16,
            },
        ],
        sideboard: vec![],
    };
    let opponent_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: WATCHWOLF_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: FOREST_ID.to_owned(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 28,
            },
        ],
        sideboard: vec![],
    };
    for (label, deck) in [("pilot", &pilot_deck), ("opponent", &opponent_deck)] {
        let size: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if size != 60 {
            return Err(format!(
                "Dark Heart {label} deck has {size} cards, expected 60"
            ));
        }
    }
    game.load_deck_into_library(PlayerId(0), &pilot_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent_deck)
        .map_err(|error| error.to_string())?;
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_POLICY_MOVES,
        ..DeckMatchConfig::default()
    };
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let (_, ability_cost, _) = candidate_values(mutation);
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(DarkHeartPolicy::new(
            PlayerId(0),
            ability_cost.mana_cost.clone(),
        )),
        Box::new(DarkHeartPolicy::new(PlayerId(1), ManaCost::new(0))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut life_at_activation = None;
    let mut life_after_gain = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Dark Heart match exceeded its bounded turn or action count".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Dark Heart policy seat missing")?;
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
        let before = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Dark Heart policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        let new_events = &game.event_log[before..];
        if new_events.iter().any(|event| {
            matches!(
                event,
                GameEvent::AbilityActivated {
                    player: PlayerId(0),
                    source: _,
                    definition: CARD_ID,
                    ability: ABILITY_ID,
                    ..
                }
            )
        }) && life_at_activation.is_none()
        {
            life_at_activation = Some(
                game.player(PlayerId(0))
                    .map_err(|error| error.to_string())?
                    .life,
            );
        }
        if new_events.iter().any(|event| {
            matches!(
                event,
                GameEvent::LifeGained {
                    player: PlayerId(0),
                    amount: 3
                }
            ) || matches!(
                event,
                GameEvent::LifeGained {
                    player: PlayerId(0),
                    amount: 2
                }
            )
        }) {
            life_after_gain = Some(
                game.player(PlayerId(0))
                    .map_err(|error| error.to_string())?
                    .life,
            );
        }
        game.validate_invariants()
            .map_err(|error| format!("Dark Heart invariant failed: {error}"))?;
    }
    let events = game.event_log.clone();
    let heart = events.iter().find_map(|event| match event {
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
    let opponent_watchwolf = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(1),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|definition| definition.id == WATCHWOLF_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let sacrificed_forest = heart.and_then(|heart| {
        events.iter().find_map(|event| match event {
            GameEvent::SacrificedAsAbilityCost {
                player: PlayerId(0),
                source,
                permanent,
            } if *source == heart => Some(*permanent),
            _ => None,
        })
    });
    let green_mana_source = heart.and_then(|heart| {
        let paid = events.iter().position(|event| matches!(event,
            GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: ABILITY_ID, .. } if *source == heart
        ))?;
        events.iter().enumerate().take(paid).rev().find_map(|(_, event)| match event {
            GameEvent::ManaAbilityActivated { player: PlayerId(0), land, color: Color::Green } => Some(*land),
            _ => None,
        })
    });
    let heart_payment = heart
        .and_then(|heart| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if *card == heart => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    let ability_payment = heart.and_then(|heart| {
        events.iter().find_map(|event| match event {
            GameEvent::AbilityManaPaid {
                player: PlayerId(0),
                source,
                ability: ABILITY_ID,
                mana_cost,
            } if *source == heart => Some(mana_cost.clone()),
            _ => None,
        })
    });
    let forest_type =
        sacrificed_forest.and_then(|forest| game.basic_land_type(forest).ok().flatten());
    Ok(Captured {
        events,
        terminal: game.winner().map_or_else(
            || "Draw".to_owned(),
            |player| format!("Winner(PlayerId({}))", player.0),
        ),
        winner: game.winner().map(|player| player.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        heart,
        opponent_watchwolf,
        sacrificed_forest,
        green_mana_source,
        heart_payment,
        ability_payment,
        forest_type,
        heart_zone: heart.and_then(|card| game.zone_of(card)),
        forest_zone: sacrificed_forest.and_then(|card| game.zone_of(card)),
        life_at_activation,
        life_after_gain,
    })
}

fn new_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
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
        .find_map(|(index, event)| predicate(event).then_some(index))
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))"
        || captured.winner != Some(0)
        || captured.rejected != 0
    {
        return Err("Dark Heart duel did not end in a clean seat-zero terminal win".to_owned());
    }
    let heart = captured.heart.ok_or("seat zero did not cast Dark Heart")?;
    let opponent_watchwolf = captured
        .opponent_watchwolf
        .ok_or("seat one did not cast Watchwolf")?;
    let forest = captured
        .sacrificed_forest
        .ok_or("Dark Heart did not sacrifice a Forest")?;
    let green_source = captured
        .green_mana_source
        .ok_or("Dark Heart activation has no green mana source receipt")?;
    if captured.heart_payment.len() != 2
        || !captured.heart_payment.contains(&Color::Black)
        || !captured.heart_payment.contains(&Color::Green)
        || captured.ability_payment.as_ref() != Some(&ManaCost::with_colors(0, [Color::Green]))
        || captured.forest_type != Some(BasicLandType::Forest)
        || captured.heart_zone != Some(Zone::Battlefield)
        || captured.forest_zone != Some(Zone::Graveyard)
        || captured.life_at_activation != Some(17)
        || captured.life_after_gain != Some(20)
    {
        return Err(format!(
            "Dark Heart cost/effect state differs: cast={:?}, ability={:?}, type={:?}, zones={:?}/{:?}, life={:?}->{:?}",
            captured.heart_payment,
            captured.ability_payment,
            captured.forest_type,
            captured.heart_zone,
            captured.forest_zone,
            captured.life_at_activation,
            captured.life_after_gain
        ));
    }
    let events = &captured.events;
    let heart_paid = new_index(events, |event| matches!(event,
        GameEvent::SpellManaPaid { player: PlayerId(0), card, colors }
            if *card == heart && colors.len() == 2 && colors.contains(&Color::Black) && colors.contains(&Color::Green)
    )).ok_or("exact {B}{G} Dark Heart cast-payment receipt missing")?;
    let heart_cast = index_after(events, heart_paid, |event| {
        matches!(event,
            GameEvent::SpellCast { player: PlayerId(0), card } if *card == heart
        )
    })
    .ok_or("Dark Heart SpellCast receipt missing")?;
    let heart_resolved = index_after(events, heart_cast, |event| {
        matches!(event,
            GameEvent::SpellResolved { card } if *card == heart
        )
    })
    .ok_or("Dark Heart did not resolve")?;
    let heart_entered = index_after(events, heart_resolved, |event| {
        matches!(event,
            GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == heart
        )
    })
    .ok_or("Dark Heart did not enter the battlefield")?;
    let watchwolf_cast = new_index(events, |event| {
        matches!(event,
            GameEvent::SpellCast { player: PlayerId(1), card } if *card == opponent_watchwolf
        )
    })
    .ok_or("opponent Watchwolf SpellCast receipt missing")?;
    let watchwolf_entered = index_after(events, watchwolf_cast, |event| {
        matches!(event,
            GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == opponent_watchwolf
        )
    })
    .ok_or("opponent Watchwolf did not enter the battlefield")?;
    let opponent_attack = new_index(events, |event| matches!(event,
        GameEvent::AttackersDeclared { player: PlayerId(1), attackers } if attackers.contains(&opponent_watchwolf)
    )).ok_or("opponent Watchwolf did not attack")?;
    let incoming_damage = index_after(events, opponent_attack, |event| matches!(event,
        GameEvent::DamageDealtToPlayer { source, player: PlayerId(0), amount: 3 } if *source == opponent_watchwolf
    )).ok_or("opponent Watchwolf did not deal exactly three damage to seat zero")?;
    let mana_activation = index_after(events, incoming_damage, |event| matches!(event,
        GameEvent::ManaAbilityActivated { player: PlayerId(0), land, color: Color::Green } if *land == green_source
    )).ok_or("Dark Heart {G} payment was not produced by the recorded land")?;
    let green_added = index_after(events, mana_activation, |event| {
        matches!(
            event,
            GameEvent::ManaAdded {
                player: PlayerId(0),
                color: Color::Green,
                amount: 1
            }
        )
    })
    .ok_or("Dark Heart green mana output receipt missing")?;
    let ability_paid = index_after(events, green_added, |event| matches!(event,
        GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: ABILITY_ID, mana_cost }
            if *source == heart && *mana_cost == ManaCost::with_colors(0, [Color::Green])
    )).ok_or("exact {G} activated-ability payment receipt missing")?;
    let forest_sacrificed = index_after(events, ability_paid, |event| {
        matches!(event,
            GameEvent::SacrificedAsAbilityCost { player: PlayerId(0), source, permanent }
                if *source == heart && *permanent == forest
        )
    })
    .ok_or("the recorded Forest was not sacrificed as Dark Heart's cost")?;
    let forest_moved = index_after(events, forest_sacrificed, |event| {
        matches!(event,
            GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == forest
        )
    })
    .ok_or("sacrificed Forest did not move to its owner's graveyard")?;
    let activated = index_after(events, forest_moved, |event| matches!(event,
        GameEvent::AbilityActivated { player: PlayerId(0), source, definition: CARD_ID, ability: ABILITY_ID, .. }
            if *source == heart
    )).ok_or("Dark Heart activation did not enter the stack after paying costs")?;
    let resolved = index_after(events, activated, |event| {
        matches!(event,
            GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == heart
        )
    })
    .ok_or("Dark Heart ability did not resolve")?;
    let life_gained = index_after(events, activated, |event| {
        matches!(
            event,
            GameEvent::LifeGained {
                player: PlayerId(0),
                amount: 3
            }
        )
    })
    .ok_or("Dark Heart did not gain exactly three life")?;
    let continued_attack = index_after(events, resolved.max(life_gained), |event| {
        matches!(event,
            GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if !attackers.is_empty()
        )
    })
    .ok_or("seat zero did not continue normal combat after the activation resolved")?;
    let game_ended = index_after(events, continued_attack, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("seat-zero terminal GameEnded receipt missing")?;
    if heart_paid >= heart_cast
        || heart_cast >= heart_resolved
        || heart_resolved >= heart_entered
        || watchwolf_entered >= opponent_attack
        || opponent_attack >= incoming_damage
        || incoming_damage >= mana_activation
        || mana_activation >= green_added
        || green_added >= ability_paid
        || ability_paid >= forest_sacrificed
        || forest_sacrificed >= forest_moved
        || forest_moved >= activated
        || activated >= resolved
        || activated >= life_gained
        || resolved >= continued_attack
        || life_gained >= continued_attack
        || continued_attack >= game_ended
    {
        return Err("Dark Heart payment, sacrifice, resolution, life gain, combat, and terminal events are out of order".to_owned());
    }
    let mut selected = vec![
        heart_paid,
        heart_cast,
        heart_resolved,
        heart_entered,
        watchwolf_cast,
        watchwolf_entered,
        opponent_attack,
        incoming_damage,
        mana_activation,
        green_added,
        ability_paid,
        forest_sacrificed,
        forest_moved,
        activated,
        resolved,
        life_gained,
        continued_attack,
        game_ended,
    ];
    selected.sort_unstable();
    let projected = project_selected_events(events, &selected);
    Ok(json!({
        "id": "rav-dark-heart-forest-sacrifice-three-life-pvp",
        "seed": seed,
        "decks": ["rav-dark-heart-golgari-forest-life", "rav-watchwolf-selesnya-pressure"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted,
        "rejected_moves": captured.rejected,
        "events_digest": event_digest(events),
        "mechanics": {
            "heart_object": heart.0,
            "heart_mana_cost": "{B}{G}",
            "heart_mana_paid": captured.heart_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "opponent_watchwolf": opponent_watchwolf.0,
            "incoming_combat_damage": 3,
            "ability_id": ABILITY_ID,
            "activation_mana_cost": "{G}",
            "activation_mana_paid": captured.ability_payment.as_ref().map(|_| "{G}"),
            "green_mana_source": green_source.0,
            "sacrificed_permanent": forest.0,
            "sacrificed_basic_land_type": captured.forest_type.map(|land_type| format!("{land_type:?}")),
            "sacrifice_moved_to_graveyard": true,
            "life_at_activation": captured.life_at_activation,
            "life_after_resolution": captured.life_after_gain,
            "life_gained": 3,
            "ordered_event_sequence": selected,
        },
        "events": projected,
        "passed": true
    }))
}

fn project_selected_events(events: &[GameEvent], indices: &[usize]) -> Vec<Value> {
    let projected = project_events(events);
    indices.iter().filter_map(|index| projected.get(*index)).map(|event: &ProjectedEvent| {
        json!({"sequence":event.sequence,"turn":event.turn,"kind":event.kind,"facts":event.facts,"canonical":event.canonical})
    }).collect()
}
fn event_digest(events: &[GameEvent]) -> String {
    let bytes = events
        .iter()
        .map(|event| format!("{event:?}\n"))
        .collect::<String>();
    let hash = bytes.bytes().fold(0xcbf29ce484222325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
    });
    format!("fnv1a64:{hash:016x}")
}

struct DarkHeartPolicy {
    player: PlayerId,
    activation_cost: ManaCost,
    heart_cast: bool,
    activation_offered: bool,
    watchwolves_cast: u8,
    opponent_cast: bool,
    opponent_attacked: bool,
}
impl DarkHeartPolicy {
    fn new(player: PlayerId, activation_cost: ManaCost) -> Self {
        Self {
            player,
            activation_cost,
            heart_cast: false,
            activation_offered: false,
            watchwolves_cast: 0,
            opponent_cast: false,
            opponent_attacked: false,
        }
    }
    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let definitions: BTreeSet<_> = view
            .own_battlefield
            .iter()
            .filter_map(|card| card.definition)
            .collect();
        let preferred = if self.player == PlayerId(0) {
            if !definitions.contains(FOREST_ID) {
                FOREST_ID
            } else if !definitions.contains("RAV-SWAMP") {
                "RAV-SWAMP"
            } else if !definitions.contains("RAV-PLAINS") {
                "RAV-PLAINS"
            } else {
                FOREST_ID
            }
        } else if !definitions.contains(FOREST_ID) {
            FOREST_ID
        } else if !definitions.contains("RAV-PLAINS") {
            "RAV-PLAINS"
        } else {
            FOREST_ID
        };
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(preferred))
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            })?;
        Some(PolicyAction::PlayLand { card: card.id })
    }
    fn tap_land(&self, view: &GameView, color: Option<Color>) -> Option<PolicyAction> {
        let land = view.own_battlefield.iter().find(|card| {
            !card.tapped
                && card.card_types.contains(&CardType::Land)
                && color.is_none_or(|color| card.mana_colors.contains(&color))
        })?;
        let color = color.or_else(|| land.mana_colors.iter().next().copied())?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }
    fn payment(&self, view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        if !cost.hybrid.is_empty() {
            return None;
        }
        let mut available: BTreeMap<Color, u8> = Color::ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        for color in &cost.colored {
            let amount = available.get_mut(color)?;
            if *amount == 0 {
                return None;
            }
            *amount -= 1;
        }
        let mut generic = Vec::new();
        for _ in 0..cost.generic {
            let color = available
                .iter()
                .filter(|(_, amount)| **amount > 0)
                .max_by_key(|(_, amount)| **amount)
                .map(|(color, _)| *color)?;
            *available.get_mut(&color)? -= 1;
            generic.push(color);
        }
        Some(ManaPaymentSelection {
            generic,
            hybrid: vec![],
        })
    }
    fn cast(&mut self, view: &GameView, id: &str, cost: &ManaCost) -> Option<PolicyAction> {
        let card = view.hand.iter().find(|card| card.definition == Some(id))?;
        if let Some(mana_selection) = self.payment(view, cost) {
            match (self.player, id) {
                (PlayerId(0), CARD_ID) => self.heart_cast = true,
                (PlayerId(0), WATCHWOLF_ID) => {
                    self.watchwolves_cast = self.watchwolves_cast.saturating_add(1)
                }
                (PlayerId(1), WATCHWOLF_ID) => self.opponent_cast = true,
                _ => {}
            }
            return Some(PolicyAction::CastWithPayment {
                request: CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection,
            });
        }
        let required = cost.generic as usize + cost.colored.len() + cost.hybrid.len();
        let missing = cost
            .colored
            .iter()
            .find(|color| view.mana_pool.amount(**color) == 0)
            .copied();
        if let Some(color) = missing {
            return self.tap_land(view, Some(color));
        }
        if usize::from(view.mana_pool.total()) < required {
            return self.tap_land(view, None);
        }
        None
    }
    fn activate_heart(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.player != PlayerId(0) || self.activation_offered || view.own_life > 17 {
            return None;
        }
        let heart = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some(CARD_ID))?;
        let forest = view
            .own_battlefield
            .iter()
            .find(|card| card.basic_land_type == Some(BasicLandType::Forest))?;
        if let Some(color) = self.activation_cost.colored.first().copied()
            && view.mana_pool.amount(color) == 0
        {
            return self.tap_land(view, Some(color));
        }
        self.activation_offered = true;
        Some(PolicyAction::ActivateAbility {
            activation: AbilityActivation {
                source: heart.id,
                ability_id: ABILITY_ID,
                sacrifice_sources: vec![forest.id],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![],
            },
        })
    }
}
impl CodePolicy for DarkHeartPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.dark-heart-forest-sacrifice-life-win.v1"
        } else {
            "rav.watchwolf-one-hit-pressure.v1"
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
            let attackers: Vec<_> = if self.player == PlayerId(0) {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.can_attack)
                    .map(|card| card.id)
                    .collect()
            } else if !self.opponent_attacked {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(WATCHWOLF_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect()
            } else {
                vec![]
            };
            if self.player == PlayerId(1) && !attackers.is_empty() {
                self.opponent_attacked = true;
            }
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
        if view.priority != self.player
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.play_land(view) {
            return action;
        }
        if self.player == PlayerId(0) {
            let has_heart = view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(CARD_ID));
            if !has_heart {
                if let Some(action) =
                    self.cast(view, CARD_ID, &candidate_dark_heart::definition().mana_cost)
                {
                    return action;
                }
            }
            if let Some(action) = self.activate_heart(view) {
                return action;
            }
            let has_heart = view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(CARD_ID));
            if has_heart
                && self.watchwolves_cast < 4
                && let Some(action) = self.cast(
                    view,
                    WATCHWOLF_ID,
                    &ManaCost::with_colors(0, [Color::Green, Color::White]),
                )
            {
                return action;
            }
        } else if !self.opponent_cast
            && let Some(action) = self.cast(
                view,
                WATCHWOLF_ID,
                &ManaCost::with_colors(0, [Color::Green, Color::White]),
            )
        {
            return action;
        }
        PolicyAction::PassPriority
    }
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 200;
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,
        "engine_status":if definition_id.is_some(){"executable_compatibility_slice"}else{"catalog_only"},
        "trace_status":if passed{"passed"}else{"not_qualified"},
        "qualification":if passed{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
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
        .find(|args| args[0] == "--output-root")
        .map(|args| args[1].clone().into())
}
fn write_outputs(root: &std::path::Path, report: &Value) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("result.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(report).map_err(|error| error.to_string())?
        ),
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(root.join("reward.txt"), "0.00000000\n").map_err(|error| error.to_string())
}
