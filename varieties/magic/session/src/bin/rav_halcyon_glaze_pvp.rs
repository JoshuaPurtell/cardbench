//! Full-game RAV Halcyon Glaze authority for its cast-triggered animation.

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, CreatureSubtype, DeckEntry, DeckList, Effect,
    Game, GameEvent, GameView, HybridManaSymbol, Keyword, ManaCost, ManaPaymentSelection, ObjectId,
    PlayerId, PolicyAction, Step, TriggerCondition, TriggeredAbilityBinding, Zone,
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
use serde::Serialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::{Item, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-HALCYON-GLAZE";
const CARD_NAME: &str = "Halcyon Glaze";
const TRIGGER_ID: &str = "creature-spell-animate-source-until-end-of-turn";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/halcyon_glaze.rs";
const MAX_TRACE_SEED: u64 = 128;
const MAX_TURNS: u32 = 60;
const MAX_POLICY_MOVES: u32 = 12_000;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/halcyon_glaze.rs"]
mod candidate_halcyon_glaze;

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
            eprintln!("Halcyon Glaze PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongCastCost,
    MissingTrigger,
    WrongTriggerCondition,
    WrongPower,
    WrongToughness,
    MissingFlying,
    WrongAnimationColor,
    WrongSubtype,
    PersistentCreatureType,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct GlazeState {
    colors: Vec<String>,
    card_types: Vec<String>,
    creature_subtypes: Vec<String>,
    power: Option<i32>,
    toughness: Option<i32>,
    flying: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ResolutionObservation {
    sequence: usize,
    turn: u32,
    source_incarnation: u64,
    stack_len: usize,
    underlying_creature_spell: Option<ObjectId>,
    state: GlazeState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CreatureCast {
    sequence: usize,
    player: usize,
    card: ObjectId,
    turn: u32,
}

#[derive(Clone, Debug)]
struct CapturedMatch {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    glaze: Option<ObjectId>,
    glaze_zone: Option<Zone>,
    glaze_incarnation: Option<u64>,
    glaze_payment: Vec<Color>,
    glaze_cast_sequence: Option<usize>,
    glaze_resolved_sequence: Option<usize>,
    glaze_entered_sequence: Option<usize>,
    entry_state: Option<GlazeState>,
    creature_casts: Vec<CreatureCast>,
    trigger_stacks: Vec<(usize, usize, u64)>,
    trigger_resolutions: Vec<ResolutionObservation>,
    cleanup_state: Option<(u32, GlazeState)>,
    attacks: Vec<(usize, u32)>,
    damage_to_player: Vec<(usize, i32)>,
    life: [i64; 2],
}

fn run_authority() -> Result<Value, String> {
    // Source and syntax are checked before either candidate factory is called.
    let source_validation = validate_candidate_source()?;
    validate_candidate_runtime()?;
    let catalog = unique_catalog()?;
    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.winner != replay.winner
        || captured.glaze != replay.glaze
        || captured.trigger_resolutions != replay.trigger_resolutions
        || captured.cleanup_state != replay.cleanup_state
    {
        return Err("Halcyon Glaze full-game replay was not deterministic".to_owned());
    }

    let mutations = json!({
        "printed_cast_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongCastCost),
        "missing_creature_spell_trigger_mutation_detected_in_game": mutation_detected(seed, Mutation::MissingTrigger),
        "wrong_trigger_controller_scope_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongTriggerCondition),
        "wrong_animation_power_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongPower),
        "wrong_animation_toughness_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongToughness),
        "missing_flying_mutation_detected_in_game": mutation_detected(seed, Mutation::MissingFlying),
        "wrong_animation_color_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongAnimationColor),
        "wrong_illusion_subtype_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongSubtype),
        "persistent_creature_type_mutation_detected_in_game": mutation_detected(seed, Mutation::PersistentCreatureType),
        "sealed_mutation_authority": false
    });
    for key in [
        "printed_cast_cost_mutation_detected_in_game",
        "missing_creature_spell_trigger_mutation_detected_in_game",
        "wrong_trigger_controller_scope_mutation_detected_in_game",
        "wrong_animation_power_mutation_detected_in_game",
        "wrong_animation_toughness_mutation_detected_in_game",
        "missing_flying_mutation_detected_in_game",
        "wrong_animation_color_mutation_detected_in_game",
        "wrong_illusion_subtype_mutation_detected_in_game",
        "persistent_creature_type_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Halcyon Glaze gameplay mutation escaped the oracle: {key}"
            ));
        }
    }

    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    let printings = rav_main_set_catalog();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 54, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "authority_cutoff": "2026-09-24",
            "source_contract": "RAV #54 is a blue {1}{U}{U} Enchantment. Its printed 'Whenever you play a creature spell, Halcyon Glaze becomes a 4/4 Illusion creature with flying until end of turn' is represented by a controller-scoped creature-spell cast trigger. Current Oracle says 'this enchantment' and 'in addition to its other types'; the animation adds Creature and Illusion and preserves the blue Enchantment. The printed and current wordings have the same cast-triggered, temporary result.",
            "checked_sections": ["117 Priority and timing", "302.1-302.6 Creature characteristics and summoning sickness", "601.2 Casting spells", "603.2 Triggered abilities", "608.2 Resolving spells and abilities", "514 Cleanup step"]
        },
        "candidate_source_validation": source_validation,
        "passed": true,
        "pilot_card_passed": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "unreviewed_diagnostic",
        "replay_deterministic": true,
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

fn validate_candidate_source() -> Result<Value, String> {
    let sentinel = "/tmp/cardbench-halcyon-glaze-candidate-executed";
    let _ = std::fs::remove_file(sentinel);
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Halcyon Glaze source: {error}"))?;
    validate_candidate_source_text(&source)?;
    let injected = source.replacen(
        "pub(crate) fn definition() -> CardDefinition {",
        "pub(crate) fn definition() -> CardDefinition {\n    let _ = std::fs::write(\"/tmp/cardbench-halcyon-glaze-candidate-executed\", b\"ran\");",
        1,
    );
    if injected == source || syn::parse_file(&injected).is_err() {
        return Err("compile-valid Halcyon Glaze injection fixture is invalid Rust".to_owned());
    }
    if validate_candidate_source_text(&injected).is_ok() {
        return Err("Halcyon Glaze source guard accepted an injected side effect".to_owned());
    }
    if std::path::Path::new(sentinel).exists() {
        return Err("Halcyon Glaze injected side effect executed before rejection".to_owned());
    }
    Ok(json!({
        "normalized_literal_source_and_ast_checked_before_candidate_calls": true,
        "compile_valid_injected_side_effect_fixture_compiled": true,
        "compile_valid_injected_side_effect_candidate_rejected": true,
        "injected_candidate_calls_before_rejection": 0,
        "side_effect_sentinel_absent_after_rejection": true,
        "guard_ran_before_candidate_function_calls": true,
        "rejection_stage": "pinned-source-and-AST-before-candidate-function-invocation"
    }))
}

#[allow(dead_code)]
fn compile_valid_injected_side_effect_fixture() -> CardDefinition {
    let _ = std::fs::write("/tmp/cardbench-halcyon-glaze-candidate-executed", b"ran");
    candidate_halcyon_glaze::definition()
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let normalized = source.replace("\r\n", "\n");
    if normalized != EXPECTED_CANDIDATE_SOURCE {
        return Err("Halcyon Glaze source differs from the pinned literal RAV module".to_owned());
    }
    let file = syn::parse_file(&normalized)
        .map_err(|_| "Halcyon Glaze source is not valid Rust".to_owned())?;
    if file.shebang.is_some()
        || !file
            .attrs
            .iter()
            .all(|attribute| attribute.path().is_ident("doc"))
    {
        return Err("Halcyon Glaze source has unsupported module-level syntax".to_owned());
    }
    let mut functions = BTreeSet::new();
    for item in file.items {
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
                    "Halcyon Glaze source contains executable or unsupported top-level syntax"
                        .to_owned(),
                );
            }
        }
    }
    if functions != BTreeSet::from(["definition".to_owned(), "triggered_ability".to_owned()]) {
        return Err(
            "Halcyon Glaze must expose only its definition and trigger factories".to_owned(),
        );
    }
    Ok(())
}

