#!/usr/bin/env python3

from __future__ import annotations

import json
import argparse
import hashlib
import os
import re
import shutil
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path

try:
    import tomllib
except ModuleNotFoundError:  # pragma: no cover - Python < 3.11
    import tomli as tomllib


POKEMON_ROOT = Path(__file__).resolve().parents[1]
ROOT = POKEMON_ROOT / "sets"
ENGINE_CORE_REPO = POKEMON_ROOT / "engine"
TARGET_ROOT = POKEMON_ROOT / ".cache" / "cargo-target"


@dataclass(frozen=True)
class GradedFileSpec:
    name: str
    gold: Path
    stub: Path
    workspace_path: Path


@dataclass(frozen=True)
class VariantFileSpec:
    source: str
    keep_gold_functions: tuple[str, ...]
    stubbed_functions: tuple[str, ...]


@dataclass(frozen=True)
class VariantSpec:
    name: str
    description: str
    files: dict[str, VariantFileSpec]


@dataclass(frozen=True)
class FamilySpec:
    root: Path
    name: str
    description: str
    engine_module: str
    task_name: str
    readiness: str
    readiness_reason: str | None
    engine_commit: str
    graded_files: dict[str, GradedFileSpec]
    eval_host_file: str
    public_data_dir: Path
    public_train_dir: Path
    train_runtime: Path
    train_scenarios: Path
    hidden_dir: Path
    hidden_runtime: Path
    hidden_scenarios: Path
    task_dir: Path
    variants: dict[str, VariantSpec]

    @property
    def variants_dir(self) -> Path:
        return self.root / "variants"

    @property
    def gold_files(self) -> dict[str, Path]:
        return {name: spec.gold for name, spec in self.graded_files.items()}

    @property
    def stub_files(self) -> dict[str, Path]:
        return {name: spec.stub for name, spec in self.graded_files.items()}


@dataclass(frozen=True)
class PublicBlockRange:
    name: str
    start: int
    end: int
    text: str


def run(
    cmd: list[str],
    *,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        cmd,
        cwd=str(cwd) if cwd else None,
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )


def cargo_env(target_dir: Path) -> dict[str, str]:
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(target_dir)
    env["CARGO_INCREMENTAL"] = "0"
    env["RUSTFLAGS"] = env.get("RUSTFLAGS", "").strip()
    env["RUSTFLAGS"] = (env["RUSTFLAGS"] + " -C debuginfo=0").strip()
    return env


def read_toml(path: Path) -> dict:
    return tomllib.loads(path.read_text())


def resolve_path(base: Path, value: str) -> Path:
    return (base / value).resolve()


def load_family(name: str) -> FamilySpec:
    family_root = ROOT / name
    manifest_path = family_root / "family.toml"
    data = read_toml(manifest_path)

    family_data = data["family"]
    public_data = data["public"]
    hidden_data = data["hidden"]
    task_data = data.get("task", {})

    graded_files: dict[str, GradedFileSpec] = {}
    for entry in data["graded_files"]:
        graded_files[entry["name"]] = GradedFileSpec(
            name=entry["name"],
            gold=resolve_path(family_root, entry["gold"]),
            stub=resolve_path(family_root, entry["stub"]),
            workspace_path=Path(entry["workspace_path"]),
        )

    variants: dict[str, VariantSpec] = {}
    for variant_name, variant_entry in data["variants"].items():
        file_specs: dict[str, VariantFileSpec] = {}
        for graded_name in graded_files:
            file_entry = variant_entry.get("files", {}).get(graded_name, {})
            file_specs[graded_name] = VariantFileSpec(
                source=file_entry.get("source", "gold"),
                keep_gold_functions=tuple(file_entry.get("keep_gold_functions", [])),
                stubbed_functions=tuple(file_entry.get("stubbed_functions", [])),
            )
        variants[variant_name] = VariantSpec(
            name=variant_name,
            description=variant_entry["description"],
            files=file_specs,
        )

    return FamilySpec(
        root=family_root,
        name=family_data["name"],
        description=family_data["description"],
        engine_module=family_data["engine_module"],
        task_name=family_data["task_name"],
        readiness=family_data.get("readiness", "ready"),
        readiness_reason=family_data.get("readiness_reason"),
        engine_commit=data["engine_core"]["commit"],
        graded_files=graded_files,
        eval_host_file=family_data["eval_host_file"],
        public_data_dir=resolve_path(family_root, public_data["data_dir"]),
        public_train_dir=resolve_path(family_root, public_data["train_dir"]),
        train_runtime=resolve_path(family_root, public_data["train_runtime"]),
        train_scenarios=resolve_path(family_root, public_data["train_scenarios"]),
        hidden_dir=resolve_path(family_root, hidden_data["dir"]),
        hidden_runtime=resolve_path(family_root, hidden_data["runtime"]),
        hidden_scenarios=resolve_path(family_root, hidden_data["scenarios"]),
        task_dir=resolve_path(family_root, task_data.get("dir", "source/task")),
        variants=variants,
    )


