//! Full-game RAV Golgari Germination controlled-creature dies-trigger authority.

use std::collections::{BTreeSet, HashSet};
use std::process::ExitCode;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, TokenSpec,
    TriggerCondition, TriggeredAbilityBinding, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-GOLGARI-GERMINATION";
const ELF_ID: &str = "RAV-ELVES-OF-DEEP-SHADOW";
const SUPPORT_ID: &str = "RAV-WATCHWOLF";
const ABILITY_ID: &str = "controlled-nontoken-creature-dies-create-saproling";
const MAX_TRACE_SEED: u64 = 256;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/golgari_germination.rs"]
mod candidate_golgari_germination;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Golgari Germination result: {error}");
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
            eprintln!("Golgari Germination PvP authority failed: {error}");
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
    WrongCost,
    RemoveTrigger,
    WrongCondition,
    WrongToken,
}

fn candidate_values(mutation: Mutation) -> (CardDefinition, Option<TriggeredAbilityBinding>) {
    let mut definition = candidate_golgari_germination::definition();
    let mut trigger = candidate_golgari_germination::triggered_ability();
    match mutation {
        Mutation::None | Mutation::RemoveTrigger => {}
        Mutation::WrongCost => {
            definition.mana_cost = ManaCost::with_colors(0, [Color::Black, Color::Green]);
        }
        Mutation::WrongCondition => {
            trigger.ability.condition = TriggerCondition::Dies;
        }
        Mutation::WrongToken => {
            trigger.ability.effects = vec![Effect::CreateToken {
                token: TokenSpec::white_spirit(),
                count: 1,
            }];
        }
    }
    (
        definition,
        (!matches!(mutation, Mutation::RemoveTrigger)).then_some(trigger),
    )
}

