"""Browse the exported decompilation: print functions and their call tree.

    fn.py show NAME...          print function bodies (blank lines and bare declarations dropped)
    fn.py tree NAME [depth]     callees, with size in lines
    fn.py callers NAME          functions that call NAME
"""

import re
import sys
from pathlib import Path

SRC = Path("work/decomp/ETLAUNCH.EXE.c")
HEAD = re.compile(r"^// ===== (\S+) @ (\S+)$", re.M)
CALL = re.compile(r"\b((?:FUN_[0-9a-f]{4}_[0-9a-f]{4})|(?:[A-Z][A-Za-z0-9]+))\(")
DECL = re.compile(r"^\s+(undefined\d?|u?int|u?long|char|bool|byte|ulong|undefined\d?\s*\*+)\s+\*?[A-Za-z_0-9\[\]]+;\s*$")


def functions() -> dict[str, str]:
    text = SRC.read_text()
    heads = list(HEAD.finditer(text))
    out = {}
    for h, nxt in zip(heads, heads[1:] + [None]):
        out[h.group(1)] = text[h.start() : nxt.start() if nxt else len(text)]
    return out


def body(src: str) -> str:
    return "\n".join(l for l in src.splitlines() if l.strip() and not DECL.match(l))


def callees(fns: dict[str, str], name: str) -> list[str]:
    seen = []
    for m in CALL.finditer(fns[name].split("\n", 3)[-1]):
        c = m.group(1)
        if c in fns and c != name and c not in seen:
            seen.append(c)
    return seen


def tree(fns, name, depth, indent=0, seen=None):
    seen = seen if seen is not None else set()
    print("  " * indent + f"{name} ({len(body(fns[name]).splitlines())})" + (" ..." if name in seen else ""))
    if name in seen or depth == 0:
        return
    seen.add(name)
    for c in callees(fns, name):
        tree(fns, c, depth - 1, indent + 1, seen)


if __name__ == "__main__":
    fns = functions()
    cmd, *args = sys.argv[1:]
    if cmd == "show":
        for a in args:
            print(body(fns[a]), "\n")
    elif cmd == "tree":
        tree(fns, args[0], int(args[1]) if len(args) > 1 else 2)
    elif cmd == "callers":
        pat = re.compile(rf"\b{re.escape(args[0])}\(")
        for n, s in fns.items():
            if n != args[0] and pat.search(s):
                print(n)
