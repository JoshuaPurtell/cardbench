//! Full-game diagnostic PvP authority for RAV Auratouched Mage.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DecisionKind, DecisionVisibility, DeckEntry, DeckList, Effect,
    Game, GameEvent, GameView, LibrarySearchDestination, LibrarySearchRequirement,
    LibrarySearchSelection, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step,
    TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings,
    rav_attachment_triggered_ability_bindings as attachment_triggers, rav_basic_land_type_bindings,
    rav_land_entry_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_static_continuous_effect_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::parse::Parser;
use syn::{Expr, Item, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-AURATOUCHED-MAGE";
const TRIGGER_ID: &str = "etb-search-and-attach-aura";
const AURA_ID: &str = "RAV-MOLDERVINE-CLOAK";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const FIXTURE_AURA_ID: &str = "AUTH-INCOMPATIBLE-LAND-AURA";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/auratouched_mage.rs";
const DECK_FIND: &str = "rav_auratouched_mage_find_compatible_aura";
const DECK_DECLINE: &str = "rav_auratouched_mage_decline_compatible_aura";
const MAX_TRACE_SEED: u64 = 0;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/auratouched_mage.rs"]
mod candidate_auratouched_mage;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Auratouched Mage result: {error}");
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
            eprintln!("Auratouched Mage PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_binding()?;
    let catalog = unique_catalog()?;
    let (seed, found, declined) = find_trace_seed()?;
    let found_trace = validate_trace(&found, SearchChoice::Find)?;
    let declined_trace = validate_trace(&declined, SearchChoice::Decline)?;
    for (capture, choice) in [
        (&found, SearchChoice::Find),
        (&declined, SearchChoice::Decline),
    ] {
        let replay = run_match(seed, choice, Mutation::None)?;
        if !same_match(capture, &replay) {
            return Err(format!(
                "Auratouched Mage {choice:?} full-game replay was not deterministic"
            ));
        }
    }
    let no_trigger = mutation_detected(seed, SearchChoice::Find, Mutation::NoTrigger)?;
    let incompatible =
        mutation_detected(seed, SearchChoice::Find, Mutation::AllowIncompatibleAura)?;
    let no_attachment = mutation_detected(seed, SearchChoice::Find, Mutation::NoAttachment)?;
    if !no_trigger || !incompatible || !no_attachment {
        return Err("an Auratouched Mage semantic mutation escaped the gameplay oracle".to_owned());
    }

    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 1, "definition_id": CARD_ID, "name": "Auratouched Mage"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117: timing and priority",
                "303.4: Aura enchant restrictions and attachment",
                "302.1 and 601.2: creature spell casting",
                "603.2 and 603.3: mandatory triggered abilities",
                "608.2: resolving a triggered ability",
                "701.19: searching a library"
            ],
            "source_contract": "RAV Auratouched Mage is a white 3/3 Creature for {5}{W}; its mandatory ETB ability privately searches its controller's library for an Aura that can legally attach to it, may fail to find, and attaches a selected Aura to that exact Mage object",
            "authority_cutoff": "2026-09-24"
        },
        "passed": true,
        "pilot_card_passed": true,
        "deterministic_replay": true,
        "cards_passed": 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": rav_main_set_catalog().len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "unreviewed_diagnostic",
        "mutation_sensitivity": {
            "etb_trigger_removed_mutation_detected_in_game": no_trigger,
            "incompatible_aura_allowed_mutation_detected_in_game": incompatible,
            "aura_attachment_removed_mutation_detected_in_game": no_attachment,
            "sealed_mutation_authority": false
        },
        "traces": [found_trace, declined_trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SearchChoice {
    Find,
    Decline,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoTrigger,
    AllowIncompatibleAura,
    NoAttachment,
}

struct CapturedMatch {
    seed: u64,
    events: Vec<GameEvent>,
    winner: Option<usize>,
    terminal: String,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    mage: Option<ObjectId>,
    mage_incarnation_at_trigger: Option<u64>,
    aura: Option<ObjectId>,
    aura_definition: Option<String>,
    aura_attached_to: Option<ObjectId>,
    requested_selection: Option<ObjectId>,
    search_options: usize,
    choices_were_private: bool,
    opponent_choice_hidden: bool,
    mage_power: Option<i32>,
    mage_toughness: Option<i32>,
    mage_combat_damage_to_opponent: i32,
}

fn find_trace_seed() -> Result<(u64, CapturedMatch, CapturedMatch), String> {
    let mut failures = Vec::new();
    for seed in 0..=MAX_TRACE_SEED {
        let found = match run_match(seed, SearchChoice::Find, Mutation::None) {
            Ok(capture) => capture,
            Err(error) => {
                failures.push(format!("{seed}: {error}"));
                continue;
            }
        };
        if let Err(error) = validate_trace(&found, SearchChoice::Find) {
            failures.push(format!("{seed}: find: {error}"));
            continue;
        }
        let declined = match run_match(seed, SearchChoice::Decline, Mutation::None) {
            Ok(capture) => capture,
            Err(error) => {
                failures.push(format!("{seed}: decline branch: {error}"));
                continue;
            }
        };
        match validate_trace(&declined, SearchChoice::Decline) {
            Ok(_) => return Ok((seed, found, declined)),
            Err(error) => failures.push(format!("{seed}: decline: {error}")),
        }
    }
    Err(format!(
        "no paired full-game Mage/Aura traces found through seed {MAX_TRACE_SEED}; {}",
        failures.join("; ")
    ))
}

fn mutation_detected(seed: u64, choice: SearchChoice, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, choice, mutation) {
        Err(_) => Ok(true),
        Ok(capture) => Ok(validate_trace(&capture, choice).is_err()),
    }
}