def extract_pub_block_ranges(text: str) -> dict[str, PublicBlockRange]:
    lines = text.splitlines(keepends=True)
    offsets: list[int] = []
    cursor = 0
    for line in lines:
        offsets.append(cursor)
        cursor += len(line)

    blocks: dict[str, PublicBlockRange] = {}
    i = 0
    while i < len(lines):
        line = lines[i]
        match = re.match(r"pub fn ([A-Za-z0-9_]+)\s*\(", line)
        if not match:
            i += 1
            continue

        name = match.group(1)
        start = i
        brace_depth = line.count("{") - line.count("}")
        seen_open_brace = "{" in line
        i += 1
        while i < len(lines):
            seen_open_brace = seen_open_brace or "{" in lines[i]
            brace_depth += lines[i].count("{") - lines[i].count("}")
            i += 1
            if seen_open_brace and brace_depth <= 0:
                break
        start_offset = offsets[start]
        end_offset = offsets[i] if i < len(offsets) else len(text)
        blocks[name] = PublicBlockRange(
            name=name,
            start=start_offset,
            end=end_offset,
            text=text[start_offset:end_offset],
        )
    return blocks


def extract_pub_blocks(text: str) -> dict[str, str]:
    return {name: block.text for name, block in extract_pub_block_ranges(text).items()}


def replace_blocks(base_text: str, replacements: dict[str, str]) -> str:
    result = base_text
    ranges = extract_pub_block_ranges(base_text)
    missing = set(replacements).difference(ranges)
    if missing:
        raise RuntimeError(f"expected to replace exactly one block for {sorted(missing)}")

    ordered = sorted(
        (ranges[name] for name in replacements),
        key=lambda block: block.start,
        reverse=True,
    )
    for block in ordered:
        replacement = replacements[block.name].rstrip("\n") + "\n"
        result = result[: block.start] + replacement + result[block.end :]
    return result


def render_variant_text(
    gold_text: str,
    stub_text: str,
    file_spec: VariantFileSpec,
) -> tuple[str, int, int]:
    gold_blocks = extract_pub_blocks(gold_text)
    stub_blocks = extract_pub_blocks(stub_text)
    total_public_functions = len(gold_blocks)

    if file_spec.keep_gold_functions:
        keep = set(file_spec.keep_gold_functions)
        unknown = keep.difference(gold_blocks)
        if unknown:
            raise RuntimeError(f"unknown keep_gold_functions: {sorted(unknown)}")
        replacements = {
            name: stub_blocks[name]
            for name in gold_blocks
            if name not in keep
        }
        return replace_blocks(gold_text, replacements), len(keep), total_public_functions

    if file_spec.stubbed_functions:
        stubbed = set(file_spec.stubbed_functions)
        unknown = stubbed.difference(gold_blocks)
        if unknown:
            raise RuntimeError(f"unknown stubbed_functions: {sorted(unknown)}")
        replacements = {name: stub_blocks[name] for name in stubbed}
        return (
            replace_blocks(gold_text, replacements),
            total_public_functions - len(stubbed),
            total_public_functions,
        )

    if file_spec.source == "stub":
        return stub_text, 0, total_public_functions

    if file_spec.source == "gold":
        return gold_text, total_public_functions, total_public_functions

    raise RuntimeError("variant file spec must define source, keep_gold_functions, or stubbed_functions")


