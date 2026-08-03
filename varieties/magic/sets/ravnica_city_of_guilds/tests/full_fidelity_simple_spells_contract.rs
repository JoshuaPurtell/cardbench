//! Ability-complete contract for small, fully represented RAV cards.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardType, Color, Effect, Keyword, ManaCost, TargetRequirement, TokenSpec,
};
use cardbench_magic_rav::{RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, run_all_scenarios};

#[test]
#[allow(clippy::too_many_lines)] // The positive manifest audit is easiest to review as one table.
fn full_fidelity_manifest_records_only_ability_complete_cards() {
    assert_eq!(
        RAV_FULL_FIDELITY_DEFINITION_IDS,
        [
            "RAV-CHAR",
            "RAV-GALVANIC-ARC",
            "RAV-FLAME-FUSILLADE",
            "RAV-LIGHTNING-HELIX",
            "RAV-LIFE-FROM-THE-LOAM",
            "RAV-PERILOUS-FORAYS",
            "RAV-SEARING-MEDITATION",
            "RAV-BLOCKBUSTER",
            "RAV-BLOOD-FUNNEL",
            "RAV-PEREGRINE-MASK",
            "RAV-VOYAGER-STAFF",
            "RAV-SPECTRAL-SEARCHLIGHT",
            "RAV-PUTREFY",
            "RAV-GLIMPSE-THE-UNTHINKABLE",
            "RAV-GAZE-OF-THE-GORGON",
            "RAV-DROOLING-GROODION",
            "RAV-DARK-HEART-OF-THE-WOOD",
            "RAV-GOLGARI-ROTWURM",
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
            "RAV-LOXODON-HIERARCH",
            "RAV-COALHAULER-SWINE",
            "RAV-SELL-SWORD-BRUTE",
            "RAV-FRENZIED-GOBLIN",
            "RAV-SPARKMAGE-APPRENTICE",
            "RAV-STONESHAKER-SHAMAN",
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
            "RAV-DRIFT-OF-PHANTASMS",
            "RAV-ETHEREAL-USHER",
            "RAV-HALCYON-GLAZE",
            "RAV-GROZOTH",
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
            "RAV-DUSKMANTLE-HOUSE-OF-SHADOW",
            "RAV-NULLMAGE-SHEPHERD",
            "RAV-VIGOR-MORTIS",
            "RAV-STONE-SEEDER-HIEROPHANT",
            "RAV-MOLDERVINE-CLOAK",
            "RAV-FISTS-OF-IRONWOOD",
            "RAV-CLINGING-DARKNESS",
            "RAV-COPY-ENCHANTMENT",
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
        ]
    );
    let definitions = card_definitions();
    let exact = [
        (
            "RAV-SINS-OF-THE-PAST",
            vec![Effect::GrantGraveyardCastPermissionUntilEndOfTurn],
        ),
        (
            "RAV-INDUCE-PARANOIA",
            vec![
                Effect::CounterTargetPhysicalSpellThenMillItsControllerByManaValueIfManaColorSpent {
                    color: Color::Blue,
                },
            ],
        ),
        (
            "RAV-QUICKCHANGE",
            vec![
                Effect::ReplaceTargetCreatureColorsWithChosenColorUntilEndOfTurn,
                Effect::DrawController,
            ],
        ),
        (
            "RAV-CHAR",
            vec![
                Effect::DealDamage {
                    amount: 4,
                    target: TargetRequirement::PlayerOrCreature,
                },
                Effect::DealDamageController { amount: 2 },
            ],
        ),
        (
            "RAV-LIGHTNING-HELIX",
            vec![
                Effect::DealDamage {
                    amount: 3,
                    target: TargetRequirement::PlayerOrCreature,
                },
                Effect::GainLifeController { amount: 3 },
            ],
        ),
        (
            "RAV-LAST-GASP",
            vec![Effect::ModifyTargetPtUntilEndOfTurn {
                power: -3,
                toughness: -3,
            }],
        ),
        (
            "RAV-CLEANSING-BEAM",
            vec![Effect::RadianceDealDamageToCreatures { amount: 2 }],
        ),
        (
            "RAV-RALLY-THE-RIGHTEOUS",
            vec![Effect::RadianceUntapAndModifyUntilEndOfTurn {
                power: 2,
                toughness: 0,
            }],
        ),
        (
            "RAV-WOJEK-SIREN",
            vec![Effect::RadianceModifyPtUntilEndOfTurn {
                power: 1,
                toughness: 1,
            }],
        ),
        (
            "RAV-RAIN-OF-EMBERS",
            vec![Effect::DealDamageToEachCreatureAndPlayer { amount: 1 }],
        ),
        (
            "RAV-DOGPILE",
            vec![Effect::DealDamageEqualToAttackingCreatures {
                target: TargetRequirement::PlayerOrCreature,
            }],
        ),
        (
            "RAV-OVERWHELM",
            vec![
                Effect::ModifyControllerCreaturesPtUntilEndOfTurn {
                    power: 3,
                    toughness: 3,
                },
                Effect::AddKeywordToControllerCreaturesUntilEndOfTurn {
                    keyword: Keyword::Trample,
                },
            ],
        ),
        (
            "RAV-DARKBLAST",
            vec![Effect::ModifyTargetPtUntilEndOfTurn {
                power: -1,
                toughness: -1,
            }],
        ),
        (
            "RAV-DISEMBOWEL",
            vec![Effect::DestroyTargetCreatureWithManaValueAtMostChosenX],
        ),
        (
            "RAV-MOONLIGHT-BARGAIN",
            vec![Effect::LookAtTopCardsChooseForLifeOrGraveyard {
                count: 5,
                life_per_card: 2,
            }],
        ),
        (
            "RAV-INCITE-HYSTERIA",
            vec![Effect::RadianceAddKeywordUntilEndOfTurn {
                keyword: Keyword::CannotBlock,
            }],
        ),
        ("RAV-DEVOURING-LIGHT", vec![Effect::ExileTargetPermanent]),
        (
            "RAV-SMASH",
            vec![Effect::DestroyTargetArtifact, Effect::DrawController],
        ),
        (
            "RAV-PUTREFY",
            vec![Effect::DestroyTargetArtifactOrCreatureNoRegeneration],
        ),
        (
            "RAV-SUNDERING-VITAE",
            vec![Effect::DestroyTargetArtifactOrEnchantment],
        ),
        (
            "RAV-SEED-SPARK",
            vec![
                Effect::DestroyTargetArtifactOrEnchantment,
                Effect::CreateToken {
                    token: TokenSpec::saproling(),
                    count: 2,
                },
            ],
        ),
        (
            "RAV-LEAVE-NO-TRACE",
            vec![Effect::RadianceDestroyEnchantments],
        ),
        (
            "RAV-HOUR-OF-RECKONING",
            vec![Effect::DestroyAllNonTokenCreatures],
        ),
        ("RAV-RECOLLECT", vec![Effect::ReturnTargetCardToHand]),
    ];
    for (id, effects) in exact {
        let definition = definitions
            .iter()
            .find(|definition| definition.id == id)
            .expect("definition exists");
        assert_eq!(definition.supported_rules[0], "full-rules-fidelity", "{id}");
        assert_eq!(definition.effects, effects, "{id}");
    }
    let elves = definitions
        .iter()
        .find(|definition| definition.id == "RAV-ELVES-OF-DEEP-SHADOW")
        .expect("Elves of Deep Shadow definition exists");
    assert_eq!(elves.supported_rules[0], "full-rules-fidelity");
    let recruit = definitions
        .iter()
        .find(|definition| definition.id == "RAV-BOROS-RECRUIT")
        .expect("Boros Recruit definition exists");
    assert_eq!(recruit.supported_rules[0], "full-rules-fidelity");
    let watchwolf = definitions
        .iter()
        .find(|definition| definition.id == "RAV-WATCHWOLF")
        .expect("Watchwolf definition exists");
    assert_eq!(watchwolf.supported_rules[0], "full-rules-fidelity");
    assert_eq!(
        watchwolf.mana_cost,
        ManaCost::with_colors(0, [Color::Green, Color::White])
    );
    assert_eq!(watchwolf.power, Some(3));
    assert_eq!(watchwolf.toughness, Some(3));
    assert_eq!(watchwolf.keywords, []);
    assert_eq!(watchwolf.effects, []);
    assert_eq!(
        watchwolf.card_types.iter().cloned().collect::<Vec<_>>(),
        [CardType::Creature]
    );

    let glass_golem = definitions
        .iter()
        .find(|definition| definition.id == "RAV-GLASS-GOLEM")
        .expect("Glass Golem definition exists");
    assert_eq!(glass_golem.supported_rules[0], "full-rules-fidelity");
    assert_eq!(glass_golem.mana_cost, ManaCost::new(5));
    assert_eq!(glass_golem.power, Some(6));
    assert_eq!(glass_golem.toughness, Some(2));
    assert_eq!(glass_golem.keywords, []);
    assert_eq!(glass_golem.effects, []);
    assert_eq!(
        glass_golem.card_types.iter().cloned().collect::<Vec<_>>(),
        [CardType::Artifact, CardType::Creature]
    );
    let siren = definitions
        .iter()
        .find(|definition| definition.id == "RAV-WOJEK-SIREN")
        .expect("Wojek Siren definition exists");
    assert_eq!(siren.mana_cost, ManaCost::with_colors(0, [Color::White]));
    assert_eq!(siren.colors, [Color::White].into_iter().collect());

    let darkblast = definitions
        .iter()
        .find(|definition| definition.id == "RAV-DARKBLAST")
        .expect("Darkblast definition exists");
    assert_eq!(
        darkblast.supported_rules,
        ["full-rules-fidelity", "targeted-layer-7-modifier", "dredge"]
    );
    assert_eq!(
        darkblast.mana_cost,
        ManaCost::with_colors(0, [Color::Black])
    );
    assert_eq!(darkblast.colors, [Color::Black].into_iter().collect());
    assert_eq!(
        darkblast.card_types,
        [CardType::Instant].into_iter().collect()
    );
    assert_eq!(darkblast.keywords, [Keyword::Dredge(3)]);
    assert_eq!(
        darkblast.effects[0].target_requirement(),
        Some(TargetRequirement::Creature)
    );

    let cloak = definitions
        .iter()
        .find(|definition| definition.id == "RAV-MOLDERVINE-CLOAK")
        .expect("Moldervine Cloak definition exists");
    assert_eq!(
        cloak.supported_rules,
        ["full-rules-fidelity", "aura-attach-and-static-pt", "dredge"]
    );
    assert_eq!(cloak.mana_cost, ManaCost::with_colors(2, [Color::Green]));
    assert_eq!(cloak.colors, [Color::Green].into_iter().collect());
    assert_eq!(
        cloak.card_types,
        [CardType::Enchantment].into_iter().collect()
    );
    assert_eq!(cloak.keywords, [Keyword::Dredge(2)]);
    assert_eq!(
        cloak.effects,
        [Effect::AttachSourceAndModifyTargetPt {
            power: 3,
            toughness: 3,
        }]
    );

    let darkness = definitions
        .iter()
        .find(|definition| definition.id == "RAV-CLINGING-DARKNESS")
        .expect("Clinging Darkness definition exists");
    assert_eq!(
        darkness.supported_rules,
        ["full-rules-fidelity", "aura-attach-and-static-pt"]
    );
    assert_eq!(darkness.mana_cost, ManaCost::with_colors(1, [Color::Black]));
    assert_eq!(darkness.colors, [Color::Black].into_iter().collect());
    assert_eq!(
        darkness.card_types,
        [CardType::Enchantment].into_iter().collect()
    );
    assert!(darkness.keywords.is_empty());
    assert_eq!(
        darkness.effects,
        [Effect::AttachSourceAndModifyTargetPt {
            power: -3,
            toughness: -1,
        }]
    );

    for (id, mana_cost, card_colors, card_types) in [
        (
            "RAV-CLEANSING-BEAM",
            ManaCost::with_colors(4, [Color::Red]),
            [Color::Red].into_iter().collect::<BTreeSet<_>>(),
            [CardType::Sorcery].into_iter().collect(),
        ),
        (
            "RAV-RALLY-THE-RIGHTEOUS",
            ManaCost::with_colors(1, [Color::Red, Color::White]),
            [Color::Red, Color::White].into_iter().collect(),
            [CardType::Instant].into_iter().collect(),
        ),
        (
            "RAV-WOJEK-SIREN",
            ManaCost::with_colors(0, [Color::White]),
            [Color::White].into_iter().collect(),
            [CardType::Instant].into_iter().collect(),
        ),
    ] {
        let definition = definitions
            .iter()
            .find(|definition| definition.id == id)
            .expect("radiance definition exists");
        assert_eq!(definition.mana_cost, mana_cost, "{id}");
        assert_eq!(definition.colors, card_colors, "{id}");
        assert_eq!(definition.card_types, card_types, "{id}");
    }

    for id in ["RAV-GATHER-COURAGE", "RAV-SEEDS-OF-STRENGTH"] {
        let definition = definitions
            .iter()
            .find(|definition| definition.id == id)
            .expect("full-fidelity definition exists");
        assert_eq!(definition.supported_rules[0], "full-rules-fidelity", "{id}");
    }
}

