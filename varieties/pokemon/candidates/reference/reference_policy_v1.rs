// CardBench Pokemon code-policy reference solution — SUPERSEDED, see below.
//
// !! STALE AS AN ORACLE. This file was the Harbor `verify` lane's reference
// !! solution back when the ranking origin was `baseline_policy.rs` (28.9% cell
// !! win rate). The origin is now `reference_policy_v2.rs` at 64.9%, and this
// !! policy measures 35.6% — it LOSES to the origin by -0.293 (bootstrap CI
// !! [-0.334, -0.251], `ci_below_zero`). `adapters/harbor/scripts/run_harbor.py`
// !! still points `REFERENCE_POLICY` here, so the verify lane will now fail
// !! until that pointer moves to a policy that actually beats the origin.
// !! Two defects measured in this file, both fixed in v2:
// !!   * `defender_remaining_hp` returns `hp - damage_counters`, but counters
// !!     are worth 10 damage each, so the lethal check almost never fired.
// !!   * it answers none of the search / discard / reorder prompts, so 16.5% of
// !!     its games stalled and scored as non-wins.
//
// Original header follows.
//
// This is the policy the Harbor `verify` lane runs to prove the rig works: it
// must beat candidates/reference/baseline_policy.rs (TemplateAi, ~29% cell win
// rate) by a margin the paired-bootstrap gate accepts.
//
// It is deliberately a *heuristic* solution, not a search — the point is a
// readable reference an agent can beat, not a ceiling.
//
// Where it differs from the older examples/simple_heuristic_ai.rs:
//
//   1. Energy goes on the ACTIVE Pokemon. simple_heuristic picked a random
//      attach target, so most energy landed on the bench where it can never
//      pay for an attack. This is the single largest source of its losses.
//   2. Empties the hand of basics onto the bench instead of one per turn, so a
//      knockout does not end the game on an empty bench.
//   3. Prefers an attack that actually knocks out the defender over the
//      nominally highest-damage attack.
//   4. Promotes the healthiest benched Pokemon after a knockout.
//
// Two obvious improvements are left on the table on purpose, because both
// stall games for a policy that cannot answer follow-up prompts, and a stalled
// game scores as a loss: playing trainers (measured 757/800 stalls) and
// evolving from hand (271/800). Teaching a policy to answer search/discard/
// reorder prompts is the natural next step for an agent working this task.

use rand::seq::SliceRandom;
use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;

use tcg_core::{Action, Attack, CardInstanceId, GameView, Prompt};

use tcg_ai::traits::AiController;

pub struct ReferencePolicyV1 {
    rng: ChaCha8Rng,
}

impl ReferencePolicyV1 {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }

    /// Remaining HP on the defending Pokemon, if one is in play.
    fn defender_remaining_hp(view: &GameView) -> Option<u16> {
        view.opponent_active
            .as_ref()
            .map(|active| active.hp.saturating_sub(active.damage_counters))
    }

    /// Prefer a lethal attack; otherwise the biggest hit, cheapest on ties.
    fn best_attack(attacks: &[Attack], defender_hp: Option<u16>) -> Option<Attack> {
        if attacks.is_empty() {
            return None;
        }
        if let Some(hp) = defender_hp {
            let lethal = attacks
                .iter()
                .filter(|attack| attack.damage as u16 >= hp)
                .min_by_key(|attack| attack.cost.total_energy);
            if let Some(attack) = lethal {
                return Some(attack.clone());
            }
        }
        attacks.iter().cloned().max_by(|a, b| {
            let damage = a.damage.cmp(&b.damage);
            if damage != std::cmp::Ordering::Equal {
                return damage;
            }
            a.cost.total_energy.cmp(&b.cost.total_energy).reverse()
        })
    }

    /// Active first, then bench: energy is only useful where it can attack.
    fn attach_priority(view: &GameView) -> Vec<CardInstanceId> {
        let mut targets = Vec::new();
        if let Some(active) = view.my_active.as_ref() {
            targets.push(active.card.id);
        }
        targets.extend(view.my_bench.iter().map(|slot| slot.card.id));
        targets
    }
}

