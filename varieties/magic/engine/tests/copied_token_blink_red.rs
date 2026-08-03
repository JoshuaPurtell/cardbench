//! Red regression: an end-step blink effect cannot retain a token return.
//!
//! Exiling a copied creature token is a non-graveyard battlefield departure:
//! it ceases immediately, is not a death, and creates no delayed return group
//! for an object that no longer exists.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AttachmentBinding, AttachmentKind, CardDefinition, CardType, CastRequest, Effect, Game,
    GameEvent, ManaCost, PlayerId, Target, TargetRequirement, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding, Zone,
};

const CREATURE: &str = "TST-COPIED-TOKEN-BLINK-CREATURE";
const COPY_AURA: &str = "TST-COPIED-TOKEN-BLINK-AURA";
const BLINK: &str = "TST-COPIED-TOKEN-BLINK-SPELL";

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
        supported_rules: &["copied-token-blink-red"],
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
#[allow(clippy::too_many_lines)] // The blink/cessation transcript is the regression contract.
fn blinking_a_copied_token_ceases_it_without_scheduling_an_impossible_return() {
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
                BLINK,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::ExileTargetCreatureUntilEndStep],
            ),
        ],
        2,
        [],
        [],
        [],
        [],
        [TriggeredAbilityBinding {
            card_definition: COPY_AURA,
            ability: TriggeredAbility {
                id: "copy-attached-creature-at-upkeep",
                condition: TriggerCondition::BeginningOfAttachedCreaturesControllerUpkeep,
                mana_cost: ManaCost::new(0),
                optional: false,
                targets: vec![],
                effects: vec![Effect::CreateTokenCopyOfAttachedCreature],
            },
        }],
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
    let blink = game
        .add_card(controller, BLINK, Zone::Hand)
        .expect("blink spell setup");
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
            card: blink,
            targets: vec![Target::Permanent(token)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("blink spell casts");
    pass_pair(&mut game);
    eprintln!("copied token blink trace={:?}", game.canonical_event_log());

    assert_eq!(
        game.object(token),
        Err(cardbench_magic_engine::RulesError::UnknownCard(token)),
        "the exiled token must cease instead of becoming a delayed-return member"
    );
    assert!(game.event_log.iter().any(
        |event| matches!(event, GameEvent::TokenCeasedToExist { token: ceased } if *ceased == token),
    ));
    assert!(
        !game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::DelayedActionScheduled { .. } | GameEvent::DelayedActionConsumed { .. }
        )),
        "the engine must not retain a delayed return for a ceased token"
    );
    assert!(
        !game
            .event_log
            .iter()
            .any(|event| matches!(event, GameEvent::CardMoved { card, .. } if *card == token),),
        "the token must not acquire an exile-zone move receipt"
    );
    game.validate_invariants()
        .expect("token blink cessation remains auditable");
}
