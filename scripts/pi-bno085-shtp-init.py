#!/usr/bin/env python3
"""BNO085 SHTP init using Adafruit-style data_ready + full packet read."""
import fcntl
import struct
import sys
import time

I2C_SLAVE = 0x0703
BUS = "/dev/i2c-1"
ADDR = 0x4B
CHANNEL_EXE = 1
CHANNEL_CONTROL = 2
PRODUCT_ID_REQ = 0xF9
PRODUCT_ID_RSP = 0xF8


def open_dev():
    return open(BUS, "rb+", buffering=0)


def set_addr(dev, addr: int) -> None:
    fcntl.ioctl(dev, I2C_SLAVE, addr)


def read_into(dev, buf: bytearray, end: int) -> None:
    got = dev.readinto(memoryview(buf)[:end])
    if got != end:
        raise OSError(f"short read {got} != {end}")


def parse_header(buf: bytes):
    raw = struct.unpack("<H", buf[:2])[0]
    count = raw & 0x7FFF
    ch, seq = buf[2], buf[3]
    data_len = max(0, count - 4)
    return count, ch, seq, data_len


def data_ready(dev, buf: bytearray) -> bool:
    read_into(dev, buf, 4)
    count, _ch, _seq, data_len = parse_header(buf)
    if count == 0x7FFF:
        return False
    return data_len > 0


def read_packet(dev, buf: bytearray) -> bytes | None:
    read_into(dev, buf, 4)
    count, ch, seq, data_len = parse_header(buf)
    if count < 4 or count == 0x7FFF:
        return bytes(buf[:4])
    total = count
    if len(buf) < total:
        raise OSError(f"buffer too small need {total}")
    read_into(dev, buf, total)  # Adafruit: second read is full packet length
    return bytes(buf[:total])


def send_packet(dev, channel: int, seq: int, payload: bytes) -> None:
    total = len(payload) + 4
    hdr = struct.pack("<HBB", total, channel, seq)
    dev.write(hdr + payload)


def soft_reset(dev, buf: bytearray, seq: list[int]) -> None:
    for _ in range(2):
        send_packet(dev, CHANNEL_EXE, seq[CHANNEL_EXE], b"\x01")
        seq[CHANNEL_EXE] = (seq[CHANNEL_EXE] + 1) & 0xFF
        time.sleep(0.5)
    for _ in range(3):
        try:
            if data_ready(dev, buf):
                read_packet(dev, buf)
        except OSError:
            time.sleep(0.5)


def wait_product_id(dev, buf: bytearray, seq: list[int], timeout: float = 8.0) -> bool:
    send_packet(dev, CHANNEL_CONTROL, seq[CHANNEL_CONTROL], bytes([PRODUCT_ID_REQ, 0]))
    seq[CHANNEL_CONTROL] = (seq[CHANNEL_CONTROL] + 1) & 0xFF

    deadline = time.time() + timeout
    while time.time() < deadline:
        if not data_ready(dev, buf):
            time.sleep(0.005)
            continue
        pkt = read_packet(dev, buf)
        if pkt is None or len(pkt) < 5:
            continue
        count, ch, seq_n, _ = parse_header(pkt)
        rid = pkt[4]
        print(f"  pkt len={count} ch={ch} seq={seq_n} rid={rid:#04x}")
        if ch == CHANNEL_CONTROL and rid == PRODUCT_ID_RSP:
            return True
        # skip channel 0/1 like Adafruit wait_for_packet_type
    return False


def main() -> int:
    addr = int(sys.argv[1], 16) if len(sys.argv) > 1 else ADDR
    dev = open_dev()
    set_addr(dev, addr)
    buf = bytearray(512)
    seq = [0] * 6

    print(f"bus={BUS} addr={addr:#x} (Adafruit read pattern, no peek)")
    print("soft reset...")
    soft_reset(dev, buf, seq)
    print("product id request...")
    ok = wait_product_id(dev, buf, seq)
    print("product id:", "OK" if ok else "TIMEOUT")
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
