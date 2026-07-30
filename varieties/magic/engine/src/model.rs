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
/// optional life payment, and either a chosen/fixed mana quantity or a paid
/// fixed bundle. It does not encode card names or printed rules text.
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

/// A player's request to activate a bound mana ability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManaAbilityActivation {
    pub source: ObjectId,
    pub ability_id: &'static str,
    /// Required for `ManaAbilityOutput::Choice` and absent for `Fixed` output.
    pub chosen_color: Option<Color>,
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
    Enchantment,
    Instant,
    Land,
    Planeswalker,
    Sorcery,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManaCost {
    pub generic: u8,
    pub colored: Vec<Color>,
}

impl ManaCost {
    #[must_use]
    pub const fn new(generic: u8) -> Self {
        Self {
            generic,
            colored: Vec::new(),
        }
    }

    pub fn with_colors(generic: u8, colored: impl IntoIterator<Item = Color>) -> Self {
        Self {
            generic,
            colored: colored.into_iter().collect(),
        }
    }

    #[must_use]
    pub fn mana_value(&self) -> u8 {
        self.generic
            .saturating_add(u8::try_from(self.colored.len()).unwrap_or(u8::MAX))
    }
}

/// A deterministic five-color mana pool. It intentionally has no floating mana source.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ManaPool {
    amounts: [u8; 5],
}

impl ManaPool {
    pub fn add(&mut self, color: Color, amount: u8) {
        self.amounts[color.index()] = self.amounts[color.index()].saturating_add(amount);
    }

    #[must_use]
    pub const fn amount(&self, color: Color) -> u8 {
        self.amounts[color.index()]
    }

    #[must_use]
    pub fn total(&self) -> u8 {
        self.total_exact().min(u16::from(u8::MAX)) as u8
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
        let mut required = [0_u8; 5];
        for color in &cost.colored {
            required[color.index()] = required[color.index()].saturating_add(1);
        }
        for color in Color::ALL {
            if self.amount(color) < required[color.index()] {
                return Err(format!("missing {color:?} mana"));
            }
        }
        for color in Color::ALL {
            self.amounts[color.index()] -= required[color.index()];
        }
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Keyword {
    Convoke,
    Defender,
    Dredge(u8),
    Transmute(ManaCost),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetRequirement {
    Any,
    Creature,
    Player,
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TokenSpec {
    pub name: &'static str,
    pub colors: BTreeSet<Color>,
    pub card_types: BTreeSet<CardType>,
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
    DealDamageController {
        amount: i16,
    },
    GainLifeController {
        amount: i16,
    },
    CreateToken {
        token: TokenSpec,
        count: u8,
    },
    ModifyTargetPtUntilEndOfTurn {
        power: i16,
        toughness: i16,
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
    /// Counter one targeted instant or sorcery spell. This is intentionally a
    /// semantic effect rather than a copied card-text string.
    CounterTargetInstantOrSorcerySpell,
}

impl Effect {
    #[must_use]
    pub const fn target_requirement(&self) -> Option<TargetRequirement> {
        match self {
            Self::DealDamage { target, .. } => Some(*target),
            Self::ModifyTargetPtUntilEndOfTurn { .. }
            | Self::RadianceUntapAndModifyUntilEndOfTurn { .. }
            | Self::RadianceModifyPtUntilEndOfTurn { .. } => Some(TargetRequirement::Creature),
            Self::CounterTargetInstantOrSorcerySpell => {
                Some(TargetRequirement::InstantOrSorcerySpell)
            }
            Self::DealDamageController { .. }
            | Self::GainLifeController { .. }
            | Self::CreateToken { .. } => None,
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
    pub damage: i16,
    pub counters: BTreeMap<&'static str, i16>,
    pub entered_turn: u32,
    pub token: Option<TokenSpec>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Characteristics {
    pub colors: BTreeSet<Color>,
    pub card_types: BTreeSet<CardType>,
    pub power: Option<i16>,
    pub toughness: Option<i16>,
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
    ModifyPowerToughness { power: i16, toughness: i16 },
}

impl ContinuousChange {
    #[must_use]
    pub const fn layer(&self) -> Layer {
        match self {
            Self::AddCardType(_) => Layer::Type,
            Self::AddColor(_) => Layer::Color,
            Self::AddKeyword(_) => Layer::Ability,
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
            Self::DeclareBlockers => Self::CombatDamage,
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
    pub life: i16,
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
    pub targets: Vec<Target>,
    pub effects: Vec<Effect>,
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
    SpellCast {
        player: PlayerId,
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
    SpellCounteredByRules {
        card: ObjectId,
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
        amount: i16,
    },
    DamageDealtToPermanent {
        source: ObjectId,
        permanent: ObjectId,
        amount: i16,
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
    Transmuted {
        player: PlayerId,
        discarded: ObjectId,
        found: ObjectId,
    },
    StepBegan {
        turn: u32,
        active_player: PlayerId,
        step: Step,
    },
}
