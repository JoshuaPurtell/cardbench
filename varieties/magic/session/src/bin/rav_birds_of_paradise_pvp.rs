//! Full-game diagnostic authority for RAV Birds of Paradise.

use std::collections::{BTreeSet, HashSet};
use std::process::ExitCode;

use cardbench_magic_engine::{
    BasicLandManaAbilityActivation, CardDefinition, CardType, CastPaymentManaAbility, CastRequest,
    Color, DeckEntry, DeckList, Game, GameEvent, GameView, Keyword, ManaAbilityActivation,
    ManaAbilityBinding, ManaAbilityOutput, ManaCost, ManaPaymentSelection, ObjectId, PlayerId,
    PolicyAction, Step, Zone,
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
use syn::{Expr, Item, Meta, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-BIRDS-OF-PARADISE";
const CARD_NAME: &str = "Birds of Paradise";
const BIRD_ABILITY: &str = "produce-one-color";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/birds_of_paradise.rs";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/birds_of_paradise.rs"]
mod candidate_birds_of_paradise;

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
            eprintln!("Birds of Paradise PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_registry()?;
    let catalog = unique_catalog()?;
    let (seed, capture) = find_trace()?;
    let trace = validate_trace(&capture, seed)?;
    let replay = run_match(seed, Mutation::None)?;
    if capture.events != replay.events
        || capture.winner != replay.winner
        || capture.turns != replay.turns
    {
        return Err("Birds of Paradise shuffled-game replay was not deterministic".to_owned());
    }
    let mutation_sensitivity = json!({
        "wrong_bird_cast_cost_detected": mutation_detected(seed, Mutation::WrongCastCost),
        "wrong_power_detected": mutation_detected(seed, Mutation::WrongPower),
        "flying_removed_detected": mutation_detected(seed, Mutation::NoFlying),
        "chosen_color_restricted_detected": mutation_detected(seed, Mutation::RedOnly),
        "tap_cost_removed_detected": mutation_detected(seed, Mutation::NoTapCost),
        "sealed_mutation_authority": false
    });
    for key in [
        "wrong_bird_cast_cost_detected",
        "wrong_power_detected",
        "flying_removed_detected",
        "chosen_color_restricted_detected",
        "tap_cost_removed_detected",
    ] {
        if mutation_sensitivity[key] != true {
            return Err(format!(
                "Birds of Paradise mutation escaped the PvP oracle: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    let cards_passed = breakdown
        .iter()
        .filter(|card| card["trace_status"] == "passed")
        .count();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 153, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": [
                "105.2 and 202.1-202.2 color and mana symbols",
                "117 priority and turn-based actions",
                "302.1-302.6 creature characteristics and summoning sickness",
                "605.1a and 605.3 activated mana abilities",
                "601.2 casting spells and paying costs",
                "510 combat damage and terminal game state"
            ],
            "source_contract": "RAV Birds of Paradise #153, {G} green 0/1 Creature with Flying; {T}: Add one mana of any color",
            "authority_cutoff": "2026-09-24",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "deterministic_replay": true,
        "cards_passed": cards_passed,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": rav_main_set_catalog().len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "diagnostic_only; independent review and candidate trial remain",
        "mutation_sensitivity": mutation_sensitivity,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongCastCost,
    WrongPower,
    NoFlying,
    RedOnly,
    NoTapCost,
}

#[derive(Clone, Debug)]
struct Capture {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted_moves: u32,
    rejected_moves: u32,
    bird: Option<CardWitness>,
    watchwolf: Option<CardWitness>,
}

#[derive(Clone, Debug)]
struct CardWitness {
    id: ObjectId,
    power: Option<i32>,
    toughness: Option<i32>,
    flying: bool,
}

fn candidate_values(mutation: Mutation) -> (CardDefinition, ManaAbilityBinding) {
    let mut definition = candidate_birds_of_paradise::definition();
    let mut mana_ability = candidate_birds_of_paradise::mana_ability();
    match mutation {
        Mutation::None => {}
        Mutation::WrongCastCost => {
            definition.mana_cost = ManaCost::with_colors(0, [Color::Red]);
        }
        Mutation::WrongPower => definition.power = Some(1),
        Mutation::NoFlying => definition.keywords.clear(),
        Mutation::RedOnly => {
            mana_ability.ability.output = ManaAbilityOutput::Choice(BTreeSet::from([Color::Red]));
        }
        Mutation::NoTapCost => mana_ability.ability.tap_cost = false,
    }
    (definition, mana_ability)
}

fn validate_candidate_registry() -> Result<(), String> {
    let definition = candidate_birds_of_paradise::definition();
    let ability = candidate_birds_of_paradise::mana_ability();
    let registered_definition = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Birds of Paradise is missing from the RAV definition registry")?;
    let registered_ability = rav_mana_ability_bindings()
        .into_iter()
        .find(|binding| binding.card_definition == CARD_ID)
        .ok_or("Birds of Paradise is missing from the RAV mana-ability registry")?;
    if registered_definition != definition || registered_ability != ability {
        return Err(
            "Birds of Paradise registry entries are not delegated to its module".to_owned(),
        );
    }
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(0, [Color::Green])
        || definition.colors != BTreeSet::from([Color::Green])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.power != Some(0)
        || definition.toughness != Some(1)
        || definition.keywords != [Keyword::Flying]
        || !definition.effects.is_empty()
        || ability.ability.id != BIRD_ABILITY
        || !ability.ability.tap_cost
        || ability.ability.amount != 1
        || ability.ability.output != ManaAbilityOutput::Choice(Color::ALL.into_iter().collect())
        || ability.ability.life_payment.is_some()
        || ability.ability.controller_damage.is_some()
    {
        return Err("Birds of Paradise definition or chosen-color mana ability differs".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Birds of Paradise candidate source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Birds of Paradise source is not valid Rust".to_owned())?;
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(_) => {}
            Item::Fn(function) => {
                if !functions.insert(function.sig.ident.to_string())
                    || function.sig.constness.is_some()
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || !function.sig.inputs.is_empty()
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || function.block.stmts.len() != 1
                    || !matches!(
                        function.block.stmts.first(),
                        Some(Stmt::Expr(Expr::Struct(_), None))
                    )
                    || !function.attrs.iter().all(|attribute| {
                        attribute.path().is_ident("must_use")
                            && matches!(attribute.meta, Meta::Path(_))
                            || attribute.path().is_ident("doc")
                    })
                {
                    return Err("Birds of Paradise source must contain only pure definition and mana-binding literals".to_owned());
                }
            }
            _ => return Err("Birds of Paradise module has unsupported executable items".to_owned()),
        }
    }
    if functions != BTreeSet::from(["definition".to_owned(), "mana_ability".to_owned()]) {
        return Err(
            "Birds of Paradise source is missing a required candidate-owned function".to_owned(),
        );
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
    {
        return Err(
            "Birds of Paradise authority observed an unexpected RAV catalog denominator".to_owned(),
        );
    }
    Ok(catalog)
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "candidate"
    } else {
        "reference"
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

fn new_game(mutation: Mutation) -> Result<Game, String> {
    let (candidate_definition, candidate_mana) = candidate_values(mutation);
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Birds of Paradise definition missing during game construction")? =
        candidate_definition;
    let mut mana_bindings = rav_mana_ability_bindings();
    mana_bindings.retain(|binding| binding.card_definition != CARD_ID);
    mana_bindings.push(candidate_mana);
    Game::new_with_mana_abilities_and_basic_land_types(
        definitions,
        2,
        mana_bindings,
        rav_basic_land_type_bindings(),
    )
    .map_err(|error| error.to_string())
}

fn find_trace() -> Result<(u64, Capture), String> {
    if let Ok(value) = std::env::var("CARDBENCH_BIRDS_OF_PARADISE_SEED") {
        let seed = value.parse::<u64>().map_err(|_| {
            "CARDBENCH_BIRDS_OF_PARADISE_SEED must be an unsigned integer".to_owned()
        })?;
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
        "no terminal Birds of Paradise trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    run_match(seed, mutation)
        .and_then(|capture| validate_trace(&capture, seed).map(|_| capture))
        .is_err()
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Capture, String> {
    let mut game = new_game(mutation)?;
    let deck0 = deck(&[
        (CARD_ID, 4),
        (WATCHWOLF_ID, 4),
        ("RAV-FOREST", 28),
        ("RAV-PLAINS", 24),
    ]);
    let deck1 = deck(&[("RAV-PLAINS", 60)]);
    for (player, deck) in [(PlayerId(0), &deck0), (PlayerId(1), &deck1)] {
        game.load_deck_into_library(player, deck)
            .map_err(|error| error.to_string())?;
        game.draw_opening_hand(player, 7)
            .map_err(|error| error.to_string())?;
    }
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(BirdsPolicy::new(PlayerId(0))),
        Box::new(EmptyBoardPolicy),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    while !game.is_game_over() {
        if game.turn > 40 || accepted >= 4_000 {
            return Err("Birds of Paradise full game exceeded its bounded run".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Birds of Paradise policy seat missing")?;
        let action = if let Some(action) = policy.propose_pending_decision(&view) {
            action
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
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Birds of Paradise policy action rejected at turn {} / {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| format!("Birds of Paradise invariant failure: {error}"))?;
    }
    let bird = find_card_witness(&game, CARD_ID)?;
    let watchwolf = find_card_witness(&game, WATCHWOLF_ID)?;
    Ok(Capture {
        events: game.event_log.clone(),
        winner: game.winner().map(|player| player.0),
        turns: game.turn,
        accepted_moves: accepted,
        rejected_moves: attempted.saturating_sub(accepted),
        bird,
        watchwolf,
    })
}

fn find_card_witness(
    game: &Game,
    definition_id: &'static str,
) -> Result<Option<CardWitness>, String> {
    let Some(card) = game.event_log.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|definition| definition.id == definition_id) =>
        {
            Some(*card)
        }
        _ => None,
    }) else {
        return Ok(None);
    };
    let characteristics = game.characteristics(card).map_err(|error| {
        format!("could not capture live characteristics for {definition_id}: {error}")
    })?;
    Ok(Some(CardWitness {
        id: card,
        power: characteristics.power,
        toughness: characteristics.toughness,
        flying: characteristics.keywords.contains(&Keyword::Flying),
    }))
}

struct BirdsPolicy {
    player: PlayerId,
}

impl BirdsPolicy {
    fn new(player: PlayerId) -> Self {
        Self { player }
    }

    fn cast(card: ObjectId, payment_mana_abilities: Vec<CastPaymentManaAbility>) -> PolicyAction {
        PolicyAction::CastWithPayment {
            request: CastRequest {
                card,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities,
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection::default(),
        }
    }
}

impl CodePolicy for BirdsPolicy {
    fn id(&self) -> &'static str {
        "rav.birds-of-paradise-chosen-color-full-game.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.active_player == self.player
            && view.step == Step::PrecombatMain
            && view.stack_depth == 0
        {
            if view.lands_played == 0 {
                let need_forest = !view
                    .own_battlefield
                    .iter()
                    .any(|permanent| permanent.definition == Some("RAV-FOREST"));
                let preferred = if need_forest {
                    "RAV-FOREST"
                } else {
                    "RAV-PLAINS"
                };
                if let Some(land) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(preferred))
                {
                    return PolicyAction::PlayLand { card: land.id };
                }
                if let Some(land) = view
                    .hand
                    .iter()
                    .find(|card| matches!(card.definition, Some("RAV-FOREST" | "RAV-PLAINS")))
                {
                    return PolicyAction::PlayLand { card: land.id };
                }
            }
            if !view
                .own_battlefield
                .iter()
                .any(|card| card.definition == Some(CARD_ID))
                && let Some(bird) = view
                    .hand
                    .iter()
                    .find(|card| card.definition == Some(CARD_ID))
            {
                if view.mana_pool.amount(Color::Green) > 0 {
                    return Self::cast(bird.id, vec![]);
                }
                if let Some(forest) = view
                    .own_battlefield
                    .iter()
                    .find(|card| card.definition == Some("RAV-FOREST") && !card.tapped)
                {
                    return PolicyAction::ActivateManaAbility {
                        land: forest.id,
                        color: Color::Green,
                    };
                }
            }
            if let (Some(bird), Some(forest), Some(watchwolf)) = (
                view.own_battlefield.iter().find(|card| {
                    card.definition == Some(CARD_ID) && !card.tapped && !card.summoning_sick
                }),
                view.own_battlefield
                    .iter()
                    .find(|card| card.definition == Some("RAV-FOREST") && !card.tapped),
                view.hand
                    .iter()
                    .find(|card| card.definition == Some(WATCHWOLF_ID)),
            ) {
                return Self::cast(
                    watchwolf.id,
                    vec![
                        CastPaymentManaAbility::Bound(ManaAbilityActivation {
                            source: bird.id,
                            ability_id: BIRD_ABILITY,
                            chosen_color: Some(Color::White),
                        }),
                        CastPaymentManaAbility::BasicLand(BasicLandManaAbilityActivation {
                            land: forest.id,
                            color: Color::Green,
                        }),
                    ],
                );
            }
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers {
                attackers: view
                    .own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(WATCHWOLF_ID) && card.can_attack)
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
        PolicyAction::PassPriority
    }
}

struct EmptyBoardPolicy;

impl CodePolicy for EmptyBoardPolicy {
    fn id(&self) -> &'static str {
        "rav.birds-of-paradise-empty-board.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.step == Step::DeclareAttackers
            && view.active_player == view.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers { attackers: vec![] };
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != view.player
            && !view.blockers_declared
        {
            return PolicyAction::DeclareBlockers {
                assignments: vec![],
            };
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(capture: &Capture, seed: u64) -> Result<Value, String> {
    let events = &capture.events;
    if capture.winner != Some(0) || capture.rejected_moves != 0 {
        return Err(format!(
            "Birds of Paradise match did not reach a clean seat-zero win: winner={:?}, turns={}, rejected={}",
            capture.winner, capture.turns, capture.rejected_moves
        ));
    }
    let bird = capture
        .bird
        .as_ref()
        .ok_or("Birds of Paradise cast receipt missing")?;
    let watchwolf_witness = capture
        .watchwolf
        .as_ref()
        .ok_or("Watchwolf cast receipt missing")?;
    let bird_paid = find_event(events, |event| {
        matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors }
            if *card == bird.id && colors == &[Color::Green])
    })
    .ok_or("Birds of Paradise did not pay its green casting cost")?;
    let bird_cast = find_event_after(events, bird_paid, |event| {
        matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == bird.id)
    })
    .ok_or("Birds of Paradise cast event is missing")?;
    let bird_resolved = find_event_after(
        events,
        bird_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == bird.id),
    )
    .ok_or("Birds of Paradise did not resolve")?;
    let bird_entry = find_event_after(events, bird_resolved, |event| {
        matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == bird.id)
    })
    .ok_or("Birds of Paradise did not enter the battlefield")?;
    if bird.power != Some(0) || bird.toughness != Some(1) || !bird.flying {
        return Err(format!(
            "Birds did not enter as the printed flying 0/1: {bird:?}"
        ));
    }
    let watchwolf_payment = find_event_after(events, bird_entry, |event| {
        matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors }
            if *card == watchwolf_witness.id
                && colors.len() == 2
                && colors.contains(&Color::Green)
                && colors.contains(&Color::White))
    })
    .ok_or("Watchwolf did not pay one green and one white mana")?;
    let watchwolf = match events.get(watchwolf_payment) {
        Some(GameEvent::SpellManaPaid { card, .. }) => *card,
        _ => return Err("Watchwolf payment event identity is malformed".to_owned()),
    };
    let cast_mana = find_event_before(events, watchwolf_payment, |event| {
        matches!(event, GameEvent::CastPaymentManaAbilityActivated {
            player: PlayerId(0), card, source, ability: BIRD_ABILITY
        } if *card == watchwolf && *source == bird.id)
    })
    .ok_or("Birds source was not selected during Watchwolf payment")?;
    let bird_output = find_event_after(events, cast_mana, |event| {
        matches!(event, GameEvent::BoundManaAbilityActivated {
            player: PlayerId(0), source, ability: BIRD_ABILITY, color: Color::White,
            amount: 1, tapped: true, ..
        } if *source == bird.id)
    })
    .ok_or("Birds did not tap for one white mana")?;
    let bird_added = find_event_after(events, bird_output, |event| {
        matches!(
            event,
            GameEvent::ManaAdded {
                player: PlayerId(0),
                color: Color::White,
                amount: 1
            }
        )
    })
    .ok_or("chosen white mana was not added to the casting player's pool")?;
    let forest_activation = find_event_before(events, watchwolf_payment, |event| {
        matches!(event, GameEvent::CastPaymentBasicLandManaAbilityActivated {
            player: PlayerId(0), card, land, color: Color::Green
        } if *card == watchwolf && *land != bird.id)
    })
    .ok_or("typed Forest was not selected as the green payment source")?;
    let forest_output = find_event_after(events, forest_activation, |event| {
        matches!(event, GameEvent::ManaAbilityActivated {
            player: PlayerId(0), land, color: Color::Green
        } if *land != bird.id)
    })
    .ok_or("Forest intrinsic green ability did not activate")?;
    let forest_added = find_event_after(events, forest_output, |event| {
        matches!(
            event,
            GameEvent::ManaAdded {
                player: PlayerId(0),
                color: Color::Green,
                amount: 1
            }
        )
    })
    .ok_or("Forest did not add green mana")?;
    if bird_added >= watchwolf_payment || forest_added >= watchwolf_payment {
        return Err("mana sources resolved after Watchwolf's cost was paid".to_owned());
    }
    let watchwolf_cast = find_event_after(events, watchwolf_payment, |event| {
        matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == watchwolf)
    })
    .ok_or("Watchwolf cast receipt missing")?;
    let watchwolf_resolved = find_event_after(
        events,
        watchwolf_cast,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == watchwolf),
    )
    .ok_or("Watchwolf did not resolve")?;
    if watchwolf_witness.power != Some(3) || watchwolf_witness.toughness != Some(3) {
        return Err(format!(
            "Watchwolf did not enter as a 3/3: {watchwolf_witness:?}"
        ));
    }
    let entry = find_event_after(events, watchwolf_resolved, |event| {
        matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == watchwolf)
    })
    .ok_or("Watchwolf did not enter the battlefield")?;
    let attack = find_event_after(events, entry, |event| {
        matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers }
            if attackers.contains(&watchwolf))
    })
    .ok_or("Watchwolf did not attack")?;
    let damage = find_event_after(events, attack, |event| {
        matches!(event, GameEvent::DamageDealtToPlayer {
            source, player: PlayerId(1), amount: 3
        } if *source == watchwolf)
    })
    .ok_or("Watchwolf did not deal three combat damage")?;
    let terminal = unique_event(events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal seat-zero win receipt missing")?;
    if !(bird_paid < bird_cast
        && bird_cast < bird_resolved
        && bird_resolved < bird_entry
        && cast_mana < bird_output
        && bird_output < bird_added
        && bird_added < watchwolf_payment
        && forest_output < forest_added
        && watchwolf_payment < watchwolf_cast
        && watchwolf_cast < watchwolf_resolved
        && watchwolf_resolved < entry
        && entry < attack
        && attack < damage
        && damage < terminal)
    {
        return Err(
            "Birds mana, Watchwolf casting, combat, and terminal receipts are out of order"
                .to_owned(),
        );
    }
    let selected = [
        bird_paid,
        bird_cast,
        bird_resolved,
        bird_entry,
        cast_mana,
        bird_output,
        bird_added,
        forest_activation,
        forest_output,
        forest_added,
        watchwolf_payment,
        watchwolf_cast,
        watchwolf_resolved,
        entry,
        attack,
        damage,
        terminal,
    ];
    let selected_set: HashSet<_> = selected.into_iter().collect();
    let projected = project_events(events);
    let projected_events: Vec<_> = projected
        .into_iter()
        .filter(|event| selected_set.contains(&(event.sequence as usize)))
        .collect();
    let sequence: Vec<_> = projected_events
        .iter()
        .map(|event| event.sequence)
        .collect();
    Ok(json!({
        "id": "rav-birds-of-paradise-chosen-color-mana-full-game",
        "seed": seed,
        "decks": ["rav-birds-watchwolf-60", "rav-plains-pass-60"],
        "policies": ["rav.birds-of-paradise-chosen-color-full-game.v1", "rav.birds-of-paradise-empty-board.v1"],
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": 0,
        "turns": capture.turns,
        "accepted_moves": capture.accepted_moves,
        "rejected_moves": capture.rejected_moves,
        "birds_object": bird.id.0,
        "birds_characteristics": {"power": 0, "toughness": 1, "flying": true},
        "chosen_mana_source": bird.id.0,
        "chosen_mana_color": "White",
        "mana_tapped": true,
        "watchwolf_object": watchwolf.0,
        "watchwolf_characteristics": {"power": 3, "toughness": 3},
        "watchwolf_cast_payment": ["Green", "White"],
        "combat_damage": 3,
        "ordered_event_sequence": sequence,
        "events": projected_events,
        "passed": true
    }))
}

fn find_event(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}

fn find_event_after(
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

fn find_event_before(
    events: &[GameEvent],
    before: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events.iter().take(before).rposition(predicate)
}

fn unique_event(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    let mut found = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let first = found.next()?;
    found.next().is_none().then_some(first)
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => Some(definition_id),
        CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = definition_id == Some(CARD_ID);
    json!({
        "collector_number": card.collector_number,
        "name": card.name,
        "definition_id": definition_id,
        "engine_status": match card.semantic_status {
            CardSemanticStatus::ExecutableCompatibilitySlice { .. } => "executable_compatibility_slice",
            CardSemanticStatus::CatalogOnly { .. } => "catalog_only"
        },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "credited": false,
        "qualification": if passed { "pilot_trace_passed; independent reviewer approval pending" } else { "not_evaluated" }
    })
}
