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
    ActivatedAbility, ActivatedAbilityBinding, ActivatedAbilityCostModifier,
    ActivatedAbilityCostModifierBinding, ActivatedManaAbility, AdditionalSpellCost,
    AdditionalSpellCostBinding, AttachmentBinding, AttachmentKind, BasicLandType,
    BasicLandTypeBinding, CardDefinition, CardType, CastRequest, Color, ContinuousChange,
    ConvokeContribution, ConvokePayment, CostReductionBinding, DamageReplacementEffect,
    DamageReplacementEffectBinding, DeckEntry, DeckList, DeckRules, Effect, Game, HybridManaSymbol,
    Keyword, LandEntryBinding, LibrarySearchDestination, LibrarySearchRequirement,
    LibrarySearchSelection, ManaAbilityBinding, ManaAbilityOutput, ManaBundle, ManaCost, PlayerId,
    ReplacementEffect, ReplacementEffectBinding, RulesError, StaticAttackRestriction,
    StaticAttackRestrictionBinding, StaticContinuousEffectBinding, StaticEntryRestriction,
    StaticEntryRestrictionBinding, Target, TargetRequirement, TokenSpec, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding, Zone,
};

pub const SET_CODE: &str = "RAV";

/// The deliberately small subset of RAV definitions for which every printed
/// functional rule is represented by the engine and covered by public tests.
/// All definitions absent from this list remain bounded compatibility slices.
pub const RAV_FULL_FIDELITY_DEFINITION_IDS: [&str; 165] = [
    "RAV-CHAR",
    "RAV-LIGHTNING-HELIX",
    "RAV-SEARING-MEDITATION",
    "RAV-PUTREFY",
    "RAV-DROOLING-GROODION",
    "RAV-GOLGARI-ROTWURM",
    "RAV-SCATTER-THE-SEEDS",
    "RAV-DOUBLING-SEASON",
    "RAV-CHORD-OF-CALLING",
    "RAV-SCION-OF-THE-WILD",
    "RAV-GUARDIAN-OF-VITU-GHAZI",
    "RAV-LAST-GASP",
    "RAV-ELVES-OF-DEEP-SHADOW",
    "RAV-BOROS-RECRUIT",
    "RAV-NIGHTGUARD-PATROL",
    "RAV-WATCHWOLF",
    "RAV-GLASS-GOLEM",
    "RAV-CLEANSING-BEAM",
    "RAV-RALLY-THE-RIGHTEOUS",
    "RAV-WOJEK-SIREN",
    "RAV-DEVOURING-LIGHT",
    "RAV-RAIN-OF-EMBERS",
    "RAV-DOGPILE",
    "RAV-OVERWHELM",
    "RAV-GATHER-COURAGE",
    "RAV-SEEDS-OF-STRENGTH",
    "RAV-DARKBLAST",
    "RAV-NIGHTMARE-VOID",
    "RAV-MOONLIGHT-BARGAIN",
    "RAV-ROLLING-SPOIL",
    "RAV-NETHERBORN-PHALANX",
    "RAV-HEX",
    "RAV-DARK-CONFIDANT",
    "RAV-SHADOW-OF-DOUBT",
    "RAV-HELLDOZER",
    "RAV-GREATER-MOSSDOG",
    "RAV-BOROS-SIGNET",
    "RAV-DIMIR-SIGNET",
    "RAV-GOLGARI-SIGNET",
    "RAV-SELESNYA-SIGNET",
    "RAV-PLAINS",
    "RAV-ISLAND",
    "RAV-SWAMP",
    "RAV-MOUNTAIN",
    "RAV-FOREST",
    "RAV-CONCLAVE-EQUENAUT",
    "RAV-SNAPPING-DRAKE",
    "RAV-TATTERED-DRAKE",
    "RAV-TERRAFORMER",
    "RAV-CERULEAN-SPHINX",
    "RAV-HUNTED-PHANTASM",
    "RAV-GOLIATH-SPIDER",
    "RAV-COURIER-HAWK",
    "RAV-SKYKNIGHT-LEGIONNAIRE",
    "RAV-MOROII",
    "RAV-SELESNYA-EVANGEL",
    "RAV-SELESNYA-GUILDMAGE",
    "RAV-GOLGARI-GUILDMAGE",
    "RAV-SANDSOWER",
    "RAV-DIVEBOMBER-GRIFFIN",
    "RAV-DROMAD-PUREBRED",
    "RAV-CARVEN-CARYATID",
    "RAV-BIRDS-OF-PARADISE",
    "RAV-FIERY-CONCLUSION",
    "RAV-RIBBONS-OF-NIGHT",
    "RAV-GOBLIN-FIRE-FIEND",
    "RAV-BOROS-SWIFTBLADE",
    "RAV-GOBLIN-SPELUNKERS",
    "RAV-GREATER-FORGELING",
    "RAV-VIASHINO-SLASHER",
    "RAV-WAR-TORCH-GOBLIN",
    "RAV-BARBARIAN-RIFTCUTTER",
    "RAV-TORPID-MOLOCH",
    "RAV-VIASHINO-FANGTAIL",
    "RAV-BOROS-GUILDMAGE",
    "RAV-WOJEK-EMBERMAGE",
    "RAV-WOJEK-APOTHECARY",
    "RAV-THUNDERSONG-TRUMPETER",
    "RAV-SABERTOOTH-ALLEY-CAT",
    "RAV-FLAME-KIN-ZEALOT",
    "RAV-SUNHOME-ENFORCER",
    "RAV-ORDRUUN-COMMANDO",
    "RAV-INDENTURED-OAF",
    "RAV-EXCRUCIATOR",
    "RAV-COALHAULER-SWINE",
    "RAV-SELL-SWORD-BRUTE",
    "RAV-FRENZIED-GOBLIN",
    "RAV-SPARKMAGE-APPRENTICE",
    "RAV-HUNTED-DRAGON",
    "RAV-HUNTED-TROLL",
    "RAV-KEENING-BANSHEE",
    "RAV-RAZIA-BOROS-ARCHANGEL",
    "RAV-HAMMERFIST-GIANT",
    "RAV-INCITE-HYSTERIA",
    "RAV-SCREECHING-GRIFFIN",
    "RAV-SURGE-OF-ZEAL",
    "RAV-SEISMIC-SPIKE",
    "RAV-SMASH",
    "RAV-SUNDERING-VITAE",
    "RAV-RECOLLECT",
    "RAV-MNEMONIC-NEXUS",
    "RAV-PEEL-FROM-REALITY",
    "RAV-QUICKCHANGE",
    "RAV-SINS-OF-THE-PAST",
    "RAV-SEWERDREG",
    "RAV-VOTARY-OF-THE-CONCLAVE",
    "RAV-GRAVE-SHELL-SCARAB",
    "RAV-UNDERCITY-SHADE",
    "RAV-THOUGHTPICKER-WITCH",
    "RAV-SADISTIC-AUGERMAGE",
    "RAV-VINDICTIVE-MOB",
    "RAV-BELLTOWER-SPHINX",
    "RAV-FLIGHT-OF-FANCY",
    "RAV-FLOW-OF-IDEAS",
    "RAV-SURVEILLING-SPRITE",
    "RAV-DREAM-LEASH",
    "RAV-REMAND",
    "RAV-INDUCE-PARANOIA",
    "RAV-TELLING-TIME",
    "RAV-MARK-OF-EVICTION",
    "RAV-VEDALKEN-ENTRANCER",
    "RAV-VEDALKEN-DISMISSER",
    "RAV-TIDEWATER-MINION",
    "RAV-SUNHOME-FORTRESS",
    "RAV-VITU-GHAZI",
    "RAV-NULLMAGE-SHEPHERD",
    "RAV-VIGOR-MORTIS",
    "RAV-STONE-SEEDER-HIEROPHANT",
    "RAV-MOLDERVINE-CLOAK",
    "RAV-FISTS-OF-IRONWOOD",
    "RAV-CLINGING-DARKNESS",
    "RAV-URSAPINE",
    "RAV-TRANSLUMINANT",
    "RAV-TWILIGHT-DROVER",
    "RAV-ELVISH-SKYSWEEPER",
    "RAV-CONVOLUTE",
    "RAV-SHAMBLING-SHELL",
    "RAV-DOWSING-SHAMAN",
    "RAV-IVY-DANCER",
    "RAV-GRAYSCALED-GHARIAL",
    "RAV-SEED-SPARK",
    "RAV-LEAVE-NO-TRACE",
    "RAV-HUNTED-LAMMASU",
    "RAV-HUNTED-HORROR",
    "RAV-HOUR-OF-RECKONING",
    "RAV-OATHSWORN-GIANT",
    "RAV-VETERAN-ARMORER",
    "RAV-GATE-HOUND",
    "RAV-BLAZING-ARCHON",
    "RAV-CAREGIVER",
    "RAV-BOROS-FURY-SHIELD",
    "RAV-BATHE-IN-LIGHT",
    "RAV-LIGHT-OF-SANCTION",
    "RAV-FAITHS-FETTERS",
    "RAV-CHANT-OF-VITU-GHAZI",
    "RAV-CENTAUR-SAFEGUARD",
    "RAV-CYCLOPEAN-SNARE",
    "RAV-GRIFTERS-BLADE",
    "RAV-FESTIVAL-OF-THE-GUILDPACT",
    "RAV-FLICKERFORM",
    "RAV-SUPPRESSION-FIELD",
    "RAV-LOXODON-GATEKEEPER",
    "RAV-THREE-DREAMS",
    "RAV-CONCLAVES-BLESSING",
    "RAV-ZEPHYR-SPIRIT",
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
        // Bounded fidelity: the live source reduces only generic cost on
        // noncreature spells, then its retained spell target is
        // sacrificed-for or countered on the stack. The controller's creature
        // selection remains a policy choice gap, so fixture execution uses
        // stable battlefield order and this definition stays outside the
        // positive full-fidelity manifest.
        CardDefinition {
            id: "RAV-BLOOD-FUNNEL",
            name: "Blood Funnel",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &[
                "noncreature-generic-cost-reduction",
                "cast-sacrifice-or-counter-trigger",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
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
        // Full fidelity: Convoke payment and the created token's count, color,
        // type, typed Saproling subtype, and base power/toughness are all
        // represented by shared engine substrates.
        CardDefinition {
            id: "RAV-SCATTER-THE-SEEDS",
            name: "Scatter the Seeds",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Green, Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["full-rules-fidelity", "convoke", "token-creation"],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Convoke],
            effects: vec![Effect::CreateToken {
                token: TokenSpec::saproling(),
                count: 3,
            }],
        },
        // Full fidelity: this permanent delegates its live, controller-scoped
        // quantity replacement behavior to the expansion-neutral replacement
        // registry. The same binding covers every currently represented token
        // creation and persistent counter-placement event.
        CardDefinition {
            id: "RAV-DOUBLING-SEASON",
            name: "Doubling Season",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "controlled-token-creation-replacement",
                "controlled-plus-one-counter-replacement",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: this static characteristic-defining effect is
        // evaluated from the current controller's battlefield rather than
        // from a stale enter-the-battlefield snapshot.
        CardDefinition {
            id: "RAV-SCION-OF-THE-WILD",
            name: "Scion of the Wild",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "dynamic-controlled-creature-count",
            ],
            power: Some(0),
            toughness: Some(0),
            keywords: vec![],
            effects: vec![],
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
        // Full fidelity: this creature Aura grants Trample through the
        // attachment lifecycle, then creates its Saprolings through its own
        // ordinary enter-the-battlefield stack trigger.
        CardDefinition {
            id: "RAV-FISTS-OF-IRONWOOD",
            name: "Fists of Ironwood",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "aura-enchant-creature-trample-etb-two-saprolings",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::AttachSourceToTarget {
                target: TargetRequirement::Creature,
                changes: vec![ContinuousChange::AddKeyword(Keyword::Trample)],
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
        // Complete public slice: the expansion binds its required controlled
        // creature sacrifice as an explicit cast cost; the shared engine then
        // moves that creature before placing this targeted damage spell on the
        // stack.
        CardDefinition {
            id: "RAV-FIERY-CONCLUSION",
            name: "Fiery Conclusion",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "additional-sacrifice-controlled-creature-cost",
                "targeted-creature-damage",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::DealDamage {
                amount: 5,
                target: cardbench_magic_engine::TargetRequirement::Creature,
            }],
        },
        // Compatibility scope: normal colored-cost casting, base
        // Full fidelity: colored casting, Trample, and the targeted-opponent
        // ETB token creation (including protection-bearing Centaurs) are typed
        // through the expansion-neutral trigger and token substrates.
        CardDefinition {
            id: "RAV-HUNTED-HORROR",
            name: "Hunted Horror",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Black, Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "trample",
                "enter-battlefield-targeted-opponent-token-creation",
                "centaur-protection-from-black",
            ],
            power: Some(7),
            toughness: Some(7),
            keywords: vec![Keyword::Trample],
            effects: vec![],
        },
        // Full fidelity: the black tap activation destroys one target land
        // and then conditionally untaps Helldozer based on the target's
        // nonbasic identity observed at resolution.
        CardDefinition {
            id: "RAV-HELLDOZER",
            name: "Helldozer",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Black, Color::Black, Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "tap-triple-black-destroy-land",
                "nonbasic-land-conditional-untap",
            ],
            power: Some(6),
            toughness: Some(5),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: the opponent-first ETB trigger creates four typed
        // blue 1/1 Flying Faeries, and the shared source-regeneration ability
        // represents the printed green activation through a one-shot shield.
        CardDefinition {
            id: "RAV-HUNTED-TROLL",
            name: "Hunted Troll",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Green, Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "etb-targeted-opponent-flying-faerie-tokens",
                "regeneration",
            ],
            power: Some(8),
            toughness: Some(4),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: Flying is a static characteristic, then the normal
        // targeted-opponent ETB trigger creates exactly one typed black 4/4
        // Horror through its own stack and priority window.
        CardDefinition {
            id: "RAV-HUNTED-LAMMASU",
            name: "Hunted Lammasu",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::White, Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "flying",
                "etb-targeted-opponent-horror-token",
            ],
            power: Some(5),
            toughness: Some(5),
            keywords: vec![Keyword::Flying],
            effects: vec![],
        },
        // Full fidelity: target a controller-owned instant or sorcery in the
        // graveyard, then cast it this turn without paying its mana cost. A
        // permission cast is exiled whether it resolves or is countered.
        CardDefinition {
            id: "RAV-SINS-OF-THE-PAST",
            name: "Sins of the Past",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::Black, Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "target-graveyard-instant-or-sorcery",
                "current-turn-mana-free-cast",
                "permission-cast-exiles",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::GrantGraveyardCastPermissionUntilEndOfTurn],
        },
        // Full fidelity: normal colored-cost casting, base characteristics,
        // Flying, and the targeted ETB -2/-2 modifier all use the normal
        // stack, target legality, and continuous-effect substrate.
        CardDefinition {
            id: "RAV-KEENING-BANSHEE",
            name: "Keening Banshee",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Black, Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "flying",
                "enter-battlefield-targeted-minus-two-minus-two",
            ],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![Keyword::Flying],
            effects: vec![],
        },
        // Compatibility scope: normal colored-cost casting, base
        // characteristics, and its stack-backed dies trigger are represented.
        // The printed target player is selected deterministically
        // opponent-first until triggered-choice actions are policy-submitted.
        CardDefinition {
            id: "RAV-INFECTIOUS-HOST",
            name: "Infectious Host",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "colored-cost-casting",
                "base-characteristics",
                "dies-target-player-life-loss",
                "deterministic-opponent-target-choice",
            ],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: the typed artifact target is destroyed during
        // resolution and the spell controller draws one card afterward.
        // The expansion-neutral engine owns target legality, zone movement,
        // and the draw receipt; this definition contains only semantic data.
        CardDefinition {
            id: "RAV-SMASH",
            name: "Smash",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["full-rules-fidelity", "artifact-destruction", "draw"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::DestroyTargetArtifact, Effect::DrawController],
        },
        // Full fidelity: target legality is artifact-or-creature at cast and
        // resolution, while this destruction instruction deliberately
        // bypasses any live regeneration replacement shield.
        CardDefinition {
            id: "RAV-PUTREFY",
            name: "Putrefy",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Black, Color::Green]),
            colors: colors([Color::Black, Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "artifact-or-creature-destruction-no-regeneration",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::DestroyTargetArtifactOrCreatureNoRegeneration],
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
        // Dredge, and static Flying. Its damage-triggered destruction behavior
        // is deliberately unsupported.
        CardDefinition {
            id: "RAV-STINKWEED-IMP",
            name: "Stinkweed Imp",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["dredge", "base-characteristics", "flying"],
            power: Some(1),
            toughness: Some(2),
            keywords: vec![Keyword::Flying, Keyword::Dredge(5)],
            effects: vec![],
        },
        // Public RAV #169 audit: its sole functional rule is the shared Dredge
        // replacement, alongside normal creature casting and characteristics.
        CardDefinition {
            id: "RAV-GREATER-MOSSDOG",
            name: "Greater Mossdog",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["full-rules-fidelity", "dredge", "base-characteristics"],
            power: Some(3),
            toughness: Some(3),
            keywords: vec![Keyword::Dredge(3)],
            effects: vec![],
        },
        // Compatibility scope: ordinary sorcery casting, Dredge 3, and a
        // deterministic public-zone return of up to three land cards owned by
        // the caster. The printed card calls for a choice, so it remains out
        // of the full-fidelity manifest until that choice is policy-submitted.
        CardDefinition {
            id: "RAV-LIFE-FROM-THE-LOAM",
            name: "Life from the Loam",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Green, Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "dredge",
                "return-up-to-three-land-cards-from-graveyard",
                "deterministic-public-zone-selection",
            ],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Dredge(3)],
            effects: vec![Effect::ReturnUpToThreeControllerGraveyardLandCardsToHand],
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
        CardDefinition {
            id: "RAV-CHORD-OF-CALLING",
            name: "Chord of Calling",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Green, Color::Green, Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "convoke",
                "chosen-x",
                "policy-submitted-library-search",
                "typed-creature-mana-value-predicate",
                "battlefield-entry",
                "library-shuffle",
            ],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Convoke],
            effects: vec![Effect::SearchControllerLibrary {
                requirement: LibrarySearchRequirement::CreatureWithManaValueAtMostChosenX,
                destination: LibrarySearchDestination::Battlefield,
                selection: LibrarySearchSelection::PolicySubmitted {
                    may_fail_to_find: true,
                },
            }],
        },
        // Compatibility scope: exact casting cost and typed non-Forest land
        // search into a tapped battlefield entry. The public engine selects
        // the first qualifying card in its deterministic library order until
        // policies can submit the hidden-zone choice, so this is deliberately
        // not a positive full-fidelity definition.
        CardDefinition {
            id: "RAV-FARSEEK",
            name: "Farseek",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "library-nonforest-land-type-search",
                "battlefield-tapped-land-entry",
                "deterministic-library-search-selection",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::SearchControllerLibrary {
                requirement: LibrarySearchRequirement::BasicLandTypes(BTreeSet::from([
                    BasicLandType::Plains,
                    BasicLandType::Island,
                    BasicLandType::Swamp,
                    BasicLandType::Mountain,
                ])),
                destination: LibrarySearchDestination::BattlefieldTapped,
                selection: LibrarySearchSelection::DeterministicFirstMatch,
            }],
        },
        // Compatibility scope: normal enchantment casting plus the
        // stack-backed `{1}`, sacrifice-a-creature cost and typed basic-land
        // search. The public engine's deterministic library selection is
        // explicit bounded behavior rather than a hidden-zone choice claim.
        CardDefinition {
            id: "RAV-PERILOUS-FORAYS",
            name: "Perilous Forays",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Green, Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &[
                "activated-sacrifice-creature-search-basic-land",
                "battlefield-tapped-land-entry",
                "deterministic-library-search-selection",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: Dredge 2 uses the shared replacement selection, and
        // the target creature remains attached to this permanent +3/+3 layer
        // effect until either attachment endpoint leaves the battlefield.
        CardDefinition {
            id: "RAV-MOLDERVINE-CLOAK",
            name: "Moldervine Cloak",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &["full-rules-fidelity", "aura-attach-and-static-pt", "dredge"],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Dredge(2)],
            effects: vec![Effect::AttachSourceAndModifyTargetPt {
                power: 3,
                toughness: 3,
            }],
        },
        // Full fidelity: the generic Aura substrate keeps the exact -3/-1
        // modifier linked to the enchantment and its attached creature until
        // either battlefield endpoint leaves.
        CardDefinition {
            id: "RAV-CLINGING-DARKNESS",
            name: "Clinging Darkness",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &["full-rules-fidelity", "aura-attach-and-static-pt"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::AttachSourceAndModifyTargetPt {
                power: -3,
                toughness: -1,
            }],
        },
        // Compatibility scope: the exact Aura attachment persists on its
        // creature target, but its combat-damage graveyard-return trigger is
        // intentionally omitted until attached-source trigger selection is
        // represented.
        CardDefinition {
            id: "RAV-NECROMANTIC-THIRST",
            name: "Necromantic Thirst",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Black, Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &[
                "aura-static-attachment-only",
                "combat-damage-trigger-not-implemented",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::AttachSourceAndModifyTargetPt {
                power: 0,
                toughness: 0,
            }],
        },
        // Compatibility scope: the exact Aura attachment persists on its
        // creature target. Its ETB targeted discard and enchanted-creature
        // regeneration activation remain deliberately unsupported.
        CardDefinition {
            id: "RAV-STRANDS-OF-UNDEATH",
            name: "Strands of Undeath",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &[
                "aura-static-attachment-only",
                "etb-discard-and-enchanted-regeneration-not-implemented",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::AttachSourceAndModifyTargetPt {
                power: 0,
                toughness: 0,
            }],
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
        // Full fidelity: fixed target-creature damage and life gain resolve
        // with the conditional draw keyed only to the explicit spell-payment
        // receipt retained on this spell's stack object.
        CardDefinition {
            id: "RAV-RIBBONS-OF-NIGHT",
            name: "Ribbons of Night",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "targeted-creature-damage",
                "life-gain",
                "explicit-spent-mana-color-condition",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::DealDamage {
                    amount: 4,
                    target: cardbench_magic_engine::TargetRequirement::Creature,
                },
                Effect::GainLifeController { amount: 4 },
                Effect::DrawControllerIfManaColorSpent { color: Color::Blue },
            ],
        },
        // Full fidelity: the shared Dredge replacement and stack-backed
        // sacrifice-to-draw activation represent each printed rule.
        CardDefinition {
            id: "RAV-GRAVE-SHELL-SCARAB",
            name: "Grave-Shell Scarab",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Black, Color::Green, Color::Green]),
            colors: colors([Color::Black, Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "dredge",
                "base-characteristics",
                "sacrifice-source-draw",
            ],
            power: Some(4),
            toughness: Some(4),
            keywords: vec![Keyword::Dredge(1)],
            effects: vec![],
        },
        // Full fidelity: the shared Dredge replacement and source-sacrifice
        // activation place the represented permanent +1/+1 counter on one
        // creature target through the ordinary stack lifecycle.
        CardDefinition {
            id: "RAV-SHAMBLING-SHELL",
            name: "Shambling Shell",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Black, Color::Green]),
            colors: colors([Color::Black, Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "dredge",
                "base-characteristics",
                "sacrifice-source-target-counter",
            ],
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
        // Full fidelity: the target spell's controller receives an explicit,
        // stale-safe no-priority payment/decline decision while Convolute
        // remains on top of the stack.  The decision accepts only a complete
        // selected `{4}` spend, with explicitly listed mana abilities.
        CardDefinition {
            id: "RAV-CONVOLUTE",
            name: "Convolute",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "counter-target-spell-unless-controller-pays-4",
                "policy-submitted-resolution-payment",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::CounterTargetSpellUnlessControllerPays {
                mana_cost: ManaCost::new(4),
            }],
        },
        // Full fidelity: each owner's graveyard moves into that owner's
        // library, and each library is shuffled at resolution.
        CardDefinition {
            id: "RAV-MNEMONIC-NEXUS",
            name: "Mnemonic Nexus",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "graveyard-to-library",
                "per-player-library-shuffle",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::ShuffleGraveyardsIntoLibraries],
        },
        // Full fidelity: the two target positions remain controller-relative,
        // and both creatures return to their owners' hands on resolution.
        CardDefinition {
            id: "RAV-PEEL-FROM-REALITY",
            name: "Peel from Reality",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "controlled-creature-target",
                "opponent-creature-target",
                "owner-preserving-bounce",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::ReturnControlledCreatureToHand,
                Effect::ReturnOpponentCreatureToHand,
            ],
        },
        // Full fidelity: a policy-selected five-color card color is retained
        // on the spell stack, replaces the creature target's colors in layer
        // five until end of turn, and then the controller draws one card.
        CardDefinition {
            id: "RAV-QUICKCHANGE",
            name: "Quickchange",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "policy-chosen-target-color-replacement-until-end-of-turn",
                "controller-draw",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::ReplaceTargetCreatureColorsWithChosenColorUntilEndOfTurn,
                Effect::DrawController,
            ],
        },
        // Only the hand-zone transmute activation is executable. The printed
        // spell effect is deliberately non-covered in this compatibility
        // definition.
        CardDefinition {
            id: "RAV-DIMIR-MACHINATIONS",
            name: "Dimir Machinations",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Black]),
            colors: colors([Color::Black]),
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
            mana_cost: ManaCost::with_colors(1, [Color::Black]),
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
        // Full fidelity within the represented library-search substrate: the
        // spell installs one public marker for the current turn, then draws
        // exactly one controller card as it resolves.
        CardDefinition {
            id: "RAV-SHADOW-OF-DOUBT",
            name: "Shadow of Doubt",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Blue, Color::Black]),
            colors: colors([Color::Blue, Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["full-rules-fidelity", "library-search-prevention", "draw"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::PreventLibrarySearchUntilEndOfTurn,
                Effect::DrawController,
            ],
        },
        // The targeted bounce face is executable.  Transmute remains an
        // explicit immediate hand-zone compatibility action, so this card is
        // intentionally bounded rather than promoted to the positive manifest.
        CardDefinition {
            id: "RAV-CLUTCH-OF-THE-UNDERCITY",
            name: "Clutch of the Undercity",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Blue, Color::Black]),
            colors: colors([Color::Blue, Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "colored-cost-casting",
                "targeted-permanent-bounce",
                "controller-life-loss",
                "immediate-hand-zone-transmute-compatibility",
            ],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Transmute(ManaCost::with_colors(
                1,
                [Color::Blue, Color::Blue],
            ))],
            effects: vec![Effect::ReturnTargetPermanentToHandAndLoseControllerLife { amount: 3 }],
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
        // The front face uses a typed nonblack-creature target and ordinary
        // regenerable destruction. Hand-zone Transmute remains bounded: it
        // does not create a stack object or response window in this slice.
        CardDefinition {
            id: "RAV-BRAINSPOIL",
            name: "Brainspoil",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Black, Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "colored-cost-casting",
                "destroy-target-nonblack-creature",
                "immediate-hand-zone-transmute-compatibility",
            ],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Transmute(ManaCost::with_colors(
                1,
                [Color::Black, Color::Black],
            ))],
            effects: vec![Effect::DestroyTargetNonblackCreature],
        },
        // Compatibility scope: normal colored-cost creature casting, static
        // Fear, and the shared immediate hand-zone Transmute operation. Its
        // sacrifice-to-regenerate activation remains explicitly outside this
        // bounded slice because the engine does not yet model a selected
        // controlled-creature sacrifice cost for non-mana abilities.
        CardDefinition {
            id: "RAV-DIMIR-HOUSE-GUARD",
            name: "Dimir House Guard",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "colored-cost-casting",
                "base-characteristics",
                "fear",
                "immediate-hand-zone-transmute-compatibility",
            ],
            power: Some(2),
            toughness: Some(3),
            keywords: vec![
                Keyword::Fear,
                Keyword::Transmute(ManaCost::with_colors(1, [Color::Black, Color::Black])),
            ],
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
        // Compatibility scope: the selected X is paid atomically, retained
        // on the spell stack object, and bounds the target's mana value at
        // both cast and resolution. Public policy/scenario syntax has not yet
        // exposed a player-authored chosen-X move, so this direct engine slice
        // remains outside the positive full-fidelity manifest.
        CardDefinition {
            id: "RAV-DISEMBOWEL",
            name: "Disembowel",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "chosen-x-targeted-creature-destruction",
                "policy-chosen-x-move-not-yet-exposed",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::DestroyTargetCreatureWithManaValueAtMostChosenX],
        },
        // Full fidelity within the current deterministic discard-choice
        // boundary: player targeting is a stack slot and the selected card is
        // the target player's oldest hand entry when this spell resolves.
        CardDefinition {
            id: "RAV-NIGHTMARE-VOID",
            name: "Nightmare Void",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "targeted-discard",
                "dredge",
                "deterministic-discard-choice",
            ],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Dredge(2)],
            effects: vec![Effect::DiscardTargetPlayer { count: 1 }],
        },
        // Full fidelity: resolving this instant pauses at an engine-owned,
        // controller-private selection boundary. The spell remains on the
        // stack and neither player receives priority until the selected cards
        // and their life payments are submitted atomically.
        CardDefinition {
            id: "RAV-MOONLIGHT-BARGAIN",
            name: "Moonlight Bargain",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Black, Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "private-library-resolution-choice",
                "per-card-life-payment",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::LookAtTopCardsChooseForLifeOrGraveyard {
                count: 5,
                life_per_card: 2,
            }],
        },
        // Full fidelity: the land is destroyed before the independent
        // all-creature batch is snapshotted, and the latter reads only the
        // stack object's explicit cast-payment receipt.
        CardDefinition {
            id: "RAV-ROLLING-SPOIL",
            name: "Rolling Spoil",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Green, Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "sorcery-casting",
                "destroy-target-land",
                "explicit-spent-mana-color-condition",
                "spent-black-global-minus-one",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::DestroyTargetLand,
                Effect::ModifyAllCreaturesPtUntilEndOfTurnIfManaColorSpent {
                    color: Color::Black,
                    power: -1,
                    toughness: -1,
                },
            ],
        },
        // Full fidelity: the normal creature cast queues one target-free ETB
        // stack object whose dynamic life-loss calculation occurs at
        // resolution and observes every opponent's live battlefield.
        CardDefinition {
            id: "RAV-NETHERBORN-PHALANX",
            name: "Netherborn Phalanx",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(5, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "enter-the-battlefield-opponent-creature-count-life-loss",
                "transmute",
            ],
            power: Some(2),
            toughness: Some(4),
            keywords: vec![Keyword::Transmute(ManaCost::with_colors(
                1,
                [Color::Black, Color::Black],
            ))],
            effects: vec![],
        },
        // Full fidelity: Hex's six typed effect occurrences carry six target
        // slots, which must be distinct when cast but are independently
        // rechecked as the spell resolves.
        CardDefinition {
            id: "RAV-HEX",
            name: "Hex",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::Black, Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &["full-rules-fidelity", "six-distinct-creature-destruction"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::DestroyDistinctTargetCreature,
                Effect::DestroyDistinctTargetCreature,
                Effect::DestroyDistinctTargetCreature,
                Effect::DestroyDistinctTargetCreature,
                Effect::DestroyDistinctTargetCreature,
                Effect::DestroyDistinctTargetCreature,
            ],
        },
        // Full fidelity: the controller's upkeep trigger exposes the top card
        // publicly, moves that same object to hand, then applies its catalog
        // mana value as life loss through the ordinary stack path.
        CardDefinition {
            id: "RAV-DARK-CONFIDANT",
            name: "Dark Confidant",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "beginning-of-upkeep-top-library-reveal-life-loss",
            ],
            power: Some(2),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
        // Bounded compatibility scope: this target-free sorcery snapshots one
        // public creature card from each player's graveyard before returning
        // those cards to hand. The printed per-player selection remains
        // deterministic until policies can submit public-zone choices.
        CardDefinition {
            id: "RAV-EMPTY-THE-CATACOMBS",
            name: "Empty the Catacombs",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "colored-cost-casting",
                "each-player-returns-creature-card-from-graveyard-to-hand",
                "deterministic-public-graveyard-selection",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::ReturnOneCreatureCardFromEachGraveyardToHand],
        },
        // Full printed behavior: the target remains in the casting player's
        // graveyard until the normal resolution-time legality check. The
        // immutable explicit mana-spend receipt controls the one persistent
        // +1/+1 counter placed after its battlefield move.
        CardDefinition {
            id: "RAV-VIGOR-MORTIS",
            name: "Vigor Mortis",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Black, Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "return-target-creature-card-from-graveyard-to-battlefield",
                "spent-green-plus-one-plus-one-counter",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::ReturnTargetCreatureCardToBattlefieldWithCounterIfManaColorSpent {
                    color: Color::Green,
                },
            ],
        },
        // Bounded compatibility scope: the controller-graveyard creature
        // target and the intervening “another creature card” condition use a
        // normal ETB stack trigger. The printed optional decision remains
        // outside the current policy interface.
        CardDefinition {
            id: "RAV-MAUSOLEUM-TURNKEY",
            name: "Mausoleum Turnkey",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "colored-cost-casting",
                "base-characteristics",
                "enter-battlefield-conditional-graveyard-return-to-hand",
                "deterministic-etb-target-selection",
            ],
            power: Some(3),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
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
            supported_rules: &["convoke", "base-characteristics", "trample"],
            power: Some(5),
            toughness: Some(5),
            keywords: vec![Keyword::Convoke, Keyword::Trample],
            effects: vec![],
        },
        // Full fidelity: Convoke payment, base characteristics, and Flying
        // blocker legality are all represented by the expansion-neutral engine.
        CardDefinition {
            id: "RAV-CONCLAVE-EQUENAUT",
            name: "Conclave Equenaut",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::White, Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "convoke",
                "base-characteristics",
                "flying",
            ],
            power: Some(3),
            toughness: Some(3),
            keywords: vec![Keyword::Convoke, Keyword::Flying],
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
        // Full fidelity: Convoke payment, base characteristics, and the
        // expansion-neutral vigilance attack-declaration exception are all
        // represented by the engine.
        CardDefinition {
            id: "RAV-GUARDIAN-OF-VITU-GHAZI",
            name: "Guardian of Vitu-Ghazi",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(6, [Color::Green, Color::White]),
            colors: colors([Color::Green, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "convoke",
                "base-characteristics",
                "vigilance",
            ],
            power: Some(4),
            toughness: Some(7),
            keywords: vec![Keyword::Convoke, Keyword::Vigilance],
            effects: vec![],
        },
        // Compatibility scope: normal creature casting, base characteristics,
        // Convoke payment, and the printed Trample keyword. Multi-block
        // damage assignment remains an explicit engine limitation.
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
            supported_rules: &["convoke", "base-characteristics", "trample"],
            power: Some(9),
            toughness: Some(14),
            keywords: vec![Keyword::Convoke, Keyword::Trample],
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
                "controller-creature-layer-6-trample-grant",
            ],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Convoke],
            effects: vec![
                Effect::ModifyControllerCreaturesPtUntilEndOfTurn {
                    power: 3,
                    toughness: 3,
                },
                Effect::AddKeywordToControllerCreaturesUntilEndOfTurn {
                    keyword: Keyword::Trample,
                },
            ],
        },
        // Full fidelity: Convoke payment, typed artifact-or-enchantment
        // targets, and ordinary destruction are all executable.
        CardDefinition {
            id: "RAV-SUNDERING-VITAE",
            name: "Sundering Vitae",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "convoke",
                "targeted-artifact-or-enchantment-destruction",
            ],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Convoke],
            effects: vec![Effect::DestroyTargetArtifactOrEnchantment],
        },
        // Full fidelity: the ordinary artifact-or-enchantment target is
        // destroyed through the normal stack lifecycle, then two typed green
        // Saproling tokens enter under the resolving controller's control.
        CardDefinition {
            id: "RAV-SEED-SPARK",
            name: "Seed Spark",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "typed-artifact-or-enchantment-target",
                "destroy",
                "saproling-token-creation",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::DestroyTargetArtifactOrEnchantment,
                Effect::CreateToken {
                    token: TokenSpec::saproling(),
                    count: 2,
                },
            ],
        },
        // Full fidelity: the narrow enchantment target is rechecked through
        // the normal all-illegal-target boundary, then Radiance snapshots the
        // target and every other color-sharing enchantment before destroying
        // that complete batch.
        CardDefinition {
            id: "RAV-LEAVE-NO-TRACE",
            name: "Leave No Trace",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "radiance-enchantment-destruction",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::RadianceDestroyEnchantments],
        },
        // Full fidelity: Convoke is paid as part of the ordinary cast, then
        // one target-free resolution instruction snapshots and destroys every
        // live nontoken creature through the normal destruction lifecycle.
        CardDefinition {
            id: "RAV-HOUR-OF-RECKONING",
            name: "Hour of Reckoning",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::White, Color::White, Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "convoke",
                "destroy-all-nontoken-creatures",
            ],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Convoke],
            effects: vec![Effect::DestroyAllNonTokenCreatures],
        },
        // Full fidelity: its base vigilance and static layer-six/layer-seven
        // grants apply only to other creatures under the same controller.
        CardDefinition {
            id: "RAV-OATHSWORN-GIANT",
            name: "Oathsworn Giant",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::White, Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "vigilance",
                "static-other-creatures-vigilance-plus-zero-two",
            ],
            power: Some(3),
            toughness: Some(4),
            keywords: vec![Keyword::Vigilance],
            effects: vec![],
        },
        // Full fidelity: this controller-scoped static layer-seven modifier
        // applies to every other creature, including other anthem sources.
        CardDefinition {
            id: "RAV-VETERAN-ARMORER",
            name: "Veteran Armorer",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "static-other-creatures-plus-zero-one",
            ],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: while any live Aura is attached to this source, the
        // source and every other creature under its controller gain Vigilance.
        CardDefinition {
            id: "RAV-GATE-HOUND",
            name: "Gate Hound",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "static-controller-vigilance-while-enchanted",
            ],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: the immutable static attack binding is evaluated
        // before any attacker state or combat receipt is created, then stops
        // applying as soon as this source leaves the battlefield.
        CardDefinition {
            id: "RAV-BLAZING-ARCHON",
            name: "Blazing Archon",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(6, [Color::White, Color::White, Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "flying",
                "static-opponents-cannot-attack-controller",
            ],
            power: Some(5),
            toughness: Some(6),
            keywords: vec![Keyword::Flying],
            effects: vec![],
        },
        // Full fidelity: the source is sacrificed as an activation cost, then
        // creates a one-shot prevention shield for a player or creature.
        CardDefinition {
            id: "RAV-CAREGIVER",
            name: "Caregiver",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "sacrifice-source-targeted-one-damage-prevention",
            ],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
        // Compatibility scope: every represented positive damage packet is
        // reduced by one live source-bound integer-halving replacement. The
        // binding is intentionally not a full-fidelity claim while the engine
        // has only bounded concurrent replacement-order coverage.
        CardDefinition {
            id: "RAV-GHOSTS-OF-THE-INNOCENT",
            name: "Ghosts of the Innocent",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(5, [Color::White, Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "colored-cost-casting",
                "base-characteristics",
                "static-global-damage-amount-halving",
            ],
            power: Some(4),
            toughness: Some(5),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: this one-target combat prevention effect keeps the
        // target creature's exact incarnation through combat and observes the
        // explicit generic-mana color receipt at resolution.
        CardDefinition {
            id: "RAV-BOROS-FURY-SHIELD",
            name: "Boros Fury-Shield",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "prevent-target-creatures-combat-damage-and-red-spend-controller-damage",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::PreventTargetCreatureCombatDamageUntilEndOfTurn {
                damage_target_controller_equal_to_power_if_mana_color_spent: Some(Color::Red),
            }],
        },
        // Full fidelity: casting supplies an explicit five-color choice that
        // is retained on the stack. Resolution snapshots the caster's current
        // creatures and grants each the corresponding temporary protection.
        CardDefinition {
            id: "RAV-BATHE-IN-LIGHT",
            name: "Bathe in Light",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "chosen-color-controller-creature-protection",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::AddChosenColorProtectionToControllerCreaturesUntilEndOfTurn],
        },
        // Full fidelity: the static controller-relative prevention is bound
        // through derived creature characteristics and is live only while
        // this enchantment remains on the battlefield.
        CardDefinition {
            id: "RAV-LIGHT-OF-SANCTION",
            name: "Light of Sanction",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::White, Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "static-prevent-friendly-source-damage",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: "RAV-FAITHS-FETTERS",
            name: "Faith's Fetters",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "aura-enchant-permanent",
                "etb-gain-four-life",
                "attached-permanent-combat-and-activation-restriction",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::AttachSourceToTarget {
                target: TargetRequirement::Permanent,
                changes: vec![
                    ContinuousChange::AddKeyword(Keyword::CannotAttackOrBlock),
                    ContinuousChange::SuppressNonManaActivatedAbilities,
                ],
            }],
        },
        // Compatibility scope: this creature uses the expansion-neutral Aura
        // attachment lifecycle, but its hidden-zone search is deliberately
        // deterministic. It always takes the first legal Aura in library
        // order; player selection and declining an otherwise legal search
        // remain outside this slice.
        CardDefinition {
            id: "RAV-AURATOUCHED-MAGE",
            name: "Auratouched Mage",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(5, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "colored-cost-casting",
                "base-characteristics",
                "etb-aura-search-and-attach",
                "deterministic-first-compatible-aura-selection",
            ],
            power: Some(3),
            toughness: Some(3),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: Convoke reduces the printed cost and the resolving
        // instruction counts every current battlefield creature, including
        // opposing creatures and tokens, before gaining that much life.
        CardDefinition {
            id: "RAV-CHANT-OF-VITU-GHAZI",
            name: "Chant of Vitu-Ghazi",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(6, [Color::White, Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "convoke-dynamic-battlefield-life-gain",
            ],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Convoke],
            effects: vec![Effect::GainLifeForEachCreature],
        },
        // Full fidelity: the spell-level chosen-X receipt pays the printed
        // additional generic amount, then creates an independent shield for
        // its controller before the ordinary card draw and terminal move.
        CardDefinition {
            id: "RAV-FESTIVAL-OF-THE-GUILDPACT",
            name: "Festival of the Guildpact",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "chosen-x-controller-damage-prevention-and-draw",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::AddControllerDamageShieldEqualToChosenXUntilEndOfTurn,
                Effect::DrawController,
            ],
        },
        // Full fidelity: this private selected batch filters Aura semantics,
        // permits zero through three choices with distinct printed names,
        // reveals each answer, moves it to hand, then shuffles immediately.
        CardDefinition {
            id: "RAV-THREE-DREAMS",
            name: "Three Dreams",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "private-up-to-three-distinct-name-aura-search-reveal-hand-shuffle",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::SearchControllerLibraryMany {
                requirement: LibrarySearchRequirement::Aura,
                destination: LibrarySearchDestination::Hand,
                cardinality:
                    cardbench_magic_engine::LibrarySearchCardinality::ZeroOrMoreDistinctNames {
                        maximum: 3,
                    },
                selection: LibrarySearchSelection::PolicySubmitted {
                    may_fail_to_find: false,
                },
                reveal_selected: true,
            }],
        },
        // Full fidelity: Convoke remains part of ordinary casting while the
        // attached layer-seven effect recalculates from the enchanted
        // creature controller's other current creatures.
        CardDefinition {
            id: "RAV-CONCLAVES-BLESSING",
            name: "Conclave's Blessing",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "aura-convoke-dynamic-other-controller-creature-toughness",
            ],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Convoke],
            effects: vec![Effect::AttachSourceToTarget {
                target: TargetRequirement::Creature,
                changes: vec![
                    ContinuousChange::ModifyPowerToughnessForEachOtherCreatureControlledByTarget {
                        power_per_creature: 0,
                        toughness_per_creature: 2,
                    },
                ],
            }],
        },
        // Full fidelity: ordinary Aura attachment preserves the exact source
        // and target incarnations used by Flickerform's stack-backed blink.
        // The shared linked-exile substrate returns the creature and every
        // Aura attached to that exact creature at the next end step.
        CardDefinition {
            id: "RAV-FLICKERFORM",
            name: "Flickerform",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "aura-enchant-creature",
                "aura-linked-exile-and-next-end-step-return",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::AttachSourceToTarget {
                target: TargetRequirement::Creature,
                changes: vec![],
            }],
        },
        // Full fidelity: this immutable battlefield binding raises every
        // nonmana activated ability's generic cost while leaving mana
        // abilities outside the modifier's scope.
        CardDefinition {
            id: "RAV-SUPPRESSION-FIELD",
            name: "Suppression Field",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "static-nonmana-activated-ability-tax",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: this static entry replacement is checked at every
        // ordinary opposing artifact, creature, or land battlefield entry.
        CardDefinition {
            id: "RAV-LOXODON-GATEKEEPER",
            name: "Loxodon Gatekeeper",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::White, Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "static-opponents-artifacts-creatures-lands-enter-tapped",
            ],
            power: Some(2),
            toughness: Some(3),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: this target is controller-scoped at both cast and
        // resolution, then moves through the normal hand-zone lifecycle.
        CardDefinition {
            id: "RAV-RECOLLECT",
            name: "Recollect",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &["full-rules-fidelity", "targeted-own-graveyard-return"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::ReturnTargetCardToHand],
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
        // Full printed behavior: every positive damage receipt to this
        // permanent queues one fixed life-gain trigger for its controller.
        CardDefinition {
            id: "RAV-DROMAD-PUREBRED",
            name: "Dromad Purebred",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "damage-received-life-gain",
            ],
            power: Some(1),
            toughness: Some(5),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: normal colored-cost casting, base characteristics,
        // and Flying blocker legality are represented by the shared engine.
        CardDefinition {
            id: "RAV-SNAPPING-DRAKE",
            name: "Snapping Drake",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "flying",
            ],
            power: Some(3),
            toughness: Some(2),
            keywords: vec![Keyword::Flying],
            effects: vec![],
        },
        // Full fidelity: double strike is enforced by the shared two-step
        // combat damage state machine.
        CardDefinition {
            id: "RAV-CARRION-HOWLER",
            name: "Carrion Howler",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "double-strike",
            ],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![Keyword::DoubleStrike],
            effects: vec![],
        },
        // Full fidelity: positive damage received creates a source-specific
        // trigger whose captured amount is dealt to every surviving player.
        CardDefinition {
            id: "RAV-COALHAULER-SWINE",
            name: "Coalhauler Swine",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::Red, Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "damage-received-to-each-player",
            ],
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
        // Full fidelity: the sorcery destroys one targeted land, then adds
        // exactly two red mana to its controller as a stack effect.
        CardDefinition {
            id: "RAV-SEISMIC-SPIKE",
            name: "Seismic Spike",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "destroy-target-land",
                "add-two-red-mana",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::DestroyTargetLand,
                Effect::AddManaController {
                    color: Color::Red,
                    amount: 2,
                },
            ],
        },
        // Full fidelity: the enter-the-battlefield trigger selects a legal
        // player or creature and deals one damage when that trigger resolves.
        CardDefinition {
            id: "RAV-SPARKMAGE-APPRENTICE",
            name: "Sparkmage Apprentice",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "etb-targeted-damage",
            ],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: haste/flying are static characteristics and the ETB
        // trigger creates three first-striking 2/2 white Knight creature
        // tokens for one targeted opponent.
        CardDefinition {
            id: "RAV-HUNTED-DRAGON",
            name: "Hunted Dragon",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Red, Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "flying",
                "haste",
                "etb-targeted-opponent-knight-tokens",
            ],
            power: Some(6),
            toughness: Some(6),
            keywords: vec![Keyword::Flying, Keyword::Haste],
            effects: vec![],
        },
        // Full fidelity: the legendary Boros creature carries its three
        // printed combat keywords and its two-target damage-redirection tap
        // activation is bound through the shared replacement substrate below.
        CardDefinition {
            id: "RAV-RAZIA-BOROS-ARCHANGEL",
            name: "Razia, Boros Archangel",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(
                4,
                [Color::Red, Color::Red, Color::White, Color::White],
            ),
            colors: colors([Color::Red, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "flying",
                "first-strike",
                "vigilance",
                "haste",
                "tap-redirect-three-damage",
            ],
            power: Some(6),
            toughness: Some(3),
            keywords: vec![Keyword::Flying, Keyword::Vigilance, Keyword::Haste],
            effects: vec![],
        },
        // Full fidelity: the tap ability snapshots every non-Flying creature
        // and deals four damage to each through the normal damage/SBA batch.
        CardDefinition {
            id: "RAV-HAMMERFIST-GIANT",
            name: "Hammerfist Giant",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::Red, Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "tap-global-nonflying-damage",
            ],
            power: Some(5),
            toughness: Some(4),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: the enchantment's life-gain trigger pays its
        // optional generic cost before stacking a two-damage target effect.
        CardDefinition {
            id: "RAV-SEARING-MEDITATION",
            name: "Searing Meditation",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Red, Color::White]),
            colors: colors([Color::Red, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "life-gain-trigger",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: the one-mana Radiance haste grant uses the shared
        // target-color selection and layer-6 keyword effect.
        CardDefinition {
            id: "RAV-SURGE-OF-ZEAL",
            name: "Surge of Zeal",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "radiance-grant-haste",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::RadianceAddKeywordUntilEndOfTurn {
                keyword: Keyword::Haste,
            }],
        },
        // Full fidelity: Radiance gives the target and each creature sharing
        // one of its colors a temporary CannotBlock keyword.
        CardDefinition {
            id: "RAV-INCITE-HYSTERIA",
            name: "Incite Hysteria",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "radiance-cannot-block",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::RadianceAddKeywordUntilEndOfTurn {
                keyword: Keyword::CannotBlock,
            }],
        },
        // Full printed behavior: static Defender and the bound stack-backed
        // enter-the-battlefield draw trigger.
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
        // Complete scoped behavior: normal colored-cost creature casting,
        // base characteristics, and the two represented combat keywords.
        CardDefinition {
            id: "RAV-SKYKNIGHT-LEGIONNAIRE",
            name: "Skyknight Legionnaire",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Red, Color::White]),
            colors: colors([Color::Red, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "flying",
                "haste",
            ],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![Keyword::Flying, Keyword::Haste],
            effects: vec![],
        },
        // Full printed behavior: normal colored-cost creature casting, base
        // characteristics, Flying, and its controller's upkeep life-loss
        // trigger, which is stack-backed before upkeep priority.
        CardDefinition {
            id: "RAV-MOROII",
            name: "Moroii",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Blue, Color::Black]),
            colors: colors([Color::Blue, Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "flying",
                "upkeep-controller-life-loss",
            ],
            power: Some(4),
            toughness: Some(4),
            keywords: vec![Keyword::Flying],
            effects: vec![],
        },
        // Compatibility scope: normal colored-cost creature casting, base
        // characteristics, and static Flying. Szadek's combat-damage mill and
        // counter trigger remains separate engine work and is not approximated.
        CardDefinition {
            id: "RAV-SZADEK",
            name: "Szadek, Lord of Secrets",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(
                3,
                [Color::Blue, Color::Blue, Color::Black, Color::Black],
            ),
            colors: colors([Color::Blue, Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["colored-cost-casting", "base-characteristics", "flying"],
            power: Some(5),
            toughness: Some(5),
            keywords: vec![Keyword::Flying],
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
        // Full fidelity: double strike is enforced by the shared two-step
        // combat damage state machine.
        CardDefinition {
            id: "RAV-BOROS-SWIFTBLADE",
            name: "Boros Swiftblade",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Red, Color::White]),
            colors: colors([Color::Red, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "double-strike",
            ],
            power: Some(1),
            toughness: Some(2),
            keywords: vec![Keyword::DoubleStrike],
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
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "defender",
                "enter-the-battlefield-draw",
            ],
            power: Some(2),
            toughness: Some(5),
            keywords: vec![Keyword::Defender],
            effects: vec![],
        },
        // Full fidelity: the source's optional dies trigger remains stack
        // backed after its graveyard move and asks its controller whether to
        // gain the fixed three life.
        CardDefinition {
            id: "RAV-CENTAUR-SAFEGUARD",
            name: "Centaur Safeguard",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Green, Color::White]),
            colors: colors([Color::Green, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "optional-dies-gain-three-life",
            ],
            power: Some(3),
            toughness: Some(1),
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
        // Full fidelity: this colorless artifact costs `{2}` and carries one
        // ordinary stack-backed `{3}, {T}` activation. At resolution it taps
        // one creature, then returns this exact permanent incarnation to its
        // owner's hand through the shared source-return instruction.
        CardDefinition {
            id: "RAV-CYCLOPEAN-SNARE",
            name: "Cyclopean Snare",
            set_code: SET_CODE,
            mana_cost: ManaCost::new(2),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Artifact]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colorless-cost-casting",
                "generic-three-tap-target-creature-return-source",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: this Equipment costs `{3}`, has Flash, attaches to
        // one controlled creature through its ordinary target-bearing ETB
        // trigger when possible, and retains its separate sorcery-speed
        // `{1}` equip activation through the shared attachment lifecycle.
        CardDefinition {
            id: "RAV-GRIFTERS-BLADE",
            name: "Grifter's Blade",
            set_code: SET_CODE,
            mana_cost: ManaCost::new(3),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Artifact]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colorless-cost-casting",
                "flash",
                "etb-attach-to-controlled-creature",
                "equipment-plus-one-plus-one",
                "sorcery-speed-equip-one",
            ],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Flash],
            effects: vec![],
        },
        // This definition is complete for the public RAV Birds of Paradise
        // card: normal creature characteristics, Flying, and its one explicit
        // chosen-color tap mana ability all use shared, directly tested rules
        // substrates below.
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
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "flying",
                "tap-choice-single-color-mana-ability",
                "cast-payment-mana-activation",
            ],
            power: Some(0),
            toughness: Some(1),
            keywords: vec![Keyword::Flying],
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
        // Full fidelity: a selected controlled creature is sacrificed as a
        // five-mana activation cost, then a Flying creature is destroyed.
        CardDefinition {
            id: "RAV-ELVISH-SKYSWEEPER",
            name: "Elvish Skysweeper",
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
                "sacrifice-creature-destroy-flying",
            ],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: the attack trigger's optional red payment and
        // target choice are represented by the attack-trigger binding.
        CardDefinition {
            id: "RAV-FRENZIED-GOBLIN",
            name: "Frenzied Goblin",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "attack-trigger-optional-red-cannot-block",
            ],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: its static typed landwalk is captured during attacker
        // declaration and rechecked against the fixed defender for blockers.
        CardDefinition {
            id: "RAV-GRAYSCALED-GHARIAL",
            name: "Grayscaled Gharial",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "islandwalk",
            ],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![Keyword::Landwalk(BasicLandType::Island)],
            effects: vec![],
        },
        // Full fidelity: the typed {1}{R} self-pump uses the shared activated
        // ability stack and layer-7 temporary effect.
        CardDefinition {
            id: "RAV-GREATER-FORGELING",
            name: "Greater Forgeling",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Red, Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "activated-plus-three-minus-three",
            ],
            power: Some(3),
            toughness: Some(4),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: normal colored-cost casting, base characteristics,
        // and Reach's Flying-block declaration exception are represented by
        // the shared engine.
        CardDefinition {
            id: "RAV-GOLIATH-SPIDER",
            name: "Goliath Spider",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(6, [Color::Green, Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "reach",
            ],
            power: Some(7),
            toughness: Some(6),
            keywords: vec![Keyword::Reach],
            effects: vec![],
        },
        CardDefinition {
            id: "RAV-IVY-DANCER",
            name: "Ivy Dancer",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "tap-target-creature-grant-forestwalk",
            ],
            power: Some(1),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
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
        // Full printed behavior: this creature's explicit generic cost plus
        // source and other-creature tap costs are paid before its stack-backed
        // Saproling creation resolves.
        CardDefinition {
            id: "RAV-SELESNYA-EVANGEL",
            name: "Selesnya Evangel",
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
                "activated-token-creation-with-creature-tap-cost",
            ],
            power: Some(1),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        // Full printed behavior: four selected untapped creatures are an
        // explicit ability cost, and the artifact-or-enchantment target is
        // rechecked through the ordinary stack resolution path.
        CardDefinition {
            id: "RAV-NULLMAGE-SHEPHERD",
            name: "Nullmage Shepherd",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "tap-four-untapped-controlled-creatures",
                "destroy-target-artifact-or-enchantment",
            ],
            power: Some(2),
            toughness: Some(4),
            keywords: vec![],
            effects: vec![],
        },
        // Full printed behavior: every represented land entry queues a
        // source-untap trigger, and the source's tap activation rechecks a
        // target land through the ordinary stack-resolution path.
        CardDefinition {
            id: "RAV-STONE-SEEDER-HIEROPHANT",
            name: "Stone-Seeder Hierophant",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Green, Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "landfall-untap",
                "tap-untap-target-land",
            ],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
        // Compatibility scope: normal colored-cost creature casting, base
        // characteristics, and Reach. Its printed tap-to-damage activation
        // remains deliberately omitted from this compatibility slice.
        CardDefinition {
            id: "RAV-SELESNYA-SAGITTARS",
            name: "Selesnya Sagittars",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Green, Color::White]),
            colors: colors([Color::Green, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["colored-cost-casting", "base-characteristics", "reach"],
            power: Some(2),
            toughness: Some(5),
            keywords: vec![Keyword::Reach],
            effects: vec![],
        },
        // Full fidelity: Flying and the source-relative blocker restriction
        // are represented by the shared combat and layer substrates.
        CardDefinition {
            id: "RAV-SCREECHING-GRIFFIN",
            name: "Screeching Griffin",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "flying",
                "activated-prevent-target-blocking-source",
            ],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![Keyword::Flying],
            effects: vec![],
        },
        // Full fidelity: damage received queues a non-targeting trigger whose
        // captured amount mills the controller of the damage source.
        CardDefinition {
            id: "RAV-BELLTOWER-SPHINX",
            name: "Belltower Sphinx",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "flying",
                "damage-received-source-controller-mill-that-many",
            ],
            power: Some(2),
            toughness: Some(5),
            keywords: vec![Keyword::Flying],
            effects: vec![],
        },
        // Full fidelity: this Aura attaches to a creature, grants Flying,
        // and its ETB trigger draws two cards through the ordinary stack.
        CardDefinition {
            id: "RAV-FLIGHT-OF-FANCY",
            name: "Flight of Fancy",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "aura-enchant-creature-flying-etb-draw-two",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::AttachSourceToTarget {
                target: TargetRequirement::Creature,
                changes: vec![ContinuousChange::AddKeyword(Keyword::Flying)],
            }],
        },
        // Full fidelity: Defender and the tap activation that mills a target
        // player for two cards are both typed engine rules.
        // Full fidelity: resolution snapshots the controller's registered
        // Island count, then makes one ordinary spell-effect draw for each
        // snapshot member. Opponent Islands never contribute.
        CardDefinition {
            id: "RAV-FLOW-OF-IDEAS",
            name: "Flow of Ideas",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(5, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &["full-rules-fidelity", "draw-for-each-controlled-island"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::DrawControllerForEachControlledBasicLandType {
                land_type: BasicLandType::Island,
            }],
        },
        // Full fidelity: the control effect is derived from the live Aura
        // source rather than the player who happened to cast it. Attachment
        // and source departure use the shared layer-two lifecycle.
        CardDefinition {
            id: "RAV-DREAM-LEASH",
            name: "Dream Leash",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Blue, Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "aura-enchant-permanent-source-controller-control",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::AttachSourceToTarget {
                target: TargetRequirement::Permanent,
                changes: vec![ContinuousChange::ChangeControllerToSourceController],
            }],
        },
        // Full fidelity: the control effect is derived from the live Aura
        // source rather than the player who happened to cast it. Attachment
        // and source departure use the shared layer-two lifecycle.
        CardDefinition {
            id: "RAV-REMAND",
            name: "Remand",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "counter-target-spell-then-draw-controller",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::CounterTargetSpell, Effect::DrawController],
        },
        // Full fidelity: the shared counter/mill instruction captures the
        // physical target spell's controller and mana value before the
        // counter terminal move. The retained explicit payment receipt gates
        // only the follow-up mill, rather than reading a mutable mana pool.
        CardDefinition {
            id: "RAV-INDUCE-PARANOIA",
            name: "Induce Paranoia",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "counter-spell-then-mill-controller-by-mana-value-if-blue-spent",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::CounterTargetPhysicalSpellThenMillItsControllerByManaValueIfManaColorSpent {
                    color: Color::Blue,
                },
            ],
        },
        // Full fidelity: the controller alone receives the exact top-three
        // snapshot once the stack spell resolves, then submits an exhaustive
        // private hand/top/bottom partition through the shared decision
        // continuation. No priority window or public candidate receipt opens.
        CardDefinition {
            id: "RAV-TELLING-TIME",
            name: "Telling Time",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "private-top-library-hand-top-bottom-partition",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::LookAtTopCardsPutOneInHandOneOnTopRestOnBottom { count: 3 }],
        },
        // Full fidelity: the Aura attaches only to a creature, then its
        // active-controller upkeep trigger reads that exact live attachment
        // endpoint when it resolves. Returning the creature follows the
        // normal owner-hand lifecycle, after which SBA cleans up the Aura.
        CardDefinition {
            id: "RAV-MARK-OF-EVICTION",
            name: "Mark of Eviction",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "aura-enchant-creature-upkeep-return-enchanted-creature",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::AttachSourceToTarget {
                target: TargetRequirement::Creature,
                changes: vec![],
            }],
        },
        // Full fidelity: the Aura attaches only to a creature, then its
        // active-controller upkeep trigger reads that exact live attachment
        // endpoint when it resolves. Returning the creature follows the
        // normal owner-hand lifecycle, after which SBA cleans up the Aura.
        // Full fidelity: Defender and the normal targeted mill activation are
        // both expansion-neutral typed engine rules.
        CardDefinition {
            id: "RAV-VEDALKEN-ENTRANCER",
            name: "Vedalken Entrancer",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "defender",
                "tap-blue-target-player-mill-two",
            ],
            power: Some(1),
            toughness: Some(4),
            keywords: vec![Keyword::Defender],
            effects: vec![],
        },
        // Full fidelity: the two independent stack abilities use the typed
        // arbitrary-permanent untap effect and source-relative temporary
        // Defender removal respectively.
        CardDefinition {
            id: "RAV-TIDEWATER-MINION",
            name: "Tidewater Minion",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "defender",
                "tap-untap-target-permanent-and-blue-lose-defender",
            ],
            power: Some(4),
            toughness: Some(4),
            keywords: vec![Keyword::Defender],
            effects: vec![],
        },
        // Full fidelity: either color pays each hybrid cast symbol; the two
        // stack-backed activations create a green 3/3 Centaur or temporarily
        // modify every creature the controller owns.
        CardDefinition {
            id: "RAV-SELESNYA-GUILDMAGE",
            name: "Selesnya Guildmage",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_hybrid(
                0,
                [],
                [
                    HybridManaSymbol {
                        first: Color::Green,
                        second: Color::White,
                    },
                    HybridManaSymbol {
                        first: Color::Green,
                        second: Color::White,
                    },
                ],
            ),
            colors: colors([Color::Green, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "hybrid-cost-casting",
                "base-characteristics",
                "activated-green-centaur-token",
                "activated-controller-creature-anthem",
            ],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: either color pays each hybrid cast symbol; both
        // target-creature activations use the stack, including one composite
        // layer-seven and layer-six modifier that retains exactly one target.
        CardDefinition {
            id: "RAV-GOLGARI-GUILDMAGE",
            name: "Golgari Guildmage",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_hybrid(
                0,
                [],
                [
                    HybridManaSymbol {
                        first: Color::Black,
                        second: Color::Green,
                    },
                    HybridManaSymbol {
                        first: Color::Black,
                        second: Color::Green,
                    },
                ],
            ),
            colors: colors([Color::Black, Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "hybrid-cost-casting",
                "base-characteristics",
                "activated-target-pump-and-trample",
                "activated-regenerate-target-creature",
            ],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        // Bounded compatibility scope: either color pays each hybrid symbol,
        // and the two target-player activations use the normal stack. The
        // target player's hidden-hand choice for discard remains deterministic
        // until policies can submit it, so this does not enter the positive
        // full-fidelity manifest.
        CardDefinition {
            id: "RAV-DIMIR-GUILDMAGE",
            name: "Dimir Guildmage",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_hybrid(
                0,
                [],
                [
                    HybridManaSymbol {
                        first: Color::Blue,
                        second: Color::Black,
                    },
                    HybridManaSymbol {
                        first: Color::Blue,
                        second: Color::Black,
                    },
                ],
            ),
            colors: colors([Color::Blue, Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "hybrid-cost-casting",
                "base-characteristics",
                "sorcery-speed-target-player-draw",
                "target-player-resolution-time-discard",
            ],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: this creature has ordinary Flying plus a stack-backed
        // Blue activation that moves only its exact live incarnation to its
        // owner's library and shuffles that owner. The operation deliberately
        // does not follow a temporary controller.
        CardDefinition {
            id: "RAV-CERULEAN-SPHINX",
            name: "Cerulean Sphinx",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::Blue, Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "flying",
                "activated-source-owner-library-shuffle",
            ],
            power: Some(5),
            toughness: Some(5),
            keywords: vec![Keyword::Flying],
            effects: vec![],
        },
        // Full fidelity: normal colored-cost creature casting, base
        // characteristics, unblockability, and its stack-backed targeted ETB
        // token trigger are represented. The shared trigger substrate exposes
        // the target choice only to the ability controller before the trigger
        // enters the stack.
        CardDefinition {
            id: "RAV-HUNTED-PHANTASM",
            name: "Hunted Phantasm",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Blue, Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "cannot-be-blocked",
                "enter-battlefield-targeted-opponent-goblin-token-creation",
                "policy-submitted-trigger-target",
            ],
            power: Some(4),
            toughness: Some(6),
            keywords: vec![Keyword::Unblockable],
            effects: vec![],
        },
        // Full fidelity: normal colored-cost casting, base characteristics,
        // Flying blocker legality, and the stack-backed black self-regeneration
        // activation are represented by the shared engine.
        CardDefinition {
            id: "RAV-TATTERED-DRAKE",
            name: "Tattered Drake",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "flying",
                "self-regeneration",
            ],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![Keyword::Flying],
            effects: vec![],
        },
        // Full fidelity: the entry trigger obtains one policy-submitted
        // creature target, then uses the owner-indexed ordinary zone lifecycle
        // to place that exact incarnation on its owner's library top.
        CardDefinition {
            id: "RAV-VEDALKEN-DISMISSER",
            name: "Vedalken Dismisser",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(5, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "etb-target-creature-owner-library-top",
                "policy-submitted-trigger-target",
            ],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: the source's own Blocks trigger queues only after
        // legal blocker commitment and required damage-order choices, then
        // returns that exact source incarnation to its owner's hand through
        // the ordinary stack lifecycle.
        CardDefinition {
            id: "RAV-ZEPHYR-SPIRIT",
            name: "Zephyr Spirit",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(5, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "blocks-return-source-owner-hand",
            ],
            power: Some(0),
            toughness: Some(6),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity for the exercised deterministic trigger path: another
        // creature dying stacks the source-identified all-player discard
        // ability. The effect's card selections are visible through explicit
        // discard and zone-change receipts.
        CardDefinition {
            id: "RAV-SADISTIC-AUGERMAGE",
            name: "Sadistic Augermage",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "another-creature-dies-each-player-discards",
            ],
            power: Some(3),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: red-source damage is prevented by a static target
        // keyword evaluated by the expansion-neutral damage dispatcher.
        CardDefinition {
            id: "RAV-INDENTURED-OAF",
            name: "Indentured Oaf",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "prevent-damage-from-red-sources",
            ],
            power: Some(4),
            toughness: Some(3),
            keywords: vec![Keyword::PreventDamageFromColor(Color::Red)],
            effects: vec![],
        },
        // Full-fidelity scope: colored-cost creature casting, base
        // characteristics, Defender, and the three-land activation.
        CardDefinition {
            id: "RAV-TORPID-MOLOCH",
            name: "Torpid Moloch",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "defender",
                "sacrifice-three-lands-remove-defender",
            ],
            power: Some(3),
            toughness: Some(2),
            keywords: vec![Keyword::Defender],
            effects: vec![],
        },
        // Full fidelity: normal colored-cost creature casting, base
        // characteristics, and the typed tap-to-deal-one activated ability.
        CardDefinition {
            id: "RAV-VIASHINO-FANGTAIL",
            name: "Viashino Fangtail",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "tap-deal-one-to-player-or-creature",
            ],
            power: Some(3),
            toughness: Some(3),
            keywords: vec![],
            effects: vec![],
        },
        // Compatibility scope: normal colored-cost creature casting, base
        // characteristics, and Defender. Its damage-prevention activation
        // remains deliberately unsupported, so this is not a full-fidelity
        // card.
        CardDefinition {
            id: "RAV-BENEVOLENT-ANCESTOR",
            name: "Benevolent Ancestor",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["colored-cost-casting", "base-characteristics", "defender"],
            power: Some(0),
            toughness: Some(4),
            keywords: vec![Keyword::Defender],
            effects: vec![],
        },
        // Full fidelity: normal colored-cost casting, Flying combat legality,
        // and the target-free dies trigger use the shared trigger stack before
        // drawing for the source controller.
        CardDefinition {
            id: "RAV-SURVEILLING-SPRITE",
            name: "Surveilling Sprite",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "flying",
                "dies-draw-controller",
            ],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![Keyword::Flying],
            effects: vec![],
        },
        // Full fidelity: a controller-selected basic land type is retained
        // through the stack and temporarily replaces every controlled land's
        // typed intrinsic mana ability in layer four.
        CardDefinition {
            id: "RAV-TERRAFORMER",
            name: "Terraformer",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "controller-lands-chosen-basic-land-type-until-end-of-turn",
            ],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
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
        // Full fidelity: normal colored-cost creature casting, base
        // characteristics, static Fear, and the stack-backed self-regeneration
        // activation are represented by the shared engine.
        CardDefinition {
            id: "RAV-SEWERDREG",
            name: "Sewerdreg",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Black, Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "fear",
                "regeneration",
            ],
            power: Some(3),
            toughness: Some(3),
            keywords: vec![Keyword::Fear],
            effects: vec![],
        },
        // Full fidelity for the printed Mountainwalk evasion restriction.
        CardDefinition {
            id: "RAV-GOBLIN-SPELUNKERS",
            name: "Goblin Spelunkers",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "mountainwalk",
            ],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![Keyword::Mountainwalk],
            effects: vec![],
        },
        // Full fidelity: the white activated prevention shield is represented
        // by the expansion-neutral damage-prevention layer and stack binding.
        CardDefinition {
            id: "RAV-ORDRUUN-COMMANDO",
            name: "Ordruun Commando",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "activated-prevent-one-damage-to-self",
            ],
            power: Some(4),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: the typed {R}, discard-a-card self-pump uses the
        // shared activated ability stack and layer-7 temporary effect.
        CardDefinition {
            id: "RAV-VIASHINO-SLASHER",
            name: "Viashino Slasher",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "activated-plus-one-minus-one",
            ],
            power: Some(1),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        // The ETB trigger uses the shared typed basic-land search and moves
        // its deterministic controller-owned selection to hand. A policy
        // still cannot choose among hidden library candidates, so this remains
        // a bounded compatibility slice rather than a fidelity promotion.
        CardDefinition {
            id: "RAV-CIVIC-WAYFINDER",
            name: "Civic Wayfinder",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "colored-cost-casting",
                "base-characteristics",
                "enter-the-battlefield-basic-land-search",
                "deterministic-controller-library-search",
            ],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        CardDefinition {
            id: "RAV-DOWSING-SHAMAN",
            name: "Dowsing Shaman",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Green, Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "return-target-enchantment-from-graveyard",
            ],
            power: Some(3),
            toughness: Some(4),
            keywords: vec![],
            effects: vec![],
        },
        // Compatibility scope: normal colored-cost creature casting, base
        // characteristics, and static Flying. Its upkeep sacrifice behavior
        // is deliberately unsupported.
        CardDefinition {
            id: "RAV-WOEBRINGER-DEMON",
            name: "Woebringer Demon",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Black, Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["colored-cost-casting", "base-characteristics", "flying"],
            power: Some(4),
            toughness: Some(4),
            keywords: vec![Keyword::Flying],
            effects: vec![],
        },
        // Full fidelity: normal colored-cost casting, base characteristics,
        // and the `{1}, sacrifice a creature` activation. Its target
        // opponent's top two cards are only exposed to the controller while
        // the stack ability is suspended; the chosen exile is public.
        CardDefinition {
            id: "RAV-THOUGHTPICKER-WITCH",
            name: "Thoughtpicker Witch",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "activated-sacrifice-creature",
                "target-opponent",
                "private-opponent-library-exile-choice",
            ],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: normal colored-cost creature casting, base
        // characteristics, static black-only evasion, and its stack-backed
        // black-mana self-pump activation.
        CardDefinition {
            id: "RAV-UNDERCITY-SHADE",
            name: "Undercity Shade",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "black-only-evasion",
                "black-mana-self-pump",
            ],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![Keyword::BlackEvasion],
            effects: vec![],
        },
        // Full fidelity for the deterministic exercised path: its mandatory
        // entry trigger sacrifices one controller-owned creature through the
        // shared stack and zone-transition substrate.
        CardDefinition {
            id: "RAV-VINDICTIVE-MOB",
            name: "Vindictive Mob",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::Black, Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "enter-battlefield-sacrifice-creature",
                "saproling-block-restriction",
            ],
            power: Some(5),
            toughness: Some(5),
            keywords: vec![Keyword::SaprolingsCannotBlock],
            effects: vec![],
        },
        // Full fidelity: the `{R}, sacrifice` land-destruction activation is
        // represented by the shared typed target and zone-change substrate.
        CardDefinition {
            id: "RAV-BARBARIAN-RIFTCUTTER",
            name: "Barbarian Riftcutter",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "sacrifice-source-destroy-land",
            ],
            power: Some(3),
            toughness: Some(3),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: Excruciator's static source property is represented
        // by the expansion-neutral damage-prevention keyword. Damage dealt by
        // this source bypasses temporary prevention shields.
        CardDefinition {
            id: "RAV-EXCRUCIATOR",
            name: "Excruciator",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(6, [Color::Red, Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "damage-cannot-be-prevented",
            ],
            power: Some(7),
            toughness: Some(7),
            keywords: vec![Keyword::DamageCannotBePrevented],
            effects: vec![],
        },
        // Compatibility scope: normal colored-cost creature casting, base
        // characteristics, and Haste. Its must-block restriction and activated
        // power boost remain deliberately unsupported, so this is not a
        // full-fidelity card.
        CardDefinition {
            id: "RAV-GOBLIN-FIRE-FIEND",
            name: "Goblin Fire Fiend",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "haste",
                "must-block-if-able",
                "activated-plus-one-power",
            ],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![Keyword::Haste, Keyword::MustBeBlockedIfAble],
            effects: vec![],
        },
        // Full fidelity: changing from the battlefield to the graveyard
        // queues a controller-directed two-damage trigger.
        CardDefinition {
            id: "RAV-SELL-SWORD-BRUTE",
            name: "Sell-Sword Brute",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "dies-deal-two-to-controller",
            ],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: `{R}, sacrifice this creature` is a typed stack
        // ability that deals two damage to a blocking creature.
        CardDefinition {
            id: "RAV-WAR-TORCH-GOBLIN",
            name: "War-Torch Goblin",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "sacrifice-source",
                "targeted-damage",
            ],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
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
        // Full fidelity: the source's dies trigger creates one typed white
        // Spirit token with Flying through the ordinary trigger stack.
        CardDefinition {
            id: "RAV-TRANSLUMINANT",
            name: "Transluminant",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "dies-create-flying-spirit",
            ],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: any other creature's battlefield departure queues a
        // target-free optional trigger, while the stack-backed activation
        // creates one typed white Flying Spirit token.
        CardDefinition {
            id: "RAV-TWILIGHT-DROVER",
            name: "Twilight Drover",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "another-creature-leaves-battlefield-plus-one-counter",
                "activated-create-flying-spirit",
            ],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
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
        // Full fidelity: a single green mana targets a creature and gives it
        // +1/+1 through the ordinary stack and layer-seven lifecycle.
        CardDefinition {
            id: "RAV-URSAPINE",
            name: "Ursapine",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Green, Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "activated-target-pump",
            ],
            power: Some(3),
            toughness: Some(3),
            keywords: vec![],
            effects: vec![],
        },
        // Compatibility scope: its exact base characteristics and controller-
        // scoped land-entry counter trigger are executable below. The card
        // remains outside the positive manifest until that trigger is also
        // exercised through the public scenario policy surface.
        bounded_creature_chassis(
            "RAV-VINELASHER-KUDZU",
            "Vinelasher Kudzu",
            ManaCost::with_colors(1, [Color::Green]),
            colors([Color::Green]),
            1,
            1,
        ),
        // Full fidelity: the selected controlled-creature sacrifice is paid
        // atomically before a target creature receives its layer-seven
        // reduction through the ordinary stack resolution path.
        CardDefinition {
            id: "RAV-DROOLING-GROODION",
            name: "Drooling Groodion",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Black, Color::Black, Color::Green]),
            colors: colors([Color::Black, Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "activated-sacrifice-creature-target-minus-two-minus-two",
            ],
            power: Some(4),
            toughness: Some(3),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: the enter-the-battlefield trigger places a target-free
        // team modifier on the stack and grants both +1/+1 and Haste.
        CardDefinition {
            id: "RAV-FLAME-KIN-ZEALOT",
            name: "Flame-Kin Zealot",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Red, Color::Red, Color::White]),
            colors: colors([Color::Red, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "etb-team-pump-haste",
            ],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: the selected controlled-creature sacrifice is paid
        // atomically before the target player loses life through the stack.
        CardDefinition {
            id: "RAV-GOLGARI-ROTWURM",
            name: "Golgari Rotwurm",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Black, Color::Green]),
            colors: colors([Color::Black, Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "activated-sacrifice-creature-target-player-life-loss",
            ],
            power: Some(5),
            toughness: Some(4),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: combat and noncombat damage from this permanent
        // creates a source-specific stack trigger whose life gain is
        // materialized from the positive damage event amount.
        CardDefinition {
            id: "RAV-SUNHOME-ENFORCER",
            name: "Sunhome Enforcer",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Red, Color::White]),
            colors: colors([Color::Red, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "damage-trigger-life-gain",
            ],
            power: Some(2),
            toughness: Some(4),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: the tapped target ability installs a temporary
        // combat-participation restriction through the shared ability layer.
        CardDefinition {
            id: "RAV-THUNDERSONG-TRUMPETER",
            name: "Thundersong Trumpeter",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Red, Color::White]),
            colors: colors([Color::Red, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "tap-prevent-target-combat",
            ],
            power: Some(2),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: static blocker legality depends on the controller's
        // current Mountain control, checked at declaration time.
        CardDefinition {
            id: "RAV-SABERTOOTH-ALLEY-CAT",
            name: "Sabertooth Alley Cat",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "mountain-required-to-block",
            ],
            power: Some(2),
            toughness: Some(1),
            keywords: vec![Keyword::CannotBlockUnlessControlsMountain],
            effects: vec![],
        },
        // Compatibility scope: normal colored-cost creature casting, base
        // characteristics, and static Flying. Its graveyard-triggered counter
        // behavior is deliberately unsupported.
        CardDefinition {
            id: "RAV-VULTUROUS-ZOMBIE",
            name: "Vulturous Zombie",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Black, Color::Green]),
            colors: colors([Color::Black, Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &["colored-cost-casting", "base-characteristics", "flying"],
            power: Some(3),
            toughness: Some(3),
            keywords: vec![Keyword::Flying],
            effects: vec![],
        },
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
        // Full fidelity: normal colored-cost casting, base characteristics,
        // Flying blocker legality, and vigilance attack declaration are all
        // represented by the shared engine.
        CardDefinition {
            id: "RAV-COURIER-HAWK",
            name: "Courier Hawk",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "flying",
                "vigilance",
            ],
            power: Some(1),
            toughness: Some(2),
            keywords: vec![Keyword::Flying, Keyword::Vigilance],
            effects: vec![],
        },
        // Full fidelity: Convoke payment and resolution-time validation that
        // the target is a creature still attacking or blocking, followed by a
        // normal zone change to exile.
        CardDefinition {
            id: "RAV-DEVOURING-LIGHT",
            name: "Devouring Light",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::White, Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "convoke",
                "attacking-or-blocking-creature-target",
                "exile",
            ],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Convoke],
            effects: vec![Effect::ExileTargetPermanent],
        },
        // Full printed behavior: Flying plus the sacrifice activation that
        // damages an attacking or blocking creature on stack resolution.
        CardDefinition {
            id: "RAV-DIVEBOMBER-GRIFFIN",
            name: "Divebomber Griffin",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::White, Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "flying",
                "sacrifice-source",
                "attacking-or-blocking-creature-target",
                "damage",
            ],
            power: Some(3),
            toughness: Some(2),
            keywords: vec![Keyword::Flying],
            effects: vec![],
        },
        // Full printed behavior: three selected controlled creature taps pay
        // its activation cost and the targeted creature taps on resolution.
        CardDefinition {
            id: "RAV-SANDSOWER",
            name: "Sandsower",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "tap-three-untapped-controlled-creatures",
                "tap-target-creature",
            ],
            power: Some(1),
            toughness: Some(3),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: the green activated regeneration shield uses the
        // expansion-neutral targeted replacement substrate below.
        CardDefinition {
            id: "RAV-VOTARY-OF-THE-CONCLAVE",
            name: "Votary of the Conclave",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "regeneration",
            ],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
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
        // Compatibility scope: normal creature casting, base characteristics,
        // Defender, and the existing immediate hand-zone Transmute operation.
        // This remains deliberately outside the full-fidelity manifest because
        // the shared Transmute substrate does not yet create a stack object or
        // response window for its activated ability.
        CardDefinition {
            id: "RAV-DRIFT-OF-PHANTASMS",
            name: "Drift of Phantasms",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "colored-cost-casting",
                "base-characteristics",
                "defender",
                "immediate-hand-zone-transmute-compatibility",
            ],
            power: Some(0),
            toughness: Some(5),
            keywords: vec![
                Keyword::Defender,
                Keyword::Transmute(ManaCost::with_colors(1, [Color::Blue, Color::Blue])),
            ],
            effects: vec![],
        },
        // Compatibility scope: exact colored casting, base characteristics,
        // Flying, and First Strike. Firemane Angel's source-zone-aware upkeep
        // trigger and graveyard return activation remain unrepresented until
        // the public engine exposes those zone-aware choices.
        CardDefinition {
            id: "RAV-FIREMANE-ANGEL",
            name: "Firemane Angel",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Red, Color::White, Color::White]),
            colors: colors([Color::Red, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "colored-cost-casting",
                "base-characteristics",
                "flying",
                "first-strike",
            ],
            power: Some(4),
            toughness: Some(3),
            keywords: vec![Keyword::Flying, Keyword::FirstStrike],
            effects: vec![],
        },
        // Compatibility scope: normal creature casting, base characteristics,
        // static unblockability, and the existing immediate hand-zone
        // Transmute operation. The shared Transmute substrate deliberately
        // remains outside the full-fidelity manifest because it has no stack
        // object or response window for an activated ability.
        CardDefinition {
            id: "RAV-DIMIR-INFILTRATOR",
            name: "Dimir Infiltrator",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Blue, Color::Black]),
            colors: colors([Color::Blue, Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "colored-cost-casting",
                "base-characteristics",
                "unblockable",
                "immediate-hand-zone-transmute-compatibility",
            ],
            power: Some(1),
            toughness: Some(3),
            keywords: vec![
                Keyword::Unblockable,
                Keyword::Transmute(ManaCost::with_colors(1, [Color::Blue, Color::Black])),
            ],
            effects: vec![],
        },
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
        // Compatibility scope: normal colored-cost creature casting, base
        // characteristics, Defender, and the existing immediate hand-zone
        // Transmute operation. Its entry-triggered library search remains
        // unsupported, and Transmute remains outside the full-fidelity
        // manifest because the shared substrate creates no stack object or
        // response window for the activated ability.
        CardDefinition {
            id: "RAV-GROZOTH",
            name: "Grozoth",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(6, [Color::Blue, Color::Blue, Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "colored-cost-casting",
                "base-characteristics",
                "defender",
                "immediate-hand-zone-transmute-compatibility",
            ],
            power: Some(9),
            toughness: Some(9),
            keywords: vec![
                Keyword::Defender,
                Keyword::Transmute(ManaCost::with_colors(1, [Color::Blue, Color::Blue])),
            ],
            effects: vec![],
        },
        // Complete supported slice: ordinary colored-cost creature casting,
        // first-strike combat damage, and vigilance's no-tap attack exception
        // account for every printed functional behavior.
        CardDefinition {
            id: "RAV-NIGHTGUARD-PATROL",
            name: "Nightguard Patrol",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "first-strike",
                "vigilance",
            ],
            power: Some(2),
            toughness: Some(1),
            keywords: vec![Keyword::FirstStrike, Keyword::Vigilance],
            effects: vec![],
        },
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
        // Full fidelity: both targeted guild abilities use the shared stack
        // and layer-6 keyword-grant substrate for their chosen color cost.
        CardDefinition {
            id: "RAV-BOROS-GUILDMAGE",
            name: "Boros Guildmage",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Red, Color::White]),
            colors: colors([Color::Red, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "activated-grant-haste",
                "activated-grant-first-strike",
            ],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: the tap ability uses the same resolution-time
        // radiance damage batch as Cleansing Beam, with one damage per
        // selected creature in the shared-color set.
        CardDefinition {
            id: "RAV-WOJEK-EMBERMAGE",
            name: "Wojek Embermage",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "tap-radiance-one-damage",
            ],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: the target-bearing tap ability uses the shared
        // resolution-time Radiance shield substrate. It snapshots the target
        // plus every current creature sharing its color and creates one
        // independently consumable prevention shield for each recipient.
        CardDefinition {
            id: "RAV-WOJEK-APOTHECARY",
            name: "Wojek Apothecary",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::White, Color::White]),
            colors: colors([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "tap-radiance-prevent-one-damage",
            ],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
        signet_definition("RAV-BOROS-SIGNET", "Boros Signet"),
        signet_definition("RAV-DIMIR-SIGNET", "Dimir Signet"),
        signet_definition("RAV-GOLGARI-SIGNET", "Golgari Signet"),
        signet_definition("RAV-SELESNYA-SIGNET", "Selesnya Signet"),
        guild_bounce_land("RAV-BOROS-GARRISON", "Boros Garrison"),
        guild_bounce_land("RAV-DIMIR-AQUEDUCT", "Dimir Aqueduct"),
        guild_bounce_land("RAV-GOLGARI-ROT-FARM", "Golgari Rot Farm"),
        guild_bounce_land("RAV-SELESNYA-SANCTUARY", "Selesnya Sanctuary"),
        // Full fidelity: the land has its colorless mana ability and its
        // stack-backed, targeted Double Strike grant. Both use the shared
        // mana and continuous-effect substrates.
        CardDefinition {
            id: "RAV-SUNHOME-FORTRESS",
            name: "Sunhome, Fortress of the Legion",
            set_code: SET_CODE,
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::from([Color::Colorless]),
            card_types: types([CardType::Land]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colorless-mana-ability",
                "activated-double-strike-grant",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: the land's intrinsic colorless mana ability and
        // stack-backed green Saproling creation both use typed shared rules.
        CardDefinition {
            id: "RAV-VITU-GHAZI",
            name: "Vitu-Ghazi, the City-Tree",
            set_code: SET_CODE,
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::from([Color::Colorless]),
            card_types: types([CardType::Land]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colorless-mana-ability",
                "activated-green-saproling-token",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        basic_land("RAV-PLAINS", "Plains", BasicLandType::Plains),
        basic_land("RAV-ISLAND", "Island", BasicLandType::Island),
        basic_land("RAV-SWAMP", "Swamp", BasicLandType::Swamp),
        basic_land("RAV-MOUNTAIN", "Mountain", BasicLandType::Mountain),
        basic_land("RAV-FOREST", "Forest", BasicLandType::Forest),
    ]
}

/// Definition-bound RAV mana abilities used by the shown mana-development scenarios.
///
/// Each Signet binding pays one mana, taps the artifact, and adds its two fixed
/// guild colors. The shared engine owns cost payment, atomically emitted
/// receipts, priority retention, payment-context activation during a spell
/// cast, and the fact that a mana ability does not use the stack.
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
        ManaAbilityBinding {
            card_definition: "RAV-SUNHOME-FORTRESS",
            ability: ActivatedManaAbility {
                id: "produce-colorless",
                tap_cost: true,
                output: ManaAbilityOutput::Fixed(Color::Colorless),
                amount: 1,
                life_payment: None,
                controller_damage: None,
            },
        },
        ManaAbilityBinding {
            card_definition: "RAV-VITU-GHAZI",
            ability: ActivatedManaAbility {
                id: "produce-colorless",
                tap_cost: true,
                output: ManaAbilityOutput::Fixed(Color::Colorless),
                amount: 1,
                life_payment: None,
                controller_damage: None,
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
        guild_bounce_land_binding(
            "RAV-BOROS-GARRISON",
            "boros-garrison-rw",
            [Color::Red, Color::White],
        ),
        guild_bounce_land_binding(
            "RAV-DIMIR-AQUEDUCT",
            "dimir-aqueduct-ub",
            [Color::Blue, Color::Black],
        ),
        guild_bounce_land_binding(
            "RAV-GOLGARI-ROT-FARM",
            "golgari-rot-farm-bg",
            [Color::Black, Color::Green],
        ),
        guild_bounce_land_binding(
            "RAV-SELESNYA-SANCTUARY",
            "selesnya-sanctuary-gw",
            [Color::Green, Color::White],
        ),
    ]
}

/// RAV lands that enter tapped. Their return instruction is represented below
/// as an ordinary ETB trigger rather than a land-play replacement, so the
/// trigger uses the stack and can legally return the newly entered land.
#[must_use]
pub fn rav_land_entry_bindings() -> Vec<LandEntryBinding> {
    [
        "RAV-BOROS-GARRISON",
        "RAV-DIMIR-AQUEDUCT",
        "RAV-GOLGARI-ROT-FARM",
        "RAV-SELESNYA-SANCTUARY",
    ]
    .into_iter()
    .map(|card_definition| LandEntryBinding {
        card_definition,
        enters_tapped: true,
    })
    .collect()
}

/// Stack-using activated abilities for the executable RAV slice. Costs and
/// effects are semantic data; the engine owns priority, payment, target
/// legality, and resolution receipts.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn rav_activated_ability_bindings() -> Vec<ActivatedAbilityBinding> {
    vec![
        ActivatedAbilityBinding {
            card_definition: "RAV-FLICKERFORM",
            ability: ActivatedAbility {
                id: "linked-exile-attached-creature-and-auras",
                mana_cost: ManaCost::with_colors(2, [Color::White, Color::White]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::ExileAttachedCreatureAndAurasUntilEndStep],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-TIDEWATER-MINION",
            ability: ActivatedAbility {
                id: "tap-untap-target-permanent",
                mana_cost: ManaCost::new(0),
                tap_cost: true,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::Permanent],
                effects: vec![Effect::UntapTargetPermanent],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-TIDEWATER-MINION",
            ability: ActivatedAbility {
                id: "blue-lose-defender",
                mana_cost: ManaCost::with_colors(0, [Color::Blue]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::RemoveSourceKeywordUntilEndOfTurn {
                    keyword: Keyword::Defender,
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-VEDALKEN-ENTRANCER",
            ability: ActivatedAbility {
                id: "tap-blue-mill-two",
                mana_cost: ManaCost::with_colors(0, [Color::Blue]),
                tap_cost: true,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::Player],
                effects: vec![Effect::MillTargetPlayer { count: 2 }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-IVY-DANCER",
            ability: ActivatedAbility {
                id: "tap-target-creature-grant-forestwalk",
                mana_cost: ManaCost::new(0),
                tap_cost: true,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::Creature],
                effects: vec![Effect::ModifyTargetKeywordUntilEndOfTurn {
                    keyword: Keyword::Landwalk(BasicLandType::Forest),
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-DOWSING-SHAMAN",
            ability: ActivatedAbility {
                id: "return-target-enchantment-from-graveyard",
                mana_cost: ManaCost::with_colors(2, [Color::Green]),
                tap_cost: true,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::EnchantmentCardInControllerGraveyard],
                effects: vec![Effect::ReturnTargetEnchantmentCardToHand],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-THOUGHTPICKER-WITCH",
            ability: ActivatedAbility {
                id: "sacrifice-creature-private-opponent-top-two-exile",
                mana_cost: ManaCost::new(1),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 1,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::Opponent],
                effects: vec![Effect::LookAtTopCardsOfTargetOpponentExileOne { count: 2 }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-GOLGARI-GUILDMAGE",
            ability: ActivatedAbility {
                id: "target-pump-and-trample",
                mana_cost: ManaCost::with_colors(0, [Color::Black, Color::Green]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::Creature],
                effects: vec![Effect::ModifyTargetPtAndKeywordUntilEndOfTurn {
                    power: 1,
                    toughness: 1,
                    keyword: Keyword::Trample,
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-GOLGARI-GUILDMAGE",
            ability: ActivatedAbility {
                id: "regenerate-target-creature",
                mana_cost: ManaCost::with_colors(2, [Color::Black, Color::Green]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::Creature],
                effects: vec![Effect::RegenerateTargetCreature],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-GOLGARI-ROTWURM",
            ability: ActivatedAbility {
                id: "sacrifice-creature-target-player-life-loss",
                mana_cost: ManaCost::with_colors(0, [Color::Black]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 1,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::Player],
                effects: vec![Effect::LoseLifeTarget { amount: 1 }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-DROOLING-GROODION",
            ability: ActivatedAbility {
                id: "sacrifice-creature-target-minus-two-minus-two",
                mana_cost: ManaCost::with_colors(0, [Color::Black, Color::Green]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 1,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::Creature],
                effects: vec![Effect::ModifyTargetPtUntilEndOfTurn {
                    power: -2,
                    toughness: -2,
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-UNDERCITY-SHADE",
            ability: ActivatedAbility {
                id: "pump-plus-one-plus-one",
                mana_cost: ManaCost::with_colors(0, [Color::Black]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::ModifySourcePtUntilEndOfTurn {
                    power: 1,
                    toughness: 1,
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-URSAPINE",
            ability: ActivatedAbility {
                id: "ursapine-target-pump",
                mana_cost: ManaCost::with_colors(0, [Color::Green]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::Creature],
                effects: vec![Effect::ModifyTargetPtUntilEndOfTurn {
                    power: 1,
                    toughness: 1,
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-TWILIGHT-DROVER",
            ability: ActivatedAbility {
                id: "create-flying-spirit",
                mana_cost: ManaCost::with_colors(2, [Color::White]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::CreateToken {
                    token: TokenSpec::white_spirit(),
                    count: 1,
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-ELVISH-SKYSWEEPER",
            ability: ActivatedAbility {
                id: "sacrifice-creature-destroy-flying",
                mana_cost: ManaCost::with_colors(4, [Color::Green]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 1,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::FlyingCreature],
                effects: vec![Effect::DestroyTargetFlyingCreature],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-SHAMBLING-SHELL",
            ability: ActivatedAbility {
                id: "shambling-shell-sacrifice-counter",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: true,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::Creature],
                effects: vec![Effect::AddPlusOneCounterToTarget],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-GRAVE-SHELL-SCARAB",
            ability: ActivatedAbility {
                id: "sacrifice-source-draw",
                mana_cost: ManaCost::with_colors(1, [Color::Black]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: true,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::DrawController],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-VOTARY-OF-THE-CONCLAVE",
            ability: ActivatedAbility {
                id: "regenerate-target-creature",
                mana_cost: ManaCost::with_colors(0, [Color::Green]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::Creature],
                effects: vec![Effect::RegenerateTargetCreature],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-SEWERDREG",
            ability: ActivatedAbility {
                id: "self-regeneration",
                mana_cost: ManaCost::with_colors(0, [Color::Black]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::RegenerateSource],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-TATTERED-DRAKE",
            ability: ActivatedAbility {
                id: "self-regeneration",
                mana_cost: ManaCost::with_colors(0, [Color::Black]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::RegenerateSource],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-CERULEAN-SPHINX",
            ability: ActivatedAbility {
                id: "shuffle-source-into-owner-library",
                mana_cost: ManaCost::with_colors(0, [Color::Blue]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::MoveSourceToOwnersLibraryAndShuffle],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-HUNTED-TROLL",
            ability: ActivatedAbility {
                id: "self-regeneration",
                mana_cost: ManaCost::with_colors(0, [Color::Green]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::RegenerateSource],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-SANDSOWER",
            ability: ActivatedAbility {
                id: "tap-target-creature",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 3,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::Creature],
                effects: vec![Effect::TapTargetCreature],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-CYCLOPEAN-SNARE",
            ability: ActivatedAbility {
                id: "tap-target-creature",
                mana_cost: ManaCost::new(3),
                tap_cost: true,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::Creature],
                effects: vec![Effect::TapTargetCreature, Effect::ReturnSourceToOwnersHand],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-VEDALKEN-ENTRANCER",
            ability: ActivatedAbility {
                id: "tap-blue-target-player-mill-two",
                mana_cost: ManaCost::with_colors(0, [Color::Blue]),
                tap_cost: true,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::Player],
                effects: vec![Effect::MillTargetPlayer { count: 2 }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-GRIFTERS-BLADE",
            ability: ActivatedAbility {
                id: "equip-plus-one-plus-one",
                mana_cost: ManaCost::new(1),
                tap_cost: false,
                sorcery_speed: true,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::ControlledCreature],
                effects: vec![Effect::AttachSourceToTarget {
                    target: TargetRequirement::ControlledCreature,
                    changes: grifters_blade_attachment_changes(),
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-NULLMAGE-SHEPHERD",
            ability: ActivatedAbility {
                id: "destroy-artifact-or-enchantment",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 4,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::ArtifactOrEnchantment],
                effects: vec![Effect::DestroyTargetArtifactOrEnchantment],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-STONE-SEEDER-HIEROPHANT",
            ability: ActivatedAbility {
                id: "untap-target-land",
                mana_cost: ManaCost::new(0),
                tap_cost: true,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::Land],
                effects: vec![Effect::UntapTargetLand],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-PERILOUS-FORAYS",
            ability: ActivatedAbility {
                id: "sacrifice-creature-search-basic-land",
                mana_cost: ManaCost::new(1),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 1,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::SearchControllerLibrary {
                    requirement: LibrarySearchRequirement::BasicLandTypes(BTreeSet::from([
                        BasicLandType::Plains,
                        BasicLandType::Island,
                        BasicLandType::Swamp,
                        BasicLandType::Mountain,
                        BasicLandType::Forest,
                    ])),
                    destination: LibrarySearchDestination::BattlefieldTapped,
                    selection: LibrarySearchSelection::DeterministicFirstMatch,
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-SELESNYA-EVANGEL",
            ability: ActivatedAbility {
                id: "create-saproling",
                mana_cost: ManaCost::new(1),
                tap_cost: true,
                sorcery_speed: false,
                additional_tap_creatures: 1,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::CreateToken {
                    token: TokenSpec::saproling(),
                    count: 1,
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-GOBLIN-FIRE-FIEND",
            ability: ActivatedAbility {
                id: "pump-plus-one-power",
                mana_cost: ManaCost::with_colors(0, [Color::Red]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::ModifySourcePtUntilEndOfTurn {
                    power: 1,
                    toughness: 0,
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-GREATER-FORGELING",
            ability: ActivatedAbility {
                id: "pump-plus-three-minus-three",
                mana_cost: ManaCost::with_colors(1, [Color::Red]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::ModifySourcePtUntilEndOfTurn {
                    power: 3,
                    toughness: -3,
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-VIASHINO-SLASHER",
            ability: ActivatedAbility {
                id: "pump-plus-one-minus-one",
                mana_cost: ManaCost::with_colors(0, [Color::Red]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 1,
                targets: vec![],
                effects: vec![Effect::ModifySourcePtUntilEndOfTurn {
                    power: 1,
                    toughness: -1,
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-DIVEBOMBER-GRIFFIN",
            ability: ActivatedAbility {
                id: "sacrifice-deal-three-to-attacker-or-blocker",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: true,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![
                    cardbench_magic_engine::TargetRequirement::AttackingOrBlockingCreature,
                ],
                effects: vec![Effect::DealDamage {
                    amount: 3,
                    target: cardbench_magic_engine::TargetRequirement::AttackingOrBlockingCreature,
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-WAR-TORCH-GOBLIN",
            ability: ActivatedAbility {
                id: "sacrifice-deal-two-to-blocker",
                mana_cost: ManaCost::with_colors(0, [Color::Red]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: true,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::BlockingCreature],
                effects: vec![Effect::DealDamage {
                    amount: 2,
                    target: cardbench_magic_engine::TargetRequirement::BlockingCreature,
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-CAREGIVER",
            ability: ActivatedAbility {
                id: "prevent-one-damage",
                mana_cost: ManaCost::with_colors(0, [Color::White]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: true,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::PlayerOrCreature],
                effects: vec![Effect::AddTargetDamageShieldUntilEndOfTurn { amount: 1 }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-SELESNYA-GUILDMAGE",
            ability: ActivatedAbility {
                id: "create-green-centaur",
                mana_cost: ManaCost::with_colors(3, [Color::Green]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::CreateToken {
                    token: TokenSpec::green_centaur(),
                    count: 1,
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-SELESNYA-GUILDMAGE",
            ability: ActivatedAbility {
                id: "anthem-controller-creatures",
                mana_cost: ManaCost::with_colors(3, [Color::White]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::ModifyControllerCreaturesPtUntilEndOfTurn {
                    power: 1,
                    toughness: 1,
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-VIASHINO-FANGTAIL",
            ability: ActivatedAbility {
                id: "tap-deal-one-to-player-or-creature",
                mana_cost: ManaCost::new(0),
                tap_cost: true,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::PlayerOrCreature],
                effects: vec![Effect::DealDamage {
                    amount: 1,
                    target: cardbench_magic_engine::TargetRequirement::PlayerOrCreature,
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-ORDRUUN-COMMANDO",
            ability: ActivatedAbility {
                id: "prevent-one-damage-to-self",
                mana_cost: ManaCost::with_colors(0, [Color::White]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::AddSourceDamageShieldUntilEndOfTurn { amount: 1 }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-BOROS-GUILDMAGE",
            ability: ActivatedAbility {
                id: "grant-haste",
                mana_cost: ManaCost::with_colors(0, [Color::Red]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::Creature],
                effects: vec![Effect::ModifyTargetKeywordUntilEndOfTurn {
                    keyword: Keyword::Haste,
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-SUNHOME-FORTRESS",
            ability: ActivatedAbility {
                id: "grant-target-double-strike",
                mana_cost: ManaCost::with_colors(3, [Color::Red, Color::White]),
                tap_cost: true,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::Creature],
                effects: vec![Effect::ModifyTargetKeywordUntilEndOfTurn {
                    keyword: Keyword::DoubleStrike,
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-VITU-GHAZI",
            ability: ActivatedAbility {
                id: "create-green-saproling",
                mana_cost: ManaCost::with_colors(2, [Color::Green, Color::White]),
                tap_cost: true,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::CreateToken {
                    token: TokenSpec::saproling(),
                    count: 1,
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-WOJEK-EMBERMAGE",
            ability: ActivatedAbility {
                id: "tap-radiance-one-damage",
                mana_cost: ManaCost::new(0),
                tap_cost: true,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::Creature],
                effects: vec![Effect::RadianceDealDamageToCreatures { amount: 1 }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-WOJEK-APOTHECARY",
            ability: ActivatedAbility {
                id: "tap-radiance-prevent-one-damage",
                mana_cost: ManaCost::new(0),
                tap_cost: true,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::Creature],
                effects: vec![Effect::RadianceAddTargetDamageShieldUntilEndOfTurn { amount: 1 }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-THUNDERSONG-TRUMPETER",
            ability: ActivatedAbility {
                id: "tap-prevent-target-combat",
                mana_cost: ManaCost::new(0),
                tap_cost: true,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::Creature],
                effects: vec![Effect::ModifyTargetKeywordUntilEndOfTurn {
                    keyword: Keyword::CannotAttackOrBlock,
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-SCREECHING-GRIFFIN",
            ability: ActivatedAbility {
                id: "prevent-target-blocking-griffin",
                mana_cost: ManaCost::with_colors(0, [Color::Red]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::Creature],
                effects: vec![Effect::PreventTargetBlockingSourceUntilEndOfTurn],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-RAZIA-BOROS-ARCHANGEL",
            ability: ActivatedAbility {
                id: "tap-redirect-three-damage",
                mana_cost: ManaCost::new(0),
                tap_cost: true,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![
                    cardbench_magic_engine::TargetRequirement::Creature,
                    cardbench_magic_engine::TargetRequirement::PlayerOrCreature,
                ],
                effects: vec![
                    Effect::BeginDamageRedirection { amount: 3 },
                    Effect::CompleteDamageRedirection,
                ],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-HAMMERFIST-GIANT",
            ability: ActivatedAbility {
                id: "tap-global-nonflying-damage",
                mana_cost: ManaCost::new(0),
                tap_cost: true,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::DealDamageToEachNonFlyingCreature { amount: 4 }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-BOROS-GUILDMAGE",
            ability: ActivatedAbility {
                id: "grant-first-strike",
                mana_cost: ManaCost::with_colors(0, [Color::White]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::Creature],
                effects: vec![Effect::ModifyTargetKeywordUntilEndOfTurn {
                    keyword: Keyword::FirstStrike,
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-TORPID-MOLOCH",
            ability: ActivatedAbility {
                id: "sacrifice-three-lands-remove-defender",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 3,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::RemoveSourceKeywordUntilEndOfTurn {
                    keyword: Keyword::Defender,
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-TERRAFORMER",
            ability: ActivatedAbility {
                id: "choose-controller-land-basic-type",
                mana_cost: ManaCost::with_colors(1, []),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::ReplaceControllerLandsWithChosenBasicLandTypeUntilEndOfTurn],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-HELLDOZER",
            ability: ActivatedAbility {
                id: "destroy-target-land",
                mana_cost: ManaCost::with_colors(0, [Color::Black, Color::Black, Color::Black]),
                tap_cost: true,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::Land],
                effects: vec![Effect::DestroyTargetLandAndUntapSourceIfNonbasic],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-BARBARIAN-RIFTCUTTER",
            ability: ActivatedAbility {
                id: "sacrifice-destroy-target-land",
                mana_cost: ManaCost::with_colors(0, [Color::Red]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: true,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::Land],
                effects: vec![Effect::DestroyTargetLand],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-DIMIR-GUILDMAGE",
            ability: ActivatedAbility {
                id: "target-player-draw",
                mana_cost: ManaCost::with_colors(3, [Color::Blue]),
                tap_cost: false,
                sorcery_speed: true,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::Player],
                effects: vec![Effect::DrawTargetPlayer],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-DIMIR-GUILDMAGE",
            ability: ActivatedAbility {
                id: "target-player-discard",
                mana_cost: ManaCost::with_colors(3, [Color::Black]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::Player],
                effects: vec![Effect::DiscardTargetPlayer { count: 1 }],
            },
        },
    ]
}

fn grifters_blade_attachment_changes() -> Vec<ContinuousChange> {
    vec![ContinuousChange::ModifyPowerToughness {
        power: 1,
        toughness: 1,
    }]
}

/// Explicit persistent-attachment metadata for the RAV Equipment slice.
///
/// The activated ability above owns target ordering and stack resolution;
/// this registry makes the ongoing attachment and its layer-seven modifier
/// auditable by the expansion-neutral Equipment lifecycle.
#[must_use]
pub fn rav_attachment_bindings() -> Vec<AttachmentBinding> {
    vec![AttachmentBinding {
        card_definition: "RAV-GRIFTERS-BLADE",
        kind: AttachmentKind::Equipment,
        target: TargetRequirement::ControlledCreature,
        changes: grifters_blade_attachment_changes(),
    }]
}

/// Battlefield-only static characteristic bindings supplied by the RAV set.
/// These do not create events or stack objects; the engine evaluates them
/// whenever a caller reads a permanent's characteristics.
#[must_use]
pub fn rav_static_continuous_effect_bindings() -> Vec<StaticContinuousEffectBinding> {
    vec![
        StaticContinuousEffectBinding {
            card_definition: "RAV-SCION-OF-THE-WILD",
            change: cardbench_magic_engine::ContinuousChange::ControlledCreatureCountPowerToughness,
        },
        StaticContinuousEffectBinding {
            card_definition: "RAV-OATHSWORN-GIANT",
            change: cardbench_magic_engine::ContinuousChange::OtherControlledCreaturesModifyPowerToughness {
                power: 0,
                toughness: 2,
            },
        },
        StaticContinuousEffectBinding {
            card_definition: "RAV-OATHSWORN-GIANT",
            change: cardbench_magic_engine::ContinuousChange::OtherControlledCreaturesAddKeyword(
                Keyword::Vigilance,
            ),
        },
        StaticContinuousEffectBinding {
            card_definition: "RAV-VETERAN-ARMORER",
            change: cardbench_magic_engine::ContinuousChange::OtherControlledCreaturesModifyPowerToughness {
                power: 0,
                toughness: 1,
            },
        },
        StaticContinuousEffectBinding {
            card_definition: "RAV-GATE-HOUND",
            change: cardbench_magic_engine::ContinuousChange::ControlledCreaturesAddKeywordIfSourceEnchanted(
                Keyword::Vigilance,
            ),
        },
        StaticContinuousEffectBinding {
            card_definition: "RAV-LIGHT-OF-SANCTION",
            change: cardbench_magic_engine::ContinuousChange::ControlledCreaturesAddKeyword(
                Keyword::PreventDamageFromControlledSources,
            ),
        },
    ]
}

/// Battlefield-only static attack restrictions supplied by the RAV set.
/// These bindings are checked before attacker state changes and never create a
/// stack object or synthetic event receipt.
#[must_use]
pub fn rav_static_attack_restriction_bindings() -> Vec<StaticAttackRestrictionBinding> {
    vec![StaticAttackRestrictionBinding {
        card_definition: "RAV-BLAZING-ARCHON",
        restriction: StaticAttackRestriction::OpponentsCannotAttackController,
    }]
}

/// Battlefield-only entry replacements supplied by RAV. Unlike ETB
/// triggers, the engine applies these during the ordinary zone transition and
/// emits source-incarnation receipt provenance when they change an entry.
#[must_use]
pub fn rav_static_entry_restriction_bindings() -> Vec<StaticEntryRestrictionBinding> {
    vec![StaticEntryRestrictionBinding {
        card_definition: "RAV-LOXODON-GATEKEEPER",
        restriction: StaticEntryRestriction::OpponentsArtifactsCreaturesAndLandsEnterTapped,
    }]
}

/// Target-free stack triggers bound to RAV permanents.
#[must_use]
#[allow(clippy::too_many_lines)] // Keep the declarative trigger registry centralized for audit review.
pub fn rav_triggered_ability_bindings() -> Vec<TriggeredAbilityBinding> {
    vec![
        TriggeredAbilityBinding {
            card_definition: "RAV-AURATOUCHED-MAGE",
            ability: TriggeredAbility {
                id: "etb-search-and-attach-aura",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![
                    Effect::SearchControllerLibraryForFirstCompatibleAuraAttachedToSource,
                ],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-FISTS-OF-IRONWOOD",
            ability: TriggeredAbility {
                id: "etb-two-saprolings",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::CreateToken {
                    token: TokenSpec::saproling(),
                    count: 2,
                }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-FLIGHT-OF-FANCY",
            ability: TriggeredAbility {
                id: "etb-draw-two",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::DrawController, Effect::DrawController],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-MARK-OF-EVICTION",
            ability: TriggeredAbility {
                id: "upkeep-return-enchanted-creature",
                condition: TriggerCondition::BeginningOfUpkeep,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::ReturnSourceAttachedPermanentToHand],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-FAITHS-FETTERS",
            ability: TriggeredAbility {
                id: "etb-gain-four-life",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::GainLifeController { amount: 4 }],
            },
        },
        // This ordinary target-bearing ETB trigger uses the same Equipment
        // attachment lifecycle as an Equip activation. If no controlled
        // creature is legal when it would be stacked, the generic trigger
        // scheduler leaves the Blade unattached rather than inventing a
        // target or skipping its ordinary permanent entry.
        TriggeredAbilityBinding {
            card_definition: "RAV-GRIFTERS-BLADE",
            ability: TriggeredAbility {
                id: "etb-attach-to-controlled-creature",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![TargetRequirement::ControlledCreature],
                effects: vec![Effect::AttachSourceToTarget {
                    target: TargetRequirement::ControlledCreature,
                    changes: grifters_blade_attachment_changes(),
                }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-BLOOD-FUNNEL",
            ability: TriggeredAbility {
                id: "cast-sacrifice-or-counter",
                condition: TriggerCondition::CastsNoncreatureSpell,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![cardbench_magic_engine::TargetRequirement::NoncreatureSpell],
                effects: vec![Effect::SacrificeCreatureOrCounterTargetSpell],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-MAUSOLEUM-TURNKEY",
            ability: TriggeredAbility {
                id: "conditional-graveyard-return",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![
                    cardbench_magic_engine::TargetRequirement::CreatureCardInControllerGraveyard,
                ],
                effects: vec![Effect::ReturnTargetCreatureCardToHandIfAnotherInControllerGraveyard],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-STONE-SEEDER-HIEROPHANT",
            ability: TriggeredAbility {
                id: "landfall-untap-source",
                condition: TriggerCondition::LandEntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::UntapSource],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-VINELASHER-KUDZU",
            ability: TriggeredAbility {
                id: "controller-landfall-plus-one-counter",
                condition: TriggerCondition::ControlledLandEntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::AddPlusOneCounterToSource],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-CIVIC-WAYFINDER",
            ability: TriggeredAbility {
                id: "etb-search-basic-land-to-hand",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::SearchControllerLibrary {
                    requirement: LibrarySearchRequirement::BasicLandTypes(BTreeSet::from([
                        BasicLandType::Plains,
                        BasicLandType::Island,
                        BasicLandType::Swamp,
                        BasicLandType::Mountain,
                        BasicLandType::Forest,
                    ])),
                    destination: LibrarySearchDestination::Hand,
                    selection: LibrarySearchSelection::DeterministicFirstMatch,
                }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-BOROS-GARRISON",
            ability: guild_bounce_land_trigger(),
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-DIMIR-AQUEDUCT",
            ability: guild_bounce_land_trigger(),
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-GOLGARI-ROT-FARM",
            ability: guild_bounce_land_trigger(),
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-SELESNYA-SANCTUARY",
            ability: guild_bounce_land_trigger(),
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-DARK-CONFIDANT",
            ability: TriggeredAbility {
                id: "upkeep-reveal-mana-value-life-loss",
                condition: TriggerCondition::BeginningOfUpkeep,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::RevealTopCardPutIntoHandLoseLifeEqualToManaValue],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-NETHERBORN-PHALANX",
            ability: TriggeredAbility {
                id: "etb-opponent-creature-count-life-loss",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::LoseLifeEachOpponentEqualToControlledCreatures],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-MOROII",
            ability: TriggeredAbility {
                id: "upkeep-lose-one-life",
                condition: TriggerCondition::BeginningOfUpkeep,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::LoseLifeController { amount: 1 }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-SEARING-MEDITATION",
            ability: TriggeredAbility {
                id: "life-gain-deal-two",
                condition: TriggerCondition::LifeGained,
                mana_cost: ManaCost::with_colors(2, []),
                optional: true,
                targets: vec![],
                effects: vec![Effect::DealDamageAfterOptionalManaPayment {
                    amount: 2,
                    target: cardbench_magic_engine::TargetRequirement::PlayerOrCreature,
                }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-CARVEN-CARYATID",
            ability: TriggeredAbility {
                id: "etb-draw-controller",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::DrawController],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-SPARKMAGE-APPRENTICE",
            ability: TriggeredAbility {
                id: "etb-deal-one",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![cardbench_magic_engine::TargetRequirement::PlayerOrCreature],
                effects: vec![Effect::DealDamage {
                    amount: 1,
                    target: cardbench_magic_engine::TargetRequirement::PlayerOrCreature,
                }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-HUNTED-DRAGON",
            ability: TriggeredAbility {
                id: "etb-opponent-knights",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![cardbench_magic_engine::TargetRequirement::Player],
                effects: vec![Effect::CreateTokenForTargetPlayer {
                    token: TokenSpec::knight(),
                    count: 3,
                }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-HUNTED-LAMMASU",
            ability: TriggeredAbility {
                id: "etb-opponent-horror",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![cardbench_magic_engine::TargetRequirement::Player],
                effects: vec![Effect::CreateTokenForTargetPlayer {
                    token: TokenSpec::horror(),
                    count: 1,
                }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-HUNTED-HORROR",
            ability: TriggeredAbility {
                id: "etb-opponent-centaurs",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![cardbench_magic_engine::TargetRequirement::Player],
                effects: vec![Effect::CreateTokenForTargetPlayer {
                    token: TokenSpec::hunted_centaur(),
                    count: 2,
                }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-HUNTED-TROLL",
            ability: TriggeredAbility {
                id: "etb-opponent-faeries",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![cardbench_magic_engine::TargetRequirement::Player],
                effects: vec![Effect::CreateTokenForTargetPlayer {
                    token: TokenSpec::blue_faerie(),
                    count: 4,
                }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-HUNTED-PHANTASM",
            ability: TriggeredAbility {
                id: "etb-opponent-goblins",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![cardbench_magic_engine::TargetRequirement::Opponent],
                effects: vec![Effect::CreateTokenForTargetOpponent {
                    token: TokenSpec::red_goblin(),
                    count: 5,
                }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-KEENING-BANSHEE",
            ability: TriggeredAbility {
                id: "etb-target-minus-two",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![cardbench_magic_engine::TargetRequirement::Creature],
                effects: vec![Effect::ModifyTargetPtUntilEndOfTurn {
                    power: -2,
                    toughness: -2,
                }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-VEDALKEN-DISMISSER",
            ability: TriggeredAbility {
                id: "etb-target-creature-owner-library-top",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![cardbench_magic_engine::TargetRequirement::Creature],
                effects: vec![Effect::PutTargetCreatureOnOwnersLibraryTop],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-INFECTIOUS-HOST",
            ability: TriggeredAbility {
                id: "dies-target-player-life-loss",
                condition: TriggerCondition::Dies,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![cardbench_magic_engine::TargetRequirement::Player],
                effects: vec![Effect::LoseLifeTarget { amount: 2 }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-TRANSLUMINANT",
            ability: TriggeredAbility {
                id: "dies-create-flying-spirit",
                condition: TriggerCondition::Dies,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::CreateToken {
                    token: TokenSpec::white_spirit(),
                    count: 1,
                }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-TWILIGHT-DROVER",
            ability: TriggeredAbility {
                id: "another-creature-leaves-plus-one-counter",
                condition: TriggerCondition::AnotherCreatureLeavesBattlefield,
                mana_cost: ManaCost::new(0),
                optional: true,
                targets: vec![],
                effects: vec![Effect::AddPlusOneCounterToSource],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-SURVEILLING-SPRITE",
            ability: TriggeredAbility {
                id: "dies-draw-controller",
                condition: TriggerCondition::Dies,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::DrawController],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-ZEPHYR-SPIRIT",
            ability: TriggeredAbility {
                id: "blocks-return-source-owner-hand",
                condition: TriggerCondition::Blocks,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::ReturnSourceToOwnersHand],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-FLAME-KIN-ZEALOT",
            ability: TriggeredAbility {
                id: "etb-team-pump-haste",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![
                    Effect::ModifyControllerCreaturesPtUntilEndOfTurn {
                        power: 1,
                        toughness: 1,
                    },
                    Effect::AddKeywordToControllerCreaturesUntilEndOfTurn {
                        keyword: Keyword::Haste,
                    },
                ],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-FRENZIED-GOBLIN",
            ability: TriggeredAbility {
                id: "attack-cannot-block",
                condition: TriggerCondition::Attacks,
                mana_cost: ManaCost::with_colors(0, [Color::Red]),
                optional: true,
                targets: vec![cardbench_magic_engine::TargetRequirement::Creature],
                effects: vec![Effect::ModifyTargetKeywordUntilEndOfTurn {
                    keyword: Keyword::CannotBlock,
                }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-DROMAD-PUREBRED",
            ability: TriggeredAbility {
                id: "damage-gain-one-life",
                condition: TriggerCondition::ReceivesDamage,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::GainLifeController { amount: 1 }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-CENTAUR-SAFEGUARD",
            ability: TriggeredAbility {
                id: "dies-may-gain-three-life",
                condition: TriggerCondition::Dies,
                mana_cost: ManaCost::new(0),
                optional: true,
                targets: vec![],
                effects: vec![Effect::GainLifeController { amount: 3 }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-SUNHOME-ENFORCER",
            ability: TriggeredAbility {
                id: "damage-life-gain",
                condition: TriggerCondition::DealsDamage,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::GainLifeControllerFromSourceDamage],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-COALHAULER-SWINE",
            ability: TriggeredAbility {
                id: "damage-each-player",
                condition: TriggerCondition::ReceivesDamage,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::DealDamageToEachPlayerFromReceivedDamage],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-BELLTOWER-SPHINX",
            ability: TriggeredAbility {
                id: "damage-source-controller-mill-that-many",
                condition: TriggerCondition::ReceivesDamage,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::MillSourceControllerFromSourceDamage],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-SELL-SWORD-BRUTE",
            ability: TriggeredAbility {
                id: "dies-deal-two-to-controller",
                condition: TriggerCondition::Dies,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::DealDamageController { amount: 2 }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-SADISTIC-AUGERMAGE",
            ability: TriggeredAbility {
                id: "another-creature-dies-each-player-discards",
                condition: TriggerCondition::AnotherCreatureDies,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::DiscardOneCardEachPlayer],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-VINDICTIVE-MOB",
            ability: TriggeredAbility {
                id: "etb-sacrifice-controller-creature",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::SacrificeControllerCreature],
            },
        },
    ]
}

/// Typed basic-land type lines for the five RAV basic-land definitions.
///
/// The engine validates that every binding names a basic land and that its
/// intrinsic one-color mana ability agrees with the registered type.
#[must_use]
pub fn rav_basic_land_type_bindings() -> Vec<BasicLandTypeBinding> {
    vec![
        BasicLandTypeBinding {
            card_definition: "RAV-PLAINS",
            land_type: BasicLandType::Plains,
        },
        BasicLandTypeBinding {
            card_definition: "RAV-ISLAND",
            land_type: BasicLandType::Island,
        },
        BasicLandTypeBinding {
            card_definition: "RAV-SWAMP",
            land_type: BasicLandType::Swamp,
        },
        BasicLandTypeBinding {
            card_definition: "RAV-MOUNTAIN",
            land_type: BasicLandType::Mountain,
        },
        BasicLandTypeBinding {
            card_definition: "RAV-FOREST",
            land_type: BasicLandType::Forest,
        },
    ]
}

/// Expansion-owned additional spell costs. The engine owns the transactional
/// cost payment; RAV only declares which public definition requires it.
#[must_use]
pub fn rav_additional_spell_cost_bindings() -> Vec<AdditionalSpellCostBinding> {
    vec![AdditionalSpellCostBinding {
        card_definition: "RAV-FIERY-CONCLUSION",
        cost: AdditionalSpellCost::SacrificeControlledCreature,
    }]
}

/// Source-bound generic reductions supplied by RAV permanents.
#[must_use]
pub fn rav_cost_reduction_bindings() -> Vec<CostReductionBinding> {
    vec![CostReductionBinding {
        source_definition: "RAV-BLOOD-FUNNEL",
        generic_amount: 2,
        noncreature_only: true,
    }]
}

/// Battlefield-scoped activation taxes supplied by RAV permanents. Each live
/// source is discovered during cost calculation, so ordinary source departure
/// revokes its contribution without a card-specific cleanup path.
#[must_use]
pub fn rav_activated_ability_cost_modifier_bindings() -> Vec<ActivatedAbilityCostModifierBinding> {
    vec![ActivatedAbilityCostModifierBinding {
        source_definition: "RAV-SUPPRESSION-FIELD",
        modifier: ActivatedAbilityCostModifier::IncreaseGeneric {
            amount: 2,
            nonmana_only: true,
        },
    }]
}

/// Source-bound quantity replacements supplied by RAV permanents. The engine
/// checks source control and battlefield membership for each event, so this
/// registry contains only immutable definition facts.
#[must_use]
pub fn rav_replacement_effect_bindings() -> Vec<ReplacementEffectBinding> {
    vec![
        ReplacementEffectBinding {
            source_definition: "RAV-DOUBLING-SEASON",
            effect: ReplacementEffect::MultiplyTokenCreation { multiplier: 2 },
        },
        ReplacementEffectBinding {
            source_definition: "RAV-DOUBLING-SEASON",
            effect: ReplacementEffect::MultiplyCounterPlacement { multiplier: 2 },
        },
    ]
}

/// Source-bound damage-amount replacements supplied by RAV permanents. The
/// engine discovers the exact live source incarnation for each prospective
/// damage packet; this registry supplies only expansion-owned definition data.
#[must_use]
pub fn rav_damage_replacement_effect_bindings() -> Vec<DamageReplacementEffectBinding> {
    vec![DamageReplacementEffectBinding {
        source_definition: "RAV-GHOSTS-OF-THE-INNOCENT",
        effect: DamageReplacementEffect::HalveDamage,
    }]
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

fn guild_bounce_land_binding(
    card_definition: &'static str,
    id: &'static str,
    colors: [Color; 2],
) -> ManaAbilityBinding {
    ManaAbilityBinding {
        card_definition,
        ability: ActivatedManaAbility {
            id,
            tap_cost: true,
            output: ManaAbilityOutput::Bundle(ManaBundle::new(
                colors.into_iter().map(|color| (color, 1)),
            )),
            amount: 0,
            life_payment: None,
            controller_damage: None,
        },
    }
}

fn guild_bounce_land_trigger() -> TriggeredAbility {
    TriggeredAbility {
        id: "return-controlled-land",
        condition: TriggerCondition::EntersBattlefield,
        mana_cost: ManaCost::new(0),
        optional: false,
        targets: vec![cardbench_magic_engine::TargetRequirement::ControlledLand],
        effects: vec![Effect::ReturnControlledLandToHand],
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
            payment_mana_abilities: vec![],
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
            payment_mana_abilities: vec![],
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
            payment_mana_abilities: vec![],
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
            payment_mana_abilities: vec![],
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
    let mut game = Game::new_with_all_bindings_and_static_continuous_effects(
        card_definitions(),
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_static_continuous_effect_bindings(),
    )?;
    game.register_static_attack_restrictions(rav_static_attack_restriction_bindings())?;
    game.register_static_entry_restriction_bindings(rav_static_entry_restriction_bindings())?;
    game.register_cost_reduction_bindings(rav_cost_reduction_bindings())?;
    game.register_activated_ability_cost_modifier_bindings(
        rav_activated_ability_cost_modifier_bindings(),
    )?;
    Ok(game)
}

fn basic_land(id: &'static str, name: &'static str, land_type: BasicLandType) -> CardDefinition {
    CardDefinition {
        id,
        name,
        set_code: SET_CODE,
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::from([land_type.intrinsic_mana_color()]),
        card_types: types([CardType::Land]),
        is_basic_land: true,
        supported_rules: &[
            "full-rules-fidelity",
            "basic-land-type-line",
            "intrinsic-single-color-mana-ability",
            "basic-land-deck-construction",
        ],
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

/// CardBench-authored executable definition for the four RAV Signets.
/// It records only public identity, artifact type, colorless casting cost, and
/// the shared paid two-color mana-ability hook; it contains no copied card
/// rules text, art, flavor text, or card-database payload.
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
        supported_rules: &[
            "full-rules-fidelity",
            "artifact-casting",
            "paid-fixed-two-color-mana-ability",
            "cast-payment-mana-activation",
        ],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![],
    }
}

/// CardBench-authored compatibility definition for a RAV guild bounce land.
/// The entry behavior and free two-color bundle are supplied by the generic
/// bindings. The current trigger target selector is deterministic until a
/// policy-submitted triggered-choice interface is available, so these remain
/// explicitly bounded rather than full-fidelity definitions.
fn guild_bounce_land(id: &'static str, name: &'static str) -> CardDefinition {
    CardDefinition {
        id,
        name,
        set_code: SET_CODE,
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: types([CardType::Land]),
        is_basic_land: false,
        supported_rules: &[
            "enters-tapped",
            "etb-return-controlled-land",
            "free-two-color-mana-bundle",
            "deterministic-etb-target-selection",
        ],
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
        assert_eq!(first.len(), 165);
        assert!(first.iter().all(|result| !result.digest.is_empty()));
        verify_reference_event_logs().expect("public RAV logs should match fixed baselines");
    }
}
