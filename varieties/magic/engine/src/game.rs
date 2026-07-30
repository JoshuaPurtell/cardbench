use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::{
    CardDefinition, CardObject, CardType, Characteristics, Color, CombatBlock, ContinuousChange,
    ContinuousEffect, DeckList, Duration, Effect, GameEvent, Keyword, ObjectId, PlayerId,
    PlayerState, PolicyMoveKind, StackObject, Step, Target, TargetRequirement, TokenSpec, Zone,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RulesError {
    UnknownPlayer(PlayerId),
    UnknownCard(ObjectId),
    UnknownDefinition(&'static str),
    WrongZone {
        card: ObjectId,
        expected: Zone,
    },
    Priority {
        expected: PlayerId,
        actual: PlayerId,
    },
    IllegalAction(&'static str),
    IllegalTarget(Target),
    Mana(String),
}

impl Display for RulesError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownPlayer(player) => write!(formatter, "unknown player {}", player.0),
            Self::UnknownCard(card) => write!(formatter, "unknown card {}", card.0),
            Self::UnknownDefinition(definition) => {
                write!(formatter, "unknown definition {definition}")
            }
            Self::WrongZone { card, expected } => {
                write!(
                    formatter,
                    "card {} is not in expected zone {expected:?}",
                    card.0
                )
            }
            Self::Priority { expected, actual } => write!(
                formatter,
                "player {} acted without priority; player {} has priority",
                actual.0, expected.0
            ),
            Self::IllegalAction(message) => formatter.write_str(message),
            Self::IllegalTarget(target) => write!(formatter, "illegal target {target:?}"),
            Self::Mana(message) => formatter.write_str(message),
        }
    }
}

impl Error for RulesError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConvokeContribution {
    Generic,
    Color(Color),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConvokePayment {
    pub creature: ObjectId,
    pub contribution: ConvokeContribution,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CastRequest {
    pub card: ObjectId,
    pub targets: Vec<Target>,
    pub convoke: Vec<ConvokePayment>,
}

/// A policy's proposed move. The engine performs all legality checks when submitted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PolicyAction {
    Cast(CastRequest),
    PassPriority,
    PlayLand { card: ObjectId },
    ActivateManaAbility { land: ObjectId, color: Color },
    DeclareAttackers { attackers: Vec<ObjectId> },
    DeclareBlockers { assignments: Vec<CombatBlock> },
    ReportEngineWeakness { code: String, detail: String },
}

impl PolicyAction {
    #[must_use]
    pub const fn kind(&self) -> PolicyMoveKind {
        match self {
            Self::Cast(_) => PolicyMoveKind::Cast,
            Self::PassPriority => PolicyMoveKind::PassPriority,
            Self::PlayLand { .. } => PolicyMoveKind::PlayLand,
            Self::ActivateManaAbility { .. } => PolicyMoveKind::ActivateManaAbility,
            Self::DeclareAttackers { .. } => PolicyMoveKind::DeclareAttackers,
            Self::DeclareBlockers { .. } => PolicyMoveKind::DeclareBlockers,
            Self::ReportEngineWeakness { .. } => PolicyMoveKind::ReportEngineWeakness,
        }
    }
}

/// A deterministic policy-facing view. It excludes the opponent's hand and library.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CardView {
    pub id: ObjectId,
    pub definition: Option<&'static str>,
    pub controller: PlayerId,
    pub tapped: bool,
    pub colors: BTreeSet<Color>,
    pub mana_colors: BTreeSet<Color>,
    pub card_types: BTreeSet<CardType>,
    pub can_attack: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameView {
    pub player: PlayerId,
    pub active_player: PlayerId,
    pub priority: PlayerId,
    /// The player expected to make the next policy decision. It differs from
    /// `priority` only for turn-based declaration actions.
    pub decision_player: PlayerId,
    pub step: Step,
    pub turn: u32,
    pub own_life: i16,
    pub opponent_life: Vec<(PlayerId, i16)>,
    pub mana_pool: crate::ManaPool,
    pub lands_played: u8,
    pub hand: Vec<CardView>,
    pub own_battlefield: Vec<CardView>,
    pub opponent_battlefield: Vec<CardView>,
    pub combat_attackers: Vec<CardView>,
    pub attackers_declared: bool,
    pub blockers_declared: bool,
    pub stack_depth: usize,
}

#[derive(Clone, Debug, Default)]
struct CombatState {
    attackers: Vec<ObjectId>,
    blockers: BTreeMap<ObjectId, ObjectId>,
    attackers_declared: bool,
    blockers_declared: bool,
}

/// A deterministic, two-or-more-player Magic game state.
///
/// Setup helpers (`add_card`, `put_on_battlefield`, and `grant_mana`) intentionally
/// do not emit events. Gameplay methods do, so scenarios can compose compact initial
/// states without contaminating the reference event log.
#[derive(Clone, Debug)]
pub struct Game {
    catalog: BTreeMap<&'static str, CardDefinition>,
    pub players: Vec<PlayerState>,
    objects: BTreeMap<ObjectId, CardObject>,
    pub stack: Vec<StackObject>,
    pub continuous_effects: Vec<ContinuousEffect>,
    pub active_player: PlayerId,
    pub priority: PlayerId,
    pub step: Step,
    pub turn: u32,
    pub event_log: Vec<GameEvent>,
    next_object_id: u64,
    next_timestamp: u64,
    consecutive_passes: usize,
    shuffle_seed: u64,
    combat: Option<CombatState>,
    /// Set only while `draw_card` delegates a pending draw replacement to
    /// `dredge`. This prevents dredge from becoming a free graveyard action.
    pending_draw_replacement: Option<PlayerId>,
}

impl Game {
    pub fn new(
        definitions: impl IntoIterator<Item = CardDefinition>,
        player_count: usize,
    ) -> Result<Self, RulesError> {
        if player_count < 2 {
            return Err(RulesError::IllegalAction(
                "Magic games need at least two seated players",
            ));
        }
        let mut catalog = BTreeMap::new();
        for definition in definitions {
            if catalog.insert(definition.id, definition).is_some() {
                return Err(RulesError::IllegalAction("duplicate card definition id"));
            }
        }
        let players = (0..player_count)
            .map(|index| PlayerState::new(PlayerId(index)))
            .collect();
        let mut game = Self {
            catalog,
            players,
            objects: BTreeMap::new(),
            stack: Vec::new(),
            continuous_effects: Vec::new(),
            active_player: PlayerId(0),
            priority: PlayerId(0),
            step: Step::PrecombatMain,
            turn: 1,
            event_log: Vec::new(),
            next_object_id: 1,
            next_timestamp: 1,
            consecutive_passes: 0,
            shuffle_seed: 0,
            combat: None,
            pending_draw_replacement: None,
        };
        game.event_log.push(GameEvent::StepBegan {
            turn: game.turn,
            active_player: game.active_player,
            step: game.step,
        });
        Ok(game)
    }

    #[must_use]
    pub fn catalog(&self) -> &BTreeMap<&'static str, CardDefinition> {
        &self.catalog
    }

    pub fn player(&self, player: PlayerId) -> Result<&PlayerState, RulesError> {
        self.players
            .get(player.0)
            .ok_or(RulesError::UnknownPlayer(player))
    }

