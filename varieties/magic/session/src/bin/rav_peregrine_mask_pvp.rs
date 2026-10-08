//! Full-game diagnostic PvP authority for RAV Peregrine Mask.

use cardbench_magic_engine::{
    AbilityActivation, CardDefinition, CardType, Color, CombatBlock, DeckEntry, DeckList,
    Effect, Game, GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, Step, Target, TargetRequirement, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings,
    rav_attachment_triggered_ability_bindings as attachment_triggers, rav_basic_land_type_bindings,
    rav_land_entry_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_static_continuous_effect_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};
use std::process::ExitCode;

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-PEREGRINE-MASK";
const CARD_NAME: &str = "Peregrine Mask";
const ABILITY_ID: &str = "equip-defender-flying-first-strike";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const SKYKNIGHT_ID: &str = "RAV-SKYKNIGHT-LEGIONNAIRE";
const MAX_TURNS: u32 = 60;
const MAX_POLICY_MOVES: u32 = 8_000;
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/peregrine_mask.rs"]
mod candidate_peregrine_mask;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            println!("{}", serde_json::to_string(&report).expect("report serializes"));
            if report["passed"].as_bool() == Some(true) {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(error) => {
            eprintln!("Peregrine Mask authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_bindings()?;
    let catalog = unique_catalog()?;
    let (seed, reference) = find_reference()?;
    let trace = validate_trace(&reference, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if reference != replay || project_events(&reference.events) != project_events(&replay.events) {
        return Err("Peregrine Mask full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "defender_removed_mutation_detected_in_game": mutation_detected(Mutation::NoDefender, seed),
        "flying_removed_mutation_detected_in_game": mutation_detected(Mutation::NoFlying, seed),
        "first_strike_removed_mutation_detected_in_game": mutation_detected(Mutation::NoFirstStrike, seed),
        "equip_cost_changed_mutation_detected_in_game": mutation_detected(Mutation::WrongEquipCost, seed),
        "sealed_mutation_authority": false
    });
    for key in [
        "defender_removed_mutation_detected_in_game",
        "flying_removed_mutation_detected_in_game",
        "first_strike_removed_mutation_detected_in_game",
        "equip_cost_changed_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!("Peregrine Mask gameplay mutation escaped the trace oracle: {key}"));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 268, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": "executable_definition_and_activated_ability", "card_definition": CARD_ID},
        "candidate_kind": "executable_definition_and_activated_ability",
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "301.1 and 301.5: artifact and Equipment characteristics",
                "117: timing and priority",
                "601.2: casting artifact spells",
                "602.2: activating equip at sorcery speed",
                "608.2: resolving activated abilities",
                "702.3: Defender",
                "702.9: Flying",
                "702.7: First Strike",
                "509-510: blocking and combat damage"
            ],
            "source_contract": "CardBench-authored semantic summary: Peregrine Mask is a colorless {1} Artifact Equipment; equip {2} at sorcery speed to a creature its controller controls; the equipped creature has Defender, Flying, and First Strike.",
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
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoDefender,
    NoFlying,
    NoFirstStrike,
    WrongEquipCost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    spell_casts: Vec<String>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    mask: Option<ObjectId>,
    target: Option<ObjectId>,
    skyknight: Option<ObjectId>,
    attached_to: Option<ObjectId>,
    mask_zone: Option<Zone>,
    target_zone: Option<Zone>,
    target_controlled_by_player0: bool,
    skyknight_zone: Option<Zone>,
    target_keywords_at_equip: Vec<Keyword>,
    target_can_attack_at_declaration: Option<bool>,
    defender_probe_attempted: bool,
    defender_probe_rejected_without_state_change: bool,
    target_blocked_skyknight: bool,
    target_damage_to_skyknight: i32,
    skyknight_damage_to_target: i32,
    equip_mana_paid: Option<ManaCost>,
}

fn find_reference() -> Result<(u64, CapturedMatch), String> {
    if let Ok(seed) = std::env::var("CARDBENCH_PEREGRINE_MASK_SEED") {
        let seed = seed
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_PEREGRINE_MASK_SEED must be an unsigned integer".to_owned())?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let Ok(capture) = run_match(seed, Mutation::None)
            && validate_trace(&capture, seed).is_ok()
        {
            return Ok((seed, capture));
        }
    }
    Err(format!("no clean Peregrine Mask full-game trace found in seeds 0..={MAX_TRACE_SEED}"))
}

fn mutation_detected(mutation: Mutation, seed: u64) -> bool {
    match run_match(seed, mutation) {
        Err(_) => true,
        Ok(capture) => validate_trace(&capture, seed).is_err(),
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definitions = card_definitions();
    let definition = candidate_peregrine_mask::definition();
    let registered = definitions
        .iter_mut()
        .find(|candidate| candidate.id == CARD_ID)
        .ok_or("Peregrine Mask definition is missing from the RAV registry")?;
    *registered = definition;

    let mut activations = rav_activated_ability_bindings();
    let activation_index = activations
        .iter()
        .position(|binding| binding.card_definition == CARD_ID)
        .ok_or("Peregrine Mask equip ability is missing from the RAV registry")?;
    activations[activation_index] = candidate_peregrine_mask::activated_ability();
    let mut attachments = rav_attachment_bindings();
    let attachment_index = attachments
        .iter()
        .position(|binding| binding.card_definition == CARD_ID)
        .ok_or("Peregrine Mask Equipment binding is missing from the RAV registry")?;
    attachments[attachment_index] = candidate_peregrine_mask::attachment_binding();
    match mutation {
        Mutation::None => {}
        Mutation::WrongEquipCost => {
            activations[activation_index].ability.mana_cost = ManaCost::new(3);
        }
        Mutation::NoDefender | Mutation::NoFlying | Mutation::NoFirstStrike => {
            let removed = match mutation {
                Mutation::NoDefender => Keyword::Defender,
                Mutation::NoFlying => Keyword::Flying,
                Mutation::NoFirstStrike => Keyword::FirstStrike,
                _ => unreachable!(),
            };
            attachments[attachment_index]
                .changes
                .retain(|change| !matches!(change, cardbench_magic_engine::ContinuousChange::AddKeyword(keyword) if *keyword == removed));
            if let Some(Effect::AttachSourceToTarget { changes, .. }) =
                activations[activation_index].ability.effects.first_mut()
            {
                changes.retain(|change| !matches!(change, cardbench_magic_engine::ContinuousChange::AddKeyword(keyword) if *keyword == removed));
            }
        }
    }
    let mut triggers = rav_triggered_ability_bindings();
    triggers.extend(attachment_triggers());
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
            DeckEntry { card: SKYKNIGHT_ID.to_owned(), count: 4 },
            DeckEntry { card: "RAV-MOUNTAIN".to_owned(), count: 28 },
            DeckEntry { card: "RAV-PLAINS".to_owned(), count: 28 },
        ],
        sideboard: vec![],
    };
    for (name, deck) in [("Peregrine Mask deck", &p0_deck), ("Skyknight deck", &p1_deck)] {
        let count: usize = deck.mainboard.iter().map(|entry| usize::from(entry.count)).sum();
        if count != 60 {
            return Err(format!("{name} has {count} cards; expected 60"));
        }
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_POLICY_MOVES,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed).map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &p0_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &p1_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), config.opening_hand_size)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(MaskPolicy::new(PlayerId(0))),
        Box::new(MaskPolicy::new(PlayerId(1))),
    ];
    let mut accepted_moves = 0_u32;
    let mut attempted_moves = 0_u32;
    let mut defender_probe_attempted = false;
    let mut defender_probe_rejected_without_state_change = false;
    let mut target_can_attack_at_declaration = None;
    let mut target_keywords_at_equip = Vec::new();
    let mut equip_mana_paid = None;
    let mut mask_object = None;
    let mut target_object = None;
    let mut skyknight_object = None;
    let mut skyknight_stack_object = None;
    let mut skyknight_resolution_pending = false;
    let mut spell_casts = Vec::new();
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || accepted_moves >= MAX_POLICY_MOVES {
            return Err(format!("Peregrine Mask duel exceeded its bound at turn {}", game.turn));
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("policy seat is unavailable")?;

        let target = target_object;
        if player == PlayerId(1) && view.step == Step::DeclareAttackers && view.active_player == PlayerId(1)
            && let Some(card) = view.own_battlefield.iter().find(|card| card.definition == Some(SKYKNIGHT_ID))
        {
            skyknight_object = Some(card.id);
        }
        let equipped_target = target.is_some_and(|target| {
            mask_object.and_then(|mask| game.object(mask).ok().and_then(|object| object.attached_to)) == Some(target)
        });
        if !defender_probe_attempted
            && player == PlayerId(0)
            && view.step == Step::DeclareAttackers
            && view.active_player == PlayerId(0)
            && equipped_target
            && let Some(target) = target
        {
            defender_probe_attempted = true;
            let before_events = game.event_log.len();
            let before_turn = game.turn;
            let before_step = game.step;
            let before_priority = game.priority;
            let before_zone = game.zone_of(target);
            let before_keywords = game.characteristics(target).ok().map(|c| c.keywords);
            attempted_moves += 1;
            let rejected = game
                .submit_policy_move(
                    player,
                    policy.id(),
                    PolicyAction::DeclareAttackers { attackers: vec![target] },
                )
                .is_err();
            defender_probe_rejected_without_state_change = rejected
                && game.event_log.len() == before_events
                && game.turn == before_turn
                && game.step == before_step
                && game.priority == before_priority
                && game.zone_of(target) == before_zone
                && game.characteristics(target).ok().map(|c| c.keywords) == before_keywords
                && game.view_for_player(player).is_ok_and(|after| !after.attackers_declared);
            if rejected {
                if !defender_probe_rejected_without_state_change {
                    return Err("illegal Defender attack probe changed game state".to_owned());
                }
            } else {
                accepted_moves += 1;
                game.validate_invariants().map_err(|error| error.to_string())?;
                continue;
            }
        }

        if view.step == Step::DeclareAttackers && view.active_player == PlayerId(0) && equipped_target
            && let Some(target) = target
            && let Some(card) = view.own_battlefield.iter().find(|card| card.id == target)
        {
            target_can_attack_at_declaration = Some(card.can_attack);
        }
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
        if matches!(&action, PolicyAction::ActivateAbility { activation } if activation.ability_id == ABILITY_ID)
            && let Some(ability) = rav_activated_ability_bindings().into_iter().find(|binding| binding.card_definition == CARD_ID)
        {
            equip_mana_paid = Some(ability.ability.mana_cost);
        }
        let cursor = game.event_log.len();
        attempted_moves += 1;
        game.submit_policy_move(player, policy.id(), action).map_err(|error| {
            format!("Peregrine Mask policy move rejected at turn {} {:?}: {error}", view.turn, view.step)
        })?;
        accepted_moves += 1;
        game.validate_invariants().map_err(|error| format!("Peregrine Mask post-move invariant failed: {error}"))?;
        for event in &game.event_log[cursor..] {
            if let GameEvent::SpellCast { player: caster, card } = event
                && let Ok(definition) = game.card_definition(*card)
            {
                spell_casts.push(format!("P{}:{}:cast-object={}", caster.0, definition.id, card.0));
                if definition.id == CARD_ID && mask_object.is_none() { mask_object = Some(*card); }
                if definition.id == WATCHWOLF_ID && target_object.is_none() { target_object = Some(*card); }
                if definition.id == SKYKNIGHT_ID && skyknight_stack_object.is_none() { skyknight_stack_object = Some(*card); }
            }
            match event {
                GameEvent::SpellResolved { card } if Some(*card) == skyknight_stack_object => skyknight_resolution_pending = true,
                GameEvent::CardMoved { card, to: Zone::Battlefield } if skyknight_resolution_pending => {
                    skyknight_object = Some(*card);
                    skyknight_resolution_pending = false;
                }
                GameEvent::AttackersDeclared { player: PlayerId(1), attackers } if !attackers.is_empty() => {
                    skyknight_object = attackers.first().copied();
                }
                _ => {}
            }
        }
        if game.event_log[cursor..].iter().any(|event| {
            matches!(event, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if game.card_definition(*source).is_ok_and(|definition| definition.id == CARD_ID))
        }) && let Some(target) = target
        {
            target_keywords_at_equip = game
                .characteristics(target)
                .map(|characteristics| characteristics.keywords)
                .unwrap_or_default();
        }
    }
    let mask = mask_object;
    let target = target_object;
    let skyknight = skyknight_object;
    let attached_to = mask.and_then(|object| game.object(object).ok().and_then(|o| o.attached_to));
    let target_damage_to_skyknight = game.event_log.iter().filter_map(|event| match event {
        GameEvent::DamageDealtToPermanent { source, permanent, amount }
            if Some(*source) == target && Some(*permanent) == skyknight => Some(*amount),
        _ => None,
    }).sum();
    let skyknight_damage_to_target = game.event_log.iter().filter_map(|event| match event {
        GameEvent::DamageDealtToPermanent { source, permanent, amount }
            if Some(*source) == skyknight && Some(*permanent) == target => Some(*amount),
        _ => None,
    }).sum();
    let target_blocked_skyknight = game.event_log.iter().any(|event| {
        matches!(event, GameEvent::BlockersDeclared { player: PlayerId(0), assignments }
            if assignments.iter().any(|(attacker, blocker)| Some(*attacker) == skyknight && Some(*blocker) == target))
    });
    let equip_mana_paid = game.event_log.iter().find_map(|event| match event {
        GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: ABILITY_ID, mana_cost }
            if Some(*source) == mask => Some(mana_cost.clone()),
        _ => None,
    }).or(equip_mana_paid);
    let winner = game.winner().map(|winner| winner.0);
    Ok(CapturedMatch {
        events: game.event_log.clone(),
        spell_casts,
        terminal: winner.map_or_else(|| "Draw".to_owned(), |seat| format!("Winner(PlayerId({seat}))")),
        winner,
        turns: game.turn,
        accepted_moves,
        rejected_moves: attempted_moves.saturating_sub(accepted_moves),
        mask,
        target,
        skyknight,
        attached_to,
        mask_zone: mask.and_then(|object| game.zone_of(object)),
        target_zone: target.and_then(|object| game.zone_of(object)),
        target_controlled_by_player0: target.is_some_and(|object| game.controller_of(object).ok() == Some(PlayerId(0))),
        skyknight_zone: skyknight.and_then(|object| game.zone_of(object)),
        target_keywords_at_equip,
        target_can_attack_at_declaration,
        defender_probe_attempted,
        defender_probe_rejected_without_state_change,
        target_blocked_skyknight,
        target_damage_to_skyknight,
        skyknight_damage_to_target,
        equip_mana_paid,
    })
}

