#!/usr/bin/env python3
"""Validate CardBench's benchmark, variety, expansion, and suite manifests."""

from __future__ import annotations

import json
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def load_toml(path: Path) -> dict:
    if not path.is_file():
        raise ValueError(f"missing manifest: {path.relative_to(ROOT)}")
    return tomllib.loads(path.read_text())


def contained(base: Path, relative: str) -> Path:
    resolved = (base / relative).resolve()
    if not resolved.is_relative_to(base.resolve()):
        raise ValueError(f"path escapes manifest root: {relative}")
    return resolved


def main() -> int:
    benchmark = load_toml(ROOT / "benchmark.toml")
    if benchmark["schema_version"] != "cardbench.benchmark.v1":
        raise ValueError("unsupported benchmark schema")
    families = benchmark["families"]
    if len(families) != len(set(families)):
        raise ValueError("duplicate benchmark family")

    variety_ids: set[str] = set()
    expansion_ids: set[tuple[str, str]] = set()
    for entry in benchmark["varieties"]:
        variety_id = entry["id"]
        if variety_id in variety_ids:
            raise ValueError(f"duplicate variety: {variety_id}")
        variety_ids.add(variety_id)
        manifest_path = contained(ROOT, entry["manifest"])
        variety = load_toml(manifest_path)
        if variety["schema_version"] != "cardbench.variety.v1":
            raise ValueError(f"unsupported variety schema: {manifest_path}")
        if variety["id"] != variety_id:
            raise ValueError(f"variety id mismatch: {variety_id}")
        for expansion in variety.get("expansions", []):
            key = (variety_id, expansion["id"])
            if key in expansion_ids:
                raise ValueError(f"duplicate expansion: {key}")
            expansion_ids.add(key)
            target = contained(manifest_path.parent, expansion["manifest"])
            loaded = load_toml(target)
            if loaded["expansion"]["id"] != expansion["id"]:
                raise ValueError(f"expansion id mismatch: {target}")

    catalog_path = ROOT / "varieties" / "pokemon" / "cards" / "catalog.json"
    catalog = json.loads(catalog_path.read_text())
    if catalog["schema_version"] != "cardbench.card-suite.v1":
        raise ValueError("unsupported card suite schema")
    seen_instances: set[str] = set()
    for instance in catalog["instances"]:
        instance_id = instance["instance_id"]
        if instance_id in seen_instances:
            raise ValueError(f"duplicate card instance: {instance_id}")
        seen_instances.add(instance_id)
        contained(ROOT / "varieties" / "pokemon" / "engine", instance["target"])
        for key in ("stub_sha256", "sealed_tests_sha256", "sealed_gold_sha256"):
            value = instance.get(key)
            if value is not None and (
                len(value) != 64 or any(ch not in "0123456789abcdef" for ch in value)
            ):
                raise ValueError(f"invalid {key} for {instance_id}")

    print(
        f"validated {len(variety_ids)} varieties, "
        f"{len(expansion_ids)} expansion manifests, "
        f"{len(seen_instances)} card instances"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
