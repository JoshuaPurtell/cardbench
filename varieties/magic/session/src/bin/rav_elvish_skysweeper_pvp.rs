//! Full-game diagnostic authority for RAV Elvish Skysweeper.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbilityBinding, CardDefinition, CardType, Color, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, Step, Target, TargetRequirement, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use syn::parse::Parser;
use syn::{Expr, Item, Meta, ReturnType, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-ELVISH-SKYSWEEPER";
const ABILITY_ID: &str = "sacrifice-creature-destroy-flying";
const BIRDS_ID: &str = "RAV-BIRDS-OF-PARADISE";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const MAX_TRACE_SEED: u64 = 64;
const MAX_TURNS: u32 = 80;
const MAX_MOVES: u32 = 5_000;
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/elvish_skysweeper.rs";

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/elvish_skysweeper.rs"]
mod candidate_elvish_skysweeper;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Elvish Skysweeper result: {error}");
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
            eprintln!("Elvish Skysweeper PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongManaCost,
    NoSacrificeCost,
    BroadTarget,
    WrongEffect,
    WrongBody,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    source: Option<ObjectId>,
    source_cast_payment: Vec<Color>,
    source_power: Option<i16>,
    source_toughness: Option<i16>,
    activation_source: Option<ObjectId>,
    activation_cost: Option<ManaCost>,
    sacrificed: Option<ObjectId>,
    sacrificed_zone: Option<Zone>,
    target: Option<ObjectId>,
    target_definition: Option<&'static str>,
    target_owner: Option<PlayerId>,
    target_zone: Option<Zone>,
    destroyed_by: Option<ObjectId>,
    attack_turns: Vec<u32>,
    damage_to_opponent: i32,
    life: [i64; 2],
}

fn run_authority() -> Result<Value, String> {
    let source_validation = validate_candidate_source()?;
    validate_candidate()?;
    let catalog = unique_catalog()?;
    let (seed, capture) = find_trace()?;
    let trace = validate_trace(&capture, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if capture != replay || project_events(&capture.events) != project_events(&replay.events) {
        return Err("Elvish Skysweeper full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "wrong_activation_cost_mutation_detected": mutation_detected(seed, Mutation::WrongManaCost),
        "missing_sacrifice_cost_mutation_detected": mutation_detected(seed, Mutation::NoSacrificeCost),
        "broad_creature_target_mutation_detected": mutation_detected(seed, Mutation::BroadTarget),
        "wrong_destroy_effect_mutation_detected": mutation_detected(seed, Mutation::WrongEffect),
        "wrong_creature_body_mutation_detected": mutation_detected(seed, Mutation::WrongBody),
        "sealed_mutation_authority": false
    });
    for key in [
        "wrong_activation_cost_mutation_detected",
        "missing_sacrifice_cost_mutation_detected",
        "broad_creature_target_mutation_detected",
        "wrong_destroy_effect_mutation_detected",
        "wrong_creature_body_mutation_detected",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Elvish Skysweeper semantic mutation escaped: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 162, "printings": 1, "definition_id": CARD_ID, "name": "Elvish Skysweeper"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "302.1-302.2: creature characteristics",
                "602.1-602.2: activating abilities and paying costs",
                "701.7: destroying a permanent",
                "702.9: flying and target legality",
                "508 and 510: combat and damage"
            ],
            "source_contract": "RAV Elvish Skysweeper #162 is a green {G} 1/1 Creature with {4}{G}, Sacrifice a creature: destroy target creature with flying.",
            "authority_cutoff": "2026-09-24",
            "no_provider_trial": true
        },
        "source_validation": source_validation,
        "passed": true,
        "pilot_card_passed": true,
        "replay_deterministic": true,
        "cards_passed": 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "diagnostic_only; independent review and candidate trial pending",
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn definition_for(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_elvish_skysweeper::definition();
    if mutation == Mutation::WrongBody {
        definition.power = Some(2);
    }
    definition
}

fn ability_for(mutation: Mutation) -> ActivatedAbilityBinding {
    let mut binding = candidate_elvish_skysweeper::activated_ability();
    match mutation {
        Mutation::None | Mutation::WrongBody => {}
        Mutation::WrongManaCost => binding.ability.mana_cost = ManaCost::new(3),
        Mutation::NoSacrificeCost => binding.ability.sacrifice_creatures = 0,
        Mutation::BroadTarget => {
            binding.ability.targets = vec![TargetRequirement::NonblackCreature];
            binding.ability.effects = vec![Effect::DestroyTargetNonblackCreature];
        }
        Mutation::WrongEffect => binding.ability.effects = vec![Effect::TapTargetCreature],
    }
    binding
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation) {
        Ok(capture) => validate_trace(&capture, seed).is_err(),
        Err(_) => true,
    }
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_ELVISH_SKYSWEEPER_SEED") {
        let seed = value.parse::<u64>().map_err(|_| {
            "CARDBENCH_ELVISH_SKYSWEEPER_SEED must be an unsigned integer".to_owned()
        })?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    let mut last = String::new();
    for seed in 0..=MAX_TRACE_SEED {
        match run_match(seed, Mutation::None)
            .and_then(|capture| validate_trace(&capture, seed).map(|_| capture))
        {
            Ok(capture) => return Ok((seed, capture)),
            Err(error) => last = error,
        }
    }
    Err(format!(
        "no bounded seed produced a legal Skysweeper destruction and combat win: {last}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let p0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: BIRDS_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 52,
            },
        ],
        sideboard: vec![],
    };
    let p1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: BIRDS_ID.to_owned(),
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
    for deck in [&p0, &p1] {
        if deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
        {
            return Err("Elvish Skysweeper fixtures must use legal 60-card decks".to_owned());
        }
    }
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Elvish Skysweeper is missing from the central catalog")? = definition_for(mutation);
    let mut abilities = rav_activated_ability_bindings();
    let binding = abilities
        .iter_mut()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("Elvish Skysweeper activation is missing from the central registry")?;
    *binding = ability_for(mutation);
    let mut game = Game::new_with_all_bindings(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
    )
    .map_err(|error| error.to_string())?;
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_MOVES,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &p0)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &p1)
        .map_err(|error| error.to_string())?;
    for seat in [PlayerId(0), PlayerId(1)] {
        game.draw_opening_hand(seat, config.opening_hand_size)
            .map_err(|error| error.to_string())?;
    }
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(SkysweeperPolicy::new(PlayerId(0), mutation)),
        Box::new(SkysweeperPolicy::new(PlayerId(1), mutation)),
    ];
    let mut accepted = 0;
    let mut attempted = 0_u32;
    let mut source = None;
    let mut source_cast_payment = vec![];
    let mut activation_source = None;
    let mut activation_cost = None;
    let mut sacrificed = None;
    let mut target = None;
    let mut target_owner = None;
    let mut target_zone = None;
    let mut destroyed_by = None;
    let mut attack_turns = vec![];
    let mut damage_to_opponent = 0;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(format!(
                "Skysweeper match exceeded bound at turn {} after {accepted} moves",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Skysweeper policy seat missing")?;
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
        let activation = match &action {
            PolicyAction::ActivateAbility { activation } if activation.ability_id == ABILITY_ID => {
                Some(activation.clone())
            }
            _ => None,
        };
        let cursor = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Skysweeper move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("Skysweeper invariant failed after move: {error}"))?;
        if let Some(activation) = activation {
            source = Some(activation.source);
            activation_source = Some(activation.source);
            sacrificed = activation.sacrifice_sources.first().copied();
            target = activation.targets.first().and_then(|target| match target {
                Target::Permanent(object) => Some(*object),
                _ => None,
            });
            target_owner = target.and_then(|id| game.object(id).ok().map(|object| object.owner));
        }
        for event in &game.event_log[cursor..] {
            match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    if activation_source.is_none() || activation_source == Some(*card) {
                        source = Some(*card)
                    }
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if source == Some(*card)
                    || game
                        .card_definition(*card)
                        .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    source_cast_payment = colors.clone()
                }
                GameEvent::AbilityManaPaid {
                    player: PlayerId(0),
                    source: object,
                    ability,
                    mana_cost,
                } if *ability == ABILITY_ID && activation_source == Some(*object) => {
                    activation_cost = Some(mana_cost.clone())
                }
                GameEvent::CardDestroyed {
                    source: effect_source,
                    card,
                } if activation_source == Some(*effect_source) => destroyed_by = Some(*card),
                GameEvent::CardMoved {
                    card,
                    to: Zone::Graveyard,
                } if target == Some(*card) => target_zone = Some(Zone::Graveyard),
                GameEvent::AttackersDeclared {
                    player: PlayerId(0),
                    attackers,
                } if attackers.iter().any(|attacker| {
                    game.card_definition(*attacker)
                        .is_ok_and(|definition| definition.id == CARD_ID)
                }) =>
                {
                    attack_turns.push(game.turn)
                }
                GameEvent::DamageDealtToPlayer {
                    player: PlayerId(1),
                    amount,
                    ..
                } => damage_to_opponent += *amount,
                _ => {}
            }
        }
    }
    game.validate_invariants()
        .map_err(|error| format!("Skysweeper terminal invariant failed: {error}"))?;
    let winner = game.winner().map(|player| player.0);
    let sacrificed_zone = sacrificed.and_then(|id| game.zone_of(id));
    let life = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    let source_definition = source.and_then(|id| game.card_definition(id).ok());
    Ok(Captured {
        events: game.event_log.clone(),
        winner,
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        source,
        source_cast_payment,
        source_power: source_definition.and_then(|definition| definition.power),
        source_toughness: source_definition.and_then(|definition| definition.toughness),
        activation_source,
        activation_cost,
        sacrificed,
        sacrificed_zone,
        target,
        target_definition: target.and_then(|id| {
            game.card_definition(id)
                .ok()
                .map(|definition| definition.id)
        }),
        target_owner,
        target_zone,
        destroyed_by,
        attack_turns,
        damage_to_opponent,
        life,
    })
}