    pub fn object(&self, card: ObjectId) -> Result<&CardObject, RulesError> {
        self.objects.get(&card).ok_or(RulesError::UnknownCard(card))
    }

    pub fn card_definition(&self, card: ObjectId) -> Result<&CardDefinition, RulesError> {
        let object = self.object(card)?;
        let definition = object
            .definition
            .ok_or(RulesError::IllegalAction("token has no card definition"))?;
        self.catalog
            .get(definition)
            .ok_or(RulesError::UnknownDefinition(definition))
    }

    pub fn add_card(
        &mut self,
        owner: PlayerId,
        definition: &'static str,
        zone: Zone,
    ) -> Result<ObjectId, RulesError> {
        self.player(owner)?;
        if !self.catalog.contains_key(definition) {
            return Err(RulesError::UnknownDefinition(definition));
        }
        let id = ObjectId(self.next_object_id);
        self.next_object_id += 1;
        self.objects.insert(
            id,
            CardObject {
                id,
                definition: Some(definition),
                owner,
                controller: owner,
                tapped: false,
                damage: 0,
                counters: BTreeMap::new(),
                entered_turn: self.turn,
                token: None,
            },
        );
        self.place_in_zone(owner, id, zone)?;
        Ok(id)
    }

    pub fn put_on_battlefield(
        &mut self,
        controller: PlayerId,
        definition: &'static str,
    ) -> Result<ObjectId, RulesError> {
        let card = self.add_card(controller, definition, Zone::Battlefield)?;
        self.objects
            .get_mut(&card)
            .ok_or(RulesError::UnknownCard(card))?
            .controller = controller;
        Ok(card)
    }

    pub fn grant_mana(
        &mut self,
        player: PlayerId,
        color: Color,
        amount: u8,
    ) -> Result<(), RulesError> {
        self.player(player)?;
        self.players[player.0].mana_pool.add(color, amount);
        Ok(())
    }

    /// Expands one public deck list into a player's library and shuffles it with
    /// the configured deterministic seed. A caller validates format legality
    /// before loading; the engine enforces only catalog and zone ownership here.
    pub fn load_deck_into_library(
        &mut self,
        player: PlayerId,
        deck: &DeckList,
    ) -> Result<(), RulesError> {
        let state = self.player(player)?;
        if !state.library.is_empty()
            || !state.hand.is_empty()
            || !state.battlefield.is_empty()
            || !state.graveyard.is_empty()
            || !state.exile.is_empty()
        {
            return Err(RulesError::IllegalAction(
                "a deck may be loaded only into an empty player state",
            ));
        }
        let mut cards = 0_u16;
        for entry in &deck.mainboard {
            let definition = self
                .catalog
                .get(entry.card.as_str())
                .ok_or(RulesError::UnknownDefinition(
                    "deck card missing from catalog",
                ))?
                .id;
            for _ in 0..entry.count {
                self.add_card(player, definition, Zone::Library)?;
                cards = cards.saturating_add(1);
            }
        }
        self.event_log.push(GameEvent::DeckLoaded { player, cards });
        self.shuffle_library(player);
        self.event_log
            .push(GameEvent::LibraryShuffled { player, cards });
        self.validate_invariants()
    }

    /// Draws a numbered opening hand from a previously loaded library.
    pub fn draw_opening_hand(&mut self, player: PlayerId, cards: u8) -> Result<(), RulesError> {
        self.player(player)?;
        self.require_game_in_progress()?;
        if self.players[player.0].library.len() < usize::from(cards) {
            return Err(RulesError::IllegalAction(
                "opening hand requires enough cards in library",
            ));
        }
        for _ in 0..cards {
            self.draw_card(player, None)?;
        }
        self.event_log
            .push(GameEvent::OpeningHandDrawn { player, cards });
        self.validate_invariants()
    }

    /// Scenario setup hook for a permanent that began the measured sequence tapped.
    /// It neither represents an in-game action nor writes to the event log.
    pub fn set_tapped_for_setup(&mut self, card: ObjectId, tapped: bool) -> Result<(), RulesError> {
        self.require_zone(card, Zone::Battlefield)?;
        self.objects
            .get_mut(&card)
            .ok_or(RulesError::UnknownCard(card))?
            .tapped = tapped;
        Ok(())
    }

    pub fn add_mana_from_action(
        &mut self,
        player: PlayerId,
        color: Color,
        amount: u8,
    ) -> Result<(), RulesError> {
        self.require_priority(player)?;
        self.players[player.0].mana_pool.add(color, amount);
        self.event_log.push(GameEvent::ManaAdded {
            player,
            color,
            amount,
        });
        Ok(())
    }

    /// Activates a basic intrinsic mana ability. It is an expansion-neutral mana
    /// action: it does not use the stack but requires priority and taps its source.
    pub fn activate_mana_ability(
        &mut self,
        player: PlayerId,
        land: ObjectId,
        color: Color,
    ) -> Result<(), RulesError> {
        self.require_priority(player)?;
        self.require_zone(land, Zone::Battlefield)?;
        let object = self.object(land)?;
        if object.controller != player || object.tapped {
            return Err(RulesError::IllegalAction(
                "mana ability requires an untapped land you control",
            ));
        }
        let definition = self.card_definition(land)?;
        if !definition.is_land() || !definition.mana_colors.contains(&color) {
            return Err(RulesError::IllegalAction(
                "that land cannot produce the requested color",
            ));
        }
        self.objects
            .get_mut(&land)
            .ok_or(RulesError::UnknownCard(land))?
            .tapped = true;
        self.players[player.0].mana_pool.add(color, 1);
        self.event_log.push(GameEvent::ManaAbilityActivated {
            player,
            land,
            color,
        });
        self.event_log.push(GameEvent::ManaAdded {
            player,
            color,
            amount: 1,
        });
        self.consecutive_passes = 0;
        self.validate_invariants()
    }

    #[must_use]
    pub fn zone_of(&self, card: ObjectId) -> Option<Zone> {
        self.players.iter().find_map(|player| {
            if player.library.contains(&card) {
                Some(Zone::Library)
            } else if player.hand.contains(&card) {
                Some(Zone::Hand)
            } else if player.battlefield.contains(&card) {
                Some(Zone::Battlefield)
            } else if player.graveyard.contains(&card) {
                Some(Zone::Graveyard)
            } else if player.exile.contains(&card) {
                Some(Zone::Exile)
            } else {
                None
            }
        })
    }

    /// Drops setup or prior-run events. This is useful at the start of a scenario's
    /// measured action sequence and never alters game state.
    pub fn clear_event_log(&mut self) {
        self.event_log.clear();
    }

