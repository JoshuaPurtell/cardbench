use cardbench_magic_engine::{
    CardType, CastRequest, Color, CombatBlock, ConvokeContribution, ConvokePayment, GameView,
    ObjectId, PlayerId, PolicyAction, Step, Target,
};

use crate::CodePolicy;

/// A deterministic three-color radiance/convoke pressure policy for public RAV
/// fixtures.
///
/// Its sequence is intentionally distinct from the existing token policy: build
/// a green board, make tokens, apply Rally to an own green attacker, then spend
/// Helix as a targeted tempo/life-swing spell. Every action comes from the
/// restricted `GameView` and is submitted to the real engine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RadianceConvokeAssaultPolicy {
    player: PlayerId,
}

impl RadianceConvokeAssaultPolicy {
    #[must_use]
    pub const fn new(player: PlayerId) -> Self {
        Self { player }
    }
}

impl CodePolicy for RadianceConvokeAssaultPolicy {
    fn id(&self) -> &'static str {
        "rav.radiance-convoke-assault.v1"
    }

    #[allow(clippy::too_many_lines)] // The priority sequence is a readable audit surface.
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

        // Lightning Helix stays legal on either player's main step or atop a
        // stack. This ordering lets the policy test player and permanent targets.
        if let Some(helix) = card_in_hand(view, "RAV-LIGHTNING-HELIX")
            && can_pay_helix(view)
            && (view.active_player != self.player || view.stack_depth != 0)
        {
            return cast_targeted(helix, helix_target(view));
        }
        if view.active_player != self.player || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }
        if view.lands_played == 0
            && let Some(land) = select_hand_land(view, Some(desired_land_color(view)))
        {
            return PolicyAction::PlayLand { card: land };
        }

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
        if let (Some(rally), Some(target)) = (
            card_in_hand(view, "RAV-RALLY-THE-RIGHTEOUS"),
            own_green_creature(view),
        ) && can_pay_rally(view)
        {
            return cast_targeted(rally, Target::Permanent(target));
        }
        if let Some(helix) = card_in_hand(view, "RAV-LIGHTNING-HELIX")
            && can_pay_helix(view)
        {
            return cast_targeted(helix, helix_target(view));
        }
        if let Some((land, color)) = select_mana_ability(view) {
            return PolicyAction::ActivateManaAbility { land, color };
        }
        PolicyAction::PassPriority
    }
}

fn card_in_hand(view: &GameView, definition: &str) -> Option<ObjectId> {
    view.hand
        .iter()
        .find(|card| card.definition == Some(definition))
        .map(|card| card.id)
}

fn cast_targeted(card: ObjectId, target: Target) -> PolicyAction {
    PolicyAction::Cast(CastRequest {
        card,
        targets: vec![target],
        convoke: vec![],
        payment_mana_abilities: vec![],
    })
}

fn helix_target(view: &GameView) -> Target {
    view.opponent_battlefield
        .iter()
        .find(|card| card.card_types.contains(&CardType::Creature))
        .map_or_else(
            || {
                Target::Player(
                    view.opponent_life
                        .first()
                        .map_or(PlayerId(0), |(player, _)| *player),
                )
            },
            |card| Target::Permanent(card.id),
        )
}

fn own_green_creature(view: &GameView) -> Option<ObjectId> {
    let creatures = view.own_battlefield.iter().filter(|card| {
        card.colors.contains(&Color::Green) && card.card_types.contains(&CardType::Creature)
    });
    creatures
        .clone()
        .find(|card| card.tapped)
        .or_else(|| creatures.into_iter().next())
        .map(|card| card.id)
}

fn can_pay_green_cost(view: &GameView, green: u8, generic: u8) -> bool {
    view.mana_pool.amount(Color::Green) >= green
        && view.mana_pool.total().saturating_sub(green) >= generic
}

fn can_pay_rally(view: &GameView) -> bool {
    view.mana_pool.amount(Color::White) >= 1
        && view.mana_pool.amount(Color::Red) >= 1
        && view.mana_pool.total() >= 3
}

fn can_pay_helix(view: &GameView) -> bool {
    view.mana_pool.amount(Color::White) >= 1
        && view.mana_pool.amount(Color::Red) >= 1
        && view.mana_pool.total() >= 2
}

fn desired_land_color(view: &GameView) -> Color {
    let green_sources = lands_producing(view, Color::Green);
    let white_sources = lands_producing(view, Color::White);
    let red_sources = lands_producing(view, Color::Red);
    let wants_green = card_in_hand(view, "RAV-GOLGARI-BROWNSCALE").is_some()
        || card_in_hand(view, "RAV-SCATTER-THE-SEEDS").is_some();
    let wants_white_red = card_in_hand(view, "RAV-RALLY-THE-RIGHTEOUS").is_some()
        || card_in_hand(view, "RAV-LIGHTNING-HELIX").is_some();
    if wants_green && green_sources < 2 {
        Color::Green
    } else if wants_white_red && white_sources == 0 {
        Color::White
    } else if wants_white_red && red_sources == 0 {
        Color::Red
    } else if wants_green {
        Color::Green
    } else {
        Color::White
    }
}

