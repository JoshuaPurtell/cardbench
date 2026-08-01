//! Deterministic, expansion-neutral rules substrate for the Magic `CardBench` variety.
//!
//! This is deliberately a rules engine, not a card-image database or a full oracle
//! implementation. Sets provide compact, executable card definitions; the engine owns
//! zones, stack and priority, turns, state-based actions, and layers.

#![forbid(unsafe_code)]

mod game;
mod model;

pub use game::{
    CardView, CastRequest, ConvokeContribution, ConvokePayment, Game, GameView, PolicyAction,
    RulesError, TransmuteSearchView,
};
pub use model::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, ActivatedManaAbility,
    AdditionalSpellCost, AdditionalSpellCostBinding, BasicLandManaAbilityActivation, BasicLandType,
    BasicLandTypeBinding, CardDefinition, CardObject, CardType, CastPaymentManaAbility,
    Characteristics, Color, CombatBlock, ContinuousChange, ContinuousEffect, CreatureSubtype,
    DeckEntry, DeckList, DeckRules, DeckValidationError, Duration, Effect, GameEvent,
    HybridManaSymbol, Keyword, LandEntryBinding, Layer, ManaAbilityActivation, ManaAbilityBinding,
    ManaAbilityOutput, ManaBundle, ManaCost, ManaPaymentSelection, ManaPool, ObjectId, PlayerId,
    PlayerState, PolicyMoveKind, StackEffectResolution, StackObject, StackResolutionPlan,
    StackTargetArityError, StaticContinuousEffectBinding, Step, Target, TargetRequirement,
    TokenSpec, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};
