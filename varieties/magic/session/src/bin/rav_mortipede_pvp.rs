//! Full-game RAV Mortipede activated must-be-blocked trace authority.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, CardDefinition, CardType, CastRequest, Color, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, Step, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::{ObjectRef, ViewerScope};
use cardbench_magic_rav::{
    RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-MORTIPEDE";
const BLOCKER_ID: &str = "RAV-LURKING-INFORMANT";
const SUPPORT_ID: &str = "RAV-WATCHWOLF";
const ABILITY_ID: &str = "green-must-be-blocked";
const MAX_TRACE_SEED: u64 = 256;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/mortipede.rs"]
mod candidate_mortipede;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Mortipede PvP result: {error}");
                    return ExitCode::FAILURE;
                }
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
            eprintln!("Mortipede PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn output_root() -> Option<std::path::PathBuf> {
    std::env::args_os()
        .collect::<Vec<_>>()
        .windows(2)
        .find(|pair| pair[0] == "--output-root")
        .map(|pair| pair[1].clone().into())
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

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

#[derive(Clone, Copy, Debug)]
enum Mutation {
    None,
    RemoveMustBlock,
    WrongAbilityCost,
    WrongCastCost,
    WrongPower,
    WrongToughness,
}

fn candidate_values(mutation: Mutation) -> (CardDefinition, ActivatedAbility) {
    let mut definition = candidate_mortipede::definition();
    let mut ability = candidate_mortipede::activated_ability().ability;
    match mutation {
        Mutation::None => {}
        Mutation::RemoveMustBlock => ability.effects.clear(),
        Mutation::WrongAbilityCost => ability.mana_cost = ManaCost::with_colors(1, [Color::Green]),
        Mutation::WrongCastCost => definition.mana_cost = ManaCost::with_colors(2, [Color::Black]),
        Mutation::WrongPower => definition.power = Some(3),
        Mutation::WrongToughness => definition.toughness = Some(2),
    }
    (definition, ability)
}

fn validate_candidate_source() -> Result<(), String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../sets/ravnica_city_of_guilds/src/cards/mortipede.rs");
    let source = std::fs::read_to_string(&path)
        .map_err(|error| format!("could not read Mortipede candidate module: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Mortipede module is not valid Rust source".to_owned())?;
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
    let required = [
        "id:\"RAV-MORTIPEDE\"",
        "name:\"Mortipede\"",
        "mana_cost:ManaCost::with_colors(3,[Color::Black])",
        "power:Some(4)",
        "toughness:Some(1)",
        "id:\"green-must-be-blocked\"",
        "mana_cost:ManaCost::with_colors(2,[Color::Green])",
        "Effect::AddSourceKeywordUntilEndOfTurn{keyword:Keyword::MustBeBlockedIfAble,}",
    ];
    if function_names != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()])
        || required.iter().any(|fragment| !compact.contains(fragment))
    {
        return Err(
            "Mortipede candidate definition or ability is outside its literal source contract"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_runtime() -> Result<(), String> {
    let (definition, ability) = candidate_values(Mutation::None);
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Mortipede definition is missing from the RAV registry")?;
    let binding = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("Mortipede activated ability is missing from the RAV registry")?;
    if registered != definition || binding.ability != ability {
        return Err(
            "Mortipede definition and ability are not delegated to its candidate module".to_owned(),
        );
    }
    if definition.id != CARD_ID
        || definition.name != "Mortipede"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(3, [Color::Black])
        || definition.colors != BTreeSet::from([Color::Black])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.power != Some(4)
        || definition.toughness != Some(1)
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
        || ability.id != ABILITY_ID
        || ability.mana_cost != ManaCost::with_colors(2, [Color::Green])
        || ability.tap_cost
        || ability.sorcery_speed
        || ability.additional_tap_creatures != 0
        || ability.sacrifice_source
        || ability.sacrifice_creatures != 0
        || ability.sacrifice_lands != 0
        || ability.discard_cards != 0
        || !ability.targets.is_empty()
        || ability.effects
            != [Effect::AddSourceKeywordUntilEndOfTurn {
                keyword: Keyword::MustBeBlockedIfAble,
            }]
    {
        return Err(
            "Mortipede identity or activated must-be-blocked feature differs from RAV #96"
                .to_owned(),
        );
    }
    Ok(())
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
        || captured.mortipede != replay.mortipede
        || captured.blocker != replay.blocker
        || captured.cast_payment != replay.cast_payment
        || captured.activation_payment != replay.activation_payment
    {
        return Err("Mortipede shuffled-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "must_be_blocked_effect_removal_detected_in_game": mutation_detected(seed, Mutation::RemoveMustBlock)?,
        "activation_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongAbilityCost)?,
        "cast_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongCastCost)?,
        "base_power_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongPower)?,
        "base_toughness_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongToughness)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "must_be_blocked_effect_removal_detected_in_game",
        "activation_cost_mutation_detected_in_game",
        "cast_cost_mutation_detected_in_game",
        "base_power_mutation_detected_in_game",
        "base_toughness_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Mortipede semantic mutation escaped the PvP trace: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(|card| {
        let definition_id = card_definitions().into_iter().find(|definition| definition.name == card.name)
            .map(|definition| definition.id).unwrap_or("");
        json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,
            "engine_status":if card.semantic_status.is_executable(){"executable_compatibility_slice"}else{"catalog_only"},
            "trace_status":if definition_id==CARD_ID{"passed"}else{"not_qualified"},
            "qualification":if definition_id==CARD_ID{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
    }).collect();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1","task_id":TASK_ID,"expansion":"RAV",
        "card":{"collector_number":96,"definition_id":CARD_ID,"name":"Mortipede"},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},"candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{"comprehensive_rules":"2026-08-07","checked_sections":["117 Priority","509.1-509.2 Blocking requirements","510.1-510.2 Combat damage","601.2 Casting and paying","602.1-602.2 Activated abilities","608.2 Ability resolution","611.2 Until-end-of-turn continuous effects","704 State-based actions"],
            "source_contract":"RAV Mortipede #96, {3}{B} 4/1 black creature; {2}{G}: this creature must be blocked if able until end of turn",
            "authority_cutoff":"2026-09-23","no_provider_trial":true},
        "passed":true,"pilot_card_passed":true,"cards_passed":1,"cards_credited":0,"cards_total":catalog.len(),
        "printings_total":printings.len(),"coverage_complete":false,"full_reward":false,"harbor_reward":0.0,"pilot_reward":0.0,
        "qualification_status":"unreviewed_diagnostic","mutation_sensitivity":mutations,"traces":[trace],"per_card_breakdown":breakdown
    }))
}

