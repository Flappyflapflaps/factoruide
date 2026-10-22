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
//! The walk is a little longer than the cheapest cell by cell (it goes through the gates). Where
//! the danger counts, the walks between the gates of a tile near mobs strong enough are found
//! again with it, once for all the levels that see the tile's mobs alike (`DangerKey`).

use super::{CELL, DIRECTIONS, Grid, MOVE, MOVE_NO_GO, MOVE_SWUM, SWUM, WALKED};
use rustc_hash::FxHashMap;
use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::Arc;

/// Walks farther than this (cells, one way or the other) go through the gates.
pub(super) const FAR: i32 = 96;
/// A run of crossings longer than this has a gate at each end too, not only its middle.
const LONG_RUN: usize = 8;
const NONE: u32 = u32::MAX;
/// Index in `DIRECTIONS` of the step the other way.
const OPPOSITE: [usize; 8] = [1, 0, 3, 2, 7, 6, 5, 4];
/// A walkable area within a tile smaller than this (cells: a ledge between steep slopes) has
/// no gate of its own.
const SMALL_AREA: u32 = 4;
/// Ends of walks in a tile whose costs to its gates are remembered, beyond which the tile's memo
/// starts over.
const ENDS_MAX: usize = 1 << 12;

/// The walks from a cell of a tile to its gates (or back), by cell, way and danger (None: none
/// counts), shared by the threads.
pub(super) type Ends = FxHashMap<((i32, i32), bool, Option<DangerKey>), Arc<[u32]>>;

/// The danger a walk counts: the character's level (0: none) and what entering a cell near a
/// mob a few levels above, or far above, costs more (`Grid::danger_step`).
pub(super) type Danger = (u8, [u32; 2]);
const SAFE: Danger = (0, [0; 2]);
/// What the danger makes the cells of a tile cost, the same for every level seeing its mobs
/// alike: how many of the levels of its mobs (`Grid::danger_levels`) are below a few levels
/// above the character's, how many below far above, and the danger costs.
pub(super) type DangerKey = (u16, u16, [u32; 2]);
/// The walks between the gates of a tile with the danger counted (`TileGraph::cost`), by danger.
pub(super) type DangerGraphs = Vec<(DangerKey, Arc<[u32]>)>;

/// Cheapest cost found to each gate (tile, index) of a walk, and the gate it came from
/// (`FROM_A`: the walk's start).
type Best = FxHashMap<(usize, usize), (u32, (usize, usize))>;
const FROM_A: (usize, usize) = (usize::MAX, usize::MAX);

