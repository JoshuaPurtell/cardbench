use std::collections::{BTreeMap, BTreeSet};

/// A stable, monotonic in-game object identifier. It is never a card database id.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ObjectId(pub u64);

/// Index of a seated player in turn order.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlayerId(pub usize);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Color {
    White,
    Blue,
    Black,
    Red,
    Green,
    /// A mana kind, not a card color. It can pay generic and explicit
    /// colorless costs but never satisfies a colored or hybrid symbol.
    Colorless,
}

/// One of Magic's five typed basic-land subtypes.
///
/// This is deliberately distinct from a land's display name and from the
/// `is_basic_land` deck-construction flag. A set binds this type to a specific
/// land definition, and the rules engine validates that the definition's
/// intrinsic mana ability produces exactly this type's color.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BasicLandType {
    Plains,
    Island,
    Swamp,
    Mountain,
    Forest,
}

impl BasicLandType {
    #[must_use]
    pub const fn intrinsic_mana_color(self) -> Color {
        match self {
            Self::Plains => Color::White,
            Self::Island => Color::Blue,
            Self::Swamp => Color::Black,
            Self::Mountain => Color::Red,
            Self::Forest => Color::Green,
        }
    }
}

/// Binds a typed basic-land type line to a set's land definition.
///
/// Keeping this alongside `CardDefinition`, like definition-bound mana
/// abilities, lets the engine remain expansion-neutral while preserving the
/// compact catalog structure. `Game` rejects a binding unless it names a
/// basic land whose intrinsic one-color mana ability matches the typed land.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BasicLandTypeBinding {
    pub card_definition: &'static str,
    pub land_type: BasicLandType,
}

/// The expansion-neutral class of card a library-search effect may select.
/// The engine works from typed catalog facts rather than copied card wording.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LibrarySearchRequirement {
    /// A land card whose registered basic-land type is one of the allowed
    /// types. The binding is a type-line fact, not a display-name match.
    BasicLandTypes(BTreeSet<BasicLandType>),
    /// A creature card whose mana value does not exceed the X value retained
    /// on the resolving spell. This is typed card information rather than a
    /// card-name or display-text predicate.
    CreatureWithManaValueAtMostChosenX,
    /// Any card with exactly the named mana value. This is a catalog fact,
    /// rather than a display-name predicate, and supplies Transmute's shared
    /// library-search boundary.
    ManaValueExactly(u8),
    /// A card whose type line contains every requested card type. This is a
    /// typed catalog predicate: it never infers card identity from display
    /// text, and it can represent an expansion-neutral "creature card",
    /// "enchantment card", or combined-type library search.
    CardTypes(BTreeSet<CardType>),
}

/// The quantity a policy-submitted library search may choose.  Exact searches
/// retain their required cardinality when enough matching cards exist;
/// otherwise they resolve as a legal failure to find rather than exposing a
/// malformed decision whose minimum exceeds its option set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibrarySearchCardinality {
    /// Select any number of matching cards through the stated inclusive upper
    /// bound. This models effects such as "up to two" without treating an
    /// empty selection as an error.
    ZeroOrMore { maximum: u8 },
    /// Select exactly this many cards when possible. A policy-submitted
    /// search with `may_fail_to_find` may instead select fewer cards from a
    /// hidden library; otherwise an insufficient candidate set is an ordinary
    /// legal failure-to-find boundary.
    Exactly(u8),
}

/// Stable identity for the rules-defined hand-zone Transmute activated
/// ability. It is intentionally not an expansion binding: any card definition
/// carrying [`Keyword::Transmute`] owns this one generic ability.
pub const TRANSMUTE_ABILITY_ID: &str = "transmute";

/// Destination for a selected library card. A tapped battlefield entry is a
/// single semantic destination so the public event log cannot claim an
/// untapped entry followed by an unrelated tap action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibrarySearchDestination {
    /// Put the selected card onto the battlefield without an entry-tapped
    /// replacement.
    Battlefield,
    BattlefieldTapped,
    Hand,
}

/// How a typed library-search instruction selects among its matching cards.
///
/// The deterministic variant preserves explicitly bounded compatibility
/// cards. Policy submission suspends the resolving stack item and exposes
/// only the controller's legal matching cards through [`GameView`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibrarySearchSelection {
    DeterministicFirstMatch,
    PolicySubmitted {
        /// A controller may choose no card even while one or more matching
        /// cards exist. This represents searches of a hidden zone that may
        /// legally fail to find a card with the requested characteristic.
        may_fail_to_find: bool,
    },
}

/// Immutable behavior applied as a land enters the battlefield.
///
/// This deliberately covers only replacement-style entry facts such as
/// "enters tapped". Any enter-the-battlefield triggered ability remains a
/// normal [`TriggeredAbilityBinding`], so it reaches the stack and exposes a
/// priority window instead of being folded into the land-play action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LandEntryBinding {
    pub card_definition: &'static str,
    pub enters_tapped: bool,
}

/// A deterministic, fixed bundle of mana produced by one mana ability.
///
/// The bundle deliberately uses one entry per color. This makes an activation
/// such as a Ravnica Signet's `{1}, {T}: add {U}{R}` distinct from an ability
/// that asks its controller to choose either blue or red mana. `new` preserves
/// the supplied amount for each color; the rules layer rejects zero entries or
/// an empty bundle when it validates a binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManaBundle {
    amounts: BTreeMap<Color, u8>,
}

impl ManaBundle {
    #[must_use]
    pub fn new(amounts: impl IntoIterator<Item = (Color, u8)>) -> Self {
        Self {
            amounts: amounts.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn amount(&self, color: Color) -> u8 {
        match self.amounts.get(&color) {
            Some(amount) => *amount,
            None => 0,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (Color, u8)> + '_ {
        self.amounts.iter().map(|(color, amount)| (*color, *amount))
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.amounts.is_empty()
    }
}

/// The mana produced by an activated mana ability.
///
/// `Choice` deliberately carries its legal choices instead of treating a
/// multi-color producer as a source of every color at once. The activating
/// player supplies one explicit choice for that variant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManaAbilityOutput {
    Fixed(Color),
    Choice(BTreeSet<Color>),
    /// Produces every entry of the fixed bundle as one non-stack mana-ability
    /// activation without paying mana as a cost. `amount` on the enclosing
    /// ability must be zero because the bundle carries the exact quantities.
    Bundle(ManaBundle),
    /// Pays the named mana cost, then produces every entry of the fixed bundle
    /// as one non-stack mana-ability activation. `amount` on the enclosing
    /// ability must be zero for this variant because the bundle carries the
    /// exact quantities itself.
    PaidBundle {
        mana_cost: ManaCost,
        bundle: ManaBundle,
    },
}

/// An expansion-neutral activated mana ability bound to a card definition.
///
/// This represents only the activation substrate: an optional tap cost, an
/// optional life-payment cost, an optional source-dealt controller-damage
/// result, and either a chosen/fixed mana quantity or a paid fixed bundle. It
/// does not encode card names or printed rules text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivatedManaAbility {
    /// Stable identifier unique within its bound card definition.
    pub id: &'static str,
    pub tap_cost: bool,
    pub output: ManaAbilityOutput,
    /// Positive mana quantity produced by a `Fixed` or `Choice` activation.
    /// It must be zero for `ManaAbilityOutput::PaidBundle`, whose quantities
    /// are carried by its `ManaBundle`.
    pub amount: u8,
    /// An optional, positive life payment made by the controller as a cost.
    pub life_payment: Option<u8>,
    /// Optional, positive damage dealt by this mana ability's source to its
    /// controller as the ability resolves without using the stack. Unlike a
    /// life payment, this is not a cost: it may reduce a player to zero life
    /// and is recorded as source-aware damage before state-based actions.
    pub controller_damage: Option<u8>,
}

/// Binds one generic mana ability to every permanent with a catalog definition.
///
/// Bindings are provided to `Game::new_with_mana_abilities`; keeping them
/// alongside, rather than inside, `CardDefinition` preserves the compact card
/// catalog API while allowing an expansion to opt into this shared substrate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManaAbilityBinding {
    pub card_definition: &'static str,
    pub ability: ActivatedManaAbility,
}

/// A non-mana activated ability bound to one expansion card definition.
///
/// The engine deliberately keeps this separate from `CardDefinition`, just as
/// it does for mana abilities: an expansion can opt into the stack substrate
/// without changing the catalog's compact identity schema. Costs are explicit
/// and paid before the ability is placed on the stack; effects resolve through
/// the same target-legality and priority machinery as spells.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivatedAbility {
    /// Stable identifier unique within its bound card definition.
    pub id: &'static str,
    pub mana_cost: ManaCost,
    pub tap_cost: bool,
    /// When true, activation is legal only while its controller is the active
    /// player in a main phase and the stack is empty. This belongs to the
    /// ability binding rather than a card-specific action path, so the same
    /// priority boundary remains reusable by later expansions.
    pub sorcery_speed: bool,
    /// Number of additional, distinct untapped creatures the activating
    /// player selects and taps as an ability cost.  These are not tap-symbol
    /// costs on those creatures, so summoning sickness does not constrain
    /// them.
    pub additional_tap_creatures: u8,
    pub sacrifice_source: bool,
    /// Number of controlled battlefield creatures required as an explicit
    /// sacrifice cost, selected in `AbilityActivation.sacrifice_sources`
    /// immediately after any required source sacrifice and before land costs.
    pub sacrifice_creatures: u8,
    /// Number of controlled battlefield lands required as an explicit cost.
    pub sacrifice_lands: u8,
    /// Number of cards the activating player must discard as an explicit cost.
    /// The concrete hand objects are selected in `AbilityActivation` so a
    /// policy cannot silently discard a hidden card or invent a cost payment.
    pub discard_cards: u8,
    /// Target slots are consumed in this order from `PolicyAction`.
    pub targets: Vec<TargetRequirement>,
    pub effects: Vec<Effect>,
}

/// Binds one stack-using activated ability to a card definition.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivatedAbilityBinding {
    pub card_definition: &'static str,
    pub ability: ActivatedAbility,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TriggerCondition {
    EntersBattlefield,
    /// A land entered the battlefield through a represented normal zone
    /// transition. Unlike `EntersBattlefield`, this condition observes every
    /// qualifying land entry rather than only the permanent that entered.
    LandEntersBattlefield,
    /// A land entered the battlefield under the source controller's control.
    /// This is intentionally distinct from the all-player land-entry
    /// condition so a controller-scoped landfall card cannot trigger from an
    /// opponent's land play.
    ControlledLandEntersBattlefield,
    /// The active player's upkeep began.  Only permanents they control are
    /// eligible; the resulting abilities are put on the stack before either
    /// player receives that upkeep's first priority.
    BeginningOfUpkeep,
    /// The source's controller gained positive life. The trigger is queued
    /// at the life-gain receipt and may optionally pay its bound mana cost
    /// before it is put on the stack.
    LifeGained,
    /// The source dealt positive damage to a player or permanent. The damage
    /// amount is materialized into the triggered stack object's effects when
    /// the receipt is emitted, so a life-gain trigger cannot inspect a later
    /// or unrelated damage event.
    DealsDamage,
    /// The source dealt positive combat damage to a creature. The triggering
    /// recipient is captured with its exact battlefield incarnation, so a
    /// later "that creature" instruction is not a free target choice.
    DealsCombatDamageToCreature,
    /// The source received positive damage. The source may leave the
    /// battlefield during state-based actions before this trigger is stacked.
    ReceivesDamage,
    /// The source changed from the battlefield to its graveyard.
    Dies,
    /// A different creature was put into a graveyard from the battlefield.
    /// The source is captured before the state-based-action batch removes
    /// either object, so simultaneous deaths retain their normal historical
    /// trigger provenance.
    AnotherCreatureDies,
    /// A card entered a graveyard owned by a player other than this source's
    /// current controller. The prior zone is intentionally unconstrained:
    /// discards, mills, destroyed permanents, countered spells, and costs all
    /// produce the same expansion-neutral observed event.
    OpponentCardPutIntoGraveyard,
    /// The source was declared as an attacker. A triggered optional mana cost
    /// is paid only on resolution, after the post-declaration priority window.
    Attacks,
    /// A noncreature spell was cast by this permanent's controller. The
    /// triggering stack item retains that exact spell as its target.
    CastsNoncreatureSpell,
}

/// A source-bound generic reduction applied while its permanent source is on
/// the battlefield. The reducer never changes colored symbols.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CostReductionBinding {
    pub source_definition: &'static str,
    pub generic_amount: u8,
    pub noncreature_only: bool,
}

/// A typed named counter carried by a live battlefield permanent.
///
/// Counter identity is rules data rather than a raw display string.  The
/// bounded built-ins cover common cross-expansion counters, while `Named`
/// lets an expansion introduce a genuinely new counter without adding a
/// card-name branch to the engine.  Counters on players, emblems, and cards
/// outside the battlefield deliberately remain outside this substrate.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CounterKind {
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
    /// A nonempty expansion-defined counter name that is not an alias for a
    /// supported built-in kind.
    Named(&'static str),
}

impl CounterKind {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::PlusOnePlusOne => "+1/+1",
            Self::MinusOneMinusOne => "-1/-1",
            Self::Charge => "charge",
            Self::Depletion => "depletion",
            Self::Doom => "doom",
            Self::Flood => "flood",
            Self::Lore => "lore",
            Self::Loyalty => "loyalty",
            Self::Spore => "spore",
            Self::Storage => "storage",
            Self::Time => "time",
            Self::Verse => "verse",
            Self::Named(name) => name,
        }
    }

    /// A named counter must have a real identity and cannot duplicate a
    /// built-in kind under a second representation. This keeps ordered maps
    /// and event receipts unambiguous.
    #[must_use]
    pub fn is_valid(self) -> bool {
        match self {
            Self::Named(name) => {
                !name.trim().is_empty()
                    && ![
                        "+1/+1",
                        "-1/-1",
                        "charge",
                        "depletion",
                        "doom",
                        "flood",
                        "lore",
                        "loyalty",
                        "spore",
                        "storage",
                        "time",
                        "verse",
                    ]
                    .contains(&name)
            }
            _ => true,
        }
    }
}

/// Whether an activated-cost calculation is for a mana ability or an ordinary
/// stack-using activated ability.
///
/// This is deliberately part of the cost context instead of an inference from
/// a card definition: a permanent may expose both kinds of activated ability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivatedAbilityKind {
    Mana,
    NonMana,
}

/// A source-bound generic adjustment to an activated ability's mana cost.
///
/// The initial substrate is intentionally limited to generic-symbol changes;
/// colored and hybrid requirements remain part of the base cost and can never
/// be removed by this binding.  Later expansions can add more adjustment
/// variants without teaching the engine about card names.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivatedAbilityCostModifier {
    IncreaseGeneric { amount: u8, nonmana_only: bool },
    ReduceGeneric { amount: u8, nonmana_only: bool },
}

impl ActivatedAbilityCostModifier {
    #[must_use]
    pub const fn generic_amount(self) -> u8 {
        match self {
            Self::IncreaseGeneric { amount, .. } | Self::ReduceGeneric { amount, .. } => amount,
        }
    }

    #[must_use]
    pub const fn applies_to(self, kind: ActivatedAbilityKind) -> bool {
        match self {
            Self::IncreaseGeneric { nonmana_only, .. }
            | Self::ReduceGeneric { nonmana_only, .. } => {
                !nonmana_only || matches!(kind, ActivatedAbilityKind::NonMana)
            }
        }
    }
}

/// Immutable expansion data that makes one live permanent modify activation
/// costs while it remains on the battlefield.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivatedAbilityCostModifierBinding {
    pub source_definition: &'static str,
    pub modifier: ActivatedAbilityCostModifier,
}

/// One live source contribution captured while calculating an activated
/// ability's mana cost.  The incarnation prevents a departed/re-entered
/// physical card from being conflated with the source that actually taxed or
/// reduced the activation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivatedAbilityCostAdjustment {
    pub source: ObjectId,
    pub source_incarnation: u64,
    pub generic_amount: u8,
}

/// The typed, auditable input and result of one activated-cost calculation.
///
/// `payment_selection` is optional because the legacy activation API has a
/// deterministic compatibility payment path.  New callers may provide an
/// explicit selection through the dedicated activation method; either form
/// still uses the same effective cost and transactional preflight.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivatedAbilityCostContext {
    pub acting_player: PlayerId,
    pub source: ObjectId,
    pub source_incarnation: u64,
    pub ability_id: &'static str,
    pub kind: ActivatedAbilityKind,
    pub base_mana_cost: ManaCost,
    pub increases: Vec<ActivatedAbilityCostAdjustment>,
    pub reductions: Vec<ActivatedAbilityCostAdjustment>,
    pub additional_tap_creatures: u8,
    pub sacrifice_source: bool,
    pub sacrifice_creatures: u8,
    pub sacrifice_lands: u8,
    pub discard_cards: u8,
    /// The policy-selected value of a bound activated ability's `{X}` cost.
    /// `base_mana_cost` already includes this amount because generic cost
    /// modifiers operate on the actual payable total.  Retaining it
    /// separately makes the printed-cost provenance auditable.
    pub chosen_x: Option<u8>,
    pub payment_selection: Option<ManaPaymentSelection>,
    pub effective_mana_cost: ManaCost,
}

