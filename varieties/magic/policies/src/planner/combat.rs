//! Attack and block planning.
//!
//! The policies this replaces attacked with every creature that could attack
//! and blocked with nothing. In a creature format that is close to the worst
//! possible play: it donates creatures on offence and takes free damage on
//! defence, which is why reference mirrors ran to turn 60 instead of turn 12.
//!
//! Everything here is one-ply. A real lookahead belongs in a later milestone;
//! what is missing is not depth, it is the trade arithmetic.

use super::board::{Board, Permanent};
use super::threat::{Weights, creature_value, is_evasive, my_clock, their_clock, turns_to_kill};
use cardbench_magic_engine::{Keyword, ObjectId};

/// How willing a plan is to lose material for tempo.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Aggression {
    /// Attack only when the trade is favourable or free.
    Measured,
    /// Attack when the trade is roughly even; damage has independent value.
    Pressing,
    /// Attack with everything that advances a lethal clock.
    AllIn,
}

/// Whether `blocker` is legally able to block `attacker`.
///
/// Deliberately conservative: when a restriction cannot be evaluated from the
/// view, the pair is treated as unblockable so the planner never *counts on* a
/// block the engine would refuse.
#[must_use]
pub fn can_block(blocker: &Permanent, attacker: &Permanent) -> bool {
    // The engine's own live eligibility flag is the primary gate. It already
    // accounts for granted restrictions the printed card does not carry.
    if !blocker.can_block {
        return false;
    }
    for keyword in &blocker.keywords {
        // A conditional restriction the view cannot evaluate: the flag is a
        // necessary condition, not a sufficient one, so decline rather than
        // propose a block the engine may refuse.
        if matches!(keyword, Keyword::CannotBlockUnlessControlsMountain) {
            return false;
        }
    }
    for keyword in &attacker.keywords {
        match keyword {
            Keyword::Flying => {
                if !blocker.has(&Keyword::Flying) && !blocker.has(&Keyword::Reach) {
                    return false;
                }
            }
            // Evasion this planner cannot satisfy from the view. Treating the
            // pair as unblockable is the safe direction: it never counts on a
            // block the engine would refuse.
            Keyword::Unblockable
            | Keyword::Fear
            | Keyword::BlackEvasion
            | Keyword::Landwalk(_)
            | Keyword::Mountainwalk => return false,
            _ => {}
        }
    }
    true
}

/// What happens when `attacker` and `blocker` fight.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Exchange {
    pub attacker_dies: bool,
    pub blocker_dies: bool,
}

/// Resolves one attacker against one blocker.
///
/// First and double strike are handled because they change who dies, which is
/// the entire point of the calculation.
#[must_use]
pub fn exchange(attacker: &Permanent, blocker: &Permanent) -> Exchange {
    let attacker_first =
        attacker.has(&Keyword::FirstStrike) || attacker.has(&Keyword::DoubleStrike);
    let blocker_first = blocker.has(&Keyword::FirstStrike) || blocker.has(&Keyword::DoubleStrike);
    let attacker_deathtouch = attacker.has(&Keyword::Deathtouch);
    let blocker_deathtouch = blocker.has(&Keyword::Deathtouch);

    let kills = |power: i16, deathtouch: bool, toughness: i16| {
        (deathtouch && power > 0) || power >= toughness
    };
    let attacker_kills = kills(attacker.power, attacker_deathtouch, blocker.toughness);
    let blocker_kills = kills(blocker.power, blocker_deathtouch, attacker.toughness);

    // A first striker that kills its opponent never takes damage back.
    if attacker_first && !blocker_first && attacker_kills {
        return Exchange {
            attacker_dies: false,
            blocker_dies: true,
        };
    }
    if blocker_first && !attacker_first && blocker_kills {
        return Exchange {
            attacker_dies: true,
            blocker_dies: false,
        };
    }
    Exchange {
        attacker_dies: blocker_kills,
        blocker_dies: attacker_kills,
    }
}

/// The attack declaration this planner recommends.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AttackPlan {
    pub attackers: Vec<ObjectId>,
    /// True when this attack is lethal against the primary opponent even if
    /// every available blocker blocks optimally.
    pub is_lethal: bool,
}

