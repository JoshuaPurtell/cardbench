//! Full-game RAV Tidewater Minion authority for its two activated abilities.

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbilityBinding, CardDefinition, CardType, Color, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, Keyword, ManaAbilityActivation, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, TargetRequirement, Zone,
};
use cardbench_magic_policies::{CodePolicy, DeckMatchConfig};
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    card_definitions, rav_activated_ability_bindings, rav_additional_spell_cost_bindings,
    rav_basic_land_type_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::parse::Parser;
use syn::{Expr, Item, Meta, ReturnType, Stmt, UseTree, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-TIDEWATER-MINION";
const CARD_NAME: &str = "Tidewater Minion";
const SIGNET_ID: &str = "RAV-BOROS-SIGNET";
const ISLAND_ID: &str = "RAV-ISLAND";
const UNTAP_ABILITY_ID: &str = "tap-untap-target-permanent";
const DEFENDER_ABILITY_ID: &str = "lose-defender-until-end-of-turn";
const SIGNET_MANA_ABILITY_ID: &str = "boros-signet-rw";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/tidewater_minion.rs";
const MAX_SEED: u64 = 128;
const MAX_TURNS: u32 = 60;
const MAX_MOVES: u32 = 8_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/tidewater_minion.rs"]
mod candidate_tidewater_minion;

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
            eprintln!("Tidewater Minion PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    // Never invoke candidate code until the complete candidate module has
    // passed the literal AST whitelist and the compile-valid injection probe.
    validate_candidate_source()?;
    let source_guard = validate_side_effect_mutation()?;
    validate_candidate_contract()?;
    validate_registry_delegation()?;
    let catalog = unique_catalog()?;
    let (seed, reference) = find_reference()?;
    let trace = validate_trace(&reference, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if reference != replay || project_events(&reference.events) != project_events(&replay.events) {
        return Err("Tidewater Minion replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "printed_mana_cost_mutation_detected": mutation_detected(seed, Mutation::WrongCardCost)?,
        "base_power_mutation_detected": mutation_detected(seed, Mutation::WrongPower)?,
        "untap_target_scope_mutation_detected": mutation_detected(seed, Mutation::LandOnlyTarget)?,
        "untap_effect_mutation_detected": mutation_detected(seed, Mutation::NoUntapEffect)?,
        "defender_ability_cost_mutation_detected": mutation_detected(seed, Mutation::WrongDefenderCost)?,
        "defender_effect_mutation_detected": mutation_detected(seed, Mutation::NoDefenderEffect)?,
        "untap_binding_removal_detected": mutation_detected(seed, Mutation::RemoveUntapBinding)?,
        "defender_binding_removal_detected": mutation_detected(seed, Mutation::RemoveDefenderBinding)?,
        "compile_valid_side_effect_source_rejected_before_candidate_calls": source_guard["side_effect_mutation_compile_valid_by_construction"] == true && source_guard["side_effect_mutation_rejected"] == true,
        "sealed_mutation_authority": false
    });
    for key in [
        "printed_mana_cost_mutation_detected",
        "base_power_mutation_detected",
        "untap_target_scope_mutation_detected",
        "untap_effect_mutation_detected",
        "defender_ability_cost_mutation_detected",
        "defender_effect_mutation_detected",
        "untap_binding_removal_detected",
        "defender_binding_removal_detected",
        "compile_valid_side_effect_source_rejected_before_candidate_calls",
    ] {
        if mutations[key] != true {
            return Err(format!("Tidewater Minion mutation escaped: {key}"));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,
        "expansion":"RAV",
        "card":{"collector_number":71,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "authority_cutoff":"2026-09-24",
            "no_provider_trial":true,
            "source_contract":"Original RAV #71: {3}{U}{U} 4/4 Creature — Elemental Minion with Defender; {4}: Tidewater Minion loses Defender until end of turn; {T}: Untap target permanent.",
            "oracle_reconciliation":"Current Oracle uses this creature and the reminder text that Defender prevents attacking; costs and effects remain {4} and {T}, respectively.",
            "subtype_model_limit":"Elemental Minion is retained in task metadata; CardDefinition currently cannot assert creature subtypes.",
            "checked_sections":["302.6: summoning sickness and tap-symbol activated costs","602.2: activating an activated ability","608.2: resolving activated abilities and targets","701.21: untapping permanents","611.2a: until-end-of-turn effects","702.3: Defender","508-510: attack declarations and combat damage"]
        },
        "source_guard":source_guard,
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
        "qualification_status":"unreviewed_diagnostic; independent reviewer approval and candidate trial remain separate",
        "replay_deterministic":true,
        "mutation_sensitivity":mutations,
        "traces":[trace],
        "per_card_breakdown":breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongCardCost,
    WrongPower,
    LandOnlyTarget,
    NoUntapEffect,
    WrongDefenderCost,
    NoDefenderEffect,
    RemoveUntapBinding,
    RemoveDefenderBinding,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    terminal: String,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    minion: Option<ObjectId>,
    signet: Option<ObjectId>,
    source_mana_cost: Option<ManaCost>,
    minion_incarnation_at_untap: Option<u64>,
    minion_incarnation_at_defender_loss: Option<u64>,
    cast_payment: Vec<Color>,
    source_resolved: bool,
    source_entered: bool,
    signet_controlled_by_opponent: bool,
    signet_was_tapped_before_untap: bool,
    signet_untapped_after_resolution: bool,
    untap_target_seen_on_stack: bool,
    untap_activation_count: usize,
    untap_resolution_count: usize,
    minion_tapped_for_untap_cost: bool,
    paid_defender_ability_count: usize,
    defender_loss_resolution_count: usize,
    defender_removed_live: bool,
    minion_untapped_after_defender_resolution: bool,
    defender_effect_expired: bool,
    defender_restored_after_expiration: bool,
    minion_attack_count: usize,
    minion_four_damage_count: usize,
    minion_zone: Option<Zone>,
    final_power: Option<i32>,
    final_toughness: Option<i32>,
    final_has_defender: bool,
    life: [i64; 2],
}

fn find_reference() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_TIDEWATER_MINION_SEED") {
        let seed = value.parse::<u64>().map_err(|_| {
            "CARDBENCH_TIDEWATER_MINION_SEED must be an unsigned integer".to_owned()
        })?;
        let capture = run_match(seed, Mutation::None)?;
        validate_trace(&capture, seed)?;
        return Ok((seed, capture));
    }
    let mut last_error = String::new();
    for seed in 0..=MAX_SEED {
        match run_match(seed, Mutation::None)
            .and_then(|capture| validate_trace(&capture, seed).map(|_| capture))
        {
            Ok(capture) => return Ok((seed, capture)),
            Err(error) => last_error = error,
        }
    }
    Err(format!(
        "no Tidewater Minion full-game trace found in seeds 0..={MAX_SEED}: {last_error}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> Result<bool, String> {
    match run_match(seed, mutation) {
        Err(_) => Ok(true),
        Ok(capture) => Ok(validate_trace(&capture, seed).is_err()),
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let pilot = deck(&[(CARD_ID, 4), (ISLAND_ID, 56)]);
    let opponent = deck(&[(SIGNET_ID, 4), (ISLAND_ID, 56)]);
    for (label, list) in [("pilot", &pilot), ("opponent", &opponent)] {
        let count: usize = list
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum();
        if count != 60 {
            return Err(format!("Tidewater Minion {label} deck has {count} cards"));
        }
    }

    let mut definitions = card_definitions();
    let mut minion_definition = candidate_tidewater_minion::definition();
    match mutation {
        Mutation::WrongCardCost => {
            minion_definition.mana_cost = ManaCost::with_colors(2, [Color::Blue, Color::Blue]);
        }
        Mutation::WrongPower => minion_definition.power = Some(5),
        _ => {}
    }
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Tidewater Minion is absent from the RAV definition registry")? = minion_definition;

    let mut activations = rav_activated_ability_bindings();
    activations.retain(|binding| binding.card_definition != CARD_ID);
    let mut untap = candidate_tidewater_minion::untap_target_permanent_ability();
    let mut defender = candidate_tidewater_minion::lose_defender_ability();
    match mutation {
        Mutation::LandOnlyTarget => untap.ability.targets = vec![TargetRequirement::Land],
        Mutation::NoUntapEffect => untap.ability.effects.clear(),
        Mutation::WrongDefenderCost => defender.ability.mana_cost = ManaCost::new(3),
        Mutation::NoDefenderEffect => defender.ability.effects.clear(),
        _ => {}
    }
    if mutation != Mutation::RemoveUntapBinding {
        activations.push(untap);
    }
    if mutation != Mutation::RemoveDefenderBinding {
        activations.push(defender);
    }
    let mut game = Game::new_with_all_bindings(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        activations,
    )
    .map_err(|error| error.to_string())?;
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    for (player, list) in [(PlayerId(0), &pilot), (PlayerId(1), &opponent)] {
        game.load_deck_into_library(player, list)
            .map_err(|error| error.to_string())?;
        game.draw_opening_hand(player, DeckMatchConfig::default().opening_hand_size)
            .map_err(|error| error.to_string())?;
    }
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(MinionPolicy::new(PlayerId(0))),
        Box::new(MinionPolicy::new(PlayerId(1))),
    ];
    let mut target_seen_on_stack = false;
    let mut signet_controlled_by_opponent = false;
    let mut target_tapped_before_untap = false;
    let mut signet_untapped_after_resolution = false;
    let mut defender_removed_live = false;
    let mut minion_tapped_for_untap_cost = false;
    let mut minion_untapped_after_defender_resolution = false;
    let mut defender_restored_after_expiration = false;
    let mut minion_incarnation_at_untap = None;
    let mut minion_incarnation_at_defender_loss = None;
    let mut accepted = 0_u32;
    let mut attempts = 0_u32;

    while !game.is_game_over() {
        if game.turn > MAX_TURNS || accepted >= MAX_MOVES {
            return Err(format!(
                "Tidewater Minion match exceeded bound on turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        if view.stack_activated_abilities.iter().any(|stack| {
            stack.source.definition == Some(CARD_ID)
                && stack.ability == UNTAP_ABILITY_ID
                && stack.targets.iter().any(|target| {
                    matches!(target, Target::Permanent(object) if Some(*object) == find_signet(&game))
                })
        }) {
            target_seen_on_stack = true;
        }
        let policy = policies
            .get_mut(player.0)
            .ok_or("Tidewater Minion policy seat missing")?;
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
        if let PolicyAction::ActivateAbility { activation } = &action
            && activation.ability_id == UNTAP_ABILITY_ID
            && let Some(Target::Permanent(target)) = activation.targets.first()
        {
            let target_view = view
                .opponent_battlefield
                .iter()
                .find(|permanent| permanent.id == *target);
            signet_controlled_by_opponent =
                target_view.is_some_and(|permanent| permanent.controller == PlayerId(1));
            target_tapped_before_untap = target_view.is_some_and(|permanent| permanent.tapped);
        }
        let cursor = game.event_log.len();
        attempts += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "policy rejected on turn {} at {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        for event in &game.event_log[cursor..] {
            match event {
                GameEvent::AbilityActivated {
                    player: PlayerId(0),
                    source,
                    source_incarnation,
                    definition: CARD_ID,
                    ability: UNTAP_ABILITY_ID,
                } => {
                    minion_incarnation_at_untap = Some((*source, *source_incarnation));
                    minion_tapped_for_untap_cost =
                        game.object(*source).is_ok_and(|permanent| permanent.tapped);
                }
                GameEvent::AbilityActivated {
                    player: PlayerId(0),
                    source,
                    source_incarnation,
                    definition: CARD_ID,
                    ability: DEFENDER_ABILITY_ID,
                } => minion_incarnation_at_defender_loss = Some((*source, *source_incarnation)),
                GameEvent::PermanentUntapped { source, card }
                    if Some(*source) == find_minion(&game) && Some(*card) == find_signet(&game) =>
                {
                    signet_untapped_after_resolution =
                        game.object(*card).is_ok_and(|permanent| !permanent.tapped);
                }
                GameEvent::AbilityResolved {
                    source,
                    ability: DEFENDER_ABILITY_ID,
                    ..
                } if Some(*source) == find_minion(&game) => {
                    defender_removed_live =
                        game.characteristics(*source).is_ok_and(|characteristics| {
                            !characteristics.keywords.contains(&Keyword::Defender)
                        });
                    minion_untapped_after_defender_resolution = game
                        .object(*source)
                        .is_ok_and(|permanent| !permanent.tapped);
                }
                GameEvent::ContinuousEffectExpired { source, target, .. }
                    if Some(*source) == find_minion(&game)
                        && Some(*target) == find_minion(&game) =>
                {
                    defender_restored_after_expiration =
                        game.characteristics(*target).is_ok_and(|characteristics| {
                            characteristics.keywords.contains(&Keyword::Defender)
                        });
                }
                _ => {}
            }
        }
        game.validate_invariants()
            .map_err(|error| format!("post-move invariant failure: {error}"))?;
    }

    let events = game.event_log.clone();
    let minion = find_minion(&game);
    let signet = find_signet(&game);
    let cast_payment = minion
        .and_then(|source| {
            events.iter().find_map(|event| match event {
                GameEvent::SpellManaPaid {
                    player: PlayerId(0),
                    card,
                    colors,
                } if *card == source => Some(colors.clone()),
                _ => None,
            })
        })
        .unwrap_or_default();
    let source_mana_cost = minion.and_then(|source| {
        game.card_definition(source)
            .ok()
            .map(|definition| definition.mana_cost.clone())
    });
    let life = [
        game.player(PlayerId(0))
            .map_err(|error| error.to_string())?
            .life,
        game.player(PlayerId(1))
            .map_err(|error| error.to_string())?
            .life,
    ];
    let source_incarnation_at_untap = minion_incarnation_at_untap
        .filter(|(source, _)| Some(*source) == minion)
        .map(|(_, incarnation)| incarnation);
    let source_incarnation_at_defender_loss = minion_incarnation_at_defender_loss
        .filter(|(source, _)| Some(*source) == minion)
        .map(|(_, incarnation)| incarnation);
    let final_characteristics = minion.and_then(|source| game.characteristics(source).ok());
    let minion_zone = minion.and_then(|source| game.zone_of(source));
    Ok(Captured {
        untap_target_seen_on_stack: target_seen_on_stack,
        signet_was_tapped_before_untap: target_tapped_before_untap,
        signet_untapped_after_resolution,
        minion_incarnation_at_untap: source_incarnation_at_untap,
        minion_incarnation_at_defender_loss: source_incarnation_at_defender_loss,
        events: events.clone(),
        winner: game.winner().map(|player| player.0),
        terminal: game.winner().map_or_else(
            || "Draw".to_owned(),
            |seat| format!("Winner(PlayerId({seat}))"),
        ),
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempts.saturating_sub(accepted),
        minion,
        signet,
        source_mana_cost,
        cast_payment,
        source_resolved: minion.is_some_and(|source| events.iter().any(|event| matches!(event,
            GameEvent::SpellResolved { card } if *card == source))),
        source_entered: minion.is_some_and(|source| events.iter().any(|event| matches!(event,
            GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == source))),
        signet_controlled_by_opponent,
        untap_activation_count: minion.map_or(0, |source| events.iter().filter(|event| matches!(event,
            GameEvent::AbilityActivated { player: PlayerId(0), source: paid_source, definition: CARD_ID, ability: UNTAP_ABILITY_ID, .. } if *paid_source == source)).count()),
        untap_resolution_count: minion.map_or(0, |source| events.iter().filter(|event| matches!(event,
            GameEvent::AbilityResolved { source: paid_source, ability: UNTAP_ABILITY_ID, .. } if *paid_source == source)).count()),
        minion_tapped_for_untap_cost,
        paid_defender_ability_count: minion.map_or(0, |source| events.iter().filter(|event| matches!(event,
            GameEvent::AbilityManaPaid { player: PlayerId(0), source: paid_source, ability: DEFENDER_ABILITY_ID, mana_cost } if *paid_source == source && *mana_cost == ManaCost::new(4))).count()),
        defender_loss_resolution_count: minion.map_or(0, |source| events.iter().filter(|event| matches!(event,
            GameEvent::AbilityResolved { source: paid_source, ability: DEFENDER_ABILITY_ID, .. } if *paid_source == source)).count()),
        defender_removed_live,
        minion_untapped_after_defender_resolution,
        defender_effect_expired: minion.is_some_and(|source| events.iter().any(|event| matches!(event,
            GameEvent::ContinuousEffectExpired { source: effect_source, target, .. } if *effect_source == source && *target == source))),
        defender_restored_after_expiration,
        minion_attack_count: minion.map_or(0, |source| events.iter().filter(|event| matches!(event,
            GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&source))).count()),
        minion_four_damage_count: minion.map_or(0, |source| events.iter().filter(|event| matches!(event,
            GameEvent::DamageDealtToPlayer { source: damage_source, player: PlayerId(1), amount: 4 } if *damage_source == source)).count()),
        minion_zone,
        final_power: final_characteristics.as_ref().and_then(|characteristics| characteristics.power),
        final_toughness: final_characteristics.as_ref().and_then(|characteristics| characteristics.toughness),
        final_has_defender: final_characteristics.is_some_and(|characteristics| characteristics.keywords.contains(&Keyword::Defender)),
        life,
    })
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

fn find_minion(game: &Game) -> Option<ObjectId> {
    game.event_log.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|definition| definition.id == CARD_ID) =>
        {
            Some(*card)
        }
        _ => None,
    })
}

fn find_signet(game: &Game) -> Option<ObjectId> {
    game.event_log.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(1),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|definition| definition.id == SIGNET_ID) =>
        {
            Some(*card)
        }
        _ => None,
    })
}

