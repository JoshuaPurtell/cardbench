use std::collections::HashMap;

use rand::Rng;
use rand::SeedableRng;
use rand::seq::SliceRandom;
use rand_chacha::ChaCha8Rng;

use tcg_core::{
    Action, Attack, AttackCost, CardInstance, CardInstanceId, GameState, GameView, PlayerId,
    PokemonView, Prompt, SelectionDestination, Type, can_execute,
};

use crate::traits::AiController;

pub struct RandomAiV4 {
    rng: ChaCha8Rng,
    /// Prompt kinds this AI could not answer. Callers must treat any entry as a
    /// dropped match, never as a silent `EndTurn`.
    unanswered: Vec<String>,
    /// Poké-Power names per card def id (e.g. `"CG-10" -> ["Excavate"]`). The view
    /// does not list powers, so `propose_free_actions_for_game` needs this to use them.
    power_names: HashMap<String, Vec<String>>,
}

/// Bench slots per player in the EX-era rules.
const BENCH_LIMIT: usize = 5;

/// The `Prompt` variant name, e.g. `"CoinFlipForEffect"`.
pub fn prompt_kind(prompt: &Prompt) -> String {
    format!("{prompt:?}")
        .split(|c: char| c == ' ' || c == '{' || c == '(')
        .next()
        .unwrap_or("Prompt")
        .to_string()
}

