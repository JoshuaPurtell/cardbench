//! Full-game diagnostic authority for the RAV Faith's Fetters task.

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, CombatBlock, ContinuousChange, DeckEntry, DeckList, Effect,
    Game, GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId,
    PolicyAction, Step, Target, TargetRequirement, TriggerCondition, Zone,
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
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-FAITHS-FETTERS";
const CARD_NAME: &str = "Faith's Fetters";
const TARGET_ID: &str = "RAV-SELESNYA-GUILDMAGE";
const ATTACKER_ID: &str = "RAV-WATCHWOLF";
const TRIGGER_ID: &str = "etb-gain-four-life";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/faiths_fetters.rs";
const MAX_TRACE_SEED: u64 = 32;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/faiths_fetters.rs"]
mod candidate_faiths_fetters;

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
            eprintln!("Faith's Fetters PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_bindings()?;
    let catalog = unique_catalog()?;
    let seed = find_trace_seed()?;
    let capture = run_match(seed, Mutation::None)?;
    validate_trace(&capture)?;
    let replay = run_match(seed, Mutation::None)?;
    if capture != replay {
        return Err("Faith's Fetters shuffled full-game replay differed".to_owned());
    }
    let mutation_sensitivity = json!({
        "missing_aura_restrictions_detected_in_game": mutation_detected(seed, Mutation::NoAuraEffect),
        "etb_life_amount_changed_detected_in_game": mutation_detected(seed, Mutation::WrongGain),
        "etb_trigger_removed_detected_in_game": mutation_detected(seed, Mutation::NoTrigger),
        "sealed_mutation_authority": false
    });
    for key in [
        "missing_aura_restrictions_detected_in_game",
        "etb_life_amount_changed_detected_in_game",
        "etb_trigger_removed_detected_in_game",
    ] {
        if mutation_sensitivity[key] != true {
            return Err(format!(
                "Faith's Fetters mutation escaped gameplay oracle: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1","task_id":TASK_ID,"expansion":"RAV",
        "card":{"collector_number":16,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},"candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{"comprehensive_rules":"2026-08-07","checked_sections":["303.4 Auras","601.2 Casting spells","603.2-603.3 Triggered abilities","608.2 Resolving spells and abilities","613 Continuous effects","509 Attacker and blocker declarations","119 Life totals"],"source_contract":"CardBench-authored semantic summary: this white Aura enchants a permanent, prevents its attacks and blocks, suppresses its nonmana activated abilities, and its mandatory ETB trigger gains four life.","authority_cutoff":"2026-09-24"},
        "passed":true,"pilot_card_passed":true,"cards_passed":1,"cards_credited":0,"cards_total":catalog.len(),"printings_total":rav_main_set_catalog().len(),
        "coverage_complete":false,"full_reward":false,"harbor_reward":0.0,"pilot_reward":0.0,"qualification_status":"unreviewed_diagnostic",
        "mutation_sensitivity":mutation_sensitivity,"traces":[trace_json(&capture)],"per_card_breakdown":breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|e| format!("could not read Faith's Fetters source: {e}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Faith's Fetters source is not valid Rust".to_owned())?;
    if !file.attrs.iter().all(|a| a.path().is_ident("doc")) {
        return Err("Faith's Fetters source has unsupported module attributes".to_owned());
    }
    let expected: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "ContinuousChange"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TargetRequirement"],
        vec!["cardbench_magic_engine", "TriggerCondition"],
        vec!["cardbench_magic_engine", "TriggeredAbility"],
        vec!["cardbench_magic_engine", "TriggeredAbilityBinding"],
    ]
    .into_iter()
    .map(|p| p.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let mut paths = Vec::new();
                if item.leading_colon.is_some() || !flatten_use_tree(&[], &item.tree, &mut paths) {
                    return Err("Faith's Fetters imports differ from source contract".to_owned());
                }
                imports.extend(paths)
            }
            Item::Fn(item)
                if matches!(&item.vis,Visibility::Restricted(v)if v.path.is_ident("crate"))
                    && item.sig.inputs.is_empty()
                    && item.sig.generics.params.is_empty()
                    && item.sig.generics.where_clause.is_none()
                    && item.block.stmts.len() == 1 =>
            {
                let output = match item.sig.ident.to_string().as_str() {
                    "definition" => "CardDefinition",
                    "triggered_ability" => "TriggeredAbilityBinding",
                    _ => return Err("Faith's Fetters has unsupported function".to_owned()),
                };
                let Stmt::Expr(Expr::Struct(literal), None) = &item.block.stmts[0] else {
                    return Err("Faith's Fetters must return direct typed literals".to_owned());
                };
                if !type_is(&item.sig.output, output)
                    || !path_is_type(&literal.path, output)
                    || literal.rest.is_some()
                {
                    return Err("Faith's Fetters must return direct typed literals".to_owned());
                }
                functions.insert(item.sig.ident.to_string());
            }
            _ => {
                return Err(
                    "Faith's Fetters contains an unsupported top-level source item".to_owned(),
                );
            }
        }
    }
    if imports != expected
        || functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()])
        || file.items.len() != 4
    {
        return Err("Faith's Fetters module structure differs from source contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_bindings() -> Result<(), String> {
    let definition = candidate_faiths_fetters::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|d| d.id == CARD_ID)
        .ok_or("Faith's Fetters definition is missing from RAV registry")?;
    if definition != registered
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(3, [Color::White])
        || definition.colors != BTreeSet::from([Color::White])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Enchantment])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
        || definition.effects
            != [Effect::AttachSourceToTarget {
                target: TargetRequirement::Permanent,
                changes: vec![
                    ContinuousChange::AddKeyword(Keyword::CannotAttackOrBlock),
                    ContinuousChange::SuppressNonManaActivatedAbilities,
                ],
            }]
    {
        return Err("Faith's Fetters literal is outside the task source contract".to_owned());
    }
    let binding = candidate_faiths_fetters::triggered_ability();
    let registered = rav_triggered_ability_bindings()
        .into_iter()
        .find(|b| b.card_definition == CARD_ID)
        .ok_or("Faith's Fetters ETB trigger is missing from RAV registry")?;
    if binding != registered
        || binding.ability.id != TRIGGER_ID
        || binding.ability.condition != TriggerCondition::EntersBattlefield
        || binding.ability.mana_cost != ManaCost::new(0)
        || binding.ability.optional
        || !binding.ability.targets.is_empty()
        || binding.ability.effects != [Effect::GainLifeController { amount: 4 }]
    {
        return Err("Faith's Fetters trigger literal is outside task source contract".to_owned());
    }
    Ok(())
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
        return Err("RAV catalog denominator differs from source contract".to_owned());
    }
    Ok(cards)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoAuraEffect,
    WrongGain,
    NoTrigger,
}
#[derive(Clone, Debug, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    aura: Option<ObjectId>,
    target: Option<ObjectId>,
    payment: Vec<Color>,
    target_controller: Option<usize>,
    cannot_attack_or_block: bool,
    nonmana_suppressed: bool,
    life_gained: i32,
    life_after_trigger: Option<i64>,
    ability_probe: Option<String>,
    ability_probe_unchanged: bool,
    attack_probe: Option<String>,
    attack_probe_unchanged: bool,
    block_probe: Option<String>,
    block_probe_unchanged: bool,
    combat_creatures: Vec<ObjectId>,
    combat_damage: i32,
}
fn find_trace_seed() -> Result<u64, String> {
    let mut last = "no attempts".to_owned();
    for seed in 0..MAX_TRACE_SEED {
        match run_match(seed, Mutation::None).and_then(|c| validate_trace(&c)) {
            Ok(()) => return Ok(seed),
            Err(e) => last = format!("seed {seed}: {e}"),
        }
    }
    Err(format!("no bounded Faith's Fetters duel completed; {last}"))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut definitions = card_definitions();
    let definition = definitions
        .iter_mut()
        .find(|d| d.id == CARD_ID)
        .ok_or("Faith's Fetters definition missing")?;
    *definition = candidate_faiths_fetters::definition();
    if mutation == Mutation::NoAuraEffect {
        definition.effects.clear()
    }
    let mut triggers = rav_triggered_ability_bindings();
    let ti = triggers
        .iter()
        .position(|b| b.card_definition == CARD_ID)
        .ok_or("Faith's Fetters trigger missing")?;
    triggers[ti] = candidate_faiths_fetters::triggered_ability();
    match mutation {
        Mutation::NoTrigger => {
            triggers.remove(ti);
        }
        Mutation::WrongGain => {
            triggers[ti].ability.effects = vec![Effect::GainLifeController { amount: 3 }]
        }
        Mutation::None | Mutation::NoAuraEffect => {}
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
    let deck0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: ATTACKER_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".into(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 24,
            },
        ],
        sideboard: vec![],
    };
    let deck1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: TARGET_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".into(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 28,
            },
        ],
        sideboard: vec![],
    };
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 80,
        max_policy_moves: 8000,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed).map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(0), &deck0)
        .map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(1), &deck1)
        .map_err(|e| e.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|e| e.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|e| e.to_string())?;
    game.begin_game().map_err(|e| e.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(FettersPolicy::new(PlayerId(0))),
        Box::new(FettersPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut attachment_snapshot: Option<(ObjectId, usize, bool, bool)> = None;
    let mut life_after_trigger = None;
    let (mut ability_probe, mut ability_probe_unchanged) = (None, false);
    let (mut attack_probe, mut attack_probe_unchanged) = (None, false);
    let (mut block_probe, mut block_probe_unchanged) = (None, false);
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(format!(
                "Faith's Fetters game exceeded bounds at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
        if player == PlayerId(1)
            && ability_probe.is_none()
            && view.priority == player
            && view.own_battlefield.iter().any(|c| {
                c.definition == Some(TARGET_ID) && c.nonmana_activated_abilities_suppressed
            })
        {
            let source = view
                .own_battlefield
                .iter()
                .find(|c| c.definition == Some(TARGET_ID))
                .ok_or("suppressed Guildmage missing")?
                .id;
            let before_events = game.event_log.len();
            let before_view = view.clone();
            let action = PolicyAction::ActivateAbility {
                activation: AbilityActivation {
                    source,
                    ability_id: "create-green-centaur",
                    sacrifice_sources: vec![],
                    additional_tap_creatures: vec![],
                    discard_cards: vec![],
                    targets: vec![],
                },
            };
            match game.submit_policy_move(player, "rav.faiths-fetters-suppression-probe.v1", action)
            {
                Ok(()) => {
                    return Err(
                        "Faith's Fetters failed to suppress Selesnya Guildmage ability".to_owned(),
                    );
                }
                Err(error) => {
                    let message = error.to_string();
                    if !message.contains("nonmana activated abilities are suppressed") {
                        return Err(format!("unexpected suppression rejection: {message}"));
                    }
                    ability_probe_unchanged = game.event_log.len() == before_events
                        && game.view_for_player(player).map_err(|e| e.to_string())? == before_view;
                    if !ability_probe_unchanged {
                        return Err("rejected activation probe changed match state".to_owned());
                    }
                    ability_probe = Some(message)
                }
            }
            continue;
        }
        if player == PlayerId(1)
            && view.step == Step::DeclareAttackers
            && view.active_player == player
            && !view.attackers_declared
            && attack_probe.is_none()
        {
            if let Some(target) = view.own_battlefield.iter().find(|c| {
                c.definition == Some(TARGET_ID)
                    && c.keywords.contains(&Keyword::CannotAttackOrBlock)
                    && !c.summoning_sick
            }) {
                let before_events = game.event_log.len();
                let before_view = view.clone();
                match game.submit_policy_move(
                    player,
                    "rav.faiths-fetters-attacker-probe.v1",
                    PolicyAction::DeclareAttackers {
                        attackers: vec![target.id],
                    },
                ) {
                    Ok(()) => return Err("Fettered Guildmage attack probe was accepted".to_owned()),
                    Err(error) => {
                        let message = error.to_string();
                        if !message.contains("illegal attacker") {
                            return Err(format!(
                                "unexpected attack restriction probe rejection: {message}"
                            ));
                        }
                        attack_probe_unchanged = game.event_log.len() == before_events
                            && game.view_for_player(player).map_err(|e| e.to_string())?
                                == before_view;
                        if !attack_probe_unchanged {
                            return Err("rejected attack probe changed match state".to_owned());
                        }
                        attack_probe = Some(message)
                    }
                }
                continue;
            }
        }
        if player == PlayerId(1)
            && view.step == Step::DeclareBlockers
            && view.active_player == PlayerId(0)
            && !view.blockers_declared
            && block_probe.is_none()
        {
            if let (Some(target), Some(attacker)) = (
                view.own_battlefield.iter().find(|c| {
                    c.definition == Some(TARGET_ID)
                        && c.keywords.contains(&Keyword::CannotAttackOrBlock)
                }),
                view.opponent_battlefield
                    .iter()
                    .find(|c| c.definition == Some(ATTACKER_ID)),
            ) {
                let before_events = game.event_log.len();
                let before_view = view.clone();
                match game.submit_policy_move(
                    player,
                    "rav.faiths-fetters-blocker-probe.v1",
                    PolicyAction::DeclareBlockers {
                        assignments: vec![CombatBlock {
                            blocker: target.id,
                            attacker: attacker.id,
                        }],
                    },
                ) {
                    Ok(()) => return Err("Fettered Guildmage block probe was accepted".to_owned()),
                    Err(error) => {
                        let message = error.to_string();
                        if !message.contains("illegal blocker") {
                            return Err(format!(
                                "unexpected block restriction rejection: {message}"
                            ));
                        }
                        block_probe_unchanged = game.event_log.len() == before_events
                            && game.view_for_player(player).map_err(|e| e.to_string())?
                                == before_view;
                        if !block_probe_unchanged {
                            return Err("rejected block probe changed match state".to_owned());
                        }
                        block_probe = Some(message)
                    }
                }
                continue;
            }
        }
        let policy = policies
            .get_mut(player.0)
            .ok_or("Faith's Fetters policy seat missing")?;
        let action = if let Some(a) = policy.propose_pending_decision(&view) {
            a
        } else if view.optional_triggered_ability_choice.is_some() {
            policy.propose_optional_triggered_ability(&view)
        } else if view.draw_replacement_pending {
            policy.propose_draw_replacement(&view)
        } else {
            policy.propose_move(&view)
        };
        attempted += 1;
        let debug = format!("{action:?}");
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|e| {
                format!(
                    "policy move rejected at turn {} {:?}: {e}; action={debug}; mana={:?}",
                    view.turn, view.step, view.mana_pool
                )
            })?;
        accepted += 1;
        game.validate_invariants().map_err(|e| e.to_string())?;
        if attachment_snapshot.is_none() {
            let aura = game.event_log.iter().find_map(|e| match e {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game.card_definition(*card).is_ok_and(|d| d.id == CARD_ID) => Some(*card),
                _ => None,
            });
            if let Some((aura, target)) = aura.and_then(|aura| {
                game.event_log.iter().find_map(|e| match e {
                    GameEvent::AuraAttached { aura: a, target } if *a == aura => {
                        Some((aura, *target))
                    }
                    _ => None,
                })
            }) {
                let char = game.characteristics(target).map_err(|e| e.to_string())?;
                let view = game
                    .view_for_player(PlayerId(1))
                    .map_err(|e| e.to_string())?;
                let card = view
                    .own_battlefield
                    .iter()
                    .find(|c| c.id == target)
                    .ok_or("attached Guildmage disappeared before live snapshot")?;
                attachment_snapshot = Some((
                    target,
                    game.controller_of(target).map_err(|e| e.to_string())?.0,
                    char.keywords.contains(&Keyword::CannotAttackOrBlock),
                    card.nonmana_activated_abilities_suppressed,
                ));
                let _ = aura;
            }
        }
        if life_after_trigger.is_none()&&game.event_log.iter().any(|e|matches!(e,GameEvent::AbilityResolved{source,ability:TRIGGER_ID,..}if game.card_definition(*source).is_ok_and(|d|d.id==CARD_ID))){life_after_trigger=game.player(PlayerId(0)).ok().map(|p|p.life);}
    }
    let events = game.event_log.clone();
    let aura = events.iter().find_map(|e| match e {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game.card_definition(*card).is_ok_and(|d| d.id == CARD_ID) => Some(*card),
        _ => None,
    });
    let target = attachment_snapshot.map(|x| x.0);
    let payment = aura
        .and_then(|aura| {
            events.iter().find_map(|e| match e {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if *card == aura => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    let life_gained = events
        .iter()
        .filter_map(|e| match e {
            GameEvent::LifeGained {
                player: PlayerId(0),
                amount,
            } => Some(*amount),
            _ => None,
        })
        .sum();
    let combat_creatures: Vec<ObjectId> = events
        .iter()
        .filter_map(|e| match e {
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } => Some(
                attackers
                    .iter()
                    .copied()
                    .filter(|id| game.card_definition(*id).is_ok_and(|d| d.id == ATTACKER_ID))
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .flatten()
        .collect();
    let combat_damage: i32 = events
        .iter()
        .filter_map(|e| match e {
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if combat_creatures.contains(source) => Some(*amount),
            _ => None,
        })
        .sum();
    let (_, target_controller, cannot_attack_or_block, nonmana_suppressed) = attachment_snapshot
        .map_or(
            (None, None, false, false),
            |(target, controller, cannot, suppressed)| {
                (Some(target), Some(controller), cannot, suppressed)
            },
        );
    Ok(Captured {
        events,
        winner: game.winner().map(|p| p.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        aura,
        target,
        payment,
        target_controller,
        cannot_attack_or_block,
        nonmana_suppressed,
        life_gained,
        life_after_trigger,
        ability_probe,
        ability_probe_unchanged,
        attack_probe,
        attack_probe_unchanged,
        block_probe,
        block_probe_unchanged,
        combat_creatures,
        combat_damage,
    })
}

struct FettersPolicy {
    player: PlayerId,
}
impl FettersPolicy {
    fn new(player: PlayerId) -> Self {
        Self { player }
    }
    fn activate(&self, view: &GameView, color: Color) -> Option<PolicyAction> {
        view.own_battlefield
            .iter()
            .find(|c| {
                !c.tapped
                    && c.card_types.contains(&CardType::Land)
                    && c.mana_colors.contains(&color)
            })
            .map(|c| PolicyAction::ActivateManaAbility { land: c.id, color })
    }
    fn activate_any(&self, view: &GameView) -> Option<PolicyAction> {
        [Color::Green, Color::White]
            .into_iter()
            .find_map(|c| self.activate(view, c))
    }
    fn play_land(&self, view: &GameView) -> Option<ObjectId> {
        let forests = view
            .own_battlefield
            .iter()
            .filter(|c| {
                c.card_types.contains(&CardType::Land) && c.mana_colors.contains(&Color::Green)
            })
            .count();
        let plains = view
            .own_battlefield
            .iter()
            .filter(|c| {
                c.card_types.contains(&CardType::Land) && c.mana_colors.contains(&Color::White)
            })
            .count();
        let pref = if forests < 3 {
            "RAV-FOREST"
        } else if plains < 1 {
            "RAV-PLAINS"
        } else if forests <= plains {
            "RAV-FOREST"
        } else {
            "RAV-PLAINS"
        };
        view.hand
            .iter()
            .find(|c| c.definition == Some(pref))
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|c| c.card_types.contains(&CardType::Land))
            })
            .map(|c| c.id)
    }
    fn cast(
        &self,
        card: ObjectId,
        targets: Vec<Target>,
        generic: Vec<Color>,
        hybrid: Vec<Color>,
    ) -> PolicyAction {
        PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card,
                targets,
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection { generic, hybrid },
        }
    }
    fn fetters_payment(&self, view: &GameView) -> Option<Vec<Color>> {
        if view.mana_pool.amount(Color::White) == 0 || view.mana_pool.total() < 4 {
            return None;
        }
        let mut left: Vec<_> = Color::MANA_ALL
            .into_iter()
            .map(|c| (c, view.mana_pool.amount(c)))
            .collect();
        left.iter_mut().find(|(c, _)| *c == Color::White)?.1 -= 1;
        let mut generic = Vec::new();
        for color in [
            Color::Green,
            Color::Blue,
            Color::Black,
            Color::Red,
            Color::White,
        ] {
            let amount = &mut left.iter_mut().find(|(c, _)| *c == color)?.1;
            while *amount > 0 && generic.len() < 3 {
                generic.push(color);
                *amount -= 1
            }
        }
        (generic.len() == 3).then_some(generic)
    }
    fn cast_selesnya(&self, view: &GameView, card: ObjectId) -> Option<PolicyAction> {
        let green = view.mana_pool.amount(Color::Green);
        let white = view.mana_pool.amount(Color::White);
        if green + white < 2 {
            return None;
        }
        let mut choices = Vec::new();
        for _ in 0..2 {
            if green > choices.iter().filter(|c| **c == Color::Green).count() as u8 {
                choices.push(Color::Green)
            } else if white > choices.iter().filter(|c| **c == Color::White).count() as u8 {
                choices.push(Color::White)
            } else {
                return None;
            }
        }
        Some(self.cast(card, vec![], vec![], choices))
    }
}
impl CodePolicy for FettersPolicy {
    fn id(&self) -> &'static str {
        "rav.faiths-fetters-aura-restriction-and-etb-life.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: if self.player == PlayerId(0) {
                    view.own_battlefield
                        .iter()
                        .filter(|c| c.definition == Some(ATTACKER_ID) && c.can_attack)
                        .map(|c| c.id)
                        .collect()
                } else {
                    vec![]
                },
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
        if view.decision_player != self.player
            || view.priority != self.player
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0
            && let Some(land) = self.play_land(view)
        {
            return PolicyAction::PlayLand { card: land };
        }
        if self.player == PlayerId(0) {
            let target = view
                .opponent_battlefield
                .iter()
                .find(|c| c.definition == Some(TARGET_ID))
                .map(|c| c.id);
            let aura_live = view
                .own_battlefield
                .iter()
                .any(|c| c.definition == Some(CARD_ID));
            if !aura_live
                && let (Some(target), Some(fetters)) = (
                    target,
                    view.hand.iter().find(|c| c.definition == Some(CARD_ID)),
                )
            {
                if view.mana_pool.amount(Color::White) == 0
                    && let Some(a) = self.activate(view, Color::White)
                {
                    return a;
                }
                if view.mana_pool.total() < 4
                    && let Some(a) = self.activate_any(view)
                {
                    return a;
                }
                if let Some(generic) = self.fetters_payment(view) {
                    return self.cast(fetters.id, vec![Target::Permanent(target)], generic, vec![]);
                }
            }
            if let Some(wolf) = view.hand.iter().find(|c| c.definition == Some(ATTACKER_ID)) {
                if view.mana_pool.amount(Color::Green) == 0
                    && let Some(a) = self.activate(view, Color::Green)
                {
                    return a;
                }
                if view.mana_pool.amount(Color::White) == 0
                    && let Some(a) = self.activate(view, Color::White)
                {
                    return a;
                }
                if view.mana_pool.amount(Color::Green) > 0
                    && view.mana_pool.amount(Color::White) > 0
                {
                    return self.cast(wolf.id, vec![], vec![], vec![]);
                }
            }
        } else if !view
            .own_battlefield
            .iter()
            .any(|c| c.definition == Some(TARGET_ID))
        {
            if let Some(guildmage) = view.hand.iter().find(|c| c.definition == Some(TARGET_ID)) {
                if view.mana_pool.total() < 2 {
                    if let Some(a) = self.activate_any(view) {
                        return a;
                    }
                }
                if let Some(action) = self.cast_selesnya(view, guildmage.id) {
                    return action;
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(c: &Captured) -> Result<(), String> {
    if c.winner != Some(0) || c.rejected != 0 {
        return Err("Faith's Fetters duel did not end in clean P0 win".to_owned());
    }
    let aura = c.aura.ok_or("Faith's Fetters was not cast")?;
    let target = c.target.ok_or("Faith's Fetters did not attach")?;
    if c.payment.len() != 4 || c.payment.iter().filter(|x| **x == Color::White).count() != 1 {
        return Err("Faith's Fetters exact {3}{W} payment differs".to_owned());
    }
    if c.target_controller != Some(1) || !c.cannot_attack_or_block || !c.nonmana_suppressed {
        return Err("Faith's Fetters did not restrict the exact opposing Guildmage".to_owned());
    }
    if c.life_gained != 4 || c.life_after_trigger != Some(24) {
        return Err(format!(
            "Faith's Fetters ETB must gain four life: gained {}, life {:?}",
            c.life_gained, c.life_after_trigger
        ));
    }
    if !c.ability_probe_unchanged
        || !c
            .ability_probe
            .as_deref()
            .is_some_and(|m| m.contains("nonmana activated abilities are suppressed"))
    {
        return Err(
            "Faith's Fetters nonmana ability suppression probe did not reject atomically"
                .to_owned(),
        );
    }
    if !c.attack_probe_unchanged
        || !c
            .attack_probe
            .as_deref()
            .is_some_and(|m| m.contains("illegal attacker"))
    {
        return Err(
            "Faith's Fetters attack restriction probe did not reject atomically".to_owned(),
        );
    }
    if !c.block_probe_unchanged
        || !c
            .block_probe
            .as_deref()
            .is_some_and(|m| m.contains("illegal blocker"))
    {
        return Err("Faith's Fetters block restriction probe did not reject atomically".to_owned());
    }
    if c.combat_damage < 20 {
        return Err("P0 did not continue through full-game combat to victory".to_owned());
    }
    let paid=event_index(&c.events,|e|matches!(e,GameEvent::SpellManaPaid{player:PlayerId(0),card,colors}if *card==aura&&colors.len()==4&&colors.iter().filter(|x|**x==Color::White).count()==1)).ok_or("Faith's Fetters exact payment receipt missing")?;
    let cast = event_index(
        &c.events,
        |e| matches!(e,GameEvent::SpellCast{player:PlayerId(0),card}if *card==aura),
    )
    .ok_or("Faith's Fetters cast receipt missing")?;
    let resolved = event_index(
        &c.events,
        |e| matches!(e,GameEvent::SpellResolved{card}if *card==aura),
    )
    .ok_or("Faith's Fetters spell resolution missing")?;
    let enters = event_index(
        &c.events,
        |e| matches!(e,GameEvent::CardMoved{card,to:Zone::Battlefield}if *card==aura),
    )
    .ok_or("Faith's Fetters battlefield entry missing")?;
    let attached = event_index(
        &c.events,
        |e| matches!(e,GameEvent::AuraAttached{aura:a,target:t}if *a==aura&&*t==target),
    )
    .ok_or("Faith's Fetters exact attachment missing")?;
    let trigger=event_index(&c.events,|e|matches!(e,GameEvent::TriggeredAbilityStacked{controller:PlayerId(0),source,ability:TRIGGER_ID,..}if *source==aura)).ok_or("Faith's Fetters trigger did not stack")?;
    let life = event_index_after(&c.events, trigger, |e| {
        matches!(
            e,
            GameEvent::LifeGained {
                player: PlayerId(0),
                amount: 4
            }
        )
    })
    .ok_or("Faith's Fetters did not gain four life")?;
    let ability = event_index_after(
        &c.events,
        life,
        |e| matches!(e,GameEvent::AbilityResolved{source,ability:TRIGGER_ID,..}if *source==aura),
    )
    .ok_or("Faith's Fetters ETB ability did not resolve")?;
    let attack=event_index(&c.events,|e|matches!(e,GameEvent::AttackersDeclared{player:PlayerId(0),attackers}if attackers.iter().any(|id|c.combat_creatures.contains(id)))).ok_or("P0 legal attack receipt missing")?;
    let damage = event_index_after(
        &c.events,
        attack,
        |e| matches!(e,GameEvent::DamageDealtToPlayer{player:PlayerId(1),amount,..}if *amount>0),
    )
    .ok_or("P0 combat damage receipt missing")?;
    let end = event_index(&c.events, |e| {
        matches!(
            e,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal P0 win receipt missing")?;
    if !(paid < cast
        && cast < resolved
        && resolved < enters
        && enters < attached
        && attached < trigger
        && trigger < life
        && life < ability
        && ability < attack
        && attack < damage
        && damage < end)
    {
        return Err(
            "Faith's Fetters cast/attachment/trigger/life/combat event order differs".to_owned(),
        );
    }
    if c.events.iter().any(|e|matches!(e,GameEvent::AbilityActivated{source,ability:"create-green-centaur",..}if *source==target)){return Err("suppressed Selesnya Guildmage ability unexpectedly activated".to_owned())}
    Ok(())
}
fn mutation_detected(seed: u64, m: Mutation) -> bool {
    run_match(seed, m).and_then(|c| validate_trace(&c)).is_err()
}
fn event_index(events: &[GameEvent], p: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(p)
}
fn event_index_after(
    events: &[GameEvent],
    s: usize,
    p: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(s + 1)
        .find_map(|(i, e)| p(e).then_some(i))
}
fn trace_json(c: &Captured) -> Value {
    let aura = c.aura;
    let target = c.target;
    let projected = project_events(&c.events);
    let events:Vec<Value>=c.events.iter().enumerate().filter_map(|(i,event)|{let include=match event{GameEvent::SpellManaPaid{card,..}|GameEvent::SpellCast{card,..}|GameEvent::SpellResolved{card}=>aura==Some(*card),GameEvent::CardMoved{card,to:Zone::Battlefield}=>aura==Some(*card),GameEvent::AuraAttached{aura:a,target:t}=>aura==Some(*a)&&target==Some(*t),GameEvent::TriggeredAbilityStacked{source,ability,..}|GameEvent::AbilityResolved{source,ability,..}=>aura==Some(*source)&&*ability==TRIGGER_ID,GameEvent::LifeGained{player:PlayerId(0),amount:4}=>true,GameEvent::AttackersDeclared{player:PlayerId(0),attackers}=>attackers.iter().any(|id|c.combat_creatures.contains(id)),GameEvent::DamageDealtToPlayer{source,player:PlayerId(1),amount}=>*amount>0&&c.combat_creatures.contains(source),GameEvent::GameEnded{winner:Some(PlayerId(0))}=>true,_=>false};if !include{return None}let e: &ProjectedEvent=projected.get(i)?;Some(json!({"sequence":e.sequence,"turn":e.turn,"kind":e.kind,"facts":e.facts,"canonical":e.canonical}))}).collect();
    json!({"trace_id":"rav-faiths-fetters-restricts-opposing-permanent-and-gains-four","terminal":c.winner.map_or_else(||"Draw".to_owned(),|s|format!("Winner(PlayerId({s}))")),"winner":c.winner,"turns":c.turns,"accepted_policy_moves":c.accepted,"rejected_policy_moves":c.rejected,"mechanics":{"shuffled_60_card_decks":true,"mana_cost":"{3}{W}","paid_mana_colors":c.payment.iter().map(|x|format!("{x:?}")).collect::<Vec<_>>(),"aura_object":aura.map(|x|x.0),"target_permanent_object":target.map(|x|x.0),"aura_attached_to_target":aura.is_some()&&target.is_some(),"target_controller_seat":c.target_controller,"cannot_attack_or_block":c.cannot_attack_or_block,"nonmana_activated_abilities_suppressed":c.nonmana_suppressed,"ability_activation_rejected":c.ability_probe.is_some()&&c.ability_probe_unchanged,"attack_declaration_rejected":c.attack_probe.is_some()&&c.attack_probe_unchanged,"block_declaration_rejected":c.block_probe.is_some()&&c.block_probe_unchanged,"etb_life_gained":c.life_gained,"controller_life_after_trigger":c.life_after_trigger,"trigger_stack_count":c.events.iter().filter(|e|matches!(e,GameEvent::TriggeredAbilityStacked{source,ability:TRIGGER_ID,..}if Some(*source)==aura)).count(),"trigger_resolve_count":c.events.iter().filter(|e|matches!(e,GameEvent::AbilityResolved{source,ability:TRIGGER_ID,..}if Some(*source)==aura)).count(),"combat_creature_objects":c.combat_creatures.iter().map(|x|x.0).collect::<Vec<_>>(),"controller_combat_damage":c.combat_damage,"deterministic_replay":true,"terminal_combat_win":c.winner==Some(0)&&c.combat_damage>=20},"events":events})
}
fn candidate_kind() -> &'static str {
    if std::env::args().any(|a| a == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}
fn card_status(card: &CatalogCard) -> Value {
    let id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 16;
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,"engine_status":if id.is_some(){"executable_compatibility_slice"}else{"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"qualification":if passed{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
}
fn flatten_use_tree(prefix: &[String], tree: &UseTree, paths: &mut Vec<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(i) => {
            let mut p = prefix.to_vec();
            p.push(i.ident.to_string());
            flatten_use_tree(&p, &i.tree, paths)
        }
        UseTree::Name(i) => {
            let mut p = prefix.to_vec();
            p.push(i.ident.to_string());
            paths.push(p);
            true
        }
        UseTree::Group(g) => g.items.iter().all(|i| flatten_use_tree(prefix, i, paths)),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}
fn type_is(output: &syn::ReturnType, expected: &str) -> bool {
    matches!(output,syn::ReturnType::Type(_,ty)if matches!(&**ty,syn::Type::Path(p)if p.qself.is_none()&&path_is_type(&p.path,expected)))
}
fn path_is_type(path: &syn::Path, expected: &str) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == 1
        && path.segments[0].ident == expected
        && matches!(path.segments[0].arguments, syn::PathArguments::None)
}
