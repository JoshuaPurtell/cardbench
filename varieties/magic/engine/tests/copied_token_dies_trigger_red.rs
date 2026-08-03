//! Red regression: a copied token queues its own Dies trigger before ceasing.
//!
//! Token cessation is not a zone move, but a copied creature token's
//! definition-bound `Dies` ability still triggers from last-known battlefield
//! information and resolves after the token is gone.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AttachmentBinding, AttachmentKind, CardDefinition, CardType, CastRequest, Effect, Game,
    GameEvent, ManaCost, PlayerId, Target, TargetRequirement, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding, Zone,
};

const CREATURE: &str = "TST-COPIED-TOKEN-DIES-CREATURE";
const COPY_AURA: &str = "TST-COPIED-TOKEN-DIES-AURA";
const KILL: &str = "TST-COPIED-TOKEN-DIES-KILL";

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
        supported_rules: &["copied-token-dies-trigger-red"],
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
#[allow(clippy::too_many_lines)] // The copied-token departure/stack transcript is the regression contract.
fn copied_token_stacks_its_dies_trigger_before_ceasing_to_exist() {
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
                KILL,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::DestroyTargetNonblackCreature],
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
                    id: "copied-token-dies",
                    condition: TriggerCondition::Dies,
                    mana_cost: ManaCost::new(0),
                    optional: false,
                    targets: vec![],
                    effects: vec![Effect::GainLifeController { amount: 1 }],
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
    let kill = game
        .add_card(controller, KILL, Zone::Hand)
        .expect("kill spell setup");
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

    let death_start = game.event_log.len();
    game.cast_spell(
        controller,
        CastRequest {
            card: kill,
            targets: vec![Target::Permanent(token)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("kill spell casts");
    pass_pair(&mut game);
    eprintln!(
        "copied token dies trigger trace={:?}",
        &game.canonical_event_log()[death_start..]
    );

    assert_eq!(
        game.object(token),
        Err(cardbench_magic_engine::RulesError::UnknownCard(token))
    );
    assert!(
        game.event_log[death_start..].iter().any(|event| {
            matches!(
                event,
                GameEvent::TriggeredAbilityStacked { source, ability, .. }
                    if *source == token && *ability == "copied-token-dies"
            )
        }),
        "the copied token must stack its historical Dies trigger before it ceases"
    );
    pass_pair(&mut game);
    assert_eq!(
        game.player(controller)
            .expect("controller remains live")
            .life,
        21,
        "the departed copied token's Dies trigger resolves"
    );
    game.validate_invariants()
        .expect("departed copied-token trigger provenance is auditable");
}
