//! Token type-line contracts for the expansion-neutral engine.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, CreatureSubtype, Effect, Game, ManaCost,
    PlayerId, TokenSpec, Zone,
};

const TOKEN_SPELL: &str = "TST-SAPROLING-TOKEN-SPELL";

fn token_spell_with(id: &'static str, token: TokenSpec) -> CardDefinition {
    CardDefinition {
        id,
        name: "Saproling token probe",
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Sorcery]),
        is_basic_land: false,
        supported_rules: &["typed-saproling-token"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::CreateToken { token, count: 1 }],
    }
}

fn token_spell() -> CardDefinition {
    token_spell_with(TOKEN_SPELL, TokenSpec::saproling())
}

fn resolved_saproling_game() -> (Game, cardbench_magic_engine::ObjectId) {
    let first = PlayerId(0);
    let second = PlayerId(1);
    let mut game = Game::new([token_spell()], 2).expect("fixture game initializes");
    let spell = game
        .add_card(first, TOKEN_SPELL, Zone::Hand)
        .expect("token spell enters hand");
    game.cast_spell(
        first,
        CastRequest {
            card: spell,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("token spell casts");
    game.pass_priority(first).expect("caster passes");
    game.pass_priority(second).expect("spell resolves");
    let token = *game.players[first.0]
        .battlefield
        .first()
        .expect("resolution creates one token");
    (game, token)
}

#[test]
fn saproling_token_keeps_a_typed_creature_subtype_through_resolution() {
    let (game, token) = resolved_saproling_game();

    let characteristics = game.characteristics(token).expect("token exists");
    assert!(characteristics.card_types.contains(&CardType::Creature));
    assert_eq!(
        characteristics.creature_subtypes,
        BTreeSet::from([CreatureSubtype::Saproling])
    );
    game.validate_invariants()
        .expect("typed token state remains internally valid");
}

#[test]
fn invariant_rejects_a_creature_subtype_on_a_noncreature_token() {
    let first = PlayerId(0);
    let second = PlayerId(1);
    let invalid_id = "TST-INVALID-TOKEN-SUBTYPE";
    let invalid_token = TokenSpec {
        name: "Invalid type-line probe",
        colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Artifact]),
        creature_subtypes: BTreeSet::from([CreatureSubtype::Saproling]),
        power: 0,
        toughness: 0,
    };
    let mut game = Game::new([token_spell_with(invalid_id, invalid_token)], 2)
        .expect("fixture game initializes");
    let spell = game
        .add_card(first, invalid_id, Zone::Hand)
        .expect("invalid token spell enters hand");
    game.cast_spell(
        first,
        CastRequest {
            card: spell,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("spell may be placed on the stack before its invalid outcome is known");
    game.pass_priority(first).expect("caster passes");

    let resolution = game.pass_priority(second);
    assert!(
        resolution.is_err(),
        "the final priority pass must reject a token whose type line violates the invariant"
    );
    assert_eq!(game.stack.len(), 1, "failed resolution rolls back the pop");
    assert_eq!(
        game.priority, second,
        "failed resolution rolls back priority"
    );
    game.validate_invariants()
        .expect("the atomic rollback leaves a valid pre-resolution state");
}
