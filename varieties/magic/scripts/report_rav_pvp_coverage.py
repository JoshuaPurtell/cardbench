#!/usr/bin/env python3
"""Read-only diagnostic ledger for RAV per-card PvP task progress.

This script cross-checks the checked-in RAV inventory against the authoritative
main-set printing catalog, discovers isolated RAV task directories, validates
reviewed-file hashes, and summarizes Harbor receipts. It never runs a verifier,
modifies source data, or treats task/review metadata as card-coverage credit.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import re
import sys
import tomllib
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

SCHEMA = "cardbench.magic.rav-pvp-coverage-diagnostic.v1"
EXPECTED_UNIQUE_CARDS = 291
EXPECTED_PRINTINGS = 306
CATALOG_REL = "varieties/magic/sets/ravnica_city_of_guilds/catalog/rav_main_set.tsv"
INVENTORY_REL = "varieties/magic/tasks/rav-set-pvp/rav-set-pvp-coverage.csv"

# Legacy Char has no harbor.toml yet. Other card tasks bind review scope and
# required files in harbor.toml, matching the Harbor route's review gate.
REQUIRED_REVIEW_FILES: dict[str, set[str]] = {
    "rav-char-pvp": {
        "adapters/harbor/bundles/set_pvp/README.md",
        "adapters/harbor/bundles/set_pvp/instruction.md",
        "adapters/harbor/bundles/set_pvp/tests/test.sh",
        "adapters/harbor/scripts/run_harbor.py",
        "varieties/magic/DATA_PROVENANCE.md",
        "varieties/magic/README.md",
        "varieties/magic/Cargo.lock",
        "varieties/magic/Cargo.toml",
        "varieties/magic/scripts/materialize_char_task.py",
        "varieties/magic/scripts/rav_card_task_workspace.py",
        "varieties/magic/scripts/score_char_pvp.py",
        "varieties/magic/session/Cargo.toml",
        "varieties/magic/session/src/project.rs",
        "varieties/magic/session/src/bin/rav_char_pvp.rs",
        "varieties/magic/engine/src/game.rs",
        "varieties/magic/engine/src/model.rs",
        "varieties/magic/sets/ravnica_city_of_guilds/src/cards/char.rs",
        "varieties/magic/sets/ravnica_city_of_guilds/src/cards/mod.rs",
        "varieties/magic/sets/ravnica_city_of_guilds/src/lib.rs",
        "varieties/magic/sets/ravnica_city_of_guilds/catalog/rav_main_set.tsv",
        "varieties/magic/tasks/rav-set-pvp/RAV_PVP_SOURCE_CONTRACT.md",
        "varieties/magic/tasks/rav-char-pvp/README.md",
        "varieties/magic/tasks/rav-char-pvp/features.toml",
    },
    "rav-lightning-helix-pvp": {
        "adapters/harbor/bundles/magic_card/README.md",
        "adapters/harbor/bundles/magic_card/instruction.md",
        "adapters/harbor/bundles/magic_card/tests/test.sh",
        "adapters/harbor/scripts/run_harbor.py",
        "varieties/magic/DATA_PROVENANCE.md",
        "varieties/magic/README.md",
        "varieties/magic/Cargo.lock",
        "varieties/magic/Cargo.toml",
        "varieties/magic/scripts/materialize_lightning_helix_task.py",
        "varieties/magic/scripts/score_lightning_helix_pvp.py",
        "varieties/magic/session/Cargo.toml",
        "varieties/magic/session/src/bin/rav_lightning_helix_pvp.rs",
        "varieties/magic/sets/ravnica_city_of_guilds/src/cards/lightning_helix.rs",
        "varieties/magic/sets/ravnica_city_of_guilds/src/cards/mod.rs",
        "varieties/magic/sets/ravnica_city_of_guilds/src/lib.rs",
        "varieties/magic/tasks/rav-lightning-helix-pvp/README.md",
        "varieties/magic/tasks/rav-lightning-helix-pvp/features.toml",
    },
}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def safe_repo_file(root: Path, relative: str) -> Path | None:
    """Resolve a declared repo-relative file without following symlink escapes."""
    rel = Path(relative)
    if rel.is_absolute() or ".." in rel.parts or not rel.parts:
        return None
    path = root / rel
    try:
        if path.is_symlink() or not path.is_file():
            return None
        resolved = path.resolve(strict=True)
        if not resolved.is_relative_to(root.resolve()):
            return None
        return resolved
    except (OSError, RuntimeError):
        return None


def load_catalog(root: Path) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    path = root / CATALOG_REL
    rows: list[dict[str, Any]] = []
    errors: list[str] = []
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except OSError as exc:
        return [], {"status": "invalid", "errors": [f"cannot read catalog: {exc}"]}

    for line_number, line in enumerate(lines, 1):
        if not line or line.startswith("#"):
            continue
        fields = line.split("\t")
        if len(fields) < 2:
            errors.append(f"catalog line {line_number}: expected collector number and name")
            continue
        try:
            number = int(fields[0])
        except ValueError:
            errors.append(f"catalog line {line_number}: invalid collector number {fields[0]!r}")
            continue
        name = fields[1].strip()
        if not name:
            errors.append(f"catalog line {line_number}: empty card name")
            continue
        rows.append({"collector_number": number, "card_name": name})

    by_name: dict[str, list[int]] = defaultdict(list)
    for row in rows:
        by_name[row["card_name"]].append(row["collector_number"])
    duplicate_numbers = [
        name for name, numbers in by_name.items() if len(numbers) != len(set(numbers))
    ]
    if duplicate_numbers:
        errors.append(f"duplicate catalog collector number(s) for {len(duplicate_numbers)} names")
    unique = len(by_name)
    printings = len(rows)
    if unique != EXPECTED_UNIQUE_CARDS:
        errors.append(f"catalog has {unique} unique names, expected {EXPECTED_UNIQUE_CARDS}")
    if printings != EXPECTED_PRINTINGS:
        errors.append(f"catalog has {printings} printings, expected {EXPECTED_PRINTINGS}")
    return rows, {
        "status": "valid" if not errors else "invalid",
        "unique_cards": unique,
        "printings": printings,
        "errors": errors,
        "by_name": {name: sorted(numbers) for name, numbers in by_name.items()},
    }


def load_inventory(root: Path) -> tuple[list[dict[str, str]], dict[str, Any]]:
    path = root / INVENTORY_REL
    errors: list[str] = []
    try:
        with path.open("r", encoding="utf-8-sig", newline="") as stream:
            rows = list(csv.DictReader(stream))
    except (OSError, csv.Error) as exc:
        return [], {"status": "invalid", "errors": [f"cannot read inventory: {exc}"]}
    required = {"collector_numbers", "printing_count", "card_name", "definition_id"}
    if not rows:
        errors.append("inventory has no data rows")
    if rows and not required.issubset(rows[0]):
        errors.append(f"inventory is missing columns: {', '.join(sorted(required - set(rows[0])))}")
        return rows, {"status": "invalid", "errors": errors}

    names = [row.get("card_name", "").strip() for row in rows]
    duplicate_names = sorted(name for name, count in Counter(names).items() if count > 1)
    if duplicate_names:
        errors.append(f"inventory has duplicate card name(s): {', '.join(duplicate_names[:5])}")
    total_printings = 0
    for row in rows:
        try:
            count = int(row["printing_count"])
            total_printings += count
        except (KeyError, TypeError, ValueError):
            errors.append(f"invalid printing_count for {row.get('card_name', '<unnamed>')}")
    if len(rows) != EXPECTED_UNIQUE_CARDS:
        errors.append(f"inventory has {len(rows)} rows, expected {EXPECTED_UNIQUE_CARDS}")
    if total_printings != EXPECTED_PRINTINGS:
        errors.append(f"inventory sums to {total_printings} printings, expected {EXPECTED_PRINTINGS}")
    return rows, {
        "status": "valid" if not errors else "invalid",
        "unique_cards": len(rows),
        "printings": total_printings,
        "errors": errors,
    }


def compare_inventory_catalog(
    inventory: list[dict[str, str]], catalog_info: dict[str, Any], inventory_info: dict[str, Any]
) -> dict[str, Any]:
    errors = list(catalog_info.get("errors", [])) + list(inventory_info.get("errors", []))
    authoritative = catalog_info.get("by_name", {})
    inventory_by_name = {row.get("card_name", "").strip(): row for row in inventory}
    catalog_names, inventory_names = set(authoritative), set(inventory_by_name)
    missing = sorted(catalog_names - inventory_names)
    extra = sorted(inventory_names - catalog_names)
    if missing:
        errors.append(f"inventory is missing {len(missing)} catalog card name(s)")
    if extra:
        errors.append(f"inventory has {len(extra)} name(s) absent from catalog")
    mismatch: list[str] = []
    for name in sorted(catalog_names & inventory_names):
        row = inventory_by_name[name]
        numbers = ";".join(str(number) for number in sorted(authoritative[name]))
        try:
            count_ok = int(row.get("printing_count", "")) == len(authoritative[name])
        except ValueError:
            count_ok = False
        if row.get("collector_numbers", "").strip() != numbers or not count_ok:
            mismatch.append(name)
    if mismatch:
        errors.append(f"collector numbers or printing counts differ for {len(mismatch)} card(s)")
    return {
        "status": "valid" if not errors else "invalid",
        "catalog_unique_cards": catalog_info.get("unique_cards", 0),
        "catalog_printings": catalog_info.get("printings", 0),
        "inventory_unique_cards": inventory_info.get("unique_cards", 0),
        "inventory_printings": inventory_info.get("printings", 0),
        "missing_catalog_names": missing,
        "extra_inventory_names": extra,
        "printing_or_number_mismatches": mismatch,
        "errors": errors,
    }


def normalize_identity(value: Any) -> str:
    return re.sub(r"[^a-z0-9]+", "", str(value or "").lower())


def task_features(task_dir: Path) -> tuple[dict[str, Any], str | None]:
    feature_path = task_dir / "features.toml"
    if feature_path.is_symlink() or not feature_path.is_file():
        return {}, "features.toml missing"
    try:
        value = tomllib.loads(feature_path.read_text(encoding="utf-8"))
    except (OSError, tomllib.TOMLDecodeError) as exc:
        return {}, f"features.toml invalid: {exc}"
    return value if isinstance(value, dict) else {}, None


def discover_tasks(root: Path, inventory: list[dict[str, str]]) -> tuple[dict[str, list[dict[str, Any]]], list[dict[str, Any]]]:
    inventory_by_id = {row.get("definition_id", "").strip(): row for row in inventory}
    inventory_by_name = {row.get("card_name", "").strip(): row for row in inventory}
    task_map: dict[str, list[dict[str, Any]]] = defaultdict(list)
    unmatched: list[dict[str, Any]] = []
    tasks_root = root / "varieties/magic/tasks"
    if not tasks_root.is_dir():
        return task_map, [{"task_id": "", "error": "Magic tasks directory missing"}]

    for task_dir in sorted(tasks_root.glob("rav-*-pvp")):
        if task_dir.is_symlink() or not task_dir.is_dir() or task_dir.name == "rav-set-pvp":
            continue
        features, feature_error = task_features(task_dir)
        set_code = str(features.get("set_code", "RAV")).upper()
        if set_code != "RAV":
            continue
        definition_id = str(features.get("definition_id", "")).strip()
        card_name = str(features.get("card_name", features.get("name", ""))).strip()
        row = inventory_by_id.get(definition_id) if definition_id else None
        row = row or inventory_by_name.get(card_name) if card_name else row
        if row is None:
            slug = task_dir.name.removeprefix("rav-").removesuffix("-pvp")
            normalized_slug = normalize_identity(slug)
            row = next(
                (item for name, item in inventory_by_name.items()
                 if normalize_identity(name) == normalized_slug),
                None,
            )
        identity_errors: list[str] = []
        if feature_error:
            identity_errors.append(feature_error)
        if row is not None:
            expected_name = row.get("card_name", "").strip()
            expected_id = row.get("definition_id", "").strip()
            if not definition_id:
                identity_errors.append("feature definition_id is missing")
            if card_name and card_name != expected_name:
                identity_errors.append(
                    f"feature card_name {card_name!r} does not match inventory {expected_name!r}"
                )
            if definition_id and definition_id != expected_id:
                identity_errors.append(
                    f"feature definition_id {definition_id!r} does not match inventory {expected_id!r}"
                )
            raw_collector_number = features.get("collector_number")
            try:
                collector_number = int(raw_collector_number)
            except (TypeError, ValueError):
                collector_number = None
            expected_numbers = {
                int(item)
                for item in row.get("collector_numbers", "").split(";")
                if item.isdigit()
            }
            if collector_number is None:
                identity_errors.append("feature collector_number is missing or invalid")
            elif collector_number not in expected_numbers:
                identity_errors.append(
                    f"feature collector_number {collector_number} is not a printing of {expected_name!r}"
                )
        elif not feature_error:
            identity_errors.append("task identity does not match a catalog inventory card")
        review_path = task_dir / "independent_review.json"
        review_present = review_path.is_file() and not review_path.is_symlink()
        found = {
            "task_id": task_dir.name,
            "path": task_dir.relative_to(root).as_posix(),
            "features_path": (task_dir / "features.toml").relative_to(root).as_posix(),
            "review_path": review_path.relative_to(root).as_posix() if review_present else None,
            "feature_card_name": card_name or None,
            "feature_definition_id": definition_id or None,
            "feature_error": feature_error,
            "task_identity_errors": identity_errors,
            "matched_inventory_card": row.get("card_name") if row else None,
        }
        if row is None:
            unmatched.append(found)
        else:
            task_map[row["definition_id"].strip()].append(found)
    return task_map, unmatched


def review_freshness(root: Path, task: dict[str, Any], card: dict[str, str]) -> dict[str, Any]:
    rel = task.get("review_path")
    if not rel:
        return {"status": "missing", "declared_files": 0, "current_files": 0, "stale_files": [], "missing_files": []}
    path = safe_repo_file(root, rel)
    if path is None:
        return {"status": "invalid_review_path", "declared_files": 0, "current_files": 0, "stale_files": [], "missing_files": [rel]}
    try:
        evidence = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        return {"status": "invalid_review_json", "error": str(exc), "declared_files": 0, "current_files": 0, "stale_files": [], "missing_files": []}
    if not isinstance(evidence, dict) or evidence.get("schema_version") != "cardbench.magic.independent-review.v1" or evidence.get("status") != "passed":
        return {"status": "not_passed_or_unknown_schema", "declared_files": 0, "current_files": 0, "stale_files": [], "missing_files": []}
    reviewed = evidence.get("reviewed_files")
    if not isinstance(reviewed, dict) or not reviewed:
        return {"status": "missing_reviewed_files", "declared_files": 0, "current_files": 0, "stale_files": [], "missing_files": []}

    current: list[str] = []
    stale: list[str] = []
    missing: list[str] = []
    invalid: list[str] = []
    for relative, expected in reviewed.items():
        if not isinstance(relative, str) or not isinstance(expected, str) or not re.fullmatch(r"[0-9a-fA-F]{64}", expected):
            invalid.append(str(relative))
            continue
        reviewed_path = safe_repo_file(root, relative)
        if reviewed_path is None:
            missing.append(relative)
            continue
        try:
            actual = sha256_file(reviewed_path)
        except OSError:
            missing.append(relative)
            continue
        (current if actual.lower() == expected.lower() else stale).append(relative)

    task_dir = root / str(task.get("path", ""))
    manifest_path = task_dir / "harbor.toml"
    manifest_error: str | None = None
    required = REQUIRED_REVIEW_FILES.get(task["task_id"])
    configured_scope: str | None = None
    if manifest_path.is_symlink():
        manifest_error = "harbor.toml is a symbolic link"
    elif manifest_path.is_file():
        try:
            manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
            configured_files = manifest.get("review_files")
            configured_scope = manifest.get("review_scope")
            if (
                isinstance(configured_files, list)
                and configured_files
                and all(isinstance(item, str) and item for item in configured_files)
                and len(set(configured_files)) == len(configured_files)
                and isinstance(configured_scope, str)
                and configured_scope.strip()
            ):
                required = set(configured_files)
            else:
                manifest_error = "harbor.toml has invalid review_files or review_scope"
        except (OSError, tomllib.TOMLDecodeError, TypeError, ValueError) as exc:
            manifest_error = f"harbor.toml cannot be read: {exc}"
    scope = evidence.get("review_scope")
    scope_text = str(scope or "").lower()
    task_id = task["task_id"]
    if task_id == "rav-char-pvp":
        scope_ok = "char" in scope_text
    elif task_id == "rav-lightning-helix-pvp":
        scope_ok = "lightning helix" in scope_text
    else:
        scope_ok = normalize_identity(card.get("card_name")) in normalize_identity(scope_text) or normalize_identity(task_id) in normalize_identity(scope_text)
    if configured_scope is not None:
        scope_ok = scope_ok and configured_scope.lower() in scope_text
    absent_required = sorted(required - set(reviewed)) if required else []
    if stale:
        status = "stale_hashes"
    elif missing or invalid or not scope_ok or manifest_error:
        status = "invalid_or_incomplete"
    elif required and absent_required:
        status = "missing_required_scope_files"
    elif required:
        status = "current_required_scope_hashes"
    else:
        status = "declared_hashes_current_scope_unverified"
    return {
        "status": status,
        "schema_valid": True,
        "review_status_passed": True,
        "scope_matches_card": scope_ok,
        "known_required_scope": required is not None,
        "manifest_error": manifest_error,
        "configured_review_scope": configured_scope,
        "declared_files": len(reviewed),
        "current_files": len(current),
        "stale_files": sorted(stale),
        "missing_files": sorted(missing),
        "invalid_entries": sorted(invalid),
        "missing_required_files": absent_required,
        "reviewer": evidence.get("reviewer"),
        "review_scope": scope,
    }


def read_receipts(root: Path) -> list[tuple[Path, dict[str, Any]]]:
    harbor_root = root / "artifacts/harbor"
    if not harbor_root.is_dir():
        return []
    receipts: list[tuple[Path, dict[str, Any]]] = []
    for path in harbor_root.rglob("lane-receipt.json"):
        if path.is_symlink() or not path.is_file():
            continue
        try:
            resolved = path.resolve(strict=True)
            if not resolved.is_relative_to(root.resolve()):
                continue
            value = json.loads(resolved.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError, RuntimeError):
            continue
        if isinstance(value, dict) and value.get("schema_version") == "cardbench.harbor.lane_receipt.v1" and value.get("variety") == "magic":
            receipts.append((resolved, value))
    return receipts


def receipt_matches_card(receipt: dict[str, Any], card: dict[str, str]) -> bool:
    verifier = receipt.get("verifier")
    if not isinstance(verifier, dict):
        return False
    targets = {card.get("definition_id", ""), card.get("card_name", "")}
    targets.discard("")
    objects: list[dict[str, Any]] = []
    for value in (verifier.get("card"), verifier.get("candidate")):
        if isinstance(value, dict):
            objects.append(value)
    for obj in objects:
        values = {str(obj.get(key, "")) for key in ("definition_id", "card_definition", "name")}
        if targets & values:
            return True
    # Set-PvP pilots identify their focused card in the candidate object. For a
    # wider set receipt, only match a per-card item that names this exact card.
    for item in verifier.get("per_card_breakdown", []) if isinstance(verifier.get("per_card_breakdown"), list) else []:
        if not isinstance(item, dict) or not (
            item.get("definition_id") == card.get("definition_id")
            or item.get("name") == card.get("card_name")
        ):
            continue
        # Set-level receipts enumerate every card. Associate one only if this
        # card's own row records a trace pass or a card credit.
        if item.get("credited") is True or item.get("trace_status") in {"passed", "qualified"}:
            return True
    return False


def receipt_identity_result(receipt: dict[str, Any], card: dict[str, str]) -> dict[str, Any]:
    verifier = receipt.get("verifier") if isinstance(receipt.get("verifier"), dict) else {}
    candidate = verifier.get("candidate") if isinstance(verifier.get("candidate"), dict) else {}
    report_card = verifier.get("card") if isinstance(verifier.get("card"), dict) else {}
    direct = (
        report_card.get("definition_id") == card.get("definition_id")
        or report_card.get("name") == card.get("card_name")
        or candidate.get("card_definition") == card.get("definition_id")
    )
    breakdown_credit = False
    breakdown_passed = False
    for item in verifier.get("per_card_breakdown", []) if isinstance(verifier.get("per_card_breakdown"), list) else []:
        if isinstance(item, dict) and (
            item.get("definition_id") == card.get("definition_id")
            or item.get("name") == card.get("card_name")
        ):
            breakdown_credit = breakdown_credit or item.get("credited") is True
            breakdown_passed = breakdown_passed or item.get("trace_status") in {"passed", "qualified"}
    return {"direct_identity": direct, "breakdown_reports_credit": breakdown_credit, "breakdown_trace_passed": breakdown_passed}


def candidate_artifact_hash_matches(receipt_path: Path, receipt: dict[str, Any]) -> tuple[bool, str | None]:
    """Corroborate a candidate receipt's source hash against saved workspace bytes."""
    verifier = receipt.get("verifier") if isinstance(receipt.get("verifier"), dict) else {}
    candidate = verifier.get("candidate") if isinstance(verifier.get("candidate"), dict) else {}
    expected = candidate.get("source_sha256")
    if not isinstance(expected, str) or not re.fullmatch(r"[0-9a-fA-F]{64}", expected):
        return False, None
    out = receipt_path.parent
    materialization = out / "task-materialization.json"
    target_file: str | None = None
    try:
        if materialization.is_file() and not materialization.is_symlink():
            materialized = json.loads(materialization.read_text(encoding="utf-8"))
            raw_target = materialized.get("target_file") if isinstance(materialized, dict) else None
            if isinstance(raw_target, str) and not Path(raw_target).is_absolute() and ".." not in Path(raw_target).parts:
                target_file = raw_target
    except (OSError, json.JSONDecodeError):
        pass

    candidates: list[Path] = []
    if target_file:
        for base in (out / "logs/verifier/candidate-workspace", out / "candidate-workspace", out / "workspace"):
            path = base / target_file
            if path.is_file() and not path.is_symlink():
                candidates.append(path)
    # When task metadata is absent or points at a transformed workspace, check
    # the common one-card implementation source paths only. This avoids hashing
    # arbitrary trees or accepting receipt-controlled paths.
    if not candidates:
        source = candidate.get("source_path") or candidate.get("definition_source")
        if isinstance(source, str) and not Path(source).is_absolute() and ".." not in Path(source).parts:
            for base in (out / "logs/verifier/candidate-workspace", out / "candidate-workspace", out / "workspace"):
                path = base / source
                if path.is_file() and not path.is_symlink():
                    candidates.append(path)
    for path in candidates:
        try:
            if sha256_file(path).lower() == expected.lower():
                return True, path.relative_to(out).as_posix()
        except OSError:
            continue
    return False, None


