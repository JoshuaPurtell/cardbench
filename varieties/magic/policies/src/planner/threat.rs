//! Board and threat evaluation, shared by targeting and combat.
//!
//! One definition of "how much is this creature worth" used by every planner.
//! The alternative -- each decision site inventing its own comparison -- is
//! how a policy ends up trading a removal spell for a mana creature.

use super::board::{Board, Permanent};
use cardbench_magic_engine::Keyword;

/// Evaluation weights. Archetypes differ mostly by these numbers, not by code.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weights {
    /// Value of one point of power on a creature I control.
    pub power: f32,
    /// Value of one point of toughness.
    pub toughness: f32,
    /// Value of a creature being evasive, per point of power.
    pub evasion: f32,
    /// Value of one point of life I have.
    pub own_life: f32,
    /// Value of one point of damage dealt to an opponent.
    pub opponent_life: f32,
    /// Value of a card in hand.
    pub card_in_hand: f32,
    /// Value of one point of available mana.
    pub mana: f32,
}

impl Weights {
    /// A neutral baseline. Archetypes scale from here.
    #[must_use]
    pub const fn balanced() -> Self {
        Self {
            power: 1.0,
            toughness: 0.8,
            evasion: 0.6,
            own_life: 0.25,
            opponent_life: 0.35,
            card_in_hand: 0.5,
            mana: 0.15,
        }
    }
}

/// Whether a creature's damage is hard for a typical board to block.
#[must_use]
pub fn is_evasive(permanent: &Permanent) -> bool {
    permanent.keywords.iter().any(|keyword| {
        matches!(
            keyword,
            Keyword::Flying
                | Keyword::Fear
                | Keyword::Trample
                | Keyword::BlackEvasion
                | Keyword::Unblockable
                | Keyword::Mountainwalk
                | Keyword::Landwalk(_)
        )
    })
}

/// How much one creature is worth on the battlefield.
///
/// Deliberately not symmetric with mana value: a 4-mana 2/2 and a 2-mana 2/2
/// are worth the same *on the board*, and only the second is a good card.
/// Card quality belongs to deck construction, not to combat math.
#[must_use]
pub fn creature_value(permanent: &Permanent, weights: &Weights) -> f32 {
    if !permanent.is_creature {
        return 0.0;
    }
    let power = f32::from(permanent.power.max(0));
    let toughness = f32::from(permanent.toughness.max(0));
    let mut value = power * weights.power + toughness * weights.toughness;
    if is_evasive(permanent) {
        value += power * weights.evasion;
    }
    if permanent.has(&Keyword::DoubleStrike) {
        value += power * weights.power;
    } else if permanent.has(&Keyword::FirstStrike) {
        value += weights.power * 0.5;
    }
    if permanent.has(&Keyword::Vigilance) {
        value += weights.toughness * 0.5;
    }
    if permanent.has(&Keyword::Defender) {
        // A wall cannot pressure anything; its toughness is its whole job.
        value = toughness * weights.toughness;
    }
    if permanent.has(&Keyword::CannotAttackOrBlock) {
        // Already neutralised. Valuing it normally makes a removal spell keep
        // choosing the same creature: an aura that only restricts leaves power
        // and toughness untouched, so the target stays "best" and the whole
        // playset piles onto one permanent.
        return 0.0;
    }
    if permanent.has(&Keyword::CannotBlock) && permanent.controller_is_opponent {
        // An opposing creature that cannot block is only a clock, never a
        // roadblock.
        value *= 0.7;
    }
    value
}

/// A scalar score of the whole position from my seat, higher is better.
///
/// Used to compare candidate lines, so only differences matter -- the absolute
/// number is meaningless.
#[must_use]
pub fn evaluate(board: &Board, weights: &Weights) -> f32 {
    let mine: f32 = board
        .my_creatures()
        .map(|permanent| creature_value(permanent, weights))
        .sum();
    let theirs: f32 = board
        .their_creatures()
        .map(|permanent| creature_value(permanent, weights))
        .sum();
    let life = f32::from(i16::try_from(board.my_life.clamp(-64, 64)).unwrap_or(0));
    // Opponent life is scored as damage already dealt, so a lower total is
    // better for me. Summed across every opponent, so a pod does not collapse
    // to one number.
    let opponents: f32 = board
        .opponents
        .iter()
        .map(|opponent| {
            let remaining = f32::from(i16::try_from(opponent.life.clamp(-64, 64)).unwrap_or(0));
            -remaining * weights.opponent_life
        })
        .sum();
    let hand =
        f32::from(u16::try_from(board.hand.len()).unwrap_or(u16::MAX)) * weights.card_in_hand;
    let mana =
        f32::from(u16::try_from(super::mana::potential(board)).unwrap_or(u16::MAX)) * weights.mana;
    mine - theirs + life * weights.own_life + opponents + hand + mana
}

/// Total damage per turn I am currently applying, ignoring blocks.
#[must_use]
pub fn my_clock(board: &Board) -> i32 {
    board
        .mine
        .iter()
        .filter(|permanent| permanent.is_creature && !permanent.has(&Keyword::Defender))
        .map(|permanent| i32::from(permanent.power.max(0)))
        .sum()
}

/// Total damage per turn an opponent could apply back.
#[must_use]
pub fn their_clock(board: &Board) -> i32 {
    board
        .their_creatures()
        .filter(|permanent| !permanent.has(&Keyword::Defender))
        .map(|permanent| i32::from(permanent.power.max(0)))
        .sum()
}

/// Turns until my board kills the given opponent, if nothing changes.
///
/// `None` when I deal no damage. A large number is not the same as never, and
/// conflating them makes a policy race a clock it cannot win.
#[must_use]
pub fn turns_to_kill(clock: i32, life: i64) -> Option<u32> {
    if clock <= 0 {
        return None;
    }
    let life = life.max(0);
    let clock = i64::from(clock);
    Some(
        u32::try_from(life.div_euclid(clock) + i64::from(life.rem_euclid(clock) != 0))
            .unwrap_or(u32::MAX),
    )
}

/// Whether I am winning the damage race against my fastest opponent.
///
/// Ties go to the player whose turn is next, which is why `on_the_play` is an
/// input rather than an assumption.
#[must_use]
pub fn winning_the_race(board: &Board, on_the_play: bool) -> bool {
    let Some(opponent) = board.primary_opponent() else {
        return false;
    };
    let mine = turns_to_kill(my_clock(board), opponent.life);
    let theirs = turns_to_kill(their_clock(board), board.my_life);
    match (mine, theirs) {
        (None, _) => false,
        (Some(_), None) => true,
        (Some(mine), Some(theirs)) => {
            if mine == theirs {
                on_the_play
            } else {
                mine < theirs
            }
        }
    }
}
