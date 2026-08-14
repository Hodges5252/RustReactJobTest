use wasm_bindgen::prelude::*;

pub mod city;
pub mod graph;
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
}
