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

/// `BaseAction` / `LuaTimedActionNew` and the per-tick action loop of `IsoGameCharacter.update`.
/// Progress advances by the game-time multiplier each tick; `finished()` is `currentTime >= maxTime`.
const TIMED_ACTIONS: &str = r#"
LuaTimedActionNew = {}
LuaTimedActionNew.__index = LuaTimedActionNew
function LuaTimedActionNew.new(t, chr)
    local a = setmetatable({ table = t, chr = chr, currentTime = -2, maxTime = t.maxTime, delta = 0,
        lastTime = -1, prevLastTime = 0, started = false, forceStop = false, forceComplete = false, loopAction = t.loopedAction or false }, LuaTimedActionNew)
    return a
end
local function call(a, name)
    local f = a.table[name]
    if f then return f(a.table) end
end
function LuaTimedActionNew:valid() return call(self, "isValid") == true end
function LuaTimedActionNew:finished() return self.currentTime >= self.maxTime and self.maxTime ~= -1 end
function LuaTimedActionNew:isStarted() return self.started end
function LuaTimedActionNew:getJobDelta() return self.delta end
function LuaTimedActionNew:setJobDelta(d) self.currentTime = self.maxTime * d; self.delta = d end
function LuaTimedActionNew:resetJobDelta() self.delta = 0; self.currentTime = 0 end
function LuaTimedActionNew:setCurrentTime(t) self.currentTime = t end
function LuaTimedActionNew:forceStop() self.forceStop = true end
function LuaTimedActionNew:forceComplete() self.forceComplete = true end
function LuaTimedActionNew:setTime(t) self.maxTime = t end
function LuaTimedActionNew:setActionAnim() end
function LuaTimedActionNew:setAnimVariable() end
function LuaTimedActionNew:setOverrideHandModelsObject() end
function LuaTimedActionNew:setOverrideHandModelsString() end
function LuaTimedActionNew:overrideWeaponType() end
function LuaTimedActionNew:restoreWeaponType() end
function LuaTimedActionNew:waitToStart()
    if call(self, "waitToStart") == false or self.table.waitToStart == nil then
        self.started = true
        self.forceComplete, self.forceStop = false, false
        self.currentTime = 0
        call(self, "start")
    end
end
function LuaTimedActionNew:update(mult)
    self.prevLastTime, self.lastTime = self.lastTime, self.currentTime
    self.currentTime = self.currentTime + mult
    if self.currentTime < 0 then self.currentTime = 0 end
    if self.maxTime == 0 then self.delta = 0 elseif self.maxTime ~= -1 then self.delta = math.min(self.currentTime / self.maxTime, 1) end
    call(self, "update")
end
function LuaTimedActionNew:hasStalled()
    if not self.started then return false end
    return (self.lastTime == self.currentTime and self.lastTime == self.prevLastTime and self.lastTime < 0) or self.currentTime < 0
end
function LuaTimedActionNew:getTable() return self.table end
function LuaTimedActionNew:stop() call(self, "stop") end
function LuaTimedActionNew:perform() call(self, "perform") end
function LuaTimedActionNew:complete() call(self, "complete") end

-- Java-side names the queue script touches at load time and on every tick.
function instanceof(o, name)
    return type(o) == "table" and o.__classes ~= nil and o.__classes[name] == true
end
for _, n in ipairs({ "ClimbThroughWindowState", "ClimbOverFenceState", "ClimbOverWallState", "ClimbSheetRopeState",
                     "ClimbDownSheetRopeState", "CloseWindowState", "OpenWindowState" }) do
    local inst = {}
    _G[n] = { instance = function() return inst end }
end
function table.wipe(t) for k in pairs(t) do t[k] = nil end return t end
ISLogSystem = { logAction = function() end }
HaloTextHelper = { addBadText = function() end }
function getCore() return { getOptionTimedActionGameSpeedReset = function() return false end } end

function __startAction(chr, act)
    chr.__actions = { act }
    if act:valid() then act:waitToStart() end
end
function __updateActions(chr, mult)
    local list = chr.__actions
    local act = list and list[1]
    if not act then return end
    local valid = act:valid()
    if valid and not act.started then act:waitToStart()
    elseif valid and not act:finished() and not act.forceComplete and not act.forceStop then act:update(mult) end
    if not valid or act:finished() or act.forceComplete or act.forceStop then
        local done = act:finished() or act.forceComplete
        if done then act:perform(); act:complete(); valid = true end
        if (done and not act.loopAction) or act.forceStop or not valid then
            if act.started and (act.forceStop or not valid) then act:stop() end
            table.remove(list, 1)
        end
        act.forceComplete = false
    end
end
"#;

pub struct Runtime { pub lua: Lua }

impl Runtime {
    pub fn new() -> Result<Self> {
        let lua = Lua::new();
        // Kahlua's `print` and the `getText` fallback are the only globals scripts need at load time.
        lua.load(PRELUDE).exec()?;
        lua.load(TIMED_ACTIONS).exec()?;
        Ok(Self { lua })
    }

    /// `triggerEvent(name, args...)`: call every callback registered with `Events.<name>.Add`.
    pub fn trigger<A: mlua::IntoLuaMulti>(&self, name: &str, args: A) -> Result<()> {
        let f: mlua::Function = self.lua.globals().get("triggerEvent")?;
        let mut v = args.into_lua_multi(&self.lua)?;
        v.push_front(mlua::Value::String(self.lua.create_string(name)?));
        f.call::<()>(v)
    }

