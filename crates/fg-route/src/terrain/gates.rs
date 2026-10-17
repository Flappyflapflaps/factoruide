//! Long walks through the tiles' gates, a hierarchy of the cells (as HPA*): searching a walk of
//! thousands of yards cell by cell explores hundreds of thousands of them.
//!
//! The gates of a tile are cells of its border a straight step crosses into the next tile: the
//! middle of each run of such cells, and its ends when the run is long. Both tiles of a border
//! see the same runs, so each gate has its twin across. The walks between the gates of a tile,
//! within it, are found when the tile is first needed (one search a gate over its 4096 cells)
//! and kept. A long walk then goes from its start to the gates of its tile, from gate to gate
//! (A* over a few dozen gates a tile), and from the gates of the last tile to its end. Steps are
//! the cells' (drops one way only): the walks are not the same both ways.
//!
//! The walk is a little longer than the cheapest cell by cell (it goes through the gates); the
//! danger is not counted (`Terrain::walk_cells` searches cell by cell when it counts).

use super::{CELL, DIRECTIONS, Grid};
use rustc_hash::FxHashMap;
use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::rc::Rc;

/// Walks farther than this (cells, one way or the other) go through the gates.
pub(super) const FAR: i32 = 96;
/// A run of crossings longer than this has a gate at each end too, not only its middle.
const LONG_RUN: usize = 8;
const NONE: u32 = u32::MAX;
/// A walkable area within a tile smaller than this (cells: a ledge between steep slopes) has
/// no gate of its own.
const SMALL_AREA: u32 = 16;
/// Ends of walks whose costs to the gates of their tile are remembered, beyond which the memo
/// starts over.
const ENDS_MAX: usize = 1 << 16;

/// The walks from a cell to the gates of its tile (or back), by grid, cell and way.
type Ends = FxHashMap<(u32, i32, i32, bool), Rc<[u32]>>;

thread_local! {
    /// The planner walks from and to the same places again and again.
    static ENDS: RefCell<Ends> = RefCell::new(Ends::default());
    /// The strongest mob near the walk within a tile between two of its cells (gates), by grid
    /// and cells: the same at every level.
    static STRONGEST: RefCell<Strongest> = RefCell::new(Strongest::default());
}
/// The strongest mob near walks within a tile, by grid and ends (`STRONGEST`).
type Strongest = FxHashMap<(u32, (i32, i32), (i32, i32)), u8>;
/// Walks whose strongest mob is remembered, beyond which the memo starts over.
const STRONGEST_MAX: usize = 1 << 16;

/// Cheapest cost found to each gate (tile, index) of a walk, and the gate it came from
/// (`FROM_A`: the walk's start).
type Best = FxHashMap<(usize, usize), (u32, (usize, usize))>;
const FROM_A: (usize, usize) = (usize::MAX, usize::MAX);

/// Version of the making of the tiles' graphs: graphs stored by another (`Terrain::gate_graphs`)
/// are made again.
pub const GATES_VERSION: u32 = 1;

/// A tile's gates and the walks between them within it.
pub(super) struct TileGraph {
    /// Cells of the border a straight step crosses into the next tile, sorted.
    gates: Vec<(i32, i32)>,
    /// Cheapest walk (tenths of a cell) from gate `i` to gate `j` within the tile, at
    /// `i * gates + j` (`NONE`: no walk).
    cost: Vec<u32>,
}

