"""Win16 API stubs and DOS INT 21h for tools/win16.py.

Each handler gets (emu, args) and returns the value for DX:AX (or None to leave them).
`args` reads Pascal-convention arguments in declaration order. Files: the game directory
C:\\ETHIS is work/cd (read-only); files the game writes go to an overlay directory.
"""

import struct
from pathlib import Path

# (module, ordinal or name) -> (name, bytes of arguments)
SPECS = {
    ("KERNEL", 91): ("INITTASK", 0),
    ("KERNEL", 30): ("WAITEVENT", 2),
    ("KERNEL", 132): ("GETWINFLAGS", 0),
    ("KERNEL", 49): ("GETMODULEFILENAME", 8),
    ("KERNEL", 15): ("GLOBALALLOC", 6),
    ("KERNEL", 17): ("GLOBALFREE", 2),
    ("KERNEL", 18): ("GLOBALLOCK", 2),
    ("KERNEL", 19): ("GLOBALUNLOCK", 2),
    ("KERNEL", 21): ("GLOBALHANDLE", 2),
    ("KERNEL", 25): ("GLOBALCOMPACT", 4),
    ("KERNEL", 51): ("MAKEPROCINSTANCE", 6),
    ("KERNEL", 52): ("FREEPROCINSTANCE", 4),
    ("KERNEL", 127): ("GETPRIVATEPROFILEINT", 14),
    ("KERNEL", 128): ("GETPRIVATEPROFILESTRING", 22),
    ("KERNEL", 129): ("WRITEPRIVATEPROFILESTRING", 16),
    ("KERNEL", 134): ("GETWINDOWSDIRECTORY", 6),
    ("KERNEL", 169): ("GETFREESPACE", 2),
    ("KERNEL", 170): ("ALLOCCSTODSALIAS", 2),
    ("KERNEL", 176): ("FREESELECTOR", 2),
    ("KERNEL", 177): ("PRESTOCHANGOSELECTOR", 4),
    ("TOOLHELP", 80): ("TIMERCOUNT", 4),
    ("WIN87EM", 1): ("__FPMATH", 0),
    ("KEYBOARD", 5): ("ANSITOOEM", 8),
    ("KEYBOARD", 6): ("OEMTOANSI", 8),
    ("DISPDIB", 1): ("DISPLAYDIB", 10),
    ("USER", 1): ("MESSAGEBOX", 12),
    ("USER", 5): ("INITAPP", 2),
    ("USER", 6): ("POSTQUITMESSAGE", 2),
    ("USER", 13): ("GETTICKCOUNT", 0),
    ("USER", 16): ("CLIPCURSOR", 4),
    ("USER", 17): ("GETCURSORPOS", 4),
    ("USER", 18): ("SETCAPTURE", 2),
    ("USER", 19): ("RELEASECAPTURE", 0),
    ("USER", 22): ("SETFOCUS", 2),
    ("USER", 23): ("GETFOCUS", 0),
    ("USER", 24): ("REMOVEPROP", 6),
    ("USER", 25): ("GETPROP", 6),
    ("USER", 26): ("SETPROP", 8),
    ("USER", 29): ("SCREENTOCLIENT", 6),
    ("USER", 31): ("ISICONIC", 2),
    ("USER", 32): ("GETWINDOWRECT", 6),
    ("USER", 36): ("GETWINDOWTEXT", 8),
    ("USER", 37): ("SETWINDOWTEXT", 6),
    ("USER", 39): ("BEGINPAINT", 6),
    ("USER", 40): ("ENDPAINT", 6),
    ("USER", 41): ("CREATEWINDOW", 30),
    ("USER", 42): ("SHOWWINDOW", 4),
    ("USER", 46): ("GETPARENT", 2),
    ("USER", 47): ("ISWINDOW", 2),
    ("USER", 48): ("ISCHILD", 4),
    ("USER", 53): ("DESTROYWINDOW", 2),
    ("USER", 57): ("REGISTERCLASS", 4),
    ("USER", 60): ("GETACTIVEWINDOW", 0),
    ("USER", 61): ("SCROLLWINDOW", 14),
    ("USER", 62): ("SETSCROLLPOS", 8),
    ("USER", 64): ("SETSCROLLRANGE", 10),
    ("USER", 66): ("GETDC", 2),
    ("USER", 68): ("RELEASEDC", 4),
    ("USER", 69): ("SETCURSOR", 2),
    ("USER", 70): ("SETCURSORPOS", 4),
    ("USER", 72): ("SETRECT", 12),
    ("USER", 81): ("FILLRECT", 8),
    ("USER", 90): ("ISDIALOGMESSAGE", 6),
    ("USER", 91): ("GETDLGITEM", 4),
    ("USER", 104): ("MESSAGEBEEP", 2),
    ("USER", 106): ("GETKEYSTATE", 2),
    ("USER", 107): ("DEFWINDOWPROC", 10),
    ("USER", 108): ("GETMESSAGE", 10),
    ("USER", 109): ("PEEKMESSAGE", 12),
    ("USER", 111): ("SENDMESSAGE", 10),
    ("USER", 112): ("WAITMESSAGE", 0),
    ("USER", 113): ("TRANSLATEMESSAGE", 4),
    ("USER", 114): ("DISPATCHMESSAGE", 4),
    ("USER", 122): ("CALLWINDOWPROC", 14),
    ("USER", 124): ("UPDATEWINDOW", 2),
    ("USER", 125): ("INVALIDATERECT", 8),
    ("USER", 133): ("GETWINDOWWORD", 4),
    ("USER", 135): ("GETWINDOWLONG", 4),
    ("USER", 136): ("SETWINDOWLONG", 8),
    ("USER", 155): ("ENABLEMENUITEM", 6),
    ("USER", 156): ("GETSYSTEMMENU", 4),
    ("USER", 163): ("CREATECARET", 8),
    ("USER", 164): ("DESTROYCARET", 0),
    ("USER", 165): ("SETCARETPOS", 4),
    ("USER", 167): ("SHOWCARET", 2),
    ("USER", 173): ("LOADCURSOR", 6),
    ("USER", 174): ("LOADICON", 6),
    ("USER", 178): ("TRANSLATEACCELERATOR", 8),
    ("USER", 179): ("GETSYSTEMMETRICS", 2),
    ("USER", 180): ("GETSYSCOLOR", 2),
    ("USER", 181): ("SETSYSCOLORS", 10),
    ("USER", 232): ("SETWINDOWPOS", 14),
    ("USER", 236): ("GETCAPTURE", 0),
    ("USER", 239): ("DIALOGBOXPARAM", 16),
    ("USER", 241): ("CREATEDIALOGPARAM", 16),
    ("USER", 247): ("GETCURSOR", 0),
    ("USER", 272): ("ISZOOMED", 2),
    ("USER", 282): ("SELECTPALETTE", 6),
    ("USER", 283): ("REALIZEPALETTE", 2),
    ("USER", 308): ("DEFDLGPROC", 10),
    ("USER", 404): ("GETCLASSINFO", 10),
    ("USER", 421): ("WVSPRINTF", 12),
    ("USER", 447): ("DEFMDICHILDPROC", 10),
    ("USER", 451): ("TRANSLATEMDISYSACCEL", 6),
    ("USER", 452): ("CREATEWINDOWEX", 34),
    ("USER", 458): ("DESTROYCURSOR", 2),
    ("GDI", 1): ("SETBKCOLOR", 6),
    ("GDI", 2): ("SETBKMODE", 4),
    ("GDI", 9): ("SETTEXTCOLOR", 6),
    ("GDI", 33): ("TEXTOUT", 12),
    ("GDI", 45): ("SELECTOBJECT", 4),
    ("GDI", 57): ("CREATEFONTINDIRECT", 4),
    ("GDI", 68): ("DELETEDC", 2),
    ("GDI", 69): ("DELETEOBJECT", 2),
    ("GDI", 80): ("GETDEVICECAPS", 4),
    ("GDI", 87): ("GETSTOCKOBJECT", 2),
    ("GDI", 93): ("GETTEXTMETRICS", 6),
    ("GDI", 346): ("SETTEXTALIGN", 4),
    ("GDI", 360): ("CREATEPALETTE", 4),
    ("GDI", 363): ("GETPALETTEENTRIES", 10),
    ("GDI", 373): ("SETSYSTEMPALETTEUSE", 4),
    ("GDI", 374): ("GETSYSTEMPALETTEUSE", 2),
    ("GDI", 375): ("GETSYSTEMPALETTEENTRIES", 10),
    ("MMSYSTEM", 103): ("JOYGETPOS", 6),
    ("MMSYSTEM", 702): ("MCISENDSTRING", 12),
    ("MMSYSTEM", 703): ("MCIGETDEVICEID", 4),
    ("MMSYSTEM", "mixerGetNumDevs"): ("MIXERGETNUMDEVS", 0),
    ("MMSYSTEM", "mixerOpen"): ("MIXEROPEN", 18),
    ("MMSYSTEM", "mixerClose"): ("MIXERCLOSE", 2),
    ("MMSYSTEM", "mixerGetDevCaps"): ("MIXERGETDEVCAPS", 8),
    ("MMSYSTEM", "mixerGetLineInfo"): ("MIXERGETLINEINFO", 10),
    ("MMSYSTEM", "mixerGetLineControls"): ("MIXERGETLINECONTROLS", 10),
    ("MMSYSTEM", "mixerGetControlDetails"): ("MIXERGETCONTROLDETAILS", 10),
    ("MMSYSTEM", "mixerSetControlDetails"): ("MIXERSETCONTROLDETAILS", 10),
    ("WING", "WINGCREATEDC"): ("WINGCREATEDC", 0),
    ("WING", "WINGRECOMMENDDIBFORMAT"): ("WINGRECOMMENDDIBFORMAT", 4),
    ("WING", "WINGCREATEBITMAP"): ("WINGCREATEBITMAP", 10),
    ("WING", "WINGBITBLT"): ("WINGBITBLT", 16),
    ("WING", "WINGSTRETCHBLT"): ("WINGSTRETCHBLT", 20),
    ("WING", "WINGSETDIBCOLORTABLE"): ("WINGSETDIBCOLORTABLE", 10),
    # DiamondWare Sound ToolKit, far pascal
    ("DWSW16", "DWS_INIT"): ("DWS_INIT", 8),
    ("DWSW16", "DWS_DETECTHARDWARE"): ("DWS_DETECTHARDWARE", 4),
    ("DWSW16", "DWS_KILL"): ("DWS_KILL", 0),
    ("DWSW16", "DWS_UPDATE"): ("DWS_UPDATE", 0),
    ("DWSW16", "DWS_DPLAY"): ("DWS_DPLAY", 4),
    ("DWSW16", "DWS_DSETINFO"): ("DWS_DSETINFO", 8),
    ("DWSW16", "DWS_DDISCARD"): ("DWS_DDISCARD", 0),
    ("DWSW16", "DWS_DDISCARDAO"): ("DWS_DDISCARDAO", 4),
    ("DWSW16", "DWS_DCLEAR"): ("DWS_DCLEAR", 0),
    ("DWSW16", "DWS_WAV2DWD"): ("DWS_WAV2DWD", 12),
    ("DWSW16", "DWS_XDIG"): ("DWS_XDIG", 2),
}


