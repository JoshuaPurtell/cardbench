//! Deterministic, expansion-neutral rules substrate for the Magic `CardBench` variety.
//!
//! This is deliberately a rules engine, not a card-image database or a full oracle
//! implementation. Sets provide compact, executable card definitions; the engine owns
//! zones, stack and priority, turns, state-based actions, and layers.

#![forbid(unsafe_code)]

mod game;
mod model;

pub use game::{
    CardView, CastRequest, ConvokeContribution, ConvokePayment, DamageReplacementChoiceView, Game,
    GameView, LibrarySearchChoiceView, PendingDecisionView, PolicyAction, PrivateLibraryChoiceView,
    RulesError, TransmuteSearchView, TriggeredAbilityEffectObjectChoiceView,
};
pub use model::{
    AbilityActivation, AbilityCostPayment, ActivatedAbility, ActivatedAbilityBinding,
    ActivatedAbilityCostAdjustment, ActivatedAbilityCostBinding, ActivatedAbilityCostContext,
    ActivatedAbilityCostModifier, ActivatedAbilityCostModifierBinding, ActivatedAbilityKind,
    ActivatedCounterCost, ActivatedCounterCostTarget, ActivatedManaAbility, AdditionalSpellCost,
    AdditionalSpellCostBinding, AttachmentBinding, AttachmentKind, BasicLandManaAbilityActivation,
    BasicLandType, BasicLandTypeBinding, CardDefinition, CardObject, CardType,
    CastPaymentManaAbility, CastPermissionPayment, CastPermissionZone, CastTiming, Characteristics,
    Color, CombatBlock, ContinuousChange, ContinuousEffect, CopiableValues, CopiedPermanent,
    CostReductionBinding, CounterKind, CreatureSubtype, DamageReplacementChoice,
    DamageReplacementEffect, DamageReplacementEffectBinding, DecisionContinuation, DecisionId,
    DecisionKind, DecisionOption, DecisionSelection, DecisionVisibility, DeckEntry, DeckList,
    DeckRules, DeckValidationError, DelayedAction, DelayedActionId, DelayedActionKind,
    DelayedActionTiming, Duration, Effect, GameEvent, GeneralizedAbilityActivation,
    GeneralizedActivatedAbilityCost, HybridManaSymbol, Keyword, LandEntryBinding, Layer,
    LibrarySearchCardinality, LibrarySearchDestination, LibrarySearchRequirement,
    LibrarySearchSelection, LinkedExileGroup, LinkedExileGroupId, LinkedExileMember,
    LinkedExileMemberRole, ManaAbilityActivation, ManaAbilityBinding, ManaAbilityOutput,
    ManaBundle, ManaCost, ManaPaymentSelection, ManaPool, ObjectId, PendingDecision, PlayerId,
    PlayerState, PolicyMoveKind, QuantityReplacementResolution, ReplacementChoice,
    ReplacementEffect, ReplacementEffectBinding, ReplacementEventKind,
    ResolutionPaymentManaAbility, StackEffectResolution, StackObject, StackResolutionPlan,
    StackTargetArityError, StaticAttackRestriction, StaticAttackRestrictionBinding,
    StaticContinuousEffectBinding, StaticEntryRestriction, StaticEntryRestrictionBinding, Step,
    TRANSMUTE_ABILITY_ID, Target, TargetRequirement, TokenSpec, TriggerCondition,
    TriggerOrderEntry, TriggeredAbility, TriggeredAbilityBinding,
    TriggeredEffectObjectDecisionKind, Zone,
};
