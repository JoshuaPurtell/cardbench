//! Full-game diagnostic authority for RAV Woebringer Demon.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DecisionKind, DecisionSelection, DeckEntry, DeckList,
    Effect, Game, GameEvent, GameView, Keyword, ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, Step, TriggerCondition, TriggeredAbilityBinding, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings,
    rav_main_set_catalog, rav_mana_ability_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-WOEBRINGER-DEMON";
const CARD_NAME: &str = "Woebringer Demon";
const GOBLIN_ID: &str = "RAV-WAR-TORCH-GOBLIN";
const TRIGGER_ID: &str = "each-upkeep-active-player-sacrifice-creature";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/woebringer_demon.rs";
const REFERENCE_SEED: u64 = 16;
const MAX_TURNS: u32 = 40;
const MAX_MOVES: u32 = 4_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/woebringer_demon.rs"]
mod candidate_woebringer_demon;

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
            eprintln!("Woebringer Demon PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_semantics()?;
    let catalog = unique_catalog()?;
    let (seed, reference) = find_reference()?;
    let trace = validate_trace(&reference, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if reference != replay || project_events(&reference.events) != project_events(&replay.events) {
        return Err("Woebringer Demon full-game replay differs".to_owned());
    }
    let mutations = json!({
        "printed_mana_cost_changed_detected": mutation_detected(seed, Mutation::WrongCost)?,
        "base_power_or_toughness_changed_detected": mutation_detected(seed, Mutation::WrongStats)?,
        "flying_removed_detected": mutation_detected(seed, Mutation::NoFlying)?,
        "each_upkeep_sacrifice_trigger_removed_detected": mutation_detected(seed, Mutation::NoTrigger)?,
        "sealed_mutation_authority": false
    });
    for key in [
        "printed_mana_cost_changed_detected",
        "base_power_or_toughness_changed_detected",
        "flying_removed_detected",
        "each_upkeep_sacrifice_trigger_removed_detected",
    ] {
        if mutations[key] != true {
            return Err(format!("Woebringer Demon semantic mutation escaped: {key}"));
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
        "card":{"collector_number":113,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":["302.1 and 601.2: creature characteristics and casting", "117: upkeep priority", "503.1 and 603.2-603.3: beginning-of-step triggers", "608: triggered ability resolution", "701.17: sacrifice", "704: state-based actions"],
            "source_contract":"CardBench-authored semantic summary for RAV Woebringer Demon #113: a black 4/4 creature with flying costing {3}{B}{B}; at the beginning of each upkeep, that upkeep's active player sacrifices a creature if able.",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true
        },
        "passed":cards_passed == 1,
        "pilot_card_passed":cards_passed == 1,
        "cards_passed":cards_passed,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
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

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let unique: Vec<_> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if unique.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || !unique
            .iter()
            .any(|card| card.collector_number == 113 && card.name == CARD_NAME)
    {
        return Err("RAV catalog identity or denominator differs for Woebringer Demon".to_owned());
    }
    Ok(unique)
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Woebringer Demon source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Woebringer Demon source is not valid Rust".to_owned())?;
    if !file.attrs.iter().all(|attribute| attribute.path().is_ident("doc")) {
        return Err("Woebringer Demon source has unsupported module attributes".to_owned());
    }
    let functions: Vec<_> = file
        .items
        .iter()
        .filter_map(|item| match item {
            syn::Item::Fn(function) => Some(function.sig.ident.to_string()),
            syn::Item::Use(_) => None,
            _ => Some("<unexpected item>".to_owned()),
        })
        .collect();
    if functions != ["definition", "triggered_ability"] {
        return Err("Woebringer Demon source must contain only its definition and trigger factories".to_owned());
    }
    Ok(())
}

fn validate_candidate_semantics() -> Result<(), String> {
    let candidate = candidate_woebringer_demon::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Woebringer Demon is absent from the RAV registry")?;
    let candidate_trigger = candidate_woebringer_demon::triggered_ability();
    let registered_trigger = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Woebringer Demon upkeep trigger is absent from the RAV registry")?;
    if candidate != registered
        || candidate.id != CARD_ID
        || candidate.name != CARD_NAME
        || candidate.set_code != "RAV"
        || candidate.mana_cost != ManaCost::with_colors(3, [Color::Black, Color::Black])
        || candidate.colors != BTreeSet::from([Color::Black])
        || !candidate.mana_colors.is_empty()
        || candidate.card_types != BTreeSet::from([CardType::Creature])
        || candidate.is_basic_land
        || candidate.power != Some(4)
        || candidate.toughness != Some(4)
        || candidate.keywords != [Keyword::Flying]
        || !candidate.effects.is_empty()
        || candidate.supported_rules
            != [
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "flying",
                "each-upkeep-active-player-sacrifice-creature",
            ]
        || candidate_trigger != registered_trigger
        || candidate_trigger.card_definition != CARD_ID
        || candidate_trigger.ability.id != TRIGGER_ID
        || candidate_trigger.ability.condition != TriggerCondition::BeginningOfAnyUpkeep
        || candidate_trigger.ability.mana_cost != ManaCost::new(0)
        || candidate_trigger.ability.optional
        || !candidate_trigger.ability.targets.is_empty()
        || candidate_trigger.ability.effects != [Effect::SacrificeUpkeepPlayerCreature]
    {
        return Err("Woebringer Demon candidate or registry differs from RAV #113 contract".to_owned());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongCost,
    WrongStats,
    NoFlying,
    NoTrigger,
}

fn definitions_with_mutation(mutation: Mutation) -> Result<Vec<CardDefinition>, String> {
    let mut definitions = card_definitions();
    let candidate = definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Woebringer Demon is absent from the RAV registry")?;
    match mutation {
        Mutation::None | Mutation::NoTrigger => {}
        Mutation::WrongCost => {
            candidate.mana_cost = ManaCost::with_colors(4, [Color::Black, Color::Black]);
        }
        Mutation::WrongStats => {
            candidate.power = Some(3);
            candidate.toughness = Some(4);
        }
        Mutation::NoFlying => candidate.keywords.clear(),
    }
    Ok(definitions)
}

fn triggers_with_mutation(mutation: Mutation) -> Vec<TriggeredAbilityBinding> {
    let mut bindings = rav_triggered_ability_bindings();
    if mutation == Mutation::NoTrigger {
        bindings.retain(|binding| binding.card_definition != CARD_ID);
    }
    bindings
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    terminal: String,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    demon: Option<ObjectId>,
    payment: Vec<Color>,
    final_zone: Option<Zone>,
    power: Option<i32>,
    toughness: Option<i32>,
    keywords: Vec<Keyword>,
    life_totals: [i64; 2],
    sacrificed_definitions: Vec<(PlayerId, ObjectId, &'static str)>,
}

fn find_reference() -> Result<(u64, Captured), String> {
    let captured = run_match(REFERENCE_SEED, Mutation::None)?;
    validate_trace(&captured, REFERENCE_SEED)?;
    Ok((REFERENCE_SEED, captured))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Err(_) => Ok(true),
        Ok(captured) => Ok(validate_trace(&captured, seed).is_err()),
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot_deck = DeckList {
        mainboard: vec![
            DeckEntry { card: CARD_ID.to_owned(), count: 4 },
            DeckEntry { card: GOBLIN_ID.to_owned(), count: 8 },
            DeckEntry { card: "RAV-SWAMP".to_owned(), count: 24 },
            DeckEntry { card: "RAV-MOUNTAIN".to_owned(), count: 24 },
        ],
        sideboard: vec![],
    };
    let opponent_deck = DeckList {
        mainboard: vec![
            DeckEntry { card: GOBLIN_ID.to_owned(), count: 4 },
            DeckEntry { card: "RAV-MOUNTAIN".to_owned(), count: 56 },
        ],
        sideboard: vec![],
    };
    for (label, deck) in [("Woebringer pilot", &pilot_deck), ("Goblin opponent", &opponent_deck)] {
        let count: usize = deck.mainboard.iter().map(|entry| usize::from(entry.count)).sum();
        if count != 60 {
            return Err(format!("{label} deck has {count} cards; expected 60"));
        }
    }
    let mut game = Game::new_with_all_bindings_and_triggers(
        definitions_with_mutation(mutation)?,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers_with_mutation(mutation),
    )
    .map_err(|error| error.to_string())?;
    game.set_shuffle_seed(seed).map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot_deck).map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent_deck).map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), 7).map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), 7).map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(WoebringerPolicy { player: PlayerId(0), cast_demon: false, goblins_cast: 0 }),
        Box::new(GoblinPolicy { player: PlayerId(1), goblins_cast: 0 }),
    ];
    let mut captured = Captured {
        events: vec![], terminal: String::new(), winner: None, turns: 0,
        accepted_moves: 0, rejected_moves: 0, demon: None, payment: vec![],
        final_zone: None, power: None, toughness: None, keywords: vec![],
        life_totals: [0, 0], sacrificed_definitions: vec![],
    };
    let mut attempts = 0u32;
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || captured.accepted_moves >= MAX_MOVES {
            return Err(format!("Woebringer duel exceeded bound at turn {}", game.turn));
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("policy seat missing")?;
        let action = if view.pending_decision.is_some() {
            policy.propose_pending_decision(&view).ok_or("required typed decision has no response")?
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
        let cursor = game.event_log.len();
        attempts += 1;
        game.submit_policy_move(player, policy.id(), action).map_err(|error| {
            format!("policy move rejected at turn {} {:?}: {error}", view.turn, view.step)
        })?;
        captured.accepted_moves += 1;
        game.validate_invariants().map_err(|error| format!("post-move invariant failed: {error}"))?;
        for event in &game.event_log[cursor..] {
            if let GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } = event
                && game.card_definition(*card).is_ok_and(|def| def.id == CARD_ID)
            {
                captured.demon = Some(*card);
                captured.payment = colors.clone();
            }
            if let GameEvent::SpellCast { player: PlayerId(0), card } = event
                && game.card_definition(*card).is_ok_and(|def| def.id == CARD_ID)
            {
                captured.demon = Some(*card);
            }
            if let GameEvent::SacrificedByEffect { source, player, permanent } = event
                && captured.demon == Some(*source)
            {
                let definition = game.card_definition(*permanent).map_err(|error| error.to_string())?.id;
                captured.sacrificed_definitions.push((*player, *permanent, definition));
            }
        }
    }
    captured.events = game.event_log.clone();
    captured.terminal = game.winner().map_or_else(|| "Draw".to_owned(), |winner| format!("Winner(PlayerId({}))", winner.0));
    captured.winner = game.winner().map(|winner| winner.0);
    captured.turns = game.turn;
    captured.rejected_moves = attempts.saturating_sub(captured.accepted_moves);
    captured.final_zone = captured.demon.and_then(|card| game.zone_of(card));
    if let Some(demon) = captured.demon {
        let characteristics = game.characteristics(demon).map_err(|error| error.to_string())?;
        captured.power = characteristics.power;
        captured.toughness = characteristics.toughness;
        captured.keywords = characteristics.keywords;
    }
    captured.life_totals = [
        game.player(PlayerId(0)).map_err(|error| error.to_string())?.life,
        game.player(PlayerId(1)).map_err(|error| error.to_string())?.life,
    ];
    Ok(captured)
}