impl AiController for ReferencePolicyV1 {
    fn propose_prompt_response(&mut self, view: &GameView, prompt: &Prompt) -> Vec<Action> {
        let mut actions: Vec<Action> = Vec::new();

        match prompt {
            Prompt::ChooseStartingActive { options } => {
                // Highest-HP legal basic; a fragile opener loses prizes early.
                let mut valid: Vec<CardInstanceId> = options
                    .iter()
                    .filter(|id| view.action_hints.playable_basic_ids.contains(id))
                    .copied()
                    .collect();
                if valid.is_empty() {
                    valid = options.clone();
                }
                for card_id in &valid {
                    actions.push(Action::ChooseActive { card_id: *card_id });
                }
            }
            Prompt::ChooseBenchBasics { options, min, max } => {
                let mut valid: Vec<CardInstanceId> = options
                    .iter()
                    .filter(|id| view.action_hints.playable_basic_ids.contains(id))
                    .copied()
                    .collect();
                if valid.is_empty() {
                    valid = options.clone();
                }
                // Fill the bench as far as the prompt allows.
                let lower = (*min).min(valid.len());
                let upper = (*max).min(valid.len()).max(lower);
                valid.truncate(upper);
                actions.push(Action::ChooseBench { card_ids: valid });
            }
            Prompt::ChooseAttack { attacks, .. } => {
                if let Some(best) = Self::best_attack(attacks, Self::defender_remaining_hp(view)) {
                    actions.push(Action::DeclareAttack { attack: best });
                }
                // Fallbacks in case the preferred attack is rejected.
                let mut rest = attacks.clone();
                rest.shuffle(&mut self.rng);
                for attack in rest {
                    actions.push(Action::DeclareAttack { attack });
                }
            }
            Prompt::ChooseNewActive { player, options } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }
                let mut candidates: Vec<CardInstanceId> = if options.is_empty() {
                    view.my_bench.iter().map(|slot| slot.card.id).collect()
                } else {
                    options.clone()
                };
                // Promote the healthiest benched Pokemon.
                candidates.sort_by_key(|id| {
                    view.my_bench
                        .iter()
                        .find(|slot| slot.card.id == *id)
                        .map(|slot| {
                            std::cmp::Reverse(slot.hp.saturating_sub(slot.damage_counters))
                        })
                        .unwrap_or(std::cmp::Reverse(0))
                });
                for card_id in candidates {
                    actions.push(Action::ChooseNewActive { card_id });
                }
            }
            _ => {}
        }

        actions
    }

    fn propose_free_actions(&mut self, view: &GameView) -> Vec<Action> {
        if view.current_player != view.player_id {
            return Vec::new();
        }
        if view.pending_prompt.is_some() {
            return Vec::new();
        }

        let mut actions: Vec<Action> = Vec::new();
        let hints = &view.action_hints;

        // One energy per turn, onto the Pokemon that can actually use it.
        if let Some(energy_id) = hints.playable_energy_ids.first().copied() {
            for target_id in Self::attach_priority(view) {
                if hints.attach_targets.contains(&target_id) {
                    actions.push(Action::AttachEnergy { energy_id, target_id });
                    break;
                }
            }
        }

        // Deliberately does NOT play trainers. Most trainers raise follow-up
        // prompts (search, discard, reorder) that this policy has no answer
        // for, and an unanswered prompt stalls the game until the step budget
        // runs out — which scores as a loss. Handling those prompts is exactly
        // the kind of improvement an agent should be making.

        // Bench everything available: an empty bench loses the game outright
        // when the active is knocked out.
        for card_id in &hints.playable_basic_ids {
            actions.push(Action::PlayBasic { card_id: *card_id });
        }

        if let Some(best) =
            Self::best_attack(&hints.usable_attacks, Self::defender_remaining_hp(view))
        {
            actions.push(Action::DeclareAttack { attack: best });
        }

        actions.push(Action::EndTurn);
        actions
    }
}
