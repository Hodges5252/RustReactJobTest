use crate::city::{block_corner_nodes, City, Zone, GRID_BLOCKS};
use crate::graph::Graph;
use crate::rng::Rng;

pub const DAY_START: f32 = 6.0 * 3600.0; // 6:00 AM in seconds since midnight
pub const DAY_END: f32 = 23.0 * 3600.0; // 11:00 PM

const MORNING_PEAK: f64 = 7.5 * 3600.0; // ~7:30 AM
const EVENING_PEAK: f64 = 17.5 * 3600.0; // ~5:30 PM
const PEAK_STD: f64 = 45.0 * 60.0; // 45 minute spread

// Errand trips (spec UPDATE 2.4): a configurable fraction of agents make an
// extra midday (work -> commercial -> work) and/or evening (home ->
// commercial -> home) round trip so the city isn't quiet between the peaks.
pub const MIDDAY_ERRAND_FRACTION: f32 = 0.15;
pub const EVENING_ERRAND_FRACTION: f32 = 0.15;
pub const MIDDAY_ERRAND_START: f32 = 11.5 * 3600.0; // 11:30 AM
pub const MIDDAY_ERRAND_END: f32 = 13.5 * 3600.0; // 1:30 PM
pub const EVENING_ERRAND_START: f32 = 19.0 * 3600.0; // 7:00 PM
pub const EVENING_ERRAND_END: f32 = 21.5 * 3600.0; // 9:30 PM
/// Latest sim-time an evening errand may still depart (so the agent gets home
/// before day end).
pub const EVENING_ERRAND_LATEST_DEPART: f32 = 21.75 * 3600.0;
/// Time spent at the errand destination before heading back.
const ERRAND_DWELL_MIN: f32 = 5.0 * 60.0;
const ERRAND_DWELL_MAX: f32 = 12.0 * 60.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    AtHome,
    ToWork,
    AtWork,
    ToMiddayErrand,
    AtMiddayErrand,
    ReturningToWork,
    ToHome,
    AtHomeEvening,
    ToEveningErrand,
    AtEveningErrand,
    ReturningHome,
    DoneForDay,
}

/// Where along a trip the agent currently is. Everything besides `Road` is a
/// short synthetic leg (spec UPDATE 2.5) between a block's interior, its
/// street-edge "curb" midpoint and the corner intersection; these legs are
/// not graph edges and carry no congestion.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TripStage {
    /// Pulling out of the origin block's interior onto the curb midpoint.
    DepartDriveway,
    /// Rolling along the origin street from the curb to the first node.
    DepartCurb,
    /// Normal travel along the road network.
    Road,
    /// Rolling along the destination street from the last node to the curb.
    ArriveCurb,
    /// Brief stop at the curb before turning into the block.
    ArrivePause,
    /// Turning from the curb midpoint into the destination block's interior.
    ArriveDriveway,
}

/// How a block is entered/exited: the road node the route uses, the midpoint
/// of the block's street edge ("curb") and a point just inside the block.
#[derive(Clone, Copy)]
pub struct Access {
    pub node: u32,
    pub curb: (f32, f32),
    pub interior: (f32, f32),
}

/// An in-progress trip along a computed route.
pub struct Trip {
    /// Node sequence from origin to destination.
    pub nodes: Vec<u32>,
    /// Edge indices; edges[i] connects nodes[i] -> nodes[i+1].
    pub edges: Vec<u32>,
    /// Index of the current leg (into `edges`).
    pub leg: usize,
    /// Distance progressed along the current leg, world units.
    pub leg_progress: f32,
    /// Sim clock when the trip started (for travel-time stats).
    pub started_at: f32,
    pub stage: TripStage,
    /// Distance progressed along the current synthetic stage, world units
    /// (sim-seconds during ArrivePause); unused while `stage == Road`.
    pub stage_progress: f32,
    /// Origin curb midpoint and block-interior point.
    pub origin_curb: (f32, f32),
    pub origin_interior: (f32, f32),
    /// Distance from the origin curb to the first route node.
    pub origin_curb_len: f32,
    /// Destination curb midpoint and block-interior point.
    pub dest_curb: (f32, f32),
    pub dest_interior: (f32, f32),
    /// Distance from the last route node to the destination curb.
    pub dest_curb_len: f32,
}

pub struct Agent {
    pub home_block: u32,
    pub work_block: u32,
    pub home_node: u32,
    pub work_node: u32,
    /// Street-edge midpoint + interior point used to enter/leave home.
    pub home_curb: (f32, f32),
    pub home_interior: (f32, f32),
    /// Street-edge midpoint + interior point used to enter/leave work.
    pub work_curb: (f32, f32),
    pub work_interior: (f32, f32),
    /// Zone type of the current destination; drives vehicle color.
    pub dest_zone: Zone,
    pub depart_work_at: f32,
    pub depart_home_at: f32,
    /// Scheduled midday errand departure, consumed when taken.
    pub midday_errand_at: Option<f32>,
    /// Scheduled evening errand departure, consumed when taken.
    pub evening_errand_at: Option<f32>,
    /// How long this agent lingers at an errand destination.
    pub errand_dwell: f32,
    /// Sim-time when the current errand dwell ends.
    pub errand_until: f32,
    /// Errand destination block/access (set when an errand departs).
    pub errand_block: u32,
    pub errand_node: u32,
    pub errand_curb: (f32, f32),
    pub errand_interior: (f32, f32),
    pub phase: Phase,
    pub trip: Option<Trip>,
}

/// How far a driveway reaches into a block from its curb midpoint.
pub const DRIVEWAY_DEPTH: f32 = 20.0;

