"""Extract the CD audio tracks (2..7: the game's music, see work/cd/BIN/TRACKS.TXT) from the raw
CloneCD image into 44.1 kHz 16-bit stereo WAVs: work/cd/AUDIO/TRACK02.WAV .. TRACK07.WAV.

    python -I tools/extract_audio.py "iso/Eat This.cue" work/cd/AUDIO

The .cue holds one BINARY file of 2352-byte sectors; a track runs from its INDEX 01 to the next
track's INDEX 00 (its pregap) or the end of the image. Audio sectors are little-endian PCM.
"""
import re
import struct
import sys
from pathlib import Path


def msf(s):
    m, sec, f = (int(v) for v in s.split(":"))
    return (m * 60 + sec) * 75 + f


def main(cue, out):
    cue = Path(cue)
    img = None
    tracks = {}  # number -> {index: frame}
    cur = None
    for line in cue.read_text().splitlines():
        line = line.strip()
        if m := re.match(r'FILE "(.*)" BINARY', line):
            img = cue.parent / m.group(1)
        elif m := re.match(r"TRACK (\d+) (\S+)", line):
            cur = int(m.group(1))
            tracks[cur] = {"type": m.group(2)}
        elif m := re.match(r"INDEX (\d+) (\S+)", line):
            tracks[cur][int(m.group(1))] = msf(m.group(2))
    total = img.stat().st_size // 2352
    out = Path(out)
    out.mkdir(parents=True, exist_ok=True)
    nums = sorted(tracks)
    with open(img, "rb") as f:
        for i, n in enumerate(nums):
            t = tracks[n]
            if t["type"] != "AUDIO":
                continue
            start = t[1]
            end = total if i + 1 == len(nums) else tracks[nums[i + 1]].get(0, tracks[nums[i + 1]][1])
            f.seek(start * 2352)
            pcm = f.read((end - start) * 2352)
            hdr = b"RIFF" + struct.pack("<I", 36 + len(pcm)) + b"WAVEfmt " + struct.pack("<IHHIIHH", 16, 1, 2, 44100, 44100 * 4, 4, 16)
            hdr += b"data" + struct.pack("<I", len(pcm))
            path = out / f"TRACK{n:02d}.WAV"
            path.write_bytes(hdr + pcm)
            print(f"{path}: {(end - start) / 75:.1f} s")


if __name__ == "__main__":
    main(*sys.argv[1:3])
