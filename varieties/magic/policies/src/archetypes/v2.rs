//! Generation 2 of the archetype policy.
//!
//! Change from v1: whole-set attack planning.
//!
//! v1 judged each attacker in isolation, against a hypothetically free best
//! blocker. That is wrong whenever attackers outnumber blockers -- only as many
//! attackers as there are blockers can be blocked, so a creature that looks
//! like a bad attack alone is often free damage in a crowd. Aggro and token
//! decks lose most of their advantage to that mistake.
//!
//! Frozen once measured. Later generations are new files; this one stays
//! runnable so every later claim of improvement has a baseline.
//!
//! One policy, parameterised by archetype, able to pilot any deck in the set.
//!
//! This replaces the deck-to-pilot binding that made deck strength
//! unmeasurable: previously each deck named a hand-written policy, so a
//! measured win rate mixed the deck's quality with its author's effort, and a
//! new deck could not be tested at all without new code.
//!
//! It contains no card-definition identifiers. Every decision routes through
//! the shared planners, weighted by [`Archetype`].

use crate::CodePolicy;
use crate::archetype::Archetype;
use crate::planner::{
    Board, CardIndex, ManaTap, Role, board::CardFacts, cast_value, mana, plan_attack_assigned,
    plan_blocks, targets_for, threat,
};
use cardbench_magic_engine::{
    CastRequest, DecisionKind, DecisionSelection, GameView, ObjectId, PlayerId, PolicyAction, Step,
    Target,
};
use std::sync::Arc;

/// A deck-agnostic policy driven by archetype weights.
#[derive(Clone, Debug)]
pub struct ArchetypePolicyV2 {
    player: PlayerId,
    archetype: Archetype,
    index: Arc<CardIndex>,
    id: &'static str,
}

impl ArchetypePolicyV2 {
    /// Builds a policy for one seat.
    ///
    /// The index is shared because building it per policy per game is a
    /// measurable share of a 2000-game campaign's runtime.
    #[must_use]
    pub fn new(player: PlayerId, archetype: Archetype, index: Arc<CardIndex>) -> Self {
        Self {
            player,
            archetype,
            index,
            id: match archetype {
                Archetype::Aggro => "rav.archetype-aggro.v2",
                Archetype::Midrange => "rav.archetype-midrange.v2",
                Archetype::Burn => "rav.archetype-burn.v2",
                Archetype::Control => "rav.archetype-control.v2",
            },
        }
    }

    fn weights(&self) -> threat::Weights {
        self.archetype.weights()
    }

    /// Turns a planned mana activation into a policy action.
    fn activation(tap: &ManaTap) -> PolicyAction {
        match tap {
            ManaTap::Basic { land, color } => PolicyAction::ActivateManaAbility {
                land: *land,
                color: *color,
            },
            ManaTap::Bound {
                source,
                ability,
                color,
            } => PolicyAction::ActivateBoundManaAbility {
                activation: cardbench_magic_engine::ManaAbilityActivation {
                    source: *source,
                    ability_id: ability,
                    chosen_color: *color,
                },
            },
            ManaTap::Bundle {
                source, ability, ..
            } => PolicyAction::ActivateBoundManaAbility {
                activation: cardbench_magic_engine::ManaAbilityActivation {
                    source: *source,
                    ability_id: ability,
                    chosen_color: None,
                },
            },
        }
    }

    /// Chooses which land to play this turn.
    ///
    /// Prefers a land that adds a colour the hand needs but the board cannot
    /// yet produce; that single rule removes most colour screw.
    #[allow(clippy::unused_self)] // Later generations use `self`; the signature stays stable across the family.
    fn land_to_play(&self, board: &Board) -> Option<ObjectId> {
        let mut produced: [bool; 6] = [false; 6];
        for permanent in &board.mine {
            if let Some(source) = &permanent.source {
                for color in source.colors() {
                    produced[color.index()] = true;
                }
            }
        }
        let mut needed: [u32; 6] = [0; 6];
        for card in board.castable_cards() {
            for color in &card.facts.cost.colored {
                needed[color.index()] += 1;
            }
        }

        board
            .hand
            .iter()
            .filter(|card| card.facts.is_land)
            .max_by_key(|card| {
                let colors = card
                    .facts
                    .source
                    .as_ref()
                    .map(crate::planner::SourceKind::colors)
                    .unwrap_or_default();
                // Rank: fixes a colour I need and lack, then total need served,
                // then how many colours it makes. Object id breaks ties so the
                // choice is deterministic.
                let fixes_gap = colors
                    .iter()
                    .any(|color| needed[color.index()] > 0 && !produced[color.index()]);
                let need_served: u32 = colors
                    .iter()
                    .map(|color| needed[color.index()])
                    .sum::<u32>();
                (
                    u8::from(fixes_gap),
                    need_served,
                    u32::try_from(colors.len()).unwrap_or(0),
                    std::cmp::Reverse(card.object.0),
                )
            })
            .map(|card| card.object)
    }

