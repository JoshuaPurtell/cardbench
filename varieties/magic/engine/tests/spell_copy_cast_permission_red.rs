//! Red regression for effect-created casting permissions and virtual stack
//! spell copies.
//!
//! A copy of an instant or sorcery is a distinct stack object, not a second
//! zone transition for the physical card.  When an effect allows new targets,
//! choosing them is a no-priority, stale-safe `DecisionId` boundary.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastPermissionPayment, CastRequest, CastTiming, Color, DecisionKind,
    DecisionSelection, Effect, Game, GameEvent, ManaCost, PlayerId, Target, TargetRequirement,
    Zone,
};

const PING: &str = "TST-COPY-PING";
const COPY: &str = "TST-COPY-EFFECT";
const COPY_RETAIN: &str = "TST-COPY-RETAIN";
const EXILE_PERMISSION: &str = "TST-EXILE-PERMISSION";
const SORCERY: &str = "TST-EXILE-SORCERY";
const ISLAND: &str = "TST-EXILE-ISLAND";

fn definition(
    id: &'static str,
    types: BTreeSet<CardType>,
    cost: ManaCost,
    effects: Vec<Effect>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: cost,
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: types,
        is_basic_land: false,
        supported_rules: &["spell-copy-cast-permission-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

fn game() -> Game {
    Game::new(
        vec![
            definition(
                PING,
                BTreeSet::from([CardType::Instant]),
                ManaCost::new(0),
                vec![Effect::DealDamage {
                    amount: 2,
                    target: TargetRequirement::Player,
                }],
            ),
            definition(
                COPY,
                BTreeSet::from([CardType::Instant]),
                ManaCost::new(0),
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: true,
                }],
            ),
            definition(
                COPY_RETAIN,
                BTreeSet::from([CardType::Instant]),
                ManaCost::new(0),
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
            definition(
                EXILE_PERMISSION,
                BTreeSet::from([CardType::Instant]),
                ManaCost::new(0),
                vec![Effect::GrantExileCastPermissionUntilEndOfTurn {
                    payment: CastPermissionPayment::PayManaCost,
                    timing: CastTiming::AsThoughInstant,
                }],
            ),
            definition(
                SORCERY,
                BTreeSet::from([CardType::Sorcery]),
                ManaCost::new(1),
                vec![Effect::DealDamage {
                    amount: 1,
                    target: TargetRequirement::Player,
                }],
            ),
            CardDefinition {
                id: ISLAND,
                name: ISLAND,
                set_code: "TST",
                mana_cost: ManaCost::new(0),
                colors: BTreeSet::new(),
                mana_colors: BTreeSet::from([Color::Blue]),
                card_types: BTreeSet::from([CardType::Land]),
                is_basic_land: false,
                supported_rules: &["spell-copy-cast-permission-red"],
                power: None,
                toughness: None,
                keywords: vec![],
                effects: vec![],
            },
        ],
        2,
    )
    .expect("fixture initializes")
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
fn copied_spell_is_a_distinct_stack_item_and_reselects_targets_via_decision_id() {
    let mut game = game();
    let ping = game
        .add_card(PlayerId(0), PING, Zone::Hand)
        .expect("ping enters hand");
    let copy = game
        .add_card(PlayerId(0), COPY, Zone::Hand)
        .expect("copy spell enters hand");
    game.begin_game().expect("game begins");

    game.cast_spell(
        PlayerId(0),
        request(ping, vec![Target::Player(PlayerId(1))]),
    )
    .expect("ping casts");
    game.cast_spell(PlayerId(0), request(copy, vec![Target::Spell(ping)]))
        .expect("copy effect casts");
    resolve_top(&mut game);

    let decision = game
        .view_for_player(PlayerId(0))
        .expect("controller view")
        .pending_decision
        .expect("copy target decision opens");
    assert_eq!(decision.kind, DecisionKind::SpellCopyTargets);
    assert!(
        decision
            .target_candidates
            .contains(&Target::Player(PlayerId(0)))
    );
    game.submit_decision(
        PlayerId(0),
        decision.id,
        DecisionSelection::Targets(vec![Target::Player(PlayerId(0))]),
    )
    .expect("controller retargets copied spell");
    assert_eq!(game.stack.len(), 2, "copy sits above the original spell");
    assert_eq!(game.zone_of(copy), Some(Zone::Graveyard));
    assert_eq!(
        game.zone_of(ping),
        None,
        "physical ping remains its original stack object"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCopied { original, .. } if *original == ping
    )));

    resolve_top(&mut game);
    assert_eq!(
        game.players[0].life, 18,
        "retargeted copy damages its new target"
    );
    assert_eq!(game.players[1].life, 20, "original has not yet resolved");
    assert!(
        game.event_log
            .iter()
            .any(|event| matches!(event, GameEvent::SpellCopyResolved { .. }))
    );
    resolve_top(&mut game);
    assert_eq!(
        game.players[1].life, 18,
        "original still resolves independently"
    );
    game.validate_invariants()
        .expect("copy lifecycle is auditable");
}

