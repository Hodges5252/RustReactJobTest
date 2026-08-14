use crate::city::{node_index, City, NODES_PER_SIDE};

/// Base travel speed in world units per simulated second, tuned so a typical
/// cross-town trip takes ~20-25 simulated minutes (a believable commute that
/// also produces visible rush-hour overlap between agents).
pub const LOCAL_BASE_SPEED: f32 = 0.55;
pub const ARTERIAL_BASE_SPEED: f32 = 0.78;
pub const LOCAL_CAPACITY: f32 = 6.0;
pub const ARTERIAL_CAPACITY: f32 = 12.0;

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

        let is_arterial_line = |i: usize| i % 3 == 2;

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
}
