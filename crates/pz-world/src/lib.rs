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
        if matches!(k, "WallN" | "WallNW" | "WindowN" | "windowN" | "collideN") { f |= flag::EDGE_N; }
        if matches!(k, "WallW" | "WallNW" | "WindowW" | "windowW" | "collideW") { f |= flag::EDGE_W; }
    }
    f
}

pub struct World {
    dir: PathBuf,
    cells: HashMap<(i32, i32), Option<Cell>>,
    /// tile name -> collision flags (only tiles with any flag).
    flags: HashMap<String, u16>,
    open_doors: HashSet<(i32, i32, usize)>,
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
        Self { dir: map_dir.into(), cells: HashMap::new(), flags: HashMap::new(), open_doors: HashSet::new() }
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

    /// Open/close the door on square (x, y, z). Returns the new state, or `None` if there is no door.
    pub fn toggle_door(&mut self, x: i32, y: i32, z: usize) -> Option<bool> {
        if self.flags(x, y, z) & (flag::DOOR_N | flag::DOOR_W) == 0 { return None; }
        let k = (x, y, z);
        Some(if self.open_doors.remove(&k) { false } else { self.open_doors.insert(k); true })
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