def materialize_variant(family: FamilySpec, variant_name: str) -> Path:
    variant = family.variants[variant_name]
    out_dir = family.variants_dir / variant_name
    out_dir.mkdir(parents=True, exist_ok=True)

    revealed_public_functions = 0
    total_public_functions = 0
    per_file: dict[str, dict[str, object]] = {}

    for name, graded_file in family.graded_files.items():
        gold_text = graded_file.gold.read_text()
        stub_text = graded_file.stub.read_text()
        rendered_text, revealed, total = render_variant_text(
            gold_text,
            stub_text,
            variant.files[name],
        )
        out_path = out_dir / graded_file.stub.name
        out_path.write_text(rendered_text)
        revealed_public_functions += revealed
        total_public_functions += total
        per_file[name] = {
            "source": variant.files[name].source,
            "revealed": revealed,
            "total": total,
            "keep_gold_functions": list(variant.files[name].keep_gold_functions),
            "stubbed_functions": list(variant.files[name].stubbed_functions),
        }

    lines = [
        f'name = "{family.name}_{variant_name}"',
        f'family = "{family.name}"',
        f'variant = "{variant_name}"',
        f'description = "{variant.description}"',
        "",
        "[engine_core]",
        f'commit = "{family.engine_commit}"',
        "",
        "[disclosure]",
        f"revealed_public_functions = {revealed_public_functions}",
        f"total_public_functions = {total_public_functions}",
    ]

    for name, file_data in per_file.items():
        lines.extend(
            [
                "",
                f"[disclosure.per_file.{name}]",
                f'source = "{file_data["source"]}"',
                f'revealed = {file_data["revealed"]}',
                f'total = {file_data["total"]}',
                "keep_gold_functions = ["
                + "".join(
                    f'\n  "{value}",' for value in file_data["keep_gold_functions"]
                )
                + ("\n]" if file_data["keep_gold_functions"] else "]"),
                "stubbed_functions = ["
                + "".join(
                    f'\n  "{value}",' for value in file_data["stubbed_functions"]
                )
                + ("\n]" if file_data["stubbed_functions"] else "]"),
            ]
        )

    (out_dir / "variant.toml").write_text("\n".join(lines) + "\n")
    return out_dir


def materialize_all_variants(family: FamilySpec) -> list[Path]:
    return [materialize_variant(family, name) for name in family.variants]


def archive_engine_core(commit: str, out_dir: Path) -> None:
    if not ENGINE_CORE_REPO.exists():
        raise RuntimeError(f"missing engine core repo at {ENGINE_CORE_REPO}")
    archive = subprocess.Popen(
        ["git", "-C", str(ENGINE_CORE_REPO), "archive", commit],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=False,
    )
    extract = subprocess.run(
        ["tar", "-xf", "-", "-C", str(out_dir)],
        stdin=archive.stdout,
        capture_output=True,
        text=False,
        check=False,
    )
    if archive.stdout:
        archive.stdout.close()
    stderr = archive.stderr.read().decode() if archive.stderr else ""
    archive_rc = archive.wait()
    if archive_rc != 0:
        raise RuntimeError(f"git archive failed for {commit}: {stderr.strip()}")
    if extract.returncode != 0:
        raise RuntimeError(f"tar extract failed: {extract.stderr.decode().strip()}")


def copy_tree(src: Path, dst: Path) -> None:
    if src.is_dir():
        shutil.copytree(src, dst, dirs_exist_ok=True)
    else:
        dst.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src, dst)


def build_workspace(family: FamilySpec, temp_root: Path) -> tuple[Path, Path]:
    app_dir = temp_root / "app"
    tests_dir = temp_root / "tests"
    app_dir.mkdir(parents=True, exist_ok=True)
    tests_dir.mkdir(parents=True, exist_ok=True)
    shutil.copytree(
        ENGINE_CORE_REPO,
        app_dir,
        dirs_exist_ok=True,
        ignore=shutil.ignore_patterns("target", ".cache", "__pycache__"),
    )

    copy_tree(family.public_data_dir, app_dir / "setbench_data")
    copy_tree(family.public_train_dir, tests_dir / "train")
    (app_dir / "setbench_data" / "train").mkdir(parents=True, exist_ok=True)
    copy_tree(
        family.train_scenarios,
        app_dir / "setbench_data" / "train" / "train_scenarios.json",
    )
    copy_tree(family.hidden_dir, tests_dir / ".hidden")
    copy_tree(family.root / "source" / "gold", tests_dir / ".hidden" / "gold")
    return app_dir, tests_dir


