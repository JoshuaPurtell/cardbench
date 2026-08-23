use cardbench_magic_engine::{
    CardType, CastRequest, Color, CombatBlock, ConvokeContribution, ConvokePayment, GameView,
    ObjectId, PlayerId, PolicyAction, Step, Target,
};

use crate::CodePolicy;

/// A deterministic Golgari board-pressure policy for the public RAV Wurm fixture.
///
/// Unlike the removal-first attrition policy, this one treats Scatter the Seeds
/// as the preferred payoff whenever it is immediately legal, then spends the
/// resulting board on Siege Wurm. The changed sequencing intentionally probes
/// token creation, later convoke payments, and the combat state after tokens die.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GolgariWurmPressPolicy {
    player: PlayerId,
}

impl GolgariWurmPressPolicy {
    #[must_use]
    pub const fn new(player: PlayerId) -> Self {
        Self { player }
    }
}

impl CodePolicy for GolgariWurmPressPolicy {
    fn id(&self) -> &'static str {
        "rav.golgari-wurm-press.v1"
    }

    #[allow(clippy::too_many_lines)] // Branch order is the behavior under test.
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

        // Defensive removal is still a legal instant response, but only when a
        // blocker-sized threat is visible. This preserves the press plan on an
        // otherwise empty opposing board.
        if let (Some(gasp), Some(target)) =
            (card_in_hand(view, "RAV-LAST-GASP"), creature_target(view))
            && view.mana_pool.amount(Color::Black) >= 1
        {
            return cast_targeted(gasp, target);
        }
        if view.active_player != self.player || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0
            && let Some(land) = select_hand_land(view, Some(preferred_land_color(view)))
        {
            return PolicyAction::PlayLand { card: land };
        }

        // The ordering is deliberate: a legal Scatter is always submitted before
        // an equally legal Wurm, making a reproducible board-width probe.
        if let Some(scatter) = card_in_hand(view, "RAV-SCATTER-THE-SEEDS")
            && let Some(convoke) = legal_green_convoke(view, 2, 3)
        {
            return PolicyAction::Cast(CastRequest {
                card: scatter,
                targets: vec![],
                convoke,
                payment_mana_abilities: vec![],
            });
        }
        if let Some(wurm) = card_in_hand(view, "RAV-SIEGE-WURM")
            && let Some(convoke) = legal_green_convoke(view, 2, 5)
        {
            return PolicyAction::Cast(CastRequest {
                card: wurm,
                targets: vec![],
                convoke,
                payment_mana_abilities: vec![],
            });
        }
        if let Some(brownscale) = card_in_hand(view, "RAV-GOLGARI-BROWNSCALE")
            && can_pay_green_cost(view, 2, 1)
        {
            return PolicyAction::Cast(CastRequest {
                card: brownscale,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            });
        }
        if let Some((land, color)) = select_mana_ability(view) {
            return PolicyAction::ActivateManaAbility { land, color };
        }
        PolicyAction::PassPriority
    }
}

fn cast_targeted(card: ObjectId, target: ObjectId) -> PolicyAction {
    PolicyAction::Cast(CastRequest {
        card,
        targets: vec![Target::Permanent(target)],
        convoke: vec![],
        payment_mana_abilities: vec![],
    })
}

fn card_in_hand(view: &GameView, definition: &str) -> Option<ObjectId> {
    view.hand
        .iter()
        .find(|card| card.definition == Some(definition))
        .map(|card| card.id)
}

fn creature_target(view: &GameView) -> Option<ObjectId> {
    view.opponent_battlefield
        .iter()
        .find(|card| card.card_types.contains(&CardType::Creature))
        .map(|card| card.id)
}