impl RandomAiV4 {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: ChaCha8Rng::seed_from_u64(seed),
            unanswered: Vec::new(),
            power_names: HashMap::new(),
        }
    }

    /// Poké-Power names per card def id, used by `propose_free_actions_for_game`.
    pub fn with_power_names(mut self, power_names: HashMap<String, Vec<String>>) -> Self {
        self.power_names = power_names;
        self
    }

    /// Free actions for `player`, each checked against the live game with
    /// `tcg_core::can_execute`. Per-turn limits (one Energy attach, one retreat, one
    /// Supporter), unaffordable attacks and other illegal actions are never proposed.
    /// Adds Tool, Stadium, Poké-Power and retreat candidates that the view-only
    /// strategy cannot see. Reads only `player`'s own board, hand and card metadata.
    /// The first action is the one to play; `EndTurn` is always last.
    pub fn propose_free_actions_for_game(
        &mut self,
        game: &GameState,
        player: PlayerId,
    ) -> Vec<Action> {
        if game.turn.player != player || game.pending_prompt.is_some() {
            return Vec::new();
        }
        let view = game.view_for_player(player);
        let plan: Vec<Action> = self
            .propose_free_actions(&view)
            .into_iter()
            .filter(|a| !matches!(a, Action::EndTurn))
            .collect();

        // (probability of trying it before the main plan, action)
        let mut extras: Vec<(f64, Action)> = Vec::new();
        let own: Vec<&PokemonView> = view.my_active.iter().chain(view.my_bench.iter()).collect();
        for pokemon in &own {
            if let Some(names) = self.power_names.get(pokemon.card.def_id.as_str()) {
                for name in names {
                    // Printed Poké-Powers are once during your turn; never repeat one the
                    // engine already recorded, even if the installed hooks don't enforce it.
                    if game.power_used_this_turn(pokemon.card.id, name) {
                        continue;
                    }
                    extras.push((
                        0.8,
                        Action::UsePower {
                            source_id: pokemon.card.id,
                            power_name: name.clone(),
                        },
                    ));
                }
            }
        }
        for card in &view.my_hand {
            let Some(meta) = game.card_meta.get(&card.def_id) else {
                continue;
            };
            let kind = meta.trainer_kind.as_deref();
            if meta.is_tool || kind == Some("Tool") {
                for pokemon in &own {
                    if pokemon.attached_tool.is_none() {
                        extras.push((
                            0.6,
                            Action::AttachTool {
                                tool_id: card.id,
                                target_id: pokemon.card.id,
                            },
                        ));
                    }
                }
            } else if meta.is_stadium || kind == Some("Stadium") {
                extras.push((0.6, Action::PlayStadium { card_id: card.id }));
            }
        }
        if let Some(retreat) = self.choose_retreat_action(&view) {
            let hurt = view
                .my_active
                .as_ref()
                .map(|a| !a.special_conditions.is_empty())
                .unwrap_or(false)
                || self
                    .active_health_ratio(&view)
                    .map(|r| r < 0.35)
                    .unwrap_or(false);
            extras.push((if hurt { 0.5 } else { 0.05 }, retreat));
        }

        extras.shuffle(&mut self.rng);
        let mut first = Vec::new();
        let mut later = Vec::new();
        for (p, action) in extras {
            if self.rng.gen_bool(p) {
                first.push(action);
            } else {
                later.push(action);
            }
        }

        let mut out: Vec<Action> = Vec::new();
        let mut seen: Vec<String> = Vec::new();
        for action in first.into_iter().chain(plan).chain(later) {
            let key = format!("{action:?}");
            if seen.contains(&key) || can_execute(game, &action).is_err() {
                continue;
            }
            seen.push(key);
            out.push(action);
        }
        out.push(Action::EndTurn);
        out
    }

    /// Prompt kinds seen with no proposed action, in order.
    pub fn unanswered(&self) -> &[String] {
        &self.unanswered
    }

    /// Own-effect targets (heals, switches): most damaged own Pokémon first.
    /// Opponent targets: the Defending Pokémon if valid, then the most damaged bench.
    /// Takes at least one target when allowed, never more than `max`.
    fn rank_pokemon_targets(
        &mut self,
        view: &GameView,
        valid: &[CardInstanceId],
        min: usize,
        max: usize,
    ) -> Vec<CardInstanceId> {
        if max == 0 || valid.is_empty() {
            return Vec::new();
        }
        let own: Vec<&PokemonView> = view.my_active.iter().chain(view.my_bench.iter()).collect();
        let opp: Vec<&PokemonView> = view
            .opponent_active
            .iter()
            .chain(view.opponent_bench.iter())
            .collect();
        let damage = |id: CardInstanceId| {
            own.iter()
                .chain(opp.iter())
                .find(|p| p.card.id == id)
                .map(|p| p.damage_counters)
                .unwrap_or(0)
        };
        let is_own = |id: CardInstanceId| own.iter().any(|p| p.card.id == id);
        let opp_active = view.opponent_active.as_ref().map(|p| p.card.id);
        let mut ranked: Vec<CardInstanceId> = valid.to_vec();
        ranked.shuffle(&mut self.rng);
        if ranked.iter().all(|id| is_own(*id)) {
            ranked.sort_by_key(|id| std::cmp::Reverse(damage(*id)));
        } else {
            ranked.sort_by_key(|id| {
                (
                    is_own(*id),
                    Some(*id) != opp_active,
                    std::cmp::Reverse(damage(*id)),
                )
            });
        }
        let want = min.max(1).min(max).min(ranked.len());
        ranked.truncate(want);
        ranked
    }

    fn dummy_attack() -> Attack {
        Attack {
            name: String::new(),
            damage: 0,
            attack_type: Type::Colorless,
            cost: AttackCost {
                total_energy: 0,
                types: Vec::new(),
            },
            effect_ast: None,
        }
    }

    fn choose_k(&mut self, options: &[CardInstanceId], k: usize) -> Vec<CardInstanceId> {
        let mut ids: Vec<_> = options.to_vec();
        ids.shuffle(&mut self.rng);
        ids.truncate(k);
        ids
    }

    fn choose_one(&mut self, options: &[CardInstanceId]) -> Option<CardInstanceId> {
        options.choose(&mut self.rng).copied()
    }

    fn energy_type_from_def_id(def_id: &str) -> Option<Type> {
        let raw = def_id.to_ascii_uppercase();
        let trimmed = raw.strip_prefix("ENERGY-").unwrap_or(&raw);
        match trimmed {
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

    fn energy_type_from_card(card: &CardInstance) -> Option<Type> {
        let normalized = card.def_id.normalize_energy_id();
        if let Some(norm) = normalized {
            return Self::energy_type_from_def_id(norm.as_str());
        }
        Self::energy_type_from_def_id(card.def_id.as_str())
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

    fn find_my_pokemon<'a>(
        view: &'a GameView,
        pokemon_id: CardInstanceId,
    ) -> Option<&'a PokemonView> {
        if let Some(active) = view.my_active.as_ref().filter(|p| p.card.id == pokemon_id) {
            return Some(active);
        }
        view.my_bench.iter().find(|p| p.card.id == pokemon_id)
    }

    fn is_my_active(view: &GameView, pokemon_id: CardInstanceId) -> bool {
        view.my_active
            .as_ref()
            .map(|p| p.card.id == pokemon_id)
            .unwrap_or(false)
    }

    fn find_opponent_pokemon<'a>(
        view: &'a GameView,
        pokemon_id: CardInstanceId,
    ) -> Option<&'a PokemonView> {
        if let Some(active) = view
            .opponent_active
            .as_ref()
            .filter(|p| p.card.id == pokemon_id)
        {
            return Some(active);
        }
        view.opponent_bench.iter().find(|p| p.card.id == pokemon_id)
    }

    fn remaining_hp(view: &PokemonView) -> i32 {
        let damage = view.damage_counters as i32 * 10;
        let remaining = view.hp as i32 - damage;
        remaining.max(0)
    }

    fn opponent_active(view: &GameView) -> Option<&PokemonView> {
        view.opponent_active.as_ref()
    }

    fn matchup_score_against_active(
        pokemon: &PokemonView,
        opponent_active: Option<&PokemonView>,
    ) -> i32 {
        let mut score = 0;
        if let Some(opponent) = opponent_active {
            if let Some(weakness) = opponent.weakness {
                if pokemon.types.contains(&weakness.type_) {
                    score += 25 * weakness.multiplier as i32;
                }
            }
            if let Some(resistance) = opponent.resistance {
                if pokemon.types.contains(&resistance.type_) {
                    score -= 15;
                }
            }
            if let Some(weakness) = pokemon.weakness {
                if opponent.types.contains(&weakness.type_) {
                    score -= 20;
                }
            }
        }
        score
    }

    fn expected_damage_vs(&self, attack: &Attack, defender: Option<&PokemonView>) -> i32 {
        let mut damage = attack.damage as i32;
        if let Some(defender) = defender {
            if let Some(weakness) = defender.weakness {
                if weakness.type_ == attack.attack_type {
                    damage = damage.saturating_mul(weakness.multiplier as i32);
                }
            }
            if let Some(resistance) = defender.resistance {
                if resistance.type_ == attack.attack_type {
                    damage = (damage - resistance.value as i32).max(0);
                }
            }
        }
        damage.max(0)
    }

    fn attack_score(&self, attack: &Attack, defender: Option<&PokemonView>) -> i64 {
        let expected_damage = self.expected_damage_vs(attack, defender) as i64;
        let cost = attack.cost.total_energy as i64;
        let effect_bonus = if attack.effect_ast.is_some() { 250 } else { 0 };
        let mut score = expected_damage * 100 - cost * 20 + effect_bonus;
        if let Some(defender) = defender {
            let remaining = Self::remaining_hp(defender) as i64;
            if expected_damage >= remaining {
                score += 100_000;
            }
            if defender.is_ex {
                score += 150;
            }
        }
        score
    }

    fn best_attack_for_view(&self, view: &GameView, attacks: &[Attack]) -> Option<Attack> {
        let defender = Self::opponent_active(view);
        attacks.iter().cloned().max_by(|a, b| {
            self.attack_score(a, defender)
                .cmp(&self.attack_score(b, defender))
                .then_with(|| b.damage.cmp(&a.damage))
                .then_with(|| b.cost.total_energy.cmp(&a.cost.total_energy))
                .then_with(|| b.cost.types.len().cmp(&a.cost.types.len()))
                .then_with(|| a.name.cmp(&b.name))
        })
    }

    fn attached_energy_count_on(&self, view: &GameView, pokemon_id: CardInstanceId) -> usize {
        if let Some(active) = view.my_active.as_ref().filter(|p| p.card.id == pokemon_id) {
            return active.attached_energy.len();
        }
        if let Some(slot) = view.my_bench.iter().find(|p| p.card.id == pokemon_id) {
            return slot.attached_energy.len();
        }
        0
    }

    fn attached_energy_type_counts(pokemon: &PokemonView) -> ([usize; 9], usize) {
        let mut counts = [0usize; 9];
        let mut unknown = 0usize;
        for energy in &pokemon.attached_energy {
            if let Some(t) = Self::energy_type_from_card(energy) {
                counts[Self::type_index(t)] += 1;
            } else {
                unknown += 1;
            }
        }
        (counts, unknown)
    }

    fn missing_energy_types_for_attack(attack: &Attack, counts: &[usize; 9]) -> Vec<Type> {
        let mut req_counts = [0usize; 9];
        for t in &attack.cost.types {
            req_counts[Self::type_index(*t)] += 1;
        }
        let mut missing = Vec::new();
        for (idx, req) in req_counts.iter().enumerate() {
            if *req > counts[idx] {
                let diff = req - counts[idx];
                for _ in 0..diff {
                    let t = match idx {
                        0 => Type::Grass,
                        1 => Type::Fire,
                        2 => Type::Water,
                        3 => Type::Lightning,
                        4 => Type::Psychic,
                        5 => Type::Fighting,
                        6 => Type::Darkness,
                        7 => Type::Metal,
                        _ => Type::Colorless,
                    };
                    missing.push(t);
                }
            }
        }
        missing
    }

    fn active_health_ratio(&self, view: &GameView) -> Option<f32> {
        let active = view.my_active.as_ref()?;
        if active.hp == 0 {
            return None;
        }
        let remaining = Self::remaining_hp(active) as f32;
        Some(remaining / active.hp as f32)
    }

    fn energy_candidates(&self, view: &GameView) -> Vec<(CardInstanceId, Option<Type>)> {
        let mut out = Vec::new();
        for energy_id in &view.action_hints.playable_energy_ids {
            let card = match view.my_hand.iter().find(|c| c.id == *energy_id) {
                Some(card) => card,
                None => continue,
            };
            out.push((*energy_id, Self::energy_type_from_card(card)));
        }
        out
    }

    fn choose_energy_for_target(
        &mut self,
        view: &GameView,
        target_id: CardInstanceId,
        best_attack: Option<&Attack>,
    ) -> Option<CardInstanceId> {
        let candidates = self.energy_candidates(view);
        if candidates.is_empty() {
            return None;
        }

        let pokemon = Self::find_my_pokemon(view, target_id);
        let (counts, _) = pokemon
            .map(Self::attached_energy_type_counts)
            .unwrap_or(([0usize; 9], 0));

        let mut preferred_types: Vec<Type> = Vec::new();
        if let Some(attack) = best_attack {
            if Self::is_my_active(view, target_id) {
                let missing = Self::missing_energy_types_for_attack(attack, &counts);
                if !missing.is_empty() {
                    preferred_types.extend(missing);
                }
            }
        }
        if preferred_types.is_empty() {
            if let Some(pokemon) = pokemon {
                preferred_types.extend(pokemon.types.iter().copied());
            }
        }

        let mut matching: Vec<CardInstanceId> = Vec::new();
        let mut known: Vec<CardInstanceId> = Vec::new();
        let mut unknown: Vec<CardInstanceId> = Vec::new();

        for (id, ty) in candidates {
            if let Some(t) = ty {
                known.push(id);
                if preferred_types.contains(&t) {
                    matching.push(id);
                }
            } else {
                unknown.push(id);
            }
        }

        if !matching.is_empty() {
            return matching.choose(&mut self.rng).copied();
        }
        if !known.is_empty() {
            return known.choose(&mut self.rng).copied();
        }
        unknown.choose(&mut self.rng).copied()
    }

    fn choose_energy_attach_target(
        &mut self,
        view: &GameView,
        attach_targets: &[CardInstanceId],
        can_attack_now: bool,
    ) -> Option<CardInstanceId> {
        if attach_targets.is_empty() {
            return None;
        }

        let active_unhealthy = view
            .my_active
            .as_ref()
            .map(|a| !a.special_conditions.is_empty())
            .unwrap_or(false)
            || self
                .active_health_ratio(view)
                .map(|r| r < 0.45)
                .unwrap_or(false);

        let opponent_active = Self::opponent_active(view);
        let mut scored: Vec<(i32, CardInstanceId)> = Vec::new();
        for &target_id in attach_targets {
            let pokemon = match Self::find_my_pokemon(view, target_id) {
                Some(pokemon) => pokemon,
                None => continue,
            };
            let energy = pokemon.attached_energy.len() as i32;
            let mut score = Self::matchup_score_against_active(pokemon, opponent_active);
            if Self::is_my_active(view, target_id) {
                score += if can_attack_now { 25 } else { 80 };
                if active_unhealthy {
                    score -= 30;
                }
                if !can_attack_now {
                    score += (3 - energy.min(3)) * 6;
                }
            } else {
                score += 35;
                if can_attack_now {
                    if active_unhealthy {
                        score += energy * 4;
                    } else {
                        score += (3 - energy.min(3)) * 4;
                    }
                } else if active_unhealthy {
                    score += energy * 3;
                } else {
                    score -= energy * 2;
                }
            }
            if !pokemon.special_conditions.is_empty() {
                score -= 10;
            }
            scored.push((score, target_id));
        }

        if scored.is_empty() {
            return self.choose_one(attach_targets);
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0));
        let best_score = scored[0].0;
        let best_ids: Vec<CardInstanceId> = scored
            .into_iter()
            .filter(|(score, _)| *score == best_score)
            .map(|(_, id)| id)
            .collect();
        best_ids.choose(&mut self.rng).copied()
    }

    fn choose_new_active_best(
        &mut self,
        view: &GameView,
        options: &[CardInstanceId],
    ) -> Option<CardInstanceId> {
        let candidates: Vec<CardInstanceId> = if !options.is_empty() {
            options.to_vec()
        } else {
            view.my_bench.iter().map(|p| p.card.id).collect()
        };
        if candidates.is_empty() {
            return None;
        }

        let mut scored: Vec<(i32, i32, CardInstanceId)> = Vec::new();
        let opponent_active = Self::opponent_active(view);
        for &pid in &candidates {
            let energy = self.attached_energy_count_on(view, pid);
            let remaining = Self::find_my_pokemon(view, pid)
                .map(Self::remaining_hp)
                .unwrap_or(0);
            let matchup = Self::find_my_pokemon(view, pid)
                .map(|p| Self::matchup_score_against_active(p, opponent_active))
                .unwrap_or(0);
            let score = (energy as i32) * 25 + remaining + matchup * 10;
            scored.push((score, remaining, pid));
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
        scored.first().map(|(_, _, pid)| *pid)
    }

    fn should_play_basic_now(&mut self, view: &GameView) -> bool {
        let bench_n = view.my_bench.len();
        if bench_n < 3 {
            return true;
        }
        if bench_n < 5 {
            return self.rng.gen_bool(0.25);
        }
        false
    }

    fn pick_hand_cards_disposable(
        &mut self,
        view: &GameView,
        pool: &[CardInstanceId],
        take: usize,
    ) -> Vec<CardInstanceId> {
        let hints = &view.action_hints;
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for card in &view.my_hand {
            *counts.entry(card.def_id.as_str()).or_insert(0) += 1;
        }

        let mut scored: Vec<(i32, CardInstanceId)> = pool
            .iter()
            .copied()
            .map(|id| {
                let mut s: i32 = 0;

                if !hints.playable_basic_ids.contains(&id) {
                    s += 10;
                } else {
                    s -= 10;
                }

                if !hints.playable_energy_ids.contains(&id) {
                    s += 6;
                } else {
                    s -= 6;
                }

                if view.my_bench.len() >= 3 && hints.playable_basic_ids.contains(&id) {
                    s += 4;
                }

                if let Some(card) = view.my_hand.iter().find(|c| c.id == id) {
                    if let Some(count) = counts.get(card.def_id.as_str()) {
                        if *count > 1 {
                            s += 8;
                        }
                    }
                }

                s += self.rng.gen_range(0..3);

                (s, id)
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));
        scored.into_iter().take(take).map(|(_, id)| id).collect()
    }

    fn pick_opponent_targets_low_hp(
        &mut self,
        view: &GameView,
        options: &[CardInstanceId],
        take: usize,
    ) -> Vec<CardInstanceId> {
        let mut scored: Vec<(i32, i32, usize, CardInstanceId)> = Vec::new();
        for (idx, &pid) in options.iter().enumerate() {
            if let Some(p) = Self::find_opponent_pokemon(view, pid) {
                let remaining = Self::remaining_hp(p);
                let ex_bonus = if p.is_ex { -5 } else { 0 };
                scored.push((remaining, ex_bonus, idx, pid));
            }
        }
        if scored.is_empty() {
            return self.choose_k(options, take);
        }
        scored.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
        scored
            .into_iter()
            .take(take)
            .map(|(_, _, _, id)| id)
            .collect()
    }

    fn pick_own_targets_low_energy(
        &mut self,
        view: &GameView,
        options: &[CardInstanceId],
        take: usize,
    ) -> Vec<CardInstanceId> {
        let mut scored: Vec<(usize, i32, usize, CardInstanceId)> = Vec::new();
        for (idx, &pid) in options.iter().enumerate() {
            let energy = self.attached_energy_count_on(view, pid);
            let remaining = Self::find_my_pokemon(view, pid)
                .map(Self::remaining_hp)
                .unwrap_or(0);
            scored.push((energy, remaining, idx, pid));
        }
        scored.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)).then(a.2.cmp(&b.2)));
        scored
            .into_iter()
            .take(take)
            .map(|(_, _, _, id)| id)
            .collect()
    }

    fn choose_evolution_action(&mut self, view: &GameView) -> Option<Action> {
        let hints = &view.action_hints;
        if hints.playable_evolution_ids.is_empty() {
            return None;
        }

        let mut best: Option<(i32, CardInstanceId, CardInstanceId)> = None;

        for &card_id in &hints.playable_evolution_ids {
            let targets = match hints.evolve_targets_by_card_id.get(&card_id) {
                Some(targets) => targets,
                None => continue,
            };
            for &target_id in targets {
                let energy = self.attached_energy_count_on(view, target_id) as i32;
                let remaining = Self::find_my_pokemon(view, target_id)
                    .map(Self::remaining_hp)
                    .unwrap_or(0);
                let active_bonus = if view
                    .my_active
                    .as_ref()
                    .map(|a| a.card.id == target_id)
                    .unwrap_or(false)
                {
                    200
                } else {
                    0
                };
                let score = active_bonus + energy * 20 + remaining;
                match best {
                    None => best = Some((score, card_id, target_id)),
                    Some((best_score, _, _)) => {
                        if score > best_score {
                            best = Some((score, card_id, target_id));
                        }
                    }
                }
            }
        }

        best.map(|(_, card_id, target_id)| Action::EvolveFromHand { card_id, target_id })
    }

    fn choose_trainer_action(&mut self, view: &GameView) -> Option<Action> {
        let hints = &view.action_hints;
        let card_id = hints.playable_trainer_ids.choose(&mut self.rng).copied()?;
        Some(Action::PlayTrainer { card_id })
    }

    fn choose_basic_action(&mut self, view: &GameView) -> Option<Action> {
        let hints = &view.action_hints;
        let card_id = hints.playable_basic_ids.choose(&mut self.rng).copied()?;
        Some(Action::PlayBasic { card_id })
    }

    fn choose_retreat_action(&mut self, view: &GameView) -> Option<Action> {
        if view.my_bench.is_empty() {
            return None;
        }
        let mut scored: Vec<(i32, i32, CardInstanceId)> = Vec::new();
        let opponent_active = Self::opponent_active(view);
        for slot in &view.my_bench {
            let energy = slot.attached_energy.len();
            let remaining = Self::remaining_hp(slot);
            let matchup = Self::matchup_score_against_active(slot, opponent_active);
            let score = (energy as i32) * 20 + remaining + matchup * 8;
            scored.push((score, remaining, slot.card.id));
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
        scored
            .first()
            .map(|(_, _, id)| Action::Retreat { to_bench_id: *id })
    }

    fn pick_energy_from_discard(
        &mut self,
        view: &GameView,
        options: &[CardInstanceId],
        take: usize,
    ) -> Vec<CardInstanceId> {
        let mut energy_ids: Vec<CardInstanceId> = Vec::new();
        let mut other_ids: Vec<CardInstanceId> = Vec::new();
        for id in options {
            if let Some(card) = view.my_discard.iter().find(|c| c.id == *id) {
                if card.def_id.normalize_energy_id().is_some() {
                    energy_ids.push(*id);
                } else {
                    other_ids.push(*id);
                }
            }
        }
        if !energy_ids.is_empty() {
            let picked = self.choose_k(&energy_ids, take.min(energy_ids.len()));
            if picked.len() == take {
                return picked;
            }
            let mut out = picked;
            let more = self.choose_k(&other_ids, take.saturating_sub(out.len()));
            out.extend(more);
            return out;
        }
        self.choose_k(options, take)
    }

    fn card_in_play_score(view: &GameView, card_id: CardInstanceId) -> i32 {
        let mut score = -5;

        let mut consider_pokemon = |pokemon: &PokemonView, opponent: bool| {
            if pokemon.card.id == card_id {
                let remaining = Self::remaining_hp(pokemon);
                let base = if opponent { 60 } else { 0 };
                let hp_bonus = (200 - remaining).max(0) / 10;
                score = score.max(base + hp_bonus);
            }
            if let Some(tool) = pokemon.attached_tool.as_ref() {
                if tool.id == card_id {
                    score = score.max(if opponent { 90 } else { 10 });
                }
            }
            if pokemon.attached_energy.iter().any(|c| c.id == card_id) {
                score = score.max(if opponent { 100 } else { 20 });
            }
        };

        if let Some(active) = view.opponent_active.as_ref() {
            consider_pokemon(active, true);
        }
        for slot in &view.opponent_bench {
            consider_pokemon(slot, true);
        }
        if let Some(active) = view.my_active.as_ref() {
            consider_pokemon(active, false);
        }
        for slot in &view.my_bench {
            consider_pokemon(slot, false);
        }

        score
    }
}

