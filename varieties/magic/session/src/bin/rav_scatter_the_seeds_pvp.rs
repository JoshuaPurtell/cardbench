//! Full-game diagnostic authority for RAV Scatter the Seeds.

use std::collections::{BTreeSet, HashSet};
use std::process::ExitCode;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, ConvokeContribution, ConvokePayment,
    CreatureSubtype, DeckEntry, DeckList, Effect, Game, GameEvent, GameView, Keyword, ManaCost,
    ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step, TokenSpec, Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    card_definitions, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings,
};
use cardbench_magic_session::project_events;
use serde_json::{Value, json};
use syn::{Expr, Item, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-SCATTER-THE-SEEDS";
const CARD_NAME: &str = "Scatter the Seeds";
const ELVES_ID: &str = "RAV-ELVES-OF-DEEP-SHADOW";
const FOREST_ID: &str = "RAV-FOREST";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/scatter_the_seeds.rs";
const MAX_TRACE_SEED: u64 = 48;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/scatter_the_seeds.rs"]
mod candidate_scatter_the_seeds;

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
            eprintln!("Scatter the Seeds PvP authority failed: {error}");
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
    if !same_match(&captured, &replay) {
        return Err("Scatter the Seeds deterministic full-game replay differed".to_owned());
    }
    let mutations = json!({
        "convoke_removed_mutation_detected": mutation_detected(seed, Mutation::NoConvoke),
        "generic_cost_increased_mutation_detected": mutation_detected(seed, Mutation::MoreGeneric),
        "two_token_mutation_detected": mutation_detected(seed, Mutation::TwoTokens),
        "wrong_saproling_body_mutation_detected": mutation_detected(seed, Mutation::WrongTokenBody),
        "sealed_mutation_authority": false
    });
    for key in [
        "convoke_removed_mutation_detected",
        "generic_cost_increased_mutation_detected",
        "two_token_mutation_detected",
        "wrong_saproling_body_mutation_detected",
    ] {
        if mutations[key] != true {
            return Err(format!("Scatter the Seeds semantic mutant escaped: {key}"));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 181, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": ["601.2 Casting spells and paying costs", "608.2 Resolving spells", "702.50 Convoke", "111.1 and 111.3 Token creation and characteristics", "508.1 Declaring attackers", "510 Combat damage"],
            "source_contract": "RAV Scatter the Seeds #181, {3}{G}{G} green Instant with Convoke that creates three green 1/1 Saproling creature tokens.",
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
    NoConvoke,
    MoreGeneric,
    TwoTokens,
    WrongTokenBody,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TokenWitness {
    id: ObjectId,
    spec: Option<TokenSpec>,
    colors: BTreeSet<Color>,
    card_types: BTreeSet<CardType>,
    creature_subtypes: BTreeSet<CreatureSubtype>,
    power: Option<i32>,
    toughness: Option<i32>,
    keywords: Vec<Keyword>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    spell: Option<ObjectId>,
    spell_zone: Option<Zone>,
    convokers: Vec<ObjectId>,
    convoker_tapped: Vec<bool>,
    mana_paid: Vec<Color>,
    resolved: bool,
    tokens: Vec<TokenWitness>,
    attack_batches: Vec<Vec<ObjectId>>,
    token_damage: Vec<(ObjectId, i32)>,
    opponent_life: Option<i64>,
}

fn candidate_value(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_scatter_the_seeds::definition();
    match mutation {
        Mutation::None => {}
        Mutation::NoConvoke => definition
            .keywords
            .retain(|keyword| *keyword != Keyword::Convoke),
        Mutation::MoreGeneric => {
            definition.mana_cost = ManaCost::with_colors(4, [Color::Green, Color::Green]);
        }
        Mutation::TwoTokens => {
            if let Some(Effect::CreateToken { count, .. }) = definition.effects.first_mut() {
                *count = 2;
            }
        }
        Mutation::WrongTokenBody => {
            let mut token = TokenSpec::saproling();
            token.power = 2;
            definition.effects = vec![Effect::CreateToken { token, count: 3 }];
        }
    }
    definition
}

fn validate_candidate_registry() -> Result<(), String> {
    let definition = candidate_scatter_the_seeds::definition();
    let runtime = card_definitions()
        .into_iter()
        .find(|card| card.id == CARD_ID)
        .ok_or("Scatter the Seeds is missing from the RAV definition registry")?;
    if runtime != definition
        || definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(3, [Color::Green, Color::Green])
        || definition.colors != BTreeSet::from([Color::Green])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Instant])
        || definition.is_basic_land
        || definition.supported_rules != ["full-rules-fidelity", "convoke", "token-creation"]
        || definition.power.is_some()
        || definition.toughness.is_some()
        || definition.keywords != [Keyword::Convoke]
        || definition.effects
            != [Effect::CreateToken {
                token: TokenSpec::saproling(),
                count: 3,
            }]
    {
        return Err("Scatter the Seeds identity, exact {3}{G}{G} cost, green Instant, Convoke, or three 1/1 Saprolings differs from source contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Scatter the Seeds source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Scatter the Seeds source is not valid Rust".to_owned())?;
    if !file
        .attrs
        .iter()
        .all(|attribute| attribute.path().is_ident("doc"))
        || file.items.len() != 3
    {
        return Err(
            "Scatter the Seeds module must contain only private imports and its definition"
                .to_owned(),
        );
    }
    let mut definition_count = 0;
    let mut use_count = 0;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) =>
            {
                use_count += 1;
            }
            Item::Fn(function) => {
                definition_count += 1;
                if function.sig.ident != "definition"
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !function.sig.inputs.is_empty()
                    || function.sig.constness.is_some()
                    || function.sig.asyncness.is_some()
                    || function.sig.unsafety.is_some()
                    || function.sig.generics.params.len() != 0
                    || function.block.stmts.len() != 1
                    || !matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(_), None))
                {
                    return Err("Scatter the Seeds candidate must be one private, direct-literal definition function".to_owned());
                }
            }
            _ => {
                return Err(
                    "Scatter the Seeds module contains executable or unsupported items".to_owned(),
                );
            }
        }
    }
    if use_count != 2 || definition_count != 1 {
        return Err(
            "Scatter the Seeds module imports or definition shape differ from the source contract"
                .to_owned(),
        );
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
        .ok_or("Scatter the Seeds definition missing during game construction")? =
        candidate_value(mutation);
    Game::new_with_mana_abilities_and_basic_land_types(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
    )
    .map_err(|error| error.to_string())
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_SCATTER_THE_SEEDS_SEED") {
        let seed = value.parse::<u64>().map_err(|_| {
            "CARDBENCH_SCATTER_THE_SEEDS_SEED must be an unsigned integer".to_owned()
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
        "no Scatter the Seeds full-game Convoke trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut game = new_game(mutation)?;
    let p0 = deck(&[(CARD_ID, 4), (ELVES_ID, 12), (FOREST_ID, 44)]);
    let p1 = deck(&[("RAV-ISLAND", 60)]);
    game.set_shuffle_seed(seed)
        .map_err(|error| error.to_string())?;
    for (player, list) in [(PlayerId(0), &p0), (PlayerId(1), &p1)] {
        if list
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
        {
            return Err("Scatter the Seeds fixtures must be legal 60-card decks".to_owned());
        }
        game.load_deck_into_library(player, list)
            .map_err(|error| error.to_string())?;
        game.draw_opening_hand(player, 7)
            .map_err(|error| error.to_string())?;
    }
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(ScatterPolicy::new(PlayerId(0))),
        Box::new(PassivePolicy::new(PlayerId(1))),
    ];
    let mut captured = Captured {
        events: vec![],
        winner: None,
        turns: 0,
        accepted: 0,
        rejected: 0,
        spell: None,
        spell_zone: None,
        convokers: vec![],
        convoker_tapped: vec![],
        mana_paid: vec![],
        resolved: false,
        tokens: vec![],
        attack_batches: vec![],
        token_damage: vec![],
        opponent_life: None,
    };
    while !game.is_game_over() {
        if game.turn > 36 || captured.accepted >= 5000 {
            return Err("Scatter the Seeds full-game trace exceeded its bounded match".to_owned());
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Scatter policy seat missing")?;
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
                    "Scatter policy move rejected on turn {} {:?}: {error}",
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
    captured.spell_zone = captured.spell.and_then(|card| game.zone_of(card));
    captured.opponent_life = game.player(PlayerId(1)).ok().map(|player| player.life);
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
                captured.spell = Some(*card);
            }
            GameEvent::ConvokeUsed {
                player: PlayerId(0),
                creature,
                contribution: None,
            } => {
                captured.convokers.push(*creature);
                captured.convoker_tapped.push(
                    game.object(*creature)
                        .map_err(|error| error.to_string())?
                        .tapped,
                );
            }
            GameEvent::SpellManaPaid {
                player: PlayerId(0),
                card,
                colors,
            } if Some(*card) == captured.spell
                || game.card_definition(*card).is_ok_and(|d| d.id == CARD_ID) =>
            {
                captured.mana_paid = colors.clone();
            }
            GameEvent::SpellResolved { card } if Some(*card) == captured.spell => {
                captured.resolved = true
            }
            GameEvent::TokenCreated {
                player: PlayerId(0),
                token,
            } => {
                if !captured.tokens.iter().any(|witness| witness.id == *token) {
                    let stats = game
                        .characteristics(*token)
                        .map_err(|error| error.to_string())?;
                    let spec = game
                        .object(*token)
                        .map_err(|error| error.to_string())?
                        .token
                        .clone();
                    captured.tokens.push(TokenWitness {
                        id: *token,
                        spec,
                        colors: stats.colors,
                        card_types: stats.card_types,
                        creature_subtypes: stats.creature_subtypes,
                        power: stats.power,
                        toughness: stats.toughness,
                        keywords: stats.keywords,
                    });
                }
            }
            GameEvent::AttackersDeclared {
                player: PlayerId(0),
                attackers,
            } => {
                let token_ids: HashSet<_> = captured.tokens.iter().map(|token| token.id).collect();
                let attacking_tokens: Vec<_> = attackers
                    .iter()
                    .copied()
                    .filter(|id| token_ids.contains(id))
                    .collect();
                if !attacking_tokens.is_empty() {
                    captured.attack_batches.push(attacking_tokens);
                }
            }
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } => {
                if captured.tokens.iter().any(|token| token.id == *source) {
                    captured.token_damage.push((*source, *amount));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

struct ScatterPolicy {
    player: PlayerId,
    scatter_cast: bool,
}

impl ScatterPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            scatter_cast: false,
        }
    }

    fn mana_source(view: &GameView) -> Option<PolicyAction> {
        view.own_battlefield
            .iter()
            .find(|card| {
                !card.tapped
                    && card.definition == Some(FOREST_ID)
                    && card.mana_colors.contains(&Color::Green)
            })
            .map(|card| PolicyAction::ActivateManaAbility {
                land: card.id,
                color: Color::Green,
            })
    }

    fn play_land(view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        view.hand
            .iter()
            .find(|card| card.definition == Some(FOREST_ID))
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }

    fn cast_elf(view: &GameView) -> Option<PolicyAction> {
        let elves = view
            .own_battlefield
            .iter()
            .filter(|card| card.definition == Some(ELVES_ID))
            .count();
        if elves >= 2 {
            return None;
        }
        let card = view
            .hand
            .iter()
            .find(|card| card.definition == Some(ELVES_ID))?;
        if view.mana_pool.amount(Color::Green) == 0 {
            return Self::mana_source(view);
        }
        Some(PolicyAction::CastWithPayment {
            request: CastRequest {
                card: card.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection::default(),
        })
    }

    fn cast_scatter(&mut self, view: &GameView) -> Option<PolicyAction> {
        if self.scatter_cast
            || !view
                .hand
                .iter()
                .any(|card| card.definition == Some(CARD_ID))
        {
            return None;
        }
        let lands = view
            .own_battlefield
            .iter()
            .filter(|card| card.definition == Some(FOREST_ID))
            .count();
        let creatures: Vec<_> = view
            .own_battlefield
            .iter()
            .filter(|card| {
                card.card_types.contains(&CardType::Creature)
                    && !card.tapped
                    && !card.summoning_sick
            })
            .take(2)
            .collect();
        if lands < 5 || creatures.len() < 2 {
            return None;
        }
        if view.mana_pool.amount(Color::Green) < 3 {
            return Self::mana_source(view);
        }
        let spell = view
            .hand
            .iter()
            .find(|card| card.definition == Some(CARD_ID))?;
        self.scatter_cast = true;
        Some(PolicyAction::CastWithPayment {
            request: CastRequest {
                card: spell.id,
                targets: vec![],
                convoke: creatures
                    .iter()
                    .map(|creature| ConvokePayment {
                        creature: creature.id,
                        contribution: ConvokeContribution::Generic,
                    })
                    .collect(),
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection {
                generic: vec![Color::Green],
                hybrid: vec![],
            },
        })
    }
}

impl CodePolicy for ScatterPolicy {
    fn id(&self) -> &'static str {
        "rav.scatter-the-seeds-convoke-tokens.v1"
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
                    .filter(|card| card.definition.is_none() && card.can_attack)
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
        if let Some(action) = Self::play_land(view) {
            return action;
        }
        if let Some(action) = self.cast_scatter(view) {
            return action;
        }
        if let Some(action) = Self::cast_elf(view) {
            return action;
        }
        PolicyAction::PassPriority
    }
}

struct PassivePolicy {
    player: PlayerId,
}
impl PassivePolicy {
    fn new(player: PlayerId) -> Self {
        Self { player }
    }
}
impl CodePolicy for PassivePolicy {
    fn id(&self) -> &'static str {
        "rav.scatter-the-seeds-passive-opponent.v1"
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player == self.player
            && view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            return PolicyAction::DeclareAttackers { attackers: vec![] };
        }
        if view.player == self.player
            && view.step == Step::DeclareBlockers
            && !view.blockers_declared
        {
            PolicyAction::DeclareBlockers {
                assignments: vec![],
            }
        } else {
            PolicyAction::PassPriority
        }
    }
}

