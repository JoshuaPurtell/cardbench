//! Full-game source-relative authority for RAV Shadow of Doubt.

use cardbench_magic_engine::{
    CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, HybridManaSymbol,
    ManaCost, ManaPaymentSelection, PlayerId, PolicyAction, Step, Zone,
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

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-SHADOW-OF-DOUBT";
const CARD_NAME: &str = "Shadow of Doubt";
const FARSEEK_ID: &str = "RAV-FARSEEK";
const FINISHER_ID: &str = "RAV-SKYKNIGHT-LEGIONNAIRE";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/shadow_of_doubt.rs";
const MAX_SEED: u64 = 96;
const MAX_TURNS: u32 = 80;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/shadow_of_doubt.rs"]
mod candidate_shadow_of_doubt;

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
            eprintln!("Shadow of Doubt PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_definition()?;
    let catalog = unique_catalog()?;
    let seed = find_reference_seed()?;
    let reference = run_match(seed, Mutation::None)?;
    validate_trace(&reference)?;
    let replay = run_match(seed, Mutation::None)?;
    if reference != replay {
        return Err("Shadow of Doubt full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "omitted_search_prevention_detected_in_game": mutation_detected(seed, Mutation::OmitPrevention)?,
        "omitted_draw_detected_in_game": mutation_detected(seed, Mutation::OmitDraw)?,
        "duplicate_draw_detected_in_game": mutation_detected(seed, Mutation::DuplicateDraw)?,
        "reversed_effect_order_detected_in_game": mutation_detected(seed, Mutation::ReverseOrder)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "omitted_search_prevention_detected_in_game",
        "omitted_draw_detected_in_game",
        "duplicate_draw_detected_in_game",
        "reversed_effect_order_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "a Shadow of Doubt semantic mutation escaped: {key}"
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
        "card": {"collector_number": 253, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "authority_cutoff": "2026-09-24",
            "source_contract": "RAV #253 is an instant with two {U/B} hybrid symbols. It prevents all players from searching libraries for the rest of the current turn, then its controller draws a card.",
            "checked_sections": ["601 Casting spells", "608 Resolving spells and abilities", "701.19 Searching a library", "121 Drawing a card", "400.2 Public and hidden zones"]
        },
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": rav_main_set_catalog().len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "unreviewed_diagnostic",
        "deterministic_replay": true,
        "mutation_sensitivity": mutations,
        "traces": [trace_json(&reference)],
        "per_card_breakdown": breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Shadow of Doubt source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Shadow of Doubt source is not valid Rust".to_owned())?;
    let definitions = file
        .items
        .iter()
        .filter(
            |item| matches!(item, syn::Item::Fn(function) if function.sig.ident == "definition"),
        )
        .count();
    let only_supported_items = file
        .items
        .iter()
        .all(|item| matches!(item, syn::Item::Use(_) | syn::Item::Fn(_)));
    if definitions != 1
        || !only_supported_items
        || !source.contains("RAV-SHADOW-OF-DOUBT")
        || !source.contains("PreventLibrarySearchUntilEndOfTurn")
        || !source.contains("DrawController")
    {
        return Err("Shadow of Doubt candidate source differs from its module contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_definition() -> Result<(), String> {
    let definition = candidate_shadow_of_doubt::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Shadow of Doubt definition is missing from the RAV registry")?;
    let cost = ManaCost::with_hybrid(
        0,
        [],
        [
            HybridManaSymbol {
                first: Color::Blue,
                second: Color::Black,
            },
            HybridManaSymbol {
                first: Color::Blue,
                second: Color::Black,
            },
        ],
    );
    if definition != registered
        || definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != cost
        || definition.colors != BTreeSet::from([Color::Blue, Color::Black])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Instant])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
        || definition.effects != expected_effects(Mutation::None)
    {
        return Err("Shadow of Doubt definition is outside its literal source contract".to_owned());
    }
    Ok(())
}

fn expected_effects(mutation: Mutation) -> Vec<Effect> {
    match mutation {
        Mutation::None => vec![
            Effect::PreventLibrarySearchUntilEndOfTurn,
            Effect::DrawController,
        ],
        Mutation::OmitPrevention => vec![Effect::DrawController],
        Mutation::OmitDraw => vec![Effect::PreventLibrarySearchUntilEndOfTurn],
        Mutation::DuplicateDraw => vec![
            Effect::PreventLibrarySearchUntilEndOfTurn,
            Effect::DrawController,
            Effect::DrawController,
        ],
        Mutation::ReverseOrder => vec![
            Effect::DrawController,
            Effect::PreventLibrarySearchUntilEndOfTurn,
        ],
    }
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
            .any(|card| card.collector_number == 253 && card.name == CARD_NAME)
    {
        return Err("RAV catalog denominator or Shadow of Doubt identity differs".to_owned());
    }
    Ok(catalog)
}

fn find_reference_seed() -> Result<u64, String> {
    for seed in 0..MAX_SEED {
        if let Ok(capture) = run_match(seed, Mutation::None)
            && validate_trace(&capture).is_ok()
        {
            return Ok(seed);
        }
    }
    Err("no bounded shuffled seed completed the Shadow of Doubt PvP scenario".to_owned())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    OmitPrevention,
    OmitDraw,
    DuplicateDraw,
    ReverseOrder,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SearchReceipt {
    source: cardbench_magic_engine::ObjectId,
    found: Option<cardbench_magic_engine::ObjectId>,
    turn: u32,
    index: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    shadow: Option<cardbench_magic_engine::ObjectId>,
    shadow_turn: Option<u32>,
    shadow_paid: Vec<Color>,
    prevention_index: Option<usize>,
    prevention_turn: Option<u32>,
    shadow_resolved_index: Option<usize>,
    shadow_draw_indices: Vec<usize>,
    farseeks: Vec<cardbench_magic_engine::ObjectId>,
    farseek_cast_turns: Vec<u32>,
    searches: Vec<SearchReceipt>,
    shuffles_after_search: Vec<bool>,
    options: Vec<String>,
    selected: Option<cardbench_magic_engine::ObjectId>,
    selected_definition: Option<String>,
    private_to_controller: bool,
    hidden_from_opponent: bool,
    selected_mountain: bool,
    selected_tapped_at_entry: bool,
    winner_damage: bool,
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definitions = card_definitions();
    let registry_card = definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Shadow of Doubt definition missing from registry")?;
    let mut changed_definition = candidate_shadow_of_doubt::definition();
    changed_definition.effects = expected_effects(mutation);
    *registry_card = changed_definition;

    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_bindings(),
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

    let deck0 = DeckList {
        mainboard: vec![
            entry(CARD_ID, 4),
            entry(FINISHER_ID, 4),
            entry("RAV-ISLAND", 13),
            entry("RAV-SWAMP", 13),
            entry("RAV-PLAINS", 13),
            entry("RAV-MOUNTAIN", 13),
        ],
        sideboard: vec![],
    };
    let deck1 = DeckList {
        mainboard: vec![
            entry(FARSEEK_ID, 4),
            entry("RAV-FOREST", 28),
            entry("RAV-MOUNTAIN", 28),
        ],
        sideboard: vec![],
    };
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: 8000,
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
        Box::new(ShadowPolicy::new(PlayerId(0))),
        Box::new(ShadowPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut event_turns = vec![game.turn; game.event_log.len()];
    let mut options = Vec::new();
    let mut selected = None;
    let mut selected_definition = None;
    let mut private_to_controller = false;
    let mut hidden_from_opponent = false;
    let mut selected_mountain = false;
    let mut selected_tapped_at_entry = false;
    let mut selected_seen_on_battlefield = false;

    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(format!(
                "Shadow of Doubt duel exceeded bounds at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let action = if let Some(choice) = view.library_search_choice.as_ref() {
            let opponent_view = game
                .view_for_player(PlayerId(0))
                .map_err(|error| error.to_string())?;
            private_to_controller = player == PlayerId(1)
                && view.pending_decision.as_ref().is_some_and(|decision| {
                    decision.visibility == cardbench_magic_engine::DecisionVisibility::Private
                });
            hidden_from_opponent = opponent_view.library_search_choice.is_none()
                && opponent_view
                    .pending_decision
                    .as_ref()
                    .is_none_or(|decision| {
                        decision.visibility != cardbench_magic_engine::DecisionVisibility::Private
                    });
            options = choice
                .cards
                .iter()
                .filter_map(|card| card.definition.map(str::to_owned))
                .collect();
            let mountain = choice
                .cards
                .iter()
                .find(|card| card.definition == Some("RAV-MOUNTAIN"));
            selected = mountain.map(|card| card.id);
            selected_definition = mountain.and_then(|card| card.definition.map(str::to_owned));
            PolicyAction::ChooseLibrarySearchCard {
                decision: choice.decision,
                source: choice.source,
                selected,
            }
        } else if let Some(action) = policies[player.0].propose_pending_decision(&view) {
            action
        } else if view.optional_triggered_ability_choice.is_some() {
            policies[player.0].propose_optional_triggered_ability(&view)
        } else if view.draw_replacement_pending {
            policies[player.0].propose_draw_replacement(&view)
        } else if view.private_library_choice.is_some() {
            policies[player.0].propose_private_library_choice(&view)
        } else if view.private_opponent_library_choice.is_some() {
            policies[player.0].propose_private_opponent_library_choice(&view)
        } else {
            policies[player.0].propose_move(&view)
        };

        attempted += 1;
        game.submit_policy_move(player, policies[player.0].id(), action)
            .map_err(|error| {
                format!(
                    "Shadow policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        while event_turns.len() < game.event_log.len() {
            event_turns.push(game.turn);
        }
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        if !selected_seen_on_battlefield
            && let Some(card) = selected
            && game.zone_of(card) == Some(Zone::Battlefield)
        {
            selected_seen_on_battlefield = true;
            selected_mountain = game.basic_land_type(card).ok().flatten()
                == Some(cardbench_magic_engine::BasicLandType::Mountain);
            selected_tapped_at_entry = game.object(card).map_err(|error| error.to_string())?.tapped;
        }
    }

    let mut shadow = None;
    let mut shadow_turn = None;
    let mut shadow_paid = Vec::new();
    let mut farseeks = Vec::new();
    let mut farseek_cast_turns = Vec::new();
    let mut searches = Vec::new();
    let mut shuffles_after_search = Vec::new();
    let mut prevention_index = None;
    let mut prevention_turn = None;
    let mut shadow_resolved_index = None;
    let mut shadow_draw_indices = Vec::new();
    let mut winner_damage = false;
    for (index, event) in game.event_log.iter().enumerate() {
        match event {
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                shadow = Some(*card);
                shadow_turn = event_turns.get(index).copied();
            }
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                shadow_paid = colors.clone()
            }
            GameEvent::SpellCast {
                player: PlayerId(1),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == FARSEEK_ID) =>
            {
                farseeks.push(*card);
                farseek_cast_turns.push(event_turns.get(index).copied().unwrap_or_default());
            }
            GameEvent::LibrarySearchesPrevented { source, until_turn }
                if Some(*source) == shadow =>
            {
                prevention_index = Some(index);
                prevention_turn = Some(*until_turn);
            }
            GameEvent::SpellResolved { card } if Some(*card) == shadow => {
                shadow_resolved_index = Some(index);
            }
            GameEvent::CardMoved {
                card: _,
                to: Zone::Hand,
            } if prevention_index.is_some() && shadow_resolved_index.is_none() => {
                shadow_draw_indices.push(index)
            }
            GameEvent::LibrarySearchResolved {
                player: PlayerId(1),
                source,
                found,
                ..
            } if farseeks.contains(source) => searches.push(SearchReceipt {
                source: *source,
                found: *found,
                turn: event_turns.get(index).copied().unwrap_or_default(),
                index,
            }),
            GameEvent::DamageDealtToPlayer {
                player: PlayerId(1),
                ..
            } => winner_damage = true,
            _ => {}
        }
    }
    for (search_index, search) in searches.iter().enumerate() {
        let next_search = searches
            .get(search_index + 1)
            .map_or(game.event_log.len(), |next| next.index);
        let shuffled = game
            .event_log
            .iter()
            .enumerate()
            .take(next_search)
            .skip(search.index + 1)
            .any(|(_, event)| {
                matches!(
                    event,
                    GameEvent::LibraryShuffled {
                        player: PlayerId(1),
                        ..
                    }
                )
            });
        shuffles_after_search.push(shuffled);
    }
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
        shadow,
        shadow_turn,
        shadow_paid,
        prevention_index,
        prevention_turn,
        shadow_resolved_index,
        shadow_draw_indices,
        farseeks,
        farseek_cast_turns,
        searches,
        shuffles_after_search,
        options,
        selected,
        selected_definition,
        private_to_controller,
        hidden_from_opponent,
        selected_mountain,
        selected_tapped_at_entry,
        winner_damage,
    })
}

fn entry(card: &str, count: u8) -> DeckEntry {
    DeckEntry {
        card: card.to_owned(),
        count,
    }
}

fn rav_triggered_bindings() -> Vec<cardbench_magic_engine::TriggeredAbilityBinding> {
    let mut bindings = rav_triggered_ability_bindings();
    bindings.extend(attachment_triggers());
    bindings
}

struct ShadowPolicy {
    player: PlayerId,
    shadow_cast: bool,
    farseek_casts: u8,
    last_farseek_turn: u32,
}

impl ShadowPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            shadow_cast: false,
            farseek_casts: 0,
            last_farseek_turn: 0,
        }
    }

    fn untapped_land(
        &self,
        view: &GameView,
        color: Color,
    ) -> Option<cardbench_magic_engine::ObjectId> {
        view.own_battlefield
            .iter()
            .find(|card| {
                !card.tapped
                    && card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&color)
            })
            .map(|card| card.id)
    }

    fn cast_action(
        &self,
        card: cardbench_magic_engine::ObjectId,
        generic: Vec<Color>,
        hybrid: Vec<Color>,
    ) -> PolicyAction {
        PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection { generic, hybrid },
        }
    }

    fn propose_shadow_response(&mut self, view: &GameView) -> Option<PolicyAction> {
        let farseek_on_stack = view
            .stack_spells
            .iter()
            .any(|card| card.definition == Some(FARSEEK_ID));
        if self.player != PlayerId(0)
            || self.shadow_cast
            || !farseek_on_stack
            || !view
                .hand
                .iter()
                .any(|card| card.definition == Some(CARD_ID))
        {
            return None;
        }
        let available = [Color::Blue, Color::Black]
            .iter()
            .map(|color| view.mana_pool.amount(*color))
            .sum::<u8>();
        if available >= 2 {
            let mut choices = Vec::new();
            for color in [Color::Blue, Color::Black] {
                for _ in 0..view.mana_pool.amount(color).min(2 - choices.len() as u8) {
                    choices.push(color);
                }
            }
            if choices.len() == 2 {
                let card = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))?;
                self.shadow_cast = true;
                return Some(self.cast_action(card.id, vec![], choices));
            }
        }
        let missing = [Color::Blue, Color::Black]
            .into_iter()
            .find(|color| {
                view.mana_pool.amount(*color) == 0 && self.untapped_land(view, *color).is_some()
            })
            .or_else(|| {
                [Color::Blue, Color::Black]
                    .into_iter()
                    .find(|color| self.untapped_land(view, *color).is_some())
            })?;
        self.untapped_land(view, missing)
            .map(|land| PolicyAction::ActivateManaAbility {
                land,
                color: missing,
            })
    }

    fn skyknight_payment(&self, view: &GameView) -> Option<Color> {
        if view.mana_pool.amount(Color::Red) == 0 || view.mana_pool.amount(Color::White) == 0 {
            return None;
        }
        [
            Color::Blue,
            Color::Black,
            Color::Green,
            Color::Red,
            Color::White,
        ]
        .into_iter()
        .find(|color| {
            let required = u8::from(*color == Color::Red) + u8::from(*color == Color::White);
            view.mana_pool.amount(*color) > required
        })
    }

    fn farseek_generic_payment(&self, view: &GameView) -> Option<Color> {
        if view.mana_pool.amount(Color::Green) > 1 {
            return Some(Color::Green);
        }
        [Color::White, Color::Blue, Color::Black, Color::Red]
            .into_iter()
            .find(|color| view.mana_pool.amount(*color) > 0)
    }
}