struct SkysweeperPolicy {
    player: PlayerId,
    mutation: Mutation,
    activated: bool,
    cast_bird: bool,
    cast_wolf: bool,
}
impl SkysweeperPolicy {
    fn new(player: PlayerId, mutation: Mutation) -> Self {
        Self {
            player,
            mutation,
            activated: false,
            cast_bird: false,
            cast_wolf: false,
        }
    }
}
impl CodePolicy for SkysweeperPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.elvish-skysweeper-pilot.v1"
        } else {
            "rav.elvish-skysweeper-opponent.v1"
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
            let attackers = if self.player == PlayerId(0) && self.activated {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(CARD_ID) && card.can_attack)
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
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.priority != self.player || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }
        if !view.active_player.eq(&self.player) || !view.step.is_main() {
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
        if self.player == PlayerId(0) && !self.activated {
            let source = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID));
            let fodder = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(BIRDS_ID));
            let flying = view
                .opponent_battlefield
                .iter()
                .find(|card| card.definition == Some(BIRDS_ID));
            let decoy = view
                .opponent_battlefield
                .iter()
                .find(|card| card.definition == Some(WATCHWOLF_ID));
            if let (Some(source), Some(fodder), Some(flying), Some(_decoy)) =
                (source, fodder, flying, decoy)
            {
                if view.mana_pool.total() < 5 || view.mana_pool.amount(Color::Green) == 0 {
                    if let Some(land) = view
                        .own_battlefield
                        .iter()
                        .find(|card| card.card_types.contains(&CardType::Land) && !card.tapped)
                    {
                        let color = if view.mana_pool.amount(Color::Green) == 0 {
                            Color::Green
                        } else {
                            Color::Green
                        };
                        return PolicyAction::ActivateManaAbility {
                            land: land.id,
                            color,
                        };
                    }
                } else {
                    self.activated = true;
                    let target = if self.mutation == Mutation::BroadTarget {
                        decoy.unwrap().id
                    } else {
                        flying.id
                    };
                    return PolicyAction::ActivateAbility {
                        activation: AbilityActivation {
                            source: source.id,
                            ability_id: ABILITY_ID,
                            sacrifice_sources: vec![fodder.id],
                            additional_tap_creatures: vec![],
                            discard_cards: vec![],
                            targets: vec![Target::Permanent(target)],
                        },
                    };
                }
            }
        }
        if view.lands_played == 0 {
            return PolicyAction::PassPriority;
        }
        let controlled_sweepers = view
            .own_battlefield
            .iter()
            .filter(|card| card.definition == Some(CARD_ID))
            .count();
        let should_cast_sweeper = controlled_sweepers == 0
            || (self.player == PlayerId(0) && self.activated && controlled_sweepers < 4);
        if self.player == PlayerId(0) && should_cast_sweeper {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            {
                let cost = ManaCost::with_colors(0, [Color::Green]);
                if let Some(selection) = payment(view, &cost) {
                    return cast(card.id, selection);
                }
                if let Some(action) = make_mana(view, Color::Green) {
                    return action;
                }
            }
        }
        if !self.cast_bird {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(BIRDS_ID))
            {
                if view.mana_pool.amount(Color::Green) > 0 {
                    self.cast_bird = true;
                    return cast(
                        card.id,
                        payment(view, &ManaCost::with_colors(0, [Color::Green])).unwrap(),
                    );
                }
                if let Some(action) = make_mana(view, Color::Green) {
                    return action;
                }
            }
        }
        if self.player == PlayerId(1) && !self.cast_wolf {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(WATCHWOLF_ID))
            {
                let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
                if let Some(selection) = payment(view, &cost) {
                    self.cast_wolf = true;
                    return cast(card.id, selection);
                }
                if let Some(action) =
                    make_mana(view, missing_color(view, &cost).unwrap_or(Color::Green))
                {
                    return action;
                }
            }
        }
        if view.mana_pool.total() < 5 {
            if let Some(action) = make_mana(view, Color::Green) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

fn cast(card: ObjectId, mana_selection: ManaPaymentSelection) -> PolicyAction {
    PolicyAction::CastWithPayment {
        request: cardbench_magic_engine::CastRequest {
            card,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        chosen_x: None,
        mana_selection,
    }
}
fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<_> = Color::MANA_ALL
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
    let mut generic = vec![];
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
fn missing_color(view: &GameView, cost: &ManaCost) -> Option<Color> {
    cost.colored
        .iter()
        .copied()
        .find(|color| view.mana_pool.amount(*color) == 0)
}
fn make_mana(view: &GameView, color: Color) -> Option<PolicyAction> {
    let land = view.own_battlefield.iter().find(|card| {
        card.card_types.contains(&CardType::Land)
            && !card.tapped
            && card.mana_colors.contains(&color)
    })?;
    Some(PolicyAction::ActivateManaAbility {
        land: land.id,
        color,
    })
}

fn validate_candidate() -> Result<(), String> {
    let definition = candidate_elvish_skysweeper::definition();
    if definition.id != CARD_ID
        || definition.name != "Elvish Skysweeper"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(0, [Color::Green])
        || definition.colors != BTreeSet::from([Color::Green])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.power != Some(1)
        || definition.toughness != Some(1)
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
    {
        return Err("Elvish Skysweeper candidate definition differs from its printed identity and {G} 1/1 body".to_owned());
    }
    let binding = candidate_elvish_skysweeper::activated_ability();
    if binding.card_definition != CARD_ID
        || binding.ability.id != ABILITY_ID
        || binding.ability.mana_cost != ManaCost::with_colors(4, [Color::Green])
        || binding.ability.tap_cost
        || binding.ability.sorcery_speed
        || binding.ability.additional_tap_creatures != 0
        || binding.ability.sacrifice_creatures != 1
        || binding.ability.sacrifice_source
        || binding.ability.sacrifice_lands != 0
        || binding.ability.discard_cards != 0
        || binding.ability.targets != vec![TargetRequirement::FlyingCreature]
        || binding.ability.effects != vec![Effect::DestroyTargetFlyingCreature]
    {
        return Err("Elvish Skysweeper candidate activation differs from {4}{G}, sacrifice a creature, destroy target creature with flying".to_owned());
    }
    let definitions = card_definitions();
    if definitions.iter().filter(|card| card.id == CARD_ID).count() != 1
        || definitions.iter().find(|card| card.id == CARD_ID) != Some(&definition)
    {
        return Err(
            "Elvish Skysweeper candidate definition is not uniquely delegated in the RAV registry"
                .to_owned(),
        );
    }
    let abilities = rav_activated_ability_bindings();
    let matches: Vec<_> = abilities
        .iter()
        .filter(|ability| ability.card_definition == CARD_ID && ability.ability.id == ABILITY_ID)
        .collect();
    if matches.len() != 1 || matches[0] != &binding {
        return Err(
            "Elvish Skysweeper candidate activation is not uniquely delegated in the RAV registry"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<Value, String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("cannot read candidate source: {error}"))?;
    validate_source_shape(&source)?;

    let signature = "pub(crate) fn definition() -> CardDefinition {";
    let injected = source.replacen(
        signature,
        "pub(crate) fn definition() -> CardDefinition {\n    let _ = std::fs::write(\"/tmp/cardbench-elvish-skysweeper-candidate-executed\", b\"ran\");",
        1,
    );
    if injected == source || syn::parse_file(&injected).is_err() {
        return Err(
            "compile-valid side-effect mutation could not be constructed as valid Rust".to_owned(),
        );
    }
    if validate_source_shape(&injected).is_ok() {
        return Err(
            "Elvish Skysweeper source guard accepted a side-effecting candidate mutation"
                .to_owned(),
        );
    }
    Ok(json!({
        "normalized_ast_schema_checked_before_candidate_calls": true,
        "side_effect_mutation_parse_valid": true,
        "side_effect_mutation_compile_valid_by_construction": true,
        "side_effect_mutation_rejected": true,
        "candidate_calls_before_guard": 0,
        "rejection_stage": "source-shape-and-declarative-AST-before-candidate-function-invocation"
    }))
}

fn validate_source_shape(source: &str) -> Result<(), String> {
    let file = syn::parse_file(source)
        .map_err(|_| "Elvish Skysweeper candidate source is not valid Rust")?;
    if !file.attrs.iter().all(|attribute| {
        attribute.path().is_ident("doc") && matches!(attribute.meta, Meta::NameValue(_))
    }) {
        return Err("Elvish Skysweeper module has an unsupported attribute".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "ActivatedAbility"],
        vec!["cardbench_magic_engine", "ActivatedAbilityBinding"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TargetRequirement"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                imports.extend(
                    flattened_use_paths(item).ok_or("unsupported Elvish Skysweeper import")?,
                );
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                let return_type = match name.as_str() {
                    "definition" => "CardDefinition",
                    "activated_ability" => "ActivatedAbilityBinding",
                    _ => {
                        return Err(
                            "Elvish Skysweeper module exposes an unsupported function".to_owned()
                        );
                    }
                };
                if !functions.insert(name.clone())
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || !function.sig.inputs.is_empty()
                    || !function.sig.generics.params.is_empty()
                    || function.sig.generics.where_clause.is_some()
                    || function.sig.variadic.is_some()
                    || function.block.stmts.len() != 1
                    || !matches!(&function.sig.output, ReturnType::Type(_, ty) if matches!(&**ty, syn::Type::Path(path) if path.qself.is_none() && path.path.is_ident(return_type)))
                    || !function.attrs.iter().all(|attribute| {
                        (attribute.path().is_ident("doc")
                            && matches!(attribute.meta, Meta::NameValue(_)))
                            || (attribute.path().is_ident("must_use")
                                && matches!(attribute.meta, Meta::Path(_)))
                    })
                {
                    return Err(
                        "Elvish Skysweeper function signature/attributes are outside the source contract"
                            .to_owned(),
                    );
                }
                let Stmt::Expr(Expr::Struct(literal), None) = &function.block.stmts[0] else {
                    return Err(
                        "Elvish Skysweeper factories must directly return struct literals"
                            .to_owned(),
                    );
                };
                if literal
                    .path
                    .segments
                    .last()
                    .is_none_or(|segment| segment.ident != return_type)
                    || literal.rest.is_some()
                    || !declarative_expr(&Expr::Struct(literal.clone()))
                {
                    return Err(
                        "Elvish Skysweeper candidate literal contains executable or unsupported code"
                            .to_owned(),
                    );
                }
                let expected_fields: BTreeSet<&str> = if name == "definition" {
                    [
                        "id",
                        "name",
                        "set_code",
                        "mana_cost",
                        "colors",
                        "mana_colors",
                        "card_types",
                        "is_basic_land",
                        "supported_rules",
                        "power",
                        "toughness",
                        "keywords",
                        "effects",
                    ]
                    .into_iter()
                    .collect()
                } else {
                    ["card_definition", "ability"].into_iter().collect()
                };
                let fields: BTreeSet<String> = literal
                    .fields
                    .iter()
                    .filter_map(|field| match &field.member {
                        syn::Member::Named(name) => Some(name.to_string()),
                        _ => None,
                    })
                    .collect();
                if fields.len() != literal.fields.len()
                    || fields.iter().map(String::as_str).collect::<BTreeSet<_>>() != expected_fields
                {
                    return Err(
                        "Elvish Skysweeper literal fields differ from the pinned schema".to_owned(),
                    );
                }
                if name == "activated_ability" && !nested_ability_literal_has_pinned_fields(literal)
                {
                    return Err(
                        "Elvish Skysweeper ability literal differs from the pinned schema"
                            .to_owned(),
                    );
                }
            }
            _ => {
                return Err(
                    "Elvish Skysweeper source permits only imports and two factory functions"
                        .to_owned(),
                );
            }
        }
    }
    if imports != expected_imports
        || functions != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()])
    {
        return Err(
            "Elvish Skysweeper import/function shape differs from the pinned schema".to_owned(),
        );
    }
    Ok(())
}