struct WoebringerPolicy {
    player: PlayerId,
    cast_demon: bool,
    goblins_cast: u8,
}

struct GoblinPolicy {
    player: PlayerId,
    goblins_cast: u8,
}

impl WoebringerPolicy {
    fn sacrifice_decision(&self, view: &GameView) -> Option<PolicyAction> {
        let decision = view.pending_decision.as_ref()?;
        if decision.kind != DecisionKind::TriggeredEffectObject {
            return None;
        }
        let selection = decision.candidates.iter()
            .find(|card| card.definition != Some(CARD_ID))
            .or_else(|| decision.candidates.first())?;
        Some(PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::Objects(vec![selection.id]),
        })
    }
}

impl GoblinPolicy {
    fn sacrifice_decision(&self, view: &GameView) -> Option<PolicyAction> {
        let decision = view.pending_decision.as_ref()?;
        if decision.kind != DecisionKind::TriggeredEffectObject { return None; }
        let selection = decision.candidates.first()?;
        Some(PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::Objects(vec![selection.id]),
        })
    }
}

impl CodePolicy for WoebringerPolicy {
    fn id(&self) -> &'static str { "rav.woebringer-demon-each-upkeep-pilot.v1" }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers && view.active_player == self.player && view.player == self.player && !view.attackers_declared {
            return PolicyAction::DeclareAttackers { attackers: view.own_battlefield.iter()
                .filter(|card| card.definition == Some(CARD_ID) && card.can_attack)
                .map(|card| card.id).collect() };
        }
        if view.step == Step::DeclareBlockers && view.active_player != self.player && view.player == self.player && !view.blockers_declared {
            return PolicyAction::DeclareBlockers { assignments: vec![] };
        }
        if view.player != self.player || view.decision_player != self.player || view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if view.active_player == self.player && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain) && view.stack_depth == 0 {
            if view.lands_played == 0 && let Some(land) = choose_land(view, self.player == PlayerId(1)) {
                return PolicyAction::PlayLand { card: land.id };
            }
            if !self.cast_demon && let Some(demon) = view.hand.iter().find(|card| card.definition == Some(CARD_ID)) {
                if let Some(generic) = demon_generic_payment(view) {
                    self.cast_demon = true;
                    return cast_action(demon.id, generic);
                }
                if let Some(color) = color_to_make_demon_castable(view) {
                    if let Some(land) = untapped_land(view, color) {
                        return PolicyAction::ActivateManaAbility { land: land.id, color };
                    }
                }
            }
            if self.goblins_cast < 8 && let Some(goblin) = view.hand.iter().find(|card| card.definition == Some(GOBLIN_ID)) {
                if view.mana_pool.amount(Color::Red) > 0 {
                    self.goblins_cast += 1;
                    return cast_action(goblin.id, vec![]);
                }
                if let Some(mountain) = untapped_land(view, Color::Red) {
                    return PolicyAction::ActivateManaAbility { land: mountain.id, color: Color::Red };
                }
            }
        }
        PolicyAction::PassPriority
    }

    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        self.sacrifice_decision(view)
    }
}

