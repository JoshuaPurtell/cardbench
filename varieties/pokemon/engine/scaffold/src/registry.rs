//! Runtime hooks registry.
//!
//! `create_hooks(set)` installs **one** expansion. A Crystal Guardians match must
//! not run Dragon Frontiers bodies.
//!
//! `create_hooks_for(&["CG", "DF"])` composes several expansions for mixed decks.
//! Hooks that describe one card (its power id, its tool rules, its energy units)
//! only consult the set that owns that card's `def_id` prefix. Hooks that describe
//! board effects (damage modifiers, retreat, power locks, attach restrictions)
//! run for every loaded set, because a printed effect applies to all Pokémon in play.

use tcg_core::runtime_hooks::{AttackOverrides, RuntimeHooks};
use tcg_core::{
    Attack, CardDefId, CardInstance, CardInstanceId, EnergyAttachmentSource, GameState, PlayerId,
    PokemonSlot, Type,
};

/// Create the RuntimeHooks vtable for a single expansion (`"CG"`, `"DF"`, or `"HP"`).
pub fn create_hooks(set: &str) -> RuntimeHooks {
    match set {
        "CG" | "cg" | "crystal_guardians" => crate::cg::create(),
        "DF" | "df" | "dragon_frontiers" => crate::df::hooks::create(),
        "HP" | "hp" | "holon_phantoms" => crate::hp::hooks::create(),
        other => panic!("unknown expansion set {other:?}; expected CG, DF, or HP"),
    }
}

const CG: u8 = 1;
const DF: u8 = 2;
const HP: u8 = 4;

type Create = fn() -> RuntimeHooks;

/// (bit, def_id set prefix, per-set vtable) in composition order.
const SETS: [(u8, &str, Create); 3] = [
    (CG, "CG", crate::cg::create as Create),
    (DF, "DF", crate::df::hooks::create as Create),
    (HP, "HP", crate::hp::hooks::create as Create),
];

fn set_bit(set: &str) -> u8 {
    match set {
        "CG" | "cg" | "crystal_guardians" => CG,
        "DF" | "df" | "dragon_frontiers" => DF,
        "HP" | "hp" | "holon_phantoms" => HP,
        other => panic!("unknown expansion set {other:?}; expected CG, DF, or HP"),
    }
}

/// Compose several expansions into one vtable, e.g. for mixed CG+DF decks.
///
/// Composition rules:
/// - `attack_overrides`: merged; `attack_cost_modifier`: summed.
/// - `retreat_cost_override`, `energy_units_override`: chained in set order.
/// - `can_attach_energy`: AND over sets (board restrictions apply to any energy).
/// - `prevents_attack_effects`, `treats_pokemon_as_delta`, `prevents_special_conditions`: OR.
/// - `can_use_pokepower_override`, `is_pokebody_active_override`: any `Some(false)`
///   wins, else any `Some(true)`, else `None`.
/// - `post_attack`, `between_turns`, `apply_tool_stadium_effects`, `after_attack`,
///   `on_energy_attached`: every set runs.
/// - Card-owned hooks dispatch to the set owning the card's `def_id` prefix, and
///   fall back to the default when no loaded set owns it: `execute_power`,
///   `register_triggers`, `can_attach_tool`, `on_tool_attached`,
///   `energy_provides_override`, `energy_units`, `power_effect_id_for`,
///   `power_is_once_per_turn`, `card_has_power_or_body`, `is_double_rainbow`,
///   `tool_discard_timing_override`, and `resolve_custom_prompt` (by effect-id prefix,
///   first true when the prefix is unknown).
pub fn create_hooks_for(sets: &[&str]) -> RuntimeHooks {
    let mask = sets.iter().fold(0u8, |mask, set| mask | set_bit(set));
    match mask {
        1 => composed::<1>(),
        2 => composed::<2>(),
        3 => composed::<3>(),
        4 => composed::<4>(),
        5 => composed::<5>(),
        6 => composed::<6>(),
        7 => composed::<7>(),
        _ => panic!("create_hooks_for needs at least one expansion"),
    }
}

fn loaded<const M: u8>() -> impl Iterator<Item = (&'static str, RuntimeHooks)> {
    SETS.into_iter()
        .filter(|(bit, _, _)| M & bit != 0)
        .map(|(_, code, create)| (code, create()))
}

fn prefix(raw: &str) -> &str {
    raw.split('-').next().unwrap_or("")
}

