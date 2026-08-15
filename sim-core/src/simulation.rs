use crate::agents::{self, Access, Agent, Phase, Trip, TripStage, DAY_END, DAY_START};
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

/// Perpendicular rendering offset to the right of the travel direction, so
/// opposite-direction traffic reads as two lanes (spec UPDATE 2.1). Sized so
/// both lanes of vehicles fit inside the rendered road width.
pub const LANE_OFFSET: f32 = 3.5;
/// Minimum gap maintained behind the vehicle ahead on the same edge and
/// direction (spec UPDATE 2.1).
pub const MIN_FOLLOW_GAP: f32 = 10.0;

/// Driveway legs (spec UPDATE 2.5): nominal length and speed of the synthetic
/// pull-out/pull-in legs at both ends of every trip. Distinctly slower than
/// LOCAL_BASE_SPEED so the maneuver is visible.
pub const DRIVEWAY_LENGTH: f32 = agents::DRIVEWAY_DEPTH;
pub const DRIVEWAY_SPEED: f32 = 0.25;
/// Speed while rolling along the street between a route node and the curb
/// midpoint (a touch under local road speed).
pub const CURB_SPEED: f32 = 0.45;
/// How long an arriving vehicle pauses at the curb before turning into the
/// block, in sim-seconds (~1 real second at 1x speed).
pub const ARRIVAL_PAUSE: f32 = 140.0;

pub struct Simulation {
    pub rng: Rng,
    pub city: City,
    pub graph: Graph,
    pub agents: Vec<Agent>,
    /// Commercial block indices, used as errand destinations.
    pub commercial_blocks: Vec<u32>,
    /// Sim clock, seconds since midnight. Runs DAY_START..DAY_END.
    pub clock: f32,
    weight_refresh_accum: f32,
    pub completed_trips: u32,
    pub total_travel_time: f32,
}

