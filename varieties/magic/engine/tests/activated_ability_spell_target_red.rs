//! Red regression: an activated ability from an instant card is not an
//! instant-or-sorcery spell on the stack.
//!
//! The policy boundary must validate the stack object's kind, rather than
//! accepting `Target::Spell(card)` merely because the physical source card is
//! an instant. Transmute provides a realistic activated ability whose source
//! is an instant card in a graveyard while the ability remains on the stack.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, Keyword, ManaCost, PlayerId, PolicyAction,
    RulesError, Target, Zone,
};

const TRANSMUTER: &str = "TST-TRANSMUTE-INSTANT";
const COPY: &str = "TST-COPY-INSTANT";
const COUNTER: &str = "TST-NONCREATURE-COUNTER";
const LOWER: &str = "TST-LOWER-INSTANT";
const POLICY: &str = "adversarial.transmute-target.v1";

fn definition(
    id: &'static str,
    mana_cost: ManaCost,
    keywords: Vec<Keyword>,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost,
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["activated-ability-spell-target-probe"],
        power: None,
        toughness: None,
        keywords,
        effects,
    }
}

fn game_with_stack_target_cards() -> (Game, PlayerId) {
    let player = PlayerId(0);
    let game = Game::new(
        [
            definition(
                TRANSMUTER,
                ManaCost::new(1),
                vec![Keyword::Transmute(ManaCost::new(0))],
                vec![],
            ),
            definition(
                COPY,
                ManaCost::new(0),
                vec![],
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
            definition(
                COUNTER,
                ManaCost::new(0),
                vec![],
                vec![Effect::CounterTargetNoncreatureSpell],
            ),
            definition(
                LOWER,
                ManaCost::new(0),
                vec![],
                vec![Effect::GainLifeController { amount: 1 }],
            ),
        ],
        2,
    )
    .expect("fixture initializes");
    (game, player)
}

fn activate_transmute(game: &mut Game, player: PlayerId) -> cardbench_magic_engine::ObjectId {
    let transmuter = game
        .add_card(player, TRANSMUTER, Zone::Hand)
        .expect("Transmute source enters hand");
    game.submit_policy_move(player, POLICY, PolicyAction::Transmute { card: transmuter })
        .expect("Transmute ability is legally activated");
    assert_eq!(game.stack.len(), 1, "only Transmute is on the stack");
    assert_eq!(game.stack[0].card, transmuter);
    assert_eq!(game.stack[0].ability_id, Some("transmute"));
    assert!(game.stack[0].targets.is_empty());
    assert_eq!(game.zone_of(transmuter), Some(Zone::Graveyard));
    assert_eq!(game.priority, player, "the activator retains priority");
    game.validate_invariants()
        .expect("the Transmute stack state is valid");
    transmuter
}

fn assert_rejected_cast_is_atomic(
    game: &mut Game,
    player: PlayerId,
    card: cardbench_magic_engine::ObjectId,
    target: Target,
) -> RulesError {
    let players_before = game.players.clone();
    let stack_before = game.stack.clone();
    let effects_before = game.continuous_effects.clone();
    let events_before = game.event_log.clone();
    let canonical_before = game.canonical_event_log();
    let card_before = game.object(card).expect("candidate card exists").clone();
    let priority_before = game.priority;
    let active_before = game.active_player;
    let step_before = game.step;
    let turn_before = game.turn;

    let error = game
        .submit_policy_move(
            player,
            POLICY,
            PolicyAction::Cast(CastRequest {
                card,
                targets: vec![target],
                convoke: vec![],
                payment_mana_abilities: vec![],
            }),
        )
        .expect_err("the stack-kind mismatch must be rejected");

    assert_eq!(game.players, players_before, "player state rolled back");
    assert_eq!(game.stack, stack_before, "stack state rolled back");
    assert_eq!(
        game.continuous_effects, effects_before,
        "continuous effects rolled back"
    );
    assert_eq!(game.event_log, events_before, "event receipts rolled back");
    assert_eq!(
        game.canonical_event_log(),
        canonical_before,
        "canonical trace is unchanged"
    );
    assert_eq!(
        game.object(card).expect("candidate remains addressable"),
        &card_before,
        "the rejected card keeps its incarnation and characteristics"
    );
    assert_eq!(game.zone_of(card), Some(Zone::Hand));
    assert_eq!(game.priority, priority_before);
    assert_eq!(game.active_player, active_before);
    assert_eq!(game.step, step_before);
    assert_eq!(game.turn, turn_before);
    assert!(
        !game.event_log.iter().any(
            |event| matches!(event, cardbench_magic_engine::GameEvent::SpellCast { card: cast, .. } if *cast == card)
        ),
        "a rejected cast has no SpellCast receipt"
    );
    game.validate_invariants()
        .expect("rejected cast leaves an invariant-valid state");
    error
}

#[test]
fn policy_cannot_target_transmute_ability_as_an_instant_spell() {
    let (mut game, player) = game_with_stack_target_cards();
    let transmuter = activate_transmute(&mut game, player);
    let copy = game
        .add_card(player, COPY, Zone::Hand)
        .expect("copy spell enters hand");
    let result = assert_rejected_cast_is_atomic(&mut game, player, copy, Target::Spell(transmuter));
    eprintln!(
        "activated-ability spell-target result: {result:?}; stack: {:?}; events: {:?}",
        game.stack,
        game.canonical_event_log(),
    );

    assert!(
        matches!(result, RulesError::IllegalTarget(Target::Spell(card)) if card == transmuter),
        "an activated ability cannot satisfy an instant-or-sorcery spell target"
    );
}

#[test]
fn policy_cannot_target_transmute_ability_as_a_noncreature_spell() {
    let (mut game, player) = game_with_stack_target_cards();
    let transmuter = activate_transmute(&mut game, player);
    let counter = game
        .add_card(player, COUNTER, Zone::Hand)
        .expect("counter spell enters hand");

    for attempt in 0..2 {
        let error =
            assert_rejected_cast_is_atomic(&mut game, player, counter, Target::Spell(transmuter));
        assert!(
            matches!(error, RulesError::IllegalTarget(Target::Spell(card)) if card == transmuter),
            "attempt {attempt} must reject the ability as a noncreature spell: {error:?}"
        );
    }
}

#[test]
fn spell_requirement_rejects_the_activated_ability_target_variant_atomically() {
    let (mut game, player) = game_with_stack_target_cards();
    let _transmuter = activate_transmute(&mut game, player);
    let ability = game.stack[0].id;
    let copy = game
        .add_card(player, COPY, Zone::Hand)
        .expect("copy spell enters hand");

    let error =
        assert_rejected_cast_is_atomic(&mut game, player, copy, Target::ActivatedAbility(ability));
    assert!(matches!(
        error,
        RulesError::IllegalTarget(Target::ActivatedAbility(rejected)) if rejected == ability
    ));
}

fn legal_spell_copy_trace() -> Vec<String> {
    let (mut game, player) = game_with_stack_target_cards();
    let opponent = PlayerId(1);
    let lower = game
        .add_card(player, LOWER, Zone::Hand)
        .expect("lower instant enters hand");
    let copy = game
        .add_card(player, COPY, Zone::Hand)
        .expect("copy instant enters hand");

    game.submit_policy_move(
        player,
        POLICY,
        PolicyAction::Cast(CastRequest {
            card: lower,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        }),
    )
    .expect("the real instant is cast");
    game.submit_policy_move(
        player,
        POLICY,
        PolicyAction::Cast(CastRequest {
            card: copy,
            targets: vec![Target::Spell(lower)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        }),
    )
    .expect("a real instant spell is a legal copy target");
    assert_eq!(game.stack.len(), 2);
    assert!(game.stack.iter().all(|object| object.ability_id.is_none()));
    assert_eq!(game.stack[1].targets, [Target::Spell(lower)]);
    assert_eq!(game.priority, player);

    for expected_depth in [2, 1, 0] {
        game.submit_policy_move(player, POLICY, PolicyAction::PassPriority)
            .expect("active player passes");
        game.submit_policy_move(opponent, POLICY, PolicyAction::PassPriority)
            .expect("opponent passes and resolves the LIFO top");
        assert_eq!(game.stack.len(), expected_depth);
        game.validate_invariants()
            .expect("each resolution boundary remains valid");
    }
    assert_eq!(game.player(player).expect("player exists").life, 22);
    assert_eq!(game.zone_of(lower), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(copy), Some(Zone::Graveyard));
    assert_eq!(game.priority, player);
    game.canonical_event_log()
}

#[test]
fn real_instant_spell_target_remains_legal_lifo_and_deterministic() {
    let first = legal_spell_copy_trace();
    let second = legal_spell_copy_trace();
    assert_eq!(
        first, second,
        "identical policy sequences have no digest drift"
    );
    assert_eq!(
        first
            .iter()
            .filter(|event| event.starts_with("SpellCast"))
            .count(),
        2
    );
    assert_eq!(
        first
            .iter()
            .filter(|event| event.starts_with("SpellCopyResolved"))
            .count(),
        1
    );
    assert_eq!(
        first
            .iter()
            .filter(|event| event.starts_with("SpellResolved"))
            .count(),
        2
    );
}
