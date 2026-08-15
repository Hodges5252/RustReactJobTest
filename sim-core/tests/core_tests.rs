use sim_core::agents::{
    Phase, Trip, TripStage, EVENING_ERRAND_END, EVENING_ERRAND_START, MIDDAY_ERRAND_END,
    MIDDAY_ERRAND_START,
};
use sim_core::city::{self, Zone, GRID_BLOCKS, NODES_PER_SIDE};
use sim_core::graph::{speed_factor, Graph};
use sim_core::pathfinding::shortest_path;
use sim_core::rng::Rng;
use sim_core::simulation::{Simulation, DRIVEWAY_LENGTH, LANE_OFFSET, MIN_FOLLOW_GAP};

/// (a) City generation must be identical across two runs with the same seed.
#[test]
fn generation_is_deterministic_for_same_seed() {
    for seed in [0u64, 1, 42, 123_456_789, u64::MAX] {
        let a = Simulation::new(seed);
        let b = Simulation::new(seed);

        assert_eq!(a.city.node_pos, b.city.node_pos, "seed {seed}: node positions differ");
        assert_eq!(a.city.block_zone, b.city.block_zone, "seed {seed}: zones differ");

        assert_eq!(a.agents.len(), b.agents.len());
        for (x, y) in a.agents.iter().zip(b.agents.iter()) {
            assert_eq!(x.home_block, y.home_block);
            assert_eq!(x.work_block, y.work_block);
            assert_eq!(x.home_node, y.home_node);
            assert_eq!(x.work_node, y.work_node);
            assert_eq!(x.depart_work_at, y.depart_work_at);
            assert_eq!(x.depart_home_at, y.depart_home_at);
        }
    }
}

#[test]
fn different_seeds_produce_different_cities() {
    let a = Simulation::new(1);
    let b = Simulation::new(2);
    assert_ne!(a.city.node_pos, b.city.node_pos);
}

#[test]
fn grid_size_is_within_spec_and_all_zone_types_exist() {
    assert!((8..=10).contains(&GRID_BLOCKS), "grid must be 8x8 to 10x10");

    let mut rng = Rng::new(7);
    let city = city::generate(&mut rng);
    for zone in [Zone::Residential, Zone::Commercial, Zone::Industrial] {
        assert!(
            city.block_zone.contains(&zone),
            "zone type {zone:?} missing from generated city"
        );
    }
}

/// (b) Pathfinding returns a valid, connected route between arbitrary blocks.
#[test]
fn pathfinding_returns_valid_connected_routes() {
    let mut rng = Rng::new(99);
    let city = city::generate(&mut rng);
    let graph = Graph::build(&city);
    let node_count = NODES_PER_SIDE * NODES_PER_SIDE;

    // Sample a spread of arbitrary node pairs, including opposite corners.
    let pairs = [
        (0u32, (node_count - 1) as u32),
        ((NODES_PER_SIDE - 1) as u32, (node_count - NODES_PER_SIDE) as u32),
        (5, 77),
        (33, 60),
    ];

    for (start, goal) in pairs {
        let (nodes, edges) =
            shortest_path(&graph, start, goal).expect("route must exist in a connected grid");
        assert_eq!(nodes.first(), Some(&start));
        assert_eq!(nodes.last(), Some(&goal));
        assert_eq!(edges.len(), nodes.len() - 1);

        // Every leg must use an edge that really connects its two nodes.
        for i in 0..edges.len() {
            let e = &graph.edges[edges[i] as usize];
            let (a, b) = (nodes[i], nodes[i + 1]);
            assert!(
                (e.a == a && e.b == b) || (e.a == b && e.b == a),
                "edge {} does not connect nodes {} -> {}",
                edges[i],
                a,
                b
            );
        }
    }
}

/// The road network must be fully connected: every intersection reachable
/// from every other (checked from node 0 by symmetry of undirected edges).
#[test]
fn road_network_is_fully_connected() {
    let mut rng = Rng::new(2024);
    let city = city::generate(&mut rng);
    let graph = Graph::build(&city);

    let mut visited = vec![false; graph.node_count];
    let mut stack = vec![0u32];
    visited[0] = true;
    while let Some(n) = stack.pop() {
        for &(_, neighbor) in &graph.adjacency[n as usize] {
            if !visited[neighbor as usize] {
                visited[neighbor as usize] = true;
                stack.push(neighbor);
            }
        }
    }
    assert!(visited.iter().all(|&v| v), "some intersections are unreachable");
}