impl Simulation {
    pub fn new(seed: u64) -> Simulation {
        let mut rng = Rng::new(seed);
        let mut city = city::generate(&mut rng);
        let mut graph = Graph::build(&city);
        graph.apply_variety(&mut city, &mut rng);
        city::compute_render_groups(&mut city, &graph);
        let agents = agents::generate_population(&city, &graph, &mut rng, POPULATION);
        let commercial_blocks = city
            .block_zone
            .iter()
            .enumerate()
            .filter(|(_, &z)| z == Zone::Commercial)
            .map(|(i, _)| i as u32)
            .collect();
        Simulation {
            rng,
            city,
            graph,
            agents,
            commercial_blocks,
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
        let clock = self.clock;
        for i in 0..self.agents.len() {
            // Snapshot the fields the departure rules need so the borrow of
            // the agent doesn't outlive the calls that mutate `self`.
            let a = &self.agents[i];
            let phase = a.phase;
            let home = Access {
                node: a.home_node,
                curb: a.home_curb,
                interior: a.home_interior,
            };
            let work = Access {
                node: a.work_node,
                curb: a.work_curb,
                interior: a.work_interior,
            };
            let errand = Access {
                node: a.errand_node,
                curb: a.errand_curb,
                interior: a.errand_interior,
            };
            let (depart_work_at, depart_home_at) = (a.depart_work_at, a.depart_home_at);
            let (midday_errand_at, evening_errand_at) = (a.midday_errand_at, a.evening_errand_at);
            let errand_until = a.errand_until;
            let (home_block, work_block) = (a.home_block, a.work_block);
            let work_zone = self.city.block_zone[work_block as usize];

            match phase {
                Phase::AtHome if clock >= depart_work_at => {
                    self.start_trip(i, home, work, Phase::ToWork, work_zone);
                }
                // Commute home takes precedence over a midday errand in the
                // (rare) case both are due.
                Phase::AtWork if clock >= depart_home_at => {
                    self.start_trip(i, work, home, Phase::ToHome, Zone::Residential);
                }
                Phase::AtWork if midday_errand_at.is_some_and(|t| clock >= t) => {
                    self.agents[i].midday_errand_at = None;
                    let dest = self.pick_errand_destination(i, work_block);
                    self.start_trip(i, work, dest, Phase::ToMiddayErrand, Zone::Commercial);
                }
                Phase::AtMiddayErrand if clock >= errand_until => {
                    self.start_trip(i, errand, work, Phase::ReturningToWork, work_zone);
                }
                Phase::AtHomeEvening if evening_errand_at.is_some_and(|t| clock >= t) => {
                    self.agents[i].evening_errand_at = None;
                    // Skip (rather than start late) if the day is nearly over.
                    if clock <= agents::EVENING_ERRAND_LATEST_DEPART {
                        let dest = self.pick_errand_destination(i, home_block);
                        self.start_trip(i, home, dest, Phase::ToEveningErrand, Zone::Commercial);
                    } else {
                        self.agents[i].phase = Phase::DoneForDay;
                    }
                }
                Phase::AtEveningErrand if clock >= errand_until => {
                    self.start_trip(i, errand, home, Phase::ReturningHome, Zone::Residential);
                }
                _ => {}
            }
        }
    }

    /// Pick a random commercial block (preferring one other than `exclude`),
    /// choose how it's accessed, and remember it on the agent for the return
    /// leg.
    fn pick_errand_destination(&mut self, agent_idx: usize, exclude: u32) -> Access {
        let choices: Vec<u32> = self
            .commercial_blocks
            .iter()
            .copied()
            .filter(|&b| b != exclude)
            .collect();
        let block = if choices.is_empty() {
            self.commercial_blocks[0]
        } else {
            choices[self.rng.gen_index(choices.len())]
        };
        let access = agents::block_access(&self.city, &self.graph, &mut self.rng, block);
        let a = &mut self.agents[agent_idx];
        a.errand_block = block;
        a.errand_node = access.node;
        a.errand_curb = access.curb;
        a.errand_interior = access.interior;
        access
    }

    fn start_trip(
        &mut self,
        agent_idx: usize,
        origin: Access,
        dest: Access,
        travel_phase: Phase,
        dest_zone: Zone,
    ) {
        let route = pathfinding::shortest_path(&self.graph, origin.node, dest.node);

        let a = &mut self.agents[agent_idx];
        a.dest_zone = dest_zone;

        match route {
            Some((nodes, edges)) => {
                let dist = |p: (f32, f32), q: (f32, f32)| {
                    ((p.0 - q.0).powi(2) + (p.1 - q.1).powi(2)).sqrt()
                };
                let first = self.city.node_pos[nodes[0] as usize];
                let last = self.city.node_pos[*nodes.last().unwrap() as usize];
                a.phase = travel_phase;
                a.trip = Some(Trip {
                    origin_curb_len: dist(origin.curb, first),
                    dest_curb_len: dist(dest.curb, last),
                    nodes,
                    edges,
                    leg: 0,
                    leg_progress: 0.0,
                    started_at: self.clock,
                    stage: TripStage::DepartDriveway,
                    stage_progress: 0.0,
                    origin_curb: origin.curb,
                    origin_interior: origin.interior,
                    dest_curb: dest.curb,
                    dest_interior: dest.interior,
                });
            }
            // Unreachable destination (cannot happen in a connected graph):
            // resolve the arrival instantly rather than stranding the agent.
            None => {
                a.phase = travel_phase;
                agents::finish_arrival(a, self.clock);
            }
        }
    }

    fn move_agents(&mut self, sim_dt: f32) {
        for a in &mut self.agents {
            let Some(trip) = a.trip.as_mut() else { continue };

            let mut time_left = sim_dt;
            loop {
                // Synthetic (non-road) stages advance stage_progress toward a
                // total at a fixed rate; ArrivePause counts sim-seconds.
                let synthetic: Option<(f32, f32)> = match trip.stage {
                    TripStage::DepartDriveway | TripStage::ArriveDriveway => {
                        Some((DRIVEWAY_LENGTH, DRIVEWAY_SPEED))
                    }
                    TripStage::DepartCurb => Some((trip.origin_curb_len, CURB_SPEED)),
                    TripStage::ArriveCurb => Some((trip.dest_curb_len, CURB_SPEED)),
                    TripStage::ArrivePause => Some((ARRIVAL_PAUSE, 1.0)),
                    TripStage::Road => None,
                };

                if let Some((total, rate)) = synthetic {
                    let time_needed = (total - trip.stage_progress).max(0.0) / rate;
                    if time_needed > time_left {
                        trip.stage_progress += rate * time_left;
                        break;
                    }
                    time_left -= time_needed;
                    trip.stage_progress = 0.0;
                    match trip.stage {
                        TripStage::DepartDriveway => trip.stage = TripStage::DepartCurb,
                        TripStage::DepartCurb => {
                            if trip.edges.is_empty() {
                                trip.stage = TripStage::ArriveCurb;
                            } else {
                                trip.stage = TripStage::Road;
                                self.graph.edges[trip.edges[0] as usize].load += 1.0;
                            }
                        }
                        TripStage::ArriveCurb => trip.stage = TripStage::ArrivePause,
                        TripStage::ArrivePause => trip.stage = TripStage::ArriveDriveway,
                        TripStage::ArriveDriveway => {
                            // Arrived at destination.
                            self.completed_trips += 1;
                            self.total_travel_time += self.clock - trip.started_at;
                            agents::finish_arrival(a, self.clock);
                            break;
                        }
                        TripStage::Road => unreachable!(),
                    }
                    continue;
                }

                // Road stage: normal congested travel along graph edges.
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
                    trip.stage = TripStage::ArriveCurb;
                    trip.stage_progress = 0.0;
                } else {
                    self.graph.edges[trip.edges[trip.leg] as usize].load += 1.0;
                }
            }
        }

        self.apply_follow_clamp();
    }