fn same_match(left: &CapturedMatch, right: &CapturedMatch) -> bool {
    left.events == right.events
        && left.winner == right.winner
        && left.terminal == right.terminal
        && left.seed == right.seed
        && left.turns == right.turns
        && left.accepted_moves == right.accepted_moves
        && left.mage == right.mage
        && left.mage_incarnation_at_trigger == right.mage_incarnation_at_trigger
        && left.aura == right.aura
        && left.aura_definition == right.aura_definition
        && left.aura_attached_to == right.aura_attached_to
        && left.requested_selection == right.requested_selection
        && left.search_options == right.search_options
        && left.choices_were_private == right.choices_were_private
        && left.opponent_choice_hidden == right.opponent_choice_hidden
        && left.mage_power == right.mage_power
        && left.mage_toughness == right.mage_toughness
        && left.mage_combat_damage_to_opponent == right.mage_combat_damage_to_opponent
}

fn run_match(seed: u64, choice: SearchChoice, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definitions = card_definitions();
    let mage_definition = definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Auratouched Mage is missing from the RAV definition registry")?;
    *mage_definition = candidate_auratouched_mage::definition();
    if mutation == Mutation::AllowIncompatibleAura {
        definitions.push(incompatible_land_aura_definition());
    }

    let mut triggers = rav_triggered_ability_bindings();
    let trigger_index = triggers
        .iter()
        .position(|binding| binding.card_definition == CARD_ID)
        .ok_or("Auratouched Mage ETB trigger is missing from the registry")?;
    triggers[trigger_index] = candidate_auratouched_mage::triggered_ability();
    match mutation {
        Mutation::None => {}
        Mutation::NoTrigger => {
            triggers.remove(trigger_index);
        }
        Mutation::AllowIncompatibleAura | Mutation::NoAttachment => {
            triggers[trigger_index].ability.effects = vec![generic_aura_search()];
        }
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
    game.register_attachment_bindings(rav_attachment_bindings())
        .map_err(|error| error.to_string())?;

    let extra_aura = u8::from(mutation == Mutation::AllowIncompatibleAura) * 4;
    let forest_count = 26_u8 - extra_aura;
    let plains_count = 26_u8;
    let mage_deck = DeckList {
        mainboard: [
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: AURA_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: forest_count,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: plains_count,
            },
        ]
        .into_iter()
        .chain((extra_aura > 0).then(|| DeckEntry {
            card: FIXTURE_AURA_ID.to_owned(),
            count: extra_aura,
        }))
        .collect(),
        sideboard: vec![],
    };
    let opponent_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: WATCHWOLF_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 28,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 28,
            },
        ],
        sideboard: vec![],
    };
    for (name, deck) in [(DECK_FIND, &mage_deck), (DECK_DECLINE, &opponent_deck)] {
        let count: u16 = deck
            .mainboard
            .iter()
            .map(|entry| u16::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{name} has {count} cards; expected 60"));
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: 60,
        max_policy_moves: 10_000,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &mage_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(MagePolicy::new(PlayerId(0))),
        Box::new(MagePolicy::new(PlayerId(1))),
    ];
    let mut capture = CapturedMatch {
        seed,
        events: vec![],
        winner: None,
        terminal: String::new(),
        turns: 0,
        accepted_moves: 0,
        rejected_moves: 0,
        mage: None,
        mage_incarnation_at_trigger: None,
        aura: None,
        aura_definition: None,
        aura_attached_to: None,
        requested_selection: None,
        search_options: 0,
        choices_were_private: false,
        opponent_choice_hidden: false,
        mage_power: None,
        mage_toughness: None,
        mage_combat_damage_to_opponent: 0,
    };
    let mut attempted = 0_u32;
    while !game.is_game_over() {
        if game.turn > config.max_turns || capture.accepted_moves >= config.max_policy_moves {
            return Err(format!(
                "Auratouched Mage match exceeded bounds at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("policy seat is unavailable")?;
        let action = if let Some(search) = view.library_search_choice.as_ref() {
            if search.source != capture.mage.unwrap_or(search.source) {
                return Err(
                    "Auratouched Mage search source differs from the entering Mage".to_owned(),
                );
            }
            capture.search_options = search.cards.len();
            capture.choices_were_private = true;
            capture.opponent_choice_hidden = game
                .view_for_player(PlayerId(1))
                .is_ok_and(|opponent| opponent.library_search_choice.is_none());
            let wanted = match mutation {
                Mutation::AllowIncompatibleAura => Some(FIXTURE_AURA_ID),
                Mutation::NoAttachment => Some(AURA_ID),
                _ => match choice {
                    SearchChoice::Find => Some(AURA_ID),
                    SearchChoice::Decline => None,
                },
            };
            let selected = wanted.and_then(|definition| {
                search
                    .cards
                    .iter()
                    .find(|card| card.definition == Some(definition))
                    .map(|card| card.id)
            });
            if wanted.is_some() && selected.is_none() {
                return Err(format!(
                    "requested Aura candidate {wanted:?} is not present in the private search choices"
                ));
            }
            capture.requested_selection = selected;
            PolicyAction::ChooseLibrarySearchCard {
                decision: search.decision,
                source: search.source,
                selected,
            }
        } else if let Some(action) = policy.propose_pending_decision(&view) {
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
            return Err(
                "library search decision was not routed to the controller policy".to_owned(),
            );
        } else {
            policy.propose_move(&view)
        };
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        capture.accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        if capture.mage.is_none() {
            capture.mage = game.event_log.iter().find_map(|event| match event {
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    Some(*card)
                }
                _ => None,
            });
        }
        if let Some(mage) = capture.mage
            && game.zone_of(mage) == Some(Zone::Battlefield)
            && let Ok(characteristics) = game.characteristics(mage)
        {
            capture.mage_power = characteristics.power;
            capture.mage_toughness = characteristics.toughness;
        }
        if let Some(mage) = capture.mage {
            if capture.mage_incarnation_at_trigger.is_none() {
                capture.mage_incarnation_at_trigger =
                    game.event_log.iter().find_map(|event| match event {
                        GameEvent::TriggeredAbilityStacked {
                            source,
                            source_incarnation,
                            ability: TRIGGER_ID,
                            ..
                        } if *source == mage => Some(*source_incarnation),
                        _ => None,
                    });
            }
            if capture.aura.is_none() {
                capture.aura = game.event_log.iter().find_map(|event| match event {
                    GameEvent::LibrarySearchResolved {
                        source,
                        found: Some(aura),
                        destination: LibrarySearchDestination::Battlefield,
                        ..
                    } if *source == mage => Some(*aura),
                    _ => None,
                });
            }
            if let Some(aura) = capture.aura {
                capture.aura_definition = game
                    .card_definition(aura)
                    .ok()
                    .map(|definition| definition.id.to_owned());
                capture.aura_attached_to =
                    game.object(aura).ok().and_then(|object| object.attached_to);
            }
        }
    }
    capture.events = game.event_log.clone();
    capture.winner = game.winner().map(|player| player.0);
    capture.terminal = capture.winner.map_or_else(
        || "Draw".to_owned(),
        |seat| format!("Winner(PlayerId({seat}))"),
    );
    capture.turns = game.turn;
    capture.rejected_moves = attempted.saturating_sub(capture.accepted_moves);
    let mage = capture.mage;
    capture.mage_combat_damage_to_opponent = capture
        .events
        .iter()
        .filter_map(|event| match event {
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if Some(*source) == mage => Some(*amount),
            _ => None,
        })
        .sum();
    if let Some(aura) = capture.aura {
        capture.aura_definition = game
            .card_definition(aura)
            .ok()
            .map(|definition| definition.id.to_owned());
    }
    Ok(capture)
}

struct MagePolicy {
    player: PlayerId,
    mage_cast: bool,
    opponent_attacked: bool,
}

impl MagePolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            mage_cast: false,
            opponent_attacked: false,
        }
    }

    fn total_mana(view: &GameView) -> u16 {
        Color::MANA_ALL
            .into_iter()
            .map(|color| u16::from(view.mana_pool.amount(color)))
            .sum()
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

    fn cast(&mut self, view: &GameView, id: &'static str, cost: ManaCost) -> Option<PolicyAction> {
        if let Some(selection) = Self::payment(view, &cost) {
            let card = view.hand.iter().find(|card| card.definition == Some(id))?;
            if id == CARD_ID {
                self.mage_cast = true;
            }
            return Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection: selection,
            });
        }
        let missing = cost
            .colored
            .iter()
            .find(|color| view.mana_pool.amount(**color) == 0)
            .copied()
            .or_else(|| {
                (Self::total_mana(view) < u16::from(cost.generic) + cost.colored.len() as u16)
                    .then_some(Color::White)
            });
        Self::mana_source(view, missing)
    }
}

