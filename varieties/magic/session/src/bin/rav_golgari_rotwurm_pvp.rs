//! Full-game diagnostic authority for RAV Golgari Rotwurm.
//!
//! Cast Rotwurm with the printed black-green cost, sacrifice another live
//! creature to pay its black activated cost, verify targeted life loss, then
//! attack through legal empty blocks to a terminal game.

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbilityBinding, CardDefinition, CardType, Color, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::{SeatId, ViewerScope};
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, card_definitions, rav_activated_ability_bindings,
    rav_activated_ability_cost_modifier_bindings, rav_additional_spell_cost_bindings,
    rav_attachment_bindings, rav_attachment_triggered_ability_bindings as attachment_triggers,
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
const CARD_ID: &str = "RAV-GOLGARI-ROTWURM";
const CARD_NAME: &str = "Golgari Rotwurm";
const ABILITY_ID: &str = "sacrifice-creature-target-player-life-loss";
const FODDER_ID: &str = "RAV-ELVES-OF-DEEP-SHADOW";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/golgari_rotwurm.rs";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/golgari_rotwurm.rs"]
mod candidate_golgari_rotwurm;

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
            eprintln!("Golgari Rotwurm PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_semantics()?;
    let catalog = unique_catalog()?;
    let (seed, capture) = find_trace()?;
    let trace = validate_trace(&capture, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if capture.events != replay.events || capture.winner != replay.winner {
        return Err("Golgari Rotwurm deterministic replay differs".to_owned());
    }
    let mutations = json!({
        "black_activated_cost_mutation_detected": mutation_detected(seed, Mutation::WrongAbilityCost),
        "creature_sacrifice_cost_mutation_detected": mutation_detected(seed, Mutation::NoCreatureSacrifice),
        "targeted_life_loss_mutation_detected": mutation_detected(seed, Mutation::NoLifeLoss),
        "sealed_mutation_authority": false
    });
    for key in [
        "black_activated_cost_mutation_detected",
        "creature_sacrifice_cost_mutation_detected",
        "targeted_life_loss_mutation_detected",
    ] {
        if mutations[key] != true {
            return Err(format!("Golgari Rotwurm semantic mutant escaped: {key}"));
        }
    }
    let cards_passed = catalog
        .iter()
        .filter(|card| card.collector_number == 210 && card.name == CARD_NAME)
        .count();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 210, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "105.2 and 202.1-202.2 colored mana costs and payment",
                "117 priority and timing",
                "302.1 creature characteristics",
                "601.2 casting and paying costs",
                "602.2 activating abilities, selecting targets, and paying costs",
                "608.2 resolving activated abilities",
                "701.17 sacrifice",
                "119.3 loss of life",
                "704 state-based actions"
            ],
            "source_contract": "RAV Golgari Rotwurm #210, {3}{B}{G} black-green 5/4 creature with {B}, sacrifice another creature: target player loses 1 life",
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
        "qualification_status": "diagnostic_only; external qualification pending",
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": catalog.iter().map(card_status).collect::<Vec<_>>()
    }))
}

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<_> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != 291
        || printings.len() != 306
        || !catalog
            .iter()
            .any(|card| card.collector_number == 210 && card.name == CARD_NAME)
    {
        return Err("RAV catalog identity or denominator changed for Golgari Rotwurm".to_owned());
    }
    Ok(catalog)
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Golgari Rotwurm source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Golgari Rotwurm source is not valid Rust".to_owned())?;
    if file.items.len() != 4 || !file.attrs.iter().all(|attr| attr.path().is_ident("doc")) {
        return Err(
            "Golgari Rotwurm source must contain two imports and two literal functions".to_owned(),
        );
    }
    let mut uses = 0;
    let mut functions = BTreeSet::new();
    for item in file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(item.vis, Visibility::Inherited) =>
            {
                uses += 1
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                let output = match name.as_str() {
                    "definition" => "CardDefinition",
                    "activated_ability" => "ActivatedAbilityBinding",
                    _ => {
                        return Err(
                            "Golgari Rotwurm source contains an unexpected function".to_owned()
                        );
                    }
                };
                let crate_visible = matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"));
                let literal = function.block.stmts.len() == 1
                    && matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(_), None));
                if !functions.insert(name)
                    || !crate_visible
                    || !function.sig.inputs.is_empty()
                    || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if type_is(ty, output))
                    || !literal
                {
                    return Err(
                        "Golgari Rotwurm functions must directly return declarative literals"
                            .to_owned(),
                    );
                }
            }
            _ => {
                return Err(
                    "Golgari Rotwurm source contains unsupported executable top-level code"
                        .to_owned(),
                );
            }
        }
    }
    if uses != 2
        || functions != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()])
    {
        return Err("Golgari Rotwurm source structure differs from its contract".to_owned());
    }
    Ok(())
}

