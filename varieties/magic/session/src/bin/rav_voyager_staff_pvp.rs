//! Full-game RAV Voyager Staff authority for linked exile and delayed return.

use cardbench_magic_engine::{
    ActivatedAbilityBinding, CardDefinition, CardType, Color, DeckEntry, DeckList,
    DelayedActionKind, Effect, Game, GameEvent, GameView, ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, Step, Target, TargetRequirement, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
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
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::{Item, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-VOYAGER-STAFF";
const CARD_NAME: &str = "Voyager Staff";
const ABILITY_ID: &str = "sacrifice-linked-exile-target-creature-until-end-step";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/voyager_staff.rs";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/voyager_staff.rs"]
mod candidate_voyager_staff;

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
            eprintln!("Voyager Staff authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    let source_validation = validate_candidate_source()?;
    validate_candidate_definition()?;
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<CatalogCard> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || !catalog
            .iter()
            .any(|card| card.collector_number == 274 && card.name == CARD_NAME)
    {
        return Err("RAV catalog denominator or Voyager Staff identity differs".to_owned());
    }

    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.winner != replay.winner
        || captured.staff != replay.staff
        || captured.target != replay.target
    {
        return Err("Voyager Staff full-game replay was not deterministic".to_owned());
    }

    let mutations = json!({
        "wrong_staff_cast_cost_detected_in_game": mutation_detected(seed, Mutation::WrongCastCost),
        "wrong_activation_cost_detected_in_game": mutation_detected(seed, Mutation::WrongActivationCost),
        "missing_source_sacrifice_detected_in_game": mutation_detected(seed, Mutation::MissingSourceSacrifice),
        "missing_delayed_return_detected_in_game": mutation_detected(seed, Mutation::MissingDelayedReturn),
        "target_requirement_mutation_detected": mutation_detected(seed, Mutation::WrongTargetRequirement),
        "sealed_mutation_authority": false
    });
    for key in [
        "wrong_staff_cast_cost_detected_in_game",
        "wrong_activation_cost_detected_in_game",
        "missing_source_sacrifice_detected_in_game",
        "missing_delayed_return_detected_in_game",
        "target_requirement_mutation_detected",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Voyager Staff gameplay mutation escaped the oracle: {key}"
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
        "card": {"collector_number": 274, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "authority_cutoff": "2026-09-24",
            "source_contract": "RAV #274 is a colorless {1} Artifact. Its {2}, sacrifice-this-artifact ability exiles target creature and returns that exact card under its owner's control at the next end step. The older printing uses remove from the game, play, and end of turn; current Oracle modernizes those zone and timing terms.",
            "checked_sections": ["117 Timing and priority", "302.1 Creature permanents", "400 Zones", "406 Exile", "602 Activated abilities", "608 Resolving spells and abilities", "514 End step"]
        },
        "candidate_source_validation": source_validation,
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "unreviewed_diagnostic",
        "replay_deterministic": true,
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongCastCost,
    WrongActivationCost,
    MissingSourceSacrifice,
    MissingDelayedReturn,
    WrongTargetRequirement,
}

#[derive(Clone, Debug)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    staff: Option<ObjectId>,
    target: Option<ObjectId>,
    own_watchwolves: Vec<ObjectId>,
    staff_cast_payment: Vec<Color>,
    activation_mana_before: Option<u8>,
    activation_mana_after: Option<u8>,
    staff_zone: Option<Zone>,
    target_zone: Option<Zone>,
    target_owner: Option<usize>,
    target_controller: Option<usize>,
    target_return_incarnation: Option<u64>,
    linked_group_source: Option<ObjectId>,
    linked_group_source_incarnation: Option<u64>,
    linked_group_controller: Option<usize>,
    linked_group_due_turn: Option<u32>,
    linked_group_members: Vec<(ObjectId, u64, bool)>,
    life: [i64; 2],
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_VOYAGER_STAFF_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_VOYAGER_STAFF_SEED must be an unsigned integer".to_owned())?;
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let Ok(captured) = run_match(seed, Mutation::None) {
            if validate_trace(&captured, seed).is_ok() {
                return Ok((seed, captured));
            }
        }
    }
    Err(format!(
        "no complete Voyager Staff PvP trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation).and_then(|captured| validate_trace(&captured, seed)) {
        Ok(_) => false,
        Err(_) => true,
    }
}

fn validate_candidate_definition() -> Result<(), String> {
    let definition = candidate_voyager_staff::definition();
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::new(1)
        || !definition.colors.is_empty()
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Artifact])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || definition.supported_rules
            != [
                "full-rules-fidelity",
                "colorless-artifact-casting",
                "sacrifice-linked-exile-target-creature-until-end-step",
            ]
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
    {
        return Err(
            "Voyager Staff artifact definition differs from its RAV source contract".to_owned(),
        );
    }
    let ability = candidate_voyager_staff::activated_ability();
    if ability.id != ABILITY_ID
        || ability.mana_cost != ManaCost::new(2)
        || ability.tap_cost
        || ability.sorcery_speed
        || ability.additional_tap_creatures != 0
        || !ability.sacrifice_source
        || ability.sacrifice_creatures != 0
        || ability.sacrifice_lands != 0
        || ability.discard_cards != 0
        || ability.targets != [TargetRequirement::Creature]
        || ability.effects != [Effect::ExileTargetCreatureUntilEndStep]
    {
        return Err(
            "Voyager Staff activated ability differs from its RAV source contract".to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<Value, String> {
    let sentinel = "/tmp/cardbench-voyager-staff-candidate-executed";
    let _ = std::fs::remove_file(sentinel);
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Voyager Staff candidate source: {error}"))?;
    validate_candidate_source_text(&source)?;
    let injected = source.replacen(
        "pub(crate) fn definition() -> CardDefinition {",
        "pub(crate) fn definition() -> CardDefinition {\n    let _ = std::fs::write(\"/tmp/cardbench-voyager-staff-candidate-executed\", b\"ran\");",
        1,
    );
    if injected == source || syn::parse_file(&injected).is_err() {
        return Err(
            "compile-valid Voyager Staff side-effect fixture could not be constructed".to_owned(),
        );
    }
    if validate_candidate_source_text(&injected).is_ok() {
        return Err("Voyager Staff source guard accepted an injected side effect".to_owned());
    }
    if std::path::Path::new(sentinel).exists() {
        return Err(
            "Voyager Staff side-effect sentinel exists after pre-call source rejection".to_owned(),
        );
    }
    Ok(json!({
        "normalized_literal_source_and_ast_checked_before_candidate_calls": true,
        "compile_valid_injected_side_effect_fixture_compiled": true,
        "compile_valid_injected_side_effect_candidate_rejected": true,
        "injected_candidate_calls_before_rejection": 0,
        "side_effect_sentinel_absent_after_rejection": true,
        "guard_ran_before_candidate_function_calls": true,
        "rejection_stage": "pinned-source-and-AST-before-candidate-function-invocation"
    }))
}

// This helper is compiled as part of the trusted authority but is deliberately
// never called. It contains the same valid side-effect expression injected
// into the candidate factory above; rejection must happen before any factory
// invocation could execute it.
#[allow(dead_code)]
fn compile_valid_injected_side_effect_fixture() -> CardDefinition {
    let _ = std::fs::write("/tmp/cardbench-voyager-staff-candidate-executed", b"ran");
    candidate_voyager_staff::definition()
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let normalized = source.replace("\r\n", "\n");
    if normalized != EXPECTED_CANDIDATE_SOURCE {
        return Err("Voyager Staff source differs from the pinned RAV literal module".to_owned());
    }
    let file = syn::parse_file(&normalized)
        .map_err(|_| "Voyager Staff candidate source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
    {
        return Err("Voyager Staff candidate source has unsupported module attributes".to_owned());
    }
    let mut functions = BTreeSet::new();
    for item in file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(item.vis, Visibility::Inherited) => {}
            Item::Fn(function)
                if matches!(function.vis, Visibility::Restricted(_))
                    && function.sig.asyncness.is_none()
                    && function.sig.unsafety.is_none()
                    && function.sig.inputs.is_empty()
                    && functions.insert(function.sig.ident.to_string()) => {}
            _ => {
                return Err(
                    "Voyager Staff source contains executable or unsupported syntax".to_owned(),
                );
            }
        }
    }
    if functions != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()]) {
        return Err("Voyager Staff source must expose exactly its two card factories".to_owned());
    }
    Ok(())
}

