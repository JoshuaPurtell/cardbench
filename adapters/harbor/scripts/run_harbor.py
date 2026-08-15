#!/usr/bin/env python3
"""Unified CardBench Harbor entry point for reference and Codex runs."""

from __future__ import annotations

import argparse
import base64
import datetime as dt
import json
import os
import shutil
import subprocess
import sys
from collections.abc import Mapping
from pathlib import Path
from typing import Any

_PROTOCOL_MISMATCH_PREFIX = "harbor_protocol_mismatch"
_SEALED_MODEL_KEYS = frozenset({"model", "model_id"})
_SEALED_VERSION_KEYS = frozenset({"multi_agent_version"})
_REQUEST_IDENTITY_KEYS = frozenset(
    {
        "codex_config_toml_append",
        "expected_model_id",
        "expected_multi_agent_version",
        "effective_model_name",
        "model_name",
    }
)
_TRACE_PATH_KEYS = ("bundle", "raw_codex_jsonl", "native_evaluation")
_MAX_SEALED_FILES = 64
_MAX_SEALED_FILE_BYTES = 20 * 1024 * 1024

REPO = Path(__file__).resolve().parents[3]
POKEMON = REPO / "varieties" / "pokemon"
EVALS = Path(os.environ.get("CARDBENCH_EVALS_ROOT", Path.home() / "Documents" / "GitHub" / "evals"))
CODEX_RUNNER = EVALS / "core" / "harbor" / "runner" / "codex_harbor_runner.py"
REFERENCE_POLICY = POKEMON / "candidates" / "reference" / "simple_heuristic_ai.rs"
REFERENCE_DECK = POKEMON / "decks" / "gardevoir_delta_control.json"
CARDS = POKEMON / "cards"
SEALED = Path(os.environ.get("CARDBENCH_SEALED_ROOT", CARDS / ".sealed"))

FAMILIES = {
    "code-policy": "code_policy",
    "code_policy": "code_policy",
    "deck-opt": "deck_opt",
    "deck_opt": "deck_opt",
    "card": "card",
    "set-engine": "set_engine",
    "set_engine": "set_engine",
    "engine": "engine",
    "full-engine": "full_engine",
    "full_engine": "full_engine",
    "react": "react",
    "cybernetic": "cybernetic",
}


class HarborProtocolError(RuntimeError):
    """Raised when sealed Harbor identity does not match the expected arm."""


def _optional_identity(value: Any) -> str | None:
    if value is None or isinstance(value, bool):
        return None
    if isinstance(value, (int, float)) and not isinstance(value, bool):
        text = str(value).strip()
    elif isinstance(value, str):
        text = value.strip()
    else:
        return None
    if not text or text.lower() in {"none", "null"}:
        return None
    return text


def _normalize_model_id(value: Any) -> str | None:
    text = _optional_identity(value)
    if text is None:
        return None
    return text.split("/", 1)[-1]


def _normalize_multi_agent_version(value: Any) -> str | None:
    text = _optional_identity(value)
    return None if text is None else text.lower()


def expected_harbor_protocol_from_env(
    env: Mapping[str, str] | None = None,
) -> tuple[str | None, str | None]:
    source = os.environ if env is None else env
    return (
        _normalize_model_id(source.get("CARDBENCH_HARBOR_EXPECTED_MODEL_ID")),
        _normalize_multi_agent_version(
            source.get("CARDBENCH_HARBOR_EXPECTED_MULTI_AGENT_VERSION")
        ),
    )