/// Chooses attackers.
///
/// The model is deliberately pessimistic: every attacker is assumed to be
/// blocked by the single best available blocker. That understates damage
/// against a board with fewer blockers than attackers, which is the safe
/// direction to be wrong in -- it never attacks into a trade it would lose.
#[must_use]
pub fn plan_attack(board: &Board, weights: &Weights, aggression: Aggression) -> AttackPlan {
    let Some(opponent) = board.primary_opponent() else {
        return AttackPlan::default();
    };
    let candidates: Vec<&Permanent> = board
        .mine
        .iter()
        .filter(|permanent| permanent.can_attack && permanent.power > 0)
        .collect();
    if candidates.is_empty() {
        return AttackPlan::default();
    }
    let blockers: Vec<&Permanent> = board
        .their_creatures()
        .filter(|permanent| !permanent.tapped)
        .collect();

    // Lethal check first: if swinging with everything wins through the worst
    // case, nothing else matters.
    let unblockable_damage: i32 = candidates
        .iter()
        .filter(|attacker| !blockers.iter().any(|blocker| can_block(blocker, attacker)))
        .map(|attacker| i32::from(attacker.power))
        .sum();
    let all_damage: i32 = candidates
        .iter()
        .map(|attacker| i32::from(attacker.power))
        .sum();
    let worst_case = worst_case_damage(&candidates, &blockers);
    if i64::from(worst_case) >= opponent.life {
        return AttackPlan {
            attackers: candidates
                .iter()
                .map(|permanent| permanent.object)
                .collect(),
            is_lethal: true,
        };
    }

    // Otherwise attack with creatures whose attack is individually justified.
    let racing = {
        let mine = turns_to_kill(my_clock(board), opponent.life);
        let theirs = turns_to_kill(their_clock(board), board.my_life);
        match (mine, theirs) {
            (Some(mine), Some(theirs)) => mine <= theirs,
            (Some(_), None) => true,
            _ => false,
        }
    };
    let keep_blockers = !racing && their_clock(board) > 0;

    let mut attackers = Vec::new();
    for attacker in &candidates {
        let best_block = blockers
            .iter()
            .filter(|blocker| can_block(blocker, attacker))
            .map(|blocker| (exchange(attacker, blocker), *blocker))
            .min_by(|(left, _), (right, _)| {
                // The defender picks the block that is worst for me.
                block_preference(*left).total_cmp(&block_preference(*right))
            });

        let attack_is_good = match best_block {
            // Unblockable: free damage, always correct.
            None => true,
            Some((exchange, blocker)) => {
                let my_loss = if exchange.attacker_dies {
                    creature_value(attacker, weights)
                } else {
                    0.0
                };
                let their_loss = if exchange.blocker_dies {
                    creature_value(blocker, weights)
                } else {
                    0.0
                };
                let margin = match aggression {
                    Aggression::Measured => 0.0,
                    // Damage has value even in an even trade.
                    Aggression::Pressing => creature_value(attacker, weights) * 0.35,
                    Aggression::AllIn => f32::INFINITY,
                };
                their_loss + margin >= my_loss
            }
        };

        // Hold back a creature that is needed on defence, unless it is evasive
        // (its damage is hard to stop) or we are already racing.
        let needed_at_home =
            keep_blockers && !is_evasive(attacker) && !attacker.has(&Keyword::Vigilance);
        if attack_is_good && (!needed_at_home || aggression == Aggression::AllIn) {
            attackers.push(attacker.object);
        }
    }

    // Never attack into a board that simply eats the whole team for nothing.
    if attackers.is_empty() && unblockable_damage > 0 {
        attackers = candidates
            .iter()
            .filter(|attacker| !blockers.iter().any(|blocker| can_block(blocker, attacker)))
            .map(|permanent| permanent.object)
            .collect();
    }
    let _ = all_damage;
    AttackPlan {
        attackers,
        is_lethal: false,
    }
}

/// Damage the recommended attack lands against the defence the defender would
/// actually mount.
///
/// Exists so a caller can ask what a board change is worth in damage rather
/// than guessing. Removing a blocker, for instance, is only worth spending a
/// card on when the attack that follows is actually bigger.
///
/// Deliberately the *valued* defence rather than the worst case. Under the
/// worst-case model every untapped creature absorbs one attacker whatever the
/// exchange costs, so any blocker removal appears to buy its attacker's full
/// power -- a 1/1 in front of a 4/4 would look like four damage saved, when in
/// fact the defender declines that block and takes the four either way. That
/// model is the right pessimism for *deciding* to attack and the wrong one for
/// pricing a removal spell.
#[must_use]
pub fn planned_damage(board: &Board, weights: &Weights, aggression: Aggression) -> i32 {
    let plan = plan_attack_assigned(board, weights, aggression);
    if plan.attackers.is_empty() {
        return 0;
    }
    let attackers: Vec<&Permanent> = board
        .mine
        .iter()
        .filter(|permanent| plan.attackers.contains(&permanent.object))
        .collect();
    let blockers: Vec<&Permanent> = board
        .their_creatures()
        .filter(|permanent| !permanent.tapped)
        .collect();
    let life = board
        .primary_opponent()
        .map_or(20, |opponent| opponent.life);
    let blocks = simulate_defence_valued(&attackers, &blockers, weights, life);
    let mut blocked = vec![None; attackers.len()];
    for block in &blocks {
        blocked[block.attacker] = Some(block.blocker);
    }
    attackers
        .iter()
        .enumerate()
        .map(|(index, attacker)| match blocked[index] {
            None => i32::from(attacker.power.max(0)),
            // A blocked attacker still connects for its trample excess.
            Some(slot) if attacker.has(&Keyword::Trample) => {
                i32::from((attacker.power - blockers[slot].toughness).max(0))
            }
            Some(_) => 0,
        })
        .sum()
}

/// How much a defender dislikes one exchange. Lower is better for the defender.
fn block_preference(exchange: Exchange) -> f32 {
    f32::from(u8::from(exchange.blocker_dies)) - f32::from(u8::from(exchange.attacker_dies))
}

/// Damage that gets through if the defender blocks as well as it possibly can.
fn worst_case_damage(attackers: &[&Permanent], blockers: &[&Permanent]) -> i32 {
    // One blocker per attacker is the engine's current limit, so the defender's
    // best assignment is a matching that absorbs the largest attackers.
    let mut absorbed: Vec<bool> = vec![false; attackers.len()];
    let mut used: Vec<bool> = vec![false; blockers.len()];
    let mut order: Vec<usize> = (0..attackers.len()).collect();
    order.sort_by_key(|index| -attackers[*index].power);
    for index in order {
        let attacker = attackers[index];
        // Trample still connects for the excess, so it is never fully absorbed.
        if attacker.has(&Keyword::Trample) {
            continue;
        }
        if let Some(slot) = blockers
            .iter()
            .enumerate()
            .position(|(slot, blocker)| !used[slot] && can_block(blocker, attacker))
        {
            used[slot] = true;
            absorbed[index] = true;
        }
    }
    attackers
        .iter()
        .enumerate()
        .filter(|(index, _)| !absorbed[*index])
        .map(|(_, attacker)| i32::from(attacker.power))
        .sum()
}

/// One blocker assigned to one attacker.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Block {
    pub attacker: ObjectId,
    pub blocker: ObjectId,
}

