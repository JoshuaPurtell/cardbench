//! Full-game diagnostic authority for RAV Duskmantle, House of Shadow.

use cardbench_magic_engine::{
    AbilityActivation, CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target,
    TargetRequirement, Zone,
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
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::parse::Parser;
use syn::{Expr, Item, Lit, Meta, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-DUSKMANTLE-HOUSE-OF-SHADOW";
const CARD_NAME: &str = "Duskmantle, House of Shadow";
const ABILITY_ID: &str = "tap-target-player-mill-one";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/duskmantle_house_of_shadow.rs";
const MAX_TRACE_SEED: u64 = 128;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/duskmantle_house_of_shadow.rs"]
mod candidate_duskmantle;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Duskmantle result: {error}");
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
            eprintln!("Duskmantle PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_registry_delegation()?;
    let catalog = unique_catalog()?;
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if !same_match(&captured, &replay)
        || project_events(&captured.events) != project_events(&replay.events)
    {
        return Err("Duskmantle full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "two_card_mill_mutation_detected_in_game": mutation_detected(seed, Mutation::MillTwo)?,
        "untapped_activation_mutation_detected_in_game": mutation_detected(seed, Mutation::NoTapCost)?,
        "untargeted_self_mill_mutation_detected_in_game": mutation_detected(seed, Mutation::MillSelf)?,
        "player_target_requirement_mutation_detected_in_game": mutation_detected(seed, Mutation::NoPlayerTarget)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "two_card_mill_mutation_detected_in_game",
        "untapped_activation_mutation_detected_in_game",
        "untargeted_self_mill_mutation_detected_in_game",
        "player_target_requirement_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!("Duskmantle semantic mutation escaped: {key}"));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,
        "expansion":"RAV",
        "card":{"collector_number":277,"printings":1,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":[
                "305.1-305.2: playing a land during a main phase with priority and an empty stack",
                "117.1b and 117.3c: priority and stack resolution",
                "602.1-602.2: activating an ability and paying its costs",
                "701.13: moving the targeted player's top library card to their graveyard",
                "400.7: zone changes create new game objects",
                "508 and 510: combat and damage"
            ],
            "source_contract":"RAV Duskmantle, House of Shadow #277 is a colorless land with {T}: target player mills one card",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true
        },
        "passed":true,
        "pilot_card_passed":true,
        "replay_deterministic":true,
        "cards_passed":cards_passed,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":rav_main_set_catalog().len(),
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":"diagnostic_only; independent review and candidate trial pending",
        "mutation_sensitivity":mutations,
        "traces":[trace],
        "per_card_breakdown":breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    MillTwo,
    NoTapCost,
    MillSelf,
    NoPlayerTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    source: Option<ObjectId>,
    target: Option<Target>,
    source_tapped_at_activation: bool,
    source_zone_at_resolution: Option<Zone>,
    mill_object: Option<ObjectId>,
    mill_owner: Option<PlayerId>,
    library_before: Option<usize>,
    library_after: Option<usize>,
    watchwolf: Option<ObjectId>,
    attacks: Vec<u32>,
    combat_damage: i32,
    life: [i64; 2],
}

fn same_match(left: &Captured, right: &Captured) -> bool {
    left.events == right.events
        && left.terminal == right.terminal
        && left.winner == right.winner
        && left.turns == right.turns
        && left.source == right.source
        && left.target == right.target
        && left.source_tapped_at_activation == right.source_tapped_at_activation
        && left.source_zone_at_resolution == right.source_zone_at_resolution
        && left.mill_object == right.mill_object
        && left.mill_owner == right.mill_owner
        && left.library_before == right.library_before
        && left.library_after == right.library_after
        && left.watchwolf == right.watchwolf
        && left.attacks == right.attacks
        && left.combat_damage == right.combat_damage
        && left.life == right.life
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_DUSKMANTLE_HOUSE_OF_SHADOW_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_DUSKMANTLE_HOUSE_OF_SHADOW_SEED must be an unsigned integer")?;
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    let mut last_error = String::new();
    for seed in 0..=MAX_TRACE_SEED {
        match run_match(seed, Mutation::None)
            .and_then(|captured| validate_trace(&captured, seed).map(|_| captured))
        {
            Ok(captured) => return Ok((seed, captured)),
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "no Duskmantle full-game one-card mill trace found: {last_error}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    Ok(match run_match(seed, mutation) {
        Ok(captured) => validate_trace(&captured, seed).is_err(),
        Err(_) => true,
    })
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
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
        mainboard: vec![DeckEntry {
            card: "RAV-PLAINS".to_owned(),
            count: 60,
        }],
        sideboard: vec![],
    };
    if [&pilot, &opponent].iter().any(|deck| {
        deck.mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
    }) {
        return Err("Duskmantle fixtures must use two legal shuffled 60-card decks".to_owned());
    }
    let definition = candidate_duskmantle::definition();
    let mut binding = candidate_duskmantle::activated_ability();
    match mutation {
        Mutation::None => {}
        Mutation::MillTwo => binding.ability.effects = vec![Effect::MillTargetPlayer { count: 2 }],
        Mutation::NoTapCost => binding.ability.tap_cost = false,
        Mutation::MillSelf => {
            binding.ability.effects = vec![Effect::MillCapturedPlayer {
                player: PlayerId(0),
                count: 1,
            }]
        }
        Mutation::NoPlayerTarget => binding.ability.targets.clear(),
    }
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|known| known.id == CARD_ID)
        .ok_or("Duskmantle definition is absent from the RAV registry")? = definition.clone();
    let mut abilities = rav_activated_ability_bindings();
    let registered = abilities
        .iter_mut()
        .find(|known| known.card_definition == CARD_ID && known.ability.id == ABILITY_ID)
        .ok_or("Duskmantle target-mill ability is absent from the RAV registry")?;
    *registered = binding.clone();
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
        max_turns: 40,
        max_policy_moves: 5000,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(DuskmantlePolicy::new(PlayerId(0))),
        Box::new(PassPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut source = None;
    let mut target = None;
    let mut source_tapped_at_activation = false;
    let mut library_before = None;
    let mut library_after = None;
    let mut mill_object = None;
    let mut mill_owner = None;
    let mut source_zone_at_resolution = None;
    let mut watchwolf = None;
    let mut attacks = Vec::new();
    let mut combat_damage = 0_i32;
    let p1_initial_library: BTreeSet<ObjectId> = game
        .player(PlayerId(1))
        .map_err(|e| e.to_string())?
        .library
        .iter()
        .copied()
        .collect();

    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(format!(
                "Duskmantle match exceeded its bound at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Duskmantle policy seat missing")?;
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
                    "Duskmantle policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("Duskmantle post-move invariant failed: {error}"))?;
        let new_events = &game.event_log[cursor..];
        if let Some(activation) = activation {
            source = Some(activation.source);
            target = activation.targets.first().copied();
            source_tapped_at_activation = game
                .object(activation.source)
                .is_ok_and(|object| object.tapped);
            library_before = Some(
                game.player(PlayerId(1))
                    .map_err(|e| e.to_string())?
                    .library
                    .len(),
            );
        }
        for event in new_events {
            match event {
                GameEvent::AbilityResolved {
                    source: resolved_source,
                    ability,
                    ..
                } if *ability == ABILITY_ID && source == Some(*resolved_source) => {
                    source_zone_at_resolution = game.zone_of(*resolved_source);
                    library_after = Some(
                        game.player(PlayerId(1))
                            .map_err(|e| e.to_string())?
                            .library
                            .len(),
                    );
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Graveyard,
                } if p1_initial_library.contains(card)
                    && game
                        .object(*card)
                        .is_ok_and(|object| object.owner == PlayerId(1)) =>
                {
                    if mill_object.is_none() {
                        mill_object = Some(*card);
                        mill_owner = Some(PlayerId(1));
                    }
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == WATCHWOLF_ID) =>
                {
                    watchwolf = Some(*card)
                }
                GameEvent::AttackersDeclared {
                    player: PlayerId(0),
                    attackers,
                } if watchwolf.is_some_and(|wolf| attackers.contains(&wolf)) => {
                    attacks.push(game.turn)
                }
                GameEvent::DamageDealtToPlayer {
                    source,
                    player: PlayerId(1),
                    amount,
                } if watchwolf == Some(*source) => combat_damage += *amount,
                _ => {}
            }
        }
    }
    game.validate_invariants()
        .map_err(|e| format!("Duskmantle terminal invariant failed: {e}"))?;
    let life = [
        game.player(PlayerId(0)).map_err(|e| e.to_string())?.life,
        game.player(PlayerId(1)).map_err(|e| e.to_string())?.life,
    ];
    let winner = game.winner().map(|player| player.0);
    Ok(Captured {
        events: game.event_log.clone(),
        terminal: winner.map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({seat}))"),
        ),
        winner,
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        source,
        target,
        source_tapped_at_activation,
        source_zone_at_resolution,
        mill_object,
        mill_owner,
        library_before,
        library_after,
        watchwolf,
        attacks,
        combat_damage,
        life,
    })
}

