//! Broad deterministic policies for the all-card RAV catalog gauntlet.

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, CombatBlock, DecisionKind, DecisionSelection,
    GameView, ManaPaymentSelection, ObjectId, PlayerId, PolicyAction, Step,
};
use cardbench_magic_rav::card_definitions;

use crate::CodePolicy;

/// Six stable strategy profiles rotate across the generated catalog bands.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogPolicyProfile {
    Pressure,
    Curve,
    Control,
    Graveyard,
    TopEnd,
    Patient,
}

impl CatalogPolicyProfile {
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Pressure => "rav.catalog-pressure.v1",
            Self::Curve => "rav.catalog-curve.v1",
            Self::Control => "rav.catalog-control.v1",
            Self::Graveyard => "rav.catalog-graveyard.v1",
            Self::TopEnd => "rav.catalog-top-end.v1",
            Self::Patient => "rav.catalog-patient.v1",
        }
    }
}

/// A public-information-only policy that develops mana, casts safe target-free
/// permanents through the normal stack, makes deterministic decisions, and
/// completes combat. Different profiles change spell ordering, combat posture,
/// and draw-replacement choices while retaining the same fail-closed ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RavCatalogPolicy {
    player: PlayerId,
    profile: CatalogPolicyProfile,
}

impl RavCatalogPolicy {
    #[must_use]
    pub const fn new(player: PlayerId, profile: CatalogPolicyProfile) -> Self {
        Self { player, profile }
    }
}

impl CodePolicy for RavCatalogPolicy {
    fn id(&self) -> &'static str {
        self.profile.id()
    }

    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        conservative_pending_decision(view)
    }

    fn propose_draw_replacement(&mut self, view: &GameView) -> PolicyAction {
        let dredge = (self.profile == CatalogPolicyProfile::Graveyard)
            .then(|| view.dredge_candidates.first().map(|card| card.id))
            .flatten();
        PolicyAction::Draw {
            decision: view
                .draw_replacement_decision
                .expect("draw replacement has an identity"),
            dredge,
        }
    }

    #[allow(clippy::too_many_lines)] // Explicit priority, mana, spell, and combat ordering is the policy contract.
    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        if let Some(choice) = &view.optional_triggered_ability_choice {
            return PolicyAction::ResolveOptionalTriggeredAbility {
                decision: choice.decision,
                source: choice.source,
                ability: choice.ability,
                pay: false,
                target: None,
            };
        }
        match view.step {
            Step::DeclareAttackers
                if view.active_player == self.player && !view.attackers_declared =>
            {
                let attackers = if self.profile == CatalogPolicyProfile::Patient {
                    vec![]
                } else {
                    view.own_battlefield
                        .iter()
                        .filter(|card| card.can_attack)
                        .map(|card| card.id)
                        .collect()
                };
                return PolicyAction::DeclareAttackers { attackers };
            }
            Step::DeclareBlockers
                if view.active_player != self.player && !view.blockers_declared =>
            {
                return PolicyAction::DeclareBlockers {
                    assignments: conservative_blocks(view),
                };
            }
            Step::PrecombatMain | Step::PostcombatMain => {}
            _ => return PolicyAction::PassPriority,
        }
        if view.priority != self.player
            || view.stack_depth != 0
            || view.active_player != self.player
        {
            return PolicyAction::PassPriority;
        }

        if view.lands_played == 0
            && let Some(land) = land_to_play(view)
        {
            return if is_shock_land(land.definition) {
                PolicyAction::PlayLandWithEntryLifePayment {
                    card: land.id,
                    pay_life: false,
                }
            } else {
                PolicyAction::PlayLand { card: land.id }
            };
        }

        let definitions = card_definitions();
        if let Some(card) = spell_to_cast(view, &definitions, self.profile) {
            return PolicyAction::Cast(CastRequest {
                card,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            });
        }
        if let Some((land, color)) = basic_mana_to_activate(view) {
            return PolicyAction::ActivateManaAbility { land, color };
        }
        PolicyAction::PassPriority
    }
}