def api_spec(mod, key):
    if (mod, key) in SPECS:
        return SPECS[(mod, key)]
    return (f"{mod}.{key}", 0)


HANDLERS = {}


def api(*names):
    def deco(f):
        for n in names:
            HANDLERS[n] = f
        return f

    return deco


def const(value, *names):
    for n in names:
        HANDLERS[n] = lambda emu, a, v=value: v


const(1, "WAITEVENT", "INITAPP", "GLOBALUNLOCK", "FREEPROCINSTANCE", "WRITEPRIVATEPROFILESTRING",
      "SETCAPTURE", "UPDATEWINDOW", "INVALIDATERECT", "ENABLEMENUITEM", "SETRECT_",
      "FILLRECT", "SETCURSORPOS", "CLIPCURSOR", "MESSAGEBEEP", "DESTROYCURSOR", "DELETEDC",
      "DELETEOBJECT", "SETBKMODE", "SETTEXTALIGN", "TEXTOUT", "SETWINDOWPOS", "SETWINDOWTEXT",
      "REALIZEPALETTE", "WINGBITBLT", "WINGSTRETCHBLT", "SETSYSCOLORS", "SETSCROLLPOS",
      "SETSCROLLRANGE", "SCROLLWINDOW", "ENDPAINT", "SHOWCARET", "SETCARETPOS", "CREATECARET",
      "DESTROYCARET", "TRANSLATEMESSAGE", "RELEASEDC", "FREESELECTOR", "DISPLAYDIB",
      "ANSITOOEM", "OEMTOANSI", "DWS_KILL", "DWS_UPDATE", "DWS_DDISCARD", "DWS_DDISCARDAO",
      "DWS_DCLEAR", "DWS_XDIG", "DWS_DSETINFO", "MIXERCLOSE", "SETFOCUS", "ISWINDOW")