def _collect_sealed_identity(
    value: Any,
    *,
    models: set[str],
    versions: set[str],
    depth: int = 0,
) -> None:
    if depth > 12 or value is None:
        return
    if isinstance(value, Mapping):
        for key, child in value.items():
            name = str(key)
            if name in _REQUEST_IDENTITY_KEYS:
                continue
            if name in _SEALED_MODEL_KEYS:
                model = _normalize_model_id(child)
                if model is not None:
                    models.add(model)
                continue
            if name in _SEALED_VERSION_KEYS:
                version = _normalize_multi_agent_version(child)
                if version is not None:
                    versions.add(version)
                continue
            _collect_sealed_identity(
                child, models=models, versions=versions, depth=depth + 1
            )
        return
    if isinstance(value, list):
        for child in value[:200]:
            _collect_sealed_identity(
                child, models=models, versions=versions, depth=depth + 1
            )


def _load_json_document(path: Path) -> Any:
    try:
        if path.stat().st_size > _MAX_SEALED_FILE_BYTES:
            return None
        text = path.read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return None
    if path.suffix == ".jsonl" or path.name.endswith(".jsonl"):
        records: list[Any] = []
        for line in text.splitlines():
            line = line.strip()
            if not line:
                continue
            try:
                records.append(json.loads(line))
            except json.JSONDecodeError:
                continue
        return records
    try:
        return json.loads(text)
    except json.JSONDecodeError:
        return None


def _trace_file_paths(path: Path) -> list[Path]:
    if not path.exists():
        return []
    if path.is_file():
        return [path]
    found: list[Path] = []
    for child in sorted(path.rglob("*")):
        if not child.is_file():
            continue
        name = child.name
        if name.endswith(".jsonl") or "trace" in name or name == "manifest.json":
            found.append(child)
        if len(found) >= _MAX_SEALED_FILES:
            break
    return found


def _search_roots(result_path: Path) -> list[Path]:
    roots = [Path(str(result_path) + ".trace_v5.json")]
    logs = result_path.parent / "logs"
    roots.extend((logs / "trace_v5", logs / "agent"))
    workspace = result_path.parent / "workspace"
    roots.extend(
        (
            workspace / "logs" / "trace_v5",
            workspace / "logs" / "agent",
            workspace / ".codex" / "sessions",
        )
    )
    unique: list[Path] = []
    seen: set[Path] = set()
    for root in roots:
        resolved = root.expanduser()
        if resolved in seen:
            continue
        seen.add(resolved)
        unique.append(resolved)
    return unique


def collect_sealed_harbor_protocol(
    result: Mapping[str, Any],
    *,
    result_path: Path | None = None,
) -> tuple[set[str], set[str]]:
    models: set[str] = set()
    versions: set[str] = set()
    metadata = result.get("metadata")
    metadata = metadata if isinstance(metadata, Mapping) else {}
    trace_v5 = metadata.get("trace_v5")
    if isinstance(trace_v5, Mapping):
        _collect_sealed_identity(trace_v5, models=models, versions=versions)
        for key in _TRACE_PATH_KEYS:
            raw_path = _optional_identity(trace_v5.get(key))
            if raw_path:
                for path in _trace_file_paths(Path(raw_path)):
                    _collect_sealed_identity(
                        _load_json_document(path), models=models, versions=versions
                    )
    if result_path is not None:
        for root in _search_roots(result_path):
            for path in _trace_file_paths(root):
                _collect_sealed_identity(
                    _load_json_document(path), models=models, versions=versions
                )
    return models, versions


def assert_expected_harbor_protocol(
    result: Mapping[str, Any],
    *,
    result_path: Path | None = None,
    env: Mapping[str, str] | None = None,
) -> None:
    expected_model, expected_version = expected_harbor_protocol_from_env(env)
    if expected_model is None and expected_version is None:
        return
    models, versions = collect_sealed_harbor_protocol(
        result, result_path=result_path
    )
    failures: list[str] = []
    if expected_model is not None:
        sealed_model = ",".join(sorted(models)) if models else "null"
        if expected_model not in models:
            failures.append(
                f"expected_model_id={expected_model} sealed={sealed_model}"
            )
    if expected_version is not None:
        sealed_version = ",".join(sorted(versions)) if versions else "null"
        if versions != {expected_version}:
            failures.append(
                f"expected_multi_agent_version={expected_version} sealed={sealed_version}"
            )
    if failures:
        raise HarborProtocolError(
            _PROTOCOL_MISMATCH_PREFIX + ":" + "; ".join(failures)
        )