struct MaskPolicy {
    player: PlayerId,
    watchwolves_cast: u8,
    skyknights_cast: u8,
    masks_cast: u8,
    equipment_activated: bool,
    blocker_used: bool,
    skyknight_attack_used: bool,
}

impl MaskPolicy {
    fn new(player: PlayerId) -> Self {
        Self { player, watchwolves_cast: 0, skyknights_cast: 0, masks_cast: 0, equipment_activated: false, blocker_used: false, skyknight_attack_used: false }
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
        let land = view.own_battlefield.iter().find(|card| card.card_types.contains(&CardType::Land) && !card.tapped && preferred.is_none_or(|color| card.mana_colors.contains(&color)))
            .or_else(|| view.own_battlefield.iter().find(|card| card.card_types.contains(&CardType::Land) && !card.tapped))?;
        let color = preferred.filter(|color| land.mana_colors.contains(color)).or_else(|| land.mana_colors.iter().next().copied())?;
        Some(PolicyAction::ActivateManaAbility { land: land.id, color })
    }

    fn cast(&mut self, view: &GameView, definition: &'static str, cost: ManaCost) -> Option<PolicyAction> {
        if let Some(mana_selection) = Self::payment(view, &cost) {
            let card = view.hand.iter().find(|card| card.definition == Some(definition))?;
            match definition {
                CARD_ID => self.masks_cast = self.masks_cast.saturating_add(1),
                WATCHWOLF_ID => self.watchwolves_cast = self.watchwolves_cast.saturating_add(1),
                SKYKNIGHT_ID => self.skyknights_cast = self.skyknights_cast.saturating_add(1),
                _ => {}
            }
            return Some(PolicyAction::CastWithPayment {
                request: cardbench_magic_engine::CastRequest { card: card.id, targets: vec![], convoke: vec![], payment_mana_abilities: vec![] },
                chosen_x: None,
                mana_selection,
            });
        }
        let missing = cost.colored.iter().find(|color| view.mana_pool.amount(**color) == 0).copied()
            .or_else(|| (Self::total_mana(view) < u16::from(cost.generic) + cost.colored.len() as u16).then_some(cost.colored.first().copied().unwrap_or(Color::Green)));
        Self::mana_source(view, missing)
    }
}

