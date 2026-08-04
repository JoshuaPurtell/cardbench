//! Generation 5 of the archetype policy.
//!
//! Carries v2's whole-set attack planning, v3's valued blocking, and v4's land
//! sequencing, and adds activated abilities.
//!
//! Nine of the thirty-eight distinct cards across the constructed decks carry
//! a stack-using activated ability, and no generation before this one ever
//! activated any of them: a repeatable damage source was a vanilla body, a
//! token engine made no tokens, a tapper tapped nothing. Six of the nine cost
//! only mana or a tap and are handled here; the three whose cost is a
//! sacrifice are reported unsupported rather than silently skipped.
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
    Board, CardIndex, ManaTap, Role, ability, board::CardFacts, cast_value, mana,
    plan_attack_assigned, plan_blocks_valued, targets_for, threat,
};
use cardbench_magic_engine::{
    CastRequest, DecisionKind, DecisionSelection, GameView, ObjectId, PlayerId, PolicyAction, Step,
    Target,
};
use std::sync::Arc;

/// A deck-agnostic policy driven by archetype weights.
#[derive(Clone, Debug)]
pub struct ArchetypePolicyV5 {
    player: PlayerId,
    archetype: Archetype,
    index: Arc<CardIndex>,
    id: &'static str,
}

impl ArchetypePolicyV5 {
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
                Archetype::Aggro => "rav.archetype-aggro.v5",
                Archetype::Midrange => "rav.archetype-midrange.v5",
                Archetype::Burn => "rav.archetype-burn.v5",
                Archetype::Control => "rav.archetype-control.v5",
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
    /// Ranked by what the land actually buys: the value of the best spell it
    /// makes castable *this* turn, then the colour gaps it closes for the rest
    /// of the hand. A land that enters tapped contributes nothing to the first
    /// term, which is what stops a karoo -- two colours and therefore the top
    /// of a coverage ranking -- from being played on turn two and bouncing a
    /// land for the privilege.
    #[allow(clippy::cast_possible_truncation)] // Bounded, rounded fixed-point ranking key.
    fn land_to_play(&self, board: &Board) -> Option<ObjectId> {
        let weights = self.weights();
        let mut produced: [bool; 6] = [false; 6];
        for permanent in &board.mine {
            if let Some(source) = &permanent.source {
                for color in source.colors() {
                    produced[color.index()] = true;
                }
            }
        }
        let mut needed: [u32; 6] = [0; 6];
        for card in &board.hand {
            for color in &card.facts.cost.colored {
                needed[color.index()] += 1;
            }
        }

        let mut best: Option<(i64, u32, u32, u64, ObjectId)> = None;
        for card in board.hand.iter().filter(|card| card.facts.is_land) {
            let colors = card
                .facts
                .source
                .as_ref()
                .map(crate::planner::SourceKind::colors)
                .unwrap_or_default();

            // What this land unlocks this turn. A tapped land unlocks nothing,
            // and a karoo also hands back a land, so its immediate value is
            // negative rather than merely zero.
            let immediate = if card.facts.enters_tapped {
                -1_i64
            } else {
                let projected = project_land(board, card.object, &colors);
                best_castable_value(&projected, &weights)
                    // Fixed point so the ranking key is exactly ordered. The
                    // value range here is single digits, so the conversion is
                    // exact in practice and clamped regardless.
                    .map_or(0, |value| (f64::from(value) * 100.0).round() as i64)
            };

            let fixes_gap = u32::from(
                colors
                    .iter()
                    .any(|color| needed[color.index()] > 0 && !produced[color.index()]),
            );
            let need_served: u32 = colors.iter().map(|color| needed[color.index()]).sum();
            let key = (
                immediate,
                fixes_gap,
                need_served,
                u64::try_from(colors.len()).unwrap_or(0),
                card.object,
            );
            if best.as_ref().is_none_or(|current| {
                (key.0, key.1, key.2, key.3, std::cmp::Reverse(key.4))
                    > (
                        current.0,
                        current.1,
                        current.2,
                        current.3,
                        std::cmp::Reverse(current.4),
                    )
            }) {
                best = Some(key);
            }
        }
        best.map(|(_, _, _, _, object)| object)
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
    fn spell_to_cast(&self, board: &Board) -> Option<(f32, CastRequest, Vec<ManaTap>)> {
        let weights = self.weights();
        let mut best: Option<(f32, ObjectId, CastRequest, Vec<ManaTap>)> = None;

        for card in &board.hand {
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
        best.map(|(value, _, request, taps)| (value, request, taps))
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

/// A copy of the board with one extra untapped land already in play.
///
/// Used to ask "what would this land let me cast?" without mutating anything.
fn project_land(board: &Board, land: ObjectId, colors: &[cardbench_magic_engine::Color]) -> Board {
    let mut projected = board.clone();
    projected.hand.retain(|card| card.object != land);
    projected.mine.push(crate::planner::Permanent {
        object: land,
        controller: board.me,
        definition: None,
        tapped: false,
        can_attack: false,
        can_block: false,
        summoning_sick: false,
        is_creature: false,
        is_land: true,
        power: 0,
        toughness: 0,
        keywords: Vec::new(),
        source: Some(crate::planner::SourceKind::BasicTyped(colors.to_vec())),
        role: Role::Land,
        abilities: Vec::new(),
        controller_is_opponent: false,
    });
    projected
}

/// The value of the best spell castable on a projected board.
fn best_castable_value(board: &Board, weights: &threat::Weights) -> Option<f32> {
    board
        .hand
        .iter()
        .filter(|card| !card.facts.is_land && !card.facts.unsupported)
        .filter(|card| mana::plan(board, &card.facts.cost).is_some())
        .filter_map(|card| cast_value(board, &card.facts, weights))
        .filter(|value| *value > 0.0)
        .max_by(f32::total_cmp)
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

impl CodePolicy for ArchetypePolicyV5 {
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
                let blocks = plan_blocks_valued(&board, &weights);
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

        // Abilities and spells compete for the same mana, so they are scored
        // on one scale and the better line wins. Checking abilities first
        // would spend mana on a ping that a creature wanted.
        let activation = ability::best_activation(&board, &weights);
        let best_cast = self.spell_to_cast(&board);
        let cast_value_now = best_cast.as_ref().map(|(value, _, _)| *value);
        if let Some(activation) = activation
            && ability::beats_casting(&activation, cast_value_now)
        {
            if let Some(tap) = activation.taps.first() {
                return Self::activation(tap);
            }
            return PolicyAction::ActivateAbility {
                activation: cardbench_magic_engine::AbilityActivation {
                    source: activation.source,
                    ability_id: activation.ability,
                    sacrifice_sources: Vec::new(),
                    additional_tap_creatures: Vec::new(),
                    discard_cards: Vec::new(),
                    targets: activation.targets,
                },
            };
        }

        if let Some((_, request, taps)) = best_cast {
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
