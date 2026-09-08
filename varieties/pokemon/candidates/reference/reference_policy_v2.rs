// CardBench Pokemon code-policy REFERENCE POLICY v2 — the ranking origin.
//
// Why this file exists
// --------------------
// A code-policy benchmark measures `candidate - reference`. If the reference is
// incompetent, every candidate beats it and the number means nothing: that is
// the defect that invalidated the gamebench rogue lane, whose "baseline"
// replayed a memorised action sequence. `candidates/reference/baseline_policy.rs`
// (TemplateAi) measured 28.9% cell win rate over the train surface and lost to
// every roster opponent, so it was a floor, not an origin. This file replaces it.
//
// Contract this file obeys — it is also the worked example agents copy, so it
// must obey the rules it teaches:
//
//   1. SELF-CONTAINED. One compilation unit. Nothing from the workspace beyond
//      the engine view types the `AiController` ABI already exposes
//      (`GameView`, `PokemonView`, `Prompt`, `Action`, `Attack`).
//   2. NO MEMORISED LINES. Every decision is a function of the *structure* of
//      the observed state — remaining HP, energy attached, weakness/resistance,
//      prompt shape. It never branches on a deck id, an opponent id, or a
//      specific card. The only card text it reads is the engine's own canonical
//      `ENERGY-<TYPE>` def-id convention, which is how any policy must know
//      what colour of energy it is holding.
//   3. DETERMINISTIC given (seed, seat). No RNG draws. Ties break on a fixed
//      mix of the seed and the card instance id, so two runs of a cell agree
//      exactly and the paired bootstrap compares signal rather than noise.
//
// The ladder (§2.2 of docs/CODE_POLICY_DEO_DESIGN.md). `propose_free_actions`
// returns an ordered candidate list and the runner applies the first legal one,
// then calls again — so the list *is* the priority ladder, and a rung that has
// already been taken this turn simply stops being legal:
//
//   1. lethal check      take the knockout now; nothing else changes the turn
//   2. forced response   active cannot act or is nearly dead: retreat
//   3. supporter/item    dig for resources before committing them
//   4. evolution         evolve the best target, active first
//   5. energy attachment onto whoever is closest to paying for an attack
//   6. board development bench basics up to a working bench
//   7. best attack       highest damage AFTER weakness and resistance
//   8. pass
//
// Rungs 1 and 7 are most of the gap from 28.9%. The second largest source of
// loss was not play quality at all: 16.5% of the old reference's games *stalled*
// — no proposed action was legal, the runner ran out of budget, and the game
// scored as a non-win. So `propose_prompt_response` answers all fifteen prompts
// the engine can actually raise, each with a legal fallback.
//
// Deliberately NOT here, because they are the interesting headroom a candidate
// should be attacking:
//   * any lookahead or search — this is a one-ply greedy policy;
//   * modelling the opponent's attacks (`PokemonView` does not expose the
//     opponent's attack list, so danger is proxied by remaining HP);
//   * card-specific play patterns for the trainers in a given deck.

use tcg_core::{PokemonView};

use tcg_ai::traits::AiController;

/// One-ply heuristic pilot. See the module comment for the ladder.
pub struct ReferencePolicyV2 {
    /// Seat/seed identity. Used only to break exact ties deterministically, so
    /// the policy is a pure function of (seed, seat, state).
    seed: u64,
}

/// Bench size the policy plays toward. Three gives a knockout replacement and
/// two evolution targets without flooding the bench with prize fodder.
const TARGET_BENCH: usize = 3;

/// Below this fraction of max HP the active is treated as about to be knocked
/// out. The view does not expose the opponent's attacks, so this is the only
/// honest danger signal available.
const DANGER_HP_FRACTION: f32 = 0.34;

