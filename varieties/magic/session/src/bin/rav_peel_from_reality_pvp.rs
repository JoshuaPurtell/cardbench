//! Full-game RAV Peel from Reality authority: return one controlled and one opponent creature.

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, CombatBlock, DeckEntry, DeckList, Effect, Game,
    GameEvent, GameView, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step,
    Target, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_basic_land_type_bindings,
    rav_main_set_catalog, rav_mana_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-PEEL-FROM-REALITY";
const CARD_NAME: &str = "Peel from Reality";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/peel_from_reality.rs";
const BIRD_ID: &str = "RAV-BIRDS-OF-PARADISE";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const GOBLIN_ID: &str = "RAV-WAR-TORCH-GOBLIN";
const REFERENCE_SEED: u64 = 85;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/peel_from_reality.rs"]
mod candidate_peel_from_reality;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Peel from Reality PvP result: {error}");
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
            eprintln!("Peel from Reality PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_identity()?;
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

    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.winner != replay.winner
        || captured.turns != replay.turns
        || captured.peel != replay.peel
        || captured.targets != replay.targets
        || captured.paid_colors != replay.paid_colors
        || captured.owner_hand_receipts != replay.owner_hand_receipts
    {
        return Err("Peel from Reality shuffled full-game replay differed".to_owned());
    }
    let mutations = json!({
        "controlled_target_scope_mutation_detected": mutation_detected(seed, Mutation::FirstTargetOpponent),
        "opponent_target_scope_mutation_detected": mutation_detected(seed, Mutation::SecondTargetControlled),
        "second_return_instruction_mutation_detected": mutation_detected(seed, Mutation::DropSecondEffect),
        "wrong_exact_cost_mutation_detected": mutation_detected(seed, Mutation::WrongCost),
        "sealed_mutation_authority": false
    });
    for key in [
        "controlled_target_scope_mutation_detected",
        "opponent_target_scope_mutation_detected",
        "second_return_instruction_mutation_detected",
        "wrong_exact_cost_mutation_detected",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Peel from Reality gameplay mutation escaped: {key}"
            ));
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
        "card":{"collector_number":61,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}", ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":["109.5 Objects and owners", "115 Targets", "117 Priority", "601.2 Casting and target announcement", "608.2b Target legality on resolution", "608.3 Resolving instructions in order", "400.3 Zone changes and owner zones"],
            "source_contract":"RAV Peel from Reality #61, {1}{U} blue instant; return a creature controlled by its caster to its owner's hand, then return a creature controlled by an opponent to its owner's hand.",
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
        .map_err(|error| format!("could not read Peel from Reality source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Peel from Reality source is not valid Rust".to_owned())?;
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
            "Peel from Reality source must be a literal factory with imports only".to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_identity() -> Result<(), String> {
    let candidate = candidate_peel_from_reality::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Peel from Reality is absent from the RAV definition registry")?;
    if candidate != registered || !definition_identity_valid(&candidate) {
        return Err(
            "Peel from Reality candidate definition differs from its feature contract".to_owned(),
        );
    }
    Ok(())
}

fn definition_identity_valid(definition: &CardDefinition) -> bool {
    definition.id == CARD_ID
        && definition.name == CARD_NAME
        && definition.set_code == "RAV"
        && definition.mana_cost == ManaCost::with_colors(1, [Color::Blue])
        && definition.colors == BTreeSet::from([Color::Blue])
        && definition.mana_colors.is_empty()
        && definition.card_types == BTreeSet::from([CardType::Instant])
        && !definition.is_basic_land
        && definition.supported_rules
            == [
                "full-rules-fidelity",
                "controlled-creature-target",
                "opponent-creature-target",
                "owner-preserving-bounce",
            ]
        && definition.power.is_none()
        && definition.toughness.is_none()
        && definition.keywords.is_empty()
        && definition.effects
            == [
                Effect::ReturnControlledCreatureToHand,
                Effect::ReturnOpponentCreatureToHand,
            ]
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    FirstTargetOpponent,
    SecondTargetControlled,
    DropSecondEffect,
    WrongCost,
}

#[derive(Clone, Debug)]
struct CapturedMatch {
    events: Vec<cardbench_magic_engine::GameEvent>,
    winner: Option<usize>,
    terminal: String,
    turns: u32,
    accepted_moves: u32,
    peel: Option<ObjectId>,
    targets: Vec<Target>,
    own_target: Option<ObjectId>,
    opponent_target: Option<ObjectId>,
    own_target_definition: Option<String>,
    opponent_target_definition: Option<String>,
    paid_colors: Vec<Color>,
    owner_hand_receipts: Vec<(ObjectId, PlayerId, bool)>,
    target_zones: Vec<(ObjectId, Option<Zone>)>,
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    let seed = if let Ok(value) = std::env::var("CARDBENCH_PEEL_FROM_REALITY_SEED") {
        value.parse::<u64>().map_err(|_| {
            "CARDBENCH_PEEL_FROM_REALITY_SEED must be an unsigned integer".to_owned()
        })?
    } else {
        REFERENCE_SEED
    };
    let captured = run_match(seed, Mutation::None)?;
    validate_trace(&captured, seed)?;
    Ok((seed, captured))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation) {
        Err(error) => {
            error.contains("policy move rejected") || error.contains("target requirements")
        }
        Ok(captured) => validate_trace(&captured, seed).is_err(),
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let pilot_deck = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: BIRD_ID.to_owned(),
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
                count: 12,
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
        ("Peel pilot", &pilot_deck),
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
        .find(|card| card.id == CARD_ID)
        .ok_or("registered Peel from Reality definition missing from catalog")?;
    apply_mutation(candidate, mutation);
    let mut game = Game::new_with_mana_abilities_and_basic_land_types(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
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
        Box::new(PeelPolicy {
            player: PlayerId(0),
            peeled: false,
        }),
        Box::new(GoblinPolicy {
            player: PlayerId(1),
        }),
    ];
    let mut accepted_moves = 0_u32;
    let mut peel = None;
    let mut targets = Vec::new();
    let mut own_target = None;
    let mut opponent_target = None;
    let mut own_target_definition = None;
    let mut opponent_target_definition = None;
    let mut paid_colors = Vec::new();
    let mut owner_hand_receipts = Vec::new();
    let mut target_zones = Vec::new();
    while !game.is_game_over() {
        if game.turn > 50 || accepted_moves >= 4000 {
            return Err(format!(
                "Peel game exceeded its action bound at turn {}",
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
        accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        let new_events = &game.event_log[old_len..];
        for event in new_events {
            match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|def| def.id == CARD_ID) =>
                {
                    paid_colors = colors.clone();
                }
                GameEvent::SpellCast {
                    player: PlayerId(0),
                    card,
                } if game
                    .card_definition(*card)
                    .is_ok_and(|def| def.id == CARD_ID) =>
                {
                    peel = Some(*card);
                    let stack_spell = game
                        .stack
                        .iter()
                        .find(|item| item.card == *card)
                        .ok_or("Peel spell cast receipt had no live stack object")?;
                    targets = stack_spell.targets.clone();
                    if let [Target::Permanent(first), Target::Permanent(second)] =
                        targets.as_slice()
                    {
                        own_target = Some(*first);
                        opponent_target = Some(*second);
                        own_target_definition = Some(
                            game.card_definition(*first)
                                .map_err(|error| error.to_string())?
                                .id
                                .to_owned(),
                        );
                        opponent_target_definition = Some(
                            game.card_definition(*second)
                                .map_err(|error| error.to_string())?
                                .id
                                .to_owned(),
                        );
                    }
                }
                GameEvent::CardMoved {
                    card,
                    to: Zone::Hand,
                } if Some(*card) == own_target || Some(*card) == opponent_target => {
                    let owner = if Some(*card) == own_target {
                        PlayerId(0)
                    } else {
                        PlayerId(1)
                    };
                    let owner_sees = game
                        .view_for_player(owner)
                        .map_err(|error| error.to_string())?
                        .hand
                        .iter()
                        .any(|view| view.id == *card);
                    owner_hand_receipts.push((*card, owner, owner_sees));
                    target_zones.push((*card, game.zone_of(*card)));
                }
                _ => {}
            }
        }
    }
    Ok(CapturedMatch {
        events: game.event_log.clone(),
        terminal: game.winner().map_or_else(
            || "Draw".to_owned(),
            |winner| format!("Winner(PlayerId({}))", winner.0),
        ),
        winner: game.winner().map(|winner| winner.0),
        turns: game.turn,
        accepted_moves,
        peel,
        targets,
        own_target,
        opponent_target,
        own_target_definition,
        opponent_target_definition,
        paid_colors,
        owner_hand_receipts,
        target_zones,
    })
}

fn apply_mutation(definition: &mut CardDefinition, mutation: Mutation) {
    match mutation {
        Mutation::None => {}
        Mutation::FirstTargetOpponent => {
            definition.effects[0] = Effect::ReturnOpponentCreatureToHand
        }
        Mutation::SecondTargetControlled => {
            definition.effects[1] = Effect::ReturnControlledCreatureToHand
        }
        Mutation::DropSecondEffect => {
            definition.effects.truncate(1);
        }
        Mutation::WrongCost => definition.mana_cost = ManaCost::with_colors(0, [Color::Blue]),
    }
}

struct PeelPolicy {
    player: PlayerId,
    peeled: bool,
}
struct GoblinPolicy {
    player: PlayerId,
}

impl CodePolicy for PeelPolicy {
    fn id(&self) -> &'static str {
        "rav.peel-from-reality-return-both-controllers.v1"
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
                    .filter(|card| card.can_attack)
                    .map(|card| card.id)
                    .collect(),
            };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && view.player == self.player
            && !view.blockers_declared
        {
            let mut used = BTreeSet::new();
            let assignments = view
                .combat_attackers
                .iter()
                .filter(|card| card.definition == Some(GOBLIN_ID))
                .filter_map(|attacker| {
                    view.own_battlefield
                        .iter()
                        .find(|blocker| {
                            blocker.can_block && !blocker.tapped && !used.contains(&blocker.id)
                        })
                        .map(|blocker| {
                            used.insert(blocker.id);
                            CombatBlock {
                                attacker: attacker.id,
                                blocker: blocker.id,
                            }
                        })
                })
                .collect();
            return PolicyAction::DeclareBlockers { assignments };
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
        if !self.peeled
            && let Some(spell) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            && let Some(own) = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some(BIRD_ID))
            && let Some(opponent) = view
                .opponent_battlefield
                .iter()
                .find(|card| card.definition == Some(GOBLIN_ID))
        {
            if view.mana_pool.total() >= 2 && view.mana_pool.amount(Color::Blue) >= 1 {
                let generic_color = if view.mana_pool.amount(Color::Green) > 0 {
                    Color::Green
                } else if view.mana_pool.amount(Color::White) > 0 {
                    Color::White
                } else if view.mana_pool.amount(Color::Red) > 0 {
                    Color::Red
                } else if view.mana_pool.amount(Color::Black) > 0 {
                    Color::Black
                } else {
                    Color::Blue
                };
                self.peeled = true;
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: spell.id,
                        targets: vec![Target::Permanent(own.id), Target::Permanent(opponent.id)],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: ManaPaymentSelection {
                        generic: vec![generic_color],
                        hybrid: vec![],
                    },
                };
            }
            if view.mana_pool.amount(Color::Blue) == 0 {
                if let Some(source) = view.own_battlefield.iter().find(|card| {
                    !card.tapped
                        && (card.mana_colors.contains(&Color::Blue)
                            || card.definition == Some(BIRD_ID))
                }) {
                    let color = Color::Blue;
                    return PolicyAction::ActivateManaAbility {
                        land: source.id,
                        color,
                    };
                }
            }
            if view.mana_pool.total() < 2
                && let Some(source) = view
                    .own_battlefield
                    .iter()
                    .find(|card| !card.tapped && card.definition != Some("RAV-ISLAND"))
            {
                let color = if source.definition == Some(BIRD_ID)
                    || source.definition == Some("RAV-FOREST")
                {
                    Color::Green
                } else {
                    Color::White
                };
                return PolicyAction::ActivateManaAbility {
                    land: source.id,
                    color,
                };
            }
        }
        if let Some(bird) = view
            .hand
            .iter()
            .find(|card| card.definition == Some(BIRD_ID))
        {
            if view.mana_pool.amount(Color::Green) >= 1 {
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: bird.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: ManaPaymentSelection {
                        generic: vec![],
                        hybrid: vec![],
                    },
                };
            }
            if let Some(source) = view
                .own_battlefield
                .iter()
                .find(|card| !card.tapped && card.mana_colors.contains(&Color::Green))
            {
                return PolicyAction::ActivateManaAbility {
                    land: source.id,
                    color: Color::Green,
                };
            }
        }
        if let Some(watchwolf) = view
            .hand
            .iter()
            .find(|card| card.definition == Some(WATCHWOLF_ID))
        {
            if view.mana_pool.amount(Color::Green) >= 1 && view.mana_pool.amount(Color::White) >= 1
            {
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: watchwolf.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: ManaPaymentSelection {
                        generic: vec![],
                        hybrid: vec![],
                    },
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
            if let Some(source) = view
                .own_battlefield
                .iter()
                .find(|card| !card.tapped && card.definition == Some(BIRD_ID))
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
        "rav.war-torch-goblin-single-attacker.v1"
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
                    .filter(|card| card.can_attack)
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
        if !view
            .own_battlefield
            .iter()
            .any(|card| card.definition == Some(GOBLIN_ID))
            && let Some(goblin) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(GOBLIN_ID))
        {
            if view.mana_pool.amount(Color::Red) >= 1 {
                return PolicyAction::CastWithPayment {
                    request: CastRequest {
                        card: goblin.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: ManaPaymentSelection {
                        generic: vec![],
                        hybrid: vec![],
                    },
                };
            }
            if let Some(mountain) = view
                .own_battlefield
                .iter()
                .find(|card| !card.tapped && card.mana_colors.contains(&Color::Red))
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
        return Err("Peel full game did not reach a seat-zero win".to_owned());
    }
    let peel = captured.peel.ok_or("Peel from Reality was not cast")?;
    let own = captured
        .own_target
        .ok_or("controlled-creature target missing")?;
    let opponent = captured
        .opponent_target
        .ok_or("opponent-creature target missing")?;
    if own == opponent || captured.targets != [Target::Permanent(own), Target::Permanent(opponent)]
    {
        return Err("Peel target order or identity is wrong".to_owned());
    }
    if captured.own_target_definition.as_deref() != Some(BIRD_ID)
        || captured.opponent_target_definition.as_deref() != Some(GOBLIN_ID)
    {
        return Err("Peel did not target the pilot's Bird and the opponent's Goblin".to_owned());
    }
    if captured.paid_colors.len() != 2
        || captured
            .paid_colors
            .iter()
            .filter(|color| **color == Color::Blue)
            .count()
            < 1
    {
        return Err(format!(
            "Peel did not record its exact {{1}}{{U}} payment: {:?}",
            captured.paid_colors
        ));
    }
    let payment = captured.events.iter().position(|event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == peel && colors.len() == 2 && colors.contains(&Color::Blue))).ok_or("Peel mana payment receipt missing")?;
    let cast = captured.events.iter().position(|event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == peel)).ok_or("Peel cast receipt missing")?;
    let own_return = captured
        .events
        .iter()
        .enumerate()
        .skip(cast + 1)
        .find_map(|(index, event)| {
            matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == own)
                .then_some(index)
        })
        .ok_or("controlled creature did not return to hand")?;
    let opponent_return = captured
        .events
        .iter()
        .enumerate()
        .skip(own_return + 1)
        .find_map(|(index, event)| {
            matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == opponent)
                .then_some(index)
        })
        .ok_or("opponent creature did not return to its owner's hand")?;
    let resolved = captured
        .events
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == peel))
        .ok_or("Peel resolution receipt missing")?;
    let spell_graveyard = captured.events.iter().position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == peel)).ok_or("resolved Peel did not move to its owner's graveyard")?;
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
        .ok_or("terminal win event missing")?;
    if !(payment < cast
        && cast < own_return
        && own_return < opponent_return
        && opponent_return < resolved
        && resolved < spell_graveyard
        && spell_graveyard < terminal)
    {
        return Err(format!(
            "Peel payment, targeting, ordered resolution, and terminal receipts are out of order: payment={payment}, cast={cast}, own_return={own_return}, opponent_return={opponent_return}, resolved={resolved}, spell_graveyard={spell_graveyard}, terminal={terminal}"
        ));
    }
    if captured.owner_hand_receipts.len() < 2
        || !captured
            .owner_hand_receipts
            .iter()
            .any(|(card, owner, visible)| *card == own && *owner == PlayerId(0) && *visible)
        || !captured
            .owner_hand_receipts
            .iter()
            .any(|(card, owner, visible)| *card == opponent && *owner == PlayerId(1) && *visible)
    {
        return Err(
            "bounced cards were not observable in their respective owners' hands".to_owned(),
        );
    }
    if !captured
        .target_zones
        .iter()
        .any(|(card, zone)| *card == own && *zone == Some(Zone::Hand))
        || !captured
            .target_zones
            .iter()
            .any(|(card, zone)| *card == opponent && *zone == Some(Zone::Hand))
    {
        return Err(
            "Peel did not move both targets into their owners' hands during resolution".to_owned(),
        );
    }
    let order = [
        payment,
        cast,
        own_return,
        opponent_return,
        resolved,
        spell_graveyard,
        terminal,
    ];
    let selected: BTreeSet<usize> = order.into_iter().collect();
    let events = project_events(&captured.events)
        .into_iter()
        .filter(|event| selected.contains(&(event.sequence as usize)))
        .collect::<Vec<_>>();
    Ok(json!({
        "trace_id":"rav-peel-from-reality-two-controller-bounce-pvp",
        "seed":seed,
        "decks":["rav-peel-from-reality-pilot-60","rav-war-torch-goblin-opponent-60"],
        "policies":["rav.peel-from-reality-return-both-controllers.v1","rav.war-torch-goblin-single-attacker.v1"],
        "terminal":captured.terminal,
        "winner_seat":captured.winner,
        "turns":captured.turns,
        "accepted_moves":captured.accepted_moves,
        "rejected_moves":0,
        "mechanics":{
            "mana_cost":"{1}{U}","paid_colors":captured.paid_colors.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "spell":peel.0,"controlled_target":own.0,"controlled_target_definition":captured.own_target_definition,
            "opponent_target":opponent.0,"opponent_target_definition":captured.opponent_target_definition,
            "target_order":"controlled_then_opponent","both_targets_returned_in_order":true,
            "controlled_target_owner_hand_seen":true,"opponent_target_owner_hand_seen":true,
            "both_targets_final_zone":"Hand","controller_won":captured.winner == Some(0)
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

fn colors<const N: usize>(values: [Color; N]) -> BTreeSet<Color> {
    values.into_iter().collect()
}
fn types<const N: usize>(values: [CardType; N]) -> BTreeSet<CardType> {
    values.into_iter().collect()
}
const SET_CODE: &str = "RAV";