    /// Produces the information a deterministic policy may use to propose one move.
    pub fn view_for_player(&self, player: PlayerId) -> Result<GameView, RulesError> {
        let state = self.player(player)?;
        let hand = state
            .hand
            .iter()
            .map(|card| self.card_view(*card))
            .collect::<Result<Vec<_>, _>>()?;
        let own_battlefield = state
            .battlefield
            .iter()
            .map(|card| self.card_view(*card))
            .collect::<Result<Vec<_>, _>>()?;
        let mut opponent_life = Vec::new();
        let mut opponent_battlefield = Vec::new();
        for opponent in self
            .players
            .iter()
            .filter(|candidate| candidate.id != player)
        {
            opponent_life.push((opponent.id, opponent.life));
            opponent_battlefield.extend(
                opponent
                    .battlefield
                    .iter()
                    .map(|card| self.card_view(*card))
                    .collect::<Result<Vec<_>, _>>()?,
            );
        }
        let (combat_attackers, attackers_declared, blockers_declared) =
            if let Some(combat) = &self.combat {
                (
                    combat
                        .attackers
                        .iter()
                        // Combat state retains historical declarations through
                        // end of combat. Tokens that died in combat have no
                        // remaining object, and non-token attackers that left
                        // play are no longer actionable policy information.
                        .filter(|card| self.zone_of(**card) == Some(Zone::Battlefield))
                        .map(|card| self.card_view(*card))
                        .collect::<Result<Vec<_>, _>>()?,
                    combat.attackers_declared,
                    combat.blockers_declared,
                )
            } else {
                (Vec::new(), false, false)
            };
        Ok(GameView {
            player,
            active_player: self.active_player,
            priority: self.priority,
            decision_player: self.policy_decision_player(),
            step: self.step,
            turn: self.turn,
            own_life: state.life,
            opponent_life,
            mana_pool: state.mana_pool.clone(),
            lands_played: state.lands_played,
            hand,
            own_battlefield,
            opponent_battlefield,
            combat_attackers,
            attackers_declared,
            blockers_declared,
            stack_depth: self.stack.len(),
        })
    }

    /// Submits one policy proposal through normal rules enforcement and records the
    /// accepted move in the canonical event log.
    pub fn submit_policy_move(
        &mut self,
        player: PlayerId,
        policy: impl Into<String>,
        action: PolicyAction,
    ) -> Result<(), RulesError> {
        let kind = action.kind();
        match action {
            PolicyAction::Cast(request) => self.cast_spell(player, request)?,
            PolicyAction::PassPriority => self.pass_priority(player)?,
            PolicyAction::PlayLand { card } => self.play_land(player, card)?,
            PolicyAction::ActivateManaAbility { land, color } => {
                self.activate_mana_ability(player, land, color)?;
            }
            PolicyAction::DeclareAttackers { attackers } => {
                self.declare_attackers(player, &attackers)?;
            }
            PolicyAction::DeclareBlockers { assignments } => {
                self.declare_blockers(player, &assignments)?;
            }
            PolicyAction::ReportEngineWeakness { code, detail } => {
                self.report_engine_weakness(player, code, detail)?;
            }
        }
        self.event_log.push(GameEvent::PolicyMoveSubmitted {
            player,
            policy: policy.into(),
            kind,
        });
        self.validate_invariants()
    }

    pub fn characteristics(&self, card: ObjectId) -> Result<Characteristics, RulesError> {
        let object = self.object(card)?;
        let mut characteristics = if let Some(token) = &object.token {
            Characteristics {
                colors: token.colors.clone(),
                card_types: token.card_types.clone(),
                power: Some(token.power),
                toughness: Some(token.toughness),
                keywords: Vec::new(),
            }
        } else {
            let definition = self.card_definition(card)?;
            Characteristics {
                colors: definition.colors.clone(),
                card_types: definition.card_types.clone(),
                power: definition.power,
                toughness: definition.toughness,
                keywords: definition.keywords.clone(),
            }
        };
        let mut effects: Vec<_> = self
            .continuous_effects
            .iter()
            .filter(|effect| effect.target == card && self.effect_is_active(effect))
            .collect();
        effects.sort_by_key(|effect| (effect.change.layer(), effect.timestamp));
        for effect in effects {
            match &effect.change {
                ContinuousChange::AddCardType(card_type) => {
                    characteristics.card_types.insert(card_type.clone());
                }
                ContinuousChange::AddColor(color) => {
                    characteristics.colors.insert(*color);
                }
                ContinuousChange::AddKeyword(keyword) => {
                    characteristics.keywords.push(keyword.clone());
                }
                ContinuousChange::ModifyPowerToughness { power, toughness } => {
                    characteristics.power = characteristics.power.map(|current| current + power);
                    characteristics.toughness =
                        characteristics.toughness.map(|current| current + toughness);
                }
            }
        }
        Ok(characteristics)
    }

    pub fn add_continuous_effect(
        &mut self,
        source: ObjectId,
        target: ObjectId,
        change: ContinuousChange,
        duration: Duration,
    ) -> Result<(), RulesError> {
        self.object(source)?;
        self.object(target)?;
        let layer = change.layer();
        self.continuous_effects.push(ContinuousEffect {
            source,
            target,
            change,
            duration,
            timestamp: self.next_timestamp,
        });
        self.next_timestamp += 1;
        self.event_log.push(GameEvent::ContinuousEffectCreated {
            source,
            target,
            layer,
        });
        Ok(())
    }

    pub fn play_land(&mut self, player: PlayerId, card: ObjectId) -> Result<(), RulesError> {
        self.require_priority(player)?;
        if player != self.active_player || !self.step.is_main() || !self.stack.is_empty() {
            return Err(RulesError::IllegalAction(
                "lands may be played only during your main phase with an empty stack",
            ));
        }
        self.require_zone(card, Zone::Hand)?;
        let definition = self.card_definition(card)?;
        if !definition.is_land() {
            return Err(RulesError::IllegalAction(
                "only a land can be played as a land",
            ));
        }
        if self.players[player.0].lands_played >= 1 {
            return Err(RulesError::IllegalAction("land play limit reached"));
        }
        if self.object(card)?.owner != player {
            return Err(RulesError::IllegalAction(
                "only your own hand card may be played",
            ));
        }
        self.players[player.0].lands_played += 1;
        self.move_to_zone(card, Zone::Battlefield)?;
        self.consecutive_passes = 0;
        self.check_state_based_actions()?;
        self.validate_invariants()
    }

    /// Performs the turn-based action of declaring attackers in the current combat.
    pub fn declare_attackers(
        &mut self,
        player: PlayerId,
        attackers: &[ObjectId],
    ) -> Result<(), RulesError> {
        self.require_priority(player)?;
        if self.step != Step::DeclareAttackers || player != self.active_player {
            return Err(RulesError::IllegalAction(
                "only the active player may declare attackers in this step",
            ));
        }
        if self
            .combat
            .as_ref()
            .is_none_or(|combat| combat.attackers_declared)
        {
            return Err(RulesError::IllegalAction("attackers were already declared"));
        }
        let mut seen = BTreeSet::new();
        for attacker in attackers {
            if !seen.insert(*attacker) {
                return Err(RulesError::IllegalAction("an attacker was declared twice"));
            }
            self.require_zone(*attacker, Zone::Battlefield)?;
            let object = self.object(*attacker)?;
            let characteristics = self.characteristics(*attacker)?;
            if object.controller != player
                || object.tapped
                || object.entered_turn >= self.turn
                || !characteristics.card_types.contains(&CardType::Creature)
                || characteristics.keywords.contains(&Keyword::Defender)
            {
                return Err(RulesError::IllegalAction("illegal attacker"));
            }
        }
        for attacker in attackers {
            self.objects
                .get_mut(attacker)
                .ok_or(RulesError::UnknownCard(*attacker))?
                .tapped = true;
        }
        let combat = self
            .combat
            .as_mut()
            .ok_or(RulesError::IllegalAction("combat was not initialized"))?;
        combat.attackers = attackers.to_vec();
        combat.attackers_declared = true;
        self.event_log.push(GameEvent::AttackersDeclared {
            player,
            attackers: attackers.to_vec(),
        });
        self.consecutive_passes = 0;
        self.validate_invariants()
    }

