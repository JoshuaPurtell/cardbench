//! Red regression: a token returned from the battlefield ceases to exist.
//!
//! A token cannot take an ordinary hand-zone transition.  Its departure is a
//! leaves-the-battlefield event (so another permanent can observe it), but it
//! is not a death and must not fire its own `Dies` trigger.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AttachmentBinding, AttachmentKind, CardDefinition, CardType, CastRequest, Effect, Game,
    GameEvent, ManaCost, PlayerId, Target, TargetRequirement, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding, Zone,
};

const CREATURE: &str = "TST-COPIED-TOKEN-BOUNCE-CREATURE";
const COPY_AURA: &str = "TST-COPIED-TOKEN-BOUNCE-AURA";
const BOUNCE: &str = "TST-COPIED-TOKEN-BOUNCE-SPELL";

fn definition(
    id: &'static str,
    card_types: BTreeSet<CardType>,
    effects: Vec<Effect>,
) -> CardDefinition {
    let creature = card_types.contains(&CardType::Creature);
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types,
        is_basic_land: false,
        supported_rules: &["copied-token-bounce-red"],
        power: creature.then_some(2),
        toughness: creature.then_some(2),
        keywords: vec![],
        effects,
    }
}

fn pass_pair(game: &mut Game) {
    let first = game.priority;
    game.pass_priority(first).expect("first player passes");
    let second = game.priority;
    game.pass_priority(second).expect("second player passes");
}

#[test]
#[allow(clippy::too_many_lines)] // The bounce/trigger transcript is the regression contract.
fn bounced_copied_token_ceases_without_dying_but_other_permanents_observe_its_departure() {
    let controller = PlayerId(0);
    let mut game = Game::new_with_all_bindings_and_triggers(
        [
            definition(CREATURE, BTreeSet::from([CardType::Creature]), vec![]),
            definition(
                COPY_AURA,
                BTreeSet::from([CardType::Enchantment]),
                vec![Effect::AttachSourceToTarget {
                    target: TargetRequirement::ControlledCreature,
                    changes: vec![],
                }],
            ),
            definition(
                BOUNCE,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::ReturnControlledCreatureToHand],
            ),
        ],
        2,
        [],
        [],
        [],
        [],
        [
            TriggeredAbilityBinding {
                card_definition: COPY_AURA,
                ability: TriggeredAbility {
                    id: "copy-attached-creature-at-upkeep",
                    condition: TriggerCondition::BeginningOfAttachedCreaturesControllerUpkeep,
                    mana_cost: ManaCost::new(0),
                    optional: false,
                    targets: vec![],
                    effects: vec![Effect::CreateTokenCopyOfAttachedCreature],
                },
            },
            TriggeredAbilityBinding {
                card_definition: CREATURE,
                ability: TriggeredAbility {
                    id: "another-creature-left",
                    condition: TriggerCondition::AnotherCreatureLeavesBattlefield,
                    mana_cost: ManaCost::new(0),
                    optional: false,
                    targets: vec![],
                    effects: vec![Effect::GainLifeController { amount: 1 }],
                },
            },
            TriggeredAbilityBinding {
                card_definition: CREATURE,
                ability: TriggeredAbility {
                    id: "must-not-die-from-bounce",
                    condition: TriggerCondition::Dies,
                    mana_cost: ManaCost::new(0),
                    optional: false,
                    targets: vec![],
                    effects: vec![Effect::GainLifeController { amount: 10 }],
                },
            },
        ],
    )
    .expect("fixture initializes");
    game.register_attachment_bindings([AttachmentBinding {
        card_definition: COPY_AURA,
        kind: AttachmentKind::Aura,
        target: TargetRequirement::ControlledCreature,
        changes: vec![],
        granted_activated_abilities: vec![],
    }])
    .expect("copy Aura binding registers");
    let creature = game
        .put_on_battlefield(controller, CREATURE)
        .expect("creature setup");
    let aura = game
        .add_card(controller, COPY_AURA, Zone::Hand)
        .expect("Aura setup");
    game.enter_attachment_without_cast(aura, creature)
        .expect("pregame Aura setup attaches");
    let bounce = game
        .add_card(controller, BOUNCE, Zone::Hand)
        .expect("bounce spell setup");
    game.begin_game()
        .expect("game begins at the attached controller upkeep");
    pass_pair(&mut game);
    let token = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::TokenCreated { player, token } if *player == controller => Some(*token),
            _ => None,
        })
        .expect("upkeep ability creates one copied token");

    game.clear_event_log();
    game.cast_spell(
        controller,
        CastRequest {
            card: bounce,
            targets: vec![Target::Permanent(token)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("bounce spell casts");
    pass_pair(&mut game);
    eprintln!("copied token bounce trace={:?}", game.canonical_event_log());

    assert_eq!(
        game.object(token),
        Err(cardbench_magic_engine::RulesError::UnknownCard(token)),
        "the bounced token must cease rather than exist in its owner's hand"
    );
    assert!(game.event_log.iter().any(
        |event| matches!(event, GameEvent::TokenCeasedToExist { token: ceased } if *ceased == token),
    ));
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::TriggeredAbilityStacked { source, ability, .. }
            if *source == creature && *ability == "another-creature-left"
    )));
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::TriggeredAbilityStacked { source, ability, .. }
                if *source == token && *ability == "must-not-die-from-bounce"
        )),
        "a token returned to hand leaves the battlefield but does not die"
    );
    assert!(
        !game.event_log.iter().any(
            |event| matches!(event, GameEvent::CardMoved { card, to: Zone::Hand } if *card == token),
        ),
        "token cessation must not forge a hand-zone move receipt"
    );
    pass_pair(&mut game);
    assert_eq!(
        game.player(controller)
            .expect("controller remains live")
            .life,
        21,
        "the original creature observes the token departure exactly once"
    );
    game.validate_invariants()
        .expect("non-graveyard token departure remains auditable");
}
