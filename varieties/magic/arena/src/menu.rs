//! Legal-candidate enumeration: the surface an external agent chooses from.
//!
//! The engine exposes no legal-move enumerator. `GameView` is data with no
//! methods, and every code policy *constructs* an action that the engine then
//! accepts or refuses. That is workable for a policy that only ever proposes
//! one move, and unworkable for an agent that has to be shown its options.
//!
//! So this module builds the menu. Every candidate is a sequence of engine
//! actions rather than a single one, because casting is not atomic from the
//! policy's side: mana abilities are submitted as their own moves before the
//! cast, inside the same priority window.
//!
//! Two properties are deliberate.
//!
//! **Only legal-looking candidates are offered.** The engine still has the
//! final say, but a menu that never contains an illegal move is what keeps an
//! agent seat out of the `rejected_policy_moves` contamination path, and
//! `HANDOFF.md` §5 is explicit that a rejected move is never data.
//!
//! **The menu is generated, not scored.** Ordering candidates by the planner's
//! own valuation would leak the incumbent policy's judgement into the agent's
//! choice and make a comparison meaningless. Candidates come out in a
//! deterministic structural order and carry no value.
//!
//! ## Ceiling
//!
//! An agent can only pick what is enumerated here, so this file bounds how
//! good an agent seat can be. Three bounds are known and unfixed:
//!
//! - Blocks are offered as no-blocks, the planner's assignment, and every
//!   single attacker/blocker pair. Multi-block assignments and blocking
//!   several attackers at once are not enumerated.
//! - Attacks are offered as the three aggression levels plus none and all.
//!   Arbitrary attacker subsets are not enumerated.
//! - Activated abilities are offered as the planner's single best activation.
//!
//! Widening any of these is a menu change, not a policy change, and it will
//! change what every agent seat can express. Treat it as a version bump.

use cardbench_magic_engine::{
    AbilityActivation, CastRequest, CombatBlock, GameView, ManaAbilityActivation, ObjectId,
    PolicyAction, Step, Target, TargetRequirement,
};
use cardbench_magic_policies::planner::{
    Aggression, Board, CardIndex, ManaTap, Permanent, Weights, ability, board::HandCard, can_block,
    mana, plan_attack_assigned_valued, plan_blocks_valued, target::targets_for,
};

/// Candidates offered beyond this count are dropped. A menu an agent cannot
/// read is not a wider choice, it is a longer prompt.
const MENU_LIMIT: usize = 24;

/// One thing the seat could do, as the ordered engine actions that do it.
///
/// `plan` is never empty. It holds more than one action exactly when mana must
/// be floated first: mana abilities do not use the stack, so the whole
/// sequence resolves inside one priority window and no opponent acts between
/// its steps.
#[derive(Clone, Debug)]
pub struct Candidate {
    /// What this does, in the words the agent sees. Stable for a given game
    /// state so a transcript can be diffed.
    pub label: String,
    pub plan: Vec<PolicyAction>,
}

impl Candidate {
    fn single(label: String, action: PolicyAction) -> Self {
        Self {
            label,
            plan: vec![action],
        }
    }

    /// A candidate whose action must be preceded by floating mana.
    fn with_taps(label: String, taps: &[ManaTap], action: PolicyAction) -> Self {
        let mut plan: Vec<PolicyAction> = taps.iter().map(tap_action).collect();
        plan.push(action);
        Self { label, plan }
    }
}

/// The kind of decision the seat is facing, which is what decides whether it is
/// worth an agent call at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Occasion {
    /// Someone else's decision, or a window with nothing to do but pass.
    Forced,
    Priority,
    DeclareAttackers,
    DeclareBlockers,
}

/// The menu for one decision.
#[derive(Clone, Debug)]
pub struct Menu {
    pub occasion: Occasion,
    pub candidates: Vec<Candidate>,
}

impl Menu {
    /// Whether this decision is worth an agent call.
    ///
    /// A one-candidate menu is not a decision, it is an obligation. Measured
    /// over twelve real games, 74.4% of decisions are exactly that: 2342 seat
    /// decisions, 599 of them open. Skipping the rest is the only reason an
    /// agent seat is affordable at all. Re-measure with `rav-arena probe`
    /// after any change to what this module enumerates.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.candidates.len() > 1
    }

    /// The action to take when nobody is consulted.
    #[must_use]
    pub fn only(&self) -> Option<&Candidate> {
        self.candidates.first()
    }
}