    /// Plays one land, answering its entry choice when it has one.
    ///
    /// A shockland offers to come in untapped for two life. The engine rejects
    /// an ordinary land play for such a land -- the choice is mandatory, not
    /// optional -- so this is a legality requirement and not only strategy.
    /// Whether to pay is where the archetype's life floor applies: an aggro
    /// deck buys the tempo down to five life, a control deck stops at ten.
    fn play_land(&self, board: &Board, land: ObjectId) -> PolicyAction {
        let payment = board
            .hand
            .iter()
            .find(|card| card.object == land)
            .and_then(|card| card.facts.entry_life_payment);
        match payment {
            None => PolicyAction::PlayLand { card: land },
            Some(life) => {
                // Pay only while it leaves us above the floor, and only when
                // the untapped land can actually be used this turn.
                let affordable = board.my_life - i64::from(life) > self.archetype.life_floor();
                let useful = board.hand.iter().any(|card| {
                    !card.facts.is_land
                        && !card.facts.unsupported
                        && u32::from(card.facts.mana_value) > mana::potential(board)
                });
                PolicyAction::PlayLandWithEntryLifePayment {
                    card: land,
                    pay_life: affordable && useful,
                }
            }
        }
    }

    /// The best spell to cast now, with its payment and targets already solved.
    fn spell_to_cast(&self, board: &Board) -> Option<(ObjectId, CastRequest, Vec<ManaTap>)> {
        let weights = self.weights();
        let mut best: Option<(f32, ObjectId, CastRequest, Vec<ManaTap>)> = None;

        for card in board.castable_cards() {
            let facts = &card.facts;
            if facts.is_land || facts.unsupported {
                continue;
            }
            // Sorcery timing is a legality rule, not a preference: it needs my
            // own main phase *and* an empty stack. Checking only "my turn"
            // submits an illegal cast during upkeep or combat.
            if !facts.is_instant_speed && !is_sorcery_window(board) {
                continue;
            }
            if facts.mana_value > self.archetype.curve_ceiling() && !board.is_my_turn {
                continue;
            }
            let Some(plan) = mana::plan(board, &facts.cost) else {
                continue;
            };
            let Some(value) = cast_value(board, facts, &weights) else {
                continue;
            };
            if value <= 0.0 {
                continue;
            }
            // One card having no legal target must skip that card, not abandon
            // the whole hand scan.
            let targets = if facts.needs_targets() {
                let Some(targets) = targets_for(board, facts, &weights) else {
                    continue;
                };
                targets
            } else {
                Vec::new()
            };
            let targets = self.retarget_burn(board, facts, targets);

            // Prefer the highest value, then the cheapest, then the lowest
            // object id. Determinism is a hard requirement: the campaign
            // compares replay digests.
            let key = value - f32::from(facts.mana_value) * 0.01;
            if best
                .as_ref()
                .is_none_or(|(current, _, _, _)| key > *current)
            {
                best = Some((
                    key,
                    card.object,
                    CastRequest {
                        card: card.object,
                        targets,
                        convoke: Vec::new(),
                        payment_mana_abilities: Vec::new(),
                    },
                    plan.taps,
                ));
            }
        }
        best.map(|(_, object, request, taps)| (object, request, taps))
    }

    /// A burn archetype points reach at the face unless a creature is an
    /// immediate problem it can actually kill.
    fn retarget_burn(&self, board: &Board, facts: &CardFacts, targets: Vec<Target>) -> Vec<Target> {
        if !self.archetype.burn_goes_face() || facts.role != Role::Burn {
            return targets;
        }
        let Some(opponent) = board.primary_opponent() else {
            return targets;
        };
        // Still kill something that is racing us faster than we are racing it.
        if threat::their_clock(board) > threat::my_clock(board) && board.my_life <= 8 {
            return targets;
        }
        targets
            .into_iter()
            .map(|target| match target {
                Target::Permanent(_) => Target::Player(opponent.seat),
                other => other,
            })
            .collect()
    }

