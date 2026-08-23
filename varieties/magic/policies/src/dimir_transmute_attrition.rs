use cardbench_magic_engine::{
    CardType, CastRequest, Color, DecisionSelection, GameView, ObjectId, PlayerId, PolicyAction,
    Step, Target,
};

use crate::CodePolicy;

/// A blue-black attrition fixture policy whose transmute search turns Muddle
/// into a selected red-white mana-value-two payoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DimirTransmuteAttritionPolicy {
    player: PlayerId,
}

impl DimirTransmuteAttritionPolicy {
    #[must_use]
    pub const fn new(player: PlayerId) -> Self {
        Self { player }
    }
}

impl CodePolicy for DimirTransmuteAttritionPolicy {
    fn id(&self) -> &'static str {
        "rav.dimir-transmute-attrition.v1"
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if let Some(decision) = &view.pending_decision {
            let selected = decision
                .candidates
                .iter()
                .find(|card| card.definition == Some("RAV-LIGHTNING-HELIX"))
                .map_or_else(Vec::new, |card| vec![card.id]);
            return PolicyAction::SubmitDecision {
                decision: decision.id,
                selection: DecisionSelection::Objects(selected),
            };
        }
        match view.step {
            Step::DeclareAttackers
                if view.active_player == self.player && !view.attackers_declared =>
            {
                return PolicyAction::DeclareAttackers { attackers: vec![] };
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
        if view.active_player != self.player
            || view.priority != self.player
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }

        let has_muddle = has_card(view, "RAV-MUDDLE-THE-MIXTURE");
        let has_gasp = has_card(view, "RAV-LAST-GASP");
        let has_helix = has_card(view, "RAV-LIGHTNING-HELIX");
        let has_char = has_card(view, "RAV-CHAR");
        if view.lands_played == 0
            && let Some(land) = select_hand_land(
                view,
                desired_land_color(view, has_muddle, has_gasp, has_helix, has_char),
            )
        {
            return PolicyAction::PlayLand { card: land };
        }
        if let Some(action) = transmute_for_helix(view) {
            return action;
        }
        if let Some(card) = card_in_hand(view, "RAV-LAST-GASP")
            && view.mana_pool.amount(Color::Black) >= 1
            && let Some(target) = opposing_creature(view)
        {
            return cast(card, Target::Permanent(target));
        }
        if let Some(card) = card_in_hand(view, "RAV-LIGHTNING-HELIX")
            && can_pay_helix(view)
        {
            return cast(card, damage_target(view));
        }
        if let Some(card) = card_in_hand(view, "RAV-CHAR")
            && view.own_life > 2
            && can_pay_char(view)
        {
            return cast(card, damage_target(view));
        }
        if (has_muddle || has_gasp || has_helix || has_char)
            && let Some(land) = select_battlefield_land(
                view,
                desired_mana_color(view, has_muddle, has_gasp, has_helix, has_char),
            )
        {
            let desired = desired_mana_color(view, has_muddle, has_gasp, has_helix, has_char);
            let color = desired
                .filter(|color| land.mana_colors.contains(color))
                .unwrap_or_else(|| *land.mana_colors.first().expect("basic land has color"));
            return PolicyAction::ActivateManaAbility {
                land: land.id,
                color,
            };
        }
        PolicyAction::PassPriority
    }
}

fn transmute_for_helix(view: &GameView) -> Option<PolicyAction> {
    if view.mana_pool.amount(Color::Blue) < 2 || view.mana_pool.total() < 3 {
        return None;
    }
    // The later resolving private search supplies candidates. Ordinary policy
    // state only establishes that the visible Muddle activation is affordable.
    card_in_hand(view, "RAV-MUDDLE-THE-MIXTURE").map(|card| PolicyAction::Transmute { card })
}