impl TileGraph {
    /// Stored: the version, the count of gates, each gate's row and column in its tile, then
    /// the walks between them, little endian, compressed.
    fn encode(&self, origin: (i32, i32)) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(6 + self.gates.len() * 2 + self.cost.len() * 4);
        bytes.extend(GATES_VERSION.to_le_bytes());
        bytes.extend((self.gates.len() as u16).to_le_bytes());
        for &(r, c) in &self.gates {
            bytes.extend([(r - origin.0) as u8, (c - origin.1) as u8]);
        }
        for &c in &self.cost {
            bytes.extend(c.to_le_bytes());
        }
        zstd::encode_all(bytes.as_slice(), 19).unwrap_or_default()
    }

    /// A graph stored by this version, None otherwise.
    fn decode(blob: &[u8], origin: (i32, i32)) -> Option<Self> {
        let bytes = zstd::decode_all(blob).ok()?;
        if u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?) != GATES_VERSION {
            return None;
        }
        let n = usize::from(u16::from_le_bytes(bytes.get(4..6)?.try_into().ok()?));
        let gates = (0..n)
            .map(|i| {
                let at = 6 + 2 * i;
                Some((
                    origin.0 + i32::from(*bytes.get(at)?),
                    origin.1 + i32::from(*bytes.get(at + 1)?),
                ))
            })
            .collect::<Option<Vec<_>>>()?;
        let costs = bytes.get(6 + 2 * n..6 + 2 * n + 4 * n * n)?;
        let cost = costs
            .as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32::from_le_bytes(*c))
            .collect();
        Some(Self { gates, cost })
    }
}