fn type_is(ty: &Type, expected: &str) -> bool {
    matches!(ty, Type::Path(path) if path.qself.is_none() && path.path.segments.last().is_some_and(|segment| segment.ident == expected))
}

fn validate_candidate_semantics() -> Result<(), String> {
    let definition = candidate_golgari_rotwurm::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Golgari Rotwurm is missing from the registry")?;
    let binding = candidate_golgari_rotwurm::activated_ability();
    let registered_binding = rav_activated_ability_bindings()
        .into_iter()
        .find(|known| known.card_definition == CARD_ID && known.ability.id == ABILITY_ID)
        .ok_or("Golgari Rotwurm ability is missing from the registry")?;
    let ability = &binding.ability;
    let rules = [
        "full-rules-fidelity",
        "colored-cost-casting",
        "base-characteristics",
        "activated-sacrifice-creature-target-player-life-loss",
    ];
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(3, [Color::Black, Color::Green])
        || definition.colors != BTreeSet::from([Color::Black, Color::Green])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.supported_rules != rules
        || definition.power != Some(5)
        || definition.toughness != Some(4)
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
        || registered.mana_cost != definition.mana_cost
        || registered.colors != definition.colors
        || registered.card_types != definition.card_types
        || registered.power != definition.power
        || registered.toughness != definition.toughness
        || binding.card_definition != CARD_ID
        || ability.id != ABILITY_ID
        || ability.mana_cost != ManaCost::with_colors(0, [Color::Black])
        || ability.tap_cost
        || ability.sorcery_speed
        || ability.additional_tap_creatures != 0
        || ability.sacrifice_source
        || ability.sacrifice_creatures != 1
        || ability.sacrifice_lands != 0
        || ability.discard_cards != 0
        || ability.targets != [cardbench_magic_engine::TargetRequirement::Player]
        || ability.effects != [Effect::LoseLifeTarget { amount: 1 }]
        || registered_binding != binding
    {
        return Err("Golgari Rotwurm identity, mana cost, creature-sacrifice cost, target, or life-loss effect differs from the RAV printing".to_owned());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongAbilityCost,
    NoCreatureSacrifice,
    NoLifeLoss,
}

fn candidate_values(mutation: Mutation) -> (CardDefinition, ActivatedAbilityBinding) {
    let definition = candidate_golgari_rotwurm::definition();
    let mut binding = candidate_golgari_rotwurm::activated_ability();
    match mutation {
        Mutation::None => {}
        Mutation::WrongAbilityCost => {
            binding.ability.mana_cost = ManaCost::with_colors(0, [Color::Green])
        }
        Mutation::NoCreatureSacrifice => binding.ability.sacrifice_creatures = 0,
        Mutation::NoLifeLoss => binding.ability.effects.clear(),
    }
    (definition, binding)
}

#[derive(Clone, Debug)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    source: Option<ObjectId>,
    fodder: Option<ObjectId>,
    cast_payment: Vec<Color>,
    live_pt: Option<(i32, i32)>,
    sacrificed_creature: Option<ObjectId>,
    fodder_in_graveyard: bool,
    ability_payment: Option<ManaCost>,
    ability_activated: bool,
    life_lost: i16,
    opponent_life_after_ability: Option<i64>,
    attacks: usize,
    damage: usize,
    opponent_life: i64,
    rejected_moves: u32,
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

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_GOLGARI_ROTWURM_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_GOLGARI_ROTWURM_SEED must be an unsigned integer".to_owned())?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let Ok(capture) = run_match(seed, Mutation::None)
            && validate_trace(&capture, seed).is_ok()
        {
            return Ok((seed, capture));
        }
    }
    Err(format!(
        "no Golgari Rotwurm full-game trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    run_match(seed, mutation)
        .and_then(|capture| validate_trace(&capture, seed))
        .is_err()
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let (definition, binding) = candidate_values(mutation);
    let mut game = new_game(definition, binding)?;
    let p0 = deck(&[
        (CARD_ID, 4),
        (FODDER_ID, 4),
        ("RAV-SWAMP", 26),
        ("RAV-FOREST", 26),
    ]);
    let p1 = deck(&[("RAV-FOREST", 60)]);
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 40,
        max_policy_moves: 6_000,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &p0)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &p1)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(RotwurmPolicy::new(PlayerId(0), mutation)),
        Box::new(PassPolicy),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut source = None;
    let mut fodder = None;
    let mut cast_payment = Vec::new();
    let mut live_pt = None;
    let mut sacrificed_creature = None;
    let mut fodder_in_graveyard = false;
    let mut ability_payment = None;
    let mut ability_activated = false;
    let mut life_lost = 0_i16;
    let mut opponent_life_after_ability = None;
    let mut attacks = 0;
    let mut damage = 0;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Golgari Rotwurm duel exceeded bounded full-game run".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Golgari Rotwurm policy seat unavailable")?;
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
        if let PolicyAction::Cast(request) | PolicyAction::CastWithPayment { request, .. } = &action
        {
            if view
                .hand
                .iter()
                .any(|card| card.id == request.card && card.definition == Some(CARD_ID))
            {
                source.get_or_insert(request.card);
            }
            if view
                .hand
                .iter()
                .any(|card| card.id == request.card && card.definition == Some(FODDER_ID))
            {
                fodder.get_or_insert(request.card);
            }
        }
        let before = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Golgari Rotwurm policy move rejected on turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        for event in &game.event_log[before..] {
            match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|known| known.id == CARD_ID) =>
                {
                    source.get_or_insert(*card);
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|known| known.id == FODDER_ID) =>
                {
                    fodder.get_or_insert(*card);
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if Some(*card) == source => cast_payment = colors.clone(),
                GameEvent::SpellResolved { card } if Some(*card) == source => {
                    let c = game.characteristics(*card).map_err(|e| e.to_string())?;
                    live_pt = Some((
                        c.power.ok_or("Rotwurm power missing")?,
                        c.toughness.ok_or("Rotwurm toughness missing")?,
                    ));
                }
                GameEvent::SacrificedAsAbilityCost {
                    player: PlayerId(0),
                    source: ability_source,
                    permanent,
                } if Some(*ability_source) == source => {
                    sacrificed_creature = Some(*permanent);
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Graveyard,
                } if Some(*card) == fodder => {
                    fodder_in_graveyard = true;
                }
                GameEvent::AbilityManaPaid {
                    player: PlayerId(0),
                    source: ability_source,
                    ability: ABILITY_ID,
                    mana_cost,
                } if Some(*ability_source) == source => {
                    ability_payment = Some(mana_cost.clone());
                }
                GameEvent::AbilityActivated {
                    player: PlayerId(0),
                    source: ability_source,
                    definition: CARD_ID,
                    ability: ABILITY_ID,
                    ..
                } if Some(*ability_source) == source => {
                    ability_activated = true;
                }
                GameEvent::LifeLost {
                    source: ability_source,
                    player: PlayerId(1),
                    amount,
                } if Some(*ability_source) == source => {
                    life_lost += *amount;
                    opponent_life_after_ability =
                        Some(game.player(PlayerId(1)).map_err(|e| e.to_string())?.life);
                }
                GameEvent::AttackersDeclared {
                    player: PlayerId(0),
                    attackers,
                } if source.is_some_and(|id| attackers.contains(&id)) => {
                    attacks += 1;
                }
                GameEvent::DamageDealtToPlayer {
                    source: damage_source,
                    player: PlayerId(1),
                    amount: 5,
                } if Some(*damage_source) == source => {
                    damage += 1;
                }
                _ => {}
            }
        }
    }
    let opponent_life = game
        .player(PlayerId(1))
        .map_err(|error| error.to_string())?
        .life;
    Ok(CapturedMatch {
        events: game.event_log.clone(),
        winner: game.winner().map(|p| p.0),
        turns: game.turn,
        accepted_moves: accepted,
        source,
        fodder,
        cast_payment,
        live_pt,
        sacrificed_creature,
        fodder_in_graveyard,
        ability_payment,
        ability_activated,
        life_lost,
        opponent_life_after_ability,
        attacks,
        damage,
        opponent_life,
        rejected_moves: attempted.saturating_sub(accepted),
    })
}

