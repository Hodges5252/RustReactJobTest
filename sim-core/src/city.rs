use crate::rng::Rng;

/// Blocks per side. Spec requires 8-10; default 9.
pub const GRID_BLOCKS: usize = 9;
/// Intersections per side.
pub const NODES_PER_SIDE: usize = GRID_BLOCKS + 1;
/// World-space distance between (unjittered) intersections.
pub const CELL_SIZE: f32 = 100.0;
/// Max jitter applied to interior intersection positions.
pub const JITTER: f32 = 14.0;
/// Total world extent per axis.
pub const WORLD_SIZE: f32 = (GRID_BLOCKS as f32) * CELL_SIZE;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Zone {
    Residential = 0,
    Commercial = 1,
    Industrial = 2,
}

pub struct City {
    /// Jittered intersection positions, row-major (NODES_PER_SIDE^2 entries).
    pub node_pos: Vec<(f32, f32)>,
    /// Zone type per block, row-major (GRID_BLOCKS^2 entries).
    pub block_zone: Vec<Zone>,
}

pub fn node_index(row: usize, col: usize) -> usize {
    row * NODES_PER_SIDE + col
}

pub fn block_index(row: usize, col: usize) -> usize {
    row * GRID_BLOCKS + col
}

/// The four intersection node indices at the corners of a block,
/// in order: top-left, top-right, bottom-right, bottom-left.
pub fn block_corner_nodes(row: usize, col: usize) -> [usize; 4] {
    [
        node_index(row, col),
        node_index(row, col + 1),
        node_index(row + 1, col + 1),
        node_index(row + 1, col),
    ]
}

pub fn generate(rng: &mut Rng) -> City {
    let node_pos = generate_intersections(rng);
    let block_zone = assign_zones(rng);
    City {
        node_pos,
        block_zone,
    }
}

fn generate_intersections(rng: &mut Rng) -> Vec<(f32, f32)> {
    let mut node_pos = Vec::with_capacity(NODES_PER_SIDE * NODES_PER_SIDE);
    for r in 0..NODES_PER_SIDE {
        for c in 0..NODES_PER_SIDE {
            let jx = rng.range_f32(-JITTER, JITTER);
            let jy = rng.range_f32(-JITTER, JITTER);
            node_pos.push((c as f32 * CELL_SIZE + jx, r as f32 * CELL_SIZE + jy));
        }
    }
    node_pos
}

/// Rule-based radial zoning: commercial core at the grid center, industrial
/// clustered at 1-2 corners, residential ring everywhere else. Bounded
/// per-block randomness keeps it from being perfectly symmetric.
fn assign_zones(rng: &mut Rng) -> Vec<Zone> {
    let center = (GRID_BLOCKS as f32 - 1.0) / 2.0;
    let last = GRID_BLOCKS - 1;
    let corners = [(0usize, 0usize), (0, last), (last, 0), (last, last)];

    // Pick one or two distinct industrial corners.
    let first_corner = rng.gen_index(4);
    let use_two = rng.next_f32() < 0.5;
    let second_corner = if use_two {
        let mut c = rng.gen_index(3);
        if c >= first_corner {
            c += 1;
        }
        Some(c)
    } else {
        None
    };

    let mut zones = Vec::with_capacity(GRID_BLOCKS * GRID_BLOCKS);
    for r in 0..GRID_BLOCKS {
        for c in 0..GRID_BLOCKS {
            let dc = ((r as f32 - center).powi(2) + (c as f32 - center).powi(2)).sqrt();
            let commercial_thresh = 1.9 + rng.range_f32(-0.5, 0.5);

            let corner_dist = |idx: usize| -> f32 {
                let (cr, cc) = corners[idx];
                ((r as f32 - cr as f32).powi(2) + (c as f32 - cc as f32).powi(2)).sqrt()
            };
            let industrial_thresh = 2.2 + rng.range_f32(-0.6, 0.4);
            let near_industrial = corner_dist(first_corner) <= industrial_thresh
                || second_corner.is_some_and(|s| corner_dist(s) <= industrial_thresh);

            let zone = if dc <= commercial_thresh {
                Zone::Commercial
            } else if near_industrial {
                Zone::Industrial
            } else {
                Zone::Residential
            };
            zones.push(zone);
        }
    }

    // Safety net: guarantee all three zone types exist.
    let center_block = block_index(GRID_BLOCKS / 2, GRID_BLOCKS / 2);
    if !zones.contains(&Zone::Commercial) {
        zones[center_block] = Zone::Commercial;
    }
    if !zones.contains(&Zone::Industrial) {
        let (cr, cc) = corners[first_corner];
        zones[block_index(cr, cc)] = Zone::Industrial;
    }
    if !zones.contains(&Zone::Residential) {
        zones[block_index(0, GRID_BLOCKS / 2)] = Zone::Residential;
    }

    zones
}