const EXPECTED_CANDIDATE_SOURCE: &str = r###"//! Candidate-owned RAV Voyager Staff definition and activated ability.

use cardbench_magic_engine::{
    ActivatedAbility, CardDefinition, CardType, Effect, ManaCost, TargetRequirement,
};
use std::collections::BTreeSet;

/// Printed identity and one-mana artifact casting cost.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-VOYAGER-STAFF",
        name: "Voyager Staff",
        set_code: "RAV",
        mana_cost: ManaCost::new(1),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colorless-artifact-casting",
            "sacrifice-linked-exile-target-creature-until-end-step",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

/// `{2}, Sacrifice Voyager Staff`: temporarily exile target creature.
#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbility {
    ActivatedAbility {
        id: "sacrifice-linked-exile-target-creature-until-end-step",
        mana_cost: ManaCost::new(2),
        tap_cost: false,
        sorcery_speed: false,
        additional_tap_creatures: 0,
        sacrifice_source: true,
        sacrifice_creatures: 0,
        sacrifice_lands: 0,
        discard_cards: 0,
        targets: vec![TargetRequirement::Creature],
        effects: vec![Effect::ExileTargetCreatureUntilEndStep],
    }
}
"###;

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut definition = candidate_voyager_staff::definition();
    if mutation == Mutation::WrongCastCost {
        definition.mana_cost = ManaCost::new(2);
    }
    let mut ability = candidate_voyager_staff::activated_ability();
    match mutation {
        Mutation::WrongActivationCost => ability.mana_cost = ManaCost::new(1),
        Mutation::MissingSourceSacrifice => ability.sacrifice_source = false,
        Mutation::MissingDelayedReturn => ability.effects = vec![Effect::ExileTargetCreature],
        Mutation::WrongTargetRequirement => {
            ability.targets = vec![TargetRequirement::Permanent];
        }
        Mutation::None | Mutation::WrongCastCost => {}
    }
    let mut game = new_game(definition, ability)?;
    let pilot = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: WATCHWOLF_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 26,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 26,
            },
        ],
        sideboard: vec![],
    };
    let opponent = DeckList {
        mainboard: vec![
            DeckEntry {
                card: WATCHWOLF_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 24,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 32,
            },
        ],
        sideboard: vec![],
    };
    if deck_size(&pilot) != 60 || deck_size(&opponent) != 60 {
        return Err("Voyager Staff match decks must each contain exactly sixty cards".to_owned());
    }
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), 7)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), 7)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(StaffPilotPolicy::new(
            PlayerId(0),
            mutation != Mutation::MissingSourceSacrifice,
        )),
        Box::new(TargetAndPassPolicy::new(PlayerId(1))),
    ];
    let mut capture = Captured {
        events: Vec::new(),
        winner: None,
        turns: 0,
        accepted_moves: 0,
        rejected_moves: 0,
        staff: None,
        target: None,
        own_watchwolves: Vec::new(),
        staff_cast_payment: Vec::new(),
        activation_mana_before: None,
        activation_mana_after: None,
        staff_zone: None,
        target_zone: None,
        target_owner: None,
        target_controller: None,
        target_return_incarnation: None,
        linked_group_source: None,
        linked_group_source_incarnation: None,
        linked_group_controller: None,
        linked_group_due_turn: None,
        linked_group_members: Vec::new(),
        life: [0, 0],
    };
    while !game.is_game_over() {
        if game.turn > 48 || capture.accepted_moves > 8_000 {
            return Err("Voyager Staff full-game duel exceeded its bounded horizon".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Voyager Staff policy seat missing")?;
        let action = policy.propose_move(&view);
        let is_staff_activation = matches!(
            &action,
            PolicyAction::ActivateAbility { activation } if activation.ability_id == ABILITY_ID
        );
        let mana_before = if is_staff_activation {
            Some(
                game.player(PlayerId(0))
                    .map_err(|error| error.to_string())?
                    .mana_pool
                    .total(),
            )
        } else {
            None
        };
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Voyager Staff move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        capture.accepted_moves += 1;
        if let Some(before) = mana_before {
            capture.activation_mana_before = Some(before);
            capture.activation_mana_after = Some(
                game.player(PlayerId(0))
                    .map_err(|error| error.to_string())?
                    .mana_pool
                    .total(),
            );
        }
        game.validate_invariants()
            .map_err(|error| format!("Voyager Staff invariant failure: {error}"))?;
        if capture.linked_group_source.is_none() {
            for action in game.delayed_actions() {
                let DelayedActionKind::ReturnLinkedExileGroup { group } = action.kind else {
                    continue;
                };
                let Some(linked) = game.linked_exile_group(group) else {
                    continue;
                };
                if !game
                    .card_definition(linked.source)
                    .is_ok_and(|definition| definition.id == CARD_ID)
                {
                    continue;
                }
                capture.linked_group_source = Some(linked.source);
                capture.linked_group_source_incarnation = Some(linked.source_incarnation);
                capture.linked_group_controller = Some(linked.controller.0);
                capture.linked_group_due_turn = Some(action.due_turn);
                capture.linked_group_members = linked
                    .members
                    .iter()
                    .map(|member| {
                        (
                            member.object,
                            member.exile_incarnation,
                            member.role
                                == cardbench_magic_engine::LinkedExileMemberRole::PrimaryCreature,
                        )
                    })
                    .collect();
                break;
            }
        }
    }
    capture.events = game.event_log.clone();
    capture.winner = game.winner().map(|player| player.0);
    capture.turns = game.turn;
    capture.staff = capture.events.iter().find_map(|event| match event {
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
    capture.target = capture.events.iter().find_map(|event| match event {
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
    capture.own_watchwolves = capture
        .events
        .iter()
        .filter_map(|event| match event {
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == WATCHWOLF_ID) =>
            {
                Some(*card)
            }
            _ => None,
        })
        .collect();
    if let Some(staff) = capture.staff {
        capture.staff_zone = game.zone_of(staff);
        capture.staff_cast_payment = capture
            .events
            .iter()
            .find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if *card == staff => Some(colors.clone()),
                _ => None,
            })
            .unwrap_or_default();
    }
    if let Some(target) = capture.target {
        capture.target_zone = game.zone_of(target);
        capture.target_owner = game.object(target).ok().map(|object| object.owner.0);
        capture.target_controller = game.controller_of(target).ok().map(|player| player.0);
        capture.target_return_incarnation =
            game.object(target).ok().map(|object| object.incarnation);
    }
    capture.life = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    Ok(capture)
}

fn new_game(
    definition: CardDefinition,
    ability: cardbench_magic_engine::ActivatedAbility,
) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|item| item.id == CARD_ID)
        .ok_or("Voyager Staff missing from set registry")? = definition;
    let mut activated = rav_activated_ability_bindings();
    activated.retain(|binding| binding.card_definition != CARD_ID);
    activated.push(ActivatedAbilityBinding {
        card_definition: CARD_ID,
        ability,
    });
    let mut triggers = rav_triggered_ability_bindings();
    triggers.extend(attachment_triggers());
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        activated,
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

struct StaffPilotPolicy {
    player: PlayerId,
    pay_sacrifice: bool,
    staff_offered: bool,
    activation_offered: bool,
}

impl StaffPilotPolicy {
    fn new(player: PlayerId, pay_sacrifice: bool) -> Self {
        Self {
            player,
            pay_sacrifice,
            staff_offered: false,
            activation_offered: false,
        }
    }

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        view.hand
            .iter()
            .find(|card| card.definition == Some("RAV-FOREST"))
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some("RAV-PLAINS"))
            })
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }

    fn cast(
        &mut self,
        view: &GameView,
        definition: &'static str,
        cost: ManaCost,
    ) -> Option<PolicyAction> {
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(definition))?;
        if let Some(mana_selection) = select_payment(view, &cost) {
            if definition == CARD_ID {
                self.staff_offered = true;
            }
            return Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection,
            });
        }
        let desired = cost
            .colored
            .iter()
            .copied()
            .find(|color| view.mana_pool.amount(*color) == 0)
            .or_else(|| {
                (u16::from(view.mana_pool.total())
                    < u16::from(cost.generic) + cost.colored.len() as u16)
                    .then(|| cost.colored.first().copied())
                    .flatten()
            });
        let land = view
            .own_battlefield
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land)
                    && !card.tapped
                    && desired.is_none_or(|color| card.mana_colors.contains(&color))
            })
            .or_else(|| {
                view.own_battlefield.iter().find(|card| {
                    card.card_types.contains(&CardType::Land)
                        && !card.tapped
                        && !card.mana_colors.is_empty()
                })
            })?;
        let color = desired
            .filter(|color| land.mana_colors.contains(color))
            .or_else(|| land.mana_colors.iter().next().copied())?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }
}

