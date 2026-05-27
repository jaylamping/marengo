#!/usr/bin/env python3
"""Plain I2C read (no register prefix) — matches BNO085 SHTP, not smbus block read."""
import fcntl
import struct
import sys

I2C_SLAVE = 0x0703
BUS = "/dev/i2c-1"


def plain_read(address: int, nbytes: int) -> bytes:
    with open(BUS, "rb", buffering=0) as dev:
        fcntl.ioctl(dev, I2C_SLAVE, address)
        return dev.read(nbytes)


def main() -> int:
    print(f"bus={BUS} (BNO085 needs plain read, not smbus register read)")
    for addr in (0x4A, 0x4B):
        try:
            data = plain_read(addr, 4)
            print(f"addr {addr:#x}: ok len={len(data)} bytes={[hex(b) for b in data]}")
        except OSError as e:
            print(f"addr {addr:#x}: {e}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
