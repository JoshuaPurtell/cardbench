//! A thin, seat-general board abstraction and a precomputed card index.
//!
//! Every planner in this module tree reads a [`Board`] rather than a
//! `GameView`. Two reasons:
//!
//! 1. `GameView` splits the world into `own_*` and `opponent_*` and flattens
//!    every opponent's permanents into one list. A planner written against
//!    that shape silently assumes two players. [`Board`] keeps opponents as
//!    identified seats, so nothing here acquires a duel assumption.
//! 2. The architecture contract commits the policy layer to eventually reading
//!    `MatchObservation` instead. Keeping one narrow struct between the
//!    planners and the source of truth makes that an adapter rather than a
//!    rewrite.
//!
//! [`CardIndex`] exists because the alternative -- every planner scanning the
//! definition list for a definition id -- is exactly the duplicated card-ID
//! search this work is meant to remove.

use cardbench_magic_engine::{
    ActivatedManaAbility, CardDefinition, CardType, CardView, Color, GameView, Keyword,
    ManaAbilityOutput, ManaCost, ObjectId, PlayerId, Step, TargetRequirement,
};
use std::collections::BTreeMap;

/// What a card is for, as far as a planner needs to care.
///
/// This is a coarse role, not a rules classification. It exists so archetype
/// weighting can say "value removal highly" without naming cards.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Role {
    Land,
    /// A creature whose main job is producing mana.
    ManaCreature,
    Creature,
    /// Destroys or exiles a permanent.
    Removal,
    /// Deals damage that can be aimed at a player.
    Burn,
    /// Creates one or more creature tokens.
    TokenMaker,
    /// Temporarily or permanently modifies power/toughness.
    Pump,
    /// An aura or equipment that attaches to a permanent.
    Attachment,
    Other,
}

/// One mana source's capability, resolved once at index build time.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceKind {
    /// A land with intrinsic basic-land mana, activated by color choice.
    BasicTyped(Vec<Color>),
    /// A bound ability producing exactly one fixed color.
    BoundFixed { ability: &'static str, color: Color },
    /// A bound ability producing one color chosen from a set.
    BoundChoice {
        ability: &'static str,
        colors: Vec<Color>,
    },
    /// A bound ability producing a fixed multi-mana bundle in one activation.
    BoundBundle {
        ability: &'static str,
        bundle: Vec<(Color, u8)>,
    },
}

impl SourceKind {
    /// Every colour this source can contribute, ignoring quantity.
    #[must_use]
    pub fn colors(&self) -> Vec<Color> {
        match self {
            Self::BasicTyped(colors) | Self::BoundChoice { colors, .. } => colors.clone(),
            Self::BoundFixed { color, .. } => vec![*color],
            Self::BoundBundle { bundle, .. } => bundle.iter().map(|(color, _)| *color).collect(),
        }
    }

    /// How much mana one activation yields.
    #[must_use]
    pub fn quantity(&self) -> u8 {
        match self {
            Self::BasicTyped(_) | Self::BoundFixed { .. } | Self::BoundChoice { .. } => 1,
            Self::BoundBundle { bundle, .. } => bundle.iter().map(|(_, amount)| *amount).sum(),
        }
    }
}

/// Everything the planners need to know about one card definition.
#[derive(Clone, Debug)]
pub struct CardFacts {
    pub id: &'static str,
    pub cost: ManaCost,
    pub mana_value: u8,
    pub is_land: bool,
    pub is_creature: bool,
    pub is_instant_speed: bool,
    pub power: i16,
    pub toughness: i16,
    pub keywords: Vec<Keyword>,
    pub role: Role,
    /// Target requirements in spell order. A planner must supply exactly one
    /// target per entry.
    pub targets: Vec<TargetRequirement>,
    /// Total damage this card deals to a single chosen target, if any.
    pub damage_to_target: u8,
    /// Damage this card deals to its own controller as a cost or side effect.
    pub self_damage: u8,
    /// Life this card gains its controller.
    pub life_gain: u8,
    /// Mana production, when this card is a mana source.
    pub source: Option<SourceKind>,
    /// Life this land offers to pay on entry to come in untapped. `None` when
    /// the land has no such choice; the engine rejects an ordinary land play
    /// for a land that does, so this is legality-relevant, not just strategy.
    pub entry_life_payment: Option<u8>,
    /// True when the card cannot be cast by this planner. Recorded rather than
    /// silently skipped so deck validation can refuse to build with it.
    pub unsupported: bool,
}

