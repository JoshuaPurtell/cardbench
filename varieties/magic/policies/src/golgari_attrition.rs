use cardbench_magic_engine::{
    CardType, CastRequest, Color, CombatBlock, ConvokeContribution, ConvokePayment, GameView,
    ObjectId, PlayerId, PolicyAction, Step, Target,
};

use crate::CodePolicy;

/// A deterministic Golgari midrange policy for the public RAV attrition fixture.
///
/// The policy develops green mana for its creature and convoke plan while keeping
/// black available for Last Gasp. It removes an opposing Brownscale when possible,
/// then uses the remaining creatures to trade in combat and eventually convoke a
/// Siege Wurm. Every proposal is deliberately limited to information exposed in
/// [`GameView`], so it remains a reproducible policy-to-engine probe.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GolgariAttritionPolicy {
    player: PlayerId,
}

impl GolgariAttritionPolicy {
    #[must_use]
    pub const fn new(player: PlayerId) -> Self {
        Self { player }
    }
}

impl CodePolicy for GolgariAttritionPolicy {
    fn id(&self) -> &'static str {
        "rav.golgari-attrition.v1"
    }

    #[allow(clippy::too_many_lines)] // Linear choices keep the policy auditable as a test probe.
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
        if view.priority != self.player {
            return PolicyAction::PassPriority;
        }

        // Last Gasp is an instant, so attrition interaction stays available on
        // either player's turn and while another spell is on the stack.
        if let (Some(gasp), Some(target)) = (last_gasp_in_hand(view), last_gasp_target(view))
            && can_pay_black(view)
        {
            return PolicyAction::Cast(CastRequest {
                card: gasp,
                targets: vec![Target::Permanent(target)],
                convoke: vec![],
            });
        }

        if view.active_player != self.player || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }

        if view.lands_played == 0
            && let Some(land) = land_to_play(view)
        {
            return PolicyAction::PlayLand { card: land };
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
            });
        }

        if let Some((land, color)) = mana_to_activate(view) {
            return PolicyAction::ActivateManaAbility { land, color };
        }
        PolicyAction::PassPriority
    }
}

fn last_gasp_in_hand(view: &GameView) -> Option<ObjectId> {
    view.hand
        .iter()
        .find(|card| card.definition == Some("RAV-LAST-GASP"))
        .map(|card| card.id)
}

fn last_gasp_target(view: &GameView) -> Option<ObjectId> {
    // Brownscale dies to the available -3/-3 effect; other creatures are still
    // legal interaction targets when removal can blunt an attack or block.
    view.opponent_battlefield
        .iter()
        .find(|card| card.definition == Some("RAV-GOLGARI-BROWNSCALE"))
        .or_else(|| {
            view.opponent_battlefield
                .iter()
                .find(|card| card.card_types.contains(&CardType::Creature))
        })
        .map(|card| card.id)
}

fn can_pay_black(view: &GameView) -> bool {
    view.mana_pool.amount(Color::Black) >= 1
}

fn can_pay_green_cost(view: &GameView, colored_green: u8, generic: u8) -> bool {
    view.mana_pool.amount(Color::Green) >= colored_green
        && view.mana_pool.total().saturating_sub(colored_green) >= generic
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

fn land_to_play(view: &GameView) -> Option<ObjectId> {
    let has_black_source = view
        .own_battlefield
        .iter()
        .any(|card| card.mana_colors.contains(&Color::Black));
    let green_sources = view
        .own_battlefield
        .iter()
        .filter(|card| card.mana_colors.contains(&Color::Green))
        .count();
    let needs_removal = last_gasp_in_hand(view).is_some() && last_gasp_target(view).is_some();
    let needs_green = view.hand.iter().any(|card| {
        matches!(
            card.definition,
            Some("RAV-GOLGARI-BROWNSCALE" | "RAV-SCATTER-THE-SEEDS" | "RAV-SIEGE-WURM")
        )
    });
    let desired = if needs_removal && !has_black_source {
        Color::Black
    } else if needs_green && green_sources < 2 {
        Color::Green
    } else if last_gasp_in_hand(view).is_some() && !has_black_source {
        Color::Black
    } else {
        Color::Green
    };
    view.hand
        .iter()
        .find(|card| {
            card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&desired)
        })
        .or_else(|| {
            view.hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
        })
        .map(|card| card.id)
}