const(0, "RELEASECAPTURE", "ISICONIC", "ISZOOMED", "GETPARENT", "ISCHILD", "MIXERGETNUMDEVS",
      "ISDIALOGMESSAGE", "TRANSLATEACCELERATOR", "TRANSLATEMDISYSACCEL", "SETCURSOR",
      "GETCURSOR", "MCIGETDEVICEID", "GETSYSTEMMENU", "GETDLGITEM", "SETBKCOLOR",
      "SETTEXTCOLOR", "SETSYSTEMPALETTEUSE", "GETSYSTEMPALETTEUSE", "WAITMESSAGE")
const(6, "MIXEROPEN", "MIXERGETDEVCAPS", "MIXERGETLINEINFO", "MIXERGETLINECONTROLS",
      "MIXERGETCONTROLDETAILS", "MIXERSETCONTROLDETAILS")  # MMSYSERR_NODRIVER
const(0x1C00, "GETSTOCKOBJECT")
const(0x1C08, "CREATEFONTINDIRECT", "CREATEPALETTE")
const(0x2C00, "LOADCURSOR", "LOADICON")
const(0x1C10, "SELECTOBJECT", "SELECTPALETTE")
const(0x1C20, "GETDC", "BEGINPAINT", "WINGCREATEDC")
const(0xC0C0C0, "GETSYSCOLOR")
const(0x7FFF0000 & 0, "GLOBALCOMPACT")


@api("INITTASK")
def inittask(emu, a):
    ds = emu.dgroup
    ne = emu.ne
    sp_top = (0x9F30 + ne.heap + ne.stack) & ~1
    # instance data: pStackTop, pStackMin, pStackBottom
    emu.ww(ds, 0x0A, sp_top - ne.stack)
    emu.ww(ds, 0x0C, sp_top)
    emu.ww(ds, 0x0E, sp_top)
    emu["cx"] = ne.stack
    emu["dx"] = 1  # nCmdShow
    emu["si"] = 0
    emu["di"] = ds  # hInstance
    emu["es"] = emu.psp
    emu["bx"] = 0x80
    return 1


@api("GETWINFLAGS")
def getwinflags(emu, a):
    return 0x0010 | 0x0400 | 0x0008 | 0x0002  # enhanced, 80x87, 486, pmode


@api("GETMODULEFILENAME")
def getmodulefilename(emu, a):
    _h, (s, o), n = a.w(), a.p(), a.w()
    return emu.wcstr(s, o, emu.fs.exe_path, n)


@api("GETWINDOWSDIRECTORY")
def getwindowsdirectory(emu, a):
    (s, o), n = a.p(), a.w()
    return emu.wcstr(s, o, "C:\\WINDOWS", n)


@api("GLOBALALLOC")
def globalalloc(emu, a):
    flags, size = a.w(), a.d()
    return emu.global_alloc(size)


@api("GLOBALFREE")
def globalfree(emu, a):
    h = a.w()
    return 0 if emu.global_free(h) else h


@api("GLOBALLOCK")
def globallock(emu, a):
    h = a.w()
    return (h << 16) if h else 0


@api("GLOBALHANDLE")
def globalhandle(emu, a):
    s = a.w()
    return (s << 16) | s


