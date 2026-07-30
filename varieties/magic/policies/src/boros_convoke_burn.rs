use cardbench_magic_engine::{
    CardType, CastRequest, Color, CombatBlock, ConvokeContribution, ConvokePayment, GameView,
    ObjectId, PlayerId, PolicyAction, Step, Target,
};

use crate::CodePolicy;

/// A broad RAV development policy combining Boros damage with the implemented
/// green convoke creature package.
///
/// The sequencing is intentionally easy to audit: turn a green board into a
/// Wurm or token burst, then use white-red burn to force a terminal game. Its
/// explicit full-convoke submissions exercise payment validation beyond normal
/// mana-only spell casts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BorosConvokeBurnPolicy {
    player: PlayerId,
}

impl BorosConvokeBurnPolicy {
    #[must_use]
    pub const fn new(player: PlayerId) -> Self {
        Self { player }
    }
}

impl CodePolicy for BorosConvokeBurnPolicy {
    fn id(&self) -> &'static str {
        "rav.boros-convoke-burn.v1"
    }

    #[allow(clippy::too_many_lines)] // Linear decision order is intentional probe provenance.
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

        let has_wurm = has_card(view, "RAV-SIEGE-WURM");
        let has_scatter = has_card(view, "RAV-SCATTER-THE-SEEDS");
        let has_brownscale = has_card(view, "RAV-GOLGARI-BROWNSCALE");
        let has_helix = has_card(view, "RAV-LIGHTNING-HELIX");
        let has_char = has_card(view, "RAV-CHAR");
        if view.lands_played == 0
            && let Some(land) = select_hand_land(
                view,
                desired_land_color(
                    view,
                    has_wurm,
                    has_scatter,
                    has_brownscale,
                    has_helix,
                    has_char,
                ),
            )
        {
            return PolicyAction::PlayLand { card: land };
        }

        if let Some(card) = card_in_hand(view, "RAV-SIEGE-WURM")
            && let Some(convoke) = legal_green_convoke(view, 2, 5)
        {
            return cast_with_convoke(card, convoke);
        }
        if let Some(card) = card_in_hand(view, "RAV-SCATTER-THE-SEEDS")
            && let Some(convoke) = legal_green_convoke(view, 2, 3)
        {
            return cast_with_convoke(card, convoke);
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
        if let Some(card) = card_in_hand(view, "RAV-CHAR")
            && view.own_life > 2
            && can_pay_char(view)
        {
            return cast(card, vec![damage_target(view)]);
        }
        if (has_wurm || has_scatter || has_brownscale || has_helix || has_char)
            && let Some(land) = select_battlefield_land(
                view,
                desired_mana_color(
                    view,
                    has_wurm,
                    has_scatter,
                    has_brownscale,
                    has_helix,
                    has_char,
                ),
            )
        {
            let desired = desired_mana_color(
                view,
                has_wurm,
                has_scatter,
                has_brownscale,
                has_helix,
                has_char,
            );
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

fn cast_with_convoke(card: ObjectId, convoke: Vec<ConvokePayment>) -> PolicyAction {
    PolicyAction::Cast(CastRequest {
        card,
        targets: vec![],
        convoke,
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

fn can_pay_helix(view: &GameView) -> bool {
    view.mana_pool.amount(Color::White) >= 1
        && view.mana_pool.amount(Color::Red) >= 1
        && view.mana_pool.total() >= 2
}

fn can_pay_char(view: &GameView) -> bool {
    view.mana_pool.amount(Color::Red) >= 1 && view.mana_pool.total() >= 3
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
    has_wurm: bool,
    has_scatter: bool,
    has_brownscale: bool,
    has_helix: bool,
    has_char: bool,
) -> Option<Color> {
    if (has_wurm || has_scatter || has_brownscale) && lands_producing(view, Color::Green) < 2 {
        Some(Color::Green)
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
    has_wurm: bool,
    has_scatter: bool,
    has_brownscale: bool,
    has_helix: bool,
    has_char: bool,
) -> Option<Color> {
    if (has_wurm || has_scatter || has_brownscale) && view.mana_pool.amount(Color::Green) < 2 {
        Some(Color::Green)
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
    fn submits_a_fully_convoked_wurm_and_preserves_invariants() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        let wurm = game
            .add_card(PlayerId(0), "RAV-SIEGE-WURM", Zone::Hand)
            .expect("Wurm in hand");
        let creatures: Vec<_> = (0..7)
            .map(|_| {
                game.put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE")
                    .expect("green convoke creature")
            })
            .collect();
        let mut policy = BorosConvokeBurnPolicy::new(PlayerId(0));
        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        let PolicyAction::Cast(request) = action else {
            panic!("policy must propose Wurm through convoke: {action:?}");
        };
        assert_eq!(request.card, wurm);
        assert_eq!(request.convoke.len(), 7);
        game.submit_policy_move(PlayerId(0), policy.id(), PolicyAction::Cast(request))
            .expect("fully convoked Wurm proposal is legal");
        assert!(
            creatures
                .iter()
                .all(|card| game.object(*card).is_ok_and(|object| object.tapped))
        );
        game.validate_invariants()
            .expect("convoke policy submission preserves invariants");
    }

    #[test]
    fn submits_char_at_the_opponent_and_preserves_invariants() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        let char = game
            .add_card(PlayerId(0), "RAV-CHAR", Zone::Hand)
            .expect("Char in hand");
        game.grant_mana(PlayerId(0), Color::Red, 3)
            .expect("Char mana");
        let mut policy = BorosConvokeBurnPolicy::new(PlayerId(0));
        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        assert_eq!(action, cast(char, vec![Target::Player(PlayerId(1))]));
        game.submit_policy_move(PlayerId(0), policy.id(), action)
            .expect("Char proposal is legal");
        resolve_top_of_stack(&mut game);
        assert_eq!(game.players[0].life, 18);
        assert_eq!(game.players[1].life, 16);
        game.validate_invariants()
            .expect("burn policy submission preserves invariants");
    }
}
