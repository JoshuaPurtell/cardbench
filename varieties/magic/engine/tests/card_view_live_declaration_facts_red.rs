//! The public card view must expose enough live state to declare a legal block
//! and to know whether a creature can pay a tap cost.
//!
//! `CardView` already exposes `can_attack`, a live derived flag. It exposes no
//! equivalent for blocking and no summoning-sickness fact, and it does not
//! expose live keywords at all -- only the card's printed definition id, from
//! which a policy can recover printed keywords but never granted or removed
//! ones.
//!
//! The consequence is that a policy cannot avoid submitting illegal
//! declarations. A creature under an aura that grants `CannotAttackOrBlock`
//! still looks like a legal blocker, and a creature that entered this turn
//! still looks like a legal mana source. Both are rejected by the engine, and
//! a rejected move is indistinguishable from a policy bug in a campaign.
//!
//! Permanent characteristics are public information, so exposing them crosses
//! no viewer boundary.

use std::collections::BTreeSet;

use cardbench_magic_engine::{
    CardDefinition, CardType, CastRequest, ContinuousChange, Effect, Game, Keyword, ManaCost,
    PlayerId, PolicyAction, Step, Target, Zone,
};

const BEAR: &str = "VIEW-FACTS-BEAR";
const SHACKLE: &str = "VIEW-FACTS-SHACKLE";

fn bear() -> CardDefinition {
    CardDefinition {
        id: BEAR,
        name: BEAR,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]),
        is_basic_land: false,
        supported_rules: &["card-view-live-declaration-facts-probe"],
        power: Some(2),
        toughness: Some(2),
        keywords: vec![],
        effects: vec![],
    }
}

fn shackle() -> CardDefinition {
    CardDefinition {
        id: SHACKLE,
        name: SHACKLE,
        set_code: "TST",
        mana_cost: ManaCost::new(0),
        colors: BTreeSet::new(),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Enchantment]),
        is_basic_land: false,
        supported_rules: &["card-view-live-declaration-facts-probe"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::AttachSourceToTarget {
            target: cardbench_magic_engine::TargetRequirement::Creature,
            changes: vec![ContinuousChange::AddKeyword(Keyword::CannotAttackOrBlock)],
        }],
    }
}

/// A creature whose live keywords differ from its printed ones must report the
/// live set, or a policy cannot tell that it may not block.
#[test]
fn the_card_view_reports_live_keywords_rather_than_printed_ones() {
    let owner = PlayerId(0);
    let opponent = PlayerId(1);
    let mut game = Game::new([bear(), shackle()], 2).expect("fixture initializes");

    let creature = game
        .add_card(owner, BEAR, Zone::Battlefield)
        .expect("creature enters");
    let aura = game
        .add_card(owner, SHACKLE, Zone::Hand)
        .expect("aura enters hand");
    for seat in [owner, opponent] {
        for _ in 0..6 {
            game.add_card(seat, BEAR, Zone::Library)
                .expect("library filler enters");
        }
    }
    game.begin_game().expect("game begins");

    let before = view_card(&game, owner, creature);
    assert!(
        before.keywords.is_empty(),
        "a printed-vanilla creature reports no keywords"
    );

    advance_to_main(&mut game);
    game.submit_policy_move(
        owner,
        "fixture",
        PolicyAction::Cast(CastRequest {
            card: aura,
            targets: vec![Target::Permanent(creature)],
            convoke: vec![],
            payment_mana_abilities: vec![],
        }),
    )
    .expect("the aura is cast");
    resolve_stack(&mut game);

    let after = view_card(&game, owner, creature);
    assert!(
        after.keywords.contains(&Keyword::CannotAttackOrBlock),
        "a granted keyword must appear in the live view: {:?}",
        after.keywords
    );
    assert!(
        !after.can_block,
        "a creature that cannot attack or block must not report itself blockable"
    );
}

/// Summoning sickness gates tap costs on mana abilities, and a policy that
/// cannot see it plans an illegal activation.
#[test]
fn the_card_view_reports_summoning_sickness() {
    let owner = PlayerId(0);
    let mut game = Game::new([bear(), shackle()], 2).expect("fixture initializes");
    let established = game
        .add_card(owner, BEAR, Zone::Battlefield)
        .expect("creature enters");
    game.begin_game().expect("game begins");

    assert!(
        !view_card(&game, owner, established).summoning_sick,
        "a creature that started the game on the battlefield is not sick"
    );

    let fresh = game
        .add_card(owner, BEAR, Zone::Battlefield)
        .expect("a creature enters this turn");
    assert!(
        view_card(&game, owner, fresh).summoning_sick,
        "a creature that entered this turn is summoning sick"
    );
}

/// Advances to the active player's precombat main phase.
fn advance_to_main(game: &mut Game) {
    for _ in 0..32 {
        if game.step == Step::PrecombatMain {
            return;
        }
        step_once(game);
    }
}

fn resolve_stack(game: &mut Game) {
    for _ in 0..24 {
        if game.stack.is_empty() || game.is_game_over() {
            return;
        }
        step_once(game);
    }
}

fn step_once(game: &mut Game) {
    let actor = game.next_policy_player();
    let pending_draw = game
        .view_for_player(actor)
        .ok()
        .and_then(|view| view.draw_replacement_decision);
    let action = pending_draw.map_or(PolicyAction::PassPriority, |decision| PolicyAction::Draw {
        decision,
        dredge: None,
    });
    let _ = game.submit_policy_move(actor, "fixture", action);
}

fn view_card(
    game: &Game,
    viewer: PlayerId,
    card: cardbench_magic_engine::ObjectId,
) -> cardbench_magic_engine::CardView {
    game.view_for_player(viewer)
        .expect("the viewer has a view")
        .own_battlefield
        .into_iter()
        .chain(
            game.view_for_player(viewer)
                .expect("the viewer has a view")
                .opponent_battlefield,
        )
        .find(|entry| entry.id == card)
        .expect("the card is on a projected battlefield")
}
