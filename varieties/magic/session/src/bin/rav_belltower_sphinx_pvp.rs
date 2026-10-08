//! Full-game RAV Belltower Sphinx damage-trigger trace authority.

use cardbench_magic_engine::{
    AbilityActivation, CardDefinition, CardType, CastRequest, Color, DeckEntry, DeckList, Effect,
    Game, GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId,
    PolicyAction, Step, Target, TriggerCondition, TriggeredAbilityBinding, Zone,
};
use cardbench_magic_policies::{
    CatalogPolicyProfile, CodePolicy, DeckMatchConfig, RavCatalogPolicy,
};
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
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::{Expr, Item, Lit, Meta, PathArguments, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-BELLTOWER-SPHINX";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/belltower_sphinx.rs";
const FANGTAIL_ID: &str = "RAV-VIASHINO-FANGTAIL";
const FANGTAIL_ABILITY: &str = "tap-deal-one-to-player-or-creature";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/belltower_sphinx.rs"]
mod candidate_belltower_sphinx;

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
            eprintln!("Belltower Sphinx PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_runtime()?;
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<CatalogCard> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
    {
        return Err("RAV catalog denominator differs from the pinned source contract".to_owned());
    }

    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.terminal != replay.terminal
        || captured.winner != replay.winner
        || captured.sphinx != replay.sphinx
        || captured.fangtail != replay.fangtail
        || captured.milled != replay.milled
    {
        return Err("Belltower Sphinx full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "flying_removal_detected_in_game": mutation_detected(seed, Mutation::RemoveFlying),
        "trigger_removal_detected_in_game": mutation_detected(seed, Mutation::RemoveTrigger),
        "mill_count_change_detected_in_game": mutation_detected(seed, Mutation::DoubleMill),
        "sealed_mutation_authority": false
    });
    for key in [
        "flying_removal_detected_in_game",
        "trigger_removal_detected_in_game",
        "mill_count_change_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Belltower Sphinx gameplay mutation escaped the trace oracle: {key}"
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
        "card": {"collector_number": 38, "definition_id": CARD_ID, "name": "Belltower Sphinx"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": ["117 Priority", "120 Damage", "302 Creatures", "601.2 Casting spells", "603 Triggered abilities", "608 Resolving spells and abilities", "702.9 Flying"],
            "source_contract": "RAV Belltower Sphinx is a 2/5 blue creature with flying; when it is dealt damage, its mandatory trigger mills that many cards from the controller of the damage source",
            "authority_cutoff": "2026-09-23"
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
        "qualification_status": "unreviewed_diagnostic",
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy)]
enum Mutation {
    None,
    RemoveFlying,
    RemoveTrigger,
    DoubleMill,
}

fn candidate_values(mutation: Mutation) -> (CardDefinition, Option<TriggeredAbilityBinding>) {
    let mut definition = candidate_belltower_sphinx::definition();
    let mut trigger = Some(candidate_belltower_sphinx::triggered_ability());
    match mutation {
        Mutation::None => {}
        Mutation::RemoveFlying => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Flying),
        Mutation::RemoveTrigger => trigger = None,
        Mutation::DoubleMill => {
            if let Some(binding) = &mut trigger {
                binding
                    .ability
                    .effects
                    .push(Effect::MillSourceControllerFromSourceDamage);
            }
        }
    }
    (definition, trigger)
}

fn validate_candidate_runtime() -> Result<(), String> {
    let (definition, trigger) = candidate_values(Mutation::None);
    let registered = card_definitions()
        .into_iter()
        .find(|item| item.id == CARD_ID)
        .ok_or("Belltower Sphinx is missing from the executable RAV registry")?;
    let registered_trigger = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Belltower Sphinx trigger is missing from the executable RAV registry")?;
    if registered != definition || Some(registered_trigger) != trigger {
        return Err(
            "Belltower Sphinx definition and trigger are not delegated to its candidate module"
                .to_owned(),
        );
    }
    let trigger = trigger.expect("validated above");
    if definition.id != CARD_ID
        || definition.name != "Belltower Sphinx"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(4, [Color::Blue])
        || definition.colors != BTreeSet::from([Color::Blue])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || !definition.effects.is_empty()
        || trigger.card_definition != CARD_ID
        || trigger.ability.id != "damage-source-controller-mill-that-many"
        || trigger.ability.condition != TriggerCondition::ReceivesDamage
        || trigger.ability.mana_cost != ManaCost::new(0)
        || trigger.ability.optional
        || !trigger.ability.targets.is_empty()
    {
        return Err("Belltower Sphinx identity or candidate-owned trigger differs from its feature contract".to_owned());
    }
    Ok(())
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation) {
        Ok(captured) => validate_trace(&captured, seed).is_err(),
        Err(_) => true,
    }
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(raw) = std::env::var("CARDBENCH_BELLTOWER_SPHINX_SEED") {
        let seed = raw
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_BELLTOWER_SPHINX_SEED must be an unsigned integer")?;
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
        "no complete Belltower Sphinx trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

#[derive(Clone, Debug)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    sphinx: Option<ObjectId>,
    fangtail: Option<ObjectId>,
    sphinx_stats: Option<(i32, i32)>,
    sphinx_keywords: Vec<Keyword>,
    paid: Vec<Color>,
    milled: Vec<ObjectId>,
    milled_definitions: Vec<Option<&'static str>>,
    p1_library_before: Option<usize>,
    p1_library_after: Option<usize>,
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let (definition, trigger) = candidate_values(mutation);
    let mut game = new_game(definition, trigger)?;
    let p0 = DeckList {
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
                card: "RAV-ISLAND".to_owned(),
                count: 16,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 16,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 16,
            },
            DeckEntry {
                card: "RAV-SNAPPING-DRAKE".to_owned(),
                count: 4,
            },
        ],
        sideboard: vec![],
    };
    let p1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: FANGTAIL_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    for (seat, deck) in [(0, &p0), (1, &p1)] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!(
                "seat {seat} constructed list has {count} cards; expected 60"
            ));
        }
    }
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &p0)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &p1)
        .map_err(|error| error.to_string())?;
    let p1_card_definitions: BTreeMap<ObjectId, &'static str> = game.players[1]
        .library
        .iter()
        .filter_map(|card| {
            game.card_definition(*card)
                .ok()
                .map(|definition| (*card, definition.id))
        })
        .collect();
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(SphinxPolicy::new(PlayerId(0))),
        Box::new(SphinxPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut p1_library_before = None;
    let mut p1_library_after = None;
    let mut p1_library_before_cards = Vec::new();
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Belltower Sphinx match reached its bounded run limit".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Belltower policy seat missing")?;
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
                    "Belltower policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        let sphinx = game.event_log.iter().find_map(|event| match event {
            GameEvent::TriggeredAbilityStacked {
                source, ability, ..
            } if *ability == "damage-source-controller-mill-that-many"
                && game
                    .card_definition(*source)
                    .is_ok_and(|card| card.id == CARD_ID) =>
            {
                Some(*source)
            }
            _ => None,
        });
        if sphinx.is_some() && p1_library_before.is_none() {
            p1_library_before = Some(game.players[1].library.len());
            p1_library_before_cards = game.players[1].library.clone();
        }
        if sphinx.is_some() && p1_library_after.is_none() && game.event_log.iter().any(|event| matches!(event,
            GameEvent::AbilityResolved { source, ability: "damage-source-controller-mill-that-many", .. } if Some(*source) == sphinx)) {
            p1_library_after = Some(game.players[1].library.len());
        }
    }
    let events = game.event_log.clone();
    let sphinx = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|item| item.id == CARD_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let fangtail = events.iter().find_map(|event| match event {
        GameEvent::AbilityActivated {
            player: PlayerId(1),
            source,
            definition: FANGTAIL_ID,
            ability: FANGTAIL_ABILITY,
            ..
        } => Some(*source),
        _ => None,
    });
    let paid = sphinx
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
    let sphinx_stats = sphinx
        .and_then(|object| game.characteristics(object).ok())
        .and_then(|chars| Some((chars.power?, chars.toughness?)));
    let sphinx_keywords = sphinx
        .and_then(|object| game.characteristics(object).ok())
        .map(|chars| chars.keywords.clone())
        .unwrap_or_default();
    let trigger_stacked = sphinx.and_then(|object| events.iter().position(|event| matches!(event, GameEvent::TriggeredAbilityStacked { source, ability: "damage-source-controller-mill-that-many", .. } if *source == object)));
    let ability_resolved = sphinx.and_then(|object| events.iter().position(|event| matches!(event, GameEvent::AbilityResolved { source, ability: "damage-source-controller-mill-that-many", .. } if *source == object)));
    let milled = if let (Some(stack), Some(resolved)) = (trigger_stacked, ability_resolved) {
        events
            .iter()
            .enumerate()
            .filter_map(|(index, event)| {
                if index <= stack || index >= resolved {
                    return None;
                }
                let GameEvent::CardMoved {
                    card,
                    to: Zone::Graveyard,
                } = event
                else {
                    return None;
                };
                p1_library_before_cards.contains(card).then_some(*card)
            })
            .collect()
    } else {
        vec![]
    };
    let milled_definitions = milled
        .iter()
        .map(|card| p1_card_definitions.get(card).copied())
        .collect();
    let winner = game.winner().map(|player| player.0);
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
        sphinx,
        fangtail,
        sphinx_stats,
        sphinx_keywords,
        paid,
        milled,
        milled_definitions,
        p1_library_before,
        p1_library_after,
    })
}

