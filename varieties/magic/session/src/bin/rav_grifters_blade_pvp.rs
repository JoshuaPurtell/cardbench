//! Full-game RAV Grifter's Blade authority: Flash, as-enters attachment, and Equip.

use cardbench_magic_engine::{
    AbilityActivation, AttachmentKind, CardType, Color, ContinuousChange, DeckEntry, DeckList,
    DecisionKind, DecisionSelection, Effect, EntryAttachmentBinding, Game, GameEvent, GameView,
    Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target,
    TargetRequirement, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_attachment_bindings, rav_basic_land_type_bindings, rav_land_entry_bindings,
    rav_main_set_catalog, rav_mana_ability_bindings, rav_static_continuous_effect_bindings,
    rav_triggered_ability_bindings, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
};
use cardbench_magic_session::{project_events, ProjectedEvent};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};
use std::process::ExitCode;
use syn::parse::Parser;
use syn::{Expr, Item, Meta, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-GRIFTERS-BLADE";
const CARD_NAME: &str = "Grifter's Blade";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/grifters_blade.rs";
const ENTRY_DECISION: DecisionKind = DecisionKind::PermanentEntryAttachment;
const EQUIP_ABILITY: &str = "equip-plus-one-plus-one";
const MAX_TURNS: u32 = 60;
const MAX_POLICY_MOVES: u32 = 8_000;
const MAX_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/grifters_blade.rs"]
mod candidate_grifters_blade;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root() {
                if let Err(error) = write_outputs(&root, &report) {
                    eprintln!("could not write Grifter's Blade PvP result: {error}");
                    return ExitCode::FAILURE;
                }
            }
            println!("{}", serde_json::to_string(&report).expect("report serializes"));
            if report["passed"].as_bool() == Some(true) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("Grifter's Blade authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    let source_guard = side_effect_source_guard_fixture()?;
    if !source_guard.0 || !source_guard.1 {
        return Err("parse-valid side-effect fixture escaped the pre-call source guard".to_owned());
    }
    validate_candidate_bindings()?;
    let catalog = unique_catalog()?;
    let (seed, reference) = find_reference()?;
    let trace = validate_trace(&reference, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if reference.events != replay.events
        || reference.terminal != replay.terminal
        || reference.winner != replay.winner
        || reference.blade != replay.blade
        || reference.entry_target != replay.entry_target
        || reference.entry_choice_actor != replay.entry_choice_actor
        || reference.equip_target != replay.equip_target
        || reference.final_attachment != replay.final_attachment
        || reference.entry_stats != replay.entry_stats
        || reference.equip_stats != replay.equip_stats
        || reference.flash_cast_during_opponent_upkeep != replay.flash_cast_during_opponent_upkeep
    {
        return Err("Grifter's Blade full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "flash_removed_mutation_detected": mutation_detected(Mutation::NoFlash, seed),
        "entry_attachment_removed_mutation_detected": mutation_detected(Mutation::NoEntryAttachment, seed),
        "plus_one_plus_one_removed_mutation_detected": mutation_detected(Mutation::NoBonus, seed),
        "equip_cost_changed_mutation_detected": mutation_detected(Mutation::WrongEquipCost, seed),
        "sealed_mutation_authority": false
    });
    for key in [
        "flash_removed_mutation_detected",
        "entry_attachment_removed_mutation_detected",
        "plus_one_plus_one_removed_mutation_detected",
        "equip_cost_changed_mutation_detected",
    ] {
        if mutations[key] != true {
            return Err(format!("Grifter's Blade mutation escaped the trace: {key}"));
        }
    }
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 263, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": "executable_definition_and_bindings", "card_definition": CARD_ID},
        "candidate_kind": if std::env::args().any(|argument| argument == "--candidate") {"candidate"} else {"reference"},
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117: priority and timing",
                "301.1 and 301.5: artifact Equipment characteristics and attachment",
                "601.2: casting the artifact spell with Flash",
                "614: as-enters replacement and choice",
                "608.2: triggered and activated ability resolution",
                "613.7: continuous +1/+1 modification from Equipment",
                "702.8: Flash",
                "702.6: Equip and sorcery-speed restriction"
            ],
            "source_contract": "RAV #263 is a colorless {3} Artifact Equipment with Flash; its controller may choose a creature they control as it enters and attach to it without targeting; the equipped creature gets +1/+1; equip {1} at sorcery speed.",
            "entry_choice_limit": "The resumable entry-choice substrate covers permanent spells resolving from the stack. Noncast effects that put this Equipment directly onto the battlefield are not modeled by this task.",
            "authority_cutoff": "2026-09-24"
        },
        "passed": true,
        "pilot_card_passed": true,
        "deterministic_replay": true,
        "cards_passed": 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "unreviewed_diagnostic",
        "source_guard": {
            "side_effect_fixture_parsed": source_guard.0,
            "side_effect_fixture_rejected_before_candidate_factory_calls": source_guard.1,
            "compile_validity_claimed": false
        },
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": catalog.iter().map(card_status).collect::<Vec<_>>()
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoFlash,
    NoEntryAttachment,
    NoBonus,
    WrongEquipCost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    blade: Option<ObjectId>,
    blade_zone: Option<Zone>,
    watchwolves: Vec<ObjectId>,
    entry_target: Option<ObjectId>,
    entry_choice_actor: Option<PlayerId>,
    equip_target: Option<ObjectId>,
    final_attachment: Option<ObjectId>,
    entry_stats: Option<(Option<i32>, Option<i32>)>,
    equip_stats: Option<(Option<i32>, Option<i32>)>,
    flash_cast_during_opponent_upkeep: bool,
    equip_cost_paid: Option<ManaCost>,
}

