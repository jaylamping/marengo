#!/usr/bin/env python3
"""Legacy smbus probe — NOT valid for BNO085 (expect EREMOTEIO). Use pi-i2c-plain-read.py."""
import sys

try:
    import smbus2
except ImportError:
    print("smbus2 not installed")
    sys.exit(2)

bus = smbus2.SMBus(1)
for addr in (0x4A, 0x4B):
    try:
        data = bus.read_i2c_block_data(addr, 0, 4)
        print(f"addr {addr:#x}: {[hex(x) for x in data]}")
    except OSError as e:
        print(f"addr {addr:#x}: OS error {e}")