impl CodePolicy for MaskPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) { "rav.peregrine-mask-defender-flight-first-strike.v1" } else { "rav.skyknight-legionnaire-flying-attack.v1" }
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player { return PolicyAction::PassPriority; }
        if view.step == Step::DeclareAttackers && view.active_player == self.player && !view.attackers_declared {
            if self.player == PlayerId(1) {
                let equipped_watchwolf_is_visible = view.opponent_battlefield.iter().any(|card| card.definition == Some(WATCHWOLF_ID)
                    && card.keywords.contains(&Keyword::Defender)
                    && card.keywords.contains(&Keyword::Flying)
                    && card.keywords.contains(&Keyword::FirstStrike));
                let attackers = if !self.skyknight_attack_used && equipped_watchwolf_is_visible {
                    view.own_battlefield.iter().find(|card| card.definition == Some(SKYKNIGHT_ID) && card.can_attack).map(|card| card.id).into_iter().collect()
                } else { vec![] };
                self.skyknight_attack_used |= !attackers.is_empty();
                return PolicyAction::DeclareAttackers { attackers };
            }
            return PolicyAction::DeclareAttackers {
                attackers: if self.player == PlayerId(0) {
                    view.own_battlefield.iter().filter(|card| card.can_attack && card.card_types.contains(&CardType::Creature)).map(|card| card.id).collect()
                } else {
                    view.own_battlefield.iter().filter(|card| card.can_attack && card.card_types.contains(&CardType::Creature)).map(|card| card.id).collect()
                },
            };
        }
        if view.step == Step::DeclareBlockers && view.active_player != self.player && !view.blockers_declared {
            if self.player == PlayerId(0) && !self.blocker_used {
                let attacker = view.combat_attackers.iter().find(|card| card.definition == Some(SKYKNIGHT_ID));
                let blocker = view.own_battlefield.iter().find(|card| card.definition == Some(WATCHWOLF_ID) && card.can_block);
                if let (Some(attacker), Some(blocker)) = (attacker, blocker) {
                    self.blocker_used = true;
                    return PolicyAction::DeclareBlockers { assignments: vec![CombatBlock { attacker: attacker.id, blocker: blocker.id }] };
                }
            }
            return PolicyAction::DeclareBlockers { assignments: vec![] };
        }
        if view.priority != self.player || view.active_player != self.player || view.step != Step::PrecombatMain || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            if let Some(land) = view.hand.iter().find(|card| card.card_types.contains(&CardType::Land)) {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if self.player == PlayerId(0) {
            if (self.watchwolves_cast == 0 || (self.equipment_activated && self.watchwolves_cast < 4)) && view.hand.iter().any(|card| card.definition == Some(WATCHWOLF_ID))
                && let Some(action) = self.cast(view, WATCHWOLF_ID, ManaCost::with_colors(0, [Color::Green, Color::White])) { return action; }
            if self.masks_cast == 0 && self.watchwolves_cast > 0 && view.hand.iter().any(|card| card.definition == Some(CARD_ID))
                && let Some(action) = self.cast(view, CARD_ID, ManaCost::new(1)) { return action; }
            if !self.equipment_activated && self.masks_cast > 0 && self.watchwolves_cast > 0
                && let Some(mask) = view.own_battlefield.iter().find(|card| card.definition == Some(CARD_ID))
                && let Some(target) = view.own_battlefield.iter().find(|card| card.definition == Some(WATCHWOLF_ID) && card.card_types.contains(&CardType::Creature))
            {
                if Self::total_mana(view) >= 2 {
                    self.equipment_activated = true;
                    return PolicyAction::ActivateAbility { activation: AbilityActivation {
                        source: mask.id, ability_id: ABILITY_ID, sacrifice_sources: vec![], additional_tap_creatures: vec![], discard_cards: vec![], targets: vec![Target::Permanent(target.id)],
                    }};
                }
                if let Some(action) = Self::mana_source(view, None) { return action; }
            }
        } else if self.skyknights_cast < 1 && view.hand.iter().any(|card| card.definition == Some(SKYKNIGHT_ID))
            && let Some(action) = self.cast(view, SKYKNIGHT_ID, ManaCost::with_colors(1, [Color::Red, Color::White])) { return action; }
        PolicyAction::PassPriority
    }
}