/// Version of the making of the tiles' graphs and of the steps (the gaps `Grid::open_gaps`
/// opens): graphs stored by another (`Terrain::gate_graphs`) are made again, walking memories
/// of another are dropped.
pub const GATES_VERSION: u32 = 3;

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
    /// (row by row in the tile), or from each of them to `from` when `back`; the danger counted
    /// but around `from` when `free` (an end of the walk, as cell by cell). Only the cells of
    /// `wanted` are sure to be found: the search ends once they are.
    fn local(
        &self,
        t: usize,
        from: (i32, i32),
        back: bool,
        danger: Danger,
        free: bool,
        wanted: &[(i32, i32)],
    ) -> Vec<u32> {
        let (tr, tc) = self.origin(t);
        let inside = |(r, c): (i32, i32)| r >= tr && r < tr + 64 && c >= tc && c < tc + 64;
        let index = |(r, c): (i32, i32)| ((r - tr) * 64 + (c - tc)) as usize;
        let mut cost = vec![NONE; 4096];
        if !inside(from) {
            return cost;
        }
        let mut left = [false; 4096];
        let mut count = 0;
        for &p in wanted.iter().filter(|&&p| inside(p)) {
            count += usize::from(!std::mem::replace(&mut left[index(p)], true));
        }
        // Cost and cell in one word: the heap compares integers.
        let mut open = BinaryHeap::new();
        cost[index(from)] = 0;
        open.push(Reverse(index(from) as u64));
        while let Some(Reverse(word)) = open.pop() {
            let (g, k) = ((word >> 12) as u32, (word & 4095) as usize);
            if g > cost[k] {
                continue;
            }
            if std::mem::take(&mut left[k]) {
                count -= 1;
                if count == 0 {
                    break;
                }
            }
            let cell = (tr + (k >> 6) as i32, tc + (k & 63) as i32);
            let here = self.moves(cell.0, cell.1);
            for (d, (dr, dc)) in DIRECTIONS.into_iter().enumerate() {
                let next = (cell.0 + dr, cell.1 + dc);
                if !inside(next) {
                    continue;
                }
                // The step taken: from `cell` to `next`, or back from `next` to `cell`.
                let (moves, d) = if back {
                    (self.moves(next.0, next.1), OPPOSITE[d])
                } else {
                    (here, d)
                };
                if moves & MOVE_NO_GO != 0 || moves >> d & MOVE == 0 {
                    continue;
                }
                // No corner cutting, as cell by cell.
                let (sr, sc) = DIRECTIONS[d];
                if d >= 4 && (moves >> usize::from(sr > 0) & moves >> (2 + usize::from(sc > 0))) & MOVE == 0 {
                    continue;
                }
                let diagonal = usize::from(d >= 4);
                let step = if moves >> d & MOVE_SWUM != 0 {
                    SWUM[diagonal]
                } else {
                    WALKED[diagonal]
                };
                let y = if back { cell } else { next };
                let step = if free && super::Terrain::near_end(y, from, from) {
                    step
                } else {
                    step + self.danger_step(y, danger)
                };
                let k = index(next);
                if g + step < cost[k] {
                    cost[k] = g + step;
                    open.push(Reverse(u64::from(g + step) << 12 | k as u64));
                }
            }
        }
        cost
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
                let walks = self.local(t, from, false, SAFE, false, &gates);
                for (j, &(r, c)) in gates.iter().enumerate() {
                    cost[i * n + j] = walks[((r - tr) * 64 + (c - tc)) as usize];
                }
            }
            TileGraph { gates, cost }
        })
    }

    /// What the danger at `danger` makes the cells of tile `t` cost (`Grid::danger_step` only
    /// tells the mobs within a few levels above, a few more, far more): None when no mob there
    /// counts at its level.
    fn danger_key(&self, t: usize, danger: Danger) -> Option<DangerKey> {
        let (level, costs) = danger;
        if level == 0
            || self
                .danger_max
                .get(t)
                .is_none_or(|&m| m < level.saturating_add(super::DANGER_GAP))
        {
            return None;
        }
        let levels = &self.danger_levels[t];
        let below = |gap: u8| levels.partition_point(|&m| i32::from(m) - i32::from(level) < i32::from(gap)) as u16;
        Some((below(super::DANGER_GAP), below(super::DANGER_GAP + 2), costs))
    }

    /// The walks between the gates of tile `t` (`TileGraph::cost`) with the danger counted,
    /// found once for all the threads and the levels seeing its mobs alike; None when no mob
    /// there counts at its level.
    fn danger_graph(&self, t: usize, danger: Danger) -> Option<Arc<[u32]>> {
        let key = self.danger_key(t, danger)?;
        let mut graphs = self.danger_graphs[t].lock().unwrap();
        if let Some((_, cost)) = graphs.iter().find(|(d, _)| *d == key) {
            return Some(Arc::clone(cost));
        }
        let cost = self.danger_costs(t, danger);
        let cost: Arc<[u32]> = cost.into();
        graphs.push((key, Arc::clone(&cost)));
        Some(cost)
    }

    /// The walks between the gates of tile `t` with the danger counted, found anew.
    pub(super) fn danger_costs(&self, t: usize, danger: Danger) -> Vec<u32> {
        let (tr, tc) = self.origin(t);
        let gates = &self.graph(t).gates;
        let n = gates.len();
        let mut cost = vec![NONE; n * n];
        for (i, &from) in gates.iter().enumerate() {
            let walks = self.local(t, from, false, danger, false, gates);
            for (j, &(r, c)) in gates.iter().enumerate() {
                cost[i * n + j] = walks[((r - tr) * 64 + (c - tc)) as usize];
            }
        }
        cost
    }

    /// The walks from cell `p` to the gates of its tile with the danger counted, found anew.
    pub(super) fn ends_anew(&self, p: (i32, i32), danger: Danger) -> Vec<u32> {
        self.tile(p.0, p.1).map_or_else(Vec::new, |(t, _)| {
            self.local(t, p, false, danger, true, &self.graph(t).gates)
        })
    }

    /// Cheapest walks from cell `p` of tile `t` to each of its gates (or from each of them to
    /// `p`, `back`), the danger counted away from `p`, remembered for all the threads.
    fn ends(&self, t: usize, p: (i32, i32), back: bool, danger: Danger) -> Arc<[u32]> {
        let dkey = self.danger_key(t, danger);
        let danger = if dkey.is_some() { danger } else { SAFE };
        let key = (p, back, dkey);
        if let Some(known) = self.ends[t].lock().unwrap().get(&key) {
            return Arc::clone(known);
        }
        let (tr, tc) = self.origin(t);
        let walks = self.local(t, p, back, danger, true, &self.graph(t).gates);
        let costs: Arc<[u32]> = (self.graph(t).gates.iter())
            .map(|&(r, c)| walks[((r - tr) * 64 + (c - tc)) as usize])
            .collect();
        let mut memo = self.ends[t].lock().unwrap();
        if memo.len() >= ENDS_MAX {
            memo.clear();
        }
        memo.insert(key, Arc::clone(&costs));
        costs
    }

    /// Walk (yards) from cell `a` to cell `b` through the gates, the danger counted, None when
    /// they do not lead there; its cells in `trace`: `a`, the gates it goes through, `b`.
    pub(super) fn through_gates(
        &self,
        a: (i32, i32),
        b: (i32, i32),
        danger: Danger,
        trace: Option<&mut Vec<(i32, i32)>>,
    ) -> Option<f64> {
        let ((ta, _), (tb, _)) = (self.tile(a.0, a.1)?, self.tile(b.0, b.1)?);
        let (start, goal) = (self.ends(ta, a, false, danger), self.ends(tb, b, true, danger));
        let mut dangers: FxHashMap<usize, Option<Arc<[u32]>>> = FxHashMap::default();
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
            let cost = dangers.entry(t).or_insert_with(|| self.danger_graph(t, danger));
            let cost = cost.as_deref().unwrap_or(&graph.cost);
            let gate = graph.gates[i];
            if t == tb {
                let rest = goal[i];
                if rest != NONE && g + rest < found {
                    (found, last) = (g + rest, (t, i));
                }
            }
            let n = graph.gates.len();
            for (j, &other) in graph.gates.iter().enumerate() {
                let c = cost[i * n + j];
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
                let step = if super::Terrain::near_end(next, a, b) {
                    step
                } else {
                    step + self.danger_step(next, danger)
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