fn find_reference() -> Result<(u64, CapturedMatch), String> {
    if let Ok(seed) = std::env::var("CARDBENCH_GRIFTERS_BLADE_SEED") {
        let seed = seed
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_GRIFTERS_BLADE_SEED must be an unsigned integer".to_owned())?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    for seed in 0..=MAX_SEED {
        if let Ok(capture) = run_match(seed, Mutation::None)
            && validate_trace(&capture, seed).is_ok()
        {
            return Ok((seed, capture));
        }
    }
    Err(format!("no Grifter's Blade reference trace found in seeds 0..={MAX_SEED}"))
}

fn mutation_detected(mutation: Mutation, seed: u64) -> bool {
    match run_match(seed, mutation) {
        Err(_) => true,
        Ok(capture) => validate_trace(&capture, seed).is_err(),
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definitions = card_definitions();
    let definition_index = definitions
        .iter()
        .position(|definition| definition.id == CARD_ID)
        .ok_or("Grifter's Blade identity is missing from the shared RAV catalog")?;
    let mut definition = candidate_grifters_blade::definition();
    if mutation == Mutation::NoFlash {
        definition.keywords.retain(|keyword| *keyword != Keyword::Flash);
    }
    definitions[definition_index] = definition;

    let mut activations = rav_activated_ability_bindings();
    let activation_index = activations
        .iter()
        .position(|binding| binding.card_definition == CARD_ID)
        .ok_or("Grifter's Blade equip binding is missing from the shared RAV registry")?;
    let mut equip = candidate_grifters_blade::equip_ability();
    if mutation == Mutation::WrongEquipCost {
        equip.ability.mana_cost = ManaCost::new(2);
    }
    if mutation == Mutation::NoBonus {
        clear_attachment_bonus(&mut equip.ability.effects);
    }
    activations[activation_index] = equip;

    let mut triggers = rav_triggered_ability_bindings();
    triggers.retain(|binding| binding.card_definition != CARD_ID);
    let mut attachments = rav_attachment_bindings();
    let attachment_index = attachments
        .iter()
        .position(|binding| binding.card_definition == CARD_ID)
        .ok_or("Grifter's Blade attachment binding is missing from the shared RAV registry")?;
    let mut attachment = candidate_grifters_blade::attachment_binding();
    if mutation == Mutation::NoBonus {
        attachment.changes.clear();
    }
    attachments[attachment_index] = attachment;

    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        activations,
        triggers,
        rav_static_continuous_effect_bindings(),
        rav_land_entry_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_attachment_bindings(attachments)
        .map_err(|error| error.to_string())?;
    if mutation != Mutation::NoEntryAttachment {
        game.register_entry_attachment_bindings([candidate_grifters_blade::entry_attachment_binding()])
            .map_err(|error| error.to_string())?;
    }

    let p0_deck = DeckList {
        mainboard: vec![
            DeckEntry { card: CARD_ID.to_owned(), count: 4 },
            DeckEntry { card: WATCHWOLF_ID.to_owned(), count: 4 },
            DeckEntry { card: "RAV-FOREST".to_owned(), count: 26 },
            DeckEntry { card: "RAV-PLAINS".to_owned(), count: 26 },
        ],
        sideboard: vec![],
    };
    let p1_deck = DeckList {
        mainboard: vec![
            DeckEntry { card: "RAV-FOREST".to_owned(), count: 30 },
            DeckEntry { card: "RAV-PLAINS".to_owned(), count: 30 },
        ],
        sideboard: vec![],
    };
    for (label, deck) in [("Grifter's Blade", &p0_deck), ("opponent", &p1_deck)] {
        let count: usize = deck.mainboard.iter().map(|entry| usize::from(entry.count)).sum();
        if count != 60 {
            return Err(format!("{label} deck has {count} cards; expected 60"));
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_POLICY_MOVES,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed).map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &p0_deck).map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &p1_deck).map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size).map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size).map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(BladePolicy::new(PlayerId(0))),
        Box::new(BladePolicy::new(PlayerId(1))),
    ];
    let mut capture = CapturedMatch {
        events: Vec::new(),
        terminal: String::new(),
        winner: None,
        turns: 0,
        accepted_moves: 0,
        rejected_moves: 0,
        blade: None,
        blade_zone: None,
        watchwolves: Vec::new(),
        entry_target: None,
        entry_choice_actor: None,
        equip_target: None,
        final_attachment: None,
        entry_stats: None,
        equip_stats: None,
        flash_cast_during_opponent_upkeep: false,
        equip_cost_paid: None,
    };
    let mut attempts = 0_u32;
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || capture.accepted_moves >= MAX_POLICY_MOVES {
            return Err(format!("Grifter's Blade duel exceeded its bound at turn {}", game.turn));
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("policy seat is unavailable")?;
        let action = if view.pending_decision.is_some() {
            policy.propose_pending_decision(&view).ok_or("entry attachment decision was not answered")?
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
        if player == PlayerId(0)
            && view.active_player == PlayerId(1)
            && view.step == Step::Upkeep
            && is_cast_of(&game, &action, CARD_ID)
        {
            capture.flash_cast_during_opponent_upkeep = true;
        }
        if let Some(decision) = view.pending_decision.as_ref()
            && decision.kind == ENTRY_DECISION
            && player == PlayerId(0)
            && let PolicyAction::SubmitDecision {
                selection: DecisionSelection::Objects(selected),
                ..
            } = &action
        {
            capture.entry_choice_actor = Some(player);
            capture.entry_target = selected.first().copied();
        }
        if let PolicyAction::ActivateAbility { activation } = &action
            && activation.source == capture.blade.unwrap_or(ObjectId(0))
            && activation.ability_id == EQUIP_ABILITY
            && let Some(Target::Permanent(target)) = activation.targets.first()
        {
            capture.equip_target = Some(*target);
        }
        let cursor = game.event_log.len();
        attempts += 1;
        game.submit_policy_move(player, policy.id(), action).map_err(|error| {
            format!("Grifter's Blade policy move rejected at turn {} {:?}: {error}", view.turn, view.step)
        })?;
        capture.accepted_moves += 1;
        game.validate_invariants().map_err(|error| format!("post-move invariant failure: {error}"))?;
        for event in &game.event_log[cursor..] {
            if let GameEvent::SpellCast { card, .. } = event
                && let Ok(definition) = game.card_definition(*card)
            {
                match definition.id {
                    CARD_ID if capture.blade.is_none() => capture.blade = Some(*card),
                    WATCHWOLF_ID => capture.watchwolves.push(*card),
                    _ => {}
                }
            }
            if matches!(event,
                GameEvent::EquipmentAttached { equipment, target, previous: None }
                    if Some(*equipment) == capture.blade && Some(*target) == capture.entry_target
            ) && let Some(target) = capture.entry_target
                && let Ok(characteristics) = game.characteristics(target)
            {
                capture.entry_stats = Some((characteristics.power, characteristics.toughness));
            }
            if matches!(event,
                GameEvent::AbilityResolved { source, ability: EQUIP_ABILITY, .. }
                    if Some(*source) == capture.blade
            ) && let Some(target) = capture.equip_target
                && let Ok(characteristics) = game.characteristics(target)
            {
                capture.equip_stats = Some((characteristics.power, characteristics.toughness));
            }
        }
    }
    capture.events = game.event_log.clone();
    capture.turns = game.turn;
    capture.winner = game.winner().map(|winner| winner.0);
    capture.terminal = capture.winner.map_or_else(|| "Draw".to_owned(), |seat| format!("Winner(PlayerId({seat}))"));
    capture.blade_zone = capture.blade.and_then(|object| game.zone_of(object));
    capture.final_attachment = capture.blade.and_then(|object| game.object(object).ok().and_then(|object| object.attached_to));
    capture.equip_cost_paid = capture.events.iter().find_map(|event| match event {
        GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: EQUIP_ABILITY, mana_cost }
            if Some(*source) == capture.blade => Some(mana_cost.clone()),
        _ => None,
    });
    capture.rejected_moves = attempts.saturating_sub(capture.accepted_moves);
    Ok(capture)
}

