//! Generation 6 of the archetype policy.
//!
//! Carries everything through v5 and adds instant timing.
//!
//! v1 through v5 cast an instant at the first priority window that could pay
//! for it. On your own turn the first such window is the upkeep -- before your
//! own draw step. Across eight traced games, 141 of 143 spells were cast on the
//! caster's own turn, 18 of those in the upkeep or draw step, and not one at an
//! opponent's end step or in any combat step. Every piece of instant-speed
//! interaction these decks own -- Putrefy, Char, Lightning Helix -- was spent
//! at the worst legal moment: no card seen, no information gained, and unable
//! to answer anything, because nothing had happened yet.
//!
//! The change is timing alone. Which spell is best, what it targets, and what
//! it is worth are all decided exactly as v5 decides them; the only new
//! question is whether *this* window is the right one to spend it in. Holding
//! is safe by construction because the opponent's end step always arrives, and
//! the exceptions enumerated in [`ArchetypePolicyV6::window_is_right`] cover
//! every case where waiting would give something up.
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
pub struct ArchetypePolicyV6 {
    player: PlayerId,
    archetype: Archetype,
    index: Arc<CardIndex>,
    id: &'static str,
}

impl ArchetypePolicyV6 {
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
                Archetype::Aggro => "rav.archetype-aggro.v6",
                Archetype::Midrange => "rav.archetype-midrange.v6",
                Archetype::Burn => "rav.archetype-burn.v6",
                Archetype::Control => "rav.archetype-control.v6",
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

            // Timing. Decided after targeting, because whether now is the right
            // window depends on what the spell ends up pointed at: removal that
            // clears a blocker belongs before combat, the same card pointed at
            // anything else belongs at the opponent's end step.
            if facts.is_instant_speed && !self.window_is_right(board, facts, &targets) {
                continue;
            }

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

    /// Whether an instant should be spent in this window rather than held.
    ///
    /// The default is to hold, because an instant kept in hand is strictly
    /// better than the same instant on the stack: it still answers whatever the
    /// opponent does next, and one more draw step may find something it should
    /// answer instead. The exceptions are the windows where holding gives
    /// something up:
    ///
    /// * **It wins now.** Nothing outranks lethal.
    /// * **Their end step.** The last window before my turn, so holding past it
    ///   buys nothing at all. This is the window v1-v5 never used once.
    /// * **They are attacking.** Removal aimed at a creature that is attacking
    ///   me, or any answer at all when the attack is lethal, has to be spent
    ///   before damage.
    /// * **My precombat main, when it buys damage.** The one case where casting
    ///   on my own turn beats waiting -- and it is asked as a projection, not
    ///   assumed: replay the attack planner on a board without the creature and
    ///   require the damage that gets through to actually go up. A creature the
    ///   defender would never block with is not a blocker, and a card is too
    ///   expensive to spend on removing one.
    ///
    /// Holding cannot strand a card. Every turn contains an opponent end step,
    /// so an instant that is never worth a better window is still cast.
    ///
    /// A non-empty stack is deliberately *not* one of the exceptions. Responding
    /// is what instant speed is for, but [`Board`] exposes only the stack's
    /// depth and not its contents, so "something is on the stack" cannot
    /// distinguish a threat worth answering from a creature spell that will be
    /// a better target once it has resolved. Acting on depth alone spends
    /// removal at random; the end step is one step later and strictly better
    /// informed. Giving `Board` the stack's contents would make a real response
    /// rule possible, and is the obvious next thing this generation wants.
    fn window_is_right(&self, board: &Board, facts: &CardFacts, targets: &[Target]) -> bool {
        if wins_now(board, facts, targets) {
            return true;
        }
        if board.is_my_turn {
            // Upkeep and draw are the windows v1-v5 actually used, and they are
            // dominated by every later one: nothing has happened yet, so there
            // is nothing to respond to and no information to act on.
            return board.step == Step::PrecombatMain
                && self.buys_damage_this_turn(board, facts, targets);
        }
        board.step == Step::End || facing_attack(board, targets)
    }

