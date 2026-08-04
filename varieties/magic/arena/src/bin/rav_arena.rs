//! The arena CLI.
//!
//! ```text
//! rav-arena probe [GAMES]                        how many decisions are decisions
//! rav-arena code-vs-code   [PAIRS] A B           two policy generations
//! rav-arena react-vs-code  [PAIRS] [--model M]   a model against a policy
//! rav-arena react-vs-react [PAIRS] [--model M] [--model-b M2]
//! ```
//!
//! Output is `key=value` lines in the same shape as `rav-policy-ladder`, so the
//! same eyes and the same greps work on both.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use cardbench_magic_arena::config::{ArenaConfig, DEFAULT_PATH};
use cardbench_magic_arena::probe::{ProbePolicy, ProbeStats};
use cardbench_magic_arena::provider::{HttpProvider, LlmProvider};
use cardbench_magic_arena::react::{ReactConfig, ReactStats};
use cardbench_magic_arena::runner::{self, ArenaResult, SeatSpec};
use cardbench_magic_engine::PlayerId;
use cardbench_magic_policies::{
    Archetype, CodePolicy, DeckMatchConfig, PolicyVersion, run_deck_matchup_with, seat_policy,
    shared_card_index,
};
use cardbench_magic_rav::load_constructed_decks;

fn percent(value: f64) -> String {
    format!("{:.1}", value * 100.0)
}

fn flag(name: &str) -> Option<String> {
    let arguments: Vec<String> = std::env::args().collect();
    let position = arguments.iter().position(|argument| argument == name)?;
    arguments.get(position + 1).cloned()
}

/// The nth argument that is neither a flag nor a flag's value.
///
/// Skipping only the flags themselves would let `--model x` donate `x` to the
/// positional list, where it would be read as a policy version.
fn positional(index: usize) -> Option<String> {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let mut positionals = Vec::new();
    let mut skip_next = false;
    for argument in arguments {
        if skip_next {
            skip_next = false;
            continue;
        }
        if argument.starts_with("--") {
            skip_next = !matches!(argument.as_str(), "--verbose");
            continue;
        }
        positionals.push(argument);
    }
    positionals.into_iter().nth(index)
}

fn decks() -> Result<Vec<(String, Archetype)>, String> {
    let decks = load_constructed_decks().map_err(|error| error.to_string())?;
    Ok(decks
        .into_iter()
        .filter_map(|deck| Archetype::parse(&deck.archetype).map(|archetype| (deck.id, archetype)))
        .collect())
}

fn version(name: &str) -> PolicyVersion {
    PolicyVersion::parse(name).unwrap_or_else(|| {
        let known: Vec<&str> = PolicyVersion::ALL.iter().map(|entry| entry.id()).collect();
        eprintln!(
            "unknown policy version `{name}`; known: {}",
            known.join(" ")
        );
        std::process::exit(2);
    })
}

fn settings() -> ArenaConfig {
    let path = flag("--config").map_or_else(|| PathBuf::from(DEFAULT_PATH), PathBuf::from);
    ArenaConfig::load(&path).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(2);
    })
}

/// Builds the provider, letting the command line override the file.
///
/// These three travel together and getting the combination wrong fails
/// silently, so they are flags rather than config-only: a reasoning model whose
/// `max_tokens` is below its own reasoning length returns an empty message, and
/// one whose latency exceeds `timeout` returns nothing at all. Both end as a
/// seat that quietly plays its fallback policy.
fn provider(model: Option<String>, config: &ArenaConfig) -> Arc<dyn LlmProvider> {
    let mut http = config.http.clone();
    if let Some(model) = model {
        http.model = model;
    }
    if let Some(effort) = flag("--reasoning-effort") {
        http.reasoning_effort = (effort != "none").then_some(effort);
    }
    if let Some(tokens) = flag("--max-tokens").and_then(|value| value.parse().ok()) {
        http.max_tokens = tokens;
    }
    if let Some(seconds) = flag("--timeout").and_then(|value| value.parse().ok()) {
        http.timeout = seconds;
    }
    match HttpProvider::new(http) {
        Ok(provider) => Arc::new(provider),
        Err(error) => {
            eprintln!("cannot seat a model: {error}");
            std::process::exit(2);
        }
    }
}

