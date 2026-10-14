//! Walking over the terrain, from the game client's map tiles (`client_terrain`, computed when
//! the databases are built): cells of 8.33 yards, 64 x 64 a tile, each with its height, its
//! water and whether it is too steep to climb (mountains, mesas, cliffs).
//!
//! A step from a cell to its neighbour costs its length, more swimming. It cannot be taken
//! into magma or the open sea (fatigue), up a slope over 50° or onto a steep cell, nor down a
//! drop that hurts: a cliff is jumped down, not climbed. With `add_danger`, the cells near
//! hostile mobs stronger than the character cost more (not near the walk's ends: the quest's
//! mobs are there).
//!
//! A straight line of plain steps (dry, safe) is the walk; otherwise the cheapest path (A* on
//! the cells, cached). Points it does not connect keep the straight line: tunnels, bridges and
//! city gates are buildings, not terrain. Except the large areas declared enclosed
//! (`[[enclosed]]` of `overrides/passes.toml`: an island, the top of a cliff), which only a boat,
//! a portal or a lift (`overrides/travel.toml`) leaves.
//!
//! The paths searched are slow to find (the first route of a long guide asks for thousands):
//! they are kept in a walking memory (`Walks`) that the app shares between generations, and
//! saved with each guide (`WalkMemory`).

mod gates;

pub use gates::GATES_VERSION;

use crate::world::Pos;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use rusqlite::Connection;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::hash::Hasher;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

const TILE: f64 = 1600.0 / 3.0;
const CELL: f64 = TILE / 64.0;
/// World coordinate of the north-west corner of tile (0, 0).
const ORIGIN: f64 = 32.0 * TILE;
/// Endpoints on a steep cell or a small walkable patch (an NPC on a slope, a ledge) move to the
/// nearest cell of a large walkable area this close.
const SNAP: i32 = 12;
pub const LARGE_AREA: u32 = 2000;
/// The path is searched in the box around both ends, this much larger (cells), then larger.
const MARGINS: [i32; 2] = [60, 240];
const SHARDS: usize = 32;
/// Ground height of a cell without terrain.
const NO_HEIGHT: i16 = i16::MIN;
/// Bits of a cell's water byte; the depth (yards) in bits 3 to 7.
const WET: u8 = 1;
const HAZARD: u8 = 2;
const DEEP: u8 = 4;
/// Water at least this deep (yards) is swum.
const SWIM_DEPTH: u8 = 2;
/// Highest rise of a straight step (yards): a 50° slope over a cell; whole yards, as heights
/// are, straight and diagonal.
const CLIMB: f64 = CELL * 1.191_753_6;
const CLIMB_STRAIGHT: i32 = CLIMB as i32;
const CLIMB_DIAGONAL: i32 = (CLIMB * std::f64::consts::SQRT_2) as i32;
/// Highest drop a step jumps down (yards): about where a fall starts to hurt.
const DROP: i32 = 15;
/// Cost of a step in tenths of a cell: straight and diagonal, walked then swum (a third
/// slower than running).
const WALKED: [u32; 2] = [10, 14];
const SWUM: [u32; 2] = [15, 21];
/// No danger counted this close to the ends of a walk (cells): the mobs of the quest are there.
const DANGER_FREE: i32 = 6;
/// Most a cell costs more for its danger (tenths of a cell).
const DANGER_MAX: u32 = 400;
/// Buckets of the path search's open cells: more than the largest rise of an estimate (the
/// dearest step and its danger, plus 14).
const BUCKETS: usize = 512;
/// The neighbours of a cell: straight, then diagonal.
const DIRECTIONS: [(i32, i32); 8] = [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (-1, 1), (1, -1), (1, 1)];

/// Continent and pair of cells of a walk, and the character's level when danger counts there
/// (0: it does not).
type Key = (i64, i32, i32, i32, i32, u8);

/// Hash of the terrain data (its fingerprint, saved with the walking memories: must not change).
#[derive(Default)]
struct Packed(u64);

impl Hasher for Packed {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    }
    fn write_u64(&mut self, n: u64) {
        self.0 = (self.0 ^ n).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
    fn write_u32(&mut self, n: u32) {
        self.write_u64(u64::from(n));
    }
}

/// Answers already given in this thread, by terrain, continent, pair of cells and level: the
/// planner asks the same walks millions of times. Negative: the straight line; infinite: not
/// connected; NaN: not known.
type Memo = FxHashMap<(u32, i64, u64, u8), f32>;
thread_local! {
    static MEMO: RefCell<Memo> = RefCell::new(Memo::default());
}
static NEXT_ID: AtomicU32 = AtomicU32::new(1);

/// The path search's work space, kept from a search to the next in each thread: the cheapest
/// cost found for each cell of the box (valid when its `round` is this search's: nothing to
/// clear) and the open cells by estimate.
struct Scratch {
    best: Vec<u32>,
    round: Vec<u32>,
    current: u32,
    open: Vec<Vec<(u32, i32, i32)>>,
}

thread_local! {
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch {
        best: Vec::new(),
        round: Vec::new(),
        current: 0,
        open: (0..BUCKETS).map(|_| Vec::new()).collect(),
    });
}

/// One continent's cells, over the box of its tiles.
struct Grid {
    /// Tells this grid's answers from another's in the thread memos (a load, a continent).
    id: u32,
    r0: i32,
    c0: i32,
    rows: i32,
    cols: i32,
    /// Steep cells (and cells without terrain), a bit per cell, row by row.
    steep: Vec<u64>,
    words: usize,
    /// Heights and water of each tile's cells (None: no terrain), by tile of 64 x 64 cells
    /// (row by row, `tile_cols` a row).
    cells: Vec<Option<Box<TileCells>>>,
    /// Connected walkable area of each cell (0: steep), by tile, and the size of each area.
    areas: Vec<TileAreas>,
    tile_cols: usize,
    sizes: Vec<u32>,
    /// The strongest hostile mob around each cell (its level, more for an elite; 0: none), by
    /// tile (None: none in the tile), and the strongest of each tile.
    danger: Vec<Option<Box<[u8; 4096]>>>,
    danger_max: Vec<u8>,
    /// The gates of each tile and the walks between them, found when first needed (`gates`).
    graphs: Vec<std::sync::OnceLock<gates::TileGraph>>,
    /// The walkable areas within each tile, found when first needed (`gates`).
    local_areas: Vec<std::sync::OnceLock<Box<[u16]>>>,
}

/// What a cell is for walking (`Terrain::view`), to look at the terrain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellView {
    /// Ground height (yards).
    pub height: i16,
    /// Too steep to climb onto.
    pub steep: bool,
    /// Water depth over the ground (yards, 0: dry); swum from `SWIM_DEPTH`.
    pub depth: u8,
    pub swum: bool,
    /// Open sea (fatigue) or magma, slime: never stood in. `sea_edge`: the edge of the open
    /// sea as the client marks it (the rest of the open sea is found from it).
    pub deep: bool,
    pub sea_edge: bool,
    pub hazard: bool,
    /// The strongest hostile mob around (its level, more for an elite; 0: none).
    pub danger: u8,
}

/// A tile's cells, row by row: ground height (yards) and water byte as read, and what a step
/// needs, computed once: the surface stood or swum on and `STEEP`, `NO_GO`, `SWIM`.
struct TileCells {
    heights: [i16; 4096],
    water: [u8; 4096],
    surface: [i16; 4096],
    flags: [u8; 4096],
}