@api("GETFREESPACE")
def getfreespace(emu, a):
    return 16 << 20


@api("MAKEPROCINSTANCE")
def makeprocinstance(emu, a):
    (s, o), _h = a.p(), a.w()
    return s << 16 | o


@api("ALLOCCSTODSALIAS")
def alloccstodsalias(emu, a):
    src = a.w()
    sel = emu.alias(src, code=False)
    return sel


@api("PRESTOCHANGOSELECTOR")
def prestochangoselector(emu, a):
    src, dst = a.w(), a.w()
    base = emu.desc_base(src)
    d = bytes(emu.uc.mem_read(0x20000000 + (src & ~7), 8))
    code = not (d[5] & 0x08)  # src is data -> dst becomes code, and vice versa
    emu.set_desc(dst, base, emu.limits.get(src & ~7, 0xFFFF), code=code)
    return dst


@api("GETPRIVATEPROFILEINT")
def getprivateprofileint(emu, a):
    (s1, o1), (s2, o2), default, (s3, o3) = a.p(), a.p(), a.i(), a.p()
    sec, key, f = emu.cstr(s1, o1), emu.cstr(s2, o2), emu.cstr(s3, o3)
    v = emu.fs.ini_get(f, sec, key)
    emu.log(f"GetPrivateProfileInt [{sec}] {key} in {f} -> {v!r} (default {default})")
    if v is None:
        return default & 0xFFFF
    try:
        return int(v.split()[0]) & 0xFFFF
    except (ValueError, IndexError):
        return 0


@api("GETPRIVATEPROFILESTRING")
def getprivateprofilestring(emu, a):
    (s1, o1), (s2, o2), (s3, o3), (s4, o4), n, (s5, o5) = a.p(), a.p(), a.p(), a.p(), a.w(), a.p()
    sec, key, default, f = emu.cstr(s1, o1), emu.cstr(s2, o2), emu.cstr(s3, o3), emu.cstr(s5, o5)
    v = emu.fs.ini_get(f, sec, key)
    emu.log(f"GetPrivateProfileString [{sec}] {key} in {f} -> {v!r} (default {default!r})")
    if v is None:
        v = default or ""
    return emu.wcstr(s4, o4, v, n)


@api("TIMERCOUNT")
def timercount(emu, a):
    s, o = a.p()
    t = emu.clock_tick()
    emu.wd(s, o + 4, t)
    emu.wd(s, o + 8, t)
    return 1


@api("GETTICKCOUNT")
def gettickcount(emu, a):
    return emu.clock_tick()


@api("__FPMATH")
def fpmath(emu, a):
    emu.log(f"__FPMATH bx={emu['bx']}")
    return 0


@api("MESSAGEBOX")
def messagebox(emu, a):
    _hwnd, (s1, o1), (s2, o2), t = a.w(), a.p(), a.p(), a.w()
    emu.log(f"MessageBox {emu.cstr(s2, o2)!r}: {emu.cstr(s1, o1)!r}")
    emu.messages.append(emu.cstr(s1, o1))
    return 1  # IDOK


@api("POSTQUITMESSAGE")
def postquitmessage(emu, a):
    emu.fail(f"PostQuitMessage({a.w()})")


@api("GETCURSORPOS")
def getcursorpos(emu, a):
    s, o = a.p()
    emu.wb(s, o, struct.pack("<hh", *emu.cursor))
    return 1


@api("SCREENTOCLIENT")
def screentoclient(emu, a):
    _h, (s, o) = a.w(), a.p()
    return 1


@api("GETFOCUS", "GETACTIVEWINDOW", "GETCAPTURE")
def getactivewindow(emu, a):
    return emu.main_hwnd


@api("GETWINDOWRECT")
def getwindowrect(emu, a):
    _h, (s, o) = a.w(), a.p()
    emu.wb(s, o, struct.pack("<hhhh", 0, 0, 640, 480))
    return 1


@api("SETRECT")
def setrect(emu, a):
    (s, o), l, t, r, b = a.p(), a.w(), a.w(), a.w(), a.w()
    emu.wb(s, o, struct.pack("<HHHH", l, t, r, b))
    return 1


@api("GETWINDOWTEXT")
def getwindowtext(emu, a):
    _h, (s, o), n = a.w(), a.p(), a.w()
    return emu.wcstr(s, o, "", n)


@api("GETSYSTEMMETRICS")
def getsystemmetrics(emu, a):
    i = a.w()
    return {0: 640, 1: 480, 4: 20, 5: 1, 6: 1, 7: 4, 8: 4, 32: 4, 33: 4, 15: 20, 16: 640, 17: 460}.get(i, 0)


@api("GETDEVICECAPS")
def getdevicecaps(emu, a):
    _hdc, i = a.w(), a.w()
    # HORZRES 8, VERTRES 10, BITSPIXEL 12, PLANES 14, RASTERCAPS 38, SIZEPALETTE 104, NUMRESERVED 106, COLORRES 108
    return {8: 640, 10: 480, 12: 8, 14: 1, 24: 256, 38: 0x0100, 104: 256, 106: 20, 108: 18}.get(i, 0)


@api("GETTEXTMETRICS")
def gettextmetrics(emu, a):
    _hdc, (s, o) = a.w(), a.p()
    emu.wb(s, o, struct.pack("<hhhhhhh", 16, 13, 3, 0, 0, 8, 16) + bytes(17))
    return 1


