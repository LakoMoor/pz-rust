//! Lazy, cell-based view of a PZ map (tile columns at integer coordinates).
use pz_formats::lot::{decode_chunk, Level, LotHeader, LotPack, CHUNK};
use std::{collections::{HashMap, HashSet}, path::{Path, PathBuf}};

pub const CELL: i32 = 256;
const CHUNKS_PER_CELL: usize = (CELL as usize) / CHUNK;

struct Cell {
    names: Vec<String>,
    pack: Vec<u8>,
    chunks: HashMap<usize, Vec<Level>>,
}

/// Collision-relevant tile flags (OR-ed over a square's tiles).
pub mod flag {
    /// Whole square blocked (solid furniture, trees, ...).
    pub const SOLID: u16 = 1;
    /// Wall/window/frame on the square's north edge.
    pub const EDGE_N: u16 = 2;
    /// Wall/window/frame on the square's west edge.
    pub const EDGE_W: u16 = 4;
    /// Stair pieces (bottom / middle / top step) of a staircase running along N (y) or W (x).
    pub const STAIR_BN: u16 = 8;
    pub const STAIR_MN: u16 = 16;
    pub const STAIR_TN: u16 = 32;
    pub const STAIR_BW: u16 = 64;
    pub const STAIR_MW: u16 = 128;
    pub const STAIR_TW: u16 = 256;
    /// Door panel on the north / west edge (blocks like a wall while closed).
    pub const DOOR_N: u16 = 512;
    pub const DOOR_W: u16 = 1024;
    /// Window on the north / west edge (also sets the matching EDGE bit; can be climbed through).
    pub const WINDOW_N: u16 = 2048;
    pub const WINDOW_W: u16 = 4096;
    /// Hoppable fence / low wall on the north / west edge (also sets the matching EDGE bit; can be climbed over).
    pub const HOP_N: u16 = 8192;
    pub const HOP_W: u16 = 16384;
}

fn tile_flags(props: &[(String, String)]) -> u16 {
    let mut f = 0;
    for (k, _) in props {
        let k = k.as_str();
        if matches!(k, "solid" | "solidtrans") { f |= flag::SOLID; }
        // Doors are treated as open for now; windows stay blocking.
        f |= match k {
            "stairsBN" => flag::STAIR_BN, "stairsMN" => flag::STAIR_MN, "stairsTN" => flag::STAIR_TN,
            "stairsBW" => flag::STAIR_BW, "stairsMW" => flag::STAIR_MW, "stairsTW" => flag::STAIR_TW,
            _ => 0,
        };
        if k == "doorN" { f |= flag::DOOR_N; }
        if k == "doorW" { f |= flag::DOOR_W; }
        if matches!(k, "WallN" | "WallNW" | "WindowN" | "windowN" | "collideN" | "HoppableN") { f |= flag::EDGE_N; }
        if matches!(k, "WindowN" | "windowN") { f |= flag::WINDOW_N; }
        if matches!(k, "WindowW" | "windowW") { f |= flag::WINDOW_W; }
        if k == "HoppableN" { f |= flag::HOP_N; }
        if k == "HoppableW" { f |= flag::HOP_W; }
        if matches!(k, "WallW" | "WallNW" | "WindowW" | "windowW" | "collideW" | "HoppableW") { f |= flag::EDGE_W; }
    }
    f
}

/// `IsoDoor.health` for a plain wooden door.
pub const DOOR_HEALTH: i32 = 500;
/// `IsoWindow.health` for a plain window.
pub const WINDOW_HEALTH: i32 = 50;

/// `IsoBarricade`: up to four planks, or a metal-bar or metal-sheet barricade, on one door or window edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Barricade { pub planks: [i32; 4], pub metal_bar: i32, pub metal: i32 }

pub const PLANK_HEALTH: i32 = 1000;
pub const METAL_BAR_HEALTH: i32 = 3000;
pub const METAL_HEALTH: i32 = 5000;

