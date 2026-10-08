//! Full-game authority for RAV Incite Hysteria's Radiance blocker restriction.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, CombatBlock, DeckEntry, DeckList, Effect, Game,
    GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction,
    Step, Target, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_basic_land_type_bindings,
    rav_main_set_catalog, rav_mana_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use syn::{Item, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-INCITE-HYSTERIA";
const CARD_NAME: &str = "Incite Hysteria";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/incite_hysteria.rs";
const TARGET_RED_ID: &str = "RAV-WAR-TORCH-GOBLIN";
const SHARED_RED_ID: &str = "RAV-VIASHINO-SLASHER";
const EXCLUDED_ID: &str = "RAV-ELVES-OF-DEEP-SHADOW";
const ATTACKER_ID: &str = "RAV-GOLGARI-ROTWURM";
const MAX_TRACE_SEED: u64 = 96;
const MAX_TURNS: u32 = 60;
const MAX_MOVES: u32 = 8_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/incite_hysteria.rs"]
mod candidate_incite_hysteria;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            if let Some(root) = output_root()
                && let Err(error) = write_outputs(&root, &report)
            {
                eprintln!("could not write Incite Hysteria result: {error}");
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
            eprintln!("Incite Hysteria PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    TargetOnly,
    WrongKeyword,
    WrongCost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CreatureFact {
    object: ObjectId,
    definition: Option<&'static str>,
    colors: BTreeSet<Color>,
    can_block: bool,
    cannot_block: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Capture {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    spell: ObjectId,
    spell_turn: u32,
    target: ObjectId,
    attacker: ObjectId,
    mana_paid: Vec<Color>,
    expected_recipients: Vec<ObjectId>,
    recipients: Vec<ObjectId>,
    restricted_red: Vec<CreatureFact>,
    unaffected_nonred: Vec<CreatureFact>,
    probe_blocker: ObjectId,
    probe_error: String,
    probe_preserved_state: bool,
    legal_blocker: Option<ObjectId>,
    legal_block_declared: bool,
    spell_in_graveyard: bool,
    life: [i64; 2],
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_definition()?;
    let (seed, capture) = find_trace()?;
    let trace = validate_trace(&capture, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if capture != replay || project_events(&capture.events) != project_events(&replay.events) {
        return Err("Incite Hysteria shuffled full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "radiance_replaced_with_target_only_mutation_detected": mutation_detected(seed, Mutation::TargetOnly)?,
        "cannot_block_keyword_changed_mutation_detected": mutation_detected(seed, Mutation::WrongKeyword)?,
        "printed_cost_changed_mutation_detected": mutation_detected(seed, Mutation::WrongCost)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "radiance_replaced_with_target_only_mutation_detected",
        "cannot_block_keyword_changed_mutation_detected",
        "printed_cost_changed_mutation_detected",
    ] {
        if mutations[key] != true {
            return Err(format!("Incite Hysteria gameplay mutation escaped: {key}"));
        }
    }

    let printings = rav_main_set_catalog();
    let mut seen = BTreeSet::new();
    let catalog: Vec<CatalogCard> = printings
        .iter()
        .copied()
        .filter(|card| seen.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || catalog.len() != 291
        || printings.len() != 306
    {
        return Err("RAV catalog denominator differs from the shared source contract".to_owned());
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number":132,"printings":1,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate": {"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules":"2026-08-07",
            "checked_sections":["601.2 Casting a spell and paying costs", "608.2 Resolving spells", "611.2 Continuous effects with end-of-turn duration", "613.1f Layer-six keyword effects", "702.50 Radiance", "509.1 Declaring blockers"],
            "source_contract":"RAV Incite Hysteria #132 is a {2}{R} red sorcery; the target creature and each other creature sharing a color with it cannot block until end of turn.",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true
        },
        "passed":true,
        "pilot_card_passed":true,
        "deterministic_replay":true,
        "cards_passed":1,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":printings.len(),
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":"diagnostic_pilot_trace_passed; independent review pending",
        "mutation_sensitivity":mutations,
        "traces":[trace],
        "per_card_breakdown":breakdown
    }))
}

fn candidate_definition(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_incite_hysteria::definition();
    match mutation {
        Mutation::None => {}
        Mutation::TargetOnly => {
            definition.effects = vec![Effect::TargetedBundle {
                target: cardbench_magic_engine::TargetRequirement::Creature,
                effects: vec![Effect::ModifyTargetKeywordUntilEndOfTurn {
                    keyword: Keyword::CannotBlock,
                }],
            }]
        }
        Mutation::WrongKeyword => {
            definition.effects = vec![Effect::RadianceAddKeywordUntilEndOfTurn {
                keyword: Keyword::Haste,
            }]
        }
        Mutation::WrongCost => definition.mana_cost = ManaCost::with_colors(1, [Color::Red]),
    }
    definition
}

fn validate_candidate_definition() -> Result<(), String> {
    let definition = candidate_incite_hysteria::definition();
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(2, [Color::Red])
        || definition.colors != BTreeSet::from([Color::Red])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Sorcery])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
        || definition.effects
            != [Effect::RadianceAddKeywordUntilEndOfTurn {
                keyword: Keyword::CannotBlock,
            }]
    {
        return Err(
            "Incite Hysteria candidate definition differs from the exact RAV #132 source contract"
                .to_owned(),
        );
    }
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Incite Hysteria is missing from the RAV registry")?;
    if registered != definition {
        return Err(
            "Incite Hysteria registry is not delegated to its candidate-owned definition"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Incite Hysteria source: {error}"))?;
    let parsed = syn::parse_file(&source)
        .map_err(|_| "Incite Hysteria source is not valid Rust".to_owned())?;
    if parsed.items.iter().any(|item| match item {
        Item::Use(item) => !matches!(item.vis, Visibility::Inherited) || !item.attrs.is_empty(),
        Item::Fn(function) => {
            function.sig.ident != "definition"
                || !matches!(function.vis, Visibility::Restricted(_))
                || !function.sig.inputs.is_empty()
        }
        _ => true,
    }) || parsed
        .items
        .iter()
        .filter(|item| matches!(item, Item::Fn(_)))
        .count()
        != 1
    {
        return Err(
            "Incite Hysteria candidate module contains unsupported top-level items".to_owned(),
        );
    }
    Ok(())
}

fn find_trace() -> Result<(u64, Capture), String> {
    if let Ok(value) = std::env::var("CARDBENCH_INCITE_HYSTERIA_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_INCITE_HYSTERIA_SEED must be unsigned".to_owned())?;
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
    Err(format!(
        "no qualifying Incite Hysteria shuffled full-game trace in seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Capture, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Incite Hysteria missing from RAV registry")? = candidate_definition(mutation);
    let mut game = Game::new_with_mana_abilities_and_basic_land_types(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
    )
    .map_err(|error| error.to_string())?;
    let deck0 = deck(&[
        (CARD_ID, 4),
        (ATTACKER_ID, 4),
        ("RAV-MOUNTAIN", 17),
        ("RAV-FOREST", 18),
        ("RAV-SWAMP", 17),
    ]);
    let deck1 = deck(&[
        (TARGET_RED_ID, 4),
        (SHARED_RED_ID, 4),
        (EXCLUDED_ID, 4),
        ("RAV-MOUNTAIN", 24),
        ("RAV-FOREST", 24),
    ]);
    if [&deck0, &deck1].iter().any(|deck| {
        deck.mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
    }) {
        return Err(
            "Incite Hysteria fixtures must be shuffled constructed 60-card decks".to_owned(),
        );
    }
    let config = DeckMatchConfig {
        shuffle_seed: seed,
        ..DeckMatchConfig::default()
    };
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

    let mut policy0 = IncitePolicy::new(PlayerId(0));
    let mut policy1 = IncitePolicy::new(PlayerId(1));
    let mut accepted = 0u32;
    let mut probe: Option<(ObjectId, String, bool)> = None;
    let mut post_spell: Option<(
        Vec<ObjectId>,
        Vec<ObjectId>,
        Vec<CreatureFact>,
        Vec<CreatureFact>,
    )> = None;
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || accepted >= MAX_MOVES {
            return Err("Incite Hysteria match exceeded the bounded turn/move budget".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        if player == PlayerId(1)
            && view.step == Step::DeclareBlockers
            && view.active_player == PlayerId(0)
            && !view.blockers_declared
            && policy0.spell.is_some_and(|spell| {
                policy0.spell_turn == view.turn
                && game.event_log.iter().any(
                    |event| matches!(event, GameEvent::SpellResolved { card } if *card == spell),
                )
            })
        {
            let spell = policy0.spell.expect("checked");
            let target = policy0.target.expect("spell records the target");
            let battlefield: Vec<_> = view
                .own_battlefield
                .iter()
                .chain(&view.opponent_battlefield)
                .filter(|card| card.card_types.contains(&CardType::Creature))
                .collect();
            let target_colors = battlefield
                .iter()
                .find(|card| card.id == target)
                .map(|card| card.colors.clone())
                .unwrap_or_default();
            let mut expected_recipients: Vec<_> = battlefield
                .iter()
                .filter(|card| card.id == target || !card.colors.is_disjoint(&target_colors))
                .map(|card| card.id)
                .collect();
            let mut recipients: Vec<_> = game
                .event_log
                .iter()
                .filter_map(|event| match event {
                    GameEvent::ContinuousEffectCreated { source, target, .. }
                        if *source == spell =>
                    {
                        Some(*target)
                    }
                    _ => None,
                })
                .collect();
            expected_recipients.sort();
            recipients.sort();
            let restricted_red = battlefield
                .iter()
                .filter(|card| card.colors.contains(&Color::Red))
                .map(|card| CreatureFact {
                    object: card.id,
                    definition: card.definition,
                    colors: card.colors.clone(),
                    can_block: card.can_block,
                    cannot_block: card.keywords.contains(&Keyword::CannotBlock),
                })
                .collect();
            let unaffected_nonred = battlefield
                .iter()
                .filter(|card| !card.colors.contains(&Color::Red))
                .map(|card| CreatureFact {
                    object: card.id,
                    definition: card.definition,
                    colors: card.colors.clone(),
                    can_block: card.can_block,
                    cannot_block: card.keywords.contains(&Keyword::CannotBlock),
                })
                .collect();
            post_spell = Some((
                expected_recipients,
                recipients,
                restricted_red,
                unaffected_nonred,
            ));

            if probe.is_none() {
                let attacker = view
                    .combat_attackers
                    .iter()
                    .find(|card| card.definition == Some(ATTACKER_ID));
                let target_view = view.own_battlefield.iter().find(|card| card.id == target);
                let shared_view = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.definition == Some(SHARED_RED_ID));
                let unrelated_view = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.definition == Some(EXCLUDED_ID));
                if let (
                    Some(attacker),
                    Some(target_view),
                    Some(shared_view),
                    Some(unrelated_view),
                ) = (attacker, target_view, shared_view, unrelated_view)
                {
                    if target_view.can_block || shared_view.can_block || !unrelated_view.can_block {
                        return Err("live Incite blocker view does not isolate red Radiance recipients from the legal green blocker".to_owned());
                    }
                    let before_events = game.event_log.len();
                    let before_view = view.clone();
                    let attempted = PolicyAction::DeclareBlockers {
                        assignments: vec![CombatBlock {
                            attacker: attacker.id,
                            blocker: target,
                        }],
                    };
                    match game.submit_policy_move(
                        player,
                        "rav.incite-hysteria-block-legality-probe.v1",
                        attempted,
                    ) {
                        Ok(()) => {
                            return Err(
                                "Incite Hysteria allowed its targeted red creature to block"
                                    .to_owned(),
                            );
                        }
                        Err(error) => {
                            let message = error.to_string();
                            if !message.contains("illegal blocker") {
                                return Err(format!(
                                    "unexpected CannotBlock legality-probe rejection: {message}"
                                ));
                            }
                            let after_view = game
                                .view_for_player(player)
                                .map_err(|error| error.to_string())?;
                            let unchanged =
                                game.event_log.len() == before_events && after_view == before_view;
                            if !unchanged {
                                return Err(
                                    "rejected Incite blocker probe changed live game state"
                                        .to_owned(),
                                );
                            }
                            probe = Some((target, message, unchanged));
                        }
                    }
                    continue;
                }
            }
        }
        let preserve_incite_targets = player == PlayerId(1)
            && view.step == Step::DeclareBlockers
            && view.active_player == PlayerId(0)
            && !view.blockers_declared
            && policy0.spell.is_none();
        let policy = if player == PlayerId(0) {
            &mut policy0
        } else {
            &mut policy1
        };
        let action = if preserve_incite_targets {
            // Keep the printed target and shared-red creature live and
            // untapped until the RAV effect can be tested in this game.
            PolicyAction::DeclareBlockers {
                assignments: vec![],
            }
        } else {
            policy_action(policy, &view)
        };
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Incite policy rejected a move at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("Incite post-move invariant failure: {error}"))?;
    }

    let Some(spell) = policy0.spell else {
        let p0 = game
            .view_for_player(PlayerId(0))
            .map_err(|error| error.to_string())?;
        let p1 = game
            .view_for_player(PlayerId(1))
            .map_err(|error| error.to_string())?;
        let casts: Vec<_> = game
            .event_log
            .iter()
            .filter_map(|event| match event {
                GameEvent::SpellCast { player, card } => Some((
                    player.0,
                    game.card_definition(*card)
                        .ok()
                        .map(|definition| definition.id),
                )),
                _ => None,
            })
            .collect();
        let relevant: Vec<_> = game
            .event_log
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    GameEvent::AttackersDeclared { .. }
                        | GameEvent::BlockersDeclared { .. }
                        | GameEvent::CardMoved { .. }
                )
            })
            .cloned()
            .collect();
        return Err(format!(
            "Incite was not cast by terminal turn {} winner {:?}; p0 hand {:?} field {:?} pool {:?}; p1 hand {:?} field {:?}; casts {:?}; combat/moves {:?}",
            game.turn,
            game.winner(),
            p0.hand
                .iter()
                .map(|card| card.definition)
                .collect::<Vec<_>>(),
            p0.own_battlefield
                .iter()
                .map(|card| (card.definition, card.tapped))
                .collect::<Vec<_>>(),
            p0.mana_pool,
            p1.hand
                .iter()
                .map(|card| card.definition)
                .collect::<Vec<_>>(),
            p1.own_battlefield
                .iter()
                .map(|card| (card.definition, card.tapped))
                .collect::<Vec<_>>(),
            casts,
            relevant
        ));
    };
    let target = policy0
        .target
        .ok_or("Incite Hysteria target was not captured")?;
    let attacker = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } => attackers.iter().copied().find(|id| {
                game.card_definition(*id)
                    .is_ok_and(|definition| definition.id == ATTACKER_ID)
            }),
            _ => None,
        })
        .ok_or("Golgari Rotwurm did not attack")?;
    let spell_resolved_at = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == spell))
        .ok_or("Incite Hysteria resolution receipt missing")?;
    let legal_blocker = game
        .event_log
        .iter()
        .enumerate()
        .skip(spell_resolved_at + 1)
        .find_map(|(_, event)| match event {
            GameEvent::BlockersDeclared {
                player: PlayerId(1),
                assignments,
            } => assignments.iter().find_map(|(declared_attacker, blocker)| {
                (*declared_attacker == attacker
                    && game
                        .card_definition(*blocker)
                        .is_ok_and(|definition| definition.id == EXCLUDED_ID))
                .then_some(*blocker)
            }),
            _ => None,
        });
    let legal_block_declared = legal_blocker.is_some();
    let (expected_recipients, recipients, restricted_red, unaffected_nonred) =
        post_spell.ok_or("Incite resolution did not reach the blocker step")?;
    let (probe_blocker, probe_error, probe_preserved_state) =
        probe.ok_or("target CannotBlock legality probe was not reached")?;
    let mana_paid = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if *card == spell => Some(colors.clone()),
            _ => None,
        })
        .unwrap_or_default();
    let p0 = game
        .view_for_player(PlayerId(0))
        .map_err(|error| error.to_string())?;
    let opponent_life = p0
        .opponent_life
        .iter()
        .find(|(seat, _)| *seat == PlayerId(1))
        .map_or(0, |(_, life)| *life);
    let winner = game.winner().map(|seat| seat.0);
    let terminal = winner.map_or_else(
        || "Draw".to_owned(),
        |seat| format!("Winner(PlayerId({seat}))"),
    );
    Ok(Capture {
        events: game.event_log.clone(),
        terminal,
        winner,
        turns: game.turn,
        accepted,
        spell,
        spell_turn: policy0.spell_turn,
        target,
        attacker,
        mana_paid,
        expected_recipients,
        recipients,
        restricted_red,
        unaffected_nonred,
        probe_blocker,
        probe_error,
        probe_preserved_state,
        legal_blocker,
        legal_block_declared,
        spell_in_graveyard: game.zone_of(spell) == Some(Zone::Graveyard),
        life: [p0.own_life, opponent_life],
    })
}