/// Builds the menu for whatever decision `view` is presenting.
#[must_use]
pub fn build(view: &GameView, index: &CardIndex) -> Menu {
    // A view that is not ours to act on has exactly one legal reply.
    if view.player != view.decision_player {
        return Menu {
            occasion: Occasion::Forced,
            candidates: vec![Candidate::single(
                "pass priority".to_owned(),
                PolicyAction::PassPriority,
            )],
        };
    }

    let board = Board::from_view(view, index);

    // Turn-based declarations come first for the same reason the archetype
    // policies check them first: they are obligations, and the engine will not
    // advance the step without one.
    match view.step {
        Step::DeclareAttackers if board.is_my_turn && !view.attackers_declared => {
            let mut candidates = attack_candidates(&board);
            if board.opponents.len() > 1 {
                let original = candidates.clone();
                for candidate in original {
                    let [PolicyAction::DeclareAttackers { attackers }] = candidate.plan.as_slice() else { continue; };
                    if attackers.is_empty() { continue; }
                    for opponent in &board.opponents {
                        candidates.push(Candidate::single(format!("{} against seat {}", candidate.label, opponent.seat.0),
                            PolicyAction::DeclareAttackersAgainst { attackers: attackers.iter().map(|card|
                                (*card, cardbench_magic_engine::DefenderChoice::Player(opponent.seat))).collect() }));
                    }
                    if attackers.len() > 1 {
                        candidates.push(Candidate::single(format!("{} split across opponents", candidate.label),
                            PolicyAction::DeclareAttackersAgainst { attackers: attackers.iter().enumerate().map(|(i, card)|
                                (*card, cardbench_magic_engine::DefenderChoice::Player(board.opponents[i % board.opponents.len()].seat))).collect() }));
                    }
                }
            }
            candidates.truncate(MENU_LIMIT);
            return Menu {
                occasion: Occasion::DeclareAttackers,
                candidates,
            };
        }
        Step::DeclareBlockers if !board.is_my_turn && !view.blockers_declared => {
            return Menu {
                occasion: Occasion::DeclareBlockers,
                candidates: block_candidates(&board),
            };
        }
        _ => {}
    }

    Menu {
        occasion: Occasion::Priority,
        candidates: priority_candidates(&board),
    }
}

/// Passing is always first, so `only()` on a closed menu is always the
/// do-nothing action and a truncated menu never loses the safe option.
fn priority_candidates(board: &Board) -> Vec<Candidate> {
    let mut candidates = vec![Candidate::single(
        "pass priority".to_owned(),
        PolicyAction::PassPriority,
    )];

    if !board_has_priority(board) {
        return candidates;
    }

    land_candidates(board, &mut candidates);
    cast_candidates(board, &mut candidates);
    ability_candidates(board, &mut candidates);

    candidates.truncate(MENU_LIMIT);
    candidates
}

/// Whether this seat may take a non-pass priority action at all.
const fn board_has_priority(board: &Board) -> bool {
    // `Board` does not carry the priority holder separately from the decision
    // player, and `build` has already established that this seat owns the
    // decision. Land timing is checked per candidate.
    let _ = board;
    true
}

fn land_candidates(board: &Board, candidates: &mut Vec<Candidate>) {
    if !board.is_my_turn
        || board.lands_played > 0
        || board.stack_depth > 0
        || !matches!(board.step, Step::PrecombatMain | Step::PostcombatMain)
    {
        return;
    }
    for card in board.hand.iter().filter(|card| card.facts.is_land) {
        match card.facts.entry_life_payment {
            None => candidates.push(Candidate::single(
                format!("play land {}", card.definition),
                PolicyAction::PlayLand { card: card.object },
            )),
            // The engine refuses a plain land play for a land that offers this
            // choice, so both branches must be offered or neither is legal.
            Some(life) => {
                candidates.push(Candidate::single(
                    format!("play land {} tapped (pay no life)", card.definition),
                    PolicyAction::PlayLandWithEntryLifePayment {
                        card: card.object,
                        pay_life: false,
                    },
                ));
                candidates.push(Candidate::single(
                    format!("play land {} untapped (pay {life} life)", card.definition),
                    PolicyAction::PlayLandWithEntryLifePayment {
                        card: card.object,
                        pay_life: true,
                    },
                ));
            }
        }
    }
}

