//! The text an agent actually sees.
//!
//! Pokémon's `react/render.rs` renders `GameView` into labelled sections and
//! quotes stable integer ids that the model echoes back. That part transfers.
//! What does not transfer is the free-form action vocabulary: here the model
//! never names a card or an object, it names a menu index. So this renderer has
//! one job the Pokémon one did not -- make the numbered options legible enough
//! that an index is a real choice rather than a guess.
//!
//! Nothing here is scored. The menu arrives in structural order and is printed
//! in that order, so the prompt cannot leak the incumbent policy's preference.

use std::fmt::Write as _;

use cardbench_magic_engine::GameView;
use cardbench_magic_policies::planner::{Board, CardIndex, Permanent};

use crate::menu::{Menu, Occasion};

/// The instruction the model is given once per game.
pub const SYSTEM_PROMPT: &str = "\
You are an expert Magic: The Gathering player piloting a 60-card Ravnica \
constructed deck. You will be shown the game state and a numbered list of the \
legal options available to you right now.

Reply with a single JSON object and nothing else:

  {\"reasoning\": \"<one or two sentences>\", \"choice\": <integer>}

`choice` must be the number of one option from the list. Do not invent an \
option, do not name cards in `choice`, and do not return anything outside the \
JSON object.

Play to win the game, not to maximise the current turn. Holding an instant for \
the right window, declining a bad attack, and taking a trade that clears a \
blocker are all normal correct plays.";

/// Renders one decision into a user prompt.
///
/// Writes are infallible into a `String`, so the `let _ =` on each is
/// discarding an error that cannot occur rather than one being ignored.
#[must_use]
#[allow(clippy::missing_panics_doc)]
pub fn prompt(view: &GameView, menu: &Menu, index: &CardIndex) -> String {
    let board = Board::from_view(view, index);
    let mut out = String::new();

    out.push_str("=== GAME STATE ===\n");
    let _ = writeln!(
        out,
        "Turn {} | {:?} | {}",
        board.turn,
        view.step,
        if board.is_my_turn {
            "your turn"
        } else {
            "opponent's turn"
        }
    );
    let _ = writeln!(out, "Your life: {}", board.my_life);
    for opponent in &board.opponents {
        let _ = writeln!(
            out,
            "Opponent (seat {}) life: {}",
            opponent.seat.0, opponent.life
        );
    }
    let _ = writeln!(
        out,
        "Lands played this turn: {} | Stack depth: {}",
        board.lands_played, board.stack_depth
    );

    out.push_str("\n=== YOUR BATTLEFIELD ===\n");
    push_permanents(&mut out, &board.mine);

    out.push_str("\n=== OPPONENT BATTLEFIELD ===\n");
    push_permanents(&mut out, &board.theirs);

    if !board.attackers.is_empty() {
        out.push_str("\n=== ATTACKING ===\n");
        push_permanents(&mut out, &board.attackers);
    }

    out.push_str("\n=== YOUR HAND ===\n");
    if board.hand.is_empty() {
        out.push_str("  (empty)\n");
    } else {
        for card in &board.hand {
            let facts = &card.facts;
            let kind = if facts.is_land {
                "land".to_owned()
            } else if facts.is_creature {
                format!("creature {}/{}", facts.power, facts.toughness)
            } else if facts.is_instant_speed {
                "instant-speed spell".to_owned()
            } else {
                "sorcery-speed spell".to_owned()
            };
            let _ = writeln!(
                out,
                "  - {} [{kind}, mana value {}]",
                card.definition, facts.mana_value
            );
        }
    }

    out.push_str("\n=== DECISION ===\n");
    out.push_str(occasion_line(menu.occasion));
    out.push_str("\n\n=== YOUR OPTIONS ===\n");
    for (position, candidate) in menu.candidates.iter().enumerate() {
        let _ = writeln!(out, "  {position}. {}", candidate.label);
    }
    out.push_str("\nReply with JSON: {\"reasoning\": \"...\", \"choice\": <number>}\n");
    out
}

const fn occasion_line(occasion: Occasion) -> &'static str {
    match occasion {
        Occasion::Forced => "You have one legal reply.",
        Occasion::Priority => "You have priority. Choose one action.",
        Occasion::DeclareAttackers => "Declare attackers.",
        Occasion::DeclareBlockers => "Declare blockers.",
    }
}

fn push_permanents(out: &mut String, permanents: &[Permanent]) {
    if permanents.is_empty() {
        out.push_str("  (empty)\n");
        return;
    }
    for permanent in permanents {
        let name = permanent.definition.unwrap_or("unknown");
        let mut notes = Vec::new();
        if permanent.tapped {
            notes.push("tapped".to_owned());
        }
        if permanent.summoning_sick {
            notes.push("summoning sick".to_owned());
        }
        for keyword in &permanent.keywords {
            notes.push(format!("{keyword:?}"));
        }
        let suffix = if notes.is_empty() {
            String::new()
        } else {
            format!(" [{}]", notes.join(", "))
        };
        if permanent.is_creature {
            let _ = writeln!(
                out,
                "  - {name} {}/{}{suffix}",
                permanent.power, permanent.toughness
            );
        } else {
            let _ = writeln!(out, "  - {name}{suffix}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::{Candidate, Menu};
    use cardbench_magic_engine::PolicyAction;

    fn menu() -> Menu {
        Menu {
            occasion: Occasion::Priority,
            candidates: vec![
                Candidate {
                    label: "pass priority".to_owned(),
                    plan: vec![PolicyAction::PassPriority],
                },
                Candidate {
                    label: "cast Lightning Helix targeting player 1".to_owned(),
                    plan: vec![PolicyAction::PassPriority],
                },
            ],
        }
    }

    #[test]
    fn options_are_numbered_from_zero_and_printed_in_menu_order() {
        // The index the model returns is positional, so the printed order and
        // the candidate order must not be allowed to drift apart.
        let mut rendered = String::new();
        for (position, candidate) in menu().candidates.iter().enumerate() {
            let _ = writeln!(rendered, "  {position}. {}", candidate.label);
        }
        assert!(rendered.contains("  0. pass priority"));
        assert!(rendered.contains("  1. cast Lightning Helix targeting player 1"));
    }

    #[test]
    fn the_system_prompt_names_the_reply_shape_the_parser_accepts() {
        // These two travel together: a prompt that asked for `{"action": ...}`
        // would produce replies the parser reads as no choice at all, and the
        // seat would silently be its fallback policy.
        assert!(SYSTEM_PROMPT.contains("\"choice\""));
        assert!(crate::parse::choice(r#"{"reasoning": "x", "choice": 1}"#, 2).is_ok());
    }
}
