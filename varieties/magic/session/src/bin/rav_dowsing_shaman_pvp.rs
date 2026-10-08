//! Full-game RAV Dowsing Shaman enchantment-recursion authority.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, Color, CombatBlock, DeckEntry, DeckList, Effect, Game, GameEvent, GameView,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target,
    TargetRequirement, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_attachment_bindings, rav_attachment_triggered_ability_bindings as attachment_triggers,
    rav_basic_land_type_bindings, rav_land_entry_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings, rav_static_continuous_effect_bindings,
    rav_triggered_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-DOWSING-SHAMAN";
const FISTS_ID: &str = "RAV-FISTS-OF-IRONWOOD";
const ELF_ID: &str = "RAV-ELVES-OF-DEEP-SHADOW";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const ABILITY_ID: &str = "return-target-enchantment-from-graveyard";
const MAX_TURNS: u32 = 60;
const MAX_POLICY_MOVES: u32 = 8_000;
const REFERENCE_SEED: u64 = 0;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/dowsing_shaman.rs"]
mod candidate_dowsing_shaman;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Dowsing Shaman result: {error}");
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
            eprintln!("Dowsing Shaman PvP authority failed: {error}");
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongActivationCost,
    WrongActivationEffect,
    WrongTargetScope,
    WrongCastCost,
    WrongPower,
    WrongToughness,
}

fn candidate_values(mutation: Mutation) -> (CardDefinition, ActivatedAbilityBinding) {
    let mut definition = candidate_dowsing_shaman::definition();
    let mut binding = candidate_dowsing_shaman::activated_ability();
    match mutation {
        Mutation::None => {}
        Mutation::WrongActivationCost => {
            binding.ability.mana_cost = ManaCost::with_colors(1, [Color::Green]);
        }
        Mutation::WrongActivationEffect => binding.ability.effects.clear(),
        Mutation::WrongTargetScope => {
            binding.ability.targets = vec![TargetRequirement::Creature];
        }
        Mutation::WrongCastCost => {
            definition.mana_cost = ManaCost::with_colors(3, [Color::Green]);
        }
        Mutation::WrongPower => definition.power = Some(2),
        Mutation::WrongToughness => definition.toughness = Some(5),
    }
    (definition, binding)
}

fn validate_candidate_source() -> Result<(), String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../sets/ravnica_city_of_guilds/src/cards/dowsing_shaman.rs");
    let source = std::fs::read_to_string(&path)
        .map_err(|error| format!("could not read Dowsing Shaman candidate module: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Dowsing Shaman module is not valid Rust source".to_owned())?;
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
        "id:\"RAV-DOWSING-SHAMAN\"",
        "name:\"DowsingShaman\"",
        "mana_cost:ManaCost::with_colors(4,[Color::Green])",
        "card_types:BTreeSet::from([CardType::Creature])",
        "power:Some(3)",
        "toughness:Some(4)",
        "id:\"return-target-enchantment-from-graveyard\"",
        "mana_cost:ManaCost::with_colors(2,[Color::Green])",
        "tap_cost:true",
        "targets:vec![TargetRequirement::EnchantmentCardInControllerGraveyard]",
        "effects:vec![Effect::ReturnTargetEnchantmentCardToHand]",
    ];
    if function_names != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()])
        || required.iter().any(|fragment| !compact.contains(fragment))
    {
        return Err(
            "Dowsing Shaman candidate definition or ability is outside its literal source contract"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_runtime() -> Result<(), String> {
    let (definition, binding) = candidate_values(Mutation::None);
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Dowsing Shaman definition is missing from the RAV registry")?;
    let registered_binding = rav_activated_ability_bindings()
        .into_iter()
        .find(|candidate| {
            candidate.card_definition == CARD_ID && candidate.ability.id == ABILITY_ID
        })
        .ok_or("Dowsing Shaman activation is missing from the RAV registry")?;
    if definition != expected_definition()
        || definition != registered
        || binding != expected_binding()
        || binding != registered_binding
    {
        return Err(
            "Dowsing Shaman definition and ability are not delegated to its exact RAV source contract"
                .to_owned(),
        );
    }
    Ok(())
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: "Dowsing Shaman",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(4, [Color::Green]),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "return-target-enchantment-from-graveyard",
        ],
        power: Some(3),
        toughness: Some(4),
        keywords: vec![],
        effects: vec![],
    }
}