fn policy_action(policy: &mut IncitePolicy, view: &GameView) -> PolicyAction {
    if let Some(action) = policy.propose_pending_decision(view) {
        action
    } else if view.optional_triggered_ability_choice.is_some() {
        policy.propose_optional_triggered_ability(view)
    } else if view.draw_replacement_pending {
        policy.propose_draw_replacement(view)
    } else if view.private_library_choice.is_some() {
        policy.propose_private_library_choice(view)
    } else if view.private_opponent_library_choice.is_some() {
        policy.propose_private_opponent_library_choice(view)
    } else if view.library_search_choice.is_some() {
        policy.propose_library_search_choice(view)
    } else {
        policy.propose_move(view)
    }
}

struct IncitePolicy {
    player: PlayerId,
    spell: Option<ObjectId>,
    target: Option<ObjectId>,
    spell_turn: u32,
}
impl IncitePolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            spell: None,
            target: None,
            spell_turn: 0,
        }
    }
    fn activate_land(view: &GameView, color: Color) -> Option<PolicyAction> {
        let preferred = match color {
            Color::Red => "RAV-MOUNTAIN",
            Color::Green => "RAV-FOREST",
            Color::Black => "RAV-SWAMP",
            _ => return None,
        };
        view.own_battlefield
            .iter()
            .find(|card| card.definition == Some(preferred) && !card.tapped)
            .map(|card| PolicyAction::ActivateManaAbility {
                land: card.id,
                color,
            })
    }
    fn ramp_any(&self, view: &GameView, except: &[Color]) -> Option<PolicyAction> {
        [Color::Green, Color::Black, Color::Red]
            .into_iter()
            .filter(|color| !except.contains(color))
            .find_map(|color| Self::activate_land(view, color))
    }
}
impl CodePolicy for IncitePolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.incite-hysteria-radiance.v1"
        } else {
            "rav.incite-hysteria-opponent.v1"
        }
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            if self.player == PlayerId(1) {
                return PolicyAction::DeclareAttackers { attackers: vec![] };
            }
            if self.spell.is_none() {
                return PolicyAction::DeclareAttackers { attackers: vec![] };
            }
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
            && !view.blockers_declared
        {
            let mut used = BTreeSet::new();
            let assignments = view
                .combat_attackers
                .iter()
                .filter_map(|attacker| {
                    view.own_battlefield
                        .iter()
                        .find(|blocker| blocker.can_block && !used.contains(&blocker.id))
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
        if view.decision_player != self.player
            || view.priority != self.player
            || view.active_player != self.player
            || view.stack_depth != 0
            || view.step != Step::PrecombatMain
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0
            && let Some(land) = view.hand.iter().find(|card| {
                matches!(
                    card.definition,
                    Some("RAV-MOUNTAIN" | "RAV-FOREST" | "RAV-SWAMP")
                )
            })
        {
            return PolicyAction::PlayLand { card: land.id };
        }
        if self.player == PlayerId(0) {
            let has_rotwurm = view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(ATTACKER_ID) && !card.summoning_sick);
            let red_target = view
                .opponent_battlefield
                .iter()
                .find(|card| card.definition == Some(TARGET_RED_ID));
            let has_shared_red = view
                .opponent_battlefield
                .iter()
                .any(|card| card.definition == Some(SHARED_RED_ID));
            let has_green_blocker = view
                .opponent_battlefield
                .iter()
                .any(|card| card.definition == Some(EXCLUDED_ID));
            if has_rotwurm
                && self.spell.is_none()
                && red_target.is_some()
                && has_shared_red
                && has_green_blocker
                && view
                    .hand
                    .iter()
                    .any(|card| card.definition == Some(CARD_ID))
            {
                if view.mana_pool.amount(Color::Red) > 0 && view.mana_pool.total() >= 3 {
                    let spell = view
                        .hand
                        .iter()
                        .find(|card| card.definition == Some(CARD_ID))
                        .expect("checked");
                    let target = red_target.expect("checked");
                    if let Some(mana_selection) =
                        payment_selection(view, &ManaCost::with_colors(2, [Color::Red]))
                        && !mana_selection.generic.contains(&Color::Red)
                    {
                        self.spell = Some(spell.id);
                        self.target = Some(target.id);
                        self.spell_turn = view.turn;
                        return PolicyAction::CastWithPayment {
                            request: CastRequest {
                                card: spell.id,
                                targets: vec![Target::Permanent(target.id)],
                                convoke: vec![],
                                payment_mana_abilities: vec![],
                            },
                            chosen_x: None,
                            mana_selection,
                        };
                    }
                }
                if view.mana_pool.amount(Color::Red) == 0
                    && let Some(action) = Self::activate_land(view, Color::Red)
                {
                    return action;
                }
                if view.mana_pool.total() < 3
                    && let Some(action) = self.ramp_any(view, &[])
                {
                    return action;
                }
            }
            if view
                .hand
                .iter()
                .any(|card| card.definition == Some(ATTACKER_ID))
            {
                if view.mana_pool.amount(Color::Black) > 0
                    && view.mana_pool.amount(Color::Green) > 0
                    && view.mana_pool.total() >= 5
                {
                    let card = view
                        .hand
                        .iter()
                        .find(|card| card.definition == Some(ATTACKER_ID))
                        .expect("checked");
                    return PolicyAction::Cast(CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    });
                }
                if view.mana_pool.amount(Color::Green) == 0
                    && let Some(action) = Self::activate_land(view, Color::Green)
                {
                    return action;
                }
                if view.mana_pool.amount(Color::Black) == 0
                    && let Some(action) = Self::activate_land(view, Color::Black)
                {
                    return action;
                }
                if view.mana_pool.total() < 5
                    && let Some(action) = self.ramp_any(view, &[])
                {
                    return action;
                }
            }
            if let Some(action) = self.ramp_any(view, &[]) {
                return action;
            }
        } else {
            let has_torch = view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(TARGET_RED_ID));
            if !has_torch
                && view
                    .hand
                    .iter()
                    .any(|card| card.definition == Some(TARGET_RED_ID))
            {
                if view.mana_pool.amount(Color::Red) > 0 {
                    let card = view
                        .hand
                        .iter()
                        .find(|card| card.definition == Some(TARGET_RED_ID))
                        .expect("checked");
                    return PolicyAction::Cast(CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    });
                }
                if let Some(action) = Self::activate_land(view, Color::Red) {
                    return action;
                }
            }
            if view
                .hand
                .iter()
                .any(|card| card.definition == Some(SHARED_RED_ID))
            {
                if view.mana_pool.amount(Color::Red) > 0 && view.mana_pool.total() >= 2 {
                    let card = view
                        .hand
                        .iter()
                        .find(|card| card.definition == Some(SHARED_RED_ID))
                        .expect("checked");
                    return PolicyAction::Cast(CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    });
                }
                if view.mana_pool.amount(Color::Red) == 0
                    && let Some(action) = Self::activate_land(view, Color::Red)
                {
                    return action;
                }
                if view.mana_pool.total() < 2
                    && let Some(action) = self.ramp_any(view, &[])
                {
                    return action;
                }
            }
            if view
                .hand
                .iter()
                .any(|card| card.definition == Some(EXCLUDED_ID))
            {
                if view.mana_pool.amount(Color::Green) > 0 {
                    let card = view
                        .hand
                        .iter()
                        .find(|card| card.definition == Some(EXCLUDED_ID))
                        .expect("checked");
                    return PolicyAction::Cast(CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    });
                }
                if let Some(action) = Self::activate_land(view, Color::Green) {
                    return action;
                }
            }
            if let Some(action) = self.ramp_any(view, &[]) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

