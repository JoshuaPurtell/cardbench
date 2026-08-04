//! Target selection for removal, burn, and pump.
//!
//! This is the component whose absence made a whole card class unplayable: the
//! previous generic policy could only cast target-free permanents, which
//! excludes every instant and sorcery in the set -- all the removal and all the
//! burn. A burn deck was literally unpilotable.

use super::board::{
    Board, CardFacts, Permanent, Role, requirement_hits_creature, requirement_hits_player,
};
use super::threat::{Weights, creature_value};
use cardbench_magic_engine::{CardType, Keyword, Target, TargetRequirement};

/// A chosen target and how good it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScoredTarget {
    pub target: Target,
    pub score: f32,
}

/// Whether a permanent can legally be chosen as a target at all.
fn targetable(permanent: &Permanent) -> bool {
    !permanent.has(&Keyword::Shroud)
}

/// Whether `permanent` satisfies `requirement`.
///
/// Conservative by construction: a requirement this planner cannot evaluate
/// returns `false`, so it declines to cast rather than submitting an illegal
/// target and eating a rejection.
fn matches_requirement(
    permanent: &Permanent,
    requirement: TargetRequirement,
    board: &Board,
) -> bool {
    if !targetable(permanent) {
        return false;
    }
    let attacking = board
        .attackers
        .iter()
        .any(|attacker| attacker.object == permanent.object);
    match requirement {
        TargetRequirement::Any | TargetRequirement::Permanent => true,
        // Every requirement that admits an ordinary battlefield creature and
        // nothing this planner can distinguish further.
        TargetRequirement::Creature
        | TargetRequirement::DistinctCreature
        | TargetRequirement::ArtifactOrCreature
        | TargetRequirement::PlayerOrCreature => permanent.is_creature,
        TargetRequirement::FlyingCreature => {
            permanent.is_creature && permanent.has(&Keyword::Flying)
        }
        TargetRequirement::AttackingOrBlockingCreature => permanent.is_creature && attacking,
        TargetRequirement::Land => permanent.is_land,
        TargetRequirement::ControlledLand => permanent.is_land && permanent.controller == board.me,
        TargetRequirement::ControlledCreature => {
            permanent.is_creature && permanent.controller == board.me
        }
        TargetRequirement::OpponentCreature => {
            permanent.is_creature && permanent.controller != board.me
        }
        // NonblackCreature needs the live colour characteristic, which the
        // board abstraction does not carry. Declining is the safe answer.
        _ => false,
    }
}

/// Picks the best target for one requirement of a hostile spell.
///
/// `damage` is the damage the spell will deal, used so a burn spell prefers a
/// creature it actually kills over a larger one it merely bruises.
#[must_use]
pub fn best_hostile_target(
    board: &Board,
    facts: &CardFacts,
    requirement: TargetRequirement,
    weights: &Weights,
) -> Option<ScoredTarget> {
    let mut best: Option<ScoredTarget> = None;

    if requirement_hits_creature(requirement) {
        for permanent in board.their_creatures() {
            if !matches_requirement(permanent, requirement, board) {
                continue;
            }
            let value = creature_value(permanent, weights);
            let kills = match facts.role {
                Role::Removal => true,
                Role::Burn => i16::from(facts.damage_to_target) >= permanent.toughness,
                _ => false,
            };
            // A removal spell that does not kill is close to worthless; a burn
            // spell that only bruises is worse than aiming at the face.
            let score = if kills { value } else { value * 0.1 };
            if best.is_none_or(|current| score > current.score) {
                best = Some(ScoredTarget {
                    target: Target::Permanent(permanent.object),
                    score,
                });
            }
        }
    }

    if requirement_hits_player(requirement) && facts.damage_to_target > 0 {
        if let Some(opponent) = board.primary_opponent() {
            let damage = f32::from(facts.damage_to_target);
            // Damage to a player is worth its share of that player's remaining
            // life; the last point is worth everything.
            let remaining = f32::from(i16::try_from(opponent.life.max(1)).unwrap_or(i16::MAX));
            let score = if i64::from(facts.damage_to_target) >= opponent.life {
                f32::INFINITY
            } else {
                damage / remaining * 8.0 * weights.opponent_life
            };
            if best.is_none_or(|current| score > current.score) {
                best = Some(ScoredTarget {
                    target: Target::Player(opponent.seat),
                    score,
                });
            }
        }
    }

    best
}

