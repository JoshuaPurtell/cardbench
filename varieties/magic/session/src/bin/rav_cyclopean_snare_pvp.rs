//! Full-game diagnostic authority for RAV Cyclopean Snare.

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
const CARD_ID: &str = "RAV-CYCLOPEAN-SNARE";
const CARD_NAME: &str = "Cyclopean Snare";
const ABILITY_ID: &str = "tap-target-creature";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/cyclopean_snare.rs";
const MAX_TRACE_SEED: u64 = 128;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/cyclopean_snare.rs"]
mod candidate_cyclopean_snare;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Cyclopean Snare result: {error}");
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
            eprintln!("Cyclopean Snare PvP authority failed: {error}");
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
        return Err("Cyclopean Snare full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "missing_target_tap_mutation_detected_in_game": mutation_detected(seed, Mutation::NoTargetTap)?,
        "missing_source_return_mutation_detected_in_game": mutation_detected(seed, Mutation::NoSourceReturn)?,
        "missing_source_tap_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::NoTapCost)?,
        "wrong_activation_mana_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongAbilityCost)?,
        "missing_creature_target_mutation_detected_in_game": mutation_detected(seed, Mutation::NoCreatureTarget)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "missing_target_tap_mutation_detected_in_game",
        "missing_source_return_mutation_detected_in_game",
        "missing_source_tap_cost_mutation_detected_in_game",
        "wrong_activation_mana_cost_mutation_detected_in_game",
        "missing_creature_target_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!("Cyclopean Snare semantic mutation escaped: {key}"));
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
        "card":{"collector_number":259,"printings":1,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":[
                "117.1b and 117.3c: priority and stack resolution",
                "301.1 and 601.2: artifact identity, casting, and payment",
                "400.7: zone changes create new game objects",
                "602.1-602.2: activation and payment of {3} and a tap cost",
                "701.21: tapping a target creature",
                "508 and 510: repeated combat to a full-game result"
            ],
            "source_contract":"RAV Cyclopean Snare #259 is a {2} artifact with {3}, {T}: tap target creature; return Cyclopean Snare to its owner's hand",
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
    NoTargetTap,
    NoSourceReturn,
    NoTapCost,
    WrongAbilityCost,
    NoCreatureTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    snare: Option<ObjectId>,
    snare_cast_payment: Vec<Color>,
    source_tapped_at_activation: bool,
    source_zone_after_resolution: Option<Zone>,
    target: Option<ObjectId>,
    target_tapped_after_resolution: bool,
    target_zone_after_resolution: Option<Zone>,
    target_owner: Option<PlayerId>,
    ability_mana_cost: Option<ManaCost>,
    watchwolf: Option<ObjectId>,
    attacks: Vec<u32>,
    damage: i32,
    life: [i64; 2],
}

