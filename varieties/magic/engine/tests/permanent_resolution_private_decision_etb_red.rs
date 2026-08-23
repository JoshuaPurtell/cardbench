//! Red regression: a permanent spell that completes through a private choice
//! must retain its ETB source before its post-entry SBA checkpoint.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DecisionSelection, Effect, Game, GameEvent, Keyword,
    ManaCost, PlayerId, Target, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const EPHEMERAL_DISCARDER: &str = "TST-PRIVATE-DECISION-EPHEMERAL";
const FILLER: &str = "TST-PRIVATE-DECISION-FILLER";

fn definition(id: &'static str) -> CardDefinition {
    let ephemeral = id == EPHEMERAL_DISCARDER;
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([if ephemeral {
            CardType::Creature
        } else {
            CardType::Instant
        }]),
        is_basic_land: false,
        supported_rules: &["permanent-private-decision-etb-red"],
        power: ephemeral.then_some(0),
        toughness: ephemeral.then_some(0),
        keywords: if ephemeral {
            vec![Keyword::Flash]
        } else {
            vec![]
        },
        effects: if ephemeral {
            vec![Effect::DiscardTargetPlayer { count: 1 }]
        } else {
            vec![]
        },
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first pass succeeds");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second pass resolves to the private discard choice");
}

#[test]
fn private_discard_permanent_entry_keeps_the_ephemeral_etb_incarnation() {
    let mut game = Game::new_with_all_bindings_and_triggers(
        [definition(EPHEMERAL_DISCARDER), definition(FILLER)],
        2,
        [],
        [],
        [],
        [],
        [TriggeredAbilityBinding {
            card_definition: EPHEMERAL_DISCARDER,
            ability: TriggeredAbility {
                id: "ephemeral-etb",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::GainLifeController { amount: 1 }],
            },
        }],
    )
    .expect("fixture constructs");
    let creature = game
        .add_card(PlayerId(0), EPHEMERAL_DISCARDER, Zone::Hand)
        .expect("ephemeral creature begins in hand");
    let discarded = game
        .add_card(PlayerId(1), FILLER, Zone::Hand)
        .expect("opponent has one discardable card");
    game.begin_game().expect("game begins");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: creature,
            targets: vec![Target::Player(PlayerId(1))],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("flash creature with private discard effect casts");
    pass_pair(&mut game);
    let decision = game
        .view_for_player(PlayerId(1))
        .expect("recipient private view")
        .pending_decision
        .expect("target player receives the private discard decision");

    let result = game.submit_decision(
        PlayerId(1),
        decision.id,
        DecisionSelection::Objects(vec![discarded]),
    );
    eprintln!(
        "private-decision permanent entry result={result:?}; trace={:?}",
        game.canonical_event_log()
    );
    result.expect("recipient supplies the one legal discard");

    let entry_move = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == creature))
        .expect("permanent spell entered the battlefield after the choice");
    let entry_incarnation = game.event_log[entry_move + 1..]
        .iter()
        .find_map(|event| match event {
            GameEvent::ObjectIncarnationAdvanced {
                object,
                incarnation,
            } if *object == creature => Some(*incarnation),
            _ => None,
        })
        .expect("battlefield entry has an incarnation receipt");
    let trigger_incarnation = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::TriggeredAbilityStacked {
                source,
                source_incarnation,
                ability,
                ..
            } if *source == creature && *ability == "ephemeral-etb" => Some(*source_incarnation),
            _ => None,
        })
        .expect("the ephemeral ETB must stack after its SBA death");
    assert_eq!(game.zone_of(creature), Some(Zone::Graveyard));
    assert_eq!(
        trigger_incarnation, entry_incarnation,
        "the ETB must retain its battlefield incarnation rather than the later graveyard incarnation",
    );
    game.validate_invariants()
        .expect("private-decision permanent entry retains auditable ETB provenance");
}
