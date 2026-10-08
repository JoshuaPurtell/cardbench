//! Full-game diagnostic authority for RAV Caregiver.

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, CardDefinition, CardType, CastRequest, Color, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    card_definitions, rav_activated_ability_bindings, rav_activated_ability_cost_modifier_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings,
    rav_attachment_triggered_ability_bindings as attachment_triggers, rav_basic_land_type_bindings,
    rav_cost_reduction_bindings, rav_damage_replacement_effect_bindings,
    rav_entry_coin_flip_bindings, rav_entry_copy_bindings,
    rav_generalized_activated_ability_cost_bindings, rav_land_entry_bindings,
    rav_legendary_permanent_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_mana_ability_cost_bindings, rav_replacement_effect_bindings,
    rav_static_attack_restriction_bindings, rav_static_continuous_effect_bindings,
    rav_static_creature_spell_cost_modifier_bindings, rav_static_entry_restriction_bindings,
    rav_static_library_top_reveal_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::{Expr, Item, PathArguments, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-CAREGIVER";
const CARD_NAME: &str = "Caregiver";
const ELF_ID: &str = "RAV-ELVES-OF-DEEP-SHADOW";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const ABILITY_ID: &str = "prevent-one-damage";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/caregiver.rs";
const MAX_TRACE_SEED: u64 = 128;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/caregiver.rs"]
mod candidate_caregiver;

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
            eprintln!("Caregiver PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_runtime()?;
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
        return Err("RAV catalog denominator differs from the pinned source contract".to_owned());
    }

    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.winner != replay.winner
        || captured.caregiver != replay.caregiver
        || captured.elf != replay.elf
        || captured.watchwolf != replay.watchwolf
    {
        return Err("Caregiver shuffled-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "wrong_white_activation_cost_detected_in_game": mutation_detected(seed, Mutation::WrongAbilityCost),
        "missing_source_sacrifice_detected_in_game": mutation_detected(seed, Mutation::NoSourceSacrifice),
        "missing_one_damage_shield_detected_in_game": mutation_detected(seed, Mutation::NoShield),
        "wrong_white_cast_cost_detected_in_game": mutation_detected(seed, Mutation::WrongCastCost),
        "sealed_mutation_authority": false
    });
    for key in [
        "wrong_white_activation_cost_detected_in_game",
        "missing_source_sacrifice_detected_in_game",
        "missing_one_damage_shield_detected_in_game",
        "wrong_white_cast_cost_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!("Caregiver compiled semantic mutant escaped: {key}"));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 6, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117 priority and timing",
                "202.1-202.2 colored mana costs and payment",
                "302.1 creature characteristics",
                "601.2 casting spells and paying costs",
                "602.2 activating abilities, selecting targets, and paying costs",
                "608.2 resolving activated abilities",
                "615.3-615.7 prevention effects and one-point damage shields",
                "611.2a end-of-turn duration",
                "508 declaring attackers",
                "509 declaring blockers",
                "510 combat damage",
                "704 state-based actions"
            ],
            "source_contract": "RAV Caregiver #6, {W} 1/1 creature with {W}, sacrifice Caregiver: prevent the next 1 damage that would be dealt to target creature or player this turn",
            "authority_cutoff": "2026-09-24",
            "no_provider_trial": true
        },
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
        "qualification_status": "diagnostic_only; independent review and candidate trial pending",
        "deterministic_replay": true,
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongAbilityCost,
    NoSourceSacrifice,
    NoShield,
    WrongCastCost,
}

fn candidate_values(mutation: Mutation) -> (CardDefinition, ActivatedAbility) {
    let mut definition = candidate_caregiver::definition();
    let mut ability = candidate_caregiver::activated_ability().ability;
    match mutation {
        Mutation::None => {}
        Mutation::WrongAbilityCost => {
            ability.mana_cost = ManaCost::with_colors(0, [Color::Green]);
        }
        Mutation::NoSourceSacrifice => ability.sacrifice_source = false,
        Mutation::NoShield => {
            ability.effects = vec![Effect::AddTargetDamageShieldUntilEndOfTurn { amount: 0 }];
        }
        Mutation::WrongCastCost => {
            definition.mana_cost = ManaCost::with_colors(0, [Color::Green]);
        }
    }
    (definition, ability)
}