fn expected_binding() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: CARD_ID,
        ability: ActivatedAbility {
            id: ABILITY_ID,
            mana_cost: ManaCost::with_colors(2, [Color::Green]),
            tap_cost: true,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![TargetRequirement::EnchantmentCardInControllerGraveyard],
            effects: vec![Effect::ReturnTargetEnchantmentCardToHand],
        },
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    shaman: Option<ObjectId>,
    elf: Option<ObjectId>,
    fists: Option<ObjectId>,
    blocker: Option<ObjectId>,
    activation_targets: Vec<Target>,
    target_zone_at_activation: Option<Zone>,
    target_zone_at_end: Option<Zone>,
    shaman_tapped_at_activation: bool,
    shaman_definition: CardDefinition,
    activation_binding: ActivatedAbilityBinding,
    shaman_cast_payment: Vec<Color>,
    fists_cast_payment: Vec<Color>,
    activation_payment: Option<ManaCost>,
    life: [i64; 2],
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Err(_) => Ok(true),
        Ok(capture) => Ok(validate_trace(&capture, seed).is_err()),
    }
}

fn reference_seed() -> Result<u64, String> {
    std::env::var("CARDBENCH_DOWSING_SHAMAN_SEED").map_or(Ok(REFERENCE_SEED), |value| {
        value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_DOWSING_SHAMAN_SEED must be an unsigned integer".to_owned())
    })
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
    let seed = reference_seed()?;
    let reference = run_match(seed, Mutation::None)?;
    let trace = validate_trace(&reference, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if reference != replay {
        return Err("Dowsing Shaman shuffled-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "activation_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongActivationCost)?,
        "activation_effect_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongActivationEffect)?,
        "target_scope_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongTargetScope)?,
        "cast_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongCastCost)?,
        "base_power_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongPower)?,
        "base_toughness_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongToughness)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "activation_cost_mutation_detected_in_game",
        "activation_effect_mutation_detected_in_game",
        "target_scope_mutation_detected_in_game",
        "cast_cost_mutation_detected_in_game",
        "base_power_mutation_detected_in_game",
        "base_toughness_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Dowsing Shaman semantic mutation escaped the PvP trace: {key}"
            ));
        }
    }
    let definitions = card_definitions();
    let breakdown: Vec<_> = catalog.iter().map(|card| {
        let id = definitions.iter().find(|definition| definition.name == card.name).map(|definition| definition.id).unwrap_or("");
        json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,
            "engine_status":if card.semantic_status.is_executable(){"executable_compatibility_slice"}else{"catalog_only"},
            "trace_status":if id==CARD_ID{"passed"}else{"not_qualified"},
            "qualification":if id==CARD_ID{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
    }).collect();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,
        "expansion":"RAV",
        "card":{"collector_number":159,"definition_id":CARD_ID,"name":"Dowsing Shaman"},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{"comprehensive_rules":"2026-08-07","checked_sections":["117 Priority","302.1 Creatures","303.4 Auras","400.7 Zone changes","602.1-602.2 Activated abilities","608.2 Ability resolution","601.2 Casting and payment"],
            "source_contract":"RAV Dowsing Shaman #159, {4}{G} 3/4 green creature; {2}{G}, tap: return target enchantment card from your graveyard to your hand",
            "authority_cutoff":"2026-09-24","no_provider_trial":true},
        "passed":true,"pilot_card_passed":true,"deterministic_replay":true,
        "cards_passed":1,"cards_credited":0,"cards_total":catalog.len(),"printings_total":printings.len(),
        "coverage_complete":false,"full_reward":false,"harbor_reward":0.0,"pilot_reward":0.0,
        "qualification_status":"unreviewed_diagnostic","mutation_sensitivity":mutations,
        "traces":[trace],"per_card_breakdown":breakdown
    }))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let (definition, binding) = candidate_values(mutation);
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Dowsing Shaman definition is missing from the RAV registry")?;
    *registered = definition.clone();
    let mut abilities = rav_activated_ability_bindings();
    let ability_index = abilities
        .iter()
        .position(|candidate| {
            candidate.card_definition == CARD_ID && candidate.ability.id == ABILITY_ID
        })
        .ok_or("Dowsing Shaman activation is missing from the RAV registry")?;
    abilities[ability_index] = binding.clone();
    let mut triggers = rav_triggered_ability_bindings();
    triggers.extend(attachment_triggers());
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
        triggers,
        rav_static_continuous_effect_bindings(),
        rav_land_entry_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_attachment_bindings(rav_attachment_bindings())
        .map_err(|error| error.to_string())?;

    let p0_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: ELF_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: FISTS_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: WATCHWOLF_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".into(),
                count: 22,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 22,
            },
        ],
        sideboard: vec![],
    };
    let p1_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: WATCHWOLF_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".into(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 28,
            },
        ],
        sideboard: vec![],
    };
    for (label, deck) in [
        ("Dowsing Shaman deck", &p0_deck),
        ("opponent deck", &p1_deck),
    ] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{label} has {count} cards, expected sixty"));
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

    let mut policies = [
        DowsingPolicy::new(PlayerId(0)),
        DowsingPolicy::new(PlayerId(1)),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut shaman = None;
    let mut elf = None;
    let mut fists = None;
    let mut blocker = None;
    let mut activation_targets = Vec::new();
    let mut target_zone_at_activation = None;
    let mut shaman_tapped_at_activation = false;
    let mut shaman_cast_payment = Vec::new();
    let mut fists_cast_payment = Vec::new();
    let mut activation_payment = None;
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || accepted >= MAX_POLICY_MOVES {
            let p0 = game
                .view_for_player(PlayerId(0))
                .map(|view| {
                    view.own_battlefield
                        .iter()
                        .map(|card| {
                            format!(
                                "{}#{}{}",
                                card.definition.unwrap_or("unknown"),
                                card.id.0,
                                if card.tapped { "(tapped)" } else { "" }
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let p1 = game
                .view_for_player(PlayerId(1))
                .map(|view| {
                    view.own_battlefield
                        .iter()
                        .map(|card| {
                            format!(
                                "{}#{}{}",
                                card.definition.unwrap_or("unknown"),
                                card.id.0,
                                if card.tapped { "(tapped)" } else { "" }
                            )
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let tail = game
                .event_log
                .iter()
                .rev()
                .take(36)
                .rev()
                .map(|event| format!("{event:?}"))
                .collect::<Vec<_>>();
            return Err(format!(
                "Dowsing Shaman duel exceeded its bound at turn {}; shaman={shaman:?}, elf={elf:?}, fists={fists:?}, blocker={blocker:?}, p0={p0:?}, p1={p1:?}, target={:?}, activated={}, returned={}, recent_events={tail:?}",
                game.turn,
                policies[0].yard_target,
                policies[0].activation_submitted,
                policies[0].return_completed
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
        let old_event_count = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Dowsing policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        for event in &game.event_log[old_event_count..] {
            match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    shaman = Some(*card)
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    shaman_cast_payment = colors.clone()
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == ELF_ID) =>
                {
                    elf = Some(*card)
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == FISTS_ID) =>
                {
                    fists = Some(*card)
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == FISTS_ID) =>
                {
                    fists_cast_payment = colors.clone()
                }
                GameEvent::SpellCast {
                    player: PlayerId(1),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == WATCHWOLF_ID) =>
                {
                    blocker = Some(*card)
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Graveyard,
                } if Some(*card) == fists => {
                    policies[0].yard_target = Some(*card);
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Hand,
                } if Some(*card) == fists => policies[0].return_completed = true,
                GameEvent::AbilityManaPaid {
                    player: PlayerId(0),
                    source,
                    ability: ABILITY_ID,
                    mana_cost,
                } if Some(*source) == shaman => activation_payment = Some(mana_cost.clone()),
                GameEvent::AbilityActivated {
                    player: PlayerId(0),
                    source,
                    definition: CARD_ID,
                    ability: ABILITY_ID,
                    ..
                } if Some(*source) == shaman => {
                    activation_targets = game
                        .stack
                        .iter()
                        .rev()
                        .find(|stack| stack.card == *source && stack.ability_id == Some(ABILITY_ID))
                        .map(|stack| stack.targets.clone())
                        .unwrap_or_default();
                    target_zone_at_activation = fists.and_then(|target| game.zone_of(target));
                    shaman_tapped_at_activation =
                        game.object(*source).is_ok_and(|object| object.tapped);
                }
                _ => {}
            }
        }
        if let Some(source) = shaman {
            policies[0].shaman = Some(source);
        }
        if let Some(card) = elf {
            policies[0].elf = Some(card);
        }
    }
    let life = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    Ok(Captured {
        events: game.event_log.clone(),
        terminal: game.winner().map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({}))", seat.0),
        ),
        winner: game.winner().map(|seat| seat.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        shaman,
        elf,
        fists,
        blocker,
        activation_targets,
        target_zone_at_activation,
        target_zone_at_end: fists.and_then(|card| game.zone_of(card)),
        shaman_tapped_at_activation,
        shaman_definition: candidate_values(mutation).0,
        activation_binding: candidate_values(mutation).1,
        shaman_cast_payment,
        fists_cast_payment,
        activation_payment,
        life,
    })
}

struct DowsingPolicy {
    player: PlayerId,
    shaman: Option<ObjectId>,
    elf: Option<ObjectId>,
    yard_target: Option<ObjectId>,
    shaman_cast: bool,
    elf_cast: bool,
    fists_cast: bool,
    activation_submitted: bool,
    return_completed: bool,
    support_cast: bool,
    elf_block_used: bool,
    attack_once: bool,
    did_attack: bool,
}

impl DowsingPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            shaman: None,
            elf: None,
            yard_target: None,
            shaman_cast: false,
            elf_cast: false,
            fists_cast: false,
            activation_submitted: false,
            return_completed: false,
            support_cast: false,
            elf_block_used: false,
            attack_once: false,
            did_attack: false,
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

    fn cast(
        &mut self,
        view: &GameView,
        definition: &'static str,
        cost: ManaCost,
        targets: Vec<Target>,
    ) -> Option<PolicyAction> {
        if let Some(mana_selection) = Self::payment(view, &cost) {
            let card = view
                .hand
                .iter()
                .find(|card| card.definition == Some(definition))?;
            match definition {
                CARD_ID => self.shaman_cast = true,
                ELF_ID => self.elf_cast = true,
                FISTS_ID => self.fists_cast = true,
                WATCHWOLF_ID => self.support_cast = true,
                _ => {}
            }
            return Some(PolicyAction::CastWithPayment {
                request: CastRequest {
                    card: card.id,
                    targets,
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
}

impl CodePolicy for DowsingPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.dowsing-shaman-enchantment-recursion.v1"
        } else {
            "rav.dowsing-shaman-block-and-finish.v1"
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
            let attackers = if self.player == PlayerId(1) {
                if self.elf_block_used && self.attack_once && !self.did_attack {
                    view.own_battlefield
                        .iter()
                        .find(|card| card.definition == Some(WATCHWOLF_ID) && card.can_attack)
                        .map(|card| vec![card.id])
                        .unwrap_or_default()
                } else {
                    vec![]
                }
            } else if self.return_completed {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.can_attack && card.card_types.contains(&CardType::Creature))
                    .map(|card| card.id)
                    .collect()
            } else if let Some(elf) = self.elf
                && self.fists_cast
                && self.yard_target.is_none()
                && self.shaman.is_some()
                && view
                    .opponent_battlefield
                    .iter()
                    .any(|card| card.definition == Some(WATCHWOLF_ID) && card.can_block)
                && view
                    .own_battlefield
                    .iter()
                    .any(|card| card.id == elf && card.can_attack)
            {
                vec![elf]
            } else {
                vec![]
            };
            if self.player == PlayerId(1) && !attackers.is_empty() {
                self.did_attack = true;
            }
            return PolicyAction::DeclareAttackers { attackers };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            if self.player == PlayerId(1) && !self.elf_block_used {
                let attacker = view
                    .combat_attackers
                    .iter()
                    .find(|card| card.definition == Some(ELF_ID));
                let blocker = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.definition == Some(WATCHWOLF_ID) && card.can_block);
                if let (Some(attacker), Some(blocker)) = (attacker, blocker) {
                    self.elf_block_used = true;
                    self.attack_once = true;
                    return PolicyAction::DeclareBlockers {
                        assignments: vec![CombatBlock {
                            attacker: attacker.id,
                            blocker: blocker.id,
                        }],
                    };
                }
            }
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.priority != self.player
            || view.active_player != self.player
            || !view.step.is_main()
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
        if self.player == PlayerId(1) {
            if !self.support_cast
                && !view
                    .own_battlefield
                    .iter()
                    .any(|card| card.definition == Some(WATCHWOLF_ID))
                && view
                    .hand
                    .iter()
                    .any(|card| card.definition == Some(WATCHWOLF_ID))
            {
                if let Some(action) = self.cast(
                    view,
                    WATCHWOLF_ID,
                    ManaCost::with_colors(0, [Color::Green, Color::White]),
                    vec![],
                ) {
                    return action;
                }
            }
            return PolicyAction::PassPriority;
        }
        if !self.shaman_cast
            && view
                .hand
                .iter()
                .any(|card| card.definition == Some(CARD_ID))
        {
            if let Some(action) = self.cast(
                view,
                CARD_ID,
                ManaCost::with_colors(4, [Color::Green]),
                vec![],
            ) {
                return action;
            }
        }
        if !self.elf_cast && view.hand.iter().any(|card| card.definition == Some(ELF_ID)) {
            if let Some(action) = self.cast(
                view,
                ELF_ID,
                ManaCost::with_colors(0, [Color::Green]),
                vec![],
            ) {
                return action;
            }
        }
        if !self.fists_cast
            && view
                .hand
                .iter()
                .any(|card| card.definition == Some(FISTS_ID))
            && let Some(elf) = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(ELF_ID))
        {
            if let Some(action) = self.cast(
                view,
                FISTS_ID,
                ManaCost::with_colors(1, [Color::Green]),
                vec![Target::Permanent(elf.id)],
            ) {
                return action;
            }
        }
        if let (Some(shaman), Some(target)) = (self.shaman, self.yard_target)
            && !self.activation_submitted
            && !self.return_completed
            && view
                .own_battlefield
                .iter()
                .any(|card| card.id == shaman && !card.tapped)
        {
            let cost = ManaCost::with_colors(2, [Color::Green]);
            if Self::payment(view, &cost).is_some() {
                self.activation_submitted = true;
                return PolicyAction::ActivateAbility {
                    activation: AbilityActivation {
                        source: shaman,
                        ability_id: ABILITY_ID,
                        sacrifice_sources: vec![],
                        additional_tap_creatures: vec![],
                        discard_cards: vec![],
                        targets: vec![Target::Permanent(target)],
                    },
                };
            }
            let preferred = cost
                .colored
                .iter()
                .find(|color| view.mana_pool.amount(**color) == 0)
                .copied()
                .or_else(|| (Self::total_mana(view) < 3).then_some(Color::Green));
            if let Some(action) = Self::mana_source(view, preferred) {
                return action;
            }
        }
        if self.return_completed
            && !self.support_cast
            && !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(WATCHWOLF_ID))
            && view
                .hand
                .iter()
                .any(|card| card.definition == Some(WATCHWOLF_ID))
        {
            if let Some(action) = self.cast(
                view,
                WATCHWOLF_ID,
                ManaCost::with_colors(0, [Color::Green, Color::White]),
                vec![],
            ) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(capture: &Captured, seed: u64) -> Result<Value, String> {
    if capture.terminal != "Winner(PlayerId(0))"
        || capture.winner != Some(0)
        || capture.rejected != 0
    {
        return Err("Dowsing Shaman duel did not reach a clean seat-zero win".to_owned());
    }
    if capture.shaman_definition != expected_definition()
        || capture.activation_binding != expected_binding()
    {
        return Err(
            "Dowsing Shaman exact printed identity or activated ability differs from RAV #159"
                .to_owned(),
        );
    }
    let shaman = capture.shaman.ok_or("Dowsing Shaman was not cast")?;
    let elf = capture.elf.ok_or("the Aura target Elf was not cast")?;
    let fists = capture.fists.ok_or("Fists of Ironwood was not cast")?;
    let blocker = capture.blocker.ok_or("opponent Watchwolf was not cast")?;
    if capture.target_zone_at_activation != Some(Zone::Graveyard)
        || capture.target_zone_at_end != Some(Zone::Hand)
        || capture.activation_targets != [Target::Permanent(fists)]
        || !capture.shaman_tapped_at_activation
        || capture.shaman_cast_payment.len() != 5
        || !capture.shaman_cast_payment.contains(&Color::Green)
        || capture.fists_cast_payment.len() != 2
        || !capture.fists_cast_payment.contains(&Color::Green)
        || capture.activation_payment.as_ref() != Some(&ManaCost::with_colors(2, [Color::Green]))
    {
        return Err(format!(
            "Dowsing Shaman did not pay the exact costs and return the exact graveyard Aura: target={:?}, activation_target={:?}, activation_cost={:?}",
            capture.target_zone_at_activation,
            capture.activation_targets,
            capture.activation_payment
        ));
    }
    let events = &capture.events;
    let shaman_paid = event_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == shaman && colors.len() == 5 && colors.contains(&Color::Green))).ok_or("Dowsing Shaman exact {4}{G} casting receipt missing")?;
    let shaman_cast = event_index_after(events, shaman_paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == shaman)).ok_or("Dowsing Shaman spell cast receipt missing")?;
    let shaman_resolved = event_index_after(
        events,
        shaman_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == shaman),
    )
    .ok_or("Dowsing Shaman spell resolution receipt missing")?;
    let shaman_entered = event_index_after(events, shaman_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == shaman)).ok_or("Dowsing Shaman battlefield entry receipt missing")?;
    let fists_paid = event_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == fists && colors.len() == 2 && colors.contains(&Color::Green))).ok_or("Fists of Ironwood exact {1}{G} payment receipt missing")?;
    let fists_cast = event_index_after(events, fists_paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == fists)).ok_or("Fists of Ironwood cast receipt missing")?;
    let fists_resolved = event_index_after(
        events,
        fists_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == fists),
    )
    .ok_or("Fists of Ironwood resolution receipt missing")?;
    let fists_entered = event_index_after(events, fists_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == fists)).ok_or("Fists of Ironwood did not enter the battlefield")?;
    let attached = event_index_after(events, fists_entered, |event| matches!(event, GameEvent::AuraAttached { aura, target } if *aura == fists && *target == elf)).ok_or("Fists of Ironwood was not attached to the exact Elf object")?;
    let attack = event_index_after(events, attached, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&elf))).ok_or("the Aura-attached Elf did not attack")?;
    let block = event_index_after(events, attack, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if assignments.contains(&(elf, blocker)))).ok_or("the exact opposing Watchwolf did not block the enchanted Elf")?;
    let elf_damage = event_index_after(events, block, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 1 } if *source == elf && *permanent == blocker)).ok_or("enchanted Elf combat damage receipt missing")?;
    let blocker_damage = event_index_after(events, block, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 3 } if *source == blocker && *permanent == elf)).ok_or("Watchwolf combat damage receipt missing")?;
    let aura_to_graveyard = event_index_after(events, blocker_damage, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == fists)).ok_or("Fists of Ironwood did not go to the controller's graveyard when its enchanted Elf died")?;
    let ability_paid = event_index_after(events, aura_to_graveyard, |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: ABILITY_ID, mana_cost } if *source == shaman && *mana_cost == ManaCost::with_colors(2, [Color::Green]))).ok_or("Dowsing Shaman exact {2}{G} activated payment receipt missing")?;
    let activated = event_index_after(events, ability_paid, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source, definition: CARD_ID, ability: ABILITY_ID, .. } if *source == shaman)).ok_or("Dowsing Shaman activation did not enter the stack")?;
    let returned = event_index_after(
        events,
        activated,
        |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == fists),
    )
    .ok_or_else(|| {
        let relevant = events
            .iter()
            .filter(|event| {
                matches!(event, GameEvent::CardMoved { card, .. } if *card == fists)
                    || matches!(event, GameEvent::AbilityActivated { source, ability: ABILITY_ID, .. } if *source == shaman)
                    || matches!(event, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == shaman)
            })
            .map(|event| format!("{event:?}"))
            .collect::<Vec<_>>();
        format!("the exact Fists of Ironwood card did not move to its owner's hand; fists={fists:?}, shaman={shaman:?}, target={:?}, target_zone={:?}, relevant={relevant:?}", capture.activation_targets, capture.target_zone_at_activation)
    })?;
    let resolved = event_index_after(events, returned, |event| matches!(event, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == shaman)).ok_or("Dowsing Shaman return ability did not resolve")?;
    let end = event_index_after(events, resolved, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal seat-zero win receipt missing after the recursion")?;
    if !(shaman_paid < shaman_cast
        && shaman_cast < shaman_resolved
        && shaman_resolved < shaman_entered
        && fists_paid < fists_cast
        && fists_cast < fists_resolved
        && fists_resolved < fists_entered
        && fists_entered < attached
        && attached < attack
        && attack < block
        && block < elf_damage
        && block < blocker_damage
        && blocker_damage < aura_to_graveyard
        && aura_to_graveyard < ability_paid
        && ability_paid < activated
        && activated < returned
        && returned < resolved
        && returned < end)
    {
        return Err("Dowsing Shaman cast, Aura death, exact activation, return, and terminal events are out of order".to_owned());
    }
    let selected = BTreeSet::from([
        shaman_paid,
        shaman_cast,
        shaman_resolved,
        shaman_entered,
        fists_paid,
        fists_cast,
        fists_resolved,
        fists_entered,
        attached,
        attack,
        block,
        elf_damage,
        blocker_damage,
        aura_to_graveyard,
        ability_paid,
        activated,
        resolved,
        returned,
        end,
    ]);
    let projected: Vec<_> = project_events(events)
        .into_iter()
        .enumerate()
        .filter_map(|(index, event)| selected.contains(&index).then_some(event))
        .collect();
    Ok(json!({
        "id":"rav-dowsing-shaman-return-enchantment-from-own-graveyard-full-game",
        "seed":seed,
        "terminal":capture.terminal,
        "winner_seat":capture.winner,
        "turns":capture.turns,
        "accepted_moves":capture.accepted,
        "rejected_moves":capture.rejected,
        "dowsing_shaman":{"object":shaman.0,"collector_number":159,"definition_id":CARD_ID,"mana_cost":"{4}{G}","mana_paid":capture.shaman_cast_payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"power":3,"toughness":4},
        "ability":{"id":ABILITY_ID,"mana_cost":"{2}{G}","tap_cost_paid":capture.shaman_tapped_at_activation,"target_object":fists.0,"target_definition":FISTS_ID,"target_zone_at_activation":"Graveyard","result_zone":"Hand"},
        "setup":{"aura_object":fists.0,"aura_attached_to":elf.0,"elf_blocked_by":blocker.0,"target_card_died_and_aura_entered_own_graveyard":true},
        "events":projected,
        "events_digest":event_digest(events),
        "opponent_life_at_terminal":capture.life[1],
        "passed":true
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
fn event_digest(events: &[GameEvent]) -> String {
    let bytes = events
        .iter()
        .map(|event| format!("{event:?}\n"))
        .collect::<String>();
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes.bytes() {
        hash = (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}
