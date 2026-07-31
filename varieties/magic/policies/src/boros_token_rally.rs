use cardbench_magic_engine::{
    CardType, CastRequest, Color, CombatBlock, ConvokeContribution, ConvokePayment, GameView,
    ObjectId, PlayerId, PolicyAction, Step, Target,
};

use crate::CodePolicy;

/// A Boros-forward token policy for the executable public RAV slice.
///
/// Green card slots are a visible development splash: they create a creature
/// board for convoke and radiance, while the policy's red-white decisions use
/// Rally and Helix. It is deliberately deterministic so any rejected proposal
/// is a reproducible policy or engine finding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BorosTokenRallyPolicy {
    player: PlayerId,
}

impl BorosTokenRallyPolicy {
    #[must_use]
    pub const fn new(player: PlayerId) -> Self {
        Self { player }
    }
}

impl CodePolicy for BorosTokenRallyPolicy {
    fn id(&self) -> &'static str {
        "rav.boros-token-rally.v1"
    }

    #[allow(clippy::too_many_lines)] // Explicit priority actions make the probe auditable.
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
        if view.active_player != self.player
            || view.priority != self.player
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }

        let has_scatter = has_card(view, "RAV-SCATTER-THE-SEEDS");
        let has_rally = has_card(view, "RAV-RALLY-THE-RIGHTEOUS");
        let has_brownscale = has_card(view, "RAV-GOLGARI-BROWNSCALE");
        let has_helix = has_card(view, "RAV-LIGHTNING-HELIX");
        if view.lands_played == 0
            && let Some(land) = select_hand_land(
                view,
                desired_land_color(view, has_scatter, has_rally, has_brownscale, has_helix),
            )
        {
            return PolicyAction::PlayLand { card: land };
        }

        if let Some(card) = card_in_hand(view, "RAV-SCATTER-THE-SEEDS")
            && let Some(convoke) = legal_green_convoke(view, 2, 3)
        {
            return PolicyAction::Cast(CastRequest {
                card,
                targets: vec![],
                convoke,
                payment_mana_abilities: vec![],
            });
        }
        if let Some(card) = card_in_hand(view, "RAV-RALLY-THE-RIGHTEOUS")
            && can_pay_rally(view)
            && let Some(target) = green_creature_target(view)
        {
            return cast(card, vec![Target::Permanent(target)]);
        }
        if let Some(card) = card_in_hand(view, "RAV-GOLGARI-BROWNSCALE")
            && can_pay_green_two_and_one(view)
        {
            return cast(card, vec![]);
        }
        if let Some(card) = card_in_hand(view, "RAV-LIGHTNING-HELIX")
            && can_pay_helix(view)
        {
            return cast(card, vec![damage_target(view)]);
        }
        if (has_scatter || has_rally || has_brownscale || has_helix)
            && let Some(land) = select_battlefield_land(
                view,
                desired_mana_color(view, has_scatter, has_rally, has_brownscale, has_helix),
            )
        {
            let desired =
                desired_mana_color(view, has_scatter, has_rally, has_brownscale, has_helix);
            let color = desired
                .filter(|color| land.mana_colors.contains(color))
                .unwrap_or_else(|| *land.mana_colors.first().expect("basic land has a color"));
            return PolicyAction::ActivateManaAbility {
                land: land.id,
                color,
            };
        }
        PolicyAction::PassPriority
    }
}

fn cast(card: ObjectId, targets: Vec<Target>) -> PolicyAction {
    PolicyAction::Cast(CastRequest {
        card,
        targets,
        convoke: vec![],
        payment_mana_abilities: vec![],
    })
}

fn has_card(view: &GameView, definition: &str) -> bool {
    card_in_hand(view, definition).is_some()
}

fn card_in_hand(view: &GameView, definition: &str) -> Option<ObjectId> {
    view.hand
        .iter()
        .find(|card| card.definition == Some(definition))
        .map(|card| card.id)
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
    let mut payments = Vec::with_capacity(required_creatures);
    for creature in creatures.iter().take(usize::from(colored_from_creatures)) {
        payments.push(ConvokePayment {
            creature: creature.id,
            contribution: ConvokeContribution::Color(Color::Green),
        });
    }
    for creature in creatures
        .iter()
        .skip(usize::from(colored_from_creatures))
        .take(usize::from(generic_from_creatures))
    {
        payments.push(ConvokePayment {
            creature: creature.id,
            contribution: ConvokeContribution::Generic,
        });
    }
    Some(payments)
}