fn is_cast_of(game: &Game, action: &PolicyAction, card_id: &str) -> bool {
    matches!(action, PolicyAction::CastWithPayment { request, .. }
        if game.card_definition(request.card).is_ok_and(|definition| definition.id == card_id))
}

fn clear_attachment_bonus(effects: &mut [Effect]) {
    for effect in effects {
        if let Effect::AttachSourceToTarget { changes, .. } = effect {
            changes.clear();
        }
    }
}

struct BladePolicy {
    player: PlayerId,
    blade_cast: bool,
    equip_activated: bool,
}

impl BladePolicy {
    fn new(player: PlayerId) -> Self {
        Self { player, blade_cast: false, equip_activated: false }
    }

    fn total_mana(view: &GameView) -> u16 {
        Color::MANA_ALL.into_iter().map(|color| u16::from(view.mana_pool.amount(color))).sum()
    }

    fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let mut available: Vec<_> = Color::MANA_ALL.into_iter().map(|color| (color, view.mana_pool.amount(color))).collect();
        for color in &cost.colored {
            let (_, amount) = available.iter_mut().find(|(candidate, _)| candidate == color)?;
            if *amount == 0 { return None; }
            *amount -= 1;
        }
        let mut generic = Vec::new();
        for _ in 0..cost.generic {
            let (color, amount) = available.iter_mut().max_by_key(|(_, amount)| *amount)?;
            if *amount == 0 { return None; }
            *amount -= 1;
            generic.push(*color);
        }
        Some(ManaPaymentSelection { generic, hybrid: vec![] })
    }

    fn mana_source(view: &GameView, preferred: Option<Color>) -> Option<PolicyAction> {
        let land = view.own_battlefield.iter()
            .find(|card| card.card_types.contains(&CardType::Land) && !card.tapped && preferred.is_none_or(|color| card.mana_colors.contains(&color)))
            .or_else(|| view.own_battlefield.iter().find(|card| card.card_types.contains(&CardType::Land) && !card.tapped))?;
        let color = preferred.filter(|color| land.mana_colors.contains(color)).or_else(|| land.mana_colors.iter().next().copied())?;
        Some(PolicyAction::ActivateManaAbility { land: land.id, color })
    }

    fn cast_or_mana(&mut self, view: &GameView, definition: &'static str, cost: ManaCost) -> Option<PolicyAction> {
        let card = view.hand.iter().find(|card| card.definition == Some(definition))?;
        if let Some(mana_selection) = Self::payment(view, &cost) {
            if definition == CARD_ID { self.blade_cast = true; }
            return Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest { card: card.id, targets: vec![], convoke: vec![], payment_mana_abilities: vec![] },
                chosen_x: None,
                mana_selection,
            });
        }
        let missing_color = cost.colored.iter().find(|color| view.mana_pool.amount(**color) == 0).copied();
        let needed = u16::from(cost.generic) + cost.colored.len() as u16;
        let preferred = missing_color.or_else(|| (Self::total_mana(view) < needed).then_some(cost.colored.first().copied()).flatten());
        Self::mana_source(view, preferred)
    }

    fn entry_attachment_action(&self, view: &GameView) -> Option<PolicyAction> {
        let decision = view.pending_decision.as_ref()?;
        if decision.kind != ENTRY_DECISION {
            return None;
        }
        let target = decision.candidates.iter().find(|card| {
            card.card_types.contains(&CardType::Creature)
                && view.own_battlefield.iter().any(|own| own.id == card.id)
        })?;
        Some(PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::Objects(vec![target.id]),
        })
    }
}

