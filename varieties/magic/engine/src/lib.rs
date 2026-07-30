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
    ActivatedManaAbility, CardDefinition, CardObject, CardType, Characteristics, Color,
    CombatBlock, ContinuousChange, ContinuousEffect, DeckEntry, DeckList, DeckRules,
    DeckValidationError, Duration, Effect, GameEvent, Keyword, Layer, ManaAbilityActivation,
    ManaAbilityBinding, ManaAbilityOutput, ManaBundle, ManaCost, ManaPool, ObjectId, PlayerId,
    PlayerState, PolicyMoveKind, StackObject, Step, Target, TargetRequirement, TokenSpec, Zone,
};
