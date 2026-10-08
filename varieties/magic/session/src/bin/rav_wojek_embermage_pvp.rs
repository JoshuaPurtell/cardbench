//! Full-game RAV Wojek Embermage Radiance damage authority.

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbilityBinding, CardDefinition, CardType, CastRequest, Color,
    DeckEntry, DeckList, Effect, Game, GameEvent, GameView, ManaCost, ManaPaymentSelection,
    ObjectId, PlayerId, PolicyAction, Step, Target, TargetRequirement, Zone,
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
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::parse::Parser;
use syn::{Expr, Item, Lit, Meta, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-WOJEK-EMBERMAGE";
const CARD_NAME: &str = "Wojek Embermage";
const ALLY_ID: &str = "RAV-WATCHWOLF";
const TARGET_ID: &str = "RAV-WATCHWOLF";
const ABILITY_ID: &str = "tap-radiance-one-damage";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/wojek_embermage.rs";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/wojek_embermage.rs"]
mod candidate_wojek_embermage;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Wojek Embermage result: {error}");
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
            eprintln!("Wojek Embermage PvP authority failed: {error}");
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
        return Err("Wojek Embermage full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "missing_radiance_mutation_detected_in_game": mutation_detected(seed, Mutation::NoRadiance)?,
        "wrong_radiance_damage_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongDamage)?,
        "missing_tap_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::NoTapCost)?,
        "wrong_target_requirement_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongTarget)?,
        "wrong_cast_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongCastCost)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "missing_radiance_mutation_detected_in_game",
        "wrong_radiance_damage_mutation_detected_in_game",
        "missing_tap_cost_mutation_detected_in_game",
        "wrong_target_requirement_mutation_detected_in_game",
        "wrong_cast_cost_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!("Wojek Embermage semantic mutation escaped: {key}"));
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
        "card":{"collector_number":152,"printings":1,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":[
                "117.1b and 117.3c: priority and stack resolution",
                "302.6: summoning sickness and tap-cost abilities",
                "601.2: spell casting and payment",
                "602.1-602.2: activation, target, and tap cost",
                "608.2: activated ability resolution",
                "120.3: damage",
                "105.2: object colors used by the shared-color recipient scope",
                "704.5g: lethal damage state-based actions",
                "510: combat damage and full-game result"
            ],
            "source_contract":"RAV Wojek Embermage #152 is a {3}{R} 1/2 red creature with {T}: Wojek Embermage deals 1 damage to target creature and each other creature that shares a color with it",
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
    NoRadiance,
    WrongDamage,
    NoTapCost,
    WrongTarget,
    WrongCastCost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    embermage: Option<ObjectId>,
    spell_payment: Vec<Color>,
    cast_turn: Option<u32>,
    ally_watchwolf: Option<ObjectId>,
    target_watchwolf: Option<ObjectId>,
    activation_source_tapped: bool,
    activation_target: Option<ObjectId>,
    activation_turn: Option<u32>,
    damage_recipients: BTreeMap<ObjectId, i32>,
    source_zone_after_resolution: Option<Zone>,
    ally_zone_after_resolution: Option<Zone>,
    target_zone_after_resolution: Option<Zone>,
    attack_turns: Vec<u32>,
    combat_damage: i32,
    life: [i64; 2],
}

fn same_match(a: &Captured, b: &Captured) -> bool {
    a.events == b.events
        && a.terminal == b.terminal
        && a.winner == b.winner
        && a.turns == b.turns
        && a.embermage == b.embermage
        && a.spell_payment == b.spell_payment
        && a.cast_turn == b.cast_turn
        && a.ally_watchwolf == b.ally_watchwolf
        && a.target_watchwolf == b.target_watchwolf
        && a.activation_source_tapped == b.activation_source_tapped
        && a.activation_target == b.activation_target
        && a.activation_turn == b.activation_turn
        && a.damage_recipients == b.damage_recipients
        && a.source_zone_after_resolution == b.source_zone_after_resolution
        && a.ally_zone_after_resolution == b.ally_zone_after_resolution
        && a.target_zone_after_resolution == b.target_zone_after_resolution
        && a.attack_turns == b.attack_turns
        && a.combat_damage == b.combat_damage
        && a.life == b.life
}