impl CodePolicy for BladePolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) { "rav.grifters-blade-pilot.v1" } else { "rav.grifters-blade-land-only-opponent.v1" }
    }

    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        self.entry_attachment_action(view)
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers && view.active_player == self.player && view.player == self.player && !view.attackers_declared {
            return PolicyAction::DeclareAttackers {
                attackers: view.own_battlefield.iter().filter(|card| card.can_attack && card.card_types.contains(&CardType::Creature)).map(|card| card.id).collect(),
            };
        }
        if view.step == Step::DeclareBlockers && view.active_player != self.player && view.player == self.player && !view.blockers_declared {
            return PolicyAction::DeclareBlockers { assignments: vec![] };
        }
        if view.player != self.player || view.decision_player != self.player || view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if self.player == PlayerId(1) {
            if view.active_player == self.player && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain) && view.lands_played == 0
                && let Some(land) = view.hand.iter().find(|card| card.card_types.contains(&CardType::Land))
            { return PolicyAction::PlayLand { card: land.id }; }
            return PolicyAction::PassPriority;
        }
        if view.active_player == self.player && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain) && view.stack_depth == 0 {
            if view.lands_played == 0 && let Some(land) = view.hand.iter().find(|card| card.card_types.contains(&CardType::Land)) {
                return PolicyAction::PlayLand { card: land.id };
            }
            let own_watchwolves = view.own_battlefield.iter().filter(|card| card.definition == Some(WATCHWOLF_ID)).count();
            if own_watchwolves < 2 && view.hand.iter().any(|card| card.definition == Some(WATCHWOLF_ID))
                && let Some(action) = self.cast_or_mana(view, WATCHWOLF_ID, ManaCost::with_colors(0, [Color::Green, Color::White]))
            { return action; }
            if !self.equip_activated && view.own_battlefield.iter().any(|card| card.definition == Some(CARD_ID)) && own_watchwolves >= 2
                && let Some(blade) = view.own_battlefield.iter().find(|card| card.definition == Some(CARD_ID))
                && let Some(target) = view.own_battlefield.iter().find(|card| card.definition == Some(WATCHWOLF_ID))
            {
                let first_host = view.own_battlefield.iter().find(|card| card.definition == Some(WATCHWOLF_ID));
                let target = if own_watchwolves > 1 { view.own_battlefield.iter().filter(|card| card.definition == Some(WATCHWOLF_ID)).nth(1).or(first_host).unwrap_or(target) } else { target };
                if Self::total_mana(view) >= 1 {
                    self.equip_activated = true;
                    return PolicyAction::ActivateAbility { activation: AbilityActivation {
                        source: blade.id, ability_id: EQUIP_ABILITY, sacrifice_sources: vec![], additional_tap_creatures: vec![], discard_cards: vec![], targets: vec![Target::Permanent(target.id)],
                    }};
                }
                if let Some(action) = Self::mana_source(view, None) { return action; }
            }
        }
        // Flash is demonstrated by waiting for Grifter's Blade to be cast with
        // its controller holding priority in the opponent's upkeep.
        if self.player == PlayerId(0) && view.active_player == PlayerId(1) && view.step == Step::Upkeep && !self.blade_cast
            && !view.own_battlefield.iter().all(|card| card.definition != Some(WATCHWOLF_ID))
            && view.hand.iter().any(|card| card.definition == Some(CARD_ID))
            && let Some(action) = self.cast_or_mana(view, CARD_ID, ManaCost::new(3))
        { return action; }
        PolicyAction::PassPriority
    }
}