/// Which permanent supplies one named-counter removal in an activated cost.
///
/// The source-relative form handles costs such as "remove a charge counter
/// from this" without allowing the policy to substitute another object.  The
/// selected form is intentionally controller-relative and receives its exact
/// public battlefield object through [`AbilityCostPayment`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivatedCounterCostTarget {
    Source,
    SelectedControlledPermanent,
}

/// One positive named-counter removal required to activate an ability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivatedCounterCost {
    pub target: ActivatedCounterCostTarget,
    pub counter: CounterKind,
    pub amount: i16,
}

/// Expansion-owned, opt-in cost data layered on top of a compact
/// [`ActivatedAbility`] binding.
///
/// Existing ability bindings retain their original shape.  An expansion that
/// needs richer costs registers this immutable profile before the game starts,
/// keeping card data separate from policy-supplied concrete objects.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GeneralizedActivatedAbilityCost {
    /// Positive life paid as a cost. Zero means this profile has no life cost.
    pub life_payment: u8,
    /// Ordered named-counter removals.  Every entry receives one aligned
    /// object choice in [`AbilityCostPayment::counter_sources`], including a
    /// source-relative entry (which must name the ability source).
    pub counter_removals: Vec<ActivatedCounterCost>,
    /// Return the ability source to its owner's hand as part of the cost.
    pub return_source_to_hand: bool,
    /// Number of additional controlled battlefield permanents that must be
    /// selected and returned to their owners' hands as part of the cost.
    pub return_controlled_permanents: u8,
    /// Whether this ability has one player-chosen nonnegative `{X}` generic
    /// symbol in addition to its bound printed mana cost.
    pub has_x_cost: bool,
}

impl GeneralizedActivatedAbilityCost {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.life_payment == 0
            && self.counter_removals.is_empty()
            && !self.return_source_to_hand
            && self.return_controlled_permanents == 0
            && !self.has_x_cost
    }
}

/// Connects an immutable generalized cost profile to one existing activated
/// ability. The tuple is unique within a game catalog.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActivatedAbilityCostBinding {
    pub card_definition: &'static str,
    pub ability_id: &'static str,
    pub cost: GeneralizedActivatedAbilityCost,
}

/// Concrete, policy-submitted selections required by an activated ability's
/// generalized cost profile.  These are supplied at the normal priority
/// action, rather than through a new pending-decision state, because paying an
/// activation cost is one atomic player action rather than a resolution-time
/// no-priority continuation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AbilityCostPayment {
    pub counter_sources: Vec<ObjectId>,
    pub return_permanents: Vec<ObjectId>,
    pub chosen_x: Option<u8>,
}

/// A normal ability activation plus its typed generalized-cost selections.
///
/// `mana_payment_selection` deliberately reuses the engine's ordinary
/// generic/hybrid allocation model instead of introducing a second payment
/// representation.  It is one component of this same priority action: the
/// engine preflights it together with every life, counter, return, discard,
/// tap, sacrifice, and X component before any cost mutation is committed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralizedAbilityActivation {
    pub activation: AbilityActivation,
    pub cost_payment: AbilityCostPayment,
    pub mana_payment_selection: Option<ManaPaymentSelection>,
}

/// A replacement event quantity that can be modified by a live permanent.
///
/// The event kind is deliberately semantic instead of card-named. Future sets
/// can add more replacement event kinds without coupling their definitions to
/// a particular existing replacement card.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplacementEventKind {
    TokenCreation,
    CounterPlacement { counter: CounterKind },
}

/// A source-bound replacement effect whose applicability is checked from the
/// live battlefield when an event would occur.
///
/// Each active source applies at most once to one pending event. The engine
/// snapshots applicable sources before it changes the quantity, preventing a
/// replacement result from recursively becoming another application of that
/// same source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplacementEffect {
    MultiplyTokenCreation { multiplier: u8 },
    MultiplyCounterPlacement { multiplier: u8 },
}

impl ReplacementEffect {
    #[must_use]
    pub const fn multiplier(self) -> u8 {
        match self {
            Self::MultiplyTokenCreation { multiplier }
            | Self::MultiplyCounterPlacement { multiplier } => multiplier,
        }
    }

    #[must_use]
    pub const fn applies_to(self, event: ReplacementEventKind) -> bool {
        matches!(
            (self, event),
            (
                Self::MultiplyTokenCreation { .. },
                ReplacementEventKind::TokenCreation
            ) | (
                Self::MultiplyCounterPlacement { .. },
                ReplacementEventKind::CounterPlacement { .. }
            )
        )
    }
}

/// Registers one expansion-owned replacement effect for every live permanent
/// with the named definition. Registration is immutable after the game starts;
/// controller and battlefield membership remain live applicability facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReplacementEffectBinding {
    pub source_definition: &'static str,
    pub effect: ReplacementEffect,
}

/// One currently applicable way to replace a prospective damage event.
///
/// This intentionally names only the bounded damage replacement substrate:
/// target-specific shields, permanent-local shields/protection, and the
/// existing redirection effect.  The identity is fully serializable and is
/// revalidated when the affected player submits it, so a policy cannot apply
/// a stale or fabricated replacement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DamageReplacementChoice {
    /// Redirect all of the bounded prospective event from `protected` to the
    /// already-selected destination.
    Redirect {
        id: u64,
        source: ObjectId,
        protected: ObjectId,
        destination: Target,
    },
    /// Consume a target-specific prevention shield.
    TargetedShield {
        id: u64,
        source: ObjectId,
        target: Target,
    },
    /// Consume a permanent's legacy, continuous-effect-backed damage shield.
    PermanentShield { permanent: ObjectId },
    /// Apply protection or a color-specific prevention keyword on the named
    /// permanent.  The source color is rechecked when selected.
    SourceColorPrevention { permanent: ObjectId },
}

/// One currently applicable replacement in the shared prospective-event
/// chain.  Quantity replacements retain both the live source incarnation and
/// immutable bound effect; a physical card that leaves and returns is a new
/// source and cannot be mistaken for an already-used replacement.
///
/// Damage continues to expose its compatibility choice shape while its
/// bounded resolver is migrated incrementally.  Keeping both identities in
/// this serializable enum makes the generic decision surface reusable by
/// prevention, redirection, token, and counter replacement paths.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplacementChoice {
    Quantity {
        source: ObjectId,
        source_incarnation: u64,
        effect: ReplacementEffect,
    },
    Damage(DamageReplacementChoice),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TriggeredAbility {
    pub id: &'static str,
    pub condition: TriggerCondition,
    /// Optional mana paid while the trigger resolves. The trigger must first
    /// reach the stack so its controller receives the ordinary response and
    /// mana-ability window.
    pub mana_cost: ManaCost,
    pub optional: bool,
    pub targets: Vec<TargetRequirement>,
    pub effects: Vec<Effect>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TriggeredAbilityBinding {
    pub card_definition: &'static str,
    pub ability: TriggeredAbility,
}

/// A player's explicit request to activate a stack-using ability.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AbilityActivation {
    pub source: ObjectId,
    pub ability_id: &'static str,
    /// Explicit permanent selections paid as the ability's nonmana cost.
    pub sacrifice_sources: Vec<ObjectId>,
    /// Explicit distinct controlled creatures tapped as a nonmana ability
    /// cost, separate from the ability source's own tap-symbol cost.
    pub additional_tap_creatures: Vec<ObjectId>,
    /// Explicit hand-card selections paid as the ability's discard cost.
    pub discard_cards: Vec<ObjectId>,
    pub targets: Vec<Target>,
}

/// A player's request to activate a bound mana ability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManaAbilityActivation {
    pub source: ObjectId,
    pub ability_id: &'static str,
    /// Required for `ManaAbilityOutput::Choice` and absent for `Fixed` output.
    pub chosen_color: Option<Color>,
}

/// A player's explicit request to use a typed basic land's intrinsic mana
/// ability while paying one spell cost.
///
/// The selected color is kept in the request so the cast transaction can
/// validate the type-to-color binding before it taps the named land. This is
/// separate from definition-bound mana abilities because basic land mana is a
/// rules-derived intrinsic ability, not catalog card text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BasicLandManaAbilityActivation {
    pub land: ObjectId,
    pub color: Color,
}

/// One ordered mana ability used during a spell's cost-payment transaction.
///
/// Each request is intentionally explicit: the engine never selects a mana
/// source or color on the policy's behalf.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CastPaymentManaAbility {
    Bound(ManaAbilityActivation),
    BasicLand(BasicLandManaAbilityActivation),
}

impl Color {
    /// Magic's five actual card colors. This intentionally excludes the
    /// colorless mana kind, so effects such as "choose a color" and Birds of
    /// Paradise cannot select it.
    pub const ALL: [Self; 5] = [Self::White, Self::Blue, Self::Black, Self::Red, Self::Green];

    /// Every represented mana kind. The ordering spends colorless mana first
    /// for deterministic generic-payment compatibility while preserving all
    /// preexisting colored-only behavior when no colorless mana exists.
    pub const MANA_ALL: [Self; 6] = [
        Self::Colorless,
        Self::White,
        Self::Blue,
        Self::Black,
        Self::Red,
        Self::Green,
    ];

    #[must_use]
    pub const fn is_colored(self) -> bool {
        !matches!(self, Self::Colorless)
    }

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::White => 0,
            Self::Blue => 1,
            Self::Black => 2,
            Self::Red => 3,
            Self::Green => 4,
            Self::Colorless => 5,
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CardType {
    Artifact,
    Creature,
    Land,
    Enchantment,
    Instant,
    Planeswalker,
    Sorcery,
}

#[derive(Clone, Eq, PartialEq)]
pub struct ManaCost {
    pub generic: u8,
    pub colored: Vec<Color>,
    /// Each entry is one colored symbol payable with either listed color.
    /// Keeping alternatives explicit prevents a hybrid symbol from being
    /// silently treated as two mandatory colored requirements.
    pub hybrid: Vec<HybridManaSymbol>,
}

/// The controller's explicit color choices for mana symbols whose color is
/// not fixed by the printed cost. `generic` is in generic-symbol order and
/// `hybrid` is in hybrid-symbol order after any Convoke contributions have
/// reduced the cost. Each vector must account for the remaining symbols
/// exactly; omission is not an engine-selected fallback.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ManaPaymentSelection {
    pub generic: Vec<Color>,
    pub hybrid: Vec<Color>,
}

impl std::fmt::Debug for ManaCost {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug = formatter.debug_struct("ManaCost");
        debug
            .field("generic", &self.generic)
            .field("colored", &self.colored);
        // Existing canonical event traces include ManaCost debug output. Keep
        // the established representation byte-for-byte when no hybrid symbol
        // is present, while exposing hybrid data for new traces.
        if !self.hybrid.is_empty() {
            debug.field("hybrid", &self.hybrid);
        }
        debug.finish()
    }
}

/// One two-color hybrid mana symbol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HybridManaSymbol {
    pub first: Color,
    pub second: Color,
}

impl ManaCost {
    #[must_use]
    pub const fn new(generic: u8) -> Self {
        Self {
            generic,
            colored: Vec::new(),
            hybrid: Vec::new(),
        }
    }

    pub fn with_colors(generic: u8, colored: impl IntoIterator<Item = Color>) -> Self {
        Self {
            generic,
            colored: colored.into_iter().collect(),
            hybrid: Vec::new(),
        }
    }

    /// Builds a cost with ordinary colored symbols and explicit two-color
    /// hybrid choices. Each pair consumes exactly one mana from either color.
    #[must_use]
    pub fn with_hybrid(
        generic: u8,
        colored: impl IntoIterator<Item = Color>,
        hybrid: impl IntoIterator<Item = HybridManaSymbol>,
    ) -> Self {
        Self {
            generic,
            colored: colored.into_iter().collect(),
            hybrid: hybrid.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn mana_value(&self) -> u8 {
        self.generic
            .saturating_add(u8::try_from(self.colored.len()).unwrap_or(u8::MAX))
            .saturating_add(u8::try_from(self.hybrid.len()).unwrap_or(u8::MAX))
    }
}

/// A deterministic six-kind mana pool: the five card colors plus colorless.
/// It intentionally has no floating mana source.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ManaPool {
    amounts: [u8; 6],
}

impl ManaPool {
    /// Whether this bounded compatibility pool can represent the requested addition.
    #[must_use]
    pub const fn can_add(&self, color: Color, amount: u8) -> bool {
        amount <= u8::MAX.saturating_sub(self.amount(color))
    }

    pub fn add(&mut self, color: Color, amount: u8) {
        self.amounts[color.index()] = self.amounts[color.index()].saturating_add(amount);
    }

    #[must_use]
    pub const fn amount(&self, color: Color) -> u8 {
        self.amounts[color.index()]
    }

    #[must_use]
    pub fn total(&self) -> u8 {
        u8::try_from(self.total_exact()).unwrap_or(u8::MAX)
    }

    /// Returns the complete pool total widened enough for every mana slot.
    ///
    /// `total` remains a bounded `u8` compatibility view for policy heuristics;
    /// payment code must use this exact value so valid large pools cannot
    /// overflow while evaluating a generic cost.
    #[must_use]
    pub fn total_exact(&self) -> u16 {
        self.amounts.iter().map(|amount| u16::from(*amount)).sum()
    }

    /// Empties floating mana at a step or phase boundary.
    pub fn clear(&mut self) {
        self.amounts = [0; 6];
    }

    pub(crate) fn pay(&mut self, cost: &ManaCost) -> Result<(), String> {
        let mut paid = self.clone();
        paid.pay_in_place(cost)?;
        *self = paid;
        Ok(())
    }

    /// Pays a cost using the controller's explicit choices for every generic
    /// and hybrid symbol, returning the complete ordered color receipt. Fixed
    /// colored symbols are represented first in their cost order, followed by
    /// hybrid choices and then generic choices.
    ///
    /// This is separate from [`Self::pay`], whose deterministic compatibility
    /// order is intentionally not a player decision and therefore cannot
    /// support cards that inspect colors spent to cast them.
    pub(crate) fn pay_selected(
        &mut self,
        cost: &ManaCost,
        selection: &ManaPaymentSelection,
    ) -> Result<Vec<Color>, String> {
        if selection.generic.len() != usize::from(cost.generic) {
            return Err("generic mana selection does not match the remaining cost".to_owned());
        }
        if selection.hybrid.len() != cost.hybrid.len() {
            return Err("hybrid mana selection does not match the remaining cost".to_owned());
        }

        let mut paid = self.clone();
        let mut colors = Vec::with_capacity(
            cost.colored.len() + selection.hybrid.len() + selection.generic.len(),
        );
        for color in &cost.colored {
            if paid.amount(*color) == 0 {
                return Err(format!("missing {color:?} mana"));
            }
            paid.amounts[color.index()] -= 1;
            colors.push(*color);
        }
        for (symbol, color) in cost.hybrid.iter().zip(&selection.hybrid) {
            if *color != symbol.first && *color != symbol.second {
                return Err("selected color cannot pay that hybrid symbol".to_owned());
            }
            if paid.amount(*color) == 0 {
                return Err(format!("missing {color:?} mana"));
            }
            paid.amounts[color.index()] -= 1;
            colors.push(*color);
        }
        for color in &selection.generic {
            if paid.amount(*color) == 0 {
                return Err(format!("missing {color:?} mana"));
            }
            paid.amounts[color.index()] -= 1;
            colors.push(*color);
        }
        *self = paid;
        Ok(colors)
    }

    /// Internal payment mutation after `pay` has made the caller's pool
    /// transactional. A missing later symbol must never leave an earlier
    /// colored or hybrid debit behind.
    fn pay_in_place(&mut self, cost: &ManaCost) -> Result<(), String> {
        // A cost may repeat one colored symbol more often than this bounded
        // pool can represent. Count in a widened type so the requirement never
        // saturates into a cheaper payable cost.
        let mut required = [0_u16; 6];
        for color in &cost.colored {
            required[color.index()] = required[color.index()].saturating_add(1);
        }
        for color in Color::MANA_ALL {
            if u16::from(self.amount(color)) < required[color.index()] {
                return Err(format!("missing {color:?} mana"));
            }
        }
        for color in Color::MANA_ALL {
            let spent = u8::try_from(required[color.index()])
                .expect("a payable bounded colored cost fits its source pool");
            self.amounts[color.index()] -= spent;
        }
        self.pay_hybrid_symbols(&cost.hybrid)?;
        if self.total_exact() < u16::from(cost.generic) {
            return Err("missing generic mana".to_owned());
        }
        let mut remaining = cost.generic;
        for color in Color::MANA_ALL {
            let spent = self.amount(color).min(remaining);
            self.amounts[color.index()] -= spent;
            remaining -= spent;
            if remaining == 0 {
                break;
            }
        }
        Ok(())
    }

