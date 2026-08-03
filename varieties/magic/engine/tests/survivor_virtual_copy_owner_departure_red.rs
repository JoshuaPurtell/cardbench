//! Red regression: a survivor-controlled spell copy remains after its
//! departed owner's physical original leaves the game.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, Effect, Game, GameEvent, ManaCost, PlayerId, Target,
    TargetRequirement, Zone,
};

const PING: &str = "TST-SURVIVOR-COPY-OWNER-DEPARTURE-PING";
const COPY: &str = "TST-SURVIVOR-COPY-OWNER-DEPARTURE-COPY";
const LETHAL: &str = "TST-SURVIVOR-COPY-OWNER-DEPARTURE-LETHAL";

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
        supported_rules: &["survivor-virtual-copy-owner-departure-red"],
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

fn pass_once(game: &mut Game) {
    let player = game.priority;
    game.pass_priority(player).expect("priority pass succeeds");
}

#[test]
#[allow(clippy::too_many_lines)] // One three-player copy/loss response chain owns this stack boundary.
fn departing_original_does_not_remove_survivors_virtual_copy() {
    let departing_owner = PlayerId(0);
    let copy_controller = PlayerId(1);
    let target = PlayerId(2);
    let mut game = Game::new(
        [
            definition(
                PING,
                vec![Effect::DealDamage {
                    amount: 1,
                    target: TargetRequirement::Player,
                }],
            ),
            definition(
                COPY,
                vec![Effect::CopyTargetInstantOrSorcerySpell {
                    may_choose_new_targets: false,
                }],
            ),
            definition(
                LETHAL,
                vec![Effect::DealDamage {
                    amount: 20,
                    target: TargetRequirement::Player,
                }],
            ),
        ],
        3,
    )
    .expect("three-player fixture initializes");
    let ping = game
        .add_card(departing_owner, PING, Zone::Hand)
        .expect("owner's spell enters hand");
    let copy_effect = game
        .add_card(copy_controller, COPY, Zone::Hand)
        .expect("copy effect enters survivor hand");
    let lethal = game
        .add_card(copy_controller, LETHAL, Zone::Hand)
        .expect("lethal response enters survivor hand");
    game.begin_game().expect("game begins");

    game.cast_spell(departing_owner, request(ping, vec![Target::Player(target)]))
        .expect("departing owner casts physical ping");
    pass_once(&mut game);
    game.cast_spell(
        copy_controller,
        request(copy_effect, vec![Target::Spell(ping)]),
    )
    .expect("survivor copies the physical ping");
    pass_once(&mut game);
    pass_once(&mut game);
    pass_once(&mut game);
    let copy = game
        .event_log
        .iter()
        .find_map(|event| match event {
            GameEvent::SpellCopied {
                copy,
                original,
                controller,
                ..
            } if *original == ping && *controller == copy_controller => Some(*copy),
            _ => None,
        })
        .expect("copy effect creates a survivor-controlled virtual spell");
    assert!(game.stack.iter().any(|item| item.card == copy));

    pass_once(&mut game);
    game.cast_spell(
        copy_controller,
        request(lethal, vec![Target::Player(departing_owner)]),
    )
    .expect("survivor casts lethal response above the copy");
    pass_once(&mut game);
    pass_once(&mut game);
    let resolver = game.priority;
    let loss_resolution = game.pass_priority(resolver);
    eprintln!(
        "survivor virtual-copy owner-departure red loss result={loss_resolution:?}; events={:?}",
        game.canonical_event_log(),
    );
    loss_resolution.expect("departing original must not invalidate survivor virtual copy");

    assert!(game.player(departing_owner).expect("owner exists").lost);
    assert_eq!(
        game.zone_of(ping),
        None,
        "physical original leaves the game"
    );
    assert_eq!(game.stack.len(), 1, "only the survivor copy remains");
    assert_eq!(game.stack[0].card, copy);
    assert_eq!(game.stack[0].controller, copy_controller);

    pass_once(&mut game);
    pass_once(&mut game);
    eprintln!(
        "survivor virtual-copy owner-departure trace: target_life={}; stack={:?}; events={:?}",
        game.player(target).expect("target exists").life,
        game.stack,
        game.canonical_event_log(),
    );
    assert_eq!(game.player(target).expect("target exists").life, 19);
    assert!(game.event_log.iter().any(|event| matches!(
        event,
        GameEvent::SpellCopyResolved {
            copy: resolved_copy,
            original,
        } if *resolved_copy == copy && *original == ping
    )));
    game.validate_invariants()
        .expect("survivor virtual copy resolves after original owner departure");
}