    /// Performs the turn-based action of assigning zero or one blocker to each
    /// attacker in the initial combat slice.
    pub fn declare_blockers(
        &mut self,
        player: PlayerId,
        assignments: &[CombatBlock],
    ) -> Result<(), RulesError> {
        if self.step != Step::DeclareBlockers || player == self.active_player {
            return Err(RulesError::IllegalAction(
                "only the defending player may declare blockers in this step",
            ));
        }
        let combat = self
            .combat
            .as_ref()
            .ok_or(RulesError::IllegalAction("combat was not initialized"))?;
        if !combat.attackers_declared || combat.blockers_declared {
            return Err(RulesError::IllegalAction("blockers cannot be declared now"));
        }
        if player != self.next_player(self.active_player) {
            return Err(RulesError::IllegalAction(
                "only the defending player may declare blockers",
            ));
        }
        let mut attackers = BTreeSet::new();
        let mut blockers = BTreeSet::new();
        for assignment in assignments {
            if !combat.attackers.contains(&assignment.attacker)
                || !attackers.insert(assignment.attacker)
                || !blockers.insert(assignment.blocker)
            {
                return Err(RulesError::IllegalAction("invalid blocker assignment"));
            }
            self.require_zone(assignment.blocker, Zone::Battlefield)?;
            let object = self.object(assignment.blocker)?;
            if object.controller != player
                || object.tapped
                || !self
                    .characteristics(assignment.blocker)?
                    .card_types
                    .contains(&CardType::Creature)
            {
                return Err(RulesError::IllegalAction("illegal blocker"));
            }
        }
        let combat = self
            .combat
            .as_mut()
            .ok_or(RulesError::IllegalAction("combat was not initialized"))?;
        for assignment in assignments {
            combat
                .blockers
                .insert(assignment.attacker, assignment.blocker);
        }
        combat.blockers_declared = true;
        self.event_log.push(GameEvent::BlockersDeclared {
            player,
            assignments: assignments
                .iter()
                .map(|assignment| (assignment.attacker, assignment.blocker))
                .collect(),
        });
        self.consecutive_passes = 0;
        self.priority = self.priority_after_resolution();
        self.validate_invariants()
    }

    /// Records an observed capability gap without pretending an unsupported rules
    /// interaction succeeded. Policies use this to fail loudly and reproducibly.
    pub fn report_engine_weakness(
        &mut self,
        player: PlayerId,
        code: impl Into<String>,
        detail: impl Into<String>,
    ) -> Result<(), RulesError> {
        self.require_priority(player)?;
        self.event_log.push(GameEvent::EngineWeaknessRevealed {
            player,
            code: code.into(),
            detail: detail.into(),
        });
        Ok(())
    }

    pub fn cast_spell(&mut self, player: PlayerId, request: CastRequest) -> Result<(), RulesError> {
        self.require_priority(player)?;
        self.require_zone(request.card, Zone::Hand)?;
        let definition = self.card_definition(request.card)?.clone();
        if definition.is_land() {
            return Err(RulesError::IllegalAction("lands are played, not cast"));
        }
        if !definition.is_permanent() && definition.effects.is_empty() {
            return Err(RulesError::IllegalAction(
                "this spell's front-face effect is unsupported; report an engine weakness",
            ));
        }
        if definition.is_permanent()
            && (player != self.active_player || !self.step.is_main() || !self.stack.is_empty())
        {
            return Err(RulesError::IllegalAction(
                "non-instant permanent spells require your main phase with an empty stack",
            ));
        }
        if self.object(request.card)?.owner != player {
            return Err(RulesError::IllegalAction(
                "only your own hand card may be cast",
            ));
        }
        self.validate_targets(&definition, &request.targets)?;
        let paid_cost =
            self.pay_cost_with_convoke(player, request.card, &definition, &request.convoke)?;
        self.players[player.0].mana_pool = paid_cost;
        for payment in &request.convoke {
            self.objects
                .get_mut(&payment.creature)
                .ok_or(RulesError::UnknownCard(payment.creature))?
                .tapped = true;
            self.event_log.push(GameEvent::ConvokeUsed {
                player,
                creature: payment.creature,
                contribution: match payment.contribution {
                    ConvokeContribution::Generic => None,
                    ConvokeContribution::Color(color) => Some(color),
                },
            });
        }
        self.remove_from_all_zones(request.card);
        self.stack.push(StackObject {
            card: request.card,
            controller: player,
            targets: request.targets,
            effects: definition.effects,
        });
        self.event_log.push(GameEvent::SpellCast {
            player,
            card: request.card,
        });
        self.consecutive_passes = 0;
        self.priority = self.next_player(player);
        Ok(())
    }

    pub fn pass_priority(&mut self, player: PlayerId) -> Result<(), RulesError> {
        self.require_priority(player)?;
        if self.step == Step::DeclareAttackers
            && self
                .combat
                .as_ref()
                .is_some_and(|combat| !combat.attackers_declared)
        {
            // Direct scenario callers historically advance an empty combat by
            // passing priority. Preserve that concise setup path while policy
            // runs use `DeclareAttackers` and receive an explicit event.
            self.combat
                .as_mut()
                .ok_or(RulesError::IllegalAction("combat was not initialized"))?
                .attackers_declared = true;
        }
        if self.step == Step::DeclareBlockers
            && self
                .combat
                .as_ref()
                .is_some_and(|combat| !combat.blockers_declared)
        {
            self.combat
                .as_mut()
                .ok_or(RulesError::IllegalAction("combat was not initialized"))?
                .blockers_declared = true;
        }
        self.event_log.push(GameEvent::PriorityPassed { player });
        self.consecutive_passes += 1;
        self.priority = self.next_player(player);
        if self.consecutive_passes == self.remaining_player_count() {
            self.consecutive_passes = 0;
            if self.stack.is_empty() {
                self.advance_step()?;
            } else {
                self.resolve_top_of_stack()?;
            }
        }
        Ok(())
    }

    pub fn draw_card(
        &mut self,
        player: PlayerId,
        dredge: Option<ObjectId>,
    ) -> Result<(), RulesError> {
        self.player(player)?;
        self.require_game_in_progress()?;
        if let Some(card) = dredge {
            self.pending_draw_replacement = Some(player);
            let result = self.dredge(player, card);
            self.pending_draw_replacement = None;
            return result;
        }
        let Some(card) = self.players[player.0].library.pop() else {
            self.lose_player(player, "attempted to draw from an empty library");
            self.normalize_priority_after_elimination();
            return Ok(());
        };
        self.players[player.0].hand.push(card);
        self.event_log.push(GameEvent::CardMoved {
            card,
            to: Zone::Hand,
        });
        Ok(())
    }

