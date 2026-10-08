//! Full-game RAV Copy Enchantment authority for replacement-style entry copy.

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DecisionKind, DecisionSelection, DeckEntry,
    DeckList, EntryCopyBinding, Game, GameEvent, GameView, ManaCost, ManaPaymentSelection,
    ObjectId, PlayerId, PolicyAction, Step, Target, TriggeredAbilityBinding, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings,
    rav_attachment_triggered_ability_bindings as attachment_triggers, rav_basic_land_type_bindings,
    rav_entry_copy_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::{Item, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-COPY-ENCHANTMENT";
const CARD_NAME: &str = "Copy Enchantment";
const SOURCE_AURA_ID: &str = "RAV-FISTS-OF-IRONWOOD";
const TARGET_CREATURE_ID: &str = "RAV-WATCHWOLF";
const COPY_COST: &str = "{2}{U}";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/copy_enchantment.rs";
const MAX_TRACE_SEED: u64 = 64;
const MAX_TURNS: u32 = 60;
const MAX_POLICY_MOVES: u32 = 12_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/copy_enchantment.rs"]
mod candidate_copy_enchantment;

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
            eprintln!("Copy Enchantment PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongCastCost,
    WrongBaseType,
    MissingEntryCopyBinding,
    WrongCopyableType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CopyReceipt {
    sequence: usize,
    source: ObjectId,
    source_incarnation: u64,
    target: ObjectId,
    target_incarnation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    watchwolf: Option<ObjectId>,
    source_aura: Option<ObjectId>,
    copies: Vec<ObjectId>,
    payments: BTreeMap<ObjectId, Vec<Color>>,
    copy_spell_casts: Vec<usize>,
    copy_spell_resolutions: BTreeMap<ObjectId, usize>,
    source_decisions_opened: Vec<usize>,
    source_decisions_completed: Vec<usize>,
    aura_decisions_opened: Vec<usize>,
    aura_decisions_completed: Vec<usize>,
    source_choice_selections: Vec<Vec<ObjectId>>,
    aura_choice_selections: Vec<Vec<ObjectId>>,
    permanent_copies: Vec<CopyReceipt>,
    copy_entries: BTreeMap<ObjectId, (usize, String)>,
    trigger_stack_sequences: Vec<usize>,
    trigger_resolve_sequences: Vec<usize>,
    token_sequences: Vec<usize>,
    watchwolf_attacks: Vec<usize>,
    watchwolf_damage: Vec<(usize, i32)>,
    terminal_sequence: Option<usize>,
    copy_definitions: Vec<Option<String>>,
    copy_attachments: Vec<Option<ObjectId>>,
    source_attachment: Option<ObjectId>,
    watchwolf_trample: bool,
    watchwolf_power: Option<i32>,
    watchwolf_toughness: Option<i32>,
    zones: BTreeMap<ObjectId, Option<Zone>>,
    source_incarnation: Option<u64>,
    copy_incarnations: Vec<u64>,
    life: [i64; 2],
}

fn run_authority() -> Result<Value, String> {
    let source_validation = validate_candidate_source()?;
    validate_candidate_definition()?;
    let catalog = unique_catalog()?;
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured != replay {
        return Err("Copy Enchantment full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "printed_cast_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongCastCost),
        "printed_enchantment_type_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongBaseType),
        "entry_copy_binding_removal_detected_in_game": mutation_detected(seed, Mutation::MissingEntryCopyBinding),
        "wrong_copyable_type_scope_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongCopyableType),
        "sealed_mutation_authority": false
    });
    for key in [
        "printed_cast_cost_mutation_detected_in_game",
        "printed_enchantment_type_mutation_detected_in_game",
        "entry_copy_binding_removal_detected_in_game",
        "wrong_copyable_type_scope_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!("Copy Enchantment gameplay mutation escaped: {key}"));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    let printings = rav_main_set_catalog();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 42, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "authority_cutoff": "2026-09-24",
            "source_contract": "RAV #42 is a blue {2}{U} Enchantment. As it enters, its controller may choose any enchantment already on the battlefield and have Copy Enchantment enter as its copy. If the chosen enchantment is an Aura, the copied Aura makes its ordinary attachment choice before entering. Current Oracle uses enter/this-enchantment wording for the same replacement-style choice.",
            "checked_sections": ["110.5 Copyable values", "303.4 Aura attachment", "400.7 Zone changes and new objects", "601.2 Casting spells", "614 Replacement effects", "614.12 As-enters replacement effects"]
        },
        "candidate_source_validation": source_validation,
        "passed": true,
        "pilot_card_passed": true,
        "replay_deterministic": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
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

fn validate_candidate_source() -> Result<Value, String> {
    let sentinel = "/tmp/cardbench-copy-enchantment-candidate-executed";
    let _ = std::fs::remove_file(sentinel);
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Copy Enchantment source: {error}"))?;
    validate_candidate_source_text(&source)?;
    let injected = source.replacen(
        "pub(crate) fn definition() -> CardDefinition {",
        "pub(crate) fn definition() -> CardDefinition { let _ = std::fs::write(\"/tmp/cardbench-copy-enchantment-candidate-executed\", b\"ran\");",
        1,
    );
    if injected == source || syn::parse_file(&injected).is_err() {
        return Err("compile-valid Copy Enchantment injection fixture is invalid Rust".to_owned());
    }
    if validate_candidate_source_text(&injected).is_ok() {
        return Err("Copy Enchantment source guard accepted an injected side effect".to_owned());
    }
    if std::path::Path::new(sentinel).exists() {
        return Err("Copy Enchantment injected side effect executed before rejection".to_owned());
    }
    Ok(json!({
        "normalized_literal_source_and_ast_checked_before_candidate_calls": true,
        "compile_valid_injected_side_effect_fixture_parsed": true,
        "compile_valid_injected_side_effect_candidate_rejected": true,
        "injected_candidate_calls_before_rejection": 0,
        "side_effect_sentinel_absent_after_rejection": true,
        "guard_ran_before_candidate_function_calls": true
    }))
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let normalized = source.replace("\r\n", "\n");
    if normalized != EXPECTED_CANDIDATE_SOURCE {
        return Err(
            "Copy Enchantment source differs from the pinned literal RAV module".to_owned(),
        );
    }
    let parsed = syn::parse_file(&normalized)
        .map_err(|_| "Copy Enchantment source is not valid Rust".to_owned())?;
    if parsed.shebang.is_some() || !parsed.attrs.iter().all(|attr| attr.path().is_ident("doc")) {
        return Err("Copy Enchantment source contains unsupported module-level syntax".to_owned());
    }
    let mut functions = BTreeSet::new();
    for item in parsed.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(item.vis, Visibility::Inherited) => {}
            Item::Fn(function)
                if matches!(function.vis, Visibility::Restricted(_))
                    && function.sig.asyncness.is_none()
                    && function.sig.unsafety.is_none()
                    && function.sig.inputs.is_empty()
                    && functions.insert(function.sig.ident.to_string()) => {}
            _ => {
                return Err(
                    "Copy Enchantment source contains executable or unsupported top-level syntax"
                        .to_owned(),
                );
            }
        }
    }
    if functions != BTreeSet::from(["definition".to_owned()]) {
        return Err("Copy Enchantment module must expose only its definition factory".to_owned());
    }
    Ok(())
}

