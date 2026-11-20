//! Intel Indeo Video 3 (fourcc IV31/IV32), the codec of every VIDEOS/*.AVI.
//!
//! A port of FFmpeg's libavcodec/indeo3.c and indeo3data.h (Maxim Poliakovski, 2009-2011,
//! LGPL-2.1-or-later), so this file is LGPL-2.1-or-later too. Output = YUV 4:1:0 with 7-bit
//! samples doubled, byte for byte what `ffmpeg -pix_fmt yuv410p` gives (tools: see re/frontend.md).
//!
//! Frame buffers hold 7-bit pixels; the SWAR word arithmetic of the C code (16/32/64-bit
//! little-endian adds that may carry across bytes, then a 0x7f mask) is kept as it is.

use std::sync::OnceLock;

const BS_8BIT_PEL: u16 = 1 << 1;
const BS_MV_Y_HALF: u16 = 1 << 4;
const BS_MV_X_HALF: u16 = 1 << 5;
const BS_BUFFER: u16 = 9;

#[derive(Clone, Copy, Default)]
struct Plane {
    /// Offsets in `Decoder::buf` of the two buffers' first pixel (one prediction line above).
    off: [usize; 2],
    width: usize,
    height: usize,
    pitch: usize,
}

#[derive(Clone, Copy)]
struct Cell {
    xpos: i32,
    ypos: i32,
    width: i32,
    height: i32,
    /// 0: motion-compensation tree, 1: VQ tree.
    tree: u8,
    /// (mv_y, mv_x) of an INTER cell.
    mv: Option<(i32, i32)>,
}

struct Vq {
    deltas: Vec<u16>,
    m10: Vec<u32>,
    num_dyads: u32,
    quad_exp: u32,
}

pub struct Decoder {
    buf: Vec<u8>,
    planes: [Plane; 3],
    pub width: usize,
    pub height: usize,
    buf_sel: usize,
    cb_offset: u8,
    alt_quant: [u8; 16],
    /// A frame was decoded (else there is nothing to show yet).
    pub have_frame: bool,
}

type Res<T> = Result<T, String>;

// ---- word access (bounds-safe: out-of-range reads give 0, writes are dropped) ----------------

#[inline]
fn rd<const N: usize>(b: &[u8], i: isize) -> u64 {
    let Ok(i) = usize::try_from(i) else { return 0 };
    match b.get(i..i + N) {
        Some(s) => {
            let mut v = [0u8; 8];
            v[..N].copy_from_slice(s);
            u64::from_le_bytes(v)
        }
        None => 0,
    }
}

#[inline]
fn wr<const N: usize>(b: &mut [u8], i: isize, v: u64) {
    let Ok(i) = usize::try_from(i) else { return };
    if let Some(s) = b.get_mut(i..i + N) {
        s.copy_from_slice(&v.to_le_bytes()[..N]);
    }
}

#[inline]
fn rd16(b: &[u8], i: isize) -> u16 {
    rd::<2>(b, i) as u16
}
#[inline]
fn rd32(b: &[u8], i: isize) -> u32 {
    rd::<4>(b, i) as u32
}
#[inline]
fn rd64(b: &[u8], i: isize) -> u64 {
    rd::<8>(b, i)
}
#[inline]
fn wr16(b: &mut [u8], i: isize, v: u16) {
    wr::<2>(b, i, v as u64)
}
#[inline]
fn wr32(b: &mut [u8], i: isize, v: u32) {
    wr::<4>(b, i, v as u64)
}
#[inline]
fn wr64(b: &mut [u8], i: isize, v: u64) {
    wr::<8>(b, i, v)
}

fn avg32(b: &mut [u8], dst: isize, src: isize, r: isize) {
    let v = (rd32(b, src).wrapping_add(rd32(b, r)) >> 1) & 0x7f7f_7f7f;
    wr32(b, dst, v);
}

fn avg64(b: &mut [u8], dst: isize, src: isize, r: isize) {
    let v = (rd64(b, src).wrapping_add(rd64(b, r)) >> 1) & 0x7f7f_7f7f_7f7f_7f7f;
    wr64(b, dst, v);
}

fn replicate64(a: u64) -> u64 {
    let a = a & 0x00ff_00ff_00ff_00ff;
    a | (a << 8)
}

fn replicate32(a: u32) -> u32 {
    let a = a & 0x00ff_00ff;
    a | (a << 8)
}

fn fill64(b: &mut [u8], mut dst: isize, pix: u64, n: i32, pitch: isize) {
    for _ in 0..n {
        wr64(b, dst, pix);
        dst += pitch;
    }
}

fn copy_block4(b: &mut [u8], mut dst: isize, mut src: isize, pitch: isize, h: i32) {
    for _ in 0..h {
        let v = rd32(b, src);
        wr32(b, dst, v);
        dst += pitch;
        src += pitch;
    }
}

// ---- tables ---------------------------------------------------------------------------------

fn requant_tab() -> &'static [[u8; 128]; 8] {
    static T: OnceLock<[[u8; 128]; 8]> = OnceLock::new();
    T.get_or_init(|| {
        const OFFSETS: [i32; 8] = [1, 1, 2, -3, -3, 3, 4, 4];
        const DELTAS: [i32; 8] = [0, 1, 0, 4, 4, 1, 0, 1];
        let mut t = [[0u8; 128]; 8];
        for i in 0..8 {
            let step = i as i32 + 2;
            for j in 0..128 {
                // C division truncates toward zero.
                t[i][j] = ((j as i32 + OFFSETS[i]) / step * step + DELTAS[i]) as u8;
            }
        }
        t[0][127] = 126;
        t[1][119] = 118;
        t[1][120] = 118;
        t[2][126] = 124;
        t[2][127] = 124;
        t[6][124] = 120;
        t[6][125] = 120;
        t[6][126] = 120;
        t[6][127] = 120;
        // Compatibility with Intel's binary decoders.
        t[1][7] = 10;
        t[4][8] = 10;
        t
    })
}

