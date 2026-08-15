use crate::graph::Graph;
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

/// Fraction of blocks that attempt to start a merge with same-zone neighbors
/// (spec UPDATE 2.3; tuned so roughly ~20% of cells end up part of a merge).
const MERGE_ATTEMPT_PROB: f32 = 0.30;
/// Chance an attempted merge tries for 3 cells instead of 2.
const MERGE_TRIPLE_PROB: f32 = 0.35;

/// A rendered block group: one or more same-zone grid cells whose separating
/// roads no longer exist (merged blocks, plus cells joined by pruning).
pub struct BlockGroup {
    /// Member block indices (row-major).
    pub blocks: Vec<u32>,
    /// Outer boundary as intersection node indices, clockwise, including
    /// every (jittered) lattice node along the perimeter so the fill hugs
    /// the surrounding streets exactly.
    pub outline: Vec<u32>,
    pub zone: Zone,
}

pub struct City {
    /// Jittered intersection positions, row-major (NODES_PER_SIDE^2 entries).
    /// Dead-end stub nodes are appended after the grid nodes by the graph pass.
    pub node_pos: Vec<(f32, f32)>,
    /// Zone type per block, row-major (GRID_BLOCKS^2 entries).
    pub block_zone: Vec<Zone>,
    /// Group id per block, row-major (indexes into `groups`); filled by
    /// `compute_render_groups` after the road-variety pass.
    pub block_group: Vec<u32>,
    /// Combined block groups for rendering; filled by `compute_render_groups`.
    pub groups: Vec<BlockGroup>,
    /// Node pairs of shared interior roads inside merged groups; the graph
    /// pass removes these edges (connectivity-checked).
    pub interior_edges: Vec<(u32, u32)>,
    /// Corridors of removed edges now occupied by a dead-end stub; the
    /// flanking blocks must not be visually merged across them.
    pub stub_corridors: Vec<(u32, u32)>,
}

pub fn node_index(row: usize, col: usize) -> usize {
    row * NODES_PER_SIDE + col
}

