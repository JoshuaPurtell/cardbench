use cardbench_magic_engine::{
    CardType, CastRequest, Color, GameView, PlayerId, PolicyAction, Step, Target,
};

use crate::CodePolicy;

/// A deterministic Boros policy for the public RAV deck fixture. It plays a land,
/// activates intrinsic mana abilities, prioritizes `Lightning Helix` over `Char`,
/// and explicitly completes combat declarations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BorosTempoPolicy {
    player: PlayerId,
}

impl BorosTempoPolicy {
    #[must_use]
    pub const fn new(player: PlayerId) -> Self {
        Self { player }
    }
}

impl CodePolicy for BorosTempoPolicy {
    fn id(&self) -> &'static str {
        "rav.boros-tempo.v1"
    }

    #[allow(clippy::too_many_lines)] // The deterministic policy is deliberately transparent.
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        match view.step {
            Step::DeclareAttackers
                if view.active_player == self.player && !view.attackers_declared =>
            {
                return PolicyAction::DeclareAttackers {
                    attackers: view
                        .own_battlefield
                        .iter()
                        .filter(|card| card.can_attack)
                        .map(|card| card.id)
                        .collect(),
                };
            }
            Step::DeclareBlockers
                if view.active_player != self.player && !view.blockers_declared =>
            {
                return PolicyAction::DeclareBlockers {
                    assignments: vec![],
                };
            }
            Step::PrecombatMain | Step::PostcombatMain => {}
            _ => return PolicyAction::PassPriority,
        }
        if view.stack_depth != 0 || view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        let opponent = view
            .opponent_life
            .first()
            .map_or(PlayerId(0), |(player, _)| *player);
        let helix = view
            .hand
            .iter()
            .find(|card| card.definition == Some("RAV-LIGHTNING-HELIX"));
        let char = view
            .hand
            .iter()
            .find(|card| card.definition == Some("RAV-CHAR"));

        if view.active_player == self.player && view.lands_played == 0 {
            let desired = if helix.is_some() && view.mana_pool.amount(Color::White) == 0 {
                Some(Color::White)
            } else if (helix.is_some() || char.is_some()) && view.mana_pool.amount(Color::Red) == 0
            {
                Some(Color::Red)
            } else {
                None
            };
            if let Some(land) = select_hand_land(view, desired) {
                return PolicyAction::PlayLand { card: land };
            }
        }
        if let Some(card) = helix
            && view.mana_pool.amount(Color::White) >= 1
            && view.mana_pool.amount(Color::Red) >= 1
        {
            return PolicyAction::Cast(CastRequest {
                card: card.id,
                targets: vec![Target::Player(opponent)],
                convoke: vec![],
            });
        }
        if let Some(card) = char
            && view.mana_pool.amount(Color::Red) >= 1
            && view.mana_pool.total() >= 3
        {
            return PolicyAction::Cast(CastRequest {
                card: card.id,
                targets: vec![Target::Player(opponent)],
                convoke: vec![],
            });
        }
        let needed = if helix.is_some() && view.mana_pool.amount(Color::White) == 0 {
            Some(Color::White)
        } else if (helix.is_some() || char.is_some()) && view.mana_pool.amount(Color::Red) == 0 {
            Some(Color::Red)
        } else {
            None
        };
        if let Some(land) = select_battlefield_land(view, needed) {
            let color = needed.unwrap_or_else(|| *land.mana_colors.first().expect("land has mana"));
            return PolicyAction::ActivateManaAbility {
                land: land.id,
                color,
            };
        }
        PolicyAction::PassPriority
    }
}

fn select_hand_land(
    view: &GameView,
    desired: Option<Color>,
) -> Option<cardbench_magic_engine::ObjectId> {
    let mut lands = view
        .hand
        .iter()
        .filter(|card| card.card_types.contains(&CardType::Land));
    if let Some(color) = desired
        && let Some(land) = lands.clone().find(|card| card.mana_colors.contains(&color))
    {
        return Some(land.id);
    }
    lands.next().map(|card| card.id)
}

fn select_battlefield_land(
    view: &GameView,
    desired: Option<Color>,
) -> Option<&cardbench_magic_engine::CardView> {
    let mut lands = view.own_battlefield.iter().filter(|card| {
        !card.tapped && card.card_types.contains(&CardType::Land) && !card.mana_colors.is_empty()
    });
    if let Some(color) = desired {
        return lands.find(|card| card.mana_colors.contains(&color));
    }
    lands.next()
}

#[cfg(test)]
mod tests {
    use cardbench_magic_engine::{Game, Zone};
    use cardbench_magic_rav::card_definitions;

    use super::*;

    #[test]
    fn does_not_play_a_land_during_the_opponents_main_phase() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        game.add_card(PlayerId(1), "RAV-MOUNTAIN", Zone::Hand)
            .expect("Mountain in hand");
        game.pass_priority(PlayerId(0))
            .expect("give player one priority");
        let mut policy = BorosTempoPolicy::new(PlayerId(1));
        assert_eq!(
            policy.propose_move(&game.view_for_player(PlayerId(1)).expect("view")),
            PolicyAction::PassPriority
        );
    }
}
