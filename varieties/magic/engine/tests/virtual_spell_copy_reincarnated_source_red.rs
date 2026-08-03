//! Red regression: copying a spell must select the exact spell stack object,
//! not an older activated ability that happens to share its physical card id.
//!
//! The source card is discarded to activate Transmute, returned to hand while
//! that ability remains pending, then cast as a new spell incarnation. This is
//! a legal way for one physical card id to identify both an old ability and a
//! new spell on the stack at the same time.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, Keyword, ManaCost, ObjectId,
    PlayerId, PolicyAction, Target, Zone,
};

const TRANSMUTER: &str = "TST-REINCARNATED-TRANSMUTER";
const RETURN: &str = "TST-RETURN-FROM-GRAVEYARD";
const COPY: &str = "TST-COPY-REINCARNATED-SPELL";
const POLICY: &str = "adversarial.reincarnated-spell-copy.v1";

fn definition(id: &'static str, keywords: Vec<Keyword>, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["reincarnated-spell-copy-stack-kind-probe"],
        power: None,
        toughness: None,
        keywords,
        effects,
    }
}

fn cast(game: &mut Game, player: PlayerId, card: ObjectId, targets: Vec<Target>) {
    game.submit_policy_move(
        player,
        POLICY,
        PolicyAction::Cast(CastRequest {
            card,
            targets,
            convoke: vec![],
            payment_mana_abilities: vec![],
        }),
    )
    .expect("the adversarial instant cast is legal");
}

fn pass_pair(game: &mut Game, first: PlayerId, second: PlayerId) {
    game.submit_policy_move(first, POLICY, PolicyAction::PassPriority)
        .expect("first player passes");
    game.submit_policy_move(second, POLICY, PolicyAction::PassPriority)
        .expect("second player passes and resolves the top stack object");
}

#[test]
#[allow(clippy::too_many_lines)] // The full zone/stack/receipt transcript is the regression contract.
fn copy_resolver_uses_the_new_spell_not_the_old_transmute_ability() {
    let player = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                TRANSMUTER,
                vec![Keyword::Transmute(ManaCost::new(0))],
                vec![Effect::GainLifeController { amount: 2 }],
            ),
            definition(RETURN, vec![], vec![Effect::ReturnTargetCardToHand]),
            definition(
                COPY,
                vec![],
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
        ],
        2,
    )
    .expect("fixture initializes");
    let transmuter = game
        .add_card(player, TRANSMUTER, Zone::Hand)
        .expect("Transmute source enters hand");
    let return_spell = game
        .add_card(player, RETURN, Zone::Hand)
        .expect("graveyard recursion enters hand");
    let copy_spell = game
        .add_card(player, COPY, Zone::Hand)
        .expect("copy spell enters hand");
    let first_incarnation = game.object(transmuter).expect("source exists").incarnation;

    game.submit_policy_move(player, POLICY, PolicyAction::Transmute { card: transmuter })
        .expect("Transmute is legally activated");
    assert_eq!(game.zone_of(transmuter), Some(Zone::Graveyard));
    assert_eq!(game.stack.len(), 1);
    assert_eq!(game.stack[0].card, transmuter);
    assert_eq!(game.stack[0].source_incarnation, first_incarnation);
    assert_eq!(game.stack[0].ability_id, Some("transmute"));

    cast(
        &mut game,
        player,
        return_spell,
        vec![Target::Permanent(transmuter)],
    );
    pass_pair(&mut game, player, opponent);
    assert_eq!(game.zone_of(return_spell), Some(Zone::Graveyard));
    assert_eq!(game.zone_of(transmuter), Some(Zone::Hand));
    assert_eq!(
        game.stack.len(),
        1,
        "the old Transmute ability remains pending"
    );
    let returned_incarnation = game
        .object(transmuter)
        .expect("source returned")
        .incarnation;
    assert!(returned_incarnation > first_incarnation);

    cast(&mut game, player, transmuter, vec![]);
    let spell_incarnation = game
        .object(transmuter)
        .expect("spell object exists")
        .incarnation;
    assert!(
        spell_incarnation > returned_incarnation,
        "casting creates a fresh stack incarnation"
    );
    assert_eq!(game.stack.len(), 2);
    assert_eq!(game.stack[0].ability_id, Some("transmute"));
    assert_eq!(game.stack[1].ability_id, None);
    assert_eq!(game.stack[1].source_incarnation, spell_incarnation);

    cast(
        &mut game,
        player,
        copy_spell,
        vec![Target::Spell(transmuter)],
    );
    assert_eq!(game.stack.len(), 3);
    let events_before_resolution = game.event_log.clone();
    game.submit_policy_move(player, POLICY, PolicyAction::PassPriority)
        .expect("copy caster passes");
    let result = game.submit_policy_move(opponent, POLICY, PolicyAction::PassPriority);

    eprintln!(
        "reincarnated spell-copy red trace: first_incarnation={first_incarnation}; returned_incarnation={returned_incarnation}; result={result:?}; stack={:?}; events={:?}",
        game.stack,
        game.canonical_event_log(),
    );
    result.expect("copying the new spell incarnation must resolve successfully");

    assert_eq!(
        game.stack.len(),
        3,
        "copy spell leaves and one virtual copy enters"
    );
    let virtual_copy = game.stack.last().expect("virtual copy is on top");
    assert_ne!(virtual_copy.card, transmuter);
    assert_eq!(virtual_copy.ability_id, None);
    assert_eq!(virtual_copy.source_incarnation, spell_incarnation);
    assert_eq!(
        virtual_copy.effects,
        [Effect::GainLifeController { amount: 2 }]
    );
    assert_eq!(game.zone_of(copy_spell), Some(Zone::Graveyard));
    assert!(matches!(
        game.event_log.last(),
        Some(GameEvent::PolicyMoveSubmitted { player: receipt_player, .. }) if *receipt_player == opponent
    ));
    assert_eq!(
        game.event_log[events_before_resolution.len()..]
            .iter()
            .filter(|event| matches!(event, GameEvent::SpellCopied { original, original_source_incarnation, .. }
                if *original == transmuter && *original_source_incarnation == spell_incarnation))
            .count(),
        1,
        "the receipt names the exact copied spell incarnation"
    );

    pass_pair(&mut game, player, opponent);
    assert_eq!(game.stack.len(), 2, "the virtual copy resolves first");
    assert_eq!(game.player(player).expect("player exists").life, 22);
    assert!(matches!(
        game.event_log
            .iter()
            .rev()
            .find(|event| matches!(event, GameEvent::SpellCopyResolved { .. })),
        Some(GameEvent::SpellCopyResolved { original, .. }) if *original == transmuter
    ));

    pass_pair(&mut game, player, opponent);
    assert_eq!(game.stack.len(), 1, "the physical spell resolves second");
    assert_eq!(game.player(player).expect("player exists").life, 24);
    assert_eq!(game.zone_of(transmuter), Some(Zone::Graveyard));
    let remaining = game.stack.last().expect("old ability remains last");
    assert_eq!(remaining.card, transmuter);
    assert_eq!(remaining.source_incarnation, first_incarnation);
    assert_eq!(remaining.ability_id, Some("transmute"));
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::AbilityResolved {
                source,
                source_incarnation,
                ability: "transmute",
            } if *source == transmuter && *source_incarnation == first_incarnation
        )),
        "the lower ability has not resolved out of LIFO order"
    );
    game.validate_invariants()
        .expect("every resolved copy/spell boundary remains invariant-valid");
}
