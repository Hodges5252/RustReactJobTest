use crate::city::{node_index, City, NODES_PER_SIDE};
use crate::rng::Rng;

/// Base travel speed in world units per simulated second, tuned so a typical
/// cross-town trip takes ~20-25 simulated minutes (a believable commute that
/// also produces visible rush-hour overlap between agents).
pub const LOCAL_BASE_SPEED: f32 = 0.55;
pub const ARTERIAL_BASE_SPEED: f32 = 0.78;
pub const LOCAL_CAPACITY: f32 = 6.0;
pub const ARTERIAL_CAPACITY: f32 = 12.0;

/// Dead-end stub roads added for organic variety (spec UPDATE 2.2).
pub const STUB_COUNT_MIN: usize = 3;
pub const STUB_COUNT_MAX: usize = 6;
/// Fraction of local (non-arterial) edges the pruning pass attempts to remove.
pub const PRUNE_FRACTION: f32 = 0.10;

pub struct Edge {
    pub a: u32,
    pub b: u32,
    pub length: f32,
    pub capacity: f32,
    pub base_speed: f32,
    pub arterial: bool,
    /// Number of agents currently traversing this segment (either direction).
    pub load: f32,
    /// Cached congestion speed factor, recomputed periodically (spec 2.4).
    pub speed_factor: f32,
}

impl Edge {
    /// Current traversal time used as the pathfinding weight.
    pub fn travel_time(&self) -> f32 {
        self.length / (self.base_speed * self.speed_factor)
    }

    pub fn effective_speed(&self) -> f32 {
        self.base_speed * self.speed_factor
    }
}

pub struct Graph {
    pub edges: Vec<Edge>,
    /// adjacency[node] = list of (edge index, neighbor node index).
    pub adjacency: Vec<Vec<(u32, u32)>>,
    pub node_count: usize,
}

/// `speed_factor = clamp(1 - (load / capacity) * 0.8, 0.2, 1.0)` per spec 2.4.
pub fn speed_factor(load: f32, capacity: f32) -> f32 {
    (1.0 - (load / capacity) * 0.8).clamp(0.2, 1.0)
}

impl Graph {
    /// Build the full grid road network: every adjacent pair of intersections
    /// is connected, so the graph is fully connected by construction.
    /// Every third grid line is an arterial for capacity/visual variety.
    pub fn build(city: &City) -> Graph {
        let node_count = NODES_PER_SIDE * NODES_PER_SIDE;
        let mut edges = Vec::new();
        let mut adjacency = vec![Vec::new(); node_count];

        let is_arterial_line = crate::city::is_arterial_line;

        let add_edge =
            |edges: &mut Vec<Edge>, adjacency: &mut Vec<Vec<(u32, u32)>>, a: usize, b: usize, arterial: bool| {
                let (ax, ay) = city.node_pos[a];
                let (bx, by) = city.node_pos[b];
                let length = ((ax - bx).powi(2) + (ay - by).powi(2)).sqrt();
                let idx = edges.len() as u32;
                edges.push(Edge {
                    a: a as u32,
                    b: b as u32,
                    length,
                    capacity: if arterial { ARTERIAL_CAPACITY } else { LOCAL_CAPACITY },
                    base_speed: if arterial { ARTERIAL_BASE_SPEED } else { LOCAL_BASE_SPEED },
                    arterial,
                    load: 0.0,
                    speed_factor: 1.0,
                });
                adjacency[a].push((idx, b as u32));
                adjacency[b].push((idx, a as u32));
            };

        for r in 0..NODES_PER_SIDE {
            for c in 0..NODES_PER_SIDE {
                if c + 1 < NODES_PER_SIDE {
                    add_edge(
                        &mut edges,
                        &mut adjacency,
                        node_index(r, c),
                        node_index(r, c + 1),
                        is_arterial_line(r),
                    );
                }
                if r + 1 < NODES_PER_SIDE {
                    add_edge(
                        &mut edges,
                        &mut adjacency,
                        node_index(r, c),
                        node_index(r + 1, c),
                        is_arterial_line(c),
                    );
                }
            }
        }

        Graph {
            edges,
            adjacency,
            node_count,
        }
    }

    /// Recompute every edge's congestion speed factor from its current load.
    pub fn refresh_speed_factors(&mut self) {
        for e in &mut self.edges {
            e.speed_factor = speed_factor(e.load, e.capacity);
        }
    }

