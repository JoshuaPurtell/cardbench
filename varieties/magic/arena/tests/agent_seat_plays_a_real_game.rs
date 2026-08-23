//! The test the Pokémon `ReAct` lane never had.
//!
//! There, the Rust seat shipped a provider that returns `Err("LLM provider not
//! implemented")` and the Python agent played a Python-side mock of the game;
//! the two halves were never joined, and nothing in CI would have noticed. So
//! the first thing this crate proves is that an agent seat plays a whole real
//! match against the real engine, with no network and no key.
//!
//! The provider is scripted rather than live for exactly that reason: a seat
//! that can only be exercised against a paid endpoint is a seat nobody runs.

use std::sync::{Arc, Mutex};

use cardbench_magic_arena::provider::{LlmProvider, ProviderError, ScriptedProvider};
use cardbench_magic_arena::react::{ReactConfig, ReactPolicy, ReactStats};
use cardbench_magic_engine::PlayerId;
use cardbench_magic_policies::{
    Archetype, CodePolicy, DeckMatchConfig, DeckMatchTermination, PolicyVersion,
    run_deck_matchup_with, seat_policy, shared_card_index,
};

const DECK: &str = "rav_boros_aggro";

fn seat(
    provider: Arc<dyn LlmProvider>,
    budget: u32,
    stats: &Arc<Mutex<ReactStats>>,
) -> Box<dyn CodePolicy> {
    let index = shared_card_index();
    let fallback = seat_policy(
        PolicyVersion::latest(),
        PlayerId(0),
        Archetype::Aggro,
        index.clone(),
    );
    Box::new(ReactPolicy::new(
        PlayerId(0),
        index,
        provider,
        fallback,
        ReactConfig {
            budget,
            record: false,
        },
        stats.clone(),
        Arc::new(Mutex::new(Vec::new())),
    ))
}

fn play(agent: Box<dyn CodePolicy>) -> cardbench_magic_policies::DeckMatchResult {
    let index = shared_card_index();
    let opponent = seat_policy(
        PolicyVersion::latest(),
        PlayerId(1),
        Archetype::Aggro,
        index,
    );
    run_deck_matchup_with(
        DeckMatchConfig {
            shuffle_seed: 11,
            ..DeckMatchConfig::default()
        },
        DECK,
        DECK,
        [agent, opponent],
    )
    .expect("the match runner should seat an agent policy like any other")
}

#[test]
fn an_agent_seat_plays_a_whole_match_without_a_single_engine_refusal() {
    let stats = Arc::new(Mutex::new(ReactStats::default()));
    let result = play(seat(
        Arc::new(ScriptedProvider::always_first()),
        10_000,
        &stats,
    ));

    // The point of enumerating only legal candidates: an agent seat must never
    // land in the `rejected_policy_moves` contamination path, because a game
    // that ended by an engine refusal is not evidence about anything.
    assert_eq!(
        result.attempted_policy_moves, result.accepted_policy_moves,
        "the engine refused a move the menu offered"
    );
    assert!(
        result.engine_findings.is_empty(),
        "engine findings: {:?}",
        result.engine_findings
    );
    assert!(
        matches!(
            result.termination,
            DeckMatchTermination::Winner(_) | DeckMatchTermination::Draw
        ),
        "the game did not end by play: {:?}",
        result.termination
    );

    let stats = *stats.lock().unwrap();
    assert!(stats.decisions > 0, "the seat was never consulted at all");
    assert!(
        stats.consulted > 0,
        "no decision was open, so nothing was measured"
    );
    assert_eq!(
        stats.provider_failures, 0,
        "a scripted provider cannot fail"
    );
    assert_eq!(
        stats.parse_failures, 0,
        "`{{\"choice\": 0}}` is always parseable"
    );
}

/// A provider that always errors, standing in for a dead endpoint or an
/// exhausted quota.
struct DeadProvider;

impl LlmProvider for DeadProvider {
    fn id(&self) -> String {
        "dead".to_owned()
    }

    fn complete(&self, _system: &str, _user: &str) -> Result<String, ProviderError> {
        Err(ProviderError("endpoint unreachable".to_owned()))
    }
}

#[test]
fn a_dead_provider_degrades_to_the_fallback_and_says_so() {
    // The failure mode worth guarding is silent: the seat keeps playing, the
    // game finishes, the win rate looks like a model result, and it is
    // entirely the fallback policy. It must finish, and it must be visible.
    let stats = Arc::new(Mutex::new(ReactStats::default()));
    let result = play(seat(Arc::new(DeadProvider), 10_000, &stats));

    assert!(
        matches!(
            result.termination,
            DeckMatchTermination::Winner(_) | DeckMatchTermination::Draw
        ),
        "a dead provider must not strand the match: {:?}",
        result.termination
    );

    let stats = *stats.lock().unwrap();
    assert!(stats.provider_failures > 0, "the failures went unrecorded");
    assert_eq!(
        stats.fallbacks, stats.open,
        "every open decision should have fallen back"
    );
    assert!(
        stats.agency() < f64::EPSILON,
        "agency was {} on a run the model never contributed to",
        stats.agency()
    );
}

#[test]
fn an_exhausted_budget_is_counted_rather_than_hidden() {
    let stats = Arc::new(Mutex::new(ReactStats::default()));
    // One consultation, then the seat is on its own for the rest of the game.
    play(seat(Arc::new(ScriptedProvider::always_first()), 1, &stats));

    let stats = *stats.lock().unwrap();
    assert_eq!(stats.consulted, 1, "the budget was not enforced");
    assert!(
        stats.budget_exhausted > 0,
        "a game this long must have hit the budget"
    );
    assert!(
        stats.agency() < 0.5,
        "a one-call budget cannot yield majority agency, got {}",
        stats.agency()
    );
}

#[test]
fn unreadable_replies_fall_back_without_stranding_the_match() {
    struct Babbling;
    impl LlmProvider for Babbling {
        fn id(&self) -> String {
            "babbling".to_owned()
        }
        fn complete(&self, _system: &str, _user: &str) -> Result<String, ProviderError> {
            Ok("I think the best play here is to attack with everything.".to_owned())
        }
    }

    let stats = Arc::new(Mutex::new(ReactStats::default()));
    let result = play(seat(Arc::new(Babbling), 10_000, &stats));

    assert!(
        matches!(
            result.termination,
            DeckMatchTermination::Winner(_) | DeckMatchTermination::Draw
        ),
        "prose replies must not strand the match: {:?}",
        result.termination
    );
    let stats = *stats.lock().unwrap();
    assert!(
        stats.parse_failures > 0,
        "prose was accepted as a choice, which means the parser read a number \
         out of the reasoning"
    );
    assert_eq!(result.attempted_policy_moves, result.accepted_policy_moves);
}