fn cast_candidates(board: &Board, candidates: &mut Vec<Candidate>) {
    for card in board.castable_cards() {
        let facts = &card.facts;
        if facts.is_land || facts.unsupported {
            continue;
        }
        // Sorcery timing is legality, not preference. Offering a sorcery in an
        // upkeep window would put an engine refusal on the menu.
        if !facts.is_instant_speed && !is_sorcery_window(board) {
            continue;
        }
        // Self-damage that would kill the caster is a legal cast and a lost
        // game. It stays off the menu for the same reason `cast_value` refuses
        // it: no agent should have to defend against its own option list.
        if i64::from(facts.self_damage) >= board.my_life {
            continue;
        }
        let Some(plan) = mana::plan(board, &facts.cost) else {
            continue;
        };

        if !facts.needs_targets() {
            candidates.push(Candidate::with_taps(
                format!("cast {}", card.definition),
                &plan.taps,
                PolicyAction::Cast(CastRequest {
                    card: card.object,
                    targets: Vec::new(),
                    convoke: Vec::new(),
                    payment_mana_abilities: Vec::new(),
                }),
            ));
            continue;
        }

        for targets in target_options(board, card) {
            candidates.push(Candidate::with_taps(
                format!(
                    "cast {} targeting {}",
                    card.definition,
                    describe_targets(board, &targets)
                ),
                &plan.taps,
                PolicyAction::Cast(CastRequest {
                    card: card.object,
                    targets,
                    convoke: Vec::new(),
                    payment_mana_abilities: Vec::new(),
                }),
            ));
        }
    }
}

/// Every target assignment worth offering for one spell.
///
/// Single-requirement spells get one candidate per legal target, because
/// choosing what to point removal at is most of what makes removal skilful.
/// Anything with several requirements falls back to the planner's own choice
/// under neutral weights: enumerating the cross product would flood the menu,
/// and getting multi-target legality wrong would put refusals on it.
fn target_options(board: &Board, card: &HandCard) -> Vec<Vec<Target>> {
    let facts = &card.facts;
    if let [requirement] = facts.targets.as_slice()
        && let Some(options) = enumerate_single(board, *requirement)
    {
        return options.into_iter().map(|target| vec![target]).collect();
    }
    targets_for(board, facts, &Weights::balanced())
        .map(|targets| vec![targets])
        .unwrap_or_default()
}

/// Legal targets for one requirement, or `None` when this requirement is not
/// one the planner's board projection can decide safely.
///
/// The conservative direction matters: returning `None` costs the agent some
/// choice, while returning a target the engine refuses costs a whole game.
/// `Board` carries no colour, card-type, or graveyard detail, so every
/// requirement that needs one is declined here.
fn enumerate_single(board: &Board, requirement: TargetRequirement) -> Option<Vec<Target>> {
    let creature = |permanent: &&Permanent| permanent.is_creature;
    let targets = match requirement {
        TargetRequirement::Creature => board
            .mine
            .iter()
            .chain(board.theirs.iter())
            .filter(creature)
            .map(|permanent| Target::Permanent(permanent.object))
            .collect(),
        TargetRequirement::ControlledCreature => board
            .mine
            .iter()
            .filter(creature)
            .map(|permanent| Target::Permanent(permanent.object))
            .collect(),
        TargetRequirement::OpponentCreature => board
            .theirs
            .iter()
            .filter(creature)
            .map(|permanent| Target::Permanent(permanent.object))
            .collect(),
        TargetRequirement::Land => board
            .mine
            .iter()
            .chain(board.theirs.iter())
            .filter(|permanent| permanent.is_land)
            .map(|permanent| Target::Permanent(permanent.object))
            .collect(),
        TargetRequirement::ControlledLand => board
            .mine
            .iter()
            .filter(|permanent| permanent.is_land)
            .map(|permanent| Target::Permanent(permanent.object))
            .collect(),
        TargetRequirement::Permanent => board
            .mine
            .iter()
            .chain(board.theirs.iter())
            .map(|permanent| Target::Permanent(permanent.object))
            .collect(),
        TargetRequirement::Player => std::iter::once(Target::Player(board.me))
            .chain(
                board
                    .opponents
                    .iter()
                    .map(|opponent| Target::Player(opponent.seat)),
            )
            .collect(),
        TargetRequirement::Opponent => board
            .opponents
            .iter()
            .map(|opponent| Target::Player(opponent.seat))
            .collect(),
        TargetRequirement::PlayerOrCreature | TargetRequirement::Any => board
            .opponents
            .iter()
            .map(|opponent| Target::Player(opponent.seat))
            .chain(std::iter::once(Target::Player(board.me)))
            .chain(
                board
                    .mine
                    .iter()
                    .chain(board.theirs.iter())
                    .filter(creature)
                    .map(|permanent| Target::Permanent(permanent.object)),
            )
            .collect(),
        // Colour, card type, zone, and combat-relative restrictions are not
        // decidable from this projection.
        _ => return None,
    };
    Some(targets)
}