def classify_receipt(path: Path, receipt: dict[str, Any], card: dict[str, str]) -> dict[str, Any]:
    verifier = receipt.get("verifier") if isinstance(receipt.get("verifier"), dict) else {}
    ident = receipt_identity_result(receipt, card)
    is_reference = receipt.get("agent") == "reference" or (verifier.get("candidate") or {}).get("kind") == "reference"
    verify_ok = receipt.get("verify_rc") == 0 and verifier.get("passed") is True
    pilot_passed = (
        (ident["direct_identity"] and verifier.get("pilot_card_passed") is True)
        or ident["breakdown_trace_passed"]
        or ident["breakdown_reports_credit"]
    )
    if is_reference:
        status = "reference_pass_uncredited" if verify_ok and pilot_passed else "reference_failed_or_incomplete"
        candidate_trial = False
        artifact_match = False
        artifact_path = None
    else:
        candidate_obj = verifier.get("candidate") if isinstance(verifier.get("candidate"), dict) else {}
        candidate_kind = candidate_obj.get("kind", verifier.get("candidate_kind"))
        runner_completed = (
            receipt.get("agent_completed") is True
            and receipt.get("codex_returncode") == 0
            and receipt.get("task_smoke_returncode") == 0
            and candidate_kind in {"candidate", "agent"}
            and verify_ok
            and pilot_passed
        )
        artifact_match, artifact_path = candidate_artifact_hash_matches(path, receipt) if runner_completed else (False, None)
        candidate_trial = bool(runner_completed and artifact_match)
        if not candidate_trial:
            status = "candidate_receipt_without_verified_artifact_or_pass"
        elif ident["breakdown_reports_credit"] or (
            ident["direct_identity"]
            and isinstance(verifier.get("cards_credited"), int)
            and verifier.get("cards_credited", 0) > 0
        ):
            status = "candidate_trial_receipt_reports_credit"
        else:
            status = "candidate_trial_verified_uncredited"
    card_credit = status == "candidate_trial_receipt_reports_credit"
    return {
        "path": path.relative_to(path.parents[3]).as_posix() if len(path.parents) > 3 else path.name,
        "status": status,
        "agent": receipt.get("agent"),
        "family": receipt.get("family"),
        "verify_rc": receipt.get("verify_rc"),
        "receipt_passed": verify_ok,
        "pilot_card_passed": bool(pilot_passed),
        "cards_credited_reported": verifier.get("cards_credited", 0),
        "candidate_trial_artifact_verified": candidate_trial,
        "candidate_artifact_path": artifact_path,
        "receipt_reports_card_credit": card_credit,
        "direct_card_identity": ident["direct_identity"],
    }