struct DuskmantlePolicy {
    player: PlayerId,
    activation_offered: bool,
    wolf_cast_offered: bool,
}
impl DuskmantlePolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            activation_offered: false,
            wolf_cast_offered: false,
        }
    }
}

impl CodePolicy for DuskmantlePolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.duskmantle-one-card-mill.v1"
        } else {
            "rav.duskmantle-pass-opponent.v1"
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
        if view.priority != self.player
            || view.active_player != self.player
            || view.stack_depth != 0
            || !view.step.is_main()
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            let land = if self.player == PlayerId(0) {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
                    .or_else(|| {
                        view.hand
                            .iter()
                            .find(|card| card.card_types.contains(&CardType::Land))
                    })
            } else {
                view.hand
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land))
            };
            if let Some(land) = land {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if self.player != PlayerId(0) {
            return PolicyAction::PassPriority;
        }
        if !self.activation_offered {
            if let Some(source) = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID) && !card.tapped)
            {
                self.activation_offered = true;
                return PolicyAction::ActivateAbility {
                    activation: AbilityActivation {
                        source: source.id,
                        ability_id: ABILITY_ID,
                        sacrifice_sources: vec![],
                        additional_tap_creatures: vec![],
                        discard_cards: vec![],
                        targets: vec![Target::Player(PlayerId(1))],
                    },
                };
            }
        }
        if !self.wolf_cast_offered
            && !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(WATCHWOLF_ID))
        {
            if let Some(wolf) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(WATCHWOLF_ID))
            {
                let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
                if let Some(selection) = payment(view, &cost) {
                    self.wolf_cast_offered = true;
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: wolf.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: selection,
                    };
                }
                if let Some(action) = make_mana(view, &cost) {
                    return action;
                }
            }
        }
        PolicyAction::PassPriority
    }
}

