//! Scores a compiled-in `(deck, pilot)` candidate against a
//! `cardbench/magic/code_policy` split.
//!
//! The command itself lives in
//! [`cardbench_magic_policies::code_policy::cli`] so that this binary and the
//! generated binary that grades a *submitted* policy cannot drift apart in how
//! they parse arguments, resolve the surface, refuse an unsealed held-out split
//! or format the report. Run with `--help` for usage.
//!
//! `None` here means "no submission": the candidate is whichever generation
//! `--candidate-pilot` names, which is what comparing generations to each other
//! needs and is not how an agent's policy is graded.

fn main() {
    cardbench_magic_policies::code_policy::cli::run(None);
}