impl CodePolicy for ShadowPolicy {
    fn id(&self) -> &'static str {
        "rav.shadow-of-doubt-full-game.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.propose_shadow_response(view) {
            return action;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(FINISHER_ID) && card.can_attack)
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
        if view.decision_player != self.player
            || view.priority != self.player
            || view.active_player != self.player
        {
            return PolicyAction::PassPriority;
        }
        if view.step != Step::PrecombatMain || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }

        if view.lands_played == 0 {
            let preferred: &[&str] = if self.player == PlayerId(1) {
                &["RAV-FOREST"]
            } else if view
                .own_battlefield
                .iter()
                .all(|card| card.definition != Some("RAV-ISLAND"))
            {
                &["RAV-ISLAND", "RAV-SWAMP", "RAV-PLAINS", "RAV-MOUNTAIN"]
            } else if view
                .own_battlefield
                .iter()
                .all(|card| card.definition != Some("RAV-SWAMP"))
            {
                &["RAV-SWAMP", "RAV-ISLAND", "RAV-PLAINS", "RAV-MOUNTAIN"]
            } else {
                &["RAV-MOUNTAIN", "RAV-PLAINS", "RAV-ISLAND", "RAV-SWAMP"]
            };
            if let Some(land) = preferred
                .iter()
                .find_map(|id| view.hand.iter().find(|card| card.definition == Some(*id)))
                .or_else(|| {
                    view.hand
                        .iter()
                        .find(|card| card.card_types.contains(&CardType::Land))
                })
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }

        if self.player == PlayerId(1)
            && self.farseek_casts < 2
            && self.last_farseek_turn != view.turn
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(FARSEEK_ID))
        {
            if let Some(generic) = self.farseek_generic_payment(view) {
                self.farseek_casts += 1;
                self.last_farseek_turn = view.turn;
                return self.cast_action(card.id, vec![generic], vec![]);
            }
            let needed = if view.mana_pool.amount(Color::Green) == 0 {
                Some(Color::Green)
            } else {
                [Color::White, Color::Blue, Color::Black, Color::Red]
                    .into_iter()
                    .find(|color| self.untapped_land(view, *color).is_some())
                    .or(Some(Color::Green))
            };
            if let Some(color) = needed
                && let Some(land) = self.untapped_land(view, color)
            {
                return PolicyAction::ActivateManaAbility { land, color };
            }
        }

        if self.player == PlayerId(0)
            && self.shadow_cast
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(FINISHER_ID))
        {
            if let Some(generic) = self.skyknight_payment(view) {
                return self.cast_action(card.id, vec![generic], vec![]);
            }
            for color in [Color::Red, Color::White] {
                if view.mana_pool.amount(color) == 0
                    && let Some(land) = self.untapped_land(view, color)
                {
                    return PolicyAction::ActivateManaAbility { land, color };
                }
            }
            let current = view.mana_pool.amount(Color::Blue)
                + view.mana_pool.amount(Color::Black)
                + view.mana_pool.amount(Color::Green)
                + view.mana_pool.amount(Color::Red)
                + view.mana_pool.amount(Color::White);
            if current < 3
                && let Some(land) = view
                    .own_battlefield
                    .iter()
                    .find(|card| !card.tapped && card.card_types.contains(&CardType::Land))
                && let Some(color) = land
                    .mana_colors
                    .iter()
                    .copied()
                    .find(|color| *color != Color::Colorless)
            {
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color,
                };
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(capture: &CapturedMatch) -> Result<(), String> {
    let shadow = capture.shadow.ok_or("Shadow of Doubt was not cast")?;
    let prevention = capture
        .prevention_index
        .ok_or("Shadow did not prevent library searches")?;
    let shadow_resolved = capture
        .shadow_resolved_index
        .ok_or("Shadow did not resolve")?;
    if capture.winner != Some(0)
        || capture.terminal != "Winner(PlayerId(0))"
        || capture.rejected_moves != 0
    {
        return Err(
            "Shadow of Doubt scenario did not finish as a clean seat-zero full-game win".to_owned(),
        );
    }
    if capture.shadow_paid.len() != 2
        || capture
            .shadow_paid
            .iter()
            .any(|color| !matches!(color, Color::Blue | Color::Black))
        || capture.shadow_turn.is_none()
        || capture.prevention_turn != capture.shadow_turn
    {
        return Err("Shadow was not paid with two hybrid colors or its prevention turn differed from its cast turn".to_owned());
    }
    if shadow_resolved <= prevention
        || capture.shadow_draw_indices.len() != 1
        || capture.shadow_draw_indices[0] <= prevention
        || capture.shadow_draw_indices[0] >= shadow_resolved
    {
        return Err("Shadow must prevent searches, draw exactly one card, and resolve those effects in order".to_owned());
    }
    if capture.farseeks.len() < 2 || capture.searches.len() < 2 {
        return Err(
            "the PvP trace must cast Farseek once while prevented and again after expiration"
                .to_owned(),
        );
    }
    let opened_search_decisions = capture
        .events
        .iter()
        .filter(|event| {
            matches!(
                event,
                GameEvent::DecisionOpened {
                    player: PlayerId(1),
                    kind: cardbench_magic_engine::DecisionKind::LibrarySearch,
                    visibility: cardbench_magic_engine::DecisionVisibility::Private,
                    ..
                }
            )
        })
        .count();
    let completed_search_decisions = capture
        .events
        .iter()
        .filter(|event| {
            matches!(
                event,
                GameEvent::DecisionCompleted {
                    player: PlayerId(1),
                    kind: cardbench_magic_engine::DecisionKind::LibrarySearch,
                    ..
                }
            )
        })
        .count();
    if opened_search_decisions != 1 || completed_search_decisions != 1 {
        return Err("the prevented search must open no choice, while the later search opens and completes one private choice".to_owned());
    }
    let first_search = &capture.searches[0];
    let second_search = &capture.searches[1];
    if first_search.source != capture.farseeks[0]
        || first_search.found.is_some()
        || first_search.turn != capture.prevention_turn.unwrap_or_default()
        || first_search.index <= shadow_resolved
        || second_search.source != capture.farseeks[1]
        || second_search.found != capture.selected
        || second_search.found.is_none()
        || second_search.turn <= capture.prevention_turn.unwrap_or_default()
        || !capture
            .shuffles_after_search
            .first()
            .copied()
            .unwrap_or(false)
        || !capture
            .shuffles_after_search
            .get(1)
            .copied()
            .unwrap_or(false)
    {
        return Err("Farseek did not fail to find during the current-turn prevention, then search and shuffle after expiration".to_owned());
    }
    if capture.options.is_empty()
        || !capture.options.iter().any(|id| id == "RAV-MOUNTAIN")
        || capture.selected_definition.as_deref() != Some("RAV-MOUNTAIN")
        || !capture.selected_mountain
        || !capture.selected_tapped_at_entry
        || !capture.private_to_controller
        || !capture.hidden_from_opponent
    {
        return Err(
            "the later Farseek search did not privately select and tap a legal Mountain".to_owned(),
        );
    }
    if !capture.winner_damage {
        return Err(
            "Shadow of Doubt duel did not reach combat damage before its terminal win".to_owned(),
        );
    }
    let _ = shadow;
    Ok(())
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Ok(capture) => Ok(validate_trace(&capture).is_err()),
        Err(_) => Ok(true),
    }
}