def fresh_output(family: str, command: str) -> Path:
    configured = os.environ.get("CARDBENCH_HARBOR_OUT")
    if configured:
        return Path(configured).expanduser().resolve()
    stamp = dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")
    return REPO / "artifacts" / "harbor" / f"{family}-{command}-{stamp}-{os.getpid()}"


def score_command(
    family: str,
    candidate: Path | None,
    output: Path,
    *,
    instance: str,
    expansion: str,
    variant: str,
    suite: str,
) -> list[str]:
    if family == "code_policy":
        return [
            sys.executable,
            str(POKEMON / "scripts" / "run_policy_sweep.py"),
            "--candidate",
            str(candidate),
            "--candidate-id",
            "reference" if candidate == REFERENCE_POLICY else "agent",
            "--output-root",
            str(output / "logs" / "verifier"),
        ]
    if family == "deck_opt":
        return [
            sys.executable,
            str(POKEMON / "scripts" / "run_deck_eval.py"),
            "--candidate",
            str(candidate),
            "--candidate-id",
            "reference" if candidate == REFERENCE_DECK else "agent",
            "--output-root",
            str(output / "logs" / "verifier"),
        ]
    if family == "card":
        command = [
            sys.executable,
            str(POKEMON / "scripts" / "run_card_eval.py"),
            "--instance",
            instance,
            "--output-root",
            str(output / "logs" / "verifier"),
        ]
        if candidate is not None:
            command.extend(["--candidate", str(candidate)])
        return command
    if family == "set_engine":
        return [
            sys.executable,
            str(POKEMON / "scripts" / "run_set_engine_eval.py"),
            "--expansion",
            expansion,
            "--variant",
            variant,
            "--subject",
            "gold",
            "--suite",
            suite,
            "--output-root",
            str(output / "logs" / "verifier"),
        ]
    if family == "engine":
        return [
            sys.executable,
            str(POKEMON / "scripts" / "run_engine_parity.py"),
            "--output-root",
            str(output / "logs" / "verifier"),
        ]
    raise ValueError(f"{family} is scaffold-only")


def write_receipt(
    output: Path,
    family: str,
    command: str,
    agent_rc: int,
    verify_rc: int,
    *,
    protocol_error: str | None = None,
) -> None:
    result_path = output / "logs" / "verifier" / "result.json"
    if family == "engine":
        result_path = output / "logs" / "verifier" / "engine-check.json"
    verifier = json.loads(result_path.read_text()) if result_path.exists() else {}
    payload = {
        "schema_version": "cardbench.harbor.lane_receipt.v1",
        "task_id": f"cardbench/pokemon/{family}",
        "family": family,
        "variety": "pokemon",
        "agent": "reference" if command == "verify" else "codex",
        "model": os.environ.get("CARDBENCH_HARBOR_MODEL", "openai/gpt-5.4-mini"),
        "effort": os.environ.get("CARDBENCH_HARBOR_EFFORT", "low"),
        "agent_rc": agent_rc,
        "verify_rc": verify_rc,
        "out_dir": str(output),
        "verifier": verifier,
    }
    if protocol_error:
        payload["reward"] = None
        payload["contract_error"] = protocol_error
        payload["error"] = protocol_error
    else:
        reward = verifier.get("harbor_reward")
        payload["reward"] = None if reward is None else float(reward)
    for key in (
        "baseline_score",
        "best_score",
        "delta_vs_baseline",
        "best_candidate_id",
        "score_metric",
        "instance_id",
        "expansion_id",
        "expansion",
        "variant",
        "suite",
        "tests_passed",
        "tests_total",
        "games_matched",
        "games_total",
        "authority",
        "suite_id",
        "suite_sha256",
        "compile_ok",
        "compile_passed",
    ):
        if key in verifier:
            payload[key] = verifier[key]
    (output / "lane-receipt.json").write_text(json.dumps(payload, indent=2) + "\n")


