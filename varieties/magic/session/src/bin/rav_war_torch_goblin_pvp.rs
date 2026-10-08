//! Full-game RAV War-Torch Goblin sacrifice-damage trace.

use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, CardDefinition, CardType, CastRequest, Color, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, Step, Target, TargetRequirement, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-WAR-TORCH-GOBLIN";
const ATTACKER_ID: &str = "RAV-GOBLIN-FIRE-FIEND";
const BLOCKER_ID: &str = "RAV-TERRAFORMER";
const ABILITY_ID: &str = "sacrifice-deal-two-to-blocker";
const MAX_TRACE_SEED: u64 = 128;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/war_torch_goblin.rs"]
mod candidate_war_torch_goblin;

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
            eprintln!("War-Torch Goblin PvP authority failed: {error}");
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
    {
        return Err("RAV catalog denominator differs from the source contract".to_owned());
    }

    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.terminal != replay.terminal
        || captured.winner != replay.winner
        || captured.goblin != replay.goblin
        || captured.attacker != replay.attacker
        || captured.blocker != replay.blocker
        || captured.attacker_payment != replay.attacker_payment
    {
        return Err("War-Torch Goblin shuffled-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "one_damage_mutation_detected_in_game": mutation_detected(seed, Mutation::OneDamage)?,
        "activation_color_mutation_detected_in_game": mutation_detected(seed, Mutation::BlueActivation)?,
        "activation_tap_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::TapSource)?,
        "source_sacrifice_mutation_detected_in_game": mutation_detected(seed, Mutation::NoSacrifice)?,
        "blocker_target_mutation_detected_in_game": mutation_detected(seed, Mutation::CreatureTarget)?,
        "red_cast_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongCastCost)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "one_damage_mutation_detected_in_game",
        "activation_color_mutation_detected_in_game",
        "activation_tap_cost_mutation_detected_in_game",
        "source_sacrifice_mutation_detected_in_game",
        "blocker_target_mutation_detected_in_game",
        "red_cast_cost_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "War-Torch Goblin mutation escaped the trace oracle: {key}"
            ));
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
        "card": {"collector_number": 151, "definition_id": CARD_ID, "name": "War-Torch Goblin"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117 Priority",
                "601.2 Casting spells and paying costs",
                "602.1-602.2 Activated abilities, targets, and costs",
                "608.2 Resolving an activated ability",
                "701.17 Sacrifice",
                "510.1-510.2 Combat damage assignment and resolution",
                "704.5g Lethal damage state-based action"
            ],
            "source_contract": "RAV War-Torch Goblin #151, {R} 1/1 creature, and {R}, sacrifice this creature: deal 2 damage to target blocking creature",
            "authority_cutoff": "2026-09-23"
        },
        "passed": true,
        "pilot_card_passed": true,
        "deterministic_replay": true,
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
    OneDamage,
    BlueActivation,
    TapSource,
    NoSacrifice,
    CreatureTarget,
    WrongCastCost,
}

fn candidate_values(mutation: Mutation) -> (CardDefinition, ActivatedAbility) {
    let mut definition = candidate_war_torch_goblin::definition();
    let mut ability = candidate_war_torch_goblin::sacrifice_deal_two_to_blocker_ability();
    match mutation {
        Mutation::None => {}
        Mutation::OneDamage => {
            ability.effects = vec![Effect::DealDamage {
                amount: 1,
                target: TargetRequirement::BlockingCreature,
            }];
        }
        Mutation::BlueActivation => {
            ability.mana_cost = ManaCost::with_colors(0, [Color::Blue]);
        }
        Mutation::TapSource => ability.tap_cost = true,
        Mutation::NoSacrifice => ability.sacrifice_source = false,
        Mutation::CreatureTarget => {
            ability.targets = vec![TargetRequirement::Creature];
            ability.effects = vec![Effect::DealDamage {
                amount: 2,
                target: TargetRequirement::Creature,
            }];
        }
        Mutation::WrongCastCost => definition.mana_cost = ManaCost::with_colors(1, [Color::Red]),
    }
    (definition, ability)
}

