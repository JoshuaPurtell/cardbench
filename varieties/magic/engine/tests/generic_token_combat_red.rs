//! Red regression: a definitionless creature token may declare an attack.
//!
//! Generic tokens have characteristics but no `CardDefinition`; combat trigger
//! dispatch must skip only nonexistent definition-bound abilities, never roll
//! back their otherwise legal attack declaration.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Color, Effect, Game, GameEvent, ManaCost, PlayerId,
    Step, TokenSpec, Zone,
};

const TOKEN_SPELL: &str = "TST-GENERIC-TOKEN-COMBAT-SPELL";

fn token_spell() -> CardDefinition {
    CardDefinition {
        id: TOKEN_SPELL,
        name: TOKEN_SPELL,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::from([Color::Green]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["generic-token-combat-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::CreateToken {
            token: TokenSpec::saproling(),
            count: 1,
        }],
    }
}

fn advance_to_turn_three_attackers(game: &mut Game) {
    for _ in 0..80 {
        if game.turn == 3
            && game.active_player == PlayerId(0)
            && game.step == Step::DeclareAttackers
        {
            return;
        }
        if game
            .view_for_player(game.next_policy_player())
            .expect("draw-replacement view")
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
                .expect("active-player combat view")
                .attackers_declared
        {
            game.declare_attackers(game.active_player, &[])
                .expect("earlier empty attack declaration succeeds");
            continue;
        }
        let player = game.priority;
        game.pass_priority(player)
            .expect("priority advances the public turn state");
    }
    panic!(
        "fixture did not reach player zero turn-three attackers: turn={} active={:?} step={:?}",
        game.turn, game.active_player, game.step
    );
}

#[test]
fn generic_token_attacks_without_a_definition_bound_trigger() {
    let controller = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new([token_spell()], 2).expect("fixture initializes");
    let maker = game
        .add_card(controller, TOKEN_SPELL, Zone::Hand)
        .expect("token maker setup");
    // Reaching the controller's next attack step performs two controller draws
    // and one opponent draw. Keep the fixture alive through that ordinary
    // turn structure without fabricating a live game state.
    for player in [controller, opponent] {
        for _ in 0..2 {
            game.add_card(player, TOKEN_SPELL, Zone::Library)
                .expect("draw filler setup");
        }
    }
    game.begin_game().expect("game begins");
    game.cast_spell(
        controller,
        CastRequest {
            card: maker,
            targets: vec![],
            convoke: vec![],
            payment_mana_abilities: vec![],
        },
    )
    .expect("token maker casts");
    let first = game.priority;
    game.pass_priority(first).expect("caster passes");
    let second = game.priority;
    game.pass_priority(second).expect("token maker resolves");
    let token = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::TokenCreated { player, token } if *player == controller => Some(*token),
            _ => None,
        })
        .expect("the spell creates one generic Saproling token");

    advance_to_turn_three_attackers(&mut game);
    game.clear_event_log();
    game.declare_attackers(controller, &[token])
        .expect("an established generic creature token may attack");
    eprintln!(
        "generic token combat trace={:?}",
        game.canonical_event_log()
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::AttackersDeclared { player, attackers }
            if *player == controller && attackers == &vec![token]
    )));
    assert!(game.object(token).expect("token remains live").tapped);
    game.validate_invariants()
        .expect("generic token attack state remains auditable");
}