impl AiController for RandomAiV4 {
    fn propose_prompt_response(&mut self, view: &GameView, prompt: &Prompt) -> Vec<Action> {
        let mut actions: Vec<Action> = Vec::new();

        match prompt {
            Prompt::ChooseStartingActive { options } => {
                let valid: Vec<_> = options
                    .iter()
                    .filter(|id| view.action_hints.playable_basic_ids.contains(id))
                    .copied()
                    .collect();

                if let Some(card_id) = valid.first().copied() {
                    actions.push(Action::ChooseActive { card_id });
                }

                let mut rest = valid;
                rest.shuffle(&mut self.rng);
                for card_id in rest {
                    actions.push(Action::ChooseActive { card_id });
                }
            }

            Prompt::ChooseBenchBasics { options, min, max } => {
                let valid: Vec<_> = options
                    .iter()
                    .filter(|id| view.action_hints.playable_basic_ids.contains(id))
                    .copied()
                    .collect();

                let required_min = (*min).min(valid.len());
                let allowed_max = (*max).min(valid.len()).max(required_min);

                let desired = if allowed_max == 0 {
                    0
                } else {
                    let goal = 3usize;
                    goal.clamp(required_min, allowed_max)
                };

                let picked = self.choose_k(&valid, desired);
                actions.push(Action::ChooseBench { card_ids: picked });

                if required_min > 0 {
                    let picked_min = self.choose_k(&valid, required_min);
                    actions.push(Action::ChooseBench {
                        card_ids: picked_min,
                    });
                }
            }

            Prompt::ChooseAttack { attacks, .. } => {
                if let Some(best) = self.best_attack_for_view(view, attacks) {
                    actions.push(Action::DeclareAttack { attack: best });
                }
                let mut shuffled = attacks.clone();
                shuffled.shuffle(&mut self.rng);
                for attack in shuffled {
                    actions.push(Action::DeclareAttack { attack });
                }
            }

            Prompt::ChooseCardsFromDeck {
                player,
                count,
                options,
                min,
                max,
                revealed_cards,
                destination,
                ..
            } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }

                let required_min = min.unwrap_or(*count);
                let required_max = max.unwrap_or(*count);

                if options.is_empty() && required_min > 0 {
                    return vec![Action::EndTurn];
                }

                // Bench searches can never take more than the free Bench slots.
                let room = if matches!(destination, SelectionDestination::Bench) {
                    BENCH_LIMIT.saturating_sub(view.my_bench.len())
                } else {
                    usize::MAX
                };
                let desired = (*count).clamp(required_min, required_max);
                let take = desired.min(options.len()).min(room);

                let mut reveal_map: HashMap<CardInstanceId, &str> = HashMap::new();
                for card in revealed_cards {
                    reveal_map.insert(card.id, card.def_id.as_str());
                }

                let need_energy = view.action_hints.playable_energy_ids.is_empty();
                let mut energy_ids = Vec::new();
                let mut other_ids = Vec::new();
                for id in options {
                    let def_id = reveal_map.get(id).copied().unwrap_or("");
                    if def_id.starts_with("ENERGY-") {
                        energy_ids.push(*id);
                    } else {
                        other_ids.push(*id);
                    }
                }

                let picked = if need_energy && !energy_ids.is_empty() {
                    self.choose_k(&energy_ids, take)
                } else if !other_ids.is_empty() {
                    self.choose_k(&other_ids, take)
                } else {
                    self.choose_k(options, take)
                };

                actions.push(Action::TakeCardsFromDeck { card_ids: picked });

                let min_take = required_min.min(options.len()).min(room);
                if min_take != take {
                    let picked_min = self.choose_k(options, min_take);
                    actions.push(Action::TakeCardsFromDeck {
                        card_ids: picked_min,
                    });
                }
            }

