//! Full-game RAV Mark of Eviction authority: at its controller's upkeep, return the enchanted creature and all its Auras to their owners' hands.

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DeckEntry, DeckList, Effect, Game, GameEvent,
    GameView, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target,
    TargetRequirement, TriggerCondition, TriggeredAbilityBinding, Zone,
};
use cardbench_magic_policies::CodePolicy;
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
const CARD_ID: &str = "RAV-MARK-OF-EVICTION";
const CARD_NAME: &str = "Mark of Eviction";
const TRIGGER_ID: &str = "upkeep-return-enchanted-creature-and-attached-auras";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/mark_of_eviction.rs";
const GOBLIN_ID: &str = "RAV-WAR-TORCH-GOBLIN";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const REFERENCE_SEED: u64 = 16;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/mark_of_eviction.rs"]
mod candidate_mark_of_eviction;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Mark of Eviction PvP result: {error}");
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
            eprintln!("Mark of Eviction PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_bindings()?;
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<CatalogCard> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || catalog.len() != 291
        || printings.len() != 306
    {
        return Err("RAV catalog denominator differs from the source contract".to_owned());
    }
    let seed = REFERENCE_SEED;
    let captured = run_match(seed, Mutation::None)?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.winner != replay.winner
        || captured.turns != replay.turns
        || captured.mark != replay.mark
        || captured.target != replay.target
        || captured.upkeep_turn != replay.upkeep_turn
        || captured.target_owner_hand_receipt != replay.target_owner_hand_receipt
        || captured.mark_owner_hand_receipt != replay.mark_owner_hand_receipt
    {
        return Err("Mark of Eviction full-game replay differed".to_owned());
    }
    let mutations = json!({
        "aura_attachment_removed_detected": mutation_detected(seed, Mutation::NoAttachment),
        "upkeep_trigger_removed_detected": mutation_detected(seed, Mutation::NoTrigger),
        "wrong_upkeep_controller_detected": mutation_detected(seed, Mutation::OpponentsUpkeep),
        "return_instruction_removed_detected": mutation_detected(seed, Mutation::NoReturnEffect),
        "aura_group_return_removed_detected": mutation_detected(seed, Mutation::ReturnOnlyCreature),
        "wrong_exact_cost_detected": mutation_detected(seed, Mutation::WrongCost),
        "sealed_mutation_authority": false
    });
    for key in [
        "aura_attachment_removed_detected",
        "upkeep_trigger_removed_detected",
        "wrong_upkeep_controller_detected",
        "return_instruction_removed_detected",
        "aura_group_return_removed_detected",
        "wrong_exact_cost_detected",
    ] {
        if mutations[key] != true {
            return Err(format!("Mark of Eviction gameplay mutation escaped: {key}"));
        }
    }
    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,
        "expansion":"RAV",
        "card":{"collector_number":58,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}", ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":["303.4 Auras", "400.3 Zone changes and owners", "603.2-603.3 Triggered abilities", "603.4 Active player and trigger conditions", "117 Priority", "601.2 Casting and target announcement", "608 Resolving triggered abilities", "704 State-based actions"],
            "source_contract":"CardBench-authored semantic summary for RAV Mark of Eviction #58: a blue {U} Aura that attaches to a creature and, at the Aura controller's upkeep, returns that creature and all attached Auras to their respective owners' hands.",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true
        },
        "passed":cards_passed == 1,
        "pilot_card_passed":cards_passed == 1,
        "cards_passed":cards_passed,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":printings.len(),
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":"unreviewed_diagnostic",
        "replay_deterministic":true,
        "mutation_sensitivity":mutations,
        "traces":[trace],
        "per_card_breakdown":breakdown
    }))
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Mark of Eviction source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Mark of Eviction source is not valid Rust".to_owned())?;
    if file
        .items
        .iter()
        .filter(|item| matches!(item, syn::Item::Fn(_)))
        .count()
        != 2
        || file
            .items
            .iter()
            .any(|item| !matches!(item, syn::Item::Use(_) | syn::Item::Fn(_)))
    {
        return Err(
            "Mark of Eviction source must contain only imports and its two literal factories"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_bindings() -> Result<(), String> {
    let definition = candidate_mark_of_eviction::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Mark of Eviction is absent from the RAV registry")?;
    if definition != registered || !definition_identity_valid(&definition) {
        return Err(
            "Mark of Eviction candidate definition differs from its feature contract".to_owned(),
        );
    }
    let candidate_trigger = candidate_mark_of_eviction::triggered_ability();
    let registered_trigger = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Mark of Eviction upkeep trigger is absent from the RAV registry")?;
    if candidate_trigger != registered_trigger || !trigger_identity_valid(&candidate_trigger) {
        return Err(
            "Mark of Eviction candidate trigger differs from its feature contract".to_owned(),
        );
    }
    Ok(())
}

fn definition_identity_valid(definition: &CardDefinition) -> bool {
    definition.id == CARD_ID
        && definition.name == CARD_NAME
        && definition.set_code == "RAV"
        && definition.mana_cost == ManaCost::with_colors(0, [Color::Blue])
        && definition.colors == BTreeSet::from([Color::Blue])
        && definition.mana_colors.is_empty()
        && definition.card_types == BTreeSet::from([CardType::Enchantment])
        && !definition.is_basic_land
        && definition.supported_rules
            == [
                "full-rules-fidelity",
                "aura-enchant-creature-controller-upkeep-return-enchanted-creature-and-attached-auras",
            ]
        && definition.power.is_none()
        && definition.toughness.is_none()
        && definition.keywords.is_empty()
        && definition.effects
            == [Effect::AttachSourceToTarget {
                target: TargetRequirement::Creature,
                changes: vec![],
            }]
}

fn trigger_identity_valid(binding: &TriggeredAbilityBinding) -> bool {
    binding.card_definition == CARD_ID
        && binding.ability.id == TRIGGER_ID
        && binding.ability.condition == TriggerCondition::BeginningOfUpkeep
        && binding.ability.mana_cost == ManaCost::new(0)
        && !binding.ability.optional
        && binding.ability.targets.is_empty()
        && binding.ability.effects == [Effect::ReturnSourceAttachedPermanentAndAurasToOwnersHands]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoAttachment,
    NoTrigger,
    OpponentsUpkeep,
    NoReturnEffect,
    ReturnOnlyCreature,
    WrongCost,
}

#[derive(Clone, Debug)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    mark: Option<ObjectId>,
    target: Option<ObjectId>,
    target_definition: Option<String>,
    paid_colors: Vec<Color>,
    attached: bool,
    trigger_turn: Option<u32>,
    trigger_controller: Option<PlayerId>,
    upkeep_turn: Option<u32>,
    target_returned: bool,
    target_owner_hand_receipt: Option<bool>,
    mark_returned: bool,
    mark_owner_hand_receipt: Option<bool>,
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let pilot_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: WATCHWOLF_ID.to_owned(),
                count: 8,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
                count: 16,
            },
            DeckEntry {
                card: "RAV-ISLAND".to_owned(),
                count: 16,
            },
            DeckEntry {
                card: "RAV-PLAINS".to_owned(),
                count: 16,
            },
        ],
        sideboard: vec![],
    };
    let opponent_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: GOBLIN_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    for (label, deck) in [
        ("Mark pilot", &pilot_deck),
        ("War-Torch Goblin opponent", &opponent_deck),
    ] {
        let count: usize = deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("{label} deck has {count} cards; expected 60"));
        }
    }
    let mut definitions = card_definitions();
    let candidate = definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Mark definition is missing from catalog")?;
    if matches!(mutation, Mutation::NoAttachment) {
        candidate.effects.clear();
    }
    if matches!(mutation, Mutation::WrongCost) {
        candidate.mana_cost = ManaCost::with_colors(1, [Color::Blue]);
    }
    let mut triggers = rav_triggered_ability_bindings();
    triggers.retain(|binding| binding.card_definition != CARD_ID);
    if !matches!(mutation, Mutation::NoTrigger) {
        let mut binding = candidate_mark_of_eviction::triggered_ability();
        if matches!(mutation, Mutation::OpponentsUpkeep) {
            binding.ability.condition = TriggerCondition::BeginningOfOpponentsUpkeep;
        }
        if matches!(mutation, Mutation::NoReturnEffect) {
            binding.ability.effects.clear();
        }
        if matches!(mutation, Mutation::ReturnOnlyCreature) {
            binding.ability.effects = vec![Effect::ReturnSourceAttachedPermanentToHand];
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
    game.load_deck_into_library(PlayerId(0), &pilot_deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent_deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), 7)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), 7)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(MarkPolicy {
            player: PlayerId(0),
        }),
        Box::new(GoblinPolicy {
            player: PlayerId(1),
            cast_once: false,
        }),
    ];
    let mut captured = CapturedMatch {
        events: Vec::new(),
        terminal: String::new(),
        winner: None,
        turns: 0,
        accepted_moves: 0,
        mark: None,
        target: None,
        target_definition: None,
        paid_colors: vec![],
        attached: false,
        trigger_turn: None,
        trigger_controller: None,
        upkeep_turn: None,
        target_returned: false,
        target_owner_hand_receipt: None,
        mark_returned: false,
        mark_owner_hand_receipt: None,
    };
    while !game.is_game_over() {
        if game.turn > 50 || captured.accepted_moves >= 4000 {
            return Err(format!(
                "Mark game exceeded its action bound at turn {}",
                game.turn
            ));
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
        let old_len = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| format!("policy move rejected: {error}"))?;
        captured.accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        for event in &game.event_log[old_len..] {
            match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    captured.paid_colors = colors.clone()
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    captured.mark = Some(*card);
                    let stack = game
                        .stack
                        .iter()
                        .find(|item| item.card == *card)
                        .ok_or("Mark spell did not remain on the stack after cast")?;
                    if let [Target::Permanent(target)] = stack.targets.as_slice() {
                        captured.target = Some(*target);
                        captured.target_definition = Some(
                            game.card_definition(*target)
                                .map_err(|error| error.to_string())?
                                .id
                                .to_owned(),
                        );
                    }
                }
                GameEvent::AttachmentEstablishedWithoutContinuousEffect {
                    attachment,
                    target,
                    kind: cardbench_magic_engine::AttachmentKind::Aura,
                    ..
                } if Some(*attachment) == captured.mark => {
                    captured.attached = Some(*target) == captured.target
                }
                GameEvent::AuraAttached { aura, target } if Some(*aura) == captured.mark => {
                    captured.attached = Some(*target) == captured.target
                }
                GameEvent::StepBegan {
                    turn,
                    active_player: PlayerId(0),
                    step: Step::Upkeep,
                } if captured.trigger_turn.is_none() => captured.upkeep_turn = Some(*turn),
                GameEvent::TriggeredAbilityStacked {
                    controller,
                    source,
                    ability,
                    ..
                } if Some(*source) == captured.mark && *ability == TRIGGER_ID => {
                    captured.trigger_turn = Some(game.turn);
                    captured.trigger_controller = Some(*controller);
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Hand,
                } if Some(*card) == captured.target => {
                    captured.target_returned = true;
                    captured.target_owner_hand_receipt = Some(
                        game.view_for_player(PlayerId(1))
                            .map_err(|error| error.to_string())?
                            .hand
                            .iter()
                            .any(|view| view.id == *card),
                    );
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Hand,
                } if Some(*card) == captured.mark => {
                    captured.mark_returned = true;
                    captured.mark_owner_hand_receipt = Some(
                        game.view_for_player(PlayerId(0))
                            .map_err(|error| error.to_string())?
                            .hand
                            .iter()
                            .any(|view| view.id == *card),
                    );
                }
                _ => {}
            }
        }
    }
    captured.events = game.event_log.clone();
    captured.terminal = game.winner().map_or_else(
        || "Draw".to_owned(),
        |winner| format!("Winner(PlayerId({}))", winner.0),
    );
    captured.winner = game.winner().map(|winner| winner.0);
    captured.turns = game.turn;
    Ok(captured)
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation) {
        Err(error) => error.contains("policy move rejected"),
        Ok(captured) => validate_trace(&captured, seed).is_err(),
    }
}