fn validate_trace(capture: &CapturedMatch, seed: u64) -> Result<Value, String> {
    let blade = capture.blade.ok_or("Grifter's Blade was not cast")?;
    let entry_target = capture.entry_target.ok_or("Grifter's Blade entry attachment was not chosen")?;
    let equip_target = capture.equip_target.ok_or("Grifter's Blade equip target was not chosen")?;
    if capture.terminal != "Winner(PlayerId(0))" || capture.winner != Some(0) || capture.rejected_moves != 0 {
        return Err(format!("full game did not finish with a clean P0 win (terminal={}, rejected={})", capture.terminal, capture.rejected_moves));
    }
    if capture.blade_zone != Some(Zone::Battlefield) || capture.final_attachment != Some(equip_target) || entry_target == equip_target {
        return Err("Blade did not enter attached and re-equip to a distinct controlled creature".to_owned());
    }
    if !capture.flash_cast_during_opponent_upkeep || capture.entry_choice_actor != Some(PlayerId(0)) {
        return Err("Blade was not cast with Flash during the opponent's upkeep and chosen by its controller".to_owned());
    }
    if capture.entry_stats != Some((Some(4), Some(4))) || capture.equip_stats != Some((Some(4), Some(4))) {
        return Err(format!("the entry and equip hosts did not each receive +1/+1: {:?} / {:?}", capture.entry_stats, capture.equip_stats));
    }
    if capture.equip_cost_paid != Some(ManaCost::new(1)) {
        return Err("equip did not pay exactly {1}".to_owned());
    }
    let events = &capture.events;
    let blade_paid = event_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == blade && colors.len() == 3)).ok_or("Blade's generic {3} casting payment was not recorded")?;
    let blade_cast = event_index_after(events, blade_paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == blade)).ok_or("Blade cast receipt missing")?;
    let decision_opened = event_index_after(events, blade_cast, |event| matches!(event, GameEvent::DecisionOpened { decision, player: PlayerId(0), kind: ENTRY_DECISION, .. } if decision.0 > 0)).ok_or("non-targeting entry attachment decision did not open for its controller")?;
    let decision_id = match events.get(decision_opened) {
        Some(GameEvent::DecisionOpened { decision, .. }) => *decision,
        _ => return Err("entry attachment decision receipt is malformed".to_owned()),
    };
    let decision_completed = event_index_after(events, decision_opened, |event| matches!(event, GameEvent::DecisionCompleted { decision, player: PlayerId(0), kind: ENTRY_DECISION } if *decision == decision_id)).ok_or("entry attachment choice did not complete")?;
    let blade_resolved = event_index_after(events, decision_completed, |event| matches!(event, GameEvent::SpellResolved { card } if *card == blade)).ok_or("Blade did not resolve after its entry choice")?;
    let blade_entered = event_index_after(events, blade_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == blade)).ok_or("Blade battlefield entry missing")?;
    let entry_attached = event_index_after(events, blade_entered, |event| matches!(event, GameEvent::EquipmentAttached { equipment, target, previous: None } if *equipment == blade && *target == entry_target)).ok_or("Blade was not attached during its entry boundary")?;
    if events.iter().any(|event| matches!(event, GameEvent::TriggeredAbilityStacked { source, .. } if *source == blade)) {
        return Err("Grifter's Blade incorrectly created a triggered ability on entry".to_owned());
    }
    let equip_paid = event_index_after(events, entry_attached, |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: EQUIP_ABILITY, mana_cost } if *source == blade && *mana_cost == ManaCost::new(1))).ok_or("equip's exact {1} payment event missing")?;
    let equip_activated = event_index_after(events, equip_paid, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source, ability: EQUIP_ABILITY, .. } if *source == blade)).ok_or("Equip activation was not placed on the stack")?;
    let equipped_again = event_index_after(events, equip_activated, |event| matches!(event, GameEvent::EquipmentAttached { equipment, target, previous: Some(previous) } if *equipment == blade && *target == equip_target && *previous == entry_target)).ok_or("Equip did not move Blade to the second controlled creature")?;
    let equip_resolved = event_index_after(events, equipped_again, |event| matches!(event, GameEvent::AbilityResolved { source, ability: EQUIP_ABILITY, .. } if *source == blade)).ok_or("Equip ability did not resolve")?;
    let ended = event_index_after(events, equip_resolved, |event| matches!(event, GameEvent::GameEnded { winner: Some(PlayerId(0)) })).ok_or("terminal P0 win event missing")?;
    if !(blade_paid < blade_cast && blade_cast < decision_opened && decision_opened < decision_completed && decision_completed < blade_resolved && blade_resolved < blade_entered && blade_entered < entry_attached && entry_attached < equip_paid && equip_paid < equip_activated && equip_activated < equipped_again && equipped_again < equip_resolved && equip_resolved < ended) {
        return Err("cast, non-targeting entry choice, attachment, Equip, and terminal event order differs".to_owned());
    }
    let selected = [blade_paid, blade_cast, decision_opened, decision_completed, blade_resolved, blade_entered, entry_attached, equip_paid, equip_activated, equipped_again, equip_resolved, ended];
    let projected = project_selected_events(events, &selected);
    Ok(json!({
        "id": "rav-grifters-blade-flash-entry-attachment-equip-full-game",
        "seed": seed,
        "terminal": capture.terminal,
        "winner_seat": capture.winner,
        "turns": capture.turns,
        "accepted_moves": capture.accepted_moves,
        "rejected_moves": capture.rejected_moves,
        "blade_object": blade.0,
        "blade_definition": CARD_ID,
        "blade_zone": format!("{:?}", capture.blade_zone),
        "flash_cast_during_opponent_upkeep": capture.flash_cast_during_opponent_upkeep,
        "entry_choice_actor_seat": capture.entry_choice_actor.map(|player| player.0),
        "entry_target_object": entry_target.0,
        "equip_target_object": equip_target.0,
        "entry_target_power_toughness": capture.entry_stats,
        "equip_target_power_toughness": capture.equip_stats,
        "final_attachment": capture.final_attachment.map(|object| object.0),
        "equip_cost_paid": {"generic": 1, "colors": []},
        "events_digest": event_digest(events),
        "events": projected,
        "passed": true
    }))
}

