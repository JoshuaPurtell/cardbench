//! Full-game RAV Searing Meditation trace for its optional life-gain trigger.

use cardbench_magic_engine::{
    BasicLandManaAbilityActivation, BasicLandType, CardType, CastRequest, Color, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, ResolutionPaymentManaAbility, Step, Target, TargetRequirement,
    TriggerCondition,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-SEARING-MEDITATION";
const HELIX_ID: &str = "RAV-LIGHTNING-HELIX";
const ABILITY_ID: &str = "life-gain-deal-two";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/searing_meditation.rs";
const MAX_TRACE_SEED: u64 = 128;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/searing_meditation.rs"]
mod candidate_searing_meditation;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Searing Meditation result: {error}");
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
            eprintln!("Searing Meditation authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_implementation()?;
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
        return Err("RAV catalog denominator changed from the source contract".to_owned());
    }

    let (seed, paid) = find_paid_trace()?;
    let paid_trace = validate_trace(&paid, seed, true)?;
    let declined = run_match(seed, Mutation::None, false)?;
    let declined_trace = validate_trace(&declined, seed, false)?;
    for (capture, accept) in [(&paid, true), (&declined, false)] {
        let replay = run_match(seed, Mutation::None, accept)?;
        if capture.events != replay.events
            || capture.terminal != replay.terminal
            || capture.winner != replay.winner
            || capture.searing != replay.searing
            || capture.helix_cards != replay.helix_cards
            || capture.paid_trigger_count != replay.paid_trigger_count
        {
            return Err("Searing Meditation full-game replay was not deterministic".to_owned());
        }
    }

    let trigger_removal = mutation_detected(seed, Mutation::RemoveTrigger)?;
    let wrong_damage = mutation_detected(seed, Mutation::WrongDamage)?;
    if !trigger_removal || !wrong_damage {
        return Err("a Searing Meditation trigger mutation escaped the full-game trace".to_owned());
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
        "card": {"collector_number": 226, "definition_id": CARD_ID, "name": "Searing Meditation"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "106.1 and 106.4: mana production and spell payment",
                "117: priority and spell resolution",
                "119: life totals and life gain",
                "120: damage and damage events",
                "303.1: enchantments and enchantment permanents",
                "601.2 and 608.3a: casting and resolving spells",
                "603.2 and 603.3: triggered abilities and putting them on the stack",
                "603.5: optional triggered ability payment and resolution"
            ],
            "source_contract": "RAV Searing Meditation #226, {2}{R}{W} enchantment; whenever its controller gains life, that player may pay {1}{R}{W}; if they do, it deals 2 damage to any target",
            "authority_cutoff": "2026-09-23",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "replay_deterministic": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "candidate_pilot_pass; independent reviewer approval remains",
        "mutation_sensitivity": {
            "life_gain_trigger_removal_detected_in_game": trigger_removal,
            "wrong_trigger_damage_amount_detected_in_game": wrong_damage,
            "optional_trigger_decline_verified": true,
            "sealed_mutation_authority": false
        },
        "traces": [paid_trace, declined_trace],
        "per_card_breakdown": breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Searing Meditation source: {error}"))?;
    let syntax = syn::parse_file(&source)
        .map_err(|_| "Searing Meditation candidate source is not valid Rust".to_owned())?;
    if syntax
        .items
        .iter()
        .any(|item| !matches!(item, syn::Item::Use(_) | syn::Item::Fn(_)))
    {
        return Err("Searing Meditation source may contain only imports and functions".to_owned());
    }
    let functions: BTreeSet<String> = syntax
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Fn(function) => Some(function.sig.ident.to_string()),
            _ => None,
        })
        .collect();
    if functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()]) {
        return Err(
            "Searing Meditation source must expose only definition() and triggered_ability()"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_implementation() -> Result<(), String> {
    let definition = candidate_searing_meditation::definition();
    if definition.id != CARD_ID
        || definition.name != "Searing Meditation"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(2, [Color::Red, Color::White])
        || definition.colors != BTreeSet::from([Color::Red, Color::White])
        || definition.card_types != BTreeSet::from([CardType::Enchantment])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.effects.is_empty()
    {
        return Err(
            "Searing Meditation identity or {2}{R}{W} enchantment definition differs".to_owned(),
        );
    }
    let binding = candidate_searing_meditation::triggered_ability();
    if binding.card_definition != CARD_ID
        || binding.ability.id != ABILITY_ID
        || binding.ability.condition != TriggerCondition::LifeGained
        || binding.ability.mana_cost != ManaCost::with_colors(1, [Color::Red, Color::White])
        || !binding.ability.optional
        || !binding.ability.targets.is_empty()
        || binding.ability.effects
            != vec![Effect::DealDamageAfterOptionalManaPayment {
                amount: 2,
                target: TargetRequirement::PlayerOrCreature,
            }]
    {
        return Err("Searing Meditation must bind optional {1}{R}{W} life-gain trigger to 2 damage at any target".to_owned());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    RemoveTrigger,
    WrongDamage,
}

struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    searing: Option<ObjectId>,
    helix_cards: Vec<ObjectId>,
    searing_payment: Vec<Color>,
    trigger_cost_receipts: Vec<ManaCost>,
    trigger_targets: Vec<Target>,
    paid_trigger_count: u32,
    optional_choice_count: u32,
    optional_target_count: u32,
    life_totals: [i64; 2],
}

fn find_paid_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_SEARING_MEDITATION_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_SEARING_MEDITATION_SEED must be unsigned".to_owned())?;
        let captured = run_match(seed, Mutation::None, true)?;
        validate_trace(&captured, seed, true)?;
        let declined = run_match(seed, Mutation::None, false)?;
        validate_trace(&declined, seed, false)?;
        return Ok((seed, captured));
    }
    let mut last_error = String::new();
    for seed in 0..=MAX_TRACE_SEED {
        let captured = run_match(seed, Mutation::None, true)?;
        match validate_trace(&captured, seed, true) {
            Ok(_) => {
                let declined = run_match(seed, Mutation::None, false)?;
                match validate_trace(&declined, seed, false) {
                    Ok(_) => return Ok((seed, captured)),
                    Err(error) => last_error = format!("seed {seed} declined branch: {error}"),
                }
            }
            Err(error) => last_error = format!("seed {seed}: {error}"),
        }
    }
    Err(format!(
        "no clean Searing Meditation trace found for seeds 0..={MAX_TRACE_SEED}; last failure: {last_error}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    let captured = run_match(seed, mutation, true)?;
    let expected = match mutation {
        Mutation::RemoveTrigger => "life-gain trigger was not stacked",
        Mutation::WrongDamage => "Searing Meditation did not deal exactly two damage",
        Mutation::None => return Ok(false),
    };
    Ok(validate_trace(&captured, seed, true)
        .err()
        .is_some_and(|error| error.contains(expected)))
}

fn run_match(seed: u64, mutation: Mutation, accept_trigger: bool) -> Result<Captured, String> {
    let p0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: HELIX_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-CHAR".to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 24,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 24,
            },
        ],
        sideboard: vec![],
    };
    let p1 = DeckList {
        mainboard: vec![DeckEntry {
            card: "RAV-MOUNTAIN".to_owned(),
            count: 60,
        }],
        sideboard: vec![],
    };
    for (name, deck) in [
        ("searing_meditation_p0", &p0),
        ("searing_meditation_p1", &p1),
    ] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!(
                "{name} has {count} cards; expected a legal 60-card library"
            ));
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 120,
        ..DeckMatchConfig::default()
    };
    let mut definitions = card_definitions();
    let registered = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Searing Meditation is missing from the RAV registry")?;
    *registered = candidate_searing_meditation::definition();
    let mut triggers = rav_triggered_ability_bindings();
    triggers.retain(|binding| binding.card_definition != CARD_ID);
    if mutation != Mutation::RemoveTrigger {
        let mut binding = candidate_searing_meditation::triggered_ability();
        if mutation == Mutation::WrongDamage {
            binding.ability.effects = vec![Effect::DealDamageAfterOptionalManaPayment {
                amount: 1,
                target: TargetRequirement::PlayerOrCreature,
            }];
        }
        triggers.push(binding);
    }
    let mut game = Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers,
    )
    .map_err(|error| error.to_string())?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &p0)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &p1)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(SearingPolicy::new(PlayerId(0), accept_trigger)),
        Box::new(LandOnlyPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut searing = None;
    let mut helix_cards = Vec::new();
    let mut searing_payment = Vec::new();
    let mut trigger_cost_receipts = Vec::new();
    let mut trigger_targets = Vec::new();
    let mut paid_trigger_count = 0_u32;
    let mut optional_choice_count = 0_u32;
    let mut optional_target_count = 0_u32;
    while !game.is_game_over() {
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(format!(
                "Searing Meditation game hit a bounded run limit: turn={} accepted={} player={:?} step={:?} priority={:?} mana={:?} hand={:?} life=({}, {})",
                game.turn,
                accepted,
                player,
                view.step,
                view.priority,
                view.mana_pool,
                view.hand
                    .iter()
                    .map(|card| card.definition)
                    .collect::<Vec<_>>(),
                game.player(PlayerId(0))
                    .map_err(|error| error.to_string())?
                    .life,
                game.player(PlayerId(1))
                    .map_err(|error| error.to_string())?
                    .life,
            ));
        }
        if let Some(choice) = &view.optional_triggered_ability_choice
            && choice.ability == ABILITY_ID
            && searing == Some(choice.source)
        {
            optional_choice_count += 1;
            optional_target_count += u32::from(
                choice
                    .conditional_targets
                    .contains(&Target::Player(PlayerId(1))),
            );
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
        if std::env::var_os("CARDBENCH_SEARING_MEDITATION_DEBUG").is_some()
            && player == PlayerId(0)
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            eprintln!(
                "turn={} player={} active={:?} decision={:?} priority={:?} step={:?} landplays={} mana={:?} hand={:?} untapped={:?} action={:?}",
                game.turn,
                player.0,
                view.active_player,
                view.decision_player,
                view.priority,
                view.step,
                view.lands_played,
                view.mana_pool,
                view.hand
                    .iter()
                    .map(|card| card.definition)
                    .collect::<Vec<_>>(),
                view.own_battlefield
                    .iter()
                    .filter(|card| card.card_types.contains(&CardType::Land) && !card.tapped)
                    .map(|card| (card.id, card.basic_land_type))
                    .collect::<Vec<_>>(),
                action,
            );
        }
        let paying_this_trigger = matches!(
            &action,
            PolicyAction::ResolveOptionalTriggeredAbilityWithManaPayment {
                source, ability: ABILITY_ID, pay: true, ..
            } if Some(*source) == searing
        ) || matches!(
            &action,
            PolicyAction::ResolveOptionalTriggeredAbility {
                source, ability: ABILITY_ID, pay: true, ..
            } if Some(*source) == searing
        );
        if let PolicyAction::ResolveOptionalTriggeredAbilityWithManaPayment {
            source,
            ability: ABILITY_ID,
            target: Some(target),
            ..
        }
        | PolicyAction::ResolveOptionalTriggeredAbility {
            source,
            ability: ABILITY_ID,
            target: Some(target),
            ..
        } = &action
            && Some(*source) == searing
        {
            trigger_targets.push(*target);
        }
        let old_event_count = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| format!("Searing task policy move rejected: {error}"))?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        if paying_this_trigger {
            paid_trigger_count += 1;
        }
        for event in &game.event_log[old_event_count..] {
            match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    searing = Some(*card)
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == HELIX_ID) =>
                {
                    helix_cards.push(*card)
                }
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    searing_payment = colors.clone()
                }
                GameEvent::AbilityManaPaid {
                    player: PlayerId(0),
                    source,
                    ability: ABILITY_ID,
                    mana_cost,
                } if Some(*source) == searing => trigger_cost_receipts.push(mana_cost.clone()),
                _ => {}
            }
        }
    }
    let winner = game.winner().map(|player| player.0);
    let life_totals = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    Ok(Captured {
        events: game.event_log.clone(),
        terminal: winner.map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({seat}))"),
        ),
        winner,
        turns: game.turn,
        accepted_moves: accepted,
        searing,
        helix_cards,
        searing_payment,
        trigger_cost_receipts,
        trigger_targets,
        paid_trigger_count,
        optional_choice_count,
        optional_target_count,
        life_totals,
    })
}