impl ReferencePolicyV2 {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }

    // ---------------------------------------------------------------- utility

    /// Deterministic tie-break. Never used to choose between options of
    /// different value — only to order equals so the result is reproducible.
    fn tiebreak(&self, id: CardInstanceId) -> u64 {
        let mut x = id.value() ^ self.seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        x ^= x >> 30;
        x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        x ^= x >> 27;
        x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
        x ^ (x >> 31)
    }

    /// Damage counters are counters, not damage: the engine knocks out at
    /// `damage_counters * 10 >= hp`. reference_policy_v1 subtracted the raw
    /// counter from HP, so it thought a 60 HP Pokemon with 50 damage on it had
    /// 55 HP left instead of 10 — which silently disabled its lethal check.
    fn remaining_hp(pokemon: &PokemonView) -> i32 {
        (pokemon.hp as i32 - pokemon.damage_counters as i32 * 10).max(0)
    }

    fn health_fraction(pokemon: &PokemonView) -> f32 {
        if pokemon.hp == 0 {
            return 0.0;
        }
        Self::remaining_hp(pokemon) as f32 / pokemon.hp as f32
    }

    fn find_mine<'a>(view: &'a GameView, id: CardInstanceId) -> Option<&'a PokemonView> {
        view.my_active
            .as_ref()
            .filter(|p| p.card.id == id)
            .or_else(|| view.my_bench.iter().find(|p| p.card.id == id))
    }

    fn find_theirs<'a>(view: &'a GameView, id: CardInstanceId) -> Option<&'a PokemonView> {
        view.opponent_active
            .as_ref()
            .filter(|p| p.card.id == id)
            .or_else(|| view.opponent_bench.iter().find(|p| p.card.id == id))
    }

    fn is_my_active(view: &GameView, id: CardInstanceId) -> bool {
        view.my_active.as_ref().map(|p| p.card.id == id).unwrap_or(false)
    }

    // ----------------------------------------------------------------- combat

    /// Damage the engine will actually apply: weakness multiplies, resistance
    /// subtracts, in that order. `calculate_damage_with_flags` is the authority
    /// this mirrors.
    fn expected_damage(attack: &Attack, defender: Option<&PokemonView>) -> i32 {
        let mut damage = attack.damage as i32;
        if let Some(defender) = defender {
            if let Some(weakness) = defender.weakness {
                if weakness.type_ == attack.attack_type {
                    damage = damage.saturating_mul(weakness.multiplier as i32);
                }
            }
            if let Some(resistance) = defender.resistance {
                if resistance.type_ == attack.attack_type {
                    damage -= resistance.value as i32;
                }
            }
        }
        damage.max(0)
    }

    fn is_lethal(attack: &Attack, defender: Option<&PokemonView>) -> bool {
        match defender {
            Some(defender) => Self::expected_damage(attack, Some(defender)) >= Self::remaining_hp(defender),
            None => false,
        }
    }

    /// Rung 1 and rung 7 in one score. A knockout dominates everything, then
    /// raw expected damage, then attacks that carry an effect, then cheapness.
    fn attack_score(&self, attack: &Attack, defender: Option<&PokemonView>) -> i64 {
        let damage = Self::expected_damage(attack, defender) as i64;
        let mut score = damage * 100;
        if Self::is_lethal(attack, defender) {
            score += 1_000_000;
            // Among lethal attacks prefer the cheapest, so energy survives for
            // the next attacker instead of being spent on overkill.
            score -= attack.cost.total_energy as i64 * 40;
        } else {
            if attack.effect_ast.is_some() {
                score += 150;
            }
            score -= attack.cost.total_energy as i64 * 20;
        }
        if let Some(defender) = defender {
            // ex Pokemon give up two prizes: pressuring them is worth more.
            if defender.is_ex && damage > 0 {
                score += 120;
            }
        }
        score
    }

    fn best_attack(&self, attacks: &[Attack], defender: Option<&PokemonView>) -> Option<Attack> {
        attacks
            .iter()
            .max_by(|a, b| {
                self.attack_score(a, defender)
                    .cmp(&self.attack_score(b, defender))
                    .then_with(|| a.damage.cmp(&b.damage))
                    .then_with(|| b.cost.total_energy.cmp(&a.cost.total_energy))
                    .then_with(|| b.name.cmp(&a.name))
            })
            .cloned()
    }

    // ----------------------------------------------------------------- energy

    /// The engine normalises every basic energy to `ENERGY-<TYPE>`. Reading that
    /// is reading the engine's own type system, not memorising a card list.
    fn energy_type_of(def_id: &str) -> Option<Type> {
        let upper = def_id.to_ascii_uppercase();
        let name = match upper.strip_prefix("ENERGY-") {
            Some(rest) => rest.to_string(),
            None => match upper.rsplit_once("-ENERGY") {
                Some((head, _)) if !head.is_empty() => head.to_string(),
                _ => return None,
            },
        };
        match name.as_str() {
            "GRASS" => Some(Type::Grass),
            "FIRE" => Some(Type::Fire),
            "WATER" => Some(Type::Water),
            "LIGHTNING" => Some(Type::Lightning),
            "PSYCHIC" => Some(Type::Psychic),
            "FIGHTING" => Some(Type::Fighting),
            "DARKNESS" | "DARK" => Some(Type::Darkness),
            "METAL" => Some(Type::Metal),
            "COLORLESS" | "DOUBLECOLORLESS" | "DOUBLE_COLORLESS" => Some(Type::Colorless),
            _ => None,
        }
    }

    fn type_index(t: Type) -> usize {
        match t {
            Type::Grass => 0,
            Type::Fire => 1,
            Type::Water => 2,
            Type::Lightning => 3,
            Type::Psychic => 4,
            Type::Fighting => 5,
            Type::Darkness => 6,
            Type::Metal => 7,
            Type::Colorless => 8,
        }
    }

    fn attached_type_counts(pokemon: &PokemonView) -> [usize; 9] {
        let mut counts = [0usize; 9];
        for card in &pokemon.attached_energy {
            if let Some(t) = Self::energy_type_of(card.def_id.as_str()) {
                counts[Self::type_index(t)] += 1;
            } else {
                // Special / rainbow energy: count it as colorless so it still
                // contributes to the "how close am I" estimate.
                counts[Self::type_index(Type::Colorless)] += 1;
            }
        }
        counts
    }

    /// Coloured requirements of `attack` that `counts` does not yet cover.
    fn missing_types(attack: &Attack, counts: &[usize; 9]) -> Vec<Type> {
        let mut have = *counts;
        let mut missing = Vec::new();
        for t in &attack.cost.types {
            if matches!(t, Type::Colorless) {
                continue;
            }
            let idx = Self::type_index(*t);
            if have[idx] > 0 {
                have[idx] -= 1;
            } else {
                missing.push(*t);
            }
        }
        missing
    }

    /// Rung 5. "Soonest to an attack" with the information the ABI gives us:
    /// the view exposes the *active's* payable attacks but not any Pokemon's
    /// full attack list, so distance-to-attack is proxied by energy already
    /// attached, and the active is preferred because only the active can spend
    /// it this turn.
    fn energy_attach_target(&self, view: &GameView, can_attack_now: bool) -> Option<CardInstanceId> {
        let targets = &view.action_hints.attach_targets;
        if targets.is_empty() {
            return None;
        }
        let defender = view.opponent_active.as_ref();
        let active_in_danger = view
            .my_active
            .as_ref()
            .map(|a| Self::health_fraction(a) < DANGER_HP_FRACTION)
            .unwrap_or(false);

        let mut best: Option<(i64, u64, CardInstanceId)> = None;
        for &id in targets {
            let pokemon = match Self::find_mine(view, id) {
                Some(p) => p,
                None => continue,
            };
            let energy = pokemon.attached_energy.len() as i64;
            let mut score = 0i64;
            if Self::is_my_active(view, id) {
                // The active is the only Pokemon that can convert energy into
                // damage this turn.
                score += if can_attack_now { 60 } else { 200 };
                if active_in_danger {
                    // About to be knocked out: energy invested here is discarded
                    // with it. Build the replacement instead.
                    score -= 170;
                }
            } else {
                // A benched attacker is worth building once the active is set
                // up, and is the only sane target when the active is dying.
                score += 40 + energy * 12;
                if active_in_danger {
                    score += 60;
                }
            }
            score += Self::matchup_bonus(pokemon, defender);
            if !pokemon.special_conditions.is_empty() {
                score -= 25;
            }
            let key = (score, self.tiebreak(id), id);
            best = match best {
                None => Some(key),
                Some(current) if key > current => Some(key),
                other => other,
            };
        }
        best.map(|(_, _, id)| id)
    }

    /// Type advantage of one of my Pokemon against the defending Pokemon.
    fn matchup_bonus(pokemon: &PokemonView, defender: Option<&PokemonView>) -> i64 {
        let defender = match defender {
            Some(d) => d,
            None => return 0,
        };
        let mut score = 0i64;
        if let Some(weakness) = defender.weakness {
            if pokemon.types.contains(&weakness.type_) {
                score += 30 * weakness.multiplier as i64;
            }
        }
        if let Some(resistance) = defender.resistance {
            if pokemon.types.contains(&resistance.type_) {
                score -= 20;
            }
        }
        if let Some(weakness) = pokemon.weakness {
            if defender.types.contains(&weakness.type_) {
                score -= 25;
            }
        }
        score
    }

    /// Pick the energy card that best advances `target_id` toward its attack.
    fn energy_card_for(&self, view: &GameView, target_id: CardInstanceId, best_attack: Option<&Attack>) -> Option<CardInstanceId> {
        let playable = &view.action_hints.playable_energy_ids;
        if playable.is_empty() {
            return None;
        }
        let pokemon = Self::find_mine(view, target_id);
        let counts = pokemon.map(Self::attached_type_counts).unwrap_or([0usize; 9]);

        let mut wanted: Vec<Type> = Vec::new();
        if let Some(attack) = best_attack {
            if Self::is_my_active(view, target_id) {
                wanted.extend(Self::missing_types(attack, &counts));
            }
        }
        if wanted.is_empty() {
            if let Some(pokemon) = pokemon {
                wanted.extend(pokemon.types.iter().copied());
            }
        }

        let mut best: Option<(i64, u64, CardInstanceId)> = None;
        for &id in playable {
            let card = match view.my_hand.iter().find(|c| c.id == id) {
                Some(card) => card,
                None => continue,
            };
            let ty = Self::energy_type_of(card.def_id.as_str());
            let score = match ty {
                Some(t) if wanted.contains(&t) => 300,
                // An unrecognised energy is a special/rainbow energy: flexible,
                // so it ranks above an off-colour basic but below an on-colour one.
                None => 200,
                Some(Type::Colorless) => 150,
                Some(_) => 50,
            };
            let key = (score, self.tiebreak(id), id);
            best = match best {
                None => Some(key),
                Some(current) if key > current => Some(key),
                other => other,
            };
        }
        best.map(|(_, _, id)| id)
    }

    // -------------------------------------------------------------- evolution

    /// Rung 4. Evolving raises HP and unlocks a bigger attack; the active first,
    /// because that is where the damage has to come from.
    fn evolution_action(&self, view: &GameView) -> Option<Action> {
        let hints = &view.action_hints;
        let mut best: Option<(i64, u64, CardInstanceId, CardInstanceId)> = None;
        for &card_id in &hints.playable_evolution_ids {
            let targets = match hints.evolve_targets_by_card_id.get(&card_id) {
                Some(t) => t,
                None => continue,
            };
            for &target_id in targets {
                let pokemon = match Self::find_mine(view, target_id) {
                    Some(p) => p,
                    None => continue,
                };
                let mut score = 0i64;
                if Self::is_my_active(view, target_id) {
                    score += 250;
                }
                // Energy already invested is energy the evolution inherits.
                score += pokemon.attached_energy.len() as i64 * 30;
                // Evolving heals nothing but raises max HP, so a damaged target
                // gains the most effective HP.
                score += Self::remaining_hp(pokemon) as i64 / 10;
                let key = (score, self.tiebreak(target_id), card_id, target_id);
                best = match best {
                    None => Some(key),
                    Some(current) if key > current => Some(key),
                    other => other,
                };
            }
        }
        best.map(|(_, _, card_id, target_id)| Action::EvolveFromHand { card_id, target_id })
    }

    // ----------------------------------------------------------------- retreat

    /// Rung 2. Only when staying in is strictly bad: the active cannot attack at
    /// all, or it is about to be knocked out and a benched Pokemon is at least
    /// as ready. Retreating discards energy, so it is never speculative.
    fn retreat_action(&self, view: &GameView, can_attack_now: bool) -> Option<Action> {
        let active = view.my_active.as_ref()?;
        if view.my_bench.is_empty() {
            return None;
        }
        // Retreating pays the retreat cost by DISCARDING energy off the active,
        // so a retreat that is not forced is a straight tempo loss. Measured:
        // retreating whenever the active looked endangered cost 18.3 points of
        // cell win rate (45.5% -> 63.8%) because the policy churned actives,
        // burned its energy on retreat costs and stopped attacking — its lost
        // games ran 46 turns with 16 energy attached and only 8.7 attacks.
        // So the rung fires only when staying in does nothing at all.
        if can_attack_now {
            return None;
        }
        let active_energy = active.attached_energy.len();
        let defender = view.opponent_active.as_ref();

        let mut best: Option<(i64, u64, CardInstanceId)> = None;
        for slot in &view.my_bench {
            let energy = slot.attached_energy.len();
            // Only swap into a Pokemon that is strictly further along than the
            // one being abandoned.
            if energy <= active_energy {
                continue;
            }
            let score = energy as i64 * 25
                + Self::remaining_hp(slot) as i64 / 5
                + Self::matchup_bonus(slot, defender);
            let key = (score, self.tiebreak(slot.card.id), slot.card.id);
            best = match best {
                None => Some(key),
                Some(current) if key > current => Some(key),
                other => other,
            };
        }
        best.map(|(_, _, id)| Action::Retreat { to_bench_id: id })
    }

    // ------------------------------------------------------------ board setup

    /// Rung 6. Bench toward `TARGET_BENCH`; an empty bench loses the game
    /// outright the moment the active is knocked out.
    fn play_basic_action(&self, view: &GameView) -> Option<Action> {
        if view.my_bench.len() >= TARGET_BENCH {
            return None;
        }
        let mut best: Option<(u64, CardInstanceId)> = None;
        for &id in &view.action_hints.playable_basic_ids {
            let key = (self.tiebreak(id), id);
            best = match best {
                None => Some(key),
                Some(current) if key > current => Some(key),
                other => other,
            };
        }
        best.map(|(_, card_id)| Action::PlayBasic { card_id })
    }

    /// Rung 3. Trainers are resource conversion; the policy plays them before
    /// committing energy so the extra cards are available to commit.
    fn trainer_action(&self, view: &GameView) -> Option<Action> {
        let mut best: Option<(u64, CardInstanceId)> = None;
        for &id in &view.action_hints.playable_trainer_ids {
            let key = (self.tiebreak(id), id);
            best = match best {
                None => Some(key),
                Some(current) if key > current => Some(key),
                other => other,
            };
        }
        best.map(|(_, card_id)| Action::PlayTrainer { card_id })
    }

    fn best_new_active(&self, view: &GameView, options: &[CardInstanceId]) -> Option<CardInstanceId> {
        let candidates: Vec<CardInstanceId> = if options.is_empty() {
            view.my_bench.iter().map(|p| p.card.id).collect()
        } else {
            options.to_vec()
        };
        let defender = view.opponent_active.as_ref();
        let mut best: Option<(i64, u64, CardInstanceId)> = None;
        for id in candidates {
            let pokemon = match Self::find_mine(view, id) {
                Some(p) => p,
                None => continue,
            };
            // Energy is the scarce resource: promote what can attack, not just
            // what has the most HP left.
            let score = pokemon.attached_energy.len() as i64 * 40
                + Self::remaining_hp(pokemon) as i64 / 2
                + Self::matchup_bonus(pokemon, defender);
            let key = (score, self.tiebreak(id), id);
            best = match best {
                None => Some(key),
                Some(current) if key > current => Some(key),
                other => other,
            };
        }
        best.map(|(_, _, id)| id)
    }

    // -------------------------------------------------------- prompt helpers

    /// Cards this policy is least sorry to lose, best first. Used for every
    /// "discard / return N cards" prompt.
    fn disposable_hand_cards(&self, view: &GameView, pool: &[CardInstanceId], take: usize) -> Vec<CardInstanceId> {
        let hints = &view.action_hints;
        let mut scored: Vec<(i64, u64, CardInstanceId)> = Vec::new();
        for &id in pool {
            let mut score = 0i64;
            if hints.playable_basic_ids.contains(&id) {
                score -= 30;
            }
            if hints.playable_energy_ids.contains(&id) {
                score -= 25;
            }
            if hints.playable_evolution_ids.contains(&id) {
                score -= 20;
            }
            if hints.playable_trainer_ids.contains(&id) {
                score -= 5;
            }
            // A duplicate is cheaper to lose than the only copy.
            if let Some(card) = view.my_hand.iter().find(|c| c.id == id) {
                let copies = view
                    .my_hand
                    .iter()
                    .filter(|c| c.def_id.as_str() == card.def_id.as_str())
                    .count();
                if copies > 1 {
                    score += 15;
                }
            }
            // Basics are dead cards once the bench is full.
            if view.my_bench.len() >= TARGET_BENCH && hints.playable_basic_ids.contains(&id) {
                score += 20;
            }
            scored.push((score, self.tiebreak(id), id));
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
        scored.into_iter().take(take).map(|(_, _, id)| id).collect()
    }

    /// Deterministic pick of `take` ids, stable under repeated prompts.
    fn take_stable(&self, options: &[CardInstanceId], take: usize) -> Vec<CardInstanceId> {
        let mut ids: Vec<(u64, CardInstanceId)> =
            options.iter().map(|&id| (self.tiebreak(id), id)).collect();
        ids.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        ids.into_iter().take(take.min(options.len())).map(|(_, id)| id).collect()
    }

    /// Searching a deck: take energy when we have none in hand, otherwise take
    /// anything else (the search is usually looking for a Pokemon or a trainer).
    fn pick_from_deck(&self, view: &GameView, options: &[CardInstanceId], revealed: &[(CardInstanceId, String)], take: usize) -> Vec<CardInstanceId> {
        let need_energy = view.action_hints.playable_energy_ids.is_empty();
        let mut energy: Vec<CardInstanceId> = Vec::new();
        let mut other: Vec<CardInstanceId> = Vec::new();
        for &id in options {
            let is_energy = revealed
                .iter()
                .find(|(rid, _)| *rid == id)
                .map(|(_, def_id)| Self::energy_type_of(def_id).is_some())
                .unwrap_or(false);
            if is_energy {
                energy.push(id);
            } else {
                other.push(id);
            }
        }
        let preferred = if need_energy && !energy.is_empty() {
            &energy
        } else if !other.is_empty() {
            &other
        } else {
            return self.take_stable(options, take);
        };
        let mut picked = self.take_stable(preferred, take);
        if picked.len() < take {
            for id in self.take_stable(options, options.len()) {
                if picked.len() >= take {
                    break;
                }
                if !picked.contains(&id) {
                    picked.push(id);
                }
            }
        }
        picked
    }

    /// Recovering from the discard: energy first, it is the resource that
    /// actually gates attacking.
    fn pick_from_discard(&self, view: &GameView, options: &[CardInstanceId], take: usize) -> Vec<CardInstanceId> {
        let mut energy: Vec<CardInstanceId> = Vec::new();
        for &id in options {
            if let Some(card) = view.my_discard.iter().find(|c| c.id == id) {
                if Self::energy_type_of(card.def_id.as_str()).is_some() {
                    energy.push(id);
                }
            }
        }
        let mut picked = self.take_stable(&energy, take);
        if picked.len() < take {
            for id in self.take_stable(options, options.len()) {
                if picked.len() >= take {
                    break;
                }
                if !picked.contains(&id) {
                    picked.push(id);
                }
            }
        }
        picked
    }

    /// Targets for an in-play selection. If the option set is the opponent's,
    /// finish the most damaged one; if it is ours, pick the least invested.
    fn pick_pokemon_targets(&self, view: &GameView, options: &[CardInstanceId], take: usize) -> Vec<CardInstanceId> {
        let opponents = options
            .iter()
            .any(|&id| Self::find_theirs(view, id).is_some());
        let mut scored: Vec<(i64, u64, CardInstanceId)> = Vec::new();
        for &id in options {
            let score = if opponents {
                match Self::find_theirs(view, id) {
                    // Lowest remaining HP first: closest to a prize.
                    Some(p) => -(Self::remaining_hp(p) as i64) + if p.is_ex { 40 } else { 0 },
                    None => -10_000,
                }
            } else {
                match Self::find_mine(view, id) {
                    // Sacrifice the least invested of ours.
                    Some(p) => -(p.attached_energy.len() as i64 * 100) - Self::remaining_hp(p) as i64 / 10,
                    None => -10_000,
                }
            };
            scored.push((score, self.tiebreak(id), id));
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
        scored.into_iter().take(take).map(|(_, _, id)| id).collect()
    }

    /// Anything in play, ours or theirs, ranked by how happy we are to see it
    /// chosen. Used for effects that remove or move a card in play.
    fn pick_cards_in_play(&self, view: &GameView, options: &[CardInstanceId], take: usize) -> Vec<CardInstanceId> {
        let mut scored: Vec<(i64, u64, CardInstanceId)> = Vec::new();
        for &id in options {
            let mut score = 0i64;
            let mut hit = false;
            for (pokemon, theirs) in view
                .opponent_active
                .iter()
                .map(|p| (p, true))
                .chain(view.opponent_bench.iter().map(|p| (p, true)))
                .chain(view.my_active.iter().map(|p| (p, false)))
                .chain(view.my_bench.iter().map(|p| (p, false)))
            {
                let base = if theirs { 100 } else { -100 };
                if pokemon.card.id == id {
                    score = base + (300 - Self::remaining_hp(pokemon) as i64) / 10;
                    hit = true;
                }
                if pokemon.attached_energy.iter().any(|c| c.id == id) {
                    score = base + 40;
                    hit = true;
                }
                if pokemon.attached_tool.as_ref().map(|c| c.id == id).unwrap_or(false) {
                    score = base + 20;
                    hit = true;
                }
            }
            if !hit {
                score = -500;
            }
            scored.push((score, self.tiebreak(id), id));
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
        scored.into_iter().take(take).map(|(_, _, id)| id).collect()
    }
}

impl AiController for ReferencePolicyV2 {
    /// Every prompt the engine can raise gets a legal answer. A prompt with no
    /// legal answer stalls the game, and a stalled game scores as a loss — that
    /// was 16.5% of the previous reference's games, more than any play mistake.
    fn propose_prompt_response(&mut self, view: &GameView, prompt: &Prompt) -> Vec<Action> {
        let mut actions: Vec<Action> = Vec::new();

        match prompt {
            Prompt::ChooseStartingActive { options } => {
                // The view exposes no stats for a card still in hand, so any
                // legal basic is as good as another; offer all of them so a
                // rejected first choice cannot stall the opening.
                let mut valid: Vec<CardInstanceId> = options
                    .iter()
                    .filter(|id| view.action_hints.playable_basic_ids.contains(id))
                    .copied()
                    .collect();
                if valid.is_empty() {
                    valid = options.clone();
                }
                for card_id in self.take_stable(&valid, valid.len()) {
                    actions.push(Action::ChooseActive { card_id });
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
                let lower = (*min).min(valid.len());
                let upper = (*max).min(valid.len()).max(lower);
                let desired = TARGET_BENCH.clamp(lower, upper);
                actions.push(Action::ChooseBench { card_ids: self.take_stable(&valid, desired) });
                if desired != lower {
                    actions.push(Action::ChooseBench { card_ids: self.take_stable(&valid, lower) });
                }
                actions.push(Action::ChooseBench { card_ids: Vec::new() });
            }

            Prompt::ChooseAttack { attacks, .. } => {
                if let Some(best) = self.best_attack(attacks, view.opponent_active.as_ref()) {
                    actions.push(Action::DeclareAttack { attack: best });
                }
                for attack in attacks {
                    actions.push(Action::DeclareAttack { attack: attack.clone() });
                }
                actions.push(Action::CancelPrompt);
            }

            Prompt::ChooseCardsFromDeck { player, count, options, min, max, revealed_cards, .. } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }
                let lower = min.unwrap_or(*count).min(options.len());
                let upper = max.unwrap_or(*count).min(options.len()).max(lower);
                let take = (*count).clamp(lower, upper);
                let revealed: Vec<(CardInstanceId, String)> = revealed_cards
                    .iter()
                    .map(|card| (card.id, card.def_id.clone()))
                    .collect();
                actions.push(Action::TakeCardsFromDeck {
                    card_ids: self.pick_from_deck(view, options, &revealed, take),
                });
                if take != lower {
                    actions.push(Action::TakeCardsFromDeck {
                        card_ids: self.pick_from_deck(view, options, &revealed, lower),
                    });
                }
                actions.push(Action::TakeCardsFromDeck { card_ids: Vec::new() });
                actions.push(Action::CancelPrompt);
            }

            Prompt::ChooseCardsFromDiscard { player, count, options, min, max, .. } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }
                let lower = min.unwrap_or(*count).min(options.len());
                let upper = max.unwrap_or(*count).min(options.len()).max(lower);
                let take = (*count).clamp(lower, upper);
                actions.push(Action::TakeCardsFromDiscard {
                    card_ids: self.pick_from_discard(view, options, take),
                });
                if take != lower {
                    actions.push(Action::TakeCardsFromDiscard {
                        card_ids: self.pick_from_discard(view, options, lower),
                    });
                }
                actions.push(Action::TakeCardsFromDiscard { card_ids: Vec::new() });
            }

            Prompt::ChoosePokemonInPlay { player, options, min, max } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }
                let take = (*max).min(options.len()).max((*min).min(options.len()));
                actions.push(Action::ChoosePokemonTargets {
                    target_ids: self.pick_pokemon_targets(view, options, take),
                });
                if take != *min {
                    actions.push(Action::ChoosePokemonTargets {
                        target_ids: self.pick_pokemon_targets(view, options, (*min).min(options.len())),
                    });
                }
                actions.push(Action::ChoosePokemonTargets { target_ids: Vec::new() });
            }

            Prompt::ReorderDeckTop { player, options } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }
                // The engine does not reveal what these cards are, so any total
                // order is equally informed; keep it stable.
                actions.push(Action::ReorderDeckTop { card_ids: options.clone() });
            }

            Prompt::ChooseAttachedEnergy { player, pokemon_id, count, min, .. } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }
                let required = min.unwrap_or(*count);
                let attached: Vec<CardInstanceId> = Self::find_mine(view, *pokemon_id)
                    .map(|p| p.attached_energy.iter().map(|c| c.id).collect())
                    .unwrap_or_default();
                // When the prompt is optional it is an attack or ability
                // offering extra effect for discarded energy, and paying is
                // worth more than hoarding: declining first measured 1.1 points
                // of cell win rate worse. The empty selection stays below as the
                // fallback, so an unpayable prompt still gets a legal answer.
                if !attached.is_empty() {
                    let take = (*count).max(required).min(attached.len());
                    // Give up the energy the attacker is least likely to need:
                    // colourless first, then whatever ranks lowest deterministically.
                    let owner = Self::find_mine(view, *pokemon_id);
                    let mut ranked: Vec<(i64, u64, CardInstanceId)> = attached
                        .iter()
                        .map(|&id| {
                            let colourless = owner
                                .and_then(|p| p.attached_energy.iter().find(|c| c.id == id))
                                .and_then(|c| Self::energy_type_of(c.def_id.as_str()))
                                .map(|t| matches!(t, Type::Colorless))
                                .unwrap_or(false);
                            (if colourless { 10 } else { 0 }, self.tiebreak(id), id)
                        })
                        .collect();
                    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
                    let picked: Vec<CardInstanceId> =
                        ranked.into_iter().take(take).map(|(_, _, id)| id).collect();
                    actions.push(Action::ChooseAttachedEnergy { energy_ids: picked });
                }
                actions.push(Action::ChooseAttachedEnergy { energy_ids: Vec::new() });
                actions.push(Action::CancelPrompt);
            }

            Prompt::ChooseCardsFromHand { player, count, options, min, max, return_to_deck, .. } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }
                let pool: Vec<CardInstanceId> = if options.is_empty() {
                    view.my_hand.iter().map(|c| c.id).collect()
                } else {
                    options.clone()
                };
                let lower = min.unwrap_or(*count).min(pool.len());
                let upper = max.unwrap_or(*count).min(pool.len()).max(lower);
                let take = (*count).clamp(lower, upper);
                let mut push = |ids: Vec<CardInstanceId>, actions: &mut Vec<Action>| {
                    if *return_to_deck {
                        actions.push(Action::ReturnCardsFromHandToDeck { card_ids: ids });
                    } else {
                        actions.push(Action::DiscardCardsFromHand { card_ids: ids });
                    }
                };
                push(self.disposable_hand_cards(view, &pool, take), &mut actions);
                if take != lower {
                    push(self.disposable_hand_cards(view, &pool, lower), &mut actions);
                }
                push(Vec::new(), &mut actions);
                actions.push(Action::CancelPrompt);
            }

            Prompt::ChooseCardsInPlay { player, options, min, max } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }
                let take = (*max).min(options.len()).max((*min).min(options.len()));
                actions.push(Action::ChooseCardsInPlay {
                    card_ids: self.pick_cards_in_play(view, options, take),
                });
                if take != *min {
                    actions.push(Action::ChooseCardsInPlay {
                        card_ids: self.pick_cards_in_play(view, options, (*min).min(options.len())),
                    });
                }
                actions.push(Action::ChooseCardsInPlay { card_ids: Vec::new() });
            }

            Prompt::ChoosePrizeCards { player, options, min, max } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }
                let take = (*max).min(options.len()).max((*min).min(options.len()));
                actions.push(Action::ChoosePrizeCards { card_ids: self.take_stable(options, take) });
                if take != *min {
                    actions.push(Action::ChoosePrizeCards {
                        card_ids: self.take_stable(options, (*min).min(options.len())),
                    });
                }
                actions.push(Action::ChoosePrizeCards { card_ids: Vec::new() });
            }

            // The engine matches this prompt with `Action::ChooseDefenderAttack`,
            // NOT with `DeclareAttack`. Answering it with a DeclareAttack — as
            // the shipped opponents do — is rejected and stalls the game.
            Prompt::ChooseDefenderAttack { player, attacks, .. } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }
                for attack_name in attacks {
                    actions.push(Action::ChooseDefenderAttack { attack_name: attack_name.clone() });
                }
            }

            Prompt::ChoosePokemonAttack { player, attacks, .. } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }
                for attack_name in attacks {
                    actions.push(Action::ChoosePokemonAttack { attack_name: attack_name.clone() });
                }
            }

            Prompt::ChooseSpecialCondition { player, options } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }
                for condition in options {
                    actions.push(Action::ChooseSpecialCondition { condition: *condition });
                }
            }

            Prompt::ChooseNewActive { player, options } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }
                if let Some(card_id) = self.best_new_active(view, options) {
                    actions.push(Action::ChooseNewActive { card_id });
                }
                let fallback: Vec<CardInstanceId> = if options.is_empty() {
                    view.my_bench.iter().map(|p| p.card.id).collect()
                } else {
                    options.clone()
                };
                for card_id in fallback {
                    actions.push(Action::ChooseNewActive { card_id });
                }
            }

            // The remaining `Prompt` variants are declared by the engine's type
            // but never raised by it. If that changes, the runner's appended
            // EndTurn is the fallback and the stall shows up in the sweep's
            // stall fraction rather than silently skewing a score.
            _ => {}
        }

        actions
    }

    /// The ladder. The runner applies the first legal action and calls again,
    /// so an earlier rung that has already been taken this turn simply stops
    /// being legal and play falls through to the next one.
    fn propose_free_actions(&mut self, view: &GameView) -> Vec<Action> {
        if view.current_player != view.player_id {
            return Vec::new();
        }
        if view.pending_prompt.is_some() {
            return Vec::new();
        }

        let hints = &view.action_hints;
        let defender = view.opponent_active.as_ref();
        let best = self.best_attack(&hints.usable_attacks, defender);
        let can_attack_now = hints.can_declare_attack && !hints.usable_attacks.is_empty();
        let lethal = best.as_ref().map(|a| Self::is_lethal(a, defender)).unwrap_or(false);

        let mut actions: Vec<Action> = Vec::new();

        // Rung 1: a knockout available now wins the prize race. Take it before
        // anything that could change which attacks are available — attaching one
        // more energy cannot remove an attack, so it stays legal above it, but
        // evolving or retreating can.
        if lethal && can_attack_now {
            if let Some(target_id) = self.energy_attach_target(view, true) {
                if let Some(energy_id) = self.energy_card_for(view, target_id, best.as_ref()) {
                    actions.push(Action::AttachEnergy { energy_id, target_id });
                }
            }
            if let Some(attack) = best {
                actions.push(Action::DeclareAttack { attack });
            }
            actions.push(Action::EndTurn);
            return actions;
        }

        // Rung 2: forced response.
        if let Some(action) = self.retreat_action(view, can_attack_now) {
            actions.push(action);
        }

        // Rung 3: convert cards into resources before spending them.
        if let Some(action) = self.trainer_action(view) {
            actions.push(action);
        }

        // Rung 4: evolution.
        if let Some(action) = self.evolution_action(view) {
            actions.push(action);
        }

        // Rung 5: energy onto whoever is closest to attacking.
        if let Some(target_id) = self.energy_attach_target(view, can_attack_now) {
            if let Some(energy_id) = self.energy_card_for(view, target_id, best.as_ref()) {
                actions.push(Action::AttachEnergy { energy_id, target_id });
            }
        }

        // Rung 6: board development.
        if let Some(action) = self.play_basic_action(view) {
            actions.push(action);
        }

        // Rung 7: best attack after weakness and resistance.
        if let Some(attack) = best {
            actions.push(Action::DeclareAttack { attack });
        }

        // Rung 8.
        actions.push(Action::EndTurn);
        actions
    }
}