/// (c) The congestion formula must match spec 2.4:
/// speed_factor = clamp(1 - (load / capacity) * 0.8, 0.2, 1.0)
#[test]
fn congestion_speed_factor_matches_spec() {
    assert_eq!(speed_factor(0.0, 10.0), 1.0); // free flow
    assert!((speed_factor(5.0, 10.0) - 0.6).abs() < 1e-6); // half capacity
    assert!((speed_factor(10.0, 10.0) - 0.2).abs() < 1e-6); // at capacity
    assert_eq!(speed_factor(100.0, 10.0), 0.2); // overload clamps to floor
    assert!(speed_factor(1_000_000.0, 1.0) >= 0.2, "factor must never reach zero");
}

// --- UPDATE_DOC.md realism & variety pass ---

/// Full BFS over the final (stubbed, pruned, merged) network: every node —
/// original grid intersections and stub leaves alike — must stay reachable.
fn reachable_count(graph: &Graph) -> usize {
    let mut visited = vec![false; graph.node_count];
    let mut stack = vec![0u32];
    visited[0] = true;
    while let Some(n) = stack.pop() {
        for &(_, neighbor) in &graph.adjacency[n as usize] {
            if !visited[neighbor as usize] {
                visited[neighbor as usize] = true;
                stack.push(neighbor);
            }
        }
    }
    visited.iter().filter(|&&v| v).count()
}

/// Connectivity must hold after dead-end stubs, pruning and block merges,
/// arterials must never be pruned, and some local edges must be removed.
#[test]
fn network_variety_preserves_connectivity() {
    let grid_nodes = NODES_PER_SIDE * NODES_PER_SIDE;
    for seed in [0u64, 1, 7, 42, 12345, 987_654_321] {
        let sim = Simulation::new(seed);
        let graph = &sim.graph;

        // Fully connected, including the new stub leaves.
        assert_eq!(
            reachable_count(graph),
            graph.node_count,
            "seed {seed}: network disconnected after variety pass"
        );

        // 3-6 genuine dead-end stubs, each a leaf reachable via one edge only,
        // and every leaf lies inside the grid footprint (in a street corridor),
        // not sticking out past the boundary.
        let stubs = graph.node_count - grid_nodes;
        assert!(
            (3..=6).contains(&stubs),
            "seed {seed}: expected 3-6 dead-end stubs, got {stubs}"
        );
        for leaf in grid_nodes..graph.node_count {
            assert_eq!(
                graph.adjacency[leaf].len(),
                1,
                "seed {seed}: stub node {leaf} is not a dead end"
            );
            let (x, y) = sim.city.node_pos[leaf];
            let lo = -city::JITTER;
            let hi = city::WORLD_SIZE + city::JITTER;
            assert!(
                x >= lo && x <= hi && y >= lo && y <= hi,
                "seed {seed}: dead-end leaf at ({x},{y}) is outside the grid"
            );
        }

        // Arterials are never pruned; some local edges are.
        let base = Graph::build(&sim.city);
        let base_arterial = base.edges.iter().filter(|e| e.arterial).count();
        let base_local = base.edges.len() - base_arterial;
        let arterial = graph.edges.iter().filter(|e| e.arterial).count();
        let local_grid = graph
            .edges
            .iter()
            .filter(|e| !e.arterial && (e.a as usize) < grid_nodes && (e.b as usize) < grid_nodes)
            .count();
        assert_eq!(arterial, base_arterial, "seed {seed}: an arterial was pruned");
        assert!(
            local_grid < base_local,
            "seed {seed}: no local edges were removed for variety"
        );
    }
}