fn validate_candidate_bindings() -> Result<(), String> {
    let definition = candidate_grifters_blade::definition();
    let registered_definition = card_definitions().into_iter().find(|value| value.id == CARD_ID).ok_or("Grifter's Blade definition missing from RAV registry")?;
    if definition != registered_definition || definition.id != CARD_ID || definition.name != CARD_NAME || definition.set_code != "RAV" || definition.mana_cost != ManaCost::new(3) || !definition.colors.is_empty() || definition.card_types != BTreeSet::from([CardType::Artifact]) || definition.keywords.as_slice() != [Keyword::Flash] || !definition.effects.is_empty() {
        return Err("Grifter's Blade definition differs from the printed identity contract".to_owned());
    }
    let entry = candidate_grifters_blade::entry_attachment_binding();
    if entry != (EntryAttachmentBinding { card_definition: CARD_ID, may_decline: true }) {
        return Err("Grifter's Blade entry choice must allow a non-targeting attach or decline".to_owned());
    }
    if rav_triggered_ability_bindings().iter().any(|binding| binding.card_definition == CARD_ID) {
        return Err("Grifter's Blade entry attachment must not be registered as a triggered ability".to_owned());
    }
    let equip = candidate_grifters_blade::equip_ability();
    let registered_equip = rav_activated_ability_bindings().into_iter().find(|binding| binding.card_definition == CARD_ID && binding.ability.id == EQUIP_ABILITY).ok_or("Grifter's Blade equip binding missing from RAV registry")?;
    if equip != registered_equip || equip.ability.mana_cost != ManaCost::new(1) || !equip.ability.sorcery_speed || equip.ability.targets.as_slice() != [TargetRequirement::ControlledCreature] || !matches!(equip.ability.effects.as_slice(), [Effect::AttachSourceToTarget { target: TargetRequirement::ControlledCreature, changes }] if changes.as_slice() == [ContinuousChange::ModifyPowerToughness { power: 1, toughness: 1 }]) {
        return Err("Grifter's Blade equip binding differs from its printed cost or recipient".to_owned());
    }
    let attachment = candidate_grifters_blade::attachment_binding();
    let registered_attachment = rav_attachment_bindings().into_iter().find(|binding| binding.card_definition == CARD_ID).ok_or("Grifter's Blade attachment metadata missing from RAV registry")?;
    if attachment != registered_attachment || attachment.kind != AttachmentKind::Equipment || attachment.target != TargetRequirement::ControlledCreature || attachment.changes.as_slice() != [ContinuousChange::ModifyPowerToughness { power: 1, toughness: 1 }] {
        return Err("Grifter's Blade attachment metadata differs from its persistent +1/+1".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH).map_err(|error| format!("could not read Grifter's Blade source: {error}"))?;
    validate_candidate_source_text(&source)
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let file = syn::parse_file(source).map_err(|_| "Grifter's Blade candidate source is not valid Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) { return Err("module attributes differ".to_owned()); }
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item) if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) => {
                if flatten_use_tree(&item.tree, Vec::new()).is_none() { return Err("unsupported candidate import".to_owned()); }
            }
            Item::Fn(function) => {
                if !functions.insert(function.sig.ident.to_string()) { return Err("duplicate candidate function".to_owned()); }
                validate_literal_function(function)?;
            }
            _ => return Err("candidate source contains executable top-level code".to_owned()),
        }
    }
    if functions != BTreeSet::from(["definition".to_owned(), "entry_attachment_binding".to_owned(), "equip_ability".to_owned(), "attachment_binding".to_owned()]) {
        return Err("candidate source does not have its four fixed constructor functions".to_owned());
    }
    Ok(())
}