struct PassPolicy {
    player: PlayerId,
}
impl PassPolicy {
    fn new(player: PlayerId) -> Self {
        Self { player }
    }
}
impl CodePolicy for PassPolicy {
    fn id(&self) -> &'static str {
        "rav.duskmantle-pass-opponent.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
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
        PolicyAction::PassPriority
    }
}

fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<_> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let (_, amount) = available
            .iter_mut()
            .find(|(available_color, _)| available_color == color)?;
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

fn make_mana(view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
    let color = cost
        .colored
        .iter()
        .copied()
        .find(|color| view.mana_pool.amount(*color) == 0)
        .or_else(|| {
            (view.mana_pool.total() < cost.generic.saturating_add(cost.colored.len() as u8))
                .then(|| cost.colored.first().copied())
                .flatten()
        })?;
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

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))"
        || captured.winner != Some(0)
        || captured.rejected != 0
    {
        return Err("Duskmantle match did not reach a clean seat-zero full-game win".to_owned());
    }
    let source = captured.source.ok_or("Duskmantle was not activated")?;
    let target = captured
        .target
        .ok_or("Duskmantle activation had no player target")?;
    let milled = captured
        .mill_object
        .ok_or("Duskmantle moved no exact opposing library object to the graveyard")?;
    if target != Target::Player(PlayerId(1)) || captured.mill_owner != Some(PlayerId(1)) {
        return Err(
            "Duskmantle must target opponent seat one and mill that owner's library".to_owned(),
        );
    }
    if !captured.source_tapped_at_activation
        || captured.source_zone_at_resolution != Some(Zone::Battlefield)
    {
        return Err(
            "Duskmantle source did not pay its tap cost and remain on the battlefield".to_owned(),
        );
    }
    if !captured
        .library_before
        .zip(captured.library_after)
        .is_some_and(|(before, after)| before.checked_sub(1) == Some(after))
    {
        return Err(format!(
            "Duskmantle must reduce the target library by exactly one: {:?}->{:?}",
            captured.library_before, captured.library_after
        ));
    }
    if captured.attacks.len() < 3
        || captured.combat_damage < 9
        || captured.life[0] <= 0
        || captured.life[1] > 0
    {
        return Err(
            "Duskmantle trace lacks repeated Watchwolf combat or the opponent did not lose"
                .to_owned(),
        );
    }
    let activated = event_index(&captured.events, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source: known, definition, ability, .. } if *known == source && *definition == CARD_ID && *ability == ABILITY_ID)).ok_or("Duskmantle AbilityActivated receipt is missing")?;
    let moved = event_index_after(&captured.events, activated, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == milled)).ok_or("exact target library CardMoved receipt is missing")?;
    let resolved = event_index_after(&captured.events, moved, |event| matches!(event, GameEvent::AbilityResolved { source: known, ability, .. } if *known == source && *ability == ABILITY_ID)).ok_or("Duskmantle AbilityResolved receipt is missing")?;
    let wolf = captured
        .watchwolf
        .ok_or("Watchwolf finisher did not resolve")?;
    let attack = event_index_after(&captured.events, resolved, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&wolf))).ok_or("Watchwolf did not attack after the mill")?;
    let damage = event_index_after(&captured.events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount } if *source == wolf && *amount > 0)).ok_or("Watchwolf dealt no combat damage after the mill")?;
    let ended = event_index_after(&captured.events, damage, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("seat-zero terminal event is missing")?;
    let projected = project_events(&captured.events);
    Ok(json!({
        "trace_id":"rav-duskmantle-targeted-one-card-mill-full-game",
        "shuffle_seed":seed,
        "terminal":captured.terminal,
        "winner_seat":0,
        "turns":captured.turns,
        "rejected_moves":captured.rejected,
        "activation":{"source_object":source.0,"ability_id":ABILITY_ID,"tap_cost_paid":true,"source_zone_after_resolution":"Battlefield","target_player":1,"library_size_before":captured.library_before,"library_size_after":captured.library_after,"milled_object":milled.0,"milled_object_owner":1},
        "combat":{"card":"Watchwolf","object":wolf.0,"attack_turns":captured.attacks,"damage_to_opponent":captured.combat_damage,"life_at_terminal":captured.life},
        "ordered_event_sequence":[activated,moved,resolved,attack,damage,ended],
        "events":projected
    }))
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
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
        return Err("RAV catalog identity or 291/306 denominator changed".to_owned());
    }
    Ok(catalog)
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 277;
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"engine_status":if definition_id.is_some(){"executable_compatibility_slice"}else{"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"qualification":if passed{"diagnostic_only"}else{"not_evaluated"},"credited":false})
}