    pub fn dredge(&mut self, player: PlayerId, card: ObjectId) -> Result<(), RulesError> {
        self.require_game_in_progress()?;
        if self.pending_draw_replacement != Some(player) {
            return Err(RulesError::IllegalAction(
                "dredge may only replace a pending draw",
            ));
        }
        self.require_zone(card, Zone::Graveyard)?;
        if self.object(card)?.owner != player {
            return Err(RulesError::IllegalAction(
                "you may dredge only your own graveyard card",
            ));
        }
        let amount = self
            .card_definition(card)?
            .dredge()
            .ok_or(RulesError::IllegalAction("card has no dredge ability"))?;
        if self.players[player.0].library.len() < usize::from(amount) {
            return Err(RulesError::IllegalAction(
                "dredge replacement requires at least that many cards in library",
            ));
        }
        for _ in 0..amount {
            let milled = self.players[player.0]
                .library
                .pop()
                .ok_or(RulesError::IllegalAction("library changed during dredge"))?;
            self.players[player.0].graveyard.push(milled);
            self.event_log.push(GameEvent::CardMoved {
                card: milled,
                to: Zone::Graveyard,
            });
        }
        self.remove_from_all_zones(card);
        self.players[player.0].hand.push(card);
        self.event_log.push(GameEvent::CardMoved {
            card,
            to: Zone::Hand,
        });
        self.event_log.push(GameEvent::Dredged {
            player,
            card,
            count: amount,
        });
        Ok(())
    }

    /// Resolve transmute's hand-zone activated ability with the searched card selected by
    /// the scenario/policy. The library is shuffled by a deterministic seed afterwards.
    pub fn transmute(
        &mut self,
        player: PlayerId,
        card: ObjectId,
        found: ObjectId,
    ) -> Result<(), RulesError> {
        self.require_priority(player)?;
        if player != self.active_player || !self.step.is_main() || !self.stack.is_empty() {
            return Err(RulesError::IllegalAction(
                "transmute is allowed only during your main phase with an empty stack",
            ));
        }
        self.require_zone(card, Zone::Hand)?;
        self.require_zone(found, Zone::Library)?;
        if self.object(card)?.owner != player || self.object(found)?.owner != player {
            return Err(RulesError::IllegalAction(
                "transmute searches only your own library",
            ));
        }
        let cost = self
            .card_definition(card)?
            .transmute_cost()
            .cloned()
            .ok_or(RulesError::IllegalAction("card has no transmute ability"))?;
        if self.card_definition(found)?.mana_cost.mana_value()
            != self.card_definition(card)?.mana_cost.mana_value()
        {
            return Err(RulesError::IllegalAction(
                "transmute may find only a card with the discarded card's mana value",
            ));
        }
        let mut pool = self.players[player.0].mana_pool.clone();
        pool.pay(&cost).map_err(RulesError::Mana)?;
        self.players[player.0].mana_pool = pool;
        self.move_to_zone(card, Zone::Graveyard)?;
        self.move_to_zone(found, Zone::Hand)?;
        self.shuffle_library(player);
        self.event_log.push(GameEvent::Transmuted {
            player,
            discarded: card,
            found,
        });
        self.consecutive_passes = 0;
        self.priority = self.next_player(player);
        Ok(())
    }

    pub fn set_shuffle_seed(&mut self, seed: u64) {
        self.shuffle_seed = seed;
    }

    /// Applies state-based actions until the game reaches a fixed point.
    pub fn check_state_based_actions(&mut self) -> Result<(), RulesError> {
        loop {
            let mut changed = false;
            for player in 0..self.players.len() {
                let player_id = PlayerId(player);
                if !self.players[player].lost && self.players[player].life <= 0 {
                    self.lose_player(player_id, "life total is zero or less");
                    changed = true;
                }
            }
            for card in self.all_battlefield_cards() {
                let characteristics = self.characteristics(card)?;
                if !characteristics.card_types.contains(&CardType::Creature) {
                    continue;
                }
                let toughness = characteristics.toughness.unwrap_or(0);
                let damage = self.object(card)?.damage;
                let reason = if toughness <= 0 {
                    Some("creature has toughness zero or less")
                } else if damage > 0 && damage >= toughness {
                    Some("creature has lethal damage")
                } else {
                    None
                };
                if let Some(reason) = reason {
                    self.event_log
                        .push(GameEvent::StateBasedAction { card, reason });
                    self.move_to_graveyard_or_remove_token(card)?;
                    changed = true;
                }
            }
            if !changed {
                self.normalize_priority_after_elimination();
                return Ok(());
            }
        }
    }

    #[must_use]
    pub fn canonical_event_log(&self) -> Vec<String> {
        self.event_log
            .iter()
            .map(|event| format!("{event:?}"))
            .collect()
    }

    #[must_use]
    pub fn is_game_over(&self) -> bool {
        self.players.iter().filter(|player| !player.lost).count() <= 1
    }

    #[must_use]
    pub fn winner(&self) -> Option<PlayerId> {
        let remaining: Vec<_> = self
            .players
            .iter()
            .filter(|player| !player.lost)
            .map(|player| player.id)
            .collect();
        (remaining.len() == 1).then_some(remaining[0])
    }

    /// Returns the player who must provide the next policy decision. During
    /// declaration steps this is the turn-based actor rather than the player who
    /// will receive priority afterwards.
    #[must_use]
    pub fn next_policy_player(&self) -> PlayerId {
        self.policy_decision_player()
    }

    /// Validates the non-negotiable internal facts relied on by all rule methods.
    /// It is public so development runners and scenario tests can fail at the first
    /// corrupted state rather than report a misleading later rules error.
    #[allow(clippy::too_many_lines)] // One ordered audit keeps the invariant contract reviewable.
    pub fn validate_invariants(&self) -> Result<(), RulesError> {
        if self.players.len() < 2
            || self.players.get(self.active_player.0).is_none()
            || self.players.get(self.priority.0).is_none()
        {
            return Err(RulesError::IllegalAction("invalid seated-player state"));
        }
        if !self.is_game_over() && self.players[self.priority.0].lost {
            return Err(RulesError::IllegalAction(
                "a continuing game assigned priority to an eliminated player",
            ));
        }
        let mut locations = BTreeMap::<ObjectId, Zone>::new();
        for player in &self.players {
            if self.players.get(player.id.0).is_none() || self.players[player.id.0].id != player.id
            {
                return Err(RulesError::IllegalAction("player id does not match seat"));
            }
            for (zone, cards) in [
                (Zone::Library, &player.library),
                (Zone::Hand, &player.hand),
                (Zone::Battlefield, &player.battlefield),
                (Zone::Graveyard, &player.graveyard),
                (Zone::Exile, &player.exile),
            ] {
                for card in cards {
                    if locations.insert(*card, zone).is_some() {
                        return Err(RulesError::IllegalAction(
                            "card appears in more than one zone",
                        ));
                    }
                    let object = self.object(*card)?;
                    if object.id != *card {
                        return Err(RulesError::IllegalAction("object id key mismatch"));
                    }
                    if (zone == Zone::Battlefield && object.controller != player.id)
                        || (zone != Zone::Battlefield && object.owner != player.id)
                    {
                        return Err(RulesError::IllegalAction(
                            "card is in the wrong player's zone",
                        ));
                    }
                    if let Some(definition) = object.definition
                        && !self.catalog.contains_key(definition)
                    {
                        return Err(RulesError::UnknownDefinition(definition));
                    }
                }
            }
        }
        let mut stack_cards = BTreeSet::new();
        for stack_object in &self.stack {
            if !stack_cards.insert(stack_object.card) || locations.contains_key(&stack_object.card)
            {
                return Err(RulesError::IllegalAction(
                    "stack card must not also exist in a zone",
                ));
            }
            self.object(stack_object.card)?;
            self.player(stack_object.controller)?;
        }
        for card in self.objects.keys() {
            if !locations.contains_key(card) && !stack_cards.contains(card) {
                return Err(RulesError::IllegalAction("object has no game location"));
            }
        }
        if let Some(combat) = &self.combat {
            if !matches!(
                self.step,
                Step::DeclareAttackers
                    | Step::DeclareBlockers
                    | Step::CombatDamage
                    | Step::EndOfCombat
            ) {
                return Err(RulesError::IllegalAction(
                    "combat state exists outside combat",
                ));
            }
            let mut attackers = BTreeSet::new();
            let mut blockers = BTreeSet::new();
            let declarations_are_current =
                matches!(self.step, Step::DeclareAttackers | Step::DeclareBlockers);
            for attacker in &combat.attackers {
                if !attackers.insert(*attacker)
                    || (declarations_are_current
                        && (self.zone_of(*attacker) != Some(Zone::Battlefield)
                            || self.object(*attacker)?.controller != self.active_player))
                {
                    return Err(RulesError::IllegalAction("invalid combat attacker state"));
                }
                if self.zone_of(*attacker).is_some() {
                    self.object(*attacker)?;
                }
            }
            for (attacker, blocker) in &combat.blockers {
                if !attackers.contains(attacker)
                    || !blockers.insert(*blocker)
                    || (declarations_are_current
                        && (self.zone_of(*blocker) != Some(Zone::Battlefield)
                            || self.object(*blocker)?.controller == self.active_player))
                {
                    return Err(RulesError::IllegalAction("invalid combat blocker state"));
                }
                if self.zone_of(*blocker).is_some() {
                    self.object(*blocker)?;
                }
            }
        }
        Ok(())
    }