/// Flags of a cell for a step.
const STEEP: u8 = 1;
/// Never stood on: no terrain, magma, the open sea.
const NO_GO: u8 = 2;
const SWIM: u8 = 4;

/// Height of the surface stood or swum on: the ground, or the water over it.
fn surface(height: i16, water: u8) -> i32 {
    i32::from(height) + if water & WET != 0 { i32::from(water >> 3) } else { 0 }
}

/// The areas of a tile's cells: one for the whole tile (no terrain, or all walkable), else each
/// cell's. A continent's box holds tens of millions of cells, most of them in such tiles.
enum TileAreas {
    Uniform(u32),
    Cells(Box<[u32]>),
}

impl Grid {
    fn steep(&self, r: i32, c: i32) -> bool {
        let (r, c) = (r - self.r0, c - self.c0);
        if r < 0 || c < 0 || r >= self.rows || c >= self.cols {
            return true;
        }
        let i = r as usize * self.words + c as usize / 64;
        self.steep[i] >> (c % 64) & 1 != 0
    }

    /// Index of a cell's tile and its index in it, None out of the box.
    fn tile(&self, r: i32, c: i32) -> Option<(usize, usize)> {
        let (r, c) = (r - self.r0, c - self.c0);
        if r < 0 || c < 0 || r >= self.rows || c >= self.cols {
            return None;
        }
        let (r, c) = (r as usize, c as usize);
        Some((r / 64 * self.tile_cols + c / 64, r % 64 * 64 + c % 64))
    }

    /// Surface height and flags of a cell (`NO_GO` out of the terrain).
    #[inline]
    fn probe(&self, r: i32, c: i32) -> (i16, u8) {
        let (r, c) = (r - self.r0, c - self.c0);
        if r < 0 || c < 0 || r >= self.rows || c >= self.cols {
            return (0, NO_GO);
        }
        let (r, c) = (r as usize, c as usize);
        match &self.cells[(r >> 6) * self.tile_cols + (c >> 6)] {
            Some(cells) => {
                let i = (r & 63) << 6 | (c & 63);
                (cells.surface[i], cells.flags[i])
            }
            None => (0, NO_GO),
        }
    }

    /// Ground height and water of a cell, None without terrain.
    fn cell(&self, r: i32, c: i32) -> Option<(i16, u8)> {
        let (t, i) = self.tile(r, c)?;
        let cells = self.cells[t].as_ref()?;
        let h = cells.heights[i];
        (h != NO_HEIGHT).then_some((h, cells.water[i]))
    }

    /// Cells never stood on: steep, without terrain, magma or the open sea.
    fn blocked(&self, r: i32, c: i32) -> bool {
        self.probe(r, c).1 & (STEEP | NO_GO) != 0
    }

    /// Cost (tenths of a cell) of the step from cell `a` to its neighbour `b`, None when it
    /// cannot be taken: no terrain, magma or the open sea, a rise steeper than 50° or onto a
    /// steep cell, a drop that hurts.
    fn step(&self, a: (i32, i32), b: (i32, i32)) -> Option<u32> {
        let (from, flags) = self.probe(a.0, a.1);
        if flags & NO_GO != 0 {
            return None;
        }
        self.step_from(from, b, a.0 != b.0 && a.1 != b.1)
    }

    /// The same from a cell of surface height `from`.
    #[inline]
    fn step_from(&self, from: i16, b: (i32, i32), diagonal: bool) -> Option<u32> {
        let (to, flags) = self.probe(b.0, b.1);
        if flags & NO_GO != 0 {
            return None;
        }
        let rise = i32::from(to) - i32::from(from);
        let steep = flags & STEEP != 0;
        if rise > 0 && (steep || rise > if diagonal { CLIMB_DIAGONAL } else { CLIMB_STRAIGHT }) {
            return None;
        }
        if rise < -DROP || (rise == 0 && steep) {
            return None;
        }
        let d = usize::from(diagonal);
        Some(if flags & SWIM != 0 { SWUM[d] } else { WALKED[d] })
    }

    /// Whether a straight step between two cells that can be stood on is taken both ways (a
    /// rise within a 50° slope; the drop back is never too high).
    fn joined(&self, a: (i32, i32), b: (i32, i32)) -> bool {
        let ((sa, fa), (sb, fb)) = (self.probe(a.0, a.1), self.probe(b.0, b.1));
        (fa | fb) & (STEEP | NO_GO) == 0 && (i32::from(sb) - i32::from(sa)).abs() <= CLIMB_STRAIGHT
    }

    /// The strongest hostile mob around a cell (0: none).
    fn danger(&self, r: i32, c: i32) -> u8 {
        self.tile(r, c)
            .and_then(|(t, i)| Some(self.danger.get(t)?.as_ref()?[i]))
            .unwrap_or(0)
    }

    /// Whether a mob of `level - 2` or more is in the tiles around the box of `a` and `b` grown
    /// by `margin` cells: danger may count on a walk between them.
    fn danger_near(&self, a: (i32, i32), b: (i32, i32), margin: i32, level: u8) -> bool {
        if self.danger_max.is_empty() {
            return false;
        }
        let at_least = level.saturating_sub(2).max(1);
        let tile_of = |v: i32, origin: i32| (v - origin).div_euclid(64);
        let rows = self.rows / 64;
        let cols = self.tile_cols as i32;
        let (t0, t1) = (
            tile_of(a.0.min(b.0) - margin, self.r0).max(0),
            tile_of(a.0.max(b.0) + margin, self.r0).min(rows - 1),
        );
        let (u0, u1) = (
            tile_of(a.1.min(b.1) - margin, self.c0).max(0),
            tile_of(a.1.max(b.1) + margin, self.c0).min(cols - 1),
        );
        (t0..=t1).any(|t| (u0..=u1).any(|u| self.danger_max[(t * cols + u) as usize] >= at_least))
    }

    fn area(&self, r: i32, c: i32) -> u32 {
        let (r, c) = (r - self.r0, c - self.c0);
        if r < 0 || c < 0 || r >= self.rows || c >= self.cols {
            return 0;
        }
        let (r, c) = (r as usize, c as usize);
        match &self.areas[r / 64 * self.tile_cols + c / 64] {
            TileAreas::Uniform(a) => *a,
            TileAreas::Cells(cells) => cells[r % 64 * 64 + c % 64],
        }
    }