    /// Pays every hybrid symbol through a capacity-aware matching pass. A
    /// greedy left-to-right choice would reject a payable cost such as
    /// `{W/U}{W/R}` from `{W}{U}`; augmenting prior choices keeps payment
    /// order-independent while preserving the bounded mana-pool boundary.
    fn pay_hybrid_symbols(&mut self, symbols: &[HybridManaSymbol]) -> Result<(), String> {
        let mut remaining = self.amounts.map(u16::from);
        let mut assignments = vec![None; symbols.len()];
        for index in 0..symbols.len() {
            let mut seen_colors = [false; 6];
            let mut seen_symbols = vec![false; symbols.len()];
            if !Self::assign_hybrid_symbol(
                index,
                symbols,
                &mut assignments,
                &mut remaining,
                &mut seen_colors,
                &mut seen_symbols,
            ) {
                return Err("missing hybrid mana".to_owned());
            }
        }
        for color in Color::MANA_ALL {
            self.amounts[color.index()] = u8::try_from(remaining[color.index()])
                .expect("hybrid payment cannot increase a bounded mana pool");
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn assign_hybrid_symbol(
        index: usize,
        symbols: &[HybridManaSymbol],
        assignments: &mut [Option<Color>],
        remaining: &mut [u16; 6],
        seen_colors: &mut [bool; 6],
        seen_symbols: &mut [bool],
    ) -> bool {
        seen_symbols[index] = true;
        for color in [symbols[index].first, symbols[index].second] {
            let color_index = color.index();
            if seen_colors[color_index] {
                continue;
            }
            seen_colors[color_index] = true;
            if remaining[color_index] > 0 {
                remaining[color_index] -= 1;
                assignments[index] = Some(color);
                return true;
            }
            for occupant in 0..assignments.len() {
                if assignments[occupant] != Some(color) || seen_symbols[occupant] {
                    continue;
                }
                assignments[occupant] = None;
                remaining[color_index] += 1;
                if Self::assign_hybrid_symbol(
                    occupant,
                    symbols,
                    assignments,
                    remaining,
                    seen_colors,
                    seen_symbols,
                ) {
                    remaining[color_index] -= 1;
                    assignments[index] = Some(color);
                    return true;
                }
                remaining[color_index] -= 1;
                assignments[occupant] = Some(color);
            }
        }
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Keyword {
    Convoke,
    Defender,
    /// Can be blocked only by a creature with Flying or Reach.
    Flying,
    /// Can be blocked only by a black or artifact creature.
    Fear,
    /// Can be blocked only by a black creature.
    BlackEvasion,
    /// This creature cannot be declared as blocked. The game records the
    /// attacker's declaration-time evasion provenance so a later continuous
    /// effect cannot retroactively legalize an already-illegal block.
    Unblockable,
    FirstStrike,
    /// This creature can attack and pay a tap cost on the turn it entered
    /// under its controller's control.
    Haste,
    /// If able, this creature must be assigned at least one blocker when it
    /// attacks. The combat declaration path enforces the restriction after
    /// all blockers have been submitted.
    MustBeBlockedIfAble,
    /// This creature cannot be declared as an attacker or blocker for the
    /// current turn. It is used by temporary combat-restriction effects.
    CannotAttackOrBlock,
    /// This creature cannot be declared as a blocker for the current turn.
    /// Unlike `CannotAttackOrBlock`, the creature remains eligible to attack.
    CannotBlock,
    /// This creature can block only while its controller controls a Mountain.
    CannotBlockUnlessControlsMountain,
    /// A controller-wide static restriction: Saproling creatures that player
    /// controls cannot be declared as blockers while this source remains on
    /// the battlefield.
    SaprolingsCannotBlock,
    /// This creature can't be blocked while the defending player controls the
    /// named basic land type.
    Mountainwalk,
    /// This creature can't be blocked while the defending player controls a
    /// land with the named basic land type. This generalizes the historical
    /// `Mountainwalk` compatibility variant without forcing a card to use a
    /// one-off keyword for every land type.
    Landwalk(BasicLandType),
    /// This creature assigns combat damage in both first-strike and normal
    /// combat-damage steps.
    DoubleStrike,
    /// Any positive damage from this source is lethal to a creature for both
    /// combat assignment and state-based actions. Marked-damage provenance is
    /// retained separately, so a later keyword change cannot rewrite damage
    /// that was already dealt.
    Deathtouch,
    /// Damage dealt by this source cannot be prevented. This does not disable
    /// non-prevention replacement effects (for example, redirection).
    DamageCannotBePrevented,
    /// Damage dealt by a source of the named color is prevented when it would
    /// be dealt to this permanent.
    PreventDamageFromColor(Color),
    /// This permanent has protection from sources of the named color. The
    /// engine's core protection substrate will enforce targeting, combat,
    /// and damage-prevention consequences for this keyword.
    Protection(Color),
    /// Can block a creature with Flying.
    Reach,
    /// When blocked, excess combat damage can be assigned to the defending
    /// player after lethal damage has been assigned to each blocker.
    Trample,
    /// Declaring this creature as an attacker does not tap it.
    Vigilance,
    Dredge(u8),
    Transmute(ManaCost),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetRequirement {
    Any,
    /// Any current battlefield permanent. This is distinct from `Any`, whose
    /// compatibility use can include other target kinds in future slices.
    Permanent,
    Creature,
    /// A battlefield creature whose current characteristics do not include
    /// Black. This is a target restriction rather than a resolution-only
    /// filter, so illegal black creatures are rejected before any spell cost
    /// or stack state changes.
    NonblackCreature,
    /// A battlefield creature whose current characteristics include Flying.
    /// This stays distinct from a generic creature target so an activation
    /// such as Elvish Skysweeper's is rejected before costs are paid.
    FlyingCreature,
    /// A battlefield creature that must be distinct from every other
    /// `DistinctCreature` target occurrence in one spell.
    DistinctCreature,
    /// A battlefield creature currently assigned as a blocker in the active
    /// combat. This preserves the narrower target restriction of sacrifice
    /// damage abilities such as War-Torch Goblin.
    BlockingCreature,
    /// A battlefield creature currently declared as an attacker or blocker
    /// in the active combat. This preserves the targeted-combat restriction
    /// of Devouring Light without weakening generic creature-exile effects.
    AttackingOrBlockingCreature,
    Land,
    /// A battlefield land controlled by the resolving source's controller.
    /// This is distinct from `Land` so source-relative return effects cannot
    /// silently accept an opponent's land.
    ControlledLand,
    /// A battlefield permanent with the Artifact card type.
    Artifact,
    /// A battlefield permanent with the Enchantment card type. This stays
    /// separate from Artifact-or-Enchantment so a Radiance spell can preserve
    /// its narrower printed target boundary at cast and resolution.
    Enchantment,
    /// A battlefield permanent with either the Artifact or Creature card
    /// type. This keeps targeted destruction from accepting an arbitrary
    /// nonland permanent.
    ArtifactOrCreature,
    /// A battlefield permanent with either the Artifact or Enchantment card
    /// type. This keeps the Sundering Vitae target boundary explicit rather
    /// than treating every noncreature permanent as a legal target.
    ArtifactOrEnchantment,
    Player,
    /// A living player different from the resolving source's controller.
    /// Keeping this distinct from `Player` makes the target boundary visible
    /// before an activated ability accepts costs or reaches the stack.
    Opponent,
    /// A player or battlefield creature, matching the executable pre-
    /// planeswalker direct-damage card slice.
    PlayerOrCreature,
    /// A nonpermanent spell card currently on the stack. This deliberately
    /// names the narrow RAV counterspell slice instead of claiming support for
    /// arbitrary abilities or every kind of spell target.
    InstantOrSorcerySpell,
    /// Any noncreature spell currently on the stack, including represented
    /// permanent artifact and enchantment spells.
    NoncreatureSpell,
    /// A card in the resolving spell controller's graveyard. The controller
    /// qualification stays in `Game` so this target remains reusable by other
    /// expansions.
    OwnGraveyardCard,
    /// A creature card in the resolving source controller's graveyard. This
    /// keeps graveyard-recursion effects from accepting an arbitrary spell or
    /// land card merely because it shares the controller's graveyard.
    CreatureCardInControllerGraveyard,
    /// An enchantment card in the resolving source controller's graveyard.
    /// This keeps enchantment-only recursion from accepting another permanent
    /// card type merely because it is owned by that controller.
    EnchantmentCardInControllerGraveyard,
    /// An instant or sorcery card in the casting player's graveyard.
    InstantOrSorceryCardInControllerGraveyard,
    /// An instant or sorcery card in the casting player's exile zone. This is
    /// intentionally distinct from a graveyard card so an effect-created
    /// casting permission cannot silently authorize a card in the wrong
    /// public zone.
    InstantOrSorceryCardInControllerExile,
    /// A battlefield creature controlled by the resolving spell's controller.
    ControlledCreature,
    /// A battlefield creature controlled by a different player from the
    /// resolving spell's controller.
    OpponentCreature,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Target {
    Player(PlayerId),
    Permanent(ObjectId),
    /// The card object representing an instant or sorcery spell on the stack.
    /// `ObjectId` remains stable while the card changes zones, so the engine
    /// verifies that it is still a qualifying spell when the effect resolves.
    Spell(ObjectId),
    /// An explicitly selected permanent used to pay a spell's bound additional
    /// sacrifice cost. This is intentionally not a spell target: it is removed
    /// from the request before the spell is placed on the stack and it never
    /// occupies a `StackObject` target slot.
    SacrificePermanent(ObjectId),
}

/// A semantic additional cost bound by an expansion to a spell definition.
///
/// The initial substrate represents the common "sacrifice a creature" cost.
/// It remains separate from effects and targets: a legal cast pays it before
/// its card becomes a stack object, and an atomic cast rollback restores the
/// permanent if any later cost cannot be paid.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AdditionalSpellCost {
    SacrificeControlledCreature,
}

/// Binds an expansion-neutral additional spell cost to one card definition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdditionalSpellCostBinding {
    pub card_definition: &'static str,
    pub cost: AdditionalSpellCost,
}

/// A creature subtype carried by a token's type line.
///
/// The initial RAV substrate needs only a small set of token subtypes, but
/// this remains a typed semantic field rather than treating a display name as
/// a rules identity.
/// Future set modules can extend the enum as they introduce token-specific
/// interactions.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CreatureSubtype {
    Centaur,
    Faerie,
    Goblin,
    Horror,
    Knight,
    Saproling,
    Spirit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenSpec {
    pub name: &'static str,
    pub colors: BTreeSet<Color>,
    pub card_types: BTreeSet<CardType>,
    /// Creature subtypes are mechanically distinct from a token's display
    /// name. An empty set is valid for a noncreature token or a token whose
    /// represented slice intentionally has no subtype.
    pub creature_subtypes: BTreeSet<CreatureSubtype>,
    pub keywords: Vec<Keyword>,
    pub power: i16,
    pub toughness: i16,
}

impl TokenSpec {
    #[must_use]
    pub fn saproling() -> Self {
        Self {
            name: "Saproling",
            colors: BTreeSet::from([Color::Green]),
            card_types: BTreeSet::from([CardType::Creature]),
            creature_subtypes: BTreeSet::from([CreatureSubtype::Saproling]),
            keywords: vec![],
            power: 1,
            toughness: 1,
        }
    }

    #[must_use]
    pub fn horror() -> Self {
        Self {
            name: "Horror",
            colors: BTreeSet::from([Color::Black]),
            card_types: BTreeSet::from([CardType::Creature]),
            creature_subtypes: BTreeSet::from([CreatureSubtype::Horror]),
            keywords: vec![],
            power: 4,
            toughness: 4,
        }
    }

    #[must_use]
    pub fn knight() -> Self {
        Self {
            name: "Knight",
            colors: BTreeSet::from([Color::White]),
            card_types: BTreeSet::from([CardType::Creature]),
            creature_subtypes: BTreeSet::from([CreatureSubtype::Knight]),
            keywords: vec![Keyword::FirstStrike],
            power: 2,
            toughness: 2,
        }
    }

    #[must_use]
    pub fn green_centaur() -> Self {
        Self {
            name: "Centaur",
            colors: BTreeSet::from([Color::Green]),
            card_types: BTreeSet::from([CardType::Creature]),
            creature_subtypes: BTreeSet::from([CreatureSubtype::Centaur]),
            keywords: vec![],
            power: 3,
            toughness: 3,
        }
    }

    #[must_use]
    pub fn hunted_centaur() -> Self {
        Self {
            name: "Centaur",
            colors: BTreeSet::from([Color::Green]),
            card_types: BTreeSet::from([CardType::Creature]),
            creature_subtypes: BTreeSet::from([CreatureSubtype::Centaur]),
            keywords: vec![Keyword::Protection(Color::Black)],
            power: 3,
            toughness: 3,
        }
    }

    #[must_use]
    pub fn blue_faerie() -> Self {
        Self {
            name: "Faerie",
            colors: BTreeSet::from([Color::Blue]),
            card_types: BTreeSet::from([CardType::Creature]),
            creature_subtypes: BTreeSet::from([CreatureSubtype::Faerie]),
            keywords: vec![Keyword::Flying],
            power: 1,
            toughness: 1,
        }
    }

    #[must_use]
    pub fn white_spirit() -> Self {
        Self {
            name: "Spirit",
            colors: BTreeSet::from([Color::White]),
            card_types: BTreeSet::from([CardType::Creature]),
            creature_subtypes: BTreeSet::from([CreatureSubtype::Spirit]),
            keywords: vec![Keyword::Flying],
            power: 1,
            toughness: 1,
        }
    }

    #[must_use]
    pub fn red_goblin() -> Self {
        Self {
            name: "Goblin",
            colors: BTreeSet::from([Color::Red]),
            card_types: BTreeSet::from([CardType::Creature]),
            creature_subtypes: BTreeSet::from([CreatureSubtype::Goblin]),
            keywords: vec![],
            power: 1,
            toughness: 1,
        }
    }
}

/// Effects are executable semantics, not copied Oracle wording.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    DealDamage {
        amount: i16,
        target: TargetRequirement,
    },
    /// Apply a layer-two control effect to one target permanent through the
    /// current turn's cleanup step. The object never changes zones: its owner
    /// remains authoritative for every nonbattlefield destination.
    GainControlTargetUntilEndOfTurn,
    /// Make one targeted player lose life without dealing damage. Prevention,
    /// redirection, and damage triggers therefore do not apply.
    LoseLifeTarget {
        amount: i16,
    },
    /// Make the resolving ability's controller lose life without dealing
    /// damage. Unlike `LoseLifeTarget`, this has no target slot.
    LoseLifeController {
        amount: i16,
    },
    /// Each living opponent loses life equal to the number of creatures that
    /// opponent controls as the instruction resolves. Every opponent's count
    /// is independently live, not captured when the ability was triggered.
    LoseLifeEachOpponentEqualToControlledCreatures,
    /// Each living player discards one card selected at the trigger-resolution
    /// decision boundary. Every resulting discard and zone move remains an
    /// explicit event-log receipt.
    DiscardOneCardEachPlayer,
    /// Discard up to the requested number of cards from a target player's
    /// hand. Until a discard-choice policy action exists, the resolver selects
    /// that player's oldest hand entries deterministically and records every
    /// discard plus zone movement.
    DiscardTargetPlayer {
        count: u8,
    },
    /// Sacrifice one creature controlled by the resolving source's controller.
    /// The controller selects the permanent at the trigger-resolution
    /// decision boundary.
    SacrificeControllerCreature,
    /// Put a positive, already materialized number of cards from one target
    /// player's library into that player's graveyard.
    MillTargetPlayer {
        count: i16,
    },
    /// A recipient-damage trigger materializes this into
    /// [`Self::MillTargetPlayer`] when the damage event is queued. Keeping the
    /// event amount out of the card binding prevents a later resolution from
    /// inspecting unrelated or stale damage.
    MillTargetPlayerFromSourceDamage,
    /// A recipient-damage trigger materializes this into
    /// [`Self::MillCapturedPlayer`] using the controller of the damage
    /// source. It has no target slot in the card binding.
    MillSourceControllerFromSourceDamage,
    /// A recipient-damage trigger materializes this into a captured-player
    /// mill instruction. The player is the controller of the source that
    /// dealt the damage, not the controller of the recipient trigger source.
    /// Keeping the player in the resolved effect preserves that relationship
    /// across source and recipient zone changes without opening a target
    /// decision.
    MillCapturedPlayer {
        player: PlayerId,
        count: i16,
    },
    /// Deal damage to one target equal to the number of creatures controlled
    /// by this spell's controller that are still attacking as it resolves.
    ///
    /// The combat selection is resolution-time, so a creature that has left
    /// the battlefield does not contribute and casting before attackers have
    /// been declared deals no damage rather than inspecting a stale board.
    DealDamageEqualToAttackingCreatures {
        target: TargetRequirement,
    },
    DealDamageController {
        amount: i16,
    },
    /// Pay an optional life-gain trigger cost at resolution, then deal the
    /// fixed amount to the policy-submitted conditional target. The target is
    /// intentionally not an initial stack slot because it is selected only
    /// after the optional payment succeeds.
    DealDamageAfterOptionalManaPayment {
        amount: i16,
        target: TargetRequirement,
    },
    /// Add a fixed amount of one color to the resolving spell controller's
    /// mana pool. This is a stack effect (not a mana ability), used by
    /// Seismic Spike after its targeted land destruction resolves.
    AddManaController {
        color: Color,
        amount: u8,
    },
    /// Place one persistent +1/+1 counter on the ability or spell source if
    /// that source is still a battlefield permanent as this instruction
    /// resolves. A departed source is an ordinary no-op, not a failed trigger
    /// resolution; only a creature source receives the derived P/T modifier.
    AddPlusOneCounterToSource,
    /// Place one persistent +1/+1 counter on a targeted creature. The source
    /// remains provenance only and may have left the battlefield as an
    /// activation cost before this instruction resolves.
    AddPlusOneCounterToTarget,
    /// Place a positive quantity of one typed counter on the resolving source
    /// while it remains a battlefield permanent. This is not creature-only;
    /// only `+1/+1` and `-1/-1` affect derived power and toughness.
    AddCountersToSource {
        counter: CounterKind,
        amount: i16,
    },
    /// Place a positive quantity of one typed counter on a targeted live
    /// battlefield permanent. Quantity replacement applies before the state
    /// mutation and receipt.
    AddCountersToTarget {
        counter: CounterKind,
        amount: i16,
    },
    /// Remove a positive quantity of one typed counter from the resolving
    /// source. Insufficient counters reject the whole atomic resolution.
    RemoveCountersFromSource {
        counter: CounterKind,
        amount: i16,
    },
    /// Remove a positive quantity of one typed counter from a targeted live
    /// battlefield permanent. Insufficient counters reject the whole atomic
    /// resolution rather than producing a partial removal.
    RemoveCountersFromTarget {
        counter: CounterKind,
        amount: i16,
    },
    /// Deal one fixed amount of damage to every creature currently on the
    /// battlefield and every player still in the game. This selection is made
    /// once while the spell resolves; state-based actions run only after the
    /// complete batch has received damage.
    DealDamageToEachCreatureAndPlayer {
        amount: i16,
    },
    /// Deal a fixed amount to each surviving player, without affecting
    /// creatures. This remains distinct from the all-creature batch so
    /// recipient damage and state-based actions are auditable.
    DealDamageToEachPlayer {
        amount: i16,
    },
    /// Deal fixed damage to every current creature whose characteristics do
    /// not include Flying. The affected set is snapshotted at resolution.
    DealDamageToEachNonFlyingCreature {
        amount: i16,
    },
    /// Deal damage to the targeted creature and every creature that shares at
    /// least one of its colors. The target remains included even if it has no
    /// colors, matching the shared radiance selection substrate.
    RadianceDealDamageToCreatures {
        amount: i16,
    },
    GainLifeController {
        amount: i16,
    },
    /// Gain life equal to the number of creatures currently on the
    /// battlefield when the instruction resolves. The count includes tokens
    /// and creatures controlled by every living player.
    GainLifeForEachCreature,
    /// Gain life equal to the positive damage amount that caused this
    /// source-specific triggered ability to fire. This is intentionally a
    /// semantic operation rather than copied card text; the trigger queue
    /// materializes it into `GainLifeController` before the ability resolves.
    GainLifeControllerFromSourceDamage,
    /// Draw one card for the controller when this effect resolves. This is
    /// intentionally a stack-only operation so public live-game setup cannot
    /// inject cards into a hand after the game has begun.
    DrawController,
    /// Draw one card for the targeted player as a stack instruction. The
    /// target slot preserves resolution-time legality instead of treating an
    /// opponent's draw as an untracked controller-side mutation.
    DrawTargetPlayer,
    /// Prevent represented library-search actions through the current turn.
    /// The marker is installed only by stack resolution and is cleared as the
    /// next turn begins, so a rejected search cannot consume mana, cards, or
    /// priority receipts.
    PreventLibrarySearchUntilEndOfTurn,
    /// Search the resolving controller's library for one card matching the
    /// typed requirement, move the deterministic selected card to the stated
    /// destination, then shuffle that controller's library. A policy may
    /// later replace the deterministic selector without altering this stack
    /// effect's lifecycle or receipts.
    SearchControllerLibrary {
        requirement: LibrarySearchRequirement,
        destination: LibrarySearchDestination,
        selection: LibrarySearchSelection,
    },
    /// Search the resolving controller's library for a policy-selected batch
    /// of cards matching one typed requirement.  The decision's cardinality,
    /// privacy, reveal state, destination, and following shuffle are owned by
    /// the effect rather than a card-specific resolver.
    SearchControllerLibraryMany {
        requirement: LibrarySearchRequirement,
        destination: LibrarySearchDestination,
        cardinality: LibrarySearchCardinality,
        selection: LibrarySearchSelection,
        /// A selected card becomes public before its ordinary zone move only
        /// when this flag is true. Private searches retain no reveal receipt.
        reveal_selected: bool,
    },
    /// Reveal the current top `count` cards of the resolving controller's
    /// library, suspend for a public ordered choice, then return the exact
    /// same cards to that library in the submitted top-to-bottom order.
    RevealTopLibraryCardsAndReorder {
        count: u8,
    },
    /// Attach this resolving permanent spell to the target creature and apply
    /// the stated persistent layer-seven modifier while both objects remain
    /// on the battlefield. This is an attachment operation, not a temporary
    /// target modifier.
    AttachSourceAndModifyTargetPt {
        power: i16,
        toughness: i16,
    },
    /// Attach this resolving Aura permanent to a target matching the typed
    /// enchant restriction and install every attachment-linked continuous
    /// change while both endpoints remain live.
    AttachSourceToTarget {
        target: TargetRequirement,
        changes: Vec<ContinuousChange>,
    },
    /// Return the permanent currently attached to this resolving Aura source
    /// to its owner's hand. The attachment endpoint is read at resolution
    /// from the source's exact battlefield incarnation; a departed, returned,
    /// or unattached source therefore cannot affect a later object merely
    /// because it retains the same stable object id.
    ReturnSourceAttachedPermanentToHand,
    /// Reveal the resolving controller's top library card, move it to hand,
    /// then make that controller lose life equal to its catalog mana value.
    /// An empty library has no card to reveal and is not a draw-loss path.
    RevealTopCardPutIntoHandLoseLifeEqualToManaValue,
    /// Materialized by a recipient-damage trigger after the source object has
    /// received positive damage. The amount is captured at receipt time.
    DealDamageToEachPlayerFromReceivedDamage,
    /// Draw one card only when this spell's explicit cast-payment receipt
    /// contains the named mana color. The receipt belongs to the stack object,
    /// so later floating mana or post-cast pool changes cannot affect it.
    DrawControllerIfManaColorSpent {
        color: Color,
    },
    /// Apply one temporary layer-seven modifier to every creature currently
    /// on the battlefield only when this spell's explicit cast-payment
    /// receipt contains the named mana color. The affected set is snapshotted
    /// as the instruction resolves, independent of the spell controller.
    ModifyAllCreaturesPtUntilEndOfTurnIfManaColorSpent {
        color: Color,
        power: i16,
        toughness: i16,
    },
    CreateToken {
        token: TokenSpec,
        count: u8,
    },
    /// Create tokens under the player selected by a targeted ETB ability.
    /// Hunted Dragon uses one targeted opponent rather than all opponents.
    CreateTokenForTargetPlayer {
        token: TokenSpec,
        count: u8,
    },
    /// First half of Razia's two-target replacement effect. The following
    /// targeted effect supplies the alternate damage recipient.
    BeginDamageRedirection {
        amount: i16,
    },
    /// Completes the pending Razia redirection using its second target.
    CompleteDamageRedirection,
    ModifyTargetPtUntilEndOfTurn {
        power: i16,
        toughness: i16,
    },
    /// Apply a temporary layer-seven adjustment and layer-six keyword grant to
    /// one creature target. Keeping the pair in one instruction preserves one
    /// target word and therefore one stack target slot.
    ModifyTargetPtAndKeywordUntilEndOfTurn {
        power: i16,
        toughness: i16,
        keyword: Keyword,
    },
    ModifyTargetKeywordUntilEndOfTurn {
        keyword: Keyword,
    },
    /// Prevent one selected creature from blocking the source permanent for
    /// this turn. This is source-relative rather than a global combat lock.
    PreventTargetBlockingSourceUntilEndOfTurn,
    /// Apply a temporary layer-7 modifier to the permanent that activated the
    /// resolving ability. This is intentionally source-relative rather than a
    /// target slot, matching self-pump abilities such as Goblin Fire Fiend.
    ModifySourcePtUntilEndOfTurn {
        power: i16,
        toughness: i16,
    },
    RemoveSourceKeywordUntilEndOfTurn {
        keyword: Keyword,
    },
    AddSourceDamageShieldUntilEndOfTurn {
        amount: i16,
    },
    /// Create an independent prevention shield for the resolving spell's
    /// controller whose amount is the chosen `{X}` retained on that spell's
    /// stack object. A zero X value is legal and simply creates no shield.
    AddControllerDamageShieldEqualToChosenXUntilEndOfTurn,
    /// Prevent the next amount of damage dealt to one target player or
    /// creature this turn. This shield is an independent replacement effect,
    /// so it remains valid even when the ability source has left the zone.
    AddTargetDamageShieldUntilEndOfTurn {
        amount: i16,
    },
    /// Put one regeneration replacement shield on a targeted creature. The
    /// shield is consumed only by the next destruction event; it does not
    /// prevent damage, sacrifice, or a zero-toughness state-based action.
    RegenerateTargetCreature,
    /// Put one regeneration replacement shield on the resolving ability's
    /// creature source. This has no target slot and models self-regeneration
    /// activations such as Sewerdreg's.
    RegenerateSource,
    /// Destroy the targeted land during resolution, sending it through the
    /// normal zone-change and continuous-effect lifecycle.
    DestroyTargetLand,
    /// Destroy one targeted land, then untap the resolving source only when
    /// that target was nonbasic at resolution.
    DestroyTargetLandAndUntapSourceIfNonbasic,
    /// Destroy the targeted artifact during resolution, sending it through
    /// the normal zone-change and continuous-effect lifecycle.
    DestroyTargetArtifact,
    /// Destroy one targeted creature with Flying through the normal,
    /// regenerable destruction lifecycle.
    DestroyTargetFlyingCreature,
    /// Destroy one targeted nonblack creature through the normal,
    /// regenerable destruction lifecycle. The typed target boundary keeps a
    /// source such as Brainspoil from accepting black creatures at cast time.
    DestroyTargetNonblackCreature,
    /// Destroy the targeted artifact or creature during resolution without
    /// allowing a regeneration shield to replace the destruction event.
    DestroyTargetArtifactOrCreatureNoRegeneration,
    /// Destroy one target creature while preserving the distinct-target
    /// provenance required by a multi-target spell such as Hex.
    DestroyDistinctTargetCreature,
    /// Destroy the creature that received the combat damage which caused this
    /// trigger. This bound triggered-effect template is materialized into an
    /// exact object-incarnation instruction before it reaches the stack.
    DestroyCombatDamagedCreature,
    /// Runtime-only materialization of [`Self::DestroyCombatDamagedCreature`].
    /// It is not legal in a printed card or registered ability binding. A
    /// recipient that changed zones before resolution is not affected through
    /// its stable object id.
    DestroyCapturedCreature {
        creature: ObjectId,
        incarnation: u64,
    },
    /// Destroy one targeted creature only when its mana value is no greater
    /// than the explicit X paid while casting this spell. This is intentionally
    /// separate from a generic destruction effect so the X-bound survives
    /// cast validation and resolution auditing.
    DestroyTargetCreatureWithManaValueAtMostChosenX,
    /// Tap one targeted creature as this spell or ability resolves. A creature
    /// that is already tapped remains a legal target but creates no duplicate
    /// tap receipt.
    TapTargetCreature,
    /// Untap the resolving permanent source while it remains a live tapped
    /// battlefield object. This is target-free because the printed reference
    /// is to the ability's own source.
    UntapSource,
    /// Untap one targeted land as the instruction resolves. A legal untapped
    /// land remains unchanged and writes no duplicate receipt.
    UntapTargetLand,
    /// Untap one targeted permanent as the instruction resolves. A legal
    /// untapped permanent remains unchanged and writes no duplicate receipt.
    /// This is intentionally broader than `UntapTargetLand`: card bindings
    /// use the typed target requirement to state whether any permanent or
    /// only a land is legal.
    UntapTargetPermanent,
    /// Apply one temporary layer-7 power/toughness modifier to every creature
    /// the resolving spell's controller currently controls. The recipient set
    /// is snapshotted while the spell resolves before any state-based action
    /// can run.
    ModifyControllerCreaturesPtUntilEndOfTurn {
        power: i16,
        toughness: i16,
    },
    AddKeywordToControllerCreaturesUntilEndOfTurn {
        keyword: Keyword,
    },
    RadianceUntapAndModifyUntilEndOfTurn {
        power: i16,
        toughness: i16,
    },
    /// Apply a temporary power/toughness modifier to the target creature and
    /// every creature sharing at least one of its colors. Unlike the existing
    /// radiance-and-untap effect, this semantic operation never changes tapped
    /// state.
    RadianceModifyPtUntilEndOfTurn {
        power: i16,
        toughness: i16,
    },
    /// Grant one keyword to the target creature and every creature sharing a
    /// color with it for the current turn. This is the keyword-only Radiance
    /// substrate used by Surge of Zeal.
    RadianceAddKeywordUntilEndOfTurn {
        keyword: Keyword,
    },
    /// Destroy the targeted enchantment and every other current enchantment
    /// sharing at least one of its colors. The Radiance recipient set is
    /// selected once during resolution before any destruction mutates zones.
    RadianceDestroyEnchantments,
    /// Destroy every nontoken creature that is on the battlefield when this
    /// instruction resolves. The candidate set is snapshotted before the
    /// first destroy instruction so zone changes cannot shrink the sweep.
    DestroyAllNonTokenCreatures,
    /// Counter one targeted instant or sorcery spell. This is intentionally a
    /// semantic effect rather than a copied card-text string.
    CounterTargetInstantOrSorcerySpell,
    /// Create one virtual copy of a targeted instant or sorcery stack object.
    /// A copy retains its source's cast-time values, but its controller may
    /// choose new legal targets through the typed decision boundary when the
    /// instruction permits it. The copied spell is not cast and never moves
    /// the original physical card between zones.
    CopyTargetInstantOrSorcerySpell {
        may_choose_new_targets: bool,
    },
    /// Sacrifice one creature controlled by the resolving source's controller
    /// if possible; otherwise counter one targeted noncreature spell.
    SacrificeCreatureOrCounterTargetSpell,
    /// Grant a current-turn mana-free cast permission for the targeted instant
    /// or sorcery in the resolving controller's graveyard.
    GrantGraveyardCastPermissionUntilEndOfTurn,
    /// Grant a current-turn permission for the targeted instant or sorcery in
    /// the resolving controller's exile zone. The typed payment and timing
    /// fields make the exceptional cast auditable rather than treating exile
    /// as an untracked second hand.
    GrantExileCastPermissionUntilEndOfTurn {
        payment: CastPermissionPayment,
        timing: CastTiming,
    },
    /// Destroy the targeted artifact or enchantment permanent. The target is
    /// rechecked as this instruction resolves, then changes zones using the
    /// ordinary destruction lifecycle.
    DestroyTargetArtifactOrEnchantment,
    /// Return a targeted permanent to its owner's hand, then make the
    /// controller it had immediately before the zone change lose life. The
    /// controller snapshot matters when controller and owner differ.
    ReturnTargetPermanentToHandAndLoseControllerLife {
        amount: i16,
    },
    /// Return the targeted card from the resolving spell controller's
    /// graveyard to that player's hand.
    ReturnTargetCardToHand,
    /// Return a targeted enchantment card from the resolving source
    /// controller's graveyard to that player's hand.
    ReturnTargetEnchantmentCardToHand,
    /// Select at most one creature card from each living player's graveyard
    /// before any zone movement, then return every selected card to its
    /// owner's hand. The selection is a target-free public-zone operation;
    /// a policy layer may replace the deterministic choice later.
    ReturnOneCreatureCardFromEachGraveyardToHand,
    /// Select up to three land cards from the resolving controller's graveyard
    /// before any of them move, then return those cards to that player's hand.
    /// The initial public slice uses deterministic public-zone selection until
    /// a policy can submit the up-to-three choice.
    ReturnUpToThreeControllerGraveyardLandCardsToHand,
    /// Suspend this spell's resolution while its controller privately chooses
    /// zero or more of the top cards of their library. Each selected card
    /// requires the stated life payment and moves to hand; the rest move to
    /// the graveyard. The engine owns the no-priority decision boundary.
    LookAtTopCardsChooseForLifeOrGraveyard {
        count: u8,
        life_per_card: i16,
    },
    /// Suspend a targeted activated ability while its controller privately
    /// inspects the top cards of the target opponent's library, then chooses
    /// exactly one available card to exile. The public event log never carries
    /// the candidate identities; only the resulting exile zone change is
    /// public.
    LookAtTopCardsOfTargetOpponentExileOne {
        count: u8,
    },
    /// Move every player's graveyard into that player's library, then shuffle
    /// each library. This is an untargeted, owner-preserving zone operation.
    ShuffleGraveyardsIntoLibraries,
    /// Return a target creature controlled by the resolving spell's controller
    /// to its owner's hand.
    ReturnControlledCreatureToHand,
    /// Return a target land controlled by the resolving source's controller
    /// to its owner's hand. This is a normal targeted stack effect, not a
    /// land-play replacement, so the newly entered land is itself legal.
    ReturnControlledLandToHand,
    /// Return a targeted creature card from the resolving source controller's
    /// graveyard to hand only while at least one other creature card remains
    /// there. This models an intervening-condition trigger at both trigger
    /// placement and resolution without treating the target as a hidden-zone
    /// free choice.
    ReturnTargetCreatureCardToHandIfAnotherInControllerGraveyard,
    /// Return a targeted creature card from the resolving controller's
    /// graveyard to the battlefield. When the named color appears in the
    /// immutable spell-payment receipt, the returned permanent receives one
    /// persistent +1/+1 counter.
    ReturnTargetCreatureCardToBattlefieldWithCounterIfManaColorSpent {
        color: Color,
    },
    /// Return a target creature controlled by another player to its owner's
    /// hand. This remains distinct so paired targets cannot silently select
    /// two creatures on one side.
    ReturnOpponentCreatureToHand,
    /// Move one targeted creature from the battlefield to its owner's exile
    /// zone.  This is a zone-change instruction rather than lethal damage, so
    /// it bypasses regeneration and preserves the target's normal
    /// zone-departure lifecycle.
    ExileTargetCreature,
    /// Move a creature that is currently attacking or blocking to exile.
    /// This remains distinct from `ExileTargetCreature`, whose target may be
    /// any battlefield creature.
    ExileTargetPermanent,
    /// Exile the Aura source's currently enchanted creature and every
    /// Aura-like permanent attached to that exact creature incarnation, then
    /// retain a typed group for a deterministic beginning-of-end-step return.
    /// The source is normally one of the attached Auras; this is a generic
    /// source-relative rules operation, not a card-name branch.
    ExileAttachedCreatureAndAurasUntilEndStep,
}

impl Effect {
    /// Whether resolving this instruction is defined only from a stack
    /// object's explicit mana-payment receipt rather than a deterministic
    /// pool drain.
    #[must_use]
    pub const fn requires_explicit_mana_spend(&self) -> bool {
        matches!(
            self,
            Self::DrawControllerIfManaColorSpent { .. }
                | Self::ModifyAllCreaturesPtUntilEndOfTurnIfManaColorSpent { .. }
                | Self::ReturnTargetCreatureCardToBattlefieldWithCounterIfManaColorSpent { .. }
        )
    }

    /// Whether this instruction requires the spell cast to name an explicit
    /// nonnegative X value. The value is paid as additional generic mana and
    /// retained in the ordered mana receipt through resolution.
    #[must_use]
    pub const fn requires_chosen_x(&self) -> bool {
        matches!(
            self,
            Self::DestroyTargetCreatureWithManaValueAtMostChosenX
                | Self::AddControllerDamageShieldEqualToChosenXUntilEndOfTurn
                | Self::SearchControllerLibrary {
                    requirement: LibrarySearchRequirement::CreatureWithManaValueAtMostChosenX,
                    ..
                }
                | Self::SearchControllerLibraryMany {
                    requirement: LibrarySearchRequirement::CreatureWithManaValueAtMostChosenX,
                    ..
                }
        )
    }

    #[must_use]
    #[allow(clippy::too_many_lines)] // One exhaustive semantic-to-target map keeps stack planning reviewable.
    pub const fn target_requirement(&self) -> Option<TargetRequirement> {
        match self {
            Self::DealDamage { target, .. }
            | Self::DealDamageEqualToAttackingCreatures { target }
            | Self::AttachSourceToTarget { target, .. } => Some(*target),
            Self::ModifyTargetPtUntilEndOfTurn { .. }
            | Self::ModifyTargetPtAndKeywordUntilEndOfTurn { .. }
            | Self::ModifyTargetKeywordUntilEndOfTurn { .. }
            | Self::AttachSourceAndModifyTargetPt { .. }
            | Self::RadianceDealDamageToCreatures { .. }
            | Self::RadianceUntapAndModifyUntilEndOfTurn { .. }
            | Self::RadianceModifyPtUntilEndOfTurn { .. }
            | Self::RadianceAddKeywordUntilEndOfTurn { .. }
            | Self::BeginDamageRedirection { .. }
            | Self::PreventTargetBlockingSourceUntilEndOfTurn
            | Self::ExileTargetCreature
            | Self::TapTargetCreature
            | Self::RegenerateTargetCreature
            | Self::AddPlusOneCounterToTarget => Some(TargetRequirement::Creature),
            Self::AddTargetDamageShieldUntilEndOfTurn { .. } => {
                Some(TargetRequirement::PlayerOrCreature)
            }
            Self::DestroyTargetCreatureWithManaValueAtMostChosenX => {
                Some(TargetRequirement::Creature)
            }
            Self::DestroyTargetNonblackCreature => Some(TargetRequirement::NonblackCreature),
            Self::DestroyDistinctTargetCreature => Some(TargetRequirement::DistinctCreature),
            Self::ExileTargetPermanent => Some(TargetRequirement::AttackingOrBlockingCreature),
            Self::CompleteDamageRedirection => Some(TargetRequirement::PlayerOrCreature),
            Self::LoseLifeTarget { .. }
            | Self::CreateTokenForTargetPlayer { .. }
            | Self::DrawTargetPlayer
            | Self::DiscardTargetPlayer { .. }
            | Self::MillTargetPlayer { .. }
            | Self::MillTargetPlayerFromSourceDamage => Some(TargetRequirement::Player),
            Self::LookAtTopCardsOfTargetOpponentExileOne { .. } => {
                Some(TargetRequirement::Opponent)
            }
            Self::DestroyTargetLand | Self::DestroyTargetLandAndUntapSourceIfNonbasic => {
                Some(TargetRequirement::Land)
            }
            Self::UntapTargetLand => Some(TargetRequirement::Land),
            Self::DestroyTargetArtifact => Some(TargetRequirement::Artifact),
            Self::RadianceDestroyEnchantments => Some(TargetRequirement::Enchantment),
            Self::DestroyTargetFlyingCreature => Some(TargetRequirement::FlyingCreature),
            Self::DestroyTargetArtifactOrCreatureNoRegeneration => {
                Some(TargetRequirement::ArtifactOrCreature)
            }
            Self::DestroyTargetArtifactOrEnchantment => {
                Some(TargetRequirement::ArtifactOrEnchantment)
            }
            Self::AddCountersToTarget { .. }
            | Self::RemoveCountersFromTarget { .. }
            | Self::ReturnTargetPermanentToHandAndLoseControllerLife { .. }
            | Self::UntapTargetPermanent
            | Self::GainControlTargetUntilEndOfTurn => Some(TargetRequirement::Permanent),
            Self::ReturnTargetCardToHand => Some(TargetRequirement::OwnGraveyardCard),
            Self::ReturnTargetEnchantmentCardToHand => {
                Some(TargetRequirement::EnchantmentCardInControllerGraveyard)
            }
            Self::ReturnTargetCreatureCardToHandIfAnotherInControllerGraveyard
            | Self::ReturnTargetCreatureCardToBattlefieldWithCounterIfManaColorSpent { .. } => {
                Some(TargetRequirement::CreatureCardInControllerGraveyard)
            }
            Self::ReturnControlledCreatureToHand => Some(TargetRequirement::ControlledCreature),
            Self::ReturnControlledLandToHand => Some(TargetRequirement::ControlledLand),
            Self::ReturnOpponentCreatureToHand => Some(TargetRequirement::OpponentCreature),
            Self::CounterTargetInstantOrSorcerySpell
            | Self::CopyTargetInstantOrSorcerySpell { .. } => {
                Some(TargetRequirement::InstantOrSorcerySpell)
            }
            Self::SacrificeCreatureOrCounterTargetSpell => {
                Some(TargetRequirement::NoncreatureSpell)
            }
            Self::GrantGraveyardCastPermissionUntilEndOfTurn => {
                Some(TargetRequirement::InstantOrSorceryCardInControllerGraveyard)
            }
            Self::GrantExileCastPermissionUntilEndOfTurn { .. } => {
                Some(TargetRequirement::InstantOrSorceryCardInControllerExile)
            }
            Self::DealDamageController { .. }
            | Self::LoseLifeController { .. }
            | Self::LoseLifeEachOpponentEqualToControlledCreatures
            | Self::DiscardOneCardEachPlayer
            | Self::SacrificeControllerCreature
            | Self::DealDamageAfterOptionalManaPayment { .. }
            | Self::DealDamageToEachCreatureAndPlayer { .. }
            | Self::DealDamageToEachPlayer { .. }
            | Self::DealDamageToEachNonFlyingCreature { .. }
            | Self::GainLifeController { .. }
            | Self::GainLifeForEachCreature
            | Self::GainLifeControllerFromSourceDamage
            | Self::DrawController
            | Self::PreventLibrarySearchUntilEndOfTurn
            | Self::SearchControllerLibrary { .. }
            | Self::SearchControllerLibraryMany { .. }
            | Self::RevealTopLibraryCardsAndReorder { .. }
            | Self::RevealTopCardPutIntoHandLoseLifeEqualToManaValue
            | Self::DealDamageToEachPlayerFromReceivedDamage
            | Self::MillSourceControllerFromSourceDamage
            | Self::MillCapturedPlayer { .. }
            | Self::AddManaController { .. }
            | Self::AddPlusOneCounterToSource
            | Self::AddCountersToSource { .. }
            | Self::RemoveCountersFromSource { .. }
            | Self::DrawControllerIfManaColorSpent { .. }
            | Self::ModifyAllCreaturesPtUntilEndOfTurnIfManaColorSpent { .. }
            | Self::CreateToken { .. }
            | Self::ReturnOneCreatureCardFromEachGraveyardToHand
            | Self::ReturnUpToThreeControllerGraveyardLandCardsToHand
            | Self::ReturnSourceAttachedPermanentToHand
            | Self::LookAtTopCardsChooseForLifeOrGraveyard { .. }
            | Self::ShuffleGraveyardsIntoLibraries
            | Self::ModifySourcePtUntilEndOfTurn { .. }
            | Self::RemoveSourceKeywordUntilEndOfTurn { .. }
            | Self::AddSourceDamageShieldUntilEndOfTurn { .. }
            | Self::AddControllerDamageShieldEqualToChosenXUntilEndOfTurn
            | Self::RegenerateSource
            | Self::UntapSource
            | Self::ModifyControllerCreaturesPtUntilEndOfTurn { .. }
            | Self::AddKeywordToControllerCreaturesUntilEndOfTurn { .. }
            | Self::DestroyAllNonTokenCreatures
            | Self::DestroyCombatDamagedCreature
            | Self::DestroyCapturedCreature { .. }
            | Self::ExileAttachedCreatureAndAurasUntilEndStep => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CardDefinition {
    /// Stable `CardBench` identifier, for example `RAV-LIGHTNING-HELIX`.
    pub id: &'static str,
    /// Human label. No rules text or image data is bundled here.
    pub name: &'static str,
    pub set_code: &'static str,
    pub mana_cost: ManaCost,
    pub colors: BTreeSet<Color>,
    /// Colors a land's intrinsic mana ability can produce in this engine slice.
    /// Nonlands leave this empty.
    pub mana_colors: BTreeSet<Color>,
    pub card_types: BTreeSet<CardType>,
    /// Basic lands are exempt from the normal four-copy deck construction limit.
    pub is_basic_land: bool,
    /// Named semantic slices intentionally supported for this card definition.
    /// This makes a partial initial expansion implementation explicit instead of
    /// silently presenting itself as the complete printed card.
    pub supported_rules: &'static [&'static str],
    pub power: Option<i16>,
    pub toughness: Option<i16>,
    pub keywords: Vec<Keyword>,
    pub effects: Vec<Effect>,
}

impl CardDefinition {
    #[must_use]
    pub fn is_permanent(&self) -> bool {
        self.card_types.iter().any(|kind| {
            matches!(
                kind,
                CardType::Artifact
                    | CardType::Creature
                    | CardType::Enchantment
                    | CardType::Land
                    | CardType::Planeswalker
            )
        })
    }

    #[must_use]
    pub fn is_creature(&self) -> bool {
        self.card_types.contains(&CardType::Creature)
    }

    #[must_use]
    pub fn is_land(&self) -> bool {
        self.card_types.contains(&CardType::Land)
    }

    #[must_use]
    pub fn dredge(&self) -> Option<u8> {
        self.keywords.iter().find_map(|keyword| match keyword {
            Keyword::Dredge(amount) => Some(*amount),
            _ => None,
        })
    }

    #[must_use]
    pub fn transmute_cost(&self) -> Option<&ManaCost> {
        self.keywords.iter().find_map(|keyword| match keyword {
            Keyword::Transmute(cost) => Some(cost),
            _ => None,
        })
    }

    #[must_use]
    pub fn has_convoke(&self) -> bool {
        self.keywords.contains(&Keyword::Convoke)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeckEntry {
    pub card: String,
    pub count: u8,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeckList {
    pub mainboard: Vec<DeckEntry>,
    pub sideboard: Vec<DeckEntry>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DeckRules {
    pub minimum_mainboard_size: u16,
    pub maximum_copies: u8,
    pub maximum_sideboard_size: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeckValidationError {
    UnknownCard(String),
    MainboardTooSmall {
        actual: u16,
        minimum: u16,
    },
    SideboardTooLarge {
        actual: u16,
        maximum: u16,
    },
    TooManyCopies {
        card: String,
        actual: u16,
        maximum: u8,
    },
}

impl std::fmt::Display for DeckValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownCard(card) => write!(formatter, "unknown deck card `{card}`"),
            Self::MainboardTooSmall { actual, minimum } => {
                write!(
                    formatter,
                    "mainboard has {actual} cards; minimum is {minimum}"
                )
            }
            Self::SideboardTooLarge { actual, maximum } => {
                write!(
                    formatter,
                    "sideboard has {actual} cards; maximum is {maximum}"
                )
            }
            Self::TooManyCopies {
                card,
                actual,
                maximum,
            } => write!(
                formatter,
                "`{card}` has {actual} copies; maximum is {maximum}"
            ),
        }
    }
}

impl std::error::Error for DeckValidationError {}

impl DeckList {
    /// Validates a deck against a supplied set/format catalog. Duplicate entries are
    /// aggregated before the copy limit is checked, avoiding a manifest loophole.
    pub fn validate(
        &self,
        catalog: &BTreeMap<&'static str, CardDefinition>,
        rules: DeckRules,
    ) -> Result<(), DeckValidationError> {
        let mut mainboard_total = 0_u16;
        let mut sideboard_total = 0_u16;
        let mut copies = BTreeMap::<&str, u16>::new();
        for entry in &self.mainboard {
            let definition = catalog
                .get(entry.card.as_str())
                .ok_or_else(|| DeckValidationError::UnknownCard(entry.card.clone()))?;
            mainboard_total += u16::from(entry.count);
            if !definition.is_basic_land {
                *copies.entry(definition.id).or_default() += u16::from(entry.count);
            }
        }
        for entry in &self.sideboard {
            let definition = catalog
                .get(entry.card.as_str())
                .ok_or_else(|| DeckValidationError::UnknownCard(entry.card.clone()))?;
            sideboard_total += u16::from(entry.count);
            if !definition.is_basic_land {
                *copies.entry(definition.id).or_default() += u16::from(entry.count);
            }
        }
        if mainboard_total < rules.minimum_mainboard_size {
            return Err(DeckValidationError::MainboardTooSmall {
                actual: mainboard_total,
                minimum: rules.minimum_mainboard_size,
            });
        }
        if sideboard_total > rules.maximum_sideboard_size {
            return Err(DeckValidationError::SideboardTooLarge {
                actual: sideboard_total,
                maximum: rules.maximum_sideboard_size,
            });
        }
        for (card, actual) in copies {
            if actual > u16::from(rules.maximum_copies) {
                return Err(DeckValidationError::TooManyCopies {
                    card: card.to_owned(),
                    actual,
                    maximum: rules.maximum_copies,
                });
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CardObject {
    pub id: ObjectId,
    pub definition: Option<&'static str>,
    /// Monotonic object incarnation. A card keeps its public `ObjectId` while
    /// changing zones, but each zone change creates a new rules object for
    /// target and continuous-effect provenance.
    pub incarnation: u64,
    pub owner: PlayerId,
    /// Base controller. On the battlefield, `Game::controller_of` derives the
    /// live controller by applying active layer-two control effects in
    /// timestamp order. Outside the battlefield this must equal `owner`.
    pub controller: PlayerId,
    pub tapped: bool,
    /// Marked damage is runtime state, not printed card data.  It is wider
    /// than a card's printed power/toughness so repeated legal effects never
    /// wrap or panic part way through stack resolution.
    pub damage: i32,
    /// Whether this incarnation has received at least one positive point of
    /// damage from a source with Deathtouch. It is separate from numeric
    /// marked damage because CR 704.5h depends on source quality.
    pub deathtouch_damage: bool,
    /// Temporary prevention shield units waiting to absorb damage.
    pub damage_shield: i32,
    pub counters: BTreeMap<CounterKind, i16>,
    /// The permanent this Aura-like object is attached to. This identity is
    /// explicit so attachment cleanup and its persistent layer effect are
    /// auditable rather than inferred from a card name or target history.
    pub attached_to: Option<ObjectId>,
    /// Incarnation captured with `attached_to`.  This prevents an Aura from
    /// silently treating a later incarnation of the same physical card as the
    /// object it was attached to before a zone change.
    pub attached_to_incarnation: Option<u64>,
    pub entered_turn: u32,
    /// Turn in which this permanent most recently changed controller. This
    /// tracks the continuous-control boundary separately from entry so a
    /// creature stolen this turn cannot attack or pay a tap-symbol cost unless
    /// it has Haste.
    pub controller_changed_turn: u32,
    pub token: Option<TokenSpec>,
    /// A layer-one copy snapshot currently applied to this battlefield
    /// incarnation.  It deliberately contains copiable values only: marked
    /// damage, counters, attachments, controller, tapped state, and ordinary
    /// timestamped effects remain runtime state of this object.
    pub copied_permanent: Option<CopiedPermanent>,
}

/// The characteristic payload an object contributes when another permanent
/// becomes a copy of it.  This is intentionally distinct from
/// [`Characteristics`], which includes later-layer effects and counters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CopiableValues {
    /// A non-token card's printed definition, or the definition that it is
    /// already copying.  Definition-bound abilities are consequently copied
    /// without cloning executable closures or borrowing live source state.
    CardDefinition(&'static str),
    /// A token's creation specification.  Copying this into a card changes
    /// its characteristics but does not turn that card into a token.
    Token(TokenSpec),
}

/// Persistent layer-one state for a permanent-copy effect.
///
/// The source identity is event and audit provenance only.  A copy effect is
/// a snapshot, so it remains after that source leaves the battlefield; the
/// target's own zone change clears it and starts a fresh incarnation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CopiedPermanent {
    pub values: CopiableValues,
    pub source: ObjectId,
    pub source_incarnation: u64,
    pub timestamp: u64,
}

impl CardObject {
    /// Returns the definition currently exposed to definition-bound rules.
    /// A token-value copy intentionally returns `None` even when the physical
    /// object is a card: that card has copied a token's characteristics and
    /// must not retain its own activated/static definition-bound abilities.
    #[must_use]
    pub fn effective_definition(&self) -> Option<&'static str> {
        match &self.copied_permanent {
            Some(CopiedPermanent {
                values: CopiableValues::CardDefinition(definition),
                ..
            }) => Some(*definition),
            Some(CopiedPermanent {
                values: CopiableValues::Token(_),
                ..
            }) => None,
            None => self.definition,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Characteristics {
    pub colors: BTreeSet<Color>,
    pub card_types: BTreeSet<CardType>,
    /// Typed creature subtypes visible to rules that inspect a creature's
    /// type line. Card definitions do not yet model subtypes, so the initial
    /// nonempty values originate from token specifications.
    pub creature_subtypes: BTreeSet<CreatureSubtype>,
    /// Derived layer-seven values.  Printed values and individual modifiers
    /// remain `i16`, while the evaluated result is widened for safe repeated
    /// continuous-effect application.
    pub power: Option<i32>,
    pub toughness: Option<i32>,
    pub keywords: Vec<Keyword>,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Layer {
    /// Layer one copy effects are represented as an object-local copiable
    /// snapshot rather than a timestamped layer-4--7 continuous effect.
    Copy = 1,
    Control = 2,
    Type = 4,
    Color = 5,
    Ability = 6,
    PowerToughness = 7,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContinuousChange {
    /// A timestamped layer-two control effect. `CardObject::controller` is
    /// the object's base controller; live controller queries apply these
    /// changes in timestamp order without moving the object between its
    /// owner's zone vectors.
    ChangeController(PlayerId),
    AddCardType(CardType),
    AddColor(Color),
    AddKeyword(Keyword),
    RemoveKeyword(Keyword),
    CannotBlockSource(ObjectId),
    AddDamageShield(i16),
    ModifyPowerToughness {
        power: i16,
        toughness: i16,
    },
    /// A static characteristic-defining effect that sets the source's power
    /// and toughness to the live number of creatures controlled by that
    /// permanent's controller. It is registered as immutable expansion data,
    /// never installed as a timestamped temporary effect.
    ControlledCreatureCountPowerToughness,
    /// A battlefield-only static layer-seven effect that modifies every
    /// other creature controlled by the source's controller. It is immutable
    /// expansion data, not a timestamped effect that can be installed during
    /// play.
    OtherControlledCreaturesModifyPowerToughness {
        power: i16,
        toughness: i16,
    },
    /// A battlefield-only static layer-six effect that grants one keyword to
    /// every other creature controlled by the source's controller.
    OtherControlledCreaturesAddKeyword(Keyword),
    /// A battlefield-only static layer-six effect that grants one keyword to
    /// every creature controlled by the source's controller, but only while
    /// at least one live Aura is attached to the source.
    ControlledCreaturesAddKeywordIfSourceEnchanted(Keyword),
    /// The affected permanent's activated nonmana abilities cannot be
    /// activated. Mana abilities remain legal and continue to bypass the stack.
    SuppressNonManaActivatedAbilities,
}

impl ContinuousChange {
    #[must_use]
    pub const fn layer(&self) -> Layer {
        match self {
            Self::ChangeController(_) => Layer::Control,
            Self::AddCardType(_) => Layer::Type,
            Self::AddColor(_) => Layer::Color,
            Self::AddKeyword(_)
            | Self::RemoveKeyword(_)
            | Self::CannotBlockSource(_)
            | Self::AddDamageShield(_)
            | Self::OtherControlledCreaturesAddKeyword(_)
            | Self::ControlledCreaturesAddKeywordIfSourceEnchanted(_)
            | Self::SuppressNonManaActivatedAbilities => Layer::Ability,
            Self::ModifyPowerToughness { .. }
            | Self::ControlledCreatureCountPowerToughness
            | Self::OtherControlledCreaturesModifyPowerToughness { .. } => Layer::PowerToughness,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Duration {
    EndOfTurn(u32),
    Permanent,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContinuousEffect {
    pub source: ObjectId,
    /// The source incarnation that created this effect.  Stable public card
    /// identity alone cannot keep a persistent effect alive across a source
    /// zone change and later return.
    pub source_incarnation: u64,
    pub target: ObjectId,
    /// The target incarnation this effect is allowed to modify.  A physical
    /// card can retain its public id after leaving and re-entering, but the
    /// returned permanent is a new rules object.
    pub target_incarnation: u64,
    pub change: ContinuousChange,
    pub duration: Duration,
    pub timestamp: u64,
}

/// The two attachment families represented by the initial general attachment
/// substrate.  An Aura must remain attached to a legal permanent; Equipment
/// may remain on the battlefield unattached and can move between legal
/// permanents through its activated ability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AttachmentKind {
    Aura,
    Equipment,
}

/// Immutable expansion data describing how one permanent attaches to another.
///
/// A binding deliberately owns the typed restriction and the linked
/// continuous changes rather than inferring them from a card name.  An Aura
/// spell and an Equipment activated ability both still carry an ordinary
/// `AttachSourceToTarget` effect so the stack retains target occurrence and
/// resolution-time legality; this binding gives that effect its attachment
/// lifecycle semantics. An Aura may have no linked continuous changes (for
/// example, an Aura whose rules text is only a source-relative trigger); an
/// Equipment binding must retain at least one linked change.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttachmentBinding {
    pub card_definition: &'static str,
    pub kind: AttachmentKind,
    pub target: TargetRequirement,
    pub changes: Vec<ContinuousChange>,
}

/// Immutable expansion data for a static continuous effect. The effect is
/// active only while a permanent with the bound definition is on the
/// battlefield; it creates neither a stack object nor an event-log receipt.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticContinuousEffectBinding {
    pub card_definition: &'static str,
    pub change: ContinuousChange,
}

/// A battlefield-only rule that restricts combat declarations without changing
/// a permanent's characteristics. These bindings are immutable expansion data,
/// rechecked from the live battlefield before attacker state is mutated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticAttackRestriction {
    /// Opponents cannot declare creatures as attacking the source's controller.
    OpponentsCannotAttackController,
}

/// Immutable expansion data for a battlefield-only attack restriction.
/// Unlike a continuous-effect binding, this rule is enforced directly at the
/// declaration legality boundary and creates no synthetic event receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticAttackRestrictionBinding {
    pub card_definition: &'static str,
    pub restriction: StaticAttackRestriction,
}

/// A battlefield-only replacement-style rule that changes how eligible
/// permanents enter. It is applied during the ordinary zone transition rather
/// than becoming a delayed trigger or a post-entry continuous effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticEntryRestriction {
    /// Artifacts, creatures, and lands controlled by an opponent of the live
    /// source enter the battlefield tapped.
    OpponentsArtifactsCreaturesAndLandsEnterTapped,
}

/// Immutable expansion data for a static entry replacement. Sources are
/// rechecked from the live battlefield at each entry; registration itself
/// never mutates a permanent or produces a gameplay receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticEntryRestrictionBinding {
    pub card_definition: &'static str,
    pub restriction: StaticEntryRestriction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Zone {
    Library,
    Hand,
    Battlefield,
    Graveyard,
    Exile,
}

/// The public zone from which an effect-created cast is authorized.  It is
/// stored in permission state and receipts rather than inferred from the
/// card's later zone, because a zone change must revoke the old permission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CastPermissionZone {
    Graveyard,
    Exile,
}

/// Whether an effect-created casting permission replaces the spell's mana
/// cost. Nonmana additional costs are deliberately outside this first slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CastPermissionPayment {
    PayManaCost,
    WithoutPayingManaCost,
}

/// The timing boundary supplied by an effect-created permission. `Normal`
/// retains normal instant/sorcery timing; `AsThoughInstant` is the bounded
/// exception needed by effects that explicitly allow the cast at instant
/// speed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CastTiming {
    Normal,
    AsThoughInstant,
}

/// Stable identity for one typed linked-exile group. A group is ephemeral
/// rules state, not a card identity: it records precisely which object
/// incarnations one resolving effect placed in exile for a later action.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LinkedExileGroupId(pub u64);

/// Stable identity for one scheduled delayed action. The id is public through
/// receipts so a replay can distinguish two otherwise identical return groups.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DelayedActionId(pub u64);

/// The role a member had when a linked-exile group was created. The bounded
/// initial substrate has one primary creature plus any Aura-like permanents
/// attached to that exact creature incarnation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkedExileMemberRole {
    PrimaryCreature,
    AttachedAura,
}

/// An exact object incarnation owned by a linked-exile group. A later zone
/// change deliberately makes this record stale; delayed return never treats a
/// stable `ObjectId` alone as permission to move a later incarnation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LinkedExileMember {
    pub object: ObjectId,
    pub exile_incarnation: u64,
    pub role: LinkedExileMemberRole,
}

/// Typed, clonable state retained while a linked-exile return is pending.
/// `source_incarnation` is historical provenance and is intentionally valid
/// after the source Aura is itself exiled with the group.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkedExileGroup {
    pub id: LinkedExileGroupId,
    pub controller: PlayerId,
    pub source: ObjectId,
    pub source_incarnation: u64,
    pub members: Vec<LinkedExileMember>,
}

/// The timing vocabulary for typed delayed actions. More timing windows can
/// be introduced without putting closures or card-specific continuations in
/// game state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DelayedActionTiming {
    EndStep,
}

/// The typed continuation a delayed action will execute. This remains a data
/// enum so cloning, replay auditing, and invariant validation never depend on
/// closures captured from a resolver.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DelayedActionKind {
    ReturnLinkedExileGroup { group: LinkedExileGroupId },
}

/// A scheduled, replay-visible continuation. `due_turn` is calculated when
/// scheduled so an action created during an end step waits for the next one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DelayedAction {
    pub id: DelayedActionId,
    pub timing: DelayedActionTiming,
    pub due_turn: u32,
    pub controller: PlayerId,
    pub kind: DelayedActionKind,
}

/// A monotonically increasing identity for one no-priority policy decision.
/// A decision id is never reused, including when the same source opens a
/// later, otherwise identical choice. This prevents a stale policy response
/// from mutating a newer continuation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DecisionId(pub u64);

/// Whether candidate identities may be projected outside the deciding policy.
/// Both variants leave the selected result to ordinary typed game receipts;
/// hidden candidates never enter the public canonical log.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecisionVisibility {
    Public,
    Private,
}

/// The expansion-neutral category of a currently supported decision.
/// Additional categories can share the same id, cardinality, projection, and
/// continuation substrate without adding more pending booleans to `Game`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecisionKind {
    LibrarySearch,
    /// A public top-library slice was revealed and must be placed back in one
    /// exact top-to-bottom order before the suspended stack item continues.
    LibraryReorder,
    TriggeredEffectObject,
    /// The attacking player orders one multi-block group after blockers are
    /// declared and before either player receives priority.
    CombatDamageOrder,
    /// The controller of an effect that copies a spell selects replacement
    /// targets before the new virtual stack object exists. This is a
    /// no-priority rules decision, never a free targeting action.
    SpellCopyTargets,
    /// The controller of a target-bearing triggered ability selects every
    /// required target before that ability enters the stack. This preserves
    /// the no-priority trigger-placement boundary while giving the choice a
    /// monotonic identity and ordinary stale-response protection.
    TriggeredAbilityTargets,
    /// One APNAP controller orders the simultaneous triggered abilities they
    /// control before any member of that controller group enters the stack.
    TriggeredAbilityOrder,
    /// The affected player orders applicable quantity replacements.
    Replacement,
}

/// One public member of an APNAP simultaneous-trigger ordering group.
///
/// Source incarnation is intentionally part of the identity: a source may
/// have left the battlefield after triggering, and a later incarnation with
/// the same stable object id must not satisfy a stale ordering response.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TriggerOrderEntry {
    pub source: ObjectId,
    pub source_incarnation: u64,
    pub ability: &'static str,
}

