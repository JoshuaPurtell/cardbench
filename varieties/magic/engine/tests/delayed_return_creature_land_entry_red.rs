//! Red regression: a delayed creature-land return is also a land-entry event.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, ManaCost, PlayerId, Step,
    Target, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const CREATURE_LAND: &str = "TST-DELAYED-RETURN-CREATURE-LAND";
const BLINK: &str = "TST-DELAYED-RETURN-CREATURE-LAND-BLINK";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
    effects: Vec<Effect>,
) -> CardDefinition {
    let creature = card_types.contains(&CardType::Creature);
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["delayed-return-creature-land-entry-red"],
        power: creature.then_some(2),
        toughness: creature.then_some(2),
        keywords: vec![],
        effects,
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

fn advance_to_end_step(game: &mut Game) {
    while game.step != Step::End {
        if game.step == Step::DeclareAttackers {
            game.declare_attackers(game.active_player, &[])
                .expect("empty attackers are declared explicitly");
        }
        pass_pair(game);
    }
}

#[test]
fn delayed_creature_land_return_captures_its_land_entry_trigger() {
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(
                CREATURE_LAND,
                BTreeSet::from([CardType::Creature, CardType::Land]),
                vec![],
            ),
            definition(
                BLINK,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::ExileTargetCreatureUntilEndStep],
            ),
        ],
        2,
        [],
        [],
        [],
        [],
        [TriggeredAbilityBinding {
            card_definition: CREATURE_LAND,
            ability: TriggeredAbility {
                id: "returned-creature-land-entry",
                condition: TriggerCondition::LandEntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::GainLifeController { amount: 1 }],
            },
        }],
    )
    .expect("fixture initializes");
    let creature_land = game
        .put_on_battlefield(PlayerId(1), CREATURE_LAND)
        .expect("creature-land setup");
    let blink = game
        .add_card(PlayerId(0), BLINK, Zone::Hand)
        .expect("blink setup");
    game.begin_game().expect("game begins");
    game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: blink,
            targets: vec![Target::Permanent(creature_land)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("blink casts");
    pass_pair(&mut game);
    assert_eq!(game.zone_of(creature_land), Some(Zone::Exile));

    game.clear_event_log();
    advance_to_end_step(&mut game);
    eprintln!(
        "delayed creature-land return trace={:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(creature_land), Some(Zone::Battlefield));
    assert!(
        game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::TriggeredAbilityStacked {
                source,
                ability: "returned-creature-land-entry",
                ..
            } if *source == creature_land
        )),
        "the delayed return must retain the creature-land's land-entry trigger"
    );
    game.validate_invariants()
        .expect("delayed creature-land return remains invariant-valid");
}
