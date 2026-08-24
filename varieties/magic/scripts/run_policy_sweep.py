#!/usr/bin/env python3
"""Grade one submitted Magic code policy against a `code_policy` split.

The Rust sweep in `policies/src/code_policy/sweep.rs` does the scoring: paired
cells, Wilson intervals per opponent, a bootstrap interval on the lift, and a
fail-closed coverage check. This script exists because that sweep has to be
*compiled with the candidate in it* -- a policy someone else wrote cannot be
selected by an already-built binary -- and because Harbor wants the verdict in
its own result shape.

    run_policy_sweep.py --candidate candidate/policy.rs --split heldout \
        --output-root artifacts/policy-sweep

Three outcomes, deliberately distinct:

* **scored** -- the sweep ran. `reward.txt` holds the delta, exit 0 on a pass
  and 1 on a fail. A candidate that does not compile lands here too: that is
  the candidate's fault and a zero is the right answer.
* **harness_failed** -- the sweep could not run at all, the canonical case
  being an absent Rust toolchain. No `reward.txt` is written and the exit code
  is 2, so the platform reports *no* reward rather than banking a zero that
  looks exactly like a candidate losing every cell.
* **coverage failure** -- the sweep ran but did not reproduce the roster's cell
  set. The Rust side already reports this as `scored: false`; it is a zero, and
  the diagnostics say which cells went missing.
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any

MAGIC_ROOT = Path(__file__).resolve().parents[1]
POLICIES_DIR = MAGIC_ROOT / "policies"
ENGINE_DIR = MAGIC_ROOT / "engine"
ROSTER = MAGIC_ROOT / "rosters" / "code_policy_v1.json"

TASK_ID = "cardbench/magic/code_policy"
FAMILY = "cardbench.magic.code_policy"

# The reference plays this deck. A candidate that names no deck is graded on the
# same one, which is the only comparison that isolates the pilot.
DEFAULT_DECK = "rav_selesnya_midrange"


class SweepError(RuntimeError):
    """The candidate's fault. Scored as a zero."""


class HarnessError(RuntimeError):
    """Ours. Never scored -- see the module docstring."""


def require_toolchain() -> None:
    """Fail as a harness error, before anything that would look like a grade.

    The platform runs the verifier under `bash -lc`. A login shell re-sources
    /etc/profile, which can drop cargo off PATH, and the failure then surfaces
    as a compile error -- indistinguishable from a candidate that does not
    build, and scored as a zero the candidate did not earn.
    """

    missing = [tool for tool in ("cargo", "rustc") if shutil.which(tool) is None]
    if missing:
        raise HarnessError(
            f"rust toolchain not on PATH: {', '.join(missing)}. "
            "The verifier runs under `bash -lc`; check that cargo survives a login shell."
        )


def roster_payload() -> dict[str, Any]:
    if not ROSTER.is_file():
        raise HarnessError(f"roster is missing: {ROSTER}")
    return json.loads(ROSTER.read_text(encoding="utf-8"))


def resolve_deck(candidate: Path, requested: str | None, roster: dict[str, Any]) -> str:
    """Pick the candidate's deck, and refuse anything outside the pinned pool.

    The pool is part of the surface the roster fixes. Letting a submission name
    an arbitrary deck fixture would let it pick a matchup rather than play the
    one it was given.
    """

    deck = requested
    if deck is None:
        sidecar = candidate.parent / "deck.txt"
        if sidecar.is_file():
            deck = sidecar.read_text(encoding="utf-8").strip()
    if not deck:
        return DEFAULT_DECK
    pool = [str(entry) for entry in roster.get("candidate_deck_pool") or []]
    if deck not in pool:
        raise SweepError(
            f"deck `{deck}` is not in the roster's candidate pool; choose one of: {' '.join(pool)}"
        )
    return deck


def target_dir() -> Path:
    """One shared cargo target tree for every candidate build.

    A cold build of the engine, the set crate and the policies crate is minutes.
    Sharing the tree makes only the candidate's own crate recompile, which is
    seconds. Kept outside the repo so a `git clean` does not cost that.
    """

    root = Path(os.environ.get("CARDBENCH_CARGO_CACHE") or (Path.home() / ".cache" / "cardbench"))
    path = root / "magic-candidate-target"
    path.mkdir(parents=True, exist_ok=True)
    return path


