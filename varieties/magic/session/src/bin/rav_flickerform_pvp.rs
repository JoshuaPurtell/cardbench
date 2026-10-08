//! Isolated full-game authority for RAV Flickerform's linked Aura blink.

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbilityBinding, CardDefinition, CardType, Color, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, LinkedExileMemberRole, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, TargetRequirement, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings, rav_basic_land_type_bindings,
    rav_land_entry_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_static_continuous_effect_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashSet};
use std::process::ExitCode;
use syn::parse::Parser;
use syn::{Expr, Item, Lit, Meta, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-FLICKERFORM";
const ABILITY_ID: &str = "linked-exile-attached-creature-and-auras";
const CLOAK_ID: &str = "RAV-MOLDERVINE-CLOAK";
const CREATURE_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/flickerform.rs";
const REFERENCE_SEED: u64 = 0;
const MAX_TURNS: u32 = 60;
const MAX_POLICY_MOVES: u32 = 8_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/flickerform.rs"]
mod candidate_flickerform;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Flickerform result: {error}");
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
            eprintln!("Flickerform PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    MissingLinkedBlink,
    WrongActivationCost,
    MissingAuraAttachment,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    source: Option<ObjectId>,
    creature: Option<ObjectId>,
    cloak: Option<ObjectId>,
    source_cast_payment: Vec<Color>,
    activation_cost: Option<ManaCost>,
    activation_turn: Option<u32>,
    scheduled_due_turn: Option<u32>,
    return_turn: Option<u32>,
    exile_incarnations: Vec<(ObjectId, u64)>,
    return_incarnations: Vec<(ObjectId, u64)>,
    returned: Vec<ObjectId>,
    creature_power_after_return: Option<i32>,
    attack_turns: Vec<u32>,
    damage: i32,
    life: [i64; 2],
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_registry_delegation()?;
    let catalog = unique_catalog()?;
    let reference = run_match(REFERENCE_SEED, Mutation::None)?;
    let trace = validate_trace(&reference)?;
    let replay = run_match(REFERENCE_SEED, Mutation::None)?;
    if reference != replay || project_events(&reference.events) != project_events(&replay.events) {
        return Err("Flickerform full-game replay was not deterministic".to_owned());
    }
    let missing_blink = mutation_detected(Mutation::MissingLinkedBlink)?;
    let wrong_cost = mutation_detected(Mutation::WrongActivationCost)?;
    let missing_aura = mutation_detected(Mutation::MissingAuraAttachment)?;
    if !missing_blink || !wrong_cost || !missing_aura {
        return Err("a Flickerform semantic mutation escaped the gameplay oracle".to_owned());
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 18, "definition_id": CARD_ID, "name": "Flickerform"},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "117: timing and priority",
                "303.4: Aura legality and attachment",
                "400.7: zone changes create new object incarnations",
                "602.1-602.2: activation and payment",
                "608.2: resolving activated abilities",
                "607.1: linked instructions and linked objects",
                "513.1: next end-step delayed return",
                "510.1: combat damage"
            ],
            "source_contract": "RAV Flickerform is a white {1}{W} Enchantment Aura for a Creature; {2}{W}{W}: exile the enchanted creature and all Auras attached to it, then return those cards at the next end step under their owners' control",
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
            "linked_blink_effect_removed_mutation_detected_in_game": missing_blink,
            "activation_cost_changed_mutation_detected_in_game": wrong_cost,
            "creature_aura_attachment_removed_mutation_detected_in_game": missing_aura,
            "sealed_mutation_authority": false
        },
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn mutation_detected(mutation: Mutation) -> Result<bool, String> {
    match run_match(REFERENCE_SEED, mutation) {
        Err(_) => Ok(true),
        Ok(capture) => Ok(validate_trace(&capture).is_err()),
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let definition = candidate_flickerform::definition();
    let mut binding = candidate_flickerform::activated_ability();
    match mutation {
        Mutation::None => {}
        Mutation::MissingLinkedBlink => binding.ability.effects.clear(),
        Mutation::WrongActivationCost => {
            binding.ability.mana_cost = ManaCost::with_colors(1, [Color::White, Color::White])
        }
        Mutation::MissingAuraAttachment => {
            let mut definition = definition.clone();
            definition.effects.clear();
            let (p0, p1) = decks();
            return run_match_with(seed, definition, binding, &p0, &p1);
        }
    }
    let (p0, p1) = decks();
    run_match_with(seed, definition, binding, &p0, &p1)
}

fn decks() -> (DeckList, DeckList) {
    let p0 = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: CLOAK_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: CREATURE_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 24,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 24,
            },
        ],
        sideboard: vec![],
    };
    let p1 = DeckList {
        mainboard: vec![DeckEntry {
            card: "RAV-FOREST".to_owned(),
            count: 60,
        }],
        sideboard: vec![],
    };
    (p0, p1)
}

