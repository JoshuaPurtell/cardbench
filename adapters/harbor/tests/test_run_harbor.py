from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "run_harbor.py"


def _load_module():
    spec = importlib.util.spec_from_file_location("cardbench_run_harbor", SCRIPT)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def test_protocol_assert_skipped_when_expected_env_unset(monkeypatch) -> None:
    harbor = _load_module()
    monkeypatch.delenv("CARDBENCH_HARBOR_EXPECTED_MODEL_ID", raising=False)
    monkeypatch.delenv("CARDBENCH_HARBOR_EXPECTED_MULTI_AGENT_VERSION", raising=False)
    harbor.assert_expected_harbor_protocol({"metadata": {"trace_v5": {}}})


def test_protocol_assert_accepts_sealed_match(monkeypatch) -> None:
    harbor = _load_module()
    monkeypatch.setenv("CARDBENCH_HARBOR_EXPECTED_MODEL_ID", "gpt-5.6-luna")
    monkeypatch.setenv("CARDBENCH_HARBOR_EXPECTED_MULTI_AGENT_VERSION", "v1")
    harbor.assert_expected_harbor_protocol(
        {
            "metadata": {
                "model_name": "gpt-5.6-terra",
                "trace_v5": {
                    "model": "gpt-5.6-luna",
                    "multi_agent_version": "v1",
                },
            }
        }
    )


def test_protocol_assert_refuses_mismatch_and_nulls(monkeypatch) -> None:
    harbor = _load_module()
    monkeypatch.setenv("CARDBENCH_HARBOR_EXPECTED_MODEL_ID", "gpt-5.6-terra")
    monkeypatch.setenv("CARDBENCH_HARBOR_EXPECTED_MULTI_AGENT_VERSION", "v2")
    try:
        harbor.assert_expected_harbor_protocol(
            {
                "metadata": {
                    "effective_model_name": "gpt-5.6-terra",
                    "trace_v5": {"model": None, "multi_agent_version": None},
                }
            }
        )
    except harbor.HarborProtocolError as exc:
        message = str(exc)
    else:
        raise AssertionError("expected HarborProtocolError")
    assert message.startswith("harbor_protocol_mismatch")
    assert "sealed=null" in message


def test_protocol_mismatch_receipt_leaves_reward_null(tmp_path: Path) -> None:
    harbor = _load_module()
    harbor.write_receipt(
        tmp_path,
        "code_policy",
        "codex",
        0,
        1,
        protocol_error="harbor_protocol_mismatch:expected_multi_agent_version=v2 sealed=v1",
    )
    receipt = json.loads((tmp_path / "lane-receipt.json").read_text(encoding="utf-8"))
    assert receipt["reward"] is None
    assert receipt["verify_rc"] == 1
    assert receipt["contract_error"].startswith("harbor_protocol_mismatch")


def test_write_receipt_does_not_default_missing_harbor_reward_to_zero(
    tmp_path: Path,
) -> None:
    harbor = _load_module()
    verifier = tmp_path / "logs" / "verifier"
    verifier.mkdir(parents=True)
    (verifier / "result.json").write_text(json.dumps({"passed": False}) + "\n")
    harbor.write_receipt(tmp_path, "code_policy", "codex", 0, 1)
    receipt = json.loads((tmp_path / "lane-receipt.json").read_text(encoding="utf-8"))
    assert receipt["reward"] is None
