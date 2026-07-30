use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::{
    ActivatedManaAbility, CardDefinition, CardObject, CardType, Characteristics, Color,
    CombatBlock, ContinuousChange, ContinuousEffect, DeckList, Duration, Effect, GameEvent,
    Keyword, ManaAbilityActivation, ManaAbilityBinding, ManaAbilityOutput, ObjectId, PlayerId,
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
    /// Chooses the normal draw or one legal dredge replacement at a draw-step
    /// replacement-decision boundary. This is not priority.
    Draw {
        dredge: Option<ObjectId>,
    },
    /// Activates transmute, selecting the matching mana-value card from the
    /// controller's library. The engine enforces timing and payment.
    Transmute {
        card: ObjectId,
        found: ObjectId,
    },
    PassPriority,
    PlayLand {
        card: ObjectId,
    },
    ActivateManaAbility {
        land: ObjectId,
        color: Color,
    },
    /// Activates a generic ability supplied through the game's definition-bound
    /// mana-ability catalog. It is still a mana ability: it never uses the stack.
    ActivateBoundManaAbility {
        activation: ManaAbilityActivation,
    },
    DeclareAttackers {
        attackers: Vec<ObjectId>,
    },
    DeclareBlockers {
        assignments: Vec<CombatBlock>,
    },
    ReportEngineWeakness {
        code: String,
        detail: String,
    },
}