impl CodePolicy for StaffPilotPolicy {
    fn id(&self) -> &'static str {
        "rav.voyager-staff-linked-exile-pilot.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers && !view.attackers_declared {
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(WATCHWOLF_ID) && card.can_attack)
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
        if !view.active_player.eq(&self.player) || !view.step.is_main() || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.play_land(view) {
            return action;
        }
        if !self.staff_offered {
            if let Some(action) = self.cast(view, CARD_ID, ManaCost::new(1)) {
                return action;
            }
        }
        if !self.activation_offered {
            let source = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID));
            let target = view.opponent_battlefield.iter().find(|card| {
                card.definition == Some(WATCHWOLF_ID)
                    && card.card_types.contains(&CardType::Creature)
            });
            if let (Some(source), Some(target)) = (source, target) {
                if view.mana_pool.total() < 2 {
                    if let Some(land) = view
                        .own_battlefield
                        .iter()
                        .find(|card| card.card_types.contains(&CardType::Land) && !card.tapped)
                    {
                        let color = land
                            .mana_colors
                            .iter()
                            .next()
                            .copied()
                            .unwrap_or(Color::Green);
                        return PolicyAction::ActivateManaAbility {
                            land: land.id,
                            color,
                        };
                    }
                } else {
                    self.activation_offered = true;
                    return PolicyAction::ActivateAbility {
                        activation: cardbench_magic_engine::AbilityActivation {
                            source: source.id,
                            ability_id: ABILITY_ID,
                            sacrifice_sources: if self.pay_sacrifice {
                                vec![source.id]
                            } else {
                                vec![]
                            },
                            additional_tap_creatures: vec![],
                            discard_cards: vec![],
                            targets: vec![Target::Permanent(target.id)],
                        },
                    };
                }
            }
        }
        if let Some(action) = self.cast(
            view,
            WATCHWOLF_ID,
            ManaCost::with_colors(0, [Color::Green, Color::White]),
        ) {
            return action;
        }
        PolicyAction::PassPriority
    }
}

