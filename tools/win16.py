"""A minimal Win16 environment on unicorn: loads an NE executable into 16-bit protected mode and
stubs the Windows API and DOS (INT 21h) just far enough to run Eat This headless.

Selectors are chosen so that NE segment n gets selector 0x1000 + (n-1)*8: addresses in the
emulator are the same as in Ghidra and re/names.csv. Every selector s has linear base
(s >> 3) << 16, so consecutive selectors are contiguous (huge blocks, __AHINCR = 8).
Imports point into a stub segment; each stub is `retf N`, and a code hook runs the Python
handler before it. Python can call into the program with `call_far`, also from inside a stub
(nested emu_start).
"""

import struct

from unicorn import UC_ARCH_X86, UC_HOOK_CODE, UC_HOOK_INSN, UC_HOOK_INTR, UC_HOOK_MEM_INVALID, UC_MODE_32, Uc, UcError
from unicorn.x86_const import (
    UC_X86_INS_IN,
    UC_X86_INS_OUT,
    UC_X86_REG_AX,
    UC_X86_REG_BP,
    UC_X86_REG_BX,
    UC_X86_REG_CR0,
    UC_X86_REG_CS,
    UC_X86_REG_CX,
    UC_X86_REG_DI,
    UC_X86_REG_DS,
    UC_X86_REG_DX,
    UC_X86_REG_EFLAGS,
    UC_X86_REG_EIP,
    UC_X86_REG_ES,
    UC_X86_REG_ESP,
    UC_X86_REG_FS,
    UC_X86_REG_GDTR,
    UC_X86_REG_GS,
    UC_X86_REG_SI,
    UC_X86_REG_SS,
)

from ne import NE

GDT_BASE = 0x20000000
STUB_SEL = 0x0800
TRAP_OFF = 0xFFF0  # in the stub segment: `call_far` returns here
DYN_FIRST = 0x2000  # first selector for GlobalAlloc
A000_SEL = 0x0A00
CARRY = 1

REGS = {
    "ax": UC_X86_REG_AX, "bx": UC_X86_REG_BX, "cx": UC_X86_REG_CX, "dx": UC_X86_REG_DX,
    "si": UC_X86_REG_SI, "di": UC_X86_REG_DI, "bp": UC_X86_REG_BP, "sp": UC_X86_REG_ESP,
    "ds": UC_X86_REG_DS, "es": UC_X86_REG_ES, "ss": UC_X86_REG_SS, "cs": UC_X86_REG_CS,
    "ip": UC_X86_REG_EIP, "flags": UC_X86_REG_EFLAGS, "fs": UC_X86_REG_FS, "gs": UC_X86_REG_GS,
}


def lin(sel):
    return (sel >> 3) << 16


class Stop(Exception):
    pass


