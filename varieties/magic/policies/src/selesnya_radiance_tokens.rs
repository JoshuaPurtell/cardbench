use cardbench_magic_engine::{
    CardType, CastRequest, Color, CombatBlock, ConvokeContribution, ConvokePayment, GameView,
    ObjectId, PlayerId, PolicyAction, Step, Target,
};

use crate::CodePolicy;

/// A deterministic Naya-colored token/radiance development policy for public RAV
/// fixtures.
///
/// The policy deliberately builds a green creature board with Brownscale, Scatter
/// the Seeds, and (where affordable) Siege Wurm. It then targets a green creature
/// with Rally the Righteous so the policy exercises the engine's radiance target
/// selection, untap, and layer-seven power/toughness paths. This is development
/// code, not an attempt to model tournament play.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelesnyaRadianceTokensPolicy {
    player: PlayerId,
}

impl SelesnyaRadianceTokensPolicy {
    #[must_use]
    pub const fn new(player: PlayerId) -> Self {
        Self { player }
    }
}

impl CodePolicy for SelesnyaRadianceTokensPolicy {
    fn id(&self) -> &'static str {
        "rav.selesnya-radiance-tokens.v1"
    }

    #[allow(clippy::too_many_lines)] // Linear branches make policy choices audit-friendly.
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
            && let Some(land) = select_hand_land(view, desired_land_color(view))
        {
            return PolicyAction::PlayLand { card: land };
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
            .find(|card| card.definition == Some("RAV-RALLY-THE-RIGHTEOUS"))
            && can_pay_rally(view)
            && let Some(target) = green_creature_target(view)
        {
            return PolicyAction::Cast(CastRequest {
                card: card.id,
                targets: vec![Target::Permanent(target)],
                convoke: vec![],
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

        if has_development_spell(view)
            && let Some(land) = select_battlefield_land(view, desired_mana_color(view))
        {
            let color = desired_mana_color(view)
                .filter(|color| land.mana_colors.contains(color))
                .unwrap_or_else(|| *land.mana_colors.first().expect("basic lands make mana"));
            return PolicyAction::ActivateManaAbility {
                land: land.id,
                color,
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

fn can_pay_rally(view: &GameView) -> bool {
    view.mana_pool.amount(Color::White) >= 1
        && view.mana_pool.amount(Color::Red) >= 1
        && view.mana_pool.total() >= 3
}

fn green_creature_target(view: &GameView) -> Option<ObjectId> {
    let creatures = view.own_battlefield.iter().filter(|card| {
        card.colors.contains(&Color::Green) && card.card_types.contains(&CardType::Creature)
    });
    creatures
        .clone()
        .find(|card| card.tapped)
        .or_else(|| creatures.into_iter().next())
        .map(|card| card.id)
}

fn has_development_spell(view: &GameView) -> bool {
    view.hand.iter().any(|card| {
        matches!(
            card.definition,
            Some(
                "RAV-SCATTER-THE-SEEDS"
                    | "RAV-RALLY-THE-RIGHTEOUS"
                    | "RAV-GOLGARI-BROWNSCALE"
                    | "RAV-SIEGE-WURM"
            )
        )
    })
}

fn desired_land_color(view: &GameView) -> Option<Color> {
    let forest_count = lands_producing(view, Color::Green);
    let plains_count = lands_producing(view, Color::White);
    let mountain_count = lands_producing(view, Color::Red);
    let wants_green = view.hand.iter().any(|card| {
        matches!(
            card.definition,
            Some("RAV-SCATTER-THE-SEEDS" | "RAV-GOLGARI-BROWNSCALE" | "RAV-SIEGE-WURM")
        )
    });
    let wants_rally = view
        .hand
        .iter()
        .any(|card| card.definition == Some("RAV-RALLY-THE-RIGHTEOUS"));
    if wants_green && forest_count < 2 {
        Some(Color::Green)
    } else if wants_rally && plains_count == 0 {
        Some(Color::White)
    } else if wants_rally && mountain_count == 0 {
        Some(Color::Red)
    } else if wants_green {
        Some(Color::Green)
    } else {
        None
    }
}

fn desired_mana_color(view: &GameView) -> Option<Color> {
    let has_scatter = view
        .hand
        .iter()
        .any(|card| card.definition == Some("RAV-SCATTER-THE-SEEDS"));
    let has_rally = view
        .hand
        .iter()
        .any(|card| card.definition == Some("RAV-RALLY-THE-RIGHTEOUS"));
    let has_green_creature = view.hand.iter().any(|card| {
        matches!(
            card.definition,
            Some("RAV-GOLGARI-BROWNSCALE" | "RAV-SIEGE-WURM")
        )
    });
    if has_scatter && view.mana_pool.amount(Color::Green) < 2 {
        Some(Color::Green)
    } else if has_rally
        && green_creature_target(view).is_some()
        && view.mana_pool.amount(Color::White) == 0
    {
        Some(Color::White)
    } else if has_rally
        && green_creature_target(view).is_some()
        && view.mana_pool.amount(Color::Red) == 0
    {
        Some(Color::Red)
    } else if has_green_creature && view.mana_pool.amount(Color::Green) < 2 {
        Some(Color::Green)
    } else {
        None
    }
}

fn lands_producing(view: &GameView, color: Color) -> usize {
    view.own_battlefield
        .iter()
        .filter(|card| {
            card.card_types.contains(&CardType::Land) && card.mana_colors.contains(&color)
        })
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

fn select_battlefield_land(
    view: &GameView,
    desired: Option<Color>,
) -> Option<&cardbench_magic_engine::CardView> {
    let mut lands = view.own_battlefield.iter().filter(|card| {
        !card.tapped && card.card_types.contains(&CardType::Land) && !card.mana_colors.is_empty()
    });
    if let Some(color) = desired
        && let Some(land) = lands.clone().find(|card| card.mana_colors.contains(&color))
    {
        return Some(land);
    }
    lands.next()
}

fn defensive_assignments(view: &GameView) -> Vec<CombatBlock> {
    let mut blockers: Vec<_> = view
        .own_battlefield
        .iter()
        .filter(|card| !card.tapped && card.card_types.contains(&CardType::Creature))
        .collect();
    blockers.sort_by_key(|card| defender_priority(card.definition));
    view.combat_attackers
        .iter()
        .zip(blockers)
        .map(|(attacker, blocker)| CombatBlock {
            attacker: attacker.id,
            blocker: blocker.id,
        })
        .collect()
}

fn defender_priority(definition: Option<&str>) -> u8 {
    match definition {
        None => 0, // Saproling tokens are expendable blockers.
        Some("RAV-GOLGARI-BROWNSCALE") => 1,
        Some("RAV-SIEGE-WURM") => 2,
        _ => 3,
    }
}

#[cfg(test)]
mod tests {
    use cardbench_magic_engine::{Game, GameEvent, Zone};
    use cardbench_magic_rav::card_definitions;

    use super::*;

    fn resolve_top_of_stack(game: &mut Game) {
        let first = game.priority;
        game.pass_priority(first).expect("opponent accepts spell");
        let second = game.priority;
        game.pass_priority(second)
            .expect("controller resolves spell");
    }

    #[test]
    fn submits_scatter_then_green_radiance_through_the_real_engine() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        let scatter = game
            .add_card(PlayerId(0), "RAV-SCATTER-THE-SEEDS", Zone::Hand)
            .expect("Scatter in hand");
        let rally = game
            .add_card(PlayerId(0), "RAV-RALLY-THE-RIGHTEOUS", Zone::Hand)
            .expect("Rally in hand");
        let brownscale = game
            .put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE")
            .expect("green radiance target");
        game.set_tapped_for_setup(brownscale, true)
            .expect("tapped setup creature");
        game.grant_mana(PlayerId(0), Color::Green, 5)
            .expect("Scatter mana");

        let mut policy = SelesnyaRadianceTokensPolicy::new(PlayerId(0));
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
            .expect("policy Scatter submission is legal");
        resolve_top_of_stack(&mut game);

        let tokens: Vec<_> = game.players[0]
            .battlefield
            .iter()
            .copied()
            .filter(|card| {
                game.object(*card)
                    .is_ok_and(|object| object.token.is_some())
            })
            .collect();
        assert_eq!(tokens.len(), 3);
        game.grant_mana(PlayerId(0), Color::White, 1)
            .expect("Rally white mana");
        game.grant_mana(PlayerId(0), Color::Red, 2)
            .expect("Rally red and generic mana");

        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        assert_eq!(
            action,
            PolicyAction::Cast(CastRequest {
                card: rally,
                targets: vec![Target::Permanent(brownscale)],
                convoke: vec![],
                payment_mana_abilities: vec![],
            })
        );
        game.submit_policy_move(PlayerId(0), policy.id(), action)
            .expect("policy Rally submission is legal");
        resolve_top_of_stack(&mut game);

        assert!(!game.object(brownscale).expect("Brownscale exists").tapped);
        assert_eq!(
            game.characteristics(brownscale)
                .expect("Brownscale characteristics")
                .power,
            Some(4)
        );
        for token in tokens {
            let characteristics = game
                .characteristics(token)
                .expect("Saproling characteristics");
            assert_eq!(characteristics.power, Some(3));
            assert_eq!(characteristics.toughness, Some(1));
        }
        assert!(game.event_log.iter().any(|event| matches!(event, GameEvent::PermanentsUntapped { cards, .. } if cards.contains(&brownscale))));
        game.validate_invariants()
            .expect("token/radiance policy submissions preserve invariants");
    }

    #[test]
    fn submits_a_fully_convoked_scatter_without_illegal_payment() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        let scatter = game
            .add_card(PlayerId(0), "RAV-SCATTER-THE-SEEDS", Zone::Hand)
            .expect("Scatter in hand");
        let creatures: Vec<_> = (0..5)
            .map(|_| {
                game.put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE")
                    .expect("green convoke creature")
            })
            .collect();
        let mut policy = SelesnyaRadianceTokensPolicy::new(PlayerId(0));
        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        let PolicyAction::Cast(request) = action else {
            panic!("expected a legal convoked Scatter proposal");
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
            .expect("convoke payment must be accepted by the real engine");
        assert!(
            creatures
                .iter()
                .all(|card| game.object(*card).is_ok_and(|object| object.tapped))
        );
        game.validate_invariants()
            .expect("convoke submission preserves invariants");
    }
}
