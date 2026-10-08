//! Full-game RAV Indentured Oaf authority for source-bound red-creature damage prevention.

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, DeckEntry, DeckList, Game, GameEvent, GameView,
    HybridManaSymbol, Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction,
    Step, Target, Zone,
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
use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;
use syn::{Item, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-INDENTURED-OAF";
const CARD_NAME: &str = "Indentured Oaf";
const BLOCKER_ID: &str = "RAV-BOROS-RECRUIT";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/indentured_oaf.rs";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/indentured_oaf.rs"]
mod candidate_indentured_oaf;

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
            eprintln!("Indentured Oaf authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    // This must remain the first runtime operation; candidate factories are
    // never called until the normalized literal and restricted AST pass.
    let source_validation = validate_candidate_source()?;
    validate_candidate_runtime()?;
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<CatalogCard> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
        .collect();
    if catalog.len() != RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT
        || printings.len() != RAV_MAIN_SET_EXPECTED_PRINTING_COUNT
        || !catalog
            .iter()
            .any(|card| card.collector_number == 133 && card.name == CARD_NAME)
    {
        return Err("RAV catalog denominator or Indentured Oaf identity differs".to_owned());
    }

    let (seed, captured) = find_trace()?;
    let trace = validate_trace(&captured, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if captured.events != replay.events
        || captured.winner != replay.winner
        || captured.oaf != replay.oaf
        || captured.blocker != replay.blocker
    {
        return Err("Indentured Oaf full-game replay was not deterministic".to_owned());
    }

    let mutations = json!({
        "printed_cast_cost_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongCastCost),
        "base_power_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongPower),
        "lethal_toughness_mutation_detected_in_game": mutation_detected(seed, Mutation::LethalToughness),
        "missing_prevention_mutation_detected_in_game": mutation_detected(seed, Mutation::MissingPrevention),
        "wrong_prevention_color_mutation_detected_in_game": mutation_detected(seed, Mutation::WrongPreventionColor),
        "reversed_prevention_direction_mutation_detected_in_game": mutation_detected(seed, Mutation::ReversedPreventionDirection),
        "sealed_mutation_authority": false
    });
    for key in [
        "printed_cast_cost_mutation_detected_in_game",
        "base_power_mutation_detected_in_game",
        "lethal_toughness_mutation_detected_in_game",
        "missing_prevention_mutation_detected_in_game",
        "wrong_prevention_color_mutation_detected_in_game",
        "reversed_prevention_direction_mutation_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Indentured Oaf gameplay mutation escaped the oracle: {key}"
            ));
        }
    }

    let breakdown: Vec<Value> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 133, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "authority_cutoff": "2026-09-24",
            "source_contract": "RAV #133 is a red {3}{R} 4/3 Creature. Prevent all damage that Indentured Oaf would deal to red creatures. The current Oracle wording substitutes ‘this creature’ for the card name without changing the effect. The RAV type line is Creature — Ogre Warrior; CardDefinition has no subtype field, so the subtype remains recorded identity metadata rather than an executable assertion.",
            "checked_sections": ["120 Damage", "614 Replacement effects", "702.16 Prevention effects", "510 Combat damage step"]
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongCastCost,
    WrongPower,
    LethalToughness,
    MissingPrevention,
    WrongPreventionColor,
    ReversedPreventionDirection,
}

#[derive(Clone, Debug)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    oaf: Option<ObjectId>,
    blocker: Option<ObjectId>,
    oaf_payment: Vec<Color>,
    blocker_payment: Vec<Color>,
    oaf_zone: Option<Zone>,
    oaf_incarnation: Option<u64>,
    blocker_zone: Option<Zone>,
    blocker_damage: Option<i32>,
    blocker_is_red_creature: bool,
    life: [i64; 2],
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_INDENTURED_OAF_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_INDENTURED_OAF_SEED must be an unsigned integer".to_owned())?;
        let captured = run_match(seed, Mutation::None)?;
        validate_trace(&captured, seed)?;
        return Ok((seed, captured));
    }
    for seed in 0..=MAX_TRACE_SEED {
        if let Ok(captured) = run_match(seed, Mutation::None) {
            if validate_trace(&captured, seed).is_ok() {
                return Ok((seed, captured));
            }
        }
    }
    Err(format!(
        "no complete Indentured Oaf PvP trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    match run_match(seed, mutation).and_then(|captured| validate_trace(&captured, seed)) {
        Ok(_) => false,
        Err(_) => true,
    }
}