fn validate_candidate_runtime() -> Result<(), String> {
    let (definition, ability) = candidate_values(Mutation::None);
    let registered_definition = card_definitions()
        .into_iter()
        .find(|known| known.id == CARD_ID)
        .ok_or("Caregiver definition is absent from the registry")?;
    let registered_ability = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("Caregiver activated ability is absent from the registry")?;
    if registered_definition != definition || registered_ability.ability != ability {
        return Err(
            "Caregiver registry does not delegate to its candidate-owned module".to_owned(),
        );
    }
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(0, [Color::White])
        || definition.colors != BTreeSet::from([Color::White])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.supported_rules
            != [
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "sacrifice-source-targeted-one-damage-prevention",
            ]
        || definition.power != Some(1)
        || definition.toughness != Some(1)
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
        || ability.id != ABILITY_ID
        || ability.mana_cost != ManaCost::with_colors(0, [Color::White])
        || ability.tap_cost
        || ability.sorcery_speed
        || ability.additional_tap_creatures != 0
        || !ability.sacrifice_source
        || ability.sacrifice_creatures != 0
        || ability.sacrifice_lands != 0
        || ability.discard_cards != 0
        || ability.targets != [cardbench_magic_engine::TargetRequirement::PlayerOrCreature]
        || ability.effects != [Effect::AddTargetDamageShieldUntilEndOfTurn { amount: 1 }]
    {
        return Err("Caregiver candidate identity, exact white costs, characteristics, source sacrifice, target, or one-point shield differs from the card contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|e| format!("could not read Caregiver candidate source: {e}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Caregiver candidate module is not valid Rust".to_owned())?;
    if !file.attrs.iter().all(|attr| attr.path().is_ident("doc")) || file.items.len() != 4 {
        return Err(
            "Caregiver source must contain only two imports and two literal functions".to_owned(),
        );
    }
    let mut functions = BTreeSet::new();
    let mut imports = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(use_item)
                if use_item.attrs.is_empty() && matches!(&use_item.vis, Visibility::Inherited) =>
            {
                collect_imports(&use_item.tree, &mut Vec::new(), &mut imports)?;
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                let expected = match name.as_str() {
                    "definition" => "CardDefinition",
                    "activated_ability" => "ActivatedAbilityBinding",
                    _ => return Err("Caregiver source has an unexpected function".to_owned()),
                };
                if !functions.insert(name.clone())
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !function.sig.inputs.is_empty()
                    || function.sig.constness.is_some()
                    || function.sig.asyncness.is_some()
                    || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if matches!(&**ty, syn::Type::Path(path) if path.qself.is_none() && path.path.segments.len()==1 && path.path.segments[0].ident==expected && matches!(path.path.segments[0].arguments,PathArguments::None)))
                    || function.block.stmts.len() != 1
                    || !matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(_), None))
                {
                    return Err("Caregiver functions must be crate-visible, argument-free direct struct literals".to_owned());
                }
            }
            _ => {
                return Err(
                    "Caregiver source contains unsupported executable top-level code".to_owned(),
                );
            }
        }
    }
    if functions != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()])
        || imports
            != BTreeSet::from([
                "std::collections::BTreeSet".to_owned(),
                "cardbench_magic_engine::ActivatedAbility".to_owned(),
                "cardbench_magic_engine::ActivatedAbilityBinding".to_owned(),
                "cardbench_magic_engine::CardDefinition".to_owned(),
                "cardbench_magic_engine::CardType".to_owned(),
                "cardbench_magic_engine::Color".to_owned(),
                "cardbench_magic_engine::Effect".to_owned(),
                "cardbench_magic_engine::ManaCost".to_owned(),
                "cardbench_magic_engine::TargetRequirement".to_owned(),
            ])
    {
        return Err(
            "Caregiver source imports or function inventory differs from the audit contract"
                .to_owned(),
        );
    }
    Ok(())
}

