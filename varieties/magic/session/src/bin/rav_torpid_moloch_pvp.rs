//! Full-game diagnostic authority for RAV Torpid Moloch.
//!
//! The pilot casts the red Defender, repeatedly sacrifices three Mountains to
//! remove Defender for a turn, and attacks with Moloch through a legal empty
//! block declaration until the shuffled game reaches a terminal win.

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbilityBinding, CardDefinition, CardType, Color, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, Keyword, Layer, ManaCost, ManaPaymentSelection,
    ObjectId, PlayerId, PolicyAction, Step, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::{SeatId, ViewerScope};
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
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
const CARD_ID: &str = "RAV-TORPID-MOLOCH";
const CARD_NAME: &str = "Torpid Moloch";
const ABILITY_ID: &str = "sacrifice-three-lands-remove-defender";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/torpid_moloch.rs";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/torpid_moloch.rs"]
mod candidate_torpid_moloch;

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
            eprintln!("Torpid Moloch PvP authority failed: {error}");
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
        return Err("Torpid Moloch shuffled full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "printed_defender_removed_mutation_detected": mutation_detected(seed, Mutation::NoPrintedDefender),
        "three_land_cost_mutation_detected": mutation_detected(seed, Mutation::WrongLandCount),
        "defender_removal_effect_mutation_detected": mutation_detected(seed, Mutation::NoDefenderRemoval),
        "sealed_mutation_authority": false
    });
    if mutations["printed_defender_removed_mutation_detected"] != true
        || mutations["three_land_cost_mutation_detected"] != true
        || mutations["defender_removal_effect_mutation_detected"] != true
    {
        return Err("a Torpid Moloch semantic mutant escaped the full-game trace".to_owned());
    }
    let cards_passed = catalog
        .iter()
        .filter(|card| card.collector_number == 147 && card.name == CARD_NAME)
        .count();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 147, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "105.2 and 202.1-202.2 colored mana cost and payment",
                "117 priority and timing",
                "302.1 creature characteristics and Defender",
                "508 declaring attackers",
                "510 combat damage",
                "602.2 activating abilities and paying costs",
                "608.2 resolving an activated ability",
                "613 continuous effects and layers",
                "701.17 sacrifice",
                "704 state-based actions"
            ],
            "source_contract": "RAV Torpid Moloch #147, {R} red 3/2 creature with Defender and no-mana sacrifice-three-lands ability that removes Defender until end of turn",
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
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || !catalog
            .iter()
            .any(|card| card.collector_number == 147 && card.name == CARD_NAME)
    {
        return Err("RAV catalog identity or denominator changed for Torpid Moloch".to_owned());
    }
    Ok(catalog)
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Torpid Moloch source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Torpid Moloch source is not valid Rust".to_owned())?;
    if file.items.len() != 4 || !file.attrs.iter().all(|attr| attr.path().is_ident("doc")) {
        return Err(
            "Torpid Moloch source must contain two imports and two declarative functions"
                .to_owned(),
        );
    }
    let mut uses = 0;
    let mut functions = BTreeSet::new();
    for item in file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(item.vis, Visibility::Inherited) =>
            {
                uses += 1;
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                let return_type = match name.as_str() {
                    "definition" => "CardDefinition",
                    "activated_ability" => "ActivatedAbilityBinding",
                    _ => {
                        return Err(
                            "Torpid Moloch source contains an unexpected function".to_owned()
                        );
                    }
                };
                let crate_visible = matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"));
                let literal = function.block.stmts.len() == 1
                    && matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(_), None));
                if !functions.insert(name)
                    || !crate_visible
                    || !function.sig.inputs.is_empty()
                    || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if type_is(ty, return_type))
                    || !literal
                {
                    return Err(
                        "Torpid Moloch functions must directly return declarative literals"
                            .to_owned(),
                    );
                }
            }
            _ => {
                return Err(
                    "Torpid Moloch source contains unsupported executable top-level code"
                        .to_owned(),
                );
            }
        }
    }
    if uses != 2
        || functions != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()])
    {
        return Err(
            "Torpid Moloch source structure differs from its candidate contract".to_owned(),
        );
    }
    Ok(())
}

