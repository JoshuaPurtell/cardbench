//! Full-game diagnostic authority for the RAV Infectious Host card task.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DeckList, Effect, Game, GameEvent, GameView,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, Zone,
};
use cardbench_magic_policies::{
    CatalogPolicyProfile, CodePolicy, DeckMatchConfig, RavCatalogPolicy,
};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, load_constructed_decks,
    rav_activated_ability_bindings, rav_activated_ability_cost_modifier_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings,
    rav_attachment_triggered_ability_bindings as attachment_triggers, rav_basic_land_type_bindings,
    rav_cost_reduction_bindings, rav_damage_replacement_effect_bindings,
    rav_entry_coin_flip_bindings, rav_entry_copy_bindings,
    rav_generalized_activated_ability_cost_bindings, rav_land_entry_bindings,
    rav_legendary_permanent_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_mana_ability_cost_bindings, rav_replacement_effect_bindings,
    rav_static_attack_restriction_bindings, rav_static_continuous_effect_bindings,
    rav_static_creature_spell_cost_modifier_bindings, rav_static_entry_restriction_bindings,
    rav_static_library_top_reveal_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Item, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/infectious_host.rs";
const DECK_P0: &str = "rav_golgari_midrange";
const DECK_P1: &str = "rav_golgari_midrange";
const MAX_TRACE_SEED: u64 = 64;
const CANDIDATE_ID: &str = "RAV-INFECTIOUS-HOST";
const PUTREFY_ID: &str = "RAV-PUTREFY";
const ABILITY_ID: &str = "dies-target-player-life-loss";

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/infectious_host.rs"]
mod candidate_infectious_host;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Infectious Host result: {error}");
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
            eprintln!("Infectious Host authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_binding()?;
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<CatalogCard> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
    {
        return Err("RAV catalog identity or printing denominator changed".to_owned());
    }

    let (seed, reference) = find_trace()?;
    let trace = validate_trace(&reference, seed)?;
    let replay = run_match(
        seed,
        candidate_infectious_host::definition(),
        Mutation::None,
    )?;
    if reference.events != replay.events
        || reference.terminal != replay.terminal
        || reference.winner != replay.winner
        || reference.host != replay.host
        || reference.putrefy != replay.putrefy
        || reference.targeted_opponent != replay.targeted_opponent
        || project_events(&reference.events) != project_events(&replay.events)
    {
        return Err("Infectious Host full-game replay was not deterministic".to_owned());
    }
    let mutation_sensitivity = json!({
        "body_mutation_detected_in_game": mutation_detected(Mutation::Power, seed),
        "dies_trigger_removal_detected_in_game": mutation_detected(Mutation::RemoveTrigger, seed),
        "one_life_mutation_detected_in_game": mutation_detected(Mutation::OneLife, seed),
        "sealed_mutation_authority": false
    });
    if mutation_sensitivity["body_mutation_detected_in_game"] != true
        || mutation_sensitivity["dies_trigger_removal_detected_in_game"] != true
        || mutation_sensitivity["one_life_mutation_detected_in_game"] != true
    {
        return Err("an Infectious Host semantic mutation escaped the trace oracle".to_owned());
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
        "card": {"collector_number": 91, "definition_id": CANDIDATE_ID, "name": "Infectious Host"},
        "candidate": {"kind": candidate_kind(), "card_definition": CANDIDATE_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117: timing and priority",
                "302.1 and 601.2: creature spells and casting",
                "603.2 and 603.6c: trigger events and dies triggers",
                "603.3 and 603.3d: target selection before a triggered ability is put on the stack",
                "608.2: resolving a targeted triggered ability",
                "119.2: life totals and life loss",
            ],
            "source_contract": "RAV printed identity, black 1/1 body, and mandatory dies trigger targeting a player who loses 2 life",
            "authority_cutoff": "2026-09-23"
        },
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": qualification_status(),
        "mutation_sensitivity": mutation_sensitivity,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Infectious Host source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Infectious Host source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
    {
        return Err("Infectious Host source has an unsupported module attribute".to_owned());
    }
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(item.vis, Visibility::Inherited) => {}
            Item::Fn(item)
                if item.attrs.iter().all(|attribute| {
                    attribute.path().is_ident("must_use") || attribute.path().is_ident("doc")
                }) && matches!(item.vis, Visibility::Restricted(_))
                    && item.sig.inputs.is_empty() =>
            {
                functions.insert(item.sig.ident.to_string());
            }
            _ => {
                return Err(
                    "Infectious Host source contains an unsupported top-level item".to_owned(),
                );
            }
        }
    }
    if functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()]) {
        return Err(
            "Infectious Host must expose only definition() and triggered_ability()".to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_binding() -> Result<(), String> {
    let runtime = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CANDIDATE_ID)
        .ok_or("Infectious Host dies binding is missing from the RAV registry")?;
    if candidate_infectious_host::triggered_ability() != runtime {
        return Err(
            "Infectious Host candidate binding differs from the executable RAV binding".to_owned(),
        );
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Mutation {
    None,
    Power,
    RemoveTrigger,
    OneLife,
}

fn mutation_detected(mutation: Mutation, seed: u64) -> bool {
    let mut definition = candidate_infectious_host::definition();
    if matches!(mutation, Mutation::Power) {
        definition.power = Some(0);
    }
    let expected = match mutation {
        Mutation::Power => "resolved Infectious Host is not a 1/1",
        Mutation::RemoveTrigger => {
            "Infectious Host target was not selected as the opponent after the Host died"
        }
        Mutation::OneLife => "opponent did not lose exactly two life to Infectious Host",
        Mutation::None => return false,
    };
    matches!(run_match(seed, definition, mutation).and_then(|capture| validate_trace(&capture, seed).map(|_| ())), Err(error) if error.contains(expected))
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(seed) = std::env::var("CARDBENCH_INFECTIOUS_HOST_SEED") {
        let seed = seed
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_INFECTIOUS_HOST_SEED must be unsigned".to_owned())?;
        let capture = run_match(
            seed,
            candidate_infectious_host::definition(),
            Mutation::None,
        )?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let Ok(capture) = run_match(
            seed,
            candidate_infectious_host::definition(),
            Mutation::None,
        ) && validate_trace(&capture, seed).is_ok()
        {
            return Ok((seed, capture));
        }
    }
    Err(format!(
        "no Infectious Host full-game trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

#[derive(Clone)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    host: Option<ObjectId>,
    putrefy: Option<ObjectId>,
    mana_paid_host: Vec<Color>,
    mana_paid_putrefy: Vec<Color>,
    host_characteristics: Option<(i32, i32)>,
    target_chosen_after_death: bool,
    targeted_opponent: Option<PlayerId>,
    source_graveyard_before_target: Option<bool>,
    trigger_incarnation: Option<u64>,
    resolved_incarnation: Option<u64>,
    opponent_life_before: Option<i64>,
    opponent_life_after: Option<i64>,
    life_loss_events: Vec<(ObjectId, PlayerId, i16)>,
}

fn task_deck(base: &DeckList) -> Result<DeckList, String> {
    let mut deck = base.clone();
    let entry = deck
        .mainboard
        .iter_mut()
        .find(|entry| entry.card == "RAV-SEWERDREG")
        .ok_or("Golgari deck no longer has a four-card Sewerdreg slot")?;
    if entry.count != 4 {
        return Err(
            "Infectious Host task deck replacement must preserve a four-of slot".to_owned(),
        );
    }
    entry.card = CANDIDATE_ID.to_owned();
    entry.count = 1;
    let forests = deck
        .mainboard
        .iter_mut()
        .find(|entry| entry.card == "RAV-FOREST")
        .ok_or("Golgari deck no longer has its basic Forest slot")?;
    forests.count = forests
        .count
        .checked_add(3)
        .ok_or("Infectious Host task deck Forest count overflow")?;
    let size: usize = deck
        .mainboard
        .iter()
        .map(|entry| usize::from(entry.count))
        .sum();
    if size != 60 {
        return Err(format!(
            "Infectious Host task deck has {size} cards, expected 60"
        ));
    }
    Ok(deck)
}

fn run_match(
    seed: u64,
    definition: CardDefinition,
    mutation: Mutation,
) -> Result<CapturedMatch, String> {
    let decks = load_constructed_decks().map_err(|error| error.to_string())?;
    let source0 = decks
        .iter()
        .find(|deck| deck.id == DECK_P0)
        .ok_or("Golgari deck missing")?;
    let source1 = decks
        .iter()
        .find(|deck| deck.id == DECK_P1)
        .ok_or("Golgari deck missing")?;
    let deck0 = task_deck(&source0.deck)?;
    let deck1 = source1.deck.clone();
    if deck1
        .mainboard
        .iter()
        .map(|entry| usize::from(entry.count))
        .sum::<usize>()
        != 60
    {
        return Err("Infectious Host opponent deck is not 60 cards".to_owned());
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
    let mut game = new_rav_game(definition, mutation)?;
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
        Box::new(InfectiousHostPolicy::new(PlayerId(0))),
        Box::new(PutrefyTaskPolicy::new(PlayerId(1))),
    ];
    let mut accepted_moves = 0_u32;
    let mut host = None;
    let mut putrefy = None;
    let mut mana_paid_host = Vec::new();
    let mut mana_paid_putrefy = Vec::new();
    let mut host_characteristics = None;
    let mut target_chosen_after_death = false;
    let mut targeted_opponent = None;
    let mut source_graveyard_before_target = None;
    let mut trigger_incarnation = None;
    let mut resolved_incarnation = None;
    let mut opponent_life_before = None;
    let mut opponent_life_after = None;
    let mut life_loss_events = Vec::new();

    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted_moves >= config.max_policy_moves {
            return Err("Infectious Host game hit a run bound".to_owned());
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

        let choosing_host_target = matches!(
            &action,
            PolicyAction::ChooseTriggeredAbilityTargets { source, ability, targets, .. }
                if Some(*source) == host && *ability == ABILITY_ID && targets == &[Target::Player(PlayerId(1))]
        );
        if let Some(choice) = &view.triggered_ability_target_choice
            && choice.ability == ABILITY_ID
            && Some(choice.source) == host
        {
            source_graveyard_before_target =
                Some(game.zone_of(choice.source) == Some(Zone::Graveyard));
            opponent_life_before = Some(
                game.player(PlayerId(1))
                    .map_err(|error| error.to_string())?
                    .life,
            );
        }

        let previous_event_count = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| format!("Infectious Host policy move rejected: {error}"))?;
        accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        if choosing_host_target {
            let top = game
                .stack
                .last()
                .ok_or("targeted Infectious Host trigger did not enter the stack")?;
            if top.targets.as_slice() != [Target::Player(PlayerId(1))] {
                return Err(
                    "Infectious Host target decision did not select the opponent".to_owned(),
                );
            }
            target_chosen_after_death = source_graveyard_before_target == Some(true);
            targeted_opponent = Some(PlayerId(1));
        }

        for event in &game.event_log[previous_event_count..] {
            match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|def| def.id == CANDIDATE_ID) =>
                {
                    host = Some(*card);
                    mana_paid_host = colors.clone();
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|def| def.id == CANDIDATE_ID) =>
                {
                    host = Some(*card)
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(1),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|def| def.id == PUTREFY_ID) =>
                {
                    putrefy = Some(*card);
                    mana_paid_putrefy = colors.clone();
                }
                GameEvent::SpellCast {
                    player: PlayerId(1),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|def| def.id == PUTREFY_ID) =>
                {
                    putrefy = Some(*card)
                }
                GameEvent::SpellResolved { card } if Some(*card) == host => {
                    let c = game
                        .characteristics(*card)
                        .map_err(|error| error.to_string())?;
                    host_characteristics = Some((
                        c.power.ok_or("Host has no power")?,
                        c.toughness.ok_or("Host has no toughness")?,
                    ));
                }
                GameEvent::TriggeredAbilityStacked {
                    source,
                    source_incarnation,
                    ability,
                    ..
                } if Some(*source) == host && *ability == ABILITY_ID => {
                    trigger_incarnation = Some(*source_incarnation)
                }
                GameEvent::AbilityResolved {
                    source,
                    source_incarnation,
                    ability,
                } if Some(*source) == host && *ability == ABILITY_ID => {
                    resolved_incarnation = Some(*source_incarnation)
                }
                GameEvent::LifeLost {
                    source,
                    player,
                    amount,
                } if Some(*source) == host => {
                    life_loss_events.push((*source, *player, *amount));
                    opponent_life_after = Some(
                        game.player(*player)
                            .map_err(|error| error.to_string())?
                            .life,
                    );
                }
                _ => {}
            }
        }
    }
    Ok(CapturedMatch {
        events: game.event_log.clone(),
        terminal: game.winner().map_or_else(
            || "Draw".to_owned(),
            |p| format!("Winner(PlayerId({}))", p.0),
        ),
        winner: game.winner().map(|p| p.0),
        turns: game.turn,
        accepted_moves,
        host,
        putrefy,
        mana_paid_host,
        mana_paid_putrefy,
        host_characteristics,
        target_chosen_after_death,
        targeted_opponent,
        source_graveyard_before_target,
        trigger_incarnation,
        resolved_incarnation,
        opponent_life_before,
        opponent_life_after,
        life_loss_events,
    })
}

fn new_rav_game(definition: CardDefinition, mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|card| card.id == CANDIDATE_ID)
        .ok_or("Infectious Host definition missing from RAV registry")? = definition;
    let mut triggers = rav_triggered_ability_bindings();
    if matches!(mutation, Mutation::RemoveTrigger) {
        triggers.retain(|binding| {
            !(binding.card_definition == CANDIDATE_ID && binding.ability.id == ABILITY_ID)
        });
    }
    if matches!(mutation, Mutation::OneLife) {
        let binding = triggers
            .iter_mut()
            .find(|binding| {
                binding.card_definition == CANDIDATE_ID && binding.ability.id == ABILITY_ID
            })
            .ok_or("Infectious Host trigger binding missing")?;
        binding.ability.effects = vec![Effect::LoseLifeTarget { amount: 1 }];
    }
    triggers.extend(attachment_triggers());
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers,
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
    Ok(game)
}

struct InfectiousHostPolicy {
    player: PlayerId,
    candidate_seen: bool,
    inner: RavCatalogPolicy,
}
impl InfectiousHostPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            candidate_seen: false,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
        }
    }
    fn target_decision(&self, view: &GameView) -> Option<PolicyAction> {
        let choice = view.triggered_ability_target_choice.as_ref()?;
        if choice.ability != ABILITY_ID
            || !choice
                .target_options
                .first()?
                .contains(&Target::Player(PlayerId(1)))
        {
            return None;
        }
        Some(PolicyAction::ChooseTriggeredAbilityTargets {
            decision: choice.decision,
            source: choice.source,
            ability: choice.ability,
            targets: vec![Target::Player(PlayerId(1))],
        })
    }
}
impl CodePolicy for InfectiousHostPolicy {
    fn id(&self) -> &'static str {
        "rav.infectious-host-target-opponent.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        let candidate_present = view
            .hand
            .iter()
            .chain(view.own_battlefield.iter())
            .any(|card| card.definition == Some(CANDIDATE_ID));
        self.candidate_seen |= candidate_present;
        if view.step == Step::DeclareAttackers
            && view.player == self.player
            && view.active_player == self.player
            && !view.attackers_declared
            && (!self.candidate_seen || candidate_present)
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
        if view.player == self.player
            && view.priority == self.player
            && view.active_player == self.player
            && view.stack_depth == 0
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            if view.lands_played == 0 {
                let action = self.inner.propose_move(view);
                if matches!(
                    action,
                    PolicyAction::PlayLand { .. }
                        | PolicyAction::PlayLandWithEntryLifePayment { .. }
                ) {
                    return action;
                }
            }
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CANDIDATE_ID))
            {
                let black = view.mana_pool.amount(Color::Black);
                let green = view.mana_pool.amount(Color::Green);
                if black >= 1 && view.mana_pool.total() >= 3 {
                    let generic = if black >= 3 {
                        vec![Color::Black, Color::Black]
                    } else if black >= 2 && green >= 1 {
                        vec![Color::Black, Color::Green]
                    } else if green >= 2 {
                        vec![Color::Green, Color::Green]
                    } else {
                        Vec::new()
                    };
                    if generic.len() == 2 {
                        return PolicyAction::CastWithPayment {
                            request: cardbench_magic_engine::CastRequest {
                                card: card.id,
                                targets: vec![],
                                convoke: vec![],
                                payment_mana_abilities: vec![],
                            },
                            chosen_x: None,
                            mana_selection: ManaPaymentSelection {
                                generic,
                                hybrid: vec![],
                            },
                        };
                    }
                }
                let wanted = if black == 0 {
                    Color::Black
                } else if green == 0 {
                    Color::Green
                } else {
                    Color::Green
                };
                if let Some(land) = view.own_battlefield.iter().find(|land| {
                    !land.tapped
                        && land.card_types.contains(&CardType::Land)
                        && land.mana_colors.contains(&wanted)
                }) {
                    return PolicyAction::ActivateManaAbility {
                        land: land.id,
                        color: wanted,
                    };
                }
            }
        }
        self.inner.propose_move(view)
    }
    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        self.target_decision(view)
            .or_else(|| self.inner.propose_pending_decision(view))
    }
    fn propose_optional_triggered_ability(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_optional_triggered_ability(view)
    }
    fn propose_draw_replacement(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_draw_replacement(view)
    }
    fn propose_private_library_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_private_library_choice(view)
    }
    fn propose_private_opponent_library_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_private_opponent_library_choice(view)
    }
    fn propose_library_search_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_library_search_choice(view)
    }
}