fn trace_json(capture: &CapturedMatch) -> Value {
    let projected = project_events(&capture.events);
    let shadow = capture.shadow;
    let first_farseek = capture.farseeks.first().copied();
    let second_farseek = capture.farseeks.get(1).copied();
    let relevant: Vec<usize> = capture
        .events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::SpellManaPaid { card, .. }
            | GameEvent::SpellCast { card, .. }
            | GameEvent::SpellResolved { card }
                if Some(*card) == shadow
                    || Some(*card) == first_farseek
                    || Some(*card) == second_farseek =>
            {
                Some(index)
            }
            GameEvent::LibrarySearchesPrevented { source, .. } if Some(*source) == shadow => {
                Some(index)
            }
            GameEvent::LibrarySearchResolved { source, .. }
                if Some(*source) == first_farseek || Some(*source) == second_farseek =>
            {
                Some(index)
            }
            GameEvent::LibraryShuffled {
                player: PlayerId(1),
                ..
            } => Some(index),
            GameEvent::CardMoved { to: Zone::Hand, .. }
                if capture.prevention_index.is_some_and(|start| index > start)
                    && capture.shadow_resolved_index.is_some_and(|end| index < end) =>
            {
                Some(index)
            }
            GameEvent::DecisionOpened {
                player: PlayerId(1),
                kind: cardbench_magic_engine::DecisionKind::LibrarySearch,
                ..
            }
            | GameEvent::DecisionCompleted {
                player: PlayerId(1),
                kind: cardbench_magic_engine::DecisionKind::LibrarySearch,
                ..
            } => Some(index),
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } if !attackers.is_empty() => Some(index),
            GameEvent::DamageDealtToPlayer {
                player: PlayerId(1),
                ..
            }
            | GameEvent::GameEnded { .. } => Some(index),
            _ => None,
        })
        .collect();
    let events: Vec<Value> = relevant.into_iter().filter_map(|index| {
        let event: &ProjectedEvent = projected.get(index)?;
        Some(json!({"sequence": event.sequence, "turn": event.turn, "kind": event.kind, "facts": event.facts, "canonical": event.canonical}))
    }).collect();
    json!({
        "trace_id": "rav-shadow-of-doubt-same-turn-search-prevention-and-expiry",
        "terminal": capture.terminal,
        "winner": capture.winner,
        "turns": capture.turns,
        "accepted_policy_moves": capture.accepted_moves,
        "rejected_policy_moves": capture.rejected_moves,
        "mechanics": {
            "mana_cost": "{U/B}{U/B}",
            "paid_mana_colors": capture.shadow_paid.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "shadow_cast_turn": capture.shadow_turn,
            "prevention_until_turn": capture.prevention_turn,
            "farseek_cast_count": capture.farseeks.len(),
            "farseek_cast_turns": capture.farseek_cast_turns,
            "prevented_search_found": capture.searches.first().and_then(|search| search.found).map(|object| object.0),
            "prevented_search_same_turn": capture.searches.first().is_some_and(|search| Some(search.turn) == capture.prevention_turn),
            "shadow_draw_count": capture.shadow_draw_indices.len(),
            "draw_after_prevention": capture.prevention_index.zip(capture.shadow_draw_indices.first().copied()).is_some_and(|(prevent, draw)| draw > prevent),
            "draw_before_shadow_resolution": capture.shadow_draw_indices.first().zip(capture.shadow_resolved_index).is_some_and(|(draw, resolve)| *draw < resolve),
            "search_after_expiry_turn": capture.searches.get(1).map(|search| search.turn),
            "later_search_found": capture.searches.get(1).and_then(|search| search.found).is_some(),
            "later_selected_definition": capture.selected_definition,
            "later_selected_mountain": capture.selected_mountain,
            "later_selected_tapped_at_entry": capture.selected_tapped_at_entry,
            "later_selection_private_to_controller": capture.private_to_controller,
            "later_selection_hidden_from_opponent": capture.hidden_from_opponent,
            "later_candidate_definitions": capture.options,
            "forest_excluded_from_later_candidates": !capture.options.iter().any(|id| id == "RAV-FOREST"),
            "shuffle_after_each_search": capture.shuffles_after_search,
            "combat_to_terminal_win": capture.winner == Some(0) && capture.winner_damage
        },
        "events": events
    })
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
    let passed = card.collector_number == 253;
    json!({
        "collector_number": card.collector_number,
        "name": card.name,
        "definition_id": if passed { definition_id } else { None },
        "trace_status": if passed { "passed" } else { "not_run" },
        "passed": passed,
        "credited": false,
        "reward": 0.0
    })
}