    fn pay_cost_with_convoke(
        &self,
        player: PlayerId,
        card: ObjectId,
        definition: &CardDefinition,
        payments: &[ConvokePayment],
    ) -> Result<crate::ManaPool, RulesError> {
        if !definition.has_convoke() && !payments.is_empty() {
            return Err(RulesError::IllegalAction(
                "only convoke spells accept convoke payments",
            ));
        }
        let mut remaining = definition.mana_cost.clone();
        let mut seen = BTreeSet::new();
        for payment in payments {
            if !seen.insert(payment.creature) {
                return Err(RulesError::IllegalAction(
                    "a creature may convoke only once",
                ));
            }
            self.require_zone(payment.creature, Zone::Battlefield)?;
            let creature = self.object(payment.creature)?;
            if creature.controller != player || creature.tapped {
                return Err(RulesError::IllegalAction(
                    "convoke requires an untapped creature you control",
                ));
            }
            let characteristics = self.characteristics(payment.creature)?;
            if !characteristics.card_types.contains(&CardType::Creature) {
                return Err(RulesError::IllegalAction("only creatures can convoke"));
            }
            match payment.contribution {
                ConvokeContribution::Generic => {
                    if remaining.generic == 0 {
                        return Err(RulesError::IllegalAction(
                            "no generic mana remains to convoke",
                        ));
                    }
                    remaining.generic -= 1;
                }
                ConvokeContribution::Color(color) => {
                    if !characteristics.colors.contains(&color) {
                        return Err(RulesError::IllegalAction(
                            "convoke creature must have the contributed color",
                        ));
                    }
                    let position = remaining
                        .colored
                        .iter()
                        .position(|required| *required == color)
                        .ok_or(RulesError::IllegalAction(
                            "that colored mana does not remain to convoke",
                        ))?;
                    remaining.colored.remove(position);
                }
            }
        }
        let mut pool = self.players[player.0].mana_pool.clone();
        pool.pay(&remaining).map_err(RulesError::Mana)?;
        let _ = card;
        Ok(pool)
    }

    fn validate_targets(
        &self,
        definition: &CardDefinition,
        targets: &[Target],
    ) -> Result<(), RulesError> {
        let requirements: Vec<_> = definition
            .effects
            .iter()
            .filter_map(Effect::target_requirement)
            .collect();
        if requirements.is_empty() {
            if targets.is_empty() {
                return Ok(());
            }
            return Err(RulesError::IllegalAction(
                "this spell does not take targets",
            ));
        }
        if targets.len() != 1 {
            return Err(RulesError::IllegalAction(
                "the initial CardBench spell slice supports exactly one target",
            ));
        }
        if requirements
            .iter()
            .any(|requirement| !self.target_matches(targets[0], *requirement))
        {
            return Err(RulesError::IllegalTarget(targets[0]));
        }
        Ok(())
    }

    fn resolve_top_of_stack(&mut self) -> Result<(), RulesError> {
        let stack_object = self.stack.pop().ok_or(RulesError::IllegalAction(
            "attempted to resolve an empty stack",
        ))?;
        let legal_target = stack_object
            .effects
            .iter()
            .filter_map(Effect::target_requirement)
            .all(|requirement| {
                stack_object
                    .targets
                    .first()
                    .is_some_and(|target| self.target_matches(*target, requirement))
            });
        if !legal_target {
            self.event_log.push(GameEvent::SpellCounteredByRules {
                card: stack_object.card,
            });
            self.move_to_graveyard_or_remove_token(stack_object.card)?;
            self.check_state_based_actions()?;
            self.priority = self.priority_after_resolution();
            return Ok(());
        }
        for effect in &stack_object.effects {
            self.resolve_effect(
                stack_object.card,
                stack_object.controller,
                effect,
                &stack_object.targets,
            )?;
        }
        self.event_log.push(GameEvent::SpellResolved {
            card: stack_object.card,
        });
        if self.card_definition(stack_object.card)?.is_permanent() {
            self.move_to_zone(stack_object.card, Zone::Battlefield)?;
        } else {
            self.move_to_graveyard_or_remove_token(stack_object.card)?;
        }
        self.check_state_based_actions()?;
        self.priority = self.priority_after_resolution();
        Ok(())
    }