impl CodePolicy for GoblinPolicy {
    fn id(&self) -> &'static str { "rav.war-torch-goblin-upkeep-opponent.v1" }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers && view.active_player == self.player && view.player == self.player && !view.attackers_declared {
            return PolicyAction::DeclareAttackers { attackers: vec![] };
        }
        if view.step == Step::DeclareBlockers && view.active_player != self.player && view.player == self.player && !view.blockers_declared {
            let attacker = view.combat_attackers.iter().find(|card| card.definition == Some(CARD_ID));
            let blocker = view.own_battlefield.iter().find(|card| card.definition == Some(GOBLIN_ID) && card.can_block);
            if let (Some(attacker), Some(blocker)) = (attacker, blocker) && !attacker.keywords.contains(&Keyword::Flying) {
                return PolicyAction::DeclareBlockers { assignments: vec![cardbench_magic_engine::CombatBlock { attacker: attacker.id, blocker: blocker.id }] };
            }
            return PolicyAction::DeclareBlockers { assignments: vec![] };
        }
        if view.player != self.player || view.decision_player != self.player || view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if view.active_player == self.player && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain) && view.stack_depth == 0 {
            if view.lands_played == 0 && let Some(land) = choose_land(view, true) {
                return PolicyAction::PlayLand { card: land.id };
            }
            if self.goblins_cast < 4 && let Some(goblin) = view.hand.iter().find(|card| card.definition == Some(GOBLIN_ID)) {
                if view.mana_pool.amount(Color::Red) > 0 {
                    self.goblins_cast += 1;
                    return cast_action(goblin.id, vec![]);
                }
                if let Some(mountain) = untapped_land(view, Color::Red) {
                    return PolicyAction::ActivateManaAbility { land: mountain.id, color: Color::Red };
                }
            }
        }
        PolicyAction::PassPriority
    }

    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        self.sacrifice_decision(view)
    }
}

