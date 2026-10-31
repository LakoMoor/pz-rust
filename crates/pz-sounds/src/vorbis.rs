//! FMOD's stripped Vorbis streams (FSB5 codec Vorbis), decoded with `lewton`.
//!
//! An FSB5 Vorbis stream carries only audio packets, each prefixed by its size as a little-endian u16. The
//! identification and setup headers are not in the bank: FMOD keeps the setup packets in a table inside `fmod.dll`
//! and the stream names the one it needs by the CRC32 in extra chunk 11. [`setup_packet`] finds that packet in the
//! user's own `fmod.dll`; nothing from the game is stored in this repository.

use lewton::audio::{read_audio_packet, PreviousWindowRight};
use lewton::header::{read_header_ident, read_header_setup};

fn u16le(b: &[u8], at: usize) -> Option<usize> { b.get(at..at + 2).map(|s| u16::from_le_bytes(s.try_into().unwrap()) as usize) }
fn u32le(b: &[u8], at: usize) -> Option<u32> { b.get(at..at + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap())) }
fn u64le(b: &[u8], at: usize) -> Option<u64> { b.get(at..at + 8).map(|s| u64::from_le_bytes(s.try_into().unwrap())) }

struct Pe { base: u64, sections: Vec<(u32, u32, u32, u32)> } // (rva, virtual size, raw offset, raw size)

fn parse_pe(b: &[u8]) -> Option<Pe> {
    let pe = u32le(b, 0x3c)? as usize;
    if b.get(pe..pe + 4)? != b"PE\0\0" { return None; }
    let nsec = u16le(b, pe + 6)?;
    let opt_size = u16le(b, pe + 20)?;
    let opt = pe + 24;
    if u16le(b, opt)? != 0x20b { return None; } // PE32+
    let base = u64le(b, opt + 24)?;
    let mut sections = Vec::new();
    for i in 0..nsec {
        let s = opt + opt_size + 40 * i;
        sections.push((u32le(b, s + 12)?, u32le(b, s + 8)?, u32le(b, s + 20)?, u32le(b, s + 16)?));
    }
    Some(Pe { base, sections })
}

impl Pe {
    fn offset(&self, rva: u64) -> Option<usize> {
        self.sections.iter().find(|(va, vs, _, rs)| rva >= *va as u64 && rva < *va as u64 + (*vs).min(*rs) as u64)
            .map(|(va, _, ro, _)| (*ro as u64 + rva - *va as u64) as usize)
    }
}

/// The Vorbis setup packet FMOD uses for streams with this CRC, found in the bytes of `fmod.dll`. Its table holds
/// 16-byte records `{ u32 size, u32 crc, u64 address }` pointing at packets that start `05 "vorbis"`.
pub fn setup_packet(dll: &[u8], crc: u32) -> Option<&[u8]> {
    let pe = parse_pe(dll)?;
    let needle = crc.to_le_bytes();
    let mut from = 4;
    while let Some(pos) = dll[from..].windows(4).position(|w| w == needle) {
        let i = from + pos;
        from = i + 1;
        if i < 4 { continue; }
        let (Some(size), Some(addr)) = (u32le(dll, i - 4), u64le(dll, i + 4)) else { continue };
        let Some(rva) = addr.checked_sub(pe.base) else { continue };
        let Some(off) = pe.offset(rva) else { continue };
        let Some(data) = dll.get(off..off + size as usize) else { continue };
        if size > 64 && data[0] == 5 && &data[1..7] == b"vorbis" { return Some(data); }
    }
    None
}

/// The identification header packet FMOD leaves out. The block sizes are FMOD's fixed pair (256 and 2048 samples).
fn ident_packet(channels: u8, rate: u32, exps: (u8, u8)) -> Vec<u8> {
    let mut p = vec![1];
    p.extend_from_slice(b"vorbis");
    p.extend_from_slice(&0u32.to_le_bytes());
    p.push(channels);
    p.extend_from_slice(&rate.to_le_bytes());
    p.extend_from_slice(&[0; 12]); // bitrate max / nominal / min
    p.push(exps.0 | exps.1 << 4);
    p.push(1); // framing
    p
}

#[derive(Debug)]
pub enum VorbisError { Header(String), Audio(String) }

/// Decode one stream to interleaved 16-bit samples, reading size-prefixed packets from `data` starting at `start`
/// until `frames` sample frames have been produced (the stream's length from the FSB5 header). Zero bytes are
/// padding (not always an even number of them) and skipped. Returns the samples and the offset just past the last packet used: streams follow each
/// other without gaps, so that is where the next stream really begins (the header offsets are rounded to 16 bytes).
pub fn decode(setup: &[u8], channels: u8, rate: u32, exps: (u8, u8), data: &[u8], start: usize, frames: u64) -> Result<(Vec<i16>, usize), VorbisError> {
    let ident = read_header_ident(&ident_packet(channels, rate, exps)).map_err(|e| VorbisError::Header(format!("{e:?}")))?;
    let setup = read_header_setup(setup, channels, (ident.blocksize_0, ident.blocksize_1)).map_err(|e| VorbisError::Header(format!("{e:?}")))?;
    let mut pwr = PreviousWindowRight::new();
    let mut out: Vec<i16> = Vec::new();
    let mut at = start;
    while (out.len() / channels as usize) < frames as usize {
        let Some(size) = u16le(data, at) else { break };
        // Padding runs of zero bytes (any length) separate packets; a lone zero low byte with a huge size is the
        // last padding byte followed by the real size.
        if size == 0 || (data[at] == 0 && size > 4096) { at += 1; continue; }
        at += 2;
        if at + size > data.len() { break; }
        let planar = read_audio_packet(&ident, &setup, &data[at..at + size], &mut pwr).map_err(|e| VorbisError::Audio(format!("{e:?} in the packet at {at} (size {size}) after {} frames", out.len() / channels as usize)))?;
        at += size;
        let n = planar.first().map_or(0, |c| c.len());
        for i in 0..n {
            for c in &planar { out.push(c[i]); }
        }
    }
    out.truncate(frames as usize * channels as usize);
    Ok((out, at))
}
