//! Automated critique of a match transcript.
//!
//! Neither variety has this. The Pokémon side renders a board image; Magic had
//! a canonical log and ad-hoc greps. Both leave the reviewer to notice the
//! problem themselves, which does not scale past a handful of games.
//!
//! Every finding here is a *suspicion*, not a verdict. A policy holding four
//! mana unspent on turn six is usually a mistake and occasionally correct; the
//! job of this pass is to put the turn number in front of a human, not to
//! decide. Findings are therefore phrased as observations with evidence
//! attached, and nothing here feeds a score.

use crate::transcript::{MatchTranscript, TurnSummary, timeline};
use serde::{Deserialize, Serialize};

/// How much attention a finding deserves.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub enum Severity {
    /// Worth knowing; often benign.
    Note,
    /// Probably a real loss of value.
    Warning,
    /// Almost certainly wrong, or not a play at all.
    Defect,
}

/// One observation about how a match went.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Finding {
    pub severity: Severity,
    /// Stable kebab-case identifier, so findings can be counted across a
    /// campaign rather than only read one match at a time.
    pub code: String,
    /// The seat this concerns, when it concerns one.
    pub seat: Option<u16>,
    /// The turn it was observed on, when it is turn-local.
    pub turn: Option<u32>,
    pub detail: String,
}

/// The whole critique of one match.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Critique {
    pub findings: Vec<Finding>,
}

impl Critique {
    #[must_use]
    pub fn worst(&self) -> Option<Severity> {
        self.findings.iter().map(|finding| finding.severity).max()
    }

    #[must_use]
    pub fn count(&self, severity: Severity) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.severity == severity)
            .count()
    }
}

/// Runs every check against a transcript.
#[must_use]
pub fn critique(transcript: &MatchTranscript) -> Critique {
    let turns = timeline(transcript);
    let mut findings = Vec::new();
    findings.extend(rejected_moves(transcript));
    findings.extend(non_terminal_stop(transcript));
    findings.extend(idle_turns(transcript, &turns));
    findings.extend(unspent_mana(&turns));
    findings.extend(passive_combat(transcript, &turns));
    findings.extend(never_blocked(transcript));
    findings.sort_by(|left, right| {
        right
            .severity
            .cmp(&left.severity)
            .then(left.turn.cmp(&right.turn))
            .then(left.code.cmp(&right.code))
    });
    Critique { findings }
}

/// An engine-refused proposal is never a play. It is either a policy bug or an
/// engine defect, and either way the game did not end the way it looks.
fn rejected_moves(transcript: &MatchTranscript) -> Vec<Finding> {
    if transcript.manifest.rejected_policy_moves == 0 {
        return Vec::new();
    }
    vec![Finding {
        severity: Severity::Defect,
        code: "engine-refused-proposal".to_owned(),
        seat: None,
        turn: Some(transcript.manifest.turns),
        detail: format!(
            "{} proposal(s) refused; termination was {}. Classify before trusting this game.",
            transcript.manifest.rejected_policy_moves, transcript.manifest.termination
        ),
    }]
}

/// A game stopped by a move or turn bound did not end by the rules, so its
/// result is not a win or a loss.
fn non_terminal_stop(transcript: &MatchTranscript) -> Vec<Finding> {
    let termination = &transcript.manifest.termination;
    if termination.starts_with("Winner") || termination == "Draw" {
        return Vec::new();
    }
    vec![Finding {
        severity: Severity::Defect,
        code: "non-terminal-stop".to_owned(),
        seat: None,
        turn: Some(transcript.manifest.turns),
        detail: format!("game stopped at {termination} rather than by the rules"),
    }]
}