struct PutrefyTaskPolicy {
    player: PlayerId,
    inner: RavCatalogPolicy,
}
impl PutrefyTaskPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            inner: RavCatalogPolicy::new(player, CatalogPolicyProfile::Curve),
        }
    }
    fn putrefy_action(&self, view: &GameView) -> Option<PolicyAction> {
        if view.player != self.player
            || view.decision_player != self.player
            || view.priority != self.player
            || view.stack_depth != 0
        {
            return None;
        }
        let target = view
            .opponent_battlefield
            .iter()
            .find(|card| card.definition == Some(CANDIDATE_ID))?;
        let spell = view
            .hand
            .iter()
            .find(|card| card.definition == Some(PUTREFY_ID))?;
        let black = view.mana_pool.amount(Color::Black);
        let green = view.mana_pool.amount(Color::Green);
        if black >= 1 && green >= 1 && u16::from(black) + u16::from(green) >= 3 {
            let generic = if green >= 2 {
                Color::Green
            } else {
                Color::Black
            };
            return Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
                    card: spell.id,
                    targets: vec![Target::Permanent(target.id)],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection: ManaPaymentSelection {
                    generic: vec![generic],
                    hybrid: vec![],
                },
            });
        }
        let needed = if black == 0 {
            Color::Black
        } else if green == 0 || u16::from(black) + u16::from(green) < 3 {
            Color::Green
        } else {
            return None;
        };
        let land = view.own_battlefield.iter().find(|land| {
            !land.tapped
                && land.card_types.contains(&CardType::Land)
                && land.mana_colors.contains(&needed)
        })?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color: needed,
        })
    }
}
impl CodePolicy for PutrefyTaskPolicy {
    fn id(&self) -> &'static str {
        "rav.infectious-host-golgari-putrefy.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.player == self.player
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
        if view.player == self.player && view.active_player == self.player && view.lands_played == 0
        {
            let action = self.inner.propose_move(view);
            if matches!(
                action,
                PolicyAction::PlayLand { .. } | PolicyAction::PlayLandWithEntryLifePayment { .. }
            ) {
                return action;
            }
        }
        self.putrefy_action(view)
            .unwrap_or_else(|| self.inner.propose_move(view))
    }
    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        self.inner.propose_pending_decision(view)
    }
    fn propose_optional_triggered_ability(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_optional_triggered_ability(view)
    }
    fn propose_draw_replacement(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_draw_replacement(view)
    }
    fn propose_private_library_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_private_library_choice(view)
    }
    fn propose_private_opponent_library_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_private_opponent_library_choice(view)
    }
    fn propose_library_search_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_library_search_choice(view)
    }
}