impl Grid {
    /// The tiles' graphs (`Terrain::gate_graphs`), each made in this thread or another.
    pub(super) fn all_graphs(&self) -> Vec<(usize, Vec<u8>)> {
        let n = self.cells.len();
        let workers = std::thread::available_parallelism().map_or(4, std::num::NonZero::get);
        std::thread::scope(|scope| {
            let parts: Vec<_> = (0..workers)
                .map(|w| {
                    scope.spawn(move || {
                        (w..n)
                            .step_by(workers)
                            .filter(|&t| self.cells[t].is_some())
                            .map(|t| (t, self.graph(t).encode(self.origin(t))))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            let mut all: Vec<(usize, Vec<u8>)> = parts.into_iter().flat_map(|p| p.join().unwrap_or_default()).collect();
            all.sort_by_key(|(t, _)| *t);
            all
        })
    }

    /// Use the graph stored for tile `t` (when made by this version).
    pub(super) fn set_graph(&self, t: usize, blob: &[u8]) {
        if let Some(graph) = TileGraph::decode(blob, self.origin(t)) {
            let _ = self.graphs[t].set(graph);
        }
    }

    /// First row and column of tile `t`.
    fn origin(&self, t: usize) -> (i32, i32) {
        (
            self.r0 + (t / self.tile_cols) as i32 * 64,
            self.c0 + (t % self.tile_cols) as i32 * 64,
        )
    }

    /// Cheapest walks (tenths of a cell) within tile `t` from cell `from` to each of its cells
    /// (row by row in the tile), or from each of them to `from` when `back`.
    fn local(&self, t: usize, from: (i32, i32), back: bool) -> Vec<u32> {
        let (tr, tc) = self.origin(t);
        let inside = |(r, c): (i32, i32)| r >= tr && r < tr + 64 && c >= tc && c < tc + 64;
        let index = |(r, c): (i32, i32)| ((r - tr) * 64 + (c - tc)) as usize;
        let mut cost = vec![NONE; 4096];
        if !inside(from) {
            return cost;
        }
        let mut open = BinaryHeap::new();
        cost[index(from)] = 0;
        open.push(Reverse((0u32, from)));
        while let Some(Reverse((g, cell))) = open.pop() {
            if g > cost[index(cell)] {
                continue;
            }
            for (dr, dc) in DIRECTIONS {
                let next = (cell.0 + dr, cell.1 + dc);
                if !inside(next) {
                    continue;
                }
                // The step taken: from `cell` to `next`, or back from `next` to `cell`.
                let (x, y) = if back { (next, cell) } else { (cell, next) };
                let Some(step) = self.step(x, y) else {
                    continue;
                };
                // No corner cutting, as cell by cell.
                if dr != 0 && dc != 0 && (self.step(x, (y.0, x.1)).is_none() || self.step(x, (x.0, y.1)).is_none()) {
                    continue;
                }
                let k = index(next);
                if g + step < cost[k] {
                    cost[k] = g + step;
                    open.push(Reverse((g + step, next)));
                }
            }
        }
        cost
    }

    /// The cells of a cheapest walk within tile `t` from `from` to `to` (both in it), in order,
    /// None when it does not join them: A* over the tile, ended at `to`.
    fn local_walk(&self, t: usize, from: (i32, i32), to: (i32, i32)) -> Option<Vec<(i32, i32)>> {
        let (tr, tc) = self.origin(t);
        let inside = |(r, c): (i32, i32)| r >= tr && r < tr + 64 && c >= tc && c < tc + 64;
        let index = |(r, c): (i32, i32)| ((r - tr) * 64 + (c - tc)) as usize;
        let cell = |k: usize| (tr + (k / 64) as i32, tc + (k % 64) as i32);
        if !inside(from) || !inside(to) {
            return None;
        }
        let h = |(r, c): (i32, i32)| {
            let (dr, dc) = ((r - to.0).unsigned_abs(), (c - to.1).unsigned_abs());
            10 * dr.max(dc) + 4 * dr.min(dc)
        };
        let mut cost = vec![NONE; 4096];
        let mut parent = vec![u16::MAX; 4096];
        let mut open = BinaryHeap::new();
        cost[index(from)] = 0;
        open.push(Reverse((h(from), 0u32, index(from) as u16)));
        while let Some(Reverse((_, g, k))) = open.pop() {
            let k = usize::from(k);
            if g > cost[k] {
                continue;
            }
            let here = cell(k);
            if here == to {
                let mut cells = vec![to];
                let mut k = k;
                while parent[k] != u16::MAX {
                    k = usize::from(parent[k]);
                    cells.push(cell(k));
                }
                cells.reverse();
                return Some(cells);
            }
            for (dr, dc) in DIRECTIONS {
                let next = (here.0 + dr, here.1 + dc);
                if !inside(next) {
                    continue;
                }
                let Some(step) = self.step(here, next) else {
                    continue;
                };
                // No corner cutting, as cell by cell.
                if dr != 0
                    && dc != 0
                    && (self.step(here, (next.0, here.1)).is_none() || self.step(here, (here.0, next.1)).is_none())
                {
                    continue;
                }
                let n = index(next);
                if g + step < cost[n] {
                    cost[n] = g + step;
                    parent[n] = k as u16;
                    open.push(Reverse((g + step + h(next), g + step, n as u16)));
                }
            }
        }
        None
    }

    /// Whether a walk through the gates (`through_gates`' trace, its ends first and last) goes
    /// near a mob of `level + DANGER_GAP` or more, the cells of its ends' surroundings left out
    /// (`DANGER_FREE`, as cell by cell): its own cells, not the tiles it crosses (a single strong
    /// mob in a tile would make every walk through it search cell by cell).
    pub(super) fn danger_along(&self, trace: &[(i32, i32)], level: u8) -> bool {
        if self.danger_max.is_empty() || trace.len() < 2 {
            return false;
        }
        let at_least = level.saturating_add(super::DANGER_GAP);
        let (a, b) = (trace[0], trace[trace.len() - 1]);
        let strongest = |cells: &[(i32, i32)], ends: bool| {
            (cells.iter())
                .filter(|&&p| !ends || !super::Terrain::near_end(p, a, b))
                .map(|&(r, c)| self.danger(r, c))
                .max()
                .unwrap_or(0)
        };
        trace.windows(2).any(|w| {
            let (p, q) = (w[0], w[1]);
            let (Some((t, _)), Some((u, _))) = (self.tile(p.0, p.1), self.tile(q.0, q.1)) else {
                return true;
            };
            if t != u {
                return strongest(&[p, q], true) >= at_least;
            }
            if self.danger_max[t] < at_least {
                return false;
            }
            let ends = p == a || q == b;
            let key = (self.id, p, q);
            let known = if ends {
                None
            } else {
                STRONGEST.with(|m| m.borrow().get(&key).copied())
            };
            let level = known.unwrap_or_else(|| {
                let level = self
                    .local_walk(t, p, q)
                    .map_or(u8::MAX, |cells| strongest(&cells, ends));
                if !ends {
                    STRONGEST.with(|m| {
                        let mut m = m.borrow_mut();
                        if m.len() >= STRONGEST_MAX {
                            m.clear();
                        }
                        m.insert(key, level);
                    });
                }
                level
            });
            level >= at_least
        })
    }

    /// The walkable areas within tile `t` (cells joined by straight steps taken both ways, as
    /// `label` but not leaving the tile), row by row; 0 for the cells never stood on.
    fn local_areas(&self, t: usize) -> &[u16] {
        self.local_areas[t].get_or_init(|| {
            let (tr, tc) = self.origin(t);
            let mut area = vec![0u16; 4096];
            let mut next = 0u16;
            let mut stack = Vec::new();
            for start in 0..4096 {
                let cell = (tr + (start / 64) as i32, tc + (start % 64) as i32);
                if area[start] != 0 || self.blocked(cell.0, cell.1) {
                    continue;
                }
                next += 1;
                area[start] = next;
                stack.push(cell);
                while let Some((r, c)) = stack.pop() {
                    for n in [(r - 1, c), (r + 1, c), (r, c - 1), (r, c + 1)] {
                        if n.0 < tr || n.0 >= tr + 64 || n.1 < tc || n.1 >= tc + 64 {
                            continue;
                        }
                        let k = ((n.0 - tr) * 64 + (n.1 - tc)) as usize;
                        if area[k] == 0 && self.joined((r, c), n) {
                            area[k] = next;
                            stack.push(n);
                        }
                    }
                }
            }
            // Small areas have no gate: their cells count as never stood on.
            let mut size = vec![0u32; usize::from(next) + 1];
            for &a in &area {
                size[usize::from(a)] += 1;
            }
            for a in &mut area {
                if size[usize::from(*a)] < SMALL_AREA {
                    *a = 0;
                }
            }
            area.into()
        })
    }

    /// The area within its tile of a cell (0: none, or out of the terrain).
    fn local_area(&self, (r, c): (i32, i32)) -> u16 {
        self.tile(r, c)
            .filter(|&(t, _)| self.cells[t].is_some())
            .map_or(0, |(t, i)| self.local_areas(t)[i])
    }

    /// The gates of tile `t`: a run of crossings ends where the areas met on either side change
    /// (a ledge, the other bank of a river: each has its gate).
    fn tile_gates(&self, t: usize) -> Vec<(i32, i32)> {
        let (tr, tc) = self.origin(t);
        let mut gates = Vec::new();
        for (dr, dc) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
            let side = |k: i32| match (dr, dc) {
                (-1, 0) => (tr, tc + k),
                (1, 0) => (tr + 63, tc + k),
                (0, -1) => (tr + k, tc),
                _ => (tr + k, tc + 63),
            };
            // Each crossing and the areas it joins (None: no crossing).
            let open: Vec<Option<(u16, u16)>> = (0..64)
                .map(|k| {
                    let p = side(k);
                    let q = (p.0 + dr, p.1 + dc);
                    let areas = (self.local_area(p), self.local_area(q));
                    (areas.0 != 0 && areas.1 != 0 && (self.step(p, q).is_some() || self.step(q, p).is_some()))
                        .then_some(areas)
                })
                .collect();
            let mut k = 0;
            while k < 64 {
                let Some(areas) = open[k] else {
                    k += 1;
                    continue;
                };
                let first = k;
                while k < 64 && open[k] == Some(areas) {
                    k += 1;
                }
                let last = k - 1;
                gates.push(side(usize::midpoint(first, last) as i32));
                if last - first + 1 > LONG_RUN {
                    gates.push(side(first as i32));
                    gates.push(side(last as i32));
                }
            }
        }
        gates.sort_unstable();
        gates.dedup();
        gates
    }

    /// The gates of tile `t` and the walks between them, found the first time.
    fn graph(&self, t: usize) -> &TileGraph {
        self.graphs[t].get_or_init(|| {
            if self.cells[t].is_none() {
                return TileGraph {
                    gates: Vec::new(),
                    cost: Vec::new(),
                };
            }
            let gates = self.tile_gates(t);
            let (tr, tc) = self.origin(t);
            let n = gates.len();
            let mut cost = vec![NONE; n * n];
            for (i, &from) in gates.iter().enumerate() {
                let walks = self.local(t, from, false);
                for (j, &(r, c)) in gates.iter().enumerate() {
                    cost[i * n + j] = walks[((r - tr) * 64 + (c - tc)) as usize];
                }
            }
            TileGraph { gates, cost }
        })
    }

    /// Cheapest walks from cell `p` of tile `t` to each of its gates (or from each of them to
    /// `p`, `back`), remembered.
    fn ends(&self, t: usize, p: (i32, i32), back: bool) -> Rc<[u32]> {
        let key = (self.id, p.0, p.1, back);
        if let Some(known) = ENDS.with(|m| m.borrow().get(&key).cloned()) {
            return known;
        }
        let (tr, tc) = self.origin(t);
        let walks = self.local(t, p, back);
        let costs: Rc<[u32]> = (self.graph(t).gates.iter())
            .map(|&(r, c)| walks[((r - tr) * 64 + (c - tc)) as usize])
            .collect();
        ENDS.with(|m| {
            let mut m = m.borrow_mut();
            if m.len() >= ENDS_MAX {
                m.clear();
            }
            m.insert(key, Rc::clone(&costs));
        });
        costs
    }

    /// Walk (yards) from cell `a` to cell `b` through the gates, None when they do not lead
    /// there; its cells in `trace`: `a`, the gates it goes through, `b`.
    pub(super) fn through_gates(
        &self,
        a: (i32, i32),
        b: (i32, i32),
        trace: Option<&mut Vec<(i32, i32)>>,
    ) -> Option<f64> {
        let ((ta, _), (tb, _)) = (self.tile(a.0, a.1)?, self.tile(b.0, b.1)?);
        let (start, goal) = (self.ends(ta, a, false), self.ends(tb, b, true));
        // Estimate: the length to `b`, as cell by cell.
        let h = |(r, c): (i32, i32)| {
            let (dr, dc) = ((r - b.0).unsigned_abs(), (c - b.1).unsigned_abs());
            10 * dr.max(dc) + 4 * dr.min(dc)
        };
        let mut best: Best = FxHashMap::default();
        let mut open = BinaryHeap::new();
        let reach = |best: &mut Best,
                     open: &mut BinaryHeap<Reverse<(u32, u32, usize, usize)>>,
                     node: (usize, usize),
                     g: u32,
                     at: (i32, i32),
                     from: (usize, usize)| {
            if best.get(&node).is_none_or(|&(known, _)| g < known) {
                best.insert(node, (g, from));
                open.push(Reverse((g + h(at), g, node.0, node.1)));
            }
        };
        for (i, &gate) in self.graph(ta).gates.iter().enumerate() {
            let g = start[i];
            if g != NONE {
                reach(&mut best, &mut open, (ta, i), g, gate, FROM_A);
            }
        }
        let (mut found, mut last) = (NONE, FROM_A);
        while let Some(Reverse((f, g, t, i))) = open.pop() {
            if f >= found {
                break;
            }
            if best.get(&(t, i)).is_some_and(|&(known, _)| g > known) {
                continue;
            }
            let graph = self.graph(t);
            let gate = graph.gates[i];
            if t == tb {
                let rest = goal[i];
                if rest != NONE && g + rest < found {
                    (found, last) = (g + rest, (t, i));
                }
            }
            let n = graph.gates.len();
            for (j, &other) in graph.gates.iter().enumerate() {
                let c = graph.cost[i * n + j];
                if j != i && c != NONE {
                    reach(&mut best, &mut open, (t, j), g + c, other, (t, i));
                }
            }
            for (dr, dc) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let next = (gate.0 + dr, gate.1 + dc);
                let Some((u, _)) = self.tile(next.0, next.1) else {
                    continue;
                };
                if u == t {
                    continue;
                }
                let Some(step) = self.step(gate, next) else {
                    continue;
                };
                if let Ok(j) = self.graph(u).gates.binary_search(&next) {
                    reach(&mut best, &mut open, (u, j), g + step, next, (t, i));
                }
            }
        }
        if found == NONE {
            return None;
        }
        if let Some(trace) = trace {
            trace.clear();
            trace.push(b);
            let mut node = last;
            while node != FROM_A {
                trace.push(self.graph(node.0).gates[node.1]);
                node = best[&node].1;
            }
            trace.push(a);
            trace.reverse();
        }
        Some(f64::from(found) / 10.0 * CELL)
    }
}