fn run_match_with(
    seed: u64,
    definition: CardDefinition,
    binding: ActivatedAbilityBinding,
    p0: &DeckList,
    p1: &DeckList,
) -> Result<Captured, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Flickerform definition missing from RAV registry")? = definition;
    let mut abilities = rav_activated_ability_bindings();
    *abilities
        .iter_mut()
        .find(|ability| ability.card_definition == CARD_ID && ability.ability.id == ABILITY_ID)
        .ok_or("Flickerform activation missing from RAV registry")? = binding;
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        abilities,
        rav_triggered_ability_bindings(),
        rav_static_continuous_effect_bindings(),
        rav_land_entry_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_attachment_bindings(rav_attachment_bindings())
        .map_err(|error| error.to_string())?;
    if [
        p0.mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>(),
        p1.mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>(),
    ] != [60, 60]
    {
        return Err("Flickerform fixtures must use shuffled 60-card decks".to_owned());
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        max_turns: MAX_TURNS,
        max_policy_moves: MAX_POLICY_MOVES,
        ..DeckMatchConfig::default()
    };
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    for (player, deck) in [(PlayerId(0), p0), (PlayerId(1), p1)] {
        game.load_deck_into_library(player, deck)
            .map_err(|error| error.to_string())?;
        game.draw_opening_hand(player, config.opening_hand_size)
            .map_err(|error| error.to_string())?;
    }
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(FlickerPolicy::new(PlayerId(0))),
        Box::new(FlickerPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut source = None;
    let mut creature = None;
    let mut cloak = None;
    let mut source_cast_payment = Vec::new();
    let mut activation_source = None;
    let mut activation_cost = None;
    let mut activation_turn = None;
    let mut scheduled_due_turn = None;
    let mut return_turn = None;
    let mut exile_incarnations = Vec::new();
    let mut return_incarnations = Vec::new();
    let mut returned = Vec::new();
    let mut attack_turns = Vec::new();
    let mut damage = 0_i32;
    let mut creature_power_after_return = None;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err(format!(
                "Flickerform match exceeded its bound on turn {}",
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
        let cast = match &action {
            PolicyAction::CastWithPayment { request, .. } => game
                .card_definition(request.card)
                .ok()
                .map(|card| (request.card, card.id)),
            _ => None,
        };
        let activated = match &action {
            PolicyAction::ActivateAbility { activation } if activation.ability_id == ABILITY_ID => {
                Some(activation.clone())
            }
            _ => None,
        };
        let start = game.event_log.len();
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Flickerform policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        if let Some((card, id)) = cast {
            if id == CARD_ID {
                source = Some(card);
            }
            if id == CLOAK_ID {
                cloak = Some(card);
            }
        }
        if let Some(activation) = activated {
            activation_source = Some(activation.source);
            creature = game
                .object(activation.source)
                .ok()
                .and_then(|object| object.attached_to);
        }
        for event in &game.event_log[start..] {
            match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if Some(*card) == source => source_cast_payment = colors.clone(),
                GameEvent::AbilityManaPaid {
                    player: PlayerId(0),
                    source: known,
                    ability,
                    mana_cost,
                } if Some(*known) == activation_source && *ability == ABILITY_ID => {
                    activation_cost = Some(mana_cost.clone())
                }
                GameEvent::AbilityActivated {
                    source: known,
                    ability,
                    ..
                } if Some(*known) == activation_source && *ability == ABILITY_ID => {
                    activation_turn = Some(game.turn)
                }
                GameEvent::DelayedActionScheduled {
                    timing: cardbench_magic_engine::DelayedActionTiming::EndStep,
                    controller: PlayerId(0),
                    due_turn,
                    ..
                } if activation_source.is_some() => scheduled_due_turn = Some(*due_turn),
                GameEvent::CardMoved {
                    card,
                    to: Zone::Exile,
                } if Some(*card) == source || Some(*card) == creature || Some(*card) == cloak => {
                    let incarnation = game
                        .object(*card)
                        .map(|object| object.incarnation)
                        .unwrap_or_default();
                    exile_incarnations.push((*card, incarnation));
                }
                GameEvent::DelayedActionConsumed {
                    returned: members, ..
                } => {
                    return_turn = Some(game.turn);
                    returned = members.clone();
                    for object in members {
                        if let Ok(card) = game.object(*object) {
                            return_incarnations.push((*object, card.incarnation));
                        }
                    }
                    if let Some(target) = creature {
                        creature_power_after_return = game
                            .characteristics(target)
                            .ok()
                            .and_then(|chars| chars.power);
                    }
                }
                GameEvent::AttackersDeclared {
                    player: PlayerId(0),
                    attackers,
                } if creature.is_some_and(|target| attackers.contains(&target)) => {
                    attack_turns.push(game.turn)
                }
                GameEvent::DamageDealtToPlayer {
                    source: attacker,
                    player: PlayerId(1),
                    amount,
                } if creature == Some(*attacker) => damage += *amount,
                _ => {}
            }
        }
        if let Some(target) = creature {
            if cloak.is_none() && game.zone_of(target) == Some(Zone::Battlefield) {
                cloak = game.event_log.iter().find_map(|event| match event {
                    GameEvent::AuraAttached {
                        aura,
                        target: attached,
                    } if *attached == target
                        && game
                            .card_definition(*aura)
                            .is_ok_and(|def| def.id == CLOAK_ID) =>
                    {
                        Some(*aura)
                    }
                    GameEvent::AttachmentEstablishedWithoutContinuousEffect {
                        attachment,
                        target: attached,
                        ..
                    } if *attached == target
                        && game
                            .card_definition(*attachment)
                            .is_ok_and(|def| def.id == CLOAK_ID) =>
                    {
                        Some(*attachment)
                    }
                    _ => None,
                });
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
    let winner = game.winner().map(|player| player.0);
    Ok(Captured {
        events: game.event_log.clone(),
        terminal: winner.map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({seat}))"),
        ),
        winner,
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        source,
        creature,
        cloak,
        source_cast_payment,
        activation_cost,
        activation_turn,
        scheduled_due_turn,
        return_turn,
        exile_incarnations,
        return_incarnations,
        returned,
        creature_power_after_return,
        attack_turns,
        damage,
        life,
    })
}

struct FlickerPolicy {
    player: PlayerId,
    creature_cast: bool,
    source_cast: bool,
    cloak_cast: bool,
    activated: bool,
    creature: Option<ObjectId>,
    attachment_target: Option<ObjectId>,
}

impl FlickerPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            creature_cast: false,
            source_cast: false,
            cloak_cast: false,
            activated: false,
            creature: None,
            attachment_target: None,
        }
    }
}

