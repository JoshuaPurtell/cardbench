//! Full-game PvP authority for the RAV Surveilling Sprite.
//!
//! A shuffled two-player game casts the Sprite, attacks over a live ground
//! blocker using Flying, then has Char kill it. Its optional dies trigger is
//! exercised once with the draw accepted and once declined.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView,
    Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target,
    TriggerCondition, TriggeredAbilityBinding, Zone,
};
use cardbench_magic_policies::{
    CatalogPolicyProfile, CodePolicy, DeckMatchConfig, RavCatalogPolicy,
};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_activated_ability_cost_modifier_bindings, rav_additional_spell_cost_bindings,
    rav_attachment_bindings,
    rav_basic_land_type_bindings, rav_cost_reduction_bindings,
    rav_damage_replacement_effect_bindings, rav_entry_coin_flip_bindings, rav_entry_copy_bindings,
    rav_generalized_activated_ability_cost_bindings, rav_land_entry_bindings,
    rav_legendary_permanent_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_mana_ability_cost_bindings, rav_replacement_effect_bindings,
    rav_static_attack_restriction_bindings, rav_static_continuous_effect_bindings,
    rav_static_creature_spell_cost_modifier_bindings, rav_static_entry_restriction_bindings,
    rav_static_library_top_reveal_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, Type, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-SURVEILLING-SPRITE";
const CARD_NAME: &str = "Surveilling Sprite";
const ABILITY_ID: &str = "dies-draw-controller";
const CHAR_ID: &str = "RAV-CHAR";
const INFORMANT_ID: &str = "RAV-LURKING-INFORMANT";
const DRAKE_ID: &str = "RAV-SNAPPING-DRAKE";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/surveilling_sprite.rs";
const MAX_TRACE_SEED: u64 = 128;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/surveilling_sprite.rs"]
mod candidate_surveilling_sprite;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Surveilling Sprite result: {error}");
                    return ExitCode::FAILURE;
                }
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
            eprintln!("Surveilling Sprite PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_definition()?;
    validate_candidate_trigger()?;
    let catalog = unique_catalog()?;
    let (seed, accepted, declined) = find_traces()?;
    let accepted_trace = validate_trace(&accepted, seed, true)?;
    let declined_trace = validate_trace(&declined, seed, false)?;
    for (capture, accept) in [(&accepted, true), (&declined, false)] {
        let replay = run_match(seed, Mutation::None, accept)?;
        if capture.events != replay.events
            || capture.winner != replay.winner
            || capture.sprite != replay.sprite
            || capture.drawn_card != replay.drawn_card
        {
            return Err("Surveilling Sprite shuffled-game replay was not deterministic".to_owned());
        }
    }

    let mutation_sensitivity = json!({
        "flying_removal_changes_live_blocking_outcome": mutation_detected(seed, Mutation::RemoveFlying),
        "cost_mutation_detected_in_game": mutation_detected(seed, Mutation::GenericCost),
        "power_mutation_detected_in_game": mutation_detected(seed, Mutation::Power),
        "dies_trigger_removal_detected_in_game": mutation_detected(seed, Mutation::RemoveTrigger),
        "draw_effect_removal_detected_in_game": mutation_detected(seed, Mutation::NoDraw),
        "mandatory_trigger_mutation_detected_in_game": mutation_detected(seed, Mutation::MandatoryTrigger),
        "sealed_mutation_authority": false
    });
    for key in [
        "flying_removal_changes_live_blocking_outcome",
        "cost_mutation_detected_in_game",
        "power_mutation_detected_in_game",
        "dies_trigger_removal_detected_in_game",
        "draw_effect_removal_detected_in_game",
        "mandatory_trigger_mutation_detected_in_game",
    ] {
        if mutation_sensitivity[key] != true {
            return Err(format!(
                "Surveilling Sprite mutation escaped the trace oracle: {key}"
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
        "card": {"collector_number": 67, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117 Priority",
                "302.6 Summoning sickness",
                "510 Combat damage",
                "601.2 Casting spells and paying costs",
                "603.2 and 603.6c Dies-trigger events",
                "603.5 Optional triggered abilities",
                "608.2 Resolving a triggered ability",
                "702.9 Flying"
            ],
            "source_contract": "RAV Surveilling Sprite #67, {1}{U} blue 1/1 creature with Flying and an optional dies trigger that draws its controller a card",
            "authority_cutoff": "2026-09-23",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "deterministic_replay": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": rav_main_set_catalog().len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "reference_pilot_pass; independent review and candidate trial remain",
        "mutation_sensitivity": mutation_sensitivity,
        "traces": [accepted_trace, declined_trace],
        "per_card_breakdown": breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Surveilling Sprite source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Surveilling Sprite source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(|attr| attr.path().is_ident("doc")) || file.items.len() != 4 {
        return Err(
            "Surveilling Sprite source must contain only two literal functions and their imports"
                .to_owned(),
        );
    }
    let mut imports = 0;
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if matches!(item.vis, Visibility::Inherited) && item.attrs.is_empty() =>
            {
                imports += 1
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                let expected_return = if name == "definition" {
                    "CardDefinition"
                } else if name == "triggered_ability" {
                    "TriggeredAbilityBinding"
                } else {
                    ""
                };
                let crate_visible = matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"));
                let body_is_literal = function.block.stmts.len() == 1
                    && matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(_), None));
                if expected_return.is_empty()
                    || !functions.insert(name.clone())
                    || !crate_visible
                    || !function.sig.inputs.is_empty()
                    || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if type_is(ty, expected_return))
                    || !body_is_literal
                {
                    return Err("Surveilling Sprite source functions must return direct declarative literals".to_owned());
                }
            }
            _ => {
                return Err(
                    "Surveilling Sprite source contains unsupported executable top-level code"
                        .to_owned(),
                );
            }
        }
    }
    if imports != 2
        || functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()])
    {
        return Err(
            "Surveilling Sprite source imports or function structure differ from the task contract"
                .to_owned(),
        );
    }
    Ok(())
}