@api("GETPALETTEENTRIES", "GETSYSTEMPALETTEENTRIES")
def getpaletteentries(emu, a):
    _h, start, n, (s, o) = a.w(), a.w(), a.w(), a.p()
    if s:
        emu.wb(s, o, bytes(4 * n))
    return n


@api("GETKEYSTATE")
def getkeystate(emu, a):
    vk = a.w()
    return emu.key_state(vk)


@api("JOYGETPOS")
def joygetpos(emu, a):
    return 167  # JOYERR_UNPLUGGED


@api("MCISENDSTRING")
def mcisendstring(emu, a):
    (s1, o1), (s2, o2), n, _h = a.p(), a.p(), a.w(), a.w()
    emu.log(f"mciSendString {emu.cstr(s1, o1)!r}")
    if s2 and n:
        emu.wcstr(s2, o2, "", n)
    return 0


@api("WVSPRINTF")
def wvsprintf(emu, a):
    (s1, o1), (s2, o2), (s3, o3) = a.p(), a.p(), a.p()
    fmt = emu.cstr(s2, o2)
    out, i, ap = [], 0, o3
    while i < len(fmt):
        c = fmt[i]
        if c != "%":
            out.append(c)
            i += 1
            continue
        j = i + 1
        while j < len(fmt) and fmt[j] in "-#0123456789l":
            j += 1
        spec, conv = fmt[i + 1 : j], fmt[j] if j < len(fmt) else ""
        i = j + 1
        if conv == "%":
            out.append("%")
        elif conv == "s":
            ps, po = emu.rfar(s3, ap)[0], emu.rfar(s3, ap)[1]
            ap += 4
            out.append(emu.cstr(ps, po) or "")
        elif conv in "dixXuc":
            if "l" in spec:
                v = emu.rd(s3, ap)
                ap += 4
            else:
                v = emu.rw(s3, ap)
                ap += 2
            if conv in "di":
                bits = 32 if "l" in spec else 16
                if v >> (bits - 1):
                    v -= 1 << bits
            out.append(chr(v & 0xFF) if conv == "c" else (f"{v:x}" if conv == "x" else f"{v:X}" if conv == "X" else str(v)))
    return emu.wcstr(s1, o1, "".join(out))


# ---- windows -------------------------------------------------------------------------------

WM_CREATE, WM_NCCREATE = 0x0001, 0x0081


@api("REGISTERCLASS")
def registerclass(emu, a):
    s, o = a.p()
    # WNDCLASS: style, lpfnWndProc, cbClsExtra, cbWndExtra, hInstance, hIcon, hCursor, hbrBackground, lpszMenuName, lpszClassName
    proc = emu.rfar(s, o + 2)
    cs, co = emu.rfar(s, o + 22)
    name = emu.cstr(cs, co) if cs else f"#{co}"
    emu.classes[name.upper()] = bytes(emu.rb(s, o, 26))
    emu.log(f"RegisterClass {name} wndproc {proc[0]:04x}:{proc[1]:04x}")
    return len(emu.classes)


@api("GETCLASSINFO")
def getclassinfo(emu, a):
    _hinst, (cs, co), (s, o) = a.w(), a.p(), a.p()
    name = (emu.cstr(cs, co) if cs else f"#{co}").upper()
    if name in emu.classes:
        emu.wb(s, o, emu.classes[name])
        return 1
    return 0


def create_window(emu, a, ex):
    if ex:
        a.d()
    (cs, co), (ns, no), style = a.p(), a.p(), a.d()
    x, y, w, h, parent, menu, inst, (ps, po) = a.i(), a.i(), a.i(), a.i(), a.w(), a.w(), a.w(), a.p()
    cname = emu.cstr(cs, co) if cs else f"#{co}"
    wc = emu.classes.get(cname.upper())
    if wc is None:
        emu.log(f"CreateWindow of unknown class {cname}")
        return 0
    hwnd = 0x3000 + 4 * len(emu.windows)
    proc = struct.unpack_from("<HH", wc, 2)
    emu.windows[hwnd] = {"proc": (proc[1], proc[0]), "class": cname, "parent": parent, "long": {}, "props": {}}
    if not emu.main_hwnd and not parent:
        emu.main_hwnd = hwnd
    emu.log(f"CreateWindow {cname} {emu.cstr(ns, no)!r} -> {hwnd:#x}")
    # CREATESTRUCT in a scratch block
    cst = emu.scratch
    emu.wb(cst, 0, struct.pack("<HHHHHHhhhhIHHHI", po, ps, inst, menu, parent, h, w, y, x, 0, style, no, ns, co, cs)[:36])
    send_message(emu, hwnd, WM_NCCREATE, 0, cst << 16)
    r = send_message(emu, hwnd, WM_CREATE, 0, cst << 16)
    if r == 0xFFFF:
        return 0
    return hwnd


@api("CREATEWINDOW")
def createwindow(emu, a):
    return create_window(emu, a, False)


@api("CREATEWINDOWEX")
def createwindowex(emu, a):
    return create_window(emu, a, True)


def send_message(emu, hwnd, msg, wp, lp):
    w = emu.windows.get(hwnd)
    if not w:
        return 0
    sel, off = w["proc"]
    r = emu.call_far(sel, off, (hwnd, msg, wp, lp >> 16, lp & 0xFFFF), regs={"ax": emu.dgroup, "ds": emu.dgroup})
    return r & 0xFFFF


@api("SENDMESSAGE")
def sendmessage(emu, a):
    hwnd, msg, wp, lp = a.w(), a.w(), a.w(), a.d()
    return send_message(emu, hwnd, msg, wp, lp)


