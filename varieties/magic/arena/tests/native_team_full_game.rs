//! Native full-game regression. Shown decks/seeds are not held-out authority.
use cardbench_magic_engine::{LondonPregame, MulliganChoice, PlayerId};
use cardbench_magic_policies::{Archetype, PolicyVersion, seat_policy, shared_card_index};
use cardbench_magic_rav::{load_constructed_decks, new_rav_game};

#[test]
fn legal_two_headed_giant_decks_reach_a_native_terminal_outcome() {
    let decks = load_constructed_decks().unwrap();
    let ids = ["rav_boros_aggro", "rav_boros_aggro", "rav_golgari_midrange", "rav_golgari_midrange"];
    for seed in [73, 192] {
        let mut game = new_rav_game(4).unwrap();
        game.configure_teams(&[vec![PlayerId(0), PlayerId(2)], vec![PlayerId(1), PlayerId(3)]], 30).unwrap();
        game.set_shuffle_seed(seed).unwrap();
        for (seat, id) in ids.iter().enumerate() {
            let deck = decks.iter().find(|deck| deck.id == *id).unwrap();
            game.load_deck_into_library(PlayerId(seat), &deck.deck).unwrap();
        }
        let mut setup = LondonPregame::for_two_headed_giant(game, PlayerId(0)).unwrap();
        for seat in [0, 2, 1, 3] { setup.choose(PlayerId(seat), 0, MulliganChoice::Keep).unwrap(); }
        let mut game = setup.finish().unwrap();
        let mut policies = (0..4).map(|seat| seat_policy(
            PolicyVersion::V8, PlayerId(seat), if seat < 2 { Archetype::Aggro } else { Archetype::Midrange },
            shared_card_index(),
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
                else { policy.propose_move(&view) };
            let kind = action.kind();
            game.submit_policy_move(player, policy.id(), action)
                .unwrap_or_else(|error| panic!("seed {seed}, turn {}, seat {}, {kind:?}: {error}", game.turn, player.0));
            game.validate_invariants().unwrap();
        }
        assert!(game.is_game_over(), "seed {seed}: development limit is not a completed game");
    }
}
