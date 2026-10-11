//! Walking around the terrain too steep to climb (mountains, mesas, cliffs), from the game
//! client's heights (`client_terrain`, computed when the databases are built): cells of
//! 8.33 yards, a tile of 64 x 64 cells per row of 64 bits.
//!
//! A straight line that only crosses walkable cells is the walk; otherwise the shortest path
//! around (A* on the cells, cached). Points the terrain does not connect (a ledge, a cave under
//! a hill) keep the straight line.
//!
//! The paths searched are slow to find (the first route of a long guide asks for thousands):
//! they are kept in a walking memory (`Walks`) that the app shares between generations, and
//! saved with each guide (`WalkMemory`).

use crate::world::Pos;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use rusqlite::Connection;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
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
const LARGE_AREA: u32 = 2000;
/// The path is searched in the box around both ends, this much larger (cells), then larger.
const MARGINS: [i32; 2] = [60, 240];
const SHARDS: usize = 32;

type Key = (i64, i32, i32, i32, i32);

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

/// Answers already given in this thread, by terrain, continent and pair of cells: the planner
/// asks the same walks millions of times. Negative: the straight line; infinite: not connected.
type Memo = FxHashMap<(u32, i64, u64), f32>;
thread_local! {
    static MEMO: RefCell<Memo> = RefCell::new(Memo::default());
}
static NEXT_ID: AtomicU32 = AtomicU32::new(1);

/// One continent's cells, densely over the box of its tiles.
struct Grid {
    r0: i32,
    c0: i32,
    rows: i32,
    cols: i32,
    /// Steep cells (and cells without terrain), a bit per cell, row by row.
    steep: Vec<u64>,
    words: usize,
    /// Connected walkable area of each cell (0: steep), by tile of 64 x 64 cells (row by row,
    /// `tile_cols` a row), and the size of each area.
    areas: Vec<TileAreas>,
    tile_cols: usize,
    sizes: Vec<u32>,
}

/// The areas of a tile's cells: one for the whole tile (no terrain, or all walkable), else each
/// cell's. A continent's box holds tens of millions of cells, most of them in such tiles.
enum TileAreas {
    Uniform(u32),
    Cells(Box<[u32]>),
}

