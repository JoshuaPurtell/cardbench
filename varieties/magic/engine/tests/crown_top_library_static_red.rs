//! Red contract for a controller-owned top-library static rule and rotation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, ActivatedManaAbility,
    CardDefinition, CardType, Color, ContinuousChange, Effect, Game, GameEvent,
    ManaAbilityActivation, ManaAbilityBinding, ManaAbilityOutput, ManaCost, PlayerId,
    StaticContinuousEffectBinding, StaticLibraryTopRevealBinding, StaticLibraryTopRevealScope,
    Zone,
};

const CROWN: &str = "TST-CROWN";
const GREEN_CREATURE: &str = "TST-GREEN-CREATURE";
const BLUE_CREATURE: &str = "TST-BLUE-CREATURE";
const RED_CREATURE: &str = "TST-RED-CREATURE";
const NONCREATURE: &str = "TST-NONCREATURE";
const GREEN_SOURCE: &str = "TST-GREEN-SOURCE";
const WHITE_SOURCE: &str = "TST-WHITE-SOURCE";

fn definition(
    id: &'static str,
    colors: BTreeSet<Color>,
    card_types: BTreeSet<CardType>,
    power: Option<i16>,
    toughness: Option<i16>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors,
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["crown-top-library-static-contract"],
        power,
        toughness,
        keywords: vec![],
        effects: vec![],
    }
}

#[allow(clippy::too_many_lines)] // Fixture binds all three independent Crown rules explicitly.
fn game() -> Game {
    let mut game = Game::new_with_all_bindings_and_static_continuous_effects(
        [
            definition(
                CROWN,
                BTreeSet::new(),
                BTreeSet::from([CardType::Artifact]),
                None,
                None,
            ),
            definition(
                GREEN_CREATURE,
                BTreeSet::from([Color::Green]),
                BTreeSet::from([CardType::Creature]),
                Some(2),
                Some(2),
            ),
            definition(
                BLUE_CREATURE,
                BTreeSet::from([Color::Blue]),
                BTreeSet::from([CardType::Creature]),
                Some(2),
                Some(2),
            ),
            definition(
                RED_CREATURE,
                BTreeSet::from([Color::Red]),
                BTreeSet::from([CardType::Creature]),
                Some(2),
                Some(2),
            ),
            definition(
                NONCREATURE,
                BTreeSet::new(),
                BTreeSet::from([CardType::Artifact]),
                None,
                None,
            ),
            definition(
                GREEN_SOURCE,
                BTreeSet::new(),
                BTreeSet::from([CardType::Land]),
                None,
                None,
            ),
            definition(
                WHITE_SOURCE,
                BTreeSet::new(),
                BTreeSet::from([CardType::Land]),
                None,
                None,
            ),
        ],
        2,
        [
            ManaAbilityBinding {
                card_definition: GREEN_SOURCE,
                ability: ActivatedManaAbility {
                    id: "green",
                    tap_cost: true,
                    output: ManaAbilityOutput::Fixed(Color::Green),
                    amount: 1,
                    life_payment: None,
                    controller_damage: None,
                },
            },
            ManaAbilityBinding {
                card_definition: WHITE_SOURCE,
                ability: ActivatedManaAbility {
                    id: "white",
                    tap_cost: true,
                    output: ManaAbilityOutput::Fixed(Color::White),
                    amount: 1,
                    life_payment: None,
                    controller_damage: None,
                },
            },
        ],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: CROWN,
            ability: ActivatedAbility {
                id: "rotate-controller-library-top-to-bottom",
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
        }],
        [StaticContinuousEffectBinding {
            card_definition: CROWN,
            change: ContinuousChange::ControlledCreaturesSharingTopLibraryCreatureCardColorsModifyPowerToughness {
                power: 1,
                toughness: 1,
            },
        }],
    )
    .expect("fixture requires Crown static substrate");
    game.register_static_library_top_reveal_bindings([StaticLibraryTopRevealBinding {
        card_definition: CROWN,
        scope: StaticLibraryTopRevealScope::SourceController,
    }])
    .expect("source-controller top reveal registers");
    game
}

#[test]
#[allow(clippy::too_many_lines)] // Visibility, layer state, payment, and event ordering are one contract.
fn controller_top_creature_reveals_only_that_library_buffs_shared_colors_and_rotates() {
    let mut game = game();
    let crown = game
        .put_on_battlefield(PlayerId(0), CROWN)
        .expect("Crown setup");
    let green = game
        .put_on_battlefield(PlayerId(0), GREEN_CREATURE)
        .expect("green recipient setup");
    let blue = game
        .put_on_battlefield(PlayerId(0), BLUE_CREATURE)
        .expect("blue nonrecipient setup");
    let red = game
        .put_on_battlefield(PlayerId(1), RED_CREATURE)
        .expect("opponent setup");
    let bottom = game
        .add_card(PlayerId(0), NONCREATURE, Zone::Library)
        .expect("controller library bottom");
    let top = game
        .add_card(PlayerId(0), GREEN_CREATURE, Zone::Library)
        .expect("controller creature library top");
    let opponent_top = game
        .add_card(PlayerId(1), RED_CREATURE, Zone::Library)
        .expect("opponent library top remains hidden");
    let green_source = game
        .put_on_battlefield(PlayerId(0), GREEN_SOURCE)
        .expect("green source setup");
    let white_source = game
        .put_on_battlefield(PlayerId(0), WHITE_SOURCE)
        .expect("white source setup");
    game.begin_game().expect("game starts");

    assert_eq!(
        game.characteristics(green)
            .expect("green characteristics")
            .power,
        Some(3)
    );
    assert_eq!(
        game.characteristics(blue)
            .expect("blue characteristics")
            .power,
        Some(2)
    );
    assert_eq!(
        game.characteristics(red)
            .expect("red characteristics")
            .power,
        Some(2)
    );
    let visible = game
        .view_for_player(PlayerId(1))
        .expect("opponent public view")
        .revealed_library_tops;
    assert_eq!(visible.len(), 1, "Crown never reveals an opponent library");
    assert_eq!(visible[0].owner, PlayerId(0));
    assert_eq!(visible[0].card.id, top);

    game.activate_bound_mana_ability(
        PlayerId(0),
        ManaAbilityActivation {
            source: green_source,
            ability_id: "green",
            chosen_color: None,
        },
    )
    .expect("green source produces mana");
    game.activate_bound_mana_ability(
        PlayerId(0),
        ManaAbilityActivation {
            source: white_source,
            ability_id: "white",
            chosen_color: None,
        },
    )
    .expect("white source produces mana");

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: crown,
            ability_id: "rotate-controller-library-top-to-bottom",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Crown activation is stack-backed");
    game.pass_priority(PlayerId(0)).expect("controller passes");
    game.pass_priority(PlayerId(1)).expect("opponent passes");

    assert_eq!(game.players[0].library, vec![top, bottom]);
    assert_eq!(
        game.characteristics(green)
            .expect("green characteristics")
            .power,
        Some(2)
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::LibraryTopMovedToBottom { player: PlayerId(0), card } if *card == top
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == crown && *ability == "rotate-controller-library-top-to-bottom"
    )));
    assert!(
        game.view_for_player(PlayerId(1))
            .expect("opponent public view after rotation")
            .revealed_library_tops
            .iter()
            .all(|view| view.owner == PlayerId(0) && view.card.id != opponent_top)
    );
    eprintln!("crown_top_library_trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("Crown static and rotation lifecycle remains invariant-valid");
}
