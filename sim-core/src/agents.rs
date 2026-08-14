use crate::city::{block_corner_nodes, City, Zone, GRID_BLOCKS};
use crate::rng::Rng;

pub const DAY_START: f32 = 6.0 * 3600.0; // 6:00 AM in seconds since midnight
pub const DAY_END: f32 = 23.0 * 3600.0; // 11:00 PM

const MORNING_PEAK: f64 = 7.5 * 3600.0; // ~7:30 AM
const EVENING_PEAK: f64 = 17.5 * 3600.0; // ~5:30 PM
const PEAK_STD: f64 = 45.0 * 60.0; // 45 minute spread

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    AtHome,
    ToWork,
    AtWork,
    ToHome,
    DoneForDay,
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
}

pub struct Agent {
    pub home_block: u32,
    pub work_block: u32,
    pub home_node: u32,
    pub work_node: u32,
    /// Zone type of the current destination; drives vehicle color.
    pub dest_zone: Zone,
    pub depart_work_at: f32,
    pub depart_home_at: f32,
    pub phase: Phase,
    pub trip: Option<Trip>,
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

/// Generate the resident pool. Homes come from residential blocks, workplaces
/// from commercial/industrial blocks, so home zone != work zone is guaranteed.
pub fn generate_population(city: &City, rng: &mut Rng, count: usize) -> Vec<Agent> {
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

        let home_corners = block_corner_nodes(home_block / GRID_BLOCKS, home_block % GRID_BLOCKS);
        let work_corners = block_corner_nodes(work_block / GRID_BLOCKS, work_block % GRID_BLOCKS);
        let home_node = home_corners[rng.gen_index(4)] as u32;
        let work_node = work_corners[rng.gen_index(4)] as u32;

        let depart_work_at = sample_morning_departure(rng);
        let depart_home_at = sample_evening_departure(rng, depart_work_at);

        agents.push(Agent {
            home_block: home_block as u32,
            work_block: work_block as u32,
            home_node,
            work_node,
            dest_zone: city.block_zone[work_block],
            depart_work_at,
            depart_home_at,
            phase: Phase::AtHome,
            trip: None,
        });
    }
    agents
}