fn collect_imports(
    tree: &syn::UseTree,
    prefix: &mut Vec<String>,
    output: &mut BTreeSet<String>,
) -> Result<(), String> {
    match tree {
        syn::UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            collect_imports(&path.tree, prefix, output)?;
            prefix.pop();
        }
        syn::UseTree::Name(name) => {
            let mut full = prefix.clone();
            full.push(name.ident.to_string());
            output.insert(full.join("::"));
        }
        syn::UseTree::Group(group) => {
            for item in &group.items {
                collect_imports(item, prefix, output)?;
            }
        }
        syn::UseTree::Rename(_) | syn::UseTree::Glob(_) => {
            return Err("Caregiver source cannot rename or glob-import authority names".to_owned());
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    caregiver: Option<ObjectId>,
    elf: Option<ObjectId>,
    watchwolf: Option<ObjectId>,
    cast_payment: Vec<Color>,
    ability_payment: Option<ManaCost>,
    source_zone: Option<Zone>,
    source_live_characteristics: Option<(BTreeSet<Color>, BTreeSet<CardType>, i32, i32)>,
    watchwolf_power_toughness: Option<(i32, i32)>,
    player_life: [i64; 2],
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_CAREGIVER_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_CAREGIVER_SEED must be an unsigned integer".to_owned())?;
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let Ok(captured) = run_match(seed, Mutation::None)
            && validate_trace(&captured, seed).is_ok()
        {
            return Ok((seed, captured));
        }
    }
    Err(format!(
        "no Caregiver full-game prevention trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    run_match(seed, mutation)
        .and_then(|capture| validate_trace(&capture, seed))
        .is_err()
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let (definition, ability) = candidate_values(mutation);
    let (mut game, _) = new_game(definition.clone(), ability)?;
    let p0 = deck(&[
        (CARD_ID, 4),
        (WATCHWOLF_ID, 4),
        ("RAV-FOREST", 26),
        ("RAV-PLAINS", 26),
    ]);
    let p1 = deck(&[(ELF_ID, 4), ("RAV-FOREST", 56)]);
    if p0
        .mainboard
        .iter()
        .map(|entry| usize::from(entry.count))
        .sum::<usize>()
        != 60
        || p1
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
    {
        return Err("Caregiver duel decks must be legal 60-card libraries".to_owned());
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 40,
        max_policy_moves: 6000,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed).map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(0), &p0)
        .map_err(|e| e.to_string())?;
    game.load_deck_into_library(PlayerId(1), &p1)
        .map_err(|e| e.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|e| e.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|e| e.to_string())?;
    game.begin_game().map_err(|e| e.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(CaregiverPolicy::new(
            PlayerId(0),
            definition.mana_cost.clone(),
            mutation,
        )),
        Box::new(CaregiverPolicy::new(
            PlayerId(1),
            ManaCost::with_colors(0, [Color::Green]),
            mutation,
        )),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut caregiver_live_characteristics = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Caregiver match exceeded bounded turn or move count".to_owned());
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Caregiver policy seat missing")?;
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
        let event_start = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|e| {
                format!(
                    "Caregiver policy move rejected on turn {} {:?}: {e}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants().map_err(|e| e.to_string())?;
        for event in &game.event_log[event_start..] {
            if let GameEvent::CardMoved {
                card,
                to: Zone::Battlefield,
            } = event
                && game
                    .card_definition(*card)
                    .is_ok_and(|known| known.id == CARD_ID)
            {
                let characteristics = game.characteristics(*card).map_err(|e| e.to_string())?;
                caregiver_live_characteristics = Some((
                    characteristics.colors.clone(),
                    characteristics.card_types.clone(),
                    characteristics
                        .power
                        .ok_or("Caregiver power missing on entry")?,
                    characteristics
                        .toughness
                        .ok_or("Caregiver toughness missing on entry")?,
                ));
            }
        }
    }
    let events = game.event_log.clone();
    let find_cast = |seat: PlayerId, id: &str| {
        events.iter().find_map(|event| match event {
            GameEvent::SpellCast { player, card }
                if *player == seat && game.card_definition(*card).is_ok_and(|d| d.id == id) =>
            {
                Some(*card)
            }
            _ => None,
        })
    };
    let caregiver = find_cast(PlayerId(0), CARD_ID);
    let elf = find_cast(PlayerId(1), ELF_ID);
    let watchwolf = find_cast(PlayerId(0), WATCHWOLF_ID);
    let cast_payment = caregiver
        .and_then(|object| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if *card == object => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    let ability_payment = caregiver.and_then(|object| {
        events.iter().find_map(|event| match event {
            GameEvent::AbilityManaPaid {
                player: PlayerId(0),
                source,
                ability: ABILITY_ID,
                mana_cost,
            } if *source == object => Some(mana_cost.clone()),
            _ => None,
        })
    });
    let source_zone = caregiver.and_then(|object| game.zone_of(object));
    let source_live_characteristics = caregiver_live_characteristics;
    let watchwolf_power_toughness = watchwolf.and_then(|object| {
        let stats = game.characteristics(object).ok()?;
        Some((stats.power?, stats.toughness?))
    });
    let player_life = [
        game.player(PlayerId(0)).map_err(|e| e.to_string())?.life,
        game.player(PlayerId(1)).map_err(|e| e.to_string())?.life,
    ];
    let winner = game.winner().map(|seat| seat.0);
    Ok(Captured {
        events,
        terminal: winner.map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({seat}))"),
        ),
        winner,
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        caregiver,
        elf,
        watchwolf,
        cast_payment,
        ability_payment,
        source_zone,
        source_live_characteristics,
        watchwolf_power_toughness,
        player_life,
    })
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

fn new_game(
    definition: CardDefinition,
    ability: ActivatedAbility,
) -> Result<(Game, Vec<CardDefinition>), String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|known| known.id == CARD_ID)
        .ok_or("Caregiver definition missing from registry")? = definition;
    let mut abilities = rav_activated_ability_bindings();
    let binding = abilities
        .iter_mut()
        .find(|known| known.card_definition == CARD_ID && known.ability.id == ABILITY_ID)
        .ok_or("Caregiver activated binding missing from registry")?;
    binding.ability = ability;
    let mut triggers = rav_triggered_ability_bindings();
    triggers.extend(attachment_triggers());
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions.clone(),
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
    game.register_replacement_effect_bindings(rav_replacement_effect_bindings())
        .map_err(|e| e.to_string())?;
    game.register_damage_replacement_effect_bindings(rav_damage_replacement_effect_bindings())
        .map_err(|e| e.to_string())?;
    Ok((game, definitions))
}

struct CaregiverPolicy {
    player: PlayerId,
    cast_cost: ManaCost,
    mutation: Mutation,
    caregiver_cast_offered: bool,
    elf_cast_offered: bool,
    wolf_cast_offered: bool,
    ability_offered: bool,
}

impl CaregiverPolicy {
    fn new(player: PlayerId, cast_cost: ManaCost, mutation: Mutation) -> Self {
        Self {
            player,
            cast_cost,
            mutation,
            caregiver_cast_offered: false,
            elf_cast_offered: false,
            wolf_cast_offered: false,
            ability_offered: false,
        }
    }

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let preferred = if self.player == PlayerId(1) {
            Color::Green
        } else {
            let white = view
                .own_battlefield
                .iter()
                .filter(|card| card.mana_colors.contains(&Color::White))
                .count();
            let green = view
                .own_battlefield
                .iter()
                .filter(|card| card.mana_colors.contains(&Color::Green))
                .count();
            if !self.ability_offered && white < 2 {
                Color::White
            } else if green == 0 {
                Color::Green
            } else {
                Color::White
            }
        };
        let land = view
            .hand
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&preferred)
            })
            .or_else(|| {
                view.hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            })?;
        Some(PolicyAction::PlayLand { card: land.id })
    }

    fn tap_land(&self, view: &GameView, color: Color) -> Option<PolicyAction> {
        let land = view.own_battlefield.iter().find(|card| {
            !card.tapped
                && card.card_types.contains(&CardType::Land)
                && card.mana_colors.contains(&color)
        })?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }

    fn payment(&self, view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let mut available: BTreeMap<Color, u8> = Color::ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        for color in &cost.colored {
            let count = available.get_mut(color)?;
            if *count == 0 {
                return None;
            }
            *count -= 1;
        }
        if !cost.hybrid.is_empty() {
            return None;
        }
        let mut generic = Vec::new();
        for _ in 0..cost.generic {
            let color = available
                .iter()
                .filter(|(_, count)| **count > 0)
                .max_by_key(|(_, count)| **count)
                .map(|(color, _)| *color)?;
            *available.get_mut(&color)? -= 1;
            generic.push(color);
        }
        Some(ManaPaymentSelection {
            generic,
            hybrid: vec![],
        })
    }

    fn cast_action(
        &mut self,
        view: &GameView,
        card_id: &str,
        cost: &ManaCost,
    ) -> Option<PolicyAction> {
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(card_id))?;
        if let Some(mana_selection) = self.payment(view, cost) {
            match card_id {
                CARD_ID => self.caregiver_cast_offered = true,
                ELF_ID => self.elf_cast_offered = true,
                WATCHWOLF_ID => self.wolf_cast_offered = true,
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
        let needed = cost
            .colored
            .iter()
            .find(|color| view.mana_pool.amount(**color) == 0)
            .copied()
            .or_else(|| {
                (u16::from(view.mana_pool.total())
                    < u16::from(cost.generic)
                        + cost.colored.len() as u16
                        + cost.hybrid.len() as u16)
                    .then_some(cost.colored.first().copied().unwrap_or(Color::White))
            });
        needed
            .and_then(|color| self.tap_land(view, color))
            .or_else(|| {
                Color::ALL
                    .into_iter()
                    .find_map(|color| self.tap_land(view, color))
            })
    }

    fn activate_prevention(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.player != PlayerId(0)
            || self.ability_offered
            || view.active_player == self.player
            || view.step != Step::DeclareAttackers
            || !view.attackers_declared
            || view.stack_depth != 0
            || !view
                .combat_attackers
                .iter()
                .any(|card| card.definition == Some(ELF_ID))
        {
            return None;
        }
        let source = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some(CARD_ID))?;
        let cost = if self.mutation == Mutation::WrongAbilityCost {
            ManaCost::with_colors(0, [Color::Green])
        } else {
            ManaCost::with_colors(0, [Color::White])
        };
        if self.payment(view, &cost).is_none() {
            let missing = cost
                .colored
                .iter()
                .find(|color| view.mana_pool.amount(**color) == 0)
                .copied()
                .unwrap_or(Color::White);
            return self.tap_land(view, missing);
        }
        self.ability_offered = true;
        Some(PolicyAction::ActivateAbility {
            activation: AbilityActivation {
                source: source.id,
                ability_id: ABILITY_ID,
                sacrifice_sources: if self.mutation == Mutation::NoSourceSacrifice {
                    vec![]
                } else {
                    vec![source.id]
                },
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![Target::Player(PlayerId(0))],
            },
        })
    }
}