impl CodePolicy for FlickerPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.flickerform.blink-aura-and-creature.v1"
        } else {
            "rav.flickerform.passive-opponent.v1"
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
            let attackers = if self.player == PlayerId(0) {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(CREATURE_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect()
            } else {
                vec![]
            };
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
        if view.priority != self.player || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }
        if self.player == PlayerId(0) && !self.activated {
            let aura = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(CARD_ID));
            if let (Some(flicker), Some(target)) = (aura, self.attachment_target) {
                let target_card = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.id == target && card.definition == Some(CREATURE_ID));
                let cloak_present = view
                    .own_battlefield
                    .iter()
                    .any(|card| card.definition == Some(CLOAK_ID))
                    && self.cloak_cast;
                let enough_white =
                    view.mana_pool.amount(Color::White) >= 2 && view.mana_pool.total() >= 4;
                if target_card.is_some() && cloak_present && enough_white {
                    self.activated = true;
                    return PolicyAction::ActivateAbility {
                        activation: AbilityActivation {
                            source: flicker.id,
                            ability_id: ABILITY_ID,
                            sacrifice_sources: vec![],
                            additional_tap_creatures: vec![],
                            discard_cards: vec![],
                            targets: vec![],
                        },
                    };
                }
                if !enough_white {
                    if let Some(action) = make_mana(
                        view,
                        &ManaCost::with_colors(2, [Color::White, Color::White]),
                    ) {
                        return action;
                    }
                }
            }
        }
        if view.active_player != self.player || !view.step.is_main() {
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
        if self.player == PlayerId(0) && !self.creature_cast {
            if let Some(action) = cast_card(
                view,
                CREATURE_ID,
                ManaCost::with_colors(0, [Color::Green, Color::White]),
            ) {
                if is_spell(&action, CREATURE_ID, view) {
                    self.creature_cast = true;
                    self.creature = match &action {
                        PolicyAction::CastWithPayment { request, .. } => Some(request.card),
                        _ => None,
                    };
                }
                return action;
            }
        }
        if self.player == PlayerId(0) && self.creature_cast && !self.source_cast {
            if let Some(target) = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(CREATURE_ID))
            {
                if let Some(action) = cast_aura(
                    view,
                    CARD_ID,
                    ManaCost::with_colors(1, [Color::White]),
                    target.id,
                ) {
                    if is_spell(&action, CARD_ID, view) {
                        self.source_cast = true;
                        self.attachment_target = Some(target.id);
                    }
                    return action;
                }
            }
        }
        if self.player == PlayerId(0) && self.source_cast && !self.cloak_cast {
            if let Some(target) = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(CREATURE_ID))
            {
                if let Some(action) = cast_aura(
                    view,
                    CLOAK_ID,
                    ManaCost::with_colors(2, [Color::Green]),
                    target.id,
                ) {
                    if is_spell(&action, CLOAK_ID, view) {
                        self.cloak_cast = true;
                        self.attachment_target = Some(target.id);
                    }
                    return action;
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn is_spell(action: &PolicyAction, id: &str, view: &GameView) -> bool {
    matches!(action, PolicyAction::CastWithPayment { request, .. } if view.hand.iter().any(|card| card.id == request.card && card.definition == Some(id)))
}

fn cast_card(view: &GameView, id: &'static str, cost: ManaCost) -> Option<PolicyAction> {
    if let Some(selection) = payment(view, &cost) {
        let card = view.hand.iter().find(|card| card.definition == Some(id))?;
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
    make_mana(view, &cost)
}

fn cast_aura(
    view: &GameView,
    id: &'static str,
    cost: ManaCost,
    target: ObjectId,
) -> Option<PolicyAction> {
    if let Some(selection) = payment(view, &cost) {
        let card = view.hand.iter().find(|card| card.definition == Some(id))?;
        return Some(PolicyAction::CastWithPayment {
            request: cardbench_magic_engine::CastRequest {
                card: card.id,
                targets: vec![Target::Permanent(target)],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: selection,
        });
    }
    make_mana(view, &cost)
}

fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: Vec<_> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let (_, amount) = available.iter_mut().find(|(known, _)| known == color)?;
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

fn make_mana(view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
    let need = cost
        .colored
        .iter()
        .copied()
        .find(|color| view.mana_pool.amount(*color) == 0)
        .or_else(|| {
            (view.mana_pool.total() < cost.generic.saturating_add(cost.colored.len() as u8))
                .then(|| cost.colored.first().copied())
                .flatten()
        });
    let land = view.own_battlefield.iter().find(|card| {
        card.card_types.contains(&CardType::Land)
            && !card.tapped
            && need.is_none_or(|color| card.mana_colors.contains(&color))
    })?;
    let color = need
        .or_else(|| land.mana_colors.iter().copied().next())
        .unwrap_or(Color::Green);
    Some(PolicyAction::ActivateManaAbility {
        land: land.id,
        color,
    })
}

fn validate_trace(c: &Captured) -> Result<Value, String> {
    if c.terminal != "Winner(PlayerId(0))" || c.winner != Some(0) || c.rejected != 0 {
        return Err("Flickerform duel did not reach a clean terminal win".to_owned());
    }
    let source = c.source.ok_or("Flickerform was not cast")?;
    let creature = c
        .creature
        .ok_or("Flickerform did not retain its exact enchanted creature")?;
    let cloak = c
        .cloak
        .ok_or("Moldervine Cloak was not attached to the same creature")?;
    if c.source_cast_payment.len() != 2
        || !c.source_cast_payment.contains(&Color::White)
        || c.activation_cost.as_ref()
            != Some(&ManaCost::with_colors(2, [Color::White, Color::White]))
        || c.activation_turn.is_none()
        || c.scheduled_due_turn != c.activation_turn
        || c.return_turn != c.activation_turn
        || c.returned.len() != 3
        || !c.returned.contains(&source)
        || !c.returned.contains(&creature)
        || !c.returned.contains(&cloak)
        || c.exile_incarnations.len() != 3
        || c.return_incarnations.len() != 3
        || c.creature_power_after_return != Some(6)
        || c.attack_turns.is_empty()
        || c.damage < 20
        || c.life[0] <= 0
        || c.life[1] > 0
    {
        return Err(format!(
            "Flickerform payment, linked members, return, reattachment, combat, or terminal facts differ: {c:?}"
        ));
    }
    let ids: BTreeSet<_> = c.returned.iter().copied().collect();
    if ids != BTreeSet::from([source, creature, cloak])
        || c.exile_incarnations
            .iter()
            .any(|(object, _)| !ids.contains(object))
        || c.return_incarnations
            .iter()
            .any(|(object, _)| !ids.contains(object))
    {
        return Err(
            "linked exile/return did not preserve the exact three card identities".to_owned(),
        );
    }
    let events = &c.events;
    let cast = event_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == source)).ok_or("Flickerform spell cast receipt missing")?;
    let paid_spell = event_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == source && colors.len() == 2 && colors.contains(&Color::White))).ok_or("Flickerform exact {1}{W} cast payment receipt missing")?;
    let spell_resolved = event_index_after(
        events,
        cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == source),
    )
    .ok_or("Flickerform spell resolution missing")?;
    let attached_source = event_index_after(events, spell_resolved, |event| matches!(event, GameEvent::AttachmentEstablishedWithoutContinuousEffect { attachment, target, .. } if *attachment == source && *target == creature)).ok_or("Flickerform did not attach to the target creature")?;
    let cloak_attached = event_index_after(events, attached_source, |event| matches!(event, GameEvent::AuraAttached { aura, target } if *aura == cloak && *target == creature)).ok_or("Moldervine Cloak did not attach to the same creature")?;
    let paid_ability = event_index_after(events, cloak_attached, |event| matches!(event, GameEvent::AbilityManaPaid { player: PlayerId(0), source: known, ability, mana_cost } if *known == source && *ability == ABILITY_ID && *mana_cost == ManaCost::with_colors(2, [Color::White, Color::White]))).ok_or("Flickerform activation payment receipt missing")?;
    let activated = event_index_after(events, paid_ability, |event| matches!(event, GameEvent::AbilityActivated { player: PlayerId(0), source: known, ability, .. } if *known == source && *ability == ABILITY_ID)).ok_or("Flickerform activation receipt missing")?;
    let scheduled = event_index_after(events, activated, |event| matches!(event, GameEvent::DelayedActionScheduled { timing: cardbench_magic_engine::DelayedActionTiming::EndStep, controller: PlayerId(0), due_turn, members, .. } if Some(*due_turn) == c.activation_turn && members.len() == 3 && members.iter().any(|member| member.object == creature && member.role == LinkedExileMemberRole::PrimaryCreature) && members.iter().any(|member| member.object == source && member.role == LinkedExileMemberRole::AttachedAura) && members.iter().any(|member| member.object == cloak && member.role == LinkedExileMemberRole::AttachedAura))).ok_or("linked group with the creature and both Auras was not scheduled for the next end step")?;
    let resolved = event_index_after(events, scheduled, |event| matches!(event, GameEvent::AbilityResolved { source: known, ability, .. } if *known == source && *ability == ABILITY_ID)).ok_or("Flickerform ability resolution receipt missing")?;
    let exile_moves: Vec<_> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            matches!(event, GameEvent::CardMoved { card, to: Zone::Exile } if [source, creature, cloak].contains(card))
                .then_some((index, event))
        })
        .collect();
    if exile_moves.len() != 3
        || exile_moves
            .iter()
            .map(|(_, event)| match event {
                GameEvent::CardMoved { card, .. } => *card,
                _ => unreachable!(),
            })
            .collect::<HashSet<_>>()
            .len()
            != 3
        || exile_moves
            .iter()
            .any(|(index, _)| *index <= activated || *index >= scheduled)
    {
        return Err(
            "all three linked cards must move to exile between activation and scheduling"
                .to_owned(),
        );
    }
    let consumed = event_index_after(events, scheduled, |event| matches!(event, GameEvent::DelayedActionConsumed { returned, .. } if returned.len() == 3 && returned.iter().any(|object| *object == creature) && returned.iter().any(|object| *object == source) && returned.iter().any(|object| *object == cloak))).ok_or("the exact linked group did not return at the end step")?;
    let returned_creature = event_index_after(events, scheduled, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == creature)).ok_or("creature return battlefield move missing")?;
    let returned_source = event_index_after(events, scheduled, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == source)).ok_or("Flickerform return battlefield move missing")?;
    let returned_cloak = event_index_after(events, scheduled, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == cloak)).ok_or("Moldervine Cloak return battlefield move missing")?;
    let reattached_flickerform = event_index_after(events, returned_source, |event| matches!(event, GameEvent::AttachmentEstablishedWithoutContinuousEffect { attachment, target, .. } if *attachment == source && *target == creature)).ok_or("Flickerform was not reattached to the returned creature")?;
    let cloak_modifier_restored = event_index_after(events, returned_cloak, |event| matches!(event, GameEvent::ContinuousEffectCreated { source: aura, target, layer: cardbench_magic_engine::Layer::PowerToughness } if *aura == cloak && *target == creature)).ok_or("Moldervine Cloak did not restore its creature modifier after returning")?;
    let reattached_cloak = event_index_after(events, cloak_modifier_restored, |event| matches!(event, GameEvent::AuraAttached { aura, target } if *aura == cloak && *target == creature)).ok_or("Moldervine Cloak was not reattached to the returned creature")?;
    let attack = event_index_after(events, consumed, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&creature))).ok_or("returned creature did not attack")?;
    let damage_index = event_index_after(events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source: attacker, player: PlayerId(1), amount } if *attacker == creature && *amount >= 6)).ok_or("returned and reattached creature failed to deal 6 combat damage")?;
    let ended = event_index_after(events, damage_index, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal win receipt missing")?;
    if c.exile_incarnations
        .iter()
        .any(|(object, exile_incarnation)| {
            c.return_incarnations
                .iter()
                .find(|(returned_object, _)| returned_object == object)
                .is_none_or(|(_, returned_incarnation)| returned_incarnation <= exile_incarnation)
        })
    {
        return Err("return must create a newer incarnation of every linked card".to_owned());
    }
    if !(paid_spell < cast
        && cast < spell_resolved
        && spell_resolved < attached_source
        && attached_source < cloak_attached
        && cloak_attached < paid_ability
        && paid_ability < activated
        && activated < scheduled
        && scheduled < resolved
        && [returned_creature, returned_source, returned_cloak]
            .iter()
            .all(|index| resolved < *index && *index < consumed)
        && returned_source < reattached_flickerform
        && returned_cloak < cloak_modifier_restored
        && cloak_modifier_restored < reattached_cloak
        && reattached_flickerform < consumed
        && reattached_cloak < consumed
        && consumed < attack
        && attack < damage_index
        && damage_index < ended)
    {
        return Err("Flickerform cast, link, delayed return, attachment, combat, and terminal events are out of order".to_owned());
    }
    let mut indices = vec![
        paid_spell,
        cast,
        spell_resolved,
        attached_source,
        cloak_attached,
        paid_ability,
        activated,
        resolved,
        scheduled,
        consumed,
        returned_creature,
        returned_source,
        returned_cloak,
        reattached_flickerform,
        cloak_modifier_restored,
        reattached_cloak,
        attack,
        damage_index,
        ended,
    ];
    indices.extend(exile_moves.iter().map(|(index, _)| *index));
    indices.sort_unstable();
    Ok(json!({
        "id": "rav-flickerform-linked-exile-return-and-combat-full-game",
        "seed": REFERENCE_SEED,
        "terminal": c.terminal,
        "winner_seat": c.winner,
        "turns": c.turns,
        "accepted_moves": c.accepted,
        "rejected_moves": c.rejected,
        "flickerform_object": source.0,
        "creature_object": creature.0,
        "attached_aura_object": cloak.0,
        "linked_exile_objects": c.exile_incarnations.iter().map(|(object, incarnation)| json!({"object":object.0,"exile_incarnation":incarnation})).collect::<Vec<_>>(),
        "returned_object_incarnations": c.return_incarnations.iter().map(|(object, incarnation)| json!({"object":object.0,"incarnation":incarnation})).collect::<Vec<_>>(),
        "returned_objects": c.returned.iter().map(|object| object.0).collect::<Vec<_>>(),
        "creature_power_after_return": c.creature_power_after_return,
        "linked_return_turns": {"activated": c.activation_turn, "due": c.scheduled_due_turn, "returned": c.return_turn},
        "combat": {"attack_turns": c.attack_turns, "damage_to_opponent": c.damage, "life_at_terminal": c.life},
        "events_digest": event_digest(events),
        "events": project_selected_events(events, &indices),
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
fn project_selected_events(events: &[GameEvent], indices: &[usize]) -> Vec<ProjectedEvent> {
    project_events(events)
        .into_iter()
        .enumerate()
        .filter_map(|(index, event)| indices.contains(&index).then_some(event))
        .collect()
}
fn event_digest(events: &[GameEvent]) -> String {
    let mut hash = 0xcbf29ce484222325_u64;
    for event in events {
        for byte in format!("{event:?}\n").bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    format!("fnv1a64:{hash:016x}")
}

fn expected_definition() -> CardDefinition {
    CardDefinition {
        id: CARD_ID,
        name: "Flickerform",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::White]),
        colors: BTreeSet::from([Color::White]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "aura-enchant-creature",
            "aura-linked-exile-and-next-end-step-return",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::AttachSourceToTarget {
            target: TargetRequirement::Creature,
            changes: vec![],
        }],
    }
}

fn expected_activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: CARD_ID,
        ability: cardbench_magic_engine::ActivatedAbility {
            id: ABILITY_ID,
            mana_cost: ManaCost::with_colors(2, [Color::White, Color::White]),
            tap_cost: false,
            sorcery_speed: false,
            additional_tap_creatures: 0,
            sacrifice_source: false,
            sacrifice_creatures: 0,
            sacrifice_lands: 0,
            discard_cards: 0,
            targets: vec![],
            effects: vec![Effect::ExileAttachedCreatureAndAurasUntilEndStep],
        },
    }
}

fn validate_registry_delegation() -> Result<(), String> {
    let definition = candidate_flickerform::definition();
    let ability = candidate_flickerform::activated_ability();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Flickerform is missing from the RAV catalog")?;
    if definition != expected_definition()
        || definition != registered
        || ability != expected_activated_ability()
        || !rav_activated_ability_bindings().contains(&ability)
    {
        return Err("Flickerform candidate definition or activated binding is not delegated by the RAV registry".to_owned());
    }
    if definition.name != "Flickerform"
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(1, [Color::White])
        || definition.colors != BTreeSet::from([Color::White])
        || definition.card_types != BTreeSet::from([CardType::Enchantment])
        || definition.effects
            != [Effect::AttachSourceToTarget {
                target: TargetRequirement::Creature,
                changes: vec![],
            }]
        || ability.ability.mana_cost != ManaCost::with_colors(2, [Color::White, Color::White])
        || ability.ability.effects != [Effect::ExileAttachedCreatureAndAurasUntilEndStep]
    {
        return Err("Flickerform identity, Aura restriction, cost, or linked-exile effect differs from its contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Flickerform source: {error}"))?;
    let file =
        syn::parse_file(&source).map_err(|_| "Flickerform source is not valid Rust".to_owned())?;
    if !file.attrs.iter().all(is_doc_attribute) {
        return Err("Flickerform source has an unsupported module attribute".to_owned());
    }
    let expected: BTreeSet<Vec<String>> = [
        vec!["cardbench_magic_engine", "ActivatedAbility"],
        vec!["cardbench_magic_engine", "ActivatedAbilityBinding"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TargetRequirement"],
        vec!["std", "collections", "BTreeSet"],
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
                flatten_use(&[], &item.tree, &mut imports)?
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !["definition", "activated_ability"].contains(&name.as_str())
                    || !functions.insert(name)
                    || !matches!(&function.vis, Visibility::Restricted(value) if value.path.is_ident("crate"))
                    || function.sig.constness.is_some()
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || !function.sig.inputs.is_empty()
                    || !function.sig.generics.params.is_empty()
                    || function.sig.generics.where_clause.is_some()
                    || function.sig.variadic.is_some()
                    || function.block.stmts.len() != 1
                    || !function
                        .attrs
                        .iter()
                        .all(|attr| is_doc_attribute(attr) || attr.path().is_ident("must_use"))
                {
                    return Err(
                        "Flickerform module must contain only two direct literal functions"
                            .to_owned(),
                    );
                }
                let Stmt::Expr(Expr::Struct(value), None) = &function.block.stmts[0] else {
                    return Err(
                        "Flickerform functions must return direct struct literals".to_owned()
                    );
                };
                if value.rest.is_some()
                    || !safe_path(&value.path)
                    || !value.fields.iter().all(|field| safe_value(&field.expr))
                {
                    return Err("Flickerform contains a nonliteral candidate expression".to_owned());
                }
            }
            _ => return Err("Flickerform module contains executable top-level code".to_owned()),
        }
    }
    if file.items.len() != 4
        || functions != BTreeSet::from(["definition".to_owned(), "activated_ability".to_owned()])
        || imports != expected
    {
        return Err(
            "Flickerform imports or candidate module structure differ from its source contract"
                .to_owned(),
        );
    }
    Ok(())
}

