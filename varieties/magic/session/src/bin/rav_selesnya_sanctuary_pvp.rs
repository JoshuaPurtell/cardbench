//! Full-game diagnostic authority for RAV Selesnya Sanctuary.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DeckEntry, DeckList, Effect, Game, GameEvent,
    GameView, LandEntryBinding, ManaAbilityBinding, ManaAbilityOutput, ManaBundle,
    ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target,
    TargetRequirement, TriggerCondition, TriggeredAbilityBinding, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_land_entry_bindings,
    rav_main_set_catalog, rav_mana_ability_bindings,
    rav_static_continuous_effect_bindings, rav_triggered_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-SELESNYA-SANCTUARY";
const CARD_NAME: &str = "Selesnya Sanctuary";
const ABILITY_ID: &str = "return-controlled-land";
const MANA_ABILITY_ID: &str = "selesnya-sanctuary-gw";
const CREATURE_ID: &str = "RAV-WATCHWOLF";
const OPPONENT_CREATURE_ID: &str = "RAV-WAR-TORCH-GOBLIN";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/selesnya_sanctuary.rs";
const MAX_TURNS: u32 = 40;
const MAX_MOVES: u32 = 4_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/selesnya_sanctuary.rs"]
mod candidate_selesnya_sanctuary;

fn main() -> ExitCode {
    match run_authority() {
        Ok(report) => {
            println!("{}", serde_json::to_string(&report).expect("report serializes"));
            if report["passed"].as_bool() == Some(true) { ExitCode::SUCCESS } else { ExitCode::FAILURE }
        }
        Err(error) => {
            eprintln!("Selesnya Sanctuary PvP authority failed: {error}");
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
        return Err("Selesnya Sanctuary full-game replay differs".to_owned());
    }
    let mutations = json!({
        "enters_tapped_removed_detected": mutation_detected(seed, Mutation::Untapped)?,
        "controlled_land_target_restriction_changed_detected": mutation_detected(seed, Mutation::WrongTarget)?,
        "return_trigger_removed_detected": mutation_detected(seed, Mutation::NoTrigger)?,
        "green_white_mana_bundle_changed_detected": mutation_detected(seed, Mutation::WrongBundle)?,
        "sealed_mutation_authority": false
    });
    for key in ["enters_tapped_removed_detected", "controlled_land_target_restriction_changed_detected", "return_trigger_removed_detected", "green_white_mana_bundle_changed_detected"] {
        if mutations[key] != true { return Err(format!("Selesnya Sanctuary gameplay mutation escaped: {key}")); }
    }
    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown.iter().filter(|card| card["trace_status"] == "passed").count();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,"expansion":"RAV",
        "card":{"collector_number":281,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},"candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "checked_sections":["305.1 and 305.2: playing lands and land characteristics", "603.2-603.3: triggered ability occurrence and stacking", "601.2 and 608: targets and resolution", "701.17: returning a permanent", "106.4-106.5: mana abilities and produced mana", "508-510: combat damage", "704: state-based actions"],
            "source_contract":"CardBench-authored semantic summary for RAV Selesnya Sanctuary #281: a land that enters tapped, triggers its controller to return a controlled land to its owner's hand, and taps for {G}{W}.",
            "authority_cutoff":"2026-09-24","no_provider_trial":true
        },
        "passed":cards_passed == 1,"pilot_card_passed":cards_passed == 1,"cards_passed":cards_passed,
        "cards_credited":0,"cards_total":catalog.len(),"printings_total":RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
        "coverage_complete":false,"full_reward":false,"harbor_reward":0.0,"pilot_reward":0.0,
        "qualification_status":"unreviewed_diagnostic","replay_deterministic":true,
        "mutation_sensitivity":mutations,"traces":[trace],"per_card_breakdown":breakdown
    }))
}

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let unique: Vec<_> = printings.iter().copied().filter(|card| names.insert(card.name)).collect();
    if unique.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || !unique.iter().any(|card| card.collector_number == 281 && card.name == CARD_NAME) {
        return Err("RAV catalog identity or denominator differs for Selesnya Sanctuary".to_owned());
    }
    Ok(unique)
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH).map_err(|error| format!("could not read Selesnya Sanctuary source: {error}"))?;
    let file = syn::parse_file(&source).map_err(|_| "Selesnya Sanctuary source is not valid Rust".to_owned())?;
    if !file.attrs.iter().all(|attribute| attribute.path().is_ident("doc")) { return Err("Selesnya Sanctuary has unsupported module attributes".to_owned()); }
    let functions: Vec<_> = file.items.iter().filter_map(|item| match item {
        syn::Item::Fn(function) => Some(function.sig.ident.to_string()),
        syn::Item::Use(_) => None,
        _ => Some("<unexpected item>".to_owned()),
    }).collect();
    if functions != ["definition", "land_entry_binding", "mana_ability", "triggered_ability"] {
        return Err("Selesnya Sanctuary source must contain only its definition and three binding factories".to_owned());
    }
    Ok(())
}

