use pz_lua::Runtime;
use std::sync::Arc;
use std::path::PathBuf;

fn lua_dir() -> Option<PathBuf> {
    let dir = std::env::var("PZ_DIR").unwrap_or_else(|_| r"E:\Games\steamapps\common\ProjectZomboid".into());
    let p = PathBuf::from(dir).join("media/lua");
    p.is_dir().then_some(p)
}

#[test]
fn the_games_own_luautils_runs_on_the_embedded_vm() {
    let Some(dir) = lua_dir() else { return };
    let rt = Runtime::new().unwrap();
    rt.run_file(&dir.join("shared/luautils.lua")).unwrap();
    let n: usize = rt.lua.load(r#"local t = luautils.split("a,b,c", ","); return #t"#).eval().unwrap();
    assert_eq!(n, 3);
}

#[test]
fn how_many_shared_scripts_load_without_the_java_api() {
    let Some(dir) = lua_dir() else { return };
    fn walk(d: &std::path::Path, out: &mut Vec<PathBuf>) {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() { walk(&p, out) } else if p.extension().is_some_and(|x| x == "lua") { out.push(p) }
        }
    }
    let mut files = Vec::new();
    walk(&dir.join("shared"), &mut files);
    let (mut ok, mut syntax) = (0, 0);
    for f in &files {
        let rt = Runtime::new().unwrap();
        // Compile only: execution needs the Java-side globals (Events, ISBaseObject, ...), which come later.
        match rt.lua.load(&std::fs::read(f).unwrap()[..]).into_function() { Ok(_) => ok += 1, Err(e) => { syntax += 1; eprintln!("{}: {e}", f.display()) } }
    }
    eprintln!("compiled {ok}/{}", files.len());
    assert!(ok * 100 >= files.len() * 99, "{syntax} of {} shared scripts fail to compile under Lua 5.1", files.len());
}