fn type_is(ty: &Type, expected: &str) -> bool {
    matches!(ty, Type::Path(path) if path.qself.is_none() && path.path.segments.last().is_some_and(|segment| segment.ident == expected))
}

fn validate_candidate_semantics() -> Result<(), String> {
    let submitted = candidate_torpid_moloch::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Torpid Moloch is missing from the RAV definition registry")?;
    let binding = candidate_torpid_moloch::activated_ability();
    let registered_binding = rav_activated_ability_bindings()
        .into_iter()
        .find(|known| known.card_definition == CARD_ID && known.ability.id == ABILITY_ID)
        .ok_or("Torpid Moloch activation is missing from the RAV binding registry")?;
    let ability = &binding.ability;
    let expected_rules = [
        "full-rules-fidelity",
        "colored-cost-casting",
        "base-characteristics",
        "defender",
        "sacrifice-three-lands-remove-defender",
    ];
    if submitted.id != CARD_ID
        || submitted.name != CARD_NAME
        || submitted.set_code != "RAV"
        || submitted.mana_cost != ManaCost::with_colors(0, [Color::Red])
        || submitted.colors != BTreeSet::from([Color::Red])
        || !submitted.mana_colors.is_empty()
        || submitted.card_types != BTreeSet::from([CardType::Creature])
        || submitted.is_basic_land
        || submitted.supported_rules != expected_rules
        || submitted.power != Some(3)
        || submitted.toughness != Some(2)
        || submitted.keywords != [Keyword::Defender]
        || !submitted.effects.is_empty()
        || registered.mana_cost != submitted.mana_cost
        || registered.colors != submitted.colors
        || registered.card_types != submitted.card_types
        || registered.power != submitted.power
        || registered.toughness != submitted.toughness
        || registered.keywords != submitted.keywords
        || binding.card_definition != CARD_ID
        || ability.id != ABILITY_ID
        || ability.mana_cost != ManaCost::new(0)
        || ability.tap_cost
        || ability.sorcery_speed
        || ability.additional_tap_creatures != 0
        || ability.sacrifice_source
        || ability.sacrifice_creatures != 0
        || ability.sacrifice_lands != 3
        || ability.discard_cards != 0
        || !ability.targets.is_empty()
        || ability.effects
            != [Effect::RemoveSourceKeywordUntilEndOfTurn {
                keyword: Keyword::Defender,
            }]
        || registered_binding != binding
    {
        return Err("Torpid Moloch identity, Defender, three-land cost, or turn-long effect differs from the RAV printing".to_owned());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoPrintedDefender,
    WrongLandCount,
    NoDefenderRemoval,
}

fn candidate_values(mutation: Mutation) -> (CardDefinition, ActivatedAbilityBinding) {
    let mut definition = candidate_torpid_moloch::definition();
    let mut binding = candidate_torpid_moloch::activated_ability();
    match mutation {
        Mutation::NoPrintedDefender => definition.keywords.clear(),
        Mutation::WrongLandCount => binding.ability.sacrifice_lands = 2,
        Mutation::NoDefenderRemoval => binding.ability.effects.clear(),
        Mutation::None => {}
    }
    (definition, binding)
}

#[derive(Clone, Debug)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    source: Option<ObjectId>,
    cast_payment: Vec<Color>,
    initial_pt: Option<(i32, i32)>,
    defender_on_entry: Option<bool>,
    defender_before_activations: Vec<bool>,
    defender_after_resolutions: Vec<bool>,
    defender_at_attacks: Vec<bool>,
    opponent_life: i64,
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
    if let Ok(value) = std::env::var("CARDBENCH_TORPID_MOLOCH_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_TORPID_MOLOCH_SEED must be an unsigned integer".to_owned())?;
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
        "no full Torpid Moloch trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    run_match(seed, mutation)
        .and_then(|capture| validate_trace(&capture, seed).map(|_| capture))
        .is_err()
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let (definition, binding) = candidate_values(mutation);
    let mut game = new_rav_game(definition, binding)?;
    let deck0 = deck(&[(CARD_ID, 4), ("RAV-MOUNTAIN", 56)]);
    let deck1 = deck(&[("RAV-MOUNTAIN", 60)]);
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 50,
        max_policy_moves: 8_000,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &deck0)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &deck1)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(MolochPolicy::new(PlayerId(0))),
        Box::new(PassPolicy),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut source = None;
    let mut cast_payment = Vec::new();
    let mut initial_pt = None;
    let mut defender_on_entry = None;
    let mut defender_before_activations = Vec::new();
    let mut defender_after_resolutions = Vec::new();
    let mut defender_at_attacks = Vec::new();
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Torpid Moloch game exceeded its bounded full-game run".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Torpid Moloch policy seat unavailable")?;
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
            && view
                .hand
                .iter()
                .any(|card| card.id == request.card && card.definition == Some(CARD_ID))
        {
            source.get_or_insert(request.card);
        }
        let before = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Torpid Moloch policy action rejected on turn {} at {:?}: {error}",
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
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    source.get_or_insert(*card);
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if Some(*card) == source => cast_payment = colors.clone(),
                GameEvent::SpellResolved { card } if Some(*card) == source => {
                    let characteristics = game
                        .characteristics(*card)
                        .map_err(|error| error.to_string())?;
                    initial_pt = Some((
                        characteristics.power.ok_or("Moloch power missing")?,
                        characteristics
                            .toughness
                            .ok_or("Moloch toughness missing")?,
                    ));
                    defender_on_entry = Some(characteristics.keywords.contains(&Keyword::Defender));
                }
                GameEvent::AbilityActivated {
                    source: ability_source,
                    definition: CARD_ID,
                    ability: ABILITY_ID,
                    ..
                } if Some(*ability_source) == source => {
                    let characteristics = game
                        .characteristics(*ability_source)
                        .map_err(|error| error.to_string())?;
                    defender_before_activations
                        .push(characteristics.keywords.contains(&Keyword::Defender));
                }
                GameEvent::AbilityResolved {
                    source: ability_source,
                    ability: ABILITY_ID,
                    ..
                } if Some(*ability_source) == source => {
                    let characteristics = game
                        .characteristics(*ability_source)
                        .map_err(|error| error.to_string())?;
                    defender_after_resolutions
                        .push(characteristics.keywords.contains(&Keyword::Defender));
                }
                GameEvent::AttackersDeclared { attackers, .. }
                    if source.is_some_and(|source| attackers.contains(&source)) =>
                {
                    let characteristics = game
                        .characteristics(source.ok_or("source disappeared")?)
                        .map_err(|error| error.to_string())?;
                    defender_at_attacks.push(characteristics.keywords.contains(&Keyword::Defender));
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
        winner: game.winner().map(|player| player.0),
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempted.saturating_sub(accepted),
        source,
        cast_payment,
        initial_pt,
        defender_on_entry,
        defender_before_activations,
        defender_after_resolutions,
        defender_at_attacks,
        opponent_life,
    })
}

