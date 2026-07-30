//! Executable Ravnica: City of Guilds fixture slice.
//!
//! The module contains only the limited card semantics exercised by its published
//! scenarios. It intentionally does not reproduce Oracle prose, card images, flavor
//! text, collector numbers, or any hidden benchmark cases.

#![forbid(unsafe_code)]

mod catalog;
mod scenarios;

pub use catalog::{
    CardSemanticStatus, CatalogCard, CatalogResolutionError, CatalogValidationError,
    RAV_MAIN_SET_BASIC_LAND_PRINTING_COUNT, RAV_MAIN_SET_CATALOG_MANIFEST,
    RAV_MAIN_SET_EXPECTED_PRINTING_COUNT, RAV_MAIN_SET_EXPECTED_UNIQUE_NAME_COUNT,
    executable_definition_id_for_collector, parse_rav_main_set_catalog, rav_main_set_catalog,
    validate_rav_main_set_catalog,
};

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, ConvokeContribution, ConvokePayment, DeckEntry,
    DeckList, DeckRules, Effect, Game, Keyword, ManaCost, PlayerId, RulesError, Target, TokenSpec,
    Zone,
};

pub const SET_CODE: &str = "RAV";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScenarioResult {
    pub id: String,
    pub event_log: Vec<String>,
    pub digest: String,
    pub summary: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManifestValidationError(pub String);

impl std::fmt::Display for ManifestValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ManifestValidationError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeckFixture {
    pub id: String,
    pub name: String,
    pub policy: String,
    pub deck: DeckList,
}

#[must_use]
#[allow(clippy::too_many_lines)] // Declarative card fixture catalog is intentionally kept together.
pub fn card_definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: "RAV-CHAR",
            name: "Char",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["damage", "self-damage"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::DealDamage {
                    amount: 4,
                    target: cardbench_magic_engine::TargetRequirement::Any,
                },
                Effect::DealDamageController { amount: 2 },
            ],
        },
        CardDefinition {
            id: "RAV-LIGHTNING-HELIX",
            name: "Lightning Helix",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::White, Color::Red]),
            colors: colors([Color::White, Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["damage", "life-gain"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::DealDamage {
                    amount: 3,
                    target: cardbench_magic_engine::TargetRequirement::Any,
                },
                Effect::GainLifeController { amount: 3 },
            ],
        },
        CardDefinition {
            id: "RAV-SCATTER-THE-SEEDS",
            name: "Scatter the Seeds",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Green, Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["convoke", "token-creation"],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Convoke],
            effects: vec![Effect::CreateToken {
                token: TokenSpec::saproling(),
                count: 3,
            }],
        },
        CardDefinition {
            id: "RAV-GATHER-COURAGE",
            name: "Gather Courage",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["convoke", "targeted-layer-7-modifier"],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Convoke],
            effects: vec![Effect::ModifyTargetPtUntilEndOfTurn {
                power: 2,
                toughness: 2,
            }],
        },
        CardDefinition {
            id: "RAV-GOLGARI-BROWNSCALE",
            name: "Golgari Brownscale",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Green, Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["dredge", "base-characteristics"],
            power: Some(2),
            toughness: Some(3),
            keywords: vec![Keyword::Dredge(2)],
            effects: vec![],
        },
        // Compatibility scope: normal creature casting, base characteristics,
        // and the engine's existing Dredge replacement. Its separate graveyard
        // trigger is intentionally unsupported.
        CardDefinition {
            id: "RAV-GOLGARI-THUG",
            name: "Golgari Thug",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["dredge", "base-characteristics"],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![Keyword::Dredge(4)],
            effects: vec![],
        },
        // Compatibility scope: normal creature casting, base characteristics,
        // and Dredge. Combat keywords and its damage-triggered destruction
        // behavior are deliberately unsupported.
        CardDefinition {
            id: "RAV-STINKWEED-IMP",
            name: "Stinkweed Imp",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["dredge", "base-characteristics"],
            power: Some(1),
            toughness: Some(2),
            keywords: vec![Keyword::Dredge(5)],
            effects: vec![],
        },
        // Compatibility scope: normal creature casting, base characteristics,
        // and Dredge only. No additional card-specific behavior is implied.
        CardDefinition {
            id: "RAV-GREATER-MOSSDOG",
            name: "Greater Mossdog",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["dredge", "base-characteristics"],
            power: Some(3),
            toughness: Some(3),
            keywords: vec![Keyword::Dredge(3)],
            effects: vec![],
        },
        CardDefinition {
            id: "RAV-MUDDLE-THE-MIXTURE",
            name: "Muddle the Mixture",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Blue, Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["counter-target-instant-or-sorcery-spell", "transmute"],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Transmute(ManaCost::with_colors(
                1,
                [Color::Blue, Color::Blue],
            ))],
            effects: vec![Effect::CounterTargetInstantOrSorcerySpell],
        },
        // This compatibility definition is deliberately limited to the target
        // creature's temporary layer-7 modifier and transmute; it does not
        // assert full-card or full-rules fidelity.
        CardDefinition {
            id: "RAV-DIZZY-SPELL",
            name: "Dizzy Spell",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["targeted-layer-7-modifier", "transmute"],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Transmute(ManaCost::with_colors(
                1,
                [Color::Blue, Color::Blue],
            ))],
            effects: vec![Effect::ModifyTargetPtUntilEndOfTurn {
                power: -3,
                toughness: 0,
            }],
        },
        // Only the hand-zone transmute activation is executable. The printed
        // creature-destruction spell effect has no representation in this
        // engine slice and is deliberately non-covered.
        CardDefinition {
            id: "RAV-BRAINSPOIL",
            name: "Brainspoil",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Black, Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &["transmute"],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Transmute(ManaCost::with_colors(
                1,
                [Color::Black, Color::Black],
            ))],
            effects: vec![],
        },
        // This compatibility definition covers only the target creature's
        // temporary layer-7 modifier and dredge.
        CardDefinition {
            id: "RAV-DARKBLAST",
            name: "Darkblast",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["targeted-layer-7-modifier", "dredge"],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Dredge(3)],
            effects: vec![Effect::ModifyTargetPtUntilEndOfTurn {
                power: -1,
                toughness: -1,
            }],
        },
        CardDefinition {
            id: "RAV-RALLY-THE-RIGHTEOUS",
            name: "Rally the Righteous",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Red, Color::White]),
            colors: colors([Color::Red, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["radiance", "untap", "layer-7-modifier"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::RadianceUntapAndModifyUntilEndOfTurn {
                power: 2,
                toughness: 0,
            }],
        },
        CardDefinition {
            id: "RAV-LAST-GASP",
            name: "Last Gasp",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["layer-7-modifier"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::ModifyTargetPtUntilEndOfTurn {
                power: -3,
                toughness: -3,
            }],
        },
        CardDefinition {
            id: "RAV-SIEGE-WURM",
            name: "Siege Wurm",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(5, [Color::Green, Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["convoke", "base-characteristics"],
            power: Some(5),
            toughness: Some(5),
            keywords: vec![Keyword::Convoke],
            effects: vec![],
        },
        // Compatibility scope: normal creature casting, base characteristics,
        // and the existing Convoke payment hook. Its enter-the-battlefield
        // counter behavior is intentionally unsupported.
        CardDefinition {
            id: "RAV-ROOT-KIN-ALLY",
            name: "Root-Kin Ally",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Green, Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["convoke", "base-characteristics"],
            power: Some(3),
            toughness: Some(3),
            keywords: vec![Keyword::Convoke],
            effects: vec![],
        },
        // This CardBench-authored compatibility definition uses only public
        // identity, mana-cost, type, color, and base-characteristic facts. It
        // deliberately includes no copied rules text, art, flavor text, or
        // claim beyond normal creature casting and permanent characteristics.
        CardDefinition {
            id: "RAV-WATCHWOLF",
            name: "Watchwolf",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Green, Color::White]),
            colors: colors([Color::Green, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["colored-cost-casting", "base-characteristics"],
            power: Some(3),
            toughness: Some(3),
            keywords: vec![],
            effects: vec![],
        },
        // Public RAV #261 verification establishes that this is a vanilla
        // artifact creature: its published rules field is empty, so this
        // definition does not omit a printed ability. The compatibility scope
        // is therefore normal colorless-cost casting and base characteristics.
        CardDefinition {
            id: "RAV-GLASS-GOLEM",
            name: "Glass Golem",
            set_code: SET_CODE,
            mana_cost: ManaCost::new(5),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Artifact, CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "colorless-cost-casting",
                "artifact-creature-base-characteristics",
            ],
            power: Some(6),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        basic_land("RAV-PLAINS", "Plains", Color::White),
        basic_land("RAV-ISLAND", "Island", Color::Blue),
        basic_land("RAV-SWAMP", "Swamp", Color::Black),
        basic_land("RAV-MOUNTAIN", "Mountain", Color::Red),
        basic_land("RAV-FOREST", "Forest", Color::Green),
    ]
}

/// Ensures that complete catalog coverage cannot quietly change the executable
/// boundary. Every executable definition must be explicitly referenced by a catalog
/// printing with the same name, and every catalog-only printing must stay absent
/// from the game-definition map.
pub fn validate_rav_catalog_executable_boundary() -> Result<(), CatalogValidationError> {
    validate_rav_main_set_catalog()?;
    let definitions = card_definitions();
    let definitions_by_id = definitions
        .iter()
        .map(|definition| (definition.id, definition))
        .collect::<std::collections::BTreeMap<_, _>>();
    if definitions_by_id.len() != definitions.len() {
        return Err(CatalogValidationError(
            "RAV executable definitions contain a duplicate id".to_owned(),
        ));
    }
    let executable_definition_names = definitions
        .iter()
        .map(|definition| definition.name)
        .collect::<BTreeSet<_>>();
    let mut catalog_definition_ids = BTreeSet::new();
    for card in rav_main_set_catalog() {
        match card.semantic_status {
            CardSemanticStatus::ExecutableCompatibilitySlice { definition_id } => {
                let definition = definitions_by_id.get(definition_id).ok_or_else(|| {
                    CatalogValidationError(format!(
                        "RAV #{} `{}` maps to missing executable definition `{definition_id}`",
                        card.collector_number, card.name
                    ))
                })?;
                if definition.name != card.name {
                    return Err(CatalogValidationError(format!(
                        "RAV #{} catalog name `{}` does not match executable `{definition_id}` name `{}`",
                        card.collector_number, card.name, definition.name
                    )));
                }
                catalog_definition_ids.insert(definition_id);
            }
            CardSemanticStatus::CatalogOnly { .. } => {
                if executable_definition_names.contains(card.name) {
                    return Err(CatalogValidationError(format!(
                        "RAV #{} `{}` is catalog-only but is present in executable definitions",
                        card.collector_number, card.name
                    )));
                }
            }
        }
    }
    let definition_ids = definitions_by_id.into_keys().collect::<BTreeSet<_>>();
    if catalog_definition_ids != definition_ids {
        return Err(CatalogValidationError(
            "RAV executable definitions and catalog executable mappings differ".to_owned(),
        ));
    }
    Ok(())
}

#[must_use]
pub fn set_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

#[must_use]
pub fn magic_root() -> PathBuf {
    set_root()
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .expect("RAV crate must live at varieties/magic/sets/ravnica_city_of_guilds")
}

/// Validates the three original-block TOML manifests without requiring a runtime TOML
/// dependency. The manifests deliberately use scalar and string-array fields only.
pub fn validate_block_manifests() -> Result<(), ManifestValidationError> {
    let required = [
        (
            "ravnica_city_of_guilds",
            "ravnica-city-of-guilds",
            "RAV",
            1_u8,
            "[\"Boros\", \"Dimir\", \"Golgari\", \"Selesnya\"]",
        ),
        (
            "guildpact",
            "guildpact",
            "GPT",
            2,
            "[\"Gruul\", \"Izzet\", \"Orzhov\"]",
        ),
        (
            "dissension",
            "dissension",
            "DIS",
            3,
            "[\"Azorius\", \"Rakdos\", \"Simic\"]",
        ),
    ];
    for (directory, id, code, order, guilds) in required {
        let path = magic_root()
            .join("sets")
            .join(directory)
            .join("expansion.toml");
        let contents = fs::read_to_string(&path)
            .map_err(|error| ManifestValidationError(format!("{}: {error}", path.display())))?;
        for expected in [
            "schema_version = \"cardbench.magic.expansion.v2\"".to_owned(),
            format!("id = \"{id}\""),
            format!("set_code = \"{code}\""),
            "block = \"ravnica\"".to_owned(),
            format!("release_order = {order}"),
            format!("guilds = {guilds}"),
            "minimum_deck_size = 60".to_owned(),
            "maximum_copies = 4".to_owned(),
            "sideboard_size = 15".to_owned(),
            "card_art = \"not-distributed\"".to_owned(),
        ] {
            if !contents.contains(&expected) {
                return Err(ManifestValidationError(format!(
                    "{} is missing `{expected}`",
                    path.display()
                )));
            }
        }
    }
    Ok(())
}

/// Checks the published deck-construction fixture against this slice's definitions.
/// It intentionally validates only public deck data; held-out pools stay absent from
/// the repository by contract.
pub fn validate_shown_deck_pool() -> Result<(), ManifestValidationError> {
    let path = set_root().join("decks/shown_pool.toml");
    let contents = fs::read_to_string(&path)
        .map_err(|error| ManifestValidationError(format!("{}: {error}", path.display())))?;
    let catalog: std::collections::BTreeMap<&'static str, CardDefinition> = card_definitions()
        .into_iter()
        .map(|definition| (definition.id, definition))
        .collect();
    let mut mainboard = Vec::new();
    let mut current_card = None;
    let mut entries = 0_u8;
    for line in contents.lines().map(str::trim) {
        if let Some(value) = line.strip_prefix("card = ") {
            current_card = Some(value.trim_matches('"'));
        }
        if let Some(value) = line.strip_prefix("count = ") {
            let count: u8 = value.parse().map_err(|error| {
                ManifestValidationError(format!(
                    "{}: invalid deck count `{value}`: {error}",
                    path.display()
                ))
            })?;
            let card = current_card.ok_or_else(|| {
                ManifestValidationError(format!("{}: count appears before card", path.display()))
            })?;
            let definition = catalog.get(card).ok_or_else(|| {
                ManifestValidationError(format!(
                    "{}: unknown RAV fixture card `{card}`",
                    path.display()
                ))
            })?;
            if definition.set_code != SET_CODE {
                return Err(ManifestValidationError(format!(
                    "{}: `{card}` is outside RAV",
                    path.display()
                )));
            }
            mainboard.push(DeckEntry {
                card: card.to_owned(),
                count,
            });
            entries += 1;
            current_card = None;
        }
    }
    if entries == 0 {
        return Err(ManifestValidationError(format!(
            "{}: shown mainboard has no entries",
            path.display()
        )));
    }
    if !contents.contains("sideboard_cards = 0") {
        return Err(ManifestValidationError(format!(
            "{}: fixture must declare its sideboard count",
            path.display()
        )));
    }
    DeckList {
        mainboard,
        sideboard: vec![],
    }
    .validate(
        &catalog,
        DeckRules {
            minimum_mainboard_size: 60,
            maximum_copies: 4,
            maximum_sideboard_size: 15,
        },
    )
    .map_err(|error| ManifestValidationError(format!("{}: {error}", path.display())))?;
    Ok(())
}

/// Loads and validates the public, CardBench-authored reference decks used by
/// the Rust policy development matches. The shown deck index is the source of
/// truth, so adding another public deck does not require changing Rust loader code.
pub fn load_reference_decks() -> Result<Vec<DeckFixture>, ManifestValidationError> {
    let index_path = set_root().join("decks/reference_decks.toml");
    let index = fs::read_to_string(&index_path)
        .map_err(|error| ManifestValidationError(format!("{}: {error}", index_path.display())))?;
    let mut filenames = Vec::new();
    for line in index.lines().map(str::trim) {
        if let Some(path) = line.strip_prefix("path = ") {
            let filename = path.trim_matches('"');
            if filename.contains('/') || filename.is_empty() {
                return Err(ManifestValidationError(format!(
                    "{}: deck path must be a nonempty filename",
                    index_path.display()
                )));
            }
            filenames.push(filename.to_owned());
        }
    }
    if filenames.is_empty() {
        return Err(ManifestValidationError(format!(
            "{}: reference deck index has no deck paths",
            index_path.display()
        )));
    }
    filenames
        .iter()
        .map(|filename| load_deck_fixture(filename))
        .collect()
}

fn load_deck_fixture(filename: &str) -> Result<DeckFixture, ManifestValidationError> {
    let path = set_root().join("decks").join(filename);
    let contents = fs::read_to_string(&path)
        .map_err(|error| ManifestValidationError(format!("{}: {error}", path.display())))?;
    let mut id = String::new();
    let mut name = String::new();
    let mut policy = String::new();
    let mut mainboard = Vec::new();
    let mut current_card = None;
    let mut in_mainboard = false;
    for line in contents.lines().map(str::trim) {
        if line == "[[mainboard]]" {
            in_mainboard = true;
        } else if line.starts_with('[') {
            in_mainboard = false;
        } else if let Some(value) = line.strip_prefix("id = ") {
            value.trim_matches('"').clone_into(&mut id);
        } else if let Some(value) = line.strip_prefix("name = ") {
            value.trim_matches('"').clone_into(&mut name);
        } else if let Some(value) = line.strip_prefix("policy = ") {
            value.trim_matches('"').clone_into(&mut policy);
        } else if in_mainboard && let Some(value) = line.strip_prefix("card = ") {
            current_card = Some(value.trim_matches('"'));
        } else if in_mainboard && let Some(value) = line.strip_prefix("count = ") {
            let card = current_card.take().ok_or_else(|| {
                ManifestValidationError(format!("{}: count appears before card", path.display()))
            })?;
            let count = value.parse().map_err(|error| {
                ManifestValidationError(format!(
                    "{}: invalid count `{value}`: {error}",
                    path.display()
                ))
            })?;
            mainboard.push(DeckEntry {
                card: card.to_owned(),
                count,
            });
        }
    }
    if id.is_empty() || name.is_empty() || policy.is_empty() || mainboard.is_empty() {
        return Err(ManifestValidationError(format!(
            "{}: deck requires id, name, policy, and mainboard entries",
            path.display()
        )));
    }
    let catalog = rav_catalog();
    let deck = DeckList {
        mainboard,
        sideboard: vec![],
    };
    deck.validate(
        &catalog,
        DeckRules {
            minimum_mainboard_size: 60,
            maximum_copies: 4,
            maximum_sideboard_size: 15,
        },
    )
    .map_err(|error| ManifestValidationError(format!("{}: {error}", path.display())))?;
    Ok(DeckFixture {
        id,
        name,
        policy,
        deck,
    })
}

fn rav_catalog() -> std::collections::BTreeMap<&'static str, CardDefinition> {
    card_definitions()
        .into_iter()
        .map(|definition| (definition.id, definition))
        .collect()
}