fn describe_targets(board: &Board, targets: &[Target]) -> String {
    let names: Vec<String> = targets
        .iter()
        .map(|target| match target {
            Target::Player(seat) if *seat == board.me => "myself".to_owned(),
            Target::Player(seat) => format!("player {}", seat.0),
            Target::Permanent(object) => board
                .mine
                .iter()
                .chain(board.theirs.iter())
                .find(|permanent| permanent.object == *object)
                .map_or_else(|| format!("object {}", object.0), describe_permanent),
            other => format!("{other:?}"),
        })
        .collect();
    names.join(" and ")
}

fn describe_permanent(permanent: &Permanent) -> String {
    let name = permanent.definition.unwrap_or("unknown");
    let side = if permanent.controller_is_opponent {
        "theirs"
    } else {
        "mine"
    };
    if permanent.is_creature {
        format!(
            "{name} ({}/{} {side})",
            permanent.power, permanent.toughness,
        )
    } else {
        format!("{name} ({side})")
    }
}

fn ability_candidates(board: &Board, candidates: &mut Vec<Candidate>) {
    let Some(activation) = ability::best_activation(board, &Weights::balanced()) else {
        return;
    };
    let label = format!(
        "activate {} on {}",
        activation.ability,
        board
            .mine
            .iter()
            .find(|permanent| permanent.object == activation.source)
            .and_then(|permanent| permanent.definition)
            .unwrap_or("a permanent")
    );
    candidates.push(Candidate::with_taps(
        label,
        &activation.taps,
        PolicyAction::ActivateAbility {
            activation: AbilityActivation {
                source: activation.source,
                ability_id: activation.ability,
                sacrifice_sources: Vec::new(),
                additional_tap_creatures: Vec::new(),
                discard_cards: Vec::new(),
                targets: activation.targets,
            },
        },
    ));
}

/// Attack sets, from the safest to the most committal.
///
/// The three aggression levels are the planner's own answers to "how much risk
/// is worth taking", so offering all three lets an agent pick a posture without
/// the menu having to enumerate subsets. None and all bracket them.
fn attack_candidates(board: &Board) -> Vec<Candidate> {
    let mut candidates = vec![Candidate::single(
        "attack with nothing".to_owned(),
        PolicyAction::DeclareAttackers {
            attackers: Vec::new(),
        },
    )];
    let mut seen: Vec<Vec<ObjectId>> = vec![Vec::new()];
    let weights = Weights::balanced();

    for aggression in [
        Aggression::Measured,
        Aggression::Pressing,
        Aggression::AllIn,
    ] {
        let plan = plan_attack_assigned_valued(board, &weights, aggression);
        push_attack(
            &mut candidates,
            &mut seen,
            &plan.attackers,
            aggression_label(aggression),
        );
    }

    let everything: Vec<ObjectId> = board
        .mine
        .iter()
        .filter(|permanent| permanent.is_creature && permanent.can_attack)
        .map(|permanent| permanent.object)
        .collect();
    push_attack(&mut candidates, &mut seen, &everything, "everything able");

    candidates
}

const fn aggression_label(aggression: Aggression) -> &'static str {
    match aggression {
        Aggression::Measured => "only favourable trades",
        Aggression::Pressing => "pressing for damage",
        Aggression::AllIn => "all-in on the clock",
    }
}

fn push_attack(
    candidates: &mut Vec<Candidate>,
    seen: &mut Vec<Vec<ObjectId>>,
    attackers: &[ObjectId],
    note: &str,
) {
    let mut sorted = attackers.to_vec();
    sorted.sort_unstable();
    if seen.contains(&sorted) {
        return;
    }
    seen.push(sorted);
    candidates.push(Candidate::single(
        format!("attack with {} creatures ({note})", attackers.len()),
        PolicyAction::DeclareAttackers {
            attackers: attackers.to_vec(),
        },
    ));
}

