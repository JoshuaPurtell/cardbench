//! The agent seat.
//!
//! A `ReactPolicy` is an ordinary [`CodePolicy`]. That is the whole trick: the
//! ladder, the archetype matrix, paired seeds, Wilson intervals, the seat
//! split, and contamination reporting were all written against that trait, so
//! seating a model costs no new statistics code.
//!
//! Three behaviours make it affordable and honest.
//!
//! **Triage.** A real match takes about a thousand policy decisions and almost
//! all of them have one legal reply. The model is consulted only when
//! [`Menu::is_open`] -- see `rav-arena probe` for the measured ratio.
//!
//! **Plan following.** Casting is several engine actions (float mana, then
//! cast). The model chooses once and the seat plays out the rest of the
//! sequence itself, so a single cast is one consultation rather than four.
//!
//! **Counted fallback.** When the model errors, returns unreadable text, or
//! runs out of budget, the seat delegates to a code policy rather than passing
//! or playing randomly. That keeps a game finishable, and it would quietly
//! turn a model seat into a v7 seat if it were not counted -- so it is
//! counted, and a run with a high fallback rate is contaminated in exactly the
//! sense `HANDOFF.md` §5 means.

use std::sync::{Arc, Mutex};

use cardbench_magic_engine::{GameView, PlayerId, PolicyAction, Step};
use cardbench_magic_policies::{CodePolicy, planner::CardIndex};

use crate::menu::{self, Menu};
use crate::parse;
use crate::provider::LlmProvider;
use crate::render;

/// What a seat did, as opposed to how it did against the opponent.
///
/// `fallbacks` is the number that decides whether a result means anything. A
/// seat that fell back on most of its open decisions measured the fallback
/// policy wearing the model's name.
#[derive(Clone, Copy, Debug, Default)]
pub struct ReactStats {
    /// Every call the runner made to this seat.
    pub decisions: u32,
    /// Decisions with more than one legal candidate.
    pub open: u32,
    /// Open decisions the model was actually asked about.
    pub consulted: u32,
    /// Actions played out from an already-chosen plan without a consultation.
    pub plan_steps: u32,
    pub parse_failures: u32,
    pub provider_failures: u32,
    /// Open decisions skipped because the per-game consultation budget ran out.
    pub budget_exhausted: u32,
    /// Open decisions answered by the fallback policy for any reason.
    pub fallbacks: u32,
    /// No-priority rules decisions (library searches, triggers, draw
    /// replacements) handled by the fallback. These are never offered to the
    /// model and are reported separately so they cannot inflate `fallbacks`.
    pub delegated: u32,
}

impl ReactStats {
    /// Share of open decisions the model actually decided.
    #[must_use]
    pub fn agency(&self) -> f64 {
        if self.open == 0 {
            return 0.0;
        }
        f64::from(self.open - self.fallbacks) / f64::from(self.open)
    }
}

/// One consultation, kept for review.
#[derive(Clone, Debug)]
pub struct DecisionRecord {
    pub turn: u32,
    pub step: String,
    pub options: Vec<String>,
    pub reply: String,
    pub chosen: Option<usize>,
    pub note: Option<String>,
}

/// How a seat is configured.
#[derive(Clone, Debug)]
pub struct ReactConfig {
    /// Consultations allowed per game. The cost of a run is this number times
    /// the number of games, so it is the dial that decides affordability.
    pub budget: u32,
    /// Keep the full prompt/reply of every consultation.
    pub record: bool,
}

impl Default for ReactConfig {
    fn default() -> Self {
        Self {
            budget: 120,
            record: false,
        }
    }
}

/// A plan the model already chose, being played out.
struct Pending {
    remaining: std::collections::VecDeque<PolicyAction>,
    turn: u32,
    step: Step,
}

pub struct ReactPolicy {
    id: &'static str,
    player: PlayerId,
    index: Arc<CardIndex>,
    provider: Arc<dyn LlmProvider>,
    fallback: Box<dyn CodePolicy>,
    config: ReactConfig,
    consulted: u32,
    pending: Option<Pending>,
    stats: Arc<Mutex<ReactStats>>,
    log: Arc<Mutex<Vec<DecisionRecord>>>,
}

impl ReactPolicy {
    /// Seats a model, with `fallback` answering whatever the model cannot.
    ///
    /// `stats` and `log` are shared rather than owned because the runner hands
    /// the seat to `run_deck_matchup_with` and never gets it back.
    #[must_use]
    pub fn new(
        player: PlayerId,
        index: Arc<CardIndex>,
        provider: Arc<dyn LlmProvider>,
        fallback: Box<dyn CodePolicy>,
        config: ReactConfig,
        stats: Arc<Mutex<ReactStats>>,
        log: Arc<Mutex<Vec<DecisionRecord>>>,
    ) -> Self {
        // The trait requires a `&'static str` and a seat id has to name the
        // model to be worth anything in an event log. A handful of leaked ids
        // per process is the cheaper of the two prices.
        let id: &'static str = Box::leak(format!("react:{}", provider.id()).into_boxed_str());
        Self {
            id,
            player,
            index,
            provider,
            fallback,
            config,
            consulted: 0,
            pending: None,
            stats,
            log,
        }
    }

    fn record(&self, apply: impl FnOnce(&mut ReactStats)) {
        if let Ok(mut stats) = self.stats.lock() {
            apply(&mut stats);
        }
    }

