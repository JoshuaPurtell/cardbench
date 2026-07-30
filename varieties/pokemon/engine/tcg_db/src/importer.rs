use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};

use rusqlite::Connection;
use serde_json::{json, Value};
use thiserror::Error;

use crate::{compile_card, CardCompilerError};
use tcg_expansions as expansions;

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("compile error: {0}")]
    Compile(#[from] CardCompilerError),
}

#[derive(Debug, Default)]
pub struct ImportReport {
    pub imported: usize,
    pub skipped: usize,
}

pub fn import_jsonl(
    conn: &Connection,
    path: &str,
    set_code: &str,
    max_number: u32,
) -> Result<ImportReport, ImportError> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut report = ImportReport::default();

    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let raw: Value = serde_json::from_str(&line)?;
        if let Some(number) = raw.get("number").and_then(Value::as_str) {
            if let Ok(num) = number.parse::<u32>() {
                if num <= max_number {
                    let normalized = normalize_card(&raw, set_code);
                    compile_card(conn, &normalized)?;
                    report.imported += 1;
                    continue;
                }
            }
        }
        report.skipped += 1;
    }

    Ok(report)
}

fn attack_effect_ast(set_code: &str, card_number: &str, attack_name: &str, attack_text: &str) -> Option<Value> {
    // Start very small: implement core Special Condition attacks that appear early (like Bulbasaur CG-45).
    // Note: raw text includes "Pokémon" with an accent.
    let text = attack_text.trim();
    if text.is_empty() {
        return None;
    }
    if set_code == "RS" {
        if let Some(effect) = rs_attack_effect_ast(set_code, card_number, attack_name, text) {
            return Some(effect);
        }
    }

    if let Some(effect) = expansions::attack_effect_ast(set_code, card_number, attack_name) {
        return Some(effect);
    }

    // Loudred (CG-23) "Surprise": choose random card from opponent hand, reveal, shuffle into deck.
    if attack_name == "Surprise"
        && text.starts_with("Choose 1 card from your opponent's hand without looking.")
    {
        return Some(json!({
            "op": "ReturnRandomCardFromHandToDeck",
            "player": "Opponent",
            "reveal": true,
            "shuffle": true,
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "Choose 1 card from your opponent's hand without looking and discard it." {
        return Some(json!({
            "op": "DiscardRandomFromHand",
            "player": "Opponent",
            "count": 1,
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "Your opponent switches the Defending Pokémon with 1 of his or her Benched Pokémon, if any." {
        return Some(json!({
            "op": "SwitchActive",
            "player": "Opponent",
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "If Manectric has a Pokémon Tool card attached to it, this attack does 20 damage to each of your opponent's Benched Pokémon-ex. (Don't apply Weakness and Resistance for Benched Pokémon.)" {
        return Some(json!({
            "op": "IfAttackerHasTool",
            "effect": {
                "op": "DealDamageToSelector",
                "player": "Current",
                "selector": {
                    "scope": "OppBench",
                    "is_ex": true
                },
                "amount": 20
            },
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Loudred (CG-23) "Bass Control": choose opponent Pokemon, deal 40 to it.
    if attack_name == "Bass Control"
        && text.starts_with("Choose 1 of your opponent's Pokémon.")
    {
        return Some(json!({
            "op": "ChoosePokemonTargets",
            "player": "Current",
            "selector": {
                "scope": "OppAll"
            },
            "min": 1,
            "max": 1,
            "effect": {
                "op": "DealDamage",
                "target": "Selected",
                "amount": 40
            },
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "Choose 1 of your opponent's Pokémon. This attack does 30 damage to that Pokémon. (Don't apply Weakness and Resistance for Benched Pokémon.)" {
        return Some(json!({
            "op": "ChoosePokemonTargets",
            "player": "Current",
            "selector": {
                "scope": "OppAll"
            },
            "min": 1,
            "max": 1,
            "effect": {
                "op": "DealDamage",
                "target": "Selected",
                "amount": 30
            },
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "Choose 1 of your opponent's Pokémon. This attack does 30 damage to that Pokémon. This attack's damage isn't affected by Weakness or Resistance." {
        return Some(json!({
            "op": "ChoosePokemonTargets",
            "player": "Current",
            "selector": {
                "scope": "OppAll"
            },
            "min": 1,
            "max": 1,
            "effect": {
                "op": "DealDamage",
                "target": "Selected",
                "amount": 30
            },
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text.starts_with("Choose 1 of your opponent's Pokémon. This attack does 10 damage for each damage counter on") {
        return Some(json!({
            "op": "ChoosePokemonTargets",
            "player": "Current",
            "selector": {
                "scope": "OppAll"
            },
            "min": 1,
            "max": 1,
            "effect": {
                "op": "DealDamageByAttackerDamage",
                "target": "Selected",
                "base": 0,
                "per_counter": 10
            },
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "If the Defending Pokémon is Pokémon-ex, this attack does 20 damage plus 30 more damage." {
        return Some(json!({
            "op": "IfTargetIsEx",
            "target": "OppActive",
            "effect": { "op": "ModifyDamage", "amount": 30 },
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "Does 50 damage plus 20 more damage for each Special Condition affecting the Defending Pokémon." {
        return Some(json!({
            "op": "TextOnly",
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "Search your deck for up to 2 Basic Pokémon and put them onto your Bench. Shuffle your deck afterward." {
        return Some(json!({
            "op": "SearchDeckWithSelector",
            "player": "Current",
            "selector": {
                "is_pokemon": true,
                "is_basic": true
            },
            "count": 2,
            "min": 0,
            "max": 2,
            "destination": "Bench",
            "shuffle": true,
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "If the Defending Pokémon is affected by a Special Condition, this attack does 30 damage plus 20 more damage." {
        return Some(json!({
            "op": "TextOnly",
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "Does 50 damage plus 20 more damage for each Water Energy attached to Blastoise but not used to pay for this attack's Energy cost. You can't add more than 40 damage in this way." {
        return Some(json!({
            "op": "TextOnly",
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "Choose 3 of your opponent's Pokémon. This attack does 10 damage to each of those Pokémon. (Don't apply Weakness and Resistance for Benched Pokémon.)" {
        return Some(json!({
            "op": "ChoosePokemonTargets",
            "player": "Current",
            "selector": {
                "scope": "OppAll"
            },
            "min": 3,
            "max": 3,
            "effect": {
                "op": "DealDamage",
                "target": "Selected",
                "amount": 10
            },
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "During your next turn, Combusken's High Jump Kick attack's base damage is 70." {
        return Some(json!({
            "op": "AddMarker",
            "target": "SelfActive",
            "name": "High Jump Kick Boost",
            "expires_after_turns": 1,
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "Prevent all effects of attacks, including damage, done to Dusclops by your opponent's Pokémon-ex during your opponent's next turn." {
        return Some(json!({
            "op": "AddMarker",
            "target": "SelfActive",
            "name": "PreventAllEffectsEx",
            "expires_after_turns": 1,
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "Does 10 damage times the number of Pokémon in play (both yours and your opponent's), excluding Grumpig." {
        return Some(json!({
            "op": "TextOnly",
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Venusaur (CG-28) "Green Blast": +10 per Grass Energy attached to your Pokemon.
    if attack_name == "Green Blast"
        && text.starts_with("Does 20 damage plus 10 more damage for each Grass Energy")
    {
        return Some(json!({
            "op": "DealDamageByEnergyInPlay",
            "player": "Current",
            "energy_type": "Grass",
            "target": "OppActive",
            "per_energy": 10,
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Venusaur (CG-28) "Toxic Sleep": Asleep + Poisoned, extra poison between turns.
    if attack_name == "Toxic Sleep"
        && text.starts_with("The Defending Pokémon is now Asleep and Poisoned.")
    {
        return Some(json!({
            "op": "Sequence",
            "effects": [
                { "op": "ApplySpecialCondition", "target": "OppActive", "condition": "Asleep" },
                { "op": "ApplySpecialCondition", "target": "OppActive", "condition": "Poisoned" },
                { "op": "AddMarker", "target": "OppActive", "name": "Toxic Sleep" }
            ],
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Grovyle (CG-32) "Detect": flip a coin, prevent all effects of attacks next turn.
    if attack_name == "Detect"
        && text.starts_with("Flip a coin. If heads, prevent all effects of an attack")
    {
        return Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": {
                "op": "AddMarker",
                "target": "SelfActive",
                "name": "Detect",
                "expires_after_turns": 1
            },
            "on_tails": { "op": "NoOp" },
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text.starts_with("Flip a coin. If heads, prevent all effects of an attack, including damage, done to") {
        return Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": {
                "op": "AddMarker",
                "target": "SelfActive",
                "name": "PreventAllDamageAndEffects",
                "expires_after_turns": 1
            },
            "on_tails": { "op": "NoOp" },
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Gulpin (CG-33) "Amnesia": choose defender attack to block.
    if attack_name == "Amnesia"
        && text.starts_with("Choose 1 of the Defending Pokémon's attacks.")
    {
        return Some(json!({
            "op": "ChooseDefenderAttack",
            "target": "OppActive",
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "Choose 1 of the Defending Pokémon's attacks. That Pokémon can't use that attack during your opponent's next turn." {
        return Some(json!({
            "op": "ChooseDefenderAttack",
            "target": "OppActive",
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Ivysaur (CG-34) "Sleep Powder": Defending Pokemon is Asleep.
    if attack_name == "Sleep Powder" || text == "The Defending Pokémon is now Asleep." {
        return Some(json!({
            "op": "ApplySpecialCondition",
            "target": "OppActive",
            "condition": "Asleep",
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Supersonic: flip a coin, Confused.
    if attack_name == "Supersonic"
        && text == "Flip a coin. If heads, the Defending Pokémon is now Confused."
    {
        return Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "ApplySpecialCondition", "target": "OppActive", "condition": "Confused" },
            "on_tails": { "op": "NoOp" },
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "If the Defending Pokémon is a Basic Pokémon, that Pokémon is now Confused." {
        return Some(json!({
            "op": "IfTargetIsBasic",
            "target": "OppActive",
            "effect": { "op": "ApplySpecialCondition", "target": "OppActive", "condition": "Confused" },
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if attack_name == "Supersonic"
        && text == "The Defending Pokémon is now Confused."
    {
        return Some(json!({
            "op": "ApplySpecialCondition",
            "target": "OppActive",
            "condition": "Confused",
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Treecko (CG-67) "Paralyzing Gaze": "Flip a coin. If heads, the Defending Pokémon is now Paralyzed."
    if text == "Flip a coin. If heads, the Defending Pokémon is now Paralyzed." {
        return Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "ApplySpecialCondition", "target": "OppActive", "condition": "Paralyzed" },
            "on_tails": { "op": "NoOp" },
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Bulbasaur (CG-45) "Poisonpowder": "The Defending Pokémon is now Poisoned."
    // Also matches other sets/cards that reuse the same templated text.
    if attack_name == "Poisonpowder" || text.contains("is now Poisoned") {
        if text == "The Defending Pokémon is now Poisoned." || text.contains("The Defending Pokémon is now Poisoned.") {
            return Some(json!({
                "op": "ApplySpecialCondition",
                "target": "OppActive",
                "condition": "Poisoned",
                "notes": format!("{set_code}-{card_number}:{attack_name}")
            }));
        }
    }

    // Jigglypuff (CG-53) "Hypnoblast": "The Defending Pokémon is now Asleep."
    if attack_name == "Hypnoblast" || text.contains("is now Asleep") {
        if text == "The Defending Pokémon is now Asleep." || text.contains("The Defending Pokémon is now Asleep.") {
            return Some(json!({
                "op": "ApplySpecialCondition",
                "target": "OppActive",
                "condition": "Asleep",
                "notes": format!("{set_code}-{card_number}:{attack_name}")
            }));
        }
    }

    // Feraligatr δ (DF-2) "Drag Off": choose an opponent Benched Pokemon and switch.
    if attack_name == "Drag Off"
        && text.starts_with("Before doing damage, you may choose 1 of your opponent's Benched Pokémon")
    {
        return Some(json!({
            "op": "ChoosePokemonTargets",
            "player": "Current",
            "selector": {
                "scope": "OppBench"
            },
            "min": 1,
            "max": 1,
            "effect": {
                "op": "SwitchToTarget",
                "player": "Opponent"
            },
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Typhlosion δ (DF-12) "Burning Ball": burn if at least 2 Fire Energy attached.
    if attack_name == "Burning Ball"
        && text.starts_with("If Typhlosion has at least 2 Fire Energy attached")
    {
        return Some(json!({
            "op": "ApplySpecialConditionIfEnergyAttached",
            "target": "OppActive",
            "energy_type": "Fire",
            "count": 2,
            "condition": "Burned",
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Arbok δ (DF-13) "Burning Venom": burned + poisoned.
    if attack_name == "Burning Venom"
        && text == "The Defending Pokémon is now Burned and Poisoned."
    {
        return Some(json!({
            "op": "Sequence",
            "effects": [
                { "op": "ApplySpecialCondition", "target": "OppActive", "condition": "Burned" },
                { "op": "ApplySpecialCondition", "target": "OppActive", "condition": "Poisoned" }
            ],
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Arbok δ (DF-13) "Strangle": +30 if defending is Delta.
    if attack_name == "Strangle"
        && text.starts_with("If the Defending Pokémon has δ on its card")
    {
        return Some(json!({
            "op": "DealDamageIfTargetDelta",
            "target": "OppActive",
            "amount": 30,
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Cloyster δ (DF-14) "Grind": +10 per Energy attached to Cloyster.
    if attack_name == "Grind"
        && text.starts_with("Does 10 damage plus 10 more damage for each Energy attached")
    {
        return Some(json!({
            "op": "DealDamageByAttachedEnergy",
            "target": "OppActive",
            "per_energy": 10,
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Lickitung δ (DF-19) "Lap Up": draw 2.
    if attack_name == "Lap Up" && text == "Draw 2 cards." {
        return Some(json!({
            "op": "DrawCards",
            "player": "Current",
            "count": 2,
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Lickitung δ (DF-19) "Delta Mind": place counters based on Delta target.
    if attack_name == "Delta Mind"
        && text.starts_with("Put 1 damage counter on 1 of your opponent's Pokémon.")
    {
        return Some(json!({
            "op": "ChoosePokemonTargets",
            "player": "Current",
            "selector": {
                "scope": "OppAll"
            },
            "min": 1,
            "max": 1,
            "effect": {
                "op": "PlaceDamageCountersIfTargetDelta",
                "target": "Selected",
                "base": 1,
                "delta": 3
            },
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Mantine δ (DF-20) "Spiral Drain": heal 1 counter.
    if attack_name == "Spiral Drain"
        && text == "Remove 1 damage counter from Mantine."
    {
        return Some(json!({
            "op": "HealDamage",
            "target": "SelfActive",
            "amount": 10,
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Seadra δ (DF-22) "Smokescreen": mark defender.
    if attack_name == "Smokescreen"
        && text.starts_with("If the Defending Pokémon tries to attack during your opponent's next turn")
    {
        return Some(json!({
            "op": "AddMarker",
            "target": "OppActive",
            "name": "Smokescreen",
            "expires_after_turns": 1,
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Vibrava δ (DF-24) "Quick Blow": flip for +20.
    if attack_name == "Quick Blow"
        && text == "Flip a coin. If heads, this attack does 20 damage plus 20 more damage."
    {
        return Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "DealDamage", "target": "OppActive", "amount": 20 },
            "on_tails": { "op": "NoOp" },
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "Flip a coin. If heads, this attack does 10 damage plus 10 more damage." {
        return Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "ModifyDamage", "amount": 10 },
            "on_tails": { "op": "NoOp" },
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "Does 60 damage plus 10 more damage for each Prize card your opponent has taken." {
        return Some(json!({
            "op": "DealDamageByOpponentPrizesTaken",
            "target": "OppActive",
            "base": 60,
            "per_prize": 10,
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "Discard all Metal Energy attached to Charizard." {
        return Some(json!({
            "op": "DiscardAttachedEnergyByType",
            "target": "SelfActive",
            "energy_type": "Metal",
            "count": 99,
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text.ends_with("does 10 damage to itself.") {
        return Some(json!({
            "op": "DealDamage",
            "target": "SelfActive",
            "amount": 10,
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Croconaw δ (DF-27) "Scary Face": flip for attack/retreat lock.
    if attack_name == "Scary Face"
        && text.starts_with("Flip a coin. If heads, the Defending Pokémon can't attack or retreat")
    {
        return Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": {
                "op": "Sequence",
                "effects": [
                    { "op": "AddMarker", "target": "OppActive", "name": "CannotAttack", "expires_after_turns": 1 },
                    { "op": "AddMarker", "target": "OppActive", "name": "CannotRetreat", "expires_after_turns": 1 }
                ]
            },
            "on_tails": { "op": "NoOp" },
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Kirlia δ (DF-33) "Flickering Flames": asleep.
    if attack_name == "Flickering Flames"
        && text == "The Defending Pokémon is now Asleep."
    {
        return Some(json!({
            "op": "ApplySpecialCondition",
            "target": "OppActive",
            "condition": "Asleep",
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Quilava δ (DF-36) "Quick Attack": flip for +20.
    if attack_name == "Quick Attack"
        && text == "Flip a coin. If heads, this attack does 30 damage plus 20 more damage."
    {
        return Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "DealDamage", "target": "OppActive", "amount": 20 },
            "on_tails": { "op": "NoOp" },
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Cyndaquil δ (DF-45) "Swift": ignore modifiers.
    if attack_name == "Swift"
        && text.starts_with("This attack's damage isn't affected by Weakness")
    {
        return Some(json!({
            "op": "DealDamageNoModifiers",
            "target": "OppActive",
            "amount": 30,
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Pupitar δ (DF-59) "Hyper Beam": flip to discard energy.
    if attack_name == "Hyper Beam"
        && text == "Flip a coin. If heads, discard an Energy card attached to the Defending Pokémon."
    {
        return Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "DiscardAttachedEnergy", "target": "OppActive", "count": 1 },
            "on_tails": { "op": "NoOp" },
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Ralts δ (DF-61) "Calm Mind": heal 2 counters.
    if attack_name == "Calm Mind"
        && text == "Remove 2 damage counters from Ralts."
    {
        return Some(json!({
            "op": "HealDamage",
            "target": "SelfActive",
            "amount": 20,
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    // Totodile δ (DF-67) "Rage": +10 per damage counter.
    if attack_name == "Rage"
        && text == "Does 10 damage plus 10 more damage for each damage counter on Totodile."
    {
        return Some(json!({
            "op": "DealDamageByAttackerDamage",
            "target": "OppActive",
            "base": 10,
            "per_counter": 10,
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "Does 60 damage plus 10 more damage for each Prize card your opponent has taken." {
        return Some(json!({
            "op": "DealDamageByOpponentPrizesTaken",
            "target": "OppActive",
            "base": 60,
            "per_prize": 10,
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "Draw a card." {
        return Some(json!({
            "op": "DrawCards",
            "player": "Current",
            "count": 1,
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "Draw 2 cards." {
        return Some(json!({
            "op": "DrawCards",
            "player": "Current",
            "count": 2,
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "Draw 3 cards." {
        return Some(json!({
            "op": "DrawCards",
            "player": "Current",
            "count": 3,
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "The Defending Pokémon is now Confused." {
        return Some(json!({
            "op": "ApplySpecialCondition",
            "target": "OppActive",
            "condition": "Confused",
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "The Defending Pokémon is now Burned." {
        return Some(json!({
            "op": "ApplySpecialCondition",
            "target": "OppActive",
            "condition": "Burned",
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "Flip a coin. If heads, the Defending Pokémon is now Confused." {
        return Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "ApplySpecialCondition", "target": "OppActive", "condition": "Confused" },
            "on_tails": { "op": "NoOp" },
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "Flip a coin. If heads, the Defending Pokémon is now Poisoned." {
        return Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "ApplySpecialCondition", "target": "OppActive", "condition": "Poisoned" },
            "on_tails": { "op": "NoOp" },
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "Flip a coin. If heads, the Defending Pokémon can't attack during your opponent's next turn." {
        return Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": {
                "op": "AddMarker",
                "target": "OppActive",
                "name": "CannotAttack",
                "expires_after_turns": 1
            },
            "on_tails": { "op": "NoOp" },
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    if text == "The Defending Pokémon can't retreat during your opponent's next turn." {
        return Some(json!({
            "op": "AddMarker",
            "target": "OppActive",
            "name": "CannotRetreat",
            "expires_after_turns": 1,
            "notes": format!("{set_code}-{card_number}:{attack_name}")
        }));
    }

    None
}

fn rs_custom_attack_ast(
    set_code: &str,
    card_number: &str,
    attack_name: &str,
    timing: Option<&str>,
) -> Value {
    let mut value = json!({
        "op": "Custom",
        "id": format!("{set_code}-{card_number}:{attack_name}"),
        "name": attack_name,
        "notes": format!("{set_code}-{card_number}:{attack_name}")
    });
    if let Some(timing) = timing {
        if let Value::Object(ref mut obj) = value {
            obj.insert("timing".to_string(), json!(timing));
        }
    }
    value
}

fn rs_attack_effect_ast(
    set_code: &str,
    card_number: &str,
    attack_name: &str,
    text: &str,
) -> Option<Value> {
    let notes = format!("{set_code}-{card_number}:{attack_name}");
    match text {
        "After your attack, remove from Pelipper the number of damage counters equal to the damage you did to the Defending Pokémon. If Pelipper has fewer damage counters than that, remove all of them." => Some(json!({
            "op": "AddMarker",
            "target": "SelfActive",
            "name": "Swallow Heal",
            "notes": notes
        })),
        "Attach 1 Energy card from your discard pile to your Active Pokémon." => Some(json!({
            "op": "AttachEnergyFromDiscard",
            "player": "Current",
            "energy_selector": { "is_energy": true },
            "target_selector": { "scope": "SelfActive" },
            "count": 1,
            "min": 1,
            "max": 1,
            "end_turn": false,
            "notes": notes
        })),
        "Attach a Lightning Energy card from your discard pile to Electrike." => Some(json!({
            "op": "AttachEnergyFromDiscard",
            "player": "Current",
            "energy_selector": { "is_energy": true, "energy_types": ["Lightning"] },
            "target_selector": { "scope": "SelfActive" },
            "count": 1,
            "min": 1,
            "max": 1,
            "end_turn": false,
            "notes": notes
        })),
        "Attach a basic Energy card from your hand to 1 of your Pokémon." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, None),
        ),
        "Attach up to 2 Energy cards from your discard pile to Mewtwo ex." => Some(json!({
            "op": "AttachEnergyFromDiscard",
            "player": "Current",
            "energy_selector": { "is_energy": true },
            "target_selector": { "scope": "SelfActive" },
            "count": 2,
            "min": 0,
            "max": 2,
            "end_turn": false,
            "notes": notes
        })),
        "Chansey ex does 60 damage to itself." => Some(json!({
            "op": "DealDamage",
            "target": "SelfActive",
            "amount": 60,
            "notes": notes
        })),
        "Choose 1 of your opponent's Benched Pokémon. This attack does 10 damage to that Pokémon. (Don't apply Weakness and Resistance for Benched Pokémon.)" => Some(json!({
            "op": "ChoosePokemonTargets",
            "player": "Current",
            "selector": { "scope": "OppBench" },
            "min": 1,
            "max": 1,
            "effect": { "op": "DealDamage", "target": "Selected", "amount": 10 },
            "notes": notes
        })),
        "Choose 1 of your opponent's Pokémon. This attack does 20 damage to that Pokémon. (Don't apply Weakness and Resistance for Benched Pokémon.)" => Some(json!({
            "op": "ChoosePokemonTargets",
            "player": "Current",
            "selector": { "scope": "OppAll" },
            "min": 1,
            "max": 1,
            "effect": { "op": "DealDamage", "target": "Selected", "amount": 20 },
            "notes": notes
        })),
        "Discard 2 basic Energy cards attached to Camerupt or this attack does nothing." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, Some("PreDamage")),
        ),
        "Discard a Fire Energy card attached to Blaziken." => Some(json!({
            "op": "DiscardAttachedEnergyByType",
            "target": "SelfActive",
            "energy_type": "Fire",
            "count": 1,
            "notes": notes
        })),
        "Discard a Fire Energy card attached to Blaziken. If you do, this attack does 10 damage to each of your opponent's Benched Pokémon. (Don't apply Weakness and Resistance for Benched Pokémon.)" => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, None),
        ),
        "Discard a basic Energy card attached to Slaking or this attack does nothing. Slaking can't attack during your next turn." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, Some("PreDamage")),
        ),
        "Does 10 damage for each damage counter on Goldeen." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, None),
        ),
        "Does 10 damage plus 10 more damage for each Energy attached to Lapras ex but not used to pay for this attack's Energy cost. You can't add more than 20 damage in this way." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, None),
        ),
        "Does 10 damage times the amount of Energy attached to all of your Active Pokémon." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, None),
        ),
        "Does 10 damage times the total amount of Energy attached to Gardevoir and the Defending Pokémon." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, None),
        ),
        "Does 10 damage to 2 of your opponent's Benched Pokémon (1 if there is only 1). (Don't apply Weakness and Resistance for Benched Pokémon.)" => Some(json!({
            "op": "ChoosePokemonTargets",
            "player": "Current",
            "selector": { "scope": "OppBench" },
            "min": 1,
            "max": 2,
            "effect": { "op": "DealDamage", "target": "Selected", "amount": 10 },
            "notes": notes
        })),
        "Does 10 damage to each Benched Pokémon (both yours and your opponent's). (Don't apply Weakness and Resistance for Benched Pokémon.)" => Some(json!({
            "op": "Sequence",
            "effects": [
                { "op": "DealDamageToSelector", "player": "Current", "selector": { "scope": "SelfBench" }, "amount": 10 },
                { "op": "DealDamageToSelector", "player": "Current", "selector": { "scope": "OppBench" }, "amount": 10 }
            ],
            "notes": notes
        })),
        "Does 20 damage plus 10 more damage for each damage counter on Vigoroth." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, None),
        ),
        "Does 20 damage to each Defending Pokémon." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, None),
        ),
        "Does 30 damage plus 10 more damage for each Energy attached to Delcatty but not used to pay for this attack's Energy cost." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, None),
        ),
        "Does 40 damage plus 10 more damage for each Fighting Energy attached to Breloom." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, None),
        ),
        "During your next turn, Spit Up's base damage is 70 instead of 30, and Swallow's base damage is 60 instead of 20." => Some(json!({
            "op": "AddMarker",
            "target": "SelfActive",
            "name": "Stockpile",
            "expires_after_turns": 1,
            "notes": notes
        })),
        "During your opponent's next turn, any damage done to Aron by attacks is reduced by 10." => Some(json!({
            "op": "AddMarker",
            "target": "SelfActive",
            "name": "Teary Eyes",
            "expires_after_turns": 1,
            "notes": notes
        })),
        "Each Defending Pokémon is now Poisoned. Does 10 damage to each of your opponent's Benched Pokémon. (Don't apply Weakness and Resistance for Benched Pokémon.)" => Some(json!({
            "op": "Sequence",
            "effects": [
                { "op": "ApplySpecialCondition", "target": "OppActive", "condition": "Poisoned" },
                { "op": "DealDamageToSelector", "player": "Current", "selector": { "scope": "OppBench" }, "amount": 10 }
            ],
            "notes": notes
        })),
        "Flip 2 coins. This attack does 10 damage times the number of heads." => Some(json!({
            "op": "FlipCoins",
            "count": 2,
            "on_heads": { "op": "ModifyDamage", "amount": 10 },
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip 2 coins. This attack does 20 damage times the number of heads." => Some(json!({
            "op": "FlipCoins",
            "count": 2,
            "on_heads": { "op": "ModifyDamage", "amount": 20 },
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip 2 coins. This attack does 30 damage plus 20 more damage for each heads." => Some(json!({
            "op": "FlipCoins",
            "count": 2,
            "on_heads": { "op": "ModifyDamage", "amount": 20 },
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip 2 coins. This attack does 40 damage plus 10 more damage for each heads." => Some(json!({
            "op": "FlipCoins",
            "count": 2,
            "on_heads": { "op": "ModifyDamage", "amount": 10 },
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip 2 coins. This attack does 40 damage times the number of heads." => Some(json!({
            "op": "FlipCoins",
            "count": 2,
            "on_heads": { "op": "ModifyDamage", "amount": 40 },
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip 2 coins. This attack does 50 damage times the number of heads." => Some(json!({
            "op": "FlipCoins",
            "count": 2,
            "on_heads": { "op": "ModifyDamage", "amount": 50 },
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip 2 coins. This attack does 60 damage times the number of heads." => Some(json!({
            "op": "FlipCoins",
            "count": 2,
            "on_heads": { "op": "ModifyDamage", "amount": 60 },
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip 2 coins. This attack does 70 damage times the number of heads." => Some(json!({
            "op": "FlipCoins",
            "count": 2,
            "on_heads": { "op": "ModifyDamage", "amount": 70 },
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip 3 coins. This attack does 10 damage times the number of heads." => Some(json!({
            "op": "FlipCoins",
            "count": 3,
            "on_heads": { "op": "ModifyDamage", "amount": 10 },
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip 3 coins. This attack does 20 damage times the number of heads." => Some(json!({
            "op": "FlipCoins",
            "count": 3,
            "on_heads": { "op": "ModifyDamage", "amount": 20 },
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip a coin for each of your Pokémon in play (including Sneasel ex). This attack does 20 damage times the number of heads." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, Some("PreDamage")),
        ),
        "Flip a coin until you get tails. This attack does 40 damage times the number of heads." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, Some("PreDamage")),
        ),
        "Flip a coin. If heads, choose 1 card from your opponent's hand without looking and discard it." => Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "DiscardRandomFromHand", "player": "Opponent", "count": 1 },
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip a coin. If heads, discard 1 Energy card attached to the Defending Pokémon." => Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "DiscardAttachedEnergy", "target": "OppActive", "count": 1, "only_basic": false, "only_special": false },
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip a coin. If heads, each Defending Pokémon is now Burned." => Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "ApplySpecialCondition", "target": "OppActive", "condition": "Burned" },
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip a coin. If heads, each Defending Pokémon is now Confused." => Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "ApplySpecialCondition", "target": "OppActive", "condition": "Confused" },
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip a coin. If heads, each Defending Pokémon is now Paralyzed." => Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "ApplySpecialCondition", "target": "OppActive", "condition": "Paralyzed" },
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip a coin. If heads, prevent all effects of an attack, including damage, done to Scyther ex during your opponent's next turn." => Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "AddMarker", "target": "SelfActive", "name": "PreventAllDamageAndEffects", "expires_after_turns": 1 },
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip a coin. If heads, put damage counters on the Defending Pokémon until it is 10 HP away from being Knocked Out." => Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": rs_custom_attack_ast(set_code, card_number, attack_name, None),
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip a coin. If heads, the Defending Pokémon is now Burned." => Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "ApplySpecialCondition", "target": "OppActive", "condition": "Burned" },
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip a coin. If heads, the Defending Pokémon is now Confused." => Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "ApplySpecialCondition", "target": "OppActive", "condition": "Confused" },
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip a coin. If heads, the Defending Pokémon is now Paralyzed." => Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "ApplySpecialCondition", "target": "OppActive", "condition": "Paralyzed" },
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip a coin. If heads, the Defending Pokémon is now Poisoned." => Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "ApplySpecialCondition", "target": "OppActive", "condition": "Poisoned" },
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip a coin. If heads, this attack does 10 damage plus 10 more damage." => Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "ModifyDamage", "amount": 10 },
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip a coin. If heads, this attack does 10 damage times the number of damage counters on Aggron." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, Some("PreDamage")),
        ),
        "Flip a coin. If heads, this attack does 10 damage to each Defending Pokémon, and each Defending Pokémon is now Paralyzed." => Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": {
                "op": "Sequence",
                "effects": [
                    { "op": "DealDamage", "target": "OppActive", "amount": 10 },
                    { "op": "ApplySpecialCondition", "target": "OppActive", "condition": "Paralyzed" }
                ]
            },
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip a coin. If heads, this attack does 30 damage plus 30 more damage." => Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "ModifyDamage", "amount": 30 },
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip a coin. If heads, this attack does 40 damage plus 20 more damage." => Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "ModifyDamage", "amount": 20 },
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip a coin. If heads, your opponent returns the Defending Pokémon and all cards attached to it to his or her hand. (If your opponent doesn't have any Benched Pokémon or other Active Pokémon, this attack does nothing.)" => Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": rs_custom_attack_ast(set_code, card_number, attack_name, None),
            "on_tails": { "op": "NoOp" },
            "notes": notes
        })),
        "Flip a coin. If tails, Electrike does 10 damage to itself." => Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "NoOp" },
            "on_tails": { "op": "DealDamage", "target": "SelfActive", "amount": 10 },
            "notes": notes
        })),
        "Flip a coin. If tails, Manectric does 10 damage to itself." => Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "NoOp" },
            "on_tails": { "op": "DealDamage", "target": "SelfActive", "amount": 10 },
            "notes": notes
        })),
        "Flip a coin. If tails, discard a Fire Energy card attached to Torchic." => Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "NoOp" },
            "on_tails": { "op": "DiscardAttachedEnergyByType", "target": "SelfActive", "energy_type": "Fire", "count": 1 },
            "notes": notes
        })),
        "Flip a coin. If tails, this attack does nothing." => Some(json!({
            "op": "FlipCoins",
            "count": 1,
            "on_heads": { "op": "NoOp" },
            "on_tails": { "op": "ModifyDamage", "amount": -10000 },
            "notes": notes
        })),
        "If 1 Energy is attached to Sceptile, the Defending Pokémon is now Asleep. If 2 Energy is attached to Sceptile, the Defending Pokémon is now Poisoned. If 3 Energy is attached to Sceptile, the Defending Pokémon is now Asleep and Poisoned. If 4 or more Energy is attached to Sceptile, the Defending Pokémon is now Asleep, Burned, and Poisoned." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, None),
        ),
        "If Ralts and the Defending Pokémon have a different amount of Energy attached to them, this attack's base damage is 10 instead of 40." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, None),
        ),
        "If any of your opponent's Active Pokémon are Evolved Pokémon, search your deck for any 1 card and put it into your hand. Shuffle your deck afterward." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, None),
        ),
        "If the Defending Pokémon has any damage counters on it, this attack does 20 damage plus 20 more damage." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, None),
        ),
        "If the Defending Pokémon is a Pokémon-ex, this attack does 40 damage plus 40 more damage." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, None),
        ),
        "If the Defending Pokémon tries to attack during your opponent's next turn, your opponent flips a coin. If tails, that attack does nothing." => Some(json!({
            "op": "AddMarker",
            "target": "OppActive",
            "name": "Smokescreen",
            "expires_after_turns": 1,
            "notes": notes
        })),
        "Koffing does 10 damage to itself." => Some(json!({
            "op": "DealDamage",
            "target": "SelfActive",
            "amount": 10,
            "notes": notes
        })),
        "Move 1 Energy card attached to the Defending Pokémon to the other Defending Pokémon. (Ignore this effect if your opponent has only 1 Defending Pokémon.)" => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, None),
        ),
        "Remove 1 damage counter from each of your Pokémon, including Beautifly." => Some(json!({
            "op": "HealDamageToSelector",
            "player": "Current",
            "selector": { "scope": "SelfAll" },
            "amount": 10,
            "notes": notes
        })),
        "Remove 2 damage counters (1 if there is only 1) from each of your Pokémon. Remove no damage counters from Chansey ex." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, None),
        ),
        "Remove all Special Conditions and 4 damage counters from Wailmer (all if there are less than 4). Wailmer is now Asleep." => Some(json!({
            "op": "Sequence",
            "effects": [
                { "op": "ClearSpecialConditions", "target": "SelfActive" },
                { "op": "HealDamage", "target": "SelfActive", "amount": 40 },
                { "op": "ApplySpecialCondition", "target": "SelfActive", "condition": "Asleep" }
            ],
            "notes": notes
        })),
        "Remove all damage counters from Slakoth. Slakoth can't attack during your next turn." => Some(json!({
            "op": "Sequence",
            "effects": [
                { "op": "HealDamage", "target": "SelfActive", "amount": 1000 },
                { "op": "AddMarker", "target": "SelfActive", "name": "CannotAttack", "expires_after_turns": 1 }
            ],
            "notes": notes
        })),
        "Search your deck for 2 basic Energy cards, show them to your opponent, and put them into your hand. Shuffle your deck afterward." => Some(json!({
            "op": "SearchDeckWithSelector",
            "player": "Current",
            "selector": { "is_energy": true, "energy_kind": "Basic" },
            "count": 2,
            "min": 0,
            "max": 2,
            "destination": "Hand",
            "shuffle": true,
            "reveal": true,
            "notes": notes
        })),
        "Search your deck for Silcoon and Beautifly, or Cascoon and Dustox cards. Show 1 card or both cards of a pair to your opponent and put them into your hand. Shuffle your deck afterward." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, None),
        ),
        "Search your deck for a Lightning Energy card and attach it to 1 of your Pokémon. Shuffle your deck afterward." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, None),
        ),
        "Search your deck for up to 2 cards and put them into your hand. Shuffle your deck afterward." => Some(json!({
            "op": "SearchDeckWithSelector",
            "player": "Current",
            "selector": {},
            "count": 2,
            "min": 0,
            "max": 2,
            "destination": "Hand",
            "shuffle": true,
            "reveal": false,
            "notes": notes
        })),
        "The Defending Pokémon can't retreat until the end of your opponent's next turn." => Some(json!({
            "op": "AddMarker",
            "target": "OppActive",
            "name": "CannotRetreat",
            "expires_after_turns": 1,
            "notes": notes
        })),
        "The Defending Pokémon is now Asleep." => Some(json!({
            "op": "ApplySpecialCondition",
            "target": "OppActive",
            "condition": "Asleep",
            "notes": notes
        })),
        "The Defending Pokémon is now Burned." => Some(json!({
            "op": "ApplySpecialCondition",
            "target": "OppActive",
            "condition": "Burned",
            "notes": notes
        })),
        "The Defending Pokémon is now Confused." => Some(json!({
            "op": "ApplySpecialCondition",
            "target": "OppActive",
            "condition": "Confused",
            "notes": notes
        })),
        "The Defending Pokémon is now Poisoned." => Some(json!({
            "op": "ApplySpecialCondition",
            "target": "OppActive",
            "condition": "Poisoned",
            "notes": notes
        })),
        "The Defending Pokémon is now Poisoned. Put 2 damage counters instead of 1 on the Defending Pokémon between turns." => Some(json!({
            "op": "Sequence",
            "effects": [
                { "op": "ApplySpecialCondition", "target": "OppActive", "condition": "Poisoned" },
                { "op": "AddMarker", "target": "OppActive", "name": "Toxic" }
            ],
            "notes": notes
        })),
        "This attack does 20 damage plus 10 more damage for each Water Energy attached to Wailmer but not used to pay for this attack's Energy cost. You can't add more than 20 damage in this way." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, None),
        ),
        "This attack's damage is not affected by Resistance." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, None),
        ),
        "This attack's damage isn't affected by Weakness, Resistance, Poké-Powers, Poké-Bodies, or any other effects on the Defending Pokémon." => Some(json!({
            "op": "DealDamageNoModifiers",
            "target": "OppActive",
            "amount": 30,
            "notes": notes
        })),
        "Wailord does 20 damage to itself." => Some(json!({
            "op": "DealDamage",
            "target": "SelfActive",
            "amount": 20,
            "notes": notes
        })),
        "You may discard a Darkness Energy card attached to Sharpedo. If you do, this attack does 40 damage plus 30 more damage." => Some(
            rs_custom_attack_ast(set_code, card_number, attack_name, Some("PreDamage")),
        ),
        "Your opponent switches the Defending Pokémon with 1 of his or her Benched Pokémon." => Some(json!({
            "op": "ChoosePokemonTargets",
            "player": "Opponent",
            "selector": { "scope": "OppBench" },
            "min": 1,
            "max": 1,
            "effect": { "op": "SwitchToTarget", "player": "Opponent" },
            "notes": notes
        })),
        _ => None,
    }
}

fn normalize_card(raw: &Value, set_code: &str) -> Value {
    let supertype = raw
        .get("supertype")
        .and_then(Value::as_str)
        .unwrap_or("Trainer");
    let supertype = match supertype {
        "Pokémon" => "Pokemon",
        other => other,
    };
    let name = raw.get("name").and_then(Value::as_str).unwrap_or("Unknown");
    let number = raw.get("number").and_then(Value::as_str).unwrap_or("0");
    let subtypes: Vec<String> = raw
        .get("subtypes")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    let mut tags = HashSet::new();
    if subtypes.iter().any(|s| s == "ex") || name.ends_with(" ex") {
        tags.insert("PokemonEx");
    }
    if subtypes.iter().any(|s| s == "Star") || name.ends_with(" Star") {
        tags.insert("PokemonStar");
    }
    if name.contains('δ') || name.contains("Delta") {
        tags.insert("DeltaSpecies");
    }

    let mut card = json!({
        "set": set_code,
        "number": number,
        "name": name.replace('δ', "Delta"),
        "supertype": supertype,
        "tags": tags.into_iter().collect::<Vec<_>>(),
    });

    match supertype {
        "Pokemon" => {
            let stage = if subtypes.iter().any(|s| s == "Basic") {
                "Basic"
            } else if subtypes.iter().any(|s| s == "Stage 1") {
                "Stage1"
            } else {
                "Stage2"
            };
            let hp = raw
                .get("hp")
                .and_then(Value::as_str)
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0);
            let types = raw.get("types").cloned().unwrap_or_else(|| json!([]));
            let weakness = normalize_modifiers(raw.get("weaknesses"));
            let resistance = normalize_modifiers(raw.get("resistances"));
            let retreat = raw
                .get("convertedRetreatCost")
                .and_then(Value::as_u64)
                .or_else(|| raw.get("retreatCost").and_then(Value::as_array).map(|a| a.len() as u64))
                .unwrap_or(0);

            let attacks = raw
                .get("attacks")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .map(|attack| {
                            let mut obj = serde_json::Map::new();
                            let name = attack.get("name").and_then(Value::as_str).unwrap_or("");
                            obj.insert("name".to_string(), json!(name));
                            let cost = attack.get("cost").cloned().unwrap_or_else(|| json!([]));
                            obj.insert("cost".to_string(), cost);
                            if let Some(damage) = attack.get("damage").and_then(Value::as_str) {
                                if !damage.is_empty() {
                                    if let Ok(value) = damage.parse::<u32>() {
                                        obj.insert("damage".to_string(), json!(value));
                                    } else {
                                        obj.insert("damage".to_string(), json!(damage));
                                    }
                                }
                            }
                            let attack_text = attack.get("text").and_then(Value::as_str).unwrap_or("");
                            let effect_ast = attack_effect_ast(set_code, number, name, attack_text)
                                .unwrap_or_else(|| json!({ "op": "NoOp" }));
                            obj.insert("effect_ast".to_string(), effect_ast);
                            Value::Object(obj)
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            let powers = raw
                .get("abilities")
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .map(|ability| {
                            let kind = ability
                                .get("type")
                                .and_then(Value::as_str)
                                .map(|v| match v {
                                    "Poké-Body" => "PokeBody",
                                    "Poké-Power" => "PokePower",
                                    other => other,
                                })
                                .unwrap_or("PokePower");
                            let power_name = ability.get("name").and_then(Value::as_str).unwrap_or("");
                            let effect_ast = match set_code {
                                "CG" => expansions::power_effect_ast("CG", number, power_name, kind),
                                "DF" => expansions::power_effect_ast("DF", number, power_name, kind),
                                "RS" => expansions::power_effect_ast("RS", number, power_name, kind),
                                _ => None,
                            }
                            .unwrap_or_else(|| json!({ "op": "NoOp" }));
                            json!({
                                "kind": kind,
                                "name": power_name,
                                "effect_ast": effect_ast
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            let evolves_from = raw.get("evolvesFrom").and_then(Value::as_str);
            let obj = card.as_object_mut().expect("card object");
            obj.insert("stage".to_string(), json!(stage));
            if let Some(value) = evolves_from {
                // Replace δ symbol with "Delta" to match how names are processed
                let mut normalized_value = value.replace('δ', "Delta");

                // If this card is a Delta species and its evolves_from doesn't already end with "Delta",
                // automatically append "Delta" to assume it evolves from the Delta variant
                let is_delta = name.contains('δ') || name.contains("Delta");
                if is_delta && !normalized_value.ends_with("Delta") {
                    normalized_value.push_str(" Delta");
                }

                obj.insert("evolves_from".to_string(), json!(normalized_value));
            }
            obj.insert("hp".to_string(), json!(hp));
            obj.insert("types".to_string(), types);
            obj.insert("weakness".to_string(), weakness);
            obj.insert("resistance".to_string(), resistance);
            obj.insert("retreat".to_string(), json!(retreat));
            obj.insert("attacks".to_string(), json!(attacks));
            obj.insert("powers".to_string(), json!(powers));
        }
        "Trainer" => {
            let trainer_kind = if subtypes.iter().any(|s| s == "Supporter") {
                "Supporter"
            } else if subtypes.iter().any(|s| s == "Stadium") {
                "Stadium"
            } else if subtypes.iter().any(|s| s == "Pokémon Tool") {
                "Tool"
            } else {
                "Item"
            };
            let obj = card.as_object_mut().expect("card object");
            obj.insert("trainer_kind".to_string(), json!(trainer_kind));
            let effect_ast = match set_code {
                "CG" => expansions::trainer_effect_ast("CG", number, name, trainer_kind),
                "DF" => expansions::trainer_effect_ast("DF", number, name, trainer_kind),
                "RS" => expansions::trainer_effect_ast("RS", number, name, trainer_kind),
                _ => None,
            }
            .unwrap_or_else(|| json!({ "op": "NoOp" }));
            obj.insert("effect_ast".to_string(), effect_ast);
        }
        "Energy" => {
            let energy_kind = if subtypes.iter().any(|s| s == "Basic") {
                "Basic"
            } else {
                "Special"
            };
            let obj = card.as_object_mut().expect("card object");
            obj.insert("energy_kind".to_string(), json!(energy_kind));
            obj.insert("effect_ast".to_string(), json!({"op": "NoOp"}));
        }
        _ => {}
    }

    card
}

#[cfg(test)]
mod importer_tests {
    use super::*;

    #[test]
    fn poisonpowder_is_not_noop() {
        let raw = json!({
            "name": "Bulbasaur",
            "number": "45",
            "supertype": "Pokémon",
            "subtypes": ["Basic"],
            "hp": "50",
            "types": ["Grass"],
            "attacks": [
                { "name": "Poisonpowder", "cost": ["Grass"], "convertedEnergyCost": 1, "damage": "", "text": "The Defending Pokémon is now Poisoned." }
            ],
            "weaknesses": [{"type":"Psychic","value":"×2"}],
            "convertedRetreatCost": 1
        });

        let normalized = normalize_card(&raw, "CG");
        let attacks = normalized.get("attacks").and_then(Value::as_array).expect("attacks");
        let effect = attacks[0].get("effect_ast").expect("effect_ast");
        assert_eq!(effect.get("op").and_then(Value::as_str), Some("ApplySpecialCondition"));
        assert_eq!(effect.get("target").and_then(Value::as_str), Some("OppActive"));
        assert_eq!(effect.get("condition").and_then(Value::as_str), Some("Poisoned"));
    }

    #[test]
    fn treecko_paralyzing_gaze_is_not_noop() {
        let raw = json!({
            "name": "Treecko",
            "number": "67",
            "supertype": "Pokémon",
            "subtypes": ["Basic"],
            "hp": "40",
            "types": ["Grass"],
            "attacks": [
                { "name": "Paralyzing Gaze", "cost": ["Colorless"], "convertedEnergyCost": 1, "damage": "", "text": "Flip a coin. If heads, the Defending Pokémon is now Paralyzed." }
            ],
            "weaknesses": [{"type":"Fire","value":"×2"}],
            "convertedRetreatCost": 1
        });

        let normalized = normalize_card(&raw, "CG");
        let attacks = normalized.get("attacks").and_then(Value::as_array).expect("attacks");
        let effect = attacks[0].get("effect_ast").expect("effect_ast");
        assert_eq!(effect.get("op").and_then(Value::as_str), Some("FlipCoins"));
        assert_eq!(effect.get("count").and_then(Value::as_u64), Some(1));
        let on_heads = effect.get("on_heads").expect("on_heads");
        assert_eq!(on_heads.get("op").and_then(Value::as_str), Some("ApplySpecialCondition"));
        assert_eq!(on_heads.get("target").and_then(Value::as_str), Some("OppActive"));
        assert_eq!(on_heads.get("condition").and_then(Value::as_str), Some("Paralyzed"));
    }

    #[test]
    fn venusaur_toxic_sleep_is_sequence_with_conditions_and_marker() {
        let raw = json!({
            "name": "Venusaur",
            "number": "28",
            "supertype": "Pokémon",
            "subtypes": ["Stage 2"],
            "hp": "110",
            "types": ["Grass"],
            "attacks": [
                {
                    "name": "Toxic Sleep",
                    "cost": ["Grass", "Grass", "Colorless"],
                    "convertedEnergyCost": 3,
                    "damage": "",
                    "text": "The Defending Pokémon is now Asleep and Poisoned. Put 2 damage counters instead of 1 on the Defending Pokémon between turns."
                }
            ],
            "weaknesses": [{"type":"Fire","value":"×2"}],
            "convertedRetreatCost": 3
        });

        let normalized = normalize_card(&raw, "CG");
        let attacks = normalized
            .get("attacks")
            .and_then(Value::as_array)
            .expect("attacks");
        let effect = attacks[0].get("effect_ast").expect("effect_ast");
        assert_eq!(effect.get("op").and_then(Value::as_str), Some("Sequence"));
        let effects = effect.get("effects").and_then(Value::as_array).expect("effects");
        assert_eq!(effects.len(), 3);
        assert_eq!(effects[0].get("op").and_then(Value::as_str), Some("ApplySpecialCondition"));
        assert_eq!(effects[0].get("condition").and_then(Value::as_str), Some("Asleep"));
        assert_eq!(effects[1].get("op").and_then(Value::as_str), Some("ApplySpecialCondition"));
        assert_eq!(effects[1].get("condition").and_then(Value::as_str), Some("Poisoned"));
        assert_eq!(effects[2].get("op").and_then(Value::as_str), Some("AddMarker"));
        assert_eq!(effects[2].get("name").and_then(Value::as_str), Some("Toxic Sleep"));
    }
}

fn normalize_modifiers(value: Option<&Value>) -> Value {
    let items = match value.and_then(Value::as_array) {
        Some(items) => items,
        None => return json!([]),
    };
    let mapped: Vec<Value> = items
        .iter()
        .filter_map(|item| {
            let type_ = item.get("type").and_then(Value::as_str)?;
            let value = item.get("value").and_then(Value::as_str).unwrap_or("");
            let value = value.replace('×', "x");
            Some(json!({ "type": type_, "value": value }))
        })
        .collect();
    json!(mapped)
}