def receipts_for_card(
    root: Path,
    receipts: list[tuple[Path, dict[str, Any]]],
    card: dict[str, str],
) -> dict[str, Any]:
    matches: list[tuple[Path, dict[str, Any]]] = [
        (path, receipt) for path, receipt in receipts if receipt_matches_card(receipt, card)
    ]
    matches.sort(key=lambda pair: (pair[0].stat().st_mtime_ns, pair[0].as_posix()), reverse=True)
    classified = [classify_receipt(path, receipt, card) for path, receipt in matches]
    latest = classified[0] if classified else None
    credited_receipt = next((item for item in classified if item["receipt_reports_card_credit"]), None)
    return {
        "status": latest["status"] if latest else "no_matching_receipt",
        "latest_receipt": latest,
        "matching_receipts": classified,
        "receipt_count": len(classified),
        "candidate_trial_artifact_count": sum(item["candidate_trial_artifact_verified"] for item in classified),
        "receipt_attested_card_credit": credited_receipt is not None,
        "credit_receipt": credited_receipt,
    }


def build_report(root: Path) -> dict[str, Any]:
    catalog_rows, catalog_info = load_catalog(root)
    inventory, inventory_info = load_inventory(root)
    catalog_validation = compare_inventory_catalog(inventory, catalog_info, inventory_info)
    tasks_by_id, unlisted_tasks = discover_tasks(root, inventory)
    receipts = read_receipts(root)

    cards: list[dict[str, Any]] = []
    for row in sorted(inventory, key=lambda item: (int(item.get("collector_numbers", "0").split(";")[0] or 0), item.get("card_name", "").lower())):
        card = {key: str(row.get(key, "")).strip() for key in ("card_name", "definition_id", "collector_numbers", "printing_count")}
        tasks = tasks_by_id.get(card["definition_id"], [])
        card_tasks = []
        for task in tasks:
            card_tasks.append({
                **task,
                "review_hash_freshness": review_freshness(root, task, card),
            })
        receipt_report = receipts_for_card(root, receipts, card)
        has_credit = receipt_report["receipt_attested_card_credit"]
        cards.append({
            **card,
            "isolated_tasks": card_tasks,
            "isolated_task_count": len(card_tasks),
            "reference_receipt_status": receipt_report["status"],
            "receipt_evidence": receipt_report,
            "receipt_attested_card_credit": has_credit,
            "full_game_per_card_pvp_coverage_credited": bool(has_credit),
            "credit_status": "candidate_trial_receipt_and_artifact_attest_credit" if has_credit else "uncredited_no_verified_candidate_trial_artifact_credit",
        })

    task_count = sum(len(value) for value in tasks_by_id.values())
    reference_passes = sum(
        1 for card in cards
        for item in card["receipt_evidence"]["matching_receipts"]
        if item["status"] == "reference_pass_uncredited"
    )
    candidate_trials = sum(card["receipt_evidence"]["candidate_trial_artifact_count"] for card in cards)
    credited_cards = sum(card["full_game_per_card_pvp_coverage_credited"] for card in cards)
    task_identity_errors = sum(
        len(task.get("task_identity_errors", []))
        for card in cards
        for task in card["isolated_tasks"]
    ) + sum(len(task.get("task_identity_errors", [])) for task in unlisted_tasks)
    return {
        "schema_version": SCHEMA,
        "status": "diagnostic_only",
        "credit_authority": "none; reports only receipt-attested diagnostic evidence and never awards set-level credit",
        "generated_from": "read-only local files",
        "catalog_validation": catalog_validation,
        "summary": {
            "inventory_cards": len(inventory),
            "inventory_printings": inventory_info.get("printings", 0),
            "discovered_rav_card_tasks": task_count,
            "tasks_for_cards_outside_inventory": len(unlisted_tasks),
            "task_identity_errors": task_identity_errors,
            "matching_magic_harbor_receipts_scanned": len(receipts),
            "reference_pass_receipts_for_listed_cards": reference_passes,
            "candidate_trial_artifacts_verified": candidate_trials,
            "receipt_attested_credited_cards": credited_cards,
            "full_set_coverage_complete": False,
        },
        "unlisted_discovered_tasks": unlisted_tasks,
        "cards": cards,
        "limitations": [
            "Task and review metadata never grant PvP credit.",
            "Reference receipts establish reference diagnostics only; they remain uncredited.",
            "A candidate credit is surfaced only when a passing candidate receipt identifies the card, reports card credit, and its source hash matches a saved candidate workspace artifact.",
            "Per-card tasks with harbor.toml require hash freshness against their declared review_files; legacy tasks without a manifest use an explicit compatibility list, while unknown schemas remain unverified.",
            "Receipt contents are read as local evidence; this CLI does not rerun scoring or independently validate gameplay traces.",
        ],
    }