fn validate_candidate_runtime() -> Result<(), String> {
    let definition = candidate_indentured_oaf::definition();
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(3, [Color::Red])
        || definition.colors != BTreeSet::from([Color::Red])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.power != Some(4)
        || definition.toughness != Some(3)
        || definition.supported_rules
            != [
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "prevent-damage-by-this-source-to-red-creatures",
            ]
        || definition.keywords != [Keyword::PreventDamageToCreaturesOfColor(Color::Red)]
        || !definition.effects.is_empty()
    {
        return Err("Indentured Oaf definition differs from its RAV printed contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<Value, String> {
    let sentinel = "/tmp/cardbench-indentured-oaf-candidate-executed";
    let _ = std::fs::remove_file(sentinel);
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Indentured Oaf candidate source: {error}"))?;
    validate_candidate_source_text(&source)?;
    let injected = source.replacen(
        "pub(crate) fn definition() -> CardDefinition {",
        "pub(crate) fn definition() -> CardDefinition {\n    let _ = std::fs::write(\"/tmp/cardbench-indentured-oaf-candidate-executed\", b\"ran\");",
        1,
    );
    if injected == source || syn::parse_file(&injected).is_err() {
        return Err(
            "compile-valid Indentured Oaf side-effect fixture could not be constructed".to_owned(),
        );
    }
    if validate_candidate_source_text(&injected).is_ok() {
        return Err("Indentured Oaf source guard accepted an injected side effect".to_owned());
    }
    if std::path::Path::new(sentinel).exists() {
        return Err("Indentured Oaf side effect ran before source rejection".to_owned());
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

// This fixture is compiled with the trusted authority but deliberately never
// called. Its candidate-side effect is the same compile-valid statement used
// by the source mutation above.
#[allow(dead_code)]
fn compile_valid_injected_side_effect_fixture() -> CardDefinition {
    let _ = std::fs::write("/tmp/cardbench-indentured-oaf-candidate-executed", b"ran");
    candidate_indentured_oaf::definition()
}

fn validate_candidate_source_text(source: &str) -> Result<(), String> {
    let normalized = source.replace("\r\n", "\n");
    if normalized != EXPECTED_CANDIDATE_SOURCE {
        return Err("Indentured Oaf source differs from the pinned RAV literal module".to_owned());
    }
    let file = syn::parse_file(&normalized)
        .map_err(|_| "Indentured Oaf candidate source is not valid Rust".to_owned())?;
    if file.shebang.is_some()
        || !file
            .attrs
            .iter()
            .all(|attribute| attribute.path().is_ident("doc"))
    {
        return Err("Indentured Oaf candidate source has unsupported module structure".to_owned());
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
                    "Indentured Oaf source contains executable or unsupported syntax".to_owned(),
                );
            }
        }
    }
    if functions != BTreeSet::from(["definition".to_owned()]) {
        return Err(
            "Indentured Oaf candidate must expose exactly its definition factory".to_owned(),
        );
    }
    Ok(())
}

const EXPECTED_CANDIDATE_SOURCE: &str = r###"//! Candidate-owned RAV Indentured Oaf identity and static damage prevention.

use std::collections::BTreeSet;

use cardbench_magic_engine::{CardDefinition, CardType, Color, Keyword, ManaCost};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-INDENTURED-OAF",
        name: "Indentured Oaf",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(3, [Color::Red]),
        colors: BTreeSet::from([Color::Red]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &[
            "full-rules-fidelity",
            "colored-cost-casting",
            "base-characteristics",
            "prevent-damage-by-this-source-to-red-creatures",
        ],
        power: Some(4),
        toughness: Some(3),
        keywords: vec![Keyword::PreventDamageToCreaturesOfColor(Color::Red)],
        effects: vec![],
    }
}
"###;

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut definition = candidate_indentured_oaf::definition();
    match mutation {
        Mutation::WrongCastCost => definition.mana_cost = ManaCost::with_colors(2, [Color::Red]),
        Mutation::WrongPower => definition.power = Some(3),
        Mutation::LethalToughness => definition.toughness = Some(1),
        Mutation::MissingPrevention => definition.keywords.clear(),
        Mutation::WrongPreventionColor => {
            definition.keywords = vec![Keyword::PreventDamageToCreaturesOfColor(Color::Blue)]
        }
        Mutation::ReversedPreventionDirection => {
            definition.keywords = vec![Keyword::PreventDamageFromColor(Color::Red)]
        }
        Mutation::None => {}
    }
    let mut game = new_game(definition)?;
    let pilot = DeckList {
        mainboard: vec![
            DeckEntry {
                card: CARD_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    let opponent = DeckList {
        mainboard: vec![
            DeckEntry {
                card: BLOCKER_ID.to_owned(),
                count: 4,
            },
            DeckEntry {
                card: "RAV-MOUNTAIN".to_owned(),
                count: 56,
            },
        ],
        sideboard: vec![],
    };
    if deck_size(&pilot) != 60 || deck_size(&opponent) != 60 {
        return Err("Indentured Oaf duel decks must each contain exactly sixty cards".to_owned());
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
        Box::new(OafPilotPolicy::new(PlayerId(0))),
        Box::new(RedBlockerPolicy::new(PlayerId(1))),
    ];
    let mut accepted_moves = 0;
    while !game.is_game_over() {
        if game.turn > 32 || accepted_moves > 5_000 {
            return Err("Indentured Oaf full-game duel exceeded its bounded horizon".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Indentured Oaf policy seat missing")?;
        let action = policy.propose_move(&view);
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Indentured Oaf move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted_moves += 1;
        game.validate_invariants()
            .map_err(|error| format!("Indentured Oaf invariant failure: {error}"))?;
    }
    let events = game.event_log.clone();
    let winner = game.winner().map(|player| player.0);
    let oaf = events.iter().find_map(|event| match event {
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
    });
    let blocker = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(1),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|definition| definition.id == BLOCKER_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let oaf_payment = oaf
        .map(|oaf| {
            events
                .iter()
                .find_map(|event| match event {
                    GameEvent::SpellManaPaid {
                        player: PlayerId(0),
                        card,
                        colors,
                    } if *card == oaf => Some(colors.clone()),
                    _ => None,
                })
                .unwrap_or_default()
        })
        .unwrap_or_default();
    let blocker_payment = blocker
        .map(|blocker| {
            events
                .iter()
                .find_map(|event| match event {
                    GameEvent::SpellManaPaid {
                        player: PlayerId(1),
                        card,
                        colors,
                    } if *card == blocker => Some(colors.clone()),
                    _ => None,
                })
                .unwrap_or_default()
        })
        .unwrap_or_default();
    let blocker_is_red_creature = blocker.is_some_and(|blocker| {
        game.characteristics(blocker).is_ok_and(|characteristics| {
            characteristics.card_types.contains(&CardType::Creature)
                && characteristics.colors.contains(&Color::Red)
        })
    });
    let blocker_damage =
        blocker.and_then(|blocker| game.object(blocker).ok().map(|object| object.damage));
    Ok(Captured {
        events,
        winner,
        turns: game.turn,
        accepted_moves,
        oaf,
        blocker,
        oaf_payment,
        blocker_payment,
        oaf_zone: oaf.and_then(|oaf| game.zone_of(oaf)),
        oaf_incarnation: oaf.and_then(|oaf| game.object(oaf).ok().map(|object| object.incarnation)),
        blocker_zone: blocker.and_then(|blocker| game.zone_of(blocker)),
        blocker_damage,
        blocker_is_red_creature,
        life: [
            game.player(PlayerId(0))
                .map_err(|error| error.to_string())?
                .life,
            game.player(PlayerId(1))
                .map_err(|error| error.to_string())?
                .life,
        ],
    })
}

fn new_game(definition: CardDefinition) -> Result<Game, String> {
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|item| item.id == CARD_ID)
        .ok_or("Indentured Oaf missing from set registry")? = definition;
    Game::new_with_mana_abilities_and_basic_land_types(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
    )
    .map_err(|error| error.to_string())
}

struct OafPilotPolicy {
    player: PlayerId,
    oaf_offered: bool,
}

impl OafPilotPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            oaf_offered: false,
        }
    }
}

