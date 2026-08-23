//! Dumps the RAV card catalog as tab-separated records.
//!
//! Deck construction and the hillclimb mutator both need to reason about the
//! real pool -- costs, types, bodies, keywords, and produced mana -- rather
//! than a hand-maintained copy that drifts from the catalog. Reading it out of
//! the engine's own definitions keeps that impossible.

use cardbench_magic_engine::{CardDefinition, CardType, Color, ManaCost};
use cardbench_magic_rav::card_definitions;

fn color_letter(color: Color) -> char {
    match color {
        Color::White => 'W',
        Color::Blue => 'U',
        Color::Black => 'B',
        Color::Red => 'R',
        Color::Green => 'G',
        Color::Colorless => 'C',
    }
}

fn cost_string(cost: &ManaCost) -> String {
    let mut text = String::new();
    if cost.generic > 0 || (cost.colored.is_empty() && cost.hybrid.is_empty()) {
        text.push_str(&cost.generic.to_string());
    }
    for color in &cost.colored {
        text.push(color_letter(*color));
    }
    for symbol in &cost.hybrid {
        text.push('(');
        text.push(color_letter(symbol.first));
        text.push('/');
        text.push(color_letter(symbol.second));
        text.push(')');
    }
    text
}

fn mana_value(cost: &ManaCost) -> u32 {
    let symbols = u32::try_from(cost.colored.len() + cost.hybrid.len()).unwrap_or(u32::MAX);
    u32::from(cost.generic) + symbols
}

fn type_string(definition: &CardDefinition) -> String {
    let mut parts: Vec<&str> = definition
        .card_types
        .iter()
        .map(|kind| match kind {
            CardType::Artifact => "Artifact",
            CardType::Creature => "Creature",
            CardType::Land => "Land",
            CardType::Enchantment => "Enchantment",
            CardType::Instant => "Instant",
            CardType::Planeswalker => "Planeswalker",
            CardType::Sorcery => "Sorcery",
        })
        .collect();
    parts.sort_unstable();
    parts.join("+")
}

fn main() {
    let catalog = card_definitions();
    println!("schema_version=cardbench.magic.card-pool.v1");
    println!("card_count={}", catalog.len());
    println!(
        "id\tname\tcost\tmv\tcolors\tmana_colors\ttypes\tbasic\tpower\ttoughness\tkeywords\teffects"
    );
    let mut rows: Vec<&CardDefinition> = catalog.iter().collect();
    rows.sort_by_key(|definition| (mana_value(&definition.mana_cost), definition.id));
    for definition in rows {
        let colors: String = definition.colors.iter().map(|c| color_letter(*c)).collect();
        let mana_colors: String = definition
            .mana_colors
            .iter()
            .map(|c| color_letter(*c))
            .collect();
        let keywords: Vec<String> = definition
            .keywords
            .iter()
            .map(|keyword| format!("{keyword:?}"))
            .collect();
        let effects: Vec<String> = definition
            .effects
            .iter()
            .map(|effect| {
                let rendered = format!("{effect:?}");
                rendered
                    .split_once([' ', '{', '('])
                    .map_or(rendered.clone(), |(head, _)| head.to_owned())
            })
            .collect();
        println!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            definition.id,
            definition.name,
            cost_string(&definition.mana_cost),
            mana_value(&definition.mana_cost),
            if colors.is_empty() { "-" } else { &colors },
            if mana_colors.is_empty() {
                "-"
            } else {
                &mana_colors
            },
            type_string(definition),
            definition.is_basic_land,
            definition.power.map_or("-".to_owned(), |p| p.to_string()),
            definition
                .toughness
                .map_or("-".to_owned(), |t| t.to_string()),
            if keywords.is_empty() {
                "-".to_owned()
            } else {
                keywords.join(",")
            },
            if effects.is_empty() {
                "-".to_owned()
            } else {
                effects.join(",")
            },
        );
    }
}