fn type_is(ty: &Type, expected: &str) -> bool {
    matches!(ty, Type::Path(path) if path.qself.is_none() && path.path.segments.last().is_some_and(|segment| segment.ident == expected))
}

fn validate_candidate_definition() -> Result<(), String> {
    let submitted = candidate_surveilling_sprite::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Surveilling Sprite is missing from the RAV registry")?;
    let supported = [
        "full-rules-fidelity",
        "colored-cost-casting",
        "base-characteristics",
        "flying",
        "dies-draw-controller",
    ];
    if submitted.id != registered.id
        || submitted.id != CARD_ID
        || submitted.name != CARD_NAME
        || submitted.set_code != "RAV"
        || submitted.mana_cost != ManaCost::with_colors(1, [Color::Blue])
        || submitted.colors != BTreeSet::from([Color::Blue])
        || !submitted.mana_colors.is_empty()
        || submitted.card_types != BTreeSet::from([CardType::Creature])
        || submitted.is_basic_land
        || submitted.power != Some(1)
        || submitted.toughness != Some(1)
        || submitted.keywords != [Keyword::Flying]
        || !submitted.effects.is_empty()
        || submitted.supported_rules != supported
        || registered.mana_cost != submitted.mana_cost
        || registered.colors != submitted.colors
        || registered.power != submitted.power
        || registered.toughness != submitted.toughness
        || registered.keywords != submitted.keywords
    {
        return Err("Surveilling Sprite identity, cost, or live characteristics differ from the RAV printing".to_owned());
    }
    Ok(())
}

fn validate_candidate_trigger() -> Result<(), String> {
    let submitted = candidate_surveilling_sprite::triggered_ability();
    let registered = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("Surveilling Sprite dies-trigger binding is missing from the RAV registry")?;
    if submitted.card_definition != CARD_ID
        || submitted.ability.id != ABILITY_ID
        || submitted.ability.condition != TriggerCondition::Dies
        || submitted.ability.mana_cost != ManaCost::new(0)
        || !submitted.ability.optional
        || !submitted.ability.targets.is_empty()
        || submitted.ability.effects != [Effect::DrawController]
        || registered.card_definition != submitted.card_definition
        || registered.ability.id != submitted.ability.id
        || registered.ability.condition != submitted.ability.condition
        || registered.ability.optional != submitted.ability.optional
        || registered.ability.effects != submitted.ability.effects
    {
        return Err(
            "Surveilling Sprite must retain its optional, target-free dies draw trigger".to_owned(),
        );
    }
    Ok(())
}

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let cards = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect::<Vec<_>>();
    if cards.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || !cards
            .iter()
            .any(|card| card.collector_number == 67 && card.name == CARD_NAME)
    {
        return Err("RAV card catalog identity or denominator changed".to_owned());
    }
    Ok(cards)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    RemoveFlying,
    GenericCost,
    Power,
    RemoveTrigger,
    NoDraw,
    MandatoryTrigger,
}

fn candidate_values(mutation: Mutation) -> (CardDefinition, TriggeredAbilityBinding) {
    let mut definition = candidate_surveilling_sprite::definition();
    let mut trigger = candidate_surveilling_sprite::triggered_ability();
    match mutation {
        Mutation::RemoveFlying => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Flying),
        Mutation::GenericCost => definition.mana_cost = ManaCost::new(2),
        Mutation::Power => definition.power = Some(2),
        Mutation::NoDraw => {
            trigger.ability.effects = vec![Effect::GainLifeController { amount: 1 }]
        }
        Mutation::MandatoryTrigger => trigger.ability.optional = false,
        Mutation::None | Mutation::RemoveTrigger => {}
    }
    (definition, trigger)
}

fn find_traces() -> Result<(u64, CapturedMatch, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_SURVEILLING_SPRITE_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_SURVEILLING_SPRITE_SEED must be unsigned".to_owned())?;
        let accepted = run_match(seed, Mutation::None, true)?;
        validate_trace(&accepted, seed, true)?;
        let declined = run_match(seed, Mutation::None, false)?;
        validate_trace(&declined, seed, false)?;
        return Ok((seed, accepted, declined));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let (Ok(accepted), Ok(declined)) = (
            run_match(seed, Mutation::None, true),
            run_match(seed, Mutation::None, false),
        ) && validate_trace(&accepted, seed, true).is_ok()
            && validate_trace(&declined, seed, false).is_ok()
        {
            return Ok((seed, accepted, declined));
        }
    }
    Err(format!(
        "no accepted and declined Surveilling Sprite traces found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    if mutation == Mutation::RemoveFlying {
        return run_match(seed, mutation, true).is_ok_and(|capture| {
            capture.blocker_ready
                && capture.blocked_sprite
                && capture
                    .sprite_keywords
                    .as_ref()
                    .is_some_and(|keywords| !keywords.contains(&Keyword::Flying))
        });
    }
    run_match(seed, mutation, true).is_ok_and(|capture| {
        let validation_fails = validate_trace(&capture, seed, true).is_err();
        let relevant_path_reached = match mutation {
            Mutation::RemoveTrigger => capture.sprite.is_some_and(|sprite| {
                capture.events.iter().any(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == sprite))
                    && !capture.events.iter().any(|event| matches!(event, GameEvent::TriggeredAbilityStacked { source, ability: ABILITY_ID, .. } if *source == sprite))
            }),
            Mutation::NoDraw => capture.optional_choice_seen && capture.ability_resolved && capture.drawn_card.is_none(),
            Mutation::MandatoryTrigger => !capture.optional_choice_seen && capture.trigger_resolved,
            Mutation::GenericCost => capture.sprite.is_some() && capture.sprite_cost.as_ref() == Some(&ManaCost::new(2)),
            Mutation::Power => capture.sprite_pt == Some((2, 1)),
            Mutation::None | Mutation::RemoveFlying => false,
        };
        validation_fails && relevant_path_reached
    })
}

