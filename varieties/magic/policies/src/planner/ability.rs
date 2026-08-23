//! Activated-ability planning.
//!
//! Nine of the thirty-eight distinct cards across the constructed decks carry
//! a stack-using activated ability, and no policy generation before v5 ever
//! activated one. A repeatable damage source was played as a vanilla body, a
//! token engine made no tokens, and a tapper tapped nothing.
//!
//! Scope is deliberately narrow. An ability whose cost is a sacrifice, a
//! discard, or extra creature taps needs explicit payment selections this
//! planner does not build; those are reported through
//! [`super::board::AbilityFacts::unsupported`] rather than silently skipped,
//! so the remaining gap stays visible.

use super::board::{AbilityFacts, Board, Permanent, requirement_hits_creature};
use super::mana::{self, ManaTap};
use super::target;
use super::threat::{Weights, creature_value};
use cardbench_magic_engine::{Keyword, ObjectId, Target, TargetRequirement};

/// One activation the policy could submit, already costed and targeted.
#[derive(Clone, Debug)]
pub struct Activation {
    pub source: ObjectId,
    pub ability: &'static str,
    pub targets: Vec<Target>,
    /// Mana activations to submit before the ability itself.
    pub taps: Vec<ManaTap>,
    /// Board-score value of resolving it, on the shared scale.
    pub value: f32,
}

/// Whether this ability can legally be activated right now, cost aside.
fn timing_allows(board: &Board, permanent: &Permanent, ability: &AbilityFacts) -> bool {
    if ability.unsupported {
        return false;
    }
    if ability.tap_cost && (permanent.tapped || permanent.summoning_sick) {
        return false;
    }
    if ability.sorcery_speed
        && !(board.is_my_turn
            && board.stack_depth == 0
            && matches!(
                board.step,
                cardbench_magic_engine::Step::PrecombatMain
                    | cardbench_magic_engine::Step::PostcombatMain
            ))
    {
        return false;
    }
    true
}

/// Chooses targets for one ability, or `None` when a requirement has none.
fn targets_for(board: &Board, ability: &AbilityFacts, weights: &Weights) -> Option<Vec<Target>> {
    let mut chosen = Vec::new();
    for requirement in &ability.targets {
        let scored = if ability.imposes_restriction {
            // A restriction is aimed at the biggest thing that attacks or
            // blocks, which is always an opponent's creature.
            best_opposing_creature(board, *requirement, weights)?
        } else if ability.damage > 0 || ability.life_loss > 0 {
            best_damage_target(board, ability, *requirement, weights)?
        } else {
            best_opposing_creature(board, *requirement, weights).or_else(|| {
                board
                    .my_creatures()
                    .next()
                    .map(|p| Target::Permanent(p.object))
            })?
        };
        chosen.push(scored);
    }
    Some(chosen)
}

fn best_opposing_creature(
    board: &Board,
    requirement: TargetRequirement,
    weights: &Weights,
) -> Option<Target> {
    if !requirement_hits_creature(requirement) {
        return None;
    }
    let attacking_only = requirement == TargetRequirement::AttackingOrBlockingCreature;
    board
        .their_creatures()
        .filter(|permanent| !permanent.has(&Keyword::Shroud))
        .filter(|permanent| {
            !attacking_only
                || board
                    .attackers
                    .iter()
                    .any(|attacker| attacker.object == permanent.object)
        })
        // Already neutralised creatures score zero, so this never re-taps the
        // same target turn after turn.
        .filter(|permanent| creature_value(permanent, weights) > 0.0)
        .max_by(|left, right| {
            creature_value(left, weights)
                .total_cmp(&creature_value(right, weights))
                .then(left.object.0.cmp(&right.object.0))
        })
        .map(|permanent| Target::Permanent(permanent.object))
}

fn best_damage_target(
    board: &Board,
    ability: &AbilityFacts,
    requirement: TargetRequirement,
    weights: &Weights,
) -> Option<Target> {
    let damage = i16::from(ability.damage.max(ability.life_loss));
    // A creature this ping actually kills is worth more than face damage.
    let killable = if requirement_hits_creature(requirement) {
        board
            .their_creatures()
            .filter(|permanent| !permanent.has(&Keyword::Shroud))
            .filter(|permanent| permanent.toughness <= damage)
            .max_by(|left, right| {
                creature_value(left, weights).total_cmp(&creature_value(right, weights))
            })
            .map(|permanent| Target::Permanent(permanent.object))
    } else {
        None
    };
    if killable.is_some() {
        return killable;
    }
    if super::board::requirement_hits_player(requirement) {
        return board
            .primary_opponent()
            .map(|opponent| Target::Player(opponent.seat));
    }
    best_opposing_creature(board, requirement, weights)
}