struct MinionPolicy {
    player: PlayerId,
    untap_activated: bool,
    defender_attempted_turn: Option<u32>,
    signet_mana_activated_turn: Option<u32>,
}

impl MinionPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            untap_activated: false,
            defender_attempted_turn: None,
            signet_mana_activated_turn: None,
        }
    }

    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let land_count = view
            .own_battlefield
            .iter()
            .filter(|permanent| permanent.card_types.contains(&CardType::Land))
            .count();
        if land_count >= 10 {
            return None;
        }
        view.hand
            .iter()
            .find(|card| card.definition == Some(ISLAND_ID))
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }
}

impl CodePolicy for MinionPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.tidewater-minion-pilot.v1"
        } else {
            "rav.tidewater-minion-signet-opponent.v1"
        }
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|permanent| permanent.can_attack)
                    .map(|permanent| permanent.id)
                    .collect(),
            };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.decision_player != self.player
            || view.priority != self.player
            || view.active_player != self.player
            || !view.step.is_main()
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(land) = self.play_land(view) {
            return land;
        }
        if self.player == PlayerId(1) {
            let signet = view
                .own_battlefield
                .iter()
                .find(|permanent| permanent.definition == Some(SIGNET_ID));
            if signet.is_none()
                && let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(SIGNET_ID))
            {
                let cost = ManaCost::new(2);
                if let Some(selection) = payment(view, &cost) {
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: selection,
                    };
                }
                if let Some(action) = make_mana(view, &cost, Color::Blue) {
                    return action;
                }
            }
            if let Some(signet) = signet.filter(|permanent| !permanent.tapped)
                && self.signet_mana_activated_turn != Some(view.turn)
            {
                let cost = ManaCost::new(1);
                if view.mana_pool.total() < 1 {
                    if let Some(action) = make_mana(view, &cost, Color::Blue) {
                        return action;
                    }
                    return PolicyAction::PassPriority;
                }
                self.signet_mana_activated_turn = Some(view.turn);
                return PolicyAction::ActivateBoundManaAbility {
                    activation: ManaAbilityActivation {
                        source: signet.id,
                        ability_id: SIGNET_MANA_ABILITY_ID,
                        chosen_color: None,
                    },
                };
            }
            return PolicyAction::PassPriority;
        }

        let minion = view
            .own_battlefield
            .iter()
            .find(|permanent| permanent.definition == Some(CARD_ID));
        if minion.is_none()
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
        {
            let cost = ManaCost::with_colors(3, [Color::Blue, Color::Blue]);
            if let Some(selection) = payment(view, &cost) {
                return PolicyAction::CastWithPayment {
                    request: cardbench_magic_engine::CastRequest {
                        card: card.id,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                    chosen_x: None,
                    mana_selection: selection,
                };
            }
            if let Some(action) = make_mana(view, &cost, Color::Blue) {
                return action;
            }
        }
        let Some(minion) = minion else {
            return PolicyAction::PassPriority;
        };
        if !self.untap_activated && !minion.summoning_sick && !minion.tapped {
            if let Some(target) = view
                .opponent_battlefield
                .iter()
                .find(|permanent| permanent.definition == Some(SIGNET_ID) && permanent.tapped)
            {
                self.untap_activated = true;
                return PolicyAction::ActivateAbility {
                    activation: AbilityActivation {
                        source: minion.id,
                        ability_id: UNTAP_ABILITY_ID,
                        sacrifice_sources: vec![],
                        additional_tap_creatures: vec![],
                        discard_cards: vec![],
                        targets: vec![Target::Permanent(target.id)],
                    },
                };
            }
        }
        let defender = minion.keywords.contains(&Keyword::Defender);
        if self.untap_activated
            && defender
            && !minion.summoning_sick
            && !minion.tapped
            && self.defender_attempted_turn != Some(view.turn)
        {
            let cost = ManaCost::new(4);
            if payment(view, &cost).is_some() {
                self.defender_attempted_turn = Some(view.turn);
                return PolicyAction::ActivateAbility {
                    activation: AbilityActivation {
                        source: minion.id,
                        ability_id: DEFENDER_ABILITY_ID,
                        sacrifice_sources: vec![],
                        additional_tap_creatures: vec![],
                        discard_cards: vec![],
                        targets: vec![],
                    },
                };
            }
            if let Some(action) = make_mana(view, &cost, Color::Blue) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
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

fn make_mana(view: &GameView, cost: &ManaCost, default: Color) -> Option<PolicyAction> {
    if payment(view, cost).is_some() {
        return None;
    }
    let preferred = cost
        .colored
        .iter()
        .copied()
        .find(|color| view.mana_pool.amount(*color) == 0)
        .unwrap_or(default);
    view.own_battlefield
        .iter()
        .find(|permanent| {
            permanent.card_types.contains(&CardType::Land)
                && !permanent.tapped
                && permanent.mana_colors.contains(&preferred)
        })
        .map(|land| PolicyAction::ActivateManaAbility {
            land: land.id,
            color: preferred,
        })
}

fn validate_trace(capture: &Captured, seed: u64) -> Result<Value, String> {
    let minion = capture.minion.ok_or("Tidewater Minion was not cast")?;
    let signet = capture
        .signet
        .ok_or("opponent did not cast the target Boros Signet")?;
    if capture.winner != Some(0)
        || capture.terminal != "Winner(PlayerId(0))"
        || capture.rejected_moves != 0
        || capture.minion_zone != Some(Zone::Battlefield)
        || capture.life[0] <= 0
        || capture.life[1] != 0
    {
        return Err("Tidewater Minion duel did not reach a clean Player 0 terminal win".to_owned());
    }
    if !capture.source_resolved
        || !capture.source_entered
        || capture.source_mana_cost != Some(ManaCost::with_colors(3, [Color::Blue, Color::Blue]))
        || capture.cast_payment.len() != 5
        || capture
            .cast_payment
            .iter()
            .filter(|color| **color == Color::Blue)
            .count()
            != 5
    {
        return Err("Tidewater Minion did not cast for exactly {3}{U}{U}".to_owned());
    }
    if !capture.signet_was_tapped_before_untap
        || !capture.signet_controlled_by_opponent
        || !capture.signet_untapped_after_resolution
        || !capture.untap_target_seen_on_stack
        || capture.untap_activation_count != 1
        || capture.untap_resolution_count != 1
        || !capture.minion_tapped_for_untap_cost
        || capture.events.iter().any(|event| matches!(event,
            GameEvent::AbilityManaPaid { source, ability: UNTAP_ABILITY_ID, .. } if *source == minion))
    {
        return Err("tap ability did not target and untap the opponent's tapped Signet".to_owned());
    }
    if capture.paid_defender_ability_count < 5
        || capture.defender_loss_resolution_count < 5
        || !capture.defender_removed_live
        || !capture.minion_untapped_after_defender_resolution
        || !capture.defender_effect_expired
        || !capture.defender_restored_after_expiration
        || capture.minion_attack_count < 5
        || capture.minion_four_damage_count < 5
        || capture.final_power != Some(4)
        || capture.final_toughness != Some(4)
    {
        return Err(format!(
            "{4} Defender removal must resolve and expire before five 4-damage attacks; paid={}, resolved={}, attacks={}, damage={}, restored={}",
            capture.paid_defender_ability_count,
            capture.defender_loss_resolution_count,
            capture.minion_attack_count,
            capture.minion_four_damage_count,
            capture.defender_restored_after_expiration
        ));
    }
    let payment = unique_event(&capture.events, |event| matches!(event,
        GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == minion && colors.len() == 5 && colors.iter().all(|color| *color == Color::Blue)))
        .ok_or("exact five-blue Tidewater Minion cast-payment receipt missing")?;
    let cast = event_after(&capture.events, payment, |event| {
        matches!(event,
        GameEvent::SpellCast { player: PlayerId(0), card } if *card == minion)
    })
    .ok_or("Tidewater Minion cast receipt missing")?;
    let resolve = event_after(&capture.events, cast, |event| {
        matches!(event,
        GameEvent::SpellResolved { card } if *card == minion)
    })
    .ok_or("Tidewater Minion spell resolution receipt missing")?;
    let entered = event_after(&capture.events, resolve, |event| {
        matches!(event,
        GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == minion)
    })
    .ok_or("Tidewater Minion battlefield entry receipt missing")?;
    let signet_tap = event_after(&capture.events, entered, |event| matches!(event,
        GameEvent::BoundManaAbilityBundleActivated { player: PlayerId(1), source, ability: SIGNET_MANA_ABILITY_ID, tapped: true, .. } if *source == signet))
        .ok_or("opponent Signet did not tap through its bound mana ability")?;
    let untap_activation = event_after(&capture.events, signet_tap, |event| matches!(event,
        GameEvent::AbilityActivated { player: PlayerId(0), source, source_incarnation, definition: CARD_ID, ability: UNTAP_ABILITY_ID } if *source == minion && Some(*source_incarnation) == capture.minion_incarnation_at_untap))
        .ok_or("Tidewater Minion tap ability activation/source incarnation missing")?;
    let target_untapped = event_after(&capture.events, untap_activation, |event| {
        matches!(event,
        GameEvent::PermanentUntapped { source, card } if *source == minion && *card == signet)
    })
    .ok_or("Tidewater Minion did not untap the exact opposing Signet")?;
    let untap_resolved = event_after(&capture.events, target_untapped, |event| matches!(event,
        GameEvent::AbilityResolved { source, source_incarnation, ability: UNTAP_ABILITY_ID } if *source == minion && Some(*source_incarnation) == capture.minion_incarnation_at_untap))
        .ok_or("Tidewater Minion tap ability resolution receipt missing")?;
    if !(payment < cast
        && cast < resolve
        && resolve < entered
        && entered < untap_activation
        && untap_activation < target_untapped
        && target_untapped < untap_resolved)
    {
        return Err(
            "cast, untap activation, state-change, and resolution order differs".to_owned(),
        );
    }
    let defender_paid = event_after(&capture.events, untap_resolved, |event| matches!(event,
        GameEvent::AbilityManaPaid { player: PlayerId(0), source, ability: DEFENDER_ABILITY_ID, mana_cost } if *source == minion && *mana_cost == ManaCost::new(4)))
        .ok_or("printed {4} Defender-removal payment receipt missing")?;
    let defender_activation = event_after(&capture.events, defender_paid, |event| matches!(event,
        GameEvent::AbilityActivated { player: PlayerId(0), source, source_incarnation, definition: CARD_ID, ability: DEFENDER_ABILITY_ID } if *source == minion && Some(*source_incarnation) == capture.minion_incarnation_at_defender_loss))
        .ok_or("Defender-removal activation/source incarnation receipt missing")?;
    let defender_resolved = event_after(&capture.events, defender_activation, |event| matches!(event,
        GameEvent::AbilityResolved { source, source_incarnation, ability: DEFENDER_ABILITY_ID } if *source == minion && Some(*source_incarnation) == capture.minion_incarnation_at_defender_loss))
        .ok_or("Defender-removal ability did not resolve")?;
    let effect_created = event_after(&capture.events, defender_resolved, |event| matches!(event,
        GameEvent::ContinuousEffectCreated { source, target, .. } if *source == minion && *target == minion))
        .ok_or("source-relative Defender removal effect was not created")?;
    let attack = event_after(&capture.events, effect_created, |event| matches!(event,
        GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&minion)))
        .ok_or("Minion did not attack after losing Defender")?;
    let damage = event_after(&capture.events, attack, |event| matches!(event,
        GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 4 } if *source == minion))
        .ok_or("unblocked Minion did not deal four combat damage")?;
    let effect_expired = event_after(&capture.events, damage, |event| matches!(event,
        GameEvent::ContinuousEffectExpired { source, target, .. } if *source == minion && *target == minion))
        .ok_or("Tidewater Minion Defender-removal effect did not expire at end of turn")?;
    let terminal = unique_event(&capture.events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("Player 0 terminal game-end receipt missing")?;
    if !(defender_paid < defender_activation
        && defender_activation < defender_resolved
        && defender_resolved < effect_created
        && effect_created < attack
        && attack < damage
        && damage < effect_expired
        && effect_expired < terminal)
    {
        return Err(
            "{4} payment, effect, attack, damage, and terminal win ordering differs".to_owned(),
        );
    }
    let events = project_events(&capture.events);
    let ordered = [
        payment,
        cast,
        resolve,
        entered,
        signet_tap,
        untap_activation,
        target_untapped,
        untap_resolved,
        defender_paid,
        defender_activation,
        defender_resolved,
        effect_created,
        attack,
        damage,
        effect_expired,
        terminal,
    ];
    let keep: BTreeSet<usize> = ordered.into_iter().collect();
    let projected: Vec<Value> = events.iter().enumerate().filter_map(|(index, event)| keep.contains(&index).then_some(json!({"sequence":event.sequence,"turn":event.turn,"kind":&event.kind,"canonical":&event.canonical,"facts":&event.facts}))).collect();
    Ok(json!({
        "id":"rav-tidewater-minion-two-abilities-full-game",
        "shuffle_seed":seed,
        "decks":["rav-tidewater-minion-island-pilot-60","rav-boros-signet-island-opponent-60"],
        "policies":["rav.tidewater-minion-pilot.v1","rav.tidewater-minion-signet-opponent.v1"],
        "terminal":capture.terminal,
        "winner_seat":capture.winner,
        "turns":capture.turns,
        "accepted_moves":capture.accepted_moves,
        "rejected_moves":capture.rejected_moves,
        "mechanics":{
            "definition_id":CARD_ID,
            "mana_cost":"{3}{U}{U}",
            "cast_payment_colors":capture.cast_payment.iter().map(|color|format!("{color:?}")).collect::<Vec<_>>(),
            "base_power":4,"base_toughness":4,"printed_subtypes":["Elemental","Minion"],
            "printed_subtypes_asserted_at_runtime":false,
            "starting_keywords":["Defender"],
            "tap_untap":{"ability":UNTAP_ABILITY_ID,"cost":"{T}","mana_cost":"{0}","target_requirement":"Permanent","target_object":signet.0,"target_definition_id":SIGNET_ID,"target_controller":1,"target_controlled_by_opponent":capture.signet_controlled_by_opponent,"target_was_tapped":capture.signet_was_tapped_before_untap,"target_seen_on_stack":capture.untap_target_seen_on_stack,"target_untapped":capture.signet_untapped_after_resolution,"activation_count":capture.untap_activation_count,"resolution_count":capture.untap_resolution_count,"source_tapped_for_cost":capture.minion_tapped_for_untap_cost,"source_incarnation":capture.minion_incarnation_at_untap},
            "lose_defender":{"ability":DEFENDER_ABILITY_ID,"cost":"{4}","paid_generic_cost":"{4}","paid_activations":capture.paid_defender_ability_count,"resolutions":capture.defender_loss_resolution_count,"removed_live":capture.defender_removed_live,"source_untapped_after_resolution":capture.minion_untapped_after_defender_resolution,"effect_expired":capture.defender_effect_expired,"defender_restored_after_expiration":capture.defender_restored_after_expiration,"source_incarnation":capture.minion_incarnation_at_defender_loss},
            "combat":{"minion_attack_count":capture.minion_attack_count,"four_damage_count":capture.minion_four_damage_count,"terminal_power":capture.final_power,"terminal_toughness":capture.final_toughness,"terminal_has_defender":capture.final_has_defender,"life_at_terminal":capture.life},
            "terminal_pilot_win":true
        },
        "ordered_event_sequence":ordered,
        "events":projected,
        "passed":true
    }))
}

fn unique_event(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    let mut matches = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let index = matches.next()?;
    matches.next().is_none().then_some(index)
}

fn event_after(
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

fn validate_candidate_contract() -> Result<(), String> {
    let card = candidate_tidewater_minion::definition();
    if card.id != CARD_ID
        || card.name != CARD_NAME
        || card.set_code != "RAV"
        || card.mana_cost != ManaCost::with_colors(3, [Color::Blue, Color::Blue])
        || card.colors != BTreeSet::from([Color::Blue])
        || card.card_types != BTreeSet::from([CardType::Creature])
        || card.is_basic_land
        || card.power != Some(4)
        || card.toughness != Some(4)
        || card.keywords != [Keyword::Defender]
        || !card.mana_colors.is_empty()
        || !card.effects.is_empty()
        || card.supported_rules
            != [
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "defender",
                "tap-untap-target-permanent-and-four-lose-defender",
            ]
    {
        return Err(
            "Tidewater Minion RAV #71 definition differs from {3}{U}{U} 4/4 Defender".to_owned(),
        );
    }
    let untap_binding = candidate_tidewater_minion::untap_target_permanent_ability();
    if untap_binding.card_definition != CARD_ID {
        return Err("Tidewater Minion tap ability is bound to the wrong card".to_owned());
    }
    let untap = untap_binding.ability;
    if untap.id != UNTAP_ABILITY_ID
        || untap.mana_cost != ManaCost::new(0)
        || !untap.tap_cost
        || untap.sorcery_speed
        || untap.additional_tap_creatures != 0
        || untap.sacrifice_source
        || untap.sacrifice_creatures != 0
        || untap.sacrifice_lands != 0
        || untap.discard_cards != 0
        || untap.targets != [TargetRequirement::Permanent]
        || untap.effects != [Effect::UntapTargetPermanent]
    {
        return Err("Tidewater Minion must bind {T}: untap target permanent".to_owned());
    }
    let defender_binding = candidate_tidewater_minion::lose_defender_ability();
    if defender_binding.card_definition != CARD_ID {
        return Err("Tidewater Minion Defender ability is bound to the wrong card".to_owned());
    }
    let defender = defender_binding.ability;
    if defender.id != DEFENDER_ABILITY_ID
        || defender.mana_cost != ManaCost::new(4)
        || defender.tap_cost
        || defender.sorcery_speed
        || defender.additional_tap_creatures != 0
        || defender.sacrifice_source
        || defender.sacrifice_creatures != 0
        || defender.sacrifice_lands != 0
        || defender.discard_cards != 0
        || !defender.targets.is_empty()
        || defender.effects
            != [Effect::RemoveSourceKeywordUntilEndOfTurn {
                keyword: Keyword::Defender,
            }]
    {
        return Err("Tidewater Minion must bind {4}: lose Defender until end of turn".to_owned());
    }
    Ok(())
}

fn validate_registry_delegation() -> Result<(), String> {
    let candidate = candidate_tidewater_minion::definition();
    if card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .as_ref()
        != Some(&candidate)
    {
        return Err(
            "Tidewater Minion registry does not delegate its definition to the candidate module"
                .to_owned(),
        );
    }
    let mut expected = vec![
        candidate_tidewater_minion::untap_target_permanent_ability(),
        candidate_tidewater_minion::lose_defender_ability(),
    ];
    expected.sort_by_key(|binding| binding.ability.id);
    let mut registered: Vec<_> = rav_activated_ability_bindings()
        .into_iter()
        .filter(|binding| binding.card_definition == CARD_ID)
        .collect();
    registered.sort_by_key(|binding| binding.ability.id);
    if registered != expected {
        return Err(
            "Tidewater Minion registry abilities do not delegate both printed activated abilities"
                .to_owned(),
        );
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Tidewater Minion candidate: {error}"))?;
    validate_source_shape(&source)
}

fn validate_source_shape(source: &str) -> Result<(), String> {
    let file = syn::parse_file(source)
        .map_err(|_| "Tidewater Minion candidate source is not valid Rust")?;
    if !file.attrs.iter().all(|attribute| {
        attribute.path().is_ident("doc") && matches!(attribute.meta, Meta::NameValue(_))
    }) {
        return Err("Tidewater Minion module has an unsupported attribute".to_owned());
    }
    let expected_imports: BTreeSet<Vec<String>> = [
        vec!["std", "collections", "BTreeSet"],
        vec!["cardbench_magic_engine", "ActivatedAbility"],
        vec!["cardbench_magic_engine", "ActivatedAbilityBinding"],
        vec!["cardbench_magic_engine", "CardDefinition"],
        vec!["cardbench_magic_engine", "CardType"],
        vec!["cardbench_magic_engine", "Color"],
        vec!["cardbench_magic_engine", "Effect"],
        vec!["cardbench_magic_engine", "Keyword"],
        vec!["cardbench_magic_engine", "ManaCost"],
        vec!["cardbench_magic_engine", "TargetRequirement"],
    ]
    .into_iter()
    .map(|path| path.into_iter().map(str::to_owned).collect())
    .collect();
    let mut imports = BTreeSet::new();
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(item.vis, Visibility::Inherited) =>
            {
                imports.extend(flattened_use_paths(item).ok_or("unsupported Tidewater import")?);
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                let return_type = match name.as_str() {
                    "definition" => "CardDefinition",
                    "untap_target_permanent_ability" | "lose_defender_ability" => {
                        "ActivatedAbilityBinding"
                    }
                    _ => return Err("Tidewater module exposes an unsupported function".to_owned()),
                };
                if !functions.insert(name.clone())
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || !function.sig.inputs.is_empty()
                    || !function.sig.generics.params.is_empty()
                    || function.sig.variadic.is_some()
                    || function.block.stmts.len() != 1
                    || !matches!(&function.sig.output, ReturnType::Type(_, ty) if matches!(&**ty, syn::Type::Path(path) if path.path.is_ident(return_type)))
                    || !function.attrs.iter().all(|attribute| {
                        (attribute.path().is_ident("doc")
                            && matches!(attribute.meta, Meta::NameValue(_)))
                            || (attribute.path().is_ident("must_use")
                                && matches!(attribute.meta, Meta::Path(_)))
                    })
                {
                    return Err(
                        "Tidewater function signature/attributes are outside the source contract"
                            .to_owned(),
                    );
                }
                let Stmt::Expr(Expr::Struct(literal), None) = &function.block.stmts[0] else {
                    return Err(
                        "Tidewater candidate factories must directly return struct literals"
                            .to_owned(),
                    );
                };
                if literal
                    .path
                    .segments
                    .last()
                    .is_none_or(|segment| segment.ident != return_type)
                    || literal.rest.is_some()
                    || !declarative_expr(&Expr::Struct(literal.clone()))
                {
                    return Err(
                        "Tidewater candidate literal contains executable or unsupported code"
                            .to_owned(),
                    );
                }
                let expected_fields: BTreeSet<&str> = if name == "definition" {
                    [
                        "id",
                        "name",
                        "set_code",
                        "mana_cost",
                        "colors",
                        "mana_colors",
                        "card_types",
                        "is_basic_land",
                        "supported_rules",
                        "power",
                        "toughness",
                        "keywords",
                        "effects",
                    ]
                    .into_iter()
                    .collect()
                } else if name == "untap_target_permanent_ability"
                    || name == "lose_defender_ability"
                {
                    ["card_definition", "ability"].into_iter().collect()
                } else {
                    unreachable!()
                };
                let fields: BTreeSet<String> = literal
                    .fields
                    .iter()
                    .filter_map(|field| match &field.member {
                        syn::Member::Named(name) => Some(name.to_string()),
                        _ => None,
                    })
                    .collect();
                if fields.len() != literal.fields.len()
                    || fields.iter().map(String::as_str).collect::<BTreeSet<_>>() != expected_fields
                {
                    return Err(
                        "Tidewater candidate literal fields differ from the pinned schema"
                            .to_owned(),
                    );
                }
            }
            _ => {
                return Err(
                    "Tidewater source permits only imports and three factory functions".to_owned(),
                );
            }
        }
    }
    if imports != expected_imports
        || functions
            != BTreeSet::from([
                "definition".to_owned(),
                "untap_target_permanent_ability".to_owned(),
                "lose_defender_ability".to_owned(),
            ])
    {
        return Err(
            "Tidewater candidate import/function shape differs from the pinned schema".to_owned(),
        );
    }
    Ok(())
}

fn declarative_expr(expression: &Expr) -> bool {
    match expression {
        Expr::Lit(_) | Expr::Path(_) => true,
        Expr::Reference(reference) => declarative_expr(&reference.expr),
        Expr::Array(array) => array.elems.iter().all(declarative_expr),
        Expr::Struct(value) => {
            matches!(
                value
                    .path
                    .segments
                    .last()
                    .map(|segment| segment.ident.to_string())
                    .as_deref(),
                Some(
                    "CardDefinition"
                        | "ActivatedAbilityBinding"
                        | "ActivatedAbility"
                        | "RemoveSourceKeywordUntilEndOfTurn"
                )
            ) && value.attrs.is_empty()
                && value.rest.is_none()
                && value.fields.iter().all(|field| {
                    matches!(field.member, syn::Member::Named(_)) && declarative_expr(&field.expr)
                })
        }
        Expr::Call(call) => {
            let allowed = match &*call.func {
                Expr::Path(path) if path.qself.is_none() => {
                    let names: Vec<_> = path
                        .path
                        .segments
                        .iter()
                        .map(|segment| segment.ident.to_string())
                        .collect();
                    matches!(names.as_slice(), [owner,method] if (owner == "BTreeSet" && (method == "from" || method == "new")) || (owner == "ManaCost" && (method == "new" || method == "with_colors")))
                        || matches!(names.as_slice(), [name] if name == "Some")
                }
                _ => false,
            };
            allowed && call.args.iter().all(declarative_expr)
        }
        Expr::Macro(mac) => {
            let parser = syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated;
            mac.mac.path.is_ident("vec")
                && parser
                    .parse2(mac.mac.tokens.clone())
                    .is_ok_and(|values| values.iter().all(declarative_expr))
        }
        _ => false,
    }
}

fn flattened_use_paths(item: &syn::ItemUse) -> Option<Vec<Vec<String>>> {
    if item.leading_colon.is_some() {
        return None;
    }
    let mut paths = Vec::new();
    flatten_use_tree(&[], &item.tree, &mut paths).then_some(paths)
}

fn flatten_use_tree(prefix: &[String], tree: &UseTree, paths: &mut Vec<Vec<String>>) -> bool {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix.to_vec();
            next.push(path.ident.to_string());
            flatten_use_tree(&next, &path.tree, paths)
        }
        UseTree::Name(name) => {
            let mut next = prefix.to_vec();
            next.push(name.ident.to_string());
            paths.push(next);
            true
        }
        UseTree::Group(group) => group
            .items
            .iter()
            .all(|item| flatten_use_tree(prefix, item, paths)),
        _ => false,
    }
}