fn new_game(definition: CardDefinition, binding: ActivatedAbilityBinding) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|known| known.id == CARD_ID)
        .ok_or("Golgari Rotwurm missing from definitions")? = definition;
    let mut abilities = rav_activated_ability_bindings();
    abilities.retain(|known| known.card_definition != CARD_ID);
    abilities.push(binding);
    let mut triggers = rav_triggered_ability_bindings();
    triggers.extend(attachment_triggers());
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
        triggers,
        rav_static_continuous_effect_bindings(),
        rav_land_entry_bindings(),
    )
    .map_err(|e| e.to_string())?;
    game.register_static_library_top_reveal_bindings(rav_static_library_top_reveal_bindings())
        .map_err(|e| e.to_string())?;
    game.register_static_attack_restrictions(rav_static_attack_restriction_bindings())
        .map_err(|e| e.to_string())?;
    game.register_static_creature_spell_cost_modifiers(
        rav_static_creature_spell_cost_modifier_bindings(),
    )
    .map_err(|e| e.to_string())?;
    game.register_attachment_bindings(rav_attachment_bindings())
        .map_err(|e| e.to_string())?;
    game.register_entry_copy_bindings(rav_entry_copy_bindings())
        .map_err(|e| e.to_string())?;
    game.register_entry_coin_flip_bindings(rav_entry_coin_flip_bindings())
        .map_err(|e| e.to_string())?;
    game.register_legendary_permanent_bindings(rav_legendary_permanent_bindings())
        .map_err(|e| e.to_string())?;
    game.register_static_entry_restriction_bindings(rav_static_entry_restriction_bindings())
        .map_err(|e| e.to_string())?;
    game.register_mana_ability_cost_bindings(rav_mana_ability_cost_bindings())
        .map_err(|e| e.to_string())?;
    game.register_cost_reduction_bindings(rav_cost_reduction_bindings())
        .map_err(|e| e.to_string())?;
    game.register_activated_ability_cost_modifier_bindings(
        rav_activated_ability_cost_modifier_bindings(),
    )
    .map_err(|e| e.to_string())?;
    game.register_generalized_activated_ability_cost_bindings(
        rav_generalized_activated_ability_cost_bindings(),
    )
    .map_err(|e| e.to_string())?;
    game.register_damage_replacement_effect_bindings(rav_damage_replacement_effect_bindings())
        .map_err(|e| e.to_string())?;
    game.register_replacement_effect_bindings(rav_replacement_effect_bindings())
        .map_err(|e| e.to_string())?;
    game.validate_invariants().map_err(|e| e.to_string())?;
    Ok(game)
}