#[derive(Clone, Debug)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    sprite: Option<ObjectId>,
    char_spell: Option<ObjectId>,
    blocker: Option<ObjectId>,
    flying_evasion_attack_index: Option<usize>,
    flying_evasion_attacker: Option<ObjectId>,
    drawn_card: Option<ObjectId>,
    sprite_cost: Option<ManaCost>,
    sprite_payment: Vec<Color>,
    char_payment: Vec<Color>,
    sprite_pt: Option<(i32, i32)>,
    sprite_keywords: Option<Vec<Keyword>>,
    blocker_ready: bool,
    flying_evasion_observed: bool,
    blocked_sprite: bool,
    optional_choice_seen: bool,
    optional_can_pay: Option<bool>,
    optional_paid: bool,
    source_in_graveyard_before_choice: Option<bool>,
    trigger_resolved: bool,
    ability_resolved: bool,
    seat_zero_life: i64,
    seat_one_life: i64,
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

fn run_match(seed: u64, mutation: Mutation, accept_draw: bool) -> Result<CapturedMatch, String> {
    let (definition, ability) = candidate_values(mutation);
    let sprite_cost = definition.mana_cost.clone();
    let pilot = deck(&[(CARD_ID, 4), (DRAKE_ID, 4), ("RAV-ISLAND", 52)]);
    let opponent = deck(&[
        (CHAR_ID, 4),
        (INFORMANT_ID, 4),
        ("RAV-ISLAND", 18),
        ("RAV-SWAMP", 18),
        ("RAV-MOUNTAIN", 16),
    ]);
    for (name, list) in [("pilot", &pilot), ("opponent", &opponent)] {
        if list
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
        {
            return Err(format!(
                "{name} Surveilling Sprite task deck is not a sixty-card list"
            ));
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    let mut game = new_game(definition, ability, mutation == Mutation::RemoveTrigger)?;
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
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(SpritePolicy::new(
            PlayerId(0),
            sprite_cost.clone(),
            accept_draw,
        )),
        Box::new(SpritePolicy::new(
            PlayerId(1),
            sprite_cost.clone(),
            accept_draw,
        )),
    ];
    let mut attempted = 0_u32;
    let mut accepted = 0_u32;
    let mut sprite = None;
    let mut char_spell = None;
    let mut blocker = None;
    let mut flying_evasion_attack_index = None;
    let mut flying_evasion_attacker = None;
    let mut drawn_card = None;
    let mut sprite_pt = None;
    let mut sprite_keywords = None;
    let mut blocker_ready = false;
    let mut flying_evasion_observed = false;
    let mut blocked_sprite = false;
    let mut optional_choice_seen = false;
    let mut optional_can_pay = None;
    let mut optional_paid = false;
    let mut source_in_graveyard_before_choice = None;
    let mut trigger_resolved = false;
    let mut ability_resolved = false;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Surveilling Sprite game hit its development run bound".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Surveilling Sprite policy seat missing")?;
        let choice = view.optional_triggered_ability_choice.clone();
        if let Some(choice) = &choice
            && choice.ability == ABILITY_ID
            && (sprite == Some(choice.source)
                || game
                    .card_definition(choice.source)
                    .is_ok_and(|card| card.id == CARD_ID))
        {
            optional_choice_seen = true;
            optional_can_pay = Some(choice.can_pay);
            source_in_graveyard_before_choice =
                Some(game.zone_of(choice.source) == Some(Zone::Graveyard));
        }
        if player == PlayerId(1) && view.step == Step::DeclareBlockers && !view.blockers_declared {
            if let Some(attacker) = view
                .combat_attackers
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
                && let Some(ground_blocker) = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.definition == Some(INFORMANT_ID) && card.can_block)
            {
                blocker = Some(ground_blocker.id);
                blocker_ready = true;
                flying_evasion_observed |= attacker.keywords.contains(&Keyword::Flying);
                if attacker.keywords.contains(&Keyword::Flying) {
                    flying_evasion_attacker = Some(attacker.id);
                    flying_evasion_attack_index = game.event_log.iter().rposition(|event| {
                        matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&attacker.id))
                    });
                }
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
        if matches!(&action, PolicyAction::ResolveOptionalTriggeredAbility { source, ability: ABILITY_ID, pay: true, .. } if sprite == Some(*source) || game.card_definition(*source).is_ok_and(|card| card.id == CARD_ID))
        {
            optional_paid = true;
        }
        if player == PlayerId(1)
            && view.step == Step::DeclareBlockers
            && matches!(&action, PolicyAction::DeclareBlockers { assignments } if assignments.iter().any(|block| Some(block.attacker) == sprite && Some(block.blocker) == blocker))
        {
            blocked_sprite = true;
        }
        let before = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action.clone())
            .map_err(|error| {
                format!(
                    "policy move rejected at turn {} / {:?} by {:?}: {:?}: {error}",
                    game.turn, view.step, player, action
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        let new_events = &game.event_log[before..];
        for event in new_events {
            match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|known| known.id == CARD_ID) =>
                {
                    sprite.get_or_insert(*card);
                }
                GameEvent::SpellCast {
                    player: PlayerId(1),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|known| known.id == CHAR_ID) =>
                {
                    char_spell.get_or_insert(*card);
                }
                GameEvent::SpellResolved { card } if Some(*card) == sprite => {
                    let characteristics = game
                        .characteristics(*card)
                        .map_err(|error| error.to_string())?;
                    sprite_pt = Some((
                        characteristics
                            .power
                            .ok_or("Surveilling Sprite has no live power")?,
                        characteristics
                            .toughness
                            .ok_or("Surveilling Sprite has no live toughness")?,
                    ));
                    sprite_keywords = Some(characteristics.keywords);
                }
                GameEvent::TriggeredAbilityStacked {
                    source,
                    ability: ABILITY_ID,
                    ..
                } if Some(*source) == sprite => trigger_resolved = true,
                GameEvent::AbilityResolved {
                    source,
                    ability: ABILITY_ID,
                    ..
                } if Some(*source) == sprite => ability_resolved = true,
                GameEvent::CardMoved {
                    card,
                    to: Zone::Hand,
                } if choice
                    .as_ref()
                    .is_some_and(|choice| choice.ability == ABILITY_ID)
                    && player == PlayerId(0) =>
                {
                    if accept_draw
                        && game
                            .view_for_player(PlayerId(0))
                            .is_ok_and(|own| own.hand.iter().any(|view| view.id == *card))
                    {
                        drawn_card.get_or_insert(*card);
                    }
                }
                _ => {}
            }
        }
    }
    let events = game.event_log.clone();
    let sprite = sprite.or_else(|| {
        events.iter().find_map(|event| match event {
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
        })
    });
    let char_spell = char_spell.or_else(|| {
        events.iter().find_map(|event| match event {
            GameEvent::SpellCast {
                player: PlayerId(1),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CHAR_ID) =>
            {
                Some(*card)
            }
            _ => None,
        })
    });
    let mut sprite_payment = Vec::new();
    let mut char_payment = Vec::new();
    let mut sprite_cost_seen = None;
    for event in &events {
        if let GameEvent::SpellManaPaid {
            player: PlayerId(0),
            card,
            colors,
        } = event
            && Some(*card) == sprite
        {
            sprite_payment = colors.clone();
        }
        if let GameEvent::SpellManaPaid {
            player: PlayerId(1),
            card,
            colors,
        } = event
            && Some(*card) == char_spell
        {
            char_payment = colors.clone();
        }
        if let Some(card) = sprite
            && matches!(event, GameEvent::SpellCast { player: PlayerId(0), card: cast } if *cast == card)
        {
            sprite_cost_seen = game
                .card_definition(card)
                .ok()
                .map(|definition| definition.mana_cost.clone());
        }
    }
    let players = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    let winner = game.winner().map(|player| player.0);
    Ok(CapturedMatch {
        events,
        terminal: winner.map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({seat}))"),
        ),
        winner,
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempted.saturating_sub(accepted),
        sprite,
        char_spell,
        blocker,
        flying_evasion_attack_index,
        flying_evasion_attacker,
        drawn_card,
        sprite_cost: sprite_cost_seen.or(Some(sprite_cost)),
        sprite_payment,
        char_payment,
        sprite_pt,
        sprite_keywords,
        blocker_ready,
        flying_evasion_observed,
        blocked_sprite,
        optional_choice_seen,
        optional_can_pay,
        optional_paid,
        source_in_graveyard_before_choice,
        trigger_resolved,
        ability_resolved,
        seat_zero_life: players[0],
        seat_one_life: players[1],
    })
}