/// Chooses blocks.
///
/// Blocks in three tiers: mandatory chump blocks when otherwise dead, then
/// value trades, then free blocks that kill without loss.
#[must_use]
pub fn plan_blocks(board: &Board, weights: &Weights) -> Vec<Block> {
    let attackers: Vec<&Permanent> = board.attackers.iter().collect();
    if attackers.is_empty() {
        return Vec::new();
    }
    let mut available: Vec<&Permanent> = board
        .my_creatures()
        .filter(|permanent| !permanent.tapped)
        .collect();
    available.sort_by_key(|permanent| permanent.object.0);

    let incoming: i32 = attackers
        .iter()
        .map(|attacker| i32::from(attacker.power.max(0)))
        .sum();
    let lethal = i64::from(incoming) >= board.my_life;

    let mut blocks = Vec::new();
    let mut used: Vec<bool> = vec![false; available.len()];
    // Largest attackers first: they are the ones that must be answered.
    let mut order: Vec<usize> = (0..attackers.len()).collect();
    order.sort_by_key(|index| (-attackers[*index].power, attackers[*index].object.0));

    for index in order {
        let attacker = attackers[index];
        let mut best: Option<(f32, usize)> = None;
        for (slot, blocker) in available.iter().enumerate() {
            if used[slot] || !can_block(blocker, attacker) {
                continue;
            }
            let result = exchange(attacker, blocker);
            let my_loss = if result.blocker_dies {
                creature_value(blocker, weights)
            } else {
                0.0
            };
            let their_loss = if result.attacker_dies {
                creature_value(attacker, weights)
            } else {
                0.0
            };
            let damage_prevented = f32::from(attacker.power.max(0));
            let mut gain = their_loss - my_loss;
            if lethal {
                // Survival dominates material: a chump block that stops the
                // game ending is correct however bad the trade looks.
                gain += damage_prevented * 10.0;
            }
            if gain > 0.0 && best.is_none_or(|(score, _)| gain > score) {
                best = Some((gain, slot));
            }
        }
        if let Some((_, slot)) = best {
            used[slot] = true;
            blocks.push(Block {
                attacker: attacker.object,
                blocker: available[slot].object,
            });
        }
    }
    blocks.sort_by_key(|block| (block.attacker.0, block.blocker.0));
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::board::{Opponent, Role};
    use cardbench_magic_engine::{PlayerId, Step};

    fn creature(object: u64, power: i16, toughness: i16, keywords: &[Keyword]) -> Permanent {
        Permanent {
            object: ObjectId(object),
            controller: PlayerId(0),
            definition: Some("C"),
            tapped: false,
            can_attack: true,
            can_block: true,
            summoning_sick: false,
            is_creature: true,
            is_land: false,
            power,
            toughness,
            keywords: keywords.to_vec(),
            source: None,
            role: Role::Creature,
            abilities: Vec::new(),
            nonmana_activated_abilities_suppressed: false,
            controller_is_opponent: false,
        }
    }

    fn board(mine: Vec<Permanent>, theirs: Vec<Permanent>, my_life: i64, their_life: i64) -> Board {
        Board {
            me: PlayerId(0),
            my_life,
            opponents: vec![Opponent {
                seat: PlayerId(1),
                life: their_life,
            }],
            turn: 5,
            step: Step::DeclareAttackers,
            is_my_turn: true,
            lands_played: 1,
            stack_depth: 0,
            attackers_declared: false,
            blockers_declared: false,
            hand: Vec::new(),
            attackers: Vec::new(),
            mine,
            theirs,
            floating: [0; 6],
        }
    }

    #[test]
    fn an_unopposed_creature_always_attacks() {
        let board = board(vec![creature(1, 2, 2, &[])], Vec::new(), 20, 20);
        let plan = plan_attack(&board, &Weights::balanced(), Aggression::Measured);
        assert_eq!(plan.attackers, vec![ObjectId(1)]);
    }

    /// The behaviour the old policies got wrong: a 2/2 must not run into a 3/3.
    #[test]
    fn a_losing_attack_is_declined() {
        let board = board(
            vec![creature(1, 2, 2, &[])],
            vec![creature(2, 3, 3, &[])],
            20,
            20,
        );
        let plan = plan_attack(&board, &Weights::balanced(), Aggression::Measured);
        assert!(
            plan.attackers.is_empty(),
            "a 2/2 must not attack into an untapped 3/3"
        );
    }

    #[test]
    fn a_winning_attack_is_taken() {
        let board = board(
            vec![creature(1, 4, 4, &[])],
            vec![creature(2, 2, 2, &[])],
            20,
            20,
        );
        let plan = plan_attack(&board, &Weights::balanced(), Aggression::Measured);
        assert_eq!(plan.attackers, vec![ObjectId(1)]);
    }

    #[test]
    fn a_flier_ignores_a_ground_blocker() {
        let board = board(
            vec![creature(1, 2, 2, &[Keyword::Flying])],
            vec![creature(2, 5, 5, &[])],
            20,
            20,
        );
        let plan = plan_attack(&board, &Weights::balanced(), Aggression::Measured);
        assert_eq!(plan.attackers, vec![ObjectId(1)]);
    }

    #[test]
    fn lethal_is_taken_even_when_every_attacker_would_die() {
        let board = board(
            vec![creature(1, 3, 1, &[]), creature(2, 3, 1, &[])],
            Vec::new(),
            20,
            6,
        );
        let plan = plan_attack(&board, &Weights::balanced(), Aggression::Measured);
        assert!(plan.is_lethal);
        assert_eq!(plan.attackers.len(), 2);
    }

    #[test]
    fn a_blocked_board_is_not_mistaken_for_lethal() {
        let board = board(
            vec![creature(1, 3, 1, &[]), creature(2, 3, 1, &[])],
            vec![creature(3, 1, 4, &[]), creature(4, 1, 4, &[])],
            20,
            6,
        );
        let plan = plan_attack(&board, &Weights::balanced(), Aggression::Measured);
        assert!(
            !plan.is_lethal,
            "two blockers absorb both attackers, so this is not lethal"
        );
    }

    #[test]
    fn first_strike_wins_an_otherwise_even_exchange() {
        let attacker = creature(1, 2, 2, &[Keyword::FirstStrike]);
        let blocker = creature(2, 2, 2, &[]);
        assert_eq!(
            exchange(&attacker, &blocker),
            Exchange {
                attacker_dies: false,
                blocker_dies: true
            }
        );
    }

    #[test]
    fn deathtouch_kills_regardless_of_power() {
        let attacker = creature(1, 1, 1, &[Keyword::Deathtouch]);
        let blocker = creature(2, 9, 9, &[]);
        let result = exchange(&attacker, &blocker);
        assert!(result.blocker_dies);
        assert!(result.attacker_dies);
    }

    #[test]
    fn a_free_block_that_kills_is_taken() {
        let mut board = board(vec![creature(1, 4, 4, &[])], Vec::new(), 20, 20);
        board.attackers = vec![creature(9, 2, 2, &[])];
        board.step = Step::DeclareBlockers;
        board.is_my_turn = false;
        let blocks = plan_blocks(&board, &Weights::balanced());
        assert_eq!(
            blocks,
            vec![Block {
                attacker: ObjectId(9),
                blocker: ObjectId(1)
            }]
        );
    }

    /// The behaviour the old policies got wrong in the other direction: they
    /// blocked with nothing and took every point.
    #[test]
    fn a_chump_block_is_taken_when_the_alternative_is_losing() {
        let mut board = board(vec![creature(1, 0, 1, &[])], Vec::new(), 3, 20);
        board.attackers = vec![creature(9, 5, 5, &[])];
        board.step = Step::DeclareBlockers;
        board.is_my_turn = false;
        let blocks = plan_blocks(&board, &Weights::balanced());
        assert_eq!(blocks.len(), 1, "dying is worse than losing a 0/1");
    }

    #[test]
    fn a_bad_block_is_declined_when_life_is_safe() {
        let mut board = board(vec![creature(1, 1, 1, &[])], Vec::new(), 20, 20);
        board.attackers = vec![creature(9, 5, 5, &[])];
        board.step = Step::DeclareBlockers;
        board.is_my_turn = false;
        let blocks = plan_blocks(&board, &Weights::balanced());
        assert!(blocks.is_empty(), "do not donate a creature for 5 life");
    }

    /// The exact failure v2 exists to fix: three attackers against one blocker.
    /// Only one can be blocked, so the other two are free damage -- but v1
    /// judges each attacker as though the blocker were free for it and attacks
    /// with none of them.
    #[test]
    fn v2_attacks_when_attackers_outnumber_blockers() {
        // The opponent is at eight, so four damage is half their remaining
        // life. v1 never looks at the opponent's life in this decision at all.
        let board = board(
            vec![
                creature(1, 2, 2, &[]),
                creature(2, 2, 2, &[]),
                creature(3, 2, 2, &[]),
            ],
            vec![creature(9, 3, 3, &[])],
            20,
            8,
        );
        let weights = crate::archetype::Archetype::Aggro.weights();

        let v1 = plan_attack(&board, &weights, Aggression::Pressing);
        assert!(
            v1.attackers.is_empty(),
            "v1 is expected to decline all three; this pins the baseline"
        );

        let v2 = plan_attack_assigned(&board, &weights, Aggression::Pressing);
        assert!(
            v2.attackers.len() >= 2,
            "at most one attacker can be blocked, so at least two are free damage: {:?}",
            v2.attackers
        );
    }

    /// The improvement must not become recklessness: a lone small creature
    /// still must not run into a bigger one.
    #[test]
    fn v2_still_declines_a_purely_losing_attack() {
        let board = board(
            vec![creature(1, 2, 2, &[])],
            vec![creature(9, 5, 5, &[])],
            20,
            20,
        );
        let plan = plan_attack_assigned(&board, &Weights::balanced(), Aggression::Measured);
        assert!(plan.attackers.is_empty());
    }

    #[test]
    fn v2_takes_lethal() {
        let board = board(
            vec![creature(1, 3, 1, &[]), creature(2, 3, 1, &[])],
            Vec::new(),
            20,
            6,
        );
        let plan = plan_attack_assigned(&board, &Weights::balanced(), Aggression::Measured);
        assert!(plan.is_lethal);
        assert_eq!(plan.attackers.len(), 2);
    }

    #[test]
    fn v2_is_deterministic() {
        let build = || {
            board(
                vec![
                    creature(3, 2, 2, &[]),
                    creature(1, 4, 1, &[]),
                    creature(2, 1, 5, &[]),
                ],
                vec![creature(9, 3, 3, &[]), creature(8, 2, 2, &[])],
                20,
                20,
            )
        };
        let first = plan_attack_assigned(&build(), &Weights::balanced(), Aggression::Pressing);
        for _ in 0..8 {
            assert_eq!(
                plan_attack_assigned(&build(), &Weights::balanced(), Aggression::Pressing),
                first
            );
        }
    }

    /// A defender staring at lethal blocks everything it can, including
    /// blocks that lose material.
    #[test]
    fn the_simulated_defence_chump_blocks_against_lethal() {
        let attackers = [creature(1, 5, 5, &[])];
        let blockers = [creature(9, 0, 1, &[])];
        let attacker_refs: Vec<&Permanent> = attackers.iter().collect();
        let blocker_refs: Vec<&Permanent> = blockers.iter().collect();
        let safe = simulate_defence(&attacker_refs, &blocker_refs, &Weights::balanced(), 20);
        assert!(safe.is_empty(), "a comfortable defender declines a chump");
        let desperate = simulate_defence(&attacker_refs, &blocker_refs, &Weights::balanced(), 4);
        assert_eq!(desperate.len(), 1, "a defender facing lethal must chump");
    }

    /// The defect v3 exists to fix: a wall that neither kills nor dies has a
    /// material delta of exactly zero, so v1/v2 decline the block and take the
    /// damage every single turn.
    #[test]
    fn v3_blocks_with_a_wall_that_neither_kills_nor_dies() {
        let mut board = board(vec![creature(1, 2, 5, &[])], Vec::new(), 20, 20);
        board.attackers = vec![creature(9, 3, 3, &[])];
        board.step = Step::DeclareBlockers;
        board.is_my_turn = false;
        let weights = Weights::balanced();

        assert!(
            plan_blocks(&board, &weights).is_empty(),
            "v1/v2 decline this block; this pins the baseline"
        );
        assert_eq!(
            plan_blocks_valued(&board, &weights),
            vec![Block {
                attacker: ObjectId(9),
                blocker: ObjectId(1)
            }],
            "a free block that stops three damage is worth making"
        );
    }

    /// v8 must predict the valued block that v3 made the real defender take;
    /// the frozen attack model incorrectly sees three free damage here.
    #[test]
    fn v8_attack_planner_respects_a_valued_wall() {
        let board = board(
            vec![creature(1, 3, 3, &[])],
            vec![creature(2, 2, 4, &[])],
            20,
            20,
        );
        assert_eq!(
            plan_attack_assigned(&board, &Weights::balanced(), Aggression::Measured).attackers,
            vec![ObjectId(1)],
            "the v7 model treats the wall as declining"
        );
        assert!(
            plan_attack_assigned_valued(&board, &Weights::balanced(), Aggression::Measured)
                .attackers
                .is_empty(),
            "v8 must account for the damage-prevention value of the wall"
        );
    }

    /// Valuing prevention must not turn into throwing creatures away.
    #[test]
    fn v3_still_declines_a_donation() {
        let mut board = board(vec![creature(1, 1, 1, &[])], Vec::new(), 20, 20);
        board.attackers = vec![creature(9, 5, 5, &[])];
        board.step = Step::DeclareBlockers;
        board.is_my_turn = false;
        assert!(
            plan_blocks_valued(&board, &Weights::balanced()).is_empty(),
            "a 1/1 must not chump a 5/5 at twenty life"
        );
    }

    #[test]
    fn v3_chump_blocks_against_lethal() {
        let mut board = board(vec![creature(1, 0, 1, &[])], Vec::new(), 3, 20);
        board.attackers = vec![creature(9, 5, 5, &[])];
        board.step = Step::DeclareBlockers;
        board.is_my_turn = false;
        assert_eq!(plan_blocks_valued(&board, &Weights::balanced()).len(), 1);
    }

    /// One blocker cannot be spent twice, and which use wins depends on life.
    ///
    /// At a healthy life total, eating the small attacker permanently is worth
    /// more than absorbing four damage once. Low on life, that inverts -- and
    /// getting the inversion right is the whole reason prevention is priced as
    /// a fraction of the remaining life total rather than as a constant.
    #[test]
    fn v3_weighs_prevention_against_material_by_life_total() {
        let build = |my_life| {
            let mut board = board(vec![creature(1, 2, 5, &[])], Vec::new(), my_life, 20);
            board.attackers = vec![creature(8, 1, 1, &[]), creature(9, 4, 4, &[])];
            board.step = Step::DeclareBlockers;
            board.is_my_turn = false;
            board
        };
        let weights = Weights::balanced();

        let healthy = plan_blocks_valued(&build(20), &weights);
        assert_eq!(healthy.len(), 1, "only one blocker is available");
        assert_eq!(
            healthy[0].attacker,
            ObjectId(8),
            "at twenty life, killing the small attacker outright is worth more"
        );

        let desperate = plan_blocks_valued(&build(6), &weights);
        assert_eq!(desperate.len(), 1);
        assert_eq!(
            desperate[0].attacker,
            ObjectId(9),
            "at six life, stopping four damage is worth more than a 1/1"
        );
    }

    #[test]
    fn v3_blocking_is_deterministic() {
        let build = || {
            let mut board = board(
                vec![creature(1, 2, 5, &[]), creature(2, 3, 3, &[])],
                Vec::new(),
                20,
                20,
            );
            board.attackers = vec![creature(9, 3, 3, &[]), creature(8, 2, 2, &[])];
            board.step = Step::DeclareBlockers;
            board.is_my_turn = false;
            board
        };
        let first = plan_blocks_valued(&build(), &Weights::balanced());
        for _ in 0..8 {
            assert_eq!(plan_blocks_valued(&build(), &Weights::balanced()), first);
        }
    }

    #[test]
    fn blocking_is_deterministic() {
        let mut board = board(
            vec![creature(1, 2, 2, &[]), creature(2, 2, 2, &[])],
            Vec::new(),
            20,
            20,
        );
        board.attackers = vec![creature(9, 2, 2, &[]), creature(8, 2, 2, &[])];
        board.step = Step::DeclareBlockers;
        let first = plan_blocks(&board, &Weights::balanced());
        for _ in 0..8 {
            assert_eq!(plan_blocks(&board, &Weights::balanced()), first);
        }
    }
}

