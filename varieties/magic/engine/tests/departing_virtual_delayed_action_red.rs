//! Red regression: delayed virtual-source actions controlled by a departing
//! player must leave with that player rather than stack after they are gone.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, GameEvent, ManaCost, ObjectId,
    PlayerId, RulesError, Target, Zone,
};

const CREATURE: &str = "TST-DEPARTING-VIRTUAL-DELAYED-CREATURE";
const GAZE: &str = "TST-DEPARTING-VIRTUAL-DELAYED-GAZE";
const COPY: &str = "TST-DEPARTING-VIRTUAL-DELAYED-COPY";
const COUNTER: &str = "TST-DEPARTING-VIRTUAL-DELAYED-COUNTER";

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
        supported_rules: &["departing-virtual-delayed-action-red"],
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

fn resolve_top(game: &mut Game) -> Result<(), RulesError> {
    for _ in 0..3 {
        let player = game.priority;
        game.pass_priority(player)?;
    }
    Ok(())
}

fn advance_empty_stack(game: &mut Game) -> Result<(), RulesError> {
    let step = game.step;
    for _ in 0..3 {
        let player = game.priority;
        game.pass_priority(player)?;
        if game.step != step {
            return Ok(());
        }
    }
    Err(RulesError::IllegalAction(
        "surviving seats did not advance an empty stack",
    ))
}

#[test]
#[allow(clippy::too_many_lines)] // The player-loss boundary must be exercised after the real copied-action lifecycle.
#[allow(clippy::similar_names)] // Fixture terms deliberately match the rules concepts under test.
fn departing_virtual_delayed_action_cannot_stack_after_player_loss() {
    let caster = PlayerId(0);
    let departing_copy_controller = PlayerId(1);
    let survivor = PlayerId(2);
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
        3,
    )
    .expect("fixture initializes");
    let creature = game
        .put_on_battlefield(caster, CREATURE)
        .expect("regeneration target begins on battlefield");
    for _ in 0..8 {
        game.add_card(caster, CREATURE, Zone::Library)
            .expect("active player has an ordinary opening/draw-step card");
    }
    let gaze = game
        .add_card(caster, GAZE, Zone::Hand)
        .expect("gaze enters hand");
    let copy = game
        .add_card(departing_copy_controller, COPY, Zone::Hand)
        .expect("copy enters hand");
    let counter = game
        .add_card(caster, COUNTER, Zone::Hand)
        .expect("counter enters hand");
    game.begin_game().expect("game begins");

    game.cast_spell(caster, request(gaze, vec![Target::Permanent(creature)]))
        .expect("gaze casts");
    game.pass_priority(caster)
        .expect("caster passes to copy controller");
    game.cast_spell(
        departing_copy_controller,
        request(copy, vec![Target::Spell(gaze)]),
    )
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
    game.cast_spell(caster, request(counter, vec![Target::Spell(gaze)]))
        .expect("counter removes physical original");
    resolve_top(&mut game).expect("physical original is countered");

    game.players[departing_copy_controller.0].life = 0;
    game.check_state_based_actions()
        .expect("one player leaves while two survivors remain");
    assert!(
        game.player(departing_copy_controller)
            .expect("departing seat is retained for history")
            .lost
    );
    assert!(
        !game
            .player(survivor)
            .expect("third seat remains in the game")
            .lost
    );

    advance_empty_stack(&mut game).expect("upkeep advances to draw");
    game.resolve_pending_draw(caster, None)
        .expect("multiplayer draw step takes the ordinary draw");
    advance_empty_stack(&mut game).expect("draw advances to precombat main");
    advance_empty_stack(&mut game).expect("precombat main advances to beginning of combat");
    advance_empty_stack(&mut game).expect("beginning of combat advances to declare attackers");
    game.declare_attackers(caster, &[])
        .expect("active player declares no attackers");
    let result = advance_empty_stack(&mut game);
    eprintln!(
        "departing virtual delayed-action red trace: result={result:?}; step={:?}; events={:?}",
        game.step,
        game.canonical_event_log()
    );
    assert!(
        result.is_ok(),
        "a delayed action controlled by a departed virtual-copy controller must leave rather than stack"
    );
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::DelayedCombatDestructionStacked { source, .. } if *source == virtual_copy
        )),
        "the departed controller's delayed action must not stack"
    );
    game.validate_invariants()
        .expect("departed delayed-action provenance is absent from continuing game state");
}
