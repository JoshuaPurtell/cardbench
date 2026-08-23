//! Red regression for policy-submitted spell color choices with stack provenance.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, GameEvent, Keyword, ManaCost,
    PlayerId, PolicyAction, Target, TargetRequirement, Zone,
};

const TEAM_PROTECTION: &str = "TST-TEAM-PROTECTION";
const ALLY: &str = "TST-TEAM-PROTECTION-ALLY";
const RED_DAMAGE: &str = "TST-TEAM-PROTECTION-RED-DAMAGE";
const GREEN_DAMAGE: &str = "TST-TEAM-PROTECTION-GREEN-DAMAGE";

fn creature(id: &'static str, colors: BTreeSet<Color>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors,
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["spell-color-choice-contract"],
        power: Some(5),
        toughness: Some(5),
        keywords: vec![],
        effects: vec![],
    }
}

fn damage(id: &'static str, color: Color) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([color]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["spell-color-choice-contract"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::DealDamage {
            amount: 2,
            target: TargetRequirement::Creature,
        }],
    }
}

fn definitions() -> Vec<CardDefinition> {
    vec![
        CardDefinition {
            id: TEAM_PROTECTION,
            name: TEAM_PROTECTION,
            set_code: "TST",
            mana_cost: ManaCost::new(0),
            colors: BTreeSet::from([Color::White]),
            mana_colors: BTreeSet::new(),
            card_types: BTreeSet::from([CardType::Instant]),
            is_basic_land: false,
            supported_rules: &["spell-color-choice-contract"],
            power: None,
            toughness: None,
            keywords: vec![],
            effects: vec![Effect::AddChosenColorProtectionToControllerCreaturesUntilEndOfTurn],
        },
        creature(ALLY, BTreeSet::from([Color::White])),
        damage(RED_DAMAGE, Color::Red),
        damage(GREEN_DAMAGE, Color::Green),
    ]
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

#[test]
#[allow(clippy::too_many_lines)] // The public trace covers selection, protection, and target legality.
fn policy_submitted_spell_color_choice_is_retained_and_grants_matching_protection() {
    let mut game = Game::new(definitions(), 2).expect("fixture builds once color choice exists");
    let ally = game
        .put_on_battlefield(PlayerId(0), ALLY)
        .expect("ally setup");
    let protection = game
        .add_card(PlayerId(0), TEAM_PROTECTION, Zone::Hand)
        .expect("protection spell setup");
    let red_damage = game
        .add_card(PlayerId(1), RED_DAMAGE, Zone::Hand)
        .expect("red damage setup");
    let green_damage = game
        .add_card(PlayerId(1), GREEN_DAMAGE, Zone::Hand)
        .expect("green damage setup");

    let missing_choice = game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: protection,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    );
    assert!(
        missing_choice.is_err(),
        "a choosing spell cannot default a color"
    );
    assert_eq!(game.zone_of(protection), Some(Zone::Hand));
    assert!(game.stack.is_empty());

    game.submit_policy_move(
        PlayerId(0),
        "test-choice-policy",
        PolicyAction::CastWithColorChoice {
            request: CastRequest {
                card: protection,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            color: Color::Colorless,
        },
    )
    .expect_err("colorless is a mana kind, not a selectable card color");
    assert_eq!(game.zone_of(protection), Some(Zone::Hand));
    assert!(game.stack.is_empty());

    game.submit_policy_move(
        PlayerId(0),
        "test-choice-policy",
        PolicyAction::CastWithColorChoice {
            request: CastRequest {
                card: protection,
                targets: vec![],
                convoke: vec![],
                payment_mana_abilities: vec![],
            },
            color: Color::Red,
        },
    )
    .expect("policy supplies the chosen color while casting");
    pass_pair(&mut game);

    assert!(
        game.characteristics(ally)
            .expect("ally has live characteristics")
            .keywords
            .contains(&Keyword::Protection(Color::Red))
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellColorChosen { player, card, color }
            if *player == PlayerId(0) && *card == protection && *color == Color::Red
    )));

    game.pass_priority(PlayerId(0))
        .expect("opponent receives priority");
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: red_damage,
            targets: vec![Target::Permanent(ally)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect_err("matching-color spell cannot target protected ally");
    assert_eq!(game.zone_of(red_damage), Some(Zone::Hand));

    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: green_damage,
            targets: vec![Target::Permanent(ally)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("nonmatching spell targets ally");
    pass_pair(&mut game);
    assert_eq!(game.object(ally).expect("ally remains").damage, 2);
    eprintln!("spell color choice trace={:?}", game.canonical_event_log());
    game.validate_invariants()
        .expect("chosen color stack provenance preserves invariants");
}