def patch_runtime_paths(text: str, app_dir: Path, tests_dir: Path) -> str:
    return (
        text.replace('include_str!("/app/', f'include_str!("{app_dir}/')
        .replace('include_str!("/tests/', f'include_str!("{tests_dir}/')
        .replace('const GOLD_PATH: &str = "/app/', f'const GOLD_PATH: &str = "{app_dir}/')
    )


def install_eval_source(
    host_target: Path,
    base_source: Path,
    eval_runtime: Path,
    app_dir: Path,
    tests_dir: Path,
) -> None:
    patched_runtime = patch_runtime_paths(eval_runtime.read_text(), app_dir, tests_dir)
    host_target.write_text(
        base_source.read_text()
        + "\n\n// ============================================================================\n"
        + "// SETBENCH EVALUATION TESTS (injected at runtime)\n"
        + "// ============================================================================\n\n"
        + patched_runtime
    )


def static_todo_hits(paths: list[Path]) -> int:
    hits = 0
    for path in paths:
        hits += path.read_text().count("SETBENCH_TODO")
    return hits


def extract_summary(output: str) -> tuple[int, int]:
    match = re.search(r"SETBENCH_SUMMARY matched=(\d+) total=(\d+)", output)
    if not match:
        return 0, 0
    return int(match.group(1)), int(match.group(2))


def select_subject_paths(
    family: FamilySpec,
    subject: str,
    variant_name: str,
) -> dict[str, Path]:
    if subject == "gold":
        return family.gold_files

    variant_dir = family.variants_dir / variant_name
    return {
        name: variant_dir / spec.stub.name
        for name, spec in family.graded_files.items()
    }


def suite_config(family: FamilySpec, suite: str) -> dict[str, object]:
    if suite == "hidden":
        return {
            "eval_runtime": family.hidden_runtime,
            "scenario_fixture": family.hidden_scenarios,
            "gold_fixture": Path(".hidden/generated_gold.json"),
            "test_filter": "setbench_eval_suite",
        }
    if suite == "train":
        return {
            "eval_runtime": family.train_runtime,
            "scenario_fixture": family.train_scenarios,
            "gold_fixture": Path("train/generated_train_gold.json"),
            "test_filter": "setbench_train_suite",
        }
    raise RuntimeError(f"unsupported suite: {suite}")


