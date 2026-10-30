//! Lua runtime for the game's own scripts (`media/lua`). The original runs them on Kahlua, a Lua 5.1 VM, so
//! this uses vendored Lua 5.1. Only the pure-Lua parts of the Java API are provided so far.

use mlua::{Lua, Result};
use std::path::Path;

pub struct Runtime { pub lua: Lua }

impl Runtime {
    pub fn new() -> Result<Self> {
        let lua = Lua::new();
        // Kahlua's `print` and the `getText` fallback are the only globals scripts need at load time.
        lua.load("function getText(k) return k end").exec()?;
        Ok(Self { lua })
    }

    /// Run a script file; the chunk is named after the file so errors point to it.
    pub fn run_file(&self, path: &Path) -> Result<()> {
        let src = std::fs::read(path).map_err(mlua::Error::external)?;
        self.lua.load(&src[..]).set_name(format!("@{}", path.display())).exec()
    }
}
