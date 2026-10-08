//! Red regression for Bathe in Light's targeted, cross-controller Radiance recipients.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, GameEvent, Keyword, ManaCost,
    PlayerId, PolicyAction, Target, TargetRequirement, Zone,
};

const BATHE: &str = "TST-RADIANCE-PROTECTION";
const WHITE_TARGET: &str = "TST-WHITE-TARGET";
const GREEN_WHITE_ALLY: &str = "TST-GREEN-WHITE-ALLY";
const BLACK_NONMATCH: &str = "TST-BLACK-NONMATCH";
const RED_WHITE_OPPONENT: &str = "TST-RED-WHITE-OPPONENT";
const BLUE_NONMATCH: &str = "TST-BLUE-NONMATCH";
const BLUE_BOLT: &str = "TST-BLUE-BOLT";

fn creature(id: &'static str, colors: &[Color]) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: colors.iter().copied().collect(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["targeted-radiance-chosen-color-protection"],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

fn definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: BATHE,
            name: BATHE,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["targeted-radiance-chosen-color-protection"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::TargetedBundle {
                target: TargetRequirement::Creature,
                effects: vec![
                    Effect::RadianceAddChosenColorProtectionToTargetAndSharedColorCreaturesUntilEndOfTurn,
                ],
            }],
        },
        creature(WHITE_TARGET, &[Color::White]),
        creature(GREEN_WHITE_ALLY, &[Color::Green, Color::White]),
        creature(BLACK_NONMATCH, &[Color::Black]),
        creature(RED_WHITE_OPPONENT, &[Color::Red, Color::White]),
        creature(BLUE_NONMATCH, &[Color::Blue]),
        CardDefinition {
            id: BLUE_BOLT,
            name: BLUE_BOLT,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::Blue]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["targeted-radiance-chosen-color-protection"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::DealDamage {
                amount: 2,
                target: TargetRequirement::Creature,
            }],
        },
    ]
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

fn has_protection(game: &Game, creature: cardbench_magic_engine::ObjectId, color: Color) -> bool {
    game.characteristics(creature)
        .expect("creature has live characteristics")
        .keywords
        .contains(&Keyword::Protection(color))
}

#[test]
fn chosen_color_radiance_targets_and_protects_matching_creatures_across_controllers() {
    let mut game = Game::new(definitions(), 2).expect("fixture builds");
    let target = game
        .put_on_battlefield(PlayerId(0), WHITE_TARGET)
        .expect("white target setup");
    let ally = game
        .put_on_battlefield(PlayerId(0), GREEN_WHITE_ALLY)
        .expect("matching allied creature setup");
    let black = game
        .put_on_battlefield(PlayerId(0), BLACK_NONMATCH)
        .expect("nonmatching allied creature setup");
    let opponent_match = game
        .put_on_battlefield(PlayerId(1), RED_WHITE_OPPONENT)
        .expect("matching opposing creature setup");
    let opponent_nonmatch = game
        .put_on_battlefield(PlayerId(1), BLUE_NONMATCH)
        .expect("nonmatching opposing creature setup");
    let bathe = game
        .add_card(PlayerId(0), BATHE, Zone::Hand)
        .expect("Bathe in Light setup");
    let blue_bolt = game
        .add_card(PlayerId(0), BLUE_BOLT, Zone::Hand)
        .expect("blue spell setup");

    let request = CastRequest {
        card: bathe,
        targets: vec![Target::Permanent(target)],
        convoke: vec![],
        payment_mana_abilities: vec![],
    };
    game.cast_spell(PlayerId(0), request.clone())
        .expect_err("the chosen card color cannot be defaulted");
    game.submit_policy_move(
        PlayerId(0),
        "test-radiance-choice-policy",
        PolicyAction::CastWithColorChoice {
            request,
            color: Color::Blue,
        },
    )
    .expect("cast declares a colored choice and one legal creature target");
    pass_pair(&mut game);

    assert!(has_protection(&game, target, Color::Blue));
    assert!(has_protection(&game, ally, Color::Blue));
    assert!(has_protection(&game, opponent_match, Color::Blue));
    assert!(!has_protection(&game, black, Color::Blue));
    assert!(!has_protection(&game, opponent_nonmatch, Color::Blue));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellColorChosen { player, card, color }
            if *player == PlayerId(0) && *card == bathe && *color == Color::Blue
    )));

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: blue_bolt,
            targets: vec![Target::Permanent(opponent_match)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect_err("Radiance protection applies to matching opposing creatures too");
    assert_eq!(game.zone_of(blue_bolt), Some(Zone::Hand));
    game.validate_invariants()
        .expect("cross-controller Radiance grants preserve invariants");
}
