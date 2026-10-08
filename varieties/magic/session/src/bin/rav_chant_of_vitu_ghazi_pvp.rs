//! Full-game diagnostic authority for RAV Chant of Vitu-Ghazi.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, ConvokeContribution, ConvokePayment,
    DamageReplacementChoice, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, Keyword,
    ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    card_definitions, rav_activated_ability_bindings, rav_activated_ability_cost_modifier_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings,
    rav_attachment_triggered_ability_bindings, rav_basic_land_type_bindings,
    rav_cost_reduction_bindings, rav_damage_replacement_effect_bindings,
    rav_entry_coin_flip_bindings, rav_entry_copy_bindings,
    rav_generalized_activated_ability_cost_bindings, rav_land_entry_bindings,
    rav_legendary_permanent_bindings, rav_mana_ability_bindings,
    rav_mana_ability_cost_bindings, rav_replacement_effect_bindings,
    rav_static_attack_restriction_bindings, rav_static_continuous_effect_bindings,
    rav_static_creature_spell_cost_modifier_bindings, rav_static_entry_restriction_bindings,
    rav_static_library_top_reveal_bindings, rav_triggered_ability_bindings,
    rav_main_set_catalog,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use syn::{Expr, Item, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-CHANT-OF-VITU-GHAZI";
const CARD_NAME: &str = "Chant of Vitu-Ghazi";
const CAREGIVER_ID: &str = "RAV-CAREGIVER";
const HAWK_ID: &str = "RAV-COURIER-HAWK";
const ELF_ID: &str = "RAV-ELVES-OF-DEEP-SHADOW";
const SPELUNKERS_ID: &str = "RAV-GOBLIN-SPELUNKERS";
const SPARKMAGE_ID: &str = "RAV-SPARKMAGE-APPRENTICE";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/chant_of_vitu_ghazi.rs";
const SPARKMAGE_ABILITY: &str = "etb-deal-one";
const DEFAULT_TRACE_SEED: u64 = 52;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/chant_of_vitu_ghazi.rs"]
mod candidate_chant_of_vitu_ghazi;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            println!("{}", serde_json::to_string(&report).expect("report serializes"));
            if report["passed"].as_bool() == Some(true) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("Chant of Vitu-Ghazi PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_registry()?;
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<_> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
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
    if captured != replay {
        return Err("Chant of Vitu-Ghazi deterministic full-game replay differed".to_owned());
    }
    let mutations = json!({
        "convoke_removed_mutation_detected": mutation_detected(seed, Mutation::NoConvoke),
        "creature_prevention_removed_mutation_detected": mutation_detected(seed, Mutation::NoPrevention),
        "combat_only_prevention_mutation_detected": mutation_detected(seed, Mutation::CombatOnly),
        "sealed_mutation_authority": false
    });
    for key in [
        "convoke_removed_mutation_detected",
        "creature_prevention_removed_mutation_detected",
        "combat_only_prevention_mutation_detected",
    ] {
        if mutations[key] != true {
            return Err(format!("Chant of Vitu-Ghazi semantic mutant escaped: {key}"));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 7, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": ["119.1 and 119.4 Life totals and life gain", "120.1-120.4 Damage", "601.2 Casting spells and paying costs", "608.2 Resolving spells", "615 Damage prevention effects", "702.50 Convoke", "508.1 Declaring attackers", "510 Combat damage"],
            "source_contract": "RAV Chant of Vitu-Ghazi #7, {6}{W}{W} white Instant with Convoke; prevent all damage that would be dealt by creatures this turn and gain life equal to damage actually prevented by this effect.",
            "authority_cutoff": "2026-09-24",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "deterministic_replay": true,
        "cards_passed": 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "diagnostic_only; external qualification pending",
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoConvoke,
    NoPrevention,
    CombatOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PreventedPacket {
    source: ObjectId,
    target: Target,
    amount: i32,
    replacement_id: u64,
    replacement_index: usize,
    damage_index: usize,
    prevention_index: usize,
    life_gain_index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    chant: Option<ObjectId>,
    chant_zone: Option<Zone>,
    mana_paid: Vec<Color>,
    convokers: Vec<ObjectId>,
    convoker_tapped: Vec<bool>,
    resolved: bool,
    prevention_id: Option<u64>,
    life_when_prevention_installed: Option<i64>,
    prevention_expired: bool,
    packets: Vec<PreventedPacket>,
    life_gains: Vec<(i32, usize)>,
    attackers: Vec<ObjectId>,
    sparkmage: Option<ObjectId>,
    sparkmage_targeted: bool,
    life: [i64; 2],
}

fn candidate_value(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_chant_of_vitu_ghazi::definition();
    match mutation {
        Mutation::None => {}
        Mutation::NoConvoke => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Convoke),
        Mutation::NoPrevention => definition.effects.clear(),
        Mutation::CombatOnly => {
            definition.effects = vec![Effect::PreventAllCombatDamageUntilEndOfTurn];
        }
    }
    definition
}

fn validate_candidate_registry() -> Result<(), String> {
    let definition = candidate_chant_of_vitu_ghazi::definition();
    let runtime = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Chant of Vitu-Ghazi is missing from the RAV definition registry")?;
    if runtime != definition
        || definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(6, [Color::White, Color::White])
        || definition.colors != BTreeSet::from([Color::White])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Instant])
        || definition.is_basic_land
        || definition.supported_rules
            != [
                "full-rules-fidelity",
                "convoke",
                "creature-damage-prevention",
                "life-gain-equal-to-damage-prevented",
            ]
        || definition.power.is_some()
        || definition.toughness.is_some()
        || definition.keywords != [Keyword::Convoke]
        || definition.effects
            != [Effect::PreventAllCreatureDamageAndGainLifeEqualToDamagePreventedUntilEndOfTurn]
    {
        return Err("Chant of Vitu-Ghazi identity, exact {6}{W}{W} cost, white Instant, Convoke, creature-damage prevention, or prevented-damage life gain differs from source contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Chant of Vitu-Ghazi source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Chant of Vitu-Ghazi source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
        || file.items.len() != 3
        || !source.contains("PreventAllCreatureDamageAndGainLifeEqualToDamagePreventedUntilEndOfTurn")
        || source.contains("GainLifeForEachCreature")
    {
        return Err("Chant of Vitu-Ghazi identity, exact {6}{W}{W} cost, white Instant, Convoke, creature-damage prevention, or prevented-damage life gain differs from source contract".to_owned());
    }
    let mut use_count = 0;
    let mut definition_count = 0;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                use_count += 1;
            }
            Item::Fn(function) if function.sig.ident == "definition" => {
                definition_count += 1;
                if !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || function.sig.inputs.len() != 0
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || !function.sig.generics.params.is_empty()
                    || function.block.stmts.len() != 1
                    || !matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(_), None))
                {
                    return Err("Chant of Vitu-Ghazi must use one private direct-literal definition function".to_owned());
                }
            }
            _ => {
                return Err("Chant of Vitu-Ghazi module contains executable or unsupported items".to_owned());
            }
        }
    }
    if use_count != 2 || definition_count != 1 {
        return Err("Chant of Vitu-Ghazi module imports or definition shape differ from the source contract".to_owned());
    }
    Ok(())
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "agent"
    } else {
        "reference"
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

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Chant of Vitu-Ghazi definition missing during game construction")? =
        candidate_value(mutation);
    let mut triggers = rav_triggered_ability_bindings();
    triggers.extend(rav_attachment_triggered_ability_bindings());
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
    game.validate_invariants().map_err(|error| error.to_string())?;
    Ok(game)
}