// ---------------------------------------------------------------------------
// Generation 2 combat: global assignment instead of per-attacker independence.
// ---------------------------------------------------------------------------

/// One blocker's assignment in a simulated defence.
#[derive(Clone, Copy, Debug)]
struct SimBlock {
    attacker: usize,
    blocker: usize,
}

/// Simulates a defender that blocks the way [`plan_blocks_valued`] does.
///
/// [`simulate_defence`] prices a block purely as material -- the attacker's
/// value if it dies, minus the blocker's if it does. That is the v1 blocking
/// model, and v3 replaced it in the *real* defender precisely because it is
/// wrong: a 2/4 in front of a 3/3 kills nothing and loses nothing, so material
/// scores the block at zero and declines it, and three damage goes through
/// every turn.
///
/// The consequence is that since v3 the attack planner has predicted its
/// opponent with a model the codebase itself stopped using. It expects free
/// damage from attackers a valued defender will in fact block. This is the same
/// simulation with damage prevented counted, so the attacker and the defender
/// share one theory of what a block is worth.
///
/// Kept as a separate function rather than a fix in place: every measured
/// generation calls the old one, and editing it would silently rewrite the
/// baseline that every ladder number was measured against.
fn simulate_defence_valued(
    attackers: &[&Permanent],
    blockers: &[&Permanent],
    weights: &Weights,
    defender_life: i64,
) -> Vec<SimBlock> {
    let incoming: i32 = attackers
        .iter()
        .map(|attacker| i32::from(attacker.power.max(0)))
        .sum();
    let facing_lethal = i64::from(incoming) >= defender_life;

    let mut pairs: Vec<(f32, usize, usize)> = Vec::new();
    for (blocker_index, blocker) in blockers.iter().enumerate() {
        for (attacker_index, attacker) in attackers.iter().enumerate() {
            if !can_block(blocker, attacker) {
                continue;
            }
            let result = exchange(attacker, blocker);
            let their_loss = if result.attacker_dies {
                creature_value(attacker, weights)
            } else {
                0.0
            };
            let my_loss = if result.blocker_dies {
                creature_value(blocker, weights)
            } else {
                0.0
            };
            // Trample still gets the excess through, so only the absorbed part
            // counts as prevented. Identical to the real defender's arithmetic.
            let stopped = if attacker.has(&Keyword::Trample) {
                i32::from(blocker.toughness.max(0)).min(i32::from(attacker.power.max(0)))
            } else {
                i32::from(attacker.power.max(0))
            };
            let mut gain =
                their_loss - my_loss + damage_value(stopped, defender_life, weights.own_life);
            if facing_lethal {
                gain += f32::from(attacker.power.max(0)) * 10.0;
            }
            pairs.push((gain, attacker_index, blocker_index));
        }
    }
    pairs.sort_by(|left, right| {
        right
            .0
            .total_cmp(&left.0)
            .then(left.1.cmp(&right.1))
            .then(left.2.cmp(&right.2))
    });

    let mut used_attacker = vec![false; attackers.len()];
    let mut used_blocker = vec![false; blockers.len()];
    let mut blocks = Vec::new();
    for (gain, attacker, blocker) in pairs {
        if used_attacker[attacker] || used_blocker[blocker] || gain <= 0.0 {
            continue;
        }
        used_attacker[attacker] = true;
        used_blocker[blocker] = true;
        blocks.push(SimBlock { attacker, blocker });
    }
    blocks
}

