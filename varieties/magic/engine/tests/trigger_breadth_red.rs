//! Red regressions for trigger conditions that require captured event provenance.
//!
//! These synthetic definitions deliberately model rules shapes rather than RAV
//! card names.  They must fail until the generic trigger scheduler can retain
//! the creature that received combat damage and observe a card entering an
//! opponent's graveyard from any zone.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, CombatBlock, Effect, Game, ManaCost, PlayerId,
    Target, TargetRequirement, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

fn card(
    id: &'static str,
    types: BTreeSet<CardType>,
    power: Option<i16>,
    toughness: Option<i16>,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Black]),
        mana_colors: BTreeSet::new(),
        card_types: types,
        is_basic_land: false,
        supported_rules: &["synthetic-trigger-breadth"],
        power,
        toughness,
        keywords: vec![],
        effects,
    }
}

fn advance_to_declare_attackers(game: &mut Game) {
    for player in [
        PlayerId(0),
        PlayerId(1),
        PlayerId(0),
        PlayerId(1),
        PlayerId(0),
        PlayerId(1),
        PlayerId(0),
        PlayerId(1),
    ] {
        game.pass_priority(player).expect("advance turn step");
    }
}

#[test]
fn combat_damage_trigger_must_retain_the_creature_that_received_damage() {
    let definitions = vec![
        card(
            "COMBAT-DAMAGE-SOURCE",
            BTreeSet::from([CardType::Creature]),
            Some(1),
            Some(3),
            vec![],
        ),
        card(
            "COMBAT-DAMAGE-RECIPIENT",
            BTreeSet::from([CardType::Creature]),
            Some(1),
            Some(3),
            vec![],
        ),
        card(
            "UNRELATED-CREATURE",
            BTreeSet::from([CardType::Creature]),
            Some(1),
            Some(3),
            vec![],
        ),
    ];
    // The current broad DealsDamage condition has no event-recipient payload,
    // so it incorrectly asks the controller to choose *any* creature.  Green
    // replaces this compatibility binding with a combat-recipient condition
    // and a non-targeting "that creature" effect.
    let binding = TriggeredAbilityBinding {
        card_definition: "COMBAT-DAMAGE-SOURCE",
        ability: TriggeredAbility {
            id: "destroy-combat-damaged-creature",
            condition: TriggerCondition::DealsDamage,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![TargetRequirement::DistinctCreature],
            effects: vec![Effect::DestroyDistinctTargetCreature],
        },
    };
    let mut game = Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        [],
        [],
        [],
        [],
        [binding],
    )
    .expect("synthetic trigger game builds");
    let source = game
        .add_card(PlayerId(0), "COMBAT-DAMAGE-SOURCE", Zone::Battlefield)
        .expect("source begins on battlefield");
    let recipient = game
        .add_card(PlayerId(1), "COMBAT-DAMAGE-RECIPIENT", Zone::Battlefield)
        .expect("recipient begins on battlefield");
    let unrelated = game
        .add_card(PlayerId(1), "UNRELATED-CREATURE", Zone::Battlefield)
        .expect("unrelated creature begins on battlefield");
    game.set_entered_turn_for_setup(source, 0)
        .expect("source predates combat turn");
    game.begin_game().expect("game starts");
    advance_to_declare_attackers(&mut game);
    game.declare_attackers(PlayerId(0), &[source])
        .expect("declare attacker");
    game.pass_priority(PlayerId(0)).expect("pass attackers");
    game.pass_priority(PlayerId(1)).expect("pass attackers");
    game.declare_blockers(
        PlayerId(1),
        &[CombatBlock {
            attacker: source,
            blocker: recipient,
        }],
    )
    .expect("declare blocker");
    game.pass_priority(PlayerId(0)).expect("pass blockers");
    game.pass_priority(PlayerId(1)).expect("resolve combat damage");

    let view = game
        .view_for_player(PlayerId(0))
        .expect("source controller view");
    println!("combat-recipient red trace: {:?}", game.canonical_event_log());
    assert!(
        view.triggered_ability_target_choice.is_none(),
        "combat damage must identify its recipient; it cannot open a free target choice such as {unrelated:?}"
    );
    assert_eq!(
        game.zone_of(unrelated),
        Some(Zone::Battlefield),
        "only the creature that received combat damage may be affected"
    );
}

#[test]
fn opponent_graveyard_trigger_observes_a_discard_from_hand() {
    let definitions = vec![
        card(
            "OPPONENT-GRAVEYARD-OBSERVER",
            BTreeSet::from([CardType::Creature]),
            Some(1),
            Some(1),
            vec![],
        ),
        card(
            "DISCARD-SPELL",
            BTreeSet::from([CardType::Sorcery]),
            None,
            None,
            vec![Effect::DiscardTargetPlayer { count: 1 }],
        ),
        card(
            "DISCARDED-CARD",
            BTreeSet::from([CardType::Instant]),
            None,
            None,
            vec![Effect::DrawController],
        ),
    ];
    // The existing battlefield-death observer intentionally cannot see a
    // hand-to-graveyard move. Green replaces it with the generic opponent
    // graveyard condition, which applies regardless of the prior zone.
    let binding = TriggeredAbilityBinding {
        card_definition: "OPPONENT-GRAVEYARD-OBSERVER",
        ability: TriggeredAbility {
            id: "opponent-card-to-graveyard-grow",
            condition: TriggerCondition::AnotherCreatureDies,
            mana_cost: ManaCost::new(0),
            optional: false,
            targets: vec![],
            effects: vec![Effect::AddPlusOneCounterToSource],
        },
    };
    let mut game = Game::new_with_all_bindings_and_triggers(
        definitions,
        2,
        [],
        [],
        [],
        [],
        [binding],
    )
    .expect("synthetic trigger game builds");
    let observer = game
        .add_card(PlayerId(0), "OPPONENT-GRAVEYARD-OBSERVER", Zone::Battlefield)
        .expect("observer begins on battlefield");
    let discard = game
        .add_card(PlayerId(0), "DISCARD-SPELL", Zone::Hand)
        .expect("discard spell begins in hand");
    let discarded = game
        .add_card(PlayerId(1), "DISCARDED-CARD", Zone::Hand)
        .expect("opponent card begins in hand");
    game.begin_game().expect("game starts");
    for player in [PlayerId(0), PlayerId(1), PlayerId(0), PlayerId(1)] {
        game.pass_priority(player).expect("advance to main phase");
    }
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: discard,
            targets: vec![Target::Player(PlayerId(1))],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("cast discard spell");
    game.pass_priority(PlayerId(0)).expect("caster passes");
    game.pass_priority(PlayerId(1)).expect("resolve discard spell");
    game.pass_priority(PlayerId(0)).expect("pass pending trigger");
    game.pass_priority(PlayerId(1)).expect("resolve pending trigger");

    println!("opponent-graveyard red trace: {:?}", game.canonical_event_log());
    assert_eq!(game.zone_of(discarded), Some(Zone::Graveyard));
    assert_eq!(
        game.characteristics(observer)
            .expect("observer remains live")
            .power,
        Some(2),
        "a card entering an opponent's graveyard from hand must trigger the observer"
    );
}
