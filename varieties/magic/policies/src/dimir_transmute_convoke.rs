use cardbench_magic_engine::{
    CardType, CastRequest, Color, ConvokeContribution, ConvokePayment, DecisionSelection, GameView,
    ObjectId, PlayerId, PolicyAction, Step, Target,
};

use crate::CodePolicy;

/// A blue-led transmute policy with an explicit green convoke development
/// package and Helix as the searched mana-value-two payoff.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DimirTransmuteConvokePolicy {
    player: PlayerId,
}

impl DimirTransmuteConvokePolicy {
    #[must_use]
    pub const fn new(player: PlayerId) -> Self {
        Self { player }
    }
}

impl CodePolicy for DimirTransmuteConvokePolicy {
    fn id(&self) -> &'static str {
        "rav.dimir-transmute-convoke.v1"
    }

    #[allow(clippy::too_many_lines)] // Explicit ordering is useful corpus provenance.
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
        if view.active_player != self.player
            || view.priority != self.player
            || view.stack_depth != 0
        {
            return PolicyAction::PassPriority;
        }

        let has_muddle = has_card(view, "RAV-MUDDLE-THE-MIXTURE");
        let has_scatter = has_card(view, "RAV-SCATTER-THE-SEEDS");
        let has_brownscale = has_card(view, "RAV-GOLGARI-BROWNSCALE");
        let has_helix = has_card(view, "RAV-LIGHTNING-HELIX");
        if view.lands_played == 0
            && let Some(land) = select_hand_land(
                view,
                desired_land_color(view, has_muddle, has_scatter, has_brownscale, has_helix),
            )
        {
            return PolicyAction::PlayLand { card: land };
        }
        if let Some(action) = transmute_for_helix(view) {
            return action;
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
        if let Some(card) = card_in_hand(view, "RAV-GOLGARI-BROWNSCALE")
            && view.mana_pool.amount(Color::Green) >= 2
            && view.mana_pool.total() >= 3
        {
            return cast(card, vec![]);
        }
        if let Some(card) = card_in_hand(view, "RAV-LIGHTNING-HELIX")
            && view.mana_pool.amount(Color::White) >= 1
            && view.mana_pool.amount(Color::Red) >= 1
            && view.mana_pool.total() >= 2
        {
            return cast(card, vec![damage_target(view)]);
        }
        if (has_muddle || has_scatter || has_brownscale || has_helix)
            && let Some(land) = select_battlefield_land(
                view,
                desired_mana_color(view, has_muddle, has_scatter, has_brownscale, has_helix),
            )
        {
            let desired =
                desired_mana_color(view, has_muddle, has_scatter, has_brownscale, has_helix);
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
    let search = view.transmute_searches.iter().find(|search| {
        view.hand
            .iter()
            .any(|card| card.id == search.card && card.definition == Some("RAV-MUDDLE-THE-MIXTURE"))
    })?;
    let _found = search
        .candidates
        .iter()
        .find(|card| card.definition == Some("RAV-LIGHTNING-HELIX"))?;
    Some(PolicyAction::Transmute { card: search.card })
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
    has_muddle: bool,
    has_scatter: bool,
    has_brownscale: bool,
    has_helix: bool,
) -> Option<Color> {
    if has_muddle && lands_producing(view, Color::Blue) < 2 {
        Some(Color::Blue)
    } else if (has_scatter || has_brownscale) && lands_producing(view, Color::Green) < 2 {
        Some(Color::Green)
    } else if has_helix && lands_producing(view, Color::White) == 0 {
        Some(Color::White)
    } else if has_helix && lands_producing(view, Color::Red) == 0 {
        Some(Color::Red)
    } else {
        None
    }
}

#[allow(clippy::fn_params_excessive_bools)] // Spell-presence flags keep ordering explicit.
fn desired_mana_color(
    view: &GameView,
    has_muddle: bool,
    has_scatter: bool,
    has_brownscale: bool,
    has_helix: bool,
) -> Option<Color> {
    if has_muddle && view.mana_pool.amount(Color::Blue) < 2 {
        Some(Color::Blue)
    } else if (has_scatter || has_brownscale) && view.mana_pool.amount(Color::Green) < 2 {
        Some(Color::Green)
    } else if has_helix && view.mana_pool.amount(Color::White) == 0 {
        Some(Color::White)
    } else if has_helix && view.mana_pool.amount(Color::Red) == 0 {
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

    #[test]
    fn submits_transmute_using_a_controller_visible_legal_candidate() {
        let mut game = Game::new(card_definitions(), 2).expect("RAV game");
        let muddle = game
            .add_card(PlayerId(0), "RAV-MUDDLE-THE-MIXTURE", Zone::Hand)
            .expect("Muddle in hand");
        let _helix = game
            .add_card(PlayerId(0), "RAV-LIGHTNING-HELIX", Zone::Library)
            .expect("Helix in library");
        game.grant_mana(PlayerId(0), Color::Blue, 2)
            .expect("blue mana");
        game.grant_mana(PlayerId(0), Color::Green, 1)
            .expect("generic mana");
        let mut policy = DimirTransmuteConvokePolicy::new(PlayerId(0));
        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        assert_eq!(action, PolicyAction::Transmute { card: muddle });
        game.submit_policy_move(PlayerId(0), policy.id(), action)
            .expect("transmute proposal is legal");
        game.validate_invariants()
            .expect("transmute submission preserves invariants");
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
        let mut policy = DimirTransmuteConvokePolicy::new(PlayerId(0));
        let action = policy.propose_move(&game.view_for_player(PlayerId(0)).expect("view"));
        let PolicyAction::Cast(request) = action else {
            panic!("policy must convoke Scatter: {action:?}");
        };
        assert_eq!(request.card, scatter);
        assert_eq!(request.convoke.len(), 5);
        game.submit_policy_move(PlayerId(0), policy.id(), PolicyAction::Cast(request))
            .expect("fully convoked Scatter proposal is legal");
        assert!(
            creatures
                .iter()
                .all(|card| game.object(*card).is_ok_and(|object| object.tapped))
        );
        game.validate_invariants()
            .expect("convoke submission preserves invariants");
    }
}