fn new_game(
    definition: CardDefinition,
    trigger: Option<TriggeredAbilityBinding>,
) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|item| item.id == CARD_ID)
        .ok_or("Belltower Sphinx missing from registry")? = definition;
    let mut triggers = rav_triggered_ability_bindings();
    triggers.retain(|binding| binding.card_definition != CARD_ID);
    if let Some(trigger) = trigger {
        triggers.push(trigger);
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

struct SphinxPolicy {
    player: PlayerId,
    inner: RavCatalogPolicy,
    ping_offered: bool,
    sphinx_cast_offered: bool,
    fangtail_cast_offered: bool,
}

impl SphinxPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
            ping_offered: false,
            sphinx_cast_offered: false,
            fangtail_cast_offered: false,
        }
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
        let mut available: BTreeMap<Color, u8> = Color::ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        let mut can_pay = true;
        for color in &cost.colored {
            match available.get_mut(color) {
                Some(amount) if *amount > 0 => *amount -= 1,
                _ => can_pay = false,
            }
        }
        let mut generic = Vec::new();
        if can_pay && cost.hybrid.is_empty() {
            for _ in 0..cost.generic {
                let Some(color) = available
                    .iter()
                    .filter(|(_, count)| **count > 0)
                    .max_by_key(|(_, count)| **count)
                    .map(|(color, _)| *color)
                else {
                    can_pay = false;
                    break;
                };
                *available.get_mut(&color)? -= 1;
                generic.push(color);
            }
        } else {
            can_pay = false;
        }
        if can_pay {
            if card_id == CARD_ID {
                self.sphinx_cast_offered = true;
            }
            if card_id == FANGTAIL_ID {
                self.fangtail_cast_offered = true;
            }
            return Some(PolicyAction::CastWithPayment {
                request: CastRequest {
                    card: card.id,
                    targets: vec![],
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
        let missing_color = cost
            .colored
            .iter()
            .find(|color| view.mana_pool.amount(**color) == 0)
            .copied();
        let total_required =
            u16::from(cost.generic) + cost.colored.len() as u16 + cost.hybrid.len() as u16;
        let wanted = missing_color.or_else(|| {
            (u16::from(view.mana_pool.total()) < total_required)
                .then(|| cost.colored.first().copied().unwrap_or(Color::Blue))
        });
        let land = view
            .own_battlefield
            .iter()
            .find(|land| {
                !land.tapped
                    && land.card_types.contains(&CardType::Land)
                    && wanted.map_or(!land.mana_colors.is_empty(), |color| {
                        land.mana_colors.contains(&color)
                    })
            })
            .or_else(|| {
                view.own_battlefield.iter().find(|land| {
                    !land.tapped
                        && land.card_types.contains(&CardType::Land)
                        && !land.mana_colors.is_empty()
                })
            })?;
        let color = wanted
            .filter(|color| land.mana_colors.contains(color))
            .or_else(|| land.mana_colors.iter().next().copied())?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }
}

impl CodePolicy for SphinxPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.belltower-sphinx-cast-trigger-win.v1"
        } else {
            "rav.belltower-sphinx-fangtail-source-damage.v1"
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
                attackers: if self.player == PlayerId(0) {
                    view.own_battlefield
                        .iter()
                        .filter(|card| card.can_attack)
                        .map(|card| card.id)
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
        if self.player == PlayerId(1) && !self.ping_offered && view.priority == self.player {
            let source = view.own_battlefield.iter().find(|card| {
                card.definition == Some(FANGTAIL_ID) && !card.tapped && !card.summoning_sick
            });
            let target = view.opponent_battlefield.iter().find(|card| {
                card.definition == Some(CARD_ID) && card.card_types.contains(&CardType::Creature)
            });
            if let (Some(source), Some(target)) = (source, target) {
                self.ping_offered = true;
                return PolicyAction::ActivateAbility {
                    activation: AbilityActivation {
                        source: source.id,
                        ability_id: FANGTAIL_ABILITY,
                        sacrifice_sources: vec![],
                        additional_tap_creatures: vec![],
                        discard_cards: vec![],
                        targets: vec![Target::Permanent(target.id)],
                    },
                };
            }
        }
        if view.priority != self.player
            || view.active_player != self.player
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain) {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            let preferred = if self.player == PlayerId(1) {
                "RAV-MOUNTAIN"
            } else {
                "RAV-ISLAND"
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
        if self.player == PlayerId(0) && !self.sphinx_cast_offered {
            let cost = ManaCost::with_colors(4, [Color::Blue]);
            if let Some(action) = self.cast_action(view, CARD_ID, &cost) {
                return action;
            }
        }
        if self.player == PlayerId(1) && !self.fangtail_cast_offered {
            let cost = ManaCost::with_colors(2, [Color::Red, Color::Red]);
            if let Some(action) = self.cast_action(view, FANGTAIL_ID, &cost) {
                return action;
            }
        }
        let action = self.inner.propose_move(view);
        if self.player == PlayerId(1)
            && matches!(&action, PolicyAction::ActivateAbility { activation } if activation.ability_id == FANGTAIL_ABILITY)
        {
            PolicyAction::PassPriority
        } else {
            action
        }
    }
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))"
        || captured.winner != Some(0)
        || captured.rejected != 0
    {
        return Err("Belltower Sphinx game did not end in a clean seat-zero win".to_owned());
    }
    let sphinx = captured
        .sphinx
        .ok_or("seat zero did not cast Belltower Sphinx")?;
    let fangtail = captured
        .fangtail
        .ok_or("seat one did not activate Viashino Fangtail")?;
    if captured.sphinx_stats != Some((2, 5)) || !captured.sphinx_keywords.contains(&Keyword::Flying)
    {
        return Err(format!(
            "resolved Belltower Sphinx is not a 2/5 with Flying: {:?} {:?}",
            captured.sphinx_stats, captured.sphinx_keywords
        ));
    }
    if captured.paid.len() != 5 || !captured.paid.contains(&Color::Blue) {
        return Err(format!(
            "Belltower Sphinx payment is not {{4}}{{U}}: {:?}",
            captured.paid
        ));
    }
    if captured.milled.len() != 1
        || captured
            .p1_library_before
            .zip(captured.p1_library_after)
            .is_none_or(|(before, after)| before != after + 1)
    {
        return Err(format!(
            "Belltower Sphinx did not mill exactly one card from its damage source controller: milled={}, library={:?}->{:?}",
            captured.milled.len(),
            captured.p1_library_before,
            captured.p1_library_after
        ));
    }
    let events = &captured.events;
    let fangtail_cast = find_event(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if events.iter().any(|later| matches!(later, GameEvent::AbilityActivated { player: PlayerId(1), source, definition: FANGTAIL_ID, ability: FANGTAIL_ABILITY, .. } if source == card)))).ok_or("seat one did not cast the Fangtail that dealt damage")?;
    let fangtail_paid = find_event(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(1), card, colors } if Some(*card) == captured.fangtail && colors.len() == 4 && colors.iter().filter(|color| **color == Color::Red).count() >= 2)).ok_or("Viashino Fangtail payment event missing")?;
    if fangtail_paid >= fangtail_cast {
        return Err("Viashino Fangtail payment did not precede its cast".to_owned());
    }
    let fangtail_resolved = find_event_after(events, fangtail_cast, |event| matches!(event, GameEvent::SpellResolved { card } if Some(*card) == captured.fangtail)).ok_or("Viashino Fangtail SpellResolved missing")?;
    let fangtail_entered = find_event_after(events, fangtail_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if Some(*card) == captured.fangtail)).ok_or("Viashino Fangtail Battlefield entry missing")?;
    let paid = find_event(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == sphinx && colors.len() == 5 && colors.contains(&Color::Blue))).ok_or("Belltower Sphinx payment event missing")?;
    let cast = find_event(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == sphinx)).ok_or("Belltower Sphinx SpellCast missing")?;
    if paid >= cast {
        return Err("Belltower Sphinx payment did not precede its cast".to_owned());
    }
    let resolved = find_event_after(
        events,
        cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == sphinx),
    )
    .ok_or("Belltower Sphinx SpellResolved missing")?;
    let entered = find_event_after(events, resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == sphinx)).ok_or("Belltower Sphinx Battlefield entry missing")?;
    let activated = find_event_after(events, fangtail_cast, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(1), source, definition: FANGTAIL_ID, ability: FANGTAIL_ABILITY, .. } if *source == fangtail)).ok_or("Fangtail activation receipt missing")?;
    let damage = find_event_after(events, activated, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 1 } if *source == fangtail && *permanent == sphinx)).ok_or("Fangtail did not deal exactly one damage to Belltower Sphinx")?;
    let stacked = find_event_after(events, damage, |event| matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, ability: "damage-source-controller-mill-that-many", .. } if *source == sphinx)).ok_or("Belltower damage trigger did not enter the stack")?;
    let ability_resolved = find_event_after(events, stacked, |event| matches!(event, GameEvent::AbilityResolved { source, ability: "damage-source-controller-mill-that-many", .. } if *source == sphinx)).ok_or("Belltower mill trigger did not resolve")?;
    let actual_mills: Vec<_> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            if index <= stacked || index >= ability_resolved {
                return None;
            }
            let GameEvent::CardMoved {
                card,
                to: Zone::Graveyard,
            } = event
            else {
                return None;
            };
            captured.milled.contains(card).then_some(index)
        })
        .collect();
    if actual_mills.len() != 1 {
        return Err(format!(
            "mill movement is not exactly one source-controller card: {actual_mills:?}"
        ));
    }
    let attack = find_event_after(events, ability_resolved, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&sphinx))).ok_or("Belltower Sphinx did not attack after its damage trigger resolved")?;
    let ended = find_event_after(events, attack, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("seat-zero terminal PvP win missing after the Sphinx attack")?;
    let selected = BTreeSet::from([
        fangtail_paid,
        fangtail_cast,
        fangtail_resolved,
        fangtail_entered,
        paid,
        cast,
        resolved,
        entered,
        activated,
        damage,
        stacked,
        ability_resolved,
        attack,
        ended,
    ]);
    let mut projected: Vec<ProjectedEvent> = project_events(events)
        .into_iter()
        .enumerate()
        .filter_map(|(index, event)| selected.contains(&index).then_some(event))
        .collect();
    for index in &actual_mills {
        if let Some(event) = project_events(events).into_iter().nth(*index) {
            projected.push(event);
        }
    }
    projected.sort_by_key(|event| event.sequence);
    let kinds: Vec<_> = projected.iter().map(|event| event.kind.as_str()).collect();
    let expected = [
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "AbilityActivated",
        "DamageDealtToPermanent",
        "TriggeredAbilityStacked",
        "CardMoved",
        "AbilityResolved",
        "AttackersDeclared",
        "GameEnded",
    ];
    if kinds != expected {
        return Err(format!(
            "Belltower ordered event trace differs: {kinds:?}; projected={projected:#?}"
        ));
    }
    Ok(json!({
        "id": "rav-belltower-sphinx-damage-source-mill-pvp",
        "seed": seed,
        "decks": ["rav-belltower-sphinx-bant-curve", "rav-viashino-fangtail-mountains"],
        "policies": ["rav.belltower-sphinx-cast-trigger-win.v1", "rav.belltower-sphinx-fangtail-source-damage.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted,
        "rejected_moves": captured.rejected,
        "digest": event_digest(events),
        "mechanics": {
            "sphinx_object": sphinx.0,
            "sphinx_mana_cost": "{4}{U}",
            "sphinx_mana_paid": captured.paid.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "sphinx_characteristics": {"power": 2, "toughness": 5, "keywords": ["Flying"]},
            "damage_source_object": fangtail.0,
            "damage_source_controller_seat": 1,
            "damage_source_ability": FANGTAIL_ABILITY,
            "damage_amount": 1,
            "trigger_stacked": true,
            "trigger_resolved": true,
            "mill_count": captured.milled.len(),
            "milled_card_owner_seat": 1,
            "milled_card_object": captured.milled.first().map(|card| card.0),
            "milled_card_definition": captured.milled_definitions.first().copied().flatten(),
            "controller_library_before": captured.p1_library_before,
            "controller_library_after": captured.p1_library_after,
            "fangtail_mana_paid": match &events[fangtail_paid] { GameEvent::SpellManaPaid { colors, .. } => colors.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), _ => unreachable!() },
            "ordered_event_sequence": [fangtail_paid, fangtail_cast, fangtail_resolved, fangtail_entered, paid, cast, resolved, entered, activated, damage, stacked, actual_mills[0], ability_resolved, attack, ended]
        },
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

fn event_digest(events: &[GameEvent]) -> String {
    use std::fmt::Write as _;
    let mut text = String::new();
    for event in events {
        let _ = writeln!(text, "{event:?}");
    }
    let hash = text
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });
    format!("fnv1a64:{hash:016x}")
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number": card.collector_number, "name": card.name, "definition_id": definition_id,
        "engine_status": match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" },
        "trace_status": if passed {"passed"} else {"not_qualified"}, "credited": false,
        "qualification": if passed {"pilot_trace_passed; independent reviewer approval pending"} else {"not_evaluated"}})
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Belltower Sphinx source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Belltower Sphinx source is not valid declarative Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) || file.items.len() != 4 {
        return Err("Belltower Sphinx module contains unsupported top-level items".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TriggerCondition"],
        vec!["cardbench_magic_engine", "TriggeredAbility"],
        vec!["cardbench_magic_engine", "TriggeredAbilityBinding"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut definition = None;
    let mut trigger = None;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let Some(paths) = flattened_use_paths(item) else {
                    return Err(
                        "Belltower Sphinx imports are outside the source contract".to_owned()
                    );
                };
                imports.extend(paths);
            }
            Item::Fn(item) if item.sig.ident == "definition" && definition.is_none() => {
                definition = Some(item)
            }
            Item::Fn(item) if item.sig.ident == "triggered_ability" && trigger.is_none() => {
                trigger = Some(item)
            }
            _ => {
                return Err(
                    "Belltower Sphinx module has executable or duplicate top-level items"
                        .to_owned(),
                );
            }
        }
    }
    if imports != expected_imports {
        return Err("Belltower Sphinx imports differ from its source contract".to_owned());
    }
    let definition = direct_struct(
        definition.ok_or("Belltower Sphinx definition() is missing")?,
        "definition",
        "CardDefinition",
    )?;
    let fields = literal_fields(definition)?;
    if fields.len() != 13
        || !check_string(fields["id"], CARD_ID)
        || !check_string(fields["name"], "Belltower Sphinx")
        || !check_string(fields["set_code"], "RAV")
        || !check_mana_cost(fields["mana_cost"])
        || !check_set(fields["colors"], &[&["Color", "Blue"]])
        || !check_empty_set(fields["mana_colors"])
        || !check_set(fields["card_types"], &[&["CardType", "Creature"]])
        || !check_bool(fields["is_basic_land"], false)
        || !check_rules(
            fields["supported_rules"],
            &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "flying",
                "damage-received-source-controller-mill-that-many",
            ],
        )
        || !check_some_number(fields["power"])
        || !check_some_number(fields["toughness"])
        || !check_keyword_vec(fields["keywords"])
        || !check_empty_vec(fields["effects"])
    {
        return Err(
            "Belltower Sphinx definition is outside its literal source contract".to_owned(),
        );
    }
    let trigger = direct_struct(
        trigger.ok_or("Belltower Sphinx triggered_ability() is missing")?,
        "triggered_ability",
        "TriggeredAbilityBinding",
    )?;
    let trigger_fields = literal_fields(trigger)?;
    if trigger_fields.len() != 2 || !check_string(trigger_fields["card_definition"], CARD_ID) {
        return Err("Belltower Sphinx trigger binding has an invalid identity".to_owned());
    }
    let ability = direct_struct_expr(trigger_fields["ability"], "TriggeredAbility")?;
    let ability_fields = literal_fields(ability)?;
    if ability_fields.len() != 6
        || !check_string(
            ability_fields["id"],
            "damage-source-controller-mill-that-many",
        )
        || !check_path(
            ability_fields["condition"],
            &["TriggerCondition", "ReceivesDamage"],
        )
        || !check_zero_cost(ability_fields["mana_cost"])
        || !check_bool(ability_fields["optional"], false)
        || !check_empty_vec(ability_fields["targets"])
        || !check_mill_effect_vec(ability_fields["effects"])
    {
        return Err("Belltower Sphinx trigger is outside its literal source contract".to_owned());
    }
    Ok(())
}