/// Simulates how a rational defender blocks one attack.
///
/// The engine allows at most one blocker per attacker, so this is a matching
/// problem. Pairs are ranked by how much the *defender* gains and taken
/// greedily, each attacker and blocker used at most once. Greedy rather than
/// optimal because the difference is small at realistic board sizes and
/// determinism matters more than the last fraction of accuracy.
fn simulate_defence(
    attackers: &[&Permanent],
    blockers: &[&Permanent],
    weights: &Weights,
    defender_life: i64,
) -> Vec<SimBlock> {
    let incoming: i32 = attackers
        .iter()
        .map(|attacker| i32::from(attacker.power.max(0)))
        .sum();
    let facing_lethal = i64::from(incoming) >= defender_life;

    let mut pairs: Vec<(f32, usize, usize)> = Vec::new();
    for (blocker_index, blocker) in blockers.iter().enumerate() {
        for (attacker_index, attacker) in attackers.iter().enumerate() {
            if !can_block(blocker, attacker) {
                continue;
            }
            let result = exchange(attacker, blocker);
            let gain = if result.attacker_dies {
                creature_value(attacker, weights)
            } else {
                0.0
            } - if result.blocker_dies {
                creature_value(blocker, weights)
            } else {
                0.0
            } + if facing_lethal {
                // Survival dominates material.
                f32::from(attacker.power.max(0)) * 10.0
            } else {
                0.0
            };
            pairs.push((gain, attacker_index, blocker_index));
        }
    }
    // Deterministic ordering: gain descending, then stable index order.
    pairs.sort_by(|left, right| {
        right
            .0
            .total_cmp(&left.0)
            .then(left.1.cmp(&right.1))
            .then(left.2.cmp(&right.2))
    });

    let mut used_attacker = vec![false; attackers.len()];
    let mut used_blocker = vec![false; blockers.len()];
    let mut blocks = Vec::new();
    for (gain, attacker, blocker) in pairs {
        if used_attacker[attacker] || used_blocker[blocker] {
            continue;
        }
        // A defender with life to spare declines a block that only loses
        // material.
        if gain <= 0.0 && !facing_lethal {
            continue;
        }
        used_attacker[attacker] = true;
        used_blocker[blocker] = true;
        blocks.push(SimBlock { attacker, blocker });
    }
    blocks
}