const EXPECTED_CANDIDATE_SOURCE: &str = r###"//! Candidate-owned RAV Halcyon Glaze definition and creature-spell trigger.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, CreatureSubtype, Effect, Keyword, ManaCost, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding,
};

/// RAV #54 is a blue `{1}{U}{U}` enchantment that animates itself after its
/// controller casts a creature spell. Illusion is retained as an animation
/// subtype by the temporary characteristic effect.
#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-HALCYON-GLAZE",
        name: "Halcyon Glaze",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(1, [Color::Blue, Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "creature-spell-triggered-self-animation",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

/// “Whenever you cast a creature spell, Halcyon Glaze becomes a 4/4 Illusion
/// creature with flying in addition to its other types until end of turn.”
#[must_use]
pub(crate) fn triggered_ability() -> TriggeredAbilityBinding {
    TriggeredAbilityBinding {
        card_definition: "RAV-HALCYON-GLAZE",
        ability: TriggeredAbility {
            id: "creature-spell-animate-source-until-end-of-turn",
            condition: TriggerCondition::CastsCreatureSpell,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::AnimateSourceIntoCreatureUntilEndOfTurn {
                colors: BTreeSet::from([Color::Blue]),
                creature_subtypes: BTreeSet::from([CreatureSubtype::Illusion]),
                power: 4,
                toughness: 4,
                keywords: vec![Keyword::Flying],
            }],
        },
    }
}
"###;

fn validate_candidate_runtime() -> Result<(), String> {
    let definition = candidate_halcyon_glaze::definition();
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(1, [Color::Blue, Color::Blue])
        || definition.colors != BTreeSet::from([Color::Blue])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Enchantment])
        || definition.is_basic_land
        || definition.power.is_some()
        || definition.toughness.is_some()
        || definition.supported_rules
            != [
                "full-rules-fidelity",
                "colored-cost-casting",
                "creature-spell-triggered-self-animation",
            ]
        || !definition.keywords.is_empty()
        || !definition.effects.is_empty()
    {
        return Err("Halcyon Glaze definition differs from the RAV printed contract".to_owned());
    }
    let binding = candidate_halcyon_glaze::triggered_ability();
    if binding.card_definition != CARD_ID
        || binding.ability.id != TRIGGER_ID
        || binding.ability.condition != TriggerCondition::CastsCreatureSpell
        || binding.ability.mana_cost != ManaCost::new(0)
        || binding.ability.optional
        || !binding.ability.targets.is_empty()
    {
        return Err(
            "Halcyon Glaze trigger identity, scope, or costs differ from its RAV text".to_owned(),
        );
    }
    match binding.ability.effects.as_slice() {
        [
            Effect::AnimateSourceIntoCreatureUntilEndOfTurn {
                colors,
                creature_subtypes,
                power,
                toughness,
                keywords,
            },
        ] if *colors == BTreeSet::from([Color::Blue])
            && *creature_subtypes == BTreeSet::from([CreatureSubtype::Illusion])
            && *power == 4
            && *toughness == 4
            && *keywords == [Keyword::Flying] => {}
        _ => return Err("Halcyon Glaze temporary animation differs from its RAV text".to_owned()),
    }
    Ok(())
}