fn validate_literal_function(function: &syn::ItemFn) -> Result<(), String> {
    let attrs_ok = function.attrs.iter().all(|attribute| is_doc_attribute(attribute) || attribute.path().is_ident("must_use") && matches!(attribute.meta, Meta::Path(_)));
    if !attrs_ok || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate")) || !function.sig.inputs.is_empty() || !function.sig.generics.params.is_empty() || function.block.stmts.len() != 1 || !matches!(&function.sig.output, syn::ReturnType::Type(_, output) if matches!(&**output, syn::Type::Path(path) if path.qself.is_none())) {
        return Err(format!("{} must be a parameterless crate-visible literal constructor", function.sig.ident));
    }
    let Stmt::Expr(Expr::Struct(literal), None) = &function.block.stmts[0] else { return Err(format!("{} must return one literal", function.sig.ident)); };
    if literal.rest.is_some() { return Err("struct update syntax is not accepted".to_owned()); }
    validate_safe_expr(&Expr::Struct(literal.clone()))
}

fn validate_safe_expr(expression: &Expr) -> Result<(), String> {
    match expression {
        Expr::Path(_) | Expr::Lit(_) => Ok(()),
        Expr::Reference(value) => validate_safe_expr(&value.expr),
        Expr::Array(value) => { for item in &value.elems { validate_safe_expr(item)?; } Ok(()) },
        Expr::Struct(value) => { for field in &value.fields { validate_safe_expr(&field.expr)?; } Ok(()) },
        Expr::Call(value) => {
            let Expr::Path(function) = &*value.func else { return Err("only named, approved constructors are accepted".to_owned()); };
            if !["ManaCost::new", "BTreeSet::new", "BTreeSet::from"].contains(&function.path.segments.iter().map(|segment| segment.ident.to_string()).collect::<Vec<_>>().join("::").as_str()) {
                return Err("candidate contains a non-allowlisted function call".to_owned());
            }
            for argument in &value.args { validate_safe_expr(argument)?; }
            Ok(())
        }
        Expr::Macro(value) => {
            if value.mac.path.leading_colon.is_some() || value.mac.path.segments.len() != 1 || !value.mac.path.is_ident("vec") { return Err("candidate contains a non-allowlisted macro".to_owned()); }
            let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
            let expressions = parser.parse2(value.mac.tokens.clone()).map_err(|_| "vector macro contents are not expressions".to_owned())?;
            for expression in expressions { validate_safe_expr(&expression)?; }
            Ok(())
        }
        _ => Err("candidate source contains an executable expression outside literal constructors".to_owned()),
    }
}

