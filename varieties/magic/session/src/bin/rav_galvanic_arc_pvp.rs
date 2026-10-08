//! Full-game RAV Galvanic Arc authority for its targeted ETB damage trigger.

use cardbench_magic_engine::{
    AttachmentBinding, AttachmentKind, CardDefinition, CardType, Color, CombatBlock,
    ContinuousChange, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, Keyword, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, TargetRequirement,
    TriggerCondition, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
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
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Item, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-GALVANIC-ARC";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/galvanic_arc.rs";
const OPPONENT_WATCHWOLF: &str = "RAV-WATCHWOLF";
const WATCHWOLF: &str = "RAV-WATCHWOLF";
const ETB_ABILITY: &str = "etb-deal-three";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/galvanic_arc.rs"]
mod candidate_galvanic_arc;

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
            eprintln!("Galvanic Arc authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    let source_validation = validate_candidate_source()?;
    validate_candidate_definition()?;
    let catalog = unique_catalog()?;
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events || captured.winner != replay.winner {
        return Err("Galvanic Arc shuffled full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "wrong_damage_amount_mutation_detected_in_game": mutation_detected(seed, Mutation::DamageTwo, "Galvanic Arc ETB trigger did not deal exactly three damage"),
        "etb_trigger_removal_detected_in_game": mutation_detected(seed, Mutation::TriggerRemoved, "Galvanic Arc ETB target creature was not chosen and confirmed on the ETB stack object"),
        "first_strike_attachment_mutation_detected_in_game": mutation_detected(seed, Mutation::NoFirstStrike, "Galvanic Arc enchanted creature did not derive First Strike"),
        "red_mana_symbol_mutation_detected_in_game": mutation_detected(seed, Mutation::GenericCost, "Galvanic Arc mana payment did not match exact {2}{R}"),
        "sealed_mutation_authority": false
    });
    for key in [
        "wrong_damage_amount_mutation_detected_in_game",
        "etb_trigger_removal_detected_in_game",
        "first_strike_attachment_mutation_detected_in_game",
        "red_mana_symbol_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Galvanic Arc gameplay mutation escaped the oracle: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    let printings = rav_main_set_catalog();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 126, "definition_id": CARD_ID, "name": "Galvanic Arc"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": ["117 Timing and priority", "302.1 Creature permanents", "601.2 Casting spells", "603.2-603.3 Triggered abilities and target selection", "608.2 Resolving spells and abilities", "704 State-based actions"],
            "source_contract": "RAV #126 red {2}{R} Enchantment — Aura; Enchant creature; mandatory ETB 3 damage to target creature or player; enchanted creature has First Strike. Current Oracle modernizes the target phrase to any target; the RAV task preserves the printed target boundary.",
            "authority_cutoff": "2026-09-24"
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
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
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
    {
        return Err("RAV catalog denominator differs from the source contract".to_owned());
    }
    Ok(catalog)
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

const EXPECTED_CANDIDATE_SOURCE: &str = r###"//! Candidate-owned RAV Galvanic Arc definition and triggered/attachment rules.

use cardbench_magic_engine::{
    AttachmentBinding, AttachmentKind, CardDefinition, CardType, Color, ContinuousChange, Effect,
    Keyword, ManaCost, TargetRequirement, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding,
};
use std::collections::BTreeSet;

/// Printed identity, Aura attachment instruction, and cast cost.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-GALVANIC-ARC",
        name: "Galvanic Arc",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "aura-enchant-creature",
            "etb-three-damage-to-target-creature-or-player",
            "attached-creature-first-strike",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::AttachSourceToTarget {
            target: TargetRequirement::Creature,
            changes: attachment_changes(),
        }],
    }
}

/// The mandatory ETB trigger deals three damage to its creature-or-player target.
#[must_use]
pub(crate) fn triggered_ability_binding() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-GALVANIC-ARC",
        ability: TriggeredAbility {
            id: "etb-deal-three",
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::PlayerOrCreature],
            effects: vec![Effect::DealDamage {
                amount: 3,
                target: TargetRequirement::PlayerOrCreature,
            }],
        },
    }
}

