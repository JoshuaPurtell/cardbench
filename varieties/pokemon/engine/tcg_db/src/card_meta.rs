use std::collections::HashMap;

use rusqlite::Connection;
use thiserror::Error;

use tcg_core::{Attack, AttackCost, CardDefId, CardMeta, CardMetaMap, EffectAst, Resistance, Type, Weakness};
use tcg_core::Stage;

#[derive(Debug, Error)]
pub enum CardMetaError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
}

pub fn load_card_meta_map(conn: &Connection) -> Result<CardMetaMap, CardMetaError> {
    // Load attacks first into a hashmap by card_def_id
    let attacks_by_card = load_attacks(conn)?;

    let mut stmt = conn.prepare(
        "SELECT card_def_id, name, supertype, stage, evolves_from, subtypes_json, tags_json, trainer_kind, energy_kind, script_payload, types_json, weakness_json, resist_json, retreat_cost, hp FROM cards",
    )?;
    let rows = stmt.query_map([], |row| {
        let card_def_id: String = row.get(0)?;
        let name: String = row.get(1)?;
        let supertype: String = row.get(2)?;
        let stage: Option<String> = row.get(3)?;
        let evolves_from: Option<String> = row.get(4)?;
        let subtypes_json: String = row.get(5)?;
        let tags_json: String = row.get(6)?;
        let trainer_kind: Option<String> = row.get(7)?;
        let energy_kind: Option<String> = row.get(8)?;
        let script_payload: String = row.get(9)?;
        let types_json: Option<String> = row.get(10)?;
        let weakness_json: Option<String> = row.get(11)?;
        let resist_json: Option<String> = row.get(12)?;
        let retreat_cost: Option<i64> = row.get(13)?;
        let hp: Option<i64> = row.get(14)?;
        Ok((
            card_def_id,
            name,
            supertype,
            stage,
            evolves_from,
            subtypes_json,
            tags_json,
            trainer_kind,
            energy_kind,
            script_payload,
            types_json,
            weakness_json,
            resist_json,
            retreat_cost,
            hp,
        ))
    })?;

    let mut meta_map: HashMap<CardDefId, CardMeta> = HashMap::new();
    for row in rows {
        let (card_def_id, name, supertype, stage, evolves_from, subtypes_json, tags_json, trainer_kind, energy_kind, script_payload, types_json, weakness_json, resist_json, retreat_cost, hp) = row?;
        let is_basic = supertype == "Pokemon" && stage.as_deref() == Some("Basic");
        let subtypes: Vec<String> =
            serde_json::from_str(&subtypes_json).unwrap_or_else(|_| Vec::new());
        let is_stadium = subtypes.iter().any(|s| s == "Stadium");
        let is_tool = subtypes.iter().any(|s| s == "Pokémon Tool");
        let tags: Vec<String> = serde_json::from_str(&tags_json).unwrap_or_else(|_| Vec::new());
        let is_ex = tags.iter().any(|tag| tag == "PokemonEx");
        let is_star = tags.iter().any(|tag| tag == "PokemonStar");
        let is_delta = tags.iter().any(|tag| tag == "DeltaSpecies");
        let is_pokemon = supertype == "Pokemon";
        let is_energy = supertype == "Energy";
        let stage = match stage.as_deref() {
            Some("Stage1") => Stage::Stage1,
            Some("Stage2") => Stage::Stage2,
            _ => Stage::Basic,
        };
        let (types, weakness, resistance, retreat_cost) = if is_pokemon {
            let types = parse_types(&types_json);
            let weakness = parse_weakness(&weakness_json);
            let resistance = parse_resistance(&resist_json);
            let retreat_cost = retreat_cost.map(|value| value as u8);
            (types, weakness, resistance, retreat_cost)
        } else {
            (Vec::new(), None, None, None)
        };
        let provides = if is_energy {
            let mut provides = parse_provides_from_payload(&script_payload);
            if provides.is_empty() {
                if energy_kind.as_deref() == Some("Basic") {
                    if let Some(type_) = type_from_name(&name) {
                        provides.push(type_);
                    }
                }
            }
            if provides.is_empty() {
                provides.push(Type::Colorless);
            }
            provides
        } else {
            Vec::new()
        };
        let trainer_effect = match trainer_kind.as_deref() {
            Some("Tool") | Some("Stadium") | Some("Item") | Some("Supporter") => {
                serde_json::from_str(&script_payload).ok()
            }
            _ => None,
        };
        let hp = if is_pokemon {
            hp.unwrap_or(0).max(0) as u16
        } else {
            0
        };
        let def_id = CardDefId::new(card_def_id);
        let attacks = attacks_by_card.get(&def_id).cloned().unwrap_or_default();
        meta_map.insert(
            def_id,
            CardMeta {
                name,
                is_basic,
                is_tool,
                is_stadium,
                is_pokemon,
                is_energy,
                hp,
                energy_kind,
                provides,
                trainer_kind,
                is_ex,
                is_star,
                is_delta,
                stage,
                types,
                weakness,
                resistance,
                retreat_cost,
                trainer_effect,
                evolves_from,
                attacks,
                card_type: if is_pokemon {
                    "Pokemon".to_string()
                } else if is_energy {
                    "Energy".to_string()
                } else {
                    "Trainer".to_string()
                },
                delta_species: is_delta,
            },
        );
    }

    Ok(meta_map)
}

