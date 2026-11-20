#!/bin/sh
# Every oracle demo (the CD's seven and tools/scenarios' ones) against the port: CHECK windows,
# valdiff (every Random value; needs a full `mise run trace` of the demo) and the hero's
# x/y/state frame by frame (the trace's --watch gHeroX,gHeroY,gHeroState). Run from the repo root.
#   tools/checkall.sh [DEMO...]
cd "$(dirname "$0")/.." || exit 1
(cd port && cargo build --release -q 2>/dev/null) || exit 1
demos="$*"
[ -n "$demos" ] || demos="GRINDER LEVEL1_1 LEVEL1_2 LEVEL2_1 LEVEL2_2 LEVEL3_1 LEVEL3_2 $(cd tools/scenarios && ls *.txt | sed 's/\.txt$//')"
for d in $demos; do
  p=work/cd/BIN/USERMAPS/$d.DMO
  [ -e "$p" ] || p=work/demos/BIN/USERMAPS/$d.DMO
  out=$(cd port && ./target/release/eatthis --headless --demo ../$p --dump ../work/port_dump_$d.tsv 2>/dev/null)
  w=$(echo "$out" | grep -E "RandSeed matched" | sed "s/.*RandSeed matched in /windows /; s/ windows$//")
  e=$(echo "$out" | grep "ended by itself" | sed "s/the level ended by itself/, ended/")
  v=$(python3 tools/valdiff.py "$d" 2>/dev/null | head -1 | sed 's/.*mismatches: //')
  h=$(python3 tools/compare_watch.py work/tracer/$d/watch.tsv work/port_dump_$d.tsv 2>/dev/null | head -1)
  printf '%-9s %s%s | valdiff %s | hero: %s\n' "$d" "$w" "$e" "$v" "${h:-same}"
done