fn new_rav_game(
    definition: CardDefinition,
    binding: ActivatedAbilityBinding,
) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|known| known.id == CARD_ID)
        .ok_or("Torpid Moloch missing from card definitions")? = definition;
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

struct MolochPolicy {
    player: PlayerId,
    cast_once: bool,
}

impl MolochPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            cast_once: false,
        }
    }

    fn red_mana_action(view: &GameView) -> Option<PolicyAction> {
        view.own_battlefield
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&Color::Red)
                    && !card.tapped
            })
            .map(|card| PolicyAction::ActivateManaAbility {
                land: card.id,
                color: Color::Red,
            })
    }
}

impl CodePolicy for MolochPolicy {
    fn id(&self) -> &'static str {
        "rav.torpid-moloch-sacrifice-and-attack.v1"
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
            if view.lands_played == 0
                && let Some(land) = view
                    .hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
            if !self.cast_once
                && let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
            {
                if view.mana_pool.amount(Color::Red) > 0 {
                    self.cast_once = true;
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: ManaPaymentSelection::default(),
                    };
                }
                return Self::red_mana_action(view).unwrap_or(PolicyAction::PassPriority);
            }
            if let Some(moloch) = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
                && !moloch.summoning_sick
                && moloch.keywords.contains(&Keyword::Defender)
            {
                let lands: Vec<_> = view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.card_types.contains(&CardType::Land))
                    .map(|card| card.id)
                    .take(3)
                    .collect();
                if lands.len() == 3 {
                    return PolicyAction::ActivateAbility {
                        activation: AbilityActivation {
                            source: moloch.id,
                            ability_id: ABILITY_ID,
                            sacrifice_sources: lands,
                            additional_tap_creatures: vec![],
                            discard_cards: vec![],
                            targets: vec![],
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
                    .filter(|card| {
                        card.definition == Some(CARD_ID)
                            && card.can_attack
                            && !card.keywords.contains(&Keyword::Defender)
                    })
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
        "rav.torpid-moloch-pass-opponent.v1"
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

fn validate_trace(capture: &CapturedMatch, seed: u64) -> Result<Value, String> {
    let source = capture.source.ok_or("Torpid Moloch was not cast")?;
    if capture.winner != Some(0) || capture.rejected_moves != 0 || capture.opponent_life > 0 {
        return Err(format!(
            "Torpid Moloch duel did not finish in a clean P0 win: winner={:?}, turns={}, moves rejected={}",
            capture.winner, capture.turns, capture.rejected_moves
        ));
    }
    if capture.cast_payment != [Color::Red]
        || capture.initial_pt != Some((3, 2))
        || capture.defender_on_entry != Some(true)
        || capture.defender_before_activations.len() != 7
        || capture
            .defender_before_activations
            .iter()
            .any(|present| !present)
        || capture.defender_after_resolutions.len() != 7
        || capture
            .defender_after_resolutions
            .iter()
            .any(|present| *present)
        || capture.defender_at_attacks.len() != 7
        || capture.defender_at_attacks.iter().any(|present| *present)
    {
        return Err(format!(
            "Torpid Moloch cost, live characteristics, temporary Defender removal, or attack state differs: payment={:?}, pt={:?}, defender_entry={:?}, defender_after={:?}, defender_attacks={:?}",
            capture.cast_payment,
            capture.initial_pt,
            capture.defender_on_entry,
            capture.defender_after_resolutions,
            capture.defender_at_attacks
        ));
    }
    let events = &capture.events;
    let paid = event_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == source && colors == &[Color::Red])).ok_or("exact red Moloch spell payment missing")?;
    let cast = event_after(events, paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == source)).ok_or("Moloch cast receipt missing")?;
    let resolved = event_after(
        events,
        cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == source),
    )
    .ok_or("Moloch spell resolution missing")?;
    let entered = event_after(events, resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == source)).ok_or("Moloch battlefield entry missing")?;
    let activations: Vec<_> = events.iter().enumerate().filter_map(|(index, event)| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source: ability_source, definition: CARD_ID, ability: ABILITY_ID, .. } if *ability_source == source).then_some(index)).collect();
    let effects: Vec<_> = events.iter().enumerate().filter_map(|(index, event)| matches!(event, GameEvent::ContinuousEffectCreated { source: effect_source, target, layer: Layer::Ability } if *effect_source == source && *target == source).then_some(index)).collect();
    let resolutions: Vec<_> = events.iter().enumerate().filter_map(|(index, event)| matches!(event, GameEvent::AbilityResolved { source: ability_source, ability: ABILITY_ID, .. } if *ability_source == source).then_some(index)).collect();
    let attacks: Vec<_> = events.iter().enumerate().filter_map(|(index, event)| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&source)).then_some(index)).collect();
    let all_empty_blocks: Vec<_> = events.iter().enumerate().filter_map(|(index, event)| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if assignments.is_empty()).then_some(index)).collect();
    let damage: Vec<_> = events.iter().enumerate().filter_map(|(index, event)| matches!(event, GameEvent::DamageDealtToPlayer { source: damage_source, player: PlayerId(1), amount: 3 } if *damage_source == source).then_some(index)).collect();
    let sacrifices: Vec<_> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::SacrificedAsAbilityCost {
                player: PlayerId(0),
                source: ability_source,
                permanent,
            } if *ability_source == source => Some((index, *permanent)),
            _ => None,
        })
        .collect();
    let land_moves: Vec<_> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::CardMoved {
                card,
                to: Zone::Graveyard,
            } if sacrifices.iter().any(|(_, land)| land == card) => Some((index, *card)),
            _ => None,
        })
        .collect();
    let expiries: Vec<_> = events.iter().enumerate().filter_map(|(index, event)| matches!(event, GameEvent::ContinuousEffectExpired { source: effect_source, target, layer: Layer::Ability } if *effect_source == source && *target == source).then_some(index)).collect();
    let ended = event_index(events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("Torpid Moloch terminal winner receipt missing")?;
    let blocks: Vec<_> = attacks
        .iter()
        .zip(&damage)
        .filter_map(|(attack, damage)| {
            all_empty_blocks
                .iter()
                .copied()
                .find(|block| *block > *attack && *block < *damage)
        })
        .collect();
    let unique_sacrifices: BTreeSet<_> = sacrifices.iter().map(|(_, land)| *land).collect();
    if activations.len() != 7
        || effects.len() != 7
        || resolutions.len() != 7
        || attacks.len() != 7
        || blocks.len() != 7
        || damage.len() != 7
        || sacrifices.len() != 21
        || land_moves.len() != 21
        || expiries.len() != 6
        || unique_sacrifices.len() != 21
    {
        return Err(format!(
            "Torpid Moloch event counts differ: activations={}, effects={}, resolutions={}, attacks={}, empty_blocks={}, damage={}, sacrifices={}, land_moves={}, expiries={}",
            activations.len(),
            effects.len(),
            resolutions.len(),
            attacks.len(),
            blocks.len(),
            damage.len(),
            sacrifices.len(),
            land_moves.len(),
            expiries.len()
        ));
    }
    if !(paid < cast
        && cast < resolved
        && resolved < entered
        && entered < sacrifices[0].0
        && sacrifices[20].0 < activations[6]
        && activations[6] < effects[6]
        && effects[6] < resolutions[6]
        && resolutions[6] < attacks[6]
        && attacks[6] < blocks[6]
        && blocks[6] < damage[6]
        && damage[6] < ended)
    {
        return Err("Moloch payment, casting, seven three-land costs, Defender removal, attacks, damage, and end events are out of order".to_owned());
    }
    for index in 0..7 {
        let start = if index == 0 {
            entered + 1
        } else {
            attacks[index - 1] + 1
        };
        let before_activation: Vec<_> = sacrifices
            .iter()
            .filter(|(event_index, _)| *event_index >= start && *event_index < activations[index])
            .collect();
        if before_activation.len() != 3
            || !(before_activation[2].0 < activations[index]
                && activations[index] < effects[index]
                && effects[index] < resolutions[index]
                && resolutions[index] < attacks[index]
                && attacks[index] < blocks[index]
                && blocks[index] < damage[index])
        {
            return Err(format!(
                "Moloch activation cycle {} does not pay three lands, resolve Defender removal, and attack legally",
                index + 1
            ));
        }
        for (sacrifice_index, land) in &before_activation {
            if !land_moves.iter().any(|(move_index, moved_land)| {
                moved_land == land
                    && *move_index > *sacrifice_index
                    && *move_index < activations[index]
            }) {
                return Err(format!(
                    "Moloch activation cycle {} failed to move every sacrificed land to the graveyard before stacking",
                    index + 1
                ));
            }
        }
        if index < 6
            && !expiries
                .iter()
                .any(|expiry| *expiry > damage[index] && *expiry < sacrifices[3 * (index + 1)].0)
        {
            return Err(format!(
                "Moloch Defender-removal effect did not expire before activation cycle {}",
                index + 2
            ));
        }
    }
    let projection = project_events(events);
    let mut selected = BTreeSet::from([paid, cast, resolved, entered, ended]);
    for index in 0..7 {
        selected.extend([
            activations[index],
            effects[index],
            resolutions[index],
            attacks[index],
            blocks[index],
            damage[index],
        ]);
    }
    selected.extend(expiries.iter().copied());
    selected.extend(sacrifices.iter().map(|(index, _)| *index));
    selected.extend(land_moves.iter().map(|(index, _)| *index));
    let relevant: Vec<ProjectedEvent> = selected
        .into_iter()
        .map(|index| projection[index].clone())
        .collect();
    let kinds: Vec<_> = relevant.iter().map(|event| event.kind.as_str()).collect();
    if kinds
        .iter()
        .filter(|kind| **kind == "AbilityActivated")
        .count()
        != 7
        || kinds
            .iter()
            .filter(|kind| **kind == "SacrificedAsAbilityCost")
            .count()
            != 21
        || kinds
            .iter()
            .filter(|kind| **kind == "ContinuousEffectCreated")
            .count()
            != 7
        || kinds
            .iter()
            .filter(|kind| **kind == "AttackersDeclared")
            .count()
            != 7
        || kinds
            .iter()
            .filter(|kind| **kind == "DamageDealtToPlayer")
            .count()
            != 7
        || kinds.last() != Some(&"GameEnded")
    {
        return Err(format!(
            "Torpid Moloch projected event sequence differs: {kinds:?}"
        ));
    }
    let first_payment = relevant
        .iter()
        .find(|event| event.kind == "SpellManaPaid")
        .ok_or("projected red spell-payment receipt missing")?;
    if first_payment.facts.seats != [SeatId(0)]
        || first_payment
            .facts
            .objects
            .iter()
            .map(|object| object.0)
            .collect::<Vec<_>>()
            != [source.0]
        || first_payment.facts.amount != Some(1)
        || first_payment.facts.mana_colors != ["Red"]
    {
        return Err("projected Torpid Moloch red spell-payment facts differ".to_owned());
    }
    Ok(json!({
        "trace_id": "rav-torpid-moloch-sacrifice-remove-defender-repeat-attack",
        "shuffle_seed": seed,
        "decks": ["rav_torpid_moloch_mountains_60", "rav_mountain_land_60"],
        "policies": ["rav.torpid-moloch-sacrifice-and-attack.v1", "rav.torpid-moloch-pass-opponent.v1"],
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": 0,
        "turns": capture.turns,
        "accepted_moves": capture.accepted_moves,
        "rejected_moves": capture.rejected_moves,
        "torpid_moloch": {"object": source.0, "collector_number": 147, "mana_cost": "{R}", "mana_paid": capture.cast_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "power": 3, "toughness": 2, "defender_on_entry": capture.defender_on_entry, "defender_before_activations": capture.defender_before_activations, "defender_removed_for_each_attack": capture.defender_at_attacks.iter().all(|present| !present)},
        "activation": {"ability": ABILITY_ID, "activations": activations.len(), "mana_cost": "{0}", "lands_sacrificed": sacrifices.len(), "land_graveyard_moves": land_moves.len(), "defender_present_after_resolution": capture.defender_after_resolutions},
        "combat": {"attacks": attacks.len(), "empty_legal_blocks": blocks.len(), "damage_per_attack": 3, "opponent_life_at_terminal": capture.opponent_life},
        "events": relevant,
        "passed": true
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
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"engine_status":match card.semantic_status {CardSemanticStatus::ExecutableCompatibilitySlice{..}=>"executable_compatibility_slice",CardSemanticStatus::CatalogOnly{..}=>"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"credited":false,"qualification":if passed{"diagnostic_only; external qualification pending"}else{"not_evaluated"}})
}
fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}