fn validate_trace(capture: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if capture.terminal != "Winner(PlayerId(0))" || capture.winner != Some(0) || capture.rejected_moves != 1 {
        return Err(format!("Peregrine Mask duel did not produce a clean P0 win with exactly one rejected Defender probe (terminal={}, winner={:?}, rejected={}, probe={}, unchanged={}, mask={:?}, target={:?}, attached={:?}, turns={})", capture.terminal, capture.winner, capture.rejected_moves, capture.defender_probe_attempted, capture.defender_probe_rejected_without_state_change, capture.mask, capture.target, capture.attached_to, capture.turns));
    }
    let mask = capture.mask.ok_or("Peregrine Mask was not cast")?;
    let target = capture.target.ok_or("Watchwolf was not cast")?;
    let skyknight = capture.skyknight.ok_or("Skyknight Legionnaire was not cast")?;
    if capture.mask_zone != Some(Zone::Battlefield) || capture.attached_to != Some(target) || capture.target_zone != Some(Zone::Battlefield) || !capture.target_controlled_by_player0 {
        return Err("Peregrine Mask did not remain attached to its exact controlled Watchwolf".to_owned());
    }
    if ![Keyword::Defender, Keyword::Flying, Keyword::FirstStrike].iter().all(|keyword| capture.target_keywords_at_equip.contains(keyword)) {
        return Err("equipped Watchwolf did not gain Defender, Flying, and First Strike".to_owned());
    }
    if capture.target_can_attack_at_declaration != Some(false) || !capture.defender_probe_attempted || !capture.defender_probe_rejected_without_state_change {
        return Err("Defender did not prohibit the equipped Watchwolf from attacking without changing state".to_owned());
    }
    let skyknight_destroyed = capture.events.iter().any(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if Some(*card) == capture.skyknight));
    if !capture.target_blocked_skyknight || !skyknight_destroyed || capture.target_damage_to_skyknight != 3 || capture.skyknight_damage_to_target != 0 {
        return Err(format!("Flying block and First Strike damage did not remove the Skyknight before it could damage Watchwolf (blocked={}, sky-zone={:?}, target-zone={:?}, target-damage={}, sky-damage={}, keywords={:?}, turns={}, casts={:?})", capture.target_blocked_skyknight, capture.skyknight_zone, capture.target_zone, capture.target_damage_to_skyknight, capture.skyknight_damage_to_target, capture.target_keywords_at_equip, capture.turns, capture.spell_casts));
    }
    if capture.equip_mana_paid != Some(ManaCost::new(2)) {
        return Err("Peregrine Mask must pay exact {2} equip cost".to_owned());
    }
    let mask_paid = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == mask && colors.len() == 1)).ok_or("Peregrine Mask exact generic {1} spell payment missing")?;
    let mask_cast = event_index_after(&capture.events, mask_paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == mask)).ok_or("Peregrine Mask cast receipt missing")?;
    let mask_resolved = event_index_after(&capture.events, mask_cast, |event| matches!(event, GameEvent::SpellResolved { card } if *card == mask)).ok_or("Peregrine Mask spell resolution receipt missing")?;
    let mask_enters = event_index_after(&capture.events, mask_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == mask)).ok_or("Peregrine Mask battlefield entry receipt missing")?;
    let ability_paid = event_index_after(&capture.events, mask_enters, |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: ABILITY_ID, mana_cost } if *source == mask && *mana_cost == ManaCost::new(2))).ok_or("Peregrine Mask exact {2} AbilityManaPaid event missing")?;
    let activated = event_index_after(&capture.events, ability_paid, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source, ability: ABILITY_ID, .. } if *source == mask)).ok_or("Peregrine Mask equip ability did not use the stack")?;
    let attached = event_index_after(&capture.events, activated, |event| matches!(event, GameEvent::EquipmentAttached { equipment, target: attached_target, previous: None } if *equipment == mask && *attached_target == target)).ok_or("Peregrine Mask did not attach to the exact Watchwolf")?;
    let ability_resolved = event_index_after(&capture.events, attached, |event| matches!(event, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == mask)).ok_or("Peregrine Mask equip ability did not resolve")?;
    let attack = event_index_after(&capture.events, ability_resolved, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(1), attackers } if attackers.contains(&skyknight)).then_some(()).is_some()).ok_or("Skyknight did not attack after equipment resolved")?;
    let block = event_index_after(&capture.events, attack, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(0), assignments } if assignments.contains(&(skyknight, target)))).ok_or("equipped flying Watchwolf did not block the Skyknight")?;
    let first_strike_damage = event_index_after(&capture.events, block, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 3 } if *source == target && *permanent == skyknight)).ok_or("Watchwolf's three first-strike damage was not recorded")?;
    let skyknight_death = event_index_after(&capture.events, first_strike_damage, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == skyknight)).ok_or("Skyknight was not destroyed by first-strike damage")?;
    if capture.events.iter().enumerate().any(|(index, event)| index > first_strike_damage && matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, .. } if *source == skyknight && *permanent == target)) {
        return Err("Skyknight dealt ordinary combat damage after Watchwolf's first strike".to_owned());
    }
    let end = event_index_after(&capture.events, skyknight_death, |event| matches!(event, GameEvent::GameEnded { winner: Some(PlayerId(0)) })).ok_or("terminal P0 victory receipt missing")?;
    if !(mask_paid < mask_cast && mask_cast < mask_resolved && mask_resolved < mask_enters && mask_enters < ability_paid && ability_paid < activated && activated < attached && attached < ability_resolved && ability_resolved < attack && attack < block && block < first_strike_damage && first_strike_damage < skyknight_death && skyknight_death < end) {
        return Err("cast, equip, flying block, First Strike damage, and terminal event order differs".to_owned());
    }
    let selected = [mask_paid, mask_cast, mask_resolved, mask_enters, ability_paid, activated, attached, ability_resolved, attack, block, first_strike_damage, skyknight_death, end];
    let events = project_selected_events(&capture.events, &selected);
    Ok(json!({
        "id": "rav-peregrine-mask-defender-flying-first-strike-full-game",
        "seed": seed,
        "terminal": capture.terminal,
        "winner_seat": capture.winner,
        "turns": capture.turns,
        "accepted_moves": capture.accepted_moves,
        "rejected_moves": capture.rejected_moves,
        "mask_object": mask.0,
        "mask_definition": CARD_ID,
        "mask_zone": format!("{:?}", capture.mask_zone),
        "target_object": target.0,
        "target_definition": WATCHWOLF_ID,
        "target_zone": format!("{:?}", capture.target_zone),
        "target_controlled_by_player0": capture.target_controlled_by_player0,
        "equipment_attached_to_exact_target": capture.attached_to == Some(target),
        "target_keywords_at_equip": capture.target_keywords_at_equip.iter().map(|keyword| format!("{keyword:?}")).collect::<Vec<_>>(),
        "target_can_attack_at_declaration": capture.target_can_attack_at_declaration,
        "defender_illegal_attack_probe_rejected_without_state_change": capture.defender_probe_rejected_without_state_change,
        "skyknight_object": skyknight.0,
        "skyknight_definition": SKYKNIGHT_ID,
        "skyknight_zone": format!("{:?}", capture.skyknight_zone),
        "skyknight_destroyed_by_first_strike": skyknight_destroyed,
        "target_blocked_skyknight": capture.target_blocked_skyknight,
        "target_damage_to_skyknight": capture.target_damage_to_skyknight,
        "skyknight_damage_to_target": capture.skyknight_damage_to_target,
        "equip_mana_paid": {"generic": 2, "colors": []},
        "events_digest": event_digest(&capture.events),
        "events": events,
        "passed": true
    }))
}