impl Barricade {
    pub fn num_planks(&self) -> usize { self.planks.iter().filter(|p| **p > 0).count() }
    pub fn is_destroyed(&self) -> bool { self.metal <= 0 && self.metal_bar <= 0 && self.num_planks() == 0 }
    /// `IsoBarricade.Damage`: the metal sheet takes the hit first, then the bars, then the highest-numbered plank
    /// still standing (planks are nailed on from slot 0 up).
    pub fn damage(&mut self, amount: i32) {
        if self.metal > 0 { self.metal = (self.metal - amount).max(0); }
        else if self.metal_bar > 0 { self.metal_bar = (self.metal_bar - amount).max(0); }
        else if let Some(p) = self.planks.iter_mut().rev().find(|p| **p > 0) { *p = (*p - amount).max(0); }
    }
    /// `IsoBarricade.getHealth`: the metal's health, else the mean health of the planks left.
    pub fn health(&self) -> i32 {
        if self.metal > 0 { return self.metal; }
        if self.metal_bar > 0 { return self.metal_bar; }
        let n = self.num_planks() as i32;
        if n == 0 { 0 } else { self.planks.iter().sum::<i32>() / n }
    }
}

pub struct World {
    dir: PathBuf,
    /// Barricades by the edge they sit on: the square owning it and whether it is the north edge.
    barricades: HashMap<(i32, i32, usize, bool), Barricade>,
    cells: HashMap<(i32, i32), Option<Cell>>,
    /// tile name -> collision flags (only tiles with any flag).
    flags: HashMap<String, u16>,
    open_doors: HashSet<(i32, i32, usize)>,
    /// Hit points left on doors that have been attacked (a fresh wooden door has [`DOOR_HEALTH`]) and doors broken down.
    door_health: HashMap<(i32, i32, usize), i32>,
    broken_doors: HashSet<(i32, i32, usize)>,
    /// Glass left in windows that have been attacked, keyed by the square owning the edge and whether it is the north
    /// edge; windows missing from the map are intact. Broken windows are in `smashed_windows`.
    window_health: HashMap<(i32, i32, usize, bool), i32>,
    smashed_windows: HashSet<(i32, i32, usize, bool)>,
}

/// One tile column: ordered tile names, bottom first.
pub struct Column<'a> { pub room: i32, pub names: Vec<&'a str> }

fn load_cell(dir: &Path, cx: i32, cy: i32) -> Option<Cell> {
    let hdr = std::fs::read(dir.join(format!("{cx}_{cy}.lotheader"))).ok()?;
    let pack = std::fs::read(dir.join(format!("world_{cx}_{cy}.lotpack"))).ok()?;
    let hdr = LotHeader::parse(&hdr).ok()?;
    LotPack::parse(&pack).ok()?;
    Some(Cell { names: hdr.tile_names, pack, chunks: HashMap::new() })
}

impl World {
    /// `map_dir` is e.g. `<PZ>/media/maps/Muldraugh, KY`.
    pub fn new(map_dir: impl Into<PathBuf>) -> Self {
        Self { dir: map_dir.into(), cells: HashMap::new(), flags: HashMap::new(), open_doors: HashSet::new(), door_health: HashMap::new(), broken_doors: HashSet::new(), window_health: HashMap::new(), smashed_windows: HashSet::new(), barricades: HashMap::new() }
    }

    /// Load collision flags from the game's `*.tiles` definition files in `media/`.
    pub fn load_tile_flags(&mut self, media: &Path) {
        for f in ["newtiledefinitions.tiles", "tiledefinitions_overlays.tiles", "tiledefinitions_erosion.tiles",
                  "tiledefinitions_b42chunkcaching.tiles", "tiledefinitions_noiseworks.patch.tiles"] {
            let Ok(defs) = pz_formats::tiles::TileDefs::open(media.join(f)) else { continue };
            for set in defs.sets {
                for (i, t) in set.tiles.iter().enumerate() {
                    let fl = tile_flags(&t.props);
                    if fl != 0 { self.flags.insert(format!("{}_{}", set.name, i), fl); }
                }
            }
        }
    }

    /// Collision flags of square (x, y, z); 0 if empty or not loaded. Loads the chunk if needed.
    pub fn flags(&mut self, x: i32, y: i32, z: usize) -> u16 {
        if !self.ensure(x, y) { return 0; }
        let mut f = self.column(x, y, z).map_or(0, |c| c.names.iter().filter_map(|n| self.flags.get(*n)).fold(0, |a, b| a | b));
        if !self.open_doors.contains(&(x, y, z)) {
            if f & flag::DOOR_N != 0 { f |= flag::EDGE_N; }
            if f & flag::DOOR_W != 0 { f |= flag::EDGE_W; }
        }
        f
    }