/// Executes the versioned public train scenarios. Each scenario's setup, actions,
/// assertions, marker requirements, and baseline digest live in TOML.
pub fn run_all_scenarios() -> Result<Vec<ScenarioResult>, String> {
    scenarios::run_public_scenarios()
}

/// Legacy in-code RAV examples retained only while downstream consumers migrate to
/// fixture-driven scenarios. They are not part of the reference verifier.
#[doc(hidden)]
pub fn legacy_in_code_scenarios() -> Result<Vec<ScenarioResult>, RulesError> {
    Ok(vec![
        run_scenario("rav_stack_lightning_helix", stack_lightning_helix)?,
        run_scenario("rav_convoke_scatter_the_seeds", convoke_scatter_the_seeds)?,
        run_scenario("rav_dredge_replaces_draw", dredge_replaces_draw)?,
        run_scenario("rav_transmute_search", transmute_search)?,
        run_scenario("rav_radiance_layers", radiance_layers)?,
        run_scenario(
            "rav_last_gasp_state_based_action",
            last_gasp_state_based_action,
        )?,
    ])
}

/// Independent, public engine-eval gate: compare every deterministic scenario event
/// log with its checked-in FNV-1a digest. A candidate engine can use the same scenario
/// manifests, while its output is compared to these fixed expectations.
pub fn verify_reference_event_logs() -> Result<Vec<ScenarioResult>, String> {
    let results = run_all_scenarios()?;
    let replay = run_all_scenarios()?;
    if results != replay {
        return Err("same public RAV scenarios produced different event logs on replay".to_owned());
    }
    Ok(results)
}

