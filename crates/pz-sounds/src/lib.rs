//! Sound definitions from the `sound` scripts (`media/scripts/generated/sounds/**`).
//!
//! Almost every sound names an FMOD Studio *event* that lives inside the game's `.bank` files; this crate only
//! knows which event a sound plays and with what settings. Decoding the banks and playing audio is separate work.
pub mod fsb5;
pub mod vorbis;
pub mod wav;
use pz_scripts::{find_all, parse, Block};
use std::{collections::HashMap, path::Path};

#[derive(Debug, Clone, PartialEq)]
pub struct Clip {
    /// FMOD event path such as `World/Weather/Thunder`, when the clip plays a bank event.
    pub event: Option<String>,
    /// Loose audio file for the few legacy clips (`file = media/sound/x.wav`).
    pub file: Option<String>,
    pub distance_max: Option<f32>,
    pub volume: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SoundDef {
    /// Script name, e.g. `Thunder`.
    pub name: String,
    pub category: String,
    pub looped: bool,
    pub clips: Vec<Clip>,
}

#[derive(Default)]
pub struct SoundDb { sounds: HashMap<String, SoundDef> }

fn flag(v: Option<&str>) -> bool { v.is_some_and(|v| v.eq_ignore_ascii_case("true")) }

fn clip(b: &Block) -> Clip {
    Clip {
        event: b.prop("event").map(str::to_string),
        file: b.prop("file").map(str::to_string),
        distance_max: b.prop("distanceMax").and_then(|v| v.parse().ok()),
        volume: b.prop("volume").and_then(|v| v.parse().ok()).unwrap_or(1.0),
    }
}

impl SoundDb {
    /// Load every `sound` block from all `*.txt` under `media/scripts`.
    pub fn load(scripts: &Path) -> std::io::Result<SoundDb> {
        let mut db = SoundDb::default();
        let mut stack = vec![scripts.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(d)? {
                let p = e?.path();
                if p.is_dir() { stack.push(p); continue; }
                if p.extension().is_none_or(|x| x != "txt") { continue; }
                let Ok(entries) = parse(&String::from_utf8_lossy(&std::fs::read(&p)?)) else { continue };
                let mut modules = vec![];
                find_all(&entries, "module", &mut modules);
                for m in modules {
                    for b in m.blocks("sound") {
                        let def = SoundDef {
                            name: b.name.clone(),
                            category: b.prop("category").unwrap_or("").to_string(),
                            looped: flag(b.prop("loop")),
                            clips: b.blocks("clip").map(clip).collect(),
                        };
                        db.sounds.insert(def.name.clone(), def);
                    }
                }
            }
        }
        Ok(db)
    }

    pub fn get(&self, name: &str) -> Option<&SoundDef> { self.sounds.get(name) }
    pub fn len(&self) -> usize { self.sounds.len() }
    pub fn is_empty(&self) -> bool { self.sounds.is_empty() }
    pub fn all(&self) -> impl Iterator<Item = &SoundDef> { self.sounds.values() }
}
