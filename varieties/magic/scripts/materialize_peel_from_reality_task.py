#!/usr/bin/env python3
"""Materialize and verify the public Peel from Reality task workspace."""
from __future__ import annotations

import argparse
import hashlib
import json
import shutil
from pathlib import Path

from rav_card_task_workspace import ignore_magic_task_workspace, stub_trusted_authority_bins

REPO = Path(__file__).resolve().parents[3]
MAGIC = REPO / "varieties" / "magic"
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/peel_from_reality.rs")
STUB = '''//! Compile-valid but semantically wrong RAV Peel from Reality starter.
use std::collections::BTreeSet;
use cardbench_magic_engine::{CardDefinition, CardType, Color, Effect, ManaCost};
use crate::{SET_CODE, colors, types};

pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-PEEL-FROM-REALITY",
        name: "Peel from Reality",
        set_code: SET_CODE,
        mana_cost: ManaCost::with_colors(0, [Color::Blue]),
        colors: colors([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: types([CardType::Instant]),
        is_basic_land: false,
        supported_rules: &["full-rules-fidelity", "controlled-creature-target"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::ReturnControlledCreatureToHand],
    }
}
'''


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def tree_manifest(root: Path) -> dict[str, str]:
    files: dict[str, str] = {}
    for path in sorted(root.rglob("*")):
        relative = path.relative_to(root)
        if path.is_symlink():
            raise ValueError(f"workspace contains a symbolic link: {relative}")
        if any(part in {".git", ".private", ".sealed"} for part in relative.parts):
            raise ValueError(f"private authority leaked into workspace: {relative}")
        if path.is_file() and relative.parts[0] not in {".codex", "logs"}:
            files[relative.as_posix()] = sha256(path.read_bytes())
    return files


def materialize(workspace: Path, manifest_path: Path) -> None:
    if workspace.exists():
        shutil.rmtree(workspace)
    workspace.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(MAGIC, workspace / "varieties" / "magic", ignore=ignore_magic_task_workspace)
    stub_trusted_authority_bins(workspace)
    target = workspace / TARGET
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(STUB, encoding="utf-8")
    hashes = tree_manifest(workspace)
    if hashes.get(TARGET.as_posix()) != sha256(STUB.encode()):
        raise RuntimeError("compile-valid Peel from Reality starter was not written")
    manifest = {
        "schema_version": "cardbench.magic.task_workspace.v1",
        "task_id": "cardbench/magic/card",
        "target_file": TARGET.as_posix(),
        "allowed_modified_files": [TARGET.as_posix()],
        "files": hashes,
        "workspace_sha256": sha256("".join(f"{name}\0{digest}\n" for name, digest in hashes.items()).encode()),
    }
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")


def verify(workspace: Path, manifest_path: Path) -> None:
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if set(manifest["allowed_modified_files"]) != {TARGET.as_posix()} or Path(manifest["target_file"]) != TARGET:
        raise ValueError("manifest must allow only the declared Peel from Reality module")
    target = workspace / TARGET
    if target.is_symlink() or not target.is_file() or target.stat().st_size > 64 * 1024:
        raise ValueError("candidate Peel from Reality source must be a regular file no larger than 64 KiB")
    expected: dict[str, str] = manifest["files"]
    actual = tree_manifest(workspace)
    unexpected = sorted(set(actual) - set(expected))
    missing = sorted(set(expected) - set(actual))
    changed = sorted(name for name in set(actual) & set(expected) if name != TARGET.as_posix() and actual[name] != expected[name])
    if unexpected or missing or changed:
        raise ValueError("candidate changed files outside the Peel from Reality module: " + json.dumps({"added": unexpected[:10], "removed": missing[:10], "changed": changed[:10]}))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--workspace", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--verify", action="store_true")
    args = parser.parse_args()
    if args.verify:
        verify(args.workspace.resolve(), args.manifest.resolve())
    else:
        materialize(args.workspace.resolve(), args.manifest.resolve())
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