fn run_scenario(
    id: &'static str,
    scenario: fn() -> Result<(Game, String), RulesError>,
) -> Result<ScenarioResult, RulesError> {
    let (game, summary) = scenario()?;
    let event_log = game.canonical_event_log();
    Ok(ScenarioResult {
        id: id.to_owned(),
        digest: event_digest(&event_log),
        event_log,
        summary,
    })
}

fn stack_lightning_helix() -> Result<(Game, String), RulesError> {
    let mut game = fresh_game()?;
    let helix = game.add_card(PlayerId(0), "RAV-LIGHTNING-HELIX", Zone::Hand)?;
    game.grant_mana(PlayerId(0), Color::White, 1)?;
    game.grant_mana(PlayerId(0), Color::Red, 1)?;
    game.clear_event_log();
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: helix,
            targets: vec![Target::Player(PlayerId(1))],
            convoke: vec![],
        },
    )?;
    game.pass_priority(PlayerId(1))?;
    game.pass_priority(PlayerId(0))?;
    if game.players[0].life != 23
        || game.players[1].life != 17
        || game.zone_of(helix) != Some(Zone::Graveyard)
    {
        return Err(RulesError::IllegalAction(
            "Lightning Helix scenario postcondition failed",
        ));
    }
    Ok((
        game,
        "stack spell resolved after both players passed; life swing is 6".to_owned(),
    ))
}

