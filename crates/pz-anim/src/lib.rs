//! Animation timing read from the game's own data: clip lengths from the text `.X` files in `media/anims_X`, and
//! animation nodes (speed scale, timed events) from `media/AnimSets`. The simulation uses these to time states that
//! the original drives from animation events (door thumping, getting up, ...). Poses are not read, only times.

use std::{collections::HashMap, path::Path};

/// Length in seconds of the animation set in a text `.X` file: its last key time over `AnimTicksPerSecond`.
pub fn x_duration(text: &str) -> Option<f32> {
    let mut ticks_per_second = None;
    let (mut in_set, mut in_key, mut max_tick) = (false, false, 0u32);
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let t = line.trim();
        if t.starts_with("AnimTicksPerSecond") && t.ends_with('{') {
            if let Some(v) = lines.peek().and_then(|l| l.trim().trim_end_matches(';').parse::<f32>().ok()) {
                ticks_per_second = Some(v);
            }
        } else if t.starts_with("AnimationSet") {
            in_set = true;
        } else if in_set && t.starts_with("AnimationKey") {
            in_key = true;
        } else if in_key && t == "}" {
            in_key = false;
        } else if in_key {
            // Key lines look like `<tick>;<n>;<values>;;,`; the header lines hold a single number.
            let mut parts = t.split(';');
            if let (Some(a), Some(b)) = (parts.next(), parts.next()) {
                if !b.is_empty() {
                    if let Ok(tick) = a.parse::<u32>() {
                        max_tick = max_tick.max(tick);
                    }
                }
            }
        }
    }
    let tps = ticks_per_second?;
    if tps > 0.0 && max_tick > 0 { Some(max_tick as f32 / tps) } else { None }
}

/// A timed event on an animation node; `time_pc` is a fraction of the clip, 0..1.
#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub name: String,
    pub time_pc: f32,
}

/// An `AnimSets` node: which clip it plays, how fast, and its timed events.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnimNode {
    pub name: String,
    pub anim_name: String,
    pub speed_scale: f32,
    pub events: Vec<Event>,
}

fn tag<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let a = s.find(&open)? + open.len();
    let b = s[a..].find(&format!("</{name}>"))? + a;
    Some(s[a..b].trim())
}

/// Parse a node file. `base` is the node named by `x_extends`: its events are inherited, and an event block without
/// a name overrides the time of the inherited event in the same position.
pub fn parse_node(xml: &str, base: Option<&AnimNode>) -> AnimNode {
    let mut node = base.cloned().unwrap_or(AnimNode { speed_scale: 1.0, ..Default::default() });
    if let Some(v) = tag(xml, "m_Name") {
        node.name = v.to_string();
    }
    if let Some(v) = tag(xml, "m_AnimName") {
        node.anim_name = v.to_string();
    }
    if let Some(v) = tag(xml, "m_SpeedScale").and_then(|v| v.parse().ok()) {
        node.speed_scale = v;
    }
    let mut rest = xml;
    let mut slot = 0usize;
    let mut added: Vec<Event> = Vec::new();
    while let Some(i) = rest.find("<m_Events") {
        let after = &rest[i..];
        let end = after.find("</m_Events>").unwrap_or(after.len());
        let block = &after[..end];
        let name = tag(block, "m_EventName").map(str::to_string);
        let time = tag(block, "m_TimePc").and_then(|v| v.parse::<f32>().ok());
        match (name, base) {
            (Some(n), _) => added.push(Event { name: n, time_pc: time.unwrap_or(0.0) }),
            (None, Some(_)) => {
                if let (Some(t), Some(e)) = (time, node.events.get_mut(slot)) {
                    e.time_pc = t;
                }
                slot += 1;
            }
            _ => {}
        }
        rest = &after[end..];
    }
    node.events.extend(added);
    node
}

/// Time in seconds, from the start of a loop, of each event with this name: `time_pc * clip length / speed scale`.
pub fn event_times(node: &AnimNode, clip_seconds: f32, event: &str) -> Vec<f32> {
    node.events.iter().filter(|e| e.name == event).map(|e| e.time_pc * clip_seconds / node.speed_scale).collect()
}

/// Clip lengths by clip name for every `.X` file one folder below `dir` (the layout of `media/anims_X`).
pub fn clip_lengths(dir: &Path) -> HashMap<String, f32> {
    let mut out = HashMap::new();
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    for sub in rd.flatten() {
        let Ok(files) = std::fs::read_dir(sub.path()) else { continue };
        for f in files.flatten() {
            let p = f.path();
            if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("x")) {
                if let (Some(stem), Ok(text)) = (p.file_stem(), std::fs::read(&p)) {
                    if let Some(d) = x_duration(&String::from_utf8_lossy(&text)) {
                        out.insert(stem.to_string_lossy().into_owned(), d);
                    }
                }
            }
        }
    }
    out
}