@api("CALLWINDOWPROC")
def callwindowproc(emu, a):
    (s, o), hwnd, msg, wp, lp = a.p(), a.w(), a.w(), a.w(), a.d()
    if (s, o) == (0, 0):
        return 0
    return emu.call_far(s, o, (hwnd, msg, wp, lp >> 16, lp & 0xFFFF), regs={"ax": emu.dgroup, "ds": emu.dgroup})


@api("DEFWINDOWPROC", "DEFDLGPROC", "DEFMDICHILDPROC")
def defwindowproc(emu, a):
    _hwnd, msg = a.w(), a.w()
    return 1 if msg == WM_NCCREATE else 0


@api("SHOWWINDOW")
def showwindow(emu, a):
    hwnd, _cmd = a.w(), a.w()
    w = emu.windows.get(hwnd)
    if w and not w.get("active") and not w["parent"]:
        w["active"] = True
        send_message(emu, hwnd, 0x0006, 1, 0)  # WM_ACTIVATE(WA_ACTIVE)
        send_message(emu, hwnd, 0x0007, 0, 0)  # WM_SETFOCUS
    return 0


@api("DESTROYWINDOW")
def destroywindow(emu, a):
    emu.windows.pop(a.w(), None)
    return 1


@api("GETWINDOWLONG")
def getwindowlong(emu, a):
    hwnd, i = a.w(), a.i()
    w = emu.windows.get(hwnd)
    if not w:
        return 0
    if i == -4:
        s, o = w["proc"]
        return s << 16 | o
    return w["long"].get(i, 0)


@api("GETWINDOWWORD")
def getwindowword(emu, a):
    hwnd, i = a.w(), a.i()
    w = emu.windows.get(hwnd)
    if i == -6:  # GWW_HINSTANCE
        return emu.dgroup
    return w["long"].get(i, 0) & 0xFFFF if w else 0


@api("SETWINDOWLONG")
def setwindowlong(emu, a):
    hwnd, i, v = a.w(), a.i(), a.d()
    w = emu.windows.get(hwnd)
    if not w:
        return 0
    if i == -4:
        s, o = w["proc"]
        w["proc"] = (v >> 16, v & 0xFFFF)
        return s << 16 | o
    old = w["long"].get(i, 0)
    w["long"][i] = v
    return old


def prop_key(emu, s, o):
    return emu.cstr(s, o) if s else f"#{o}"


@api("SETPROP")
def setprop(emu, a):
    hwnd, (s, o), h = a.w(), a.p(), a.w()
    w = emu.windows.get(hwnd)
    if w:
        w["props"][prop_key(emu, s, o)] = h
    return 1


@api("GETPROP")
def getprop(emu, a):
    hwnd, (s, o) = a.w(), a.p()
    w = emu.windows.get(hwnd)
    return w["props"].get(prop_key(emu, s, o), 0) if w else 0


@api("REMOVEPROP")
def removeprop(emu, a):
    hwnd, (s, o) = a.w(), a.p()
    w = emu.windows.get(hwnd)
    return w["props"].pop(prop_key(emu, s, o), 0) if w else 0


@api("PEEKMESSAGE")
def peekmessage(emu, a):
    emu.on_idle()
    return 0


@api("GETMESSAGE")
def getmessage(emu, a):
    emu.fail("GetMessage: the program is waiting for a message")
    return 0


@api("DISPATCHMESSAGE")
def dispatchmessage(emu, a):
    return 0


@api("DIALOGBOXPARAM", "CREATEDIALOGPARAM")
def dialogbox(emu, a):
    emu.fail("dialog box")
    return 0


# ---- WinG and sound --------------------------------------------------------------------------


@api("WINGRECOMMENDDIBFORMAT")
def wingrecommend(emu, a):
    s, o = a.p()
    emu.wb(s, o, struct.pack("<IiiHHIIiiII", 40, 320, -200, 1, 8, 0, 0, 0, 0, 0, 0))
    return 1


@api("WINGCREATEBITMAP")
def wingcreatebitmap(emu, a):
    _hdc, (s, o), (ps, po) = a.w(), a.p(), a.p()
    bits = emu.global_alloc(0x10000)
    if ps:
        emu.wb(ps, po, struct.pack("<HH", 0, bits))
    return 0x1C30


@api("WINGSETDIBCOLORTABLE")
def wingsetdibcolortable(emu, a):
    _hdc, _start, n, _p = a.w(), a.w(), a.w(), a.p()
    return n


@api("DWS_DETECTHARDWARE", "DWS_INIT")
def dws_init(emu, a):
    return 1 if emu.sound else 0


@api("DWS_DPLAY")
def dws_dplay(emu, a):
    return 1


@api("DWS_WAV2DWD")
def dws_wav2dwd(emu, a):
    return 1


# ---- DOS -----------------------------------------------------------------------------------


def interrupt(emu, n):
    if n == 0x21:
        dos(emu)
    elif n in (0x2F,):
        emu["ax"] = 0
    elif n == 0x31:
        emu.fail(f"DPMI int 31h ax={emu['ax']:04x} at {emu.where()}")
    elif n < 0x20:
        emu.fail(f"CPU exception {n} at {emu.where()}")
    else:
        emu.fail(f"int {n:02x}h ax={emu['ax']:04x} at {emu.where()}")