impl CodePolicy for MagePolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.auratouched-mage-aura-search.v1"
        } else {
            "rav.single-watchwolf-attacker.v1"
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
                let attackers = view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(CARD_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect();
                return PolicyAction::DeclareAttackers { attackers };
            }
            if self.opponent_attacked {
                return PolicyAction::DeclareAttackers { attackers: vec![] };
            }
            let attackers: Vec<_> = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(WATCHWOLF_ID) && card.can_attack)
                .map(|card| vec![card.id])
                .unwrap_or_default();
            self.opponent_attacked = !attackers.is_empty();
            return PolicyAction::DeclareAttackers { attackers };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if view.active_player != self.player
            || view.step != Step::PrecombatMain
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
        if self.player == PlayerId(0)
            && !self.mage_cast
            && view
                .hand
                .iter()
                .any(|card| card.definition == Some(CARD_ID))
        {
            if let Some(action) = self.cast(view, CARD_ID, ManaCost::with_colors(5, [Color::White]))
            {
                return action;
            }
        }
        if self.player == PlayerId(1)
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
            ) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(capture: &CapturedMatch, choice: SearchChoice) -> Result<Value, String> {
    if capture.terminal != "Winner(PlayerId(0))"
        || capture.winner != Some(0)
        || capture.rejected_moves != 0
    {
        return Err("Auratouched Mage duel did not end in a clean P0 win".to_owned());
    }
    let mage = capture.mage.ok_or("Auratouched Mage was not cast")?;
    if capture.search_options == 0
        || !capture.choices_were_private
        || !capture.opponent_choice_hidden
    {
        return Err(
            "the mandatory ETB choice must be private and retain compatible candidates".to_owned(),
        );
    }
    let cast = event_index(&capture.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == mage)).ok_or("Mage cast receipt missing")?;
    let enters = event_index(&capture.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == mage)).ok_or("Mage battlefield entry receipt missing")?;
    let trigger = event_index(&capture.events, |event| matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, ability: TRIGGER_ID, .. } if *source == mage)).ok_or("Mage ETB trigger was not stacked")?;
    let opened = event_index(&capture.events, |event| {
        matches!(
            event,
            GameEvent::DecisionOpened {
                player: PlayerId(0),
                kind: DecisionKind::LibrarySearch,
                visibility: DecisionVisibility::Private,
                min_selections: 0,
                max_selections: 1,
                ..
            }
        )
    })
    .ok_or("private Aura search choice did not open")?;
    let completed = event_index(&capture.events, |event| {
        matches!(
            event,
            GameEvent::DecisionCompleted {
                player: PlayerId(0),
                kind: DecisionKind::LibrarySearch,
                ..
            }
        )
    })
    .ok_or("private Aura search choice did not complete")?;
    let search = event_index(&capture.events, |event| matches!(event, GameEvent::LibrarySearchResolved { player: PlayerId(0), source, destination: LibrarySearchDestination::Battlefield, .. } if *source == mage)).ok_or("source-linked Aura search receipt missing")?;
    let shuffle = event_index_after(&capture.events, search, |event| {
        matches!(
            event,
            GameEvent::LibraryShuffled {
                player: PlayerId(0),
                ..
            }
        )
    })
    .ok_or("Auratouched Mage library shuffle receipt missing")?;
    let resolved = event_index_after(&capture.events, search, |event| matches!(event, GameEvent::AbilityResolved { source, ability: TRIGGER_ID, .. } if *source == mage)).ok_or("Mage ETB ability resolution receipt missing")?;
    let mage_spell_resolved = event_index(
        &capture.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == mage),
    )
    .ok_or("Mage permanent spell resolution receipt missing")?;
    if !(cast < mage_spell_resolved
        && mage_spell_resolved < enters
        && enters < trigger
        && trigger < opened
        && opened < completed
        && completed < search
        && search < shuffle
        && shuffle < resolved)
    {
        return Err(
            "Mage cast, ETB trigger, private search, shuffle, and ability resolution order differs"
                .to_owned(),
        );
    }
    let found = capture
        .events
        .iter()
        .find_map(|event| match event {
            GameEvent::LibrarySearchResolved { source, found, .. } if *source == mage => {
                Some(*found)
            }
            _ => None,
        })
        .ok_or("Mage search result missing")?;
    let mut aura_moved_event = None;
    let mut aura_attached_event = None;
    match choice {
        SearchChoice::Find => {
            let aura = found.ok_or("find branch failed to select an Aura")?;
            if capture.aura != Some(aura)
                || capture.aura_definition.as_deref() != Some(AURA_ID)
                || capture.requested_selection != Some(aura)
                || capture.aura_attached_to != Some(mage)
            {
                return Err(
                    "the found Moldervine Cloak did not attach to the exact Auratouched Mage"
                        .to_owned(),
                );
            }
            let moved = event_index(&capture.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == aura)).ok_or("selected Moldervine Cloak did not enter the battlefield")?;
            let attached = event_index(&capture.events, |event| matches!(event, GameEvent::AuraAttached { aura: event_aura, target } if *event_aura == aura && *target == mage)).ok_or("Moldervine Cloak did not attach to the Mage")?;
            if !(completed < moved && moved < attached && attached < search) {
                return Err(
                    "Aura entry, exact attachment, and search receipt order differs".to_owned(),
                );
            }
            aura_moved_event = Some(moved);
            aura_attached_event = Some(attached);
            if capture.mage_power != Some(6) || capture.mage_toughness != Some(6) {
                return Err(format!(
                    "attached Mage should be 6/6, got {}/{}",
                    capture.mage_power.unwrap_or_default(),
                    capture.mage_toughness.unwrap_or_default()
                ));
            }
        }
        SearchChoice::Decline => {
            if found.is_some() || capture.requested_selection.is_some() || capture.aura.is_some() {
                return Err(
                    "decline branch unexpectedly selected or attached a library Aura".to_owned(),
                );
            }
            if capture.mage_power != Some(3) || capture.mage_toughness != Some(3) {
                return Err(
                    "declining the Aura search changed the Mage's 3/3 characteristics".to_owned(),
                );
            }
        }
    }
    if capture.mage_combat_damage_to_opponent < 20 {
        return Err(format!(
            "full game did not continue to a P0 combat win (Mage damage {})",
            capture.mage_combat_damage_to_opponent
        ));
    }
    let attack = event_index(&capture.events, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&mage))).ok_or("Mage never attacked after the ETB decision")?;
    let damage = event_index_after(&capture.events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), .. } if *source == mage)).ok_or("Mage combat damage after the Aura decision is missing")?;
    let end = event_index(&capture.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("P0 terminal receipt missing")?;
    if !(resolved < attack && attack < damage && damage < end) {
        return Err(
            "the Aura decision must resolve before the Mage wins through combat".to_owned(),
        );
    }
    let mut indices = vec![
        cast,
        enters,
        mage_spell_resolved,
        trigger,
        opened,
        completed,
        search,
        shuffle,
        resolved,
        attack,
        damage,
        end,
    ];
    if let Some(index) = aura_moved_event {
        indices.push(index);
    }
    if let Some(index) = aura_attached_event {
        indices.push(index);
    }
    indices.sort_unstable();
    indices.dedup();
    let events = project_selected_events(&capture.events, &indices);
    Ok(json!({
        "id": match choice { SearchChoice::Find => "rav-auratouched-mage-finds-and-attaches-moldervine-cloak", SearchChoice::Decline => "rav-auratouched-mage-declines-with-compatible-aura-available" },
        "seed": capture.seed,
        "choice": match choice { SearchChoice::Find => "find", SearchChoice::Decline => "decline" },
        "terminal": capture.terminal,
        "winner_seat": capture.winner,
        "turns": capture.turns,
        "accepted_moves": capture.accepted_moves,
        "rejected_moves": capture.rejected_moves,
        "mage_object": mage.0,
        "mage_incarnation_at_trigger": capture.mage_incarnation_at_trigger,
        "aura_object": capture.aura.map(|aura| aura.0),
        "aura_definition": capture.aura_definition,
        "aura_attached_to_mage": capture.aura_attached_to == Some(mage),
        "private_search_choice": capture.choices_were_private,
        "opponent_saw_no_search_choices": capture.opponent_choice_hidden,
        "search_option_count": capture.search_options,
        "requested_selection_object": capture.requested_selection.map(|object| object.0),
        "mage_power_toughness_after_choice": [capture.mage_power, capture.mage_toughness],
        "mage_combat_damage_to_opponent": capture.mage_combat_damage_to_opponent,
        "events_digest": event_digest(&capture.events),
        "events": events,
        "passed": true
    }))
}

