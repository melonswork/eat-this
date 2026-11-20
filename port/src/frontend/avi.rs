//! VIDEOS/*.AVI: a RIFF AVI reader (one Indeo 3 video stream, optional PCM audio stream) and
//! the player state the front end drives (FUN_1008_1365's MCI "AVIVideo" device).

use super::indeo3::Decoder;
use crate::sound::Sample;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;

pub struct Avi {
    file: File,
    /// Video chunks: (file offset of the data, size).
    frames: Vec<(u64, u32)>,
    pub usec_per_frame: u32,
    /// The audio stream (8-bit unsigned PCM or 16-bit signed), decoded whole.
    pub audio: Option<Arc<Sample>>,
    dec: Decoder,
    /// The next frame to decode.
    next: usize,
    chunk: Vec<u8>,
}

fn u32le(b: &[u8], i: usize) -> u32 {
    b.get(i..i + 4).map_or(0, |s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

fn u16le(b: &[u8], i: usize) -> u16 {
    b.get(i..i + 2).map_or(0, |s| u16::from_le_bytes([s[0], s[1]]))
}

impl Avi {
    pub fn open(path: &Path) -> Result<Avi, String> {
        let mut file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let len = file.metadata().map_err(|e| e.to_string())?.len();
        let mut hdr = [0u8; 12];
        file.read_exact(&mut hdr).map_err(|e| e.to_string())?;
        if &hdr[0..4] != b"RIFF" || &hdr[8..12] != b"AVI " {
            return Err(format!("{}: not an AVI", path.display()));
        }
        let mut avi = Avi { file, frames: Vec::new(), usec_per_frame: 66667, audio: None, dec: Decoder::new(), next: 0, chunk: Vec::new() };
        // Streams in header order: (fourcc type, handler, format).
        let mut streams: Vec<([u8; 4], Vec<u8>)> = Vec::new();
        let mut pcm: Vec<u8> = Vec::new();
        avi.walk(12, len, &mut streams, &mut pcm)?;
        let video = streams.iter().position(|(t, _)| t == b"vids").ok_or("no video stream")?;
        let _ = video;
        if let Some((_, fmt)) = streams.iter().find(|(t, _)| t == b"auds") {
            let (tag, ch, rate, bits) = (u16le(fmt, 0), u16le(fmt, 2).max(1) as usize, u32le(fmt, 4), u16le(fmt, 14));
            if tag == 1 && !pcm.is_empty() {
                let mono: Vec<f32> = if bits == 8 {
                    pcm.chunks_exact(ch).map(|c| c.iter().map(|&v| (v as f32 - 128.0) / 128.0).sum::<f32>() / ch as f32).collect()
                } else {
                    pcm.chunks_exact(2 * ch)
                        .map(|c| c.chunks_exact(2).map(|s| i16::from_le_bytes([s[0], s[1]]) as f32 / 32768.0).sum::<f32>() / ch as f32)
                        .collect()
                };
                avi.audio = Some(Arc::new(Sample { data: mono, rate }));
            }
        }
        Ok(avi)
    }

    fn walk(&mut self, mut at: u64, end: u64, streams: &mut Vec<([u8; 4], Vec<u8>)>, pcm: &mut Vec<u8>) -> Result<(), String> {
        let mut h = [0u8; 12];
        while at + 8 <= end {
            self.file.seek(SeekFrom::Start(at)).map_err(|e| e.to_string())?;
            let n = self.file.read(&mut h).map_err(|e| e.to_string())?;
            if n < 8 {
                break;
            }
            let id: [u8; 4] = h[0..4].try_into().unwrap();
            let size = u32le(&h, 4) as u64;
            let body = at + 8;
            match &id {
                b"LIST" | b"RIFF" => self.walk(body + 4, (body + size).min(end), streams, pcm)?,
                b"avih" | b"strh" | b"strf" => {
                    let b = self.read_at(body, size as usize)?;
                    match &id {
                        b"avih" => self.usec_per_frame = u32le(&b, 0).max(1),
                        b"strh" => streams.push((b[0..4].try_into().unwrap_or([0; 4]), Vec::new())),
                        _ => {
                            if let Some(s) = streams.last_mut() {
                                s.1 = b;
                            }
                        }
                    }
                }
                _ if id[0].is_ascii_digit() && id[1].is_ascii_digit() => {
                    let stream = ((id[0] - b'0') * 10 + (id[1] - b'0')) as usize;
                    match streams.get(stream).map(|s| &s.0) {
                        Some(b"vids") => self.frames.push((body, size as u32)),
                        Some(b"auds") => pcm.extend(self.read_at(body, size as usize)?),
                        _ => {}
                    }
                }
                _ => {}
            }
            at = body + size + (size & 1);
        }
        Ok(())
    }

    fn read_at(&mut self, at: u64, size: usize) -> Result<Vec<u8>, String> {
        let mut b = vec![0u8; size];
        self.file.seek(SeekFrom::Start(at)).map_err(|e| e.to_string())?;
        self.file.read_exact(&mut b).map_err(|e| e.to_string())?;
        Ok(b)
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    /// Decodes frames up to and including `n` (from the start again if `n` is behind).
    pub fn seek(&mut self, n: usize) -> Result<(), String> {
        let n = n.min(self.frames.len().saturating_sub(1));
        if n + 1 < self.next {
            self.next = 0;
            self.dec = Decoder::new();
        }
        while self.next <= n && self.next < self.frames.len() {
            let (at, size) = self.frames[self.next];
            let mut chunk = std::mem::take(&mut self.chunk);
            chunk.resize(size as usize, 0);
            self.file.seek(SeekFrom::Start(at)).map_err(|e| e.to_string())?;
            self.file.read_exact(&mut chunk).map_err(|e| e.to_string())?;
            let r = if size == 0 { Ok(false) } else { self.dec.decode(&chunk) };
            self.chunk = chunk;
            self.next += 1;
            r.map_err(|e| format!("frame {}: {e}", self.next - 1))?;
        }
        Ok(())
    }

    pub fn size(&self) -> (usize, usize) {
        (self.dec.width, self.dec.height)
    }

    pub fn have_frame(&self) -> bool {
        self.dec.have_frame
    }

    pub fn yuv(&self, out: &mut Vec<u8>) {
        self.dec.yuv(out)
    }

    pub fn rgb(&self, out: &mut [u8]) {
        self.dec.rgb(out)
    }
}