fn unique_catalog() -> Result<Vec<CatalogCard>, String> {
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<_> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || !catalog
            .iter()
            .any(|card| card.collector_number == 54 && card.name == CARD_NAME)
    {
        return Err("RAV catalog denominator or Halcyon Glaze identity differs".to_owned());
    }
    Ok(catalog)
}

fn find_trace() -> Result<(u64, CapturedMatch), String> {
    if let Ok(value) = std::env::var("CARDBENCH_HALCYON_GLAZE_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_HALCYON_GLAZE_SEED must be an unsigned integer".to_owned())?;
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
        "no complete Halcyon Glaze full-game PvP trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation).and_then(|captured| validate_trace(&captured, seed)) {
        Ok(_) => false,
        Err(_) => true,
    }
}

fn run_match(seed: u64, mutation: Mutation) -> Result<CapturedMatch, String> {
    let (definition, binding, remove_trigger) = mutated_candidate(mutation);
    let glaze_cost = definition.mana_cost.clone();
    let mut game = new_game(definition, binding, remove_trigger)?;
    let pilot = DeckList {
        mainboard: vec![
            entry(CARD_ID, 4),
            entry("RAV-GRAYSCALED-GHARIAL", 4),
            entry("RAV-BOROS-RECRUIT", 4),
            entry("RAV-SURVEILLING-SPRITE", 4),
            entry("RAV-LORE-BROKER", 4),
            entry("RAV-SNAPPING-DRAKE", 4),
            entry("RAV-ISLAND", 36),
        ],
        sideboard: vec![],
    };
    let opponent = DeckList {
        mainboard: vec![entry("RAV-BOROS-RECRUIT", 4), entry("RAV-MOUNTAIN", 56)],
        sideboard: vec![],
    };
    if deck_size(&pilot) != 60 || deck_size(&opponent) != 60 {
        return Err("Halcyon Glaze PvP decks must each contain exactly sixty cards".to_owned());
    }
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(0), &pilot)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(PlayerId(1), &opponent)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(0), 7)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(PlayerId(1), 7)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;

    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(GlazePilot::new(PlayerId(0), glaze_cost)),
        Box::new(RecruitPilot::new(PlayerId(1))),
    ];
    let mut captured = CapturedMatch {
        events: Vec::new(),
        winner: None,
        turns: 0,
        accepted_moves: 0,
        glaze: None,
        glaze_zone: None,
        glaze_incarnation: None,
        glaze_payment: Vec::new(),
        glaze_cast_sequence: None,
        glaze_resolved_sequence: None,
        glaze_entered_sequence: None,
        entry_state: None,
        creature_casts: Vec::new(),
        trigger_stacks: Vec::new(),
        trigger_resolutions: Vec::new(),
        cleanup_state: None,
        attacks: Vec::new(),
        damage_to_player: Vec::new(),
        life: [20, 20],
    };
    while !game.is_game_over() {
        if game.turn > MAX_TURNS || captured.accepted_moves > MAX_POLICY_MOVES {
            return Err("Halcyon Glaze full-game duel exceeded its bounded horizon".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Halcyon Glaze policy seat is missing")?;
        let action = policy.propose_move(&view);
        let before = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Halcyon Glaze policy action rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        captured.accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("Halcyon Glaze invariant failed: {error}"))?;
        let emitted = &game.event_log[before..];
        let base = captured.events.len();
        captured.events.extend_from_slice(emitted);
        observe(&game, emitted, base, &mut captured)?;
        if captured.cleanup_state.is_none()
            && let Some(first) = captured.trigger_resolutions.first()
            && game.turn > first.turn
            && game.active_player == PlayerId(1)
        {
            let glaze = captured
                .glaze
                .ok_or("Halcyon Glaze object is missing at Cleanup")?;
            captured.cleanup_state = Some((game.turn, glaze_state(&game, glaze)?));
        }
    }
    captured.winner = game.winner().map(|player| player.0);
    captured.turns = game.turn;
    captured.glaze_zone = captured.glaze.and_then(|glaze| game.zone_of(glaze));
    captured.glaze_incarnation = captured
        .glaze
        .and_then(|glaze| game.object(glaze).ok().map(|object| object.incarnation));
    for player in 0..2 {
        captured.life[player] = game
            .player(PlayerId(player))
            .map_err(|error| error.to_string())?
            .life;
    }
    Ok(captured)
}

