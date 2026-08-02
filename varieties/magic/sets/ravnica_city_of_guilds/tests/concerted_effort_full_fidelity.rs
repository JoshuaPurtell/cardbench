//! Event-log contracts for Concerted Effort's upkeep keyword snapshot.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Game, GameEvent, Keyword, ManaCost, PlayerId, Step, Zone,
};
use cardbench_magic_rav::{
    RAV_FULL_FIDELITY_DEFINITION_IDS, card_definitions, rav_activated_ability_bindings,
    rav_additional_spell_cost_bindings, rav_basic_land_type_bindings, rav_mana_ability_bindings,
    rav_triggered_ability_bindings,
};

const FLYER: &str = "TEST-CONCERTED-FLYER";
const FIRST_STRIKER: &str = "TEST-CONCERTED-FIRST-STRIKER";
const DOUBLE_STRIKER: &str = "TEST-CONCERTED-DOUBLE-STRIKER";
const ISLANDWALKER: &str = "TEST-CONCERTED-ISLANDWALKER";
const PROTECTED: &str = "TEST-CONCERTED-PROTECTED";
const TRAMPLER: &str = "TEST-CONCERTED-TRAMPLER";
const BLANK: &str = "TEST-CONCERTED-BLANK";

fn fixture_creature(id: &'static str, keywords: Vec<Keyword>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["test-fixture"],
        power: Some(1),
        toughness: Some(1),
        keywords,
        effects: vec![],
    }
}

fn game_with_concerted_fixture_creatures() -> Game {
    let mut definitions = card_definitions();
    definitions.extend([
        fixture_creature(FLYER, vec![Keyword::Flying]),
        fixture_creature(FIRST_STRIKER, vec![Keyword::FirstStrike]),
        fixture_creature(DOUBLE_STRIKER, vec![Keyword::DoubleStrike]),
        fixture_creature(
            ISLANDWALKER,
            vec![Keyword::Landwalk(
                cardbench_magic_engine::BasicLandType::Island,
            )],
        ),
        fixture_creature(PROTECTED, vec![Keyword::Protection(Color::Blue)]),
        fixture_creature(TRAMPLER, vec![Keyword::Trample]),
        fixture_creature(BLANK, vec![]),
    ]);
    Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        rav_mana_ability_bindings(),
        rav_basic_land_type_bindings(),
        rav_additional_spell_cost_bindings(),
        rav_activated_ability_bindings(),
        rav_triggered_ability_bindings(),
    )
    .expect("RAV fixture game builds")
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first priority pass");
    let second = game.priority;
    game.pass_priority(second).expect("second priority pass");
}

fn advance_one_policy_action(game: &mut Game) {
    let active = game.active_player;
    if game
        .view_for_player(active)
        .expect("active player view")
        .draw_replacement_pending
    {
        game.draw_card(active, None)
            .expect("ordinary draw resolves before priority");
        return;
    }
    if game.step == Step::DeclareAttackers
        && !game
            .view_for_player(active)
            .expect("active combat view")
            .attackers_declared
    {
        game.declare_attackers(active, &[])
            .expect("empty attacker declaration");
        return;
    }
    if game.step == Step::DeclareBlockers
        && !game
            .view_for_player(game.next_policy_player())
            .expect("defender combat view")
            .blockers_declared
    {
        let defender = game.next_policy_player();
        game.declare_blockers(defender, &[])
            .expect("empty blocker declaration");
        return;
    }
    let player = game.priority;
    game.pass_priority(player)
        .expect("priority holder advances turn structure");
}

fn advance_to(game: &mut Game, turn: u32, step: Step) {
    for _ in 0..64 {
        if game.turn == turn && game.step == step {
            return;
        }
        advance_one_policy_action(game);
        game.validate_invariants()
            .expect("turn transition preserves invariants");
    }
    panic!(
        "turn machine did not reach turn {turn} step {step:?}; reached turn {} step {:?}",
        game.turn, game.step
    );
}

