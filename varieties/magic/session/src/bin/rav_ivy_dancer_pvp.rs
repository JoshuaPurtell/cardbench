//! Full-game RAV Ivy Dancer target-Forestwalk trace authority.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    AbilityActivation, BasicLandType, CardDefinition, CardType, CastRequest, Color, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, Step, Target, TargetRequirement, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::{ObjectRef, SeatId, ViewerScope};
use cardbench_magic_rav::{
    RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-IVY-DANCER";
const SUPPORT_ID: &str = "RAV-WATCHWOLF";
const BLOCKER_ID: &str = "RAV-ELVES-OF-DEEP-SHADOW";
const ABILITY_ID: &str = "tap-target-creature-grant-forestwalk";
const MAX_TRACE_SEED: u64 = 256;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/ivy_dancer.rs"]
mod candidate_ivy_dancer;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Ivy Dancer PvP result: {error}");
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
            eprintln!("Ivy Dancer PvP authority failed: {error}");
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
    WrongEffect,
    WrongAbilityCost,
    WrongCastCost,
    WrongPower,
    WrongToughness,
}

fn candidate_values(
    mutation: Mutation,
) -> (CardDefinition, cardbench_magic_engine::ActivatedAbility) {
    let mut definition = candidate_ivy_dancer::definition();
    let mut ability = candidate_ivy_dancer::activated_ability().ability;
    match mutation {
        Mutation::None => {}
        Mutation::WrongEffect => {
            ability.effects = vec![Effect::ModifyTargetKeywordUntilEndOfTurn {
                keyword: Keyword::Vigilance,
            }]
        }
        Mutation::WrongAbilityCost => ability.mana_cost = ManaCost::with_colors(0, [Color::Green]),
        Mutation::WrongCastCost => definition.mana_cost = ManaCost::with_colors(1, [Color::Green]),
        Mutation::WrongPower => definition.power = Some(2),
        Mutation::WrongToughness => definition.toughness = Some(1),
    }
    (definition, ability)
}

