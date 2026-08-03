//! Red regression: a copy of a virtual spell may choose new targets.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DecisionKind, Effect, Game, ManaCost, ObjectId,
    PlayerId, Target, TargetRequirement, Zone,
};

const PING: &str = "TST-VIRTUAL-COPY-RETARGET-PING";
const COPY_RETAIN: &str = "TST-VIRTUAL-COPY-RETARGET-RETAIN";
const COPY_RETARGET: &str = "TST-VIRTUAL-COPY-RETARGET-NEW-TARGET";

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
        supported_rules: &["virtual-spell-copy-chain-retarget-red"],
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
    game.pass_priority(first).expect("first pass succeeds");
    let second = game.priority;
    game.pass_priority(second).expect("second pass succeeds");
}

#[test]
fn virtual_spell_copy_can_open_its_own_retarget_decision() {
    let caster = PlayerId(0);
    let opponent = PlayerId(1);
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
                COPY_RETAIN,
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
            definition(
                COPY_RETARGET,
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: true,
                }],
            ),
        ],
        2,
    )
    .expect("fixture initializes");
    let ping = game.add_card(caster, PING, Zone::Hand).expect("ping");
    let first_copy_effect = game
        .add_card(caster, COPY_RETAIN, Zone::Hand)
        .expect("retained-target copy effect");
    let second_copy_effect = game
        .add_card(caster, COPY_RETARGET, Zone::Hand)
        .expect("retargeting copy effect");
    game.begin_game().expect("game begins");

    game.cast_spell(caster, request(ping, vec![Target::Player(opponent)]))
        .expect("physical spell casts");
    game.cast_spell(
        caster,
        request(first_copy_effect, vec![Target::Spell(ping)]),
    )
    .expect("first copy effect casts");
    resolve_top(&mut game);
    let first_copy = game
        .stack
        .last()
        .expect("first virtual copy remains on the stack")
        .card;

    game.cast_spell(
        caster,
        request(second_copy_effect, vec![Target::Spell(first_copy)]),
    )
    .expect("second copy effect may target the virtual spell");
    resolve_top(&mut game);
    let pending = game
        .view_for_player(caster)
        .expect("caster view")
        .pending_decision;
    eprintln!(
        "virtual-copy retarget red trace: pending={:?}; events={:?}",
        pending,
        game.canonical_event_log()
    );
    assert_eq!(
        pending
            .as_ref()
            .expect("copying a target-bearing virtual spell opens a decision")
            .kind,
        DecisionKind::SpellCopyTargets
    );
}
