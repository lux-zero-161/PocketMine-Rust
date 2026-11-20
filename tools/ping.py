#!/usr/bin/env python3
"""Sends a RakNet unconnected ping and prints the Bedrock server-list data.

Usage: python3 tools/ping.py [port] [host]
"""
import socket
import struct
import sys

MAGIC = bytes([
    0x00, 0xFF, 0xFF, 0x00, 0xFE, 0xFE, 0xFE, 0xFE,
    0xFD, 0xFD, 0xFD, 0xFD, 0x12, 0x34, 0x56, 0x78,
])


def main() -> int:
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 19132
    host = sys.argv[2] if len(sys.argv) > 2 else "127.0.0.1"

    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.settimeout(2.0)
    ping_time = 123456789
    packet = b"\x01" + struct.pack(">q", ping_time) + MAGIC + struct.pack(">q", 1)
    sock.sendto(packet, (host, port))

    try:
        data, _ = sock.recvfrom(4096)
    except socket.timeout:
        print(f"No pong from {host}:{port} (server not running?)")
        return 1

    if not data or data[0] != 0x1C:
        print(f"Unexpected packet id: {data[:1].hex()}")
        return 1

    echoed = struct.unpack(">q", data[1:9])[0]
    guid = struct.unpack(">Q", data[9:17])[0]
    length = struct.unpack(">H", data[33:35])[0]
    motd = data[35:35 + length].decode("utf-8", "replace")
    fields = motd.rstrip(";").split(";")

    print(f"Pong from {host}:{port} (echoed {echoed}, server id {guid})")
    labels = [
        "edition", "motd", "protocol", "version", "online", "max",
        "server_id", "sub_motd", "gamemode", "gamemode_id", "port_v4", "port_v6",
    ]
    for label, value in zip(labels, fields):
        print(f"  {label:12} {value}")
    if len(fields) < 12:
        print("  warning: pong has fewer fields than expected")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
