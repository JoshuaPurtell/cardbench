//! Red regression: an activated ability keeps its activator as its stack
//! controller after a response changes control of the battlefield source.
//!
//! CR 113.7a captures control of an activated ability when it is activated.
//! A later layer-two control effect on its source must not make the historical
//! stack item invalid or retarget its controller.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, Color, Effect, Game, GameEvent, ManaCost, PlayerId, Target, TargetRequirement,
    Zone,
};

const PINGER: &str = "TST-ACTIVATED-ABILITY-CONTROL-PINGER";
const STEAL: &str = "TST-ACTIVATED-ABILITY-CONTROL-STEAL";

fn definition(
    id: &'static str,
    card_types: impl IntoIterator<Item = CardType>,
    effects: Vec<Effect>,
) -> CardDefinition {
    let card_types = card_types.into_iter().collect::<BTreeSet<_>>();
    let is_creature = card_types.contains(&CardType::Creature);
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["activated-ability-control-change-stack-provenance-red"],
        power: is_creature.then_some(1),
        toughness: is_creature.then_some(1),
        keywords: vec![],
        effects,
    }
}

fn pass_once(game: &mut Game) {
    let player = game.priority;
    game.pass_priority(player).expect("priority pass succeeds");
}

#[test]
fn activated_ability_resolves_for_original_activator_after_source_is_stolen() {
    let mut game = Game::new_with_all_bindings(
        [
            definition(PINGER, [CardType::Creature], vec![]),
            definition(
                STEAL,
                [CardType::Instant],
                vec![Effect::GainControlTargetUntilEndOfTurn],
            ),
        ],
        2,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: PINGER,
            ability: ActivatedAbility {
                id: "ping-player",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![TargetRequirement::Player],
                effects: vec![Effect::DealDamage {
                    amount: 1,
                    target: TargetRequirement::Player,
                }],
            },
        }],
    )
    .expect("fixture initializes");
    let pinger = game
        .put_on_battlefield(PlayerId(0), PINGER)
        .expect("pinger begins on player zero battlefield");
    let steal = game
        .add_card(PlayerId(1), STEAL, Zone::Hand)
        .expect("steal spell begins in player one hand");
    game.begin_game().expect("fixture begins");
    game.clear_event_log();

    game.activate_ability(
        PlayerId(0),
        AbilityActivation {
            source: pinger,
            ability_id: "ping-player",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Player(PlayerId(1))],
        },
    )
    .expect("player zero activates the ability");
    pass_once(&mut game);
    game.cast_spell(
        PlayerId(1),
        CastRequest {
            card: steal,
            targets: vec![Target::Permanent(pinger)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("player one casts the response");
    pass_once(&mut game);
    pass_once(&mut game);

    assert_eq!(
        game.controller_of(pinger).expect("pinger remains on battlefield"),
        PlayerId(1),
        "the response changes control of the pinger before its older ability resolves"
    );
    game.validate_invariants()
        .expect("a control-changed ability source remains a valid live stack state");

    pass_once(&mut game);
    pass_once(&mut game);

    eprintln!(
        "activated_ability_control_change_events={:?}",
        game.canonical_event_log()
    );
    assert_eq!(
        game.player(PlayerId(1)).expect("player one exists").life,
        19,
        "the older stack ability remains controlled by player zero and resolves normally"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityActivated {
            player: PlayerId(0),
            source,
            ability: "ping-player",
            ..
        } if *source == pinger
    )));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DamageDealtToPlayer {
            source,
            player: PlayerId(1),
            amount: 1,
        } if *source == pinger
    )));
    game.validate_invariants()
        .expect("the resolved ability retains valid historical controller provenance");
}
