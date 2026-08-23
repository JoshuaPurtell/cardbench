//! Measuring how many decisions are actually decisions.
//!
//! A 38-turn match takes about a thousand policy decisions. If an agent seat
//! had to answer all of them, a single game would cost a thousand model calls
//! and the whole idea would be unaffordable. The claim that makes it work is
//! that the overwhelming majority of those decisions have exactly one legal
//! reply -- and a claim that load-bearing should be measured rather than
//! asserted, so this wraps a code policy, builds the menu it would have shown
//! an agent, counts, and then lets the code policy play normally.
//!
//! The games are therefore ordinary code-versus-code games. Nothing here
//! changes what is played.

use std::sync::{Arc, Mutex};

use cardbench_magic_engine::{GameView, PolicyAction};
use cardbench_magic_policies::{CodePolicy, planner::CardIndex};

use crate::menu;

/// Decision counts for one probed seat.
#[derive(Clone, Copy, Debug, Default)]
pub struct ProbeStats {
    pub decisions: u32,
    /// Decisions whose menu held more than one candidate.
    pub open: u32,
    /// Open decisions, by occasion.
    pub open_priority: u32,
    pub open_attacks: u32,
    pub open_blocks: u32,
    /// The widest menu seen, which bounds prompt size.
    pub widest_menu: usize,
}

impl ProbeStats {
    /// Share of decisions that would have cost a model call.
    #[must_use]
    pub fn open_share(&self) -> f64 {
        if self.decisions == 0 {
            return 0.0;
        }
        f64::from(self.open) / f64::from(self.decisions)
    }
}

/// A code policy that counts the menu it would have offered.
pub struct ProbePolicy {
    inner: Box<dyn CodePolicy>,
    index: Arc<CardIndex>,
    stats: Arc<Mutex<ProbeStats>>,
}

impl ProbePolicy {
    #[must_use]
    pub fn new(
        inner: Box<dyn CodePolicy>,
        index: Arc<CardIndex>,
        stats: Arc<Mutex<ProbeStats>>,
    ) -> Self {
        Self {
            inner,
            index,
            stats,
        }
    }
}

impl CodePolicy for ProbePolicy {
    fn id(&self) -> &'static str {
        self.inner.id()
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        let built = menu::build(view, &self.index);
        if let Ok(mut stats) = self.stats.lock() {
            stats.decisions += 1;
            stats.widest_menu = stats.widest_menu.max(built.candidates.len());
            if built.is_open() {
                stats.open += 1;
                match built.occasion {
                    menu::Occasion::Priority => stats.open_priority += 1,
                    menu::Occasion::DeclareAttackers => stats.open_attacks += 1,
                    menu::Occasion::DeclareBlockers => stats.open_blocks += 1,
                    menu::Occasion::Forced => {}
                }
            }
        }
        self.inner.propose_move(view)
    }

    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        self.inner.propose_pending_decision(view)
    }

    fn propose_optional_triggered_ability(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_optional_triggered_ability(view)
    }

    fn propose_draw_replacement(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_draw_replacement(view)
    }

    fn propose_private_library_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_private_library_choice(view)
    }

    fn propose_private_opponent_library_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_private_opponent_library_choice(view)
    }

    fn propose_library_search_choice(&mut self, view: &GameView) -> PolicyAction {
        self.inner.propose_library_search_choice(view)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_share_of_nothing_is_zero_rather_than_undefined() {
        assert!((ProbeStats::default().open_share() - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn open_share_is_the_fraction_that_would_cost_a_call() {
        let stats = ProbeStats {
            decisions: 1000,
            open: 90,
            ..ProbeStats::default()
        };
        assert!((stats.open_share() - 0.09).abs() < 1e-9);
    }
}