fn load_attacks(conn: &Connection) -> Result<HashMap<CardDefId, Vec<Attack>>, CardMetaError> {
    let mut stmt = conn.prepare(
        "SELECT card_def_id, name, cost_json, damage_expr, effect_ast FROM attacks ORDER BY card_def_id, idx",
    )?;

    let rows = stmt.query_map([], |row| {
        let card_def_id: String = row.get(0)?;
        let name: String = row.get(1)?;
        let cost_json: String = row.get(2)?;
        let damage_expr: String = row.get(3)?;
        let effect_ast_json: String = row.get(4)?;
        Ok((card_def_id, name, cost_json, damage_expr, effect_ast_json))
    })?;

    let mut attacks_by_card: HashMap<CardDefId, Vec<Attack>> = HashMap::new();
    for row in rows {
        let (card_def_id, name, cost_json, damage_expr, effect_ast_json) = row?;

        // Parse cost
        let cost = parse_attack_cost(&cost_json);

        // Parse damage (strip + suffix for variable damage)
        let trimmed_expr = damage_expr.trim();
        let is_multiplier = trimmed_expr.contains('x') || trimmed_expr.contains('×');
        let damage = if is_multiplier {
            0
        } else {
            trimmed_expr
                .trim_end_matches('+')
                .trim_end_matches('×')
                .parse::<u16>()
                .unwrap_or(0)
        };

        // Determine attack type from first non-colorless energy in cost, or default to Colorless
        let attack_type = cost.types.iter()
            .find(|t| **t != Type::Colorless)
            .copied()
            .unwrap_or(Type::Colorless);

        // Parse effect AST
        let effect_ast: Option<EffectAst> = if effect_ast_json.is_empty() || effect_ast_json == "null" {
            None
        } else {
            serde_json::from_str(&effect_ast_json).ok()
        };

        let attack = Attack {
            name,
            damage,
            attack_type,
            cost,
            effect_ast,
        };

        attacks_by_card
            .entry(CardDefId::new(card_def_id))
            .or_default()
            .push(attack);
    }

    Ok(attacks_by_card)
}

fn parse_attack_cost(cost_json: &str) -> AttackCost {
    // Cost is stored as an array of type strings like ["Colorless", "Grass", "Grass"]
    let type_strings: Vec<String> = serde_json::from_str(cost_json).unwrap_or_default();
    let mut types = Vec::new();

    for type_str in &type_strings {
        if let Some(type_) = parse_type(type_str) {
            types.push(type_);
        }
    }

    AttackCost {
        total_energy: types.len() as u8,
        types,
    }
}

fn parse_types(value: &Option<String>) -> Vec<Type> {
    let Some(raw) = value.as_ref() else {
        return Vec::new();
    };
    let Ok(items) = serde_json::from_str::<Vec<String>>(raw) else {
        return Vec::new();
    };
    items
        .into_iter()
        .filter_map(|item| parse_type(item.as_str()))
        .collect()
}

fn parse_weakness(value: &Option<String>) -> Option<Weakness> {
    let Some(raw) = value.as_ref() else {
        return None;
    };
    let Ok(items) = serde_json::from_str::<Vec<serde_json::Value>>(raw) else {
        return None;
    };
    let first = items.first()?;
    let type_ = first.get("type").and_then(serde_json::Value::as_str)?;
    let value = first.get("value").and_then(serde_json::Value::as_str)?;
    let multiplier = value
        .trim_start_matches('×')
        .trim_start_matches('x')
        .parse::<u8>()
        .ok()?;
    Some(Weakness {
        type_: parse_type(type_)?,
        multiplier,
    })
}

fn parse_resistance(value: &Option<String>) -> Option<Resistance> {
    let Some(raw) = value.as_ref() else {
        return None;
    };
    let Ok(items) = serde_json::from_str::<Vec<serde_json::Value>>(raw) else {
        return None;
    };
    let first = items.first()?;
    let type_ = first.get("type").and_then(serde_json::Value::as_str)?;
    let value = first.get("value").and_then(serde_json::Value::as_str)?;
    let amount = value
        .trim_start_matches('-')
        .parse::<u16>()
        .ok()?;
    Some(Resistance {
        type_: parse_type(type_)?,
        value: amount,
    })
}

fn parse_provides_from_payload(payload: &str) -> Vec<Type> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(payload) else {
        return Vec::new();
    };
    let Some(items) = value.get("provides").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| item.as_str())
        .filter_map(parse_type)
        .collect()
}

fn type_from_name(name: &str) -> Option<Type> {
    let token = name.split_whitespace().next()?;
    parse_type(token)
}

fn parse_type(value: &str) -> Option<Type> {
    match value {
        "Grass" => Some(Type::Grass),
        "Fire" => Some(Type::Fire),
        "Water" => Some(Type::Water),
        "Lightning" => Some(Type::Lightning),
        "Psychic" => Some(Type::Psychic),
        "Fighting" => Some(Type::Fighting),
        "Darkness" => Some(Type::Darkness),
        "Metal" => Some(Type::Metal),
        "Colorless" => Some(Type::Colorless),
        _ => None,
    }
}