    /// One game tick of the player's action queue (`mult` is the game-time multiplier, 1.0 at normal speed).
    pub fn tick_actions(&self, mult: f64) -> Result<()> {
        let f: mlua::Function = self.lua.globals().get("__tickPlayerActions")?;
        f.call::<()>(mult)
    }

    /// Let `require "Name"` find `Name.lua` under `root` (the game resolves requires against the
    /// `shared`, `client` and `server` folders).
    pub fn add_lua_root(&self, root: &Path) -> Result<()> {
        let package: mlua::Table = self.lua.globals().get("package")?;
        let old: String = package.get("path")?;
        let root = root.display().to_string().replace('\\', "/");
        package.set("path", format!("{root}/?.lua;{old}"))
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
        // Plain flags scripts set on the character; nothing in the simulation reads them yet.
        for (setter, field) in [("setTimedActionToRetrigger", "__retrigger"), ("setIsFarming", "__farming")] {
            let me = t.clone();
            t.set(setter, lua.create_function(move |_, (_this, v): (mlua::Value, mlua::Value)| me.set(field, v))?)?;
        }
        lua.load(r#"
            local chr = ...
            chr.__classes = { IsoGameCharacter = true, IsoPlayer = true }
            chr.__actions = {}
            function chr:getCharacterActions()
                local list = self.__actions
                return { contains = function(_, a) for _, x in ipairs(list) do if x == a then return true end end return false end,
                         isEmpty = function() return #list == 0 end }
            end
            function chr:getTimedActionToRetrigger() return self.__retrigger end
            function chr:isAsleep() return false end
            function chr:isFarming() return self.__farming == true end
            function chr:isDraggingCorpse() return false end
            function chr:isLocalPlayer() return true end
            function chr:getCurrentState() return nil end
            function chr:StopAllActionQueue()
                local a = self.__actions[1]
                if a then if a.started then a:stop() else local f = a.table.interruptWaitToStart; if f then f(a.table) end end end
                self.__actions = {}
            end
        "#).call::<()>(t.clone())?;
        let start: mlua::Function = lua.globals().get("__startAction")?;
        let tt0 = t.clone();
        t.set("StartAction", lua.create_function(move |_, (_this, act): (mlua::Value, mlua::Value)| start.call::<()>((tt0.clone(), act)))?)?;
        let update: mlua::Function = lua.globals().get("__updateActions")?;
        let tt1 = t.clone();
        self.lua.globals().set("__tickPlayerActions", lua.create_function(move |_, mult: f64| update.call::<()>((tt1.clone(), mult)))?)?;
        let g = lua.globals();
        let tt = t.clone();
        g.set("getPlayer", lua.create_function(move |_, ()| Ok(tt.clone()))?)?;
        g.set("getSpecificPlayer", lua.create_function(move |_, _: i32| Ok(t.clone()))?)
    }
}

impl Runtime {
    /// `getCell():getGridSquare(x, y, z)`: nil outside loaded map data, otherwise a square with the collision
    /// queries scripts use. Flags are read when the square is requested.
    pub fn bind_world(&self, world: Rc<RefCell<pz_world::World>>) -> Result<()> {
        use pz_world::flag as f;
        let lua = &self.lua;
        let cell = lua.create_table()?;
        let world2 = world.clone();
        cell.set("getGridSquare", lua.create_function(move |lua, (_this, x, y, z): (mlua::Value, i32, i32, i32)| {
            let mut w = world.borrow_mut();
            if z < 0 || !w.ensure(x, y) { return Ok(mlua::Value::Nil); }
            let bits = w.flags(x, y, z as usize);
            let sq = lua.create_table()?;
            let has = |mask: u16| bits & mask != 0;
            let (solid, door, window) = (has(f::SOLID), has(f::DOOR_N | f::DOOR_W), has(f::WINDOW_N | f::WINDOW_W));
            let (floor, open) = (w.has_floor(x, y, z as usize), w.door_open(x, y, z as usize));
            for (name, v) in [("getX", x as f64), ("getY", y as f64), ("getZ", z as f64)] {
                sq.set(name, lua.create_function(move |_, _: mlua::MultiValue| Ok(v))?)?;
            }
            for (name, v) in [("isSolid", solid), ("isFree", !solid), ("HasStairs", has(f::STAIR_BN | f::STAIR_MN | f::STAIR_TN | f::STAIR_BW | f::STAIR_MW | f::STAIR_TW)),
                              ("isDoor", door), ("isWindow", window), ("isDoorOpen", open), ("hasFloor", floor)] {
                sq.set(name, lua.create_function(move |_, _: mlua::MultiValue| Ok(v))?)?;
            }
            // Interacting changes the live world (the same toggle the player's E key performs).
            let w2 = world2.clone();
            sq.set("toggleDoor", lua.create_function(move |_, _: mlua::MultiValue| Ok(w2.borrow_mut().toggle_door(x, y, z as usize)))?)?;
            Ok(mlua::Value::Table(sq))
        })?)?;
        let c2 = cell.clone();
        lua.globals().set("getCell", lua.create_function(move |_, ()| Ok(c2.clone()))?)
    }
}
