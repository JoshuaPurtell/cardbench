use cardbench_magic_engine::{
    CardType, CastRequest, Color, GameView, ObjectId, PlayerId, PolicyAction, Step, Target,
};

use crate::CodePolicy;

/// A conservative Boros burn-control policy for the public Char development deck.
///
/// It develops red mana first, uses Char as the primary four-damage interaction,
/// preserves itself at two life or less, and reserves Lightning Helix for a
/// resource-efficient lethal line or follow-up interaction. The fixture has no
/// creatures, but the policy still submits the required empty combat declarations
/// so whole-deck probes exercise the turn-based combat transitions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BorosCharControlPolicy {
    player: PlayerId,
}

impl BorosCharControlPolicy {
    #[must_use]
    pub const fn new(player: PlayerId) -> Self {
        Self { player }
    }
}

impl CodePolicy for BorosCharControlPolicy {
    fn id(&self) -> &'static str {
        "rav.boros-char-control.v1"
    }

    #[allow(clippy::too_many_lines)] // The explicit choices are probe-auditable policy code.
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
                    assignments: vec![],
                };
            }
            Step::PrecombatMain | Step::PostcombatMain => {}
            _ => return PolicyAction::PassPriority,
        }
        if view.priority != self.player || view.stack_depth != 0 {
            return PolicyAction::PassPriority;
        }

        let opponent = view
            .opponent_life
            .first()
            .map_or(PlayerId(0), |(player, _)| *player);
        let opponent_life = view
            .opponent_life
            .first()
            .map_or(i64::MAX, |(_, life)| *life);
        let char = view
            .hand
            .iter()
            .find(|card| card.definition == Some("RAV-CHAR"));
        let helix = view
            .hand
            .iter()
            .find(|card| card.definition == Some("RAV-LIGHTNING-HELIX"));

        if view.active_player == self.player && view.lands_played == 0 {
            if let Some(land) = select_hand_land(
                view,
                desired_land_color(view, char.is_some(), helix.is_some()),
            ) {
                return PolicyAction::PlayLand { card: land };
            }
        }

        // A two-mana Helix is preferred when it wins immediately; otherwise Char
        // is our primary interaction if its self-damage does not lose the game.
        if let Some(card) = helix
            && opponent_life <= 3
            && can_cast_helix(view)
        {
            return cast_at_priority_target(card.id, Target::Player(opponent));
        }
        if let Some(card) = char
            && view.own_life > 2
            && can_cast_char(view)
        {
            return cast_at_priority_target(card.id, damage_target(view, opponent));
        }
        if let Some(card) = helix
            && can_cast_helix(view)
        {
            return cast_at_priority_target(card.id, damage_target(view, opponent));
        }

        if let Some(land) = select_battlefield_land(
            view,
            desired_land_color(view, char.is_some(), helix.is_some()),
        ) {
            let color = desired_land_color(view, char.is_some(), helix.is_some())
                .filter(|color| land.mana_colors.contains(color))
                .unwrap_or_else(|| *land.mana_colors.first().expect("basic land has mana color"));
            return PolicyAction::ActivateManaAbility {
                land: land.id,
                color,
            };
        }
        PolicyAction::PassPriority
    }
}

fn cast_at_priority_target(card: ObjectId, target: Target) -> PolicyAction {
    PolicyAction::Cast(CastRequest {
        card,
        targets: vec![target],
        convoke: vec![],
    })
}

fn can_cast_char(view: &GameView) -> bool {
    view.mana_pool.amount(Color::Red) >= 1 && view.mana_pool.total() >= 3
}

fn can_cast_helix(view: &GameView) -> bool {
    view.mana_pool.amount(Color::White) >= 1
        && view.mana_pool.amount(Color::Red) >= 1
        && view.mana_pool.total() >= 2
}

fn desired_land_color(view: &GameView, has_char: bool, has_helix: bool) -> Option<Color> {
    if has_char && view.own_life > 2 && view.mana_pool.amount(Color::Red) == 0 {
        return Some(Color::Red);
    }
    if has_helix && view.mana_pool.amount(Color::White) == 0 {
        return Some(Color::White);
    }
    if has_helix && view.mana_pool.amount(Color::Red) == 0 {
        return Some(Color::Red);
    }
    None
}