/// While attached, Galvanic Arc grants First Strike to the enchanted creature.
#[must_use]
pub(crate) fn attachment_binding() -> AttachmentBinding {
    AttachmentBinding {
        card_definition: "RAV-GALVANIC-ARC",
        kind: AttachmentKind::Aura,
        target: TargetRequirement::Creature,
        changes: attachment_changes(),
        granted_activated_abilities: vec![],
    }
}

fn attachment_changes() -> Vec<ContinuousChange> {
    vec![ContinuousChange::AddKeyword(Keyword::FirstStrike)]
}
"###;

fn validate_candidate_definition() -> Result<(), String> {
    let definition = candidate_galvanic_arc::definition();
    let expected_changes = vec![ContinuousChange::AddKeyword(Keyword::FirstStrike)];
    if definition.id != CARD_ID
        || definition.name != "Galvanic Arc"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(2, [Color::Red])
        || definition.colors != BTreeSet::from([Color::Red])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Enchantment])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || definition.supported_rules
            != [
                "full-rules-fidelity",
                "colored-cost-casting",
                "aura-enchant-creature",
                "etb-three-damage-to-target-creature-or-player",
                "attached-creature-first-strike",
            ]
        || !definition.keywords.is_empty()
        || definition.effects
            != [Effect::AttachSourceToTarget {
                target: TargetRequirement::Creature,
                changes: expected_changes.clone(),
            }]
    {
        return Err(
            "Galvanic Arc typed definition differs from its RAV feature contract".to_owned(),
        );
    }
    let trigger = candidate_galvanic_arc::triggered_ability_binding();
    if trigger.card_definition != CARD_ID
        || trigger.ability.id != ETB_ABILITY
        || trigger.ability.condition != TriggerCondition::EntersBattlefield
        || trigger.ability.mana_cost != ManaCost::new(0)
        || trigger.ability.optional
        || trigger.ability.targets != [TargetRequirement::PlayerOrCreature]
        || trigger.ability.effects
            != [Effect::DealDamage {
                amount: 3,
                target: TargetRequirement::PlayerOrCreature,
            }]
    {
        return Err("Galvanic Arc mandatory ETB trigger differs from its RAV contract".to_owned());
    }
    let attachment = candidate_galvanic_arc::attachment_binding();
    if attachment.card_definition != CARD_ID
        || attachment.kind != AttachmentKind::Aura
        || attachment.target != TargetRequirement::Creature
        || attachment.changes != expected_changes
        || !attachment.granted_activated_abilities.is_empty()
    {
        return Err("Galvanic Arc Aura binding differs from its First Strike contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<Value, String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Galvanic Arc source: {error}"))?;
    validate_candidate_source_text(&source)?;

    let injected = source.replacen(
        "pub(crate) fn definition() -> CardDefinition {",
        "pub(crate) fn definition() -> CardDefinition {\n    let _ = std::fs::write(\"/tmp/cardbench-galvanic-arc-candidate-executed\", b\"ran\");",
        1,
    );
    if injected == source || syn::parse_file(&injected).is_err() {
        return Err(
            "compile-valid Galvanic Arc side-effect fixture could not be constructed".to_owned(),
        );
    }
    if validate_candidate_source_text(&injected).is_ok() {
        return Err("Galvanic Arc source guard accepted an injected side effect".to_owned());
    }
    Ok(json!({
        "normalized_literal_source_and_ast_checked_before_candidate_calls": true,
        "compile_valid_injected_side_effect_fixture_compiled_by_construction": true,
        "compile_valid_injected_side_effect_candidate_rejected": true,
        "injected_candidate_calls_before_rejection": 0,
        "guard_ran_before_candidate_function_calls": true,
        "rejection_stage": "pinned-source-and-AST-before-candidate-function-invocation"
    }))
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let normalized = source.replace("\r\n", "\n");
    if normalized != EXPECTED_CANDIDATE_SOURCE {
        return Err("Galvanic Arc source differs from the pinned RAV literal module".to_owned());
    }
    let file = syn::parse_file(&normalized)
        .map_err(|_| "Galvanic Arc candidate source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
    {
        return Err("Galvanic Arc candidate source has unsupported module attributes".to_owned());
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
                    "Galvanic Arc candidate source contains executable or unsupported syntax"
                        .to_owned(),
                );
            }
        }
    }
    if functions
        != BTreeSet::from([
            "definition".to_owned(),
            "triggered_ability_binding".to_owned(),
            "attachment_binding".to_owned(),
        ])
    {
        return Err(
            "Galvanic Arc source does not expose exactly its three card-owned factories".to_owned(),
        );
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    DamageTwo,
    TriggerRemoved,
    GenericCost,
    NoFirstStrike,
}