fn generic_aura_search() -> Effect {
    Effect::SearchControllerLibrary {
        requirement: LibrarySearchRequirement::Aura,
        destination: LibrarySearchDestination::Battlefield,
        selection: LibrarySearchSelection::PolicySubmitted {
            may_fail_to_find: true,
        },
        reveal_selected: false,
    }
}

fn incompatible_land_aura_definition() -> CardDefinition {
    CardDefinition {
        id: FIXTURE_AURA_ID,
        name: "Authority Incompatible Land Aura",
        set_code: "AUTH",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &["authority-only-incompatible-aura-control"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::AttachSourceToTarget {
            target: TargetRequirement::Land,
            changes: vec![],
        }],
    }
}

fn validate_candidate_binding() -> Result<(), String> {
    let definition = candidate_auratouched_mage::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Auratouched Mage is missing from the RAV catalog")?;
    if definition != registered
        || definition.name != "Auratouched Mage"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(5, [Color::White])
        || definition.colors != BTreeSet::from([Color::White])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.power != Some(3)
        || definition.toughness != Some(3)
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
    {
        return Err("Auratouched Mage literal is outside the task source contract".to_owned());
    }
    let binding = candidate_auratouched_mage::triggered_ability();
    let registered = rav_triggered_ability_bindings()
        .into_iter()
        .find(|candidate| candidate.card_definition == CARD_ID)
        .ok_or("Auratouched Mage trigger is missing from the registry")?;
    if binding != registered || binding != expected_trigger() {
        return Err(
            "Auratouched Mage trigger literal is outside the task source contract".to_owned(),
        );
    }
    Ok(())
}