#[test]
#[allow(clippy::too_many_lines)] // The complete public scenario receipt matrix is intentionally atomic.
fn full_fidelity_card_scenarios_emit_their_complete_effect_receipts() {
    let results = run_all_scenarios().expect("public RAV scenarios run");
    let scenarios: &[(&str, &[&str])] = &[
        (
            "rav_stack_lightning_helix",
            &["DamageDealtToPlayer", "LifeGained"],
        ),
        (
            "rav_last_gasp_state_based_action",
            &["ContinuousEffectCreated", "StateBasedAction"],
        ),
        (
            "rav_elves_of_deep_shadow_complete_mana_ability",
            &["ManaAdded", "DamageDealtToPlayer"],
        ),
        (
            "rav_boros_recruit_hybrid_first_strike",
            &["SpellCast", "FirstStrikeCombatDamage"],
        ),
        (
            "rav_watchwolf_colored_cost",
            &["SpellCast", "SpellResolved"],
        ),
        (
            "rav_glass_golem_colorless_cost",
            &["SpellCast", "SpellResolved"],
        ),
        (
            "rav_cleansing_beam_radiance_damage",
            &["DamageDealtToPermanent", "StateBasedAction"],
        ),
        (
            "rav_wojek_siren_radiance",
            &["ContinuousEffectCreated", "SpellResolved"],
        ),
        (
            "rav_radiance_cleanup_expiration",
            &["PermanentsUntapped", "ContinuousEffectExpired"],
        ),
        (
            "rav_rain_of_embers_global_damage",
            &["DamageDealtToPermanent", "StateBasedAction"],
        ),
        (
            "rav_dogpile_combat_count_damage",
            &["DamageDealtToPermanent", "StateBasedAction"],
        ),
        (
            "rav_overwhelm_convoke_wide_modifier",
            &["ConvokeUsed", "ContinuousEffectCreated"],
        ),
        (
            "rav_moldervine_cloak_persistent_attachment",
            &["AuraAttached", "SpellResolved"],
        ),
        (
            "rav_clinging_darkness_persistent_attachment",
            &["AuraAttached", "SpellResolved"],
        ),
        (
            "rav_disembowel_policy_chosen_x",
            &[
                "PolicyMoveSubmitted",
                "SpellManaPaid",
                "CardDestroyed",
                "SpellResolved",
            ],
        ),
    ];
    for &(id, markers) in scenarios {
        let result = results
            .iter()
            .find(|result| result.id == id)
            .expect("scenario exists");
        for marker in markers {
            assert!(
                result.event_log.iter().any(|event| event.contains(marker)),
                "{id} lacks {marker}"
            );
        }
    }

    assert_darkblast_trace(&results);

    let rejected_target = results
        .iter()
        .find(|result| result.id == "rav_cleansing_beam_requires_creature_target")
        .expect("Cleansing Beam target-legality scenario exists");
    assert!(
        rejected_target.event_log.is_empty(),
        "an invalid target must not put the spell on the stack or emit receipts"
    );
}

fn assert_darkblast_trace(results: &[cardbench_magic_rav::ScenarioResult]) {
    let darkblast = results
        .iter()
        .find(|result| result.id == "rav_darkblast_modifier_and_dredge")
        .expect("Darkblast scenario exists");
    assert_eq!(darkblast.digest, "fnv1a64:de22311339e6cdf5");
    for marker in [
        "SpellCast",
        "ContinuousEffectCreated",
        "SpellResolved",
        "Dredged",
    ] {
        assert!(
            darkblast
                .event_log
                .iter()
                .any(|event| event.contains(marker)),
            "rav_darkblast_modifier_and_dredge lacks {marker}"
        );
    }
    let effect = darkblast
        .event_log
        .iter()
        .position(|event| event.contains("ContinuousEffectCreated"))
        .expect("Darkblast modifier receipt");
    let resolved = darkblast
        .event_log
        .iter()
        .position(|event| event.contains("SpellResolved"))
        .expect("Darkblast resolution receipt");
    let dredged = darkblast
        .event_log
        .iter()
        .position(|event| event.contains("Dredged"))
        .expect("Darkblast Dredge receipt");
    assert!(
        effect < resolved && resolved < dredged,
        "Darkblast must resolve before its later Dredge replacement"
    );
}