fn vq_tab() -> &'static [Vq] {
    static T: OnceLock<Vec<Vq>> = OnceLock::new();
    T.get_or_init(|| {
        let sets: [(&[(u8, i8, i8)], usize, u32, u32); 21] = [
            (TAB_1_1, 195, 195, 7),
            (TAB_1_2, 159, 159, 9),
            (TAB_1_3, 133, 133, 10),
            (TAB_1_4, 115, 115, 11),
            (TAB_1_5, 101, 101, 12),
            (TAB_1_6, 93, 93, 12),
            (TAB_1_7, 87, 87, 12),
            (TAB_1_8, 77, 77, 13),
            (TAB_2_1, 195, 195, 7),
            (TAB_2_2, 159, 159, 9),
            (TAB_2_3, 133, 133, 10),
            (TAB_2_4, 115, 115, 11),
            (TAB_2_5, 101, 101, 12),
            (TAB_2_6, 93, 93, 12),
            (TAB_2_7, 87, 87, 12),
            (TAB_2_8, 77, 77, 13),
            (TAB_3_1, 128, 128, 11),
            (TAB_3_2, 79, 79, 13),
            (TAB_3_3, 79, 79, 13),
            (TAB_3_4, 79, 79, 13),
            (TAB_3_5, 79, 79, 13),
        ];
        let mut v: Vec<Vq> = sets
            .iter()
            .map(|&(tab, size, num_dyads, quad_exp)| {
                let mut pairs = Vec::new();
                for &(kind, a, b) in tab {
                    let (a, b) = (a as i64, b as i64);
                    pairs.push((a, b));
                    if kind >= 2 {
                        pairs.push((-a, -b));
                    }
                    if kind == 4 {
                        pairs.push((b, a));
                        pairs.push((-b, -a));
                    }
                }
                pairs.resize(size, (0, 0));
                Vq {
                    deltas: pairs.iter().map(|&(a, b)| (b * 256 + a) as i16 as u16).collect(),
                    m10: pairs.iter().map(|&(a, b)| (b * (1 << 24) + b * (1 << 16) + a * 256 + a) as i32 as u32).collect(),
                    num_dyads,
                    quad_exp,
                }
            })
            .collect();
        // Entries 21..23 repeat table 3_5.
        for _ in 0..3 {
            let last = &v[20];
            let copy = Vq { deltas: last.deltas.clone(), m10: last.m10.clone(), num_dyads: last.num_dyads, quad_exp: last.quad_exp };
            v.push(copy);
        }
        v
    })
}

// ---- bitstream ------------------------------------------------------------------------------

/// The state of one plane's decode: `d` = the plane's data after its vector count.
struct Ctx<'a> {
    d: &'a [u8],
    num_vectors: usize,
    /// The bit reader starts at byte `num_vectors * 2` of `d`.
    bit: usize,
    bits: usize,
    skip_bits: usize,
    need_resync: bool,
    next_cell: usize,
}

impl Ctx<'_> {
    fn get_bits(&mut self, n: usize) -> u32 {
        let mut v = 0;
        for _ in 0..n {
            let at = self.num_vectors * 2 + (self.bit >> 3);
            let byte = self.d.get(at).copied().unwrap_or(0);
            v = (v << 1) | ((byte >> (7 - (self.bit & 7))) & 1) as u32;
            self.bit += 1;
        }
        v
    }

    fn resync(&mut self) {
        if self.need_resync && self.bit & 7 == 0 {
            self.bit += self.skip_bits;
            self.skip_bits = 0;
            self.need_resync = false;
        }
    }

    fn update_bitpos(&mut self, n: usize) {
        self.skip_bits += n;
        self.need_resync = true;
    }

    fn byte_pos(&self) -> usize {
        self.num_vectors * 2 + ((self.bit + 7) >> 3)
    }
}

const RLE_ESC_F9: u8 = 249;
const RLE_ESC_FA: u8 = 250;
const RLE_ESC_FB: u8 = 251;
const RLE_ESC_FC: u8 = 252;
const RLE_ESC_FD: u8 = 253;
const RLE_ESC_FE: u8 = 254;
const RLE_ESC_FF: u8 = 255;

impl Decoder {
    pub fn new() -> Decoder {
        Decoder {
            buf: Vec::new(),
            planes: [Plane::default(); 3],
            width: 0,
            height: 0,
            buf_sel: 0,
            cb_offset: 0,
            alt_quant: [0; 16],
            have_frame: false,
        }
    }

    fn allocate(&mut self, w: usize, h: usize) {
        self.width = w;
        self.height = h;
        let cw = ((w >> 2) + 3) & !3;
        let ch = ((h >> 2) + 3) & !3;
        let lp = (w + 15) & !15;
        let cp = (cw + 15) & !15;
        let mut buf = Vec::new();
        for p in 0..3 {
            let (pw, ph, pitch) = if p == 0 { (w, h, lp) } else { (cw, ch, cp) };
            let mut off = [0; 2];
            for o in off.iter_mut() {
                // The INTRA prediction line (64), the plane, slack rows.
                buf.extend(std::iter::repeat(0x40).take(pitch));
                *o = buf.len();
                buf.extend(std::iter::repeat(0).take(pitch * (ph + 8)));
            }
            self.planes[p] = Plane { off, width: pw, height: ph, pitch };
        }
        self.buf = buf;
        self.have_frame = false;
    }

    /// Decodes one frame; Ok(false) for a sync (null) frame that changes nothing.
    pub fn decode(&mut self, pkt: &[u8]) -> Res<bool> {
        let le16 = |i: usize| -> u32 { pkt.get(i..i + 2).map_or(0, |s| u16::from_le_bytes([s[0], s[1]]) as u32) };
        let le32 = |i: usize| -> u32 { pkt.get(i..i + 4).map_or(0, |s| u32::from_le_bytes([s[0], s[1], s[2], s[3]])) };
        if pkt.len() < 16 {
            return Ok(false);
        }
        let (frame_num, word2, check_sum, data_size) = (le32(0), le32(4), le32(8), le32(12));
        if frame_num ^ word2 ^ data_size ^ u32::from_be_bytes(*b"FRMH") != check_sum {
            return Err("OS header checksum mismatch".into());
        }
        let bs = 16;
        if le16(bs) != 32 {
            return Err("unsupported codec version".into());
        }
        let flags = le16(bs + 2) as u16;
        let mut size = ((le32(bs + 4) as usize) + 7) >> 3;
        self.cb_offset = pkt[bs + 8];
        if size == 16 {
            return Ok(false);
        }
        size = size.min(pkt.len() - 16);
        let height = le16(bs + 12) as usize;
        let width = le16(bs + 14) as usize;
        if width != self.width || height != self.height {
            if !(16..=640).contains(&width) || !(16..=480).contains(&height) || width & 3 != 0 || height & 3 != 0 {
                return Err(format!("invalid picture dimensions {width}x{height}"));
            }
            self.allocate(width, height);
        }
        let starts = [le32(bs + 16) as usize, le32(bs + 20) as usize, le32(bs + 24) as usize]; // y, v, u
        let mut ends = [size; 3];
        for j in 0..3 {
            for i in (0..3).rev() {
                if starts[i] < ends[j] && starts[i] > starts[j] {
                    ends[j] = starts[i];
                }
            }
        }
        let hdr_end = 32; // the bitstream header up to the alt_quant table
        if starts.iter().any(|&s| s + 16 >= size || s < hdr_end + 16) || (0..3).any(|j| ends[j] <= starts[j]) {
            return Err("invalid y/u/v offsets".into());
        }
        self.alt_quant.copy_from_slice(pkt.get(bs + hdr_end..bs + hdr_end + 16).ok_or("short frame")?);
        if flags & BS_8BIT_PEL != 0 {
            return Err("8-bit pixel format".into());
        }
        if flags & (BS_MV_X_HALF | BS_MV_Y_HALF) != 0 {
            return Err("half-pel motion vectors".into());
        }
        self.buf_sel = ((flags >> BS_BUFFER) & 1) as usize;
        let plane_data = |i: usize| pkt.get(bs + starts[i]..(bs + ends[i]).min(pkt.len())).unwrap_or(&[]);
        self.decode_plane(0, plane_data(0), 40)?;
        self.decode_plane(1, plane_data(2), 10)?; // U
        self.decode_plane(2, plane_data(1), 10)?; // V
        self.have_frame = true;
        Ok(true)
    }

