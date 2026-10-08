//! Executable Ravnica: City of Guilds fixture slice.
//!
//! The module contains only the limited card semantics exercised by its published
//! scenarios. It intentionally does not reproduce Oracle prose, card images, flavor
//! text, collector numbers, or any hidden benchmark cases.

#![forbid(unsafe_code)]

mod cards;
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
    ActivatedAbility, ActivatedAbilityBinding, ActivatedAbilityCostBinding,
    ActivatedAbilityCostModifierBinding, ActivatedCounterCost, ActivatedCounterCostTarget,
    ActivatedManaAbility, AdditionalSpellCostBinding, AttachmentBinding, AttachmentKind,
    BasicLandType, BasicLandTypeBinding, CardDefinition, CardType, CastRequest, Color,
    ContinuousChange, ConvokeContribution, ConvokePayment, CostReductionBinding, CounterKind,
    CreatureSubtype, DamageReplacementEffect, DamageReplacementEffectBinding, DeckEntry, DeckList,
    DeckRules, Effect, EntryAttachmentBinding, EntryCharacteristicOverride, EntryCoinFlipBinding,
    EntryCopyBinding, Game, GeneralizedActivatedAbilityCost, HybridManaSymbol, Keyword,
    LandEntryBinding, LegendaryPermanentBinding, LibrarySearchCardinality,
    LibrarySearchDestination, LibrarySearchRequirement, LibrarySearchSelection, ManaAbilityBinding,
    ManaAbilityCostBinding, ManaAbilityOutput, ManaCost, PlayerId, ReplacementEffect,
    ReplacementEffectBinding, RulesError, StaticAttackRestrictionBinding,
    StaticContinuousEffectBinding, StaticCreatureSpellCostModifier,
    StaticCreatureSpellCostModifierBinding, StaticEntryRestriction, StaticEntryRestrictionBinding,
    StaticLibraryTopRevealBinding, Target, TargetRequirement, TokenSpec, TriggerCondition,
    TriggeredAbility, TriggeredAbilityBinding, Zone,
};

pub const SET_CODE: &str = "RAV";

/// The authoritative RAV definitions for which every represented printed
/// functional rule is implemented by the engine and covered by public tests.
/// Any future bounded slice must remain outside this manifest until its
/// generic substrate and regression contract are complete.
pub const RAV_FULL_FIDELITY_DEFINITION_IDS: [&str; 291] = [
    "RAV-CHAR",
    "RAV-AGRUS-KOS-WOJEK-VETERAN",
    "RAV-INSTILL-FUROR",
    "RAV-GALVANIC-ARC",
    "RAV-BREATH-OF-FURY",
    "RAV-FLAME-FUSILLADE",
    "RAV-LIGHTNING-HELIX",
    "RAV-LIFE-FROM-THE-LOAM",
    "RAV-PERILOUS-FORAYS",
    "RAV-PRIVILEGED-POSITION",
    "RAV-SEARING-MEDITATION",
    "RAV-BLOCKBUSTER",
    "RAV-BLOOD-FUNNEL",
    "RAV-BLOODBOND-MARCH",
    "RAV-PEREGRINE-MASK",
    "RAV-VOYAGER-STAFF",
    "RAV-SPECTRAL-SEARCHLIGHT",
    "RAV-PUTREFY",
    "RAV-GLIMPSE-THE-UNTHINKABLE",
    "RAV-GAZE-OF-THE-GORGON",
    "RAV-DROOLING-GROODION",
    "RAV-DARK-HEART-OF-THE-WOOD",
    "RAV-GOLGARI-ROTWURM",
    "RAV-SAVRA-QUEEN-OF-THE-GOLGARI",
    "RAV-GOLGARI-GERMINATION",
    "RAV-NULLSTONE-GARGOYLE",
    "RAV-SCATTER-THE-SEEDS",
    "RAV-SIEGE-WURM",
    "RAV-DOUBLING-SEASON",
    "RAV-GLARE-OF-SUBDUAL",
    "RAV-DRYADS-CARESS",
    "RAV-CHORD-OF-CALLING",
    "RAV-CONGREGATION-AT-DAWN",
    "RAV-FARSEEK",
    "RAV-MUDDLE-THE-MIXTURE",
    "RAV-SCION-OF-THE-WILD",
    "RAV-GUARDIAN-OF-VITU-GHAZI",
    "RAV-CONCLAVE-PHALANX",
    "RAV-ROOT-KIN-ALLY",
    "RAV-LAST-GASP",
    "RAV-ELVES-OF-DEEP-SHADOW",
    "RAV-BOROS-RECRUIT",
    "RAV-NIGHTGUARD-PATROL",
    "RAV-WATCHWOLF",
    "RAV-CHORUS-OF-THE-CONCLAVE",
    "RAV-EYE-OF-THE-STORM",
    "RAV-FOLLOWED-FOOTSTEPS",
    "RAV-GLASS-GOLEM",
    "RAV-OVERGROWN-TOMB",
    "RAV-SACRED-FOUNDRY",
    "RAV-TEMPLE-GARDEN",
    "RAV-WATERY-GRAVE",
    "RAV-JUNKTROLLER",
    "RAV-LEASHLING",
    "RAV-CROWN-OF-CONVERGENCE",
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
    "RAV-NECROPLASM",
    "RAV-DIZZY-SPELL",
    "RAV-BRAINSPOIL",
    "RAV-CLUTCH-OF-THE-UNDERCITY",
    "RAV-DISEMBOWEL",
    "RAV-BRIGHTFLAME",
    "RAV-PSYCHIC-DRAIN",
    "RAV-NIGHTMARE-VOID",
    "RAV-MOONLIGHT-BARGAIN",
    "RAV-ROLLING-SPOIL",
    "RAV-NETHERBORN-PHALANX",
    "RAV-HEX",
    "RAV-DARK-CONFIDANT",
    "RAV-EMPTY-THE-CATACOMBS",
    "RAV-MAUSOLEUM-TURNKEY",
    "RAV-SHADOW-OF-DOUBT",
    "RAV-SHRED-MEMORY",
    "RAV-HELLDOZER",
    "RAV-GREATER-MOSSDOG",
    "RAV-STINKWEED-IMP",
    "RAV-GOLGARI-THUG",
    "RAV-GOLGARI-BROWNSCALE",
    "RAV-GOLGARI-GRAVE-TROLL",
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
    "RAV-DRAKE-FAMILIAR",
    "RAV-SPAWNBROKER",
    "RAV-TATTERED-DRAKE",
    "RAV-TERRAFORMER",
    "RAV-ROOFSTALKER-WIGHT",
    "RAV-CERULEAN-SPHINX",
    "RAV-HUNTED-PHANTASM",
    "RAV-GOLIATH-SPIDER",
    "RAV-COURIER-HAWK",
    "RAV-SKYKNIGHT-LEGIONNAIRE",
    "RAV-MOROII",
    "RAV-SELESNYA-EVANGEL",
    "RAV-SELESNYA-GUILDMAGE",
    "RAV-GOLGARI-GUILDMAGE",
    "RAV-DIMIR-GUILDMAGE",
    "RAV-DIMIR-CUTPURSE",
    "RAV-DIMIR-DOPPELGANGER",
    "RAV-SISTERS-OF-STONE-DEATH",
    "RAV-MINDLEECH-MASS",
    "RAV-GLEANCRAWLER",
    "RAV-DIMIR-HOUSE-GUARD",
    "RAV-DIMIR-MACHINATIONS",
    "RAV-PERPLEX",
    "RAV-WOODWRAITH-CORRUPTER",
    "RAV-WOODWRAITH-STRANGLER",
    "RAV-TOLSIMIR-WOLFBLOOD",
    "RAV-DIMIR-INFILTRATOR",
    "RAV-LURKING-INFORMANT",
    "RAV-SANDSOWER",
    "RAV-DIVEBOMBER-GRIFFIN",
    "RAV-DROMAD-PUREBRED",
    "RAV-CARVEN-CARYATID",
    "RAV-BRAMBLE-ELEMENTAL",
    "RAV-CIVIC-WAYFINDER",
    "RAV-BIRDS-OF-PARADISE",
    "RAV-FIERY-CONCLUSION",
    "RAV-RIBBONS-OF-NIGHT",
    "RAV-GOBLIN-FIRE-FIEND",
    "RAV-BOROS-SWIFTBLADE",
    "RAV-GOBLIN-SPELUNKERS",
    "RAV-GREATER-FORGELING",
    "RAV-VIASHINO-SLASHER",
    "RAV-WAR-TORCH-GOBLIN",
    "RAV-WARP-WORLD",
    "RAV-BARBARIAN-RIFTCUTTER",
    "RAV-TORPID-MOLOCH",
    "RAV-VIASHINO-FANGTAIL",
    "RAV-BOROS-GUILDMAGE",
    "RAV-WOJEK-EMBERMAGE",
    "RAV-WOJEK-APOTHECARY",
    "RAV-THUNDERSONG-TRUMPETER",
    "RAV-SABERTOOTH-ALLEY-CAT",
    "RAV-FLAME-KIN-ZEALOT",
    "RAV-FLASH-CONSCRIPTION",
    "RAV-SUNHOME-ENFORCER",
    "RAV-ORDRUUN-COMMANDO",
    "RAV-INDENTURED-OAF",
    "RAV-MOLTEN-SENTRY",
    "RAV-MINDMOIL",
    "RAV-EXCRUCIATOR",
    "RAV-LOXODON-HIERARCH",
    "RAV-PHYTOHYDRA",
    "RAV-COALHAULER-SWINE",
    "RAV-SELL-SWORD-BRUTE",
    "RAV-FRENZIED-GOBLIN",
    "RAV-SPARKMAGE-APPRENTICE",
    "RAV-STONESHAKER-SHAMAN",
    "RAV-HUNTED-DRAGON",
    "RAV-HUNTED-TROLL",
    "RAV-KEENING-BANSHEE",
    "RAV-RAZIA-BOROS-ARCHANGEL",
    "RAV-RAZIAS-PURIFICATION",
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
    "RAV-REROUTE",
    "RAV-SINS-OF-THE-PAST",
    "RAV-SEWERDREG",
    "RAV-VOTARY-OF-THE-CONCLAVE",
    "RAV-GRAVE-SHELL-SCARAB",
    "RAV-UNDERCITY-SHADE",
    "RAV-THOUGHTPICKER-WITCH",
    "RAV-SADISTIC-AUGERMAGE",
    "RAV-VINDICTIVE-MOB",
    "RAV-BELLTOWER-SPHINX",
    "RAV-COMPULSIVE-RESEARCH",
    "RAV-TWISTED-JUSTICE",
    "RAV-DRIFT-OF-PHANTASMS",
    "RAV-ETHEREAL-USHER",
    "RAV-HALCYON-GLAZE",
    "RAV-GROZOTH",
    "RAV-FLIGHT-OF-FANCY",
    "RAV-POLLENBRIGHT-WINGS",
    "RAV-FLOW-OF-IDEAS",
    "RAV-SURVEILLING-SPRITE",
    "RAV-DREAM-LEASH",
    "RAV-REMAND",
    "RAV-INDUCE-PARANOIA",
    "RAV-TELLING-TIME",
    "RAV-TUNNEL-VISION",
    "RAV-MASTER-WARCRAFT",
    "RAV-LORE-BROKER",
    "RAV-MARK-OF-EVICTION",
    "RAV-VEDALKEN-ENTRANCER",
    "RAV-VEDALKEN-DISMISSER",
    "RAV-TIDEWATER-MINION",
    "RAV-SUNHOME-FORTRESS",
    "RAV-VITU-GHAZI",
    "RAV-SVOGTHOS-THE-RESTLESS-TOMB",
    "RAV-DUSKMANTLE-HOUSE-OF-SHADOW",
    "RAV-NULLMAGE-SHEPHERD",
    "RAV-VIGOR-MORTIS",
    "RAV-STONE-SEEDER-HIEROPHANT",
    "RAV-MOLDERVINE-CLOAK",
    "RAV-FISTS-OF-IRONWOOD",
    "RAV-CLINGING-DARKNESS",
    "RAV-COPY-ENCHANTMENT",
    "RAV-NECROMANTIC-THIRST",
    "RAV-STRANDS-OF-UNDEATH",
    "RAV-FIREMANE-ANGEL",
    "RAV-URSAPINE",
    "RAV-TRANSLUMINANT",
    "RAV-INFECTIOUS-HOST",
    "RAV-CARRION-HOWLER",
    "RAV-MORTIPEDE",
    "RAV-SELESNYA-SAGITTARS",
    "RAV-AUTOCHTHON-WURM",
    "RAV-PRIMORDIAL-SAGE",
    "RAV-TWILIGHT-DROVER",
    "RAV-TROPHY-HUNTER",
    "RAV-ELVISH-SKYSWEEPER",
    "RAV-BOROS-GARRISON",
    "RAV-DIMIR-AQUEDUCT",
    "RAV-GOLGARI-ROT-FARM",
    "RAV-SELESNYA-SANCTUARY",
    "RAV-CONVOLUTE",
    "RAV-CONSULT-THE-NECROSAGES",
    "RAV-CIRCU-DIMIR-LOBOTOMIST",
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
    "RAV-BENEVOLENT-ANCESTOR",
    "RAV-BOROS-FURY-SHIELD",
    "RAV-BATHE-IN-LIGHT",
    "RAV-LIGHT-OF-SANCTION",
    "RAV-FAITHS-FETTERS",
    "RAV-STASIS-CELL",
    "RAV-CONCERTED-EFFORT",
    "RAV-CHANT-OF-VITU-GHAZI",
    "RAV-CENTAUR-SAFEGUARD",
    "RAV-BLOODLETTER-QUILL",
    "RAV-BOTTLED-CLOISTER",
    "RAV-CYCLOPEAN-SNARE",
    "RAV-CLOUDSTONE-CURIO",
    "RAV-PLAGUE-BOILER",
    "RAV-TERRARION",
    "RAV-GRIFTERS-BLADE",
    "RAV-PARIAHS-SHIELD",
    "RAV-SUNFORGER",
    "RAV-FESTIVAL-OF-THE-GUILDPACT",
    "RAV-FLICKERFORM",
    "RAV-SUPPRESSION-FIELD",
    "RAV-LOXODON-GATEKEEPER",
    "RAV-AURATOUCHED-MAGE",
    "RAV-THREE-DREAMS",
    "RAV-CONCLAVES-BLESSING",
    "RAV-WOEBRINGER-DEMON",
    "RAV-ZEPHYR-SPIRIT",
    "RAV-WIZENED-SNITCHES",
    "RAV-VULTUROUS-ZOMBIE",
    "RAV-VINELASHER-KUDZU",
    "RAV-GHOSTS-OF-THE-INNOCENT",
    "RAV-SZADEK",
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
    /// The hand-written policy this fixture is piloted by, when it has one.
    /// Coverage fixtures name a policy; constructed decks name an archetype
    /// instead, so that a deck's measured strength is not an artifact of how
    /// well someone wrote its bespoke pilot.
    pub policy: String,
    /// The archetype a generic policy should pilot this deck as. Empty for
    /// the legacy coverage fixtures.
    pub archetype: String,
    pub deck: DeckList,
}