    /// Organic-variety post-processing (spec UPDATE 2.2 + 2.3): removal of
    /// merged-block interior roads, connectivity-checked pruning of local
    /// edges, and dead-end stubs carved out of pruned street corridors. Full
    /// connectivity among the original grid intersections is preserved
    /// throughout.
    pub fn apply_variety(&mut self, city: &mut City, rng: &mut Rng) {
        // Merged-block interior roads go first, while every block perimeter
        // is still intact (so their removal can essentially never fail the
        // connectivity check); pruning then works on the resulting graph and
        // never sees those edges as candidates.
        self.remove_interior_edges(&city.interior_edges);

        // One shuffled pool of local grid edges feeds both the prune pass and
        // the dead-end conversion pass.
        let grid_nodes = (NODES_PER_SIDE * NODES_PER_SIDE) as u32;
        let mut candidates: Vec<(u32, u32)> = self
            .edges
            .iter()
            .filter(|e| !e.arterial && e.a < grid_nodes && e.b < grid_nodes)
            .map(|e| (e.a, e.b))
            .collect();
        let attempts = ((candidates.len() as f32) * PRUNE_FRACTION).round() as usize;
        shuffle(&mut candidates, rng);
        let mut pool = candidates.into_iter();

        // Prune ~10% of local edges (each removal connectivity-checked).
        for _ in 0..attempts {
            let Some((a, b)) = pool.next() else { break };
            self.try_remove_edge(a, b);
        }

        // Dead-end stubs: convert 3-6 more local street corridors into
        // cul-de-sacs — remove the through-edge (connectivity-checked) and
        // grow a leaf road partway down the now-empty corridor, so every
        // dead end sits inside the grid where a street used to be.
        let target = STUB_COUNT_MIN + rng.gen_index(STUB_COUNT_MAX - STUB_COUNT_MIN + 1);
        let mut made = 0;
        while made < target {
            let Some((a, b)) = pool.next() else { break };
            if !self.try_remove_edge(a, b) {
                continue;
            }
            let (from, to) = if rng.next_f32() < 0.5 { (a, b) } else { (b, a) };
            let t = rng.range_f32(0.4, 0.6);
            let (fx, fy) = city.node_pos[from as usize];
            let (tx, ty) = city.node_pos[to as usize];
            let leaf_pos = (fx + (tx - fx) * t, fy + (ty - fy) * t);
            let leaf = city.node_pos.len() as u32;
            city.node_pos.push(leaf_pos);

            let idx = self.edges.len() as u32;
            self.edges.push(Edge {
                a: from,
                b: leaf,
                length: ((leaf_pos.0 - fx).powi(2) + (leaf_pos.1 - fy).powi(2)).sqrt(),
                capacity: LOCAL_CAPACITY,
                base_speed: LOCAL_BASE_SPEED,
                arterial: false,
                load: 0.0,
                speed_factor: 1.0,
            });
            self.adjacency.push(Vec::new());
            self.adjacency[from as usize].push((idx, leaf));
            self.adjacency[leaf as usize].push((idx, from));
            self.node_count += 1;
            // Remember the corridor so the render pass never merges the two
            // flanking blocks across a street that still holds a stub.
            city.stub_corridors.push((a, b));
            made += 1;
        }
    }

    /// Whether an edge currently connects `a` and `b`.
    pub fn has_edge(&self, a: u32, b: u32) -> bool {
        self.adjacency[a as usize].iter().any(|&(_, n)| n == b)
    }

    /// Remove the shared interior roads of merged blocks (spec UPDATE 2.3),
    /// each subject to the same connectivity check as pruning.
    fn remove_interior_edges(&mut self, pairs: &[(u32, u32)]) {
        for &(a, b) in pairs {
            self.try_remove_edge(a, b);
        }
    }

    /// Remove the edge between `a` and `b` if it exists and its removal keeps
    /// every node reachable; returns whether it was removed.
    fn try_remove_edge(&mut self, a: u32, b: u32) -> bool {
        let Some(idx) = self
            .edges
            .iter()
            .position(|e| (e.a == a && e.b == b) || (e.a == b && e.b == a))
        else {
            return false;
        };
        if !self.is_connected_excluding(idx) {
            return false;
        }
        self.edges.swap_remove(idx);
        self.rebuild_adjacency();
        true
    }

    /// BFS from node 0 skipping one edge; true if every node stays reachable.
    fn is_connected_excluding(&self, skip_edge: usize) -> bool {
        let mut visited = vec![false; self.node_count];
        let mut stack = vec![0u32];
        visited[0] = true;
        let mut seen = 1;
        while let Some(n) = stack.pop() {
            for &(edge_idx, neighbor) in &self.adjacency[n as usize] {
                if edge_idx as usize == skip_edge || visited[neighbor as usize] {
                    continue;
                }
                visited[neighbor as usize] = true;
                seen += 1;
                stack.push(neighbor);
            }
        }
        seen == self.node_count
    }

    fn rebuild_adjacency(&mut self) {
        for adj in &mut self.adjacency {
            adj.clear();
        }
        for (i, e) in self.edges.iter().enumerate() {
            self.adjacency[e.a as usize].push((i as u32, e.b));
            self.adjacency[e.b as usize].push((i as u32, e.a));
        }
    }
}

/// Deterministic Fisher-Yates shuffle driven by the seeded PRNG.
fn shuffle<T>(items: &mut [T], rng: &mut Rng) {
    for i in (1..items.len()).rev() {
        let j = rng.gen_index(i + 1);
        items.swap(i, j);
    }
}