fn same_match(a: &Captured, b: &Captured) -> bool {
    a.events == b.events
        && a.terminal == b.terminal
        && a.winner == b.winner
        && a.turns == b.turns
        && a.snare == b.snare
        && a.snare_cast_payment == b.snare_cast_payment
        && a.source_tapped_at_activation == b.source_tapped_at_activation
        && a.source_zone_after_resolution == b.source_zone_after_resolution
        && a.target == b.target
        && a.target_tapped_after_resolution == b.target_tapped_after_resolution
        && a.target_zone_after_resolution == b.target_zone_after_resolution
        && a.target_owner == b.target_owner
        && a.ability_mana_cost == b.ability_mana_cost
        && a.watchwolf == b.watchwolf
        && a.attacks == b.attacks
        && a.damage == b.damage
        && a.life == b.life
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_CYCLOPEAN_SNARE_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_CYCLOPEAN_SNARE_SEED must be an unsigned integer")?;
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
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
        "no bounded seed produced the Cyclopean Snare target-tap/source-return game: {last}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    Ok(match run_match(seed, mutation) {
        Ok(capture) => validate_trace(&capture, seed).is_err(),
        Err(_) => true,
    })
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
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
    let p1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: WATCHWOLF_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 28,
            },
        ],
        sideboard: vec![],
    };
    if [&p0, &p1].iter().any(|deck| {
        deck.mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
    }) {
        return Err("Cyclopean Snare fixtures must be legal shuffled 60-card decks".to_owned());
    }
    let definition = candidate_cyclopean_snare::definition();
    let mut binding = candidate_cyclopean_snare::activated_ability();
    match mutation {
        Mutation::None => {}
        Mutation::NoTargetTap => binding.ability.effects = vec![Effect::ReturnSourceToOwnersHand],
        Mutation::NoSourceReturn => binding.ability.effects = vec![Effect::TapTargetCreature],
        Mutation::NoTapCost => binding.ability.tap_cost = false,
        Mutation::WrongAbilityCost => binding.ability.mana_cost = ManaCost::new(2),
        Mutation::NoCreatureTarget => binding.ability.targets.clear(),
    }
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|known| known.id == CARD_ID)
        .ok_or("Cyclopean Snare missing from card definitions")? = definition;
    let mut abilities = rav_activated_ability_bindings();
    let registered = abilities
        .iter_mut()
        .find(|known| known.card_definition == CARD_ID && known.ability.id == ABILITY_ID)
        .ok_or("Cyclopean Snare binding missing from registry")?;
    *registered = binding;
    let mut game = Game::new_with_all_bindings(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
    )
    .map_err(|e| e.to_string())?;
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 60,
        max_policy_moves: 1200,
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
        Box::new(SnarePolicy::new(PlayerId(0))),
        Box::new(SnarePolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut snare = None;
    let mut snare_cast_payment = vec![];
    let mut activation_source = None;
    let mut source_tapped_at_activation = false;
    let mut target = None;
    let mut source_zone_after_resolution = None;
    let mut target_tapped_after_resolution = false;
    let mut target_zone_after_resolution = None;
    let mut target_owner = None;
    let mut ability_mana_cost = None;
    let mut watchwolf = None;
    let mut attacks = vec![];
    let mut damage = 0;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(format!(
                "Cyclopean Snare match exceeded bound at turn {} after {accepted} policy moves (active={:?}, next={:?}, step={:?}, priority={:?}, lands_played={}, stack_depth={})",
                game.turn,
                game.active_player,
                game.next_policy_player(),
                game.step,
                game.priority,
                game.view_for_player(game.next_policy_player())
                    .map(|v| v.lands_played)
                    .unwrap_or_default(),
                game.stack.len()
            ));
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Cyclopean Snare policy seat missing")?;
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
            .map_err(|e| {
                format!(
                    "Cyclopean Snare move rejected at turn {} {:?}: {e}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|e| format!("Cyclopean Snare post-move invariant failed: {e}"))?;
        if let Some(activation) = activation {
            activation_source = Some(activation.source);
            target = activation.targets.first().and_then(|value| {
                if let Target::Permanent(card) = value {
                    Some(*card)
                } else {
                    None
                }
            });
            source_tapped_at_activation = game
                .object(activation.source)
                .is_ok_and(|object| object.tapped);
        }
        for event in &game.event_log[cursor..] {
            match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|def| def.id == CARD_ID) =>
                {
                    snare = Some(*card)
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    snare_cast_payment = colors.clone()
                }
                GameEvent::AbilityManaPaid {
                    player: PlayerId(0),
                    source,
                    ability,
                    mana_cost,
                } if *ability == ABILITY_ID && activation_source == Some(*source) => {
                    ability_mana_cost = Some(mana_cost.clone())
                }
                GameEvent::AbilityResolved {
                    source, ability, ..
                } if *ability == ABILITY_ID && activation_source == Some(*source) => {
                    source_zone_after_resolution = game.zone_of(*source);
                    if let Some(target) = target {
                        target_tapped_after_resolution =
                            game.object(target).is_ok_and(|object| object.tapped);
                        target_zone_after_resolution = game.zone_of(target);
                        target_owner = game.object(target).ok().map(|object| object.owner);
                    }
                }
                GameEvent::SpellCast {
                    player: PlayerId(seat),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|def| def.id == WATCHWOLF_ID) =>
                {
                    if watchwolf.is_none() || *seat == 0 {
                        watchwolf = Some(*card);
                    }
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
                } if watchwolf == Some(*source) => damage += *amount,
                _ => {}
            }
        }
    }
    game.validate_invariants()
        .map_err(|e| format!("Cyclopean Snare terminal invariant failed: {e}"))?;
    let life = [
        game.player(PlayerId(0)).map_err(|e| e.to_string())?.life,
        game.player(PlayerId(1)).map_err(|e| e.to_string())?.life,
    ];
    let winner = game.winner().map(|p| p.0);
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
        snare,
        snare_cast_payment,
        source_tapped_at_activation,
        source_zone_after_resolution,
        target,
        target_tapped_after_resolution,
        target_zone_after_resolution,
        target_owner,
        ability_mana_cost,
        watchwolf,
        attacks,
        damage,
        life,
    })
}