fn validate_candidate_source() -> Result<(), String> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../sets/ravnica_city_of_guilds/src/cards/golgari_germination.rs");
    let source = std::fs::read_to_string(&path)
        .map_err(|error| format!("could not read Golgari Germination candidate module: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Golgari Germination module is not valid Rust source".to_owned())?;
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
        "id:\"RAV-GOLGARI-GERMINATION\"",
        "name:\"GolgariGermination\"",
        "mana_cost:ManaCost::with_colors(1,[Color::Black,Color::Green])",
        "card_types:BTreeSet::from([CardType::Enchantment])",
        "id:\"controlled-nontoken-creature-dies-create-saproling\"",
        "condition:TriggerCondition::ControlledNontokenCreatureDies",
        "optional:false",
        "Effect::CreateToken{token:TokenSpec::saproling(),count:1,}",
    ];
    if function_names != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()])
        || required.iter().any(|fragment| !compact.contains(fragment))
    {
        return Err(
            "Golgari Germination candidate definition or trigger is outside its literal source contract"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_runtime() -> Result<(), String> {
    let definition = candidate_golgari_germination::definition();
    let trigger = candidate_golgari_germination::triggered_ability();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Golgari Germination definition is missing from the RAV registry")?;
    let registered_trigger = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Golgari Germination trigger is missing from the RAV registry")?;
    if registered != definition || registered_trigger != trigger {
        return Err("Golgari Germination is not delegated to its candidate-owned module".into());
    }
    if definition.id != CARD_ID
        || definition.name != "Golgari Germination"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(1, [Color::Black, Color::Green])
        || definition.colors != BTreeSet::from([Color::Black, Color::Green])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Enchantment])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
        || trigger.ability.id != ABILITY_ID
        || trigger.ability.condition != TriggerCondition::ControlledNontokenCreatureDies
        || trigger.ability.mana_cost != ManaCost::new(0)
        || trigger.ability.optional
        || !trigger.ability.targets.is_empty()
        || trigger.ability.effects
            != [Effect::CreateToken {
                token: TokenSpec::saproling(),
                count: 1,
            }]
    {
        return Err(
            "Golgari Germination identity or Saproling dies trigger differs from RAV #209".into(),
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
    let (seed, capture) = find_trace()?;
    let trace = validate_trace(&capture, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if capture.events != replay.events
        || capture.terminal != replay.terminal
        || capture.winner != replay.winner
        || capture.germination != replay.germination
        || capture.victim != replay.victim
        || capture.blocker != replay.blocker
        || capture.first_token != replay.first_token
    {
        return Err("Golgari Germination shuffled-game replay was not deterministic".into());
    }
    let mutations = json!({
        "casting_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongCost),
        "trigger_removal_detected_in_game": mutation_detected(seed, Mutation::RemoveTrigger),
        "trigger_condition_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongCondition),
        "saproling_token_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongToken),
        "sealed_mutation_authority": false
    });
    for key in [
        "casting_cost_mutation_detected_in_game",
        "trigger_removal_detected_in_game",
        "trigger_condition_mutation_detected_in_game",
        "saproling_token_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Golgari Germination semantic mutation escaped the full-game trace: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog
        .iter()
        .map(|card| {
            let id = card_definitions()
                .into_iter()
                .find(|definition| definition.name == card.name)
                .map(|definition| definition.id)
                .unwrap_or("");
            json!({
                "collector_number": card.collector_number,
                "name": card.name,
                "definition_id": id,
                "engine_status": if card.semantic_status.is_executable() { "executable_compatibility_slice" } else { "catalog_only" },
                "trace_status": if id == CARD_ID { "passed" } else { "not_qualified" },
                "qualification": if id == CARD_ID { "unreviewed_diagnostic" } else { "not_evaluated" },
                "credited": false
            })
        })
        .collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 209, "definition_id": CARD_ID, "name": "Golgari Germination"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": ["110.5 Tokens", "117 Priority", "302.1 Creature characteristics", "603.2 Trigger events", "603.6c Leaves-the-battlefield triggers", "608.2 Resolving abilities", "704 State-based actions", "111 Token characteristics"],
            "source_contract": "RAV Golgari Germination #209, {1}{B}{G} black-green enchantment; whenever a nontoken creature you control dies, create one green 1/1 Saproling",
            "authority_cutoff": "2026-09-23",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": 1,
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

#[derive(Clone, Debug)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    germination: Option<ObjectId>,
    mana_paid: Vec<Color>,
    victim: Option<ObjectId>,
    blocker: Option<ObjectId>,
    first_token: Option<ObjectId>,
    first_token_characteristics: Option<(
        BTreeSet<Color>,
        BTreeSet<cardbench_magic_engine::CreatureSubtype>,
        BTreeSet<CardType>,
        i32,
        i32,
        String,
    )>,
    p0_creature_cards: HashSet<ObjectId>,
    p1_support_cards: HashSet<ObjectId>,
    life: [i64; 2],
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_GOLGARI_GERMINATION_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_GOLGARI_GERMINATION_SEED must be unsigned")?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let Ok(capture) = run_match(seed, Mutation::None) {
            if validate_trace(&capture, seed).is_ok() {
                return Ok((seed, capture));
            }
        }
    }
    Err(format!(
        "no Golgari Germination full-game trigger trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation) {
        Ok(capture) => validate_trace(&capture, seed).is_err(),
        Err(_) => true,
    }
}

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let (definition, trigger) = candidate_values(mutation);
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Golgari Germination is missing from the RAV registry")? = definition;
    let mut triggers = rav_triggered_ability_bindings();
    triggers.retain(|binding| binding.card_definition != CARD_ID);
    if let Some(trigger) = trigger {
        triggers.push(trigger);
    }
    Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers,
    )
    .map_err(|error| error.to_string())
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let (candidate_definition, _) = candidate_values(mutation);
    let mut game = new_game(mutation)?;
    let p0 = DeckList {
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
                card: SUPPORT_ID.into(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".into(),
                count: 16,
            },
            DeckEntry {
                card: "RAV-SWAMP".into(),
                count: 16,
            },
            DeckEntry {
                card: "RAV-PLAINS".into(),
                count: 16,
            },
        ],
        sideboard: vec![],
    };
    let p1 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: SUPPORT_ID.into(),
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
        return Err("Golgari Germination duel decks must each contain 60 cards".into());
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
    let germination_cost = candidate_definition.mana_cost;
    let elf_cost = card_definitions()
        .into_iter()
        .find(|card| card.id == ELF_ID)
        .ok_or("Elves of Deep Shadow is missing")?
        .mana_cost;
    let support_cost = card_definitions()
        .into_iter()
        .find(|card| card.id == SUPPORT_ID)
        .ok_or("Watchwolf is missing")?
        .mana_cost;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(GerminationPolicy::new(
            PlayerId(0),
            &germination_cost,
            &elf_cost,
            &support_cost,
        )),
        Box::new(GerminationPolicy::new(
            PlayerId(1),
            &germination_cost,
            &elf_cost,
            &support_cost,
        )),
    ];
    let (mut accepted, mut attempted) = (0_u32, 0_u32);
    let (mut germination, mut victim, mut blocker, mut first_token) = (None, None, None, None);
    let mut mana_paid = Vec::new();
    let mut first_token_characteristics = None;
    let mut p0_creature_cards = HashSet::new();
    let mut p1_support_cards = HashSet::new();
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Golgari Germination match exceeded bounded move count".into());
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
        let before = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Golgari Germination policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("Golgari Germination invariant failed: {error}"))?;
        for event in &game.event_log[before..] {
            match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    germination = Some(*card);
                    mana_paid = colors.clone();
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    germination = Some(*card);
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } => {
                    if game.card_definition(*card).is_ok_and(|definition| {
                        definition.id == ELF_ID || definition.id == SUPPORT_ID
                    }) {
                        p0_creature_cards.insert(*card);
                    }
                    if victim.is_none()
                        && game
                            .card_definition(*card)
                            .is_ok_and(|definition| definition.id == ELF_ID)
                    {
                        victim = Some(*card);
                    }
                }
                GameEvent::SpellCast {
                    player: PlayerId(1),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == SUPPORT_ID) =>
                {
                    p1_support_cards.insert(*card);
                }
                GameEvent::BlockersDeclared {
                    player: PlayerId(1),
                    assignments,
                } if blocker.is_none() => {
                    if let Some(victim) = victim {
                        blocker = assignments.iter().find_map(|(attacker, blocker)| {
                            (*attacker == victim).then_some(*blocker)
                        });
                    }
                }
                GameEvent::TokenCreated {
                    player: PlayerId(0),
                    token,
                } if first_token.is_none() => {
                    first_token = Some(*token);
                    let characteristics = game
                        .characteristics(*token)
                        .map_err(|error| error.to_string())?;
                    let spec = game
                        .object(*token)
                        .map_err(|error| error.to_string())?
                        .token
                        .as_ref()
                        .ok_or("Golgari Germination token receipt was not a token")?;
                    first_token_characteristics = Some((
                        characteristics.colors,
                        characteristics.creature_subtypes,
                        characteristics.card_types,
                        characteristics.power.ok_or("Saproling has no power")?,
                        characteristics
                            .toughness
                            .ok_or("Saproling has no toughness")?,
                        spec.name.to_owned(),
                    ));
                }
                _ => {}
            }
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
            || "Draw".into(),
            |seat| format!("Winner(PlayerId({}))", seat.0),
        ),
        winner: game.winner().map(|seat| seat.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        germination,
        mana_paid,
        victim,
        blocker,
        first_token,
        first_token_characteristics,
        p0_creature_cards,
        p1_support_cards,
        life,
    })
}