fn validate_trace(capture: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if capture.winner != Some(0) || capture.terminal != "Winner(PlayerId(0))" {
        return Err("Infectious Host match did not reach a seat-zero win".to_owned());
    }
    if capture.accepted_moves == 0 {
        return Err("Infectious Host game has no accepted moves".to_owned());
    }
    let host = capture.host.ok_or("Infectious Host was not cast")?;
    let putrefy = capture
        .putrefy
        .ok_or("Putrefy was not cast at Infectious Host")?;
    if capture.host_characteristics != Some((1, 1)) {
        return Err("resolved Infectious Host is not a 1/1".to_owned());
    }
    if capture.mana_paid_host.len() != 3 || !capture.mana_paid_host.contains(&Color::Black) {
        return Err("Infectious Host did not pay two generic and one black mana".to_owned());
    }
    if capture.mana_paid_putrefy.len() != 3
        || !capture.mana_paid_putrefy.contains(&Color::Black)
        || !capture.mana_paid_putrefy.contains(&Color::Green)
    {
        return Err("Putrefy did not pay black, green, and generic mana".to_owned());
    }
    if !capture.target_chosen_after_death
        || capture.targeted_opponent != Some(PlayerId(1))
        || capture.source_graveyard_before_target != Some(true)
    {
        return Err(
            "Infectious Host target was not selected as the opponent after the Host died"
                .to_owned(),
        );
    }
    if capture.trigger_incarnation.is_none()
        || capture.trigger_incarnation != capture.resolved_incarnation
    {
        return Err(
            "Infectious Host trigger source incarnation changed before resolution".to_owned(),
        );
    }
    if capture.life_loss_events != [(host, PlayerId(1), 2)]
        || capture
            .opponent_life_before
            .zip(capture.opponent_life_after)
            .is_none_or(|(before, after)| before - after != 2)
    {
        return Err("opponent did not lose exactly two life to Infectious Host".to_owned());
    }
    let p = event_positions(&capture.events, host, putrefy)?;
    if !(p.host_payment < p.host_cast
        && p.host_cast < p.host_resolved
        && p.host_resolved < p.entry
        && p.entry < p.putrefy_payment
        && p.putrefy_payment < p.putrefy_cast
        && p.putrefy_cast < p.destroyed
        && p.destroyed < p.graveyard
        && p.graveyard < p.putrefy_resolved
        && p.putrefy_resolved < p.trigger
        && p.trigger < p.life_loss
        && p.life_loss < p.ability_resolved
        && p.ability_resolved < p.game_end)
    {
        return Err("Infectious Host cast, death, target trigger, life loss, or terminal event order differs".to_owned());
    }
    let projection = project_events(&capture.events);
    let selected = [
        p.host_payment,
        p.host_cast,
        p.host_resolved,
        p.entry,
        p.putrefy_payment,
        p.putrefy_cast,
        p.destroyed,
        p.graveyard,
        p.putrefy_resolved,
        p.trigger,
        p.life_loss,
        p.ability_resolved,
        p.game_end,
    ];
    let projected: Vec<_> = selected.iter().map(|i| projection[*i].clone()).collect();
    let expected = [
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "CardMoved",
        "SpellManaPaid",
        "SpellCast",
        "CardDestroyed",
        "CardMoved",
        "SpellResolved",
        "TriggeredAbilityStacked",
        "LifeLost",
        "AbilityResolved",
        "GameEnded",
    ];
    if projected
        .iter()
        .map(|e| e.kind.as_str())
        .collect::<Vec<_>>()
        != expected
    {
        return Err("Infectious Host typed event trace differs".to_owned());
    }
    let facts: Vec<_> = projected.iter().map(|e| &e.facts).collect();
    let has_objects = |index: usize, expected: &[u64]| {
        facts[index]
            .objects
            .iter()
            .map(|object| object.0)
            .eq(expected.iter().copied())
    };
    let has_seats = |index: usize, expected: &[u16]| {
        facts[index]
            .seats
            .iter()
            .map(|seat| seat.0)
            .eq(expected.iter().copied())
    };
    if !has_objects(0, &[host.0])
        || !has_seats(0, &[0])
        || facts[0].mana_colors.len() != 3
        || !facts[0].mana_colors.contains(&"Black".to_owned())
        || !has_objects(1, &[host.0])
        || !has_seats(1, &[0])
        || !has_objects(2, &[host.0])
        || !has_objects(3, &[host.0])
        || facts[3].zone.as_deref() != Some("Battlefield")
        || !has_objects(4, &[putrefy.0])
        || !has_seats(4, &[1])
        || facts[4].mana_colors.len() != 3
        || !facts[4].mana_colors.contains(&"Black".to_owned())
        || !facts[4].mana_colors.contains(&"Green".to_owned())
        || !has_objects(5, &[putrefy.0])
        || !has_seats(5, &[1])
        || !has_objects(6, &[putrefy.0, host.0])
        || !has_objects(7, &[host.0])
        || facts[7].zone.as_deref() != Some("Graveyard")
        || !has_objects(8, &[putrefy.0])
        || !has_objects(9, &[host.0])
        || facts[9].ability.as_deref() != Some(ABILITY_ID)
        || facts[9].incarnation != capture.trigger_incarnation
        || !has_objects(10, &[host.0])
        || !has_seats(10, &[1])
        || facts[10].amount != Some(2)
        || !has_objects(11, &[host.0])
        || facts[11].ability.as_deref() != Some(ABILITY_ID)
        || facts[11].incarnation != facts[9].incarnation
        || !has_seats(12, &[0])
    {
        return Err(
            "Infectious Host typed payment, target life-loss, or incarnation facts differ"
                .to_owned(),
        );
    }
    Ok(json!({
        "id":"rav-infectious-host-targeted-dies-life-loss",
        "seed":seed,
        "decks":[format!("{DECK_P0}+1-infectious-host"),DECK_P1],
        "policies":["rav.infectious-host-target-opponent.v1","rav.infectious-host-golgari-putrefy.v1"],
        "terminal":capture.terminal,"winner_seat":capture.winner,"turns":capture.turns,"accepted_moves":capture.accepted_moves,"rejected_moves":0,
        "digest":event_digest(&capture.events),"host_object":host.0,"putrefy_object":putrefy.0,
        "mana_paid":{"host":capture.mana_paid_host.iter().map(|c|format!("{c:?}")).collect::<Vec<_>>(),"putrefy":capture.mana_paid_putrefy.iter().map(|c|format!("{c:?}")).collect::<Vec<_>>()},
        "resolved_characteristics":{"power":1,"toughness":1},
        "targeted_trigger":{"source_in_graveyard_before_target":capture.source_graveyard_before_target,"target_seat":capture.targeted_opponent.map(|p|p.0),"source_incarnation":capture.trigger_incarnation,"resolved_incarnation":capture.resolved_incarnation,"opponent_life_before":capture.opponent_life_before,"opponent_life_after":capture.opponent_life_after,"life_loss":2},
        "events":projected,"passed":true
    }))
}