/// Merged blocks must exist for typical seeds, only combine same-zone cells,
/// and the merge pass's interior roads must be gone from the graph.
#[test]
fn merged_blocks_are_valid_and_interior_roads_removed() {
    let mut merged_seen = false;
    for seed in [0u64, 1, 7, 42, 12345, 987_654_321] {
        let sim = Simulation::new(seed);
        let city = &sim.city;

        for group in &city.groups {
            assert!(!group.blocks.is_empty(), "seed {seed}: empty render group");
            for &b in &group.blocks {
                assert_eq!(
                    city.block_zone[b as usize], group.zone,
                    "seed {seed}: merged group mixes zone types"
                );
            }
            if group.blocks.len() >= 2 {
                merged_seen = true;
            }
        }

        // Every block belongs to exactly one group.
        let mut seen = vec![false; city.block_zone.len()];
        for group in &city.groups {
            for &b in &group.blocks {
                assert!(!seen[b as usize], "seed {seed}: block in two groups");
                seen[b as usize] = true;
            }
        }
        assert!(seen.iter().all(|&s| s), "seed {seed}: block missing from groups");

        // No interior road edge of a merged group survives in the graph.
        for &(a, b) in &city.interior_edges {
            assert!(
                !sim.graph
                    .edges
                    .iter()
                    .any(|e| (e.a == a && e.b == b) || (e.a == b && e.b == a)),
                "seed {seed}: interior road ({a},{b}) of a merged block still exists"
            );
        }
    }
    assert!(
        merged_seen,
        "no seed produced a merged block of 2+ cells (expected for typical seeds)"
    );
}

/// Every group's outline (rectangles and L-shaped trominoes alike) must trace
/// exactly the boundary of its member cells: the set of unit lattice segments
/// that belong to exactly one cell in the group.
#[test]
fn group_outlines_match_member_cell_boundaries() {
    let seg = |a: usize, b: usize| (a.min(b), a.max(b));
    let mut tromino_seen = false;

    for seed in [0u64, 1, 7, 42, 12345, 987_654_321] {
        let sim = Simulation::new(seed);
        for group in &sim.city.groups {
            // Boundary segments = cell-edge segments used exactly once.
            let mut expected: Vec<(usize, usize)> = Vec::new();
            for &b in &group.blocks {
                let (r, c) = (b as usize / GRID_BLOCKS, b as usize % GRID_BLOCKS);
                let n = city::node_index;
                for s in [
                    seg(n(r, c), n(r, c + 1)),         // top
                    seg(n(r + 1, c), n(r + 1, c + 1)), // bottom
                    seg(n(r, c), n(r + 1, c)),         // left
                    seg(n(r, c + 1), n(r + 1, c + 1)), // right
                ] {
                    if let Some(pos) = expected.iter().position(|&e| e == s) {
                        expected.swap_remove(pos); // shared => interior, drop
                    } else {
                        expected.push(s);
                    }
                }
            }
            expected.sort_unstable();

            // Expand the outline's (possibly multi-cell) sides into unit segments.
            let mut actual: Vec<(usize, usize)> = Vec::new();
            let outline = &group.outline;
            for i in 0..outline.len() {
                let a = outline[i] as usize;
                let b = outline[(i + 1) % outline.len()] as usize;
                let (ar, ac) = (a / NODES_PER_SIDE, a % NODES_PER_SIDE);
                let (br, bc) = (b / NODES_PER_SIDE, b % NODES_PER_SIDE);
                assert!(ar == br || ac == bc, "seed {seed}: outline side not axis-aligned");
                let steps = ar.abs_diff(br).max(ac.abs_diff(bc));
                let dr = (br as isize - ar as isize).signum();
                let dc = (bc as isize - ac as isize).signum();
                let at = |k: usize| {
                    city::node_index(
                        (ar as isize + dr * k as isize) as usize,
                        (ac as isize + dc * k as isize) as usize,
                    )
                };
                for k in 0..steps {
                    actual.push(seg(at(k), at(k + 1)));
                }
            }
            actual.sort_unstable();

            assert_eq!(
                actual, expected,
                "seed {seed}: outline does not match member-cell boundary (group blocks {:?})",
                group.blocks
            );
            // L-shaped (non-collinear) 3-cell group = tromino.
            if group.blocks.len() == 3 {
                let rows: Vec<usize> = group.blocks.iter().map(|&b| b as usize / GRID_BLOCKS).collect();
                let cols: Vec<usize> = group.blocks.iter().map(|&b| b as usize % GRID_BLOCKS).collect();
                let collinear = rows.iter().all(|&r| r == rows[0]) || cols.iter().all(|&c| c == cols[0]);
                if !collinear {
                    tromino_seen = true;
                }
            }
        }
    }
    assert!(
        tromino_seen,
        "no seed produced an L-shaped tromino merge (expected across sampled seeds)"
    );
}

