//! Full-game RAV Followed Footsteps authority.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Target, TriggerCondition, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CardSemanticStatus, CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT,
    RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_attachment_bindings, rav_basic_land_type_bindings,
    rav_legendary_permanent_bindings, rav_main_set_catalog, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::{Item, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-FOLLOWED-FOOTSTEPS";
const CARD_NAME: &str = "Followed Footsteps";
const TARGET_ID: &str = "RAV-WATCHWOLF";
const ATTACKER_ID: &str = "RAV-TATTERED-DRAKE";
const ABILITY_ID: &str = "aura-controller-upkeep-token-copy";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/followed_footsteps.rs";
const MAX_SEED: u64 = 64;
const MAX_TURNS: u32 = 64;
const MAX_MOVES: u32 = 10_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/followed_footsteps.rs"]
mod candidate_followed_footsteps;

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
            eprintln!("Followed Footsteps PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongCost,
    NoAuraAttachmentBinding,
    NoUpkeepTrigger,
    WrongUpkeepControllerScope,
    NoTokenCopyEffect,
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
struct TokenState {
    id: ObjectId,
    controller: usize,
    zone: Option<Zone>,
    incarnation: u64,
    definition: Option<String>,
    power: Option<i32>,
    toughness: Option<i32>,
    is_token: bool,
    copied_from: Option<ObjectId>,
    copied_from_incarnation: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    source: Option<ObjectId>,
    source_incarnation: Option<u64>,
    source_attachment: Option<ObjectId>,
    source_attachment_incarnation: Option<u64>,
    source_payment: Vec<Color>,
    target: Option<ObjectId>,
    target_controller: Option<usize>,
    target_incarnation: Option<u64>,
    attacker: Option<ObjectId>,
    attacker_payment: Vec<Color>,
    attachment_events: Vec<(usize, ObjectId, ObjectId)>,
    active_upkeep: Option<usize>,
    upkeep_steps: Vec<(usize, usize)>,
    trigger_stacks: Vec<(usize, usize, usize, u64)>,
    trigger_resolutions: Vec<(usize, u64)>,
    tokens: Vec<(usize, usize, ObjectId)>,
    copies: Vec<CopyReceipt>,
    token_states: Vec<TokenState>,
    attacker_declarations: Vec<usize>,
    attacker_damage: Vec<(usize, i32)>,
    terminal_sequence: Option<usize>,
    life: [i64; 2],
}

fn run_authority() -> Result<Value, String> {
    // Verify the exact candidate source and its AST before invoking any module function.
    let source_guard = validate_candidate_source()?;
    validate_candidate_contract()?;
    let catalog = unique_catalog()?;
    let (seed, capture, trace) = find_trace()?;
    let replay = run_match(seed, Mutation::None)?;
    if capture != replay || project_events(&capture.events) != project_events(&replay.events) {
        return Err("Followed Footsteps full-game replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "printed_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongCost),
        "aura_attachment_binding_removal_detected_in_game": mutation_detected(seed, Mutation::NoAuraAttachmentBinding),
        "upkeep_trigger_removal_detected_in_game": mutation_detected(seed, Mutation::NoUpkeepTrigger),
        "wrong_upkeep_controller_scope_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongUpkeepControllerScope),
        "token_copy_effect_removal_detected_in_game": mutation_detected(seed, Mutation::NoTokenCopyEffect),
        "sealed_mutation_authority": false
    });
    for key in [
        "printed_cost_mutation_detected_in_game",
        "aura_attachment_binding_removal_detected_in_game",
        "upkeep_trigger_removal_detected_in_game",
        "wrong_upkeep_controller_scope_mutation_detected_in_game",
        "token_copy_effect_removal_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Followed Footsteps gameplay mutation escaped: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    let printings = rav_main_set_catalog();
    Ok(json!({
        "schema_version":"cardbench.magic.card-pvp.check.v1",
        "task_id":TASK_ID,
        "expansion":"RAV",
        "card":{"collector_number":51,"definition_id":CARD_ID,"name":CARD_NAME},
        "candidate":{"kind":candidate_kind(),"card_definition":CARD_ID},
        "candidate_kind":candidate_kind(),
        "viewer_scope":format!("{:?}",ViewerScope::Judge),
        "rules_authority":{
            "comprehensive_rules":"2026-08-07",
            "authority_cutoff":"2026-09-24",
            "source_contract":"Original RAV #51 is a blue {3}{U}{U} Enchantment — Aura with enchant creature. At the beginning of the Aura controller's upkeep, create a creature token that's a copy of enchanted creature. The Aura controller controls the triggered ability and the created token, even when another player controls the enchanted creature.",
            "oracle_reconciliation":"Current Oracle modernizes the token wording from 'put a creature token into play' to 'create a token'; it preserves the printed Aura-controller upkeep scope and copy behavior.",
            "checked_sections":["303.4 Aura attachment", "508-511 turn structure and upkeep", "603.2-603.3 triggered abilities", "608.2 resolving abilities", "110.5 copyable values", "111.10 token copies", "400.7 zone changes and new objects"]
        },
        "candidate_source_validation":source_guard,
        "passed":true,
        "pilot_card_passed":true,
        "replay_deterministic":true,
        "cards_passed":cards_passed,
        "cards_credited":0,
        "cards_total":catalog.len(),
        "printings_total":printings.len(),
        "coverage_complete":false,
        "full_reward":false,
        "harbor_reward":0.0,
        "pilot_reward":0.0,
        "qualification_status":"unreviewed_diagnostic",
        "mutation_sensitivity":mutations,
        "traces":[trace],
        "per_card_breakdown":breakdown
    }))
}

fn validate_candidate_source() -> Result<Value, String> {
    let sentinel = "/tmp/cardbench-followed-footsteps-candidate-executed";
    let _ = std::fs::remove_file(sentinel);
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Followed Footsteps source: {error}"))?;
    validate_candidate_source_text(&source)?;
    let injected = source.replacen(
        "pub(crate) fn definition() -> CardDefinition {",
        "pub(crate) fn definition() -> CardDefinition { let _ = std::fs::write(\"/tmp/cardbench-followed-footsteps-candidate-executed\", b\"ran\");",
        1,
    );
    if injected == source || syn::parse_file(&injected).is_err() {
        return Err(
            "compile-valid Followed Footsteps injection fixture is invalid Rust".to_owned(),
        );
    }
    if validate_candidate_source_text(&injected).is_ok() {
        return Err("Followed Footsteps source guard accepted an injected side effect".to_owned());
    }
    if std::path::Path::new(sentinel).exists() {
        return Err("Followed Footsteps injected side effect executed before rejection".to_owned());
    }
    Ok(json!({
        "normalized_literal_source_and_ast_checked_before_candidate_calls":true,
        "compile_valid_injected_side_effect_fixture_parsed":true,
        "compile_valid_injected_side_effect_candidate_rejected":true,
        "injected_candidate_calls_before_rejection":0,
        "side_effect_sentinel_absent_after_rejection":true,
        "guard_ran_before_candidate_function_calls":true
    }))
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let normalized = source.replace("\r\n", "\n");
    if normalized != EXPECTED_CANDIDATE_SOURCE {
        return Err(
            "Followed Footsteps source differs from the pinned literal RAV module".to_owned(),
        );
    }
    let parsed = syn::parse_file(&normalized)
        .map_err(|_| "Followed Footsteps source is not valid Rust".to_owned())?;
    if parsed.shebang.is_some() || !parsed.attrs.iter().all(|attr| attr.path().is_ident("doc")) {
        return Err(
            "Followed Footsteps source contains unsupported module-level syntax".to_owned(),
        );
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
                    "Followed Footsteps source contains executable or unsupported top-level syntax"
                        .to_owned(),
                );
            }
        }
    }
    if functions
        != BTreeSet::from([
            "definition".to_owned(),
            "attachment_binding".to_owned(),
            "triggered_ability_binding".to_owned(),
        ])
    {
        return Err(
            "Followed Footsteps module must expose only its three candidate factories".to_owned(),
        );
    }
    Ok(())
}