def write_candidate_crate(candidate: Path, candidate_id: str, work_dir: Path) -> Path:
    """Generate a standalone crate whose binary is the sweep plus this policy."""

    src = work_dir / "src"
    src.mkdir(parents=True, exist_ok=True)

    code = candidate.read_text(encoding="utf-8")
    (src / "candidate.rs").write_text(code, encoding="utf-8")

    # `id` reaches the report label only. It is rendered through a Rust string
    # literal, so it must not be able to close one.
    safe_id = "".join(ch for ch in candidate_id if ch.isalnum() or ch in "._-")[:64] or "candidate"

    (src / "main.rs").write_text(
        "//! Generated by scripts/run_policy_sweep.py. Do not edit.\n"
        "//!\n"
        "//! The whole command lives in the policies crate; this file exists only\n"
        "//! to bind the submitted policy's `build_policy` into it as the\n"
        "//! candidate seat factory.\n"
        "\n"
        "mod candidate;\n"
        "\n"
        "use cardbench_magic_policies::code_policy::cli;\n"
        "\n"
        "fn main() {\n"
        "    let seat = |player, archetype, index| candidate::build_policy(player, archetype, index);\n"
        f"    cli::run(Some(cli::Candidate {{ id: \"{safe_id}\", seat: &seat }}));\n"
        "}\n",
        encoding="utf-8",
    )

    # A detached `[workspace]` on purpose: this crate is generated into a temp
    # directory and must not be adopted by whatever workspace happens to be
    # above it.
    (work_dir / "Cargo.toml").write_text(
        "[package]\n"
        "name = \"cardbench-magic-candidate\"\n"
        "version = \"0.0.0\"\n"
        "edition = \"2024\"\n"
        "\n"
        "[workspace]\n"
        "\n"
        "[dependencies]\n"
        f"cardbench-magic-policies = {{ path = \"{POLICIES_DIR}\" }}\n"
        f"cardbench-magic-engine = {{ path = \"{ENGINE_DIR}\" }}\n"
        "\n"
        "[[bin]]\n"
        "name = \"candidate-sweep\"\n"
        "path = \"src/main.rs\"\n",
        encoding="utf-8",
    )
    return work_dir


def build_candidate(work_dir: Path) -> Path:
    require_toolchain()
    environment = {**os.environ, "CARGO_TARGET_DIR": str(target_dir())}
    # Offline is NOT forced here. The bake's prewarm runs this same function to
    # populate CARGO_HOME, and `--offline` against an empty cache fails to
    # resolve at all. The Dockerfile sets CARGO_NET_OFFLINE=true *after* the
    # prewarm, which cargo honours on its own -- so the graded build is offline
    # and the build that fills the cache is not.
    completed = subprocess.run(
        ["cargo", "build", "--release", "--bin", "candidate-sweep"],
        cwd=work_dir,
        text=True,
        capture_output=True,
        env=environment,
        check=False,
    )
    if completed.returncode != 0:
        combined = (completed.stdout + "\n" + completed.stderr)[-6000:]
        raise SweepError(f"candidate did not compile:\n{combined}")
    binary = target_dir() / "release" / "candidate-sweep"
    if not binary.is_file():
        raise HarnessError(f"cargo reported success but {binary} is absent")
    return binary


