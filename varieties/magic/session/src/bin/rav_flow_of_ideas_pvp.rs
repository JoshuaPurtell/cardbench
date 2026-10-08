//! Full-game diagnostic authority for RAV Flow of Ideas.

use std::collections::{BTreeMap, BTreeSet};
use std::process::ExitCode;

use cardbench_magic_engine::{
    BasicLandType, CardDefinition, CardType, CastRequest, Color, DeckEntry, DeckList, Effect, Game,
    GameEvent, GameView, ManaCost, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step,
    Zone,
};
use cardbench_magic_policies::CodePolicy;
use cardbench_magic_protocol::ViewerScope;
use cardbench_magic_rav::{
    CatalogCard, RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    card_definitions, rav_basic_land_type_bindings, rav_main_set_catalog,
    rav_mana_ability_bindings,
};
use cardbench_magic_session::{ProjectedEvent, project_events};
use serde_json::{Value, json};
use syn::{Expr, Item, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-FLOW-OF-IDEAS";
const CARD_NAME: &str = "Flow of Ideas";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const ISLAND_ID: &str = "RAV-ISLAND";
const FOREST_ID: &str = "RAV-FOREST";
const PLAINS_ID: &str = "RAV-PLAINS";
const SOURCE_PATH: &str = "varieties/magic/sets/ravnica_city_of_guilds/src/cards/flow_of_ideas.rs";
const MAX_TRACE_SEED: u64 = 48;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/flow_of_ideas.rs"]
mod candidate_flow_of_ideas;

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
            eprintln!("Flow of Ideas PvP authority failed: {error}");
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
    if captured.events != replay.events
        || captured.winner != replay.winner
        || captured.flow_drawn != replay.flow_drawn
        || captured.islands_at_cast != replay.islands_at_cast
    {
        return Err("Flow of Ideas deterministic full-game replay differed".to_owned());
    }
    let mutations = json!({
        "wrong_exact_sorcery_cost_detected": mutation_detected(seed, Mutation::WrongCost),
        "wrong_card_color_detected": mutation_detected(seed, Mutation::WrongColor),
        "wrong_land_type_detected": mutation_detected(seed, Mutation::WrongLandType),
        "fixed_one_card_draw_detected": mutation_detected(seed, Mutation::DrawOne),
        "sealed_mutation_authority": false
    });
    for key in [
        "wrong_exact_sorcery_cost_detected",
        "wrong_card_color_detected",
        "wrong_land_type_detected",
        "fixed_one_card_draw_detected",
    ] {
        if mutations[key] != true {
            return Err(format!("Flow of Ideas semantic mutant escaped: {key}"));
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
        "card": {"collector_number": 50, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": ["105.2 and 202.1-202.2 colored mana", "117 priority and timing", "305.6 basic land types", "601.2 casting spells and paying costs", "608.2 resolving spells", "121.2-121.3 drawing cards", "510 combat damage"],
            "source_contract": "RAV Flow of Ideas #50, {5}{U} blue sorcery; draw a card for each Island you control.",
            "authority_cutoff": "2026-09-24",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "deterministic_replay": true,
        "cards_passed": cards_passed,
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
    WrongCost,
    WrongColor,
    WrongLandType,
    DrawOne,
}

#[derive(Clone, Debug)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    flow: Option<ObjectId>,
    flow_definition: Option<CardDefinition>,
    flow_zone: Option<Zone>,
    flow_drawn: Vec<ObjectId>,
    islands_at_cast: [usize; 2],
    watchwolf: Option<ObjectId>,
    player_life: [i64; 2],
}

fn candidate_value(mutation: Mutation) -> CardDefinition {
    let mut definition = candidate_flow_of_ideas::definition();
    match mutation {
        Mutation::None => {}
        Mutation::WrongCost => definition.mana_cost = ManaCost::new(5),
        Mutation::WrongColor => definition.colors = BTreeSet::from([Color::White]),
        Mutation::WrongLandType => {
            definition.effects = vec![Effect::DrawControllerForEachControlledBasicLandType {
                land_type: BasicLandType::Forest,
            }]
        }
        Mutation::DrawOne => definition.effects = vec![Effect::DrawController],
    }
    definition
}

fn validate_candidate_registry() -> Result<(), String> {
    let definition = candidate_flow_of_ideas::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|item| item.id == CARD_ID)
        .ok_or("Flow of Ideas is missing from the RAV definition registry")?;
    if registered != definition {
        return Err(
            "Flow of Ideas registry definition is not delegated to its candidate module".to_owned(),
        );
    }
    if definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != ManaCost::with_colors(5, [Color::Blue])
        || definition.colors != BTreeSet::from([Color::Blue])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Sorcery])
        || definition.is_basic_land
        || definition.supported_rules != ["full-rules-fidelity", "draw-for-each-controlled-island"]
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
        || definition.effects
            != [Effect::DrawControllerForEachControlledBasicLandType {
                land_type: BasicLandType::Island,
            }]
    {
        return Err("Flow of Ideas identity, exact {5}{U} cost, blue sorcery characteristics, or Island draw effect differs from the source contract".to_owned());
    }
    Ok(())
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|e| format!("could not read Flow of Ideas source: {e}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Flow of Ideas source is not valid Rust".to_owned())?;
    if !file.attrs.iter().all(|attr| attr.path().is_ident("doc")) || file.items.len() != 3 {
        return Err(
            "Flow of Ideas source must contain two private imports and one direct-literal function"
                .to_owned(),
        );
    }
    let mut functions = BTreeSet::new();
    for item in &file.items {
        match item {
            Item::Use(use_item)
                if use_item.attrs.is_empty() && matches!(&use_item.vis, Visibility::Inherited) => {}
            Item::Fn(function) => {
                let name = function.sig.ident.to_string();
                if !functions.insert(name.clone())
                    || name != "definition"
                    || !matches!(&function.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    || !function.sig.inputs.is_empty()
                    || function.sig.constness.is_some()
                    || function.sig.asyncness.is_some()
                    || function.block.stmts.len() != 1
                    || !matches!(&function.block.stmts[0], Stmt::Expr(Expr::Struct(_), None))
                {
                    return Err(
                        "Flow of Ideas module contains unsupported executable source".to_owned(),
                    );
                }
            }
            _ => return Err(
                "Flow of Ideas module permits only private imports and its declarative definition"
                    .to_owned(),
            ),
        }
    }
    if functions != BTreeSet::from(["definition".to_owned()]) {
        return Err("Flow of Ideas candidate definition function is missing".to_owned());
    }
    Ok(())
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
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
        .ok_or("Flow of Ideas definition missing during game construction")? =
        candidate_value(mutation);
    Game::new_with_mana_abilities_and_basic_land_types(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
    )
    .map_err(|e| e.to_string())
}

