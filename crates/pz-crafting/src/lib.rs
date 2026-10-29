//! `craftRecipe` scripts: parsing and crafting against an [`Inventory`].
use pz_items::{Inventory, ItemDb};
use pz_scripts::{find_all, parse, Block, Entry};
use std::{collections::HashMap, path::Path};

#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    /// Any of these item ids (`[Base.A;Base.B]`).
    Items(Vec<String>),
    /// Any item carrying this tag (`tags[base:x]`).
    Tag(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Input { pub count: u32, pub source: Source, pub keep: bool }

#[derive(Debug, Clone, PartialEq)]
pub struct Recipe {
    pub name: String,
    /// Action time in the game's units.
    pub time: f32,
    pub inputs: Vec<Input>,
    pub outputs: Vec<(String, u32)>,
    /// False when the recipe uses fluids, item mappers or other features not ported yet.
    pub supported: bool,
}

fn parse_item_line(line: &str) -> Option<(u32, Source, bool, String)> {
    let rest = line.strip_prefix("item ")?;
    let (count, rest) = rest.split_once(' ')?;
    let count: u32 = count.parse().ok()?;
    let rest = rest.trim();
    let keep = rest.contains("mode:keep");
    let source = if let Some(t) = rest.strip_prefix("tags[") {
        Source::Tag(t.split(']').next()?.split(';').next()?.to_string())
    } else if let Some(l) = rest.strip_prefix('[') {
        Source::Items(l.split(']').next()?.split(';').map(|s| s.trim().to_string()).collect())
    } else {
        Source::Items(vec![rest.split_whitespace().next()?.to_string()])
    };
    Some((count, source, keep, rest.to_string()))
}

fn recipe(b: &Block) -> Recipe {
    let mut supported = b.blocks("itemMapper").next().is_none();
    let (mut inputs, mut outputs) = (vec![], vec![]);
    for blk in b.blocks("inputs") {
        for e in &blk.entries {
            let Entry::Line(l) = e else { continue };
            match parse_item_line(l) {
                Some((count, source, keep, rest)) => {
                    if rest.contains("mappers[") || rest.contains("mapper:") { supported = false; }
                    inputs.push(Input { count, source, keep });
                }
                None => supported = false, // fluids, energy, ...
            }
        }
    }
    for blk in b.blocks("outputs") {
        for e in &blk.entries {
            let Entry::Line(l) = e else { continue };
            match parse_item_line(l) {
                Some((count, Source::Items(ids), _, rest)) if ids.len() == 1 && !rest.contains("mapper:") => outputs.push((ids[0].clone(), count)),
                _ => supported = false,
            }
        }
    }
    Recipe { name: b.name.clone(), time: b.prop("time").and_then(|t| t.parse().ok()).unwrap_or(0.0), inputs, outputs, supported }
}

#[derive(Default)]
pub struct Recipes { map: HashMap<String, Recipe> }

impl Recipes {
    pub fn load(scripts: &Path) -> std::io::Result<Recipes> {
        let mut out = Recipes::default();
        let mut stack = vec![scripts.to_path_buf()];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(d)? {
                let p = e?.path();
                if p.is_dir() { stack.push(p); continue; }
                if p.extension().is_none_or(|x| x != "txt") { continue; }
                let Ok(entries) = parse(&String::from_utf8_lossy(&std::fs::read(&p)?)) else { continue };
                let mut mods = vec![];
                find_all(&entries, "module", &mut mods);
                for m in mods {
                    for b in m.blocks("craftRecipe") {
                        let r = recipe(b);
                        out.map.insert(format!("{}.{}", m.name, r.name), r);
                    }
                }
            }
        }
        Ok(out)
    }
    pub fn get(&self, id: &str) -> Option<&Recipe> { self.map.get(id) }
    pub fn len(&self) -> usize { self.map.len() }
    pub fn is_empty(&self) -> bool { self.map.is_empty() }
    pub fn supported(&self) -> impl Iterator<Item = (&String, &Recipe)> { self.map.iter().filter(|(_, r)| r.supported) }
}

#[derive(Debug, PartialEq)]
pub enum CraftError { Unsupported, Missing(String), NoRoom }

/// Which inventory stacks satisfy an input, as (item id, count) picks in deterministic order.
fn pick(inv: &Inventory, db: &ItemDb, input: &Input) -> Option<Vec<(String, u32)>> {
    let candidates: Vec<String> = match &input.source {
        Source::Items(ids) => ids.clone(),
        Source::Tag(t) => db.with_tag(t).into_iter().map(String::from).collect(),
    };
    let mut need = input.count;
    let mut picks = vec![];
    for id in candidates {
        let take = inv.count(&id).min(need);
        if take > 0 {
            picks.push((id, take));
            need -= take;
        }
        if need == 0 { return Some(picks); }
    }
    None
}

/// Craft `recipe` once: consume non-kept inputs, add outputs. Atomic: on error the inventory is unchanged.
pub fn craft(recipe: &Recipe, db: &ItemDb, inv: &mut Inventory) -> Result<(), CraftError> {
    if !recipe.supported { return Err(CraftError::Unsupported); }
    let mut consume = vec![];
    for input in &recipe.inputs {
        let picks = pick(inv, db, input).ok_or_else(|| CraftError::Missing(format!("{:?}", input.source)))?;
        if !input.keep { consume.extend(picks); }
    }
    // Work on a copy so a failed craft (no room for the outputs) leaves the inventory untouched.
    let mut trial = inv.clone();
    for (id, n) in &consume { trial.remove(db, id, *n); }
    for (id, n) in &recipe.outputs { trial.add(db, id, *n).map_err(|_| CraftError::NoRoom)?; }
    *inv = trial;
    Ok(())
}