fn validate_candidate_source() -> Result<(), String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../sets/ravnica_city_of_guilds/src/cards/ivy_dancer.rs");
    let source = std::fs::read_to_string(&path)
        .map_err(|error| format!("could not read Ivy Dancer candidate module: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Ivy Dancer module is not valid Rust source".to_owned())?;
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
        "id:\"RAV-IVY-DANCER\"",
        "name:\"IvyDancer\"",
        "mana_cost:ManaCost::with_colors(2,[Color::Green])",
        "power:Some(1)",
        "toughness:Some(2)",
        "id:\"tap-target-creature-grant-forestwalk\"",
        "tap_cost:true",
        "targets:vec![TargetRequirement::Creature]",
        "Effect::ModifyTargetKeywordUntilEndOfTurn{keyword:Keyword::Landwalk(BasicLandType::Forest),}",
    ];
    if function_names != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()])
        || required.iter().any(|fragment| !compact.contains(fragment))
    {
        return Err(
            "Ivy Dancer candidate definition or ability is outside its literal source contract"
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
        .ok_or("Ivy Dancer definition is missing from the RAV registry")?;
    let binding = rav_activated_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("Ivy Dancer activated ability is missing from the RAV registry")?;
    if registered != definition || binding.ability != ability {
        return Err(
            "Ivy Dancer definition and ability are not delegated to its candidate module".into(),
        );
    }
    if definition.id != CARD_ID
        || definition.name != "Ivy Dancer"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(2, [Color::Green])
        || definition.colors != BTreeSet::from([Color::Green])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.power != Some(1)
        || definition.toughness != Some(2)
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
        || ability.id != ABILITY_ID
        || ability.mana_cost != ManaCost::new(0)
        || !ability.tap_cost
        || ability.sorcery_speed
        || ability.additional_tap_creatures != 0
        || ability.sacrifice_source
        || ability.sacrifice_creatures != 0
        || ability.sacrifice_lands != 0
        || ability.discard_cards != 0
        || ability.targets != [TargetRequirement::Creature]
        || ability.effects
            != [Effect::ModifyTargetKeywordUntilEndOfTurn {
                keyword: Keyword::Landwalk(BasicLandType::Forest),
            }]
    {
        return Err(
            "Ivy Dancer identity or activated Forestwalk feature differs from RAV #171".into(),
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
        return Err("RAV catalog denominator differs from its source contract".into());
    }
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.terminal != replay.terminal
        || captured.winner != replay.winner
        || captured.ivy != replay.ivy
        || captured.target != replay.target
        || captured.target_keywords != replay.target_keywords
        || captured.illegal_block_rejection != replay.illegal_block_rejection
    {
        return Err("Ivy Dancer shuffled-game replay was not deterministic".into());
    }
    let mutations = json!({
        "forestwalk_effect_mutation_detected_in_game":mutation_detected(seed,Mutation::WrongEffect)?,
        "activated_ability_cost_mutation_detected_in_game":mutation_detected(seed,Mutation::WrongAbilityCost)?,
        "casting_cost_mutation_detected_in_game":mutation_detected(seed,Mutation::WrongCastCost)?,
        "base_power_mutation_detected_in_game":mutation_detected(seed,Mutation::WrongPower)?,
        "base_toughness_mutation_detected_in_game":mutation_detected(seed,Mutation::WrongToughness)?,
        "sealed_mutation_authority":false
    });
    for key in [
        "forestwalk_effect_mutation_detected_in_game",
        "activated_ability_cost_mutation_detected_in_game",
        "casting_cost_mutation_detected_in_game",
        "base_power_mutation_detected_in_game",
        "base_toughness_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Ivy Dancer semantic mutation escaped the full-game trace: {key}"
            ));
        }
    }
    let breakdown:Vec<_>=catalog.iter().map(|card|{let id=card_definitions().into_iter().find(|definition|definition.name==card.name).map(|definition|definition.id).unwrap_or("");json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,"engine_status":if card.semantic_status.is_executable(){"executable_compatibility_slice"}else{"catalog_only"},"trace_status":if id==CARD_ID{"passed"}else{"not_qualified"},"qualification":if id==CARD_ID{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})}).collect();
    Ok(
        json!({"schema_version":"cardbench.magic.card-pvp.check.v1","task_id":TASK_ID,"expansion":"RAV","card":{"collector_number":171,"definition_id":CARD_ID,"name":"Ivy Dancer"},"candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},"candidate_kind":candidate_kind(),"viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{"comprehensive_rules":"2026-08-07","checked_sections":["117 Priority","302.6 Summoning sickness","508.1 Declaring attackers","509.1-509.2 Declaring blockers and landwalk legality","510.1-510.2 Combat damage","601.2 Casting and payment","602.1-602.2 Activated abilities","608.2 Ability resolution","611.2 Until-end-of-turn effects","702.14 Landwalk"],"source_contract":"RAV Ivy Dancer #171, {2}{G} 1/2 green creature; {T}: target creature gains Forestwalk until end of turn","authority_cutoff":"2026-09-23","no_provider_trial":true},
        "passed":true,"pilot_card_passed":true,"cards_passed":1,"cards_credited":0,"cards_total":catalog.len(),"printings_total":printings.len(),"coverage_complete":false,"full_reward":false,"harbor_reward":0.0,"pilot_reward":0.0,"qualification_status":"unreviewed_diagnostic","mutation_sensitivity":mutations,"traces":[trace],"per_card_breakdown":breakdown}),
    )
}

#[derive(Clone, Debug)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    ivy: Option<ObjectId>,
    target: Option<ObjectId>,
    cast_cost: ManaCost,
    cast_payment: Vec<Color>,
    definition: CardDefinition,
    ability: cardbench_magic_engine::ActivatedAbility,
    target_keywords: Vec<String>,
    forest_at_attack: bool,
    target_zone: Option<Zone>,
    ivy_zone: Option<Zone>,
    target_blocked: bool,
    illegal_block_rejection: Option<String>,
    expiry_observed: bool,
    target_damage: i64,
    life: [i64; 2],
    ability_turn: Option<u32>,
    attack_turn: Option<u32>,
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_IVY_DANCER_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_IVY_DANCER_SEED must be an unsigned integer")?;
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
        "no Ivy Dancer full-game Forestwalk trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Ok(c) => Ok(validate_trace(&c, seed).is_err()),
        Err(_) => Ok(true),
    }
}