            Prompt::ChooseCardsFromDiscard {
                player,
                count,
                options,
                min,
                max,
                ..
            } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }

                let required_min = min.unwrap_or(*count);
                let required_max = max.unwrap_or(*count);

                if options.is_empty() && required_min > 0 {
                    return vec![Action::EndTurn];
                }

                let desired = (*count).clamp(required_min, required_max);
                let take = desired.min(options.len());
                let picked = self.pick_energy_from_discard(view, options, take);
                actions.push(Action::TakeCardsFromDiscard { card_ids: picked });

                let min_take = required_min.min(options.len());
                if min_take != take {
                    let picked_min = self.pick_energy_from_discard(view, options, min_take);
                    actions.push(Action::TakeCardsFromDiscard {
                        card_ids: picked_min,
                    });
                }
            }

            Prompt::ChoosePokemonInPlay {
                player,
                options,
                min,
                max,
            } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }
                let take = (*min).min(*max).min(options.len());
                if take == 0 {
                    actions.push(Action::ChoosePokemonTargets { target_ids: vec![] });
                    return actions;
                }

                let any_opponent = options
                    .iter()
                    .any(|id| Self::find_opponent_pokemon(view, *id).is_some());
                let picked = if any_opponent {
                    self.pick_opponent_targets_low_hp(view, options, take)
                } else {
                    self.pick_own_targets_low_energy(view, options, take)
                };
                actions.push(Action::ChoosePokemonTargets { target_ids: picked });
            }

            Prompt::ReorderDeckTop { player, options } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }
                let mut ids = options.clone();
                ids.shuffle(&mut self.rng);
                actions.push(Action::ReorderDeckTop { card_ids: ids });
            }

            Prompt::ChooseAttachedEnergy {
                player,
                pokemon_id,
                count,
                min,
                ..
            } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }

                let required_min = min.unwrap_or(*count);
                let mut energies: Vec<CardInstanceId> = Vec::new();

                if let Some(active) = view.my_active.as_ref().filter(|p| p.card.id == *pokemon_id) {
                    energies = active.attached_energy.iter().map(|c| c.id).collect();
                } else if let Some(slot) = view.my_bench.iter().find(|p| p.card.id == *pokemon_id) {
                    energies = slot.attached_energy.iter().map(|c| c.id).collect();
                }

                energies.shuffle(&mut self.rng);

                if energies.is_empty() {
                    // If the prompt is optional (min==0), choosing none is valid.
                    // If it's required (e.g. retreat cost, min==None), choosing none will be rejected.
                    // In that required case, fall back to cancelling the prompt so the AI doesn't hard-stall.
                    if required_min == 0 {
                        actions.push(Action::ChooseAttachedEnergy { energy_ids: vec![] });
                    } else {
                        actions.push(Action::CancelPrompt);
                    }
                    return actions;
                }

                if required_min == 0 && self.rng.gen_bool(0.75) {
                    actions.push(Action::ChooseAttachedEnergy { energy_ids: vec![] });
                } else {
                    let take = (*count).max(required_min).min(energies.len());
                    let mut picked = energies;
                    picked.truncate(take);
                    actions.push(Action::ChooseAttachedEnergy { energy_ids: picked });
                }
            }

            Prompt::ChooseCardsFromHand {
                player,
                count,
                options,
                min,
                max,
                return_to_deck,
                ..
            } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }

                let required_min = min.unwrap_or(*count);
                let required_max = max.unwrap_or(*count);

                let pool: Vec<CardInstanceId> = if options.is_empty() {
                    view.my_hand.iter().map(|c| c.id).collect()
                } else {
                    options.clone()
                };

                if pool.is_empty() && required_min > 0 {
                    return vec![Action::EndTurn];
                }

                let desired = (*count).clamp(required_min, required_max);
                let take = desired.min(pool.len());

                let picked = self.pick_hand_cards_disposable(view, &pool, take);

                if *return_to_deck {
                    actions.push(Action::ReturnCardsFromHandToDeck { card_ids: picked });
                } else {
                    actions.push(Action::DiscardCardsFromHand { card_ids: picked });
                }

                let min_take = required_min.min(pool.len());
                if min_take != take {
                    let picked_min = self.pick_hand_cards_disposable(view, &pool, min_take);
                    if *return_to_deck {
                        actions.push(Action::ReturnCardsFromHandToDeck {
                            card_ids: picked_min,
                        });
                    } else {
                        actions.push(Action::DiscardCardsFromHand {
                            card_ids: picked_min,
                        });
                    }
                }
            }

            Prompt::ChooseCardsInPlay {
                player,
                options,
                min,
                max,
            } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }
                let take = (*min).min(*max).min(options.len());
                if take == 0 {
                    actions.push(Action::ChooseCardsInPlay { card_ids: vec![] });
                    return actions;
                }
                let mut scored: Vec<(i32, usize, CardInstanceId)> = Vec::new();
                for (idx, id) in options.iter().enumerate() {
                    let score = Self::card_in_play_score(view, *id);
                    scored.push((score, idx, *id));
                }
                scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
                let picked: Vec<CardInstanceId> =
                    scored.into_iter().take(take).map(|(_, _, id)| id).collect();
                actions.push(Action::ChooseCardsInPlay { card_ids: picked });
            }

            Prompt::ChoosePrizeCards {
                player,
                options,
                min,
                max,
            } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }
                let take = (*min).min(*max).min(options.len());
                let picked = self.choose_k(options, take);
                actions.push(Action::ChoosePrizeCards { card_ids: picked });
            }

            // Disable / Amnesia: pick one of the Defending Pokémon's attacks to lock.
            Prompt::ChooseDefenderAttack {
                player, attacks, ..
            } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }
                let mut names = attacks.clone();
                names.shuffle(&mut self.rng);
                for attack_name in names {
                    actions.push(Action::ChooseDefenderAttack { attack_name });
                }
            }

            Prompt::ChooseNewActive { player, options } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }

                if let Some(best) = self.choose_new_active_best(view, options) {
                    actions.push(Action::ChooseNewActive { card_id: best });
                }

                let mut candidates: Vec<CardInstanceId> = if options.is_empty() {
                    view.my_bench.iter().map(|p| p.card.id).collect()
                } else {
                    options.clone()
                };

                candidates.shuffle(&mut self.rng);
                for card_id in candidates {
                    actions.push(Action::ChooseNewActive { card_id });
                }
            }

            Prompt::ChoosePokemonAttack {
                player, attacks, ..
            } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }
                if let Some(name) = attacks.iter().max().cloned() {
                    actions.push(Action::ChoosePokemonAttack { attack_name: name });
                }
                let mut names = attacks.clone();
                names.shuffle(&mut self.rng);
                for name in names {
                    actions.push(Action::ChoosePokemonAttack { attack_name: name });
                }
            }
            Prompt::ChooseSpecialCondition { player, options } => {
                if *player != view.player_id {
                    return vec![Action::EndTurn];
                }
                if let Some(condition) = options.first().copied() {
                    actions.push(Action::ChooseSpecialCondition { condition });
                }
                let mut choices = options.clone();
                choices.shuffle(&mut self.rng);
                for condition in choices {
                    actions.push(Action::ChooseSpecialCondition { condition });
                }
            }
            Prompt::ChoosePokemonTargets {
                min,
                max,
                valid_targets,
                ..
            } => {
                let target_ids = self.rank_pokemon_targets(view, valid_targets, *min, *max);
                if !target_ids.is_empty() || *min == 0 {
                    actions.push(Action::ChoosePokemonTargets { target_ids });
                }
            }
            // Legacy target prompt; same strategy, answered with the targets action.
            Prompt::ChooseTargets { count, options, .. } => {
                let target_ids = self.rank_pokemon_targets(view, options, *count, *count);
                if !target_ids.is_empty() {
                    actions.push(Action::ChoosePokemonTargets { target_ids });
                }
            }
            // Top-deck reorder: keep the current order.
            Prompt::ReorderCards { cards, .. } => {
                actions.push(Action::ReorderDeckTop {
                    card_ids: cards.clone(),
                });
            }
            // No `Action` variant answers these prompt kinds, so there is no legal
            // default: coin/reveal acknowledgements, number choices, bench
            // selection by either player, and optional discards. They fall through
            // and are recorded as unanswered below.
            Prompt::CoinFlipForEffect { .. }
            | Prompt::RevealCards { .. }
            | Prompt::SelectBenchedPokemon { .. }
            | Prompt::OpponentSelectsBenchedPokemon { .. }
            | Prompt::OptionalDiscardAttachedEnergy { .. }
            | Prompt::OptionalDiscardForEffect { .. }
            | Prompt::DiscardForDrawEffect { .. }
            | Prompt::ChooseNumber { .. }
            | Prompt::ChooseDrawCount { .. } => {}
        }

        if actions.is_empty() {
            self.unanswered.push(prompt_kind(prompt));
        }
        actions
    }

    fn unanswered_prompts(&self) -> Vec<String> {
        self.unanswered.clone()
    }

    fn propose_free_actions(&mut self, view: &GameView) -> Vec<Action> {
        if view.current_player != view.player_id {
            return Vec::new();
        }
        if view.pending_prompt.is_some() {
            return Vec::new();
        }

        let hints = &view.action_hints;
        let mut actions: Vec<Action> = Vec::new();

        let can_attack_now = hints.can_declare_attack && !hints.usable_attacks.is_empty();
        let can_attach_energy =
            !hints.playable_energy_ids.is_empty() && !hints.attach_targets.is_empty();
        let can_play_basic = !hints.playable_basic_ids.is_empty();
        let want_play_basic = can_play_basic && self.should_play_basic_now(view);
        let can_play_trainer = !hints.playable_trainer_ids.is_empty();
        let can_evolve = !hints.playable_evolution_ids.is_empty();

        // Mostly the best-scoring attack, sometimes any usable one, so attacks whose
        // damage is set by an effect (printed damage 0) still get used.
        let best_attack = if hints.usable_attacks.len() > 1 && self.rng.gen_bool(0.25) {
            hints.usable_attacks.choose(&mut self.rng).cloned()
        } else {
            self.best_attack_for_view(view, &hints.usable_attacks)
        };
        let defender = Self::opponent_active(view);
        let can_ko = best_attack
            .as_ref()
            .map(|attack| {
                self.expected_damage_vs(attack, defender)
                    >= defender.map(Self::remaining_hp).unwrap_or(9999)
            })
            .unwrap_or(false);
        let urgent_attack = can_attack_now && can_ko;

        if can_evolve && !urgent_attack {
            if let Some(action) = self.choose_evolution_action(view) {
                actions.push(action);
                if can_play_trainer {
                    if let Some(action) = self.choose_trainer_action(view) {
                        actions.push(action);
                    }
                }
                if can_attach_energy {
                    if let Some(target_id) = self.choose_energy_attach_target(
                        view,
                        &hints.attach_targets,
                        can_attack_now,
                    ) {
                        let energy_id = self
                            .choose_energy_for_target(view, target_id, best_attack.as_ref())
                            .or_else(|| self.choose_one(&hints.playable_energy_ids));
                        if let Some(energy_id) = energy_id {
                            actions.push(Action::AttachEnergy {
                                energy_id,
                                target_id,
                            });
                        }
                    }
                }
                if want_play_basic {
                    if let Some(action) = self.choose_basic_action(view) {
                        actions.push(action);
                    }
                }
                if let Some(best) = best_attack.clone() {
                    actions.push(Action::DeclareAttack { attack: best });
                } else if hints.can_declare_attack {
                    actions.push(Action::DeclareAttack {
                        attack: Self::dummy_attack(),
                    });
                }
                actions.push(Action::EndTurn);
                return actions;
            }
        }

        if can_play_trainer && !urgent_attack {
            if let Some(action) = self.choose_trainer_action(view) {
                actions.push(action);
                if can_attach_energy {
                    if let Some(target_id) = self.choose_energy_attach_target(
                        view,
                        &hints.attach_targets,
                        can_attack_now,
                    ) {
                        let energy_id = self
                            .choose_energy_for_target(view, target_id, best_attack.as_ref())
                            .or_else(|| self.choose_one(&hints.playable_energy_ids));
                        if let Some(energy_id) = energy_id {
                            actions.push(Action::AttachEnergy {
                                energy_id,
                                target_id,
                            });
                        }
                    }
                }
                if want_play_basic {
                    if let Some(action) = self.choose_basic_action(view) {
                        actions.push(action);
                    }
                }
                if let Some(best) = best_attack.clone() {
                    actions.push(Action::DeclareAttack { attack: best });
                } else if hints.can_declare_attack {
                    actions.push(Action::DeclareAttack {
                        attack: Self::dummy_attack(),
                    });
                }
                actions.push(Action::EndTurn);
                return actions;
            }
        }

        if can_attach_energy {
            if want_play_basic && can_attack_now {
                if let Some(action) = self.choose_basic_action(view) {
                    actions.push(action);
                }
            }

            if let Some(target_id) =
                self.choose_energy_attach_target(view, &hints.attach_targets, can_attack_now)
            {
                let energy_id = self
                    .choose_energy_for_target(view, target_id, best_attack.as_ref())
                    .or_else(|| self.choose_one(&hints.playable_energy_ids));
                if let Some(energy_id) = energy_id {
                    actions.push(Action::AttachEnergy {
                        energy_id,
                        target_id,
                    });
                }
            }

            if want_play_basic && !can_attack_now {
                if let Some(action) = self.choose_basic_action(view) {
                    actions.push(action);
                }
            }

            if let Some(best) = best_attack.clone() {
                actions.push(Action::DeclareAttack { attack: best });
            } else if hints.can_declare_attack {
                actions.push(Action::DeclareAttack {
                    attack: Self::dummy_attack(),
                });
            }

            actions.push(Action::EndTurn);
            return actions;
        }

        if !can_attack_now {
            if let Some(action) = self.choose_retreat_action(view) {
                actions.push(action);
            }
        }

        if want_play_basic {
            if let Some(action) = self.choose_basic_action(view) {
                actions.push(action);
            }
        }

        if let Some(best) = best_attack {
            actions.push(Action::DeclareAttack { attack: best });
        } else if hints.can_declare_attack {
            actions.push(Action::DeclareAttack {
                attack: Self::dummy_attack(),
            });
        }

        actions.push(Action::EndTurn);
        actions
    }
}