def run_reference(
    family: str,
    output: Path,
    *,
    instance: str,
    expansion: str,
    variant: str,
    suite: str,
) -> int:
    candidate: Path | None = None
    if family == "code_policy":
        candidate = REFERENCE_POLICY
    elif family == "deck_opt":
        candidate = REFERENCE_DECK
    elif family == "card":
        data = json.loads((CARDS / "instances" / f"{instance}.json").read_text())
        module = Path(data["card_file"]).stem
        candidate = SEALED / "pokemon" / "card" / "implementations" / f"{module}.rs"
    command = score_command(
        family,
        candidate,
        output,
        instance=instance,
        expansion=expansion,
        variant=variant,
        suite=suite,
    )
    completed = subprocess.run(command)
    write_receipt(output, family, "verify", 0, completed.returncode)
    print(f"receipt: {output / 'lane-receipt.json'}")
    return completed.returncode


def copy_workspace(destination: Path) -> None:
    def ignore(_path: str, names: list[str]) -> set[str]:
        return {name for name in names if name in {".git", "artifacts", "target", ".cache", "__pycache__"}}

    shutil.copytree(REPO, destination, ignore=ignore)


def run_codex(
    family: str,
    output: Path,
    *,
    instance: str,
    expansion: str,
    variant: str,
    suite: str,
) -> int:
    if family not in {"code_policy", "deck_opt"}:
        raise ValueError(f"Codex is not promoted for {family}")
    if not CODEX_RUNNER.is_file():
        raise FileNotFoundError(f"missing shared evals Harbor runner: {CODEX_RUNNER}")
    auth = Path(os.environ.get("CODEX_HOME", Path.home() / ".codex")) / "auth.json"
    if not auth.is_file():
        raise FileNotFoundError("Codex auth unavailable; run `codex login`")
    workspace = output / "workspace"
    output.mkdir(parents=True, exist_ok=True)
    copy_workspace(workspace)
    candidate_rel = Path("candidate") / ("policy.rs" if family == "code_policy" else "deck.json")
    task_root = REPO / "adapters" / "harbor" / "bundles" / family
    payload = {
        "trace_correlation_id": f"cardbench-{family}-{output.name}",
        "deployment_name": f"cardbench_{family}",
        "codex_timeout_seconds": int(os.environ.get("CARDBENCH_HARBOR_TIMEOUT_SEC", "3600")),
        "harbor_agent": {
            "name": "codex",
            "model_name": os.environ.get("CARDBENCH_HARBOR_MODEL", "openai/gpt-5.4-mini"),
            "kwargs": {
                "reasoning_effort": os.environ.get("CARDBENCH_HARBOR_EFFORT", "low"),
                "codex_timeout_seconds": int(os.environ.get("CARDBENCH_HARBOR_TIMEOUT_SEC", "3600")),
            },
        },
        "task_metadata": {
            "workspace_dir": str(workspace),
            "task_id": f"cardbench/pokemon/{family}",
            "benchmark": "cardbench",
            "submission_path": str(candidate_rel),
        },
        "submission_path": str(candidate_rel),
        "codex_auth_json_b64": base64.b64encode(auth.read_bytes()).decode("ascii"),
        "codex_auth_source": "host_codex_auth_json",
        "env": {"CARDBENCH_WORKSPACE_ROOT": str(workspace)},
    }
    append = str(os.environ.get("CARDBENCH_HARBOR_CODEX_CONFIG_TOML_APPEND") or "").strip()
    if append:
        payload["codex_config_toml_append"] = append
    expected_model = str(
        os.environ.get("CARDBENCH_HARBOR_EXPECTED_MODEL_ID") or ""
    ).strip() or str(payload["harbor_agent"]["model_name"])
    payload["expected_model_id"] = expected_model
    expected_version = str(
        os.environ.get("CARDBENCH_HARBOR_EXPECTED_MULTI_AGENT_VERSION") or ""
    ).strip()
    if expected_version:
        payload["expected_multi_agent_version"] = expected_version
    rollout = output / "rollout.json"
    rollout.write_text(json.dumps(payload, indent=2) + "\n")
    agent_result = output / "rollout_result.json"
    agent = subprocess.run(
        [
            sys.executable,
            str(CODEX_RUNNER),
            "--input",
            str(rollout),
            "--output",
            str(agent_result),
            "--task-root",
            str(task_root),
        ]
    )
    try:
        agent_payload = json.loads(agent_result.read_text(encoding="utf-8"))
        if not isinstance(agent_payload, dict):
            agent_payload = {}
    except (OSError, json.JSONDecodeError):
        agent_payload = {}
    try:
        assert_expected_harbor_protocol(agent_payload, result_path=agent_result)
    except HarborProtocolError as exc:
        write_receipt(
            output,
            family,
            "codex",
            agent.returncode,
            1,
            protocol_error=str(exc),
        )
        print(f"receipt: {output / 'lane-receipt.json'}")
        print(f"CardBench Harbor protocol mismatch: {exc}", file=sys.stderr)
        return 1
    candidate = workspace / candidate_rel
    if candidate.exists():
        scored = subprocess.run(
            score_command(
                family,
                candidate,
                output,
                instance=instance,
                expansion=expansion,
                variant=variant,
                suite=suite,
            )
        )
        verify_rc = scored.returncode
    else:
        verify_rc = 1
        verifier_dir = output / "logs" / "verifier"
        verifier_dir.mkdir(parents=True, exist_ok=True)
        (verifier_dir / "result.json").write_text(
            json.dumps({"passed": False, "harbor_reward": 0.0, "error": f"missing {candidate_rel}"}, indent=2) + "\n"
        )
        (verifier_dir / "reward.txt").write_text("0.0\n")
    write_receipt(output, family, "codex", agent.returncode, verify_rc)
    print(f"receipt: {output / 'lane-receipt.json'}")
    return 0 if agent.returncode == 0 and verify_rc == 0 else 1


