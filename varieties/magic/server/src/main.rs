#![forbid(unsafe_code)]
#![allow(clippy::struct_excessive_bools, clippy::too_many_lines)]

use axum::{
    Json, Router,
    extract::{
        Path, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, CombatBlock, Game, GameView, ManaPool, ObjectId,
    PlayerId, PolicyAction, Step, Target, TargetRequirement,
};
use cardbench_magic_policies::{
    Archetype, CodePolicy, PolicyVersion, seat_policy, shared_card_index,
};
use cardbench_magic_rav::{
    DeckFixture, load_constructed_decks, new_rav_game, rav_land_entry_bindings,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    env,
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;

const HUMAN: PlayerId = PlayerId(0);
const AI: PlayerId = PlayerId(1);
const OPENING_HAND: u8 = 7;

type Shared = Arc<Mutex<AppState>>;

#[derive(Default)]
struct AppState {
    next_match: u64,
    matches: BTreeMap<String, MatchSession>,
}

struct MatchSession {
    id: String,
    human_deck: String,
    ai_deck: String,
    opponent_policy: String,
    seed: u64,
    game: Game,
    ai: Box<dyn CodePolicy>,
    revision: u64,
    error: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMessage {
    Start {
        human_deck: String,
        ai_deck: String,
        policy_version: Option<String>,
        seed: Option<u64>,
    },
    Action {
        match_id: String,
        action: UserAction,
    },
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum UserAction {
    PassPriority,
    Draw,
    PlayLand {
        card: u64,
        pay_life: bool,
    },
    AddMana {
        land: u64,
        color: String,
    },
    Cast {
        card: u64,
        target: Option<TargetInput>,
    },
    DeclareAttackers {
        attackers: Vec<u64>,
    },
    DeclareBlockers {
        assignments: Vec<BlockInput>,
    },
}

#[derive(Clone, Debug, Deserialize)]
struct TargetInput {
    kind: String,
    id: u64,
}

#[derive(Clone, Debug, Deserialize)]
struct BlockInput {
    attacker: u64,
    blocker: u64,
}

#[derive(Clone, Debug, Serialize)]
struct DeckSummary {
    id: String,
    name: String,
    archetype: String,
    cards: usize,
}

#[derive(Clone, Debug, Serialize)]
struct CardDto {
    id: u64,
    name: String,
    definition: Option<String>,
    types: Vec<String>,
    mana_cost: String,
    colors: Vec<String>,
    mana_colors: Vec<String>,
    power: Option<i16>,
    toughness: Option<i16>,
    tapped: bool,
    can_attack: bool,
    can_block: bool,
    summoning_sick: bool,
}

#[derive(Clone, Debug, Serialize)]
struct SeatDto {
    seat: usize,
    life: i64,
    hand: Vec<CardDto>,
    battlefield: Vec<CardDto>,
    graveyard_count: usize,
    library_count: usize,
    mana: ManaDto,
    lands_played: u8,
}

#[derive(Clone, Debug, Serialize)]
struct ManaDto {
    white: u8,
    blue: u8,
    black: u8,
    red: u8,
    green: u8,
    colorless: u8,
}

#[derive(Clone, Debug, Serialize)]
struct TargetDto {
    kind: String,
    id: u64,
    label: String,
}

#[derive(Clone, Debug, Serialize)]
struct ActionDto {
    kind: String,
    card: Option<u64>,
    label: String,
    pay_life: Option<bool>,
    targets: Vec<TargetDto>,
    attackers: Vec<u64>,
    mana_color: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct StackDto {
    id: u64,
    card: Option<CardDto>,
    controller: usize,
    targets: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
struct PublicState {
    schema: String,
    match_id: String,
    revision: u64,
    seed: u64,
    human_deck: String,
    ai_deck: String,
    opponent_policy: String,
    turn: u32,
    step: String,
    active_player: usize,
    priority: usize,
    decision_player: usize,
    awaiting_human: bool,
    terminal: bool,
    winner: Option<usize>,
    error: Option<String>,
    human: SeatDto,
    opponent: SeatDto,
    stack: Vec<StackDto>,
    attackers: Vec<u64>,
    attackers_declared: bool,
    blockers_declared: bool,
    action_surface: Vec<ActionDto>,
    log: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
struct ApiError {
    error: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bind = env::var("RAV_MAGIC_BIND").unwrap_or_else(|_| "127.0.0.1:3000".to_owned());
    let state = Arc::new(Mutex::new(AppState::default()));
    let app = Router::new()
        .route("/api/decks", get(list_decks))
        .route("/api/matches/:id", get(get_match))
        .route("/api/matches/:id/action", post(post_action))
        .route("/ws", get(websocket))
        .with_state(state)
        .layer(CorsLayer::permissive());
    let listener = TcpListener::bind(&bind).await?;
    println!("rav-magic-server listening on http://{bind}");
    axum::serve(listener, app).await?;
    Ok(())
}

async fn list_decks() -> impl IntoResponse {
    match load_constructed_decks() {
        Ok(decks) => Json(decks.into_iter().map(deck_summary).collect::<Vec<_>>()).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ApiError {
                error: error.to_string(),
            }),
        )
            .into_response(),
    }
}

async fn get_match(State(state): State<Shared>, Path(id): Path<String>) -> impl IntoResponse {
    let result = state
        .lock()
        .map_err(|_| "match state lock poisoned".to_owned())
        .and_then(|mut app| {
            let session = app
                .matches
                .get_mut(&id)
                .ok_or_else(|| "match not found".to_owned())?;
            session.run_ai_until_human();
            Ok(session.public_state())
        });
    match result {
        Ok(state) => Json(state).into_response(),
        Err(error) => (StatusCode::NOT_FOUND, Json(ApiError { error })).into_response(),
    }
}

async fn post_action(
    State(state): State<Shared>,
    Path(id): Path<String>,
    Json(action): Json<UserAction>,
) -> impl IntoResponse {
    let result = state
        .lock()
        .map_err(|_| "match state lock poisoned".to_owned())
        .and_then(|mut app| {
            let session = app
                .matches
                .get_mut(&id)
                .ok_or_else(|| "match not found".to_owned())?;
            session.apply_human_action(action)?;
            session.run_ai_until_human();
            Ok(session.public_state())
        });
    match result {
        Ok(state) => Json(state).into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, Json(ApiError { error })).into_response(),
    }
}

async fn websocket(State(state): State<Shared>, upgrade: WebSocketUpgrade) -> impl IntoResponse {
    upgrade.on_upgrade(move |socket| websocket_session(socket, state))
}

async fn websocket_session(mut socket: WebSocket, state: Shared) {
    while let Some(Ok(message)) = socket.next().await {
        let Message::Text(text) = message else {
            continue;
        };
        let response = match serde_json::from_str::<ClientMessage>(&text) {
            Ok(ClientMessage::Start {
                human_deck,
                ai_deck,
                policy_version,
                seed,
            }) => match create_match(&state, &human_deck, &ai_deck, policy_version, seed) {
                Ok(snapshot) => snapshot,
                Err(error) => error_message(&error),
            },
            Ok(ClientMessage::Action { match_id, action }) => {
                match apply_match_action(&state, &match_id, action) {
                    Ok(snapshot) => snapshot,
                    Err(error) => error_message(&error),
                }
            }
            Err(error) => error_message(&format!("invalid client message: {error}")),
        };
        if socket.send(Message::Text(response)).await.is_err() {
            break;
        }
    }
}

fn create_match(
    state: &Shared,
    human_deck: &str,
    ai_deck: &str,
    requested_policy: Option<String>,
    seed: Option<u64>,
) -> Result<String, String> {
    let decks = load_constructed_decks().map_err(|error| error.to_string())?;
    let human = find_deck(&decks, human_deck)?;
    let opponent = find_deck(&decks, ai_deck)?;
    let archetype = Archetype::parse(&opponent.archetype)
        .ok_or_else(|| format!("unknown opponent archetype `{}`", opponent.archetype))?;
    let requested_policy = requested_policy.unwrap_or_else(|| "v7".to_owned());
    let policy_version = PolicyVersion::parse(&requested_policy)
        .ok_or_else(|| format!("unknown policy version `{requested_policy}`"))?;
    let match_seed = seed.unwrap_or_else(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(73, |duration| {
                u64::try_from(duration.as_nanos()).unwrap_or(73)
            })
    });
    let mut game = new_rav_game(2).map_err(|error| error.to_string())?;
    game.set_shuffle_seed(match_seed)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(HUMAN, &human.deck)
        .map_err(|error| error.to_string())?;
    game.load_deck_into_library(AI, &opponent.deck)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(HUMAN, OPENING_HAND)
        .map_err(|error| error.to_string())?;
    game.draw_opening_hand(AI, OPENING_HAND)
        .map_err(|error| error.to_string())?;
    game.begin_game().map_err(|error| error.to_string())?;
    let mut app = state
        .lock()
        .map_err(|_| "match state lock poisoned".to_owned())?;
    app.next_match = app.next_match.saturating_add(1);
    let id = format!("rav-local-{}", app.next_match);
    let mut session = MatchSession {
        id: id.clone(),
        human_deck: human.id.clone(),
        ai_deck: opponent.id.clone(),
        opponent_policy: policy_version.id().to_owned(),
        seed: match_seed,
        game,
        ai: seat_policy(policy_version, AI, archetype, shared_card_index()),
        revision: 0,
        error: None,
    };
    session.run_ai_until_human();
    let snapshot = session.public_state_json();
    app.matches.insert(id, session);
    Ok(snapshot)
}

fn apply_match_action(state: &Shared, id: &str, action: UserAction) -> Result<String, String> {
    let mut app = state
        .lock()
        .map_err(|_| "match state lock poisoned".to_owned())?;
    let session = app
        .matches
        .get_mut(id)
        .ok_or_else(|| "match not found".to_owned())?;
    session.apply_human_action(action)?;
    session.run_ai_until_human();
    Ok(session.public_state_json())
}

impl MatchSession {
    fn run_ai_until_human(&mut self) {
        const MAX_AUTOMATIC_MOVES: usize = 10_000;
        for _ in 0..MAX_AUTOMATIC_MOVES {
            if self.game.is_game_over() {
                self.revision = self.revision.saturating_add(1);
                return;
            }
            let view = match self.game.view_for_player(AI) {
                Ok(view) => view,
                Err(error) => {
                    self.error = Some(error.to_string());
                    return;
                }
            };
            if view.decision_player != AI {
                return;
            }
            let action = if view.draw_replacement_pending {
                self.ai.propose_draw_replacement(&view)
            } else if view.private_library_choice.is_some() {
                self.ai.propose_private_library_choice(&view)
            } else if view.private_opponent_library_choice.is_some() {
                self.ai.propose_private_opponent_library_choice(&view)
            } else if view.library_search_choice.is_some()
                || view.triggered_ability_target_choice.is_some()
                || view.triggered_ability_effect_object_choice.is_some()
                || view.pending_decision.is_some()
            {
                if let Some(action) = self.ai.propose_pending_decision(&view) {
                    action
                } else {
                    self.error = Some(format!(
                        "{} did not provide a mandatory decision",
                        self.ai.id()
                    ));
                    return;
                }
            } else if view.optional_triggered_ability_choice.is_some() {
                self.ai.propose_optional_triggered_ability(&view)
            } else {
                self.ai.propose_move(&view)
            };
            let policy = self.ai.id();
            if let Err(error) = self.game.submit_policy_move(AI, policy, action) {
                self.error = Some(format!("{policy} rejected by engine: {error}"));
                return;
            }
            self.revision = self.revision.saturating_add(1);
        }
        self.error = Some("automatic policy loop exceeded 10,000 moves".to_owned());
    }

    fn apply_human_action(&mut self, action: UserAction) -> Result<(), String> {
        if self.game.is_game_over() {
            return Err("the match is already over".to_owned());
        }
        let policy = "human-local-v1";
        let engine_action = match action {
            UserAction::PassPriority => PolicyAction::PassPriority,
            UserAction::Draw => PolicyAction::Draw {
                decision: self
                    .game
                    .view_for_player(HUMAN)
                    .map_err(|error| error.to_string())?
                    .draw_replacement_decision
                    .ok_or_else(|| "the draw decision is no longer pending".to_owned())?,
                dredge: None,
            },
            UserAction::PlayLand { card, pay_life } => {
                let object = ObjectId(card);
                let definition = self
                    .game
                    .card_definition(object)
                    .map_err(|error| error.to_string())?;
                if rav_land_entry_bindings()
                    .into_iter()
                    .any(|binding| binding.card_definition == definition.id)
                {
                    PolicyAction::PlayLandWithEntryLifePayment {
                        card: object,
                        pay_life,
                    }
                } else {
                    PolicyAction::PlayLand { card: object }
                }
            }
            UserAction::AddMana { land, color } => PolicyAction::ActivateManaAbility {
                land: ObjectId(land),
                color: parse_color(&color)?,
            },
            UserAction::Cast { card, target } => {
                let object = ObjectId(card);
                let targets = match target {
                    Some(target) => vec![target_to_engine(&target)?],
                    None => Vec::new(),
                };
                PolicyAction::Cast(CastRequest {
                    card: object,
                    targets,
                    convoke: Vec::new(),
                    payment_mana_abilities: Vec::new(),
                })
            }
            UserAction::DeclareAttackers { attackers } => PolicyAction::DeclareAttackers {
                attackers: attackers.into_iter().map(ObjectId).collect(),
            },
            UserAction::DeclareBlockers { assignments } => PolicyAction::DeclareBlockers {
                assignments: assignments
                    .into_iter()
                    .map(|block| CombatBlock {
                        attacker: ObjectId(block.attacker),
                        blocker: ObjectId(block.blocker),
                    })
                    .collect(),
            },
        };
        self.game
            .submit_policy_move(HUMAN, policy, engine_action)
            .map_err(|error| error.to_string())?;
        self.revision = self.revision.saturating_add(1);
        Ok(())
    }

    fn public_state_json(&self) -> String {
        serde_json::to_string(&self.public_state()).unwrap_or_else(|error| {
            error_message(&format!("failed to serialize match state: {error}"))
        })
    }

    fn public_state(&self) -> PublicState {
        let human_view = self.game.view_for_player(HUMAN).ok();
        let ai_view = self.game.view_for_player(AI).ok();
        let decision_player = human_view
            .as_ref()
            .map_or(self.game.priority.0, |view| view.decision_player.0);
        let awaiting_human = human_view
            .as_ref()
            .is_some_and(|view| view.decision_player == HUMAN || self.game.priority == HUMAN);
        let winner = self.game.winner().map(|player| player.0);
        PublicState {
            schema: "cardbench.magic.local.v1".to_owned(),
            match_id: self.id.clone(),
            revision: self.revision,
            seed: self.seed,
            human_deck: self.human_deck.clone(),
            ai_deck: self.ai_deck.clone(),
            opponent_policy: self.opponent_policy.clone(),
            turn: self.game.turn,
            step: format!("{:?}", self.game.step),
            active_player: self.game.active_player.0,
            priority: self.game.priority.0,
            decision_player,
            awaiting_human,
            terminal: self.game.is_game_over(),
            winner,
            error: self.error.clone(),
            human: seat_dto(&self.game, HUMAN, human_view.as_ref()),
            opponent: seat_dto(&self.game, AI, ai_view.as_ref()),
            stack: stack_dto(&self.game, human_view.as_ref()),
            attackers: human_view
                .as_ref()
                .map(|view| view.combat_attackers.iter().map(|card| card.id.0).collect())
                .unwrap_or_default(),
            attackers_declared: human_view
                .as_ref()
                .is_some_and(|view| view.attackers_declared),
            blockers_declared: human_view
                .as_ref()
                .is_some_and(|view| view.blockers_declared),
            action_surface: action_surface(&self.game, human_view.as_ref(), self.error.is_none()),
            log: self
                .game
                .event_log
                .iter()
                .rev()
                .take(12)
                .map(|event| format!("{event:?}"))
                .collect(),
        }
    }
}

fn action_surface(game: &Game, view: Option<&GameView>, enabled: bool) -> Vec<ActionDto> {
    if !enabled {
        return Vec::new();
    }
    let Some(view) = view else { return Vec::new() };
    if view.decision_player != HUMAN && game.priority != HUMAN {
        return Vec::new();
    }
    if view.draw_replacement_pending {
        return vec![ActionDto {
            kind: "draw".to_owned(),
            card: None,
            label: "Draw for turn".to_owned(),
            pay_life: None,
            targets: Vec::new(),
            attackers: Vec::new(),
            mana_color: None,
        }];
    }
    if view.pending_decision.is_some() {
        return vec![ActionDto {
            kind: "decision".to_owned(),
            card: None,
            label: "A mandatory choice is pending (not yet surfaced by the local UI)".to_owned(),
            pay_life: None,
            targets: Vec::new(),
            attackers: Vec::new(),
            mana_color: None,
        }];
    }
    if view.step == Step::DeclareAttackers
        && view.decision_player == HUMAN
        && !view.attackers_declared
    {
        return vec![ActionDto {
            kind: "declare_attackers".to_owned(),
            card: None,
            label: "Declare attackers".to_owned(),
            pay_life: None,
            targets: Vec::new(),
            attackers: view
                .own_battlefield
                .iter()
                .filter(|card| card.can_attack)
                .map(|card| card.id.0)
                .collect(),
            mana_color: None,
        }];
    }
    if view.step == Step::DeclareBlockers
        && view.decision_player == HUMAN
        && !view.blockers_declared
    {
        return vec![ActionDto {
            kind: "declare_blockers".to_owned(),
            card: None,
            label: "Declare blockers (or take no blocks)".to_owned(),
            pay_life: None,
            targets: Vec::new(),
            attackers: Vec::new(),
            mana_color: None,
        }];
    }
    if game.priority != HUMAN {
        return Vec::new();
    }
    let mut actions = vec![ActionDto {
        kind: "pass_priority".to_owned(),
        card: None,
        label: "Pass priority".to_owned(),
        pay_life: None,
        targets: Vec::new(),
        attackers: Vec::new(),
        mana_color: None,
    }];
    if view.step.is_main() && view.active_player == HUMAN && view.lands_played == 0 {
        for card in &view.hand {
            if let Some(definition) = card.definition.and_then(|id| game.catalog().get(id))
                && definition.is_land()
            {
                let entry = rav_land_entry_bindings()
                    .into_iter()
                    .find(|binding| binding.card_definition == definition.id);
                if let Some(life) = entry.and_then(|binding| binding.optional_life_payment) {
                    actions.push(ActionDto {
                        kind: "play_land".to_owned(),
                        card: Some(card.id.0),
                        label: format!("Play {} tapped", definition.name),
                        pay_life: Some(false),
                        targets: Vec::new(),
                        attackers: Vec::new(),
                        mana_color: None,
                    });
                    actions.push(ActionDto {
                        kind: "play_land".to_owned(),
                        card: Some(card.id.0),
                        label: format!("Play {} untapped (pay {} life)", definition.name, life),
                        pay_life: Some(true),
                        targets: Vec::new(),
                        attackers: Vec::new(),
                        mana_color: None,
                    });
                } else {
                    actions.push(ActionDto {
                        kind: "play_land".to_owned(),
                        card: Some(card.id.0),
                        label: format!("Play {}", definition.name),
                        pay_life: None,
                        targets: Vec::new(),
                        attackers: Vec::new(),
                        mana_color: None,
                    });
                }
            }
        }
    }
    for card in &view.own_battlefield {
        if !card.tapped && !card.mana_colors.is_empty() && game.priority == HUMAN {
            for color in &card.mana_colors {
                actions.push(ActionDto {
                    kind: "add_mana".to_owned(),
                    card: Some(card.id.0),
                    label: format!("Tap for {color:?}"),
                    pay_life: None,
                    targets: Vec::new(),
                    attackers: Vec::new(),
                    mana_color: Some(format!("{color:?}")),
                });
            }
        }
    }
    for card in &view.hand {
        let Some(definition) = card.definition.and_then(|id| game.catalog().get(id)) else {
            continue;
        };
        if definition.is_land() {
            continue;
        }
        actions.push(ActionDto {
            kind: "cast".to_owned(),
            card: Some(card.id.0),
            label: format!("Cast {}", definition.name),
            pay_life: None,
            targets: target_options(game, HUMAN, definition),
            attackers: Vec::new(),
            mana_color: None,
        });
    }
    actions
}

fn target_options(game: &Game, player: PlayerId, definition: &CardDefinition) -> Vec<TargetDto> {
    let requirement = definition
        .effects
        .iter()
        .flat_map(cardbench_magic_engine::Effect::target_requirements)
        .flatten()
        .next();
    let Some(requirement) = requirement else {
        return Vec::new();
    };
    let mut options = Vec::new();
    if matches!(
        requirement,
        TargetRequirement::Player
            | TargetRequirement::Opponent
            | TargetRequirement::PlayerOrCreature
            | TargetRequirement::Any
    ) {
        for seat in 0..game.players.len() {
            if game.players[seat].lost {
                continue;
            }
            if matches!(requirement, TargetRequirement::Opponent) && seat == player.0 {
                continue;
            }
            options.push(TargetDto {
                kind: "player".to_owned(),
                id: seat as u64,
                label: if seat == player.0 {
                    "You".to_owned()
                } else {
                    "Opponent".to_owned()
                },
            });
        }
    }
    if matches!(
        requirement,
        TargetRequirement::Any
            | TargetRequirement::Permanent
            | TargetRequirement::Creature
            | TargetRequirement::NonblackCreature
            | TargetRequirement::FlyingCreature
            | TargetRequirement::DistinctCreature
            | TargetRequirement::Land
            | TargetRequirement::ControlledLand
            | TargetRequirement::Artifact
            | TargetRequirement::Enchantment
            | TargetRequirement::ArtifactOrCreature
            | TargetRequirement::ArtifactOrEnchantment
            | TargetRequirement::ControlledCreature
            | TargetRequirement::OpponentCreature
            | TargetRequirement::PlayerOrCreature
    ) {
        let mut battlefield = Vec::new();
        for seat in 0..game.players.len() {
            let Ok(view) = game.view_for_player(PlayerId(seat)) else {
                continue;
            };
            battlefield.extend(view.own_battlefield);
        }
        for view in battlefield {
            let card = view.id;
            let allowed = match requirement {
                TargetRequirement::Creature
                | TargetRequirement::NonblackCreature
                | TargetRequirement::FlyingCreature
                | TargetRequirement::DistinctCreature
                | TargetRequirement::ControlledCreature
                | TargetRequirement::OpponentCreature
                | TargetRequirement::PlayerOrCreature => {
                    view.card_types.contains(&CardType::Creature)
                }
                TargetRequirement::Land | TargetRequirement::ControlledLand => {
                    view.card_types.contains(&CardType::Land)
                }
                TargetRequirement::Artifact => view.card_types.contains(&CardType::Artifact),
                TargetRequirement::Enchantment => view.card_types.contains(&CardType::Enchantment),
                TargetRequirement::ArtifactOrCreature => {
                    view.card_types.contains(&CardType::Artifact)
                        || view.card_types.contains(&CardType::Creature)
                }
                TargetRequirement::ArtifactOrEnchantment => {
                    view.card_types.contains(&CardType::Artifact)
                        || view.card_types.contains(&CardType::Enchantment)
                }
                _ => true,
            } && (!matches!(
                requirement,
                TargetRequirement::ControlledCreature | TargetRequirement::ControlledLand
            ) || view.controller == player)
                && (!matches!(requirement, TargetRequirement::OpponentCreature)
                    || view.controller != player);
            if allowed {
                options.push(TargetDto {
                    kind: "permanent".to_owned(),
                    id: card.0,
                    label: card_label(game, card),
                });
            }
        }
    }
    if matches!(
        requirement,
        TargetRequirement::Spell
            | TargetRequirement::PhysicalSpell
            | TargetRequirement::InstantOrSorcerySpell
            | TargetRequirement::NoncreatureSpell
    ) {
        for stack in &game.stack {
            if stack.ability_id.is_none() {
                options.push(TargetDto {
                    kind: "spell".to_owned(),
                    id: stack.card.0,
                    label: card_label(game, stack.card),
                });
            }
        }
    }
    options
}

fn target_to_engine(target: &TargetInput) -> Result<Target, String> {
    match target.kind.as_str() {
        "player" => Ok(Target::Player(PlayerId(
            usize::try_from(target.id).map_err(|_| "player id is too large".to_owned())?,
        ))),
        "permanent" => Ok(Target::Permanent(ObjectId(target.id))),
        "spell" => Ok(Target::Spell(ObjectId(target.id))),
        _ => Err(format!("unknown target kind `{}`", target.kind)),
    }
}

fn seat_dto(game: &Game, player: PlayerId, view: Option<&GameView>) -> SeatDto {
    let state = &game.players[player.0];
    let own = view.filter(|view| view.player == player);
    SeatDto {
        seat: player.0,
        life: state.life,
        hand: own.map_or_else(Vec::new, |view| {
            view.hand.iter().map(|card| card_dto(game, card)).collect()
        }),
        battlefield: own.map_or_else(Vec::new, |view| {
            view.own_battlefield
                .iter()
                .map(|card| card_dto(game, card))
                .collect()
        }),
        graveyard_count: state.graveyard.len(),
        library_count: state.library.len(),
        mana: mana_dto(&state.mana_pool),
        lands_played: state.lands_played,
    }
}

fn stack_dto(game: &Game, view: Option<&GameView>) -> Vec<StackDto> {
    let Some(view) = view else { return Vec::new() };
    game.stack
        .iter()
        .map(|stack| StackDto {
            id: stack.id.0,
            card: view
                .stack_spells
                .iter()
                .find(|card| card.id == stack.card)
                .map(|card| card_dto(game, card)),
            controller: stack.controller.0,
            targets: stack
                .targets
                .iter()
                .map(|target| format!("{target:?}"))
                .collect(),
        })
        .collect()
}

fn card_dto(game: &Game, card: &cardbench_magic_engine::CardView) -> CardDto {
    let definition = card.definition.and_then(|id| game.catalog().get(id));
    CardDto {
        id: card.id.0,
        name: definition.map_or_else(
            || "Token".to_owned(),
            |definition| definition.name.to_owned(),
        ),
        definition: card.definition.map(str::to_owned),
        types: card
            .card_types
            .iter()
            .map(|kind| format!("{kind:?}"))
            .collect(),
        mana_cost: definition.map_or_else(
            || "—".to_owned(),
            |definition| format_mana_cost(&definition.mana_cost),
        ),
        colors: card
            .colors
            .iter()
            .map(|color| format!("{color:?}"))
            .collect(),
        mana_colors: card
            .mana_colors
            .iter()
            .map(|color| format!("{color:?}"))
            .collect(),
        power: definition.and_then(|definition| definition.power),
        toughness: definition.and_then(|definition| definition.toughness),
        tapped: card.tapped,
        can_attack: card.can_attack,
        can_block: card.can_block,
        summoning_sick: card.summoning_sick,
    }
}

fn format_mana_cost(cost: &cardbench_magic_engine::ManaCost) -> String {
    let mut parts = Vec::new();
    if cost.generic > 0 {
        parts.push(cost.generic.to_string());
    }
    parts.extend(cost.colored.iter().map(|color| {
        match color {
            Color::White => "W",
            Color::Blue => "U",
            Color::Black => "B",
            Color::Red => "R",
            Color::Green => "G",
            Color::Colorless => "C",
        }
        .to_owned()
    }));
    parts.extend(
        cost.hybrid
            .iter()
            .map(|symbol| format!("{:?}/{:?}", symbol.first, symbol.second)),
    );
    if parts.is_empty() {
        "0".to_owned()
    } else {
        parts.join("")
    }
}

fn mana_dto(pool: &ManaPool) -> ManaDto {
    ManaDto {
        white: pool.amount(Color::White),
        blue: pool.amount(Color::Blue),
        black: pool.amount(Color::Black),
        red: pool.amount(Color::Red),
        green: pool.amount(Color::Green),
        colorless: pool.amount(Color::Colorless),
    }
}

fn card_label(game: &Game, card: ObjectId) -> String {
    game.card_definition(card).map_or_else(
        |_| format!("Object {}", card.0),
        |definition| definition.name.to_owned(),
    )
}

fn parse_color(value: &str) -> Result<Color, String> {
    match value.to_ascii_lowercase().as_str() {
        "w" | "white" => Ok(Color::White),
        "u" | "blue" => Ok(Color::Blue),
        "b" | "black" => Ok(Color::Black),
        "r" | "red" => Ok(Color::Red),
        "g" | "green" => Ok(Color::Green),
        "c" | "colorless" => Ok(Color::Colorless),
        _ => Err(format!("unknown mana color `{value}`")),
    }
}

fn find_deck<'a>(decks: &'a [DeckFixture], id: &str) -> Result<&'a DeckFixture, String> {
    decks
        .iter()
        .find(|deck| deck.id == id)
        .ok_or_else(|| format!("unknown constructed deck `{id}`"))
}

fn deck_summary(deck: DeckFixture) -> DeckSummary {
    DeckSummary {
        id: deck.id,
        name: deck.name,
        archetype: deck.archetype,
        cards: deck
            .deck
            .mainboard
            .iter()
            .map(|entry| usize::from(entry.count))
            .sum(),
    }
}

fn error_message(error: &str) -> String {
    serde_json::json!({"schema":"cardbench.magic.local.v1.error", "error": error}).to_string()
}