/// Same-direction vehicles on one edge must keep MIN_FOLLOW_GAP between each
/// other (vehicles still entering at the node, progress 0, are exempt).
#[test]
fn following_distance_is_maintained() {
    let mut sim = Simulation::new(555);
    // Sample repeatedly through the morning rush.
    while sim.clock < 9.5 * 3600.0 {
        sim.tick(0.05);

        let mut lanes: Vec<Vec<f32>> = vec![Vec::new(); sim.graph.edges.len() * 2];
        for a in &sim.agents {
            let Some(trip) = &a.trip else { continue };
            if trip.stage != TripStage::Road {
                continue;
            }
            let e = trip.edges[trip.leg] as usize;
            let forward = trip.nodes[trip.leg] == sim.graph.edges[e].a;
            lanes[e * 2 + forward as usize].push(trip.leg_progress);
        }
        for lane in &mut lanes {
            if lane.len() < 2 {
                continue;
            }
            lane.sort_by(|x, y| y.partial_cmp(x).unwrap());
            for pair in lane.windows(2) {
                let (leader, follower) = (pair[0], pair[1]);
                if follower > 0.0 {
                    assert!(
                        leader - follower >= MIN_FOLLOW_GAP - 1e-3,
                        "follow gap violated at t={}: leader {leader}, follower {follower}",
                        sim.clock
                    );
                }
            }
        }
    }
}

/// Errand scheduling: ~15% of agents get each errand type, departures fall in
/// the configured windows, and the city stays visibly active midday and late
/// evening while commute peaks stay within the original concurrency target.
#[test]
fn errand_trips_fill_midday_and_evening() {
    let mut sim = Simulation::new(555);
    let n = sim.agents.len() as f32;

    let midday = sim.agents.iter().filter(|a| a.midday_errand_at.is_some()).count() as f32;
    let evening = sim.agents.iter().filter(|a| a.evening_errand_at.is_some()).count() as f32;
    assert!((0.05..=0.30).contains(&(midday / n)), "midday errand fraction off: {}", midday / n);
    assert!((0.05..=0.30).contains(&(evening / n)), "evening errand fraction off: {}", evening / n);

    for a in &sim.agents {
        if let Some(t) = a.midday_errand_at {
            assert!((MIDDAY_ERRAND_START..=MIDDAY_ERRAND_END).contains(&t));
        }
        if let Some(t) = a.evening_errand_at {
            assert!((EVENING_ERRAND_START..=EVENING_ERRAND_END).contains(&t));
        }
    }

    // Run the full day, sampling activity and peak concurrency.
    let midday_samples = [12.0f32, 12.5, 13.0, 13.5];
    let evening_samples = [19.5f32, 20.0, 20.5, 21.0];
    let mut midday_active = vec![0u32; midday_samples.len()];
    let mut evening_active = vec![0u32; evening_samples.len()];
    let mut peak_active = 0u32;
    let mut errand_travelers = 0u32;

    while sim.clock < 22.5 * 3600.0 {
        sim.tick(0.05);
        let hours = sim.clock / 3600.0;
        for (i, &h) in midday_samples.iter().enumerate() {
            if (hours - h).abs() < 0.05 {
                midday_active[i] = midday_active[i].max(sim.active_trip_count());
            }
        }
        for (i, &h) in evening_samples.iter().enumerate() {
            if (hours - h).abs() < 0.05 {
                evening_active[i] = evening_active[i].max(sim.active_trip_count());
            }
        }
        peak_active = peak_active.max(sim.active_trip_count());
        errand_travelers += sim
            .agents
            .iter()
            .filter(|a| {
                matches!(
                    a.phase,
                    Phase::ToMiddayErrand | Phase::ToEveningErrand
                ) && a.trip.is_some()
            })
            .count() as u32;
    }

    for (i, &c) in midday_active.iter().enumerate() {
        assert!(c > 0, "no active trips at midday sample {}h", midday_samples[i]);
    }
    for (i, &c) in evening_active.iter().enumerate() {
        assert!(c > 0, "no active trips at evening sample {}h", evening_samples[i]);
    }
    assert!(errand_travelers > 0, "no agent ever traveled to an errand");
    assert!(
        (100..=240).contains(&peak_active),
        "peak concurrency out of the original target band: {peak_active}"
    );
}

