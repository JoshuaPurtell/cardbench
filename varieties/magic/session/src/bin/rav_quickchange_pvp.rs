//! Isolated full-game PvP authority for RAV Quickchange.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, Layer,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings, rav_basic_land_type_bindings,
    rav_land_entry_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_static_continuous_effect_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashSet};
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-QUICKCHANGE";
const TARGET_ID: &str = "RAV-ELVES-OF-DEEP-SHADOW";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/quickchange.rs";
const MAX_TURNS: u32 = 60;
const MAX_POLICY_MOVES: u32 = 8_000;
const REFERENCE_SEED: u64 = 0;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/quickchange.rs"]
mod candidate_quickchange;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Quickchange result: {error}");
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
            eprintln!("Quickchange PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoColorChange,
    NoDraw,
    WrongCost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    spell: Option<ObjectId>,
    target: Option<ObjectId>,
    target_original_colors: BTreeSet<Color>,
    target_colors_during_effect: Option<BTreeSet<Color>>,
    target_colors_after_expiry: Option<BTreeSet<Color>>,
    spell_definition_is_exact: bool,
    spell_colors_are_exact: bool,
    spell_cost_is_exact: bool,
    mana_paid: Vec<Color>,
    drawn_card: Option<ObjectId>,
    card_definition: Option<CardDefinition>,
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_binding()?;
    let catalog = unique_catalog()?;
    let reference = run_match(REFERENCE_SEED, Mutation::None)?;
    let trace = validate_trace(&reference)?;
    let replay = run_match(REFERENCE_SEED, Mutation::None)?;
    if reference != replay {
        return Err("Quickchange full-game replay was not deterministic".to_owned());
    }
    let no_color_change = mutation_detected(Mutation::NoColorChange)?;
    let no_draw = mutation_detected(Mutation::NoDraw)?;
    let wrong_cost = mutation_detected(Mutation::WrongCost)?;
    if !no_color_change || !no_draw || !wrong_cost {
        return Err("a Quickchange semantic mutation escaped the gameplay oracle".to_owned());
    }

    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 62, "definition_id": CARD_ID, "name": "Quickchange"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117: timing and priority",
                "601.2: instant spell casting and choices",
                "608.2: resolving spells and abilities",
                "613.1e: color-changing effects",
                "514: cleanup step and end-of-turn expiration",
                "121: card drawing"
            ],
            "source_contract": "RAV Quickchange is a blue {1}{U} Instant; choose one of the five colors, the target creature becomes only that color until end of turn, then Quickchange's controller draws one card",
            "authority_cutoff": "2026-09-24"
        },
        "passed": true,
        "pilot_card_passed": true,
        "deterministic_replay": true,
        "cards_passed": 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": rav_main_set_catalog().len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "unreviewed_diagnostic",
        "mutation_sensitivity": {
            "color_replacement_removed_mutation_detected_in_game": no_color_change,
            "draw_removed_mutation_detected_in_game": no_draw,
            "printed_cost_changed_mutation_detected_in_game": wrong_cost,
            "sealed_mutation_authority": false
        },
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn mutation_detected(mutation: Mutation) -> Result<bool, String> {
    match run_match(REFERENCE_SEED, mutation) {
        Err(_) => Ok(true),
        Ok(capture) => Ok(validate_trace(&capture).is_err()),
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definitions = card_definitions();
    let card = definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Quickchange definition is missing from the RAV registry")?;
    *card = candidate_quickchange::definition();
    match mutation {
        Mutation::None => {}
        Mutation::NoColorChange => {
            card.effects = vec![Effect::DrawController];
        }
        Mutation::NoDraw => {
            card.effects = vec![Effect::ReplaceTargetCreatureColorsWithChosenColorUntilEndOfTurn];
        }
        Mutation::WrongCost => card.mana_cost = ManaCost::with_colors(1, [Color::Green]),
    }

    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
        rav_static_continuous_effect_bindings(),
        rav_land_entry_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_attachment_bindings(rav_attachment_bindings())
        .map_err(|error| error.to_string())?;

    let p0_deck = DeckList {
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
                count: 24,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 12,
            },
            DeckEntry {
                card: "RAV-ISLAND".to_owned(),
                count: 16,
            },
        ],
        sideboard: vec![],
    };
    let p1_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: TARGET_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-SWAMP".to_owned(),
                count: 28,
            },
        ],
        sideboard: vec![],
    };
    for (name, deck) in [
        ("Quickchange task deck", &p0_deck),
        ("target task deck", &p1_deck),
    ] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{name} has {count} cards; expected 60"));
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_POLICY_MOVES,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &p0_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &p1_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(QuickchangePolicy::new(PlayerId(0))),
        Box::new(QuickchangePolicy::new(PlayerId(1))),
    ];
    let mut accepted_moves = 0_u32;
    let mut attempted_moves = 0_u32;
    let mut spell = None;
    let mut target = None;
    let mut target_original_colors = BTreeSet::new();
    let mut target_colors_during_effect = None;
    let mut target_colors_after_expiry = None;
    let mut mana_paid = Vec::new();
    let mut drawn_card = None;
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || accepted_moves >= MAX_POLICY_MOVES {
            return Err(format!(
                "Quickchange duel exceeded its bound at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("policy seat is unavailable")?;
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
        let targeted_quickchange = match &action {
            PolicyAction::CastWithColorChoice {
                request,
                color: Color::Red,
            } if player == PlayerId(0)
                && game
                    .card_definition(request.card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                request.targets.iter().find_map(|target| match target {
                    Target::Permanent(object) => Some((request.card, *object)),
                    _ => None,
                })
            }
            _ => None,
        };
        if let Some((candidate, target_object)) = targeted_quickchange {
            spell = Some(candidate);
            target = Some(target_object);
            target_original_colors = game
                .characteristics(target_object)
                .map_err(|error| error.to_string())?
                .colors;
        }
        let quickchange_mana_before = targeted_quickchange.is_some().then(|| {
            Color::MANA_ALL
                .into_iter()
                .map(|color| (color, view.mana_pool.amount(color)))
                .collect::<Vec<_>>()
        });
        let old_event_count = game.event_log.len();
        attempted_moves += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Quickchange policy action rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted_moves += 1;
        if let Some(before) = quickchange_mana_before {
            let after = &game
                .player(PlayerId(0))
                .map_err(|error| error.to_string())?
                .mana_pool;
            for (color, before_amount) in before {
                let spent = before_amount.saturating_sub(after.amount(color));
                for _ in 0..spent {
                    mana_paid.push(color);
                }
            }
        }
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        for event in &game.event_log[old_event_count..] {
            match event {
                GameEvent::ContinuousEffectCreated {
                    source,
                    target: affected,
                    layer: Layer::Color,
                } if Some(*source) == spell && Some(*affected) == target => {
                    target_colors_during_effect = Some(
                        game.characteristics(*affected)
                            .map_err(|error| error.to_string())?
                            .colors,
                    );
                }
                GameEvent::ContinuousEffectExpired {
                    source,
                    target: affected,
                    layer: Layer::Color,
                } if Some(*source) == spell && Some(*affected) == target => {
                    target_colors_after_expiry = Some(
                        game.characteristics(*affected)
                            .map_err(|error| error.to_string())?
                            .colors,
                    );
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Hand,
                } if spell.is_some() && drawn_card.is_none() && Some(*card) != spell => {
                    drawn_card = Some(*card);
                }
                _ => {}
            }
        }
    }

    let quickchange = spell;
    let card_definition = quickchange.and_then(|card| game.card_definition(card).ok().cloned());
    let spell_definition_is_exact = card_definition.as_ref().is_some_and(|definition| {
        definition.id == CARD_ID
            && definition.name == "Quickchange"
            && definition.card_types == BTreeSet::from([CardType::Instant])
    });
    let spell_colors_are_exact = card_definition
        .as_ref()
        .is_some_and(|definition| definition.colors == BTreeSet::from([Color::Blue]));
    let spell_cost_is_exact = card_definition
        .as_ref()
        .is_some_and(|definition| definition.mana_cost == ManaCost::with_colors(1, [Color::Blue]));
    Ok(CapturedMatch {
        events: game.event_log.clone(),
        terminal: game.winner().map_or_else(
            || "Draw".to_owned(),
            |winner| format!("Winner(PlayerId({}))", winner.0),
        ),
        winner: game.winner().map(|winner| winner.0),
        turns: game.turn,
        accepted_moves,
        rejected_moves: attempted_moves.saturating_sub(accepted_moves),
        spell: quickchange,
        target,
        target_original_colors,
        target_colors_during_effect,
        target_colors_after_expiry,
        spell_definition_is_exact,
        spell_colors_are_exact,
        spell_cost_is_exact,
        mana_paid,
        drawn_card,
        card_definition,
    })
}

struct QuickchangePolicy {
    player: PlayerId,
    spell_cast: bool,
    target_creature_cast: bool,
    opponent_target_cast: bool,
}

impl QuickchangePolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            spell_cast: false,
            target_creature_cast: false,
            opponent_target_cast: false,
        }
    }

    fn total_mana(view: &GameView) -> u16 {
        Color::MANA_ALL
            .into_iter()
            .map(|color| u16::from(view.mana_pool.amount(color)))
            .sum()
    }

    fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let mut available: Vec<_> = Color::MANA_ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        for color in &cost.colored {
            let (_, amount) = available
                .iter_mut()
                .find(|(candidate, _)| candidate == color)?;
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

    fn mana_source(view: &GameView, preferred: Option<Color>) -> Option<PolicyAction> {
        let land = view
            .own_battlefield
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land)
                    && !card.tapped
                    && preferred.is_none_or(|color| card.mana_colors.contains(&color))
            })
            .or_else(|| {
                view.own_battlefield
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land) && !card.tapped)
            })?;
        let color = preferred
            .filter(|color| land.mana_colors.contains(color))
            .or_else(|| land.mana_colors.iter().next().copied())?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }

    fn cast_with_payment(
        &mut self,
        view: &GameView,
        definition: &'static str,
        cost: ManaCost,
    ) -> Option<PolicyAction> {
        if let Some(mana_selection) = Self::payment(view, &cost) {
            let card = view
                .hand
                .iter()
                .find(|card| card.definition == Some(definition))?;
            if definition == WATCHWOLF_ID {
                self.target_creature_cast = true;
            }
            if definition == TARGET_ID {
                self.opponent_target_cast = true;
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
        let missing = cost
            .colored
            .iter()
            .find(|color| view.mana_pool.amount(**color) == 0)
            .copied()
            .or_else(|| {
                (Self::total_mana(view) < u16::from(cost.generic) + cost.colored.len() as u16)
                    .then_some(Color::Green)
            });
        Self::mana_source(view, missing)
    }

    fn cast_quickchange(&mut self, view: &GameView, target: ObjectId) -> Option<PolicyAction> {
        let cost = ManaCost::with_colors(1, [Color::Blue]);
        if Self::payment(view, &cost).is_some() {
            let spell = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))?;
            self.spell_cast = true;
            Some(PolicyAction::CastWithColorChoice {
                request: cardbench_magic_engine::CastRequest {
                    card: spell.id,
                    targets: vec![Target::Permanent(target)],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                color: Color::Red,
            })
        } else {
            let missing = if view.mana_pool.amount(Color::Blue) == 0 {
                Some(Color::Blue)
            } else if Self::total_mana(view) < 2 {
                Some(Color::Green)
            } else {
                None
            };
            Self::mana_source(view, missing)
        }
    }
}

