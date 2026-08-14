use sim_core::simulation::Simulation;

fn main() {
    let mut sim = Simulation::new(12345);
    let mut max_active = 0;
    let mut min_factor: f32 = 1.0;
    let mut max_congested_edges = 0;
    // One full day at 1x = 420 real seconds; tick at 60fps.
    for i in 0..(420 * 60) {
        sim.tick(1.0 / 60.0);
        max_active = max_active.max(sim.active_trip_count());
        let congested = sim
            .graph
            .edges
            .iter()
            .filter(|e| e.speed_factor < 0.9)
            .count();
        max_congested_edges = max_congested_edges.max(congested);
        for e in &sim.graph.edges {
            min_factor = min_factor.min(e.speed_factor);
        }
        if i % (30 * 60) == 0 {
            let h = (sim.clock / 3600.0) as u32;
            let m = ((sim.clock % 3600.0) / 60.0) as u32;
            println!(
                "t={:02}:{:02} active={} completed={}",
                h,
                m,
                sim.active_trip_count(),
                sim.completed_trips
            );
        }
    }
    println!(
        "day done: max_active={} completed={} avg_travel_s={:.0} min_speed_factor={:.2} max_congested_edges={}",
        max_active,
        sim.completed_trips,
        sim.total_travel_time / sim.completed_trips.max(1) as f32,
        min_factor,
        max_congested_edges
    );
}