    /// Whether killing this spell's target increases the damage my attack lands
    /// this turn.
    ///
    /// The same projection idea as v4's land ranking, applied to a removal
    /// spell: build the board that would exist after the spell resolves, ask
    /// the attack planner what it would do there, and compare. A rule that
    /// merely checked "they have an untapped creature I can kill" fires on
    /// almost every board and just moves the cast from my upkeep to my main
    /// phase, which is not the improvement being sought.
    fn buys_damage_this_turn(&self, board: &Board, facts: &CardFacts, targets: &[Target]) -> bool {
        let Some(object) = killed_creature(board, facts, targets) else {
            return false;
        };
        let weights = self.weights();
        let aggression = self.archetype.aggression();
        let mut without = board.clone();
        without
            .theirs
            .retain(|permanent| permanent.object != object);
        crate::planner::planned_damage(&without, &weights, aggression)
            > crate::planner::planned_damage(board, &weights, aggression)
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

/// Whether casting this spell at these targets ends the game immediately.
///
/// Asked of the *final* target list rather than of the card, because a burn
/// archetype redirects reach to the face after targeting: the same Char is
/// lethal or not depending on where it ended up pointed.
fn wins_now(board: &Board, facts: &CardFacts, targets: &[Target]) -> bool {
    facts.damage_to_target > 0
        && targets.iter().any(|target| match target {
            Target::Player(seat) => board.opponents.iter().any(|opponent| {
                opponent.seat == *seat && i64::from(facts.damage_to_target) >= opponent.life
            }),
            _ => false,
        })
}

/// The opposing creature this spell would actually kill, if any.
///
/// `None` when the spell is pointed at a player, at a creature it cannot kill,
/// or at one already tapped and therefore not blocking anything this turn.
fn killed_creature(board: &Board, facts: &CardFacts, targets: &[Target]) -> Option<ObjectId> {
    targets.iter().find_map(|target| {
        let Target::Permanent(object) = target else {
            return None;
        };
        board
            .theirs
            .iter()
            .find(|permanent| {
                permanent.object == *object
                    && permanent.is_creature
                    && permanent.can_block
                    && !permanent.tapped
                    && kills(facts, permanent)
            })
            .map(|permanent| permanent.object)
    })
}

/// Whether this spell must be spent during the opponent's combat.
///
/// Either the attack is lethal -- in which case anything that might stop it is
/// worth more now than in any later window -- or the spell kills a creature
/// that is currently attacking me.
fn facing_attack(board: &Board, targets: &[Target]) -> bool {
    if board.attackers.is_empty() {
        return false;
    }
    let incoming: i32 = board
        .attackers
        .iter()
        .map(|attacker| i32::from(attacker.power.max(0)))
        .sum();
    if i64::from(incoming) >= board.my_life {
        return true;
    }
    targets.iter().any(|target| {
        let Target::Permanent(object) = target else {
            return false;
        };
        board
            .attackers
            .iter()
            .any(|attacker| attacker.object == *object)
    })
}

/// Whether this spell's effect kills the given creature.
///
/// Destruction and exile kill regardless of size; damage has to be at least the
/// creature's toughness. This mirrors the same judgement in the target scorer
/// so the two cannot disagree about what a removal spell does.
fn kills(facts: &CardFacts, permanent: &crate::planner::Permanent) -> bool {
    matches!(facts.role, Role::Removal)
        || (facts.damage_to_target > 0 && i16::from(facts.damage_to_target) >= permanent.toughness)
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

impl CodePolicy for ArchetypePolicyV6 {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::board::{Opponent, Permanent};
    use cardbench_magic_engine::{Keyword, ManaCost, TargetRequirement};

    /// The timing rule reads only archetype weights and aggression, so any
    /// seat will do; midrange is the neutral one.
    fn policy() -> ArchetypePolicyV6 {
        ArchetypePolicyV6::new(
            PlayerId(0),
            Archetype::Midrange,
            crate::deck_match::shared_card_index(),
        )
    }

    fn removal(damage: u8) -> CardFacts {
        CardFacts {
            id: "TEST-REMOVAL",
            cost: ManaCost::with_colors(2, []),
            mana_value: 2,
            is_land: false,
            is_creature: false,
            is_instant_speed: true,
            power: 0,
            toughness: 0,
            keywords: Vec::new(),
            role: if damage > 0 {
                Role::Burn
            } else {
                Role::Removal
            },
            targets: vec![TargetRequirement::PlayerOrCreature],
            damage_to_target: damage,
            self_damage: 0,
            life_gain: 0,
            source: None,
            entry_life_payment: None,
            enters_tapped: false,
            abilities: Vec::new(),
            unsupported: false,
        }
    }

    fn creature(object: u64, power: i16, toughness: i16, opposing: bool) -> Permanent {
        Permanent {
            object: ObjectId(object),
            controller: PlayerId(usize::from(opposing)),
            definition: Some("TEST-BEAR"),
            tapped: false,
            can_attack: true,
            can_block: true,
            summoning_sick: false,
            is_creature: true,
            is_land: false,
            power,
            toughness,
            keywords: Vec::new(),
            source: None,
            role: Role::Creature,
            abilities: Vec::new(),
            controller_is_opponent: opposing,
        }
    }

    fn board(step: Step, is_my_turn: bool) -> Board {
        Board {
            me: PlayerId(0),
            my_life: 20,
            opponents: vec![Opponent {
                seat: PlayerId(1),
                life: 20,
            }],
            turn: 5,
            step,
            is_my_turn,
            lands_played: 0,
            stack_depth: 0,
            attackers_declared: false,
            blockers_declared: false,
            hand: Vec::new(),
            mine: Vec::new(),
            theirs: Vec::new(),
            attackers: Vec::new(),
            floating: [0; 6],
        }
    }

    /// The defect this generation exists to fix. Every instant in these decks
    /// was cast here: the first priority window of the caster's own turn,
    /// before their own draw step, with nothing to respond to.
    #[test]
    fn an_instant_is_not_cast_in_my_own_upkeep() {
        let facts = removal(0);
        let targets = vec![Target::Permanent(ObjectId(9))];
        for step in [Step::Upkeep, Step::Draw] {
            let mut board = board(step, true);
            board.theirs.push(creature(9, 2, 2, true));
            board.mine.push(creature(1, 2, 2, false));
            assert!(
                !policy().window_is_right(&board, &facts, &targets),
                "{step:?} is dominated by every later window"
            );
        }
    }

    /// The window no earlier generation ever used, across three hundred games.
    #[test]
    fn an_instant_is_cast_at_the_opponents_end_step() {
        let facts = removal(0);
        let targets = vec![Target::Permanent(ObjectId(9))];
        let mut board = board(Step::End, false);
        board.theirs.push(creature(9, 2, 2, true));
        assert!(policy().window_is_right(&board, &facts, &targets));
    }

    /// Holding must never beat winning.
    #[test]
    fn lethal_reach_is_taken_immediately() {
        let facts = removal(6);
        let targets = vec![Target::Player(PlayerId(1))];
        let mut board = board(Step::Upkeep, true);
        board.opponents[0].life = 5;
        assert!(
            policy().window_is_right(&board, &facts, &targets),
            "six damage at five life ends the game now"
        );
        board.opponents[0].life = 7;
        assert!(
            !policy().window_is_right(&board, &facts, &targets),
            "the same spell that does not win is still held"
        );
    }

    /// The one case where casting on my own turn beats waiting: the creature
    /// killed would otherwise block, so the removal converts into damage now.
    #[test]
    fn removal_that_clears_a_blocker_is_cast_before_combat() {
        let facts = removal(0);
        let targets = vec![Target::Permanent(ObjectId(9))];
        let mut board = board(Step::PrecombatMain, true);
        // A 2/4 against a 3/3 attacker: the blocker survives, kills nothing,
        // and stops three damage, so the defender takes that block every time
        // and removing it is worth exactly those three damage.
        board.theirs.push(creature(9, 2, 4, true));
        board.mine.push(creature(1, 3, 3, false));
        assert!(policy().window_is_right(&board, &facts, &targets));

        // With nothing able to attack, the tempo is worth nothing and the card
        // is held for a window where it might answer something instead.
        board.mine.clear();
        assert!(!policy().window_is_right(&board, &facts, &targets));

        // A creature that is already tapped was never going to block.
        board.mine.push(creature(1, 3, 3, false));
        board.theirs[0].tapped = true;
        assert!(!policy().window_is_right(&board, &facts, &targets));
    }

    /// The distinction the projection buys over "they have a creature I can
    /// kill": a creature that cannot block what I am attacking with is not
    /// costing me any damage, so killing it before combat is worth nothing this
    /// turn and the card is held for a window where it might matter.
    #[test]
    fn removal_is_held_when_killing_the_creature_buys_no_damage() {
        let facts = removal(0);
        let targets = vec![Target::Permanent(ObjectId(9))];
        let mut board = board(Step::PrecombatMain, true);
        // A ground 3/3 cannot block a flier, so the attack lands the same three
        // damage whether or not the removal is spent.
        board.theirs.push(creature(9, 3, 3, true));
        let mut flier = creature(1, 3, 3, false);
        flier.keywords.push(Keyword::Flying);
        board.mine.push(flier);
        assert!(
            !policy().window_is_right(&board, &facts, &targets),
            "removing a creature that cannot block buys no damage this turn"
        );
    }

    /// Held interaction still has to be spent when the attack it would answer
    /// is actually happening.
    #[test]
    fn an_answer_is_spent_during_the_attack_it_answers() {
        let facts = removal(0);
        let attacker = creature(9, 4, 4, true);
        let targets = vec![Target::Permanent(ObjectId(9))];
        let mut board = board(Step::DeclareBlockers, false);
        board.theirs.push(attacker.clone());
        board.attackers.push(attacker);
        assert!(policy().window_is_right(&board, &facts, &targets));
    }

    /// A lethal attack justifies spending anything, whatever it is aimed at.
    #[test]
    fn a_lethal_attack_spends_the_card_regardless_of_target() {
        let facts = removal(0);
        let targets = vec![Target::Permanent(ObjectId(9))];
        let mut board = board(Step::DeclareBlockers, false);
        board.my_life = 3;
        let mut attacker = creature(7, 5, 5, true);
        attacker.object = ObjectId(7);
        board.attackers.push(attacker);
        assert!(policy().window_is_right(&board, &facts, &targets));
    }

    /// Depth alone is not information. Until `Board` carries the stack's
    /// contents, "something is happening" cannot tell a threat worth answering
    /// from a creature spell that will be a better target once it resolves, and
    /// the end step is one step later and strictly better informed.
    #[test]
    fn a_stack_the_planner_cannot_read_is_not_a_reason_to_act() {
        let facts = removal(0);
        let targets = vec![Target::Permanent(ObjectId(9))];
        let mut board = board(Step::Upkeep, true);
        board.stack_depth = 1;
        assert!(!policy().window_is_right(&board, &facts, &targets));
    }
}