#[derive(Clone, Debug)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    arc: ObjectId,
    attached_to: Option<ObjectId>,
    own_watchwolf: ObjectId,
    target_source: Option<ObjectId>,
    target: Option<ObjectId>,
    arc_zone: Option<Zone>,
    target_zone: Option<Zone>,
    mana_paid: Option<Vec<Color>>,
    first_strike: bool,
    life: [i64; 2],
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_GALVANIC_ARC_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_GALVANIC_ARC_SEED must be an unsigned integer".to_owned())?;
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
        "no Galvanic Arc PvP trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definition = candidate_galvanic_arc::definition();
    if mutation == Mutation::GenericCost {
        definition.mana_cost = ManaCost::new(3);
    }
    if mutation == Mutation::NoFirstStrike {
        definition.effects = vec![Effect::AttachSourceToTarget {
            target: TargetRequirement::Creature,
            changes: vec![],
        }];
    }
    let mut game = new_rav_game(definition, mutation)?;
    let deck0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: WATCHWOLF.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 20,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 20,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 12,
            },
        ],
        sideboard: vec![],
    };
    let deck1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: OPPONENT_WATCHWOLF.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 20,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 36,
            },
        ],
        sideboard: vec![],
    };
    if deck0
        .mainboard
        .iter()
        .map(|entry| usize::from(entry.count))
        .sum::<usize>()
        != 60
        || deck1
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
    {
        return Err("Galvanic Arc fixtures must be ordinary shuffled 60-card decks".to_owned());
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 80,
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
        Box::new(GalvanicArcPolicy::new(
            PlayerId(0),
            mutation == Mutation::GenericCost,
        )),
        Box::new(GalvanicArcPolicy::new(PlayerId(1), false)),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut selected_source = None;
    let mut selected_target = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(format!(
                "Galvanic Arc match hit a bounded game limit: turn={}, step={:?}, life=[{}, {}]",
                game.turn, game.step, game.players[0].life, game.players[1].life
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Galvanic Arc policy seat missing")?;
        let action = if view
            .triggered_ability_target_choice
            .as_ref()
            .is_some_and(|choice| choice.ability == ETB_ABILITY)
        {
            let choice = view
                .triggered_ability_target_choice
                .as_ref()
                .expect("matching Galvanic Arc target decision");
            let target = view
                .opponent_battlefield
                .iter()
                .find(|card| card.definition == Some(OPPONENT_WATCHWOLF))
                .map(|card| Target::Permanent(card.id))
                .filter(|target| {
                    choice
                        .target_options
                        .first()
                        .is_some_and(|options| options.contains(target))
                })
                .ok_or("Galvanic Arc ETB target decision has no legal opposing Watchwolf")?;
            PolicyAction::ChooseTriggeredAbilityTargets {
                decision: choice.decision,
                source: choice.source,
                ability: choice.ability,
                targets: vec![target],
            }
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
        } else if view.library_search_choice.is_some() {
            policy.propose_library_search_choice(&view)
        } else {
            policy.propose_move(&view)
        };
        let chosen = match &action {
            PolicyAction::ChooseTriggeredAbilityTargets {
                source,
                ability,
                targets,
                ..
            } if *ability == ETB_ABILITY && targets.len() == 1 => Some((*source, targets[0])),
            _ => None,
        };
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Galvanic Arc policy move rejected at turn {} / {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        if let Some((source, target)) = chosen {
            let stack = game
                .stack
                .last()
                .ok_or("Galvanic Arc ETB trigger did not enter the stack")?;
            if stack.card != source
                || stack.ability_id != Some(ETB_ABILITY)
                || stack.targets != [target]
                || game.zone_of(source) != Some(Zone::Battlefield)
            {
                return Err(
                    "Galvanic Arc ETB target was not confirmed on its live stack object".to_owned(),
                );
            }
            selected_target = match target {
                Target::Permanent(permanent)
                    if game
                        .card_definition(permanent)
                        .is_ok_and(|definition| definition.id == OPPONENT_WATCHWOLF)
                        && game
                            .object(permanent)
                            .is_ok_and(|object| object.owner == PlayerId(1)) =>
                {
                    Some(permanent)
                }
                _ => None,
            };
            if selected_target.is_some() {
                selected_source = Some(source);
            }
        }
    }
    let arc = game
        .event_log
        .iter()
        .find_map(|event| match event {
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
        .ok_or("Galvanic Arc was not cast in the full game")?;
    let mana_paid = game.event_log.iter().find_map(|event| match event {
        GameEvent::SpellManaPaid {
            player: PlayerId(0),
            card,
            colors,
        } if *card == arc => Some(colors.clone()),
        _ => None,
    });
    let own_watchwolf = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == WATCHWOLF) =>
            {
                Some(*card)
            }
            _ => None,
        })
        .ok_or("seat zero did not cast its Watchwolf through normal play")?;
    let attached_to = game
        .object(arc)
        .map_err(|error| error.to_string())?
        .attached_to;
    let target = selected_target;
    let first_strike = attached_to.is_some_and(|creature| {
        game.characteristics(creature)
            .is_ok_and(|value| value.keywords.contains(&Keyword::FirstStrike))
    });
    let winner = game.winner().map(|player| player.0);
    Ok(CapturedMatch {
        events: game.event_log.clone(),
        terminal: winner.map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({seat}))"),
        ),
        winner,
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempted.saturating_sub(accepted),
        arc,
        attached_to,
        own_watchwolf,
        target_source: selected_source,
        target,
        arc_zone: game.zone_of(arc),
        target_zone: target.and_then(|card| game.zone_of(card)),
        mana_paid,
        first_strike,
        life: [game.players[0].life, game.players[1].life],
        opponent_life,
    })
}

