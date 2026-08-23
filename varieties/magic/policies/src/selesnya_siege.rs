use cardbench_magic_engine::{
    CardType, CastRequest, Color, CombatBlock, ConvokeContribution, ConvokePayment, GameView,
    PlayerId, PolicyAction, Step,
};

use crate::CodePolicy;

/// A deterministic green-convoke policy for the public RAV Siege fixture.
///
/// It develops Forests, grows its board with Brownscales and Scatter the Seeds,
/// then spends the available convoke resources on Siege Wurms. Combat choices are
/// deliberately explicit: it attacks with every eligible creature and blocks one
/// attacker per untapped creature, preserving Wurms before expendable tokens.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelesnyaSiegePolicy {
    player: PlayerId,
}

impl SelesnyaSiegePolicy {
    #[must_use]
    pub const fn new(player: PlayerId) -> Self {
        Self { player }
    }
}

impl CodePolicy for SelesnyaSiegePolicy {
    fn id(&self) -> &'static str {
        "rav.selesnya-siege.v1"
    }

    #[allow(clippy::too_many_lines)] // Kept linear so the policy's legal choices are auditable.
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
                    assignments: defensive_assignments(view),
                };
            }
            Step::PrecombatMain | Step::PostcombatMain => {}
            _ => return PolicyAction::PassPriority,
        }
        if view.priority != self.player
            || view.active_player != self.player
            || view.stack_depth != 0
        {
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
            .find(|card| card.definition == Some("RAV-SIEGE-WURM"))
            && let Some(convoke) = legal_green_convoke(view, 2, 5)
        {
            return PolicyAction::Cast(CastRequest {
                card: card.id,
                targets: vec![],
                convoke,
                payment_mana_abilities: vec![],
            });
        }
        if let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some("RAV-SCATTER-THE-SEEDS"))
            && let Some(convoke) = legal_green_convoke(view, 2, 3)
        {
            return PolicyAction::Cast(CastRequest {
                card: card.id,
                targets: vec![],
                convoke,
                payment_mana_abilities: vec![],
            });
        }
        if let Some(card) = view
            .hand
            .iter()
            .find(|card| card.definition == Some("RAV-GOLGARI-BROWNSCALE"))
            && can_pay_green_cost(view, 2, 1)
        {
            return PolicyAction::Cast(CastRequest {
                card: card.id,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            });
        }

        if has_spell_to_develop(view)
            && let Some(land) = view.own_battlefield.iter().find(|card| {
                !card.tapped
                    && card.card_types.contains(&CardType::Land)
                    && card.mana_colors.contains(&Color::Green)
            })
        {
            return PolicyAction::ActivateManaAbility {
                land: land.id,
                color: Color::Green,
            };
        }
        PolicyAction::PassPriority
    }
}

fn legal_green_convoke(
    view: &GameView,
    colored_green: u8,
    generic: u8,
) -> Option<Vec<ConvokePayment>> {
    let creatures: Vec<_> = view
        .own_battlefield
        .iter()
        .filter(|card| {
            !card.tapped
                && card.colors.contains(&Color::Green)
                && card.card_types.contains(&CardType::Creature)
        })
        .collect();
    let colored_from_creatures = colored_green.saturating_sub(view.mana_pool.amount(Color::Green));
    if usize::from(colored_from_creatures) > creatures.len() {
        return None;
    }
    let remaining_colored = colored_green - colored_from_creatures;
    let mana_after_colored = view.mana_pool.total().checked_sub(remaining_colored)?;
    let generic_from_creatures = generic.saturating_sub(mana_after_colored);
    let required_creatures = usize::from(colored_from_creatures + generic_from_creatures);
    if required_creatures > creatures.len() {
        return None;
    }
    let mut convoke = Vec::with_capacity(required_creatures);
    for creature in creatures.iter().take(usize::from(colored_from_creatures)) {
        convoke.push(ConvokePayment {
            creature: creature.id,
            contribution: ConvokeContribution::Color(Color::Green),
        });
    }
    for creature in creatures
        .iter()
        .skip(usize::from(colored_from_creatures))
        .take(usize::from(generic_from_creatures))
    {
        convoke.push(ConvokePayment {
            creature: creature.id,
            contribution: ConvokeContribution::Generic,
        });
    }
    Some(convoke)
}

fn can_pay_green_cost(view: &GameView, colored_green: u8, generic: u8) -> bool {
    view.mana_pool.amount(Color::Green) >= colored_green
        && view.mana_pool.total().saturating_sub(colored_green) >= generic
}

fn has_spell_to_develop(view: &GameView) -> bool {
    view.hand.iter().any(|card| {
        matches!(
            card.definition,
            Some("RAV-SIEGE-WURM" | "RAV-SCATTER-THE-SEEDS" | "RAV-GOLGARI-BROWNSCALE")
        )
    })
}

