use cardbench_magic_engine::{
    CardType, CastRequest, Color, ConvokeContribution, ConvokePayment, GameView, PlayerId,
    PolicyAction,
};

use crate::CodePolicy;

/// A deterministic reference policy that converts three green creatures into a
/// `Scatter the Seeds` convoke payment when the prepared mana can finish the cost.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelesnyaConvokePolicy {
    player: PlayerId,
}

impl SelesnyaConvokePolicy {
    #[must_use]
    pub const fn new(player: PlayerId) -> Self {
        Self { player }
    }
}

impl CodePolicy for SelesnyaConvokePolicy {
    fn id(&self) -> &'static str {
        "rav.selesnya-convoke.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        let can_finish_cost = view.mana_pool.total() >= 2;
        let creatures: Vec<_> = view
            .own_battlefield
            .iter()
            .filter(|card| {
                !card.tapped
                    && card.colors.contains(&Color::Green)
                    && card.card_types.contains(&CardType::Creature)
            })
            .take(3)
            .collect();
        if view.player == self.player
            && view.priority == self.player
            && can_finish_cost
            && creatures.len() == 3
        {
            if let Some(card) = view
                .hand
                .iter()
                .find(|card| card.definition == Some("RAV-SCATTER-THE-SEEDS"))
            {
                return PolicyAction::Cast(CastRequest {
                    card: card.id,
                    targets: vec![],
                    convoke: vec![
                        ConvokePayment {
                            creature: creatures[0].id,
                            contribution: ConvokeContribution::Color(Color::Green),
                        },
                        ConvokePayment {
                            creature: creatures[1].id,
                            contribution: ConvokeContribution::Color(Color::Green),
                        },
                        ConvokePayment {
                            creature: creatures[2].id,
                            contribution: ConvokeContribution::Generic,
                        },
                    ],
                });
            }
        }
        PolicyAction::PassPriority
    }
}