impl PolicyAction {
    #[must_use]
    pub const fn kind(&self) -> PolicyMoveKind {
        match self {
            Self::Cast(_) => PolicyMoveKind::Cast,
            Self::Draw { .. } => PolicyMoveKind::Draw,
            Self::Transmute { .. } => PolicyMoveKind::Transmute,
            Self::PassPriority => PolicyMoveKind::PassPriority,
            Self::PlayLand { .. } => PolicyMoveKind::PlayLand,
            Self::ActivateManaAbility { .. } => PolicyMoveKind::ActivateManaAbility,
            Self::ActivateBoundManaAbility { .. } => PolicyMoveKind::ActivateBoundManaAbility,
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

/// Controller-only legal search choices for one transmute card in hand.
///
/// This projects only cards that the named ability could find; it does not
/// expose either player's full library to a general code policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransmuteSearchView {
    pub card: ObjectId,
    pub candidates: Vec<CardView>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameView {
    pub player: PlayerId,
    pub active_player: PlayerId,
    pub priority: PlayerId,
    /// The player expected to make the next policy decision. It differs from
    /// `priority` for turn-based declarations and mandatory draw replacements.
    pub decision_player: PlayerId,
    pub step: Step,
    pub turn: u32,
    pub own_life: i16,
    pub opponent_life: Vec<(PlayerId, i16)>,
    pub mana_pool: crate::ManaPool,
    pub lands_played: u8,
    pub hand: Vec<CardView>,
    /// True only for the active player while a draw must be taken or replaced.
    pub draw_replacement_pending: bool,
    /// Controller-visible, currently legal dredge choices for that pending draw.
    pub dredge_candidates: Vec<CardView>,
    /// Legal controller-owned library choices for each transmute card in hand.
    pub transmute_searches: Vec<TransmuteSearchView>,
    pub own_battlefield: Vec<CardView>,
    pub opponent_battlefield: Vec<CardView>,
    pub combat_attackers: Vec<CardView>,
    pub attackers_declared: bool,
    pub blockers_declared: bool,
    /// Public spell-card identities currently on the stack. Policies need this
    /// narrow projection to submit a legal counterspell target without seeing
    /// either player's hidden zones.
    pub stack_spells: Vec<CardView>,
    pub stack_depth: usize,
}

#[derive(Clone, Debug, Default)]
struct CombatState {
    attackers: Vec<ObjectId>,
    blockers: BTreeMap<ObjectId, ObjectId>,
    /// This initial slice attacks the next living seat. It records that seat
    /// at declaration time rather than recomputing turn order after a player
    /// leaves in the middle of combat.
    defending_player: Option<PlayerId>,
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
    mana_abilities: BTreeMap<&'static str, BTreeMap<&'static str, ActivatedManaAbility>>,
    pub players: Vec<PlayerState>,
    objects: BTreeMap<ObjectId, CardObject>,
    pub stack: Vec<StackObject>,
    pub continuous_effects: Vec<ContinuousEffect>,
    pub active_player: PlayerId,
    pub priority: PlayerId,
    pub step: Step,
    pub turn: u32,
    pub event_log: Vec<GameEvent>,
    // Kept private so the public, replay-facing event log can still be read
    // directly while the invariant audit detects an out-of-transition edit.
    event_log_integrity: Vec<GameEvent>,
    seated_player_count: usize,
    next_object_id: u64,
    next_timestamp: u64,
    consecutive_passes: usize,
    shuffle_seed: u64,
    /// `Game::new` intentionally leaves a fixture/setup state available. A
    /// full-deck runner must call `begin_game` after decks and opening hands
    /// are established, which starts turn one at Untap then Upkeep.
    started: bool,
    terminal_event_emitted: bool,
    combat: Option<CombatState>,
    /// The player currently choosing a replacement for a draw. This prevents
    /// dredge from becoming a free graveyard action and makes that compulsory
    /// decision visible to submitted policies.
    pending_draw_replacement: Option<PlayerId>,
}

impl Game {
    pub fn new(
        definitions: impl IntoIterator<Item = CardDefinition>,
        player_count: usize,
    ) -> Result<Self, RulesError> {
        Self::new_with_mana_abilities(
            definitions,
            player_count,
            std::iter::empty::<ManaAbilityBinding>(),
        )
    }

    /// Creates a game with expansion-provided, definition-bound activated mana
    /// abilities. The existing `Game::new` remains a no-binding convenience for
    /// current compact card slices.
    pub fn new_with_mana_abilities(
        definitions: impl IntoIterator<Item = CardDefinition>,
        player_count: usize,
        bindings: impl IntoIterator<Item = ManaAbilityBinding>,
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
        let mut mana_abilities =
            BTreeMap::<&'static str, BTreeMap<&'static str, ActivatedManaAbility>>::new();
        for binding in bindings {
            let definition = catalog
                .get(binding.card_definition)
                .ok_or(RulesError::UnknownDefinition(binding.card_definition))?;
            if !definition.is_permanent() {
                return Err(RulesError::IllegalAction(
                    "a definition-bound mana ability requires a permanent source",
                ));
            }
            Self::validate_mana_ability_definition(&binding.ability)?;
            let abilities = mana_abilities.entry(binding.card_definition).or_default();
            if abilities
                .insert(binding.ability.id, binding.ability)
                .is_some()
            {
                return Err(RulesError::IllegalAction(
                    "duplicate mana ability id for card definition",
                ));
            }
        }
        let players = (0..player_count)
            .map(|index| PlayerState::new(PlayerId(index)))
            .collect();
        let game = Self {
            catalog,
            mana_abilities,
            players,
            objects: BTreeMap::new(),
            stack: Vec::new(),
            continuous_effects: Vec::new(),
            active_player: PlayerId(0),
            priority: PlayerId(0),
            step: Step::PrecombatMain,
            turn: 1,
            event_log: Vec::new(),
            event_log_integrity: Vec::new(),
            seated_player_count: player_count,
            next_object_id: 1,
            next_timestamp: 1,
            consecutive_passes: 0,
            shuffle_seed: 0,
            started: false,
            terminal_event_emitted: false,
            combat: None,
            pending_draw_replacement: None,
        };
        Ok(game)
    }

    /// Starts a prepared game at the real first-turn boundary. Deck loading,
    /// shuffling, and opening-hand setup must occur before this call so the
    /// canonical log never claims the turn began before setup completed.
    pub fn begin_game(&mut self) -> Result<(), RulesError> {
        if self.started {
            return Err(RulesError::IllegalAction("the game has already begun"));
        }
        self.require_game_in_progress()?;
        self.started = true;
        self.active_player = PlayerId(0);
        self.priority = PlayerId(0);
        self.step = Step::Untap;
        self.turn = 1;
        self.consecutive_passes = 0;
        self.start_step()?;
        self.validate_invariants()
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
        if !self.players[player.0].mana_pool.can_add(color, amount) {
            return Err(RulesError::IllegalAction(
                "mana pool cannot hold the requested mana",
            ));
        }
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
        if self.started {
            return Err(RulesError::IllegalAction(
                "a deck may be loaded only before the game begins",
            ));
        }
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
        // Resolve every entry before creating an object. A malformed deck is a
        // rejected setup transaction, never a partially loaded library.
        let entries = deck
            .mainboard
            .iter()
            .map(|entry| {
                self.catalog
                    .get(entry.card.as_str())
                    .map(|definition| (definition.id, entry.count))
                    .ok_or(RulesError::UnknownDefinition(
                        "deck card missing from catalog",
                    ))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut cards = 0_u16;
        for (definition, count) in entries {
            for _ in 0..count {
                self.add_card(player, definition, Zone::Library)?;
                cards = cards.saturating_add(1);
            }
        }
        self.record_event(GameEvent::DeckLoaded { player, cards });
        self.shuffle_library(player);
        self.record_event(GameEvent::LibraryShuffled { player, cards });
        self.validate_invariants()
    }

    /// Draws a numbered opening hand from a previously loaded library.
    pub fn draw_opening_hand(&mut self, player: PlayerId, cards: u8) -> Result<(), RulesError> {
        self.player(player)?;
        self.require_game_in_progress()?;
        if self.started {
            return Err(RulesError::IllegalAction(
                "an opening hand may be drawn only before the game begins",
            ));
        }
        if !self.players[player.0].hand.is_empty() {
            return Err(RulesError::IllegalAction(
                "an opening hand requires an empty hand",
            ));
        }
        if self.players[player.0].library.len() < usize::from(cards) {
            return Err(RulesError::IllegalAction(
                "opening hand requires enough cards in library",
            ));
        }
        for _ in 0..cards {
            self.draw_card(player, None)?;
        }
        self.record_event(GameEvent::OpeningHandDrawn { player, cards });
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
        if amount == 0 {
            return Err(RulesError::IllegalAction(
                "a mana action must add positive mana",
            ));
        }
        if !self.players[player.0].mana_pool.can_add(color, amount) {
            return Err(RulesError::IllegalAction(
                "mana pool cannot hold the requested mana",
            ));
        }
        self.players[player.0].mana_pool.add(color, amount);
        self.record_event(GameEvent::ManaAdded {
            player,
            color,
            amount,
        });
        // This public helper stands in for a priority-consuming mana action
        // in compact scenarios. As with an intrinsic mana ability, it breaks
        // a previous consecutive-pass sequence and leaves its controller's
        // response window intact.
        self.consecutive_passes = 0;
        self.validate_invariants()
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
        if !self.players[player.0].mana_pool.can_add(color, 1) {
            return Err(RulesError::IllegalAction(
                "mana pool cannot hold the requested mana",
            ));
        }
        self.objects
            .get_mut(&land)
            .ok_or(RulesError::UnknownCard(land))?
            .tapped = true;
        self.players[player.0].mana_pool.add(color, 1);
        self.record_event(GameEvent::ManaAbilityActivated {
            player,
            land,
            color,
        });
        self.record_event(GameEvent::ManaAdded {
            player,
            color,
            amount: 1,
        });
        self.consecutive_passes = 0;
        self.validate_invariants()
    }

    /// Activates a definition-bound mana ability without using the stack.
    ///
    /// Every condition is preflighted before an object, life total, mana pool,
    /// pass sequence, or event changes. A creature's tap ability observes this
    /// engine slice's summoning-sickness boundary; the current substrate has no
    /// haste exception.
    #[allow(clippy::too_many_lines)] // One method keeps the activation transaction atomic and auditable.
    pub fn activate_bound_mana_ability(
        &mut self,
        player: PlayerId,
        activation: ManaAbilityActivation,
    ) -> Result<(), RulesError> {
        self.require_priority(player)?;
        self.require_zone(activation.source, Zone::Battlefield)?;
        let source = self.object(activation.source)?.clone();
        if source.controller != player {
            return Err(RulesError::IllegalAction(
                "mana ability source must be controlled by its activator",
            ));
        }
        let definition = source.definition.ok_or(RulesError::IllegalAction(
            "a token has no definition-bound mana ability",
        ))?;
        let ability = self
            .mana_abilities
            .get(definition)
            .and_then(|abilities| abilities.get(activation.ability_id))
            .ok_or(RulesError::IllegalAction(
                "source does not have the requested mana ability",
            ))?
            .clone();
        let paid_bundle = match &ability.output {
            ManaAbilityOutput::PaidBundle { mana_cost, bundle } => {
                if activation.chosen_color.is_some() {
                    return Err(RulesError::IllegalAction(
                        "fixed mana bundle ability does not accept a color choice",
                    ));
                }
                let mut paid_pool = self.players[player.0].mana_pool.clone();
                paid_pool.pay(mana_cost).map_err(RulesError::Mana)?;
                for (color, amount) in bundle.iter() {
                    if !paid_pool.can_add(color, amount) {
                        return Err(RulesError::IllegalAction(
                            "mana pool cannot hold the produced mana",
                        ));
                    }
                }
                Some((mana_cost.clone(), bundle.clone(), paid_pool))
            }
            ManaAbilityOutput::Fixed(_) | ManaAbilityOutput::Choice(_) => None,
        };
        let color = if paid_bundle.is_none() {
            Some(Self::resolve_mana_ability_color(
                &ability.output,
                activation.chosen_color,
            )?)
        } else {
            None
        };
        if ability.tap_cost {
            if source.tapped {
                return Err(RulesError::IllegalAction(
                    "mana ability requires an untapped source",
                ));
            }
            if self
                .characteristics(activation.source)?
                .card_types
                .contains(&CardType::Creature)
                && source.entered_turn >= self.turn
            {
                return Err(RulesError::IllegalAction(
                    "a summoning-sick creature cannot pay a tap mana-ability cost",
                ));
            }
        }
        if let Some(life_payment) = ability.life_payment
            && self.players[player.0].life < i16::from(life_payment)
        {
            return Err(RulesError::IllegalAction(
                "cannot pay more life than the controller has",
            ));
        }
        if let Some(color) = color
            && !self.players[player.0]
                .mana_pool
                .can_add(color, ability.amount)
        {
            return Err(RulesError::IllegalAction(
                "mana pool cannot hold the produced mana",
            ));
        }

        if ability.tap_cost {
            self.objects
                .get_mut(&activation.source)
                .ok_or(RulesError::UnknownCard(activation.source))?
                .tapped = true;
        }
        if let Some(life_payment) = ability.life_payment {
            self.players[player.0].life -= i16::from(life_payment);
        }
        if let Some((mana_cost, bundle, paid_pool)) = paid_bundle {
            self.players[player.0].mana_pool = paid_pool;
            self.record_event(GameEvent::BoundManaAbilityBundleActivated {
                player,
                source: activation.source,
                ability: ability.id,
                mana_cost: mana_cost.clone(),
                bundle: bundle.clone(),
                tapped: ability.tap_cost,
                life_payment: ability.life_payment,
            });
            self.record_event(GameEvent::ManaAbilityManaPaid { player, mana_cost });
            if let Some(amount) = ability.life_payment {
                self.record_event(GameEvent::ManaAbilityLifePaid { player, amount });
            }
            for (color, amount) in bundle.iter() {
                self.players[player.0].mana_pool.add(color, amount);
                self.record_event(GameEvent::ManaAdded {
                    player,
                    color,
                    amount,
                });
            }
        } else {
            let color = color.expect("single-color mana ability output was preflighted");
            self.players[player.0].mana_pool.add(color, ability.amount);
            self.record_event(GameEvent::BoundManaAbilityActivated {
                player,
                source: activation.source,
                ability: ability.id,
                color,
                amount: ability.amount,
                tapped: ability.tap_cost,
                life_payment: ability.life_payment,
            });
            if let Some(amount) = ability.life_payment {
                self.record_event(GameEvent::ManaAbilityLifePaid { player, amount });
            }
            self.record_event(GameEvent::ManaAdded {
                player,
                color,
                amount: ability.amount,
            });
        }
        self.consecutive_passes = 0;
        self.priority = player;
        self.check_state_based_actions()?;
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
        self.event_log_integrity.clear();
    }

    /// Appends a canonical event and seals it for the invariant audit. All
    /// engine transitions must use this rather than writing the public vector
    /// directly, so a runner can distinguish an engine-produced receipt from
    /// an external mutation of the replay log.
    fn record_event(&mut self, event: GameEvent) {
        self.event_log_integrity.push(event.clone());
        self.event_log.push(event);
    }

    /// Produces the information a deterministic policy may use to propose one move.
    #[allow(clippy::too_many_lines)] // One projection keeps visibility limits auditable.
    pub fn view_for_player(&self, player: PlayerId) -> Result<GameView, RulesError> {
        let state = self.player(player)?;
        let hand = state
            .hand
            .iter()
            .map(|card| self.card_view(*card))
            .collect::<Result<Vec<_>, _>>()?;
        let mut transmute_searches = Vec::new();
        for card in &state.hand {
            let definition = self.card_definition(*card)?;
            if definition.transmute_cost().is_none() {
                continue;
            }
            let mana_value = definition.mana_cost.mana_value();
            let candidates = state
                .library
                .iter()
                .filter(|candidate| {
                    self.card_definition(**candidate)
                        .is_ok_and(|candidate_definition| {
                            candidate_definition.mana_cost.mana_value() == mana_value
                        })
                })
                .map(|candidate| self.card_view(*candidate))
                .collect::<Result<Vec<_>, _>>()?;
            transmute_searches.push(TransmuteSearchView {
                card: *card,
                candidates,
            });
        }
        let own_battlefield = state
            .battlefield
            .iter()
            .map(|card| self.card_view(*card))
            .collect::<Result<Vec<_>, _>>()?;
        let draw_replacement_pending = self.pending_draw_replacement == Some(player);
        let dredge_candidates = if draw_replacement_pending {
            state
                .graveyard
                .iter()
                .filter(|card| {
                    self.card_definition(**card)
                        .ok()
                        .and_then(CardDefinition::dredge)
                        .is_some_and(|count| state.library.len() >= usize::from(count))
                })
                .map(|card| self.card_view(*card))
                .collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };
        let mut opponent_life = Vec::new();
        let mut opponent_battlefield = Vec::new();
        for opponent in self
            .players
            .iter()
            // A departed seat is not a current opponent. In particular, a
            // policy must not be handed a life-total target that the rules
            // layer will reject as eliminated.
            .filter(|candidate| candidate.id != player && !candidate.lost)
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
        let stack_spells = self
            .stack
            .iter()
            .map(|stack_object| self.card_view(stack_object.card))
            .collect::<Result<Vec<_>, _>>()?;
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
            draw_replacement_pending,
            dredge_candidates,
            transmute_searches,
            own_battlefield,
            opponent_battlefield,
            combat_attackers,
            attackers_declared,
            blockers_declared,
            stack_spells,
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
            PolicyAction::Draw { dredge } => self.resolve_pending_draw(player, dredge)?,
            PolicyAction::Transmute { card, found } => self.transmute(player, card, found)?,
            PolicyAction::PassPriority => self.pass_priority(player)?,
            PolicyAction::PlayLand { card } => self.play_land(player, card)?,
            PolicyAction::ActivateManaAbility { land, color } => {
                self.activate_mana_ability(player, land, color)?;
            }
            PolicyAction::ActivateBoundManaAbility { activation } => {
                self.activate_bound_mana_ability(player, activation)?;
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
        // A submitted pass can drive an automatic draw or combat-damage
        // transition that ends the game. Keep the accepted-action receipt in
        // causal order and make `GameEnded` the terminal event in the trace.
        let terminal_event = if self.terminal_event_emitted {
            match (self.event_log.pop(), self.event_log_integrity.pop()) {
                (
                    Some(GameEvent::GameEnded { winner }),
                    Some(GameEvent::GameEnded {
                        winner: sealed_winner,
                    }),
                ) if winner == sealed_winner => Some(winner),
                (event, sealed_event) => {
                    if let Some(event) = event {
                        self.event_log.push(event);
                    }
                    if let Some(event) = sealed_event {
                        self.event_log_integrity.push(event);
                    }
                    None
                }
            }
        } else {
            None
        };
        self.record_event(GameEvent::PolicyMoveSubmitted {
            player,
            policy: policy.into(),
            kind,
        });
        if let Some(winner) = terminal_event {
            self.record_event(GameEvent::GameEnded { winner });
        }
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
        self.require_game_in_progress()?;
        self.install_continuous_effect(source, target, change, duration)?;
        // A public installation is a completed state-changing transition, so
        // its new characteristics must reach the SBA fixed point before a
        // caller receives control again.
        self.check_state_based_actions()?;
        self.validate_invariants()
    }

    /// Installs an effect while a spell is resolving. The resolver performs
    /// state-based actions only after the whole spell has resolved, so this
    /// internal primitive intentionally omits the public transition boundary.
    fn install_continuous_effect(
        &mut self,
        source: ObjectId,
        target: ObjectId,
        change: ContinuousChange,
        duration: Duration,
    ) -> Result<(), RulesError> {
        self.object(source)?;
        self.object(target)?;
        match duration {
            Duration::Permanent
                if self.zone_of(source) != Some(Zone::Battlefield)
                    || self.zone_of(target) != Some(Zone::Battlefield) =>
            {
                return Err(RulesError::IllegalAction(
                    "a permanent continuous effect requires battlefield source and target",
                ));
            }
            Duration::EndOfTurn(turn) if turn != self.turn => {
                return Err(RulesError::IllegalAction(
                    "an end-of-turn effect must expire this turn",
                ));
            }
            Duration::EndOfTurn(_) if self.zone_of(target) != Some(Zone::Battlefield) => {
                return Err(RulesError::IllegalAction(
                    "an end-of-turn effect requires a battlefield target",
                ));
            }
            _ => {}
        }
        let layer = change.layer();
        self.continuous_effects.push(ContinuousEffect {
            source,
            target,
            change,
            duration,
            timestamp: self.next_timestamp,
        });
        self.next_timestamp += 1;
        self.record_event(GameEvent::ContinuousEffectCreated {
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
        self.require_game_in_progress()?;
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
        let defending_player = self.next_player(player);
        let combat = self
            .combat
            .as_mut()
            .ok_or(RulesError::IllegalAction("combat was not initialized"))?;
        combat.attackers = attackers.to_vec();
        combat.defending_player = Some(defending_player);
        combat.attackers_declared = true;
        self.record_event(GameEvent::AttackersDeclared {
            player,
            attackers: attackers.to_vec(),
        });
        self.consecutive_passes = 0;
        // CR 508.2: the active player receives priority after attackers are
        // declared. Declaration itself is a turn-based action, not a normal
        // priority action, so a stale pre-declaration holder cannot block it.
        self.priority = self.active_player;
        self.validate_invariants()
    }

    /// Performs the turn-based action of assigning zero or one blocker to each
    /// attacker in the initial combat slice.
    pub fn declare_blockers(
        &mut self,
        player: PlayerId,
        assignments: &[CombatBlock],
    ) -> Result<(), RulesError> {
        self.require_game_in_progress()?;
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
        if combat.defending_player != Some(player) {
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
        self.record_event(GameEvent::BlockersDeclared {
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
        self.record_event(GameEvent::EngineWeaknessRevealed {
            player,
            code: code.into(),
            detail: detail.into(),
        });
        // A report is observational, but it is still an accepted non-pass
        // policy action made with priority. It must reopen the response cycle
        // rather than letting an earlier opponent pass advance the step.
        self.consecutive_passes = 0;
        self.validate_invariants()
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
        if !definition.card_types.contains(&CardType::Instant)
            && (player != self.active_player || !self.step.is_main() || !self.stack.is_empty())
        {
            return Err(RulesError::IllegalAction(
                "non-instant spells require your main phase with an empty stack",
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
            self.record_event(GameEvent::ConvokeUsed {
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
        self.record_event(GameEvent::SpellCast {
            player,
            card: request.card,
        });
        self.consecutive_passes = 0;
        // CR 601.2i / 117.3c: after completing a cast, the acting player
        // receives priority again. Opponents get their response window only
        // after that player passes.
        self.priority = player;
        Ok(())
    }

    pub fn pass_priority(&mut self, player: PlayerId) -> Result<(), RulesError> {
        self.require_priority(player)?;
        if self.pending_draw_replacement.is_some() {
            return Err(RulesError::IllegalAction(
                "the draw replacement decision must resolve before priority can pass",
            ));
        }
        if self.step == Step::DeclareAttackers
            && self
                .combat
                .as_ref()
                .is_some_and(|combat| !combat.attackers_declared)
        {
            return Err(RulesError::IllegalAction(
                "the active player must declare attackers before priority can pass",
            ));
        }
        if self.step == Step::DeclareBlockers
            && self
                .combat
                .as_ref()
                .is_some_and(|combat| !combat.blockers_declared)
        {
            return Err(RulesError::IllegalAction(
                "the defending player must declare blockers before priority can pass",
            ));
        }
        self.record_event(GameEvent::PriorityPassed { player });
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
        // Opening-hand setup deliberately uses this primitive before the game
        // begins. Once a game is live, an ordinary draw can happen only at the
        // active player's pending Draw-step replacement boundary. This keeps a
        // public helper from becoming an arbitrary-card-to-hand action.
        // A fixture may deliberately drive the public turn machine before
        // `begin_game`; if that machine creates a Draw marker, resolving it
        // still has to consume it. `started` controls only whether arbitrary
        // direct draws are prohibited, not marker cleanup.
        let resolves_pending_draw = self.pending_draw_replacement == Some(player);
        if self.started
            && (self.step != Step::Draw || player != self.active_player || !resolves_pending_draw)
        {
            return Err(RulesError::IllegalAction(
                "a live draw requires the active player's pending draw-step decision",
            ));
        }
        if let Some(card) = dredge {
            // Public scenarios may model a single replacement draw before
            // `begin_game` without constructing a full turn. A live game,
            // above, must already have created this marker at the real Draw
            // step and may not synthesize one arbitrarily.
            if !self.started {
                self.pending_draw_replacement = Some(player);
            }
            let result = self.dredge(player, card);
            self.pending_draw_replacement = None;
            return result;
        }
        let Some(card) = self.players[player.0].library.pop() else {
            self.lose_player(player, "attempted to draw from an empty library");
            self.normalize_priority_after_elimination()?;
            self.record_game_end_if_needed();
            if resolves_pending_draw {
                self.pending_draw_replacement = None;
            }
            return Ok(());
        };
        self.players[player.0].hand.push(card);
        self.record_event(GameEvent::CardMoved {
            card,
            to: Zone::Hand,
        });
        if resolves_pending_draw {
            self.pending_draw_replacement = None;
        }
        Ok(())
    }

    /// Resolves the draw replacement decision exposed during a normal draw step.
    /// `None` takes the ordinary draw; a card selects that card's dredge ability.
    pub fn resolve_pending_draw(
        &mut self,
        player: PlayerId,
        dredge: Option<ObjectId>,
    ) -> Result<(), RulesError> {
        self.require_game_in_progress()?;
        if self.pending_draw_replacement != Some(player)
            || self.step != Step::Draw
            || player != self.active_player
        {
            return Err(RulesError::IllegalAction(
                "this player has no pending draw replacement decision",
            ));
        }
        if let Some(card) = dredge {
            self.dredge(player, card)?;
            self.pending_draw_replacement = None;
        } else {
            self.draw_card(player, None)?;
        }
        self.consecutive_passes = 0;
        self.validate_invariants()
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
            self.record_event(GameEvent::CardMoved {
                card: milled,
                to: Zone::Graveyard,
            });
        }
        self.remove_from_all_zones(card);
        self.players[player.0].hand.push(card);
        self.record_event(GameEvent::CardMoved {
            card,
            to: Zone::Hand,
        });
        self.record_event(GameEvent::Dredged {
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
        let cards = u16::try_from(self.players[player.0].library.len()).unwrap_or(u16::MAX);
        self.record_event(GameEvent::LibraryShuffled { player, cards });
        self.record_event(GameEvent::Transmuted {
            player,
            discarded: card,
            found,
        });
        self.consecutive_passes = 0;
        // The supported atomic transmute activation completes at the same
        // priority boundary as any other non-pass action: its controller
        // retains priority until they choose to pass.
        self.priority = player;
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
                    self.record_event(GameEvent::StateBasedAction { card, reason });
                    self.move_to_graveyard_or_remove_token(card)?;
                    changed = true;
                }
            }
            if !changed {
                self.normalize_priority_after_elimination()?;
                self.record_game_end_if_needed();
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
        if remaining.len() == 1 {
            Some(remaining[0])
        } else {
            None
        }
    }

    /// Returns the player who must provide the next policy decision. During
    /// declaration steps and draw replacements this can differ from the player
    /// who would otherwise receive priority.
    #[must_use]
    pub fn next_policy_player(&self) -> PlayerId {
        self.policy_decision_player()
    }

    /// Validates the non-negotiable internal facts relied on by all rule methods.
    /// It is public so development runners and scenario tests can fail at the first
    /// corrupted state rather than report a misleading later rules error.
    #[allow(clippy::too_many_lines)] // One ordered audit keeps the invariant contract reviewable.
    pub fn validate_invariants(&self) -> Result<(), RulesError> {
        if self.players.len() != self.seated_player_count
            || self.players.len() < 2
            || self.players.get(self.active_player.0).is_none()
            || self.players.get(self.priority.0).is_none()
            || self.turn == 0
        {
            return Err(RulesError::IllegalAction("invalid seated-player state"));
        }
        if !self.is_game_over() && self.players[self.priority.0].lost {
            return Err(RulesError::IllegalAction(
                "a continuing game assigned priority to an eliminated player",
            ));
        }
        if !self.is_game_over() && self.players[self.active_player.0].lost {
            return Err(RulesError::IllegalAction(
                "a continuing game assigned the active turn to an eliminated player",
            ));
        }
        if self.event_log != self.event_log_integrity {
            return Err(RulesError::IllegalAction(
                "canonical event log was mutated outside an engine transition",
            ));
        }
        if self.started && !self.is_game_over() && !self.step.grants_priority() {
            return Err(RulesError::IllegalAction(
                "an automatic turn step remained stable with player priority",
            ));
        }
        if self.terminal_event_emitted != self.is_game_over() {
            return Err(RulesError::IllegalAction(
                "terminal game state and GameEnded lifecycle record disagree",
            ));
        }
        let terminal_event_count = self
            .event_log
            .iter()
            .filter(|event| matches!(event, GameEvent::GameEnded { .. }))
            .count();
        if self.terminal_event_emitted {
            if terminal_event_count != 1
                || !matches!(
                    self.event_log.last(),
                    Some(GameEvent::GameEnded { winner }) if *winner == self.winner()
                )
            {
                return Err(RulesError::IllegalAction(
                    "terminal game must end with exactly one matching GameEnded event",
                ));
            }
        } else if terminal_event_count != 0 {
            return Err(RulesError::IllegalAction(
                "a continuing game has a spurious GameEnded event",
            ));
        }
        if !self.is_game_over() && self.consecutive_passes >= self.remaining_player_count() {
            return Err(RulesError::IllegalAction(
                "pass sequence was not reset after every surviving player passed",
            ));
        }
        if let Some(player) = self.pending_draw_replacement
            && (self.step != Step::Draw
                || player != self.active_player
                || player != self.priority
                || self.consecutive_passes != 0
                || self.players[player.0].lost)
        {
            return Err(RulesError::IllegalAction(
                "draw-replacement marker escaped its draw-step decision boundary",
            ));
        }
        if self.next_object_id == 0 || self.next_timestamp == 0 {
            return Err(RulesError::IllegalAction(
                "object or effect identifier counter wrapped to zero",
            ));
        }
        for definition in self.catalog.values() {
            if definition.id.is_empty()
                || definition.name.is_empty()
                || definition.set_code.is_empty()
                || definition.card_types.is_empty()
            {
                return Err(RulesError::IllegalAction(
                    "card definition lacks a required identity or type",
                ));
            }
            if definition.is_basic_land && !definition.is_land() {
                return Err(RulesError::IllegalAction(
                    "a basic-land definition is missing the land type",
                ));
            }
            if definition.is_basic_land && definition.mana_cost.mana_value() != 0 {
                return Err(RulesError::IllegalAction(
                    "a basic land definition has a mana cost",
                ));
            }
            if !definition.is_land() && !definition.mana_colors.is_empty() {
                return Err(RulesError::IllegalAction(
                    "only land definitions may expose an intrinsic mana ability",
                ));
            }
            if definition.is_creature()
                != (definition.power.is_some() && definition.toughness.is_some())
            {
                return Err(RulesError::IllegalAction(
                    "creature definitions require both power and toughness",
                ));
            }
            if definition.dredge() == Some(0) {
                return Err(RulesError::IllegalAction("dredge amount must be positive"));
            }
            let mut supported_rules = BTreeSet::new();
            if definition.supported_rules.is_empty()
                || definition
                    .supported_rules
                    .iter()
                    .any(|rule| rule.is_empty() || !supported_rules.insert(*rule))
            {
                return Err(RulesError::IllegalAction(
                    "card definition has missing or duplicate supported-rule markers",
                ));
            }
        }
        for (definition_id, abilities) in &self.mana_abilities {
            let definition = self
                .catalog
                .get(definition_id)
                .ok_or(RulesError::UnknownDefinition(definition_id))?;
            if !definition.is_permanent() {
                return Err(RulesError::IllegalAction(
                    "a definition-bound mana ability requires a permanent source",
                ));
            }
            for (ability_id, ability) in abilities {
                if *ability_id != ability.id {
                    return Err(RulesError::IllegalAction(
                        "mana ability catalog key does not match its ability identity",
                    ));
                }
                Self::validate_mana_ability_definition(ability)?;
            }
        }
        let mut locations = BTreeMap::<ObjectId, Zone>::new();
        for (seat, player) in self.players.iter().enumerate() {
            if player.id != PlayerId(seat) {
                return Err(RulesError::IllegalAction("player id does not match seat"));
            }
            if player.lands_played > 1 {
                return Err(RulesError::IllegalAction(
                    "player exceeded the one-land-per-turn rules slice",
                ));
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
                    if object.entered_turn > self.turn || object.damage < 0 {
                        return Err(RulesError::IllegalAction(
                            "object has impossible turn metadata or negative damage",
                        ));
                    }
                    match (object.definition, object.token.as_ref()) {
                        (Some(definition), None) => {
                            let definition = self
                                .catalog
                                .get(definition)
                                .ok_or(RulesError::UnknownDefinition(definition))?;
                            if zone == Zone::Battlefield && !definition.is_permanent() {
                                return Err(RulesError::IllegalAction(
                                    "a nonpermanent card occupies the battlefield",
                                ));
                            }
                        }
                        (None, Some(_)) if zone == Zone::Battlefield => {}
                        (None, Some(_)) => {
                            return Err(RulesError::IllegalAction(
                                "a token exists outside the battlefield",
                            ));
                        }
                        (Some(_), Some(_)) => {
                            return Err(RulesError::IllegalAction(
                                "an object cannot be both a card and token",
                            ));
                        }
                        (None, None) => {
                            return Err(RulesError::IllegalAction(
                                "an object has neither card definition nor token characteristics",
                            ));
                        }
                    }
                    if zone != Zone::Battlefield && object.controller != object.owner {
                        return Err(RulesError::IllegalAction(
                            "a nonbattlefield card retained a non-owner controller",
                        ));
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
            let object = self.object(stack_object.card)?;
            self.player(stack_object.controller)?;
            if object.controller != stack_object.controller {
                return Err(RulesError::IllegalAction(
                    "stack controller does not match its card object",
                ));
            }
            let definition = object
                .definition
                .and_then(|definition| self.catalog.get(definition))
                .ok_or(RulesError::IllegalAction(
                    "a stack object must be a non-token card with a catalog definition",
                ))?;
            if object.token.is_some() || definition.is_land() {
                return Err(RulesError::IllegalAction(
                    "a token or land occupies the stack",
                ));
            }
            if definition.effects != stack_object.effects {
                return Err(RulesError::IllegalAction(
                    "stack effects do not match the card definition",
                ));
            }
            let target_count = usize::from(
                definition
                    .effects
                    .iter()
                    .any(|effect| effect.target_requirement().is_some()),
            );
            if stack_object.targets.len() != target_count {
                return Err(RulesError::IllegalAction(
                    "stack object has an invalid target count",
                ));
            }
            // A target may become illegal after a legal cast (for example, a
            // player can lose or a permanent can leave the battlefield), but
            // a player-seat identity can never cease to exist. Reject an
            // unseated player injected into a public stack object instead of
            // mistaking it for a rules-counterable target.
            for target in &stack_object.targets {
                if let Target::Player(player) = target {
                    self.player(*player)?;
                }
            }
        }
        for card in self.objects.keys() {
            if !locations.contains_key(card) && !stack_cards.contains(card) {
                return Err(RulesError::IllegalAction("object has no game location"));
            }
        }
        if self
            .objects
            .keys()
            .map(|card| card.0)
            .max()
            .is_some_and(|highest| highest >= self.next_object_id)
        {
            return Err(RulesError::IllegalAction(
                "next object identifier would reuse an existing object",
            ));
        }
        let mut effect_timestamps = BTreeSet::new();
        for effect in &self.continuous_effects {
            self.object(effect.source)?;
            self.object(effect.target)?;
            if effect.timestamp == 0
                || !effect_timestamps.insert(effect.timestamp)
                || effect.timestamp >= self.next_timestamp
            {
                return Err(RulesError::IllegalAction(
                    "continuous-effect timestamps are not unique and monotonic",
                ));
            }
            match effect.duration {
                Duration::EndOfTurn(turn)
                    if turn == self.turn
                        && self.zone_of(effect.target) == Some(Zone::Battlefield) => {}
                Duration::EndOfTurn(_) => {
                    return Err(RulesError::IllegalAction(
                        "end-of-turn effect has invalid duration or target zone",
                    ));
                }
                Duration::Permanent if self.zone_of(effect.source) != Some(Zone::Battlefield) => {
                    return Err(RulesError::IllegalAction(
                        "permanent continuous effect outlived its battlefield source",
                    ));
                }
                Duration::Permanent => {}
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
            if self.step == Step::DeclareBlockers && !combat.attackers_declared {
                return Err(RulesError::IllegalAction(
                    "blocker declaration began before attackers were declared",
                ));
            }
            if self.step == Step::CombatDamage
                && (!combat.attackers_declared || !combat.blockers_declared)
            {
                return Err(RulesError::IllegalAction(
                    "combat damage began without both combat declarations",
                ));
            }
            if combat.attackers_declared {
                let defending_player = combat.defending_player.ok_or(RulesError::IllegalAction(
                    "declared combat is missing its defending player",
                ))?;
                self.player(defending_player)?;
                if defending_player == self.active_player {
                    return Err(RulesError::IllegalAction(
                        "combat defender cannot equal the active player",
                    ));
                }
            } else if combat.defending_player.is_some() {
                return Err(RulesError::IllegalAction(
                    "undeclared combat has a defending player",
                ));
            }
            for attacker in &combat.attackers {
                if !attackers.insert(*attacker) {
                    return Err(RulesError::IllegalAction("invalid combat attacker state"));
                }
                if self.zone_of(*attacker).is_some() {
                    self.object(*attacker)?;
                }
            }
            for (attacker, blocker) in &combat.blockers {
                if !attackers.contains(attacker) || !blockers.insert(*blocker) {
                    return Err(RulesError::IllegalAction("invalid combat blocker state"));
                }
                if self.zone_of(*blocker).is_some() {
                    self.object(*blocker)?;
                }
            }
        } else if matches!(
            self.step,
            Step::DeclareAttackers | Step::DeclareBlockers | Step::CombatDamage
        ) {
            return Err(RulesError::IllegalAction(
                "a combat step is missing its combat state",
            ));
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
            self.record_event(GameEvent::SpellCounteredByRules {
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
        self.record_event(GameEvent::SpellResolved {
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
                    self.record_event(GameEvent::DamageDealtToPlayer {
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
                    self.record_event(GameEvent::DamageDealtToPermanent {
                        source,
                        permanent: *permanent,
                        amount: *amount,
                    });
                }
                Target::Spell(card) => return Err(RulesError::IllegalTarget(Target::Spell(*card))),
            },
            Effect::DealDamageController { amount } => {
                self.players[controller.0].life -= amount;
                self.record_event(GameEvent::DamageDealtToPlayer {
                    source,
                    player: controller,
                    amount: *amount,
                });
            }
            Effect::DealDamageToEachCreatureAndPlayer { amount } => {
                // Snapshot the complete affected set before mutating the
                // board. The resolution path invokes state-based actions only
                // after every effect finishes, so simultaneous all-creature
                // damage cannot remove a later recipient early.
                let creatures = self
                    .objects
                    .keys()
                    .copied()
                    .filter(|candidate| {
                        self.zone_of(*candidate) == Some(Zone::Battlefield)
                            && self
                                .characteristics(*candidate)
                                .is_ok_and(|characteristics| {
                                    characteristics.card_types.contains(&CardType::Creature)
                                })
                    })
                    .collect::<Vec<_>>();
                for creature in creatures {
                    self.objects
                        .get_mut(&creature)
                        .ok_or(RulesError::UnknownCard(creature))?
                        .damage += amount;
                    self.record_event(GameEvent::DamageDealtToPermanent {
                        source,
                        permanent: creature,
                        amount: *amount,
                    });
                }
                for player in 0..self.players.len() {
                    if self.players[player].lost {
                        continue;
                    }
                    let player = PlayerId(player);
                    self.players[player.0].life -= amount;
                    self.record_event(GameEvent::DamageDealtToPlayer {
                        source,
                        player,
                        amount: *amount,
                    });
                }
            }
            Effect::RadianceDealDamageToCreatures { amount } => {
                let target = Self::target_permanent(targets)?;
                // Select once before mutating damage. State-based actions run
                // after the complete spell resolves, so every selected
                // creature receives this effect's damage in the same batch.
                for candidate in self.radiance_creatures_sharing_color(target)? {
                    self.objects
                        .get_mut(&candidate)
                        .ok_or(RulesError::UnknownCard(candidate))?
                        .damage += amount;
                    self.record_event(GameEvent::DamageDealtToPermanent {
                        source,
                        permanent: candidate,
                        amount: *amount,
                    });
                }
            }
            Effect::GainLifeController { amount } => {
                self.players[controller.0].life += amount;
                self.record_event(GameEvent::LifeGained {
                    player: controller,
                    amount: *amount,
                });
            }
            Effect::CreateToken { token, count } => {
                for _ in 0..*count {
                    let token_id = self.create_token(controller, token.clone())?;
                    self.record_event(GameEvent::TokenCreated {
                        player: controller,
                        token: token_id,
                    });
                }
            }
            Effect::ModifyTargetPtUntilEndOfTurn { power, toughness } => {
                let target = Self::target_permanent(targets)?;
                self.install_continuous_effect(
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
                let matching = self.radiance_creatures_sharing_color(target)?;
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
                    self.install_continuous_effect(
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
                    self.record_event(GameEvent::PermanentsUntapped {
                        player: controller,
                        cards: untapped,
                    });
                }
            }
            Effect::RadianceModifyPtUntilEndOfTurn { power, toughness } => {
                let target = Self::target_permanent(targets)?;
                for candidate in self.radiance_creatures_sharing_color(target)? {
                    self.install_continuous_effect(
                        source,
                        candidate,
                        ContinuousChange::ModifyPowerToughness {
                            power: *power,
                            toughness: *toughness,
                        },
                        Duration::EndOfTurn(self.turn),
                    )?;
                }
            }
            Effect::CounterTargetInstantOrSorcerySpell => {
                let target = Self::target_spell(targets)?;
                let position = self
                    .stack
                    .iter()
                    .position(|stack_object| stack_object.card == target)
                    .ok_or(RulesError::IllegalTarget(Target::Spell(target)))?;
                self.stack.remove(position);
                self.record_event(GameEvent::SpellCountered {
                    card: target,
                    source,
                });
                self.move_to_graveyard_or_remove_token(target)?;
            }
        }
        Ok(())
    }

    fn target_permanent(targets: &[Target]) -> Result<ObjectId, RulesError> {
        match targets.first() {
            Some(Target::Permanent(card)) => Ok(*card),
            Some(Target::Player(player)) => Err(RulesError::IllegalTarget(Target::Player(*player))),
            Some(Target::Spell(card)) => Err(RulesError::IllegalTarget(Target::Spell(*card))),
            None => Err(RulesError::IllegalAction("missing permanent target")),
        }
    }

    fn target_spell(targets: &[Target]) -> Result<ObjectId, RulesError> {
        match targets.first() {
            Some(Target::Spell(card)) => Ok(*card),
            Some(Target::Player(player)) => Err(RulesError::IllegalTarget(Target::Player(*player))),
            Some(Target::Permanent(card)) => {
                Err(RulesError::IllegalTarget(Target::Permanent(*card)))
            }
            None => Err(RulesError::IllegalAction("missing spell target")),
        }
    }

    fn target_matches(&self, target: Target, requirement: TargetRequirement) -> bool {
        match (target, requirement) {
            (
                Target::Player(player),
                TargetRequirement::Any
                | TargetRequirement::Player
                | TargetRequirement::PlayerOrCreature,
            ) => self.players.get(player.0).is_some_and(|state| !state.lost),
            (Target::Permanent(card), TargetRequirement::Any) => {
                self.zone_of(card) == Some(Zone::Battlefield)
            }
            (
                Target::Permanent(card),
                TargetRequirement::Creature | TargetRequirement::PlayerOrCreature,
            ) => {
                self.zone_of(card) == Some(Zone::Battlefield)
                    && self.characteristics(card).is_ok_and(|characteristics| {
                        characteristics.card_types.contains(&CardType::Creature)
                    })
            }
            (Target::Spell(card), TargetRequirement::InstantOrSorcerySpell) => {
                self.stack
                    .iter()
                    .any(|stack_object| stack_object.card == card)
                    && self.card_definition(card).is_ok_and(|definition| {
                        definition.card_types.contains(&CardType::Instant)
                            || definition.card_types.contains(&CardType::Sorcery)
                    })
            }
            _ => false,
        }
    }

    fn advance_step(&mut self) -> Result<(), RulesError> {
        for player in &mut self.players {
            player.mana_pool.clear();
        }
        let skip_combat = matches!(self.step, Step::DeclareAttackers | Step::DeclareBlockers)
            && self.combat.as_ref().is_some_and(|combat| {
                combat.attackers_declared
                    && (combat.attackers.is_empty()
                        || combat
                            .defending_player
                            .is_some_and(|player| self.players[player.0].lost))
            });
        // CR 508.8: no attackers means there is no declare-blockers or
        // combat-damage step. Likewise, when the fixed defending player
        // leaves, attackers are removed from combat instead of retargeting a
        // later seat. The combat state is cleared at EndOfCombat.
        self.step = if skip_combat {
            Step::EndOfCombat
        } else {
            self.step.next()
        };
        if self.step == Step::Untap {
            self.active_player = self.next_player(self.active_player);
            self.turn += 1;
        }
        self.priority = self.priority_after_resolution();
        self.start_step()
    }

    fn start_step(&mut self) -> Result<(), RulesError> {
        // The boundary marker must precede *all* turn-based work in the step
        // so an event log can be replayed in chronological order.
        self.record_event(GameEvent::StepBegan {
            turn: self.turn,
            active_player: self.active_player,
            step: self.step,
        });
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
                    self.record_event(GameEvent::PermanentsUntapped {
                        player: self.active_player,
                        cards: untapped,
                    });
                }
            }
            Step::Draw => {
                // CR 103.8a/103.8c: only a two-player game's starting player
                // skips its first draw. Multiplayer games do not skip it.
                if self.players.len() != 2 || self.turn != 1 || self.active_player != PlayerId(0) {
                    self.pending_draw_replacement = Some(self.active_player);
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
                let expired = self
                    .continuous_effects
                    .iter()
                    .filter(|effect| effect.duration == Duration::EndOfTurn(self.turn))
                    .cloned()
                    .collect::<Vec<_>>();
                self.continuous_effects
                    .retain(|effect| effect.duration != Duration::EndOfTurn(self.turn));
                for effect in expired {
                    self.record_event(GameEvent::ContinuousEffectExpired {
                        source: effect.source,
                        target: effect.target,
                        layer: effect.change.layer(),
                    });
                }
                self.check_state_based_actions()?;
            }
            _ => {}
        }
        if (self.step == Step::Untap || self.step == Step::Cleanup) && !self.is_game_over() {
            // CR 117.3a and 514.3: neither normal Untap nor this slice's
            // ordinary Cleanup gives priority. Advance immediately.
            self.advance_step()?;
        }
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
        let defending_player = combat.defending_player.ok_or(RulesError::IllegalAction(
            "combat damage is missing its defending player",
        ))?;
        if self.players[defending_player.0].lost {
            return Ok(());
        }
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
                    // A creature that was blocked remains blocked even if its
                    // blocker leaves combat before damage. This slice has no
                    // trample, so it deals no combat damage to the defending
                    // player in that case.
                    continue;
                }
                let blocker_power = self
                    .characteristics(blocker)?
                    .power
                    .ok_or(RulesError::IllegalAction("blocker lacks power"))?;
                if attacker_power > 0 {
                    permanent_damage.push((attacker, blocker, attacker_power));
                }
                if blocker_power > 0 {
                    permanent_damage.push((blocker, attacker, blocker_power));
                }
            } else if attacker_power > 0 {
                player_damage.push((attacker, defending_player, attacker_power));
            }
        }
        for (source, permanent, amount) in permanent_damage {
            self.objects
                .get_mut(&permanent)
                .ok_or(RulesError::UnknownCard(permanent))?
                .damage += amount;
            self.record_event(GameEvent::DamageDealtToPermanent {
                source,
                permanent,
                amount,
            });
        }
        for (source, player, amount) in player_damage {
            self.players[player.0].life -= amount;
            self.record_event(GameEvent::DamageDealtToPlayer {
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
            self.record_event(GameEvent::TokenCeasedToExist { token: card });
            self.expire_continuous_effects_involving(card);
            return Ok(());
        }
        self.move_to_zone(card, Zone::Graveyard)
    }

    fn move_to_zone(&mut self, card: ObjectId, zone: Zone) -> Result<(), RulesError> {
        let object = self.object(card)?.clone();
        let left_battlefield =
            self.zone_of(card) == Some(Zone::Battlefield) && zone != Zone::Battlefield;
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
        self.record_event(GameEvent::CardMoved { card, to: zone });
        if left_battlefield {
            // Permanent effects cease when either their source or target
            // changes zones. End-of-turn effects from instants remain because
            // their source was never a battlefield permanent.
            self.expire_continuous_effects_involving(card);
        }
        Ok(())
    }

    fn expire_continuous_effects_involving(&mut self, card: ObjectId) {
        let expired = self
            .continuous_effects
            .iter()
            .filter(|effect| effect.source == card || effect.target == card)
            .cloned()
            .collect::<Vec<_>>();
        self.continuous_effects
            .retain(|effect| effect.source != card && effect.target != card);
        for effect in expired {
            self.record_event(GameEvent::ContinuousEffectExpired {
                source: effect.source,
                target: effect.target,
                layer: effect.change.layer(),
            });
        }
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

    fn validate_mana_ability_definition(ability: &ActivatedManaAbility) -> Result<(), RulesError> {
        if ability.id.is_empty() {
            return Err(RulesError::IllegalAction("mana ability lacks an identity"));
        }
        if matches!(
            &ability.output,
            ManaAbilityOutput::Fixed(_) | ManaAbilityOutput::Choice(_)
        ) && ability.amount == 0
        {
            return Err(RulesError::IllegalAction(
                "mana ability must produce positive mana",
            ));
        }
        if let ManaAbilityOutput::PaidBundle { mana_cost, bundle } = &ability.output {
            if ability.amount != 0 {
                return Err(RulesError::IllegalAction(
                    "paid mana bundle ability must use bundle quantities instead of amount",
                ));
            }
            if mana_cost.mana_value() == 0 {
                return Err(RulesError::IllegalAction(
                    "paid mana bundle ability requires a positive mana cost",
                ));
            }
            if bundle.is_empty() || bundle.iter().any(|(_, amount)| amount == 0) {
                return Err(RulesError::IllegalAction(
                    "mana ability bundle must contain positive mana amounts",
                ));
            }
        }
        if ability.life_payment == Some(0) {
            return Err(RulesError::IllegalAction(
                "mana ability life payment must be positive when present",
            ));
        }
        if matches!(&ability.output, ManaAbilityOutput::Choice(colors) if colors.is_empty()) {
            return Err(RulesError::IllegalAction(
                "mana ability color choice must not be empty",
            ));
        }
        Ok(())
    }

    fn resolve_mana_ability_color(
        output: &ManaAbilityOutput,
        chosen_color: Option<Color>,
    ) -> Result<Color, RulesError> {
        match output {
            ManaAbilityOutput::Fixed(color) if chosen_color.is_none() => Ok(*color),
            ManaAbilityOutput::Fixed(_) => Err(RulesError::IllegalAction(
                "fixed-color mana ability does not accept a color choice",
            )),
            ManaAbilityOutput::Choice(colors) => {
                chosen_color.filter(|color| colors.contains(color)).ok_or(
                    RulesError::IllegalAction("mana ability requires one supported color choice"),
                )
            }
            ManaAbilityOutput::PaidBundle { .. } => Err(RulesError::IllegalAction(
                "fixed mana bundle ability does not resolve to one color",
            )),
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
        if !self.step.grants_priority() {
            return Err(RulesError::IllegalAction(
                "no player receives priority during this automatic step",
            ));
        }
        // CR 508.1 and 509.1 make combat declaration turn-based actions, not
        // ordinary priority windows. `priority` names the player who will act
        // after the declaration; it must not authorize a spell, mana ability,
        // pass, or weakness report before that declaration happens.
        if self.step == Step::DeclareAttackers
            && self
                .combat
                .as_ref()
                .is_some_and(|combat| !combat.attackers_declared)
        {
            return Err(RulesError::IllegalAction(
                "attackers must be declared before priority actions",
            ));
        }
        if self.step == Step::DeclareBlockers
            && self
                .combat
                .as_ref()
                .is_some_and(|combat| !combat.blockers_declared)
        {
            return Err(RulesError::IllegalAction(
                "blockers must be declared before priority actions",
            ));
        }
        // A pending draw replacement is a mandatory turn-based choice, not a
        // priority window (CR 616.1 / 121.6).  In particular, an instant,
        // mana ability, or capability report must not be able to interleave
        // with the choice and leave the marker pointing at a stale holder.
        if self.pending_draw_replacement.is_some() {
            return Err(RulesError::IllegalAction(
                "the draw replacement decision must resolve before priority actions",
            ));
        }
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

    fn normalize_priority_after_elimination(&mut self) -> Result<(), RulesError> {
        if self.is_game_over() {
            return Ok(());
        }
        if self.players[self.active_player.0].lost {
            // A departed active player cannot complete a declaration, receive
            // priority, or resume their turn. Abort the remaining turn and
            // begin the next survivor's turn at its automatic Untap boundary.
            self.active_player = self.next_player(self.active_player);
            self.priority = self.active_player;
            self.step = Step::Untap;
            self.turn += 1;
            self.combat = None;
            self.consecutive_passes = 0;
            return self.start_step();
        }
        if self.players[self.priority.0].lost {
            self.priority = self.next_player(self.priority);
            self.consecutive_passes = 0;
        }
        Ok(())
    }

    fn require_game_in_progress(&self) -> Result<(), RulesError> {
        if self.is_game_over() {
            Err(RulesError::IllegalAction("the game has already ended"))
        } else {
            Ok(())
        }
    }

    fn policy_decision_player(&self) -> PlayerId {
        if let Some(player) = self.pending_draw_replacement {
            return player;
        }
        match (&self.combat, self.step) {
            (Some(combat), Step::DeclareAttackers)
                if !combat.attackers_declared && !self.players[self.active_player.0].lost =>
            {
                self.active_player
            }
            (Some(combat), Step::DeclareBlockers) if !combat.blockers_declared => combat
                .defending_player
                .filter(|player| !self.players[player.0].lost)
                .unwrap_or(self.priority),
            _ => self.priority,
        }
    }

    fn all_battlefield_cards(&self) -> Vec<ObjectId> {
        self.players
            .iter()
            .flat_map(|player| player.battlefield.iter().copied())
            .collect()
    }

    /// Returns the radiance set at resolution: every battlefield creature that
    /// shares at least one color with the already-legal creature target.
    fn radiance_creatures_sharing_color(
        &self,
        target: ObjectId,
    ) -> Result<Vec<ObjectId>, RulesError> {
        let target_colors = self.characteristics(target)?.colors;
        Ok(self
            .all_battlefield_cards()
            .into_iter()
            .filter(|candidate| {
                *candidate == target
                    || self
                        .characteristics(*candidate)
                        .is_ok_and(|characteristics| {
                            characteristics.card_types.contains(&CardType::Creature)
                                && !characteristics.colors.is_disjoint(&target_colors)
                        })
            })
            .collect())
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
            self.record_event(GameEvent::PlayerLost { player, reason });
            self.remove_departing_players_objects(player);
            if self
                .combat
                .as_ref()
                .is_some_and(|combat| combat.defending_player == Some(player))
            {
                // CR 800.4a / 506.4a: creatures attacking a player who left
                // the game are removed from combat. Do not redirect them to
                // the next living seat merely because turn order changed.
                let combat = self
                    .combat
                    .as_mut()
                    .expect("combat was present when the departed defender was checked");
                combat.attackers.clear();
                combat.blockers.clear();
            }
        }
    }

    /// Applies the relevant portion of CR 800.4a for this ownership-only
    /// substrate. Objects owned by a departing player leave the game; a
    /// non-owned object under that player's control is exiled to its owner's
    /// zone. This must occur inside the loss transition, not as a later policy
    /// action, so no stale object can receive priority or participate in SBA.
    fn remove_departing_players_objects(&mut self, player: PlayerId) {
        let owned_objects = self
            .objects
            .iter()
            .filter_map(|(id, object)| (object.owner == player).then_some((*id, object.owner)))
            .collect::<Vec<_>>();
        for (object, object_owner) in owned_objects {
            self.remove_from_all_zones(object);
            self.stack
                .retain(|stack_object| stack_object.card != object);
            self.objects.remove(&object);
            self.record_event(GameEvent::ObjectLeftGame {
                object,
                owner: object_owner,
            });
            self.expire_continuous_effects_involving(object);
        }

        let controlled_but_not_owned = self
            .objects
            .iter()
            .filter_map(|(id, object)| {
                (object.controller == player && object.owner != player).then_some(*id)
            })
            .collect::<Vec<_>>();
        for object in controlled_but_not_owned {
            self.stack
                .retain(|stack_object| stack_object.card != object);
            let owner = self.objects[&object].owner;
            self.remove_from_all_zones(object);
            let object_state = self
                .objects
                .get_mut(&object)
                .expect("controlled object was collected from this game");
            object_state.controller = owner;
            self.players[owner.0].exile.push(object);
            self.record_event(GameEvent::CardMoved {
                card: object,
                to: Zone::Exile,
            });
            self.expire_continuous_effects_involving(object);
        }
    }

    fn record_game_end_if_needed(&mut self) {
        if !self.terminal_event_emitted && self.is_game_over() {
            self.terminal_event_emitted = true;
            self.record_event(GameEvent::GameEnded {
                winner: self.winner(),
            });
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
        for _ in 0..6 {
            if game.step == Step::DeclareAttackers {
                game.declare_attackers(game.active_player, &[])
                    .expect("empty attackers are an explicit turn action");
            }
            let first = game.priority;
            game.pass_priority(first).expect("first pass");
            let second = game.priority;
            game.pass_priority(second).expect("second pass");
        }
        assert_eq!(game.turn, 2);
        assert_eq!(game.active_player, PlayerId(1));
        assert_eq!(game.step, Step::Upkeep);
        assert_eq!(game.priority, PlayerId(1));
    }
}
