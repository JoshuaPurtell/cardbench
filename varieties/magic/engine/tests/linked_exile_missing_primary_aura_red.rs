//! RED: an Aura-linked delayed return must not return its Auras when the
//! exiled primary creature card has ceased to exist before the end step.
//!
//! The Aura return is conditional on returning the creature.  This is an
//! ordinary three-player game: the creature's owner loses after the blink,
//! which removes that exiled card from the game while the Aura controller
//! remains alive to reach the scheduled end step.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AbilityActivation, ActivatedAbility, ActivatedAbilityBinding, AttachmentBinding,
    AttachmentKind, CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, ManaCost,
    PlayerId, Step, Target, TargetRequirement, Zone,
};

const CREATURE: &str = "TST-LINKED-EXILE-MISSING-PRIMARY-CREATURE";
const AURA: &str = "TST-LINKED-EXILE-MISSING-PRIMARY-AURA";
const KILLER: &str = "TST-LINKED-EXILE-MISSING-PRIMARY-KILLER";

fn definition(id: &'static str, card_type: CardType, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["linked-exile-missing-primary-aura-red"],
        power: (id == CREATURE).then_some(2),
        toughness: (id == CREATURE).then_some(2),
        keywords: vec![],
        effects,
    }
}

fn resolve_top(game: &mut Game) {
    for _ in 0..game.players.iter().filter(|player| !player.lost).count() {
        let player = game.priority;
        game.pass_priority(player)
            .expect("each living player passes the top stack object");
    }
}

fn advance_to_end_step(game: &mut Game) {
    for _ in 0..32 {
        if game.step == Step::End {
            return;
        }
        if game
            .view_for_player(game.next_policy_player())
            .expect("draw view is available")
            .draw_replacement_pending
        {
            let player = game.next_policy_player();
            game.resolve_pending_draw(player, None)
                .expect("ordinary draw resolves before priority");
            continue;
        }
        if game.step == Step::DeclareAttackers
            && !game
                .view_for_player(game.active_player)
                .expect("combat view is available")
                .attackers_declared
        {
            game.declare_attackers(game.active_player, &[])
                .expect("empty attackers are declared");
            continue;
        }
        if game.step == Step::DeclareBlockers
            && !game
                .view_for_player(game.next_policy_player())
                .expect("combat view is available")
                .blockers_declared
        {
            game.declare_blockers(game.next_policy_player(), &[])
                .expect("empty blockers are declared");
            continue;
        }
        let player = game.priority;
        game.pass_priority(player)
            .expect("living priority holder advances turn structure");
    }
    panic!("fixture did not reach the scheduled end step");
}

#[test]
#[allow(clippy::too_many_lines)] // The player-departure and delayed-action transcript is the regression.
fn linked_auras_stay_exiled_when_the_primary_card_left_the_game() {
    let aura_controller = PlayerId(0);
    let primary_owner = PlayerId(1);
    let third_player = PlayerId(2);
    let mut game = Game::new_with_all_bindings(
        [
            definition(CREATURE, CardType::Creature, vec![]),
            definition(
                AURA,
                CardType::Enchantment,
                vec![Effect::AttachSourceToTarget {
                    target: TargetRequirement::Creature,
                    changes: vec![],
                }],
            ),
            definition(
                KILLER,
                CardType::Instant,
                vec![Effect::DealDamage {
                    amount: 20,
                    target: TargetRequirement::Player,
                }],
            ),
        ],
        3,
        [],
        [],
        [],
        [ActivatedAbilityBinding {
            card_definition: AURA,
            ability: ActivatedAbility {
                id: "exile-linked-group",
                mana_cost: ManaCost::new(0),
                tap_cost: false,
                sorcery_speed: false,
                additional_tap_creatures: 0,
                sacrifice_source: false,
                sacrifice_creatures: 0,
                sacrifice_lands: 0,
                discard_cards: 0,
                targets: vec![],
                effects: vec![Effect::ExileAttachedCreatureAndAurasUntilEndStep],
            },
        }],
    )
    .expect("fixture initializes");
    game.register_attachment_bindings([AttachmentBinding {
        card_definition: AURA,
        kind: AttachmentKind::Aura,
        target: TargetRequirement::Creature,
        changes: vec![],
        granted_activated_abilities: vec![],
    }])
    .expect("Aura binding registers");

    let creature = game
        .put_on_battlefield(primary_owner, CREATURE)
        .expect("primary creature begins on the battlefield");
    let aura = game
        .add_card(aura_controller, AURA, Zone::Hand)
        .expect("Aura begins in hand");
    let killer = game
        .add_card(aura_controller, KILLER, Zone::Hand)
        .expect("lethal spell begins in hand");
    game.add_card(aura_controller, CREATURE, Zone::Library)
        .expect("active player has an ordinary draw-step card");
    game.enter_attachment_without_cast(aura, creature)
        .expect("pregame Aura setup attaches to the opposing creature");
    game.begin_game().expect("game begins");

    game.activate_ability(
        aura_controller,
        AbilityActivation {
            source: aura,
            ability_id: "exile-linked-group",
            sacrifice_sources: vec![],
            additional_tap_creatures: vec![],
            discard_cards: vec![],
            targets: vec![],
        },
    )
    .expect("Aura-linked blink activation stacks");
    resolve_top(&mut game);
    assert_eq!(game.zone_of(creature), Some(Zone::Exile));
    assert_eq!(game.zone_of(aura), Some(Zone::Exile));

    game.cast_spell(
        aura_controller,
        CastRequest {
            card: killer,
            targets: vec![Target::Player(primary_owner)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("surviving Aura controller casts the lethal spell");
    resolve_top(&mut game);
    assert!(
        game.player(primary_owner)
            .expect("departing seat remains queryable")
            .lost
    );
    assert!(!game.player(third_player).expect("third seat exists").lost);
    assert_eq!(
        game.object(creature),
        Err(cardbench_magic_engine::RulesError::UnknownCard(creature)),
        "the exiled primary card leaves with its owner"
    );

    advance_to_end_step(&mut game);
    eprintln!(
        "missing-primary Aura-linked return trace={:?}",
        game.canonical_event_log()
    );
    assert_eq!(
        game.zone_of(aura),
        Some(Zone::Exile),
        "an Aura return is contingent on returning the primary creature card"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::DelayedActionConsumed { returned, .. } if returned.is_empty()
    )));
    game.validate_invariants()
        .expect("missing-primary linked return remains auditable");
}