#[cfg(test)]
mod prompt_coverage_tests {
    use super::*;
    use tcg_core::{CardDefId, GameState, PlayerId, PokemonSlot};
    use tcg_rules_ex::RulesetConfig;

    struct Board {
        view: GameView,
        own: Vec<CardInstanceId>,
        opp: Vec<CardInstanceId>,
    }

    fn slot(def: &str, owner: PlayerId, damage: u16) -> PokemonSlot {
        let mut slot = PokemonSlot::new(CardInstance::new(CardDefId::new(def), owner));
        slot.hp = 100;
        slot.damage_counters = damage;
        slot
    }

    /// P1's view; index 0 of each side is the Active, the rest the Bench.
    fn board(own_damage: &[u16], opp_damage: &[u16]) -> Board {
        let mut game = GameState::new(Vec::new(), Vec::new(), 7, RulesetConfig::default());
        let mut ids = [Vec::new(), Vec::new()];
        for (side, (owner, damages)) in [(PlayerId::P1, own_damage), (PlayerId::P2, opp_damage)]
            .into_iter()
            .enumerate()
        {
            for (i, damage) in damages.iter().enumerate() {
                let s = slot(&format!("T-{side}{i}"), owner, *damage);
                ids[side].push(s.card.id);
                if i == 0 {
                    game.players[side].active = Some(s);
                } else {
                    game.players[side].bench.push(s);
                }
            }
        }
        let [own, opp] = ids;
        Board {
            view: game.view_for_player(PlayerId::P1),
            own,
            opp,
        }
    }