    /// The next action of an in-flight plan, when it is still valid.
    ///
    /// A plan is abandoned the moment the turn or step changes. Mana abilities
    /// do not use the stack and do not pass priority, so a valid sequence
    /// always completes inside one window; anything that survives past the
    /// window boundary is stale and would be an illegal submission.
    fn next_planned(&mut self, view: &GameView) -> Option<PolicyAction> {
        let pending = self.pending.as_mut()?;
        if pending.turn != view.turn || pending.step != view.step {
            self.pending = None;
            return None;
        }
        let action = pending.remaining.pop_front();
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.remaining.is_empty())
        {
            self.pending = None;
        }
        action
    }

    fn commit(&mut self, view: &GameView, plan: Vec<PolicyAction>) -> PolicyAction {
        let mut remaining: std::collections::VecDeque<PolicyAction> = plan.into();
        let first = remaining.pop_front().unwrap_or(PolicyAction::PassPriority);
        if !remaining.is_empty() {
            self.pending = Some(Pending {
                remaining,
                turn: view.turn,
                step: view.step,
            });
        }
        first
    }

    fn fall_back(&mut self, view: &GameView, note: &str) -> PolicyAction {
        self.record(|stats| stats.fallbacks += 1);
        if self.config.record
            && let Ok(mut log) = self.log.lock()
        {
            log.push(DecisionRecord {
                turn: view.turn,
                step: format!("{:?}", view.step),
                options: Vec::new(),
                reply: String::new(),
                chosen: None,
                note: Some(note.to_owned()),
            });
        }
        self.fallback.propose_move(view)
    }

    /// Asks the model and returns the chosen candidate index.
    fn consult(&mut self, view: &GameView, menu: &Menu) -> Result<usize, String> {
        let prompt = render::prompt(view, menu, &self.index);
        let reply = self
            .provider
            .complete(render::SYSTEM_PROMPT, &prompt)
            .map_err(|error| {
                self.record(|stats| stats.provider_failures += 1);
                error.to_string()
            })?;
        let choice = parse::choice(&reply, menu.candidates.len()).map_err(|error| {
            self.record(|stats| stats.parse_failures += 1);
            error.to_string()
        });
        if self.config.record
            && let Ok(mut log) = self.log.lock()
        {
            log.push(DecisionRecord {
                turn: view.turn,
                step: format!("{:?}", view.step),
                options: menu
                    .candidates
                    .iter()
                    .map(|candidate| candidate.label.clone())
                    .collect(),
                reply: reply.clone(),
                chosen: choice.as_ref().ok().map(|choice| choice.index),
                note: choice.as_ref().err().cloned(),
            });
        }
        choice.map(|choice| choice.index)
    }
}

impl CodePolicy for ReactPolicy {
    fn id(&self) -> &'static str {
        self.id
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        self.record(|stats| stats.decisions += 1);

        if let Some(action) = self.next_planned(view) {
            self.record(|stats| stats.plan_steps += 1);
            return action;
        }

        // A view that is not this seat's to act on never reaches the model.
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }

        let menu = menu::build(view, &self.index);
        if !menu.is_open() {
            let plan = menu.only().map_or_else(
                || vec![PolicyAction::PassPriority],
                |candidate| candidate.plan.clone(),
            );
            return self.commit(view, plan);
        }

        self.record(|stats| stats.open += 1);

        if self.consulted >= self.config.budget {
            self.record(|stats| stats.budget_exhausted += 1);
            return self.fall_back(view, "consultation budget exhausted");
        }

        self.consulted += 1;
        self.record(|stats| stats.consulted += 1);
        match self.consult(view, &menu) {
            Ok(index) => {
                let plan = menu.candidates[index].plan.clone();
                self.commit(view, plan)
            }
            Err(note) => self.fall_back(view, &note),
        }
    }

    // Every no-priority rules decision goes to the fallback. These are
    // mandatory bookkeeping (which card a search finds, which trigger target,
    // whether to dredge) rather than the strategic choices the seat exists to
    // measure, and routing them through a menu would multiply the cost of a
    // run without changing what it measures.
    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        let action = self.fallback.propose_pending_decision(view);
        if action.is_some() {
            self.record(|stats| stats.delegated += 1);
        }
        action
    }

    fn propose_optional_triggered_ability(&mut self, view: &GameView) -> PolicyAction {
        self.record(|stats| stats.delegated += 1);
        self.fallback.propose_optional_triggered_ability(view)
    }

    fn propose_draw_replacement(&mut self, view: &GameView) -> PolicyAction {
        self.record(|stats| stats.delegated += 1);
        self.fallback.propose_draw_replacement(view)
    }

    fn propose_private_library_choice(&mut self, view: &GameView) -> PolicyAction {
        self.record(|stats| stats.delegated += 1);
        self.fallback.propose_private_library_choice(view)
    }

    fn propose_private_opponent_library_choice(&mut self, view: &GameView) -> PolicyAction {
        self.record(|stats| stats.delegated += 1);
        self.fallback.propose_private_opponent_library_choice(view)
    }

    fn propose_library_search_choice(&mut self, view: &GameView) -> PolicyAction {
        self.record(|stats| stats.delegated += 1);
        self.fallback.propose_library_search_choice(view)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agency_is_zero_when_nothing_was_ever_open() {
        let stats = ReactStats::default();
        assert!((stats.agency() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn agency_reports_the_share_the_model_actually_decided() {
        // Ten open decisions, two of which the seat answered for the model.
        let stats = ReactStats {
            open: 10,
            fallbacks: 2,
            ..ReactStats::default()
        };
        assert!((stats.agency() - 0.8).abs() < 1e-9);
    }

    #[test]
    fn a_seat_that_fell_back_on_everything_has_no_agency() {
        // This is the shape of a run that looks like a model result and is
        // entirely the fallback policy's.
        let stats = ReactStats {
            open: 7,
            fallbacks: 7,
            ..ReactStats::default()
        };
        assert!((stats.agency() - 0.0).abs() < f64::EPSILON);
    }
}