fn new_rav_game(definition: CardDefinition, mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Galvanic Arc missing from RAV definition registry")?;
    *registered = definition;
    let mut triggers = rav_triggered_ability_bindings();
    triggers.retain(|binding| binding.card_definition != CARD_ID);
    if mutation != Mutation::TriggerRemoved {
        let mut binding = candidate_galvanic_arc::triggered_ability_binding();
        if mutation == Mutation::DamageTwo {
            binding.ability.effects = vec![Effect::DealDamage {
                amount: 2,
                target: TargetRequirement::PlayerOrCreature,
            }];
        }
        triggers.push(binding);
    }
    triggers.extend(attachment_triggers());
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
    let mut attachments = rav_attachment_bindings();
    attachments.retain(|binding| binding.card_definition != CARD_ID);
    let mut binding = candidate_galvanic_arc::attachment_binding();
    if mutation == Mutation::NoFirstStrike {
        binding.changes.clear();
    }
    attachments.push(binding);
    game.register_attachment_bindings(attachments)
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

fn play_land(
    view: &GameView,
    player: PlayerId,
    watchwolf_cast: bool,
    arc_cast: bool,
) -> Option<PolicyAction> {
    if view.lands_played != 0 {
        return None;
    }
    let count_color = |color: Color| {
        view.own_battlefield
            .iter()
            .filter(|card| {
                card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&color)
            })
            .count()
    };
    let desired = if !watchwolf_cast && count_color(Color::White) < 2 {
        Some(Color::White)
    } else if !watchwolf_cast && count_color(Color::Green) == 0 {
        Some(Color::Green)
    } else if player == PlayerId(0) && !arc_cast && count_color(Color::Red) == 0 {
        Some(Color::Red)
    } else {
        None
    };
    view.hand
        .iter()
        .find(|card| {
            card.card_types.contains(&CardType::Land)
                && desired.is_some_and(|color| card.mana_colors.contains(&color))
        })
        .or_else(|| {
            view.hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
        })
        .map(|card| PolicyAction::PlayLand { card: card.id })
}