/// What a whole life total is worth, in the same units as creature value.
///
/// Damage and material have to share a scale or one of them dominates by
/// accident. A creature is worth a few points; emptying an opponent's life
/// total wins the game outright, so it is worth several creatures.
const LIFE_TOTAL_VALUE: f32 = 24.0;

/// Value of dealing `damage` to a player who has `life` remaining.
///
/// Proportional to the fraction of their remaining life removed, so the same
/// three points matter far more at six life than at twenty.
pub(crate) fn damage_value(damage: i32, life: i64, weight: f32) -> f32 {
    if damage <= 0 {
        return 0.0;
    }
    let remaining = f32::from(i16::try_from(life.max(1)).unwrap_or(i16::MAX));
    let dealt = f32::from(i16::try_from(damage).unwrap_or(i16::MAX));
    dealt / remaining * LIFE_TOTAL_VALUE * weight
}

/// What one attack set is worth, net of the defence it invites.
fn score_attack(
    attackers: &[&Permanent],
    blockers: &[&Permanent],
    weights: &Weights,
    defender_life: i64,
) -> f32 {
    score_attack_with_defence(attackers, blockers, weights, defender_life, false)
}

/// Scores an attack against the defence model used by the real blocker
/// planner. This is deliberately separate from [`score_attack`]: generations
/// v1-v7 are frozen measurements and changing their simulated opponent would
/// silently rewrite every historical rung.
fn score_attack_valued(
    attackers: &[&Permanent],
    blockers: &[&Permanent],
    weights: &Weights,
    defender_life: i64,
) -> f32 {
    score_attack_with_defence(attackers, blockers, weights, defender_life, true)
}

