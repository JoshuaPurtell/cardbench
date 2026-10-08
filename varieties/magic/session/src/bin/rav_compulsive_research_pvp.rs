//! Full-game diagnostic authority for RAV Compulsive Research.

use cardbench_magic_engine::{
    CardType, CastRequest, Color, DecisionKind, DecisionSelection, DecisionVisibility, DeckEntry,
    DeckList, Effect, Game, GameEvent, GameView, ManaPaymentSelection, PlayerId, PolicyAction,
    Step, Target, Zone,
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
use std::collections::BTreeSet;
use std::process::ExitCode;
use syn::{Expr, Item, Stmt, Visibility};

const TASK_ID: &str = "cardbench/magic/card";
const CARD_ID: &str = "RAV-COMPULSIVE-RESEARCH";
const CARD_NAME: &str = "Compulsive Research";
const WATCHWOLF_ID: &str = "RAV-WATCHWOLF";
const SOURCE_PATH: &str =
    "varieties/magic/sets/ravnica_city_of_guilds/src/cards/compulsive_research.rs";
const MAX_TRACE_SEED: u64 = 8;

#[path = "../../../sets/ravnica_city_of_guilds/src/cards/compulsive_research.rs"]
mod candidate_compulsive_research;

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
            eprintln!("Compulsive Research PvP authority failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run_authority() -> Result<Value, String> {
    validate_candidate_source()?;
    validate_candidate_definition()?;
    let catalog = unique_catalog()?;
    let seed = find_trace_seed()?;
    let land = run_match(seed, ChoiceMode::OneLand, Mutation::None)?;
    let land_trace = validate_trace(&land, ChoiceMode::OneLand)?;
    let land_replay = run_match(seed, ChoiceMode::OneLand, Mutation::None)?;
    if land != land_replay {
        return Err("Compulsive Research land-discard replay was not deterministic".to_owned());
    }
    let pair = run_match(seed, ChoiceMode::TwoCards, Mutation::None)?;
    let pair_trace = validate_trace(&pair, ChoiceMode::TwoCards)?;
    let pair_replay = run_match(seed, ChoiceMode::TwoCards, Mutation::None)?;
    if pair != pair_replay {
        return Err("Compulsive Research two-card-discard replay was not deterministic".to_owned());
    }
    let mutations = json!({
        "wrong_mana_cost_detected_in_game": mutation_detected(seed, ChoiceMode::OneLand, Mutation::WrongCost),
        "draw_one_instead_of_three_detected_in_game": mutation_detected(seed, ChoiceMode::OneLand, Mutation::DrawOne),
        "wrong_target_player_detected_in_game": mutation_detected(seed, ChoiceMode::OneLand, Mutation::WrongTarget),
        "sealed_mutation_authority": false
    });
    for key in [
        "wrong_mana_cost_detected_in_game",
        "draw_one_instead_of_three_detected_in_game",
        "wrong_target_player_detected_in_game",
    ] {
        if mutations[key] != true {
            return Err(format!(
                "Compulsive Research semantic mutant escaped: {key}"
            ));
        }
    }
    let breakdown: Vec<_> = catalog.iter().map(card_status).collect();
    Ok(json!({
        "schema_version": "cardbench.magic.card-pvp.check.v1",
        "task_id": TASK_ID,
        "expansion": "RAV",
        "card": {"collector_number": 40, "definition_id": CARD_ID, "name": CARD_NAME},
        "candidate": {"kind": candidate_kind(), "card_definition": CARD_ID},
        "candidate_kind": candidate_kind(),
        "viewer_scope": format!("{:?}", ViewerScope::Judge),
        "rules_authority": {
            "comprehensive_rules": "2026-08-07",
            "checked_sections": ["400.2 public and hidden zones", "601.2 casting spells", "608.2 resolving spells", "121.2-121.3 drawing cards", "701.8 discarding cards", "510 combat damage"],
            "source_contract": "RAV Compulsive Research #40 is a blue Sorcery for {2}{U}; target player draws three cards, then discards a land card or two cards.",
            "authority_cutoff": "2026-09-24",
            "no_provider_trial": true
        },
        "passed": true,
        "pilot_card_passed": true,
        "deterministic_replay": true,
        "cards_passed": 1,
        "cards_credited": 0,
        "cards_total": catalog.len(),
        "printings_total": rav_main_set_catalog().len(),
        "coverage_complete": false,
        "full_reward": false,
        "harbor_reward": 0.0,
        "pilot_reward": 0.0,
        "qualification_status": "unreviewed_diagnostic",
        "mutation_sensitivity": mutations,
        "traces": [land_trace, pair_trace],
        "per_card_breakdown": breakdown
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ChoiceMode {
    OneLand,
    TwoCards,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mutation {
    None,
    WrongCost,
    DrawOne,
    WrongTarget,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Captured {
    events: Vec<GameEvent>,
    winner: Option<usize>,
    turns: u32,
    accepted: u32,
    rejected: u32,
    research: Option<cardbench_magic_engine::ObjectId>,
    watchwolf: Option<cardbench_magic_engine::ObjectId>,
    discarded: Vec<(cardbench_magic_engine::ObjectId, String)>,
    drawn_count: usize,
    decision_visible_to_target: bool,
    decision_hidden_from_caster: bool,
    choice_visibility: Option<DecisionVisibility>,
    choice_player: Option<usize>,
    combat_damage: i64,
    terminal_life: [i64; 2],
}

fn candidate_kind() -> &'static str {
    if std::env::args().any(|arg| arg == "--candidate") {
        "agent"
    } else {
        "reference"
    }
}

fn candidate_value(mutation: Mutation) -> cardbench_magic_engine::CardDefinition {
    let mut definition = candidate_compulsive_research::definition();
    match mutation {
        Mutation::None | Mutation::WrongTarget => {}
        Mutation::WrongCost => definition.mana_cost.generic += 1,
        Mutation::DrawOne => definition.effects = vec![Effect::DrawTargetPlayer],
    }
    definition
}

fn validate_candidate_source() -> Result<(), String> {
    let source = std::fs::read_to_string(SOURCE_PATH)
        .map_err(|error| format!("could not read Compulsive Research source: {error}"))?;
    let file = syn::parse_file(&source)
        .map_err(|_| "Compulsive Research source is not valid Rust".to_owned())?;
    if file.items.len() != 3 || !file.attrs.iter().all(|attr| attr.path().is_ident("doc")) {
        return Err(
            "Compulsive Research source must contain private imports and one literal definition"
                .to_owned(),
        );
    }
    let mut definitions = 0;
    for item in &file.items {
        match item {
            Item::Use(item)
                if item.attrs.is_empty() && matches!(&item.vis, Visibility::Inherited) => {}
            Item::Fn(item)
                if item.sig.ident == "definition"
                    && item.sig.inputs.is_empty()
                    && item.sig.generics.params.is_empty()
                    && matches!(&item.sig.output, syn::ReturnType::Type(_, ty) if matches!(ty.as_ref(), syn::Type::Path(path) if path.path.is_ident("CardDefinition")))
                    && matches!(&item.vis, Visibility::Restricted(vis) if vis.path.is_ident("crate"))
                    && item.block.stmts.len() == 1
                    && matches!(&item.block.stmts[0], Stmt::Expr(Expr::Struct(value), None) if value.rest.is_none() && value.path.is_ident("CardDefinition")) =>
            {
                definitions += 1;
            }
            _ => {
                return Err(
                    "Compulsive Research module structure differs from its source contract"
                        .to_owned(),
                );
            }
        }
    }
    if definitions != 1 {
        return Err("Compulsive Research direct definition literal is missing".to_owned());
    }
    Ok(())
}

fn validate_candidate_definition() -> Result<(), String> {
    let definition = candidate_compulsive_research::definition();
    let registered = card_definitions()
        .into_iter()
        .find(|item| item.id == CARD_ID)
        .ok_or("Compulsive Research is missing from the RAV registry")?;
    if definition != registered
        || definition.id != CARD_ID
        || definition.name != CARD_NAME
        || definition.set_code != "RAV"
        || definition.mana_cost != cardbench_magic_engine::ManaCost::with_colors(2, [Color::Blue])
        || definition.colors != BTreeSet::from([Color::Blue])
        || !definition.mana_colors.is_empty()
        || definition.card_types != BTreeSet::from([CardType::Sorcery])
        || definition.is_basic_land
        || definition.supported_rules
            != [
                "full-rules-fidelity",
                "target-player-draw-three-conditional-private-discard",
                "policy-submitted-private-discard",
            ]
        || definition.power.is_some()
        || definition.toughness.is_some()
        || !definition.keywords.is_empty()
        || definition.effects != [Effect::DrawTargetPlayerThenConditionalPrivateDiscard]
    {
        return Err("Compulsive Research candidate identity, exact {2}{U} cost, blue Sorcery characteristics, or targeted effect differs".to_owned());
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
        || catalog.len() != 291
        || printings.len() != 306
    {
        return Err("RAV catalog denominator differs from the source contract".to_owned());
    }
    Ok(catalog)
}

fn find_trace_seed() -> Result<u64, String> {
    if let Ok(value) = std::env::var("CARDBENCH_COMPULSIVE_RESEARCH_SEED") {
        let seed = value.parse::<u64>().map_err(|_| {
            "CARDBENCH_COMPULSIVE_RESEARCH_SEED must be an unsigned integer".to_owned()
        })?;
        let capture = run_match(seed, ChoiceMode::OneLand, Mutation::None)?;
        validate_trace(&capture, ChoiceMode::OneLand)
            .map_err(|error| format!("seed {seed}: {error}"))?;
        return Ok(seed);
    }
    let mut last_error = "no trace attempt".to_owned();
    for seed in 0..MAX_TRACE_SEED {
        match run_match(seed, ChoiceMode::OneLand, Mutation::None) {
            Ok(capture) => match validate_trace(&capture, ChoiceMode::OneLand) {
                Ok(_) => return Ok(seed),
                Err(error) => last_error = format!("seed {seed}: {error}"),
            },
            Err(error) => last_error = format!("seed {seed}: {error}"),
        }
    }
    Err(format!(
        "no bounded shuffle seed completed Compulsive Research's target discard and terminal duel; {last_error}"
    ))
}

fn mutation_detected(seed: u64, choice: ChoiceMode, mutation: Mutation) -> bool {
    run_match(seed, choice, mutation)
        .and_then(|capture| validate_trace(&capture, choice))
        .is_err()
}

fn run_match(seed: u64, choice: ChoiceMode, mutation: Mutation) -> Result<Captured, String> {
    let mut definitions = card_definitions();
    let definition = definitions
        .iter_mut()
        .find(|item| item.id == CARD_ID)
        .ok_or("Compulsive Research missing from RAV registry")?;
    *definition = candidate_value(mutation);
    let mut game = Game::new_with_mana_abilities_and_basic_land_types(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
    )
    .map_err(|error| error.to_string())?;
    let p0 = deck(&[
        (CARD_ID, 4),
        (WATCHWOLF_ID, 4),
        ("RAV-ISLAND", 18),
        ("RAV-FOREST", 17),
        ("RAV-PLAINS", 17),
    ]);
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
            return Err("Compulsive Research duel decks must contain 60 cards".to_owned());
        }
        game.load_deck_into_library(player, list)
            .map_err(|error| error.to_string())?;
        game.draw_opening_hand(player, 7)
            .map_err(|error| error.to_string())?;
    }
    game.begin_game().map_err(|error| error.to_string())?;
    let mut policies: [ResearchPolicy; 2] = [
        ResearchPolicy::new(PlayerId(0), choice, mutation),
        ResearchPolicy::new(PlayerId(1), choice, mutation),
    ];
    let mut accepted = 0_u32;
    let mut attempted = 0_u32;
    let mut decision_visible_to_target = false;
    let mut decision_hidden_from_caster = false;
    while !game.is_game_over() {
        if game.turn > 80 || accepted >= 8_000 {
            return Err(format!(
                "Compulsive Research game exceeded bounds at turn {}",
                game.turn
            ));
        }
        let player = game.next_policy_player();
        let view = game
            .view_for_player(player)
            .map_err(|error| error.to_string())?;
        if let Some(decision) = &view.pending_decision
            && decision.kind == DecisionKind::ConditionalPrivateDiscard
        {
            decision_visible_to_target =
                player == PlayerId(1) && decision.visibility == DecisionVisibility::Private;
            let caster_view = game
                .view_for_player(PlayerId(0))
                .map_err(|error| error.to_string())?;
            decision_hidden_from_caster = caster_view.pending_decision.is_none();
        }
        let policy = policies
            .get_mut(player.0)
            .ok_or("Compulsive Research policy seat missing")?;
        let action = if let Some(action) = policy.propose_pending_decision(&view) {
            action
        } else if view.draw_replacement_pending {
            policy.propose_draw_replacement(&view)
        } else {
            policy.propose_move(&view)
        };
        attempted += 1;
        game.submit_policy_move(player, policy.id(), action)
            .map_err(|error| {
                format!(
                    "Compulsive Research policy move rejected at turn {} {:?}: {error}",
                    view.turn, view.step
                )
            })?;
        accepted += 1;
        game.validate_invariants()
            .map_err(|error| error.to_string())?;
    }
    let events = game.event_log.clone();
    let research = events.iter().find_map(|event| match event {
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
    let watchwolf = events.iter().find_map(|event| match event {
        GameEvent::SpellCast {
            player: PlayerId(0),
            card,
        } if game
            .card_definition(*card)
            .is_ok_and(|definition| definition.id == WATCHWOLF_ID) =>
        {
            Some(*card)
        }
        _ => None,
    });
    let decision_range = events.iter().enumerate().find_map(|(start, event)| {
        if !matches!(event, GameEvent::DecisionOpened { kind: DecisionKind::ConditionalPrivateDiscard, .. }) {
            return None;
        }
        let decision = match event { GameEvent::DecisionOpened { decision, .. } => *decision, _ => return None };
        let completed = events.iter().enumerate().skip(start + 1).find_map(|(index, event)| matches!(event, GameEvent::DecisionCompleted { decision: completed, kind: DecisionKind::ConditionalPrivateDiscard, .. } if *completed == decision).then_some(index))?;
        let resolved = events.iter().enumerate().skip(completed + 1).find_map(|(index, event)| matches!(event, GameEvent::SpellResolved { .. }).then_some(index))?;
        Some((completed, resolved))
    });
    let discarded: Vec<_> = decision_range.map_or_else(Vec::new, |(start, end)| {
        events[start + 1..end]
            .iter()
            .filter_map(|event| match event {
                GameEvent::CardDiscarded { player: _, card } => game
                    .card_definition(*card)
                    .ok()
                    .map(|definition| (*card, definition.id.to_owned())),
                _ => None,
            })
            .collect()
    });
    let (drawn_count, choice_player, choice_visibility) =
        research.map_or((0, None, None), |spell| {
            let cast = events.iter().position(
                |event| matches!(event, GameEvent::SpellCast { card, .. } if *card == spell),
            );
            let opened = events.iter().position(|event| {
                matches!(
                    event,
                    GameEvent::DecisionOpened {
                        player: PlayerId(1),
                        kind: DecisionKind::ConditionalPrivateDiscard,
                        ..
                    }
                )
            });
            let draw_count = match (cast, opened) {
                (Some(cast), Some(opened)) if cast < opened => events[cast + 1..opened]
                    .iter()
                    .filter(|event| matches!(event, GameEvent::CardMoved { to: Zone::Hand, .. }))
                    .count(),
                _ => 0,
            };
            let decision = events.iter().find_map(|event| match event {
                GameEvent::DecisionOpened {
                    player,
                    kind: DecisionKind::ConditionalPrivateDiscard,
                    visibility,
                    ..
                } => Some((player.0, *visibility)),
                _ => None,
            });
            (
                draw_count,
                decision.map(|value| value.0),
                decision.map(|value| value.1),
            )
        });
    let combat_damage = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::DamageDealtToPlayer {
                player: PlayerId(1),
                amount,
                ..
            } => Some(i64::from(*amount)),
            _ => None,
        })
        .sum();
    let terminal_life = [PlayerId(0), PlayerId(1)].map(|player| {
        game.player(player)
            .map(|state| state.life)
            .unwrap_or_default()
    });
    Ok(Captured {
        events,
        winner: game.winner().map(|player| player.0),
        turns: game.turn,
        accepted,
        rejected: attempted.saturating_sub(accepted),
        research,
        watchwolf,
        discarded,
        drawn_count,
        decision_visible_to_target,
        decision_hidden_from_caster,
        choice_visibility,
        choice_player,
        combat_damage,
        terminal_life,
    })
}

fn deck(entries: &[(&str, u8)]) -> DeckList {
    DeckList {
        mainboard: entries
            .iter()
            .map(|(card, count)| DeckEntry {
                card: (*card).into(),
                count: *count,
            })
            .collect(),
        sideboard: vec![],
    }
}

struct ResearchPolicy {
    player: PlayerId,
    choice: ChoiceMode,
    mutation: Mutation,
    research_cast: bool,
    delegate: cardbench_magic_policies::RavCatalogPolicy,
}

impl ResearchPolicy {
    fn new(player: PlayerId, choice: ChoiceMode, mutation: Mutation) -> Self {
        Self {
            player,
            choice,
            mutation,
            research_cast: false,
            delegate: cardbench_magic_policies::RavCatalogPolicy::new(
                player,
                cardbench_magic_policies::CatalogPolicyProfile::Patient,
            ),
        }
    }

    fn land_to_play(&self, view: &GameView) -> Option<PolicyAction> {
        if view.lands_played != 0 {
            return None;
        }
        let preferences: &[&str] = if self.player == PlayerId(1) {
            &["RAV-ISLAND"]
        } else if self.research_cast {
            &["RAV-FOREST", "RAV-PLAINS", "RAV-ISLAND"]
        } else {
            &["RAV-ISLAND", "RAV-FOREST", "RAV-PLAINS"]
        };
        view.hand
            .iter()
            .find(|card| {
                card.card_types.contains(&CardType::Land)
                    && preferences.contains(&card.definition.unwrap_or(""))
            })
            .map(|card| PolicyAction::PlayLand { card: card.id })
    }

    fn activate(&self, view: &GameView, color: Color) -> Option<PolicyAction> {
        view.own_battlefield
            .iter()
            .find(|land| {
                !land.tapped
                    && land.card_types.contains(&CardType::Land)
                    && land.mana_colors.contains(&color)
            })
            .map(|land| PolicyAction::ActivateManaAbility {
                land: land.id,
                color,
            })
    }

    fn cast(
        &self,
        card: cardbench_magic_engine::ObjectId,
        targets: Vec<Target>,
        generic: Vec<Color>,
    ) -> PolicyAction {
        PolicyAction::CastWithPayment {
            request: CastRequest {
                card,
                targets,
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            chosen_x: None,
            mana_selection: ManaPaymentSelection {
                generic,
                hybrid: vec![],
            },
        }
    }

    fn research_generic_payment(&self, view: &GameView) -> Option<Vec<Color>> {
        let mut available: Vec<_> = Color::MANA_ALL
            .into_iter()
            .map(|color| (color, view.mana_pool.amount(color)))
            .collect();
        let blue = available
            .iter_mut()
            .find(|(color, _)| *color == Color::Blue)?;
        if blue.1 == 0 {
            return None;
        }
        blue.1 -= 1;
        let mut generic = Vec::with_capacity(2);
        for _ in 0..2 {
            let (color, amount) = available.iter_mut().find(|(_, amount)| *amount > 0)?;
            *amount -= 1;
            generic.push(*color);
        }
        Some(generic)
    }

    fn has_creature(&self, view: &GameView, id: &str) -> bool {
        view.own_battlefield
            .iter()
            .any(|card| card.definition == Some(id))
    }
}

impl CodePolicy for ResearchPolicy {
    fn id(&self) -> &'static str {
        if self.player == PlayerId(0) {
            "rav.compulsive-research-caster.v1"
        } else {
            "rav.compulsive-research-target.v1"
        }
    }

    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        let decision = view.pending_decision.as_ref()?;
        if decision.kind != DecisionKind::ConditionalPrivateDiscard || view.player != self.player {
            return self.delegate.propose_pending_decision(view);
        }
        let selected: Vec<_> = match self.choice {
            ChoiceMode::OneLand => decision
                .candidates
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
                .map(|card| vec![card.id])
                .unwrap_or_else(|| {
                    decision
                        .candidates
                        .iter()
                        .take(2)
                        .map(|card| card.id)
                        .collect()
                }),
            ChoiceMode::TwoCards => decision
                .candidates
                .iter()
                .take(2)
                .map(|card| card.id)
                .collect(),
        };
        Some(PolicyAction::SubmitDecision {
            decision: decision.id,
            selection: DecisionSelection::Objects(selected),
        })
    }

    fn propose_draw_replacement(&mut self, view: &GameView) -> PolicyAction {
        self.delegate.propose_draw_replacement(view)
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
                    .filter(|card| card.can_attack)
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
        if view.step != Step::PrecombatMain && view.step != Step::PostcombatMain
            || view.priority != self.player
            || view.active_player != self.player
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }
        if let Some(action) = self.land_to_play(view) {
            return action;
        }
        if self.player == PlayerId(0) && !self.research_cast {
            if let Some(spell) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(CARD_ID))
                .map(|card| card.id)
            {
                if view.mana_pool.total() < 3 || view.mana_pool.amount(Color::Blue) == 0 {
                    return self
                        .activate(view, Color::Blue)
                        .unwrap_or(PolicyAction::PassPriority);
                }
                let Some(generic) = self.research_generic_payment(view) else {
                    return self
                        .activate(view, Color::Blue)
                        .unwrap_or(PolicyAction::PassPriority);
                };
                self.research_cast = true;
                let target = if self.mutation == Mutation::WrongTarget {
                    PlayerId(0)
                } else {
                    PlayerId(1)
                };
                return self.cast(spell, vec![Target::Player(target)], generic);
            }
            return PolicyAction::PassPriority;
        }
        if self.player == PlayerId(0)
            && self.research_cast
            && !self.has_creature(view, WATCHWOLF_ID)
        {
            if let Some(creature) = view
                .hand
                .iter()
                .find(|card| card.definition == Some(WATCHWOLF_ID))
                .map(|card| card.id)
            {
                if view.mana_pool.amount(Color::Green) == 0 {
                    return self
                        .activate(view, Color::Green)
                        .unwrap_or(PolicyAction::PassPriority);
                }
                if view.mana_pool.amount(Color::White) == 0 {
                    return self
                        .activate(view, Color::White)
                        .unwrap_or(PolicyAction::PassPriority);
                }
                return self.cast(creature, vec![], vec![]);
            }
            return PolicyAction::PassPriority;
        }
        if self.player == PlayerId(1) {
            if let Some(action) = self.activate(view, Color::Blue) {
                return action;
            }
        }
        PolicyAction::PassPriority
    }
}

fn validate_trace(capture: &Captured, choice: ChoiceMode) -> Result<Value, String> {
    if capture.winner != Some(0)
        || capture.rejected != 0
        || capture.terminal_life[0] <= 0
        || capture.terminal_life[1] > 0
    {
        return Err(format!(
            "Compulsive Research match did not reach a clean seat-zero win: {:?} {:?}",
            capture.winner, capture.terminal_life
        ));
    }
    let spell = capture
        .research
        .ok_or("seat zero did not cast Compulsive Research")?;
    let registered = card_definitions()
        .into_iter()
        .find(|item| item.id == CARD_ID)
        .ok_or("Compulsive Research definition missing")?;
    if registered.mana_cost != cardbench_magic_engine::ManaCost::with_colors(2, [Color::Blue])
        || registered.effects != [Effect::DrawTargetPlayerThenConditionalPrivateDiscard]
    {
        return Err(
            "Compulsive Research live cost or target-player draw/discard effect differs".to_owned(),
        );
    }
    if capture.drawn_count != 3
        || !capture.decision_visible_to_target
        || !capture.decision_hidden_from_caster
        || capture.choice_player != Some(1)
        || capture.choice_visibility != Some(DecisionVisibility::Private)
    {
        return Err(format!(
            "target draw/private decision contract differs: {} draws, target={}, hidden={}, choice={:?}/{:?}",
            capture.drawn_count,
            capture.decision_visible_to_target,
            capture.decision_hidden_from_caster,
            capture.choice_player,
            capture.choice_visibility
        ));
    }
    let discarded: Vec<_> = capture.discarded.iter().filter(|(card, _)| capture.events.iter().any(|event| matches!(event, GameEvent::CardDiscarded { player: PlayerId(1), card: discarded } if discarded == card))).collect();
    let branch_ok = match choice {
        ChoiceMode::OneLand => {
            discarded.len() == 1
                && card_definitions().iter().any(|definition| {
                    definition.id == discarded[0].1
                        && definition.card_types.contains(&CardType::Land)
                })
        }
        ChoiceMode::TwoCards => discarded.len() == 2 && discarded[0].0 != discarded[1].0,
    };
    if !branch_ok {
        return Err(format!(
            "target discard branch {:?} did not commit the required distinct card count: {:?}",
            choice, discarded
        ));
    }
    let events = &capture.events;
    let paid = events.iter().position(|event| matches!(event, GameEvent::SpellManaPaid { player: PlayerId(0), card, colors } if *card == spell && colors.len() == 3 && colors.iter().all(|color| *color == Color::Blue))).ok_or("Compulsive Research did not pay exactly three blue mana")?;
    let cast = after(events, paid, |event| matches!(event, GameEvent::SpellCast { player: PlayerId(0), card } if *card == spell)).ok_or("Compulsive Research cast receipt missing")?;
    let opened = after(events, cast, |event| {
        matches!(
            event,
            GameEvent::DecisionOpened {
                player: PlayerId(1),
                kind: DecisionKind::ConditionalPrivateDiscard,
                visibility: DecisionVisibility::Private,
                min_selections: 1,
                max_selections: 2,
                ..
            }
        )
    })
    .ok_or("recipient-private one-or-two-card decision receipt missing")?;
    let drawn: Vec<_> = (cast + 1..opened)
        .filter(|index| matches!(events[*index], GameEvent::CardMoved { to: Zone::Hand, .. }))
        .collect();
    if drawn.len() != 3 {
        return Err(format!(
            "Compulsive Research moved {} cards before its discard decision",
            drawn.len()
        ));
    }
    let completed = after(events, opened, |event| {
        matches!(
            event,
            GameEvent::DecisionCompleted {
                player: PlayerId(1),
                kind: DecisionKind::ConditionalPrivateDiscard,
                ..
            }
        )
    })
    .ok_or("private discard completion receipt missing")?;
    let resolved = after(
        events,
        completed,
        |event| matches!(event, GameEvent::SpellResolved { card } if *card == spell),
    )
    .ok_or("Compulsive Research did not resolve after the target decision")?;
    let discard_indices: Vec<_> = events
        .iter()
        .enumerate()
        .filter_map(|(index, event)| {
            (index > completed
                && index < resolved
                && matches!(
                    event,
                    GameEvent::CardDiscarded {
                        player: PlayerId(1),
                        ..
                    }
                ))
            .then_some(index)
        })
        .collect();
    let graveyard = after(events, resolved, |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Graveyard } if *card == spell)).ok_or("Compulsive Research did not go to its owner's graveyard")?;
    let wolf = capture
        .watchwolf
        .ok_or("seat zero did not cast a post-resolution Watchwolf")?;
    let attack = after(events, graveyard, |event| matches!(event, GameEvent::AttackersDeclared { player: PlayerId(0), attackers } if attackers.contains(&wolf))).ok_or("Watchwolf did not attack after Compulsive Research resolved")?;
    let damage = after(events, attack, |event| {
        matches!(
            event,
            GameEvent::DamageDealtToPlayer {
                player: PlayerId(1),
                ..
            }
        )
    })
    .ok_or("Watchwolf combat damage receipt missing")?;
    let terminal = events
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::GameEnded {
                    winner: Some(PlayerId(0))
                }
            )
        })
        .ok_or("terminal seat-zero win receipt missing")?;
    if capture.combat_damage < 20
        || !(paid < cast
            && cast < drawn[0]
            && drawn[2] < opened
            && opened < completed
            && completed < discard_indices.first().copied().unwrap_or(completed)
            && discard_indices.last().copied().unwrap_or(completed) < resolved
            && resolved < graveyard
            && graveyard < attack
            && attack < damage
            && damage < terminal)
    {
        return Err("Compulsive Research payment, draw, discard, resolution, or terminal combat receipts are out of order".to_owned());
    }
    let selected: BTreeSet<usize> = [
        paid, cast, opened, completed, resolved, graveyard, attack, damage, terminal,
    ]
    .into_iter()
    .chain(drawn)
    .chain(discard_indices)
    .collect();
    let projected: Vec<ProjectedEvent> = project_events(events)
        .into_iter()
        .filter(|event| selected.contains(&(event.sequence as usize)))
        .collect();
    let kinds: Vec<_> = projected.iter().map(|event| event.kind.as_str()).collect();
    for kind in [
        "SpellManaPaid",
        "SpellCast",
        "CardMoved",
        "DecisionOpened",
        "DecisionCompleted",
        "CardDiscarded",
        "SpellResolved",
        "AttackersDeclared",
        "DamageDealtToPlayer",
        "GameEnded",
    ] {
        if !kinds.contains(&kind) {
            return Err(format!(
                "projected Compulsive Research trace omitted {kind}: {kinds:?}"
            ));
        }
    }
    Ok(json!({
        "trace_id": if choice == ChoiceMode::OneLand { "rav-compulsive-research-target-draw-three-discard-land-full-game" } else { "rav-compulsive-research-target-draw-three-discard-two-full-game" },
        "terminal": "Winner(PlayerId(0))", "winner": 0, "turns": capture.turns, "accepted_moves": capture.accepted, "rejected_moves": capture.rejected,
        "mana_cost": "{2}{U}", "paid_mana_colors": ["Blue", "Blue", "Blue"], "target_player": 1,
        "cards_drawn_before_decision": capture.drawn_count, "decision_visibility": "Private", "decision_player": capture.choice_player,
        "decision_visible_to_target": capture.decision_visible_to_target, "decision_hidden_from_caster": capture.decision_hidden_from_caster,
        "discarded_cards": capture.discarded.iter().map(|(_, definition)| definition).collect::<Vec<_>>(),
        "discard_branch": if choice == ChoiceMode::OneLand { "one_land" } else { "two_distinct_cards" },
        "combat": {"watchwolf_combat_damage": capture.combat_damage, "terminal_life": capture.terminal_life},
        "events": projected, "passed": true
    }))
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
        .find_map(|(index, event)| predicate(event).then_some(index))
}

fn card_status(card: &CatalogCard) -> Value {
    let id = card_definitions()
        .into_iter()
        .find(|definition| definition.name == card.name)
        .map(|definition| definition.id)
        .unwrap_or("");
    json!({"collector_number":card.collector_number,"name":card.name,"definition_id":id,"engine_status":if id.is_empty(){"catalog_only"}else{"executable_compatibility_slice"},"trace_status":if id==CARD_ID{"passed"}else{"not_qualified"},"qualification":if id==CARD_ID{"unreviewed_diagnostic"}else{"not_evaluated"},"credited":false})
}