fn validate_candidate_runtime() -> Result<(), String> {
    let (definition, ability) = candidate_values(Mutation::None);
    let registered_definition = card_definitions()
        .into_iter()
        .find(|registered| registered.id == CARD_ID)
        .ok_or("War-Torch Goblin definition is missing from the RAV registry")?;
    let registered_ability = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("War-Torch Goblin activated ability is missing from the RAV registry")?;
    if registered_definition != definition || registered_ability.ability != ability {
        return Err(
            "War-Torch Goblin definition or ability is not delegated to its candidate module"
                .to_owned(),
        );
    }
    if definition.id != CARD_ID
        || definition.name != "War-Torch Goblin"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(0, [Color::Red])
        || definition.colors != BTreeSet::from([Color::Red])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.power != Some(1)
        || definition.toughness != Some(1)
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
        || ability.id != ABILITY_ID
        || ability.mana_cost != ManaCost::with_colors(0, [Color::Red])
        || ability.tap_cost
        || ability.sorcery_speed
        || ability.additional_tap_creatures != 0
        || !ability.sacrifice_source
        || ability.sacrifice_creatures != 0
        || ability.sacrifice_lands != 0
        || ability.discard_cards != 0
        || ability.targets != [TargetRequirement::BlockingCreature]
        || ability.effects
            != [Effect::DealDamage {
                amount: 2,
                target: TargetRequirement::BlockingCreature,
            }]
    {
        return Err(
            "War-Torch Goblin identity or activated ability differs from its feature contract"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../sets/ravnica_city_of_guilds/src/cards/war_torch_goblin.rs");
    let source = std::fs::read_to_string(&path)
        .map_err(|error| format!("could not read War-Torch Goblin candidate module: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "War-Torch Goblin module is not valid Rust source".to_owned())?;
    let function_names: BTreeSet<_> = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Fn(function) => Some(function.sig.ident.to_string()),
            _ => None,
        })
        .collect();
    let compact: String = source
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();
    let required_fragments = [
        "id:\"RAV-WAR-TORCH-GOBLIN\"",
        "name:\"War-TorchGoblin\"",
        "mana_cost:ManaCost::with_colors(0,[Color::Red])",
        "power:Some(1)",
        "toughness:Some(1)",
        "id:\"sacrifice-deal-two-to-blocker\"",
        "sacrifice_source:true",
        "targets:vec![TargetRequirement::BlockingCreature]",
        "Effect::DealDamage{amount:2,target:TargetRequirement::BlockingCreature",
    ];
    if function_names
        != BTreeSet::from([
            "definition".to_owned(),
            "sacrifice_deal_two_to_blocker_ability".to_owned(),
        ])
        || required_fragments
            .iter()
            .any(|fragment| !compact.contains(fragment))
    {
        return Err("War-Torch Goblin module is outside its literal source contract".to_owned());
    }
    Ok(())
}

fn find_trace() -> Result<(u64, Captured), String> {
    let requested = std::env::var("CARDBENCH_WAR_TORCH_GOBLIN_SEED")
        .ok()
        .and_then(|value| value.parse::<u64>().ok());
    if let Some(seed) = requested {
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
        "no War-Torch Goblin full-game combat trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Ok(captured) => Ok(validate_trace(&captured, seed).is_err()),
        Err(_) => Ok(true),
    }
}

#[derive(Clone, Debug)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    goblin: Option<ObjectId>,
    attacker: Option<ObjectId>,
    blocker: Option<ObjectId>,
    cast_cost: ManaCost,
    cast_payment: Vec<Color>,
    attacker_payment: Vec<Color>,
    ability: ActivatedAbility,
    ability_payment: Option<ManaCost>,
    source_zone: Option<Zone>,
    attacker_zone: Option<Zone>,
    blocker_zone: Option<Zone>,
    life: [i64; 2],
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let (definition, ability) = candidate_values(mutation);
    let cast_cost = definition.mana_cost.clone();
    let (mut game, _definitions) = new_game(definition, ability.clone())?;
    let p0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: ATTACKER_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 52,
            },
        ],
        sideboard: vec![],
    };
    let p1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: BLOCKER_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-ISLAND".to_owned(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    if [p0.mainboard.as_slice(), p1.mainboard.as_slice()]
        .into_iter()
        .any(|board| {
            board
                .iter()
                .map(|entry| entry.count as usize)
                .sum::<usize>()
                != 60
        })
    {
        return Err("War-Torch Goblin decks must be shuffled legal 60-card lists".to_owned());
    }
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &p0)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &p1)
        .map_err(|error| error.to_string())?;
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
        Box::new(WarTorchPolicy::new(
            PlayerId(0),
            &cast_cost,
            &ability.mana_cost,
            ability.sacrifice_source,
        )),
        Box::new(WarTorchPolicy::new(
            PlayerId(1),
            &ManaCost::with_colors(2, [Color::Blue]),
            &ManaCost::new(0),
            false,
        )),
    ];
    let mut attempted = 0_u32;
    let mut accepted = 0_u32;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("War-Torch Goblin match exceeded its bounded move count".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("policy seat missing")?;
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
                    "War-Torch Goblin policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("War-Torch Goblin invariant failed after move: {error}"))?;
    }
    let events = game.event_log.clone();
    let goblin = events.iter().find_map(|event| match event {
        GameEvent::AbilityActivated {
            player: PlayerId(0),
            source,
            definition: CARD_ID,
            ability: ABILITY_ID,
            ..
        } => Some(*source),
        _ => None,
    });
    if goblin.is_none() {
        let diagnostic: Vec<_> = events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    GameEvent::SpellCast { .. }
                        | GameEvent::CardMoved {
                            to: Zone::Battlefield | Zone::Graveyard,
                            ..
                        }
                        | GameEvent::AttackersDeclared { .. }
                        | GameEvent::BlockersDeclared { .. }
                        | GameEvent::AbilityActivated { .. }
                        | GameEvent::AbilityResolved { .. }
                        | GameEvent::DamageDealtToPermanent { .. }
                )
            })
            .map(|event| format!("{event:?}"))
            .collect();
        return Err(format!(
            "no ability activation; relevant events: {diagnostic:?}"
        ));
    }
    let attacker = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|definition| definition.id == ATTACKER_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let blocker = events.iter().find_map(|event| match event {
        GameEvent::DamageDealtToPermanent {
            source,
            permanent,
            amount: 2,
        } if Some(*source) == goblin => Some(*permanent),
        _ => None,
    });
    if blocker.is_none() && goblin.is_some() {
        let relevant: Vec<_> = events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    GameEvent::BlockersDeclared { .. }
                        | GameEvent::AbilityManaPaid { .. }
                        | GameEvent::SacrificedAsAbilityCost { .. }
                        | GameEvent::AbilityActivated { .. }
                        | GameEvent::AbilityResolved { .. }
                        | GameEvent::DamageDealtToPermanent { .. }
                        | GameEvent::DamagePrevented { .. }
                        | GameEvent::StateBasedAction { .. }
                )
            })
            .map(|event| format!("{event:?}"))
            .collect();
        return Err(format!(
            "ability trace had no two-damage source-target packet: {relevant:?}"
        ));
    }
    let cast_payment = goblin
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
    let attacker_payment = attacker
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
    let ability_payment = goblin.and_then(|source| {
        events.iter().find_map(|event| match event {
            GameEvent::AbilityManaPaid {
                player: PlayerId(0),
                source: paid_source,
                ability: ABILITY_ID,
                mana_cost,
            } if *paid_source == source => Some(mana_cost.clone()),
            _ => None,
        })
    });
    let blocker_zone = blocker.and_then(|target| {
        events.iter().find_map(|event| match event {
            GameEvent::CardMoved {
                card,
                to: Zone::Graveyard,
            } if *card == target => Some(Zone::Graveyard),
            _ => None,
        })
    });
    let life = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    Ok(Captured {
        events,
        terminal: game.winner().map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({}))", seat.0),
        ),
        winner: game.winner().map(|player| player.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        goblin,
        attacker,
        blocker,
        cast_cost,
        cast_payment,
        attacker_payment,
        ability,
        ability_payment,
        source_zone: goblin.and_then(|object| game.zone_of(object)),
        attacker_zone: attacker.and_then(|object| game.zone_of(object)),
        blocker_zone,
        life,
    })
}