fn validate_registry_delegation() -> Result<(), String> {
    let candidate = candidate_duskmantle::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Duskmantle is absent from the RAV definition registry")?;
    if registered != candidate {
        return Err("Duskmantle registry does not delegate to its candidate module".to_owned());
    }
    let binding = candidate_duskmantle::activated_ability();
    if !rav_activated_ability_bindings()
        .iter()
        .any(|registered| registered == &binding)
    {
        return Err(
            "Duskmantle activation registry does not delegate to its candidate module".to_owned(),
        );
    }
    if candidate.id != CARD_ID
        || candidate.name != CARD_NAME
        || candidate.set_code != "RAV"
        || candidate.mana_cost != ManaCost::new(0)
        || !candidate.colors.is_empty()
        || !candidate.mana_colors.is_empty()
        || candidate.card_types != BTreeSet::from([CardType::Land])
        || candidate.is_basic_land
        || binding.card_definition != CARD_ID
        || binding.ability.id != ABILITY_ID
        || binding.ability.mana_cost != ManaCost::new(0)
        || !binding.ability.tap_cost
        || binding.ability.sorcery_speed
        || binding.ability.targets != [TargetRequirement::Player]
        || binding.ability.effects != [Effect::MillTargetPlayer { count: 1 }]
    {
        return Err(
            "Duskmantle identity or target-mill-one ability differs from its feature contract"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Duskmantle source: {error}"))?;
    let file =
        syn::parse_file(&source).map_err(|_| "Duskmantle source is not valid Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Duskmantle has an unsupported module attribute".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["cardbench_magic_engine", "ActivatedAbility"],
        vec!["cardbench_magic_engine", "ActivatedAbilityBinding"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TargetRequirement"],
        vec!["std", "collections", "BTreeSet"],
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
                flatten_use(&[], &item.tree, &mut imports)?
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !["definition", "activated_ability"].contains(&name.as_str())
                    || !functions.insert(name)
                    || !matches!(&function.vis,Visibility::Restricted(value) if value.path.is_ident("crate"))
                    || function.sig.constness.is_some()
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || !function.sig.generics.params.is_empty()
                    || function.sig.generics.where_clause.is_some()
                    || !function.sig.inputs.is_empty()
                    || function.sig.variadic.is_some()
                    || function.block.stmts.len() != 1
                    || !function
                        .attrs
                        .iter()
                        .all(|attr| is_doc_attribute(attr) || attr.path().is_ident("must_use"))
                {
                    return Err(
                        "Duskmantle module must contain only the two direct literal functions"
                            .to_owned(),
                    );
                }
                let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
                    return Err(
                        "Duskmantle functions must return direct structure literals".to_owned()
                    );
                };
                if value.rest.is_some()
                    || !safe_path(&value.path)
                    || !value
                        .fields
                        .iter()
                        .all(|field| safe_value_expr(&field.expr))
                {
                    return Err(
                        "Duskmantle source contains a nonliteral field expression".to_owned()
                    );
                }
            }
            _ => return Err("Duskmantle module contains executable top-level code".to_owned()),
        }
    }
    if file.items.len() != 4
        || functions != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()])
        || imports != expected_imports
    {
        return Err(format!(
            "Duskmantle imports or module structure differ from its source contract: items={}, functions={functions:?}, imports={imports:?}",
            file.items.len()
        ));
    }
    Ok(())
}