struct EventPositions {
    host_payment: usize,
    host_cast: usize,
    host_resolved: usize,
    entry: usize,
    putrefy_payment: usize,
    putrefy_cast: usize,
    putrefy_resolved: usize,
    destroyed: usize,
    graveyard: usize,
    trigger: usize,
    life_loss: usize,
    ability_resolved: usize,
    game_end: usize,
}
fn event_positions(
    events: &[GameEvent],
    host: ObjectId,
    putrefy: ObjectId,
) -> Result<EventPositions, String> {
    let pos = |predicate: &dyn Fn(&GameEvent) -> bool, start: usize, label: &str| {
        events
            .iter()
            .enumerate()
            .skip(start)
            .find_map(|(i, e)| predicate(e).then_some(i))
            .ok_or_else(|| format!("{label} event missing"))
    };
    let host_payment = pos(
        &|e| matches!(e,GameEvent::SpellManaPaid{player:PlayerId(0),card,colors}if *card==host&&colors.len()==3&&colors.contains(&Color::Black)),
        0,
        "Host payment",
    )?;
    let host_cast = pos(
        &|e| matches!(e,GameEvent::SpellCast{player:PlayerId(0),card}if *card==host),
        host_payment + 1,
        "Host cast",
    )?;
    let host_resolved = pos(
        &|e| matches!(e,GameEvent::SpellResolved{card}if *card==host),
        host_cast + 1,
        "Host resolution",
    )?;
    let entry = pos(
        &|e| matches!(e,GameEvent::CardMoved{card,to:Zone::Battlefield}if *card==host),
        host_resolved + 1,
        "Host entry",
    )?;
    let putrefy_payment = pos(
        &|e| matches!(e,GameEvent::SpellManaPaid{player:PlayerId(1),card,colors}if *card==putrefy&&colors.len()==3&&colors.contains(&Color::Black)&&colors.contains(&Color::Green)),
        entry + 1,
        "Putrefy payment",
    )?;
    let putrefy_cast = pos(
        &|e| matches!(e,GameEvent::SpellCast{player:PlayerId(1),card}if *card==putrefy),
        putrefy_payment + 1,
        "Putrefy cast",
    )?;
    let destroyed = pos(
        &|e| matches!(e,GameEvent::CardDestroyed{source,card}if *source==putrefy&&*card==host),
        putrefy_cast + 1,
        "Host destruction",
    )?;
    let graveyard = pos(
        &|e| matches!(e,GameEvent::CardMoved{card,to:Zone::Graveyard}if *card==host),
        destroyed + 1,
        "Host graveyard move",
    )?;
    let putrefy_resolved = pos(
        &|e| matches!(e,GameEvent::SpellResolved{card}if *card==putrefy),
        graveyard + 1,
        "Putrefy resolution",
    )?;
    let trigger = pos(
        &|e| matches!(e,GameEvent::TriggeredAbilityStacked{controller:PlayerId(0),source,ability,..}if *source==host&&*ability==ABILITY_ID),
        putrefy_resolved + 1,
        "Host dies trigger",
    )?;
    let life_loss = pos(
        &|e| matches!(e,GameEvent::LifeLost{source,player:PlayerId(1),amount:2}if *source==host),
        trigger + 1,
        "Host target life loss",
    )?;
    let ability_resolved = pos(
        &|e| matches!(e,GameEvent::AbilityResolved{source,ability,..}if *source==host&&*ability==ABILITY_ID),
        life_loss + 1,
        "Host ability resolution",
    )?;
    let game_end = pos(
        &|e| {
            matches!(
                e,
                GameEvent::GameEnded {
                    winner: Some(PlayerId(0))
                }
            )
        },
        ability_resolved + 1,
        "terminal result",
    )?;
    Ok(EventPositions {
        host_payment,
        host_cast,
        host_resolved,
        entry,
        putrefy_payment,
        putrefy_cast,
        putrefy_resolved,
        destroyed,
        graveyard,
        trigger,
        life_loss,
        ability_resolved,
        game_end,
    })
}

fn event_digest(events: &[GameEvent]) -> String {
    let mut text = String::new();
    for event in events {
        use std::fmt::Write as _;
        let _ = writeln!(text, "{event:?}");
    }
    let hash = text.as_bytes().iter().fold(0xcbf29ce484222325_u64, |h, b| {
        (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
    });
    format!("fnv1a64:{hash:016x}")
}
fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CANDIDATE_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"engine_status":match card.semantic_status{CardSemanticStatus::ExecutableCompatibilitySlice{..}=>"executable_compatibility_slice",CardSemanticStatus::CatalogOnly{..}=>"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"credited":false,"qualification":if passed{"pilot_trace_passed; independent reviewer approval pending"}else{"not_evaluated"}})
}
fn candidate_kind() -> &'static str {
    if std::env::args().any(|a| a == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}
fn qualification_status() -> &'static str {
    if candidate_kind() == "candidate" {
        "candidate_pilot_pass; independent reviewer approval remains"
    } else {
        "reference_pilot_pass; independent review and candidate trial remain"
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
