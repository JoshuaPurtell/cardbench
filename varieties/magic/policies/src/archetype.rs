//! Archetypes as objective functions.
//!
//! An archetype is not a card list -- it is what the pilot is trying to do.
//! Aggro spends resources to end the game early and treats life as a currency;
//! midrange trades efficiently and wins on board; burn ignores the board and
//! counts to twenty. Expressing that as weights over the shared planners means
//! one policy implementation covers all three, and a deck's measured strength
//! is not an artefact of which pilot someone hand-wrote for it.

use crate::planner::{Aggression, Weights};
use std::fmt::{Display, Formatter};

/// The play pattern a deck is piloted with.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Archetype {
    /// Deploy cheap threats, attack relentlessly, use burn as reach.
    Aggro,
    /// Trade efficiently, build a superior board, win the long game.
    Midrange,
    /// Ignore the board where possible; point damage at the opponent's face.
    Burn,
    /// Preserve life and material, deploy late, win with card quality.
    Control,
}

impl Archetype {
    pub const ALL: [Self; 4] = [Self::Aggro, Self::Midrange, Self::Burn, Self::Control];

    /// The stable identifier used in deck files and result records.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Aggro => "aggro",
            Self::Midrange => "midrange",
            Self::Burn => "burn",
            Self::Control => "control",
        }
    }

    /// Parses an identifier from a deck file.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|archetype| archetype.id() == value)
    }

    /// Evaluation weights for this archetype.
    #[must_use]
    pub const fn weights(self) -> Weights {
        match self {
            // Own life is nearly worthless as anything but a clock buffer, and
            // every point of opponent life removed is progress.
            Self::Aggro => Weights {
                power: 1.35,
                toughness: 0.45,
                evasion: 0.9,
                own_life: 0.08,
                opponent_life: 0.75,
                card_in_hand: 0.3,
                mana: 0.1,
            },
            Self::Midrange => Weights {
                power: 1.0,
                toughness: 0.9,
                evasion: 0.55,
                own_life: 0.3,
                opponent_life: 0.35,
                card_in_hand: 0.6,
                mana: 0.2,
            },
            // The board is a means, not an end; damage to the face is the only
            // real currency, and cards in hand are stored damage.
            Self::Burn => Weights {
                power: 0.7,
                toughness: 0.35,
                evasion: 0.7,
                own_life: 0.12,
                opponent_life: 1.1,
                card_in_hand: 0.55,
                mana: 0.15,
            },
            Self::Control => Weights {
                power: 0.75,
                toughness: 1.1,
                evasion: 0.4,
                own_life: 0.45,
                opponent_life: 0.2,
                card_in_hand: 0.85,
                mana: 0.25,
            },
        }
    }

    /// How readily this archetype accepts an even or losing combat trade.
    #[must_use]
    pub const fn aggression(self) -> Aggression {
        match self {
            Self::Aggro => Aggression::Pressing,
            Self::Midrange | Self::Burn => Aggression::Measured,
            Self::Control => Aggression::Measured,
        }
    }

    /// The mana value at and below which this archetype prefers to keep
    /// deploying rather than holding up interaction.
    #[must_use]
    pub const fn curve_ceiling(self) -> u8 {
        match self {
            Self::Aggro | Self::Burn => 4,
            Self::Midrange => 6,
            Self::Control => 8,
        }
    }

    /// Whether burn should be pointed at the opponent's face by default rather
    /// than used as removal.
    #[must_use]
    pub const fn burn_goes_face(self) -> bool {
        matches!(self, Self::Burn)
    }

    /// Life total below which this archetype stops spending life on its own
    /// effects (shocklands, painful mana, self-damaging burn).
    #[must_use]
    pub const fn life_floor(self) -> i64 {
        match self {
            Self::Aggro | Self::Burn => 5,
            Self::Midrange => 8,
            Self::Control => 10,
        }
    }
}

impl Display for Archetype {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_archetype_round_trips_through_its_id() {
        for archetype in Archetype::ALL {
            assert_eq!(Archetype::parse(archetype.id()), Some(archetype));
        }
        assert_eq!(Archetype::parse("combo"), None);
    }

    /// The weights must actually differ, or "three archetypes" is one policy
    /// wearing three names and every matchup number is meaningless.
    #[test]
    fn archetype_weights_are_distinct() {
        for (index, left) in Archetype::ALL.into_iter().enumerate() {
            for right in Archetype::ALL.into_iter().skip(index + 1) {
                assert_ne!(
                    left.weights(),
                    right.weights(),
                    "{left} and {right} share an objective function"
                );
            }
        }
    }

    /// The defining contrast: aggro must value damage over its own life, and
    /// control the reverse.
    #[test]
    fn aggro_and_control_value_life_oppositely() {
        let aggro = Archetype::Aggro.weights();
        let control = Archetype::Control.weights();
        assert!(aggro.opponent_life > aggro.own_life * 4.0);
        assert!(control.own_life > aggro.own_life);
        assert!(control.card_in_hand > aggro.card_in_hand);
    }
}