impl Grid {
    fn blocked(&self, r: i32, c: i32) -> bool {
        let (r, c) = (r - self.r0, c - self.c0);
        if r < 0 || c < 0 || r >= self.rows || c >= self.cols {
            return true;
        }
        let i = r as usize * self.words + c as usize / 64;
        self.steep[i] >> (c % 64) & 1 != 0
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

    /// Label the walkable areas (4-connected, as paths do not cut corners).
    fn label(&mut self) {
        let mut area = vec![0; (self.rows * self.cols) as usize];
        self.sizes = vec![0];
        let mut stack = Vec::new();
        for start in 0..area.len() {
            let (r, c) = ((start as i32) / self.cols, (start as i32) % self.cols);
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
                    if area[k] == 0 && !self.blocked(nr + self.r0, nc + self.c0) {
                        area[k] = id;
                        stack.push((nr, nc));
                    }
                }
            }
            self.sizes.push(size);
        }
        // Rows and columns are whole tiles.
        let cols = self.cols as usize;
        self.tile_cols = cols / 64;
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
fn shard((_, r1, c1, r2, c2): Key) -> usize {
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

    /// Format 1: terrain fingerprint, count, then each path (continent, cells, distance), little
    /// endian, compressed.
    fn encode(&self) -> String {
        let mut bytes = Vec::with_capacity(12 + self.paths.len() * 24);
        bytes.extend(self.terrain.to_le_bytes());
        bytes.extend((self.paths.len() as u32).to_le_bytes());
        for &((map, r1, c1, r2, c2), d) in &self.paths {
            bytes.extend((map as i32).to_le_bytes());
            for v in [r1, c1, r2, c2] {
                bytes.extend(v.to_le_bytes());
            }
            bytes.extend(d.to_le_bytes());
        }
        let packed = zstd::encode_all(bytes.as_slice(), 9).unwrap_or_default();
        format!("1:{}", BASE64.encode(packed))
    }

    fn decode(text: &str) -> Option<Self> {
        let bytes = zstd::decode_all(BASE64.decode(text.strip_prefix("1:")?).ok()?.as_slice()).ok()?;
        let terrain = u64::from_le_bytes(bytes.get(..8)?.try_into().ok()?);
        let count = u32::from_le_bytes(bytes.get(8..12)?.try_into().ok()?) as usize;
        let int = |at: usize| Some(i32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?));
        let paths = (0..count)
            .map(|i| {
                let at = 12 + i * 24;
                let key = (
                    i64::from(int(at)?),
                    int(at + 4)?,
                    int(at + 8)?,
                    int(at + 12)?,
                    int(at + 16)?,
                );
                Some((key, f32::from_le_bytes(bytes.get(at + 20..at + 24)?.try_into().ok()?)))
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
}

impl Terrain {
    pub fn load(conn: &Connection) -> rusqlite::Result<Self> {
        let mut terrain = Self {
            id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
            fingerprint: 0,
            grids: FxHashMap::default(),
            walks: Arc::new(Walks::default()),
            used: (0..SHARDS).map(|_| Mutex::new(HashSet::new())).collect(),
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
        let mut tiles: HashMap<i64, Vec<(i32, i32, [u64; 64])>> = HashMap::new();
        let mut stmt = conn.prepare("SELECT map_id, tile_x, tile_y, blocked FROM client_terrain")?;
        for row in stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i32>(1)?,
                r.get::<_, i32>(2)?,
                r.get::<_, Vec<u8>>(3)?,
            ))
        })? {
            let (map, x, y, blob) = row?;
            // Whatever order the rows come in: a sum of the tiles' hashes.
            let mut h = Packed::default();
            h.write_u64(map as u64);
            h.write_u32(x as u32);
            h.write_u32(y as u32);
            h.write(&blob);
            terrain.fingerprint = terrain.fingerprint.wrapping_add(h.finish());
            if blob.len() == 512 {
                let mut rows = [0u64; 64];
                for (r, b) in rows.iter_mut().zip(blob.as_chunks::<8>().0) {
                    *r = u64::from_le_bytes(*b);
                }
                tiles.entry(map).or_default().push((x, y, rows));
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
            for (x, y, bits) in list {
                for (r, row) in bits.iter().enumerate() {
                    steep[((y - ty0) as usize * 64 + r) * words + (x - tx0) as usize] = *row;
                }
            }
            let mut grid = Grid {
                r0: ty0 * 64,
                c0: tx0 * 64,
                rows,
                cols,
                steep,
                words,
                areas: Vec::new(),
                tile_cols: 0,
                sizes: Vec::new(),
            };
            grid.label();
            terrain.grids.insert(map, grid);
        }
        terrain.walks.bind(terrain.fingerprint);
        Ok(terrain)
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

    /// Whether the straight line between two cells only crosses walkable cells.
    fn clear(grid: &Grid, a: (i32, i32), b: (i32, i32)) -> bool {
        let (dr, dc) = (b.0 - a.0, b.1 - a.1);
        let n = dr.abs().max(dc.abs()) * 2;
        (0..=n).all(|k| {
            let t = if n == 0 { 0.0 } else { f64::from(k) / f64::from(n) };
            let r = (f64::from(a.0) + t * f64::from(dr)).round() as i32;
            let c = (f64::from(a.1) + t * f64::from(dc)).round() as i32;
            !grid.blocked(r, c)
        })
    }

    /// Walking distance (yards) between two points of the same continent, around the steep
    /// terrain: the straight line when nothing is in the way, None when the terrain does not
    /// connect them.
    pub fn distance(&self, a: &Pos, b: &Pos) -> Option<f64> {
        let straight = a.dist(b);
        let Some(grid) = self.grids.get(&a.continent) else {
            return Some(straight);
        };
        if a.continent != b.continent || !straight.is_finite() {
            return Some(straight);
        }
        let (ca, cb) = (Self::cell(a), Self::cell(b));
        let (lo, hi) = if ca <= cb { (ca, cb) } else { (cb, ca) };
        let pack = |(r, c): (i32, i32)| (u64::from(r as u16) << 16) | u64::from(c as u16);
        let key = (self.id, a.continent, (pack(lo) << 32) | pack(hi));
        let known = MEMO.with(|m| m.borrow().get(&key).copied());
        let walk = known.unwrap_or_else(|| {
            let w = self
                .walk_cells(grid, a.continent, ca, cb)
                .map_or(f32::INFINITY, |d| d as f32);
            MEMO.with(|m| m.borrow_mut().insert(key, w));
            w
        });
        if walk < 0.0 {
            Some(straight)
        } else if walk.is_finite() {
            Some(f64::from(walk).max(straight))
        } else {
            None
        }
    }

    /// Walk between two cells (yards), negative for the straight line, None when not connected.
    fn walk_cells(&self, grid: &Grid, map: i64, ca: (i32, i32), cb: (i32, i32)) -> Option<f64> {
        let (ca, cb) = (Self::snap(grid, ca), Self::snap(grid, cb));
        if ca == cb || Self::clear(grid, ca, cb) {
            return Some(-1.0);
        }
        let area = grid.area(ca.0, ca.1);
        if area == 0 || area != grid.area(cb.0, cb.1) {
            return None;
        }
        let key = if ca <= cb {
            (map, ca.0, ca.1, cb.0, cb.1)
        } else {
            (map, cb.0, cb.1, ca.0, ca.1)
        };
        if let Some(used) = self.used.get(shard(key)) {
            used.lock().unwrap().insert(key);
        }
        if let Some(d) = self.walks.get(key) {
            return d.is_finite().then_some(f64::from(d));
        }
        let found = MARGINS.iter().find_map(|&m| Self::path(grid, ca, cb, m));
        self.walks.insert(key, found.map_or(f32::INFINITY, |d| d as f32));
        found
    }

    /// Shortest walk between two cells (yards), within their box grown by `margin` cells.
    fn path(grid: &Grid, a: (i32, i32), b: (i32, i32), margin: i32) -> Option<f64> {
        let (r0, c0) = (a.0.min(b.0) - margin, a.1.min(b.1) - margin);
        let (rows, cols) = (a.0.max(b.0) + margin - r0 + 1, a.1.max(b.1) + margin - c0 + 1);
        let index = |r: i32, c: i32| ((r - r0) * cols + (c - c0)) as usize;
        let mut best = vec![u32::MAX; (rows * cols) as usize];
        // Costs in tenths of a cell: 10 straight, 14 diagonal.
        let h = |r: i32, c: i32| {
            let (dr, dc) = ((r - b.0).unsigned_abs(), (c - b.1).unsigned_abs());
            10 * dr.max(dc) + 4 * dr.min(dc)
        };
        let mut open = BinaryHeap::new();
        best[index(a.0, a.1)] = 0;
        open.push(Reverse((h(a.0, a.1), 0u32, a.0, a.1)));
        while let Some(Reverse((_, g, r, c))) = open.pop() {
            if (r, c) == b {
                return Some(f64::from(g) / 10.0 * CELL);
            }
            if g > best[index(r, c)] {
                continue;
            }
            for (dr, dc) in [(-1, 0), (1, 0), (0, -1), (0, 1), (-1, -1), (-1, 1), (1, -1), (1, 1)] {
                let (nr, nc) = (r + dr, c + dc);
                if nr < r0 || nc < c0 || nr >= r0 + rows || nc >= c0 + cols || grid.blocked(nr, nc) {
                    continue;
                }
                let diagonal = dr != 0 && dc != 0;
                // No corner cutting through a steep cell.
                if diagonal && (grid.blocked(r + dr, c) || grid.blocked(r, c + dc)) {
                    continue;
                }
                let ng = g + if diagonal { 14 } else { 10 };
                let k = index(nr, nc);
                if ng < best[k] {
                    best[k] = ng;
                    open.push(Reverse((ng + h(nr, nc), ng, nr, nc)));
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
        let d = t.distance(&a, &b).unwrap();
        assert!(d > 2.0 * 40.0 * CELL, "around the wall through its gap: {d}");
        // Same side: straight.
        let c = at(30.0, 20.0);
        assert!((t.distance(&a, &c).unwrap() - a.dist(&c)).abs() < 1e-6);
    }

    /// The memory of a terrain, saved (JSON) and given to another load of the same data, spares
    /// the search: the walk is the same and no path is added.
    #[test]
    fn saved_memory_spares_the_search() {
        let first = terrain(&[60, 61, 62, 63]);
        let (a, b) = (at(10.0, 20.0), at(10.0, 44.0));
        let walk = first.distance(&a, &b).unwrap();
        let memory = first.memory();
        assert_eq!(memory.len(), 1);
        let saved: WalkMemory = serde_json::from_str(&serde_json::to_string(&memory).unwrap()).unwrap();
        assert_eq!(saved, memory);
        let walks = Arc::new(Walks::default());
        walks.add(saved);
        let mut second = terrain(&[60, 61, 62, 63]);
        second.share(Arc::clone(&walks));
        assert_eq!(walks.len(), 1);
        assert!((second.distance(&a, &b).unwrap() - walk).abs() < 1e-3);
        assert_eq!(walks.len(), 1, "searched again");
    }

    /// A memory of other terrain data (the game changed) is not used: its walks would be wrong.
    #[test]
    fn memory_of_other_terrain_is_dropped() {
        let old = terrain(&[60, 61, 62, 63]);
        old.distance(&at(10.0, 20.0), &at(10.0, 44.0)).unwrap();
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
            t.distance(&p(a), &p(b)).unwrap()
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
        assert!(t.distance(&at(10.0, 20.0), &at(10.0, 44.0)).is_none());
    }
}