fn new_game(
    definition: CardDefinition,
    ability: TriggeredAbilityBinding,
    remove_trigger: bool,
) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Surveilling Sprite is missing from card definitions")? = definition;
    // This duel uses no other triggered permanents. Loading the target card's
    // binding alone keeps unrelated RAV trigger implementations out of its
    // task authority while still testing the exact candidate-owned trigger.
    let triggers = if remove_trigger { Vec::new() } else { vec![ability] };
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers,
        rav_static_continuous_effect_bindings(),
        rav_land_entry_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_static_library_top_reveal_bindings(rav_static_library_top_reveal_bindings())
        .map_err(|error| error.to_string())?;
    game.register_static_attack_restrictions(rav_static_attack_restriction_bindings())
        .map_err(|error| error.to_string())?;
    game.register_static_creature_spell_cost_modifiers(
        rav_static_creature_spell_cost_modifier_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_attachment_bindings(rav_attachment_bindings())
        .map_err(|error| error.to_string())?;
    game.register_entry_copy_bindings(rav_entry_copy_bindings())
        .map_err(|error| error.to_string())?;
    game.register_entry_coin_flip_bindings(rav_entry_coin_flip_bindings())
        .map_err(|error| error.to_string())?;
    game.register_legendary_permanent_bindings(rav_legendary_permanent_bindings())
        .map_err(|error| error.to_string())?;
    game.register_static_entry_restriction_bindings(rav_static_entry_restriction_bindings())
        .map_err(|error| error.to_string())?;
    game.register_mana_ability_cost_bindings(rav_mana_ability_cost_bindings())
        .map_err(|error| error.to_string())?;
    game.register_cost_reduction_bindings(rav_cost_reduction_bindings())
        .map_err(|error| error.to_string())?;
    game.register_activated_ability_cost_modifier_bindings(
        rav_activated_ability_cost_modifier_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_generalized_activated_ability_cost_bindings(
        rav_generalized_activated_ability_cost_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_damage_replacement_effect_bindings(rav_damage_replacement_effect_bindings())
        .map_err(|error| error.to_string())?;
    game.register_replacement_effect_bindings(rav_replacement_effect_bindings())
        .map_err(|error| error.to_string())?;
    game.validate_invariants()
        .map_err(|error| error.to_string())?;
    Ok(game)
}

struct SpritePolicy {
    player: PlayerId,
    accept_draw: bool,
    sprite_cost: ManaCost,
    inner: RavCatalogPolicy,
    char_used: bool,
    sprite_attacked_past_informant: bool,
}

impl SpritePolicy {
    fn new(player: PlayerId, sprite_cost: ManaCost, accept_draw: bool) -> Self {
        Self {
            player,
            accept_draw,
            sprite_cost,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
            char_used: false,
            sprite_attacked_past_informant: false,
        }
    }

    fn play_land(&mut self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let hand_has_informant = view
            .hand
            .iter()
            .any(|card| card.definition == Some(INFORMANT_ID));
        let hand_has_char = view
            .hand
            .iter()
            .any(|card| card.definition == Some(CHAR_ID));
        let has_color = |color: Color| {
            view.own_battlefield
                .iter()
                .any(|card| card.mana_colors.contains(&color))
        };
        let priorities = if self.player == PlayerId(0) {
            vec![Color::Blue]
        } else {
            let mut colors = Vec::new();
            if hand_has_informant && !has_color(Color::Blue) {
                colors.push(Color::Blue);
            }
            if hand_has_informant && !has_color(Color::Black) {
                colors.push(Color::Black);
            }
            if hand_has_char && !has_color(Color::Red) {
                colors.push(Color::Red);
            }
            colors
        };
        for color in priorities {
            if let Some(land) = view.hand.iter().find(|card| {
                card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&color)
            }) {
                return Some(PolicyAction::PlayLand { card: land.id });
            }
        }
        let action = self.inner.propose_move(view);
        matches!(
            action,
            PolicyAction::PlayLand { .. } | PolicyAction::PlayLandWithEntryLifePayment { .. }
        )
        .then_some(action)
    }

    fn activate_land(&self, view: &GameView, color: Option<Color>) -> Option<PolicyAction> {
        view.own_battlefield
            .iter()
            .find(|land| {
                !land.tapped
                    && land.card_types.contains(&CardType::Land)
                    && color.is_none_or(|color| land.mana_colors.contains(&color))
            })
            .map(|land| {
                let chosen = color
                    .or_else(|| land.mana_colors.iter().next().copied())
                    .expect("mana-producing land has a color");
                PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: chosen,
                }
            })
    }

    fn cast_blue(&self, view: &GameView, card: ObjectId, cost: &ManaCost) -> Option<PolicyAction> {
        let total = cost.mana_value();
        if view.mana_pool.amount(Color::Blue) >= total && view.mana_pool.total() >= total {
            return Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
                    card,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection: ManaPaymentSelection {
                    generic: vec![Color::Blue; usize::from(cost.generic)],
                    hybrid: vec![],
                },
            });
        }
        let missing_blue = view.mana_pool.amount(Color::Blue) < total;
        self.activate_land(view, missing_blue.then_some(Color::Blue))
            .or(Some(PolicyAction::PassPriority))
    }

    fn cast_char(&mut self, view: &GameView, sprite: ObjectId) -> Option<PolicyAction> {
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(CHAR_ID))?;
        let cost = ManaCost::with_colors(2, [Color::Red]);
        if view.mana_pool.amount(Color::Red) >= 1 && view.mana_pool.total() >= 3 {
            let mut generic = Vec::new();
            let mut available: Vec<(Color, u8)> = Color::MANA_ALL
                .into_iter()
                .map(|color| (color, view.mana_pool.amount(color)))
                .collect();
            let red = available
                .iter_mut()
                .find(|(color, _)| *color == Color::Red)
                .expect("red is a mana color");
            if red.1 == 0 {
                return self.activate_land(view, Some(Color::Red));
            }
            red.1 -= 1;
            for _ in 0..usize::from(cost.generic) {
                let (color, amount) = available.iter_mut().find(|(_, amount)| *amount > 0)?;
                *amount -= 1;
                generic.push(*color);
            }
            self.char_used = true;
            return Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
                    card: card.id,
                    targets: vec![Target::Permanent(sprite)],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection: ManaPaymentSelection {
                    generic,
                    hybrid: vec![],
                },
            });
        }
        if view.mana_pool.amount(Color::Red) == 0 {
            self.activate_land(view, Some(Color::Red))
        } else {
            self.activate_land(view, None)
        }
    }

    fn cast_informant(&self, view: &GameView) -> Option<PolicyAction> {
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(INFORMANT_ID))?;
        if view.mana_pool.amount(Color::Blue) > 0
            && view.mana_pool.amount(Color::Black) > 0
            && view.mana_pool.total() >= 2
        {
            return Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection: ManaPaymentSelection {
                    generic: vec![Color::Blue],
                    hybrid: vec![Color::Black],
                },
            });
        }
        if view.mana_pool.amount(Color::Blue) == 0 {
            self.activate_land(view, Some(Color::Blue))
        } else if view.mana_pool.amount(Color::Black) == 0 {
            self.activate_land(view, Some(Color::Black))
        } else {
            self.activate_land(view, None)
        }
    }
}

