//! Mana planning: which spells are castable now, and what to tap for them.
//!
//! The policies this replaces asked `can_pay(view, cost)` against mana that
//! was *already floating*, which meant a policy could only cast a spell if it
//! had happened to activate the right sources first -- and it had no way to
//! know which those were. That is the direct cause of decks flooding out with
//! a castable hand.
//!
//! This solves the actual problem: given the untapped sources on the board and
//! a printed cost, produce an ordered tap sequence that pays it, or prove none
//! exists.

use super::board::{Board, Permanent, SourceKind};
use cardbench_magic_engine::{Color, ManaCost, ObjectId};

/// One mana activation the policy should submit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ManaTap {
    /// A land with intrinsic basic-land mana, tapped for one colour.
    Basic { land: ObjectId, color: Color },
    /// A bound ability with a fixed or chosen single-colour output.
    Bound {
        source: ObjectId,
        ability: &'static str,
        color: Option<Color>,
    },
    /// A bound ability producing a fixed multi-mana bundle.
    Bundle {
        source: ObjectId,
        ability: &'static str,
        bundle: Vec<(Color, u8)>,
    },
}

impl ManaTap {
    #[must_use]
    pub const fn source(&self) -> ObjectId {
        match self {
            Self::Basic { land, .. } => *land,
            Self::Bound { source, .. } | Self::Bundle { source, .. } => *source,
        }
    }
}

/// A complete payment for one cost.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManaPlan {
    /// Activations to submit, in order, before casting.
    pub taps: Vec<ManaTap>,
    /// Mana produced beyond the cost. Bundles routinely overshoot; knowing by
    /// how much lets the caller prefer an exact payment.
    pub waste: u8,
}

/// An untapped source available this window.
#[derive(Clone, Debug)]
struct Source {
    object: ObjectId,
    kind: SourceKind,
    /// Lower sorts first. Sources that are harder to replace, or whose use has
    /// a cost, are held back.
    priority: u8,
}

/// Colour-indexed mana counts.
type Pool = [u8; 6];

fn add(pool: &mut Pool, color: Color, amount: u8) {
    pool[color.index()] = pool[color.index()].saturating_add(amount);
}

fn total(pool: Pool) -> u32 {
    pool.iter().map(|amount| u32::from(*amount)).sum()
}

/// Collects every source that can be activated right now.
fn available(board: &Board) -> Vec<Source> {
    let mut sources: Vec<Source> = board
        .mine
        .iter()
        // A summoning-sick creature cannot pay a tap cost, so it is not a
        // source this turn however much mana it would produce.
        .filter(|permanent| !permanent.tapped && !permanent.summoning_sick)
        .filter_map(|permanent| {
            let kind = permanent.source.clone()?;
            Some(Source {
                object: permanent.object,
                priority: source_priority(permanent, &kind),
                kind,
            })
        })
        .collect();
    // Deterministic: identical boards must produce identical plans, or replay
    // digests diverge.
    sources.sort_by_key(|source| (source.priority, source.object.0));
    sources
}

fn source_priority(permanent: &Permanent, kind: &SourceKind) -> u8 {
    // Spend the least flexible source first so a dual or a Birds is still
    // available for a colour only it can make. Hold creature sources back:
    // tapping one removes a blocker.
    let flexibility = u8::try_from(kind.colors().len()).unwrap_or(u8::MAX);
    let creature_penalty = u8::from(permanent.is_creature) * 8;
    flexibility.saturating_add(creature_penalty)
}

