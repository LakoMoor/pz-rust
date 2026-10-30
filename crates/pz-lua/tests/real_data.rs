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