struct RotwurmPolicy {
    player: PlayerId,
    mutation: Mutation,
    ability_used: bool,
    source_cast: bool,
    fodder_cast: bool,
}
impl RotwurmPolicy {
    fn new(player: PlayerId, mutation: Mutation) -> Self {
        Self {
            player,
            mutation,
            ability_used: false,
            source_cast: false,
            fodder_cast: false,
        }
    }
    fn activate_land(view: &GameView, color: Color) -> Option<PolicyAction> {
        view.own_battlefield
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&color)
                    && !card.tapped
            })
            .map(|card| PolicyAction::ActivateManaAbility {
                land: card.id,
                color,
            })
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
    fn cast(view: &GameView, card: ObjectId, cost: &ManaCost) -> Option<PolicyAction> {
        Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: Self::payment_selection(view, cost)?,
        })
    }
}
impl CodePolicy for RotwurmPolicy {
    fn id(&self) -> &'static str {
        "rav.golgari-rotwurm-life-loss.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::PrecombatMain
            && view.active_player == self.player
            && view.priority == self.player
            && view.stack_depth == 0
        {
            if view.lands_played == 0 {
                let black_sources = view
                    .own_battlefield
                    .iter()
                    .filter(|card| {
                        card.card_types.contains(&CardType::Land)
                            && card.mana_colors.contains(&Color::Black)
                    })
                    .count();
                let green_sources = view
                    .own_battlefield
                    .iter()
                    .filter(|card| {
                        card.card_types.contains(&CardType::Land)
                            && card.mana_colors.contains(&Color::Green)
                    })
                    .count();
                let preferred = if black_sources <= green_sources {
                    "RAV-SWAMP"
                } else {
                    "RAV-FOREST"
                };
                if let Some(land) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(preferred))
                    .or_else(|| {
                        view.hand
                            .iter()
                            .find(|card| card.card_types.contains(&CardType::Land))
                    })
                {
                    return PolicyAction::PlayLand { card: land.id };
                }
            }
            if !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(FODDER_ID))
                && !self.fodder_cast
                && let Some(elves) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(FODDER_ID))
            {
                if view.mana_pool.amount(Color::Green) > 0 {
                    if let Some(action) =
                        Self::cast(view, elves.id, &ManaCost::with_colors(0, [Color::Green]))
                    {
                        self.fodder_cast = true;
                        return action;
                    }
                }
                return Self::activate_land(view, Color::Green)
                    .unwrap_or(PolicyAction::PassPriority);
            }
            if !self.source_cast
                && let Some(rotwurm) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
            {
                let black = view.mana_pool.amount(Color::Black);
                let green = view.mana_pool.amount(Color::Green);
                let total = view.mana_pool.total();
                if black > 0 && green > 0 && total >= 5 {
                    let cost = ManaCost::with_colors(3, [Color::Black, Color::Green]);
                    if let Some(action) = Self::cast(view, rotwurm.id, &cost) {
                        self.source_cast = true;
                        return action;
                    }
                }
                let missing = if black == 0 {
                    Some(Color::Black)
                } else if green == 0 {
                    Some(Color::Green)
                } else if total < 5 {
                    view.own_battlefield
                        .iter()
                        .find(|card| card.card_types.contains(&CardType::Land) && !card.tapped)
                        .and_then(|land| land.mana_colors.iter().next().copied())
                } else {
                    None
                };
                if let Some(color) = missing
                    && let Some(action) = Self::activate_land(view, color)
                {
                    return action;
                }
                if total < 5
                    && let Some(color) = [Color::Black, Color::Green]
                        .into_iter()
                        .find(|color| Self::activate_land(view, *color).is_some())
                {
                    return Self::activate_land(view, color).unwrap_or(PolicyAction::PassPriority);
                }
            }
            let rotwurm = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID));
            if !self.ability_used
                && let Some(source) = rotwurm
            {
                if let Some(fodder) = view.own_battlefield.iter().find(|card| {
                    card.id != source.id && card.card_types.contains(&CardType::Creature)
                }) {
                    let payment_color = if self.mutation == Mutation::WrongAbilityCost {
                        Color::Green
                    } else {
                        Color::Black
                    };
                    if view.mana_pool.amount(payment_color) == 0 {
                        return Self::activate_land(view, payment_color)
                            .unwrap_or(PolicyAction::PassPriority);
                    }
                    self.ability_used = true;
                    let sacrifices = if self.mutation == Mutation::NoCreatureSacrifice {
                        vec![]
                    } else {
                        vec![fodder.id]
                    };
                    return PolicyAction::ActivateAbility {
                        activation: AbilityActivation {
                            source: source.id,
                            ability_id: ABILITY_ID,
                            sacrifice_sources: sacrifices,
                            additional_tap_creatures: vec![],
                            discard_cards: vec![],
                            targets: vec![Target::Player(PlayerId(1))],
                        },
                    };
                }
            }
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
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
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        PolicyAction::PassPriority
    }
}