/// Picks a target for a friendly spell, such as a pump or an aura.
#[must_use]
pub fn best_friendly_target(
    board: &Board,
    requirement: TargetRequirement,
    weights: &Weights,
) -> Option<ScoredTarget> {
    if requirement == TargetRequirement::ControlledLand {
        // Karoo return: give back the land that produces the least, which is a
        // tapped one if any, so the bounce costs no mana this turn.
        return board
            .mine
            .iter()
            .filter(|permanent| permanent.is_land)
            .max_by_key(|permanent| (u8::from(permanent.tapped), permanent.object.0))
            .map(|permanent| ScoredTarget {
                target: Target::Permanent(permanent.object),
                score: 1.0,
            });
    }
    board
        .my_creatures()
        .filter(|permanent| matches_requirement(permanent, requirement, board))
        // Buff the creature that most improves the attack: highest existing
        // power, so the pump converts to damage rather than padding a wall.
        .max_by(|left, right| {
            creature_value(left, weights)
                .total_cmp(&creature_value(right, weights))
                .then(left.object.0.cmp(&right.object.0))
        })
        .map(|permanent| ScoredTarget {
            target: Target::Permanent(permanent.object),
            score: creature_value(permanent, weights),
        })
}

/// Chooses every target one spell needs, in printed order.
///
/// Returns `None` when any requirement has no legal target -- the spell then
/// cannot legally be cast and must not be attempted.
#[must_use]
pub fn targets_for(board: &Board, facts: &CardFacts, weights: &Weights) -> Option<Vec<Target>> {
    let friendly = matches!(facts.role, Role::Pump | Role::Attachment);
    let mut chosen = Vec::new();
    for requirement in &facts.targets {
        let scored = if friendly {
            best_friendly_target(board, *requirement, weights)
        } else {
            best_hostile_target(board, facts, *requirement, weights)
        }?;
        // Distinct-creature requirements must not repeat a target.
        if *requirement == TargetRequirement::DistinctCreature && chosen.contains(&scored.target) {
            return None;
        }
        chosen.push(scored.target);
    }
    Some(chosen)
}

/// How much casting this card is worth right now, in board-score units.
///
/// This is the single number the policy sorts its hand by. A card that cannot
/// legally be cast scores `None` rather than a low number, so it is never
/// attempted.
#[must_use]
pub fn cast_value(board: &Board, facts: &CardFacts, weights: &Weights) -> Option<f32> {
    if facts.unsupported {
        return None;
    }
    // Self-damage that would kill me is never worth it.
    if i64::from(facts.self_damage) >= board.my_life {
        return None;
    }
    let targets = if facts.needs_targets() {
        Some(targets_for(board, facts, weights)?)
    } else {
        None
    };

    let mut value = match facts.role {
        Role::Creature => {
            let body = Permanent {
                object: cardbench_magic_engine::ObjectId(0),
                controller: board.me,
                definition: Some(facts.id),
                tapped: false,
                can_attack: false,
                can_block: true,
                summoning_sick: false,
                is_creature: true,
                is_land: false,
                power: facts.power,
                toughness: facts.toughness,
                keywords: facts.keywords.clone(),
                source: None,
                role: facts.role,
                abilities: Vec::new(),
                controller_is_opponent: false,
            };
            creature_value(&body, weights)
        }
        // A mana creature is worth the mana it unlocks, not its body.
        Role::ManaCreature => weights.mana * 6.0,
        Role::Removal | Role::Burn => targets.as_ref().map_or(0.0, |targets| {
            targets
                .iter()
                .map(|target| target_worth(board, facts, *target, weights))
                .sum()
        }),
        Role::TokenMaker => weights.power * 2.0,
        Role::Pump | Role::Attachment => weights.power * 1.5,
        Role::Land | Role::Other => 0.0,
    };
    value += f32::from(facts.life_gain) * weights.own_life;
    value -= f32::from(facts.self_damage) * weights.own_life;
    Some(value)
}

fn target_worth(board: &Board, facts: &CardFacts, target: Target, weights: &Weights) -> f32 {
    match target {
        Target::Player(seat) => {
            let Some(opponent) = board.opponents.iter().find(|entry| entry.seat == seat) else {
                return 0.0;
            };
            if i64::from(facts.damage_to_target) >= opponent.life {
                return f32::INFINITY;
            }
            f32::from(facts.damage_to_target) * weights.opponent_life
        }
        Target::Permanent(object) => board
            .theirs
            .iter()
            .find(|permanent| permanent.object == object)
            .map_or(0.0, |permanent| {
                let kills = matches!(facts.role, Role::Removal)
                    || i16::from(facts.damage_to_target) >= permanent.toughness;
                if kills {
                    creature_value(permanent, weights)
                } else {
                    0.0
                }
            }),
        _ => 0.0,
    }
}

/// Whether this card type may be cast while an opponent has priority or during
/// their turn.
#[must_use]
pub fn is_instant_speed(facts: &CardFacts) -> bool {
    facts.is_instant_speed || facts.keywords.contains(&Keyword::Flash)
}

/// Whether the card is a permanent that occupies the battlefield.
#[must_use]
pub fn is_permanent(types: &std::collections::BTreeSet<CardType>) -> bool {
    types.iter().any(|kind| {
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