struct SnarePolicy {
    player: PlayerId,
    snare_cast: bool,
    activation_offered: bool,
    wolf_cast: bool,
    opponent_wolf_cast: bool,
}
impl SnarePolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            snare_cast: false,
            activation_offered: false,
            wolf_cast: false,
            opponent_wolf_cast: false,
        }
    }
}
impl CodePolicy for SnarePolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.cyclopean-snare-pilot.v1"
        } else {
            "rav.cyclopean-snare-fixture.v1"
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
            if self.player == PlayerId(0) {
                return PolicyAction::DeclareAttackers {
                    attackers: view
                        .own_battlefield
                        .iter()
                        .filter(|card| card.definition == Some(WATCHWOLF_ID) && card.can_attack)
                        .map(|card| card.id)
                        .collect(),
                };
            }
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
        if view.priority != self.player || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }
        if self.player == PlayerId(0)
            && !self.activation_offered
            && view.active_player != self.player
        {
            let source = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID) && !card.tapped);
            let target = view.opponent_battlefield.iter().find(|card| {
                card.definition == Some(WATCHWOLF_ID)
                    && card.card_types.contains(&CardType::Creature)
                    && !card.tapped
            });
            if let (Some(source), Some(target)) = (source, target) {
                if view.mana_pool.total() < 3 {
                    if let Some(land) = view.own_battlefield.iter().find(|card| {
                        card.card_types.contains(&CardType::Land)
                            && !card.tapped
                            && card.definition != Some(CARD_ID)
                    }) {
                        let color = land
                            .mana_colors
                            .iter()
                            .copied()
                            .next()
                            .unwrap_or(Color::Green);
                        return PolicyAction::ActivateManaAbility {
                            land: land.id,
                            color,
                        };
                    }
                } else {
                    self.activation_offered = true;
                    return PolicyAction::ActivateAbility {
                        activation: AbilityActivation {
                            source: source.id,
                            ability_id: ABILITY_ID,
                            sacrifice_sources: vec![],
                            additional_tap_creatures: vec![],
                            discard_cards: vec![],
                            targets: vec![Target::Permanent(target.id)],
                        },
                    };
                }
            }
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
        if self.player == PlayerId(0) && !self.snare_cast {
            if let Some(snare) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            {
                let cost = ManaCost::new(2);
                if let Some(selection) = payment(view, &cost) {
                    self.snare_cast = true;
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: snare.id,
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
        if self.player == PlayerId(1)
            && !self.opponent_wolf_cast
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
                    self.opponent_wolf_cast = true;
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
        if self.player == PlayerId(0)
            && !self.wolf_cast
            && !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(WATCHWOLF_ID))
            && self.activation_offered
        {
            if let Some(wolf) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(WATCHWOLF_ID))
            {
                let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
                if let Some(selection) = payment(view, &cost) {
                    self.wolf_cast = true;
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
fn make_mana(view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
    let need = cost
        .colored
        .iter()
        .copied()
        .find(|color| view.mana_pool.amount(*color) == 0)
        .or_else(|| {
            (view.mana_pool.total() < cost.generic.saturating_add(cost.colored.len() as u8))
                .then(|| cost.colored.first().copied())
                .flatten()
        });
    let land = view.own_battlefield.iter().find(|card| {
        card.card_types.contains(&CardType::Land)
            && !card.tapped
            && need.is_none_or(|color| card.mana_colors.contains(&color))
    })?;
    let color = need
        .or_else(|| land.mana_colors.iter().copied().next())
        .unwrap_or(Color::Green);
    Some(PolicyAction::ActivateManaAbility {
        land: land.id,
        color,
    })
}

fn validate_trace(c: &Captured, seed: u64) -> Result<Value, String> {
    if c.terminal != "Winner(PlayerId(0))" || c.winner != Some(0) || c.rejected != 0 {
        return Err("Cyclopean Snare match did not reach a clean seat-zero win".to_owned());
    }
    let source = c.snare.ok_or("Cyclopean Snare was not cast")?;
    let target = c
        .target
        .ok_or("Cyclopean Snare ability did not target a creature")?;
    let wolf = c.watchwolf.ok_or("pilot Watchwolf finisher was not cast")?;
    if c.snare_cast_payment.len() != 2
        || c.source_tapped_at_activation == false
        || c.source_zone_after_resolution != Some(Zone::Hand)
        || c.target != Some(target)
        || !c.target_tapped_after_resolution
        || c.target_zone_after_resolution != Some(Zone::Battlefield)
        || c.target_owner != Some(PlayerId(1))
        || c.ability_mana_cost
            .as_ref()
            .is_none_or(|cost| *cost != ManaCost::new(3))
    {
        return Err(format!(
            "Cyclopean Snare cost, tap, target, or source-return state differs: cast_payment={:?}, source_tapped={}, source_zone={:?}, target={:?}, target_tapped={}, target_zone={:?}, target_owner={:?}, ability_cost={:?}",
            c.snare_cast_payment,
            c.source_tapped_at_activation,
            c.source_zone_after_resolution,
            c.target,
            c.target_tapped_after_resolution,
            c.target_zone_after_resolution,
            c.target_owner,
            c.ability_mana_cost
        ));
    }
    if c.attacks.len() < 3 || c.damage < 9 || c.life[0] <= 0 || c.life[1] > 0 {
        return Err("Cyclopean Snare full-game combat terminal evidence is missing".to_owned());
    }
    let events = &c.events;
    let paid=event_index(events,|event|matches!(event,GameEvent::SpellManaPaid{player:PlayerId(0),card,colors}if *card==source&&colors.len()==2)).ok_or("Snare's exact {2} spell payment is missing")?;
    let cast = event_index_after(
        events,
        paid,
        |event| matches!(event,GameEvent::SpellCast{player:PlayerId(0),card}if *card==source),
    )
    .ok_or("Cyclopean Snare SpellCast receipt is missing")?;
    let resolved = event_index_after(
        events,
        cast,
        |event| matches!(event,GameEvent::SpellResolved{card}if *card==source),
    )
    .ok_or("Cyclopean Snare SpellResolved receipt is missing")?;
    let entered = event_index_after(
        events,
        resolved,
        |event| matches!(event,GameEvent::CardMoved{card,to:Zone::Battlefield}if *card==source),
    )
    .ok_or("Cyclopean Snare battlefield entry receipt is missing")?;
    let cost=event_index_after(events,entered,|event|matches!(event,GameEvent::AbilityManaPaid{player:PlayerId(0),source:known,ability,mana_cost}if *known==source&&*ability==ABILITY_ID&&*mana_cost==ManaCost::new(3))).ok_or("Snare {3} activation payment receipt is missing")?;
    let activated=event_index_after(events,cost,|event|matches!(event,GameEvent::AbilityActivated{player:PlayerId(0),source:known,ability,..}if *known==source&&*ability==ABILITY_ID)).ok_or("Snare AbilityActivated receipt is missing")?;
    let tapped=event_index_after(events,activated,|event|matches!(event,GameEvent::PermanentTapped{source:known,card}if *known==source&&*card==target)).ok_or("target PermanentTapped receipt is missing")?;
    let returned = event_index_after(
        events,
        tapped,
        |event| matches!(event,GameEvent::CardMoved{card,to:Zone::Hand}if *card==source),
    )
    .ok_or("Snare source return-to-hand receipt is missing")?;
    let ability_resolved=event_index_after(events,returned,|event|matches!(event,GameEvent::AbilityResolved{source:known,ability,..}if *known==source&&*ability==ABILITY_ID)).ok_or("Snare AbilityResolved receipt is missing")?;
    let attack=event_index_after(events,ability_resolved,|event|matches!(event,GameEvent::AttackersDeclared{player:PlayerId(0),attackers}if attackers.contains(&wolf))).ok_or("Watchwolf did not attack after Snare resolved")?;
    let damage=event_index_after(events,attack,|event|matches!(event,GameEvent::DamageDealtToPlayer{source:known,player:PlayerId(1),amount}if *known==wolf&&*amount>0)).ok_or("Watchwolf damage receipt is missing")?;
    let ended = event_index_after(events, damage, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("seat-zero GameEnded receipt is missing")?;
    let projected = project_events(events);
    Ok(
        json!({"trace_id":"rav-cyclopean-snare-tap-and-return-full-game","shuffle_seed":seed,"terminal":c.terminal,"winner_seat":0,"turns":c.turns,"rejected_moves":c.rejected,
            "snare":{"object":source.0,"mana_cost":"{2}","mana_paid":c.snare_cast_payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"ability_id":ABILITY_ID,"ability_mana_cost":"{3}","activation_mana_paid":3,"tap_cost_paid":true,"source_zone_after_resolution":"Hand"},
        "target":{"object":target.0,"definition":"RAV-WATCHWOLF","owner":1,"zone_after_resolution":"Battlefield","tapped_after_resolution":true},
        "combat":{"card":"Watchwolf","object":wolf.0,"attack_turns":c.attacks,"damage_to_opponent":c.damage,"life_at_terminal":c.life},
        "ordered_event_sequence":[paid,cast,resolved,entered,cost,activated,tapped,returned,ability_resolved,attack,damage,ended],"events":projected}),
    )
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
    let passed = card.collector_number == 259;
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"engine_status":if definition_id.is_some(){"executable_compatibility_slice"}else{"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"qualification":if passed{"diagnostic_only"}else{"not_evaluated"},"credited":false})
}

fn validate_registry_delegation() -> Result<(), String> {
    let definition = candidate_cyclopean_snare::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|known| known.id == CARD_ID)
        .ok_or("Cyclopean Snare is absent from the RAV registry")?;
    let ability = candidate_cyclopean_snare::activated_ability();
    if registered != definition
        || !rav_activated_ability_bindings()
            .iter()
            .any(|known| known == &ability)
    {
        return Err("Cyclopean Snare definition or ability registry does not delegate to the candidate module".to_owned());
    }
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::new(2)
        || !definition.colors.is_empty()
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Artifact])
        || ability.card_definition != CARD_ID
        || ability.ability.id != ABILITY_ID
        || ability.ability.mana_cost != ManaCost::new(3)
        || !ability.ability.tap_cost
        || ability.ability.targets != [TargetRequirement::Creature]
        || ability.ability.effects != [Effect::TapTargetCreature, Effect::ReturnSourceToOwnersHand]
    {
        return Err(
            "Cyclopean Snare identity or activated ability differs from its feature contract"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|e| format!("could not read Cyclopean Snare source: {e}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Cyclopean Snare source is not valid Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Cyclopean Snare has an unsupported module attribute".to_owned());
    }
    let expected: BTreeSet<Vec<String>> = [
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
    .map(|p| p.into_iter().map(str::to_owned).collect())
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
                    || !matches!(&function.vis,Visibility::Restricted(value)if value.path.is_ident("crate"))
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
                        .all(|a| is_doc_attribute(a) || a.path().is_ident("must_use"))
                {
                    return Err(
                        "Cyclopean Snare module must contain only two direct literal functions"
                            .to_owned(),
                    );
                }
                let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
                    return Err(
                        "Cyclopean Snare functions must return direct struct literals".to_owned(),
                    );
                };
                if value.rest.is_some() || !value.fields.iter().all(|field| safe_value(&field.expr))
                {
                    return Err(
                        "Cyclopean Snare contains a nonliteral candidate expression".to_owned()
                    );
                }
            }
            _ => return Err("Cyclopean Snare module contains executable top-level code".to_owned()),
        }
    }
    if file.items.len() != 4
        || functions != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()])
        || imports != expected
    {
        return Err(
            "Cyclopean Snare imports or module structure differ from its source contract"
                .to_owned(),
        );
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
            let mut next = prefix.to_vec();
            next.push(name.ident.to_string());
            out.insert(next);
            Ok(())
        }
        UseTree::Group(group) => {
            for item in &group.items {
                flatten_use(prefix, item, out)?;
            }
            Ok(())
        }
        _ => Err("Cyclopean Snare has an unsupported import form".to_owned()),
    }
}
fn is_doc_attribute(a: &syn::Attribute) -> bool {
    a.path().is_ident("doc") && matches!(a.meta, Meta::NameValue(_))
}
fn safe_path(path: &syn::Path) -> bool {
    path.leading_colon.is_none()
        && path
            .segments
            .iter()
            .all(|segment| matches!(&segment.arguments, syn::PathArguments::None))
}
fn safe_value(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(value) => matches!(&value.lit, Lit::Str(_) | Lit::Int(_) | Lit::Bool(_)),
        Expr::Path(value) => value.qself.is_none() && safe_path(&value.path),
        Expr::Array(value) => value.elems.iter().all(safe_value),
        Expr::Reference(value) => value.mutability.is_none() && safe_value(&value.expr),
        Expr::Struct(value) => {
            value.rest.is_none()
                && safe_path(&value.path)
                && value.fields.iter().all(|field| safe_value(&field.expr))
        }
        Expr::Call(value) => {
            let allowed = match &*value.func {
                Expr::Path(path) => {
                    let names: Vec<_> = path
                        .path
                        .segments
                        .iter()
                        .map(|s| s.ident.to_string())
                        .collect();
                    matches!(names.as_slice(),[a,b]if(a=="BTreeSet"&&(b=="new"||b=="from"))||(a=="ManaCost"&&b=="new"))
                }
                _ => false,
            };
            allowed && value.args.iter().all(safe_value)
        }
        Expr::Macro(value) => {
            value.mac.path.is_ident("vec")
                && syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
                    .parse2(value.mac.tokens.clone())
                    .is_ok_and(|items| items.iter().all(safe_value))
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