fn damage_target(view: &GameView, opponent: PlayerId) -> Target {
    view.opponent_battlefield
        .iter()
        .find(|card| card.card_types.contains(&CardType::Creature))
        .map_or(Target::Player(opponent), |card| Target::Permanent(card.id))
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
    if let Some(color) = desired {
        return lands.find(|card| card.mana_colors.contains(&color));
    }
    lands.next()
}

#[cfg(test)]
mod tests {
    use cardbench_magic_engine::{Game, Zone};
    use cardbench_magic_rav::card_definitions;

    use super::*;

    #[test]
    fn char_is_preferred_when_its_self_damage_is_safe() {
        let mut game = Game::new(card_definitions(), 2).expect("game");
        let char = game
            .add_card(PlayerId(0), "RAV-CHAR", Zone::Hand)
            .expect("Char in hand");
        game.add_card(PlayerId(0), "RAV-LIGHTNING-HELIX", Zone::Hand)
            .expect("Helix in hand");
        game.grant_mana(PlayerId(0), Color::White, 1)
            .expect("white mana");
        game.grant_mana(PlayerId(0), Color::Red, 2)
            .expect("red mana");

        let mut policy = BorosCharControlPolicy::new(PlayerId(0));
        assert_eq!(
            policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view")),
            cast_at_priority_target(char, Target::Player(PlayerId(1)))
        );
    }

    #[test]
    fn char_is_held_at_two_life_and_helix_remains_available() {
        let mut game = Game::new(card_definitions(), 2).expect("game");
        game.players[0].life = 2;
        game.add_card(PlayerId(0), "RAV-CHAR", Zone::Hand)
            .expect("Char in hand");
        let helix = game
            .add_card(PlayerId(0), "RAV-LIGHTNING-HELIX", Zone::Hand)
            .expect("Helix in hand");
        game.grant_mana(PlayerId(0), Color::White, 1)
            .expect("white mana");
        game.grant_mana(PlayerId(0), Color::Red, 2)
            .expect("red mana");

        let mut policy = BorosCharControlPolicy::new(PlayerId(0));
        assert_eq!(
            policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view")),
            cast_at_priority_target(helix, Target::Player(PlayerId(1)))
        );
    }

    #[test]
    fn policy_develops_red_mana_and_submits_empty_combat_actions() {
        let mut game = Game::new(card_definitions(), 2).expect("game");
        let mountain = game
            .add_card(PlayerId(0), "RAV-MOUNTAIN", Zone::Hand)
            .expect("Mountain in hand");
        game.add_card(PlayerId(0), "RAV-CHAR", Zone::Hand)
            .expect("Char in hand");
        let mut policy = BorosCharControlPolicy::new(PlayerId(0));

        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        assert_eq!(action, PolicyAction::PlayLand { card: mountain });
        game.submit_policy_move(PlayerId(0), policy.id(), action)
            .expect("legal land play");
        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        assert_eq!(
            action,
            PolicyAction::ActivateManaAbility {
                land: mountain,
                color: Color::Red,
            }
        );

        for _ in 0..2 {
            let player = game.priority;
            game.pass_priority(player).expect("pass to next step");
            let player = game.priority;
            game.pass_priority(player).expect("pass to next step");
        }
        assert_eq!(game.step, Step::DeclareAttackers);
        let attackers = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        assert_eq!(
            attackers,
            PolicyAction::DeclareAttackers { attackers: vec![] }
        );
        game.submit_policy_move(PlayerId(0), policy.id(), attackers)
            .expect("empty attack declaration");
        let mut defending_policy = BorosCharControlPolicy::new(PlayerId(1));
        let pass = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        assert_eq!(pass, PolicyAction::PassPriority);
        game.submit_policy_move(PlayerId(0), policy.id(), pass)
            .expect("post-attack priority pass");
        let pass = defending_policy.propose_move(&game.view_for_player(PlayerId(1)).expect("view"));
        assert_eq!(pass, PolicyAction::PassPriority);
        game.submit_policy_move(PlayerId(1), defending_policy.id(), pass)
            .expect("defender priority pass");
        assert_eq!(game.step, Step::EndOfCombat);
        assert!(
            !game
                .canonical_event_log()
                .iter()
                .any(|event| event.contains("BlockersDeclared")),
            "CR 508.8 skips blockers when no attackers were declared"
        );
    }
}