fn expected_trigger() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: CARD_ID,
        ability: TriggeredAbility {
            id: TRIGGER_ID,
            condition: TriggerCondition::EntersBattlefield,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![
                Effect::SearchControllerLibraryForCompatibleAuraAttachedToSource {
                    selection: LibrarySearchSelection::PolicySubmitted {
                        may_fail_to_find: true,
                    },
                },
            ],
        },
    }
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Auratouched Mage source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Auratouched Mage source is not declarative Rust".to_owned())?;
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "LibrarySearchSelection"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TriggerCondition"],
        vec!["cardbench_magic_engine", "TriggeredAbility"],
        vec!["cardbench_magic_engine", "TriggeredAbilityBinding"],
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
                let mut paths = Vec::new();
                if item.leading_colon.is_some() || !flatten_use_tree(&[], &item.tree, &mut paths) {
                    return Err("Auratouched Mage source has unsupported imports".to_owned());
                }
                imports.extend(paths);
            }
            Item::Fn(item)
                if item.attrs.iter().all(|attribute| {
                    attribute.path().is_ident("doc")
                        || attribute.path().is_ident("must_use")
                            && matches!(attribute.meta, syn::Meta::Path(_))
                }) && matches!(&item.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    && item.sig.inputs.is_empty()
                    && item.sig.generics.params.is_empty()
                    && item.sig.generics.where_clause.is_none()
                    && item.sig.constness.is_none()
                    && item.sig.asyncness.is_none()
                    && item.sig.unsafety.is_none()
                    && item.sig.abi.is_none()
                    && item.sig.variadic.is_none()
                    && item.block.stmts.len() == 1 =>
            {
                let (expected, expected_type) = match item.sig.ident.to_string().as_str() {
                    "definition" => ("CardDefinition", "CardDefinition"),
                    "triggered_ability" => ("TriggeredAbilityBinding", "TriggeredAbilityBinding"),
                    _ => {
                        return Err(
                            "Auratouched Mage source has an unsupported function".to_owned()
                        );
                    }
                };
                let direct = matches!(&item.block.stmts[0], Stmt::Expr(Expr::Struct(value), None) if value.attrs.is_empty() && value.rest.is_none() && path_is_type_path(&value.path, expected));
                if !matches!(&item.sig.output, syn::ReturnType::Type(_, ty) if type_is(ty, expected_type))
                    || !direct
                    || !functions.insert(item.sig.ident.to_string())
                {
                    return Err(
                        "Auratouched Mage functions must return direct typed literals".to_owned(),
                    );
                }
                let Stmt::Expr(Expr::Struct(value), None) = &item.block.stmts[0] else {
                    unreachable!()
                };
                if !value.fields.iter().all(|field| safe_expr(&field.expr)) {
                    return Err("Auratouched Mage source contains a nonliteral field".to_owned());
                }
            }
            _ => {
                return Err(
                    "Auratouched Mage source contains an unsupported top-level item".to_owned(),
                );
            }
        }
    }
    if imports != expected_imports
        || functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()])
        || file.items.len() != 4
    {
        return Err(
            "Auratouched Mage source module structure differs from the source contract".to_owned(),
        );
    }
    Ok(())
}

