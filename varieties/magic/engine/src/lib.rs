//! Deterministic, expansion-neutral rules substrate for the Magic `CardBench` variety.
//!
//! This is deliberately a rules engine, not a card-image database or a full oracle
//! implementation. Sets provide compact, executable card definitions; the engine owns
//! zones, stack and priority, turns, state-based actions, and layers.

#![forbid(unsafe_code)]

mod game;
mod model;

pub use game::{
    ActivatedAbilityStackView, CardView, CastRequest, ConvokeContribution, ConvokePayment,
    DamageReplacementChoiceView, Game, GameView, LibrarySearchChoiceView, PendingDecisionView,
    PolicyAction, PrivateLibraryChoiceView, RevealedLibraryTopView, RulesError,
    TransmuteSearchView, TriggeredAbilityEffectObjectChoiceView,
};
pub use model::{
    AbilityActivation, AbilityCostPayment, ActivatedAbility, ActivatedAbilityBinding,
    ActivatedAbilityCostAdjustment, ActivatedAbilityCostBinding, ActivatedAbilityCostContext,
    ActivatedAbilityCostModifier, ActivatedAbilityCostModifierBinding, ActivatedAbilityKind,
    ActivatedCounterCost, ActivatedCounterCostTarget, ActivatedManaAbility, AdditionalSpellCost,
    AdditionalSpellCostBinding, AttachmentBinding, AttachmentKind, BasicLandManaAbilityActivation,
    BasicLandType, BasicLandTypeBinding, BattlefieldCreatureSnapshot, CapturedCombatParticipant,
    CapturedConvokeCreature, CardDefinition, CardObject, CardType, CastPaymentManaAbility,
    CastPermissionPayment, CastPermissionZone, CastTiming, Characteristics, Color, CombatBlock,
    ContinuousChange, ContinuousEffect, CopiableValues, CopiedPermanent, CostReductionBinding,
    CounterKind, CreatureSubtype, DELAYED_COMBAT_HISTORY_DESTRUCTION_ABILITY_ID,
    DamageReplacementChoice, DamageReplacementEffect, DamageReplacementEffectBinding,
    DamageReplacementPacket, DecisionContinuation, DecisionId, DecisionKind, DecisionOption,
    DecisionSelection, DecisionVisibility, DeckEntry, DeckList, DeckRules, DeckValidationError,
    DelayedAction, DelayedActionId, DelayedActionKind, DelayedActionTiming, Duration, Effect,
    EntryCopyBinding, EntryCopySnapshot, GameEvent, GeneralizedAbilityActivation,
    GeneralizedActivatedAbilityCost, GraveyardCreatureCardSnapshot, GraveyardLandCardSnapshot,
    HandCardSnapshot, HybridManaSymbol, Keyword, LandEntryBinding, Layer, LibrarySearchCardinality,
    LibrarySearchDestination, LibrarySearchRequirement, LibrarySearchSelection, LinkedExileGroup,
    LinkedExileGroupId, LinkedExileMember, LinkedExileMemberRole, ManaAbilityActivation,
    ManaAbilityBinding, ManaAbilityBundleChoiceActivation, ManaAbilityCostBinding,
    ManaAbilityOutput, ManaBundle, ManaCost, ManaPaymentSelection, ManaPool, ObjectId,
    PendingDecision, PlayerId, PlayerState, PolicyMoveKind, QuantityReplacementResolution,
    ReplacementChoice, ReplacementEffect, ReplacementEffectBinding, ReplacementEventKind,
    ResolutionPaymentManaAbility, SharedKeywordFamily, StackEffectResolution, StackObject,
    StackObjectId, StackResolutionPlan, StackTargetArityError, StaticAttackRestriction,
    StaticAttackRestrictionBinding, StaticContinuousEffectBinding, StaticEntryRestriction,
    StaticEntryRestrictionBinding, StaticLibraryTopRevealBinding, StaticLibraryTopRevealScope,
    Step, TRANSMUTE_ABILITY_ID, Target, TargetRequirement, TokenSpec, TriggerCondition,
    TriggerOrderEntry, TriggeredAbility, TriggeredAbilityBinding,
    TriggeredEffectObjectDecisionKind, Zone,
};