impl CodePolicy for QuickchangePolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.quickchange.color-replacement-draw.v1"
        } else {
            "rav.quickchange.green-target-pilot.v1"
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
            let attackers = view
                .own_battlefield
                .iter()
                .filter(|card| card.can_attack && card.card_types.contains(&CardType::Creature))
                .map(|card| card.id)
                .collect();
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
        if view.lands_played == 0 {
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if self.player == PlayerId(0) {
            if !self.target_creature_cast
                && !view
                    .own_battlefield
                    .iter()
                    .any(|card| card.definition == Some(WATCHWOLF_ID))
                && view
                    .hand
                    .iter()
                    .any(|card| card.definition == Some(WATCHWOLF_ID))
                && let Some(action) = self.cast_with_payment(
                    view,
                    WATCHWOLF_ID,
                    ManaCost::with_colors(0, [Color::Green, Color::White]),
                )
            {
                return action;
            }
            if !self.spell_cast
                && view
                    .hand
                    .iter()
                    .any(|card| card.definition == Some(CARD_ID))
                && let Some(target) = view.own_battlefield.iter().find(|card| {
                    card.definition == Some(WATCHWOLF_ID)
                        && card.card_types.contains(&CardType::Creature)
                })
                && let Some(action) = self.cast_quickchange(view, target.id)
            {
                return action;
            }
        } else if !self.opponent_target_cast
            && !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(TARGET_ID))
            && view
                .hand
                .iter()
                .any(|card| card.definition == Some(TARGET_ID))
            && let Some(action) =
                self.cast_with_payment(view, TARGET_ID, ManaCost::with_colors(0, [Color::Green]))
        {
            return action;
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(capture: &CapturedMatch) -> Result<Value, String> {
    if capture.terminal != "Winner(PlayerId(0))"
        || capture.winner != Some(0)
        || capture.rejected_moves != 0
    {
        return Err("Quickchange duel did not end in a clean P0 win".to_owned());
    }
    if !capture.spell_definition_is_exact
        || !capture.spell_colors_are_exact
        || !capture.spell_cost_is_exact
    {
        return Err(
            "Quickchange identity, blue color, Instant type, or exact {1}{U} cost differs"
                .to_owned(),
        );
    }
    let spell = capture.spell.ok_or("Quickchange was not cast")?;
    let target = capture
        .target
        .ok_or("Quickchange creature target was not captured")?;
    if capture.mana_paid.len() != 2 || !capture.mana_paid.contains(&Color::Blue) {
        return Err(
            "Quickchange did not spend exactly one blue and one generic mana from its pool"
                .to_owned(),
        );
    }
    if capture.target_original_colors != BTreeSet::from([Color::Green, Color::White])
        || capture.target_colors_during_effect != Some(BTreeSet::from([Color::Red]))
        || capture.target_colors_after_expiry != Some(BTreeSet::from([Color::Green, Color::White]))
    {
        return Err(format!(
            "Quickchange did not replace only the exact target's color until end of turn (before {:?}, during {:?}, after {:?})",
            capture.target_original_colors,
            capture.target_colors_during_effect,
            capture.target_colors_after_expiry
        ));
    }
    let drawn = capture
        .drawn_card
        .ok_or("Quickchange did not move a card to the caster's hand")?;
    let choice = event_index(&capture.events, |event| matches!(event, GameEvent::SpellColorChosen { player: PlayerId(0), card, color: Color::Red } if *card == spell)).ok_or("Quickchange policy-submitted Red choice receipt missing")?;
    let cast = event_index(&capture.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == spell)).ok_or("Quickchange SpellCast receipt missing")?;
    let effect = event_index_after(&capture.events, cast, |event| matches!(event, GameEvent::ContinuousEffectCreated { source, target: affected, layer: Layer::Color } if *source == spell && *affected == target)).ok_or("Quickchange did not create a layer-five color effect on its exact creature target")?;
    let draw = event_index_after(
        &capture.events,
        effect,
        |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == drawn),
    )
    .ok_or("Quickchange's exact drawn card movement receipt missing")?;
    let resolved = event_index_after(
        &capture.events,
        cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == spell),
    )
    .ok_or("Quickchange did not resolve")?;
    let graveyard = event_index_after(&capture.events, resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == spell)).ok_or("Quickchange did not move to graveyard after resolution")?;
    let expiry = event_index_after(&capture.events, graveyard, |event| matches!(event, GameEvent::ContinuousEffectExpired { source, target: affected, layer: Layer::Color } if *source == spell && *affected == target)).ok_or("Quickchange color effect did not expire at end of turn")?;
    let attack = event_index_after(&capture.events, expiry, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&target))).ok_or("the affected creature did not participate in a later attack")?;
    let combat = event_index_after(&capture.events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount } if *source == target && *amount > 0)).ok_or("the affected creature did not deal combat damage after its color reverted")?;
    let end = event_index(&capture.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal P0 win receipt missing")?;
    if !(choice < cast
        && cast < effect
        && effect < draw
        && draw < resolved
        && resolved < graveyard
        && graveyard < expiry
        && expiry < attack
        && attack < combat
        && combat < end)
    {
        return Err("Quickchange payment, choice, effect, draw, expiration, combat, and terminal receipts are out of order".to_owned());
    }
    let indices = vec![
        choice, cast, effect, draw, resolved, graveyard, expiry, attack, combat, end,
    ];
    let mut indices = indices;
    indices.sort_unstable();
    Ok(json!({
        "id":"rav-quickchange-chosen-color-draw-full-game",
        "seed": REFERENCE_SEED,
        "terminal": capture.terminal,
        "winner_seat": capture.winner,
        "turns": capture.turns,
        "accepted_moves": capture.accepted_moves,
        "rejected_moves": capture.rejected_moves,
        "quickchange_object": spell.0,
        "target_object": target.0,
        "target_definition": WATCHWOLF_ID,
        "target_original_colors": capture.target_original_colors.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
        "chosen_color": "Red",
        "mana_paid": capture.mana_paid.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
        "target_colors_during_effect": capture.target_colors_during_effect.as_ref().unwrap().iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
        "target_colors_after_expiry": capture.target_colors_after_expiry.as_ref().unwrap().iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
        "drawn_card_object": drawn.0,
        "target_attacked_after_expiration": true,
        "target_combat_damage_after_expiration": match capture.events.get(combat) { Some(GameEvent::DamageDealtToPlayer { amount, .. }) => *amount, _ => 0 },
        "events_digest": event_digest(&capture.events),
        "events": project_selected_events(&capture.events, &indices),
        "passed": true
    }))
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: "Quickchange",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "policy-chosen-target-color-replacement-until-end-of-turn",
            "controller-draw",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![
            Effect::ReplaceTargetCreatureColorsWithChosenColorUntilEndOfTurn,
            Effect::DrawController,
        ],
    }
}