    /// The current picture as YUV 4:1:0 planes (width x height, then two (width/4) x (height/4)).
    pub fn yuv(&self, out: &mut Vec<u8>) {
        out.clear();
        let ch = (self.height + 3) >> 2;
        for (p, rows) in self.planes.iter().zip([self.height, ch, ch]) {
            let base = p.off[self.buf_sel];
            for y in 0..rows.min(p.height) {
                let row = &self.buf[base + y * p.pitch..base + y * p.pitch + p.width];
                out.extend(row.iter().map(|&v| (v & 0x7f) << 1));
            }
        }
    }

    /// The current picture as RGB24 (BT.601, nearest chroma).
    pub fn rgb(&self, out: &mut [u8]) {
        let (w, h) = (self.width, self.height);
        let [py, pu, pv] = self.planes;
        let (by, bu, bv) = (py.off[self.buf_sel], pu.off[self.buf_sel], pv.off[self.buf_sel]);
        for y in 0..h {
            for x in 0..w {
                let yy = ((self.buf[by + y * py.pitch + x] & 0x7f) as i32) << 1;
                let ci = (y >> 2) * pu.pitch + (x >> 2);
                let u = (((self.buf[bu + ci] & 0x7f) as i32) << 1) - 128;
                let v = (((self.buf[bv + ci] & 0x7f) as i32) << 1) - 128;
                let c = 298 * (yy - 16);
                let px = &mut out[(y * w + x) * 3..(y * w + x) * 3 + 3];
                px[0] = ((c + 409 * v + 128) >> 8).clamp(0, 255) as u8;
                px[1] = ((c - 100 * u - 208 * v + 128) >> 8).clamp(0, 255) as u8;
                px[2] = ((c + 516 * u + 128) >> 8).clamp(0, 255) as u8;
            }
        }
    }

    fn decode_plane(&mut self, p: usize, data: &[u8], strip_width: i32) -> Res<()> {
        if data.len() < 4 {
            return Err("short plane".into());
        }
        let num_vectors = u32::from_le_bytes([data[0], data[1], data[2], data[3]]) as usize;
        let d = &data[4..];
        if num_vectors > 256 || num_vectors * 2 > d.len() {
            return Err("invalid number of motion vectors".into());
        }
        let mut ctx =
            Ctx { d, num_vectors, bit: 0, bits: (d.len() - num_vectors * 2) * 8, skip_bits: 0, need_resync: false, next_cell: 0 };
        let pl = self.planes[p];
        let mut cell = Cell { xpos: 0, ypos: 0, width: (pl.width >> 2) as i32, height: (pl.height >> 2) as i32, tree: 0, mv: None };
        self.parse_bintree(&mut ctx, p, 2, &mut cell, 20, strip_width)
    }

    fn parse_bintree(&mut self, ctx: &mut Ctx, p: usize, code: u32, ref_cell: &mut Cell, depth: i32, strip_width: i32) -> Res<()> {
        if depth <= 0 {
            return Err("corrupted binary tree".into());
        }
        let split = |size: i32| if size > 2 { ((size + 2) >> 2) << 1 } else { 1 };
        let mut cur = *ref_cell;
        if code == 0 {
            cur.height = split(ref_cell.height);
            ref_cell.ypos += cur.height;
            ref_cell.height -= cur.height;
            if ref_cell.height <= 0 || cur.height <= 0 {
                return Err("bad split".into());
            }
        } else if code == 1 {
            if cur.width > strip_width {
                cur.width = if cur.width <= strip_width << 1 { 1 } else { 2 } * strip_width;
            } else {
                cur.width = split(ref_cell.width);
            }
            ref_cell.xpos += cur.width;
            ref_cell.width -= cur.width;
            if ref_cell.width <= 0 || cur.width <= 0 {
                return Err("bad split".into());
            }
        }
        let pl = self.planes[p];
        let check = |c: &Cell| -> Res<()> {
            if c.xpos + c.width > (pl.width >> 2) as i32 || c.ypos + c.height > (pl.height >> 2) as i32 {
                Err("invalid cell".into())
            } else {
                Ok(())
            }
        };
        while ctx.bits >= ctx.bit + 2 {
            ctx.resync();
            let code = ctx.get_bits(2);
            match code {
                0 | 1 => self.parse_bintree(ctx, p, code, &mut cur, depth - 1, strip_width)?,
                2 => {
                    if cur.tree == 0 {
                        cur.mv = None;
                        cur.tree = 1;
                    } else {
                        ctx.resync();
                        if ctx.get_bits(2) >= 2 {
                            return Err("invalid VQ_NULL code".into());
                        }
                        check(&cur)?;
                        if cur.mv.is_none() {
                            return Err("VQ_NULL in an INTRA cell".into());
                        }
                        return self.copy_cell(&pl, &cur);
                    }
                }
                _ => {
                    if !ctx.need_resync {
                        ctx.next_cell = ctx.byte_pos();
                    }
                    if cur.tree == 0 {
                        if ctx.next_cell >= ctx.d.len() {
                            return Err("motion vector out of array".into());
                        }
                        let idx = ctx.d[ctx.next_cell] as usize;
                        ctx.next_cell += 1;
                        if idx >= ctx.num_vectors {
                            return Err("motion vector index out of range".into());
                        }
                        cur.mv = Some((ctx.d[idx * 2] as i8 as i32, ctx.d[idx * 2 + 1] as i8 as i32));
                        cur.tree = 1;
                        ctx.update_bitpos(8);
                    } else {
                        check(&cur)?;
                        let used = self.decode_cell(&pl, &cur, ctx.d, ctx.next_cell)?;
                        ctx.update_bitpos(used << 3);
                        ctx.next_cell += used;
                        return Ok(());
                    }
                }
            }
        }
        Err("out of tree data".into())
    }