def render_terminal(report: dict[str, Any], limit: int | None) -> None:
    validation = report["catalog_validation"]
    summary = report["summary"]
    print(
        "RAV PvP ledger [diagnostic only]: "
        f"catalog={validation['status']} "
        f"{validation['inventory_unique_cards']}/{validation['catalog_unique_cards']} unique, "
        f"{validation['inventory_printings']}/{validation['catalog_printings']} printings"
    )
    print(
        f"isolated tasks={summary['discovered_rav_card_tasks']} | "
        f"reference pass receipts={summary['reference_pass_receipts_for_listed_cards']} | "
        f"verified candidate trials={summary['candidate_trial_artifacts_verified']} | "
        f"receipt-attested card credits={summary['receipt_attested_credited_cards']} | "
        f"task identity errors={summary['task_identity_errors']} | "
        "set coverage=incomplete"
    )
    cards = report["cards"]
    shown = cards if limit is None else cards[:limit]
    for card in shown:
        task_bits = []
        for task in card["isolated_tasks"]:
            review_status = task["review_hash_freshness"]["status"]
            identity_status = "invalid" if task.get("task_identity_errors") else "catalog-bound"
            task_bits.append(f"{task['task_id']} identity={identity_status} review={review_status}")
        task_text = ", ".join(task_bits) if task_bits else "no isolated task"
        print(
            f"{card['definition_id']}: {card['card_name']} | {task_text} | "
            f"receipt={card['reference_receipt_status']} | "
            f"credit={'receipt-attested' if card['full_game_per_card_pvp_coverage_credited'] else 'uncredited'}"
        )
    if len(shown) != len(cards):
        print(f"displayed={len(shown)}/{len(cards)} cards; JSON/summary counts cover the full inventory")
    if report["unlisted_discovered_tasks"]:
        names = ", ".join(item["task_id"] for item in report["unlisted_discovered_tasks"])
        print(f"discovered task(s) not mapped to inventory cards: {names}")
    if validation["errors"]:
        print("catalog/inventory diagnostics: " + " | ".join(validation["errors"]), file=sys.stderr)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[3], help="CardBench repository root")
    parser.add_argument("--limit", type=int, default=None, help="print only the first N card rows (all cards are still checked)")
    parser.add_argument("--json-out", type=Path, default=None, help="also write the full diagnostic report as JSON")
    args = parser.parse_args(argv)
    if args.limit is not None and args.limit < 0:
        parser.error("--limit must be nonnegative")
    try:
        root = args.root.resolve(strict=True)
    except OSError as exc:
        parser.error(f"repository root unavailable: {exc}")
    if not (root / CATALOG_REL).is_file() or not (root / INVENTORY_REL).is_file():
        parser.error(f"--root must contain {CATALOG_REL} and {INVENTORY_REL}")

    report = build_report(root)
    render_terminal(report, args.limit)
    if args.json_out is not None:
        output = args.json_out
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        print(f"json={output}")
    return 0 if report["catalog_validation"]["status"] == "valid" else 2


if __name__ == "__main__":
    raise SystemExit(main())
