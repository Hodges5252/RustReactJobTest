use crate::graph::Graph;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// Min-heap entry; ordering reversed on cost.
struct QueueItem {
    cost: f32,
    node: u32,
}

impl PartialEq for QueueItem {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost
    }
}
impl Eq for QueueItem {}
impl PartialOrd for QueueItem {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for QueueItem {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse for min-heap; costs are finite non-NaN by construction.
        other
            .cost
            .partial_cmp(&self.cost)
            .unwrap_or(Ordering::Equal)
    }
}

/// Dijkstra over current edge weights (travel time under congestion).
/// Returns (nodes, edges) where `edges[i]` connects `nodes[i]` -> `nodes[i+1]`.
pub fn shortest_path(graph: &Graph, start: u32, goal: u32) -> Option<(Vec<u32>, Vec<u32>)> {
    if start == goal {
        return Some((vec![start], Vec::new()));
    }

    let n = graph.node_count;
    let mut dist = vec![f32::INFINITY; n];
    let mut prev: Vec<Option<(u32, u32)>> = vec![None; n]; // (prev node, edge used)
    let mut heap = BinaryHeap::new();

    dist[start as usize] = 0.0;
    heap.push(QueueItem {
        cost: 0.0,
        node: start,
    });

    while let Some(QueueItem { cost, node }) = heap.pop() {
        if node == goal {
            break;
        }
        if cost > dist[node as usize] {
            continue;
        }
        for &(edge_idx, neighbor) in &graph.adjacency[node as usize] {
            let next = cost + graph.edges[edge_idx as usize].travel_time();
            if next < dist[neighbor as usize] {
                dist[neighbor as usize] = next;
                prev[neighbor as usize] = Some((node, edge_idx));
                heap.push(QueueItem {
                    cost: next,
                    node: neighbor,
                });
            }
        }
    }

    if dist[goal as usize].is_infinite() {
        return None;
    }

    let mut nodes = vec![goal];
    let mut edges = Vec::new();
    let mut cur = goal;
    while cur != start {
        let (p, e) = prev[cur as usize]?;
        edges.push(e);
        nodes.push(p);
        cur = p;
    }
    nodes.reverse();
    edges.reverse();
    Some((nodes, edges))
}
