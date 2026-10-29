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
    pub crit_chance: f32,
    pub crit_dmg_multiplier: f32,
    pub push_back_mod: f32,
    /// `SoundRadius` / `SoundVolume`: how far a swing of this weapon is heard.
    pub sound_radius: f32,
    pub sound_volume: f32,
    /// Firearm stats; `ammo_type` is `Some` for ranged weapons.
    pub ammo_type: Option<String>,
    pub hit_chance: f32,
    pub min_sight_range: f32,
    pub max_sight_range: f32,
    pub recoil_delay: f32,
    pub max_ammo: f32,
    /// `Categories` contains `base:smallblade` (such weapons never stagger zombies).
    pub small_blade: bool,
    /// `Tags = base:a;base:b`
    pub tags: Vec<String>,
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
        crit_chance: num(b, "CriticalChance"),
        crit_dmg_multiplier: num(b, "CritDmgMultiplier"),
        ammo_type: b.prop("AmmoType").map(str::to_string),
        hit_chance: num(b, "HitChance"),
        min_sight_range: num(b, "MinSightRange"),
        max_sight_range: num(b, "MaxSightRange"),
        recoil_delay: num(b, "RecoilDelay"),
        max_ammo: num(b, "MaxAmmo"),
        sound_radius: b.prop("SoundRadius").and_then(|v| v.parse().ok()).unwrap_or(0.0),
        sound_volume: b.prop("SoundVolume").and_then(|v| v.parse().ok()).unwrap_or(0.0),
        push_back_mod: b.prop("PushBackMod").and_then(|v| v.parse().ok()).unwrap_or(1.0),
        tags: b.prop("Tags").map(|t| t.split(";").map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default(),
        small_blade: b.prop("Categories").is_some_and(|c| c.contains("smallblade")),
    }
}

impl ItemDef {
    pub fn is_firearm(&self) -> bool { self.ammo_type.is_some() }
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
    /// Ids of all items carrying `tag` (sorted, for deterministic matching).
    pub fn with_tag(&self, tag: &str) -> Vec<&str> {
        let mut v: Vec<&str> = self.items.values().filter(|d| d.tags.iter().any(|t| t == tag)).map(|d| d.id.as_str()).collect();
        v.sort_unstable();
        v
    }
    pub fn get(&self, id: &str) -> Option<&ItemDef> { self.items.get(id) }
    pub fn len(&self) -> usize { self.items.len() }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }
}

/// A container with a weight capacity (PZ's default player capacity is 8 before strength bonuses).
#[derive(Clone)]
pub struct Inventory { capacity: f32, stacks: Vec<(String, u32)>, weight: f32 }

#[derive(Debug, PartialEq)]
pub enum AddError { UnknownItem, TooHeavy }

/// IsoGameCharacter.maxWeightBase: a character carries 8 weight units before strength bonuses.
pub const BASE_MAX_WEIGHT: f32 = 8.0;

impl Inventory {
    /// A player's main inventory (capacity 8).
    pub fn player() -> Self { Self::new(BASE_MAX_WEIGHT) }

    /// IsoGameCharacter.isOverEncumbered: carried weight / max weight > 1.
    pub fn over_encumbered(&self) -> bool { self.weight / self.capacity > 1.0 }

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
