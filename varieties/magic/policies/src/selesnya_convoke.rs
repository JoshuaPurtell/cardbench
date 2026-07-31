use cardbench_magic_engine::{
    CardType, CastRequest, Color, CombatBlock, ConvokeContribution, ConvokePayment, GameView,
    PlayerId, PolicyAction, Step,
};

use crate::CodePolicy;

/// A deterministic Selesnya policy for the public RAV deck fixture. It develops
/// Forests and Brownscales, uses convoke when legal, attacks with eligible
/// creatures, and makes simple one-for-one blocks.
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

    #[allow(clippy::too_many_lines)] // The policy doubles as readable development fixture code.
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
                let mut available = view
                    .own_battlefield
                    .iter()
                    .filter(|card| !card.tapped && card.card_types.contains(&CardType::Creature));
                let assignments = view
                    .combat_attackers
                    .iter()
                    .filter_map(|attacker| {
                        available.next().map(|blocker| CombatBlock {
                            attacker: attacker.id,
                            blocker: blocker.id,
                        })
                    })
                    .collect();
                return PolicyAction::DeclareBlockers { assignments };
            }
            Step::PrecombatMain | Step::PostcombatMain => {}
            _ => return PolicyAction::PassPriority,
        }
        if view.priority != self.player {
            return PolicyAction::PassPriority;
        }
        let green_creatures: Vec<_> = view
            .own_battlefield
            .iter()
            .filter(|card| {
                !card.tapped
                    && card.colors.contains(&Color::Green)
                    && card.card_types.contains(&CardType::Creature)
            })
            .take(3)
            .collect();
        if let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some("RAV-SCATTER-THE-SEEDS"))
            && green_creatures.len() == 3
            && view.mana_pool.total() >= 2
        {
            return PolicyAction::Cast(CastRequest {
                card: card.id,
                targets: vec![],
                convoke: vec![
                    ConvokePayment {
                        creature: green_creatures[0].id,
                        contribution: ConvokeContribution::Color(Color::Green),
                    },
                    ConvokePayment {
                        creature: green_creatures[1].id,
                        contribution: ConvokeContribution::Color(Color::Green),
                    },
                    ConvokePayment {
                        creature: green_creatures[2].id,
                        contribution: ConvokeContribution::Generic,
                    },
                ],
                payment_mana_abilities: vec![],
            });
        }
        if view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }
        if view.active_player != self.player {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0
            && let Some(land) = view.hand.iter().find(|card| {
                card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&Color::Green)
            })
        {
            return PolicyAction::PlayLand { card: land.id };
        }
        if let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some("RAV-GOLGARI-BROWNSCALE"))
            && view.mana_pool.amount(Color::Green) >= 2
            && view.mana_pool.total() >= 3
        {
            return PolicyAction::Cast(CastRequest {
                card: card.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            });
        }
        let wants_green = view
            .hand
            .iter()
            .any(|card| card.definition == Some("RAV-GOLGARI-BROWNSCALE"));
        if let Some(land) = view.own_battlefield.iter().find(|card| {
            !card.tapped
                && card.card_types.contains(&CardType::Land)
                && card.mana_colors.contains(&Color::Green)
        }) && (wants_green || view.mana_pool.total() < 2)
        {
            return PolicyAction::ActivateManaAbility {
                land: land.id,
                color: Color::Green,
            };
        }
        PolicyAction::PassPriority
    }
}
