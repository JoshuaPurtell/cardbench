#!/usr/bin/env python3
"""Fail-closed validation for CardBench's versioned Pokemon engine lanes."""

from __future__ import annotations

import argparse
import subprocess
import sys
import tomllib
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
PINS = ROOT / "engine_pins.toml"
COMPONENT_KEYS = {
    "tcg_core_tree": "tcg_core",
    "tcg_rules_ex_tree": "tcg_rules_ex",
    "scaffold_tree": "scaffold",
}


def fail(message: str) -> None:
    raise ValueError(message)


def git(*args: str, cwd: Path) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=cwd,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if result.returncode != 0:
        fail(result.stderr.strip() or f"git {' '.join(args)} failed")
    return result.stdout.strip()


def load_lanes() -> list[dict[str, Any]]:
    data = tomllib.loads(PINS.read_text(encoding="utf-8"))
    lanes = data.get("pokemon_engine_lane")
    if not isinstance(lanes, list) or not lanes:
        fail("engine_pins.toml has no pokemon_engine_lane entries")

    seen: set[str] = set()
    for lane in lanes:
        if not isinstance(lane, dict):
            fail("pokemon_engine_lane entries must be tables")
        lane_id = lane.get("id")
        if not isinstance(lane_id, str) or not lane_id:
            fail("each Pokemon engine lane needs a non-empty id")
        if lane_id in seen:
            fail(f"duplicate Pokemon engine lane id: {lane_id}")
        seen.add(lane_id)
        commit = lane.get("commit")
        if not isinstance(commit, str) or len(commit) != 40 or any(
            char not in "0123456789abcdef" for char in commit
        ):
            fail(f"{lane_id}: commit must be a full lowercase SHA-1")
        for key in COMPONENT_KEYS:
            value = lane.get(key)
            if value is not None and (
                not isinstance(value, str)
                or len(value) != 40
                or any(char not in "0123456789abcdef" for char in value)
            ):
                fail(f"{lane_id}: {key} must be a full lowercase Git tree id")
    return lanes


def require_lane(lanes: list[dict[str, Any]], lane_id: str) -> dict[str, Any]:
    matches = [lane for lane in lanes if lane["id"] == lane_id]
    if len(matches) != 1:
        fail(f"unknown Pokemon engine lane: {lane_id}")
    return matches[0]


def validate_source(lane: dict[str, Any], checkout: Path) -> None:
    if not checkout.is_dir():
        fail(f"source checkout does not exist: {checkout}")
    head = git("rev-parse", "HEAD", cwd=checkout)
    if head != lane["commit"]:
        fail(f"{lane['id']}: checkout HEAD is {head}, expected {lane['commit']}")
    if git("status", "--porcelain", "--untracked-files=no", cwd=checkout):
        fail(f"{lane['id']}: source checkout has tracked modifications")
    for key, component in COMPONENT_KEYS.items():
        expected = lane.get(key)
        if expected is None:
            continue
        actual = git("rev-parse", f"HEAD:{component}", cwd=checkout)
        if actual != expected:
            fail(f"{lane['id']}: {component} tree is {actual}, expected {expected}")


def validate_vendored(lane: dict[str, Any]) -> None:
    relative = lane.get("vendored_path")
    if relative is None:
        return
    if not isinstance(relative, str) or not relative:
        fail(f"{lane['id']}: vendored_path must be a non-empty relative path")
    path = ROOT / relative
    if not path.is_dir():
        fail(f"{lane['id']}: missing vendored path {relative}")
    for key, component in COMPONENT_KEYS.items():
        expected = lane.get(key)
        component_path = path / component
        if expected is None or not component_path.is_dir():
            continue
        actual = git("rev-parse", f"HEAD:{relative}/{component}", cwd=ROOT)
        if actual != expected:
            fail(f"{lane['id']}: vendored {component} tree is {actual}, expected {expected}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--lane", help="lane id to validate against --source-checkout")
    parser.add_argument("--source-checkout", type=Path)
    args = parser.parse_args()
    if bool(args.lane) != bool(args.source_checkout):
        parser.error("--lane and --source-checkout must be supplied together")

    try:
        lanes = load_lanes()
        for lane in lanes:
            validate_vendored(lane)
        if args.lane:
            validate_source(require_lane(lanes, args.lane), args.source_checkout.resolve())
    except ValueError as exc:
        print(f"engine lane validation failed: {exc}", file=sys.stderr)
        return 1

    print(f"validated {len(lanes)} Pokemon engine lanes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
