//! `.pack` texture atlases ("PZPK"): pages of named sub-images + embedded PNG.
use crate::{io::Reader, Error, Result};

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    /// Rectangle inside the page PNG.
    pub x: u32, pub y: u32, pub w: u32, pub h: u32,
    /// Offset of the trimmed rect inside the original (untrimmed) frame.
    pub ox: u32, pub oy: u32,
    /// Original (untrimmed) frame size.
    pub ow: u32, pub oh: u32,
}

#[derive(Debug)]
pub struct Page {
    pub name: String,
    pub flags: u32,
    pub entries: Vec<Entry>,
    /// Raw PNG bytes of the atlas page.
    pub png: Vec<u8>,
}

#[derive(Debug)]
pub struct Pack { pub version: u32, pub pages: Vec<Page> }

impl Pack {
    pub fn parse(data: &[u8]) -> Result<Pack> {
        let mut r = Reader::new(data);
        r.magic("PZPK")?;
        let version = r.u32()?;
        let pages = r.u32()?;
        let mut out = Vec::with_capacity(pages as usize);
        for _ in 0..pages {
            let name = r.string()?;
            let n = r.u32()?;
            let flags = r.u32()?;
            let mut entries = Vec::with_capacity(n as usize);
            for _ in 0..n {
                entries.push(Entry {
                    name: r.string()?,
                    x: r.u32()?, y: r.u32()?, w: r.u32()?, h: r.u32()?,
                    ox: r.u32()?, oy: r.u32()?, ow: r.u32()?, oh: r.u32()?,
                });
            }
            let len = r.u32()? as usize;
            let png = r.take(len)?.to_vec();
            out.push(Page { name, flags, entries, png });
        }
        Ok(Pack { version, pages: out })
    }

    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Pack> { Self::parse(&std::fs::read(path)?) }
}

/// Decode a page PNG to RGBA8.
pub fn decode_png_rgba(png_bytes: &[u8]) -> Result<(u32, u32, Vec<u8>)> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(png_bytes));
    dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut rd = dec.read_info().map_err(|e| Error::Invalid(e.to_string()))?;
    let mut buf = vec![0; rd.output_buffer_size()];
    let info = rd.next_frame(&mut buf).map_err(|e| Error::Invalid(e.to_string()))?;
    buf.truncate(info.buffer_size());
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => buf.chunks(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect(),
        png::ColorType::GrayscaleAlpha => buf.chunks(2).flat_map(|c| [c[0], c[0], c[0], c[1]]).collect(),
        png::ColorType::Grayscale => buf.iter().flat_map(|&g| [g, g, g, 255]).collect(),
        png::ColorType::Indexed => return Err(Error::Invalid("indexed png".into())),
    };
    Ok((info.width, info.height, rgba))
}
