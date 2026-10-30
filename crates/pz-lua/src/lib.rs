//! Lua runtime for the game's own scripts (`media/lua`). The original runs them on Kahlua, a Lua 5.1 VM, so
//! this uses vendored Lua 5.1. Only the pure-Lua parts of the Java API are provided so far.

use mlua::{Lua, Result};
use std::path::Path;

/// Java-side globals the scripts rely on at load time. `Events.<Name>.Add(fn)` registers callbacks;
/// [`Runtime::trigger`] calls them. Any unknown event name is created on first use, like the original.
const PRELUDE: &str = r#"
function getText(k) return k end
function isTable(o) return type(o) == "table" end
local noop = function() end
DebugType = setmetatable({}, { __index = function() return setmetatable({}, { __index = function() return noop end }) end })
LogSeverity = setmetatable({}, { __index = function(_, k) return k end })
__events = {}
Events = setmetatable({}, { __index = function(t, name)
    local list = {}
    __events[name] = list
    local ev = {
        Add = function(f) table.insert(list, f) end,
        Remove = function(f) for i, g in ipairs(list) do if g == f then table.remove(list, i) break end end end,
    }
    rawset(t, name, ev)
    return ev
end })
function triggerEvent(name, ...)
    local list = __events[name]
    if list then for _, f in ipairs({ unpack(list) }) do f(...) end end
end
"#;

pub struct Runtime { pub lua: Lua }

impl Runtime {
    pub fn new() -> Result<Self> {
        let lua = Lua::new();
        // Kahlua's `print` and the `getText` fallback are the only globals scripts need at load time.
        lua.load(PRELUDE).exec()?;
        Ok(Self { lua })
    }

    /// `triggerEvent(name, args...)`: call every callback registered with `Events.<name>.Add`.
    pub fn trigger<A: mlua::IntoLuaMulti>(&self, name: &str, args: A) -> Result<()> {
        let f: mlua::Function = self.lua.globals().get("triggerEvent")?;
        let mut v = args.into_lua_multi(&self.lua)?;
        v.push_front(mlua::Value::String(self.lua.create_string(name)?));
        f.call::<()>(v)
    }

    /// Run a script file; the chunk is named after the file so errors point to it.
    pub fn run_file(&self, path: &Path) -> Result<()> {
        let src = std::fs::read(path).map_err(mlua::Error::external)?;
        self.lua.load(&src[..]).set_name(format!("@{}", path.display())).exec()
    }
}