/// Deterministic fail-closed completion for every generic decision currently
/// exposed by the public engine view. All policies inherit this vocabulary so
/// an unfamiliar card can test the engine instead of stalling the campaign.
pub(crate) fn conservative_pending_decision(view: &GameView) -> Option<PolicyAction> {
    let decision = view.pending_decision.as_ref()?;
    let object_ids = || {
        decision
            .candidates
            .iter()
            .map(|card| card.id)
            .collect::<Vec<_>>()
    };
    let minimum_objects = || {
        object_ids()
            .into_iter()
            .take(usize::from(decision.min_selections))
            .collect::<Vec<_>>()
    };
    let selection = match decision.kind {
        DecisionKind::CommanderReturn | DecisionKind::CommanderZoneReplacement => DecisionSelection::Objects(object_ids()),
        DecisionKind::WarpWorldEntry
        | DecisionKind::LibrarySearch
        | DecisionKind::TriggeredEffectObject
        | DecisionKind::ConditionalPrivateDiscard
        | DecisionKind::TargetPlayerSacrificeCreatureThenControllerDrawsEqualToPower
        | DecisionKind::TargetPlayerLibraryTopMayGraveyard
        | DecisionKind::PublicGraveyardCreatureReturn
        | DecisionKind::PublicGraveyardLandReturn
        | DecisionKind::SourceLinkedExileCreatureReturn
        | DecisionKind::PreserveControlledPermanents
        | DecisionKind::RetargetActivatedAbility
        | DecisionKind::PermanentEntryCopySource
        | DecisionKind::PermanentEntryCopyAuraAttachment
        | DecisionKind::LegendRule
        | DecisionKind::CleanupDiscard => DecisionSelection::Objects(minimum_objects()),
        DecisionKind::WarpWorldBottom
        | DecisionKind::LibraryReorder
        | DecisionKind::HandToLibraryBottomDraw
        | DecisionKind::CombatDamageOrder => DecisionSelection::Objects(object_ids()),
        DecisionKind::LibrarySearchAndCast => DecisionSelection::LibrarySearchAndCast {
            selected: None,
            targets: vec![],
        },
        DecisionKind::LibraryTopPartition => {
            let mut cards = object_ids();
            let hand = *cards.first()?;
            cards.remove(0);
            let top = (!cards.is_empty()).then(|| cards.remove(0));
            cards.reverse();
            DecisionSelection::LibraryTopPartition {
                hand,
                top,
                bottom: cards,
            }
        }
        DecisionKind::TargetPlayerLibraryTopReorder => {
            DecisionSelection::TargetPlayerLibraryTopReorder {
                top: object_ids(),
                bottom: vec![],
            }
        }
        DecisionKind::SpellCopyTargets => DecisionSelection::Targets(vec![]),
        DecisionKind::ExiledSpellCopyCast => DecisionSelection::ExiledSpellCopyCast {
            card: None,
            targets: vec![],
            mode: None,
            color: None,
        },
        DecisionKind::TriggeredAbilityTargets => DecisionSelection::Targets(
            decision
                .target_candidates
                .iter()
                .copied()
                .take(usize::from(decision.min_selections))
                .collect(),
        ),
        DecisionKind::TriggeredAbilityOrder => {
            DecisionSelection::TriggerOrder(decision.trigger_candidates.clone())
        }
        DecisionKind::Replacement => DecisionSelection::Replacements(
            decision
                .replacement_candidates
                .first()
                .copied()
                .into_iter()
                .collect(),
        ),
        DecisionKind::CounterUnlessPaysMana => DecisionSelection::CounterUnlessPaysMana {
            pay: false,
            mana_abilities: vec![],
            mana_selection: ManaPaymentSelection::default(),
        },
        DecisionKind::CounterUnlessDiscardsHand => {
            DecisionSelection::CounterUnlessDiscardsHand { discard: false }
        }
        DecisionKind::TargetPlayerManaColor => {
            DecisionSelection::Color(*decision.color_candidates.first()?)
        }
        DecisionKind::NamedCardTargetLibraryTraversal => {
            DecisionSelection::CardName(decision.card_name_candidates.first()?)
        }
    };
    Some(PolicyAction::SubmitDecision {
        decision: decision.id,
        selection,
    })
}