const EXPECTED_CANDIDATE_SOURCE: &str = r###"//! Candidate-owned RAV Followed Footsteps definition and bindings.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AttachmentBinding, AttachmentKind, CardDefinition, CardType, Color, Effect, ManaCost,
    TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding,
};

/// RAV #51 is a blue `{3}{U}{U}` Aura that enchants a creature.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-FOLLOWED-FOOTSTEPS",
        name: "Followed Footsteps",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Blue, Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "aura-enchant-creature",
            "aura-controller-upkeep-token-copy",
            "layer-one-token-copy-provenance",
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

#[must_use]
pub(crate) fn attachment_binding() -> AttachmentBinding {
    AttachmentBinding {
        card_definition: "RAV-FOLLOWED-FOOTSTEPS",
        kind: AttachmentKind::Aura,
        target: TargetRequirement::Creature,
        changes: vec![],
        granted_activated_abilities: vec![],
    }
}

/// The Aura's controller gets one token copy at each of their own upkeeps
/// while the Aura remains attached to the exact enchanted creature.
#[must_use]
pub(crate) fn triggered_ability_binding() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-FOLLOWED-FOOTSTEPS",
        ability: TriggeredAbility {
            id: "aura-controller-upkeep-token-copy",
            condition: TriggerCondition::BeginningOfUpkeep,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::CreateTokenCopyOfAttachedCreature],
        },
    }
}
"###;