fn safe_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(value) => value.attrs.is_empty(),
        Expr::Path(value) => {
            value.attrs.is_empty()
                && value.qself.is_none()
                && [
                    &["CardType", "Creature"][..],
                    &["Color", "White"],
                    &["TriggerCondition", "EntersBattlefield"],
                    &["LibrarySearchSelection", "PolicySubmitted"],
                    &[
                        "Effect",
                        "SearchControllerLibraryForCompatibleAuraAttachedToSource",
                    ],
                    &["None"],
                ]
                .iter()
                .any(|path| path_is(&value.path, path))
        }
        Expr::Array(value) => value.attrs.is_empty() && value.elems.iter().all(safe_expr),
        Expr::Reference(value) => {
            value.attrs.is_empty() && value.mutability.is_none() && safe_expr(&value.expr)
        }
        Expr::Call(value) => {
            if !value.attrs.is_empty() || !value.args.iter().all(safe_expr) {
                return false;
            }
            let Expr::Path(path) = &*value.func else {
                return false;
            };
            [
                &["BTreeSet", "from"][..],
                &["BTreeSet", "new"],
                &["ManaCost", "with_colors"],
                &["ManaCost", "new"],
                &["Some"],
            ]
            .iter()
            .any(|expected| path_is(&path.path, expected))
        }
        Expr::Struct(value) => {
            value.attrs.is_empty()
                && value.rest.is_none()
                && [
                    &["CardDefinition"][..],
                    &["TriggeredAbilityBinding"],
                    &["TriggeredAbility"],
                    &[
                        "Effect",
                        "SearchControllerLibraryForCompatibleAuraAttachedToSource",
                    ],
                    &["LibrarySearchSelection", "PolicySubmitted"],
                ]
                .iter()
                .any(|expected| path_is(&value.path, expected))
                && value.fields.iter().all(|field| {
                    field.attrs.is_empty()
                        && matches!(field.member, syn::Member::Named(_))
                        && field.colon_token.is_some()
                        && safe_expr(&field.expr)
                })
        }
        Expr::Macro(value) => {
            if !value.attrs.is_empty() || !value.mac.path.is_ident("vec") {
                return false;
            }
            syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
                .parse2(value.mac.tokens.clone())
                .is_ok_and(|expressions| expressions.iter().all(safe_expr))
        }
        _ => false,
    }
}