    fn mv_check(pl: &Plane, cell: &Cell, mv_y: i32, mv_x: i32) -> Res<()> {
        if (cell.ypos << 2) + mv_y < -1
            || (cell.xpos << 2) + mv_x < 0
            || ((cell.ypos + cell.height) << 2) + mv_y > pl.height as i32
            || ((cell.xpos + cell.width) << 2) + mv_x > pl.width as i32
        {
            return Err("motion vectors point out of the frame".into());
        }
        Ok(())
    }

    fn copy_cell(&mut self, pl: &Plane, cell: &Cell) -> Res<()> {
        let (mv_y, mv_x) = cell.mv.unwrap_or((0, 0));
        Self::mv_check(pl, cell, mv_y, mv_x)?;
        let pitch = pl.pitch as isize;
        let off_dst = ((cell.ypos << 2) as isize) * pitch + (cell.xpos << 2) as isize;
        let dst = pl.off[self.buf_sel] as isize + off_dst;
        let src = pl.off[self.buf_sel ^ 1] as isize + off_dst + mv_y as isize * pitch + mv_x as isize;
        let w = (cell.width << 2) as usize;
        for row in 0..(cell.height << 2) as isize {
            let (s, t) = ((src + row * pitch) as usize, (dst + row * pitch) as usize);
            self.buf.copy_within(s..s + w, t);
        }
        Ok(())
    }

    /// Returns the number of bytes used from `d[start..]`.
    fn decode_cell(&mut self, pl: &Plane, cell: &Cell, d: &[u8], start: usize) -> Res<usize> {
        let code = *d.get(start).ok_or("out of data")?;
        let mut ptr = start + 1;
        let mode = (code >> 4) as i32;
        let mut vq_index = (code & 0xf) as usize;
        let pitch = pl.pitch as isize;
        let offset = ((cell.ypos << 2) as isize) * pitch + (cell.xpos << 2) as isize;
        let block = pl.off[self.buf_sel] as isize + offset;
        let mut ref_block: Option<isize> = None;
        match cell.mv {
            None => ref_block = Some(block - pitch),
            Some(_) if mode >= 10 => self.copy_cell(pl, cell)?,
            Some((mv_y, mv_x)) => {
                Self::mv_check(pl, cell, mv_y, mv_x)?;
                ref_block = Some(pl.off[self.buf_sel ^ 1] as isize + offset + mv_y as isize * pitch + mv_x as isize);
            }
        }
        let cb = self.cb_offset as usize;
        let (prim, second) = if mode == 1 || mode == 4 {
            let c = self.alt_quant[vq_index];
            ((c >> 4) as usize + cb, (c & 0xf) as usize + cb)
        } else {
            vq_index += cb;
            (vq_index, vq_index)
        };
        if prim >= 24 || second >= 24 {
            return Err("invalid VQ table indexes".into());
        }
        let tabs = vq_tab();
        let delta = [&tabs[second], &tabs[prim]];
        let swap_quads = [second >= 16, prim >= 16];
        if vq_index >= 8 {
            if let Some(r) = ref_block {
                let rq = &requant_tab()[vq_index & 7];
                for x in 0..(cell.width << 2) as isize {
                    if let Ok(i) = usize::try_from(r + x) {
                        if let Some(b) = self.buf.get_mut(i) {
                            *b = rq[(*b & 127) as usize];
                        }
                    }
                }
            }
        }
        let rb = ref_block.unwrap_or(0);
        let r = match mode {
            0 | 1 | 3 | 4 => {
                if mode >= 3 && cell.mv.is_some() {
                    return Err("mode 3/4 in an INTER cell".into());
                }
                self.decode_cell_data(cell, block, rb, pitch, 0, (mode >= 3) as i32, mode, delta, swap_quads, d, &mut ptr)
            }
            10 | 11 => {
                if mode == 10 && cell.mv.is_none() {
                    self.decode_cell_data(cell, block, rb, pitch, 1, 1, mode, delta, swap_quads, d, &mut ptr)
                } else {
                    if mode == 11 && cell.mv.is_none() {
                        return Err("mode 11 in an INTRA cell".into());
                    }
                    self.decode_cell_data(cell, block, rb, pitch, (mode == 10) as i32, 1, mode, delta, swap_quads, d, &mut ptr)
                }
            }
            _ => return Err(format!("unsupported coding mode {mode}")),
        };
        r.map_err(|e| format!("mode {mode}: {e}"))?;
        Ok(ptr - start)
    }

