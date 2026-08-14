use crate::agents::{self, Agent, Phase, Trip, DAY_END, DAY_START};
use crate::city::{self, City, Zone};
use crate::graph::Graph;
use crate::pathfinding;
use crate::rng::Rng;

/// One simulated day (61,200 sim-seconds) passes in ~7 real minutes at 1x
/// speed, within the 5-10 minute range required by spec 2.5.
pub const TIME_SCALE: f32 = (DAY_END - DAY_START) / 420.0;

/// Total resident pool. Only mid-trip agents count toward the 100-200
/// concurrently-active target (spec 2.3/2.9); this pool size produces
/// peak-period concurrency in that band.
pub const POPULATION: usize = 700;

/// Congestion-driven edge weights are refreshed at this sim-time interval.
const WEIGHT_REFRESH_INTERVAL: f32 = 30.0;

pub struct Simulation {
    pub rng: Rng,
    pub city: City,
    pub graph: Graph,
    pub agents: Vec<Agent>,
    /// Sim clock, seconds since midnight. Runs DAY_START..DAY_END.
    pub clock: f32,
    weight_refresh_accum: f32,
    pub completed_trips: u32,
    pub total_travel_time: f32,
}

impl Simulation {
    pub fn new(seed: u64) -> Simulation {
        let mut rng = Rng::new(seed);
        let city = city::generate(&mut rng);
        let graph = Graph::build(&city);
        let agents = agents::generate_population(&city, &mut rng, POPULATION);
        Simulation {
            rng,
            city,
            graph,
            agents,
            clock: DAY_START,
            weight_refresh_accum: 0.0,
            completed_trips: 0,
            total_travel_time: 0.0,
        }
    }

    /// Advance the simulation by `real_dt` real seconds (already multiplied by
    /// the UI speed setting on the JS side).
    pub fn tick(&mut self, real_dt: f32) {
        let sim_dt = real_dt * TIME_SCALE;
        self.clock += sim_dt;

        if self.clock >= DAY_END {
            self.start_new_day();
            return;
        }

        self.weight_refresh_accum += sim_dt;
        if self.weight_refresh_accum >= WEIGHT_REFRESH_INTERVAL {
            self.weight_refresh_accum = 0.0;
            self.graph.refresh_speed_factors();
        }

        self.process_departures();
        self.move_agents(sim_dt);
    }

    fn process_departures(&mut self) {
        for i in 0..self.agents.len() {
            let (phase, depart_work, depart_home) = {
                let a = &self.agents[i];
                (a.phase, a.depart_work_at, a.depart_home_at)
            };
            match phase {
                Phase::AtHome if self.clock >= depart_work => {
                    self.start_trip(i, true);
                }
                Phase::AtWork if self.clock >= depart_home => {
                    self.start_trip(i, false);
                }
                _ => {}
            }
        }
    }

    fn start_trip(&mut self, agent_idx: usize, to_work: bool) {
        let (start, goal) = {
            let a = &self.agents[agent_idx];
            if to_work {
                (a.home_node, a.work_node)
            } else {
                (a.work_node, a.home_node)
            }
        };

        let route = pathfinding::shortest_path(&self.graph, start, goal);
        let a = &mut self.agents[agent_idx];
        a.dest_zone = if to_work {
            self.city.block_zone[a.work_block as usize]
        } else {
            Zone::Residential
        };

        match route {
            Some((nodes, edges)) if !edges.is_empty() => {
                self.graph.edges[edges[0] as usize].load += 1.0;
                a.phase = if to_work { Phase::ToWork } else { Phase::ToHome };
                a.trip = Some(Trip {
                    nodes,
                    edges,
                    leg: 0,
                    leg_progress: 0.0,
                    started_at: self.clock,
                });
            }
            // Trivial or unreachable route: arrive instantly.
            _ => {
                a.phase = if to_work { Phase::AtWork } else { Phase::DoneForDay };
            }
        }
    }

    fn move_agents(&mut self, sim_dt: f32) {
        for a in &mut self.agents {
            let Some(trip) = a.trip.as_mut() else { continue };

            let mut time_left = sim_dt;
            loop {
                let edge = &self.graph.edges[trip.edges[trip.leg] as usize];
                let speed = edge.effective_speed();
                let leg_remaining = edge.length - trip.leg_progress;
                let time_needed = leg_remaining / speed;

                if time_needed > time_left {
                    trip.leg_progress += speed * time_left;
                    break;
                }

                // Finish this leg and move to the next.
                time_left -= time_needed;
                self.graph.edges[trip.edges[trip.leg] as usize].load -= 1.0;
                trip.leg += 1;
                trip.leg_progress = 0.0;

                if trip.leg >= trip.edges.len() {
                    // Arrived at destination.
                    self.completed_trips += 1;
                    self.total_travel_time += self.clock - trip.started_at;
                    a.phase = match a.phase {
                        Phase::ToWork => Phase::AtWork,
                        _ => Phase::DoneForDay,
                    };
                    a.trip = None;
                    break;
                }
                self.graph.edges[trip.edges[trip.leg] as usize].load += 1.0;
            }
        }
    }

    /// Loop back to 6:00 AM with a freshly-scheduled day (spec 2.5): agents
    /// keep home/work assignments, only departure sampling re-rolls.
    fn start_new_day(&mut self) {
        self.clock = DAY_START;
        self.weight_refresh_accum = 0.0;
        for e in &mut self.graph.edges {
            e.load = 0.0;
            e.speed_factor = 1.0;
        }
        for a in &mut self.agents {
            a.phase = Phase::AtHome;
            a.trip = None;
            a.depart_work_at = agents::sample_morning_departure(&mut self.rng);
            a.depart_home_at = agents::sample_evening_departure(&mut self.rng, a.depart_work_at);
        }
    }

    pub fn active_trip_count(&self) -> u32 {
        self.agents.iter().filter(|a| a.trip.is_some()).count() as u32
    }

    /// Interpolated world position of an in-flight trip.
    pub fn trip_position(&self, trip: &Trip) -> (f32, f32) {
        let edge = &self.graph.edges[trip.edges[trip.leg] as usize];
        let from = trip.nodes[trip.leg] as usize;
        let to = trip.nodes[trip.leg + 1] as usize;
        let t = (trip.leg_progress / edge.length).clamp(0.0, 1.0);
        let (ax, ay) = self.city.node_pos[from];
        let (bx, by) = self.city.node_pos[to];
        (ax + (bx - ax) * t, ay + (by - ay) * t)
    }
}