fn flatten_use(
    prefix: &[String],
    tree: &UseTree,
    out: &mut BTreeSet<Vec<String>>,
) -> Result<(), String> {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix.to_vec();
            next.push(path.ident.to_string());
            flatten_use(&next, &path.tree, out)
        }
        UseTree::Name(name) => {
            let mut next = prefix.to_vec();
            next.push(name.ident.to_string());
            out.insert(next);
            Ok(())
        }
        UseTree::Group(group) => {
            for item in &group.items {
                flatten_use(prefix, item, out)?;
            }
            Ok(())
        }
        _ => Err("Flickerform has an unsupported import form".to_owned()),
    }
}
fn is_doc_attribute(attr: &syn::Attribute) -> bool {
    attr.path().is_ident("doc") && matches!(attr.meta, Meta::NameValue(_))
}
fn safe_path(path: &syn::Path) -> bool {
    path.leading_colon.is_none()
        && path
            .segments
            .iter()
            .all(|segment| matches!(&segment.arguments, syn::PathArguments::None))
}
fn safe_value(expr: &Expr) -> bool {
    match expr {
        Expr::Lit(value) => matches!(&value.lit, Lit::Str(_) | Lit::Int(_) | Lit::Bool(_)),
        Expr::Path(value) => value.qself.is_none() && safe_path(&value.path),
        Expr::Array(value) => value.elems.iter().all(safe_value),
        Expr::Reference(value) => value.mutability.is_none() && safe_value(&value.expr),
        Expr::Struct(value) => {
            value.rest.is_none()
                && safe_path(&value.path)
                && value.fields.iter().all(|field| safe_value(&field.expr))
        }
        Expr::Call(value) => {
            let allowed = match &*value.func {
                Expr::Path(path) => {
                    let names: Vec<_> = path
                        .path
                        .segments
                        .iter()
                        .map(|part| part.ident.to_string())
                        .collect();
                    matches!(names.as_slice(), [a, b] if (a == "BTreeSet" && (b == "new" || b == "from")) || (a == "ManaCost" && b == "with_colors"))
                }
                _ => false,
            };
            allowed && value.args.iter().all(safe_value)
        }
        Expr::Macro(value) => {
            value.mac.path.is_ident("vec")
                && syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated
                    .parse2(value.mac.tokens.clone())
                    .is_ok_and(|items| items.iter().all(safe_value))
        }
        _ => false,
    }
}
fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let cards: Vec<_> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if cards.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
    {
        return Err("RAV catalog identity or 291/306 denominator changed".to_owned());
    }
    Ok(cards)
}
fn card_status(card: &CatalogCard) -> Value {
    let definition = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 18;
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition,"engine_status":if definition.is_some(){"executable_compatibility_slice"}else{"catalog_only"},"trace_status":if passed{"passed"}else{"not_qualified"},"qualification":if passed{"diagnostic_only"}else{"not_evaluated"},"credited":false})
}
fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "candidate"
    } else {
        "reference"
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