def run_sweep(binary: Path, *, deck: str, split: str, out: Path) -> dict[str, Any]:
    report_path = out / "sweep_report.json"
    completed = subprocess.run(
        [
            str(binary),
            "--roster", str(ROSTER),
            "--split", split,
            "--candidate-deck", deck,
            "--json", str(report_path),
        ],
        cwd=MAGIC_ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    (out / "sweep_stdout.txt").write_text(completed.stdout, encoding="utf-8")
    if completed.returncode == 2:
        # Usage. The only caller is this script, so a usage error is a bug here
        # rather than anything the candidate did.
        raise HarnessError(f"sweep rejected its arguments:\n{completed.stderr[-4000:]}")
    if completed.returncode != 0 or not report_path.is_file():
        raise HarnessError(f"sweep could not grade:\n{completed.stderr[-4000:]}")
    return json.loads(report_path.read_text(encoding="utf-8"))


def prewarm(splits: list[str]) -> None:
    """Build the candidate crate once at bake time.

    The graded container runs with no network, so every crate has to be in
    CARGO_HOME already, and a cold build of the engine, the set crate and the
    policies crate does not fit in the verifier's budget. Building the reference
    candidate through the ordinary path populates both the registry cache and
    the shared target tree, so the graded run recompiles only the submitted
    file.

    `splits` is accepted for symmetry with the other varieties and ignored: the
    compile does not depend on which split will be swept.
    """

    del splits
    reference = MAGIC_ROOT / "candidates" / "reference" / "reference_policy_v1.rs"
    if not reference.is_file():
        raise HarnessError(f"no reference candidate to prewarm with: {reference}")
    with tempfile.TemporaryDirectory() as temp:
        work = write_candidate_crate(reference, "prewarm", Path(temp))
        binary = build_candidate(work)
    print(f"prewarmed candidate crate: {binary}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--candidate-id", default="agent")
    parser.add_argument("--deck", default=None, help="a deck id from the roster's pool")
    parser.add_argument("--split", choices=["train", "heldout"], default="heldout")
    parser.add_argument(
        "--output-root", type=Path, default=MAGIC_ROOT / "artifacts" / "policy-sweep"
    )
    args = parser.parse_args()

    output = args.output_root.resolve()
    output.mkdir(parents=True, exist_ok=True)
    result_path = output / "result.json"

    try:
        roster = roster_payload()
        candidate = args.candidate.resolve()
        if not candidate.is_file():
            raise SweepError(f"missing candidate policy: {candidate}")
        deck = resolve_deck(candidate, args.deck, roster)

        with tempfile.TemporaryDirectory() as temp:
            work = write_candidate_crate(candidate, args.candidate_id, Path(temp))
            binary = build_candidate(work)
            report = run_sweep(binary, deck=deck, split=args.split, out=output)

        (output / "leaderboard.json").write_text(
            json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )

        # `passes` is the Rust gate: the paired lower bound strictly above zero
        # AND no significant per-opponent regression. Train is feedback, so a
        # positive mean delta is enough there.
        passed = bool(report.get("passes")) if args.split == "heldout" else float(
            report.get("reward") or 0.0
        ) > 0.0
        reward = float(report.get("reward") or 0.0)
        result = {
            "schema_version": "cardbench.harbor.result.v1",
            "benchmark_family": FAMILY,
            "task_id": TASK_ID,
            "split": args.split,
            "roster_id": report.get("roster_id"),
            "heldout_manifest_sha256": report.get("heldout_manifest_sha256"),
            "score_metric": report.get("score_metric"),
            "candidate_deck": deck,
            "best_candidate_id": args.candidate_id,
            "baseline_score": report.get("reference_overall_win_rate"),
            "best_score": report.get("candidate_overall_win_rate"),
            "delta_vs_baseline": reward,
            "reward_ci": report.get("reward_ci"),
            "cell_count": (report.get("coverage") or {}).get("expected"),
            "coverage_complete": (report.get("coverage") or {}).get("complete"),
            "scored": report.get("scored"),
            "regressions": report.get("regressions"),
            "evaluated_candidate_count": 1,
            "leaderboard_path": str(output / "leaderboard.json"),
            "passed": passed,
            "harbor_reward": max(0.0, min(1.0, reward)) if passed else 0.0,
            "reward_status": "scored",
        }
    except SweepError as exc:
        result = {
            "schema_version": "cardbench.harbor.result.v1",
            "benchmark_family": FAMILY,
            "task_id": TASK_ID,
            "split": args.split,
            "passed": False,
            "harbor_reward": 0.0,
            "reward_status": "scored",
            "error": str(exc),
        }
    except Exception as exc:  # noqa: BLE001 - anything unclassified is ours, not theirs
        result = {
            "schema_version": "cardbench.harbor.result.v1",
            "benchmark_family": FAMILY,
            "task_id": TASK_ID,
            "split": args.split,
            "passed": False,
            "reward_status": "harness_failed",
            "error": f"{type(exc).__name__}: {exc}",
        }

    result_path.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(result, indent=2))

    if result.get("reward_status") == "harness_failed":
        print("no reward.txt written: the harness failed, so there is no grade", file=sys.stderr)
        return 2

    (output / "reward.txt").write_text(f"{result['harbor_reward']:.8f}\n", encoding="utf-8")
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
