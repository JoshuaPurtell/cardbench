//! Full-game RAV Disembowel authority for policy-submitted X and exact mana value.

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DeckEntry, DeckList, Effect, Game, GameEvent,
    GameView, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target,
    TargetRequirement, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-DISEMBOWEL";
const CARD_NAME: &str = "Disembowel";
const TARGET_ID: &str = "RAV-WAR-TORCH-GOBLIN";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/disembowel.rs";
const REFERENCE_SEED: u64 = 37;
const CHOSEN_X: u8 = 1;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/disembowel.rs"]
mod candidate_disembowel;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Disembowel PvP result: {error}");
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
            eprintln!("Disembowel PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_definition()?;
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
    if !mismatch_cast_is_rejected(Mutation::None)? {
        return Err("Disembowel accepted a creature whose mana value differs from X".to_owned());
    }
    let seed = REFERENCE_SEED;
    let captured = run_match(seed, Mutation::None)?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured != replay {
        return Err("Disembowel full-game replay differed".to_owned());
    }
    let mutations = json!({
        "destruction_instruction_removed_detected": mutation_detected(seed, Mutation::NoDestroy)?,
        "mana_value_at_most_replaces_exact_equality_detected": mutation_detected(seed, Mutation::AtMostX)?,
        "printed_cost_changed_detected": mutation_detected(seed, Mutation::WrongCost)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "destruction_instruction_removed_detected",
        "mana_value_at_most_replaces_exact_equality_detected",
        "printed_cost_changed_detected",
    ] {
        if mutations[key] != true {
            return Err(format!("Disembowel gameplay mutation escaped: {key}"));
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
        "card":{"collector_number":85,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}", ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":["202.3 and 107.3a: mana value and chosen X", "106 and 601.2: mana and casting", "109.5 and 115: target identity and target legality", "117: priority and instant-speed actions", "608.2b-608.3a: target recheck and spell resolution", "701.7: destroy", "704: state-based actions"],
            "source_contract":"CardBench-authored semantic summary for RAV Disembowel #85: a black instant with cost {X}{B} that destroys a target creature only when its mana value equals the chosen X.",
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
        .map_err(|error| format!("could not read Disembowel source: {error}"))?;
    let file =
        syn::parse_file(&source).map_err(|_| "Disembowel source is not valid Rust".to_owned())?;
    if file
        .items
        .iter()
        .filter(|item| matches!(item, syn::Item::Fn(_)))
        .count()
        != 1
        || file
            .items
            .iter()
            .any(|item| !matches!(item, syn::Item::Use(_) | syn::Item::Fn(_)))
    {
        return Err(
            "Disembowel source must contain only imports and its literal definition factory"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_definition() -> Result<(), String> {
    let candidate = candidate_disembowel::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Disembowel is absent from the RAV registry")?;
    if candidate != registered
        || candidate.id != CARD_ID
        || candidate.name != CARD_NAME
        || candidate.set_code != "RAV"
        || candidate.mana_cost != ManaCost::with_colors(0, [Color::Black])
        || candidate.colors != BTreeSet::from([Color::Black])
        || !candidate.mana_colors.is_empty()
        || candidate.card_types != BTreeSet::from([CardType::Instant])
        || candidate.is_basic_land
        || candidate.supported_rules
            != [
                "full-rules-fidelity",
                "chosen-x-targeted-creature-destruction",
                "policy-submitted-chosen-x-mana-spend",
            ]
        || candidate.power.is_some()
        || candidate.toughness.is_some()
        || !candidate.keywords.is_empty()
        || candidate.effects != [Effect::DestroyTargetCreatureWithManaValueEqualToChosenX]
        || candidate.effects[0].target_requirement() != Some(TargetRequirement::Creature)
    {
        return Err(
            "Disembowel candidate or registry differs from its RAV #85 feature contract".to_owned(),
        );
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoDestroy,
    AtMostX,
    WrongCost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    spell: Option<ObjectId>,
    target: Option<ObjectId>,
    target_definition: Option<String>,
    chosen_x: Option<u8>,
    payment: Vec<Color>,
    target_destroyed: bool,
    target_final_zone: Option<Zone>,
    spell_final_zone: Option<Zone>,
    life_totals: [i64; 2],
}

fn definitions_with_mutation(mutation: Mutation) -> Result<Vec<CardDefinition>, String> {
    let mut definitions = card_definitions();
    let candidate = definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Disembowel is absent from the RAV registry")?;
    match mutation {
        Mutation::None => {}
        Mutation::NoDestroy => candidate.effects.clear(),
        Mutation::AtMostX => {
            candidate.effects = vec![Effect::DestroyTargetCreatureWithManaValueAtMostChosenX]
        }
        Mutation::WrongCost => candidate.mana_cost = ManaCost::with_colors(1, [Color::Black]),
    }
    Ok(definitions)
}

fn mismatch_cast_is_rejected(mutation: Mutation) -> Result<bool, String> {
    let mut game =
        Game::new(definitions_with_mutation(mutation)?, 2).map_err(|error| error.to_string())?;
    let spell = game
        .add_card(PlayerId(0), CARD_ID, Zone::Hand)
        .map_err(|error| error.to_string())?;
    let target = game
        .add_card(PlayerId(1), TARGET_ID, Zone::Battlefield)
        .map_err(|error| error.to_string())?;
    game.grant_mana(PlayerId(0), Color::Black, 3)
        .map_err(|error| error.to_string())?;
    let before = game.canonical_event_log();
    let result = game.cast_spell_with_x(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
        2,
        ManaPaymentSelection {
            generic: vec![Color::Black, Color::Black],
            hybrid: vec![],
        },
    );
    let unchanged = game.canonical_event_log() == before
        && game.zone_of(spell) == Some(Zone::Hand)
        && game.zone_of(target) == Some(Zone::Battlefield)
        && game.players[0].mana_pool.amount(Color::Black) == 3
        && game.stack.is_empty();
    game.validate_invariants()
        .map_err(|error| format!("mismatched-X preflight left invalid state: {error}"))?;
    Ok(result.is_err() && unchanged)
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    if mutation == Mutation::AtMostX {
        return Ok(!mismatch_cast_is_rejected(Mutation::AtMostX)?);
    }
    match run_match(seed, mutation) {
        Err(error) => Ok(error.contains("policy move rejected")),
        Ok(captured) => Ok(validate_trace(&captured, seed).is_err()),
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: WATCHWOLF_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-SWAMP".to_owned(),
                count: 20,
            },
            DeckEntry {
                card: "RAV-FOREST".to_owned(),
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
                card: TARGET_ID.to_owned(),
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
        ("Disembowel pilot", &pilot_deck),
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
    let mut game = Game::new_with_all_bindings(
        definitions_with_mutation(mutation)?,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
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
        Box::new(DisembowelPolicy {
            player: PlayerId(0),
        }),
        Box::new(TargetPolicy {
            player: PlayerId(1),
            cast_once: false,
        }),
    ];
    let mut captured = Captured {
        events: vec![],
        terminal: String::new(),
        winner: None,
        turns: 0,
        accepted_moves: 0,
        rejected_moves: 0,
        spell: None,
        target: None,
        target_definition: None,
        chosen_x: None,
        payment: vec![],
        target_destroyed: false,
        target_final_zone: None,
        spell_final_zone: None,
        life_totals: [0, 0],
    };
    let mut attempted_moves = 0u32;
    while !game.is_game_over() {
        if game.turn > 45 || captured.accepted_moves >= 4000 {
            return Err(format!(
                "Disembowel duel exceeded its bound at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("policy seat is missing")?;
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
        let event_cursor = game.event_log.len();
        attempted_moves += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "policy move rejected on turn {} at {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        captured.accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        for event in &game.event_log[event_cursor..] {
            match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    captured.spell = Some(*card);
                    captured.payment = colors.clone();
                    if let Some(stack) = game.stack.iter().find(|item| item.card == *card) {
                        captured.chosen_x = stack.chosen_x;
                    }
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|definition| definition.id == CARD_ID) =>
                {
                    captured.spell = Some(*card);
                    if let Some(stack) = game.stack.iter().find(|item| item.card == *card) {
                        captured.chosen_x = stack.chosen_x;
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
                }
                GameEvent::CardDestroyed { source, card }
                    if Some(*source) == captured.spell && Some(*card) == captured.target =>
                {
                    captured.target_destroyed = true;
                }
                GameEvent::CardMoved { card, to } if Some(*card) == captured.target => {
                    captured.target_final_zone = Some(*to);
                }
                GameEvent::CardMoved { card, to } if Some(*card) == captured.spell => {
                    captured.spell_final_zone = Some(*to);
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
    captured.rejected_moves = attempted_moves.saturating_sub(captured.accepted_moves);
    captured.target_final_zone = captured
        .target_final_zone
        .or_else(|| captured.target.and_then(|target| game.zone_of(target)));
    captured.spell_final_zone = captured
        .spell_final_zone
        .or_else(|| captured.spell.and_then(|spell| game.zone_of(spell)));
    captured.life_totals = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    Ok(captured)
}

struct DisembowelPolicy {
    player: PlayerId,
}

impl CodePolicy for DisembowelPolicy {
    fn id(&self) -> &'static str {
        "rav.disembowel-exact-x-removal.v1"
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
        {
            return PolicyAction::PassPriority;
        }

        if let Some(target) = view
            .opponent_battlefield
            .iter()
            .find(|card| card.definition == Some(TARGET_ID))
            && let Some(spell) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
        {
            if view.mana_pool.amount(Color::Black) >= 2 {
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: spell.id,
                        targets: vec![Target::Permanent(target.id)],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: Some(CHOSEN_X),
                    mana_selection: ManaPaymentSelection {
                        generic: vec![Color::Black],
                        hybrid: vec![],
                    },
                };
            }
            if let Some(swamp) = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some("RAV-SWAMP") && !card.tapped)
            {
                return PolicyAction::ActivateManaAbility {
                    land: swamp.id,
                    color: Color::Black,
                };
            }
        }

        if self.player == PlayerId(0)
            && view.active_player == self.player
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
            && view.stack_depth == 0
        {
            if view.lands_played == 0
                && let Some(land) = preferred_land(view)
            {
                return PolicyAction::PlayLand { card: land.id };
            }
            if let Some(watchwolf) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(WATCHWOLF_ID))
            {
                let cost = ManaCost::with_colors(0, [Color::Green, Color::White]);
                if view.mana_pool.amount(Color::Green) > 0
                    && view.mana_pool.amount(Color::White) > 0
                {
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
                if let Some(action) = make_mana(view, &cost) {
                    return action;
                }
            }
        }
        PolicyAction::PassPriority
    }
}

struct TargetPolicy {
    player: PlayerId,
    cast_once: bool,
}

impl CodePolicy for TargetPolicy {
    fn id(&self) -> &'static str {
        "rav.war-torch-goblin-cast-once.v1"
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
                    .filter(|card| card.definition == Some(TARGET_ID) && card.can_attack)
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
        {
            return PolicyAction::PassPriority;
        }
        if view.active_player == self.player
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
            && view.stack_depth == 0
        {
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
                    .find(|card| card.definition == Some(TARGET_ID))
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
        }
        PolicyAction::PassPriority
    }
}

fn preferred_land(view: &GameView) -> Option<&cardbench_magic_engine::CardView> {
    let swamp_count = view
        .own_battlefield
        .iter()
        .filter(|card| card.definition == Some("RAV-SWAMP"))
        .count();
    let forest_count = view
        .own_battlefield
        .iter()
        .filter(|card| card.definition == Some("RAV-FOREST"))
        .count();
    let plains_count = view
        .own_battlefield
        .iter()
        .filter(|card| card.definition == Some("RAV-PLAINS"))
        .count();
    let preferred = if swamp_count < 2 {
        "RAV-SWAMP"
    } else if forest_count == 0 {
        "RAV-FOREST"
    } else if plains_count == 0 {
        "RAV-PLAINS"
    } else {
        "RAV-SWAMP"
    };
    view.hand
        .iter()
        .find(|card| card.definition == Some(preferred))
        .or_else(|| {
            view.hand.iter().find(|card| {
                matches!(
                    card.definition,
                    Some("RAV-SWAMP" | "RAV-FOREST" | "RAV-PLAINS")
                )
            })
        })
}

fn make_mana(view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
    let preferred =
        if view.mana_pool.amount(Color::Green) == 0 && cost.colored.contains(&Color::Green) {
            Color::Green
        } else if view.mana_pool.amount(Color::White) == 0 && cost.colored.contains(&Color::White) {
            Color::White
        } else {
            return None;
        };
    let land = view.own_battlefield.iter().find(|card| {
        card.card_types.contains(&CardType::Land)
            && !card.tapped
            && card.mana_colors.contains(&preferred)
    })?;
    Some(PolicyAction::ActivateManaAbility {
        land: land.id,
        color: preferred,
    })
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))" || captured.winner != Some(0) {
        return Err("Disembowel duel did not reach a seat-zero terminal win".to_owned());
    }
    let spell = captured.spell.ok_or("Disembowel was not cast")?;
    let target = captured
        .target
        .ok_or("opponent's War-Torch Goblin was not targeted")?;
    if captured.target_definition.as_deref() != Some(TARGET_ID) {
        return Err("Disembowel targeted the wrong creature identity".to_owned());
    }
    if captured.chosen_x != Some(CHOSEN_X) {
        return Err(format!(
            "Disembowel chosen X differs: {:?}",
            captured.chosen_x
        ));
    }
    let target_definition = card_definitions()
        .into_iter()
        .find(|definition| definition.id == TARGET_ID)
        .ok_or("target definition is absent")?;
    let target_mana_value = target_definition.mana_cost.mana_value();
    if target_mana_value != CHOSEN_X {
        return Err(format!(
            "authority target mana value {target_mana_value} differs from chosen X {CHOSEN_X}"
        ));
    }
    if captured.payment != [Color::Black, Color::Black] {
        return Err(format!(
            "Disembowel payment differs from {{X}}{{B}}: {:?}",
            captured.payment
        ));
    }
    if !captured.target_destroyed || captured.target_final_zone != Some(Zone::Graveyard) {
        return Err(format!(
            "Disembowel did not destroy the exact target into its owner's graveyard: spell={:?} target={:?} x={:?} payment={:?} destroyed={} target_zone={:?} events={:?}",
            captured.spell,
            captured.target,
            captured.chosen_x,
            captured.payment,
            captured.target_destroyed,
            captured.target_final_zone,
            captured
                .events
                .iter()
                .filter(|event| matches!(
                    event,
                    GameEvent::SpellManaPaid { .. }
                        | GameEvent::SpellCast { .. }
                        | GameEvent::SpellResolved { .. }
                        | GameEvent::CardDestroyed { .. }
                        | GameEvent::CardMoved { .. }
                        | GameEvent::SpellCounteredByRules { .. }
                ))
                .collect::<Vec<_>>()
        ));
    }
    if captured.spell_final_zone != Some(Zone::Graveyard) {
        return Err("resolved Disembowel did not reach its owner's graveyard".to_owned());
    }
    if captured.rejected_moves != 0 || captured.life_totals[0] <= 0 || captured.life_totals[1] > 0 {
        return Err(format!(
            "Disembowel PvP game did not end cleanly: lives={:?}",
            captured.life_totals
        ));
    }

    let target_payment = event_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::SpellManaPaid { player: PlayerId(1), card, colors }
                if *card == target && colors == &[Color::Red]
        )
    })
    .ok_or("opposing Goblin's {R} payment receipt is missing")?;
    let target_cast = event_index_after(&captured.events, target_payment, |event| {
        matches!(
            event,
            GameEvent::SpellCast { player: PlayerId(1), card } if *card == target
        )
    })
    .ok_or("opposing Goblin cast receipt is missing")?;
    let target_resolved = event_index_after(&captured.events, target_cast, |event| {
        matches!(
            event, GameEvent::SpellResolved { card } if *card == target
        )
    })
    .ok_or("opposing Goblin did not resolve")?;
    let target_entered = event_index_after(&captured.events, target_resolved, |event| {
        matches!(
            event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == target
        )
    })
    .ok_or("opposing Goblin did not enter the battlefield")?;
    let payment = event_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::SpellManaPaid { player: PlayerId(0), card, colors }
                if *card == spell && colors == &[Color::Black, Color::Black]
        )
    })
    .ok_or("Disembowel exact {X}{B} payment receipt is missing")?;
    let cast = event_index_after(&captured.events, payment, |event| {
        matches!(
            event,
            GameEvent::SpellCast { player: PlayerId(0), card } if *card == spell
        )
    })
    .ok_or("Disembowel cast receipt is missing")?;
    let destroyed = event_index_after(&captured.events, cast, |event| {
        matches!(
            event, GameEvent::CardDestroyed { source, card } if *source == spell && *card == target
        )
    })
    .ok_or("Disembowel destruction receipt is missing")?;
    let moved = event_index_after(&captured.events, destroyed, |event| {
        matches!(
            event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == target
        )
    })
    .ok_or("destroyed Goblin did not move to its owner's graveyard")?;
    let resolved = event_index_after(&captured.events, moved, |event| {
        matches!(
            event, GameEvent::SpellResolved { card } if *card == spell
        )
    })
    .ok_or("Disembowel did not resolve")?;
    let terminal = event_index_after(&captured.events, moved, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal seat-zero win receipt is missing")?;
    let order = [
        target_payment,
        target_cast,
        target_resolved,
        target_entered,
        payment,
        cast,
        destroyed,
        moved,
        resolved,
        terminal,
    ];
    if order.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(format!(
            "Disembowel cast/target/resolution order invalid: {order:?}"
        ));
    }
    let selected: BTreeSet<usize> = order.into_iter().collect();
    let events = project_events(&captured.events)
        .into_iter()
        .filter(|event| selected.contains(&(event.sequence as usize)))
        .collect::<Vec<_>>();
    Ok(json!({
        "trace_id":"rav-disembowel-exact-chosen-x-mana-value-pvp",
        "seed":seed,
        "decks":["rav-disembowel-golgari-control-60","rav-war-torch-goblin-opponent-60"],
        "policies":["rav.disembowel-exact-x-removal.v1","rav.war-torch-goblin-cast-once.v1"],
        "terminal":captured.terminal,
        "winner_seat":captured.winner,
        "turns":captured.turns,
        "accepted_moves":captured.accepted_moves,
        "rejected_moves":captured.rejected_moves,
        "mechanics":{
            "mana_cost":"{X}{B}","chosen_x":captured.chosen_x,"target_mana_value":target_mana_value,
            "paid_colors":captured.payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),
            "spell":spell.0,"target":target.0,"target_definition":captured.target_definition,
            "exact_mana_value_match":true,"target_destroyed":captured.target_destroyed,
            "target_final_zone":"Graveyard","spell_final_zone":"Graveyard","controller_won":captured.winner == Some(0)
        },
        "ordered_event_sequence":order,
        "events":events,
        "passed":true
    }))
}

fn event_index(
    events: &[GameEvent],
    mut predicate: impl FnMut(&GameEvent) -> bool,
) -> Option<usize> {
    events.iter().position(|event| predicate(event))
}

fn event_index_after(
    events: &[GameEvent],
    after: usize,
    mut predicate: impl FnMut(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(after + 1)
        .find_map(|(index, event)| predicate(event).then_some(index))
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
    std::fs::write(root.join("reward.txt"), "0.00000000\n")
}
