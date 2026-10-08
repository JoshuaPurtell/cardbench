#!/usr/bin/env python3
"""Materialize and verify the public Shambling Shell single-card workspace."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
from pathlib import Path

from rav_card_task_workspace import ignore_magic_task_workspace, stub_trusted_authority_bins

REPO = Path(__file__).resolve().parents[3]
MAGIC = REPO / "varieties" / "magic"
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/shambling_shell.rs")
STUB = '''use cardbench_magic_engine::{ActivatedAbility, ActivatedAbilityBinding, CardDefinition, CardType, Color, Effect, Keyword, ManaCost, TargetRequirement};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition {
        id: "RAV-SHAMBLING-SHELL", name: "Shambling Shell", set_code: "RAV",
        mana_cost: ManaCost::with_colors(2, [Color::Green]),
        colors: BTreeSet::from([Color::Green]), mana_colors: BTreeSet::new(),
        card_types: BTreeSet::from([CardType::Creature]), is_basic_land: false,
        supported_rules: &["full-rules-fidelity", "dredge", "base-characteristics"],
        power: Some(3), toughness: Some(1), keywords: vec![Keyword::Dredge(2)], effects: vec![],
    }
}

#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding {
        card_definition: "RAV-SHAMBLING-SHELL",
        ability: ActivatedAbility {
            id: "shambling-shell-sacrifice-counter", mana_cost: ManaCost::new(0),
            tap_cost: false, sorcery_speed: false, additional_tap_creatures: 0,
            sacrifice_source: false, sacrifice_creatures: 0, sacrifice_lands: 0,
            discard_cards: 0, targets: vec![TargetRequirement::Creature],
            effects: vec![Effect::DrawController],
        },
    }
}
'''


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def tree_manifest(root: Path) -> dict[str, str]:
    for path in root.rglob("*"):
        relative = path.relative_to(root)
        if path.is_symlink():
            raise ValueError(f"workspace contains a symbolic link: {relative}")
        if any(part in {".git", ".private", ".sealed"} for part in relative.parts):
            raise ValueError(f"private authority leaked into workspace: {relative}")
    return {
        path.relative_to(root).as_posix(): sha256(path)
        for path in sorted(root.rglob("*"))
        if path.is_file() and path.relative_to(root).parts[0] not in {".codex", "logs"}
    }


def materialize(workspace: Path, manifest_path: Path) -> None:
    if workspace.exists():
        shutil.rmtree(workspace)
    workspace.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(MAGIC, workspace / "varieties" / "magic", ignore=ignore_magic_task_workspace)
    stub_trusted_authority_bins(workspace)
    target = workspace / TARGET
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(STUB, encoding="utf-8")
    files = tree_manifest(workspace)
    if files.get(TARGET.as_posix()) != hashlib.sha256(STUB.encode()).hexdigest():
        raise RuntimeError("compile-valid Shambling Shell stub was not written at the target")
    manifest = {
        "schema_version": "cardbench.magic.task_workspace.v1",
        "task_id": "cardbench/magic/card",
        "target_file": TARGET.as_posix(),
        "allowed_modified_files": [TARGET.as_posix()],
        "files": files,
        "workspace_sha256": hashlib.sha256(
            "".join(name + chr(0) + digest + chr(10) for name, digest in files.items()).encode()
        ).hexdigest(),
    }
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")


def verify(workspace: Path, manifest_path: Path) -> None:
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if set(manifest["allowed_modified_files"]) != {TARGET.as_posix()} or Path(manifest["target_file"]) != TARGET:
        raise ValueError("manifest must allow only the declared Shambling Shell module")
    target = workspace / TARGET
    if target.is_symlink() or not target.is_file() or target.stat().st_size > 64 * 1024:
        raise ValueError("candidate Shambling Shell module must be a regular file no larger than 64 KiB")
    expected: dict[str, str] = manifest["files"]
    actual = tree_manifest(workspace)
    added = sorted(set(actual) - set(expected))
    removed = sorted(set(expected) - set(actual))
    changed = sorted(name for name in set(actual) & set(expected) if name != TARGET.as_posix() and actual[name] != expected[name])
    if added or removed or changed:
        raise ValueError("candidate changed files outside Shambling Shell module: " + json.dumps({"added": added[:10], "removed": removed[:10], "changed": changed[:10]}))


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