fn cast(card: ObjectId, target: Target) -> PolicyAction {
    PolicyAction::Cast(CastRequest {
        card,
        targets: vec![target],
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

fn can_pay_helix(view: &GameView) -> bool {
    view.mana_pool.amount(Color::White) >= 1
        && view.mana_pool.amount(Color::Red) >= 1
        && view.mana_pool.total() >= 2
}

fn can_pay_char(view: &GameView) -> bool {
    view.mana_pool.amount(Color::Red) >= 1 && view.mana_pool.total() >= 3
}

fn opposing_creature(view: &GameView) -> Option<ObjectId> {
    view.opponent_battlefield
        .iter()
        .find(|card| card.card_types.contains(&CardType::Creature))
        .map(|card| card.id)
}

fn damage_target(view: &GameView) -> Target {
    opposing_creature(view).map_or_else(
        || {
            Target::Player(
                view.opponent_life
                    .first()
                    .map_or(PlayerId(0), |(id, _)| *id),
            )
        },
        Target::Permanent,
    )
}

#[allow(clippy::fn_params_excessive_bools)] // Spell-presence flags keep ordering explicit.
fn desired_land_color(
    view: &GameView,
    has_muddle: bool,
    has_gasp: bool,
    has_helix: bool,
    has_char: bool,
) -> Option<Color> {
    if has_muddle && lands_producing(view, Color::Blue) < 2 {
        Some(Color::Blue)
    } else if has_gasp && lands_producing(view, Color::Black) == 0 {
        Some(Color::Black)
    } else if has_helix && lands_producing(view, Color::White) == 0 {
        Some(Color::White)
    } else if (has_helix || has_char) && lands_producing(view, Color::Red) == 0 {
        Some(Color::Red)
    } else {
        None
    }
}

#[allow(clippy::fn_params_excessive_bools)] // Spell-presence flags keep ordering explicit.
fn desired_mana_color(
    view: &GameView,
    has_muddle: bool,
    has_gasp: bool,
    has_helix: bool,
    has_char: bool,
) -> Option<Color> {
    if has_muddle && view.mana_pool.amount(Color::Blue) < 2 {
        Some(Color::Blue)
    } else if has_gasp && view.mana_pool.amount(Color::Black) == 0 {
        Some(Color::Black)
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
    fn submits_transmute_for_a_visible_legal_helix_candidate() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        let muddle = game
            .add_card(PlayerId(0), "RAV-MUDDLE-THE-MIXTURE", Zone::Hand)
            .expect("Muddle in hand");
        let _helix = game
            .add_card(PlayerId(0), "RAV-LIGHTNING-HELIX", Zone::Library)
            .expect("Helix in library");
        game.grant_mana(PlayerId(0), Color::Blue, 2)
            .expect("blue mana");
        game.grant_mana(PlayerId(0), Color::Black, 1)
            .expect("generic mana");
        let mut policy = DimirTransmuteAttritionPolicy::new(PlayerId(0));
        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        assert_eq!(action, PolicyAction::Transmute { card: muddle });
        game.submit_policy_move(PlayerId(0), policy.id(), action)
            .expect("transmute proposal is legal");
        game.validate_invariants()
            .expect("transmute submission preserves invariants");
    }

    #[test]
    fn submits_last_gasp_then_observes_state_based_cleanup() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        let gasp = game
            .add_card(PlayerId(0), "RAV-LAST-GASP", Zone::Hand)
            .expect("Gasp in hand");
        let victim = game
            .put_on_battlefield(PlayerId(1), "RAV-GOLGARI-BROWNSCALE")
            .expect("victim");
        game.grant_mana(PlayerId(0), Color::Black, 1)
            .expect("black mana");
        let mut policy = DimirTransmuteAttritionPolicy::new(PlayerId(0));
        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        assert_eq!(action, cast(gasp, Target::Permanent(victim)));
        game.submit_policy_move(PlayerId(0), policy.id(), action)
            .expect("Gasp proposal is legal");
        resolve_top_of_stack(&mut game);
        assert_eq!(game.zone_of(victim), Some(Zone::Graveyard));
        game.validate_invariants()
            .expect("targeted-removal submission preserves invariants");
    }
}
