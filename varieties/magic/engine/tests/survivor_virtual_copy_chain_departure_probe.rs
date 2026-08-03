//! Scratch adversarial probe: a child virtual copy survives its immediate
//! virtual predecessor's controller leaving a multiplayer game.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, ManaCost, ObjectId, PlayerId,
    Target, TargetRequirement, Zone,
};

const PING: &str = "TST-SURVIVOR-CHAIN-PING";
const COPY: &str = "TST-SURVIVOR-CHAIN-COPY";
const LETHAL: &str = "TST-SURVIVOR-CHAIN-LETHAL";

fn definition(id: &'static str, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["survivor-virtual-copy-chain-departure-probe"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

fn request(card: ObjectId, targets: Vec<Target>) -> CastRequest {
    CastRequest {
        card,
        targets,
        convoke: vec![],
        payment_mana_abilities: vec![],
    }
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first pass");
    let second = game.priority;
    game.pass_priority(second).expect("second pass");
    let third = game.priority;
    game.pass_priority(third).expect("third pass");
}

#[test]
fn child_copy_survives_immediate_virtual_predecessor_controller_departure() {
    let first_controller = PlayerId(0);
    let surviving_controller = PlayerId(1);
    let target = PlayerId(2);
    let mut game = Game::new(
        [
            definition(
                PING,
                vec![Effect::DealDamage {
                    amount: 1,
                    target: TargetRequirement::Player,
                }],
            ),
            definition(
                COPY,
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
            definition(
                LETHAL,
                vec![Effect::DealDamage {
                    amount: 20,
                    target: TargetRequirement::Player,
                }],
            ),
        ],
        3,
    )
    .expect("fixture initializes");
    let ping = game
        .add_card(first_controller, PING, Zone::Hand)
        .expect("ping");
    let copy_one = game
        .add_card(first_controller, COPY, Zone::Hand)
        .expect("copy one");
    let copy_two = game
        .add_card(surviving_controller, COPY, Zone::Hand)
        .expect("copy two");
    let lethal = game.add_card(target, LETHAL, Zone::Hand).expect("lethal");
    game.begin_game().expect("game begins");

    game.cast_spell(
        first_controller,
        request(ping, vec![Target::Player(target)]),
    )
    .expect("physical spell");
    game.cast_spell(
        first_controller,
        request(copy_one, vec![Target::Spell(ping)]),
    )
    .expect("first copy spell");
    resolve_top(&mut game);
    let first_copy = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCopied { copy, original, .. } if *original == ping => Some(*copy),
            _ => None,
        })
        .expect("first virtual copy");

    game.pass_priority(first_controller)
        .expect("pass to player one");
    game.cast_spell(
        surviving_controller,
        request(copy_two, vec![Target::Spell(first_copy)]),
    )
    .expect("second copy spell");
    resolve_top(&mut game);
    let child_copy = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCopied { copy, original, .. } if *original == first_copy => Some(*copy),
            _ => None,
        })
        .expect("child copy");

    game.pass_priority(first_controller)
        .expect("pass to player one");
    game.pass_priority(surviving_controller)
        .expect("pass to player two");
    game.cast_spell(
        target,
        request(lethal, vec![Target::Player(first_controller)]),
    )
    .expect("lethal spell");
    resolve_top(&mut game);
    assert!(game.player(first_controller).expect("player exists").lost);
    assert!(game.stack.iter().all(|item| item.card != first_copy));
    assert!(game.stack.iter().any(|item| item.card == child_copy));

    let first = game.priority;
    game.pass_priority(first)
        .expect("survivor passes for child-copy resolution");
    let second = game.priority;
    game.pass_priority(second)
        .expect("remaining opponent passes for child-copy resolution");
    assert_eq!(game.player(target).expect("target exists").life, 19);
    eprintln!(
        "survivor virtual-copy chain departure trace: {:?}",
        game.canonical_event_log()
    );
    game.validate_invariants()
        .expect("survivor child is auditable");
}