    #[allow(clippy::too_many_arguments)]
    fn decode_cell_data(
        &mut self,
        cell: &Cell,
        mut block: isize,
        mut ref_block: isize,
        row: isize,
        h_zoom: i32,
        v_zoom: i32,
        mode: i32,
        delta: [&Vq; 2],
        swap_quads: [bool; 2],
        d: &[u8],
        ptr: &mut usize,
    ) -> Res<()> {
        let b = &mut self.buf;
        let inter = cell.mv.is_some();
        let mut rle_blocks = 0i32;
        let mut skip_flag = false;
        let mut is_first_row = true;
        let blk_row_offset = (row << (2 + v_zoom)) - ((cell.width as isize) << 2);
        let line_offset = if v_zoom != 0 { row } else { 0 };
        if cell.height & v_zoom != 0 || cell.width & h_zoom != 0 {
            return Err("invalid VQ data".into());
        }
        let next = |ptr: &mut usize| -> Res<u8> {
            let v = *d.get(*ptr).ok_or("attempt to read past end of buffer")?;
            *ptr += 1;
            Ok(v)
        };
        let mut y = 0;
        while y < cell.height {
            let mut x = 0;
            while x < cell.width {
                let mut r = ref_block;
                let mut dst = block;
                if rle_blocks > 0 {
                    if mode <= 4 {
                        if inter || !skip_flag {
                            copy_block4(b, dst, r, row, 4 << v_zoom);
                        }
                    } else if mode == 10 && !inter {
                        let mut pix = rd64(b, r);
                        if is_first_row {
                            pix = replicate64(pix);
                            fill64(b, dst + row, pix, 7, row);
                            avg64(b, dst, r, dst + row);
                        } else {
                            fill64(b, dst, pix, 8, row);
                        }
                    }
                    rle_blocks -= 1;
                } else {
                    let mut line = 0;
                    while line < 4 {
                        let mut num_lines = 1;
                        let is_top_of_cell = is_first_row && line == 0;
                        let tab = if mode <= 4 { delta[(line & 1) as usize] } else { delta[1] };
                        let mut code = next(ptr)?;
                        if code < 248 {
                            let (dyad1, dyad2);
                            if (code as u32) < tab.num_dyads {
                                let d1 = next(ptr)? as u32;
                                if d1 >= tab.num_dyads || d1 >= 248 {
                                    return Err("invalid VQ data".into());
                                }
                                dyad1 = d1 as usize;
                                dyad2 = code as usize;
                            } else {
                                let c = code as u32 - tab.num_dyads;
                                let (mut d1, mut d2) = ((c / tab.quad_exp) as usize, (c % tab.quad_exp) as usize);
                                if swap_quads[(line & 1) as usize] {
                                    std::mem::swap(&mut d1, &mut d2);
                                }
                                dyad1 = d1;
                                dyad2 = d2;
                            }
                            if mode <= 4 {
                                let v = (rd16(b, r).wrapping_add(tab.deltas[dyad1])) & 0x7f7f;
                                wr16(b, dst + line_offset, v);
                                let v = (rd16(b, r + 2).wrapping_add(tab.deltas[dyad2])) & 0x7f7f;
                                wr16(b, dst + line_offset + 2, v);
                                if mode >= 3 {
                                    if is_top_of_cell && cell.ypos == 0 {
                                        let v = rd32(b, dst + row);
                                        wr32(b, dst, v);
                                    } else {
                                        avg32(b, dst, r, dst + row);
                                    }
                                }
                            } else if mode == 10 && !inter {
                                let (mut a, mut c) = (rd32(b, r), rd32(b, r + 4));
                                if is_top_of_cell {
                                    a = replicate32(a);
                                    c = replicate32(c);
                                }
                                wr32(b, dst + row, a.wrapping_add(tab.m10[dyad1]) & 0x7f7f_7f7f);
                                wr32(b, dst + row + 4, c.wrapping_add(tab.m10[dyad2]) & 0x7f7f_7f7f);
                                if is_top_of_cell && cell.ypos == 0 {
                                    let v = rd64(b, dst + row);
                                    wr64(b, dst, v);
                                } else {
                                    avg64(b, dst, r, dst + row);
                                }
                            } else if mode == 10 {
                                for at in [dst, dst + row] {
                                    let v = rd32(b, at).wrapping_add(tab.m10[dyad1]) & 0x7f7f_7f7f;
                                    wr32(b, at, v);
                                    let v = rd32(b, at + 4).wrapping_add(tab.m10[dyad2]) & 0x7f7f_7f7f;
                                    wr32(b, at + 4, v);
                                }
                            } else {
                                for at in [dst, dst + row] {
                                    let v = rd16(b, at).wrapping_add(tab.deltas[dyad1]) & 0x7f7f;
                                    wr16(b, at, v);
                                    let v = rd16(b, at + 2).wrapping_add(tab.deltas[dyad2]) & 0x7f7f;
                                    wr16(b, at + 2, v);
                                }
                            }
                        } else {
                            let lines_copy = |b: &mut [u8], num_lines: i32| {
                                if mode <= 4 {
                                    copy_block4(b, dst, r, row, num_lines << v_zoom);
                                } else if mode == 10 && !inter {
                                    let mut pix = rd64(b, r);
                                    if is_top_of_cell {
                                        pix = replicate64(pix);
                                        fill64(b, dst + row, pix, (num_lines << 1) - 1, row);
                                        avg64(b, dst, r, dst + row);
                                    } else {
                                        fill64(b, dst, pix, num_lines << 1, row);
                                    }
                                }
                            };
                            match code {
                                RLE_ESC_FC | RLE_ESC_FF | RLE_ESC_FE | RLE_ESC_FD => {
                                    if code == RLE_ESC_FC {
                                        skip_flag = false;
                                        rle_blocks = 1;
                                        code = 253;
                                    }
                                    num_lines = 257 - code as i32 - line;
                                    if num_lines <= 0 {
                                        return Err("RLE code not allowed at the current line".into());
                                    }
                                    lines_copy(b, num_lines);
                                }
                                RLE_ESC_FB => {
                                    let c = next(ptr)?;
                                    rle_blocks = (c & 0x1f) as i32 - 1;
                                    if c >= 64 || rle_blocks < 0 {
                                        return Err("RLE-FB invalid counter".into());
                                    }
                                    skip_flag = c & 0x20 != 0;
                                    num_lines = 4 - line;
                                    if mode >= 10 || inter || !skip_flag {
                                        lines_copy(b, num_lines);
                                    }
                                }
                                RLE_ESC_F9 | RLE_ESC_FA => {
                                    if code == RLE_ESC_F9 {
                                        skip_flag = true;
                                        rle_blocks = 1;
                                    }
                                    if line != 0 {
                                        return Err("RLE code not allowed at the current line".into());
                                    }
                                    num_lines = 4;
                                    if inter && mode <= 4 {
                                        lines_copy(b, num_lines);
                                    }
                                }
                                _ => return Err(format!("unsupported RLE code {code:X}")),
                            }
                        }
                        line += num_lines;
                        r += row * (num_lines << v_zoom) as isize;
                        dst += row * (num_lines << v_zoom) as isize;
                    }
                }
                block += 4 << h_zoom;
                ref_block += 4 << h_zoom;
                x += 1 + h_zoom;
            }
            ref_block += blk_row_offset;
            block += blk_row_offset;
            is_first_row = false;
            y += 1 + v_zoom;
        }
        Ok(())
    }
}

