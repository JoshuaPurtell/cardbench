use cardbench_magic_engine::{CastRequest, Color, GameView, PlayerId, PolicyAction, Target};

use crate::CodePolicy;

/// A deterministic reference policy that prioritizes `Lightning Helix`, then `Char`.
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

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        let can_pay_helix =
            view.mana_pool.amount(Color::White) >= 1 && view.mana_pool.amount(Color::Red) >= 1;
        if view.player != self.player || view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        let opponent = view
            .opponent_life
            .first()
            .map_or(PlayerId(0), |(player, _)| *player);
        if can_pay_helix {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-LIGHTNING-HELIX"))
            {
                return PolicyAction::Cast(CastRequest {
                    card: card.id,
                    targets: vec![Target::Player(opponent)],
                    convoke: vec![],
                });
            }
        }
        if view.mana_pool.amount(Color::Red) >= 3
            && let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-CHAR"))
        {
            return PolicyAction::Cast(CastRequest {
                card: card.id,
                targets: vec![Target::Player(opponent)],
                convoke: vec![],
            });
        }
        PolicyAction::PassPriority
    }
}