fn candidate_values(mutation: Mutation) -> (CardDefinition, ActivatedAbilityBinding) {
    let mut definition = candidate_wojek_embermage::definition();
    let mut binding = candidate_wojek_embermage::activated_ability();
    match mutation {
        Mutation::None => {}
        Mutation::NoRadiance => {
            binding.ability.effects = vec![Effect::DealDamage {
                amount: 1,
                target: TargetRequirement::Creature,
            }]
        }
        Mutation::WrongDamage => {
            binding.ability.effects = vec![Effect::RadianceDealDamageToCreatures { amount: 2 }]
        }
        Mutation::NoTapCost => binding.ability.tap_cost = false,
        Mutation::WrongTarget => binding.ability.targets = vec![TargetRequirement::Player],
        Mutation::WrongCastCost => definition.mana_cost = ManaCost::new(5),
    }
    (definition, binding)
}

fn validate_registry_delegation() -> Result<(), String> {
    let (definition, binding) = candidate_values(Mutation::None);
    let registered_definition = card_definitions()
        .into_iter()
        .find(|known| known.id == CARD_ID)
        .ok_or("Wojek Embermage is absent from the RAV registry")?;
    let registered_binding = rav_activated_ability_bindings()
        .into_iter()
        .find(|known| known.card_definition == CARD_ID && known.ability.id == ABILITY_ID)
        .ok_or("Wojek Embermage activation is absent from the RAV registry")?;
    if registered_definition != definition || registered_binding != binding {
        return Err(
            "Wojek Embermage definition or binding does not delegate to its module".to_owned(),
        );
    }
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(3, [Color::Red])
        || definition.colors != BTreeSet::from([Color::Red])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.power != Some(1)
        || definition.toughness != Some(2)
        || definition.is_basic_land
        || !definition.effects.is_empty()
        || !definition.keywords.is_empty()
        || binding.card_definition != CARD_ID
        || binding.ability.id != ABILITY_ID
        || binding.ability.mana_cost != ManaCost::new(0)
        || !binding.ability.tap_cost
        || binding.ability.sorcery_speed
        || binding.ability.targets != [TargetRequirement::Creature]
        || binding.ability.effects != [Effect::RadianceDealDamageToCreatures { amount: 1 }]
    {
        return Err(
            "Wojek Embermage identity or Radiance activation differs from its feature contract"
                .to_owned(),
        );
    }
    Ok(())
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_WOJEK_EMBERMAGE_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_WOJEK_EMBERMAGE_SEED must be an unsigned integer")?;
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
        "no bounded seed produced the Radiance damage full-game trace: {last}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    Ok(match run_match(seed, mutation) {
        Ok(capture) => validate_trace(&capture, seed).is_err(),
        Err(_) => true,
    })
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let (definition, binding) = candidate_values(mutation);
    let cast_cost = definition.mana_cost.clone();
    let p0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: ALLY_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 18,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 18,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 16,
            },
        ],
        sideboard: vec![],
    };
    let p1 = DeckList {
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
        return Err("Wojek Embermage fixtures must be legal shuffled 60-card decks".to_owned());
    }
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|known| known.id == CARD_ID)
        .ok_or("Wojek Embermage definition missing")? = definition;
    let mut abilities = rav_activated_ability_bindings();
    *abilities
        .iter_mut()
        .find(|known| known.card_definition == CARD_ID && known.ability.id == ABILITY_ID)
        .ok_or("Wojek Embermage activation missing")? = binding;
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
        max_policy_moves: 8_000,
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
        Box::new(EmbermagePolicy::new(PlayerId(0), cast_cost)),
        Box::new(EmbermagePolicy::new(
            PlayerId(1),
            ManaCost::with_colors(0, [Color::Green, Color::White]),
        )),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut embermage = None;
    let mut spell_payment = vec![];
    let mut cast_turn = None;
    let mut ally_watchwolf = None;
    let mut target_watchwolf = None;
    let mut activation_source = None;
    let mut activation_target = None;
    let mut activation_source_tapped = false;
    let mut activation_turn = None;
    let mut damage_recipients = BTreeMap::new();
    let mut source_zone_after_resolution = None;
    let mut ally_zone_after_resolution = None;
    let mut target_zone_after_resolution = None;
    let mut attack_turns = vec![];
    let mut combat_damage = 0;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(format!(
                "Wojek Embermage match exceeded bound at turn {} after {accepted} moves",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Wojek Embermage policy seat missing")?;
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
                    "Wojek Embermage policy move rejected at turn {} {:?}: {e}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|e| format!("Wojek Embermage invariant failed after move: {e}"))?;
        if let Some(activation) = activation {
            activation_source = Some(activation.source);
            activation_target = activation.targets.first().and_then(|target| {
                if let Target::Permanent(id) = target {
                    Some(*id)
                } else {
                    None
                }
            });
            activation_source_tapped = game
                .object(activation.source)
                .is_ok_and(|object| object.tapped);
            activation_turn = Some(game.turn);
        }
        for event in &game.event_log[cursor..] {
            match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|known| known.id == CARD_ID) =>
                {
                    spell_payment = colors.clone()
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|known| known.id == CARD_ID) =>
                {
                    embermage = Some(*card);
                    cast_turn = Some(game.turn);
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|known| known.id == ALLY_ID)
                    && ally_watchwolf.is_none() =>
                {
                    ally_watchwolf = Some(*card)
                }
                GameEvent::SpellCast {
                    player: PlayerId(1),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|known| known.id == TARGET_ID)
                    && target_watchwolf.is_none() =>
                {
                    target_watchwolf = Some(*card)
                }
                GameEvent::DamageDealtToPermanent {
                    source,
                    permanent,
                    amount,
                } if Some(*source) == embermage && Some(*source) == activation_source => {
                    *damage_recipients.entry(*permanent).or_insert(0) += *amount;
                }
                GameEvent::AbilityResolved {
                    source, ability, ..
                } if *ability == ABILITY_ID && Some(*source) == activation_source => {
                    source_zone_after_resolution = game.zone_of(*source);
                    ally_zone_after_resolution = ally_watchwolf.and_then(|id| game.zone_of(id));
                    target_zone_after_resolution = target_watchwolf.and_then(|id| game.zone_of(id));
                }
                GameEvent::AttackersDeclared {
                    player: PlayerId(0),
                    attackers,
                } if ally_watchwolf.is_some_and(|id| attackers.contains(&id)) => {
                    attack_turns.push(game.turn)
                }
                GameEvent::DamageDealtToPlayer {
                    source,
                    player: PlayerId(1),
                    amount,
                } if Some(*source) == ally_watchwolf => combat_damage += *amount,
                _ => {}
            }
        }
    }
    game.validate_invariants()
        .map_err(|e| format!("Wojek Embermage terminal invariant failed: {e}"))?;
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
        embermage,
        spell_payment,
        cast_turn,
        ally_watchwolf,
        target_watchwolf,
        activation_source_tapped,
        activation_target,
        activation_turn,
        damage_recipients,
        source_zone_after_resolution,
        ally_zone_after_resolution,
        target_zone_after_resolution,
        attack_turns,
        combat_damage,
        life,
    })
}

