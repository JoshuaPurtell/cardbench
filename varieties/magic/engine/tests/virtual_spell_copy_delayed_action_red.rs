//! Red regression: a delayed action created by a virtual spell copy retains
//! immutable source provenance after the copy has left the stack.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, GameEvent, ManaCost, ObjectId,
    PlayerId, Target, Zone,
};

const CREATURE: &str = "TST-VIRTUAL-DELAYED-CREATURE";
const GAZE: &str = "TST-VIRTUAL-DELAYED-GAZE";
const COPY: &str = "TST-VIRTUAL-DELAYED-COPY";
const COUNTER: &str = "TST-VIRTUAL-DELAYED-COUNTER";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    let creature = card_type == CardType::Creature;
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Black, Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["virtual-spell-copy-delayed-action-red"],
        power: creature.then_some(3),
        toughness: creature.then_some(3),
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

fn advance_empty_stack(game: &mut Game) -> Result<(), cardbench_magic_engine::RulesError> {
    let first = game.priority;
    game.pass_priority(first)?;
    let second = game.priority;
    game.pass_priority(second)
}

#[test]
#[allow(clippy::too_many_lines)] // The copy, physical-original counter, and real turn path are one provenance contract.
fn virtual_copy_delayed_action_reaches_end_of_combat_without_source_zone_lookup() {
    let caster = PlayerId(0);
    let copy_controller = PlayerId(1);
    let mut game = Game::new(
        [
            definition(CREATURE, CardType::Creature, vec![]),
            definition(
                GAZE,
                CardType::Instant,
                vec![Effect::RegenerateTargetCreatureAndScheduleCombatHistoryDestruction],
            ),
            definition(
                COPY,
                CardType::Instant,
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
            definition(COUNTER, CardType::Instant, vec![Effect::CounterTargetSpell]),
        ],
        2,
    )
    .expect("fixture initializes");
    let creature = game
        .put_on_battlefield(caster, CREATURE)
        .expect("regeneration target begins on battlefield");
    let gaze = game
        .add_card(caster, GAZE, Zone::Hand)
        .expect("gaze enters hand");
    let copy = game
        .add_card(copy_controller, COPY, Zone::Hand)
        .expect("copy enters hand");
    let counter = game
        .add_card(caster, COUNTER, Zone::Hand)
        .expect("counter enters hand");
    game.begin_game().expect("game begins");

    game.cast_spell(caster, request(gaze, vec![Target::Permanent(creature)]))
        .expect("gaze casts");
    game.pass_priority(caster)
        .expect("caster passes to copy controller");
    game.cast_spell(copy_controller, request(copy, vec![Target::Spell(gaze)]))
        .expect("copy casts");
    resolve_top(&mut game).expect("copy instruction creates virtual gaze");
    resolve_top(&mut game).expect("virtual gaze schedules delayed action");
    let virtual_copy = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCopied { copy, original, .. } if *original == gaze => Some(*copy),
            _ => None,
        })
        .expect("copy receipt identifies virtual gaze");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DelayedCombatDestructionScheduled { source, .. } if *source == virtual_copy
    )));
    game.cast_spell(caster, request(counter, vec![Target::Spell(gaze)]))
        .expect("counter removes physical original so only virtual delayed provenance remains");
    resolve_top(&mut game).expect("physical original is countered");

    advance_empty_stack(&mut game).expect("upkeep advances to draw");
    advance_empty_stack(&mut game).expect("draw advances to precombat main");
    advance_empty_stack(&mut game).expect("precombat main advances to beginning of combat");
    advance_empty_stack(&mut game).expect("beginning of combat advances to declare attackers");
    game.declare_attackers(caster, &[])
        .expect("active player declares no attackers");
    let result = advance_empty_stack(&mut game);
    eprintln!(
        "virtual delayed-action red trace: result={result:?}; step={:?}; events={:?}",
        game.step,
        game.canonical_event_log()
    );
    assert!(
        result.is_ok(),
        "end-of-combat delayed action must use retained virtual-copy provenance rather than UnknownCard"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DelayedCombatDestructionStacked { source, .. } if *source == virtual_copy
    )));
    game.validate_invariants()
        .expect("virtual delayed-action provenance remains auditable");
}