fn validate_candidate_semantics() -> Result<(), String> {
    let candidate = candidate_selesnya_sanctuary::definition();
    let registered = card_definitions().into_iter().find(|definition| definition.id == CARD_ID).ok_or("Selesnya Sanctuary is absent from the RAV registry")?;
    let entry = candidate_selesnya_sanctuary::land_entry_binding();
    let registered_entry = rav_land_entry_bindings().into_iter().find(|binding| binding.card_definition == CARD_ID).ok_or("Selesnya Sanctuary land-entry binding is absent")?;
    let mana = candidate_selesnya_sanctuary::mana_ability();
    let registered_mana = rav_mana_ability_bindings().into_iter().find(|binding| binding.card_definition == CARD_ID).ok_or("Selesnya Sanctuary mana ability is absent")?;
    let trigger = candidate_selesnya_sanctuary::triggered_ability();
    let registered_trigger = rav_triggered_ability_bindings().into_iter().find(|binding| binding.card_definition == CARD_ID).ok_or("Selesnya Sanctuary return trigger is absent")?;
    let output_matches = matches!(&mana.ability.output, ManaAbilityOutput::Bundle(bundle) if bundle.amount(Color::Green) == 1 && bundle.amount(Color::White) == 1 && bundle.iter().count() == 2);
    if candidate != registered || candidate.id != CARD_ID || candidate.name != CARD_NAME || candidate.set_code != "RAV"
        || candidate.mana_cost != ManaCost::new(0) || !candidate.colors.is_empty() || !candidate.mana_colors.is_empty()
        || candidate.card_types != BTreeSet::from([CardType::Land]) || candidate.is_basic_land
        || candidate.power.is_some() || candidate.toughness.is_some() || !candidate.keywords.is_empty() || !candidate.effects.is_empty()
        || candidate.supported_rules != ["full-rules-fidelity", "enters-tapped", "etb-return-controlled-land", "fixed-green-white-mana-bundle"]
        || entry != registered_entry || entry.enters_tapped != true || entry.optional_life_payment.is_some()
        || mana != registered_mana || mana.ability.id != MANA_ABILITY_ID || !mana.ability.tap_cost || !output_matches
        || mana.ability.amount != 0 || mana.ability.life_payment.is_some() || mana.ability.controller_damage.is_some()
        || trigger != registered_trigger || trigger.ability.id != ABILITY_ID || trigger.ability.condition != TriggerCondition::EntersBattlefield
        || trigger.ability.optional || trigger.ability.targets != [TargetRequirement::ControlledLand]
        || trigger.ability.effects != [Effect::ReturnControlledLandToHand] {
        return Err("Selesnya Sanctuary candidate or registry differs from RAV #281 contract".to_owned());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation { None, Untapped, WrongTarget, NoTrigger, WrongBundle }

fn definitions_with_mutation(_mutation: Mutation) -> Vec<CardDefinition> { card_definitions() }

fn entries_with_mutation(mutation: Mutation) -> Vec<LandEntryBinding> {
    let mut entries = rav_land_entry_bindings();
    if mutation == Mutation::Untapped {
        if let Some(binding) = entries.iter_mut().find(|binding| binding.card_definition == CARD_ID) { binding.enters_tapped = false; }
    }
    entries
}

fn mana_with_mutation(mutation: Mutation) -> Vec<ManaAbilityBinding> {
    let mut abilities = rav_mana_ability_bindings();
    if mutation == Mutation::WrongBundle {
        if let Some(binding) = abilities.iter_mut().find(|binding| binding.card_definition == CARD_ID) {
            binding.ability.output = ManaAbilityOutput::Bundle(ManaBundle::new([(Color::Green, 1), (Color::Red, 1)]));
        }
    }
    abilities
}

fn triggers_with_mutation(mutation: Mutation) -> Vec<TriggeredAbilityBinding> {
    let mut triggers = rav_triggered_ability_bindings();
    if mutation == Mutation::NoTrigger { triggers.retain(|binding| binding.card_definition != CARD_ID); }
    if mutation == Mutation::WrongTarget {
        if let Some(binding) = triggers.iter_mut().find(|binding| binding.card_definition == CARD_ID) {
            binding.ability.targets = vec![TargetRequirement::Creature];
        }
    }
    triggers
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>, terminal: String, winner: Option<usize>, turns: u32,
    accepted_moves: u32, rejected_moves: u32, sanctuary: Option<ObjectId>, target: Option<ObjectId>,
    target_controller: Option<usize>, target_definition: Option<&'static str>,
    sanctuary_tapped: Option<bool>, target_returned_to_hand: bool, target_final_zone: Option<Zone>, bundle: [u8; 2],
    bundle_tapped: bool, bundle_mana_pool: [u8; 2], creature: Option<ObjectId>, creature_zone: Option<Zone>,
    creature_payment: Vec<Color>, life_totals: [i64; 2],
}

fn find_reference() -> Result<(u64, Captured), String> {
    let seed = 0;
    let capture = run_match(seed, Mutation::None)?;
    validate_trace(&capture, seed)?;
    Ok((seed, capture))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) { Err(_) => Ok(true), Ok(capture) => Ok(validate_trace(&capture, seed).is_err()) }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot_deck = DeckList { mainboard: vec![
        DeckEntry { card: CARD_ID.to_owned(), count: 4 },
        DeckEntry { card: CREATURE_ID.to_owned(), count: 4 },
        DeckEntry { card: "RAV-PLAINS".to_owned(), count: 26 },
        DeckEntry { card: "RAV-FOREST".to_owned(), count: 26 },
    ], sideboard: vec![] };
    let opponent_deck = DeckList { mainboard: vec![
        DeckEntry { card: OPPONENT_CREATURE_ID.to_owned(), count: 4 },
        DeckEntry { card: "RAV-MOUNTAIN".to_owned(), count: 56 },
    ], sideboard: vec![] };
    for (label, deck) in [("Selesnya pilot", &pilot_deck), ("Goblin opponent", &opponent_deck)] {
        let count: usize = deck.mainboard.iter().map(|entry| usize::from(entry.count)).sum();
        if count != 60 { return Err(format!("{label} deck has {count} cards; expected 60")); }
    }
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions_with_mutation(mutation), 2, mana_with_mutation(mutation), rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(), rav_activated_ability_bindings(), triggers_with_mutation(mutation),
        rav_static_continuous_effect_bindings(), entries_with_mutation(mutation),
    ).map_err(|error| error.to_string())?;
    game.set_shuffle_seed(seed).map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot_deck).map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent_deck).map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), 7).map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), 7).map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [Box::new(SanctuaryPolicy { player: PlayerId(0), sanctuary_played: false }), Box::new(GoblinPolicy { player: PlayerId(1), goblins_cast: 0 })];
    let mut capture = Captured { events: vec![], terminal: String::new(), winner: None, turns: 0, accepted_moves: 0, rejected_moves: 0, sanctuary: None, target: None, target_controller: None, target_definition: None, sanctuary_tapped: None, target_returned_to_hand: false, target_final_zone: None, bundle: [0, 0], bundle_tapped: false, bundle_mana_pool: [0, 0], creature: None, creature_zone: None, creature_payment: vec![], life_totals: [0, 0] };
    let mut attempts = 0_u32;
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || capture.accepted_moves >= MAX_MOVES { return Err(format!("Selesnya duel exceeded bound at turn {}", game.turn)); }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|error| error.to_string())?;
        let policy = policies.get_mut(player.0).ok_or("policy seat missing")?;
        let action = if view.pending_decision.is_some() { policy.propose_pending_decision(&view).ok_or("triggered ability target choice received no response")? }
            else if view.optional_triggered_ability_choice.is_some() { policy.propose_optional_triggered_ability(&view) }
            else if view.draw_replacement_pending { policy.propose_draw_replacement(&view) }
            else if view.private_library_choice.is_some() { policy.propose_private_library_choice(&view) }
            else if view.private_opponent_library_choice.is_some() { policy.propose_private_opponent_library_choice(&view) }
            else if view.library_search_choice.is_some() { policy.propose_library_search_choice(&view) }
            else { policy.propose_move(&view) };
        if let PolicyAction::ChooseTriggeredAbilityTargets { source, targets, .. } = &action
            && game.card_definition(*source).is_ok_and(|definition| definition.id == CARD_ID)
        {
            if let Some(Target::Permanent(target)) = targets.first() {
                capture.target = Some(*target);
                capture.target_controller = game.controller_of(*target).ok().map(|owner| owner.0);
                capture.target_definition = game.card_definition(*target).ok().map(|definition| definition.id);
            }
        }
        let cursor = game.event_log.len();
        attempts += 1;
        game.submit_policy_move(player, policy.id(), action).map_err(|error| format!("policy action rejected turn {} {:?}: {error}", view.turn, view.step))?;
        capture.accepted_moves += 1;
        game.validate_invariants().map_err(|error| format!("post-move invariant failure: {error}"))?;
        for event in &game.event_log[cursor..] {
            if let GameEvent::CardMoved { card, to: Zone::Battlefield } = event
                && game.card_definition(*card).is_ok_and(|definition| definition.id == CARD_ID)
            {
                capture.sanctuary = Some(*card);
                capture.sanctuary_tapped = game.object(*card).ok().map(|object| object.tapped);
            }
            if let GameEvent::CardMoved { card, to: Zone::Hand } = event && Some(*card) == capture.target {
                capture.target_returned_to_hand = true;
            }
            if let GameEvent::BoundManaAbilityFreeBundleActivated { source, ability, bundle, tapped, .. } = event
                && Some(*source) == capture.sanctuary && *ability == MANA_ABILITY_ID
            {
                capture.bundle = [bundle.amount(Color::Green), bundle.amount(Color::White)];
                capture.bundle_tapped = *tapped;
                let pool = &game.player(PlayerId(0)).map_err(|error| error.to_string())?.mana_pool;
                capture.bundle_mana_pool = [pool.amount(Color::Green), pool.amount(Color::White)];
            }
            if let GameEvent::SpellCast { player: PlayerId(0), card } = event
                && game.card_definition(*card).is_ok_and(|definition| definition.id == CREATURE_ID)
            { capture.creature = Some(*card); }
            if let GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } = event
                && game.card_definition(*card).is_ok_and(|definition| definition.id == CREATURE_ID)
            { capture.creature_payment = colors.clone(); }
        }
    }
    capture.events = game.event_log.clone();
    capture.terminal = game.winner().map_or_else(|| "Draw".to_owned(), |winner| format!("Winner(PlayerId({}))", winner.0));
    capture.winner = game.winner().map(|winner| winner.0);
    capture.turns = game.turn;
    capture.rejected_moves = attempts.saturating_sub(capture.accepted_moves);
    capture.target_final_zone = capture.target.and_then(|target| game.zone_of(target));
    capture.creature_zone = capture.creature.and_then(|creature| game.zone_of(creature));
    capture.life_totals = [game.player(PlayerId(0)).map_err(|error| error.to_string())?.life, game.player(PlayerId(1)).map_err(|error| error.to_string())?.life];
    Ok(capture)
}