    /// The closed door standing on the edge between two orthogonally adjacent squares, if any: `DOOR_N` on a square
    /// blocks its north edge, `DOOR_W` its west edge.
    pub fn closed_door_between(&mut self, a: (i32, i32), b: (i32, i32), z: usize) -> Option<(i32, i32, usize)> {
        let (key, bit) = match (b.0 - a.0, b.1 - a.1) {
            (0, -1) => ((a.0, a.1, z), flag::DOOR_N),
            (0, 1) => ((b.0, b.1, z), flag::DOOR_N),
            (-1, 0) => ((a.0, a.1, z), flag::DOOR_W),
            (1, 0) => ((b.0, b.1, z), flag::DOOR_W),
            _ => return None,
        };
        let has = self.flags(key.0, key.1, z) & bit != 0;
        (has && !self.open_doors.contains(&key)).then_some(key)
    }

    /// Hit a door for `damage` points (`IsoDoor.Damage`). At zero it is broken down for good: open and unclosable.
    /// Returns the hit points left, or `None` if there is no door there.
    pub fn damage_door(&mut self, x: i32, y: i32, z: usize, damage: i32) -> Option<i32> {
        if self.flags(x, y, z) & (flag::DOOR_N | flag::DOOR_W) == 0 || self.broken_doors.contains(&(x, y, z)) { return None; }
        let key = (x, y, z);
        let north = self.flags(x, y, z) & flag::DOOR_N != 0;
        if self.hit_barricade((x, y, z, north), damage).is_some() { return Some(self.door_health.get(&key).copied().unwrap_or(DOOR_HEALTH)); }
        let hp = self.door_health.entry(key).or_insert(DOOR_HEALTH);
        *hp = (*hp - damage).max(0);
        let left = *hp;
        if left == 0 { self.open_doors.insert(key); self.broken_doors.insert(key); }
        Some(left)
    }

    /// The intact window on the edge between two orthogonally adjacent squares, if any. Windows start closed and
    /// whole; zombies have to smash them before they can climb through.
    pub fn intact_window_between(&mut self, a: (i32, i32), b: (i32, i32), z: usize) -> Option<(i32, i32, usize, bool)> {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        if dx.abs() + dy.abs() != 1 { return None; }
        let (sq, north) = if dx > 0 || dy > 0 { (b, dy > 0) } else { (a, dy < 0) };
        let bit = if north { flag::WINDOW_N } else { flag::WINDOW_W };
        let key = (sq.0, sq.1, z, north);
        (self.flags(sq.0, sq.1, z) & bit != 0 && (!self.smashed_windows.contains(&key) || self.barricades.contains_key(&key))).then_some(key)
    }

    /// Hit a window for `damage` (`IsoWindow.damage`): at zero the glass is smashed for good. Returns the glass left.
    pub fn damage_window(&mut self, key: (i32, i32, usize, bool), damage: i32) -> i32 {
        if self.hit_barricade(key, damage).is_some() { return self.window_health.get(&key).copied().unwrap_or(WINDOW_HEALTH).max(1); }
        let hp = self.window_health.entry(key).or_insert(WINDOW_HEALTH);
        *hp = (*hp - damage).max(0);
        if *hp == 0 { self.smashed_windows.insert(key); }
        *hp
    }

    /// Whether the way through the window is open: glass gone and no barricade over it.
    pub fn window_smashed(&self, key: (i32, i32, usize, bool)) -> bool { self.smashed_windows.contains(&key) && !self.barricades.contains_key(&key) }

    /// Nail a plank of `health` onto the edge (`IsoBarricade.addPlank`); false if four are already there or it is metal.
    pub fn add_plank(&mut self, edge: (i32, i32, usize, bool), health: i32) -> bool {
        let b = self.barricades.entry(edge).or_default();
        if b.metal > 0 || b.metal_bar > 0 { return false; }
        match b.planks.iter_mut().find(|p| **p <= 0) { Some(p) => { *p = health.clamp(1, PLANK_HEALTH); true } None => false }
    }

