//! FSB5 sound bank container (the audio payload inside FMOD `.bank` files).
//!
//! Only the headers are parsed: the stream list with codec, rate, channels, length and data offset. Decoding the
//! samples (FMOD's Vorbis variant keeps its setup headers out of the stream) is not implemented.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec { Pcm8, Pcm16, Pcm24, Pcm32, PcmFloat, GcAdpcm, ImaAdpcm, Vag, Hevag, Xma, Mpeg, Celt, At9, Xwma, Vorbis, Other(u32) }

impl Codec {
    fn from(mode: u32) -> Codec {
        match mode {
            1 => Codec::Pcm8, 2 => Codec::Pcm16, 3 => Codec::Pcm24, 4 => Codec::Pcm32, 5 => Codec::PcmFloat,
            6 => Codec::GcAdpcm, 7 => Codec::ImaAdpcm, 8 => Codec::Vag, 9 => Codec::Hevag, 10 => Codec::Xma,
            11 => Codec::Mpeg, 12 => Codec::Celt, 13 => Codec::At9, 14 => Codec::Xwma, 15 => Codec::Vorbis,
            n => Codec::Other(n),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stream {
    pub frequency: u32,
    pub channels: u32,
    /// Length in sample frames.
    pub samples: u64,
    /// Offset of this stream's data from the start of the data section.
    pub data_offset: u64,
    /// CRC32 of the Vorbis setup header (extra chunk type 11), needed to rebuild the codebooks.
    pub vorbis_crc: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fsb5 {
    pub version: u32,
    pub codec: Codec,
    pub data_size: u32,
    /// Byte offset of the data section from the start of the FSB5 blob.
    pub data_start: usize,
    pub streams: Vec<Stream>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Fsb5Error { NotFsb5, Truncated }

const FREQ: [u32; 10] = [4000, 8000, 11000, 11025, 16000, 22050, 24000, 32000, 44100, 48000];

fn u32le(b: &[u8], at: usize) -> Result<u32, Fsb5Error> {
    b.get(at..at + 4).map(|s| u32::from_le_bytes(s.try_into().unwrap())).ok_or(Fsb5Error::Truncated)
}

impl Fsb5 {
    /// Parse the header and sample headers from the beginning of an FSB5 blob (the first
    /// `60 + sample_header_size` bytes are enough, see [`Fsb5::header_len`]).
    pub fn parse(b: &[u8]) -> Result<Fsb5, Fsb5Error> {
        if b.get(..4) != Some(b"FSB5") { return Err(Fsb5Error::NotFsb5); }
        let version = u32le(b, 4)?;
        let count = u32le(b, 8)? as usize;
        let sample_hdr = u32le(b, 12)? as usize;
        let name_table = u32le(b, 16)? as usize;
        let data_size = u32le(b, 20)?;
        let mode = u32le(b, 24)?;
        let base = if version == 0 { 64 } else { 60 };
        let data_start = base + sample_hdr + name_table;
        let mut streams = Vec::with_capacity(count);
        let mut p = base;
        for _ in 0..count {
            let raw = u32le(b, p)? as u64 | (u32le(b, p + 4)? as u64) << 32;
            p += 8;
            let mut frequency = FREQ.get(((raw >> 1) & 0xF) as usize).copied().unwrap_or(0);
            let mut channels = match (raw >> 5) & 3 { 0 => 1, 1 => 2, 2 => 6, _ => 8 };
            let data_offset = ((raw >> 7) & 0x07FF_FFFF) * 16;
            let samples = raw >> 34;
            let mut vorbis_crc = None;
            let mut more = raw & 1 != 0;
            while more {
                let chunk = u32le(b, p)?;
                p += 4;
                let (size, kind) = (((chunk >> 1) & 0x00FF_FFFF) as usize, chunk >> 25);
                match kind {
                    1 => channels = *b.get(p).ok_or(Fsb5Error::Truncated)? as u32,
                    2 => frequency = u32le(b, p)?,
                    11 => vorbis_crc = Some(u32le(b, p)?),
                    _ => {}
                }
                p += size;
                more = chunk & 1 != 0;
            }
            streams.push(Stream { frequency, channels, samples, data_offset, vorbis_crc });
        }
        Ok(Fsb5 { version, codec: Codec::from(mode), data_size, data_start, streams })
    }

    /// Bytes needed from the start of the blob to parse the headers: reads the fixed fields first.
    pub fn header_len(b: &[u8]) -> Result<usize, Fsb5Error> {
        if b.get(..4) != Some(b"FSB5") { return Err(Fsb5Error::NotFsb5); }
        Ok(if u32le(b, 4)? == 0 { 64 } else { 60 } + u32le(b, 12)? as usize)
    }
}

/// Find the FSB5 blob inside a `.bank` (a RIFF `FEV ` file): returns its offset within the file and the
/// `SND ` chunk size, reading only chunk headers through `read_at`.
pub fn locate_in_bank(len: u64, mut read_at: impl FnMut(u64, usize) -> std::io::Result<Vec<u8>>) -> std::io::Result<Option<(u64, u64)>> {
    let head = read_at(0, 12)?;
    if head.len() < 12 || &head[..4] != b"RIFF" || &head[8..12] != b"FEV " { return Ok(None); }
    let mut pos = 12u64;
    while pos + 8 <= len {
        let h = read_at(pos, 8)?;
        let size = u32::from_le_bytes(h[4..8].try_into().unwrap()) as u64;
        if &h[..4] == b"SND " {
            // The payload is padded for alignment before the FSB5 magic; scan the first bytes for it.
            let probe = read_at(pos + 8, 64.min(size as usize))?;
            if let Some(i) = probe.windows(4).position(|w| w == b"FSB5") { return Ok(Some((pos + 8 + i as u64, size - i as u64))); }
            return Ok(None);
        }
        pos += 8 + size + (size & 1);
    }
    Ok(None)
}