struct EmbermagePolicy {
    player: PlayerId,
    cast_cost: ManaCost,
    source_cast_offered: bool,
    ally_cast_offered: bool,
    target_cast_offered: bool,
    ability_offered: bool,
    activated: bool,
}
impl EmbermagePolicy {
    fn new(player: PlayerId, cast_cost: ManaCost) -> Self {
        Self {
            player,
            cast_cost,
            source_cast_offered: false,
            ally_cast_offered: false,
            target_cast_offered: false,
            ability_offered: false,
            activated: false,
        }
    }
    fn payment(&self, view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let mut available: BTreeMap<Color, u8> = Color::MANA_ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        for color in &cost.colored {
            let amount = available.get_mut(color)?;
            if *amount == 0 {
                return None;
            }
            *amount -= 1;
        }
        let mut generic = vec![];
        for _ in 0..cost.generic {
            let color = available
                .iter()
                .filter(|(_, amount)| **amount > 0)
                .max_by_key(|(_, amount)| **amount)
                .map(|(color, _)| *color)?;
            *available.get_mut(&color)? -= 1;
            generic.push(color);
        }
        Some(ManaPaymentSelection {
            generic,
            hybrid: vec![],
        })
    }
    fn make_mana(&self, view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
        let missing_color = cost
            .colored
            .iter()
            .copied()
            .find(|color| view.mana_pool.amount(*color) == 0);
        let short_generic =
            usize::from(view.mana_pool.total()) < usize::from(cost.generic) + cost.colored.len();
        let land = view.own_battlefield.iter().find(|card| {
            card.card_types.contains(&CardType::Land)
                && !card.tapped
                && missing_color.is_none_or(|color| card.mana_colors.contains(&color))
        })?;
        let color = missing_color
            .or_else(|| land.mana_colors.iter().next().copied())
            .unwrap_or(Color::Green);
        if short_generic && missing_color.is_none() {
            return Some(PolicyAction::ActivateManaAbility {
                land: land.id,
                color,
            });
        }
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }
    fn cast(&mut self, view: &GameView, card_id: &str, cost: &ManaCost) -> Option<PolicyAction> {
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(card_id))?;
        if let Some(mana_selection) = self.payment(view, cost) {
            match card_id {
                CARD_ID => self.source_cast_offered = true,
                ALLY_ID if self.player == PlayerId(0) => self.ally_cast_offered = true,
                TARGET_ID => self.target_cast_offered = true,
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
        self.make_mana(view, cost)
    }
}
impl CodePolicy for EmbermagePolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.wojek-embermage-radiance-pilot.v1"
        } else {
            "rav.wojek-embermage-target-fixture.v1"
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
                attackers: if self.player == PlayerId(0) && self.activated {
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
        if view.priority != self.player || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }
        if self.player == PlayerId(0) && !self.ability_offered && view.active_player != self.player
        {
            let source = view.own_battlefield.iter().find(|card| {
                card.definition == Some(CARD_ID) && !card.tapped && !card.summoning_sick
            });
            let target = view.opponent_battlefield.iter().find(|card| {
                card.definition == Some(TARGET_ID) && card.card_types.contains(&CardType::Creature)
            });
            if let (Some(source), Some(target)) = (source, target) {
                self.ability_offered = true;
                self.activated = true;
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
        if view.active_player != self.player || !view.step.is_main() {
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
            if !self.ally_cast_offered
                && !view
                    .own_battlefield
                    .iter()
                    .any(|card| card.definition == Some(ALLY_ID))
            {
                let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
                if let Some(action) = self.cast(view, ALLY_ID, &cost) {
                    return action;
                }
            }
            if !self.source_cast_offered
                && !view
                    .own_battlefield
                    .iter()
                    .any(|card| card.definition == Some(CARD_ID))
                && view
                    .own_battlefield
                    .iter()
                    .any(|card| card.definition == Some(ALLY_ID))
            {
                let cost = self.cast_cost.clone();
                if let Some(action) = self.cast(view, CARD_ID, &cost) {
                    return action;
                }
            }
        } else if !self.target_cast_offered
            && !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(TARGET_ID))
        {
            let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
            if let Some(action) = self.cast(view, TARGET_ID, &cost) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(c: &Captured, seed: u64) -> Result<Value, String> {
    if c.terminal != "Winner(PlayerId(0))" || c.winner != Some(0) || c.rejected != 0 {
        return Err("Wojek Embermage match did not reach a clean seat-zero win".to_owned());
    }
    let source = c.embermage.ok_or("Wojek Embermage was not cast")?;
    let ally = c.ally_watchwolf.ok_or("pilot Watchwolf was not cast")?;
    let target = c
        .target_watchwolf
        .ok_or("opponent Watchwolf target was not cast")?;
    let expected_cost = ManaCost::with_colors(3, [Color::Red]);
    if candidate_wojek_embermage::definition().mana_cost != expected_cost
        || c.spell_payment.len() != 4
        || c.spell_payment
            .iter()
            .filter(|color| **color == Color::Red)
            .count()
            != 1
    {
        return Err("Wojek Embermage did not pay exact {3}{R} casting cost".to_owned());
    }
    if c.activation_target != Some(target)
        || !c.activation_source_tapped
        || c.source_zone_after_resolution != Some(Zone::Battlefield)
        || c.ally_zone_after_resolution != Some(Zone::Battlefield)
        || c.target_zone_after_resolution != Some(Zone::Battlefield)
    {
        return Err(format!(
            "Wojek Embermage activation or surviving Radiance creatures differ: ally={ally:?}, target={:?}/{target:?}, tapped={}, source_zone={:?}, ally_zone={:?}, opponent_zone={:?}, recipients={:?}, activation_turn={:?}, cast_turn={:?}",
            c.activation_target,
            c.activation_source_tapped,
            c.source_zone_after_resolution,
            c.ally_zone_after_resolution,
            c.target_zone_after_resolution,
            c.damage_recipients,
            c.activation_turn,
            c.cast_turn
        ));
    }
    let expected = BTreeMap::from([(ally, 1), (target, 1)]);
    if c.damage_recipients != expected {
        return Err(format!(
            "Radiance recipient/damage set differs: expected={expected:?}, actual={:?}",
            c.damage_recipients
        ));
    }
    if c.cast_turn
        .zip(c.activation_turn)
        .is_none_or(|(cast, activation)| activation <= cast)
    {
        return Err("Wojek Embermage activated before summoning sickness expired".to_owned());
    }
    if c.attack_turns.len() < 3 || c.combat_damage < 9 || c.life[0] <= 0 || c.life[1] > 0 {
        return Err("Wojek Embermage full-game combat evidence is incomplete".to_owned());
    }
    let events = &c.events;
    let paid=event_index(events,|event|matches!(event,GameEvent::SpellManaPaid{player:PlayerId(0),card,colors}if *card==source&&colors.len()==4&&colors.iter().filter(|color|**color==Color::Red).count()==1)).ok_or("Embermage {3}{R} payment event missing")?;
    let cast = event_index_after(
        events,
        paid,
        |event| matches!(event,GameEvent::SpellCast{player:PlayerId(0),card}if *card==source),
    )
    .ok_or("Embermage SpellCast receipt missing")?;
    let resolved = event_index_after(
        events,
        cast,
        |event| matches!(event,GameEvent::SpellResolved{card}if *card==source),
    )
    .ok_or("Embermage SpellResolved receipt missing")?;
    let entered = event_index_after(
        events,
        resolved,
        |event| matches!(event,GameEvent::CardMoved{card,to:Zone::Battlefield}if *card==source),
    )
    .ok_or("Embermage battlefield entry missing")?;
    let activated=event_index_after(events,entered,|event|matches!(event,GameEvent::AbilityActivated{player:PlayerId(0),source:known,definition:CARD_ID,ability:ABILITY_ID,..}if *known==source)).ok_or("Embermage activation receipt missing")?;
    let ally_hit=event_index_after(events,activated,|event|matches!(event,GameEvent::DamageDealtToPermanent{source:known,permanent,amount:1}if *known==source&&*permanent==ally)).ok_or("Radiance did not hit the friendly shared-color Watchwolf")?;
    let target_hit=event_index_after(events,activated,|event|matches!(event,GameEvent::DamageDealtToPermanent{source:known,permanent,amount:1}if *known==source&&*permanent==target)).ok_or("Radiance did not hit the targeted opponent Watchwolf")?;
    let ability_resolved=event_index_after(events,ally_hit.max(target_hit),|event|matches!(event,GameEvent::AbilityResolved{source:known,ability:ABILITY_ID,..}if *known==source)).ok_or("Embermage AbilityResolved receipt missing")?;
    let attack=event_index_after(events,ability_resolved,|event|matches!(event,GameEvent::AttackersDeclared{player:PlayerId(0),attackers}if attackers.contains(&ally)).then_some(()) .is_some()).ok_or("Watchwolf did not attack after Embermage activation")?;
    let damage=event_index_after(events,attack,|event|matches!(event,GameEvent::DamageDealtToPlayer{source:known,player:PlayerId(1),amount}if *known==ally&&*amount>0)).ok_or("pilot Watchwolf combat damage receipt missing")?;
    let ended = event_index_after(events, damage, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("seat-zero GameEnded receipt missing")?;
    let projected = project_events(events);
    Ok(json!({
        "trace_id":"rav-wojek-embermage-radiance-full-game",
        "shuffle_seed":seed,"terminal":c.terminal,"winner_seat":0,"turns":c.turns,"rejected_moves":c.rejected,
        "embermage":{"object":source.0,"definition":CARD_ID,"mana_cost":"{3}{R}","mana_paid":c.spell_payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"power":1,"toughness":2,"cast_turn":c.cast_turn},
        "activation":{"ability":ABILITY_ID,"source":source.0,"target":target.0,"source_tapped_after_cost":c.activation_source_tapped,"activation_turn":c.activation_turn},
        "radiance":{"target":{"object":target.0,"definition":TARGET_ID,"controller":1,"damage":c.damage_recipients.get(&target).copied().unwrap_or(0)},"same_color_creature":{"object":ally.0,"definition":ALLY_ID,"controller":0,"damage":c.damage_recipients.get(&ally).copied().unwrap_or(0)},"recipients":c.damage_recipients.iter().map(|(object,amount)|json!({"object":object.0,"damage":amount})).collect::<Vec<_>>(),"source_not_damaged":!c.damage_recipients.contains_key(&source)},
        "combat":{"card":"Watchwolf","object":ally.0,"attack_turns":c.attack_turns,"damage_to_opponent":c.combat_damage,"life_at_terminal":c.life},
        "ordered_event_sequence":[paid,cast,resolved,entered,activated,ally_hit.min(target_hit),ally_hit.max(target_hit),ability_resolved,attack,damage,ended],"events":projected
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
    let passed = card.collector_number == 152;
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"engine_status":if definition_id.is_some(){"executable_compatibility_slice"}else{"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"qualification":if passed{"diagnostic_only"}else{"not_evaluated"},"credited":false})
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|e| format!("could not read Wojek Embermage module: {e}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Wojek Embermage module is invalid Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Wojek Embermage has an unsupported module attribute".to_owned());
    }
    let expected: BTreeSet<Vec<String>> = [
        vec!["cardbench_magic_engine", "ActivatedAbility"],
        vec!["cardbench_magic_engine", "ActivatedAbilityBinding"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
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
                        .all(|attr| is_doc_attribute(attr) || attr.path().is_ident("must_use"))
                {
                    return Err(
                        "Wojek Embermage module must contain only direct declarative functions"
                            .to_owned(),
                    );
                }
                let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
                    return Err(
                        "Wojek Embermage functions must return direct struct literals".to_owned(),
                    );
                };
                if value.rest.is_some() {
                    return Err("Wojek Embermage struct update syntax is not allowed".to_owned());
                }
                if let Some(field) = value.fields.iter().find(|field| !safe_value(&field.expr)) {
                    let field_name = match &field.member {
                        syn::Member::Named(name) => name.to_string(),
                        syn::Member::Unnamed(index) => index.index.to_string(),
                    };
                    return Err(format!(
                        "Wojek Embermage contains a nonliteral expression in {field_name}"
                    ));
                }
            }
            _ => return Err("Wojek Embermage module contains executable top-level code".to_owned()),
        }
    }
    if file.items.len() != 4
        || functions != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()])
        || imports != expected
    {
        return Err(
            "Wojek Embermage module imports or structure differ from the declarative contract"
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
        _ => Err("Wojek Embermage module has an unsupported import form".to_owned()),
    }
}
fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc") && matches!(attribute.meta, Meta::NameValue(_))
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
                        .map(|segment| segment.ident.to_string())
                        .collect();
                    matches!(names.as_slice(),[a]if a=="Some")
                        || matches!(names.as_slice(),[a,b]if(a=="BTreeSet"&&(b=="new"||b=="from"))||(a=="ManaCost"&&(b=="new"||b=="with_colors")))
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
