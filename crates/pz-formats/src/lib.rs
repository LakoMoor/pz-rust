//! Readers for Project Zomboid (Build 42) data formats.
pub mod io;
pub mod pack;
pub mod tiles;
pub mod lot;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("bad magic: expected {expected}, got {got:?}")]
    Magic { expected: &'static str, got: [u8; 4] },
    #[error("unexpected end of data at {0}")]
    Eof(usize),
    #[error("invalid data: {0}")]
    Invalid(String),
}
pub type Result<T> = std::result::Result<T, Error>;