fn new_game(
    definition: CardDefinition,
    ability: ActivatedAbility,
) -> Result<(Game, Vec<CardDefinition>), String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|item| item.id == CARD_ID)
        .ok_or("War-Torch Goblin definition missing from registry")? = definition;
    let mut abilities = rav_activated_ability_bindings();
    let binding = abilities
        .iter_mut()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("War-Torch Goblin activated ability binding missing from registry")?;
    binding.ability = ability;
    let game = Game::new_with_all_bindings(
        definitions.clone(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
    )
    .map_err(|error| error.to_string())?;
    Ok((game, definitions))
}

struct WarTorchPolicy {
    player: PlayerId,
    cast_cost: ManaCost,
    activation_cost: ManaCost,
    sacrifice_source: bool,
    source_cast_offered: bool,
    blocker_cast_offered: bool,
    ability_offered: bool,
}

impl WarTorchPolicy {
    fn new(
        player: PlayerId,
        cast_cost: &ManaCost,
        activation_cost: &ManaCost,
        sacrifice_source: bool,
    ) -> Self {
        Self {
            player,
            cast_cost: cast_cost.clone(),
            activation_cost: activation_cost.clone(),
            sacrifice_source,
            source_cast_offered: false,
            blocker_cast_offered: false,
            ability_offered: false,
        }
    }

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let card = if self.player == PlayerId(0) {
            view.hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))?
        } else {
            view.hand
                .iter()
                .find(|card| card.definition == Some("RAV-ISLAND"))?
        };
        Some(PolicyAction::PlayLand { card: card.id })
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

    fn payment_selection(&self, view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let mut available: BTreeMap<Color, u8> = Color::ALL
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
        if let Some(mana_selection) = self.payment_selection(view, cost) {
            if card_id == CARD_ID && self.player == PlayerId(0) {
                self.source_cast_offered = true;
            } else if card_id == BLOCKER_ID {
                self.blocker_cast_offered = true;
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
        let missing_colored = cost
            .colored
            .iter()
            .find(|color| view.mana_pool.amount(**color) == 0)
            .copied();
        let total_required =
            u16::from(cost.generic) + cost.colored.len() as u16 + cost.hybrid.len() as u16;
        let color = missing_colored.or_else(|| {
            (u16::from(view.mana_pool.total()) < total_required)
                .then_some(cost.colored.first().copied().unwrap_or(Color::Red))
        })?;
        self.tap_land(view, color)
    }

    fn activate_blocker_ability(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.player != PlayerId(0)
            || self.ability_offered
            || view.active_player != self.player
            || view.step != Step::DeclareBlockers
            || !view.blockers_declared
            || view.stack_depth != 0
        {
            return None;
        }
        let _attacker = view
            .combat_attackers
            .iter()
            .find(|attacker| attacker.definition == Some(ATTACKER_ID))?;
        let source = view
            .own_battlefield
            .iter()
            .find(|source| source.definition == Some(CARD_ID))?;
        let blocker = view
            .opponent_battlefield
            .iter()
            .find(|blocker| blocker.definition == Some(BLOCKER_ID))?;
        if view.mana_pool.amount(Color::Red) == 0 {
            let needed = self
                .activation_cost
                .colored
                .first()
                .copied()
                .unwrap_or(Color::Red);
            return self.tap_land(view, needed);
        }
        self.ability_offered = true;
        Some(PolicyAction::ActivateAbility {
            activation: AbilityActivation {
                source: source.id,
                ability_id: ABILITY_ID,
                sacrifice_sources: if self.sacrifice_source {
                    vec![source.id]
                } else {
                    vec![]
                },
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![Target::Permanent(blocker.id)],
            },
        })
    }
}

impl CodePolicy for WarTorchPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.war-torch-goblin-blocker-sacrifice.v1"
        } else {
            "rav.terraformer-block-war-torch.v1"
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
            let blocker_ready = view
                .opponent_battlefield
                .iter()
                .any(|card| card.definition == Some(BLOCKER_ID) && card.can_block);
            let attackers = if self.player == PlayerId(0) {
                if blocker_ready && !self.ability_offered {
                    view.own_battlefield
                        .iter()
                        .filter(|card| card.can_attack && card.definition == Some(ATTACKER_ID))
                        .map(|card| card.id)
                        .collect()
                } else {
                    view.own_battlefield
                        .iter()
                        .filter(|card| card.can_attack)
                        .map(|card| card.id)
                        .collect()
                }
            } else {
                Vec::new()
            };
            return PolicyAction::DeclareAttackers { attackers };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            let assignments = if self.player == PlayerId(1) {
                view.combat_attackers
                    .iter()
                    .find(|attacker| attacker.definition == Some(ATTACKER_ID))
                    .and_then(|attacker| {
                        view.own_battlefield
                            .iter()
                            .find(|blocker| {
                                blocker.definition == Some(BLOCKER_ID) && blocker.can_block
                            })
                            .map(|blocker| cardbench_magic_engine::CombatBlock {
                                attacker: attacker.id,
                                blocker: blocker.id,
                            })
                    })
                    .into_iter()
                    .collect()
            } else {
                Vec::new()
            };
            return PolicyAction::DeclareBlockers { assignments };
        }
        if view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.activate_blocker_ability(view) {
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
            if !self.blocker_cast_offered {
                if let Some(action) =
                    self.cast_action(view, BLOCKER_ID, &ManaCost::with_colors(2, [Color::Blue]))
                {
                    return action;
                }
            }
            return PolicyAction::PassPriority;
        }
        if !self.source_cast_offered {
            if let Some(action) = self.cast_action(view, CARD_ID, &self.cast_cost.clone()) {
                return action;
            }
            return PolicyAction::PassPriority;
        }
        if !self.ability_offered {
            let has_source = view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(CARD_ID));
            let available = view.mana_pool.total()
                + u8::try_from(
                    view.own_battlefield
                        .iter()
                        .filter(|card| {
                            !card.tapped
                                && card.card_types.contains(&CardType::Land)
                                && card.mana_colors.contains(&Color::Red)
                        })
                        .count(),
                )
                .unwrap_or(u8::MAX);
            if has_source
                && available >= 5
                && view
                    .hand
                    .iter()
                    .any(|card| card.definition == Some(ATTACKER_ID))
                && let Some(action) =
                    self.cast_action(view, ATTACKER_ID, &ManaCost::with_colors(3, [Color::Red]))
            {
                return action;
            }
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.cast_action(view, CARD_ID, &self.cast_cost.clone()) {
            return action;
        }
        if let Some(action) =
            self.cast_action(view, ATTACKER_ID, &ManaCost::with_colors(3, [Color::Red]))
        {
            return action;
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))"
        || captured.winner != Some(0)
        || captured.rejected != 0
    {
        return Err("War-Torch Goblin game did not reach a clean seat-zero win".to_owned());
    }
    let goblin = captured
        .goblin
        .ok_or("seat zero did not activate a cast War-Torch Goblin")?;
    let attacker = captured
        .attacker
        .ok_or("seat zero did not cast Goblin Fire Fiend")?;
    let blocker = captured
        .blocker
        .ok_or("the ability did not damage a live blocker")?;
    if captured.cast_cost != ManaCost::with_colors(0, [Color::Red])
        || captured.cast_payment != [Color::Red]
    {
        return Err(format!(
            "War-Torch Goblin did not pay exactly {{R}}: {:?}",
            captured.cast_payment
        ));
    }
    if captured.attacker_payment.len() != 4
        || captured
            .attacker_payment
            .iter()
            .any(|color| *color != Color::Red)
        || captured.attacker_zone != Some(Zone::Battlefield)
    {
        return Err(format!(
            "the haste attacker did not resolve as a paid 1/1: payment={:?} zone={:?}",
            captured.attacker_payment, captured.attacker_zone
        ));
    }
    if captured.ability.id != ABILITY_ID
        || captured.ability.mana_cost != ManaCost::with_colors(0, [Color::Red])
        || captured.ability.tap_cost
        || captured.ability.sorcery_speed
        || !captured.ability.sacrifice_source
        || captured.ability.targets != [TargetRequirement::BlockingCreature]
        || captured.ability.effects
            != [Effect::DealDamage {
                amount: 2,
                target: TargetRequirement::BlockingCreature,
            }]
        || captured.ability_payment.as_ref() != Some(&ManaCost::with_colors(0, [Color::Red]))
    {
        return Err(
            "War-Torch Goblin's exact red sacrifice-and-damage activation was not paid".to_owned(),
        );
    }
    if captured.source_zone != Some(Zone::Graveyard)
        || captured.blocker_zone != Some(Zone::Graveyard)
        || captured.life[1] > 0
    {
        return Err(format!(
            "ability or terminal state is wrong: goblin={:?}, blocker={:?}, life={:?}",
            captured.source_zone, captured.blocker_zone, captured.life
        ));
    }
    let events = &captured.events;
    let cast_payment = find_event(events, |event| {
        matches!(event,
        GameEvent::SpellManaPaid { player: PlayerId(0), card, colors }
            if *card == goblin && colors == &[Color::Red])
    })
    .ok_or("exact red War-Torch Goblin casting receipt is missing")?;
    let cast = find_event_after(events, cast_payment, |event| {
        matches!(event,
        GameEvent::SpellCast { player: PlayerId(0), card } if *card == goblin)
    })
    .ok_or("War-Torch Goblin SpellCast receipt is missing")?;
    let resolved = find_event_after(events, cast, |event| {
        matches!(event,
        GameEvent::SpellResolved { card } if *card == goblin)
    })
    .ok_or("War-Torch Goblin SpellResolved receipt is missing")?;
    let entered = find_event_after(events, resolved, |event| {
        matches!(event,
        GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == goblin)
    })
    .ok_or("War-Torch Goblin did not enter the battlefield")?;
    let blocker_cast = find_event(events, |event| {
        matches!(event,
        GameEvent::SpellCast { player: PlayerId(1), card } if *card == blocker)
    })
    .ok_or("opponent did not cast a real blocker")?;
    let blocker_entered = find_event_after(events, blocker_cast, |event| {
        matches!(event,
        GameEvent::CardMoved { card: moved, to: Zone::Battlefield } if *moved == blocker)
    })
    .ok_or("the targeted blocker did not enter the battlefield")?;
    let attacker_cast_payment = find_event(events, |event| matches!(event,
        GameEvent::SpellManaPaid { player: PlayerId(0), card, colors }
            if *card == attacker && colors.len() == 4 && colors.iter().all(|color| *color == Color::Red)))
        .ok_or("exact Goblin Fire Fiend mana payment is missing")?;
    let attacker_cast = find_event_after(events, attacker_cast_payment, |event| {
        matches!(event,
        GameEvent::SpellCast { player: PlayerId(0), card } if *card == attacker)
    })
    .ok_or("Goblin Fire Fiend SpellCast receipt is missing")?;
    let attacker_resolved = find_event_after(events, attacker_cast, |event| {
        matches!(event,
        GameEvent::SpellResolved { card } if *card == attacker)
    })
    .ok_or("Goblin Fire Fiend SpellResolved receipt is missing")?;
    let attacker_entered = find_event_after(events, attacker_resolved, |event| {
        matches!(event,
        GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == attacker)
    })
    .ok_or("Goblin Fire Fiend did not enter the battlefield")?;
    let attack = find_event(events, |event| matches!(event,
        GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&attacker)))
        .ok_or("the hasty Goblin Fire Fiend did not attack")?;
    let block = find_event_after(events, attack, |event| {
        matches!(event,
        GameEvent::BlockersDeclared { player: PlayerId(1), assignments }
            if assignments.contains(&(attacker, blocker)))
    })
    .ok_or("the live Terraformer did not block Goblin Fire Fiend")?;
    let red_mana = find_event_after(events, block, |event| {
        matches!(
            event,
            GameEvent::ManaAbilityActivated {
                player: PlayerId(0),
                color: Color::Red,
                ..
            }
        )
    })
    .ok_or("seat zero did not produce red mana after the block declaration")?;
    let ability_paid = find_event_after(events, red_mana, |event| {
        matches!(event,
        GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: ABILITY_ID, mana_cost }
            if *source == goblin && mana_cost == &ManaCost::with_colors(0, [Color::Red]))
    })
    .ok_or("the red activated-ability payment receipt is missing")?;
    let sacrificed = find_event_after(events, ability_paid, |event| {
        matches!(event,
        GameEvent::SacrificedAsAbilityCost { player: PlayerId(0), source, permanent }
            if *source == goblin && *permanent == goblin)
    })
    .ok_or("War-Torch Goblin was not sacrificed as the ability's activation cost")?;
    let source_died = find_event_after(events, sacrificed, |event| {
        matches!(event,
        GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == goblin)
    })
    .ok_or("sacrificed War-Torch Goblin did not move to its graveyard")?;
    let activated = find_event_after(events, source_died, |event| matches!(event,
        GameEvent::AbilityActivated { player: PlayerId(0), source, definition: CARD_ID, ability: ABILITY_ID, .. }
            if *source == goblin))
        .ok_or("candidate-owned War-Torch Goblin ability activation receipt is missing")?;
    let damage = find_event_after(events, activated, |event| {
        matches!(event,
        GameEvent::DamageDealtToPermanent { source, permanent, amount: 2 }
            if *source == goblin && *permanent == blocker)
    })
    .ok_or("resolving ability did not deal exactly 2 damage to its declared blocker")?;
    let ability_resolved = find_event_after(events, damage, |event| {
        matches!(event,
        GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == goblin)
    })
    .ok_or("War-Torch Goblin activated ability did not resolve")?;
    let lethal = find_event_after(events, damage, |event| matches!(event,
        GameEvent::StateBasedAction { card, reason: "creature has lethal damage" } if *card == blocker))
        .ok_or("the damage did not make the 2/2 blocker lethal")?;
    let blocker_died = find_event_after(events, lethal, |event| {
        matches!(event,
        GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == blocker)
    })
    .ok_or("lethally damaged blocker did not move to its graveyard")?;
    let later_attack = find_event_after(events, blocker_died, |event| {
        matches!(event,
        GameEvent::AttackersDeclared { player: PlayerId(0), attackers }
            if attackers.contains(&attacker))
    })
    .ok_or("Goblin Fire Fiend did not attack after the blocker died")?;
    let later_damage = find_event_after(events, later_attack, |event| {
        matches!(event,
        GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 1 }
            if *source == attacker)
    })
    .ok_or("the later Fire Fiend attack did not damage the opponent")?;
    let ended = find_event_after(events, later_damage, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("seat zero's terminal win is missing")?;
    if !(cast_payment < cast
        && cast < resolved
        && resolved < entered
        && blocker_cast < blocker_entered
        && entered < attacker_cast_payment
        && attacker_cast_payment < attacker_cast
        && attacker_cast < attacker_resolved
        && attacker_resolved < attacker_entered
        && blocker_entered < attack
        && attacker_entered < attack
        && attack < block
        && block < red_mana
        && red_mana < ability_paid
        && ability_paid < sacrificed
        && sacrificed < source_died
        && source_died < activated
        && activated < damage
        && damage < ability_resolved
        && ability_resolved < lethal
        && lethal < blocker_died
        && blocker_died < later_attack
        && later_attack < later_damage
        && later_damage < ended)
    {
        return Err(
            "War-Torch Goblin payment, cost, resolution, and combat evidence is out of order"
                .to_owned(),
        );
    }
    if events.iter().any(
        |event| matches!(event, GameEvent::DamageDealtToPlayer { source, .. } if *source == goblin),
    ) {
        return Err("sacrificed War-Torch Goblin dealt later combat damage".to_owned());
    }
    let selected = BTreeSet::from([
        cast_payment,
        cast,
        resolved,
        entered,
        attacker_cast_payment,
        attacker_cast,
        attacker_resolved,
        attacker_entered,
        blocker_cast,
        blocker_entered,
        attack,
        block,
        red_mana,
        ability_paid,
        sacrificed,
        source_died,
        activated,
        ability_resolved,
        damage,
        lethal,
        blocker_died,
        later_attack,
        later_damage,
        ended,
    ]);
    let projected: Vec<ProjectedEvent> = project_events(events)
        .into_iter()
        .enumerate()
        .filter_map(|(index, event)| selected.contains(&index).then_some(event))
        .collect();
    let event_kinds: Vec<_> = projected.iter().map(|event| event.kind.as_str()).collect();
    for required in [
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "AbilityManaPaid",
        "AbilityActivated",
        "SacrificedAsAbilityCost",
        "AbilityResolved",
        "DamageDealtToPermanent",
        "BlockersDeclared",
        "StateBasedAction",
        "GameEnded",
    ] {
        if !event_kinds.contains(&required) {
            return Err(format!("projected War-Torch trace omitted {required}"));
        }
    }
    Ok(json!({
        "trace_id": "rav-war-torch-goblin-blocker-sacrifice-pvp",
        "shuffle_seed": seed,
        "decks": ["rav_war_torch_goblin_mountain", "rav_terraformer_island"],
        "policies": ["rav.war-torch-goblin-blocker-sacrifice.v1", "rav.terraformer-block-war-torch.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted,
        "rejected_moves": captured.rejected,
        "war_torch_goblin": {"object": goblin.0, "collector_number": 151, "mana_cost": "{R}", "mana_paid": captured.cast_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "zone_after_cost": "Graveyard"},
        "attacker": {"object": attacker.0, "definition": ATTACKER_ID, "mana_cost": "{3}{R}", "mana_paid": captured.attacker_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "haste_attack_same_turn": true, "zone_at_terminal": captured.attacker_zone.map(|zone| format!("{zone:?}"))},
        "blocker": {"object": blocker.0, "definition": BLOCKER_ID, "blocked_attacker": attacker.0, "damage": 2, "zone_after_damage": "Graveyard"},
        "activation": {"ability": ABILITY_ID, "source": goblin.0, "mana_cost": "{R}", "red_mana_paid": true, "source_sacrificed_as_cost": true, "effect_damage_and_resolution_before_combat_damage": true},
        "combat": {"blocker_died_before_combat_damage": true, "fire_fiend_later_attacked": true, "opponent_life_at_terminal": captured.life[1], "life_at_terminal": captured.life},
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

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn card_status(card: &CatalogCard) -> Value {
    let status = if card.semantic_status.is_executable() {
        "executable_compatibility_slice"
    } else {
        "catalog_only"
    };
    let definition_id = card_definitions()
        .into_iter()
        .find(|definition| definition.name == card.name)
        .map(|definition| definition.id)
        .unwrap_or("");
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"engine_status":status,"trace_status":if definition_id==CARD_ID{"passed"}else{"not_qualified"},"qualification":if definition_id==CARD_ID{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
}