#[derive(Clone, Debug)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    mortipede: Option<ObjectId>,
    blocker: Option<ObjectId>,
    cast_payment: Vec<Color>,
    activation_payment: Option<ManaCost>,
    cast_cost: ManaCost,
    definition: CardDefinition,
    ability: ActivatedAbility,
    support_objects: Vec<ObjectId>,
    mortipede_zone: Option<Zone>,
    blocker_zone: Option<Zone>,
    life: [i64; 2],
}

fn find_trace() -> Result<(u64, Captured), String> {
    let requested = std::env::var("CARDBENCH_MORTIPEDE_SEED")
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
        "no Mortipede full-game trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Ok(captured) => Ok(validate_trace(&captured, seed).is_err()),
        Err(_) => Ok(true),
    }
}

fn new_game(definition: CardDefinition, ability: ActivatedAbility) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|item| item.id == CARD_ID)
        .ok_or("Mortipede definition missing from registry")? = definition;
    let mut abilities = rav_activated_ability_bindings();
    let binding = abilities
        .iter_mut()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("Mortipede ability missing from registry")?;
    binding.ability = ability;
    Game::new_with_all_bindings(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
    )
    .map_err(|error| error.to_string())
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let (definition, ability) = candidate_values(mutation);
    let cast_cost = definition.mana_cost.clone();
    let mut game = new_game(definition.clone(), ability.clone())?;
    let p0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: SUPPORT_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-SWAMP".into(),
                count: 16,
            },
            DeckEntry {
                card: "RAV-FOREST".into(),
                count: 18,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 18,
            },
        ],
        sideboard: vec![],
    };
    let p1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: BLOCKER_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-ISLAND".into(),
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
        return Err("Mortipede decks are not legal 60-card lists".into());
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
    let blocker_cost = card_definitions()
        .into_iter()
        .find(|card| card.id == BLOCKER_ID)
        .ok_or("Lurking Informant definition missing")?
        .mana_cost;
    let support_cost = card_definitions()
        .into_iter()
        .find(|card| card.id == SUPPORT_ID)
        .ok_or("Watchwolf definition missing")?
        .mana_cost;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(MortipedePolicy::new(
            PlayerId(0),
            &cast_cost,
            &ability.mana_cost,
            &support_cost,
        )),
        Box::new(MortipedePolicy::new(
            PlayerId(1),
            &blocker_cost,
            &ManaCost::new(0),
            &support_cost,
        )),
    ];
    let (mut accepted, mut attempted) = (0u32, 0u32);
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Mortipede match exceeded bounded move count".into());
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
                    "Mortipede policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("Mortipede game invariant failed: {error}"))?;
    }
    let events = game.event_log.clone();
    let mortipede = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|card| card.id == CARD_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let blocker = mortipede.and_then(|attacker| {
        events.iter().find_map(|event| match event {
            GameEvent::BlockersDeclared {
                player: PlayerId(1),
                assignments,
            } => assignments
                .iter()
                .find_map(|(declared, blocker)| (*declared == attacker).then_some(*blocker)),
            _ => None,
        })
    });
    let cast_payment = mortipede
        .and_then(|card| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card: paid,
                    colors,
                } if *paid == card => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    let activation_payment = mortipede.and_then(|card| {
        events.iter().find_map(|event| match event {
            GameEvent::AbilityManaPaid {
                player: PlayerId(0),
                source,
                ability: ABILITY_ID,
                mana_cost,
            } if *source == card => Some(mana_cost.clone()),
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
    let mortipede_zone = mortipede.and_then(|object| {
        events.iter().any(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == object))
            .then_some(Zone::Graveyard)
            .or_else(|| game.zone_of(object))
    });
    let blocker_zone = blocker.and_then(|object| {
        events.iter().any(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == object))
            .then_some(Zone::Graveyard)
            .or_else(|| game.zone_of(object))
    });
    let support_objects = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == SUPPORT_ID) =>
            {
                Some(*card)
            }
            _ => None,
        })
        .collect();
    Ok(Captured {
        events,
        terminal: game.winner().map_or_else(
            || "Draw".into(),
            |seat| format!("Winner(PlayerId({}))", seat.0),
        ),
        winner: game.winner().map(|seat| seat.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        mortipede,
        blocker,
        cast_payment,
        activation_payment,
        cast_cost,
        definition,
        ability,
        support_objects,
        mortipede_zone,
        blocker_zone,
        life,
    })
}

