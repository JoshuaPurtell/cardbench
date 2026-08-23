//! Red regression: targeted creature reanimation also captures land entry.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedManaAbility, CardDefinition, CardType, CastPaymentManaAbility, CastRequest, Color,
    Effect, Game, GameEvent, ManaAbilityActivation, ManaAbilityBinding, ManaAbilityOutput,
    ManaCost, ManaPaymentSelection, PlayerId, Target, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding, Zone,
};

const REANIMATE: &str = "TST-REANIMATE-CREATURE-LAND";
const CREATURE_LAND: &str = "TST-REANIMATED-CREATURE-LAND";
const MANA_SOURCE: &str = "TST-REANIMATE-CREATURE-LAND-MANA";

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
        mana_cost: ManaCost::new(u8::from(id == REANIMATE)),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["reanimate-creature-land-entry-red"],
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

#[test]
fn targeted_creature_land_reanimation_captures_its_land_entry_trigger() {
    let controller = PlayerId(0);
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(
                REANIMATE,
                BTreeSet::from([CardType::Instant]),
                vec![
                    Effect::ReturnTargetCreatureCardToBattlefieldWithCounterIfManaColorSpent {
                        color: cardbench_magic_engine::Color::Green,
                    },
                ],
            ),
            definition(
                CREATURE_LAND,
                BTreeSet::from([CardType::Creature, CardType::Land]),
                vec![],
            ),
            definition(MANA_SOURCE, BTreeSet::from([CardType::Artifact]), vec![]),
        ],
        2,
        [ManaAbilityBinding {
            card_definition: MANA_SOURCE,
            ability: ActivatedManaAbility {
                id: "produce-black",
                tap_cost: true,
                output: ManaAbilityOutput::Fixed(Color::Black),
                amount: 1,
                life_payment: None,
                controller_damage: None,
            },
        }],
        [],
        [],
        [],
        [TriggeredAbilityBinding {
            card_definition: CREATURE_LAND,
            ability: TriggeredAbility {
                id: "reanimated-creature-land-entry",
                condition: TriggerCondition::LandEntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::GainLifeController { amount: 1 }],
            },
        }],
    )
    .expect("fixture initializes");
    let reanimate = game
        .add_card(controller, REANIMATE, Zone::Hand)
        .expect("reanimation spell setup");
    let creature_land = game
        .add_card(controller, CREATURE_LAND, Zone::Graveyard)
        .expect("creature-land graveyard setup");
    let mana_source = game
        .put_on_battlefield(controller, MANA_SOURCE)
        .expect("mana source setup");
    game.begin_game().expect("game begins");
    game.cast_spell_with_mana_spend(
        controller,
        CastRequest {
            card: reanimate,
            targets: vec![Target::Permanent(creature_land)],
            convoke: vec![],
            payment_mana_abilities: vec![CastPaymentManaAbility::Bound(ManaAbilityActivation {
                source: mana_source,
                ability_id: "produce-black",
                chosen_color: None,
            })],
        },
        ManaPaymentSelection {
            generic: vec![Color::Black],
            hybrid: vec![],
        },
    )
    .expect("reanimation spell casts");
    pass_pair(&mut game);

    eprintln!(
        "reanimated creature-land trace={:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(creature_land), Some(Zone::Battlefield));
    assert!(
        game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::TriggeredAbilityStacked {
                source,
                ability: "reanimated-creature-land-entry",
                ..
            } if *source == creature_land
        )),
        "targeted creature-land reanimation must retain its land-entry trigger"
    );
    game.validate_invariants()
        .expect("reanimated creature-land state remains invariant-valid");
}
