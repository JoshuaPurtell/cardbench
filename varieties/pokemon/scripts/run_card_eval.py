#!/usr/bin/env python3
"""Evaluate one authored Pokémon card against its deterministic test module."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import tempfile
from pathlib import Path

POKEMON = Path(__file__).resolve().parents[1]
CARDS = POKEMON / "cards"
ENGINE = POKEMON / "engine"
SEALED = Path(os.environ.get("CARDBENCH_SEALED_ROOT", CARDS / ".sealed"))


def load_instance(instance_id: str) -> tuple[dict, str, str]:
    path = CARDS / "instances" / f"{instance_id}.json"
    if not path.is_file():
        raise ValueError(f"unknown card instance: {instance_id}")
    data = json.loads(path.read_text())
    module = Path(data["card_file"]).stem
    expansion = data["expansion"]
    set_dir = {"dragon_frontiers": "df", "holon_phantoms": "hp"}.get(expansion)
    if not set_dir:
        raise ValueError(f"unsupported Pokémon expansion: {expansion}")
    return data, module, set_dir


def merge_stub_constants(candidate: str, stub: str) -> str:
    candidate_names = set(re.findall(r"^pub const ([A-Za-z0-9_]+)", candidate, re.MULTILINE))
    missing = []
    for line in stub.splitlines():
        match = re.match(r"pub const ([A-Za-z0-9_]+)", line)
        if match and match.group(1) not in candidate_names:
            missing.append(line)
    if not missing:
        return candidate
    first_function = re.search(r"^(?:pub )?fn ", candidate, re.MULTILINE)
    offset = first_function.start() if first_function else len(candidate)
    return candidate[:offset] + "\n".join(missing) + "\n\n" + candidate[offset:]


def run(cmd: list[str], cwd: Path, env: dict[str, str]) -> subprocess.CompletedProcess[str]:
    return subprocess.run(cmd, cwd=cwd, env=env, text=True, capture_output=True, check=False)


def evaluate(instance_id: str, candidate: Path, output_root: Path) -> dict:
    instance, module, set_dir = load_instance(instance_id)
    filename = f"{module}.rs"
    stub = CARDS / "stubs" / filename
    tests = SEALED / "pokemon" / "card" / "tests" / f"{module}_eval.rs"
    missing = [str(path) for path in (candidate, stub, tests) if not path.is_file()]
    if missing:
        raise FileNotFoundError("missing card task assets: " + ", ".join(missing))

    expected_tests = tests.read_text().count("#[test]")
    catalog_path = CARDS / "catalog.json"
    catalog = json.loads(catalog_path.read_text())
    result = {
        "schema_version": "cardbench.card.result.v1",
        "task_id": "cardbench/pokemon/card",
        "instance_id": instance_id,
        "suite_id": catalog["suite_id"],
        "suite_sha256": hashlib.sha256(catalog_path.read_bytes()).hexdigest(),
        "expansion_id": instance["expansion"],
        "authority": "sealed_reference",
        "score_metric": "functional_test_pass_rate",
        "module": module,
        "compile_ok": 0,
        "tests_passed": 0,
        "tests_total": expected_tests,
        "score": 0.0,
        "passed": False,
    }

    with tempfile.TemporaryDirectory(prefix=f"cardbench-card-{instance_id}-") as raw_tmp:
        tmp = Path(raw_tmp)
        app = tmp / "app"
        shutil.copytree(
            ENGINE,
            app,
            ignore=shutil.ignore_patterns("target", ".cache", "__pycache__"),
        )
        destination = app / "scaffold" / "src" / set_dir / "cards" / filename
        destination.parent.mkdir(parents=True, exist_ok=True)
        source = merge_stub_constants(candidate.read_text(), stub.read_text())
        destination.write_text(
            source
            + "\n\n// CARDBENCH CARD EVALUATION TESTS\n"
            + tests.read_text()
        )
        mod_file = destination.parent / "mod.rs"
        declaration = f"pub mod {module};"
        current_mod = mod_file.read_text() if mod_file.exists() else ""
        if declaration not in current_mod:
            mod_file.write_text(current_mod.rstrip() + f"\n{declaration}\n")

        env = os.environ.copy()
        env["CARGO_TARGET_DIR"] = str(
            Path(os.environ.get("CARDBENCH_CARGO_TARGET", POKEMON / ".cache" / "cargo-target"))
            / "card"
            / instance_id
        )
        check = run(["cargo", "check", "--package", "tcg_expansions"], app, env)
        result["compile_stdout"] = check.stdout[-4000:]
        result["compile_stderr"] = check.stderr[-4000:]
        if check.returncode == 0:
            result["compile_ok"] = 1
            tested = run(
                [
                    "cargo",
                    "test",
                    "--package",
                    "tcg_expansions",
                    module,
                    "--",
                    "--test-threads=1",
                ],
                app,
                env,
            )
            result["test_stdout"] = tested.stdout[-8000:]
            result["test_stderr"] = tested.stderr[-8000:]
            summaries = re.findall(r"test result: \w+\. (\d+) passed; (\d+) failed", tested.stdout)
            if summaries:
                result["tests_passed"] = max(int(passed) for passed, _ in summaries)
                observed = max(int(passed) + int(failed) for passed, failed in summaries)
                result["tests_total"] = max(expected_tests, observed)

    total = int(result["tests_total"])
    passed = int(result["tests_passed"])
    if result["compile_ok"] and total:
        result["score"] = round(0.3 + 0.7 * passed / total, 4)
    result["passed"] = bool(result["compile_ok"] and total and passed == total)
    result["harbor_reward"] = result["score"]
    output_root.mkdir(parents=True, exist_ok=True)
    (output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    (output_root / "reward.txt").write_text(f"{result['score']:.4f}\n")
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--instance", default="df-097-rayquaza-ex")
    parser.add_argument("--candidate", type=Path)
    parser.add_argument("--output-root", type=Path, required=True)
    parser.add_argument("--list", action="store_true")
    args = parser.parse_args()
    if args.list:
        for path in sorted((CARDS / "instances").glob("*.json")):
            data, module, _ = load_instance(path.stem)
            complete = all(
                candidate.is_file()
                for candidate in (
                    CARDS / "stubs" / f"{module}.rs",
                    SEALED / "pokemon" / "card" / "implementations" / f"{module}.rs",
                    SEALED / "pokemon" / "card" / "tests" / f"{module}_eval.rs",
                )
            )
            if complete:
                print(f"{data['id']}\t{data['expansion']}\t{data['name']}")
        return 0

    _, module, _ = load_instance(args.instance)
    candidate = (
        args.candidate
        or SEALED / "pokemon" / "card" / "implementations" / f"{module}.rs"
    )
    result = evaluate(args.instance, candidate.resolve(), args.output_root.resolve())
    print(json.dumps(result, indent=2))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