/// A concrete option retained in typed pending-decision state. This first
/// migration supports object choices; the enum deliberately keeps the policy
/// surface extensible for targets, colors, ordering, and replacements.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecisionOption {
    Object(ObjectId),
    /// A public stack/battlefield/player target option. Target choices use a
    /// distinct variant so a card selected from a private library cannot be
    /// confused with a public target selected for a copied spell.
    Target(Target),
    /// A public triggered-ability identity available in one exact APNAP
    /// controller group. It is distinct from an object choice because two
    /// abilities on the same source are independently orderable.
    TriggerOrder(TriggerOrderEntry),
    /// A public, typed replacement identity. Hidden-zone candidate cards are
    /// never represented by this option shape.
    Replacement(ReplacementChoice),
}

/// A submitted answer to a typed decision. The continuation determines which
/// shapes are legal; it never stores an executable closure in game state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecisionSelection {
    Objects(Vec<ObjectId>),
    Targets(Vec<Target>),
    TriggerOrder(Vec<TriggerOrderEntry>),
    Replacements(Vec<ReplacementChoice>),
}

/// Stateful continuation details for the migrated trigger-effect object
/// choices. The values are plain cloned data, so they remain replay-auditable
/// across multi-player discard prompts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TriggeredEffectObjectDecisionKind {
    DiscardEachPlayer {
        remaining_players: Vec<PlayerId>,
        selected: Vec<(PlayerId, ObjectId)>,
    },
    SacrificeControllerCreature,
}