struct SearingPolicy {
    player: PlayerId,
    accept_trigger: bool,
    searing_cast: bool,
}
impl SearingPolicy {
    fn new(player: PlayerId, accept_trigger: bool) -> Self {
        Self {
            player,
            accept_trigger,
            searing_cast: false,
        }
    }
}
impl CodePolicy for SearingPolicy {
    fn id(&self) -> &'static str {
        "rav.searing-meditation-full-game.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
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
        if view.active_player != self.player
            || view.priority != self.player
            || view.stack_depth != 0
            || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0
            && let Some(land) = view
                .hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
        {
            return PolicyAction::PlayLand { card: land.id };
        }
        if !self.searing_cast
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
        {
            let cost = ManaCost::with_colors(2, [Color::Red, Color::White]);
            if let Some(mana_selection) = payment(view, &cost) {
                self.searing_cast = true;
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection,
                };
            }
            if let Some(action) = make_mana(view, &cost) {
                return action;
            }
        }
        if let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some(HELIX_ID))
        {
            let cost = ManaCost::with_colors(0, [Color::Red, Color::White]);
            if let Some(mana_selection) = payment(view, &cost) {
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: card.id,
                        targets: vec![Target::Player(PlayerId(1))],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection,
                };
            }
            if let Some(action) = make_mana(view, &cost) {
                return action;
            }
        }
        if let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some("RAV-CHAR"))
        {
            let cost = ManaCost::with_colors(2, [Color::Red, Color::Red]);
            if let Some(mana_selection) = payment(view, &cost) {
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: card.id,
                        targets: vec![Target::Player(PlayerId(1))],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection,
                };
            }
            if let Some(action) = make_mana(view, &cost) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
    fn propose_optional_triggered_ability(&mut self, view: &GameView) -> PolicyAction {
        let choice = view
            .optional_triggered_ability_choice
            .as_ref()
            .expect("optional choice is visible");
        if choice.ability != ABILITY_ID || !self.accept_trigger {
            return PolicyAction::ResolveOptionalTriggeredAbility {
                decision: choice.decision,
                source: choice.source,
                ability: choice.ability,
                pay: false,
                target: None,
            };
        }
        let mut selected = BTreeSet::new();
        let mut mana_abilities = Vec::new();
        let can_supply = if choice.can_pay {
            true
        } else {
            let red = add_resolution_mana(view, Color::Red, &mut selected, &mut mana_abilities);
            let white = add_resolution_mana(view, Color::White, &mut selected, &mut mana_abilities);
            let generic = view
                .own_battlefield
                .iter()
                .find(|card| {
                    card.card_types.contains(&CardType::Land)
                        && !card.tapped
                        && !selected.contains(&card.id)
                })
                .is_some_and(|land| {
                    let color = land
                        .mana_colors
                        .iter()
                        .copied()
                        .next()
                        .unwrap_or(Color::Red);
                    selected.insert(land.id);
                    mana_abilities.push(ResolutionPaymentManaAbility::IntrinsicLand(
                        BasicLandManaAbilityActivation {
                            land: land.id,
                            color,
                        },
                    ));
                    true
                });
            red && white && generic
        };
        if can_supply {
            PolicyAction::ResolveOptionalTriggeredAbilityWithManaPayment {
                decision: choice.decision,
                source: choice.source,
                ability: choice.ability,
                pay: true,
                target: Some(Target::Player(PlayerId(1))),
                mana_abilities,
            }
        } else {
            PolicyAction::ResolveOptionalTriggeredAbility {
                decision: choice.decision,
                source: choice.source,
                ability: choice.ability,
                pay: false,
                target: None,
            }
        }
    }
}

fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<(Color, u8)> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let (_, amount) = available.iter_mut().find(|(current, _)| current == color)?;
        if *amount == 0 {
            return None;
        }
        *amount -= 1;
    }
    let mut generic = Vec::with_capacity(usize::from(cost.generic));
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
    let mut colored_available: Vec<(Color, u8)> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    let mut missing = None;
    for color in &cost.colored {
        let Some((_, amount)) = colored_available
            .iter_mut()
            .find(|(current, _)| current == color)
        else {
            missing = Some(*color);
            break;
        };
        if *amount == 0 {
            missing = Some(*color);
            break;
        }
        *amount -= 1;
    }
    let required = u16::from(cost.generic) + u16::try_from(cost.colored.len()).ok()?;
    if missing.is_none() && u16::from(view.mana_pool.total()) >= required {
        return None;
    }
    let land = view.own_battlefield.iter().find(|card| {
        !card.tapped
            && card.card_types.contains(&CardType::Land)
            && match (card.basic_land_type, missing) {
                (Some(BasicLandType::Plains), Some(Color::White))
                | (Some(BasicLandType::Mountain), Some(Color::Red)) => true,
                (Some(BasicLandType::Plains | BasicLandType::Mountain), None) => true,
                _ => false,
            }
    })?;
    let color = match land.basic_land_type? {
        BasicLandType::Plains => Color::White,
        BasicLandType::Mountain => Color::Red,
        _ => return None,
    };
    Some(PolicyAction::ActivateManaAbility {
        land: land.id,
        color,
    })
}