fn convoke_scatter_the_seeds() -> Result<(Game, String), RulesError> {
    let mut game = fresh_game()?;
    let scatter = game.add_card(PlayerId(0), "RAV-SCATTER-THE-SEEDS", Zone::Hand)?;
    let first = game.put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE")?;
    let second = game.put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE")?;
    let third = game.put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE")?;
    game.grant_mana(PlayerId(0), Color::Red, 2)?;
    game.clear_event_log();
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: scatter,
            targets: vec![],
            convoke: vec![
                ConvokePayment {
                    creature: first,
                    contribution: ConvokeContribution::Color(Color::Green),
                },
                ConvokePayment {
                    creature: second,
                    contribution: ConvokeContribution::Color(Color::Green),
                },
                ConvokePayment {
                    creature: third,
                    contribution: ConvokeContribution::Generic,
                },
            ],
        },
    )?;
    game.pass_priority(PlayerId(1))?;
    game.pass_priority(PlayerId(0))?;
    let tokens = game.players[0]
        .battlefield
        .iter()
        .filter(|card| {
            game.object(**card)
                .is_ok_and(|object| object.token.is_some())
        })
        .count();
    if tokens != 3
        || ![first, second, third]
            .iter()
            .all(|card| game.object(*card).is_ok_and(|object| object.tapped))
    {
        return Err(RulesError::IllegalAction(
            "convoke scenario postcondition failed",
        ));
    }
    Ok((
        game,
        "three colored/ generic convoke payments created three Saprolings".to_owned(),
    ))
}