/// The ordinary event that will commit after every applicable quantity
/// replacement has been applied. It stores the effect payload rather than a
/// resolver closure so a suspended stack item remains cloneable and auditable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QuantityReplacementResolution {
    CreateTokens {
        player: PlayerId,
        token: TokenSpec,
    },
    PlaceCounters {
        card: ObjectId,
        counter: CounterKind,
    },
}

/// The typed continuation that resumes when a pending decision completes.
/// This replaces specialized game-local marker structs for the migrated
/// decision paths while preserving their existing effect-specific receipts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecisionContinuation {
    LibrarySearch {
        source: ObjectId,
        requirement: LibrarySearchRequirement,
        destination: LibrarySearchDestination,
        may_fail_to_find: bool,
    },
    LibrarySearchMany {
        source: ObjectId,
        requirement: LibrarySearchRequirement,
        destination: LibrarySearchDestination,
        cardinality: LibrarySearchCardinality,
        may_fail_to_find: bool,
        reveal_selected: bool,
    },
    LibraryReorder {
        source: ObjectId,
        /// Captured current top cards in public top-to-bottom order. Exact
        /// candidates prevent a library mutation or a stale decision from
        /// rearranging a later library state.
        cards: Vec<ObjectId>,
    },
    TriggeredEffectObject {
        source: ObjectId,
        controller: PlayerId,
        ability: &'static str,
        kind: TriggeredEffectObjectDecisionKind,
    },
    /// Continues CR 509.2 blocker ordering. Only one blocker group is exposed
    /// at a time so the generic decision boundary still has one chooser and
    /// one exact option set. `remaining` preserves the other groups.
    CombatDamageOrder {
        attacker: ObjectId,
        remaining: Vec<(ObjectId, Vec<ObjectId>)>,
    },
    /// A copy effect remains on the stack while this public target decision
    /// is pending. `source` identifies that copying spell, while `original`
    /// identifies the lower instant/sorcery whose stack values will be cloned
    /// once the decision completes.
    SpellCopyTargets {
        source: ObjectId,
        source_incarnation: u64,
        controller: PlayerId,
        original: ObjectId,
        original_source_incarnation: u64,
    },
    /// A target-bearing triggered ability remains outside the stack while its
    /// controller supplies one target for each declared target occurrence.
    /// Source identity and colors are captured at trigger time: the source
    /// may leave the battlefield before selection without changing the
    /// already-triggered ability's origin or targeting color provenance.
    TriggeredAbilityTargets {
        source: ObjectId,
        source_incarnation: u64,
        source_colors: BTreeSet<Color>,
        controller: PlayerId,
        ability: TriggeredAbility,
        effects: Vec<Effect>,
    },
    /// Resumes one controller's group from CR 603.3b. The scheduler retains
    /// the full event payload; this continuation exposes only public ordering
    /// identities to the policy surface.
    TriggeredAbilityOrder { controller: PlayerId },
    /// A stack item retained while concurrent token/counter multipliers are
    /// resolved in the affected player's selected order. `used` stores the
    /// exact source incarnation/effect identity so the same replacement can
    /// never apply twice to one prospective event.
    QuantityReplacement {
        source: ObjectId,
        source_incarnation: u64,
        controller: PlayerId,
        event: ReplacementEventKind,
        original_amount: i16,
        amount: i16,
        used: Vec<ReplacementChoice>,
        resolution: QuantityReplacementResolution,
    },
    /// Resumes the bounded one-target direct-damage replacement chain. The
    /// candidate identity is shared with quantity replacement decisions, but
    /// the committed event still has its existing source-aware damage
    /// receipts. `used` retains the exact replacement identities already
    /// applied to this prospective event, so redirected damage cannot reuse a
    /// shield or redirection from an earlier incarnation.
    DamageReplacement {
        source: ObjectId,
        source_incarnation: u64,
        controller: PlayerId,
        original_target: Target,
        target: Target,
        target_incarnation: Option<u64>,
        amount: i32,
        used: Vec<DamageReplacementChoice>,
    },
}