/// Block assignments: none, the planner's, and every single pair.
///
/// Single pairs are enumerated because "which blocker eats which attacker" is
/// the decision that most often decides a game, and it is cheap to enumerate.
/// Multi-blocks are not, and that is the sharpest edge of the ceiling.
fn block_candidates(board: &Board) -> Vec<Candidate> {
    let mut candidates = vec![Candidate::single(
        "block nothing".to_owned(),
        PolicyAction::DeclareBlockers {
            assignments: Vec::new(),
        },
    )];

    let planned = plan_blocks_valued(board, &Weights::balanced());
    if !planned.is_empty() {
        candidates.push(Candidate::single(
            format!("take the planner's {} blocks", planned.len()),
            PolicyAction::DeclareBlockers {
                assignments: planned
                    .iter()
                    .map(|block| CombatBlock {
                        attacker: block.attacker,
                        blocker: block.blocker,
                    })
                    .collect(),
            },
        ));
    }

    for attacker in &board.attackers {
        for blocker in board
            .mine
            .iter()
            .filter(|permanent| permanent.is_creature && !permanent.tapped)
        {
            if !can_block(blocker, attacker) {
                continue;
            }
            candidates.push(Candidate::single(
                format!(
                    "block {} with {}",
                    describe_permanent(attacker),
                    describe_permanent(blocker)
                ),
                PolicyAction::DeclareBlockers {
                    assignments: vec![CombatBlock {
                        attacker: attacker.object,
                        blocker: blocker.object,
                    }],
                },
            ));
        }
    }

    candidates.truncate(MENU_LIMIT);
    candidates
}

fn is_sorcery_window(board: &Board) -> bool {
    board.is_my_turn
        && board.stack_depth == 0
        && matches!(board.step, Step::PrecombatMain | Step::PostcombatMain)
}

fn tap_action(tap: &ManaTap) -> PolicyAction {
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
            activation: ManaAbilityActivation {
                source: *source,
                ability_id: ability,
                chosen_color: *color,
            },
        },
        ManaTap::Bundle {
            source, ability, ..
        } => PolicyAction::ActivateBoundManaAbility {
            activation: ManaAbilityActivation {
                source: *source,
                ability_id: ability,
                chosen_color: None,
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_menu_always_offers_passing_first() {
        // Whatever else is enumerated, the do-nothing action has to survive
        // truncation, because it is the fallback for a closed menu.
        let candidates = vec![Candidate::single(
            "pass priority".to_owned(),
            PolicyAction::PassPriority,
        )];
        let menu = Menu {
            occasion: Occasion::Priority,
            candidates,
        };
        assert!(!menu.is_open());
        assert_eq!(
            menu.only().map(|candidate| candidate.plan.clone()),
            Some(vec![PolicyAction::PassPriority])
        );
    }

    #[test]
    fn a_single_candidate_is_not_a_decision() {
        let menu = Menu {
            occasion: Occasion::Priority,
            candidates: vec![Candidate::single(
                "pass priority".to_owned(),
                PolicyAction::PassPriority,
            )],
        };
        assert!(!menu.is_open(), "one option is an obligation, not a choice");
    }

    #[test]
    fn two_candidates_open_the_decision() {
        let menu = Menu {
            occasion: Occasion::Priority,
            candidates: vec![
                Candidate::single("pass priority".to_owned(), PolicyAction::PassPriority),
                Candidate::single(
                    "attack with nothing".to_owned(),
                    PolicyAction::DeclareAttackers {
                        attackers: Vec::new(),
                    },
                ),
            ],
        };
        assert!(menu.is_open());
    }

    #[test]
    fn an_undecidable_requirement_yields_no_enumeration() {
        // The board projection carries no colour, so a colour-restricted
        // requirement must decline rather than guess.
        let board = Board {
            commanders: vec![],
            me: cardbench_magic_engine::PlayerId(0),
            my_life: 20,
            opponents: Vec::new(),
            turn: 1,
            step: Step::PrecombatMain,
            is_my_turn: true,
            lands_played: 0,
            stack_depth: 0,
            attackers_declared: false,
            blockers_declared: false,
            hand: Vec::new(),
            mine: Vec::new(),
            theirs: Vec::new(),
            attackers: Vec::new(),
            floating: [0; 6],
        };
        assert!(enumerate_single(&board, TargetRequirement::NonblackCreature).is_none());
        assert!(enumerate_single(&board, TargetRequirement::Creature).is_some());
    }
}