fn validate_side_effect_mutation() -> Result<Value, String> {
    let source = std::fs::read_to_string(SOURCE_PATH).map_err(|error| error.to_string())?;
    let function = source
        .find("fn definition")
        .ok_or("definition factory missing")?;
    let open = source[function..]
        .find('{')
        .map(|offset| function + offset + 1)
        .ok_or("definition body missing")?;
    let injected = format!(
        "{}\n    let _ = std::fs::write(\"/tmp/cardbench-tidewater-candidate-executed\", b\"ran\");{}",
        &source[..open],
        &source[open..]
    );
    syn::parse_file(&injected).map_err(|_| "side-effect mutation must remain parse-valid Rust")?;
    if validate_source_shape(&injected).is_ok() {
        return Err(
            "Tidewater AST guard accepted the side-effecting candidate mutation".to_owned(),
        );
    }
    Ok(json!({
        "normalized_ast_schema_checked_before_candidate_calls":true,
        "side_effect_mutation_parse_valid":true,
        "side_effect_mutation_compile_valid_by_construction":true,
        "side_effect_mutation_rejected":true,
        "candidate_calls_before_guard":0,
        "rejection_stage":"source-shape-and-declarative-AST-before-candidate-function-invocation"
    }))
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
        || !cards
            .iter()
            .any(|card| card.collector_number == 71 && card.name == CARD_NAME)
    {
        return Err("RAV catalog identity/denominator differs for Tidewater Minion".to_owned());
    }
    Ok(cards)
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        cardbench_magic_rav::CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => {
            Some(definition_id)
        }
        cardbench_magic_rav::CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({"collector_number":card.collector_number,"definition_id":definition_id,"name":card.name,"trace_status":if passed{"passed"}else{"not_qualified"},"qualification":if passed{"pilot_trace_passed; independent review pending"}else{"not_evaluated"},"credited":false})
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}