    fn targets(actions: &[Action]) -> Vec<CardInstanceId> {
        match actions {
            [Action::ChoosePokemonTargets { target_ids }] => target_ids.clone(),
            other => panic!("expected one ChoosePokemonTargets, got {other:?}"),
        }
    }

    fn pokemon_targets(valid: Vec<CardInstanceId>, min: usize, max: usize) -> Prompt {
        Prompt::ChoosePokemonTargets {
            player: PlayerId::P1,
            min,
            max,
            valid_targets: valid,
            effect_description: String::new(),
        }
    }

    #[test]
    fn choose_pokemon_targets_prefers_defending_pokemon() {
        let b = board(&[0], &[0, 5, 2]);
        let mut ai = RandomAiV4::new(1);
        let actions = ai.propose_prompt_response(&b.view, &pokemon_targets(b.opp.clone(), 1, 1));
        assert_eq!(targets(&actions), vec![b.opp[0]]);
        assert!(ai.unanswered().is_empty());
    }

    #[test]
    fn choose_pokemon_targets_bench_takes_most_damaged() {
        let b = board(&[0], &[0, 5, 2]);
        let mut ai = RandomAiV4::new(2);
        let actions =
            ai.propose_prompt_response(&b.view, &pokemon_targets(b.opp[1..].to_vec(), 1, 1));
        assert_eq!(targets(&actions), vec![b.opp[1]]);
    }

    #[test]
    fn choose_pokemon_targets_own_effect_takes_most_damaged_own() {
        let b = board(&[0, 3, 1], &[0]);
        let mut ai = RandomAiV4::new(3);
        let actions = ai.propose_prompt_response(&b.view, &pokemon_targets(b.own.clone(), 1, 1));
        assert_eq!(targets(&actions), vec![b.own[1]]);
    }

    #[test]
    fn choose_pokemon_targets_honours_min_and_max() {
        let b = board(&[0], &[0, 5, 2]);
        let mut ai = RandomAiV4::new(4);
        let picked =
            targets(&ai.propose_prompt_response(&b.view, &pokemon_targets(b.opp.clone(), 2, 3)));
        assert_eq!(picked.len(), 2);
        assert_eq!(picked[0], b.opp[0]);
        let none =
            targets(&ai.propose_prompt_response(&b.view, &pokemon_targets(b.opp.clone(), 0, 0)));
        assert!(none.is_empty());
        assert!(ai.unanswered().is_empty());
    }

    #[test]
    fn choose_targets_picks_count_targets() {
        let b = board(&[0], &[0, 5, 2]);
        let mut ai = RandomAiV4::new(5);
        let prompt = Prompt::ChooseTargets {
            player: PlayerId::P1,
            count: 2,
            options: b.opp.clone(),
            effect_description: String::new(),
        };
        assert_eq!(
            targets(&ai.propose_prompt_response(&b.view, &prompt)).len(),
            2
        );
    }

    #[test]
    fn reorder_cards_keeps_order() {
        let b = board(&[0], &[0, 1, 2]);
        let mut ai = RandomAiV4::new(6);
        let prompt = Prompt::ReorderCards {
            player: PlayerId::P1,
            cards: b.opp.clone(),
            destination_description: String::new(),
        };
        match &ai.propose_prompt_response(&b.view, &prompt)[..] {
            [Action::ReorderDeckTop { card_ids }] => assert_eq!(card_ids, &b.opp),
            other => panic!("expected ReorderDeckTop, got {other:?}"),
        }
    }

    /// No `Action` answers these kinds; the AI must say so instead of dropping silently.
    macro_rules! unanswerable {
        ($name:ident, $kind:literal, $prompt:expr) => {
            #[test]
            fn $name() {
                let b = board(&[0, 1], &[0, 1]);
                let mut ai = RandomAiV4::new(9);
                let prompt: Prompt = $prompt(&b);
                assert!(ai.propose_prompt_response(&b.view, &prompt).is_empty());
                assert_eq!(ai.unanswered(), &[String::from($kind)]);
                assert_eq!(
                    AiController::unanswered_prompts(&ai),
                    vec![String::from($kind)]
                );
            }
        };
    }