struct SanctuaryPolicy { player: PlayerId, sanctuary_played: bool }
struct GoblinPolicy { player: PlayerId, goblins_cast: u8 }

impl SanctuaryPolicy {
    fn target_action(&self, view: &GameView) -> Option<PolicyAction> {
        let choice = view.triggered_ability_target_choice.as_ref()?;
        if choice.ability != ABILITY_ID { return None; }
        let target = choice.target_options.first()?.iter().find_map(|target| match target {
            Target::Permanent(id) if view.own_battlefield.iter().any(|card| card.id == *id && card.definition != Some(CARD_ID) && card.card_types.contains(&CardType::Land)) => Some(*target),
            _ => None,
        }).or_else(|| choice.target_options.first()?.first().copied())?;
        Some(PolicyAction::ChooseTriggeredAbilityTargets { decision: choice.decision, source: choice.source, ability: choice.ability, targets: vec![target] })
    }
}

impl CodePolicy for SanctuaryPolicy {
    fn id(&self) -> &'static str { "rav.selesnya-sanctuary-gw-bounce-pilot.v1" }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers && view.active_player == self.player && view.player == self.player && !view.attackers_declared {
            return PolicyAction::DeclareAttackers { attackers: view.own_battlefield.iter().filter(|card| card.definition == Some(CREATURE_ID) && card.can_attack).map(|card| card.id).collect() };
        }
        if view.step == Step::DeclareBlockers && view.active_player != self.player && view.player == self.player && !view.blockers_declared { return PolicyAction::DeclareBlockers { assignments: vec![] }; }
        if view.player != self.player || view.decision_player != self.player || view.priority != self.player { return PolicyAction::PassPriority; }
        if view.active_player == self.player && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain) && view.stack_depth == 0 {
            if view.lands_played == 0 && let Some(land) = choose_pilot_land(view, self.sanctuary_played) {
                if land.definition == Some(CARD_ID) { self.sanctuary_played = true; }
                return PolicyAction::PlayLand { card: land.id };
            }
            if let Some(creature) = view.hand.iter().find(|card| card.definition == Some(CREATURE_ID)) {
                if view.mana_pool.amount(Color::Green) > 0 && view.mana_pool.amount(Color::White) > 0 {
                    return PolicyAction::CastWithPayment { request: cardbench_magic_engine::CastRequest { card: creature.id, targets: vec![], convoke: vec![], payment_mana_abilities: vec![] }, chosen_x: None, mana_selection: ManaPaymentSelection::default() };
                }
                if let Some(sanctuary) = view.own_battlefield.iter().find(|card| card.definition == Some(CARD_ID) && !card.tapped) {
                    return PolicyAction::ActivateBoundManaAbility { activation: cardbench_magic_engine::ManaAbilityActivation { source: sanctuary.id, ability_id: MANA_ABILITY_ID, chosen_color: None } };
                }
                let missing = if view.mana_pool.amount(Color::Green) == 0 { Color::Green } else { Color::White };
                if let Some(land) = view.own_battlefield.iter().find(|card| !card.tapped && card.definition == Some(if missing == Color::Green { "RAV-FOREST" } else { "RAV-PLAINS" })) {
                    return PolicyAction::ActivateManaAbility { land: land.id, color: missing };
                }
            }
            if view.lands_played == 0 && let Some(land) = choose_pilot_land(view, self.sanctuary_played) { return PolicyAction::PlayLand { card: land.id }; }
        }
        PolicyAction::PassPriority
    }
    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> { self.target_action(view) }
}