/// Every distinct way one source can be activated, as a produced pool.
fn options(kind: &SourceKind) -> Vec<(Pool, Option<Color>)> {
    match kind {
        SourceKind::BasicTyped(colors) | SourceKind::BoundChoice { colors, .. } => colors
            .iter()
            .map(|color| {
                let mut pool = [0_u8; 6];
                add(&mut pool, *color, 1);
                (pool, Some(*color))
            })
            .collect(),
        SourceKind::BoundFixed { color, .. } => {
            let mut pool = [0_u8; 6];
            add(&mut pool, *color, 1);
            vec![(pool, Some(*color))]
        }
        SourceKind::BoundBundle { bundle, .. } => {
            let mut pool = [0_u8; 6];
            for (color, amount) in bundle {
                add(&mut pool, *color, *amount);
            }
            vec![(pool, None)]
        }
    }
}

fn tap_for(source: &Source, color: Option<Color>) -> ManaTap {
    match &source.kind {
        SourceKind::BasicTyped(_) => ManaTap::Basic {
            land: source.object,
            color: color.expect("a basic-typed activation names its colour"),
        },
        SourceKind::BoundFixed { ability, .. } => ManaTap::Bound {
            source: source.object,
            ability,
            color: None,
        },
        SourceKind::BoundChoice { ability, .. } => ManaTap::Bound {
            source: source.object,
            ability,
            color,
        },
        SourceKind::BoundBundle { ability, bundle } => ManaTap::Bundle {
            source: source.object,
            ability,
            bundle: bundle.clone(),
        },
    }
}

/// The colour requirements of a cost, as a demand a pool must satisfy.
#[derive(Clone, Debug)]
struct Demand {
    /// Fixed colour symbols still unpaid.
    colored: Vec<Color>,
    /// Hybrid symbols, each payable with either listed colour.
    hybrid: Vec<(Color, Color)>,
    generic: u8,
}

impl Demand {
    fn from_cost(cost: &ManaCost) -> Self {
        Self {
            colored: cost.colored.clone(),
            hybrid: cost
                .hybrid
                .iter()
                .map(|symbol| (symbol.first, symbol.second))
                .collect(),
            generic: cost.generic,
        }
    }

    fn is_empty(&self) -> bool {
        self.colored.is_empty() && self.hybrid.is_empty() && self.generic == 0
    }

    /// Removes from this demand everything `pool` can pay, most constrained
    /// symbols first. Returns the leftover pool.
    fn settle(&mut self, mut pool: Pool) -> Pool {
        self.colored.retain(|color| {
            if pool[color.index()] > 0 {
                pool[color.index()] -= 1;
                false
            } else {
                true
            }
        });
        self.hybrid.retain(|(first, second)| {
            for color in [first, second] {
                if pool[color.index()] > 0 {
                    pool[color.index()] -= 1;
                    return false;
                }
            }
            true
        });
        while self.generic > 0 {
            let Some(index) = pool.iter().position(|amount| *amount > 0) else {
                break;
            };
            pool[index] -= 1;
            self.generic -= 1;
        }
        pool
    }
}

/// The most activations one plan will consider. A hand can hold a cost of at
/// most a handful of symbols; this only bounds pathological board states.
const MAX_TAPS: usize = 24;

/// Plans a payment for `cost` from the board's untapped sources.
///
/// Returns `None` when no combination pays it. That is a real answer: the
/// caller must not attempt the cast.
#[must_use]
pub fn plan(board: &Board, cost: &ManaCost) -> Option<ManaPlan> {
    let sources = available(board);
    let mut demand = Demand::from_cost(cost);
    // Mana already floating is spent before anything is tapped.
    let leftover = demand.settle(board.floating);
    if demand.is_empty() {
        return Some(ManaPlan {
            taps: Vec::new(),
            waste: u8::try_from(total(leftover)).unwrap_or(u8::MAX),
        });
    }
    let mut taps = Vec::new();
    let mut used = vec![false; sources.len()];
    search(&sources, &mut used, &demand, leftover, &mut taps).map(|waste| ManaPlan { taps, waste })
}

/// Whether `cost` is payable right now.
#[must_use]
pub fn can_pay(board: &Board, cost: &ManaCost) -> bool {
    plan(board, cost).is_some()
}