impl CodePolicy for OafPilotPolicy {
    fn id(&self) -> &'static str {
        "rav.indentured-oaf-pilot.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers && !view.attackers_declared {
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(CARD_ID) && card.can_attack)
                    .map(|card| card.id)
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
        if view.active_player != self.player || !view.step.is_main() || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-MOUNTAIN"))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if !self.oaf_offered {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
            {
                let cost = ManaCost::with_colors(3, [Color::Red]);
                if let Some(mana_selection) = select_payment(view, &cost) {
                    self.oaf_offered = true;
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection,
                    };
                }
                if let Some(land) = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land) && !card.tapped)
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

struct RedBlockerPolicy {
    player: PlayerId,
    blocker_offered: bool,
    blocked_oaf_once: bool,
}

impl RedBlockerPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            blocker_offered: false,
            blocked_oaf_once: false,
        }
    }
}

impl CodePolicy for RedBlockerPolicy {
    fn id(&self) -> &'static str {
        "rav.indentured-oaf-red-blocker.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers && !view.attackers_declared {
            return PolicyAction::DeclareAttackers { attackers: vec![] };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            if !self.blocked_oaf_once {
                let attacker = view.opponent_battlefield.iter().find(|card| {
                    card.definition == Some(CARD_ID)
                        && card.card_types.contains(&CardType::Creature)
                });
                let blocker = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.definition == Some(BLOCKER_ID) && card.can_block);
                if let (Some(attacker), Some(blocker)) = (attacker, blocker) {
                    self.blocked_oaf_once = true;
                    return PolicyAction::DeclareBlockers {
                        assignments: vec![(attacker.id, blocker.id)],
                    };
                }
            }
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        if view.active_player != self.player || !view.step.is_main() || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            if let Some(land) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-MOUNTAIN"))
            {
                return PolicyAction::PlayLand { card: land.id };
            }
        }
        if !self.blocker_offered {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(BLOCKER_ID))
            {
                let cost = ManaCost::with_hybrid(
                    0,
                    [],
                    [HybridManaSymbol {
                        first: Color::Red,
                        second: Color::White,
                    }],
                );
                if let Some(mana_selection) = select_payment(view, &cost) {
                    self.blocker_offered = true;
                    return PolicyAction::CastWithPayment {
                        request: cardbench_magic_engine::CastRequest {
                            card: card.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection,
                    };
                }
                if let Some(land) = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.card_types.contains(&CardType::Land) && !card.tapped)
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

fn select_payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
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
    let mut hybrid = Vec::with_capacity(cost.hybrid.len());
    for symbol in &cost.hybrid {
        let color = [symbol.first, symbol.second]
            .into_iter()
            .find(|color| available.get(color).is_some_and(|amount| *amount > 0))?;
        *available.get_mut(&color)? -= 1;
        hybrid.push(color);
    }
    let mut generic = Vec::with_capacity(usize::from(cost.generic));
    for _ in 0..cost.generic {
        let color = available
            .iter()
            .filter(|(_, amount)| **amount > 0)
            .max_by_key(|(_, amount)| **amount)
            .map(|(color, _)| *color)?;
        *available.get_mut(&color)? -= 1;
        generic.push(color);
    }
    Some(ManaPaymentSelection { generic, hybrid })
}