    /// Answers a mandatory typed decision.
    ///
    /// These are rules obligations, not strategy. The shared conservative
    /// completion already covers every decision kind the engine exposes and is
    /// exercised by the whole-catalog gauntlet, so this improves only the one
    /// case where the planners have a real opinion -- aiming a triggered
    /// ability -- and delegates the rest rather than growing a second, less
    /// tested vocabulary.
    fn decide(&self, view: &GameView, board: &Board) -> Option<PolicyAction> {
        let decision = view.pending_decision.as_ref()?;
        if decision.kind == DecisionKind::TriggeredAbilityTargets
            && !decision.target_candidates.is_empty()
        {
            return Some(PolicyAction::SubmitDecision {
                decision: decision.id,
                selection: DecisionSelection::Targets(pick_targets(
                    decision,
                    board,
                    &self.weights(),
                )),
            });
        }
        crate::conservative_pending_decision(view)
    }
}

/// Whether sorcery-speed spells are legal for me right now.
fn is_sorcery_window(board: &Board) -> bool {
    board.is_my_turn
        && board.stack_depth == 0
        && matches!(board.step, Step::PrecombatMain | Step::PostcombatMain)
}

fn pick_targets(
    decision: &cardbench_magic_engine::PendingDecisionView,
    board: &Board,
    weights: &threat::Weights,
) -> Vec<Target> {
    let mut candidates = decision.target_candidates.clone();
    // Prefer an opposing creature, then an opponent, then anything, so a
    // mandatory choice never aims a harmful effect at our own board by
    // accident of candidate ordering.
    candidates.sort_by(|left, right| {
        target_rank(*right, board, weights)
            .total_cmp(&target_rank(*left, board, weights))
            .then(format!("{left:?}").cmp(&format!("{right:?}")))
    });
    candidates.truncate(usize::from(decision.min_selections).max(1));
    candidates
}

fn target_rank(target: Target, board: &Board, weights: &threat::Weights) -> f32 {
    match target {
        Target::Player(seat) if seat != board.me => 5.0,
        Target::Permanent(object) => board
            .theirs
            .iter()
            .find(|permanent| permanent.object == object)
            .map_or(-1.0, |permanent| threat::creature_value(permanent, weights)),
        _ => 0.0,
    }
}

impl CodePolicy for ArchetypePolicyV2 {
    fn id(&self) -> &'static str {
        self.id
    }

    fn propose_move(&mut self, view: &GameView) -> PolicyAction {
        if view.player != self.player || view.decision_player != self.player {
            return PolicyAction::PassPriority;
        }
        let board = Board::from_view(view, &self.index);
        let weights = self.weights();

        // Turn-based declarations come first: they are obligations, not
        // options, and the engine will not advance without them.
        match view.step {
            Step::DeclareAttackers if board.is_my_turn && !view.attackers_declared => {
                let plan = plan_attack_assigned(&board, &weights, self.archetype.aggression());
                return PolicyAction::DeclareAttackers {
                    attackers: plan.attackers,
                };
            }
            Step::DeclareBlockers if !board.is_my_turn && !view.blockers_declared => {
                let blocks = plan_blocks(&board, &weights);
                return PolicyAction::DeclareBlockers {
                    assignments: blocks
                        .into_iter()
                        .map(|block| cardbench_magic_engine::CombatBlock {
                            attacker: block.attacker,
                            blocker: block.blocker,
                        })
                        .collect(),
                };
            }
            _ => {}
        }

        if let Some(action) = self.decide(view, &board) {
            return action;
        }

        if view.priority != self.player {
            return PolicyAction::PassPriority;
        }

        // Land drop before spells: it is free and it raises the ceiling for
        // everything considered below.
        if board.is_my_turn
            && board.lands_played == 0
            && matches!(view.step, Step::PrecombatMain | Step::PostcombatMain)
            && view.stack_depth == 0
            && let Some(land) = self.land_to_play(&board)
        {
            return self.play_land(&board, land);
        }

        if let Some((_, request, taps)) = self.spell_to_cast(&board) {
            // Float the mana the plan calls for, one activation per move, then
            // cast. Mana abilities do not use the stack, so this stays inside
            // one priority window.
            if let Some(tap) = taps.first() {
                return Self::activation(tap);
            }
            return PolicyAction::Cast(request);
        }

        PolicyAction::PassPriority
    }

    fn propose_pending_decision(&mut self, view: &GameView) -> Option<PolicyAction> {
        let board = Board::from_view(view, &self.index);
        self.decide(view, &board)
    }
}
