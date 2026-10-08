#!/usr/bin/env python3
"""Create and verify the public Leashling candidate workspace."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
from pathlib import Path

from rav_card_task_workspace import ignore_magic_task_workspace, stub_trusted_authority_bins

MAGIC = Path(__file__).resolve().parents[1]
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/leashling.rs")
STUB = '''//! Candidate-owned RAV Leashling definition and activated cost.

use cardbench_magic_engine::{ActivatedAbility, ActivatedAbilityBinding, ActivatedAbilityCostBinding, CardDefinition, CardType, GeneralizedActivatedAbilityCost, ManaCost};
use std::collections::BTreeSet;

#[must_use]
pub(crate) fn definition() -> CardDefinition {
    CardDefinition { id: "RAV-LEASHLING", name: "Leashling", set_code: "RAV", mana_cost: ManaCost::new(6), colors: BTreeSet::new(), mana_colors: BTreeSet::new(), card_types: BTreeSet::from([CardType::Artifact, CardType::Creature]), is_basic_land: false, supported_rules: &["full-rules-fidelity"], power: Some(0), toughness: Some(3), keywords: vec![], effects: vec![] }
}

#[must_use]
pub(crate) fn activated_ability() -> ActivatedAbilityBinding {
    ActivatedAbilityBinding { card_definition: "RAV-LEASHLING", ability: ActivatedAbility { id: "hand-card-library-top-return-source", mana_cost: ManaCost::new(0), tap_cost: false, sorcery_speed: false, additional_tap_creatures: 0, sacrifice_source: false, sacrifice_creatures: 0, sacrifice_lands: 0, discard_cards: 0, targets: vec![], effects: vec![] } }
}

#[must_use]
pub(crate) fn activated_ability_cost() -> ActivatedAbilityCostBinding {
    ActivatedAbilityCostBinding { card_definition: "RAV-LEASHLING", ability_id: "hand-card-library-top-return-source", cost: GeneralizedActivatedAbilityCost::default() }
}
'''


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def tree_manifest(root: Path) -> dict[str, str]:
    for path in root.rglob("*"):
        relative = path.relative_to(root)
        if path.is_symlink():
            raise ValueError(f"workspace contains a symbolic link: {relative}")
        if any(part in {".git", ".private", ".sealed"} for part in relative.parts):
            raise ValueError(f"private authority leaked into workspace: {relative}")
    return {
        path.relative_to(root).as_posix(): sha256(path.read_bytes())
        for path in sorted(path for path in root.rglob("*") if path.is_file())
        if path.relative_to(root).parts[0] not in {".codex", "logs"}
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
    hashes = tree_manifest(workspace)
    if hashes.get(TARGET.as_posix()) != sha256(STUB.encode()):
        raise RuntimeError("candidate stub was not written at the declared Leashling target")
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
    target_rel = Path(manifest["target_file"])
    if set(manifest["allowed_modified_files"]) != {TARGET.as_posix()} or target_rel != TARGET:
        raise ValueError("manifest must allow only the declared Leashling source file")
    target = workspace / target_rel
    if target.is_symlink() or not target.is_file() or target.stat().st_size > 64 * 1024:
        raise ValueError("candidate Leashling source must be a regular file under 64 KiB")
    expected: dict[str, str] = manifest["files"]
    actual = tree_manifest(workspace)
    unexpected = sorted(set(actual) - set(expected))
    missing = sorted(set(expected) - set(actual))
    changed = sorted(name for name in set(actual) & set(expected) if name != TARGET.as_posix() and actual[name] != expected[name])
    if unexpected or missing or changed:
        raise ValueError("candidate changed files outside Leashling module: " + json.dumps({"added": unexpected[:10], "removed": missing[:10], "changed": changed[:10]}))


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