    unanswerable!(
        coin_flip_for_effect_is_reported,
        "CoinFlipForEffect",
        |_b: &Board| Prompt::CoinFlipForEffect {
            player: PlayerId::P1,
            effect_description: String::new(),
        }
    );
    unanswerable!(reveal_cards_is_reported, "RevealCards", |_b: &Board| {
        Prompt::RevealCards {
            player: PlayerId::P1,
            cards: Vec::new(),
            source_description: String::new(),
        }
    });
    unanswerable!(
        select_benched_pokemon_is_reported,
        "SelectBenchedPokemon",
        |_b: &Board| Prompt::SelectBenchedPokemon {
            player: PlayerId::P1,
            target_player_idx: 1,
            count: 1,
            effect_description: String::new(),
        }
    );
    unanswerable!(
        opponent_selects_benched_pokemon_is_reported,
        "OpponentSelectsBenchedPokemon",
        |_b: &Board| Prompt::OpponentSelectsBenchedPokemon {
            player: PlayerId::P1,
            effect_description: String::new(),
        }
    );
    unanswerable!(
        optional_discard_attached_energy_is_reported,
        "OptionalDiscardAttachedEnergy",
        |b: &Board| Prompt::OptionalDiscardAttachedEnergy {
            player: PlayerId::P1,
            source_id: b.own[0],
            options: Vec::new(),
            effect_description: String::new(),
            follow_up_search_zone: String::new(),
            attach_to: b.own[0],
        }
    );
    unanswerable!(
        optional_discard_for_effect_is_reported,
        "OptionalDiscardForEffect",
        |b: &Board| Prompt::OptionalDiscardForEffect {
            player: PlayerId::P1,
            options: vec![b.own[1]],
            effect_description: String::new(),
            follow_up_targets: Vec::new(),
            damage_amount: 0,
        }
    );
    unanswerable!(
        discard_for_draw_effect_is_reported,
        "DiscardForDrawEffect",
        |b: &Board| Prompt::DiscardForDrawEffect {
            player: PlayerId::P1,
            count: 1,
            options: vec![b.own[1]],
            base_draw: 1,
            bonus_draw_condition: String::new(),
            bonus_draw: 0,
        }
    );
    unanswerable!(choose_number_is_reported, "ChooseNumber", |_b: &Board| {
        Prompt::ChooseNumber {
            player: PlayerId::P1,
            min: 0,
            max: 3,
            effect_description: String::new(),
        }
    });
    unanswerable!(
        choose_draw_count_is_reported,
        "ChooseDrawCount",
        |_b: &Board| Prompt::ChooseDrawCount {
            player: PlayerId::P1,
            min: 0,
            max: 3,
        }
    );
}

#[cfg(test)]
mod game_action_tests {
    use super::*;
    use tcg_core::runtime_hooks::RuntimeHooks;
    use tcg_core::{CardDefId, CardMeta, CardMetaMap, Phase, PokemonSlot, SpecialCondition, Stage};
    use tcg_rules_ex::RulesetConfig;

    const MON: &str = "T-MON";
    const ENERGY: &str = "ENERGY-COLORLESS";
    const TOOL: &str = "T-TOOL";
    const STADIUM: &str = "T-STADIUM";
    const SUPPORTER: &str = "T-SUPPORTER";
    const POWER: &str = "Test Power";

    fn meta(name: &str) -> CardMeta {
        CardMeta {
            name: name.into(),
            is_basic: false,
            is_tool: false,
            is_stadium: false,
            is_pokemon: false,
            is_energy: false,
            hp: 0,
            energy_kind: None,
            provides: Vec::new(),
            trainer_kind: None,
            is_ex: false,
            is_star: false,
            is_delta: false,
            stage: Stage::Basic,
            types: Vec::new(),
            weakness: None,
            resistance: None,
            retreat_cost: None,
            trainer_effect: None,
            evolves_from: None,
            attacks: Vec::new(),
            card_type: String::new(),
            delta_species: false,
        }
    }

    fn big_attack() -> Attack {
        Attack {
            name: "Big Hit".into(),
            damage: 30,
            attack_type: Type::Colorless,
            cost: AttackCost {
                total_energy: 2,
                types: vec![Type::Colorless, Type::Colorless],
            },
            effect_ast: None,
        }
    }

    fn catalog() -> CardMetaMap {
        let mut m = CardMetaMap::new();
        let mut mon = meta("Mon");
        mon.is_pokemon = true;
        mon.is_basic = true;
        mon.hp = 60;
        mon.types = vec![Type::Colorless];
        mon.retreat_cost = Some(0);
        mon.attacks = vec![big_attack()];
        m.insert(CardDefId::new(MON), mon);
        let mut energy = meta("Colorless Energy");
        energy.is_energy = true;
        energy.energy_kind = Some("Basic".into());
        energy.provides = vec![Type::Colorless];
        m.insert(CardDefId::new(ENERGY), energy);
        let mut tool = meta("Test Tool");
        tool.is_tool = true;
        tool.trainer_kind = Some("Tool".into());
        m.insert(CardDefId::new(TOOL), tool);
        let mut stadium = meta("Test Stadium");
        stadium.is_stadium = true;
        stadium.trainer_kind = Some("Stadium".into());
        m.insert(CardDefId::new(STADIUM), stadium);
        let mut supporter = meta("Test Supporter");
        supporter.trainer_kind = Some("Supporter".into());
        m.insert(CardDefId::new(SUPPORTER), supporter);
        m
    }

    fn mon(owner: PlayerId) -> PokemonSlot {
        let mut slot = PokemonSlot::new(CardInstance::new(CardDefId::new(MON), owner));
        slot.hp = 60;
        slot.attacks = vec![big_attack()];
        slot.types = vec![Type::Colorless];
        slot.retreat_cost = 0;
        slot
    }

    /// P1 to act in Main, turn 3: Active + one Benched Mon each side, `hand` in P1's hand.
    fn table(hand: &[&str]) -> (GameState, Vec<CardInstanceId>) {
        let deck = |o| {
            (0..20)
                .map(|_| CardInstance::new(CardDefId::new(MON), o))
                .collect()
        };
        let mut game = GameState::new_with_card_meta(
            deck(PlayerId::P1),
            deck(PlayerId::P2),
            11,
            RulesetConfig::default(),
            catalog(),
        );
        for (i, owner) in [PlayerId::P1, PlayerId::P2].into_iter().enumerate() {
            game.players[i].active = Some(mon(owner));
            game.players[i].bench.push(mon(owner));
        }
        let mut ids = Vec::new();
        for def in hand {
            let card = CardInstance::new(CardDefId::new(*def), PlayerId::P1);
            ids.push(card.id);
            game.players[0].hand.add(card);
        }
        game.turn.player = PlayerId::P1;
        game.turn.number = 3;
        game.turn.phase = Phase::Main;
        (game, ids)
    }

    fn proposals(game: &GameState) -> Vec<Action> {
        RandomAiV4::new(21)
            .with_power_names(HashMap::from([(MON.to_string(), vec![POWER.to_string()])]))
            .propose_free_actions_for_game(game, PlayerId::P1)
    }

    fn has(actions: &[Action], pred: impl Fn(&Action) -> bool) -> bool {
        actions.iter().any(pred)
    }

    #[test]
    fn every_game_proposal_is_legal_and_ends_with_end_turn() {
        let (game, _) = table(&[ENERGY, TOOL, STADIUM, SUPPORTER]);
        let actions = proposals(&game);
        assert!(matches!(actions.last(), Some(Action::EndTurn)));
        for action in &actions {
            assert!(
                can_execute(&game, action).is_ok(),
                "illegal proposal {action:?}"
            );
        }
    }

    #[test]
    fn energy_attach_respects_one_per_turn() {
        let (mut game, _) = table(&[ENERGY]);
        assert!(has(&proposals(&game), |a| matches!(
            a,
            Action::AttachEnergy { .. }
        )));
        game.players[0].energy_attached_this_turn = true;
        assert!(!has(&proposals(&game), |a| matches!(
            a,
            Action::AttachEnergy { .. }
        )));
    }