impl CardFacts {
    /// Whether this card needs the planner to choose targets.
    #[must_use]
    pub fn needs_targets(&self) -> bool {
        !self.targets.is_empty()
    }
}

/// Precomputed facts for the whole catalog, built once per policy.
#[derive(Clone, Debug)]
pub struct CardIndex {
    facts: BTreeMap<&'static str, CardFacts>,
}

impl CardIndex {
    /// Builds the index from a definition list and its mana-ability bindings.
    ///
    /// Bindings are passed in rather than imported so this stays expansion
    /// neutral: nothing here names RAV.
    #[must_use]
    pub fn build(
        definitions: &[CardDefinition],
        mana_bindings: &[(&'static str, ActivatedManaAbility)],
        additional_costs: &[&'static str],
        entry_life_payments: &[(&'static str, u8)],
    ) -> Self {
        let bound: BTreeMap<&'static str, &ActivatedManaAbility> = mana_bindings
            .iter()
            .map(|(id, ability)| (*id, ability))
            .collect();
        let entry: BTreeMap<&'static str, u8> = entry_life_payments.iter().copied().collect();
        let facts = definitions
            .iter()
            .map(|definition| {
                (
                    definition.id,
                    Self::facts_for(
                        definition,
                        bound.get(definition.id).copied(),
                        additional_costs.contains(&definition.id),
                        entry.get(definition.id).copied(),
                    ),
                )
            })
            .collect();
        Self { facts }
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<&CardFacts> {
        self.facts.get(id)
    }

    /// Facts for a card view, when its identity is disclosed.
    #[must_use]
    pub fn view(&self, card: &CardView) -> Option<&CardFacts> {
        card.definition.and_then(|id| self.get(id))
    }

    fn facts_for(
        definition: &CardDefinition,
        bound: Option<&ActivatedManaAbility>,
        has_additional_cost: bool,
        entry_life_payment: Option<u8>,
    ) -> CardFacts {
        let is_land = definition.card_types.contains(&CardType::Land);
        let is_creature = definition.card_types.contains(&CardType::Creature);
        let mana_value = definition.mana_cost.generic
            + u8::try_from(definition.mana_cost.colored.len()).unwrap_or(u8::MAX)
            + u8::try_from(definition.mana_cost.hybrid.len()).unwrap_or(0);

        let mut targets = Vec::new();
        let mut damage_to_target = 0_u8;
        let mut self_damage = 0_u8;
        let mut life_gain = 0_u8;
        for effect in &definition.effects {
            for requirement in effect.target_requirements().into_iter().flatten() {
                targets.push(requirement);
            }
            let rendered = format!("{effect:?}");
            if let Some(amount) = scalar_field(&rendered, "amount") {
                if rendered.starts_with("DealDamageController") {
                    self_damage = self_damage.saturating_add(amount);
                } else if rendered.starts_with("DealDamage") {
                    damage_to_target = damage_to_target.saturating_add(amount);
                } else if rendered.starts_with("GainLife") {
                    life_gain = life_gain.saturating_add(amount);
                }
            }
        }

        let source = Self::source_for(definition, bound);
        let role = Self::role_for(definition, &targets, damage_to_target, source.as_ref());

        CardFacts {
            id: definition.id,
            cost: definition.mana_cost.clone(),
            mana_value,
            is_land,
            is_creature,
            is_instant_speed: definition.card_types.contains(&CardType::Instant),
            power: definition.power.unwrap_or(0),
            toughness: definition.toughness.unwrap_or(0),
            keywords: definition.keywords.clone(),
            role,
            targets,
            damage_to_target,
            self_damage,
            life_gain,
            source,
            // A bound ability whose activation this planner cannot express, or
            // a cost it cannot pay, is marked rather than quietly skipped.
            entry_life_payment,
            // An additional cost this planner cannot pay -- sacrificing a
            // creature, for instance -- makes the card uncastable here. Flagged
            // rather than skipped so deck validation refuses to build with it
            // instead of shipping a deck with dead cards in it.
            unsupported: has_additional_cost
                || bound.is_some_and(|ability| {
                    matches!(
                        ability.output,
                        ManaAbilityOutput::PaidBundle { .. }
                            | ManaAbilityOutput::PaidChoiceBundle { .. }
                    )
                }),
        }
    }

    fn source_for(
        definition: &CardDefinition,
        bound: Option<&ActivatedManaAbility>,
    ) -> Option<SourceKind> {
        if let Some(ability) = bound {
            return match &ability.output {
                ManaAbilityOutput::Fixed(color) => Some(SourceKind::BoundFixed {
                    ability: ability.id,
                    color: *color,
                }),
                ManaAbilityOutput::Choice(colors) => Some(SourceKind::BoundChoice {
                    ability: ability.id,
                    colors: colors.iter().copied().collect(),
                }),
                ManaAbilityOutput::Bundle(bundle) => Some(SourceKind::BoundBundle {
                    ability: ability.id,
                    bundle: bundle
                        .iter()
                        .map(|(color, amount)| (color, amount))
                        .collect(),
                }),
                // A paid bundle costs mana to activate. Planning that needs a
                // second payment solve nested inside the first; it is out of
                // scope and reported through `unsupported`.
                ManaAbilityOutput::PaidBundle { .. }
                | ManaAbilityOutput::PaidChoiceBundle { .. } => None,
            };
        }
        if definition.mana_colors.is_empty() {
            return None;
        }
        Some(SourceKind::BasicTyped(
            definition.mana_colors.iter().copied().collect(),
        ))
    }

    fn role_for(
        definition: &CardDefinition,
        targets: &[TargetRequirement],
        damage_to_target: u8,
        source: Option<&SourceKind>,
    ) -> Role {
        if definition.card_types.contains(&CardType::Land) {
            return Role::Land;
        }
        if definition.card_types.contains(&CardType::Creature) {
            // A creature that taps for mana is a mana source first; its body is
            // incidental and must not be valued as a threat.
            if source.is_some() {
                return Role::ManaCreature;
            }
            return Role::Creature;
        }
        let effects: Vec<String> = definition
            .effects
            .iter()
            .map(|effect| format!("{effect:?}"))
            .collect();
        let names = effects.join(" ");
        if damage_to_target > 0
            && targets
                .iter()
                .any(|requirement| requirement_hits_player(*requirement))
        {
            return Role::Burn;
        }
        if names.contains("Destroy") || names.contains("Exile") {
            return Role::Removal;
        }
        if names.contains("CreateToken") {
            return Role::TokenMaker;
        }
        if names.contains("AttachSource") {
            // An aura is only friendly if it helps. The restriction lives in
            // the effect's continuous changes, not in the card's own keyword
            // list, so reading `definition.keywords` here would classify
            // Faith's Fetters as a buff and aim it at our own creature.
            return if is_hostile_attachment(&names) {
                Role::Removal
            } else {
                Role::Attachment
            };
        }
        if names.contains("ModifyTargetPt") || names.contains("ModifyControllerCreaturesPt") {
            return Role::Pump;
        }
        Role::Other
    }
}

/// Whether a target requirement admits a player.
#[must_use]
pub const fn requirement_hits_player(requirement: TargetRequirement) -> bool {
    matches!(
        requirement,
        TargetRequirement::Any
            | TargetRequirement::Player
            | TargetRequirement::Opponent
            | TargetRequirement::PlayerOrCreature
    )
}

/// Whether a target requirement admits a battlefield creature.
#[must_use]
pub const fn requirement_hits_creature(requirement: TargetRequirement) -> bool {
    matches!(
        requirement,
        TargetRequirement::Any
            | TargetRequirement::Permanent
            | TargetRequirement::Creature
            | TargetRequirement::NonblackCreature
            | TargetRequirement::FlyingCreature
            | TargetRequirement::DistinctCreature
            | TargetRequirement::ArtifactOrCreature
            | TargetRequirement::PlayerOrCreature
    )
}

/// Whether an attachment effect hurts the permanent it lands on.
///
/// Detected from the rendered continuous changes: a negative power or
/// toughness modifier, or a keyword that restricts the creature.
fn is_hostile_attachment(rendered: &str) -> bool {
    rendered.contains("power: -")
        || rendered.contains("toughness: -")
        || rendered.contains("CannotAttackOrBlock")
        || rendered.contains("CannotBlock")
        || rendered.contains("Defender")
}

fn scalar_field(rendered: &str, field: &str) -> Option<u8> {
    let needle = format!("{field}: ");
    let start = rendered.find(&needle)? + needle.len();
    let rest = &rendered[start..];
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// One permanent on the battlefield, with the facts a planner needs inline.
#[derive(Clone, Debug)]
pub struct Permanent {
    pub object: ObjectId,
    pub controller: PlayerId,
    pub definition: Option<&'static str>,
    pub tapped: bool,
    pub can_attack: bool,
    /// Live blocking eligibility, ignoring any individual attacker's evasion.
    pub can_block: bool,
    /// Whether this creature arrived too recently to attack or pay a tap cost.
    pub summoning_sick: bool,
    pub is_creature: bool,
    pub is_land: bool,
    pub power: i16,
    pub toughness: i16,
    pub keywords: Vec<Keyword>,
    pub source: Option<SourceKind>,
    pub role: Role,
    /// Whether this permanent belongs to someone other than the viewer.
    /// Carried on the permanent so an evaluator never needs the whole board
    /// just to know which side a creature is on.
    pub controller_is_opponent: bool,
}

impl Permanent {
    #[must_use]
    pub fn has(&self, keyword: Keyword) -> bool {
        self.keywords.contains(&keyword)
    }

    /// A creature that can profitably be left back to block, i.e. one whose
    /// body is not purely a mana engine.
    #[must_use]
    pub fn is_body(&self) -> bool {
        self.is_creature && self.power + self.toughness > 1
    }
}

/// One card in hand, with its facts resolved.
#[derive(Clone, Debug)]
pub struct HandCard {
    pub object: ObjectId,
    pub definition: &'static str,
    pub facts: CardFacts,
}

/// One opponent, kept identified rather than flattened.
#[derive(Clone, Copy, Debug)]
pub struct Opponent {
    pub seat: PlayerId,
    pub life: i64,
}

/// The planner's view of the table.
#[derive(Clone, Debug)]
pub struct Board {
    pub me: PlayerId,
    pub my_life: i64,
    pub opponents: Vec<Opponent>,
    pub turn: u32,
    pub step: Step,
    pub is_my_turn: bool,
    pub lands_played: u8,
    pub stack_depth: usize,
    pub attackers_declared: bool,
    pub blockers_declared: bool,
    pub hand: Vec<HandCard>,
    /// Every permanent I control.
    pub mine: Vec<Permanent>,
    /// Every permanent an opponent controls, still carrying its controller.
    pub theirs: Vec<Permanent>,
    /// Creatures currently attacking.
    pub attackers: Vec<Permanent>,
    /// Mana already floating, by colour index.
    pub floating: [u8; 6],
}

impl Board {
    /// Projects a `GameView` into the planner's shape.
    #[must_use]
    pub fn from_view(view: &GameView, index: &CardIndex) -> Self {
        let permanent = |card: &CardView| -> Permanent {
            let facts = index.view(card);
            Permanent {
                object: card.id,
                controller: card.controller,
                definition: card.definition,
                tapped: card.tapped,
                can_attack: card.can_attack,
                can_block: card.can_block,
                summoning_sick: card.summoning_sick,
                is_creature: card.card_types.contains(&CardType::Creature),
                is_land: card.card_types.contains(&CardType::Land),
                power: facts.map_or(0, |facts| facts.power),
                toughness: facts.map_or(0, |facts| facts.toughness),
                // Live keywords, not printed ones: a granted evasion keyword
                // or a granted CannotBlock changes what is legal, and the
                // definition would miss both.
                keywords: card.keywords.clone(),
                // Prefer the live view's mana colours: a land whose type was
                // changed on the battlefield produces what the view says, not
                // what the printed definition says.
                controller_is_opponent: card.controller != view.player,
                source: if card.mana_colors.is_empty() {
                    facts.and_then(|facts| facts.source.clone())
                } else {
                    Some(SourceKind::BasicTyped(
                        card.mana_colors.iter().copied().collect(),
                    ))
                },
                role: facts.map_or(Role::Other, |facts| facts.role),
            }
        };

        let mut floating = [0_u8; 6];
        for color in Color::MANA_ALL {
            floating[color.index()] = u8::try_from(view.mana_pool.amount(color)).unwrap_or(u8::MAX);
        }

        Self {
            me: view.player,
            my_life: view.own_life,
            opponents: view
                .opponent_life
                .iter()
                .map(|(seat, life)| Opponent {
                    seat: *seat,
                    life: *life,
                })
                .collect(),
            turn: view.turn,
            step: view.step,
            is_my_turn: view.active_player == view.player,
            lands_played: view.lands_played,
            stack_depth: view.stack_depth,
            attackers_declared: view.attackers_declared,
            blockers_declared: view.blockers_declared,
            hand: view
                .hand
                .iter()
                .filter_map(|card| {
                    let definition = card.definition?;
                    let facts = index.get(definition)?.clone();
                    Some(HandCard {
                        object: card.id,
                        definition,
                        facts,
                    })
                })
                .collect(),
            mine: view.own_battlefield.iter().map(permanent).collect(),
            theirs: view.opponent_battlefield.iter().map(permanent).collect(),
            attackers: view.combat_attackers.iter().map(permanent).collect(),
            floating,
        }
    }

    /// The opponent with the lowest life, which is the default aggression
    /// target in any seat count.
    ///
    /// Returns `None` rather than falling back to a seat constant. A planner
    /// that cannot name an opponent must decline to act, not guess -- the
    /// `PlayerId(0)` fallback in the older policies would target the acting
    /// seat itself in a pod.
    #[must_use]
    pub fn primary_opponent(&self) -> Option<Opponent> {
        self.opponents
            .iter()
            .copied()
            .min_by_key(|opponent| opponent.life)
    }

    /// My creatures, excluding lands and noncreature permanents.
    #[must_use]
    pub fn my_creatures(&self) -> impl Iterator<Item = &Permanent> {
        self.mine.iter().filter(|permanent| permanent.is_creature)
    }

    /// Every opponent creature, still attributable to its controller.
    #[must_use]
    pub fn their_creatures(&self) -> impl Iterator<Item = &Permanent> {
        self.theirs.iter().filter(|permanent| permanent.is_creature)
    }

    /// Total power I could attack with if everything attacked.
    #[must_use]
    pub fn my_attacking_power(&self) -> i32 {
        self.mine
            .iter()
            .filter(|permanent| permanent.can_attack)
            .map(|permanent| i32::from(permanent.power))
            .sum()
    }
}