/// Every third grid line is an arterial (shared with `Graph::build`). Merges
/// never span an arterial line, so arterial roads are never removed.
pub fn is_arterial_line(i: usize) -> bool {
    i % 3 == 2
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

impl City {
    /// Interior point of a block (average of its 4 corner intersections);
    /// used as the start/end of driveway legs.
    pub fn block_centroid(&self, block: usize) -> (f32, f32) {
        let (r, c) = (block / GRID_BLOCKS, block % GRID_BLOCKS);
        let mut x = 0.0;
        let mut y = 0.0;
        for n in block_corner_nodes(r, c) {
            x += self.node_pos[n].0 / 4.0;
            y += self.node_pos[n].1 / 4.0;
        }
        (x, y)
    }
}

pub fn generate(rng: &mut Rng) -> City {
    let node_pos = generate_intersections(rng);
    let block_zone = assign_zones(rng);
    let interior_edges = merge_blocks(rng, &block_zone);
    City {
        node_pos,
        block_zone,
        block_group: Vec::new(),
        groups: Vec::new(),
        interior_edges,
        stub_corridors: Vec::new(),
    }
}

/// A candidate merged shape anchored at a block (r, c): its member cells and
/// the shared interior road edges (node pairs) to remove.
struct MergeShape {
    cells: Vec<(usize, usize)>,
    interior: Vec<(usize, usize)>,
}

/// Straight 2-3 cell run (horizontal or vertical) anchored at (r, c).
fn run_shape(r: usize, c: usize, horizontal: bool, len: usize) -> MergeShape {
    let cell = |k: usize| if horizontal { (r, c + k) } else { (r + k, c) };
    let cells = (0..len).map(cell).collect();
    let interior = (1..len)
        .map(|k| {
            if horizontal {
                (node_index(r, c + k), node_index(r + 1, c + k))
            } else {
                (node_index(r + k, c), node_index(r + k, c + 1))
            }
        })
        .collect();
    MergeShape { cells, interior }
}

/// L-shaped ("tromino") 3-cell merge anchored at (r, c) — the optional
/// stretch goal of spec UPDATE 2.3. `orient` picks which cell of the 2x2
/// square is left out (0: bottom-right, 1: bottom-left, 2: top-right).
fn tromino_shape(r: usize, c: usize, orient: usize) -> MergeShape {
    let n = node_index;
    match orient {
        // XX
        // X.
        0 => MergeShape {
            cells: vec![(r, c), (r, c + 1), (r + 1, c)],
            interior: vec![
                (n(r, c + 1), n(r + 1, c + 1)),
                (n(r + 1, c), n(r + 1, c + 1)),
            ],
        },
        // XX
        // .X
        1 => MergeShape {
            cells: vec![(r, c), (r, c + 1), (r + 1, c + 1)],
            interior: vec![
                (n(r, c + 1), n(r + 1, c + 1)),
                (n(r + 1, c + 1), n(r + 1, c + 2)),
            ],
        },
        // X.
        // XX
        _ => MergeShape {
            cells: vec![(r, c), (r + 1, c), (r + 1, c + 1)],
            interior: vec![
                (n(r + 1, c), n(r + 1, c + 1)),
                (n(r + 1, c + 1), n(r + 2, c + 1)),
            ],
        },
    }
}

/// Merge pass (spec UPDATE 2.3): 2-3 adjacent same-zone cells in a straight
/// line — or occasionally an L-shaped tromino — may merge into one larger
/// block. Returns the shared interior road edges to remove; the rendered
/// groups themselves are derived later from the final road graph.
fn merge_blocks(rng: &mut Rng, zones: &[Zone]) -> Vec<(u32, u32)> {
    const UNASSIGNED: u32 = u32::MAX;
    /// Chance a 3-cell merge attempt tries an L-shape before a straight run.
    const TROMINO_PROB: f32 = 0.4;

    let mut block_group = vec![UNASSIGNED; zones.len()];
    let mut next_gid = 0u32;
    let mut interior_edges: Vec<(u32, u32)> = Vec::new();

    // Whether a candidate shape fits: in bounds, same zone as the anchor,
    // not already merged, and none of its shared interior roads lying on an
    // arterial grid line (arterials must never be removed).
    let shape_fits = |shape: &MergeShape, zone: Zone, taken: &[u32]| -> bool {
        for &(rr, cc) in &shape.cells {
            if rr >= GRID_BLOCKS || cc >= GRID_BLOCKS {
                return false;
            }
            let b = block_index(rr, cc);
            if zones[b] != zone || taken[b] != UNASSIGNED {
                return false;
            }
        }
        shape.interior.iter().all(|&(a, b)| {
            // An interior edge lies on an arterial line if it is a vertical
            // edge on an arterial column or a horizontal edge on an arterial row.
            let (ar, ac) = (a / NODES_PER_SIDE, a % NODES_PER_SIDE);
            let (br, bc) = (b / NODES_PER_SIDE, b % NODES_PER_SIDE);
            if ac == bc {
                !is_arterial_line(ac)
            } else {
                !(ar == br && is_arterial_line(ar))
            }
        })
    };

    for r in 0..GRID_BLOCKS {
        for c in 0..GRID_BLOCKS {
            let b = block_index(r, c);
            if block_group[b] != UNASSIGNED {
                continue;
            }
            let gid = next_gid;
            next_gid += 1;
            let zone = zones[b];

            // Decide the shape: a merged run/tromino when eligible, else a
            // single cell.
            let mut shape = run_shape(r, c, true, 1);
            if rng.next_f32() < MERGE_ATTEMPT_PROB {
                let horizontal = rng.next_f32() < 0.5;
                let triple = rng.next_f32() < MERGE_TRIPLE_PROB;
                let mut candidates: Vec<MergeShape> = Vec::new();
                if triple {
                    if rng.next_f32() < TROMINO_PROB {
                        candidates.push(tromino_shape(r, c, rng.gen_index(3)));
                    }
                    candidates.push(run_shape(r, c, horizontal, 3));
                    candidates.push(run_shape(r, c, !horizontal, 3));
                }
                candidates.push(run_shape(r, c, horizontal, 2));
                candidates.push(run_shape(r, c, !horizontal, 2));

                if let Some(fit) = candidates
                    .into_iter()
                    .find(|s| shape_fits(s, zone, &block_group))
                {
                    shape = fit;
                }
            }

            for &(rr, cc) in &shape.cells {
                block_group[block_index(rr, cc)] = gid;
            }
            for &(a, bb) in &shape.interior {
                interior_edges.push((a as u32, bb as u32));
            }
        }
    }

    interior_edges
}

/// Compute the rendered block groups from the *final* road graph: adjacent
/// same-zone cells with no surviving road (and no dead-end stub) on their
/// shared boundary are drawn as one combined shape. Outlines are traced at
/// unit-segment resolution so they follow every jittered intersection and
/// never cut across a street.
pub fn compute_render_groups(city: &mut City, graph: &Graph) {
    let n_blocks = GRID_BLOCKS * GRID_BLOCKS;
    let has_stub = |a: usize, b: usize| {
        city.stub_corridors
            .iter()
            .any(|&(pa, pb)| {
                (pa as usize == a && pb as usize == b) || (pa as usize == b && pb as usize == a)
            })
    };
    // Two cells are visually joined when they share a zone and the road on
    // their common boundary is gone (merged away or pruned) with no stub in it.
    let joined = |r1: usize, c1: usize, r2: usize, c2: usize| -> bool {
        if city.block_zone[block_index(r1, c1)] != city.block_zone[block_index(r2, c2)] {
            return false;
        }
        let (a, b) = if r1 == r2 {
            // Horizontal neighbors: shared vertical boundary at max col.
            let cc = c1.max(c2);
            (node_index(r1, cc), node_index(r1 + 1, cc))
        } else {
            // Vertical neighbors: shared horizontal boundary at max row.
            let rr = r1.max(r2);
            (node_index(rr, c1), node_index(rr, c1 + 1))
        };
        !graph.has_edge(a as u32, b as u32) && !has_stub(a, b)
    };

    // Connected components over the "joined" relation.
    let mut comp = vec![u32::MAX; n_blocks];
    let mut comp_count = 0u32;
    for start in 0..n_blocks {
        if comp[start] != u32::MAX {
            continue;
        }
        comp[start] = comp_count;
        let mut stack = vec![start];
        while let Some(b) = stack.pop() {
            let (r, c) = (b / GRID_BLOCKS, b % GRID_BLOCKS);
            let mut neighbors: Vec<(usize, usize)> = Vec::new();
            if c + 1 < GRID_BLOCKS {
                neighbors.push((r, c + 1));
            }
            if c > 0 {
                neighbors.push((r, c - 1));
            }
            if r + 1 < GRID_BLOCKS {
                neighbors.push((r + 1, c));
            }
            if r > 0 {
                neighbors.push((r - 1, c));
            }
            for (nr, nc) in neighbors {
                let nb = block_index(nr, nc);
                if comp[nb] == u32::MAX && joined(r, c, nr, nc) {
                    comp[nb] = comp_count;
                    stack.push(nb);
                }
            }
        }
        comp_count += 1;
    }

    // Directed boundary unit segments (clockwise around each cell, y-down):
    // a cell edge is boundary when the neighbor across it is another component
    // or the grid exterior. Shared segments between in-component neighbors
    // appear in both directions and are skipped.
    struct Seg {
        from: usize,
        to: usize,
        owner: usize,
        comp: u32,
    }
    let mut segs: Vec<Seg> = Vec::new();
    for b in 0..n_blocks {
        let (r, c) = (b / GRID_BLOCKS, b % GRID_BLOCKS);
        let cid = comp[b];
        let other = |rr: isize, cc: isize| -> bool {
            rr < 0
                || cc < 0
                || rr >= GRID_BLOCKS as isize
                || cc >= GRID_BLOCKS as isize
                || comp[block_index(rr as usize, cc as usize)] != cid
        };
        let (ri, ci) = (r as isize, c as isize);
        // (neighbor, from, to) per side, clockwise orientation.
        let sides = [
            (other(ri - 1, ci), node_index(r, c), node_index(r, c + 1)), // top
            (other(ri, ci + 1), node_index(r, c + 1), node_index(r + 1, c + 1)), // right
            (other(ri + 1, ci), node_index(r + 1, c + 1), node_index(r + 1, c)), // bottom
            (other(ri, ci - 1), node_index(r + 1, c), node_index(r, c)), // left
        ];
        for (is_boundary, from, to) in sides {
            if is_boundary {
                segs.push(Seg { from, to, owner: b, comp: cid });
            }
        }
    }

    // Outgoing-segment lookup per node for loop tracing.
    let grid_nodes = NODES_PER_SIDE * NODES_PER_SIDE;
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); grid_nodes];
    for (i, s) in segs.iter().enumerate() {
        out[s.from].push(i);
    }
    let dir_of = |s: &Seg| -> (isize, isize) {
        let (fr, fc) = (s.from / NODES_PER_SIDE, s.from % NODES_PER_SIDE);
        let (tr, tc) = (s.to / NODES_PER_SIDE, s.to % NODES_PER_SIDE);
        (tr as isize - fr as isize, tc as isize - fc as isize)
    };

    // Chain segments into closed clockwise loops. Where two boundary paths
    // cross at one node (rare pinch), prefer the sharpest right turn so each
    // loop stays a simple polygon.
    let mut used = vec![false; segs.len()];
    let mut groups: Vec<BlockGroup> = Vec::new();
    let mut block_group = vec![u32::MAX; n_blocks];
    for s0 in 0..segs.len() {
        if used[s0] {
            continue;
        }
        let gid = groups.len() as u32;
        let mut outline: Vec<u32> = vec![segs[s0].from as u32];
        let mut cur = s0;
        loop {
            used[cur] = true;
            if block_group[segs[cur].owner] == u32::MAX {
                block_group[segs[cur].owner] = gid;
            }
            let end = segs[cur].to;
            if end == segs[s0].from {
                break;
            }
            outline.push(end as u32);

            let (dr, dc) = dir_of(&segs[cur]);
            let candidates: Vec<usize> = out[end]
                .iter()
                .copied()
                .filter(|&s| !used[s] && segs[s].comp == segs[cur].comp)
                .collect();
            // Preference: right turn, straight, left turn.
            let next = [(dc, -dr), (dr, dc), (-dc, dr)]
                .into_iter()
                .find_map(|want| candidates.iter().copied().find(|&s| dir_of(&segs[s]) == want))
                .or_else(|| candidates.first().copied());
            match next {
                Some(s) => cur = s,
                None => break, // degenerate; loop is closed as far as it goes
            }
        }
        groups.push(BlockGroup {
            blocks: Vec::new(),
            outline,
            zone: city.block_zone[segs[s0].owner],
        });
    }

    // Assign every cell to a group: boundary-contributing cells were tagged
    // while tracing; fully-enclosed cells (no boundary segments, effectively
    // impossible) inherit a joined neighbor's group.
    for b in 0..n_blocks {
        if block_group[b] == u32::MAX {
            let (r, c) = (b / GRID_BLOCKS, b % GRID_BLOCKS);
            let inherit = [(r, c.wrapping_sub(1)), (r.wrapping_sub(1), c), (r, c + 1), (r + 1, c)]
                .into_iter()
                .filter(|&(rr, cc)| rr < GRID_BLOCKS && cc < GRID_BLOCKS)
                .map(|(rr, cc)| block_index(rr, cc))
                .find(|&nb| comp[nb] == comp[b] && block_group[nb] != u32::MAX);
            block_group[b] = inherit.map_or(0, |nb| block_group[nb]);
        }
        groups[block_group[b] as usize].blocks.push(b as u32);
    }

    city.groups = groups;
    city.block_group = block_group;
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