fn validate_candidate_binding() -> Result<(), String> {
    let definition = candidate_quickchange::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Quickchange is missing from the RAV catalog")?;
    if definition != expected_definition() || definition != registered {
        return Err("Quickchange literal is outside the task source contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Quickchange source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Quickchange source is not declarative Rust".to_owned())?;
    let mut imports = BTreeSet::new();
    let mut definitions = 0;
    let expected_imports = BTreeSet::from(
        [
            vec!["cardbench_magic_engine", "CardDefinition"],
            vec!["cardbench_magic_engine", "CardType"],
            vec!["cardbench_magic_engine", "Color"],
            vec!["cardbench_magic_engine", "Effect"],
            vec!["cardbench_magic_engine", "ManaCost"],
            vec!["std", "collections", "BTreeSet"],
        ]
        .map(|path| path.into_iter().map(str::to_owned).collect::<Vec<_>>()),
    );
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                let mut paths = Vec::new();
                if item.leading_colon.is_some() || !flatten_use_tree(&[], &item.tree, &mut paths) {
                    return Err("Quickchange source has unsupported imports".to_owned());
                }
                imports.extend(paths);
            }
            Item::Fn(function)
                if function.attrs.iter().all(|attribute| {
                    attribute.path().is_ident("doc") || attribute.path().is_ident("must_use")
                }) && matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    && function.sig.ident == "definition"
                    && function.sig.inputs.is_empty()
                    && function.sig.generics.params.is_empty()
                    && function.sig.generics.where_clause.is_none()
                    && function.sig.constness.is_none()
                    && function.sig.asyncness.is_none()
                    && function.sig.unsafety.is_none()
                    && function.sig.abi.is_none()
                    && function.sig.variadic.is_none()
                    && function.block.stmts.len() == 1 =>
            {
                let direct = matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(value), None) if value.attrs.is_empty() && value.rest.is_none() && path_is_type_path(&value.path, "CardDefinition"));
                if !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if type_is(ty, "CardDefinition"))
                    || !direct
                {
                    return Err(
                        "Quickchange definition must return a direct typed literal".to_owned()
                    );
                }
                definitions += 1;
            }
            _ => return Err("Quickchange source contains an unsupported top-level item".to_owned()),
        }
    }
    if imports != expected_imports || definitions != 1 || file.items.len() != 3 {
        return Err(
            "Quickchange module structure differs from the candidate source contract".to_owned(),
        );
    }
    Ok(())
}

