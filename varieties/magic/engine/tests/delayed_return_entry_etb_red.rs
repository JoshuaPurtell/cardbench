//! Red regression: a delayed blink return must preserve its creature's ETB.
//!
//! The end-step delayed action performs a real exile-to-battlefield transition.
//! Its entry event must be captured while the returned incarnation is live,
//! reach the shared SBA boundary, then stack normally before a priority window.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, ManaCost, PlayerId, Step,
    Target, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const CREATURE: &str = "TST-DELAYED-RETURN-ETB-CREATURE";
const BLINK: &str = "TST-DELAYED-RETURN-ETB-BLINK";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    let creature = card_type == CardType::Creature;
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["delayed-return-entry-etb-red"],
        power: creature.then_some(2),
        toughness: creature.then_some(2),
        keywords: vec![],
        effects,
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first pass");
    let second = game.priority;
    game.pass_priority(second).expect("second pass");
}

fn advance_to_end_step(game: &mut Game) {
    while game.step != Step::End {
        if game.step == Step::DeclareAttackers {
            game.declare_attackers(game.active_player, &[])
                .expect("empty attacker declaration");
        }
        pass_pair(game);
    }
}

#[test]
fn delayed_return_creature_stacks_its_historical_etb_at_end_step() {
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(CREATURE, CardType::Creature, vec![]),
            definition(
                BLINK,
                CardType::Instant,
                vec![Effect::ExileTargetCreatureUntilEndStep],
            ),
        ],
        2,
        [],
        [],
        [],
        [],
        [TriggeredAbilityBinding {
            card_definition: CREATURE,
            ability: TriggeredAbility {
                id: "returned-creature-etb",
                condition: TriggerCondition::EntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::GainLifeController { amount: 1 }],
            },
        }],
    )
    .expect("fixture initializes");
    let creature = game
        .put_on_battlefield(PlayerId(1), CREATURE)
        .expect("creature setup");
    let blink = game
        .add_card(PlayerId(0), BLINK, Zone::Hand)
        .expect("blink setup");
    game.begin_game().expect("game begins");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: blink,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("blink casts");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(creature), Some(Zone::Exile));

    game.clear_event_log();
    advance_to_end_step(&mut game);
    eprintln!("delayed return trace={:?}", game.canonical_event_log());

    let returned_incarnation = game
        .object(creature)
        .expect("creature returned")
        .incarnation;
    assert_eq!(game.zone_of(creature), Some(Zone::Battlefield));
    assert!(
        game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::TriggeredAbilityStacked {
                source,
                source_incarnation,
                ability: "returned-creature-etb",
                ..
            } if *source == creature && *source_incarnation == returned_incarnation
        )),
        "delayed return must retain and stack the returned creature's ETB"
    );
    game.validate_invariants()
        .expect("returned-creature ETB transition remains auditable");
}