struct GerminationPolicy {
    player: PlayerId,
    germination_cost: ManaCost,
    elf_cost: ManaCost,
    support_cost: ManaCost,
    saproling_blocked_once: bool,
}

impl GerminationPolicy {
    fn new(
        player: PlayerId,
        germination_cost: &ManaCost,
        elf_cost: &ManaCost,
        support_cost: &ManaCost,
    ) -> Self {
        Self {
            player,
            germination_cost: germination_cost.clone(),
            elf_cost: elf_cost.clone(),
            support_cost: support_cost.clone(),
            saproling_blocked_once: false,
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
        let needed = cost
            .colored
            .iter()
            .find(|color| view.mana_pool.amount(**color) == 0)
            .copied()
            .or_else(|| {
                (view.mana_pool.total() < cost.generic.saturating_add(cost.colored.len() as u8))
                    .then_some(Color::Green)
            })?;
        let land = view
            .own_battlefield
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land)
                    && !card.tapped
                    && card.mana_colors.contains(&needed)
            })
            .or_else(|| {
                view.own_battlefield.iter().find(|card| {
                    card.card_types.contains(&CardType::Land)
                        && !card.tapped
                        && !card.mana_colors.is_empty()
                })
            })?;
        let color = if land.mana_colors.contains(&needed) {
            needed
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
                request: cardbench_magic_engine::CastRequest {
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
}

impl CodePolicy for GerminationPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.golgari-germination-trigger.v1"
        } else {
            "rav.watchwolf-defender.v1"
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
            let attackers: Vec<_> = if self.player == PlayerId(0) {
                let has_saproling = view
                    .own_battlefield
                    .iter()
                    .any(|card| card.definition.is_none());
                let elves: Vec<_> = view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(ELF_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect();
                let can_force_the_death_trigger = view
                    .opponent_battlefield
                    .iter()
                    .any(|card| card.definition == Some(SUPPORT_ID) && card.can_block);
                if !has_saproling && can_force_the_death_trigger && !elves.is_empty() {
                    vec![elves[0]]
                } else if !has_saproling {
                    vec![]
                } else {
                    view.own_battlefield
                        .iter()
                        .filter(|card| card.can_attack)
                        .map(|card| card.id)
                        .collect()
                }
            } else {
                vec![]
            };
            return PolicyAction::DeclareAttackers { attackers };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            if self.player != PlayerId(1) {
                return PolicyAction::DeclareBlockers {
                    assignments: vec![],
                };
            }
            let blocker = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(SUPPORT_ID) && card.can_block);
            let assignment = blocker.and_then(|blocker| {
                let token_attacker = view
                    .combat_attackers
                    .iter()
                    .find(|card| card.definition.is_none());
                let attacker = if !self.saproling_blocked_once {
                    token_attacker.inspect(|_| self.saproling_blocked_once = true)
                } else {
                    None
                }
                .or_else(|| {
                    view.combat_attackers
                        .iter()
                        .find(|card| card.definition == Some(ELF_ID))
                })?;
                Some(cardbench_magic_engine::CombatBlock {
                    attacker: attacker.id,
                    blocker: blocker.id,
                })
            });
            return PolicyAction::DeclareBlockers {
                assignments: assignment.into_iter().collect(),
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
        let has_germination = view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some(CARD_ID));
        if self.player == PlayerId(0) {
            if !has_germination {
                return self
                    .make_cast(view, CARD_ID, &self.germination_cost)
                    .unwrap_or(PolicyAction::PassPriority);
            }
            if !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(ELF_ID))
            {
                if let Some(action) = self.make_cast(view, ELF_ID, &self.elf_cost) {
                    return action;
                }
            }
            if !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(SUPPORT_ID))
            {
                if let Some(action) = self.make_cast(view, SUPPORT_ID, &self.support_cost) {
                    return action;
                }
            }
        } else if !view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some(SUPPORT_ID))
        {
            if let Some(action) = self.make_cast(view, SUPPORT_ID, &self.support_cost) {
                return action;
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
        return Err(
            "Golgari Germination match did not reach a clean seat-zero terminal win".into(),
        );
    }
    let germination = c
        .germination
        .ok_or("seat zero did not cast Golgari Germination")?;
    let victim = c
        .victim
        .ok_or("seat zero did not cast an Elves of Deep Shadow victim")?;
    let blocker = c.blocker.ok_or_else(|| {
        let attackers: Vec<_> = c
            .events
            .iter()
            .filter_map(|event| match event {
                GameEvent::AttackersDeclared { player, attackers } => {
                    Some((*player, attackers.clone()))
                }
                _ => None,
            })
            .collect();
        let blocks: Vec<_> = c
            .events
            .iter()
            .filter_map(|event| match event {
                GameEvent::BlockersDeclared { player, assignments } => {
                    Some((*player, assignments.clone()))
                }
                _ => None,
            })
            .collect();
        format!(
            "opponent did not assign a blocker to the controlled victim: victim={:?}, p1_watchwolves={:?}, attackers={attackers:?}, blocks={blocks:?}",
            c.victim, c.p1_support_cards
        )
    })?;
    let token = c
        .first_token
        .ok_or("Golgari Germination did not create a token")?;
    if !c.p1_support_cards.contains(&blocker) {
        return Err("controlled victim was not blocked by an opponent Watchwolf".into());
    }
    if c.mana_paid.len() != 3
        || !c.mana_paid.contains(&Color::Black)
        || !c.mana_paid.contains(&Color::Green)
    {
        return Err(format!(
            "Golgari Germination did not pay exact {{1}}{{B}}{{G}}: {:?}",
            c.mana_paid
        ));
    }
    let (colors, subtypes, types, power, toughness, name) = c
        .first_token_characteristics
        .as_ref()
        .ok_or("Saproling characteristics were not captured")?;
    if colors != &BTreeSet::from([Color::Green])
        || subtypes != &BTreeSet::from([cardbench_magic_engine::CreatureSubtype::Saproling])
        || types != &BTreeSet::from([CardType::Creature])
        || (*power, *toughness) != (1, 1)
        || name != "Saproling"
    {
        return Err("Golgari Germination did not create one green 1/1 Saproling".into());
    }
    let events = &c.events;
    let paid = event_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == germination && colors.len() == 3 && colors.contains(&Color::Black) && colors.contains(&Color::Green))).ok_or("Golgari Germination payment receipt missing")?;
    let cast = event_index_after(events, paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == germination)).ok_or("Golgari Germination cast receipt missing")?;
    let resolved = event_index_after(
        events,
        cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == germination),
    )
    .ok_or("Golgari Germination did not resolve")?;
    let entered = event_index_after(events, resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == germination)).ok_or("Golgari Germination did not enter the battlefield")?;
    let victim_attack = event_index_after(events, entered, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&victim))).ok_or("controlled nontoken victim did not attack")?;
    let block = event_index_after(events, victim_attack, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if assignments.iter().any(|(attacker, assigned)| *attacker == victim && *assigned == blocker))).ok_or("available blocker did not block the controlled victim")?;
    let died = event_index_after(events, block, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == victim)).ok_or("controlled nontoken creature did not die")?;
    let stacked = event_index_after(events, died, |event| matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, ability: ABILITY_ID, .. } if *source == germination)).ok_or("Golgari Germination trigger did not stack after the creature died")?;
    let created = event_index_after(events, stacked, |event| matches!(event, GameEvent::TokenCreated { player: PlayerId(0), token: created } if *created == token)).ok_or("Saproling token creation receipt missing")?;
    let ability_resolved = event_index_after(events, created, |event| matches!(event, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == germination)).ok_or("Golgari Germination trigger did not resolve")?;
    let token_attack = event_index_after(events, ability_resolved, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&token))).ok_or("created Saproling did not attack in the legal game")?;
    let token_block = event_index_after(events, token_attack, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if assignments.iter().any(|(attacker, _)| *attacker == token))).ok_or("the created Saproling was not blocked")?;
    let token_left = event_index_after(events, token_block, |event| matches!(event, GameEvent::TokenCeasedToExist { token: departed } if *departed == token)).ok_or("the Saproling did not leave after combat damage")?;
    let next_nontoken_death = events.iter().enumerate().skip(token_left + 1).find_map(|(index, event)| {
        matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if c.p0_creature_cards.contains(card)).then_some(index)
    }).unwrap_or(events.len());
    let token_death_retrigger = events.iter().enumerate().skip(token_left + 1).take(next_nontoken_death.saturating_sub(token_left + 1)).any(|(_, event)| {
        matches!(event, GameEvent::TriggeredAbilityStacked { source, ability: ABILITY_ID, .. } if *source == germination)
    });
    if token_death_retrigger {
        return Err("a token death incorrectly triggered Golgari Germination".into());
    }
    let ended = event_index(events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal seat-zero victory receipt missing")?;
    if !(paid < cast
        && cast < resolved
        && resolved < entered
        && entered < victim_attack
        && victim_attack < block
        && block < died
        && died < stacked
        && stacked < created
        && created < ability_resolved
        && ability_resolved < token_attack
        && token_attack < token_block
        && token_block < token_left
        && token_left < ended)
    {
        return Err("Golgari Germination cast, death trigger, Saproling, combat, and terminal events are out of order".into());
    }
    let all_projected = project_events(events);
    let selected = BTreeSet::from([
        paid,
        cast,
        resolved,
        entered,
        victim_attack,
        block,
        died,
        stacked,
        created,
        ability_resolved,
        token_attack,
        token_block,
        token_left,
        ended,
    ]);
    let projected: Vec<ProjectedEvent> = all_projected
        .iter()
        .enumerate()
        .filter_map(|(index, event)| selected.contains(&index).then_some(event.clone()))
        .collect();
    let damage_total = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if *source != token => Some(i64::from(*amount)),
            _ => None,
        })
        .sum::<i64>();
    if c.life[1] > 0 || c.life[0] <= 0 || damage_total < 20 {
        return Err(format!(
            "legal game did not finish with seat zero damage: terminal={}, turns={}, accepted={}, life={:?}, non-token-damage={}, terminal-events={:?}, damage-events={:?}",
            c.terminal,
            c.turns,
            c.accepted,
            c.life,
            damage_total,
            events
                .iter()
                .filter(|event| matches!(event, GameEvent::GameEnded { .. }))
                .collect::<Vec<_>>(),
            events
                .iter()
                .filter(|event| matches!(event, GameEvent::DamageDealtToPlayer { .. }))
                .collect::<Vec<_>>(),
        ));
    }
    Ok(json!({
        "trace_id": "rav-golgari-germination-controlled-nontoken-dies-pvp",
        "shuffle_seed": seed,
        "decks": ["rav_golgari_germination_saproling_pressure", "rav_forest_watchwolf_defender"],
        "policies": ["rav.golgari-germination-trigger.v1", "rav.watchwolf-defender.v1"],
        "terminal": c.terminal,
        "winner_seat": c.winner,
        "turns": c.turns,
        "accepted_moves": c.accepted,
        "rejected_moves": c.rejected,
        "germination": {"object": germination.0, "collector_number": 209, "mana_cost": "{1}{B}{G}", "mana_paid": c.mana_paid.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>()},
        "victim": {"object": victim.0, "definition": ELF_ID, "controller": 0, "nontoken": true, "attacked_before_dying": true, "blocker": blocker.0, "blocker_definition": SUPPORT_ID},
        "trigger": {"source": germination.0, "ability": ABILITY_ID, "condition": "ControlledNontokenCreatureDies", "controller": 0, "mandatory": true, "stacked_after_death": true, "resolved": true},
        "saproling": {"object": token.0, "name": name, "colors": colors.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(), "subtypes": subtypes.iter().map(|subtype| format!("{subtype:?}")).collect::<Vec<_>>(), "card_types": types.iter().map(|card_type| format!("{card_type:?}")).collect::<Vec<_>>(), "power": power, "toughness": toughness, "attacked": true, "died_without_retrigger": true},
        "combat": {"damage_to_finish_game": damage_total, "life_at_terminal": c.life},
        "events": projected,
        "passed": true
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