fn side_effect_source_guard_fixture() -> Result<(bool, bool), String> {
    let source = std::fs::read_to_string(SOURCE_PATH).map_err(|error| error.to_string())?;
    let signature = "pub(crate) fn definition() -> CardDefinition {";
    let start = source.find(signature).ok_or("candidate definition signature missing")?;
    let body = start + signature.len();
    let fixture = format!("{}\n        std::process::abort();{}", &source[..body], &source[body..]);
    let parsed = syn::parse_file(&fixture).is_ok();
    let rejected = parsed && validate_candidate_source_text(&fixture).is_err();
    Ok((parsed, rejected))
}

fn flatten_use_tree(tree: &UseTree, prefix: Vec<String>) -> Option<Vec<Vec<String>>> {
    match tree {
        UseTree::Path(path) => { let mut next = prefix; next.push(path.ident.to_string()); flatten_use_tree(&path.tree, next) }
        UseTree::Name(name) => { let mut next = prefix; next.push(name.ident.to_string()); Some(vec![next]) }
        UseTree::Group(group) => { let mut paths = Vec::new(); for item in &group.items { paths.extend(flatten_use_tree(item, prefix.clone())?); } Some(paths) }
        UseTree::Rename(_) | UseTree::Glob(_) => None,
    }
}

fn is_doc_attribute(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("doc") && matches!(attribute.meta, Meta::NameValue(_))
}

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let unique: Vec<_> = printings.iter().copied().filter(|card| names.insert(card.name)).collect();
    if unique.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT || unique.iter().filter(|card| card.collector_number == 263 && card.name == CARD_NAME).count() != 1 {
        return Err("RAV catalog identity or printing totals differ from the checked contract".to_owned());
    }
    Ok(unique)
}

fn card_status(card: &CatalogCard) -> Value {
    json!({"collector_number": card.collector_number, "name": card.name, "semantic_status": format!("{:?}", card.semantic_status), "pilot_card_passed": card.collector_number == 263 && card.name == CARD_NAME})
}

fn output_root() -> Option<std::path::PathBuf> {
    let arguments: Vec<_> = std::env::args_os().collect();
    arguments.windows(2).find(|pair| pair[0] == "--output-root").map(|pair| pair[1].clone().into())
}

fn write_outputs(root: &std::path::Path, report: &Value) -> Result<(), String> {
    std::fs::create_dir_all(root).map_err(|error| error.to_string())?;
    std::fs::write(root.join("result.json"), serde_json::to_vec_pretty(report).map_err(|error| error.to_string())?).map_err(|error| error.to_string())?;
    std::fs::write(root.join("reward.txt"), "0.00000000\n").map_err(|error| error.to_string())?;
    Ok(())
}

fn event_digest(events: &[GameEvent]) -> String {
    let bytes = events.iter().map(|event| format!("{event:?}\n")).collect::<String>();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn project_selected_events(events: &[GameEvent], indices: &[usize]) -> Vec<ProjectedEvent> {
    let selected: Vec<_> = indices.iter().filter_map(|index| events.get(*index).cloned()).collect();
    project_events(&selected)
}

fn event_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    let matches: Vec<_> = events.iter().enumerate().filter_map(|(index, event)| predicate(event).then_some(index)).collect();
    if matches.len() == 1 { Some(matches[0]) } else { None }
}

fn event_index_after(events: &[GameEvent], after: usize, predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().enumerate().skip(after + 1).find_map(|(index, event)| predicate(event).then_some(index))
}
