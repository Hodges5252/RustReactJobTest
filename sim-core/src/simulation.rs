use crate::city::{self, City};
use crate::graph::Graph;
use crate::rng::Rng;

pub struct Simulation {
    pub rng: Rng,
    pub city: City,
    pub graph: Graph,
}

impl Simulation {
    pub fn new(seed: u64) -> Simulation {
        let mut rng = Rng::new(seed);
        let city = city::generate(&mut rng);
        let graph = Graph::build(&city);
        Simulation { rng, city, graph }
    }
}