const EXPECTED_CANDIDATE_SOURCE: &str = r###"//! Candidate-owned RAV Copy Enchantment card definition.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, ManaCost};

/// RAV #42 is a blue `{2}{U}` enchantment. Its entry-copy choice is wired
/// through the set's typed `EntryCopyBinding` registry.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-COPY-ENCHANTMENT",
        name: "Copy Enchantment",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "may-enter-as-copy-of-enchantment",
            "public-entry-copy-decision",
            "copied-aura-entry-attachment",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}
"###;

fn validate_candidate_definition() -> Result<(), String> {
    let definition = candidate_copy_enchantment::definition();
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(2, [Color::Blue])
        || definition.colors != BTreeSet::from([Color::Blue])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Enchantment])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
        || definition.supported_rules
            != [
                "full-rules-fidelity",
                "may-enter-as-copy-of-enchantment",
                "public-entry-copy-decision",
                "copied-aura-entry-attachment",
            ]
    {
        return Err(
            "Copy Enchantment definition differs from the RAV #42 printed contract".to_owned(),
        );
    }
    Ok(())
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
            .any(|card| card.collector_number == 42 && card.name == CARD_NAME)
    {
        return Err("RAV catalog identity or denominator differs for Copy Enchantment".to_owned());
    }
    Ok(cards)
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_COPY_ENCHANTMENT_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_COPY_ENCHANTMENT_SEED must be an unsigned integer")?;
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
        "no full-game Copy Enchantment trace found in seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation).and_then(|capture| validate_trace(&capture, seed)) {
        Ok(_) => false,
        Err(_) => true,
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let mut definition = candidate_copy_enchantment::definition();
    let mut entry_copy_bindings: Vec<EntryCopyBinding> = rav_entry_copy_bindings();
    match mutation {
        Mutation::None => {}
        Mutation::WrongCastCost => definition.mana_cost = ManaCost::with_colors(1, [Color::Blue]),
        Mutation::WrongBaseType => definition.card_types = BTreeSet::from([CardType::Artifact]),
        Mutation::MissingEntryCopyBinding => {
            entry_copy_bindings.retain(|binding| binding.card_definition != CARD_ID)
        }
        Mutation::WrongCopyableType => {
            if let Some(binding) = entry_copy_bindings
                .iter_mut()
                .find(|binding| binding.card_definition == CARD_ID)
            {
                binding.copyable_type = CardType::Creature;
            }
        }
    }
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Copy Enchantment is missing from the RAV definition registry")? = definition;
    let mut triggers = rav_triggered_ability_bindings();
    triggers.extend(attachment_triggers());
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
    game.register_attachment_bindings(rav_attachment_bindings())
        .map_err(|error| error.to_string())?;
    game.register_entry_copy_bindings(entry_copy_bindings)
        .map_err(|error| error.to_string())?;
    let p0 = DeckList {
        mainboard: vec![
            entry(TARGET_CREATURE_ID, 4),
            entry(SOURCE_AURA_ID, 4),
            entry(CARD_ID, 4),
            entry("RAV-FOREST", 16),
            entry("RAV-PLAINS", 16),
            entry("RAV-ISLAND", 16),
        ],
        sideboard: vec![],
    };
    let p1 = DeckList {
        mainboard: vec![entry("RAV-MOUNTAIN", 60)],
        sideboard: vec![],
    };
    if deck_size(&p0) != 60 || deck_size(&p1) != 60 {
        return Err(
            "Copy Enchantment constructed decks must contain exactly sixty cards".to_owned(),
        );
    }
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &p0)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &p1)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), 7)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), 7)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut pilots = [CopyPilot::new(PlayerId(0)), CopyPilot::new(PlayerId(1))];
    let mut captured = CapturedMatch {
        events: vec![],
        winner: None,
        turns: 0,
        accepted_moves: 0,
        rejected_moves: 0,
        watchwolf: None,
        source_aura: None,
        copies: vec![],
        payments: BTreeMap::new(),
        copy_spell_casts: vec![],
        copy_spell_resolutions: BTreeMap::new(),
        source_decisions_opened: vec![],
        source_decisions_completed: vec![],
        aura_decisions_opened: vec![],
        aura_decisions_completed: vec![],
        permanent_copies: vec![],
        source_choice_selections: vec![],
        aura_choice_selections: vec![],
        copy_entries: BTreeMap::new(),
        trigger_stack_sequences: vec![],
        trigger_resolve_sequences: vec![],
        token_sequences: vec![],
        watchwolf_attacks: vec![],
        watchwolf_damage: vec![],
        terminal_sequence: None,
        copy_definitions: vec![],
        copy_attachments: vec![],
        source_attachment: None,
        watchwolf_trample: false,
        watchwolf_power: None,
        watchwolf_toughness: None,
        zones: BTreeMap::new(),
        life: [20, 20],
        source_incarnation: None,
        copy_incarnations: vec![],
    };
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || captured.accepted_moves >= MAX_POLICY_MOVES {
            return Err("Copy Enchantment duel exceeded its bounded horizon".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let pilot = pilots
            .get_mut(player.0)
            .ok_or("Copy Enchantment policy seat is missing")?;
        let action = pilot
            .propose_pending_decision(&view)
            .unwrap_or_else(|| pilot.propose_move(&view));
        let before = game.event_log.len();
        game.submit_policy_move(player, pilot.id(), action)
            .map_err(|error| {
                format!(
                    "Copy Enchantment policy action rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        captured.accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("Copy Enchantment invariant failed: {error}"))?;
        observe(
            &game,
            &game.event_log[before..],
            captured.events.len(),
            &mut captured,
        )?;
        captured.events.extend_from_slice(&game.event_log[before..]);
    }
    captured.winner = game.winner().map(|player| player.0);
    captured.turns = game.turn;
    captured.terminal_sequence = captured.events.iter().position(|event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    });
    for player in 0..2 {
        captured.life[player] = game
            .player(PlayerId(player))
            .map_err(|error| error.to_string())?
            .life;
    }
    captured.source_choice_selections = pilots[0].source_choice_selections.clone();
    captured.aura_choice_selections = pilots[0].aura_choice_selections.clone();
    let watchwolf = captured.watchwolf.ok_or("Watchwolf target was not cast")?;
    let source = captured
        .source_aura
        .ok_or("Fists of Ironwood was not cast")?;
    captured.source_incarnation = game.object(source).ok().map(|object| object.incarnation);
    captured.source_attachment = game
        .object(source)
        .ok()
        .and_then(|object| object.attached_to);
    let live = game
        .characteristics(watchwolf)
        .map_err(|error| error.to_string())?;
    captured.watchwolf_trample = live
        .keywords
        .contains(&cardbench_magic_engine::Keyword::Trample);
    captured.watchwolf_power = live.power;
    captured.watchwolf_toughness = live.toughness;
    captured.zones.insert(watchwolf, game.zone_of(watchwolf));
    captured.zones.insert(source, game.zone_of(source));
    for copy in &captured.copies {
        captured.zones.insert(*copy, game.zone_of(*copy));
        captured.copy_incarnations.push(
            game.object(*copy)
                .ok()
                .map_or(0, |object| object.incarnation),
        );
        captured.copy_definitions.push(
            game.card_definition(*copy)
                .ok()
                .map(|definition| definition.id.to_owned()),
        );
        captured.copy_attachments.push(
            game.object(*copy)
                .ok()
                .and_then(|object| object.attached_to),
        );
    }
    Ok(captured)
}

struct CopyPilot {
    player: PlayerId,
    copies_cast: usize,
    source_decisions: usize,
    source_choice_selections: Vec<Vec<ObjectId>>,
    aura_choice_selections: Vec<Vec<ObjectId>>,
}

impl CopyPilot {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            copies_cast: 0,
            source_decisions: 0,
            source_choice_selections: vec![],
            aura_choice_selections: vec![],
        }
    }

    fn cast(
        &self,
        card: ObjectId,
        cost: &ManaCost,
        view: &GameView,
        targets: Vec<Target>,
    ) -> Option<PolicyAction> {
        if let Some(mana_selection) = payment(view, cost) {
            Some(PolicyAction::CastWithPayment {
                request: CastRequest {
                    card,
                    targets,
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection,
            })
        } else {
            make_mana(view, cost)
        }
    }
}

impl CodePolicy for CopyPilot {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.copy-enchantment-aura-copy-and-decline.v1"
        } else {
            "rav.copy-enchantment-noninterference.v1"
        }
    }

    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        if view.player != self.player || self.player != PlayerId(0) {
            return None;
        }
        let decision = view.pending_decision.as_ref()?;
        match decision.kind {
            DecisionKind::PermanentEntryCopySource => {
                let selected = if self.source_decisions == 0 {
                    decision
                        .candidates
                        .iter()
                        .find(|candidate| candidate.definition == Some(SOURCE_AURA_ID))
                        .map(|candidate| vec![candidate.id])
                        .unwrap_or_default()
                } else {
                    vec![]
                };
                self.source_decisions += 1;
                self.source_choice_selections.push(selected.clone());
                Some(PolicyAction::SubmitDecision {
                    decision: decision.id,
                    selection: DecisionSelection::Objects(selected),
                })
            }
            DecisionKind::PermanentEntryCopyAuraAttachment => {
                let selected = decision
                    .candidates
                    .iter()
                    .find(|candidate| candidate.definition == Some(TARGET_CREATURE_ID))
                    .map(|candidate| vec![candidate.id])
                    .unwrap_or_default();
                self.aura_choice_selections.push(selected.clone());
                Some(PolicyAction::SubmitDecision {
                    decision: decision.id,
                    selection: DecisionSelection::Objects(selected),
                })
            }
            _ => None,
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
            return PolicyAction::DeclareAttackers {
                attackers: if self.player == PlayerId(0)
                    && self.copies_cast >= 2
                    && self.source_decisions >= 2
                {
                    view.own_battlefield
                        .iter()
                        .filter(|permanent| permanent.can_attack)
                        .map(|permanent| permanent.id)
                        .collect()
                } else {
                    vec![]
                },
            };
        }
        if view.active_player != self.player
            || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            let preferred = match self.player {
                PlayerId(0) => ["RAV-FOREST", "RAV-ISLAND", "RAV-PLAINS"].as_slice(),
                _ => ["RAV-MOUNTAIN"].as_slice(),
            };
            if let Some(land) = view.hand.iter().find(|card| {
                card.definition
                    .is_some_and(|definition| preferred.contains(&definition))
            }) {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if self.player != PlayerId(0) {
            return PolicyAction::PassPriority;
        }
        let creature = view
            .own_battlefield
            .iter()
            .find(|permanent| permanent.definition == Some(TARGET_CREATURE_ID));
        if creature.is_none()
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(TARGET_CREATURE_ID))
            && let Some(action) = self.cast(
                card.id,
                &ManaCost::with_colors(0, [Color::Green, Color::White]),
                view,
                vec![],
            )
        {
            return action;
        }
        let aura = view
            .own_battlefield
            .iter()
            .find(|permanent| permanent.definition == Some(SOURCE_AURA_ID));
        if aura.is_none()
            && let Some(creature) = creature
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(SOURCE_AURA_ID))
            && let Some(action) = self.cast(
                card.id,
                &ManaCost::with_colors(1, [Color::Green]),
                view,
                vec![Target::Permanent(creature.id)],
            )
        {
            return action;
        }
        if aura.is_some()
            && self.copies_cast < 2
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            && let Some(action) = self.cast(
                card.id,
                &ManaCost::with_colors(2, [Color::Blue]),
                view,
                vec![],
            )
        {
            if matches!(action, PolicyAction::CastWithPayment { .. }) {
                self.copies_cast += 1;
            }
            return action;
        }
        PolicyAction::PassPriority
    }
}

fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
    let mut available: BTreeMap<Color, u8> = Color::MANA_ALL
        .into_iter()
        .map(|color| (color, view.mana_pool.amount(color)))
        .collect();
    for color in &cost.colored {
        let amount = available.get_mut(color)?;
        if *amount == 0 {
            return None;
        }
        *amount -= 1;
    }
    let mut generic = Vec::new();
    for _ in 0..cost.generic {
        let color = Color::MANA_ALL
            .into_iter()
            .find(|color| available.get(color).copied().unwrap_or_default() > 0)?;
        *available.get_mut(&color)? -= 1;
        generic.push(color);
    }
    Some(ManaPaymentSelection {
        generic,
        hybrid: vec![],
    })
}

fn make_mana(view: &GameView, cost: &ManaCost) -> Option<PolicyAction> {
    if payment(view, cost).is_some() {
        return None;
    }
    let desired = cost
        .colored
        .iter()
        .copied()
        .find(|color| view.mana_pool.amount(*color) == 0)
        .or_else(|| cost.colored.first().copied())?;
    let land = view.own_battlefield.iter().find(|permanent| {
        permanent.card_types.contains(&CardType::Land)
            && !permanent.tapped
            && permanent.mana_colors.contains(&desired)
    })?;
    Some(PolicyAction::ActivateManaAbility {
        land: land.id,
        color: desired,
    })
}