fn validate_candidate_contract() -> Result<(), String> {
    let definition = candidate_followed_footsteps::definition();
    let registered_definition = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Followed Footsteps definition is missing from the RAV registry")?;
    let attachment = candidate_followed_footsteps::attachment_binding();
    let registered_attachment = rav_attachment_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Followed Footsteps Aura binding is missing from the registry")?;
    let trigger = candidate_followed_footsteps::triggered_ability_binding();
    let registered_trigger = rav_triggered_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("Followed Footsteps upkeep trigger is missing from the registry")?;
    if definition != registered_definition
        || definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(3, [Color::Blue, Color::Blue])
        || definition.colors != BTreeSet::from([Color::Blue])
        || definition.card_types != BTreeSet::from([CardType::Enchantment])
        || definition.effects
            != vec![Effect::AttachSourceToTarget {
                target: cardbench_magic_engine::TargetRequirement::Creature,
                changes: vec![],
            }]
        || attachment != registered_attachment
        || attachment.kind != cardbench_magic_engine::AttachmentKind::Aura
        || attachment.target != cardbench_magic_engine::TargetRequirement::Creature
        || trigger != registered_trigger
        || trigger.ability.condition != TriggerCondition::BeginningOfUpkeep
        || trigger.ability.effects != vec![Effect::CreateTokenCopyOfAttachedCreature]
    {
        return Err("Followed Footsteps identity, Aura attachment, or upkeep token-copy behavior differs from RAV #51".to_owned());
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
            .any(|card| card.collector_number == 51 && card.name == CARD_NAME)
    {
        return Err(
            "RAV catalog identity or denominator differs for Followed Footsteps".to_owned(),
        );
    }
    Ok(cards)
}

fn find_trace() -> Result<(u64, Captured, Value), String> {
    if let Ok(value) = std::env::var("CARDBENCH_FOLLOWED_FOOTSTEPS_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_FOLLOWED_FOOTSTEPS_SEED must be an unsigned integer")?;
        let capture = run_match(seed, Mutation::None)?;
        let trace = validate_trace(&capture, seed)?;
        return Ok((seed, capture, trace));
    }
    let mut last_error = "no seed attempted".to_owned();
    for seed in 0..=MAX_SEED {
        match run_match(seed, Mutation::None)
            .and_then(|capture| validate_trace(&capture, seed).map(|trace| (capture, trace)))
        {
            Ok((capture, trace)) => return Ok((seed, capture, trace)),
            Err(error) => last_error = format!("seed {seed}: {error}"),
        }
    }
    Err(format!(
        "no bounded Followed Footsteps full-game trace succeeded: {last_error}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation).and_then(|capture| validate_trace(&capture, seed)) {
        Ok(_) => false,
        Err(_) => true,
    }
}

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let mut definitions = card_definitions();
    let definition = definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Followed Footsteps registry definition missing")?;
    *definition = candidate_followed_footsteps::definition();
    match mutation {
        Mutation::WrongCost => definition.mana_cost = ManaCost::with_colors(2, [Color::Blue]),
        Mutation::None
        | Mutation::NoAuraAttachmentBinding
        | Mutation::NoUpkeepTrigger
        | Mutation::WrongUpkeepControllerScope
        | Mutation::NoTokenCopyEffect => {}
    }
    let mut attachments = rav_attachment_bindings();
    let attachment_index = attachments
        .iter()
        .position(|binding| binding.card_definition == CARD_ID)
        .ok_or("Followed Footsteps attachment binding missing from RAV registry")?;
    attachments[attachment_index] = candidate_followed_footsteps::attachment_binding();
    let mut triggers = rav_triggered_ability_bindings();
    let trigger_index = triggers
        .iter()
        .position(|binding| binding.card_definition == CARD_ID && binding.ability.id == ABILITY_ID)
        .ok_or("Followed Footsteps trigger missing from RAV registry")?;
    triggers[trigger_index] = candidate_followed_footsteps::triggered_ability_binding();
    match mutation {
        Mutation::NoAuraAttachmentBinding => {
            attachments.retain(|binding| binding.card_definition != CARD_ID);
        }
        Mutation::NoUpkeepTrigger => {
            triggers.retain(|binding| binding.card_definition != CARD_ID);
        }
        Mutation::WrongUpkeepControllerScope => {
            triggers[trigger_index].ability.condition =
                TriggerCondition::BeginningOfAttachedCreaturesControllerUpkeep;
        }
        Mutation::NoTokenCopyEffect => triggers[trigger_index].ability.effects.clear(),
        Mutation::None | Mutation::WrongCost => {}
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
    game.register_attachment_bindings(attachments)
        .map_err(|error| error.to_string())?;
    game.register_legendary_permanent_bindings(rav_legendary_permanent_bindings())
        .map_err(|error| error.to_string())?;
    Ok(game)
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut game = new_game(mutation)?;
    let pilot = DeckList {
        mainboard: vec![
            entry(CARD_ID, 4),
            entry(ATTACKER_ID, 4),
            entry("RAV-ISLAND", 24),
            entry("RAV-FOREST", 16),
            entry("RAV-PLAINS", 12),
        ],
        sideboard: vec![],
    };
    let opponent = DeckList {
        mainboard: vec![
            entry(TARGET_ID, 4),
            entry("RAV-FOREST", 28),
            entry("RAV-PLAINS", 28),
        ],
        sideboard: vec![],
    };
    if deck_size(&pilot) != 60 || deck_size(&opponent) != 60 {
        return Err(
            "Followed Footsteps constructed decks must each contain exactly sixty cards".to_owned(),
        );
    }
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    for (seat, deck) in [(PlayerId(0), &pilot), (PlayerId(1), &opponent)] {
        game.load_deck_into_library(seat, deck)
            .map_err(|error| error.to_string())?;
        game.draw_opening_hand(seat, 7)
            .map_err(|error| error.to_string())?;
    }
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies = [
        FootstepsPilot::new(PlayerId(0)),
        FootstepsPilot::new(PlayerId(1)),
    ];
    let mut captured = Captured {
        events: vec![],
        winner: None,
        turns: 0,
        accepted_moves: 0,
        rejected_moves: 0,
        source: None,
        source_incarnation: None,
        source_attachment: None,
        source_attachment_incarnation: None,
        source_payment: vec![],
        target: None,
        target_controller: None,
        target_incarnation: None,
        attacker: None,
        attacker_payment: vec![],
        attachment_events: vec![],
        active_upkeep: None,
        upkeep_steps: vec![],
        trigger_stacks: vec![],
        trigger_resolutions: vec![],
        tokens: vec![],
        copies: vec![],
        token_states: vec![],
        attacker_declarations: vec![],
        attacker_damage: vec![],
        terminal_sequence: None,
        life: [20, 20],
    };
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || captured.accepted_moves >= MAX_MOVES {
            return Err("Followed Footsteps duel exceeded its bounded horizon".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let pilot = policies
            .get_mut(player.0)
            .ok_or("Followed Footsteps policy seat is missing")?;
        let action = pilot.propose_move(&view);
        let before = game.event_log.len();
        game.submit_policy_move(player, pilot.id(), action)
            .map_err(|error| {
                format!(
                    "Followed Footsteps action rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        captured.accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("Followed Footsteps invariant failed: {error}"))?;
        observe(
            &game,
            &game.event_log[before..],
            captured.events.len(),
            &mut captured,
        )?;
        captured.events.extend_from_slice(&game.event_log[before..]);
    }
    captured.winner = game.winner().map(|winner| winner.0);
    captured.turns = game.turn;
    for seat in 0..2 {
        captured.life[seat] = game
            .player(PlayerId(seat))
            .map_err(|error| error.to_string())?
            .life;
    }
    if let Some(source) = captured.source {
        let object = game.object(source).map_err(|error| error.to_string())?;
        captured.source_incarnation = Some(object.incarnation);
        captured.source_attachment = object.attached_to;
        captured.source_attachment_incarnation = object.attached_to_incarnation;
        if let Some(attachment) = object.attached_to {
            captured.target_incarnation = game
                .object(attachment)
                .ok()
                .map(|target| target.incarnation);
            captured.target_controller = game
                .controller_of(attachment)
                .ok()
                .map(|controller| controller.0);
        }
    }
    for seat in 0..2 {
        let view = game
            .view_for_player(PlayerId(seat))
            .map_err(|error| error.to_string())?;
        for permanent in view.own_battlefield {
            let object = game
                .object(permanent.id)
                .map_err(|error| error.to_string())?;
            if object.token.is_none() {
                continue;
            }
            let characteristics = game
                .characteristics(permanent.id)
                .map_err(|error| error.to_string())?;
            let copied = object.copied_permanent.as_ref();
            captured.token_states.push(TokenState {
                id: permanent.id,
                controller: game
                    .controller_of(permanent.id)
                    .map_err(|error| error.to_string())?
                    .0,
                zone: game.zone_of(permanent.id),
                incarnation: object.incarnation,
                definition: object.effective_definition().map(str::to_owned),
                power: characteristics.power,
                toughness: characteristics.toughness,
                is_token: true,
                copied_from: copied.map(|snapshot| snapshot.source),
                copied_from_incarnation: copied.map(|snapshot| snapshot.source_incarnation),
            });
        }
    }
    Ok(captured)
}

fn observe(
    game: &Game,
    emitted: &[GameEvent],
    base: usize,
    captured: &mut Captured,
) -> Result<(), String> {
    for (offset, event) in emitted.iter().enumerate() {
        let sequence = base + offset;
        match event {
            GameEvent::StepBegan {
                active_player,
                step: Step::Upkeep,
                ..
            } => {
                captured.active_upkeep = Some(active_player.0);
                captured.upkeep_steps.push((sequence, active_player.0));
            }
            GameEvent::StepBegan { .. } => captured.active_upkeep = None,
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                captured.source = Some(*card);
            }
            GameEvent::SpellCast {
                player: PlayerId(1),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == TARGET_ID) =>
            {
                captured.target = Some(*card);
            }
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == ATTACKER_ID) =>
            {
                captured.attacker = Some(*card);
            }
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                captured.source_payment = colors.clone();
            }
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == ATTACKER_ID) =>
            {
                captured.attacker_payment = colors.clone();
            }
            GameEvent::AuraAttached { aura, target } if captured.source == Some(*aura) => {
                captured.attachment_events.push((sequence, *aura, *target));
            }
            GameEvent::TriggeredAbilityStacked {
                controller,
                source,
                source_incarnation,
                ability,
            } if *ability == ABILITY_ID && captured.source == Some(*source) => {
                captured.trigger_stacks.push((
                    sequence,
                    controller.0,
                    captured.active_upkeep.unwrap_or(usize::MAX),
                    *source_incarnation,
                ));
            }
            GameEvent::AbilityResolved {
                source,
                source_incarnation,
                ability,
            } if *ability == ABILITY_ID && captured.source == Some(*source) => {
                captured
                    .trigger_resolutions
                    .push((sequence, *source_incarnation));
            }
            GameEvent::TokenCreated { player, token } => {
                captured.tokens.push((sequence, player.0, *token));
            }
            GameEvent::PermanentCopied {
                source,
                source_incarnation,
                target,
                target_incarnation,
                ..
            } if captured.target == Some(*source) => {
                captured.copies.push(CopyReceipt {
                    sequence,
                    source: *source,
                    source_incarnation: *source_incarnation,
                    target: *target,
                    target_incarnation: *target_incarnation,
                });
            }
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } if captured
                .attacker
                .is_some_and(|attacker| attackers.contains(&attacker)) =>
            {
                captured.attacker_declarations.push(sequence);
            }
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if captured.attacker == Some(*source) => {
                captured.attacker_damage.push((sequence, *amount));
            }
            GameEvent::GameEnded {
                winner: Some(PlayerId(0)),
            } => captured.terminal_sequence = Some(sequence),
            _ => {}
        }
    }
    Ok(())
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.winner != Some(0)
        || captured.turns == 0
        || captured.turns > MAX_TURNS
        || captured.rejected_moves != 0
        || captured.terminal_sequence.is_none()
        || captured.life[0] <= 0
        || captured.life[1] != 0
    {
        return Err(format!(
            "Followed Footsteps did not produce a bounded P0 terminal win: winner={:?}, life={:?}, turns={}",
            captured.winner, captured.life, captured.turns
        ));
    }
    let source = captured.source.ok_or("Followed Footsteps was not cast")?;
    let target = captured.target.ok_or("opponent Watchwolf was not cast")?;
    let attacker = captured.attacker.ok_or("P0 Tattered Drake was not cast")?;
    let source_incarnation = captured
        .source_incarnation
        .ok_or("Aura incarnation is missing")?;
    let target_incarnation = captured
        .target_incarnation
        .ok_or("enchanted creature incarnation is missing")?;
    let terminal = captured
        .terminal_sequence
        .ok_or("P0 terminal receipt is missing")?;
    let copied_tokens = captured
        .tokens
        .iter()
        .filter(|(_, _, token)| {
            captured
                .copies
                .iter()
                .any(|receipt| receipt.target == *token)
        })
        .collect::<Vec<_>>();
    if captured.source_attachment != Some(target)
        || captured.source_attachment_incarnation != Some(target_incarnation)
        || captured.target_controller != Some(1)
        || captured.attachment_events.len() != 1
        || captured.attachment_events[0].1 != source
        || captured.attachment_events[0].2 != target
        || captured.trigger_stacks.len() < 2
        || captured.trigger_resolutions.len() != captured.trigger_stacks.len()
        || captured.tokens.len() != captured.trigger_resolutions.len()
        || captured.copies.len() != captured.tokens.len()
        || copied_tokens.len() != captured.tokens.len()
        || captured
            .trigger_stacks
            .iter()
            .any(|(_, controller, active, incarnation)| {
                *controller != 0 || *active != 0 || *incarnation != source_incarnation
            })
        || captured
            .trigger_resolutions
            .iter()
            .any(|(_, incarnation)| *incarnation != source_incarnation)
        || captured
            .tokens
            .iter()
            .any(|(_, controller, _)| *controller != 0)
        || captured
            .upkeep_steps
            .iter()
            .filter(|(_, active)| *active == 0)
            .count()
            < captured.trigger_stacks.len()
        || captured.token_states.len() != captured.tokens.len()
        || captured.token_states.iter().any(|token| {
            token.controller != 0
                || token.zone != Some(Zone::Battlefield)
                || token.definition.as_deref() != Some(TARGET_ID)
                || token.power != Some(3)
                || token.toughness != Some(3)
                || !token.is_token
                || token.copied_from != Some(target)
                || token.copied_from_incarnation != Some(target_incarnation)
        })
        || captured.attacker_declarations.is_empty()
        || captured.attacker_damage.is_empty()
        || captured
            .attacker_damage
            .iter()
            .any(|(_, amount)| *amount != 2)
        || captured
            .attacker_damage
            .iter()
            .any(|(sequence, _)| *sequence >= terminal)
    {
        return Err(format!(
            "Followed Footsteps missed its Aura-controller upkeep/copy contract: attached={:?}, target controller={:?}, upkeep steps={:?}, stacks={:?}, resolves={:?}, tokens={}, copies={}, token states={}",
            captured.source_attachment,
            captured.target_controller,
            captured.upkeep_steps,
            captured.trigger_stacks,
            captured.trigger_resolutions,
            captured.tokens.len(),
            captured.copies.len(),
            captured.token_states.len()
        ));
    }
    let target_cast = event_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == target))
        .ok_or("opponent Watchwolf cast receipt is missing")?;
    let source_cast = event_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == source))
        .ok_or("Followed Footsteps cast receipt is missing")?;
    let source_paid = event_index(&captured.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == source && colors == &captured.source_payment))
        .ok_or("Followed Footsteps payment receipt is missing")?;
    let source_entry = event_index_after(&captured.events, source_cast, |event| matches!(event, GameEvent::AuraAttached { aura, target: attached } if *aura == source && *attached == target))
        .ok_or("Followed Footsteps attachment receipt is missing")?;
    let attacker_cast = event_index(&captured.events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == attacker))
        .ok_or("Tattered Drake cast receipt is missing")?;
    let attacker_payment = event_index(&captured.events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == attacker && colors == &captured.attacker_payment))
        .ok_or("Tattered Drake payment receipt is missing")?;
    if captured.source_payment.len() != 5
        || captured
            .source_payment
            .iter()
            .filter(|color| **color == Color::Blue)
            .count()
            != 2
        || captured.attacker_payment.len() != 5
        || captured
            .attacker_payment
            .iter()
            .filter(|color| **color == Color::Blue)
            .count()
            != 1
        || !(target_cast < source_cast
            && source_paid < source_cast
            && source_cast < source_entry
            && attacker_payment < attacker_cast
            && captured.trigger_stacks[0].0 > source_entry)
    {
        return Err(
            "Followed Footsteps/creature mana and attachment event order is invalid".to_owned(),
        );
    }
    let mut ordered = Vec::new();
    for (index, stack) in captured.trigger_stacks.iter().enumerate() {
        let resolve = captured.trigger_resolutions[index].0;
        let (created, _, token) = captured.tokens[index];
        let copy = captured
            .copies
            .iter()
            .find(|receipt| receipt.target == token)
            .ok_or("token copy receipt is missing")?;
        let step = captured
            .upkeep_steps
            .iter()
            .filter(|(sequence, active)| {
                *active == 0 && *sequence > source_entry && *sequence < stack.0
            })
            .next_back()
            .map(|(sequence, _)| *sequence)
            .ok_or("trigger was not preceded by the Aura controller's upkeep")?;
        if !(step < stack.0
            && stack.0 < created
            && created < copy.sequence
            && copy.sequence < resolve)
        {
            return Err("Followed Footsteps upkeep, trigger, token, copy, and resolution ordering is invalid".to_owned());
        }
        ordered.extend([step, stack.0, created, copy.sequence, resolve]);
    }
    let last_damage = captured
        .attacker_damage
        .last()
        .map(|(sequence, _)| *sequence)
        .unwrap_or(0);
    ordered.extend([
        source_paid,
        source_cast,
        source_entry,
        attacker_payment,
        attacker_cast,
        last_damage,
        terminal,
    ]);
    ordered.sort_unstable();
    ordered.dedup();
    let tokens_json: Vec<_> = captured
        .token_states
        .iter()
        .map(|token| {
            json!({
                    "object":token.id.0,
                    "controller_seat":token.controller,
                    "zone":format!("{:?}",token.zone),
                    "incarnation":token.incarnation,
            "copied_definition_id":token.definition.as_deref(),
                    "power":token.power,
                    "toughness":token.toughness,
                    "is_token":token.is_token,
                    "copied_from":token.copied_from.map(|object| object.0),
                    "copied_from_incarnation":token.copied_from_incarnation
                })
        })
        .collect();
    let projected = project_events(&captured.events);
    Ok(json!({
        "id":"rav-followed-footsteps-aura-controller-upkeep-token-copy-full-game",
        "seed":seed,
        "passed":true,
        "terminal":"Winner(PlayerId(0))",
        "winner_seat":0,
        "turns":captured.turns,
        "accepted_moves":captured.accepted_moves,
        "rejected_moves":captured.rejected_moves,
        "mechanics":{
            "definition_id":CARD_ID,
            "mana_cost":"{3}{U}{U}",
            "source_object":source.0,
            "source_incarnation":source_incarnation,
            "attached_creature_definition_id":TARGET_ID,
            "attached_creature_object":target.0,
            "attached_creature_incarnation":target_incarnation,
            "attached_creature_controller_seat":captured.target_controller,
            "aura_controller_seat":0,
            "paid_mana_colors":captured.source_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "attached_to_exact_creature_incarnation":captured.source_attachment_incarnation == Some(target_incarnation),
            "trigger_ability_id":ABILITY_ID,
            "trigger_stack_count":captured.trigger_stacks.len(),
            "trigger_stack_controllers":captured.trigger_stacks.iter().map(|(_, controller, _, _)| *controller).collect::<Vec<_>>(),
            "trigger_stack_incarnations":captured.trigger_stacks.iter().map(|(_, _, _, incarnation)| *incarnation).collect::<Vec<_>>(),
            "trigger_active_upkeep_seats":captured.trigger_stacks.iter().map(|(_, _, active, _)| *active).collect::<Vec<_>>(),
            "trigger_resolution_count":captured.trigger_resolutions.len(),
            "created_token_count":captured.tokens.len(),
            "token_copy_receipts":captured.copies.iter().map(|receipt| json!({"sequence":receipt.sequence,"source":receipt.source.0,"source_incarnation":receipt.source_incarnation,"target":receipt.target.0,"target_incarnation":receipt.target_incarnation})).collect::<Vec<_>>(),
            "token_states":tokens_json,
            "attacking_tattered_drake":ATTACKER_ID,
            "attacking_drake_payment":captured.attacker_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "tattered_drake_attacks":captured.attacker_declarations.len(),
            "tattered_drake_damage_packets":captured.attacker_damage.iter().map(|(_, amount)| *amount).collect::<Vec<_>>(),
            "terminal_life":captured.life,
            "terminal_pilot_win":true,
            "event_order":{"target_cast":target_cast,"source_payment":source_paid,"source_cast":source_cast,"aura_attached":source_entry,"first_upkeep":captured.upkeep_steps.iter().find(|(sequence, active)| *active == 0 && *sequence > source_entry).map(|(sequence, _)| *sequence),"first_trigger":captured.trigger_stacks[0].0,"last_trigger":captured.trigger_stacks.last().map(|(sequence, _, _, _)| *sequence),"last_drake_damage":last_damage,"game_end":terminal}
        },
        "ordered_event_sequence":ordered,
        "events":projected
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

struct FootstepsPilot {
    player: PlayerId,
}

impl FootstepsPilot {
    fn new(player: PlayerId) -> Self {
        Self { player }
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

impl CodePolicy for FootstepsPilot {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.followed-footsteps-full-game-pilot.v1"
        } else {
            "rav.followed-footsteps-creature-controller-opponent.v1"
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
                    .filter(|permanent| permanent.can_attack)
                    .map(|permanent| permanent.id)
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
        if view.step == Step::DeclareBlockers
            && view.active_player == self.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.active_player != self.player
            || !matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0
            && let Some(land) = view
                .hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
        {
            return PolicyAction::PlayLand { card: land.id };
        }
        if self.player == PlayerId(1)
            && !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(TARGET_ID))
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(TARGET_ID))
            && let Some(action) = self.cast(
                card.id,
                &ManaCost::with_colors(0, [Color::Green, Color::White]),
                view,
                vec![],
            )
        {
            return action;
        }
        if self.player == PlayerId(0)
            && !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(ATTACKER_ID))
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(ATTACKER_ID))
            && let Some(action) = self.cast(
                card.id,
                &ManaCost::with_colors(4, [Color::Blue]),
                view,
                vec![],
            )
        {
            return action;
        }
        if self.player == PlayerId(0)
            && !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(CARD_ID))
            && let Some(target) = view_for_opponent_target(view)
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            && let Some(action) = self.cast(
                card.id,
                &ManaCost::with_colors(3, [Color::Blue, Color::Blue]),
                view,
                vec![Target::Permanent(target)],
            )
        {
            return action;
        }
        PolicyAction::PassPriority
    }
}

