//! Full-game diagnostic authority for RAV Blazing Archon.

use std::collections::BTreeSet;
use std::process::ExitCode;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, DeckEntry, DeckList, Game, GameEvent, GameView,
    Keyword, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    card_definitions, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings, rav_static_attack_restriction_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use syn::{Expr, Item, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-BLAZING-ARCHON";
const CARD_NAME: &str = "Blazing Archon";
const ATTACKER_ID: &str = "RAV-ELVES-OF-DEEP-SHADOW";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/blazing_archon.rs";
const MAX_TRACE_SEED: u64 = 64;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/blazing_archon.rs"]
mod candidate_blazing_archon;

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
            eprintln!("Blazing Archon PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_registry()?;
    let printings = rav_main_set_catalog();
    let mut names = BTreeSet::new();
    let catalog: Vec<_> = printings
        .iter()
        .copied()
        .filter(|card| names.insert(card.name))
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
    if captured != replay {
        return Err("Blazing Archon deterministic full-game replay differed".to_owned());
    }
    let mutations = json!({
        "opponents_can_attack_mutation_detected": mutation_detected(seed, Mutation::NoAttackRestriction),
        "flying_removed_mutation_detected": mutation_detected(seed, Mutation::NoFlying),
        "sealed_mutation_authority": false
    });
    for key in [
        "opponents_can_attack_mutation_detected",
        "flying_removed_mutation_detected",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Blazing Archon compiled semantic mutant escaped: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 4, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": ["302.1-302.2 Creature permanents and control", "508.1 Declaring attackers", "509 Declaring blockers", "601.2 Casting spells and paying costs", "608.2 Resolving spells", "702.9 Flying"],
            "source_contract": "RAV Blazing Archon #4, {6}{W}{W}{W} white 5/6 Creature with Flying; opponents cannot attack its controller while it is on the battlefield.",
            "authority_cutoff": "2026-09-24",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "deterministic_replay": true,
        "cards_passed": 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": printings.len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "diagnostic_only; external qualification pending",
        "mutation_sensitivity": mutations,
        "traces": [trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    NoAttackRestriction,
    NoFlying,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OpponentAttack {
    turn: u32,
    attackers: Vec<ObjectId>,
    archon_live: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    archon: Option<ObjectId>,
    archon_payment: Vec<Color>,
    archon_stats: Option<(i32, i32, bool)>,
    archon_zone: Option<Zone>,
    archon_entry_turn: Option<u32>,
    opponent_attacks: Vec<OpponentAttack>,
    archon_attack_batches: Vec<Vec<ObjectId>>,
    archon_damage: Vec<i32>,
    life: [i64; 2],
}

fn candidate_value(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_blazing_archon::definition();
    if mutation == Mutation::NoFlying {
        definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Flying);
    }
    definition
}

fn validate_candidate_registry() -> Result<(), String> {
    let definition = candidate_blazing_archon::definition();
    let runtime = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Blazing Archon is missing from the RAV card registry")?;
    let binding = candidate_blazing_archon::static_attack_restriction_binding();
    let runtime_binding = rav_static_attack_restriction_bindings()
        .into_iter()
        .find(|item| item.card_definition == CARD_ID)
        .ok_or("Blazing Archon attack restriction is missing from the RAV registry")?;
    if definition != runtime
        || binding != runtime_binding
        || definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(6, [Color::White; 3])
        || definition.colors != BTreeSet::from([Color::White])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Creature])
        || definition.is_basic_land
        || definition.supported_rules
            != [
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "flying",
                "static-opponents-cannot-attack-controller",
            ]
        || definition.power != Some(5)
        || definition.toughness != Some(6)
        || definition.keywords != [Keyword::Flying]
        || !definition.effects.is_empty()
        || binding.card_definition != CARD_ID
        || binding.restriction
            != cardbench_magic_engine::StaticAttackRestriction::OpponentsCannotAttackController
    {
        return Err("Blazing Archon identity, {6}{W}{W}{W}, white 5/6 Flying body, or controller-protection binding differs from source contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Blazing Archon source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Blazing Archon source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
        || file.items.len() != 4
    {
        return Err("Blazing Archon module must contain private imports and its two direct-literal functions".to_owned());
    }
    let mut imports = 0;
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                imports += 1;
            }
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                let output = match name.as_str() {
                    "definition" => "CardDefinition",
                    "static_attack_restriction_binding" => "StaticAttackRestrictionBinding",
                    _ => {
                        return Err(
                            "Blazing Archon source contains an unexpected function".to_owned()
                        );
                    }
                };
                if !functions.insert(name.clone())
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !function.sig.inputs.is_empty()
                    || function.sig.constness.is_some()
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || !function.sig.generics.params.is_empty()
                    || function.sig.generics.where_clause.is_some()
                    || !matches!(&function.sig.output, syn::ReturnType::Type(_, ty) if matches!(ty.as_ref(), syn::Type::Path(path) if path.path.is_ident(output)))
                    || function.block.stmts.len() != 1
                    || !matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(value), None) if value.rest.is_none())
                {
                    return Err(format!(
                        "Blazing Archon {name} must be a private direct-literal function"
                    ));
                }
            }
            _ => return Err("Blazing Archon source contains executable module items".to_owned()),
        }
    }
    if imports != 2
        || functions
            != BTreeSet::from([
                "definition".to_owned(),
                "static_attack_restriction_binding".to_owned(),
            ])
    {
        return Err("Blazing Archon source shape differs from its candidate contract".to_owned());
    }
    Ok(())
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|argument| argument == "--candidate") {
        "agent"
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
    let mut definitions = card_definitions();
    *definitions
        .iter_mut()
        .find(|definition| definition.id == CARD_ID)
        .ok_or("Blazing Archon definition missing during game construction")? =
        candidate_value(mutation);
    let mut restrictions = rav_static_attack_restriction_bindings();
    let index = restrictions
        .iter()
        .position(|binding| binding.card_definition == CARD_ID)
        .ok_or("Blazing Archon restriction binding missing during game construction")?;
    if mutation == Mutation::NoAttackRestriction {
        restrictions.remove(index);
    } else {
        restrictions[index] = candidate_blazing_archon::static_attack_restriction_binding();
    }
    let mut game = Game::new_with_mana_abilities_and_basic_land_types(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
    )
    .map_err(|error| error.to_string())?;
    game.register_static_attack_restrictions(restrictions)
        .map_err(|error| error.to_string())?;
    Ok(game)
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_BLAZING_ARCHON_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_BLAZING_ARCHON_SEED must be an unsigned integer".to_owned())?;
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
        "no Blazing Archon full-game restriction trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut game = new_game(mutation)?;
    let player0 = deck(&[(CARD_ID, 4), ("RAV-PLAINS", 56)]);
    let player1 = deck(&[(ATTACKER_ID, 4), ("RAV-FOREST", 56)]);
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    for (player, list) in [(PlayerId(0), &player0), (PlayerId(1), &player1)] {
        if list
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
        {
            return Err("Blazing Archon fixtures must be legal 60-card decks".to_owned());
        }
        game.load_deck_into_library(player, list)
            .map_err(|error| error.to_string())?;
        game.draw_opening_hand(player, 7)
            .map_err(|error| error.to_string())?;
    }
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(ArchonPolicy::new(PlayerId(0))),
        Box::new(AttackerPolicy::new(
            PlayerId(1),
            mutation == Mutation::NoAttackRestriction,
        )),
    ];
    let mut captured = Captured {
        events: vec![],
        winner: None,
        turns: 0,
        accepted: 0,
        rejected: 0,
        archon: None,
        archon_payment: vec![],
        archon_stats: None,
        archon_zone: None,
        archon_entry_turn: None,
        opponent_attacks: vec![],
        archon_attack_batches: vec![],
        archon_damage: vec![],
        life: [20, 20],
    };
    while !game.is_game_over() {
        if game.turn > 36 || captured.accepted >= 5000 {
            return Err("Blazing Archon full-game match exceeded its bound".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Blazing policy seat missing")?;
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
        let before = game.event_log.len();
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Blazing Archon policy action failed on turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        captured.accepted += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
        observe(&game, &game.event_log[before..], &mut captured)?;
    }
    captured.events = game.event_log.clone();
    captured.winner = game.winner().map(|winner| winner.0);
    captured.turns = game.turn;
    captured.archon_zone = captured.archon.and_then(|card| game.zone_of(card));
    captured.life =
        [PlayerId(0), PlayerId(1)].map(|player| game.player(player).map_or(0, |state| state.life));
    captured.rejected = 0;
    Ok(captured)
}

fn observe(game: &Game, events: &[GameEvent], captured: &mut Captured) -> Result<(), String> {
    for event in events {
        match event {
            GameEvent::SpellCast {
                player: PlayerId(0),
                card,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                captured.archon = Some(*card);
            }
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if game
                .card_definition(*card)
                .is_ok_and(|definition| definition.id == CARD_ID) =>
            {
                captured.archon_payment = colors.clone();
            }
            GameEvent::CardMoved {
                card,
                to: Zone::Battlefield,
            } if Some(*card) == captured.archon => {
                captured.archon_entry_turn.get_or_insert(game.turn);
            }
            GameEvent::AttackersDeclared {
                player: PlayerId(1),
                attackers,
            } => {
                let live = captured
                    .archon
                    .is_some_and(|archon| game.zone_of(archon) == Some(Zone::Battlefield));
                captured.opponent_attacks.push(OpponentAttack {
                    turn: game.turn,
                    attackers: attackers.clone(),
                    archon_live: live,
                });
            }
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } => {
                if let Some(archon) = captured.archon {
                    if attackers.contains(&archon) {
                        captured.archon_attack_batches.push(attackers.clone());
                    }
                }
            }
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if Some(*source) == captured.archon => {
                captured.archon_damage.push(*amount);
            }
            _ => {}
        }
    }
    if captured.archon_stats.is_none()
        && let Some(archon) = captured.archon
        && game.zone_of(archon) == Some(Zone::Battlefield)
    {
        let stats = game
            .characteristics(archon)
            .map_err(|error| error.to_string())?;
        captured.archon_stats = Some((
            stats.power.ok_or("Blazing Archon has no live power")?,
            stats
                .toughness
                .ok_or("Blazing Archon has no live toughness")?,
            stats.keywords.contains(&Keyword::Flying),
        ));
    }
    Ok(())
}