fn mana_to_activate(view: &GameView) -> Option<(ObjectId, Color)> {
    let black_land = || {
        view.own_battlefield
            .iter()
            .find(|card| !card.tapped && card.mana_colors.contains(&Color::Black))
            .map(|card| (card.id, Color::Black))
    };
    let green_land = || {
        view.own_battlefield
            .iter()
            .find(|card| !card.tapped && card.mana_colors.contains(&Color::Green))
            .map(|card| (card.id, Color::Green))
    };
    if last_gasp_in_hand(view).is_some() && last_gasp_target(view).is_some() && !can_pay_black(view)
    {
        return black_land();
    }
    if view.hand.iter().any(|card| {
        matches!(
            card.definition,
            Some("RAV-GOLGARI-BROWNSCALE" | "RAV-SCATTER-THE-SEEDS" | "RAV-SIEGE-WURM")
        )
    }) {
        return green_land();
    }
    if last_gasp_in_hand(view).is_some() && !can_pay_black(view) {
        return black_land();
    }
    None
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
        Some("RAV-GOLGARI-BROWNSCALE") => 0,
        Some("RAV-SIEGE-WURM") => 1,
        _ => 2,
    }
}

#[cfg(test)]
mod tests {
    use cardbench_magic_engine::{Game, PlayerId, Step, Zone};
    use cardbench_magic_rav::card_definitions;

    use super::*;

    #[test]
    fn removes_an_opposing_brownscale_through_policy_submission() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        let gasp = game
            .add_card(PlayerId(0), "RAV-LAST-GASP", Zone::Hand)
            .expect("Gasp in hand");
        let target = game
            .put_on_battlefield(PlayerId(1), "RAV-GOLGARI-BROWNSCALE")
            .expect("opposing Brownscale");
        game.grant_mana(PlayerId(0), Color::Black, 1)
            .expect("black mana");

        let mut policy = GolgariAttritionPolicy::new(PlayerId(0));
        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        assert_eq!(
            action,
            PolicyAction::Cast(CastRequest {
                card: gasp,
                targets: vec![Target::Permanent(target)],
                convoke: vec![],
            })
        );
        game.submit_policy_move(PlayerId(0), policy.id(), action)
            .expect("policy removal must submit legally");
        game.pass_priority(PlayerId(1)).expect("opponent passes");
        game.pass_priority(PlayerId(0)).expect("controller passes");

        assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
        game.validate_invariants()
            .expect("removal line must preserve engine invariants");
    }

    #[test]
    fn convokes_scatter_only_with_legal_green_creatures() {
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

        let mut policy = GolgariAttritionPolicy::new(PlayerId(0));
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
            .expect("legal convoke proposal");
        game.validate_invariants()
            .expect("convoke submission must preserve engine invariants");
    }

    #[test]
    fn submits_explicit_attrition_combat_actions_and_preserves_invariants() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        let attacker = game
            .put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE")
            .expect("attacking Brownscale");
        let blocker = game
            .put_on_battlefield(PlayerId(1), "RAV-GOLGARI-BROWNSCALE")
            .expect("defending Brownscale");
        game.put_on_battlefield(PlayerId(1), "RAV-SIEGE-WURM")
            .expect("defending Wurm");
        game.add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
            .expect("player zero draw card");
        game.add_card(PlayerId(1), "RAV-FOREST", Zone::Library)
            .expect("player one draw card");
        while !(game.turn == 3 && game.step == Step::DeclareAttackers) {
            let priority = game.priority;
            game.pass_priority(priority).expect("advance to combat");
        }

        let mut attacker_policy = GolgariAttritionPolicy::new(PlayerId(0));
        let attack =
            attacker_policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        assert_eq!(
            attack,
            PolicyAction::DeclareAttackers {
                attackers: vec![attacker]
            }
        );
        game.submit_policy_move(PlayerId(0), attacker_policy.id(), attack)
            .expect("policy attack declaration");
        for _ in 0..2 {
            let priority = game.priority;
            game.pass_priority(priority).expect("advance to blockers");
        }

        let mut defender_policy = GolgariAttritionPolicy::new(PlayerId(1));
        let block = defender_policy.propose_move(&game.view_for_player(PlayerId(1)).expect("view"));
        assert_eq!(
            block,
            PolicyAction::DeclareBlockers {
                assignments: vec![CombatBlock { attacker, blocker }]
            }
        );
        game.submit_policy_move(PlayerId(1), defender_policy.id(), block)
            .expect("policy block declaration");
        game.validate_invariants()
            .expect("combat submissions must preserve engine invariants");
    }
}