fn defensive_assignments(view: &GameView) -> Vec<CombatBlock> {
    let mut blockers: Vec<_> = view
        .own_battlefield
        .iter()
        .filter(|card| !card.tapped && card.card_types.contains(&CardType::Creature))
        .collect();
    blockers.sort_by_key(|card| blocker_priority(card.definition));
    view.combat_attackers
        .iter()
        .zip(blockers)
        .map(|(attacker, blocker)| CombatBlock {
            attacker: attacker.id,
            blocker: blocker.id,
        })
        .collect()
}

fn blocker_priority(definition: Option<&str>) -> u8 {
    match definition {
        None => 0,
        Some("RAV-GOLGARI-BROWNSCALE") => 1,
        Some("RAV-SIEGE-WURM") => 2,
        _ => 3,
    }
}

#[cfg(test)]
mod tests {
    use cardbench_magic_engine::{Game, PlayerId, Zone};
    use cardbench_magic_rav::card_definitions;

    use super::*;

    #[test]
    fn casts_siege_wurm_only_through_a_legal_engine_submission() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        let wurm = game
            .add_card(PlayerId(0), "RAV-SIEGE-WURM", Zone::Hand)
            .expect("Wurm in hand");
        game.grant_mana(PlayerId(0), Color::Green, 7)
            .expect("development mana");
        let mut policy = SelesnyaSiegePolicy::new(PlayerId(0));
        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        assert_eq!(
            action,
            PolicyAction::Cast(CastRequest {
                card: wurm,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            })
        );
        game.submit_policy_move(PlayerId(0), policy.id(), action)
            .expect("policy's Wurm proposal must be legal");
    }

    #[test]
    fn uses_only_untapped_green_creatures_for_convoke() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        let scatter = game
            .add_card(PlayerId(0), "RAV-SCATTER-THE-SEEDS", Zone::Hand)
            .expect("Scatter in hand");
        let creatures: Vec<_> = (0..5)
            .map(|_| {
                game.put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE")
                    .expect("Brownscale")
            })
            .collect();
        let mut policy = SelesnyaSiegePolicy::new(PlayerId(0));
        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        let PolicyAction::Cast(request) = action else {
            panic!("policy should convoke Scatter: {action:?}");
        };
        assert_eq!(request.card, scatter);
        assert_eq!(request.convoke.len(), 5);
        assert!(
            request
                .convoke
                .iter()
                .all(|payment| creatures.contains(&payment.creature))
        );
        game.submit_policy_move(PlayerId(0), policy.id(), PolicyAction::Cast(request))
            .expect("policy's Scatter proposal must be legal");
    }

    #[test]
    fn submits_explicit_attack_and_lowest_value_block_assignment() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        let attacker = game
            .put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE")
            .expect("attacking Brownscale");
        let blocker = game
            .put_on_battlefield(PlayerId(1), "RAV-GOLGARI-BROWNSCALE")
            .expect("expendable defending Brownscale");
        game.put_on_battlefield(PlayerId(1), "RAV-SIEGE-WURM")
            .expect("valuable defending Wurm");
        game.add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
            .expect("player zero draw card");
        game.add_card(PlayerId(1), "RAV-FOREST", Zone::Library)
            .expect("player one draw card");
        while !(game.turn == 3 && game.step == Step::DeclareAttackers) {
            if game.step == Step::DeclareAttackers
                && !game
                    .view_for_player(game.next_policy_player())
                    .expect("combat view")
                    .attackers_declared
            {
                game.declare_attackers(game.active_player, &[])
                    .expect("empty declarations are explicit turn actions");
            }
            if game
                .view_for_player(game.next_policy_player())
                .expect("draw decision view")
                .draw_replacement_pending
            {
                let player = game.next_policy_player();
                game.resolve_pending_draw(player, None)
                    .expect("take ordinary draw");
            }
            let priority = game.priority;
            game.pass_priority(priority)
                .expect("advance to player zero combat");
        }

        let mut attacker_policy = SelesnyaSiegePolicy::new(PlayerId(0));
        let attack =
            attacker_policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        assert_eq!(
            attack,
            PolicyAction::DeclareAttackers {
                attackers: vec![attacker]
            }
        );
        game.submit_policy_move(PlayerId(0), attacker_policy.id(), attack)
            .expect("attack declaration must be legal");
        for _ in 0..2 {
            if game
                .view_for_player(game.next_policy_player())
                .expect("draw decision view")
                .draw_replacement_pending
            {
                let player = game.next_policy_player();
                game.resolve_pending_draw(player, None)
                    .expect("take ordinary draw");
            }
            let priority = game.priority;
            game.pass_priority(priority)
                .expect("advance to blockers declaration");
        }

        let mut defender_policy = SelesnyaSiegePolicy::new(PlayerId(1));
        let block = defender_policy.propose_move(&game.view_for_player(PlayerId(1)).expect("view"));
        assert_eq!(
            block,
            PolicyAction::DeclareBlockers {
                assignments: vec![CombatBlock { attacker, blocker }]
            }
        );
        game.submit_policy_move(PlayerId(1), defender_policy.id(), block)
            .expect("block declaration must be legal");
        game.validate_invariants()
            .expect("policy combat submissions must preserve invariants");
    }
}