struct ArchonPolicy {
    player: PlayerId,
    archon_cast: bool,
}
impl ArchonPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            archon_cast: false,
        }
    }
}
impl CodePolicy for ArchonPolicy {
    fn id(&self) -> &'static str {
        "rav.blazing-archon-defend-and-attack.v1"
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
        if view.priority != self.player
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            if let Some(plains) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-PLAINS"))
            {
                return PolicyAction::PlayLand { card: plains.id };
            }
        }
        if !self.archon_cast {
            let archon = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID));
            if let Some(archon) = archon {
                if view.mana_pool.total() >= 9 {
                    self.archon_cast = true;
                    return PolicyAction::CastWithPayment {
                        request: CastRequest {
                            card: archon.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: ManaPaymentSelection {
                            generic: vec![Color::White; 6],
                            hybrid: vec![],
                        },
                    };
                }
            }
            if let Some(plains) = view
                .own_battlefield
                .iter()
                .find(|card| !card.tapped && card.definition == Some("RAV-PLAINS"))
            {
                return PolicyAction::ActivateManaAbility {
                    land: plains.id,
                    color: Color::White,
                };
            }
        }
        PolicyAction::PassPriority
    }
}

struct AttackerPolicy {
    player: PlayerId,
    cast_attacker: bool,
    force_through_archon: bool,
}
impl AttackerPolicy {
    fn new(player: PlayerId, force_through_archon: bool) -> Self {
        Self {
            player,
            cast_attacker: false,
            force_through_archon,
        }
    }
}
impl CodePolicy for AttackerPolicy {
    fn id(&self) -> &'static str {
        "rav.blazing-archon-opponent-attacker.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let archon_visible = view
                .opponent_battlefield
                .iter()
                .any(|card| card.definition == Some(CARD_ID));
            let attackers = if archon_visible && !self.force_through_archon {
                vec![]
            } else {
                view.own_battlefield
                    .iter()
                    .filter(|card| card.definition == Some(ATTACKER_ID) && card.can_attack)
                    .map(|card| card.id)
                    .collect()
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
        if view.priority != self.player
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0 {
            if let Some(forest) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-FOREST"))
            {
                return PolicyAction::PlayLand { card: forest.id };
            }
        }
        if !self.cast_attacker {
            if let Some(attacker) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(ATTACKER_ID))
            {
                if view.mana_pool.amount(Color::Green) > 0 {
                    self.cast_attacker = true;
                    return PolicyAction::CastWithPayment {
                        request: CastRequest {
                            card: attacker.id,
                            targets: vec![],
                            convoke: vec![],
                            payment_mana_abilities: vec![],
                        },
                        chosen_x: None,
                        mana_selection: ManaPaymentSelection::default(),
                    };
                }
                if let Some(forest) = view
                    .own_battlefield
                    .iter()
                    .find(|card| !card.tapped && card.definition == Some("RAV-FOREST"))
                {
                    return PolicyAction::ActivateManaAbility {
                        land: forest.id,
                        color: Color::Green,
                    };
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.winner != Some(0) || captured.rejected != 0 || captured.accepted == 0 {
        return Err("Blazing Archon duel did not finish as a clean P0 win".to_owned());
    }
    let archon = captured.archon.ok_or("Blazing Archon was not cast")?;
    if captured.archon_zone != Some(Zone::Battlefield)
        || captured.archon_stats != Some((5, 6, true))
    {
        return Err(format!(
            "live Blazing Archon is not a 5/6 Flying creature on the battlefield: {:?}/{:?}",
            captured.archon_stats, captured.archon_zone
        ));
    }
    if captured.archon_payment != [Color::White; 9] {
        return Err(format!(
            "Blazing Archon did not pay exactly six generic plus three white mana: {:?}",
            captured.archon_payment
        ));
    }
    let early = captured
        .opponent_attacks
        .iter()
        .find(|attack| !attack.archon_live && !attack.attackers.is_empty())
        .ok_or("opponent did not demonstrate an earlier legal attack before Archon entered")?;
    let protected = captured.opponent_attacks.iter().find(|attack| attack.archon_live && attack.attackers.is_empty())
        .ok_or("opponent did not make an empty attacker declaration while Archon protected its controller")?;
    if captured
        .opponent_attacks
        .iter()
        .any(|attack| attack.archon_live && !attack.attackers.is_empty())
    {
        return Err(
            "an opponent attacked while the live Blazing Archon protected its controller"
                .to_owned(),
        );
    }
    if captured.archon_attack_batches.is_empty()
        || captured
            .archon_attack_batches
            .iter()
            .any(|batch| !batch.contains(&archon))
        || captured.archon_damage.iter().sum::<i32>() < 20
        || captured.life[1] > 0
    {
        return Err(
            "Blazing Archon did not attack and deal enough combat damage for the terminal win"
                .to_owned(),
        );
    }
    let events = &captured.events;
    let payment = unique_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == archon && colors == &[Color::White; 9])).ok_or("exact nine-white SpellManaPaid receipt missing")?;
    let cast = unique_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == archon)).ok_or("Blazing Archon cast receipt missing")?;
    let resolve = unique_index(
        events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == archon),
    )
    .ok_or("Blazing Archon resolution receipt missing")?;
    let entry = unique_index(events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == archon)).ok_or("Blazing Archon battlefield entry receipt missing")?;
    let early_index = first_index(
        events,
        |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(1), attackers } if !attackers.is_empty()),
    );
    let protected_index = first_index_after(events, entry, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(1), attackers } if attackers.is_empty())).ok_or("typed empty opponent AttackersDeclared receipt after Archon entry missing")?;
    let archon_attack = first_index_after(events, entry, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&archon))).ok_or("Archon did not attack after entering")?;
    let damage = first_index_after(events, archon_attack, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 5 } if *source == archon)).ok_or("Archon did not deal five combat damage to P1")?;
    let ended = unique_index(events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal P0 game-end receipt missing")?;
    if payment >= cast
        || cast >= resolve
        || resolve >= entry
        || early_index.is_none_or(|index| index >= entry)
        || protected_index >= archon_attack
        || archon_attack >= damage
        || damage >= ended
        || protected.turn < captured.archon_entry_turn.unwrap_or_default()
        || early.turn >= protected.turn
    {
        return Err(format!(
            "Blazing Archon event order differs: payment={payment}, cast={cast}, entry={entry}, resolve={resolve}, early={early_index:?}/{}, protected={protected_index}/{}, entry_turn={:?}, archon_attack={archon_attack}, damage={damage}, ended={ended}",
            early.turn, protected.turn, captured.archon_entry_turn
        ));
    }
    let selected: BTreeSet<_> = events.iter().enumerate().filter_map(|(index, event)| {
        let include = matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, .. } if *card == archon)
            || matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } | GameEvent::SpellResolved { card } if *card == archon)
            || matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == archon)
            || matches!(event, GameEvent::AttackersDeclared { player: PlayerId(1), .. })
            || matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&archon))
            || matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), .. } if *source == archon)
            || matches!(event, GameEvent::GameEnded { winner: Some(PlayerId(0)) });
        include.then_some(index)
    }).collect();
    let all = project_events(events);
    let projected: Vec<_> = selected
        .into_iter()
        .map(|index| serde_json::to_value(&all[index]).map_err(|error| error.to_string()))
        .collect::<Result<_, _>>()?;
    let event_kinds: Vec<_> = projected
        .iter()
        .filter_map(|event| event["kind"].as_str())
        .collect();
    if !event_kinds.contains(&"SpellManaPaid")
        || !event_kinds.contains(&"SpellCast")
        || !event_kinds.contains(&"SpellResolved")
        || !event_kinds.contains(&"AttackersDeclared")
        || !event_kinds.contains(&"DamageDealtToPlayer")
        || !event_kinds.contains(&"GameEnded")
    {
        return Err(
            "projected Archon trace omitted required cast, protection, or terminal combat events"
                .to_owned(),
        );
    }
    let attack_rows: Vec<_> = captured.opponent_attacks.iter().map(|attack| json!({
        "turn": attack.turn, "attackers": attack.attackers.iter().map(|id| id.0).collect::<Vec<_>>(), "archon_live": attack.archon_live
    })).collect();
    Ok(json!({
        "trace_id": "rav-blazing-archon-prevent-opponent-attacks-terminal-combat",
        "shuffle_seed": seed,
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": 0,
        "turns": captured.turns,
        "accepted_moves": captured.accepted,
        "rejected_moves": captured.rejected,
        "decks": ["rav_blazing_archon_plains_60", "rav_elves_forest_60"],
        "policies": ["rav.blazing-archon-defend-and-attack.v1", "rav.blazing-archon-opponent-attacker.v1"],
        "passed": true,
        "blazing_archon": {
            "object": archon.0, "collector_number": 4, "mana_cost": "{6}{W}{W}{W}",
            "mana_paid": captured.archon_payment.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "live_characteristics": {"colors": ["White"], "card_types": ["Creature"], "power": 5, "toughness": 6, "flying": true},
            "zone_after_game": format!("{:?}", captured.archon_zone), "entry_turn": captured.archon_entry_turn,
            "opponent_attacks": attack_rows,
            "empty_attacker_declaration_while_live": protected.attackers.is_empty(),
            "archon_attack_batches": captured.archon_attack_batches.iter().map(|batch| batch.iter().map(|id| id.0).collect::<Vec<_>>()).collect::<Vec<_>>(),
            "combat_damage_to_opponent": captured.archon_damage,
            "life_totals": captured.life
        },
        "events": projected
    }))
}

fn unique_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    let mut indices = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let first = indices.next()?;
    indices.next().is_none().then_some(first)
}
fn first_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn first_index_after(
    events: &[GameEvent],
    index: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(index + 1)
        .find_map(|(index, event)| predicate(event).then_some(index))
}
fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    run_match(seed, mutation)
        .and_then(|captured| validate_trace(&captured, seed))
        .is_err()
}
fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        cardbench_magic_rav::CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => {
            Some(definition_id)
        }
        cardbench_magic_rav::CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 4;
    json!({
        "collector_number": card.collector_number, "name": card.name, "definition_id": definition_id,
        "engine_status": if definition_id.is_some() { "executable_compatibility_slice" } else { "catalog_only" },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "qualification": if passed { "diagnostic_only" } else { "not_evaluated" }, "credited": false
    })
}