    /// Label the walkable areas: cells joined by straight steps taken both ways (as paths do
    /// not cut corners), a drop down a cliff does not join its top to its foot.
    fn label(&mut self) {
        let mut area = vec![0; (self.rows * self.cols) as usize];
        self.sizes = vec![0];
        let mut stack = Vec::new();
        // Tile by tile: those without terrain have nothing to label.
        let starts = (0..self.cells.len())
            .filter(|&t| self.cells[t].is_some())
            .flat_map(|t| (0..4096).map(move |i| (t, i)))
            .map(|(t, i)| {
                let (tr, tc) = (t / self.tile_cols, t % self.tile_cols);
                ((tr * 64 + i / 64) as i32, (tc * 64 + i % 64) as i32)
            })
            .collect::<Vec<_>>();
        for (r, c) in starts {
            let start = (r * self.cols + c) as usize;
            if area[start] != 0 || self.blocked(r + self.r0, c + self.c0) {
                continue;
            }
            let id = self.sizes.len() as u32;
            let mut size = 0;
            area[start] = id;
            stack.push((r, c));
            while let Some((r, c)) = stack.pop() {
                size += 1;
                for (nr, nc) in [(r - 1, c), (r + 1, c), (r, c - 1), (r, c + 1)] {
                    if nr < 0 || nc < 0 || nr >= self.rows || nc >= self.cols {
                        continue;
                    }
                    let k = (nr * self.cols + nc) as usize;
                    if area[k] == 0 && self.joined((r + self.r0, c + self.c0), (nr + self.r0, nc + self.c0)) {
                        area[k] = id;
                        stack.push((nr, nc));
                    }
                }
            }
            self.sizes.push(size);
        }
        // Rows and columns are whole tiles.
        let cols = self.cols as usize;
        self.areas = (0..self.rows as usize / 64 * self.tile_cols)
            .map(|t| {
                let (r0, c0) = (t / self.tile_cols * 64, t % self.tile_cols * 64);
                let cells: Vec<u32> = (0..64)
                    .flat_map(|r| area[(r0 + r) * cols + c0..][..64].iter().copied())
                    .collect();
                if cells.iter().all(|&a| a == cells[0]) {
                    TileAreas::Uniform(cells[0])
                } else {
                    TileAreas::Cells(cells.into())
                }
            })
            .collect();
    }
}

/// Shard of a pair of cells in the sharded maps.
fn shard((_, r1, c1, r2, c2, _): Key) -> usize {
    (r1 ^ r2 ^ c1.wrapping_mul(31) ^ c2.wrapping_mul(17)).unsigned_abs() as usize % SHARDS
}

/// Walking memory: the paths around the terrain already searched (walking distance between two
/// cells, infinite when not connected), shared by every route built on the same terrain data.
/// The app keeps one between generations, filled with the memories saved with the guides.
pub struct Walks {
    /// Fingerprint of the terrain data the paths were searched on (None: not known yet).
    terrain: Mutex<Option<u64>>,
    shards: Vec<Mutex<HashMap<Key, f32>>>,
    /// Memories added before the terrain was known, merged once it is.
    waiting: Mutex<Vec<WalkMemory>>,
}

impl Default for Walks {
    fn default() -> Self {
        Self {
            terrain: Mutex::new(None),
            shards: (0..SHARDS).map(|_| Mutex::new(HashMap::new())).collect(),
            waiting: Mutex::new(Vec::new()),
        }
    }
}

impl Walks {
    /// Remember the paths of a saved memory (dropped when searched on other terrain data).
    pub fn add(&self, memory: WalkMemory) {
        let terrain = *self.terrain.lock().unwrap();
        match terrain {
            Some(t) if t == memory.terrain => self.insert_all(&memory),
            Some(_) => {}
            None => self.waiting.lock().unwrap().push(memory),
        }
    }

    /// Paths remembered.
    pub fn len(&self) -> usize {
        self.shards.iter().map(|s| s.lock().unwrap().len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Ties the memory to the terrain data of fingerprint `terrain`: the paths of other data are
    /// forgotten, the memories waiting for this one merged.
    fn bind(&self, terrain: u64) {
        let mut bound = self.terrain.lock().unwrap();
        if *bound == Some(terrain) {
            return;
        }
        if bound.is_some() {
            for s in &self.shards {
                s.lock().unwrap().clear();
            }
        }
        *bound = Some(terrain);
        for memory in std::mem::take(&mut *self.waiting.lock().unwrap()) {
            if memory.terrain == terrain {
                self.insert_all(&memory);
            }
        }
    }

    fn insert_all(&self, memory: &WalkMemory) {
        for &(key, d) in &memory.paths {
            self.shards[shard(key)].lock().unwrap().insert(key, d);
        }
    }

    fn get(&self, key: Key) -> Option<f32> {
        self.shards[shard(key)].lock().unwrap().get(&key).copied()
    }

    fn insert(&self, key: Key, d: f32) {
        self.shards[shard(key)].lock().unwrap().insert(key, d);
    }
}

/// The paths a guide needed, saved with it (and in the files it is shared as) so that computing
/// it again, or other guides, do not search them again. In JSON: a string (sorted, zstd, base64).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WalkMemory {
    /// Fingerprint of the terrain data the paths were searched on.
    terrain: u64,
    paths: Vec<(Key, f32)>,
}

impl WalkMemory {
    pub fn len(&self) -> usize {
        self.paths.len()
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    /// Format 2: terrain fingerprint, count, then each path (continent, cells, level, distance),
    /// little endian, compressed. Format 1 (no level) was for terrain without heights: dropped.
    fn encode(&self) -> String {
        let mut bytes = Vec::with_capacity(12 + self.paths.len() * 25);
        bytes.extend(self.terrain.to_le_bytes());
        bytes.extend((self.paths.len() as u32).to_le_bytes());
        for &((map, r1, c1, r2, c2, level), d) in &self.paths {
            bytes.extend((map as i32).to_le_bytes());
            for v in [r1, c1, r2, c2] {
                bytes.extend(v.to_le_bytes());
            }
            bytes.push(level);
            bytes.extend(d.to_le_bytes());
        }
        let packed = zstd::encode_all(bytes.as_slice(), 9).unwrap_or_default();
        format!("2:{}", BASE64.encode(packed))
    }

    fn decode(text: &str) -> Option<Self> {
        let bytes = zstd::decode_all(BASE64.decode(text.strip_prefix("2:")?).ok()?.as_slice()).ok()?;
        let terrain = u64::from_le_bytes(bytes.get(..8)?.try_into().ok()?);
        let count = u32::from_le_bytes(bytes.get(8..12)?.try_into().ok()?) as usize;
        let int = |at: usize| Some(i32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
        let paths = (0..count)
            .map(|i| {
                let at = 12 + i * 25;
                let key = (
                    i64::from(int(at)?),
                    int(at + 4)?,
                    int(at + 8)?,
                    int(at + 12)?,
                    int(at + 16)?,
                    *bytes.get(at + 20)?,
                );
                Some((key, f32::from_le_bytes(bytes.get(at + 21..at + 25)?.try_into().ok()?)))
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Self { terrain, paths })
    }
}

impl Serialize for WalkMemory {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.encode())
    }
}

/// A memory that cannot be read (another format) is empty: it only saves time.
impl<'de> Deserialize<'de> for WalkMemory {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self::decode(&String::deserialize(deserializer)?).unwrap_or_default())
    }
}

#[derive(Default)]
pub struct Terrain {
    /// Tells this terrain's answers from another's in the thread caches.
    id: u32,
    /// Hash of the terrain data: tells which data a saved memory belongs to.
    fingerprint: u64,
    grids: FxHashMap<i64, Grid>,
    walks: Arc<Walks>,
    /// Paths this terrain asked its memory (or searched), for the memory of its guide.
    used: Vec<Mutex<HashSet<Key>>>,
    /// Large areas left only by boat, portal or lift (continent, area): `Terrain::enclose`.
    enclosed: HashSet<(i64, u32)>,
    /// How much the danger counts (`add_danger`; 0: not at all).
    danger_weight: f64,
}