def main() -> int:
    parser = argparse.ArgumentParser(
        usage="./adapters/harbor/run.sh <family> <verify|codex|list> [pokemon]"
    )
    parser.add_argument("family", choices=sorted(FAMILIES))
    parser.add_argument("command", choices=["verify", "codex", "list"])
    parser.add_argument("variety", nargs="?", default="pokemon")
    parser.add_argument("--instance", default="df-097-rayquaza-ex")
    parser.add_argument("--expansion", default="crystal_guardians")
    parser.add_argument("--variant", default="0pct")
    parser.add_argument("--suite", choices=["train", "hidden"], default="train")
    args = parser.parse_args()
    family = FAMILIES[args.family]
    if args.variety != "pokemon":
        parser.error("only the pokemon variety is runnable; magic is reserved")
    if args.command == "list":
        status = (
            "reference-runnable"
            if family in {"code_policy", "deck_opt", "card", "set_engine", "engine"}
            else "scaffold"
        )
        print(f"cardbench/pokemon/{family}\t{status}")
        return 0
    if family in {"react", "cybernetic", "full_engine"}:
        print(f"cardbench/pokemon/{family} is scaffold-only", file=sys.stderr)
        return 2
    output = fresh_output(family, args.command)
    output.mkdir(parents=True, exist_ok=True)
    try:
        return (
            run_reference(
                family,
                output,
                instance=args.instance,
                expansion=args.expansion,
                variant=args.variant,
                suite=args.suite,
            )
            if args.command == "verify"
            else run_codex(
                family,
                output,
                instance=args.instance,
                expansion=args.expansion,
                variant=args.variant,
                suite=args.suite,
            )
        )
    except Exception as exc:
        print(f"CardBench Harbor failed: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