/// The loaded set that owns `def_id`, if any.
fn owner<const M: u8>(def_id: &CardDefId) -> Option<RuntimeHooks> {
    let code = prefix(def_id.as_str());
    loaded::<M>().find(|(c, _)| *c == code).map(|(_, hooks)| hooks)
}

/// Definition of an instance anywhere a hook can see it: in play, attached, or in hand.
fn def_of(game: &GameState, id: CardInstanceId) -> Option<CardDefId> {
    if let Some(slot) = game.slot_by_id(id) {
        return Some(slot.card.def_id.clone());
    }
    for player in &game.players {
        if let Some(card) = player.hand.get(id) {
            return Some(card.def_id.clone());
        }
        for slot in player.active.iter().chain(player.bench.iter()) {
            if let Some(tool) = slot.attached_tool.as_ref().filter(|tool| tool.id == id) {
                return Some(tool.def_id.clone());
            }
            if let Some(energy) = slot.attached_energy.iter().find(|energy| energy.id == id) {
                return Some(energy.def_id.clone());
            }
        }
    }
    None
}

fn composed<const M: u8>() -> RuntimeHooks {
    RuntimeHooks {
        attack_overrides: attack_overrides::<M>,
        attack_cost_modifier: attack_cost_modifier::<M>,
        retreat_cost_override: retreat_cost_override::<M>,
        post_attack: post_attack::<M>,
        between_turns: between_turns::<M>,
        execute_power: execute_power::<M>,
        register_triggers: register_triggers::<M>,
        apply_tool_stadium_effects: apply_tool_stadium_effects::<M>,
        can_attach_tool: can_attach_tool::<M>,
        can_attach_energy: can_attach_energy::<M>,
        on_tool_attached: on_tool_attached::<M>,
        energy_provides_override: energy_provides_override::<M>,
        on_energy_attached: on_energy_attached::<M>,
        after_attack: after_attack::<M>,
        can_use_pokepower_override: can_use_pokepower_override::<M>,
        is_pokebody_active_override: is_pokebody_active_override::<M>,
        resolve_custom_prompt: resolve_custom_prompt::<M>,
        power_effect_id_for: power_effect_id_for::<M>,
        power_is_once_per_turn: power_is_once_per_turn::<M>,
        card_has_power_or_body: card_has_power_or_body::<M>,
        is_double_rainbow: is_double_rainbow::<M>,
        prevents_attack_effects: prevents_attack_effects::<M>,
        tool_discard_timing_override: tool_discard_timing_override::<M>,
        energy_units: energy_units::<M>,
        energy_units_override: energy_units_override::<M>,
        treats_pokemon_as_delta: treats_pokemon_as_delta::<M>,
        prevents_special_conditions: prevents_special_conditions::<M>,
    }
}

fn attack_overrides<const M: u8>(
    game: &GameState,
    attack: &Attack,
    attacker_id: CardInstanceId,
    defender_id: CardInstanceId,
) -> AttackOverrides {
    let mut overrides = AttackOverrides::default();
    for (_, hooks) in loaded::<M>() {
        overrides.merge((hooks.attack_overrides)(game, attack, attacker_id, defender_id));
    }
    overrides
}

fn attack_cost_modifier<const M: u8>(game: &GameState, attacker_id: CardInstanceId, attack: &Attack) -> i32 {
    loaded::<M>()
        .map(|(_, hooks)| (hooks.attack_cost_modifier)(game, attacker_id, attack))
        .sum()
}

fn retreat_cost_override<const M: u8>(game: &GameState, pokemon_id: CardInstanceId, base: i32) -> i32 {
    loaded::<M>().fold(base, |cost, (_, hooks)| (hooks.retreat_cost_override)(game, pokemon_id, cost))
}

fn post_attack<const M: u8>(game: &mut GameState, attacker_id: CardInstanceId, defender_id: CardInstanceId, damage: u16) {
    for (_, hooks) in loaded::<M>() {
        (hooks.post_attack)(game, attacker_id, defender_id, damage);
    }
}

fn between_turns<const M: u8>(game: &mut GameState) {
    for (_, hooks) in loaded::<M>() {
        (hooks.between_turns)(game);
    }
}

fn execute_power<const M: u8>(game: &mut GameState, power_name: &str, source_id: CardInstanceId) -> bool {
    match def_of(game, source_id) {
        Some(def_id) => owner::<M>(&def_id)
            .map(|hooks| (hooks.execute_power)(game, power_name, source_id))
            .unwrap_or(false),
        None => loaded::<M>().any(|(_, hooks)| (hooks.execute_power)(game, power_name, source_id)),
    }
}