fn find_trace() -> Result<(u64, Captured), String> {
    if let Ok(value) = std::env::var("CARDBENCH_FLOW_OF_IDEAS_SEED") {
        let seed = value
            .parse::<u64>()
            .map_err(|_| "CARDBENCH_FLOW_OF_IDEAS_SEED must be an unsigned integer".to_owned())?;
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
        "no Flow of Ideas full-game trace found for seeds 0..={MAX_TRACE_SEED}"
    ))
}

fn mutation_detected(seed: u64, mutation: Mutation) -> bool {
    run_match(seed, mutation)
        .and_then(|capture| validate_trace(&capture, seed))
        .is_err()
}

fn run_match(seed: u64, mutation: Mutation) -> Result<Captured, String> {
    let mut game = new_game(mutation)?;
    let p0 = deck(&[
        (CARD_ID, 4),
        (WATCHWOLF_ID, 12),
        (ISLAND_ID, 28),
        (FOREST_ID, 8),
        (PLAINS_ID, 8),
    ]);
    let p1 = deck(&[(ISLAND_ID, 60)]);
    game.set_shuffle_seed(seed).map_err(|e| e.to_string())?;
    for (player, list) in [(PlayerId(0), &p0), (PlayerId(1), &p1)] {
        if list
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum::<usize>()
            != 60
        {
            return Err("Flow of Ideas duel decks must each contain 60 cards".to_owned());
        }
        game.load_deck_into_library(player, list)
            .map_err(|e| e.to_string())?;
        game.draw_opening_hand(player, 7)
            .map_err(|e| e.to_string())?;
    }
    game.begin_game().map_err(|e| e.to_string())?;
    let mut policies: [Box<dyn CodePolicy>; 2] = [
        Box::new(FlowPolicy::new(PlayerId(0))),
        Box::new(FlowPolicy::new(PlayerId(1))),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut islands_at_cast = None;
    while !game.is_game_over() {
        if game.turn > 60 || accepted >= 7_500 {
            return Err("Flow of Ideas full-game trace exceeded its bounded match".to_owned());
        }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).map_err(|e| e.to_string())?;
        let policy = policies
            .get_mut(player.0)
            .ok_or("Flow policy seat missing")?;
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
            .map_err(|e| {
                format!(
                    "Flow policy move rejected on turn {} {:?}: {e}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        if islands_at_cast.is_none()
            && game.event_log.iter().any(|event| {
                matches!(event, GameEvent::SpellCast { player: PlayerId(0), card }
                    if game.card_definition(*card).is_ok_and(|definition| definition.id == CARD_ID))
            })
        {
            islands_at_cast = Some([PlayerId(0), PlayerId(1)].map(|seat| {
                game.player(seat)
                    .map(|state| {
                        state
                            .battlefield
                            .iter()
                            .filter(|card| {
                                game.card_definition(**card)
                                    .is_ok_and(|definition| definition.id == ISLAND_ID)
                            })
                            .count()
                    })
                    .unwrap_or_default()
            }));
        }
        game.validate_invariants().map_err(|e| e.to_string())?;
    }
    let events = game.event_log.clone();
    let find_cast = |seat: PlayerId, definition: &str| {
        events.iter().find_map(|event| match event {
            GameEvent::SpellCast { player, card }
                if *player == seat
                    && game
                        .card_definition(*card)
                        .is_ok_and(|known| known.id == definition) =>
            {
                Some(*card)
            }
            _ => None,
        })
    };
    let flow = find_cast(PlayerId(0), CARD_ID);
    let flow_definition = flow.and_then(|id| game.card_definition(id).ok().cloned());
    let watchwolf = find_cast(PlayerId(0), WATCHWOLF_ID);
    let flow_cast_index = flow.and_then(|id| events.iter().position(|event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == id)));
    let flow_resolved_index = flow.and_then(|id| {
        events
            .iter()
            .position(|event| matches!(event, GameEvent::SpellResolved { card } if *card == id))
    });
    let flow_drawn = match (flow_cast_index, flow_resolved_index) {
        (Some(cast), Some(resolved)) => events
            .iter()
            .enumerate()
            .filter_map(|(index, event)| {
                (index > cast && index < resolved)
                    .then_some(event)
                    .and_then(|event| match event {
                        GameEvent::CardMoved {
                            card,
                            to: Zone::Hand,
                        } => Some(*card),
                        _ => None,
                    })
            })
            .collect(),
        _ => Vec::new(),
    };
    let islands_at_cast = islands_at_cast.ok_or("Flow of Ideas cast snapshot was not captured")?;
    let flow_zone = flow.and_then(|id| game.zone_of(id));
    let player_life = [
        game.player(PlayerId(0)).map_err(|e| e.to_string())?.life,
        game.player(PlayerId(1)).map_err(|e| e.to_string())?.life,
    ];
    Ok(Captured {
        events,
        winner: game.winner().map(|p| p.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        flow,
        flow_definition,
        flow_zone,
        flow_drawn,
        islands_at_cast,
        watchwolf,
        player_life,
    })
}

struct FlowPolicy {
    player: PlayerId,
    flow_offered: bool,
    watchwolf_offered: bool,
}

impl FlowPolicy {
    fn new(player: PlayerId) -> Self {
        Self {
            player,
            flow_offered: false,
            watchwolf_offered: false,
        }
    }
    fn battlefield_land(&self, view: &GameView, definition: &str) -> bool {
        view.own_battlefield
            .iter()
            .any(|card| card.definition == Some(definition))
    }
    fn island_count(&self, view: &GameView) -> usize {
        view.own_battlefield
            .iter()
            .filter(|card| card.definition == Some(ISLAND_ID))
            .count()
    }
    fn play_land(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let islands = self.island_count(view);
        let desired = if self.player == PlayerId(0) && islands < 6 {
            ISLAND_ID
        } else if self.player == PlayerId(1) && islands < 5 {
            ISLAND_ID
        } else if self.player == PlayerId(0) && !self.battlefield_land(view, FOREST_ID) {
            FOREST_ID
        } else if self.player == PlayerId(0) && !self.battlefield_land(view, PLAINS_ID) {
            PLAINS_ID
        } else {
            return None;
        };
        view.hand
            .iter()
            .find(|card| {
                card.definition == Some(desired) && card.card_types.contains(&CardType::Land)
            })
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }
    fn payment(view: &GameView, cost: &ManaCost) -> Option<ManaPaymentSelection> {
        let mut available: BTreeMap<Color, u8> = Color::ALL
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
        if !cost.hybrid.is_empty() {
            return None;
        }
        let mut generic = Vec::new();
        for _ in 0..cost.generic {
            let color = available
                .iter()
                .filter(|(_, amount)| **amount > 0)
                .max_by_key(|(_, amount)| **amount)
                .map(|(color, _)| *color)?;
            *available.get_mut(&color)? -= 1;
            generic.push(color);
        }
        Some(ManaPaymentSelection {
            generic,
            hybrid: vec![],
        })
    }
    fn tap_land(view: &GameView, color: Color) -> Option<PolicyAction> {
        let land = view.own_battlefield.iter().find(|card| {
            !card.tapped
                && card.card_types.contains(&CardType::Land)
                && card.mana_colors.contains(&color)
        })?;
        Some(PolicyAction::ActivateManaAbility {
            land: land.id,
            color,
        })
    }
    fn cast(&mut self, view: &GameView, id: &str, cost: &ManaCost) -> Option<PolicyAction> {
        let card = view.hand.iter().find(|card| card.definition == Some(id))?;
        if let Some(mana_selection) = Self::payment(view, cost) {
            match id {
                CARD_ID => self.flow_offered = true,
                WATCHWOLF_ID => self.watchwolf_offered = true,
                _ => {}
            }
            return Some(PolicyAction::CastWithPayment {
                request: CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
                chosen_x: None,
                mana_selection,
            });
        }
        let needed = cost
            .colored
            .iter()
            .find(|color| view.mana_pool.amount(**color) == 0)
            .copied()
            .or_else(|| {
                (u16::from(view.mana_pool.total())
                    < u16::from(cost.generic)
                        + cost.colored.len() as u16
                        + cost.hybrid.len() as u16)
                    .then_some(cost.colored.first().copied().unwrap_or(Color::Blue))
            });
        needed
            .and_then(|color| Self::tap_land(view, color))
            .or_else(|| {
                Color::ALL
                    .into_iter()
                    .find_map(|color| Self::tap_land(view, color))
            })
    }
    fn combat(&self, view: &GameView) -> Option<PolicyAction> {
        if view.step == Step::DeclareAttackers
            && view.active_player == self.player
            && !view.attackers_declared
        {
            let attackers = view
                .own_battlefield
                .iter()
                .filter(|card| card.definition == Some(WATCHWOLF_ID) && card.can_attack)
                .map(|card| card.id)
                .collect();
            return Some(PolicyAction::DeclareAttackers { attackers });
        }
        if view.step == Step::DeclareBlockers
            && view.active_player != self.player
            && !view.blockers_declared
        {
            return Some(PolicyAction::DeclareBlockers {
                assignments: vec![],
            });
        }
        None
    }
}

impl CodePolicy for FlowPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.flow-of-ideas-island-draw.v1"
        } else {
            "rav.flow-of-ideas-island-opponent.v1"
        }
    }
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.combat(view) {
            return action;
        }
        if view.priority != self.player
            || view.active_player != self.player
            || view.step != Step::PrecombatMain
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.play_land(view) {
            return action;
        }
        if self.player == PlayerId(0) {
            if !self.flow_offered && self.island_count(view) >= 6 {
                if let Some(action) =
                    self.cast(view, CARD_ID, &ManaCost::with_colors(5, [Color::Blue]))
                {
                    return action;
                }
            }
            if self.flow_offered
                && !self.watchwolf_offered
                && !view
                    .own_battlefield
                    .iter()
                    .any(|card| card.definition == Some(WATCHWOLF_ID))
            {
                if let Some(action) = self.cast(
                    view,
                    WATCHWOLF_ID,
                    &ManaCost::with_colors(0, [Color::Green, Color::White]),
                ) {
                    return action;
                }
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(c: &Captured, seed: u64) -> Result<Value, String> {
    if c.winner != Some(0) || c.rejected != 0 || c.player_life[0] <= 0 || c.player_life[1] > 0 {
        return Err(format!(
            "Flow of Ideas match did not reach a clean seat-zero win: {:?} {:?}",
            c.winner, c.player_life
        ));
    }
    let flow = c.flow.ok_or("seat zero did not cast Flow of Ideas")?;
    let definition = c
        .flow_definition
        .as_ref()
        .ok_or("Flow of Ideas live card definition missing")?;
    let wolf = c.watchwolf.ok_or("seat zero did not cast Watchwolf")?;
    if definition.id != CARD_ID
        || definition.mana_cost != ManaCost::with_colors(5, [Color::Blue])
        || definition.colors != BTreeSet::from([Color::Blue])
        || definition.card_types != BTreeSet::from([CardType::Sorcery])
        || definition.effects
            != [Effect::DrawControllerForEachControlledBasicLandType {
                land_type: BasicLandType::Island,
            }]
    {
        return Err("Flow of Ideas live identity, exact blue sorcery cost/color/type, or Island draw effect differs".to_owned());
    }
    if c.flow_zone != Some(Zone::Graveyard) {
        return Err("resolved Flow of Ideas did not move to its owner's graveyard".to_owned());
    }
    if c.islands_at_cast[0] != 6 || c.islands_at_cast[1] == 0 {
        return Err(format!(
            "Flow snapshot should include six controller Islands and ignore the opponent's Islands: {:?}",
            c.islands_at_cast
        ));
    }
    if c.flow_drawn.len() != c.islands_at_cast[0]
        || c.flow_drawn.iter().copied().collect::<BTreeSet<_>>().len() != c.flow_drawn.len()
    {
        return Err(format!(
            "Flow drew {} cards for {} controlled Islands",
            c.flow_drawn.len(),
            c.islands_at_cast[0]
        ));
    }
    let events = &c.events;
    let flow_payment = event(events, |e| matches!(e, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == flow && colors.len() == 6 && colors.iter().all(|color| *color == Color::Blue))).ok_or("Flow of Ideas did not pay exactly one blue and five generic blue mana")?;
    let flow_cast = after(
        events,
        flow_payment,
        |e| matches!(e, GameEvent::SpellCast { player: PlayerId(0), card } if *card == flow),
    )
    .ok_or("Flow of Ideas SpellCast receipt missing")?;
    let flow_resolved = after(
        events,
        flow_cast,
        |e| matches!(e, GameEvent::SpellResolved { card } if *card == flow),
    )
    .ok_or("Flow of Ideas SpellResolved receipt missing")?;
    let draw_events: Vec<usize> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            (index > flow_cast
                && index < flow_resolved
                && matches!(event, GameEvent::CardMoved { to: Zone::Hand, .. }))
            .then_some(index)
        })
        .collect();
    if draw_events.len() != 6 {
        return Err(format!(
            "Flow resolution moved {} cards into hand instead of six",
            draw_events.len()
        ));
    }
    let flow_graveyard = after(
        events,
        flow_resolved,
        |e| matches!(e, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == flow),
    )
    .ok_or("Flow of Ideas did not move to its owner's graveyard after resolution")?;
    let wolf_payment = event(events, |e| matches!(e, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == wolf && colors.len() == 2 && colors.contains(&Color::Green) && colors.contains(&Color::White))).ok_or("Watchwolf did not pay exactly {G}{W}")?;
    let wolf_cast = after(
        events,
        wolf_payment,
        |e| matches!(e, GameEvent::SpellCast { player: PlayerId(0), card } if *card == wolf),
    )
    .ok_or("Watchwolf SpellCast receipt missing")?;
    let wolf_resolved = after(
        events,
        wolf_cast,
        |e| matches!(e, GameEvent::SpellResolved { card } if *card == wolf),
    )
    .ok_or("Watchwolf did not resolve")?;
    let wolf_entered = after(
        events,
        wolf_resolved,
        |e| matches!(e, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == wolf),
    )
    .ok_or("Watchwolf did not enter the battlefield")?;
    let attack = after(events, wolf_entered, |e| matches!(e, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&wolf))).ok_or("Watchwolf did not attack")?;
    let damage = after(events, attack, |e| matches!(e, GameEvent::DamageDealtToPlayer { source, player: PlayerId(1), amount: 3 } if *source == wolf)).ok_or("Watchwolf did not deal combat damage")?;
    let terminal = events
        .iter()
        .enumerate()
        .find_map(|(index, event)| {
            (index > damage
                && matches!(
                    event,
                    GameEvent::GameEnded {
                        winner: Some(PlayerId(0))
                    }
                ))
            .then_some(index)
        })
        .ok_or("terminal win receipt missing")?;
    let total_damage: i64 = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::DamageDealtToPlayer {
                source,
                player: PlayerId(1),
                amount,
            } if *source == wolf => Some(i64::from(*amount)),
            _ => None,
        })
        .sum();
    if total_damage < 20
        || !(flow_payment < flow_cast
            && flow_cast < draw_events[0]
            && draw_events.last().copied().unwrap() < flow_resolved
            && flow_resolved < flow_graveyard
            && flow_graveyard < wolf_payment
            && wolf_payment < wolf_cast
            && wolf_cast < wolf_resolved
            && wolf_resolved < wolf_entered
            && wolf_entered < attack
            && attack < damage
            && damage < terminal)
    {
        return Err("Flow payment, six draws, resolution, graveyard movement, or terminal combat events are out of order".to_owned());
    }
    let mut selected: BTreeSet<usize> = [
        flow_payment,
        flow_cast,
        flow_resolved,
        flow_graveyard,
        wolf_payment,
        wolf_cast,
        wolf_resolved,
        wolf_entered,
        attack,
        damage,
        terminal,
    ]
    .into_iter()
    .collect();
    selected.extend(draw_events.iter().copied());
    let projected: Vec<ProjectedEvent> = project_events(events)
        .into_iter()
        .filter(|event| selected.contains(&(event.sequence as usize)))
        .collect();
    let kinds: Vec<_> = projected.iter().map(|event| event.kind.as_str()).collect();
    for kind in [
        "SpellManaPaid",
        "SpellCast",
        "CardMoved",
        "SpellResolved",
        "AttackersDeclared",
        "DamageDealtToPlayer",
        "GameEnded",
    ] {
        if !kinds.contains(&kind) {
            return Err(format!("projected Flow trace omitted {kind}: {kinds:?}"));
        }
    }
    if projected
        .windows(2)
        .any(|pair| pair[0].sequence >= pair[1].sequence)
    {
        return Err("projected Flow event sequence is not strictly increasing".to_owned());
    }
    let draw_objects: Vec<u64> = draw_events
        .iter()
        .filter_map(|index| match events.get(*index) {
            Some(GameEvent::CardMoved { card, .. }) => Some(card.0),
            _ => None,
        })
        .collect();
    Ok(json!({
        "trace_id": "rav-flow-of-ideas-draw-per-controlled-island-full-game",
        "shuffle_seed": seed,
        "decks": ["rav_flow_of_ideas_watchwolf_60", "rav_island_opponent_60"],
        "policies": ["rav.flow-of-ideas-island-draw.v1", "rav.flow-of-ideas-island-opponent.v1"],
        "terminal": "Winner(PlayerId(0))", "winner_seat": 0, "turns": c.turns, "accepted_moves": c.accepted, "rejected_moves": c.rejected,
        "flow_of_ideas": {"object": flow.0, "collector_number": 50, "mana_cost": "{5}{U}", "mana_paid": ["Blue", "Blue", "Blue", "Blue", "Blue", "Blue"], "colors": ["Blue"], "card_types": ["Sorcery"], "zone_after_resolution": "Graveyard", "controller_island_count": c.islands_at_cast[0], "opponent_island_count_ignored": c.islands_at_cast[1], "cards_drawn": draw_objects, "draw_count": c.flow_drawn.len()},
        "combat": {"watchwolf": wolf.0, "power": 3, "toughness": 3, "total_damage_to_opponent": total_damage, "terminal_life": c.player_life},
        "events": projected, "passed": true
    }))
}

fn event(events: &[GameEvent], predicate: impl Fn(&GameEvent) -> bool) -> Option<usize> {
    events.iter().position(predicate)
}
fn after(
    events: &[GameEvent],
    index: usize,
    predicate: impl Fn(&GameEvent) -> bool,
) -> Option<usize> {
    events
        .iter()
        .enumerate()
        .skip(index + 1)
        .find_map(|(i, event)| predicate(event).then_some(i))
}
fn card_status(card: &CatalogCard) -> Value {
    let id = card_definitions()
        .into_iter()
        .find(|definition| definition.name == card.name)
        .map(|definition| definition.id)
        .unwrap_or("");
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,"engine_status":if id.is_empty(){"catalog_only"}else{"executable_compatibility_slice"},"trace_status":if id==CARD_ID{"passed"}else{"not_qualified"},"qualification":if id==CARD_ID{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
}
