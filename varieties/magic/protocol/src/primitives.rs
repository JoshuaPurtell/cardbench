//! Owned mirrors of the engine's game vocabulary.
//!
//! These deliberately duplicate engine enums instead of re-exporting them.
//! The engine is free to add an internal variant or change a representation;
//! this schema may only change with a version bump. Keeping the two apart is
//! what stops a transport from becoming a second rules implementation.

use crate::ids::{ObjectRef, SeatId, StackObjectRef};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A card colour, or the colourless mana kind.
///
/// `Colorless` is a mana kind rather than a card colour and is never a legal
/// answer to a "choose a colour" decision.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ColorDto {
    White,
    Blue,
    Black,
    Red,
    Green,
    Colorless,
}

impl ColorDto {
    /// The five card colours, in the engine's canonical order.
    pub const CARD_COLORS: [Self; 5] =
        [Self::White, Self::Blue, Self::Black, Self::Red, Self::Green];

    /// Every mana kind, including colourless.
    pub const ALL: [Self; 6] = [
        Self::White,
        Self::Blue,
        Self::Black,
        Self::Red,
        Self::Green,
        Self::Colorless,
    ];
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum CardTypeDto {
    Artifact,
    Creature,
    Land,
    Enchantment,
    Instant,
    Planeswalker,
    Sorcery,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum BasicLandTypeDto {
    Plains,
    Island,
    Swamp,
    Mountain,
    Forest,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ZoneDto {
    Library,
    Hand,
    Battlefield,
    Graveyard,
    Exile,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum StepDto {
    Untap,
    Upkeep,
    Draw,
    PrecombatMain,
    BeginningOfCombat,
    DeclareAttackers,
    DeclareBlockers,
    FirstStrikeCombatDamage,
    CombatDamage,
    EndOfCombat,
    PostcombatMain,
    End,
    Cleanup,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct HybridManaSymbolDto {
    pub first: ColorDto,
    pub second: ColorDto,
}

/// A printed or computed mana cost.
#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ManaCostDto {
    pub generic: u8,
    pub colored: Vec<ColorDto>,
    pub hybrid: Vec<HybridManaSymbolDto>,
}

/// One colour/amount pair in a pool or bundle.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ManaAmountDto {
    pub color: ColorDto,
    pub amount: u8,
}

/// A mana pool projected as an ordered, sparse list.
///
/// A sparse list rather than a map keeps the JSON encoding stable across
/// serialisers and avoids depending on enum-keyed map support.
#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ManaPoolDto {
    /// Nonzero amounts only, ordered by [`ColorDto::ALL`].
    pub amounts: Vec<ManaAmountDto>,
}

impl ManaPoolDto {
    #[must_use]
    pub fn amount(&self, color: ColorDto) -> u8 {
        self.amounts
            .iter()
            .find(|entry| entry.color == color)
            .map_or(0, |entry| entry.amount)
    }

    #[must_use]
    pub fn total(&self) -> u32 {
        self.amounts
            .iter()
            .map(|entry| u32::from(entry.amount))
            .sum()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.amounts.is_empty()
    }
}

/// A selectable multi-colour mana output, projected like a pool.
#[derive(Clone, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ManaBundleDto {
    pub amounts: Vec<ManaAmountDto>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum CounterKindDto {
    PlusOnePlusOne,
    MinusOneMinusOne,
    Charge,
    Depletion,
    Doom,
    Flood,
    Lore,
    Loyalty,
    Spore,
    Storage,
    Time,
    Verse,
    Other,
}

/// Anything a spell, ability, or cost can name.
///
/// [`Self::Seat`] replaces the engine's `Target::Player`. Naming it a seat at
/// the transport boundary is deliberate: a seat is addressable after
/// elimination and is independent of team membership.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum TargetDto {
    Seat(SeatId),
    Permanent(ObjectRef),
    Spell(ObjectRef),
    ActivatedAbility(StackObjectRef),
    SacrificePermanent(ObjectRef),
    BasicLandType(BasicLandTypeDto),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ReplacementEffectDto {
    MultiplyTokenCreation { multiplier: u8 },
    MultiplyCounterPlacement { multiplier: u8 },
}

/// One applicable replacement for a prospective damage event.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum DamageReplacementChoiceDto {
    HalveDamage {
        source: ObjectRef,
        source_incarnation: u64,
    },
    PreventSelfDamageAndAddPlusOneCounters {
        source: ObjectRef,
        source_incarnation: u64,
    },
    CombatDamageMillAndCounters {
        source: ObjectRef,
        source_incarnation: u64,
    },
    CombatDamagePrevention {
        id: u64,
        source: ObjectRef,
    },
    Redirect {
        id: u64,
        source: ObjectRef,
        protected: ObjectRef,
        destination: TargetDto,
    },
    AttachedRedirect {
        attachment: ObjectRef,
        attachment_incarnation: u64,
        protected: ObjectRef,
        protected_incarnation: u64,
        destination: SeatId,
    },
    TargetedShield {
        id: u64,
        source: ObjectRef,
        target: TargetDto,
    },
    PermanentShield {
        permanent: ObjectRef,
    },
    SourceColorPrevention {
        permanent: ObjectRef,
    },
}

/// A typed replacement identity offered to an affected seat for ordering.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum ReplacementChoiceDto {
    Quantity {
        source: ObjectRef,
        source_incarnation: u64,
        effect: ReplacementEffectDto,
    },
    Damage(DamageReplacementChoiceDto),
}

/// One member of a simultaneous-trigger ordering group.
///
/// Source incarnation is part of the identity: a source may have left the
/// battlefield after triggering, and a later incarnation reusing the same
/// object id must not satisfy a stale ordering response.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct TriggerOrderEntryDto {
    pub source: ObjectRef,
    pub source_incarnation: u64,
    pub ability: crate::ids::AbilityName,
    /// One-based, scoped to one pending group.
    pub occurrence: u8,
}

/// A card as projected to one viewer.
///
/// `definition` is `None` when the viewer is not entitled to the identity.
/// That is the single redaction switch for card identity: a hidden card is
/// still counted and still has a stable [`ObjectRef`], but no name.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CardDto {
    pub object: ObjectRef,
    pub definition: Option<crate::ids::CardName>,
    pub controller: SeatId,
    pub tapped: bool,
    pub colors: BTreeSet<ColorDto>,
    pub mana_colors: BTreeSet<ColorDto>,
    pub basic_land_type: Option<BasicLandTypeDto>,
    pub card_types: BTreeSet<CardTypeDto>,
    pub can_attack: bool,
}

impl CardDto {
    /// Strips every characteristic a viewer outside the owning zone must not
    /// learn, leaving a positional placeholder.
    ///
    /// Colours and types are removed alongside the name because a hidden card
    /// whose colour is known is still a partial reveal.
    #[must_use]
    pub fn redacted(&self) -> Self {
        Self {
            object: self.object,
            definition: None,
            controller: self.controller,
            tapped: self.tapped,
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            basic_land_type: None,
            card_types: BTreeSet::new(),
            can_attack: false,
        }
    }

    /// Whether this projection still names the card.
    #[must_use]
    pub const fn is_identified(&self) -> bool {
        self.definition.is_some()
    }
}