fn register_triggers<const M: u8>(game: &mut GameState, slot: &PokemonSlot) {
    if let Some(hooks) = owner::<M>(&slot.card.def_id) {
        (hooks.register_triggers)(game, slot);
    }
}

fn apply_tool_stadium_effects<const M: u8>(game: &mut GameState) {
    for (_, hooks) in loaded::<M>() {
        (hooks.apply_tool_stadium_effects)(game);
    }
}

fn can_attach_tool<const M: u8>(game: &GameState, player: PlayerId, tool_id: CardInstanceId, target_id: CardInstanceId) -> bool {
    def_of(game, tool_id)
        .and_then(|def_id| owner::<M>(&def_id))
        .map(|hooks| (hooks.can_attach_tool)(game, player, tool_id, target_id))
        .unwrap_or(true)
}

fn can_attach_energy<const M: u8>(game: &GameState, player: PlayerId, energy_id: CardInstanceId, target_id: CardInstanceId) -> bool {
    loaded::<M>().all(|(_, hooks)| (hooks.can_attach_energy)(game, player, energy_id, target_id))
}

fn on_tool_attached<const M: u8>(game: &mut GameState, tool_id: CardInstanceId, target_id: CardInstanceId) {
    if let Some(hooks) = def_of(game, tool_id).and_then(|def_id| owner::<M>(&def_id)) {
        (hooks.on_tool_attached)(game, tool_id, target_id);
    }
}

fn energy_provides_override<const M: u8>(game: &GameState, card: &CardInstance) -> Option<Vec<Type>> {
    owner::<M>(&card.def_id).and_then(|hooks| (hooks.energy_provides_override)(game, card))
}

fn on_energy_attached<const M: u8>(
    game: &mut GameState,
    energy_id: CardInstanceId,
    target_id: CardInstanceId,
    source: EnergyAttachmentSource,
) {
    for (_, hooks) in loaded::<M>() {
        (hooks.on_energy_attached)(game, energy_id, target_id, source);
    }
}

fn after_attack<const M: u8>(game: &mut GameState, attacker_id: CardInstanceId, defender_id: CardInstanceId) {
    for (_, hooks) in loaded::<M>() {
        (hooks.after_attack)(game, attacker_id, defender_id);
    }
}

fn combine_overrides(results: impl Iterator<Item = Option<bool>>) -> Option<bool> {
    let mut allowed = None;
    for result in results {
        match result {
            Some(false) => return Some(false),
            Some(true) => allowed = Some(true),
            None => {}
        }
    }
    allowed
}

fn can_use_pokepower_override<const M: u8>(game: &GameState, player: PlayerId, pokemon_id: CardInstanceId) -> Option<bool> {
    combine_overrides(loaded::<M>().map(|(_, hooks)| (hooks.can_use_pokepower_override)(game, player, pokemon_id)))
}

fn is_pokebody_active_override<const M: u8>(game: &GameState, player: PlayerId, pokemon_id: CardInstanceId) -> Option<bool> {
    combine_overrides(loaded::<M>().map(|(_, hooks)| (hooks.is_pokebody_active_override)(game, player, pokemon_id)))
}

fn resolve_custom_prompt<const M: u8>(
    game: &mut GameState,
    effect_id: &str,
    source_id: Option<CardInstanceId>,
    target_ids: &[CardInstanceId],
) -> bool {
    let code = prefix(effect_id);
    if let Some((_, hooks)) = loaded::<M>().find(|(c, _)| *c == code) {
        return (hooks.resolve_custom_prompt)(game, effect_id, source_id, target_ids);
    }
    loaded::<M>().any(|(_, hooks)| (hooks.resolve_custom_prompt)(game, effect_id, source_id, target_ids))
}

fn power_effect_id_for<const M: u8>(def_id: &CardDefId, power_name: &str) -> Option<String> {
    owner::<M>(def_id).and_then(|hooks| (hooks.power_effect_id_for)(def_id, power_name))
}

fn power_is_once_per_turn<const M: u8>(def_id: &CardDefId, power_name: &str) -> bool {
    owner::<M>(def_id)
        .map(|hooks| (hooks.power_is_once_per_turn)(def_id, power_name))
        .unwrap_or(false)
}

fn card_has_power_or_body<const M: u8>(def_id: &CardDefId) -> bool {
    owner::<M>(def_id)
        .map(|hooks| (hooks.card_has_power_or_body)(def_id))
        .unwrap_or(false)
}

fn is_double_rainbow<const M: u8>(def_id: &CardDefId) -> bool {
    owner::<M>(def_id)
        .map(|hooks| (hooks.is_double_rainbow)(def_id))
        .unwrap_or(false)
}

