//! Red regression: a virtual copy of a copying spell may complete its own
//! retarget decision without attempting a physical terminal-zone move.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, DecisionKind, DecisionSelection, Effect, Game,
    GameEvent, ManaCost, ObjectId, PlayerId, Target, TargetRequirement, Zone,
};

const PING: &str = "TST-VIRTUAL-COPY-SPELL-RETARGET-PING";
const COPY_RETAIN: &str = "TST-VIRTUAL-COPY-SPELL-RETARGET-RETAIN";
const COPY_RETARGET: &str = "TST-VIRTUAL-COPY-SPELL-RETARGET-NEW-TARGET";

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
        supported_rules: &["virtual-copy-spell-retarget-terminal-red"],
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

fn resolve_top(game: &mut Game) -> Result<(), cardbench_magic_engine::RulesError> {
    let first = game.priority;
    game.pass_priority(first)?;
    let second = game.priority;
    game.pass_priority(second)
}

#[test]
fn virtual_copy_of_copy_spell_can_complete_its_retarget_decision() {
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
    let retarget_copy = game
        .add_card(opponent, COPY_RETARGET, Zone::Hand)
        .expect("retargeting copy effect");
    let retain_copy = game
        .add_card(caster, COPY_RETAIN, Zone::Hand)
        .expect("copying copy effect");
    game.begin_game().expect("game begins");

    game.cast_spell(caster, request(ping, vec![Target::Player(opponent)]))
        .expect("physical ping casts");
    game.pass_priority(caster)
        .expect("caster passes to opponent");
    game.cast_spell(opponent, request(retarget_copy, vec![Target::Spell(ping)]))
        .expect("opponent casts retargeting copy effect");
    game.pass_priority(opponent)
        .expect("opponent passes to caster");
    game.cast_spell(
        caster,
        request(retain_copy, vec![Target::Spell(retarget_copy)]),
    )
    .expect("caster copies the copying spell");
    resolve_top(&mut game).expect("physical retain-copy makes virtual retarget copy");

    let virtual_retarget_copy = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCopied { copy, original, .. } if *original == retarget_copy => {
                Some(*copy)
            }
            _ => None,
        })
        .expect("virtual retargeting-copy receipt exists");
    resolve_top(&mut game).expect("virtual copy opens its own retarget decision");
    let decision = game
        .view_for_player(caster)
        .expect("caster view")
        .pending_decision
        .expect("virtual copy opens a retarget decision");
    assert_eq!(decision.kind, DecisionKind::SpellCopyTargets);
    let result = game.submit_decision(
        caster,
        decision.id,
        DecisionSelection::Targets(vec![Target::Player(caster)]),
    );
    eprintln!(
        "virtual-copying-spell retarget red trace: result={result:?}; events={:?}",
        game.canonical_event_log()
    );
    assert!(
        result.is_ok(),
        "a virtual copying spell must not attempt a physical terminal-zone move after retargeting"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCopyResolved { copy, original }
            if *copy == virtual_retarget_copy && *original == retarget_copy
    )));
    game.validate_invariants()
        .expect("virtual copying-spell decision lifecycle remains auditable");
}