fn new_game(
    definition: CardDefinition,
    ability: cardbench_magic_engine::ActivatedAbility,
) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|item| item.id == CARD_ID)
        .ok_or("Ivy Dancer definition missing from registry")? = definition;
    let mut abilities = rav_activated_ability_bindings();
    let binding = abilities
        .iter_mut()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("Ivy Dancer ability missing from registry")?;
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
                card: "RAV-FOREST".into(),
                count: 26,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 26,
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
                card: "RAV-FOREST".into(),
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
        return Err("Ivy Dancer duel decks must each contain 60 cards".into());
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
    let support_cost = card_definitions()
        .into_iter()
        .find(|card| card.id == SUPPORT_ID)
        .ok_or("Watchwolf definition missing")?
        .mana_cost;
    let blocker_cost = card_definitions()
        .into_iter()
        .find(|card| card.id == BLOCKER_ID)
        .ok_or("Elves of Deep Shadow definition missing")?
        .mana_cost;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(IvyPolicy::new(
            PlayerId(0),
            &cast_cost,
            &ability.mana_cost,
            &support_cost,
        )),
        Box::new(IvyPolicy::new(
            PlayerId(1),
            &blocker_cost,
            &ManaCost::new(0),
            &support_cost,
        )),
    ];
    let (mut accepted, mut attempted) = (0u32, 0u32);
    let (mut ivy, mut target) = (None, None);
    let (mut target_keywords, mut cast_payment) = (Vec::new(), Vec::new());
    let (mut forest_at_attack, mut expiry_observed) = (false, false);
    let mut illegal_block_rejection = None;
    let (mut ability_turn, mut attack_turn) = (None, None);
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Ivy Dancer match exceeded bounded move count".into());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        if !expiry_observed
            && player == PlayerId(0)
            && view.step == Step::PrecombatMain
            && ability_turn.is_some_and(|turn| view.turn > turn)
        {
            if let Some(target_id) = target {
                if let Some(target_view) = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.id == target_id)
                {
                    expiry_observed = !target_view
                        .keywords
                        .contains(&Keyword::Landwalk(BasicLandType::Forest));
                }
            }
        }
        if player == PlayerId(0)
            && view.step == Step::DeclareAttackers
            && view.active_player == player
        {
            if let Some(target_id) = target {
                let carries = view.own_battlefield.iter().any(|card| {
                    card.id == target_id
                        && card
                            .keywords
                            .contains(&Keyword::Landwalk(BasicLandType::Forest))
                });
                let defender_has_forest = view.opponent_battlefield.iter().any(|card| {
                    card.definition == Some("RAV-FOREST")
                        && card.card_types.contains(&CardType::Land)
                });
                if carries && defender_has_forest {
                    forest_at_attack = true;
                }
            }
        }
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
        if illegal_block_rejection.is_none()
            && player == PlayerId(1)
            && view.step == Step::DeclareBlockers
            && view.active_player == PlayerId(0)
            && !view.blockers_declared
        {
            if let Some(attacker) = view.combat_attackers.iter().find(|card| {
                Some(card.id) == target
                    && card
                        .keywords
                        .contains(&Keyword::Landwalk(BasicLandType::Forest))
            }) {
                if view.own_battlefield.iter().any(|card| {
                    card.definition == Some("RAV-FOREST")
                        && card.card_types.contains(&CardType::Land)
                }) {
                    if let Some(blocker) = view.own_battlefield.iter().find(|card| card.can_block) {
                        let mut probe = game.clone();
                        let block = PolicyAction::DeclareBlockers {
                            assignments: vec![cardbench_magic_engine::CombatBlock {
                                attacker: attacker.id,
                                blocker: blocker.id,
                            }],
                        };
                        if let Err(error) = probe.submit_policy_move(player, policy.id(), block) {
                            illegal_block_rejection = Some(error.to_string());
                        }
                    }
                }
            }
        }
        let before = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Ivy Dancer policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("Ivy Dancer invariant failed after move: {error}"))?;
        let new_events = &game.event_log[before..];
        if ivy.is_none() {
            ivy = new_events.iter().find_map(|event| match event {
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
        }
        if let Some(source) = ivy {
            if cast_payment.is_empty() {
                cast_payment = new_events
                    .iter()
                    .find_map(|event| match event {
                        GameEvent::SpellManaPaid {
                            player: PlayerId(0),
                            card,
                            colors,
                        } if *card == source => Some(colors.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
            }
            if ability_turn.is_none()&&new_events.iter().any(|event|matches!(event,GameEvent::AbilityResolved{source:resolved,ability:ABILITY_ID,..}if *resolved==source)) {
                ability_turn=Some(game.turn); let after=game.view_for_player(PlayerId(0)).map_err(|error|error.to_string())?;
                if let Some(chosen)=after.own_battlefield.iter().find(|permanent|permanent.definition==Some(SUPPORT_ID)&&permanent.keywords.contains(&Keyword::Landwalk(BasicLandType::Forest))) {
                    target=Some(chosen.id); target_keywords=chosen.keywords.iter().map(|keyword|format!("{keyword:?}")).collect();
                }
            }
        }
        if let Some(target_id) = target {
            if attack_turn.is_none() && new_events.iter().any(|event|matches!(event,GameEvent::AttackersDeclared{player:PlayerId(0),attackers}if attackers.contains(&target_id))){attack_turn=Some(game.turn);}
        }
    }
    let events = game.event_log.clone();
    let target_blocked=target.is_some_and(|target_id|events.iter().any(|event|matches!(event,GameEvent::BlockersDeclared{assignments,..}if assignments.iter().any(|(attacker,_)|*attacker==target_id))));
    let target_damage = target.map_or(0, |target_id| {
        events
            .iter()
            .filter_map(|event| match event {
                GameEvent::DamageDealtToPlayer {
                    source,
                    player: PlayerId(1),
                    amount,
                } if *source == target_id => Some(i64::from(*amount)),
                _ => None,
            })
            .sum()
    });
    let grave_moved = |object: ObjectId| {
        events.iter().any(
            |event| matches!(event,GameEvent::CardMoved{card,to:Zone::Graveyard}if *card==object),
        )
    };
    let ivy_zone = ivy.and_then(|object| {
        grave_moved(object)
            .then_some(Zone::Graveyard)
            .or_else(|| game.zone_of(object))
    });
    let target_zone = target.and_then(|object| {
        grave_moved(object)
            .then_some(Zone::Graveyard)
            .or_else(|| game.zone_of(object))
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
            || "Draw".into(),
            |seat| format!("Winner(PlayerId({}))", seat.0),
        ),
        winner: game.winner().map(|seat| seat.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        ivy,
        target,
        cast_cost,
        cast_payment,
        definition,
        ability,
        target_keywords,
        forest_at_attack,
        target_zone,
        ivy_zone,
        target_blocked,
        illegal_block_rejection,
        expiry_observed,
        target_damage,
        life,
        ability_turn,
        attack_turn,
    })
}

struct IvyPolicy {
    player: PlayerId,
    cast_cost: ManaCost,
    activation_cost: ManaCost,
    support_cost: ManaCost,
    activation_offered_turn: Option<u32>,
}

impl IvyPolicy {
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
                (view.mana_pool.total() < cost.generic.saturating_add(cost.colored.len() as u8))
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
    fn make_cast(&self, view: &GameView, id: &str, cost: &ManaCost) -> Option<PolicyAction> {
        let card = view.hand.iter().find(|card| card.definition == Some(id))?;
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
    fn activate_ivy(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.player != PlayerId(0)
            || view.active_player != self.player
            || view.priority != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
            || self.activation_offered_turn == Some(view.turn)
        {
            return None;
        }
        let source = view.own_battlefield.iter().find(|card| {
            card.definition == Some(CARD_ID) && !card.tapped && !card.summoning_sick
        })?;
        let target = view
            .own_battlefield
            .iter()
            .find(|card| card.definition == Some(SUPPORT_ID) && card.can_attack)?;
        if payment(view, &self.activation_cost).is_some() {
            self.activation_offered_turn = Some(view.turn);
            Some(PolicyAction::ActivateAbility {
                activation: AbilityActivation {
                    source: source.id,
                    ability_id: ABILITY_ID,
                    sacrifice_sources: vec![],
                    additional_tap_creatures: vec![],
                    discard_cards: vec![],
                    targets: vec![Target::Permanent(target.id)],
                },
            })
        } else {
            self.tap_for(view, &self.activation_cost)
        }
    }
    fn declare_blocks(&self, view: &GameView) -> PolicyAction {
        let blockers: Vec<_> = view
            .own_battlefield
            .iter()
            .filter(|card| card.can_block)
            .collect();
        let assignments = view
            .combat_attackers
            .iter()
            .filter(|attacker| {
                !attacker
                    .keywords
                    .contains(&Keyword::Landwalk(BasicLandType::Forest))
            })
            .zip(blockers)
            .map(|(attacker, blocker)| cardbench_magic_engine::CombatBlock {
                attacker: attacker.id,
                blocker: blocker.id,
            })
            .collect();
        PolicyAction::DeclareBlockers { assignments }
    }
}

impl CodePolicy for IvyPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.ivy-dancer-forestwalk.v1"
        } else {
            "rav.forest-defender-block-policy.v1"
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
                        .filter(|card| card.can_attack && card.definition == Some(SUPPORT_ID))
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
            if let Some(action) = self.activate_ivy(view) {
                return action;
            }
            if !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(CARD_ID))
            {
                if let Some(action) = self.make_cast(view, CARD_ID, &self.cast_cost) {
                    return action;
                }
            }
            let number = view
                .own_battlefield
                .iter()
                .filter(|card| card.definition == Some(SUPPORT_ID))
                .count();
            if number < 1 {
                if let Some(action) = self.make_cast(view, SUPPORT_ID, &self.support_cost) {
                    return action;
                }
            }
        } else {
            let number = view
                .own_battlefield
                .iter()
                .filter(|card| card.definition == Some(BLOCKER_ID))
                .count();
            if number < 4 {
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

fn validate_trace(c: &Captured, seed: u64) -> Result<Value, String> {
    if c.terminal != "Winner(PlayerId(0))" || c.winner != Some(0) || c.rejected != 0 {
        return Err("Ivy Dancer match did not reach a clean seat-zero terminal win".into());
    }
    let ivy = c.ivy.ok_or("seat zero did not cast Ivy Dancer")?;
    let target = c
        .target
        .ok_or("Ivy Dancer did not grant Forestwalk to the intended Watchwolf")?;
    if c.definition.id != CARD_ID
        || c.definition.name != "Ivy Dancer"
        || c.definition.mana_cost != ManaCost::with_colors(2, [Color::Green])
        || c.definition.colors != BTreeSet::from([Color::Green])
        || c.definition.power != Some(1)
        || c.definition.toughness != Some(2)
    {
        return Err("Ivy Dancer exact {2}{G} 1/2 definition was not observed".into());
    }
    if c.cast_cost != ManaCost::with_colors(2, [Color::Green])
        || c.cast_payment.len() != 3
        || !c.cast_payment.contains(&Color::Green)
    {
        return Err(format!(
            "Ivy Dancer did not pay exact {{2}}{{G}}: {:?}",
            c.cast_payment
        ));
    }
    if c.ability.id != ABILITY_ID
        || c.ability.mana_cost != ManaCost::new(0)
        || !c.ability.tap_cost
        || c.ability.sorcery_speed
        || c.ability.additional_tap_creatures != 0
        || c.ability.sacrifice_source
        || c.ability.sacrifice_creatures != 0
        || c.ability.sacrifice_lands != 0
        || c.ability.discard_cards != 0
        || c.ability.targets != [TargetRequirement::Creature]
        || c.ability.effects
            != [Effect::ModifyTargetKeywordUntilEndOfTurn {
                keyword: Keyword::Landwalk(BasicLandType::Forest),
            }]
    {
        return Err(format!(
            "Ivy Dancer's exact tap-target Forestwalk activation is wrong: {:?}",
            c.ability
        ));
    }
    if c.ability_turn.is_none() || c.attack_turn != c.ability_turn {
        return Err(format!(
            "Ivy Dancer did not resolve the activation before a same-turn Forestwalk attack: ability_turn={:?}, attack_turn={:?}",
            c.ability_turn, c.attack_turn
        ));
    }
    if !c
        .target_keywords
        .iter()
        .any(|keyword| keyword == "Landwalk(Forest)")
        || !c.forest_at_attack
    {
        return Err(format!(
            "target Watchwolf lacked live Forestwalk or the defending player lacked a Forest at attack declaration: {:?}",
            c.target_keywords
        ));
    }
    if c.illegal_block_rejection.as_deref()
        != Some("landwalk attacker cannot be blocked while defender controls its land type")
        || c.target_blocked
    {
        return Err(format!(
            "engine failed to reject an attempted block on the Forestwalk attacker: {:?}",
            c.illegal_block_rejection
        ));
    }
    if !c.expiry_observed {
        return Err("Forestwalk continuous effect did not expire by the next turn".into());
    }
    if c.target_zone != Some(Zone::Battlefield) || c.ivy_zone != Some(Zone::Battlefield) {
        return Err(format!(
            "Ivy Dancer or targeted Watchwolf left the battlefield before the terminal game: {:?} {:?}",
            c.ivy_zone, c.target_zone
        ));
    }
    if c.target_damage < 20 || c.life[1] > 0 || c.life[0] <= 0 {
        return Err(format!(
            "targeted Forestwalk attacker did not deal lethal cumulative damage in the complete game: damage={} life={:?}",
            c.target_damage, c.life
        ));
    }
    let e = &c.events;
    let mana_paid=find_event(e,|event|matches!(event,GameEvent::SpellManaPaid{player:PlayerId(0),card,colors}if *card==ivy&&colors.len()==3&&colors.contains(&Color::Green))).ok_or("Ivy Dancer spell mana receipt missing")?;
    let cast = find_event_after(
        e,
        mana_paid,
        |event| matches!(event,GameEvent::SpellCast{player:PlayerId(0),card}if *card==ivy),
    )
    .ok_or("Ivy Dancer cast receipt missing")?;
    let cast_resolved = find_event_after(
        e,
        cast,
        |event| matches!(event,GameEvent::SpellResolved{card}if *card==ivy),
    )
    .ok_or("Ivy Dancer did not resolve")?;
    let entered = find_event_after(
        e,
        cast_resolved,
        |event| matches!(event,GameEvent::CardMoved{card,to:Zone::Battlefield}if *card==ivy),
    )
    .ok_or("Ivy Dancer did not enter the battlefield")?;
    let activated=find_event_after(e,entered,|event|matches!(event,GameEvent::AbilityActivated{player:PlayerId(0),source,definition:CARD_ID,ability:ABILITY_ID,..}if *source==ivy)).ok_or("Ivy Dancer activation receipt missing")?;
    let resolved=find_event_after(e,activated,|event|matches!(event,GameEvent::AbilityResolved{source,ability:ABILITY_ID,..}if *source==ivy)).ok_or("Ivy Dancer Forestwalk activation did not resolve")?;
    let attack=find_event_after(e,resolved,|event|matches!(event,GameEvent::AttackersDeclared{player:PlayerId(0),attackers}if attackers.contains(&target))).ok_or("targeted Forestwalk Watchwolf did not attack after ability resolution")?;
    let damage=find_event_after(e,attack,|event|matches!(event,GameEvent::DamageDealtToPlayer{source,player:PlayerId(1),amount:3}if *source==target)).ok_or("targeted Forestwalk Watchwolf did not deal direct combat damage")?;
    let ended = find_event(e, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal seat-zero victory receipt missing")?;
    if !(mana_paid < cast
        && cast < cast_resolved
        && cast_resolved < entered
        && entered < activated
        && activated < resolved
        && resolved < attack
        && attack < damage
        && damage < ended)
    {
        return Err(
            "Ivy Dancer cast, activation, Forestwalk attack, and terminal events are out of order"
                .into(),
        );
    }
    let projected_all = project_events(e);
    let projected_attack = projected_all
        .iter()
        .find(|event| {
            event.kind == "AttackersDeclared"
                && event.facts.objects.contains(&ObjectRef(target.0))
                && event.turn == c.attack_turn.unwrap_or_default()
        })
        .ok_or("projected targeted Watchwolf attack missing")?;
    let projected_damage = projected_all
        .iter()
        .find(|event| {
            event.kind == "DamageDealtToPlayer"
                && event.facts.objects.contains(&ObjectRef(target.0))
                && event.facts.seats == [SeatId(1)]
                && event.facts.amount == Some(3)
        })
        .ok_or("projected Forestwalk combat damage missing")?;
    let terminal_index = projected_all
        .iter()
        .rposition(|event| event.kind == "GameEnded" && event.facts.seats == [SeatId(0)])
        .ok_or("projected terminal event missing")?;
    let selected = BTreeSet::from([
        mana_paid,
        cast,
        cast_resolved,
        entered,
        activated,
        resolved,
        attack,
        damage,
        ended,
    ]);
    let projected: Vec<ProjectedEvent> = projected_all
        .iter()
        .enumerate()
        .filter_map(|(index, event)| selected.contains(&index).then_some(event.clone()))
        .collect();
    if projected_attack.sequence >= projected_damage.sequence
        || usize::try_from(projected_damage.sequence).is_err()
        || terminal_index >= projected_all.len()
    {
        return Err("projected Ivy Dancer event sequence is invalid".into());
    }
    Ok(
        json!({"trace_id":"rav-ivy-dancer-target-forestwalk-pvp","shuffle_seed":seed,"decks":["rav_ivy_dancer_selesnya","rav_forest_defender_elves"],"policies":["rav.ivy-dancer-forestwalk.v1","rav.forest-defender-block-policy.v1"],"terminal":c.terminal,"winner_seat":c.winner,"turns":c.turns,"accepted_moves":c.accepted,"rejected_moves":c.rejected,
        "ivy_dancer":{"object":ivy.0,"collector_number":171,"mana_cost":"{2}{G}","mana_paid":c.cast_payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"base_power":1,"base_toughness":2,"zone_at_terminal":c.ivy_zone.map(|zone|format!("{zone:?}")).unwrap_or_default()},
        "activation":{"ability":ABILITY_ID,"source":ivy.0,"mana_cost":"{T}","tap_cost_paid":true,"target":target.0,"target_definition":SUPPORT_ID,"target_keyword_snapshot":c.target_keywords,"resolved_turn":c.ability_turn,"target_attack_turn":c.attack_turn,"forestwalk_expired_next_turn":c.expiry_observed},
        "defender":{"controls_forest_at_attack":c.forest_at_attack,"illegal_block_diagnostic":c.illegal_block_rejection,"target_was_blocked":c.target_blocked},
        "combat":{"target_watchwolf":target.0,"direct_damage_total":c.target_damage,"target_zone_at_terminal":c.target_zone.map(|zone|format!("{zone:?}")).unwrap_or_default(),"opponent_life":c.life[1],"life_at_terminal":c.life},"events":projected,"passed":true}),
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