struct MortipedePolicy {
    player: PlayerId,
    cast_cost: ManaCost,
    activation_cost: ManaCost,
    support_cost: ManaCost,
    activation_offered_turn: Option<u32>,
}

impl MortipedePolicy {
    fn new(
        player: PlayerId,
        cast_cost: &ManaCost,
        activation_cost: &ManaCost,
        support_cost: &ManaCost,
    ) -> Self {
        Self {
            player,
            cast_cost: cast_cost.clone(),
            activation_cost: activation_cost.clone(),
            support_cost: support_cost.clone(),
            activation_offered_turn: None,
        }
    }

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let land = view
            .hand
            .iter()
            .find(|card| card.card_types.contains(&CardType::Land))?;
        Some(PolicyAction::PlayLand { card: land.id })
    }

    fn tap_for(&self, view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
        let need = cost
            .colored
            .iter()
            .find(|color| view.mana_pool.amount(**color) == 0)
            .copied()
            .or_else(|| {
                cost.hybrid
                    .iter()
                    .find(|symbol| {
                        view.mana_pool.amount(symbol.first) == 0
                            && view.mana_pool.amount(symbol.second) == 0
                    })
                    .map(|symbol| symbol.first)
            })
            .or_else(|| {
                (view.mana_pool.total()
                    < cost
                        .generic
                        .saturating_add(cost.colored.len() as u8)
                        .saturating_add(cost.hybrid.len() as u8))
                .then_some(Color::Green)
            });
        let preferred = need?;
        let land = view
            .own_battlefield
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land)
                    && !card.tapped
                    && card.mana_colors.contains(&preferred)
            })
            .or_else(|| {
                view.own_battlefield.iter().find(|card| {
                    card.card_types.contains(&CardType::Land)
                        && !card.tapped
                        && !card.mana_colors.is_empty()
                })
            })?;
        let color = if land.mana_colors.contains(&preferred) {
            preferred
        } else {
            *land.mana_colors.iter().next()?
        };
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }

    fn make_cast(&self, view: &GameView, card_id: &str, cost: &ManaCost) -> Option<PolicyAction> {
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(card_id))?;
        if let Some(mana_selection) = payment(view, cost) {
            Some(PolicyAction::CastWithPayment {
                request: CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection,
            })
        } else {
            self.tap_for(view, cost)
        }
    }

    fn declare_blocks(&self, view: &GameView) -> PolicyAction {
        let attackers: Vec<_> = view
            .combat_attackers
            .iter()
            .filter(|attacker| {
                attacker.definition == Some(CARD_ID)
                    && attacker.keywords.contains(&Keyword::MustBeBlockedIfAble)
            })
            .collect();
        let blockers: Vec<_> = view
            .own_battlefield
            .iter()
            .filter(|card| card.can_block)
            .collect();
        let assignments = attackers
            .into_iter()
            .zip(blockers)
            .map(|(attacker, blocker)| cardbench_magic_engine::CombatBlock {
                attacker: attacker.id,
                blocker: blocker.id,
            })
            .collect();
        PolicyAction::DeclareBlockers { assignments }
    }

    fn activate_must_block(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.player != PlayerId(0)
            || view.active_player != self.player
            || view.priority != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
            || self.activation_offered_turn == Some(view.turn)
        {
            return None;
        }
        let attacker = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some(CARD_ID) && card.can_attack)?;
        if payment(view, &self.activation_cost).is_some() {
            self.activation_offered_turn = Some(view.turn);
            Some(PolicyAction::ActivateAbility {
                activation: AbilityActivation {
                    source: attacker.id,
                    ability_id: ABILITY_ID,
                    sacrifice_sources: vec![],
                    additional_tap_creatures: vec![],
                    discard_cards: vec![],
                    targets: vec![],
                },
            })
        } else {
            self.tap_for(view, &self.activation_cost)
        }
    }
}