fn prevents_attack_effects<const M: u8>(game: &GameState, attacker_id: CardInstanceId, defender_id: CardInstanceId) -> bool {
    loaded::<M>().any(|(_, hooks)| (hooks.prevents_attack_effects)(game, attacker_id, defender_id))
}

fn tool_discard_timing_override<const M: u8>(game: &GameState, def_id: &CardDefId) -> Option<&'static str> {
    owner::<M>(def_id).and_then(|hooks| (hooks.tool_discard_timing_override)(game, def_id))
}

fn energy_units<const M: u8>(game: &GameState, holder_id: CardInstanceId, energy: &CardInstance, provides: &[Type]) -> usize {
    owner::<M>(&energy.def_id)
        .map(|hooks| (hooks.energy_units)(game, holder_id, energy, provides))
        .unwrap_or(1)
}

fn energy_units_override<const M: u8>(game: &GameState, holder: &PokemonSlot, energy: &CardInstance, units: usize) -> usize {
    loaded::<M>().fold(units, |units, (_, hooks)| (hooks.energy_units_override)(game, holder, energy, units))
}

fn treats_pokemon_as_delta<const M: u8>(game: &GameState, player: PlayerId) -> bool {
    loaded::<M>().any(|(_, hooks)| (hooks.treats_pokemon_as_delta)(game, player))
}

