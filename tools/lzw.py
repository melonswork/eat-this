"""LZW as used by Image Line archives (ETLAUNCH.EXE seg 1108: decoder 1108:00d6, encoder 1108:05f0).

GIF-style: codes start at 9 bits and grow to 12, read LSB-first; 0x100 clears the dictionary,
0x101 ends the stream, the first free code is 0x102. A compressed entry is prefixed by its
unpacked size as u32.
"""

import struct

CLEAR, END, FIRST = 0x100, 0x101, 0x102


def decompress(data: bytes) -> bytes:
    if not data:
        return b""
    size = struct.unpack_from("<I", data)[0]
    bits = int.from_bytes(data[4:], "little")
    nbits = (len(data) - 4) * 8
    pos = 0
    out = bytearray()
    width, limit = 9, 0x200
    table: list[bytes] = []
    prev: bytes | None = None
    while pos + width <= nbits:
        code = (bits >> pos) & ((1 << width) - 1)
        pos += width
        if code == END:
            break
        if code == CLEAR:
            width, limit = 9, 0x200
            table = [bytes([i]) for i in range(256)] + [b"", b""]
            prev = None
            continue
        if code < len(table):
            entry = table[code]
        elif code == len(table) and prev is not None:
            entry = prev + prev[:1]
        else:
            raise ValueError(f"bad LZW code {code:#x} at bit {pos - width}")
        out += entry
        if prev is not None and len(table) < 0x1000:
            table.append(prev + entry[:1])
            if len(table) >= limit and width < 12:
                width, limit = width + 1, limit << 1
        prev = entry
    if len(out) != size:
        raise ValueError(f"unpacked {len(out)} bytes, header says {size}")
    return bytes(out)