fn view_for_opponent_target(view: &GameView) -> Option<ObjectId> {
    view.opponent_battlefield
        .iter()
        .find(|permanent| permanent.definition == Some(TARGET_ID))
        .map(|permanent| permanent.id)
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
    let missing = cost.colored.iter().copied().find(|color| {
        let needed = cost
            .colored
            .iter()
            .filter(|required| **required == *color)
            .count();
        usize::from(view.mana_pool.amount(*color)) < needed
    });
    let total = u16::from(cost.generic) + u16::try_from(cost.colored.len()).ok()?;
    let desired = missing.or_else(|| {
        (u16::from(view.mana_pool.total()) < total)
            .then(|| cost.colored.first().copied().unwrap_or(Color::Blue))
    })?;
    let land = view
        .own_battlefield
        .iter()
        .find(|permanent| {
            permanent.card_types.contains(&CardType::Land)
                && !permanent.tapped
                && permanent.mana_colors.contains(&desired)
        })
        .or_else(|| {
            view.own_battlefield.iter().find(|permanent| {
                permanent.card_types.contains(&CardType::Land) && !permanent.tapped
            })
        })?;
    let color = land.basic_land_type.and_then(|land_type| match land_type {
        cardbench_magic_engine::BasicLandType::Plains => Some(Color::White),
        cardbench_magic_engine::BasicLandType::Island => Some(Color::Blue),
        cardbench_magic_engine::BasicLandType::Forest => Some(Color::Green),
        cardbench_magic_engine::BasicLandType::Mountain => Some(Color::Red),
        cardbench_magic_engine::BasicLandType::Swamp => Some(Color::Black),
        _ => None,
    })?;
    Some(PolicyAction::ActivateManaAbility {
        land: land.id,
        color,
    })
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
    let passed = card.collector_number == 51 && card.name == CARD_NAME;
    json!({"collector_number":card.collector_number,"definition_id":definition_id,"name":card.name,"trace_status":if passed {"passed"} else {"not_qualified"},"qualification":if passed {"unreviewed_diagnostic"} else {"not_evaluated"},"credited":false})
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}
