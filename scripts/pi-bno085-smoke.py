#!/usr/bin/env python3
"""One-shot BNO085 smoke test (Adafruit CircuitPython). Run on Pi bench."""
import sys
import time

try:
    import board
    import busio
    from adafruit_bno08x.i2c import BNO08X_I2C
except ImportError as e:
    print(f"import failed: {e}")
    print("use: ~/imu-venv/bin/python scripts/pi-bno085-smoke.py")
    sys.exit(2)

ADDR = 0x4B


def main() -> int:
    print(f"opening I2C address {ADDR:#x} ...")
    i2c = busio.I2C(board.SCL, board.SDA)
    while not i2c.try_lock():
        time.sleep(0.01)
    try:
        bno = BNO08X_I2C(i2c, address=ADDR)
        bno.enable_feature(0x05)  # rotation vector
        time.sleep(0.5)
        for i in range(3):
            q = bno.quaternion
            print(f"sample={i + 1} quaternion={q}")
        return 0
    except Exception as e:
        print(f"FAILED: {e}")
        return 1
    finally:
        i2c.unlock()


if __name__ == "__main__":
    sys.exit(main())