fn dredge_replaces_draw() -> Result<(Game, String), RulesError> {
    let mut game = fresh_game()?;
    let brownscale = game.add_card(PlayerId(0), "RAV-GOLGARI-BROWNSCALE", Zone::Graveyard)?;
    let first = game.add_card(PlayerId(0), "RAV-FOREST", Zone::Library)?;
    let second = game.add_card(PlayerId(0), "RAV-FOREST", Zone::Library)?;
    game.clear_event_log();
    game.draw_card(PlayerId(0), Some(brownscale))?;
    if game.zone_of(brownscale) != Some(Zone::Hand)
        || game.zone_of(first) != Some(Zone::Graveyard)
        || game.zone_of(second) != Some(Zone::Graveyard)
    {
        return Err(RulesError::IllegalAction(
            "dredge scenario postcondition failed",
        ));
    }
    Ok((
        game,
        "dredge replaced a draw, milled two, and returned the graveyard card".to_owned(),
    ))
}

fn transmute_search() -> Result<(Game, String), RulesError> {
    let mut game = fresh_game()?;
    let muddle = game.add_card(PlayerId(0), "RAV-MUDDLE-THE-MIXTURE", Zone::Hand)?;
    let helix = game.add_card(PlayerId(0), "RAV-LIGHTNING-HELIX", Zone::Library)?;
    game.grant_mana(PlayerId(0), Color::Blue, 2)?;
    game.grant_mana(PlayerId(0), Color::Green, 1)?;
    game.set_shuffle_seed(41);
    game.clear_event_log();
    game.transmute(PlayerId(0), muddle, helix)?;
    if game.zone_of(muddle) != Some(Zone::Graveyard) || game.zone_of(helix) != Some(Zone::Hand) {
        return Err(RulesError::IllegalAction(
            "transmute scenario postcondition failed",
        ));
    }
    Ok((
        game,
        "transmute discarded one mana-value-two card and searched another".to_owned(),
    ))
}