fn prevents_special_conditions<const M: u8>(game: &GameState, pokemon_id: CardInstanceId) -> bool {
    loaded::<M>().any(|(_, hooks)| (hooks.prevents_special_conditions)(game, pokemon_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tcg_core::runtime_hooks::def_id_matches;
    use tcg_core::{
        CardDefId, CardInstance, CardMeta, CardMetaMap, GameState, PlayerId, PokemonSlot, Stage,
        Type,
    };
    use tcg_rules_ex::RulesetConfig;

    fn holon_veil_game() -> (GameState, tcg_core::CardInstanceId) {
        let mut card_meta = CardMetaMap::new();
        card_meta.insert(
            CardDefId::new("DF-1"),
            CardMeta {
                name: "Ampharos Delta".to_string(),
                is_basic: false,
                is_tool: false,
                is_stadium: false,
                is_pokemon: true,
                is_energy: false,
                hp: 0,
                energy_kind: None,
                provides: Vec::new(),
                trainer_kind: None,
                is_ex: false,
                is_star: false,
                is_delta: true,
                stage: Stage::Stage2,
                types: vec![Type::Lightning],
                weakness: None,
                resistance: None,
                retreat_cost: None,
                trainer_effect: None,
                evolves_from: Some("Flaaffy".to_string()),
                attacks: Vec::new(),
                card_type: String::new(),
                delta_species: false,
            },
        );
        card_meta.insert(
            CardDefId::new("CG-050"),
            CardMeta {
                name: "Non-Delta".to_string(),
                is_basic: true,
                is_tool: false,
                is_stadium: false,
                is_pokemon: true,
                is_energy: false,
                hp: 0,
                energy_kind: None,
                provides: Vec::new(),
                trainer_kind: None,
                is_ex: false,
                is_star: false,
                is_delta: false,
                stage: Stage::Basic,
                types: vec![Type::Colorless],
                weakness: None,
                resistance: None,
                retreat_cost: None,
                trainer_effect: None,
                evolves_from: None,
                attacks: Vec::new(),
                card_type: String::new(),
                delta_species: false,
            },
        );
        let mut game = GameState::new_with_card_meta(
            Vec::new(),
            Vec::new(),
            5,
            RulesetConfig::default(),
            card_meta,
        );
        let ampharos = PokemonSlot::new(CardInstance::new(CardDefId::new("DF-1"), PlayerId::P1));
        let other = PokemonSlot::new(CardInstance::new(CardDefId::new("CG-050"), PlayerId::P1));
        let other_id = other.card.id;
        game.players[0].active = Some(ampharos);
        game.players[0].bench.push(other);
        (game, other_id)
    }

    #[test]
    fn df_hooks_apply_holon_veil() {
        let (mut game, other_id) = holon_veil_game();
        game.set_hooks(create_hooks("DF"));
        assert!(game.pokemon_is_delta(other_id));
        assert!(def_id_matches(&CardDefId::new("DF-1"), "DF", 1));
    }

    #[test]
    fn cg_hooks_do_not_apply_holon_veil() {
        let (mut game, other_id) = holon_veil_game();
        game.set_hooks(create_hooks("CG"));
        assert!(!game.pokemon_is_delta(other_id));
    }

    #[test]
    fn mixed_cg_df_runs_df_body_on_the_shared_board() {
        let (mut game, other_id) = holon_veil_game();
        game.set_hooks(create_hooks_for(&["CG", "DF"]));
        // Holon Veil is printed for all of its owner's Pokémon, so a CG Pokémon counts.
        assert!(game.pokemon_is_delta(other_id));
        let (mut cg_only, cg_other) = holon_veil_game();
        cg_only.set_hooks(create_hooks_for(&["CG"]));
        assert!(!cg_only.pokemon_is_delta(cg_other));
    }

    #[test]
    fn mixed_cg_df_card_rules_stay_in_their_set() {
        let (game, holder) = holon_veil_game();
        let mixed = create_hooks_for(&["CG", "DF"]);
        let df = create_hooks("DF");
        // CG set rules fire for CG cards only.
        assert!((mixed.is_double_rainbow)(&CardDefId::new("CG-88")));
        assert!(!(mixed.is_double_rainbow)(&CardDefId::new("DF-88")));
        assert_eq!((mixed.tool_discard_timing_override)(&game, &CardDefId::new("CG-80")), Some("EndOfTurnIfAttacked"));
        assert_eq!((mixed.tool_discard_timing_override)(&game, &CardDefId::new("DF-80")), None);
        // DF Boost Energy (DF-87) pays 3; the CG card with the same number does not.
        let boost = CardInstance::new(CardDefId::new("DF-87"), PlayerId::P1);
        let potion = CardInstance::new(CardDefId::new("CG-87"), PlayerId::P1);
        assert_eq!((mixed.energy_units)(&game, holder, &boost, &[Type::Colorless]), 3);
        assert_eq!((mixed.energy_units)(&game, holder, &potion, &[Type::Colorless]), 1);
        // DF power metadata answers for DF def ids only, and matches DF alone.
        for spec in crate::df::DF_POWERS {
            let df_id = CardDefId::new(format!("DF-{}", spec.number));
            let cg_id = CardDefId::new(format!("CG-{}", spec.number));
            assert_eq!((mixed.card_has_power_or_body)(&df_id), (df.card_has_power_or_body)(&df_id));
            assert_eq!(
                (mixed.power_effect_id_for)(&df_id, spec.power_name),
                (df.power_effect_id_for)(&df_id, spec.power_name)
            );
            assert_eq!((mixed.power_effect_id_for)(&cg_id, spec.power_name), None);
            assert_eq!(
                (mixed.card_has_power_or_body)(&cg_id),
                (create_hooks("CG").card_has_power_or_body)(&cg_id)
            );
        }
    }

    #[test]
    fn single_set_composition_matches_create_hooks() {
        let composed = create_hooks_for(&["CG"]);
        let direct = create_hooks("CG");
        let (game, holder) = holon_veil_game();
        let ids = (1..=100)
            .map(|n| format!("CG-{n}"))
            .chain(["ENERGY-WATER".to_string(), "ENERGY-COLORLESS".to_string()]);
        for raw in ids {
            let def_id = CardDefId::new(raw.as_str());
            assert_eq!((composed.is_double_rainbow)(&def_id), (direct.is_double_rainbow)(&def_id), "{raw}");
            assert_eq!((composed.card_has_power_or_body)(&def_id), (direct.card_has_power_or_body)(&def_id), "{raw}");
            assert_eq!(
                (composed.tool_discard_timing_override)(&game, &def_id),
                (direct.tool_discard_timing_override)(&game, &def_id),
                "{raw}"
            );
            assert_eq!(
                (composed.power_effect_id_for)(&def_id, "Baby Evolution"),
                (direct.power_effect_id_for)(&def_id, "Baby Evolution"),
                "{raw}"
            );
            let card = CardInstance::new(def_id.clone(), PlayerId::P1);
            let provides = [Type::Water, Type::Fire];
            assert_eq!(
                (composed.energy_units)(&game, holder, &card, &provides),
                (direct.energy_units)(&game, holder, &card, &provides),
                "{raw}"
            );
        }
        let (mut a, a_other) = holon_veil_game();
        a.set_hooks(composed);
        let (mut b, b_other) = holon_veil_game();
        b.set_hooks(direct);
        assert_eq!(a.pokemon_is_delta(a_other), b.pokemon_is_delta(b_other));
        assert_eq!(a.retreat_cost(a_other), b.retreat_cost(b_other));
    }
}