impl CodePolicy for MortipedePolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.mortipede-must-be-blocked.v1"
        } else {
            "rav.lurking-informant-blocks-required-attacker.v1"
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
            return if self.player == PlayerId(1) {
                self.declare_blocks(view)
            } else {
                PolicyAction::DeclareBlockers {
                    assignments: vec![],
                }
            };
        }
        if view.priority != self.player
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.play_land(view) {
            return action;
        }
        if self.player == PlayerId(0) {
            if let Some(action) = self.activate_must_block(view) {
                return action;
            }
            let already_has_mortipede = view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(CARD_ID));
            if !already_has_mortipede {
                if let Some(action) = self.make_cast(view, CARD_ID, &self.cast_cost) {
                    return action;
                }
            }
            if let Some(action) = self.make_cast(view, SUPPORT_ID, &self.support_cost) {
                return action;
            }
        } else {
            let blockers = view
                .own_battlefield
                .iter()
                .filter(|card| card.definition == Some(BLOCKER_ID))
                .count();
            if blockers < 4 {
                if let Some(action) = self.make_cast(view, BLOCKER_ID, &self.cast_cost) {
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
        let (_, amount) = available
            .iter_mut()
            .find(|(candidate, _)| candidate == color)?;
        if *amount == 0 {
            return None;
        }
        *amount -= 1;
    }
    let mut hybrid = Vec::new();
    for symbol in &cost.hybrid {
        let color = if available
            .iter()
            .any(|(candidate, amount)| *candidate == symbol.first && *amount > 0)
        {
            symbol.first
        } else if available
            .iter()
            .any(|(candidate, amount)| *candidate == symbol.second && *amount > 0)
        {
            symbol.second
        } else {
            return None;
        };
        let (_, amount) = available
            .iter_mut()
            .find(|(candidate, _)| *candidate == color)?;
        *amount -= 1;
        hybrid.push(color);
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
    Some(ManaPaymentSelection { generic, hybrid })
}

fn validate_trace(c: &Captured, seed: u64) -> Result<Value, String> {
    if c.terminal != "Winner(PlayerId(0))" || c.winner != Some(0) || c.rejected != 0 {
        return Err("Mortipede match did not reach a clean seat-zero terminal win".into());
    }
    let mortipede = c.mortipede.ok_or("seat zero did not cast Mortipede")?;
    let blocker = c.blocker.ok_or(
        "Mortipede was not blocked by an available Lurking Informant after its activation",
    )?;
    if c.cast_cost != ManaCost::with_colors(3, [Color::Black])
        || c.cast_payment.len() != 4
        || !c.cast_payment.contains(&Color::Black)
    {
        return Err(format!(
            "Mortipede did not pay the printed {{3}}{{B}} cost: {:?}",
            c.cast_payment
        ));
    }
    if c.definition.id != CARD_ID
        || c.definition.name != "Mortipede"
        || c.definition.mana_cost != ManaCost::with_colors(3, [Color::Black])
        || c.definition.colors != BTreeSet::from([Color::Black])
        || c.definition.power != Some(4)
        || c.definition.toughness != Some(1)
        || c.ability.id != ABILITY_ID
        || c.ability.mana_cost != ManaCost::with_colors(2, [Color::Green])
        || c.ability.tap_cost
        || c.ability.sorcery_speed
        || c.ability.sacrifice_source
        || c.ability.sacrifice_creatures != 0
        || c.ability.sacrifice_lands != 0
        || c.ability.discard_cards != 0
        || !c.ability.targets.is_empty()
        || c.ability.effects
            != [Effect::AddSourceKeywordUntilEndOfTurn {
                keyword: Keyword::MustBeBlockedIfAble,
            }]
        || c.activation_payment.as_ref() != Some(&ManaCost::with_colors(2, [Color::Green]))
    {
        return Err(format!(
            "Mortipede's exact 4/1 identity or {{2}}{{G}} must-be-blocked ability was not observed: {:?}, {:?}",
            c.definition, c.ability
        ));
    }
    if c.mortipede_zone != Some(Zone::Graveyard) || c.blocker_zone != Some(Zone::Graveyard) {
        let combat: Vec<_> = c
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    GameEvent::BlockersDeclared { .. }
                        | GameEvent::DamageDealtToPermanent { .. }
                        | GameEvent::StateBasedAction { .. }
                        | GameEvent::CardMoved {
                            to: Zone::Graveyard,
                            ..
                        }
                )
            })
            .map(|event| format!("{event:?}"))
            .collect();
        return Err(format!(
            "Mortipede and its required 1/2 blocker did not both die in combat: {:?} {:?}; events={combat:?}",
            c.mortipede_zone, c.blocker_zone,
        ));
    }
    let e = &c.events;
    let paid=find_event(e,|event|matches!(event,GameEvent::SpellManaPaid{player:PlayerId(0),card,colors}if *card==mortipede&&colors.len()==4&&colors.contains(&Color::Black))).ok_or("Mortipede spell payment receipt missing")?;
    let cast = find_event_after(
        e,
        paid,
        |event| matches!(event,GameEvent::SpellCast{player:PlayerId(0),card}if *card==mortipede),
    )
    .ok_or("Mortipede cast event missing")?;
    let resolved = find_event_after(
        e,
        cast,
        |event| matches!(event,GameEvent::SpellResolved{card}if *card==mortipede),
    )
    .ok_or("Mortipede resolution missing")?;
    let entered = find_event_after(
        e,
        resolved,
        |event| matches!(event,GameEvent::CardMoved{card,to:Zone::Battlefield}if *card==mortipede),
    )
    .ok_or("Mortipede battlefield entry missing")?;
    let ability_paid=find_event(e,|event|matches!(event,GameEvent::AbilityManaPaid{player:PlayerId(0),source,ability:ABILITY_ID,mana_cost}if *source==mortipede&&mana_cost==&ManaCost::with_colors(2,[Color::Green]))).ok_or("Mortipede exact green activation payment missing")?;
    let activated=find_event_after(e,ability_paid,|event|matches!(event,GameEvent::AbilityActivated{player:PlayerId(0),source,definition:CARD_ID,ability:ABILITY_ID,..}if *source==mortipede)).ok_or("Mortipede ability activation event missing")?;
    let ability_resolved=find_event_after(e,activated,|event|matches!(event,GameEvent::AbilityResolved{source,ability:ABILITY_ID,..}if *source==mortipede)).ok_or("Mortipede must-be-blocked effect did not resolve")?;
    let attack=find_event_after(e,ability_resolved,|event|matches!(event,GameEvent::AttackersDeclared{player:PlayerId(0),attackers}if attackers.contains(&mortipede))).ok_or("Mortipede did not attack after its must-be-blocked effect resolved")?;
    let block=find_event_after(e,attack,|event|matches!(event,GameEvent::BlockersDeclared{player:PlayerId(1),assignments}if assignments.contains(&(mortipede,blocker)))).ok_or("opponent did not assign an available blocker to Mortipede")?;
    let attack_damage=find_event_after(e,block,|event|matches!(event,GameEvent::DamageDealtToPermanent{source,permanent,amount:4}if *source==mortipede&&*permanent==blocker)).ok_or("Mortipede did not deal its 4 combat damage to its required blocker")?;
    let block_damage=find_event_after(e,attack_damage,|event|matches!(event,GameEvent::DamageDealtToPermanent{source,permanent,amount:1}if *source==blocker&&*permanent==mortipede)).ok_or("required blocker did not deal combat damage back to Mortipede")?;
    let end = find_event(e, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("full-game seat-zero winner receipt missing")?;
    let mortipede_died = find_event_after(
        e,
        block_damage,
        |event| matches!(event,GameEvent::CardMoved{card,to:Zone::Graveyard}if *card==mortipede),
    )
    .ok_or("Mortipede did not die after the required block")?;
    let blocker_died = find_event_after(
        e,
        attack_damage,
        |event| matches!(event,GameEvent::CardMoved{card,to:Zone::Graveyard}if *card==blocker),
    )
    .ok_or("required blocker did not die to Mortipede combat damage")?;
    let support_damage: i64 = e
        .iter()
        .filter_map(|event| match event {
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if c.support_objects.contains(source) => Some(i64::from(*amount)),
            _ => None,
        })
        .sum();
    let later_damage=find_event_after(e,mortipede_died,|event|matches!(event,GameEvent::DamageDealtToPlayer{player:PlayerId(1),amount,..}if *amount>0)).ok_or("later support combat damage missing")?;
    if support_damage < 20
        || !(paid < cast
            && cast < resolved
            && resolved < entered
            && entered < ability_paid
            && ability_paid < activated
            && activated < ability_resolved
            && ability_resolved < attack
            && attack < block
            && block < attack_damage
            && attack_damage < block_damage
            && block_damage < mortipede_died
            && attack_damage < blocker_died
            && mortipede_died < later_damage
            && later_damage < end)
    {
        return Err(format!(
            "Mortipede cast, green activation, required block, death, and complete game did not follow the required sequence (support damage {support_damage})"
        ));
    }
    let projected_all = project_events(e);
    let cast_turn = projected_all
        .iter()
        .find(|event| {
            event.kind == "SpellCast" && event.facts.objects.contains(&ObjectRef(mortipede.0))
        })
        .map(|event| event.turn)
        .ok_or("projected Mortipede cast turn missing")?;
    let ability_turn = projected_all
        .iter()
        .find(|event| {
            event.kind == "AbilityActivated"
                && event.facts.objects.contains(&ObjectRef(mortipede.0))
        })
        .map(|event| event.turn)
        .ok_or("projected Mortipede activation turn missing")?;
    let attack_turn = projected_all
        .iter()
        .find(|event| {
            event.kind == "AttackersDeclared"
                && event.facts.objects.contains(&ObjectRef(mortipede.0))
                && event.turn >= ability_turn
        })
        .map(|event| event.turn)
        .ok_or("projected Mortipede attack turn missing")?;
    if !(cast_turn <= ability_turn && ability_turn == attack_turn) {
        return Err("Mortipede activation and required-block attack must occur in the same legal full-game turn after it is cast".into());
    }
    let selected = BTreeSet::from([
        paid,
        cast,
        resolved,
        entered,
        ability_paid,
        activated,
        ability_resolved,
        attack,
        block,
        attack_damage,
        block_damage,
        mortipede_died,
        blocker_died,
        later_damage,
        end,
    ]);
    let projected: Vec<ProjectedEvent> = projected_all
        .into_iter()
        .enumerate()
        .filter_map(|(index, event)| selected.contains(&index).then_some(event))
        .collect();
    Ok(
        json!({"trace_id":"rav-mortipede-green-must-be-blocked-pvp","shuffle_seed":seed,"decks":["rav_mortipede_three_color_basics","rav_lurking_informant_island"],"policies":["rav.mortipede-must-be-blocked.v1","rav.lurking-informant-blocks-required-attacker.v1"],"terminal":c.terminal,"winner_seat":c.winner,"turns":c.turns,"accepted_moves":c.accepted,"rejected_moves":c.rejected,"mortipede":{"object":mortipede.0,"collector_number":96,"mana_cost":"{3}{B}","mana_paid":c.cast_payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"base_power":4,"base_toughness":1,"zone_after_required_combat":"Graveyard"},"blocker":{"object":blocker.0,"definition":BLOCKER_ID,"must_block_assignment":true,"zone_after_combat":"Graveyard"},"activation":{"ability":ABILITY_ID,"source":mortipede.0,"mana_cost":"{2}{G}","green_mana_paid":true,"effect":"MustBeBlockedIfAble until end of turn","resolved_before_attack":true},"combat":{"actual_required_block":true,"mortipede_dealt_4":true,"both_creatures_died":true,"support_damage_to_finish_game":support_damage,"opponent_life_at_terminal":c.life[1]},"events":projected,"passed":true}),
    )
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
