//! Reusable planning components shared by every archetype policy.
//!
//! The point of this module tree is that no policy should contain a search for
//! a card definition id. Mana payment, board evaluation, target choice, and
//! combat math are card-agnostic problems; solving them once and weighting them
//! per archetype is what makes a deck's strength measurable independently of
//! how well someone hand-wrote its pilot.
//!
//! Layering: [`board`] adapts the engine's view into a seat-general shape, and
//! everything else reads only that. Nothing here submits an action; the policy
//! decides, using these answers.

pub mod board;
pub mod combat;
pub mod mana;
pub mod target;
pub mod threat;

pub use board::{Board, CardFacts, CardIndex, Permanent, Role, SourceKind};
pub use combat::{
    Aggression, AttackPlan, Block, can_block, plan_attack, plan_attack_assigned, plan_blocks,
    plan_blocks_valued,
};
pub use mana::{ManaPlan, ManaTap, can_pay, plan, potential};
pub use target::{cast_value, targets_for};
pub use threat::{Weights, creature_value, evaluate, my_clock, their_clock, winning_the_race};