    /// Bolt on a metal-bar or metal-sheet barricade; false if the edge already carries planks or metal.
    pub fn add_metal(&mut self, edge: (i32, i32, usize, bool), bars: bool) -> bool {
        let b = self.barricades.entry(edge).or_default();
        if !b.is_destroyed() { return false; }
        if bars { b.metal_bar = METAL_BAR_HEALTH } else { b.metal = METAL_HEALTH }
        true
    }

    pub fn barricade(&self, edge: (i32, i32, usize, bool)) -> Option<Barricade> { self.barricades.get(&edge).copied() }

    /// Hit the barricade on this edge; returns its health after the hit, or `None` if there is none. A destroyed
    /// barricade is removed (`IsoBarricade.Thump`).
    fn hit_barricade(&mut self, edge: (i32, i32, usize, bool), damage: i32) -> Option<i32> {
        let b = self.barricades.get_mut(&edge)?;
        b.damage(damage);
        let left = b.health();
        if b.is_destroyed() { self.barricades.remove(&edge); }
        Some(left)
    }

    pub fn door_broken(&self, x: i32, y: i32, z: usize) -> bool { self.broken_doors.contains(&(x, y, z)) }

    /// Open/close the door on square (x, y, z). Returns the new state, or `None` if there is no door.
    pub fn toggle_door(&mut self, x: i32, y: i32, z: usize) -> Option<bool> {
        if self.flags(x, y, z) & (flag::DOOR_N | flag::DOOR_W) == 0 || self.broken_doors.contains(&(x, y, z)) { return None; }
        let k = (x, y, z);
        let north = self.flags(x, y, z) & flag::DOOR_N != 0;
        if self.barricades.contains_key(&(x, y, z, north)) { return None; }
        Some(if self.open_doors.remove(&k) { false } else { self.open_doors.insert(k); true })
    }

    /// Open doors as `(x, y, z)` in a stable order (for network snapshots).
    pub fn open_doors(&self) -> Vec<(i32, i32, usize)> {
        let mut v: Vec<_> = self.open_doors.iter().copied().collect();
        v.sort_unstable();
        v
    }

    /// Replace the set of open doors (client side, from a server snapshot).
    pub fn set_open_doors(&mut self, doors: impl IntoIterator<Item = (i32, i32, usize)>) {
        self.open_doors = doors.into_iter().collect();
    }

    pub fn door_open(&self, x: i32, y: i32, z: usize) -> bool { self.open_doors.contains(&(x, y, z)) }

    /// Is this tile name a door panel (hidden while its door is open)?
    pub fn is_door_tile(&self, name: &str) -> bool {
        self.flags.get(name).is_some_and(|f| f & (flag::DOOR_N | flag::DOOR_W) != 0)
    }

    /// True if there is a floor (or roof deck) tile to stand on at level `z`.
    /// Squares above staircases only carry railings, so they count as holes.
    pub fn has_floor(&mut self, x: i32, y: i32, z: usize) -> bool {
        self.ensure(x, y)
            && self.column(x, y, z).is_some_and(|c| c.names.iter().any(|n| n.starts_with("floors_") || n.starts_with("roofs_")))
    }

