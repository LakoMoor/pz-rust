use pz_lua::Runtime;
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
