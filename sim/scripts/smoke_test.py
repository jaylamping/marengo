#!/usr/bin/env python3
"""Headless MuJoCo smoke test for CI."""
from __future__ import annotations

import sys
from pathlib import Path

import mujoco

_FIXTURES = Path(__file__).resolve().parents[1] / "fixtures"
_DEFAULT_MODEL = _FIXTURES / "minimal.xml"


def main() -> None:
    path = Path(sys.argv[1]) if len(sys.argv) > 1 else _DEFAULT_MODEL
    model = mujoco.MjModel.from_xml_path(str(path))
    data = mujoco.MjData(model)
    for _ in range(500):
        mujoco.mj_step(model, data)
    if model.nq < 1:
        raise SystemExit("model has no DOF")
    print(f"smoke_test: ok nq={model.nq} nv={model.nv} path={path}")


if __name__ == "__main__":
    main()