fn validate_candidate_bindings() -> Result<(), String> {
    let definition = candidate_peregrine_mask::definition();
    let registered_definition = card_definitions().into_iter().find(|value| value.id == CARD_ID).ok_or("Peregrine Mask missing from RAV registry")?;
    let expected = CardDefinition {
        id: CARD_ID,
        name: CARD_NAME,
        set_code: "RAV",
        mana_cost: ManaCost::new(1),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact]),
        is_basic_land: false,
        supported_rules: &["full-rules-fidelity", "colorless-equipment-casting", "equipment-defender-flying-first-strike", "sorcery-speed-equip-two"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    };
    if definition != expected || definition != registered_definition {
        return Err("Peregrine Mask literal is outside the printed identity/source contract".to_owned());
    }
    let ability = candidate_peregrine_mask::activated_ability();
    let registered_ability = rav_activated_ability_bindings().into_iter().find(|value| value.card_definition == CARD_ID).ok_or("Peregrine Mask equip ability missing from RAV registry")?;
    if ability != registered_ability || ability.ability.id != ABILITY_ID || ability.ability.mana_cost != ManaCost::new(2) || !ability.ability.sorcery_speed || ability.ability.targets != [TargetRequirement::ControlledCreature] || !matches!(ability.ability.effects.as_slice(), [Effect::AttachSourceToTarget { target: TargetRequirement::ControlledCreature, changes }] if changes == &vec![cardbench_magic_engine::ContinuousChange::AddKeyword(Keyword::Defender), cardbench_magic_engine::ContinuousChange::AddKeyword(Keyword::Flying), cardbench_magic_engine::ContinuousChange::AddKeyword(Keyword::FirstStrike)]) {
        return Err("Peregrine Mask equip ability is outside the printed source contract".to_owned());
    }
    let attachment = candidate_peregrine_mask::attachment_binding();
    let registered_attachment = rav_attachment_bindings().into_iter().find(|value| value.card_definition == CARD_ID).ok_or("Peregrine Mask Equipment attachment binding missing from RAV registry")?;
    if attachment != registered_attachment || attachment.kind != cardbench_magic_engine::AttachmentKind::Equipment || attachment.target != TargetRequirement::ControlledCreature || attachment.changes != match ability.ability.effects.first() { Some(Effect::AttachSourceToTarget { changes, .. }) => changes.clone(), _ => vec![] } {
        return Err("Peregrine Mask attachment binding is outside its granted-characteristics contract".to_owned());
    }
    Ok(())
}

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let catalog = rav_main_set_catalog();
    let mut seen = BTreeSet::new();
    let unique: Vec<_> = catalog.into_iter().filter(|card| seen.insert(card.name)).collect();
    if unique.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT || rav_main_set_catalog().len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT || unique.iter().filter(|card| card.collector_number == 268 && card.name == CARD_NAME).count() != 1 {
        return Err("RAV catalog identity or printing totals differ from the checked set contract".to_owned());
    }
    Ok(unique)
}

fn card_status(card: &CatalogCard) -> Value {
    json!({"collector_number":card.collector_number,"name":card.name,"semantic_status":format!("{:?}",card.semantic_status),"pilot_card_passed":card.collector_number == 268 && card.name == CARD_NAME})
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

fn unique_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    let matches: Vec<_> = events.iter().enumerate().filter_map(|(index, event)| predicate(event).then_some(index)).collect();
    if matches.len() == 1 { Some(matches[0]) } else { None }
}

fn event_index_after(events: &[GameEvent], after: usize, predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().enumerate().skip(after + 1).find_map(|(index, event)| predicate(event).then_some(index))
}