    #[allow(clippy::too_many_lines)] // Effect dispatch stays centralized so stack resolution has one rules path.
    fn resolve_effect(
        &mut self,
        source: ObjectId,
        controller: PlayerId,
        effect: &Effect,
        targets: &[Target],
    ) -> Result<(), RulesError> {
        match effect {
            Effect::DealDamage { amount, .. } => match targets
                .first()
                .ok_or(RulesError::IllegalAction("missing damage target"))?
            {
                Target::Player(player) => {
                    self.players[player.0].life -= amount;
                    self.event_log.push(GameEvent::DamageDealtToPlayer {
                        source,
                        player: *player,
                        amount: *amount,
                    });
                }
                Target::Permanent(permanent) => {
                    self.objects
                        .get_mut(permanent)
                        .ok_or(RulesError::UnknownCard(*permanent))?
                        .damage += amount;
                    self.event_log.push(GameEvent::DamageDealtToPermanent {
                        source,
                        permanent: *permanent,
                        amount: *amount,
                    });
                }
            },
            Effect::DealDamageController { amount } => {
                self.players[controller.0].life -= amount;
                self.event_log.push(GameEvent::DamageDealtToPlayer {
                    source,
                    player: controller,
                    amount: *amount,
                });
            }
            Effect::GainLifeController { amount } => {
                self.players[controller.0].life += amount;
                self.event_log.push(GameEvent::LifeGained {
                    player: controller,
                    amount: *amount,
                });
            }
            Effect::CreateToken { token, count } => {
                for _ in 0..*count {
                    let token_id = self.create_token(controller, token.clone())?;
                    self.event_log.push(GameEvent::TokenCreated {
                        player: controller,
                        token: token_id,
                    });
                }
            }
            Effect::ModifyTargetPtUntilEndOfTurn { power, toughness } => {
                let target = Self::target_permanent(targets)?;
                self.add_continuous_effect(
                    source,
                    target,
                    ContinuousChange::ModifyPowerToughness {
                        power: *power,
                        toughness: *toughness,
                    },
                    Duration::EndOfTurn(self.turn),
                )?;
            }
            Effect::RadianceUntapAndModifyUntilEndOfTurn { power, toughness } => {
                let target = Self::target_permanent(targets)?;
                let target_colors = self.characteristics(target)?.colors;
                let matching: Vec<_> = self
                    .all_battlefield_cards()
                    .into_iter()
                    .filter(|candidate| {
                        self.characteristics(*candidate)
                            .is_ok_and(|characteristics| {
                                characteristics.card_types.contains(&CardType::Creature)
                                    && !characteristics.colors.is_disjoint(&target_colors)
                            })
                    })
                    .collect();
                let mut untapped = Vec::new();
                for candidate in matching {
                    let object = self
                        .objects
                        .get_mut(&candidate)
                        .ok_or(RulesError::UnknownCard(candidate))?;
                    if object.tapped {
                        object.tapped = false;
                        untapped.push(candidate);
                    }
                    self.add_continuous_effect(
                        source,
                        candidate,
                        ContinuousChange::ModifyPowerToughness {
                            power: *power,
                            toughness: *toughness,
                        },
                        Duration::EndOfTurn(self.turn),
                    )?;
                }
                if !untapped.is_empty() {
                    self.event_log.push(GameEvent::PermanentsUntapped {
                        player: controller,
                        cards: untapped,
                    });
                }
            }
        }
        Ok(())
    }

    fn target_permanent(targets: &[Target]) -> Result<ObjectId, RulesError> {
        match targets.first() {
            Some(Target::Permanent(card)) => Ok(*card),
            Some(Target::Player(player)) => Err(RulesError::IllegalTarget(Target::Player(*player))),
            None => Err(RulesError::IllegalAction("missing permanent target")),
        }
    }

    fn target_matches(&self, target: Target, requirement: TargetRequirement) -> bool {
        match (target, requirement) {
            (Target::Player(player), TargetRequirement::Any | TargetRequirement::Player) => {
                self.players.get(player.0).is_some_and(|state| !state.lost)
            }
            (Target::Permanent(card), TargetRequirement::Any) => {
                self.zone_of(card) == Some(Zone::Battlefield)
            }
            (Target::Permanent(card), TargetRequirement::Creature) => {
                self.zone_of(card) == Some(Zone::Battlefield)
                    && self.characteristics(card).is_ok_and(|characteristics| {
                        characteristics.card_types.contains(&CardType::Creature)
                    })
            }
            _ => false,
        }
    }

    fn advance_step(&mut self) -> Result<(), RulesError> {
        for player in &mut self.players {
            player.mana_pool.clear();
        }
        self.step = self.step.next();
        if self.step == Step::Untap {
            self.active_player = self.next_player(self.active_player);
            self.turn += 1;
        }
        self.priority = self.priority_after_resolution();
        self.start_step()
    }

    fn start_step(&mut self) -> Result<(), RulesError> {
        if self.step == Step::CombatDamage {
            self.resolve_combat_damage()?;
        }
        match self.step {
            Step::Untap => {
                self.players[self.active_player.0].lands_played = 0;
                let battlefield = self.players[self.active_player.0].battlefield.clone();
                let mut untapped = Vec::new();
                for card in battlefield {
                    let object = self
                        .objects
                        .get_mut(&card)
                        .ok_or(RulesError::UnknownCard(card))?;
                    if object.tapped {
                        object.tapped = false;
                        untapped.push(card);
                    }
                }
                if !untapped.is_empty() {
                    self.event_log.push(GameEvent::PermanentsUntapped {
                        player: self.active_player,
                        cards: untapped,
                    });
                }
            }
            Step::Draw => {
                if self.turn != 1 || self.active_player != PlayerId(0) {
                    self.draw_card(self.active_player, None)?;
                }
            }
            Step::DeclareAttackers => {
                self.combat = Some(CombatState::default());
            }
            Step::EndOfCombat => {
                self.combat = None;
            }
            Step::Cleanup => {
                for card in self.all_battlefield_cards() {
                    self.objects
                        .get_mut(&card)
                        .ok_or(RulesError::UnknownCard(card))?
                        .damage = 0;
                }
                self.continuous_effects
                    .retain(|effect| effect.duration != Duration::EndOfTurn(self.turn));
                self.check_state_based_actions()?;
            }
            _ => {}
        }
        self.event_log.push(GameEvent::StepBegan {
            turn: self.turn,
            active_player: self.active_player,
            step: self.step,
        });
        Ok(())
    }

    fn resolve_combat_damage(&mut self) -> Result<(), RulesError> {
        let combat = self.combat.clone().ok_or(RulesError::IllegalAction(
            "combat damage without combat state",
        ))?;
        if !combat.attackers_declared || !combat.blockers_declared {
            return Err(RulesError::IllegalAction(
                "combat damage before attackers and blockers were declared",
            ));
        }
        let defending_player = self.next_player(self.active_player);
        let mut permanent_damage = Vec::new();
        let mut player_damage = Vec::new();
        for attacker in combat.attackers {
            if self.zone_of(attacker) != Some(Zone::Battlefield) {
                continue;
            }
            let attacker_power = self
                .characteristics(attacker)?
                .power
                .ok_or(RulesError::IllegalAction("attacker lacks power"))?;
            if let Some(blocker) = combat.blockers.get(&attacker).copied() {
                if self.zone_of(blocker) != Some(Zone::Battlefield) {
                    player_damage.push((attacker, defending_player, attacker_power));
                    continue;
                }
                let blocker_power = self
                    .characteristics(blocker)?
                    .power
                    .ok_or(RulesError::IllegalAction("blocker lacks power"))?;
                permanent_damage.push((attacker, blocker, attacker_power));
                permanent_damage.push((blocker, attacker, blocker_power));
            } else {
                player_damage.push((attacker, defending_player, attacker_power));
            }
        }
        for (source, permanent, amount) in permanent_damage {
            self.objects
                .get_mut(&permanent)
                .ok_or(RulesError::UnknownCard(permanent))?
                .damage += amount;
            self.event_log.push(GameEvent::DamageDealtToPermanent {
                source,
                permanent,
                amount,
            });
        }
        for (source, player, amount) in player_damage {
            self.players[player.0].life -= amount;
            self.event_log.push(GameEvent::DamageDealtToPlayer {
                source,
                player,
                amount,
            });
        }
        self.check_state_based_actions()?;
        Ok(())
    }