/// Pick how a block is accessed: a random surviving street edge on the
/// block's perimeter, entered at its midpoint, routed via a random endpoint
/// of that edge. Falls back to a corner node if every perimeter street was
/// removed (effectively impossible).
pub fn block_access(city: &City, graph: &Graph, rng: &mut Rng, block: u32) -> Access {
    let (r, c) = (block as usize / GRID_BLOCKS, block as usize % GRID_BLOCKS);
    let corners = block_corner_nodes(r, c); // TL, TR, BR, BL
    let sides = [
        (corners[0], corners[1]),
        (corners[1], corners[2]),
        (corners[2], corners[3]),
        (corners[3], corners[0]),
    ];
    let existing: Vec<(usize, usize)> = sides
        .into_iter()
        .filter(|&(a, b)| graph.has_edge(a as u32, b as u32))
        .collect();

    let centroid = city.block_centroid(block as usize);
    let toward = |from: (f32, f32), to: (f32, f32), dist: f32| {
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        (from.0 + dx / len * dist, from.1 + dy / len * dist)
    };

    if existing.is_empty() {
        let node = corners[rng.gen_index(4)];
        let curb = city.node_pos[node];
        return Access {
            node: node as u32,
            curb,
            interior: toward(curb, centroid, DRIVEWAY_DEPTH),
        };
    }

    let (a, b) = existing[rng.gen_index(existing.len())];
    let node = if rng.next_f32() < 0.5 { a } else { b };
    let (ax, ay) = city.node_pos[a];
    let (bx, by) = city.node_pos[b];
    let curb = ((ax + bx) / 2.0, (ay + by) / 2.0);
    Access {
        node: node as u32,
        curb,
        interior: toward(curb, centroid, DRIVEWAY_DEPTH),
    }
}

/// Sample a morning departure in the morning-peak window, clamped inside the day.
pub fn sample_morning_departure(rng: &mut Rng) -> f32 {
    (rng.normal(MORNING_PEAK, PEAK_STD) as f32).clamp(DAY_START + 5.0 * 60.0, 11.0 * 3600.0)
}

/// Sample an evening departure in the evening-peak window.
pub fn sample_evening_departure(rng: &mut Rng, after: f32) -> f32 {
    (rng.normal(EVENING_PEAK, PEAK_STD) as f32)
        .clamp(after + 30.0 * 60.0, DAY_END - 60.0 * 60.0)
}

/// Sample this agent's errand plan for the day:
/// (midday departure, evening departure, dwell duration).
pub fn sample_errands(rng: &mut Rng) -> (Option<f32>, Option<f32>, f32) {
    let midday = (rng.next_f32() < MIDDAY_ERRAND_FRACTION)
        .then(|| rng.range_f32(MIDDAY_ERRAND_START, MIDDAY_ERRAND_END));
    let evening = (rng.next_f32() < EVENING_ERRAND_FRACTION)
        .then(|| rng.range_f32(EVENING_ERRAND_START, EVENING_ERRAND_END));
    let dwell = rng.range_f32(ERRAND_DWELL_MIN, ERRAND_DWELL_MAX);
    (midday, evening, dwell)
}

/// Phase transition when a trip's arrival driveway completes.
pub fn finish_arrival(a: &mut Agent, clock: f32) {
    a.phase = match a.phase {
        Phase::ToWork | Phase::ReturningToWork => Phase::AtWork,
        Phase::ToMiddayErrand => {
            a.errand_until = clock + a.errand_dwell;
            Phase::AtMiddayErrand
        }
        Phase::ToEveningErrand => {
            a.errand_until = clock + a.errand_dwell;
            Phase::AtEveningErrand
        }
        Phase::ToHome if a.evening_errand_at.is_some() => Phase::AtHomeEvening,
        _ => Phase::DoneForDay,
    };
    a.trip = None;
}

/// Generate the resident pool. Homes come from residential blocks, workplaces
/// from commercial/industrial blocks, so home zone != work zone is guaranteed.
/// Needs the final road graph to pick surviving street edges for block access.
pub fn generate_population(city: &City, graph: &Graph, rng: &mut Rng, count: usize) -> Vec<Agent> {
    let mut residential = Vec::new();
    let mut workplaces = Vec::new();
    for (i, z) in city.block_zone.iter().enumerate() {
        match z {
            Zone::Residential => residential.push(i),
            Zone::Commercial | Zone::Industrial => workplaces.push(i),
        }
    }

    let mut agents = Vec::with_capacity(count);
    for _ in 0..count {
        let home_block = residential[rng.gen_index(residential.len())];
        let work_block = workplaces[rng.gen_index(workplaces.len())];

        let home = block_access(city, graph, rng, home_block as u32);
        let work = block_access(city, graph, rng, work_block as u32);

        let depart_work_at = sample_morning_departure(rng);
        let depart_home_at = sample_evening_departure(rng, depart_work_at);
        let (midday_errand_at, evening_errand_at, errand_dwell) = sample_errands(rng);

        agents.push(Agent {
            home_block: home_block as u32,
            work_block: work_block as u32,
            home_node: home.node,
            work_node: work.node,
            home_curb: home.curb,
            home_interior: home.interior,
            work_curb: work.curb,
            work_interior: work.interior,
            dest_zone: city.block_zone[work_block],
            depart_work_at,
            depart_home_at,
            midday_errand_at,
            evening_errand_at,
            errand_dwell,
            errand_until: 0.0,
            errand_block: work_block as u32,
            errand_node: work.node,
            errand_curb: work.curb,
            errand_interior: work.interior,
            phase: Phase::AtHome,
            trip: None,
        });
    }
    agents
}
