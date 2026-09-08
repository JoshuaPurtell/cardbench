//! Optional commander replacements at a suspended stack instruction boundary.
//! Choices are exact-incarnation, single-use permissions, never global policy
//! preferences and never a post-hoc move out of the requested destination.
use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PendingActivation {
    pub player: PlayerId,
    source_incarnation: u64,
    pub request: GeneralizedAbilityActivation,
    pub choices: BTreeMap<ObjectId, (u64, Zone, Zone, Option<bool>)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum SelectedReturns {
    Creatures(Vec<GraveyardCreatureCardSnapshot>),
    Lands(Vec<GraveyardLandCardSnapshot>),
    TriggeredReturn(ObjectId),
    Search {
        selected: Option<ObjectId>,
        destination: LibrarySearchDestination,
        reveal_selected: bool,
    },
    SearchMany {
        selected: Vec<ObjectId>,
        destination: LibrarySearchDestination,
        reveal_selected: bool,
    },
    Partition {
        cards: Vec<ObjectId>,
        hand: ObjectId,
        top: Option<ObjectId>,
        bottom: Vec<ObjectId>,
    },
    Recycle {
        selected: Vec<ObjectId>,
        remaining_draws: Option<usize>,
    },
    PaidLook {
        cards: Vec<ObjectId>,
        selected: Vec<ObjectId>,
    },
    TerminalDraw {
        player: PlayerId,
        count: usize,
    },
}

impl Game {
    pub(super) fn finish_commander_terminal_draw(
        &mut self,
        top: StackObject,
        player: PlayerId,
        count: usize,
    ) -> Result<(), RulesError> {
        self.format
            .commander_selected_returns
            .insert(top.id, SelectedReturns::TerminalDraw { player, count });
        let current = self
            .stack
            .pop()
            .ok_or(RulesError::IllegalAction("terminal draw lost its stack"))?;
        if current.id != top.id {
            return Err(RulesError::IllegalAction(
                "terminal draw changed stack identity",
            ));
        }
        if self.resolve_commander_draw_sequence(&top, 0, player, count, None, None, None)? {
            return Ok(());
        }
        self.format.commander_selected_returns.remove(&top.id);
        self.stack_effect_cursors.remove(&top.id);
        self.stack_target_legality_snapshots.remove(&top.id);
        self.finish_resolved_decision_stack_item(&top)
    }
    pub(super) fn validate_commander_dredge_replacement(
        &self,
        decision: &PendingDecision,
    ) -> Result<(), RulesError> {
        let DecisionContinuation::CommanderDredgeReplacement {
            player,
            commander,
            incarnation,
            amount,
        } = decision.continuation
        else {
            return Err(RulesError::IllegalAction(
                "wrong commander dredge continuation",
            ));
        };
        if self.step != Step::Draw
            || !self.is_active_seat(player)
            || !self.stack.is_empty()
            || decision.player != player
            || decision.kind != DecisionKind::CommanderZoneReplacement
            || decision.visibility != DecisionVisibility::Public
            || decision.min_selections != 0
            || decision.max_selections != 1
            || decision.options != vec![DecisionOption::Object(commander)]
            || self.zone_of(commander) != Some(Zone::Graveyard)
            || self.card_definition(commander)?.dredge() != Some(amount)
            || self
                .format
                .commanders
                .get(&commander)
                .map(|record| record.owner)
                != Some(player)
            || !self.object_has_incarnation(commander, incarnation)
        {
            return Err(RulesError::IllegalAction(
                "stale commander dredge replacement",
            ));
        }
        Ok(())
    }
    pub(super) fn commander_cost_move_receipts(&self, start: usize, card: ObjectId) -> bool {
        matches!(self.event_log.get(start), Some(GameEvent::CardMoved { card: moved, to: Zone::Command }) if *moved == card)
            && matches!(self.event_log.get(start + 1), Some(GameEvent::ObjectIncarnationAdvanced { object, .. }) if *object == card)
            && matches!(self.event_log.get(start + 2), Some(GameEvent::CommanderReturnedToCommandZone { commander, player })
                if *commander == card && self.object(card).is_ok_and(|object| object.owner == *player))
    }
    pub(super) fn suspend_commander_activation(
        &mut self,
        player: PlayerId,
        source_incarnation: u64,
        request: GeneralizedAbilityActivation,
    ) -> Result<bool, RulesError> {
        if let Some(pending) = &self.format.commander_activation {
            if pending.player != player
                || pending.source_incarnation != source_incarnation
                || pending.request != request
            {
                return Err(RulesError::IllegalAction(
                    "commander cost choice belongs to another activation",
                ));
            }
        } else {
            let mut choices = BTreeMap::new();
            for (card, from, to) in request
                .cost_payment
                .return_permanents
                .iter()
                .map(|card| (*card, Zone::Battlefield, Zone::Hand))
                .chain(
                    request
                        .cost_payment
                        .hand_cards_to_library_top
                        .iter()
                        .map(|card| (*card, Zone::Hand, Zone::Library)),
                )
            {
                if self.is_commander(card) {
                    choices.insert(card, (self.object(card)?.incarnation, from, to, None));
                }
            }
            if choices.is_empty() {
                return Ok(false);
            }
            self.format.commander_activation = Some(PendingActivation {
                player,
                source_incarnation,
                request,
                choices,
            });
        }
        let pending = self
            .format
            .commander_activation
            .as_ref()
            .expect("installed activation");
        let next = pending
            .request
            .cost_payment
            .return_permanents
            .iter()
            .chain(
                pending
                    .request
                    .cost_payment
                    .hand_cards_to_library_top
                    .iter(),
            )
            .find_map(|card| {
                pending
                    .choices
                    .get(card)
                    .filter(|(_, _, _, choice)| choice.is_none())
                    .map(|(incarnation, from, _, _)| (*card, *incarnation, *from))
            });
        let Some((commander, incarnation, from)) = next else {
            return Ok(false);
        };
        let owner = self.object(commander)?.owner;
        self.open_pending_decision(
            owner,
            if from == Zone::Hand {
                DecisionVisibility::Private
            } else {
                DecisionVisibility::Public
            },
            DecisionKind::CommanderZoneReplacement,
            0,
            1,
            vec![DecisionOption::Object(commander)],
            DecisionContinuation::CommanderActivationReplacement {
                commander,
                incarnation,
            },
        )?;
        Ok(true)
    }

    pub(super) fn validate_commander_activation_replacement(
        &self,
        decision: &PendingDecision,
    ) -> Result<(), RulesError> {
        let DecisionContinuation::CommanderActivationReplacement {
            commander,
            incarnation,
        } = decision.continuation
        else {
            return Err(RulesError::IllegalAction(
                "wrong commander activation continuation",
            ));
        };
        let pending = self
            .format
            .commander_activation
            .as_ref()
            .ok_or(RulesError::IllegalAction("missing suspended activation"))?;
        let Some((expected_incarnation, from, _, choice)) = pending.choices.get(&commander) else {
            return Err(RulesError::IllegalAction(
                "commander is not a pending activation cost",
            ));
        };
        if choice.is_some()
            || incarnation != *expected_incarnation
            || !self.object_has_incarnation(
                pending.request.activation.source,
                pending.source_incarnation,
            )
            || self
                .format
                .commanders
                .get(&commander)
                .map(|record| record.owner)
                != Some(decision.player)
            || decision.kind != DecisionKind::CommanderZoneReplacement
            || decision.visibility
                != if *from == Zone::Hand {
                    DecisionVisibility::Private
                } else {
                    DecisionVisibility::Public
                }
            || decision.min_selections != 0
            || decision.max_selections != 1
            || decision.options != vec![DecisionOption::Object(commander)]
            || pending
                .choices
                .iter()
                .any(|(card, (incarnation, from, _, _))| {
                    self.zone_of(*card) != Some(*from)
                        || !self.object_has_incarnation(*card, *incarnation)
                })
        {
            return Err(RulesError::IllegalAction(
                "stale commander activation replacement",
            ));
        }
        Ok(())
    }

    pub(super) fn consume_commander_activation_choice(
        &mut self,
        card: ObjectId,
        incarnation: u64,
        from: Option<Zone>,
        to: Zone,
    ) -> Option<bool> {
        let pending = self.format.commander_activation.as_mut()?;
        let (expected_incarnation, expected_from, expected_to, choice) =
            *pending.choices.get(&card)?;
        if incarnation != expected_incarnation || from != Some(expected_from) || to != expected_to {
            return None;
        }
        let choice = choice?;
        pending.choices.remove(&card);
        Some(choice)
    }
    pub(super) fn resolve_commander_stack_draws(
        &mut self,
        top: &StackObject,
        index: usize,
        resolution: &StackEffectResolution,
        optional_trigger: Option<(bool, Option<Target>)>,
        counter_payment: Option<bool>,
        counter_discard: Option<bool>,
    ) -> Result<Option<bool>, RulesError> {
        if self.format.commanders.is_empty() {
            return Ok(None);
        }
        let effect = &top.effects[index];
        if matches!(
            effect,
            Effect::RevealTopCardPutIntoHandLoseLifeEqualToManaValue
        ) {
            let key = (top.id, index);
            let (_, mana_value) =
                if let Some(revealed) = self.format.commander_revealed_cards.get(&key) {
                    *revealed
                } else {
                    let Some(card) = self.players[top.controller.0].library.last().copied() else {
                        return Ok(Some(false));
                    };
                    let definition = self.card_definition(card)?;
                    let value = definition.mana_cost.mana_value();
                    self.record_event(GameEvent::CardRevealed {
                        player: top.controller,
                        card,
                        definition: definition.id,
                    });
                    self.format
                        .commander_revealed_cards
                        .insert(key, (card, value));
                    (card, value)
                };
            if self.resolve_commander_draw_sequence(
                top,
                index,
                top.controller,
                1,
                optional_trigger,
                counter_payment,
                counter_discard,
            )? {
                return Ok(Some(true));
            }
            self.format.commander_revealed_cards.remove(&key);
            if mana_value > 0 {
                self.adjust_life(top.controller, -i64::from(mana_value));
                self.record_event(GameEvent::LifeLost {
                    source: top.card,
                    player: top.controller,
                    amount: i16::from(mana_value),
                });
            }
            return Ok(Some(false));
        }
        let targeted = match resolution {
            StackEffectResolution::Targeted {
                target: Target::Player(player),
                legal: true,
                ..
            } if !self.players[player.0].lost => Some(*player),
            _ => None,
        };
        let draw = match effect {
            Effect::DrawController => Some((top.controller, 1)),
            Effect::DrawControllerIfManaColorSpent { color } => Some((
                top.controller,
                usize::from(
                    top.mana_spent
                        .as_ref()
                        .is_some_and(|spent| spent.contains(color)),
                ),
            )),
            Effect::DrawTargetPlayer => targeted.map(|player| (player, 1)),
            Effect::DrawTargetPlayerCards { count } => {
                targeted.map(|player| (player, usize::from(*count)))
            }
            Effect::DrawControllerForEachControlledBasicLandType { land_type } => Some((
                top.controller,
                self.controlled_basic_land_type_count(top.controller, *land_type),
            )),
            _ => return Ok(None),
        };
        let Some((player, count)) = draw else {
            return Ok(None);
        };
        self.resolve_commander_draw_sequence(
            top,
            index,
            player,
            count,
            optional_trigger,
            counter_payment,
            counter_discard,
        )
        .map(Some)
    }

    pub(super) fn resolve_commander_draw_sequence(
        &mut self,
        top: &StackObject,
        index: usize,
        player: PlayerId,
        count: usize,
        optional_trigger: Option<(bool, Option<Target>)>,
        counter_payment: Option<bool>,
        counter_discard: Option<bool>,
    ) -> Result<bool, RulesError> {
        let key = (top.id, index);
        let (player, mut remaining) = *self
            .format
            .commander_stack_draws
            .entry(key)
            .or_insert((player, count));
        while remaining > 0 {
            if self.players[player.0]
                .library
                .last()
                .is_some_and(|card| self.is_commander(*card))
            {
                self.stack.push(top.clone());
                if index > 0 {
                    self.stack_effect_cursors.insert(top.id, index);
                }
                if self.suspend_top_for_commander_zone_replacement(
                    optional_trigger,
                    counter_payment,
                    counter_discard,
                )? {
                    return Ok(true);
                }
                self.stack.pop();
                self.stack_effect_cursors.remove(&top.id);
            }
            self.draw_card_from_spell_effect(player)?;
            remaining -= 1;
            self.format
                .commander_stack_draws
                .insert(key, (player, remaining));
        }
        self.format.commander_stack_draws.remove(&key);
        Ok(false)
    }
    pub(super) fn validate_commander_draw_replacement(
        &self,
        decision: &PendingDecision,
    ) -> Result<(), RulesError> {
        let DecisionContinuation::CommanderDrawReplacement {
            player,
            commander,
            incarnation,
        } = decision.continuation
        else {
            return Err(RulesError::IllegalAction(
                "wrong commander draw continuation",
            ));
        };
        if self.step != Step::Draw
            || !self.is_active_seat(player)
            || !self.stack.is_empty()
            || decision.player != player
            || decision.kind != DecisionKind::CommanderZoneReplacement
            || decision.visibility != DecisionVisibility::Private
            || decision.min_selections != 0
            || decision.max_selections != 1
            || decision.options != vec![DecisionOption::Object(commander)]
            || self.players[player.0].library.last() != Some(&commander)
            || self
                .format
                .commanders
                .get(&commander)
                .map(|record| record.owner)
                != Some(player)
            || !self.object_has_incarnation(commander, incarnation)
        {
            return Err(RulesError::IllegalAction(
                "stale commander draw replacement",
            ));
        }
        Ok(())
    }
    pub(super) fn validate_commander_replacement_state(&self) -> Result<(), RulesError> {
        for (stack, remaining) in &self.format.commander_group_draws {
            if remaining.is_empty()
                || self
                    .format
                    .commander_stack_draws
                    .get(&(*stack, 0))
                    .map(|(player, _)| player)
                    != remaining.first()
            {
                return Err(RulesError::IllegalAction(
                    "group draw queue escaped its pending recipient",
                ));
            }
        }
        for (key, (card, _)) in &self.format.commander_revealed_cards {
            if !self.format.commander_stack_draws.contains_key(key)
                || self.zone_of(*card) != Some(Zone::Library)
            {
                return Err(RulesError::IllegalAction(
                    "revealed commander escaped its pending move",
                ));
            }
        }
        if self.format.commander_activation.is_some()
            && !matches!(
                self.pending_decision
                    .as_ref()
                    .map(|decision| &decision.continuation),
                Some(DecisionContinuation::CommanderActivationReplacement { .. })
            )
        {
            return Err(RulesError::IllegalAction(
                "commander activation escaped its choice boundary",
            ));
        }
        if self.format.commander_draw_choice.is_some() {
            return Err(RulesError::IllegalAction(
                "commander draw permission escaped its atomic move",
            ));
        }
        for ((stack, index), (player, remaining)) in &self.format.commander_stack_draws {
            if *remaining == 0
                || self.players.get(player.0).is_none()
                || !matches!(self.pending_decision.as_ref().map(|decision| &decision.continuation),
                    Some(DecisionContinuation::CommanderZoneReplacement { stack: pending, effect_index, .. })
                        if stack == pending && index == effect_index)
            {
                return Err(RulesError::IllegalAction(
                    "partial draws escaped their replacement instruction",
                ));
            }
        }
        for ((stack, index, card), (incarnation, from, to, _)) in
            &self.format.commander_zone_choices
        {
            if !matches!(self.pending_decision.as_ref().map(|decision| &decision.continuation),
                Some(DecisionContinuation::CommanderZoneReplacement { stack: pending_stack, effect_index, .. })
                    if pending_stack == stack && effect_index == index)
                || !self.stack.last().is_some_and(|top| {
                    top.id == *stack && self.stack_effect_cursor(top).ok() == Some(*index)
                })
                || !self.is_commander(*card)
                || !matches!(to, Zone::Hand | Zone::Library)
                || self.zone_of(*card) != *from
                || !self.object_has_incarnation(*card, *incarnation)
            {
                return Err(RulesError::IllegalAction(
                    "commander replacement choice escaped its exact paused instruction",
                ));
            }
        }
        for stack in self.format.commander_selected_returns.keys() {
            if !matches!(self.pending_decision.as_ref().map(|decision| &decision.continuation),
                Some(DecisionContinuation::CommanderZoneReplacement { stack: pending_stack, .. }) if pending_stack == stack)
                || !self.stack.last().is_some_and(|top| top.id == *stack)
            {
                return Err(RulesError::IllegalAction(
                    "selected commander return escaped its replacement continuation",
                ));
            }
        }
        Ok(())
    }
    pub(super) fn resume_commander_selected_returns(
        &mut self,
        stack: StackObjectId,
    ) -> Result<(), RulesError> {
        let top = self
            .stack
            .last()
            .cloned()
            .ok_or(RulesError::IllegalAction("selected return lost its stack"))?;
        if top.id != stack {
            return Err(RulesError::IllegalAction("selected return stack changed"));
        }
        match self
            .format
            .commander_selected_returns
            .get(&stack)
            .cloned()
            .ok_or(RulesError::IllegalAction(
                "selected return continuation missing",
            ))? {
            SelectedReturns::Creatures(selected) => self
                .finish_public_graveyard_creature_return_spell(
                    top.id,
                    top.card,
                    top.source_incarnation,
                    top.controller,
                    selected,
                ),
            SelectedReturns::Lands(selected) => self.finish_public_graveyard_land_return_spell(
                top.id,
                top.card,
                top.source_incarnation,
                top.controller,
                &selected,
            ),
            SelectedReturns::TriggeredReturn(selected) => self.finish_chosen_trigger_return(
                top.card,
                top.ability_id.ok_or(RulesError::IllegalAction(
                    "selected return lost its ability",
                ))?,
                Some(selected),
            ),
            SelectedReturns::Search {
                selected,
                destination,
                reveal_selected,
            } => {
                let index = self.stack_effect_cursor(&top)?;
                self.finish_library_search_selection(
                    top,
                    destination,
                    selected,
                    reveal_selected,
                    index,
                )
            }
            SelectedReturns::SearchMany {
                selected,
                destination,
                reveal_selected,
            } => self.finish_library_search_many_selection(
                top,
                destination,
                reveal_selected,
                &selected,
            ),
            SelectedReturns::Partition {
                cards,
                hand,
                top: library_top,
                bottom,
            } => {
                self.finish_library_top_partition_selection(top, &cards, hand, library_top, &bottom)
            }
            SelectedReturns::Recycle {
                selected,
                remaining_draws,
            } => self.finish_hand_recycle_selection(top, &selected, remaining_draws),
            SelectedReturns::PaidLook { cards, selected } => {
                self.finish_paid_library_look(top, &cards, &selected)
            }
            SelectedReturns::TerminalDraw { player, count } => {
                self.finish_commander_terminal_draw(top, player, count)
            }
        }
    }
    fn commander_zone_intents(
        &self,
        top: &StackObject,
    ) -> Result<Vec<(ObjectId, Zone)>, RulesError> {
        let index = self.stack_effect_cursor(top)?;
        if self.format.warp_world.contains_key(&(top.id, index)) {
            return Ok(Vec::new());
        }
        let Some(effect) = top.effects.get(index) else {
            return Ok(Vec::new());
        };
        let offset = Self::effect_target_offset(&top.effects, index);
        let target = top.targets.get(offset).copied();
        let targeted_zone = match effect {
            Effect::ReturnTargetPermanentToHandAndLoseControllerLife { .. }
            | Effect::ReturnTargetEnchantmentToOwnersHand
            | Effect::ReturnTargetCardToHand
            | Effect::ReturnTargetCreatureCardToHand
            | Effect::ReturnTargetCreatureCardFromGraveyardToOwnersHand
            | Effect::ReturnTargetEnchantmentCardToHand
            | Effect::ReturnControlledCreatureToHand
            | Effect::ReturnControlledLandToHand
            | Effect::ReturnOpponentCreatureToHand => Some(Zone::Hand),
            Effect::ReturnTargetCreatureCardToHandIfAnotherInControllerGraveyard
                if self.controller_creature_cards_in_graveyard(top.controller) >= 2 =>
            {
                Some(Zone::Hand)
            }
            Effect::PutTargetCreatureOnOwnersLibraryTop
            | Effect::PutTargetCreatureCardInControllerGraveyardOnOwnersLibraryTop
            | Effect::PutTargetGraveyardCardOnOwnersLibraryBottom => Some(Zone::Library),
            _ => None,
        };
        let mut intents = Vec::new();
        if let Some((player, remaining)) = self.format.commander_stack_draws.get(&(top.id, index))
            && *remaining > 0
        {
            intents.extend(
                self.players[player.0]
                    .library
                    .last()
                    .map(|card| (*card, Zone::Hand)),
            );
        }
        if let Some(selected) = self.format.commander_selected_returns.get(&top.id) {
            match selected {
                SelectedReturns::Creatures(cards) => {
                    intents.extend(cards.iter().map(|card| (card.card, Zone::Hand)))
                }
                SelectedReturns::Lands(cards) => {
                    intents.extend(cards.iter().map(|card| (card.card, Zone::Hand)))
                }
                SelectedReturns::TriggeredReturn(card) => intents.push((*card, Zone::Hand)),
                SelectedReturns::Search {
                    selected,
                    destination: LibrarySearchDestination::Hand,
                    ..
                } => {
                    intents.extend(selected.iter().map(|card| (*card, Zone::Hand)));
                }
                SelectedReturns::Search { .. } => {}
                SelectedReturns::SearchMany {
                    selected,
                    destination: LibrarySearchDestination::Hand,
                    ..
                } => intents.extend(selected.iter().map(|card| (*card, Zone::Hand))),
                SelectedReturns::SearchMany { .. } => {}
                SelectedReturns::Partition { hand, .. } => intents.push((*hand, Zone::Hand)),
                SelectedReturns::Recycle {
                    selected,
                    remaining_draws: None,
                } => intents.extend(selected.iter().map(|card| (*card, Zone::Library))),
                SelectedReturns::Recycle {
                    remaining_draws: Some(remaining),
                    ..
                } if *remaining > 0 => intents.extend(
                    self.players[top.controller.0]
                        .library
                        .last()
                        .map(|card| (*card, Zone::Hand)),
                ),
                SelectedReturns::Recycle { .. } => {}
                SelectedReturns::PaidLook { selected, .. } => {
                    intents.extend(selected.iter().map(|card| (*card, Zone::Hand)))
                }
                SelectedReturns::TerminalDraw { .. } => {}
            }
        }
        if let (Some(to), Some(target), Some(requirement)) =
            (targeted_zone, target, effect.target_requirement())
        {
            let legal = self
                .stack_target_legality_snapshots
                .get(&top.id)
                .and_then(|snapshot| snapshot.get(index))
                .map_or_else(
                    || {
                        self.target_matches_for_colors(
                            top.controller,
                            target,
                            requirement,
                            &top.source_colors,
                        )
                    },
                    |resolution| {
                        matches!(
                            resolution,
                            StackEffectResolution::Targeted { legal: true, .. }
                        )
                    },
                );
            if legal
                && self.stack_target_incarnation_matches(top, offset, target)
                && self.target_remains_in_resolution_zone(target, requirement)
                && let Target::Permanent(card) = target
            {
                intents.push((card, to));
            }
        }
        match effect {
            Effect::CounterTargetSpellToOwnersHand => {
                if let Some(Target::Spell(card)) = target
                    && !self.virtual_spell_copies.contains_key(&card)
                    && !self.exile_on_resolution.contains(&card)
                    && self.stack.iter().any(|item| item.card == card && item.ability_id.is_none())
                    && self.stack_target_incarnation_matches(top, offset, Target::Spell(card))
                {
                    intents.push((card, Zone::Hand));
                }
            }
            Effect::SearchControllerLibrary {
                requirement,
                destination: LibrarySearchDestination::Hand,
                selection: LibrarySearchSelection::DeterministicFirstMatch,
                ..
            } if self.library_search_prevented_until != Some(self.turn) => {
                intents.extend(
                    self.library_search_candidates(top.controller, requirement, top.chosen_x)?
                        .first()
                        .map(|card| (*card, Zone::Hand)),
                );
            }
            Effect::SearchControllerLibraryMany {
                requirement,
                destination: LibrarySearchDestination::Hand,
                cardinality,
                selection: LibrarySearchSelection::DeterministicFirstMatch,
                ..
            } if self.library_search_prevented_until != Some(self.turn) => {
                let candidates =
                    self.library_search_candidates(top.controller, requirement, top.chosen_x)?;
                let (_, max) =
                    Self::library_search_cardinality_bounds(*cardinality, candidates.len(), false)?;
                intents.extend(
                    self.deterministic_multi_library_search_selection(
                        candidates,
                        *cardinality,
                        max,
                    )?
                    .into_iter()
                    .map(|card| (card, Zone::Hand)),
                );
            }
            Effect::WarpOwnedPermanentsIntoLibrariesThenRevealAndReturnPermanentCards => {
                intents.extend(
                    self.all_battlefield_cards()
                        .into_iter()
                        .map(|card| (card, Zone::Library)),
                );
            }
            Effect::ReturnLinkedHandExileToControllerHand => {
                if let Some(group) = self
                    .linked_hand_exile_groups
                    .get(&(top.card, top.source_incarnation))
                {
                    intents.extend(group.members.iter().filter_map(|member| {
                        (self.zone_of(member.object) == Some(Zone::Exile)
                            && self.object_has_incarnation(member.object, member.exile_incarnation))
                        .then_some((member.object, Zone::Hand))
                    }));
                }
            }
            Effect::ReturnSourceAttachedPermanentToHand
                if self.zone_of(top.card) == Some(Zone::Battlefield)
                    && self.object_has_incarnation(top.card, top.source_incarnation) =>
            {
                let source = self.object(top.card)?;
                if let (Some(card), Some(incarnation)) =
                    (source.attached_to, source.attached_to_incarnation)
                    && self.zone_of(card) == Some(Zone::Battlefield)
                    && self.object_has_incarnation(card, incarnation)
                {
                    intents.push((card, Zone::Hand));
                }
            }
            Effect::ReturnControllerCreatureCardsPutIntoGraveyardFromBattlefieldThisTurnToHand => {
                intents.extend(
                    self.creature_cards_put_into_graveyard_from_battlefield_this_turn
                        .iter()
                        .filter_map(|(owner, card, incarnation)| {
                            (*owner == top.controller
                                && self.zone_of(*card) == Some(Zone::Graveyard)
                                && self.object(*card).is_ok_and(|object| {
                                    object.owner == *owner && object.incarnation == *incarnation
                                }))
                            .then_some((*card, Zone::Hand))
                        }),
                );
            }
            Effect::ReturnSourceToOwnersHand | Effect::MoveSourceToOwnersLibraryAndShuffle
                if self.zone_of(top.card) == Some(Zone::Battlefield)
                    && self.object_has_incarnation(top.card, top.source_incarnation) =>
            {
                intents.push((
                    top.card,
                    if matches!(effect, Effect::ReturnSourceToOwnersHand) {
                        Zone::Hand
                    } else {
                        Zone::Library
                    },
                ));
            }
            Effect::ReturnSourceFromOwnersGraveyardToHand
                if self.zone_of(top.card) == Some(Zone::Graveyard)
                    && self.object_has_incarnation(top.card, top.source_incarnation) =>
            {
                intents.push((top.card, Zone::Hand));
            }
            Effect::ShuffleGraveyardsIntoLibraries => {
                for player in &self.players {
                    intents.extend(player.graveyard.iter().map(|card| (*card, Zone::Library)));
                }
            }
            _ => {}
        }
        intents.retain(|(card, to)| self.is_commander(*card) && self.zone_of(*card) != Some(*to));
        intents.sort_by_key(|(card, _)| {
            let owner = self.format.commanders[card].owner.0;
            (
                (owner + self.players.len() - self.active_player.0) % self.players.len(),
                *card,
            )
        });
        intents.dedup();
        Ok(intents)
    }

    pub(super) fn suspend_top_for_commander_zone_replacement(
        &mut self,
        optional_trigger: Option<(bool, Option<Target>)>,
        counter_payment: Option<bool>,
        counter_discard: Option<bool>,
    ) -> Result<bool, RulesError> {
        if self.format.commanders.is_empty() {
            return Ok(false);
        }
        let Some(top) = self.stack.last() else {
            return Ok(false);
        };
        let stack = top.id;
        let effect_index = self.stack_effect_cursor(top)?;
        if effect_index == 0 && self.stack_item_targets_all_illegal(top)? {
            return Ok(false);
        }
        for (commander, to) in self.commander_zone_intents(top)? {
            if self
                .format
                .commander_zone_choices
                .contains_key(&(stack, effect_index, commander))
            {
                continue;
            }
            let object = self.object(commander)?;
            let incarnation = object.incarnation;
            let owner = object.owner;
            if self.players[owner.0].lost {
                continue;
            }
            let from = self.zone_of(commander);
            if from.is_none() && !self.stack.iter().any(|item| item.card == commander && item.ability_id.is_none()) {
                return Err(RulesError::IllegalAction("commander replacement source is not in a zone or on the stack"));
            }
            let visibility = if matches!(from, Some(Zone::Hand | Zone::Library)) {
                DecisionVisibility::Private
            } else {
                DecisionVisibility::Public
            };
            self.open_pending_decision(
                owner,
                visibility,
                DecisionKind::CommanderZoneReplacement,
                0,
                1,
                vec![DecisionOption::Object(commander)],
                DecisionContinuation::CommanderZoneReplacement {
                    stack,
                    effect_index,
                    commander,
                    incarnation,
                    from,
                    to,
                    optional_trigger,
                    counter_payment,
                    counter_discard,
                },
            )?;
            return Ok(true);
        }
        Ok(false)
    }

    pub(super) fn validate_commander_zone_replacement(
        &self,
        decision: &PendingDecision,
    ) -> Result<(), RulesError> {
        let DecisionContinuation::CommanderZoneReplacement {
            stack,
            effect_index,
            commander,
            incarnation,
            from,
            to,
            ..
        } = &decision.continuation
        else {
            return Err(RulesError::IllegalAction(
                "wrong commander replacement continuation",
            ));
        };
        let top = self.stack.last().ok_or(RulesError::IllegalAction(
            "commander replacement lost its enclosing stack item",
        ))?;
        if top.id != *stack
            || self.stack_effect_cursor(top)? != *effect_index
            || decision.kind != DecisionKind::CommanderZoneReplacement
            || decision.visibility
                != if matches!(from, Some(Zone::Hand | Zone::Library)) {
                    DecisionVisibility::Private
                } else {
                    DecisionVisibility::Public
                }
            || decision.min_selections != 0
            || decision.max_selections != 1
            || decision.options != vec![DecisionOption::Object(*commander)]
            || !matches!(to, Zone::Hand | Zone::Library)
            || self
                .format
                .commanders
                .get(commander)
                .map(|record| record.owner)
                != Some(decision.player)
            || self.zone_of(*commander) != *from
            || !self.object_has_incarnation(*commander, *incarnation)
            || !self
                .commander_zone_intents(top)?
                .contains(&(*commander, *to))
            || self
                .format
                .commander_zone_choices
                .contains_key(&(*stack, *effect_index, *commander))
        {
            return Err(RulesError::IllegalAction(
                "stale commander replacement instruction or incarnation",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(effects: Vec<Effect>, zone: Zone) -> (Game, ObjectId, ObjectId) {
        setup_owned(effects, zone, PlayerId(1))
    }

    fn setup_owned(
        effects: Vec<Effect>,
        zone: Zone,
        owner: PlayerId,
    ) -> (Game, ObjectId, ObjectId) {
        let body = CardDefinition {
            id: "REPLACEMENT-COMMANDER",
            name: "Replacement unit-test commander",
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["unit-test"],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        };
        let spell = CardDefinition {
            id: "REPLACEMENT-SPELL",
            name: "Replacement unit-test spell",
            card_types: BTreeSet::from([CardType::Instant]),
            power: None,
            toughness: None,
            effects,
            ..body.clone()
        };
        let mut game = Game::new([body, spell], 4).unwrap();
        game.configure_commander_format(40, 21).unwrap();
        let commander = game
            .put_on_battlefield(owner, "REPLACEMENT-COMMANDER")
            .unwrap();
        game.designate_commander(owner, commander).unwrap();
        if zone != Zone::Battlefield {
            game.move_to_zone(commander, zone).unwrap();
        }
        let spell = game
            .add_card(PlayerId(0), "REPLACEMENT-SPELL", Zone::Hand)
            .unwrap();
        game.add_card(PlayerId(1), "REPLACEMENT-COMMANDER", Zone::Library)
            .unwrap();
        game.begin_game().unwrap();
        // A commander deliberately retained in its graveyard is legal. The
        // fixture uses the real optional SBA decision, not a forged flag.
        while let Some(decision) = game.pending_decision.clone() {
            assert_eq!(decision.kind, DecisionKind::CommanderReturn);
            game.submit_decision(
                decision.player,
                decision.id,
                DecisionSelection::Objects(vec![]),
            )
            .unwrap();
        }
        (game, commander, spell)
    }

    fn cast_and_resolve(game: &mut Game, spell: ObjectId, target: ObjectId) {
        game.cast_spell(
            PlayerId(0),
            CastRequest {
                card: spell,
                targets: vec![Target::Permanent(target)],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
        )
        .unwrap();
        game.atomic_transition(|game| game.resolve_top_of_stack())
            .unwrap();
    }

    #[test]
    fn draw_step_commander_choice_is_private_optional_and_consumes_exactly_one_draw() {
        for accept in [false, true] {
            let (mut game, commander, _) = setup_owned(vec![], Zone::Library, PlayerId(0));
            let incarnation = game.object(commander).unwrap().incarnation;
            for _ in 0..4 {
                game.pass_priority(game.priority).unwrap();
            }
            assert_eq!(game.step, Step::Draw);
            assert_eq!(game.pending_draw_replacement, Some(PlayerId(0)));
            game.resolve_pending_draw(PlayerId(0), None).unwrap();
            let decision = game.pending_decision.clone().unwrap();
            assert_eq!(decision.visibility, DecisionVisibility::Private);
            assert!(matches!(
                decision.continuation,
                DecisionContinuation::CommanderDrawReplacement { .. }
            ));
            assert_eq!(game.zone_of(commander), Some(Zone::Library));
            assert!(game.pass_priority(PlayerId(0)).is_err());
            assert!(game.resolve_pending_draw(PlayerId(0), None).is_err());
            assert!(
                game.submit_decision(PlayerId(1), decision.id, DecisionSelection::Objects(vec![]))
                    .is_err()
            );
            game.submit_decision(
                PlayerId(0),
                decision.id,
                DecisionSelection::Objects(if accept { vec![commander] } else { vec![] }),
            )
            .unwrap();
            assert_eq!(
                game.zone_of(commander),
                Some(if accept { Zone::Command } else { Zone::Hand })
            );
            assert_eq!(game.object(commander).unwrap().incarnation, incarnation + 1);
            assert!(game.pending_draw_replacement.is_none());
            assert!(game.pending_decision.is_none());
            assert!(game.format.commander_draw_choice.is_none());
            assert!(
                game.submit_decision(PlayerId(0), decision.id, DecisionSelection::Objects(vec![]))
                    .is_err()
            );
            for _ in 0..4 {
                game.pass_priority(game.priority).unwrap();
            }
            assert_eq!(game.step, Step::PrecombatMain);
            game.validate_invariants().unwrap();
        }
    }

    #[test]
    fn multi_draw_retains_completed_draws_and_resumes_the_spell_suffix() {
        for accept in [false, true] {
            let (mut game, commander, spell) = setup(
                vec![
                    Effect::GainLifeController { amount: 2 },
                    Effect::DrawTargetPlayerCards { count: 2 },
                    Effect::GainLifeController { amount: 5 },
                ],
                Zone::Library,
            );
            let first = *game.players[1].library.last().unwrap();
            assert_ne!(first, commander);
            game.cast_spell(
                PlayerId(0),
                CastRequest {
                    card: spell,
                    targets: vec![Target::Player(PlayerId(1))],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
            )
            .unwrap();
            game.atomic_transition(|game| game.resolve_top_of_stack())
                .unwrap();
            assert_eq!(game.zone_of(first), Some(Zone::Hand));
            assert_eq!(game.zone_of(commander), Some(Zone::Library));
            assert_eq!(game.players[0].life, 42);
            let decision = game.pending_decision.clone().unwrap();
            assert_eq!(decision.visibility, DecisionVisibility::Private);
            assert_eq!(decision.player, PlayerId(1));
            game.submit_decision(
                PlayerId(1),
                decision.id,
                DecisionSelection::Objects(if accept { vec![commander] } else { vec![] }),
            )
            .unwrap();
            assert_eq!(
                game.zone_of(commander),
                Some(if accept { Zone::Command } else { Zone::Hand })
            );
            assert_eq!(game.players[0].life, 47);
            assert!(
                !game.players[1].lost,
                "completed draws must not replay and overdraw"
            );
            assert_eq!(game.zone_of(spell), Some(Zone::Graveyard));
            assert!(game.format.commander_stack_draws.is_empty());
            assert_eq!(
                game.event_log
                    .iter()
                    .filter(|event| matches!(event,
                GameEvent::CardMoved { card, to: Zone::Hand } if *card == first))
                    .count(),
                1
            );
            game.validate_invariants().unwrap();
        }
    }

    #[test]
    fn shared_turn_waits_for_commander_choice_before_opening_teammates_draw() {
        let (mut game, commander, _) = setup_owned(vec![], Zone::Library, PlayerId(0));
        // Explicit rules-unit boundary, not a scored game or benchmark fixture.
        game.started = false;
        game.refresh_public_state_integrity();
        game.configure_teams(
            &[
                vec![PlayerId(0), PlayerId(2)],
                vec![PlayerId(1), PlayerId(3)],
            ],
            30,
        )
        .unwrap();
        let teammate_card = game
            .add_card(PlayerId(2), "REPLACEMENT-COMMANDER", Zone::Library)
            .unwrap();
        game.step = Step::Draw;
        game.turn = 3;
        game.started = true;
        game.pending_team_draws = vec![PlayerId(2)];
        game.open_pending_draw_replacement(PlayerId(0)).unwrap();
        game.refresh_public_state_integrity();
        game.validate_invariants().unwrap();
        game.resolve_pending_draw(PlayerId(0), None).unwrap();
        assert_eq!(game.pending_draw_replacement, None);
        assert_eq!(game.pending_team_draws, vec![PlayerId(2)]);
        assert_eq!(game.zone_of(teammate_card), Some(Zone::Library));
        let decision = game.pending_decision.clone().unwrap();
        game.submit_decision(
            PlayerId(0),
            decision.id,
            DecisionSelection::Objects(vec![commander]),
        )
        .unwrap();
        assert_eq!(game.pending_draw_replacement, Some(PlayerId(2)));
        game.resolve_pending_draw(PlayerId(2), None).unwrap();
        assert_eq!(game.zone_of(teammate_card), Some(Zone::Hand));
        assert!(game.pending_team_draws.is_empty());
        game.validate_invariants().unwrap();
    }

    #[test]
    fn activation_collects_public_and_private_cost_choices_without_repaying() {
        let template = setup(vec![], Zone::Battlefield).0.catalog["REPLACEMENT-COMMANDER"].clone();
        let mut game = Game::new_with_all_bindings(
            [template],
            2,
            [],
            [],
            [],
            [ActivatedAbilityBinding {
                card_definition: "REPLACEMENT-COMMANDER",
                ability: ActivatedAbility {
                    id: "return-and-put",
                    mana_cost: ManaCost::new(0),
                    tap_cost: false,
                    sorcery_speed: false,
                    additional_tap_creatures: 0,
                    sacrifice_source: false,
                    sacrifice_creatures: 0,
                    sacrifice_lands: 0,
                    discard_cards: 0,
                    targets: vec![],
                    effects: vec![Effect::GainLifeController { amount: 1 }],
                },
            }],
        )
        .unwrap();
        game.register_generalized_activated_ability_cost_bindings([ActivatedAbilityCostBinding {
            card_definition: "REPLACEMENT-COMMANDER",
            ability_id: "return-and-put",
            cost: GeneralizedActivatedAbilityCost {
                life_payment: 2,
                return_source_to_hand: true,
                put_hand_cards_on_library_top: 1,
                ..Default::default()
            },
        }])
        .unwrap();
        game.configure_commander_format(40, 21).unwrap();
        let source = game
            .put_on_battlefield(PlayerId(0), "REPLACEMENT-COMMANDER")
            .unwrap();
        let hand = game
            .put_on_battlefield(PlayerId(0), "REPLACEMENT-COMMANDER")
            .unwrap();
        game.designate_commander(PlayerId(0), source).unwrap();
        game.designate_commander(PlayerId(0), hand).unwrap();
        game.move_to_zone(hand, Zone::Hand).unwrap();
        game.begin_game().unwrap();
        game.activate_ability_with_generalized_costs(
            PlayerId(0),
            GeneralizedAbilityActivation {
                activation: AbilityActivation {
                    source,
                    ability_id: "return-and-put",
                    sacrifice_sources: vec![],
                    additional_tap_creatures: vec![],
                    discard_cards: vec![],
                    targets: vec![],
                },
                cost_payment: AbilityCostPayment {
                    return_permanents: vec![source],
                    hand_cards_to_library_top: vec![hand],
                    ..Default::default()
                },
                mana_payment_selection: None,
            },
        )
        .unwrap();
        let first = game.pending_decision.clone().unwrap();
        assert_eq!(first.visibility, DecisionVisibility::Public);
        game.submit_decision(
            PlayerId(0),
            first.id,
            DecisionSelection::Objects(vec![source]),
        )
        .unwrap();
        let second = game.pending_decision.clone().unwrap();
        assert_eq!(second.visibility, DecisionVisibility::Private);
        assert_eq!(game.players[0].life, 40);
        assert_eq!(game.zone_of(source), Some(Zone::Battlefield));
        assert_eq!(game.zone_of(hand), Some(Zone::Hand));
        game.submit_decision(PlayerId(0), second.id, DecisionSelection::Objects(vec![]))
            .unwrap();
        assert_eq!(game.zone_of(source), Some(Zone::Command));
        assert_eq!(game.zone_of(hand), Some(Zone::Library));
        assert_eq!(game.players[0].life, 38);
        assert_eq!(game.stack.len(), 1);
        assert!(game.format.commander_activation.is_none());
        for _ in 0..2 {
            game.pass_priority(game.priority).unwrap();
        }
        assert_eq!(game.players[0].life, 39);
        game.validate_invariants().unwrap();
    }

    #[test]
    fn dredge_commander_choice_occurs_after_milling_and_never_repeats_the_mill() {
        for accept in [false, true] {
            let (mut game, commander, _) = setup_owned(vec![], Zone::Graveyard, PlayerId(0));
            // Define the dredge interaction for this rules unit, not a legal-deck claim.
            game.started = false;
            game.catalog
                .get_mut("REPLACEMENT-COMMANDER")
                .unwrap()
                .keywords
                .push(Keyword::Dredge(2));
            game.refresh_public_state_integrity();
            let first = game
                .add_card(PlayerId(0), "REPLACEMENT-COMMANDER", Zone::Library)
                .unwrap();
            let second = game
                .add_card(PlayerId(0), "REPLACEMENT-COMMANDER", Zone::Library)
                .unwrap();
            game.started = true;
            game.refresh_public_state_integrity();
            for _ in 0..4 {
                game.pass_priority(game.priority).unwrap();
            }
            game.resolve_pending_draw(PlayerId(0), Some(commander))
                .unwrap();
            assert_eq!(game.zone_of(first), Some(Zone::Graveyard));
            assert_eq!(game.zone_of(second), Some(Zone::Graveyard));
            assert_eq!(game.zone_of(commander), Some(Zone::Graveyard));
            let decision = game.pending_decision.clone().unwrap();
            assert!(matches!(
                decision.continuation,
                DecisionContinuation::CommanderDredgeReplacement { .. }
            ));
            assert_eq!(decision.visibility, DecisionVisibility::Public);
            game.submit_decision(
                PlayerId(0),
                decision.id,
                DecisionSelection::Objects(if accept { vec![commander] } else { vec![] }),
            )
            .unwrap();
            assert_eq!(
                game.zone_of(commander),
                Some(if accept { Zone::Command } else { Zone::Hand })
            );
            assert!(!game.players[0].lost);
            assert_eq!(
                game.event_log
                    .iter()
                    .filter(|event| matches!(event,
                GameEvent::Dredged { card, count: 2, .. } if *card == commander))
                    .count(),
                1
            );
            for card in [first, second] {
                assert_eq!(
                    game.event_log
                        .iter()
                        .filter(|event| matches!(event,
                    GameEvent::CardMoved { card: moved, to: Zone::Graveyard } if *moved == card))
                        .count(),
                    1
                );
            }
            game.validate_invariants().unwrap();
        }
    }

    #[test]
    fn bounce_choice_precedes_move_and_resumes_suffix_without_replaying_prefix() {
        for accept in [false, true] {
            let (mut game, commander, spell) = setup(
                vec![
                    Effect::GainLifeController { amount: 2 },
                    Effect::ReturnTargetPermanentToHandAndLoseControllerLife { amount: 3 },
                    Effect::GainLifeController { amount: 5 },
                ],
                Zone::Battlefield,
            );
            let incarnation = game.object(commander).unwrap().incarnation;
            cast_and_resolve(&mut game, spell, commander);
            assert_eq!(game.players[0].life, 42);
            assert_eq!(game.players[1].life, 40);
            assert_eq!(game.zone_of(commander), Some(Zone::Battlefield));
            let decision = game.pending_decision.clone().unwrap();
            assert_eq!(decision.kind, DecisionKind::CommanderZoneReplacement);
            assert_eq!(decision.player, PlayerId(1));
            assert!(game.pass_priority(PlayerId(1)).is_err());
            assert!(
                game.submit_decision(
                    PlayerId(2),
                    decision.id,
                    DecisionSelection::Objects(vec![commander])
                )
                .is_err()
            );
            game.submit_decision(
                PlayerId(1),
                decision.id,
                DecisionSelection::Objects(if accept { vec![commander] } else { vec![] }),
            )
            .unwrap();
            assert_eq!(
                game.zone_of(commander),
                Some(if accept { Zone::Command } else { Zone::Hand })
            );
            assert_eq!(game.object(commander).unwrap().incarnation, incarnation + 1);
            assert_eq!(game.players[0].life, 47);
            assert_eq!(game.players[1].life, 37);
            assert_eq!(game.zone_of(spell), Some(Zone::Graveyard));
            assert!(game.format.commander_zone_choices.is_empty());
            assert!(
                game.submit_decision(PlayerId(1), decision.id, DecisionSelection::Objects(vec![]))
                    .is_err()
            );
            game.validate_invariants().unwrap();
        }
    }

    #[test]
    fn library_bottom_replacement_does_not_pop_an_unrelated_card() {
        for accept in [false, true] {
            let (mut game, commander, spell) = setup(
                vec![Effect::PutTargetGraveyardCardOnOwnersLibraryBottom],
                Zone::Graveyard,
            );
            let library = game.players[1].library.clone();
            cast_and_resolve(&mut game, spell, commander);
            let decision = game.pending_decision.clone().unwrap();
            game.submit_decision(
                PlayerId(1),
                decision.id,
                DecisionSelection::Objects(if accept { vec![commander] } else { vec![] }),
            )
            .unwrap();
            if accept {
                assert_eq!(game.zone_of(commander), Some(Zone::Command));
                assert_eq!(game.players[1].library, library);
            } else {
                assert_eq!(game.zone_of(commander), Some(Zone::Library));
                assert_eq!(game.players[1].library[0], commander);
                assert_eq!(game.players[1].library[1..], library);
            }
            game.validate_invariants().unwrap();
        }
    }

    #[test]
    fn simultaneous_library_returns_collect_all_owner_choices_before_moving() {
        let (mut game, commander, spell) = setup(
            vec![Effect::ShuffleGraveyardsIntoLibraries],
            Zone::Graveyard,
        );
        // A separately designated unit-fixture commander in another graveyard.
        game.started = false;
        game.refresh_public_state_integrity();
        let other = game
            .put_on_battlefield(PlayerId(2), "REPLACEMENT-COMMANDER")
            .unwrap();
        game.designate_commander(PlayerId(2), other).unwrap();
        game.move_to_zone(other, Zone::Graveyard).unwrap();
        game.started = true;
        game.refresh_public_state_integrity();
        game.check_state_based_actions().unwrap();
        let arrival = game.pending_decision.clone().unwrap();
        game.submit_decision(PlayerId(2), arrival.id, DecisionSelection::Objects(vec![]))
            .unwrap();
        game.cast_spell(
            PlayerId(0),
            CastRequest {
                card: spell,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
        )
        .unwrap();
        game.atomic_transition(|game| game.resolve_top_of_stack())
            .unwrap();
        let first = game.pending_decision.clone().unwrap();
        assert_eq!(first.player, PlayerId(1));
        game.submit_decision(
            PlayerId(1),
            first.id,
            DecisionSelection::Objects(vec![commander]),
        )
        .unwrap();
        assert_eq!(game.zone_of(commander), Some(Zone::Graveyard));
        assert_eq!(game.zone_of(other), Some(Zone::Graveyard));
        let second = game.pending_decision.clone().unwrap();
        assert_eq!(second.player, PlayerId(2));
        game.submit_decision(PlayerId(2), second.id, DecisionSelection::Objects(vec![]))
            .unwrap();
        assert_eq!(game.zone_of(commander), Some(Zone::Command));
        assert_eq!(game.zone_of(other), Some(Zone::Library));
        assert!(game.format.commander_zone_choices.is_empty());
        game.validate_invariants().unwrap();
    }

    #[test]
    fn chosen_graveyard_card_is_retained_while_owner_replaces_the_return() {
        let (mut game, commander, spell) = setup(
            vec![Effect::ReturnOneCreatureCardFromEachGraveyardToHand],
            Zone::Graveyard,
        );
        game.cast_spell(
            PlayerId(0),
            CastRequest {
                card: spell,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
        )
        .unwrap();
        game.atomic_transition(|game| game.resolve_top_of_stack())
            .unwrap();
        let selection = game.pending_decision.clone().unwrap();
        assert_eq!(selection.kind, DecisionKind::PublicGraveyardCreatureReturn);
        game.submit_decision(
            PlayerId(1),
            selection.id,
            DecisionSelection::Objects(vec![commander]),
        )
        .unwrap();
        let replacement = game.pending_decision.clone().unwrap();
        assert_eq!(replacement.kind, DecisionKind::CommanderZoneReplacement);
        assert_eq!(game.zone_of(commander), Some(Zone::Graveyard));
        game.submit_decision(
            PlayerId(1),
            replacement.id,
            DecisionSelection::Objects(vec![commander]),
        )
        .unwrap();
        assert_eq!(game.zone_of(commander), Some(Zone::Command));
        assert_eq!(game.zone_of(spell), Some(Zone::Graveyard));
        assert!(game.format.commander_selected_returns.is_empty());
        assert!(game.format.commander_zone_choices.is_empty());
        game.validate_invariants().unwrap();
    }

    #[test]
    fn private_library_search_retains_selection_and_resumes_after_replacement() {
        for accept in [false, true] {
            let (mut game, commander, spell) = setup_owned(
                vec![Effect::SearchControllerLibrary {
                    requirement: LibrarySearchRequirement::ManaValueExactly(0),
                    destination: LibrarySearchDestination::Hand,
                    selection: LibrarySearchSelection::PolicySubmitted {
                        may_fail_to_find: true,
                    },
                    reveal_selected: true,
                }],
                Zone::Library,
                PlayerId(0),
            );
            game.cast_spell(
                PlayerId(0),
                CastRequest {
                    card: spell,
                    targets: vec![],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
            )
            .unwrap();
            game.atomic_transition(|game| game.resolve_top_of_stack())
                .unwrap();
            let search = game.pending_decision.clone().unwrap();
            assert_eq!(search.kind, DecisionKind::LibrarySearch);
            game.submit_decision(
                PlayerId(0),
                search.id,
                DecisionSelection::Objects(vec![commander]),
            )
            .unwrap();
            let replacement = game.pending_decision.clone().unwrap();
            assert_eq!(replacement.kind, DecisionKind::CommanderZoneReplacement);
            assert_eq!(replacement.visibility, DecisionVisibility::Private);
            assert_eq!(game.zone_of(commander), Some(Zone::Library));
            game.submit_decision(
                PlayerId(0),
                replacement.id,
                DecisionSelection::Objects(if accept { vec![commander] } else { vec![] }),
            )
            .unwrap();
            assert_eq!(
                game.zone_of(commander),
                Some(if accept { Zone::Command } else { Zone::Hand })
            );
            assert_eq!(game.zone_of(spell), Some(Zone::Graveyard));
            assert!(game.format.commander_selected_returns.is_empty());
            assert!(game.format.commander_zone_choices.is_empty());
            game.validate_invariants().unwrap();
        }
    }

    #[test]
    fn multi_search_and_private_partition_retain_the_selected_commander() {
        for partition in [false, true] {
            for accept in [false, true] {
                let effect = if partition {
                    Effect::LookAtTopCardsPutOneInHandOneOnTopRestOnBottom { count: 3 }
                } else {
                    Effect::SearchControllerLibraryMany {
                        requirement: LibrarySearchRequirement::ManaValueExactly(0),
                        destination: LibrarySearchDestination::Hand,
                        cardinality: LibrarySearchCardinality::Exactly(1),
                        selection: LibrarySearchSelection::PolicySubmitted {
                            may_fail_to_find: false,
                        },
                        reveal_selected: true,
                    }
                };
                let (mut game, commander, spell) =
                    setup_owned(vec![effect], Zone::Library, PlayerId(0));
                game.cast_spell(
                    PlayerId(0),
                    CastRequest {
                        card: spell,
                        targets: vec![],
                        convoke: vec![],
                        payment_mana_abilities: vec![],
                    },
                )
                .unwrap();
                game.atomic_transition(|game| game.resolve_top_of_stack())
                    .unwrap();
                let selection = game.pending_decision.clone().unwrap();
                game.submit_decision(
                    PlayerId(0),
                    selection.id,
                    if partition {
                        DecisionSelection::LibraryTopPartition {
                            hand: commander,
                            top: None,
                            bottom: vec![],
                        }
                    } else {
                        DecisionSelection::Objects(vec![commander])
                    },
                )
                .unwrap();
                let replacement = game.pending_decision.clone().unwrap();
                assert_eq!(replacement.kind, DecisionKind::CommanderZoneReplacement);
                assert_eq!(replacement.visibility, DecisionVisibility::Private);
                assert_eq!(game.zone_of(commander), Some(Zone::Library));
                game.submit_decision(
                    PlayerId(0),
                    replacement.id,
                    DecisionSelection::Objects(if accept { vec![commander] } else { vec![] }),
                )
                .unwrap();
                assert_eq!(
                    game.zone_of(commander),
                    Some(if accept { Zone::Command } else { Zone::Hand })
                );
                assert_eq!(game.zone_of(spell), Some(Zone::Graveyard));
                assert!(game.format.commander_selected_returns.is_empty());
                game.validate_invariants().unwrap();
            }
        }
    }

    #[test]
    fn warp_commander_replacement_precedes_shuffle_and_preserves_reveal_count() {
        let (mut game, commander, spell) = setup(
            vec![Effect::WarpOwnedPermanentsIntoLibrariesThenRevealAndReturnPermanentCards],
            Zone::Battlefield,
        );
        let revealed = *game.players[1].library.last().unwrap();
        game.cast_spell(
            PlayerId(0),
            CastRequest {
                card: spell,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
        )
        .unwrap();
        game.atomic_transition(|game| game.resolve_top_of_stack())
            .unwrap();
        let decision = game.pending_decision.clone().unwrap();
        assert_eq!(game.zone_of(commander), Some(Zone::Battlefield));
        game.submit_decision(
            PlayerId(1),
            decision.id,
            DecisionSelection::Objects(vec![commander]),
        )
        .unwrap();
        assert_eq!(game.zone_of(commander), Some(Zone::Command));
        assert_eq!(game.zone_of(revealed), Some(Zone::Battlefield));
        assert_eq!(game.zone_of(spell), Some(Zone::Graveyard));
        game.validate_invariants().unwrap();
    }

    #[test]
    fn hand_recycle_offers_fresh_choices_for_the_two_distinct_zone_moves() {
        let template = setup(vec![], Zone::Battlefield).0.catalog["REPLACEMENT-COMMANDER"].clone();
        let mut game = Game::new_with_all_bindings(
            [template],
            2,
            [],
            [],
            [],
            [ActivatedAbilityBinding {
                card_definition: "REPLACEMENT-COMMANDER",
                ability: ActivatedAbility {
                    id: "recycle",
                    mana_cost: ManaCost::new(0),
                    tap_cost: false,
                    sorcery_speed: false,
                    additional_tap_creatures: 0,
                    sacrifice_source: false,
                    sacrifice_creatures: 0,
                    sacrifice_lands: 0,
                    discard_cards: 0,
                    targets: vec![],
                    effects: vec![Effect::PutControllerHandOnLibraryBottomThenDrawSameCount],
                },
            }],
        )
        .unwrap();
        game.configure_commander_format(40, 21).unwrap();
        let source = game
            .put_on_battlefield(PlayerId(0), "REPLACEMENT-COMMANDER")
            .unwrap();
        let commander = game
            .put_on_battlefield(PlayerId(0), "REPLACEMENT-COMMANDER")
            .unwrap();
        game.designate_commander(PlayerId(0), commander).unwrap();
        game.move_to_zone(commander, Zone::Hand).unwrap();
        game.begin_game().unwrap();
        game.activate_ability(
            PlayerId(0),
            AbilityActivation {
                source,
                ability_id: "recycle",
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![],
            },
        )
        .unwrap();
        for _ in 0..2 {
            game.pass_priority(game.priority).unwrap();
        }
        let order = game.pending_decision.clone().unwrap();
        assert_eq!(order.kind, DecisionKind::HandToLibraryBottomDraw);
        game.submit_decision(
            PlayerId(0),
            order.id,
            DecisionSelection::Objects(vec![commander]),
        )
        .unwrap();
        let first = game.pending_decision.clone().unwrap();
        assert_eq!(game.zone_of(commander), Some(Zone::Hand));
        game.submit_decision(PlayerId(0), first.id, DecisionSelection::Objects(vec![]))
            .unwrap();
        let second = game.pending_decision.clone().unwrap();
        assert_eq!(game.zone_of(commander), Some(Zone::Library));
        assert_ne!(first.id, second.id);
        game.submit_decision(
            PlayerId(0),
            second.id,
            DecisionSelection::Objects(vec![commander]),
        )
        .unwrap();
        assert_eq!(game.zone_of(commander), Some(Zone::Command));
        assert!(game.stack.is_empty());
        assert_eq!(
            game.event_log
                .iter()
                .filter(|event| matches!(event, GameEvent::HandPutOnLibraryBottomThenDrawn { .. }))
                .count(),
            1
        );
        game.validate_invariants().unwrap();
    }

    #[test]
    fn conditional_discard_waits_for_all_draws_and_commander_replacement() {
        for accept in [false, true] {
            let (mut game, commander, spell) = setup(
                vec![Effect::DrawTargetPlayerThenConditionalPrivateDiscard],
                Zone::Library,
            );
            let first = *game.players[1].library.last().unwrap();
            game.started = false;
            game.refresh_public_state_integrity();
            let second = game
                .add_card(PlayerId(1), "REPLACEMENT-COMMANDER", Zone::Library)
                .unwrap();
            game.started = true;
            game.refresh_public_state_integrity();
            game.cast_spell(
                PlayerId(0),
                CastRequest {
                    card: spell,
                    targets: vec![Target::Player(PlayerId(1))],
                    convoke: vec![],
                    payment_mana_abilities: vec![],
                },
            )
            .unwrap();
            game.atomic_transition(|game| game.resolve_top_of_stack())
                .unwrap();
            let choice = game.pending_decision.clone().unwrap();
            assert_eq!(choice.kind, DecisionKind::CommanderZoneReplacement);
            assert_eq!(game.players[1].hand.len(), 2);
            game.submit_decision(
                PlayerId(1),
                choice.id,
                DecisionSelection::Objects(if accept { vec![commander] } else { vec![] }),
            )
            .unwrap();
            let discard = game.pending_decision.clone().unwrap();
            assert_eq!(discard.kind, DecisionKind::ConditionalPrivateDiscard);
            assert_eq!(discard.player, PlayerId(1));
            game.submit_decision(
                PlayerId(1),
                discard.id,
                DecisionSelection::Objects(vec![first, second]),
            )
            .unwrap();
            assert_eq!(
                game.zone_of(commander),
                Some(if accept { Zone::Command } else { Zone::Hand })
            );
            assert_eq!(game.zone_of(spell), Some(Zone::Graveyard));
            assert!(!game.players[1].lost);
            game.validate_invariants().unwrap();
        }
    }

    #[test]
    fn paid_library_selection_does_not_refund_or_repeat_life_when_replaced() {
        let (mut game, commander, spell) = setup_owned(
            vec![Effect::LookAtTopCardsChooseForLifeOrGraveyard {
                count: 1,
                life_per_card: 2,
            }],
            Zone::Library,
            PlayerId(0),
        );
        game.cast_spell(
            PlayerId(0),
            CastRequest {
                card: spell,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
        )
        .unwrap();
        game.atomic_transition(|game| game.resolve_top_of_stack())
            .unwrap();
        let look = game.pending_private_library_choice.clone().unwrap();
        game.choose_private_library_cards(PlayerId(0), look.decision, spell, vec![commander])
            .unwrap();
        assert_eq!(game.players[0].life, 38);
        assert_eq!(game.zone_of(commander), Some(Zone::Library));
        let replacement = game.pending_decision.clone().unwrap();
        game.submit_decision(
            PlayerId(0),
            replacement.id,
            DecisionSelection::Objects(vec![commander]),
        )
        .unwrap();
        assert_eq!(game.players[0].life, 38);
        assert_eq!(game.zone_of(commander), Some(Zone::Command));
        assert_eq!(game.zone_of(spell), Some(Zone::Graveyard));
        game.validate_invariants().unwrap();
    }

    #[test]
    fn top_reveal_precedes_replacement_and_life_loss_is_preserved() {
        let (mut game, commander, spell) = setup_owned(
            vec![
                Effect::RevealTopCardPutIntoHandLoseLifeEqualToManaValue,
                Effect::GainLifeController { amount: 2 },
            ],
            Zone::Library,
            PlayerId(0),
        );
        game.catalog
            .get_mut("REPLACEMENT-COMMANDER")
            .unwrap()
            .mana_cost = ManaCost::new(7);
        game.refresh_public_state_integrity();
        game.cast_spell(
            PlayerId(0),
            CastRequest {
                card: spell,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
        )
        .unwrap();
        game.atomic_transition(|game| game.resolve_top_of_stack())
            .unwrap();
        assert!(
            matches!(game.event_log.get(game.event_log.len() - 2), Some(GameEvent::CardRevealed { card, .. }) if *card == commander)
        );
        assert_eq!(game.players[0].life, 40);
        let replacement = game.pending_decision.clone().unwrap();
        game.submit_decision(
            PlayerId(0),
            replacement.id,
            DecisionSelection::Objects(vec![commander]),
        )
        .unwrap();
        assert_eq!(game.players[0].life, 35);
        assert_eq!(game.zone_of(commander), Some(Zone::Command));
        assert_eq!(game.event_log.iter().filter(|event| matches!(event, GameEvent::CardRevealed { card, .. } if *card == commander)).count(), 1);
        game.validate_invariants().unwrap();
    }

    #[test]
    fn sacrifice_draw_continuation_does_not_repeat_the_sacrifice() {
        let (mut game, commander, spell) = setup_owned(
            vec![Effect::TargetPlayerSacrificesCreatureThenControllerDrawsEqualToPower],
            Zone::Library,
            PlayerId(0),
        );
        game.started = false;
        game.refresh_public_state_integrity();
        let sacrificed = game
            .put_on_battlefield(PlayerId(1), "REPLACEMENT-COMMANDER")
            .unwrap();
        let first = game
            .add_card(PlayerId(0), "REPLACEMENT-COMMANDER", Zone::Library)
            .unwrap();
        game.started = true;
        game.refresh_public_state_integrity();
        game.cast_spell(
            PlayerId(0),
            CastRequest {
                card: spell,
                targets: vec![Target::Player(PlayerId(1))],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
        )
        .unwrap();
        game.atomic_transition(|game| game.resolve_top_of_stack())
            .unwrap();
        let sacrifice = game.pending_decision.clone().unwrap();
        game.submit_decision(
            PlayerId(1),
            sacrifice.id,
            DecisionSelection::Objects(vec![sacrificed]),
        )
        .unwrap();
        assert_eq!(game.zone_of(sacrificed), Some(Zone::Graveyard));
        assert_eq!(game.zone_of(first), Some(Zone::Hand));
        let replacement = game.pending_decision.clone().unwrap();
        game.submit_decision(
            PlayerId(0),
            replacement.id,
            DecisionSelection::Objects(vec![commander]),
        )
        .unwrap();
        assert_eq!(game.zone_of(commander), Some(Zone::Command));
        assert_eq!(game.zone_of(spell), Some(Zone::Graveyard));
        assert_eq!(
            game.event_log
                .iter()
                .filter(|event| matches!(event,
            GameEvent::SacrificedByEffect { permanent, .. } if *permanent == sacrificed))
                .count(),
            1
        );
        game.validate_invariants().unwrap();
    }

    #[test]
    fn group_draw_preserves_prior_players_draws_before_collecting_discards() {
        let template = setup(vec![], Zone::Battlefield).0.catalog["REPLACEMENT-COMMANDER"].clone();
        let mut game = Game::new_with_all_bindings(
            [template],
            2,
            [],
            [],
            [],
            [ActivatedAbilityBinding {
                card_definition: "REPLACEMENT-COMMANDER",
                ability: ActivatedAbility {
                    id: "group-draw",
                    mana_cost: ManaCost::new(0),
                    tap_cost: false,
                    sorcery_speed: false,
                    additional_tap_creatures: 0,
                    sacrifice_source: false,
                    sacrifice_creatures: 0,
                    sacrifice_lands: 0,
                    discard_cards: 0,
                    targets: vec![],
                    effects: vec![Effect::EachPlayerDrawsThenDiscardsOneCard],
                },
            }],
        )
        .unwrap();
        game.configure_commander_format(40, 21).unwrap();
        let source = game
            .put_on_battlefield(PlayerId(0), "REPLACEMENT-COMMANDER")
            .unwrap();
        let commander = game
            .put_on_battlefield(PlayerId(1), "REPLACEMENT-COMMANDER")
            .unwrap();
        game.designate_commander(PlayerId(1), commander).unwrap();
        game.move_to_zone(commander, Zone::Library).unwrap();
        let first = game
            .add_card(PlayerId(0), "REPLACEMENT-COMMANDER", Zone::Library)
            .unwrap();
        game.begin_game().unwrap();
        game.activate_ability(
            PlayerId(0),
            AbilityActivation {
                source,
                ability_id: "group-draw",
                sacrifice_sources: vec![],
                additional_tap_creatures: vec![],
                discard_cards: vec![],
                targets: vec![],
            },
        )
        .unwrap();
        for _ in 0..2 {
            game.pass_priority(game.priority).unwrap();
        }
        let replacement = game.pending_decision.clone().unwrap();
        assert_eq!(replacement.player, PlayerId(1));
        assert_eq!(game.zone_of(first), Some(Zone::Hand));
        game.submit_decision(
            PlayerId(1),
            replacement.id,
            DecisionSelection::Objects(vec![commander]),
        )
        .unwrap();
        while let Some(discard) = game.pending_decision.clone() {
            assert_eq!(discard.kind, DecisionKind::TriggeredEffectObject);
            let cards = if discard.player == PlayerId(0) {
                vec![first]
            } else {
                vec![]
            };
            game.submit_decision(
                discard.player,
                discard.id,
                DecisionSelection::Objects(cards),
            )
            .unwrap();
        }
        assert_eq!(game.zone_of(commander), Some(Zone::Command));
        assert_eq!(game.zone_of(first), Some(Zone::Graveyard));
        assert_eq!(
            game.event_log
                .iter()
                .filter(|event| matches!(event,
            GameEvent::CardMoved { card, to: Zone::Hand } if *card == first))
                .count(),
            1
        );
        assert!(game.stack.is_empty());
        game.validate_invariants().unwrap();
    }

    #[test]
    fn unsupported_live_commander_zone_move_fails_atomically_instead_of_skipping_choice() {
        let (mut game, commander, _) = setup(
            vec![Effect::GainLifeController { amount: 1 }],
            Zone::Battlefield,
        );
        let before = game.canonical_event_log();
        assert!(
            game.atomic_transition(|game| game.move_to_zone(commander, Zone::Hand))
                .is_err()
        );
        assert_eq!(game.zone_of(commander), Some(Zone::Battlefield));
        assert_eq!(game.canonical_event_log(), before);
        game.validate_invariants().unwrap();
    }

    #[test]
    fn source_that_left_before_bounce_has_no_replacement_choice() {
        let (mut game, commander, spell) = setup(
            vec![Effect::ReturnOpponentCreatureToHand],
            Zone::Battlefield,
        );
        game.cast_spell(
            PlayerId(0),
            CastRequest {
                card: spell,
                targets: vec![Target::Permanent(commander)],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
        )
        .unwrap();
        game.atomic_transition(|game| game.move_to_zone(commander, Zone::Command))
            .unwrap();
        game.atomic_transition(|game| game.resolve_top_of_stack())
            .unwrap();
        assert!(game.pending_decision.is_none());
        assert_eq!(game.zone_of(commander), Some(Zone::Command));
        assert_eq!(game.zone_of(spell), Some(Zone::Graveyard));
        game.validate_invariants().unwrap();
    }
}