fn observe(
    game: &Game,
    emitted: &[GameEvent],
    base: usize,
    captured: &mut CapturedMatch,
) -> Result<(), String> {
    for (offset, event) in emitted.iter().enumerate() {
        let sequence = base + offset;
        match event {
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } => {
                let definition = game
                    .card_definition(*card)
                    .map_err(|error| error.to_string())?
                    .id;
                if definition == TARGET_CREATURE_ID {
                    captured.watchwolf = Some(*card);
                } else if definition == SOURCE_AURA_ID {
                    captured.source_aura = Some(*card);
                } else if definition == CARD_ID {
                    captured.copies.push(*card);
                    captured.copy_spell_casts.push(sequence);
                }
            }
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } => {
                captured.payments.insert(*card, colors.clone());
            }
            GameEvent::SpellResolved { card } if captured.copies.contains(card) => {
                captured.copy_spell_resolutions.insert(*card, sequence);
            }
            GameEvent::DecisionOpened {
                kind: DecisionKind::PermanentEntryCopySource,
                ..
            } => captured.source_decisions_opened.push(sequence),
            GameEvent::DecisionCompleted {
                kind: DecisionKind::PermanentEntryCopySource,
                ..
            } => captured.source_decisions_completed.push(sequence),
            GameEvent::DecisionOpened {
                kind: DecisionKind::PermanentEntryCopyAuraAttachment,
                ..
            } => captured.aura_decisions_opened.push(sequence),
            GameEvent::DecisionCompleted {
                kind: DecisionKind::PermanentEntryCopyAuraAttachment,
                ..
            } => captured.aura_decisions_completed.push(sequence),
            GameEvent::PermanentCopied {
                source,
                source_incarnation,
                target,
                target_incarnation,
                ..
            } => captured.permanent_copies.push(CopyReceipt {
                sequence,
                source: *source,
                source_incarnation: *source_incarnation,
                target: *target,
                target_incarnation: *target_incarnation,
            }),
            GameEvent::CardMoved {
                card,
                to: Zone::Battlefield,
            } if captured.copies.contains(card) => {
                let definition = game
                    .card_definition(*card)
                    .map_err(|error| error.to_string())?
                    .id
                    .to_owned();
                captured.copy_entries.insert(*card, (sequence, definition));
            }
            GameEvent::TriggeredAbilityStacked {
                source,
                ability: "etb-two-saprolings",
                ..
            } if game
                .card_definition(*source)
                .is_ok_and(|definition| definition.id == SOURCE_AURA_ID) =>
            {
                captured.trigger_stack_sequences.push(sequence)
            }
            GameEvent::AbilityResolved {
                source,
                ability: "etb-two-saprolings",
                ..
            } if game
                .card_definition(*source)
                .is_ok_and(|definition| definition.id == SOURCE_AURA_ID) =>
            {
                captured.trigger_resolve_sequences.push(sequence)
            }
            GameEvent::TokenCreated {
                player: PlayerId(0),
                ..
            } => captured.token_sequences.push(sequence),
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } if captured
                .watchwolf
                .is_some_and(|watchwolf| attackers.contains(&watchwolf)) =>
            {
                captured.watchwolf_attacks.push(sequence)
            }
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if captured.watchwolf == Some(*source) => {
                captured.watchwolf_damage.push((sequence, *amount))
            }
            GameEvent::GameEnded {
                winner: Some(PlayerId(0)),
            } => captured.terminal_sequence = Some(sequence),
            _ => {}
        }
    }
    Ok(())
}

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if captured.winner != Some(0)
        || captured.turns == 0
        || captured.turns > MAX_TURNS
        || captured.rejected_moves != 0
        || captured.terminal_sequence.is_none()
        || captured.life[0] <= 0
        || captured.life[1] != 0
    {
        return Err(format!(
            "Copy Enchantment did not produce a bounded P0 terminal win: winner={:?}, life={:?}, turns={}",
            captured.winner, captured.life, captured.turns
        ));
    }
    let watchwolf = captured.watchwolf.ok_or("Watchwolf was not cast")?;
    let source = captured
        .source_aura
        .ok_or("Fists of Ironwood was not cast")?;
    if captured.copies.len() != 2
        || captured.copy_spell_casts.len() != 2
        || captured.copy_spell_resolutions.len() != 2
        || captured.source_decisions_opened.len() != 2
        || captured.source_decisions_completed.len() != 2
        || captured.aura_decisions_opened.len() != 1
        || captured.aura_decisions_completed.len() != 1
        || captured.source_choice_selections.len() != 2
        || captured.source_choice_selections[0] != [source]
        || !captured.source_choice_selections[1].is_empty()
        || captured.aura_choice_selections != [vec![watchwolf]]
        || captured.permanent_copies.len() != 1
        || captured.copy_definitions != [Some(SOURCE_AURA_ID.to_owned()), Some(CARD_ID.to_owned())]
        || captured.copy_attachments != [Some(watchwolf), None]
        || captured.source_attachment != Some(watchwolf)
    {
        return Err(format!(
            "Copy Enchantment did not cover selected Aura copy, copied-Aura attachment, and declined-copy branches: copies={:?}, source_decisions={:?}/{:?}, aura_decisions={:?}/{:?}, selections={:?}/{:?}, definitions={:?}, attachments={:?}",
            captured.copies,
            captured.source_decisions_opened,
            captured.source_decisions_completed,
            captured.aura_decisions_opened,
            captured.aura_decisions_completed,
            captured.source_choice_selections,
            captured.aura_choice_selections,
            captured.copy_definitions,
            captured.copy_attachments,
        ));
    }
    if captured.zones.get(&watchwolf) != Some(&Some(Zone::Battlefield))
        || captured.zones.get(&source) != Some(&Some(Zone::Battlefield))
        || captured
            .copies
            .iter()
            .any(|copy| captured.zones.get(copy) != Some(&Some(Zone::Battlefield)))
        || !captured.watchwolf_trample
        || captured.watchwolf_power != Some(3)
        || captured.watchwolf_toughness != Some(3)
    {
        return Err(
            "the original and copied Fists Auras did not remain attached to Watchwolf with Trample"
                .to_owned(),
        );
    }
    let receipt = &captured.permanent_copies[0];
    let copied_aura = captured.copies[0];
    let declined = captured.copies[1];
    let source_incarnation = captured
        .source_incarnation
        .ok_or("source Aura incarnation is missing")?;
    if receipt.source != source
        || receipt.source_incarnation != source_incarnation
        || receipt.target != copied_aura
        || receipt.target_incarnation != captured.copy_incarnations[0]
    {
        return Err("PermanentCopied receipt does not bind the live Fists source and first Copy Enchantment incarnation".to_owned());
    }
    if captured.trigger_stack_sequences.len() != 2
        || captured.trigger_resolve_sequences.len() != 2
        || captured.token_sequences.len() != 4
    {
        return Err(format!(
            "the copied Aura did not retain its ETB trigger: stacks={}, resolves={}, P0 tokens={}",
            captured.trigger_stack_sequences.len(),
            captured.trigger_resolve_sequences.len(),
            captured.token_sequences.len()
        ));
    }
    let watchwolf_cast = event_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == watchwolf))
        .ok_or("Watchwolf cast receipt is missing")?;
    let watchwolf_paid = event_index(&captured.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == watchwolf && colors.len() == 2))
        .ok_or("Watchwolf exact colored cast payment is missing")?;
    let watchwolf_resolved = event_index_after(
        &captured.events,
        watchwolf_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == watchwolf),
    )
    .ok_or("Watchwolf spell resolution is missing")?;
    let source_cast = event_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == source))
        .ok_or("Fists of Ironwood cast receipt is missing")?;
    let source_paid = event_index(&captured.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == source && colors.len() == 2))
        .ok_or("Fists of Ironwood exact cast payment is missing")?;
    let source_resolved = event_index_after(
        &captured.events,
        source_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == source),
    )
    .ok_or("Fists of Ironwood spell resolution is missing")?;
    let source_entry = event_index_after(&captured.events, source_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == source))
        .ok_or("Fists of Ironwood battlefield entry is missing")?;
    let mut copy_event_orders = Vec::new();
    for (index, copy) in captured.copies.iter().enumerate() {
        let payment = captured
            .payments
            .get(copy)
            .ok_or("Copy Enchantment payment receipt is missing")?;
        if payment.len() != 3
            || payment
                .iter()
                .filter(|color| **color == Color::Blue)
                .count()
                != 1
        {
            return Err(format!(
                "Copy Enchantment #{index} did not pay exact {{2}}{{U}}: {payment:?}"
            ));
        }
        let cast = captured.copy_spell_casts[index];
        let paid = event_index(&captured.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == *copy && colors == payment))
            .ok_or("Copy Enchantment payment event ordering is missing")?;
        let resolved = *captured
            .copy_spell_resolutions
            .get(copy)
            .ok_or("Copy Enchantment resolution receipt is missing")?;
        let entry = captured
            .copy_entries
            .get(copy)
            .map(|(sequence, _)| *sequence)
            .ok_or("Copy Enchantment battlefield entry receipt is missing")?;
        if paid >= cast || cast >= resolved || resolved >= entry {
            return Err(format!(
                "Copy Enchantment #{index} payment/cast/resolution/entry order is invalid"
            ));
        }
        copy_event_orders.push((paid, cast, resolved, entry));
    }
    let copied_entry = captured
        .copy_entries
        .get(&copied_aura)
        .map(|(sequence, _)| *sequence)
        .ok_or("copied Aura entry receipt is missing")?;
    let declined_entry = captured
        .copy_entries
        .get(&declined)
        .map(|(sequence, _)| *sequence)
        .ok_or("declined Copy Enchantment entry receipt is missing")?;
    let first_copy_resolved = copy_event_orders[0].2;
    let second_copy_resolved = copy_event_orders[1].2;
    if !(watchwolf_paid < watchwolf_cast
        && watchwolf_cast < watchwolf_resolved
        && source_paid < source_cast
        && source_cast < source_resolved
        && source_resolved < source_entry
        && source_entry < captured.copy_spell_casts[0]
        && captured.source_decisions_opened[0] < captured.source_decisions_completed[0]
        && captured.source_decisions_completed[0] < captured.aura_decisions_opened[0]
        && captured.aura_decisions_opened[0] < captured.aura_decisions_completed[0]
        && captured.aura_decisions_completed[0] < first_copy_resolved
        && first_copy_resolved < copied_entry
        && copied_entry < receipt.sequence
        && receipt.sequence < captured.copy_spell_casts[1]
        && captured.source_decisions_opened[1] < captured.source_decisions_completed[1]
        && captured.source_decisions_completed[1] < second_copy_resolved
        && second_copy_resolved < declined_entry
        && captured
            .watchwolf_attacks
            .last()
            .is_some_and(|attack| *attack < captured.terminal_sequence.unwrap_or(usize::MAX))
        && captured
            .watchwolf_damage
            .last()
            .is_some_and(|(damage, _)| *damage < captured.terminal_sequence.unwrap_or(usize::MAX)))
    {
        return Err("Copy Enchantment casting, replacement decisions, copy receipt, and terminal combat events are out of order".to_owned());
    }
    let terminal = captured
        .terminal_sequence
        .ok_or("GameEnded P0 receipt is missing")?;
    let projected = project_events(&captured.events);
    let mana = |object: ObjectId| -> Vec<String> {
        captured
            .payments
            .get(&object)
            .into_iter()
            .flatten()
            .map(|color| format!("{color:?}"))
            .collect()
    };
    Ok(json!({
        "id": "rav-copy-enchantment-entry-copy-aura-and-decline-full-game",
        "seed": seed,
        "passed": true,
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": 0,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": 0,
        "copy_enchantment": {
            "definition_id": CARD_ID,
            "mana_cost": COPY_COST,
            "copy_spell_objects": captured.copies.iter().map(|object| object.0).collect::<Vec<_>>(),
            "paid_mana_colors": captured.copies.iter().map(|object| mana(*object)).collect::<Vec<_>>(),
            "source_choice_count": captured.source_decisions_opened.len(),
            "aura_attachment_choice_count": captured.aura_decisions_opened.len(),
            "source_selections": captured.source_choice_selections.iter().map(|selection| selection.iter().map(|object| object.0).collect::<Vec<_>>()).collect::<Vec<_>>(),
            "aura_attachment_selections": captured.aura_choice_selections.iter().map(|selection| selection.iter().map(|object| object.0).collect::<Vec<_>>()).collect::<Vec<_>>(),
            "copied_aura_definition_id": captured.copy_definitions[0],
            "declined_copy_definition_id": captured.copy_definitions[1],
            "copied_aura_attached_to": captured.copy_attachments[0].map(|object| object.0),
            "source_aura_attached_to": captured.source_attachment.map(|object| object.0),
            "permanent_copied_receipts": captured.permanent_copies.iter().map(|receipt| json!({"sequence":receipt.sequence,"source":receipt.source.0,"source_incarnation":receipt.source_incarnation,"target":receipt.target.0,"target_incarnation":receipt.target_incarnation})).collect::<Vec<_>>(),
            "copied_aura_etb_trigger_stacks": captured.trigger_stack_sequences.len(),
            "copied_aura_etb_trigger_resolutions": captured.trigger_resolve_sequences.len(),
            "saproling_token_events": captured.token_sequences.len(),
            "watchwolf_definition_id": TARGET_CREATURE_ID,
            "watchwolf_received_trample": captured.watchwolf_trample,
            "watchwolf_power": captured.watchwolf_power,
            "watchwolf_toughness": captured.watchwolf_toughness,
            "watchwolf_attacks": captured.watchwolf_attacks.len(),
            "watchwolf_damage_packets": captured.watchwolf_damage.iter().map(|(_, amount)| *amount).collect::<Vec<_>>(),
            "terminal_life": captured.life,
            "terminal_pilot_win": true,
            "event_order": {"watchwolf_cast":watchwolf_cast,"source_aura_cast":source_cast,"source_aura_entry":source_entry,"first_copy_cast":captured.copy_spell_casts[0],"copy_choice_opened":captured.source_decisions_opened[0],"aura_choice_opened":captured.aura_decisions_opened[0],"first_copy_resolved":first_copy_resolved,"copied_aura_entry":copied_entry,"permanent_copied":receipt.sequence,"declined_copy_cast":captured.copy_spell_casts[1],"declined_copy_choice":captured.source_decisions_opened[1],"declined_copy_resolved":second_copy_resolved,"declined_copy_entry":declined_entry,"last_watchwolf_damage":captured.watchwolf_damage.last().map(|(sequence, _)| *sequence),"game_end":terminal}
        },
        "ordered_event_sequence": [watchwolf_cast, source_cast, source_entry, captured.copy_spell_casts[0], captured.source_decisions_opened[0], captured.aura_decisions_opened[0], receipt.sequence, captured.copy_spell_casts[1], captured.source_decisions_opened[1], declined_entry, terminal],
        "events": projected
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

fn entry(card: &str, count: u8) -> DeckEntry {
    DeckEntry {
        card: card.to_owned(),
        count,
    }
}

fn deck_size(deck: &DeckList) -> usize {
    deck.mainboard
        .iter()
        .map(|entry| usize::from(entry.count))
        .sum()
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 42 && card.name == CARD_NAME;
    json!({"collector_number":card.collector_number,"definition_id":definition_id,"name":card.name,"trace_status":if passed {"passed"} else {"not_qualified"},"qualification":if passed {"unreviewed_diagnostic"} else {"not_evaluated"},"credited":false})
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}