    /// Follow-the-leader clamp (spec UPDATE 2.1): per edge and direction,
    /// order agents by progress and clamp each trailing agent so it never
    /// closes to less than MIN_FOLLOW_GAP behind the one ahead. Leaders are
    /// unconstrained, so this can never deadlock.
    fn apply_follow_clamp(&mut self) {
        // Two "lanes" per edge, one per travel direction.
        let mut lanes: Vec<Vec<(f32, usize)>> = vec![Vec::new(); self.graph.edges.len() * 2];
        for (i, a) in self.agents.iter().enumerate() {
            let Some(trip) = &a.trip else { continue };
            if trip.stage != TripStage::Road {
                continue;
            }
            let e = trip.edges[trip.leg] as usize;
            let forward = trip.nodes[trip.leg] == self.graph.edges[e].a;
            lanes[e * 2 + forward as usize].push((trip.leg_progress, i));
        }

        for lane in &mut lanes {
            if lane.len() < 2 {
                continue;
            }
            // Leader first; deterministic tie-break by agent index.
            lane.sort_by(|x, y| {
                y.0.partial_cmp(&x.0)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(x.1.cmp(&y.1))
            });
            let mut allowed = f32::INFINITY;
            for &(progress, idx) in lane.iter() {
                let clamped = progress.min(allowed).max(0.0);
                if clamped < progress {
                    self.agents[idx].trip.as_mut().unwrap().leg_progress = clamped;
                }
                allowed = clamped - MIN_FOLLOW_GAP;
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
            let (midday, evening, dwell) = agents::sample_errands(&mut self.rng);
            a.midday_errand_at = midday;
            a.evening_errand_at = evening;
            a.errand_dwell = dwell;
            a.errand_until = 0.0;
        }
    }

    pub fn active_trip_count(&self) -> u32 {
        self.agents.iter().filter(|a| a.trip.is_some()).count() as u32
    }

    /// Interpolated world position of an in-flight trip, including the
    /// synthetic curb/driveway stages and the per-direction lane offset
    /// (spec UPDATE 2.1/2.5).
    pub fn trip_position(&self, trip: &Trip) -> (f32, f32) {
        let lerp = |(ax, ay): (f32, f32), (bx, by): (f32, f32), t: f32| {
            (ax + (bx - ax) * t, ay + (by - ay) * t)
        };
        let frac = |progress: f32, total: f32| {
            if total <= 1e-6 {
                1.0
            } else {
                (progress / total).clamp(0.0, 1.0)
            }
        };
        match trip.stage {
            TripStage::DepartDriveway => lerp(
                trip.origin_interior,
                trip.origin_curb,
                frac(trip.stage_progress, DRIVEWAY_LENGTH),
            ),
            TripStage::DepartCurb => {
                let first = self.city.node_pos[trip.nodes[0] as usize];
                lerp(trip.origin_curb, first, frac(trip.stage_progress, trip.origin_curb_len))
            }
            TripStage::ArriveCurb => {
                let last = self.city.node_pos[*trip.nodes.last().unwrap() as usize];
                lerp(last, trip.dest_curb, frac(trip.stage_progress, trip.dest_curb_len))
            }
            TripStage::ArrivePause => trip.dest_curb,
            TripStage::ArriveDriveway => lerp(
                trip.dest_curb,
                trip.dest_interior,
                frac(trip.stage_progress, DRIVEWAY_LENGTH),
            ),
            TripStage::Road => {
                let edge = &self.graph.edges[trip.edges[trip.leg] as usize];
                let from = trip.nodes[trip.leg] as usize;
                let to = trip.nodes[trip.leg + 1] as usize;
                let t = (trip.leg_progress / edge.length).clamp(0.0, 1.0);
                let (ax, ay) = self.city.node_pos[from];
                let (bx, by) = self.city.node_pos[to];
                let (x, y) = (ax + (bx - ax) * t, ay + (by - ay) * t);
                // Offset to the right-hand side of the travel direction
                // (y-down coordinates), separating opposing traffic.
                let (dx, dy) = (bx - ax, by - ay);
                let len = (dx * dx + dy * dy).sqrt().max(1e-6);
                (x - dy / len * LANE_OFFSET, y + dx / len * LANE_OFFSET)
            }
        }
    }
}