    #[test]
    fn retreat_respects_one_per_turn() {
        let (mut game, _) = table(&[]);
        assert!(has(&proposals(&game), |a| matches!(
            a,
            Action::Retreat { .. }
        )));
        game.players[0].retreated_this_turn = true;
        assert!(!has(&proposals(&game), |a| matches!(
            a,
            Action::Retreat { .. }
        )));
    }

    #[test]
    fn supporter_respects_one_per_turn() {
        let (mut game, _) = table(&[SUPPORTER]);
        assert!(has(&proposals(&game), |a| matches!(
            a,
            Action::PlayTrainer { .. }
        )));
        game.players[0].played_supporter_this_turn = true;
        assert!(!has(&proposals(&game), |a| matches!(
            a,
            Action::PlayTrainer { .. }
        )));
    }

    #[test]
    fn unaffordable_attack_is_not_proposed() {
        let (mut game, _) = table(&[]);
        assert!(!has(&proposals(&game), |a| matches!(
            a,
            Action::DeclareAttack { .. }
        )));
        let active = game.players[0].active.as_mut().unwrap();
        for _ in 0..2 {
            active
                .attached_energy
                .push(CardInstance::new(CardDefId::new(ENERGY), PlayerId::P1));
        }
        assert!(has(&proposals(&game), |a| matches!(
            a,
            Action::DeclareAttack { .. }
        )));
    }

    #[test]
    fn tool_is_proposed_only_for_pokemon_without_a_tool() {
        let (mut game, ids) = table(&[TOOL]);
        let tool = ids[0];
        assert!(has(
            &proposals(&game),
            |a| matches!(a, Action::AttachTool { tool_id, .. } if *tool_id == tool)
        ));
        let p1 = &mut game.players[0];
        for slot in p1.active.iter_mut().chain(p1.bench.iter_mut()) {
            slot.attached_tool = Some(CardInstance::new(CardDefId::new(TOOL), PlayerId::P1));
        }
        assert!(!has(&proposals(&game), |a| matches!(
            a,
            Action::AttachTool { .. }
        )));
    }

    #[test]
    fn stadium_is_proposed_unless_same_name_in_play() {
        let (mut game, _) = table(&[STADIUM]);
        assert!(has(&proposals(&game), |a| matches!(
            a,
            Action::PlayStadium { .. }
        )));
        game.stadium_in_play = Some(CardInstance::new(CardDefId::new(STADIUM), PlayerId::P2));
        assert!(!has(&proposals(&game), |a| matches!(
            a,
            Action::PlayStadium { .. }
        )));
    }

    #[test]
    fn power_is_proposed_when_the_engine_accepts_it() {
        let (mut game, _) = table(&[]);
        // Without a power-id hook, cores differ: the converged core falls back to
        // `{def_id}:{power}`, the older pin rejects. Either way the AI must match the engine.
        let source = game.players[0].active.as_ref().unwrap().card.id;
        let bare = Action::UsePower {
            source_id: source,
            power_name: POWER.into(),
        };
        assert_eq!(
            has(
                &proposals(&game),
                |a| matches!(a, Action::UsePower { source_id, .. } if *source_id == source)
            ),
            can_execute(&game, &bare).is_ok(),
            "AI proposes a power exactly when the engine accepts it"
        );
        let mut hooks = RuntimeHooks::empty();
        hooks.power_effect_id_for = |_, name| (name == POWER).then(|| format!("T:{name}"));
        hooks.power_is_once_per_turn = |_, name| name == POWER;
        game.set_hooks(hooks);
        let actions = proposals(&game);
        assert!(has(
            &actions,
            |a| matches!(a, Action::UsePower { power_name, .. } if power_name == POWER)
        ));
        let active = game.players[0].active.as_ref().unwrap().card.id;
        game.mark_power_used(active, POWER);
        let bench = game.players[0].bench[0].card.id;
        let actions = proposals(&game);
        assert!(!has(
            &actions,
            |a| matches!(a, Action::UsePower { source_id, .. } if *source_id == active)
        ));
        assert!(has(
            &actions,
            |a| matches!(a, Action::UsePower { source_id, .. } if *source_id == bench)
        ));
        // A Pokémon with a Special Condition can't use its Poké-Power.
        let _ = game.players[0].bench[0].add_special_condition(SpecialCondition::Asleep);
        assert!(!has(
            &proposals(&game),
            |a| matches!(a, Action::UsePower { source_id, .. } if *source_id == bench)
        ));
    }

    #[test]
    fn power_is_not_repeated_in_a_turn_even_without_hook_limits() {
        let (mut game, _) = table(&[]);
        let mut hooks = RuntimeHooks::empty();
        hooks.power_effect_id_for = |_, name| (name == POWER).then(|| format!("T:{name}"));
        game.set_hooks(hooks);
        let active = game.players[0].active.as_ref().unwrap().card.id;
        assert!(has(
            &proposals(&game),
            |a| matches!(a, Action::UsePower { source_id, .. } if *source_id == active)
        ));
        game.mark_power_used(active, POWER);
        assert!(!has(
            &proposals(&game),
            |a| matches!(a, Action::UsePower { source_id, .. } if *source_id == active)
        ));
    }

    #[test]
    fn every_usable_attack_is_sometimes_chosen_first() {
        let (mut game, _) = table(&[]);
        let mut small = big_attack();
        small.name = "Small Hit".into();
        small.damage = 0;
        let active = game.players[0].active.as_mut().unwrap();
        active.attacks.push(small);
        for _ in 0..2 {
            active
                .attached_energy
                .push(CardInstance::new(CardDefId::new(ENERGY), PlayerId::P1));
        }
        let mut first_attacks = std::collections::BTreeSet::new();
        for seed in 0..60 {
            let actions = RandomAiV4::new(seed).propose_free_actions_for_game(&game, PlayerId::P1);
            if let Some(Action::DeclareAttack { attack }) = actions
                .iter()
                .find(|a| matches!(a, Action::DeclareAttack { .. }))
            {
                first_attacks.insert(attack.name.clone());
            }
        }
        assert!(
            first_attacks.contains("Big Hit") && first_attacks.contains("Small Hit"),
            "{first_attacks:?}"
        );
    }

    #[test]
    fn game_proposals_are_deterministic() {
        let (game, _) = table(&[ENERGY, TOOL, STADIUM, SUPPORTER]);
        let a = format!("{:?}", proposals(&game));
        let b = format!("{:?}", proposals(&game));
        assert_eq!(a, b);
    }

    #[test]
    fn choose_defender_attack_answers_with_choose_defender_attack() {
        let (game, _) = table(&[]);
        let view = game.view_for_player(PlayerId::P1);
        let defender = game.players[1].active.as_ref().unwrap().card.id;
        let prompt = Prompt::ChooseDefenderAttack {
            player: PlayerId::P1,
            defender_id: defender,
            attacks: vec!["Big Hit".into(), "Other".into()],
        };
        let mut ai = RandomAiV4::new(3);
        let actions = ai.propose_prompt_response(&view, &prompt);
        assert_eq!(actions.len(), 2);
        for action in &actions {
            match action {
                Action::ChooseDefenderAttack { attack_name } => {
                    assert!(attack_name == "Big Hit" || attack_name == "Other")
                }
                other => panic!("expected ChooseDefenderAttack, got {other:?}"),
            }
        }
        assert!(ai.unanswered().is_empty());
    }

    #[test]
    fn bench_search_takes_at_most_the_free_bench_slots() {
        let (mut game, _) = table(&[]);
        for _ in 0..3 {
            game.players[0].bench.push(mon(PlayerId::P1));
        }
        let options: Vec<CardInstanceId> = (0..3)
            .map(|_| {
                let card = CardInstance::new(CardDefId::new(MON), PlayerId::P1);
                let id = card.id;
                game.players[0].deck.add(card);
                id
            })
            .collect();
        let view = game.view_for_player(PlayerId::P1);
        let prompt = Prompt::ChooseCardsFromDeck {
            player: PlayerId::P1,
            count: 2,
            options,
            revealed_cards: Vec::new(),
            min: Some(0),
            max: Some(2),
            destination: SelectionDestination::Bench,
            shuffle: true,
        };
        let actions = RandomAiV4::new(5).propose_prompt_response(&view, &prompt);
        for action in &actions {
            match action {
                Action::TakeCardsFromDeck { card_ids } => {
                    assert!(card_ids.len() <= 1, "bench has 1 free slot")
                }
                other => panic!("expected TakeCardsFromDeck, got {other:?}"),
            }
        }
    }
}