impl CodePolicy for CaregiverPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.caregiver-shield-and-watchwolf.v1"
        } else {
            "rav.caregiver-elf-pressure.v1"
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
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| {
                        card.can_attack
                            && (self.player == PlayerId(0) && card.definition == Some(WATCHWOLF_ID)
                                || self.player == PlayerId(1) && card.definition == Some(ELF_ID))
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
        if view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.activate_prevention(view) {
            return action;
        }
        if view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.play_land(view) {
            return action;
        }
        if self.player == PlayerId(1) {
            if !self.elf_cast_offered {
                if let Some(action) =
                    self.cast_action(view, ELF_ID, &ManaCost::with_colors(0, [Color::Green]))
                {
                    return action;
                }
            }
            return PolicyAction::PassPriority;
        }
        let has_caregiver = view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some(CARD_ID));
        if !has_caregiver && !self.caregiver_cast_offered {
            let cost = self.cast_cost.clone();
            if let Some(action) = self.cast_action(view, CARD_ID, &cost) {
                return action;
            }
        }
        let has_wolf = view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some(WATCHWOLF_ID));
        if self.ability_offered && !has_wolf && !self.wolf_cast_offered {
            if let Some(action) = self.cast_action(
                view,
                WATCHWOLF_ID,
                &ManaCost::with_colors(0, [Color::Green, Color::White]),
            ) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))"
        || captured.winner != Some(0)
        || captured.rejected != 0
    {
        return Err("Caregiver duel did not reach a clean seat-zero terminal win".to_owned());
    }
    let caregiver = captured
        .caregiver
        .ok_or("seat zero did not cast Caregiver")?;
    let elf = captured
        .elf
        .ok_or("seat one did not cast Elves of Deep Shadow")?;
    let watchwolf = captured
        .watchwolf
        .ok_or("seat zero did not cast Watchwolf")?;
    if captured.cast_payment != [Color::White]
        || captured.ability_payment != Some(ManaCost::with_colors(0, [Color::White]))
    {
        return Err(format!(
            "Caregiver did not pay exact {{W}} for both spell and activation: {:?} / {:?}",
            captured.cast_payment, captured.ability_payment
        ));
    }
    if captured.source_zone != Some(Zone::Graveyard)
        || captured.source_live_characteristics
            != Some((
                BTreeSet::from([Color::White]),
                BTreeSet::from([CardType::Creature]),
                1,
                1,
            ))
        || captured.watchwolf_power_toughness != Some((3, 3))
    {
        return Err(format!(
            "Caregiver/Watchwolf live characteristics or source-sacrifice state differ: {:?}, {:?}, {:?}",
            captured.source_zone,
            captured.source_live_characteristics,
            captured.watchwolf_power_toughness
        ));
    }
    if captured.player_life[0] <= 0 || captured.player_life[1] > 0 {
        return Err(
            "shield trace or Watchwolf combat failed to produce expected terminal life totals"
                .to_owned(),
        );
    }
    let events = &captured.events;
    let caregiver_paid = find_event(events, |e| matches!(e, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == caregiver && colors == &[Color::White])).ok_or("Caregiver exact white SpellManaPaid event is missing")?;
    let caregiver_cast = find_event_after(
        events,
        caregiver_paid,
        |e| matches!(e, GameEvent::SpellCast { player: PlayerId(0), card } if *card == caregiver),
    )
    .ok_or("Caregiver SpellCast event is missing")?;
    let caregiver_resolved = find_event_after(
        events,
        caregiver_cast,
        |e| matches!(e, GameEvent::SpellResolved { card } if *card == caregiver),
    )
    .ok_or("Caregiver SpellResolved event is missing")?;
    let caregiver_entered = find_event_after(
        events,
        caregiver_resolved,
        |e| matches!(e, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == caregiver),
    )
    .ok_or("Caregiver battlefield entry is missing")?;
    let elf_paid = find_event(events, |e| matches!(e, GameEvent::SpellManaPaid { player: PlayerId(1), card, colors } if *card == elf && colors == &[Color::Green])).ok_or("Elves of Deep Shadow exact green payment is missing")?;
    let elf_cast = find_event_after(
        events,
        elf_paid,
        |e| matches!(e, GameEvent::SpellCast { player: PlayerId(1), card } if *card == elf),
    )
    .ok_or("Elf SpellCast is missing")?;
    let elf_resolved = find_event_after(
        events,
        elf_cast,
        |e| matches!(e, GameEvent::SpellResolved { card } if *card == elf),
    )
    .ok_or("Elf SpellResolved is missing")?;
    let elf_entered = find_event_after(
        events,
        elf_resolved,
        |e| matches!(e, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == elf),
    )
    .ok_or("Elf battlefield entry is missing")?;
    let elf_attack = find_event_after(events, elf_entered, |e| matches!(e, GameEvent::AttackersDeclared { player: PlayerId(1), attackers } if attackers.contains(&elf))).ok_or("opposing Elf did not legally attack")?;
    if caregiver_entered >= elf_attack {
        return Err("Caregiver did not enter before the prevention window".to_owned());
    }
    let ability_paid = find_event_after(events, elf_attack, |e| matches!(e, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: ABILITY_ID, mana_cost } if *source == caregiver && *mana_cost == ManaCost::with_colors(0, [Color::White]))).ok_or("Caregiver exact white AbilityManaPaid event is missing")?;
    let sacrificed = find_event_after(events, ability_paid, |e| matches!(e, GameEvent::SacrificedAsAbilityCost { player: PlayerId(0), source, permanent } if *source == caregiver && *permanent == caregiver)).ok_or("Caregiver was not sacrificed as its activation cost")?;
    let source_moved = find_event_after(
        events,
        sacrificed,
        |e| matches!(e, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == caregiver),
    )
    .ok_or("sacrificed Caregiver did not move to its graveyard")?;
    let activated = find_event_after(events, source_moved, |e| matches!(e, GameEvent::AbilityActivated { player: PlayerId(0), source, definition: CARD_ID, ability: ABILITY_ID, .. } if *source == caregiver)).ok_or("Caregiver AbilityActivated receipt is missing")?;
    let shield = find_event_after(events, activated, |e| matches!(e, GameEvent::DamageShieldCreated { source, target: Target::Player(PlayerId(0)), amount: 1 } if *source == caregiver)).ok_or("Caregiver did not create exactly one shield for seat zero")?;
    let resolved = find_event_after(events, shield, |e| matches!(e, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == caregiver)).ok_or("Caregiver ability resolution receipt is missing")?;
    let blocks = find_event_after(events, resolved, |e| matches!(e, GameEvent::BlockersDeclared { player: PlayerId(0), assignments } if assignments.is_empty())).ok_or("seat zero's legal empty blocker declaration is missing")?;
    let prevented = find_event_after(events, blocks, |e| matches!(e, GameEvent::DamagePrevented { source, target: Target::Player(PlayerId(0)), amount: 1 } if *source == elf)).ok_or("Caregiver shield did not prevent the Elf's next damage")?;
    if events.iter().enumerate().any(|(index, e)| index > elf_attack && index < prevented && matches!(e, GameEvent::DamageDealtToPlayer { source, player: PlayerId(0), .. } if *source == elf)) {
        return Err("prevented Elf damage was also recorded as damage dealt".to_owned());
    }
    let wolf_paid = find_event(events, |e| matches!(e, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == watchwolf && colors.len() == 2 && colors.contains(&Color::Green) && colors.contains(&Color::White))).ok_or("Watchwolf exact green-white spell payment is missing")?;
    let wolf_cast = find_event_after(
        events,
        wolf_paid,
        |e| matches!(e, GameEvent::SpellCast { player: PlayerId(0), card } if *card == watchwolf),
    )
    .ok_or("Watchwolf SpellCast is missing")?;
    let wolf_resolved = find_event_after(
        events,
        wolf_cast,
        |e| matches!(e, GameEvent::SpellResolved { card } if *card == watchwolf),
    )
    .ok_or("Watchwolf SpellResolved is missing")?;
    let wolf_entered = find_event_after(
        events,
        wolf_resolved,
        |e| matches!(e, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == watchwolf),
    )
    .ok_or("Watchwolf battlefield entry is missing")?;
    let wolf_attack = find_event_after(events, wolf_entered, |e| matches!(e, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&watchwolf))).ok_or("Watchwolf did not attack")?;
    let first_wolf_damage = find_event_after(events, wolf_attack, |e| matches!(e, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 3 } if *source == watchwolf)).ok_or("Watchwolf did not deal three combat damage")?;
    let ended = find_event_after(events, first_wolf_damage, |e| {
        matches!(
            e,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("seat zero's terminal win receipt is missing")?;
    let damage_total: i64 = events
        .iter()
        .filter_map(|e| match e {
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if *source == watchwolf => Some(i64::from(*amount)),
            _ => None,
        })
        .sum();
    if damage_total < 20
        || !(caregiver_paid < caregiver_cast
            && caregiver_cast < caregiver_resolved
            && caregiver_resolved < caregiver_entered
            && elf_paid < elf_cast
            && elf_cast < elf_resolved
            && elf_resolved < elf_entered
            && elf_attack < ability_paid
            && ability_paid < sacrificed
            && sacrificed < source_moved
            && source_moved < activated
            && activated < shield
            && shield < resolved
            && resolved < blocks
            && blocks < prevented
            && prevented < wolf_paid
            && wolf_paid < wolf_cast
            && wolf_cast < wolf_resolved
            && wolf_resolved < wolf_entered
            && wolf_entered < wolf_attack
            && wolf_attack < first_wolf_damage
            && first_wolf_damage < ended)
    {
        return Err(
            "Caregiver cast, sacrifice, prevention, and terminal combat event order differs"
                .to_owned(),
        );
    }
    let mut selected = BTreeSet::from([
        caregiver_paid,
        caregiver_cast,
        caregiver_resolved,
        caregiver_entered,
        elf_paid,
        elf_cast,
        elf_resolved,
        elf_entered,
        elf_attack,
        ability_paid,
        sacrificed,
        source_moved,
        activated,
        shield,
        resolved,
        blocks,
        prevented,
        wolf_paid,
        wolf_cast,
        wolf_resolved,
        wolf_entered,
        ended,
    ]);
    for (index, event) in events.iter().enumerate() {
        if matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&watchwolf))
            || matches!(
                event,
                GameEvent::BlockersDeclared {
                    player: PlayerId(1),
                    ..
                }
            )
            || matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 3 } if *source == watchwolf)
        {
            selected.insert(index);
        }
    }
    let projected: Vec<ProjectedEvent> = project_events(events)
        .into_iter()
        .enumerate()
        .filter_map(|(i, event)| selected.contains(&i).then_some(event))
        .collect();
    let kinds: Vec<_> = projected.iter().map(|event| event.kind.as_str()).collect();
    for required in [
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "AbilityManaPaid",
        "SacrificedAsAbilityCost",
        "AbilityActivated",
        "DamageShieldCreated",
        "AbilityResolved",
        "AttackersDeclared",
        "BlockersDeclared",
        "DamagePrevented",
        "DamageDealtToPlayer",
        "GameEnded",
    ] {
        if !kinds.contains(&required) {
            return Err(format!(
                "projected Caregiver trace omitted typed event {required}: {kinds:?}"
            ));
        }
    }
    let sequences: Vec<_> = projected.iter().map(|event| event.sequence).collect();
    if sequences.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err("projected event sequences are not strictly ordered".to_owned());
    }
    Ok(json!({
        "trace_id": "rav-caregiver-sacrifice-targeted-prevention-full-game",
        "shuffle_seed": seed,
        "decks": ["rav_caregiver_watchwolf", "rav_elf_pressure"],
        "policies": ["rav.caregiver-shield-and-watchwolf.v1", "rav.caregiver-elf-pressure.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted,
        "rejected_moves": captured.rejected,
        "caregiver": {"object": caregiver.0, "collector_number": 6, "mana_cost": "{W}", "mana_paid": ["White"], "colors": ["White"], "card_types": ["Creature"], "power": 1, "toughness": 1, "characteristics_checked_on_battlefield_entry": true, "zone_after_cost": "Graveyard"},
        "activation": {"ability": ABILITY_ID, "mana_cost": "{W}", "mana_paid": "White", "sacrificed_source": caregiver.0, "shield_target": "PlayerId(0)", "shield_amount": 1, "prevention_resolved_before_combat_damage": resolved < prevented},
        "combat": {"attacking_elf": elf.0, "damage_prevented": 1, "prevented_damage_was_not_dealt": true, "watchwolf": {"object": watchwolf.0, "power": 3, "toughness": 3, "attacks": events.iter().filter(|event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&watchwolf))).count(), "total_damage_to_opponent": damage_total}, "terminal_life": captured.player_life},
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
    let definition_id = card_definitions()
        .into_iter()
        .find(|definition| definition.name == card.name)
        .map(|definition| definition.id)
        .unwrap_or("");
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"engine_status":if definition_id.is_empty(){"catalog_only"}else{"executable_compatibility_slice"},"trace_status":if definition_id==CARD_ID{"passed"}else{"not_qualified"},"qualification":if definition_id==CARD_ID{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}