/// One serializable, no-priority decision boundary. Candidate options remain
/// internal until projected through the deciding player's `GameView`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PendingDecision {
    pub id: DecisionId,
    pub player: PlayerId,
    pub visibility: DecisionVisibility,
    pub kind: DecisionKind,
    pub min_selections: u8,
    pub max_selections: u8,
    pub options: Vec<DecisionOption>,
    pub continuation: DecisionContinuation,
}

/// The narrow action vocabulary supplied by a code policy to the engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyMoveKind {
    Cast,
    Draw,
    ChoosePrivateLibraryCards,
    ChoosePrivateOpponentLibraryCardToExile,
    ChooseLibrarySearchCard,
    ChooseTriggeredAbilityTargets,
    ChooseTriggeredAbilityEffectObject,
    ChooseDamageReplacement,
    ResolveOptionalTriggeredAbility,
    SubmitDecision,
    Transmute,
    PassPriority,
    PlayLand,
    ActivateManaAbility,
    ActivateBoundManaAbility,
    ActivateAbility,
    DeclareAttackers,
    DeclareBlockers,
    ReportEngineWeakness,
}

/// One blocker assigned to one attacker. This initial combat substrate permits one
/// blocker per attacker; cards requiring multi-block assignment are reported as a
/// capability gap rather than being approximated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CombatBlock {
    pub attacker: ObjectId,
    pub blocker: ObjectId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
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