fn radiance_layers() -> Result<(Game, String), RulesError> {
    let mut game = fresh_game()?;
    let rally = game.add_card(PlayerId(0), "RAV-RALLY-THE-RIGHTEOUS", Zone::Hand)?;
    let target = game.put_on_battlefield(PlayerId(0), "RAV-GOLGARI-BROWNSCALE")?;
    let allied = game.put_on_battlefield(PlayerId(0), "RAV-SIEGE-WURM")?;
    let opposing = game.put_on_battlefield(PlayerId(1), "RAV-GOLGARI-BROWNSCALE")?;
    game.object(target)?;
    game.object(allied)?;
    game.object(opposing)?;
    game.grant_mana(PlayerId(0), Color::White, 1)?;
    game.grant_mana(PlayerId(0), Color::Red, 1)?;
    game.grant_mana(PlayerId(0), Color::Green, 1)?;
    game.clear_event_log();
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: rally,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
        },
    )?;
    game.pass_priority(PlayerId(1))?;
    game.pass_priority(PlayerId(0))?;
    let expected = [(target, 4), (allied, 7), (opposing, 4)];
    if expected.iter().any(
        |(card, power)| !matches!(game.characteristics(*card), Ok(c) if c.power == Some(*power)),
    ) {
        return Err(RulesError::IllegalAction(
            "radiance layer scenario postcondition failed",
        ));
    }
    Ok((
        game,
        "radiance selected every creature sharing green and applied layer 7 modifiers".to_owned(),
    ))
}