impl CodePolicy for SpritePolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.surveilling-sprite-flight-and-draw.v1"
        } else {
            "rav.char-ground-blocker-opponent.v1"
        }
    }

    fn propose_optional_triggered_ability(&mut self, view: &GameView) -> PolicyAction {
        let choice = view
            .optional_triggered_ability_choice
            .as_ref()
            .expect("visible optional-trigger choice");
        PolicyAction::ResolveOptionalTriggeredAbility {
            decision: choice.decision,
            source: choice.source,
            ability: choice.ability,
            pay: self.accept_draw,
            target: None,
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
            let attackers = if self.player == PlayerId(0) {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.can_attack && card.card_types.contains(&CardType::Creature))
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
            if self.player == PlayerId(1)
                && view.combat_attackers.iter().any(|attacker| {
                    attacker.definition == Some(CARD_ID)
                        && attacker.keywords.contains(&Keyword::Flying)
                })
                && view
                    .own_battlefield
                    .iter()
                    .any(|blocker| blocker.definition == Some(INFORMANT_ID) && blocker.can_block)
            {
                self.sprite_attacked_past_informant = true;
            }
            let assignments = if self.player == PlayerId(1) {
                view.combat_attackers
                    .iter()
                    .filter(|attacker| {
                        attacker.definition == Some(CARD_ID)
                            && !attacker.keywords.contains(&Keyword::Flying)
                    })
                    .find_map(|attacker| {
                        view.own_battlefield
                            .iter()
                            .find(|blocker| {
                                blocker.definition == Some(INFORMANT_ID) && blocker.can_block
                            })
                            .map(|blocker| cardbench_magic_engine::CombatBlock {
                                attacker: attacker.id,
                                blocker: blocker.id,
                            })
                    })
                    .into_iter()
                    .collect()
            } else {
                vec![]
            };
            return PolicyAction::DeclareBlockers { assignments };
        }
        if view.active_player != self.player
            || view.priority != self.player
            || view.stack_depth != 0
            || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.play_land(view) {
            return action;
        }
        if self.player == PlayerId(0) {
            let sprite_live = view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(CARD_ID));
            if !sprite_live
                && let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
            {
                return self
                    .cast_blue(view, card.id, &self.sprite_cost)
                    .unwrap_or(PolicyAction::PassPriority);
            }
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(DRAKE_ID))
            {
                let cost = ManaCost::with_colors(3, [Color::Blue]);
                return self
                    .cast_blue(view, card.id, &cost)
                    .unwrap_or(PolicyAction::PassPriority);
            }
        } else {
            let informant_live = view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(INFORMANT_ID));
            if !informant_live
                && view
                    .hand
                    .iter()
                    .any(|card| card.definition == Some(INFORMANT_ID))
            {
                return self
                    .cast_informant(view)
                    .unwrap_or(PolicyAction::PassPriority);
            }
            if self.sprite_attacked_past_informant
                && !self.char_used
                && let Some(sprite) = view
                    .opponent_battlefield
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
            {
                return self
                    .cast_char(view, sprite.id)
                    .unwrap_or(PolicyAction::PassPriority);
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &CapturedMatch, seed: u64, accept: bool) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))"
        || captured.winner != Some(0)
        || captured.rejected_moves != 0
    {
        return Err(
            "Surveilling Sprite shuffled match did not end in a clean seat-zero win".to_owned(),
        );
    }
    let sprite = captured.sprite.ok_or("Surveilling Sprite was not cast")?;
    let char_spell = captured
        .char_spell
        .ok_or("Char did not target the Sprite")?;
    if captured.sprite_cost.as_ref() != Some(&ManaCost::with_colors(1, [Color::Blue]))
        || captured.sprite_payment.len() != 2
        || captured
            .sprite_payment
            .iter()
            .any(|color| *color != Color::Blue)
        || captured.sprite_pt != Some((1, 1))
        || captured
            .sprite_keywords
            .as_ref()
            .is_none_or(|keywords| !keywords.contains(&Keyword::Flying))
        || captured.char_payment.len() != 3
        || !captured.char_payment.contains(&Color::Red)
    {
        return Err(format!(
            "Surveilling Sprite identity, {{1}}{{U}} payment, 1/1 Flying, or Char payment differs: cost={:?}, sprite_payment={:?}, pt={:?}, keywords={:?}, char={:?}, char_payment={:?}",
            captured.sprite_cost,
            captured.sprite_payment,
            captured.sprite_pt,
            captured.sprite_keywords,
            captured.char_spell,
            captured.char_payment
        ));
    }
    if !captured.blocker_ready || !captured.flying_evasion_observed || captured.blocked_sprite {
        return Err(format!(
            "Surveilling Sprite did not fly past a live ground blocker in combat: blocker_ready={}, flying_evasion={}, blocked={}, block/cast events={:?}",
            captured.blocker_ready,
            captured.flying_evasion_observed,
            captured.blocked_sprite,
            captured
                .events
                .iter()
                .filter(|event| matches!(
                    event,
                    GameEvent::BlockersDeclared { .. }
                        | GameEvent::SpellCast { .. }
                        | GameEvent::CardMoved {
                            to: Zone::Battlefield,
                            ..
                        }
                ))
                .take(30)
                .collect::<Vec<_>>()
        ));
    }
    if !captured.optional_choice_seen
        || captured.optional_can_pay != Some(true)
        || captured.optional_paid != accept
        || captured.source_in_graveyard_before_choice != Some(true)
        || !captured.trigger_resolved
        || !captured.ability_resolved
        || captured.drawn_card.is_some() != accept
    {
        return Err(
            "optional dies-trigger choice, graveyard source, or draw result differs".to_owned(),
        );
    }
    let events = &captured.events;
    let sprite_paid = find_event(events, |event| matches!(event,
        GameEvent::SpellManaPaid { player: PlayerId(0), card, colors }
            if *card == sprite && colors.len() == 2 && colors.iter().all(|color| *color == Color::Blue)))
        .ok_or("Surveilling Sprite blue payment event missing")?;
    let sprite_cast = find_event_after(events, sprite_paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == sprite)).ok_or("Sprite SpellCast event missing")?;
    let sprite_resolved = find_event_after(
        events,
        sprite_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == sprite),
    )
    .ok_or("Sprite SpellResolved event missing")?;
    let sprite_entry = find_event_after(events, sprite_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == sprite)).ok_or("Sprite battlefield entry missing")?;
    let attack = captured
        .flying_evasion_attack_index
        .ok_or("Flying attack with a live Informant was not recorded")?;
    let attacker = captured
        .flying_evasion_attacker
        .ok_or("Flying Sprite attacker identity was not recorded")?;
    if attacker != sprite {
        return Err("recorded Flying combat attacker is not the cast Sprite".to_owned());
    }
    let blocks = find_event_after(events, attack, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if !assignments.iter().any(|(blocked, _)| *blocked == sprite))).ok_or("unblocked flying Sprite blocker declaration missing")?;
    let char_paid = find_event(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(1), card, colors } if *card == char_spell && colors.len() == 3)).ok_or("Char mana payment event missing")?;
    let char_cast = find_event_after(events, char_paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == char_spell)).ok_or("Char SpellCast event missing")?;
    let damage = find_event_after(events, char_cast, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 4 } if *source == char_spell && *permanent == sprite)).ok_or_else(|| format!("Char did not deal four lethal damage to Sprite: {:?}", events.iter().filter(|event| matches!(event, GameEvent::SpellCast { .. } | GameEvent::SpellResolved { .. } | GameEvent::DamageDealtToPermanent { .. } | GameEvent::CardMoved { to: Zone::Battlefield, .. } | GameEvent::AttackersDeclared { .. } | GameEvent::BlockersDeclared { .. })).collect::<Vec<_>>()))?;
    let char_resolved = find_event_after(
        events,
        damage,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == char_spell),
    )
    .ok_or("Char SpellResolved event missing after its damage effect")?;
    let death = find_event_after(events, damage, |event| matches!(event, GameEvent::StateBasedAction { card, reason: "creature has lethal damage" } if *card == sprite)).ok_or("lethal-damage state-based action for Sprite missing")?;
    let graveyard = find_event_after(events, death, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == sprite)).ok_or("Sprite graveyard movement missing")?;
    let trigger = find_event_after(events, graveyard, |event| matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, ability: ABILITY_ID, .. } if *source == sprite)).ok_or("Sprite dies trigger did not stack after it died")?;
    let draw_move = if accept {
        Some(find_event_after(events, trigger + 1, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if Some(*card) == captured.drawn_card)).ok_or("accepted optional trigger did not move a card to hand")?)
    } else {
        None
    };
    let ability_resolved = find_event_after(events, trigger, |event| matches!(event, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == sprite)).ok_or("Sprite dies trigger did not resolve")?;
    let optional_decision = find_event_after(events, ability_resolved, |event| matches!(event, GameEvent::PolicyMoveSubmitted { player: PlayerId(0), policy, kind } if policy == "rav.surveilling-sprite-flight-and-draw.v1" && *kind == cardbench_magic_engine::PolicyMoveKind::ResolveOptionalTriggeredAbility)).ok_or("optional draw choice event missing")?;
    let game_end = find_event_after(
        events,
        optional_decision.max(draw_move.unwrap_or(optional_decision)),
        |event| {
            matches!(
                event,
                GameEvent::GameEnded {
                    winner: Some(PlayerId(0))
                }
            )
        },
    )
    .ok_or("terminal seat-zero win event missing")?;
    if !(sprite_paid < sprite_cast
        && sprite_cast < sprite_resolved
        && sprite_resolved < sprite_entry
        && sprite_entry < attack
        && attack < blocks
        && blocks < char_paid
        && char_paid < char_cast
        && char_cast < damage
        && damage < char_resolved
        && damage < death
        && death < graveyard
        && graveyard < trigger
        && trigger < ability_resolved
        && ability_resolved < optional_decision
        && draw_move
            .is_none_or(|index| trigger < index && index < ability_resolved && index < game_end)
        && optional_decision < game_end)
    {
        return Err("Surveilling Sprite cast, flying combat, death-trigger, choice, draw, or game-end order differs".to_owned());
    }
    let projection = project_events(events);
    let mut selected = vec![
        sprite_paid,
        sprite_cast,
        sprite_resolved,
        sprite_entry,
        attack,
        blocks,
        char_paid,
        char_cast,
        char_resolved,
        damage,
        death,
        graveyard,
        trigger,
    ];
    if let Some(draw) = draw_move {
        selected.push(draw);
    }
    selected.extend([ability_resolved, optional_decision, game_end]);
    selected.sort_unstable();
    let projected: Vec<ProjectedEvent> = selected
        .into_iter()
        .map(|index| projection[index].clone())
        .collect();
    let expected_base = [
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "AttackersDeclared",
        "BlockersDeclared",
        "SpellManaPaid",
        "SpellCast",
        "DamageDealtToPermanent",
        "SpellResolved",
        "StateBasedAction",
        "CardMoved",
        "TriggeredAbilityStacked",
    ];
    if projected
        .iter()
        .take(expected_base.len())
        .map(|event| event.kind.as_str())
        .ne(expected_base)
    {
        return Err(
            "Surveilling Sprite projected spell, combat, and dies-trigger events differ".to_owned(),
        );
    }
    let mut ordered = projected
        .iter()
        .map(|event| event.kind.as_str())
        .collect::<Vec<_>>();
    let mut expected = expected_base.to_vec();
    if accept {
        expected.push("CardMoved");
    }
    expected.push("AbilityResolved");
    expected.extend(["PolicyMoveSubmitted", "GameEnded"]);
    if ordered != expected {
        ordered.shrink_to_fit();
        return Err(format!(
            "Surveilling Sprite projected events differ: {ordered:?}"
        ));
    }
    let facts: Vec<_> = projected.iter().map(|event| &event.facts).collect();
    let objects = |index: usize| {
        facts[index]
            .objects
            .iter()
            .map(|object| object.0)
            .collect::<Vec<_>>()
    };
    let seat = |index: usize, expected: usize| {
        facts[index].seats.len() == 1 && usize::from(facts[index].seats[0].0) == expected
    };
    if objects(0) != [sprite.0]
        || !seat(0, 0)
        || facts[0].mana_colors != ["Blue", "Blue"]
        || objects(1) != [sprite.0]
        || objects(2) != [sprite.0]
        || objects(3) != [sprite.0]
        || facts[3].zone.as_deref() != Some("Battlefield")
        || !facts[4].objects.iter().any(|object| object.0 == sprite.0)
        || !seat(4, 0)
        || !seat(5, 1)
        || objects(6) != [char_spell.0]
        || !seat(6, 1)
        || !facts[6].mana_colors.iter().any(|color| color == "Red")
        || objects(7) != [char_spell.0]
        || objects(8) != [char_spell.0, sprite.0]
        || facts[8].amount != Some(4)
        || objects(9) != [char_spell.0]
    {
        return Err(format!(
            "Surveilling Sprite projected facts do not bind exact objects, costs, attack, and lethal damage: {:?}",
            projected
                .iter()
                .map(|event| (&event.kind, &event.facts))
                .collect::<Vec<_>>()
        ));
    }
    Ok(json!({
        "trace_id": if accept { "rav-surveilling-sprite-dies-draw-accepted-pvp" } else { "rav-surveilling-sprite-dies-draw-declined-pvp" },
        "shuffle_seed": seed,
        "decks": ["rav_surveilling_sprite_island_fliers_60", "rav_char_dimir_interaction_60"],
        "policies": ["rav.surveilling-sprite-flight-and-draw.v1", "rav.char-ground-blocker-opponent.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "surveilling_sprite": {"object": sprite.0, "collector_number": 67, "mana_cost": "{1}{U}", "mana_paid": captured.sprite_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "power": captured.sprite_pt.unwrap().0, "toughness": captured.sprite_pt.unwrap().1, "flying": captured.sprite_keywords.as_ref().is_some_and(|keywords| keywords.contains(&Keyword::Flying)), "zone_after_death": "Graveyard"},
        "ground_blocker": {"object": captured.blocker.map(|object| object.0), "definition": INFORMANT_ID, "live_and_can_block": captured.blocker_ready, "blocked_flying_sprite": captured.blocked_sprite},
        "opponent_char": {"object": char_spell.0, "mana_cost": "{2}{R}", "mana_paid": captured.char_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "dealt_four_lethal_damage": true},
        "dies_trigger": {"ability": ABILITY_ID, "source": sprite.0, "condition": "Dies", "optional_choice_seen": captured.optional_choice_seen, "source_in_graveyard_before_choice": captured.source_in_graveyard_before_choice, "choice": if accept { "accepted" } else { "declined" }, "paid": captured.optional_paid, "drawn_card": captured.drawn_card.map(|object| object.0)},
        "combat": {"flying_evasion_observed": captured.flying_evasion_observed, "opponent_life_at_terminal": captured.seat_one_life, "life_at_terminal": [captured.seat_zero_life, captured.seat_one_life]},
        "events": projected,
        "passed": true
    }))
}

fn find_event(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn find_event_after(
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

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"engine_status":match card.semantic_status {CardSemanticStatus::ExecutableCompatibilitySlice{..}=>"executable_compatibility_slice",CardSemanticStatus::CatalogOnly{..}=>"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"credited":false,"qualification":if passed{"pilot_trace_passed; independent reviewer approval pending"}else{"not_evaluated"}})
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