struct PassPolicy;
impl CodePolicy for PassPolicy {
    fn id(&self) -> &'static str {
        "rav.golgari-rotwurm-pass.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == view.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers { attackers: vec![] };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != view.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(c: &CapturedMatch, seed: u64) -> Result<Value, String> {
    let source = c.source.ok_or("Golgari Rotwurm was not cast")?;
    let fodder = c
        .fodder
        .ok_or("Elves of Deep Shadow sacrifice fodder was not cast")?;
    if source == fodder || c.winner != Some(0) || c.rejected_moves != 0 || c.opponent_life > 0 {
        return Err(format!(
            "Golgari Rotwurm game lacks a clean terminal win: winner={:?}, life={}, rejected={}",
            c.winner, c.opponent_life, c.rejected_moves
        ));
    }
    let mana_counts = |colors: &[Color]| {
        (
            colors.iter().filter(|c| **c == Color::Black).count(),
            colors.iter().filter(|c| **c == Color::Green).count(),
        )
    };
    let (black, green) = mana_counts(&c.cast_payment);
    if c.cast_payment.len() != 5 || black < 1 || green < 1 || c.live_pt != Some((5, 4)) {
        return Err(format!(
            "Golgari Rotwurm casting cost or live characteristics differ: mana={:?}, pt={:?}",
            c.cast_payment, c.live_pt
        ));
    }
    if c.sacrificed_creature != Some(fodder)
        || !c.fodder_in_graveyard
        || c.ability_payment != Some(ManaCost::with_colors(0, [Color::Black]))
        || !c.ability_activated
        || c.life_lost != 1
        || c.opponent_life_after_ability != Some(19)
    {
        return Err(format!(
            "Rotwurm must pay {{B}}, sacrifice another creature, and make the target player lose exactly one life: turns={}, attacks={}, damage={}, fodder={:?}, moved={}, cost={:?}, activated={}, life_lost={}, target_life={:?}",
            c.turns,
            c.attacks,
            c.damage,
            c.sacrificed_creature,
            c.fodder_in_graveyard,
            c.ability_payment,
            c.ability_activated,
            c.life_lost,
            c.opponent_life_after_ability
        ));
    }
    if c.attacks != 4 || c.damage != 4 {
        return Err(format!(
            "Rotwurm must make four legal 5-power attacks after life loss: attacks={}, damage={}",
            c.attacks, c.damage
        ));
    }
    let events = &c.events;
    let fodder_payment=event_index(events,|e|matches!(e,GameEvent::SpellManaPaid{player:PlayerId(0),card,colors}if *card==fodder&&colors==&[Color::Green])).ok_or("Elves exact green payment missing")?;
    let fodder_cast = event_after(
        events,
        fodder_payment,
        |e| matches!(e,GameEvent::SpellCast{player:PlayerId(0),card}if *card==fodder),
    )
    .ok_or("Elves cast event missing")?;
    let fodder_resolve = event_after(
        events,
        fodder_cast,
        |e| matches!(e,GameEvent::SpellResolved{card}if *card==fodder),
    )
    .ok_or("Elves resolution missing")?;
    let fodder_entry = event_after(
        events,
        fodder_resolve,
        |e| matches!(e,GameEvent::CardMoved{card,to:Zone::Battlefield}if *card==fodder),
    )
    .ok_or("Elves battlefield entry missing")?;
    let cast=event_index(events,|e|matches!(e,GameEvent::SpellManaPaid{player:PlayerId(0),card,colors}if *card==source&&colors.len()==5&&colors.iter().filter(|c|**c==Color::Black).count()>=1&&colors.iter().filter(|c|**c==Color::Green).count()>=1)).ok_or("Rotwurm colored five-mana receipt missing")?;
    let cast_event = event_after(
        events,
        cast,
        |e| matches!(e,GameEvent::SpellCast{player:PlayerId(0),card}if *card==source),
    )
    .ok_or("Rotwurm cast event missing")?;
    let resolved = event_after(
        events,
        cast_event,
        |e| matches!(e,GameEvent::SpellResolved{card}if *card==source),
    )
    .ok_or("Rotwurm resolution missing")?;
    let entry = event_after(
        events,
        resolved,
        |e| matches!(e,GameEvent::CardMoved{card,to:Zone::Battlefield}if *card==source),
    )
    .ok_or("Rotwurm battlefield entry missing")?;
    let paid=event_after(events,entry,|e|matches!(e,GameEvent::AbilityManaPaid{player:PlayerId(0),source:ability_source,ability:ABILITY_ID,mana_cost}if *ability_source==source&&*mana_cost==ManaCost::with_colors(0,[Color::Black]))).ok_or("exact black activated-ability payment missing")?;
    let sacrificed=event_after(events,paid,|e|matches!(e,GameEvent::SacrificedAsAbilityCost{player:PlayerId(0),source:ability_source,permanent}if *ability_source==source&&*permanent==fodder)).ok_or("another creature was not sacrificed as the ability cost")?;
    let fodder_move = event_after(
        events,
        sacrificed,
        |e| matches!(e,GameEvent::CardMoved{card,to:Zone::Graveyard}if *card==fodder),
    )
    .ok_or("sacrificed fodder did not move to graveyard")?;
    let activated=event_after(events,fodder_move,|e|matches!(e,GameEvent::AbilityActivated{player:PlayerId(0),source:ability_source,definition:CARD_ID,ability:ABILITY_ID,..}if *ability_source==source)).ok_or("Rotwurm ability was not put on stack after its cost")?;
    let loss=event_after(events,activated,|e|matches!(e,GameEvent::LifeLost{source:ability_source,player:PlayerId(1),amount:1}if *ability_source==source)).ok_or("target opponent life-loss event missing")?;
    let ability_resolved=event_after(events,loss,|e|matches!(e,GameEvent::AbilityResolved{source:ability_source,ability:ABILITY_ID,..}if *ability_source==source)).ok_or("Rotwurm activated ability resolution missing")?;
    let game_end = event_index(events, |e| {
        matches!(
            e,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("seat-zero terminal receipt missing")?;
    if !(fodder_payment < fodder_cast
        && fodder_cast < fodder_resolve
        && fodder_resolve < fodder_entry
        && cast_event < resolved
        && resolved < entry
        && entry < paid
        && paid < sacrificed
        && sacrificed < fodder_move
        && fodder_move < activated
        && activated < loss
        && loss < ability_resolved
        && ability_resolved < game_end)
    {
        return Err("Golgari Rotwurm cast, payment, sacrifice, target life loss, resolution, and terminal event order differs".to_owned());
    }
    let attack_indexes:Vec<_>=events.iter().enumerate().filter_map(|(i,e)|matches!(e,GameEvent::AttackersDeclared{player:PlayerId(0),attackers}if attackers.contains(&source)).then_some(i)).collect();
    let block_indexes:Vec<_>=events.iter().enumerate().filter_map(|(i,e)|matches!(e,GameEvent::BlockersDeclared{player:PlayerId(1),assignments}if assignments.is_empty()).then_some(i)).collect();
    let damage_indexes:Vec<_>=events.iter().enumerate().filter_map(|(i,e)|matches!(e,GameEvent::DamageDealtToPlayer{source:attacker,player:PlayerId(1),amount:5}if *attacker==source).then_some(i)).collect();
    if attack_indexes.len() != 4
        || block_indexes.len() < 4
        || damage_indexes.len() != 4
        || attack_indexes
            .iter()
            .zip(&damage_indexes)
            .any(|(attack, damage)| {
                attack >= damage
                    || !block_indexes
                        .iter()
                        .any(|block| attack < block && block < damage)
            })
    {
        return Err(
            "Rotwurm legal attacks, empty blocks, or five-damage receipts differ".to_owned(),
        );
    }
    let projection = project_events(events);
    let mut selected = BTreeSet::from([
        fodder_payment,
        fodder_cast,
        fodder_resolve,
        fodder_entry,
        cast,
        cast_event,
        resolved,
        entry,
        paid,
        sacrificed,
        fodder_move,
        activated,
        loss,
        ability_resolved,
        game_end,
    ]);
    for (attack, damage) in attack_indexes.iter().zip(&damage_indexes) {
        selected.insert(*attack);
        selected.insert(*damage);
        if let Some(block) = block_indexes
            .iter()
            .copied()
            .find(|b| *b > *attack && *b < *damage)
        {
            selected.insert(block);
        }
    }
    let relevant: Vec<ProjectedEvent> = selected
        .into_iter()
        .map(|i| projection[i].clone())
        .collect();
    let kinds: Vec<_> = relevant.iter().map(|e| e.kind.as_str()).collect();
    let mut expected = vec![
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "AbilityManaPaid",
        "SacrificedAsAbilityCost",
        "CardMoved",
        "AbilityActivated",
        "LifeLost",
        "AbilityResolved",
    ];
    for _ in 0..4 {
        expected.extend([
            "AttackersDeclared",
            "BlockersDeclared",
            "DamageDealtToPlayer",
        ]);
    }
    expected.push("GameEnded");
    if kinds != expected {
        return Err(format!(
            "projected typed Rotwurm lifecycle differs: {kinds:?}"
        ));
    }
    let pay = &relevant[4].facts;
    if pay.seats != [SeatId(0)]
        || pay.objects.iter().map(|o| o.0).collect::<Vec<_>>() != [source.0]
        || pay.amount != Some(5)
        || !pay.mana_colors.contains(&"Black".to_owned())
        || !pay.mana_colors.contains(&"Green".to_owned())
    {
        return Err("projected black-green Rotwurm spell payment facts differ".to_owned());
    }
    let fodder_pay = &relevant[0].facts;
    if fodder_pay.seats != [SeatId(0)]
        || fodder_pay.objects.iter().map(|o| o.0).collect::<Vec<_>>() != [fodder.0]
        || fodder_pay.amount != Some(1)
        || fodder_pay.mana_colors != ["Green"]
    {
        return Err("projected exact green fodder casting payment facts differ".to_owned());
    }
    let ability_pay = &relevant[8].facts;
    if ability_pay.seats != [SeatId(0)]
        || ability_pay.objects.iter().map(|o| o.0).collect::<Vec<_>>() != [source.0]
        || ability_pay.mana_cost_colors != ["Black"]
    {
        return Err("projected exact black ability payment facts differ".to_owned());
    }
    let life = &relevant[12].facts;
    if life.seats != [SeatId(1)]
        || life.objects.iter().map(|o| o.0).collect::<Vec<_>>() != [source.0]
        || life.amount != Some(1)
    {
        return Err("projected target-player life-loss facts differ".to_owned());
    }
    let sacrificed_event = &relevant[9];
    if sacrificed_event.facts != Default::default()
        || !sacrificed_event.canonical.contains(&format!(
            "source: ObjectId({}), permanent: ObjectId({})",
            source.0, fodder.0
        ))
        || relevant[10]
            .facts
            .objects
            .iter()
            .map(|o| o.0)
            .collect::<Vec<_>>()
            != [fodder.0]
        || relevant[10].facts.zone.as_deref() != Some("Graveyard")
        || relevant[11].facts.seats != [SeatId(0)]
        || relevant[11]
            .facts
            .objects
            .iter()
            .map(|o| o.0)
            .collect::<Vec<_>>()
            != [source.0]
        || relevant[11].facts.ability.as_deref() != Some(ABILITY_ID)
        || relevant[13]
            .facts
            .objects
            .iter()
            .map(|o| o.0)
            .collect::<Vec<_>>()
            != [source.0]
        || relevant[13].facts.ability.as_deref() != Some(ABILITY_ID)
    {
        return Err(
            "typed sacrifice, zone movement, activation, or resolution receipts differ".to_owned(),
        );
    }
    for cycle in 0..4 {
        let base = 14 + cycle * 3;
        let attackers = &relevant[base].facts;
        let blockers = &relevant[base + 1].facts;
        let damage = &relevant[base + 2].facts;
        if attackers.seats != [SeatId(0)]
            || attackers.objects.iter().map(|o| o.0).collect::<Vec<_>>() != [source.0]
            || blockers.seats != [SeatId(1)]
            || blockers.amount != Some(0)
            || damage.seats != [SeatId(1)]
            || damage.objects.iter().map(|o| o.0).collect::<Vec<_>>() != [source.0]
            || damage.amount != Some(5)
        {
            return Err(format!("Rotwurm combat cycle {} facts differ", cycle + 1));
        }
    }
    if relevant
        .last()
        .is_none_or(|event| event.facts.seats != [SeatId(0)])
    {
        return Err("terminal winner facts differ".to_owned());
    }
    Ok(json!({
        "trace_id":"rav-golgari-rotwurm-sacrifice-life-loss-attack-pvp","shuffle_seed":seed,"decks":["rav_golgari_rotwurm_elves_60","rav_forest_land_60"],"policies":["rav.golgari-rotwurm-life-loss.v1","rav.golgari-rotwurm-pass.v1"],"terminal":"Winner(PlayerId(0))","winner_seat":0,"turns":c.turns,"accepted_moves":c.accepted_moves,"rejected_moves":c.rejected_moves,
        "golgari_rotwurm":{"object":source.0,"collector_number":210,"mana_cost":"{3}{B}{G}","mana_paid":c.cast_payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"power":5,"toughness":4},
        "activation":{"ability":ABILITY_ID,"mana_cost":"{B}","mana_paid":"Black","sacrificed_creature":fodder.0,"fodder_moved_to_graveyard":c.fodder_in_graveyard,"life_lost":c.life_lost,"target_seat":1,"opponent_life_after_activation":c.opponent_life_after_ability},
        "combat":{"attacks":c.attacks,"damage_per_attack":5,"terminal_opponent_life":c.opponent_life},"events":relevant,"passed":true
    }))
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn event_after(
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
    let id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,"engine_status":match card.semantic_status{CardSemanticStatus::ExecutableCompatibilitySlice{..}=>"executable_compatibility_slice",CardSemanticStatus::CatalogOnly{..}=>"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"credited":false,"qualification":if passed{"diagnostic_only; external qualification pending"}else{"not_evaluated"}})
}
fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}