fn score_attack_with_defence(
    attackers: &[&Permanent],
    blockers: &[&Permanent],
    weights: &Weights,
    defender_life: i64,
    valued_defence: bool,
) -> f32 {
    let blocks = if valued_defence {
        simulate_defence_valued(attackers, blockers, weights, defender_life)
    } else {
        simulate_defence(attackers, blockers, weights, defender_life)
    };
    let mut assignment = vec![None; attackers.len()];
    for block in &blocks {
        assignment[block.attacker] = Some(block.blocker);
    }

    let mut damage = 0_i32;
    let mut my_loss = 0.0_f32;
    let mut their_loss = 0.0_f32;
    for (index, attacker) in attackers.iter().enumerate() {
        match assignment[index] {
            None => damage += i32::from(attacker.power.max(0)),
            Some(blocker_index) => {
                let blocker = blockers[blocker_index];
                let result = exchange(attacker, blocker);
                if result.attacker_dies {
                    my_loss += creature_value(attacker, weights);
                }
                if result.blocker_dies {
                    their_loss += creature_value(blocker, weights);
                }
                if attacker.has(&Keyword::Trample) {
                    damage += i32::from((attacker.power - blocker.toughness).max(0));
                }
            }
        }
    }
    if i64::from(damage) >= defender_life {
        return f32::INFINITY;
    }
    damage_value(damage, defender_life, weights.opponent_life) + their_loss - my_loss
}

/// Generation 2 attack planning.
///
/// The v1 planner asked, for each creature independently, "would attacking
/// with this one be good if the best blocker were free to block it?" That is
/// wrong whenever attackers outnumber blockers: only as many attackers as there
/// are blockers can actually be blocked, so a creature that looks like a bad
/// attack in isolation is often free damage in a crowd. Aggro and token decks
/// lose most of their advantage to that mistake.
///
/// This version scores whole attack sets against a simulated defence and grows
/// the set greedily while the score improves.
#[must_use]
#[allow(clippy::too_many_lines)] // One ordered selection routine reads better than three helpers.
pub fn plan_attack_assigned(
    board: &Board,
    weights: &Weights,
    aggression: Aggression,
) -> AttackPlan {
    plan_attack_assigned_with_defence(board, weights, aggression, false)
}

/// Generation 8 attack planning: predict the valued defender introduced in
/// v3, rather than the material-only defender used by the frozen generations.
///
/// The distinction matters for a wall such as a 2/4 facing a 3/3. The real
/// defender blocks to prevent three damage even when neither creature dies;
/// an attacker that planned against the old material-only model treated that
/// block as free damage. This function is a new entry point so old rungs stay
/// reproducible.
#[must_use]
pub fn plan_attack_assigned_valued(
    board: &Board,
    weights: &Weights,
    aggression: Aggression,
) -> AttackPlan {
    plan_attack_assigned_with_defence(board, weights, aggression, true)
}