// ---- compressed VQ tables (indeo3data.h): (kind, a, b), kind 0 = the pair, 2 = +-(a,b),
// 4 = +-(a,b), +-(b,a) -------------------------------------------------------------------------

const TAB_1_1: &[(u8, i8, i8)] = &[
    (0, 0, 0), (2, 2, 2), (4, -1, 3), (2, 4, 4), (4, 1, 5), (2, -4, 4), (4, -2, 6), (4, 4, 9), (2, 9, 9),
    (4, 1, 10), (4, -5, 8), (4, 9, 15), (4, -3, 12), (4, 4, 16), (2, 16, 16), (4, 0, 18), (2, -12, 12),
    (4, -9, 16), (4, 11, 27), (4, 19, 28), (4, -6, 22), (4, 4, 29), (2, 30, 30), (4, -2, 33), (4, -18, 23),
    (4, -15, 30), (4, 22, 46), (4, 13, 47), (4, 35, 49), (4, -11, 41), (4, 4, 51), (2, 54, 54), (2, -34, 34),
    (4, -29, 42), (4, -6, 60), (4, 27, 76), (4, 43, 77), (4, -24, 55), (4, 14, 79), (4, 63, 83), (4, -20, 74),
    (4, 2, 88), (2, 93, 93), (4, -52, 61), (4, 52, 120), (4, -45, 75), (4, 75, 125), (4, 33, 122),
    (4, -13, 103), (4, -40, 96), (4, -34, 127), (2, -89, 89), (4, -78, 105), (2, 12, 12), (2, 23, 23),
    (2, 42, 42), (2, 73, 73),
];
const TAB_1_2: &[(u8, i8, i8)] = &[
    (0, 0, 0), (2, 3, 3), (4, -1, 4), (2, 7, 7), (4, 2, 8), (4, -2, 9), (2, -6, 6), (4, 6, 13), (2, 13, 13),
    (4, 1, 14), (4, -8, 12), (4, 14, 23), (4, -5, 18), (4, 6, 24), (2, 24, 24), (4, -1, 27), (2, -17, 17),
    (4, -13, 23), (4, 16, 40), (4, 28, 41), (4, -9, 33), (4, 6, 43), (2, 46, 46), (4, -4, 50), (4, -27, 34),
    (4, -22, 45), (4, 34, 69), (4, 19, 70), (4, 53, 73), (4, -17, 62), (4, 5, 77), (2, 82, 82), (2, -51, 51),
    (4, -43, 64), (4, -10, 90), (4, 41, 114), (4, 64, 116), (4, -37, 82), (4, 22, 119), (4, 95, 124),
    (4, -30, 111), (4, -78, 92), (4, -68, 113), (2, 18, 18), (2, 34, 34), (2, 63, 63), (2, 109, 109),
];
const TAB_1_3: &[(u8, i8, i8)] = &[
    (0, 0, 0), (2, 4, 4), (4, -1, 5), (4, 3, 10), (2, 9, 9), (2, -7, 7), (4, -3, 12), (4, 8, 17), (2, 17, 17),
    (4, 1, 19), (4, -11, 16), (4, -6, 23), (4, 18, 31), (4, 8, 32), (2, 33, 33), (4, -1, 36), (2, -23, 23),
    (4, -17, 31), (4, 21, 54), (4, 37, 55), (4, -12, 44), (4, 8, 57), (2, 61, 61), (4, -5, 66), (4, -36, 45),
    (4, -29, 60), (4, 45, 92), (4, 25, 93), (4, 71, 97), (4, -22, 83), (4, 7, 102), (2, 109, 109),
    (2, -68, 68), (4, -57, 85), (4, -13, 120), (4, -49, 110), (4, -104, 123), (2, 24, 24), (2, 46, 46),
    (2, 84, 84),
];
const TAB_1_4: &[(u8, i8, i8)] = &[
    (0, 0, 0), (2, 5, 5), (4, -2, 7), (2, 11, 11), (4, 3, 13), (2, -9, 9), (4, -4, 15), (4, 11, 22),
    (2, 21, 21), (4, 2, 24), (4, -14, 20), (4, 23, 38), (4, -8, 29), (4, 11, 39), (2, 41, 41), (4, -1, 45),
    (2, -29, 29), (4, -22, 39), (4, 27, 67), (4, 47, 69), (4, -15, 56), (4, 11, 71), (2, 76, 76), (4, -6, 83),
    (4, -45, 57), (4, -36, 75), (4, 56, 115), (4, 31, 117), (4, 88, 122), (4, -28, 104), (2, -85, 85),
    (4, -72, 106), (2, 30, 30), (2, 58, 58), (2, 105, 105),
];
const TAB_1_5: &[(u8, i8, i8)] = &[
    (0, 0, 0), (2, 6, 6), (4, -2, 8), (2, 13, 13), (4, 4, 15), (2, -11, 11), (4, -5, 18), (4, 13, 26),
    (2, 26, 26), (4, 2, 29), (4, -16, 24), (4, 28, 46), (4, -9, 35), (4, 13, 47), (2, 49, 49), (4, -1, 54),
    (2, -35, 35), (4, -26, 47), (4, 32, 81), (4, 56, 83), (4, -18, 67), (4, 13, 86), (2, 91, 91), (4, -7, 99),
    (4, -54, 68), (4, -44, 90), (4, -33, 124), (2, -103, 103), (4, -86, 127), (2, 37, 37), (2, 69, 69),
];
const TAB_1_6: &[(u8, i8, i8)] = &[
    (0, 0, 0), (2, 7, 7), (4, -3, 10), (2, 16, 16), (4, 5, 18), (2, -13, 13), (4, -6, 21), (4, 15, 30),
    (2, 30, 30), (4, 2, 34), (4, -19, 28), (4, 32, 54), (4, -11, 41), (4, 15, 55), (2, 57, 57), (4, -1, 63),
    (2, -40, 40), (4, -30, 55), (4, 37, 94), (4, 65, 96), (4, -21, 78), (4, 15, 100), (2, 106, 106),
    (4, -8, 116), (4, -63, 79), (4, -51, 105), (2, -120, 120), (2, 43, 43), (2, 80, 80),
];
const TAB_1_7: &[(u8, i8, i8)] = &[
    (0, 0, 0), (2, 8, 8), (4, -3, 11), (2, 18, 18), (4, 5, 20), (2, -15, 15), (4, -7, 24), (4, 17, 35),
    (2, 34, 34), (4, 3, 38), (4, -22, 32), (4, 37, 61), (4, -13, 47), (4, 17, 63), (2, 65, 65), (4, -1, 72),
    (2, -46, 46), (4, -35, 63), (4, 43, 107), (4, 75, 110), (4, -24, 89), (4, 17, 114), (2, 121, 121),
    (4, -72, 91), (4, -58, 120), (2, 49, 49), (2, 92, 92),
];
const TAB_1_8: &[(u8, i8, i8)] = &[
    (0, 0, 0), (2, 9, 9), (4, -3, 12), (2, 20, 20), (4, 6, 23), (2, -17, 17), (4, -7, 27), (4, 19, 39),
    (2, 39, 39), (4, 3, 43), (4, -24, 36), (4, 42, 69), (4, -14, 53), (4, 19, 71), (2, 73, 73), (4, -2, 80),
    (2, -52, 52), (4, -39, 70), (4, 48, 121), (4, 84, 124), (4, -27, 100), (4, -81, 102), (2, 55, 55),
    (2, 104, 104),
];
const TAB_2_1: &[(u8, i8, i8)] = &[
    (0, 0, 0), (2, 2, 2), (4, 0, 2), (2, 4, 4), (4, 0, 4), (2, -4, 4), (4, -2, 6), (4, 4, 8), (2, 8, 8),
    (4, 0, 10), (4, -4, 8), (4, 8, 14), (4, -2, 12), (4, 4, 16), (2, 16, 16), (4, 0, 18), (2, -12, 12),
    (4, -8, 16), (4, 10, 26), (4, 18, 28), (4, -6, 22), (4, 4, 28), (2, 30, 30), (4, -2, 32), (4, -18, 22),
    (4, -14, 30), (4, 22, 46), (4, 12, 46), (4, 34, 48), (4, -10, 40), (4, 4, 50), (2, 54, 54), (2, -34, 34),
    (4, -28, 42), (4, -6, 60), (4, 26, 76), (4, 42, 76), (4, -24, 54), (4, 14, 78), (4, 62, 82), (4, -20, 74),
    (4, 2, 88), (2, 92, 92), (4, -52, 60), (4, 52, 118), (4, -44, 74), (4, 74, 118), (4, 32, 118),
    (4, -12, 102), (4, -40, 96), (4, -34, 118), (2, -88, 88), (4, -78, 104), (2, 12, 12), (2, 22, 22),
    (2, 42, 42), (2, 72, 72),
];
const TAB_2_2: &[(u8, i8, i8)] = &[
    (0, 0, 0), (2, 3, 3), (4, 0, 3), (2, 6, 6), (4, 3, 9), (4, -3, 9), (2, -6, 6), (4, 6, 12), (2, 12, 12),
    (4, 0, 15), (4, -9, 12), (4, 15, 24), (4, -6, 18), (4, 6, 24), (2, 24, 24), (4, 0, 27), (2, -18, 18),
    (4, -12, 24), (4, 15, 39), (4, 27, 42), (4, -9, 33), (4, 6, 42), (2, 45, 45), (4, -3, 51), (4, -27, 33),
    (4, -21, 45), (4, 33, 69), (4, 18, 69), (4, 54, 72), (4, -18, 63), (4, 6, 78), (2, 81, 81), (2, -51, 51),
    (4, -42, 63), (4, -9, 90), (4, 42, 114), (4, 63, 117), (4, -36, 81), (4, 21, 120), (4, 96, 123),
    (4, -30, 111), (4, -78, 93), (4, -69, 114), (2, 18, 18), (2, 33, 33), (2, 63, 63), (2, 108, 108),
];
const TAB_2_3: &[(u8, i8, i8)] = &[
    (0, 0, 0), (2, 4, 4), (4, 0, 4), (4, 4, 8), (2, 8, 8), (2, -8, 8), (4, -4, 12), (4, 8, 16), (2, 16, 16),
    (4, 0, 20), (4, -12, 16), (4, -4, 24), (4, 16, 32), (4, 8, 32), (2, 32, 32), (4, 0, 36), (2, -24, 24),
    (4, -16, 32), (4, 20, 52), (4, 36, 56), (4, -12, 44), (4, 8, 56), (2, 60, 60), (4, -4, 64), (4, -36, 44),
    (4, -28, 60), (4, 44, 92), (4, 24, 92), (4, 72, 96), (4, -20, 84), (4, 8, 100), (2, 108, 108),
    (2, -68, 68), (4, -56, 84), (4, -12, 120), (4, -48, 108), (4, -104, 124), (2, 24, 24), (2, 44, 44),
    (2, 84, 84),
];
const TAB_2_4: &[(u8, i8, i8)] = &[
    (0, 0, 0), (2, 5, 5), (4, 0, 5), (2, 10, 10), (4, 5, 15), (2, -10, 10), (4, -5, 15), (4, 10, 20),
    (2, 20, 20), (4, 0, 25), (4, -15, 20), (4, 25, 40), (4, -10, 30), (4, 10, 40), (2, 40, 40), (4, 0, 45),
    (2, -30, 30), (4, -20, 40), (4, 25, 65), (4, 45, 70), (4, -15, 55), (4, 10, 70), (2, 75, 75), (4, -5, 85),
    (4, -45, 55), (4, -35, 75), (4, 55, 115), (4, 30, 115), (4, 90, 120), (4, -30, 105), (2, -85, 85),
    (4, -70, 105), (2, 30, 30), (2, 60, 60), (2, 105, 105),
];
const TAB_2_5: &[(u8, i8, i8)] = &[
    (0, 0, 0), (2, 6, 6), (4, 0, 6), (2, 12, 12), (4, 6, 12), (2, -12, 12), (4, -6, 18), (4, 12, 24),
    (2, 24, 24), (4, 0, 30), (4, -18, 24), (4, 30, 48), (4, -6, 36), (4, 12, 48), (2, 48, 48), (4, 0, 54),
    (2, -36, 36), (4, -24, 48), (4, 30, 78), (4, 54, 84), (4, -18, 66), (4, 12, 84), (2, 90, 90), (4, -6, 96),
    (4, -54, 66), (4, -42, 90), (4, -30, 126), (2, -102, 102), (4, -84, 126), (2, 36, 36), (2, 66, 66),
];
const TAB_2_6: &[(u8, i8, i8)] = &[
    (0, 0, 0), (2, 7, 7), (4, 0, 7), (2, 14, 14), (4, 7, 21), (2, -14, 14), (4, -7, 21), (4, 14, 28),
    (2, 28, 28), (4, 0, 35), (4, -21, 28), (4, 35, 56), (4, -14, 42), (4, 14, 56), (2, 56, 56), (4, 0, 63),
    (2, -42, 42), (4, -28, 56), (4, 35, 91), (4, 63, 98), (4, -21, 77), (4, 14, 98), (2, 105, 105),
    (4, -7, 119), (4, -63, 77), (4, -49, 105), (2, -119, 119), (2, 42, 42), (2, 77, 77),
];
const TAB_2_7: &[(u8, i8, i8)] = &[
    (0, 0, 0), (2, 8, 8), (4, 0, 8), (2, 16, 16), (4, 8, 16), (2, -16, 16), (4, -8, 24), (4, 16, 32),
    (2, 32, 32), (4, 0, 40), (4, -24, 32), (4, 40, 64), (4, -16, 48), (4, 16, 64), (2, 64, 64), (4, 0, 72),
    (2, -48, 48), (4, -32, 64), (4, 40, 104), (4, 72, 112), (4, -24, 88), (4, 16, 112), (2, 120, 120),
    (4, -72, 88), (4, -56, 120), (2, 48, 48), (2, 88, 88),
];
const TAB_2_8: &[(u8, i8, i8)] = &[
    (0, 0, 0), (2, 9, 9), (4, 0, 9), (2, 18, 18), (4, 9, 27), (2, -18, 18), (4, -9, 27), (4, 18, 36),
    (2, 36, 36), (4, 0, 45), (4, -27, 36), (4, 45, 72), (4, -18, 54), (4, 18, 72), (2, 72, 72), (4, 0, 81),
    (2, -54, 54), (4, -36, 72), (4, 45, 117), (4, 81, 126), (4, -27, 99), (4, -81, 99), (2, 54, 54),
    (2, 108, 108),
];
const TAB_3_1: &[(u8, i8, i8)] = &[
    (0, 0, 0), (2, 2, 2), (4, 0, 3), (2, 6, 6), (4, 0, 7), (2, -5, 5), (2, 5, -5), (4, 6, 11), (4, 0, 8),
    (2, 11, 11), (4, 0, 12), (4, 12, 17), (2, 17, 17), (4, 6, 18), (4, -8, 11), (4, 0, 15), (4, 0, 20),
    (4, 18, 25), (4, 11, 25), (2, 25, 25), (2, -14, 14), (2, 14, -14), (4, 0, 26), (4, -11, 18), (4, -7, 22),
    (4, 26, 34), (4, 18, 34), (2, 34, 34), (4, 11, 35), (4, 0, 29), (4, -19, 22), (4, -15, 26), (4, 0, 37),
    (4, 27, 44), (4, 36, 44), (4, 18, 44), (4, -10, 33), (2, 45, 45),
];
const TAB_3_2: &[(u8, i8, i8)] = &[
    (0, 0, 0), (4, 0, 2), (2, 2, 2), (2, 6, 6), (4, 0, 6), (2, -4, 4), (2, 10, -6), (2, 0, -12), (0, -6, -12),
    (2, 6, -12), (0, 6, 12), (2, -14, 0), (2, 12, 12), (2, 0, -18), (2, 14, -12), (0, -18, -6), (2, 18, -6),
    (0, 18, 6), (0, -10, -18), (2, 10, -18), (0, 10, 18), (2, -22, 0), (2, 0, -24), (0, -22, -12),
    (2, 22, -12), (0, 22, 12), (0, -8, -24), (2, 8, -24), (0, 8, 24), (0, -26, -6), (2, 26, -6), (0, 26, 6),
    (2, -28, 0), (2, 20, 20), (2, -14, -26), (2, -30, -12), (2, -10, -32), (2, -18, -32), (2, -26, -26),
    (2, -34, -20), (2, -38, -12), (2, -32, -32), (0, 32, 32), (0, -22, -40), (2, -34, -34),
];
const TAB_3_3: &[(u8, i8, i8)] = &[
    (0, 0, 0), (4, 0, 2), (2, 4, 4), (2, 10, 10), (4, 0, 10), (2, -6, 6), (2, 14, -8), (2, -18, 0),
    (2, 10, -16), (2, 0, -24), (0, -24, -8), (2, 24, -8), (0, 24, 8), (2, 18, 18), (2, 20, -16),
    (0, -14, -26), (2, 14, -26), (0, 14, 26), (2, -30, 0), (2, 0, -34), (0, -34, -8), (2, 34, -8), (0, 34, 8),
    (0, -30, -18), (2, 30, -18), (0, 30, 18), (0, -10, -34), (2, 10, -34), (0, 10, 34), (2, -20, -34),
    (2, -40, 0), (2, 30, 30), (2, -40, -18), (2, 0, -44), (2, -16, -44), (0, -36, -36), (2, -36, -36),
    (2, -26, -44), (2, -46, -26), (2, -52, -18), (0, -20, -54), (2, -44, -44), (0, -32, -54), (0, -46, -46),
    (2, -46, -46),
];
const TAB_3_4: &[(u8, i8, i8)] = &[
    (0, 0, 0), (4, 0, 4), (2, 4, 4), (2, 12, 12), (4, 0, 12), (2, -8, 8), (2, 8, -16), (2, 0, -24),
    (0, -24, -8), (2, 24, -8), (0, 24, 8), (2, 20, -16), (2, -28, 0), (0, -16, -24), (2, 16, -24),
    (0, 16, 24), (2, 0, -32), (0, -28, -16), (2, 28, -16), (0, 28, 16), (0, -8, -32), (0, 8, -32),
    (0, -32, -8), (2, 32, -8), (0, 32, 8), (0, -8, 32), (0, 8, 32), (2, 24, 24), (2, 24, -24), (2, -20, -32),
    (2, -40, 0), (2, -40, -16), (0, 0, -44), (0, 0, -44), (2, -44, 0), (0, 0, 44), (0, 0, 44), (2, -32, -32),
    (2, -16, -44), (0, -24, -44), (2, -44, -24), (0, 24, 44), (2, -48, -16), (0, -36, -36), (2, -36, -36),
    (0, 36, 36), (0, -20, -52), (2, 40, 40), (0, -32, -52),
];
const TAB_3_5: &[(u8, i8, i8)] = &[
    (0, 0, 0), (2, 2, 2), (2, 6, 6), (2, 12, 12), (2, 20, 20), (2, 32, 32), (2, 46, 46),
];