def dos(emu):
    ah = emu["ax"] >> 8
    al = emu["ax"] & 0xFF
    fs = emu.fs
    emu.set_carry(False)

    def err(code):
        emu["ax"] = code
        emu.set_carry(True)

    if ah == 0x30:
        emu["ax"] = 0x0005  # DOS 5.0
        emu["bx"] = 0
    elif ah == 0x19:
        emu["ax"] = (emu["ax"] & 0xFF00) | 2  # C:
    elif ah == 0x0E:
        emu["ax"] = (emu["ax"] & 0xFF00) | 26
    elif ah == 0x47:  # get current directory (without drive and leading backslash)
        emu.wcstr(emu["ds"], emu["si"], fs.cwd[3:])
        emu["ax"] = 0x0100
    elif ah == 0x3B:
        path = fs.full(emu.cstr(emu["ds"], emu["dx"]))
        if fs.isdir(path):
            fs.cwd = path.rstrip("\\") if len(path) > 3 else path
        else:
            err(3)
    elif ah == 0x2A:
        emu["cx"], emu["dx"], emu["ax"] = 1998, 3 << 8 | 6, (emu["ax"] & 0xFF00) | 5
    elif ah == 0x2C:
        t = emu.clock_ms // 10
        emu["cx"] = ((t // 360000) % 24) << 8 | (t // 6000) % 60
        emu["dx"] = ((t // 100) % 60) << 8 | t % 100
    elif ah == 0x25:
        pass
    elif ah == 0x35:
        emu["es"], emu["bx"] = 0, 0
    elif ah == 0x1A:
        emu.dta = (emu["ds"], emu["dx"])
    elif ah == 0x2F:
        emu["es"], emu["bx"] = emu.dta
    elif ah in (0x3D, 0x3C, 0x5B):
        name = emu.cstr(emu["ds"], emu["dx"])
        h = fs.open(name, create=ah != 0x3D, mode=al & 3 if ah == 0x3D else 2)
        emu.log(f"DOS open {name!r} mode {al:#x} create={ah != 0x3D} -> {h}")
        if h is None:
            err(2)
        else:
            emu["ax"] = h
    elif ah == 0x3E:
        if not fs.close(emu["bx"]):
            err(6)
    elif ah == 0x3F:
        data = fs.read(emu["bx"], emu["cx"])
        if data is None:
            err(6)
        else:
            emu.wb(emu["ds"], emu["dx"], data)
            emu["ax"] = len(data)
    elif ah == 0x40:
        data = emu.rb(emu["ds"], emu["dx"], emu["cx"])
        n = fs.write(emu["bx"], data)
        if n is None:
            err(6)
        else:
            emu["ax"] = n
    elif ah == 0x42:
        pos = fs.seek(emu["bx"], (emu["cx"] << 16) | emu["dx"], al)
        if pos is None:
            err(6)
        else:
            emu["dx"], emu["ax"] = pos >> 16, pos & 0xFFFF
    elif ah == 0x43:
        name = emu.cstr(emu["ds"], emu["dx"])
        p = fs.resolve(name)
        if p is None:
            err(2)
        else:
            emu["cx"] = 0x10 if p.is_dir() else 0x01
    elif ah == 0x44:
        if al == 0:
            emu["dx"] = 0x80 if emu["bx"] < 3 else 0x02
        else:
            emu["ax"] = 0
    elif ah == 0x45:
        h = fs.dup(emu["bx"])
        emu["ax"] = h
    elif ah == 0x41:  # delete: only files the game wrote (in the overlay)
        p = fs.writable(emu.cstr(emu["ds"], emu["dx"]))
        emu.log(f"DOS delete {emu.cstr(emu['ds'], emu['dx'])!r} -> {p}")
        if p is None:
            err(5)
        else:
            p.unlink()
    elif ah == 0x56:  # rename ds:dx -> es:di, inside the overlay
        a = fs.writable(emu.cstr(emu["ds"], emu["dx"]))
        b = fs.full(emu.cstr(emu["es"], emu["di"]))
        emu.log(f"DOS rename {a} -> {b}")
        gd = fs.game_dir.upper()
        if a is None or not b.startswith(gd + "\\"):
            err(5)
        else:
            target = fs.overlay / b[len(gd) + 1 :].replace("\\", "/")
            target = next((c for c in target.parent.iterdir() if c.name.upper() == target.name.upper()), target)
            a.rename(target)
    elif ah == 0x36:
        emu["ax"], emu["bx"], emu["cx"], emu["dx"] = 64, 0x7FFF, 512, 0xFFFF
    elif ah == 0x4E:
        name = emu.cstr(emu["ds"], emu["dx"])
        emu.find[emu.dta] = fs.find(name, emu["cx"])
        emu.log(f"DOS findfirst {name!r} attr {emu['cx']:#x}: {len(emu.find[emu.dta])} found")
        find_next(emu, err)
    elif ah == 0x4F:
        find_next(emu, err)
    elif ah == 0x59:
        emu["ax"] = 2
    elif ah == 0x4C:
        emu.fail(f"DOS exit {al}")
    elif ah == 0x0D:
        pass
    elif ah == 0x57:
        emu["cx"], emu["dx"] = 0, (18 << 9) | (3 << 5) | 6
    else:
        emu.fail(f"unimplemented int 21h ah={ah:02x} at {emu.where()}")


def find_next(emu, err):
    found = emu.find.get(emu.dta)
    if not found:
        err(18)
        return
    p = found.pop(0)
    s, o = emu.dta
    attr = 0x10 if p.is_dir() else 0x20
    size = 0 if p.is_dir() else p.stat().st_size
    name = p.name.upper().encode("latin-1")[:12]
    emu.wb(s, o + 0x15, bytes([attr]) + struct.pack("<HHI", 0, (18 << 9) | (3 << 5) | 6, size) + name + b"\0")


class FileSystem:
    """DOS paths -> host files. C:\\ETHIS is the CD; writes go to `overlay`."""

    def __init__(self, root, overlay, game_dir="C:\\ETHIS"):
        self.root = Path(root)
        self.overlay = Path(overlay)
        self.game_dir = game_dir
        self.cwd = game_dir
        self.exe_path = game_dir + "\\ETLAUNCH.EXE"
        self.files = {}
        self.next = 5
        self.ini = {}  # KEY -> value: overrides for GetPrivateProfile*
        self.hidden = ["\\VIDEOS\\"]  # no MCI: pretend the videos are missing, so they are skipped

    def full(self, name):
        name = name.replace("/", "\\")
        if len(name) >= 2 and name[1] == ":":
            drive, rest = name[:2].upper(), name[2:]
        else:
            drive, rest = "C:", name
        if not rest.startswith("\\"):
            base = self.cwd if drive == "C:" else drive + "\\"
            rest = base[2:].rstrip("\\") + "\\" + rest
        parts = []
        for p in rest.split("\\"):
            if p in ("", "."):
                continue
            if p == "..":
                if parts:
                    parts.pop()
                continue
            parts.append(p.upper())
        return drive + "\\" + "\\".join(parts)

    def _host(self, full, base):
        gd = self.game_dir.upper()
        if not (full == gd or full.startswith(gd + "\\")):
            return None
        rel = full[len(gd) + 1 :].split("\\") if full != gd else []
        p = base
        for part in rel:
            if not p.is_dir():
                return None
            for c in p.iterdir():
                if c.name.upper() == part:
                    p = c
                    break
            else:
                return p / part, False
        return p, True

    def resolve(self, name, must_exist=True):
        full = self.full(name)
        if any(h in full for h in self.hidden):
            return None
        for base in (self.overlay, self.root):
            r = self._host(full, base)
            if r and r[1]:
                return r[0]
        return None

    def writable(self, name):
        """The host file for `name` if it exists in the overlay (a file the game may change)."""
        p = self.resolve(name)
        if p is None or p.is_dir() or self.overlay.resolve() not in p.resolve().parents:
            return None
        return p

    def isdir(self, full):
        p = self.resolve(full)
        return p is not None and p.is_dir()

    def open(self, name, create=False, mode=0):
        """Read-only opens and mode-2 opens of existing files read the CD (or the overlay);
        creating a file writes it into the overlay."""
        full = self.full(name)
        if create or mode == 1:
            gd = self.game_dir.upper()
            if not full.startswith(gd + "\\"):
                return None
            target = self.overlay / full[len(gd) + 1 :].replace("\\", "/")
            target.parent.mkdir(parents=True, exist_ok=True)
            f = open(target, "w+b")
        else:
            p = self.resolve(name)
            if p is None or p.is_dir():
                return None
            f = open(p, "r+b" if mode == 2 and self.writable(name) else "rb")  # read/write opens of files the game wrote
        h = 5
        while h in self.files:
            h += 1
        self.files[h] = f
        return h

    def close(self, h):
        f = self.files.pop(h, None)
        if f:
            f.close()
            return True
        return h < 5

    def read(self, h, n):
        if h == 0:
            return b""
        f = self.files.get(h)
        return f.read(n) if f else None

    def write(self, h, data):
        if h in (1, 2):
            sys_stdout_write(data)
            return len(data)
        f = self.files.get(h)
        if not f:
            return None
        if not data:
            f.truncate()
            return 0
        return f.write(data)

    def seek(self, h, pos, whence):
        f = self.files.get(h)
        if not f:
            return None
        if whence == 1 and pos & 0x80000000:
            pos -= 1 << 32
        return f.seek(pos, whence)

    def dup(self, h):
        f = self.files.get(h)
        n = 5
        while n in self.files:
            n += 1
        self.files[n] = open(f.name, f.mode) if f else None
        return n

    def find(self, pattern, attr):
        import fnmatch

        full = self.full(pattern)
        d, _, pat = full.rpartition("\\")
        out = {}
        for base in (self.root, self.overlay):
            r = self._host(d, base)
            if not r or not r[1] or not r[0].is_dir():
                continue
            for c in sorted(r[0].iterdir()):
                n = c.name.upper()
                if c.is_dir() and not attr & 0x10:
                    continue
                if fnmatch.fnmatch(n, pat.replace("*.*", "*")) or (pat == "*.*"):
                    out[n] = c
        return list(out.values())

    def ini_get(self, fname, sec, key):
        if key and key.upper() in self.ini:
            return self.ini[key.upper()]
        if not fname:
            return None
        if "\\" not in fname and ":" not in fname:
            fname = "C:\\WINDOWS\\" + fname
        p = self.resolve(fname)
        if p is None or sec is None or key is None:
            return None
        cur = None
        for line in p.read_text("latin-1").splitlines():
            line = line.strip()
            if line.startswith("["):
                cur = line[1:].split("]")[0].upper()
            elif "=" in line and cur == sec.upper():
                k, v = line.split("=", 1)
                if k.strip().upper() == key.upper():
                    return v.strip()
        return None


def sys_stdout_write(data):
    import sys

    sys.stderr.write(data.decode("latin-1"))