impl CodePolicy for GoblinPolicy {
    fn id(&self) -> &'static str { "rav.war-torch-goblin-ground-blocker.v1" }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers && view.active_player == self.player && view.player == self.player && !view.attackers_declared { return PolicyAction::DeclareAttackers { attackers: vec![] }; }
        if view.step == Step::DeclareBlockers && view.active_player != self.player && view.player == self.player && !view.blockers_declared {
            let attacker = view.combat_attackers.iter().find(|card| card.definition == Some(CREATURE_ID));
            let blocker = view.own_battlefield.iter().find(|card| card.definition == Some(OPPONENT_CREATURE_ID) && card.can_block);
            if let (Some(attacker), Some(blocker)) = (attacker, blocker) { return PolicyAction::DeclareBlockers { assignments: vec![cardbench_magic_engine::CombatBlock { attacker: attacker.id, blocker: blocker.id }] }; }
            return PolicyAction::DeclareBlockers { assignments: vec![] };
        }
        if view.player != self.player || view.decision_player != self.player || view.priority != self.player { return PolicyAction::PassPriority; }
        if view.active_player == self.player && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain) && view.stack_depth == 0 {
            if view.lands_played == 0 && let Some(land) = view.hand.iter().find(|card| card.definition == Some("RAV-MOUNTAIN")) { return PolicyAction::PlayLand { card: land.id }; }
            if self.goblins_cast < 4 && let Some(goblin) = view.hand.iter().find(|card| card.definition == Some(OPPONENT_CREATURE_ID)) {
                if view.mana_pool.amount(Color::Red) > 0 {
                    self.goblins_cast += 1;
                    return PolicyAction::CastWithPayment { request: cardbench_magic_engine::CastRequest { card: goblin.id, targets: vec![], convoke: vec![], payment_mana_abilities: vec![] }, chosen_x: None, mana_selection: ManaPaymentSelection::default() };
                }
                if let Some(mountain) = view.own_battlefield.iter().find(|card| card.definition == Some("RAV-MOUNTAIN") && !card.tapped) { return PolicyAction::ActivateManaAbility { land: mountain.id, color: Color::Red }; }
            }
        }
        PolicyAction::PassPriority
    }
}