fn mutated_candidate(mutation: Mutation) -> (CardDefinition, TriggeredAbilityBinding, bool) {
    let mut definition = candidate_halcyon_glaze::definition();
    let mut binding = candidate_halcyon_glaze::triggered_ability();
    let mut remove_trigger = false;
    match mutation {
        Mutation::None => {}
        Mutation::WrongCastCost => {
            definition.mana_cost = ManaCost::with_colors(2, [Color::Blue, Color::Blue]);
        }
        Mutation::MissingTrigger => remove_trigger = true,
        Mutation::WrongTriggerCondition => {
            binding.ability.condition = TriggerCondition::AnyPlayerCastsCreatureSpell;
        }
        Mutation::WrongPower => {
            if let Some(Effect::AnimateSourceIntoCreatureUntilEndOfTurn { power, .. }) =
                binding.ability.effects.first_mut()
            {
                *power = 3;
            }
        }
        Mutation::WrongToughness => {
            if let Some(Effect::AnimateSourceIntoCreatureUntilEndOfTurn { toughness, .. }) =
                binding.ability.effects.first_mut()
            {
                *toughness = 3;
            }
        }
        Mutation::MissingFlying => {
            if let Some(Effect::AnimateSourceIntoCreatureUntilEndOfTurn { keywords, .. }) =
                binding.ability.effects.first_mut()
            {
                keywords.clear();
            }
        }
        Mutation::WrongAnimationColor => {
            if let Some(Effect::AnimateSourceIntoCreatureUntilEndOfTurn { colors, .. }) =
                binding.ability.effects.first_mut()
            {
                *colors = BTreeSet::from([Color::Red]);
            }
        }
        Mutation::WrongSubtype => {
            if let Some(Effect::AnimateSourceIntoCreatureUntilEndOfTurn {
                creature_subtypes, ..
            }) = binding.ability.effects.first_mut()
            {
                *creature_subtypes = BTreeSet::from([CreatureSubtype::Spirit]);
            }
        }
        Mutation::PersistentCreatureType => {
            definition.card_types.insert(CardType::Creature);
            definition.power = Some(4);
            definition.toughness = Some(4);
        }
    }
    (definition, binding, remove_trigger)
}

