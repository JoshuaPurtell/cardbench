use cardbench_magic_engine::{
    CardType, CastRequest, Color, CombatBlock, ConvokeContribution, ConvokePayment, GameView,
    ObjectId, PlayerId, PolicyAction, Step, Target,
};

use crate::CodePolicy;

/// A deterministic black-green grind policy for the public RAV dredge fixture.
///
/// It establishes green creatures before spending them for Scatter the Seeds,
/// retains black mana for legal Last Gasp interaction, and puts inexpensive
/// token creatures in front of attacks first. When the game offers a draw
/// replacement decision, it chooses Golgari Brownscale's dredge ability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GolgariDredgeGrindPolicy {
    player: PlayerId,
}

impl GolgariDredgeGrindPolicy {
    #[must_use]
    pub const fn new(player: PlayerId) -> Self {
        Self { player }
    }
}

impl CodePolicy for GolgariDredgeGrindPolicy {
    fn id(&self) -> &'static str {
        "rav.golgari-dredge-grind.v1"
    }

    #[allow(clippy::too_many_lines)] // Explicit ordering makes this an auditable engine probe.
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

        // Keep this instant live on either player's turn, including in response
        // to a spell, provided the restricted view exposes a legal creature.
        if let (Some(gasp), Some(target)) = (last_gasp_in_hand(view), removal_target(view))
            && view.mana_pool.amount(Color::Black) >= 1
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
            && let Some(land) = select_hand_land(view, Some(desired_land_color(view)))
        {
            return PolicyAction::PlayLand { card: land };
        }

        // This fixture makes the token spell before any missing Brownscale only
        // when the board can legally supply its convoke payment.
        if let Some(card) = card_in_hand(view, "RAV-SCATTER-THE-SEEDS")
            && let Some(convoke) = legal_green_convoke(view, 2, 3)
        {
            return PolicyAction::Cast(CastRequest {
                card,
                targets: vec![],
                convoke,
            });
        }
        if let Some(card) = card_in_hand(view, "RAV-GOLGARI-BROWNSCALE")
            && can_pay_green_cost(view, 2, 1)
        {
            return PolicyAction::Cast(CastRequest {
                card,
                targets: vec![],
                convoke: vec![],
            });
        }
        if let Some((land, color)) = select_mana_ability(view) {
            return PolicyAction::ActivateManaAbility { land, color };
        }
        PolicyAction::PassPriority
    }

    fn propose_draw_replacement(&mut self, view: &GameView) -> PolicyAction {
        let dredge = (view.player == self.player && view.draw_replacement_pending)
            .then(|| {
                view.dredge_candidates
                    .iter()
                    .find(|card| card.definition == Some("RAV-GOLGARI-BROWNSCALE"))
                    .map(|card| card.id)
            })
            .flatten();
        PolicyAction::Draw { dredge }
    }
}

fn card_in_hand(view: &GameView, definition: &str) -> Option<ObjectId> {
    view.hand
        .iter()
        .find(|card| card.definition == Some(definition))
        .map(|card| card.id)
}

fn last_gasp_in_hand(view: &GameView) -> Option<ObjectId> {
    card_in_hand(view, "RAV-LAST-GASP")
}

fn removal_target(view: &GameView) -> Option<ObjectId> {
    view.opponent_battlefield
        .iter()
        .find(|card| card.card_types.contains(&CardType::Creature))
        .map(|card| card.id)
}

fn desired_land_color(view: &GameView) -> Color {
    let has_black_source = view
        .own_battlefield
        .iter()
        .any(|card| card.mana_colors.contains(&Color::Black));
    let green_sources = view
        .own_battlefield
        .iter()
        .filter(|card| card.mana_colors.contains(&Color::Green))
        .count();
    if last_gasp_in_hand(view).is_some() && removal_target(view).is_some() && !has_black_source {
        Color::Black
    } else if green_sources < 2 && card_in_hand(view, "RAV-GOLGARI-BROWNSCALE").is_some() {
        Color::Green
    } else if !has_black_source && last_gasp_in_hand(view).is_some() {
        Color::Black
    } else {
        Color::Green
    }
}