struct MarkPolicy {
    player: PlayerId,
}
struct GoblinPolicy {
    player: PlayerId,
    cast_once: bool,
}

impl CodePolicy for MarkPolicy {
    fn id(&self) -> &'static str {
        "rav.mark-of-eviction-controller-upkeep-return.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && view.player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(WATCHWOLF_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect(),
            };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && view.player == self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.player != self.player
            || view.decision_player != self.player
            || view.priority != self.player
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.active_player == self.player
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
            && view.lands_played == 0
            && let Some(land) = view.hand.iter().find(|card| {
                matches!(
                    card.definition,
                    Some("RAV-FOREST" | "RAV-ISLAND" | "RAV-PLAINS")
                )
            })
        {
            return PolicyAction::PlayLand { card: land.id };
        }
        if view.active_player != self.player
            || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            return PolicyAction::PassPriority;
        }
        if let (Some(mark), Some(goblin)) = (
            view.hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID)),
            view.opponent_battlefield
                .iter()
                .find(|card| card.definition == Some(GOBLIN_ID)),
        ) {
            if view.mana_pool.amount(Color::Blue) > 0 {
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: mark.id,
                        targets: vec![Target::Permanent(goblin.id)],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: ManaPaymentSelection::default(),
                };
            }
            if let Some(island) = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some("RAV-ISLAND") && !card.tapped)
            {
                return PolicyAction::ActivateManaAbility {
                    land: island.id,
                    color: Color::Blue,
                };
            }
        }
        if let Some(watchwolf) = view
            .hand
            .iter()
            .find(|card| card.definition == Some(WATCHWOLF_ID))
        {
            if view.mana_pool.amount(Color::Green) > 0 && view.mana_pool.amount(Color::White) > 0 {
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: watchwolf.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: ManaPaymentSelection::default(),
                };
            }
            let desired = if view.mana_pool.amount(Color::Green) == 0 {
                Color::Green
            } else {
                Color::White
            };
            if let Some(source) = view
                .own_battlefield
                .iter()
                .find(|card| !card.tapped && card.mana_colors.contains(&desired))
            {
                return PolicyAction::ActivateManaAbility {
                    land: source.id,
                    color: desired,
                };
            }
        }
        PolicyAction::PassPriority
    }
}

