//! Red regression: a begin-game SBA pass must not panic or retain its
//! death/trigger-placement prefix when later trigger placement rejects.

use std::{collections::BTreeSet, panic::AssertUnwindSafe};

use cardbench_magic_engine::{
    CardDefinition, CardType, Color, Effect, Game, ManaCost, PlayerId, RulesError,
    TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const DYING: &str = "TST-PUBLIC-SBA-ATOMIC-DYING";
const TARGET: &str = "TST-PUBLIC-SBA-ATOMIC-TARGET";

fn creature(id: &'static str, color: Color, toughness: i16) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([color]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["public-sba-trigger-atomicity-red"],
        power: Some(1),
        toughness: Some(toughness),
        keywords: vec![],
        effects: vec![],
    }
}

#[test]
fn begin_game_rejects_an_unrepresentable_dies_trigger_without_committing_its_prefix() {
    let target_count = usize::from(u8::MAX) + 1;
    let binding = TriggeredAbilityBinding {
        card_definition: DYING,
        ability: TriggeredAbility {
            id: "too-many-dies-trigger-targets",
            condition: TriggerCondition::Dies,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::OpponentCreature; target_count],
            effects: (0..target_count)
                .map(|_| Effect::ReturnOpponentCreatureToHand)
                .collect(),
        },
    };
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            creature(DYING, Color::Black, 0),
            creature(TARGET, Color::Green, 1),
        ],
        2,
        [],
        [],
        [],
        [],
        [binding],
    )
    .expect("fixture initializes");
    let dying = game
        .add_card(PlayerId(0), DYING, Zone::Battlefield)
        .expect("zero-toughness creature begins on battlefield");
    game.add_card(PlayerId(1), TARGET, Zone::Battlefield)
        .expect("one legal opposing target exists");
    let before_events = game.canonical_event_log();

    let result = std::panic::catch_unwind(AssertUnwindSafe(|| game.begin_game()));
    eprintln!(
        "begin-game oversized dies-trigger result: {result:?}; zone={:?}; events={:?}",
        game.zone_of(dying),
        game.canonical_event_log(),
    );
    assert!(matches!(
        result,
        Ok(Err(RulesError::IllegalAction(
            "trigger target count exceeds decision range"
        )))
    ));
    assert_eq!(
        game.zone_of(dying),
        Some(Zone::Battlefield),
        "a rejected begin-game SBA pass must retain the creature before its death boundary"
    );
    assert_eq!(
        game.canonical_event_log(),
        before_events,
        "a rejected begin-game SBA pass must not retain state-based or trigger receipts"
    );
    game.validate_invariants()
        .expect("the rejected begin-game pass leaves an auditable pre-SBA state");
}