fn find_trace() -> Result<(u64, Captured), String> {
    let seed = match std::env::var("CARDBENCH_CHANT_OF_VITU_GHAZI_SEED") {
        Ok(value) => value.parse::<u64>().map_err(|_| {
            "CARDBENCH_CHANT_OF_VITU_GHAZI_SEED must be an unsigned integer".to_owned()
        })?,
        Err(_) => DEFAULT_TRACE_SEED,
    };
    let captured = run_match(seed, Mutation::None)?;
    validate_trace(&captured, seed)?;
    Ok((seed, captured))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut game = new_game(mutation)?;
    let p0 = deck(&[
        (CARD_ID, 4),
        (CAREGIVER_ID, 4),
        (HAWK_ID, 4),
        ("RAV-PLAINS", 48),
    ]);
    let p1 = deck(&[
        (ELF_ID, 4),
        (SPELUNKERS_ID, 4),
        (SPARKMAGE_ID, 4),
        ("RAV-FOREST", 24),
        ("RAV-MOUNTAIN", 24),
    ]);
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    for (player, list) in [(PlayerId(0), &p0), (PlayerId(1), &p1)] {
        if list
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
        {
            return Err("Chant of Vitu-Ghazi fixtures must be legal 60-card decks".to_owned());
        }
        game.load_deck_into_library(player, list)
            .map_err(|error| error.to_string())?;
        game.draw_opening_hand(player, 7)
            .map_err(|error| error.to_string())?;
    }
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(ChantPolicy::new(PlayerId(0))),
        Box::new(ChantPolicy::new(PlayerId(1))),
    ];
    let mut captured = Captured {
        events: vec![],
        winner: None,
        turns: 0,
        accepted: 0,
        rejected: 0,
        chant: None,
        chant_zone: None,
        mana_paid: vec![],
        convokers: vec![],
        convoker_tapped: vec![],
        resolved: false,
        prevention_id: None,
        life_when_prevention_installed: None,
        prevention_expired: false,
        packets: vec![],
        life_gains: vec![],
        attackers: vec![],
        sparkmage: None,
        sparkmage_targeted: false,
        life: [20, 20],
    };
    while !game.is_game_over() {
        if game.turn > 60 || captured.accepted >= 5000 {
            return Err(format!(
                "Chant of Vitu-Ghazi full-game trace exceeded its bound at turn {} / {:?}; chant={:?} resolved={} prevention={:?} attackers={} sparkmage={:?} life=[{}, {}]",
                game.turn, game.step, captured.chant, captured.resolved,
                captured.prevention_id, captured.attackers.len(), captured.sparkmage,
                game.player(PlayerId(0)).map(|p| p.life).unwrap_or_default(),
                game.player(PlayerId(1)).map(|p| p.life).unwrap_or_default()
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Chant policy seat missing")?;
        let action = if view
            .triggered_ability_target_choice
            .as_ref()
            .is_some_and(|choice| choice.ability == SPARKMAGE_ABILITY)
        {
            let choice = view
                .triggered_ability_target_choice
                .as_ref()
                .expect("matching Sparkmage target decision");
            let target = Target::Player(PlayerId(0));
            if !choice
                .target_options
                .first()
                .is_some_and(|options| options.contains(&target))
            {
                return Err("Sparkmage target decision does not include seat zero".to_owned());
            }
            PolicyAction::ChooseTriggeredAbilityTargets {
                decision: choice.decision,
                source: choice.source,
                ability: choice.ability,
                targets: vec![target],
            }
        } else if let Some(action) = policy.propose_pending_decision(&view) {
            action
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
        let chosen_target = match &action {
            PolicyAction::ChooseTriggeredAbilityTargets {
                source,
                ability,
                targets,
                ..
            } if *ability == SPARKMAGE_ABILITY && targets == &[Target::Player(PlayerId(0))] => {
                Some(*source)
            }
            _ => None,
        };
        let before = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Chant policy move rejected on turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        captured.accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        if game.turn > 14 && !captured.attackers.is_empty() && captured.chant.is_none() {
            return Err("P0 had no Chant available to cast before the turn-14 creature combat damage".to_owned());
        }
        if let Some(source) = chosen_target {
            if game.stack.last().is_none_or(|stack| {
                stack.card != source
                    || stack.targets != [Target::Player(PlayerId(0))]
            }) {
                return Err("Sparkmage target did not remain bound to its live trigger".to_owned());
            }
            captured.sparkmage_targeted = true;
        }
        observe(&game, before, &game.event_log[before..], &mut captured)?;
        captured
            .events
            .extend_from_slice(&game.event_log[before..]);
        if game.turn > 14 && !captured.sparkmage_targeted {
            return Err("P1 did not resolve Sparkmage's creature damage during Chant's duration".to_owned());
        }
    }
    captured.events = game.event_log.clone();
    captured.winner = game.winner().map(|winner| winner.0);
    captured.turns = game.turn;
    captured.chant_zone = captured.chant.and_then(|card| game.zone_of(card));
    captured.life = [
        game.player(PlayerId(0)).map_err(|e| e.to_string())?.life,
        game.player(PlayerId(1)).map_err(|e| e.to_string())?.life,
    ];
    Ok(captured)
}

fn observe(
    game: &Game,
    absolute_start: usize,
    events: &[GameEvent],
    captured: &mut Captured,
) -> Result<(), String> {
    for event in events {
        match event {
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                captured.chant = Some(*card);
            }
            GameEvent::SpellCast {
                player: PlayerId(1),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == SPARKMAGE_ID) =>
            {
                captured.sparkmage = Some(*card);
            }
            GameEvent::ConvokeUsed {
                player: PlayerId(0),
                creature,
                contribution: None,
            } => {
                captured.convokers.push(*creature);
                captured.convoker_tapped.push(
                    game.object(*creature)
                        .map_err(|error| error.to_string())?
                        .tapped,
                );
            }
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if Some(*card) == captured.chant
                || game.card_definition(*card).is_ok_and(|d| d.id == CARD_ID) =>
            {
                captured.mana_paid = colors.clone()
            }
            GameEvent::SpellResolved { card } if Some(*card) == captured.chant => {
                captured.resolved = true;
            }
            GameEvent::GlobalCreatureDamagePreventionCreated {
                id,
                source,
                controller: PlayerId(0),
                ..
            } if Some(*source) == captured.chant => {
                captured.prevention_id = Some(*id);
                captured.life_when_prevention_installed = Some(
                    game.player(PlayerId(0))
                        .map_err(|error| error.to_string())?
                        .life,
                );
            }
            GameEvent::DamageReplacementApplied {
                replacement: DamageReplacementChoice::CreatureDamagePrevention { id, source },
                ..
            } if Some(*id) == captured.prevention_id && Some(*source) == captured.chant => {}
            GameEvent::CreatureDamagePrevented {
                source,
                prevented_by,
                controller: PlayerId(0),
                target,
                amount,
                replacement_id,
                ..
            } if Some(*prevented_by) == captured.chant => {
                let local_index = events.iter().position(|item| std::ptr::eq(item, event)).unwrap_or(0);
                let absolute_prevention_index = absolute_start + local_index;
                let applied_index = events[..local_index]
                    .iter()
                    .rposition(|prior| matches!(prior,
                        GameEvent::DamageReplacementApplied {
                            replacement: DamageReplacementChoice::CreatureDamagePrevention { id, source }, ..
                        } if *id == *replacement_id && Some(*id) == captured.prevention_id && Some(*source) == captured.chant
                    ))
                    .map(|index| absolute_start + index)
                    .ok_or("creature prevention receipt has no applied replacement receipt")?;
                let damage_index = events[..local_index]
                    .iter()
                    .rposition(|prior| matches!(prior,
                        GameEvent::DamagePrevented {
                            source: damage_source, target: damage_target, amount: damage_amount
                        } if damage_source == source && damage_target == target && damage_amount == amount
                    ))
                    .map(|index| absolute_start + index)
                    .ok_or("creature prevention receipt has no damage-prevented receipt")?;
                captured.packets.push(PreventedPacket {
                    source: *source,
                    target: *target,
                    amount: *amount,
                    replacement_id: *replacement_id,
                    replacement_index: applied_index,
                    damage_index,
                    prevention_index: absolute_prevention_index,
                    life_gain_index: usize::MAX,
                });
            }
            GameEvent::LifeGained {
                player: PlayerId(0),
                amount,
            } if captured.prevention_id.is_some() => {
                captured.life_gains.push((*amount, absolute_start + events.iter().position(|item| std::ptr::eq(item, event)).unwrap_or(0)));
            }
            GameEvent::GlobalCreatureDamagePreventionExpired { id, source, .. }
                if Some(*id) == captured.prevention_id && Some(*source) == captured.chant =>
            {
                captured.prevention_expired = true;
            }
            GameEvent::AttackersDeclared {
                player: PlayerId(1),
                attackers,
            } if game.turn == 14 && !attackers.is_empty() => {
                captured.attackers = attackers.clone();
            }
            _ => {}
        }
    }
    let start = absolute_start;
    for (offset, event) in events.iter().enumerate() {
        if let GameEvent::LifeGained {
            player: PlayerId(0),
            amount,
        } = event
            && captured.prevention_id.is_some()
        {
            if let Some(packet) = captured.packets.iter_mut().find(|packet| {
                packet.life_gain_index == usize::MAX
                    && packet.prevention_index < start + offset
                    && packet.amount == *amount
            }) {
                packet.life_gain_index = start + offset;
            }
        }
    }
    Ok(())
}

struct ChantPolicy {
    player: PlayerId,
    chant_cast: bool,
    sparkmage_cast: bool,
}

impl ChantPolicy {
    const fn new(player: PlayerId) -> Self {
        Self {
            player,
            chant_cast: false,
            sparkmage_cast: false,
        }
    }

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let preferred = if self.player == PlayerId(0) {
            Color::White
        } else {
            let forest = view
                .own_battlefield
                .iter()
                .filter(|card| card.mana_colors.contains(&Color::Green))
                .count();
            let mountain = view
                .own_battlefield
                .iter()
                .filter(|card| card.mana_colors.contains(&Color::Red))
                .count();
            if forest <= mountain {
                Color::Green
            } else {
                Color::Red
            }
        };
        view.hand
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&preferred)
            })
            .or_else(|| view.hand.iter().find(|card| card.card_types.contains(&CardType::Land)))
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }

    fn make_mana(&self, view: &GameView, color: Color) -> Option<PolicyAction> {
        view.own_battlefield
            .iter()
            .find(|card| {
                !card.tapped
                    && card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&color)
            })
            .map(|land| PolicyAction::ActivateManaAbility {
                land: land.id,
                color,
            })
    }

    fn pool_total(view: &GameView) -> u16 {
        [Color::White, Color::Blue, Color::Black, Color::Red, Color::Green]
            .into_iter()
            .map(|color| u16::from(view.mana_pool.amount(color)))
            .sum()
    }

    fn cast_creature(&self, view: &GameView) -> Option<PolicyAction> {
        if self.player == PlayerId(0) {
            let own_lands = view
                .own_battlefield
                .iter()
                .filter(|card| card.card_types.contains(&CardType::Land))
                .count();
            if !self.chant_cast && own_lands >= 7 {
                return None;
            }
            let creature = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CAREGIVER_ID))
                .or_else(|| view.hand.iter().find(|card| card.definition == Some(HAWK_ID)))?;
            let cost = if creature.definition == Some(CAREGIVER_ID) {
                1
            } else {
                2
            };
            if view.mana_pool.amount(Color::White) == 0 {
                return self.make_mana(view, Color::White);
            }
            if Self::pool_total(view) < cost {
                return [Color::White, Color::White, Color::White]
                    .into_iter()
                    .find_map(|color| self.make_mana(view, color));
            }
            return Some(PolicyAction::Cast(CastRequest {
                card: creature.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            }));
        }

        if view.turn >= 14 || view.active_player != self.player {
            return None;
        }
        if let Some(card) = view.hand.iter().find(|card| card.definition == Some(ELF_ID)) {
            if view.mana_pool.amount(Color::Green) == 0 {
                return self.make_mana(view, Color::Green);
            }
            return Some(PolicyAction::Cast(CastRequest {
                card: card.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            }));
        }
        if let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some(SPELUNKERS_ID))
        {
            if view.mana_pool.amount(Color::Red) == 0 {
                return self.make_mana(view, Color::Red);
            }
            if Self::pool_total(view) < 3 {
                return self
                    .make_mana(view, Color::Green)
                    .or_else(|| self.make_mana(view, Color::Red));
            }
            return Some(PolicyAction::Cast(CastRequest {
                card: card.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            }));
        }
        None
    }

    fn cast_chant(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.player != PlayerId(0)
            || self.chant_cast
            || view.active_player != PlayerId(1)
            || view.step != Step::DeclareAttackers
            || !view.attackers_declared
            || view.turn != 14
            || view.stack_depth != 0
        {
            return None;
        }
        let card = view.hand.iter().find(|card| card.definition == Some(CARD_ID))?;
        if view.mana_pool.amount(Color::White) < 7 {
            return self.make_mana(view, Color::White);
        }
        let convoker = view
            .own_battlefield
            .iter()
            .find(|card| card.can_block && !card.tapped && !card.card_types.contains(&CardType::Land))?;
        self.chant_cast = true;
        Some(PolicyAction::CastWithPayment {
            request: CastRequest {
                card: card.id,
                targets: vec![],
                convoke: vec![ConvokePayment {
                    creature: convoker.id,
                    contribution: ConvokeContribution::Generic,
                }],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection {
                generic: vec![Color::White; 5],
                hybrid: vec![],
            },
        })
    }

    fn cast_sparkmage(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.player != PlayerId(1)
            || self.sparkmage_cast
            || view.active_player != self.player
            || view.turn != 14
            || view.step != Step::PostcombatMain
            || view.stack_depth != 0
        {
            return None;
        }
        let card = view.hand.iter().find(|card| card.definition == Some(SPARKMAGE_ID))?;
        if view.mana_pool.amount(Color::Red) == 0 {
            return self.make_mana(view, Color::Red);
        }
        if Self::pool_total(view) < 2 {
            return self
                .make_mana(view, Color::Green)
                .or_else(|| self.make_mana(view, Color::Red));
        }
        self.sparkmage_cast = true;
        Some(PolicyAction::Cast(CastRequest {
            card: card.id,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        }))
    }
}

