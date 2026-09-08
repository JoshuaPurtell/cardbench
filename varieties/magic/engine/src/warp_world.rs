//! Warp World's two simultaneous entry waves and no-priority owner choices.
use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum WarpChoice {
    Life { card: ObjectId, amount: u8 },
    Aura { card: ObjectId },
    Bottom { player: PlayerId },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct PendingWarp {
    waves: [Vec<ObjectId>; 2],
    bottom: Vec<Vec<ObjectId>>,
    order: Vec<PlayerId>,
    wave: usize,
    cursor: usize,
    paid: BTreeSet<ObjectId>,
    attachments: BTreeMap<ObjectId, ObjectId>,
    waiting: Option<WarpChoice>,
}

impl PendingWarp {
    pub(super) fn source_card(&self) -> Option<ObjectId> {
        match self.waiting {
            Some(WarpChoice::Life { card, .. } | WarpChoice::Aura { card }) => Some(card),
            _ => None,
        }
    }
    pub(super) fn new(first: Vec<ObjectId>, second: Vec<ObjectId>, bottom: Vec<Vec<ObjectId>>, order: Vec<PlayerId>) -> Self {
        Self { waves: [first, second], bottom, order, wave: 0, cursor: 0,
            paid: BTreeSet::new(), attachments: BTreeMap::new(), waiting: None }
    }
}

impl Game {
    pub(super) fn resolve_warp_world_instruction(&mut self, top: &StackObject, index: usize) -> Result<bool, RulesError> {
        let key = (top.id, index);
        let mut state = match self.format.warp_world.remove(&key) {
            Some(state) => state,
            None => self.begin_warp_world()?,
        };
        while state.wave < 2 {
            while state.cursor < state.waves[state.wave].len() {
                let card = state.waves[state.wave][state.cursor];
                let player = self.object(card)?.owner;
                self.require_zone(card, Zone::Library)?;
                if let Some(binding) = self.land_entry_behaviors.get(self.card_definition(card)?.id)
                    && let Some(amount) = binding.optional_life_payment
                    && self.players[player.0].life >= i64::from(amount)
                {
                    state.waiting = Some(WarpChoice::Life { card, amount });
                    return self.pause_warp(top, index, state, player, DecisionKind::WarpWorldEntry,
                        0, 1, vec![card]);
                }
                if let Some(binding) = self.attachment_binding_for(card)?
                    && binding.kind == AttachmentKind::Aura
                {
                    let candidates = self.all_battlefield_cards().into_iter().filter(|target|
                        self.attachment_matches_for_source(player, card, Target::Permanent(*target), binding.target)
                    ).collect::<Vec<_>>();
                    if candidates.is_empty() {
                        state.bottom[player.0].push(card);
                        state.waves[state.wave].remove(state.cursor);
                        continue;
                    }
                    state.waiting = Some(WarpChoice::Aura { card });
                    return self.pause_warp(top, index, state, player, DecisionKind::WarpWorldEntry,
                        1, 1, candidates);
                }
                state.cursor += 1;
            }
            let entrants = &state.waves[state.wave];
            let before = self.all_battlefield_cards();
            for card in entrants {
                self.move_to_battlefield_for_simultaneous_entry(*card)?;
                if let Some(binding) = self.land_entry_behaviors.get(self.card_definition(*card)?.id)
                    && (binding.enters_tapped || (binding.optional_life_payment.is_some() && !state.paid.contains(card)))
                {
                    self.objects.get_mut(card).ok_or(RulesError::UnknownCard(*card))?.tapped = true;
                }
            }
            for card in entrants {
                let mut sources = before.clone();
                sources.push(*card);
                self.apply_static_entry_restriction_from_sources(*card, &sources)?;
                if let Some(target) = state.attachments.get(card) {
                    let binding = self.attachment_binding_for(*card)?.ok_or(RulesError::IllegalAction("Warp Aura lost binding"))?;
                    self.attach_with_binding(*card, *target, &binding, false)?;
                }
            }
            if !entrants.is_empty() {
                self.capture_simultaneous_entry_triggers_and_land_entries(entrants)?;
            }
            state.wave += 1;
            state.cursor = 0;
        }
        while state.cursor < state.order.len() {
            let player = state.order[state.cursor];
            let cards = state.bottom[player.0].clone();
            if cards.len() > 1 {
                let count = u8::try_from(cards.len()).map_err(|_| RulesError::IllegalAction("Warp ordering exceeds decision cardinality"))?;
                state.waiting = Some(WarpChoice::Bottom { player });
                return self.pause_warp(top, index, state, player, DecisionKind::WarpWorldBottom, count, count, cards);
            }
            self.warp_bottom(player, &cards)?;
            state.cursor += 1;
        }
        Ok(false)
    }

    #[allow(clippy::too_many_arguments)]
    fn pause_warp(&mut self, top: &StackObject, index: usize, state: PendingWarp, player: PlayerId,
        kind: DecisionKind, min: u8, max: u8, cards: Vec<ObjectId>) -> Result<bool, RulesError> {
        self.format.warp_world.insert((top.id, index), state);
        self.stack.push(top.clone());
        self.stack_effect_cursors.insert(top.id, index);
        self.open_pending_decision(player, DecisionVisibility::Public, kind, min, max,
            cards.into_iter().map(DecisionOption::Object).collect(),
            DecisionContinuation::WarpWorld { stack: top.id, effect_index: index })?;
        Ok(true)
    }

    fn warp_bottom(&mut self, player: PlayerId, cards: &[ObjectId]) -> Result<(), RulesError> {
        for card in cards {
            self.require_zone(*card, Zone::Library)?;
            if self.object(*card)?.owner != player { return Err(RulesError::IllegalAction("Warp bottom owner changed")); }
        }
        let library = &mut self.players[player.0].library;
        library.retain(|card| !cards.contains(card));
        for card in cards { library.insert(0, *card); }
        if !cards.is_empty() {
            self.record_event(GameEvent::WarpWorldBottomOrdered { player, top_to_bottom: cards.to_vec() });
        }
        Ok(())
    }

    pub(super) fn validate_warp_world_decision(&self, decision: &PendingDecision) -> Result<(), RulesError> {
        let DecisionContinuation::WarpWorld { stack, effect_index } = decision.continuation else {
            return Err(RulesError::IllegalAction("not a Warp decision"));
        };
        let state = self.format.warp_world.get(&(stack, effect_index)).ok_or(RulesError::IllegalAction("missing Warp continuation"))?;
        let top = self.stack.last().ok_or(RulesError::IllegalAction("missing Warp stack"))?;
        if top.id != stack || self.stack_effect_cursor(top)? != effect_index
            || !matches!(top.effects.get(effect_index), Some(Effect::WarpOwnedPermanentsIntoLibrariesThenRevealAndReturnPermanentCards))
            || decision.visibility != DecisionVisibility::Public {
            return Err(RulesError::IllegalAction("stale Warp stack boundary"));
        }
        let (player, kind, min, max, cards) = match state.waiting.as_ref() {
            Some(WarpChoice::Life { card, amount }) => {
                self.require_zone(*card, Zone::Library)?;
                let player = self.object(*card)?.owner;
                if state.waves.get(state.wave).and_then(|wave| wave.get(state.cursor)) != Some(card)
                    || self.players[player.0].life < i64::from(*amount)
                    || self.land_entry_behaviors.get(self.card_definition(*card)?.id).and_then(|b| b.optional_life_payment) != Some(*amount) {
                    return Err(RulesError::IllegalAction("stale Warp life choice"));
                }
                (player, DecisionKind::WarpWorldEntry, 0, 1, vec![*card])
            }
            Some(WarpChoice::Aura { card }) => {
                self.require_zone(*card, Zone::Library)?;
                if state.waves.get(state.wave).and_then(|wave| wave.get(state.cursor)) != Some(card) {
                    return Err(RulesError::IllegalAction("stale Warp Aura cursor"));
                }
                let player = self.object(*card)?.owner;
                let binding = self.attachment_binding_for(*card)?.ok_or(RulesError::IllegalAction("missing Warp Aura"))?;
                let cards = self.all_battlefield_cards().into_iter().filter(|target|
                    self.attachment_matches_for_source(player, *card, Target::Permanent(*target), binding.target)
                ).collect();
                (player, DecisionKind::WarpWorldEntry, 1, 1, cards)
            }
            Some(WarpChoice::Bottom { player }) => {
                if state.wave != 2 || state.order.get(state.cursor) != Some(player) {
                    return Err(RulesError::IllegalAction("stale Warp bottom cursor"));
                }
                let cards = state.bottom[player.0].clone();
                for card in &cards { self.require_zone(*card, Zone::Library)?; }
                let count = u8::try_from(cards.len()).map_err(|_| RulesError::IllegalAction("Warp order too large"))?;
                (*player, DecisionKind::WarpWorldBottom, count, count, cards)
            }
            None => return Err(RulesError::IllegalAction("Warp decision has no choice")),
        };
        if decision.player != player || decision.kind != kind || decision.min_selections != min
            || decision.max_selections != max || decision.options != cards.into_iter().map(DecisionOption::Object).collect::<Vec<_>>() {
            return Err(RulesError::IllegalAction("Warp decision projection mismatch"));
        }
        Ok(())
    }

    pub(super) fn resolve_warp_world_decision(&mut self, decision: &PendingDecision, stack: StackObjectId,
        index: usize, selection: DecisionSelection) -> Result<(), RulesError> {
        self.validate_warp_world_decision(decision)?;
        let selected = Self::validate_object_decision_selection(decision, selection)?;
        self.complete_pending_decision(decision)?;
        let mut state = self.format.warp_world.remove(&(stack, index)).expect("validated Warp state");
        match state.waiting.take().expect("validated Warp choice") {
            WarpChoice::Life { card, amount } => if !selected.is_empty() {
                self.adjust_life(decision.player, -i64::from(amount));
                state.paid.insert(card);
            },
            WarpChoice::Aura { card } => { state.attachments.insert(card, selected[0]); },
            WarpChoice::Bottom { player } => self.warp_bottom(player, &selected)?,
        }
        state.cursor += 1;
        self.format.warp_world.insert((stack, index), state);
        self.resolve_top_of_stack_with_optional_decision(None, None, None)
    }
}