    /// A* over orthogonal steps allowed by `can_step`; returns the squares to walk through
    /// (excluding `from`, including `to`). Gives up after `max_nodes` expansions.
    pub fn find_path(&mut self, from: (i32, i32), to: (i32, i32), z: usize, max_nodes: usize) -> Option<Vec<(i32, i32)>> {
        use std::cmp::Reverse;
        use std::collections::BinaryHeap;
        if from == to { return Some(vec![]); }
        let h = |p: (i32, i32)| ((p.0 - to.0).abs() + (p.1 - to.1).abs()) as u32;
        let mut open = BinaryHeap::new();
        let mut g: HashMap<(i32, i32), u32> = HashMap::new();
        let mut parent: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
        g.insert(from, 0);
        open.push(Reverse((h(from), 0u32, from)));
        let mut expanded = 0;
        while let Some(Reverse((_, cost, cur))) = open.pop() {
            if cur == to {
                let mut path = vec![cur];
                let mut c = cur;
                while let Some(&p) = parent.get(&c) { if p == from { break; } path.push(p); c = p; }
                path.reverse();
                return Some(path);
            }
            if cost > *g.get(&cur).unwrap_or(&u32::MAX) { continue; }
            expanded += 1;
            if expanded > max_nodes { return None; }
            for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let n = (cur.0 + dx, cur.1 + dy);
                if !self.has_floor(n.0, n.1, z) || !self.can_step(cur, n, z) { continue; }
                let ng = cost + 1;
                if ng < *g.get(&n).unwrap_or(&u32::MAX) {
                    g.insert(n, ng);
                    parent.insert(n, cur);
                    open.push(Reverse((ng + h(n), ng, n)));
                }
            }
        }
        None
    }

    /// Can a walker step from square `from` into the orthogonally adjacent square `to`?
    pub fn can_step(&mut self, from: (i32, i32), to: (i32, i32), z: usize) -> bool {
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        debug_assert!(dx.abs() + dy.abs() == 1);
        let t = self.flags(to.0, to.1, z);
        if t & flag::SOLID != 0 { return false; }
        // Entering from the west/north crosses the target's W/N edge; leaving that way crosses the source's.
        match (dx, dy) {
            (1, 0) => t & flag::EDGE_W == 0,
            (0, 1) => t & flag::EDGE_N == 0,
            (-1, 0) => self.flags(from.0, from.1, z) & flag::EDGE_W == 0,
            _ => self.flags(from.0, from.1, z) & flag::EDGE_N == 0,
        }
    }

    /// True when the step `from -> to` is blocked only by a window or hoppable fence on the crossed edge, so a
    /// zombie (or player) can climb it. Windows are treated as always climbable: their open/broken state is not modelled.
    pub fn hurdle(&mut self, from: (i32, i32), to: (i32, i32), z: usize) -> bool {
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        if dx.abs() + dy.abs() != 1 { return false; }
        let t = self.flags(to.0, to.1, z);
        if t & flag::SOLID != 0 || !self.has_floor(to.0, to.1, z) { return false; }
        // The crossed edge belongs to the target square when moving east/south, to the source when moving west/north.
        let (edge_sq, n, w) = if dx > 0 || dy > 0 { (t, dy > 0, dx > 0) } else { (self.flags(from.0, from.1, z), dy < 0, dx < 0) };
        if n { edge_sq & (flag::WINDOW_N | flag::HOP_N) != 0 } else if w { edge_sq & (flag::WINDOW_W | flag::HOP_W) != 0 } else { false }
    }

    /// Make sure the chunk holding tile (x, y) is decoded. Returns false if no map data there.
    pub fn ensure(&mut self, x: i32, y: i32) -> bool {
        let (cx, cy) = (x.div_euclid(CELL), y.div_euclid(CELL));
        let dir = &self.dir;
        self.cells.entry((cx, cy)).or_insert_with(|| load_cell(dir, cx, cy));
        let Some(cell) = self.cells.get_mut(&(cx, cy)).and_then(|c| c.as_mut()) else { return false };
        let (lx, ly) = (x.rem_euclid(CELL) as usize / CHUNK, y.rem_euclid(CELL) as usize / CHUNK);
        let idx = lx * CHUNKS_PER_CELL + ly;
        if !cell.chunks.contains_key(&idx) {
            let pack = LotPack::parse(&cell.pack).unwrap();
            let levels = decode_chunk(pack.chunk_bytes(idx)).unwrap_or_default();
            cell.chunks.insert(idx, levels);
        }
        true
    }

    /// Column at (x, y, z). Call `ensure` first; returns `None` for empty/unloaded squares.
    pub fn column(&self, x: i32, y: i32, z: usize) -> Option<Column<'_>> {
        let (cx, cy) = (x.div_euclid(CELL), y.div_euclid(CELL));
        let cell = self.cells.get(&(cx, cy))?.as_ref()?;
        let (lx, ly) = (x.rem_euclid(CELL) as usize, y.rem_euclid(CELL) as usize);
        let idx = (lx / CHUNK) * CHUNKS_PER_CELL + ly / CHUNK;
        let sq = cell.chunks.get(&idx)?.get(z)?.get((lx % CHUNK) * CHUNK + ly % CHUNK)?;
        if sq.tiles.is_empty() { return None; }
        Some(Column { room: sq.room, names: sq.tiles.iter().map(|&t| cell.names[t as usize].as_str()).collect() })
    }
}