fn cast_action(card: ObjectId, generic: Vec<Color>) -> PolicyAction {
    PolicyAction::CastWithPayment {
        request: cardbench_magic_engine::CastRequest { card, targets: vec![], convoke: vec![], payment_mana_abilities: vec![] },
        chosen_x: None,
        mana_selection: ManaPaymentSelection { generic, hybrid: vec![] },
    }
}

fn choose_land(view: &GameView, only_mountain: bool) -> Option<&cardbench_magic_engine::CardView> {
    if only_mountain {
        return view.hand.iter().find(|card| card.definition == Some("RAV-MOUNTAIN"));
    }
    let swamps = view.own_battlefield.iter().filter(|card| card.definition == Some("RAV-SWAMP")).count();
    let mountains = view.own_battlefield.iter().filter(|card| card.definition == Some("RAV-MOUNTAIN")).count();
    let preferred = if swamps <= mountains { "RAV-SWAMP" } else { "RAV-MOUNTAIN" };
    view.hand.iter().find(|card| card.definition == Some(preferred))
        .or_else(|| view.hand.iter().find(|card| matches!(card.definition, Some("RAV-SWAMP" | "RAV-MOUNTAIN"))) )
}

fn untapped_land(view: &GameView, color: Color) -> Option<&cardbench_magic_engine::CardView> {
    let definition = if color == Color::Black { "RAV-SWAMP" } else { "RAV-MOUNTAIN" };
    view.own_battlefield.iter().find(|card| card.definition == Some(definition) && !card.tapped)
}

