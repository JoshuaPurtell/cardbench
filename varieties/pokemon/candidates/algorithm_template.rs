
use rand::seq::SliceRandom;
use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;

use tcg_core::{Action, Attack, CardInstanceId, GameView, Prompt};
use tcg_ai::traits::AiController;

/// TemplateAi - starting point for a Pokemon TCG code policy.
///
/// This is a SKELETON, not a baseline. It measures 28.8% cell win rate on the
/// train surface and loses to all five train opponents. The scored reference —
/// the policy your submission is measured *against* — is
/// `candidates/reference/reference_policy_v2.rs` at 64.9%. Read that file: it is
/// the worked example, it is self-contained under this same ABI, and beating it
/// is the task.
///
/// What `GameView` actually gives you (checked against
/// `engine/tcg_core/src/view.rs`, not from memory):
/// - view.my_active: Option<PokemonView>; view.my_bench: Vec<PokemonView>
/// - view.opponent_active / view.opponent_bench: the same, for the opponent
/// - view.my_hand: Vec<CardInstance>; view.my_discard, view.opponent_discard
/// - view.my_prizes_count / view.opponent_prizes_count
/// - view.current_player / view.player_id / view.phase / view.pending_prompt
/// - view.action_hints: playable_basic_ids, playable_energy_ids,
///   playable_trainer_ids, playable_evolution_ids, evolve_targets_by_card_id,
///   attach_targets, can_declare_attack, can_end_turn, usable_attacks
///   (NOTE: there is no `can_retreat` hint. Propose `Action::Retreat` and let
///   the engine reject it if it is illegal.)
///
/// `PokemonView` has:
/// - card: CardInstance, which is ONLY { id, def_id, owner } — no name, no
///   card_type. Nothing tells you what a card in hand is except which
///   `action_hints` list its id appears in.
/// - hp: u16, the MAXIMUM hp of this Pokemon
/// - damage_counters: u16, COUNTERS not damage. The engine knocks out at
///   `damage_counters * 10 >= hp`, so remaining hp is `hp - damage_counters*10`.
///   Getting this wrong silently disables every lethal check you write.
/// - attached_energy: Vec<CardInstance>, attached_tool: Option<CardInstance>
/// - types: Vec<Type>, weakness: Option<Weakness>, resistance: Option<Resistance>
/// - is_ex, is_star, special_conditions
/// - there is NO attack list on `PokemonView`. You can see your ACTIVE's payable
///   attacks via `action_hints.usable_attacks` and nothing else — not your
///   bench's, and not the opponent's.
///
/// Two things that cost more win rate than play quality does:
/// 1. An unanswered prompt STALLS the game, and a stalled game scores as a
///    non-win. Fifteen `Prompt` variants can actually be raised; answer all of
///    them, with a legal fallback each.
/// 2. `Prompt::ChooseDefenderAttack` is answered with
///    `Action::ChooseDefenderAttack`, not `Action::DeclareAttack`.
pub struct TemplateAi {
    rng: ChaCha8Rng,
}

impl TemplateAi {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }

    /// Pick best attack by damage, preferring lower energy cost as tiebreaker
    fn best_attack(attacks: &[Attack]) -> Option<Attack> {
        attacks.iter().cloned().max_by(|a, b| {
            let dmg = a.damage.cmp(&b.damage);
            if dmg != std::cmp::Ordering::Equal {
                return dmg;
            }
            a.cost.total_energy.cmp(&b.cost.total_energy).reverse()
        })
    }
}

impl AiController for TemplateAi {
    fn propose_prompt_response(&mut self, view: &GameView, prompt: &Prompt) -> Vec<Action> {
        let mut actions: Vec<Action> = Vec::new();

        match prompt {
            Prompt::ChooseStartingActive { options } => {
                // TODO: Pick the best starter (consider HP, retreat cost, attack potential)
                if let Some(&card_id) = options.first() {
                    actions.push(Action::ChooseActive { card_id });
                }
            }
            Prompt::ChooseBenchBasics { options, min, max } => {
                // TODO: Strategically choose which basics to bench
                let count = (*min).max(1).min(*max).min(options.len());
                let picked: Vec<CardInstanceId> = options.iter().take(count).copied().collect();
                actions.push(Action::ChooseBench { card_ids: picked });
            }
            Prompt::ChooseAttack { attacks, .. } => {
                // Pick highest damage attack
                if let Some(best) = Self::best_attack(attacks) {
                    actions.push(Action::DeclareAttack { attack: best });
                }
                // Strict: try all attacks
                for attack in attacks {
                    actions.push(Action::DeclareAttack { attack: attack.clone() });
                }
            }
            Prompt::ChooseNewActive { player, options } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }
                // TODO: Pick best replacement (consider HP, energy, matchup)
                let candidates: Vec<CardInstanceId> = if options.is_empty() {
                    view.my_bench.iter().map(|p| p.card.id).collect()
                } else {
                    options.clone()
                };
                if let Some(&card_id) = candidates.first() {
                    actions.push(Action::ChooseNewActive { card_id });
                }
            }
            _ => {
                // Handle other prompts with EndTurn strict
            }
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

        // TODO: Improve energy attachment strategy
        // - Prioritize Pokemon that can attack this turn
        // - Consider evolution targets
        if let Some(&energy_id) = hints.playable_energy_ids.first() {
            if let Some(&target_id) = hints.attach_targets.first() {
                actions.push(Action::AttachEnergy { energy_id, target_id });
            }
        }

        // Attack with best available attack
        if let Some(best) = Self::best_attack(&hints.usable_attacks) {
            actions.push(Action::DeclareAttack { attack: best });
        }

        // TODO: Improve bench strategy
        // - Set up evolution lines
        // - Maintain backup attackers
        if let Some(&card_id) = hints.playable_basic_ids.first() {
            actions.push(Action::PlayBasic { card_id });
        }

        // TODO: Consider retreat when advantageous
        // if hints.can_retreat { ... }

        actions.push(Action::EndTurn);
        actions
    }
}