impl CodePolicy for ChantPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.chant_of_vitu_ghazi-convoke-damage-prevention.v1"
        } else {
            "rav.chant_of_vitu_ghazi-creature-damage-opponent.v1"
        }
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.cast_chant(view) {
            return action;
        }
        if let Some(action) = self.cast_sparkmage(view) {
            return action;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attack = if self.player == PlayerId(1) && view.turn == 14 {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.can_attack)
                    .map(|card| card.id)
                    .collect()
            } else if self.player == PlayerId(0) && self.chant_cast {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(HAWK_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect()
            } else {
                vec![]
            };
            return PolicyAction::DeclareAttackers { attackers: attack };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
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
        if let Some(action) = self.cast_creature(view) {
            return action;
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.winner != Some(0) || captured.turns <= 14 {
        return Err("Chant of Vitu-Ghazi full game did not finish after its turn-14 prevention trace".to_owned());
    }
    let terminal = unique_index(&captured.events, |event| {
        matches!(event, GameEvent::GameEnded { winner: Some(PlayerId(0)) })
    })
    .ok_or("Chant of Vitu-Ghazi full-game terminal event is missing")?;
    if captured.accepted == 0 || captured.rejected != 0 {
        return Err("Chant of Vitu-Ghazi trace has invalid policy accounting".to_owned());
    }
    let chant = captured.chant.ok_or("Chant was not cast")?;
    if captured.chant_zone != Some(Zone::Graveyard) || !captured.resolved {
        return Err("Chant did not resolve and leave the stack normally".to_owned());
    }
    if captured.mana_paid.len() != 7
        || captured.mana_paid.iter().any(|color| *color != Color::White)
        || captured.convokers.len() != 1
        || captured.convoker_tapped != [true]
        || captured.prevention_id.is_none()
        || !captured.prevention_expired
        || captured.attackers.is_empty()
        || !captured.sparkmage_targeted
    {
        return Err(format!(
            "Chant cost, Convoke, prevention duration, combat, or Sparkmage facts are incomplete: mana={}, convokers={}, prevention_id={:?}, expired={}, attackers={}, sparkmage_targeted={}",
            captured.mana_paid.len(), captured.convokers.len(), captured.prevention_id,
            captured.prevention_expired, captured.attackers.len(), captured.sparkmage_targeted
        ));
    }
    let installed_life = captured
        .life_when_prevention_installed
        .ok_or("Chant prevention lifetime was not observed")?;
    if captured.packets.len() < 2 {
        return Err("Chant did not prevent both combat and noncombat creature damage".to_owned());
    }
    let combat_packets: Vec<_> = captured
        .packets
        .iter()
        .filter(|packet| captured.attackers.contains(&packet.source))
        .collect();
    let sparkmage = captured.sparkmage.ok_or("Sparkmage was not cast after Chant")?;
    let noncombat = captured
        .packets
        .iter()
        .find(|packet| packet.source == sparkmage && packet.target == Target::Player(PlayerId(0)))
        .ok_or("Chant did not prevent Sparkmage's noncombat creature damage to seat zero")?;
    if combat_packets.is_empty() || noncombat.amount != 1 {
        return Err("Chant prevention packets do not include combat and one Sparkmage damage".to_owned());
    }
    let prevented: i32 = captured.packets.iter().map(|packet| packet.amount).sum();
    let gained: i32 = captured.life_gains.iter().map(|(amount, _)| *amount).sum();
    if prevented != gained
        || i64::from(gained) != captured.life[0] - installed_life
        || captured.life[1] > 0
    {
        return Err(format!(
            "prevented damage and Chant life gain diverged (prevented {prevented}, gained {gained}, life {} -> {})",
            installed_life, captured.life[0]
        ));
    }
    for packet in &captured.packets {
        if !(packet.replacement_index < packet.damage_index
            && packet.damage_index < packet.prevention_index
            && packet.prevention_index + 1 == packet.life_gain_index)
        {
            return Err("replacement, creature-prevention, and life-gain event order is invalid".to_owned());
        }
    }
    let created = unique_index(&captured.events, |event| {
        matches!(event, GameEvent::GlobalCreatureDamagePreventionCreated {
            id, source, controller: PlayerId(0), expires_turn: 14
        } if Some(*id) == captured.prevention_id && *source == chant)
    })
    .ok_or("Chant did not install one controller-owned turn-scoped creature prevention")?;
    let expired = unique_index(&captured.events, |event| {
        matches!(event, GameEvent::GlobalCreatureDamagePreventionExpired {
            id, source, controller: PlayerId(0)
        } if Some(*id) == captured.prevention_id && *source == chant)
    })
    .ok_or("Chant's exact creature-prevention record did not expire")?;
    let spell_resolved = unique_index(&captured.events, |event| {
        matches!(event, GameEvent::SpellResolved { card } if *card == chant)
    })
    .ok_or("Chant resolution event is missing")?;
    if !(created < spell_resolved
        && captured.packets.iter().all(|packet| created < packet.replacement_index
            && packet.life_gain_index < expired))
    {
        return Err("Chant lifetime creation, packet replacements, or expiry order is invalid".to_owned());
    }
    let chant_graveyard = unique_index(&captured.events, |event| {
        matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == chant)
    })
    .ok_or("resolved Chant did not move from the stack to the graveyard")?;
    if !(spell_resolved < chant_graveyard && chant_graveyard < terminal) {
        return Err("Chant resolution, graveyard move, and terminal event order is invalid".to_owned());
    }
    let cast = unique_index(&captured.events, |event| {
        matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == chant)
    })
    .ok_or("Chant cast receipt is missing")?;
    let paid = unique_index(&captured.events, |event| {
        matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors }
            if *card == chant && colors.len() == 7 && colors.iter().all(|color| *color == Color::White))
    })
    .ok_or("Chant's seven-white-mana payment receipt is missing")?;
    let convoke = unique_index(&captured.events, |event| {
        matches!(event, GameEvent::ConvokeUsed { player: PlayerId(0), contribution: None, .. })
    })
    .ok_or("Chant's generic Convoke payment event is missing")?;
    let attackers = unique_index(&captured.events, |event| {
        matches!(event, GameEvent::AttackersDeclared { player: PlayerId(1), attackers }
            if !attackers.is_empty())
    })
    .ok_or("opponent attacker declaration event is missing")?;
    let spark_cast = unique_index(&captured.events, |event| {
        matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if Some(*card) == captured.sparkmage)
    })
    .ok_or("post-combat Sparkmage cast event is missing")?;
    let spark_trigger = unique_index(&captured.events, |event| {
        matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(1), source, ability: SPARKMAGE_ABILITY, .. }
            if Some(*source) == captured.sparkmage)
    })
    .ok_or("post-combat Sparkmage ETB trigger event is missing")?;
    let spark_packet = captured
        .packets
        .iter()
        .find(|packet| packet.source == sparkmage && packet.target == Target::Player(PlayerId(0)))
        .ok_or("post-combat Sparkmage damage prevention event is missing")?;
    if !(attackers < convoke && convoke < paid && paid < cast
        && created < spell_resolved
        && spark_cast < spark_trigger
        && spark_trigger < spark_packet.replacement_index)
    {
        return Err("Chant payment/resolution or Sparkmage trigger/prevention event order is invalid".to_owned());
    }
    let projected = project_events(&captured.events);
    let typed_prevented = projected
        .iter()
        .filter(|event| event.kind == "CreatureDamagePrevented")
        .count();
    let typed_life = projected
        .iter()
        .filter(|event| event.kind == "LifeGained")
        .count();
    if typed_prevented != captured.packets.len() || typed_life < captured.life_gains.len() {
        return Err("typed session projection omitted Chant prevention or life-gain events".to_owned());
    }
    let mut relevant_event_indices = BTreeSet::from([
        paid,
        cast,
        convoke,
        created,
        spell_resolved,
        chant_graveyard,
        expired,
        attackers,
        spark_cast,
        spark_trigger,
        terminal,
    ]);
    for packet in &captured.packets {
        relevant_event_indices.extend([
            packet.replacement_index,
            packet.damage_index,
            packet.prevention_index,
            packet.life_gain_index,
        ]);
    }
    let relevant_typed_events = relevant_event_indices
        .iter()
        .filter_map(|index| projected.get(*index))
        .cloned()
        .collect::<Vec<_>>();
    let relevant_canonical_events = relevant_event_indices
        .iter()
        .filter_map(|index| captured.events.get(*index).map(|event| json!({
            "sequence": index,
            "canonical": format!("{event:?}"),
        })))
        .collect::<Vec<_>>();
    Ok(json!({
        "trace_id": "rav-chant-of-vitu-ghazi-convoke-prevent-creature-damage-gain-life-terminal",
        "passed": true,
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": captured.winner,
        "seed": seed,
        "turns": captured.turns,
        "winner": captured.winner,
        "decks": ["rav_chant_vitu_ghazi_white_60", "rav_creature_damage_red_green_60"],
        "policies": ["rav.chant_of_vitu_ghazi-convoke-damage-prevention.v1", "rav.chant_of_vitu_ghazi-creature-damage-opponent.v1"],
        "terminal_event_sequence": terminal,
        "accepted_moves": captured.accepted,
        "rejected_moves": captured.rejected,
        "spell": chant.0,
        "spell_zone": format!("{:?}", captured.chant_zone),
        "exact_payment_colors": captured.mana_paid.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
        "convoke_contributors": captured.convokers.iter().map(|id| id.0).collect::<Vec<_>>(),
        "convoke_contributors_tapped": captured.convoker_tapped,
        "prevention_record_id": captured.prevention_id,
        "combat_attackers": captured.attackers.iter().map(|id| id.0).collect::<Vec<_>>(),
        "creature_damage_prevented": captured.packets.iter().map(|packet| json!({
            "source": packet.source.0,
            "target": format!("{:?}", packet.target),
            "amount": packet.amount,
            "replacement_id": packet.replacement_id,
            "replacement_event_sequence": packet.replacement_index,
            "damage_prevented_event_sequence": packet.damage_index,
            "prevention_event_sequence": packet.prevention_index,
            "life_gain_event_sequence": packet.life_gain_index,
        })).collect::<Vec<_>>(),
        "prevented_damage_total": prevented,
        "life_gained_total": gained,
        "life_when_prevention_installed": installed_life,
        "life_at_terminal": captured.life,
        "sparkmage_source": sparkmage.0,
        "sparkmage_noncombat_damage_prevented": noncombat.amount,
        "global_prevention_expired": captured.prevention_expired,
        "chant_graveyard_event_sequence": chant_graveyard,
        "typed_event_trace": relevant_typed_events,
        "canonical_event_trace": relevant_canonical_events,
    }))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation) {
        Ok(captured) => validate_trace(&captured, seed).is_err(),
        Err(_) => true,
    }
}

fn unique_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    let mut found = events
        .iter()
        .enumerate()
        .filter(|(_, event)| predicate(event))
        .map(|(index, _)| index);
    let first = found.next()?;
    found.next().is_none().then_some(first)
}

fn card_status(card: &CatalogCard) -> Value {
    json!({
        "collector_number": card.collector_number,
        "name": card.name,
        "status": format!("{:?}", card.semantic_status),
        "passed": card.name == CARD_NAME,
        "credited": false
    })
}