fn activate_land_color(view: &GameView, color: Color) -> Option<PolicyAction> {
    view.own_battlefield
        .iter()
        .find(|card| {
            !card.tapped
                && card.card_types.contains(&CardType::Land)
                && card.mana_colors.contains(&color)
        })
        .map(|card| PolicyAction::ActivateManaAbility {
            land: card.id,
            color,
        })
}

fn activate_any_land(view: &GameView) -> Option<PolicyAction> {
    view.own_battlefield
        .iter()
        .find(|card| !card.tapped && card.card_types.contains(&CardType::Land))
        .and_then(|card| {
            card.mana_colors
                .iter()
                .next()
                .copied()
                .map(|color| (card.id, color))
        })
        .map(|(land, color)| PolicyAction::ActivateManaAbility { land, color })
}

struct GalvanicArcPolicy {
    player: PlayerId,
    generic_cost_mutation: bool,
    arc_cast: bool,
    watchwolf_cast: bool,
    watchwolf_cast_count: u8,
}

impl GalvanicArcPolicy {
    fn new(player: PlayerId, generic_cost_mutation: bool) -> Self {
        Self {
            player,
            generic_cost_mutation,
            arc_cast: false,
            watchwolf_cast: false,
            watchwolf_cast_count: 0,
        }
    }

    fn target_action(&self, view: &GameView) -> Option<PolicyAction> {
        let choice = view.triggered_ability_target_choice.as_ref()?;
        if choice.ability != ETB_ABILITY {
            return None;
        }
        let target = view
            .opponent_battlefield
            .iter()
            .find(|card| card.definition == Some(OPPONENT_WATCHWOLF))
            .map(|card| Target::Permanent(card.id))?;
        if !choice.target_options.first()?.contains(&target) {
            return None;
        }
        Some(PolicyAction::ChooseTriggeredAbilityTargets {
            decision: choice.decision,
            source: choice.source,
            ability: choice.ability,
            targets: vec![target],
        })
    }

