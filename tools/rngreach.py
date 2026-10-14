"""Which functions can reach Random(), from a root, and how directly.

    rngreach.py ROOT [depth]   prints the call tree below ROOT restricted to functions that reach Random
"""

import re
import sys

from fn import callees, functions

TARGETS = {"Random", "RandomNext"}


def reaching(fns):
    memo: dict[str, bool] = {}

    def go(n, stack=()):
        if n in TARGETS:
            return True
        if n in memo:
            return memo[n]
        if n in stack:
            return False
        memo[n] = any(go(c, stack + (n,)) for c in callees(fns, n))
        return memo[n]

    return go


def show(fns, go, name, depth, indent=0, seen=None):
    seen = seen if seen is not None else set()
    direct = len(re.findall(r"\bRandom\(", fns[name])) if name in fns else 0
    print("  " * indent + name + (f"  [{direct} direct]" if direct else "") + (" ..." if name in seen else ""))
    if name in seen or depth == 0 or name in TARGETS:
        return
    seen.add(name)
    for c in callees(fns, name):
        if c not in TARGETS and go(c):
            show(fns, go, c, depth - 1, indent + 1, seen)


if __name__ == "__main__":
    fns = functions()
    show(fns, reaching(fns), sys.argv[1], int(sys.argv[2]) if len(sys.argv) > 2 else 3)