fn validate_trace(captured: &Captured, seed: u64) -> Result<Value, String> {
    if captured.winner != Some(0) || captured.rejected != 0 || captured.accepted == 0 {
        return Err("Scatter the Seeds full game did not end in a clean P0 win".to_owned());
    }
    let spell = captured.spell.ok_or("Scatter the Seeds was not cast")?;
    if captured.spell_zone != Some(Zone::Graveyard) || !captured.resolved {
        return Err("Scatter instant did not resolve and move to its owner's graveyard".to_owned());
    }
    if captured.convokers.len() != 2
        || captured.convokers.iter().collect::<BTreeSet<_>>().len() != 2
        || captured.convoker_tapped != [true, true]
    {
        return Err(format!(
            "Scatter did not tap two distinct creatures for generic Convoke: {:?}",
            captured.convokers
        ));
    }
    if captured.mana_paid != [Color::Green, Color::Green, Color::Green] {
        return Err(format!(
            "Scatter exact {{3}}{{G}}{{G}} payment must use three green mana plus two generic Convoke; got {:?}",
            captured.mana_paid
        ));
    }
    if captured.tokens.len() != 3 {
        return Err(format!(
            "Scatter must create exactly three Saprolings, got {}",
            captured.tokens.len()
        ));
    }
    let expected = TokenSpec::saproling();
    if captured.tokens.iter().any(|token| {
        token.spec.as_ref() != Some(&expected)
            || token.colors != BTreeSet::from([Color::Green])
            || token.card_types != BTreeSet::from([CardType::Creature])
            || token.creature_subtypes != BTreeSet::from([CreatureSubtype::Saproling])
            || token.power != Some(1)
            || token.toughness != Some(1)
            || !token.keywords.is_empty()
    }) {
        return Err(
            "live created token characteristics differ from green 1/1 Saproling creature tokens"
                .to_owned(),
        );
    }
    let token_ids: BTreeSet<_> = captured.tokens.iter().map(|token| token.id).collect();
    if captured.attack_batches.is_empty()
        || captured
            .attack_batches
            .iter()
            .any(|batch| batch.iter().any(|id| !token_ids.contains(id)))
    {
        return Err(
            "only live Scatter Saproling tokens must attack in the terminal full game".to_owned(),
        );
    }
    if captured.token_damage.is_empty()
        || captured
            .token_damage
            .iter()
            .any(|(id, amount)| !token_ids.contains(id) || *amount != 1)
        || captured
            .token_damage
            .iter()
            .map(|(_, amount)| amount)
            .sum::<i32>()
            < 20
        || !captured.opponent_life.is_some_and(|life| life <= 0)
    {
        return Err(format!(
            "Saproling combat damage did not cause the terminal opponent loss: damage={}, life={:?}, token_damage={:?}",
            captured
                .token_damage
                .iter()
                .map(|(_, amount)| amount)
                .sum::<i32>(),
            captured.opponent_life,
            captured.token_damage
        ));
    }

    let events = &captured.events;
    let convoke: Vec<_> = events.iter().enumerate().filter_map(|(index, event)| {
        matches!(event, GameEvent::ConvokeUsed { player: PlayerId(0), creature, contribution: None } if captured.convokers.contains(creature)).then_some(index)
    }).collect();
    let payment = unique_index(events, |event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == spell && colors == &[Color::Green, Color::Green, Color::Green])).ok_or("exact three-green SpellManaPaid receipt is missing")?;
    let cast = unique_index(events, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == spell)).ok_or("Scatter SpellCast receipt is missing")?;
    let resolution = unique_index(
        events,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == spell),
    )
    .ok_or("Scatter SpellResolved receipt is missing")?;
    let grave = unique_index(events, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == spell)).ok_or("resolved Scatter graveyard movement is missing")?;
    let tokens: Vec<_> = events.iter().enumerate().filter_map(|(index, event)| matches!(event, GameEvent::TokenCreated { player: PlayerId(0), token } if token_ids.contains(token)).then_some(index)).collect();
    let attack = unique_or_first(events, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.iter().any(|id| token_ids.contains(id)))).ok_or("typed Saproling AttackersDeclared receipt is missing")?;
    let damage = unique_or_first(events, |event| matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 1 } if token_ids.contains(source))).ok_or("typed Saproling combat damage receipt is missing")?;
    let end = unique_index(events, |event| {
        matches!(
            event,
            GameEvent::GameEnded {
                winner: Some(PlayerId(0))
            }
        )
    })
    .ok_or("terminal P0 GameEnded receipt is missing")?;
    if convoke.len() != 2
        || convoke.iter().any(|index| *index >= payment)
        || payment >= cast
        || cast >= tokens[0]
        || tokens.len() != 3
        || tokens[2] >= resolution
        || resolution >= grave
        || grave >= attack
        || attack >= damage
        || damage >= end
    {
        return Err(format!(
            "Convoke/payment/resolve/token/attack/damage/end receipts out of order: convoke={convoke:?}, payment={payment}, cast={cast}, resolved={resolution}, grave={grave}, tokens={tokens:?}, attack={attack}, damage={damage}, end={end}"
        ));
    }

    let all = project_events(events);
    let selected: BTreeSet<_> = events.iter().enumerate().filter_map(|(index, event)| {
        let include = matches!(event,
            GameEvent::SpellManaPaid { player: PlayerId(0), card, .. } if *card == spell
        ) || matches!(event, GameEvent::ConvokeUsed { player: PlayerId(0), creature, contribution: None } if captured.convokers.contains(creature))
            || matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } | GameEvent::SpellResolved { card } if *card == spell)
            || matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == spell)
            || matches!(event, GameEvent::TokenCreated { player: PlayerId(0), token } if token_ids.contains(token))
            || matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.iter().any(|id| token_ids.contains(id)))
            || matches!(event, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 1 } if token_ids.contains(source))
            || matches!(event, GameEvent::GameEnded { winner: Some(PlayerId(0)) });
        include.then_some(index)
    }).collect();
    let mut projected = Vec::new();
    for index in selected {
        let mut value =
            serde_json::to_value(all.get(index).ok_or("typed event projection missing")?)
                .map_err(|error| error.to_string())?;
        if let GameEvent::ConvokeUsed {
            player,
            creature,
            contribution: None,
        } = &events[index]
        {
            value["facts"] = json!({
                "seats": [player.0], "objects": [creature.0], "contribution": "Generic"
            });
        }
        projected.push(value);
    }
    let kinds: Vec<_> = projected
        .iter()
        .filter_map(|event| event["kind"].as_str())
        .collect();
    if kinds.iter().filter(|kind| **kind == "ConvokeUsed").count() != 2
        || kinds.iter().filter(|kind| **kind == "TokenCreated").count() != 3
        || !kinds.contains(&"AttackersDeclared")
        || !kinds.contains(&"DamageDealtToPlayer")
    {
        return Err("typed projection lost Convoke, token, or combat evidence".to_owned());
    }
    Ok(json!({
        "trace_id": "rav-scatter-the-seeds-convoke-three-saprolings-full-game",
        "shuffle_seed": seed,
        "terminal": "Winner(PlayerId(0))",
        "winner_seat": 0,
        "turns": captured.turns,
        "accepted_moves": captured.accepted,
        "rejected_moves": captured.rejected,
        "decks": ["rav_scatter_elves_forest_60", "rav_island_passive_60"],
        "policies": ["rav.scatter-the-seeds-convoke-tokens.v1", "rav.scatter-the-seeds-passive-opponent.v1"],
        "passed": true,
        "scatter_the_seeds": {
            "object": spell.0, "collector_number": 181, "mana_cost": "{3}{G}{G}", "card_types": ["Instant"], "colors": ["Green"],
            "mana_paid": captured.mana_paid.iter().map(|color| format!("{color:?}")).collect::<Vec<_>>(),
            "generic_paid_by_convoke": 2,
            "convoke_creatures": captured.convokers.iter().map(|id| id.0).collect::<Vec<_>>(),
            "convoke_contribution": "Generic", "convoke_creatures_tapped": captured.convoker_tapped,
            "resolved": captured.resolved, "zone_after_resolution": format!("{:?}", captured.spell_zone),
            "token_objects": captured.tokens.iter().map(|token| token.id.0).collect::<Vec<_>>(),
            "live_token_characteristics": {"name": "Saproling", "colors": ["Green"], "card_types": ["Creature"], "creature_subtypes": ["Saproling"], "power": 1, "toughness": 1, "keywords": []},
            "attack_batches": captured.attack_batches.iter().map(|batch| batch.iter().map(|id| id.0).collect::<Vec<_>>()).collect::<Vec<_>>(),
            "token_combat_damage": captured.token_damage.iter().map(|(id, amount)| json!({"source": id.0, "amount": amount})).collect::<Vec<_>>(),
            "opponent_life_after_combat": captured.opponent_life
        },
        "events": projected
    }))
}

fn unique_index(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    let mut matches = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| predicate(event).then_some(index));
    let first = matches.next()?;
    matches.next().is_none().then_some(first)
}

fn unique_or_first(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    run_match(seed, mutation)
        .and_then(|captured| validate_trace(&captured, seed))
        .is_err()
}

fn same_match(left: &Captured, right: &Captured) -> bool {
    left == right
}

fn card_status(card: &CatalogCard) -> Value {
    let definition_id = match card.semantic_status {
        cardbench_magic_rav::CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => {
            Some(definition_id)
        }
        cardbench_magic_rav::CardSemanticStatus::CatalogOnly { .. } => None,
    };
    let passed = card.collector_number == 181;
    json!({
        "collector_number": card.collector_number, "name": card.name, "definition_id": definition_id,
        "engine_status": if definition_id.is_some() { "executable_compatibility_slice" } else { "catalog_only" },
        "trace_status": if passed { "passed" } else { "not_qualified" },
        "qualification": if passed { "diagnostic_only" } else { "not_evaluated" },
        "credited": false
    })
}
