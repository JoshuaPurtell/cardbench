//! Red regression: a narrower shared-target requirement entails its member target kind.
//!
//! One printed “target nonblack creature” occurrence may drive several
//! target-preserving creature instructions.  The bundle must preserve that
//! one narrow target occurrence, not reject the definition merely because its
//! members use the general `Creature` instruction substrate.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, Keyword, ManaCost, PlayerId, Target,
    TargetRequirement, Zone,
};

const CREATURE: &str = "TST-BUNDLE-SUBTYPE-CREATURE";
const SPELL: &str = "TST-BUNDLE-SUBTYPE-SPELL";

fn definition(
    id: &'static str,
    card_type: CardType,
    effects: Vec<Effect>,
    power: Option<i16>,
    toughness: Option<i16>,
) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([card_type]),
        is_basic_land: false,
        supported_rules: &["targeted-bundle-requirement-subtyping-red"],
        power,
        toughness,
        keywords: vec![],
        effects,
    }
}

#[test]
fn narrower_creature_target_can_drive_a_shared_creature_instruction_bundle() {
    let mut game = Game::new(
        [
            definition(CREATURE, CardType::Creature, vec![], Some(1), Some(1)),
            definition(
                SPELL,
                CardType::Instant,
                vec![Effect::TargetedBundle {
                    target: TargetRequirement::NonblackCreature,
                    effects: vec![
                        Effect::ModifyTargetPtUntilEndOfTurn {
                            power: 1,
                            toughness: 1,
                        },
                        Effect::ModifyTargetKeywordUntilEndOfTurn {
                            keyword: Keyword::Haste,
                        },
                    ],
                }],
                None,
                None,
            ),
        ],
        2,
    )
    .expect("fixture initializes");
    let target = game
        .put_on_battlefield(PlayerId(1), CREATURE)
        .expect("target enters battlefield");
    let spell = game
        .add_card(PlayerId(0), SPELL, Zone::Hand)
        .expect("spell enters hand");
    game.begin_game().expect("game begins");

    let result = game.cast_spell(
        PlayerId(0),
        CastRequest {
            card: spell,
            targets: vec![Target::Permanent(target)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    );
    eprintln!(
        "narrow shared-target bundle result: {result:?}; stack={:?}; events={:?}",
        game.stack,
        game.canonical_event_log()
    );
    assert!(
        result.is_ok(),
        "a nonblack-creature target occurrence entails its creature bundle members"
    );
}