fn demon_generic_payment(view: &GameView) -> Option<Vec<Color>> {
    if view.mana_pool.total() < 5 || view.mana_pool.amount(Color::Black) < 2 { return None; }
    let mut colors = Vec::new();
    colors.extend(std::iter::repeat(Color::Black).take(usize::from(view.mana_pool.amount(Color::Black).saturating_sub(2))));
    colors.extend(std::iter::repeat(Color::Red).take(usize::from(view.mana_pool.amount(Color::Red))));
    if colors.len() < 3 { return None; }
    colors.truncate(3);
    Some(colors)
}

fn color_to_make_demon_castable(view: &GameView) -> Option<Color> {
    if view.mana_pool.amount(Color::Black) < 2 {
        Some(Color::Black)
    } else if view.mana_pool.total() < 5 {
        Some(if view.mana_pool.amount(Color::Red) < view.mana_pool.amount(Color::Black) { Color::Red } else { Color::Black })
    } else { None }
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.terminal != "Winner(PlayerId(0))" || captured.winner != Some(0) {
        return Err(format!("Woebringer Demon duel did not reach pilot win: {}", captured.terminal));
    }
    if captured.rejected_moves != 0 || captured.life_totals[0] <= 0 || captured.life_totals[1] > 0 {
        return Err(format!("Woebringer duel ended with invalid life/moves: {:?}", captured.life_totals));
    }
    let demon = captured.demon.ok_or("Woebringer Demon was not cast")?;
    if captured.payment.len() != 5 || captured.payment.iter().filter(|color| **color == Color::Black).count() < 2 {
        return Err(format!("Woebringer Demon did not pay exact {{3}}{{B}}{{B}}: {:?}", captured.payment));
    }
    if captured.final_zone != Some(Zone::Battlefield) || captured.power != Some(4)
        || captured.toughness != Some(4) || captured.keywords != [Keyword::Flying] {
        return Err(format!("Woebringer Demon characteristics differ from 4/4 Flying: {:?}/{:?} {:?} {:?}", captured.power, captured.toughness, captured.keywords, captured.final_zone));
    }
    let paid = event_index(&captured.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == demon && colors.len() == 5 && colors.iter().filter(|color| **color == Color::Black).count() >= 2)).ok_or("exact five-mana {3}{B}{B} payment receipt missing")?;
    let cast = event_index_after(&captured.events, paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == demon)).ok_or("Woebringer Demon cast receipt missing")?;
    let resolved = event_index_after(&captured.events, cast, |event| matches!(event, GameEvent::SpellResolved { card } if *card == demon)).ok_or("Woebringer Demon did not resolve")?;
    let entered = event_index_after(&captured.events, resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == demon)).ok_or("Woebringer Demon did not enter battlefield")?;
    let first_cycle = upkeep_sacrifice_cycle(captured, demon, PlayerId(1), entered)?;
    let second_cycle = upkeep_sacrifice_cycle(captured, demon, PlayerId(0), first_cycle.6)?;
    let attack = event_index_after(&captured.events, second_cycle.6, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&demon))).ok_or("Woebringer Demon did not attack")?;
    let damage = event_index_after(&captured.events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 4 } if *source == demon)).ok_or("4-point Demon combat damage is missing")?;
    let terminal = event_index_after(&captured.events, damage, |event| matches!(event, GameEvent::GameEnded { winner: Some(PlayerId(0)) })).ok_or("terminal pilot victory receipt missing")?;
    let order = [paid, cast, resolved, entered,
        first_cycle.0, first_cycle.1, first_cycle.2, first_cycle.3, first_cycle.4, first_cycle.5, first_cycle.6,
        second_cycle.0, second_cycle.1, second_cycle.2, second_cycle.3, second_cycle.4, second_cycle.5, second_cycle.6,
        attack, damage, terminal];
    if order.windows(2).any(|pair| pair[0] >= pair[1]) { return Err(format!("Woebringer event sequence is out of order: {order:?}")); }
    let selected: BTreeSet<usize> = order.into_iter().collect();
    let events = project_events(&captured.events).into_iter().filter(|event| selected.contains(&(event.sequence as usize))).collect::<Vec<_>>();
    Ok(json!({
        "trace_id":"rav-woebringer-demon-each-upkeep-active-player-sacrifice-pvp",
        "seed":seed,
        "decks":["rav-woebringer-demon-black-red-60","rav-war-torch-goblin-opponent-60"],
        "policies":["rav.woebringer-demon-each-upkeep-pilot.v1","rav.war-torch-goblin-upkeep-opponent.v1"],
        "terminal":captured.terminal,"winner_seat":captured.winner,"turns":captured.turns,
        "accepted_moves":captured.accepted_moves,"rejected_moves":captured.rejected_moves,
        "mechanics":{
            "mana_cost":"{3}{B}{B}",
            "paid_colors":captured.payment.iter().map(|c|format!("{c:?}")).collect::<Vec<_>>(),
            "power":captured.power,"toughness":captured.toughness,
            "keywords":captured.keywords.iter().map(|k|format!("{k:?}")).collect::<Vec<_>>(),
            "demon":demon.0,"upkeep_trigger":TRIGGER_ID,
            "opponent_upkeep_player":first_cycle.7.0.0,
            "opponent_sacrificed_permanent":first_cycle.7.1.0,
            "opponent_sacrificed_definition":first_cycle.7.2,
            "controller_upkeep_player":second_cycle.7.0.0,
            "controller_sacrificed_permanent":second_cycle.7.1.0,
            "controller_sacrificed_definition":second_cycle.7.2,
            "decision_kind":"TriggeredEffectObject",
            "active_player_choice_provenance":true,
            "terminal_pilot_win":captured.winner == Some(0)
        },
        "ordered_event_sequence":order,"events":events,"passed":true
    }))
}