class Win16:
    def __init__(self, exe, log=None):
        self.ne = NE(exe)
        self.log = log or (lambda *a: None)
        uc = self.uc = Uc(UC_ARCH_X86, UC_MODE_32)
        uc.mem_map(GDT_BASE, 0x10000)
        self.mapped = set()
        self.limits = {}
        self.free_sels = []
        self.next_sel = DYN_FIRST
        self.handles = {}  # selector -> size (GlobalAlloc)
        self.nested = 0
        self.trace_api = False
        self.sound = True  # DiamondWare init succeeds (the stubs play nothing)
        self._did_nested = False
        self.refire = None  # (address, sp) of a hook that will fire again after a nested call
        self.stubs = {}  # offset -> (name, purge, handler)
        self.stub_by_name = {}
        self.next_stub = 0
        self.api_counts = {}

        # segments
        for seg in self.ne.segments:
            sel = self.seg_sel(seg.num)
            size = max(len(seg.data), seg.minalloc)
            if seg.num == self.ne.autodata:
                size = 0x10000
            self.alloc_at(sel, size, code=not seg.is_data)
            uc.mem_write(lin(sel), seg.data)
        self.dgroup = self.seg_sel(self.ne.autodata)
        self.alloc_at(STUB_SEL, 0x10000, code=True)
        self.alloc_at(A000_SEL, 0x10000)
        uc.mem_write(lin(STUB_SEL) + TRAP_OFF, b"\xf4")  # hlt (never reached: emu_start stops first)
        self.apply_relocations()

        uc.reg_write(UC_X86_REG_GDTR, (0, GDT_BASE, 0xFFFF, 0))
        uc.reg_write(UC_X86_REG_CR0, uc.reg_read(UC_X86_REG_CR0) | 1)
        uc.hook_add(UC_HOOK_CODE, self._stub_hook, begin=lin(STUB_SEL), end=lin(STUB_SEL) + 0xFFEF)
        uc.hook_add(UC_HOOK_INTR, self._intr_hook)
        uc.hook_add(UC_HOOK_INSN, self._in_hook, None, 1, 0, UC_X86_INS_IN)
        uc.hook_add(UC_HOOK_INSN, self._out_hook, None, 1, 0, UC_X86_INS_OUT)
        uc.hook_add(UC_HOOK_MEM_INVALID, self._mem_invalid)

    # ---- selectors and memory -------------------------------------------------------------

    def seg_sel(self, n):
        return 0x1000 + (n - 1) * 8

    def set_desc(self, sel, base, limit, code=False):
        acc = 0x9B if code else 0x93
        d = struct.pack("<HHBBBB", limit & 0xFFFF, base & 0xFFFF, (base >> 16) & 0xFF, acc, (limit >> 16) & 0xF, (base >> 24) & 0xFF)
        self.uc.mem_write(GDT_BASE + (sel & ~7), d)
        self.limits[sel & ~7] = limit

    def alloc_at(self, sel, size, code=False):
        """Selectors sel, sel+8, ... covering size bytes (tiled like a Win16 huge block)."""
        n = max(1, (size + 0xFFFF) >> 16)
        for i in range(n):
            s = sel + 8 * i
            base = lin(s)
            if base not in self.mapped:
                self.uc.mem_map(base, 0x10000)
                self.mapped.add(base)
            else:
                self.uc.mem_write(base, bytes(0x10000))
            self.set_desc(s, base, min(max(size - 0x10000 * i, 0x10000), 0x100000) - 1, code)
        return sel

    def global_alloc(self, size):
        size = max(size, 16)
        n = max(1, (size + 0xFFFF) >> 16)
        # first fit in the free list of runs
        for i, (s, k) in enumerate(self.free_sels):
            if k >= n:
                if k == n:
                    self.free_sels.pop(i)
                else:
                    self.free_sels[i] = (s + 8 * n, k - n)
                break
        else:
            s = self.next_sel
            self.next_sel += 8 * n
            if self.next_sel >= 0xFFF8:
                raise Stop("out of selectors")
        self.alloc_at(s, size)
        self.handles[s] = (size, n)
        return s

    def global_free(self, sel):
        sel &= ~7
        if sel in self.handles:
            _, n = self.handles.pop(sel)
            self.free_sels.append((sel, n))
            for i in range(n):
                self.uc.mem_write(GDT_BASE + sel + 8 * i, bytes(8))
            return True
        return False

    def desc_base(self, sel):
        d = bytes(self.uc.mem_read(GDT_BASE + (sel & ~7), 8))
        return d[2] | d[3] << 8 | d[4] << 16 | d[7] << 24

    def addr(self, sel, off):
        return self.desc_base(sel) + off

    def rb(self, sel, off, n):
        return bytes(self.uc.mem_read(self.addr(sel, off), n))

    def wb(self, sel, off, data):
        self.uc.mem_write(self.addr(sel, off), bytes(data))

    def rw(self, sel, off):
        return struct.unpack("<H", self.rb(sel, off, 2))[0]

    def rd(self, sel, off):
        return struct.unpack("<I", self.rb(sel, off, 4))[0]

    def ww(self, sel, off, v):
        self.wb(sel, off, struct.pack("<H", v & 0xFFFF))

    def wd(self, sel, off, v):
        self.wb(sel, off, struct.pack("<I", v & 0xFFFFFFFF))

    def rfar(self, sel, off):
        o, s = struct.unpack("<HH", self.rb(sel, off, 4))
        return s, o

    def cstr(self, sel, off, maxlen=1024):
        if sel == 0:
            return None
        b = self.rb(sel, off, maxlen) if self.limits.get(sel & ~7, 0) - off >= maxlen else self.rb(sel, off, max(0, self.limits.get(sel & ~7, 0xFFFF) - off + 1))
        return b.split(b"\0", 1)[0].decode("latin-1")

    def pstr(self, sel, off):
        n = self.rb(sel, off, 1)[0]
        return self.rb(sel, off + 1, n).decode("latin-1")

    def wcstr(self, sel, off, s, size=None):
        b = s.encode("latin-1")
        if size is not None:
            b = b[: size - 1]
        self.wb(sel, off, b + b"\0")
        return len(b)

    # ---- registers ------------------------------------------------------------------------

    def __getitem__(self, r):
        v = self.uc.reg_read(REGS[r])
        return v & 0xFFFF if r not in ("flags",) else v

    def __setitem__(self, r, v):
        self.uc.reg_write(REGS[r], v & 0xFFFF if r not in ("flags", "ip", "sp") else v)

    def push(self, v):
        sp = (self["sp"] - 2) & 0xFFFF
        self["sp"] = sp
        self.ww(self["ss"], sp, v)

    def set_carry(self, on):
        f = self["flags"]
        self["flags"] = (f | CARRY) if on else (f & ~CARRY)

    # ---- relocations ----------------------------------------------------------------------

    def import_target(self, mod, key):
        """(selector, offset) of the stub for an imported function, or an int for constants."""
        if mod == "KERNEL" and key == 114:  # __AHINCR
            return 8
        if mod == "KERNEL" and key == 174:  # __A000H
            return A000_SEL
        from win16api import api_spec

        name, purge = api_spec(mod, key)
        if name in self.stub_by_name:
            return STUB_SEL, self.stub_by_name[name]
        off = self.next_stub
        self.next_stub += 4
        self.uc.mem_write(lin(STUB_SEL) + off, bytes([0xCA]) + struct.pack("<H", purge) + b"\x90")
        self.stubs[off] = (name, purge)
        self.stub_by_name[name] = off
        return STUB_SEL, off

    def apply_relocations(self):
        ne = self.ne
        for seg in ne.segments:
            sel = self.seg_sel(seg.num)
            base = lin(sel)
            for r in seg.relocs:
                if r.kind == 3:
                    continue  # floating point fixups: we have an FPU
                if r.kind == 0:
                    if r.a == 0xFF:
                        tseg, toff = ne.entries[r.b]
                    else:
                        tseg, toff = r.a, r.b
                    target = (self.seg_sel(tseg), toff)
                elif r.kind == 1:
                    target = self.import_target(ne.modules[r.a - 1], r.b)
                else:
                    target = self.import_target(ne.modules[r.a - 1], ne.import_name(r.b))
                if isinstance(target, int):
                    tsel, toff = target, target
                else:
                    tsel, toff = target
                off = r.offset
                seen = set()
                while True:
                    cur = struct.unpack("<H", bytes(self.uc.mem_read(base + off, 2)))[0]
                    if r.src == 3:  # far pointer
                        if r.additive:
                            self.uc.mem_write(base + off, struct.pack("<HH", (cur + toff) & 0xFFFF, tsel))
                        else:
                            self.uc.mem_write(base + off, struct.pack("<HH", toff, tsel))
                    elif r.src == 2:
                        self.uc.mem_write(base + off, struct.pack("<H", tsel))
                    elif r.src == 5:
                        self.uc.mem_write(base + off, struct.pack("<H", ((cur if r.additive else 0) + toff) & 0xFFFF))
                    elif r.src == 0:
                        self.uc.mem_write(base + off, bytes([toff & 0xFF]))
                    else:
                        raise ValueError(f"reloc type {r.src}")
                    if r.additive or cur == 0xFFFF or r.src == 0:
                        break
                    seen.add(off)
                    off = cur
                    if off in seen:
                        break

    def alias(self, src, code):
        sel = self.global_alloc(16)
        self.set_desc(sel, self.desc_base(src), self.limits.get(src & ~7, 0xFFFF), code=code)
        return sel

    # ---- process --------------------------------------------------------------------------

    def setup_process(self, fs):
        """PSP, environment, the Win16 registers at the entry point."""
        self.fs = fs
        self.clock_ms = 0
        self.classes = {}
        self.windows = {}
        self.main_hwnd = 0
        self.cursor = (160, 100)
        self.messages = []
        self.dta = (0, 0)
        self.find = {}  # DTA -> remaining matches
        self.error = None
        env = self.global_alloc(0x100)
        self.wb(env, 0, b"PATH=C:\\WINDOWS\0\0\x01\0" + fs.exe_path.encode() + b"\0")
        self.psp = self.global_alloc(0x100)
        self.wb(self.psp, 0, b"\xcd\x20")
        self.ww(self.psp, 0x2C, env)
        self.wb(self.psp, 0x80, b"\x00\x0d")
        self.scratch = self.global_alloc(0x1000)
        ne = self.ne
        sp_top = (0x9F30 + ne.heap + ne.stack) & ~1
        for r in ("ds", "ss"):
            self[r] = self.dgroup
        self["es"] = self.psp
        self["sp"] = sp_top
        self["ax"], self["bx"], self["cx"], self["dx"] = 0, ne.stack, ne.heap, 0
        self["si"], self["di"], self["bp"] = 0, self.dgroup, 0
        self.entry = (self.seg_sel(ne.cs), ne.ip)

    def clock_tick(self, ms=1):
        self.clock_ms += ms
        return self.clock_ms

    def key_state(self, vk):
        return 0

    def on_idle(self):
        pass

    def run(self):
        """Run from the NE entry point until the program stops (or `fail`/`emu_stop`)."""
        sel, off = self.entry
        self.uc.reg_write(UC_X86_REG_CS, sel)
        self.nested += 1
        try:
            self.uc.emu_start(off, lin(STUB_SEL) + TRAP_OFF)
        except UcError as e:
            self.error = self.error or f"{e} at {self.where()}"
        finally:
            self.nested -= 1

    # ---- calls ----------------------------------------------------------------------------

    def save_regs(self):
        return {r: self.uc.reg_read(REGS[r]) for r in REGS}

    def load_regs(self, regs):
        # selectors first; cs last together with ip
        for r in ("ss", "ds", "es", "fs", "gs", "sp", "ax", "bx", "cx", "dx", "si", "di", "bp", "flags"):
            self.uc.reg_write(REGS[r], regs[r])
        self.uc.reg_write(UC_X86_REG_CS, regs["cs"])
        self.uc.reg_write(UC_X86_REG_EIP, regs["ip"])

    def call_far(self, sel, off, args=(), regs=None):
        """Call sel:off (Pascal convention: args pushed left to right). Returns DX:AX."""
        nested = self.nested > 0
        saved = self.save_regs() if nested else None
        if regs:
            for k, v in regs.items():
                self[k] = v
        for a in args:
            self.push(a)
        self.push(STUB_SEL)
        self.push(TRAP_OFF)
        self.uc.reg_write(UC_X86_REG_CS, sel)
        self.nested += 1
        try:
            self.uc.emu_start(off, lin(STUB_SEL) + TRAP_OFF)
        except UcError as e:
            self.error = self.error or f"{e} at {self.where()}"
        finally:
            self.nested -= 1
        ret = self["dx"] << 16 | self["ax"]
        if self.error:
            raise Stop(self.error)
        if nested:  # back to the interrupted instruction (a stub handler then sets DX:AX itself)
            self._did_nested = True
            self.load_regs(saved)
        return ret

    def where(self):
        return f"{self['cs']:04x}:{self['ip']:04x}"

    # ---- hooks ----------------------------------------------------------------------------

    def _stub_hook(self, uc, address, size, _):
        off = address - lin(STUB_SEL)
        if off not in self.stubs:
            return
        if self._refired(address):
            return
        name, purge = self.stubs[off]
        self.api_counts[name] = self.api_counts.get(name, 0) + 1
        from win16api import HANDLERS

        h = HANDLERS.get(name)
        if h is None:
            sp = self["sp"]
            ret = self.rfar(self["ss"], sp)
            self.fail(f"unimplemented API {name} called from {ret[0]:04x}:{ret[1]:04x}")
            return
        args = ArgReader(self, purge)
        if self.trace_api:
            rs, ro = args.ret_addr()
            self.log(f"API {name} from {rs:04x}:{ro:04x}")
        self._did_nested = False
        r = h(self, args)
        if r is not None:
            self["ax"] = r & 0xFFFF
            self["dx"] = (r >> 16) & 0xFFFF
        self._after_hook(address)

    def _refired(self, address):
        """After a nested emu_start inside a hook, unicorn runs the hook of the interrupted
        instruction again when the outer emulation resumes: skip that second run."""
        if self.refire is None:
            return False
        hit = self.refire == (address, self["sp"])
        self.refire = None
        return hit

    def _after_hook(self, address):
        if self._did_nested:
            self._did_nested = False
            self.refire = (address, self["sp"])

    def hook(self, sel, off, fn):
        """Run fn(emu) before the instruction at sel:off (re-fires are filtered after nested calls)."""
        address = lin(sel) + off

        def cb(uc, addr, size, _):
            if self._refired(address):
                return
            self._did_nested = False
            fn(self)
            self._after_hook(address)

        return self.uc.hook_add(UC_HOOK_CODE, cb, begin=address, end=address)

    def fail(self, msg):
        self.error = msg
        self.uc.emu_stop()

    def _intr_hook(self, uc, intno, _):
        from win16api import interrupt

        interrupt(self, intno)

    def _in_hook(self, uc, port, size, _):
        if port == 0x3DA:
            self._retrace = getattr(self, "_retrace", 0) ^ 8
            return self._retrace
        return 0

    def _out_hook(self, uc, port, size, value, _):
        pass

    def _mem_invalid(self, uc, access, address, size, value, _):
        self.error = f"invalid memory access {access} at {address:#x} size {size} ({self.where()})"
        return False


class ArgReader:
    """Pascal-convention arguments: the first argument is deepest on the stack."""

    def __init__(self, emu, purge):
        self.emu = emu
        self.ss = emu["ss"]
        self.base = emu["sp"] + 4
        self.purge = purge
        self.pos = purge  # bytes from base to the end of the first argument

    def w(self):
        self.pos -= 2
        return self.emu.rw(self.ss, self.base + self.pos)

    def i(self):
        v = self.w()
        return v - 0x10000 if v & 0x8000 else v

    def d(self):
        self.pos -= 4
        return self.emu.rd(self.ss, self.base + self.pos)

    def p(self):
        """far pointer -> (sel, off)"""
        self.pos -= 4
        o, s = struct.unpack("<HH", self.emu.rb(self.ss, self.base + self.pos, 4))
        return s, o

    def ret_addr(self):
        return self.emu.rfar(self.ss, self.base - 4)
