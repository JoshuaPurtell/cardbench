//! Red regression: an ability controlled by a survivor remains on the stack
//! when its physical source's owner leaves the game.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType,
    CastRequest, Effect, Game, GameEvent, ManaCost, PlayerId, Target, TargetRequirement, Zone,
};

const PINGER: &str = "TST-DEPARTING-OWNER-CONTROLLED-ABILITY-PINGER";
const STEAL: &str = "TST-DEPARTING-OWNER-CONTROLLED-ABILITY-STEAL";
const LETHAL: &str = "TST-DEPARTING-OWNER-CONTROLLED-ABILITY-LETHAL";

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
        supported_rules: &["departing-owner-controlled-ability-red"],
        power: creature.then_some(1),
        toughness: creature.then_some(1),
        keywords: vec![],
        effects,
    }
}

fn pass_once(game: &mut Game) {
    let player = game.priority;
    game.pass_priority(player).expect("priority pass succeeds");
}

#[test]
#[allow(clippy::too_many_lines)] // One three-player stack/loss sequence owns the regression boundary.
fn departing_owner_does_not_remove_survivors_activated_ability() {
    let owner = PlayerId(0);
    let controller = PlayerId(1);
    let target = PlayerId(2);
    let mut game = Game::new_with_all_bindings(
        [
            definition(PINGER, CardType::Creature, vec![]),
            definition(
                STEAL,
                CardType::Instant,
                vec![Effect::GainControlTargetUntilEndOfTurn],
            ),
            definition(
                LETHAL,
                CardType::Instant,
                vec![Effect::DealDamage {
                    amount: 20,
                    target: TargetRequirement::Player,
                }],
            ),
        ],
        3,
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
        .put_on_battlefield(owner, PINGER)
        .expect("owner's pinger enters");
    let steal = game
        .add_card(controller, STEAL, Zone::Hand)
        .expect("control spell enters hand");
    let lethal = game
        .add_card(controller, LETHAL, Zone::Hand)
        .expect("lethal spell enters hand");
    game.begin_game().expect("game begins");

    pass_once(&mut game);
    game.cast_spell(
        controller,
        CastRequest {
            card: steal,
            targets: vec![Target::Permanent(pinger)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("survivor steals pinger");
    pass_once(&mut game);
    pass_once(&mut game);
    pass_once(&mut game);
    assert_eq!(game.controller_of(pinger), Ok(controller));

    pass_once(&mut game);
    game.activate_ability(
        controller,
        AbilityActivation {
            source: pinger,
            ability_id: "ping-player",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![Target::Player(target)],
        },
    )
    .expect("survivor activates the stolen pinger");
    game.cast_spell(
        controller,
        CastRequest {
            card: lethal,
            targets: vec![Target::Player(owner)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("survivor puts owner-lethal response above the ability");
    pass_once(&mut game);
    pass_once(&mut game);
    let resolver = game.priority;
    let loss_resolution = game.pass_priority(resolver);
    eprintln!(
        "departing owner controlled-ability red loss result={loss_resolution:?}; events={:?}",
        game.canonical_event_log(),
    );
    loss_resolution.expect("owner departure must leave the survivor-controlled ability on stack");

    assert!(game.player(owner).expect("owner exists").lost);
    assert_eq!(game.zone_of(pinger), None, "owned source leaves the game");
    assert_eq!(
        game.stack.len(),
        1,
        "the survivor's independent activated ability remains after the source owner leaves"
    );
    assert_eq!(game.stack[0].card, pinger);
    assert_eq!(game.stack[0].controller, controller);

    pass_once(&mut game);
    pass_once(&mut game);
    eprintln!(
        "departing owner controlled-ability red trace: target_life={}; stack={:?}; events={:?}",
        game.player(target).expect("target exists").life,
        game.stack,
        game.canonical_event_log(),
    );
    assert_eq!(
        game.player(target).expect("target exists").life,
        19,
        "the survivor's ability resolves after its departed physical source"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AbilityResolved {
            source,
            ability: "ping-player",
            ..
        } if *source == pinger
    )));
    game.validate_invariants()
        .expect("survivor-controlled ability leaves a valid terminal game state");
}
