//! Full-game RAV Surge of Zeal authority for target-bound Radiance haste.

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
use std::collections::BTreeSet;
use std::process::ExitCode;

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-SURGE-OF-ZEAL";
const CARD_NAME: &str = "Surge of Zeal";
const SPELL_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/surge_of_zeal.rs";
const CREATURE_ID: &str = "RAV-GOBLIN-SPELUNKERS";
const OPPONENT_RED_ID: &str = "RAV-WAR-TORCH-GOBLIN";
const COLORLESS_ID: &str = "RAV-GLASS-GOLEM";
const MAX_TRACE_SEED: u64 = 96;

// The candidate module is compiled as a child of this authority binary, so it
// receives the same small set-construction helpers as it does in the RAV crate.
const SET_CODE: &str = "RAV";

fn colors<const N: usize>(values: [Color; N]) -> BTreeSet<Color> {
    values.into_iter().collect()
}

fn types<const N: usize>(values: [CardType; N]) -> BTreeSet<CardType> {
    values.into_iter().collect()
}

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/surge_of_zeal.rs"]
mod candidate_surge_of_zeal;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
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
            eprintln!("Surge of Zeal PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_source()?;
    validate_definition()?;
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
        return Err("RAV catalog denominator differs from the source contract".to_owned());
    }

    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.winner != replay.winner
        || captured.target != replay.target
        || captured.spell != replay.spell
        || captured.recipients != replay.recipients
        || captured.excluded_creatures != replay.excluded_creatures
        || captured.expected_recipients != replay.expected_recipients
        || captured.haste_state != replay.haste_state
    {
        return Err("Surge of Zeal shuffled full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "radiance_shared_color_scope_mutation_detected": mutation_detected(seed, Mutation::TargetOnly),
        "haste_keyword_mutation_detected": mutation_detected(seed, Mutation::CannotBlock),
        "red_cost_mutation_detected": mutation_detected(seed, Mutation::WrongCost),
        "sealed_mutation_authority": false
    });
    for key in [
        "radiance_shared_color_scope_mutation_detected",
        "haste_keyword_mutation_detected",
        "red_cost_mutation_detected",
    ] {
        if mutations[key] != true {
            return Err(format!("Surge of Zeal semantic mutation escaped: {key}"));
        }
    }

    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let passed_count = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,
        "expansion":"RAV",
        "card":{"collector_number":146,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}", ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":[
                "117 Priority and casting instants",
                "202.1-202.2 Color identity and colored mana symbols",
                "601.2 Casting spells, targets, and paying costs",
                "608.2b-608.2c Target legality and resolution",
                "611.2 Continuous effects with end-of-turn duration",
                "613.1f-613.1g Layer six keyword-granting effects",
                "702.50 Radiance shared-color recipients",
                "302.6 Summoning sickness and haste"
            ],
            "source_contract":"RAV Surge of Zeal #146, {R} red instant; target creature and each other creature sharing a color with it gain haste until end of turn.",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true
        },
        "passed":passed_count == 1,
        "pilot_card_passed":passed_count == 1,
        "cards_passed":passed_count,
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

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn validate_definition() -> Result<(), String> {
    let definition = candidate_surge_of_zeal::definition();
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(0, [Color::Red])
        || definition.colors != BTreeSet::from([Color::Red])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Instant])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
        || definition.effects
            != [Effect::RadianceAddKeywordUntilEndOfTurn {
                keyword: Keyword::Haste,
            }]
    {
        return Err("Surge of Zeal typed definition differs from its feature contract".to_owned());
    }
    let registered = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Surge of Zeal is missing from the RAV registry")?;
    if registered != definition {
        return Err("Surge of Zeal registry is not delegated to its candidate module".to_owned());
    }
    Ok(())
}

fn validate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SPELL_PATH)
        .map_err(|error| format!("could not read Surge of Zeal source: {error}"))?;
    let parsed = syn::parse_file(&source)
        .map_err(|_| "Surge of Zeal source is not valid Rust".to_owned())?;
    if parsed
        .items
        .iter()
        .filter(|item| matches!(item, syn::Item::Fn(_)))
        .count()
        != 1
        || parsed
            .items
            .iter()
            .any(|item| !matches!(item, syn::Item::Use(_) | syn::Item::Fn(_)))
    {
        return Err(
            "Surge of Zeal module must contain only private imports and its declarative function"
                .to_owned(),
        );
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    TargetOnly,
    CannotBlock,
    WrongCost,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CreatureFact {
    object: ObjectId,
    definition: Option<&'static str>,
    controller: PlayerId,
    colors: BTreeSet<Color>,
    has_haste: bool,
}

#[derive(Clone, Debug)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    spell_turn: u32,
    life: [i64; 2],
    accepted: u32,
    rejected: u32,
    spell: ObjectId,
    target: ObjectId,
    target_was_summoning_sick: bool,
    target_attacked_this_turn: bool,
    mana_paid: Vec<Color>,
    expected_recipients: Vec<ObjectId>,
    recipients: Vec<ObjectId>,
    excluded_creatures: Vec<CreatureFact>,
    haste_state: Vec<CreatureFact>,
    spell_left_for_graveyard: bool,
}

