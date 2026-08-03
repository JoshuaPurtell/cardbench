//! Red regression: a copied token's received-damage trigger survives lethal
//! damage and the resulting SBA token cessation.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    AttachmentBinding, AttachmentKind, CardDefinition, CardType, CastRequest, Effect, Game,
    GameEvent, ManaCost, PlayerId, Target, TargetRequirement, TriggerCondition, TriggeredAbility,
    TriggeredAbilityBinding, Zone,
};

const CREATURE: &str = "TST-COPIED-TOKEN-LETHAL-DAMAGE-CREATURE";
const COPY_AURA: &str = "TST-COPIED-TOKEN-LETHAL-DAMAGE-AURA";
const DAMAGE: &str = "TST-COPIED-TOKEN-LETHAL-DAMAGE-SPELL";

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
        supported_rules: &["copied-token-lethal-damage-trigger-red"],
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
#[allow(clippy::too_many_lines)] // The damage/SBA/departed-trigger transcript is the regression contract.
fn lethal_damage_token_sba_keeps_its_received_damage_trigger_provenance() {
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
                DAMAGE,
                BTreeSet::from([CardType::Instant]),
                vec![Effect::DealDamage {
                    amount: 2,
                    target: TargetRequirement::Creature,
                }],
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
                    id: "received-damage-life",
                    condition: TriggerCondition::ReceivesDamage,
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
    let damage = game
        .add_card(controller, DAMAGE, Zone::Hand)
        .expect("damage spell setup");
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
            card: damage,
            targets: vec![Target::Permanent(token)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("lethal damage spell casts");
    pass_pair(&mut game);
    eprintln!(
        "copied token lethal-damage trigger trace={:?}",
        game.canonical_event_log()
    );
    assert!(
        game.event_log.iter().any(|event| matches!(
            event,
            GameEvent::TriggeredAbilityStacked { source, ability, .. }
                if *source == token && *ability == "received-damage-life"
        )),
        "the token's received-damage trigger must survive its lethal SBA departure"
    );
    pass_pair(&mut game);
    assert_eq!(
        game.player(controller)
            .expect("controller remains live")
            .life,
        21
    );
    game.validate_invariants()
        .expect("departed received-damage token source remains auditable");
}