fn preferred_land_color(view: &GameView) -> Color {
    let green_sources = view
        .own_battlefield
        .iter()
        .filter(|card| card.mana_colors.contains(&Color::Green))
        .count();
    let black_sources = view
        .own_battlefield
        .iter()
        .filter(|card| card.mana_colors.contains(&Color::Black))
        .count();
    if green_sources < 2 {
        Color::Green
    } else if creature_target(view).is_some()
        && card_in_hand(view, "RAV-LAST-GASP").is_some()
        && black_sources == 0
    {
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
    let remaining_colored = colored_green - colored_from_creatures;
    if usize::from(colored_from_creatures) > creatures.len() {
        return None;
    }
    let generic_from_creatures =
        generic.saturating_sub(view.mana_pool.total().checked_sub(remaining_colored)?);
    let required = usize::from(colored_from_creatures + generic_from_creatures);
    if required > creatures.len() {
        return None;
    }
    let mut payment = Vec::with_capacity(required);
    for creature in creatures.iter().take(usize::from(colored_from_creatures)) {
        payment.push(ConvokePayment {
            creature: creature.id,
            contribution: ConvokeContribution::Color(Color::Green),
        });
    }
    for creature in creatures
        .iter()
        .skip(usize::from(colored_from_creatures))
        .take(usize::from(generic_from_creatures))
    {
        payment.push(ConvokePayment {
            creature: creature.id,
            contribution: ConvokeContribution::Generic,
        });
    }
    Some(payment)
}

fn select_mana_ability(view: &GameView) -> Option<(ObjectId, Color)> {
    let desired = if card_in_hand(view, "RAV-LAST-GASP").is_some()
        && creature_target(view).is_some()
        && view.mana_pool.amount(Color::Black) == 0
    {
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
        None => 0,
        Some("RAV-GOLGARI-BROWNSCALE") => 1,
        Some("RAV-SIEGE-WURM") => 2,
        _ => 3,
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
    use cardbench_magic_engine::{Game, Zone};
    use cardbench_magic_rav::card_definitions;

    use super::*;

    #[test]
    fn prefers_a_legal_scatter_before_an_equally_legal_wurm_submission() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        let scatter = game
            .add_card(PlayerId(0), "RAV-SCATTER-THE-SEEDS", Zone::Hand)
            .expect("Scatter in hand");
        game.add_card(PlayerId(0), "RAV-SIEGE-WURM", Zone::Hand)
            .expect("Wurm in hand");
        game.grant_mana(PlayerId(0), Color::Green, 7)
            .expect("mana for either card");
        let mut policy = GolgariWurmPressPolicy::new(PlayerId(0));
        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        assert_eq!(
            action,
            PolicyAction::Cast(CastRequest {
                card: scatter,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            })
        );
        game.submit_policy_move(PlayerId(0), policy.id(), action)
            .expect("Scatter proposal is accepted by the engine");
        game.validate_invariants()
            .expect("submission preserves invariants");
    }

    #[test]
    fn submits_a_fully_convoked_wurm_and_taps_only_its_contributors() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        let wurm = game
            .add_card(PlayerId(0), "RAV-SIEGE-WURM", Zone::Hand)
            .expect("Wurm in hand");
        let creatures: Vec<_> = (0..7)
            .map(|_| {
                game.put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE")
                    .expect("green contributor")
            })
            .collect();
        let mut policy = GolgariWurmPressPolicy::new(PlayerId(0));
        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        let PolicyAction::Cast(request) = action else {
            panic!("expected Wurm proposal: {action:?}");
        };
        assert_eq!(request.card, wurm);
        assert_eq!(request.convoke.len(), 7);
        assert!(
            request
                .convoke
                .iter()
                .all(|payment| creatures.contains(&payment.creature))
        );
        game.submit_policy_move(PlayerId(0), policy.id(), PolicyAction::Cast(request))
            .expect("engine accepts Wurm convoke payment");
        game.validate_invariants()
            .expect("Wurm submission preserves invariants");
    }

    #[test]
    fn submits_explicit_combat_and_spends_a_brownscale_before_a_wurm_as_blocker() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        let attacker = game
            .put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE")
            .expect("attacking creature");
        let brownscale = game
            .put_on_battlefield(PlayerId(1), "RAV-GOLGARI-BROWNSCALE")
            .expect("expendable blocker");
        game.put_on_battlefield(PlayerId(1), "RAV-SIEGE-WURM")
            .expect("valuable defender");
        game.add_card(PlayerId(0), "RAV-FOREST", Zone::Library)
            .expect("draw card");
        game.add_card(PlayerId(1), "RAV-FOREST", Zone::Library)
            .expect("draw card");
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
            game.pass_priority(priority).expect("advance game");
        }
        let mut attacker_policy = GolgariWurmPressPolicy::new(PlayerId(0));
        let attack =
            attacker_policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        game.submit_policy_move(PlayerId(0), attacker_policy.id(), attack)
            .expect("attack submission");
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
            game.pass_priority(priority).expect("advance to blockers");
        }
        let mut defender_policy = GolgariWurmPressPolicy::new(PlayerId(1));
        let block = defender_policy.propose_move(&game.view_for_player(PlayerId(1)).expect("view"));
        assert_eq!(
            block,
            PolicyAction::DeclareBlockers {
                assignments: vec![CombatBlock {
                    attacker,
                    blocker: brownscale,
                }],
            }
        );
        game.submit_policy_move(PlayerId(1), defender_policy.id(), block)
            .expect("Brownscale block submission");
        game.validate_invariants()
            .expect("combat submissions preserve invariants");
    }
}