struct TargetAndPassPolicy {
    player: PlayerId,
    target_offered: bool,
}

impl TargetAndPassPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            target_offered: false,
        }
    }
}

impl CodePolicy for TargetAndPassPolicy {
    fn id(&self) -> &'static str {
        "rav.voyager-staff-target-and-pass.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers && !view.attackers_declared {
            return PolicyAction::DeclareAttackers { attackers: vec![] };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if !view.active_player.eq(&self.player) || !view.step.is_main() || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-FOREST"))
                .or_else(|| {
                    view.hand
                        .iter()
                        .find(|card| card.definition == Some("RAV-PLAINS"))
                })
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if !self.target_offered {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(WATCHWOLF_ID))
            {
                let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
                if let Some(mana_selection) = select_payment(view, &cost) {
                    self.target_offered = true;
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection,
                    };
                }
                let missing = cost
                    .colored
                    .iter()
                    .copied()
                    .find(|color| view.mana_pool.amount(*color) == 0);
                if let Some(land) = view.own_battlefield.iter().find(|card| {
                    card.card_types.contains(&CardType::Land)
                        && !card.tapped
                        && missing.is_none_or(|color| card.mana_colors.contains(&color))
                }) {
                    let color = missing
                        .filter(|color| land.mana_colors.contains(color))
                        .or_else(|| land.mana_colors.iter().next().copied())
                        .unwrap_or(Color::Green);
                    return PolicyAction::ActivateManaAbility {
                        land: land.id,
                        color,
                    };
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn select_payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: BTreeMap<Color, u8> = Color::MANA_ALL
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
    let mut generic = Vec::with_capacity(usize::from(cost.generic));
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

fn deck_size(deck: &DeckList) -> usize {
    deck.mainboard
        .iter()
        .map(|entry| usize::from(entry.count))
        .sum()
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    let staff = captured.staff.ok_or("pilot did not cast Voyager Staff")?;
    let target = captured
        .target
        .ok_or("opponent did not cast the target Watchwolf")?;
    if captured.winner != Some(0) || captured.rejected_moves != 0 || captured.turns == 0 {
        return Err("Voyager Staff shuffled duel did not end in a clean seat-zero win".to_owned());
    }
    if captured.staff_cast_payment.len() != 1 || captured.staff_zone != Some(Zone::Graveyard) {
        return Err(
            "Voyager Staff exact {1} cast or source-sacrifice zone is incorrect".to_owned(),
        );
    }
    if captured.target_zone != Some(Zone::Battlefield)
        || captured.target_owner != Some(1)
        || captured.target_controller != Some(1)
    {
        return Err("Voyager Staff target did not return to its owner's battlefield".to_owned());
    }
    let (before, after) = captured
        .activation_mana_before
        .zip(captured.activation_mana_after)
        .ok_or("activated-cost pool snapshot missing")?;
    if before < 2 || before != after.saturating_add(2) {
        return Err("Voyager Staff activation did not spend exactly two generic mana".to_owned());
    }

    let events = &captured.events;
    let staff_paid = unique_event_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == staff && colors.len() == 1)).ok_or("Voyager Staff one-mana cast receipt missing")?;
    let staff_cast = event_index_after(events, staff_paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == staff)).ok_or("Voyager Staff cast event missing")?;
    let staff_resolved = event_index_after(
        events,
        staff_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == staff),
    )
    .ok_or("Voyager Staff spell did not resolve")?;
    let staff_entered = event_index_after(events, staff_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == staff)).ok_or("Voyager Staff did not enter the battlefield")?;

    let target_paid = unique_event_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(1), card, colors } if *card == target && colors.len() == 2 && colors.contains(&Color::Green) && colors.contains(&Color::White))).ok_or("target Watchwolf exact colored payment missing")?;
    let target_cast = event_index_after(events, target_paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == target)).ok_or("target Watchwolf cast event missing")?;
    let target_resolved = event_index_after(
        events,
        target_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == target),
    )
    .ok_or("target Watchwolf did not resolve")?;
    let target_entered = event_index_after(events, target_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == target)).ok_or("target Watchwolf did not enter battlefield")?;
    let target_incarnation = event_index_after(events, target_entered, |event| matches!(event, GameEvent::ObjectIncarnationAdvanced { object, .. } if *object == target)).ok_or("target battlefield incarnation receipt missing")?;

    let ability_paid = unique_event_index(events, |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: ABILITY_ID, mana_cost } if *source == staff && *mana_cost == ManaCost::new(2))).ok_or("Voyager Staff exact {2} ability payment missing")?;
    let sacrifice = event_index_after(events, staff_entered, |event| matches!(event, GameEvent::SacrificedAsAbilityCost { player: PlayerId(0), source, permanent } if *source == staff && *permanent == staff)).ok_or("Voyager Staff was not sacrificed as the activated cost")?;
    let staff_to_graveyard = event_index_after(events, sacrifice, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == staff)).ok_or("sacrificed Voyager Staff did not move to its graveyard")?;
    let activated = event_index_after(events, ability_paid.min(sacrifice), |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source, definition: CARD_ID, ability: ABILITY_ID, .. } if *source == staff)).ok_or("Voyager Staff activated ability did not enter the stack")?;
    if ability_paid >= activated
        || sacrifice >= activated
        || staff_to_graveyard >= activated
        || target_incarnation >= activated
    {
        return Err(
            "Voyager Staff activation costs/target were not committed before the ability stacked"
                .to_owned(),
        );
    }
    let target_exiled = event_index_after(
        events,
        activated,
        |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Exile } if *card == target),
    )
    .ok_or("ability did not exile the exact targeted creature")?;
    let exile_incarnation_index = event_index_after(events, target_exiled, |event| matches!(event, GameEvent::ObjectIncarnationAdvanced { object, .. } if *object == target)).ok_or("target exile incarnation receipt missing")?;
    let scheduled = event_index_after(events, exile_incarnation_index, |event| matches!(event, GameEvent::DelayedActionScheduled { timing: cardbench_magic_engine::DelayedActionTiming::EndStep, members, .. } if members.len() == 1 && members[0].object == target && members[0].role == cardbench_magic_engine::LinkedExileMemberRole::PrimaryCreature)).ok_or("exact target incarnation was not scheduled for the next end step")?;
    let (action_id, group_id, due_turn, exile_incarnation) = match &events[scheduled] {
        GameEvent::DelayedActionScheduled {
            action,
            group,
            due_turn,
            members,
            ..
        } => (*action, *group, *due_turn, members[0].exile_incarnation),
        _ => unreachable!(),
    };
    let activated_source_incarnation = match &events[activated] {
        GameEvent::AbilityActivated {
            source_incarnation, ..
        } => *source_incarnation,
        _ => unreachable!(),
    };
    if captured.linked_group_source != Some(staff)
        || captured.linked_group_source_incarnation != Some(activated_source_incarnation)
        || captured.linked_group_controller != Some(0)
        || captured.linked_group_due_turn != Some(due_turn)
        || captured.linked_group_members.len() != 1
        || captured.linked_group_members.first() != Some(&(target, exile_incarnation, true))
    {
        return Err("Voyager Staff delayed action lost its exact source incarnation or target group provenance".to_owned());
    }
    let resolved = event_index_after(events, scheduled, |event| matches!(event, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == staff)).ok_or("Voyager Staff activated ability did not resolve")?;
    let activation_window = event_index_before(events, scheduled, |event| matches!(event, GameEvent::StepBegan { turn, active_player: PlayerId(0), step } if *turn == due_turn && step.is_main())).ok_or("linked exile was not scheduled in its activation turn's main phase")?;
    let next_end_step = event_index_after(events, scheduled, |event| matches!(event, GameEvent::StepBegan { turn, active_player: PlayerId(0), step: Step::End } if *turn == due_turn)).ok_or("scheduled return did not reach its controller's next end step")?;
    let consumed = event_index_after(events, scheduled, |event| matches!(event, GameEvent::DelayedActionConsumed { action, group, returned } if *action == action_id && *group == group_id && returned == &vec![target])).ok_or("next-end-step delayed action did not return exactly the linked target")?;
    if next_end_step >= consumed || resolved >= consumed {
        return Err(
            "linked target returned before the ability resolved or before the scheduled end step"
                .to_owned(),
        );
    }
    let target_returned = event_index_after(events, consumed, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == target)).ok_or("exact target card did not return to the battlefield")?;
    let return_incarnation_index = event_index_after(events, target_returned, |event| matches!(event, GameEvent::ObjectIncarnationAdvanced { object, incarnation } if *object == target && *incarnation > exile_incarnation)).ok_or("returned target did not acquire a new incarnation")?;
    let target_return_incarnation = match &events[return_incarnation_index] {
        GameEvent::ObjectIncarnationAdvanced { incarnation, .. } => *incarnation,
        _ => unreachable!(),
    };
    let original_incarnation = match &events[target_incarnation] {
        GameEvent::ObjectIncarnationAdvanced { incarnation, .. } => *incarnation,
        _ => unreachable!(),
    };
    if original_incarnation >= exile_incarnation
        || target_return_incarnation <= exile_incarnation
        || captured.target_return_incarnation != Some(target_return_incarnation)
    {
        return Err(
            "Voyager Staff target did not follow distinct battlefield/exile/return incarnations"
                .to_owned(),
        );
    }

    let end = unique_event_index(events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("full-game terminal win receipt missing")?;
    let final_damage = event_index_before(events, end, |event| matches!(event, GameEvent::DamageDealtToPlayer { player: PlayerId(1), source, .. } if captured.own_watchwolves.contains(source))).ok_or("seat-zero Watchwolf damage did not produce the terminal result")?;
    let attack = event_index_before(events, final_damage, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if captured.own_watchwolves.iter().any(|watchwolf| attackers.contains(watchwolf)))).ok_or("pilot Watchwolf did not attack before the terminal damage")?;

    let selected = BTreeSet::from([
        staff_paid,
        staff_cast,
        staff_resolved,
        staff_entered,
        target_paid,
        target_cast,
        target_resolved,
        target_entered,
        target_incarnation,
        ability_paid,
        sacrifice,
        staff_to_graveyard,
        activated,
        resolved,
        target_exiled,
        exile_incarnation_index,
        scheduled,
        activation_window,
        next_end_step,
        consumed,
        target_returned,
        return_incarnation_index,
        attack,
        final_damage,
        end,
    ]);
    let projected: Vec<_> = project_events(events)
        .into_iter()
        .enumerate()
        .filter_map(|(index, event)| selected.contains(&index).then_some(event))
        .collect();
    let owner = captured.target_owner.ok_or("target owner missing")?;
    let controller = captured
        .target_controller
        .ok_or("returned target controller missing")?;
    let turns_life = captured.life;
    Ok(json!({
        "trace_id": "rav-voyager-staff-linked-exile-next-end-step-full-game",
        "shuffle_seed": seed,
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "full_game_event_count": events.len(),
        "witness_event_count": selected.len(),
        "life_at_end": [{"seat":0,"life":turns_life[0]},{"seat":1,"life":turns_life[1]}],
        "mechanics": {
            "staff_object": staff.0,
            "staff_definition_id": CARD_ID,
            "staff_cast_cost": "{1}",
            "staff_cast_payment": captured.staff_cast_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "staff_zone_after_activation": captured.staff_zone.map(|zone| format!("{zone:?}")),
            "activation_ability": ABILITY_ID,
            "activation_cost": "{2}",
            "activation_mana_paid": before.saturating_sub(after),
            "activation_source_sacrificed": true,
            "target_object": target.0,
            "target_definition_id": WATCHWOLF_ID,
            "target_owner": owner,
            "target_controller_after_return": controller,
            "target_original_incarnation": original_incarnation,
            "target_exile_incarnation": exile_incarnation,
            "target_return_incarnation": target_return_incarnation,
            "target_zone_after_return": captured.target_zone.map(|zone| format!("{zone:?}")),
            "delayed_action": {"timing":"EndStep","due_turn":due_turn,"action_id":action_id.0,"group_id":group_id.0,"returned_objects":[target.0]},
            "linked_group_source": captured.linked_group_source.map(|object| object.0),
            "linked_group_source_incarnation": captured.linked_group_source_incarnation,
            "linked_group_controller": captured.linked_group_controller,
            "linked_group_members": captured.linked_group_members.iter().map(|(object, incarnation, primary)| json!({"object":object.0,"exile_incarnation":incarnation,"primary_creature":primary})).collect::<Vec<_>>(),
            "returned_to_owner": owner == 1 && controller == 1,
            "new_incarnation_on_return": target_return_incarnation > exile_incarnation,
        },
        "staff_cast_sequence": staff_paid,
        "target_cast_sequence": target_cast,
        "ability_payment_sequence": ability_paid,
        "activation_sequence": activated,
        "sacrifice_sequence": sacrifice,
        "ability_resolution_sequence": resolved,
        "exile_sequence": target_exiled,
        "schedule_sequence": scheduled,
        "activation_window_sequence": activation_window,
        "next_end_step_sequence": next_end_step,
        "return_sequence": consumed,
        "return_move_sequence": target_returned,
        "terminal_sequence": end,
        "events": projected
    }))
}

fn event_index_after(
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

fn event_index_before(
    events: &[GameEvent],
    before: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .take(before)
        .rev()
        .find_map(|(index, event)| predicate(event).then_some(index))
}

fn unique_event_index(
    events: &[GameEvent],
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    let mut matches = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 274;
    json!({
        "collector_number": card.collector_number,
        "name": card.name,
        "definition_id": definition_id,
        "engine_status": if definition_id.is_some() { "executable_compatibility_slice" } else { "catalog_only" },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "qualification": if passed { "unreviewed_diagnostic" } else { "not_evaluated" },
        "credited": false
    })
}
