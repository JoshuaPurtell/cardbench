use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};

use crate::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, ActivatedAbilityCostAdjustment,
    ActivatedAbilityCostContext, ActivatedAbilityCostModifier, ActivatedAbilityCostModifierBinding,
    ActivatedAbilityKind, ActivatedManaAbility, AdditionalSpellCost, AdditionalSpellCostBinding,
    BasicLandType, BasicLandTypeBinding, CardDefinition, CardObject, CardType,
    CastPaymentManaAbility, Characteristics, Color, CombatBlock, ContinuousChange,
    ContinuousEffect, CopiableValues, CopiedPermanent, CostReductionBinding, CounterKind,
    CreatureSubtype, DamageReplacementChoice, DecisionContinuation, DecisionId, DecisionKind,
    DecisionOption, DecisionSelection, DecisionVisibility, DeckList, DelayedAction,
    DelayedActionId, DelayedActionKind, DelayedActionTiming, Duration, Effect, GameEvent, Keyword,
    LandEntryBinding, LibrarySearchDestination, LibrarySearchRequirement, LibrarySearchSelection,
    LinkedExileGroup, LinkedExileGroupId, LinkedExileMember, LinkedExileMemberRole,
    ManaAbilityActivation, ManaAbilityBinding, ManaAbilityOutput, ManaCost, ManaPaymentSelection,
    ObjectId, PendingDecision, PlayerId, PlayerState, PolicyMoveKind, ReplacementEffect,
    ReplacementEffectBinding, ReplacementEventKind, StackEffectResolution, StackObject,
    StackResolutionPlan, StaticAttackRestriction, StaticAttackRestrictionBinding,
    StaticContinuousEffectBinding, Step, Target, TargetRequirement, TokenSpec, TriggerCondition,
    TriggeredAbilityBinding, TriggeredEffectObjectDecisionKind, Zone,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GraveyardCastPermission {
    player: PlayerId,
    expires_turn: u32,
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
    /// Resolves one controller-private library choice that was opened while a
    /// spell was resolving. This is a decision boundary, not priority.
    ChoosePrivateLibraryCards {
        spell: ObjectId,
        selected: Vec<ObjectId>,
    },
    /// Completes one controller-private selection opened during a targeted
    /// activated ability's resolution. This is not a priority action and the
    /// target opponent never receives the candidate identities in a view.
    ChoosePrivateOpponentLibraryCardToExile {
        source: ObjectId,
        ability: &'static str,
        selected: Option<ObjectId>,
    },
    /// Selects one currently matching controller-owned library card, or
    /// deliberately fails to find when that suspended search permits it.
    /// This is a resolution decision, never a priority action.
    ChooseLibrarySearchCard {
        source: ObjectId,
        selected: Option<ObjectId>,
    },
    /// Supplies the ordered targets for the next triggered ability waiting to
    /// be put onto the stack. This is a no-priority rules decision.
    ChooseTriggeredAbilityTargets {
        source: ObjectId,
        ability: &'static str,
        targets: Vec<Target>,
    },
    /// Selects the one public object required by a suspended trigger effect.
    /// A `None` selection is legal only when the projected candidate list is
    /// empty (for example, a player with no hand cards to discard).
    ChooseTriggeredAbilityEffectObject {
        source: ObjectId,
        ability: &'static str,
        selected: Option<ObjectId>,
    },
    /// Selects one applicable replacement for a prospective damage event.
    /// This is a no-priority decision made by the affected player while the
    /// original one-effect damage spell remains on the stack.
    ChooseDamageReplacement {
        source: ObjectId,
        source_incarnation: u64,
        target: Target,
        replacement: DamageReplacementChoice,
    },
    /// Accepts or declines an optional triggered mana payment after every
    /// player has passed. A conditional target is supplied only when paying.
    ResolveOptionalTriggeredAbility {
        source: ObjectId,
        ability: &'static str,
        pay: bool,
        target: Option<Target>,
    },
    /// Submits an answer to the one current typed decision. The exact
    /// `DecisionId` is mandatory: a response to an earlier prompt cannot
    /// accidentally resolve a later matching source/ability prompt.
    SubmitDecision {
        decision: DecisionId,
        selection: DecisionSelection,
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
            Self::ChoosePrivateLibraryCards { .. } => PolicyMoveKind::ChoosePrivateLibraryCards,
            Self::ChoosePrivateOpponentLibraryCardToExile { .. } => {
                PolicyMoveKind::ChoosePrivateOpponentLibraryCardToExile
            }
            Self::ChooseLibrarySearchCard { .. } => PolicyMoveKind::ChooseLibrarySearchCard,
            Self::ChooseTriggeredAbilityTargets { .. } => {
                PolicyMoveKind::ChooseTriggeredAbilityTargets
            }
            Self::ChooseTriggeredAbilityEffectObject { .. } => {
                PolicyMoveKind::ChooseTriggeredAbilityEffectObject
            }
            Self::ChooseDamageReplacement { .. } => PolicyMoveKind::ChooseDamageReplacement,
            Self::ResolveOptionalTriggeredAbility { .. } => {
                PolicyMoveKind::ResolveOptionalTriggeredAbility
            }
            Self::SubmitDecision { .. } => PolicyMoveKind::SubmitDecision,
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

/// Controller-only private-library selection exposed while a resolving spell
/// waits for a mandatory choice. Other players see no candidate identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateLibraryChoiceView {
    pub spell: ObjectId,
    pub cards: Vec<CardView>,
    pub life_per_card: i16,
}

/// Controller-only candidates exposed while an activated ability resolves a
/// private inspection of a targeted opponent's library. Candidate identities
/// are intentionally absent from every other player's `GameView` and from the
/// public event log.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PrivateOpponentLibraryChoiceView {
    pub source: ObjectId,
    pub ability: &'static str,
    pub cards: Vec<CardView>,
}

/// Controller-only matching cards for a suspended typed library search.
/// Candidate identities never appear in another player's view or the public
/// event log before the selected card changes zones.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LibrarySearchChoiceView {
    pub source: ObjectId,
    pub cards: Vec<CardView>,
    pub destination: LibrarySearchDestination,
    pub may_fail_to_find: bool,
}

/// Public legal target options for one trigger waiting to enter the stack.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TriggeredAbilityTargetChoiceView {
    pub source: ObjectId,
    pub ability: &'static str,
    /// One ordered option set for each target occurrence.
    pub target_options: Vec<Vec<Target>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OptionalTriggeredAbilityChoiceView {
    pub source: ObjectId,
    pub ability: &'static str,
    pub mana_cost: ManaCost,
    pub can_pay: bool,
    /// Legal choices for a target selected only if the optional cost is paid.
    pub conditional_targets: Vec<Target>,
}

/// One public-zone (or chooser-private hand) object selection requested while
/// a trigger is resolving. Only the chooser receives the candidate identities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TriggeredAbilityEffectObjectChoiceView {
    pub source: ObjectId,
    pub ability: &'static str,
    pub candidates: Vec<CardView>,
}

/// The generic projection of one pending decision. Candidate identities are
/// exposed only to the deciding player; existing specialized views remain as
/// compatibility projections for migrated consumers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingDecisionView {
    pub id: DecisionId,
    pub kind: DecisionKind,
    pub visibility: DecisionVisibility,
    pub min_selections: u8,
    pub max_selections: u8,
    pub candidates: Vec<CardView>,
}

/// Public, stale-safe details for a prospective damage event requiring an
/// affected-player replacement choice. Unlike hidden-zone choices, all of
/// these identities are already public battlefield/player information.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DamageReplacementChoiceView {
    pub source: ObjectId,
    pub source_incarnation: u64,
    pub target: Target,
    pub target_incarnation: Option<u64>,
    pub amount: i32,
    pub replacements: Vec<DamageReplacementChoice>,
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
    /// Controller-only private cards currently awaiting a choice within a
    /// suspended spell resolution. This is never a priority window.
    pub private_library_choice: Option<PrivateLibraryChoiceView>,
    /// Controller-only candidates for a suspended targeted activated ability
    /// that will exile one card from an opponent's library. This is likewise
    /// never a priority window.
    pub private_opponent_library_choice: Option<PrivateOpponentLibraryChoiceView>,
    /// Controller-only candidates for a typed library search awaiting a
    /// no-priority selection during stack resolution.
    pub library_search_choice: Option<LibrarySearchChoiceView>,
    /// Present only to the trigger's controller while target selection is due.
    pub triggered_ability_target_choice: Option<TriggeredAbilityTargetChoiceView>,
    pub optional_triggered_ability_choice: Option<OptionalTriggeredAbilityChoiceView>,
    pub triggered_ability_effect_object_choice: Option<TriggeredAbilityEffectObjectChoiceView>,
    /// The canonical typed no-priority decision surface. This first migration
    /// covers library search and triggered discard/sacrifice selections.
    pub pending_decision: Option<PendingDecisionView>,
    /// Present only to the affected player while a bounded prospective damage
    /// event requires replacement ordering.
    pub damage_replacement_choice: Option<DamageReplacementChoiceView>,
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
    /// Attackers that could not be blocked when they were declared. This is
    /// declaration provenance: a later continuous effect does not retroactively
    /// make an earlier block legal.
    unblockable_attackers: BTreeSet<ObjectId>,
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
    /// Declaration-time landwalk provenance. An attacker may carry more than
    /// one named basic land type through independent continuous effects.
    landwalk_attackers: BTreeMap<ObjectId, BTreeSet<BasicLandType>>,
    /// Ordered blocker groups keyed by attacker. The vector order is the
    /// deterministic compatibility damage-assignment order until the policy
    /// decision layer exposes the attacking player's CR 509.2 choice.
    blockers: BTreeMap<ObjectId, Vec<ObjectId>>,
    /// A blocker that regenerated remains associated with its attacker (so
    /// that attacker stays blocked) but no longer assigns or receives combat
    /// damage. This preserves the difference between leaving combat and
    /// ceasing to block altogether.
    removed_from_combat: BTreeSet<ObjectId>,
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

/// A resolution-time private library decision. The stack object remains live
/// while this marker is present, but no player has priority until its
/// controller submits a legal selection.
#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingPrivateLibraryChoice {
    spell: ObjectId,
    controller: PlayerId,
    cards: Vec<ObjectId>,
    life_per_card: i16,
}

/// A suspended targeted ability awaiting its controller's private choice of a
/// card from the target opponent's current top-of-library snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingPrivateOpponentLibraryExileChoice {
    source: ObjectId,
    ability: &'static str,
    controller: PlayerId,
    opponent: PlayerId,
    cards: Vec<ObjectId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingTriggeredAbilityTargetChoice {
    source: ObjectId,
    source_incarnation: u64,
    source_colors: BTreeSet<Color>,
    controller: PlayerId,
    ability: crate::TriggeredAbility,
    effects: Vec<Effect>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingOptionalTriggeredAbilityChoice {
    source: ObjectId,
    source_incarnation: u64,
    source_colors: BTreeSet<Color>,
    controller: PlayerId,
    ability: crate::TriggeredAbility,
}

#[derive(Clone, Debug)]
enum TriggerEventPayload {
    /// The triggering event has no dynamic payload. Its bound effects are
    /// copied unchanged when the ability is placed on the stack.
    None,
    /// Damage has already happened. Dynamic damage-trigger effects must use
    /// this captured amount instead of inspecting a later game state.
    DamageAmount(i16),
    /// The event itself identifies the trigger's sole target. This is used
    /// for the represented Blood Funnel cast trigger, where the triggering
    /// spell is not a policy-selected target.
    ExactTargets(Vec<Target>),
}

/// One captured rules event awaiting APNAP-safe trigger placement.  Every
/// represented trigger condition flows through this same payload so deferred
/// damage, dies, life-gain, ETB, land-entry, upkeep, and attack paths share
/// target selection and stack construction instead of each owning a
/// card-shaped queue.
#[derive(Clone, Debug)]
struct PendingTriggeredAbilityEvent {
    source: ObjectId,
    source_incarnation: u64,
    source_colors: BTreeSet<Color>,
    controller: PlayerId,
    ability: crate::TriggeredAbility,
    payload: TriggerEventPayload,
}

#[derive(Clone, Debug)]
struct PendingDamageRedirection {
    source: ObjectId,
    protected: ObjectId,
    remaining: i32,
}

#[derive(Clone, Debug)]
struct DamageRedirection {
    id: u64,
    source: ObjectId,
    protected: ObjectId,
    destination: Target,
    remaining: i32,
    expires_turn: u32,
}

#[derive(Clone, Debug)]
struct DamagePreventionShield {
    id: u64,
    source: ObjectId,
    target: Target,
    remaining: i32,
    expires_turn: u32,
}

/// A deliberately narrow resumable damage-resolution continuation.  It is
/// opened only for a single targeted `DealDamage` instant/sorcery with two or
/// more live applicable replacements. The top stack item remains in place;
/// there is no priority until the affected player selects one legal effect.
#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingDamageReplacementChoice {
    source: ObjectId,
    source_incarnation: u64,
    controller: PlayerId,
    affected_player: PlayerId,
    original_target: Target,
    target: Target,
    target_incarnation: Option<u64>,
    amount: i32,
    used: Vec<DamageReplacementChoice>,
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
    static_attack_restrictions: BTreeMap<&'static str, Vec<StaticAttackRestriction>>,
    static_continuous_effects: BTreeMap<&'static str, Vec<ContinuousChange>>,
    cost_reductions: BTreeMap<&'static str, CostReductionBinding>,
    activated_ability_cost_modifiers: BTreeMap<&'static str, Vec<ActivatedAbilityCostModifier>>,
    replacement_effects: BTreeMap<&'static str, Vec<ReplacementEffect>>,
    basic_land_types: BTreeMap<&'static str, BasicLandType>,
    land_entry_behaviors: BTreeMap<&'static str, LandEntryBinding>,
    additional_spell_costs: BTreeMap<&'static str, Vec<AdditionalSpellCost>>,
    graveyard_cast_permissions: BTreeMap<ObjectId, GraveyardCastPermission>,
    exile_on_resolution: BTreeSet<ObjectId>,
    /// Exact-incarnation exile groups that still own one delayed return.
    /// This is typed, clonable state; no resolver closure escapes onto the
    /// game state machine.
    linked_exile_groups: BTreeMap<LinkedExileGroupId, LinkedExileGroup>,
    delayed_actions: Vec<DelayedAction>,
    next_linked_exile_group_id: u64,
    next_delayed_action_id: u64,
    /// Source-identified replacement shields created by regeneration. A
    /// target may have multiple pending shields, consumed LIFO one at a time.
    regeneration_shields: BTreeMap<ObjectId, Vec<ObjectId>>,
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
    /// The next identity assigned to an opened typed decision. It is never
    /// rewound by a successful continuation, so stale policy responses cannot
    /// alias a later prompt from the same source.
    next_decision_id: u64,
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
    pending_private_library_choice: Option<PendingPrivateLibraryChoice>,
    pending_private_opponent_library_exile_choice: Option<PendingPrivateOpponentLibraryExileChoice>,
    pending_decision: Option<PendingDecision>,
    pending_trigger_target_choices: Vec<PendingTriggeredAbilityTargetChoice>,
    pending_optional_trigger_choice: Option<PendingOptionalTriggeredAbilityChoice>,
    pending_damage_replacement_choice: Option<PendingDamageReplacementChoice>,
    /// A resolving rule may prevent every represented library search until the
    /// current turn ends. It is deliberately a turn number, rather than a
    /// boolean, so invariant checks detect an expired marker crossing a turn
    /// transition.
    library_search_prevented_until: Option<u32>,
    /// Condition events are captured where they happen and placed as one
    /// APNAP-ordered batch after the enclosing action has finished. A separate
    /// placement queue preserves that order while target-bearing triggers wait
    /// for their controllers' no-priority choices.
    pending_trigger_events: Vec<PendingTriggeredAbilityEvent>,
    pending_trigger_placements: Vec<PendingTriggeredAbilityEvent>,
    /// Land entries produced while a spell or ability resolves. Their trigger
    /// batches wait until that enclosing stack object has completed its own
    /// terminal lifecycle, matching the ordinary post-resolution trigger
    /// window rather than creating a nested stack object mid-resolution.
    pending_land_entry_trigger_batches: Vec<PlayerId>,
    pending_damage_redirection: Option<PendingDamageRedirection>,
    damage_redirections: Vec<DamageRedirection>,
    damage_prevention_shields: Vec<DamagePreventionShield>,
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
            Self::validate_card_colors(&definition.colors)?;
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
            static_attack_restrictions: BTreeMap::new(),
            static_continuous_effects: BTreeMap::new(),
            cost_reductions: BTreeMap::new(),
            activated_ability_cost_modifiers: BTreeMap::new(),
            replacement_effects: BTreeMap::new(),
            basic_land_types,
            land_entry_behaviors: BTreeMap::new(),
            additional_spell_costs,
            graveyard_cast_permissions: BTreeMap::new(),
            exile_on_resolution: BTreeSet::new(),
            linked_exile_groups: BTreeMap::new(),
            delayed_actions: Vec::new(),
            next_linked_exile_group_id: 1,
            next_delayed_action_id: 1,
            regeneration_shields: BTreeMap::new(),
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
            next_decision_id: 1,
            departed_card_definitions: BTreeMap::new(),
            consecutive_passes: 0,
            shuffle_seed: 0,
            started: false,
            terminal_event_emitted: false,
            combat: None,
            pending_draw_replacement: None,
            pending_private_library_choice: None,
            pending_private_opponent_library_exile_choice: None,
            pending_decision: None,
            pending_trigger_target_choices: Vec::new(),
            pending_optional_trigger_choice: None,
            pending_damage_replacement_choice: None,
            library_search_prevented_until: None,
            pending_trigger_events: Vec::new(),
            pending_trigger_placements: Vec::new(),
            pending_land_entry_trigger_batches: Vec::new(),
            pending_damage_redirection: None,
            damage_redirections: Vec::new(),
            damage_prevention_shields: Vec::new(),
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

    /// Registers source-bound generic-cost reductions before the game begins.
    /// Their sources are rechecked on the battlefield for each cast, so normal
    /// zone changes automatically stop the reduction.
    pub fn register_cost_reduction_bindings(
        &mut self,
        bindings: impl IntoIterator<Item = CostReductionBinding>,
    ) -> Result<(), RulesError> {
        if self.started {
            return Err(RulesError::IllegalAction(
                "cost-reduction bindings cannot be changed after the game starts",
            ));
        }
        for binding in bindings {
            let definition = self
                .catalog
                .get(binding.source_definition)
                .ok_or(RulesError::UnknownDefinition(binding.source_definition))?;
            if !definition.is_permanent() || binding.generic_amount == 0 {
                return Err(RulesError::IllegalAction(
                    "a cost reduction requires a permanent source and positive amount",
                ));
            }
            if self
                .cost_reductions
                .insert(binding.source_definition, binding)
                .is_some()
            {
                return Err(RulesError::IllegalAction(
                    "duplicate cost-reduction binding for card definition",
                ));
            }
        }
        self.validate_invariants()
    }

    /// Registers immutable, battlefield-scoped activated-cost modifiers before
    /// a game begins.  A binding names expansion data, while each individual
    /// permanent is discovered from the live battlefield when an ability is
    /// activated; ordinary source departure therefore revokes its adjustment
    /// without a card-specific cleanup hook.
    pub fn register_activated_ability_cost_modifier_bindings(
        &mut self,
        bindings: impl IntoIterator<Item = ActivatedAbilityCostModifierBinding>,
    ) -> Result<(), RulesError> {
        if self.started {
            return Err(RulesError::IllegalAction(
                "activated-cost modifier bindings cannot be changed after the game starts",
            ));
        }
        for binding in bindings {
            let definition = self
                .catalog
                .get(binding.source_definition)
                .ok_or(RulesError::UnknownDefinition(binding.source_definition))?;
            if !definition.is_permanent() || binding.modifier.generic_amount() == 0 {
                return Err(RulesError::IllegalAction(
                    "an activated-cost modifier requires a permanent source and positive amount",
                ));
            }
            if binding.modifier.applies_to(ActivatedAbilityKind::Mana) {
                return Err(RulesError::IllegalAction(
                    "the initial activated-cost modifier slice supports nonmana abilities only",
                ));
            }
            let modifiers = self
                .activated_ability_cost_modifiers
                .entry(binding.source_definition)
                .or_default();
            if modifiers.contains(&binding.modifier) {
                return Err(RulesError::IllegalAction(
                    "duplicate activated-cost modifier binding",
                ));
            }
            modifiers.push(binding.modifier);
        }
        self.validate_invariants()
    }

    /// Registers immutable, source-bound quantity replacements before the
    /// game begins. Applicability is evaluated for each event from the live
    /// battlefield and the affected player's current controller state.
    pub fn register_replacement_effect_bindings(
        &mut self,
        bindings: impl IntoIterator<Item = ReplacementEffectBinding>,
    ) -> Result<(), RulesError> {
        if self.started {
            return Err(RulesError::IllegalAction(
                "replacement-effect bindings cannot be changed after the game starts",
            ));
        }
        for binding in bindings {
            let definition = self
                .catalog
                .get(binding.source_definition)
                .ok_or(RulesError::UnknownDefinition(binding.source_definition))?;
            if !definition.is_permanent() || binding.effect.multiplier() < 2 {
                return Err(RulesError::IllegalAction(
                    "replacement effect requires a permanent source and multiplier of at least two",
                ));
            }
            let effects = self
                .replacement_effects
                .entry(binding.source_definition)
                .or_default();
            if effects.contains(&binding.effect) {
                return Err(RulesError::IllegalAction(
                    "duplicate replacement-effect binding for card definition",
                ));
            }
            effects.push(binding.effect);
        }
        self.validate_invariants()
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
                        | TriggerCondition::LandEntersBattlefield
                        | TriggerCondition::ControlledLandEntersBattlefield
                        | TriggerCondition::BeginningOfUpkeep
                        | TriggerCondition::LifeGained
                        | TriggerCondition::DealsDamage
                        | TriggerCondition::ReceivesDamage
                        | TriggerCondition::Dies
                        | TriggerCondition::AnotherCreatureDies
                        | TriggerCondition::Attacks
                        | TriggerCondition::CastsNoncreatureSpell
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

    /// Creates a game with immutable, battlefield-only static continuous
    /// bindings in addition to the ordinary expansion bindings.
    pub fn new_with_all_bindings_and_static_continuous_effects(
        definitions: impl IntoIterator<Item = CardDefinition>,
        player_count: usize,
        mana_bindings: impl IntoIterator<Item = ManaAbilityBinding>,
        basic_land_types: impl IntoIterator<Item = BasicLandTypeBinding>,
        additional_spell_costs: impl IntoIterator<Item = AdditionalSpellCostBinding>,
        ability_bindings: impl IntoIterator<Item = ActivatedAbilityBinding>,
        static_bindings: impl IntoIterator<Item = StaticContinuousEffectBinding>,
    ) -> Result<Self, RulesError> {
        let mut game = Self::new_with_all_bindings(
            definitions,
            player_count,
            mana_bindings,
            basic_land_types,
            additional_spell_costs,
            ability_bindings,
        )?;
        game.register_static_continuous_effects(static_bindings)?;
        game.validate_invariants()?;
        Ok(game)
    }

    /// Creates a game with triggers and immutable battlefield-only static
    /// continuous bindings. Keeping this explicit avoids a hidden global set
    /// registry and lets a scenario declare every active rule substrate.
    #[allow(clippy::too_many_arguments)] // Expansion bindings stay explicit at construction.
    pub fn new_with_all_bindings_triggers_and_static_continuous_effects(
        definitions: impl IntoIterator<Item = CardDefinition>,
        player_count: usize,
        mana_bindings: impl IntoIterator<Item = ManaAbilityBinding>,
        basic_land_types: impl IntoIterator<Item = BasicLandTypeBinding>,
        additional_spell_costs: impl IntoIterator<Item = AdditionalSpellCostBinding>,
        ability_bindings: impl IntoIterator<Item = ActivatedAbilityBinding>,
        trigger_bindings: impl IntoIterator<Item = TriggeredAbilityBinding>,
        static_bindings: impl IntoIterator<Item = StaticContinuousEffectBinding>,
    ) -> Result<Self, RulesError> {
        let mut game = Self::new_with_all_bindings_and_triggers(
            definitions,
            player_count,
            mana_bindings,
            basic_land_types,
            additional_spell_costs,
            ability_bindings,
            trigger_bindings,
        )?;
        game.register_static_continuous_effects(static_bindings)?;
        game.validate_invariants()?;
        Ok(game)
    }

    /// Creates a game with trigger, static-continuous, and land-entry
    /// bindings. Land-entry bindings model only replacement-style entry
    /// behavior; any ETB ability stays in `trigger_bindings` and therefore
    /// follows the normal stack and priority lifecycle.
    #[allow(clippy::too_many_arguments)] // Expansion bindings stay explicit at construction.
    pub fn new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        definitions: impl IntoIterator<Item = CardDefinition>,
        player_count: usize,
        mana_bindings: impl IntoIterator<Item = ManaAbilityBinding>,
        basic_land_types: impl IntoIterator<Item = BasicLandTypeBinding>,
        additional_spell_costs: impl IntoIterator<Item = AdditionalSpellCostBinding>,
        ability_bindings: impl IntoIterator<Item = ActivatedAbilityBinding>,
        trigger_bindings: impl IntoIterator<Item = TriggeredAbilityBinding>,
        static_bindings: impl IntoIterator<Item = StaticContinuousEffectBinding>,
        land_entry_bindings: impl IntoIterator<Item = LandEntryBinding>,
    ) -> Result<Self, RulesError> {
        let mut game = Self::new_with_all_bindings_triggers_and_static_continuous_effects(
            definitions,
            player_count,
            mana_bindings,
            basic_land_types,
            additional_spell_costs,
            ability_bindings,
            trigger_bindings,
            static_bindings,
        )?;
        game.register_land_entry_behaviors(land_entry_bindings)?;
        game.validate_invariants()?;
        Ok(game)
    }

    fn register_land_entry_behaviors(
        &mut self,
        bindings: impl IntoIterator<Item = LandEntryBinding>,
    ) -> Result<(), RulesError> {
        for binding in bindings {
            let definition = self
                .catalog
                .get(binding.card_definition)
                .ok_or(RulesError::UnknownDefinition(binding.card_definition))?;
            if !definition.is_land() || !binding.enters_tapped {
                return Err(RulesError::IllegalAction(
                    "land-entry binding requires a land that enters tapped",
                ));
            }
            if self
                .land_entry_behaviors
                .insert(binding.card_definition, binding)
                .is_some()
            {
                return Err(RulesError::IllegalAction(
                    "duplicate land-entry binding for card definition",
                ));
            }
        }
        Ok(())
    }

    /// Registers immutable, battlefield-only attack restrictions before a game
    /// starts. The source is rechecked from the live battlefield during each
    /// attacker declaration, so normal zone changes revoke the rule without a
    /// synthetic event or cleanup marker.
    pub fn register_static_attack_restrictions(
        &mut self,
        bindings: impl IntoIterator<Item = StaticAttackRestrictionBinding>,
    ) -> Result<(), RulesError> {
        if self.started {
            return Err(RulesError::IllegalAction(
                "static attack restrictions cannot be changed after the game starts",
            ));
        }
        for binding in bindings {
            let definition = self
                .catalog
                .get(binding.card_definition)
                .ok_or(RulesError::UnknownDefinition(binding.card_definition))?;
            if !definition.is_permanent() {
                return Err(RulesError::IllegalAction(
                    "a static attack restriction requires a permanent source",
                ));
            }
            let restrictions = self
                .static_attack_restrictions
                .entry(binding.card_definition)
                .or_default();
            if restrictions.contains(&binding.restriction) {
                return Err(RulesError::IllegalAction(
                    "duplicate static attack-restriction binding",
                ));
            }
            restrictions.push(binding.restriction);
        }
        self.validate_invariants()
    }

    fn register_static_continuous_effects(
        &mut self,
        bindings: impl IntoIterator<Item = StaticContinuousEffectBinding>,
    ) -> Result<(), RulesError> {
        for binding in bindings {
            let definition = self
                .catalog
                .get(binding.card_definition)
                .ok_or(RulesError::UnknownDefinition(binding.card_definition))?;
            if !definition.is_creature()
                || !matches!(
                    binding.change,
                    ContinuousChange::ControlledCreatureCountPowerToughness
                        | ContinuousChange::OtherControlledCreaturesModifyPowerToughness { .. }
                        | ContinuousChange::OtherControlledCreaturesAddKeyword(_)
                        | ContinuousChange::ControlledCreaturesAddKeywordIfSourceEnchanted(_)
                )
            {
                return Err(RulesError::IllegalAction(
                    "static continuous binding has unsupported source or change",
                ));
            }
            let changes = self
                .static_continuous_effects
                .entry(binding.card_definition)
                .or_default();
            if changes.contains(&binding.change) {
                return Err(RulesError::IllegalAction(
                    "duplicate static continuous-effect binding",
                ));
            }
            changes.push(binding.change);
        }
        Ok(())
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

    /// Returns the rules-derived controller of an object.
    ///
    /// `CardObject::controller` is the base controller. Only a live
    /// battlefield permanent can have that value replaced by layer-two
    /// continuous effects, which are applied in timestamp order. Zone vectors
    /// stay owner-indexed throughout; controlling a permanent never performs
    /// a hidden zone move.
    pub fn controller_of(&self, card: ObjectId) -> Result<PlayerId, RulesError> {
        let object = self.object(card)?;
        let mut controller = object.controller;
        if self.zone_of(card) != Some(Zone::Battlefield) {
            return Ok(controller);
        }
        let mut effects = self
            .continuous_effects
            .iter()
            .filter(|effect| {
                effect.target == card
                    && self.effect_is_active(effect)
                    && matches!(effect.change, ContinuousChange::ChangeController(_))
            })
            .collect::<Vec<_>>();
        effects.sort_by_key(|effect| effect.timestamp);
        for effect in effects {
            if let ContinuousChange::ChangeController(next) = effect.change {
                controller = next;
            }
        }
        self.player(controller)?;
        Ok(controller)
    }

    /// Returns immutable typed provenance for one unresolved linked-exile
    /// group. The group disappears as part of consuming its delayed action.
    #[must_use]
    pub fn linked_exile_group(&self, group: LinkedExileGroupId) -> Option<&LinkedExileGroup> {
        self.linked_exile_groups.get(&group)
    }

    /// Returns every unresolved delayed action. This is an inspection surface
    /// for policies and audit tests; gameplay cannot mutate the schedule
    /// outside a rules transition.
    #[must_use]
    pub fn delayed_actions(&self) -> &[DelayedAction] {
        &self.delayed_actions
    }

    pub fn card_definition(&self, card: ObjectId) -> Result<&CardDefinition, RulesError> {
        let definition = self
            .effective_definition_id(card)?
            .ok_or(RulesError::IllegalAction("token has no card definition"))?;
        self.catalog
            .get(definition)
            .ok_or(RulesError::UnknownDefinition(definition))
    }

    /// Returns the definition currently used by definition-bound rules for an
    /// object.  A layer-one card copy deliberately changes this identity for
    /// characteristics, activated abilities, static bindings, and triggers;
    /// `CardObject::definition` remains the physical card's printed identity
    /// for ownership and zone bookkeeping.
    fn effective_definition_id(&self, card: ObjectId) -> Result<Option<&'static str>, RulesError> {
        Ok(self.object(card)?.effective_definition())
    }

    /// Returns the layer-one characteristics another permanent would copy
    /// from this object.  It never evaluates counters, marked damage,
    /// attachments, or later-layer continuous effects.
    pub fn copiable_values(&self, card: ObjectId) -> Result<CopiableValues, RulesError> {
        let object = self.object(card)?;
        if let Some(copy) = &object.copied_permanent {
            return Ok(copy.values.clone());
        }
        if let Some(token) = &object.token {
            return Ok(CopiableValues::Token(token.clone()));
        }
        object
            .definition
            .map(CopiableValues::CardDefinition)
            .ok_or(RulesError::IllegalAction(
                "a copy source must have copiable card or token values",
            ))
    }

    /// Applies a persistent layer-one copy snapshot from `source` to
    /// `target`.  The source and target must be distinct live permanents. The
    /// snapshot survives source departure but expires when the target changes
    /// zones, at which point its next incarnation resumes its printed values.
    pub fn copy_permanent(&mut self, target: ObjectId, source: ObjectId) -> Result<(), RulesError> {
        self.atomic_transition(|game| {
            game.require_game_in_progress()?;
            if target == source {
                return Err(RulesError::IllegalAction(
                    "a copy effect requires distinct source and target permanents",
                ));
            }
            game.require_zone(source, Zone::Battlefield)?;
            game.require_zone(target, Zone::Battlefield)?;
            let values = game.copiable_values(source)?;
            let source_incarnation = game.object(source)?.incarnation;
            let target_incarnation = game.object(target)?.incarnation;
            let timestamp = game.next_timestamp;
            game.next_timestamp =
                game.next_timestamp
                    .checked_add(1)
                    .ok_or(RulesError::IllegalAction(
                        "copy-effect timestamp counter overflowed",
                    ))?;
            game.objects
                .get_mut(&target)
                .ok_or(RulesError::UnknownCard(target))?
                .copied_permanent = Some(CopiedPermanent {
                values,
                source,
                source_incarnation,
                timestamp,
            });
            game.record_event(GameEvent::PermanentCopied {
                source,
                source_incarnation,
                target,
                target_incarnation,
                timestamp,
            });
            // A copy can make a creature noncreature or reduce its toughness;
            // reach the ordinary SBA fixed point before exposing the result.
            game.check_state_based_actions()?;
            Ok(())
        })
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
                incarnation: 1,
                owner,
                controller: owner,
                tapped: false,
                damage: 0,
                damage_shield: 0,
                counters: BTreeMap::new(),
                attached_to: None,
                attached_to_incarnation: None,
                entered_turn: self.turn,
                controller_changed_turn: self.turn,
                token: None,
                copied_permanent: None,
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
        object.controller_changed_turn = entered_turn;
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
        if self.controller_of(land)? != player || object.tapped {
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
            game.activate_ability_impl(player, activation, None)?;
            game.flush_pending_dies_triggers();
            game.check_state_based_actions()?;
            game.validate_invariants()
        })
    }

    /// Activates a bound nonmana ability with an explicit allocation for every
    /// generic or hybrid mana symbol in its calculated effective cost.
    ///
    /// The legacy [`Self::activate_ability`] method remains the deterministic
    /// compatibility entry point. Both entry points use the same cost context,
    /// live increases/reductions, and all-or-error cost transaction.
    #[allow(clippy::needless_pass_by_value)] // The selected allocation crosses the transaction boundary.
    pub fn activate_ability_with_mana_spend(
        &mut self,
        player: PlayerId,
        activation: AbilityActivation,
        selection: ManaPaymentSelection,
    ) -> Result<(), RulesError> {
        self.atomic_transition(|game| {
            game.require_priority(player)?;
            game.activate_ability_impl(player, activation, Some(&selection))?;
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
        mana_payment_selection: Option<&ManaPaymentSelection>,
    ) -> Result<(), RulesError> {
        self.require_zone(activation.source, Zone::Battlefield)?;
        let source = self.object(activation.source)?.clone();
        if self.controller_of(activation.source)? != player {
            return Err(RulesError::IllegalAction(
                "activated ability source must be controlled by its activator",
            ));
        }
        if self.nonmana_activated_abilities_suppressed(activation.source) {
            return Err(RulesError::IllegalAction(
                "this permanent's nonmana activated abilities are suppressed",
            ));
        }
        let definition_id =
            self.effective_definition_id(activation.source)?
                .ok_or(RulesError::IllegalAction(
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
        // Preserve the ability source's current colors before any activation
        // cost can move it away and remove its continuous effects.  Later
        // protection checks must use this source-incarnation snapshot.
        let source_colors = self.characteristics(activation.source)?.colors;
        if ability.sorcery_speed
            && (player != self.active_player || !self.step.is_main() || !self.stack.is_empty())
        {
            return Err(RulesError::IllegalAction(
                "this activated ability is allowed only during your main phase with an empty stack",
            ));
        }
        if activation.targets.len() != ability.targets.len() {
            return Err(RulesError::IllegalAction(
                "activated ability target count does not match its definition",
            ));
        }
        for (target, requirement) in activation.targets.iter().zip(&ability.targets) {
            if !Self::target_shape_matches(*target, *requirement)
                || !self.target_matches_for_source(player, activation.source, *target, *requirement)
            {
                return Err(RulesError::IllegalTarget(*target));
            }
        }
        if activation.additional_tap_creatures.len()
            != usize::from(ability.additional_tap_creatures)
        {
            return Err(RulesError::IllegalAction(
                "activated ability additional tap selection count does not match its definition",
            ));
        }
        let mut selected_taps = BTreeSet::new();
        for permanent in &activation.additional_tap_creatures {
            if *permanent == activation.source {
                return Err(RulesError::IllegalAction(
                    "an additional creature tap cost cannot name the ability source",
                ));
            }
            if !selected_taps.insert(*permanent) {
                return Err(RulesError::IllegalAction(
                    "an activated ability cannot tap the same additional creature twice",
                ));
            }
            self.require_zone(*permanent, Zone::Battlefield)?;
            let object = self.object(*permanent)?;
            if self.controller_of(*permanent)? != player {
                return Err(RulesError::IllegalAction(
                    "an activated ability can tap only a controlled additional creature",
                ));
            }
            if object.tapped {
                return Err(RulesError::IllegalAction(
                    "an activated ability requires each additional creature to be untapped",
                ));
            }
            if !self
                .characteristics(*permanent)?
                .card_types
                .contains(&CardType::Creature)
            {
                return Err(RulesError::IllegalAction(
                    "an activated ability additional tap cost requires a creature",
                ));
            }
        }
        let expected_sacrifices = usize::from(ability.sacrifice_source)
            + usize::from(ability.sacrifice_creatures)
            + usize::from(ability.sacrifice_lands);
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
            if self.controller_of(*permanent)? != player {
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
            } else if index
                < usize::from(ability.sacrifice_source) + usize::from(ability.sacrifice_creatures)
            {
                if !self
                    .characteristics(*permanent)?
                    .card_types
                    .contains(&CardType::Creature)
                {
                    return Err(RulesError::IllegalAction(
                        "this activated ability requires a sacrificed creature",
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
        let cost_context = self.calculate_activated_ability_cost(
            player,
            activation.source,
            ability.id,
            ActivatedAbilityKind::NonMana,
            &ability.mana_cost,
            ability.additional_tap_creatures,
            ability.sacrifice_source,
            ability.sacrifice_creatures,
            ability.sacrifice_lands,
            ability.discard_cards,
            mana_payment_selection.cloned(),
        )?;
        let mut paid_pool = self.players[player.0].mana_pool.clone();
        if let Some(selection) = mana_payment_selection {
            paid_pool
                .pay_selected(&cost_context.effective_mana_cost, selection)
                .map_err(RulesError::Mana)?;
        } else {
            paid_pool
                .pay(&cost_context.effective_mana_cost)
                .map_err(RulesError::Mana)?;
        }
        if ability.tap_cost {
            if source.tapped {
                return Err(RulesError::IllegalAction(
                    "activated ability requires an untapped source",
                ));
            }
            let characteristics = self.characteristics(activation.source)?;
            if characteristics.card_types.contains(&CardType::Creature)
                && source.controller_changed_turn >= self.turn
                && !characteristics.keywords.contains(&Keyword::Haste)
            {
                return Err(RulesError::IllegalAction(
                    "a summoning-sick creature cannot pay an activated tap cost",
                ));
            }
        }
        self.players[player.0].mana_pool = paid_pool;
        self.record_activated_ability_cost_context_if_modified(cost_context.clone());
        if cost_context.effective_mana_cost.mana_value() > 0 {
            self.record_event(GameEvent::AbilityManaPaid {
                player,
                source: activation.source,
                ability: ability.id,
                mana_cost: cost_context.effective_mana_cost,
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
        for permanent in &activation.additional_tap_creatures {
            self.objects
                .get_mut(permanent)
                .ok_or(RulesError::UnknownCard(*permanent))?
                .tapped = true;
            self.record_event(GameEvent::AdditionalCreatureTappedAsAbilityCost {
                player,
                source: activation.source,
                permanent: *permanent,
            });
        }
        let target_incarnations = self.target_incarnations(&activation.targets);
        self.stack.push(StackObject {
            card: activation.source,
            source_incarnation: source.incarnation,
            source_colors,
            controller: player,
            ability_id: Some(ability.id),
            targets: activation.targets,
            target_incarnations,
            effects: ability.effects,
            chosen_x: None,
            mana_spent: None,
            convoke_symbols: 0,
            generic_cost_reduction: 0,
        });
        self.record_event(GameEvent::AbilityActivated {
            player,
            source: activation.source,
            source_incarnation: source.incarnation,
            definition: definition_id,
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
        if self.controller_of(activation.source)? != player {
            return Err(RulesError::IllegalAction(
                "mana ability source must be controlled by its activator",
            ));
        }
        let definition =
            self.effective_definition_id(activation.source)?
                .ok_or(RulesError::IllegalAction(
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
        let base_mana_cost = match &ability.output {
            ManaAbilityOutput::PaidBundle { mana_cost, .. } => mana_cost.clone(),
            ManaAbilityOutput::Fixed(_)
            | ManaAbilityOutput::Choice(_)
            | ManaAbilityOutput::Bundle(_) => ManaCost::new(0),
        };
        let cost_context = self.calculate_activated_ability_cost(
            player,
            activation.source,
            ability.id,
            ActivatedAbilityKind::Mana,
            &base_mana_cost,
            0,
            false,
            0,
            0,
            0,
            None,
        )?;
        let mut mana_pool_after_payment = self.players[player.0].mana_pool.clone();
        mana_pool_after_payment
            .pay(&cost_context.effective_mana_cost)
            .map_err(RulesError::Mana)?;
        let paid_bundle = match &ability.output {
            ManaAbilityOutput::PaidBundle { bundle, .. } => {
                if activation.chosen_color.is_some() {
                    return Err(RulesError::IllegalAction(
                        "fixed mana bundle ability does not accept a color choice",
                    ));
                }
                for (color, amount) in bundle.iter() {
                    if !mana_pool_after_payment.can_add(color, amount) {
                        return Err(RulesError::IllegalAction(
                            "mana pool cannot hold the produced mana",
                        ));
                    }
                }
                Some((
                    cost_context.effective_mana_cost.clone(),
                    bundle.clone(),
                    mana_pool_after_payment.clone(),
                ))
            }
            ManaAbilityOutput::Fixed(_)
            | ManaAbilityOutput::Choice(_)
            | ManaAbilityOutput::Bundle(_) => None,
        };
        let free_bundle = match &ability.output {
            ManaAbilityOutput::Bundle(bundle) => {
                if activation.chosen_color.is_some() {
                    return Err(RulesError::IllegalAction(
                        "fixed mana bundle ability does not accept a color choice",
                    ));
                }
                if bundle
                    .iter()
                    .any(|(color, amount)| !mana_pool_after_payment.can_add(color, amount))
                {
                    return Err(RulesError::IllegalAction(
                        "mana pool cannot hold the produced mana",
                    ));
                }
                Some(bundle.clone())
            }
            ManaAbilityOutput::Fixed(_)
            | ManaAbilityOutput::Choice(_)
            | ManaAbilityOutput::PaidBundle { .. } => None,
        };
        let color = if paid_bundle.is_none() && free_bundle.is_none() {
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
                && source.controller_changed_turn >= self.turn
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
        self.record_activated_ability_cost_context_if_modified(cost_context);

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
        } else if let Some(bundle) = free_bundle {
            self.players[player.0].mana_pool = mana_pool_after_payment;
            self.record_event(GameEvent::BoundManaAbilityFreeBundleActivated {
                player,
                source: activation.source,
                ability: ability.id,
                bundle: bundle.clone(),
                tapped: ability.tap_cost,
                life_payment: ability.life_payment,
            });
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
            self.players[player.0].mana_pool = mana_pool_after_payment;
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
        self.object(card)?;
        Ok(self
            .effective_definition_id(card)?
            .and_then(|definition| self.basic_land_types.get(definition).copied()))
    }

    fn player_controls_basic_land_type(&self, player: PlayerId, land_type: BasicLandType) -> bool {
        self.all_battlefield_cards().into_iter().any(|card| {
            self.controller_of(card) == Ok(player)
                && self
                    .basic_land_type(card)
                    .is_ok_and(|registered| registered == Some(land_type))
        })
    }

    fn controller_saprolings_cannot_block(&self, player: PlayerId) -> bool {
        self.all_battlefield_cards().into_iter().any(|source| {
            self.controller_of(source) == Ok(player)
                && self.characteristics(source).is_ok_and(|characteristics| {
                    characteristics
                        .keywords
                        .contains(&Keyword::SaprolingsCannotBlock)
                })
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

    /// Returns whether either combat permanent has protection from the other
    /// permanent's current colors. Protection prevents blocking in both
    /// directions: a protected attacker cannot be blocked by a matching-color
    /// creature, and a protected blocker cannot block a matching-color
    /// attacker.
    fn protection_prevents_block(&self, blocker: ObjectId, attacker: ObjectId) -> bool {
        let Ok(blocker_characteristics) = self.characteristics(blocker) else {
            return false;
        };
        let Ok(attacker_characteristics) = self.characteristics(attacker) else {
            return false;
        };
        self.permanent_has_protection_from_colors(blocker, &attacker_characteristics.colors)
            || self.permanent_has_protection_from_colors(attacker, &blocker_characteristics.colors)
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
        let own_battlefield = self
            .all_battlefield_cards()
            .into_iter()
            .filter(|card| self.controller_of(*card) == Ok(player))
            .map(|card| self.card_view(card))
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
        let private_library_choice = self
            .pending_private_library_choice
            .as_ref()
            .filter(|choice| choice.controller == player)
            .map(|choice| {
                choice
                    .cards
                    .iter()
                    .map(|card| self.card_view(*card))
                    .collect::<Result<Vec<_>, _>>()
                    .map(|cards| PrivateLibraryChoiceView {
                        spell: choice.spell,
                        cards,
                        life_per_card: choice.life_per_card,
                    })
            })
            .transpose()?;
        let private_opponent_library_choice = self
            .pending_private_opponent_library_exile_choice
            .as_ref()
            .filter(|choice| choice.controller == player)
            .map(|choice| {
                choice
                    .cards
                    .iter()
                    .map(|card| self.card_view(*card))
                    .collect::<Result<Vec<_>, _>>()
                    .map(|cards| PrivateOpponentLibraryChoiceView {
                        source: choice.source,
                        ability: choice.ability,
                        cards,
                    })
            })
            .transpose()?;
        let pending_decision = self
            .pending_decision
            .as_ref()
            .filter(|decision| decision.player == player)
            .map(|decision| {
                self.decision_candidate_cards(decision)
                    .map(|candidates| PendingDecisionView {
                        id: decision.id,
                        kind: decision.kind,
                        visibility: decision.visibility,
                        min_selections: decision.min_selections,
                        max_selections: decision.max_selections,
                        candidates,
                    })
            })
            .transpose()?;
        let library_search_choice = self
            .pending_decision
            .as_ref()
            .filter(|decision| decision.player == player)
            .and_then(|decision| match &decision.continuation {
                DecisionContinuation::LibrarySearch {
                    source,
                    destination,
                    may_fail_to_find,
                    ..
                } => Some((*source, *destination, *may_fail_to_find)),
                DecisionContinuation::TriggeredEffectObject { .. } => None,
            })
            .map(|(source, destination, may_fail_to_find)| {
                self.decision_candidate_cards(
                    self.pending_decision
                        .as_ref()
                        .expect("decision continuation came from pending state"),
                )
                .map(|cards| LibrarySearchChoiceView {
                    source,
                    cards,
                    destination,
                    may_fail_to_find,
                })
            })
            .transpose()?;
        let triggered_ability_target_choice = self
            .pending_trigger_target_choices
            .first()
            .filter(|choice| choice.controller == player)
            .map(|choice| TriggeredAbilityTargetChoiceView {
                source: choice.source,
                ability: choice.ability.id,
                target_options: choice
                    .ability
                    .targets
                    .iter()
                    .map(|requirement| {
                        self.legal_trigger_targets_for_colors(
                            choice.controller,
                            &choice.source_colors,
                            *requirement,
                        )
                    })
                    .collect(),
            });
        let optional_triggered_ability_choice = self
            .pending_optional_trigger_choice
            .as_ref()
            .filter(|choice| choice.controller == player)
            .map(|choice| {
                let conditional_targets = Self::optional_trigger_target_requirement(
                    &choice.ability,
                )
                .map_or_else(Vec::new, |requirement| {
                    self.legal_trigger_targets_for_colors(
                        choice.controller,
                        &choice.source_colors,
                        requirement,
                    )
                });
                let mut pool = self.players[choice.controller.0].mana_pool.clone();
                OptionalTriggeredAbilityChoiceView {
                    source: choice.source,
                    ability: choice.ability.id,
                    mana_cost: choice.ability.mana_cost.clone(),
                    can_pay: pool.pay(&choice.ability.mana_cost).is_ok(),
                    conditional_targets,
                }
            });
        let triggered_ability_effect_object_choice = self
            .pending_decision
            .as_ref()
            .filter(|decision| decision.player == player)
            .and_then(|decision| match &decision.continuation {
                DecisionContinuation::TriggeredEffectObject {
                    source, ability, ..
                } => Some((*source, *ability)),
                DecisionContinuation::LibrarySearch { .. } => None,
            })
            .map(|(source, ability)| {
                self.decision_candidate_cards(
                    self.pending_decision
                        .as_ref()
                        .expect("decision continuation came from pending state"),
                )
                .map(|candidates| TriggeredAbilityEffectObjectChoiceView {
                    source,
                    ability,
                    candidates,
                })
            })
            .transpose()?;
        let damage_replacement_choice = self
            .pending_damage_replacement_choice
            .as_ref()
            .filter(|choice| choice.affected_player == player)
            .map(|choice| {
                Ok(DamageReplacementChoiceView {
                    source: choice.source,
                    source_incarnation: choice.source_incarnation,
                    target: choice.target,
                    target_incarnation: choice.target_incarnation,
                    amount: choice.amount,
                    replacements: self.damage_replacement_candidates(
                        choice.source,
                        choice.target,
                        choice.amount,
                        &choice.used,
                    )?,
                })
            })
            .transpose()?;
        let mut opponent_life = Vec::new();
        let mut opponent_battlefield = Vec::new();
        for opponent in self.players.iter().filter(|candidate| {
            // A departed seat is not a current opponent. In particular, a
            // policy must not be handed a life-total target that the rules
            // layer will reject as eliminated.
            candidate.id != player && !candidate.lost
        }) {
            opponent_life.push((opponent.id, opponent.life));
        }
        opponent_battlefield.extend(
            self.all_battlefield_cards()
                .into_iter()
                .filter(|card| {
                    self.controller_of(*card)
                        .is_ok_and(|controller| controller != player)
                })
                .map(|card| self.card_view(card))
                .collect::<Result<Vec<_>, _>>()?,
        );
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
            private_library_choice,
            private_opponent_library_choice,
            library_search_choice,
            triggered_ability_target_choice,
            optional_triggered_ability_choice,
            triggered_ability_effect_object_choice,
            pending_decision,
            damage_replacement_choice,
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
    #[allow(clippy::too_many_lines)] // One public action boundary centralizes invariant-audited dispatch.
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
            PolicyAction::ChoosePrivateLibraryCards { spell, selected } => {
                self.choose_private_library_cards(player, spell, selected)?;
            }
            PolicyAction::ChoosePrivateOpponentLibraryCardToExile {
                source,
                ability,
                selected,
            } => {
                self.choose_private_opponent_library_card_to_exile(
                    player, source, ability, selected,
                )?;
            }
            PolicyAction::ChooseLibrarySearchCard { source, selected } => {
                self.choose_library_search_card(player, source, selected)?;
            }
            PolicyAction::ChooseTriggeredAbilityTargets {
                source,
                ability,
                targets,
            } => {
                self.choose_triggered_ability_targets(player, source, ability, targets)?;
            }
            PolicyAction::ChooseTriggeredAbilityEffectObject {
                source,
                ability,
                selected,
            } => {
                self.choose_triggered_ability_effect_object(player, source, ability, selected)?;
            }
            PolicyAction::ChooseDamageReplacement {
                source,
                source_incarnation,
                target,
                replacement,
            } => {
                self.choose_damage_replacement(
                    player,
                    source,
                    source_incarnation,
                    target,
                    replacement,
                )?;
            }
            PolicyAction::ResolveOptionalTriggeredAbility {
                source,
                ability,
                pay,
                target,
            } => {
                self.resolve_optional_triggered_ability(player, source, ability, pay, target)?;
            }
            PolicyAction::SubmitDecision {
                decision,
                selection,
            } => self.submit_decision(player, decision, selection)?,
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

    #[allow(clippy::too_many_lines)] // One pure layer derivation is easier to audit in order.
    pub fn characteristics(&self, card: ObjectId) -> Result<Characteristics, RulesError> {
        let object = self.object(card)?;
        // Begin with layer-one copiable values.  This deliberately precedes
        // all static and timestamped continuous effects, then counters at
        // layer seven; no derived runtime state of the copy source is read.
        let mut characteristics = match self.copiable_values(card)? {
            CopiableValues::Token(token) => Characteristics {
                colors: token.colors,
                card_types: token.card_types,
                creature_subtypes: token.creature_subtypes,
                power: Some(i32::from(token.power)),
                toughness: Some(i32::from(token.toughness)),
                keywords: token.keywords,
            },
            CopiableValues::CardDefinition(definition_id) => {
                let definition = self
                    .catalog
                    .get(definition_id)
                    .ok_or(RulesError::UnknownDefinition(definition_id))?;
                Characteristics {
                    colors: definition.colors.clone(),
                    card_types: definition.card_types.clone(),
                    creature_subtypes: BTreeSet::new(),
                    power: definition.power.map(i32::from),
                    toughness: definition.toughness.map(i32::from),
                    keywords: definition.keywords.clone(),
                }
            }
        };
        if self.zone_of(card) == Some(Zone::Battlefield) {
            // Static bindings are keyed by source definition, but their
            // recipient can be another permanent. Iterate all live sources
            // rather than only the queried card so controller-scoped anthems
            // do not disappear from their intended recipients.
            for source in self.all_battlefield_cards() {
                let Some(definition) = self.effective_definition_id(source)? else {
                    continue;
                };
                let Some(changes) = self.static_continuous_effects.get(definition) else {
                    continue;
                };
                for change in changes {
                    self.apply_static_continuous_change(
                        source,
                        card,
                        &mut characteristics,
                        change,
                    )?;
                }
            }
        }
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
                ContinuousChange::ChangeController(_)
                | ContinuousChange::CannotBlockSource(_)
                | ContinuousChange::AddDamageShield(_)
                | ContinuousChange::SuppressNonManaActivatedAbilities => {}
                ContinuousChange::ModifyPowerToughness { power, toughness } => {
                    characteristics.power = characteristics
                        .power
                        .map(|current| current + i32::from(*power));
                    characteristics.toughness = characteristics
                        .toughness
                        .map(|current| current + i32::from(*toughness));
                }
                ContinuousChange::ControlledCreatureCountPowerToughness
                | ContinuousChange::OtherControlledCreaturesModifyPowerToughness { .. }
                | ContinuousChange::OtherControlledCreaturesAddKeyword(_)
                | ContinuousChange::ControlledCreaturesAddKeywordIfSourceEnchanted(_) => {
                    return Err(RulesError::IllegalAction(
                        "a static continuous change cannot be a timestamped effect",
                    ));
                }
            }
        }
        // Only the two P/T counter kinds modify characteristics. Every other
        // typed named counter is still real permanent state, but it carries
        // no implicit characteristic rule in this bounded substrate.
        let plus_one = object
            .counters
            .get(&CounterKind::PlusOnePlusOne)
            .copied()
            .unwrap_or_default();
        let minus_one = object
            .counters
            .get(&CounterKind::MinusOneMinusOne)
            .copied()
            .unwrap_or_default();
        characteristics.power = characteristics
            .power
            .map(|power| power + i32::from(plus_one) - i32::from(minus_one));
        characteristics.toughness = characteristics
            .toughness
            .map(|toughness| toughness + i32::from(plus_one) - i32::from(minus_one));
        Ok(characteristics)
    }

    fn apply_static_continuous_change(
        &self,
        source: ObjectId,
        card: ObjectId,
        characteristics: &mut Characteristics,
        change: &ContinuousChange,
    ) -> Result<(), RulesError> {
        match change {
            ContinuousChange::ControlledCreatureCountPowerToughness => {
                if source != card {
                    return Ok(());
                }
                let controller = self.controller_of(card)?;
                let count =
                    i32::try_from(self.controlled_creature_count(controller)).map_err(|_| {
                        RulesError::IllegalAction(
                            "controlled creature count exceeds supported range",
                        )
                    })?;
                characteristics.power = Some(count);
                characteristics.toughness = Some(count);
                Ok(())
            }
            ContinuousChange::OtherControlledCreaturesModifyPowerToughness { power, toughness } => {
                if source == card
                    || self.controller_of(source)? != self.controller_of(card)?
                    || !characteristics.card_types.contains(&CardType::Creature)
                {
                    return Ok(());
                }
                characteristics.power = characteristics
                    .power
                    .map(|current| current + i32::from(*power));
                characteristics.toughness = characteristics
                    .toughness
                    .map(|current| current + i32::from(*toughness));
                Ok(())
            }
            ContinuousChange::OtherControlledCreaturesAddKeyword(keyword) => {
                if source == card
                    || self.controller_of(source)? != self.controller_of(card)?
                    || !characteristics.card_types.contains(&CardType::Creature)
                {
                    return Ok(());
                }
                if !characteristics.keywords.contains(keyword) {
                    characteristics.keywords.push(keyword.clone());
                }
                Ok(())
            }
            ContinuousChange::ControlledCreaturesAddKeywordIfSourceEnchanted(keyword) => {
                if self.controller_of(source)? != self.controller_of(card)?
                    || !characteristics.card_types.contains(&CardType::Creature)
                    || !self.source_has_live_aura_attachment(source)?
                {
                    return Ok(());
                }
                if !characteristics.keywords.contains(keyword) {
                    characteristics.keywords.push(keyword.clone());
                }
                Ok(())
            }
            _ => Err(RulesError::IllegalAction(
                "unsupported static continuous change",
            )),
        }
    }

    fn source_has_live_aura_attachment(&self, source: ObjectId) -> Result<bool, RulesError> {
        for aura in self.all_battlefield_cards() {
            if self.is_aura_like(aura)?
                && self.object(aura)?.attached_to == Some(source)
                && self
                    .object(aura)?
                    .attached_to_incarnation
                    .is_some_and(|incarnation| self.object_has_incarnation(source, incarnation))
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn nonmana_activated_abilities_suppressed(&self, source: ObjectId) -> bool {
        self.continuous_effects.iter().any(|effect| {
            effect.target == source
                && self.effect_is_active(effect)
                && matches!(
                    effect.change,
                    ContinuousChange::SuppressNonManaActivatedAbilities
                )
        })
    }

    fn controlled_creature_count(&self, controller: PlayerId) -> usize {
        self.all_battlefield_cards()
            .into_iter()
            .filter(|card| self.controller_of(*card) == Ok(controller))
            .filter(|card| {
                self.objects.get(card).is_some_and(|object| {
                    object.token.as_ref().map_or_else(
                        || {
                            self.effective_definition_id(*card)
                                .ok()
                                .flatten()
                                .and_then(|definition| self.catalog.get(definition))
                                .is_some_and(CardDefinition::is_creature)
                        },
                        |token| token.card_types.contains(&CardType::Creature),
                    )
                })
            })
            .count()
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
        if matches!(change, ContinuousChange::AddColor(Color::Colorless)) {
            return Err(RulesError::IllegalAction(
                "continuous effects may not add the colorless mana kind as a card color",
            ));
        }
        if matches!(
            change,
            ContinuousChange::ControlledCreatureCountPowerToughness
                | ContinuousChange::OtherControlledCreaturesModifyPowerToughness { .. }
                | ContinuousChange::OtherControlledCreaturesAddKeyword(_)
                | ContinuousChange::ControlledCreaturesAddKeywordIfSourceEnchanted(_)
        ) {
            return Err(RulesError::IllegalAction(
                "a static continuous change cannot be installed dynamically",
            ));
        }
        if let ContinuousChange::ChangeController(controller) = &change {
            if self.player(*controller)?.lost {
                return Err(RulesError::IllegalAction(
                    "a departed player cannot control a permanent",
                ));
            }
        }
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
        let control_destination = match &change {
            ContinuousChange::ChangeController(controller) => Some(*controller),
            _ => None,
        };
        let source_incarnation = self.object(source)?.incarnation;
        let target_incarnation = self.object(target)?.incarnation;
        let controller_before = self.controller_of(target)?;
        if let ContinuousChange::AddDamageShield(amount) = &change {
            self.objects
                .get_mut(&target)
                .ok_or(RulesError::UnknownCard(target))?
                .damage_shield += i32::from(*amount);
        }
        self.continuous_effects.push(ContinuousEffect {
            source,
            source_incarnation,
            target,
            target_incarnation,
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
        if let Some(controller) = control_destination {
            if controller_before != controller {
                self.objects
                    .get_mut(&target)
                    .ok_or(RulesError::UnknownCard(target))?
                    .controller_changed_turn = self.turn;
                self.record_event(GameEvent::ControllerChanged {
                    source,
                    target,
                    from: controller_before,
                    to: controller,
                });
            }
        }
        Ok(())
    }

    /// Completes an Aura-like permanent spell's successful resolution. This
    /// runs only after the source has entered the battlefield, so the
    /// permanent continuous-effect invariants apply from its first public
    /// attachment receipt onward.
    fn attach_aura_with_changes(
        &mut self,
        aura: ObjectId,
        target: ObjectId,
        requirement: TargetRequirement,
        changes: Vec<ContinuousChange>,
    ) -> Result<(), RulesError> {
        self.require_zone(aura, Zone::Battlefield)?;
        if !self.is_aura_like(aura)? || !self.target_matches(Target::Permanent(target), requirement)
        {
            return Err(RulesError::IllegalTarget(Target::Permanent(target)));
        }
        let target_incarnation = self.object(target)?.incarnation;
        let object = self
            .objects
            .get_mut(&aura)
            .ok_or(RulesError::UnknownCard(aura))?;
        if object.attached_to.replace(target).is_some() {
            return Err(RulesError::IllegalAction(
                "an aura-like permanent was already attached",
            ));
        }
        object.attached_to_incarnation = Some(target_incarnation);
        for change in changes {
            self.install_continuous_effect(aura, target, change, Duration::Permanent)?;
        }
        self.record_event(GameEvent::AuraAttached { aura, target });
        Ok(())
    }

    fn is_aura_like(&self, card: ObjectId) -> Result<bool, RulesError> {
        if self.object(card)?.token.is_some() {
            return Ok(false);
        }
        let Some(definition) = self.effective_definition_id(card)? else {
            return Ok(false);
        };
        Ok(self.catalog[definition].effects.iter().any(|effect| {
            matches!(
                effect,
                Effect::AttachSourceAndModifyTargetPt { .. } | Effect::AttachSourceToTarget { .. }
            )
        }))
    }

    fn aura_attachment_requirement(
        &self,
        card: ObjectId,
    ) -> Result<Option<TargetRequirement>, RulesError> {
        if self.object(card)?.token.is_some() {
            return Ok(None);
        }
        let Some(definition) = self.effective_definition_id(card)? else {
            return Ok(None);
        };
        Ok(self.catalog[definition]
            .effects
            .iter()
            .find_map(|effect| match effect {
                Effect::AttachSourceAndModifyTargetPt { .. } => Some(TargetRequirement::Creature),
                Effect::AttachSourceToTarget { target, .. } => Some(*target),
                _ => None,
            }))
    }

    fn aura_attachment_spec(
        &self,
        card: ObjectId,
    ) -> Result<Option<(TargetRequirement, Vec<ContinuousChange>)>, RulesError> {
        if self.object(card)?.token.is_some() {
            return Ok(None);
        }
        Ok(self
            .card_definition(card)?
            .effects
            .iter()
            .find_map(|effect| match effect {
                Effect::AttachSourceAndModifyTargetPt { power, toughness } => Some((
                    TargetRequirement::Creature,
                    vec![ContinuousChange::ModifyPowerToughness {
                        power: *power,
                        toughness: *toughness,
                    }],
                )),
                Effect::AttachSourceToTarget { target, changes } => {
                    Some((*target, changes.clone()))
                }
                _ => None,
            }))
    }

    /// Captures an Aura-relative linked-exile group. The source-relative
    /// identity check is deliberately a no-op for a departed source: a later
    /// incarnation with the same stable id may not start a historical
    /// ability's exile group.
    #[allow(clippy::too_many_lines)] // One transaction preserves the group snapshot and ordered zone receipts.
    fn exile_attached_creature_and_auras_until_end_step(
        &mut self,
        source: ObjectId,
        source_incarnation: u64,
        controller: PlayerId,
    ) -> Result<(), RulesError> {
        if self.zone_of(source) != Some(Zone::Battlefield)
            || !self.object_has_incarnation(source, source_incarnation)
        {
            return Ok(());
        }
        if !self.is_aura_like(source)? {
            return Err(RulesError::IllegalAction(
                "linked exile requires an Aura-like permanent source",
            ));
        }
        let source_object = self.object(source)?.clone();
        if self.controller_of(source)? != controller {
            return Err(RulesError::IllegalAction(
                "linked exile source controller does not match resolving ability",
            ));
        }
        let Some(primary) = source_object.attached_to else {
            return Ok(());
        };
        let Some(primary_incarnation) = source_object.attached_to_incarnation else {
            return Ok(());
        };
        if self.zone_of(primary) != Some(Zone::Battlefield)
            || !self.object_has_incarnation(primary, primary_incarnation)
            || !self
                .characteristics(primary)?
                .card_types
                .contains(&CardType::Creature)
        {
            return Ok(());
        }

        let mut attached_auras = self
            .all_battlefield_cards()
            .into_iter()
            .filter(|candidate| {
                self.is_aura_like(*candidate).is_ok_and(|is_aura| is_aura)
                    && self.object(*candidate).is_ok_and(|object| {
                        object.attached_to == Some(primary)
                            && object.attached_to_incarnation == Some(primary_incarnation)
                    })
            })
            .collect::<Vec<_>>();
        attached_auras.sort_unstable();
        if !attached_auras.contains(&source) {
            return Err(RulesError::IllegalAction(
                "linked exile source lost its exact Aura attachment",
            ));
        }

        self.move_to_zone(primary, Zone::Exile)?;
        let mut members = vec![LinkedExileMember {
            object: primary,
            exile_incarnation: self.object(primary)?.incarnation,
            role: LinkedExileMemberRole::PrimaryCreature,
        }];
        for aura in attached_auras {
            self.move_to_zone(aura, Zone::Exile)?;
            members.push(LinkedExileMember {
                object: aura,
                exile_incarnation: self.object(aura)?.incarnation,
                role: LinkedExileMemberRole::AttachedAura,
            });
        }

        let group = LinkedExileGroupId(self.next_linked_exile_group_id);
        self.next_linked_exile_group_id =
            self.next_linked_exile_group_id
                .checked_add(1)
                .ok_or(RulesError::IllegalAction(
                    "linked exile group id overflowed",
                ))?;
        let action = DelayedActionId(self.next_delayed_action_id);
        self.next_delayed_action_id = self
            .next_delayed_action_id
            .checked_add(1)
            .ok_or(RulesError::IllegalAction("delayed action id overflowed"))?;
        let due_turn = if self.step == Step::End {
            self.turn
                .checked_add(1)
                .ok_or(RulesError::IllegalAction("delayed action turn overflowed"))?
        } else {
            self.turn
        };
        let linked_group = LinkedExileGroup {
            id: group,
            controller,
            source,
            source_incarnation,
            members: members.clone(),
        };
        self.linked_exile_groups.insert(group, linked_group);
        self.delayed_actions.push(DelayedAction {
            id: action,
            timing: DelayedActionTiming::EndStep,
            due_turn,
            controller,
            kind: DelayedActionKind::ReturnLinkedExileGroup { group },
        });
        self.record_event(GameEvent::DelayedActionScheduled {
            action,
            timing: DelayedActionTiming::EndStep,
            due_turn,
            controller,
            group,
            members,
        });
        Ok(())
    }

    fn member_is_still_in_linked_exile(&self, member: LinkedExileMember) -> bool {
        self.zone_of(member.object) == Some(Zone::Exile)
            && self.object_has_incarnation(member.object, member.exile_incarnation)
    }

    /// Returns every group member that still names the exact exile
    /// incarnation owned by the delayed action. The primary creature enters
    /// first; attached Auras follow stable object-id order and attach only to
    /// that new creature incarnation if their ordinary attachment restriction
    /// remains legal. A member that left exile independently is never moved.
    fn return_linked_exile_group(
        &mut self,
        group: &LinkedExileGroup,
    ) -> Result<Vec<ObjectId>, RulesError> {
        let primary = group
            .members
            .iter()
            .copied()
            .find(|member| member.role == LinkedExileMemberRole::PrimaryCreature)
            .ok_or(RulesError::IllegalAction(
                "linked exile group lacks its primary creature",
            ))?;
        let mut returned = Vec::new();
        let returned_primary = if self.member_is_still_in_linked_exile(primary) {
            self.move_to_zone(primary.object, Zone::Battlefield)?;
            returned.push(primary.object);
            Some(primary.object)
        } else {
            None
        };

        let mut auras = group
            .members
            .iter()
            .copied()
            .filter(|member| member.role == LinkedExileMemberRole::AttachedAura)
            .collect::<Vec<_>>();
        auras.sort_by_key(|member| member.object);
        for aura in auras {
            if !self.member_is_still_in_linked_exile(aura) {
                continue;
            }
            self.move_to_zone(aura.object, Zone::Battlefield)?;
            returned.push(aura.object);
            if let Some(primary) = returned_primary {
                if let Some((requirement, changes)) = self.aura_attachment_spec(aura.object)? {
                    let controller = self.controller_of(aura.object)?;
                    if self.target_matches_for_source(
                        controller,
                        aura.object,
                        Target::Permanent(primary),
                        requirement,
                    ) {
                        self.attach_aura_with_changes(aura.object, primary, requirement, changes)?;
                    }
                }
            }
        }
        Ok(returned)
    }

    fn consume_due_delayed_actions(&mut self) -> Result<(), RulesError> {
        if self.step != Step::End {
            return Ok(());
        }
        let due = self
            .delayed_actions
            .iter()
            .copied()
            .filter(|action| {
                action.timing == DelayedActionTiming::EndStep && action.due_turn == self.turn
            })
            .collect::<Vec<_>>();
        self.delayed_actions.retain(|action| {
            !(action.timing == DelayedActionTiming::EndStep && action.due_turn == self.turn)
        });
        for action in due {
            let DelayedActionKind::ReturnLinkedExileGroup { group } = action.kind;
            let group =
                self.linked_exile_groups
                    .remove(&group)
                    .ok_or(RulesError::IllegalAction(
                        "delayed action references no linked exile group",
                    ))?;
            let returned = self.return_linked_exile_group(&group)?;
            self.record_event(GameEvent::DelayedActionConsumed {
                action: action.id,
                group: group.id,
                returned,
            });
        }
        self.check_state_based_actions()?;
        Ok(())
    }

    pub fn play_land(&mut self, player: PlayerId, card: ObjectId) -> Result<(), RulesError> {
        self.atomic_transition(|game| game.play_land_impl(player, card))
    }

    fn play_land_impl(&mut self, player: PlayerId, card: ObjectId) -> Result<(), RulesError> {
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
        let definition_id = definition.id;
        self.players[player.0].lands_played += 1;
        self.move_to_zone(card, Zone::Battlefield)?;
        if self
            .land_entry_behaviors
            .get(definition_id)
            .is_some_and(|behavior| behavior.enters_tapped)
        {
            self.objects
                .get_mut(&card)
                .ok_or(RulesError::UnknownCard(card))?
                .tapped = true;
        }
        self.consecutive_passes = 0;
        self.check_state_based_actions()?;
        self.flush_pending_dies_triggers();
        self.enqueue_enter_triggers(card, definition_id, player);
        self.enqueue_land_entry_triggers(player)?;
        Ok(())
    }

    fn defending_player_is_protected_from_attacks(
        &self,
        attacking_player: PlayerId,
        defending_player: PlayerId,
    ) -> Result<bool, RulesError> {
        if attacking_player == defending_player {
            return Ok(false);
        }
        for source in self.all_battlefield_cards() {
            if self.controller_of(source)? != defending_player {
                continue;
            }
            let Some(definition_id) = self.effective_definition_id(source)? else {
                continue;
            };
            if self
                .static_attack_restrictions
                .get(definition_id)
                .is_some_and(|restrictions| {
                    restrictions.contains(&StaticAttackRestriction::OpponentsCannotAttackController)
                })
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Performs the turn-based action of declaring attackers in the current combat.
    #[allow(clippy::too_many_lines)] // Declaration captures every keyword's auditable provenance atomically.
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
        let defending_player = self.next_player(player);
        if !attackers.is_empty()
            && self.defending_player_is_protected_from_attacks(player, defending_player)?
        {
            return Err(RulesError::IllegalAction(
                "cannot attack a player protected by a static attack restriction",
            ));
        }
        let mut seen = BTreeSet::new();
        let mut hasty_attackers = BTreeSet::new();
        let mut flying_attackers = BTreeSet::new();
        let mut fear_attackers = BTreeSet::new();
        let mut black_evasion_attackers = BTreeSet::new();
        let mut unblockable_attackers = BTreeSet::new();
        let mut vigilant_attackers = BTreeSet::new();
        let mut trampling_attackers = BTreeSet::new();
        let mut must_be_blocked_attackers = BTreeSet::new();
        let mut landwalk_attackers = BTreeMap::<ObjectId, BTreeSet<BasicLandType>>::new();
        for attacker in attackers {
            if !seen.insert(*attacker) {
                return Err(RulesError::IllegalAction("an attacker was declared twice"));
            }
            self.require_zone(*attacker, Zone::Battlefield)?;
            let object = self.object(*attacker)?;
            let characteristics = self.characteristics(*attacker)?;
            let has_haste = characteristics.keywords.contains(&Keyword::Haste);
            if self.controller_of(*attacker)? != player
                || object.tapped
                || (object.controller_changed_turn >= self.turn && !has_haste)
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
            if characteristics.keywords.contains(&Keyword::Unblockable) {
                unblockable_attackers.insert(*attacker);
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
                landwalk_attackers
                    .entry(*attacker)
                    .or_default()
                    .insert(BasicLandType::Mountain);
            }
            for keyword in &characteristics.keywords {
                if let Keyword::Landwalk(land_type) = keyword {
                    landwalk_attackers
                        .entry(*attacker)
                        .or_default()
                        .insert(*land_type);
                }
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
        let combat = self
            .combat
            .as_mut()
            .ok_or(RulesError::IllegalAction("combat was not initialized"))?;
        combat.attackers = attackers.to_vec();
        combat.hasty_attackers = hasty_attackers;
        combat.flying_attackers = flying_attackers;
        combat.fear_attackers = fear_attackers;
        combat.black_evasion_attackers = black_evasion_attackers;
        combat.unblockable_attackers = unblockable_attackers;
        combat.vigilant_attackers = vigilant_attackers;
        combat.trampling_attackers = trampling_attackers;
        combat.must_be_blocked_attackers = must_be_blocked_attackers;
        combat.landwalk_attackers = landwalk_attackers;
        combat.defending_player = Some(defending_player);
        combat.attackers_declared = true;
        self.record_event(GameEvent::AttackersDeclared {
            player,
            attackers: attackers.to_vec(),
        });
        for attacker in attackers {
            self.enqueue_attack_triggers(*attacker)?;
        }
        self.flush_pending_trigger_events();
        self.consecutive_passes = 0;
        // CR 508.2: the active player receives priority after attackers are
        // declared. Declaration itself is a turn-based action, not a normal
        // priority action, so a stale pre-declaration holder cannot block it.
        self.priority = self.active_player;
        self.validate_invariants()
    }

    /// Performs the turn-based action of assigning each blocker to one
    /// attacker. More than one blocker may be assigned to the same attacker;
    /// declaration order is retained as the bounded damage-assignment order.
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
        let mut blocked_attackers = BTreeSet::new();
        let mut blockers = BTreeSet::new();
        let mut evasion_qualified_blockers = BTreeSet::new();
        let mut fear_qualified_blockers = BTreeSet::new();
        let mut black_evasion_qualified_blockers = BTreeSet::new();
        for assignment in assignments {
            if !combat.attackers.contains(&assignment.attacker)
                || !blockers.insert(assignment.blocker)
            {
                return Err(RulesError::IllegalAction("invalid blocker assignment"));
            }
            blocked_attackers.insert(assignment.attacker);
            if combat.unblockable_attackers.contains(&assignment.attacker) {
                return Err(RulesError::IllegalAction(
                    "unblockable attacker cannot be blocked",
                ));
            }
            self.require_zone(assignment.blocker, Zone::Battlefield)?;
            let object = self.object(assignment.blocker)?;
            let characteristics = self.characteristics(assignment.blocker)?;
            if self.controller_of(assignment.blocker)? != player
                || object.tapped
                || !characteristics.card_types.contains(&CardType::Creature)
                || self.target_cannot_block_attacker(assignment.blocker, assignment.attacker)
                || self.protection_prevents_block(assignment.blocker, assignment.attacker)
                || characteristics
                    .keywords
                    .contains(&Keyword::CannotAttackOrBlock)
                || characteristics.keywords.contains(&Keyword::CannotBlock)
                || (characteristics
                    .keywords
                    .contains(&Keyword::CannotBlockUnlessControlsMountain)
                    && !self.player_controls_basic_land_type(player, BasicLandType::Mountain))
                || (self.controller_saprolings_cannot_block(player)
                    && characteristics
                        .creature_subtypes
                        .contains(&CreatureSubtype::Saproling))
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
            if combat
                .landwalk_attackers
                .get(&assignment.attacker)
                .is_some_and(|land_types| {
                    land_types
                        .iter()
                        .any(|land_type| self.player_controls_basic_land_type(player, *land_type))
                })
            {
                return Err(RulesError::IllegalAction(
                    "landwalk attacker cannot be blocked while defender controls its land type",
                ));
            }
        }
        for attacker in &combat.must_be_blocked_attackers {
            if blocked_attackers.contains(attacker) {
                continue;
            }
            if combat.unblockable_attackers.contains(attacker) {
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
                    || (self.controller_saprolings_cannot_block(player)
                        && characteristics
                            .creature_subtypes
                            .contains(&CreatureSubtype::Saproling))
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
                if self.protection_prevents_block(*candidate, *attacker) {
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
                .entry(assignment.attacker)
                .or_default()
                .push(assignment.blocker);
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
        self.atomic_transition(|game| game.cast_spell_impl(player, &request, None, None))
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
        self.atomic_transition(|game| {
            game.cast_spell_impl(player, &request, Some(&selection), None)
        })
    }

    /// Casts a spell with an explicit chosen X value. The caller supplies the
    /// exact generic-color allocation for the printed generic symbols plus X;
    /// the atomic cast retains the complete receipt for resolution-time rules.
    #[allow(clippy::needless_pass_by_value)] // Public selection ownership crosses the atomic boundary.
    pub fn cast_spell_with_x(
        &mut self,
        player: PlayerId,
        request: CastRequest,
        x_value: u8,
        selection: ManaPaymentSelection,
    ) -> Result<(), RulesError> {
        self.atomic_transition(|game| {
            game.cast_spell_impl(player, &request, Some(&selection), Some(x_value))
        })
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
        chosen_x: Option<u8>,
    ) -> Result<(), RulesError> {
        self.require_priority(player)?;
        let from_graveyard = self
            .graveyard_cast_permissions
            .get(&request.card)
            .is_some_and(|permission| {
                permission.player == player
                    && permission.expires_turn >= self.turn
                    && self.zone_of(request.card) == Some(Zone::Graveyard)
            });
        if !from_graveyard {
            self.require_zone(request.card, Zone::Hand)?;
        }
        let definition = self.card_definition(request.card)?.clone();
        if definition.is_land() {
            return Err(RulesError::IllegalAction("lands are played, not cast"));
        }
        if !definition.is_permanent() && definition.effects.is_empty() {
            return Err(RulesError::IllegalAction(
                "this spell's front-face effect is unsupported; report an engine weakness",
            ));
        }
        let requires_chosen_x = definition.effects.iter().any(Effect::requires_chosen_x);
        if requires_chosen_x != chosen_x.is_some() {
            return Err(RulesError::IllegalAction(if requires_chosen_x {
                "this spell requires an explicit chosen X value"
            } else {
                "chosen X is not legal for this spell"
            }));
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
            return Err(RulesError::IllegalAction("only your own card may be cast"));
        }
        let (spell_targets, additional_cost_selections) =
            self.split_cast_targets(&definition, &request.targets)?;
        self.validate_targets(player, &definition, &spell_targets)?;
        if let Some(x_value) = chosen_x {
            self.validate_chosen_x_targets(&definition, &spell_targets, x_value)?;
        }
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
        if from_graveyard
            && (!request.payment_mana_abilities.is_empty()
                || !request.convoke.is_empty()
                || mana_payment_selection.is_some())
        {
            return Err(RulesError::IllegalAction(
                "a graveyard permission cast cannot use mana or convoke",
            ));
        }
        let mut payment_definition = definition.clone();
        if let Some(x_value) = chosen_x {
            payment_definition.mana_cost.generic = payment_definition
                .mana_cost
                .generic
                .checked_add(x_value)
                .ok_or(RulesError::IllegalAction(
                    "chosen X overflows generic mana cost",
                ))?;
        }
        let applied_generic_cost_reduction = self
            .generic_cost_reduction(player, &payment_definition)
            .min(payment_definition.mana_cost.generic);
        payment_definition.mana_cost.generic -= applied_generic_cost_reduction;
        let (paid_cost, mana_spent) = if from_graveyard {
            (self.players[player.0].mana_pool.clone(), None)
        } else {
            self.pay_cost_with_convoke(
                player,
                request.card,
                &payment_definition,
                &request.convoke,
                mana_payment_selection,
            )?
        };
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
        let source_incarnation = self.move_to_stack(request.card)?;
        let target_incarnations = self.target_incarnations(&spell_targets);
        self.stack.push(StackObject {
            card: request.card,
            source_incarnation,
            source_colors: definition.colors.clone(),
            controller: player,
            ability_id: None,
            targets: spell_targets,
            target_incarnations,
            effects: definition.effects,
            chosen_x,
            mana_spent: mana_spent.clone(),
            convoke_symbols: request.convoke.len(),
            generic_cost_reduction: applied_generic_cost_reduction,
        });
        if let Some(colors) = mana_spent {
            self.record_event(GameEvent::SpellManaPaid {
                player,
                card: request.card,
                colors,
            });
        }
        if from_graveyard {
            self.graveyard_cast_permissions.remove(&request.card);
            self.exile_on_resolution.insert(request.card);
            self.record_event(GameEvent::SpellCastFromGraveyard {
                player,
                card: request.card,
            });
        }
        self.record_event(GameEvent::SpellCast {
            player,
            card: request.card,
        });
        self.record_event(GameEvent::ObjectIncarnationAdvanced {
            object: request.card,
            incarnation: source_incarnation,
        });
        if !definition.card_types.contains(&CardType::Creature) {
            self.enqueue_cast_noncreature_triggers(player, request.card)?;
        }
        self.consecutive_passes = 0;
        // CR 601.2i / 117.3c: after completing a cast, the acting player
        // receives priority again. Opponents get their response window only
        // after that player passes.
        self.priority = player;
        self.check_state_based_actions()?;
        self.flush_pending_dies_triggers();
        Ok(())
    }

    /// Verifies target constraints that depend on the selected X before any
    /// cost, zone, or receipt mutation. Tokens have mana value zero in this
    /// public slice, while a catalog card reads its printed mana cost.
    fn validate_chosen_x_targets(
        &self,
        definition: &CardDefinition,
        targets: &[Target],
        x_value: u8,
    ) -> Result<(), RulesError> {
        let mut targets = targets.iter().copied();
        for effect in &definition.effects {
            if matches!(
                effect,
                Effect::LookAtTopCardsOfTargetOpponentExileOne { .. }
            ) {
                return Err(RulesError::IllegalAction(
                    "private opponent-library choice is valid only on an activated ability",
                ));
            }
            let Some(requirement) = effect.target_requirement() else {
                continue;
            };
            let target = targets.next().ok_or(RulesError::IllegalAction(
                "chosen-X spell target is missing",
            ))?;
            if !effect.requires_chosen_x() {
                continue;
            }
            if requirement != TargetRequirement::Creature {
                return Err(RulesError::IllegalAction(
                    "chosen-X instruction has an unsupported target requirement",
                ));
            }
            let Target::Permanent(card) = target else {
                return Err(RulesError::IllegalTarget(target));
            };
            let mana_value = if self.object(card)?.token.is_some() {
                0
            } else {
                self.card_definition(card)?.mana_cost.mana_value()
            };
            if mana_value > x_value {
                return Err(RulesError::IllegalTarget(target));
            }
        }
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
        if self.pending_private_library_choice.is_some() {
            return Err(RulesError::IllegalAction(
                "the private library choice must resolve before priority can pass",
            ));
        }
        if self.pending_private_opponent_library_exile_choice.is_some() {
            return Err(RulesError::IllegalAction(
                "the private opponent-library choice must resolve before priority can pass",
            ));
        }
        if self.pending_decision.is_some() {
            return Err(RulesError::IllegalAction(
                "the pending decision must resolve before priority can pass",
            ));
        }
        if !self.pending_trigger_target_choices.is_empty() {
            return Err(RulesError::IllegalAction(
                "trigger targets must be chosen before priority can pass",
            ));
        }
        if self.pending_optional_trigger_choice.is_some() {
            return Err(RulesError::IllegalAction(
                "optional trigger payment must resolve before priority can pass",
            ));
        }
        if self.pending_damage_replacement_choice.is_some() {
            return Err(RulesError::IllegalAction(
                "damage replacement choice must resolve before priority can pass",
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
        let Some(card) = self.players[player.0].library.last().copied() else {
            self.lose_player(player, "attempted to draw from an empty library");
            self.normalize_priority_after_elimination()?;
            self.record_game_end_if_needed();
            if resolves_pending_draw {
                self.pending_draw_replacement = None;
            }
            return Ok(());
        };
        self.move_to_zone(card, Zone::Hand)?;
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
        let Some(card) = self.players[player.0].library.last().copied() else {
            self.lose_player(player, "attempted to draw from an empty library");
            self.normalize_priority_after_elimination()?;
            // The enclosing stack resolver still owes its `AbilityResolved`
            // or terminal source-lifecycle receipt. It records `GameEnded`
            // only after that receipt so the terminal event stays last.
            return Ok(());
        };
        self.move_to_zone(card, Zone::Hand)?;
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

    /// Completes a private library choice that was opened while an untargeted
    /// spell began resolving. Unlike a priority action, no other move may
    /// interleave with this selection.
    #[allow(clippy::needless_pass_by_value)] // Mirrors the owned policy-action payload at the public boundary.
    pub fn choose_private_library_cards(
        &mut self,
        player: PlayerId,
        spell: ObjectId,
        selected: Vec<ObjectId>,
    ) -> Result<(), RulesError> {
        self.atomic_transition(|game| {
            game.resolve_pending_private_library_choice(player, spell, &selected)
        })
    }

    #[allow(clippy::too_many_lines)] // The full suspension/resumption transaction is one audit unit.
    fn resolve_pending_private_library_choice(
        &mut self,
        player: PlayerId,
        spell: ObjectId,
        selected: &[ObjectId],
    ) -> Result<(), RulesError> {
        self.require_game_in_progress()?;
        let choice =
            self.pending_private_library_choice
                .clone()
                .ok_or(RulesError::IllegalAction(
                    "there is no pending private library choice",
                ))?;
        if choice.controller != player || choice.spell != spell {
            return Err(RulesError::IllegalAction(
                "only the resolving controller may submit this private library choice",
            ));
        }
        let stack_object = self.stack.last().ok_or(RulesError::IllegalAction(
            "private library choice has no live stack spell",
        ))?;
        if stack_object.card != spell
            || stack_object.controller != player
            || stack_object.ability_id.is_some()
            || !matches!(
                stack_object.effects.as_slice(),
                [Effect::LookAtTopCardsChooseForLifeOrGraveyard { life_per_card, .. }]
                    if *life_per_card == choice.life_per_card
            )
        {
            return Err(RulesError::IllegalAction(
                "private library choice no longer matches the live stack spell",
            ));
        }
        if selected
            .iter()
            .enumerate()
            .any(|(index, card)| selected[..index].contains(card))
            || selected.iter().any(|card| !choice.cards.contains(card))
        {
            return Err(RulesError::IllegalAction(
                "private library choice contains an illegal or duplicate card",
            ));
        }
        let selected_count = i16::try_from(selected.len()).map_err(|_| {
            RulesError::IllegalAction("private library choice count exceeds life-payment range")
        })?;
        let life_payment =
            selected_count
                .checked_mul(choice.life_per_card)
                .ok_or(RulesError::IllegalAction(
                    "private library choice life payment overflows",
                ))?;
        if life_payment < 0 || self.players[player.0].life < i64::from(life_payment) {
            return Err(RulesError::IllegalAction(
                "private library choice exceeds available life payment",
            ));
        }
        if choice.cards.iter().any(|card| {
            self.zone_of(*card) != Some(Zone::Library)
                || self
                    .object(*card)
                    .map_or(true, |object| object.owner != player)
        }) {
            return Err(RulesError::IllegalAction(
                "private library choice cards are no longer in the controller library",
            ));
        }

        self.stack.pop().ok_or(RulesError::IllegalAction(
            "private library choice stack spell disappeared before resolution",
        ))?;
        self.pending_private_library_choice = None;
        if life_payment > 0 {
            self.players[player.0].life -= i64::from(life_payment);
            self.record_event(GameEvent::LifePaid {
                source: spell,
                player,
                amount: life_payment,
            });
        }
        for card in choice.cards {
            self.move_to_zone(
                card,
                if selected.contains(&card) {
                    Zone::Hand
                } else {
                    Zone::Graveyard
                },
            )?;
        }
        self.record_event(GameEvent::SpellResolved { card: spell });
        self.move_to_spell_terminal_zone(spell)?;
        self.check_state_based_actions()?;
        self.flush_pending_dies_triggers();
        self.flush_pending_land_entry_triggers()?;
        self.flush_pending_damage_triggers();
        self.flush_pending_life_gain_triggers();
        self.flush_pending_dies_triggers();
        self.priority = self.priority_after_resolution();
        Ok(())
    }

    /// Completes the private opponent-library selection opened by a resolving
    /// activated ability. The choice is atomic: no priority action can happen
    /// after the candidates are seen and before the selected card moves to
    /// exile.
    pub fn choose_private_opponent_library_card_to_exile(
        &mut self,
        player: PlayerId,
        source: ObjectId,
        ability: &'static str,
        selected: Option<ObjectId>,
    ) -> Result<(), RulesError> {
        self.atomic_transition(|game| {
            game.resolve_pending_private_opponent_library_exile_choice(
                player, source, ability, selected,
            )
        })
    }

    #[allow(clippy::too_many_lines)] // The suspension/resumption transaction is one auditable boundary.
    fn resolve_pending_private_opponent_library_exile_choice(
        &mut self,
        player: PlayerId,
        source: ObjectId,
        ability: &'static str,
        selected: Option<ObjectId>,
    ) -> Result<(), RulesError> {
        self.require_game_in_progress()?;
        let choice = self
            .pending_private_opponent_library_exile_choice
            .clone()
            .ok_or(RulesError::IllegalAction(
                "there is no pending private opponent-library choice",
            ))?;
        if choice.controller != player || choice.source != source || choice.ability != ability {
            return Err(RulesError::IllegalAction(
                "only the resolving controller may submit this private opponent-library choice",
            ));
        }
        let stack_object = self.stack.last().ok_or(RulesError::IllegalAction(
            "private opponent-library choice has no live stack ability",
        ))?;
        let count = match stack_object.effects.as_slice() {
            [Effect::LookAtTopCardsOfTargetOpponentExileOne { count }] => *count,
            _ => {
                return Err(RulesError::IllegalAction(
                    "private opponent-library choice no longer matches its ability effect",
                ));
            }
        };
        if stack_object.card != source
            || stack_object.controller != player
            || stack_object.ability_id != Some(ability)
            || stack_object.targets.as_slice() != [Target::Player(choice.opponent)]
        {
            return Err(RulesError::IllegalAction(
                "private opponent-library choice no longer matches the live stack ability",
            ));
        }
        let source_incarnation = stack_object.source_incarnation;
        let expected_cards = self.players[choice.opponent.0]
            .library
            .iter()
            .rev()
            .take(usize::from(count))
            .copied()
            .collect::<Vec<_>>();
        if choice.cards != expected_cards
            || choice.cards.iter().any(|card| {
                self.zone_of(*card) != Some(Zone::Library)
                    || self
                        .object(*card)
                        .map_or(true, |object| object.owner != choice.opponent)
            })
        {
            return Err(RulesError::IllegalAction(
                "private opponent-library candidates changed before selection",
            ));
        }
        if choice.cards.is_empty() {
            if selected.is_some() {
                return Err(RulesError::IllegalAction(
                    "an empty private opponent-library choice cannot exile a card",
                ));
            }
        } else {
            let selected_card = selected.ok_or(RulesError::IllegalAction(
                "a nonempty private opponent-library choice must exile one card",
            ))?;
            if !choice.cards.contains(&selected_card) {
                return Err(RulesError::IllegalAction(
                    "private opponent-library choice selected a card outside its snapshot",
                ));
            }
        }

        self.stack.pop().ok_or(RulesError::IllegalAction(
            "private opponent-library choice stack ability disappeared before resolution",
        ))?;
        self.pending_private_opponent_library_exile_choice = None;
        if let Some(card) = selected {
            self.move_to_zone(card, Zone::Exile)?;
        }
        self.record_event(GameEvent::AbilityResolved {
            source,
            source_incarnation,
            ability,
        });
        self.check_state_based_actions()?;
        self.flush_pending_land_entry_triggers()?;
        self.flush_pending_damage_triggers();
        self.flush_pending_life_gain_triggers();
        self.flush_pending_dies_triggers();
        self.priority = self.priority_after_resolution();
        Ok(())
    }

    /// Completes an explicit controller-private library search selection. The
    /// suspended spell or ability remains the stack top until this atomic
    /// resolution finishes, so no player can act using unrevealed candidates.
    pub fn choose_library_search_card(
        &mut self,
        player: PlayerId,
        source: ObjectId,
        selected: Option<ObjectId>,
    ) -> Result<(), RulesError> {
        self.atomic_transition(|game| {
            let decision = game
                .pending_decision
                .as_ref()
                .ok_or(RulesError::IllegalAction(
                    "there is no pending policy-submitted library search",
                ))?;
            if !matches!(
                decision.continuation,
                DecisionContinuation::LibrarySearch { source: pending_source, .. }
                    if pending_source == source
            ) {
                return Err(RulesError::IllegalAction(
                    "library search compatibility action does not match the pending decision",
                ));
            }
            game.resolve_pending_decision(
                player,
                decision.id,
                DecisionSelection::Objects(selected.into_iter().collect()),
            )
        })
    }

    /// Submits a typed, id-bearing answer to the current no-priority
    /// decision. Unlike compatibility shims, callers must supply the exact
    /// monotonic identity observed in their `GameView`.
    pub fn submit_decision(
        &mut self,
        player: PlayerId,
        decision: DecisionId,
        selection: DecisionSelection,
    ) -> Result<(), RulesError> {
        self.atomic_transition(|game| game.resolve_pending_decision(player, decision, selection))
    }

    fn resolve_pending_decision(
        &mut self,
        player: PlayerId,
        decision_id: DecisionId,
        selection: DecisionSelection,
    ) -> Result<(), RulesError> {
        self.require_game_in_progress()?;
        let decision = self
            .pending_decision
            .clone()
            .ok_or(RulesError::IllegalAction(
                "there is no pending typed decision",
            ))?;
        if decision.id != decision_id {
            return Err(RulesError::IllegalAction("stale or unknown decision id"));
        }
        if decision.player != player {
            return Err(RulesError::IllegalAction(
                "only the decision player may submit this decision",
            ));
        }
        let selected = Self::validate_decision_selection(&decision, selection)?;
        match decision.continuation.clone() {
            DecisionContinuation::LibrarySearch {
                source,
                requirement,
                destination,
                may_fail_to_find,
            } => self.resolve_library_search_decision(
                &decision,
                source,
                &requirement,
                destination,
                may_fail_to_find,
                selected.into_iter().next(),
            ),
            DecisionContinuation::TriggeredEffectObject {
                source,
                controller,
                ability,
                kind,
            } => self.resolve_triggered_effect_object_decision(
                &decision,
                source,
                controller,
                ability,
                kind,
                selected.into_iter().next(),
            ),
        }
    }

    fn validate_decision_selection(
        decision: &PendingDecision,
        selection: DecisionSelection,
    ) -> Result<Vec<ObjectId>, RulesError> {
        let DecisionSelection::Objects(selected) = selection;
        let count = u8::try_from(selected.len())
            .map_err(|_| RulesError::IllegalAction("decision selection exceeds engine range"))?;
        if count < decision.min_selections || count > decision.max_selections {
            return Err(RulesError::IllegalAction(
                "decision selection violates its minimum or maximum cardinality",
            ));
        }
        let mut seen = HashSet::new();
        for card in &selected {
            if !seen.insert(*card) || !decision.options.contains(&DecisionOption::Object(*card)) {
                return Err(RulesError::IllegalAction(
                    "decision selection contains a duplicate or illegal option",
                ));
            }
        }
        Ok(selected)
    }

    #[allow(clippy::too_many_lines)] // The suspended selection and terminal stack lifecycle are one transaction.
    fn resolve_library_search_decision(
        &mut self,
        decision: &PendingDecision,
        source: ObjectId,
        requirement: &LibrarySearchRequirement,
        destination: LibrarySearchDestination,
        may_fail_to_find: bool,
        selected: Option<ObjectId>,
    ) -> Result<(), RulesError> {
        let player = decision.player;
        let (ability, chosen_x, source_incarnation) = {
            let top = self.stack.last().ok_or(RulesError::IllegalAction(
                "library search choice has no live stack item",
            ))?;
            let matches_pending_search = matches!(
                top.effects.as_slice(),
                [Effect::SearchControllerLibrary {
                    requirement: stack_requirement,
                    destination: stack_destination,
                    selection:
                        LibrarySearchSelection::PolicySubmitted {
                            may_fail_to_find: stack_may_fail,
                        },
                }] if stack_requirement == requirement
                    && *stack_destination == destination
                    && *stack_may_fail == may_fail_to_find
            );
            if top.card != source || top.controller != player || !matches_pending_search {
                return Err(RulesError::IllegalAction(
                    "library search choice no longer matches the live stack item",
                ));
            }
            (top.ability_id, top.chosen_x, top.source_incarnation)
        };
        let expected_cards = self.library_search_candidates(player, requirement, chosen_x)?;
        if decision.options
            != expected_cards
                .iter()
                .copied()
                .map(DecisionOption::Object)
                .collect::<Vec<_>>()
        {
            return Err(RulesError::IllegalAction(
                "library search candidates changed before selection",
            ));
        }
        match selected {
            Some(card) if expected_cards.contains(&card) => {}
            Some(_) => {
                return Err(RulesError::IllegalAction(
                    "library search selected a card outside its legal candidates",
                ));
            }
            None if may_fail_to_find || expected_cards.is_empty() => {}
            None => {
                return Err(RulesError::IllegalAction(
                    "this library search must select a matching card when one exists",
                ));
            }
        }

        self.stack.pop().ok_or(RulesError::IllegalAction(
            "library search stack item disappeared before resolution",
        ))?;
        self.complete_pending_decision(decision)?;

        let mut entered_permanent = None;
        if let Some(card) = selected {
            match destination {
                LibrarySearchDestination::Battlefield
                | LibrarySearchDestination::BattlefieldTapped => {
                    self.move_to_zone(card, Zone::Battlefield)?;
                    if destination == LibrarySearchDestination::BattlefieldTapped {
                        self.objects
                            .get_mut(&card)
                            .ok_or(RulesError::UnknownCard(card))?
                            .tapped = true;
                    }
                    let object = self.object(card)?;
                    let definition =
                        self.effective_definition_id(card)?
                            .ok_or(RulesError::IllegalAction(
                                "a token cannot be selected from a library",
                            ))?;
                    entered_permanent = Some((card, definition, object.controller));
                }
                LibrarySearchDestination::Hand => self.move_to_zone(card, Zone::Hand)?,
            }
        }
        self.record_event(GameEvent::LibrarySearchResolved {
            player,
            source,
            found: selected,
            destination,
        });
        self.shuffle_library(player);
        let cards = u16::try_from(self.players[player.0].library.len()).unwrap_or(u16::MAX);
        self.record_event(GameEvent::LibraryShuffled { player, cards });

        if let Some(ability) = ability {
            self.record_event(GameEvent::AbilityResolved {
                source,
                source_incarnation,
                ability,
            });
        } else {
            self.record_event(GameEvent::SpellResolved { card: source });
            self.move_to_spell_terminal_zone(source)?;
        }
        self.check_state_based_actions()?;
        self.flush_pending_dies_triggers();
        if let Some((card, definition, controller)) = entered_permanent
            && self.zone_of(card) == Some(Zone::Battlefield)
        {
            self.enqueue_enter_triggers(card, definition, controller);
            if self.card_definition(card)?.is_land() {
                self.queue_land_entry_trigger_batch(controller)?;
            }
        }
        self.flush_pending_land_entry_triggers()?;
        self.flush_pending_damage_triggers();
        self.flush_pending_life_gain_triggers();
        self.flush_pending_dies_triggers();
        self.priority = self.priority_after_resolution();
        Ok(())
    }

    fn resolve_triggered_effect_object_decision(
        &mut self,
        decision: &PendingDecision,
        source: ObjectId,
        controller: PlayerId,
        ability: &'static str,
        mut kind: TriggeredEffectObjectDecisionKind,
        selected: Option<ObjectId>,
    ) -> Result<(), RulesError> {
        let player = decision.player;
        match &mut kind {
            TriggeredEffectObjectDecisionKind::DiscardEachPlayer {
                remaining_players,
                selected: selections,
            } => {
                if remaining_players.first() != Some(&player) {
                    return Err(RulesError::IllegalAction(
                        "discard choice player is not next in the resolving trigger",
                    ));
                }
                if let Some(card) = selected {
                    selections.push((player, card));
                }
                remaining_players.remove(0);
                self.complete_pending_decision(decision)?;
                if let Some(next_player) = remaining_players.first().copied() {
                    let options = self.players[next_player.0]
                        .hand
                        .iter()
                        .copied()
                        .map(DecisionOption::Object)
                        .collect::<Vec<_>>();
                    let (min_selections, max_selections) =
                        if options.is_empty() { (0, 0) } else { (1, 1) };
                    self.open_pending_decision(
                        next_player,
                        DecisionVisibility::Private,
                        DecisionKind::TriggeredEffectObject,
                        min_selections,
                        max_selections,
                        options,
                        DecisionContinuation::TriggeredEffectObject {
                            source,
                            controller,
                            ability,
                            kind,
                        },
                    )?;
                    return Ok(());
                }
                let selections = selections.clone();
                self.finish_trigger_effect_object_choice(source, ability, |game| {
                    for (discarding_player, card) in selections {
                        if game.zone_of(card) != Some(Zone::Hand)
                            || game.object(card).is_err_and(|_| true)
                            || game.object(card)?.owner != discarding_player
                        {
                            return Err(RulesError::IllegalAction(
                                "chosen discard card left its chooser hand before resolution",
                            ));
                        }
                        game.record_event(GameEvent::CardDiscarded {
                            player: discarding_player,
                            card,
                        });
                        game.move_to_zone(card, Zone::Graveyard)?;
                    }
                    Ok(())
                })?;
            }
            TriggeredEffectObjectDecisionKind::SacrificeControllerCreature => {
                self.complete_pending_decision(decision)?;
                self.finish_trigger_effect_object_choice(source, ability, |game| {
                    if let Some(permanent) = selected {
                        if game.zone_of(permanent) != Some(Zone::Battlefield)
                            || game.object(permanent)?.controller != player
                            || !game
                                .characteristics(permanent)?
                                .card_types
                                .contains(&CardType::Creature)
                        {
                            return Err(RulesError::IllegalAction(
                                "chosen sacrifice permanent is no longer a controlled creature",
                            ));
                        }
                        game.record_event(GameEvent::SacrificedByEffect {
                            source,
                            player,
                            permanent,
                        });
                        game.move_to_graveyard_or_remove_token(permanent)?;
                    }
                    Ok(())
                })?;
            }
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)] // Typed scalar boundary keeps each continuation call explicit and auditable.
    fn open_pending_decision(
        &mut self,
        player: PlayerId,
        visibility: DecisionVisibility,
        kind: DecisionKind,
        min_selections: u8,
        max_selections: u8,
        options: Vec<DecisionOption>,
        continuation: DecisionContinuation,
    ) -> Result<DecisionId, RulesError> {
        if self.pending_decision.is_some() {
            return Err(RulesError::IllegalAction(
                "a second typed decision attempted to open before the first completed",
            ));
        }
        if min_selections > max_selections || usize::from(max_selections) > options.len() {
            return Err(RulesError::IllegalAction(
                "pending decision has invalid selection cardinality",
            ));
        }
        let id = DecisionId(self.next_decision_id);
        self.next_decision_id = self
            .next_decision_id
            .checked_add(1)
            .ok_or(RulesError::IllegalAction("decision id space exhausted"))?;
        let decision = PendingDecision {
            id,
            player,
            visibility,
            kind,
            min_selections,
            max_selections,
            options,
            continuation,
        };
        self.record_event(GameEvent::DecisionOpened {
            decision: id,
            player,
            kind,
            visibility,
            min_selections,
            max_selections,
        });
        self.pending_decision = Some(decision);
        self.priority = player;
        self.consecutive_passes = 0;
        Ok(id)
    }

    fn complete_pending_decision(&mut self, decision: &PendingDecision) -> Result<(), RulesError> {
        if self.pending_decision.as_ref() != Some(decision) {
            return Err(RulesError::IllegalAction(
                "pending decision changed before completion",
            ));
        }
        self.pending_decision = None;
        self.record_event(GameEvent::DecisionCompleted {
            decision: decision.id,
            player: decision.player,
            kind: decision.kind,
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
                .last()
                .copied()
                .ok_or(RulesError::IllegalAction("library changed during dredge"))?;
            self.move_to_zone(milled, Zone::Graveyard)?;
        }
        self.move_to_zone(card, Zone::Hand)?;
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
        if self.library_search_prevented_until == Some(self.turn) {
            return Err(RulesError::IllegalAction(
                "library searches are prevented this turn",
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

    /// Resolves the compatibility-selector branch of one typed library search.
    /// A policy-submitted search is suspended before this method runs unless a
    /// current-turn prevention effect has made that search fail automatically.
    fn resolve_controller_library_search(
        &mut self,
        source: ObjectId,
        player: PlayerId,
        requirement: &LibrarySearchRequirement,
        destination: LibrarySearchDestination,
        selection: LibrarySearchSelection,
        chosen_x: Option<u8>,
    ) -> Result<(), RulesError> {
        let prevented = self.library_search_prevented_until == Some(self.turn);
        let candidates = (!prevented)
            .then(|| self.library_search_candidates(player, requirement, chosen_x))
            .transpose()?
            .unwrap_or_default();
        let found = match selection {
            LibrarySearchSelection::DeterministicFirstMatch => candidates.first().copied(),
            LibrarySearchSelection::PolicySubmitted { .. } if prevented => None,
            LibrarySearchSelection::PolicySubmitted { .. } => {
                return Err(RulesError::IllegalAction(
                    "policy-submitted library search reached resolution without a selection",
                ));
            }
        };
        if let Some(card) = found {
            match destination {
                LibrarySearchDestination::Battlefield => {
                    self.move_to_zone(card, Zone::Battlefield)?;
                    if self.card_definition(card)?.is_land() {
                        self.queue_land_entry_trigger_batch(player)?;
                    }
                }
                LibrarySearchDestination::BattlefieldTapped => {
                    self.move_to_zone(card, Zone::Battlefield)?;
                    self.objects
                        .get_mut(&card)
                        .ok_or(RulesError::UnknownCard(card))?
                        .tapped = true;
                    if self.card_definition(card)?.is_land() {
                        self.queue_land_entry_trigger_batch(player)?;
                    }
                }
                LibrarySearchDestination::Hand => self.move_to_zone(card, Zone::Hand)?,
            }
        }
        self.record_event(GameEvent::LibrarySearchResolved {
            player,
            source,
            found,
            destination,
        });
        self.shuffle_library(player);
        let cards = u16::try_from(self.players[player.0].library.len()).unwrap_or(u16::MAX);
        self.record_event(GameEvent::LibraryShuffled { player, cards });
        Ok(())
    }

    fn library_search_candidates(
        &self,
        player: PlayerId,
        requirement: &LibrarySearchRequirement,
        chosen_x: Option<u8>,
    ) -> Result<Vec<ObjectId>, RulesError> {
        let mut candidates = Vec::new();
        for card in &self.players[player.0].library {
            if self.zone_of(*card) == Some(Zone::Library)
                && self.object(*card)?.owner == player
                && self.library_search_matches(*card, requirement, chosen_x)?
            {
                candidates.push(*card);
            }
        }
        Ok(candidates)
    }

    fn library_search_matches(
        &self,
        card: ObjectId,
        requirement: &LibrarySearchRequirement,
        chosen_x: Option<u8>,
    ) -> Result<bool, RulesError> {
        let definition = self.card_definition(card)?;
        match requirement {
            LibrarySearchRequirement::BasicLandTypes(types) => Ok(definition.is_land()
                && self
                    .basic_land_type(card)?
                    .is_some_and(|land_type| types.contains(&land_type))),
            LibrarySearchRequirement::CreatureWithManaValueAtMostChosenX => {
                let x_value = chosen_x.ok_or(RulesError::IllegalAction(
                    "chosen-X library search lacks its selected X value",
                ))?;
                Ok(definition.card_types.contains(&CardType::Creature)
                    && definition.mana_cost.mana_value() <= x_value)
            }
        }
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
            for aura in self.all_battlefield_cards() {
                if !self.is_aura_like(aura)? {
                    continue;
                }
                let object = self.object(aura)?;
                let attached_to = object.attached_to;
                let attached_to_incarnation = object.attached_to_incarnation;
                let requirement =
                    self.aura_attachment_requirement(aura)?
                        .ok_or(RulesError::IllegalAction(
                            "aura-like permanent lacks an enchant restriction",
                        ))?;
                let attached_to_live_permanent = attached_to.is_some_and(|target| {
                    self.zone_of(target) == Some(Zone::Battlefield)
                        && attached_to_incarnation.is_some_and(|incarnation| {
                            self.object_has_incarnation(target, incarnation)
                        })
                        && self.target_matches_for_source(
                            self.controller_of(aura)
                                .unwrap_or(self.objects[&aura].controller),
                            aura,
                            Target::Permanent(target),
                            requirement,
                        )
                });
                if !attached_to_live_permanent {
                    self.record_event(GameEvent::StateBasedAction {
                        card: aura,
                        reason: if requirement == TargetRequirement::Creature {
                            "Aura is not attached to a battlefield creature"
                        } else {
                            "Aura is not attached to a legal battlefield permanent"
                        },
                    });
                    self.move_to_graveyard_or_remove_token(aura)?;
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
                    if self.use_regeneration_shield(card)? {
                        changed = true;
                        continue;
                    }
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
        if !self.pending_trigger_events.is_empty() {
            return Err(RulesError::IllegalAction(
                "pending trigger event escaped its enclosing rules action",
            ));
        }
        if !self.pending_trigger_placements.is_empty()
            && self.pending_trigger_target_choices.is_empty()
        {
            return Err(RulesError::IllegalAction(
                "trigger placement batch escaped without its target decision",
            ));
        }
        if !self.pending_land_entry_trigger_batches.is_empty() {
            return Err(RulesError::IllegalAction(
                "pending land-entry trigger escaped its resolving stack object",
            ));
        }
        Self::validate_mana_ability_event_order(&self.event_log)?;
        Self::validate_object_incarnation_event_order(&self.event_log)?;
        Self::validate_permanent_copy_event_order(&self.event_log)?;
        Self::validate_library_search_event_order(&self.event_log)?;
        Self::validate_aura_attachment_event_order(&self.event_log)?;
        Self::validate_delayed_action_event_order(&self.event_log)?;
        self.validate_linked_exile_state()?;
        Self::validate_spell_mana_payment_event_order(&self.event_log)?;
        self.validate_additional_spell_cost_event_order()?;
        self.validate_stack_terminal_event_order()?;
        self.validate_ability_event_order()?;
        self.validate_activated_ability_cost_event_order()?;
        let outstanding_private_opponent_library_choices =
            Self::validate_private_opponent_library_choice_event_order(&self.event_log)?;
        self.validate_ability_sacrifice_cost_event_order()?;
        self.validate_ability_discard_cost_event_order()?;
        Self::validate_effect_discard_event_order(&self.event_log)?;
        Self::validate_effect_sacrifice_event_order(&self.event_log)?;
        self.validate_ability_additional_tap_cost_event_order()?;
        Self::validate_counter_lifecycle_events(&self.event_log)?;
        self.validate_counter_removal_receipt_accounting()?;
        self.validate_replacement_effect_events()?;
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
        if let Some(choice) = &self.pending_private_library_choice {
            let top = self.stack.last().ok_or(RulesError::IllegalAction(
                "private-library choice escaped its stack spell",
            ))?;
            let count = match top.effects.as_slice() {
                [Effect::LookAtTopCardsChooseForLifeOrGraveyard { count, .. }] => *count,
                _ => 0,
            };
            let effect_matches = matches!(
                top.effects.as_slice(),
                [Effect::LookAtTopCardsChooseForLifeOrGraveyard {
                    life_per_card,
                    ..
                }] if *life_per_card == choice.life_per_card
            );
            let expected_cards = self.players[choice.controller.0]
                .library
                .iter()
                .rev()
                .take(usize::from(count))
                .copied()
                .collect::<Vec<_>>();
            if self.pending_draw_replacement.is_some()
                || self.pending_decision.is_some()
                || self.pending_private_opponent_library_exile_choice.is_some()
                || top.card != choice.spell
                || top.controller != choice.controller
                || top.ability_id.is_some()
                || !effect_matches
                || self.priority != choice.controller
                || self.consecutive_passes != 0
                || self.players[choice.controller.0].lost
                || choice.cards != expected_cards
                || choice.cards.iter().any(|card| {
                    self.zone_of(*card) != Some(Zone::Library)
                        || self
                            .object(*card)
                            .map_or(true, |object| object.owner != choice.controller)
                })
            {
                return Err(RulesError::IllegalAction(
                    "private-library choice escaped its resolution boundary",
                ));
            }
        }
        if let Some(choice) = &self.pending_private_opponent_library_exile_choice {
            let top = self.stack.last().ok_or(RulesError::IllegalAction(
                "private opponent-library choice escaped its stack ability",
            ))?;
            let count = match top.effects.as_slice() {
                [Effect::LookAtTopCardsOfTargetOpponentExileOne { count }] => *count,
                _ => 0,
            };
            let effect_matches = matches!(
                top.effects.as_slice(),
                [Effect::LookAtTopCardsOfTargetOpponentExileOne { count: effect_count }]
                    if *effect_count > 0
            );
            let expected_cards = self.players[choice.opponent.0]
                .library
                .iter()
                .rev()
                .take(usize::from(count))
                .copied()
                .collect::<Vec<_>>();
            if self.pending_draw_replacement.is_some()
                || self.pending_decision.is_some()
                || self.pending_private_library_choice.is_some()
                || top.card != choice.source
                || top.controller != choice.controller
                || top.ability_id != Some(choice.ability)
                || top.targets.as_slice() != [Target::Player(choice.opponent)]
                || !effect_matches
                || !self.target_matches_for_controller(
                    choice.controller,
                    Target::Player(choice.opponent),
                    TargetRequirement::Opponent,
                )
                || self.priority != choice.controller
                || self.consecutive_passes != 0
                || self.players[choice.controller.0].lost
                || choice.cards != expected_cards
                || choice.cards.iter().any(|card| {
                    self.zone_of(*card) != Some(Zone::Library)
                        || self
                            .object(*card)
                            .map_or(true, |object| object.owner != choice.opponent)
                })
            {
                return Err(RulesError::IllegalAction(
                    "private opponent-library choice escaped its resolution boundary",
                ));
            }
        }
        if (self.pending_private_opponent_library_exile_choice.is_some()
            && outstanding_private_opponent_library_choices != 1)
            || (self.pending_private_opponent_library_exile_choice.is_none()
                && outstanding_private_opponent_library_choices != 0)
        {
            return Err(RulesError::IllegalAction(
                "private opponent-library event receipts disagree with the live resolution boundary",
            ));
        }
        self.validate_pending_decision()?;
        for (index, choice) in self.pending_trigger_target_choices.iter().enumerate() {
            let registered = self
                .card_definition(choice.source)
                .ok()
                .and_then(|definition| self.triggered_abilities.get(definition.id))
                .and_then(|abilities| abilities.get(choice.ability.id));
            if choice.ability.targets.is_empty()
                || choice.source_colors.contains(&Color::Colorless)
                || registered != Some(&choice.ability)
                || self.players.get(choice.controller.0).is_none()
                || self.players[choice.controller.0].lost
                || choice.ability.targets.iter().any(|requirement| {
                    self.legal_trigger_targets_for_colors(
                        choice.controller,
                        &choice.source_colors,
                        *requirement,
                    )
                    .is_empty()
                })
                || (index == 0 && self.consecutive_passes != 0)
                || self.pending_draw_replacement.is_some()
                || self.pending_private_library_choice.is_some()
                || self.pending_private_opponent_library_exile_choice.is_some()
                || self.pending_decision.is_some()
                || self.pending_optional_trigger_choice.is_some()
            {
                return Err(RulesError::IllegalAction(
                    "trigger-target choice escaped its no-priority decision boundary",
                ));
            }
        }
        if let Some(choice) = &self.pending_optional_trigger_choice {
            let top = self.stack.last().ok_or(RulesError::IllegalAction(
                "optional trigger decision escaped its stack object",
            ))?;
            let registered = self
                .card_definition(choice.source)
                .ok()
                .and_then(|definition| self.triggered_abilities.get(definition.id))
                .and_then(|abilities| abilities.get(choice.ability.id));
            if top.card != choice.source
                || top.controller != choice.controller
                || top.ability_id != Some(choice.ability.id)
                || top.source_incarnation != choice.source_incarnation
                || top.source_colors != choice.source_colors
                || !choice.ability.optional
                || registered != Some(&choice.ability)
                || self.players[choice.controller.0].lost
                || self.consecutive_passes != 0
                || self.pending_draw_replacement.is_some()
                || self.pending_private_library_choice.is_some()
                || self.pending_private_opponent_library_exile_choice.is_some()
                || self.pending_decision.is_some()
                || !self.pending_trigger_target_choices.is_empty()
            {
                return Err(RulesError::IllegalAction(
                    "optional trigger choice escaped its no-priority resolution boundary",
                ));
            }
        }
        if let Some(choice) = &self.pending_damage_replacement_choice {
            let top = self.stack.last().ok_or(RulesError::IllegalAction(
                "damage replacement choice escaped its stack spell",
            ))?;
            let stack_shape_matches = matches!(
                top.effects.as_slice(),
                [Effect::DealDamage { amount, .. }] if *amount > 0
            );
            let target_is_current =
                self.damage_target_incarnation(choice.target)? == choice.target_incarnation;
            let candidates = self.damage_replacement_candidates(
                choice.source,
                choice.target,
                choice.amount,
                &choice.used,
            )?;
            let used_are_unique = !choice
                .used
                .iter()
                .enumerate()
                .any(|(index, effect)| choice.used[index + 1..].contains(effect));
            if top.card != choice.source
                || top.source_incarnation != choice.source_incarnation
                || top.controller != choice.controller
                || top.ability_id.is_some()
                || top.targets.as_slice() != [choice.original_target]
                || !stack_shape_matches
                || !self.stack_target_incarnation_matches(top, 0, choice.original_target)
                || choice.amount <= 0
                || !target_is_current
                || self.affected_player_for_damage_target(choice.target)? != choice.affected_player
                || candidates.len() < 2
                || !used_are_unique
                || self.priority != choice.affected_player
                || self.consecutive_passes != 0
                || self.players[choice.affected_player.0].lost
                || self.pending_draw_replacement.is_some()
                || self.pending_private_library_choice.is_some()
                || self.pending_private_opponent_library_exile_choice.is_some()
                || self.pending_decision.is_some()
                || !self.pending_trigger_target_choices.is_empty()
                || self.pending_optional_trigger_choice.is_some()
            {
                return Err(RulesError::IllegalAction(
                    "damage replacement choice escaped its no-priority resolution boundary",
                ));
            }
        }
        if self
            .library_search_prevented_until
            .is_some_and(|until_turn| until_turn != self.turn)
        {
            return Err(RulesError::IllegalAction(
                "library-search prevention marker escaped its turn boundary",
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
        for (definition_id, behavior) in &self.land_entry_behaviors {
            let definition = self
                .catalog
                .get(definition_id)
                .ok_or(RulesError::UnknownDefinition(definition_id))?;
            if behavior.card_definition != *definition_id
                || !definition.is_land()
                || !behavior.enters_tapped
            {
                return Err(RulesError::IllegalAction(
                    "land-entry binding has invalid definition or behavior",
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
                            | TriggerCondition::LandEntersBattlefield
                            | TriggerCondition::ControlledLandEntersBattlefield
                            | TriggerCondition::BeginningOfUpkeep
                            | TriggerCondition::LifeGained
                            | TriggerCondition::DealsDamage
                            | TriggerCondition::ReceivesDamage
                            | TriggerCondition::Dies
                            | TriggerCondition::AnotherCreatureDies
                            | TriggerCondition::Attacks
                            | TriggerCondition::CastsNoncreatureSpell
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
                    if object.owner != player.id {
                        return Err(RulesError::IllegalAction(
                            "card is in the wrong player's zone",
                        ));
                    }
                    if object.incarnation == 0
                        || object.entered_turn > self.turn
                        || object.controller_changed_turn > self.turn
                        || object.damage < 0
                        || object.damage_shield < 0
                        || (zone != Zone::Battlefield && !object.counters.is_empty())
                        || object
                            .counters
                            .iter()
                            .any(|(counter, amount)| !counter.is_valid() || *amount <= 0)
                    {
                        return Err(RulesError::IllegalAction(
                            "object has impossible turn metadata, damage/shield, or counters",
                        ));
                    }
                    if let Some(copy) = &object.copied_permanent {
                        if zone != Zone::Battlefield
                            || copy.source == *card
                            || copy.source_incarnation == 0
                            || copy.timestamp == 0
                        {
                            return Err(RulesError::IllegalAction(
                                "permanent copy state has an invalid endpoint or provenance",
                            ));
                        }
                        match &copy.values {
                            CopiableValues::CardDefinition(definition) => {
                                if !self.catalog.contains_key(definition) {
                                    return Err(RulesError::UnknownDefinition(definition));
                                }
                            }
                            CopiableValues::Token(token) => {
                                Self::validate_token_spec(token)?;
                            }
                        }
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
                            Self::validate_card_colors(&token.colors)?;
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
                    if object.controller != object.owner {
                        return Err(RulesError::IllegalAction(
                            "a card object retained a non-owner base controller",
                        ));
                    }
                }
            }
        }
        for (target, sources) in &self.regeneration_shields {
            if sources.is_empty()
                || self.zone_of(*target) != Some(Zone::Battlefield)
                || !self.characteristics(*target).is_ok_and(|characteristics| {
                    characteristics.card_types.contains(&CardType::Creature)
                })
            {
                return Err(RulesError::IllegalAction(
                    "regeneration shield lacks a live battlefield creature target",
                ));
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
            if stack_object.source_incarnation == 0 {
                return Err(RulesError::IllegalAction(
                    "stack object has no source incarnation provenance",
                ));
            }
            if stack_object.source_colors.contains(&Color::Colorless) {
                return Err(RulesError::IllegalAction(
                    "stack object source colors include the colorless mana kind",
                ));
            }
            if !is_ability && object.incarnation != stack_object.source_incarnation {
                return Err(RulesError::IllegalAction(
                    "stack spell does not retain its current stack incarnation",
                ));
            }
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
                && self.controller_of(stack_object.card)? != stack_object.controller
            {
                return Err(RulesError::IllegalAction(
                    "a battlefield ability source must remain controlled by its activator",
                ));
            }
            let definition = self
                .effective_definition_id(stack_object.card)?
                .and_then(|definition| self.catalog.get(definition))
                .ok_or(RulesError::IllegalAction(
                    "a stack object must be a non-token card with a catalog definition",
                ))?;
            if !is_ability && stack_object.source_colors != definition.colors {
                return Err(RulesError::IllegalAction(
                    "stack spell source colors do not match its printed characteristics",
                ));
            }
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
                                ) || matches!(
                                    (bound, actual),
                                    (
                                        Effect::MillTargetPlayerFromSourceDamage,
                                        Effect::MillTargetPlayer { count }
                                    ) if *count > 0
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
            let requires_chosen_x = definition.effects.iter().any(Effect::requires_chosen_x);
            if is_ability && stack_object.chosen_x.is_some() {
                return Err(RulesError::IllegalAction(
                    "a stack ability cannot retain a spell chosen-X value",
                ));
            }
            if !is_ability && requires_chosen_x != stack_object.chosen_x.is_some() {
                return Err(RulesError::IllegalAction(
                    "a chosen-X stack spell lacks or fabricates its selected value",
                ));
            }
            if requires_chosen_x && stack_object.mana_spent.is_none() {
                return Err(RulesError::IllegalAction(
                    "a chosen-X stack spell lacks an explicit payment receipt",
                ));
            }
            if let Some(chosen_x) = stack_object.chosen_x {
                let total_symbols = usize::from(definition.mana_cost.mana_value())
                    .checked_add(usize::from(chosen_x))
                    .ok_or(RulesError::IllegalAction(
                        "chosen-X stack spell has an impossible paid-cost size",
                    ))?;
                let observed_convoke_symbols = self
                    .event_log
                    .iter()
                    .rposition(|event| {
                        matches!(event, GameEvent::SpellCast { card, .. } if *card == stack_object.card)
                    })
                    .and_then(|cast_index| cast_index.checked_sub(1))
                    .filter(|payment_index| {
                        matches!(
                            self.event_log.get(*payment_index),
                            Some(GameEvent::SpellManaPaid { player, card, .. })
                                if *player == stack_object.controller && *card == stack_object.card
                        )
                    })
                    .map_or(0, |payment_index| {
                        self.event_log[..payment_index]
                            .iter()
                            .rev()
                            .take_while(|event| {
                                matches!(
                                    event,
                                    GameEvent::ConvokeUsed { player, .. }
                                        if *player == stack_object.controller
                                )
                            })
                            .count()
                    });
                if stack_object.convoke_symbols != observed_convoke_symbols {
                    return Err(RulesError::IllegalAction(
                        "chosen-X stack spell Convoke provenance disagrees with its cast receipt",
                    ));
                }
                let expected_mana_symbols = total_symbols
                    .checked_sub(usize::from(stack_object.generic_cost_reduction))
                    .and_then(|symbols| symbols.checked_sub(stack_object.convoke_symbols))
                    .ok_or(RulesError::IllegalAction(
                        "chosen-X stack spell cost provenance exceeds its total cost",
                    ))?;
                if stack_object.mana_spent.as_ref().map_or(0, Vec::len) != expected_mana_symbols {
                    return Err(RulesError::IllegalAction(
                        "chosen-X stack spell receipt does not match its post-Convoke cost",
                    ));
                }
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
            if !stack_object.target_incarnations.is_empty()
                && stack_object.target_incarnations.len() != stack_object.targets.len()
            {
                return Err(RulesError::IllegalAction(
                    "stack target incarnation receipt has invalid arity",
                ));
            }
            if stack_object
                .target_incarnations
                .iter()
                .flatten()
                .any(|incarnation| *incarnation == 0)
            {
                return Err(RulesError::IllegalAction(
                    "stack target incarnation receipt contains zero identity",
                ));
            }
            // A target may become illegal after a legal cast (for example, a
            // player can lose or a permanent can leave the battlefield), but
            // it cannot change its enum kind. Validate only immutable target
            // shape here; dynamic legality remains the resolution rule.
            let mut distinct_targets = HashSet::new();
            for (target, requirement) in stack_object.targets.iter().zip(
                definition
                    .effects
                    .iter()
                    .filter_map(Effect::target_requirement),
            ) {
                if !Self::target_shape_matches(*target, requirement) {
                    return Err(RulesError::IllegalTarget(*target));
                }
                if requirement == TargetRequirement::DistinctCreature
                    && !distinct_targets.insert(*target)
                {
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
                        .and_then(CardObject::effective_definition)
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
        for (definition_id, restrictions) in &self.static_attack_restrictions {
            let definition = self
                .catalog
                .get(definition_id)
                .ok_or(RulesError::UnknownDefinition(definition_id))?;
            if !definition.is_permanent()
                || restrictions.is_empty()
                || restrictions
                    .iter()
                    .enumerate()
                    .any(|(index, restriction)| restrictions[..index].contains(restriction))
            {
                return Err(RulesError::IllegalAction(
                    "static attack-restriction binding has invalid source or duplicates",
                ));
            }
        }
        for (definition_id, changes) in &self.static_continuous_effects {
            let definition = self
                .catalog
                .get(definition_id)
                .ok_or(RulesError::UnknownDefinition(definition_id))?;
            if !definition.is_creature()
                || changes.is_empty()
                || changes.iter().any(|change| {
                    !matches!(
                        change,
                        ContinuousChange::ControlledCreatureCountPowerToughness
                            | ContinuousChange::OtherControlledCreaturesModifyPowerToughness { .. }
                            | ContinuousChange::OtherControlledCreaturesAddKeyword(_)
                            | ContinuousChange::ControlledCreaturesAddKeywordIfSourceEnchanted(_)
                    )
                })
            {
                return Err(RulesError::IllegalAction(
                    "static continuous-effect binding has invalid definition or change",
                ));
            }
            if changes
                .iter()
                .enumerate()
                .any(|(index, change)| changes[..index].contains(change))
            {
                return Err(RulesError::IllegalAction(
                    "static continuous-effect binding has duplicate changes",
                ));
            }
        }
        for (definition_id, binding) in &self.cost_reductions {
            let definition = self
                .catalog
                .get(definition_id)
                .ok_or(RulesError::UnknownDefinition(definition_id))?;
            if binding.source_definition != *definition_id
                || !definition.is_permanent()
                || binding.generic_amount == 0
            {
                return Err(RulesError::IllegalAction(
                    "cost-reduction binding has invalid source or amount",
                ));
            }
        }
        for (definition_id, modifiers) in &self.activated_ability_cost_modifiers {
            let definition = self
                .catalog
                .get(definition_id)
                .ok_or(RulesError::UnknownDefinition(definition_id))?;
            if !definition.is_permanent()
                || modifiers.is_empty()
                || modifiers.iter().any(|modifier| {
                    modifier.generic_amount() == 0
                        || modifier.applies_to(ActivatedAbilityKind::Mana)
                })
                || modifiers
                    .iter()
                    .enumerate()
                    .any(|(index, modifier)| modifiers[..index].contains(modifier))
            {
                return Err(RulesError::IllegalAction(
                    "activated-cost modifier binding has invalid source, scope, amount, or duplicate",
                ));
            }
        }
        for (definition_id, effects) in &self.replacement_effects {
            let definition = self
                .catalog
                .get(definition_id)
                .ok_or(RulesError::UnknownDefinition(definition_id))?;
            if !definition.is_permanent()
                || effects.is_empty()
                || effects.iter().any(|effect| effect.multiplier() < 2)
                || effects
                    .iter()
                    .enumerate()
                    .any(|(index, effect)| effects[..index].contains(effect))
            {
                return Err(RulesError::IllegalAction(
                    "replacement-effect binding has invalid source, multiplier, or duplicate",
                ));
            }
        }
        let mut effect_timestamps = BTreeSet::new();
        for card in self.all_battlefield_cards() {
            if let Some(copy) = &self.object(card)?.copied_permanent
                && (copy.timestamp >= self.next_timestamp
                    || !effect_timestamps.insert(copy.timestamp))
            {
                return Err(RulesError::IllegalAction(
                    "permanent-copy timestamps are not unique and monotonic",
                ));
            }
        }
        for effect in &self.continuous_effects {
            if effect.source_incarnation == 0
                || effect.target_incarnation == 0
                || !self.object_has_incarnation(effect.target, effect.target_incarnation)
            {
                return Err(RulesError::IllegalAction(
                    "continuous effect target incarnation is stale or invalid",
                ));
            }
            if matches!(effect.change, ContinuousChange::AddColor(Color::Colorless)) {
                return Err(RulesError::IllegalAction(
                    "continuous effects may not add the colorless mana kind as a card color",
                ));
            }
            if matches!(
                effect.change,
                ContinuousChange::ControlledCreatureCountPowerToughness
                    | ContinuousChange::OtherControlledCreaturesModifyPowerToughness { .. }
                    | ContinuousChange::OtherControlledCreaturesAddKeyword(_)
                    | ContinuousChange::ControlledCreaturesAddKeywordIfSourceEnchanted(_)
            ) {
                return Err(RulesError::IllegalAction(
                    "a static continuous change appeared in the timestamped effect list",
                ));
            }
            if let ContinuousChange::ChangeController(controller) = effect.change {
                if self.players.get(controller.0).is_none() || self.players[controller.0].lost {
                    return Err(RulesError::IllegalAction(
                        "control effect names an invalid or departed controller",
                    ));
                }
                if self.zone_of(effect.target) != Some(Zone::Battlefield) {
                    return Err(RulesError::IllegalAction(
                        "control effect target is not a live battlefield permanent",
                    ));
                }
            }
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
                Duration::Permanent
                    if self.zone_of(effect.source) != Some(Zone::Battlefield)
                        || self.zone_of(effect.target) != Some(Zone::Battlefield)
                        || !self
                            .object_has_incarnation(effect.source, effect.source_incarnation) =>
                {
                    return Err(RulesError::IllegalAction(
                        "permanent continuous effect outlived a battlefield endpoint",
                    ));
                }
                Duration::Permanent => {}
            }
        }
        for aura in self.all_battlefield_cards() {
            let object = self.object(aura)?;
            if object.token.is_some() {
                if object.attached_to.is_some() || object.attached_to_incarnation.is_some() {
                    return Err(RulesError::IllegalAction(
                        "a token permanent retains an attachment target",
                    ));
                }
                continue;
            }
            let Some(definition) = self.effective_definition_id(aura)? else {
                if object.attached_to.is_some() || object.attached_to_incarnation.is_some() {
                    return Err(RulesError::IllegalAction(
                        "a token-value copied permanent retains an attachment target",
                    ));
                }
                continue;
            };
            let attachment_specs = self
                .catalog
                .get(definition)
                .ok_or(RulesError::UnknownDefinition(definition))?
                .effects
                .iter()
                .filter_map(|effect| match effect {
                    Effect::AttachSourceAndModifyTargetPt { power, toughness } => Some((
                        TargetRequirement::Creature,
                        vec![ContinuousChange::ModifyPowerToughness {
                            power: *power,
                            toughness: *toughness,
                        }],
                    )),
                    Effect::AttachSourceToTarget { target, changes } => {
                        Some((*target, changes.clone()))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            if attachment_specs.is_empty() {
                if object.attached_to.is_some() || object.attached_to_incarnation.is_some() {
                    return Err(RulesError::IllegalAction(
                        "a non-aura permanent retains an attachment target",
                    ));
                }
                continue;
            }
            if attachment_specs.len() != 1 {
                return Err(RulesError::IllegalAction(
                    "an aura-like definition has multiple attachment modifiers",
                ));
            }
            let target = object.attached_to.ok_or(RulesError::IllegalAction(
                "a battlefield aura-like permanent has no attachment target",
            ))?;
            let target_incarnation =
                object
                    .attached_to_incarnation
                    .ok_or(RulesError::IllegalAction(
                        "a battlefield aura-like permanent lacks target incarnation provenance",
                    ))?;
            let (requirement, changes) = &attachment_specs[0];
            if self.zone_of(target) != Some(Zone::Battlefield)
                || !self.object_has_incarnation(target, target_incarnation)
                || !self.target_matches_for_source(
                    self.controller_of(aura)?,
                    aura,
                    Target::Permanent(target),
                    *requirement,
                )
            {
                return Err(RulesError::IllegalAction(
                    "a battlefield aura-like permanent has an illegal attachment target",
                ));
            }
            for change in changes {
                let matching_effects = self
                    .continuous_effects
                    .iter()
                    .filter(|effect| {
                        effect.source == aura
                            && effect.target == target
                            && effect.source_incarnation == object.incarnation
                            && effect.target_incarnation == target_incarnation
                            && effect.duration == Duration::Permanent
                            && &effect.change == change
                    })
                    .count();
                if matching_effects != 1 {
                    return Err(RulesError::IllegalAction(
                        "aura attachment lacks exactly one matching continuous effect",
                    ));
                }
            }
        }
        for redirect in &self.damage_redirections {
            self.object(redirect.protected)?;
            self.object(redirect.source)?;
            if redirect.id == 0
                || redirect.id >= self.next_timestamp
                || redirect.remaining <= 0
                || redirect.expires_turn < self.turn
            {
                return Err(RulesError::IllegalAction(
                    "damage redirection has invalid identity, remaining amount, or lifetime",
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
        let mut redirection_ids = BTreeSet::new();
        if self
            .damage_redirections
            .iter()
            .any(|redirect| !redirection_ids.insert(redirect.id))
        {
            return Err(RulesError::IllegalAction(
                "damage redirection identifiers are not unique",
            ));
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
        for shield in &self.damage_prevention_shields {
            self.object(shield.source)?;
            if shield.id == 0
                || shield.id >= self.next_timestamp
                || shield.remaining <= 0
                || shield.expires_turn < self.turn
            {
                return Err(RulesError::IllegalAction(
                    "damage prevention shield has invalid identity, remaining amount, or lifetime",
                ));
            }
            if !self.target_matches(shield.target, TargetRequirement::PlayerOrCreature) {
                return Err(RulesError::IllegalAction(
                    "damage prevention shield has an illegal target",
                ));
            }
        }
        let mut prevention_shield_ids = BTreeSet::new();
        if self
            .damage_prevention_shields
            .iter()
            .any(|shield| !prevention_shield_ids.insert(shield.id))
        {
            return Err(RulesError::IllegalAction(
                "damage prevention shield identifiers are not unique",
            ));
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
                    || !combat.unblockable_attackers.is_empty()
                    || !combat.vigilant_attackers.is_empty()
                    || !combat.trampling_attackers.is_empty()
                    || !combat.must_be_blocked_attackers.is_empty()
                    || !combat.landwalk_attackers.is_empty())
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
                    if object.controller_changed_turn >= self.turn
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
            if !combat.unblockable_attackers.is_subset(&attackers) {
                return Err(RulesError::IllegalAction(
                    "unblockable declaration provenance contains a nonattacker",
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
            if !combat
                .landwalk_attackers
                .keys()
                .all(|attacker| attackers.contains(attacker))
                || combat.landwalk_attackers.values().any(BTreeSet::is_empty)
            {
                return Err(RulesError::IllegalAction(
                    "landwalk declaration provenance is invalid",
                ));
            }
            for (attacker, assigned_blockers) in &combat.blockers {
                if !attackers.contains(attacker) || assigned_blockers.is_empty() {
                    return Err(RulesError::IllegalAction("invalid combat blocker state"));
                }
                if combat.unblockable_attackers.contains(attacker) {
                    return Err(RulesError::IllegalAction(
                        "unblockable attacker has blocker provenance",
                    ));
                }
                for blocker in assigned_blockers {
                    if !blockers.insert(*blocker) {
                        return Err(RulesError::IllegalAction("invalid combat blocker state"));
                    }
                    if self.zone_of(*blocker).is_some() {
                        self.object(*blocker)?;
                    }
                }
            }
            if !combat.removed_from_combat.is_subset(&blockers) {
                return Err(RulesError::IllegalAction(
                    "removed combat provenance names a non-blocker",
                ));
            }
            let expected_evasion_blockers = combat
                .blockers
                .iter()
                .filter(|(attacker, _)| combat.flying_attackers.contains(attacker))
                .flat_map(|(_, blockers)| blockers.iter().copied())
                .collect::<BTreeSet<_>>();
            if combat.evasion_qualified_blockers != expected_evasion_blockers {
                return Err(RulesError::IllegalAction(
                    "flying blocker declaration provenance is incoherent",
                ));
            }
            let expected_fear_blockers = combat
                .blockers
                .iter()
                .filter(|(attacker, _)| combat.fear_attackers.contains(attacker))
                .flat_map(|(_, blockers)| blockers.iter().copied())
                .collect::<BTreeSet<_>>();
            if combat.fear_qualified_blockers != expected_fear_blockers {
                return Err(RulesError::IllegalAction(
                    "Fear blocker declaration provenance is incoherent",
                ));
            }
            let expected_black_evasion_blockers = combat
                .blockers
                .iter()
                .filter(|(attacker, _)| combat.black_evasion_attackers.contains(attacker))
                .flat_map(|(_, blockers)| blockers.iter().copied())
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

    /// Applies every currently applicable quantity replacement exactly once.
    /// The collected sources are a pre-replacement snapshot, so recording a
    /// replacement receipt cannot cause the same source to see its own result.
    fn replace_event_quantity(
        &mut self,
        affected_player: PlayerId,
        event: ReplacementEventKind,
        amount: i16,
    ) -> Result<i16, RulesError> {
        self.player(affected_player)?;
        if amount <= 0 {
            return Err(RulesError::IllegalAction(
                "replacement quantities must be positive",
            ));
        }
        let applicable = self
            .all_battlefield_cards()
            .into_iter()
            .filter(|source| self.controller_of(*source) == Ok(affected_player))
            .flat_map(|source| {
                let Some(definition) = self
                    .objects
                    .get(&source)
                    .and_then(CardObject::effective_definition)
                else {
                    return Vec::new();
                };
                self.replacement_effects
                    .get(definition)
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(move |effect| effect.applies_to(event))
                    .map(move |effect| (source, effect))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut replaced = amount;
        for (source, effect) in applicable {
            let next = replaced.checked_mul(i16::from(effect.multiplier())).ok_or(
                RulesError::IllegalAction("replacement quantity exceeds supported range"),
            )?;
            self.record_event(GameEvent::ReplacementEffectApplied {
                source,
                affected_player,
                event,
                original_amount: replaced,
                replacement_amount: next,
            });
            replaced = next;
        }
        Ok(replaced)
    }

    fn generic_cost_reduction(&self, player: PlayerId, definition: &CardDefinition) -> u8 {
        self.all_battlefield_cards()
            .into_iter()
            .filter_map(|source| {
                if self.controller_of(source) != Ok(player) {
                    return None;
                }
                let source_definition = self.effective_definition_id(source).ok()??;
                let binding = self.cost_reductions.get(source_definition)?;
                if binding.noncreature_only && definition.card_types.contains(&CardType::Creature) {
                    return None;
                }
                Some(binding.generic_amount)
            })
            .fold(0_u8, u8::saturating_add)
    }

    /// Calculates the mana portion of one activated ability's total cost
    /// before any nonmana cost can mutate state.  The first RAV-sufficient
    /// boundary changes only generic symbols: all applicable increases are
    /// added first, then all applicable reductions are applied, while colored
    /// and hybrid symbols remain byte-for-byte identical to the base cost.
    #[allow(clippy::too_many_arguments)] // The context intentionally names every cost component.
    fn calculate_activated_ability_cost(
        &self,
        acting_player: PlayerId,
        source: ObjectId,
        ability_id: &'static str,
        kind: ActivatedAbilityKind,
        base_mana_cost: &ManaCost,
        additional_tap_creatures: u8,
        sacrifice_source: bool,
        sacrifice_creatures: u8,
        sacrifice_lands: u8,
        discard_cards: u8,
        payment_selection: Option<ManaPaymentSelection>,
    ) -> Result<ActivatedAbilityCostContext, RulesError> {
        let source_object = self.object(source)?;
        let mut increases = Vec::new();
        let mut reductions = Vec::new();
        for modifier_source in self.all_battlefield_cards() {
            let object = self.object(modifier_source)?;
            let Some(definition) = self.effective_definition_id(modifier_source)? else {
                continue;
            };
            let Some(modifiers) = self.activated_ability_cost_modifiers.get(definition) else {
                continue;
            };
            for modifier in modifiers {
                if !modifier.applies_to(kind) {
                    continue;
                }
                let adjustment = ActivatedAbilityCostAdjustment {
                    source: modifier_source,
                    source_incarnation: object.incarnation,
                    generic_amount: modifier.generic_amount(),
                };
                match modifier {
                    ActivatedAbilityCostModifier::IncreaseGeneric { .. } => {
                        increases.push(adjustment);
                    }
                    ActivatedAbilityCostModifier::ReduceGeneric { .. } => {
                        reductions.push(adjustment);
                    }
                }
            }
        }
        increases.sort_by_key(|adjustment| adjustment.source);
        reductions.sort_by_key(|adjustment| adjustment.source);
        let total_increase = increases
            .iter()
            .try_fold(u16::from(base_mana_cost.generic), |total, adjustment| {
                total.checked_add(u16::from(adjustment.generic_amount))
            })
            .ok_or(RulesError::IllegalAction(
                "activated ability generic cost exceeds supported range",
            ))?;
        let total_reduction = reductions
            .iter()
            .try_fold(0_u16, |total, adjustment| {
                total.checked_add(u16::from(adjustment.generic_amount))
            })
            .ok_or(RulesError::IllegalAction(
                "activated ability generic cost exceeds supported range",
            ))?;
        let effective_generic = total_increase.saturating_sub(total_reduction);
        let effective_generic = u8::try_from(effective_generic).map_err(|_| {
            RulesError::IllegalAction("activated ability generic cost exceeds supported range")
        })?;
        let effective_mana_cost = ManaCost {
            generic: effective_generic,
            colored: base_mana_cost.colored.clone(),
            hybrid: base_mana_cost.hybrid.clone(),
        };
        Ok(ActivatedAbilityCostContext {
            acting_player,
            source,
            source_incarnation: source_object.incarnation,
            ability_id,
            kind,
            base_mana_cost: base_mana_cost.clone(),
            increases,
            reductions,
            additional_tap_creatures,
            sacrifice_source,
            sacrifice_creatures,
            sacrifice_lands,
            discard_cards,
            payment_selection,
            effective_mana_cost,
        })
    }

    /// Emits calculated-cost provenance only when at least one live modifier
    /// actually changed the activation.  Untaxed legacy traces retain their
    /// established receipts, while a taxed or reduced cost remains replayable
    /// from its base symbols and exact live source incarnations.
    fn record_activated_ability_cost_context_if_modified(
        &mut self,
        context: ActivatedAbilityCostContext,
    ) {
        if !context.increases.is_empty() || !context.reductions.is_empty() {
            self.record_event(GameEvent::ActivatedAbilityCostCalculated { context });
        }
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
            if self.controller_of(payment.creature)? != player || creature.tapped {
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
        controller: PlayerId,
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
        let mut distinct_targets = HashSet::new();
        for (target, requirement) in targets.iter().zip(requirements) {
            if !self.target_matches_for_colors(controller, *target, requirement, &definition.colors)
            {
                return Err(RulesError::IllegalTarget(*target));
            }
            if requirement == TargetRequirement::DistinctCreature
                && !distinct_targets.insert(*target)
            {
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
                        || self.controller_of(*card)? != player
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
    #[allow(clippy::too_many_lines)] // One exhaustive, auditable effect preflight table.
    fn validate_cast_effects(definition: &CardDefinition) -> Result<(), RulesError> {
        let private_library_choice_effects = definition
            .effects
            .iter()
            .filter(|effect| {
                matches!(
                    effect,
                    Effect::LookAtTopCardsChooseForLifeOrGraveyard { .. }
                )
            })
            .count();
        if private_library_choice_effects > 0 && definition.effects.len() != 1 {
            return Err(RulesError::IllegalAction(
                "a private-library choice spell must contain exactly one effect",
            ));
        }
        let policy_submitted_library_searches = definition
            .effects
            .iter()
            .filter(|effect| {
                matches!(
                    effect,
                    Effect::SearchControllerLibrary {
                        selection: LibrarySearchSelection::PolicySubmitted { .. },
                        ..
                    }
                )
            })
            .count();
        if policy_submitted_library_searches > 0 && definition.effects.len() != 1 {
            return Err(RulesError::IllegalAction(
                "a policy-submitted library search spell must contain exactly one effect",
            ));
        }
        for effect in &definition.effects {
            if matches!(
                effect,
                Effect::LookAtTopCardsChooseForLifeOrGraveyard { count: 0, .. }
                    | Effect::LookAtTopCardsChooseForLifeOrGraveyard {
                        life_per_card: ..=0,
                        ..
                    }
            ) {
                return Err(RulesError::IllegalAction(
                    "private-library choice must inspect cards for a positive life payment",
                ));
            }
            if matches!(effect, Effect::DiscardTargetPlayer { count: 0 }) {
                return Err(RulesError::IllegalAction(
                    "targeted discard must request at least one card",
                ));
            }
            if matches!(
                effect,
                Effect::SearchControllerLibrary {
                    requirement: LibrarySearchRequirement::BasicLandTypes(types),
                    ..
                } if types.is_empty()
            ) {
                return Err(RulesError::IllegalAction(
                    "library search must name at least one allowed land type",
                ));
            }
            let amount = match effect {
                Effect::DealDamage { amount, .. }
                | Effect::LoseLifeTarget { amount }
                | Effect::LoseLifeController { amount }
                | Effect::ReturnTargetPermanentToHandAndLoseControllerLife { amount }
                | Effect::DealDamageController { amount }
                | Effect::DealDamageAfterOptionalManaPayment { amount, .. }
                | Effect::DealDamageToEachCreatureAndPlayer { amount }
                | Effect::DealDamageToEachPlayer { amount }
                | Effect::DealDamageToEachNonFlyingCreature { amount }
                | Effect::RadianceDealDamageToCreatures { amount }
                | Effect::BeginDamageRedirection { amount }
                | Effect::GainLifeController { amount }
                | Effect::MillTargetPlayer { count: amount } => *amount,
                Effect::AddManaController { amount, .. } => i16::from(*amount),
                Effect::CreateToken { .. }
                | Effect::CreateTokenForTargetPlayer { .. }
                | Effect::AttachSourceToTarget { .. }
                | Effect::GainControlTargetUntilEndOfTurn
                | Effect::AddPlusOneCounterToSource
                | Effect::LoseLifeEachOpponentEqualToControlledCreatures
                | Effect::DiscardOneCardEachPlayer
                | Effect::DiscardTargetPlayer { .. }
                | Effect::SacrificeControllerCreature
                | Effect::CompleteDamageRedirection
                | Effect::DrawControllerIfManaColorSpent { .. }
                | Effect::ModifyAllCreaturesPtUntilEndOfTurnIfManaColorSpent { .. }
                | Effect::DrawController
                | Effect::DrawTargetPlayer
                | Effect::PreventLibrarySearchUntilEndOfTurn
                | Effect::SearchControllerLibrary { .. }
                | Effect::AttachSourceAndModifyTargetPt { .. }
                | Effect::RevealTopCardPutIntoHandLoseLifeEqualToManaValue
                | Effect::GainLifeControllerFromSourceDamage
                | Effect::MillTargetPlayerFromSourceDamage
                | Effect::GainLifeForEachCreature
                | Effect::DealDamageToEachPlayerFromReceivedDamage
                | Effect::DealDamageEqualToAttackingCreatures { .. }
                | Effect::ModifyTargetPtUntilEndOfTurn { .. }
                | Effect::ModifyTargetPtAndKeywordUntilEndOfTurn { .. }
                | Effect::ModifyTargetKeywordUntilEndOfTurn { .. }
                | Effect::PreventTargetBlockingSourceUntilEndOfTurn
                | Effect::ModifySourcePtUntilEndOfTurn { .. }
                | Effect::RemoveSourceKeywordUntilEndOfTurn { .. }
                | Effect::AddSourceDamageShieldUntilEndOfTurn { .. }
                | Effect::AddTargetDamageShieldUntilEndOfTurn { .. }
                | Effect::RegenerateTargetCreature
                | Effect::RegenerateSource
                | Effect::AddKeywordToControllerCreaturesUntilEndOfTurn { .. }
                | Effect::DestroyTargetLand
                | Effect::DestroyTargetLandAndUntapSourceIfNonbasic
                | Effect::DestroyTargetArtifact
                | Effect::DestroyTargetFlyingCreature
                | Effect::DestroyTargetNonblackCreature
                | Effect::AddPlusOneCounterToTarget
                | Effect::DestroyTargetArtifactOrCreatureNoRegeneration
                | Effect::DestroyDistinctTargetCreature
                | Effect::DestroyTargetCreatureWithManaValueAtMostChosenX
                | Effect::TapTargetCreature
                | Effect::UntapSource
                | Effect::UntapTargetLand
                | Effect::DestroyTargetArtifactOrEnchantment
                | Effect::ReturnTargetCardToHand
                | Effect::ReturnTargetEnchantmentCardToHand
                | Effect::ReturnTargetCreatureCardToHandIfAnotherInControllerGraveyard
                | Effect::ReturnTargetCreatureCardToBattlefieldWithCounterIfManaColorSpent {
                    ..
                }
                | Effect::ReturnOneCreatureCardFromEachGraveyardToHand
                | Effect::ReturnUpToThreeControllerGraveyardLandCardsToHand
                | Effect::LookAtTopCardsChooseForLifeOrGraveyard { .. }
                | Effect::LookAtTopCardsOfTargetOpponentExileOne { .. }
                | Effect::ShuffleGraveyardsIntoLibraries
                | Effect::ReturnControlledCreatureToHand
                | Effect::ReturnControlledLandToHand
                | Effect::ReturnOpponentCreatureToHand
                | Effect::ModifyControllerCreaturesPtUntilEndOfTurn { .. }
                | Effect::RadianceUntapAndModifyUntilEndOfTurn { .. }
                | Effect::RadianceModifyPtUntilEndOfTurn { .. }
                | Effect::RadianceAddKeywordUntilEndOfTurn { .. }
                | Effect::RadianceDestroyEnchantments
                | Effect::DestroyAllNonTokenCreatures
                | Effect::CounterTargetInstantOrSorcerySpell
                | Effect::SacrificeCreatureOrCounterTargetSpell
                | Effect::GrantGraveyardCastPermissionUntilEndOfTurn
                | Effect::ExileTargetCreature
                | Effect::ExileTargetPermanent
                | Effect::ExileAttachedCreatureAndAurasUntilEndStep => continue,
                Effect::AddCountersToSource { counter, amount }
                | Effect::AddCountersToTarget { counter, amount }
                | Effect::RemoveCountersFromSource { counter, amount }
                | Effect::RemoveCountersFromTarget { counter, amount } => {
                    if !counter.is_valid() || *amount <= 0 {
                        return Err(RulesError::IllegalAction(
                            "counter effects require a valid kind and positive amount",
                        ));
                    }
                    continue;
                }
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
        self.resolve_top_of_stack_with_optional_decision(None)
    }

    #[allow(clippy::too_many_lines)] // Spell and activated-ability resolution share one audited path.
    fn resolve_top_of_stack_with_optional_decision(
        &mut self,
        optional_decision: Option<(bool, Option<Target>)>,
    ) -> Result<(), RulesError> {
        if optional_decision.is_none()
            && let Some(top) = self.stack.last()
            && let Some(ability_id) = top.ability_id
            && let Some(ability) = self
                .triggered_abilities
                .get(self.card_definition(top.card)?.id)
                .and_then(|abilities| abilities.get(ability_id))
                .filter(|ability| ability.optional)
        {
            self.pending_optional_trigger_choice = Some(PendingOptionalTriggeredAbilityChoice {
                source: top.card,
                source_incarnation: top.source_incarnation,
                source_colors: top.source_colors.clone(),
                controller: top.controller,
                ability: ability.clone(),
            });
            self.priority = top.controller;
            self.consecutive_passes = 0;
            return Ok(());
        }
        if self.suspend_top_stack_item_for_library_search_choice()? {
            return Ok(());
        }
        if self.suspend_top_spell_for_private_library_choice()? {
            return Ok(());
        }
        if self.suspend_top_trigger_for_effect_object_choice()? {
            return Ok(());
        }
        if self.suspend_top_ability_for_private_opponent_library_exile_choice()? {
            return Ok(());
        }
        if self.suspend_top_stack_item_for_damage_replacement_choice()? {
            return Ok(());
        }
        let stack_object = self.stack.pop().ok_or(RulesError::IllegalAction(
            "attempted to resolve an empty stack",
        ))?;
        // Target legality is snapshotted once, per target occurrence, before
        // any instruction resolves to establish the all-illegal boundary. An
        // initially legal slot is rechecked before its own instruction: an
        // earlier instruction can legally remove a later repeated target.
        // The model-owned plan preserves repeated targets as independent slots.
        let mut target_index = 0;
        let plan = stack_object
            .resolution_plan(|target, requirement| {
                let occurrence = target_index;
                target_index += 1;
                self.stack_target_incarnation_matches(&stack_object, occurrence, target)
                    && self.target_matches_for_colors(
                        stack_object.controller,
                        target,
                        requirement,
                        &stack_object.source_colors,
                    )
            })
            .map_err(|_| RulesError::IllegalAction("stack object has an invalid target count"))?;
        if matches!(plan, StackResolutionPlan::CounteredByRules) {
            if let Some(ability) = stack_object.ability_id {
                self.record_event(GameEvent::AbilityCounteredByRules {
                    source: stack_object.card,
                    source_incarnation: stack_object.source_incarnation,
                    ability,
                });
            } else {
                self.record_event(GameEvent::SpellCounteredByRules {
                    card: stack_object.card,
                });
                self.move_to_spell_terminal_zone(stack_object.card)?;
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
        // Triggered costs are paid on resolution.  In particular, an attack
        // trigger must be visible on the stack before its controller gets the
        // post-declaration priority window in which to activate mana abilities.
        let mut trigger_payment_paid = true;
        if let Some(ability_id) = stack_object.ability_id
            && let Some(ability) = self
                .triggered_abilities
                .get(self.card_definition(stack_object.card)?.id)
                .and_then(|abilities| abilities.get(ability_id))
        {
            let should_pay = if ability.optional {
                optional_decision
                    .as_ref()
                    .map(|decision| decision.0)
                    .ok_or(RulesError::IllegalAction(
                        "optional triggered cost resolved without a policy decision",
                    ))?
            } else {
                true
            };
            if !should_pay {
                trigger_payment_paid = false;
            }
            if should_pay && ability.mana_cost.mana_value() > 0 {
                let mut paid_pool = self.players[stack_object.controller.0].mana_pool.clone();
                paid_pool
                    .pay(&ability.mana_cost)
                    .map_err(RulesError::Mana)?;
                self.players[stack_object.controller.0].mana_pool = paid_pool;
                self.record_event(GameEvent::AbilityManaPaid {
                    player: stack_object.controller,
                    source: stack_object.card,
                    ability: ability.id,
                    mana_cost: ability.mana_cost.clone(),
                });
            }
        }
        let mut pending_aura_attachment: Option<(
            ObjectId,
            TargetRequirement,
            Vec<ContinuousChange>,
        )> = None;
        let mut target_index = 0;
        for (effect_index, (effect, target_resolution)) in stack_object
            .effects
            .iter()
            .zip(effect_resolutions)
            .enumerate()
        {
            if !trigger_payment_paid {
                continue;
            }
            match target_resolution {
                StackEffectResolution::Untargeted => {
                    if let Effect::DealDamageAfterOptionalManaPayment { amount, .. } = effect {
                        let selected = optional_decision
                            .as_ref()
                            .and_then(|decision| decision.1)
                            .ok_or(RulesError::IllegalAction(
                            "paid optional trigger is missing its conditional target",
                        ))?;
                        match selected {
                            Target::Player(player) => self.deal_damage_to_player(
                                stack_object.card,
                                player,
                                i32::from(*amount),
                            )?,
                            Target::Permanent(permanent) => self
                                .deal_damage_to_permanent_from_colors(
                                    stack_object.card,
                                    &stack_object.source_colors,
                                    permanent,
                                    i32::from(*amount),
                                )?,
                            Target::Spell(card) => {
                                return Err(RulesError::IllegalTarget(Target::Spell(card)));
                            }
                            Target::SacrificePermanent(card) => {
                                return Err(RulesError::IllegalTarget(Target::SacrificePermanent(
                                    card,
                                )));
                            }
                        }
                        continue;
                    }
                    self.resolve_effect(
                        stack_object.card,
                        stack_object.source_incarnation,
                        &stack_object.source_colors,
                        stack_object.controller,
                        stack_object.chosen_x,
                        stack_object.mana_spent.as_deref(),
                        effect,
                        None,
                    )?;
                }
                StackEffectResolution::Targeted {
                    target,
                    legal: true,
                } => {
                    let occurrence = target_index;
                    target_index += 1;
                    let requirement =
                        effect
                            .target_requirement()
                            .ok_or(RulesError::IllegalAction(
                                "target-resolution plan named an untargeted effect",
                            ))?;
                    if self.stack_target_incarnation_matches(&stack_object, occurrence, target)
                        && self.target_matches_for_colors(
                            stack_object.controller,
                            target,
                            requirement,
                            &stack_object.source_colors,
                        )
                    {
                        if let Effect::AttachSourceAndModifyTargetPt { power, toughness } = effect {
                            if pending_aura_attachment
                                .replace((
                                    Self::target_permanent(Some(target))?,
                                    TargetRequirement::Creature,
                                    vec![ContinuousChange::ModifyPowerToughness {
                                        power: *power,
                                        toughness: *toughness,
                                    }],
                                ))
                                .is_some()
                            {
                                return Err(RulesError::IllegalAction(
                                    "a permanent spell has multiple attachment effects",
                                ));
                            }
                        } else if let Effect::AttachSourceToTarget {
                            target: requirement,
                            changes,
                        } = effect
                        {
                            if pending_aura_attachment
                                .replace((
                                    Self::target_permanent(Some(target))?,
                                    *requirement,
                                    changes.clone(),
                                ))
                                .is_some()
                            {
                                return Err(RulesError::IllegalAction(
                                    "a permanent spell has multiple attachment effects",
                                ));
                            }
                        } else {
                            self.resolve_effect(
                                stack_object.card,
                                stack_object.source_incarnation,
                                &stack_object.source_colors,
                                stack_object.controller,
                                stack_object.chosen_x,
                                stack_object.mana_spent.as_deref(),
                                effect,
                                Some(target),
                            )?;
                        }
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
                    target_index += 1;
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
            if pending_aura_attachment.is_some() {
                return Err(RulesError::IllegalAction(
                    "an activated ability cannot establish an aura attachment",
                ));
            }
            self.record_event(GameEvent::AbilityResolved {
                source: stack_object.card,
                source_incarnation: stack_object.source_incarnation,
                ability,
            });
            self.check_state_based_actions()?;
            self.flush_pending_land_entry_triggers()?;
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
            self.record_game_end_if_needed();
            return Ok(());
        }
        self.record_event(GameEvent::SpellResolved {
            card: stack_object.card,
        });
        let definition_id = self.card_definition(stack_object.card)?.id;
        let permanent_resolution = self.card_definition(stack_object.card)?.is_permanent();
        let entering_is_land = self.card_definition(stack_object.card)?.is_land();
        let entering_controller = self.object(stack_object.card)?.controller;
        if permanent_resolution {
            self.move_to_zone(stack_object.card, Zone::Battlefield)?;
            if let Some((target, requirement, changes)) = pending_aura_attachment {
                self.attach_aura_with_changes(stack_object.card, target, requirement, changes)?;
            }
        } else if pending_aura_attachment.is_some() {
            return Err(RulesError::IllegalAction(
                "an aura attachment effect requires a permanent spell",
            ));
        } else {
            self.move_to_spell_terminal_zone(stack_object.card)?;
        }
        self.check_state_based_actions()?;
        self.flush_pending_dies_triggers();
        if permanent_resolution {
            self.enqueue_enter_triggers(stack_object.card, definition_id, entering_controller);
            if entering_is_land {
                self.enqueue_land_entry_triggers(entering_controller)?;
            }
        }
        self.flush_pending_land_entry_triggers()?;
        self.flush_pending_damage_triggers();
        self.flush_pending_life_gain_triggers();
        self.flush_pending_dies_triggers();
        self.priority = self.priority_after_resolution();
        Ok(())
    }

    /// Opens the bounded no-priority damage-replacement boundary. This first
    /// slice intentionally supports only one targeted direct-damage spell;
    /// complex multi-instruction and partial-redirection continuations remain
    /// on the existing deterministic path until they receive their own red
    /// regression and resumable stack representation.
    fn suspend_top_stack_item_for_damage_replacement_choice(&mut self) -> Result<bool, RulesError> {
        if self.pending_damage_replacement_choice.is_some() {
            return Err(RulesError::IllegalAction(
                "a second damage replacement choice attempted to open during resolution",
            ));
        }
        let Some(top) = self.stack.last() else {
            return Ok(false);
        };
        let (source, source_incarnation, controller, target, amount, requirement) = match (
            top.ability_id,
            top.targets.as_slice(),
            top.effects.as_slice(),
        ) {
            (
                None,
                [target],
                [
                    Effect::DealDamage {
                        amount,
                        target: requirement,
                    },
                ],
            ) => (
                top.card,
                top.source_incarnation,
                top.controller,
                *target,
                i32::from(*amount),
                *requirement,
            ),
            _ => return Ok(false),
        };
        if amount <= 0
            || !self.stack_target_incarnation_matches(top, 0, target)
            || !self.target_matches_for_source(controller, source, target, requirement)
        {
            // The ordinary resolver owns malformed/illegal target handling.
            return Ok(false);
        }
        let candidates = self.damage_replacement_candidates(source, target, amount, &[])?;
        if candidates.len() < 2 {
            return Ok(false);
        }
        let affected_player = self.affected_player_for_damage_target(target)?;
        self.pending_damage_replacement_choice = Some(PendingDamageReplacementChoice {
            source,
            source_incarnation,
            controller,
            affected_player,
            original_target: target,
            target,
            target_incarnation: self.damage_target_incarnation(target)?,
            amount,
            used: Vec::new(),
        });
        self.priority = affected_player;
        self.consecutive_passes = 0;
        Ok(true)
    }

    /// Opens a no-priority boundary for a stack item whose typed library
    /// search requires an explicit controller selection. Search prevention
    /// bypasses the boundary: the ordinary resolver records a failed search
    /// and its required shuffle without exposing nonexistent candidates.
    fn suspend_top_stack_item_for_library_search_choice(&mut self) -> Result<bool, RulesError> {
        if self.pending_decision.is_some()
            || self.pending_private_library_choice.is_some()
            || self.pending_private_opponent_library_exile_choice.is_some()
        {
            return Err(RulesError::IllegalAction(
                "a second hidden-library choice attempted to open during resolution",
            ));
        }
        if self.library_search_prevented_until == Some(self.turn) {
            return Ok(false);
        }
        let Some(top) = self.stack.last() else {
            return Ok(false);
        };
        let (source, controller, requirement, destination, may_fail_to_find, chosen_x) =
            match top.effects.as_slice() {
                [
                    Effect::SearchControllerLibrary {
                        requirement,
                        destination,
                        selection: LibrarySearchSelection::PolicySubmitted { may_fail_to_find },
                    },
                ] => (
                    top.card,
                    top.controller,
                    requirement.clone(),
                    *destination,
                    *may_fail_to_find,
                    top.chosen_x,
                ),
                _ => return Ok(false),
            };
        let cards = self.library_search_candidates(controller, &requirement, chosen_x)?;
        let options = cards
            .into_iter()
            .map(DecisionOption::Object)
            .collect::<Vec<_>>();
        let (min_selections, max_selections) = if options.is_empty() {
            (0, 0)
        } else if may_fail_to_find {
            (0, 1)
        } else {
            (1, 1)
        };
        self.open_pending_decision(
            controller,
            DecisionVisibility::Private,
            DecisionKind::LibrarySearch,
            min_selections,
            max_selections,
            options,
            DecisionContinuation::LibrarySearch {
                source,
                requirement,
                destination,
                may_fail_to_find,
            },
        )?;
        Ok(true)
    }

    /// Opens the no-priority boundary for the one-effect private library
    /// selection substrate. The spell stays on the stack until its controller
    /// submits the selection, so no player can respond using information that
    /// belongs to its unresolved hidden-zone instruction.
    fn suspend_top_spell_for_private_library_choice(&mut self) -> Result<bool, RulesError> {
        if self.pending_decision.is_some()
            || self.pending_private_library_choice.is_some()
            || self.pending_private_opponent_library_exile_choice.is_some()
        {
            return Err(RulesError::IllegalAction(
                "a second private-library choice attempted to open during resolution",
            ));
        }
        let Some(top) = self.stack.last() else {
            return Ok(false);
        };
        let (spell, controller, count, life_per_card) =
            match (top.ability_id, top.effects.as_slice()) {
                (
                    None,
                    [
                        Effect::LookAtTopCardsChooseForLifeOrGraveyard {
                            count,
                            life_per_card,
                        },
                    ],
                ) => (top.card, top.controller, *count, *life_per_card),
                (_, [Effect::LookAtTopCardsChooseForLifeOrGraveyard { .. }]) => {
                    return Err(RulesError::IllegalAction(
                        "private-library choice effect is unsupported on an ability",
                    ));
                }
                _ => return Ok(false),
            };
        if count == 0 || life_per_card <= 0 {
            return Err(RulesError::IllegalAction(
                "private-library choice spell has invalid selection parameters",
            ));
        }
        let cards = self.players[controller.0]
            .library
            .iter()
            .rev()
            .take(usize::from(count))
            .copied()
            .collect::<Vec<_>>();
        self.record_event(GameEvent::CardsLookedAt {
            viewer: controller,
            cards: cards.clone(),
        });
        self.pending_private_library_choice = Some(PendingPrivateLibraryChoice {
            spell,
            controller,
            cards,
            life_per_card,
        });
        self.priority = controller;
        self.consecutive_passes = 0;
        Ok(true)
    }

    /// Opens the no-priority boundary for an activated ability that privately
    /// inspects a target opponent's library and must choose one available card
    /// for exile. The stack ability remains live while its controller decides;
    /// only the controller's `GameView` receives candidate identities.
    fn suspend_top_ability_for_private_opponent_library_exile_choice(
        &mut self,
    ) -> Result<bool, RulesError> {
        if self.pending_decision.is_some()
            || self.pending_private_library_choice.is_some()
            || self.pending_private_opponent_library_exile_choice.is_some()
        {
            return Err(RulesError::IllegalAction(
                "a second private-library choice attempted to open during resolution",
            ));
        }
        let Some(top) = self.stack.last() else {
            return Ok(false);
        };
        let (source, source_incarnation, ability, controller, opponent, count) = match (
            top.ability_id,
            top.targets.as_slice(),
            top.effects.as_slice(),
        ) {
            (
                Some(ability),
                [Target::Player(opponent)],
                [Effect::LookAtTopCardsOfTargetOpponentExileOne { count }],
            ) => (
                top.card,
                top.source_incarnation,
                ability,
                top.controller,
                *opponent,
                *count,
            ),
            _ => return Ok(false),
        };
        if count == 0 {
            return Err(RulesError::IllegalAction(
                "private opponent-library choice must inspect at least one card",
            ));
        }
        // An illegal target is handled by the ordinary all-targets-illegal
        // rules-counter path below. It must not reveal or snapshot cards.
        if !self.target_matches_for_controller(
            controller,
            Target::Player(opponent),
            TargetRequirement::Opponent,
        ) {
            return Ok(false);
        }
        let cards = self.players[opponent.0]
            .library
            .iter()
            .rev()
            .take(usize::from(count))
            .copied()
            .collect::<Vec<_>>();
        self.record_event(GameEvent::PrivateOpponentLibraryChoiceOpened {
            controller,
            source,
            source_incarnation,
            ability,
            opponent,
            count: u8::try_from(cards.len()).map_err(|_| {
                RulesError::IllegalAction(
                    "private opponent-library choice count exceeds event range",
                )
            })?,
        });
        self.pending_private_opponent_library_exile_choice =
            Some(PendingPrivateOpponentLibraryExileChoice {
                source,
                ability,
                controller,
                opponent,
                cards,
            });
        self.priority = controller;
        self.consecutive_passes = 0;
        Ok(true)
    }

    fn enqueue_enter_triggers(
        &mut self,
        source: ObjectId,
        definition: &'static str,
        controller: PlayerId,
    ) {
        self.enqueue_triggers_for_source(
            source,
            definition,
            controller,
            TriggerCondition::EntersBattlefield,
        );
        self.flush_pending_trigger_events();
    }

    /// A cast trigger retains the exact spell that caused it, rather than
    /// choosing an arbitrary visible spell. The trigger is placed above that
    /// spell after the `SpellCast` receipt and before its caster receives the
    /// normal post-cast priority window.
    fn enqueue_cast_noncreature_triggers(
        &mut self,
        controller: PlayerId,
        spell: ObjectId,
    ) -> Result<(), RulesError> {
        let sources = self
            .all_battlefield_cards()
            .into_iter()
            .filter(|source| self.controller_of(*source) == Ok(controller))
            .collect::<Vec<_>>();
        for source in sources {
            let Some(definition) = self.effective_definition_id(source)? else {
                continue;
            };
            let source_colors = self.characteristics(source)?.colors;
            let triggers = self
                .triggered_abilities
                .get(definition)
                .into_iter()
                .flat_map(|abilities| abilities.values())
                .filter(|ability| ability.condition == TriggerCondition::CastsNoncreatureSpell)
                .cloned()
                .collect::<Vec<_>>();
            for ability in triggers {
                if ability.targets != [TargetRequirement::NoncreatureSpell] {
                    return Err(RulesError::IllegalAction(
                        "a cast-noncreature trigger must retain one spell target",
                    ));
                }
                self.pending_trigger_events
                    .push(PendingTriggeredAbilityEvent {
                        source,
                        source_incarnation: self.object(source)?.incarnation,
                        source_colors: source_colors.clone(),
                        controller,
                        ability,
                        payload: TriggerEventPayload::ExactTargets(vec![Target::Spell(spell)]),
                    });
            }
        }
        self.flush_pending_trigger_events();
        Ok(())
    }

    /// Stacks every represented land-entry trigger on a live permanent. A
    /// land entering does not have to share a controller with the triggered
    /// source: the observer is the source permanent, not the land-play
    /// action. Each represented entry path calls this after the land is live,
    /// state-based actions are stable, and its own ETB triggers are queued.
    fn enqueue_land_entry_triggers(
        &mut self,
        entering_controller: PlayerId,
    ) -> Result<(), RulesError> {
        let sources = self.all_battlefield_cards();
        for source in sources {
            let (definition, controller) = {
                (
                    self.effective_definition_id(source)?,
                    self.controller_of(source)?,
                )
            };
            let Some(definition) = definition else {
                continue;
            };
            self.enqueue_triggers_for_source(
                source,
                definition,
                controller,
                TriggerCondition::LandEntersBattlefield,
            );
            if controller == entering_controller {
                self.enqueue_triggers_for_source(
                    source,
                    definition,
                    controller,
                    TriggerCondition::ControlledLandEntersBattlefield,
                );
            }
        }
        self.flush_pending_trigger_events();
        Ok(())
    }

    fn queue_land_entry_trigger_batch(
        &mut self,
        entering_controller: PlayerId,
    ) -> Result<(), RulesError> {
        self.player(entering_controller)?;
        self.pending_land_entry_trigger_batches
            .push(entering_controller);
        Ok(())
    }

    fn flush_pending_land_entry_triggers(&mut self) -> Result<(), RulesError> {
        let controllers = std::mem::take(&mut self.pending_land_entry_trigger_batches);
        for controller in controllers {
            self.enqueue_land_entry_triggers(controller)?;
        }
        Ok(())
    }

    fn enqueue_triggers_for_source(
        &mut self,
        source: ObjectId,
        definition: &'static str,
        controller: PlayerId,
        condition: TriggerCondition,
    ) {
        let source_incarnation = match self.object(source) {
            Ok(object) => object.incarnation,
            Err(_) => return,
        };
        let source_colors = match self.characteristics(source) {
            Ok(characteristics) => characteristics.colors,
            Err(_) => return,
        };
        self.enqueue_triggers_for_source_at_incarnation(
            source,
            source_incarnation,
            &source_colors,
            definition,
            controller,
            condition,
        );
    }

    /// Queues a trigger using a last-known source incarnation captured when
    /// its condition occurred.  Dies triggers call this after zone movement,
    /// when querying the card again would otherwise report its graveyard
    /// incarnation instead of the historical battlefield object.
    fn enqueue_triggers_for_source_at_incarnation(
        &mut self,
        source: ObjectId,
        source_incarnation: u64,
        source_colors: &BTreeSet<Color>,
        definition: &'static str,
        controller: PlayerId,
        condition: TriggerCondition,
    ) {
        let triggers = self
            .triggered_abilities
            .get(definition)
            .into_iter()
            .flat_map(|abilities| abilities.values())
            .filter(|ability| ability.condition == condition)
            .cloned()
            .collect::<Vec<_>>();
        for ability in triggers {
            if ability.effects.iter().any(|effect| {
                matches!(
                    effect,
                    Effect::ReturnTargetCreatureCardToHandIfAnotherInControllerGraveyard
                )
            }) && self.controller_creature_cards_in_graveyard(controller) < 2
            {
                // This is an intervening condition: no ability is put on the
                // stack unless the target creature card has another creature
                // card alongside it in its controller's graveyard.
                continue;
            }
            self.pending_trigger_events
                .push(PendingTriggeredAbilityEvent {
                    source,
                    source_incarnation,
                    source_colors: source_colors.clone(),
                    controller,
                    ability,
                    payload: TriggerEventPayload::None,
                });
        }
    }

    /// Moves every just-observed trigger condition into one APNAP-ordered
    /// placement batch.  The active player's triggers are placed first (and
    /// therefore sit lower on the stack), followed by each next living seat.
    /// Within one controller the public binding/source order remains stable
    /// until policy-supplied ordering is added as a later decision layer.
    fn flush_pending_trigger_events(&mut self) {
        let events = std::mem::take(&mut self.pending_trigger_events);
        if events.is_empty() {
            return;
        }
        for offset in 0..self.players.len() {
            let controller = PlayerId((self.active_player.0 + offset) % self.players.len());
            if self.players[controller.0].lost {
                continue;
            }
            self.pending_trigger_placements.extend(
                events
                    .iter()
                    .filter(|event| event.controller == controller)
                    .cloned(),
            );
        }
        self.advance_pending_trigger_placements();
    }

    /// Continues a deterministic trigger-placement batch until a controller
    /// must choose targets.  It is also called immediately after that choice,
    /// so a later targetless trigger cannot jump ahead of an earlier
    /// target-bearing one while priority remains blocked.
    fn advance_pending_trigger_placements(&mut self) {
        if !self.pending_trigger_target_choices.is_empty() {
            return;
        }
        while !self.pending_trigger_placements.is_empty() {
            let event = self.pending_trigger_placements.remove(0);
            if self.players[event.controller.0].lost {
                continue;
            }
            let effects = Self::materialize_trigger_effects(&event.ability, &event.payload);
            let exact_targets = match &event.payload {
                TriggerEventPayload::ExactTargets(targets) => Some(targets.clone()),
                TriggerEventPayload::None | TriggerEventPayload::DamageAmount(_) => None,
            };
            if let Some(targets) = exact_targets {
                if targets.len() == event.ability.targets.len()
                    && targets
                        .iter()
                        .zip(&event.ability.targets)
                        .all(|(target, requirement)| {
                            self.target_matches_for_colors(
                                event.controller,
                                *target,
                                *requirement,
                                &event.source_colors,
                            )
                        })
                {
                    self.stack_triggered_event(&event, targets, effects);
                }
                continue;
            }
            if event.ability.targets.is_empty() {
                self.stack_triggered_event(&event, Vec::new(), effects);
                continue;
            }
            if event.ability.targets.iter().all(|requirement| {
                !self
                    .legal_trigger_targets_for_colors(
                        event.controller,
                        &event.source_colors,
                        *requirement,
                    )
                    .is_empty()
            }) {
                self.pending_trigger_target_choices
                    .push(PendingTriggeredAbilityTargetChoice {
                        source: event.source,
                        source_incarnation: event.source_incarnation,
                        source_colors: event.source_colors,
                        controller: event.controller,
                        ability: event.ability,
                        effects,
                    });
                break;
            }
            // The represented trigger has no legal target at its trigger
            // placement boundary, so it cannot become a legal stack object.
        }
    }

    fn materialize_trigger_effects(
        ability: &crate::TriggeredAbility,
        payload: &TriggerEventPayload,
    ) -> Vec<Effect> {
        ability
            .effects
            .iter()
            .cloned()
            .map(|effect| match (effect, payload) {
                (
                    Effect::GainLifeControllerFromSourceDamage,
                    TriggerEventPayload::DamageAmount(amount),
                ) => Effect::GainLifeController { amount: *amount },
                (
                    Effect::DealDamageToEachPlayerFromReceivedDamage,
                    TriggerEventPayload::DamageAmount(amount),
                ) => Effect::DealDamageToEachPlayer { amount: *amount },
                (
                    Effect::MillTargetPlayerFromSourceDamage,
                    TriggerEventPayload::DamageAmount(amount),
                ) => Effect::MillTargetPlayer { count: *amount },
                (effect, _) => effect,
            })
            .collect()
    }

    fn stack_triggered_event(
        &mut self,
        event: &PendingTriggeredAbilityEvent,
        targets: Vec<Target>,
        effects: Vec<Effect>,
    ) {
        let target_incarnations = self.target_incarnations(&targets);
        self.stack.push(StackObject {
            card: event.source,
            source_incarnation: event.source_incarnation,
            source_colors: event.source_colors.clone(),
            controller: event.controller,
            ability_id: Some(event.ability.id),
            targets,
            target_incarnations,
            effects,
            chosen_x: None,
            mana_spent: None,
            convoke_symbols: 0,
            generic_cost_reduction: 0,
        });
        self.record_event(GameEvent::TriggeredAbilityStacked {
            controller: event.controller,
            source: event.source,
            source_incarnation: event.source_incarnation,
            ability: event.ability.id,
        });
        self.consecutive_passes = 0;
    }

    /// Stacks each permanent controlled by the active player whose ability
    /// triggers at the beginning of upkeep.  This runs after the public
    /// `StepBegan` receipt and before either player receives priority, which
    /// preserves the mandatory trigger window at the state-machine boundary.
    fn enqueue_upkeep_triggers(&mut self) -> Result<(), RulesError> {
        let controller = self.active_player;
        let sources = self
            .all_battlefield_cards()
            .into_iter()
            .filter(|source| self.controller_of(*source) == Ok(controller))
            .collect::<Vec<_>>();
        for source in sources {
            let (source_controller, source_is_token, source_incarnation, source_colors) = {
                let object = self.object(source)?;
                (
                    self.controller_of(source)?,
                    object.token.is_some(),
                    object.incarnation,
                    self.characteristics(source)?.colors,
                )
            };
            if source_controller != controller || source_is_token {
                continue;
            }
            let definition = self.card_definition(source)?.id;
            let triggers = self
                .triggered_abilities
                .get(definition)
                .into_iter()
                .flat_map(|abilities| abilities.values())
                .filter(|ability| ability.condition == TriggerCondition::BeginningOfUpkeep)
                .cloned()
                .collect::<Vec<_>>();
            for ability in triggers {
                self.pending_trigger_events
                    .push(PendingTriggeredAbilityEvent {
                        source,
                        source_incarnation,
                        source_colors: source_colors.clone(),
                        controller,
                        ability,
                        payload: TriggerEventPayload::None,
                    });
            }
        }
        self.flush_pending_trigger_events();
        Ok(())
    }

    fn select_trigger_targets(
        &self,
        source: ObjectId,
        controller: PlayerId,
        requirements: &[TargetRequirement],
    ) -> Option<Vec<Target>> {
        let mut targets = Vec::with_capacity(requirements.len());
        for requirement in requirements {
            let opponent_player = (0..self.players.len())
                .map(PlayerId)
                .find(|player| *player != controller && !self.players[player.0].lost)
                .map(Target::Player)
                .filter(|target| {
                    self.target_matches_for_source(controller, source, *target, *requirement)
                });
            let opponent_permanent = self
                .objects
                .keys()
                .copied()
                .filter(|candidate| self.zone_of(*candidate) == Some(Zone::Battlefield))
                .filter(|candidate| {
                    self.controller_of(*candidate)
                        .is_ok_and(|target_controller| target_controller != controller)
                })
                .map(Target::Permanent)
                .find(|target| {
                    self.target_matches_for_source(controller, source, *target, *requirement)
                });
            let any_permanent = self
                .objects
                .keys()
                .copied()
                .filter(|candidate| self.zone_of(*candidate) == Some(Zone::Battlefield))
                .map(Target::Permanent)
                .find(|target| {
                    self.target_matches_for_source(controller, source, *target, *requirement)
                });
            let any_player = (0..self.players.len())
                .map(PlayerId)
                .filter(|player| !self.players[player.0].lost)
                .map(Target::Player)
                .find(|target| {
                    self.target_matches_for_source(controller, source, *target, *requirement)
                });
            let controller_graveyard_card = self.players[controller.0]
                .graveyard
                .iter()
                .copied()
                .map(Target::Permanent)
                .find(|target| {
                    self.target_matches_for_source(controller, source, *target, *requirement)
                });
            let target = opponent_player
                .or(opponent_permanent)
                .or(any_permanent)
                .or(any_player)
                .or(controller_graveyard_card)?;
            targets.push(target);
        }
        Some(targets)
    }

    fn legal_trigger_targets_for_colors(
        &self,
        controller: PlayerId,
        source_colors: &BTreeSet<Color>,
        requirement: TargetRequirement,
    ) -> Vec<Target> {
        (0..self.players.len())
            .map(PlayerId)
            .map(Target::Player)
            .chain(self.objects.keys().copied().map(Target::Permanent))
            .chain(self.stack.iter().map(|item| Target::Spell(item.card)))
            .filter(|target| {
                self.target_matches_for_colors(controller, *target, requirement, source_colors)
            })
            .collect()
    }

    fn choose_triggered_ability_targets(
        &mut self,
        player: PlayerId,
        source: ObjectId,
        ability_id: &'static str,
        targets: Vec<Target>,
    ) -> Result<(), RulesError> {
        self.atomic_transition(|game| {
            let choice = game.pending_trigger_target_choices.first().cloned().ok_or(
                RulesError::IllegalAction("no triggered ability is awaiting targets"),
            )?;
            if player != choice.controller {
                return Err(RulesError::IllegalAction(
                    "only the trigger controller may choose its targets",
                ));
            }
            if source != choice.source || ability_id != choice.ability.id {
                return Err(RulesError::IllegalAction(
                    "submitted trigger identity does not match the pending choice",
                ));
            }
            if targets.len() != choice.ability.targets.len()
                || targets
                    .iter()
                    .zip(&choice.ability.targets)
                    .any(|(target, requirement)| {
                        !game.target_matches_for_colors(
                            choice.controller,
                            *target,
                            *requirement,
                            &choice.source_colors,
                        )
                    })
            {
                return Err(RulesError::IllegalAction(
                    "submitted trigger targets are not legal for every target slot",
                ));
            }
            game.pending_trigger_target_choices.remove(0);
            let target_incarnations = game.target_incarnations(&targets);
            game.stack.push(StackObject {
                card: choice.source,
                source_incarnation: choice.source_incarnation,
                source_colors: choice.source_colors,
                controller: choice.controller,
                ability_id: Some(choice.ability.id),
                targets,
                target_incarnations,
                effects: choice.effects,
                chosen_x: None,
                mana_spent: None,
                convoke_symbols: 0,
                generic_cost_reduction: 0,
            });
            game.record_event(GameEvent::TriggeredAbilityStacked {
                controller: choice.controller,
                source: choice.source,
                source_incarnation: choice.source_incarnation,
                ability: choice.ability.id,
            });
            game.consecutive_passes = 0;
            game.advance_pending_trigger_placements();
            Ok(())
        })
    }

    fn optional_trigger_target_requirement(
        ability: &crate::TriggeredAbility,
    ) -> Option<TargetRequirement> {
        ability.effects.iter().find_map(|effect| match effect {
            Effect::DealDamageAfterOptionalManaPayment { target, .. } => Some(*target),
            _ => None,
        })
    }

    fn resolve_optional_triggered_ability(
        &mut self,
        player: PlayerId,
        source: ObjectId,
        ability_id: &'static str,
        pay: bool,
        target: Option<Target>,
    ) -> Result<(), RulesError> {
        self.atomic_transition(|game| {
            let choice =
                game.pending_optional_trigger_choice
                    .clone()
                    .ok_or(RulesError::IllegalAction(
                        "no optional triggered ability is awaiting a decision",
                    ))?;
            if player != choice.controller
                || source != choice.source
                || ability_id != choice.ability.id
            {
                return Err(RulesError::IllegalAction(
                    "optional trigger decision does not match the pending controller or identity",
                ));
            }
            let requirement = Self::optional_trigger_target_requirement(&choice.ability);
            match (pay, requirement, target) {
                (false, _, None) | (true, None, None) => {}
                (true, Some(requirement), Some(target))
                    if game.target_matches_for_colors(
                        choice.controller,
                        target,
                        requirement,
                        &choice.source_colors,
                    ) => {}
                _ => {
                    return Err(RulesError::IllegalAction(
                        "optional trigger payment has an invalid conditional target",
                    ));
                }
            }
            if pay {
                let mut pool = game.players[player.0].mana_pool.clone();
                pool.pay(&choice.ability.mana_cost)
                    .map_err(RulesError::Mana)?;
            }
            game.pending_optional_trigger_choice = None;
            game.resolve_top_of_stack_with_optional_decision(Some((pay, target)))
        })
    }

    /// Resolves one affected-player replacement choice. The selected effect is
    /// applied exactly once, then applicability is recomputed against the
    /// transformed prospective event before damage is committed.
    fn choose_damage_replacement(
        &mut self,
        player: PlayerId,
        source: ObjectId,
        source_incarnation: u64,
        target: Target,
        replacement: DamageReplacementChoice,
    ) -> Result<(), RulesError> {
        self.atomic_transition(|game| {
            let mut pending =
                game.pending_damage_replacement_choice
                    .take()
                    .ok_or(RulesError::IllegalAction(
                        "no prospective damage event is awaiting replacement selection",
                    ))?;
            if player != pending.affected_player
                || source != pending.source
                || source_incarnation != pending.source_incarnation
                || target != pending.target
            {
                return Err(RulesError::IllegalAction(
                    "submitted damage replacement identity does not match the pending event",
                ));
            }
            if pending.target_incarnation != game.damage_target_incarnation(pending.target)? {
                return Err(RulesError::IllegalAction(
                    "prospective damage target changed incarnation before replacement selection",
                ));
            }
            let candidates = game.damage_replacement_candidates(
                pending.source,
                pending.target,
                pending.amount,
                &pending.used,
            )?;
            if candidates.len() < 2 || !candidates.contains(&replacement) {
                return Err(RulesError::IllegalAction(
                    "submitted damage replacement is not one of the live choice options",
                ));
            }
            game.apply_damage_replacement(&mut pending, replacement)?;
            let original_target = pending.original_target;
            match game.advance_damage_replacement_pipeline(pending)? {
                Some(next) => {
                    game.priority = next.affected_player;
                    game.consecutive_passes = 0;
                    game.pending_damage_replacement_choice = Some(next);
                }
                None => game.finish_suspended_damage_replacement_spell(
                    source,
                    source_incarnation,
                    original_target,
                )?,
            }
            Ok(())
        })
    }

    /// Finishes the one-effect instant/sorcery retained by the bounded
    /// replacement decision. The direct-damage instruction has already
    /// committed (or been fully prevented), so this records only the normal
    /// spell terminal lifecycle and its post-resolution trigger/SBA boundary.
    fn finish_suspended_damage_replacement_spell(
        &mut self,
        source: ObjectId,
        source_incarnation: u64,
        original_target: Target,
    ) -> Result<(), RulesError> {
        let stack_object = self.stack.pop().ok_or(RulesError::IllegalAction(
            "damage replacement decision escaped its stack spell",
        ))?;
        if stack_object.card != source
            || stack_object.source_incarnation != source_incarnation
            || stack_object.ability_id.is_some()
            || stack_object.targets.as_slice() != [original_target]
            || !matches!(stack_object.effects.as_slice(), [Effect::DealDamage { .. }])
        {
            return Err(RulesError::IllegalAction(
                "damage replacement continuation has an invalid stack shape",
            ));
        }
        self.record_event(GameEvent::SpellResolved { card: source });
        self.move_to_spell_terminal_zone(source)?;
        self.check_state_based_actions()?;
        self.flush_pending_dies_triggers();
        self.flush_pending_damage_triggers();
        self.flush_pending_life_gain_triggers();
        self.flush_pending_dies_triggers();
        self.priority = self.priority_after_resolution();
        Ok(())
    }

    fn choose_triggered_ability_effect_object(
        &mut self,
        player: PlayerId,
        source: ObjectId,
        ability: &'static str,
        selected: Option<ObjectId>,
    ) -> Result<(), RulesError> {
        self.atomic_transition(|game| {
            let decision = game.pending_decision.as_ref().ok_or(RulesError::IllegalAction(
                "no triggered ability is awaiting an effect-object choice",
            ))?;
            if !matches!(
                decision.continuation,
                DecisionContinuation::TriggeredEffectObject {
                    source: pending_source,
                    ability: pending_ability,
                    ..
                } if pending_source == source && pending_ability == ability
            ) {
                return Err(RulesError::IllegalAction(
                    "trigger effect-object compatibility action does not match the pending decision",
                ));
            }
            game.resolve_pending_decision(
                player,
                decision.id,
                DecisionSelection::Objects(selected.into_iter().collect()),
            )
        })
    }

    fn finish_trigger_effect_object_choice(
        &mut self,
        source: ObjectId,
        ability: &'static str,
        apply: impl FnOnce(&mut Self) -> Result<(), RulesError>,
    ) -> Result<(), RulesError> {
        let top = self.stack.last().ok_or(RulesError::IllegalAction(
            "trigger effect-object choice has no live stack ability",
        ))?;
        if top.card != source
            || top.controller != self.controller_of(source)?
            || top.ability_id != Some(ability)
        {
            return Err(RulesError::IllegalAction(
                "trigger effect-object choice no longer matches its stack ability",
            ));
        }
        let source_incarnation = top.source_incarnation;
        self.stack.pop();
        apply(self)?;
        self.record_event(GameEvent::AbilityResolved {
            source,
            source_incarnation,
            ability,
        });
        self.check_state_based_actions()?;
        self.flush_pending_land_entry_triggers()?;
        self.flush_pending_damage_triggers();
        self.flush_pending_life_gain_triggers();
        self.flush_pending_dies_triggers();
        self.priority = self.priority_after_resolution();
        Ok(())
    }

    fn suspend_top_trigger_for_effect_object_choice(&mut self) -> Result<bool, RulesError> {
        if self.pending_decision.is_some() {
            return Err(RulesError::IllegalAction(
                "a second typed decision attempted to open during resolution",
            ));
        }
        let Some(top) = self.stack.last() else {
            return Ok(false);
        };
        let Some(ability) = top.ability_id else {
            return Ok(false);
        };
        let kind = match top.effects.as_slice() {
            [Effect::DiscardOneCardEachPlayer] => {
                TriggeredEffectObjectDecisionKind::DiscardEachPlayer {
                    remaining_players: self
                        .players
                        .iter()
                        .filter(|player| !player.lost)
                        .map(|player| player.id)
                        .collect(),
                    selected: Vec::new(),
                }
            }
            [Effect::SacrificeControllerCreature] => {
                TriggeredEffectObjectDecisionKind::SacrificeControllerCreature
            }
            _ => return Ok(false),
        };
        let chooser = match &kind {
            TriggeredEffectObjectDecisionKind::DiscardEachPlayer {
                remaining_players, ..
            } => *remaining_players.first().ok_or(RulesError::IllegalAction(
                "a continuing game has no player for trigger discard choice",
            ))?,
            TriggeredEffectObjectDecisionKind::SacrificeControllerCreature => top.controller,
        };
        let candidates = match &kind {
            TriggeredEffectObjectDecisionKind::DiscardEachPlayer { .. } => {
                self.players[chooser.0].hand.clone()
            }
            TriggeredEffectObjectDecisionKind::SacrificeControllerCreature => self
                .all_battlefield_cards()
                .into_iter()
                .filter(|card| {
                    self.controller_of(*card)
                        .is_ok_and(|controller| controller == chooser)
                        && self.characteristics(*card).is_ok_and(|characteristics| {
                            characteristics.card_types.contains(&CardType::Creature)
                        })
                })
                .collect(),
        };
        let options = candidates
            .into_iter()
            .map(DecisionOption::Object)
            .collect::<Vec<_>>();
        let (min_selections, max_selections) = if options.is_empty() { (0, 0) } else { (1, 1) };
        self.open_pending_decision(
            chooser,
            match &kind {
                TriggeredEffectObjectDecisionKind::DiscardEachPlayer { .. } => {
                    DecisionVisibility::Private
                }
                TriggeredEffectObjectDecisionKind::SacrificeControllerCreature => {
                    DecisionVisibility::Public
                }
            },
            DecisionKind::TriggeredEffectObject,
            min_selections,
            max_selections,
            options,
            DecisionContinuation::TriggeredEffectObject {
                source: top.card,
                controller: top.controller,
                ability,
                kind,
            },
        )?;
        Ok(true)
    }

    fn controller_creature_cards_in_graveyard(&self, controller: PlayerId) -> usize {
        self.players[controller.0]
            .graveyard
            .iter()
            .filter(|card| {
                self.card_definition(**card)
                    .is_ok_and(|definition| definition.card_types.contains(&CardType::Creature))
            })
            .count()
    }

    /// Captures attack-trigger conditions for the shared placement pipeline.
    /// Target-bearing triggers are not allowed to reach the stack until that
    /// pipeline opens the same controller decision used by every condition.
    fn enqueue_attack_triggers(&mut self, source: ObjectId) -> Result<(), RulesError> {
        // Tokens have no catalog definition and therefore cannot have a
        // definition-bound attack trigger in this substrate. Their attack is
        // still fully legal; simply skip the definition lookup and continue
        // through the ordinary post-declaration priority transition.
        if self.object(source)?.token.is_some() {
            return Ok(());
        }
        let definition = self.card_definition(source)?.id;
        let controller = self.controller_of(source)?;
        let source_colors = self.characteristics(source)?.colors;
        let triggers = self
            .triggered_abilities
            .get(definition)
            .into_iter()
            .flat_map(|abilities| abilities.values())
            .filter(|ability| ability.condition == TriggerCondition::Attacks)
            .cloned()
            .collect::<Vec<_>>();
        for ability in triggers {
            self.pending_trigger_events
                .push(PendingTriggeredAbilityEvent {
                    source,
                    source_incarnation: self.object(source)?.incarnation,
                    source_colors: source_colors.clone(),
                    controller,
                    ability,
                    payload: TriggerEventPayload::None,
                });
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
        let controller = self.controller_of(source)?;
        let source_colors = self.characteristics(source)?.colors;
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
            self.pending_trigger_events
                .push(PendingTriggeredAbilityEvent {
                    source,
                    source_incarnation: self.object(source)?.incarnation,
                    source_colors: source_colors.clone(),
                    controller,
                    ability,
                    payload: TriggerEventPayload::DamageAmount(damage_amount),
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
        let controller = self.controller_of(recipient)?;
        let source_colors = self.characteristics(recipient)?.colors;
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
            self.pending_trigger_events
                .push(PendingTriggeredAbilityEvent {
                    source: recipient,
                    source_incarnation: self.object(recipient)?.incarnation,
                    source_colors: source_colors.clone(),
                    controller,
                    ability,
                    payload: TriggerEventPayload::DamageAmount(damage_amount),
                });
        }
        Ok(())
    }

    /// Queues a source-specific dies trigger after the permanent changes
    /// zones. Unlike damage-batch triggers this is stacked immediately at the
    /// state-based-action boundary, while retaining the dead card object as
    /// the historical source for resolution and event auditing.
    fn enqueue_dies_triggers(
        &mut self,
        source: ObjectId,
        source_incarnation: u64,
        source_colors: &BTreeSet<Color>,
        definition: &'static str,
        controller: PlayerId,
    ) {
        self.enqueue_triggers_for_source_at_incarnation(
            source,
            source_incarnation,
            source_colors,
            definition,
            controller,
            TriggerCondition::Dies,
        );
    }

    /// Captures every battlefield permanent with an "another creature dies"
    /// trigger before the dying object leaves. This preserves last-known
    /// battlefield state for simultaneous creature deaths and token deaths.
    fn enqueue_another_creature_dies_triggers(
        &mut self,
        dying_creature: ObjectId,
    ) -> Result<(), RulesError> {
        if !self
            .characteristics(dying_creature)?
            .card_types
            .contains(&CardType::Creature)
        {
            return Ok(());
        }
        let observers = self
            .all_battlefield_cards()
            .into_iter()
            .filter(|source| *source != dying_creature)
            .filter_map(|source| {
                let object = self.object(source).ok()?;
                if object.token.is_some() {
                    return None;
                }
                self.card_definition(source)
                    .ok()
                    .map(|definition| (source, definition.id))
            })
            .filter(|(_, definition)| {
                self.triggered_abilities
                    .get(definition)
                    .into_iter()
                    .flat_map(|abilities| abilities.values())
                    .any(|ability| ability.condition == TriggerCondition::AnotherCreatureDies)
            })
            .collect::<Vec<_>>();
        for (source, definition) in observers {
            let controller = self.object(source)?.controller;
            self.enqueue_triggers_for_source(
                source,
                definition,
                controller,
                TriggerCondition::AnotherCreatureDies,
            );
        }
        Ok(())
    }

    /// Captures life-gain triggers only from permanents controlled by the
    /// player who gained positive life. Their optional mana costs are paid at
    /// trigger resolution; the pending queue retains the source and bound
    /// ability until the enclosing resolution completes.
    fn enqueue_life_gain_triggers(&mut self, life_gain_player: PlayerId) {
        let sources = self
            .all_battlefield_cards()
            .into_iter()
            .filter_map(|source| {
                let definition = self.card_definition(source).ok()?.id;
                let object = self.object(source).ok()?;
                (self.controller_of(source).ok()? == life_gain_player).then_some((
                    source,
                    definition,
                    self.controller_of(source).ok()?,
                    object.incarnation,
                    self.characteristics(source).ok()?.colors,
                ))
            })
            .collect::<Vec<_>>();
        for (source, definition, controller, source_incarnation, source_colors) in sources {
            let triggers = self
                .triggered_abilities
                .get(definition)
                .into_iter()
                .flat_map(|abilities| abilities.values())
                .filter(|ability| ability.condition == TriggerCondition::LifeGained)
                .cloned()
                .collect::<Vec<_>>();
            for ability in triggers {
                self.pending_trigger_events
                    .push(PendingTriggeredAbilityEvent {
                        source,
                        source_incarnation,
                        source_colors: source_colors.clone(),
                        controller,
                        ability,
                        payload: TriggerEventPayload::None,
                    });
            }
        }
    }

    /// Places dies triggers above the already-completed action's stack object.
    /// Deferring until costs/effects finish preserves the rule that a trigger
    /// created during an activation cost is stacked after that activation.
    fn flush_pending_dies_triggers(&mut self) {
        self.flush_pending_trigger_events();
    }

    /// Pushes all triggers observed during the just-completed damage batch.
    /// Their effects are materialized from the captured event amount before
    /// the `TriggeredAbilityStacked` receipt is emitted.
    fn flush_pending_damage_triggers(&mut self) {
        self.flush_pending_trigger_events();
    }

    /// Puts life-gain triggers on the stack after the enclosing effect has
    /// finished. Target legality is rechecked by the normal stack resolver,
    /// while the trigger's source and controller remain historical identities.
    fn flush_pending_life_gain_triggers(&mut self) {
        self.flush_pending_trigger_events();
    }

    fn damage_cannot_be_prevented(&self, source: ObjectId) -> bool {
        self.characteristics(source).is_ok_and(|characteristics| {
            characteristics
                .keywords
                .contains(&Keyword::DamageCannotBePrevented)
        })
    }

    fn target_prevents_damage_from_colors(
        &self,
        target: ObjectId,
        source_colors: &BTreeSet<Color>,
    ) -> bool {
        self.characteristics(target).is_ok_and(|characteristics| {
            characteristics.keywords.iter().any(|keyword| {
                matches!(
                    keyword,
                    Keyword::PreventDamageFromColor(color)
                        | Keyword::Protection(color)
                        if source_colors.contains(color)
                )
            })
        })
    }

    fn target_prevents_damage_from_source(&self, source: ObjectId, target: ObjectId) -> bool {
        self.characteristics(source).is_ok_and(|characteristics| {
            self.target_prevents_damage_from_colors(target, &characteristics.colors)
        })
    }

    /// Lists every represented replacement applicable to one prospective
    /// damage event. A previously used replacement is excluded even if its
    /// source remains live, preventing one effect from recursively replacing
    /// its own result.  This is intentionally bounded to the existing RAV
    /// shields/protection/redirection substrate; it does not claim general
    /// replacement-effect coverage for arbitrary future effects.
    fn damage_replacement_candidates(
        &self,
        source: ObjectId,
        target: Target,
        amount: i32,
        used: &[DamageReplacementChoice],
    ) -> Result<Vec<DamageReplacementChoice>, RulesError> {
        if amount <= 0 {
            return Ok(Vec::new());
        }
        let mut candidates = Vec::new();
        let prevention_allowed = !self.damage_cannot_be_prevented(source);
        match target {
            Target::Permanent(permanent) => {
                // A bounded redirected event must be wholly redirected. The
                // legacy direct path retains support for partial redirects;
                // that broader continuation is an explicit future extension.
                for redirect in &self.damage_redirections {
                    let candidate = DamageReplacementChoice::Redirect {
                        id: redirect.id,
                        source: redirect.source,
                        protected: redirect.protected,
                        destination: redirect.destination,
                    };
                    if redirect.protected == permanent
                        && redirect.remaining >= amount
                        && redirect.destination != target
                        && self.target_matches(
                            redirect.destination,
                            TargetRequirement::PlayerOrCreature,
                        )
                        && !used.contains(&candidate)
                    {
                        candidates.push(candidate);
                    }
                }
                if prevention_allowed {
                    if self.target_prevents_damage_from_source(source, permanent) {
                        let candidate =
                            DamageReplacementChoice::SourceColorPrevention { permanent };
                        if !used.contains(&candidate) {
                            candidates.push(candidate);
                        }
                    }
                    for shield in &self.damage_prevention_shields {
                        let candidate = DamageReplacementChoice::TargetedShield {
                            id: shield.id,
                            source: shield.source,
                            target: shield.target,
                        };
                        if shield.target == target
                            && shield.remaining > 0
                            && shield.expires_turn >= self.turn
                            && !used.contains(&candidate)
                        {
                            candidates.push(candidate);
                        }
                    }
                    if self.object(permanent)?.damage_shield > 0 {
                        let candidate = DamageReplacementChoice::PermanentShield { permanent };
                        if !used.contains(&candidate) {
                            candidates.push(candidate);
                        }
                    }
                }
            }
            Target::Player(_) => {
                if prevention_allowed {
                    for shield in &self.damage_prevention_shields {
                        let candidate = DamageReplacementChoice::TargetedShield {
                            id: shield.id,
                            source: shield.source,
                            target: shield.target,
                        };
                        if shield.target == target
                            && shield.remaining > 0
                            && shield.expires_turn >= self.turn
                            && !used.contains(&candidate)
                        {
                            candidates.push(candidate);
                        }
                    }
                }
            }
            Target::Spell(card) => return Err(RulesError::IllegalTarget(Target::Spell(card))),
            Target::SacrificePermanent(card) => {
                return Err(RulesError::IllegalTarget(Target::SacrificePermanent(card)));
            }
        }
        Ok(candidates)
    }

    fn affected_player_for_damage_target(&self, target: Target) -> Result<PlayerId, RulesError> {
        match target {
            Target::Player(player) if !self.player(player)?.lost => Ok(player),
            Target::Permanent(permanent) => self.controller_of(permanent),
            Target::Player(player) => {
                Err(RulesError::IllegalAction(if self.player(player)?.lost {
                    "an eliminated player cannot choose a damage replacement"
                } else {
                    "damage replacement target has no affected player"
                }))
            }
            Target::Spell(card) => Err(RulesError::IllegalTarget(Target::Spell(card))),
            Target::SacrificePermanent(card) => {
                Err(RulesError::IllegalTarget(Target::SacrificePermanent(card)))
            }
        }
    }

    fn damage_target_incarnation(&self, target: Target) -> Result<Option<u64>, RulesError> {
        match target {
            Target::Player(_) => Ok(None),
            Target::Permanent(permanent) => Ok(Some(self.object(permanent)?.incarnation)),
            Target::Spell(card) => Err(RulesError::IllegalTarget(Target::Spell(card))),
            Target::SacrificePermanent(card) => {
                Err(RulesError::IllegalTarget(Target::SacrificePermanent(card)))
            }
        }
    }

    #[allow(clippy::too_many_lines)] // Each typed replacement owns a distinct state/event transition.
    fn apply_damage_replacement(
        &mut self,
        pending: &mut PendingDamageReplacementChoice,
        replacement: DamageReplacementChoice,
    ) -> Result<(), RulesError> {
        let candidates = self.damage_replacement_candidates(
            pending.source,
            pending.target,
            pending.amount,
            &pending.used,
        )?;
        if !candidates.contains(&replacement) {
            return Err(RulesError::IllegalAction(
                "submitted damage replacement is no longer applicable",
            ));
        }
        self.record_event(GameEvent::DamageReplacementApplied {
            affected_player: pending.affected_player,
            target: pending.target,
            replacement,
        });
        match replacement {
            DamageReplacementChoice::Redirect {
                id,
                source: redirect_source,
                protected,
                destination,
            } => {
                let exhausted = {
                    let redirect = self
                        .damage_redirections
                        .iter_mut()
                        .find(|redirect| {
                            redirect.id == id
                                && redirect.source == redirect_source
                                && redirect.protected == protected
                                && redirect.destination == destination
                                && redirect.remaining >= pending.amount
                        })
                        .ok_or(RulesError::IllegalAction(
                            "damage redirection disappeared before selection",
                        ))?;
                    redirect.remaining -= pending.amount;
                    redirect.remaining == 0
                };
                self.record_event(GameEvent::DamageRedirected {
                    source: pending.source,
                    from: protected,
                    to: destination,
                    amount: pending.amount,
                });
                if exhausted {
                    self.damage_redirections
                        .retain(|candidate| candidate.id != id);
                }
                pending.target = destination;
                pending.target_incarnation = self.damage_target_incarnation(destination)?;
                pending.affected_player = self.affected_player_for_damage_target(destination)?;
            }
            DamageReplacementChoice::TargetedShield {
                id,
                source: shield_source,
                target,
            } => {
                let shield = self
                    .damage_prevention_shields
                    .iter_mut()
                    .find(|shield| {
                        shield.id == id
                            && shield.source == shield_source
                            && shield.target == target
                            && shield.remaining > 0
                    })
                    .ok_or(RulesError::IllegalAction(
                        "damage-prevention shield disappeared before selection",
                    ))?;
                let prevented = pending.amount.min(shield.remaining);
                shield.remaining -= prevented;
                if shield.remaining == 0 {
                    self.damage_prevention_shields
                        .retain(|candidate| candidate.id != id);
                }
                pending.amount -= prevented;
                self.record_event(GameEvent::DamagePrevented {
                    source: pending.source,
                    target,
                    amount: prevented,
                });
            }
            DamageReplacementChoice::PermanentShield { permanent } => {
                let object = self
                    .objects
                    .get_mut(&permanent)
                    .ok_or(RulesError::UnknownCard(permanent))?;
                let prevented = pending.amount.min(object.damage_shield);
                if prevented <= 0 {
                    return Err(RulesError::IllegalAction(
                        "permanent damage shield disappeared before selection",
                    ));
                }
                object.damage_shield -= prevented;
                pending.amount -= prevented;
                self.record_event(GameEvent::DamagePrevented {
                    source: pending.source,
                    target: Target::Permanent(permanent),
                    amount: prevented,
                });
            }
            DamageReplacementChoice::SourceColorPrevention { permanent } => {
                if !self.target_prevents_damage_from_source(pending.source, permanent) {
                    return Err(RulesError::IllegalAction(
                        "color prevention disappeared before selection",
                    ));
                }
                let prevented = pending.amount;
                pending.amount = 0;
                self.record_event(GameEvent::DamagePrevented {
                    source: pending.source,
                    target: Target::Permanent(permanent),
                    amount: prevented,
                });
            }
        }
        pending.used.push(replacement);
        Ok(())
    }

    /// Applies required single replacements until the prospective event is
    /// ready to commit or needs another affected-player choice.  Only the
    /// latter returns `Some`; replacement receipts precede any resulting
    /// `DamageDealt*` receipt, so triggers see committed damage only.
    fn advance_damage_replacement_pipeline(
        &mut self,
        mut pending: PendingDamageReplacementChoice,
    ) -> Result<Option<PendingDamageReplacementChoice>, RulesError> {
        loop {
            if pending.amount == 0 {
                return Ok(None);
            }
            let candidates = self.damage_replacement_candidates(
                pending.source,
                pending.target,
                pending.amount,
                &pending.used,
            )?;
            match candidates.as_slice() {
                [] => {
                    self.commit_damage_event(pending.source, pending.target, pending.amount)?;
                    return Ok(None);
                }
                [replacement] => self.apply_damage_replacement(&mut pending, *replacement)?,
                _ => return Ok(Some(pending)),
            }
        }
    }

    fn commit_damage_event(
        &mut self,
        source: ObjectId,
        target: Target,
        amount: i32,
    ) -> Result<(), RulesError> {
        if amount <= 0 {
            return Ok(());
        }
        match target {
            Target::Player(player) => {
                self.player(player)?;
                self.players[player.0].life -= i64::from(amount);
                self.record_event(GameEvent::DamageDealtToPlayer {
                    source,
                    player,
                    amount,
                });
                self.enqueue_damage_triggers(source, amount)?;
            }
            Target::Permanent(permanent) => {
                self.objects
                    .get_mut(&permanent)
                    .ok_or(RulesError::UnknownCard(permanent))?
                    .damage += amount;
                self.record_event(GameEvent::DamageDealtToPermanent {
                    source,
                    permanent,
                    amount,
                });
                self.enqueue_damage_triggers(source, amount)?;
                self.enqueue_received_damage_triggers(permanent, amount)?;
            }
            Target::Spell(card) => return Err(RulesError::IllegalTarget(Target::Spell(card))),
            Target::SacrificePermanent(card) => {
                return Err(RulesError::IllegalTarget(Target::SacrificePermanent(card)));
            }
        }
        Ok(())
    }

    fn install_damage_prevention_shield(
        &mut self,
        source: ObjectId,
        target: Target,
        amount: i16,
    ) -> Result<(), RulesError> {
        if amount <= 0 || !self.target_matches(target, TargetRequirement::PlayerOrCreature) {
            return Err(RulesError::IllegalTarget(target));
        }
        self.object(source)?;
        let amount = i32::from(amount);
        self.damage_prevention_shields.push(DamagePreventionShield {
            id: self.next_timestamp,
            source,
            target,
            remaining: amount,
            expires_turn: self.turn,
        });
        self.next_timestamp += 1;
        self.record_event(GameEvent::DamageShieldCreated {
            source,
            target,
            amount,
        });
        Ok(())
    }

    fn consume_damage_prevention_shield(&mut self, target: Target, amount: i32) -> i32 {
        let Some(index) = self.damage_prevention_shields.iter().position(|shield| {
            shield.target == target && shield.expires_turn >= self.turn && shield.remaining > 0
        }) else {
            return 0;
        };
        let prevented = amount.min(self.damage_prevention_shields[index].remaining);
        self.damage_prevention_shields[index].remaining -= prevented;
        if self.damage_prevention_shields[index].remaining == 0 {
            self.damage_prevention_shields.remove(index);
        }
        prevented
    }

    fn deal_damage_to_permanent(
        &mut self,
        source: ObjectId,
        permanent: ObjectId,
        amount: i32,
    ) -> Result<(), RulesError> {
        let source_colors = self.characteristics(source)?.colors;
        self.deal_damage_to_permanent_from_colors(source, &source_colors, permanent, amount)
    }

    fn deal_damage_to_permanent_from_colors(
        &mut self,
        source: ObjectId,
        source_colors: &BTreeSet<Color>,
        permanent: ObjectId,
        amount: i32,
    ) -> Result<(), RulesError> {
        // “Can't be prevented” excludes only prevention effects. A Razia-like
        // redirection is a non-prevention replacement, so it remains
        // applicable and may produce a new prospective damage recipient.
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
                        self.deal_damage_to_permanent_from_colors(
                            source,
                            source_colors,
                            target,
                            redirected,
                        )?;
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
                return self.deal_damage_to_permanent_from_colors(
                    source,
                    source_colors,
                    permanent,
                    amount - redirected,
                );
            }
            self.damage_redirections.remove(index);
        }
        let (prevented, consumes_shield) = if self.damage_cannot_be_prevented(source) {
            (0, false)
        } else if self.target_prevents_damage_from_colors(permanent, source_colors) {
            (amount, false)
        } else {
            let targeted =
                self.consume_damage_prevention_shield(Target::Permanent(permanent), amount);
            if targeted > 0 {
                (targeted, false)
            } else {
                let object = self.object(permanent)?;
                (amount.min(object.damage_shield), true)
            }
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
        let prevented = if self.damage_cannot_be_prevented(source) {
            0
        } else {
            self.consume_damage_prevention_shield(Target::Player(player), amount)
        };
        if prevented > 0 {
            self.record_event(GameEvent::DamagePrevented {
                source,
                target: Target::Player(player),
                amount: prevented,
            });
        }
        let remaining = amount - prevented;
        if remaining > 0 {
            self.players[player.0].life -= i64::from(remaining);
            self.record_event(GameEvent::DamageDealtToPlayer {
                source,
                player,
                amount: remaining,
            });
            self.enqueue_damage_triggers(source, remaining)?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_lines)] // Effect dispatch stays centralized so stack resolution has one rules path.
    #[allow(clippy::too_many_arguments)] // The stack snapshot is passed explicitly for replayable resolution provenance.
    fn resolve_effect(
        &mut self,
        source: ObjectId,
        source_incarnation: u64,
        source_colors: &BTreeSet<Color>,
        controller: PlayerId,
        chosen_x: Option<u8>,
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
                    self.deal_damage_to_permanent_from_colors(
                        source,
                        source_colors,
                        permanent,
                        i32::from(*amount),
                    )?;
                }
                Target::Spell(card) => return Err(RulesError::IllegalTarget(Target::Spell(card))),
                Target::SacrificePermanent(card) => {
                    return Err(RulesError::IllegalTarget(Target::SacrificePermanent(card)));
                }
            },
            Effect::GainControlTargetUntilEndOfTurn => {
                let target = Self::target_permanent(target)?;
                self.require_zone(target, Zone::Battlefield)?;
                self.install_continuous_effect(
                    source,
                    target,
                    ContinuousChange::ChangeController(controller),
                    Duration::EndOfTurn(self.turn),
                )?;
            }
            Effect::LoseLifeTarget { amount } => {
                let player =
                    match target.ok_or(RulesError::IllegalAction("missing life-loss target"))? {
                        Target::Player(player) if !self.players[player.0].lost => player,
                        other => return Err(RulesError::IllegalTarget(other)),
                    };
                self.players[player.0].life -= i64::from(*amount);
                self.record_event(GameEvent::LifeLost {
                    source,
                    player,
                    amount: *amount,
                });
            }
            Effect::LoseLifeController { amount } => {
                self.players[controller.0].life -= i64::from(*amount);
                self.record_event(GameEvent::LifeLost {
                    source,
                    player: controller,
                    amount: *amount,
                });
            }
            Effect::LoseLifeEachOpponentEqualToControlledCreatures => {
                let losses = self
                    .players
                    .iter()
                    .filter(|player| player.id != controller && !player.lost)
                    .map(|player| {
                        let amount = i16::try_from(self.controlled_creature_count(player.id))
                            .map_err(|_| {
                                RulesError::IllegalAction(
                                    "opponent creature count exceeds life-loss event capacity",
                                )
                            })?;
                        Ok((player.id, amount))
                    })
                    .collect::<Result<Vec<_>, RulesError>>()?;
                for (player, amount) in losses.into_iter().filter(|(_, amount)| *amount > 0) {
                    self.players[player.0].life -= i64::from(amount);
                    self.record_event(GameEvent::LifeLost {
                        source,
                        player,
                        amount,
                    });
                }
            }
            Effect::DiscardOneCardEachPlayer => {
                // Each player makes this selection independently. Until
                // discard-choice submission is policy-visible, the engine
                // deterministically uses the oldest hand object and keeps the
                // choice auditable through the ordinary discard and zone logs.
                let discards = self
                    .players
                    .iter()
                    .filter(|player| !player.lost)
                    .filter_map(|player| player.hand.first().copied().map(|card| (player.id, card)))
                    .collect::<Vec<_>>();
                for (player, card) in discards {
                    if self.zone_of(card) == Some(Zone::Hand)
                        && self
                            .object(card)
                            .is_ok_and(|object| object.controller == player)
                    {
                        self.record_event(GameEvent::CardDiscarded { player, card });
                        self.move_to_zone(card, Zone::Graveyard)?;
                    }
                }
            }
            Effect::DiscardTargetPlayer { count } => {
                let player = match target
                    .ok_or(RulesError::IllegalAction("missing targeted-discard player"))?
                {
                    Target::Player(player) if !self.players[player.0].lost => player,
                    other => return Err(RulesError::IllegalTarget(other)),
                };
                // The choice is visible through the receipt sequence. A
                // policy-declared card-selection action is still absent, so
                // the deterministic boundary is the target's oldest hand
                // entries at resolution, never a stale cast-time snapshot.
                let cards = self.players[player.0]
                    .hand
                    .iter()
                    .copied()
                    .take(usize::from(*count))
                    .collect::<Vec<_>>();
                for card in cards {
                    if self.zone_of(card) == Some(Zone::Hand)
                        && self
                            .object(card)
                            .is_ok_and(|object| object.controller == player)
                    {
                        self.record_event(GameEvent::CardDiscarded { player, card });
                        self.move_to_zone(card, Zone::Graveyard)?;
                    }
                }
            }
            Effect::MillTargetPlayer { count } => {
                let player =
                    match target.ok_or(RulesError::IllegalAction("missing mill target player"))? {
                        Target::Player(player) if !self.players[player.0].lost => player,
                        other => return Err(RulesError::IllegalTarget(other)),
                    };
                if *count <= 0 {
                    return Err(RulesError::IllegalAction(
                        "mill instruction requires a positive materialized amount",
                    ));
                }
                for _ in 0..usize::try_from(*count).expect("positive i16 fits usize") {
                    let Some(card) = self.players[player.0].library.pop() else {
                        break;
                    };
                    self.move_to_zone(card, Zone::Graveyard)?;
                }
            }
            Effect::MillTargetPlayerFromSourceDamage => {
                return Err(RulesError::IllegalAction(
                    "source-damage mill trigger was not materialized before resolution",
                ));
            }
            Effect::SacrificeControllerCreature => {
                let candidate = self
                    .all_battlefield_cards()
                    .into_iter()
                    .filter(|candidate| *candidate != source)
                    .find(|candidate| {
                        self.controller_of(*candidate)
                            .is_ok_and(|candidate_controller| candidate_controller == controller)
                            && self
                                .characteristics(*candidate)
                                .is_ok_and(|characteristics| {
                                    characteristics.card_types.contains(&CardType::Creature)
                                })
                    })
                    .or_else(|| {
                        (self.zone_of(source) == Some(Zone::Battlefield)
                            && self
                                .controller_of(source)
                                .is_ok_and(|source_controller| source_controller == controller)
                            && self.characteristics(source).is_ok_and(|characteristics| {
                                characteristics.card_types.contains(&CardType::Creature)
                            }))
                        .then_some(source)
                    });
                if let Some(permanent) = candidate {
                    self.record_event(GameEvent::SacrificedByEffect {
                        source,
                        player: controller,
                        permanent,
                    });
                    self.move_to_graveyard_or_remove_token(permanent)?;
                }
            }
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
                            self.deal_damage_to_permanent_from_colors(
                                source,
                                source_colors,
                                permanent,
                                amount,
                            )?;
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
                let Some(selected) = self.select_trigger_targets(source, controller, &[*target])
                else {
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
                        self.deal_damage_to_permanent_from_colors(
                            source,
                            source_colors,
                            permanent,
                            i32::from(*amount),
                        )?;
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
            Effect::AddPlusOneCounterToSource => {
                if self.zone_of(source) == Some(Zone::Battlefield)
                    && self.object_has_incarnation(source, source_incarnation)
                {
                    self.place_counter(source, source, CounterKind::PlusOnePlusOne, 1)?;
                }
            }
            Effect::AddPlusOneCounterToTarget => {
                let target = Self::target_permanent(target)?;
                if !self.target_matches(Target::Permanent(target), TargetRequirement::Creature) {
                    return Err(RulesError::IllegalTarget(Target::Permanent(target)));
                }
                self.place_counter(source, target, CounterKind::PlusOnePlusOne, 1)?;
            }
            Effect::AddCountersToSource { counter, amount } => {
                if self.zone_of(source) == Some(Zone::Battlefield)
                    && self.object_has_incarnation(source, source_incarnation)
                {
                    self.place_counter(source, source, *counter, *amount)?;
                }
            }
            Effect::AddCountersToTarget { counter, amount } => {
                let target = Self::target_permanent(target)?;
                if !self.target_matches(Target::Permanent(target), TargetRequirement::Permanent) {
                    return Err(RulesError::IllegalTarget(Target::Permanent(target)));
                }
                self.place_counter(source, target, *counter, *amount)?;
            }
            Effect::RemoveCountersFromSource { counter, amount } => {
                if self.zone_of(source) == Some(Zone::Battlefield)
                    && self.object_has_incarnation(source, source_incarnation)
                {
                    self.remove_counter(source, source, *counter, *amount)?;
                }
            }
            Effect::RemoveCountersFromTarget { counter, amount } => {
                let target = Self::target_permanent(target)?;
                if !self.target_matches(Target::Permanent(target), TargetRequirement::Permanent) {
                    return Err(RulesError::IllegalTarget(Target::Permanent(target)));
                }
                self.remove_counter(source, target, *counter, *amount)?;
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
                    self.deal_damage_to_permanent_from_colors(
                        source,
                        source_colors,
                        creature,
                        i32::from(*amount),
                    )?;
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
                    self.deal_damage_to_permanent_from_colors(
                        source,
                        source_colors,
                        creature,
                        i32::from(*amount),
                    )?;
                }
            }
            Effect::RadianceDealDamageToCreatures { amount } => {
                let target = Self::target_permanent(target)?;
                // Select once before mutating damage. State-based actions run
                // after the complete spell resolves, so every selected
                // creature receives this effect's damage in the same batch.
                for candidate in self.radiance_creatures_sharing_color(target)? {
                    self.deal_damage_to_permanent_from_colors(
                        source,
                        source_colors,
                        candidate,
                        i32::from(*amount),
                    )?;
                }
            }
            Effect::GainLifeController { amount } => {
                self.players[controller.0].life += i64::from(*amount);
                self.record_event(GameEvent::LifeGained {
                    player: controller,
                    amount: *amount,
                });
                self.enqueue_life_gain_triggers(controller);
            }
            Effect::GainLifeForEachCreature => {
                let count = self
                    .all_battlefield_cards()
                    .into_iter()
                    .filter(|card| {
                        self.characteristics(*card).is_ok_and(|characteristics| {
                            characteristics.card_types.contains(&CardType::Creature)
                        })
                    })
                    .count();
                let amount = i16::try_from(count).map_err(|_| {
                    RulesError::IllegalAction(
                        "battlefield creature count exceeds life-gain event capacity",
                    )
                })?;
                if amount > 0 {
                    self.players[controller.0].life += i64::from(amount);
                    self.record_event(GameEvent::LifeGained {
                        player: controller,
                        amount,
                    });
                    self.enqueue_life_gain_triggers(controller);
                }
            }
            Effect::GainLifeControllerFromSourceDamage => {
                return Err(RulesError::IllegalAction(
                    "unmaterialized source-damage life-gain trigger",
                ));
            }
            Effect::DrawController => {
                self.draw_card_from_spell_effect(controller)?;
            }
            Effect::DrawTargetPlayer => {
                let player = match target.ok_or(RulesError::IllegalAction(
                    "missing target-player draw target",
                ))? {
                    Target::Player(player) if !self.players[player.0].lost => player,
                    other => return Err(RulesError::IllegalTarget(other)),
                };
                self.draw_card_from_spell_effect(player)?;
            }
            Effect::PreventLibrarySearchUntilEndOfTurn => {
                self.library_search_prevented_until = Some(self.turn);
                self.record_event(GameEvent::LibrarySearchesPrevented {
                    source,
                    until_turn: self.turn,
                });
            }
            Effect::SearchControllerLibrary {
                requirement,
                destination,
                selection,
            } => {
                self.resolve_controller_library_search(
                    source,
                    controller,
                    requirement,
                    *destination,
                    *selection,
                    chosen_x,
                )?;
            }
            Effect::AttachSourceAndModifyTargetPt { .. } | Effect::AttachSourceToTarget { .. } => {
                return Err(RulesError::IllegalAction(
                    "aura attachment bypassed permanent-spell resolution",
                ));
            }
            Effect::RevealTopCardPutIntoHandLoseLifeEqualToManaValue => {
                let Some(card) = self.players[controller.0].library.last().copied() else {
                    return Ok(());
                };
                let definition = self.card_definition(card)?.id;
                let mana_value = self.card_definition(card)?.mana_cost.mana_value();
                self.record_event(GameEvent::CardRevealed {
                    player: controller,
                    card,
                    definition,
                });
                self.move_to_zone(card, Zone::Hand)?;
                if mana_value > 0 {
                    let amount = i16::from(mana_value);
                    self.players[controller.0].life -= i64::from(amount);
                    self.record_event(GameEvent::LifeLost {
                        source,
                        player: controller,
                        amount,
                    });
                }
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
            Effect::ModifyAllCreaturesPtUntilEndOfTurnIfManaColorSpent {
                color,
                power,
                toughness,
            } => {
                if mana_spent.is_some_and(|spent| spent.contains(color)) {
                    // Snapshot every current creature after earlier effects
                    // (including land destruction) have resolved, then
                    // install the complete batch before post-resolution SBAs.
                    let creatures = self
                        .all_battlefield_cards()
                        .into_iter()
                        .filter(|candidate| {
                            self.characteristics(*candidate)
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
            }
            Effect::CreateToken { token, count } => {
                self.create_tokens(controller, token, *count)?;
            }
            Effect::CreateTokenForTargetPlayer { token, count } => {
                let target_player = match target {
                    Some(Target::Player(player)) if !self.players[player.0].lost => player,
                    Some(other) => return Err(RulesError::IllegalTarget(other)),
                    None => return Err(RulesError::IllegalAction("missing token-player target")),
                };
                self.create_tokens(target_player, token, *count)?;
            }
            Effect::BeginDamageRedirection { amount } => {
                let protected = Self::target_permanent(target)?;
                if *amount <= 0
                    || self.zone_of(source) != Some(Zone::Battlefield)
                    || !self.object_has_incarnation(source, source_incarnation)
                    || self.zone_of(protected) != Some(Zone::Battlefield)
                    || !self
                        .controller_of(protected)
                        .is_ok_and(|target_controller| target_controller == controller)
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
                    id: self.next_timestamp,
                    source: pending.source,
                    protected: pending.protected,
                    destination,
                    remaining: pending.remaining,
                    expires_turn: self.turn,
                });
                self.next_timestamp += 1;
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
            Effect::ModifyTargetPtAndKeywordUntilEndOfTurn {
                power,
                toughness,
                keyword,
            } => {
                let target = Self::target_permanent(target)?;
                if !self
                    .characteristics(target)?
                    .card_types
                    .contains(&CardType::Creature)
                {
                    return Err(RulesError::IllegalTarget(Target::Permanent(target)));
                }
                self.install_continuous_effect(
                    source,
                    target,
                    ContinuousChange::ModifyPowerToughness {
                        power: *power,
                        toughness: *toughness,
                    },
                    Duration::EndOfTurn(self.turn),
                )?;
                self.install_continuous_effect(
                    source,
                    target,
                    ContinuousChange::AddKeyword(keyword.clone()),
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
                    || !self.object_has_incarnation(source, source_incarnation)
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
                if self.zone_of(source) != Some(Zone::Battlefield)
                    || !self.object_has_incarnation(source, source_incarnation)
                {
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
                if self.zone_of(source) != Some(Zone::Battlefield)
                    || !self.object_has_incarnation(source, source_incarnation)
                {
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
                if *amount <= 0
                    || self.zone_of(source) != Some(Zone::Battlefield)
                    || !self.object_has_incarnation(source, source_incarnation)
                {
                    return Ok(());
                }
                self.install_continuous_effect(
                    source,
                    source,
                    ContinuousChange::AddDamageShield(*amount),
                    Duration::EndOfTurn(self.turn),
                )?;
            }
            Effect::AddTargetDamageShieldUntilEndOfTurn { amount } => {
                let target =
                    target.ok_or(RulesError::IllegalAction("missing damage-shield target"))?;
                self.install_damage_prevention_shield(source, target, *amount)?;
            }
            Effect::RegenerateTargetCreature => {
                let target = Self::target_permanent(target)?;
                if !self.target_matches(Target::Permanent(target), TargetRequirement::Creature) {
                    return Err(RulesError::IllegalTarget(Target::Permanent(target)));
                }
                self.regeneration_shields
                    .entry(target)
                    .or_default()
                    .push(source);
                self.record_event(GameEvent::RegenerationShieldCreated { source, target });
            }
            Effect::RegenerateSource => {
                if self.zone_of(source) == Some(Zone::Battlefield)
                    && self.object_has_incarnation(source, source_incarnation)
                    && self.characteristics(source).is_ok_and(|characteristics| {
                        characteristics.card_types.contains(&CardType::Creature)
                    })
                {
                    self.regeneration_shields
                        .entry(source)
                        .or_default()
                        .push(source);
                    self.record_event(GameEvent::RegenerationShieldCreated {
                        source,
                        target: source,
                    });
                }
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
                self.destroy_permanent(source, land)?;
            }
            Effect::DestroyTargetLandAndUntapSourceIfNonbasic => {
                let target = target.ok_or(RulesError::IllegalAction("missing land target"))?;
                let Target::Permanent(land) = target else {
                    return Err(RulesError::IllegalTarget(target));
                };
                if self.zone_of(land) != Some(Zone::Battlefield)
                    || !self.card_definition(land)?.is_land()
                {
                    return Err(RulesError::IllegalTarget(target));
                }
                let target_was_nonbasic = !self.card_definition(land)?.is_basic_land;
                self.destroy_permanent(source, land)?;
                if target_was_nonbasic
                    && self.zone_of(source) == Some(Zone::Battlefield)
                    && self.object_has_incarnation(source, source_incarnation)
                    && self.object(source)?.tapped
                {
                    self.objects
                        .get_mut(&source)
                        .ok_or(RulesError::UnknownCard(source))?
                        .tapped = false;
                    self.record_event(GameEvent::PermanentUntapped {
                        source,
                        card: source,
                    });
                }
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
                self.destroy_permanent(source, artifact)?;
            }
            Effect::DestroyTargetFlyingCreature => {
                let target = Self::target_permanent(target)?;
                if !self
                    .target_matches(Target::Permanent(target), TargetRequirement::FlyingCreature)
                {
                    return Err(RulesError::IllegalTarget(Target::Permanent(target)));
                }
                self.destroy_permanent(source, target)?;
            }
            Effect::DestroyTargetNonblackCreature => {
                let target = Self::target_permanent(target)?;
                if !self.target_matches(
                    Target::Permanent(target),
                    TargetRequirement::NonblackCreature,
                ) {
                    return Err(RulesError::IllegalTarget(Target::Permanent(target)));
                }
                self.destroy_permanent(source, target)?;
            }
            Effect::DestroyTargetArtifactOrCreatureNoRegeneration => {
                let target = Self::target_permanent(target)?;
                if !self.target_matches(
                    Target::Permanent(target),
                    TargetRequirement::ArtifactOrCreature,
                ) {
                    return Err(RulesError::IllegalTarget(Target::Permanent(target)));
                }
                self.destroy_permanent_without_regeneration(source, target)?;
            }
            Effect::DestroyDistinctTargetCreature => {
                let target = Self::target_permanent(target)?;
                if !self.target_matches(
                    Target::Permanent(target),
                    TargetRequirement::DistinctCreature,
                ) {
                    return Err(RulesError::IllegalTarget(Target::Permanent(target)));
                }
                self.destroy_permanent(source, target)?;
            }
            Effect::DestroyTargetCreatureWithManaValueAtMostChosenX => {
                let target = Self::target_permanent(target)?;
                if !self.target_matches(Target::Permanent(target), TargetRequirement::Creature) {
                    return Err(RulesError::IllegalTarget(Target::Permanent(target)));
                }
                let x_value = usize::from(chosen_x.ok_or(RulesError::IllegalAction(
                    "chosen-X resolution lacks the selected X value",
                ))?);
                let mana_value = if self.object(target)?.token.is_some() {
                    0
                } else {
                    usize::from(self.card_definition(target)?.mana_cost.mana_value())
                };
                if mana_value <= x_value {
                    self.destroy_permanent(source, target)?;
                }
            }
            Effect::TapTargetCreature => {
                let target = Self::target_permanent(target)?;
                if !self.target_matches(Target::Permanent(target), TargetRequirement::Creature) {
                    return Err(RulesError::IllegalTarget(Target::Permanent(target)));
                }
                let object = self
                    .objects
                    .get_mut(&target)
                    .ok_or(RulesError::UnknownCard(target))?;
                if !object.tapped {
                    object.tapped = true;
                    self.record_event(GameEvent::PermanentTapped {
                        source,
                        card: target,
                    });
                }
            }
            Effect::UntapSource => {
                if self.zone_of(source) == Some(Zone::Battlefield)
                    && self.object_has_incarnation(source, source_incarnation)
                    && self.object(source)?.tapped
                {
                    self.objects
                        .get_mut(&source)
                        .ok_or(RulesError::UnknownCard(source))?
                        .tapped = false;
                    self.record_event(GameEvent::PermanentUntapped {
                        source,
                        card: source,
                    });
                }
            }
            Effect::UntapTargetLand => {
                let target = Self::target_permanent(target)?;
                if !self.target_matches(Target::Permanent(target), TargetRequirement::Land) {
                    return Err(RulesError::IllegalTarget(Target::Permanent(target)));
                }
                let object = self
                    .objects
                    .get_mut(&target)
                    .ok_or(RulesError::UnknownCard(target))?;
                if object.tapped {
                    object.tapped = false;
                    self.record_event(GameEvent::PermanentUntapped {
                        source,
                        card: target,
                    });
                }
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
                        self.controller_of(*candidate)
                            .is_ok_and(|candidate_controller| candidate_controller == controller)
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
                        self.controller_of(*candidate)
                            .is_ok_and(|candidate_controller| {
                                candidate_controller == controller
                                    && self.characteristics(*candidate).is_ok_and(
                                        |characteristics| {
                                            characteristics.card_types.contains(&CardType::Creature)
                                        },
                                    )
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
            Effect::RadianceDestroyEnchantments => {
                let target = Self::target_permanent(target)?;
                if !self.target_matches(Target::Permanent(target), TargetRequirement::Enchantment) {
                    return Err(RulesError::IllegalTarget(Target::Permanent(target)));
                }
                // Select the full radiance set before moving any permanent,
                // so every qualifying enchantment receives the same resolving
                // instruction even when an earlier destruction changes a
                // source's zone.
                for candidate in
                    self.radiance_permanents_sharing_color_of_type(target, &CardType::Enchantment)?
                {
                    self.destroy_permanent(source, candidate)?;
                }
            }
            Effect::DestroyAllNonTokenCreatures => {
                // Snapshot every live non-token creature before destruction.
                // In particular, a zone change caused by an earlier destroy
                // instruction must not change this spell's recipient set.
                let creatures = self
                    .all_battlefield_cards()
                    .into_iter()
                    .filter(|candidate| {
                        self.object(*candidate)
                            .is_ok_and(|object| object.token.is_none())
                            && self
                                .characteristics(*candidate)
                                .is_ok_and(|characteristics| {
                                    characteristics.card_types.contains(&CardType::Creature)
                                })
                    })
                    .collect::<Vec<_>>();
                for creature in creatures {
                    self.destroy_permanent(source, creature)?;
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
                self.move_to_spell_terminal_zone(target)?;
            }
            Effect::SacrificeCreatureOrCounterTargetSpell => {
                let target = Self::target_spell(target)?;
                let candidate = self.all_battlefield_cards().into_iter().find(|candidate| {
                    self.controller_of(*candidate)
                        .is_ok_and(|candidate_controller| candidate_controller == controller)
                        && self
                            .characteristics(*candidate)
                            .is_ok_and(|characteristics| {
                                characteristics.card_types.contains(&CardType::Creature)
                            })
                });
                if let Some(permanent) = candidate {
                    self.record_event(GameEvent::SacrificedByEffect {
                        source,
                        player: controller,
                        permanent,
                    });
                    self.move_to_graveyard_or_remove_token(permanent)?;
                } else {
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
                    self.move_to_spell_terminal_zone(target)?;
                }
            }
            Effect::GrantGraveyardCastPermissionUntilEndOfTurn => {
                let target = Self::target_permanent(target)?;
                if !self.target_matches_for_controller(
                    controller,
                    Target::Permanent(target),
                    TargetRequirement::InstantOrSorceryCardInControllerGraveyard,
                ) {
                    return Err(RulesError::IllegalTarget(Target::Permanent(target)));
                }
                self.graveyard_cast_permissions.insert(
                    target,
                    GraveyardCastPermission {
                        player: controller,
                        expires_turn: self.turn,
                    },
                );
                self.record_event(GameEvent::GraveyardCastPermissionGranted {
                    source,
                    player: controller,
                    card: target,
                    until_turn: self.turn,
                });
            }
            Effect::ExileTargetCreature | Effect::ExileTargetPermanent => {
                let target = Self::target_permanent(target)?;
                self.move_to_zone(target, Zone::Exile)?;
            }
            Effect::ExileAttachedCreatureAndAurasUntilEndStep => {
                self.exile_attached_creature_and_auras_until_end_step(
                    source,
                    source_incarnation,
                    controller,
                )?;
            }
            Effect::DestroyTargetArtifactOrEnchantment => {
                let target = Self::target_permanent(target)?;
                if !self.target_matches(
                    Target::Permanent(target),
                    TargetRequirement::ArtifactOrEnchantment,
                ) {
                    return Err(RulesError::IllegalTarget(Target::Permanent(target)));
                }
                self.destroy_permanent(source, target)?;
            }
            Effect::ReturnTargetPermanentToHandAndLoseControllerLife { amount } => {
                let target = Self::target_permanent(target)?;
                if !self.target_matches(Target::Permanent(target), TargetRequirement::Permanent) {
                    return Err(RulesError::IllegalTarget(Target::Permanent(target)));
                }
                let target_controller = self.controller_of(target)?;
                self.move_to_zone(target, Zone::Hand)?;
                self.players[target_controller.0].life -= i64::from(*amount);
                self.record_event(GameEvent::LifeLost {
                    source,
                    player: target_controller,
                    amount: *amount,
                });
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
            Effect::ReturnTargetEnchantmentCardToHand => {
                let target = Self::target_permanent(target)?;
                if !self.target_matches_for_controller(
                    controller,
                    Target::Permanent(target),
                    TargetRequirement::EnchantmentCardInControllerGraveyard,
                ) {
                    return Err(RulesError::IllegalTarget(Target::Permanent(target)));
                }
                self.move_to_zone(target, Zone::Hand)?;
            }
            Effect::ReturnTargetCreatureCardToHandIfAnotherInControllerGraveyard => {
                let target = Self::target_permanent(target)?;
                if !self.target_matches_for_controller(
                    controller,
                    Target::Permanent(target),
                    TargetRequirement::CreatureCardInControllerGraveyard,
                ) {
                    return Err(RulesError::IllegalTarget(Target::Permanent(target)));
                }
                if self.controller_creature_cards_in_graveyard(controller) >= 2 {
                    self.move_to_zone(target, Zone::Hand)?;
                }
            }
            Effect::ReturnTargetCreatureCardToBattlefieldWithCounterIfManaColorSpent { color } => {
                let target = Self::target_permanent(target)?;
                if !self.target_matches_for_controller(
                    controller,
                    Target::Permanent(target),
                    TargetRequirement::CreatureCardInControllerGraveyard,
                ) {
                    return Err(RulesError::IllegalTarget(Target::Permanent(target)));
                }
                self.move_to_zone(target, Zone::Battlefield)?;
                if mana_spent.is_some_and(|spent| spent.contains(color)) {
                    self.place_counter(source, target, CounterKind::PlusOnePlusOne, 1)?;
                }
            }
            Effect::ReturnOneCreatureCardFromEachGraveyardToHand => {
                // The required choices happen before any card moves. This
                // matters when a future policy interface replaces the
                // deterministic first-card selection with explicit choices:
                // no earlier player's move can change what another player was
                // allowed to choose.
                let returns = (0..self.players.len())
                    .filter_map(|index| {
                        let player = PlayerId(index);
                        (!self.players[index].lost)
                            .then(|| {
                                self.players[index].graveyard.iter().copied().find(|card| {
                                    self.card_definition(*card).is_ok_and(|definition| {
                                        definition.card_types.contains(&CardType::Creature)
                                    })
                                })
                            })
                            .flatten()
                            .map(|card| (player, card))
                    })
                    .collect::<Vec<_>>();
                for (player, card) in returns {
                    debug_assert_eq!(self.object(card)?.owner, player);
                    if self.zone_of(card) == Some(Zone::Graveyard) {
                        self.move_to_zone(card, Zone::Hand)?;
                    }
                }
            }
            Effect::ReturnUpToThreeControllerGraveyardLandCardsToHand => {
                // Snapshot the full selection before any move. The public
                // compatibility slice uses the deterministic first-three
                // selection from a public zone until policy-submitted choices
                // are available.
                let returns = self.players[controller.0]
                    .graveyard
                    .iter()
                    .copied()
                    .filter(|card| {
                        self.card_definition(*card)
                            .is_ok_and(CardDefinition::is_land)
                    })
                    .take(3)
                    .collect::<Vec<_>>();
                for card in returns {
                    if self.zone_of(card) == Some(Zone::Graveyard)
                        && self.object(card)?.owner == controller
                    {
                        self.move_to_zone(card, Zone::Hand)?;
                    }
                }
            }
            Effect::LookAtTopCardsChooseForLifeOrGraveyard { .. } => {
                return Err(RulesError::IllegalAction(
                    "private-library choice effect bypassed its resolution boundary",
                ));
            }
            Effect::LookAtTopCardsOfTargetOpponentExileOne { .. } => {
                return Err(RulesError::IllegalAction(
                    "private opponent-library choice effect bypassed its resolution boundary",
                ));
            }
            Effect::ShuffleGraveyardsIntoLibraries => {
                for player_index in 0..self.players.len() {
                    let player = PlayerId(player_index);
                    let cards = self.players[player_index].graveyard.clone();
                    for card in cards {
                        self.move_to_zone(card, Zone::Library)?;
                    }
                    let count =
                        u16::try_from(self.players[player_index].library.len()).unwrap_or(u16::MAX);
                    self.shuffle_library(player);
                    self.record_event(GameEvent::LibraryShuffled {
                        player,
                        cards: count,
                    });
                }
            }
            Effect::ReturnControlledCreatureToHand => {
                let target = Self::target_permanent(target)?;
                if !self.target_matches_for_controller(
                    controller,
                    Target::Permanent(target),
                    TargetRequirement::ControlledCreature,
                ) {
                    return Err(RulesError::IllegalTarget(Target::Permanent(target)));
                }
                self.move_to_zone(target, Zone::Hand)?;
            }
            Effect::ReturnControlledLandToHand => {
                let target = Self::target_permanent(target)?;
                if !self.target_matches_for_controller(
                    controller,
                    Target::Permanent(target),
                    TargetRequirement::ControlledLand,
                ) {
                    return Err(RulesError::IllegalTarget(Target::Permanent(target)));
                }
                self.move_to_zone(target, Zone::Hand)?;
            }
            Effect::ReturnOpponentCreatureToHand => {
                let target = Self::target_permanent(target)?;
                if !self.target_matches_for_controller(
                    controller,
                    Target::Permanent(target),
                    TargetRequirement::OpponentCreature,
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
                        .controller_of(**attacker)
                        .is_ok_and(|attacker_controller| attacker_controller == controller)
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

    /// Captures the object incarnation for each target occurrence at the
    /// moment a spell or ability is placed on the stack. Player targets have
    /// no object incarnation and retain `None` in the ordered receipt.
    fn target_incarnations(&self, targets: &[Target]) -> Vec<Option<u64>> {
        targets
            .iter()
            .map(|target| match target {
                Target::Permanent(card)
                | Target::Spell(card)
                | Target::SacrificePermanent(card) => {
                    self.object(*card).ok().map(|object| object.incarnation)
                }
                Target::Player(_) => None,
            })
            .collect()
    }

    /// Returns whether a target occurrence still names the same rules object
    /// incarnation captured when its stack object was created. Publicly
    /// fabricated stack objects with no receipt remain on the dynamic-legality
    /// path, while engine-created stack objects always carry one.
    fn stack_target_incarnation_matches(
        &self,
        stack_object: &StackObject,
        target_index: usize,
        target: Target,
    ) -> bool {
        let Some(expected) = stack_object
            .target_incarnations
            .get(target_index)
            .copied()
            .flatten()
        else {
            return true;
        };
        match target {
            Target::Permanent(card) | Target::Spell(card) | Target::SacrificePermanent(card) => {
                self.object(card)
                    .is_ok_and(|object| object.incarnation == expected)
            }
            Target::Player(_) => true,
        }
    }

    #[allow(clippy::too_many_lines)] // The typed target-kind matrix is intentionally exhaustive.
    fn target_matches(&self, target: Target, requirement: TargetRequirement) -> bool {
        match (target, requirement) {
            (
                Target::Player(player),
                TargetRequirement::Any
                | TargetRequirement::Player
                | TargetRequirement::Opponent
                | TargetRequirement::PlayerOrCreature,
            ) => self.players.get(player.0).is_some_and(|state| !state.lost),
            (Target::Permanent(card), TargetRequirement::Any | TargetRequirement::Permanent) => {
                self.zone_of(card) == Some(Zone::Battlefield)
            }
            (
                Target::Permanent(card),
                TargetRequirement::Creature
                | TargetRequirement::NonblackCreature
                | TargetRequirement::FlyingCreature
                | TargetRequirement::DistinctCreature
                | TargetRequirement::PlayerOrCreature
                | TargetRequirement::BlockingCreature
                | TargetRequirement::AttackingOrBlockingCreature
                | TargetRequirement::ControlledCreature
                | TargetRequirement::OpponentCreature,
            ) => self.creature_target_matches(card, requirement),
            (
                Target::Permanent(card),
                TargetRequirement::Land | TargetRequirement::ControlledLand,
            ) => {
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
            (Target::Permanent(card), TargetRequirement::ArtifactOrCreature) => {
                self.zone_of(card) == Some(Zone::Battlefield)
                    && self.characteristics(card).is_ok_and(|characteristics| {
                        characteristics.card_types.contains(&CardType::Artifact)
                            || characteristics.card_types.contains(&CardType::Creature)
                    })
            }
            (Target::Permanent(card), TargetRequirement::ArtifactOrEnchantment) => {
                self.zone_of(card) == Some(Zone::Battlefield)
                    && self.characteristics(card).is_ok_and(|characteristics| {
                        characteristics.card_types.contains(&CardType::Artifact)
                            || characteristics.card_types.contains(&CardType::Enchantment)
                    })
            }
            (Target::Permanent(card), TargetRequirement::Enchantment) => {
                self.zone_of(card) == Some(Zone::Battlefield)
                    && self.characteristics(card).is_ok_and(|characteristics| {
                        characteristics.card_types.contains(&CardType::Enchantment)
                    })
            }
            (Target::Permanent(card), TargetRequirement::OwnGraveyardCard) => {
                self.zone_of(card) == Some(Zone::Graveyard)
            }
            (Target::Permanent(card), TargetRequirement::CreatureCardInControllerGraveyard) => {
                self.zone_of(card) == Some(Zone::Graveyard)
                    && self
                        .card_definition(card)
                        .is_ok_and(|definition| definition.card_types.contains(&CardType::Creature))
            }
            (Target::Permanent(card), TargetRequirement::EnchantmentCardInControllerGraveyard) => {
                self.zone_of(card) == Some(Zone::Graveyard)
                    && self.card_definition(card).is_ok_and(|definition| {
                        definition.card_types.contains(&CardType::Enchantment)
                    })
            }
            (
                Target::Permanent(card),
                TargetRequirement::InstantOrSorceryCardInControllerGraveyard,
            ) => {
                self.zone_of(card) == Some(Zone::Graveyard)
                    && self.card_definition(card).is_ok_and(|definition| {
                        definition.card_types.contains(&CardType::Instant)
                            || definition.card_types.contains(&CardType::Sorcery)
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
            (Target::Spell(card), TargetRequirement::NoncreatureSpell) => {
                self.stack
                    .iter()
                    .any(|stack_object| stack_object.card == card)
                    && self.card_definition(card).is_ok_and(|definition| {
                        !definition.card_types.contains(&CardType::Creature)
                    })
            }
            _ => false,
        }
    }

    fn is_nonblack_creature(&self, card: ObjectId) -> bool {
        self.characteristics(card)
            .is_ok_and(|characteristics| !characteristics.colors.contains(&Color::Black))
    }

    fn creature_target_matches(&self, card: ObjectId, requirement: TargetRequirement) -> bool {
        self.zone_of(card) == Some(Zone::Battlefield)
            && self.characteristics(card).is_ok_and(|characteristics| {
                characteristics.card_types.contains(&CardType::Creature)
            })
            && (!matches!(requirement, TargetRequirement::BlockingCreature)
                || self.combat.as_ref().is_some_and(|combat| {
                    combat
                        .blockers
                        .values()
                        .flatten()
                        .any(|blocker| *blocker == card)
                }))
            && (!matches!(requirement, TargetRequirement::AttackingOrBlockingCreature)
                || self.combat.as_ref().is_some_and(|combat| {
                    combat.attackers.contains(&card)
                        || combat
                            .blockers
                            .values()
                            .flatten()
                            .any(|blocker| *blocker == card)
                }))
            && (!matches!(requirement, TargetRequirement::FlyingCreature)
                || self.characteristics(card).is_ok_and(|characteristics| {
                    characteristics.keywords.contains(&Keyword::Flying)
                }))
            && (!matches!(requirement, TargetRequirement::NonblackCreature)
                || self.is_nonblack_creature(card))
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
                (Target::Permanent(card), TargetRequirement::CreatureCardInControllerGraveyard) => {
                    self.object(card)
                        .is_ok_and(|object| object.owner == controller)
                }
                (
                    Target::Permanent(card),
                    TargetRequirement::EnchantmentCardInControllerGraveyard,
                ) => self
                    .object(card)
                    .is_ok_and(|object| object.owner == controller),
                (
                    Target::Permanent(card),
                    TargetRequirement::InstantOrSorceryCardInControllerGraveyard,
                ) => self
                    .object(card)
                    .is_ok_and(|object| object.owner == controller),
                (Target::Permanent(card), TargetRequirement::ControlledCreature) => self
                    .controller_of(card)
                    .is_ok_and(|target_controller| target_controller == controller),
                (Target::Permanent(card), TargetRequirement::ControlledLand) => self
                    .controller_of(card)
                    .is_ok_and(|target_controller| target_controller == controller),
                (Target::Permanent(card), TargetRequirement::OpponentCreature) => self
                    .controller_of(card)
                    .is_ok_and(|target_controller| target_controller != controller),
                (Target::Player(player), TargetRequirement::Opponent) => player != controller,
                _ => true,
            }
    }

    /// Extends controller-scoped target legality with protection from the
    /// source's current colors. A player target is unaffected; protection is
    /// a permanent-facing restriction in this engine slice.
    fn target_matches_for_source(
        &self,
        controller: PlayerId,
        source: ObjectId,
        target: Target,
        requirement: TargetRequirement,
    ) -> bool {
        let Ok(source_characteristics) = self.characteristics(source) else {
            return false;
        };
        self.target_matches_for_colors(
            controller,
            target,
            requirement,
            &source_characteristics.colors,
        )
    }

    fn target_matches_for_colors(
        &self,
        controller: PlayerId,
        target: Target,
        requirement: TargetRequirement,
        source_colors: &BTreeSet<Color>,
    ) -> bool {
        self.target_matches_for_controller(controller, target, requirement)
            && !self.permanent_has_protection_from_colors_for_target(target, source_colors)
    }

    fn permanent_has_protection_from_colors_for_target(
        &self,
        target: Target,
        source_colors: &BTreeSet<Color>,
    ) -> bool {
        match target {
            Target::Permanent(card) => {
                self.permanent_has_protection_from_colors(card, source_colors)
            }
            Target::Player(_) | Target::Spell(_) | Target::SacrificePermanent(_) => false,
        }
    }

    fn permanent_has_protection_from_colors(
        &self,
        permanent: ObjectId,
        source_colors: &BTreeSet<Color>,
    ) -> bool {
        self.characteristics(permanent)
            .is_ok_and(|characteristics| {
                characteristics.keywords.iter().any(|keyword| {
                    matches!(
                        keyword,
                        Keyword::Protection(color) if source_colors.contains(color)
                    )
                })
            })
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
                    | TargetRequirement::Opponent
                    | TargetRequirement::PlayerOrCreature
            ) | (
                Target::Permanent(_),
                TargetRequirement::Any
                    | TargetRequirement::Permanent
                    | TargetRequirement::Creature
                    | TargetRequirement::NonblackCreature
                    | TargetRequirement::FlyingCreature
                    | TargetRequirement::DistinctCreature
                    | TargetRequirement::BlockingCreature
                    | TargetRequirement::AttackingOrBlockingCreature
                    | TargetRequirement::Land
                    | TargetRequirement::ControlledLand
                    | TargetRequirement::Artifact
                    | TargetRequirement::Enchantment
                    | TargetRequirement::ArtifactOrCreature
                    | TargetRequirement::ArtifactOrEnchantment
                    | TargetRequirement::OwnGraveyardCard
                    | TargetRequirement::CreatureCardInControllerGraveyard
                    | TargetRequirement::EnchantmentCardInControllerGraveyard
                    | TargetRequirement::InstantOrSorceryCardInControllerGraveyard
                    | TargetRequirement::ControlledCreature
                    | TargetRequirement::OpponentCreature
                    | TargetRequirement::PlayerOrCreature
            ) | (
                Target::Spell(_),
                TargetRequirement::InstantOrSorcerySpell | TargetRequirement::NoncreatureSpell
            )
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
            self.library_search_prevented_until = None;
        }
        self.priority = self.priority_after_resolution();
        self.start_step()
    }

    #[allow(clippy::too_many_lines)] // Turn-boundary cleanup keeps all expiration receipts ordered.
    fn start_step(&mut self) -> Result<(), RulesError> {
        // The boundary marker must precede *all* turn-based work in the step
        // so an event log can be replayed in chronological order.
        self.record_event(GameEvent::StepBegan {
            turn: self.turn,
            active_player: self.active_player,
            step: self.step,
        });
        if self.step == Step::Upkeep {
            self.enqueue_upkeep_triggers()?;
        }
        if self.step == Step::FirstStrikeCombatDamage {
            self.resolve_combat_damage(true)?;
        }
        if self.step == Step::CombatDamage {
            self.resolve_combat_damage(false)?;
        }
        if self.step == Step::End {
            self.consume_due_delayed_actions()?;
        }
        match self.step {
            Step::Untap => {
                self.players[self.active_player.0].lands_played = 0;
                let battlefield = self
                    .all_battlefield_cards()
                    .into_iter()
                    .filter(|card| self.controller_of(*card) == Ok(self.active_player))
                    .collect::<Vec<_>>();
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
                let expired_permissions = self
                    .graveyard_cast_permissions
                    .iter()
                    .filter_map(|(card, permission)| {
                        (permission.expires_turn == self.turn).then_some(*card)
                    })
                    .collect::<Vec<_>>();
                for card in expired_permissions {
                    self.graveyard_cast_permissions.remove(&card);
                    self.record_event(GameEvent::GraveyardCastPermissionExpired { card });
                }
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
                let control_before = self.control_targets_before_expiration(&expired)?;
                self.continuous_effects
                    .retain(|effect| effect.duration != Duration::EndOfTurn(self.turn));
                self.damage_redirections
                    .retain(|redirect| redirect.expires_turn != self.turn);
                let expired_shields = self
                    .damage_prevention_shields
                    .iter()
                    .filter(|shield| shield.expires_turn == self.turn)
                    .cloned()
                    .collect::<Vec<_>>();
                self.damage_prevention_shields
                    .retain(|shield| shield.expires_turn != self.turn);
                for effect in expired {
                    self.remove_damage_shield_for_effect(&effect);
                    self.record_event(GameEvent::ContinuousEffectExpired {
                        source: effect.source,
                        target: effect.target,
                        layer: effect.change.layer(),
                    });
                }
                self.record_control_reversions(&control_before)?;
                for shield in expired_shields {
                    self.record_event(GameEvent::DamageShieldExpired {
                        source: shield.source,
                        target: shield.target,
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
            .chain(combat.blockers.values().flatten().copied())
        {
            if !combat.removed_from_combat.contains(&creature)
                && self.zone_of(creature) == Some(Zone::Battlefield)
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
                .chain(combat.blockers.values().flatten().copied())
                .filter(|creature| {
                    !combat.removed_from_combat.contains(creature)
                        && self.zone_of(*creature) == Some(Zone::Battlefield)
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
            if combat.removed_from_combat.contains(&creature)
                || self.zone_of(creature) != Some(Zone::Battlefield)
            {
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
            if let Some(declared_blockers) = combat.blockers.get(&attacker) {
                let live_blockers = declared_blockers
                    .iter()
                    .copied()
                    .filter(|blocker| {
                        self.zone_of(*blocker) == Some(Zone::Battlefield)
                            && !combat.removed_from_combat.contains(blocker)
                    })
                    .collect::<Vec<_>>();
                if live_blockers.is_empty() {
                    // A creature that was blocked remains blocked even if its
                    // blockers leave combat before damage. A live trample
                    // attacker can assign all of its positive damage to the
                    // defending player; other blocked attackers assign none.
                    if attacker_eligible && attacker_has_trample && attacker_power > 0 {
                        player_damage.push((attacker, defending_player, attacker_power));
                    }
                    continue;
                }
                if attacker_eligible && attacker_power > 0 {
                    if live_blockers.len() == 1 && !attacker_has_trample {
                        permanent_damage.push((attacker, live_blockers[0], attacker_power));
                    } else {
                        let mut remaining = attacker_power;
                        let mut assignments = Vec::with_capacity(live_blockers.len());
                        for blocker in &live_blockers {
                            let blocker_characteristics = self.characteristics(*blocker)?;
                            let blocker_toughness = blocker_characteristics
                                .toughness
                                .ok_or(RulesError::IllegalAction("blocker lacks toughness"))?;
                            let marked_damage = self.object(*blocker)?.damage;
                            let lethal = blocker_toughness.saturating_sub(marked_damage).max(0);
                            let assigned = remaining.min(lethal);
                            assignments.push((*blocker, assigned));
                            remaining -= assigned;
                        }
                        if attacker_has_trample {
                            if remaining > 0 {
                                player_damage.push((attacker, defending_player, remaining));
                            }
                        } else if remaining > 0 {
                            // The compatibility policy assigns any damage left
                            // after lethal has been assigned to every blocker
                            // to the first blocker in declaration order.
                            assignments[0].1 = assignments[0].1.saturating_add(remaining);
                        }
                        for (blocker, assigned) in assignments {
                            if assigned > 0 {
                                permanent_damage.push((attacker, blocker, assigned));
                            }
                        }
                    }
                }
                for blocker in live_blockers {
                    let blocker_power = self
                        .characteristics(blocker)?
                        .power
                        .ok_or(RulesError::IllegalAction("blocker lacks power"))?;
                    if eligible(blocker) && blocker_power > 0 {
                        permanent_damage.push((blocker, attacker, blocker_power));
                    }
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

    /// Removes a regenerated permanent from combat while retaining the fact
    /// that its attacker was blocked. A nontrample attacker must not become
    /// unblocked merely because its blocker regenerated.
    fn remove_from_combat(&mut self, card: ObjectId) {
        let Some(combat) = self.combat.as_mut() else {
            return;
        };
        if combat.attackers.contains(&card) {
            combat.attackers.retain(|attacker| *attacker != card);
            combat.hasty_attackers.remove(&card);
            combat.flying_attackers.remove(&card);
            combat.fear_attackers.remove(&card);
            combat.black_evasion_attackers.remove(&card);
            combat.unblockable_attackers.remove(&card);
            combat.vigilant_attackers.remove(&card);
            combat.trampling_attackers.remove(&card);
            combat.must_be_blocked_attackers.remove(&card);
            combat.landwalk_attackers.remove(&card);
            combat.blockers.remove(&card);
        }
        if combat
            .blockers
            .values()
            .flatten()
            .any(|blocker| *blocker == card)
        {
            combat.removed_from_combat.insert(card);
        }
    }

    /// Consumes one source-identified regeneration shield, if present, and
    /// performs its replacement work. This intentionally does not cover
    /// sacrifice or a zero-toughness state-based action.
    fn use_regeneration_shield(&mut self, target: ObjectId) -> Result<bool, RulesError> {
        let source = self
            .regeneration_shields
            .get_mut(&target)
            .and_then(Vec::pop);
        let Some(source) = source else {
            return Ok(false);
        };
        if self
            .regeneration_shields
            .get(&target)
            .is_some_and(Vec::is_empty)
        {
            self.regeneration_shields.remove(&target);
        }
        {
            let object = self
                .objects
                .get_mut(&target)
                .ok_or(RulesError::UnknownCard(target))?;
            object.tapped = true;
            object.damage = 0;
        }
        self.remove_from_combat(target);
        self.record_event(GameEvent::RegenerationShieldUsed { source, target });
        Ok(true)
    }

    /// Applies a destroy instruction, allowing one live regeneration shield
    /// to replace it before a `CardDestroyed` or zone-move receipt is emitted.
    fn destroy_permanent(&mut self, source: ObjectId, card: ObjectId) -> Result<(), RulesError> {
        self.destroy_permanent_with_regeneration(source, card, true)
    }

    /// Applies a destroy instruction whose source explicitly forbids a
    /// regeneration replacement, while preserving ordinary destruction and
    /// zone-departure receipts.
    fn destroy_permanent_without_regeneration(
        &mut self,
        source: ObjectId,
        card: ObjectId,
    ) -> Result<(), RulesError> {
        self.destroy_permanent_with_regeneration(source, card, false)
    }

    fn destroy_permanent_with_regeneration(
        &mut self,
        source: ObjectId,
        card: ObjectId,
        allow_regeneration: bool,
    ) -> Result<(), RulesError> {
        if self.zone_of(card) != Some(Zone::Battlefield) {
            return Ok(());
        }
        if allow_regeneration
            && self.characteristics(card).is_ok_and(|characteristics| {
                characteristics.card_types.contains(&CardType::Creature)
            })
            && self.use_regeneration_shield(card)?
        {
            return Ok(());
        }
        self.record_event(GameEvent::CardDestroyed { source, card });
        self.move_to_graveyard_or_remove_token(card)
    }

    /// Creates a token batch after its applicable quantity replacements have
    /// been resolved. The individual token constructor intentionally performs
    /// no replacement lookup, which makes the batch one non-recursive event.
    fn create_tokens(
        &mut self,
        controller: PlayerId,
        token: &TokenSpec,
        count: u8,
    ) -> Result<Vec<ObjectId>, RulesError> {
        self.player(controller)?;
        Self::validate_token_spec(token)?;
        if count == 0 {
            return Ok(Vec::new());
        }
        let replaced_count = self.replace_event_quantity(
            controller,
            ReplacementEventKind::TokenCreation,
            i16::from(count),
        )?;
        let count = u8::try_from(replaced_count).map_err(|_| {
            RulesError::IllegalAction("token creation quantity exceeds supported range")
        })?;
        let mut created = Vec::with_capacity(usize::from(count));
        for _ in 0..count {
            let token_id = self.create_token(controller, token.clone())?;
            self.record_event(GameEvent::TokenCreated {
                player: controller,
                token: token_id,
            });
            created.push(token_id);
        }
        Ok(created)
    }

    fn create_token(
        &mut self,
        controller: PlayerId,
        token: TokenSpec,
    ) -> Result<ObjectId, RulesError> {
        self.player(controller)?;
        Self::validate_token_spec(&token)?;
        let id = ObjectId(self.next_object_id);
        self.next_object_id += 1;
        self.objects.insert(
            id,
            CardObject {
                id,
                definition: None,
                incarnation: 1,
                owner: controller,
                controller,
                tapped: false,
                damage: 0,
                damage_shield: 0,
                counters: BTreeMap::new(),
                attached_to: None,
                attached_to_incarnation: None,
                entered_turn: self.turn,
                controller_changed_turn: self.turn,
                token: Some(token),
                copied_permanent: None,
            },
        );
        self.place_in_zone(controller, id, Zone::Battlefield)?;
        Ok(id)
    }

    fn move_to_graveyard_or_remove_token(&mut self, card: ObjectId) -> Result<(), RulesError> {
        if self.object(card)?.token.is_some() {
            let was_battlefield = self.zone_of(card) == Some(Zone::Battlefield);
            let expired_copy = self.object(card)?.copied_permanent.clone();
            let token_incarnation = self.object(card)?.incarnation;
            if was_battlefield {
                self.enqueue_another_creature_dies_triggers(card)?;
            }
            self.remove_from_all_zones(card);
            self.objects.remove(&card);
            self.regeneration_shields.remove(&card);
            self.record_event(GameEvent::TokenCeasedToExist { token: card });
            if let Some(copy) = expired_copy {
                self.record_event(GameEvent::PermanentCopyExpired {
                    target: card,
                    target_incarnation: token_incarnation,
                    timestamp: copy.timestamp,
                });
            }
            self.expire_continuous_effects_involving(card);
            return Ok(());
        }
        let was_battlefield = self.zone_of(card) == Some(Zone::Battlefield);
        let battlefield_incarnation = self.object(card)?.incarnation;
        let battlefield_controller = was_battlefield
            .then(|| self.controller_of(card))
            .transpose()?;
        let battlefield_colors = was_battlefield
            .then(|| {
                self.characteristics(card)
                    .map(|characteristics| characteristics.colors)
            })
            .transpose()?;
        let definition = self.card_definition(card)?.id;
        if was_battlefield {
            self.enqueue_another_creature_dies_triggers(card)?;
        }
        self.move_to_zone(card, Zone::Graveyard)?;
        if was_battlefield {
            let battlefield_colors = battlefield_colors.ok_or(RulesError::IllegalAction(
                "battlefield departure lacks source-color provenance",
            ))?;
            self.enqueue_dies_triggers(
                card,
                battlefield_incarnation,
                &battlefield_colors,
                definition,
                battlefield_controller.ok_or(RulesError::IllegalAction(
                    "battlefield departure lacks controller provenance",
                ))?,
            );
        }
        Ok(())
    }

    fn move_to_spell_terminal_zone(&mut self, card: ObjectId) -> Result<(), RulesError> {
        if self.exile_on_resolution.remove(&card) {
            self.move_to_zone(card, Zone::Exile)
        } else {
            self.move_to_graveyard_or_remove_token(card)
        }
    }

    /// Moves a spell card from a normal zone onto the stack.  The stack is a
    /// rules-relevant zone even though it is represented separately from the
    /// player zone vectors, so this transition must create a new incarnation
    /// before stack provenance is captured.
    fn move_to_stack(&mut self, card: ObjectId) -> Result<u64, RulesError> {
        self.object(card)?;
        self.advance_object_incarnation(card)?;
        self.remove_from_all_zones(card);
        Ok(self.object(card)?.incarnation)
    }

    /// Advances the stable card's monotonic zone-change identity and records
    /// the replay-visible provenance receipt.  Callers must perform exactly
    /// one real rules-zone transition after this succeeds.
    fn advance_object_incarnation(&mut self, card: ObjectId) -> Result<(), RulesError> {
        let incarnation =
            self.object(card)?
                .incarnation
                .checked_add(1)
                .ok_or(RulesError::IllegalAction(
                    "object incarnation counter overflowed",
                ))?;
        self.objects
            .get_mut(&card)
            .ok_or(RulesError::UnknownCard(card))?
            .incarnation = incarnation;
        Ok(())
    }

    fn move_to_zone(&mut self, card: ObjectId, zone: Zone) -> Result<(), RulesError> {
        let object = self.object(card)?.clone();
        let previous_zone = self.zone_of(card);
        let advanced_incarnation = previous_zone != Some(zone);
        if advanced_incarnation {
            self.advance_object_incarnation(card)?;
        }
        let left_battlefield =
            previous_zone == Some(Zone::Battlefield) && zone != Zone::Battlefield;
        self.remove_from_all_zones(card);
        let expired_copy = if left_battlefield {
            self.objects
                .get_mut(&card)
                .ok_or(RulesError::UnknownCard(card))?
                .copied_permanent
                .take()
        } else {
            None
        };
        if left_battlefield {
            self.regeneration_shields.remove(&card);
            self.objects
                .get_mut(&card)
                .ok_or(RulesError::UnknownCard(card))?
                .counters
                .clear();
            self.objects
                .get_mut(&card)
                .ok_or(RulesError::UnknownCard(card))?
                .attached_to = None;
            self.objects
                .get_mut(&card)
                .ok_or(RulesError::UnknownCard(card))?
                .attached_to_incarnation = None;
        }
        // Every public zone vector is ownership-indexed. Control effects are
        // derived layer-two state and therefore never relocate a permanent
        // from the owner's battlefield vector.
        let destination_owner = object.owner;
        if zone == Zone::Battlefield {
            let battlefield_object = self
                .objects
                .get_mut(&card)
                .ok_or(RulesError::UnknownCard(card))?;
            battlefield_object.tapped = false;
            battlefield_object.damage = 0;
            battlefield_object.damage_shield = 0;
            battlefield_object.attached_to = None;
            battlefield_object.attached_to_incarnation = None;
            battlefield_object.entered_turn = self.turn;
            battlefield_object.controller_changed_turn = self.turn;
        }
        self.place_in_zone(destination_owner, card, zone)?;
        self.record_event(GameEvent::CardMoved { card, to: zone });
        if advanced_incarnation {
            self.record_event(GameEvent::ObjectIncarnationAdvanced {
                object: card,
                incarnation: self.object(card)?.incarnation,
            });
        }
        if let Some(copy) = expired_copy {
            self.record_event(GameEvent::PermanentCopyExpired {
                target: card,
                target_incarnation: object.incarnation,
                timestamp: copy.timestamp,
            });
        }
        if left_battlefield {
            // Permanent effects cease when either their source or target
            // changes zones. End-of-turn effects from instants remain because
            // their source was never a battlefield permanent.
            self.expire_continuous_effects_involving(card);
        }
        Ok(())
    }

    /// Adds a represented persistent counter to one live permanent. Counter
    /// state belongs to the battlefield object and is cleared by the normal
    /// zone-departure transition above; it is not a hidden continuous effect.
    fn place_counter(
        &mut self,
        source: ObjectId,
        card: ObjectId,
        counter: CounterKind,
        amount: i16,
    ) -> Result<(), RulesError> {
        if !counter.is_valid() || amount <= 0 {
            return Err(RulesError::IllegalAction(
                "counter placement requires a valid positive counter",
            ));
        }
        self.object(source)?;
        self.require_zone(card, Zone::Battlefield)?;
        let amount = self.replace_event_quantity(
            self.object(card)?.controller,
            ReplacementEventKind::CounterPlacement { counter },
            amount,
        )?;
        let counters = &mut self
            .objects
            .get_mut(&card)
            .ok_or(RulesError::UnknownCard(card))?
            .counters;
        let next = counters
            .get(&counter)
            .copied()
            .unwrap_or_default()
            .checked_add(amount)
            .filter(|count| *count > 0)
            .ok_or(RulesError::IllegalAction(
                "counter total exceeds supported range",
            ))?;
        counters.insert(counter, next);
        self.record_event(GameEvent::CounterPlaced {
            source,
            card,
            counter,
            amount,
        });
        Ok(())
    }

    /// Removes a represented positive quantity of a typed counter from one
    /// live permanent. This does not pass through quantity replacement: a
    /// replacement such as Doubling Season changes placement, never a cost or
    /// effect that removes counters. Validation happens before mutation so a
    /// failed request rolls back its entire enclosing transition.
    fn remove_counter(
        &mut self,
        source: ObjectId,
        card: ObjectId,
        counter: CounterKind,
        amount: i16,
    ) -> Result<(), RulesError> {
        if !counter.is_valid() || amount <= 0 {
            return Err(RulesError::IllegalAction(
                "counter removal requires a valid positive counter",
            ));
        }
        self.object(source)?;
        self.require_zone(card, Zone::Battlefield)?;
        let counters = &mut self
            .objects
            .get_mut(&card)
            .ok_or(RulesError::UnknownCard(card))?
            .counters;
        let current = counters.get(&counter).copied().unwrap_or_default();
        if current < amount {
            return Err(RulesError::IllegalAction(
                "counter removal requires sufficient counters",
            ));
        }
        let remaining = current - amount;
        if remaining == 0 {
            counters.remove(&counter);
        } else {
            counters.insert(counter, remaining);
        }
        self.record_event(GameEvent::CounterRemoved {
            source,
            card,
            counter,
            amount,
        });
        Ok(())
    }

    fn expire_continuous_effects_involving(&mut self, card: ObjectId) {
        self.damage_redirections
            .retain(|redirect| redirect.protected != card);
        let expired_shields = self
            .damage_prevention_shields
            .iter()
            .filter(|shield| shield.target == Target::Permanent(card))
            .cloned()
            .collect::<Vec<_>>();
        self.damage_prevention_shields
            .retain(|shield| shield.target != Target::Permanent(card));
        for shield in expired_shields {
            self.record_event(GameEvent::DamageShieldExpired {
                source: shield.source,
                target: shield.target,
            });
        }
        let expired = self
            .continuous_effects
            .iter()
            .filter(|effect| effect.source == card || effect.target == card)
            .cloned()
            .collect::<Vec<_>>();
        let control_before = self
            .control_targets_before_expiration(&expired)
            .unwrap_or_default();
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
        // This helper is called only for transitions whose surrounding public
        // method runs the invariant audit. An absent target is an ordinary
        // zone-change case; a malformed live control effect is caught there.
        let _ = self.record_control_reversions(&control_before);
    }

    fn control_targets_before_expiration(
        &self,
        expired: &[ContinuousEffect],
    ) -> Result<BTreeMap<ObjectId, (PlayerId, ObjectId)>, RulesError> {
        let mut targets = BTreeMap::new();
        for effect in expired {
            if !matches!(effect.change, ContinuousChange::ChangeController(_))
                || self.zone_of(effect.target) != Some(Zone::Battlefield)
            {
                continue;
            }
            let controller = self.controller_of(effect.target)?;
            targets
                .entry(effect.target)
                .or_insert((controller, effect.source));
        }
        Ok(targets)
    }

    fn record_control_reversions(
        &mut self,
        before: &BTreeMap<ObjectId, (PlayerId, ObjectId)>,
    ) -> Result<(), RulesError> {
        for (target, (from, source)) in before {
            if self.zone_of(*target) != Some(Zone::Battlefield) {
                continue;
            }
            let to = self.controller_of(*target)?;
            if *from != to {
                self.objects
                    .get_mut(target)
                    .ok_or(RulesError::UnknownCard(*target))?
                    .controller_changed_turn = self.turn;
                self.record_event(GameEvent::ControllerChanged {
                    source: *source,
                    target: *target,
                    from: *from,
                    to,
                });
            }
        }
        Ok(())
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
        if let ManaAbilityOutput::Bundle(bundle) = &ability.output {
            if ability.amount != 0 {
                return Err(RulesError::IllegalAction(
                    "mana bundle ability must use bundle quantities instead of amount",
                ));
            }
            if bundle.is_empty() || bundle.iter().any(|(_, amount)| amount == 0) {
                return Err(RulesError::IllegalAction(
                    "mana ability bundle must contain positive mana amounts",
                ));
            }
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
        if matches!(&ability.output, ManaAbilityOutput::Choice(colors) if colors.contains(&Color::Colorless))
        {
            return Err(RulesError::IllegalAction(
                "a mana ability color choice may not offer colorless",
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
        let private_opponent_library_choice_effects = effects
            .iter()
            .filter(|effect| {
                matches!(
                    effect,
                    Effect::LookAtTopCardsOfTargetOpponentExileOne { .. }
                )
            })
            .count();
        if private_opponent_library_choice_effects > 0 && effects.len() != 1 {
            return Err(RulesError::IllegalAction(
                "a private opponent-library choice ability must contain exactly one effect",
            ));
        }
        for effect in effects {
            if effect.requires_chosen_x() {
                return Err(RulesError::IllegalAction(
                    "chosen-X effects are valid only on spells",
                ));
            }
            if matches!(
                effect,
                Effect::AttachSourceAndModifyTargetPt { .. } | Effect::AttachSourceToTarget { .. }
            ) {
                return Err(RulesError::IllegalAction(
                    "aura attachment effects are valid only on permanent spells",
                ));
            }
            if matches!(
                effect,
                Effect::LookAtTopCardsOfTargetOpponentExileOne { count: 0 }
            ) {
                return Err(RulesError::IllegalAction(
                    "private opponent-library choice must inspect at least one card",
                ));
            }
            let amount = match effect {
                Effect::DealDamage { amount, .. }
                | Effect::LoseLifeTarget { amount }
                | Effect::LoseLifeController { amount }
                | Effect::ReturnTargetPermanentToHandAndLoseControllerLife { amount }
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

    /// Each library search must retain its own atomic movement (when it found
    /// a card) and immediately-following shuffle receipt. This makes the
    /// hidden-zone selection observable without exposing every library card
    /// to an opponent or allowing a later event to impersonate the search.
    fn validate_library_search_event_order(events: &[GameEvent]) -> Result<(), RulesError> {
        for (index, event) in events.iter().enumerate() {
            let GameEvent::LibrarySearchResolved {
                player,
                found,
                destination,
                ..
            } = event
            else {
                continue;
            };
            if let Some(card) = found {
                let expected_zone = match destination {
                    LibrarySearchDestination::Battlefield
                    | LibrarySearchDestination::BattlefieldTapped => Zone::Battlefield,
                    LibrarySearchDestination::Hand => Zone::Hand,
                };
                let preceding_index = index.checked_sub(1).ok_or(RulesError::IllegalAction(
                    "library-search receipt lacks selected-card movement",
                ))?;
                let move_index = match events.get(preceding_index) {
                    Some(GameEvent::ObjectIncarnationAdvanced { object, .. }) if object == card => {
                        preceding_index
                            .checked_sub(1)
                            .ok_or(RulesError::IllegalAction(
                                "library-search incarnation receipt lacks selected-card movement",
                            ))?
                    }
                    _ => preceding_index,
                };
                if !matches!(
                    events.get(move_index),
                    Some(GameEvent::CardMoved { card: moved, to }) if moved == card && *to == expected_zone
                ) {
                    return Err(RulesError::IllegalAction(
                        "library-search receipt lacks movement to its declared destination",
                    ));
                }
            }
            if !matches!(
                events.get(index + 1),
                Some(GameEvent::LibraryShuffled { player: shuffled, .. }) if shuffled == player
            ) {
                return Err(RulesError::IllegalAction(
                    "library-search receipt lacks its immediate controller shuffle",
                ));
            }
        }
        Ok(())
    }

    /// An Aura attachment receipt is the public boundary between establishing
    /// its attachment-linked continuous effects and exposing that attachment
    /// to later state-based actions. It immediately follows the final matching
    /// effect receipt so replay cannot describe a modifier without attachment.
    fn validate_aura_attachment_event_order(events: &[GameEvent]) -> Result<(), RulesError> {
        for (index, event) in events.iter().enumerate() {
            let GameEvent::AuraAttached { aura, target } = event else {
                continue;
            };
            if !matches!(
                events.get(index.checked_sub(1).ok_or(RulesError::IllegalAction(
                    "aura attachment receipt lacks its continuous-effect receipt",
                ))?),
                Some(GameEvent::ContinuousEffectCreated {
                    source,
                    target: effect_target,
                    ..
                }) if source == aura && effect_target == target
            ) {
                return Err(RulesError::IllegalAction(
                    "aura attachment receipt is not paired with its final continuous effect",
                ));
            }
        }
        Ok(())
    }

    /// Audits delayed-action receipts independently from mutable game state.
    /// A schedule names unique nonzero action/group identities and exact
    /// positive member incarnations; one later consume may reference only its
    /// own schedule and report a duplicate-free subset of that group.
    fn validate_delayed_action_event_order(events: &[GameEvent]) -> Result<(), RulesError> {
        let mut scheduled =
            BTreeMap::<DelayedActionId, (LinkedExileGroupId, BTreeSet<ObjectId>)>::new();
        let mut consumed = BTreeSet::<DelayedActionId>::new();
        for event in events {
            match event {
                GameEvent::DelayedActionScheduled {
                    action,
                    timing,
                    due_turn,
                    group,
                    members,
                    ..
                } => {
                    if *action == DelayedActionId(0)
                        || *group == LinkedExileGroupId(0)
                        || *due_turn == 0
                        || *timing != DelayedActionTiming::EndStep
                        || members.is_empty()
                        || scheduled.contains_key(action)
                    {
                        return Err(RulesError::IllegalAction(
                            "delayed-action schedule receipt has invalid identity or timing",
                        ));
                    }
                    let mut objects = BTreeSet::new();
                    let primary_count = members
                        .iter()
                        .filter(|member| member.role == LinkedExileMemberRole::PrimaryCreature)
                        .count();
                    if primary_count != 1
                        || members.iter().any(|member| {
                            member.object.0 == 0
                                || member.exile_incarnation == 0
                                || !objects.insert(member.object)
                        })
                    {
                        return Err(RulesError::IllegalAction(
                            "delayed-action schedule receipt has invalid linked-exile members",
                        ));
                    }
                    scheduled.insert(*action, (*group, objects));
                }
                GameEvent::DelayedActionConsumed {
                    action,
                    group,
                    returned,
                } => {
                    let Some((scheduled_group, members)) = scheduled.get(action) else {
                        return Err(RulesError::IllegalAction(
                            "delayed-action consume receipt lacks a prior schedule",
                        ));
                    };
                    let mut returned_unique = BTreeSet::new();
                    if *scheduled_group != *group
                        || !consumed.insert(*action)
                        || returned
                            .iter()
                            .any(|card| !members.contains(card) || !returned_unique.insert(*card))
                    {
                        return Err(RulesError::IllegalAction(
                            "delayed-action consume receipt does not match its schedule",
                        ));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn validate_linked_exile_state(&self) -> Result<(), RulesError> {
        let mut actions_by_group = BTreeMap::<LinkedExileGroupId, usize>::new();
        let mut action_ids = BTreeSet::new();
        let mut largest_action = 0_u64;
        for action in &self.delayed_actions {
            if action.id == DelayedActionId(0)
                || !action_ids.insert(action.id)
                || action.due_turn < self.turn
            {
                return Err(RulesError::IllegalAction(
                    "delayed action has invalid identity or expired timing",
                ));
            }
            largest_action = largest_action.max(action.id.0);
            let DelayedActionKind::ReturnLinkedExileGroup { group } = action.kind;
            *actions_by_group.entry(group).or_default() += 1;
        }
        if self.next_delayed_action_id <= largest_action {
            return Err(RulesError::IllegalAction(
                "next delayed-action identity is not monotonic",
            ));
        }

        let mut largest_group = 0_u64;
        for (id, group) in &self.linked_exile_groups {
            if *id != group.id
                || *id == LinkedExileGroupId(0)
                || group.source.0 == 0
                || group.source_incarnation == 0
                || actions_by_group.get(id) != Some(&1)
            {
                return Err(RulesError::IllegalAction(
                    "linked exile group lacks exactly one delayed return action",
                ));
            }
            largest_group = largest_group.max(id.0);
            let mut members = BTreeSet::new();
            let primary_count = group
                .members
                .iter()
                .filter(|member| member.role == LinkedExileMemberRole::PrimaryCreature)
                .count();
            if group.members.is_empty()
                || primary_count != 1
                || group.members.iter().any(|member| {
                    member.object.0 == 0
                        || member.exile_incarnation == 0
                        || !members.insert(member.object)
                })
            {
                return Err(RulesError::IllegalAction(
                    "linked exile group has invalid exact-incarnation members",
                ));
            }
        }
        if self.next_linked_exile_group_id <= largest_group {
            return Err(RulesError::IllegalAction(
                "next linked-exile group identity is not monotonic",
            ));
        }
        if actions_by_group
            .keys()
            .any(|group| !self.linked_exile_groups.contains_key(group))
        {
            return Err(RulesError::IllegalAction(
                "delayed action references a missing linked-exile group",
            ));
        }
        Ok(())
    }

    /// Audits public incarnation receipts independently of the current game
    /// state.  A source may legitimately have left the game later, so replay
    /// checks only require each known stable object id to advance strictly and
    /// never use zero or an unallocated id.
    fn validate_object_incarnation_event_order(events: &[GameEvent]) -> Result<(), RulesError> {
        let mut last = BTreeMap::<ObjectId, u64>::new();
        for (index, event) in events.iter().enumerate() {
            let GameEvent::ObjectIncarnationAdvanced {
                object,
                incarnation,
            } = event
            else {
                continue;
            };
            if object.0 == 0 || *incarnation == 0 {
                return Err(RulesError::IllegalAction(
                    "object incarnation receipt has an invalid identity",
                ));
            }
            if last
                .get(object)
                .is_some_and(|previous| *incarnation <= *previous)
            {
                return Err(RulesError::IllegalAction(
                    "object incarnation receipts do not increase monotonically",
                ));
            }
            let predecessor = index
                .checked_sub(1)
                .and_then(|previous| events.get(previous));
            let follows_zone_transition = matches!(
                predecessor,
                Some(GameEvent::CardMoved { card, .. }) if card == object
            ) || matches!(
                predecessor,
                Some(GameEvent::SpellCast { card, .. }) if card == object
            );
            if !follows_zone_transition {
                return Err(RulesError::IllegalAction(
                    "object incarnation receipt is not adjacent to its zone transition",
                ));
            }
            last.insert(*object, *incarnation);
        }
        Ok(())
    }

    /// Copy receipts contain all source/target incarnation provenance needed
    /// to replay the layer-one lifecycle without consulting mutable derived
    /// characteristics.  The live-object audit separately proves that an
    /// active snapshot has not crossed a zone boundary.
    fn validate_permanent_copy_event_order(events: &[GameEvent]) -> Result<(), RulesError> {
        let mut created = BTreeSet::new();
        let mut expired = BTreeSet::new();
        for event in events {
            match event {
                GameEvent::PermanentCopied {
                    source,
                    source_incarnation,
                    target,
                    target_incarnation,
                    timestamp,
                } => {
                    if source.0 == 0
                        || target.0 == 0
                        || source == target
                        || *source_incarnation == 0
                        || *target_incarnation == 0
                        || *timestamp == 0
                        || !created.insert(*timestamp)
                    {
                        return Err(RulesError::IllegalAction(
                            "permanent-copy receipt has invalid or duplicate provenance",
                        ));
                    }
                }
                GameEvent::PermanentCopyExpired {
                    target,
                    target_incarnation,
                    timestamp,
                } if target.0 == 0
                    || *target_incarnation == 0
                    || *timestamp == 0
                    || !expired.insert(*timestamp) =>
                {
                    return Err(RulesError::IllegalAction(
                        "permanent-copy expiry receipt has invalid provenance",
                    ));
                }
                _ => {}
            }
        }
        Ok(())
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
                        Some(GameEvent::BoundManaAbilityFreeBundleActivated {
                            player: receipt_player,
                            source: receipt_source,
                            ability: receipt_ability,
                            bundle,
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
                                        "cast-payment free-bundle activation lacks its life-payment receipt",
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
                                        "cast-payment free-bundle activation lacks an ordered mana-output receipt",
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
        for (index, event) in events.iter().enumerate() {
            let GameEvent::BoundManaAbilityFreeBundleActivated {
                player,
                bundle,
                life_payment,
                ..
            } = event
            else {
                continue;
            };
            let mut output_index = index + 1;
            if let Some(amount) = life_payment {
                if !matches!(
                    events.get(output_index),
                    Some(GameEvent::ManaAbilityLifePaid {
                        player: receipt_player,
                        amount: receipt_amount,
                    }) if receipt_player == player && receipt_amount == amount
                ) {
                    return Err(RulesError::IllegalAction(
                        "free mana bundle activation lacks its life-payment receipt",
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
                    }) if receipt_player == player && receipt_color == &color && receipt_amount == &amount
                ) {
                    return Err(RulesError::IllegalAction(
                        "free mana bundle activation lacks an ordered mana-output receipt",
                    ));
                }
                output_index += 1;
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
        let mut open = BTreeMap::<(ObjectId, u64, &'static str), usize>::new();
        for event in &self.event_log {
            match event {
                GameEvent::AbilityActivated {
                    source,
                    source_incarnation,
                    ability,
                    ..
                }
                | GameEvent::TriggeredAbilityStacked {
                    source,
                    source_incarnation,
                    ability,
                    ..
                } => {
                    *open
                        .entry((*source, *source_incarnation, *ability))
                        .or_default() += 1;
                }
                GameEvent::AbilityResolved {
                    source,
                    source_incarnation,
                    ability,
                }
                | GameEvent::AbilityCounteredByRules {
                    source,
                    source_incarnation,
                    ability,
                } => {
                    let count = open
                        .get_mut(&(*source, *source_incarnation, *ability))
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
                BTreeMap::<(ObjectId, u64, &'static str), usize>::new(),
                |mut counts, item| {
                    *counts
                        .entry((
                            item.card,
                            item.source_incarnation,
                            item.ability_id.expect("checked above"),
                        ))
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

    /// Validates the public provenance emitted by the calculated activated
    /// cost pipeline.  The binding registry is immutable after setup, but the
    /// contributing permanents are intentionally historical here: a legal
    /// activation may sacrifice its source or a modifier after the total has
    /// already been calculated and paid.
    #[allow(clippy::too_many_lines)] // One receipt audit preserves total-cost provenance in one ordered scan.
    fn validate_activated_ability_cost_event_order(&self) -> Result<(), RulesError> {
        for (index, event) in self.event_log.iter().enumerate() {
            let GameEvent::ActivatedAbilityCostCalculated { context } = event else {
                continue;
            };
            if !matches!(context.kind, ActivatedAbilityKind::NonMana)
                || (context.increases.is_empty() && context.reductions.is_empty())
            {
                return Err(RulesError::IllegalAction(
                    "activated-cost receipt lacks a supported nonmana modification",
                ));
            }
            Self::validate_mana_cost(&context.base_mana_cost)?;
            Self::validate_mana_cost(&context.effective_mana_cost)?;
            if context.base_mana_cost.colored != context.effective_mana_cost.colored
                || context.base_mana_cost.hybrid != context.effective_mana_cost.hybrid
            {
                return Err(RulesError::IllegalAction(
                    "activated-cost modifier changed a colored or hybrid requirement",
                ));
            }
            if context.source_incarnation == 0
                || self.object(context.source)?.incarnation < context.source_incarnation
            {
                return Err(RulesError::IllegalAction(
                    "activated-cost receipt has an invalid source incarnation",
                ));
            }
            let definition = self.card_definition(context.source)?;
            let ability = self
                .activated_abilities
                .get(definition.id)
                .and_then(|abilities| abilities.get(context.ability_id))
                .ok_or(RulesError::IllegalAction(
                    "activated-cost receipt names an unbound ability",
                ))?;
            if ability.mana_cost != context.base_mana_cost
                || ability.additional_tap_creatures != context.additional_tap_creatures
                || ability.sacrifice_source != context.sacrifice_source
                || ability.sacrifice_creatures != context.sacrifice_creatures
                || ability.sacrifice_lands != context.sacrifice_lands
                || ability.discard_cards != context.discard_cards
            {
                return Err(RulesError::IllegalAction(
                    "activated-cost receipt does not match its ability binding",
                ));
            }
            if let Some(selection) = &context.payment_selection
                && (selection.generic.len() != usize::from(context.effective_mana_cost.generic)
                    || selection.hybrid.len() != context.effective_mana_cost.hybrid.len())
            {
                return Err(RulesError::IllegalAction(
                    "activated-cost receipt has an invalid mana selection",
                ));
            }
            let adjustment_matches = |adjustment: &ActivatedAbilityCostAdjustment,
                                      increase: bool|
             -> Result<bool, RulesError> {
                if adjustment.source_incarnation == 0
                    || self.object(adjustment.source)?.incarnation < adjustment.source_incarnation
                {
                    return Ok(false);
                }
                let source_definition = self.card_definition(adjustment.source)?.id;
                Ok(self
                    .activated_ability_cost_modifiers
                    .get(source_definition)
                    .is_some_and(|modifiers| {
                        modifiers.iter().any(|modifier| {
                            modifier.generic_amount() == adjustment.generic_amount
                                && !modifier.applies_to(ActivatedAbilityKind::Mana)
                                && matches!(
                                    (increase, modifier),
                                    (true, ActivatedAbilityCostModifier::IncreaseGeneric { .. })
                                        | (
                                            false,
                                            ActivatedAbilityCostModifier::ReduceGeneric { .. }
                                        )
                                )
                        })
                    }))
            };
            for adjustment in &context.increases {
                if adjustment.generic_amount == 0 || !adjustment_matches(adjustment, true)? {
                    return Err(RulesError::IllegalAction(
                        "activated-cost receipt names an invalid modifier contribution",
                    ));
                }
            }
            for adjustment in &context.reductions {
                if adjustment.generic_amount == 0 || !adjustment_matches(adjustment, false)? {
                    return Err(RulesError::IllegalAction(
                        "activated-cost receipt names an invalid modifier contribution",
                    ));
                }
            }
            let increased = context
                .increases
                .iter()
                .try_fold(
                    u16::from(context.base_mana_cost.generic),
                    |total, adjustment| total.checked_add(u16::from(adjustment.generic_amount)),
                )
                .ok_or(RulesError::IllegalAction(
                    "activated-cost receipt overflows its generic calculation",
                ))?;
            let reduced = context
                .reductions
                .iter()
                .try_fold(0_u16, |total, adjustment| {
                    total.checked_add(u16::from(adjustment.generic_amount))
                })
                .ok_or(RulesError::IllegalAction(
                    "activated-cost receipt overflows its generic calculation",
                ))?;
            if u16::from(context.effective_mana_cost.generic) != increased.saturating_sub(reduced) {
                return Err(RulesError::IllegalAction(
                    "activated-cost receipt has an incorrect effective generic cost",
                ));
            }
            if context.effective_mana_cost.mana_value() > 0
                && !matches!(
                    self.event_log.get(index + 1),
                    Some(GameEvent::AbilityManaPaid {
                        player,
                        source,
                        ability,
                        mana_cost,
                    }) if *player == context.acting_player
                        && *source == context.source
                        && *ability == context.ability_id
                        && *mana_cost == context.effective_mana_cost
                )
            {
                return Err(RulesError::IllegalAction(
                    "activated-cost receipt is not followed by its effective mana payment",
                ));
            }
        }
        Ok(())
    }

    /// A public private-opponent-library opening receipt must correspond to a
    /// still-open activated ability and must be consumed by that ability's one
    /// terminal resolution receipt. The event type deliberately has no card
    /// list; candidate identity is private state projected only to its
    /// controller while the choice is pending.
    fn validate_private_opponent_library_choice_event_order(
        events: &[GameEvent],
    ) -> Result<usize, RulesError> {
        let mut open_abilities = BTreeMap::<(ObjectId, u64, &'static str), usize>::new();
        let mut opened_choices = BTreeMap::<(ObjectId, u64, &'static str), usize>::new();
        for event in events {
            match event {
                GameEvent::AbilityActivated {
                    source,
                    source_incarnation,
                    ability,
                    ..
                } => {
                    *open_abilities
                        .entry((*source, *source_incarnation, *ability))
                        .or_default() += 1;
                }
                GameEvent::PrivateOpponentLibraryChoiceOpened {
                    source,
                    source_incarnation,
                    ability,
                    ..
                } => {
                    let key = (*source, *source_incarnation, *ability);
                    let open = open_abilities.get(&key).copied().unwrap_or_default();
                    let choices = opened_choices.entry(key).or_default();
                    *choices += 1;
                    if *choices > open {
                        return Err(RulesError::IllegalAction(
                            "private opponent-library opening lacks a live activated ability",
                        ));
                    }
                }
                GameEvent::AbilityResolved {
                    source,
                    source_incarnation,
                    ability,
                }
                | GameEvent::AbilityCounteredByRules {
                    source,
                    source_incarnation,
                    ability,
                } => {
                    let key = (*source, *source_incarnation, *ability);
                    // Triggered abilities have terminal receipts too, but this
                    // provenance audit owns only activated abilities. An
                    // unrelated trigger (for example Civic Wayfinder's ETB)
                    // must not be required to have an `AbilityActivated`
                    // receipt merely because another ability can suspend for a
                    // private opponent-library choice.
                    if let Some(open) = open_abilities.get_mut(&key) {
                        if *open == 0 {
                            return Err(RulesError::IllegalAction(
                                "private opponent-library receipt saw excess ability terminals",
                            ));
                        }
                        *open -= 1;
                        if let Some(choices) = opened_choices.get_mut(&key) {
                            if *choices > 0 {
                                *choices -= 1;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(opened_choices.values().sum())
    }

    /// A discard performed by a resolving effect must immediately enter its
    /// owner's graveyard. This is distinct from an activation's explicit
    /// `DiscardedAsAbilityCost` receipt, which is audited separately.
    fn validate_effect_discard_event_order(events: &[GameEvent]) -> Result<(), RulesError> {
        for (index, event) in events.iter().enumerate() {
            let GameEvent::CardDiscarded { card, .. } = event else {
                continue;
            };
            if !matches!(
                events.get(index + 1),
                Some(GameEvent::CardMoved {
                    card: moved_card,
                    to: Zone::Graveyard,
                }) if moved_card == card
            ) {
                return Err(RulesError::IllegalAction(
                    "effect discard receipt lacks an immediate graveyard move",
                ));
            }
        }
        Ok(())
    }

    /// A sacrifice performed by a resolving effect must transition its chosen
    /// permanent immediately, either to a graveyard or out of existence for a
    /// token. It may not leave an orphaned sacrifice receipt in a replay.
    fn validate_effect_sacrifice_event_order(events: &[GameEvent]) -> Result<(), RulesError> {
        for (index, event) in events.iter().enumerate() {
            let GameEvent::SacrificedByEffect { permanent, .. } = event else {
                continue;
            };
            let transitions_to_graveyard = matches!(
                events.get(index + 1),
                Some(GameEvent::CardMoved {
                    card: moved_card,
                    to: Zone::Graveyard,
                }) if moved_card == permanent
            );
            let token_ceases = matches!(
                events.get(index + 1),
                Some(GameEvent::TokenCeasedToExist { token }) if token == permanent
            );
            if !transitions_to_graveyard && !token_ceases {
                return Err(RulesError::IllegalAction(
                    "effect sacrifice receipt lacks its immediate zone transition",
                ));
            }
        }
        Ok(())
    }

    /// Counter receipts are stable, positive, battlefield-only state
    /// mutations. The live-zone audit proves the latter property; this replay
    /// audit prevents malformed named kinds or quantities from entering a
    /// canonical event trace. Placement and removal remain separate receipts
    /// because only placement is eligible for quantity replacement.
    fn validate_counter_lifecycle_events(events: &[GameEvent]) -> Result<(), RulesError> {
        for event in events {
            let (counter, amount) = match event {
                GameEvent::CounterPlaced {
                    counter, amount, ..
                }
                | GameEvent::CounterRemoved {
                    counter, amount, ..
                } => (*counter, *amount),
                _ => continue,
            };
            if !counter.is_valid() || amount <= 0 {
                return Err(RulesError::IllegalAction(
                    "counter receipt has an invalid kind or nonpositive amount",
                ));
            }
        }
        Ok(())
    }

    /// A counter removal must have enough previously recorded placement for
    /// the same live object in the current measured event epoch. This is a
    /// provenance check in addition to the live state invariant: it catches a
    /// fabricated negative transition even when its final map happens to look
    /// plausible. `clear_event_log` intentionally starts a fresh measured
    /// epoch, so this audit only checks removals whose matching placement is
    /// visible in the canonical trace.
    fn validate_counter_removal_receipt_accounting(&self) -> Result<(), RulesError> {
        let mut known = BTreeMap::<(ObjectId, CounterKind), i16>::new();
        for event in &self.event_log {
            match event {
                GameEvent::CounterPlaced {
                    card,
                    counter,
                    amount,
                    ..
                } => {
                    let entry = known.entry((*card, *counter)).or_default();
                    *entry = entry.checked_add(*amount).ok_or(RulesError::IllegalAction(
                        "counter receipt accounting overflowed",
                    ))?;
                }
                GameEvent::CounterRemoved {
                    card,
                    counter,
                    amount,
                    ..
                } => {
                    let entry = known.entry((*card, *counter)).or_default();
                    // A cleared or deliberately truncated event log can
                    // begin with a valid removal from extant counter state.
                    // Only a visible positive balance is therefore
                    // constrained here; the live transition enforces the
                    // actual sufficiency atomically.
                    if *entry > 0 {
                        *entry = entry.checked_sub(*amount).ok_or(RulesError::IllegalAction(
                            "counter removal receipt lacks matching placement",
                        ))?;
                        if *entry < 0 {
                            return Err(RulesError::IllegalAction(
                                "counter removal receipt exceeds visible placements",
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Replacement receipts form a finite quantity chain. A source can apply
    /// once to the event snapshot, producing either the next replacement's
    /// input or the first ordinary receipt for the replaced event.
    fn validate_replacement_effect_events(&self) -> Result<(), RulesError> {
        for (index, event) in self.event_log.iter().enumerate() {
            let GameEvent::ReplacementEffectApplied {
                source,
                affected_player,
                event: event_kind,
                original_amount,
                replacement_amount,
            } = event
            else {
                continue;
            };
            if affected_player.0 >= self.players.len() || *original_amount <= 0 {
                return Err(RulesError::IllegalAction(
                    "replacement receipt has an invalid affected player or amount",
                ));
            }
            let definition = self
                .objects
                .get(source)
                .and_then(CardObject::effective_definition)
                .or_else(|| self.departed_card_definitions.get(source).copied())
                .ok_or(RulesError::IllegalAction(
                    "replacement receipt source has no catalog definition",
                ))?;
            let effect_is_bound = self
                .replacement_effects
                .get(definition)
                .is_some_and(|effects| {
                    effects.iter().any(|effect| {
                        effect.applies_to(*event_kind)
                            && original_amount.checked_mul(i16::from(effect.multiplier()))
                                == Some(*replacement_amount)
                    })
                });
            if !effect_is_bound {
                return Err(RulesError::IllegalAction(
                    "replacement receipt does not match a registered source effect",
                ));
            }
            let next_is_chain_or_effect = match self.event_log.get(index + 1) {
                Some(GameEvent::ReplacementEffectApplied {
                    affected_player: next_player,
                    event: next_kind,
                    original_amount: next_original,
                    ..
                }) => {
                    next_player == affected_player
                        && next_kind == event_kind
                        && next_original == replacement_amount
                }
                Some(GameEvent::TokenCreated { .. }) => {
                    *event_kind == ReplacementEventKind::TokenCreation
                        && usize::try_from(*replacement_amount).is_ok_and(|expected| {
                            self.event_log[index + 1..]
                                .iter()
                                .take(expected)
                                .all(|candidate| {
                                    matches!(
                                        candidate,
                                        GameEvent::TokenCreated { player, .. }
                                            if player == affected_player
                                    )
                                })
                                && self.event_log[index + 1..].iter().take(expected).count()
                                    == expected
                        })
                }
                Some(GameEvent::CounterPlaced {
                    counter: placed_counter,
                    amount,
                    ..
                }) => matches!(
                    event_kind,
                    ReplacementEventKind::CounterPlacement {
                        counter: replacement_counter,
                    } if replacement_counter == placed_counter && amount == replacement_amount
                ),
                _ => false,
            };
            if !next_is_chain_or_effect {
                return Err(RulesError::IllegalAction(
                    "replacement receipt does not lead to its replaced event",
                ));
            }
        }
        Ok(())
    }

    /// Audits selected activated-ability sacrifice costs. A replay records a
    /// cost as one or more `SacrificedAsAbilityCost` plus immediate zone
    /// transitions before the eventual activation receipt. The set of
    /// receipts must be exactly the binding's source/creature/land cardinality
    /// in that order; orphaned or duplicate receipts cannot describe a legal
    /// state-machine transition.
    #[allow(clippy::too_many_lines)] // One ordered receipt audit preserves cost causality.
    fn validate_ability_sacrifice_cost_event_order(&self) -> Result<(), RulesError> {
        let mut consumed = BTreeSet::new();
        for (activation_index, event) in self.event_log.iter().enumerate() {
            let GameEvent::AbilityActivated {
                player,
                source,
                definition: source_definition,
                ability,
                ..
            } = event
            else {
                continue;
            };
            let binding = self
                .activated_abilities
                .get(source_definition)
                .and_then(|abilities| abilities.get(ability))
                .ok_or(RulesError::IllegalAction(
                    "sacrifice-cost receipt names an unbound ability",
                ))?;
            let expected = usize::from(binding.sacrifice_source)
                + usize::from(binding.sacrifice_creatures)
                + usize::from(binding.sacrifice_lands);
            let mut selected = Vec::new();
            let mut index = activation_index;
            while let Some(previous) = index.checked_sub(1) {
                match self.event_log.get(previous) {
                    Some(GameEvent::ObjectIncarnationAdvanced { object, .. }) => {
                        let Some(move_index) = previous.checked_sub(1) else {
                            break;
                        };
                        if !matches!(
                            self.event_log.get(move_index),
                            Some(GameEvent::CardMoved { card, .. }) if card == object
                        ) {
                            break;
                        }
                        // The incarnation receipt is structural provenance for
                        // the immediately preceding cost move. Step over it so
                        // the ordinary sacrifice/discard audit can consume the
                        // causal receipt and transition as one unit.
                        index = previous;
                    }
                    Some(GameEvent::AdditionalCreatureTappedAsAbilityCost {
                        player: receipt_player,
                        source: receipt_source,
                        ..
                    }) if receipt_player == player && receipt_source == source => {
                        index = previous;
                    }
                    Some(GameEvent::CardMoved {
                        card,
                        to: Zone::Graveyard,
                    }) => {
                        let Some(receipt_index) = previous.checked_sub(1) else {
                            break;
                        };
                        match self.event_log.get(receipt_index) {
                            Some(GameEvent::SacrificedAsAbilityCost {
                                player: receipt_player,
                                source: receipt_source,
                                permanent,
                            }) if receipt_player == player
                                && receipt_source == source
                                && permanent == card =>
                            {
                                consumed.insert(receipt_index);
                                selected.push(*permanent);
                                index = receipt_index;
                            }
                            Some(GameEvent::DiscardedAsAbilityCost {
                                player: receipt_player,
                                source: receipt_source,
                                card: discarded,
                            }) if receipt_player == player
                                && receipt_source == source
                                && discarded == card =>
                            {
                                index = receipt_index;
                            }
                            _ => break,
                        }
                    }
                    Some(GameEvent::TokenCeasedToExist { token }) => {
                        let Some(receipt_index) = previous.checked_sub(1) else {
                            break;
                        };
                        let Some(GameEvent::SacrificedAsAbilityCost {
                            player: receipt_player,
                            source: receipt_source,
                            permanent,
                        }) = self.event_log.get(receipt_index)
                        else {
                            break;
                        };
                        if receipt_player != player
                            || receipt_source != source
                            || permanent != token
                        {
                            break;
                        }
                        consumed.insert(receipt_index);
                        selected.push(*permanent);
                        index = receipt_index;
                    }
                    Some(GameEvent::AbilityManaPaid {
                        player: receipt_player,
                        source: receipt_source,
                        ability: receipt_ability,
                        ..
                    }) if receipt_player == player
                        && receipt_source == source
                        && receipt_ability == ability =>
                    {
                        break;
                    }
                    _ => break,
                }
            }
            selected.reverse();
            if selected.len() != expected {
                return Err(RulesError::IllegalAction(
                    "sacrifice-cost receipts do not match the activated ability binding",
                ));
            }
            if binding.sacrifice_source && selected.first() != Some(source) {
                return Err(RulesError::IllegalAction(
                    "sacrifice-cost receipts omit the required ability source",
                ));
            }
            if selected.iter().copied().collect::<BTreeSet<_>>().len() != selected.len() {
                return Err(RulesError::IllegalAction(
                    "sacrifice-cost receipts are not distinct",
                ));
            }
        }
        for (index, event) in self.event_log.iter().enumerate() {
            if matches!(event, GameEvent::SacrificedAsAbilityCost { .. })
                && !consumed.contains(&index)
            {
                return Err(RulesError::IllegalAction(
                    "sacrifice-cost receipt has no matching ability activation",
                ));
            }
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
            let (source_definition, ability_id) = self
                .event_log
                .iter()
                .skip(index + 1)
                .find_map(|event| match event {
                    GameEvent::AbilityActivated {
                        source: activated_source,
                        definition,
                        ability,
                        ..
                    } if activated_source == source => Some((*definition, *ability)),
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

    /// Audits explicit extra-creature tap selections. They are costs, not
    /// effects: every receipt must be contiguous with its ability activation
    /// and match the binding's declared cardinality.
    fn validate_ability_additional_tap_cost_event_order(&self) -> Result<(), RulesError> {
        let mut consumed = BTreeSet::new();
        for (activation_index, event) in self.event_log.iter().enumerate() {
            let GameEvent::AbilityActivated {
                player,
                source,
                definition: source_definition,
                ability,
                ..
            } = event
            else {
                continue;
            };
            let mut selected = Vec::new();
            let mut index = activation_index;
            while let Some(GameEvent::AdditionalCreatureTappedAsAbilityCost {
                player: receipt_player,
                source: receipt_source,
                permanent,
            }) = index
                .checked_sub(1)
                .and_then(|previous| self.event_log.get(previous))
            {
                if receipt_player != player || receipt_source != source {
                    break;
                }
                index -= 1;
                consumed.insert(index);
                selected.push(*permanent);
            }
            let bound = self
                .activated_abilities
                .get(source_definition)
                .and_then(|abilities| abilities.get(ability))
                .ok_or(RulesError::IllegalAction(
                    "additional tap-cost receipt names an unbound ability",
                ))?;
            if selected.len() != usize::from(bound.additional_tap_creatures) {
                return Err(RulesError::IllegalAction(
                    "additional tap-cost receipts do not match the activated ability binding",
                ));
            }
            let unique = selected.iter().copied().collect::<BTreeSet<_>>();
            if unique.len() != selected.len() || unique.contains(source) {
                return Err(RulesError::IllegalAction(
                    "additional tap-cost receipts are not distinct non-source creatures",
                ));
            }
        }
        for (index, event) in self.event_log.iter().enumerate() {
            if matches!(
                event,
                GameEvent::AdditionalCreatureTappedAsAbilityCost { .. }
            ) && !consumed.contains(&index)
            {
                return Err(RulesError::IllegalAction(
                    "additional tap-cost receipt has no matching ability activation",
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
            )) || (events[..index].iter().any(|event| {
                matches!(event, GameEvent::SpellCastFromGraveyard { card: cast_card, .. } if *cast_card == card)
            }) && matches!(
                events.get(index + 1),
                Some(GameEvent::CardMoved {
                    card: moved_card,
                    to: Zone::Exile,
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
        if cost.hybrid.iter().any(|symbol| {
            symbol.first == symbol.second
                || !symbol.first.is_colored()
                || !symbol.second.is_colored()
        }) {
            return Err(RulesError::IllegalAction(
                "a hybrid mana symbol requires two distinct card colors",
            ));
        }
        Ok(())
    }

    fn validate_card_colors(colors: &BTreeSet<Color>) -> Result<(), RulesError> {
        if colors.iter().any(|color| !color.is_colored()) {
            return Err(RulesError::IllegalAction(
                "card colors may not include the colorless mana kind",
            ));
        }
        Ok(())
    }

    fn validate_token_spec(token: &TokenSpec) -> Result<(), RulesError> {
        Self::validate_card_colors(&token.colors)?;
        if !token.creature_subtypes.is_empty() && !token.card_types.contains(&CardType::Creature) {
            return Err(RulesError::IllegalAction(
                "a creature subtype requires the creature card type",
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
            ManaAbilityOutput::Bundle(_) | ManaAbilityOutput::PaidBundle { .. } => {
                Err(RulesError::IllegalAction(
                    "fixed mana bundle ability does not resolve to one color",
                ))
            }
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
        if self.pending_private_library_choice.is_some() {
            return Err(RulesError::IllegalAction(
                "the private library choice must resolve before priority actions",
            ));
        }
        if self.pending_private_opponent_library_exile_choice.is_some() {
            return Err(RulesError::IllegalAction(
                "the private opponent-library choice must resolve before priority actions",
            ));
        }
        if self.pending_decision.is_some() {
            return Err(RulesError::IllegalAction(
                "the pending decision must resolve before priority actions",
            ));
        }
        if !self.pending_trigger_target_choices.is_empty() {
            return Err(RulesError::IllegalAction(
                "trigger targets must be chosen before priority actions",
            ));
        }
        if self.pending_optional_trigger_choice.is_some() {
            return Err(RulesError::IllegalAction(
                "optional trigger payment must resolve before priority actions",
            ));
        }
        if self.pending_damage_replacement_choice.is_some() {
            return Err(RulesError::IllegalAction(
                "damage replacement choice must resolve before priority actions",
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
        if let Some(choice) = &self.pending_private_library_choice {
            return choice.controller;
        }
        if let Some(choice) = &self.pending_private_opponent_library_exile_choice {
            return choice.controller;
        }
        if let Some(decision) = &self.pending_decision {
            return decision.player;
        }
        if let Some(choice) = self.pending_trigger_target_choices.first() {
            return choice.controller;
        }
        if let Some(choice) = &self.pending_optional_trigger_choice {
            return choice.controller;
        }
        if let Some(choice) = &self.pending_damage_replacement_choice {
            return choice.affected_player;
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

    fn decision_candidate_cards(
        &self,
        decision: &PendingDecision,
    ) -> Result<Vec<CardView>, RulesError> {
        decision
            .options
            .iter()
            .map(|option| match option {
                DecisionOption::Object(card) => self.card_view(*card),
            })
            .collect()
    }

    /// Audits the single generalized no-priority boundary and the public
    /// open/complete receipt lifecycle. Candidate identities remain only in
    /// private state and player-local views; the event log names just the
    /// decision identity and safe metadata.
    #[allow(clippy::too_many_lines)] // One cross-continuation audit keeps the exclusive decision boundary reviewable.
    fn validate_pending_decision(&self) -> Result<(), RulesError> {
        let mut receipt_state = BTreeMap::<DecisionId, (PlayerId, DecisionKind, bool)>::new();
        for event in &self.event_log {
            match event {
                GameEvent::DecisionOpened {
                    decision,
                    player,
                    kind,
                    ..
                } => {
                    if decision.0 == 0
                        || decision.0 >= self.next_decision_id
                        || receipt_state
                            .insert(*decision, (*player, *kind, false))
                            .is_some()
                    {
                        return Err(RulesError::IllegalAction(
                            "decision-opened receipts do not form a monotonic unique lifecycle",
                        ));
                    }
                }
                GameEvent::DecisionCompleted {
                    decision,
                    player,
                    kind,
                } => {
                    let Some((opened_player, opened_kind, completed)) =
                        receipt_state.get_mut(decision)
                    else {
                        return Err(RulesError::IllegalAction(
                            "decision-completed receipt lacks an opened decision",
                        ));
                    };
                    if *completed || *opened_player != *player || *opened_kind != *kind {
                        return Err(RulesError::IllegalAction(
                            "decision-completed receipt does not match its opened decision",
                        ));
                    }
                    *completed = true;
                }
                _ => {}
            }
        }

        let Some(decision) = &self.pending_decision else {
            if receipt_state.values().any(|(_, _, completed)| !completed) {
                return Err(RulesError::IllegalAction(
                    "a decision receipt remains open without pending decision state",
                ));
            }
            return Ok(());
        };
        if decision.id.0 == 0
            || decision.id.0 >= self.next_decision_id
            || decision.min_selections > decision.max_selections
            || usize::from(decision.max_selections) > decision.options.len()
            || self.players.get(decision.player.0).is_none()
            || self.players[decision.player.0].lost
            || self.priority != decision.player
            || self.consecutive_passes != 0
            || self.pending_draw_replacement.is_some()
            || self.pending_private_library_choice.is_some()
            || self.pending_private_opponent_library_exile_choice.is_some()
            || !self.pending_trigger_target_choices.is_empty()
            || self.pending_optional_trigger_choice.is_some()
            || self.pending_damage_replacement_choice.is_some()
            || decision
                .options
                .iter()
                .enumerate()
                .any(|(index, option)| decision.options[index + 1..].contains(option))
            || receipt_state.get(&decision.id) != Some(&(decision.player, decision.kind, false))
        {
            return Err(RulesError::IllegalAction(
                "pending decision violates its identity, cardinality, or no-priority boundary",
            ));
        }

        match &decision.continuation {
            DecisionContinuation::LibrarySearch {
                source,
                requirement,
                destination,
                may_fail_to_find,
            } => {
                let top = self.stack.last().ok_or(RulesError::IllegalAction(
                    "library-search decision escaped its stack item",
                ))?;
                let matches_stack = matches!(
                    top.effects.as_slice(),
                    [Effect::SearchControllerLibrary {
                        requirement: stack_requirement,
                        destination: stack_destination,
                        selection: LibrarySearchSelection::PolicySubmitted {
                            may_fail_to_find: stack_may_fail,
                        },
                    }] if stack_requirement == requirement
                        && stack_destination == destination
                        && stack_may_fail == may_fail_to_find
                );
                let expected_options = self
                    .library_search_candidates(decision.player, requirement, top.chosen_x)?
                    .into_iter()
                    .map(DecisionOption::Object)
                    .collect::<Vec<_>>();
                let (expected_min, expected_max) = if expected_options.is_empty() {
                    (0, 0)
                } else if *may_fail_to_find {
                    (0, 1)
                } else {
                    (1, 1)
                };
                if decision.kind != DecisionKind::LibrarySearch
                    || decision.visibility != DecisionVisibility::Private
                    || top.card != *source
                    || top.controller != decision.player
                    || !matches_stack
                    || decision.options != expected_options
                    || decision.min_selections != expected_min
                    || decision.max_selections != expected_max
                {
                    return Err(RulesError::IllegalAction(
                        "library-search decision escaped its typed continuation boundary",
                    ));
                }
            }
            DecisionContinuation::TriggeredEffectObject {
                source,
                controller,
                ability,
                kind,
            } => {
                let top = self.stack.last().ok_or(RulesError::IllegalAction(
                    "trigger-effect decision escaped its stack ability",
                ))?;
                let registered = self
                    .card_definition(*source)
                    .ok()
                    .and_then(|definition| self.triggered_abilities.get(definition.id))
                    .and_then(|abilities| abilities.get(ability));
                let (expected_options, expected_visibility, expected_effects) = match kind {
                    TriggeredEffectObjectDecisionKind::DiscardEachPlayer {
                        remaining_players,
                        selected,
                    } => {
                        if remaining_players.first() != Some(&decision.player)
                            || remaining_players
                                .iter()
                                .any(|player| self.players[player.0].lost)
                            || selected.iter().any(|(player, card)| {
                                self.zone_of(*card) != Some(Zone::Hand)
                                    || self
                                        .object(*card)
                                        .map_or(true, |object| object.owner != *player)
                            })
                        {
                            return Err(RulesError::IllegalAction(
                                "discard decision continuation has stale player or hand provenance",
                            ));
                        }
                        (
                            self.players[decision.player.0]
                                .hand
                                .iter()
                                .copied()
                                .map(DecisionOption::Object)
                                .collect::<Vec<_>>(),
                            DecisionVisibility::Private,
                            matches!(top.effects.as_slice(), [Effect::DiscardOneCardEachPlayer]),
                        )
                    }
                    TriggeredEffectObjectDecisionKind::SacrificeControllerCreature => (
                        self.all_battlefield_cards()
                            .into_iter()
                            .filter(|card| {
                                self.object(*card)
                                    .is_ok_and(|object| object.controller == decision.player)
                                    && self.characteristics(*card).is_ok_and(|characteristics| {
                                        characteristics.card_types.contains(&CardType::Creature)
                                    })
                            })
                            .map(DecisionOption::Object)
                            .collect::<Vec<_>>(),
                        DecisionVisibility::Public,
                        matches!(
                            top.effects.as_slice(),
                            [Effect::SacrificeControllerCreature]
                        ),
                    ),
                };
                let expected_min = u8::from(!expected_options.is_empty());
                if decision.kind != DecisionKind::TriggeredEffectObject
                    || top.card != *source
                    || top.controller != *controller
                    || top.ability_id != Some(*ability)
                    || registered.is_none()
                    || !expected_effects
                    || decision.visibility != expected_visibility
                    || decision.options != expected_options
                    || decision.min_selections != expected_min
                    || decision.max_selections != expected_min
                {
                    return Err(RulesError::IllegalAction(
                        "trigger-effect decision escaped its typed continuation boundary",
                    ));
                }
            }
        }
        Ok(())
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
        self.radiance_permanents_sharing_color_of_type(target, &CardType::Creature)
    }

    /// Returns the Radiance recipient set of one permanent card type. The
    /// legal target is included even when colorless; every other recipient
    /// must share at least one live color with that target at resolution.
    fn radiance_permanents_sharing_color_of_type(
        &self,
        target: ObjectId,
        card_type: &CardType,
    ) -> Result<Vec<ObjectId>, RulesError> {
        let target_colors = self.characteristics(target)?.colors;
        Ok(self
            .all_battlefield_cards()
            .into_iter()
            .filter(|candidate| {
                self.characteristics(*candidate)
                    .is_ok_and(|characteristics| {
                        characteristics.card_types.contains(card_type)
                            && (*candidate == target
                                || !characteristics.colors.is_disjoint(&target_colors))
                    })
            })
            .collect())
    }

    fn card_view(&self, card: ObjectId) -> Result<CardView, RulesError> {
        let object = self.object(card)?;
        let controller = self.controller_of(card)?;
        let characteristics = self.characteristics(card)?;
        let effective_definition = self.effective_definition_id(card)?;
        let mana_colors = effective_definition
            .and_then(|definition| self.catalog.get(definition))
            .map_or_else(BTreeSet::new, |definition| definition.mana_colors.clone());
        let basic_land_type = effective_definition
            .and_then(|definition| self.basic_land_types.get(definition).copied());
        let can_attack = controller == self.active_player
            && self.zone_of(card) == Some(Zone::Battlefield)
            && !object.tapped
            && (object.controller_changed_turn < self.turn
                || characteristics.keywords.contains(&Keyword::Haste))
            && characteristics.card_types.contains(&CardType::Creature)
            && !characteristics.keywords.contains(&Keyword::Defender);
        Ok(CardView {
            id: card,
            definition: effective_definition,
            controller,
            tapped: object.tapped,
            colors: characteristics.colors,
            mana_colors,
            basic_land_type,
            card_types: characteristics.card_types,
            can_attack,
        })
    }

    fn object_has_incarnation(&self, object: ObjectId, incarnation: u64) -> bool {
        self.object(object)
            .is_ok_and(|current| current.incarnation == incarnation)
    }

    fn effect_is_active(&self, effect: &ContinuousEffect) -> bool {
        if !self.object_has_incarnation(effect.target, effect.target_incarnation) {
            return false;
        }
        match effect.duration {
            Duration::EndOfTurn(turn) => turn == self.turn,
            Duration::Permanent => {
                self.zone_of(effect.source) == Some(Zone::Battlefield)
                    && self.object_has_incarnation(effect.source, effect.source_incarnation)
            }
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
                combat.hasty_attackers.clear();
                combat.flying_attackers.clear();
                combat.fear_attackers.clear();
                combat.black_evasion_attackers.clear();
                combat.unblockable_attackers.clear();
                combat.vigilant_attackers.clear();
                combat.trampling_attackers.clear();
                combat.must_be_blocked_attackers.clear();
                combat.landwalk_attackers.clear();
                combat.blockers.clear();
                combat.removed_from_combat.clear();
                combat.evasion_qualified_blockers.clear();
                combat.fear_qualified_blockers.clear();
                combat.black_evasion_qualified_blockers.clear();
                combat.first_strike_damage_sources.clear();
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
            self.regeneration_shields.remove(&object);
            self.record_event(GameEvent::ObjectLeftGame {
                object,
                owner: object_owner,
            });
            self.expire_continuous_effects_involving(object);
        }

        let controlled_but_not_owned = self
            .all_battlefield_cards()
            .into_iter()
            .filter(|object| {
                self.controller_of(*object) == Ok(player)
                    && self
                        .object(*object)
                        .is_ok_and(|state| state.owner != player)
            })
            .collect::<Vec<_>>();
        for object in controlled_but_not_owned {
            self.stack
                .retain(|stack_object| stack_object.card != object);
            let owner = self.objects[&object].owner;
            self.move_to_zone(object, Zone::Exile)
                .expect("controlled object must have a valid owner exile zone");
            self.regeneration_shields.remove(&object);
            debug_assert_eq!(self.objects[&object].controller, owner);
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

    #[test]
    fn defending_player_departure_must_clear_attacker_keyword_provenance() {
        let attacker = ObjectId(99);
        let mut game = Game::new(Vec::<CardDefinition>::new(), 3).expect("three-player game");
        game.step = Step::CombatDamage;
        game.combat = Some(CombatState {
            attackers_declared: true,
            blockers_declared: true,
            defending_player: Some(PlayerId(1)),
            attackers: vec![attacker],
            trampling_attackers: BTreeSet::from([attacker]),
            ..CombatState::default()
        });

        game.lose_player(PlayerId(1), "fixture defender departure");

        game.validate_invariants().expect(
            "defender departure must not leave attacker keyword provenance after removing attackers from combat",
        );
    }
}
