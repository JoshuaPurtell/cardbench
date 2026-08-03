//! Red regression: a departing player must take virtual spell copies with them.
//!
//! A spell copy is stack-only metadata, so player-loss cleanup cannot rely on
//! the ordinary owned-object zone loop. In a continuing multiplayer game, a
//! copy controlled by a player who left must cease alongside their physical
//! spells; it must never survive as a stack object controlled by a departed
//! seat.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, ManaCost, PlayerId, Target,
    Zone,
};

const PING: &str = "TST-DEPARTING-COPY-PING";
const COPY: &str = "TST-DEPARTING-COPY-EFFECT";

fn definition(id: &'static str, effects: Vec<Effect>) -> CardDefinition {
    CardDefinition {
        id,
        name: id,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["virtual-spell-copy-departure-red"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects,
    }
}

fn request(card: cardbench_magic_engine::ObjectId, targets: Vec<Target>) -> CastRequest {
    CastRequest {
        card,
        targets,
        convoke: vec![],
        payment_mana_abilities: vec![],
    }
}

fn resolve_top_in_three_player_game(game: &mut Game) {
    for _ in 0..3 {
        let player = game.priority;
        game.pass_priority(player)
            .expect("each surviving player passes priority");
    }
}

#[test]
fn player_loss_removes_their_virtual_spell_copy_from_a_continuing_game() {
    let departing = PlayerId(0);
    let target = PlayerId(1);
    let mut game = Game::new(
        [
            definition(
                PING,
                vec![Effect::DealDamage {
                    amount: 1,
                    target: cardbench_magic_engine::TargetRequirement::Player,
                }],
            ),
            definition(
                COPY,
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
        ],
        3,
    )
    .expect("three-player fixture initializes");
    let ping = game
        .add_card(departing, PING, Zone::Hand)
        .expect("physical spell enters hand");
    let copy_effect = game
        .add_card(departing, COPY, Zone::Hand)
        .expect("copy effect enters hand");
    game.begin_game().expect("game begins");

    game.cast_spell(departing, request(ping, vec![Target::Player(target)]))
        .expect("physical spell casts");
    game.cast_spell(departing, request(copy_effect, vec![Target::Spell(ping)]))
        .expect("copy effect casts above physical spell");
    resolve_top_in_three_player_game(&mut game);

    let copy = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCopied {
                copy, controller, ..
            } if *controller == departing => Some(*copy),
            _ => None,
        })
        .expect("copy effect created a virtual spell copy");
    assert!(
        game.stack.iter().any(|stack| stack.card == copy),
        "the virtual copy waits above its original spell"
    );

    // This is the same externally visible state that follows lethal damage
    // before the next priority window. Player 2 remains, so the game itself
    // must continue after player 0 leaves.
    game.set_fixture_player_life(departing, 0)
        .expect("fixture marks departing player at zero life");
    game.check_state_based_actions()
        .expect("player-loss SBA processing completes");

    eprintln!(
        "departing virtual-copy trace: stack={:?}; events={:?}",
        game.stack,
        game.canonical_event_log()
    );
    assert!(
        game.stack.is_empty(),
        "a player who left the game cannot retain a virtual spell copy on the stack"
    );
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCopyLeftGame {
            copy: departed_copy,
            original,
            controller,
        } if *departed_copy == copy && *original == ping && *controller == departing
    )));
    game.validate_invariants()
        .expect("no stack object remains controlled by the departed player");
}
