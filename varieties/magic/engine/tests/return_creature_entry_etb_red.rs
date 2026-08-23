//! Red regression: a creature returned during spell resolution must retain
//! its entry trigger through the ensuing SBA checkpoint.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    ActivatedManaAbility, CardDefinition, CardType, CastPaymentManaAbility, CastRequest, Color,
    Effect, Game, GameEvent, ManaAbilityActivation, ManaAbilityBinding, ManaAbilityOutput,
    ManaCost, ManaPaymentSelection, PlayerId, Target, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding, Zone,
};

const REANIMATE: &str = "TST-RETURN-CREATURE-ENTRY";
const EPHEMERAL: &str = "TST-RETURN-CREATURE-EPHEMERAL";
const MANA_SOURCE: &str = "TST-RETURN-CREATURE-MANA";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    let ephemeral = id == EPHEMERAL;
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(u8::from(id == REANIMATE)),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["return-creature-entry-etb-red"],
        power: ephemeral.then_some(0),
        toughness: ephemeral.then_some(0),
        keywords: vec![],
        effects,
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first pass succeeds");
    let second = game.priority;
    game.pass_priority(second)
        .expect("second pass resolves the return spell");
}

#[test]
#[allow(clippy::too_many_lines)] // The complete legal payment and entry-provenance transcript stays together.
fn returned_short_lived_creature_keeps_its_etb_incarnation() {
    let controller = PlayerId(0);
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(
                REANIMATE,
                CardType::Instant,
                vec![
                    Effect::ReturnTargetCreatureCardToBattlefieldWithCounterIfManaColorSpent {
                        color: Color::Green,
                    },
                ],
            ),
            definition(EPHEMERAL, CardType::Creature, vec![]),
            definition(MANA_SOURCE, CardType::Artifact, vec![]),
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
            card_definition: EPHEMERAL,
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
    let spell = game
        .add_card(controller, REANIMATE, Zone::Hand)
        .expect("return spell begins in hand");
    let creature = game
        .add_card(controller, EPHEMERAL, Zone::Graveyard)
        .expect("short-lived ETB creature begins in graveyard");
    let mana_source = game
        .put_on_battlefield(controller, MANA_SOURCE)
        .expect("mana artifact begins on the battlefield before the game");
    game.begin_game().expect("game begins");
    game.cast_spell_with_mana_spend(
        controller,
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(creature)],
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
    .expect("return spell casts with its creature-card target");
    pass_pair(&mut game);

    eprintln!(
        "return-creature entry trace: {:?}",
        game.canonical_event_log()
    );
    assert_eq!(game.zone_of(creature), Some(Zone::Graveyard));
    let entry_move = game
        .event_log
        .iter()
        .position(|event| matches!(event, GameEvent::CardMoved { card, to: Zone::Battlefield } if *card == creature))
        .expect("return effect moved the creature to the battlefield");
    let entry_incarnation = game.event_log[entry_move + 1..]
        .iter()
        .find_map(|event| match event {
            GameEvent::ObjectIncarnationAdvanced {
                object,
                incarnation,
            } if *object == creature => Some(*incarnation),
            _ => None,
        })
        .expect("entry has an incarnation receipt");
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
        .expect("returned creature must retain its historical ETB trigger");
    assert_eq!(trigger_incarnation, entry_incarnation);
    game.validate_invariants()
        .expect("returned creature entry remains auditable");
}
