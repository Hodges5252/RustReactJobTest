use wasm_bindgen::prelude::*;

pub mod agents;
pub mod city;
pub mod graph;
pub mod pathfinding;
pub mod rng;
pub mod simulation;

use city::GRID_BLOCKS;
use simulation::Simulation;

/// WASM-facing simulation handle. All per-frame state crosses the boundary as
/// flat typed arrays (spec 2.9), never JSON.
#[wasm_bindgen]
pub struct Sim {
    inner: Simulation,
}

#[wasm_bindgen]
impl Sim {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u64) -> Sim {
        Sim {
            inner: Simulation::new(seed),
        }
    }

    pub fn grid_blocks(&self) -> u32 {
        GRID_BLOCKS as u32
    }

    pub fn world_size(&self) -> f32 {
        city::WORLD_SIZE
    }

    /// Intersection positions as [x0, y0, x1, y1, ...].
    pub fn node_positions(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.inner.city.node_pos.len() * 2);
        for &(x, y) in &self.inner.city.node_pos {
            out.push(x);
            out.push(y);
        }
        out
    }

    /// Road segment endpoints as node-index pairs [a0, b0, a1, b1, ...].
    pub fn edge_endpoints(&self) -> Vec<u32> {
        let mut out = Vec::with_capacity(self.inner.graph.edges.len() * 2);
        for e in &self.inner.graph.edges {
            out.push(e.a);
            out.push(e.b);
        }
        out
    }

    /// 1 if the road segment is an arterial, else 0.
    pub fn edge_arterial(&self) -> Vec<u8> {
        self.inner
            .graph
            .edges
            .iter()
            .map(|e| e.arterial as u8)
            .collect()
    }

    /// Zone type per block (0 residential, 1 commercial, 2 industrial), row-major.
    pub fn block_zones(&self) -> Vec<u8> {
        self.inner
            .city
            .block_zone
            .iter()
            .map(|&z| z as u8)
            .collect()
    }

    /// Corner intersection node indices per block, 4 per block
    /// (top-left, top-right, bottom-right, bottom-left).
    pub fn block_corner_nodes(&self) -> Vec<u32> {
        let mut out = Vec::with_capacity(GRID_BLOCKS * GRID_BLOCKS * 4);
        for r in 0..GRID_BLOCKS {
            for c in 0..GRID_BLOCKS {
                for n in city::block_corner_nodes(r, c) {
                    out.push(n as u32);
                }
            }
        }
        out
    }

    // --- Merged block groups (spec UPDATE 2.3) ---

    /// Group id per block, row-major (indexes into the group arrays below).
    pub fn block_group_ids(&self) -> Vec<u32> {
        self.inner.city.block_group.clone()
    }

    /// Zone type per block group (0 residential, 1 commercial, 2 industrial).
    pub fn block_group_zones(&self) -> Vec<u8> {
        self.inner.city.groups.iter().map(|g| g.zone as u8).collect()
    }

    /// Offsets into `block_group_outline_nodes` per group; length is
    /// group count + 1, so group g's outline is nodes[offsets[g]..offsets[g+1]].
    pub fn block_group_outline_offsets(&self) -> Vec<u32> {
        let mut out = Vec::with_capacity(self.inner.city.groups.len() + 1);
        let mut acc = 0u32;
        out.push(0);
        for g in &self.inner.city.groups {
            acc += g.outline.len() as u32;
            out.push(acc);
        }
        out
    }

    /// Concatenated outer-boundary node indices of every block group
    /// (clockwise), indexed via `block_group_outline_offsets`.
    pub fn block_group_outline_nodes(&self) -> Vec<u32> {
        let mut out = Vec::new();
        for g in &self.inner.city.groups {
            out.extend_from_slice(&g.outline);
        }
        out
    }

    // --- Per-frame simulation API ---

    /// Advance the simulation by `real_dt` real seconds (speed multiplier
    /// already applied by the caller).
    pub fn tick(&mut self, real_dt: f32) {
        self.inner.tick(real_dt);
    }

    /// Current sim clock in seconds since midnight.
    pub fn clock_seconds(&self) -> f32 {
        self.inner.clock
    }

    /// Active (mid-trip) agents as a flat buffer: [x, y, dest_zone] per agent.
    pub fn agent_states(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.inner.agents.len() * 3);
        for a in &self.inner.agents {
            if let Some(trip) = &a.trip {
                let (x, y) = self.inner.trip_position(trip);
                out.push(x);
                out.push(y);
                out.push(a.dest_zone as u8 as f32);
            }
        }
        out
    }

    /// Current congestion speed factor per road segment (1.0 = free flow).
    pub fn edge_speed_factors(&self) -> Vec<f32> {
        self.inner
            .graph
            .edges
            .iter()
            .map(|e| e.speed_factor)
            .collect()
    }

    pub fn active_trip_count(&self) -> u32 {
        self.inner.active_trip_count()
    }

    /// Average completed-trip travel time in sim seconds (0 if none yet).
    pub fn avg_travel_time(&self) -> f32 {
        if self.inner.completed_trips == 0 {
            0.0
        } else {
            self.inner.total_travel_time / self.inner.completed_trips as f32
        }
    }
}