/// Stable number of five-color, exact-sixty-card catalog gauntlet decks.
pub const RAV_CATALOG_COVERAGE_DECK_COUNT: usize = 24;

const CATALOG_POLICY_IDS: [&str; 6] = [
    "rav.catalog-pressure.v1",
    "rav.catalog-curve.v1",
    "rav.catalog-control.v1",
    "rav.catalog-graveyard.v1",
    "rav.catalog-top-end.v1",
    "rav.catalog-patient.v1",
];

/// Builds the public all-card gauntlet decks.
///
/// Every nonbasic executable RAV identity appears in exactly one twelve-name
/// band at three copies. Each band is completed to sixty cards with an even
/// five-basic-land manabase. The result is a collection of legal, reproducible
/// five-color development decks with ordinary spell/land proportions, rather
/// than hundreds of one-card fixture games. The five basic identities appear
/// in every deck; together the corpus covers all 291 executable identities and
/// therefore every one of the 306 printing records in the catalog.
pub fn load_catalog_coverage_decks() -> Result<Vec<DeckFixture>, ManifestValidationError> {
    let basic_ids = [
        "RAV-PLAINS",
        "RAV-ISLAND",
        "RAV-SWAMP",
        "RAV-MOUNTAIN",
        "RAV-FOREST",
    ];
    let basic_set = basic_ids.into_iter().collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let nonbasic_ids = rav_main_set_catalog()
        .into_iter()
        .filter_map(|card| match card.semantic_status {
            CardSemanticStatus::ExecutableCompatibilitySlice { definition_id }
                if !basic_set.contains(definition_id) && seen.insert(definition_id) =>
            {
                Some(definition_id)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let catalog = rav_catalog();
    let mut decks = Vec::new();
    for (index, band) in nonbasic_ids.chunks(12).enumerate() {
        let mut mainboard = band
            .iter()
            .map(|card| DeckEntry {
                card: (*card).to_owned(),
                count: 3,
            })
            .collect::<Vec<_>>();
        let spell_cards = u8::try_from(band.len() * 3).map_err(|_| {
            ManifestValidationError("catalog band card count exceeds deck range".to_owned())
        })?;
        let land_cards = 60_u8.checked_sub(spell_cards).ok_or_else(|| {
            ManifestValidationError("catalog band exceeds sixty cards".to_owned())
        })?;
        let per_basic = land_cards / 5;
        let remainder = land_cards % 5;
        mainboard.extend(
            basic_ids
                .iter()
                .enumerate()
                .map(|(basic_index, card)| DeckEntry {
                    card: (*card).to_owned(),
                    count: per_basic + u8::from(basic_index < usize::from(remainder)),
                }),
        );
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
        .map_err(|error| {
            ManifestValidationError(format!("catalog coverage deck {}: {error}", index + 1))
        })?;
        decks.push(DeckFixture {
            id: format!("rav_catalog_band_{:02}", index + 1),
            name: format!("RAV Catalog Band {:02}", index + 1),
            policy: CATALOG_POLICY_IDS[index % CATALOG_POLICY_IDS.len()].to_owned(),
            archetype: String::new(),
            deck,
        });
    }
    if decks.len() != RAV_CATALOG_COVERAGE_DECK_COUNT {
        return Err(ManifestValidationError(format!(
            "catalog coverage expected {RAV_CATALOG_COVERAGE_DECK_COUNT} decks, found {}",
            decks.len()
        )));
    }
    Ok(decks)
}

#[must_use]
#[allow(clippy::too_many_lines)] // Declarative card fixture catalog is intentionally kept together.
pub fn card_definitions() -> Vec<CardDefinition> {
    vec![
        cards::char::definition(),
        cards::lightning_helix::definition(),
        // Full fidelity: its attack trigger snapshots declared attackers and
        // applies independent color-specific temporary modifiers.
        cards::agrus_kos_wojek_veteran::definition(),
        cards::galvanic_arc::definition(),
        // Full fidelity: an Aura-relative combat trigger captures the exact
        // creature that dealt combat damage, sacrifices it, then suspends for
        // its controller's required reattachment choice. A successful
        // reattachment untaps every controlled creature and inserts one
        // additional combat phase before ordinary postcombat main.
        CardDefinition {
            id: "RAV-BREATH-OF-FURY",
            name: "Breath of Fury",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Red, Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "aura-enchant-controlled-creature",
                "attached-creature-combat-damage-sacrifice-reattach",
                "controller-creature-untap",
                "additional-combat-phase",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::AttachSourceToTarget {
                target: TargetRequirement::ControlledCreature,
                changes: vec![],
            }],
        },
        // Full fidelity: resolving the sorcery snapshots the controller's
        // current creatures, granting each an ordinary stack-backed tap
        // damage ability through cleanup. The layer-six grants carry the
        // provider provenance while every recipient remains its own source.
        CardDefinition {
            id: "RAV-FLAME-FUSILLADE",
            name: "Flame Fusillade",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "controller-creature-tap-one-damage-grant-until-end-of-turn",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::GrantActivatedAbilityToControllerCreaturesUntilEndOfTurn {
                    ability: ActivatedAbility {
                        id: "granted-tap-deal-one-to-player-or-creature",
                        mana_cost: ManaCost::new(0),
                        tap_cost: true,
                        sorcery_speed: false,
                        additional_tap_creatures: 0,
                        sacrifice_source: false,
                        sacrifice_creatures: 0,
                        sacrifice_lands: 0,
                        discard_cards: 0,
                        targets: vec![TargetRequirement::PlayerOrCreature],
                        effects: vec![Effect::DealDamage {
                            amount: 1,
                            target: TargetRequirement::PlayerOrCreature,
                        }],
                    },
                },
            ],
        },
        // Full fidelity: a live source reduces only generic cost on
        // noncreature spells, then its retained spell target is sacrificed-for
        // or countered on the stack. The trigger suspends for the source
        // controller's mandatory public creature selection when one exists.
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
                "full-rules-fidelity",
                "noncreature-generic-cost-reduction",
                "cast-sacrifice-or-counter-trigger",
                "controller-selected-sacrifice-or-counter",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: every creature spell cast by any player captures its
        // public card identity before the resulting trigger returns all
        // matching creature cards from every graveyard simultaneously.
        CardDefinition {
            id: "RAV-BLOODBOND-MARCH",
            name: "Bloodbond March",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Black, Color::Green]),
            colors: colors([Color::Black, Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "any-player-creature-cast-matching-graveyard-creature-return",
                "simultaneous-graveyard-creature-battlefield-entry",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: this global artifact trigger observes each player's
        // independently tracked first noncreature spell in every turn, then
        // retains that exact spell as the ordinary counter target.
        CardDefinition {
            id: "RAV-NULLSTONE-GARGOYLE",
            name: "Nullstone Gargoyle",
            set_code: SET_CODE,
            mana_cost: ManaCost::new(9),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Artifact, CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colorless-cost-casting",
                "base-characteristics",
                "flying",
                "first-noncreature-spell-each-player-each-turn-counter",
            ],
            power: Some(4),
            toughness: Some(5),
            keywords: vec![Keyword::Flying],
            effects: vec![],
        },
        // Full fidelity: the complete target-free global-damage resolution.
        // Every creature and every player receives the fixed damage in one
        // batch before state-based actions run.
        cards::rain_of_embers::definition(),
        // Full card behavior uses the shared Radiance damage substrate:
        // target-color selection, one damage batch, then state-based actions.
        cards::cleansing_beam::definition(),
        cards::scatter_the_seeds::definition(),
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
        cards::scion_of_the_wild::definition(),
        cards::gather_courage::definition(),
        cards::fiery_conclusion::definition(),
        cards::seeds_of_strength::definition(),
        cards::fists_of_ironwood::definition(),
        // Full fidelity: Glare's two activated abilities share the same
        // explicit one-creature tap cost while retaining their distinct
        // target-free/global-prevention versus targeted-tap resolutions.
        CardDefinition {
            id: "RAV-GLARE-OF-SUBDUAL",
            name: "Glare of Subdual",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Green, Color::White]),
            colors: colors([Color::Green, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "tap-untapped-controlled-creature-tap-target-creature",
                "tap-untapped-controlled-creature-prevent-all-combat-damage",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: life gain counts battlefield creatures, and spending
        // White on the instant also untaps creatures its controller controls.
        cards::dryads_caress::definition(),
        cards::hunted_horror::definition(),
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
        cards::hunted_troll::definition(),
        // Full fidelity: Flying is a static characteristic, then the normal
        // targeted-opponent ETB trigger creates exactly one typed black 4/4
        // Horror through its own stack and priority window.
        cards::hunted_lammasu::definition(),
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
        // Full fidelity: normal casting and Flying coexist with the
        // candidate-owned targeted ETB -2/-2 trigger.
        cards::keening_banshee::definition(),
        cards::infectious_host::definition(),
        cards::smash::definition(),
        cards::blockbuster::definition(),
        cards::peregrine_mask::definition(),
        cards::voyager_staff::definition(),
        // Full fidelity: the target-bearing activation is deliberately a
        // stack ability rather than a mana ability. The exact targeted player
        // supplies one of the five colored outputs at resolution through the
        // shared public decision boundary.
        cards::spectral_searchlight::definition(),
        // Full fidelity: target legality is artifact-or-creature at cast and
        // resolution, while this destruction instruction deliberately
        // bypasses any live regeneration replacement shield.
        cards::putrefy::definition(),
        cards::glimpse_the_unthinkable::definition(),
        // Full fidelity: casting retains the controller's explicit draw or
        // discard mode and materializes its ordinary player target before the
        // card leaves the hand. The target-discard branch pauses at an exact
        // recipient-private hand selection; the caster never observes or
        // chooses the discarded cards.
        CardDefinition {
            id: "RAV-CONSULT-THE-NECROSAGES",
            name: "Consult the Necrosages",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Blue, Color::Black]),
            colors: colors([Color::Blue, Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "caster-selected-modal-target-player-draw-or-discard",
                "target-player-draw-two",
                "target-player-discard-two-private-recipient-selection",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::ChooseOneOf(vec![
                vec![Effect::DrawTargetPlayerCards { count: 2 }],
                vec![Effect::DiscardTargetPlayer { count: 2 }],
            ])],
        },
        // Full fidelity: each controller-cast Blue or Black spell queues its
        // own targeted opponent-library exile trigger. The source-bound
        // exile restriction blocks an opponent's later effect-created cast
        // permission only while this exact Circu incarnation remains live.
        CardDefinition {
            id: "RAV-CIRCU-DIMIR-LOBOTOMIST",
            name: "Circu, Dimir Lobotomist",
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
                "legendary-permanent",
                "controller-blue-spell-target-opponent-library-exile",
                "controller-black-spell-target-opponent-library-exile",
                "source-bound-opponent-exile-cast-restriction",
            ],
            power: Some(2),
            toughness: Some(3),
            keywords: vec![],
            effects: vec![],
        },
        cards::gaze_of_the_gorgon::definition(),
        cards::golgari_brownscale::definition(),
        cards::golgari_thug::definition(),
        cards::stinkweed_imp::definition(),
        // Public RAV #169 audit: Dredge and characteristics are candidate-owned.
        cards::greater_mossdog::definition(),
        // Full fidelity: its Dredge replacement and controller-submitted
        // public selection return exactly zero through three graveyard land
        // cards through the ordinary suspended spell lifecycle.
        CardDefinition {
            id: "RAV-LIFE-FROM-THE-LOAM",
            name: "Life from the Loam",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "dredge",
                "return-up-to-three-land-cards-from-graveyard",
                "policy-submitted-public-graveyard-land-selection",
            ],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Dredge(3)],
            effects: vec![Effect::ReturnUpToThreeControllerGraveyardLandCardsToHand],
        },
        // Full fidelity: this base-zero creature receives its persistent
        // graveyard-derived +1/+1 counters in the entry replacement boundary,
        // before state-based actions. Its counter-removal regeneration is an
        // ordinary stack ability with an atomic physical counter cost.
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
                "full-rules-fidelity",
                "dredge",
                "base-characteristics",
                "entry-plus-one-counters-equal-controller-graveyard-creature-cards",
                "remove-plus-one-counter-regenerate",
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
            mana_cost: ManaCost::with_colors(0, [Color::Green, Color::Green, Color::Green]),
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
                reveal_selected: false,
            }],
        },
        // Full fidelity: this is a controller-private selected creature
        // batch. The exact selected order is retained through shuffling the
        // remaining library, then becomes the new revealed top-to-bottom
        // order without fabricating zone moves for library cards.
        CardDefinition {
            id: "RAV-CONGREGATION-AT-DAWN",
            name: "Congregation at Dawn",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Green, Color::Green, Color::White]),
            colors: colors([Color::Green, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "private-up-to-three-creature-search-reveal-shuffle-ordered-library-top",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::SearchControllerLibraryMany {
                requirement: LibrarySearchRequirement::CardTypes(BTreeSet::from([
                    CardType::Creature,
                ])),
                destination: LibrarySearchDestination::LibraryTop,
                cardinality: LibrarySearchCardinality::ZeroOrMore { maximum: 3 },
                selection: LibrarySearchSelection::PolicySubmitted {
                    may_fail_to_find: false,
                },
                reveal_selected: true,
            }],
        },
        cards::farseek::definition(),
        // Full fidelity: the controller pays the typed `{1}` and selected
        // creature-sacrifice cost, then privately selects or declines an
        // eligible basic land through the ordinary suspended stack ability.
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
                "full-rules-fidelity",
                "activated-sacrifice-creature-search-basic-land",
                "battlefield-tapped-land-entry",
                "policy-submitted-basic-land-search",
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
        cards::clinging_darkness::definition(),
        cards::copy_enchantment::definition(),
        // Full fidelity: the Aura keeps its ordinary attachment endpoint,
        // then uses the shared attached-combat trigger and public graveyard
        // target choice to return the selected creature card to its owner.
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
                "full-rules-fidelity",
                "aura-attach-and-static-pt",
                "attached-combat-damage-public-graveyard-creature-return",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::AttachSourceAndModifyTargetPt {
                power: 0,
                toughness: 0,
            }],
        },
        // Full fidelity: entry uses an ordinary target-player trigger, while
        // the linked Aura binding grants its exact enchanted creature a
        // stack-backed regeneration activation.
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
                "full-rules-fidelity",
                "aura-attach-and-static-pt",
                "etb-target-player-discard",
                "attached-creature-granted-regeneration-activation",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::AttachSourceAndModifyTargetPt {
                power: 0,
                toughness: 0,
            }],
        },
        // Full fidelity: the shared Dredge replacement and both
        // beginning-of-upkeep triggers preserve controller-selected ordering.
        // A pending counter sweep remains dynamic while this exact source is
        // live, then materializes its last-known counter total on departure.
        CardDefinition {
            id: "RAV-NECROPLASM",
            name: "Necroplasm",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Black, Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "dredge",
                "base-characteristics",
                "upkeep-add-plus-one-counter",
                "upkeep-destroy-creatures-by-plus-one-counter-mana-value",
                "source-departure-last-known-counter-value",
            ],
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
        // The shared Dredge replacement and candidate-owned activation
        // represent the Scarab's complete RAV #211 gameplay slice.
        cards::grave_shell_scarab::definition(),
        cards::shambling_shell::definition(),
        cards::muddle_the_mixture::definition(),
        cards::convolute::definition(),
        cards::mnemonic_nexus::definition(),
        // Full fidelity: owner-relative permanent exchange, per-player
        // shuffle/reveal, and simultaneous return of revealed permanent cards.
        CardDefinition {
            id: "RAV-WARP-WORLD",
            name: "Warp World",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(5, [Color::Red, Color::Red, Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "owner-permanents-to-library",
                "per-player-library-shuffle",
                "reveal-count-equals-owned-permanents",
                "return-revealed-artifact-creature-land-cards",
                "revealed-nonpermanents-library-bottom",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::WarpOwnedPermanentsIntoLibrariesThenRevealAndReturnPermanentCards,
            ],
        },
        // Full fidelity: the two target positions remain controller-relative,
        // and both creatures return to their owners' hands on resolution.
        cards::peel_from_reality::definition(),
        cards::quickchange::definition(),
        // Full fidelity: each activated ability receives its own public stack
        // identity, so the policy can select exactly one live one-target
        // ability at resolution and replace its target with a different legal
        // target before the controller draws.
        CardDefinition {
            id: "RAV-REROUTE",
            name: "Reroute",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "hybrid-cost-casting",
                "target-single-target-activated-stack-ability",
                "resolution-time-different-legal-target-choice",
                "controller-draw",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::ChangeTargetOfTargetActivatedAbility,
                Effect::DrawController,
            ],
        },
        // Full fidelity: the black sorcery privately inspects the exact top
        // three cards of any targeted player's library. Its controller then
        // orders the retained top cards and chosen bottom cards separately;
        // both candidate identities and submitted order stay out of public
        // receipts. The generic stack-backed Transmute activation remains
        // independently available from hand.
        CardDefinition {
            id: "RAV-DIMIR-MACHINATIONS",
            name: "Dimir Machinations",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "private-target-player-top-three-library-reorder",
                "stack-backed-private-transmute-library-search",
            ],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Transmute(ManaCost::with_colors(
                1,
                [Color::Blue, Color::Blue],
            ))],
            effects: vec![Effect::LookAtTopCardsOfTargetPlayerAndReorder { count: 3 }],
        },
        // Full fidelity: the instant may retain zero through four distinct
        // card targets from one graveyard, then exiles every target that is
        // still legal when it resolves. Its hand-zone transmute activation
        // uses the ordinary exact-mana-value library search substrate.
        CardDefinition {
            id: "RAV-SHRED-MEMORY",
            name: "Shred Memory",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Black]),
            colors: colors([Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "exile-up-to-four-target-cards-single-graveyard",
                "transmute",
            ],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Transmute(ManaCost::with_colors(
                1,
                [Color::Black, Color::Black],
            ))],
            effects: vec![Effect::ExileUpToTargetGraveyardCards { maximum: 4 }],
        },
        cards::shadow_of_doubt::definition(),
        // Full fidelity: the targeted bounce front face snapshots the last
        // battlefield controller for the life loss, and the generic Transmute
        // activation resolves through its private, stack-backed search flow.
        CardDefinition {
            id: "RAV-CLUTCH-OF-THE-UNDERCITY",
            name: "Clutch of the Undercity",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Blue, Color::Blue, Color::Black]),
            colors: colors([Color::Blue, Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "targeted-permanent-bounce",
                "controller-life-loss",
                "stack-backed-private-transmute",
            ],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Transmute(ManaCost::with_colors(
                1,
                [Color::Blue, Color::Blue],
            ))],
            effects: vec![Effect::ReturnTargetPermanentToHandAndLoseControllerLife { amount: 3 }],
        },
        // The counter-or-discard decision stays stack-bound: the target spell
        // controller explicitly accepts the complete current-hand discard
        // (including an empty hand) or lets Perplex counter that physical
        // lower stack object. Its generic Transmute activation remains
        // independently available from hand.
        CardDefinition {
            id: "RAV-PERPLEX",
            name: "Perplex",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Blue, Color::Black]),
            colors: colors([Color::Blue, Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "counter-target-spell-unless-controller-discards-hand",
                "stack-backed-private-transmute-library-search",
            ],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Transmute(ManaCost::with_colors(
                1,
                [Color::Blue, Color::Black],
            ))],
            effects: vec![Effect::CounterTargetSpellUnlessControllerDiscardsHand],
        },
        // Full fidelity: this task-owned module defines the layer-7 modifier
        // and the hand-zone Transmute cost.
        cards::dizzy_spell::definition(),
        // Full fidelity: the front face uses a typed nonblack-creature target
        // and ordinary regenerable destruction. Its hand-zone Transmute is
        // a stack-backed, controller-private library search after ordinary
        // payment and priority handling.
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
                "full-rules-fidelity",
                "colored-cost-casting",
                "destroy-target-nonblack-creature",
                "stack-backed-private-transmute",
            ],
            power: None,
            toughness: None,
            keywords: vec![Keyword::Transmute(ManaCost::with_colors(
                1,
                [Color::Black, Color::Black],
            ))],
            effects: vec![Effect::DestroyTargetNonblackCreature],
        },
        // Full fidelity: normal colored-cost creature casting, static Fear,
        // the stack-backed private Transmute search, and the selected-creature
        // sacrifice self-regeneration activation are represented by existing
        // expansion-neutral substrates.
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
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "fear",
                "stack-backed-private-transmute",
                "sacrifice-creature-regenerate",
            ],
            power: Some(2),
            toughness: Some(3),
            keywords: vec![
                Keyword::Fear,
                Keyword::Transmute(ManaCost::with_colors(1, [Color::Black, Color::Black])),
            ],
            effects: vec![],
        },
        // Full fidelity: targeted temporary -1/-1 modifier and Dredge 3.
        cards::darkblast::definition(),
        cards::disembowel::definition(),
        // Full fidelity: the policy-declared X is paid as additional generic
        // mana, remains on the stack through the normal target/priority
        // lifecycle, then supplies the same ordered mill and life-gain
        // amount. An empty target library mills as far as possible while the
        // life gain remains the declared X value.
        CardDefinition {
            id: "RAV-BRIGHTFLAME",
            name: "Brightflame",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(
                0,
                [Color::Red, Color::Red, Color::White, Color::White],
            ),
            colors: colors([Color::Red, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "policy-submitted-chosen-x-mana-spend",
                "radiance-chosen-x-damage-total-life-gain",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::RadianceDealChosenXDamageToCreaturesAndGainLifeEqualToDamageDealt,
            ],
        },
        // Full fidelity: the policy-declared X remains through ordinary
        // casting and Radiance targeting. The coupled resolver totals only
        // committed damage receipts after replacement/prevention before it
        // records the controller's corresponding life gain.
        CardDefinition {
            id: "RAV-PSYCHIC-DRAIN",
            name: "Psychic Drain",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(0, [Color::Blue, Color::Black]),
            colors: colors([Color::Blue, Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "chosen-x-targeted-mill-and-controller-life-gain",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::MillTargetPlayerAndGainLifeControllerEqualToChosenX],
        },
        // Full fidelity: player targeting remains a stack slot and, at
        // resolution, the targeted player makes an exact private selection
        // from their current hand. The suspended stack item retains both
        // target and hand-snapshot provenance until that selection completes.
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
                "recipient-private-discard-choice",
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
        cards::empty_the_catacombs::definition(),
        cards::vigor_mortis::definition(),
        // The controller submits the graveyard-card target while the trigger
        // is placed on the stack, then accepts or declines its zero-mana
        // optional resolution through the ordinary policy boundary.
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
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "enter-battlefield-conditional-graveyard-return-to-hand",
                "policy-submitted-optional-etb-target-selection",
            ],
            power: Some(3),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        cards::rally_the_righteous::definition(),
        cards::wojek_siren::definition(),
        cards::last_gasp::definition(),
        // Full fidelity: ordinary Convoke and Trample behavior is owned by
        // the independently materializable card module.
        cards::siege_wurm::definition(),
        cards::conclave_equenaut::definition(),
        // Full fidelity: the ETB trigger reads the controller's live white
        // creature count at resolution after ordinary Convoke payment.
        cards::conclave_phalanx::definition(),
        cards::guardian_of_vitu_ghazi::definition(),
        // Full printed behavior: Convoke cost payment and the source's
        // Trample keyword use the shared multi-block damage-order decision
        // and combat-damage substrate.
        cards::autochthon_wurm::definition(),
        // Printed Convoke and a tap-two-creatures activated pump; no ETB.
        CardDefinition {
            id: "RAV-ROOT-KIN-ALLY",
            name: "Root-Kin Ally",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::Green, Color::Green]),
            colors: colors([Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "convoke",
                "base-characteristics",
                "tap-two-controlled-creatures-for-temporary-self-pump",
            ],
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
        cards::sundering_vitae::definition(),
        // Full fidelity: the ordinary artifact-or-enchantment target is
        // destroyed through the normal stack lifecycle, then two typed green
        // Saproling tokens enter under the resolving controller's control.
        cards::seed_spark::definition(),
        // Full fidelity: Radiance snapshots the target and color-sharing
        // enchantments before destroying that complete batch.
        cards::leave_no_trace::definition(),
        // Full fidelity: Convoke is paid as part of the ordinary cast, then
        // one target-free resolution instruction snapshots and destroys every
        // live nontoken creature through the normal destruction lifecycle.
        cards::hour_of_reckoning::definition(),
        cards::oathsworn_giant::definition(),
        // Full fidelity: this controller-scoped static layer-seven modifier
        // applies to every other creature, including other anthem sources.
        cards::veteran_armorer::definition(),
        cards::gate_hound::definition(),
        cards::blazing_archon::definition(),
        cards::caregiver::definition(),
        // Full fidelity: every represented positive damage packet is reduced
        // by the live source-bound integer-halving replacement.  Concurrent
        // damage replacements use the affected player's public ordering
        // decision before either replacement commits.
        cards::ghosts_of_the_innocent::definition(),
        cards::boros_fury_shield::definition(),
        cards::bathe_in_light::definition(),
        cards::concerted_effort::definition(),
        cards::light_of_sanction::definition(),
        cards::faiths_fetters::definition(),
        // Full fidelity: a creature Aura that restricts combat, with its
        // colored-cost activated reattachment ability registered separately.
        cards::stasis_cell::definition(),
        // Full fidelity: the source-bound Aura search opens a private
        // controller choice over every currently compatible Aura and permits
        // the optional failure to find before the selected Aura attaches.
        cards::auratouched_mage::definition(),
        cards::chant_of_vitu_ghazi::definition(),
        cards::festival_of_the_guildpact::definition(),
        cards::three_dreams::definition(),
        // Full fidelity: Convoke remains part of ordinary casting while the
        // attached layer-seven effect recalculates from the enchanted
        // creature controller's other current creatures.
        cards::conclaves_blessing::definition(),
        // Full fidelity: ordinary Aura attachment preserves the exact source
        // and target incarnations used by Flickerform's stack-backed blink.
        // The shared linked-exile substrate returns the creature and every
        // Aura attached to that exact creature at the next end step.
        cards::flickerform::definition(),
        // Full fidelity: this immutable battlefield binding raises every
        // nonmana activated ability's generic cost while leaving mana
        // abilities outside the modifier's scope.
        cards::suppression_field::definition(),
        cards::loxodon_gatekeeper::definition(),
        cards::recollect::definition(),
        cards::watchwolf::definition(),
        // Full fidelity: each live source offers one policy-declared optional
        // extra mana payment while its controller casts a creature spell. The
        // shared engine captures that exact source incarnation, pays it inside
        // the cast transaction, and places matching +1/+1 counters only if
        // the physical creature spell resolves onto the battlefield.
        CardDefinition {
            id: "RAV-CHORUS-OF-THE-CONCLAVE",
            name: "Chorus of the Conclave",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(
                4,
                [Color::Green, Color::Green, Color::White, Color::White],
            ),
            colors: colors([Color::Green, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "forestwalk",
                "battlefield-optional-any-mana-creature-cast-entry-counters",
            ],
            power: Some(3),
            toughness: Some(8),
            keywords: vec![Keyword::Landwalk(BasicLandType::Forest)],
            effects: vec![],
        },
        cards::followed_footsteps::definition(),
        cards::dromad_purebred::definition(),
        cards::snapping_drake::definition(),
        // Full fidelity: the target-pair ETB keeps both creature targets on
        // the trigger stack, rechecks the dependent power restriction, and
        // installs the resulting durable control exchange without zone moves.
        CardDefinition {
            id: "RAV-SPAWNBROKER",
            name: "Spawnbroker",
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
                "etb-optional-two-creature-control-exchange",
                "opponent-creature-power-at-most-controlled-target",
            ],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
        cards::carrion_howler::definition(),
        cards::coalhauler_swine::definition(),
        // Full fidelity for the original RAV printing: select a player or
        // creature, then count controller-owned creatures still attacking as
        // the spell resolves to determine the damage.
        cards::dogpile::definition(),
        cards::seismic_spike::definition(),
        cards::sparkmage_apprentice::definition(),
        // Full fidelity: each end step captures the active player and asks
        // that player to sacrifice one of their currently untapped lands.
        CardDefinition {
            id: "RAV-STONESHAKER-SHAMAN",
            name: "Stoneshaker Shaman",
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
                "each-end-step-active-player-sacrifices-untapped-land",
            ],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
        cards::hunted_dragon::definition(),
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
        // Full fidelity: resolving the target-free sorcery suspends for each
        // living player's public selection of up to three controlled
        // permanents, then sacrifices every unselected permanent as one batch
        // before the spell reaches its terminal zone.
        CardDefinition {
            id: "RAV-RAZIAS-PURIFICATION",
            name: "Razia's Purification",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::Red, Color::White]),
            colors: colors([Color::Red, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "each-player-preserves-up-to-three-controlled-permanents",
                "simultaneous-sacrifice-of-unpreserved-permanents",
                "public-policy-submitted-permanent-selection",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::EachPlayerPreservesUpToThreeControlledPermanentsThenSacrificesRest,
            ],
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
        cards::searing_meditation::definition(),
        // Full fidelity: this creature Aura persists as an ordinary typed
        // attachment, then its enchanted creature's controller receives the
        // exact end-step attack-history sacrifice trigger.
        CardDefinition {
            id: "RAV-INSTILL-FUROR",
            name: "Instill Furor",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "aura-enchant-creature",
                "attached-creature-end-step-attack-sacrifice",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::AttachSourceToTarget {
                target: TargetRequirement::Creature,
                changes: vec![],
            }],
        },
        // Full fidelity: temporarily take control of one creature, untap it,
        // and grant Haste through the current turn. One shared target bundle
        // retains the printed target's identity and incarnation through every
        // ordered instruction.
        CardDefinition {
            id: "RAV-FLASH-CONSCRIPTION",
            name: "Flash Conscription",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(5, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "gain-control-until-eot",
                "untap-target-permanent",
                "grant-haste-until-eot",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::TargetedBundle {
                target: TargetRequirement::Creature,
                effects: vec![
                    Effect::GainControlTargetUntilEndOfTurn,
                    Effect::UntapTargetPermanent,
                    Effect::ModifyTargetPtAndKeywordUntilEndOfTurn {
                        power: 0,
                        toughness: 0,
                        keyword: Keyword::Haste,
                    },
                ],
            }],
        },
        // Full fidelity: the one-mana Radiance haste grant uses the shared
        // target-color selection and layer-6 keyword effect.
        cards::surge_of_zeal::definition(),
        cards::incite_hysteria::definition(),
        cards::bramble_elemental::definition(),
        cards::skyknight_legionnaire::definition(),
        cards::moroii::definition(),
        // Full fidelity: a source-bound combat replacement converts player
        // damage into immediate library movement and source counters.  The
        // affected player orders it against every other represented damage
        // replacement before either outcome commits.
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
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "flying",
                "combat-player-damage-mill-and-counter-replacement",
                "affected-player-combat-damage-replacement-ordering",
            ],
            power: Some(5),
            toughness: Some(5),
            keywords: vec![Keyword::Flying],
            effects: vec![],
        },
        cards::loxodon_hierarch::definition(),
        // Full fidelity: the source-bound replacement operates on every
        // prospective packet aimed at this exact live permanent. It prevents
        // the packet, records the prevention, then places that many +1/+1
        // counters before ordinary damage commitment or state-based actions.
        CardDefinition {
            id: "RAV-PHYTOHYDRA",
            name: "Phytohydra",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Green, Color::White, Color::White]),
            colors: colors([Color::Green, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "self-damage-prevention-plus-one-counters",
            ],
            power: Some(1),
            toughness: Some(1),
            keywords: vec![],
            effects: vec![],
        },
        cards::boros_swiftblade::definition(),
        cards::carven_caryatid::definition(),
        cards::centaur_safeguard::definition(),
        cards::bottled_cloister::definition(),
        // Full fidelity: this colorless artifact's first activation places a
        // typed blood counter before drawing and reading the source's current
        // counter total for life loss. Its second activation uses the shared
        // atomic source-counter cost profile rather than a card-specific
        // payment path.
        CardDefinition {
            id: "RAV-BLOODLETTER-QUILL",
            name: "Bloodletter Quill",
            set_code: SET_CODE,
            mana_cost: ManaCost::new(3),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Artifact]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colorless-artifact-casting",
                "generic-two-tap-add-blood-draw-lose-life-per-blood",
                "blue-black-remove-blood-counter",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        cards::glass_golem::definition(),
        cards::junktroller::definition(),
        cards::leashling::definition(),
        // Full fidelity: this colorless artifact reveals only its controller's
        // current top library card, applies a live shared-color creature
        // layer-seven modifier when that card is a creature, and has one
        // ordinary paid library-rotation activation.
        CardDefinition {
            id: "RAV-CROWN-OF-CONVERGENCE",
            name: "Crown of Convergence",
            set_code: SET_CODE,
            mana_cost: ManaCost::new(2),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Artifact]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colorless-artifact-casting",
                "controller-top-library-revealed",
                "top-creature-shared-color-creatures-plus-one-plus-one",
                "green-white-rotate-controller-library-top-to-bottom",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: this artifact observes each nonartifact permanent
        // entering under its controller, retains the entrant's card types as
        // trigger provenance, then opens an optional non-targeting choice of
        // one other controlled permanent sharing any captured type.
        CardDefinition {
            id: "RAV-CLOUDSTONE-CURIO",
            name: "Cloudstone Curio",
            set_code: SET_CODE,
            mana_cost: ManaCost::new(3),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Artifact]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colorless-artifact-casting",
                "controlled-nonartifact-etb-may-bounce-another-sharing-card-type",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: the mandatory upkeep trigger places one named
        // plague counter while the artifact is live. Its `{1}, sacrifice`
        // activation snapshots that exact counter total before its cost
        // clears counters at zone change, then sweeps the complete set of
        // nonland permanents at that mana value.
        CardDefinition {
            id: "RAV-PLAGUE-BOILER",
            name: "Plague Boiler",
            set_code: SET_CODE,
            mana_cost: ManaCost::new(3),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Artifact]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colorless-artifact-casting",
                "upkeep-add-plague-counter",
                "activated-sacrifice-sweep-nonlands-by-plague-counters",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        cards::cyclopean_snare::definition(),
        // Full fidelity: this artifact uses one source-relative entry
        // replacement, then a paid mana activation whose two colored units
        // are explicitly selected by the policy. Its graveyard trigger stays
        // separate from the mana ability and preserves normal priority.
        CardDefinition {
            id: "RAV-TERRARION",
            name: "Terrarion",
            set_code: SET_CODE,
            mana_cost: ManaCost::new(1),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Artifact]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colorless-artifact-casting",
                "self-enters-tapped",
                "paid-tap-sacrifice-source-selected-two-colored-mana",
                "battlefield-graveyard-triggered-draw",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        cards::grifters_blade::definition(),
        // Full fidelity: this colorless Equipment costs `{5}`, has a
        // sorcery-speed `{3}` equip activation, and installs a persistent
        // non-prevention replacement while attached.
        CardDefinition {
            id: "RAV-PARIAHS-SHIELD",
            name: "Pariah's Shield",
            set_code: SET_CODE,
            mana_cost: ManaCost::new(5),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Artifact]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colorless-cost-casting",
                "equipment-all-damage-to-equipped-creature-to-controller",
                "sorcery-speed-equip-three",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: this colorless Equipment costs `{3}`, grants its
        // exact equipped creature +4/+0, has ordinary sorcery-speed `{3}`
        // equip, and uses a `{R}{W}` detach cost to suspend a policy-selected
        // controller-library instant search/cast continuation. The fetched
        // red or white instant has mana value at most four and is cast with
        // ordinary targets and stack receipts without paying its mana cost.
        CardDefinition {
            id: "RAV-SUNFORGER",
            name: "Sunforger",
            set_code: SET_CODE,
            mana_cost: ManaCost::new(3),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Artifact]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colorless-equipment-casting",
                "equipment-plus-four-plus-zero",
                "sorcery-speed-equip-three",
                "equip-three-and-detach-search-red-or-white-instant-mana-value-at-most-four-cast-without-mana",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        cards::birds_of_paradise::definition(),
        cards::elves_of_deep_shadow::definition(),
        cards::elvish_skysweeper::definition(),
        // Full fidelity: the attack trigger's optional red payment and
        // target choice are represented by the attack-trigger binding.
        cards::frenzied_goblin::definition(),
        cards::grayscaled_gharial::definition(),
        cards::greater_forgeling::definition(),
        cards::goliath_spider::definition(),
        cards::ivy_dancer::definition(),
        cards::lore_broker::definition(),
        cards::mortipede::definition(),
        cards::selesnya_evangel::definition(),
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
        // Only a land entering under this creature's controller's control
        // triggers the source untap; the source can also tap to untap a land.
        cards::stone_seeder_hierophant::definition(),
        cards::selesnya_sagittars::definition(),
        // Full fidelity: Flying and the source-relative blocker restriction
        // are represented by the shared combat and layer substrates.
        cards::screeching_griffin::definition(),
        cards::belltower_sphinx::definition(),
        cards::wizened_snitches::definition(),
        cards::compulsive_research::definition(),
        // Full fidelity: the one target is a player, not a creature. That
        // player selects a currently controlled creature only when this
        // sorcery resolves; its positive power is retained before sacrifice
        // and determines the resolving controller's ordinary draw count.
        CardDefinition {
            id: "RAV-TWISTED-JUSTICE",
            name: "Twisted Justice",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::Blue, Color::Black]),
            colors: colors([Color::Blue, Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "target-player-sacrifice-creature-power-derived-draw",
                "recipient-selected-public-creature-sacrifice",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::TargetPlayerSacrificesCreatureThenControllerDrawsEqualToPower],
        },
        cards::flight_of_fancy::definition(),
        // Full fidelity: this Aura attaches to a creature, grants Flying,
        // and observes only that exact attached creature's committed combat
        // damage to a player. The trigger materializes its token count from
        // the captured damage packet rather than a later power lookup.
        CardDefinition {
            id: "RAV-POLLENBRIGHT-WINGS",
            name: "Pollenbright Wings",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::Green, Color::White]),
            colors: colors([Color::Green, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "aura-enchant-creature-flying",
                "attached-creature-combat-damage-saproling-count",
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
        cards::flow_of_ideas::definition(),
        // Full fidelity: target restriction is checked when cast, while the
        // live Aura source determines the ongoing control effect.
        cards::dream_leash::definition(),
        cards::remand::definition(),
        cards::induce_paranoia::definition(),
        cards::telling_time::definition(),
        // Full fidelity: the name is a public resolution-time decision drawn
        // from the represented catalog, never inferred from the hidden target
        // library.  The generic traversal then reveals only the printed
        // prefix, moves preceding revealed cards only if the name was found,
        // and records the required target-library shuffle.
        CardDefinition {
            id: "RAV-TUNNEL-VISION",
            name: "Tunnel Vision",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(5, [Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Sorcery]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "target-player-library-named-card-traversal",
                "named-card-placed-on-library-top-before-shuffle",
                "other-revealed-cards-move-to-target-graveyard",
                "target-library-shuffled-after-named-traversal",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![
                Effect::TraverseTargetPlayerLibraryUntilNamedCardThenMillOthersAndShuffle,
            ],
        },
        // Full fidelity: this current-turn rules effect changes only the
        // player who makes each combat declaration; creature control and all
        // ordinary declaration legality remain unchanged.
        CardDefinition {
            id: "RAV-MASTER-WARCRAFT",
            name: "Master Warcraft",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_hybrid(
                2,
                [],
                [
                    HybridManaSymbol {
                        first: Color::Red,
                        second: Color::White,
                    },
                    HybridManaSymbol {
                        first: Color::Red,
                        second: Color::White,
                    },
                ],
            ),
            colors: colors([Color::Red, Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "hybrid-cost-casting",
                "cast-only-before-attackers-declared",
                "controller-chooses-all-attackers-this-turn",
                "controller-chooses-all-blockers-and-assignments-this-turn",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::ChooseAllCombatDeclarationsThisTurn],
        },
        cards::mark_of_eviction::definition(),
        // Full fidelity: the Aura attaches only to a creature, then its
        // active-controller upkeep trigger reads that exact live attachment
        // endpoint when it resolves. Returning the creature follows the
        // normal owner-hand lifecycle, after which SBA cleans up the Aura.
        // Full fidelity: Defender and the normal targeted mill activation are
        // both expansion-neutral typed engine rules.
        cards::vedalken_entrancer::definition(),
        cards::tidewater_minion::definition(),
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
        // Full fidelity: either color pays each hybrid cast symbol. Both
        // target-player activations are sorcery-speed stack abilities; the
        // discard mode suspends for the targeted player's private card choice.
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
                "full-rules-fidelity",
                "hybrid-cost-casting",
                "base-characteristics",
                "sorcery-speed-target-player-draw",
                "sorcery-speed-target-player-recipient-private-discard",
            ],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: a positive combat-damage receipt to a player queues
        // a source-owned trigger. That event captures its player recipient,
        // who makes the private discard selection before the controller's
        // later draw instruction resumes from the same stack object.
        CardDefinition {
            id: "RAV-DIMIR-CUTPURSE",
            name: "Dimir Cutpurse",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Blue, Color::Black]),
            colors: colors([Color::Blue, Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "combat-player-trigger-private-discard-then-draw",
            ],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: the ability targets a public creature card in either
        // graveyard, snapshots its copiable values, exiles that incarnation,
        // then applies the snapshot to the still-live source. The source's
        // physical activated ability is deliberately retained by the generic
        // graveyard-copy substrate rather than approximated as a virtual card.
        CardDefinition {
            id: "RAV-DIMIR-DOPPELGANGER",
            name: "Dimir Doppelganger",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(1, [Color::Blue, Color::Black]),
            colors: colors([Color::Blue, Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "exile-target-creature-card-from-any-graveyard-copy-source",
                "copied-form-retains-graveyard-copy-activation",
                "graveyard-target-incarnation-provenance",
            ],
            power: Some(0),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: three independent activated abilities use the
        // expansion-neutral source-relative combat and linked-exile
        // substrates. The later return is a public policy choice from only
        // this exact live source incarnation's retained creature cards.
        CardDefinition {
            id: "RAV-SISTERS-OF-STONE-DEATH",
            name: "Sisters of Stone Death",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(
                4,
                [Color::Black, Color::Black, Color::Green, Color::Green],
            ),
            colors: colors([Color::Black, Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "legendary-permanent",
                "colored-cost-casting",
                "base-characteristics",
                "target-creature-must-block-source-this-turn-if-able",
                "exile-target-creature-blocking-or-blocked-by-source",
                "return-source-linked-exiled-creature-under-controller-control",
            ],
            power: Some(7),
            toughness: Some(5),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: a positive combat-damage receipt to a player queues
        // a source-owned trigger. Its exact combat recipient privately selects
        // three current hand cards, and the ability resolves only after that
        // fixed-count decision completes.
        CardDefinition {
            id: "RAV-MINDLEECH-MASS",
            name: "Mindleech Mass",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(5, [Color::Blue, Color::Black, Color::Black]),
            colors: colors([Color::Blue, Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "trample",
                "combat-player-trigger-recipient-private-three-card-discard",
            ],
            power: Some(6),
            toughness: Some(6),
            keywords: vec![Keyword::Trample],
            effects: vec![],
        },
        // Full fidelity: the source controller's own end step reads only
        // that turn's exact creature-card battlefield-to-graveyard
        // incarnations, then returns every still-matching card to hand.
        CardDefinition {
            id: "RAV-GLEANCRAWLER",
            name: "Gleancrawler",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_hybrid(
                3,
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
                "colored-cost-casting",
                "base-characteristics",
                "trample",
                "controller-end-step-return-creature-cards-put-into-graveyard-from-battlefield-this-turn",
            ],
            power: Some(6),
            toughness: Some(6),
            keywords: vec![Keyword::Trample],
            effects: vec![],
        },
        // Full fidelity: a player-targeted stack activation pays generic two
        // and taps this hybrid creature, then suspends for the controller's
        // private, explicit may-choice over the exact current top card of the
        // selected player's library. Candidate identity never enters the
        // public event log; the optional graveyard movement does.
        CardDefinition {
            id: "RAV-LURKING-INFORMANT",
            name: "Lurking Informant",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_hybrid(
                1,
                [],
                [HybridManaSymbol {
                    first: Color::Blue,
                    second: Color::Black,
                }],
            ),
            colors: colors([Color::Blue, Color::Black]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "hybrid-cost-casting",
                "base-characteristics",
                "tap-two-target-player-private-top-library-may-graveyard",
            ],
            power: Some(1),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: this creature has ordinary Flying plus a stack-backed
        // Blue activation that moves only its exact live incarnation to its
        // owner's library and shuffles that owner. The operation deliberately
        // does not follow a temporary controller.
        cards::cerulean_sphinx::definition(),
        // Full fidelity: ordinary colored-cost casting, the 4/6 unblockable
        // body, and its policy-targeted Goblin ETB are card-module owned.
        cards::hunted_phantasm::definition(),
        // Full fidelity: normal colored-cost casting, base characteristics,
        // Flying blocker legality, and the stack-backed black self-regeneration
        // activation are represented by the shared engine.
        cards::tattered_drake::definition(),
        // Full fidelity: the candidate-owned entry trigger chooses and
        // returns one creature to its owner's library top.
        cards::vedalken_dismisser::definition(),
        cards::zephyr_spirit::definition(),
        // Full fidelity: when this exact source dies, every player privately
        // chooses one card to put on top of their own library.
        cards::sadistic_augermage::definition(),
        cards::indentured_oaf::definition(),
        // Full fidelity: an entry-time deterministic coin outcome selects
        // one persistent copiable P/T and keyword shape before state-based
        // actions or ETB triggers observe this creature.
        CardDefinition {
            id: "RAV-MOLTEN-SENTRY",
            name: "Molten Sentry",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(3, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "entry-coin-flip-copiable-characteristics",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: every controller-cast spell queues one mandatory
        // source trigger. Resolution snapshots the controller's whole hand,
        // accepts only a private exhaustive bottom-to-top order, moves that
        // exact group to library bottom, then draws the same count.
        CardDefinition {
            id: "RAV-MINDMOIL",
            name: "Mindmoil",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(4, [Color::Red]),
            colors: colors([Color::Red]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "controller-casts-spell-private-hand-bottom-draw-same-count",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        // Full-fidelity scope: colored-cost creature casting, base
        // characteristics, Defender, and the three-land activation.
        cards::torpid_moloch::definition(),
        cards::viashino_fangtail::definition(),
        cards::benevolent_ancestor::definition(),
        // Full fidelity: normal colored-cost casting, Flying combat legality,
        // and the target-free dies trigger use the shared trigger stack before
        // drawing for the source controller.
        cards::surveilling_sprite::definition(),
        // Full fidelity: its controller-relative land-type choice is in the
        // candidate-owned definition and activated-ability binding.
        cards::terraformer::definition(),
        // Full fidelity: exact Black creature identity plus a target-free,
        // stack-backed `{1}{U}` self-Flying grant through end of turn.
        cards::roofstalker_wight::definition(),
        // Full fidelity: normal colored-cost creature casting, base
        // characteristics, static Fear, and the stack-backed self-regeneration
        // activation are represented by the shared engine.
        cards::sewerdreg::definition(),
        cards::goblin_spelunkers::definition(),
        cards::ordruun_commando::definition(),
        // Full fidelity: the typed {R}, discard-a-card self-pump uses the
        // shared activated ability stack and layer-7 temporary effect.
        cards::viashino_slasher::definition(),
        cards::civic_wayfinder::definition(),
        cards::dowsing_shaman::definition(),
        cards::woebringer_demon::definition(),
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
        // Full fidelity for the printed black evasion and self-pump ability.
        cards::undercity_shade::definition(),
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
        cards::barbarian_riftcutter::definition(),
        // Its source-bound damage-prevention exception is candidate-owned.
        cards::excruciator::definition(),
        // Full fidelity: Haste, must-block legality, and the activated power
        // boost all use the shared combat, stack, and layer-7 substrates.
        cards::goblin_fire_fiend::definition(),
        // Full fidelity: changing from the battlefield to the graveyard
        // queues a controller-directed two-damage trigger.
        cards::sell_sword_brute::definition(),
        cards::war_torch_goblin::definition(),
        // Full fidelity: its controller's creature spells create an optional
        // draw trigger above the spell they cast.
        cards::primordial_sage::definition(),
        cards::transluminant::definition(),
        cards::twilight_drover::definition(),
        cards::trophy_hunter::definition(),
        cards::ursapine::definition(),
        cards::vinelasher_kudzu::definition(),
        // Full fidelity: controlled nontoken deaths create one typed
        // Saproling through the candidate-owned mandatory trigger binding.
        cards::golgari_germination::definition(),
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
        cards::flame_kin_zealot::definition(),
        cards::golgari_rotwurm::definition(),
        // Full fidelity: each creature sacrifice observes every one of its
        // colors before zone movement. The black observation is optional and
        // has an explicit life-payment policy decision; the green observation
        // gains life for this card's controller.
        CardDefinition {
            id: "RAV-SAVRA-QUEEN-OF-THE-GOLGARI",
            name: "Savra, Queen of the Golgari",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Black, Color::Green]),
            colors: colors([Color::Black, Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "controller-sacrifices-black-creature-optional-life-each-opponent-sacrifices",
                "controller-sacrifices-green-creature-may-gain-two-life",
                "multicolored-sacrifice-observes-each-matching-color",
            ],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        cards::sunhome_enforcer::definition(),
        cards::thundersong_trumpeter::definition(),
        // Full fidelity: the card-owned blocker restriction checks its
        // controller's current Mountain control at declaration time.
        cards::sabertooth_alley_cat::definition(),
        // Full fidelity: every ordinary opponent-owned card move into a
        // graveyard is observed from the battlefield and queues the source's
        // counter trigger on the ordinary stack.
        CardDefinition {
            id: "RAV-VULTUROUS-ZOMBIE",
            name: "Vulturous Zombie",
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
                "flying",
                "opponent-card-to-graveyard-plus-one-counter",
            ],
            power: Some(3),
            toughness: Some(3),
            keywords: vec![Keyword::Flying],
            effects: vec![],
        },
        // Full fidelity: the typed Forest animation retains its independent
        // target-incarnation-bound layer changes after this source leaves.
        CardDefinition {
            id: "RAV-WOODWRAITH-CORRUPTER",
            name: "Woodwraith Corrupter",
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
                "activated-persistent-forest-animation",
            ],
            power: Some(3),
            toughness: Some(6),
            keywords: vec![],
            effects: vec![],
        },
        // Full fidelity: the controller explicitly chooses a creature card
        // from their own graveyard and exiles it as the activation cost before
        // the ordinary regeneration shield is created on the stack.
        CardDefinition {
            id: "RAV-WOODWRAITH-STRANGLER",
            name: "Woodwraith Strangler",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(2, [Color::Black, Color::Green]),
            colors: colors([Color::Black, Color::Green]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Creature]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "exile-controller-graveyard-creature-card-regenerate-source",
            ],
            power: Some(2),
            toughness: Some(2),
            keywords: vec![],
            effects: vec![],
        },
        cards::tolsimir_wolfblood::definition(),
        cards::courier_hawk::definition(),
        cards::devouring_light::definition(),
        // Full printed behavior: Flying plus the sacrifice activation that
        // damages an attacking or blocking creature on stack resolution.
        cards::divebomber_griffin::definition(),
        // Full printed behavior: its definition and three-creature activation
        // are candidate-owned and materialize together.
        cards::sandsower::definition(),
        cards::votary_of_the_conclave::definition(),
        // Full printed behavior: its target-free ETB choice returns an
        // enchantment its controller controls, or sacrifices this source.
        cards::drake_familiar::definition(),
        // Full fidelity: candidate-owned identity, Defender/Flying, and the
        // rules-defined hand-zone Transmute ability. Its activation discards
        // this physical card, then resolves a private library search.
        cards::drift_of_phantasms::definition(),
        // Full fidelity: both its live-permanent and owner-graveyard upkeep
        // lifegain triggers share the normal stack lifecycle. The return
        // activation retains the source's exact graveyard incarnation.
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
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "flying",
                "first-strike",
                "battlefield-and-owner-graveyard-upkeep-life-gain",
                "owner-graveyard-return-activation",
            ],
            power: Some(4),
            toughness: Some(3),
            keywords: vec![Keyword::Flying, Keyword::FirstStrike],
            effects: vec![],
        },
        // Full fidelity: printed unblockability and stack-backed private
        // Transmute are carried by the candidate-owned definition.
        cards::dimir_infiltrator::definition(),
        cards::ethereal_usher::definition(),
        cards::halcyon_glaze::definition(),
        // Full fidelity: the optional entry trigger uses the ordinary stack
        // and a private policy-submitted multi-card library search. Transmute
        // likewise enters the stack before that private search, so opponents
        // receive the usual response window without seeing library contents.
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
                "full-rules-fidelity",
                "colored-cost-casting",
                "base-characteristics",
                "defender",
                "optional-private-multi-card-mana-value-search",
                "stack-backed-private-transmute",
            ],
            power: Some(9),
            toughness: Some(9),
            keywords: vec![
                Keyword::Defender,
                Keyword::Transmute(ManaCost::with_colors(1, [Color::Blue, Color::Blue])),
            ],
            effects: vec![],
        },
        cards::nightguard_patrol::definition(),
        cards::boros_recruit::definition(),
        cards::boros_guildmage::definition(),
        cards::wojek_embermage::definition(),
        cards::wojek_apothecary::definition(),
        cards::boros_signet::definition(),
        cards::dimir_signet::definition(),
        cards::golgari_signet::definition(),
        cards::selesnya_signet::definition(),
        cards::boros_garrison::definition(),
        cards::dimir_aqueduct::definition(),
        cards::golgari_rot_farm::definition(),
        cards::selesnya_sanctuary::definition(),
        // Full fidelity: the controller makes the required explicit
        // replacement-style choice while playing this typed Black/Green land;
        // paying two life permits an untapped entry, otherwise it enters
        // tapped. Its normal mana activation then chooses one registered
        // color without using the stack.
        cards::overgrown_tomb::definition(),
        cards::sacred_foundry::definition(),
        cards::temple_garden::definition(),
        cards::watery_grave::definition(),
        cards::sunhome_fortress_of_the_legion::definition(),
        cards::vitu_ghazi::definition(),
        // Full fidelity: this land supplies colorless mana and a stack-backed
        // self-animation whose layer-7b P/T reads the resolving controller's
        // current creature-card graveyard count through cleanup.
        CardDefinition {
            id: "RAV-SVOGTHOS-THE-RESTLESS-TOMB",
            name: "Svogthos, the Restless Tomb",
            set_code: SET_CODE,
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::new(),
            mana_colors: BTreeSet::from([Color::Colorless]),
            card_types: types([CardType::Land]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "colorless-mana-ability",
                "activated-dynamic-graveyard-creature-animation",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        cards::duskmantle_house_of_shadow::definition(),
        cards::dark_heart_of_the_wood::definition(),
        // Full fidelity: the source grants Hexproof to every other permanent
        // its controller currently controls through a shared static layer.
        cards::privileged_position::definition(),
        // Full fidelity: the source-scoped cast trigger captures each exact
        // physical instant/sorcery exile incarnation, gives the original
        // caster serial no-priority copy/cast decisions, and treats observed
        // virtual copies as terminal rather than recursively retaining them.
        CardDefinition {
            id: "RAV-EYE-OF-THE-STORM",
            name: "Eye of the Storm",
            set_code: SET_CODE,
            mana_cost: ManaCost::with_colors(5, [Color::Blue, Color::Blue]),
            colors: colors([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: types([CardType::Enchantment]),
            is_basic_land: false,
            supported_rules: &[
                "full-rules-fidelity",
                "any-player-instant-sorcery-cast-exile-and-copy",
                "cast-exiled-spell-copies-without-paying-mana",
                "source-scoped-exile-incarnation-provenance",
                "serial-no-priority-copy-cast-decisions",
            ],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![],
        },
        cards::plains::definition(),
        cards::island::definition(),
        cards::swamp::definition(),
        cards::mountain::definition(),
        cards::forest::definition(),
    ]
}

/// Definition-bound RAV mana abilities used by the shown mana-development scenarios.
///
/// Each Signet binding pays one mana, taps the artifact, and adds its two fixed
/// guild colors. The shared engine owns cost payment, atomically emitted
/// receipts, priority retention, payment-context activation during a spell
/// cast, and the fact that a mana ability does not use the stack.
#[must_use]
#[allow(clippy::too_many_lines)] // Set-owned mana bindings remain one auditable registry.
pub fn rav_mana_ability_bindings() -> Vec<ManaAbilityBinding> {
    vec![
        cards::birds_of_paradise::mana_ability(),
        cards::elves_of_deep_shadow::mana_ability(),
        ManaAbilityBinding {
            card_definition: "RAV-SUNHOME-FORTRESS",
            ability: cards::sunhome_fortress_of_the_legion::colorless_mana_ability(),
        },
        ManaAbilityBinding {
            card_definition: "RAV-VITU-GHAZI",
            ability: cards::vitu_ghazi::colorless_mana_ability(),
        },
        ManaAbilityBinding {
            card_definition: "RAV-SVOGTHOS-THE-RESTLESS-TOMB",
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
            card_definition: "RAV-TERRARION",
            ability: ActivatedManaAbility {
                id: "sacrifice-add-two-chosen-mana",
                tap_cost: true,
                output: ManaAbilityOutput::PaidChoiceBundle {
                    mana_cost: ManaCost::new(2),
                    colors: colors(Color::ALL),
                    amount: 2,
                },
                amount: 0,
                life_payment: None,
                controller_damage: None,
            },
        },
        cards::overgrown_tomb::mana_ability(),
        cards::sacred_foundry::mana_ability(),
        cards::temple_garden::mana_ability(),
        cards::watery_grave::mana_ability(),
        cards::boros_signet::mana_ability(),
        cards::dimir_signet::mana_ability(),
        cards::golgari_signet::mana_ability(),
        cards::selesnya_signet::mana_ability(),
        cards::boros_garrison::mana_ability(),
        cards::dimir_aqueduct::mana_ability(),
        cards::golgari_rot_farm::mana_ability(),
        cards::selesnya_sanctuary::mana_ability(),
    ]
}

/// Physical costs for bound RAV mana abilities. This is separate from output
/// bindings so a source sacrifice remains a reusable engine cost.
#[must_use]
pub fn rav_mana_ability_cost_bindings() -> Vec<ManaAbilityCostBinding> {
    vec![ManaAbilityCostBinding {
        card_definition: "RAV-TERRARION",
        ability_id: "sacrifice-add-two-chosen-mana",
        sacrifice_source: true,
    }]
}

/// RAV lands that enter tapped. Their return instruction is represented below
/// as an ordinary ETB trigger rather than a land-play replacement, so the
/// trigger uses the stack and can legally return the newly entered land.
#[must_use]
pub fn rav_land_entry_bindings() -> Vec<LandEntryBinding> {
    [
        cards::boros_garrison::land_entry_binding(),
        cards::dimir_aqueduct::land_entry_binding(),
        cards::golgari_rot_farm::land_entry_binding(),
        cards::selesnya_sanctuary::land_entry_binding(),
        cards::temple_garden::land_entry_binding(),
    ]
    .into_iter()
    .chain(std::iter::once(cards::overgrown_tomb::land_entry_binding()))
    .chain(std::iter::once(cards::sacred_foundry::land_entry_binding()))
    .chain(std::iter::once(cards::watery_grave::land_entry_binding()))
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
            card_definition: "RAV-ROOT-KIN-ALLY",
            ability: ActivatedAbility {
                id: "tap-two-creatures-pump-self",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 2,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::ModifySourcePtUntilEndOfTurn {
                    power: 2,
                    toughness: 2,
                }],
            },
        },
        cards::lore_broker::activated_ability(),
        ActivatedAbilityBinding {
            card_definition: "RAV-FIREMANE-ANGEL",
            ability: ActivatedAbility {
                id: "owner-graveyard-return-to-battlefield",
                mana_cost: ManaCost::with_colors(
                    6,
                    [Color::Red, Color::Red, Color::White, Color::White],
                ),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::ReturnSourceFromOwnersGraveyardToBattlefield],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-WOODWRAITH-STRANGLER",
            ability: ActivatedAbility {
                id: "exile-creature-card-regenerate-source",
                mana_cost: ManaCost::new(0),
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
            card_definition: "RAV-WOODWRAITH-CORRUPTER",
            ability: ActivatedAbility {
                id: "animate-target-forest",
                mana_cost: ManaCost::with_colors(1, [Color::Black, Color::Green]),
                tap_cost: true,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::LandWithBasicLandType(
                    BasicLandType::Forest,
                )],
                effects: vec![Effect::AnimateTargetLand {
                    land_type: BasicLandType::Forest,
                    colors: colors([Color::Black, Color::Green]),
                    creature_subtypes: BTreeSet::from([
                        CreatureSubtype::Elemental,
                        CreatureSubtype::Horror,
                    ]),
                    power: 4,
                    toughness: 4,
                }],
            },
        },
        cards::tolsimir_wolfblood::activated_ability(),
        cards::blockbuster::activated_ability(),
        cards::flickerform::activated_ability(),
        cards::tidewater_minion::untap_target_permanent_ability(),
        cards::tidewater_minion::lose_defender_ability(),
        ActivatedAbilityBinding {
            card_definition: "RAV-ROOFSTALKER-WIGHT",
            ability: cards::roofstalker_wight::activated_ability(),
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-VEDALKEN-ENTRANCER",
            ability: cards::vedalken_entrancer::activated_ability(),
        },
        cards::ivy_dancer::activated_ability(),
        cards::dowsing_shaman::activated_ability(),
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
        cards::golgari_rotwurm::activated_ability(),
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
        cards::mortipede::activated_ability(),
        ActivatedAbilityBinding {
            card_definition: "RAV-CARRION-HOWLER",
            ability: cards::carrion_howler::life_pump_ability(),
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-UNDERCITY-SHADE",
            ability: cards::undercity_shade::activated_ability(),
        },
        cards::ursapine::activated_ability(),
        cards::twilight_drover::activated_ability(),
        cards::elvish_skysweeper::activated_ability(),
        cards::trophy_hunter::activated_ability_binding(),
        cards::dark_heart_of_the_wood::activated_ability(),
        cards::shambling_shell::activated_ability(),
        ActivatedAbilityBinding {
            card_definition: "RAV-GRAVE-SHELL-SCARAB",
            ability: cards::grave_shell_scarab::activated_ability(),
        },
        cards::votary_of_the_conclave::activated_ability(),
        cards::loxodon_hierarch::activated_ability(),
        cards::sewerdreg::activated_ability(),
        ActivatedAbilityBinding {
            card_definition: "RAV-DIMIR-HOUSE-GUARD",
            ability: ActivatedAbility {
                id: "sacrifice-creature-regenerate",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 1,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::RegenerateSource],
            },
        },
        cards::tattered_drake::activated_ability(),
        cards::cerulean_sphinx::activated_ability(),
        cards::hunted_troll::activated_ability(),
        ActivatedAbilityBinding {
            card_definition: "RAV-GOLGARI-GRAVE-TROLL",
            ability: ActivatedAbility {
                id: "remove-plus-one-counter-regenerate",
                mana_cost: ManaCost::new(1),
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
        cards::sandsower::activated_ability(),
        ActivatedAbilityBinding {
            card_definition: "RAV-CROWN-OF-CONVERGENCE",
            ability: ActivatedAbility {
                id: "green-white-rotate-controller-library-top-to-bottom",
                mana_cost: ManaCost::with_colors(0, [Color::Green, Color::White]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::PutTopCardOfControllerLibraryOnBottom],
            },
        },
        cards::leashling::activated_ability(),
        cards::cyclopean_snare::activated_ability(),
        ActivatedAbilityBinding {
            card_definition: "RAV-BLOODLETTER-QUILL",
            ability: ActivatedAbility {
                id: "two-tap-add-blood-draw-lose-for-blood",
                mana_cost: ManaCost::new(2),
                tap_cost: true,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![
                    Effect::AddCountersToSource {
                        counter: CounterKind::Named("blood"),
                        amount: 1,
                    },
                    Effect::DrawController,
                    Effect::LoseLifeControllerForCountersOnSource {
                        counter: CounterKind::Named("blood"),
                    },
                ],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-BLOODLETTER-QUILL",
            ability: ActivatedAbility {
                id: "blue-black-remove-blood",
                mana_cost: ManaCost::with_colors(0, [Color::Blue, Color::Black]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-PLAGUE-BOILER",
            ability: ActivatedAbility {
                id: "one-sacrifice-sweep-nonlands-by-plague-counters",
                mana_cost: ManaCost::new(1),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: true,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![
                    Effect::DestroyAllNonlandPermanentsWithManaValueEqualToSourceCounters {
                        counter: CounterKind::Named("plague"),
                    },
                ],
            },
        },
        cards::junktroller::activated_ability(),
        cards::stasis_cell::activated_ability(),
        cards::grifters_blade::equip_ability(),
        ActivatedAbilityBinding {
            card_definition: "RAV-PARIAHS-SHIELD",
            ability: ActivatedAbility {
                id: "equip-damage-redirection",
                mana_cost: ManaCost::new(3),
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
                    changes: pariahs_shield_attachment_changes(),
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-SUNFORGER",
            ability: ActivatedAbility {
                id: "equip-plus-four-plus-zero",
                mana_cost: ManaCost::new(3),
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
                    changes: sunforger_attachment_changes(),
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-SUNFORGER",
            ability: ActivatedAbility {
                id: "red-white-detach-search-and-cast-instant",
                mana_cost: ManaCost::with_colors(0, [Color::Red, Color::White]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![
                    Effect::SearchControllerLibraryAndCastInstantWithoutPayingManaCost {
                        requirement:
                            LibrarySearchRequirement::InstantWithAnyColorAndManaValueAtMost {
                                colors: colors([Color::Red, Color::White]),
                                mana_value: 4,
                            },
                        selection: LibrarySearchSelection::PolicySubmitted {
                            may_fail_to_find: true,
                        },
                    },
                ],
            },
        },
        cards::peregrine_mask::activated_ability(),
        ActivatedAbilityBinding {
            card_definition: "RAV-VOYAGER-STAFF",
            ability: cards::voyager_staff::activated_ability(),
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-SPECTRAL-SEARCHLIGHT",
            ability: cards::spectral_searchlight::activated_ability(),
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
        cards::stone_seeder_hierophant::activated_ability(),
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
                    selection: LibrarySearchSelection::PolicySubmitted {
                        may_fail_to_find: true,
                    },
                    reveal_selected: false,
                }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-SELESNYA-EVANGEL",
            ability: cards::selesnya_evangel::activated_ability(),
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-GLARE-OF-SUBDUAL",
            ability: ActivatedAbility {
                id: "tap-target-creature",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 1,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::Creature],
                effects: vec![Effect::TapTargetCreature],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-GLARE-OF-SUBDUAL",
            ability: ActivatedAbility {
                id: "prevent-all-combat-damage",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 1,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::PreventAllCombatDamageUntilEndOfTurn],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-GOBLIN-FIRE-FIEND",
            ability: cards::goblin_fire_fiend::pump_plus_one_power_ability(),
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-GREATER-FORGELING",
            ability: cards::greater_forgeling::pump_ability(),
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-VIASHINO-SLASHER",
            ability: cards::viashino_slasher::pump_ability(),
        },
        cards::divebomber_griffin::activated_ability(),
        ActivatedAbilityBinding {
            card_definition: "RAV-SELESNYA-SAGITTARS",
            ability: cards::selesnya_sagittars::tap_damage_ability(),
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-WAR-TORCH-GOBLIN",
            ability: cards::war_torch_goblin::sacrifice_deal_two_to_blocker_ability(),
        },
        cards::caregiver::activated_ability(),
        ActivatedAbilityBinding {
            card_definition: "RAV-BENEVOLENT-ANCESTOR",
            ability: cards::benevolent_ancestor::prevention_ability(),
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
            ability: cards::viashino_fangtail::tap_deal_one_ability(),
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-ORDRUUN-COMMANDO",
            ability: cards::ordruun_commando::prevent_one_damage_ability(),
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-BOROS-GUILDMAGE",
            ability: cards::boros_guildmage::grant_haste_ability(),
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-SUNHOME-FORTRESS",
            ability: cards::sunhome_fortress_of_the_legion::double_strike_ability(),
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-VITU-GHAZI",
            ability: cards::vitu_ghazi::create_saproling_ability(),
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-SVOGTHOS-THE-RESTLESS-TOMB",
            ability: ActivatedAbility {
                id: "animate-self-from-controller-graveyard-creatures",
                mana_cost: ManaCost::with_colors(3, [Color::Black, Color::Green]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![
                    Effect::AnimateSourceIntoCreatureWithControllerGraveyardCountUntilEndOfTurn {
                        colors: BTreeSet::from([Color::Black, Color::Green]),
                        creature_subtypes: BTreeSet::from([
                            CreatureSubtype::Plant,
                            CreatureSubtype::Zombie,
                        ]),
                    },
                ],
            },
        },
        cards::duskmantle_house_of_shadow::activated_ability(),
        cards::wojek_embermage::activated_ability(),
        cards::wojek_apothecary::activated_ability(),
        cards::thundersong_trumpeter::activated_ability(),
        ActivatedAbilityBinding {
            card_definition: "RAV-SCREECHING-GRIFFIN",
            ability: cards::screeching_griffin::activated_ability(),
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-ETHEREAL-USHER",
            ability: cards::ethereal_usher::activated_ability(),
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
            ability: cards::boros_guildmage::grant_first_strike_ability(),
        },
        cards::torpid_moloch::activated_ability(),
        cards::terraformer::activated_ability(),
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
        cards::barbarian_riftcutter::activated_ability(),
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
                sorcery_speed: true,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![cardbench_magic_engine::TargetRequirement::Player],
                effects: vec![Effect::DiscardTargetPlayer { count: 1 }],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-DIMIR-DOPPELGANGER",
            ability: ActivatedAbility {
                id: "exile-graveyard-creature-card-copy-source",
                mana_cost: ManaCost::with_colors(1, [Color::Blue, Color::Black]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::CreatureCardInGraveyard],
                effects: vec![
                    Effect::ExileTargetCreatureCardFromGraveyardAndCopySourceRetainingAbility {
                        ability: "exile-graveyard-creature-card-copy-source",
                    },
                ],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-SISTERS-OF-STONE-DEATH",
            ability: ActivatedAbility {
                id: "green-target-creature-must-block-source",
                mana_cost: ManaCost::with_colors(0, [Color::Green]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::Creature],
                effects: vec![Effect::RequireTargetCreatureBlockSourceUntilEndOfTurn],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-SISTERS-OF-STONE-DEATH",
            ability: ActivatedAbility {
                id: "black-green-exile-creature-blocking-or-blocked-by-source",
                mana_cost: ManaCost::with_colors(0, [Color::Black, Color::Green]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::CreatureBlockingOrBlockedBySource],
                effects: vec![Effect::ExileTargetCreatureBlockingOrBlockedBySource],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-SISTERS-OF-STONE-DEATH",
            ability: ActivatedAbility {
                id: "two-black-return-source-linked-exiled-creature",
                mana_cost: ManaCost::with_colors(2, [Color::Black]),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![
                    Effect::ReturnSourceLinkedExiledCreatureToBattlefieldUnderControllerControl,
                ],
            },
        },
        ActivatedAbilityBinding {
            card_definition: "RAV-LURKING-INFORMANT",
            ability: ActivatedAbility {
                id: "two-tap-target-player-top-library-may-graveyard",
                mana_cost: ManaCost::new(2),
                tap_cost: true,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::Player],
                effects: vec![Effect::LookAtTargetPlayerTopLibraryMayPutIntoGraveyard],
            },
        },
    ]
}

fn pariahs_shield_attachment_changes() -> Vec<ContinuousChange> {
    vec![ContinuousChange::RedirectDamageToAttachmentController]
}

fn sunforger_attachment_changes() -> Vec<ContinuousChange> {
    vec![ContinuousChange::ModifyPowerToughness {
        power: 4,
        toughness: 0,
    }]
}

/// Equipment whose controller may choose a non-targeting attachment as it enters.
#[must_use]
pub fn rav_entry_attachment_bindings() -> Vec<EntryAttachmentBinding> {
    vec![cards::grifters_blade::entry_attachment_binding()]
}

/// Explicit persistent-attachment metadata for the RAV Equipment slice.
///
/// The activated ability above owns target ordering and stack resolution;
/// this registry makes the ongoing attachment and its layer-seven modifier
/// auditable by the expansion-neutral Equipment lifecycle.
#[must_use]
#[allow(clippy::too_many_lines)]
pub fn rav_attachment_bindings() -> Vec<AttachmentBinding> {
    vec![
        cards::grifters_blade::attachment_binding(),
        AttachmentBinding {
            card_definition: "RAV-PARIAHS-SHIELD",
            kind: AttachmentKind::Equipment,
            target: TargetRequirement::ControlledCreature,
            changes: pariahs_shield_attachment_changes(),
            granted_activated_abilities: vec![],
        },
        AttachmentBinding {
            card_definition: "RAV-SUNFORGER",
            kind: AttachmentKind::Equipment,
            target: TargetRequirement::ControlledCreature,
            changes: sunforger_attachment_changes(),
            granted_activated_abilities: vec![],
        },
        cards::peregrine_mask::attachment_binding(),
        cards::stasis_cell::attachment_binding(),
        cards::followed_footsteps::attachment_binding(),
        AttachmentBinding {
            card_definition: "RAV-NECROMANTIC-THIRST",
            kind: AttachmentKind::Aura,
            target: TargetRequirement::Creature,
            changes: vec![ContinuousChange::ModifyPowerToughness {
                power: 0,
                toughness: 0,
            }],
            granted_activated_abilities: vec![],
        },
        AttachmentBinding {
            card_definition: "RAV-STRANDS-OF-UNDEATH",
            kind: AttachmentKind::Aura,
            target: TargetRequirement::Creature,
            changes: vec![ContinuousChange::ModifyPowerToughness {
                power: 0,
                toughness: 0,
            }],
            granted_activated_abilities: vec![ActivatedAbility {
                id: "attached-creature-regeneration",
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
            }],
        },
        cards::galvanic_arc::attachment_binding(),
        AttachmentBinding {
            card_definition: "RAV-BREATH-OF-FURY",
            kind: AttachmentKind::Aura,
            target: TargetRequirement::ControlledCreature,
            changes: vec![],
            granted_activated_abilities: vec![],
        },
        AttachmentBinding {
            card_definition: "RAV-INSTILL-FUROR",
            kind: AttachmentKind::Aura,
            target: TargetRequirement::Creature,
            changes: vec![],
            granted_activated_abilities: vec![],
        },
    ]
}

/// Replacement-style entry-copy bindings supplied by RAV. The controller's
/// optional choice occurs while the permanent spell resolves, before entry
/// triggers or state-based actions—not as a cast target or ordinary effect.
#[must_use]
pub fn rav_entry_copy_bindings() -> Vec<EntryCopyBinding> {
    vec![EntryCopyBinding {
        card_definition: "RAV-COPY-ENCHANTMENT",
        copyable_type: CardType::Enchantment,
    }]
}

/// Deterministic entry-time coin-flip replacements supplied by RAV. Each
/// selected characteristic shape is stored as layer-one values of the entrant,
/// so later copies inherit that result without consuming another coin flip.
#[must_use]
pub fn rav_entry_coin_flip_bindings() -> Vec<EntryCoinFlipBinding> {
    vec![EntryCoinFlipBinding {
        card_definition: "RAV-MOLTEN-SENTRY",
        heads: EntryCharacteristicOverride {
            power: 5,
            toughness: 2,
            keywords: vec![Keyword::Haste],
        },
        tails: EntryCharacteristicOverride {
            power: 2,
            toughness: 5,
            keywords: vec![Keyword::Defender],
        },
    }]
}

/// Legendary-supertype metadata for executable RAV permanents. This registry
/// intentionally tracks layer-one definition values so Followed Footsteps and
/// other copy effects preserve the legend rule without naming card identities
/// in the core engine.
#[must_use]
pub fn rav_legendary_permanent_bindings() -> Vec<LegendaryPermanentBinding> {
    vec![
        LegendaryPermanentBinding {
            card_definition: "RAV-AGRUS-KOS-WOJEK-VETERAN",
        },
        LegendaryPermanentBinding {
            card_definition: "RAV-CIRCU-DIMIR-LOBOTOMIST",
        },
        LegendaryPermanentBinding {
            card_definition: "RAV-RAZIA-BOROS-ARCHANGEL",
        },
        LegendaryPermanentBinding {
            card_definition: "RAV-SZADEK",
        },
        LegendaryPermanentBinding {
            card_definition: "RAV-SISTERS-OF-STONE-DEATH",
        },
        LegendaryPermanentBinding {
            card_definition: "RAV-TOLSIMIR-WOLFBLOOD",
        },
    ]
}

/// Battlefield-only static characteristic bindings supplied by the RAV set.
/// These do not create events or stack objects; the engine evaluates them
/// whenever a caller reads a permanent's characteristics.
#[must_use]
pub fn rav_static_continuous_effect_bindings() -> Vec<StaticContinuousEffectBinding> {
    vec![
        StaticContinuousEffectBinding {
            card_definition: "RAV-CROWN-OF-CONVERGENCE",
            change: cardbench_magic_engine::ContinuousChange::ControlledCreaturesSharingTopLibraryCreatureCardColorsModifyPowerToughness {
                power: 1,
                toughness: 1,
            },
        },
        cards::scion_of_the_wild::static_continuous_effect(),
        cards::oathsworn_giant::static_continuous_effects()[0].clone(),
        cards::oathsworn_giant::static_continuous_effects()[1].clone(),
        cards::veteran_armorer::static_continuous_effect(),
        cards::gate_hound::static_continuous_effect(),
        cards::light_of_sanction::static_continuous_effect(),
        cards::privileged_position::static_continuous_effect(),
        cards::tolsimir_wolfblood::static_continuous_effects()[0].clone(),
        cards::tolsimir_wolfblood::static_continuous_effects()[1].clone(),
    ]
}

/// Battlefield-only public-information bindings supplied by RAV. These do
/// not create a stack object or cache a reveal receipt: each binding declares
/// whether a live source reveals every top card or only its controller's.
#[must_use]
pub fn rav_static_library_top_reveal_bindings() -> Vec<StaticLibraryTopRevealBinding> {
    vec![
        cards::wizened_snitches::static_library_top_reveal_binding(),
        StaticLibraryTopRevealBinding {
            card_definition: "RAV-CROWN-OF-CONVERGENCE",
            scope: cardbench_magic_engine::StaticLibraryTopRevealScope::SourceController,
        },
    ]
}

/// Battlefield-only static attack restrictions supplied by the RAV set.
/// These bindings are checked before attacker state changes and never create a
/// stack object or synthetic event receipt.
#[must_use]
pub fn rav_static_attack_restriction_bindings() -> Vec<StaticAttackRestrictionBinding> {
    vec![cards::blazing_archon::static_attack_restriction_binding()]
}

/// Battlefield-only optional creature-spell payment sources supplied by RAV.
/// The engine checks the actual live source and records the selected mana as
/// part of casting; set data names only the semantic modifier.
#[must_use]
pub fn rav_static_creature_spell_cost_modifier_bindings()
-> Vec<StaticCreatureSpellCostModifierBinding> {
    vec![StaticCreatureSpellCostModifierBinding {
        source_definition: "RAV-CHORUS-OF-THE-CONCLAVE",
        modifier: StaticCreatureSpellCostModifier::OptionalAnyManaForEntryCounters,
    }]
}

/// Battlefield-only entry replacements supplied by RAV. Unlike ETB
/// triggers, the engine applies these during the ordinary zone transition and
/// emits source-incarnation receipt provenance when they change an entry.
#[must_use]
pub fn rav_static_entry_restriction_bindings() -> Vec<StaticEntryRestrictionBinding> {
    vec![
        cards::loxodon_gatekeeper::static_entry_restriction_binding(),
        StaticEntryRestrictionBinding {
            card_definition: "RAV-TERRARION",
            restriction: StaticEntryRestriction::SourceEntersTapped,
        },
        StaticEntryRestrictionBinding {
            card_definition: "RAV-GOLGARI-GRAVE-TROLL",
            restriction: StaticEntryRestriction::SourceEntersWithPlusOneCountersEqualToControllerGraveyardCreatureCards,
        },
    ]
}

/// Target-free stack triggers bound to RAV permanents.
#[must_use]
#[allow(clippy::too_many_lines)] // Keep the declarative trigger registry centralized for audit review.
pub fn rav_triggered_ability_bindings() -> Vec<TriggeredAbilityBinding> {
    vec![
        TriggeredAbilityBinding {
            card_definition: "RAV-FIREMANE-ANGEL",
            ability: TriggeredAbility {
                id: "controller-upkeep-gain-one-life",
                condition: TriggerCondition::BeginningOfUpkeep,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::GainLifeController { amount: 1 }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-FIREMANE-ANGEL",
            ability: TriggeredAbility {
                id: "owner-graveyard-upkeep-gain-one-life",
                condition: TriggerCondition::BeginningOfOwnersUpkeepWhileInGraveyard,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::GainLifeController { amount: 1 }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-STRANDS-OF-UNDEATH",
            ability: TriggeredAbility {
                id: "etb-target-player-discard",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![TargetRequirement::Player],
                effects: vec![Effect::DiscardTargetPlayer { count: 1 }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-SAVRA-QUEEN-OF-THE-GOLGARI",
            ability: TriggeredAbility {
                id: "controller-sacrifices-black-creature-pay-two-life-each-opponent-sacrifices",
                condition: TriggerCondition::ControllerSacrificesCreatureOfColor(Color::Black),
                mana_cost: ManaCost::new(0),
                optional: true,
                targets: vec![],
                effects: vec![
                    Effect::EachOpponentSacrificesCreatureAfterOptionalLifePayment {
                        life_payment: 2,
                    },
                ],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-SAVRA-QUEEN-OF-THE-GOLGARI",
            ability: TriggeredAbility {
                id: "controller-sacrifices-green-creature-may-gain-two-life",
                condition: TriggerCondition::ControllerSacrificesCreatureOfColor(Color::Green),
                mana_cost: ManaCost::new(0),
                optional: true,
                targets: vec![],
                effects: vec![Effect::GainLifeController { amount: 2 }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-CIRCU-DIMIR-LOBOTOMIST",
            ability: TriggeredAbility {
                id: "controller-casts-blue-spell-exile-opponent-library-top",
                condition: TriggerCondition::CastsBlueSpell,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![TargetRequirement::Opponent],
                effects: vec![
                    Effect::ExileTopCardOfTargetOpponentLibraryAndRestrictOwnerCasting,
                ],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-CIRCU-DIMIR-LOBOTOMIST",
            ability: TriggeredAbility {
                id: "controller-casts-black-spell-exile-opponent-library-top",
                condition: TriggerCondition::CastsBlackSpell,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![TargetRequirement::Opponent],
                effects: vec![
                    Effect::ExileTopCardOfTargetOpponentLibraryAndRestrictOwnerCasting,
                ],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-MINDMOIL",
            ability: TriggeredAbility {
                id: "controller-casts-spell-hand-bottom-draw-same-count",
                condition: TriggerCondition::CastsSpell,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::PutControllerHandOnLibraryBottomThenDrawSameCount],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-EYE-OF-THE-STORM",
            ability: TriggeredAbility {
                id: "any-player-instant-or-sorcery-cast-exile-and-copy",
                condition: TriggerCondition::AnyPlayerCastsInstantOrSorcerySpell,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::ExileCastInstantOrSorceryThenCopyExiledCards],
            },
        },
        cards::followed_footsteps::triggered_ability_binding(),
        TriggeredAbilityBinding {
            card_definition: "RAV-BLOODBOND-MARCH",
            ability: TriggeredAbility {
                id: "any-player-creature-cast-return-matching-graveyard-creatures",
                condition: TriggerCondition::AnyPlayerCastsCreatureSpell,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![
                    Effect::ReturnAllCreatureCardsMatchingCastCreatureSpellNameFromGraveyards,
                ],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-DIMIR-CUTPURSE",
            ability: TriggeredAbility {
                id: "combat-player-discard-then-controller-draw",
                condition: TriggerCondition::DealsCombatDamageToPlayer,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![
                    Effect::DiscardCombatDamagePlayer { count: 1 },
                    Effect::DrawController,
                ],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-MINDLEECH-MASS",
            ability: TriggeredAbility {
                id: "combat-player-recipient-private-three-card-discard",
                condition: TriggerCondition::DealsCombatDamageToPlayer,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::DiscardCombatDamagePlayer { count: 3 }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-GLEANCRAWLER",
            ability: TriggeredAbility {
                id: "controller-end-step-return-this-turn-battlefield-creatures",
                condition: TriggerCondition::BeginningOfControllerEndStep,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![
                    Effect::ReturnControllerCreatureCardsPutIntoGraveyardFromBattlefieldThisTurnToHand,
                ],
            },
        },
        cards::halcyon_glaze::triggered_ability(),
        cards::conclave_phalanx::triggered_ability(),
        cards::bottled_cloister::opponent_upkeep_trigger(),
        cards::stinkweed_imp::triggered_ability(),
        cards::golgari_thug::triggered_ability(),
        cards::golgari_brownscale::triggered_ability(),
        TriggeredAbilityBinding {
            card_definition: "RAV-VULTUROUS-ZOMBIE",
            ability: TriggeredAbility {
                id: "opponent-card-to-graveyard-plus-one-counter",
                condition: TriggerCondition::OpponentCardPutIntoGraveyard,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::AddPlusOneCounterToSource],
            },
        },
        cards::bottled_cloister::controller_upkeep_trigger(),
        TriggeredAbilityBinding {
            card_definition: "RAV-CLOUDSTONE-CURIO",
            ability: TriggeredAbility {
                id: "controlled-nonartifact-etb-may-bounce-sharing-card-type",
                condition: TriggerCondition::ControlledNonartifactPermanentEntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: true,
                targets: vec![],
                effects: vec![Effect::ReturnAnotherControlledPermanentSharingEnteredCardTypes],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-PLAGUE-BOILER",
            ability: TriggeredAbility {
                id: "upkeep-add-plague-counter",
                condition: TriggerCondition::BeginningOfUpkeep,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::AddCountersToSource {
                    counter: CounterKind::Named("plague"),
                    amount: 1,
                }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-NECROPLASM",
            ability: TriggeredAbility {
                id: "upkeep-add-plus-one-counter",
                condition: TriggerCondition::BeginningOfUpkeep,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::AddCountersToSource {
                    counter: CounterKind::PlusOnePlusOne,
                    amount: 1,
                }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-NECROPLASM",
            ability: TriggeredAbility {
                id: "upkeep-destroy-creatures-by-plus-one-counter-mana-value",
                condition: TriggerCondition::BeginningOfUpkeep,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![
                    Effect::DestroyAllCreaturesWithManaValueEqualToSourceCounters {
                        counter: CounterKind::PlusOnePlusOne,
                    },
                ],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-TERRARION",
            ability: TriggeredAbility {
                id: "graveyard-draw",
                condition: TriggerCondition::Dies,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::DrawController],
            },
        },
        cards::auratouched_mage::triggered_ability(),
        cards::fists_of_ironwood::triggered_ability(),
        cards::flight_of_fancy::triggered_ability(),
        TriggeredAbilityBinding {
            card_definition: "RAV-POLLENBRIGHT-WINGS",
            ability: TriggeredAbility {
                id: "attached-creature-combat-damage-create-saprolings",
                condition: TriggerCondition::AttachedCreatureDealsCombatDamageToPlayer,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::CreateTokensForControllerEqualToCombatDamage {
                    token: TokenSpec::saproling(),
                }],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-BREATH-OF-FURY",
            ability: TriggeredAbility {
                id: "attached-creature-combat-damage-sacrifice-reattach-untap-add-combat",
                condition: TriggerCondition::AttachedCreatureDealsCombatDamageToPlayer,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![
                    Effect::SacrificeAttachedCombatDamagerReattachAuraUntapControllerCreaturesAddCombat,
                ],
            },
        },
        cards::mark_of_eviction::triggered_ability(),
        cards::concerted_effort::upkeep_trigger(),
        cards::faiths_fetters::triggered_ability(),
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
        cards::primordial_sage::triggered_ability(),
        TriggeredAbilityBinding {
            card_definition: "RAV-NULLSTONE-GARGOYLE",
            ability: TriggeredAbility {
                id: "first-noncreature-spell-counter",
                condition: TriggerCondition::FirstNoncreatureSpellCastEachTurn,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![cardbench_magic_engine::TargetRequirement::NoncreatureSpell],
                effects: vec![Effect::CounterTargetNoncreatureSpell],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-MAUSOLEUM-TURNKEY",
            ability: TriggeredAbility {
                id: "conditional-graveyard-return",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: true,
                targets: vec![
                    cardbench_magic_engine::TargetRequirement::CreatureCardInControllerGraveyard,
                ],
                effects: vec![Effect::ReturnTargetCreatureCardToHandIfAnotherInControllerGraveyard],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-GROZOTH",
            ability: TriggeredAbility {
                id: "etb-search-mana-value-nine",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: true,
                targets: vec![],
                effects: vec![Effect::SearchControllerLibraryMany {
                    requirement: LibrarySearchRequirement::ManaValueExactly(9),
                    destination: LibrarySearchDestination::Hand,
                    cardinality: cardbench_magic_engine::LibrarySearchCardinality::ZeroOrMore {
                        maximum: u8::MAX,
                    },
                    selection: LibrarySearchSelection::PolicySubmitted {
                        may_fail_to_find: false,
                    },
                    reveal_selected: true,
                }],
            },
        },
        cards::stone_seeder_hierophant::triggered_ability(),
        cards::vinelasher_kudzu::triggered_ability(),
        cards::civic_wayfinder::triggered_ability(),
        cards::boros_garrison::triggered_ability(),
        cards::dimir_aqueduct::triggered_ability(),
        cards::golgari_rot_farm::triggered_ability(),
        cards::selesnya_sanctuary::triggered_ability(),
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
        cards::moroii::triggered_ability(),
        TriggeredAbilityBinding {
            card_definition: "RAV-SPAWNBROKER",
            ability: TriggeredAbility {
                id: "etb-may-exchange-control-creatures",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: true,
                targets: vec![
                    TargetRequirement::ControlledCreature,
                    TargetRequirement::OpponentCreature,
                ],
                effects: vec![Effect::ExchangeControlOfTargetCreatures],
            },
        },
        cards::woebringer_demon::triggered_ability(),
        TriggeredAbilityBinding {
            card_definition: "RAV-STONESHAKER-SHAMAN",
            ability: TriggeredAbility {
                id: "each-end-step-active-player-sacrifice-untapped-land",
                condition: TriggerCondition::BeginningOfAnyEndStep,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::SacrificeEndStepPlayerUntappedLand],
            },
        },
        cards::searing_meditation::triggered_ability(),
        cards::carven_caryatid::triggered_ability(),
        cards::drake_familiar::triggered_ability(),
        cards::bramble_elemental::triggered_ability(),
        cards::sparkmage_apprentice::triggered_ability_binding(),
        cards::hunted_dragon::triggered_ability(),
        cards::hunted_lammasu::triggered_ability(),
        cards::hunted_horror::triggered_ability(),
        cards::hunted_troll::triggered_ability(),
        cards::hunted_phantasm::triggered_ability(),
        cards::keening_banshee::triggered_ability(),
        cards::vedalken_dismisser::triggered_ability(),
        cards::infectious_host::triggered_ability(),
        cards::transluminant::triggered_ability(),
        cards::trophy_hunter::triggered_ability_binding(),
        cards::twilight_drover::triggered_ability(),
        cards::surveilling_sprite::triggered_ability(),
        cards::zephyr_spirit::triggered_ability(),
        cards::flame_kin_zealot::triggered_ability(),
        cards::frenzied_goblin::triggered_ability(),
        cards::agrus_kos_wojek_veteran::triggered_ability(),
        cards::dromad_purebred::triggered_ability(),
        cards::loxodon_hierarch::triggered_ability(),
        cards::centaur_safeguard::triggered_ability(),
        cards::sunhome_enforcer::triggered_ability(),
        cards::coalhauler_swine::triggered_ability(),
        cards::belltower_sphinx::triggered_ability(),
        cards::sell_sword_brute::triggered_ability(),
        cards::sadistic_augermage::triggered_ability(),
        cards::golgari_germination::triggered_ability(),
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

/// Attachment-relative RAV triggers are registered separately because their
/// typed Aura bindings must be installed in the same setup transaction. This
/// keeps synthetic fixtures that exercise unrelated triggers from claiming a
/// complete attachment registry while still making the Instill Furor line
/// explicit for full-fidelity games.
#[must_use]
pub fn rav_attachment_triggered_ability_bindings() -> Vec<TriggeredAbilityBinding> {
    vec![
        TriggeredAbilityBinding {
            card_definition: "RAV-NECROMANTIC-THIRST",
            ability: TriggeredAbility {
                id: "attached-combat-damage-return-creature-card-to-owner-hand",
                condition: TriggerCondition::AttachedCreatureDealsCombatDamageToPlayer,
                mana_cost: ManaCost::new(0),
                optional: true,
                targets: vec![TargetRequirement::CreatureCardInGraveyard],
                effects: vec![Effect::ReturnTargetCreatureCardFromGraveyardToOwnersHand],
            },
        },
        TriggeredAbilityBinding {
            card_definition: "RAV-INSTILL-FUROR",
            ability: TriggeredAbility {
                id: "attached-creature-controller-end-step-sacrifice-unless-attacked",
                condition: TriggerCondition::BeginningOfAttachedCreaturesControllerEndStep,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::SacrificeAttachedCreatureUnlessItAttackedThisTurn],
            },
        },
        cards::galvanic_arc::triggered_ability_binding(),
    ]
}

/// Typed basic-land type lines for the five RAV basic-land definitions.
///
/// The engine validates that every binding names a basic land and that its
/// intrinsic one-color mana ability agrees with the registered type.
#[must_use]
pub fn rav_basic_land_type_bindings() -> Vec<BasicLandTypeBinding> {
    vec![
        cards::plains::basic_land_type_binding(),
        cards::island::basic_land_type_binding(),
        cards::swamp::basic_land_type_binding(),
        cards::mountain::basic_land_type_binding(),
        cards::forest::basic_land_type_binding(),
    ]
}

/// Expansion-owned additional spell costs. The engine owns the transactional
/// cost payment; RAV only declares which public definition requires it.
#[must_use]
pub fn rav_additional_spell_cost_bindings() -> Vec<AdditionalSpellCostBinding> {
    vec![cards::fiery_conclusion::additional_spell_cost_binding()]
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
    vec![cards::suppression_field::cost_modifier_binding()]
}

/// Immutable generalized activation costs used by the executable RAV slice.
/// Policies submit every concrete zone or counter choice with the activation.
#[must_use]
pub fn rav_generalized_activated_ability_cost_bindings() -> Vec<ActivatedAbilityCostBinding> {
    vec![
        ActivatedAbilityCostBinding {
            card_definition: "RAV-WOODWRAITH-STRANGLER",
            ability_id: "exile-creature-card-regenerate-source",
            cost: GeneralizedActivatedAbilityCost {
                exile_controller_graveyard_creature_cards: 1,
                ..GeneralizedActivatedAbilityCost::default()
            },
        },
        cards::carrion_howler::life_pump_cost(),
        cards::leashling::activated_ability_cost(),
        ActivatedAbilityCostBinding {
            card_definition: "RAV-BLOODLETTER-QUILL",
            ability_id: "blue-black-remove-blood",
            cost: GeneralizedActivatedAbilityCost {
                counter_removals: vec![ActivatedCounterCost {
                    target: ActivatedCounterCostTarget::Source,
                    counter: CounterKind::Named("blood"),
                    amount: 1,
                }],
                ..GeneralizedActivatedAbilityCost::default()
            },
        },
        ActivatedAbilityCostBinding {
            card_definition: "RAV-GOLGARI-GRAVE-TROLL",
            ability_id: "remove-plus-one-counter-regenerate",
            cost: GeneralizedActivatedAbilityCost {
                counter_removals: vec![ActivatedCounterCost {
                    target: ActivatedCounterCostTarget::Source,
                    counter: CounterKind::PlusOnePlusOne,
                    amount: 1,
                }],
                ..GeneralizedActivatedAbilityCost::default()
            },
        },
        cards::dark_heart_of_the_wood::activated_ability_cost(),
        cards::twilight_drover::activated_ability_cost(),
        ActivatedAbilityCostBinding {
            card_definition: "RAV-SUNFORGER",
            ability_id: "red-white-detach-search-and-cast-instant",
            cost: GeneralizedActivatedAbilityCost {
                detach_source_equipment: true,
                ..GeneralizedActivatedAbilityCost::default()
            },
        },
    ]
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
    vec![
        cards::ghosts_of_the_innocent::damage_replacement_binding(),
        DamageReplacementEffectBinding {
            source_definition: "RAV-PHYTOHYDRA",
            effect: DamageReplacementEffect::PreventSelfDamageAndAddPlusOneCounters,
        },
        DamageReplacementEffectBinding {
            source_definition: "RAV-SZADEK",
            effect: DamageReplacementEffect::ReplaceCombatDamageToPlayerWithMillAndCounters,
        },
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
    load_deck_index("decks/reference_decks.toml")
}

/// Loads the constructed decks -- real 24-land, 36-spell lists intended to be
/// played rather than to exercise rules coverage.
///
/// Kept separate from [`load_reference_decks`] deliberately. The reference
/// fixtures are 44-52 lands with four to seven distinct cards and are
/// load-bearing for the existing catalog gauntlets; mixing the two would make
/// every matchup number a blend of "deck" and "coverage fixture".
///
/// # Errors
///
/// Returns an error when the index or any deck file is missing, malformed, or
/// fails deck-construction validation.
pub fn load_constructed_decks() -> Result<Vec<DeckFixture>, ManifestValidationError> {
    load_deck_index("decks/constructed_decks.toml")
}

/// Loads deck candidates kept outside the frozen constructed-deck control
/// group. The policy ladder and public matrix intentionally do not see these;
/// the hill-climb runner compares them against the controls with the same
/// paired-seed match harness.
pub fn load_hillclimb_decks() -> Result<Vec<DeckFixture>, ManifestValidationError> {
    load_deck_index("decks/hillclimb_decks.toml")
}

fn load_deck_index(relative: &str) -> Result<Vec<DeckFixture>, ManifestValidationError> {
    let index_path = set_root().join(relative);
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
            "{}: deck index has no deck paths",
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
    let mut archetype = String::new();
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
        } else if let Some(value) = line.strip_prefix("archetype = ") {
            value.trim_matches('"').clone_into(&mut archetype);
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
    if id.is_empty() || name.is_empty() || mainboard.is_empty() {
        return Err(ManifestValidationError(format!(
            "{}: deck requires id, name, and mainboard entries",
            path.display()
        )));
    }
    if policy.is_empty() && archetype.is_empty() {
        return Err(ManifestValidationError(format!(
            "{}: deck must name either a policy or an archetype",
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
        archetype,
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

/// Executes one named versioned public train scenario. This is a focused
/// development/debugging surface; reference verification still runs every
/// public scenario through [`run_all_scenarios`].
pub fn run_public_scenario(id: &str) -> Result<ScenarioResult, String> {
    scenarios::run_public_scenario(id)
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
    game.set_shuffle_seed(41)?;
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

/// Constructs a RAV game with every executable expansion binding installed.
///
/// Policy campaigns must use this boundary instead of pairing
/// [`card_definitions`] with `Game::new`: definitions alone do not install the
/// set's triggers, attachments, activated abilities, replacements, or land
/// entry behavior.
pub fn new_rav_game(player_count: usize) -> Result<Game, RulesError> {
    let mut triggers = rav_triggered_ability_bindings();
    triggers.extend(rav_attachment_triggered_ability_bindings());
    let mut game = Game::new_with_all_bindings_triggers_static_continuous_effects_and_land_entries(
        card_definitions(),
        player_count,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        triggers,
        rav_static_continuous_effect_bindings(),
        rav_land_entry_bindings(),
    )?;
    game.register_static_library_top_reveal_bindings(rav_static_library_top_reveal_bindings())?;
    game.register_static_attack_restrictions(rav_static_attack_restriction_bindings())?;
    game.register_static_creature_spell_cost_modifiers(
        rav_static_creature_spell_cost_modifier_bindings(),
    )?;
    game.register_attachment_bindings(rav_attachment_bindings())?;
    game.register_entry_attachment_bindings(rav_entry_attachment_bindings())?;
    game.register_entry_copy_bindings(rav_entry_copy_bindings())?;
    game.register_entry_coin_flip_bindings(rav_entry_coin_flip_bindings())?;
    game.register_legendary_permanent_bindings(rav_legendary_permanent_bindings())?;
    game.register_static_entry_restriction_bindings(rav_static_entry_restriction_bindings())?;
    game.register_mana_ability_cost_bindings(rav_mana_ability_cost_bindings())?;
    game.register_cost_reduction_bindings(rav_cost_reduction_bindings())?;
    game.register_activated_ability_cost_modifier_bindings(
        rav_activated_ability_cost_modifier_bindings(),
    )?;
    game.register_generalized_activated_ability_cost_bindings(
        rav_generalized_activated_ability_cost_bindings(),
    )?;
    game.register_damage_replacement_effect_bindings(rav_damage_replacement_effect_bindings())?;
    game.register_replacement_effect_bindings(rav_replacement_effect_bindings())?;
    game.validate_invariants()?;
    Ok(game)
}

fn fresh_game() -> Result<Game, RulesError> {
    new_rav_game(2)
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
        assert_eq!(first.len(), 192);
        assert!(first.iter().all(|result| !result.digest.is_empty()));
        verify_reference_event_logs().expect("public RAV logs should match fixed baselines");
    }
}