/// Trips must begin with a visible pull-out from the block interior to the
/// curb, pause at the destination curb, and pull into the block interior —
/// instead of popping in/out at an intersection corner.
#[test]
fn driveway_legs_move_between_interior_and_curb() {
    let mut sim = Simulation::new(555);
    let mut saw_departing = false;
    let mut saw_pausing = false;
    let mut saw_arriving = false;

    while sim.clock < 12.0 * 3600.0 && !(saw_departing && saw_pausing && saw_arriving) {
        sim.tick(0.05);
        for a in &sim.agents {
            let Some(trip) = &a.trip else { continue };
            let (x, y) = sim.trip_position(trip);
            match trip.stage {
                TripStage::DepartDriveway if trip.stage_progress > 0.0 => {
                    // Strictly away from the route's first intersection: the
                    // vehicle is still between the block interior and the curb.
                    let (nx, ny) = sim.city.node_pos[trip.nodes[0] as usize];
                    assert!(
                        (x - nx).hypot(y - ny) > 1.0,
                        "departing agent teleported to the intersection"
                    );
                    saw_departing = true;
                }
                TripStage::ArrivePause => {
                    // Holding exactly at the destination curb midpoint.
                    assert!((x - trip.dest_curb.0).abs() < 1e-3);
                    assert!((y - trip.dest_curb.1).abs() < 1e-3);
                    saw_pausing = true;
                }
                TripStage::ArriveDriveway if trip.stage_progress > 0.25 * DRIVEWAY_LENGTH => {
                    let (nx, ny) = sim.city.node_pos[*trip.nodes.last().unwrap() as usize];
                    assert!(
                        (x - nx).hypot(y - ny) > 1.0,
                        "arriving agent still stuck at the intersection"
                    );
                    saw_arriving = true;
                }
                _ => {}
            }
        }
    }
    assert!(saw_departing, "no agent was ever observed departing via a driveway");
    assert!(saw_pausing, "no agent was ever observed pausing at the curb");
    assert!(saw_arriving, "no agent was ever observed arriving via a driveway");
}

/// Opposite-direction traffic on the same edge must render on distinct lanes.
#[test]
fn lane_offset_separates_opposing_traffic() {
    let sim = Simulation::new(42);
    let edge = &sim.graph.edges[0];
    let (a, b) = (edge.a, edge.b);

    let make_trip = |from: u32, to: u32, progress: f32| Trip {
        nodes: vec![from, to],
        edges: vec![0],
        leg: 0,
        leg_progress: progress,
        started_at: 0.0,
        stage: TripStage::Road,
        stage_progress: 0.0,
        origin_curb: (0.0, 0.0),
        origin_interior: (0.0, 0.0),
        origin_curb_len: 0.0,
        dest_curb: (0.0, 0.0),
        dest_interior: (0.0, 0.0),
        dest_curb_len: 0.0,
    };

    // Same physical point on the edge, traveling opposite directions.
    let mid = edge.length / 2.0;
    let (x1, y1) = sim.trip_position(&make_trip(a, b, mid));
    let (x2, y2) = sim.trip_position(&make_trip(b, a, edge.length - mid));
    let separation = (x1 - x2).hypot(y1 - y2);
    assert!(
        (separation - 2.0 * LANE_OFFSET).abs() < 1e-3,
        "opposing traffic not separated into lanes (separation {separation})"
    );
}

/// Agents must depart in morning/evening windows and complete trips.
#[test]
fn simulated_day_produces_trips_in_both_peaks() {
    let mut sim = Simulation::new(555);
    let total_agents = sim.agents.len() as u32;

    // Run until noon (sim starts 06:00; 420 real seconds = full day).
    while sim.clock < 12.0 * 3600.0 {
        sim.tick(0.05);
    }
    let morning_done = sim.completed_trips;
    assert!(
        morning_done > total_agents / 2,
        "most agents should have commuted by noon (got {morning_done}/{total_agents})"
    );

    // Run to end of day; evening commute should roughly double completions.
    while sim.clock < 22.5 * 3600.0 {
        sim.tick(0.05);
    }
    assert!(
        sim.completed_trips > morning_done + total_agents / 2,
        "evening commute missing (noon {morning_done}, day end {})",
        sim.completed_trips
    );
}