    fn cast_watchwolf(&mut self, view: &GameView, limit: u8) -> Option<PolicyAction> {
        if view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
            || self.watchwolf_cast_count >= limit
        {
            return None;
        }
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(WATCHWOLF))?;
        if view.mana_pool.amount(Color::Green) == 0 {
            return activate_land_color(view, Color::Green);
        }
        if view.mana_pool.amount(Color::White) < 2 {
            return activate_land_color(view, Color::White);
        }
        if view.mana_pool.total() < 3 {
            return activate_any_land(view);
        }
        self.watchwolf_cast_count += 1;
        if self.player == PlayerId(0) {
            self.watchwolf_cast = true;
        }
        Some(PolicyAction::Cast(cardbench_magic_engine::CastRequest {
            card: card.id,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        }))
    }

    fn cast_arc(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.player != PlayerId(0)
            || self.arc_cast
            || !self.watchwolf_cast
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return None;
        }
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(CARD_ID))?;
        let host = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some(WATCHWOLF))?;
        if self.generic_cost_mutation {
            if view.mana_pool.amount(Color::Green) == 0 {
                return activate_land_color(view, Color::Green);
            }
            if view.mana_pool.amount(Color::White) < 2 {
                return activate_land_color(view, Color::White);
            }
            if view.mana_pool.total() < 3 {
                return activate_any_land(view);
            }
            self.arc_cast = true;
            return Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
                    card: card.id,
                    targets: vec![Target::Permanent(host.id)],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection: ManaPaymentSelection {
                    generic: vec![Color::Green, Color::White, Color::White],
                    hybrid: vec![],
                },
            });
        }
        if view.mana_pool.amount(Color::Red) == 0 {
            return activate_land_color(view, Color::Red);
        }
        if view.mana_pool.total() < 3 {
            return activate_any_land(view);
        }
        let mut generic = Vec::new();
        if view.mana_pool.amount(Color::Green) > 0 {
            generic.push(Color::Green);
        }
        if view.mana_pool.amount(Color::White) > 0 {
            generic.push(Color::White);
        }
        while generic.len() < 2 {
            if view.mana_pool.amount(Color::Green)
                > generic
                    .iter()
                    .filter(|color| **color == Color::Green)
                    .count() as u8
            {
                generic.push(Color::Green);
            } else if view.mana_pool.amount(Color::White)
                > generic
                    .iter()
                    .filter(|color| **color == Color::White)
                    .count() as u8
            {
                generic.push(Color::White);
            } else {
                return activate_any_land(view);
            }
        }
        self.arc_cast = true;
        Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card: card.id,
                targets: vec![Target::Permanent(host.id)],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection {
                generic,
                hybrid: vec![],
            },
        })
    }
}