fn land_to_play(view: &GameView) -> Option<&cardbench_magic_engine::CardView> {
    view.hand
        .iter()
        .find(|card| card.basic_land_type.is_some())
        .or_else(|| {
            view.hand
                .iter()
                .find(|card| card.card_types.contains(&CardType::Land))
        })
}

fn is_shock_land(definition: Option<&str>) -> bool {
    matches!(
        definition,
        Some(
            "RAV-OVERGROWN-TOMB" | "RAV-SACRED-FOUNDRY" | "RAV-TEMPLE-GARDEN" | "RAV-WATERY-GRAVE"
        )
    )
}

fn spell_to_cast(
    view: &GameView,
    definitions: &[CardDefinition],
    profile: CatalogPolicyProfile,
) -> Option<ObjectId> {
    let mut candidates = view
        .hand
        .iter()
        .filter_map(|card| {
            let definition = definitions
                .iter()
                .find(|definition| Some(definition.id) == card.definition)?;
            safe_target_free_permanent(definition).then_some((
                card.id,
                definition.mana_cost.mana_value(),
                definition,
            ))
        })
        .filter(|(_, _, definition)| can_pay(view, &definition.mana_cost))
        .collect::<Vec<_>>();
    candidates.sort_by_key(|(card, mana_value, _)| (*mana_value, *card));
    if matches!(
        profile,
        CatalogPolicyProfile::TopEnd | CatalogPolicyProfile::Control
    ) {
        candidates.last().map(|(card, _, _)| *card)
    } else {
        candidates.first().map(|(card, _, _)| *card)
    }
}

fn safe_target_free_permanent(definition: &CardDefinition) -> bool {
    definition.mana_cost.hybrid.is_empty()
        && definition.card_types.iter().any(|kind| {
            matches!(
                kind,
                CardType::Artifact | CardType::Creature | CardType::Enchantment
            )
        })
        && definition.effects.iter().all(|effect| {
            effect
                .target_requirements()
                .into_iter()
                .flatten()
                .next()
                .is_none()
        })
}

fn can_pay(view: &GameView, cost: &cardbench_magic_engine::ManaCost) -> bool {
    let mut fixed = [0_u8; 6];
    for color in &cost.colored {
        fixed[color.index()] = fixed[color.index()].saturating_add(1);
    }
    if Color::MANA_ALL
        .into_iter()
        .any(|color| view.mana_pool.amount(color) < fixed[color.index()])
    {
        return false;
    }
    let Ok(fixed_total) = u16::try_from(cost.colored.len()) else {
        return false;
    };
    view.mana_pool.total_exact().saturating_sub(fixed_total) >= u16::from(cost.generic)
}

fn basic_mana_to_activate(view: &GameView) -> Option<(ObjectId, Color)> {
    view.own_battlefield
        .iter()
        .find(|card| !card.tapped && card.basic_land_type.is_some())
        .and_then(|card| {
            card.mana_colors
                .first()
                .copied()
                .map(|color| (card.id, color))
        })
}

fn conservative_blocks(view: &GameView) -> Vec<CombatBlock> {
    let blockers = view
        .own_battlefield
        .iter()
        .filter(|card| !card.tapped && card.card_types.contains(&CardType::Creature));
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
    use super::*;

    #[test]
    fn profile_ids_are_distinct_and_versioned() {
        let ids = [
            CatalogPolicyProfile::Pressure,
            CatalogPolicyProfile::Curve,
            CatalogPolicyProfile::Control,
            CatalogPolicyProfile::Graveyard,
            CatalogPolicyProfile::TopEnd,
            CatalogPolicyProfile::Patient,
        ]
        .map(CatalogPolicyProfile::id);
        assert_eq!(
            ids.into_iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            6
        );
        assert!(ids.iter().all(|id| id.split('.').next_back() == Some("v1")));
    }
}