#[test]
fn copy_effect_can_retain_the_original_targets_without_opening_a_decision() {
    let mut game = game();
    let ping = game
        .add_card(PlayerId(0), PING, Zone::Hand)
        .expect("ping enters hand");
    let copy = game
        .add_card(PlayerId(0), COPY_RETAIN, Zone::Hand)
        .expect("retained-target copy enters hand");
    game.begin_game().expect("game begins");

    game.cast_spell(
        PlayerId(0),
        request(ping, vec![Target::Player(PlayerId(1))]),
    )
    .expect("ping casts");
    game.cast_spell(PlayerId(0), request(copy, vec![Target::Spell(ping)]))
        .expect("retained-target copy casts");
    resolve_top(&mut game);
    assert!(
        game.view_for_player(PlayerId(0))
            .expect("view")
            .pending_decision
            .is_none(),
        "retaining targets is not a synthetic policy prompt"
    );
    assert_eq!(game.stack.len(), 2, "copy sits above original");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCopied {
            original,
            retargeted: false,
            ..
        } if *original == ping
    )));
    resolve_top(&mut game);
    resolve_top(&mut game);
    assert_eq!(
        game.players[1].life, 16,
        "copy and original retain the same target"
    );
    game.validate_invariants()
        .expect("retained-target copy lifecycle is auditable");
}

#[test]
fn exile_permission_can_pay_and_cast_sorcery_at_instant_speed() {
    let mut game = game();
    let permission = game
        .add_card(PlayerId(0), EXILE_PERMISSION, Zone::Hand)
        .expect("permission spell enters hand");
    let sorcery = game
        .add_card(PlayerId(0), SORCERY, Zone::Exile)
        .expect("sorcery starts exiled");
    let island = game
        .put_on_battlefield(PlayerId(0), ISLAND)
        .expect("mana source enters before game");
    game.begin_game().expect("game begins");

    game.cast_spell(
        PlayerId(0),
        request(permission, vec![Target::Permanent(sorcery)]),
    )
    .expect("permission spell casts");
    resolve_top(&mut game);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::CastPermissionGranted { card, .. } if *card == sorcery
    )));
    game.activate_mana_ability(PlayerId(0), island, Color::Blue)
        .expect("standard-cost permission permits an ordinary mana ability");

    game.cast_spell(
        PlayerId(0),
        request(sorcery, vec![Target::Player(PlayerId(1))]),
    )
    .expect("permission allows a sorcery while the stack is nonempty or off-main timing");
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCastFromPermission { card, from: Zone::Exile, .. } if *card == sorcery
    )));
    resolve_top(&mut game);
    assert_eq!(game.zone_of(sorcery), Some(Zone::Graveyard));
    game.validate_invariants()
        .expect("permission state is consumed safely");
}