    fn shuffle_library(&mut self, player: PlayerId) {
        let seed = self.shuffle_seed;
        self.players[player.0]
            .library
            .sort_by_key(|card| deterministic_mix(seed ^ card.0));
        self.shuffle_seed = self.shuffle_seed.wrapping_add(1);
    }

    fn create_token(
        &mut self,
        controller: PlayerId,
        token: TokenSpec,
    ) -> Result<ObjectId, RulesError> {
        self.player(controller)?;
        let id = ObjectId(self.next_object_id);
        self.next_object_id += 1;
        self.objects.insert(
            id,
            CardObject {
                id,
                definition: None,
                owner: controller,
                controller,
                tapped: false,
                damage: 0,
                counters: BTreeMap::new(),
                entered_turn: self.turn,
                token: Some(token),
            },
        );
        self.place_in_zone(controller, id, Zone::Battlefield)?;
        Ok(id)
    }

    fn move_to_graveyard_or_remove_token(&mut self, card: ObjectId) -> Result<(), RulesError> {
        if self.object(card)?.token.is_some() {
            self.remove_from_all_zones(card);
            self.objects.remove(&card);
            self.continuous_effects
                .retain(|effect| effect.source != card && effect.target != card);
            return Ok(());
        }
        self.move_to_zone(card, Zone::Graveyard)
    }

    fn move_to_zone(&mut self, card: ObjectId, zone: Zone) -> Result<(), RulesError> {
        let object = self.object(card)?.clone();
        self.remove_from_all_zones(card);
        let destination_owner = if zone == Zone::Battlefield {
            object.controller
        } else {
            object.owner
        };
        if zone == Zone::Battlefield {
            let battlefield_object = self
                .objects
                .get_mut(&card)
                .ok_or(RulesError::UnknownCard(card))?;
            battlefield_object.tapped = false;
            battlefield_object.damage = 0;
            battlefield_object.entered_turn = self.turn;
        }
        self.place_in_zone(destination_owner, card, zone)?;
        self.event_log.push(GameEvent::CardMoved { card, to: zone });
        Ok(())
    }

    fn place_in_zone(
        &mut self,
        player: PlayerId,
        card: ObjectId,
        zone: Zone,
    ) -> Result<(), RulesError> {
        self.player(player)?;
        let state = &mut self.players[player.0];
        match zone {
            Zone::Library => state.library.push(card),
            Zone::Hand => state.hand.push(card),
            Zone::Battlefield => state.battlefield.push(card),
            Zone::Graveyard => state.graveyard.push(card),
            Zone::Exile => state.exile.push(card),
        }
        Ok(())
    }

    fn remove_from_all_zones(&mut self, card: ObjectId) {
        for player in &mut self.players {
            player.library.retain(|candidate| *candidate != card);
            player.hand.retain(|candidate| *candidate != card);
            player.battlefield.retain(|candidate| *candidate != card);
            player.graveyard.retain(|candidate| *candidate != card);
            player.exile.retain(|candidate| *candidate != card);
        }
    }

    fn require_zone(&self, card: ObjectId, expected: Zone) -> Result<(), RulesError> {
        self.object(card)?;
        if self.zone_of(card) == Some(expected) {
            Ok(())
        } else {
            Err(RulesError::WrongZone { card, expected })
        }
    }

    fn require_priority(&self, player: PlayerId) -> Result<(), RulesError> {
        self.player(player)?;
        self.require_game_in_progress()?;
        if self.players[player.0].lost {
            return Err(RulesError::IllegalAction("an eliminated player cannot act"));
        }
        if player == self.priority {
            Ok(())
        } else {
            Err(RulesError::Priority {
                expected: self.priority,
                actual: player,
            })
        }
    }

    fn next_player(&self, player: PlayerId) -> PlayerId {
        for offset in 1..=self.players.len() {
            let candidate = PlayerId((player.0 + offset) % self.players.len());
            if !self.players[candidate.0].lost {
                return candidate;
            }
        }
        player
    }

    fn remaining_player_count(&self) -> usize {
        self.players.iter().filter(|player| !player.lost).count()
    }

    fn priority_after_resolution(&self) -> PlayerId {
        if self.players[self.active_player.0].lost {
            self.next_player(self.active_player)
        } else {
            self.active_player
        }
    }

    fn normalize_priority_after_elimination(&mut self) {
        if !self.is_game_over() && self.players[self.priority.0].lost {
            self.priority = self.next_player(self.priority);
            self.consecutive_passes = 0;
        }
    }

    fn require_game_in_progress(&self) -> Result<(), RulesError> {
        if self.is_game_over() {
            Err(RulesError::IllegalAction("the game has already ended"))
        } else {
            Ok(())
        }
    }

    fn policy_decision_player(&self) -> PlayerId {
        match (&self.combat, self.step) {
            (Some(combat), Step::DeclareAttackers)
                if !combat.attackers_declared && !self.players[self.active_player.0].lost =>
            {
                self.active_player
            }
            (Some(combat), Step::DeclareBlockers) if !combat.blockers_declared => {
                self.next_player(self.active_player)
            }
            _ => self.priority,
        }
    }

    fn all_battlefield_cards(&self) -> Vec<ObjectId> {
        self.players
            .iter()
            .flat_map(|player| player.battlefield.iter().copied())
            .collect()
    }

    fn card_view(&self, card: ObjectId) -> Result<CardView, RulesError> {
        let object = self.object(card)?;
        let characteristics = self.characteristics(card)?;
        let mana_colors = object
            .definition
            .and_then(|definition| self.catalog.get(definition))
            .map_or_else(BTreeSet::new, |definition| definition.mana_colors.clone());
        let can_attack = object.controller == self.active_player
            && self.zone_of(card) == Some(Zone::Battlefield)
            && !object.tapped
            && object.entered_turn < self.turn
            && characteristics.card_types.contains(&CardType::Creature)
            && !characteristics.keywords.contains(&Keyword::Defender);
        Ok(CardView {
            id: card,
            definition: object.definition,
            controller: object.controller,
            tapped: object.tapped,
            colors: characteristics.colors,
            mana_colors,
            card_types: characteristics.card_types,
            can_attack,
        })
    }

    fn effect_is_active(&self, effect: &ContinuousEffect) -> bool {
        match effect.duration {
            Duration::EndOfTurn(turn) => turn == self.turn,
            Duration::Permanent => self.zone_of(effect.source) == Some(Zone::Battlefield),
        }
    }

    fn lose_player(&mut self, player: PlayerId, reason: &'static str) {
        if !self.players[player.0].lost {
            self.players[player.0].lost = true;
            self.event_log
                .push(GameEvent::PlayerLost { player, reason });
        }
    }
}

#[must_use]
fn deterministic_mix(mut value: u64) -> u64 {
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^= value >> 27;
    value = value.wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_stack_passes_advance_through_a_full_turn() {
        let mut game = Game::new(Vec::<CardDefinition>::new(), 2).expect("two-player game");
        for _ in 0..9 {
            let first = game.priority;
            game.pass_priority(first).expect("first pass");
            let second = game.priority;
            game.pass_priority(second).expect("second pass");
        }
        assert_eq!(game.turn, 2);
        assert_eq!(game.active_player, PlayerId(1));
        assert_eq!(game.step, Step::Untap);
        assert_eq!(game.priority, PlayerId(1));
    }
}