/// A turn where the active seat produced mana and then cast nothing and played
/// no land is usually a hand it could not use, and sometimes a planner that
/// failed to find a line.
fn idle_turns(transcript: &MatchTranscript, turns: &[TurnSummary]) -> Vec<Finding> {
    let mut findings = Vec::new();
    let mut consecutive = 0_u32;
    let mut first_idle = None;
    for turn in turns.iter().filter(|turn| turn.turn > 0) {
        let idle = turn.spells_cast.is_empty()
            && turn.lands_played.is_empty()
            && turn.attackers_declared.iter().all(|(_, count)| *count == 0);
        if idle {
            consecutive += 1;
            first_idle.get_or_insert(turn.turn);
        } else {
            consecutive = 0;
            first_idle = None;
        }
        // A single quiet turn is ordinary. Four in a row is a stalled game.
        if consecutive == 4 {
            findings.push(Finding {
                severity: Severity::Warning,
                code: "stalled-development".to_owned(),
                seat: turn.active_seat,
                turn: first_idle,
                detail: format!(
                    "four consecutive turns from {} with no land, spell, or attack",
                    first_idle.unwrap_or(turn.turn)
                ),
            });
        }
    }
    let _ = transcript;
    findings
}

/// Mana produced and not converted into a spell is the clearest single signal
/// that a planner is leaving value on the table.
fn unspent_mana(turns: &[TurnSummary]) -> Vec<Finding> {
    let mut findings = Vec::new();
    for turn in turns.iter().filter(|turn| turn.turn > 0) {
        let produced: i64 = turn.mana_added.iter().map(|(_, amount)| *amount).sum();
        if produced >= 3 && turn.spells_cast.is_empty() {
            findings.push(Finding {
                severity: Severity::Warning,
                code: "mana-produced-unspent".to_owned(),
                seat: turn.active_seat,
                turn: Some(turn.turn),
                detail: format!("{produced} mana produced, no spell cast"),
            });
        }
    }
    findings
}

/// A seat that developed a board and never once declared an attacker is
/// almost certainly leaving the game unwinnable for itself.
///
/// Checked **per seat**. Summing across both seats hides the case this exists
/// to catch: one aggressive pilot attacking every turn makes a match-wide
/// total nonzero while its opponent never attacks at all.
fn passive_combat(transcript: &MatchTranscript, turns: &[TurnSummary]) -> Vec<Finding> {
    let mut findings = Vec::new();
    let seats: Vec<u16> = vec![0, 1];
    for seat in seats {
        let attacked: i64 = turns
            .iter()
            .flat_map(|turn| turn.attackers_declared.iter())
            .filter(|(who, _)| *who == seat)
            .map(|(_, count)| *count)
            .sum();
        if attacked > 0 {
            continue;
        }
        let spells = transcript
            .of_kind("SpellCast")
            .filter(|event| event.facts.seats.first().map(|s| s.0) == Some(seat))
            .count();
        // A seat that never resolved much cannot be expected to attack.
        if spells < 3 {
            continue;
        }
        findings.push(Finding {
            severity: Severity::Warning,
            code: "seat-never-attacked".to_owned(),
            seat: Some(seat),
            turn: None,
            detail: format!(
                "seat {seat} cast {spells} spells across {} turns and never declared an attacker",
                transcript.manifest.turns
            ),
        });
    }
    findings
}

/// Damage taken with blockers never assigned is the failure mode that made a
/// wall worthless before the valued-blocking generation.
fn never_blocked(transcript: &MatchTranscript) -> Vec<Finding> {
    let blocks: i64 = transcript
        .of_kind("BlockersDeclared")
        .filter_map(|event| event.facts.amount)
        .sum();
    if blocks > 0 {
        return Vec::new();
    }
    let attacks: i64 = transcript
        .of_kind("AttackersDeclared")
        .filter_map(|event| event.facts.amount)
        .sum();
    if attacks < 6 {
        return Vec::new();
    }
    vec![Finding {
        severity: Severity::Note,
        code: "never-blocked".to_owned(),
        seat: None,
        turn: None,
        detail: format!("{attacks} attackers declared across the match and no block assigned"),
    }]
}

/// Renders a critique for a terminal.
#[must_use]
pub fn render(critique: &Critique) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    if critique.findings.is_empty() {
        out.push_str("no findings\n");
        return out;
    }
    for finding in &critique.findings {
        let seat = finding
            .seat
            .map_or_else(|| "-".to_owned(), |seat| format!("s{seat}"));
        let turn = finding
            .turn
            .map_or_else(|| "-".to_owned(), |turn| turn.to_string());
        let _ = writeln!(
            out,
            "{:<8} {:<26} seat={:<3} turn={:<4} {}",
            format!("{:?}", finding.severity).to_lowercase(),
            finding.code,
            seat,
            turn,
            finding.detail
        );
    }
    out
}