fn select_hand_land(view: &GameView, desired: Option<Color>) -> Option<ObjectId> {
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

fn can_pay_green_cost(view: &GameView, green: u8, generic: u8) -> bool {
    view.mana_pool.amount(Color::Green) >= green
        && view.mana_pool.total().saturating_sub(green) >= generic
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
    let mana_after_colored = view
        .mana_pool
        .total()
        .checked_sub(colored_green - colored_from_creatures)?;
    let generic_from_creatures = generic.saturating_sub(mana_after_colored);
    let required = usize::from(colored_from_creatures + generic_from_creatures);
    if required > creatures.len() {
        return None;
    }
    Some(
        creatures
            .iter()
            .take(usize::from(colored_from_creatures))
            .map(|card| ConvokePayment {
                creature: card.id,
                contribution: ConvokeContribution::Color(Color::Green),
            })
            .chain(
                creatures
                    .iter()
                    .skip(usize::from(colored_from_creatures))
                    .take(usize::from(generic_from_creatures))
                    .map(|card| ConvokePayment {
                        creature: card.id,
                        contribution: ConvokeContribution::Generic,
                    }),
            )
            .collect::<Vec<_>>(),
    )
}

fn select_mana_ability(view: &GameView) -> Option<(ObjectId, Color)> {
    let desired = if last_gasp_in_hand(view).is_some() && removal_target(view).is_some() {
        Color::Black
    } else {
        Color::Green
    };
    view.own_battlefield
        .iter()
        .find(|card| !card.tapped && card.mana_colors.contains(&desired))
        .or_else(|| {
            view.own_battlefield.iter().find(|card| {
                !card.tapped
                    && card.card_types.contains(&CardType::Land)
                    && !card.mana_colors.is_empty()
            })
        })
        .map(|card| {
            let color = if card.mana_colors.contains(&desired) {
                desired
            } else {
                *card.mana_colors.first().expect("lands make mana")
            };
            (card.id, color)
        })
}

fn defensive_assignments(view: &GameView) -> Vec<CombatBlock> {
    let mut blockers: Vec<_> = view
        .own_battlefield
        .iter()
        .filter(|card| !card.tapped && card.card_types.contains(&CardType::Creature))
        .collect();
    blockers.sort_by_key(|card| match card.definition {
        None => 0, // Saprolings are spent first.
        Some("RAV-GOLGARI-BROWNSCALE") => 1,
        _ => 2,
    });
    view.combat_attackers
        .iter()
        .zip(blockers)
        .map(|(attacker, blocker)| CombatBlock {
            attacker: attacker.id,
            blocker: blocker.id,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use cardbench_magic_engine::{Game, GameEvent, Zone};
    use cardbench_magic_rav::card_definitions;

    use super::*;

    fn resolve_top_of_stack(game: &mut Game) {
        let opponent = game.priority;
        game.pass_priority(opponent)
            .expect("opponent accepts spell");
        let controller = game.priority;
        game.pass_priority(controller)
            .expect("controller resolves spell");
    }

    #[test]
    fn submits_removal_and_reaches_a_state_based_action_fixed_point() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        let gasp = game
            .add_card(PlayerId(0), "RAV-LAST-GASP", Zone::Hand)
            .expect("Gasp in hand");
        let target = game
            .put_on_battlefield(PlayerId(1), "RAV-GOLGARI-BROWNSCALE")
            .expect("target creature");
        game.grant_mana(PlayerId(0), Color::Black, 1)
            .expect("black mana");

        let mut policy = GolgariDredgeGrindPolicy::new(PlayerId(0));
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
            .expect("policy removal submission");
        resolve_top_of_stack(&mut game);
        assert_eq!(game.zone_of(target), Some(Zone::Graveyard));
        game.validate_invariants()
            .expect("removal and SBAs preserve invariants");
    }

    #[test]
    fn submits_full_green_convoke_without_using_a_tapped_or_wrong_color_creature() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        let scatter = game
            .add_card(PlayerId(0), "RAV-SCATTER-THE-SEEDS", Zone::Hand)
            .expect("Scatter in hand");
        let creatures: Vec<_> = (0..5)
            .map(|_| {
                game.put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE")
                    .expect("green creature")
            })
            .collect();
        let mut policy = GolgariDredgeGrindPolicy::new(PlayerId(0));
        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        let PolicyAction::Cast(request) = action else {
            panic!("expected legal convoke proposal: {action:?}");
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
            .expect("engine accepts fully convoked Scatter");
        game.validate_invariants()
            .expect("convoke submission preserves invariants");
    }

    #[test]
    fn brownscale_dredge_replacement_preserves_zone_and_event_invariants() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        let brownscale = game
            .add_card(PlayerId(0), "RAV-GOLGARI-BROWNSCALE", Zone::Graveyard)
            .expect("Brownscale in graveyard");
        game.add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
            .expect("first mill card");
        game.add_card(PlayerId(0), "RAV-SWAMP", Zone::Library)
            .expect("second mill card");
        game.draw_card(PlayerId(0), Some(brownscale))
            .expect("legal dredge replacement");
        assert_eq!(game.zone_of(brownscale), Some(Zone::Hand));
        assert!(game.event_log.iter().any(|event| {
            matches!(event, GameEvent::Dredged { player, card, count } if *player == PlayerId(0) && *card == brownscale && *count == 2)
        }));
        game.validate_invariants()
            .expect("dredge must retain a single valid location for every card");
    }

    #[test]
    fn policy_submits_brownscale_dredge_at_the_real_draw_decision_boundary() {
        let player = PlayerId(0);
        let mut game = Game::new(card_definitions(), 3).expect("three-player RAV game");
        let brownscale = game
            .add_card(player, "RAV-GOLGARI-BROWNSCALE", Zone::Graveyard)
            .expect("Brownscale in graveyard");
        game.add_card(player, "RAV-FOREST", Zone::Library)
            .expect("first mill card");
        game.add_card(player, "RAV-SWAMP", Zone::Library)
            .expect("second mill card");
        game.begin_game().expect("prepared game begins at upkeep");
        for _ in 0..3 {
            let holder = game.priority;
            game.pass_priority(holder)
                .expect("every player passes the first upkeep");
        }

        let view = game.view_for_player(player).expect("draw decision view");
        assert!(view.draw_replacement_pending);
        assert_eq!(
            view.dredge_candidates
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            vec![brownscale]
        );
        let mut policy = GolgariDredgeGrindPolicy::new(player);
        let action = policy.propose_draw_replacement(&view);
        assert_eq!(
            action,
            PolicyAction::Draw {
                dredge: Some(brownscale)
            }
        );
        game.submit_policy_move(player, policy.id(), action)
            .expect("policy chooses the legal Dredge replacement");

        assert_eq!(game.zone_of(brownscale), Some(Zone::Hand));
        assert!(game.event_log.iter().any(|event| {
            matches!(event, GameEvent::Dredged { player: event_player, card, count } if *event_player == player && *card == brownscale && *count == 2)
        }));
        assert!(matches!(
            game.event_log.last(),
            Some(GameEvent::PolicyMoveSubmitted { player: event_player, kind: cardbench_magic_engine::PolicyMoveKind::Draw, .. }) if *event_player == player
        ));
        game.validate_invariants()
            .expect("policy-submitted Dredge preserves state-machine invariants");
    }
}
