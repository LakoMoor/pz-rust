//! Typed item database (from the game's scripts) and a weight-limited inventory.
use pz_scripts::{find_all, parse, Block};
use std::{collections::HashMap, path::Path};

#[derive(Debug, Clone, PartialEq)]
pub struct ItemDef {
    /// Full id, e.g. `Base.Axe`.
    pub id: String,
    pub display_category: String,
    pub item_type: String,
    pub weight: f32,
    pub icon: String,
    /// Melee stats (0 for non-weapons).
    pub min_damage: f32,
    pub max_damage: f32,
    pub max_range: f32,
    pub swing_time: f32,
}

#[derive(Default)]
pub struct ItemDb { items: HashMap<String, ItemDef> }

fn num(b: &Block, k: &str) -> f32 { b.prop(k).and_then(|v| v.parse().ok()).unwrap_or(0.0) }

fn def(module: &str, b: &Block) -> ItemDef {
    ItemDef {
        id: format!("{module}.{}", b.name),
        display_category: b.prop("DisplayCategory").unwrap_or("").to_string(),
        item_type: b.prop("ItemType").unwrap_or("").to_string(),
        // PZ's default item weight is 1.
        weight: b.prop("Weight").and_then(|w| w.parse().ok()).unwrap_or(1.0),
        icon: b.prop("Icon").unwrap_or("").to_string(),
        min_damage: num(b, "MinDamage"),
        max_damage: num(b, "MaxDamage"),
        max_range: num(b, "MaxRange"),
        swing_time: num(b, "Swingtime"),
    }
}

impl ItemDb {
    /// Load every `item` from all `*.txt` under `media/scripts`.
    pub fn load(scripts: &Path) -> std::io::Result<ItemDb> {
        let mut db = ItemDb::default();
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
                    for b in m.blocks("item") { let d = def(&m.name, b); db.items.insert(d.id.clone(), d); }
                }
            }
        }
        Ok(db)
    }
    pub fn get(&self, id: &str) -> Option<&ItemDef> { self.items.get(id) }
    pub fn len(&self) -> usize { self.items.len() }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

/// A container with a weight capacity (PZ's default player capacity is 8 before strength bonuses).
pub struct Inventory { capacity: f32, stacks: Vec<(String, u32)>, weight: f32 }

#[derive(Debug, PartialEq)]
pub enum AddError { UnknownItem, TooHeavy }

impl Inventory {
    pub fn new(capacity: f32) -> Self { Self { capacity, stacks: Vec::new(), weight: 0.0 } }
    pub fn weight(&self) -> f32 { self.weight }
    pub fn count(&self, id: &str) -> u32 { self.stacks.iter().find(|(i, _)| i == id).map_or(0, |s| s.1) }

    pub fn add(&mut self, db: &ItemDb, id: &str, n: u32) -> Result<(), AddError> {
        let d = db.get(id).ok_or(AddError::UnknownItem)?;
        let w = d.weight * n as f32;
        if self.weight + w > self.capacity + 1e-4 { return Err(AddError::TooHeavy); }
        match self.stacks.iter_mut().find(|(i, _)| i == id) {
            Some(s) => s.1 += n,
            None => self.stacks.push((id.to_string(), n)),
        }
        self.weight += w;
        Ok(())
    }

    /// Remove up to `n`; returns how many were removed.
    pub fn remove(&mut self, db: &ItemDb, id: &str, n: u32) -> u32 {
        let Some(i) = self.stacks.iter().position(|(s, _)| s == id) else { return 0 };
        let k = n.min(self.stacks[i].1);
        self.stacks[i].1 -= k;
        self.weight -= db.get(id).map_or(0.0, |d| d.weight) * k as f32;
        if self.stacks[i].1 == 0 { self.stacks.remove(i); }
        k
    }
}