// Returns the seven ordered references for the event sequence and the
// corresponding chosen permanent. The event order proves that the upkeep's
// active player, rather than the Demon controller unconditionally, answered.
type CycleResult = (usize, usize, usize, usize, usize, usize, usize, (PlayerId, ObjectId, &'static str));

fn upkeep_sacrifice_cycle(captured: &Captured, demon: ObjectId, active: PlayerId, after: usize) -> Result<CycleResult, String> {
    let upkeep = event_index_after(&captured.events, after, |event| matches!(event, GameEvent::StepBegan { active_player, step: Step::Upkeep, .. } if *active_player == active)).ok_or("required active-player upkeep receipt missing")?;
    let stacked = event_index_after(&captured.events, upkeep, |event| matches!(event, GameEvent::TriggeredAbilityStacked { controller: PlayerId(0), source, ability: TRIGGER_ID, .. } if *source == demon)).ok_or("Woebringer Demon did not stack its any-upkeep trigger")?;
    let decision_open = event_index_after(&captured.events, stacked, |event| matches!(event, GameEvent::DecisionOpened { player, kind: DecisionKind::TriggeredEffectObject, .. } if *player == active)).ok_or("upkeep active player did not receive the typed creature choice")?;
    let decision_complete = event_index_after(&captured.events, decision_open, |event| matches!(event, GameEvent::DecisionCompleted { player, kind: DecisionKind::TriggeredEffectObject, .. } if *player == active)).ok_or("upkeep active player did not complete their sacrifice choice")?;
    let sacrifice = event_index_after(&captured.events, decision_complete, |event| matches!(event, GameEvent::SacrificedByEffect { source, player, .. } if *source == demon && *player == active)).ok_or("active upkeep player did not sacrifice a controlled creature")?;
    let permanent = match &captured.events[sacrifice] { GameEvent::SacrificedByEffect { permanent, .. } => *permanent, _ => unreachable!() };
    let moved = event_index_after(&captured.events, sacrifice, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == permanent)).ok_or("chosen creature did not move to its owner's graveyard")?;
    let resolved = event_index_after(&captured.events, moved, |event| matches!(event, GameEvent::AbilityResolved { source, ability: TRIGGER_ID, .. } if *source == demon)).ok_or("Woebringer upkeep trigger did not resolve")?;
    let definition = captured.sacrificed_definitions.iter().find(|(player, card, _)| *player == active && *card == permanent).map(|(_, _, definition)| *definition).ok_or("sacrifice definition receipt was not captured")?;
    if definition == CARD_ID { return Err("policy sacrificed the Demon instead of preserving it as the engine source".to_owned()); }
    Ok((upkeep, stacked, decision_open, decision_complete, sacrifice, moved, resolved, (active, permanent, definition)))
}

fn event_index(events: &[GameEvent], mut predicate: impl FnMut(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(|event| predicate(event))
}

fn event_index_after(events: &[GameEvent], after: usize, mut predicate: impl FnMut(&GameEvent) -> bool) -> Option<usize> {
    events.iter().enumerate().skip(after + 1).find_map(|(index, event)| predicate(event).then_some(index))
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
    if std::env::args().any(|argument| argument == "--candidate") { "candidate" } else { "reference" }
}