fn main() -> ExitCode {
    let mode = positional(0).unwrap_or_else(|| "probe".to_owned());
    match mode.as_str() {
        "probe" => probe(),
        "code-vs-code" => code_vs_code(),
        "react-vs-code" => react_vs_code(),
        "react-vs-react" => react_vs_react(),
        other => {
            eprintln!("unknown mode `{other}`");
            eprintln!(
                "usage: rav-arena [probe|code-vs-code|react-vs-code|react-vs-react] [PAIRS] ..."
            );
            ExitCode::from(2)
        }
    }
}

/// Plays ordinary code-versus-code games and reports how many decisions would
/// have cost a model call. This is the number the whole design rests on, and it
/// needs no key and no network.
fn probe() -> ExitCode {
    let games: u32 = positional(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(4);
    let decks = match decks() {
        Ok(decks) => decks,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let index = shared_card_index();
    let stats = Arc::new(Mutex::new(ProbeStats::default()));

    println!("schema_version=cardbench.magic.arena-probe.v1");
    let mut both_seats = 0_u32;
    for (deck, archetype) in &decks {
        for seed in 0..u64::from(games) {
            let pilots: [Box<dyn CodePolicy>; 2] = [
                Box::new(ProbePolicy::new(
                    seat_policy(
                        PolicyVersion::latest(),
                        PlayerId(0),
                        *archetype,
                        index.clone(),
                    ),
                    index.clone(),
                    stats.clone(),
                )),
                seat_policy(
                    PolicyVersion::latest(),
                    PlayerId(1),
                    *archetype,
                    index.clone(),
                ),
            ];
            let result = match run_deck_matchup_with(
                DeckMatchConfig {
                    shuffle_seed: seed,
                    ..DeckMatchConfig::default()
                },
                deck,
                deck,
                pilots,
            ) {
                Ok(result) => result,
                Err(error) => {
                    eprintln!("probe failed: {error}");
                    return ExitCode::FAILURE;
                }
            };
            both_seats += result.attempted_policy_moves;
            println!(
                "game deck={deck} seed={seed} turns={} policy_moves={}",
                result.turns, result.attempted_policy_moves
            );
        }
    }

    let stats = *stats.lock().expect("probe stats");
    let played = games * u32::try_from(decks.len()).unwrap_or(1);
    println!("\nseat_decisions={}", stats.decisions);
    println!("both_seat_policy_moves={both_seats}");
    println!("open_decisions={}", stats.open);
    println!("open_share={}%", percent(stats.open_share()));
    println!(
        "open_priority={} open_attacks={} open_blocks={}",
        stats.open_priority, stats.open_attacks, stats.open_blocks
    );
    println!("widest_menu={}", stats.widest_menu);
    println!(
        "model_calls_per_game_estimate={:.1}",
        if played == 0 {
            0.0
        } else {
            f64::from(stats.open) / f64::from(played)
        }
    );
    ExitCode::SUCCESS
}

fn code_vs_code() -> ExitCode {
    let config = settings();
    let pairs: u32 = positional(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(config.pairs);
    let seat_a = SeatSpec::Code(version(
        &positional(2).unwrap_or_else(|| PolicyVersion::latest().id().to_owned()),
    ));
    let seat_b = SeatSpec::Code(version(&positional(3).unwrap_or_else(|| "v5".to_owned())));
    play(&seat_a, &seat_b, pairs)
}

fn react_vs_code() -> ExitCode {
    let config = settings();
    let pairs: u32 = positional(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(config.pairs);
    let fallback = version(&flag("--fallback").unwrap_or_else(|| "v7".to_owned()));
    let seat_a = SeatSpec::React {
        provider: provider(flag("--model"), &config),
        fallback,
        config: ReactConfig {
            budget: config.budget,
            record: config.record,
        },
    };
    let seat_b = SeatSpec::Code(version(&flag("--code").unwrap_or_else(|| "v7".to_owned())));
    play(&seat_a, &seat_b, pairs)
}

fn react_vs_react() -> ExitCode {
    let config = settings();
    let pairs: u32 = positional(1)
        .and_then(|value| value.parse().ok())
        .unwrap_or(config.pairs);
    let fallback = version(&flag("--fallback").unwrap_or_else(|| "v7".to_owned()));
    let react = |model: Option<String>| SeatSpec::React {
        provider: provider(model, &config),
        fallback,
        config: ReactConfig {
            budget: config.budget,
            record: config.record,
        },
    };
    let seat_a = react(flag("--model"));
    // Defaulting seat B to the same model makes this a mirror, which is the
    // calibration case: a model against itself must land at 50%.
    let seat_b = react(flag("--model-b").or_else(|| flag("--model")));
    play(&seat_a, &seat_b, pairs)
}

fn play(seat_a: &SeatSpec, seat_b: &SeatSpec, pairs: u32) -> ExitCode {
    let decks = match decks() {
        Ok(decks) => decks,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    println!("schema_version=cardbench.magic.arena.v1");
    // Printed from the same precedence the provider uses, so a result line can
    // never disagree with the settings that produced it.
    let effective = settings().http;
    println!(
        "provider max_tokens={} reasoning_effort={} timeout={}",
        flag("--max-tokens").unwrap_or_else(|| effective.max_tokens.to_string()),
        flag("--reasoning-effort")
            .or(effective.reasoning_effort)
            .unwrap_or_else(|| "none".to_owned()),
        flag("--timeout").unwrap_or_else(|| effective.timeout.to_string()),
    );
    println!(
        "seat_a={} seat_b={} seed_pairs={pairs} games_per_deck={} deck_count={}",
        seat_a.label(),
        seat_b.label(),
        pairs * 2,
        decks.len()
    );

    let verbose = std::env::args().any(|argument| argument == "--verbose");
    // Games are latency-bound against a model seat, so the default is well
    // above the core count. Against two code seats it is pure CPU and the
    // default costs nothing but a few idle threads.
    let jobs: usize = flag("--jobs")
        .and_then(|value| value.parse().ok())
        .unwrap_or(8);
    println!("jobs={jobs}");
    let result = runner::run(
        seat_a,
        seat_b,
        &decks,
        pairs,
        &DeckMatchConfig::default(),
        jobs,
        |line| {
            if verbose {
                println!("progress {line}");
            }
        },
    );
    let result = match result {
        Ok(result) => result,
        Err(error) => {
            eprintln!("arena run failed: {error}");
            return ExitCode::FAILURE;
        }
    };
    report(&result, seat_a.is_react(), seat_b.is_react());
    if result.is_valid() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn report(result: &ArenaResult, a_is_react: bool, b_is_react: bool) {
    for deck in &result.per_deck {
        println!(
            "cell deck={} archetype={} rate={}% ci=[{},{}] n={} mean_turns={:.1} rejected={} truncated={}",
            deck.deck,
            deck.archetype,
            percent(deck.win_rate.point),
            percent(deck.win_rate.low),
            percent(deck.win_rate.high),
            deck.win_rate.samples,
            deck.mean_turns,
            deck.rejected_moves,
            deck.truncated,
        );
        // The paired rate cancels anything seat-dependent exactly. Reporting
        // the halves is the ladder's hard-won lesson and applies unchanged to
        // a model seat.
        println!(
            "  seats deck={} on_play={}% on_draw={}%",
            deck.deck,
            percent(deck.win_rate_on_the_play.point),
            percent(deck.win_rate_on_the_draw.point),
        );
    }
    println!(
        "overall seat_a={} rate={}% ci=[{},{}] n={} a_stronger={}",
        result.seat_a,
        percent(result.overall.point),
        percent(result.overall.low),
        percent(result.overall.high),
        result.overall.samples,
        result.a_is_stronger(),
    );
    if a_is_react {
        print_seat_stats("a", &result.stats_a);
    }
    if b_is_react {
        print_seat_stats("b", &result.stats_b);
    }
    println!("valid={}", result.is_valid());
}

/// A model seat's own numbers, which decide whether the rate above is about the
/// model at all.
fn print_seat_stats(seat: &str, stats: &ReactStats) {
    println!(
        "seat_{seat} decisions={} open={} consulted={} plan_steps={} delegated={}",
        stats.decisions, stats.open, stats.consulted, stats.plan_steps, stats.delegated
    );
    println!(
        "seat_{seat} fallbacks={} parse_failures={} provider_failures={} budget_exhausted={} agency={}%",
        stats.fallbacks,
        stats.parse_failures,
        stats.provider_failures,
        stats.budget_exhausted,
        percent(stats.agency()),
    );
}
