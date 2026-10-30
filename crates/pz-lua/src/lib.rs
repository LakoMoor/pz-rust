//! Lua runtime for the game's own scripts (`media/lua`). The original runs them on Kahlua, a Lua 5.1 VM, so
//! this uses vendored Lua 5.1. Only the pure-Lua parts of the Java API are provided so far.

use mlua::{Lua, Result};
use pz_items::{ItemDb, ItemDef};
use std::path::Path;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

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

fn item_table(lua: &Lua, def: &ItemDef) -> Result<mlua::Table> {
    let t = lua.create_table()?;
    let id = def.id.clone();
    let short = id.split_once('.').map_or(id.clone(), |(_, n)| n.to_string());
    let mut getter = |name: &str, v: mlua::Value| -> Result<()> {
        let f = lua.create_function(move |_, _: mlua::MultiValue| Ok(v.clone()))?;
        t.set(name, f)
    };
    getter("getFullType", mlua::Value::String(lua.create_string(&id)?))?;
    getter("getName", mlua::Value::String(lua.create_string(&short)?))?;
    getter("getType", mlua::Value::String(lua.create_string(&short)?))?;
    getter("getTypeString", mlua::Value::String(lua.create_string(&def.item_type)?))?;
    getter("getDisplayCategory", mlua::Value::String(lua.create_string(&def.display_category)?))?;
    getter("getActualWeight", mlua::Value::Number(def.weight as f64))?;
    getter("getWeight", mlua::Value::Number(def.weight as f64))?;
    getter("getMinDamage", mlua::Value::Number(def.min_damage as f64))?;
    getter("getMaxDamage", mlua::Value::Number(def.max_damage as f64))?;
    getter("getMaxRange", mlua::Value::Number(def.max_range as f64))?;
    getter("isRanged", mlua::Value::Boolean(def.is_firearm()))?;
    Ok(t)
}

impl Runtime {
    /// Expose the parsed item scripts the way scripts see them:
    /// `getScriptManager():getItem("Base.Axe")` (nil if unknown) and `InventoryItemFactory.CreateItem(id)`.
    pub fn bind_items(&self, db: Arc<ItemDb>) -> Result<()> {
        let lua = &self.lua;
        let lookup = {
            let db = db.clone();
            lua.create_function(move |lua, id: String| match db.get(&id) {
                Some(def) => item_table(lua, def).map(mlua::Value::Table),
                None => Ok(mlua::Value::Nil),
            })?
        };
        let manager = lua.create_table()?;
        let l2 = lookup.clone();
        manager.set("getItem", lua.create_function(move |_, (_this, id): (mlua::Value, String)| l2.call::<mlua::Value>(id))?)?;
        let m2 = manager.clone();
        lua.globals().set("getScriptManager", lua.create_function(move |_, ()| Ok(m2.clone()))?)?;
        let factory = lua.create_table()?;
        factory.set("CreateItem", lookup)?;
        lua.globals().set("InventoryItemFactory", factory)
    }
}

impl Runtime {
    /// `getPlayer()` / `getSpecificPlayer(0)` for the local player, reading live simulation state on each call.
    pub fn bind_player(&self, player: Rc<RefCell<pz_sim::Player>>) -> Result<()> {
        let lua = &self.lua;
        let t = lua.create_table()?;
        let num = |name: &str, f: fn(&pz_sim::Player) -> f64| -> Result<()> {
            let p = player.clone();
            t.set(name, lua.create_function(move |_, _: mlua::MultiValue| Ok(f(&p.borrow())))?)
        };
        num("getX", |p| p.pos[0] as f64)?;
        num("getY", |p| p.pos[1] as f64)?;
        num("getZ", |p| p.z as f64)?;
        num("getCurrentSquareX", |p| pz_sim::square_of(p.pos).0 as f64)?;
        num("getCurrentSquareY", |p| pz_sim::square_of(p.pos).1 as f64)?;
        let p = player.clone();
        t.set("isDead", lua.create_function(move |_, _: mlua::MultiValue| Ok(p.borrow().health <= 0.0))?)?;
        let p = player.clone();
        t.set("isAlive", lua.create_function(move |_, _: mlua::MultiValue| Ok(p.borrow().health > 0.0))?)?;
        // BodyDamage is modelled as a single health pool here: 100 = unhurt.
        let body = lua.create_table()?;
        let p = player.clone();
        body.set("getHealth", lua.create_function(move |_, _: mlua::MultiValue| Ok(p.borrow().health as f64))?)?;
        let p = player.clone();
        body.set("IsInfected", lua.create_function(move |_, _: mlua::MultiValue| Ok(p.borrow().infected))?)?;
        t.set("getBodyDamage", lua.create_function(move |_, _: mlua::MultiValue| Ok(body.clone()))?)?;
        let g = lua.globals();
        let tt = t.clone();
        g.set("getPlayer", lua.create_function(move |_, ()| Ok(tt.clone()))?)?;
        g.set("getSpecificPlayer", lua.create_function(move |_, _: i32| Ok(t.clone()))?)
    }
}