/// A tile as read: its x and y, steep cells (rows of bits) and cells.
type TileRow = (i32, i32, [u64; 64], Box<TileCells>);

/// A tile's cells from its `heights` and `water` blobs (zstd; the heights as differences with
/// the cell before): flat and dry when they cannot be read.
fn tile_cells(heights: &[u8], water: &[u8]) -> Box<TileCells> {
    let mut cells = Box::new(TileCells {
        heights: [0; 4096],
        water: [0; 4096],
        surface: [0; 4096],
        flags: [0; 4096],
    });
    if let Ok(deltas) = zstd::decode_all(heights)
        && deltas.len() == 2 * 4096
    {
        let mut height = 0i16;
        for (h, d) in cells.heights.iter_mut().zip(deltas.as_chunks::<2>().0) {
            height = height.wrapping_add(i16::from_le_bytes(*d));
            *h = height;
        }
    }
    if let Ok(water) = zstd::decode_all(water)
        && water.len() == 4096
    {
        cells.water.copy_from_slice(&water);
    }
    cells
}

impl Terrain {
    pub fn load(conn: &Connection) -> rusqlite::Result<Self> {
        let mut terrain = Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            fingerprint: 0,
            grids: FxHashMap::default(),
            walks: Arc::new(Walks::default()),
            used: (0..SHARDS).map(|_| Mutex::new(HashSet::new())).collect(),
            enclosed: HashSet::new(),
            danger_weight: 0.0,
        };
        let exists: bool = conn.query_row(
            "SELECT count(*) > 0 FROM sqlite_schema WHERE name = 'client_terrain'",
            [],
            |r| r.get(0),
        )?;
        if !exists {
            terrain.walks.bind(terrain.fingerprint);
            return Ok(terrain);
        }
        // Databases built before the heights: flat and dry.
        let heights: bool = conn.query_row(
            "SELECT count(*) > 0 FROM pragma_table_info('client_terrain') WHERE name = 'heights'",
            [],
            |r| r.get(0),
        )?;
        let query = if heights {
            "SELECT map_id, tile_x, tile_y, blocked, heights, water FROM client_terrain"
        } else {
            "SELECT map_id, tile_x, tile_y, blocked, x'', x'' FROM client_terrain"
        };
        let mut tiles: HashMap<i64, Vec<TileRow>> = HashMap::new();
        let mut stmt = conn.prepare(query)?;
        for row in stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i32>(1)?,
                r.get::<_, i32>(2)?,
                r.get::<_, Vec<u8>>(3)?,
                r.get::<_, Vec<u8>>(4)?,
                r.get::<_, Vec<u8>>(5)?,
            ))
        })? {
            let (map, x, y, blob, heights, water) = row?;
            // Whatever order the rows come in: a sum of the tiles' hashes.
            let mut h = Packed::default();
            h.write_u64(map as u64);
            h.write_u32(x as u32);
            h.write_u32(y as u32);
            h.write(&blob);
            h.write(&heights);
            h.write(&water);
            terrain.fingerprint = terrain.fingerprint.wrapping_add(h.finish());
            if blob.len() == 512 {
                let mut rows = [0u64; 64];
                for (r, b) in rows.iter_mut().zip(blob.as_chunks::<8>().0) {
                    *r = u64::from_le_bytes(*b);
                }
                tiles
                    .entry(map)
                    .or_default()
                    .push((x, y, rows, tile_cells(&heights, &water)));
            }
        }
        for (map, list) in tiles {
            let tx0 = list.iter().map(|t| t.0).min().unwrap_or(0);
            let tx1 = list.iter().map(|t| t.0).max().unwrap_or(0);
            let ty0 = list.iter().map(|t| t.1).min().unwrap_or(0);
            let ty1 = list.iter().map(|t| t.1).max().unwrap_or(0);
            let (rows, cols) = ((ty1 - ty0 + 1) * 64, (tx1 - tx0 + 1) * 64);
            let words = cols as usize / 64;
            // Without terrain: not walkable.
            let mut steep = vec![u64::MAX; rows as usize * words];
            let tile_cols = words;
            let mut cells: Vec<Option<Box<TileCells>>> = (0..rows as usize / 64 * tile_cols).map(|_| None).collect();
            for (x, y, bits, mut tile) in list {
                for (r, row) in bits.iter().enumerate() {
                    steep[((y - ty0) as usize * 64 + r) * words + (x - tx0) as usize] = *row;
                }
                for i in 0..4096 {
                    let (h, w) = (tile.heights[i], tile.water[i]);
                    tile.surface[i] = if h == NO_HEIGHT {
                        0
                    } else {
                        surface(h, w).clamp(-32_000, 32_000) as i16
                    };
                    let mut f = if bits[i / 64] >> (i % 64) & 1 != 0 { STEEP } else { 0 };
                    if h == NO_HEIGHT || w & (HAZARD | DEEP) != 0 {
                        f |= NO_GO;
                    }
                    if w & WET != 0 && w >> 3 >= SWIM_DEPTH {
                        f |= SWIM;
                    }
                    tile.flags[i] = f;
                }
                cells[(y - ty0) as usize * tile_cols + (x - tx0) as usize] = Some(tile);
            }
            let mut grid = Grid {
                id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
                r0: ty0 * 64,
                c0: tx0 * 64,
                rows,
                cols,
                steep,
                words,
                cells,
                areas: Vec::new(),
                tile_cols,
                sizes: Vec::new(),
                danger: Vec::new(),
                danger_max: Vec::new(),
                graphs: Vec::new(),
                local_areas: Vec::new(),
            };
            grid.label();
            grid.graphs = (0..grid.cells.len()).map(|_| std::sync::OnceLock::new()).collect();
            grid.local_areas = (0..grid.cells.len()).map(|_| std::sync::OnceLock::new()).collect();
            terrain.grids.insert(map, grid);
        }
        // The gates' graphs the databases hold (made when they were built): none to make.
        let stored: bool = conn.query_row(
            "SELECT count(*) > 0 FROM sqlite_schema WHERE name = 'client_terrain_gates'",
            [],
            |r| r.get(0),
        )?;
        if stored {
            let mut stmt = conn.prepare("SELECT map_id, tile_x, tile_y, graph FROM client_terrain_gates")?;
            for row in stmt.query_map([], |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, i32>(1)?,
                    r.get::<_, i32>(2)?,
                    r.get::<_, Vec<u8>>(3)?,
                ))
            })? {
                let (map, x, y, blob) = row?;
                if let Some(grid) = terrain.grids.get(&map)
                    && let Some((t, _)) = grid.tile(y * 64, x * 64)
                {
                    grid.set_graph(t, &blob);
                }
            }
        }
        terrain.walks.bind(terrain.fingerprint);
        Ok(terrain)
    }

    /// The graphs of the gates of every tile with terrain (map, tile x and y, stored graph): for
    /// the databases to hold them, made once (see `gates`).
    pub fn gate_graphs(&self) -> Vec<(i64, i32, i32, Vec<u8>)> {
        let mut out = Vec::new();
        for (&map, grid) in &self.grids {
            for (t, blob) in grid.all_graphs() {
                let (r, c) = (
                    grid.r0 + (t / grid.tile_cols) as i32 * 64,
                    grid.c0 + (t % grid.tile_cols) as i32 * 64,
                );
                out.push((map, c / 64, r / 64, blob));
            }
        }
        out
    }

    /// Use `walks` as memory (the app's, kept between generations): paths already searched on
    /// this terrain data are not searched again, and the new ones stay for the next routes.
    pub fn share(&mut self, walks: Arc<Walks>) {
        walks.bind(self.fingerprint);
        self.walks = walks;
    }

    /// The paths this terrain needed so far: to save with the guide.
    pub fn memory(&self) -> WalkMemory {
        let mut paths: Vec<(Key, f32)> = self
            .used
            .iter()
            .flat_map(|s| s.lock().unwrap().iter().copied().collect::<Vec<_>>())
            .filter_map(|key| Some((key, self.walks.get(key)?)))
            .collect();
        paths.sort_by_key(|p| p.0);
        WalkMemory {
            terrain: self.fingerprint,
            paths,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.grids.is_empty()
    }

    /// Cell (row from the north, column from the west) of a world position.
    fn cell(p: &Pos) -> (i32, i32) {
        // `f64::floor` is a call into the C library here: truncation, one less when negative.
        let floor = |v: f64| {
            let t = v as i32;
            if f64::from(t) > v { t - 1 } else { t }
        };
        (floor((ORIGIN - p.x) / CELL), floor((ORIGIN - p.y) / CELL))
    }

    /// The nearest cell of a large walkable area (the cell itself when in one), else the
    /// nearest walkable cell, else the cell.
    fn snap(grid: &Grid, (r, c): (i32, i32)) -> (i32, i32) {
        let large = |r: i32, c: i32| {
            let a = grid.area(r, c);
            a != 0 && grid.sizes[a as usize] >= LARGE_AREA
        };
        if large(r, c) {
            return (r, c);
        }
        let mut walkable = None;
        for d in 1..=SNAP {
            for dr in -d..=d {
                for dc in -d..=d {
                    if dr.abs() != d && dc.abs() != d {
                        continue;
                    }
                    if large(r + dr, c + dc) {
                        return (r + dr, c + dc);
                    }
                    if walkable.is_none() && !grid.blocked(r + dr, c + dc) {
                        walkable = Some((r + dr, c + dc));
                    }
                }
            }
        }
        if grid.blocked(r, c) {
            walkable.unwrap_or((r, c))
        } else {
            (r, c)
        }
    }

    /// Extra cost (tenths of a cell) of entering a cell for its danger, at `level` (0: none).
    fn danger_cost(&self, grid: &Grid, (r, c): (i32, i32), level: u8) -> u32 {
        if level == 0 {
            return 0;
        }
        let mob = grid.danger(r, c);
        if mob == 0 {
            return 0;
        }
        // Mobs below the character pull rarely and die fast; above, they hurt and slow.
        let base = match i32::from(mob) - i32::from(level) {
            ..=-3 => 0.0,
            -2..=0 => 2.0,
            1..=2 => 6.0,
            3..=4 => 20.0,
            _ => 60.0,
        };
        ((base * self.danger_weight).round() as u32).min(DANGER_MAX)
    }

    /// The straight line between two cells when each of its steps can be taken on safe ground
    /// (danger counted at `level` away from the ends): -1 when all are walked, else what its
    /// swum steps make it cost (yards). None when it cannot be followed.
    fn line(&self, grid: &Grid, a: (i32, i32), b: (i32, i32), level: u8) -> Option<f64> {
        let (dr, dc) = (b.0 - a.0, b.1 - a.1);
        let n = dr.abs().max(dc.abs()) * 2;
        let mut previous = a;
        if grid.blocked(a.0, a.1) {
            return None;
        }
        let (mut cost, mut swum) = (0u32, false);
        for k in 1..=n {
            let t = f64::from(k) / f64::from(n);
            let r = (f64::from(a.0) + t * f64::from(dr)).round() as i32;
            let c = (f64::from(a.1) + t * f64::from(dc)).round() as i32;
            if (r, c) == previous {
                continue;
            }
            let from = std::mem::replace(&mut previous, (r, c));
            let diagonal = usize::from(from.0 != r && from.1 != c);
            let step = grid.step(from, (r, c))?;
            if !Self::near_end((r, c), a, b) && self.danger_cost(grid, (r, c), level) != 0 {
                return None;
            }
            swum |= step != WALKED[diagonal];
            cost += step;
        }
        Some(if swum { f64::from(cost) / 10.0 * CELL } else { -1.0 })
    }

    /// Whether a cell is close to an end of the walk (no danger counted there).
    fn near_end(p: (i32, i32), a: (i32, i32), b: (i32, i32)) -> bool {
        let close = |q: (i32, i32)| (p.0 - q.0).abs() <= DANGER_FREE && (p.1 - q.1).abs() <= DANGER_FREE;
        close(a) || close(b)
    }

    /// Walking distance (yards) between two points of the same continent over the terrain (see
    /// the module), for a character of `level` when the danger counts (0: it does not): the
    /// straight line when nothing is in the way, infinite when no walk goes between them
    /// (`separated`), None when it does not know the way.
    pub fn distance(&self, a: &Pos, b: &Pos, level: u8) -> Option<f64> {
        let straight = a.dist(b);
        let Some(grid) = self.grids.get(&a.continent) else {
            return Some(straight);
        };
        if a.continent != b.continent || !straight.is_finite() {
            return Some(straight);
        }
        let level = if self.danger_weight > 0.0 { level } else { 0 };
        let (ca, cb) = (Self::cell(a), Self::cell(b));
        let pack = |(r, c): (i32, i32)| (u64::from(r as u16) << 16) | u64::from(c as u16);
        let key = (self.id, a.continent, (pack(ca) << 32) | pack(cb), level);
        let known = MEMO.with(|m| m.borrow().get(&key).copied());
        let walk = known.unwrap_or_else(|| {
            let w = self
                .walk_cells(grid, a.continent, ca, cb, level)
                .map_or(f32::NAN, |d| d as f32);
            MEMO.with(|m| m.borrow_mut().insert(key, w));
            w
        });
        if walk.is_nan() {
            None
        } else if walk < 0.0 {
            Some(straight)
        } else {
            Some(f64::from(walk).max(straight))
        }
    }

    /// The walkable area a point stands in (after snapping to the nearest large one) and its
    /// size in cells: None without terrain for its continent, area 0 on steep ground.
    pub fn area_of(&self, p: &Pos) -> Option<(u32, u32)> {
        let grid = self.grids.get(&p.continent)?;
        let (r, c) = Self::snap(grid, Self::cell(p));
        let area = grid.area(r, c);
        Some((area, grid.sizes.get(area as usize).copied().unwrap_or(0)))
    }

    /// The large area at `p` is left only by boat, portal or lift (Rut'theran Village, at the
    /// foot of Teldrassil): no walk between it and the other areas. Its area and size, None when
    /// `p` is not in a large area.
    pub fn enclose(&mut self, p: &Pos) -> Option<(u32, u32)> {
        let (area, size) = self.area_of(p).filter(|&(a, size)| a != 0 && size >= LARGE_AREA)?;
        // Swimming joins the shore to the mainland: enclosing it would enclose the continent.
        if self.main_area(p.continent) == Some(area) {
            return None;
        }
        self.enclosed.insert((p.continent, area));
        // Answers given before are kept by terrain: this one is another.
        self.id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        Some((area, size))
    }

    /// Count the danger of hostile mobs (`spawns`: position and level, more for an elite) for a
    /// character of the level given to `distance`, `weight` times the base costs (0: not at all).
    /// Each mob makes the cells within its pull's reach (about 20 yards) as dangerous as it.
    pub fn add_danger(&mut self, spawns: impl IntoIterator<Item = (Pos, u8)>, weight: f64) {
        self.danger_weight = weight.max(0.0);
        let mut stamped = 0u64;
        for (p, level) in spawns {
            let Some(grid) = self.grids.get_mut(&p.continent) else {
                continue;
            };
            if grid.danger.is_empty() {
                grid.danger = (0..grid.cells.len()).map(|_| None).collect();
                grid.danger_max = vec![0; grid.cells.len()];
            }
            let (r, c) = Self::cell(&p);
            for dr in -2..=2 {
                for dc in -2..=2 {
                    if dr * dr + dc * dc > 5 {
                        continue;
                    }
                    let Some((t, i)) = grid.tile(r + dr, c + dc) else {
                        continue;
                    };
                    let cells = grid.danger[t].get_or_insert_with(|| Box::new([0; 4096]));
                    cells[i] = cells[i].max(level);
                    grid.danger_max[t] = grid.danger_max[t].max(level);
                }
            }
            stamped = stamped
                .wrapping_mul(31)
                .wrapping_add(u64::from(level) ^ (p.x.to_bits() >> 20));
        }
        // Other paths: the memories of the terrain without danger are not this one's.
        let mut h = Packed::default();
        h.write_u64(self.danger_weight.to_bits());
        h.write_u64(stamped);
        self.fingerprint = self.fingerprint.wrapping_add(h.finish());
        self.walks.bind(self.fingerprint);
        self.id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    }

    /// The box of a continent's cells: first row and column, rows and columns (rows from the
    /// north, columns from the west).
    pub fn bounds(&self, continent: i64) -> Option<(i32, i32, i32, i32)> {
        let g = self.grids.get(&continent)?;
        Some((g.r0, g.c0, g.rows, g.cols))
    }

    /// A cell of a continent, None without terrain.
    pub fn view(&self, continent: i64, r: i32, c: i32) -> Option<CellView> {
        let grid = self.grids.get(&continent)?;
        let (height, water) = grid.cell(r, c)?;
        let depth = if water & WET != 0 { water >> 3 } else { 0 };
        Some(CellView {
            height,
            steep: grid.steep(r, c),
            depth,
            swum: depth >= SWIM_DEPTH,
            deep: water & DEEP != 0,
            sea_edge: water & DEEP != 0 && depth > 0,
            hazard: water & HAZARD != 0,
            danger: grid.danger(r, c),
        })
    }

    /// What entering a cell costs more for its danger, for a character of `level` (tenths of a
    /// cell, a step costing 10; 0: safe or the danger not counted).
    pub fn danger_cost_at(&self, continent: i64, r: i32, c: i32, level: u8) -> u32 {
        self.grids
            .get(&continent)
            .map_or(0, |grid| self.danger_cost(grid, (r, c), level))
    }

    /// Continents the terrain covers.
    pub fn continents(&self) -> Vec<i64> {
        let mut c: Vec<i64> = self.grids.keys().copied().collect();
        c.sort_unstable();
        c
    }

    /// The areas declared enclosed (continent, area).
    pub fn enclosed_areas(&self) -> impl Iterator<Item = (i64, u32)> + '_ {
        self.enclosed.iter().copied()
    }

    /// The largest walkable area of a continent (its mainland).
    pub fn main_area(&self, continent: i64) -> Option<u32> {
        let grid = self.grids.get(&continent)?;
        (1..grid.sizes.len()).max_by_key(|&a| grid.sizes[a]).map(|a| a as u32)
    }

    /// Whether area `a` (of continent `map`) is left only by boat, portal or lift.
    pub fn is_enclosed(&self, map: i64, a: u32) -> bool {
        self.enclosed.contains(&(map, a))
    }

    /// Two areas the terrain does not connect, large, one of them enclosed: no walk between them.
    fn apart(&self, grid: &Grid, map: i64, area: u32, other: u32) -> bool {
        let large = |a: u32| a != 0 && grid.sizes[a as usize] >= LARGE_AREA;
        area != other
            && large(area)
            && large(other)
            && (self.enclosed.contains(&(map, area)) || self.enclosed.contains(&(map, other)))
    }

    /// Whether no walk goes between `a` and `b`: two areas the terrain does not connect, one of
    /// them enclosed (no path searched).
    pub fn separated(&self, a: &Pos, b: &Pos) -> bool {
        if self.enclosed.is_empty() || a.continent != b.continent {
            return false;
        }
        let Some(grid) = self.grids.get(&a.continent) else {
            return false;
        };
        let (ca, cb) = (Self::snap(grid, Self::cell(a)), Self::snap(grid, Self::cell(b)));
        self.apart(grid, a.continent, grid.area(ca.0, ca.1), grid.area(cb.0, cb.1))
    }

    /// Walk between two cells (yards), negative for the straight line, infinite when no walk
    /// goes between them (`separated`), None when the terrain does not know the way (two areas
    /// it does not connect, which a tunnel or a bridge may: a small patch, a ledge, a cave, or no
    /// path found).
    fn walk_cells(&self, grid: &Grid, map: i64, ca: (i32, i32), cb: (i32, i32), level: u8) -> Option<f64> {
        let (ca, cb) = (Self::snap(grid, ca), Self::snap(grid, cb));
        // No mob strong enough around: the danger does not count (one search for every level).
        let level = if level > 0 && grid.danger_near(ca, cb, MARGINS[MARGINS.len() - 1], level) {
            level
        } else {
            0
        };
        if ca == cb {
            return Some(-1.0);
        }
        // A river or a lake on the way: swum straight across, as long as that can be done.
        if let Some(d) = self.line(grid, ca, cb, level) {
            return Some(d);
        }
        let (area, other) = (grid.area(ca.0, ca.1), grid.area(cb.0, cb.1));
        if area == 0 || other == 0 {
            return None;
        }
        if area != other {
            return self.apart(grid, map, area, other).then_some(f64::INFINITY);
        }
        // The cheapest path is not the same both ways (drops): the key keeps the direction.
        let key = (map, ca.0, ca.1, cb.0, cb.1, level);
        if let Some(used) = self.used.get(shard(key)) {
            used.lock().unwrap().insert(key);
        }
        if let Some(d) = self.walks.get(key) {
            return d.is_finite().then_some(f64::from(d));
        }
        // Far apart, at no danger: through the tiles' gates; else cell by cell.
        let far = (ca.0 - cb.0).abs().max((ca.1 - cb.1).abs()) > gates::FAR;
        let found = if far && level == 0 {
            grid.through_gates(ca, cb)
                .or_else(|| self.path(grid, ca, cb, MARGINS[MARGINS.len() - 1], level))
        } else {
            MARGINS.iter().find_map(|&m| self.path(grid, ca, cb, m, level))
        };
        self.walks.insert(key, found.map_or(f32::INFINITY, |d| d as f32));
        found
    }

    /// Cheapest walk between two cells (yards, danger counted as more of them), within their
    /// box grown by `margin` cells.
    fn path(&self, grid: &Grid, a: (i32, i32), b: (i32, i32), margin: i32, level: u8) -> Option<f64> {
        let (r0, c0) = (a.0.min(b.0) - margin, a.1.min(b.1) - margin);
        let (rows, cols) = (a.0.max(b.0) + margin - r0 + 1, a.1.max(b.1) + margin - c0 + 1);
        let index = |r: i32, c: i32| ((r - r0) * cols + (c - c0)) as usize;
        SCRATCH.with(|scratch| {
            let scratch = &mut *scratch.borrow_mut();
            let n = (rows * cols) as usize;
            if scratch.best.len() < n {
                scratch.best.resize(n, u32::MAX);
                scratch.round.resize(n, 0);
            }
            scratch.current = scratch.current.wrapping_add(1);
            if scratch.current == 0 {
                scratch.round.fill(0);
                scratch.current = 1;
            }
            for bucket in &mut scratch.open {
                bucket.clear();
            }
            self.search(grid, a, b, (r0, c0, rows, cols), level, scratch, &index)
        })
    }

    /// The A* of `path` in `scratch`, over the box of first row and column `r0`, `c0`, `rows` and
    /// `cols` (`index`: a cell's place in it).
    #[expect(clippy::too_many_arguments)]
    fn search(
        &self,
        grid: &Grid,
        a: (i32, i32),
        b: (i32, i32),
        (r0, c0, rows, cols): (i32, i32, i32, i32),
        level: u8,
        scratch: &mut Scratch,
        index: &impl Fn(i32, i32) -> usize,
    ) -> Option<f64> {
        let current = scratch.current;
        let (best, round, open) = (&mut scratch.best, &mut scratch.round, &mut scratch.open);
        let get = |best: &[u32], round: &[u32], k: usize| if round[k] == current { best[k] } else { u32::MAX };
        // Costs in tenths of a cell: 10 straight, 14 diagonal, more swimming or near danger.
        let h = |r: i32, c: i32| {
            let (dr, dc) = ((r - b.0).unsigned_abs(), (c - b.1).unsigned_abs());
            10 * dr.max(dc) + 4 * dr.min(dc)
        };
        // Open cells by estimate, in a ring of buckets: the heuristic is consistent (no step costs
        // less than its length), so a step raises the estimate by 0 to its cost plus 14 (fewer
        // than `BUCKETS`) and the first time the goal comes out is the cheapest.
        let (mut f, mut queued) = (h(a.0, a.1), 1usize);
        let start = index(a.0, a.1);
        (best[start], round[start]) = (0, current);
        open[f as usize % BUCKETS].push((0, a.0, a.1));
        while queued > 0 {
            let Some((g, r, c)) = open[f as usize % BUCKETS].pop() else {
                f += 1;
                continue;
            };
            queued -= 1;
            if (r, c) == b {
                return Some(f64::from(g) / 10.0 * CELL);
            }
            if g > get(best, round, index(r, c)) {
                continue;
            }
            let from = grid.probe(r, c).0;
            for (dr, dc) in DIRECTIONS {
                let (nr, nc) = (r + dr, c + dc);
                if nr < r0 || nc < c0 || nr >= r0 + rows || nc >= c0 + cols {
                    continue;
                }
                let diagonal = dr != 0 && dc != 0;
                let Some(cost) = grid.step_from(from, (nr, nc), diagonal) else {
                    continue;
                };
                // No corner cutting: both straight steps around a diagonal one can be taken.
                if diagonal
                    && (grid.step_from(from, (r + dr, c), false).is_none()
                        || grid.step_from(from, (r, c + dc), false).is_none())
                {
                    continue;
                }
                let danger = if Self::near_end((nr, nc), a, b) {
                    0
                } else {
                    self.danger_cost(grid, (nr, nc), level)
                };
                let ng = g + cost + danger;
                let k = index(nr, nc);
                if ng < get(best, round, k) {
                    (best[k], round[k]) = (ng, current);
                    open[(ng + h(nr, nc)) as usize % BUCKETS].push((ng, nr, nc));
                    queued += 1;
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tile with a wall of steep cells on column 32, open at the rows given.
    fn terrain(open: &[usize]) -> Terrain {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE client_terrain (map_id INTEGER, tile_x INTEGER, tile_y INTEGER, blocked BLOB)",
        )
        .unwrap();
        let mut blob = Vec::new();
        for r in 0..64 {
            let row: u64 = if open.contains(&r) { 0 } else { 1 << 32 };
            blob.extend(row.to_le_bytes());
        }
        conn.execute("INSERT INTO client_terrain VALUES (1, 32, 32, ?1)", [blob])
            .unwrap();
        Terrain::load(&conn).unwrap()
    }

    /// Centre of cell (r, c) of tile (32, 32).
    fn at(r: f64, c: f64) -> Pos {
        Pos {
            continent: 1,
            x: ORIGIN - 32.0 * TILE - (r + 0.5) * CELL,
            y: ORIGIN - 32.0 * TILE - (c + 0.5) * CELL,
            zone: 0,
        }
    }

    #[test]
    fn walks_around_a_wall() {
        let t = terrain(&[60, 61, 62, 63]);
        let (a, b) = (at(10.0, 20.0), at(10.0, 44.0));
        let d = t.distance(&a, &b, 0).unwrap();
        assert!(d > 2.0 * 40.0 * CELL, "around the wall through its gap: {d}");
        // Same side: straight.
        let c = at(30.0, 20.0);
        assert!((t.distance(&a, &c, 0).unwrap() - a.dist(&c)).abs() < 1e-6);
    }

    /// The memory of a terrain, saved (JSON) and given to another load of the same data, spares
    /// the search: the walk is the same and no path is added.
    #[test]
    fn saved_memory_spares_the_search() {
        let first = terrain(&[60, 61, 62, 63]);
        let (a, b) = (at(10.0, 20.0), at(10.0, 44.0));
        let walk = first.distance(&a, &b, 0).unwrap();
        let memory = first.memory();
        assert_eq!(memory.len(), 1);
        let saved: WalkMemory = serde_json::from_str(&serde_json::to_string(&memory).unwrap()).unwrap();
        assert_eq!(saved, memory);
        let walks = Arc::new(Walks::default());
        walks.add(saved);
        let mut second = terrain(&[60, 61, 62, 63]);
        second.share(Arc::clone(&walks));
        assert_eq!(walks.len(), 1);
        assert!((second.distance(&a, &b, 0).unwrap() - walk).abs() < 1e-3);
        assert_eq!(walks.len(), 1, "searched again");
    }

    /// Two large areas the terrain does not connect are walked between in a straight line (a
    /// tunnel or a bridge may join them), unless one of them is enclosed: then no walk does.
    #[test]
    fn enclosed_area_is_not_walked_out_of() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE client_terrain (map_id INTEGER, tile_x INTEGER, tile_y INTEGER, blocked BLOB)",
        )
        .unwrap();
        // Tile (33, 32), east of (32, 32), has its first column steep: a cliff between them.
        for (x, row) in [(32, 0u64), (33, 1)] {
            let blob: Vec<u8> = (0..64).flat_map(|_| row.to_le_bytes()).collect();
            conn.execute(
                "INSERT INTO client_terrain VALUES (1, ?1, 32, ?2)",
                rusqlite::params![x, blob],
            )
            .unwrap();
        }
        let mut t = Terrain::load(&conn).unwrap();
        let (west, east) = (at(10.0, 20.0), at(10.0, 64.0 + 20.0));
        assert_eq!(t.distance(&west, &east, 0), None, "way unknown: straight line");
        assert!(!t.separated(&west, &east));
        assert!(t.enclose(&east).is_some());
        assert_eq!(t.distance(&west, &east, 0), Some(f64::INFINITY));
        assert!(t.separated(&west, &east));
        assert!(
            t.distance(&east, &at(30.0, 64.0 + 40.0), 0).unwrap().is_finite(),
            "walked within"
        );
    }

    /// A memory of other terrain data (the game changed) is not used: its walks would be wrong.
    #[test]
    fn memory_of_other_terrain_is_dropped() {
        let old = terrain(&[60, 61, 62, 63]);
        old.distance(&at(10.0, 20.0), &at(10.0, 44.0), 0).unwrap();
        let walks = Arc::new(Walks::default());
        walks.add(old.memory());
        let mut new = terrain(&[0, 1, 2, 3]);
        new.share(Arc::clone(&walks));
        assert!(walks.is_empty());
    }

    /// Two continents share cell numbers: the answers of one are not reused for the other.
    #[test]
    fn continents_do_not_share_answers() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE client_terrain (map_id INTEGER, tile_x INTEGER, tile_y INTEGER, blocked BLOB)",
        )
        .unwrap();
        let wall: Vec<u8> = (0..64)
            .flat_map(|r: u64| (if r >= 60 { 0u64 } else { 1 << 32 }).to_le_bytes())
            .collect();
        let open = vec![0u8; 512];
        conn.execute("INSERT INTO client_terrain VALUES (1, 32, 32, ?1)", [wall])
            .unwrap();
        conn.execute("INSERT INTO client_terrain VALUES (0, 32, 32, ?1)", [open])
            .unwrap();
        let t = Terrain::load(&conn).unwrap();
        let on = |continent, (a, b): (Pos, Pos)| {
            let p = |q: Pos| Pos { continent, ..q };
            t.distance(&p(a), &p(b), 0).unwrap()
        };
        let pair = (at(10.0, 20.0), at(10.0, 44.0));
        let around = on(1, pair);
        let straight = on(0, pair);
        assert!(around > straight + 100.0, "walled {around} vs open {straight}");
        assert!((on(1, pair) - around).abs() < 1e-6);
    }

    /// Points on both sides of a cell border, west and east of the origin, fall in the cell
    /// `floor` gives.
    #[test]
    fn cell_of_a_point() {
        for v in [
            -3.5 * CELL,
            -CELL,
            -0.25,
            0.0,
            0.25,
            CELL,
            2.0 * CELL - 1e-9,
            70.0 * CELL,
        ] {
            let p = Pos {
                continent: 1,
                x: ORIGIN - v,
                y: ORIGIN - v,
                zone: 0,
            };
            let expected = ((ORIGIN - p.x) / CELL).floor() as i32;
            assert_eq!(Terrain::cell(&p), (expected, expected), "{v}");
        }
    }

    #[test]
    fn unconnected_sides() {
        let t = terrain(&[]);
        assert!(t.distance(&at(10.0, 20.0), &at(10.0, 44.0), 0).is_none());
    }

    /// Tile (32, 32) of continent 1 from a height and a water byte for each cell (row, column),
    /// stored as the databases do (heights as differences, zstd).
    fn shaped(cell: impl Fn(usize, usize) -> (i16, u8)) -> Terrain {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE client_terrain (map_id INTEGER, tile_x INTEGER, tile_y INTEGER, blocked BLOB,
             heights BLOB, water BLOB)",
        )
        .unwrap();
        let (mut deltas, mut water, mut previous) = (Vec::new(), Vec::new(), 0i16);
        for r in 0..64 {
            for c in 0..64 {
                let (h, w) = cell(r, c);
                deltas.extend(h.wrapping_sub(previous).to_le_bytes());
                previous = h;
                water.push(w);
            }
        }
        let pack = |b: &[u8]| zstd::encode_all(b, 3).unwrap();
        conn.execute(
            "INSERT INTO client_terrain VALUES (1, 32, 32, ?1, ?2, ?3)",
            rusqlite::params![vec![0u8; 512], pack(&deltas), pack(&water)],
        )
        .unwrap();
        Terrain::load(&conn).unwrap()
    }

    /// A cliff (13 yards high from column 32: too steep to climb, low enough to jump) is jumped
    /// down from its top, not climbed from its foot: the walk up goes round by the ramp at the
    /// bottom rows, the walk down is straight.
    #[test]
    fn cliff_jumped_down_not_climbed() {
        let t = shaped(|r, c| {
            let top = c >= 32 && r < 56;
            // The ramp: from column 32 the bottom rows rise slowly to the top.
            let ramp = c >= 32 && r >= 56;
            (
                if top {
                    13
                } else if ramp {
                    ((c - 32) * 5).min(13) as i16
                } else {
                    0
                },
                0,
            )
        });
        let (foot, top) = (at(10.0, 20.0), at(10.0, 44.0));
        let down = t.distance(&top, &foot, 0).unwrap();
        let up = t.distance(&foot, &top, 0).unwrap();
        assert!((down - top.dist(&foot)).abs() < CELL, "jumped straight down: {down}");
        assert!(up > 2.0 * down, "up by the ramp: {up} vs {down}");
    }

    /// Water deep enough is swum, slower than walking: the walk goes round a lake rather than
    /// across, unless round is much longer. The open sea is never entered.
    #[test]
    fn lakes_swum_slower_and_open_sea_avoided() {
        let lake = |deep: bool| {
            shaped(move |r, c| {
                if (8..56).contains(&r) && (28..36).contains(&c) {
                    (0, WET | (10 << 3) | if deep { DEEP } else { 0 })
                } else {
                    (5, 0)
                }
            })
        };
        let (a, b) = (at(30.0, 20.0), at(30.0, 44.0));
        let swum = lake(false).distance(&a, &b, 0).unwrap();
        assert!(
            swum > a.dist(&b) + 2.0 * CELL,
            "the lake costs more than walking: {swum}"
        );
        let round = lake(true).distance(&a, &b, 0).unwrap();
        assert!(round > swum, "the open sea is walked round: {round} vs {swum}");
    }

    /// Mobs well above the character make a walk past them dearer; for a character above them,
    /// or with the danger not counted, nothing changes.
    #[test]
    fn danger_counts_for_weaker_characters() {
        let mut t = shaped(|_, _| (0, 0));
        let (a, b) = (at(30.0, 4.0), at(30.0, 60.0));
        let plain = t.distance(&a, &b, 10).unwrap();
        let camp: Vec<(Pos, u8)> = (20..40).map(|r| (at(f64::from(r), 32.0), 30)).collect();
        t.add_danger(camp, 1.0);
        let weak = t.distance(&a, &b, 10).unwrap();
        let strong = t.distance(&a, &b, 40).unwrap();
        assert!(weak > plain + 2.0 * CELL, "around the camp: {weak} vs {plain}");
        assert!((strong - plain).abs() < 1e-3, "a level 40 walks through: {strong}");
        assert!((t.distance(&a, &b, 0).unwrap() - plain).abs() < 1e-3);
    }
}
