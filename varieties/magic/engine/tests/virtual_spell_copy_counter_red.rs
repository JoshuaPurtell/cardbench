//! Regression for countering a virtual spell copy.
//!
//! A stack copy is still a spell and may be targeted by a counter effect, but
//! it is not a card and therefore must not be sent to a card zone.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, ManaCost, PlayerId, Target,
    TargetRequirement, Zone,
};

const PING: &str = "TST-VIRTUAL-COUNTER-PING";
const COPY: &str = "TST-VIRTUAL-COUNTER-COPY";
const COUNTER: &str = "TST-VIRTUAL-COUNTER";

fn definition(id: &'static str, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["virtual-spell-copy-counter-red"],
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
    game.pass_priority(first).expect("first pass");
    let second = game.priority;
    game.pass_priority(second).expect("second pass");
}

#[test]
fn counterspell_can_counter_a_virtual_spell_copy_without_a_zone_move() {
    let mut game = Game::new(
        vec![
            definition(
                PING,
                vec![Effect::DealDamage {
                    amount: 2,
                    target: TargetRequirement::Player,
                }],
            ),
            definition(
                COPY,
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
            definition(COUNTER, vec![Effect::CounterTargetSpell]),
        ],
        2,
    )
    .expect("fixture initializes");
    let ping = game
        .add_card(PlayerId(0), PING, Zone::Hand)
        .expect("ping enters hand");
    let copy = game
        .add_card(PlayerId(0), COPY, Zone::Hand)
        .expect("copy enters hand");
    let counter = game
        .add_card(PlayerId(0), COUNTER, Zone::Hand)
        .expect("counter enters hand");
    game.begin_game().expect("game begins");

    game.cast_spell(
        PlayerId(0),
        request(ping, vec![Target::Player(PlayerId(1))]),
    )
    .expect("ping casts");
    game.cast_spell(PlayerId(0), request(copy, vec![Target::Spell(ping)]))
        .expect("copy effect casts");
    resolve_top(&mut game);
    let virtual_copy = game
        .event_log
        .iter()
        .rev()
        .find_map(|event| match event {
            cardbench_magic_engine::GameEvent::SpellCopied { copy, .. } => Some(*copy),
            _ => None,
        })
        .expect("copy receipt identifies the virtual stack spell");
    assert_eq!(game.zone_of(virtual_copy), None, "copies have no card zone");

    game.cast_spell(
        PlayerId(0),
        request(counter, vec![Target::Spell(virtual_copy)]),
    )
    .expect("counter may target a virtual spell copy");
    resolve_top(&mut game);

    assert_eq!(game.zone_of(virtual_copy), None, "countered copy stays zoneless");
    assert_eq!(game.zone_of(counter), Some(Zone::Graveyard));
    assert_eq!(game.stack.len(), 1, "only the physical original remains");
    game.validate_invariants()
        .expect("copy countering has an auditable terminal lifecycle");
}