fn can_pay_green_two_and_one(view: &GameView) -> bool {
    view.mana_pool.amount(Color::Green) >= 2 && view.mana_pool.total() >= 3
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

fn damage_target(view: &GameView) -> Target {
    view.opponent_battlefield
        .iter()
        .find(|card| card.card_types.contains(&CardType::Creature))
        .map_or_else(
            || {
                Target::Player(
                    view.opponent_life
                        .first()
                        .map_or(PlayerId(0), |(id, _)| *id),
                )
            },
            |card| Target::Permanent(card.id),
        )
}

#[allow(clippy::fn_params_excessive_bools)] // Spell-presence flags keep ordering explicit.
fn desired_land_color(
    view: &GameView,
    has_scatter: bool,
    has_rally: bool,
    has_brownscale: bool,
    has_helix: bool,
) -> Option<Color> {
    if (has_scatter || has_brownscale) && lands_producing(view, Color::Green) < 2 {
        Some(Color::Green)
    } else if (has_rally || has_helix) && lands_producing(view, Color::White) == 0 {
        Some(Color::White)
    } else if (has_rally || has_helix) && lands_producing(view, Color::Red) == 0 {
        Some(Color::Red)
    } else {
        None
    }
}

#[allow(clippy::fn_params_excessive_bools)] // Spell-presence flags keep ordering explicit.
fn desired_mana_color(
    view: &GameView,
    has_scatter: bool,
    has_rally: bool,
    has_brownscale: bool,
    has_helix: bool,
) -> Option<Color> {
    if (has_scatter || has_brownscale) && view.mana_pool.amount(Color::Green) < 2 {
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
    } else if has_helix && view.mana_pool.amount(Color::White) == 0 {
        Some(Color::White)
    } else if (has_rally || has_helix) && view.mana_pool.amount(Color::Red) == 0 {
        Some(Color::Red)
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
    view.combat_attackers
        .iter()
        .zip(
            view.own_battlefield
                .iter()
                .filter(|card| !card.tapped && card.card_types.contains(&CardType::Creature)),
        )
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

    fn resolve_top_of_stack(game: &mut Game) {
        let first = game.priority;
        game.pass_priority(first).expect("first pass");
        let second = game.priority;
        game.pass_priority(second).expect("second pass resolves");
    }

    #[test]
    fn submits_scatter_then_rally_on_a_created_token() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        let scatter = game
            .add_card(PlayerId(0), "RAV-SCATTER-THE-SEEDS", Zone::Hand)
            .expect("Scatter in hand");
        let rally = game
            .add_card(PlayerId(0), "RAV-RALLY-THE-RIGHTEOUS", Zone::Hand)
            .expect("Rally in hand");
        game.grant_mana(PlayerId(0), Color::Green, 5)
            .expect("Scatter mana");
        let mut policy = BorosTokenRallyPolicy::new(PlayerId(0));
        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        assert_eq!(action, cast(scatter, vec![]));
        game.submit_policy_move(PlayerId(0), policy.id(), action)
            .expect("Scatter policy submission is legal");
        resolve_top_of_stack(&mut game);
        let target = game.players[0]
            .battlefield
            .iter()
            .copied()
            .find(|id| game.object(*id).is_ok_and(|object| object.token.is_some()))
            .expect("Scatter creates token");
        game.grant_mana(PlayerId(0), Color::White, 1)
            .expect("white mana");
        game.grant_mana(PlayerId(0), Color::Red, 1)
            .expect("red mana");
        game.grant_mana(PlayerId(0), Color::Green, 1)
            .expect("generic mana");
        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        assert_eq!(action, cast(rally, vec![Target::Permanent(target)]));
        game.submit_policy_move(PlayerId(0), policy.id(), action)
            .expect("Rally policy submission is legal");
        resolve_top_of_stack(&mut game);
        assert_eq!(game.characteristics(target).expect("token").power, Some(3));
        game.validate_invariants()
            .expect("token/radiance submissions preserve invariants");
    }

    #[test]
    fn submits_a_fully_convoked_scatter_and_preserves_invariants() {
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
        let mut policy = BorosTokenRallyPolicy::new(PlayerId(0));
        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        let PolicyAction::Cast(request) = action else {
            panic!("policy must propose Scatter through convoke: {action:?}");
        };
        assert_eq!(request.card, scatter);
        assert_eq!(request.convoke.len(), 5);
        game.submit_policy_move(PlayerId(0), policy.id(), PolicyAction::Cast(request))
            .expect("fully convoked policy submission is legal");
        assert!(
            creatures
                .iter()
                .all(|card| game.object(*card).is_ok_and(|object| object.tapped))
        );
        game.validate_invariants()
            .expect("convoke policy submission preserves invariants");
    }
}