fn add_resolution_mana(
    view: &GameView,
    color: Color,
    selected: &mut BTreeSet<ObjectId>,
    mana_abilities: &mut Vec<ResolutionPaymentManaAbility>,
) -> bool {
    let Some(land) = view.own_battlefield.iter().find(|card| {
        card.card_types.contains(&CardType::Land)
            && !card.tapped
            && card.mana_colors.contains(&color)
            && !selected.contains(&card.id)
    }) else {
        return false;
    };
    selected.insert(land.id);
    mana_abilities.push(ResolutionPaymentManaAbility::IntrinsicLand(
        BasicLandManaAbilityActivation {
            land: land.id,
            color,
        },
    ));
    true
}

struct LandOnlyPolicy {
    player: PlayerId,
}
impl LandOnlyPolicy {
    fn new(player: PlayerId) -> Self {
        Self { player }
    }
}
impl CodePolicy for LandOnlyPolicy {
    fn id(&self) -> &'static str {
        "rav.searing-meditation-land-only.v1"
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
        if view.active_player == self.player
            && view.decision_player == self.player
            && view.priority == self.player
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
            && view.lands_played == 0
            && let Some(land) = view
                .hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
        {
            return PolicyAction::PlayLand { card: land.id };
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &Captured, seed: u64, expect_paid: bool) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))" || captured.winner != Some(0) {
        return Err("Searing Meditation game did not reach a terminal seat-zero win".to_owned());
    }
    let searing = captured.searing.ok_or("Searing Meditation was not cast")?;
    if captured.searing_payment.len() != 4
        || !captured.searing_payment.contains(&Color::Red)
        || !captured.searing_payment.contains(&Color::White)
    {
        return Err(format!(
            "Searing Meditation did not pay {{2}}{{R}}{{W}}: {:?}",
            captured.searing_payment
        ));
    }
    if captured.helix_cards.is_empty() {
        return Err(
            "Lightning Helix did not create the Searing Meditation life-gain event".to_owned(),
        );
    }
    let searing_cast = find_event(&captured.events, |event| {
        matches!(event,
            GameEvent::SpellCast { player: PlayerId(0), card } if *card == searing
        )
    })
    .ok_or("Searing Meditation cast receipt is missing")?;
    let searing_resolved = find_event_after(&captured.events, searing_cast, |event| {
        matches!(event,
            GameEvent::SpellResolved { card } if *card == searing
        )
    })
    .ok_or("Searing Meditation resolution receipt is missing")?;
    let life = find_event_after(&captured.events, searing_resolved, |event| {
        matches!(
            event,
            GameEvent::LifeGained {
                player: PlayerId(0),
                amount: 3
            }
        )
    })
    .ok_or("controller did not gain three life from Lightning Helix")?;
    let trigger = find_event_after(&captured.events, life, |event| matches!(event,
        GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, ability: ABILITY_ID, .. }
            if *source == searing
    )).ok_or("Searing Meditation life-gain trigger was not stacked")?;
    if captured.optional_choice_count == 0 || captured.optional_target_count == 0 {
        return Err("Searing Meditation optional target choice was not available".to_owned());
    }
    let paid = captured.paid_trigger_count > 0;
    if paid != expect_paid {
        return Err("Searing Meditation optional trigger payment branch differs".to_owned());
    }
    let searing_damage_events = captured
        .events
        .iter()
        .filter(|event| matches!(event,
            GameEvent::DamageDealtToPlayer { source, .. } if *source == searing
        ))
        .count();
    if (expect_paid && searing_damage_events == 0)
        || (!expect_paid
            && (searing_damage_events != 0
                || !captured.trigger_cost_receipts.is_empty()
                || !captured.trigger_targets.is_empty()))
    {
        return Err("Searing Meditation optional choice dealt damage or paid mana on the wrong branch".to_owned());
    }
    if expect_paid
        && !captured
            .trigger_targets
            .contains(&Target::Player(PlayerId(1)))
    {
        return Err("Searing Meditation did not choose the opponent as a legal target".to_owned());
    }
    let mut positions = vec![
        find_event(&captured.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, .. } if *card == searing)).ok_or("Searing Meditation mana-payment receipt is missing")?,
        searing_cast,
        searing_resolved,
        life,
        trigger,
    ];
    let mut expected = vec![
        "SpellManaPaid",
        "SpellCast",
        "SpellResolved",
        "LifeGained",
        "TriggeredAbilityStacked",
    ];
    if expect_paid {
        let paid_cost = find_event_after(&captured.events, trigger, |event| matches!(event,
            GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: ABILITY_ID, mana_cost }
                if *source == searing && *mana_cost == ManaCost::with_colors(1, [Color::Red, Color::White])
        )).ok_or("Searing Meditation did not pay exact {1}{R}{W}")?;
        if !captured
            .trigger_cost_receipts
            .contains(&ManaCost::with_colors(1, [Color::Red, Color::White]))
        {
            return Err("Searing Meditation exact trigger mana cost is missing".to_owned());
        }
        let damage = find_event_after(&captured.events, paid_cost, |event| {
            matches!(event,
                GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 2 }
                    if *source == searing
            )
        })
        .ok_or("Searing Meditation did not deal exactly two damage to its chosen target")?;
        let resolved = find_event_after(&captured.events, damage, |event| {
            matches!(event,
                GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == searing
            )
        })
        .ok_or("Searing Meditation triggered ability did not resolve after its damage")?;
        positions.extend([paid_cost, damage, resolved]);
        expected.extend(["AbilityManaPaid", "DamageDealtToPlayer", "AbilityResolved"]);
    }
    let end = find_event(&captured.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal player-zero win event missing")?;
    positions.push(end);
    expected.push("GameEnded");
    if positions.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(format!(
            "Searing Meditation required event ordering differs: {:?}",
            positions
                .iter()
                .map(|index| (*index, format!("{:?}", captured.events[*index])))
                .collect::<Vec<_>>()
        ));
    }
    let projected = project_events(&captured.events);
    let selected: Vec<_> = positions
        .iter()
        .map(|index| projected[*index].clone())
        .collect();
    let events: Vec<_> = selected.iter().map(|event| event.kind.as_str()).collect();
    if events != expected {
        return Err(format!(
            "Searing Meditation ordered event trace differs: {events:?}"
        ));
    }
    Ok(json!({
        "id": if expect_paid { "rav-searing-meditation-paid-trigger" } else { "rav-searing-meditation-declined-trigger" },
        "seed": seed,
        "decks": ["rav_searing_meditation_boros_helix_60", "rav_searing_meditation_mountain_60"],
        "policies": ["rav.searing-meditation-full-game.v1", "rav.searing-meditation-land-only.v1"],
        "terminal": captured.terminal,
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": 0,
        "searing_meditation_object": searing.0,
        "lightning_helix_casts": captured.helix_cards.len(),
        "optional_trigger_paid_count": captured.paid_trigger_count,
        "searing_damage_events": searing_damage_events,
        "card_payment": {"required": "{2}{R}{W}", "generic_symbols": 2, "red": captured.searing_payment.contains(&Color::Red), "white": captured.searing_payment.contains(&Color::White), "spent": captured.searing_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>()},
        "optional_trigger": {"ability": ABILITY_ID, "triggered": captured.optional_choice_count, "legal_player_target": captured.optional_target_count > 0, "chosen_target": if expect_paid { "PlayerId(1)" } else { "none" }, "paid": expect_paid, "paid_mana_cost": if expect_paid { "{1}{R}{W}" } else { "none" }},
        "life_totals": captured.life_totals,
        "events": selected,
        "ordered_event_sequence": positions.iter().map(|index| projected[*index].sequence).collect::<Vec<_>>(),
        "passed": true
    }))
}

fn find_event(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn find_event_after(
    events: &[GameEvent],
    start: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, event)| predicate(event).then_some(index))
}

fn card_status(card: &CatalogCard) -> Value {
    let (status, definition_id) = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => {
            ("executable_compatibility_slice", Some(definition_id))
        }
        CardSemanticStatus::CatalogOnly { .. } => ("catalog_only", None),
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"definition_id":definition_id,"name":card.name,"engine_status":status,"qualification":if passed{"pilot_trace_passed; independent reviewer approval pending"}else{"not_evaluated"},"trace_status":if passed{"passed"}else{"not_qualified"},"credited":false})
}
fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}
fn output_root() -> Option<std::path::PathBuf> {
    std::env::var_os("CARDBENCH_MAGIC_OUTPUT_DIR").map(std::path::PathBuf::from)
}
fn write_outputs(root: &std::path::Path, report: &Value) -> std::io::Result<()> {
    std::fs::create_dir_all(root)?;
    std::fs::write(
        root.join("report.json"),
        serde_json::to_vec_pretty(report).expect("report serializes"),
    )?;
    std::fs::write(root.join("reward.txt"), "0.0\n")
}
