//! Red regression: a spell copy is itself a spell that may be copied.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, ManaCost, PlayerId, Target,
    TargetRequirement, Zone,
};

const PING: &str = "TST-VIRTUAL-COPY-CHAIN-PING";
const COPY: &str = "TST-VIRTUAL-COPY-CHAIN-COPY";

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
        supported_rules: &["virtual-spell-copy-chain-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

fn request(card: cardbench_magic_engine::ObjectId, targets: Vec<Target>) -> CastRequest {
    CastRequest {
        card,
        targets,
        convoke: vec![],
        payment_mana_abilities: vec![],
    }
}

fn resolve_top(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first pass succeeds");
    let second = game.priority;
    game.pass_priority(second).expect("second pass succeeds");
}

#[test]
fn a_virtual_spell_copy_can_be_copied_again() {
    let caster = PlayerId(0);
    let second_copy_controller = PlayerId(1);
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
        ],
        2,
    )
    .expect("fixture initializes");
    let ping = game
        .add_card(caster, PING, Zone::Hand)
        .expect("ping enters caster hand");
    let first_copy_effect = game
        .add_card(caster, COPY, Zone::Hand)
        .expect("first copy effect enters caster hand");
    let second_copy_effect = game
        .add_card(second_copy_controller, COPY, Zone::Hand)
        .expect("second copy effect enters opponent hand");
    game.begin_game().expect("game begins");

    game.cast_spell(
        caster,
        request(ping, vec![Target::Player(second_copy_controller)]),
    )
    .expect("physical ping casts");
    game.cast_spell(
        caster,
        request(first_copy_effect, vec![Target::Spell(ping)]),
    )
    .expect("first copy effect casts");
    resolve_top(&mut game);
    let first_copy = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCopied { copy, original, .. } if *original == ping => Some(*copy),
            _ => None,
        })
        .expect("first virtual copy exists");

    game.pass_priority(caster)
        .expect("caster passes to the second copy controller");
    game.cast_spell(
        second_copy_controller,
        request(second_copy_effect, vec![Target::Spell(first_copy)]),
    )
    .expect("a virtual copy is a legal copy target");
    resolve_top(&mut game);

    let second_copy = game
        .event_log
        .iter()
        .rev()
        .find_map(|event| match event {
            GameEvent::SpellCopied {
                copy,
                original,
                controller,
                ..
            } if *original == first_copy && *controller == second_copy_controller => Some(*copy),
            _ => None,
        })
        .expect("second virtual copy is created from the first");
    assert!(game.stack.iter().any(|item| item.card == second_copy));

    resolve_top(&mut game);
    resolve_top(&mut game);
    resolve_top(&mut game);
    eprintln!(
        "virtual-copy chain trace: life={}; events={:?}",
        game.player(second_copy_controller)
            .expect("player exists")
            .life,
        game.canonical_event_log(),
    );
    assert_eq!(
        game.player(second_copy_controller)
            .expect("player exists")
            .life,
        17
    );
    game.validate_invariants()
        .expect("three spell objects have independently auditable lifecycles");
}
