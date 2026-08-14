use sim_core::city::{self, Zone, GRID_BLOCKS, NODES_PER_SIDE};
use sim_core::graph::{speed_factor, Graph};
use sim_core::pathfinding::shortest_path;
use sim_core::rng::Rng;
use sim_core::simulation::Simulation;

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
