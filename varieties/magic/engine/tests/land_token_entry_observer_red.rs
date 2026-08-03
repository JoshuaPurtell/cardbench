//! Red regression: an ordinary land token must produce a land-entry event.
//!
//! A generic `CreateToken` instruction may create a token with the Land type.
//! It has no printed definition of its own, but existing represented
//! permanents still observe that land entering the battlefield.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, ManaCost, PlayerId,
    TokenSpec, TriggerCondition, TriggeredAbility, TriggeredAbilityBinding, Zone,
};

const SPELL: &str = "TST-LAND-TOKEN-CREATOR";
const WATCHER: &str = "TST-LAND-TOKEN-WATCHER";

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
        supported_rules: &["land-token-entry-observer-red"],
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
fn generic_land_token_entry_stacks_existing_land_observers() {
    let controller = PlayerId(0);
    let mut land_token = TokenSpec::saproling();
    land_token.name = "Land Saproling";
    land_token.card_types.insert(CardType::Land);
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(
                SPELL,
                CardType::Instant,
                vec![Effect::CreateToken {
                    token: land_token,
                    count: 1,
                }],
            ),
            definition(WATCHER, CardType::Creature, vec![]),
        ],
        2,
        [],
        [],
        [],
        [],
        [TriggeredAbilityBinding {
            card_definition: WATCHER,
            ability: TriggeredAbility {
                id: "controlled-land-token-entry",
                condition: TriggerCondition::ControlledLandEntersBattlefield,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::GainLifeController { amount: 1 }],
            },
        }],
    )
    .expect("fixture initializes");
    let watcher = game
        .put_on_battlefield(controller, WATCHER)
        .expect("watcher setup");
    let spell = game
        .add_card(controller, SPELL, Zone::Hand)
        .expect("token spell setup");
    game.begin_game().expect("game begins");

    game.cast_spell(
        controller,
        CastRequest {
            card: spell,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("land-token spell casts");
    pass_pair(&mut game);

    let token = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::TokenCreated { player, token } if *player == controller => Some(*token),
            _ => None,
        })
        .expect("spell creates one land token");
    eprintln!("land token entry trace={:?}", game.canonical_event_log());

    assert_eq!(game.zone_of(token), Some(Zone::Battlefield));
    assert!(game.event_log.iter().any(|event| {
        matches!(
            event,
            GameEvent::TriggeredAbilityStacked { source, ability, .. }
                if *source == watcher && *ability == "controlled-land-token-entry"
        )
    }), "an existing controlled-land observer must stack for a land token entry");
    game.validate_invariants()
        .expect("land-token entry keeps the game state valid");
}
