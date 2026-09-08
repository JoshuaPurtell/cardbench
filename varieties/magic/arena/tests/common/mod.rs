//! Full native game test driver. No legality fallback and no fixture mutations.
use cardbench_magic_engine::{Game, PlayerId, PolicyAction, Step};
use cardbench_magic_policies::{Archetype, PolicyVersion, seat_policy, shared_card_index};

pub fn finish_native_game(mut game: Game, archetypes: &[Archetype]) -> Game {
    let mut policies = archetypes.iter().enumerate().map(|(seat, archetype)| seat_policy(
        PolicyVersion::V8, PlayerId(seat), *archetype, shared_card_index(),
    )).collect::<Vec<_>>();
    for _ in 0..10_000 {
        if game.is_game_over() { break; }
        let player = game.next_policy_player();
        let view = game.view_for_player(player).unwrap();
        let policy = &mut policies[player.0];
        let action = if let Some(action) = policy.propose_pending_decision(&view) { action }
            else if view.optional_triggered_ability_choice.is_some() { policy.propose_optional_triggered_ability(&view) }
            else if view.draw_replacement_pending { policy.propose_draw_replacement(&view) }
            else if view.private_library_choice.is_some() { policy.propose_private_library_choice(&view) }
            else if view.private_opponent_library_choice.is_some() { policy.propose_private_opponent_library_choice(&view) }
            else if view.library_search_choice.is_some() { policy.propose_library_search_choice(&view) }
            else if view.step == Step::DeclareAttackers && !view.attackers_declared {
                // Explicit development pilot: exercise combat, rather than
                // measuring a four-seat policy's strategic conservatism.
                PolicyAction::DeclareAttackers { attackers: view.own_battlefield.iter()
                    .filter(|card| card.can_attack).map(|card| card.id).collect() }
            } else if view.step == Step::DeclareBlockers && !view.blockers_declared {
                // The same deliberately weak smoke pilot declines blocks.
                // Competitive blocking is covered by the separate 2HG games.
                PolicyAction::DeclareBlockers { assignments: Vec::new() }
            } else { policy.propose_move(&view) };
        let kind = action.kind();
        game.submit_policy_move(player, policy.id(), action)
            .unwrap_or_else(|error| panic!("turn {}, seat {}, {kind:?}: {error}", game.turn, player.0));
        game.validate_invariants().unwrap();
    }
    assert!(game.is_game_over(), "development limit is not a completed game");
    game
}
