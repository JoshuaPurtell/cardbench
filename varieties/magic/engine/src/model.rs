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
        self.amounts.iter().copied().sum()
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
        if self.total() < cost.generic {
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
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Target {
    Player(PlayerId),
    Permanent(ObjectId),
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
}

impl Effect {
    #[must_use]
    pub const fn target_requirement(&self) -> Option<TargetRequirement> {
        match self {
            Self::DealDamage { target, .. } => Some(*target),
            Self::ModifyTargetPtUntilEndOfTurn { .. }
            | Self::RadianceUntapAndModifyUntilEndOfTurn { .. } => {
                Some(TargetRequirement::Creature)
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
            if !catalog.contains_key(entry.card.as_str()) {
                return Err(DeckValidationError::UnknownCard(entry.card.clone()));
            }
            sideboard_total += u16::from(entry.count);
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
    PassPriority,
    PlayLand,
    ActivateManaAbility,
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
    ContinuousEffectCreated {
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