impl Step {
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Untap => Self::Upkeep,
            Self::Upkeep => Self::Draw,
            Self::Draw => Self::PrecombatMain,
            Self::PrecombatMain => Self::BeginningOfCombat,
            Self::BeginningOfCombat => Self::DeclareAttackers,
            Self::DeclareAttackers => Self::DeclareBlockers,
            Self::DeclareBlockers => Self::FirstStrikeCombatDamage,
            Self::FirstStrikeCombatDamage => Self::CombatDamage,
            Self::CombatDamage => Self::EndOfCombat,
            Self::EndOfCombat => Self::PostcombatMain,
            Self::PostcombatMain => Self::End,
            Self::End => Self::Cleanup,
            Self::Cleanup => Self::Untap,
        }
    }

    #[must_use]
    pub const fn is_main(self) -> bool {
        matches!(self, Self::PrecombatMain | Self::PostcombatMain)
    }

    /// Untap and ordinary cleanup are automatic turn-based steps in this
    /// substrate. No player may receive priority while either is stable.
    #[must_use]
    pub const fn grants_priority(self) -> bool {
        !matches!(self, Self::Untap | Self::Cleanup)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerState {
    pub id: PlayerId,
    /// Magic life totals are unbounded by the rules; the engine uses a wide
    /// signed representation instead of coupling them to effect amounts.
    pub life: i64,
    pub library: Vec<ObjectId>,
    pub hand: Vec<ObjectId>,
    pub battlefield: Vec<ObjectId>,
    pub graveyard: Vec<ObjectId>,
    pub exile: Vec<ObjectId>,
    pub mana_pool: ManaPool,
    pub lands_played: u8,
    pub lost: bool,
}

impl PlayerState {
    pub(crate) fn new(id: PlayerId) -> Self {
        Self {
            id,
            life: 20,
            library: Vec::new(),
            hand: Vec::new(),
            battlefield: Vec::new(),
            graveyard: Vec::new(),
            exile: Vec::new(),
            mana_pool: ManaPool::default(),
            lands_played: 0,
            lost: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StackObject {
    pub card: ObjectId,
    /// The exact incarnation that became this spell or produced this ability.
    /// An ability may resolve after its source left the battlefield, but it
    /// must never treat a later incarnation with the same public id as its
    /// historical source for source-relative instructions.
    pub source_incarnation: u64,
    /// Colors the source had when this spell or ability became a stack
    /// object.  A source can leave the battlefield, which removes its
    /// continuous characteristic effects, before target legality or damage
    /// prevention is checked at resolution.  This retained last-known
    /// information keeps source-color rules (including protection) tied to
    /// the source incarnation that actually created the stack object.
    pub source_colors: BTreeSet<Color>,
    pub controller: PlayerId,
    /// `None` denotes a spell; `Some` denotes a non-mana activated ability
    /// whose source is `card` and whose printed identity is the bound id.
    pub ability_id: Option<&'static str>,
    pub targets: Vec<Target>,
    /// One captured object incarnation per target occurrence. `None` is used
    /// for player targets, preserving the public target enum while retaining
    /// zone-change identity for permanent/spell references.
    pub target_incarnations: Vec<Option<u64>>,
    pub effects: Vec<Effect>,
    /// The value chosen for X while casting this spell.  Abilities and spells
    /// without an X instruction retain `None`; the value is authoritative at
    /// resolution and is never reconstructed from payment colors.
    pub chosen_x: Option<u8>,
    /// Full color receipt for an explicitly selected spell or activated-ability
    /// payment. `None` denotes the legacy deterministic payment path, which
    /// is deliberately unavailable to effects that inspect colors spent.
    pub mana_spent: Option<Vec<Color>>,
    /// Number of cost symbols paid by Convoke while this spell was cast. The
    /// stack retains this cast-time provenance because a source of a generic
    /// reduction may leave the battlefield before the spell resolves.
    pub convoke_symbols: usize,
    /// Generic cost reduction actually applied after a chosen X value and
    /// before Convoke. This is a cost-payment fact, not a live query of a
    /// source that might later leave the battlefield.
    pub generic_cost_reduction: u8,
}

/// The resolution status for the target occurrence, if any, owned by one
/// effect in a stack object. This is deliberately aligned one-for-one with
/// `StackObject::effects`, rather than with the deduplicated set of objects
/// named as targets: one permanent can legally occupy several target slots.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StackEffectResolution {
    Untargeted,
    Targeted { target: Target, legal: bool },
}

/// The initial target-resolution decision for one stack object.
///
/// The game evaluates target legality once immediately before it begins
/// resolving effects, making the all-targets-illegal rules-counter boundary
/// explicit. An initially legal occurrence is dynamically rechecked before its
/// own instruction because an earlier instruction can remove that target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StackResolutionPlan {
    CounteredByRules,
    Resolve { effects: Vec<StackEffectResolution> },
}

/// The stack object did not retain the same number of target slots as the
/// target-bearing effects it was cast with. This is an engine-integrity
/// failure, not a dynamic target-legality result.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StackTargetArityError {
    pub expected: usize,
    pub actual: usize,
}

impl StackObject {
    /// Returns the target-slot count mandated by this object's effect list.
    #[must_use]
    pub fn target_count(&self) -> usize {
        self.effects
            .iter()
            .filter(|effect| effect.target_requirement().is_some())
            .count()
    }

    /// Produces the one-shot target decision used by a stack resolution.
    ///
    /// A caller supplies the current game-dependent legality predicate. The
    /// returned plan preserves every target occurrence in effect order and
    /// calls the predicate exactly once per occurrence. It never rewrites the
    /// target vector, so independent repeated selections remain independent.
    pub fn resolution_plan(
        &self,
        mut target_is_legal: impl FnMut(Target, TargetRequirement) -> bool,
    ) -> Result<StackResolutionPlan, StackTargetArityError> {
        let expected = self.target_count();
        if self.targets.len() != expected {
            return Err(StackTargetArityError {
                expected,
                actual: self.targets.len(),
            });
        }

        let mut targets = self.targets.iter().copied();
        let mut has_target = false;
        let mut has_legal_target = false;
        let effects = self
            .effects
            .iter()
            .map(|effect| match effect.target_requirement() {
                None => StackEffectResolution::Untargeted,
                Some(requirement) => {
                    has_target = true;
                    // The exact arity check above proves this is present.
                    let target = targets.next().expect("target occurrence is present");
                    let legal = target_is_legal(target, requirement);
                    has_legal_target |= legal;
                    StackEffectResolution::Targeted { target, legal }
                }
            })
            .collect();

        if has_target && !has_legal_target {
            Ok(StackResolutionPlan::CounteredByRules)
        } else {
            Ok(StackResolutionPlan::Resolve { effects })
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GameEvent {
    PolicyMoveSubmitted {
        player: PlayerId,
        policy: String,
        kind: PolicyMoveKind,
    },
    /// A typed no-priority decision opened. Candidate identities and the
    /// submitted answer deliberately remain absent from the public receipt.
    DecisionOpened {
        decision: DecisionId,
        player: PlayerId,
        kind: DecisionKind,
        visibility: DecisionVisibility,
        min_selections: u8,
        max_selections: u8,
    },
    /// A typed decision completed successfully. Effect-specific public
    /// receipts, such as a sacrifice or search movement, follow through the
    /// continuation without revealing private unselected candidates.
    DecisionCompleted {
        decision: DecisionId,
        player: PlayerId,
        kind: DecisionKind,
    },
    /// One controller's complete CR 603.3b ordering for an APNAP
    /// simultaneous-trigger group. This follows the corresponding generic
    /// decision completion and precedes any member entering the stack.
    TriggeredAbilityOrderChosen {
        controller: PlayerId,
        order: Vec<TriggerOrderEntry>,
    },
    CardMoved {
        card: ObjectId,
        to: Zone,
    },
    /// A physical card moved to a new zone (including the stack) and became a
    /// new rules object.  The stable id remains public; this receipt carries
    /// the monotonic incarnation needed to replay stack and effect provenance
    /// without conflating a returned object with its former self.
    ObjectIncarnationAdvanced {
        object: ObjectId,
        incarnation: u64,
    },
    /// A live static entry restriction changed an ordinary battlefield entry
    /// from untapped to tapped. The zone and incarnation receipts immediately
    /// before this event remain the authoritative transition provenance.
    PermanentEnteredTapped {
        permanent: ObjectId,
        permanent_incarnation: u64,
        controller: PlayerId,
        source: ObjectId,
        source_incarnation: u64,
    },
    /// The named controller inspected these currently top library cards while
    /// a resolving instruction was suspended for a private choice. The cards'
    /// identities are not exposed through an opponent `GameView`.
    CardsLookedAt {
        viewer: PlayerId,
        cards: Vec<ObjectId>,
    },
    /// A resolving ability opened a private inspection of a target opponent's
    /// library. Candidate identities deliberately stay out of the canonical
    /// public event log; the resulting exile zone movement remains public.
    PrivateOpponentLibraryChoiceOpened {
        controller: PlayerId,
        source: ObjectId,
        source_incarnation: u64,
        ability: &'static str,
        opponent: PlayerId,
        count: u8,
    },
    /// A resolving private-library choice paid life for the selected cards.
    /// This is distinct from damage and from a mana-ability life-payment cost.
    LifePaid {
        source: ObjectId,
        player: PlayerId,
        amount: i16,
    },
    /// A resolving effect made a player discard the named hand card before it
    /// moved to that player's graveyard.
    CardDiscarded {
        player: PlayerId,
        card: ObjectId,
    },
    /// A resolving effect sacrificed a controller-owned creature before its
    /// ordinary zone-change lifecycle completed.
    SacrificedByEffect {
        source: ObjectId,
        player: PlayerId,
        permanent: ObjectId,
    },
    CardDestroyed {
        source: ObjectId,
        card: ObjectId,
    },
    /// A resolving spell or ability placed a persistent, positive counter on
    /// a battlefield permanent. Counters clear when that permanent leaves the
    /// battlefield, so the receipt never implies a zone-independent modifier.
    CounterPlaced {
        source: ObjectId,
        card: ObjectId,
        counter: CounterKind,
        amount: i16,
    },
    /// A resolving spell or ability removed a persistent positive quantity of
    /// one typed counter from a live battlefield permanent. The final count
    /// either remains positive or its key is removed entirely.
    CounterRemoved {
        source: ObjectId,
        card: ObjectId,
        counter: CounterKind,
        amount: i16,
    },
    /// One live permanent replaced an event quantity before the corresponding
    /// token-creation or counter-placement receipts were emitted. `source` is
    /// the replacement source, not the source that caused the original event.
    ReplacementEffectApplied {
        source: ObjectId,
        source_incarnation: u64,
        affected_player: PlayerId,
        event: ReplacementEventKind,
        original_amount: i16,
        replacement_amount: i16,
    },
    /// A resolving spell or ability created a source-identified regeneration
    /// replacement shield on a live creature.
    RegenerationShieldCreated {
        source: ObjectId,
        target: ObjectId,
    },
    /// A pending regeneration replacement prevented destruction, tapped the
    /// target, removed marked damage, and removed it from combat.
    RegenerationShieldUsed {
        source: ObjectId,
        target: ObjectId,
    },
    ManaAdded {
        player: PlayerId,
        color: Color,
        amount: u8,
    },
    ManaAbilityActivated {
        player: PlayerId,
        land: ObjectId,
        color: Color,
    },
    /// A typed basic land activated as one explicitly selected step while a
    /// spell's cost is paid. This receipt precedes the intrinsic activation
    /// and mana-output receipts; it is not a stack object.
    CastPaymentBasicLandManaAbilityActivated {
        player: PlayerId,
        card: ObjectId,
        land: ObjectId,
        color: Color,
    },
    /// A definition-bound mana ability declared in the typed payment context
    /// of one spell cast. This receipt precedes that ability's normal bound
    /// activation and mana-output events; it is not a stack object.
    CastPaymentManaAbilityActivated {
        player: PlayerId,
        card: ObjectId,
        source: ObjectId,
        ability: &'static str,
    },
    /// A selected permanent is being sacrificed as an explicit additional
    /// cost for this spell. It is immediately followed by its zone-change
    /// receipt and always precedes this spell's `SpellCast` receipt.
    SacrificedAsAdditionalSpellCost {
        player: PlayerId,
        card: ObjectId,
        permanent: ObjectId,
    },
    /// Receipt for the generic definition-bound mana-ability substrate. It is
    /// intentionally distinct from the legacy intrinsic-land receipt above.
    BoundManaAbilityActivated {
        player: PlayerId,
        source: ObjectId,
        ability: &'static str,
        color: Color,
        amount: u8,
        tapped: bool,
        life_payment: Option<u8>,
    },
    /// Receipt for a paid, fixed multi-color mana bundle. This preserves the
    /// legacy single-color receipt for existing bindings while making the cost
    /// and every produced color auditable for Signet-style abilities.
    BoundManaAbilityBundleActivated {
        player: PlayerId,
        source: ObjectId,
        ability: &'static str,
        mana_cost: ManaCost,
        bundle: ManaBundle,
        tapped: bool,
        life_payment: Option<u8>,
    },
    /// Receipt for a fixed multi-color mana bundle that has no mana-payment
    /// cost. This remains separate from `BoundManaAbilityBundleActivated` so
    /// the event log cannot falsely claim that a zero-cost payment occurred.
    BoundManaAbilityFreeBundleActivated {
        player: PlayerId,
        source: ObjectId,
        ability: &'static str,
        bundle: ManaBundle,
        tapped: bool,
        life_payment: Option<u8>,
    },
    /// Mana spent as an activation cost for a bound paid-bundle ability.
    ManaAbilityManaPaid {
        player: PlayerId,
        mana_cost: ManaCost,
    },
    /// A life payment made as part of a bound mana-ability activation cost.
    ManaAbilityLifePaid {
        player: PlayerId,
        amount: u8,
    },
    DeckLoaded {
        player: PlayerId,
        cards: u16,
    },
    LibraryShuffled {
        player: PlayerId,
        cards: u16,
    },
    /// One represented search instruction completed. `None` means the search
    /// found nothing (including a current-turn search-prevention effect), but
    /// the required following shuffle retains its own receipt.
    LibrarySearchResolved {
        player: PlayerId,
        source: ObjectId,
        found: Option<ObjectId>,
        destination: LibrarySearchDestination,
    },
    /// One policy-submitted multi-card library search completed.  `found`
    /// retains the policy's selected order so replay can prove both the
    /// selected set and every following ordinary zone move without exposing
    /// candidates that were not selected.
    LibrarySearchBatchResolved {
        player: PlayerId,
        source: ObjectId,
        found: Vec<ObjectId>,
        destination: LibrarySearchDestination,
    },
    /// A public top-library decision established the exact new top-to-bottom
    /// order of the revealed cards.  The cards themselves were already made
    /// public through preceding `CardRevealed` receipts.
    LibraryReordered {
        player: PlayerId,
        top_to_bottom: Vec<ObjectId>,
    },
    /// A resolving Aura-like permanent established its explicit attachment
    /// after entering the battlefield and after its final persistent layer
    /// effect was installed.
    AuraAttached {
        aura: ObjectId,
        target: ObjectId,
    },
    /// A typed Equipment attached to a legal permanent through a resolving
    /// activated ability. `previous` preserves reattachment provenance
    /// without turning an old attachment into a live endpoint.
    EquipmentAttached {
        equipment: ObjectId,
        target: ObjectId,
        previous: Option<ObjectId>,
    },
    /// A legal Aura or Equipment attachment established no persistent layer
    /// effect. This is distinct from an attachment receipt that must follow a
    /// `ContinuousEffectCreated` record, so replay can preserve the complete
    /// lifecycle of pure attachments such as Flickerform.
    AttachmentEstablishedWithoutContinuousEffect {
        attachment: ObjectId,
        target: ObjectId,
        kind: AttachmentKind,
        previous: Option<ObjectId>,
    },
    /// A non-Aura attachment stopped modifying its former target. Equipment
    /// stays on the battlefield unattached; Auras instead use their ordinary
    /// state-based graveyard transition.
    AttachmentDetached {
        attachment: ObjectId,
        target: ObjectId,
        kind: AttachmentKind,
        reason: &'static str,
    },
    /// A resolver created one exact-incarnation exile group and scheduled its
    /// typed return continuation. Ordinary `CardMoved` receipts remain the
    /// zone truth; this receipt supplies the link and delayed-action
    /// provenance that replay cannot infer from unrelated exile moves.
    DelayedActionScheduled {
        action: DelayedActionId,
        timing: DelayedActionTiming,
        due_turn: u32,
        controller: PlayerId,
        group: LinkedExileGroupId,
        members: Vec<LinkedExileMember>,
    },
    /// The named delayed continuation was consumed exactly once. `returned`
    /// contains only members that were still in exile with the captured
    /// incarnation when the action executed.
    DelayedActionConsumed {
        action: DelayedActionId,
        group: LinkedExileGroupId,
        returned: Vec<ObjectId>,
    },
    OpeningHandDrawn {
        player: PlayerId,
        cards: u8,
    },
    /// Exact colors consumed from the controller's mana pool while one spell
    /// was cast through an explicit payment selection. This is recorded before
    /// `SpellCast` and copied to its stack object for resolution-time effects.
    SpellManaPaid {
        player: PlayerId,
        card: ObjectId,
        colors: Vec<Color>,
    },
    SpellCast {
        player: PlayerId,
        card: ObjectId,
    },
    /// A non-mana activated ability entered the stack. `source` remains on
    /// the battlefield while this stack object resolves.
    AbilityActivated {
        player: PlayerId,
        source: ObjectId,
        /// The source's incarnation at activation time.  A stable object ID
        /// can leave and return before its ability resolves.
        source_incarnation: u64,
        /// Effective definition-bound identity at activation time. A copied
        /// permanent can later change zones and resume its printed definition,
        /// so replay must not look up this historical ability from mutable
        /// current characteristics.
        definition: &'static str,
        ability: &'static str,
    },
    TriggeredAbilityStacked {
        controller: PlayerId,
        source: ObjectId,
        /// The historical incarnation that caused this trigger.  Dies
        /// triggers normally name a prior battlefield incarnation.
        source_incarnation: u64,
        ability: &'static str,
    },
    AbilityManaPaid {
        player: PlayerId,
        source: ObjectId,
        ability: &'static str,
        mana_cost: ManaCost,
    },
    /// A positive life payment made while activating a stack-using ability.
    /// This is distinct from effect loss of life and from mana-ability life
    /// costs, so replay can preserve the complete activated-cost transaction.
    AbilityLifePaid {
        player: PlayerId,
        source: ObjectId,
        ability: &'static str,
        amount: u8,
    },
    /// A typed counter was selected and removed as an activation cost. The
    /// ordinary `CounterRemoved` receipt immediately follows this provenance
    /// receipt and carries the authoritative state mutation.
    CounterRemovedAsAbilityCost {
        player: PlayerId,
        source: ObjectId,
        card: ObjectId,
        counter: CounterKind,
        amount: i16,
    },
    /// A selected permanent returned to its owner's hand as an activation
    /// cost. The matching zone-change receipt immediately follows.
    ReturnedAsAbilityCost {
        player: PlayerId,
        source: ObjectId,
        permanent: ObjectId,
    },
    /// The policy chose this nonnegative value for one activated ability's
    /// additional generic `{X}` cost. It remains attached to the stack item
    /// as immutable activation provenance.
    AbilityXCostChosen {
        player: PlayerId,
        source: ObjectId,
        ability: &'static str,
        x: u8,
    },
    /// A live source changed the generic portion of an activated ability's
    /// mana cost.  This receipt is emitted immediately before the matching
    /// mana-payment receipt (when one remains) and preserves enough context
    /// for replay to audit the printed cost, live modifier sources, and final
    /// payable cost without inspecting private engine bindings.
    ActivatedAbilityCostCalculated {
        context: ActivatedAbilityCostContext,
    },
    /// A controlled creature was selected and tapped as an explicit
    /// additional cost for a non-mana activated ability.
    AdditionalCreatureTappedAsAbilityCost {
        player: PlayerId,
        source: ObjectId,
        permanent: ObjectId,
    },
    /// A resolving spell or ability changed a target creature from untapped
    /// to tapped. Unlike an ability-cost receipt, this is a stack effect.
    PermanentTapped {
        source: ObjectId,
        card: ObjectId,
    },
    /// A resolving spell or ability untapped a battlefield permanent. This is
    /// distinct from the automatic untap step, which has its own turn receipt.
    PermanentUntapped {
        source: ObjectId,
        card: ObjectId,
    },
    SacrificedAsAbilityCost {
        player: PlayerId,
        source: ObjectId,
        permanent: ObjectId,
    },
    /// A selected hand card is discarded as an explicit activated-ability
    /// cost. The receipt precedes the ability's activation receipt and the
    /// card's `CardMoved { to: Graveyard }` receipt.
    DiscardedAsAbilityCost {
        player: PlayerId,
        source: ObjectId,
        card: ObjectId,
    },
    ConvokeUsed {
        player: PlayerId,
        creature: ObjectId,
        contribution: Option<Color>,
    },
    PriorityPassed {
        player: PlayerId,
    },
    SpellResolved {
        card: ObjectId,
    },
    AbilityResolved {
        source: ObjectId,
        source_incarnation: u64,
        ability: &'static str,
    },
    SpellCounteredByRules {
        card: ObjectId,
    },
    /// A resolving effect created a virtual stack object from the current
    /// values of an instant or sorcery. `copy` is a fresh virtual identity;
    /// `original` remains the physical card (or earlier copy) whose values
    /// were cloned and is never moved by this receipt.
    SpellCopied {
        copy: ObjectId,
        original: ObjectId,
        original_source_incarnation: u64,
        controller: PlayerId,
        retargeted: bool,
    },
    /// A virtual spell copy completed resolution. Copies have no card-zone
    /// terminal move, so this is intentionally distinct from `SpellResolved`.
    SpellCopyResolved {
        copy: ObjectId,
        original: ObjectId,
    },
    /// Every target of a virtual spell copy was illegal at resolution. The
    /// copy ceases to exist without moving the original physical card.
    SpellCopyCounteredByRules {
        copy: ObjectId,
        original: ObjectId,
    },
    /// A player left a continuing multiplayer game while controlling this
    /// stack-only spell copy. Copies have no owner-zone transition, so this
    /// distinct terminal receipt prevents the departed controller from being
    /// silently retained by the stack.
    SpellCopyLeftGame {
        copy: ObjectId,
        original: ObjectId,
        controller: PlayerId,
    },
    AbilityCounteredByRules {
        source: ObjectId,
        source_incarnation: u64,
        ability: &'static str,
    },
    /// A spell still resolved because it retained another legal target, but
    /// this particular target-bearing instruction did nothing. The stable
    /// effect index identifies the original target occurrence in the card's
    /// executable definition, including repeated selections of the same
    /// object.
    TargetInstructionSkipped {
        card: ObjectId,
        effect_index: usize,
        target: Target,
    },
    /// A spell was countered by a resolving effect, rather than because every
    /// target became illegal under the rules.
    SpellCountered {
        card: ObjectId,
        source: ObjectId,
    },
    SpellCastFromGraveyard {
        player: PlayerId,
        card: ObjectId,
    },
    GraveyardCastPermissionGranted {
        source: ObjectId,
        player: PlayerId,
        card: ObjectId,
        until_turn: u32,
    },
    GraveyardCastPermissionExpired {
        card: ObjectId,
    },
    /// A typed effect-created cast permission was installed for an exact card
    /// incarnation in a named public zone. The source provenance lets a replay
    /// distinguish two same-card permissions granted in one turn.
    CastPermissionGranted {
        source: ObjectId,
        source_incarnation: u64,
        player: PlayerId,
        card: ObjectId,
        zone: CastPermissionZone,
        payment: CastPermissionPayment,
        timing: CastTiming,
        until_turn: u32,
    },
    /// A spell used a previously granted effect-created permission. It is a
    /// casting receipt, never a second card movement receipt.
    SpellCastFromPermission {
        player: PlayerId,
        card: ObjectId,
        from: Zone,
    },
    /// A permission expired at cleanup without being used. A zone-changing
    /// card instead has its stale permission silently revoked as part of that
    /// atomic transition; it cannot survive to this receipt.
    CastPermissionExpired {
        card: ObjectId,
        from: Zone,
    },
    DamageDealtToPlayer {
        source: ObjectId,
        player: PlayerId,
        amount: i32,
    },
    /// A non-damage effect made one target player lose life.
    LifeLost {
        source: ObjectId,
        player: PlayerId,
        amount: i16,
    },
    DamageDealtToPermanent {
        source: ObjectId,
        permanent: ObjectId,
        amount: i32,
    },
    DamageRedirected {
        source: ObjectId,
        from: ObjectId,
        to: Target,
        amount: i32,
    },
    /// The affected player chose this applicable replacement while a bounded
    /// prospective damage event waited at a no-priority decision boundary.
    /// The following prevention, redirection, or ordinary damage receipt is
    /// the authoritative committed consequence.
    DamageReplacementApplied {
        affected_player: PlayerId,
        target: Target,
        replacement: DamageReplacementChoice,
    },
    DamagePrevented {
        source: ObjectId,
        target: Target,
        amount: i32,
    },
    /// A resolving spell or ability created a one-shot damage-prevention
    /// shield for a target player or creature.
    DamageShieldCreated {
        source: ObjectId,
        target: Target,
        amount: i32,
    },
    /// A target-specific damage shield expired or was removed with its target
    /// leaving the battlefield before it was consumed.
    DamageShieldExpired {
        source: ObjectId,
        target: Target,
    },
    LifeGained {
        player: PlayerId,
        amount: i16,
    },
    TokenCreated {
        player: PlayerId,
        token: ObjectId,
    },
    TokenCeasedToExist {
        token: ObjectId,
    },
    /// A live permanent received a layer-one snapshot of another permanent's
    /// copiable values.  The source incarnation is provenance only: later
    /// source zone changes do not revoke the copied values.
    PermanentCopied {
        source: ObjectId,
        source_incarnation: u64,
        target: ObjectId,
        target_incarnation: u64,
        timestamp: u64,
    },
    /// A copied permanent left the battlefield, so that incarnation's
    /// layer-one snapshot ceased.  The ordinary zone and incarnation receipts
    /// remain the source of truth for the new object.
    PermanentCopyExpired {
        target: ObjectId,
        target_incarnation: u64,
        timestamp: u64,
    },
    ObjectLeftGame {
        object: ObjectId,
        owner: PlayerId,
    },
    ContinuousEffectCreated {
        source: ObjectId,
        target: ObjectId,
        layer: Layer,
    },
    ContinuousEffectExpired {
        source: ObjectId,
        target: ObjectId,
        layer: Layer,
    },
    /// A layer-two continuous effect changed the current controller of a
    /// battlefield permanent. The permanent remains in its owner's
    /// battlefield-zone vector; this receipt is the replay-visible boundary
    /// for policy views and controller-relative rules.
    ControllerChanged {
        source: ObjectId,
        target: ObjectId,
        from: PlayerId,
        to: PlayerId,
    },
    PermanentsUntapped {
        player: PlayerId,
        cards: Vec<ObjectId>,
    },
    AttackersDeclared {
        player: PlayerId,
        attackers: Vec<ObjectId>,
    },
    BlockersDeclared {
        player: PlayerId,
        assignments: Vec<(ObjectId, ObjectId)>,
    },
    /// The attacking player submitted the complete damage-assignment order
    /// for one multi-block group. Unlike `DecisionCompleted`, this is public
    /// combat information required to replay the later damage batch.
    CombatDamageOrderChosen {
        player: PlayerId,
        attacker: ObjectId,
        blockers: Vec<ObjectId>,
    },
    EngineWeaknessRevealed {
        player: PlayerId,
        code: String,
        detail: String,
    },
    StateBasedAction {
        card: ObjectId,
        reason: &'static str,
    },
    PlayerLost {
        player: PlayerId,
        reason: &'static str,
    },
    GameEnded {
        winner: Option<PlayerId>,
    },
    Dredged {
        player: PlayerId,
        card: ObjectId,
        count: u8,
    },
    /// A card became public while resolving an instruction that requires it to
    /// be shown before it changes zones. `definition` is the public catalog
    /// identifier, deliberately avoiding a copy of card text in the trace.
    CardRevealed {
        player: PlayerId,
        card: ObjectId,
        definition: &'static str,
    },
    Transmuted {
        player: PlayerId,
        discarded: ObjectId,
        /// A hidden-zone quality search may legally choose no matching card.
        found: Option<ObjectId>,
    },
    /// A resolving effect installed a turn-scoped marker consulted by every
    /// represented library-search action. The receipt names the source and
    /// exact current turn so event logs do not hide this rules-state change.
    LibrarySearchesPrevented {
        source: ObjectId,
        until_turn: u32,
    },
    StepBegan {
        turn: u32,
        active_player: PlayerId,
        step: Step,
    },
}
