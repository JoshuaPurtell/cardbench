//! RED regression: simultaneous creature deaths must preserve each dying
//! "another creature dies" source as last-known information.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, GameEvent, ManaCost, PlayerId,
    TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const OBSERVER: &str = "TST-SIMULTANEOUS-DIES-OBSERVER";
const SWEEP: &str = "TST-SIMULTANEOUS-DIES-SWEEP";

fn definition(
    id: &'static str,
    card_types: impl IntoIterator<Item = CardType>,
    toughness: Option<i16>,
    effects: Vec<Effect>,
) -> CardDefinition {
    let card_types: BTreeSet<_> = card_types.into_iter().collect();
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: card_types.clone(),
        is_basic_land: false,
        supported_rules: &["synthetic-simultaneous-dies-lki-contract"],
        power: card_types.contains(&CardType::Creature).then_some(1),
        toughness,
        keywords: vec![],
        effects,
    }
}

#[test]
fn simultaneous_deaths_stack_another_dies_triggers_from_both_lki_sources() {
    let binding = TriggeredAbilityBinding {
        card_definition: OBSERVER,
        ability: TriggeredAbility {
            id: "another-creature-died",
            condition: TriggerCondition::AnotherCreatureDies,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::AddPlusOneCounterToSource],
        },
    };
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(OBSERVER, [CardType::Creature], Some(1), vec![]),
            definition(
                SWEEP,
                [CardType::Sorcery],
                None,
                vec![Effect::DealDamageToEachCreatureAndPlayer { amount: 1 }],
            ),
        ],
        2,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        [binding],
    )
    .expect("two-player fixture initializes");
    let first = game
        .put_on_battlefield(PlayerId(0), OBSERVER)
        .expect("first observer enters");
    let second = game
        .put_on_battlefield(PlayerId(1), OBSERVER)
        .expect("second observer enters");
    let sweep = game
        .add_card(PlayerId(0), SWEEP, Zone::Hand)
        .expect("sweep enters the caster hand");

    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: sweep,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("the global sweep casts");
    game.pass_priority(PlayerId(0))
        .expect("caster passes the sweep");
    game.pass_priority(PlayerId(1))
        .expect("opponent resolves the sweep");

    assert_eq!(game.zone_of(first), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(second), Some(Zone::Graveyard));
    let trigger_sources = game
        .event_log
        .iter()
        .filter_map(|event| match event {
            GameEvent::TriggeredAbilityStacked {
                source, ability, ..
            } if *ability == "another-creature-died" => Some(*source),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    println!(
        "simultaneous-another-dies red trace: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(
        trigger_sources,
        BTreeSet::from([first, second]),
        "both creatures must observe the other simultaneous death through LKI"
    );
    game.validate_invariants()
        .expect("simultaneous death trigger provenance stays valid");
}