fn flatten_use_tree(prefix: &[String], tree: &UseTree, paths: &mut Vec<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(item) => {
            let mut next = prefix.to_vec();
            next.push(item.ident.to_string());
            flatten_use_tree(&next, &item.tree, paths)
        }
        UseTree::Name(item) => {
            let mut path = prefix.to_vec();
            path.push(item.ident.to_string());
            paths.push(path);
            true
        }
        UseTree::Group(items) => items
            .items
            .iter()
            .all(|item| flatten_use_tree(prefix, item, paths)),
        UseTree::Rename(_) | UseTree::Glob(_) => false,
    }
}
fn type_is(ty: &syn::Type, name: &str) -> bool {
    matches!(ty, syn::Type::Path(path) if path.qself.is_none() && path_is_type_path(&path.path, name))
}
fn path_is_type_path(path: &syn::Path, name: &str) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == 1
        && path.segments[0].ident == name
        && matches!(path.segments[0].arguments, syn::PathArguments::None)
}
fn path_is(path: &syn::Path, names: &[&str]) -> bool {
    path.leading_colon.is_none()
        && path.segments.len() == names.len()
        && path.segments.iter().zip(names).all(|(segment, name)| {
            segment.ident == *name && matches!(segment.arguments, syn::PathArguments::None)
        })
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
        return Err("RAV catalog denominator differs from the source contract".to_owned());
    }
    Ok(catalog)
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn event_index_after(
    events: &[GameEvent],
    index: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(index + 1)
        .find_map(|(index, event)| predicate(event).then_some(index))
}
fn project_selected_events(events: &[GameEvent], indices: &[usize]) -> Vec<Value> {
    let projected = project_events(events);
    indices.iter().filter_map(|index| projected.get(*index)).map(|event: &ProjectedEvent| json!({"sequence":event.sequence,"turn":event.turn,"kind":event.kind,"facts":event.facts,"canonical":event.canonical})).collect()
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
fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}
fn card_status(card: &CatalogCard) -> Value {
    let id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 1;
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,"engine_status":if id.is_some(){"executable_compatibility_slice"}else{"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"qualification":if passed{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
}
fn output_root() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--output-root" {
            return args.next().map(Into::into);
        }
    }
    None
}
fn write_outputs(root: &std::path::Path, report: &Value) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    std::fs::write(
        root.join("result.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(report).map_err(|error| error.to_string())?
        ),
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(root.join("reward.txt"), "0.00000000\n").map_err(|error| error.to_string())?;
    Ok(())
}
