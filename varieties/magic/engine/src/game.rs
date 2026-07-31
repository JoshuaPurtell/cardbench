use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, ActivatedManaAbility,
    AdditionalSpellCost, AdditionalSpellCostBinding, BasicLandType, BasicLandTypeBinding,
    CardDefinition, CardObject, CardType, CastPaymentManaAbility, Characteristics, Color,
    CombatBlock, ContinuousChange, ContinuousEffect, DeckList, Duration, Effect, GameEvent,
    Keyword, ManaAbilityActivation, ManaAbilityBinding, ManaAbilityOutput, ManaPaymentSelection,
    ObjectId, PlayerId, PlayerState, PolicyMoveKind, StackEffectResolution, StackObject,
    StackResolutionPlan, Step, Target, TargetRequirement, TokenSpec, TriggerCondition,
    TriggeredAbilityBinding, Zone,
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
    /// Ordered mana activations performed while this spell's cost is being
    /// paid. These never become stack objects. The engine
    /// preflights and applies them as part of the enclosing cast transaction,
    /// so a later failed activation or spell payment restores every earlier
    /// tap, mana-pool debit/output, and event receipt.
    pub payment_mana_abilities: Vec<CastPaymentManaAbility>,
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
    /// Activates transmute, optionally selecting a matching mana-value card
    /// from the controller's library. A hidden-zone quality search may find
    /// nothing; the engine enforces timing and payment in either case.
    Transmute {
        card: ObjectId,
        found: Option<ObjectId>,
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
    /// Activates a definition-bound non-mana ability. Unlike mana abilities,
    /// the activation becomes a stack object and opens a normal response
    /// window for every surviving player.
    ActivateAbility {
        activation: AbilityActivation,
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
            Self::ActivateAbility { .. } => PolicyMoveKind::ActivateAbility,
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
    /// Typed basic-land type line, when this card's expansion registered one.
    pub basic_land_type: Option<BasicLandType>,
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
    pub own_life: i64,
    pub opponent_life: Vec<(PlayerId, i64)>,
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
    /// Attackers that had Haste when they were declared. This preserves the
    /// declaration-time exception to summoning sickness separately from a
    /// later characteristics query.
    hasty_attackers: BTreeSet<ObjectId>,
    /// Attackers that had flying when they were declared. Blocking legality is
    /// determined at declaration time, so this cannot be reconstructed from a
    /// later characteristics query after a continuous effect changes a card.
    flying_attackers: BTreeSet<ObjectId>,
    /// Attackers that had Fear when they were declared. Blocking legality is
    /// determined at declaration time, so this cannot be reconstructed from a
    /// later characteristics query after a continuous effect changes a card.
    fear_attackers: BTreeSet<ObjectId>,
    /// Attackers that had the RAV black-only evasion restriction when declared.
    black_evasion_attackers: BTreeSet<ObjectId>,
    /// Attackers that were declared with vigilance. This is declaration
    /// provenance, not a live tapped-state assertion: a vigilant attacker can
    /// later pay a legal tap cost while it remains in combat.
    vigilant_attackers: BTreeSet<ObjectId>,
    /// Attackers that had Trample when declared. Unlike evasion, trample is
    /// evaluated from the attacker's live characteristics at combat-damage
    /// assignment; this set is retained only as declaration provenance for
    /// the public combat audit.
    trampling_attackers: BTreeSet<ObjectId>,
    must_be_blocked_attackers: BTreeSet<ObjectId>,
    mountainwalk_attackers: BTreeSet<ObjectId>,
    blockers: BTreeMap<ObjectId, ObjectId>,
    /// Blockers admitted against a declared flying attacker because they had
    /// either Flying or Reach at blocker declaration. This is provenance, not
    /// an assertion that the blocker retains either keyword afterward.
    evasion_qualified_blockers: BTreeSet<ObjectId>,
    /// Blockers admitted against a declared Fear attacker because they were
    /// black or artifact creatures at blocker declaration. This is provenance
    /// rather than a live characteristics assertion.
    fear_qualified_blockers: BTreeSet<ObjectId>,
    /// Blockers admitted against a black-only evasion attacker because they
    /// were black at blocker declaration.
    black_evasion_qualified_blockers: BTreeSet<ObjectId>,
    /// This initial slice attacks the next living seat. It records that seat
    /// at declaration time rather than recomputing turn order after a player
    /// leaves in the middle of combat.
    defending_player: Option<PlayerId>,
    attackers_declared: bool,
    blockers_declared: bool,
    /// Sources that assigned damage in the first-strike damage step. They
    /// cannot assign again in this combat's later normal damage step.
    first_strike_damage_sources: BTreeSet<ObjectId>,
}

#[derive(Clone, Debug)]
struct PendingDamageTrigger {
    source: ObjectId,
    controller: PlayerId,
    ability: crate::TriggeredAbility,
    damage_amount: i16,
}

#[derive(Clone, Debug)]
struct PendingDiesTrigger {
    source: ObjectId,
    definition: &'static str,
}

#[derive(Clone, Debug)]
struct PendingDamageRedirection {
    source: ObjectId,
    protected: ObjectId,
    remaining: i32,
}

#[derive(Clone, Debug)]
struct PendingLifeGainTrigger {
    source: ObjectId,
    controller: PlayerId,
    ability: crate::TriggeredAbility,
}

#[derive(Clone, Debug)]
struct DamageRedirection {
    protected: ObjectId,
    destination: Target,
    remaining: i32,
    expires_turn: u32,
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
    activated_abilities: BTreeMap<&'static str, BTreeMap<&'static str, ActivatedAbility>>,
    triggered_abilities: BTreeMap<&'static str, BTreeMap<&'static str, crate::TriggeredAbility>>,
    basic_land_types: BTreeMap<&'static str, BasicLandType>,
    additional_spell_costs: BTreeMap<&'static str, Vec<AdditionalSpellCost>>,
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
    /// Immutable definitions retained for cards removed by CR 800.4a. A
    /// departed spell may remain named by a lower stack object's historical
    /// target even though its live object and zone membership are gone.
    departed_card_definitions: BTreeMap<ObjectId, &'static str>,
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
    /// Positive damage triggers are collected during a damage batch and only
    /// put on the stack after the enclosing combat or stack object finishes
    /// resolving. This preserves Magic's event ordering and leaves one clean
    /// priority boundary for the resulting abilities.
    pending_damage_triggers: Vec<PendingDamageTrigger>,
    pending_life_gain_triggers: Vec<PendingLifeGainTrigger>,
    pending_dies_triggers: Vec<PendingDiesTrigger>,
    pending_damage_redirection: Option<PendingDamageRedirection>,
    damage_redirections: Vec<DamageRedirection>,
}

impl Game {
    pub fn new(
        definitions: impl IntoIterator<Item = CardDefinition>,
        player_count: usize,
    ) -> Result<Self, RulesError> {
        Self::new_with_mana_abilities_and_basic_land_types(
            definitions,
            player_count,
            std::iter::empty::<ManaAbilityBinding>(),
            std::iter::empty::<BasicLandTypeBinding>(),
        )
    }

    /// Creates a game with expansion-provided typed basic-land definitions.
    ///
    /// Existing `Game::new` callers stay valid when a set does not yet expose
    /// a typed basic-land type line.
    pub fn new_with_basic_land_types(
        definitions: impl IntoIterator<Item = CardDefinition>,
        player_count: usize,
        basic_land_type_bindings: impl IntoIterator<Item = BasicLandTypeBinding>,
    ) -> Result<Self, RulesError> {
        Self::new_with_mana_abilities_and_basic_land_types(
            definitions,
            player_count,
            std::iter::empty::<ManaAbilityBinding>(),
            basic_land_type_bindings,
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
        Self::new_with_mana_abilities_and_basic_land_types(
            definitions,
            player_count,
            bindings,
            std::iter::empty::<BasicLandTypeBinding>(),
        )
    }

    /// Creates a game with both definition-bound mana abilities and typed
    /// basic-land type lines supplied by an expansion module.
    #[allow(clippy::too_many_lines)] // Construction validates immutable expansion bindings together.
    pub fn new_with_mana_abilities_and_basic_land_types(
        definitions: impl IntoIterator<Item = CardDefinition>,
        player_count: usize,
        bindings: impl IntoIterator<Item = ManaAbilityBinding>,
        basic_land_type_bindings: impl IntoIterator<Item = BasicLandTypeBinding>,
    ) -> Result<Self, RulesError> {
        Self::new_with_mana_abilities_basic_land_types_and_additional_spell_costs(
            definitions,
            player_count,
            bindings,
            basic_land_type_bindings,
            std::iter::empty::<AdditionalSpellCostBinding>(),
        )
    }

    /// Creates a game with expansion-bound mana abilities, typed basic lands,
    /// and explicit additional spell costs. The cost bindings are immutable
    /// catalog data; a `CastRequest` selects their concrete permanents.
    #[allow(clippy::too_many_arguments, clippy::too_many_lines)]
    pub fn new_with_mana_abilities_basic_land_types_and_additional_spell_costs(
        definitions: impl IntoIterator<Item = CardDefinition>,
        player_count: usize,
        bindings: impl IntoIterator<Item = ManaAbilityBinding>,
        basic_land_type_bindings: impl IntoIterator<Item = BasicLandTypeBinding>,
        additional_spell_cost_bindings: impl IntoIterator<Item = AdditionalSpellCostBinding>,
    ) -> Result<Self, RulesError> {
        if player_count < 2 {
            return Err(RulesError::IllegalAction(
                "Magic games need at least two seated players",
            ));
        }
        let mut catalog = BTreeMap::new();
        for definition in definitions {
            Self::validate_mana_cost(&definition.mana_cost)?;
            for keyword in &definition.keywords {
                if let Keyword::Transmute(cost) = keyword {
                    Self::validate_mana_cost(cost)?;
                }
            }
            if catalog.insert(definition.id, definition).is_some() {
                return Err(RulesError::IllegalAction("duplicate card definition id"));
            }
        }
        let mut basic_land_types = BTreeMap::<&'static str, BasicLandType>::new();
        for binding in basic_land_type_bindings {
            let definition = catalog
                .get(binding.card_definition)
                .ok_or(RulesError::UnknownDefinition(binding.card_definition))?;
            if !definition.is_basic_land || !definition.is_land() {
                return Err(RulesError::IllegalAction(
                    "a basic land type binding requires a basic land definition",
                ));
            }
            if definition.mana_colors != BTreeSet::from([binding.land_type.intrinsic_mana_color()])
            {
                return Err(RulesError::IllegalAction(
                    "a basic land type must match its intrinsic mana color",
                ));
            }
            if basic_land_types
                .insert(binding.card_definition, binding.land_type)
                .is_some()
            {
                return Err(RulesError::IllegalAction(
                    "duplicate basic land type binding for card definition",
                ));
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
        let mut additional_spell_costs = BTreeMap::<&'static str, Vec<AdditionalSpellCost>>::new();
        for binding in additional_spell_cost_bindings {
            let definition = catalog
                .get(binding.card_definition)
                .ok_or(RulesError::UnknownDefinition(binding.card_definition))?;
            if definition.is_land() {
                return Err(RulesError::IllegalAction(
                    "an additional spell cost binding requires a nonland spell definition",
                ));
            }
            let costs = additional_spell_costs
                .entry(binding.card_definition)
                .or_default();
            if costs.contains(&binding.cost) {
                return Err(RulesError::IllegalAction(
                    "duplicate additional spell cost binding for card definition",
                ));
            }
            costs.push(binding.cost);
        }
        let players = (0..player_count)
            .map(|index| PlayerState::new(PlayerId(index)))
            .collect();
        let game = Self {
            catalog,
            mana_abilities,
            activated_abilities: BTreeMap::new(),
            triggered_abilities: BTreeMap::new(),
            basic_land_types,
            additional_spell_costs,
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
            departed_card_definitions: BTreeMap::new(),
            consecutive_passes: 0,
            shuffle_seed: 0,
            started: false,
            terminal_event_emitted: false,
            combat: None,
            pending_draw_replacement: None,
            pending_damage_triggers: Vec::new(),
            pending_life_gain_triggers: Vec::new(),
            pending_dies_triggers: Vec::new(),
            pending_damage_redirection: None,
            damage_redirections: Vec::new(),
        };
        // Construction is the first observable state-machine boundary. Do not
        // hand a caller a game whose immutable catalog or initial seats already
        // violate the same contract every later transition relies on.
        game.validate_invariants()?;
        Ok(game)
    }

    /// Creates a game with expansion-provided, stack-using activated abilities
    /// in addition to the existing mana/basic-land/additional-cost bindings.
    pub fn new_with_all_bindings(
        definitions: impl IntoIterator<Item = CardDefinition>,
        player_count: usize,
        mana_bindings: impl IntoIterator<Item = ManaAbilityBinding>,
        basic_land_types: impl IntoIterator<Item = BasicLandTypeBinding>,
        additional_spell_costs: impl IntoIterator<Item = AdditionalSpellCostBinding>,
        ability_bindings: impl IntoIterator<Item = ActivatedAbilityBinding>,
    ) -> Result<Self, RulesError> {
        let mut game = Self::new_with_mana_abilities_basic_land_types_and_additional_spell_costs(
            definitions,
            player_count,
            mana_bindings,
            basic_land_types,
            additional_spell_costs,
        )?;
        for binding in ability_bindings {
            let definition = game
                .catalog
                .get(binding.card_definition)
                .ok_or(RulesError::UnknownDefinition(binding.card_definition))?;
            if !definition.is_permanent() {
                return Err(RulesError::IllegalAction(
                    "an activated ability binding requires a permanent source",
                ));
            }
            Self::validate_activated_ability_definition(&binding.ability)?;
            let abilities = game
                .activated_abilities
                .entry(binding.card_definition)
                .or_default();
            if abilities
                .insert(binding.ability.id, binding.ability)
                .is_some()
            {
                return Err(RulesError::IllegalAction(
                    "duplicate activated ability id for card definition",
                ));
            }
        }
        game.validate_invariants()?;
        Ok(game)
    }

    /// Creates a game with stack-using activated abilities and target-free
    /// enter-the-battlefield, life-gain, or source-damage triggers. Enter
    /// triggers queue after a permanent spell resolves; damage and life-gain
    /// triggers queue after their enclosing effect, so each exposes a normal
    /// priority window.
    pub fn new_with_all_bindings_and_triggers(
        definitions: impl IntoIterator<Item = CardDefinition>,
        player_count: usize,
        mana_bindings: impl IntoIterator<Item = ManaAbilityBinding>,
        basic_land_types: impl IntoIterator<Item = BasicLandTypeBinding>,
        additional_spell_costs: impl IntoIterator<Item = AdditionalSpellCostBinding>,
        ability_bindings: impl IntoIterator<Item = ActivatedAbilityBinding>,
        trigger_bindings: impl IntoIterator<Item = TriggeredAbilityBinding>,
    ) -> Result<Self, RulesError> {
        let mut game = Self::new_with_all_bindings(
            definitions,
            player_count,
            mana_bindings,
            basic_land_types,
            additional_spell_costs,
            ability_bindings,
        )?;
        for binding in trigger_bindings {
            let definition = game
                .catalog
                .get(binding.card_definition)
                .ok_or(RulesError::UnknownDefinition(binding.card_definition))?;
            if !definition.is_permanent()
                || !matches!(
                    binding.ability.condition,
                    TriggerCondition::EntersBattlefield
                        | TriggerCondition::LifeGained
                        | TriggerCondition::DealsDamage
                        | TriggerCondition::ReceivesDamage
                        | TriggerCondition::Dies
                        | TriggerCondition::Attacks
                )
                || binding.ability.targets
                    != binding
                        .ability
                        .effects
                        .iter()
                        .filter_map(Effect::target_requirement)
                        .collect::<Vec<_>>()
            {
                return Err(RulesError::IllegalAction(
                    "trigger binding requires a permanent and matching target requirements",
                ));
            }
            let abilities = game
                .triggered_abilities
                .entry(binding.card_definition)
                .or_default();
            if abilities
                .insert(binding.ability.id, binding.ability)
                .is_some()
            {
                return Err(RulesError::IllegalAction(
                    "duplicate triggered ability id for card definition",
                ));
            }
        }
        game.validate_invariants()?;
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
        if self.started {
            return Err(RulesError::IllegalAction(
                "cards may be added only before the game begins",
            ));
        }
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
                damage_shield: 0,
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
        if self.started {
            return Err(RulesError::IllegalAction(
                "mana may be granted only before the game begins",
            ));
        }
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
        if self.started {
            return Err(RulesError::IllegalAction("tap state is setup-only"));
        }
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

    /// Adjusts battlefield-entry provenance only while building a fixture.
    /// This lets a public scenario model a creature that entered on an earlier
    /// turn without exposing a live-game summoning-sickness bypass.
    pub fn set_entered_turn_for_setup(
        &mut self,
        card: ObjectId,
        entered_turn: u32,
    ) -> Result<(), RulesError> {
        if self.started {
            return Err(RulesError::IllegalAction(
                "battlefield entry turn is setup-only",
            ));
        }
        if entered_turn > self.turn {
            return Err(RulesError::IllegalAction(
                "setup battlefield entry turn cannot be in the future",
            ));
        }
        let object = self
            .objects
            .get_mut(&card)
            .ok_or(RulesError::UnknownCard(card))?;
        object.entered_turn = entered_turn;
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
        self.atomic_transition(|game| {
            game.require_priority(player)?;
            game.activate_intrinsic_mana_ability_impl(player, land, color, None)?;
            game.consecutive_passes = 0;
            Ok(())
        })
    }

    /// Applies an intrinsic land mana ability after the caller has established
    /// its action context. Direct activation requires priority; cast payment
    /// keeps its enclosing spell transaction open around this primitive.
    fn activate_intrinsic_mana_ability_impl(
        &mut self,
        player: PlayerId,
        land: ObjectId,
        color: Color,
        payment_spell: Option<ObjectId>,
    ) -> Result<(), RulesError> {
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
        if self
            .basic_land_type(land)?
            .is_some_and(|land_type| land_type.intrinsic_mana_color() != color)
        {
            return Err(RulesError::IllegalAction(
                "that basic land type cannot produce the requested color",
            ));
        }
        if !self.players[player.0].mana_pool.can_add(color, 1) {
            return Err(RulesError::IllegalAction(
                "mana pool cannot hold the requested mana",
            ));
        }
        if let Some(card) = payment_spell {
            self.record_event(GameEvent::CastPaymentBasicLandManaAbilityActivated {
                player,
                card,
                land,
                color,
            });
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
        Ok(())
    }

    /// Applies one explicitly selected typed basic-land mana ability while a
    /// spell cost is paid. Only a registered type line may use this narrower
    /// context, so an untyped land cannot masquerade as a basic intrinsic
    /// source inside a cast request.
    fn activate_cast_payment_basic_land_mana_ability(
        &mut self,
        player: PlayerId,
        card: ObjectId,
        land: ObjectId,
        color: Color,
    ) -> Result<(), RulesError> {
        if self.basic_land_type(land)?.is_none() {
            return Err(RulesError::IllegalAction(
                "cast-payment intrinsic mana ability requires a typed basic land",
            ));
        }
        self.activate_intrinsic_mana_ability_impl(player, land, color, Some(card))
    }

    /// Activates a definition-bound mana ability without using the stack.
    ///
    /// Every condition is preflighted before an object, life total, mana pool,
    /// pass sequence, or event changes. A creature's tap ability observes this
    /// engine slice's summoning-sickness boundary unless it has Haste. Controller damage is an ability result rather than a
    /// life-payment cost, so it remains legal even when it will cause a loss.
    #[allow(clippy::too_many_lines)] // One method keeps the activation transaction atomic and auditable.
    pub fn activate_bound_mana_ability(
        &mut self,
        player: PlayerId,
        activation: ManaAbilityActivation,
    ) -> Result<(), RulesError> {
        self.atomic_transition(|game| {
            game.require_priority(player)?;
            game.activate_bound_mana_ability_impl(player, activation, None)?;
            game.consecutive_passes = 0;
            game.priority = player;
            game.check_state_based_actions()?;
            Ok(())
        })
    }

    /// Activates a definition-bound non-mana ability. Costs are paid
    /// atomically, then the ability is placed on the stack and the activating
    /// player retains priority under CR 117.3c. The source remains a normal
    /// battlefield/graveyard object; `StackObject::ability_id` distinguishes
    /// this stack item from a spell so resolution never moves the source card.
    pub fn activate_ability(
        &mut self,
        player: PlayerId,
        activation: AbilityActivation,
    ) -> Result<(), RulesError> {
        self.atomic_transition(|game| {
            game.require_priority(player)?;
            game.activate_ability_impl(player, activation)?;
            game.flush_pending_dies_triggers();
            game.check_state_based_actions()?;
            game.validate_invariants()
        })
    }

    #[allow(clippy::too_many_lines)]
    fn activate_ability_impl(
        &mut self,
        player: PlayerId,
        activation: AbilityActivation,
    ) -> Result<(), RulesError> {
        self.require_zone(activation.source, Zone::Battlefield)?;
        let source = self.object(activation.source)?.clone();
        if source.controller != player {
            return Err(RulesError::IllegalAction(
                "activated ability source must be controlled by its activator",
            ));
        }
        let definition_id = source.definition.ok_or(RulesError::IllegalAction(
            "a token has no definition-bound activated ability",
        ))?;
        let ability = self
            .activated_abilities
            .get(definition_id)
            .and_then(|abilities| abilities.get(activation.ability_id))
            .ok_or(RulesError::IllegalAction(
                "source does not have the requested activated ability",
            ))?
            .clone();
        if activation.targets.len() != ability.targets.len() {
            return Err(RulesError::IllegalAction(
                "activated ability target count does not match its definition",
            ));
        }
        for (target, requirement) in activation.targets.iter().zip(&ability.targets) {
            if !Self::target_shape_matches(*target, *requirement)
                || !self.target_matches(*target, *requirement)
            {
                return Err(RulesError::IllegalTarget(*target));
            }
        }
        let expected_sacrifices =
            usize::from(ability.sacrifice_lands) + usize::from(ability.sacrifice_source);
        if activation.sacrifice_sources.len() != expected_sacrifices {
            return Err(RulesError::IllegalAction(
                "activated ability sacrifice selection count does not match its definition",
            ));
        }
        let mut selected_sacrifices = BTreeSet::new();
        for (index, permanent) in activation.sacrifice_sources.iter().enumerate() {
            if !selected_sacrifices.insert(*permanent) {
                return Err(RulesError::IllegalAction(
                    "an activated ability cannot sacrifice the same permanent twice",
                ));
            }
            self.require_zone(*permanent, Zone::Battlefield)?;
            let object = self.object(*permanent)?;
            if object.controller != player {
                return Err(RulesError::IllegalAction(
                    "an activated ability can sacrifice only a controlled permanent",
                ));
            }
            if index < usize::from(ability.sacrifice_source) {
                if *permanent != activation.source {
                    return Err(RulesError::IllegalAction(
                        "the source sacrifice selection must name the ability source",
                    ));
                }
            } else if !self.card_definition(*permanent)?.is_land() {
                return Err(RulesError::IllegalAction(
                    "this activated ability requires a sacrificed land",
                ));
            }
        }
        if activation.discard_cards.len() != usize::from(ability.discard_cards) {
            return Err(RulesError::IllegalAction(
                "activated ability discard selection count does not match its definition",
            ));
        }
        let mut selected_discards = BTreeSet::new();
        for card in &activation.discard_cards {
            if !selected_discards.insert(*card) {
                return Err(RulesError::IllegalAction(
                    "an activated ability cannot discard the same card twice",
                ));
            }
            self.require_zone(*card, Zone::Hand)?;
            if self.object(*card)?.owner != player {
                return Err(RulesError::IllegalAction(
                    "an activated ability can discard only the activating player's card",
                ));
            }
        }
        let mut paid_pool = self.players[player.0].mana_pool.clone();
        paid_pool
            .pay(&ability.mana_cost)
            .map_err(RulesError::Mana)?;
        if ability.tap_cost {
            if source.tapped {
                return Err(RulesError::IllegalAction(
                    "activated ability requires an untapped source",
                ));
            }
            let characteristics = self.characteristics(activation.source)?;
            if characteristics.card_types.contains(&CardType::Creature)
                && source.entered_turn >= self.turn
                && !characteristics.keywords.contains(&Keyword::Haste)
            {
                return Err(RulesError::IllegalAction(
                    "a summoning-sick creature cannot pay an activated tap cost",
                ));
            }
        }
        self.players[player.0].mana_pool = paid_pool;
        if ability.mana_cost.mana_value() > 0 {
            self.record_event(GameEvent::AbilityManaPaid {
                player,
                source: activation.source,
                ability: ability.id,
                mana_cost: ability.mana_cost.clone(),
            });
        }
        for card in &activation.discard_cards {
            self.record_event(GameEvent::DiscardedAsAbilityCost {
                player,
                source: activation.source,
                card: *card,
            });
            self.move_to_graveyard_or_remove_token(*card)?;
        }
        if ability.tap_cost {
            self.objects
                .get_mut(&activation.source)
                .ok_or(RulesError::UnknownCard(activation.source))?
                .tapped = true;
        }
        if ability.sacrifice_source {
            self.record_event(GameEvent::SacrificedAsAbilityCost {
                player,
                source: activation.source,
                permanent: activation.source,
            });
            self.move_to_graveyard_or_remove_token(activation.source)?;
        }
        for permanent in activation
            .sacrifice_sources
            .iter()
            .skip(usize::from(ability.sacrifice_source))
        {
            self.record_event(GameEvent::SacrificedAsAbilityCost {
                player,
                source: activation.source,
                permanent: *permanent,
            });
            self.move_to_graveyard_or_remove_token(*permanent)?;
        }
        self.stack.push(StackObject {
            card: activation.source,
            controller: player,
            ability_id: Some(ability.id),
            targets: activation.targets,
            effects: ability.effects,
            mana_spent: None,
        });
        self.record_event(GameEvent::AbilityActivated {
            player,
            source: activation.source,
            ability: ability.id,
        });
        self.consecutive_passes = 0;
        self.priority = player;
        Ok(())
    }

    /// Applies an already-authorized definition-bound mana ability. Public
    /// activation obtains priority before entering this primitive; cast
    /// payment instead holds the single cast transaction open around its
    /// ordered payment-context activations.
    #[allow(clippy::too_many_lines)] // One primitive keeps the activation transaction auditable.
    fn activate_bound_mana_ability_impl(
        &mut self,
        player: PlayerId,
        activation: ManaAbilityActivation,
        payment_spell: Option<ObjectId>,
    ) -> Result<(), RulesError> {
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
            let characteristics = self.characteristics(activation.source)?;
            if characteristics.card_types.contains(&CardType::Creature)
                && source.entered_turn >= self.turn
                && !characteristics.keywords.contains(&Keyword::Haste)
            {
                return Err(RulesError::IllegalAction(
                    "a summoning-sick creature cannot pay a tap mana-ability cost",
                ));
            }
        }
        if let Some(life_payment) = ability.life_payment
            && self.players[player.0].life < i64::from(life_payment)
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

        if let Some(card) = payment_spell {
            self.record_event(GameEvent::CastPaymentManaAbilityActivated {
                player,
                card,
                source: activation.source,
                ability: ability.id,
            });
        }

        if ability.tap_cost {
            self.objects
                .get_mut(&activation.source)
                .ok_or(RulesError::UnknownCard(activation.source))?
                .tapped = true;
        }
        if let Some(life_payment) = ability.life_payment {
            self.players[player.0].life -= i64::from(life_payment);
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
            if let Some(amount) = ability.controller_damage {
                self.deal_damage_to_player(activation.source, player, i32::from(amount))?;
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
            if let Some(amount) = ability.controller_damage {
                self.deal_damage_to_player(activation.source, player, i32::from(amount))?;
            }
        }
        Ok(())
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

    /// Returns the registered typed basic-land type line for a card object.
    pub fn basic_land_type(&self, card: ObjectId) -> Result<Option<BasicLandType>, RulesError> {
        let object = self.object(card)?;
        Ok(object
            .definition
            .and_then(|definition| self.basic_land_types.get(definition).copied()))
    }

    fn player_controls_basic_land_type(&self, player: PlayerId, land_type: BasicLandType) -> bool {
        self.players[player.0].battlefield.iter().any(|card| {
            self.basic_land_type(*card)
                .is_ok_and(|registered| registered == Some(land_type))
        })
    }

    fn target_cannot_block_attacker(&self, blocker: ObjectId, attacker: ObjectId) -> bool {
        self.continuous_effects.iter().any(|effect| {
            effect.target == blocker
                && self.effect_is_active(effect)
                && matches!(
                    &effect.change,
                    ContinuousChange::CannotBlockSource(source) if *source == attacker
                )
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
            PolicyAction::ActivateAbility { activation } => {
                self.activate_ability(player, activation)?;
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
                creature_subtypes: token.creature_subtypes.clone(),
                power: Some(i32::from(token.power)),
                toughness: Some(i32::from(token.toughness)),
                keywords: token.keywords.clone(),
            }
        } else {
            let definition = self.card_definition(card)?;
            Characteristics {
                colors: definition.colors.clone(),
                card_types: definition.card_types.clone(),
                creature_subtypes: BTreeSet::new(),
                power: definition.power.map(i32::from),
                toughness: definition.toughness.map(i32::from),
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
                ContinuousChange::RemoveKeyword(keyword) => {
                    characteristics
                        .keywords
                        .retain(|candidate| candidate != keyword);
                }
                ContinuousChange::CannotBlockSource(_) | ContinuousChange::AddDamageShield(_) => {}
                ContinuousChange::ModifyPowerToughness { power, toughness } => {
                    characteristics.power = characteristics
                        .power
                        .map(|current| current + i32::from(*power));
                    characteristics.toughness = characteristics
                        .toughness
                        .map(|current| current + i32::from(*toughness));
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
        if let ContinuousChange::AddDamageShield(amount) = &change {
            self.objects
                .get_mut(&target)
                .ok_or(RulesError::UnknownCard(target))?
                .damage_shield += i32::from(*amount);
        }
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
        let mut hasty_attackers = BTreeSet::new();
        let mut flying_attackers = BTreeSet::new();
        let mut fear_attackers = BTreeSet::new();
        let mut black_evasion_attackers = BTreeSet::new();
        let mut vigilant_attackers = BTreeSet::new();
        let mut trampling_attackers = BTreeSet::new();
        let mut must_be_blocked_attackers = BTreeSet::new();
        let mut mountainwalk_attackers = BTreeSet::new();
        for attacker in attackers {
            if !seen.insert(*attacker) {
                return Err(RulesError::IllegalAction("an attacker was declared twice"));
            }
            self.require_zone(*attacker, Zone::Battlefield)?;
            let object = self.object(*attacker)?;
            let characteristics = self.characteristics(*attacker)?;
            let has_haste = characteristics.keywords.contains(&Keyword::Haste);
            if object.controller != player
                || object.tapped
                || (object.entered_turn >= self.turn && !has_haste)
                || !characteristics.card_types.contains(&CardType::Creature)
                || characteristics.keywords.contains(&Keyword::Defender)
                || characteristics
                    .keywords
                    .contains(&Keyword::CannotAttackOrBlock)
            {
                return Err(RulesError::IllegalAction("illegal attacker"));
            }
            if has_haste {
                hasty_attackers.insert(*attacker);
            }
            if characteristics.keywords.contains(&Keyword::Flying) {
                flying_attackers.insert(*attacker);
            }
            if characteristics.keywords.contains(&Keyword::Fear) {
                fear_attackers.insert(*attacker);
            }
            if characteristics.keywords.contains(&Keyword::BlackEvasion) {
                black_evasion_attackers.insert(*attacker);
            }
            if characteristics.keywords.contains(&Keyword::Vigilance) {
                vigilant_attackers.insert(*attacker);
            }
            if characteristics.keywords.contains(&Keyword::Trample) {
                trampling_attackers.insert(*attacker);
            }
            if characteristics
                .keywords
                .contains(&Keyword::MustBeBlockedIfAble)
            {
                must_be_blocked_attackers.insert(*attacker);
            }
            if characteristics.keywords.contains(&Keyword::Mountainwalk) {
                mountainwalk_attackers.insert(*attacker);
            }
        }
        for attacker in attackers {
            if !vigilant_attackers.contains(attacker) {
                self.objects
                    .get_mut(attacker)
                    .ok_or(RulesError::UnknownCard(*attacker))?
                    .tapped = true;
            }
        }
        let defending_player = self.next_player(player);
        let combat = self
            .combat
            .as_mut()
            .ok_or(RulesError::IllegalAction("combat was not initialized"))?;
        combat.attackers = attackers.to_vec();
        combat.hasty_attackers = hasty_attackers;
        combat.flying_attackers = flying_attackers;
        combat.fear_attackers = fear_attackers;
        combat.black_evasion_attackers = black_evasion_attackers;
        combat.vigilant_attackers = vigilant_attackers;
        combat.trampling_attackers = trampling_attackers;
        combat.must_be_blocked_attackers = must_be_blocked_attackers;
        combat.mountainwalk_attackers = mountainwalk_attackers;
        combat.defending_player = Some(defending_player);
        combat.attackers_declared = true;
        self.record_event(GameEvent::AttackersDeclared {
            player,
            attackers: attackers.to_vec(),
        });
        for attacker in attackers {
            self.enqueue_attack_triggers(*attacker)?;
        }
        self.consecutive_passes = 0;
        // CR 508.2: the active player receives priority after attackers are
        // declared. Declaration itself is a turn-based action, not a normal
        // priority action, so a stale pre-declaration holder cannot block it.
        self.priority = self.active_player;
        self.validate_invariants()
    }

    /// Performs the turn-based action of assigning zero or one blocker to each
    /// attacker in the initial combat slice.
    #[allow(clippy::too_many_lines)] // Declaration validates every evasion and restriction atomically.
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
        let mut evasion_qualified_blockers = BTreeSet::new();
        let mut fear_qualified_blockers = BTreeSet::new();
        let mut black_evasion_qualified_blockers = BTreeSet::new();
        for assignment in assignments {
            if !combat.attackers.contains(&assignment.attacker)
                || !attackers.insert(assignment.attacker)
                || !blockers.insert(assignment.blocker)
            {
                return Err(RulesError::IllegalAction("invalid blocker assignment"));
            }
            self.require_zone(assignment.blocker, Zone::Battlefield)?;
            let object = self.object(assignment.blocker)?;
            let characteristics = self.characteristics(assignment.blocker)?;
            if object.controller != player
                || object.tapped
                || !characteristics.card_types.contains(&CardType::Creature)
                || self.target_cannot_block_attacker(assignment.blocker, assignment.attacker)
                || characteristics
                    .keywords
                    .contains(&Keyword::CannotAttackOrBlock)
                || characteristics.keywords.contains(&Keyword::CannotBlock)
                || (characteristics
                    .keywords
                    .contains(&Keyword::CannotBlockUnlessControlsMountain)
                    && !self.player_controls_basic_land_type(player, BasicLandType::Mountain))
            {
                return Err(RulesError::IllegalAction("illegal blocker"));
            }
            if combat.flying_attackers.contains(&assignment.attacker) {
                if !(characteristics.keywords.contains(&Keyword::Flying)
                    || characteristics.keywords.contains(&Keyword::Reach))
                {
                    return Err(RulesError::IllegalAction(
                        "flying attacker can be blocked only by flying or reach",
                    ));
                }
                evasion_qualified_blockers.insert(assignment.blocker);
            }
            if combat.fear_attackers.contains(&assignment.attacker)
                && !characteristics.card_types.contains(&CardType::Artifact)
                && !characteristics.colors.contains(&Color::Black)
            {
                return Err(RulesError::IllegalAction(
                    "fear attacker can be blocked only by black or artifact creatures",
                ));
            }
            if combat.fear_attackers.contains(&assignment.attacker) {
                fear_qualified_blockers.insert(assignment.blocker);
            }
            if combat
                .black_evasion_attackers
                .contains(&assignment.attacker)
                && !characteristics.colors.contains(&Color::Black)
            {
                return Err(RulesError::IllegalAction(
                    "black-only evasion attacker can be blocked only by black creatures",
                ));
            }
            if combat
                .black_evasion_attackers
                .contains(&assignment.attacker)
            {
                black_evasion_qualified_blockers.insert(assignment.blocker);
            }
            if combat.mountainwalk_attackers.contains(&assignment.attacker)
                && self.player_controls_basic_land_type(player, BasicLandType::Mountain)
            {
                return Err(RulesError::IllegalAction(
                    "mountainwalk attacker cannot be blocked while defender controls a Mountain",
                ));
            }
        }
        for attacker in &combat.must_be_blocked_attackers {
            if attackers.contains(attacker) {
                continue;
            }
            let has_legal_blocker = self.players[player.0].battlefield.iter().any(|candidate| {
                if blockers.contains(candidate) {
                    return false;
                }
                let Ok(object) = self.object(*candidate) else {
                    return false;
                };
                let Ok(characteristics) = self.characteristics(*candidate) else {
                    return false;
                };
                if object.tapped
                    || !characteristics.card_types.contains(&CardType::Creature)
                    || characteristics
                        .keywords
                        .contains(&Keyword::CannotAttackOrBlock)
                    || characteristics.keywords.contains(&Keyword::CannotBlock)
                    || (characteristics
                        .keywords
                        .contains(&Keyword::CannotBlockUnlessControlsMountain)
                        && !self.player_controls_basic_land_type(player, BasicLandType::Mountain))
                {
                    return false;
                }
                if combat.flying_attackers.contains(attacker)
                    && !(characteristics.keywords.contains(&Keyword::Flying)
                        || characteristics.keywords.contains(&Keyword::Reach))
                {
                    return false;
                }
                if combat.black_evasion_attackers.contains(attacker)
                    && !characteristics.colors.contains(&Color::Black)
                {
                    return false;
                }
                if combat.fear_attackers.contains(attacker)
                    && !characteristics.card_types.contains(&CardType::Artifact)
                    && !characteristics.colors.contains(&Color::Black)
                {
                    return false;
                }
                if self.target_cannot_block_attacker(*candidate, *attacker) {
                    return false;
                }
                true
            });
            if has_legal_blocker {
                return Err(RulesError::IllegalAction(
                    "a must-be-blocked attacker had a legal unassigned blocker",
                ));
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
        combat.evasion_qualified_blockers = evasion_qualified_blockers;
        combat.fear_qualified_blockers = fear_qualified_blockers;
        combat.black_evasion_qualified_blockers = black_evasion_qualified_blockers;
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

    #[allow(clippy::needless_pass_by_value)] // Public cast requests remain owned transactional inputs.
    pub fn cast_spell(&mut self, player: PlayerId, request: CastRequest) -> Result<(), RulesError> {
        self.atomic_transition(|game| game.cast_spell_impl(player, &request, None))
    }

    /// Casts a spell with explicit player-selected colors for all generic and
    /// hybrid symbols still payable with mana after Convoke. The exact colors
    /// are recorded before `SpellCast` and retained on the stack object.
    #[allow(clippy::needless_pass_by_value)] // Selection ownership is consumed by the atomic closure boundary.
    pub fn cast_spell_with_mana_spend(
        &mut self,
        player: PlayerId,
        request: CastRequest,
        selection: ManaPaymentSelection,
    ) -> Result<(), RulesError> {
        self.atomic_transition(|game| game.cast_spell_impl(player, &request, Some(&selection)))
    }

    /// Applies a cast under one all-or-error transaction. Mana abilities named
    /// in `CastRequest::payment_mana_abilities` are the only non-stack actions
    /// admitted between cast validation and final spell-cost payment.
    #[allow(clippy::too_many_lines)] // One cast transaction owns all cost receipts and rollback.
    fn cast_spell_impl(
        &mut self,
        player: PlayerId,
        request: &CastRequest,
        mana_payment_selection: Option<&ManaPaymentSelection>,
    ) -> Result<(), RulesError> {
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
        if definition
            .effects
            .iter()
            .any(Effect::requires_explicit_mana_spend)
            && mana_payment_selection.is_none()
        {
            return Err(RulesError::IllegalAction(
                "spell requires an explicit mana-spend selection",
            ));
        }
        Self::validate_cast_effects(&definition)?;
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
        let (spell_targets, additional_cost_selections) =
            self.split_cast_targets(&definition, &request.targets)?;
        self.validate_targets(&definition, &spell_targets)?;
        self.validate_additional_spell_cost_selections(
            &definition,
            player,
            &additional_cost_selections,
        )?;
        self.validate_effect_capacity(&definition, player)?;
        // The selected sacrifice is a genuine component of paying the total
        // cost, not an effect. Keeping it before later selected mana
        // activations also exercises the enclosing atomic cast boundary: a
        // later failed activation restores this zone move and its receipt.
        self.pay_additional_spell_costs(
            &definition,
            player,
            request.card,
            &additional_cost_selections,
        )?;
        for activation in &request.payment_mana_abilities {
            match activation {
                CastPaymentManaAbility::Bound(activation) => {
                    self.activate_bound_mana_ability_impl(player, *activation, Some(request.card))?;
                }
                CastPaymentManaAbility::BasicLand(activation) => {
                    self.activate_cast_payment_basic_land_mana_ability(
                        player,
                        request.card,
                        activation.land,
                        activation.color,
                    )?;
                }
            }
        }
        let (paid_cost, mana_spent) = self.pay_cost_with_convoke(
            player,
            request.card,
            &definition,
            &request.convoke,
            mana_payment_selection,
        )?;
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
            ability_id: None,
            targets: spell_targets,
            effects: definition.effects,
            mana_spent: mana_spent.clone(),
        });
        if let Some(colors) = mana_spent {
            self.record_event(GameEvent::SpellManaPaid {
                player,
                card: request.card,
                colors,
            });
        }
        self.record_event(GameEvent::SpellCast {
            player,
            card: request.card,
        });
        self.consecutive_passes = 0;
        // CR 601.2i / 117.3c: after completing a cast, the acting player
        // receives priority again. Opponents get their response window only
        // after that player passes.
        self.priority = player;
        self.check_state_based_actions()?;
        self.flush_pending_dies_triggers();
        Ok(())
    }

    pub fn pass_priority(&mut self, player: PlayerId) -> Result<(), RulesError> {
        self.atomic_transition(|game| game.pass_priority_impl(player))
    }

    /// Applies a priority pass inside the state-machine transaction boundary.
    /// Keeping the mutable implementation private ensures a failed resolution
    /// cannot leave behind its triggering pass receipt or a popped stack item.
    fn pass_priority_impl(&mut self, player: PlayerId) -> Result<(), RulesError> {
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

    /// Draws the ordinary top card while a spell instruction resolves. This is
    /// intentionally private: public live draws remain restricted to the
    /// draw-step replacement boundary, while a resolved executable effect has
    /// already passed through stack timing and target legality.
    fn draw_card_from_spell_effect(&mut self, player: PlayerId) -> Result<(), RulesError> {
        self.player(player)?;
        let Some(card) = self.players[player.0].library.pop() else {
            self.lose_player(player, "attempted to draw from an empty library");
            self.normalize_priority_after_elimination()?;
            self.record_game_end_if_needed();
            return Ok(());
        };
        self.players[player.0].hand.push(card);
        self.record_event(GameEvent::CardMoved {
            card,
            to: Zone::Hand,
        });
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

    /// Resolves transmute's hand-zone activated ability. A selected matching card is revealed
    /// before moving to hand; `None` models the legal choice to find nothing in a hidden zone.
    /// The library is shuffled by a deterministic seed afterwards.
    pub fn transmute(
        &mut self,
        player: PlayerId,
        card: ObjectId,
        found: Option<ObjectId>,
    ) -> Result<(), RulesError> {
        self.require_priority(player)?;
        if player != self.active_player || !self.step.is_main() || !self.stack.is_empty() {
            return Err(RulesError::IllegalAction(
                "transmute is allowed only during your main phase with an empty stack",
            ));
        }
        self.require_zone(card, Zone::Hand)?;
        if self.object(card)?.owner != player {
            return Err(RulesError::IllegalAction(
                "transmute searches only your own library",
            ));
        }
        let cost = self
            .card_definition(card)?
            .transmute_cost()
            .cloned()
            .ok_or(RulesError::IllegalAction("card has no transmute ability"))?;
        let found_definition = if let Some(found) = found {
            self.require_zone(found, Zone::Library)?;
            if self.object(found)?.owner != player {
                return Err(RulesError::IllegalAction(
                    "transmute searches only your own library",
                ));
            }
            let definition = self.card_definition(found)?;
            if definition.mana_cost.mana_value()
                != self.card_definition(card)?.mana_cost.mana_value()
            {
                return Err(RulesError::IllegalAction(
                    "transmute may find only a card with the discarded card's mana value",
                ));
            }
            Some(definition.id)
        } else {
            None
        };
        let mut pool = self.players[player.0].mana_pool.clone();
        pool.pay(&cost).map_err(RulesError::Mana)?;
        self.players[player.0].mana_pool = pool;
        self.move_to_zone(card, Zone::Graveyard)?;
        if let Some(found) = found {
            self.record_event(GameEvent::CardRevealed {
                player,
                card: found,
                definition: found_definition.expect("selected card has a definition"),
            });
            self.move_to_zone(found, Zone::Hand)?;
        }
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
                self.flush_pending_dies_triggers();
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
        if !self.is_game_over() && self.players[self.active_player.0].lost && self.stack.is_empty()
        {
            return Err(RulesError::IllegalAction(
                "a continuing game assigned the active turn to an eliminated player",
            ));
        }
        if self.event_log != self.event_log_integrity {
            return Err(RulesError::IllegalAction(
                "canonical event log was mutated outside an engine transition",
            ));
        }
        if !self.pending_damage_triggers.is_empty() {
            return Err(RulesError::IllegalAction(
                "pending damage trigger escaped its enclosing damage batch",
            ));
        }
        if !self.pending_life_gain_triggers.is_empty() {
            return Err(RulesError::IllegalAction(
                "pending life-gain trigger escaped its enclosing life-gain batch",
            ));
        }
        if !self.pending_dies_triggers.is_empty() {
            return Err(RulesError::IllegalAction(
                "pending dies trigger escaped its state-based-action batch",
            ));
        }
        Self::validate_mana_ability_event_order(&self.event_log)?;
        Self::validate_spell_mana_payment_event_order(&self.event_log)?;
        self.validate_additional_spell_cost_event_order()?;
        self.validate_stack_terminal_event_order()?;
        self.validate_ability_event_order()?;
        self.validate_ability_discard_cost_event_order()?;
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
        for (definition_id, land_type) in &self.basic_land_types {
            let definition = self
                .catalog
                .get(definition_id)
                .ok_or(RulesError::UnknownDefinition(definition_id))?;
            if !definition.is_basic_land || !definition.is_land() {
                return Err(RulesError::IllegalAction(
                    "a basic land type binding requires a basic land definition",
                ));
            }
            if definition.mana_colors != BTreeSet::from([land_type.intrinsic_mana_color()]) {
                return Err(RulesError::IllegalAction(
                    "a basic land type must match its intrinsic mana color",
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
        for (definition_id, abilities) in &self.activated_abilities {
            let definition = self
                .catalog
                .get(definition_id)
                .ok_or(RulesError::UnknownDefinition(definition_id))?;
            if !definition.is_permanent() {
                return Err(RulesError::IllegalAction(
                    "an activated ability binding requires a permanent source",
                ));
            }
            for (ability_id, ability) in abilities {
                if *ability_id != ability.id {
                    return Err(RulesError::IllegalAction(
                        "activated ability catalog key does not match its ability identity",
                    ));
                }
                Self::validate_activated_ability_definition(ability)?;
            }
        }
        for (definition_id, abilities) in &self.triggered_abilities {
            let definition = self
                .catalog
                .get(definition_id)
                .ok_or(RulesError::UnknownDefinition(definition_id))?;
            if !definition.is_permanent() {
                return Err(RulesError::IllegalAction(
                    "a triggered ability binding requires a permanent source",
                ));
            }
            for (ability_id, ability) in abilities {
                if *ability_id != ability.id
                    || !matches!(
                        ability.condition,
                        TriggerCondition::EntersBattlefield
                            | TriggerCondition::LifeGained
                            | TriggerCondition::DealsDamage
                            | TriggerCondition::ReceivesDamage
                            | TriggerCondition::Dies
                            | TriggerCondition::Attacks
                    )
                    || ability.targets
                        != ability
                            .effects
                            .iter()
                            .filter_map(Effect::target_requirement)
                            .collect::<Vec<_>>()
                {
                    return Err(RulesError::IllegalAction(
                        "triggered ability binding has invalid target requirements",
                    ));
                }
                Self::validate_cast_effects_for_ability(&ability.effects)?;
            }
        }
        for (definition_id, costs) in &self.additional_spell_costs {
            let definition = self
                .catalog
                .get(definition_id)
                .ok_or(RulesError::UnknownDefinition(definition_id))?;
            if definition.is_land() || costs.is_empty() {
                return Err(RulesError::IllegalAction(
                    "additional spell cost binding must name a nonland spell and at least one cost",
                ));
            }
            let mut unique_costs = BTreeSet::new();
            if costs.iter().any(|cost| !unique_costs.insert(*cost)) {
                return Err(RulesError::IllegalAction(
                    "additional spell cost binding has duplicate cost kinds",
                ));
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
                    if object.entered_turn > self.turn
                        || object.damage < 0
                        || object.damage_shield < 0
                    {
                        return Err(RulesError::IllegalAction(
                            "object has impossible turn metadata or negative damage/shield",
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
                        (None, Some(token)) if zone == Zone::Battlefield => {
                            if !token.creature_subtypes.is_empty()
                                && !token.card_types.contains(&CardType::Creature)
                            {
                                return Err(RulesError::IllegalAction(
                                    "a creature subtype requires the creature card type",
                                ));
                            }
                        }
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
        for (stack_index, stack_object) in self.stack.iter().enumerate() {
            let is_ability = stack_object.ability_id.is_some();
            let stack_card_conflict = if is_ability {
                false
            } else {
                locations.contains_key(&stack_object.card) || !stack_cards.insert(stack_object.card)
            };
            if stack_card_conflict {
                return Err(RulesError::IllegalAction(
                    "stack card must not also exist in a zone",
                ));
            }
            let object = self.object(stack_object.card)?;
            self.player(stack_object.controller)?;
            if self.players[stack_object.controller.0].lost {
                return Err(RulesError::IllegalAction(
                    "a departed player controls a stack object",
                ));
            }
            if !is_ability && object.controller != stack_object.controller {
                return Err(RulesError::IllegalAction(
                    "stack controller does not match its card object",
                ));
            }
            if is_ability
                && self.zone_of(stack_object.card) == Some(Zone::Battlefield)
                && object.controller != stack_object.controller
            {
                return Err(RulesError::IllegalAction(
                    "a battlefield ability source must remain controlled by its activator",
                ));
            }
            let definition = object
                .definition
                .and_then(|definition| self.catalog.get(definition))
                .ok_or(RulesError::IllegalAction(
                    "a stack object must be a non-token card with a catalog definition",
                ))?;
            if object.token.is_some() || (!is_ability && definition.is_land()) {
                return Err(RulesError::IllegalAction(
                    "a token or land occupies the stack",
                ));
            }
            if !is_ability && !definition.is_permanent() && definition.effects.is_empty() {
                return Err(RulesError::IllegalAction(
                    "an unsupported nonpermanent card occupies the stack",
                ));
            }
            if !is_ability
                && !definition.card_types.contains(&CardType::Instant)
                && (stack_index != 0
                    || stack_object.controller != self.active_player
                    || !self.step.is_main())
            {
                return Err(RulesError::IllegalAction(
                    "a non-instant stack object has impossible sorcery timing",
                ));
            }
            if !is_ability && definition.effects != stack_object.effects {
                return Err(RulesError::IllegalAction(
                    "stack effects do not match the card definition",
                ));
            }
            if let Some(ability_id) = stack_object.ability_id {
                let activated = self
                    .activated_abilities
                    .get(definition.id)
                    .and_then(|abilities| abilities.get(ability_id));
                let triggered = self
                    .triggered_abilities
                    .get(definition.id)
                    .and_then(|abilities| abilities.get(ability_id));
                let (effects, target_count, trigger_condition) = activated
                    .map(|ability| (&ability.effects, ability.targets.len(), None))
                    .or_else(|| {
                        triggered.map(|ability| {
                            (
                                &ability.effects,
                                ability.targets.len(),
                                Some(ability.condition),
                            )
                        })
                    })
                    .ok_or(RulesError::IllegalAction(
                        "stack ability is not bound to its source definition",
                    ))?;
                let effects_match = if matches!(
                    trigger_condition,
                    Some(TriggerCondition::DealsDamage | TriggerCondition::ReceivesDamage)
                ) {
                    effects.len() == stack_object.effects.len()
                        && effects
                            .iter()
                            .zip(&stack_object.effects)
                            .all(|(bound, actual)| {
                                matches!(
                                    (bound, actual),
                                    (
                                        Effect::GainLifeControllerFromSourceDamage,
                                        Effect::GainLifeController { amount }
                                    ) if *amount > 0
                                ) || matches!(
                                    (bound, actual),
                                    (
                                        Effect::DealDamageToEachPlayerFromReceivedDamage,
                                        Effect::DealDamageToEachPlayer { amount }
                                    ) if *amount > 0
                                ) || bound == actual
                            })
                } else {
                    *effects == stack_object.effects
                };
                if !effects_match || target_count != stack_object.target_count() {
                    return Err(RulesError::IllegalAction(
                        "stack ability does not match its bound definition",
                    ));
                }
            }
            if !is_ability
                && definition
                    .effects
                    .iter()
                    .any(Effect::requires_explicit_mana_spend)
                && stack_object.mana_spent.as_ref().is_none_or(Vec::is_empty)
            {
                return Err(RulesError::IllegalAction(
                    "a spent-mana conditional stack spell lacks its payment receipt",
                ));
            }
            if let Some(colors) = &stack_object.mana_spent
                && let Some(cast_index) = self.event_log.iter().rposition(|event| {
                    matches!(event, GameEvent::SpellCast { card, .. } if *card == stack_object.card)
                })
                && !matches!(
                    self.event_log.get(cast_index.saturating_sub(1)),
                    Some(GameEvent::SpellManaPaid { player, card, colors: receipt_colors })
                        if *player == stack_object.controller
                            && *card == stack_object.card
                            && receipt_colors == colors
                )
            {
                return Err(RulesError::IllegalAction(
                    "stack payment receipt disagrees with its visible cast receipt",
                ));
            }
            // Each target requirement records one distinct occurrence of the
            // word "target" in the executable effect model. A target may be
            // selected again for a later occurrence, but it still occupies a
            // separate slot on the authoritative stack object.
            let target_count = stack_object.target_count();
            if stack_object.targets.len() != target_count {
                return Err(RulesError::IllegalAction(
                    "stack object has an invalid target count",
                ));
            }
            // A target may become illegal after a legal cast (for example, a
            // player can lose or a permanent can leave the battlefield), but
            // it cannot change its enum kind. Validate only immutable target
            // shape here; dynamic legality remains the resolution rule.
            for (target, requirement) in stack_object.targets.iter().zip(
                definition
                    .effects
                    .iter()
                    .filter_map(Effect::target_requirement),
            ) {
                if !Self::target_shape_matches(*target, requirement) {
                    return Err(RulesError::IllegalTarget(*target));
                }
                if let Target::Player(player) = target {
                    self.player(*player)?;
                }
                if let Target::Permanent(card) = target
                    && (card.0 == 0 || card.0 >= self.next_object_id)
                {
                    // Unlike current legality, cast-time target provenance
                    // survives a later zone change or player departure. An
                    // object id outside the monotonic allocation range,
                    // however, could never have named a permanent when this
                    // spell was cast.
                    return Err(RulesError::IllegalTarget(*target));
                }
                if let Target::Spell(card) = target {
                    let target_definition = self
                        .object(*card)
                        .ok()
                        .and_then(|object| object.definition)
                        .or_else(|| self.departed_card_definitions.get(card).copied())
                        .and_then(|definition| self.catalog.get(definition));
                    if !target_definition.is_some_and(|definition| {
                        definition.card_types.contains(&CardType::Instant)
                            || definition.card_types.contains(&CardType::Sorcery)
                    }) {
                        return Err(RulesError::IllegalTarget(*target));
                    }
                    if self
                        .stack
                        .iter()
                        .position(|candidate| candidate.card == *card)
                        .is_some_and(|target_index| target_index >= stack_index)
                    {
                        return Err(RulesError::IllegalAction(
                            "a stack spell target must be lower than its source",
                        ));
                    }
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
        for redirect in &self.damage_redirections {
            self.object(redirect.protected)?;
            if redirect.remaining <= 0 || redirect.expires_turn < self.turn {
                return Err(RulesError::IllegalAction(
                    "damage redirection has invalid remaining amount or lifetime",
                ));
            }
            if !matches!(
                redirect.destination,
                Target::Player(_) | Target::Permanent(_)
            ) {
                return Err(RulesError::IllegalAction(
                    "damage redirection destination has invalid target shape",
                ));
            }
        }
        if let Some(pending) = &self.pending_damage_redirection {
            self.object(pending.source)?;
            self.object(pending.protected)?;
            if pending.remaining <= 0 {
                return Err(RulesError::IllegalAction(
                    "pending damage redirection has nonpositive amount",
                ));
            }
        }
        if let Some(combat) = &self.combat {
            if !matches!(
                self.step,
                Step::DeclareAttackers
                    | Step::DeclareBlockers
                    | Step::FirstStrikeCombatDamage
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
            if !combat.attackers_declared
                && (!combat.hasty_attackers.is_empty()
                    || !combat.flying_attackers.is_empty()
                    || !combat.fear_attackers.is_empty()
                    || !combat.black_evasion_attackers.is_empty()
                    || !combat.vigilant_attackers.is_empty()
                    || !combat.trampling_attackers.is_empty()
                    || !combat.must_be_blocked_attackers.is_empty()
                    || !combat.mountainwalk_attackers.is_empty())
            {
                return Err(RulesError::IllegalAction(
                    "undeclared combat retained attacker keyword provenance",
                ));
            }
            if matches!(
                self.step,
                Step::FirstStrikeCombatDamage | Step::CombatDamage
            ) && (!combat.attackers_declared || !combat.blockers_declared)
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
                if self.zone_of(*attacker) == Some(Zone::Battlefield) {
                    let object = self.object(*attacker)?;
                    if object.entered_turn >= self.turn
                        && !combat.hasty_attackers.contains(attacker)
                    {
                        return Err(RulesError::IllegalAction(
                            "same-turn attacker lacks haste declaration provenance",
                        ));
                    }
                    let currently_vigilant = self
                        .characteristics(*attacker)?
                        .keywords
                        .contains(&Keyword::Vigilance);
                    if currently_vigilant != combat.vigilant_attackers.contains(attacker) {
                        return Err(RulesError::IllegalAction(
                            "vigilance declaration provenance disagrees with attacker keyword",
                        ));
                    }
                }
            }
            if !combat.vigilant_attackers.is_subset(&attackers) {
                return Err(RulesError::IllegalAction(
                    "vigilance declaration provenance contains a nonattacker",
                ));
            }
            if !combat.hasty_attackers.is_subset(&attackers) {
                return Err(RulesError::IllegalAction(
                    "haste declaration provenance contains a nonattacker",
                ));
            }
            if !combat.flying_attackers.is_subset(&attackers) {
                return Err(RulesError::IllegalAction(
                    "flying declaration provenance contains a nonattacker",
                ));
            }
            if !combat.fear_attackers.is_subset(&attackers) {
                return Err(RulesError::IllegalAction(
                    "Fear declaration provenance contains a nonattacker",
                ));
            }
            if !combat.black_evasion_attackers.is_subset(&attackers) {
                return Err(RulesError::IllegalAction(
                    "black-only evasion declaration provenance contains a nonattacker",
                ));
            }
            if !combat.trampling_attackers.is_subset(&attackers) {
                return Err(RulesError::IllegalAction(
                    "trample declaration provenance contains a nonattacker",
                ));
            }
            if !combat.must_be_blocked_attackers.is_subset(&attackers) {
                return Err(RulesError::IllegalAction(
                    "must-block declaration provenance contains a nonattacker",
                ));
            }
            if !combat.mountainwalk_attackers.is_subset(&attackers) {
                return Err(RulesError::IllegalAction(
                    "mountainwalk declaration provenance contains a nonattacker",
                ));
            }
            for (attacker, blocker) in &combat.blockers {
                if !attackers.contains(attacker) || !blockers.insert(*blocker) {
                    return Err(RulesError::IllegalAction("invalid combat blocker state"));
                }
                if self.zone_of(*blocker).is_some() {
                    self.object(*blocker)?;
                }
            }
            let expected_evasion_blockers = combat
                .blockers
                .iter()
                .filter_map(|(attacker, blocker)| {
                    combat
                        .flying_attackers
                        .contains(attacker)
                        .then_some(*blocker)
                })
                .collect::<BTreeSet<_>>();
            if combat.evasion_qualified_blockers != expected_evasion_blockers {
                return Err(RulesError::IllegalAction(
                    "flying blocker declaration provenance is incoherent",
                ));
            }
            let expected_fear_blockers = combat
                .blockers
                .iter()
                .filter_map(|(attacker, blocker)| {
                    combat.fear_attackers.contains(attacker).then_some(*blocker)
                })
                .collect::<BTreeSet<_>>();
            if combat.fear_qualified_blockers != expected_fear_blockers {
                return Err(RulesError::IllegalAction(
                    "Fear blocker declaration provenance is incoherent",
                ));
            }
            let expected_black_evasion_blockers = combat
                .blockers
                .iter()
                .filter_map(|(attacker, blocker)| {
                    combat
                        .black_evasion_attackers
                        .contains(attacker)
                        .then_some(*blocker)
                })
                .collect::<BTreeSet<_>>();
            if combat.black_evasion_qualified_blockers != expected_black_evasion_blockers {
                return Err(RulesError::IllegalAction(
                    "black-only evasion blocker declaration provenance is incoherent",
                ));
            }
            let combatants = attackers.union(&blockers).copied().collect::<BTreeSet<_>>();
            if !combat.first_strike_damage_sources.is_subset(&combatants) {
                return Err(RulesError::IllegalAction(
                    "first-strike damage source is not a combat participant",
                ));
            }
            if self.step == Step::FirstStrikeCombatDamage
                && combat.first_strike_damage_sources.is_empty()
            {
                return Err(RulesError::IllegalAction(
                    "first-strike combat damage lacks recorded sources",
                ));
            }
        } else if matches!(
            self.step,
            Step::DeclareAttackers
                | Step::DeclareBlockers
                | Step::FirstStrikeCombatDamage
                | Step::CombatDamage
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
        mana_payment_selection: Option<&ManaPaymentSelection>,
    ) -> Result<(crate::ManaPool, Option<Vec<Color>>), RulesError> {
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
                    if let Some(position) = remaining
                        .colored
                        .iter()
                        .position(|required| *required == color)
                    {
                        remaining.colored.remove(position);
                    } else if let Some(position) = remaining
                        .hybrid
                        .iter()
                        .position(|required| required.first == color || required.second == color)
                    {
                        remaining.hybrid.remove(position);
                    } else {
                        return Err(RulesError::IllegalAction(
                            "that colored or hybrid mana does not remain to convoke",
                        ));
                    }
                }
            }
        }
        let mut pool = self.players[player.0].mana_pool.clone();
        let mana_spent = if let Some(selection) = mana_payment_selection {
            Some(
                pool.pay_selected(&remaining, selection)
                    .map_err(RulesError::Mana)?,
            )
        } else {
            pool.pay(&remaining).map_err(RulesError::Mana)?;
            None
        };
        let _ = card;
        Ok((pool, mana_spent))
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
        if targets.len() != requirements.len() {
            return Err(RulesError::IllegalAction(
                "the supplied targets do not match the spell's target occurrences",
            ));
        }
        for (target, requirement) in targets.iter().zip(requirements) {
            if !self.target_matches(*target, requirement) {
                return Err(RulesError::IllegalTarget(*target));
            }
        }
        Ok(())
    }

    /// Separates effect targets from the explicitly selected additional-cost
    /// permanents carried in a cast request. Cost selections use their own
    /// `Target` variant solely to preserve the established compact request
    /// shape; they are never copied onto the resulting stack object.
    fn split_cast_targets(
        &self,
        definition: &CardDefinition,
        selections: &[Target],
    ) -> Result<(Vec<Target>, Vec<Target>), RulesError> {
        let target_count = definition
            .effects
            .iter()
            .filter(|effect| effect.target_requirement().is_some())
            .count();
        let cost_count = self
            .additional_spell_costs
            .get(definition.id)
            .map_or(0, Vec::len);
        if cost_count == 0 {
            // Preserve the ordinary target-validation path for spells without
            // expansion-bound additional costs. This keeps a spurious target
            // on a target-free spell classified as a target error rather than
            // exposing the internal cost-selection partitioning.
            return Ok((selections.to_vec(), Vec::new()));
        }
        if selections.len() != target_count + cost_count {
            return Err(RulesError::IllegalAction(
                "the supplied targets and additional-cost selections do not match this spell",
            ));
        }
        Ok((
            selections[..target_count].to_vec(),
            selections[target_count..].to_vec(),
        ))
    }

    /// Validates the concrete choices for expansion-bound additional spell
    /// costs before mana, convoke taps, zones, stack, or event log mutate.
    fn validate_additional_spell_cost_selections(
        &self,
        definition: &CardDefinition,
        player: PlayerId,
        selections: &[Target],
    ) -> Result<(), RulesError> {
        let costs = self
            .additional_spell_costs
            .get(definition.id)
            .map_or(&[][..], Vec::as_slice);
        if costs.len() != selections.len() {
            return Err(RulesError::IllegalAction(
                "additional spell cost selection count does not match its definition",
            ));
        }
        let mut used_permanents = BTreeSet::new();
        for (cost, selection) in costs.iter().zip(selections) {
            match (cost, selection) {
                (
                    AdditionalSpellCost::SacrificeControlledCreature,
                    Target::SacrificePermanent(card),
                ) => {
                    if !used_permanents.insert(*card) {
                        return Err(RulesError::IllegalAction(
                            "the same permanent cannot pay two additional spell costs",
                        ));
                    }
                    if self.zone_of(*card) != Some(Zone::Battlefield)
                        || self.object(*card)?.controller != player
                        || !self
                            .characteristics(*card)?
                            .card_types
                            .contains(&CardType::Creature)
                    {
                        return Err(RulesError::IllegalAction(
                            "additional sacrifice cost requires a controlled battlefield creature",
                        ));
                    }
                }
                (AdditionalSpellCost::SacrificeControlledCreature, target) => {
                    return Err(RulesError::IllegalTarget(*target));
                }
            }
        }
        Ok(())
    }

    /// Pays validated nonmana additional costs in request order. This runs
    /// after mana/convoke payment so a creature may legally convoke and then
    /// be sacrificed, but before the spell card leaves hand for the stack.
    fn pay_additional_spell_costs(
        &mut self,
        definition: &CardDefinition,
        player: PlayerId,
        spell: ObjectId,
        selections: &[Target],
    ) -> Result<(), RulesError> {
        let costs = self
            .additional_spell_costs
            .get(definition.id)
            .cloned()
            .unwrap_or_default();
        for (cost, selection) in costs.iter().zip(selections) {
            match (cost, selection) {
                (
                    AdditionalSpellCost::SacrificeControlledCreature,
                    Target::SacrificePermanent(card),
                ) => {
                    self.record_event(GameEvent::SacrificedAsAdditionalSpellCost {
                        player,
                        card: spell,
                        permanent: *card,
                    });
                    self.move_to_graveyard_or_remove_token(*card)?;
                }
                (AdditionalSpellCost::SacrificeControlledCreature, target) => {
                    return Err(RulesError::IllegalTarget(*target));
                }
            }
        }
        Ok(())
    }

    /// Reject malformed executable effect data before any casting cost, zone,
    /// stack, or event transition can be committed.  Printed modifiers may be
    /// negative, but the currently modelled damage and life-gain operations
    /// are positive quantities.
    fn validate_cast_effects(definition: &CardDefinition) -> Result<(), RulesError> {
        for effect in &definition.effects {
            let amount = match effect {
                Effect::DealDamage { amount, .. }
                | Effect::DealDamageController { amount }
                | Effect::DealDamageAfterOptionalManaPayment { amount, .. }
                | Effect::DealDamageToEachCreatureAndPlayer { amount }
                | Effect::DealDamageToEachPlayer { amount }
                | Effect::DealDamageToEachNonFlyingCreature { amount }
                | Effect::RadianceDealDamageToCreatures { amount }
                | Effect::BeginDamageRedirection { amount }
                | Effect::GainLifeController { amount } => *amount,
                Effect::AddManaController { amount, .. } => i16::from(*amount),
                Effect::CreateToken { .. }
                | Effect::CreateTokenForTargetPlayer { .. }
                | Effect::CompleteDamageRedirection
                | Effect::DrawControllerIfManaColorSpent { .. }
                | Effect::DrawController
                | Effect::GainLifeControllerFromSourceDamage
                | Effect::DealDamageToEachPlayerFromReceivedDamage
                | Effect::DealDamageEqualToAttackingCreatures { .. }
                | Effect::ModifyTargetPtUntilEndOfTurn { .. }
                | Effect::ModifyTargetKeywordUntilEndOfTurn { .. }
                | Effect::PreventTargetBlockingSourceUntilEndOfTurn
                | Effect::ModifySourcePtUntilEndOfTurn { .. }
                | Effect::RemoveSourceKeywordUntilEndOfTurn { .. }
                | Effect::AddSourceDamageShieldUntilEndOfTurn { .. }
                | Effect::AddKeywordToControllerCreaturesUntilEndOfTurn { .. }
                | Effect::DestroyTargetLand
                | Effect::DestroyTargetArtifact
                | Effect::DestroyTargetArtifactOrEnchantment
                | Effect::ReturnTargetCardToHand
                | Effect::ModifyControllerCreaturesPtUntilEndOfTurn { .. }
                | Effect::RadianceUntapAndModifyUntilEndOfTurn { .. }
                | Effect::RadianceModifyPtUntilEndOfTurn { .. }
                | Effect::RadianceAddKeywordUntilEndOfTurn { .. }
                | Effect::CounterTargetInstantOrSorcerySpell
                | Effect::ExileTargetCreature
                | Effect::ExileTargetPermanent => continue,
            };
            if amount <= 0 {
                return Err(RulesError::IllegalAction(
                    "damage and life-gain effect amounts must be positive",
                ));
            }
        }
        Ok(())
    }

    /// Reject an otherwise legal cast before any costs or zones mutate when a
    /// dynamic effect cannot fit the public event representation. Combat's
    /// attacker list is fixed after declaration in this engine slice, so the
    /// checked value cannot grow between this cast boundary and resolution.
    fn validate_effect_capacity(
        &self,
        definition: &CardDefinition,
        controller: PlayerId,
    ) -> Result<(), RulesError> {
        if definition
            .effects
            .iter()
            .any(|effect| matches!(effect, Effect::DealDamageEqualToAttackingCreatures { .. }))
        {
            let _ = self.attacking_creature_count(controller)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)] // Spell and activated-ability resolution share one audited path.
    fn resolve_top_of_stack(&mut self) -> Result<(), RulesError> {
        let stack_object = self.stack.pop().ok_or(RulesError::IllegalAction(
            "attempted to resolve an empty stack",
        ))?;
        // Target legality is snapshotted once, per target occurrence, before
        // any instruction resolves to establish the all-illegal boundary. An
        // initially legal slot is rechecked before its own instruction: an
        // earlier instruction can legally remove a later repeated target.
        // The model-owned plan preserves repeated targets as independent slots.
        let plan = stack_object
            .resolution_plan(|target, requirement| {
                self.target_matches_for_controller(stack_object.controller, target, requirement)
            })
            .map_err(|_| RulesError::IllegalAction("stack object has an invalid target count"))?;
        if matches!(plan, StackResolutionPlan::CounteredByRules) {
            if let Some(ability) = stack_object.ability_id {
                self.record_event(GameEvent::AbilityCounteredByRules {
                    source: stack_object.card,
                    ability,
                });
            } else {
                self.record_event(GameEvent::SpellCounteredByRules {
                    card: stack_object.card,
                });
                self.move_to_graveyard_or_remove_token(stack_object.card)?;
            }
            self.check_state_based_actions()?;
            self.flush_pending_dies_triggers();
            self.priority = self.priority_after_resolution();
            return Ok(());
        }
        let StackResolutionPlan::Resolve {
            effects: effect_resolutions,
        } = plan
        else {
            unreachable!("rules-counter plan returned above");
        };
        let mut life_gain_payment_paid = true;
        if let Some(ability_id) = stack_object.ability_id
            && let Some(ability) = self
                .triggered_abilities
                .get(self.card_definition(stack_object.card)?.id)
                .and_then(|abilities| abilities.get(ability_id))
            && ability.condition == TriggerCondition::LifeGained
            && ability.mana_cost.mana_value() > 0
        {
            let mut paid_pool = self.players[stack_object.controller.0].mana_pool.clone();
            match paid_pool.pay(&ability.mana_cost) {
                Ok(()) => {
                    self.players[stack_object.controller.0].mana_pool = paid_pool;
                    self.record_event(GameEvent::AbilityManaPaid {
                        player: stack_object.controller,
                        source: stack_object.card,
                        ability: ability.id,
                        mana_cost: ability.mana_cost.clone(),
                    });
                }
                Err(_error) if ability.optional => {
                    life_gain_payment_paid = false;
                }
                Err(error) => return Err(RulesError::Mana(error)),
            }
        }
        for (effect_index, (effect, target_resolution)) in stack_object
            .effects
            .iter()
            .zip(effect_resolutions)
            .enumerate()
        {
            if !life_gain_payment_paid {
                continue;
            }
            match target_resolution {
                StackEffectResolution::Untargeted => {
                    self.resolve_effect(
                        stack_object.card,
                        stack_object.controller,
                        stack_object.mana_spent.as_deref(),
                        effect,
                        None,
                    )?;
                }
                StackEffectResolution::Targeted {
                    target,
                    legal: true,
                } => {
                    let requirement =
                        effect
                            .target_requirement()
                            .ok_or(RulesError::IllegalAction(
                                "target-resolution plan named an untargeted effect",
                            ))?;
                    if self.target_matches(target, requirement) {
                        self.resolve_effect(
                            stack_object.card,
                            stack_object.controller,
                            stack_object.mana_spent.as_deref(),
                            effect,
                            Some(target),
                        )?;
                    } else {
                        self.record_event(GameEvent::TargetInstructionSkipped {
                            card: stack_object.card,
                            effect_index,
                            target,
                        });
                    }
                }
                StackEffectResolution::Targeted {
                    target,
                    legal: false,
                } => {
                    // A remaining legal target lets the spell resolve, but an
                    // instruction addressed to a target that has since become
                    // illegal does nothing.
                    self.record_event(GameEvent::TargetInstructionSkipped {
                        card: stack_object.card,
                        effect_index,
                        target,
                    });
                }
            }
        }
        // A malformed or countered two-target redirection ability must not
        // leave a half-built replacement effect behind for a later action.
        self.pending_damage_redirection = None;
        if let Some(ability) = stack_object.ability_id {
            self.record_event(GameEvent::AbilityResolved {
                source: stack_object.card,
                ability,
            });
            self.check_state_based_actions()?;
            self.flush_pending_damage_triggers();
            self.flush_pending_life_gain_triggers();
            self.flush_pending_dies_triggers();
            self.priority = self.priority_after_resolution();
            return Ok(());
        }
        if !self.objects.contains_key(&stack_object.card) {
            // CR 800.4a cleanup can remove the owner and this already-popped
            // source while an instruction resolves (for example, an empty
            // library draw). `ObjectLeftGame` is then the source's terminal
            // stack lifecycle receipt; do not manufacture a later resolution
            // or zone-move receipt by dereferencing the removed object.
            self.priority = self.priority_after_resolution();
            return Ok(());
        }
        self.record_event(GameEvent::SpellResolved {
            card: stack_object.card,
        });
        let definition_id = self.card_definition(stack_object.card)?.id;
        let permanent_resolution = self.card_definition(stack_object.card)?.is_permanent();
        let entering_controller = self.object(stack_object.card)?.controller;
        if permanent_resolution {
            self.move_to_zone(stack_object.card, Zone::Battlefield)?;
        } else {
            self.move_to_graveyard_or_remove_token(stack_object.card)?;
        }
        self.check_state_based_actions()?;
        self.flush_pending_dies_triggers();
        if permanent_resolution {
            self.enqueue_enter_triggers(stack_object.card, definition_id, entering_controller);
        }
        self.flush_pending_damage_triggers();
        self.flush_pending_life_gain_triggers();
        self.flush_pending_dies_triggers();
        self.priority = self.priority_after_resolution();
        Ok(())
    }

    fn enqueue_enter_triggers(
        &mut self,
        source: ObjectId,
        definition: &'static str,
        controller: PlayerId,
    ) {
        let triggers = self
            .triggered_abilities
            .get(definition)
            .into_iter()
            .flat_map(|abilities| abilities.values())
            .filter(|ability| ability.condition == TriggerCondition::EntersBattlefield)
            .cloned()
            .collect::<Vec<_>>();
        for ability in triggers {
            let Some(targets) = self.select_trigger_targets(controller, &ability.targets) else {
                // A mandatory trigger with no legal target is not stackable;
                // an optional one simply does not trigger.  Current RAV ETB
                // bindings use an opponent-first deterministic selector until
                // policy-submitted triggered choices are exposed.
                continue;
            };
            self.stack.push(StackObject {
                card: source,
                controller,
                ability_id: Some(ability.id),
                targets,
                effects: ability.effects,
                mana_spent: None,
            });
            self.record_event(GameEvent::TriggeredAbilityStacked {
                controller,
                source,
                ability: ability.id,
            });
        }
        if !self.stack.is_empty() {
            self.consecutive_passes = 0;
        }
    }

    fn select_trigger_targets(
        &self,
        controller: PlayerId,
        requirements: &[TargetRequirement],
    ) -> Option<Vec<Target>> {
        let mut targets = Vec::with_capacity(requirements.len());
        for requirement in requirements {
            let opponent_player = (0..self.players.len())
                .map(PlayerId)
                .find(|player| *player != controller && !self.players[player.0].lost)
                .map(Target::Player)
                .filter(|target| self.target_matches(*target, *requirement));
            let opponent_permanent = self
                .objects
                .keys()
                .copied()
                .filter(|candidate| self.zone_of(*candidate) == Some(Zone::Battlefield))
                .filter(|candidate| {
                    self.object(*candidate)
                        .is_ok_and(|object| object.controller != controller)
                })
                .map(Target::Permanent)
                .find(|target| self.target_matches(*target, *requirement));
            let any_permanent = self
                .objects
                .keys()
                .copied()
                .filter(|candidate| self.zone_of(*candidate) == Some(Zone::Battlefield))
                .map(Target::Permanent)
                .find(|target| self.target_matches(*target, *requirement));
            let any_player = (0..self.players.len())
                .map(PlayerId)
                .filter(|player| !self.players[player.0].lost)
                .map(Target::Player)
                .find(|target| self.target_matches(*target, *requirement));
            let target = opponent_player
                .or(opponent_permanent)
                .or(any_permanent)
                .or(any_player)?;
            targets.push(target);
        }
        Some(targets)
    }

    /// Stacks attack triggers after attacker declaration. Optional mana is
    /// paid from the controller's pool before the trigger receipt; target
    /// slots are selected from currently legal permanents in a deterministic
    /// opponent-first order so the submitted declaration remains atomic.
    fn enqueue_attack_triggers(&mut self, source: ObjectId) -> Result<(), RulesError> {
        // Tokens have no catalog definition and therefore cannot have a
        // definition-bound attack trigger in this substrate. Their attack is
        // still fully legal; simply skip the definition lookup and continue
        // through the ordinary post-declaration priority transition.
        if self.object(source)?.token.is_some() {
            return Ok(());
        }
        let definition = self.card_definition(source)?.id;
        let controller = self.object(source)?.controller;
        let triggers = self
            .triggered_abilities
            .get(definition)
            .into_iter()
            .flat_map(|abilities| abilities.values())
            .filter(|ability| ability.condition == TriggerCondition::Attacks)
            .cloned()
            .collect::<Vec<_>>();
        for ability in triggers {
            let mut targets = Vec::with_capacity(ability.targets.len());
            for requirement in &ability.targets {
                let target = self
                    .objects
                    .keys()
                    .copied()
                    .filter(|candidate| self.zone_of(*candidate) == Some(Zone::Battlefield))
                    .filter(|candidate| {
                        self.object(*candidate)
                            .is_ok_and(|object| object.controller != controller)
                    })
                    .map(Target::Permanent)
                    .find(|target| self.target_matches(*target, *requirement))
                    .or_else(|| {
                        self.objects
                            .keys()
                            .copied()
                            .filter(|candidate| self.zone_of(*candidate) == Some(Zone::Battlefield))
                            .map(Target::Permanent)
                            .find(|target| self.target_matches(*target, *requirement))
                    });
                let Some(target) = target else {
                    if ability.optional {
                        targets.clear();
                        break;
                    }
                    return Err(RulesError::IllegalAction(
                        "mandatory attack trigger has no legal target",
                    ));
                };
                targets.push(target);
            }
            if targets.len() != ability.targets.len() {
                continue;
            }
            let mut paid_pool = self.players[controller.0].mana_pool.clone();
            if let Err(error) = paid_pool.pay(&ability.mana_cost) {
                if ability.optional {
                    continue;
                }
                return Err(RulesError::Mana(error));
            }
            self.players[controller.0].mana_pool = paid_pool;
            if ability.mana_cost.mana_value() > 0 {
                self.record_event(GameEvent::AbilityManaPaid {
                    player: controller,
                    source,
                    ability: ability.id,
                    mana_cost: ability.mana_cost.clone(),
                });
            }
            self.stack.push(StackObject {
                card: source,
                controller,
                ability_id: Some(ability.id),
                targets,
                effects: ability.effects,
                mana_spent: None,
            });
            self.record_event(GameEvent::TriggeredAbilityStacked {
                controller,
                source,
                ability: ability.id,
            });
        }
        if !self.stack.is_empty() {
            self.consecutive_passes = 0;
        }
        Ok(())
    }

    /// Queues source-specific damage triggers after a positive damage receipt.
    /// The source and its definition are captured before state-based actions,
    /// so a creature that deals and receives lethal damage in the same damage
    /// batch still creates the pending trigger record that Magic requires.
    /// Dynamic source-damage life gain is materialized when the enclosing
    /// damage batch finishes, not recomputed at later ability resolution.
    fn enqueue_damage_triggers(&mut self, source: ObjectId, amount: i32) -> Result<(), RulesError> {
        if amount <= 0
            || self.zone_of(source) != Some(Zone::Battlefield)
            || self.object(source)?.token.is_some()
        {
            return Ok(());
        }
        let definition = self.card_definition(source)?.id;
        let controller = self.object(source)?.controller;
        let triggers = self
            .triggered_abilities
            .get(definition)
            .into_iter()
            .flat_map(|abilities| abilities.values())
            .filter(|ability| ability.condition == TriggerCondition::DealsDamage)
            .cloned()
            .collect::<Vec<_>>();
        if triggers.is_empty() {
            return Ok(());
        }
        let damage_amount = i16::try_from(amount).map_err(|_| {
            RulesError::IllegalAction("damage-trigger life gain exceeds effect representation")
        })?;
        for ability in triggers {
            self.pending_damage_triggers.push(PendingDamageTrigger {
                source,
                controller,
                ability,
                damage_amount,
            });
        }
        Ok(())
    }

    /// Queues triggers caused by a permanent receiving positive damage. This
    /// intentionally does not require the recipient to remain on the
    /// battlefield: state-based actions run after the complete damage batch,
    /// and a dies trigger may need the captured damage amount after the source
    /// has moved to its graveyard.
    fn enqueue_received_damage_triggers(
        &mut self,
        recipient: ObjectId,
        amount: i32,
    ) -> Result<(), RulesError> {
        if amount <= 0 || self.object(recipient)?.token.is_some() {
            return Ok(());
        }
        let definition = self.card_definition(recipient)?.id;
        let controller = self.object(recipient)?.controller;
        let triggers = self
            .triggered_abilities
            .get(definition)
            .into_iter()
            .flat_map(|abilities| abilities.values())
            .filter(|ability| ability.condition == TriggerCondition::ReceivesDamage)
            .cloned()
            .collect::<Vec<_>>();
        if triggers.is_empty() {
            return Ok(());
        }
        let damage_amount = i16::try_from(amount).map_err(|_| {
            RulesError::IllegalAction("received-damage trigger exceeds effect representation")
        })?;
        for ability in triggers {
            self.pending_damage_triggers.push(PendingDamageTrigger {
                source: recipient,
                controller,
                ability,
                damage_amount,
            });
        }
        Ok(())
    }

    /// Queues a source-specific dies trigger after the permanent changes
    /// zones. Unlike damage-batch triggers this is stacked immediately at the
    /// state-based-action boundary, while retaining the dead card object as
    /// the historical source for resolution and event auditing.
    fn enqueue_dies_triggers(&mut self, source: ObjectId, definition: &'static str) {
        self.pending_dies_triggers
            .push(PendingDiesTrigger { source, definition });
    }

    /// Captures triggered abilities controlled by permanents on the
    /// battlefield when a player gains positive life. Their optional mana
    /// costs are paid at trigger resolution; the pending queue only retains
    /// the source and bound ability until the enclosing resolution completes.
    fn enqueue_life_gain_triggers(&mut self) {
        let sources = self
            .all_battlefield_cards()
            .into_iter()
            .filter_map(|source| {
                let definition = self.card_definition(source).ok()?.id;
                let controller = self.object(source).ok()?.controller;
                Some((source, definition, controller))
            })
            .collect::<Vec<_>>();
        for (source, definition, controller) in sources {
            let triggers = self
                .triggered_abilities
                .get(definition)
                .into_iter()
                .flat_map(|abilities| abilities.values())
                .filter(|ability| ability.condition == TriggerCondition::LifeGained)
                .cloned()
                .collect::<Vec<_>>();
            for ability in triggers {
                self.pending_life_gain_triggers
                    .push(PendingLifeGainTrigger {
                        source,
                        controller,
                        ability,
                    });
            }
        }
    }

    /// Places dies triggers above the already-completed action's stack object.
    /// Deferring until costs/effects finish preserves the rule that a trigger
    /// created during an activation cost is stacked after that activation.
    fn flush_pending_dies_triggers(&mut self) {
        let pending = std::mem::take(&mut self.pending_dies_triggers);
        for pending in pending {
            let controller = self
                .object(pending.source)
                .map(|object| object.controller)
                .expect("dies source retained");
            let definition = pending.definition;
            let triggers = self
                .triggered_abilities
                .get(definition)
                .into_iter()
                .flat_map(|abilities| abilities.values())
                .filter(|ability| ability.condition == TriggerCondition::Dies)
                .cloned()
                .collect::<Vec<_>>();
            for ability in triggers {
                self.stack.push(StackObject {
                    card: pending.source,
                    controller,
                    ability_id: Some(ability.id),
                    targets: vec![],
                    effects: ability.effects,
                    mana_spent: None,
                });
                self.record_event(GameEvent::TriggeredAbilityStacked {
                    controller,
                    source: pending.source,
                    ability: ability.id,
                });
            }
        }
        if !self.pending_dies_triggers.is_empty() {
            unreachable!("pending dies triggers were not drained");
        }
        if !self.stack.is_empty() {
            self.consecutive_passes = 0;
        }
    }

    /// Pushes all triggers observed during the just-completed damage batch.
    /// Their effects are materialized from the captured event amount before
    /// the `TriggeredAbilityStacked` receipt is emitted.
    fn flush_pending_damage_triggers(&mut self) {
        let pending = std::mem::take(&mut self.pending_damage_triggers);
        for pending in pending {
            let effects = pending
                .ability
                .effects
                .into_iter()
                .map(|effect| match effect {
                    Effect::GainLifeControllerFromSourceDamage => Effect::GainLifeController {
                        amount: pending.damage_amount,
                    },
                    Effect::DealDamageToEachPlayerFromReceivedDamage => {
                        Effect::DealDamageToEachPlayer {
                            amount: pending.damage_amount,
                        }
                    }
                    effect => effect,
                })
                .collect();
            self.stack.push(StackObject {
                card: pending.source,
                controller: pending.controller,
                ability_id: Some(pending.ability.id),
                targets: vec![],
                effects,
                mana_spent: None,
            });
            self.record_event(GameEvent::TriggeredAbilityStacked {
                controller: pending.controller,
                source: pending.source,
                ability: pending.ability.id,
            });
        }
        if !self.pending_damage_triggers.is_empty() {
            unreachable!("pending damage triggers were not drained");
        }
        if !self.stack.is_empty() {
            self.consecutive_passes = 0;
        }
    }

    /// Puts life-gain triggers on the stack after the enclosing effect has
    /// finished. Target legality is rechecked by the normal stack resolver,
    /// while the trigger's source and controller remain historical identities.
    fn flush_pending_life_gain_triggers(&mut self) {
        let pending = std::mem::take(&mut self.pending_life_gain_triggers);
        for pending in pending {
            self.stack.push(StackObject {
                card: pending.source,
                controller: pending.controller,
                ability_id: Some(pending.ability.id),
                targets: vec![],
                effects: pending.ability.effects,
                mana_spent: None,
            });
            self.record_event(GameEvent::TriggeredAbilityStacked {
                controller: pending.controller,
                source: pending.source,
                ability: pending.ability.id,
            });
        }
        if !self.pending_life_gain_triggers.is_empty() {
            unreachable!("pending life-gain triggers were not drained");
        }
        if !self.stack.is_empty() {
            self.consecutive_passes = 0;
        }
    }

    fn damage_cannot_be_prevented(&self, source: ObjectId) -> bool {
        self.characteristics(source).is_ok_and(|characteristics| {
            characteristics
                .keywords
                .contains(&Keyword::DamageCannotBePrevented)
        })
    }

    fn target_prevents_damage_from_source(&self, source: ObjectId, target: ObjectId) -> bool {
        let Ok(source_characteristics) = self.characteristics(source) else {
            return false;
        };
        self.characteristics(target).is_ok_and(|characteristics| {
            characteristics.keywords.iter().any(|keyword| {
                matches!(
                    keyword,
                    Keyword::PreventDamageFromColor(color)
                        if source_characteristics.colors.contains(color)
                )
            })
        })
    }

    fn deal_damage_to_permanent(
        &mut self,
        source: ObjectId,
        permanent: ObjectId,
        amount: i32,
    ) -> Result<(), RulesError> {
        let redirect_index = self
            .damage_redirections
            .iter()
            .position(|redirect| redirect.protected == permanent && redirect.remaining > 0);
        if let Some(index) = redirect_index {
            let destination = self.damage_redirections[index].destination;
            let destination_is_legal = self
                .target_matches(destination, TargetRequirement::PlayerOrCreature)
                && destination != Target::Permanent(permanent);
            if destination_is_legal {
                let redirected = amount.min(self.damage_redirections[index].remaining);
                self.damage_redirections[index].remaining -= redirected;
                self.record_event(GameEvent::DamageRedirected {
                    source,
                    from: permanent,
                    to: destination,
                    amount: redirected,
                });
                match destination {
                    Target::Player(player) => {
                        self.deal_damage_to_player(source, player, redirected)?;
                    }
                    Target::Permanent(target) => {
                        self.deal_damage_to_permanent(source, target, redirected)?;
                    }
                    Target::Spell(_) | Target::SacrificePermanent(_) => {
                        return Err(RulesError::IllegalTarget(destination));
                    }
                }
                if amount == redirected {
                    if self.damage_redirections[index].remaining == 0 {
                        self.damage_redirections.remove(index);
                    }
                    return Ok(());
                }
                if self.damage_redirections[index].remaining == 0 {
                    self.damage_redirections.remove(index);
                }
                return self.deal_damage_to_permanent(source, permanent, amount - redirected);
            }
            self.damage_redirections.remove(index);
        }
        let (prevented, consumes_shield) = if self.damage_cannot_be_prevented(source) {
            (0, false)
        } else if self.target_prevents_damage_from_source(source, permanent) {
            (amount, false)
        } else {
            let object = self.object(permanent)?;
            (amount.min(object.damage_shield), true)
        };
        if prevented > 0 {
            if consumes_shield {
                self.objects
                    .get_mut(&permanent)
                    .ok_or(RulesError::UnknownCard(permanent))?
                    .damage_shield -= prevented;
            }
            self.record_event(GameEvent::DamagePrevented {
                source,
                target: Target::Permanent(permanent),
                amount: prevented,
            });
        }
        let remaining = amount - prevented;
        if remaining > 0 {
            self.objects
                .get_mut(&permanent)
                .ok_or(RulesError::UnknownCard(permanent))?
                .damage += remaining;
            self.record_event(GameEvent::DamageDealtToPermanent {
                source,
                permanent,
                amount: remaining,
            });
            self.enqueue_damage_triggers(source, remaining)?;
            self.enqueue_received_damage_triggers(permanent, remaining)?;
        }
        Ok(())
    }

    fn deal_damage_to_player(
        &mut self,
        source: ObjectId,
        player: PlayerId,
        amount: i32,
    ) -> Result<(), RulesError> {
        self.players[player.0].life -= i64::from(amount);
        self.record_event(GameEvent::DamageDealtToPlayer {
            source,
            player,
            amount,
        });
        self.enqueue_damage_triggers(source, amount)
    }

    #[allow(clippy::too_many_lines)] // Effect dispatch stays centralized so stack resolution has one rules path.
    fn resolve_effect(
        &mut self,
        source: ObjectId,
        controller: PlayerId,
        mana_spent: Option<&[Color]>,
        effect: &Effect,
        target: Option<Target>,
    ) -> Result<(), RulesError> {
        match effect {
            Effect::DealDamage { amount, .. } => match target
                .ok_or(RulesError::IllegalAction("missing damage target"))?
            {
                Target::Player(player) => {
                    self.deal_damage_to_player(source, player, i32::from(*amount))?;
                }
                Target::Permanent(permanent) => {
                    self.deal_damage_to_permanent(source, permanent, i32::from(*amount))?;
                }
                Target::Spell(card) => return Err(RulesError::IllegalTarget(Target::Spell(card))),
                Target::SacrificePermanent(card) => {
                    return Err(RulesError::IllegalTarget(Target::SacrificePermanent(card)));
                }
            },
            Effect::DealDamageEqualToAttackingCreatures { .. } => {
                let amount = self.attacking_creature_count(controller)?;
                // Magic treats zero damage as no damage. Keeping that boundary
                // explicit avoids inventing a damage receipt or triggering
                // damage-dependent behavior when the combat count is zero.
                if amount != 0 {
                    match target.ok_or(RulesError::IllegalAction("missing damage target"))? {
                        Target::Player(player) => {
                            self.deal_damage_to_player(source, player, amount)?;
                        }
                        Target::Permanent(permanent) => {
                            self.deal_damage_to_permanent(source, permanent, amount)?;
                        }
                        Target::Spell(card) => {
                            return Err(RulesError::IllegalTarget(Target::Spell(card)));
                        }
                        Target::SacrificePermanent(card) => {
                            return Err(RulesError::IllegalTarget(Target::SacrificePermanent(
                                card,
                            )));
                        }
                    }
                }
            }
            Effect::DealDamageController { amount } => {
                self.deal_damage_to_player(source, controller, i32::from(*amount))?;
            }
            Effect::DealDamageAfterOptionalManaPayment { amount, target } => {
                let Some(selected) = self.select_trigger_targets(controller, &[*target]) else {
                    return Ok(());
                };
                let Some(selected) = selected.first().copied() else {
                    return Ok(());
                };
                match selected {
                    Target::Player(player) => {
                        self.deal_damage_to_player(source, player, i32::from(*amount))?;
                    }
                    Target::Permanent(permanent) => {
                        self.deal_damage_to_permanent(source, permanent, i32::from(*amount))?;
                    }
                    Target::Spell(card) => {
                        return Err(RulesError::IllegalTarget(Target::Spell(card)));
                    }
                    Target::SacrificePermanent(card) => {
                        return Err(RulesError::IllegalTarget(Target::SacrificePermanent(card)));
                    }
                }
            }
            Effect::AddManaController { color, amount } => {
                if *amount == 0
                    || !self.players[controller.0]
                        .mana_pool
                        .can_add(*color, *amount)
                {
                    return Err(RulesError::IllegalAction(
                        "spell effect cannot add the requested mana",
                    ));
                }
                self.players[controller.0].mana_pool.add(*color, *amount);
                self.record_event(GameEvent::ManaAdded {
                    player: controller,
                    color: *color,
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
                    self.deal_damage_to_permanent(source, creature, i32::from(*amount))?;
                }
                for player in 0..self.players.len() {
                    if self.players[player].lost {
                        continue;
                    }
                    let player = PlayerId(player);
                    self.deal_damage_to_player(source, player, i32::from(*amount))?;
                }
            }
            Effect::DealDamageToEachPlayer { amount } => {
                for player in 0..self.players.len() {
                    if self.players[player].lost {
                        continue;
                    }
                    self.deal_damage_to_player(source, PlayerId(player), i32::from(*amount))?;
                }
            }
            Effect::DealDamageToEachNonFlyingCreature { amount } => {
                let creatures = self
                    .all_battlefield_cards()
                    .into_iter()
                    .filter(|candidate| {
                        self.characteristics(*candidate)
                            .is_ok_and(|characteristics| {
                                characteristics.card_types.contains(&CardType::Creature)
                                    && !characteristics.keywords.contains(&Keyword::Flying)
                            })
                    })
                    .collect::<Vec<_>>();
                for creature in creatures {
                    self.deal_damage_to_permanent(source, creature, i32::from(*amount))?;
                }
            }
            Effect::RadianceDealDamageToCreatures { amount } => {
                let target = Self::target_permanent(target)?;
                // Select once before mutating damage. State-based actions run
                // after the complete spell resolves, so every selected
                // creature receives this effect's damage in the same batch.
                for candidate in self.radiance_creatures_sharing_color(target)? {
                    self.deal_damage_to_permanent(source, candidate, i32::from(*amount))?;
                }
            }
            Effect::GainLifeController { amount } => {
                self.players[controller.0].life += i64::from(*amount);
                self.record_event(GameEvent::LifeGained {
                    player: controller,
                    amount: *amount,
                });
                self.enqueue_life_gain_triggers();
            }
            Effect::GainLifeControllerFromSourceDamage => {
                return Err(RulesError::IllegalAction(
                    "unmaterialized source-damage life-gain trigger",
                ));
            }
            Effect::DrawController => {
                self.draw_card_from_spell_effect(controller)?;
            }
            Effect::DealDamageToEachPlayerFromReceivedDamage => {
                return Err(RulesError::IllegalAction(
                    "unmaterialized received-damage trigger",
                ));
            }
            Effect::DrawControllerIfManaColorSpent { color } => {
                if mana_spent.is_some_and(|spent| spent.contains(color)) {
                    self.draw_card_from_spell_effect(controller)?;
                }
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
            Effect::CreateTokenForTargetPlayer { token, count } => {
                let target_player = match target {
                    Some(Target::Player(player)) if !self.players[player.0].lost => player,
                    Some(other) => return Err(RulesError::IllegalTarget(other)),
                    None => return Err(RulesError::IllegalAction("missing token-player target")),
                };
                for _ in 0..*count {
                    let token_id = self.create_token(target_player, token.clone())?;
                    self.record_event(GameEvent::TokenCreated {
                        player: target_player,
                        token: token_id,
                    });
                }
            }
            Effect::BeginDamageRedirection { amount } => {
                let protected = Self::target_permanent(target)?;
                if *amount <= 0
                    || self.zone_of(source) != Some(Zone::Battlefield)
                    || self.zone_of(protected) != Some(Zone::Battlefield)
                    || !self
                        .object(protected)
                        .is_ok_and(|object| object.controller == controller)
                {
                    return Err(RulesError::IllegalTarget(Target::Permanent(protected)));
                }
                self.pending_damage_redirection = Some(PendingDamageRedirection {
                    source,
                    protected,
                    remaining: i32::from(*amount),
                });
            }
            Effect::CompleteDamageRedirection => {
                let destination = target.ok_or(RulesError::IllegalAction(
                    "missing damage-redirection destination",
                ))?;
                if !self.target_matches(destination, TargetRequirement::PlayerOrCreature) {
                    return Err(RulesError::IllegalTarget(destination));
                }
                let Some(pending) = self.pending_damage_redirection.take() else {
                    // The protected target may have become illegal after the
                    // first instruction was snapshotted. The remaining
                    // destination instruction then has no work to do.
                    return Ok(());
                };
                self.damage_redirections.push(DamageRedirection {
                    protected: pending.protected,
                    destination,
                    remaining: pending.remaining,
                    expires_turn: self.turn,
                });
            }
            Effect::ModifyTargetPtUntilEndOfTurn { power, toughness } => {
                let target = Self::target_permanent(target)?;
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
            Effect::ModifyTargetKeywordUntilEndOfTurn { keyword } => {
                let target = Self::target_permanent(target)?;
                if self
                    .characteristics(target)?
                    .card_types
                    .contains(&CardType::Creature)
                {
                    self.install_continuous_effect(
                        source,
                        target,
                        ContinuousChange::AddKeyword(keyword.clone()),
                        Duration::EndOfTurn(self.turn),
                    )?;
                }
            }
            Effect::PreventTargetBlockingSourceUntilEndOfTurn => {
                let target = Self::target_permanent(target)?;
                if !self
                    .characteristics(target)?
                    .card_types
                    .contains(&CardType::Creature)
                    || self.zone_of(source) != Some(Zone::Battlefield)
                {
                    return Err(RulesError::IllegalTarget(Target::Permanent(target)));
                }
                self.install_continuous_effect(
                    source,
                    target,
                    ContinuousChange::CannotBlockSource(source),
                    Duration::EndOfTurn(self.turn),
                )?;
            }
            Effect::ModifySourcePtUntilEndOfTurn { power, toughness } => {
                if self.zone_of(source) != Some(Zone::Battlefield) {
                    return Ok(());
                }
                self.install_continuous_effect(
                    source,
                    source,
                    ContinuousChange::ModifyPowerToughness {
                        power: *power,
                        toughness: *toughness,
                    },
                    Duration::EndOfTurn(self.turn),
                )?;
            }
            Effect::RemoveSourceKeywordUntilEndOfTurn { keyword } => {
                if self.zone_of(source) != Some(Zone::Battlefield) {
                    return Ok(());
                }
                self.install_continuous_effect(
                    source,
                    source,
                    ContinuousChange::RemoveKeyword(keyword.clone()),
                    Duration::EndOfTurn(self.turn),
                )?;
            }
            Effect::AddSourceDamageShieldUntilEndOfTurn { amount } => {
                if *amount <= 0 || self.zone_of(source) != Some(Zone::Battlefield) {
                    return Ok(());
                }
                self.install_continuous_effect(
                    source,
                    source,
                    ContinuousChange::AddDamageShield(*amount),
                    Duration::EndOfTurn(self.turn),
                )?;
            }
            Effect::DestroyTargetLand => {
                let target = target.ok_or(RulesError::IllegalAction("missing land target"))?;
                let Target::Permanent(land) = target else {
                    return Err(RulesError::IllegalTarget(target));
                };
                if self.zone_of(land) != Some(Zone::Battlefield)
                    || !self.card_definition(land)?.is_land()
                {
                    return Err(RulesError::IllegalTarget(target));
                }
                self.record_event(GameEvent::CardDestroyed { source, card: land });
                self.move_to_graveyard_or_remove_token(land)?;
            }
            Effect::DestroyTargetArtifact => {
                let target = target.ok_or(RulesError::IllegalAction("missing artifact target"))?;
                let Target::Permanent(artifact) = target else {
                    return Err(RulesError::IllegalTarget(target));
                };
                if self.zone_of(artifact) != Some(Zone::Battlefield)
                    || !self
                        .card_definition(artifact)?
                        .card_types
                        .contains(&CardType::Artifact)
                {
                    return Err(RulesError::IllegalTarget(target));
                }
                self.record_event(GameEvent::CardDestroyed {
                    source,
                    card: artifact,
                });
                self.move_to_graveyard_or_remove_token(artifact)?;
            }
            Effect::ModifyControllerCreaturesPtUntilEndOfTurn { power, toughness } => {
                // Snapshot the affected battlefield objects before installing
                // any effects. State-based actions run only once the complete
                // spell has resolved, so every eligible controller-owned
                // creature receives the same temporary modifier.
                let creatures = self
                    .all_battlefield_cards()
                    .into_iter()
                    .filter(|candidate| {
                        self.object(*candidate)
                            .is_ok_and(|object| object.controller == controller)
                            && self
                                .characteristics(*candidate)
                                .is_ok_and(|characteristics| {
                                    characteristics.card_types.contains(&CardType::Creature)
                                })
                    })
                    .collect::<Vec<_>>();
                for creature in creatures {
                    self.install_continuous_effect(
                        source,
                        creature,
                        ContinuousChange::ModifyPowerToughness {
                            power: *power,
                            toughness: *toughness,
                        },
                        Duration::EndOfTurn(self.turn),
                    )?;
                }
            }
            Effect::AddKeywordToControllerCreaturesUntilEndOfTurn { keyword } => {
                let creatures = self
                    .all_battlefield_cards()
                    .into_iter()
                    .filter(|candidate| {
                        self.object(*candidate).is_ok_and(|object| {
                            object.controller == controller
                                && self
                                    .characteristics(*candidate)
                                    .is_ok_and(|characteristics| {
                                        characteristics.card_types.contains(&CardType::Creature)
                                    })
                        })
                    })
                    .collect::<Vec<_>>();
                for creature in creatures {
                    self.install_continuous_effect(
                        source,
                        creature,
                        ContinuousChange::AddKeyword(keyword.clone()),
                        Duration::EndOfTurn(self.turn),
                    )?;
                }
            }
            Effect::RadianceUntapAndModifyUntilEndOfTurn { power, toughness } => {
                let target = Self::target_permanent(target)?;
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
                let target = Self::target_permanent(target)?;
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
            Effect::RadianceAddKeywordUntilEndOfTurn { keyword } => {
                let target = Self::target_permanent(target)?;
                for candidate in self.radiance_creatures_sharing_color(target)? {
                    self.install_continuous_effect(
                        source,
                        candidate,
                        ContinuousChange::AddKeyword(keyword.clone()),
                        Duration::EndOfTurn(self.turn),
                    )?;
                }
            }
            Effect::CounterTargetInstantOrSorcerySpell => {
                let target = Self::target_spell(target)?;
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
            Effect::ExileTargetCreature => {
                let target = Self::target_permanent(target)?;
                self.move_to_zone(target, Zone::Exile)?;
            }
            Effect::ExileTargetPermanent => {
                let target = Self::target_permanent(target)?;
                self.move_to_zone(target, Zone::Exile)?;
            }
            Effect::DestroyTargetArtifactOrEnchantment => {
                let target = Self::target_permanent(target)?;
                if !self.target_matches(
                    Target::Permanent(target),
                    TargetRequirement::ArtifactOrEnchantment,
                ) {
                    return Err(RulesError::IllegalTarget(Target::Permanent(target)));
                }
                self.record_event(GameEvent::CardDestroyed {
                    source,
                    card: target,
                });
                self.move_to_graveyard_or_remove_token(target)?;
            }
            Effect::ReturnTargetCardToHand => {
                let target = Self::target_permanent(target)?;
                if !self.target_matches_for_controller(
                    controller,
                    Target::Permanent(target),
                    TargetRequirement::OwnGraveyardCard,
                ) {
                    return Err(RulesError::IllegalTarget(Target::Permanent(target)));
                }
                self.move_to_zone(target, Zone::Hand)?;
            }
        }
        Ok(())
    }

    fn target_permanent(target: Option<Target>) -> Result<ObjectId, RulesError> {
        match target {
            Some(Target::Permanent(card)) => Ok(card),
            Some(Target::Player(player)) => Err(RulesError::IllegalTarget(Target::Player(player))),
            Some(Target::Spell(card)) => Err(RulesError::IllegalTarget(Target::Spell(card))),
            Some(Target::SacrificePermanent(card)) => {
                Err(RulesError::IllegalTarget(Target::SacrificePermanent(card)))
            }
            None => Err(RulesError::IllegalAction("missing permanent target")),
        }
    }

    /// Counts the resolving spell controller's creatures that remain attackers
    /// in the current combat. The value is bounded by the engine's signed
    /// damage receipt representation; a synthetically oversized combat fails
    /// closed rather than wrapping into life gain or damage removal.
    fn attacking_creature_count(&self, controller: PlayerId) -> Result<i32, RulesError> {
        let Some(combat) = &self.combat else {
            return Ok(0);
        };
        let count = combat
            .attackers
            .iter()
            .filter(|attacker| {
                self.zone_of(**attacker) == Some(Zone::Battlefield)
                    && self
                        .object(**attacker)
                        .is_ok_and(|object| object.controller == controller)
                    && self
                        .characteristics(**attacker)
                        .is_ok_and(|characteristics| {
                            characteristics.card_types.contains(&CardType::Creature)
                        })
            })
            .count();
        i32::try_from(count).map_err(|_| {
            RulesError::IllegalAction("attacking-creature damage exceeds the engine receipt range")
        })
    }

    fn target_spell(target: Option<Target>) -> Result<ObjectId, RulesError> {
        match target {
            Some(Target::Spell(card)) => Ok(card),
            Some(Target::Player(player)) => Err(RulesError::IllegalTarget(Target::Player(player))),
            Some(Target::Permanent(card)) => {
                Err(RulesError::IllegalTarget(Target::Permanent(card)))
            }
            Some(Target::SacrificePermanent(card)) => {
                Err(RulesError::IllegalTarget(Target::SacrificePermanent(card)))
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
                TargetRequirement::Creature
                | TargetRequirement::PlayerOrCreature
                | TargetRequirement::BlockingCreature
                | TargetRequirement::AttackingOrBlockingCreature,
            ) => {
                self.zone_of(card) == Some(Zone::Battlefield)
                    && self.characteristics(card).is_ok_and(|characteristics| {
                        characteristics.card_types.contains(&CardType::Creature)
                    })
                    && (!matches!(requirement, TargetRequirement::BlockingCreature)
                        || self.combat.as_ref().is_some_and(|combat| {
                            combat.blockers.values().any(|blocker| *blocker == card)
                        }))
                    && (!matches!(requirement, TargetRequirement::AttackingOrBlockingCreature)
                        || self.combat.as_ref().is_some_and(|combat| {
                            combat.attackers.contains(&card)
                                || combat.blockers.values().any(|blocker| *blocker == card)
                        }))
            }
            (Target::Permanent(card), TargetRequirement::Land) => {
                self.zone_of(card) == Some(Zone::Battlefield)
                    && self
                        .card_definition(card)
                        .is_ok_and(CardDefinition::is_land)
            }
            (Target::Permanent(card), TargetRequirement::Artifact) => {
                self.zone_of(card) == Some(Zone::Battlefield)
                    && self
                        .card_definition(card)
                        .is_ok_and(|definition| definition.card_types.contains(&CardType::Artifact))
            }
            (Target::Permanent(card), TargetRequirement::ArtifactOrEnchantment) => {
                self.zone_of(card) == Some(Zone::Battlefield)
                    && self.characteristics(card).is_ok_and(|characteristics| {
                        characteristics.card_types.contains(&CardType::Artifact)
                            || characteristics.card_types.contains(&CardType::Enchantment)
                    })
            }
            (Target::Permanent(card), TargetRequirement::OwnGraveyardCard) => {
                self.zone_of(card) == Some(Zone::Graveyard)
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

    /// Extends target legality with controller-scoped requirements. Keeping
    /// this separate from the general target predicate lets stack validation
    /// retain the original controller even after the source changes zones.
    fn target_matches_for_controller(
        &self,
        controller: PlayerId,
        target: Target,
        requirement: TargetRequirement,
    ) -> bool {
        self.target_matches(target, requirement)
            && match (target, requirement) {
                (Target::Permanent(card), TargetRequirement::OwnGraveyardCard) => self
                    .object(card)
                    .is_ok_and(|object| object.owner == controller),
                _ => true,
            }
    }

    /// Checks the target variant that could have been selected at cast time,
    /// without requiring its current object to remain legal at resolution.
    /// This separates invariant provenance checks from the rules-counter path.
    fn target_shape_matches(target: Target, requirement: TargetRequirement) -> bool {
        matches!(
            (target, requirement),
            (
                Target::Player(_),
                TargetRequirement::Any
                    | TargetRequirement::Player
                    | TargetRequirement::PlayerOrCreature
            ) | (
                Target::Permanent(_),
                TargetRequirement::Any
                    | TargetRequirement::Creature
                    | TargetRequirement::BlockingCreature
                    | TargetRequirement::AttackingOrBlockingCreature
                    | TargetRequirement::Land
                    | TargetRequirement::Artifact
                    | TargetRequirement::ArtifactOrEnchantment
                    | TargetRequirement::OwnGraveyardCard
                    | TargetRequirement::PlayerOrCreature
            ) | (Target::Spell(_), TargetRequirement::InstantOrSorcerySpell)
        )
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
        } else if self.step == Step::DeclareBlockers && !self.combat_has_first_striker()? {
            // First-strike combat damage is an additional step only when at
            // least one attacking or blocking creature can assign there.
            Step::CombatDamage
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
        if self.step == Step::FirstStrikeCombatDamage {
            self.resolve_combat_damage(true)?;
        }
        if self.step == Step::CombatDamage {
            self.resolve_combat_damage(false)?;
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
                self.damage_redirections
                    .retain(|redirect| redirect.expires_turn != self.turn);
                for effect in expired {
                    self.remove_damage_shield_for_effect(&effect);
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
        if self.step == Step::Untap {
            // Untap itself has no priority window. Stabilize the battlefield
            // after its automatic work and before advancing to the first
            // priority-bearing Upkeep state.
            self.check_state_based_actions()?;
        }
        if (self.step == Step::Untap || self.step == Step::Cleanup) && !self.is_game_over() {
            // CR 117.3a and 514.3: neither normal Untap nor this slice's
            // ordinary Cleanup gives priority. Advance immediately.
            self.advance_step()?;
        }
        Ok(())
    }

    fn combat_has_first_striker(&self) -> Result<bool, RulesError> {
        let Some(combat) = &self.combat else {
            return Ok(false);
        };
        for creature in combat
            .attackers
            .iter()
            .copied()
            .chain(combat.blockers.values().copied())
        {
            if self.zone_of(creature) == Some(Zone::Battlefield)
                && self
                    .characteristics(creature)?
                    .keywords
                    .iter()
                    .any(|keyword| matches!(keyword, Keyword::FirstStrike | Keyword::DoubleStrike))
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    #[allow(clippy::too_many_lines)] // Combat damage owns both assignment and SBA receipts.
    fn resolve_combat_damage(&mut self, first_strike: bool) -> Result<(), RulesError> {
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
        let first_strike_sources = if first_strike {
            let sources = combat
                .attackers
                .iter()
                .copied()
                .chain(combat.blockers.values().copied())
                .filter(|creature| {
                    self.zone_of(*creature) == Some(Zone::Battlefield)
                        && self
                            .characteristics(*creature)
                            .is_ok_and(|characteristics| {
                                characteristics.keywords.iter().any(|keyword| {
                                    matches!(keyword, Keyword::FirstStrike | Keyword::DoubleStrike)
                                })
                            })
                })
                .collect::<BTreeSet<_>>();
            if sources.is_empty() {
                return Err(RulesError::IllegalAction(
                    "first-strike damage step lacks a first-strike creature",
                ));
            }
            self.combat
                .as_mut()
                .ok_or(RulesError::IllegalAction(
                    "combat damage without combat state",
                ))?
                .first_strike_damage_sources
                .clone_from(&sources);
            sources
        } else {
            combat.first_strike_damage_sources.clone()
        };
        let eligible = |creature| {
            if self.zone_of(creature) != Some(Zone::Battlefield) {
                return false;
            }
            if first_strike {
                first_strike_sources.contains(&creature)
            } else {
                !first_strike_sources.contains(&creature)
                    || self.characteristics(creature).is_ok_and(|characteristics| {
                        characteristics.keywords.contains(&Keyword::DoubleStrike)
                    })
            }
        };
        let mut permanent_damage = Vec::new();
        let mut player_damage = Vec::new();
        for attacker in combat.attackers {
            if self.zone_of(attacker) != Some(Zone::Battlefield) {
                continue;
            }
            let attacker_eligible = eligible(attacker);
            let attacker_power = self
                .characteristics(attacker)?
                .power
                .ok_or(RulesError::IllegalAction("attacker lacks power"))?;
            let attacker_has_trample = self
                .characteristics(attacker)?
                .keywords
                .contains(&Keyword::Trample);
            if let Some(blocker) = combat.blockers.get(&attacker).copied() {
                if self.zone_of(blocker) != Some(Zone::Battlefield) {
                    // A creature that was blocked remains blocked even if its
                    // blocker leaves combat before damage. A live trample
                    // attacker can assign all of its positive damage to the
                    // defending player; other blocked attackers assign none.
                    if attacker_eligible && attacker_has_trample && attacker_power > 0 {
                        player_damage.push((attacker, defending_player, attacker_power));
                    }
                    continue;
                }
                let blocker_power = self
                    .characteristics(blocker)?
                    .power
                    .ok_or(RulesError::IllegalAction("blocker lacks power"))?;
                if attacker_eligible && attacker_power > 0 {
                    if attacker_has_trample {
                        let blocker_characteristics = self.characteristics(blocker)?;
                        let blocker_toughness = blocker_characteristics
                            .toughness
                            .ok_or(RulesError::IllegalAction("blocker lacks toughness"))?;
                        let marked_damage = self.object(blocker)?.damage;
                        // The initial combat representation admits exactly
                        // one blocker per attacker. With no deathtouch or
                        // damage-prevention substrate, lethal damage is the
                        // blocker's remaining toughness after damage already
                        // marked on it; excess is assigned to the defender.
                        let lethal = blocker_toughness.saturating_sub(marked_damage).max(0);
                        let assigned_to_blocker = attacker_power.min(lethal);
                        if assigned_to_blocker > 0 {
                            permanent_damage.push((attacker, blocker, assigned_to_blocker));
                        }
                        let excess = attacker_power - assigned_to_blocker;
                        if excess > 0 {
                            player_damage.push((attacker, defending_player, excess));
                        }
                    } else {
                        permanent_damage.push((attacker, blocker, attacker_power));
                    }
                }
                if eligible(blocker) && blocker_power > 0 {
                    permanent_damage.push((blocker, attacker, blocker_power));
                }
            } else if attacker_eligible && attacker_power > 0 {
                player_damage.push((attacker, defending_player, attacker_power));
            }
        }
        for (source, permanent, amount) in permanent_damage {
            self.deal_damage_to_permanent(source, permanent, amount)?;
        }
        for (source, player, amount) in player_damage {
            self.deal_damage_to_player(source, player, amount)?;
        }
        self.check_state_based_actions()?;
        self.flush_pending_damage_triggers();
        self.flush_pending_life_gain_triggers();
        self.flush_pending_dies_triggers();
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
                damage_shield: 0,
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
        let was_battlefield = self.zone_of(card) == Some(Zone::Battlefield);
        let definition = self.card_definition(card)?.id;
        self.move_to_zone(card, Zone::Graveyard)?;
        if was_battlefield {
            self.enqueue_dies_triggers(card, definition);
        }
        Ok(())
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
            battlefield_object.damage_shield = 0;
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
        self.damage_redirections
            .retain(|redirect| redirect.protected != card);
        let expired = self
            .continuous_effects
            .iter()
            .filter(|effect| effect.source == card || effect.target == card)
            .cloned()
            .collect::<Vec<_>>();
        self.continuous_effects
            .retain(|effect| effect.source != card && effect.target != card);
        for effect in expired {
            self.remove_damage_shield_for_effect(&effect);
            self.record_event(GameEvent::ContinuousEffectExpired {
                source: effect.source,
                target: effect.target,
                layer: effect.change.layer(),
            });
        }
    }

    fn remove_damage_shield_for_effect(&mut self, effect: &ContinuousEffect) {
        if let ContinuousChange::AddDamageShield(amount) = &effect.change {
            if let Some(object) = self.objects.get_mut(&effect.target) {
                object.damage_shield = object
                    .damage_shield
                    .saturating_sub(i32::from(*amount))
                    .max(0);
            }
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
            Self::validate_mana_cost(mana_cost)?;
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
        if ability.controller_damage == Some(0) {
            return Err(RulesError::IllegalAction(
                "mana ability controller damage must be positive when present",
            ));
        }
        if matches!(&ability.output, ManaAbilityOutput::Choice(colors) if colors.is_empty()) {
            return Err(RulesError::IllegalAction(
                "mana ability color choice must not be empty",
            ));
        }
        Ok(())
    }

    fn validate_activated_ability_definition(ability: &ActivatedAbility) -> Result<(), RulesError> {
        if ability.id.is_empty() {
            return Err(RulesError::IllegalAction(
                "activated ability lacks an identity",
            ));
        }
        Self::validate_mana_cost(&ability.mana_cost)?;
        let effect_targets = ability
            .effects
            .iter()
            .filter_map(Effect::target_requirement)
            .collect::<Vec<_>>();
        if effect_targets != ability.targets {
            return Err(RulesError::IllegalAction(
                "activated ability targets do not match effect order",
            ));
        }
        Self::validate_cast_effects_for_ability(&ability.effects)
    }

    fn validate_cast_effects_for_ability(effects: &[Effect]) -> Result<(), RulesError> {
        for effect in effects {
            let amount = match effect {
                Effect::DealDamage { amount, .. }
                | Effect::DealDamageController { amount }
                | Effect::DealDamageAfterOptionalManaPayment { amount, .. }
                | Effect::DealDamageToEachCreatureAndPlayer { amount }
                | Effect::DealDamageToEachPlayer { amount }
                | Effect::RadianceDealDamageToCreatures { amount }
                | Effect::GainLifeController { amount } => *amount,
                _ => continue,
            };
            if amount <= 0 {
                return Err(RulesError::IllegalAction(
                    "damage and life-gain effect amounts must be positive",
                ));
            }
        }
        Ok(())
    }

    /// Audits the additional-cost receipt boundary. A sacrifice is not an
    /// effect or a target: its selected permanent must change zones before
    /// the spell becomes a stack object, with no intervening priority event.
    fn validate_additional_spell_cost_event_order(&self) -> Result<(), RulesError> {
        for (index, event) in self.event_log.iter().enumerate() {
            let GameEvent::SacrificedAsAdditionalSpellCost {
                player,
                card,
                permanent,
            } = event
            else {
                continue;
            };
            if let Some(definition) = self.definition_for_historical_spell_receipt(index, *card)? {
                if !self
                    .additional_spell_costs
                    .get(definition.id)
                    .is_some_and(|costs| {
                        costs.contains(&AdditionalSpellCost::SacrificeControlledCreature)
                    })
                {
                    return Err(RulesError::IllegalAction(
                        "sacrifice-cost receipt names a spell without that bound cost",
                    ));
                }
            }
            if permanent == card {
                return Err(RulesError::IllegalAction(
                    "a spell card cannot sacrifice itself from hand as its additional cost",
                ));
            }
            if !matches!(
                self.event_log.get(index + 1),
                Some(GameEvent::CardMoved { card: moved, to: Zone::Graveyard }) if moved == permanent
            ) && !matches!(
                self.event_log.get(index + 1),
                Some(GameEvent::TokenCeasedToExist { token }) if token == permanent
            ) {
                return Err(RulesError::IllegalAction(
                    "sacrifice-cost receipt lacks its immediate battlefield departure receipt",
                ));
            }
            let cast_follows = self.event_log[index + 2..]
                .iter()
                .find(|candidate| {
                    matches!(
                        candidate,
                        GameEvent::SpellCast { .. } | GameEvent::PriorityPassed { .. }
                    )
                })
                .is_some_and(|candidate| {
                    matches!(
                        candidate,
                        GameEvent::SpellCast {
                            player: caster,
                            card: spell,
                        } if caster == player && spell == card
                    )
                });
            if !cast_follows {
                return Err(RulesError::IllegalAction(
                    "sacrifice-cost receipt is not followed by its spell cast before priority",
                ));
            }
        }

        for (index, event) in self.event_log.iter().enumerate() {
            let GameEvent::SpellCast { player, card } = event else {
                continue;
            };
            let needs_sacrifice_cost = self
                .definition_for_historical_spell_receipt(index, *card)?
                .is_some_and(|definition| {
                    self.additional_spell_costs
                        .get(definition.id)
                        .is_some_and(|costs| {
                            costs.contains(&AdditionalSpellCost::SacrificeControlledCreature)
                        })
                });
            let has_sacrifice_receipt = self.event_log[..index]
                .iter()
                .rev()
                .take_while(|candidate| {
                    !matches!(
                        candidate,
                        GameEvent::SpellCast { .. } | GameEvent::PriorityPassed { .. }
                    )
                })
                .any(|candidate| {
                    matches!(
                        candidate,
                        GameEvent::SacrificedAsAdditionalSpellCost {
                            player: payer,
                            card: spell,
                            ..
                        } if payer == player && spell == card
                    )
                });
            if needs_sacrifice_cost && !has_sacrifice_receipt {
                return Err(RulesError::IllegalAction(
                    "a spell with a bound sacrifice cost was cast without its cost receipt",
                ));
            }
        }
        Ok(())
    }

    /// Looks up a spell definition while auditing an event history. CR 800.4a
    /// can legitimately remove an owned spell object from `objects` after its
    /// cast receipt was recorded, so a later transition must not fail merely
    /// because the historical receipt can no longer dereference that object.
    ///
    /// The sole tolerated absence is a later `ObjectLeftGame` terminal receipt
    /// for the same card. All other missing objects remain invariant failures.
    fn definition_for_historical_spell_receipt(
        &self,
        receipt_index: usize,
        card: ObjectId,
    ) -> Result<Option<&CardDefinition>, RulesError> {
        match self.card_definition(card) {
            Ok(definition) => Ok(Some(definition)),
            Err(RulesError::UnknownCard(missing))
                if missing == card
                    && self
                        .event_log
                        .get(receipt_index.saturating_add(1)..)
                        .is_some_and(|later_events| {
                            later_events.iter().any(|event| {
                                matches!(event, GameEvent::ObjectLeftGame { object, .. } if *object == card)
                            })
                        }) =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }

    /// Audits the causally significant mana-ability receipt sequences. These
    /// are state-machine invariants rather than UI diagnostics: a replay must
    /// never claim that a cast used a mana ability unless every activation,
    /// output, and enclosing spell-cast receipt occurs in one causal order.
    #[allow(clippy::too_many_lines)] // One ordered replay audit keeps receipt causality reviewable.
    fn validate_mana_ability_event_order(events: &[GameEvent]) -> Result<(), RulesError> {
        let mut index = 0;
        while index < events.len() {
            let Some((player, card)) = Self::cast_payment_context(events.get(index)) else {
                index += 1;
                continue;
            };

            let output_end = match events.get(index) {
                Some(GameEvent::CastPaymentBasicLandManaAbilityActivated {
                    land, color, ..
                }) => {
                    if !matches!(
                        events.get(index + 1),
                        Some(GameEvent::ManaAbilityActivated {
                            player: receipt_player,
                            land: receipt_land,
                            color: receipt_color,
                        }) if *receipt_player == player && receipt_land == land && receipt_color == color
                    ) {
                        return Err(RulesError::IllegalAction(
                            "cast-payment basic-land activation lacks its matching intrinsic receipt",
                        ));
                    }
                    if !matches!(
                        events.get(index + 2),
                        Some(GameEvent::ManaAdded {
                            player: receipt_player,
                            color: receipt_color,
                            amount: 1,
                        }) if *receipt_player == player && receipt_color == color
                    ) {
                        return Err(RulesError::IllegalAction(
                            "cast-payment basic-land activation lacks its matching mana-output receipt",
                        ));
                    }
                    index + 3
                }
                Some(GameEvent::CastPaymentManaAbilityActivated {
                    source, ability, ..
                }) => {
                    let receipt_index = index + 1;
                    let mut output_index = match events.get(receipt_index) {
                        Some(GameEvent::BoundManaAbilityActivated {
                            player: receipt_player,
                            source: receipt_source,
                            ability: receipt_ability,
                            color,
                            amount,
                            life_payment,
                            ..
                        }) if *receipt_player == player
                            && receipt_source == source
                            && receipt_ability == ability =>
                        {
                            let mut output_index = receipt_index + 1;
                            if let Some(life_amount) = life_payment {
                                if !matches!(
                                    events.get(output_index),
                                    Some(GameEvent::ManaAbilityLifePaid {
                                        player: receipt_player,
                                        amount: receipt_amount,
                                    }) if *receipt_player == player && receipt_amount == life_amount
                                ) {
                                    return Err(RulesError::IllegalAction(
                                        "cast-payment mana activation lacks its life-payment receipt",
                                    ));
                                }
                                output_index += 1;
                            }
                            if !matches!(
                                events.get(output_index),
                                Some(GameEvent::ManaAdded {
                                    player: receipt_player,
                                    color: receipt_color,
                                    amount: receipt_amount,
                                }) if *receipt_player == player
                                    && receipt_color == color
                                    && receipt_amount == amount
                            ) {
                                return Err(RulesError::IllegalAction(
                                    "cast-payment mana activation lacks its matching mana-output receipt",
                                ));
                            }
                            output_index + 1
                        }
                        Some(GameEvent::BoundManaAbilityBundleActivated {
                            player: receipt_player,
                            source: receipt_source,
                            ability: receipt_ability,
                            mana_cost,
                            bundle,
                            life_payment,
                            ..
                        }) if *receipt_player == player
                            && receipt_source == source
                            && receipt_ability == ability =>
                        {
                            if !matches!(
                                events.get(receipt_index + 1),
                                Some(GameEvent::ManaAbilityManaPaid {
                                    player: payment_player,
                                    mana_cost: receipt_cost,
                                }) if *payment_player == player && receipt_cost == mana_cost
                            ) {
                                return Err(RulesError::IllegalAction(
                                    "cast-payment paid-bundle activation lacks its payment receipt",
                                ));
                            }
                            let mut output_index = receipt_index + 2;
                            if let Some(life_amount) = life_payment {
                                if !matches!(
                                    events.get(output_index),
                                    Some(GameEvent::ManaAbilityLifePaid {
                                        player: receipt_player,
                                        amount: receipt_amount,
                                    }) if *receipt_player == player && receipt_amount == life_amount
                                ) {
                                    return Err(RulesError::IllegalAction(
                                        "cast-payment paid-bundle activation lacks its life-payment receipt",
                                    ));
                                }
                                output_index += 1;
                            }
                            for (color, amount) in bundle.iter() {
                                if !matches!(
                                    events.get(output_index),
                                    Some(GameEvent::ManaAdded {
                                        player: receipt_player,
                                        color: receipt_color,
                                        amount: receipt_amount,
                                    }) if *receipt_player == player
                                        && receipt_color == &color
                                        && receipt_amount == &amount
                                ) {
                                    return Err(RulesError::IllegalAction(
                                        "cast-payment paid-bundle activation lacks an ordered mana-output receipt",
                                    ));
                                }
                                output_index += 1;
                            }
                            output_index
                        }
                        _ => {
                            return Err(RulesError::IllegalAction(
                                "cast-payment mana activation lacks its matching bound receipt",
                            ));
                        }
                    };
                    if matches!(
                        events.get(output_index),
                        Some(GameEvent::DamageDealtToPlayer {
                            source: receipt_source,
                            player: receipt_player,
                            ..
                        }) if receipt_source == source && *receipt_player == player
                    ) {
                        output_index += 1;
                    }
                    output_index
                }
                _ => unreachable!("payment context helper only accepts payment marker events"),
            };

            let mut cursor = output_end;
            loop {
                match events.get(cursor) {
                    Some(candidate) if Self::cast_payment_context(Some(candidate)).is_some() => {
                        let (next_player, next_card) = Self::cast_payment_context(Some(candidate))
                            .expect("marker was checked");
                        if next_player != player || next_card != card {
                            return Err(RulesError::IllegalAction(
                                "cast-payment receipts changed their enclosing spell before SpellCast",
                            ));
                        }
                        index = cursor;
                        break;
                    }
                    Some(GameEvent::ConvokeUsed {
                        player: receipt_player,
                        ..
                    }) if *receipt_player == player => {
                        cursor += 1;
                    }
                    Some(GameEvent::SpellManaPaid {
                        player: receipt_player,
                        card: receipt_card,
                        ..
                    }) if *receipt_player == player && *receipt_card == card => {
                        cursor += 1;
                    }
                    Some(GameEvent::SpellCast {
                        player: receipt_player,
                        card: receipt_card,
                    }) if *receipt_player == player && *receipt_card == card => {
                        index = cursor + 1;
                        break;
                    }
                    Some(GameEvent::PriorityPassed { .. }) => {
                        return Err(RulesError::IllegalAction(
                            "cast-payment activation is not followed by its spell receipt before priority passes",
                        ));
                    }
                    _ => {
                        return Err(RulesError::IllegalAction(
                            "cast-payment receipts do not form one ordered spell-cost transaction",
                        ));
                    }
                }
            }
        }

        for (index, event) in events.iter().enumerate() {
            if let GameEvent::BoundManaAbilityBundleActivated {
                player,
                mana_cost,
                bundle,
                life_payment,
                ..
            } = event
            {
                if !matches!(
                    events.get(index + 1),
                    Some(GameEvent::ManaAbilityManaPaid {
                        player: receipt_player,
                        mana_cost: receipt_cost,
                    }) if receipt_player == player && receipt_cost == mana_cost
                ) {
                    return Err(RulesError::IllegalAction(
                        "paid mana bundle activation lacks its payment receipt",
                    ));
                }
                let mut output_index = index + 2;
                if let Some(amount) = life_payment {
                    if !matches!(
                        events.get(output_index),
                        Some(GameEvent::ManaAbilityLifePaid {
                            player: receipt_player,
                            amount: receipt_amount,
                        }) if receipt_player == player && receipt_amount == amount
                    ) {
                        return Err(RulesError::IllegalAction(
                            "paid mana bundle activation lacks its life-payment receipt",
                        ));
                    }
                    output_index += 1;
                }
                for (color, amount) in bundle.iter() {
                    if !matches!(
                        events.get(output_index),
                        Some(GameEvent::ManaAdded {
                            player: receipt_player,
                            color: receipt_color,
                            amount: receipt_amount,
                        }) if receipt_player == player
                            && receipt_color == &color
                            && receipt_amount == &amount
                    ) {
                        return Err(RulesError::IllegalAction(
                            "paid mana bundle activation lacks an ordered mana-output receipt",
                        ));
                    }
                    output_index += 1;
                }
            }
        }
        Ok(())
    }

    /// Audits explicit spell-payment receipts independently of whether a cast
    /// used a mana ability. A replay must never attach colors to a different
    /// spell or insert a priority window between payment and stack entry.
    fn validate_spell_mana_payment_event_order(events: &[GameEvent]) -> Result<(), RulesError> {
        for (index, event) in events.iter().enumerate() {
            let GameEvent::SpellManaPaid {
                player,
                card,
                colors: _,
            } = event
            else {
                continue;
            };
            if !matches!(
                events.get(index + 1),
                Some(GameEvent::SpellCast { player: caster, card: spell })
                    if caster == player && spell == card
            ) {
                return Err(RulesError::IllegalAction(
                    "spell mana-payment receipt is not immediately followed by its spell cast",
                ));
            }
        }
        Ok(())
    }

    fn cast_payment_context(event: Option<&GameEvent>) -> Option<(PlayerId, ObjectId)> {
        match event {
            Some(
                GameEvent::CastPaymentBasicLandManaAbilityActivated { player, card, .. }
                | GameEvent::CastPaymentManaAbilityActivated { player, card, .. },
            ) => Some((*player, *card)),
            _ => None,
        }
    }

    /// Audits terminal stack receipts that remain meaningful in a trace suffix.
    /// `clear_event_log` intentionally permits a measured log to begin after a
    /// cast, so a terminal receipt need not have a visible `SpellCast` before
    /// it. Once a cast is visible, though, it has one visible terminal path:
    /// resolution, a rules/effect counter, or an owner leaving the game.
    fn validate_stack_terminal_event_order(&self) -> Result<(), RulesError> {
        let mut open_casts = BTreeSet::new();
        let mut terminal_cards = BTreeSet::new();

        for (index, event) in self.event_log.iter().enumerate() {
            match event {
                GameEvent::SpellCast { card, .. } => {
                    if !open_casts.insert(*card) {
                        return Err(RulesError::IllegalAction(
                            "spell receipt opened a second stack lifecycle for the same card",
                        ));
                    }
                    terminal_cards.remove(card);
                }
                GameEvent::SpellResolved { card } => {
                    Self::validate_terminal_destination(&self.event_log, index, *card, false)?;
                    Self::close_stack_receipt_lifecycle(
                        &mut open_casts,
                        &mut terminal_cards,
                        *card,
                    )?;
                }
                GameEvent::SpellCounteredByRules { card } => {
                    Self::validate_terminal_destination(&self.event_log, index, *card, true)?;
                    Self::close_stack_receipt_lifecycle(
                        &mut open_casts,
                        &mut terminal_cards,
                        *card,
                    )?;
                }
                GameEvent::SpellCountered { card, source } => {
                    if card == source {
                        return Err(RulesError::IllegalAction(
                            "a resolving spell cannot counter itself",
                        ));
                    }
                    Self::validate_terminal_destination(&self.event_log, index, *card, true)?;
                    Self::close_stack_receipt_lifecycle(
                        &mut open_casts,
                        &mut terminal_cards,
                        *card,
                    )?;
                }
                GameEvent::ObjectLeftGame { object, .. } => {
                    // CR 800.4a removes a departed owner's spell from the
                    // stack without treating it as a resolved or countered
                    // spell. It is still a terminal lifecycle outcome for a
                    // visible `SpellCast` receipt.
                    open_casts.remove(object);
                    terminal_cards.insert(*object);
                }
                _ => {}
            }
        }

        if open_casts.iter().any(|card| {
            !self
                .stack
                .iter()
                .any(|stack_object| stack_object.card == *card)
        }) {
            return Err(RulesError::IllegalAction(
                "a visible spell-cast receipt has no live stack object or terminal outcome",
            ));
        }
        Ok(())
    }

    fn validate_ability_event_order(&self) -> Result<(), RulesError> {
        let mut open = BTreeMap::<(ObjectId, &'static str), usize>::new();
        for event in &self.event_log {
            match event {
                GameEvent::AbilityActivated {
                    source, ability, ..
                }
                | GameEvent::TriggeredAbilityStacked {
                    source, ability, ..
                } => {
                    *open.entry((*source, *ability)).or_default() += 1;
                }
                GameEvent::AbilityResolved { source, ability }
                | GameEvent::AbilityCounteredByRules { source, ability } => {
                    let count =
                        open.get_mut(&(*source, *ability))
                            .ok_or(RulesError::IllegalAction(
                                "ability terminal receipt lacks an activation receipt",
                            ))?;
                    if *count == 0 {
                        return Err(RulesError::IllegalAction(
                            "ability has more terminal receipts than activations",
                        ));
                    }
                    *count -= 1;
                }
                _ => {}
            }
        }
        let live = self
            .stack
            .iter()
            .filter(|item| item.ability_id.is_some())
            .fold(
                BTreeMap::<(ObjectId, &'static str), usize>::new(),
                |mut counts, item| {
                    *counts
                        .entry((item.card, item.ability_id.expect("checked above")))
                        .or_default() += 1;
                    counts
                },
            );
        if open
            .into_iter()
            .filter(|(_, count)| *count > 0)
            .collect::<BTreeMap<_, _>>()
            != live
        {
            return Err(RulesError::IllegalAction(
                "ability activation and terminal receipts disagree with the live stack",
            ));
        }
        Ok(())
    }

    /// Audits explicit discard costs independently from stack lifecycle
    /// receipts. A discard is a cost receipt, not an ability effect: it must
    /// move an owned hand card immediately to the graveyard and precede the
    /// matching activation receipt for a binding that actually requires it.
    fn validate_ability_discard_cost_event_order(&self) -> Result<(), RulesError> {
        for (index, event) in self.event_log.iter().enumerate() {
            let GameEvent::DiscardedAsAbilityCost {
                player,
                source,
                card,
            } = event
            else {
                continue;
            };
            if !matches!(
                self.event_log.get(index + 1),
                Some(GameEvent::CardMoved {
                    card: moved,
                    to: Zone::Graveyard,
                }) if moved == card
            ) {
                return Err(RulesError::IllegalAction(
                    "discard-cost receipt is not followed by a graveyard move",
                ));
            }
            let object = self.object(*card)?;
            if object.owner != *player {
                return Err(RulesError::IllegalAction(
                    "discard-cost receipt names a card not owned by its payer",
                ));
            }
            let source_definition =
                self.object(*source)?
                    .definition
                    .ok_or(RulesError::IllegalAction(
                        "discard-cost receipt names a token source",
                    ))?;
            let ability_id = self
                .event_log
                .iter()
                .skip(index + 1)
                .find_map(|event| match event {
                    GameEvent::AbilityActivated {
                        source: activated_source,
                        ability,
                        ..
                    } if activated_source == source => Some(*ability),
                    _ => None,
                })
                .ok_or(RulesError::IllegalAction(
                    "discard-cost receipt has no matching ability activation",
                ))?;
            let ability = self
                .activated_abilities
                .get(source_definition)
                .and_then(|abilities| abilities.get(ability_id))
                .ok_or(RulesError::IllegalAction(
                    "discard-cost receipt names an unbound ability",
                ))?;
            if ability.discard_cards == 0 {
                return Err(RulesError::IllegalAction(
                    "discard-cost receipt names an ability without a discard cost",
                ));
            }
        }
        Ok(())
    }

    fn validate_terminal_destination(
        events: &[GameEvent],
        index: usize,
        card: ObjectId,
        must_move_to_graveyard: bool,
    ) -> Result<(), RulesError> {
        let destination_is_valid = matches!(
            events.get(index + 1),
            Some(GameEvent::CardMoved {
                card: moved_card,
                to: Zone::Graveyard,
            }) if *moved_card == card
        ) || (!must_move_to_graveyard
            && matches!(
                events.get(index + 1),
                Some(GameEvent::CardMoved {
                    card: moved_card,
                    to: Zone::Battlefield,
                }) if *moved_card == card
            ));
        if destination_is_valid {
            Ok(())
        } else {
            Err(RulesError::IllegalAction(
                "terminal stack receipt lacks its immediate destination move",
            ))
        }
    }

    fn close_stack_receipt_lifecycle(
        open_casts: &mut BTreeSet<ObjectId>,
        terminal_cards: &mut BTreeSet<ObjectId>,
        card: ObjectId,
    ) -> Result<(), RulesError> {
        if !open_casts.remove(&card) && !terminal_cards.insert(card) {
            return Err(RulesError::IllegalAction(
                "a spell has more than one terminal stack receipt in one event trace",
            ));
        }
        terminal_cards.insert(card);
        Ok(())
    }

    fn validate_mana_cost(cost: &crate::ManaCost) -> Result<(), RulesError> {
        if cost
            .hybrid
            .iter()
            .any(|symbol| symbol.first == symbol.second)
        {
            return Err(RulesError::IllegalAction(
                "a hybrid mana symbol requires two distinct colors",
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
            if !self.stack.is_empty() {
                // CR 800.4i: the current turn persists without its active
                // player until pending stack work has completed. Keep the
                // departed seat as turn identity, but restart priority among
                // survivors instead of starting a new turn early.
                self.priority = self.next_player(self.active_player);
                self.consecutive_passes = 0;
                return Ok(());
            }
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

    /// Runs one public transition as an all-or-error operation. The current
    /// engine is deterministic and fully in-memory, so cloning is the most
    /// reviewable transaction journal: every mutable field, including the
    /// authoritative stack and sealed event log, is restored on failure.
    fn atomic_transition<T>(
        &mut self,
        apply: impl FnOnce(&mut Self) -> Result<T, RulesError>,
    ) -> Result<T, RulesError> {
        let checkpoint = self.clone();
        match apply(self).and_then(|result| {
            self.validate_invariants()?;
            Ok(result)
        }) {
            Ok(result) => Ok(result),
            Err(error) => {
                *self = checkpoint;
                Err(error)
            }
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
        let basic_land_type = object
            .definition
            .and_then(|definition| self.basic_land_types.get(definition).copied());
        let can_attack = object.controller == self.active_player
            && self.zone_of(card) == Some(Zone::Battlefield)
            && !object.tapped
            && (object.entered_turn < self.turn
                || characteristics.keywords.contains(&Keyword::Haste))
            && characteristics.card_types.contains(&CardType::Creature)
            && !characteristics.keywords.contains(&Keyword::Defender);
        Ok(CardView {
            id: card,
            definition: object.definition,
            controller: object.controller,
            tapped: object.tapped,
            colors: characteristics.colors,
            mana_colors,
            basic_land_type,
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
            .filter_map(|(id, object)| {
                (object.owner == player).then_some((*id, object.owner, object.definition))
            })
            .collect::<Vec<_>>();
        for (object, object_owner, definition) in owned_objects {
            if let Some(definition) = definition {
                self.departed_card_definitions.insert(object, definition);
            }
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

    #[test]
    fn payment_trace_rejects_a_basic_land_marker_without_its_mana_output() {
        let player = PlayerId(0);
        let card = ObjectId(10);
        let land = ObjectId(11);
        let events = vec![
            GameEvent::CastPaymentBasicLandManaAbilityActivated {
                player,
                card,
                land,
                color: Color::Green,
            },
            GameEvent::ManaAbilityActivated {
                player,
                land,
                color: Color::Green,
            },
            GameEvent::SpellCast { player, card },
        ];

        assert_eq!(
            Game::validate_mana_ability_event_order(&events),
            Err(RulesError::IllegalAction(
                "cast-payment basic-land activation lacks its matching mana-output receipt",
            ))
        );
    }

    #[test]
    fn payment_trace_requires_mixed_basic_and_bound_receipts_to_close_one_cast() {
        let player = PlayerId(0);
        let card = ObjectId(10);
        let other_card = ObjectId(12);
        let land = ObjectId(11);
        let source = ObjectId(13);
        let events = vec![
            GameEvent::CastPaymentBasicLandManaAbilityActivated {
                player,
                card,
                land,
                color: Color::Green,
            },
            GameEvent::ManaAbilityActivated {
                player,
                land,
                color: Color::Green,
            },
            GameEvent::ManaAdded {
                player,
                color: Color::Green,
                amount: 1,
            },
            GameEvent::CastPaymentManaAbilityActivated {
                player,
                card,
                source,
                ability: "bundle",
            },
            GameEvent::BoundManaAbilityBundleActivated {
                player,
                source,
                ability: "bundle",
                mana_cost: crate::ManaCost::new(1),
                bundle: crate::ManaBundle::new([(Color::Blue, 1), (Color::Red, 1)]),
                tapped: true,
                life_payment: None,
            },
            GameEvent::ManaAbilityManaPaid {
                player,
                mana_cost: crate::ManaCost::new(1),
            },
            GameEvent::ManaAdded {
                player,
                color: Color::Blue,
                amount: 1,
            },
            GameEvent::ManaAdded {
                player,
                color: Color::Red,
                amount: 1,
            },
            GameEvent::SpellCast { player, card },
        ];
        Game::validate_mana_ability_event_order(&events)
            .expect("the mixed receipt trace is ordered and closes its cast");

        let mut interleaved = events;
        interleaved[3] = GameEvent::CastPaymentManaAbilityActivated {
            player,
            card: other_card,
            source,
            ability: "bundle",
        };
        interleaved[8] = GameEvent::SpellCast {
            player,
            card: other_card,
        };
        assert_eq!(
            Game::validate_mana_ability_event_order(&interleaved),
            Err(RulesError::IllegalAction(
                "cast-payment receipts changed their enclosing spell before SpellCast",
            ))
        );
    }

    #[test]
    fn stack_terminal_trace_requires_one_immediate_destination_move() {
        let player = PlayerId(0);
        let card = ObjectId(10);
        let mut game = Game::new(Vec::<CardDefinition>::new(), 2).expect("two-player game");
        game.event_log = vec![
            GameEvent::SpellCast { player, card },
            GameEvent::SpellResolved { card },
            GameEvent::ManaAdded {
                player,
                color: Color::Blue,
                amount: 1,
            },
        ];
        game.event_log_integrity = game.event_log.clone();

        assert_eq!(
            game.validate_stack_terminal_event_order(),
            Err(RulesError::IllegalAction(
                "terminal stack receipt lacks its immediate destination move",
            ))
        );
    }

    #[test]
    fn stack_terminal_trace_allows_a_measured_suffix_but_rejects_double_terminal_receipts() {
        let card = ObjectId(10);
        let mut game = Game::new(Vec::<CardDefinition>::new(), 2).expect("two-player game");
        game.event_log = vec![
            GameEvent::SpellCounteredByRules { card },
            GameEvent::CardMoved {
                card,
                to: Zone::Graveyard,
            },
        ];
        game.event_log_integrity = game.event_log.clone();
        game.validate_stack_terminal_event_order()
            .expect("a measured trace may begin during an existing stack lifecycle");

        game.event_log.extend([
            GameEvent::SpellResolved { card },
            GameEvent::CardMoved {
                card,
                to: Zone::Graveyard,
            },
        ]);
        game.event_log_integrity = game.event_log.clone();
        assert_eq!(
            game.validate_stack_terminal_event_order(),
            Err(RulesError::IllegalAction(
                "a spell has more than one terminal stack receipt in one event trace",
            ))
        );
    }

    #[test]
    fn invariant_rejects_trample_provenance_that_is_not_a_declared_attacker() {
        let mut game = Game::new(Vec::<CardDefinition>::new(), 2).expect("two-player game");
        game.step = Step::DeclareAttackers;
        game.combat = Some(CombatState {
            attackers_declared: true,
            defending_player: Some(PlayerId(1)),
            trampling_attackers: BTreeSet::from([ObjectId(99)]),
            ..CombatState::default()
        });

        assert_eq!(
            game.validate_invariants(),
            Err(RulesError::IllegalAction(
                "trample declaration provenance contains a nonattacker",
            ))
        );
    }
}