#[derive(Clone, Debug)]
struct ResolutionSnapshot {
    expected_recipients: Vec<ObjectId>,
    recipients: Vec<ObjectId>,
    excluded_creatures: Vec<CreatureFact>,
    haste_state: Vec<CreatureFact>,
}

fn candidate_value(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_surge_of_zeal::definition();
    match mutation {
        Mutation::None => {}
        Mutation::TargetOnly => {
            definition.effects = vec![Effect::TargetedBundle {
                target: cardbench_magic_engine::TargetRequirement::Creature,
                effects: vec![Effect::ModifyTargetKeywordUntilEndOfTurn {
                    keyword: Keyword::Haste,
                }],
            }]
        }
        Mutation::CannotBlock => {
            definition.effects = vec![Effect::RadianceAddKeywordUntilEndOfTurn {
                keyword: Keyword::CannotBlock,
            }]
        }
        Mutation::WrongCost => definition.mana_cost = ManaCost::with_colors(1, [Color::Red]),
    }
    definition
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_SURGE_OF_ZEAL_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_SURGE_OF_ZEAL_SEED must be unsigned".to_owned())?;
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let Ok(captured) = run_match(seed, Mutation::None)
            && validate_trace(&captured, seed).is_ok()
        {
            return Ok((seed, captured));
        }
    }
    Err(format!(
        "no qualifying shuffled Surge of Zeal full-game trace for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Surge of Zeal missing from RAV registry")? = candidate_value(mutation);
    let mut game = Game::new_with_mana_abilities_and_basic_land_types(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
    )
    .map_err(|error| error.to_string())?;
    let deck0 = deck(&[(CARD_ID, 4), (CREATURE_ID, 4), ("RAV-MOUNTAIN", 52)]);
    let deck1 = deck(&[
        (OPPONENT_RED_ID, 4),
        (COLORLESS_ID, 4),
        ("RAV-MOUNTAIN", 52),
    ]);
    for value in [&deck0, &deck1] {
        if value
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
        {
            return Err(
                "Surge of Zeal fixtures must use complete shuffled 60-card decks".to_owned(),
            );
        }
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

    let mut policy0 = SurgePolicy::new(PlayerId(0));
    let mut policy1 = SurgePolicy::new(PlayerId(1));
    let mut resolution_snapshot: Option<ResolutionSnapshot> = None;
    let mut accepted = 0u32;
    let mut attempted = 0u32;
    while !game.is_game_over() {
        if game.turn > config.max_turns || accepted >= config.max_policy_moves {
            return Err("Surge of Zeal match exceeded bounded game limits".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let action = if player == PlayerId(0) {
            policy_action(&mut policy0, &view)
        } else {
            policy_action(&mut policy1, &view)
        };
        attempted += 1;
        game.submit_policy_move(
            player,
            if player == PlayerId(0) {
                policy0.id()
            } else {
                policy1.id()
            },
            action,
        )
        .map_err(|error| {
            format!(
                "Surge policy move rejected at turn {} {:?}: {error}",
                view.turn, view.step
            )
        })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
        if resolution_snapshot.is_none()
            && policy0.spell.is_some_and(|spell| {
                game.event_log.iter().any(
                    |event| matches!(event, GameEvent::SpellResolved { card } if *card == spell),
                )
            })
        {
            let spell = policy0.spell.expect("checked");
            let target = policy0.target.expect("cast stores its target");
            let board = game
                .view_for_player(PlayerId(0))
                .map_err(|error| error.to_string())?;
            let creatures: Vec<_> = board
                .own_battlefield
                .iter()
                .chain(&board.opponent_battlefield)
                .filter(|card| card.card_types.contains(&CardType::Creature))
                .collect();
            let target_colors = creatures
                .iter()
                .find(|card| card.id == target)
                .map(|card| card.colors.clone())
                .unwrap_or_default();
            let expected_recipients: Vec<_> = creatures
                .iter()
                .filter(|card| card.id == target || !card.colors.is_disjoint(&target_colors))
                .map(|card| card.id)
                .collect();
            let recipients: Vec<_> = game
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
            let excluded_creatures: Vec<_> = creatures
                .iter()
                .filter(|card| card.id != target && card.colors.is_disjoint(&target_colors))
                .map(|card| CreatureFact {
                    object: card.id,
                    definition: card.definition,
                    controller: card.controller,
                    colors: card.colors.clone(),
                    has_haste: card.keywords.contains(&Keyword::Haste),
                })
                .collect();
            let mut haste_state: Vec<_> = creatures
                .iter()
                .filter(|card| expected_recipients.contains(&card.id))
                .map(|card| CreatureFact {
                    object: card.id,
                    definition: card.definition,
                    controller: card.controller,
                    colors: card.colors.clone(),
                    has_haste: card.keywords.contains(&Keyword::Haste),
                })
                .collect();
            haste_state.sort_by_key(|fact| fact.object);
            resolution_snapshot = Some(ResolutionSnapshot {
                expected_recipients,
                recipients,
                excluded_creatures,
                haste_state,
            });
        }
    }

    let spell = policy0
        .spell
        .ok_or("Surge of Zeal was not cast from the shuffled library")?;
    let target = policy0
        .target
        .ok_or("Surge of Zeal target was not captured")?;
    let seat0 = game
        .view_for_player(PlayerId(0))
        .map_err(|error| error.to_string())?;
    let snapshot =
        resolution_snapshot.ok_or("Surge did not produce a captured resolution snapshot")?;
    let target_attackers_step = game.event_log.iter().position(|event| matches!(event,
        GameEvent::StepBegan { turn, step: Step::DeclareAttackers, .. } if *turn == policy0.spell_turn));
    let target_attacked_this_turn = target_attackers_step.is_some_and(|start| game.event_log.iter().skip(start).any(|event| matches!(event,
        GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&target))));
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
    let terminal = game.winner().map_or_else(
        || "Draw".to_owned(),
        |seat| format!("Winner(PlayerId({}))", seat.0),
    );
    let opponent_life = seat0
        .opponent_life
        .iter()
        .find(|(player, _)| *player == PlayerId(1))
        .map_or(0, |(_, life)| *life);
    Ok(Captured {
        events: game.event_log.clone(),
        terminal,
        winner: game.winner().map(|seat| seat.0),
        turns: game.turn,
        spell_turn: policy0.spell_turn,
        life: [seat0.own_life, opponent_life],
        accepted,
        rejected: attempted.saturating_sub(accepted),
        spell,
        target,
        target_was_summoning_sick: policy0.target_was_summoning_sick,
        target_attacked_this_turn,
        mana_paid,
        expected_recipients: snapshot.expected_recipients,
        recipients: snapshot.recipients,
        excluded_creatures: snapshot.excluded_creatures,
        haste_state: snapshot.haste_state,
        spell_left_for_graveyard: game.zone_of(spell) == Some(Zone::Graveyard),
    })
}

fn policy_action(policy: &mut SurgePolicy, view: &GameView) -> PolicyAction {
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

struct SurgePolicy {
    player: PlayerId,
    spell: Option<ObjectId>,
    target: Option<ObjectId>,
    spell_turn: u32,
    target_was_summoning_sick: bool,
}
impl SurgePolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            spell: None,
            target: None,
            spell_turn: 0,
            target_was_summoning_sick: false,
        }
    }
}
impl CodePolicy for SurgePolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.surge-of-zeal-radiance.v1"
        } else {
            "rav.surge-opponent-shell.v1"
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
                .filter(|attacker| !attacker.keywords.contains(&Keyword::Mountainwalk))
                .filter_map(|attacker| {
                    let blocker = view
                        .own_battlefield
                        .iter()
                        .find(|blocker| blocker.can_block && !used.contains(&blocker.id))?;
                    used.insert(blocker.id);
                    Some(CombatBlock {
                        attacker: attacker.id,
                        blocker: blocker.id,
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
            && let Some(land) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-MOUNTAIN"))
        {
            return PolicyAction::PlayLand { card: land.id };
        }
        if self.player == PlayerId(0)
            && self.spell.is_none()
            && view
                .opponent_battlefield
                .iter()
                .any(|card| card.definition == Some(COLORLESS_ID))
        {
            if let (Some(spell), Some(target)) = (
                view.hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID)),
                view.own_battlefield
                    .iter()
                    .find(|card| card.definition == Some(CREATURE_ID) && card.summoning_sick),
            ) {
                if view.mana_pool.amount(Color::Red) > 0 {
                    self.spell = Some(spell.id);
                    self.target = Some(target.id);
                    self.spell_turn = view.turn;
                    self.target_was_summoning_sick = target.summoning_sick;
                    return PolicyAction::CastWithPayment {
                        request: CastRequest {
                            card: spell.id,
                            targets: vec![Target::Permanent(target.id)],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: ManaPaymentSelection::default(),
                    };
                }
                if let Some(land) = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.definition == Some("RAV-MOUNTAIN") && !card.tapped)
                {
                    return PolicyAction::ActivateManaAbility {
                        land: land.id,
                        color: Color::Red,
                    };
                }
            }
        }
        if self.player == PlayerId(0)
            && view
                .hand
                .iter()
                .any(|card| card.definition == Some(CREATURE_ID))
        {
            if view.mana_pool.total() >= 3 {
                let creature = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(CREATURE_ID))
                    .expect("checked");
                return PolicyAction::Cast(CastRequest {
                    card: creature.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                });
            }
            if let Some(land) = view
                .own_battlefield
                .iter()
                .find(|card| card.definition == Some("RAV-MOUNTAIN") && !card.tapped)
            {
                return PolicyAction::ActivateManaAbility {
                    land: land.id,
                    color: Color::Red,
                };
            }
        }
        if self.player == PlayerId(1) {
            if view
                .hand
                .iter()
                .any(|card| card.definition == Some(OPPONENT_RED_ID))
            {
                if view.mana_pool.amount(Color::Red) > 0 {
                    let creature = view
                        .hand
                        .iter()
                        .find(|card| card.definition == Some(OPPONENT_RED_ID))
                        .expect("checked");
                    return PolicyAction::Cast(CastRequest {
                        card: creature.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    });
                }
                if let Some(land) = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.definition == Some("RAV-MOUNTAIN") && !card.tapped)
                {
                    return PolicyAction::ActivateManaAbility {
                        land: land.id,
                        color: Color::Red,
                    };
                }
            }
            if view
                .hand
                .iter()
                .any(|card| card.definition == Some(COLORLESS_ID))
            {
                if view.mana_pool.total() >= 5 {
                    let artifact = view
                        .hand
                        .iter()
                        .find(|card| card.definition == Some(COLORLESS_ID))
                        .expect("checked");
                    return PolicyAction::Cast(CastRequest {
                        card: artifact.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    });
                }
                if let Some(land) = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.definition == Some("RAV-MOUNTAIN") && !card.tapped)
                {
                    return PolicyAction::ActivateManaAbility {
                        land: land.id,
                        color: Color::Red,
                    };
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.winner != Some(0) || captured.terminal != "Winner(PlayerId(0))" {
        return Err(format!(
            "Surge of Zeal game terminal outcome was {} after {} turns, life {:?}",
            captured.terminal, captured.turns, captured.life
        ));
    }
    if captured.rejected != 0 {
        return Err("Surge of Zeal reference match contains rejected policy actions".to_owned());
    }
    if captured.mana_paid != [Color::Red] {
        return Err(format!(
            "Surge of Zeal paid unexpected colors: {:?}",
            captured.mana_paid
        ));
    }
    if !captured.spell_left_for_graveyard {
        return Err("Surge of Zeal did not reach its owner's graveyard".to_owned());
    }
    if !captured.target_was_summoning_sick {
        return Err("trace target was not newly cast this turn".to_owned());
    }
    if !captured.target_attacked_this_turn {
        return Err("haste-granted target did not attack on the turn it entered".to_owned());
    }
    if captured.recipients != captured.expected_recipients || captured.recipients.len() < 2 {
        return Err(format!(
            "Radiance recipient set differed from shared-color battlefield: expected {:?}, got {:?}",
            captured.expected_recipients, captured.recipients
        ));
    }
    if !captured
        .excluded_creatures
        .iter()
        .any(|fact| fact.definition == Some(COLORLESS_ID) && fact.colors.is_empty())
    {
        return Err(
            "a colorless Glass Golem was not present outside the Radiance recipient set".to_owned(),
        );
    }
    if captured
        .excluded_creatures
        .iter()
        .any(|fact| captured.recipients.contains(&fact.object))
    {
        return Err(
            "Radiance granted haste to a creature sharing no color with the target".to_owned(),
        );
    }
    if !captured.haste_state.iter().all(|fact| fact.has_haste) {
        return Err("one or more expected Radiance recipients lacks live haste".to_owned());
    }
    if !captured.recipients.contains(&captured.target) {
        return Err("target creature did not receive haste".to_owned());
    }
    if !captured
        .haste_state
        .iter()
        .any(|fact| fact.controller == PlayerId(1) && fact.colors.contains(&Color::Red))
    {
        return Err("cross-controller red creature did not receive Radiance haste".to_owned());
    }
    let cast = unique_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == captured.spell)).ok_or("Surge cast receipt missing")?;
    let paid = unique_index(&captured.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == captured.spell && colors == &[Color::Red])).ok_or("exact red payment receipt missing")?;
    let effect = unique_index(&captured.events, |event| matches!(event, GameEvent::ContinuousEffectCreated { source, target, .. } if *source == captured.spell && *target == captured.target)).ok_or("target haste effect receipt missing")?;
    let resolved = unique_index(
        &captured.events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == captured.spell),
    )
    .ok_or("Surge resolution receipt missing")?;
    let graveyard = unique_index(&captured.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == captured.spell)).ok_or("Surge spell did not move to graveyard")?;
    let attack_step = first_index(&captured.events, |event| matches!(event,
        GameEvent::StepBegan { turn, step: Step::DeclareAttackers, .. } if *turn == captured.spell_turn
    )).ok_or("same-turn attacker step missing")?;
    let target_attack_index = captured.events.iter().enumerate().skip(attack_step).find_map(|(index, event)| matches!(event,
        GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&captured.target)
    ).then_some(index)).ok_or("target did not attack during Surge's turn")?;
    let attacks = target_attack_index;
    let ended = unique_index(&captured.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal win event missing")?;
    if !(paid < cast
        && cast < effect
        && effect < resolved
        && resolved < graveyard
        && resolved < attacks
        && attacks < ended)
    {
        return Err("Surge cast, payment, Radiance, resolution, same-turn attack, and terminal event ordering failed".to_owned());
    }
    let selected: Vec<GameEvent> = captured
        .events
        .iter()
        .enumerate()
        .filter(|(index, event)| match event {
            GameEvent::SpellManaPaid { card, .. }
            | GameEvent::SpellCast { card, .. }
            | GameEvent::SpellResolved { card } => *card == captured.spell,
            GameEvent::ContinuousEffectCreated { source, target, .. } => {
                *source == captured.spell && captured.recipients.contains(target)
            }
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } => *index == target_attack_index && attackers.contains(&captured.target),
            GameEvent::CardMoved {
                card,
                to: Zone::Graveyard,
            } => *card == captured.spell,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0)),
            } => true,
            _ => false,
        })
        .map(|(_, event)| event.clone())
        .collect();
    Ok(json!({
        "trace_id":"rav-surge-of-zeal-radiance-haste-pvp",
        "shuffle_seed":seed,
        "terminal":captured.terminal,
        "turns":captured.turns,
        "accepted_policy_moves":captured.accepted,
        "rejected_policy_moves":captured.rejected,
        "full_game_event_count":captured.events.len(),
        "mechanics":{
            "cast_from_shuffled_library":true,
            "spell":captured.spell.0,
            "mana_cost":"{R}",
            "mana_paid":["red"],
            "target":captured.target.0,
            "target_was_summoning_sick":captured.target_was_summoning_sick,
            "target_attacked_the_same_turn":captured.target_attacked_this_turn,
            "radiance_recipients":captured.recipients.iter().map(|id| id.0).collect::<Vec<_>>(),
            "expected_recipients":captured.expected_recipients.iter().map(|id| id.0).collect::<Vec<_>>(),
            "excluded_nonmatching_color_creatures":captured.excluded_creatures.iter().map(|fact| fact.object.0).collect::<Vec<_>>(),
            "excluded_colorless_creatures":captured.excluded_creatures.iter().filter(|fact| fact.definition == Some(COLORLESS_ID)).map(|fact| fact.object.0).collect::<Vec<_>>(),
            "cross_controller_recipient":captured.haste_state.iter().find(|fact| fact.controller == PlayerId(1) && fact.colors.contains(&Color::Red)).map(|fact| fact.object.0),
            "recipients_have_live_haste":captured.haste_state.iter().all(|fact| fact.has_haste),
            "spell_in_graveyard":captured.spell_left_for_graveyard
        },
        "events":project_events(&selected)
    }))
}

fn unique_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    let mut indexes = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let first = indexes.next()?;
    indexes.next().is_none().then_some(first)
}
fn first_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation) {
        Err(_) => true,
        Ok(captured) => validate_trace(&captured, seed).is_err(),
    }
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 146;
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"trace_status":if passed {"passed"} else if definition_id.is_some() {"not_requested"} else {"catalog_only"},"credited":false})
}
