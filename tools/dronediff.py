"""First frame where the drone's x, y, bob, angle, reload or burst differ between the tracer
(--watch 1170:4d9e,4da0,4da2,4dac,4dae,4db0,4db2,4db4,4db6,4db8,4dba -> work/tracer_drone2/watch.tsv)
and the port (ET_DRONE=1, DRONE lines saved as /tmp/pd.tsv with the prefix removed)."""
import sys
o = {}
for l in open('work/tracer_drone2/watch.tsv').read().splitlines()[1:]:
    f = l.split('\t'); o[int(f[0])] = f[1:]
p = {}
for l in open('/tmp/pd.tsv'):
    f = l.rstrip('\n').split('\t'); p[int(f[0])] = f[1:]
start = int(sys.argv[1]) if len(sys.argv) > 1 else 0
n = 0
for fr in sorted(o):
    if fr < start or fr not in p:
        continue
    a, b = o[fr], p[fr]
    A = [a[0], a[1], a[2], a[3], a[9], a[10]]; B = [b[0], b[1], b[2], b[3], b[7], b[8]]
    if A != B:
        print(fr, 'orig', A, 'port', B, 'orig target', a[5:9])
        n += 1
        if n > 4:
            break
