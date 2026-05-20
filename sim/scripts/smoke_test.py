#!/usr/bin/env python3
"""Headless MuJoCo smoke test for CI."""
import sys

import mujoco


def main() -> None:
    path = sys.argv[1] if len(sys.argv) > 1 else "sim/fixtures/minimal.xml"
    model = mujoco.MjModel.from_xml_path(path)
    data = mujoco.MjData(model)
    for _ in range(500):
        mujoco.mj_step(model, data)
    if model.nq < 1:
        raise SystemExit("model has no DOF")
    print(f"smoke_test: ok nq={model.nq} nv={model.nv} path={path}")


if __name__ == "__main__":
    main()