/// How much resolving this ability is worth.
fn value_of(board: &Board, ability: &AbilityFacts, targets: &[Target], weights: &Weights) -> f32 {
    let mut value = 0.0_f32;
    if let Some(Target::Permanent(object)) = targets.first()
        && let Some(victim) = board
            .theirs
            .iter()
            .find(|permanent| permanent.object == *object)
    {
        let damage = i16::from(ability.damage.max(ability.life_loss));
        if ability.imposes_restriction {
            // Neutralising a creature is worth most of removing it.
            value += creature_value(victim, weights) * 0.8;
        } else if damage >= victim.toughness {
            value += creature_value(victim, weights);
        } else if damage > 0 {
            value += creature_value(victim, weights) * 0.15;
        }
    }
    if let Some(Target::Player(seat)) = targets.first()
        && let Some(opponent) = board.opponents.iter().find(|entry| entry.seat == *seat)
    {
        let damage = i32::from(ability.damage.max(ability.life_loss));
        if i64::from(damage) >= opponent.life {
            return f32::INFINITY;
        }
        value += super::combat::damage_value(damage, opponent.life, weights.opponent_life);
    }
    // A token is a new body: value it like a modest creature.
    value += f32::from(ability.tokens) * (weights.power + weights.toughness);
    value
}

/// What tapping this permanent gives up.
///
/// Zero for a noncreature source or an ability with no tap cost. For a
/// creature it is the combat contribution lost: on my turn an attacker I can
/// no longer send, on theirs a blocker I no longer have. Vigilance pays
/// nothing, because a vigilant creature was never going to tap to attack.
fn tap_opportunity_cost(
    board: &Board,
    permanent: &Permanent,
    ability: &AbilityFacts,
    weights: &Weights,
) -> f32 {
    if !ability.tap_cost || !permanent.is_creature {
        return 0.0;
    }
    if permanent.has(&Keyword::Vigilance) {
        return 0.0;
    }
    let body = creature_value(permanent, weights);
    if board.is_my_turn {
        // Only worth something if it could actually have attacked.
        if permanent.can_attack {
            body * 0.6
        } else {
            0.0
        }
    } else if super::threat::their_clock(board) > 0 {
        // It is a blocker, and blockers only matter against a live clock.
        body * 0.6
    } else {
        0.0
    }
}

/// The best activation available right now, if any is worth making.
///
/// Deterministic: ties break on source object id, then ability id.
#[must_use]
pub fn best_activation(board: &Board, weights: &Weights) -> Option<Activation> {
    best_activation_with_suppression(board, weights, false)
}

/// Chooses an activation while respecting the engine's live suppression fact.
///
/// This is a new entry point for v8. Older generations remain wired to
/// [`best_activation`] so their frozen measurements do not acquire a hidden
/// dependency on a newer view field.
#[must_use]
pub fn best_activation_respecting_suppression(
    board: &Board,
    weights: &Weights,
) -> Option<Activation> {
    best_activation_with_suppression(board, weights, true)
}

fn best_activation_with_suppression(
    board: &Board,
    weights: &Weights,
    respect_suppression: bool,
) -> Option<Activation> {
    let mut best: Option<Activation> = None;
    for permanent in &board.mine {
        if respect_suppression && permanent.nonmana_activated_abilities_suppressed {
            continue;
        }
        for ability in &permanent.abilities {
            if !timing_allows(board, permanent, ability) {
                continue;
            }
            let Some(plan) = mana::plan(board, &ability.mana_cost) else {
                continue;
            };
            let Some(targets) = targets_for(board, ability, weights) else {
                continue;
            };
            let mut value = value_of(board, ability, &targets, weights);
            // Tapping a creature for its ability costs whatever that creature
            // would have done in combat. Without this the planner happily taps
            // a 2/5 wall to ping for one, which is how v5 first measured at
            // exactly no gain across 479 games despite activating constantly.
            value -= tap_opportunity_cost(board, permanent, ability, weights);
            if value <= 0.0 {
                continue;
            }
            let candidate = Activation {
                source: permanent.object,
                ability: ability.id,
                targets,
                taps: plan.taps,
                value,
            };
            let better = best.as_ref().is_none_or(|current| {
                candidate.value > current.value
                    || (candidate.value.to_bits() == current.value.to_bits()
                        && (candidate.source.0, candidate.ability)
                            < (current.source.0, current.ability))
            });
            if better {
                best = Some(candidate);
            }
        }
    }
    best
}

/// Whether any activation would be worth more than casting the best spell.
///
/// Exposed so a policy can order the two without duplicating the comparison.
#[must_use]
pub fn beats_casting(activation: &Activation, best_cast_value: Option<f32>) -> bool {
    best_cast_value.is_none_or(|cast| activation.value > cast)
}

/// Re-exported so a policy can score a spell on the same scale.
#[must_use]
pub fn cast_value_of(
    board: &Board,
    facts: &super::board::CardFacts,
    weights: &Weights,
) -> Option<f32> {
    target::cast_value(board, facts, weights)
}