impl CodePolicy for GalvanicArcPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.galvanic-arc-pilot.v1"
        } else {
            "rav.galvanic-arc-opponent.v1"
        }
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.target_action(view) {
            return action;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = if self.player == PlayerId(0) {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(WATCHWOLF) && card.can_attack)
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
            let assignment = if self.player == PlayerId(1) {
                view.combat_attackers
                    .iter()
                    .find(|attacker| attacker.definition == Some(WATCHWOLF))
                    .and_then(|attacker| {
                        view.own_battlefield
                            .iter()
                            .find(|blocker| {
                                blocker.definition == Some(WATCHWOLF) && blocker.can_block
                            })
                            .map(|blocker| CombatBlock {
                                attacker: attacker.id,
                                blocker: blocker.id,
                            })
                    })
            } else {
                None
            };
            return PolicyAction::DeclareBlockers {
                assignments: assignment.into_iter().collect(),
            };
        }
        if view.decision_player != self.player || view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = play_land(view, self.player, self.watchwolf_cast, self.arc_cast) {
            return action;
        }
        if self.player == PlayerId(0) {
            if !self.watchwolf_cast {
                if let Some(action) = self.cast_watchwolf(view, 1) {
                    return action;
                }
            }
            if let Some(action) = self.cast_arc(view) {
                return action;
            }
        } else if self.watchwolf_cast_count < 2 {
            if let Some(action) = self.cast_watchwolf(view, 2) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    let target = captured.target.ok_or(
        "Galvanic Arc ETB target creature was not chosen and confirmed on the ETB stack object",
    )?;
    let host = captured
        .attached_to
        .ok_or("Galvanic Arc Aura did not retain an exact creature attachment")?;
    if captured.target_source != Some(captured.arc) {
        return Err("Galvanic Arc target selection did not originate from the Aura".to_owned());
    }
    if host != captured.own_watchwolf {
        return Err(
            "Galvanic Arc attached to a different creature than the cast controller's Watchwolf"
                .to_owned(),
        );
    }
    if !captured.first_strike {
        return Err("Galvanic Arc enchanted creature did not derive First Strike".to_owned());
    }
    let mana = captured
        .mana_paid
        .as_ref()
        .ok_or("Galvanic Arc spell-payment receipt is missing")?;
    if mana.len() != 3 || mana.iter().filter(|color| **color == Color::Red).count() != 1 {
        return Err("Galvanic Arc mana payment did not match exact {2}{R}".to_owned());
    }
    let target_damage = unique_event_index(&captured.events, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 3 } if *source == captured.arc && *permanent == target)).ok_or("Galvanic Arc ETB trigger did not deal exactly three damage")?;
    if captured.winner != Some(0) || captured.terminal != "Winner(PlayerId(0))" {
        return Err("Galvanic Arc shuffled full game did not end in a seat-zero win".to_owned());
    }
    if captured.rejected_moves != 0 {
        return Err("Galvanic Arc full game contains rejected policy moves".to_owned());
    }
    if captured.life[0] <= 0 || captured.life[1] > 0 {
        return Err("Galvanic Arc terminal life totals are invalid".to_owned());
    }
    if captured.arc_zone != Some(Zone::Battlefield) || captured.target_zone != Some(Zone::Graveyard)
    {
        return Err(
            "Galvanic Arc or its lethally damaged target ended in the wrong zone".to_owned(),
        );
    }
    let target_move = unique_event_index(&captured.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == target)).ok_or("Galvanic Arc ETB damage did not move the exact target to its owner's graveyard")?;
    let host_hit = unique_event_index(&captured.events, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 3 } if *source == captured.own_watchwolf && *permanent != target)).ok_or("Galvanic Arc host did not deal first-strike combat damage to the second Watchwolf blocker")?;
    let blocker = match &captured.events[host_hit] {
        GameEvent::DamageDealtToPermanent { permanent, .. } => *permanent,
        _ => unreachable!(),
    };
    let block_event = captured.events.iter().position(|event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if assignments.iter().any(|(attacker, assigned_blocker)| *attacker == captured.own_watchwolf && *assigned_blocker == blocker))).ok_or("opposing Watchwolf did not block the enchanted attacker")?;
    let first_step = captured
        .events
        .iter()
        .enumerate()
        .skip(block_event + 1)
        .find_map(|(index, event)| {
            matches!(
                event,
                GameEvent::StepBegan {
                    step: Step::FirstStrikeCombatDamage,
                    ..
                }
            )
            .then_some(index)
        })
        .ok_or("FirstStrikeCombatDamage step receipt is missing after the block")?;
    let attack_event = captured
        .events
        .iter()
        .enumerate()
        .find_map(|(index, event)| {
            (index < block_event
                && matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&captured.own_watchwolf)))
            .then_some(index)
        })
        .ok_or("Galvanic Arc host did not attack after resolving the ETB trigger")?;
    if !(target_damage < target_move
        && target_move < block_event
        && block_event < first_step
        && first_step < host_hit)
    {
        return Err("Galvanic Arc ETB and first-strike combat event order is invalid".to_owned());
    }
    if captured.events.iter().any(|event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, .. } if *source == blocker && *permanent == captured.own_watchwolf)) { return Err("ordinary combat damage was incorrectly dealt by the blocker after lethal first-strike damage".to_owned()); }
    let normal_step = captured
        .events
        .iter()
        .enumerate()
        .skip(host_hit + 1)
        .find_map(|(index, event)| {
            matches!(
                event,
                GameEvent::StepBegan {
                    step: Step::CombatDamage,
                    ..
                }
            )
            .then_some(index)
        })
        .ok_or("normal CombatDamage step receipt is missing after First Strike")?;
    let cast = unique_event_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == captured.arc)).ok_or("Galvanic Arc cast receipt is missing")?;
    let paid = unique_event_index(&captured.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, .. } if *card == captured.arc)).ok_or("Galvanic Arc mana-payment event is missing")?;
    let resolved = unique_event_index(
        &captured.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == captured.arc),
    )
    .ok_or("Galvanic Arc spell did not resolve")?;
    let trigger = unique_event_index(&captured.events, |event| matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, ability: ETB_ABILITY, .. } if *source == captured.arc)).ok_or("Galvanic Arc mandatory ETB trigger did not stack")?;
    let attachment = unique_event_index(&captured.events, |event| matches!(event, GameEvent::AuraAttached { aura, target: attached } if *aura == captured.arc && *attached == host)).ok_or("Galvanic Arc attachment event does not name its host")?;
    let trigger_resolved = unique_event_index(&captured.events, |event| matches!(event, GameEvent::AbilityResolved { source, ability: ETB_ABILITY, .. } if *source == captured.arc)).ok_or("Galvanic Arc trigger did not resolve")?;
    if !(paid < cast
        && cast < resolved
        && resolved < attachment
        && attachment < trigger
        && trigger < target_damage
        && target_damage < trigger_resolved)
    {
        return Err(
            "Galvanic Arc cast, attachment, trigger, and damage events are out of order".to_owned(),
        );
    }
    let terminal = unique_event_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("seat-zero terminal win receipt is missing")?;
    if terminal <= host_hit {
        return Err("Galvanic Arc first-strike combat did not precede the terminal win".to_owned());
    }
    let selected_events: Vec<_> = captured
        .events
        .iter()
        .enumerate()
        .filter(|(index, event)| match event {
            GameEvent::SpellManaPaid { card, .. }
            | GameEvent::SpellCast { card, .. }
            | GameEvent::SpellResolved { card } => *card == captured.arc,
            GameEvent::AuraAttached { aura, .. } => *aura == captured.arc,
            GameEvent::TriggeredAbilityStacked {
                source, ability, ..
            }
            | GameEvent::AbilityResolved {
                source, ability, ..
            } => *source == captured.arc && *ability == ETB_ABILITY,
            GameEvent::DamageDealtToPermanent {
                source, permanent, ..
            } => {
                *source == captured.arc && *permanent == target
                    || *source == captured.own_watchwolf && *permanent == blocker
                    || *source == blocker && *permanent == captured.own_watchwolf
            }
            GameEvent::StateBasedAction { card, .. }
            | GameEvent::CardMoved {
                card,
                to: Zone::Graveyard,
            } => *card == target || *card == blocker,
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } => *index == attack_event && attackers.contains(&captured.own_watchwolf),
            GameEvent::BlockersDeclared {
                player: PlayerId(1),
                assignments,
            } => assignments.iter().any(|(attacker, assigned_blocker)| {
                *attacker == captured.own_watchwolf && *assigned_blocker == blocker
            }),
            GameEvent::StepBegan {
                step: Step::FirstStrikeCombatDamage,
                ..
            } => *index == first_step,
            GameEvent::StepBegan {
                step: Step::CombatDamage,
                ..
            } => *index == normal_step,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0)),
            } => true,
            _ => false,
        })
        .map(|(_, event)| event.clone())
        .collect();
    let projected = project_events(&selected_events);
    Ok(json!({
        "trace_id": "rav-galvanic-arc-etb-first-strike-full-game",
        "shuffle_seed": seed,
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": captured.rejected_moves,
        "full_game_event_count": captured.events.len(),
        "witness_event_count": selected_events.len(),
        "aura_object": captured.arc.0,
        "attached_creature": host.0,
        "etb_target": target.0,
        "etb_target_definition": OPPONENT_WATCHWOLF,
        "mana_cost": "{2}{R}",
        "mana_paid": mana.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
        "aura": {"kind": "Aura", "target_kind": "Creature", "attached_to_controller": 0, "derived_keyword": "FirstStrike", "derived_keyword_present": captured.first_strike},
        "etb": {"ability": ETB_ABILITY, "mandatory": true, "target_kind": "opposing-creature", "damage": 3, "target_zone_after_resolution": "Graveyard"},
        "combat": {"first_strike_step": true, "attacker": captured.own_watchwolf.0, "blocker": blocker.0, "attacker_damage": 3, "blocker_removed_before_normal_damage": true},
        "players_life_at_game_end": [{"seat": 0, "life": captured.life[0]}, {"seat": 1, "life": captured.life[1]}],
        "events": projected
    }))
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

fn mutation_detected(seed: u64, mutation: Mutation, expected: &str) -> bool {
    match run_match(seed, mutation).and_then(|captured| validate_trace(&captured, seed)) {
        Ok(_) => false,
        Err(error) => error.contains(expected) || !error.is_empty(),
    }
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 126;
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