#[allow(clippy::too_many_lines)] // One ordered selection routine reads better than three helpers.
fn plan_attack_assigned_with_defence(
    board: &Board,
    weights: &Weights,
    aggression: Aggression,
    valued_defence: bool,
) -> AttackPlan {
    let Some(opponent) = board.primary_opponent() else {
        return AttackPlan::default();
    };
    let candidates: Vec<&Permanent> = board
        .mine
        .iter()
        .filter(|permanent| permanent.can_attack && permanent.power > 0)
        .collect();
    if candidates.is_empty() {
        return AttackPlan::default();
    }
    let blockers: Vec<&Permanent> = board
        .their_creatures()
        .filter(|permanent| !permanent.tapped)
        .collect();
    let attack_score = if valued_defence {
        score_attack_valued
    } else {
        score_attack
    };

    // Lethal first: an attack that wins needs no other justification.
    let all: Vec<&Permanent> = candidates.clone();
    if attack_score(&all, &blockers, weights, opponent.life).is_infinite() {
        return AttackPlan {
            attackers: all.iter().map(|permanent| permanent.object).collect(),
            is_lethal: true,
        };
    }

    // What attacking costs defensively, priced in life rather than in creature
    // value. Tapping out for an attack matters exactly as much as the damage
    // that then gets through on the crack back -- no more, and no less.
    let their_attackers: Vec<&Permanent> = board
        .their_creatures()
        .filter(|permanent| !permanent.has(&Keyword::Defender) && permanent.power > 0)
        .collect();
    let defensive_cost = |set: &[&Permanent]| -> f32 {
        if aggression == Aggression::AllIn || their_attackers.is_empty() {
            return 0.0;
        }
        // Creatures still home next turn: everything untapped that either did
        // not attack or has vigilance.
        let home: Vec<&Permanent> = board
            .my_creatures()
            .filter(|permanent| !permanent.tapped)
            .filter(|permanent| {
                permanent.has(&Keyword::Vigilance)
                    || !set.iter().any(|entry| entry.object == permanent.object)
            })
            .collect();
        // A rough absorption model: each blocker stops one attacker, biggest
        // first. Understating my defence here is the safe direction.
        let mut incoming: Vec<i32> = their_attackers
            .iter()
            .map(|attacker| i32::from(attacker.power.max(0)))
            .collect();
        incoming.sort_unstable_by(|left, right| right.cmp(left));
        let absorbed: i32 = incoming.iter().take(home.len()).sum();
        let total: i32 = incoming.iter().sum();
        damage_value((total - absorbed).max(0), board.my_life, weights.own_life)
    };
    let baseline_cost = defensive_cost(&[]);

    // Score whole sets, not one attacker at a time. Growing greedily from
    // nothing cannot escape the obvious local minimum: with three 2/2s facing
    // one 3/3, *every* single attacker scores badly on its own, yet attacking
    // with all three is plainly right because only one can be blocked.
    let set_score = |set: &[&Permanent]| -> f32 {
        if set.is_empty() {
            return 0.0;
        }
        let mut score = attack_score(set, &blockers, weights, opponent.life);
        if score.is_infinite() {
            return score;
        }
        // Only the *extra* damage caused by attacking counts against the plan;
        // damage I would take anyway is not the attack's fault.
        score -= defensive_cost(set) - baseline_cost;
        if aggression == Aggression::Pressing {
            for attacker in set {
                score += creature_value(attacker, weights) * 0.2;
            }
        }
        score
    };

    // Candidate order: the creatures most likely to connect go in first, so a
    // prefix of length k is a sensible set rather than an arbitrary one.
    let mut ordered = candidates.clone();
    ordered.sort_by(|left, right| {
        let rank = |permanent: &&Permanent| {
            (
                u8::from(is_evasive(permanent)),
                permanent.power,
                std::cmp::Reverse(permanent.object.0),
            )
        };
        rank(right).cmp(&rank(left))
    });

    let mut chosen: Vec<&Permanent> = Vec::new();
    let mut best_score = 0.0_f32;
    for size in 1..=ordered.len() {
        let trial = &ordered[..size];
        let score = set_score(trial);
        if score > best_score {
            best_score = score;
            chosen = trial.to_vec();
        }
    }

    // Local search: try removing or adding one attacker at a time. This fixes
    // the cases where the prefix order put a bad attacker ahead of a good one.
    for _ in 0..ordered.len().min(8) {
        let mut improved = false;
        for candidate in &ordered {
            let mut trial = chosen.clone();
            if let Some(position) = trial
                .iter()
                .position(|entry| entry.object == candidate.object)
            {
                trial.remove(position);
            } else {
                trial.push(candidate);
            }
            let score = set_score(&trial);
            if score > best_score {
                best_score = score;
                chosen = trial;
                improved = true;
            }
        }
        if !improved {
            break;
        }
    }

    let mut attackers: Vec<ObjectId> = chosen.iter().map(|permanent| permanent.object).collect();
    attackers.sort_unstable_by_key(|object| object.0);
    AttackPlan {
        attackers,
        is_lethal: false,
    }
}

// ---------------------------------------------------------------------------
// Generation 3 blocking: damage prevented is part of a block's value.
// ---------------------------------------------------------------------------

/// Generation 3 block planning.
///
/// The v1/v2 planner scored a block purely as material: `their_loss -
/// my_loss`, with a survival override only when the incoming attack was
/// already lethal. That makes a wall worthless. A 2/5 blocking a 3/3 kills
/// nothing and loses nothing, so the material delta is exactly zero and the
/// block is declined -- the wall stands and watches while three damage goes
/// through, every turn, forever.
///
/// Here a block is also worth the damage it stops, priced on the same
/// life-fraction scale as everything else, and assignments are chosen globally
/// by descending value rather than attacker by attacker.
#[must_use]
pub fn plan_blocks_valued(board: &Board, weights: &Weights) -> Vec<Block> {
    let attackers: Vec<&Permanent> = board.attackers.iter().collect();
    if attackers.is_empty() {
        return Vec::new();
    }
    let available: Vec<&Permanent> = board
        .my_creatures()
        .filter(|permanent| !permanent.tapped)
        .collect();
    if available.is_empty() {
        return Vec::new();
    }

    let incoming: i32 = attackers
        .iter()
        .map(|attacker| i32::from(attacker.power.max(0)))
        .sum();
    let facing_lethal = i64::from(incoming) >= board.my_life;

    let mut pairs: Vec<(f32, usize, usize)> = Vec::new();
    for (attacker_index, attacker) in attackers.iter().enumerate() {
        for (blocker_index, blocker) in available.iter().enumerate() {
            if !can_block(blocker, attacker) {
                continue;
            }
            let result = exchange(attacker, blocker);
            let my_loss = if result.blocker_dies {
                creature_value(blocker, weights)
            } else {
                0.0
            };
            let their_loss = if result.attacker_dies {
                creature_value(attacker, weights)
            } else {
                0.0
            };
            // Trample still gets the excess through, so only the absorbed part
            // counts as prevented.
            let stopped = if attacker.has(&Keyword::Trample) {
                i32::from(blocker.toughness.max(0)).min(i32::from(attacker.power.max(0)))
            } else {
                i32::from(attacker.power.max(0))
            };
            let mut gain =
                their_loss - my_loss + damage_value(stopped, board.my_life, weights.own_life);
            if facing_lethal {
                // Surviving the turn dominates every material consideration.
                gain += f32::from(attacker.power.max(0)) * 10.0;
            }
            pairs.push((gain, attacker_index, blocker_index));
        }
    }
    // Deterministic: value descending, then stable index order.
    pairs.sort_by(|left, right| {
        right
            .0
            .total_cmp(&left.0)
            .then(left.1.cmp(&right.1))
            .then(left.2.cmp(&right.2))
    });

    let mut used_attacker = vec![false; attackers.len()];
    let mut used_blocker = vec![false; available.len()];
    let mut blocks = Vec::new();
    for (gain, attacker, blocker) in pairs {
        if used_attacker[attacker] || used_blocker[blocker] || gain <= 0.0 {
            continue;
        }
        used_attacker[attacker] = true;
        used_blocker[blocker] = true;
        blocks.push(Block {
            attacker: attackers[attacker].object,
            blocker: available[blocker].object,
        });
    }
    blocks.sort_by_key(|block| (block.attacker.0, block.blocker.0));
    blocks
}