fn direct_struct<'a>(
    function: &'a syn::ItemFn,
    name: &str,
    return_type: &str,
) -> Result<&'a syn::ExprStruct, String> {
    let attrs_ok = function.attrs.iter().all(|attr| {
        is_doc_attribute(attr)
            || attr.path().is_ident("must_use") && matches!(attr.meta, Meta::Path(_))
    });
    let crate_visibility =
        matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"));
    if !attrs_ok
        || !crate_visibility
        || function.sig.ident != name
        || function.sig.constness.is_some()
        || function.sig.asyncness.is_some()
        || function.sig.unsafety.is_some()
        || function.sig.abi.is_some()
        || !function.sig.generics.params.is_empty()
        || function.sig.generics.where_clause.is_some()
        || !function.sig.inputs.is_empty()
        || function.sig.variadic.is_some()
        || function.block.stmts.len() != 1
        || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if path_is_type(ty, return_type))
    {
        return Err(format!(
            "Belltower Sphinx {name}() has an unsupported signature"
        ));
    }
    direct_struct_expr(
        match &function.block.stmts[0] {
            Stmt::Expr(expression, None) => expression,
            _ => {
                return Err(format!(
                    "Belltower Sphinx {name}() must return one direct literal"
                ));
            }
        },
        return_type,
    )
}