impl CodePolicy for GoblinPolicy {
    fn id(&self) -> &'static str {
        "rav.war-torch-goblin-single-cast.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && view.player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(GOBLIN_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect(),
            };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && view.player == self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.player != self.player
            || view.decision_player != self.player
            || view.priority != self.player
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.active_player != self.player
            || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0
            && let Some(land) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-MOUNTAIN"))
        {
            return PolicyAction::PlayLand { card: land.id };
        }
        if !self.cast_once
            && let Some(goblin) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(GOBLIN_ID))
        {
            if view.mana_pool.amount(Color::Red) > 0 {
                self.cast_once = true;
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: goblin.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: ManaPaymentSelection::default(),
                };
            }
            if let Some(mountain) = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some("RAV-MOUNTAIN") && !card.tapped)
            {
                return PolicyAction::ActivateManaAbility {
                    land: mountain.id,
                    color: Color::Red,
                };
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))" || captured.winner != Some(0) {
        return Err("Mark duel did not reach a seat-zero terminal win".to_owned());
    }
    let mark = captured.mark.ok_or("Mark of Eviction was not cast")?;
    let target = captured.target.ok_or("Mark did not retain a target")?;
    if captured.target_definition.as_deref() != Some(GOBLIN_ID) {
        return Err("Mark did not target the opponent's War-Torch Goblin".to_owned());
    }
    if !captured.attached {
        return Err("Mark Aura attachment to the selected Goblin was not recorded".to_owned());
    }
    if captured.paid_colors != [Color::Blue] {
        return Err(format!("Mark payment differs: {:?}", captured.paid_colors));
    }
    if captured.trigger_controller != Some(PlayerId(0)) {
        return Err("Mark upkeep trigger was not controlled by seat zero".to_owned());
    }
    let trigger_turn = captured
        .trigger_turn
        .ok_or("Mark upkeep trigger never entered the stack")?;
    let upkeep_turn = captured
        .upkeep_turn
        .ok_or("P0 upkeep step was not captured")?;
    if trigger_turn != upkeep_turn || trigger_turn <= 3 {
        return Err(format!(
            "Mark trigger did not occur on its controller's later upkeep: trigger_turn={trigger_turn}, latest_p0_upkeep={upkeep_turn}"
        ));
    }
    if !captured.target_returned || captured.target_owner_hand_receipt != Some(true) {
        return Err("enchanted creature did not return to its owner's hand".to_owned());
    }
    if !captured.mark_returned || captured.mark_owner_hand_receipt != Some(true) {
        return Err(
            "Mark did not return to its owner's hand with its enchanted creature".to_owned(),
        );
    }
    let payment = captured.events.iter().position(|event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == mark && colors == &[Color::Blue])).ok_or("Mark {U} payment receipt missing")?;
    let cast = captured.events.iter().position(|event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == mark)).ok_or("Mark cast receipt missing")?;
    let resolved = captured
        .events
        .iter()
        .enumerate()
        .skip(cast + 1)
        .find_map(|(index, event)| {
            matches!(event, GameEvent::SpellResolved { card } if *card == mark).then_some(index)
        })
        .ok_or("Mark spell resolution receipt missing")?;
    let attached = captured.events.iter().enumerate().skip(resolved + 1).find_map(|(index,event)| matches!(event, GameEvent::AttachmentEstablishedWithoutContinuousEffect { attachment, target: attached_target, kind: cardbench_magic_engine::AttachmentKind::Aura, .. } if *attachment == mark && *attached_target == target).then_some(index)).or_else(|| captured.events.iter().enumerate().skip(resolved + 1).find_map(|(index,event)| matches!(event, GameEvent::AuraAttached { aura, target: attached_target } if *aura == mark && *attached_target == target).then_some(index))).ok_or("Aura attachment event missing")?;
    let p1_upkeep = captured
        .events
        .iter()
        .enumerate()
        .skip(attached + 1)
        .find_map(|(index, event)| {
            matches!(
                event,
                GameEvent::StepBegan {
                    active_player: PlayerId(1),
                    step: Step::Upkeep,
                    ..
                }
            )
            .then_some(index)
        })
        .ok_or("opponent upkeep step missing")?;
    let trigger = captured.events.iter().position(|event| matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, ability: TRIGGER_ID, .. } if *source == mark)).ok_or("upkeep trigger-stack receipt missing")?;
    let mark_returned = captured
        .events
        .iter()
        .enumerate()
        .skip(trigger + 1)
        .find_map(|(index, event)| {
            matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == mark)
                .then_some(index)
        })
        .ok_or("trigger did not return Mark to its owner's hand")?;
    let target_returned = captured
        .events
        .iter()
        .enumerate()
        .skip(trigger + 1)
        .find_map(|(index, event)| {
            matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == target)
                .then_some(index)
        })
        .ok_or("trigger did not return the Goblin to its owner's hand")?;
    let ability_resolved = captured.events.iter().enumerate().skip(trigger + 1).find_map(|(index,event)| matches!(event, GameEvent::AbilityResolved { source, ability: TRIGGER_ID, .. } if *source == mark).then_some(index)).ok_or("upkeep ability resolution receipt missing")?;
    let terminal = captured
        .events
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::GameEnded {
                    winner: Some(PlayerId(0))
                }
            )
        })
        .ok_or("terminal seat-zero win receipt missing")?;
    if !(payment < cast
        && cast < resolved
        && resolved < attached
        && p1_upkeep < trigger
        && trigger < mark_returned
        && mark_returned < target_returned
        && target_returned < ability_resolved
        && ability_resolved < terminal)
    {
        return Err(format!(
            "Mark Aura/trigger resolution order is invalid: payment={payment},cast={cast},resolved={resolved},attached={attached},p1_upkeep={p1_upkeep},trigger={trigger},mark_returned={mark_returned},target_returned={target_returned},ability_resolved={ability_resolved},terminal={terminal}"
        ));
    }
    let order = [
        payment,
        cast,
        resolved,
        attached,
        p1_upkeep,
        trigger,
        mark_returned,
        target_returned,
        ability_resolved,
        terminal,
    ];
    let selected: BTreeSet<usize> = order.into_iter().collect();
    let events = project_events(&captured.events)
        .into_iter()
        .filter(|event| selected.contains(&(event.sequence as usize)))
        .collect::<Vec<_>>();
    Ok(json!({
        "trace_id":"rav-mark-of-eviction-attached-creature-upkeep-return-pvp",
        "seed":seed,
        "decks":["rav-mark-of-eviction-pilot-60","rav-war-torch-goblin-opponent-60"],
        "policies":["rav.mark-of-eviction-controller-upkeep-return.v1","rav.war-torch-goblin-single-cast.v1"],
        "terminal":captured.terminal,
        "winner_seat":captured.winner,
        "turns":captured.turns,
        "accepted_moves":captured.accepted_moves,
        "rejected_moves":0,
        "mechanics":{
            "mana_cost":"{U}","paid_colors":captured.paid_colors.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "aura":mark.0,"attached_creature":target.0,"attached_creature_definition":captured.target_definition,
            "attachment_observed":true,"trigger_id":TRIGGER_ID,"trigger_controller":0,"trigger_upkeep_turn":trigger_turn,
            "opponent_upkeep_passed_before_trigger":true,"creature_returned_to_owner_hand":true,"mark_returned_to_its_owner_hand":true,"mark_owner_hand_receipt":captured.mark_owner_hand_receipt,"creature_owner_hand_receipt":captured.target_owner_hand_receipt,"attached_aura_final_zone":"Hand","controller_won":captured.winner == Some(0)
        },
        "ordered_event_sequence":order,
        "events":events,
        "passed":true
    }))
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"engine_status":match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" },"trace_status":if passed { "passed" } else { "not_qualified" },"credited":false,"qualification":if passed { "pilot_trace_passed; independent reviewer approval pending" } else { "not_evaluated"}})
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
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
