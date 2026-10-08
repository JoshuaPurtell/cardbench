#!/usr/bin/env python3
"""Materialize and verify the public Boros Signet task workspace."""

from __future__ import annotations

import argparse
import hashlib
import json
import shutil
from pathlib import Path

from rav_card_task_workspace import ignore_magic_task_workspace, stub_trusted_authority_bins

REPO = Path(__file__).resolve().parents[3]
MAGIC = REPO / "varieties" / "magic"
TARGET = Path("varieties/magic/sets/ravnica_city_of_guilds/src/cards/boros_signet.rs")
STUB = '//! Compile-valid but semantically wrong starter stub for RAV Boros Signet.\n\nuse cardbench_magic_engine::{ActivatedManaAbility, CardDefinition, CardType, Color, ManaAbilityBinding, ManaAbilityOutput, ManaBundle, ManaCost};\nuse std::collections::BTreeSet;\n\n#[must_use]\npub(crate) fn definition() -> CardDefinition {\n    CardDefinition {\n        id: "RAV-BOROS-SIGNET",\n        name: "Boros Signet",\n        set_code: "RAV",\n        mana_cost: ManaCost::new(2),\n        colors: BTreeSet::new(),\n        mana_colors: BTreeSet::new(),\n        card_types: BTreeSet::from([CardType::Artifact]),\n        is_basic_land: false,\n        supported_rules: &["full-rules-fidelity", "artifact-casting", "paid-fixed-two-color-mana-ability", "cast-payment-mana-activation"],\n        power: None,\n        toughness: None,\n        keywords: vec![],\n        effects: vec![],\n    }\n}\n\n#[must_use]\npub(crate) fn mana_ability() -> ManaAbilityBinding {\n    ManaAbilityBinding {\n        card_definition: "RAV-BOROS-SIGNET",\n        ability: ActivatedManaAbility {\n            id: "boros-signet-wr",\n            tap_cost: true,\n            output: ManaAbilityOutput::PaidBundle {\n                mana_cost: ManaCost::new(1),\n                bundle: ManaBundle::new([(Color::White, 1), (Color::Green, 1)]),\n            },\n            amount: 0,\n            life_payment: None,\n            controller_damage: None,\n        },\n    }\n}\n'


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def tree_manifest(root: Path) -> dict[str, str]:
    for path in root.rglob("*"):
        relative = path.relative_to(root)
        if path.is_symlink():
            raise ValueError(f"workspace contains a symbolic link: {relative}")
        if any(part in {".git", ".private", ".sealed"} for part in relative.parts):
            raise ValueError(f"private authority leaked into workspace: {relative}")
    return {path.relative_to(root).as_posix(): sha256(path) for path in sorted(root.rglob("*")) if path.is_file() and path.relative_to(root).parts[0] not in {".codex", "logs"}}


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
    if hashes.get(TARGET.as_posix()) != hashlib.sha256(STUB.encode()).hexdigest():
        raise RuntimeError("compile-valid Boros Signet stub was not written at the target")
    manifest = {
        "schema_version": "cardbench.magic.task_workspace.v1",
        "task_id": "cardbench/magic/card",
        "target_file": TARGET.as_posix(),
        "allowed_modified_files": [TARGET.as_posix()],
        "files": hashes,
        "workspace_sha256": hashlib.sha256("".join(f"{name}\0{digest}\n" for name, digest in hashes.items()).encode()).hexdigest(),
    }
    manifest_path.parent.mkdir(parents=True, exist_ok=True)
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")


def verify(workspace: Path, manifest_path: Path) -> None:
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if set(manifest["allowed_modified_files"]) != {TARGET.as_posix()} or Path(manifest["target_file"]) != TARGET:
        raise ValueError("manifest must allow only the declared Boros Signet module")
    target = workspace / TARGET
    if target.is_symlink() or not target.is_file():
        raise ValueError("candidate Boros Signet source must be a regular file")
    if target.stat().st_size > 64 * 1024:
        raise ValueError("candidate Boros Signet source exceeds 64 KiB")
    expected: dict[str, str] = manifest["files"]
    actual = tree_manifest(workspace)
    unexpected = sorted(set(actual) - set(expected))
    missing = sorted(set(expected) - set(actual))
    changed = sorted(name for name in set(actual) & set(expected) if name != TARGET.as_posix() and actual[name] != expected[name])
    if unexpected or missing or changed:
        raise ValueError("candidate changed files outside the Boros Signet module: " + json.dumps({"added": unexpected[:10], "removed": missing[:10], "changed": changed[:10]}))


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
