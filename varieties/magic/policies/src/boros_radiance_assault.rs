use cardbench_magic_engine::{
    CardType, CastRequest, Color, CombatBlock, GameView, ObjectId, PlayerId, PolicyAction, Step,
    Target,
};

use crate::CodePolicy;

/// A transparent red-white assault policy with an explicit green development
/// splash for the executable RAV radiance path.
///
/// It establishes a durable creature first, applies Rally to that creature when
/// the mana and target are present, and then uses its burn spells to remove an
/// opposing creature or reduce the defending player's life total. This is a
/// deterministic engine probe, not a claim to reproduce an historical deck.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BorosRadianceAssaultPolicy {
    player: PlayerId,
}

impl BorosRadianceAssaultPolicy {
    #[must_use]
    pub const fn new(player: PlayerId) -> Self {
        Self { player }
    }
}

impl CodePolicy for BorosRadianceAssaultPolicy {
    fn id(&self) -> &'static str {
        "rav.boros-radiance-assault.v1"
    }

    #[allow(clippy::too_many_lines)] // Branches deliberately document every engine submission.
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

        let has_brownscale = has_card(view, "RAV-GOLGARI-BROWNSCALE");
        let has_rally = has_card(view, "RAV-RALLY-THE-RIGHTEOUS");
        let has_helix = has_card(view, "RAV-LIGHTNING-HELIX");
        let has_char = has_card(view, "RAV-CHAR");

        if view.lands_played == 0
            && let Some(land) = select_hand_land(
                view,
                desired_land_color(view, has_brownscale, has_rally, has_helix, has_char),
            )
        {
            return PolicyAction::PlayLand { card: land };
        }

        if let Some(card) = card_in_hand(view, "RAV-GOLGARI-BROWNSCALE")
            && can_pay_green_two_and_one(view)
        {
            return cast(card, vec![]);
        }
        if let Some(card) = card_in_hand(view, "RAV-RALLY-THE-RIGHTEOUS")
            && can_pay_rally(view)
            && let Some(target) = green_creature_target(view)
        {
            return cast(card, vec![Target::Permanent(target)]);
        }
        if let Some(card) = card_in_hand(view, "RAV-LIGHTNING-HELIX")
            && can_pay_helix(view)
        {
            return cast(card, vec![damage_target(view)]);
        }
        if let Some(card) = card_in_hand(view, "RAV-CHAR")
            && view.own_life > 2
            && can_pay_char(view)
        {
            return cast(card, vec![damage_target(view)]);
        }
        if (has_brownscale || has_rally || has_helix || has_char)
            && let Some(land) = select_battlefield_land(
                view,
                desired_mana_color(view, has_brownscale, has_rally, has_helix, has_char),
            )
        {
            let desired = desired_mana_color(view, has_brownscale, has_rally, has_helix, has_char);
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

fn can_pay_char(view: &GameView) -> bool {
    view.mana_pool.amount(Color::Red) >= 1 && view.mana_pool.total() >= 3
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
    has_brownscale: bool,
    has_rally: bool,
    has_helix: bool,
    has_char: bool,
) -> Option<Color> {
    if has_brownscale && lands_producing(view, Color::Green) < 2 {
        Some(Color::Green)
    } else if has_rally && lands_producing(view, Color::White) == 0 {
        Some(Color::White)
    } else if (has_rally || has_helix || has_char) && lands_producing(view, Color::Red) == 0 {
        Some(Color::Red)
    } else if has_helix && lands_producing(view, Color::White) == 0 {
        Some(Color::White)
    } else {
        None
    }
}

#[allow(clippy::fn_params_excessive_bools)] // Spell-presence flags keep ordering explicit.
fn desired_mana_color(
    view: &GameView,
    has_brownscale: bool,
    has_rally: bool,
    has_helix: bool,
    has_char: bool,
) -> Option<Color> {
    if has_brownscale && view.mana_pool.amount(Color::Green) < 2 {
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
    } else if (has_helix || has_char) && view.mana_pool.amount(Color::Red) == 0 {
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
    fn submits_creature_then_radiance_and_preserves_invariants() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        let brownscale = game
            .add_card(PlayerId(0), "RAV-GOLGARI-BROWNSCALE", Zone::Hand)
            .expect("Brownscale in hand");
        let rally = game
            .add_card(PlayerId(0), "RAV-RALLY-THE-RIGHTEOUS", Zone::Hand)
            .expect("Rally in hand");
        game.grant_mana(PlayerId(0), Color::Green, 3)
            .expect("green development mana");
        let mut policy = BorosRadianceAssaultPolicy::new(PlayerId(0));

        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        assert_eq!(action, cast(brownscale, vec![]));
        game.submit_policy_move(PlayerId(0), policy.id(), action)
            .expect("Brownscale proposal is legal");
        resolve_top_of_stack(&mut game);
        game.set_tapped_for_setup(brownscale, true)
            .expect("tapped target setup");
        game.grant_mana(PlayerId(0), Color::White, 1)
            .expect("white mana");
        game.grant_mana(PlayerId(0), Color::Red, 2)
            .expect("red and generic mana");

        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        assert_eq!(action, cast(rally, vec![Target::Permanent(brownscale)]));
        game.submit_policy_move(PlayerId(0), policy.id(), action)
            .expect("Rally proposal is legal");
        resolve_top_of_stack(&mut game);
        assert!(!game.object(brownscale).expect("Brownscale exists").tapped);
        assert_eq!(
            game.characteristics(brownscale)
                .expect("characteristics")
                .power,
            Some(4)
        );
        game.validate_invariants()
            .expect("radiance policy submission preserves invariants");
    }

    #[test]
    fn submits_helix_at_an_opposing_creature_and_preserves_invariants() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        let helix = game
            .add_card(PlayerId(0), "RAV-LIGHTNING-HELIX", Zone::Hand)
            .expect("Helix in hand");
        let victim = game
            .put_on_battlefield(PlayerId(1), "RAV-GOLGARI-BROWNSCALE")
            .expect("opposing creature");
        game.grant_mana(PlayerId(0), Color::White, 1)
            .expect("white mana");
        game.grant_mana(PlayerId(0), Color::Red, 1)
            .expect("red mana");
        let mut policy = BorosRadianceAssaultPolicy::new(PlayerId(0));
        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        assert_eq!(action, cast(helix, vec![Target::Permanent(victim)]));
        game.submit_policy_move(PlayerId(0), policy.id(), action)
            .expect("Helix proposal is legal");
        resolve_top_of_stack(&mut game);
        game.validate_invariants()
            .expect("burn policy submission preserves invariants");
    }
}
