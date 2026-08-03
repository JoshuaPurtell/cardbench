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

#[test]
fn policy_cannot_target_transmute_ability_as_an_instant_spell() {
    let player = PlayerId(0);
    let mut game = Game::new(
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
        ],
        2,
    )
    .expect("fixture initializes");
    let transmuter = game
        .add_card(player, TRANSMUTER, Zone::Hand)
        .expect("Transmute source enters hand");
    let copy = game
        .add_card(player, COPY, Zone::Hand)
        .expect("copy spell enters hand");

    game.submit_policy_move(
        player,
        "adversarial.transmute-target.v1",
        PolicyAction::Transmute { card: transmuter },
    )
    .expect("Transmute ability is legally activated");

    let before = game.canonical_event_log();
    let result = game.submit_policy_move(
        player,
        "adversarial.transmute-target.v1",
        PolicyAction::Cast(CastRequest {
            card: copy,
            targets: vec![Target::Spell(transmuter)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        }),
    );
    eprintln!(
        "activated-ability spell-target result: {result:?}; stack: {:?}; events: {:?}",
        game.stack,
        game.canonical_event_log(),
    );

    assert!(
        matches!(result, Err(RulesError::IllegalTarget(Target::Spell(card))) if card == transmuter),
        "an activated ability cannot satisfy an instant-or-sorcery spell target"
    );
    assert_eq!(
        game.canonical_event_log(),
        before,
        "the rejected policy move must be atomic"
    );
    game.validate_invariants()
        .expect("rejected target leaves the game invariant-valid");
}