fn nested_ability_literal_has_pinned_fields(binding: &syn::ExprStruct) -> bool {
    let Some(field) = binding
        .fields
        .iter()
        .find(|field| matches!(&field.member, syn::Member::Named(name) if name == "ability"))
    else {
        return false;
    };
    let Expr::Struct(ability) = &field.expr else {
        return false;
    };
    let expected = BTreeSet::from([
        "id",
        "mana_cost",
        "tap_cost",
        "sorcery_speed",
        "additional_tap_creatures",
        "sacrifice_source",
        "sacrifice_creatures",
        "sacrifice_lands",
        "discard_cards",
        "targets",
        "effects",
    ]);
    let fields: BTreeSet<String> = ability
        .fields
        .iter()
        .filter_map(|field| match &field.member {
            syn::Member::Named(name) => Some(name.to_string()),
            _ => None,
        })
        .collect();
    ability
        .path
        .segments
        .last()
        .is_some_and(|segment| segment.ident == "ActivatedAbility")
        && ability.rest.is_none()
        && fields.len() == ability.fields.len()
        && fields.iter().map(String::as_str).collect::<BTreeSet<_>>() == expected
}

fn declarative_expr(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(_) | Expr::Path(_) => true,
        Expr::Reference(reference) => declarative_expr(&reference.expr),
        Expr::Array(array) => array.elems.iter().all(declarative_expr),
        Expr::Struct(value) => {
            matches!(
                value
                    .path
                    .segments
                    .last()
                    .map(|segment| segment.ident.to_string())
                    .as_deref(),
                Some("CardDefinition" | "ActivatedAbilityBinding" | "ActivatedAbility")
            ) && value.attrs.is_empty()
                && value.rest.is_none()
                && value.fields.iter().all(|field| {
                    matches!(field.member, syn::Member::Named(_)) && declarative_expr(&field.expr)
                })
        }
        Expr::Call(call) => {
            let allowed = match &*call.func {
                Expr::Path(path) if path.qself.is_none() => {
                    let names: Vec<_> = path
                        .path
                        .segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect();
                    matches!(names.as_slice(), [owner, method] if (owner == "BTreeSet" && method == "from") || (owner == "ManaCost" && method == "with_colors"))
                        || matches!(names.as_slice(), [name] if name == "Some")
                }
                _ => false,
            };
            allowed && call.args.iter().all(declarative_expr)
        }
        Expr::Macro(mac) => {
            let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
            mac.mac.path.is_ident("vec")
                && parser
                    .parse2(mac.mac.tokens.clone())
                    .is_ok_and(|values| values.iter().all(declarative_expr))
        }
        _ => false,
    }
}

