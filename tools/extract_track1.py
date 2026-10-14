"""Extract the Mode 1/Mode 2 Form 1 data track from a raw 2352-byte/sector CD image into a 2048-byte/sector ISO."""
import sys
src, dst, nsect = sys.argv[1], sys.argv[2], int(sys.argv[3])
with open(src,'rb') as f, open(dst,'wb') as o:
    for i in range(nsect):
        s = f.read(2352)
        if len(s) < 2352: break
        mode = s[15]
        if mode == 1: o.write(s[16:16+2048])
        elif mode == 2:
            form2 = s[18] & 0x20
            if form2: o.write(s[24:24+2324][:2048])  # rare; truncated
            else: o.write(s[24:24+2048])
        else: o.write(b'\0'*2048)