fn deck(entries: &[(&str, u8)]) -> DeckList {
    DeckList {
        mainboard: entries
            .iter()
            .map(|(card, count)| DeckEntry {
                card: (*card).to_owned(),
                count: *count,
            })
            .collect(),
        sideboard: vec![],
    }
}

fn payment_selection(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available = [0_u8; 6];
    for color in Color::MANA_ALL {
        available[color.index()] = view.mana_pool.amount(color);
    }
    for color in &cost.colored {
        let amount = &mut available[color.index()];
        if *amount == 0 {
            return None;
        }
        *amount -= 1;
    }
    if !cost.hybrid.is_empty() {
        return None;
    }
    let mut generic = Vec::with_capacity(usize::from(cost.generic));
    for _ in 0..cost.generic {
        let color = Color::MANA_ALL
            .into_iter()
            .filter(|color| *color != Color::Red && available[color.index()] > 0)
            .max_by_key(|color| available[color.index()])
            .or_else(|| (available[Color::Red.index()] > 0).then_some(Color::Red))?;
        available[color.index()] -= 1;
        generic.push(color);
    }
    Some(ManaPaymentSelection {
        generic,
        hybrid: vec![],
    })
}

fn validate_trace(capture: &Capture, seed: u64) -> Result<Value, String> {
    if capture.winner != Some(0) || capture.terminal != "Winner(PlayerId(0))" {
        return Err(format!(
            "Incite match ended as {} with life {:?}",
            capture.terminal, capture.life
        ));
    }
    if capture.turns > MAX_TURNS || capture.accepted == 0 {
        return Err("Incite game exceeded the bounded move contract".to_owned());
    }
    if capture.mana_paid.len() != 3
        || capture
            .mana_paid
            .iter()
            .filter(|color| **color == Color::Red)
            .count()
            != 1
    {
        return Err(format!(
            "Incite paid an inexact {{2}}{{R}} cost: {:?}",
            capture.mana_paid
        ));
    }
    if capture.expected_recipients != capture.recipients
        || !capture.expected_recipients.contains(&capture.target)
        || capture.expected_recipients.len() < 2
    {
        return Err(
            "Radiance recipient set did not equal target plus all shared-color creatures"
                .to_owned(),
        );
    }
    if capture
        .restricted_red
        .iter()
        .filter(|fact| {
            fact.definition == Some(TARGET_RED_ID) || fact.definition == Some(SHARED_RED_ID)
        })
        .count()
        < 2
        || capture
            .restricted_red
            .iter()
            .any(|fact| fact.can_block || !fact.cannot_block)
    {
        return Err("red target/shared-color creatures do not carry live CannotBlock".to_owned());
    }
    if !capture
        .unaffected_nonred
        .iter()
        .any(|fact| fact.definition == Some(EXCLUDED_ID) && fact.can_block && !fact.cannot_block)
    {
        return Err("unrelated green creature was not left able to block".to_owned());
    }
    if capture.probe_blocker != capture.target
        || !capture.probe_preserved_state
        || !capture.probe_error.contains("illegal blocker")
    {
        return Err("the rejected red block attempt did not preserve game state".to_owned());
    }
    if capture.legal_blocker.is_none() || !capture.legal_block_declared {
        let blocks: Vec<_> = capture
            .events
            .iter()
            .filter(|event| matches!(event, GameEvent::BlockersDeclared { .. }))
            .cloned()
            .collect();
        return Err(format!(
            "the unrelated green blocker was not subsequently declared legally: attacker {:?}, selected blocker {:?}, declared {}, red {:?}, unaffected {:?}, blocks {:?}",
            capture.attacker,
            capture.legal_blocker,
            capture.legal_block_declared,
            capture.restricted_red,
            capture.unaffected_nonred,
            blocks
        ));
    }
    if !capture.spell_in_graveyard {
        return Err("Incite Hysteria did not move to its owner's graveyard".to_owned());
    }

    let paid = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == capture.spell && colors == &capture.mana_paid)).ok_or("exact Incite spell payment receipt missing")?;
    let cast = unique_index(&capture.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == capture.spell)).ok_or("Incite cast receipt missing")?;
    let effect = unique_index(&capture.events, |event| matches!(event, GameEvent::ContinuousEffectCreated { source, target, .. } if *source == capture.spell && *target == capture.target)).ok_or("Incite target CannotBlock effect receipt missing")?;
    let resolved = unique_index(
        &capture.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == capture.spell),
    )
    .ok_or("Incite resolution receipt missing")?;
    let attack = capture.events.iter().enumerate().skip(resolved + 1).find_map(|(index, event)| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&capture.attacker)).then_some(index)).ok_or("same-turn P0 Rotwurm attack receipt missing")?;
    let legal_block = capture.events.iter().enumerate().skip(attack + 1).find_map(|(index, event)| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if capture.legal_blocker.is_some_and(|blocker| assignments.contains(&(capture.attacker, blocker)))).then_some(index)).ok_or("P1 legal green-blocker receipt missing")?;
    let terminal = unique_index(&capture.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal P0 win receipt missing")?;
    if !(paid < cast
        && cast < effect
        && effect < resolved
        && resolved < attack
        && attack < legal_block
        && legal_block < terminal)
    {
        return Err(
            "Incite payment, cast, Radiance, resolution, combat and terminal event order failed"
                .to_owned(),
        );
    }
    let mut selected = Vec::new();
    for (index, event) in capture.events.iter().enumerate() {
        match event {
            GameEvent::SpellManaPaid { card, .. }
            | GameEvent::SpellCast { card, .. }
            | GameEvent::SpellResolved { card }
                if *card == capture.spell =>
            {
                selected.push(event.clone())
            }
            GameEvent::ContinuousEffectCreated { source, target, .. }
                if *source == capture.spell && capture.recipients.contains(target) =>
            {
                selected.push(event.clone())
            }
            GameEvent::GameEnded {
                winner: Some(PlayerId(0)),
            } => selected.push(event.clone()),
            GameEvent::CardMoved {
                card,
                to: Zone::Graveyard,
            } if *card == capture.spell => selected.push(event.clone()),
            _ if index == attack || index == legal_block => selected.push(event.clone()),
            _ => {}
        }
    }
    let trace = json!({
        "trace_id":"rav-incite-hysteria-radiance-cannot-block-pvp",
        "shuffle_seed":seed,
        "terminal":capture.terminal,
        "turns":capture.turns,
        "accepted_policy_moves":capture.accepted,
        "rejected_policy_moves":0,
        "full_game_event_count":capture.events.len(),
        "mechanics":{
            "cast_from_shuffled_library":true,
            "spell":capture.spell.0,
            "spell_turn":capture.spell_turn,
            "mana_cost":"{2}{R}",
            "mana_paid":capture.mana_paid.iter().map(|c| format!("{c:?}").to_lowercase()).collect::<Vec<_>>(),
            "target":capture.target.0,
            "attacker":capture.attacker.0,
            "radiance_recipients":capture.recipients.iter().map(|id| id.0).collect::<Vec<_>>(),
            "expected_recipients":capture.expected_recipients.iter().map(|id| id.0).collect::<Vec<_>>(),
            "target_and_shared_red_cannot_block":capture.restricted_red.iter().filter(|fact| fact.definition == Some(TARGET_RED_ID) || fact.definition == Some(SHARED_RED_ID)).all(|fact| !fact.can_block && fact.cannot_block),
            "unrelated_green_can_block":capture.unaffected_nonred.iter().any(|fact| fact.definition == Some(EXCLUDED_ID) && fact.can_block && !fact.cannot_block),
            "target_block_legality_probe_rejected":true,
            "probe_error":capture.probe_error,
            "probe_state_unchanged":capture.probe_preserved_state,
            "legal_unrelated_blocker":capture.legal_blocker.map(|id| id.0),
            "legal_block_declared":capture.legal_block_declared,
            "spell_in_graveyard":capture.spell_in_graveyard,
            "life_at_terminal":capture.life
        },
        "events":project_events(&selected)
    });
    Ok(trace)
}

fn unique_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    let mut matches = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Err(_) => Ok(true),
        Ok(capture) => Ok(validate_trace(&capture, seed).is_err()),
    }
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"trace_status":if passed {"passed"} else if definition_id.is_some() {"not_requested"} else {"catalog_only"},"credited":false})
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn output_root() -> Option<std::path::PathBuf> {
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        if argument == "--output-root" {
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