fn flattened_use_paths(item: &syn::ItemUse) -> Option<Vec<Vec<String>>> {
    if item.leading_colon.is_some() {
        return None;
    }
    let mut paths = Vec::new();
    flatten_use_tree(&[], &item.tree, &mut paths).then_some(paths)
}

fn flatten_use_tree(prefix: &[String], tree: &UseTree, paths: &mut Vec<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix.to_vec();
            next.push(path.ident.to_string());
            flatten_use_tree(&next, &path.tree, paths)
        }
        UseTree::Name(name) => {
            let mut next = prefix.to_vec();
            next.push(name.ident.to_string());
            paths.push(next);
            true
        }
        UseTree::Group(group) => group
            .items
            .iter()
            .all(|item| flatten_use_tree(prefix, item, paths)),
        _ => false,
    }
}

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let unique: Vec<_> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if unique.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || unique.len() != 291
        || printings.len() != 306
    {
        return Err("RAV catalog denominator differs from the shared source contract".to_owned());
    }
    Ok(unique)
}

fn validate_trace(c: &Captured, seed: u64) -> Result<Value, String> {
    if c.winner != Some(0) || c.rejected != 0 {
        return Err("Skysweeper full-game match did not reach a clean seat-zero win".to_owned());
    }
    let source = c.source.ok_or("Elvish Skysweeper was not cast")?;
    let activation_source = c
        .activation_source
        .ok_or("Skysweeper's ability was not activated")?;
    let sacrificed = c
        .sacrificed
        .ok_or("the required creature was not sacrificed")?;
    let target = c.target.ok_or("no legal flying creature was targeted")?;
    if activation_source != source
        || c.source_cast_payment != [Color::Green]
        || c.source_power != Some(1)
        || c.source_toughness != Some(1)
        || c.activation_cost != Some(ManaCost::with_colors(4, [Color::Green]))
        || c.sacrificed_zone != Some(Zone::Graveyard)
        || sacrificed == source
        || c.target_definition != Some(BIRDS_ID)
        || c.target_owner != Some(PlayerId(1))
        || c.target_zone != Some(Zone::Graveyard)
        || c.destroyed_by != Some(target)
        || c.attack_turns.len() < 3
        || c.damage_to_opponent < 20
        || c.life[1] > 0
    {
        return Err(format!(
            "Skysweeper trace mismatch: source={:?} pay={:?}, activation={:?} cost={:?}, sac={:?} zone={:?}, target={:?} def={:?} owner={:?} zone={:?} destroyed={:?}, attacks={:?}, damage={}, life={:?}, winner={:?}",
            c.source,
            c.source_cast_payment,
            c.activation_source,
            c.activation_cost,
            c.sacrificed,
            c.sacrificed_zone,
            c.target,
            c.target_definition,
            c.target_owner,
            c.target_zone,
            c.destroyed_by,
            c.attack_turns,
            c.damage_to_opponent,
            c.life,
            c.winner
        ));
    }
    let projected = project_events(&c.events);
    let mut positions = Vec::new();
    for (name, pred) in [
        (
            "SpellManaPaid",
            Box::new(
                |event: &GameEvent| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == source && colors == &[Color::Green]),
            ) as Box<dyn Fn(&GameEvent) -> bool>,
        ),
        (
            "SpellCast",
            Box::new(
                |event: &GameEvent| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == source),
            ),
        ),
        (
            "SpellResolved",
            Box::new(
                |event: &GameEvent| matches!(event, GameEvent::SpellResolved { card } if *card == source),
            ),
        ),
        (
            "SkysweeperEnteredBattlefield",
            Box::new(
                |event: &GameEvent| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == source),
            ),
        ),
        (
            "AbilityManaPaid",
            Box::new(
                |event: &GameEvent| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source: object, ability: ABILITY_ID, mana_cost } if *object == source && *mana_cost == ManaCost::with_colors(4, [Color::Green])),
            ),
        ),
        (
            "SacrificedAsAbilityCost",
            Box::new(
                |event: &GameEvent| matches!(event, GameEvent::SacrificedAsAbilityCost { player: PlayerId(0), source: object, permanent } if *object == source && *permanent == sacrificed),
            ),
        ),
        (
            "SacrificeMovedToGraveyard",
            Box::new(
                |event: &GameEvent| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == sacrificed),
            ),
        ),
        (
            "AbilityActivated",
            Box::new(
                |event: &GameEvent| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source: object, ability: ABILITY_ID, .. } if *object == source),
            ),
        ),
        (
            "CardDestroyed",
            Box::new(
                |event: &GameEvent| matches!(event, GameEvent::CardDestroyed { source: object, card } if *object == source && *card == target),
            ),
        ),
        (
            "TargetMovedToGraveyard",
            Box::new(
                |event: &GameEvent| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == target),
            ),
        ),
        (
            "AbilityResolved",
            Box::new(
                |event: &GameEvent| matches!(event, GameEvent::AbilityResolved { source: object, ability: ABILITY_ID, .. } if *object == source),
            ),
        ),
        (
            "GameEnded",
            Box::new(|event: &GameEvent| {
                matches!(
                    event,
                    GameEvent::GameEnded {
                        winner: Some(PlayerId(0))
                    }
                )
            }),
        ),
    ] {
        let index = c
            .events
            .iter()
            .position(|event| pred(event))
            .ok_or_else(|| format!("Skysweeper trace is missing {name}"))?;
        positions.push(index);
    }
    if positions.windows(2).any(|window| window[0] >= window[1]) {
        return Err(
            "Skysweeper cast/activation/destruction/terminal receipts are out of order".to_owned(),
        );
    }
    Ok(json!({
        "trace_id": "rav-elvish-skysweeper-sacrifice-destroy-flyer-full-game",
        "shuffle_seed": seed,
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": 0,
        "turns": c.turns,
        "accepted_moves": c.accepted,
        "rejected_moves": c.rejected,
        "skysweeper": {"object": source.0, "mana_cost": "{G}", "mana_paid": ["Green"], "ability_id": ABILITY_ID, "ability_mana_cost": "{4}{G}", "sacrifice_cost": 1},
        "sacrifice": {"object": sacrificed.0, "definition": BIRDS_ID, "zone_after_cost": format!("{:?}", c.sacrificed_zone)},
        "target": {"object": target.0, "definition": c.target_definition, "owner": c.target_owner.map(|player| player.0), "zone_after_resolution": format!("{:?}", c.target_zone), "destroyed_by": c.destroyed_by.map(|id| id.0)},
        "combat": {"attack_turns": c.attack_turns, "damage_to_opponent": c.damage_to_opponent, "life_at_terminal": c.life},
        "ordered_event_indices": positions,
        "events": projected,
    }))
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    json!({"collector_number": card.collector_number, "name": card.name, "definition_id": definition_id, "trace_status": if definition_id == Some(CARD_ID) { "passed" } else { "not_run" }, "semantic_status": format!("{:?}", card.semantic_status)})
}
fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}
fn output_root() -> Option<std::path::PathBuf> {
    std::env::var_os("CARDBENCH_CARD_TASK_OUTPUT_ROOT").map(Into::into)
}
fn write_outputs(root: &std::path::Path, report: &Value) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(root)?;
    std::fs::write(root.join("result.json"), serde_json::to_vec_pretty(report)?)?;
    std::fs::write(
        root.join("trace.json"),
        serde_json::to_vec_pretty(&report["traces"][0])?,
    )?;
    Ok(())
}
