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
    ActivatedManaAbility, CardDefinition, CardType, CastRequest, Color, ConvokeContribution,
    ConvokePayment, DeckEntry, DeckList, DeckRules, Effect, Game, HybridManaSymbol, Keyword,
    ManaAbilityBinding, ManaAbilityOutput, ManaBundle, ManaCost, PlayerId, RulesError, Target,
    TokenSpec, Zone,
};

pub const SET_CODE: &str = "RAV";

/// The deliberately small subset of RAV definitions for which every printed
/// functional rule is represented by the engine and covered by public tests.
/// All definitions absent from this list remain bounded compatibility slices.
pub const RAV_FULL_FIDELITY_DEFINITION_IDS: [&str; 16] = [
    "RAV-CHAR",
    "RAV-LIGHTNING-HELIX",
    "RAV-LAST-GASP",
    "RAV-ELVES-OF-DEEP-SHADOW",
    "RAV-BOROS-RECRUIT",
    "RAV-WATCHWOLF",
    "RAV-GLASS-GOLEM",
    "RAV-CLEANSING-BEAM",
    "RAV-RALLY-THE-RIGHTEOUS",
    "RAV-WOJEK-SIREN",
    "RAV-RAIN-OF-EMBERS",
    "RAV-DOGPILE",
    "RAV-OVERWHELM",
    "RAV-GATHER-COURAGE",
    "RAV-SEEDS-OF-STRENGTH",
    "RAV-DARKBLAST",
];

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
            supported_rules: &["full-rules-fidelity", "targeted-damage", "self-damage"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::DealDamage {
                    amount: 4,
                    target: cardbench_magic_engine::TargetRequirement::PlayerOrCreature,
                },
                Effect::DealDamageController { amount: 2 },
            ],
        },
        // Full fidelity: the complete target-free global-damage resolution.
        // Every creature and every player receives the fixed damage in one
        // batch before state-based actions run.
        CardDefinition {
            id: "RAV-RAIN-OF-EMBERS",
            name: "Rain of Embers",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &["full-rules-fidelity", "global-creature-and-player-damage"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::DealDamageToEachCreatureAndPlayer { amount: 1 }],
        },
        // Full card behavior is represented by the shared radiance damage
        // substrate: legal creature target, resolution-time color selection,
        // one damage batch, and the resulting state-based actions.
        CardDefinition {
            id: "RAV-CLEANSING-BEAM",
            name: "Cleansing Beam",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "radiance",
                "targeted-creature-damage",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::RadianceDealDamageToCreatures { amount: 2 }],
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
            supported_rules: &["full-rules-fidelity", "targeted-damage", "life-gain"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::DealDamage {
                    amount: 3,
                    target: cardbench_magic_engine::TargetRequirement::PlayerOrCreature,
                },
                Effect::GainLifeController { amount: 3 },
            ],
        },
        // Compatibility boundary: the token object has a name, color, card
        // type, and power/toughness, but this engine has no creature-subtype
        // substrate. The printed token's subtype is therefore not claimed as
        // represented.
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
            supported_rules: &[
                "full-rules-fidelity",
                "convoke",
                "targeted-layer-7-modifier",
            ],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Convoke],
            effects: vec![Effect::ModifyTargetPtUntilEndOfTurn {
                power: 2,
                toughness: 2,
            }],
        },
        // This is the complete fixed three-modifier resolution: each modifier
        // is recorded separately so the shared continuous-effect layer owns
        // their normal lifetime and target legality.
        CardDefinition {
            id: "RAV-SEEDS-OF-STRENGTH",
            name: "Seeds of Strength",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Green, Color::White]),
            colors: colors([Color::Green, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "three-targeted-layer-7-modifiers",
                "partial-target-resolution",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::ModifyTargetPtUntilEndOfTurn {
                    power: 1,
                    toughness: 1,
                },
                Effect::ModifyTargetPtUntilEndOfTurn {
                    power: 1,
                    toughness: 1,
                },
                Effect::ModifyTargetPtUntilEndOfTurn {
                    power: 1,
                    toughness: 1,
                },
            ],
        },
        // Compatibility scope: the exact two-token creation side effect only.
        // The persistent Aura attachment and its granted combat capability are
        // deliberately absent until the engine represents attachments.
        CardDefinition {
            id: "RAV-FISTS-OF-IRONWOOD",
            name: "Fists of Ironwood",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &["two-saproling-token-creation"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::CreateToken {
                token: TokenSpec::saproling(),
                count: 2,
            }],
        },
        // Compatibility scope: the controller life-gain component only. The
        // selected graveyard-card return is intentionally not approximated.
        CardDefinition {
            id: "RAV-DRYADS-CARESS",
            name: "Dryad's Caress",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Green, Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &["controller-life-gain"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::GainLifeController { amount: 1 }],
        },
        // Compatibility scope: target-creature damage only. Its additional
        // sacrifice cost is deliberately omitted rather than silently paid.
        CardDefinition {
            id: "RAV-FIERY-CONCLUSION",
            name: "Fiery Conclusion",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["targeted-creature-damage"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::DealDamage {
                amount: 5,
                target: cardbench_magic_engine::TargetRequirement::Creature,
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
        // Compatibility scope: normal creature casting, base characteristics,
        // and the engine's existing Dredge replacement. The printed counter-
        // based entry behavior and regeneration activation are intentionally
        // unsupported. Its printed 0/0 base characteristics are retained; an
        // unmodified resolved permanent will therefore be removed by SBAs.
        CardDefinition {
            id: "RAV-GOLGARI-GRAVE-TROLL",
            name: "Golgari Grave-Troll",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "dredge",
                "base-characteristics",
                "zero-toughness-state-based-action",
            ],
            power: Some(0),
            toughness: Some(0),
            keywords: vec![Keyword::Dredge(6)],
            effects: vec![],
        },
        // Compatibility scope: normal creature casting, base characteristics,
        // and the shared Dredge replacement. Its printed upkeep and end-step
        // behavior is intentionally unsupported.
        CardDefinition {
            id: "RAV-NECROPLASM",
            name: "Necroplasm",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Black, Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["dredge", "base-characteristics"],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![Keyword::Dredge(2)],
            effects: vec![],
        },
        // Compatibility scope: the fixed target-creature damage and life gain
        // resolve together. The payment-color-conditioned card draw is
        // deliberately omitted because this slice does not retain the colors
        // spent to cast a spell; this is not full-card fidelity.
        CardDefinition {
            id: "RAV-RIBBONS-OF-NIGHT",
            name: "Ribbons of Night",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &["targeted-creature-damage", "life-gain"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::DealDamage {
                    amount: 4,
                    target: cardbench_magic_engine::TargetRequirement::Creature,
                },
                Effect::GainLifeController { amount: 4 },
            ],
        },
        // Compatibility scope: normal creature casting, base characteristics,
        // and the shared Dredge replacement. Its printed sacrifice activation
        // is intentionally unsupported.
        CardDefinition {
            id: "RAV-GRAVE-SHELL-SCARAB",
            name: "Grave-Shell Scarab",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Black, Color::Green, Color::Green]),
            colors: colors([Color::Black, Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["dredge", "base-characteristics"],
            power: Some(4),
            toughness: Some(4),
            keywords: vec![Keyword::Dredge(1)],
            effects: vec![],
        },
        // Compatibility scope: normal creature casting, base characteristics,
        // and the shared Dredge replacement. Its printed sacrifice activation
        // is intentionally unsupported.
        CardDefinition {
            id: "RAV-SHAMBLING-SHELL",
            name: "Shambling Shell",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Black, Color::Green]),
            colors: colors([Color::Black, Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["dredge", "base-characteristics"],
            power: Some(3),
            toughness: Some(1),
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
        // Only the hand-zone transmute activation is executable. The printed
        // spell effect is deliberately non-covered in this compatibility
        // definition.
        CardDefinition {
            id: "RAV-DIMIR-MACHINATIONS",
            name: "Dimir Machinations",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &["transmute"],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Transmute(ManaCost::with_colors(
                1,
                [Color::Blue, Color::Blue],
            ))],
            effects: vec![],
        },
        // Only the hand-zone transmute activation is executable. Its printed
        // spell effect is deliberately non-covered.
        CardDefinition {
            id: "RAV-SHRED-MEMORY",
            name: "Shred Memory",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
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
        // Only the hand-zone transmute activation is executable. Its printed
        // spell effect is deliberately non-covered.
        CardDefinition {
            id: "RAV-CLUTCH-OF-THE-UNDERCITY",
            name: "Clutch of the Undercity",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Blue, Color::Black]),
            colors: colors([Color::Blue, Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["transmute"],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Transmute(ManaCost::with_colors(
                1,
                [Color::Blue, Color::Blue],
            ))],
            effects: vec![],
        },
        // Only the hand-zone transmute activation is executable. Its printed
        // spell effect is deliberately non-covered.
        CardDefinition {
            id: "RAV-PERPLEX",
            name: "Perplex",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Blue, Color::Black, Color::Black]),
            colors: colors([Color::Blue, Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["transmute"],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Transmute(ManaCost::with_colors(
                1,
                [Color::Blue, Color::Black],
            ))],
            effects: vec![],
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
        // Full fidelity: this card's complete functional behavior is the
        // creature-targeted temporary layer-7 modifier plus its fixed Dredge
        // replacement. Both are represented by the shared engine substrates.
        CardDefinition {
            id: "RAV-DARKBLAST",
            name: "Darkblast",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["full-rules-fidelity", "targeted-layer-7-modifier", "dredge"],
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
            supported_rules: &[
                "full-rules-fidelity",
                "radiance",
                "untap",
                "layer-7-modifier",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::RadianceUntapAndModifyUntilEndOfTurn {
                power: 2,
                toughness: 0,
            }],
        },
        // This is the distinct radiance-only temporary modifier operation;
        // it intentionally does not reuse Rally's untap behavior.
        CardDefinition {
            id: "RAV-WOJEK-SIREN",
            name: "Wojek Siren",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["full-rules-fidelity", "radiance", "layer-7-modifier"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::RadianceModifyPtUntilEndOfTurn {
                power: 1,
                toughness: 1,
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
            supported_rules: &["full-rules-fidelity", "targeted-layer-7-modifier"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::ModifyTargetPtUntilEndOfTurn {
                power: -3,
                toughness: -3,
            }],
        },
        // Compatibility scope: normal creature casting, base characteristics,
        // and the existing Convoke payment hook. Trample combat-damage
        // assignment is intentionally unsupported.
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
        // and the existing Convoke payment hook. Its printed combat keyword is
        // intentionally unsupported.
        CardDefinition {
            id: "RAV-CONCLAVE-EQUENAUT",
            name: "Conclave Equenaut",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::White, Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["convoke", "base-characteristics"],
            power: Some(3),
            toughness: Some(3),
            keywords: vec![Keyword::Convoke],
            effects: vec![],
        },
        // Compatibility scope: normal creature casting, base characteristics,
        // and the existing Convoke payment hook. Its printed enter-the-
        // battlefield behavior is intentionally unsupported.
        CardDefinition {
            id: "RAV-CONCLAVE-PHALANX",
            name: "Conclave Phalanx",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["convoke", "base-characteristics"],
            power: Some(2),
            toughness: Some(4),
            keywords: vec![Keyword::Convoke],
            effects: vec![],
        },
        // Compatibility scope: normal creature casting, base characteristics,
        // and the existing Convoke payment hook. Vigilance's attack/tap rule
        // is intentionally unsupported.
        CardDefinition {
            id: "RAV-GUARDIAN-OF-VITU-GHAZI",
            name: "Guardian of Vitu-Ghazi",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(6, [Color::Green, Color::White]),
            colors: colors([Color::Green, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["convoke", "base-characteristics"],
            power: Some(4),
            toughness: Some(7),
            keywords: vec![Keyword::Convoke],
            effects: vec![],
        },
        // Compatibility scope: normal creature casting, base characteristics,
        // and the existing Convoke payment hook. Its printed combat keyword is
        // intentionally unsupported.
        CardDefinition {
            id: "RAV-AUTOCHTHON-WURM",
            name: "Autochthon Wurm",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(
                10,
                [
                    Color::Green,
                    Color::Green,
                    Color::Green,
                    Color::White,
                    Color::White,
                ],
            ),
            colors: colors([Color::Green, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["convoke", "base-characteristics"],
            power: Some(9),
            toughness: Some(14),
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
        // Full fidelity: Convoke payment and the complete target-free,
        // controller-wide temporary layer-7 modifier.
        CardDefinition {
            id: "RAV-OVERWHELM",
            name: "Overwhelm",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(5, [Color::Green, Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "convoke",
                "controller-creature-layer-7-modifier",
            ],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Convoke],
            effects: vec![Effect::ModifyControllerCreaturesPtUntilEndOfTurn {
                power: 3,
                toughness: 3,
            }],
        },
        // Public RAV verification establishes that this is a vanilla creature:
        // there is no printed functional ability omitted from this definition.
        CardDefinition {
            id: "RAV-WATCHWOLF",
            name: "Watchwolf",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Green, Color::White]),
            colors: colors([Color::Green, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
            ],
            power: Some(3),
            toughness: Some(3),
            keywords: vec![],
            effects: vec![],
        },
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed damage-triggered behavior is
        // deliberately omitted from this compatibility slice.
        CardDefinition {
            id: "RAV-DROMAD-PUREBRED",
            name: "Dromad Purebred",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["colored-cost-casting", "base-characteristics"],
            power: Some(1),
            toughness: Some(5),
            keywords: vec![],
            effects: vec![],
        },
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed flying is deliberately omitted.
        CardDefinition {
            id: "RAV-SNAPPING-DRAKE",
            name: "Snapping Drake",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["colored-cost-casting", "base-characteristics"],
            power: Some(3),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Every printed card-specific behavior is
        // deliberately omitted from this slice.
        CardDefinition {
            id: "RAV-CARRION-HOWLER",
            name: "Carrion Howler",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["colored-cost-casting", "base-characteristics"],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Every printed card-specific behavior is
        // deliberately omitted from this slice.
        CardDefinition {
            id: "RAV-COALHAULER-SWINE",
            name: "Coalhauler Swine",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::Red, Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["colored-cost-casting", "base-characteristics"],
            power: Some(4),
            toughness: Some(4),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity for the original RAV printing: select a player or
        // creature, then count controller-owned creatures still attacking as
        // the spell resolves to determine the damage.
        CardDefinition {
            id: "RAV-DOGPILE",
            name: "Dogpile",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "player-or-creature-targeting",
                "attacking-creature-count-damage",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::DealDamageEqualToAttackingCreatures {
                target: cardbench_magic_engine::TargetRequirement::PlayerOrCreature,
            }],
        },
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Every printed card-specific behavior is
        // deliberately omitted from this slice.
        CardDefinition {
            id: "RAV-BRAMBLE-ELEMENTAL",
            name: "Bramble Elemental",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Green, Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["colored-cost-casting", "base-characteristics"],
            power: Some(4),
            toughness: Some(4),
            keywords: vec![],
            effects: vec![],
        },
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Every printed card-specific behavior is
        // deliberately omitted from this slice.
        CardDefinition {
            id: "RAV-SKYKNIGHT-LEGIONNAIRE",
            name: "Skyknight Legionnaire",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Red, Color::White]),
            colors: colors([Color::Red, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["colored-cost-casting", "base-characteristics"],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Every printed card-specific behavior is
        // deliberately omitted from this slice.
        CardDefinition {
            id: "RAV-MOROII",
            name: "Moroii",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Blue, Color::Black]),
            colors: colors([Color::Blue, Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["colored-cost-casting", "base-characteristics"],
            power: Some(4),
            toughness: Some(4),
            keywords: vec![],
            effects: vec![],
        },
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Every printed card-specific behavior is
        // deliberately omitted from this slice.
        CardDefinition {
            id: "RAV-LOXODON-HIERARCH",
            name: "Loxodon Hierarch",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Green, Color::White]),
            colors: colors([Color::Green, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["colored-cost-casting", "base-characteristics"],
            power: Some(4),
            toughness: Some(4),
            keywords: vec![],
            effects: vec![],
        },
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Every printed card-specific behavior is
        // deliberately omitted from this slice.
        CardDefinition {
            id: "RAV-BOROS-SWIFTBLADE",
            name: "Boros Swiftblade",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Red, Color::White]),
            colors: colors([Color::Red, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["colored-cost-casting", "base-characteristics"],
            power: Some(1),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Every printed card-specific behavior is
        // deliberately omitted from this slice.
        CardDefinition {
            id: "RAV-CARVEN-CARYATID",
            name: "Carven Caryatid",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Green, Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["colored-cost-casting", "base-characteristics"],
            power: Some(2),
            toughness: Some(5),
            keywords: vec![],
            effects: vec![],
        },
        // Public RAV #261 verification establishes that this is a vanilla
        // artifact creature: its published rules field is empty, so this
        // definition does not omit a printed ability.
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
                "full-rules-fidelity",
                "colorless-cost-casting",
                "artifact-creature-base-characteristics",
            ],
            power: Some(6),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        // This bounded compatibility definition covers normal colored-cost
        // casting, base creature characteristics, and the separate
        // definition-bound mana-ability binding below. It intentionally does
        // not represent combat keywords or any other card-specific behavior.
        CardDefinition {
            id: "RAV-BIRDS-OF-PARADISE",
            name: "Birds of Paradise",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "colored-cost-casting",
                "base-characteristics",
                "bound-tap-choice-mana-ability",
            ],
            power: Some(0),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
        // This definition is complete for the public Elves of Deep Shadow
        // card: ordinary creature characteristics plus its one source-aware,
        // non-stack tap mana ability are both represented by the binding below.
        CardDefinition {
            id: "RAV-ELVES-OF-DEEP-SHADOW",
            name: "Elves of Deep Shadow",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "bound-tap-black-mana-ability",
                "source-aware-controller-damage",
            ],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed activated behavior is deliberately
        // omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-ELVISH-SKYSWEEPER",
            "Elvish Skysweeper",
            ManaCost::with_colors(0, [Color::Green]),
            colors([Color::Green]),
            1,
            1,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed combat-triggered behavior is
        // deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-FRENZIED-GOBLIN",
            "Frenzied Goblin",
            ManaCost::with_colors(0, [Color::Red]),
            colors([Color::Red]),
            1,
            1,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed evasion keyword is deliberately
        // omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-GRAYSCALED-GHARIAL",
            "Grayscaled Gharial",
            ManaCost::with_colors(0, [Color::Blue]),
            colors([Color::Blue]),
            1,
            1,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed activated power/toughness change
        // is deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-GREATER-FORGELING",
            "Greater Forgeling",
            ManaCost::with_colors(3, [Color::Red, Color::Red]),
            colors([Color::Red]),
            3,
            4,
        ),
        bounded_creature_chassis(
            "RAV-GOLIATH-SPIDER",
            "Goliath Spider",
            ManaCost::with_colors(6, [Color::Green, Color::Green]),
            colors([Color::Green]),
            7,
            6,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed activated behavior is deliberately
        // omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-IVY-DANCER",
            "Ivy Dancer",
            ManaCost::with_colors(2, [Color::Green]),
            colors([Color::Green]),
            1,
            2,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed activated behavior is deliberately
        // omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-LORE-BROKER",
            "Lore Broker",
            ManaCost::with_colors(1, [Color::Blue]),
            colors([Color::Blue]),
            1,
            2,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed activated combat behavior is
        // deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-MORTIPEDE",
            "Mortipede",
            ManaCost::with_colors(3, [Color::Black]),
            colors([Color::Black]),
            4,
            1,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed token-making activation is
        // deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-SELESNYA-EVANGEL",
            "Selesnya Evangel",
            ManaCost::with_colors(0, [Color::Green, Color::White]),
            colors([Color::Green, Color::White]),
            1,
            2,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed combat capabilities are deliberately
        // omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-SELESNYA-SAGITTARS",
            "Selesnya Sagittars",
            ManaCost::with_colors(3, [Color::Green, Color::White]),
            colors([Color::Green, Color::White]),
            2,
            5,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed evasion and activated combat
        // behavior are deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-SCREECHING-GRIFFIN",
            "Screeching Griffin",
            ManaCost::with_colors(3, [Color::White]),
            colors([Color::White]),
            2,
            2,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed flying and damage-triggered
        // behavior are deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-BELLTOWER-SPHINX",
            "Belltower Sphinx",
            ManaCost::with_colors(4, [Color::Blue]),
            colors([Color::Blue]),
            2,
            5,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed flying and activated library
        // behavior are deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-CERULEAN-SPHINX",
            "Cerulean Sphinx",
            ManaCost::with_colors(4, [Color::Blue, Color::Blue]),
            colors([Color::Blue]),
            5,
            5,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed evasion and opponent-token entry
        // behavior are deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-HUNTED-PHANTASM",
            "Hunted Phantasm",
            ManaCost::with_colors(1, [Color::Blue, Color::Blue]),
            colors([Color::Blue]),
            4,
            6,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed flying and regeneration activation
        // are deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-TATTERED-DRAKE",
            "Tattered Drake",
            ManaCost::with_colors(4, [Color::Blue]),
            colors([Color::Blue]),
            2,
            2,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed enter-the-battlefield library
        // movement is deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-VEDALKEN-DISMISSER",
            "Vedalken Dismisser",
            ManaCost::with_colors(5, [Color::Blue]),
            colors([Color::Blue]),
            2,
            2,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed blocking trigger is deliberately
        // omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-ZEPHYR-SPIRIT",
            "Zephyr Spirit",
            ManaCost::with_colors(5, [Color::Blue]),
            colors([Color::Blue]),
            0,
            6,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed death trigger is deliberately
        // omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-SADISTIC-AUGERMAGE",
            "Sadistic Augermage",
            ManaCost::with_colors(2, [Color::Black]),
            colors([Color::Black]),
            3,
            1,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed prevention behavior is deliberately
        // omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-INDENTURED-OAF",
            "Indentured Oaf",
            ManaCost::with_colors(3, [Color::Red]),
            colors([Color::Red]),
            4,
            3,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed defender and land-sacrifice
        // behavior are deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-TORPID-MOLOCH",
            "Torpid Moloch",
            ManaCost::with_colors(0, [Color::Red]),
            colors([Color::Red]),
            3,
            2,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed defender and prevention activation
        // are deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-BENEVOLENT-ANCESTOR",
            "Benevolent Ancestor",
            ManaCost::with_colors(2, [Color::White]),
            colors([Color::White]),
            0,
            4,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed evasion and death-triggered
        // behavior are deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-SURVEILLING-SPRITE",
            "Surveilling Sprite",
            ManaCost::with_colors(1, [Color::Blue]),
            colors([Color::Blue]),
            1,
            1,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed land-type activation is deliberately
        // omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-TERRAFORMER",
            "Terraformer",
            ManaCost::with_colors(2, [Color::Blue]),
            colors([Color::Blue]),
            2,
            2,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed temporary evasion activation is
        // deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-ROOFSTALKER-WIGHT",
            "Roofstalker Wight",
            ManaCost::with_colors(1, [Color::Blue]),
            colors([Color::Blue]),
            2,
            1,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed evasion and regeneration behavior
        // are deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-SEWERDREG",
            "Sewerdreg",
            ManaCost::with_colors(3, [Color::Black, Color::Black]),
            colors([Color::Black]),
            3,
            3,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed landwalk ability is deliberately
        // omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-GOBLIN-SPELUNKERS",
            "Goblin Spelunkers",
            ManaCost::with_colors(2, [Color::Red]),
            colors([Color::Red]),
            2,
            2,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed damage-prevention activation is
        // deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-ORDRUUN-COMMANDO",
            "Ordruun Commando",
            ManaCost::with_colors(3, [Color::Red]),
            colors([Color::Red]),
            4,
            1,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed power/toughness activation is
        // deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-VIASHINO-SLASHER",
            "Viashino Slasher",
            ManaCost::with_colors(1, [Color::Red]),
            colors([Color::Red]),
            1,
            2,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed land-search triggered behavior is
        // deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-CIVIC-WAYFINDER",
            "Civic Wayfinder",
            ManaCost::with_colors(2, [Color::Green]),
            colors([Color::Green]),
            2,
            2,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed graveyard-recursion activation is
        // deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-DOWSING-SHAMAN",
            "Dowsing Shaman",
            ManaCost::with_colors(2, [Color::Green, Color::Green]),
            colors([Color::Green]),
            3,
            4,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed flying and upkeep sacrifice
        // triggers are deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-WOEBRINGER-DEMON",
            "Woebringer Demon",
            ManaCost::with_colors(3, [Color::Black, Color::Black]),
            colors([Color::Black]),
            4,
            4,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed sacrifice-and-library activation
        // is deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-THOUGHTPICKER-WITCH",
            "Thoughtpicker Witch",
            ManaCost::with_colors(0, [Color::Black]),
            colors([Color::Black]),
            1,
            1,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed evasion and temporary P/T
        // activation are deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-UNDERCITY-SHADE",
            "Undercity Shade",
            ManaCost::with_colors(4, [Color::Black]),
            colors([Color::Black]),
            1,
            1,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed entry sacrifice and Saproling
        // blocking restriction are deliberately omitted from this slice.
        bounded_creature_chassis(
            "RAV-VINDICTIVE-MOB",
            "Vindictive Mob",
            ManaCost::with_colors(4, [Color::Black, Color::Black]),
            colors([Color::Black]),
            5,
            5,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed sacrifice-to-destroy-land
        // activation is deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-BARBARIAN-RIFTCUTTER",
            "Barbarian Riftcutter",
            ManaCost::with_colors(4, [Color::Red]),
            colors([Color::Red]),
            3,
            3,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed damage-prevention exception is
        // deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-EXCRUCIATOR",
            "Excruciator",
            ManaCost::with_colors(6, [Color::Red, Color::Red]),
            colors([Color::Red]),
            7,
            7,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed haste, attack restriction, and
        // temporary power activation are deliberately omitted from this slice.
        bounded_creature_chassis(
            "RAV-GOBLIN-FIRE-FIEND",
            "Goblin Fire Fiend",
            ManaCost::with_colors(3, [Color::Red]),
            colors([Color::Red]),
            1,
            1,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed death damage trigger is
        // deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-SELL-SWORD-BRUTE",
            "Sell-Sword Brute",
            ManaCost::with_colors(1, [Color::Red]),
            colors([Color::Red]),
            2,
            2,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed creature-cast trigger is deliberately
        // omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-PRIMORDIAL-SAGE",
            "Primordial Sage",
            ManaCost::with_colors(4, [Color::Green, Color::Green]),
            colors([Color::Green]),
            4,
            5,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed activated behavior is deliberately
        // omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-TRANSLUMINANT",
            "Transluminant",
            ManaCost::with_colors(1, [Color::Green]),
            colors([Color::Green]),
            2,
            2,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed flying-creature interaction is
        // deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-TROPHY-HUNTER",
            "Trophy Hunter",
            ManaCost::with_colors(2, [Color::Green]),
            colors([Color::Green]),
            2,
            3,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed activated stat modification is
        // deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-URSAPINE",
            "Ursapine",
            ManaCost::with_colors(3, [Color::Green, Color::Green]),
            colors([Color::Green]),
            3,
            3,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed land-entry trigger is deliberately
        // omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-VINELASHER-KUDZU",
            "Vinelasher Kudzu",
            ManaCost::with_colors(1, [Color::Green]),
            colors([Color::Green]),
            1,
            1,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed activated stat modification is
        // deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-DROOLING-GROODION",
            "Drooling Groodion",
            ManaCost::with_colors(3, [Color::Black, Color::Black, Color::Green]),
            colors([Color::Black, Color::Green]),
            4,
            3,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed enter-the-battlefield team modifier
        // is deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-FLAME-KIN-ZEALOT",
            "Flame-Kin Zealot",
            ManaCost::with_colors(1, [Color::Red, Color::Red, Color::White]),
            colors([Color::Red, Color::White]),
            2,
            2,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed sacrifice activation is deliberately
        // omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-GOLGARI-ROTWURM",
            "Golgari Rotwurm",
            ManaCost::with_colors(3, [Color::Black, Color::Green]),
            colors([Color::Black, Color::Green]),
            5,
            4,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed activated combat behavior is
        // deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-SUNHOME-ENFORCER",
            "Sunhome Enforcer",
            ManaCost::with_colors(2, [Color::Red, Color::White]),
            colors([Color::Red, Color::White]),
            2,
            4,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed activated combat behavior is
        // deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-THUNDERSONG-TRUMPETER",
            "Thundersong Trumpeter",
            ManaCost::with_colors(0, [Color::Red, Color::White]),
            colors([Color::Red, Color::White]),
            2,
            1,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed evasion and library-movement trigger
        // are deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-VULTUROUS-ZOMBIE",
            "Vulturous Zombie",
            ManaCost::with_colors(3, [Color::Black, Color::Green]),
            colors([Color::Black, Color::Green]),
            3,
            3,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed graveyard-exile activation is
        // deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-WOODWRAITH-CORRUPTER",
            "Woodwraith Corrupter",
            ManaCost::with_colors(3, [Color::Black, Color::Black, Color::Green]),
            colors([Color::Black, Color::Green]),
            3,
            6,
        ),
        bounded_creature_chassis(
            "RAV-COURIER-HAWK",
            "Courier Hawk",
            ManaCost::with_colors(1, [Color::White]),
            colors([Color::White]),
            1,
            2,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed flying and activated combat
        // behavior are deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-DIVEBOMBER-GRIFFIN",
            "Divebomber Griffin",
            ManaCost::with_colors(3, [Color::White, Color::White]),
            colors([Color::White]),
            3,
            2,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed activated behavior is deliberately
        // omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-SANDSOWER",
            "Sandsower",
            ManaCost::with_colors(3, [Color::White]),
            colors([Color::White]),
            1,
            3,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed combat keyword is deliberately
        // omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-VOTARY-OF-THE-CONCLAVE",
            "Votary of the Conclave",
            ManaCost::with_colors(0, [Color::White]),
            colors([Color::White]),
            1,
            1,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed entry-triggered behavior is
        // deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-DRAKE-FAMILIAR",
            "Drake Familiar",
            ManaCost::with_colors(1, [Color::Blue]),
            colors([Color::Blue]),
            2,
            1,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed defender and hand-zone transmute
        // behavior are deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-DRIFT-OF-PHANTASMS",
            "Drift of Phantasms",
            ManaCost::with_colors(2, [Color::Blue]),
            colors([Color::Blue]),
            0,
            5,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed evasion and hand-zone transmute
        // behavior are deliberately omitted from this compatibility slice.
        bounded_creature_chassis(
            "RAV-ETHEREAL-USHER",
            "Ethereal Usher",
            ManaCost::with_colors(5, [Color::Blue]),
            colors([Color::Blue]),
            2,
            3,
        ),
        // Compatibility scope: normal colored-cost creature casting and base
        // characteristics only. Its printed defender, entry-triggered, and
        // hand-zone transmute behavior are deliberately omitted from this
        // compatibility slice.
        bounded_creature_chassis(
            "RAV-GROZOTH",
            "Grozoth",
            ManaCost::with_colors(6, [Color::Blue, Color::Blue, Color::Blue]),
            colors([Color::Blue]),
            9,
            9,
        ),
        // Complete supported slice: either color pays the one hybrid symbol,
        // and first-strike creatures assign combat damage in the dedicated
        // earlier damage step. There are no additional printed abilities.
        CardDefinition {
            id: "RAV-BOROS-RECRUIT",
            name: "Boros Recruit",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_hybrid(
                0,
                [],
                [HybridManaSymbol {
                    first: Color::Red,
                    second: Color::White,
                }],
            ),
            colors: colors([Color::Red, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["full-rules-fidelity", "hybrid-cost-casting", "first-strike"],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![Keyword::FirstStrike],
            effects: vec![],
        },
        signet_definition("RAV-BOROS-SIGNET", "Boros Signet"),
        signet_definition("RAV-DIMIR-SIGNET", "Dimir Signet"),
        signet_definition("RAV-GOLGARI-SIGNET", "Golgari Signet"),
        signet_definition("RAV-SELESNYA-SIGNET", "Selesnya Signet"),
        basic_land("RAV-PLAINS", "Plains", Color::White),
        basic_land("RAV-ISLAND", "Island", Color::Blue),
        basic_land("RAV-SWAMP", "Swamp", Color::Black),
        basic_land("RAV-MOUNTAIN", "Mountain", Color::Red),
        basic_land("RAV-FOREST", "Forest", Color::Green),
    ]
}

/// Definition-bound RAV mana abilities used by the shown mana-development scenarios.
///
/// Each binding is intentionally a narrow compatibility slice: pay one mana,
/// tap the artifact, and add its two fixed guild colors. The shared engine owns
/// cost payment, atomically emitted receipts, priority retention, and the fact
/// that a mana ability does not use the stack.
#[must_use]
pub fn rav_mana_ability_bindings() -> Vec<ManaAbilityBinding> {
    vec![
        ManaAbilityBinding {
            card_definition: "RAV-BIRDS-OF-PARADISE",
            ability: ActivatedManaAbility {
                id: "produce-one-color",
                tap_cost: true,
                output: ManaAbilityOutput::Choice(colors(Color::ALL)),
                amount: 1,
                life_payment: None,
                controller_damage: None,
            },
        },
        ManaAbilityBinding {
            card_definition: "RAV-ELVES-OF-DEEP-SHADOW",
            ability: ActivatedManaAbility {
                id: "produce-black-and-damage-controller",
                tap_cost: true,
                output: ManaAbilityOutput::Fixed(Color::Black),
                amount: 1,
                life_payment: None,
                controller_damage: Some(1),
            },
        },
        signet_binding(
            "RAV-BOROS-SIGNET",
            "boros-signet-wr",
            [Color::White, Color::Red],
        ),
        signet_binding(
            "RAV-DIMIR-SIGNET",
            "dimir-signet-ub",
            [Color::Blue, Color::Black],
        ),
        signet_binding(
            "RAV-GOLGARI-SIGNET",
            "golgari-signet-bg",
            [Color::Black, Color::Green],
        ),
        signet_binding(
            "RAV-SELESNYA-SIGNET",
            "selesnya-signet-gw",
            [Color::White, Color::Green],
        ),
    ]
}

fn signet_binding(
    card_definition: &'static str,
    id: &'static str,
    colors: [Color; 2],
) -> ManaAbilityBinding {
    ManaAbilityBinding {
        card_definition,
        ability: ActivatedManaAbility {
            id,
            tap_cost: true,
            output: ManaAbilityOutput::PaidBundle {
                mana_cost: ManaCost::new(1),
                bundle: ManaBundle::new(colors.into_iter().map(|color| (color, 1))),
            },
            amount: 0,
            life_payment: None,
            controller_damage: None,
        },
    }
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
    game.transmute(PlayerId(0), muddle, Some(helix))?;
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

/// A deliberately bounded RAV creature slice. The caller supplies only public
/// identity, mana-cost, color, and base-characteristic facts; card-specific
/// printed abilities must remain outside this generic chassis.
fn bounded_creature_chassis(
    id: &'static str,
    name: &'static str,
    mana_cost: ManaCost,
    colors: BTreeSet<Color>,
    power: i16,
    toughness: i16,
) -> CardDefinition {
    CardDefinition {
        id,
        name,
        set_code: SET_CODE,
        mana_cost,
        colors,
        mana_colors: BTreeSet::new(),
        card_types: types([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["colored-cost-casting", "base-characteristics"],
        power: Some(power),
        toughness: Some(toughness),
        keywords: vec![],
        effects: vec![],
    }
}

/// CardBench-authored compatibility definition for the four RAV Signets.
/// It intentionally records only public identity, artifact type, colorless
/// casting cost, and the shared paid two-color mana-ability hook; it contains
/// no copied card rules text, art, flavor text, or full-card fidelity claim.
fn signet_definition(id: &'static str, name: &'static str) -> CardDefinition {
    CardDefinition {
        id,
        name,
        set_code: SET_CODE,
        mana_cost: ManaCost::new(2),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: types([CardType::Artifact]),
        is_basic_land: false,
        supported_rules: &["artifact-casting", "paid-fixed-two-color-mana-ability"],
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
        assert_eq!(first.len(), 82);
        assert!(first.iter().all(|result| !result.digest.is_empty()));
        verify_reference_event_logs().expect("public RAV logs should match fixed baselines");
    }
}