fn direct_struct_expr<'a>(expr: &'a Expr, ty: &str) -> Result<&'a syn::ExprStruct, String> {
    let Expr::Struct(value) = expr else {
        return Err(format!(
            "Belltower Sphinx return must be a direct {ty} literal"
        ));
    };
    if !path_is(&value.path, &[ty]) || value.rest.is_some() || !value.attrs.is_empty() {
        return Err(format!(
            "Belltower Sphinx return must be a direct {ty} literal"
        ));
    }
    Ok(value)
}

fn literal_fields(value: &syn::ExprStruct) -> Result<BTreeMap<String, &Expr>, String> {
    let mut fields = BTreeMap::new();
    for field in &value.fields {
        let syn::Member::Named(name) = &field.member else {
            return Err("Belltower Sphinx literal fields must be named".to_owned());
        };
        if field.colon_token.is_none() || fields.insert(name.to_string(), &field.expr).is_some() {
            return Err("Belltower Sphinx literal has shorthand or duplicate fields".to_owned());
        }
    }
    Ok(fields)
}

fn flattened_use_paths(item: &syn::ItemUse) -> Option<Vec<Vec<String>>> {
    fn walk(prefix: &[String], tree: &UseTree, out: &mut Vec<Vec<String>>) -> bool {
        match tree {
            UseTree::Path(path) => {
                let mut next = prefix.to_vec();
                next.push(path.ident.to_string());
                walk(&next, &path.tree, out)
            }
            UseTree::Name(name) => {
                let mut next = prefix.to_vec();
                next.push(name.ident.to_string());
                out.push(next);
                true
            }
            UseTree::Group(group) => group.items.iter().all(|item| walk(prefix, item, out)),
            _ => false,
        }
    }
    if item.leading_colon.is_some() {
        return None;
    }
    let mut out = Vec::new();
    walk(&[], &item.tree, &mut out).then_some(out)
}

fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc") && matches!(attribute.meta, Meta::NameValue(_))
}
fn path_is_type(ty: &syn::Type, name: &str) -> bool {
    matches!(ty, syn::Type::Path(path) if path.qself.is_none() && path_is(&path.path, &[name]))
}
fn path_is(path: &syn::Path, expected: &[&str]) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == expected.len()
        && path.segments.iter().zip(expected).all(|(segment, name)| {
            segment.ident == *name && matches!(&segment.arguments, PathArguments::None)
        })
}
fn check_path(expr: &Expr, expected: &[&str]) -> bool {
    matches!(expr, Expr::Path(path) if path.attrs.is_empty() && path.qself.is_none() && path_is(&path.path, expected))
}
fn check_string(expr: &Expr, expected: &str) -> bool {
    matches!(expr, Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Str(lit) if lit.value() == expected))
}
fn check_bool(expr: &Expr, expected: bool) -> bool {
    matches!(expr, Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Bool(lit) if lit.value == expected))
}
fn check_int(expr: &Expr, expected: u64) -> bool {
    matches!(expr, Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Int(lit) if lit.base10_parse::<u64>().ok() == Some(expected)))
}
fn array_items(expr: &Expr) -> Option<Vec<&Expr>> {
    match expr {
        Expr::Array(array) if array.attrs.is_empty() => Some(array.elems.iter().collect()),
        _ => None,
    }
}
fn call_args<'a>(expr: &'a Expr, expected: &[&str]) -> Option<Vec<&'a Expr>> {
    match expr {
        Expr::Call(call) if call.attrs.is_empty() && check_path(&call.func, expected) => {
            Some(call.args.iter().collect())
        }
        _ => None,
    }
}
fn check_set(expr: &Expr, expected: &[&[&str]]) -> bool {
    let Some(args) = call_args(expr, &["BTreeSet", "from"]) else {
        return false;
    };
    if args.len() != 1 {
        return false;
    }
    let Some(items) = array_items(args[0]) else {
        return false;
    };
    items.len() == expected.len()
        && items
            .iter()
            .zip(expected)
            .all(|(item, path)| check_path(item, path))
}
fn check_empty_set(expr: &Expr) -> bool {
    call_args(expr, &["BTreeSet", "new"]).is_some_and(|args| args.is_empty())
}
fn check_mana_cost(expr: &Expr) -> bool {
    let Some(args) = call_args(expr, &["ManaCost", "with_colors"]) else {
        return false;
    };
    args.len() == 2
        && check_int(args[0], 4)
        && array_items(args[1])
            .is_some_and(|items| items.len() == 1 && check_path(items[0], &["Color", "Blue"]))
}
fn check_zero_cost(expr: &Expr) -> bool {
    call_args(expr, &["ManaCost", "new"])
        .is_some_and(|args| args.len() == 1 && check_int(args[0], 0))
}
fn check_rules(expr: &Expr, expected: &[&str]) -> bool {
    let Expr::Reference(reference) = expr else {
        return false;
    };
    if !reference.attrs.is_empty() || reference.mutability.is_some() {
        return false;
    }
    let Some(items) = array_items(&reference.expr) else {
        return false;
    };
    items.len() == expected.len()
        && items
            .iter()
            .zip(expected)
            .all(|(item, expected)| check_string(item, expected))
}
fn check_some_number(expr: &Expr) -> bool {
    let Some(args) = call_args(expr, &["Some"]) else {
        return false;
    };
    args.len() == 1
        && matches!(args[0], Expr::Lit(value) if value.attrs.is_empty() && matches!(&value.lit, Lit::Int(_)))
}
fn macro_tokens(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Macro(mac) if mac.attrs.is_empty() && mac.mac.path.is_ident("vec") => {
            Some(mac.mac.tokens.to_string())
        }
        _ => None,
    }
}
fn check_empty_vec(expr: &Expr) -> bool {
    macro_tokens(expr).is_some_and(|tokens| tokens.is_empty())
}
fn check_keyword_vec(expr: &Expr) -> bool {
    macro_tokens(expr).is_some_and(|tokens| tokens.is_empty() || tokens == "Keyword :: Flying")
}
fn check_mill_effect_vec(expr: &Expr) -> bool {
    macro_tokens(expr).is_some_and(|tokens| tokens.is_empty() || tokens == "Effect :: MillSourceControllerFromSourceDamage" || tokens == "Effect :: MillSourceControllerFromSourceDamage , Effect :: MillSourceControllerFromSourceDamage")
}