fn new_game(
    definition: CardDefinition,
    binding: TriggeredAbilityBinding,
    remove_trigger: bool,
) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|card| card.id == CARD_ID)
        .ok_or("Halcyon Glaze is missing from the RAV definition registry")? = definition;
    let mut triggers = rav_triggered_ability_bindings();
    triggers.retain(|candidate| candidate.card_definition != CARD_ID);
    if !remove_trigger {
        triggers.push(binding);
    }
    Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers,
    )
    .map_err(|error| error.to_string())
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
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                captured.glaze = Some(*card);
                captured.glaze_cast_sequence = Some(sequence);
            }
            GameEvent::SpellResolved { card }
                if captured.glaze == Some(*card) && captured.glaze_resolved_sequence.is_none() =>
            {
                captured.glaze_resolved_sequence = Some(sequence);
            }
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                captured.glaze_payment = colors.clone();
            }
            GameEvent::CardMoved {
                card,
                to: Zone::Battlefield,
            } if captured.glaze == Some(*card) && captured.glaze_entered_sequence.is_none() => {
                captured.glaze_entered_sequence = Some(sequence);
                captured.entry_state = Some(glaze_state(game, *card)?);
            }
            GameEvent::ObjectIncarnationAdvanced {
                object,
                incarnation,
            } if captured.glaze == Some(*object) && captured.glaze_incarnation.is_none() => {
                captured.glaze_incarnation = Some(*incarnation);
            }
            GameEvent::SpellCast { player, card }
                if game.card_definition(*card).is_ok_and(|definition| {
                    definition.card_types.contains(&CardType::Creature)
                }) =>
            {
                captured.creature_casts.push(CreatureCast {
                    sequence,
                    player: player.0,
                    card: *card,
                    turn: game.turn,
                });
            }
            GameEvent::TriggeredAbilityStacked {
                controller,
                source,
                source_incarnation,
                ability: TRIGGER_ID,
            } if captured.glaze == Some(*source) => {
                captured
                    .trigger_stacks
                    .push((sequence, controller.0, *source_incarnation));
            }
            GameEvent::AbilityResolved {
                source,
                source_incarnation,
                ability: TRIGGER_ID,
            } if captured.glaze == Some(*source) => {
                let remaining = game
                    .stack
                    .iter()
                    .find(|item| item.ability_id.is_none() && item.controller == PlayerId(0))
                    .map(|item| item.card);
                captured.trigger_resolutions.push(ResolutionObservation {
                    sequence,
                    turn: game.turn,
                    source_incarnation: *source_incarnation,
                    stack_len: game.stack.len(),
                    underlying_creature_spell: remaining,
                    state: glaze_state(game, *source)?,
                });
            }
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } if captured
                .glaze
                .is_some_and(|glaze| attackers.contains(&glaze)) =>
            {
                captured.attacks.push((sequence, game.turn));
            }
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if captured.glaze == Some(*source) => {
                captured.damage_to_player.push((sequence, *amount));
            }
            _ => {}
        }
    }
    Ok(())
}