#[test]
#[allow(clippy::too_many_lines)] // One trace verifies the complete selected-keyword family.
fn concerted_effort_snapshots_other_creatures_keyword_instances_until_cleanup() {
    let controller = PlayerId(0);
    let mut game = game_with_concerted_fixture_creatures();
    let concerted = game
        .put_on_battlefield(controller, "RAV-CONCERTED-EFFORT")
        .expect("Concerted Effort setup");
    let flyer = game
        .put_on_battlefield(controller, FLYER)
        .expect("flying source setup");
    let first_striker = game
        .put_on_battlefield(controller, FIRST_STRIKER)
        .expect("first-strike source setup");
    let double_striker = game
        .put_on_battlefield(controller, DOUBLE_STRIKER)
        .expect("double-strike source setup");
    let islandwalker = game
        .put_on_battlefield(controller, ISLANDWALKER)
        .expect("landwalk source setup");
    let protected = game
        .put_on_battlefield(controller, PROTECTED)
        .expect("protection source setup");
    let trampler = game
        .put_on_battlefield(controller, TRAMPLER)
        .expect("trample source setup");
    let blank = game
        .put_on_battlefield(controller, BLANK)
        .expect("blank creature setup");
    game.add_card(controller, "RAV-PLAINS", Zone::Library)
        .expect("turn-one draw setup");

    game.begin_game().expect("game begins at upkeep");
    assert_eq!(game.step, Step::Upkeep);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == concerted && *ability == "upkeep-share-controller-creature-keywords"
    )));
    pass_pair(&mut game);

    let expected = [
        Keyword::Flying,
        Keyword::FirstStrike,
        Keyword::DoubleStrike,
        Keyword::Landwalk(cardbench_magic_engine::BasicLandType::Island),
        Keyword::Protection(Color::Blue),
        Keyword::Trample,
    ];
    for creature in [
        flyer,
        first_striker,
        double_striker,
        islandwalker,
        protected,
        trampler,
        blank,
    ] {
        let keywords = &game
            .characteristics(creature)
            .expect("fixture creature survives")
            .keywords;
        for keyword in &expected {
            assert!(
                keywords.contains(keyword),
                "recipient {creature:?} receives {keyword:?} from an other creature"
            );
        }
    }
    let flyer_keywords = &game
        .characteristics(flyer)
        .expect("flyer survives")
        .keywords;
    assert_eq!(
        flyer_keywords
            .iter()
            .filter(|keyword| **keyword == Keyword::Flying)
            .count(),
        1,
        "the flyer cannot use itself to produce a duplicate Flying grant"
    );
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(
                event,
                GameEvent::ContinuousEffectCreated { source, layer, .. }
                    if *source == concerted && *layer == cardbench_magic_engine::Layer::Ability
            ))
            .count(),
        36,
        "six singleton keyword providers grant five other recipients each, while the blank receives all six"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved { source, ability, .. }
            if *source == concerted && *ability == "upkeep-share-controller-creature-keywords"
    )));

    advance_to(&mut game, 2, Step::Upkeep);
    assert!(
        game.characteristics(blank)
            .expect("blank survives cleanup")
            .keywords
            .is_empty(),
        "every temporary shared keyword expires at end of turn"
    );
    assert_eq!(
        game.event_log
            .iter()
            .filter(|event| matches!(
                event,
                GameEvent::ContinuousEffectExpired { source, layer, .. }
                    if *source == concerted && *layer == cardbench_magic_engine::Layer::Ability
            ))
            .count(),
        36,
        "cleanup records every one of the snapshot grants"
    );
    println!(
        "concerted_effort_event_log={:#?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("Concerted Effort trace preserves trigger, layer, and cleanup invariants");
}

#[test]
fn concerted_effort_is_positive_full_fidelity_manifest_entry() {
    assert!(RAV_FULL_FIDELITY_DEFINITION_IDS.contains(&"RAV-CONCERTED-EFFORT"));
}
