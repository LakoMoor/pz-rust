use crate::{Error, Result};

/// Little-endian cursor over a byte slice.
pub struct Reader<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(data: &'a [u8]) -> Self { Self { data, pos: 0 } }
    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(n).filter(|&e| e <= self.data.len()).ok_or(Error::Eof(self.pos))?;
        let s = &self.data[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    pub fn u8(&mut self) -> Result<u8> { Ok(self.take(1)?[0]) }
    pub fn u32(&mut self) -> Result<u32> { Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap())) }
    pub fn i32(&mut self) -> Result<i32> { Ok(self.u32()? as i32) }
    pub fn u64(&mut self) -> Result<u64> { Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap())) }
    pub fn magic(&mut self, expected: &'static str) -> Result<()> {
        let got: [u8; 4] = self.take(4)?.try_into().unwrap();
        if got != *expected.as_bytes() { return Err(Error::Magic { expected, got }); }
        Ok(())
    }
    /// u32 length-prefixed UTF-8 string.
    pub fn string(&mut self) -> Result<String> {
        let n = self.u32()? as usize;
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
    /// Newline-terminated string.
    pub fn line(&mut self) -> Result<String> {
        let rest = &self.data[self.pos..];
        let n = rest.iter().position(|&b| b == b'\n').ok_or(Error::Eof(self.data.len()))?;
        let s = String::from_utf8_lossy(&rest[..n]).into_owned();
        self.pos += n + 1;
        Ok(s)
    }
}