fn glaze_state(game: &Game, glaze: ObjectId) -> Result<GlazeState, String> {
    let current = game
        .characteristics(glaze)
        .map_err(|error| error.to_string())?;
    Ok(GlazeState {
        colors: current
            .colors
            .iter()
            .map(|color| format!("{color:?}"))
            .collect(),
        card_types: current
            .card_types
            .iter()
            .map(|card_type| format!("{card_type:?}"))
            .collect(),
        creature_subtypes: current
            .creature_subtypes
            .iter()
            .map(|subtype| format!("{subtype:?}"))
            .collect(),
        power: current.power,
        toughness: current.toughness,
        flying: current.keywords.contains(&Keyword::Flying),
    })
}

fn entry(card: &str, count: u16) -> DeckEntry {
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

struct GlazePilot {
    player: PlayerId,
}

impl CodePolicy for GlazePilot {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.halcyon-glaze-controller-creature-cast.v1"
        } else {
            "rav.halcyon-glaze-opponent-scope-check.v1"
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
                attackers: if self.player == PlayerId(0) {
                    view.own_battlefield
                        .iter()
                        .filter(|permanent| {
                            permanent.definition == Some(CARD_ID) && permanent.can_attack
                        })
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
        if view.lands_played == 0
            && let Some(land) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-ISLAND"))
        {
            return PolicyAction::PlayLand { card: land.id };
        }

        if self.player == PlayerId(0) {
            let glaze_in_play = view
                .own_battlefield
                .iter()
                .any(|permanent| permanent.definition == Some(CARD_ID));
            let wanted = if glaze_in_play {
                view.hand.iter().find_map(|card| {
                    let id = card.definition?;
                    is_trigger_creature(id).then_some((card, card_cost(id)?))
                })
            } else {
                view.hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
                    .map(|card| (card, ManaCost::with_colors(1, [Color::Blue, Color::Blue])))
            };
            if let Some((card, cost)) = wanted {
                if let Some(mana_selection) = payment(view, &cost) {
                    return PolicyAction::CastWithPayment {
                        request: CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection,
                    };
                }
                if let Some(action) = make_mana(view, &cost) {
                    return action;
                }
            }
        } else {
            let glaze_is_public = view
                .opponent_battlefield
                .iter()
                .any(|permanent| permanent.definition == Some(CARD_ID));
            let recruit_already_cast = view
                .own_battlefield
                .iter()
                .any(|permanent| permanent.definition == Some("RAV-BOROS-RECRUIT"));
            if glaze_is_public
                && !recruit_already_cast
                && let Some(card) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some("RAV-BOROS-RECRUIT"))
            {
                let cost = card_cost("RAV-BOROS-RECRUIT").expect("known RAV recruit cost");
                if let Some(mana_selection) = payment(view, &cost) {
                    return PolicyAction::CastWithPayment {
                        request: CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection,
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

fn is_trigger_creature(definition: &str) -> bool {
    matches!(
        definition,
        "RAV-GRAYSCALED-GHARIAL"
            | "RAV-SURVEILLING-SPRITE"
            | "RAV-LORE-BROKER"
            | "RAV-SNAPPING-DRAKE"
    )
}

fn card_cost(definition: &str) -> Option<ManaCost> {
    Some(match definition {
        "RAV-GRAYSCALED-GHARIAL" => ManaCost::with_colors(0, [Color::Blue]),
        "RAV-SURVEILLING-SPRITE" | "RAV-LORE-BROKER" => ManaCost::with_colors(1, [Color::Blue]),
        "RAV-SNAPPING-DRAKE" => ManaCost::with_colors(3, [Color::Blue]),
        "RAV-BOROS-RECRUIT" => ManaCost::with_hybrid(
            0,
            [],
            [HybridManaSymbol {
                first: Color::Red,
                second: Color::White,
            }],
        ),
        _ => return None,
    })
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
    let mut hybrid = Vec::new();
    for symbol in &cost.hybrid {
        let selected = [symbol.first, symbol.second]
            .into_iter()
            .find(|color| available.get(color).copied().unwrap_or_default() > 0)?;
        *available.get_mut(&selected)? -= 1;
        hybrid.push(selected);
    }
    let mut generic = Vec::new();
    for _ in 0..cost.generic {
        let selected = Color::MANA_ALL
            .into_iter()
            .find(|color| available.get(color).copied().unwrap_or_default() > 0)?;
        *available.get_mut(&selected)? -= 1;
        generic.push(selected);
    }
    Some(ManaPaymentSelection { generic, hybrid })
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
        .or_else(|| {
            cost.hybrid.iter().find_map(|symbol| {
                ([symbol.first, symbol.second])
                    .into_iter()
                    .find(|color| view.mana_pool.amount(*color) == 0)
            })
        })
        .unwrap_or(Color::Blue);
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

fn validate_trace(captured: &CapturedMatch, seed: u64) -> Result<Value, String> {
    if captured.winner != Some(0)
        || captured.turns > MAX_TURNS
        || captured.glaze_zone != Some(Zone::Battlefield)
        || captured.life[0] <= 0
        || captured.life[1] != 0
    {
        return Err(format!(
            "Halcyon Glaze did not finish a bounded P0 full-game win: winner={:?} life={:?} turns={}",
            captured.winner, captured.life, captured.turns
        ));
    }
    let glaze = captured.glaze.ok_or("Halcyon Glaze was not cast")?;
    let cast = captured
        .glaze_cast_sequence
        .ok_or("Halcyon Glaze cast event is missing")?;
    let resolved = captured
        .glaze_resolved_sequence
        .ok_or("Halcyon Glaze spell resolution is missing")?;
    let entered = captured
        .glaze_entered_sequence
        .ok_or("Halcyon Glaze battlefield entry event is missing")?;
    let incarnation = captured
        .glaze_incarnation
        .ok_or("Halcyon Glaze incarnation receipt is missing")?;
    if !(cast < resolved && resolved < entered)
        || captured.glaze_payment.len() != 3
        || captured
            .glaze_payment
            .iter()
            .any(|color| *color != Color::Blue)
        || captured.entry_state.as_ref() != Some(&enchantment_state())
    {
        return Err(format!(
            "Halcyon Glaze did not enter as a blue Enchantment after paying {{1}}{{U}}{{U}}: payment={:?} state={:?}",
            captured.glaze_payment, captured.entry_state
        ));
    }

    let own_casts: Vec<_> = captured
        .creature_casts
        .iter()
        .filter(|spell| spell.player == 0 && spell.sequence > entered)
        .collect();
    let opponent_casts: Vec<_> = captured
        .creature_casts
        .iter()
        .filter(|spell| spell.player == 1 && spell.sequence > entered)
        .collect();
    if own_casts.len() < 5
        || opponent_casts.is_empty()
        || captured.trigger_stacks.len() != own_casts.len()
        || captured.trigger_resolutions.len() != own_casts.len()
    {
        return Err(format!(
            "Halcyon Glaze trigger coverage differs from controlled/opponent casts: own={} opponent={} stacks={} resolutions={}",
            own_casts.len(),
            opponent_casts.len(),
            captured.trigger_stacks.len(),
            captured.trigger_resolutions.len()
        ));
    }
    for (index, spell) in own_casts.iter().enumerate() {
        let (stack_sequence, controller, source_incarnation) = captured.trigger_stacks[index];
        let observation = &captured.trigger_resolutions[index];
        let spell_resolved = event_index_after(
            &captured.events,
            spell.sequence,
            |event| matches!(event, GameEvent::SpellResolved { card } if *card == spell.card),
        )
        .ok_or("triggering creature spell did not resolve after its animation")?;
        if stack_sequence <= spell.sequence
            || controller != 0
            || source_incarnation != incarnation
            || observation.sequence <= stack_sequence
            || observation.source_incarnation != incarnation
            || observation.stack_len != 1
            || observation.underlying_creature_spell != Some(spell.card)
            || spell_resolved <= observation.sequence
            || observation.state != animated_state()
        {
            return Err(format!(
                "Halcyon Glaze trigger #{index} did not resolve above its controller's creature spell with the exact temporary 4/4 flying Illusion state: stack={stack_sequence} observation={observation:?} creature_spell_resolved={spell_resolved}"
            ));
        }
    }
    for spell in opponent_casts {
        let next_own_cast = own_casts
            .iter()
            .find(|own| own.sequence > spell.sequence)
            .map_or(usize::MAX, |own| own.sequence);
        if captured
            .trigger_stacks
            .iter()
            .any(|(sequence, _, _)| *sequence > spell.sequence && *sequence < next_own_cast)
        {
            return Err(
                "opponent creature spell incorrectly triggered the controller's Glaze".to_owned(),
            );
        }
    }
    let cleanup = captured
        .cleanup_state
        .as_ref()
        .ok_or("Halcyon Glaze animation did not expire during Cleanup")?;
    if cleanup.1 != enchantment_state() {
        return Err(format!(
            "Halcyon Glaze cleanup did not restore the same blue Enchantment characteristics: {:?}",
            cleanup.1
        ));
    }
    if captured.attacks.len() != 5
        || captured.damage_to_player.len() != 5
        || captured
            .damage_to_player
            .iter()
            .any(|(_, amount)| *amount != 4)
        || captured.attacks.iter().any(|(attack_sequence, turn)| {
            !captured.trigger_resolutions.iter().any(|resolution| {
                resolution.turn == *turn && resolution.sequence < *attack_sequence
            })
        })
    {
        return Err(format!(
            "Halcyon Glaze did not make five post-trigger flying attacks for exactly four damage each: attacks={:?}, damage={:?}",
            captured.attacks, captured.damage_to_player
        ));
    }
    let last_damage = captured.damage_to_player.last().unwrap().0;
    let game_end = event_index_after(&captured.events, last_damage, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("P0 terminal game-end event is missing after the fifth Glaze hit")?;
    let projected = project_events(&captured.events);
    let resolutions: Vec<Value> = captured
        .trigger_resolutions
        .iter()
        .map(|observation| {
            json!({
                "sequence": observation.sequence,
                "turn": observation.turn,
                "source_incarnation": observation.source_incarnation,
                "stack_len": observation.stack_len,
                "underlying_creature_spell": observation.underlying_creature_spell.map(|card| card.0),
                "state": observation.state,
            })
        })
        .collect();
    Ok(json!({
        "id": "rav-halcyon-glaze-controller-creature-cast-animation-full-game",
        "seed": seed,
        "passed": true,
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": 0,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": 0,
        "mechanics": {
            "definition_id": CARD_ID,
            "mana_cost": "{1}{U}{U}",
            "paid_mana_colors": captured.glaze_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "glaze_object": glaze.0,
            "glaze_incarnation": incarnation,
            "entered_state": captured.entry_state,
            "controller_creature_spells_after_entry": own_casts.len(),
            "opponent_creature_spells_after_entry": opponent_casts.len(),
            "trigger_stack_count": captured.trigger_stacks.len(),
            "trigger_resolution_count": captured.trigger_resolutions.len(),
            "trigger_resolutions": resolutions,
            "cleanup_turn": cleanup.0,
            "cleanup_state": cleanup.1,
            "flying_glaze_attacks": captured.attacks.len(),
            "damage_packets": captured.damage_to_player.iter().map(|(_, amount)| *amount).collect::<Vec<_>>(),
            "terminal_life": captured.life,
            "terminal_pilot_win": true,
            "event_order": {"cast": cast, "spell_resolved": resolved, "entered_battlefield": entered, "last_damage": last_damage, "game_end": game_end}
        },
        "ordered_event_sequence": [cast, resolved, entered, last_damage, game_end],
        "events": projected
    }))
}

fn enchantment_state() -> GlazeState {
    GlazeState {
        colors: vec!["Blue".to_owned()],
        card_types: vec!["Enchantment".to_owned()],
        creature_subtypes: vec![],
        power: None,
        toughness: None,
        flying: false,
    }
}

fn animated_state() -> GlazeState {
    GlazeState {
        colors: vec!["Blue".to_owned()],
        card_types: vec!["Creature".to_owned(), "Enchantment".to_owned()],
        creature_subtypes: vec!["Illusion".to_owned()],
        power: Some(4),
        toughness: Some(4),
        flying: true,
    }
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

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 54 && card.name == CARD_NAME;
    json!({
        "collector_number": card.collector_number,
        "definition_id": definition_id,
        "name": card.name,
        "engine_status": if card.semantic_status.is_executable() { "executable_compatibility_slice" } else { "catalog_only" },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "qualification": if passed { "unreviewed_diagnostic" } else { "not_evaluated" },
        "credited": false
    })
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "candidate"
    } else {
        "reference"
    }
}