fn flatten_use(
    prefix: &[String],
    tree: &UseTree,
    out: &mut BTreeSet<Vec<String>>,
) -> Result<(), String> {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix.to_vec();
            next.push(path.ident.to_string());
            flatten_use(&next, &path.tree, out)
        }
        UseTree::Name(name) => {
            let mut path = prefix.to_vec();
            path.push(name.ident.to_string());
            out.insert(path);
            Ok(())
        }
        UseTree::Group(group) => {
            for item in &group.items {
                flatten_use(prefix, item, out)?;
            }
            Ok(())
        }
        _ => Err("Duskmantle has an unsupported import form".to_owned()),
    }
}
fn is_doc_attribute(attr: &syn::Attribute) -> bool {
    attr.path().is_ident("doc") && matches!(attr.meta, Meta::NameValue(_))
}
fn safe_path(path: &syn::Path) -> bool {
    path.leading_colon.is_none()
        && path
            .segments
            .iter()
            .all(|segment| matches!(&segment.arguments, syn::PathArguments::None))
}
fn safe_value_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(value) => matches!(&value.lit, Lit::Str(_) | Lit::Int(_) | Lit::Bool(_)),
        Expr::Path(value) => value.qself.is_none() && safe_path(&value.path),
        Expr::Array(value) => value.elems.iter().all(safe_value_expr),
        Expr::Reference(value) => value.mutability.is_none() && safe_value_expr(&value.expr),
        Expr::Struct(value) => {
            value.rest.is_none()
                && safe_path(&value.path)
                && value
                    .fields
                    .iter()
                    .all(|field| safe_value_expr(&field.expr))
        }
        Expr::Call(value) => {
            let constructor = match &*value.func {
                Expr::Path(path) => {
                    let names: Vec<_> = path
                        .path
                        .segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect();
                    matches!(names.as_slice(),[a,b] if (a=="BTreeSet"&&(b=="new"||b=="from"))||(a=="ManaCost"&&b=="new"))
                }
                _ => false,
            };
            constructor && value.args.iter().all(safe_value_expr)
        }
        Expr::Macro(value) => {
            value.mac.path.is_ident("vec")
                && syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
                    .parse2(value.mac.tokens.clone())
                    .is_ok_and(|items| items.iter().all(safe_value_expr))
        }
        _ => false,
    }
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}
fn output_root() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--output-root" {
            return args.next().map(std::path::PathBuf::from);
        }
    }
    None
}
fn write_outputs(root: &std::path::Path, report: &Value) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    std::fs::write(
        root.join("result.json"),
        serde_json::to_vec_pretty(report).expect("report serializes"),
    )?;
    std::fs::write(root.join("reward.txt"), "0.0\n")
}
