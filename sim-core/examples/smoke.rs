use sim_core::simulation::Simulation;

fn main() {
    let mut sim = Simulation::new(12345);
    let mut max_active = 0;
    // One full day at 1x = 420 real seconds; tick at 60fps.
    for i in 0..(420 * 60) {
        sim.tick(1.0 / 60.0);
        max_active = max_active.max(sim.active_trip_count());
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
        "day done: max_active={} completed={} avg_travel_s={:.0}",
        max_active,
        sim.completed_trips,
        sim.total_travel_time / sim.completed_trips.max(1) as f32
    );
}