/// Total mana this board could produce if every source were tapped.
///
/// Used for curve decisions -- "hold this land, I already have enough" -- not
/// for legality.
#[must_use]
pub fn potential(board: &Board) -> u32 {
    available(board)
        .iter()
        .map(|source| u32::from(source.kind.quantity()))
        .sum::<u32>()
        + total(board.floating)
}

/// Depth-first assignment of sources to unmet symbols.
///
/// Recursion is on the *demand*, not on the source list: it picks the single
/// most constrained unpaid symbol and tries only sources that can pay it. That
/// keeps the branching factor at the number of sources producing one colour
/// instead of the number of ways to tap the whole board.
fn search(
    sources: &[Source],
    used: &mut Vec<bool>,
    demand: &Demand,
    floating: Pool,
    taps: &mut Vec<ManaTap>,
) -> Option<u8> {
    if demand.is_empty() {
        return Some(u8::try_from(total(floating)).unwrap_or(u8::MAX));
    }
    if taps.len() >= MAX_TAPS {
        return None;
    }

    // Pay a fixed colour symbol first: it admits the fewest sources.
    let wanted: Vec<Color> = if let Some(color) = demand.colored.first() {
        vec![*color]
    } else if let Some((first, second)) = demand.hybrid.first() {
        vec![*first, *second]
    } else {
        // Only generic remains; any source will do.
        Color::MANA_ALL.to_vec()
    };

    for (index, source) in sources.iter().enumerate() {
        if used[index] {
            continue;
        }
        let produced = options(&source.kind);
        for (pool, color) in produced {
            if !wanted.iter().any(|want| pool[want.index()] > 0) {
                continue;
            }
            let mut combined = floating;
            for want in Color::MANA_ALL {
                combined[want.index()] = combined[want.index()].saturating_add(pool[want.index()]);
            }
            let mut next = demand.clone();
            let leftover = next.settle(combined);
            used[index] = true;
            taps.push(tap_for(source, color));
            if let Some(waste) = search(sources, used, &next, leftover, taps) {
                return Some(waste);
            }
            taps.pop();
            used[index] = false;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::board::{Opponent, Role};
    use cardbench_magic_engine::Step;

    fn land(object: u64, colors: &[Color]) -> Permanent {
        Permanent {
            object: ObjectId(object),
            controller: cardbench_magic_engine::PlayerId(0),
            definition: Some("LAND"),
            tapped: false,
            can_attack: false,
            can_block: false,
            summoning_sick: false,
            is_creature: false,
            is_land: true,
            power: 0,
            toughness: 0,
            keywords: Vec::new(),
            source: Some(SourceKind::BasicTyped(colors.to_vec())),
            role: Role::Land,
            abilities: Vec::new(),
            nonmana_activated_abilities_suppressed: false,
            controller_is_opponent: false,
        }
    }

    fn karoo(object: u64, colors: [Color; 2]) -> Permanent {
        Permanent {
            source: Some(SourceKind::BoundBundle {
                ability: "karoo",
                bundle: vec![(colors[0], 1), (colors[1], 1)],
            }),
            ..land(object, &[])
        }
    }

    fn board(permanents: Vec<Permanent>) -> Board {
        Board {
            commanders: vec![],
            me: cardbench_magic_engine::PlayerId(0),
            my_life: 20,
            opponents: vec![Opponent {
                seat: cardbench_magic_engine::PlayerId(1),
                life: 20,
            }],
            turn: 1,
            step: Step::PrecombatMain,
            is_my_turn: true,
            lands_played: 0,
            stack_depth: 0,
            attackers_declared: false,
            blockers_declared: false,
            hand: Vec::new(),
            mine: permanents,
            theirs: Vec::new(),
            attackers: Vec::new(),
            floating: [0; 6],
        }
    }

    #[test]
    fn a_single_colored_symbol_taps_one_matching_land() {
        let board = board(vec![land(1, &[Color::Red]), land(2, &[Color::White])]);
        let plan = plan(&board, &ManaCost::with_colors(0, [Color::Red])).expect("payable");
        assert_eq!(
            plan.taps,
            vec![ManaTap::Basic {
                land: ObjectId(1),
                color: Color::Red
            }]
        );
        assert_eq!(plan.waste, 0);
    }

    /// The reason a naive greedy planner fails: the dual is the only source of
    /// white, so it must not be spent on the red symbol.
    #[test]
    fn a_dual_is_reserved_for_the_color_only_it_can_make() {
        let board = board(vec![
            land(1, &[Color::Red]),
            land(2, &[Color::White, Color::Red]),
        ]);
        let plan = plan(
            &board,
            &ManaCost::with_colors(0, [Color::White, Color::Red]),
        )
        .expect("payable");
        assert_eq!(plan.taps.len(), 2);
        assert!(plan.taps.contains(&ManaTap::Basic {
            land: ObjectId(2),
            color: Color::White
        }));
        assert!(plan.taps.contains(&ManaTap::Basic {
            land: ObjectId(1),
            color: Color::Red
        }));
    }

    #[test]
    fn generic_costs_accept_any_source() {
        let board = board(vec![
            land(1, &[Color::Red]),
            land(2, &[Color::White]),
            land(3, &[Color::Green]),
        ]);
        let plan = plan(&board, &ManaCost::with_colors(2, [Color::Red])).expect("payable");
        assert_eq!(plan.taps.len(), 3);
    }

    #[test]
    fn a_bundle_land_pays_two_symbols_with_one_activation() {
        let board = board(vec![karoo(1, [Color::Red, Color::White])]);
        let plan = plan(
            &board,
            &ManaCost::with_colors(0, [Color::Red, Color::White]),
        )
        .expect("payable");
        assert_eq!(plan.taps.len(), 1);
        assert_eq!(plan.waste, 0);
    }

    #[test]
    fn an_unpayable_cost_is_reported_rather_than_guessed() {
        let board = board(vec![land(1, &[Color::Red]), land(2, &[Color::Red])]);
        assert!(plan(&board, &ManaCost::with_colors(0, [Color::Blue])).is_none());
        assert!(plan(&board, &ManaCost::with_colors(5, [Color::Red])).is_none());
    }

    #[test]
    fn floating_mana_is_spent_before_anything_is_tapped() {
        let mut board = board(vec![land(1, &[Color::Red])]);
        board.floating[Color::Red.index()] = 1;
        let plan = plan(&board, &ManaCost::with_colors(0, [Color::Red])).expect("payable");
        assert!(plan.taps.is_empty(), "floating mana needs no activation");
    }

    #[test]
    fn tapped_sources_are_not_available() {
        let mut permanent = land(1, &[Color::Red]);
        permanent.tapped = true;
        let board = board(vec![permanent]);
        assert!(plan(&board, &ManaCost::with_colors(0, [Color::Red])).is_none());
    }

    /// Identical boards must produce identical plans or replay digests drift.
    #[test]
    fn planning_is_deterministic() {
        let build = || {
            board(vec![
                land(3, &[Color::Red, Color::White]),
                land(1, &[Color::Red]),
                land(2, &[Color::White]),
            ])
        };
        let cost = ManaCost::with_colors(1, [Color::Red, Color::White]);
        let first = plan(&build(), &cost).expect("payable");
        for _ in 0..8 {
            assert_eq!(plan(&build(), &cost).expect("payable"), first);
        }
    }

    #[test]
    fn hybrid_symbols_accept_either_color() {
        let board = board(vec![land(1, &[Color::Green])]);
        let cost = ManaCost::with_hybrid(
            0,
            [],
            [cardbench_magic_engine::HybridManaSymbol {
                first: Color::White,
                second: Color::Green,
            }],
        );
        assert!(plan(&board, &cost).is_some());
    }
}
