//! Turning a model's reply into a menu index.
//!
//! This is deliberately the smallest parser that can work. Pokémon's
//! `action_parser.rs` is 688 lines because it reconstructs a whole typed action
//! from prose; here the model returns one integer, so the only failure modes
//! are "no integer" and "integer out of range". Both are reported rather than
//! repaired: a seat that quietly substitutes its own move when the model is
//! unreadable is measuring itself.
//!
//! The tolerated shapes are the ones models actually emit -- a bare object, an
//! object inside a fenced code block, an object after a paragraph of prose --
//! and then, as a last resort, a bare integer. Anything looser risks reading
//! the *reasoning* for a number and playing it.

/// What the model said.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub index: usize,
    pub reasoning: Option<String>,
}

/// Why a reply could not be used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// Nothing in the reply looked like a choice.
    NoChoice,
    /// A choice was found and names an option that does not exist.
    OutOfRange { index: usize, options: usize },
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoChoice => write!(formatter, "no choice found in reply"),
            Self::OutOfRange { index, options } => {
                write!(formatter, "choice {index} but only {options} options")
            }
        }
    }
}

impl std::error::Error for ParseError {}

/// Reads a choice out of a model reply and checks it against the menu size.
///
/// # Errors
///
/// Returns [`ParseError::NoChoice`] when no index can be read, and
/// [`ParseError::OutOfRange`] when the index names no option.
pub fn choice(reply: &str, options: usize) -> Result<Choice, ParseError> {
    let parsed = from_json(reply).or_else(|| from_bare_integer(reply));
    let Some(parsed) = parsed else {
        return Err(ParseError::NoChoice);
    };
    if parsed.index >= options {
        return Err(ParseError::OutOfRange {
            index: parsed.index,
            options,
        });
    }
    Ok(parsed)
}

/// Finds the outermost balanced `{...}` span and reads it as JSON.
///
/// Scanning for a balanced span rather than the first `{` and last `}` is what
/// lets a reply survive a model that narrates around its JSON, or wraps it in a
/// fenced block, without a second parser for each case.
fn from_json(reply: &str) -> Option<Choice> {
    let bytes = reply.as_bytes();
    let start = reply.find('{')?;
    let mut depth = 0_usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, byte) in bytes.iter().enumerate().skip(start) {
        if in_string {
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    let span = &reply[start..=offset];
                    return read_object(span);
                }
            }
            _ => {}
        }
    }
    None
}

fn read_object(span: &str) -> Option<Choice> {
    let value: serde_json::Value = serde_json::from_str(span).ok()?;
    let index = value.get("choice").and_then(|choice| match choice {
        serde_json::Value::Number(number) => number.as_u64(),
        // A model that quotes its integer is still unambiguous.
        serde_json::Value::String(text) => text.trim().parse::<u64>().ok(),
        _ => None,
    })?;
    let reasoning = value
        .get("reasoning")
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned);
    Some(Choice {
        index: usize::try_from(index).ok()?,
        reasoning,
    })
}

/// A reply that is nothing but a number.
///
/// Restricted to a reply whose entire trimmed content is one integer. Reading
/// the first integer out of arbitrary prose would happily play the `3` in
/// "3 damage".
fn from_bare_integer(reply: &str) -> Option<Choice> {
    let index = reply.trim().parse::<usize>().ok()?;
    Some(Choice {
        index,
        reasoning: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_object_is_read() {
        let parsed = choice(r#"{"reasoning": "trade up", "choice": 2}"#, 5).unwrap();
        assert_eq!(parsed.index, 2);
        assert_eq!(parsed.reasoning.as_deref(), Some("trade up"));
    }

    #[test]
    fn narration_around_the_object_is_tolerated() {
        let reply =
            "Let me think about the board.\n\n```json\n{\"choice\": 1}\n```\nThat is my play.";
        assert_eq!(choice(reply, 4).unwrap().index, 1);
    }

    #[test]
    fn a_brace_inside_reasoning_does_not_end_the_object() {
        // A naive first-brace/last-brace scan reads this span as invalid JSON
        // and falls through to the bare-integer path, which then finds nothing.
        let reply = r#"{"reasoning": "the text said {this}", "choice": 3}"#;
        assert_eq!(choice(reply, 5).unwrap().index, 3);
    }

    #[test]
    fn a_quoted_integer_is_still_a_choice() {
        assert_eq!(choice(r#"{"choice": "4"}"#, 6).unwrap().index, 4);
    }

    #[test]
    fn a_bare_integer_is_accepted_only_when_it_is_the_whole_reply() {
        assert_eq!(choice("2", 5).unwrap().index, 2);
        // Otherwise "deals 3 damage" would be read as the choice 3.
        assert_eq!(
            choice("I would take the trade, it deals 3 damage", 5),
            Err(ParseError::NoChoice)
        );
    }

    #[test]
    fn an_index_past_the_menu_is_refused_rather_than_clamped() {
        // Clamping would silently play option 4 when the model asked for 9,
        // and the transcript would record a move nobody chose.
        assert_eq!(
            choice(r#"{"choice": 9}"#, 5),
            Err(ParseError::OutOfRange {
                index: 9,
                options: 5
            })
        );
    }

    #[test]
    fn an_empty_reply_is_a_parse_failure_not_a_pass() {
        assert_eq!(choice("", 3), Err(ParseError::NoChoice));
    }
}
