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
    pub sacrifice_source: bool,
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
    /// The source dealt positive damage to a player or permanent. The damage
    /// amount is materialized into the triggered stack object's effects when
    /// the receipt is emitted, so a life-gain trigger cannot inspect a later
    /// or unrelated damage event.
    DealsDamage,
    /// The source received positive damage. The source may leave the
    /// battlefield during state-based actions before this trigger is stacked.
    ReceivesDamage,
    /// The source changed from the battlefield to its graveyard.
    Dies,
    /// The source was declared as an attacker. Optional trigger costs are
    /// paid from the controller's pool when the trigger is stacked.
    Attacks,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TriggeredAbility {
    pub id: &'static str,
    pub condition: TriggerCondition,
    /// Optional mana paid while the trigger is put on the stack. This keeps
    /// attack-trigger payment separate from the resolving effect.
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
    pub const ALL: [Self; 5] = [Self::White, Self::Blue, Self::Black, Self::Red, Self::Green];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::White => 0,
            Self::Blue => 1,
            Self::Black => 2,
            Self::Red => 3,
            Self::Green => 4,
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

/// A deterministic five-color mana pool. It intentionally has no floating mana source.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ManaPool {
    amounts: [u8; 5],
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

    /// Returns the complete pool total widened enough for all five color slots.
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
        self.amounts = [0; 5];
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
        let mut required = [0_u16; 5];
        for color in &cost.colored {
            required[color.index()] = required[color.index()].saturating_add(1);
        }
        for color in Color::ALL {
            if u16::from(self.amount(color)) < required[color.index()] {
                return Err(format!("missing {color:?} mana"));
            }
        }
        for color in Color::ALL {
            let spent = u8::try_from(required[color.index()])
                .expect("a payable bounded colored cost fits its source pool");
            self.amounts[color.index()] -= spent;
        }
        self.pay_hybrid_symbols(&cost.hybrid)?;
        if self.total_exact() < u16::from(cost.generic) {
            return Err("missing generic mana".to_owned());
        }
        let mut remaining = cost.generic;
        for color in Color::ALL {
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
    /// order-independent while preserving the five-color pool boundary.
    fn pay_hybrid_symbols(&mut self, symbols: &[HybridManaSymbol]) -> Result<(), String> {
        let mut remaining = self.amounts.map(u16::from);
        let mut assignments = vec![None; symbols.len()];
        for index in 0..symbols.len() {
            let mut seen_colors = [false; 5];
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
        for color in Color::ALL {
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
        remaining: &mut [u16; 5],
        seen_colors: &mut [bool; 5],
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
    /// This creature can't be blocked while the defending player controls the
    /// named basic land type.
    Mountainwalk,
    /// This creature assigns combat damage in both first-strike and normal
    /// combat-damage steps.
    DoubleStrike,
    /// Damage dealt by this source ignores prevention and redirection effects.
    DamageCannotBePrevented,
    /// Damage dealt by a source of the named color is prevented when it would
    /// be dealt to this permanent.
    PreventDamageFromColor(Color),
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
    Creature,
    /// A battlefield creature currently assigned as a blocker in the active
    /// combat. This preserves the narrower target restriction of sacrifice
    /// damage abilities such as War-Torch Goblin.
    BlockingCreature,
    Land,
    /// A battlefield permanent with the Artifact card type.
    Artifact,
    Player,
    /// A player or battlefield creature, matching the executable pre-
    /// planeswalker direct-damage card slice.
    PlayerOrCreature,
    /// A nonpermanent spell card currently on the stack. This deliberately
    /// names the narrow RAV counterspell slice instead of claiming support for
    /// arbitrary abilities or every kind of spell target.
    InstantOrSorcerySpell,
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
/// The initial RAV substrate needs only Saproling, but this remains a typed
/// semantic field rather than treating a display name as a rules identity.
/// Future set modules can extend the enum as they introduce token-specific
/// interactions.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CreatureSubtype {
    Knight,
    Saproling,
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
}

/// Effects are executable semantics, not copied Oracle wording.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    DealDamage {
        amount: i16,
        target: TargetRequirement,
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
    /// Add a fixed amount of one color to the resolving spell controller's
    /// mana pool. This is a stack effect (not a mana ability), used by
    /// Seismic Spike after its targeted land destruction resolves.
    AddManaController {
        color: Color,
        amount: u8,
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
    /// Gain life equal to the positive damage amount that caused this
    /// source-specific triggered ability to fire. This is intentionally a
    /// semantic operation rather than copied card text; the trigger queue
    /// materializes it into `GainLifeController` before the ability resolves.
    GainLifeControllerFromSourceDamage,
    /// Draw one card for the controller when this effect resolves. This is
    /// intentionally a stack-only operation so public live-game setup cannot
    /// inject cards into a hand after the game has begun.
    DrawController,
    /// Materialized by a recipient-damage trigger after the source object has
    /// received positive damage. The amount is captured at receipt time.
    DealDamageToEachPlayerFromReceivedDamage,
    /// Draw one card only when this spell's explicit cast-payment receipt
    /// contains the named mana color. The receipt belongs to the stack object,
    /// so later floating mana or post-cast pool changes cannot affect it.
    DrawControllerIfManaColorSpent {
        color: Color,
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
    /// Destroy the targeted land during resolution, sending it through the
    /// normal zone-change and continuous-effect lifecycle.
    DestroyTargetLand,
    /// Destroy the targeted artifact during resolution, sending it through
    /// the normal zone-change and continuous-effect lifecycle.
    DestroyTargetArtifact,
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
    /// Counter one targeted instant or sorcery spell. This is intentionally a
    /// semantic effect rather than a copied card-text string.
    CounterTargetInstantOrSorcerySpell,
}

impl Effect {
    /// Whether resolving this instruction is defined only from the spell's
    /// explicit mana-payment receipt rather than a deterministic pool drain.
    #[must_use]
    pub const fn requires_explicit_mana_spend(&self) -> bool {
        matches!(self, Self::DrawControllerIfManaColorSpent { .. })
    }

    #[must_use]
    pub const fn target_requirement(&self) -> Option<TargetRequirement> {
        match self {
            Self::DealDamage { target, .. }
            | Self::DealDamageEqualToAttackingCreatures { target } => Some(*target),
            Self::ModifyTargetPtUntilEndOfTurn { .. }
            | Self::ModifyTargetKeywordUntilEndOfTurn { .. }
            | Self::RadianceDealDamageToCreatures { .. }
            | Self::RadianceUntapAndModifyUntilEndOfTurn { .. }
            | Self::RadianceModifyPtUntilEndOfTurn { .. }
            | Self::RadianceAddKeywordUntilEndOfTurn { .. }
            | Self::BeginDamageRedirection { .. }
            | Self::PreventTargetBlockingSourceUntilEndOfTurn => Some(TargetRequirement::Creature),
            Self::CompleteDamageRedirection => Some(TargetRequirement::PlayerOrCreature),
            Self::CreateTokenForTargetPlayer { .. } => Some(TargetRequirement::Player),
            Self::DestroyTargetLand => Some(TargetRequirement::Land),
            Self::DestroyTargetArtifact => Some(TargetRequirement::Artifact),
            Self::CounterTargetInstantOrSorcerySpell => {
                Some(TargetRequirement::InstantOrSorcerySpell)
            }
            Self::DealDamageController { .. }
            | Self::DealDamageToEachCreatureAndPlayer { .. }
            | Self::DealDamageToEachPlayer { .. }
            | Self::DealDamageToEachNonFlyingCreature { .. }
            | Self::GainLifeController { .. }
            | Self::GainLifeControllerFromSourceDamage
            | Self::DrawController
            | Self::DealDamageToEachPlayerFromReceivedDamage
            | Self::AddManaController { .. }
            | Self::DrawControllerIfManaColorSpent { .. }
            | Self::CreateToken { .. }
            | Self::ModifySourcePtUntilEndOfTurn { .. }
            | Self::RemoveSourceKeywordUntilEndOfTurn { .. }
            | Self::AddSourceDamageShieldUntilEndOfTurn { .. }
            | Self::ModifyControllerCreaturesPtUntilEndOfTurn { .. }
            | Self::AddKeywordToControllerCreaturesUntilEndOfTurn { .. } => None,
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
    pub owner: PlayerId,
    pub controller: PlayerId,
    pub tapped: bool,
    /// Marked damage is runtime state, not printed card data.  It is wider
    /// than a card's printed power/toughness so repeated legal effects never
    /// wrap or panic part way through stack resolution.
    pub damage: i32,
    /// Temporary prevention shield units waiting to absorb damage.
    pub damage_shield: i32,
    pub counters: BTreeMap<&'static str, i16>,
    pub entered_turn: u32,
    pub token: Option<TokenSpec>,
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
    Type = 4,
    Color = 5,
    Ability = 6,
    PowerToughness = 7,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContinuousChange {
    AddCardType(CardType),
    AddColor(Color),
    AddKeyword(Keyword),
    RemoveKeyword(Keyword),
    CannotBlockSource(ObjectId),
    AddDamageShield(i16),
    ModifyPowerToughness { power: i16, toughness: i16 },
}

impl ContinuousChange {
    #[must_use]
    pub const fn layer(&self) -> Layer {
        match self {
            Self::AddCardType(_) => Layer::Type,
            Self::AddColor(_) => Layer::Color,
            Self::AddKeyword(_)
            | Self::RemoveKeyword(_)
            | Self::CannotBlockSource(_)
            | Self::AddDamageShield(_) => Layer::Ability,
            Self::ModifyPowerToughness { .. } => Layer::PowerToughness,
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
    pub target: ObjectId,
    pub change: ContinuousChange,
    pub duration: Duration,
    pub timestamp: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Zone {
    Library,
    Hand,
    Battlefield,
    Graveyard,
    Exile,
}

/// The narrow action vocabulary supplied by a code policy to the engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PolicyMoveKind {
    Cast,
    Draw,
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
    pub controller: PlayerId,
    /// `None` denotes a spell; `Some` denotes a non-mana activated ability
    /// whose source is `card` and whose printed identity is the bound id.
    pub ability_id: Option<&'static str>,
    pub targets: Vec<Target>,
    pub effects: Vec<Effect>,
    /// Full color receipt for an explicitly selected spell payment. `None`
    /// denotes the legacy deterministic payment path, which is deliberately
    /// unavailable to effects that inspect colors spent to cast the spell.
    pub mana_spent: Option<Vec<Color>>,
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
    CardMoved {
        card: ObjectId,
        to: Zone,
    },
    CardDestroyed {
        source: ObjectId,
        card: ObjectId,
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
        ability: &'static str,
    },
    TriggeredAbilityStacked {
        controller: PlayerId,
        source: ObjectId,
        ability: &'static str,
    },
    AbilityManaPaid {
        player: PlayerId,
        source: ObjectId,
        ability: &'static str,
        mana_cost: ManaCost,
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
        ability: &'static str,
    },
    SpellCounteredByRules {
        card: ObjectId,
    },
    AbilityCounteredByRules {
        source: ObjectId,
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
    DamageDealtToPlayer {
        source: ObjectId,
        player: PlayerId,
        amount: i32,
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
    DamagePrevented {
        source: ObjectId,
        target: Target,
        amount: i32,
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
    StepBegan {
        turn: u32,
        active_player: PlayerId,
        step: Step,
    },
}