fn deck_size(deck: &DeckList) -> usize {
    deck.mainboard
        .iter()
        .map(|entry| usize::from(entry.count))
        .sum()
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    let oaf = captured.oaf.ok_or("pilot did not cast Indentured Oaf")?;
    let blocker = captured
        .blocker
        .ok_or("opponent did not cast the red blocker")?;
    if captured.winner != Some(0)
        || captured.turns == 0
        || captured.life[0] <= 0
        || captured.life[1] != 0
    {
        return Err(
            "Indentured Oaf duel did not finish in a clean seat-zero terminal win".to_owned(),
        );
    }
    if captured.oaf_payment.len() != 4
        || captured
            .oaf_payment
            .iter()
            .any(|color| *color != Color::Red)
    {
        return Err("Indentured Oaf was not paid as {3}{R} from the printed red source".to_owned());
    }
    if captured.blocker_payment != [Color::Red] {
        return Err("Boros Recruit fixture did not pay red for its hybrid symbol".to_owned());
    }
    if captured.oaf_zone != Some(Zone::Battlefield)
        || captured.blocker_zone != Some(Zone::Battlefield)
        || captured.blocker_damage != Some(0)
        || !captured.blocker_is_red_creature
    {
        return Err("the red creature failed to survive undamaged on the battlefield".to_owned());
    }
    let events = &captured.events;
    let oaf_paid = unique_event_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == oaf && colors.len() == 4 && colors.iter().all(|color| *color == Color::Red))).ok_or("exact Oaf cast receipt missing")?;
    let oaf_cast = event_index_after(
        events,
        oaf_paid,
        |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == oaf),
    )
    .ok_or("Oaf cast event missing")?;
    let oaf_resolved = event_index_after(
        events,
        oaf_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == oaf),
    )
    .ok_or("Oaf did not resolve")?;
    let oaf_entered = event_index_after(events, oaf_resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == oaf)).ok_or("Oaf did not enter the battlefield")?;
    let oaf_incarnation_index = event_index_after(events, oaf_entered, |event| matches!(event, GameEvent::ObjectIncarnationAdvanced { object, .. } if *object == oaf)).ok_or("Oaf battlefield incarnation receipt missing")?;
    let oaf_incarnation = match &events[oaf_incarnation_index] {
        GameEvent::ObjectIncarnationAdvanced { incarnation, .. } => *incarnation,
        _ => unreachable!(),
    };
    let blocker_paid = unique_event_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(1), card, colors } if *card == blocker && colors == &[Color::Red])).ok_or("exact red hybrid blocker cast receipt missing")?;
    let blocker_cast = event_index_after(events, blocker_paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(1), card } if *card == blocker)).ok_or("red blocker cast event missing")?;
    let blocker_entered = event_index_after(events, blocker_cast, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == blocker)).ok_or("red blocker did not enter battlefield")?;
    let attack = events.iter().position(|event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&oaf))).ok_or("Oaf attack receipt missing")?;
    if captured.oaf_incarnation != Some(oaf_incarnation)
        || oaf_incarnation_index >= attack
        || oaf_entered >= attack
        || blocker_entered >= attack
        || events.iter().enumerate().any(|(index, event)| {
            index > oaf_incarnation_index && index < attack && matches!(event,
                GameEvent::ObjectIncarnationAdvanced { object, .. } | GameEvent::CardMoved { card: object, .. }
                    if *object == oaf)
        })
    {
        return Err("Oaf was not the same battlefield source incarnation through combat".to_owned());
    }
    let block = event_index_after(events, attack, |event| matches!(event, GameEvent::BlockersDeclared { player: PlayerId(1), assignments } if assignments.contains(&(oaf, blocker)))).ok_or("red creature did not block the Oaf")?;
    let first_strike_step = event_index_after(events, block, |event| {
        matches!(
            event,
            GameEvent::StepBegan {
                active_player: PlayerId(0),
                step: Step::FirstStrikeCombatDamage,
                ..
            }
        )
    })
    .ok_or("Boros Recruit first-strike damage step missing")?;
    let retaliation = event_index_after(events, first_strike_step, |event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, amount: 1 } if *source == blocker && *permanent == oaf)).ok_or("red blocker did not deal its one first-strike damage to Oaf")?;
    let normal_damage_step = event_index_after(events, retaliation, |event| {
        matches!(
            event,
            GameEvent::StepBegan {
                active_player: PlayerId(0),
                step: Step::CombatDamage,
                ..
            }
        )
    })
    .ok_or("ordinary combat damage step after first strike missing")?;
    let prevented = event_index_after(events, normal_damage_step, |event| matches!(event, GameEvent::DamagePrevented { source, target: Target::Permanent(permanent), amount: 4 } if *source == oaf && *permanent == blocker)).ok_or("Oaf's four damage to the red creature was not prevented")?;
    if events.iter().any(|event| matches!(event, GameEvent::DamageDealtToPermanent { source, permanent, .. } if *source == oaf && *permanent == blocker)) {
        return Err("Indentured Oaf dealt damage to the red creature despite its printed prevention".to_owned());
    }
    let oaf_player_damage: Vec<(usize, i32)> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| match event {
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if *source == oaf => Some((index, *amount)),
            _ => None,
        })
        .collect();
    if oaf_player_damage
        .iter()
        .map(|(_, amount)| amount)
        .sum::<i32>()
        != 20
        || oaf_player_damage.iter().any(|(_, amount)| *amount != 4)
    {
        return Err(
            "Oaf's full-game damage total did not match five unblocked four-damage attacks"
                .to_owned(),
        );
    }
    let end = unique_event_index(events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal full-game win event missing")?;
    let final_damage = oaf_player_damage
        .last()
        .map(|(index, _)| *index)
        .ok_or("Oaf had no terminal combat damage")?;
    if final_damage >= end
        || attack >= block
        || block >= first_strike_step
        || first_strike_step >= retaliation
        || retaliation >= normal_damage_step
        || normal_damage_step >= prevented
        || retaliation >= end
    {
        return Err(
            "Indentured Oaf prevention, opposing damage, and terminal event order is incorrect"
                .to_owned(),
        );
    }
    let selected = BTreeSet::from([
        oaf_paid,
        oaf_cast,
        oaf_resolved,
        oaf_entered,
        oaf_incarnation_index,
        blocker_paid,
        blocker_cast,
        blocker_entered,
        attack,
        block,
        first_strike_step,
        retaliation,
        normal_damage_step,
        prevented,
        final_damage,
        end,
    ]);
    let projected: Vec<_> = project_events(events)
        .into_iter()
        .enumerate()
        .filter_map(|(index, event)| selected.contains(&index).then_some(event))
        .collect();
    Ok(json!({
        "trace_id": "rav-indentured-oaf-source-bound-red-creature-prevention-full-game",
        "shuffle_seed": seed,
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": captured.winner,
        "turns": captured.turns,
        "accepted_moves": captured.accepted_moves,
        "rejected_moves": 0,
        "full_game_event_count": events.len(),
        "witness_event_count": selected.len(),
        "life_at_end": [{"seat":0,"life":captured.life[0]},{"seat":1,"life":captured.life[1]}],
        "mechanics": {
            "oaf_object": oaf.0,
            "oaf_incarnation": oaf_incarnation,
            "definition_id": CARD_ID,
            "printed_cost": "{3}{R}",
            "mana_payment": captured.oaf_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "power": 4,
            "toughness": 3,
            "prevented_source": oaf.0,
            "prevented_target": blocker.0,
            "target_object": blocker.0,
            "target_definition_id": BLOCKER_ID,
            "target_mana_cost": "{R/W}",
            "target_hybrid_payment": captured.blocker_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "target_is_red_creature": captured.blocker_is_red_creature,
            "prevented_damage": 4,
            "damage_dealt_to_target": 0,
            "damage_dealt_back_to_oaf": 1,
            "target_marked_damage_at_terminal": captured.blocker_damage,
            "target_zone_at_terminal": captured.blocker_zone.map(|zone| format!("{zone:?}")),
            "source_bound_direction": "Oaf-to-red-creature",
            "opposite_direction_damage_not_prevented": true,
            "oaf_player_damage_total": 20,
            "oaf_player_damage_packets": oaf_player_damage.iter().map(|(_, amount)| *amount).collect::<Vec<_>>(),
        },
        "oaf_mana_sequence": oaf_paid,
        "oaf_cast_sequence": oaf_cast,
        "blocker_mana_sequence": blocker_paid,
        "blocker_cast_sequence": blocker_cast,
        "attack_sequence": attack,
        "block_sequence": block,
        "first_strike_sequence": first_strike_step,
        "retaliation_sequence": retaliation,
        "normal_damage_sequence": normal_damage_step,
        "prevention_sequence": prevented,
        "terminal_damage_sequence": final_damage,
        "terminal_sequence": end,
        "events": projected
    }))
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

fn unique_event_index(
    events: &[GameEvent],
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    let mut matches = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 133;
    json!({
        "collector_number": card.collector_number,
        "name": card.name,
        "definition_id": definition_id,
        "engine_status": if definition_id.is_some() { "executable_compatibility_slice" } else { "catalog_only" },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "qualification": if passed { "unreviewed_diagnostic" } else { "not_evaluated" },
        "credited": false
    })
}