#[test]
fn class_hierarchy_and_events_work_with_the_games_isbaseobject() {
    let Some(dir) = lua_dir() else { return };
    let rt = Runtime::new().unwrap();
    rt.run_file(&dir.join("shared/ISBaseObject.lua")).unwrap();
    let ok: bool = rt.lua.load(r#"
        Animal = ISBaseObject:derive("Animal")
        Dog = Animal:derive("Dog")
        local d = Dog:new()
        return ISBaseObject:instanceof(d, "Animal") and ISBaseObject:instanceof(d, "Dog") and not ISBaseObject:instanceof(d, "Cat")
    "#).eval().unwrap();
    assert!(ok);
    rt.lua.load(r#"
        hits = {}
        Events.OnTick.Add(function(n) table.insert(hits, n) end)
        local f = function(n) table.insert(hits, -n) end
        Events.OnTick.Add(f)
        Events.OnTick.Remove(f)
    "#).exec().unwrap();
    rt.trigger("OnTick", 7).unwrap();
    rt.trigger("OnTick", 8).unwrap();
    rt.trigger("OnNeverRegistered", ()).unwrap();
    let hits: Vec<i32> = rt.lua.globals().get::<mlua::Table>("hits").unwrap().sequence_values().map(|v| v.unwrap()).collect();
    assert_eq!(hits, vec![7, 8]);
}

#[test]
fn scripts_can_look_up_item_definitions_from_the_real_item_scripts() {
    let Some(dir) = lua_dir() else { return };
    let scripts = dir.parent().unwrap().join("scripts");
    let db = std::sync::Arc::new(pz_items::ItemDb::load(&scripts).unwrap());
    let axe_weight = db.get("Base.Axe").unwrap().weight as f64;
    let rt = Runtime::new().unwrap();
    rt.bind_items(db).unwrap();
    let (w, ranged, pistol_ranged, name): (f64, bool, bool, String) = rt.lua.load(r#"
        local axe = getScriptManager():getItem("Base.Axe")
        local pistol = InventoryItemFactory.CreateItem("Base.Pistol")
        return axe:getActualWeight(), axe:isRanged(), pistol:isRanged(), pistol:getFullType()
    "#).eval().unwrap();
    assert_eq!((w, ranged, pistol_ranged, name.as_str()), (axe_weight, false, true, "Base.Pistol"));
    let missing: bool = rt.lua.load(r#"return getScriptManager():getItem("Base.NoSuchThing") == nil"#).eval().unwrap();
    assert!(missing);
}

#[test]
fn scripts_see_the_live_player_state() {
    use std::{cell::RefCell, rc::Rc};
    let rt = Runtime::new().unwrap();
    let player = Rc::new(RefCell::new(pz_sim::Player::new(10.4, -3.0)));
    rt.bind_player(player.clone()).unwrap();
    let read = |rt: &Runtime| -> (f64, f64, f64, bool) {
        rt.lua.load("local p = getPlayer(); return p:getX(), p:getCurrentSquareX(), p:getBodyDamage():getHealth(), p:isDead()").eval().unwrap()
    };
    assert_eq!(read(&rt), (10.4f32 as f64, 10.0, 100.0, false));
    // Simulation changes show up on the next call, no copying.
    player.borrow_mut().health = 0.0;
    player.borrow_mut().pos[0] = 11.6;
    assert_eq!(read(&rt), (11.6f32 as f64, 12.0, 0.0, true));
    let same: bool = rt.lua.load("return getSpecificPlayer(0) == getPlayer()").eval().unwrap();
    assert!(same);
}

#[test]
fn scripts_query_squares_of_the_real_map() {
    use std::{cell::RefCell, rc::Rc};
    let Some(dir) = lua_dir() else { return };
    let media = dir.parent().unwrap().to_path_buf();
    let mut w = pz_world::World::new(media.join("maps").join("Muldraugh, KY"));
    w.load_tile_flags(&media);
    // Find a solid and a free ground square near Muldraugh for the script to ask about.
    let (mut solid, mut free) = (None, None);
    'scan: for y in 1500..1560 {
        for x in 12500..12560 {
            let fl = w.flags(x, y, 0);
            if fl & pz_world::flag::SOLID != 0 && solid.is_none() { solid = Some((x, y)); }
            if fl == 0 && w.has_floor(x, y, 0) && free.is_none() { free = Some((x, y)); }
            if solid.is_some() && free.is_some() { break 'scan; }
        }
    }
    let (solid, free) = (solid.unwrap(), free.unwrap());
    let rt = Runtime::new().unwrap();
    rt.bind_world(Rc::new(RefCell::new(w))).unwrap();
    let q = |(x, y): (i32, i32)| -> (bool, bool, bool) {
        rt.lua.load(format!("local s = getCell():getGridSquare({x}, {y}, 0); return s:isSolid(), s:isFree(), s:hasFloor()")).eval().unwrap()
    };
    assert_eq!(q(solid).0, true);
    assert_eq!(q(free), (false, true, true));
    let nowhere: bool = rt.lua.load("return getCell():getGridSquare(-5000, -5000, 0) == nil").eval().unwrap();
    assert!(nowhere);
}

#[test]
fn a_script_can_open_and_close_a_door_in_the_live_world() {
    use std::{cell::RefCell, rc::Rc};
    use pz_world::flag;
    let Some(dir) = lua_dir() else { return };
    let media = dir.parent().unwrap().to_path_buf();
    let mut w = pz_world::World::new(media.join("maps").join("Muldraugh, KY"));
    w.load_tile_flags(&media);
    let door = (12600..12700).flat_map(|x| (1600..1700).map(move |y| (x, y))).find(|&(x, y)| w.flags(x, y, 0) & flag::DOOR_W != 0).expect("a door");
    let world = Rc::new(RefCell::new(w));
    let rt = Runtime::new().unwrap();
    rt.bind_world(world.clone()).unwrap();
    let (x, y) = door;
    let (a, b, c): (bool, bool, bool) = rt.lua.load(format!(
        "local s = getCell():getGridSquare({x}, {y}, 0); local o = s:toggleDoor(); local o2 = getCell():getGridSquare({x}, {y}, 0):isDoorOpen(); s:toggleDoor(); return o, o2, getCell():getGridSquare({x}, {y}, 0):isDoorOpen()")).eval().unwrap();
    assert_eq!((a, b, c), (true, true, false));
    assert!(!world.borrow().door_open(x, y, 0));
}

#[test]
fn a_real_timed_action_runs_through_start_update_perform() {
    use std::{cell::RefCell, rc::Rc};
    let Some(dir) = lua_dir() else { return };
    let rt = Runtime::new().unwrap();
    rt.bind_player(Rc::new(RefCell::new(pz_sim::Player::new(0.0, 0.0)))).unwrap();
    rt.add_lua_root(&dir.join("shared")).unwrap();
    rt.lua.load("ISTimedActionQueue = {}").exec().unwrap();
    rt.run_file(&dir.join("shared/TimedActions/ISBaseTimedAction.lua")).unwrap();
    // A game action built on the real base class. Moodles and body damage (adjustMaxTime) are not bound yet,
    // so the test action keeps its duration as is.
    rt.lua.load(r#"
        log = {}
        Walk = ISBaseTimedAction:derive("Walk")
        function Walk:isValid() return not self.cancelled end
        function Walk:adjustMaxTime(t) return t end
        function Walk:start() table.insert(log, "start") end
        function Walk:update() table.insert(log, "update " .. string.format("%.2f", self:getJobDelta())) end
        function Walk:stop() table.insert(log, "stop") end
        function Walk:perform() table.insert(log, "perform") end
        function Walk:complete() table.insert(log, "complete") end
        function Walk:new(chr, t) local o = ISBaseTimedAction.new(self, chr); o.maxTime = t; return o end
        player = getPlayer()
        a = Walk:new(player, 4); a:begin()
    "#).exec().unwrap();
    for _ in 0..6 { rt.tick_actions(1.0).unwrap(); }
    let log: String = rt.lua.load(r#"return table.concat(log, ",")"#).eval().unwrap();
    // begin() starts the action at once (StartAction -> waitToStart -> start); four ticks reach maxTime, then it completes.
    assert_eq!(log, "start,update 0.25,update 0.50,update 0.75,update 1.00,perform,complete");
    // A cancelled action is stopped, not completed.
    rt.lua.load(r#"log = {}; b = Walk:new(player, 100); b:begin()"#).exec().unwrap();
    rt.tick_actions(1.0).unwrap(); rt.tick_actions(1.0).unwrap();
    rt.lua.load("b.cancelled = true").exec().unwrap();
    rt.tick_actions(1.0).unwrap();
    let log: String = rt.lua.load(r#"return table.concat(log, ",")"#).eval().unwrap();
    assert_eq!(log, "start,update 0.01,update 0.02,stop");
}

#[test]
fn the_real_action_queue_chains_actions_one_after_another() {
    use std::{cell::RefCell, rc::Rc};
    let Some(dir) = lua_dir() else { return };
    let rt = Runtime::new().unwrap();
    rt.bind_player(Rc::new(RefCell::new(pz_sim::Player::new(0.0, 0.0)))).unwrap();
    rt.add_lua_root(&dir.join("shared")).unwrap();
    rt.run_file(&dir.join("shared/TimedActions/ISBaseTimedAction.lua")).unwrap();
    rt.run_file(&dir.join("client/TimedActions/ISTimedActionQueue.lua")).unwrap();
    rt.lua.load(r#"
        log = {}
        Step = ISBaseTimedAction:derive("Step")
        function Step:isValid() return true end
        function Step:adjustMaxTime(t) return t end
        function Step:start() table.insert(log, self.name .. ":start") end
        function Step:perform()
            table.insert(log, self.name .. ":perform")
            ISBaseTimedAction.perform(self)
        end
        function Step:new(chr, name, t) local o = ISBaseTimedAction.new(self, chr); o.name = name; o.maxTime = t; return o end
        player = getPlayer()
        ISTimedActionQueue.add(Step:new(player, "a", 2))
        ISTimedActionQueue.add(Step:new(player, "b", 2))
        assert(ISTimedActionQueue.hasActionType(player, "Step"))
    "#).exec().unwrap();
    for _ in 0..8 { rt.tick_actions(1.0).unwrap(); }
    let log: String = rt.lua.load(r#"return table.concat(log, ",")"#).eval().unwrap();
    assert_eq!(log, "a:start,a:perform,b:start,b:perform");
    let empty: bool = rt.lua.load("return not ISTimedActionQueue.hasActionType(player, 'Step') and player:getCharacterActions():isEmpty()").eval().unwrap();
    assert!(empty);
    // Clearing cancels what is queued and stops the running action.
    rt.lua.load(r#"log = {}; ISTimedActionQueue.add(Step:new(player, "c", 50)); ISTimedActionQueue.add(Step:new(player, "d", 5))"#).exec().unwrap();
    rt.tick_actions(1.0).unwrap();
    rt.lua.load("ISTimedActionQueue.clear(player)").exec().unwrap();
    let left: bool = rt.lua.load("return #ISTimedActionQueue.getTimedActionQueue(player).queue == 0 and player:getCharacterActions():isEmpty()").eval().unwrap();
    assert!(left);
}

#[test]
fn scripts_use_the_players_inventory() {
    use std::{cell::RefCell, rc::Rc};
    let Some(dir) = lua_dir() else { return };
    let db = Arc::new(pz_items::ItemDb::load(&dir.parent().unwrap().join("scripts")).unwrap());
    let inv = Rc::new(RefCell::new(pz_items::Inventory::player()));
    let rt = Runtime::new().unwrap();
    rt.bind_player(Rc::new(RefCell::new(pz_sim::Player::new(0.0, 0.0)))).unwrap();
    rt.bind_inventory(db.clone(), inv.clone()).unwrap();
    let (has_before, n, has_after, gone, unknown, weight): (bool, i64, bool, bool, bool, f64) = rt.lua.load(r#"
        local inv = getPlayer():getInventory()
        local before = inv:contains("Axe")
        inv:AddItem("Base.Bullets9mm"); inv:AddItem("Bullets9mm"); inv:AddItem("Axe")
        local n = inv:getItemCount("Bullets9mm")
        local after = inv:contains("Base.Axe")
        inv:RemoveOneOf("Axe")
        return before, n, after, not inv:contains("Axe"), inv:AddItem("Base.NoSuchThing") == nil, inv:getCapacityWeight()
    "#).eval().unwrap();
    assert_eq!((has_before, n, has_after, gone, unknown), (false, 2, true, true, true));
    assert_eq!(inv.borrow().count("Base.Bullets9mm"), 2);
    assert!((weight - inv.borrow().weight() as f64).abs() < 1e-6);
}

#[test]
fn the_real_adjust_max_time_reacts_to_moodles_and_pain() {
    use std::{cell::RefCell, rc::Rc};
    let Some(dir) = lua_dir() else { return };
    let rt = Runtime::new().unwrap();
    rt.bind_player(Rc::new(RefCell::new(pz_sim::Player::new(0.0, 0.0)))).unwrap();
    rt.add_lua_root(&dir.join("shared")).unwrap();
    rt.run_file(&dir.join("shared/TimedActions/ISBaseTimedAction.lua")).unwrap();
    let t = |setup: &str| -> f64 {
        rt.lua.load(format!("local p = getPlayer(); p.__moodles, p.__pain = {{}}, {{}}; {setup}; return ISBaseTimedAction:new(p):adjustMaxTime(100)")).eval().unwrap()
    };
    assert_eq!(t("local _ = 0"), 100.0);
    assert_eq!(t("p:setMoodleLevel('UNHAPPY', 2)"), 150.0);
    assert_eq!(t("p:setMoodleLevel('UNHAPPY', 2); p:setMoodleLevel('DRUNK', 4)"), 300.0);
    // Hurt hands: 0..3 are Hand_L..ForeArm_R, each pain point adds 1/300 of the time.
    assert!((t("p:setPain(0, 30); p:setPain(3, 30)") - 120.0).abs() < 1e-9);
    // Instant actions are never stretched.
    let one: f64 = rt.lua.load("return ISBaseTimedAction:new(getPlayer()):adjustMaxTime(1)").eval().unwrap();
    assert_eq!(one, 1.0);
}

#[test]
fn scripts_read_wounds_from_the_simulated_body() {
    use std::{cell::RefCell, rc::Rc};
    let rt = Runtime::new().unwrap();
    let player = Rc::new(RefCell::new(pz_sim::Player::new(0.0, 0.0)));
    rt.bind_player(player.clone()).unwrap();
    let q = |code: &str| -> (f64, i64, bool, bool) {
        rt.lua.load(format!("local b = getPlayer():getBodyDamage(); {code}")).eval().unwrap()
    };
    assert_eq!(q("return b:getOverallBodyHealth(), b:getNumPartsBleeding(), b:getBodyPart(8):bleeding(), b:getBodyPart(8):scratched()"), (100.0, 0, false, false));
    player.borrow_mut().body.scratch(8, 0.5, 0.5);
    player.borrow_mut().body.parts[8].health = 50.0;
    let (overall, bleeding, head_bleeds, head_scratched) = q("return b:getOverallBodyHealth(), b:getNumPartsBleeding(), b:getBodyPart(8):bleeding(), b:getBodyPart(8):scratched()");
    assert!((overall - 70.0).abs() < 1e-3, "head 50% -> 30 damage, got {overall}");
    assert_eq!((bleeding, head_bleeds, head_scratched), (1, true, true));
    let none: bool = rt.lua.load("return getPlayer():getBodyDamage():getBodyPart(99) == nil").eval().unwrap();
    assert!(none);
}

#[test]
fn script_pain_comes_from_the_wounds() {
    use std::{cell::RefCell, rc::Rc};
    let rt = Runtime::new().unwrap();
    let player = Rc::new(RefCell::new(pz_sim::Player::new(0.0, 0.0)));
    rt.bind_player(player.clone()).unwrap();
    player.borrow_mut().body.parts[2].scratch_time = 10.0;
    let pain: f64 = rt.lua.load("return getPlayer():getBodyDamage():getBodyPart(2):getPain()").eval().unwrap();
    assert!((pain - 17.0).abs() < 1e-4);
}