fn choose_pilot_land(view: &GameView, sanctuary_played: bool) -> Option<&cardbench_magic_engine::CardView> {
    let basic = view.hand.iter().find(|card| matches!(card.definition, Some("RAV-PLAINS" | "RAV-FOREST")));
    if sanctuary_played { return basic; }
    let sanctuary = view.hand.iter().find(|card| card.definition == Some(CARD_ID));
    if view.own_battlefield.iter().any(|card| card.card_types.contains(&CardType::Land)) { sanctuary.or(basic) } else { basic.or(sanctuary) }
}

fn validate_trace(c: &Captured, seed: u64) -> Result<Value, String> {
    if c.terminal != "Winner(PlayerId(0))" || c.winner != Some(0) || c.rejected_moves != 0 || c.life_totals[0] <= 0 || c.life_totals[1] > 0 { return Err(format!("Selesnya duel did not reach clean pilot victory: {} {:?}", c.terminal, c.life_totals)); }
    let sanctuary = c.sanctuary.ok_or("Selesnya Sanctuary never entered battlefield")?;
    let target = c.target.ok_or("controlled-land target selection missing")?;
    if c.sanctuary_tapped != Some(true) || c.target_controller != Some(0) || c.target_definition == Some(CARD_ID) || !c.target_returned_to_hand {
        return Err(format!("Selesnya entry or return mismatch: tapped={:?}, target controller={:?}, target={:?}, zone={:?}", c.sanctuary_tapped, c.target_controller, c.target_definition, c.target_final_zone));
    }
    if c.bundle != [1,1] || !c.bundle_tapped || c.bundle_mana_pool[0] < 1 || c.bundle_mana_pool[1] < 1 { return Err(format!("Selesnya did not tap for exact {{G}}{{W}} bundle: {:?} pool {:?}", c.bundle, c.bundle_mana_pool)); }
    let creature = c.creature.ok_or("Watchwolf was not cast")?;
    if c.creature_zone != Some(Zone::Battlefield) { return Err("Watchwolf left the battlefield before terminal win".to_owned()); }
    if c.creature_payment.len() != 2 || !c.creature_payment.contains(&Color::Green) || !c.creature_payment.contains(&Color::White) {
        return Err(format!("Watchwolf did not pay exact {{G}}{{W}} from mana pool: {:?}", c.creature_payment));
    }
    let moved_in = event_index(&c.events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == sanctuary)).ok_or("Sanctuary battlefield entry receipt missing")?;
    let trigger = event_index_after(&c.events, moved_in, |event| matches!(event, GameEvent::TriggeredAbilityStacked { source, ability: ABILITY_ID, .. } if *source == sanctuary)).ok_or("Sanctuary return-land ETB trigger was not stacked")?;
    let returned = event_index_after(&c.events, trigger, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == target)).ok_or("exact controlled land target did not return to hand")?;
    let resolved = event_index_after(&c.events, returned, |event| matches!(event, GameEvent::AbilityResolved { source, ability: ABILITY_ID, .. } if *source == sanctuary)).ok_or("Sanctuary ETB trigger did not resolve")?;
    let bundle = event_index_after(&c.events, resolved, |event| matches!(event, GameEvent::BoundManaAbilityFreeBundleActivated { player: PlayerId(0), source, ability: MANA_ABILITY_ID, bundle, tapped: true, .. } if *source == sanctuary && bundle.amount(Color::Green) == 1 && bundle.amount(Color::White) == 1 && bundle.iter().count() == 2)).ok_or("Sanctuary did not tap for one green and one white mana")?;
    let paid = event_index_after(&c.events, bundle, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == creature && colors.len() == 2 && colors.contains(&Color::Green) && colors.contains(&Color::White))).ok_or("Watchwolf did not spend the Sanctuary's green-white bundle")?;
    let cast = event_index_after(&c.events, paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == creature)).ok_or("Watchwolf did not cast after paying the bundle")?;
    let attack = event_index_after(&c.events, cast, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&creature))).ok_or("Watchwolf did not attack")?;
    let damage = event_index_after(&c.events, attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 3 } if *source == creature)).ok_or("Watchwolf did not deal combat damage to the opponent")?;
    let terminal = event_index_after(&c.events, damage, |event| matches!(event, GameEvent::GameEnded { winner: Some(PlayerId(0)) })).ok_or("terminal pilot victory receipt missing")?;
    if !(moved_in < trigger && trigger < returned && returned < resolved && resolved < bundle && bundle < paid && paid < cast && cast < attack && attack < damage && damage < terminal) { return Err("Selesnya entry, trigger, mana, combat, and terminal receipts are out of order".to_owned()); }
    let order = [moved_in, trigger, returned, resolved, bundle, paid, cast, attack, damage, terminal];
    let selected: BTreeSet<usize> = order.into_iter().collect();
    let events = project_events(&c.events).into_iter().filter(|event| selected.contains(&(event.sequence as usize))).collect::<Vec<_>>();
    Ok(json!({
        "trace_id":"rav-selesnya-sanctuary-tapped-bounce-bundle-pvp","seed":seed,
        "decks":["rav-selesnya-sanctuary-watchwolf-60","rav-war-torch-goblin-opponent-60"],
        "policies":["rav.selesnya-sanctuary-gw-bounce-pilot.v1","rav.war-torch-goblin-ground-blocker.v1"],
        "terminal":c.terminal,"winner_seat":c.winner,"turns":c.turns,"accepted_moves":c.accepted_moves,"rejected_moves":c.rejected_moves,
        "mechanics":{"land":sanctuary.0,"enters_tapped":true,"target":target.0,"target_controller":c.target_controller,"target_definition":c.target_definition,"target_returns_to_hand":c.target_returned_to_hand,"target_final_zone":format!("{:?}",c.target_final_zone),"mana_ability":MANA_ABILITY_ID,"bundle":["Green","White"],"bundle_tapped_source":true,"mana_pool_at_activation":c.bundle_mana_pool,"watchwolf":creature.0,"watchwolf_paid_colors":c.creature_payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),"watchwolf_attacked":true,"terminal_pilot_win":c.winner == Some(0)},
        "ordered_event_sequence":order,"events":events,"passed":true
    }))
}

fn event_index(events: &[GameEvent], mut predicate: impl FnMut(&GameEvent) -> bool) -> Option<usize> { events.iter().position(|event| predicate(event)) }
fn event_index_after(events: &[GameEvent], after: usize, mut predicate: impl FnMut(&GameEvent) -> bool) -> Option<usize> { events.iter().enumerate().skip(after + 1).find_map(|(index, event)| predicate(event).then_some(index)) }

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id), CardSemanticStatus::CatalogOnly { .. } => None };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":definition_id,"engine_status":match card.semantic_status { CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice", CardSemanticStatus::CatalogOnly { .. } => "catalog_only" },"trace_status":if passed { "passed" } else { "not_qualified" },"credited":false,"qualification":if passed { "pilot_trace_passed; independent reviewer approval pending" } else { "not_evaluated"}})
}
fn candidate_kind() -> &'static str { if std::env::args().any(|argument| argument == "--candidate") { "candidate" } else { "reference" } }
