//! Deterministic, expansion-neutral rules substrate for the Magic `CardBench` variety.
//!
//! This is deliberately a rules engine, not a card-image database or a full oracle
//! implementation. Sets provide compact, executable card definitions; the engine owns
//! zones, stack and priority, turns, state-based actions, and layers.

#![forbid(unsafe_code)]

mod game;
mod model;

pub use game::{
    CardView, CastRequest, ConvokeContribution, ConvokePayment, Game, GameView,
    LibrarySearchChoiceView, PolicyAction, PrivateLibraryChoiceView, RulesError,
    TransmuteSearchView, TriggeredAbilityEffectObjectChoiceView,
};
pub use model::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, ActivatedAbilityCostAdjustment,
    ActivatedAbilityCostContext, ActivatedAbilityCostModifier, ActivatedAbilityCostModifierBinding,
    ActivatedAbilityKind, ActivatedManaAbility, AdditionalSpellCost, AdditionalSpellCostBinding,
    BasicLandManaAbilityActivation, BasicLandType, BasicLandTypeBinding, CardDefinition,
    CardObject, CardType, CastPaymentManaAbility, Characteristics, Color, CombatBlock,
    ContinuousChange, ContinuousEffect, CostReductionBinding, CounterKind, CreatureSubtype,
    DeckEntry, DeckList, DeckRules, DeckValidationError, Duration, Effect, GameEvent,
    HybridManaSymbol, Keyword, LandEntryBinding, Layer, LibrarySearchDestination,
    LibrarySearchRequirement, LibrarySearchSelection, ManaAbilityActivation, ManaAbilityBinding,
    ManaAbilityOutput, ManaBundle, ManaCost, ManaPaymentSelection, ManaPool, ObjectId, PlayerId,
    PlayerState, PolicyMoveKind, ReplacementEffect, ReplacementEffectBinding, ReplacementEventKind,
    StackEffectResolution, StackObject, StackResolutionPlan, StackTargetArityError,
    StaticAttackRestriction, StaticAttackRestrictionBinding, StaticContinuousEffectBinding, Step,
    Target, TargetRequirement, TokenSpec, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding, Zone,
};