fn last_gasp_state_based_action() -> Result<(Game, String), RulesError> {
    let mut game = fresh_game()?;
    let gasp = game.add_card(PlayerId(0), "RAV-LAST-GASP", Zone::Hand)?;
    let victim = game.put_on_battlefield(PlayerId(1), "RAV-GOLGARI-BROWNSCALE")?;
    game.grant_mana(PlayerId(0), Color::Black, 1)?;
    game.clear_event_log();
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: gasp,
            targets: vec![Target::Permanent(victim)],
            convoke: vec![],
        },
    )?;
    game.pass_priority(PlayerId(1))?;
    game.pass_priority(PlayerId(0))?;
    if game.zone_of(victim) != Some(Zone::Graveyard) {
        return Err(RulesError::IllegalAction(
            "Last Gasp did not trigger state-based action",
        ));
    }
    Ok((
        game,
        "continuous -3/-3 effect made toughness zero, then state-based actions moved it".to_owned(),
    ))
}

fn fresh_game() -> Result<Game, RulesError> {
    Game::new(card_definitions(), 2)
}

fn basic_land(id: &'static str, name: &'static str, color: Color) -> CardDefinition {
    CardDefinition {
        id,
        name,
        set_code: SET_CODE,
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::from([color]),
        card_types: types([CardType::Land]),
        is_basic_land: true,
        supported_rules: &["basic-land-deck-construction"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

fn colors(colors: impl IntoIterator<Item = Color>) -> BTreeSet<Color> {
    colors.into_iter().collect()
}

fn types(card_types: impl IntoIterator<Item = CardType>) -> BTreeSet<CardType> {
    card_types.into_iter().collect()
}

#[must_use]
pub fn event_digest(events: &[String]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in events.iter().flat_map(|event| event.bytes().chain(*b"\n")) {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_block_manifests_are_formal_and_consistent() {
        validate_block_manifests().expect("Ravnica block manifests should validate");
        validate_shown_deck_pool().expect("shown RAV deck pool should be legal");
        validate_rav_catalog_executable_boundary()
            .expect("full RAV catalog must preserve the executable boundary");
        assert!(
            load_reference_decks()
                .expect("reference RAV decks should be legal")
                .len()
                >= 2
        );
    }

    #[test]
    fn representative_rav_scenarios_are_deterministic() {
        let first = run_all_scenarios().expect("first scenario execution");
        let second = run_all_scenarios().expect("second scenario execution");
        assert_eq!(first, second);
        assert_eq!(first.len(), 23);
        assert!(first.iter().all(|result| !result.digest.is_empty()));
        verify_reference_event_logs().expect("public RAV logs should match fixed baselines");
    }
}