fn flatten_use_tree(prefix: &[String], tree: &UseTree, paths: &mut Vec<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(item) => {
            let mut next = prefix.to_vec();
            next.push(item.ident.to_string());
            flatten_use_tree(&next, &item.tree, paths)
        }
        UseTree::Name(item) => {
            let mut path = prefix.to_vec();
            path.push(item.ident.to_string());
            paths.push(path);
            true
        }
        UseTree::Group(items) => items
            .items
            .iter()
            .all(|item| flatten_use_tree(prefix, item, paths)),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}

fn type_is(ty: &syn::Type, name: &str) -> bool {
    matches!(ty, syn::Type::Path(path) if path.qself.is_none() && path_is_type_path(&path.path, name))
}

fn path_is_type_path(path: &syn::Path, name: &str) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == 1
        && path.segments[0].ident == name
        && matches!(path.segments[0].arguments, syn::PathArguments::None)
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
        return Err("RAV catalog identity or printing denominator differs".to_owned());
    }
    let definitions = card_definitions();
    let ids: HashSet<_> = definitions.iter().map(|definition| definition.id).collect();
    for card in &catalog {
        if let CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } =
            card.semantic_status
        {
            if !ids.contains(definition_id) {
                return Err(format!(
                    "executable RAV identity {definition_id} is missing its definition"
                ));
            }
        }
    }
    Ok(catalog)
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({
        "collector_number": card.collector_number,
        "name": card.name,
        "definition_id": definition_id,
        "engine_status": if definition_id.is_some() { "executable_compatibility_slice" } else { "catalog_only" },
        "credited": false,
        "qualification": if passed { "unreviewed_diagnostic" } else { "not_evaluated" },
        "trace_status": if passed { "passed" } else { "not_qualified" }
    })
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}

fn event_index_after(
    events: &[GameEvent],
    start: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, event)| predicate(event).then_some(index))
}

fn project_selected_events(events: &[GameEvent], indices: &[usize]) -> Vec<ProjectedEvent> {
    project_events(events)
        .into_iter()
        .enumerate()
        .filter_map(|(index, event)| indices.contains(&index).then_some(event))
        .collect()
}

fn event_digest(events: &[GameEvent]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in format!("{events:?}").bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}

fn output_root() -> Option<std::path::PathBuf> {
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--output-root" {
            return arguments.next().map(std::path::PathBuf::from);
        }
    }
    None
}

fn write_outputs(root: &std::path::Path, report: &Value) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(root.join("reward.txt"), "0.00000000\n").map_err(|error| error.to_string())
}
