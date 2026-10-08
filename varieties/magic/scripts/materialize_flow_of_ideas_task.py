#!/usr/bin/env python3
"""Create and verify the isolated public Flow of Ideas candidate workspace."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
from pathlib import Path

from rav_card_task_workspace import ignore_magic_task_workspace, stub_trusted_authority_bins

REPO = Path(__file__).resolve().parents[3]
MAGIC = REPO / "varieties" / "magic"
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/flow_of_ideas.rs")
STUB = '''//! Candidate implementation for the RAV Flow of Ideas diagnostic.

use std::collections::BTreeSet;

use cardbench_magic_engine::{BasicLandType, CardDefinition, CardType, Color, Effect, ManaCost};

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-FLOW-OF-IDEAS",
        name: "Flow of Ideas",
        set_code: "RAV",
        mana_cost: ManaCost::with_colors(5, [Color::Blue]),
        colors: BTreeSet::from([Color::Blue]),
        mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Sorcery]),
        is_basic_land: false,
        supported_rules: &["full-rules-fidelity", "draw-for-each-controlled-island"],
        power: None,
        toughness: None,
        keywords: vec![],
        effects: vec![Effect::DrawControllerForEachControlledBasicLandType {
            land_type: BasicLandType::Forest,
        }],
    }
}
'''


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def tree_manifest(root: Path) -> dict[str, str]:
    for path in root.rglob("*"):
        if path.is_symlink():
            raise ValueError(f"workspace contains a symbolic link: {path.relative_to(root)}")
        if any(part in {".git", ".private", ".sealed"} for part in path.relative_to(root).parts):
            raise ValueError(f"private authority leaked into workspace: {path.relative_to(root)}")
    return {
        path.relative_to(root).as_posix(): sha256(path)
        for path in sorted(root.rglob("*"))
        if path.is_file() and path.relative_to(root).parts[0] not in {".codex", "logs"}
    }


def ignore(relative_path: str, names: list[str]) -> set[str]:
    return ignore_magic_task_workspace(relative_path, names)


def materialize(workspace: Path, manifest_path: Path) -> None:
    if workspace.exists():
        shutil.rmtree(workspace)
    workspace.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(MAGIC, workspace / "varieties" / "magic", ignore=ignore)
    stub_trusted_authority_bins(workspace)
    target = workspace / TARGET
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(STUB, encoding="utf-8")

    hashes = tree_manifest(workspace)
    if hashes.get(TARGET.as_posix()) != hashlib.sha256(STUB.encode()).hexdigest():
        raise RuntimeError("candidate stub was not written at the declared target")
    manifest = {
        "schema_version": "cardbench.magic.task_workspace.v1",
        "task_id": "cardbench/magic/card",
        "target_file": TARGET.as_posix(),
        "allowed_modified_files": [TARGET.as_posix()],
        "files": hashes,
        "workspace_sha256": hashlib.sha256(
            "".join(f"{name}\0{digest}\n" for name, digest in hashes.items()).encode()
        ).hexdigest(),
    }
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")


def verify(workspace: Path, manifest_path: Path) -> None:
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    target_rel = Path(manifest["target_file"])
    if set(manifest["allowed_modified_files"]) != {TARGET.as_posix()} or target_rel != TARGET:
        raise ValueError("manifest must allow only the Flow of Ideas candidate source")
    target = workspace / target_rel
    if target.is_symlink() or not target.is_file():
        raise ValueError("candidate Flow of Ideas source must be a regular file")
    if target.stat().st_size > 64 * 1024:
        raise ValueError("candidate Flow of Ideas source exceeds 64 KiB")
    expected: dict[str, str] = manifest["files"]
    actual = tree_manifest(workspace)
    unexpected = sorted(set(actual) - set(expected))
    missing = sorted(set(expected) - set(actual))
    changed = sorted(
        name
        for name in set(actual) & set(expected)
        if name != TARGET.as_posix() and actual[name] != expected[name]
    )
    if unexpected or missing or changed:
        raise ValueError(
            "candidate changed files outside the Flow of Ideas module: "
            + json.dumps({"added": unexpected[:10], "removed": missing[:10], "changed": changed[:10]})
        )


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