def run_suite(
    family: FamilySpec,
    *,
    variant: str,
    subject: str,
    suite: str,
) -> dict[str, object]:
    if family.readiness == "blocked":
        return {
            "family": family.name,
            "variant": variant,
            "subject": subject,
            "suite": suite,
            "status": "blocked",
            "reason": family.readiness_reason or "family not ready",
        }

    suite_data = suite_config(family, suite)
    scenario_fixture = suite_data["scenario_fixture"]
    eval_runtime = suite_data["eval_runtime"]
    gold_fixture_rel = suite_data["gold_fixture"]
    test_filter = suite_data["test_filter"]
    expected_games = scenario_fixture.read_text().count('"name"')
    candidate_paths = select_subject_paths(family, subject, variant)
    todo_hits = static_todo_hits(list(candidate_paths.values()))

    result: dict[str, object] = {
        "family": family.name,
        "suite": suite,
        "variant": variant,
        "subject": subject,
        "static_ok": 1 if todo_hits == 0 else 0,
        "compile_ok": 0,
        "games_matched": 0,
        "games_total": expected_games,
        "score": 0.0,
    }
    if todo_hits:
        result["todo_hits"] = todo_hits
        return result

    with tempfile.TemporaryDirectory(prefix=f"setbench-{family.name}-{suite}-") as tmp:
        temp_root = Path(tmp)
        app_dir, tests_dir = build_workspace(family, temp_root)
        gold_fixture = tests_dir / gold_fixture_rel
        gold_fixture.parent.mkdir(parents=True, exist_ok=True)

        host_file = family.graded_files[family.eval_host_file]
        host_target = app_dir / host_file.workspace_path

        for spec in family.graded_files.values():
            copy_tree(spec.gold, app_dir / spec.workspace_path)
        install_eval_source(host_target, host_file.gold, eval_runtime, app_dir, tests_dir)

        env = cargo_env(TARGET_ROOT / f"{family.name}-{suite}-{variant}-{subject}")
        env["SETBENCH_GOLD_OUTPUT"] = str(gold_fixture)
        gold = run(
            [
                "cargo",
                "test",
                "--package",
                "tcg_expansions",
                "setbench_eventlog_write_gold_fixture",
                "--",
                "--ignored",
                "--nocapture",
            ],
            cwd=app_dir,
            env=env,
        )
        result["gold_stdout"] = gold.stdout[-4000:]
        result["gold_stderr"] = gold.stderr[-4000:]
        if gold.returncode != 0 or not gold_fixture.exists():
            result["gold_ok"] = 0
            return result
        result["gold_ok"] = 1

        for name, spec in family.graded_files.items():
            copy_tree(candidate_paths[name], app_dir / spec.workspace_path)
        install_eval_source(host_target, candidate_paths[family.eval_host_file], eval_runtime, app_dir, tests_dir)

        compile_env = cargo_env(Path(env["CARGO_TARGET_DIR"]))
        compile_proc = run(
            ["cargo", "check", "--package", "tcg_expansions"],
            cwd=app_dir,
            env=compile_env,
        )
        result["compile_stdout"] = compile_proc.stdout[-4000:]
        result["compile_stderr"] = compile_proc.stderr[-4000:]
        if compile_proc.returncode != 0:
            return result
        result["compile_ok"] = 1

        eval_env = cargo_env(Path(env["CARGO_TARGET_DIR"]))
        eval_env["SETBENCH_GOLD_PATH"] = str(gold_fixture)
        suite_proc = run(
            [
                "cargo",
                "test",
                "--package",
                "tcg_expansions",
                str(test_filter),
                "--",
                "--test-threads=1",
                "--nocapture",
            ],
            cwd=app_dir,
            env=eval_env,
        )
        result["suite_stdout"] = suite_proc.stdout[-4000:]
        result["suite_stderr"] = suite_proc.stderr[-4000:]
        matched, total = extract_summary(suite_proc.stdout + "\n" + suite_proc.stderr)
        if suite == "train" and not total and suite_proc.returncode == 0:
            matched = expected_games
            total = expected_games
        if total:
            result["games_total"] = total
        result["games_matched"] = matched
        if result["games_total"]:
            result["score"] = round(matched / int(result["games_total"]), 4)
        return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--expansion", default="crystal_guardians")
    parser.add_argument("--variant", default="0pct")
    parser.add_argument("--subject", choices=["gold", "variant"], default="gold")
    parser.add_argument("--suite", choices=["train", "hidden"], default="train")
    parser.add_argument("--output-root", type=Path, required=True)
    args = parser.parse_args()
    family = load_family(args.expansion)
    result = run_suite(
        family,
        variant=args.variant,
        subject=args.subject,
        suite=args.suite,
    )
    result.update(
        {
            "schema_version": "cardbench.set-engine.result.v1",
            "task_id": "cardbench/pokemon/set_engine",
            "expansion_id": family.name,
            "suite_id": f"pokemon-{family.name}-{args.variant}-{args.suite}-v1",
            "suite_sha256": hashlib.sha256(
                (family.root / "family.toml").read_bytes()
            ).hexdigest(),
            "score_metric": "canonical_eventlog_parity",
            "passed": bool(
                result.get("compile_ok")
                and result.get("games_total")
                and result.get("games_matched") == result.get("games_total")
            ),
            "harbor_reward": float(result.get("score", 0.0)),
            "authority": (
                "public_train_reference"
                if args.suite == "train"
                else "legacy_public_fixture_not_sealed"
            ),
        }
    )
    args.output_root.mkdir(parents=True, exist_ok=True)
    (args.output_root / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    (args.output_root / "reward.txt").write_text(
        f"{float(result.get('score', 0.0)):.4f}\n"
    )
    print(json.dumps(result, indent=2))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