fn lands_producing(view: &GameView, color: Color) -> usize {
    view.own_battlefield
        .iter()
        .filter(|card| card.mana_colors.contains(&color))
        .count()
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
    let generic_needed = card_in_hand(view, "RAV-SCATTER-THE-SEEDS").is_some()
        && (view.mana_pool.amount(Color::Green) < 2 || view.mana_pool.total() < 5)
        || card_in_hand(view, "RAV-GOLGARI-BROWNSCALE").is_some()
            && (view.mana_pool.amount(Color::Green) < 2 || view.mana_pool.total() < 3)
        || own_green_creature(view).is_some()
            && card_in_hand(view, "RAV-RALLY-THE-RIGHTEOUS").is_some()
            && view.mana_pool.total() < 3
        || card_in_hand(view, "RAV-LIGHTNING-HELIX").is_some() && view.mana_pool.total() < 2;
    let desired = if card_in_hand(view, "RAV-SCATTER-THE-SEEDS").is_some()
        && view.mana_pool.amount(Color::Green) < 2
    {
        Some(Color::Green)
    } else if own_green_creature(view).is_some()
        && card_in_hand(view, "RAV-RALLY-THE-RIGHTEOUS").is_some()
        && view.mana_pool.amount(Color::White) == 0
    {
        Some(Color::White)
    } else if (card_in_hand(view, "RAV-RALLY-THE-RIGHTEOUS").is_some()
        || card_in_hand(view, "RAV-LIGHTNING-HELIX").is_some())
        && view.mana_pool.amount(Color::Red) == 0
    {
        Some(Color::Red)
    } else if card_in_hand(view, "RAV-GOLGARI-BROWNSCALE").is_some()
        && view.mana_pool.amount(Color::Green) < 2
    {
        Some(Color::Green)
    } else if generic_needed {
        None
    } else {
        return None;
    };
    view.own_battlefield
        .iter()
        .find(|card| {
            !card.tapped
                && desired.is_none_or(|color| card.mana_colors.contains(&color))
                && !card.mana_colors.is_empty()
        })
        .map(|card| {
            let color = desired
                .filter(|color| card.mana_colors.contains(color))
                .unwrap_or_else(|| *card.mana_colors.first().expect("lands make mana"));
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
    fn submits_radiance_with_a_real_target_and_preserves_layer_invariants() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        let rally = game
            .add_card(PlayerId(0), "RAV-RALLY-THE-RIGHTEOUS", Zone::Hand)
            .expect("Rally in hand");
        let target = game
            .put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE")
            .expect("green target");
        game.set_tapped_for_setup(target, true)
            .expect("tapped setup creature");
        game.grant_mana(PlayerId(0), Color::White, 1)
            .expect("white mana");
        game.grant_mana(PlayerId(0), Color::Red, 2)
            .expect("red and generic mana");
        let mut policy = RadianceConvokeAssaultPolicy::new(PlayerId(0));
        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        assert_eq!(action, cast_targeted(rally, Target::Permanent(target)));
        game.submit_policy_move(PlayerId(0), policy.id(), action)
            .expect("Rally proposal is legal");
        resolve_top_of_stack(&mut game);
        assert!(!game.object(target).expect("target exists").tapped);
        assert_eq!(
            game.characteristics(target).expect("characteristics").power,
            Some(4)
        );
        assert!(game.event_log.iter().any(|event| {
            matches!(event, GameEvent::PermanentsUntapped { cards, .. } if cards.contains(&target))
        }));
        game.validate_invariants()
            .expect("radiance resolution preserves invariants");
    }

    #[test]
    fn submits_helix_to_a_visible_opponent_and_records_the_life_swing() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        let helix = game
            .add_card(PlayerId(0), "RAV-LIGHTNING-HELIX", Zone::Hand)
            .expect("Helix in hand");
        game.players[0].life = 11;
        game.grant_mana(PlayerId(0), Color::White, 1)
            .expect("white mana");
        game.grant_mana(PlayerId(0), Color::Red, 1)
            .expect("red mana");
        let mut policy = RadianceConvokeAssaultPolicy::new(PlayerId(0));
        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        assert_eq!(action, cast_targeted(helix, Target::Player(PlayerId(1))));
        game.submit_policy_move(PlayerId(0), policy.id(), action)
            .expect("Helix proposal is legal");
        resolve_top_of_stack(&mut game);
        assert_eq!(game.players[0].life, 14);
        assert_eq!(game.players[1].life, 17);
        game.validate_invariants()
            .expect("Helix resolution preserves invariants");
    }

    #[test]
    fn submits_full_convoke_then_retains_valid_policy_view_for_all_creatures() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        let scatter = game
            .add_card(PlayerId(0), "RAV-SCATTER-THE-SEEDS", Zone::Hand)
            .expect("Scatter in hand");
        let creatures: Vec<_> = (0..5)
            .map(|_| {
                game.put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE")
                    .expect("convoke contributor")
            })
            .collect();
        let mut policy = RadianceConvokeAssaultPolicy::new(PlayerId(0));
        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        let PolicyAction::Cast(request) = action else {
            panic!("expected Scatter proposal: {action:?}");
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
            .expect("fully convoked Scatter is legal");
        resolve_top_of_stack(&mut game);
        assert_eq!(
            game.players[0]
                .battlefield
                .iter()
                .filter(|card| game
                    .object(**card)
                    .is_ok_and(|object| object.token.is_some()))
                .count(),
            3
        );
        game.validate_invariants()
            .expect("token creation preserves invariants");
    }
}
