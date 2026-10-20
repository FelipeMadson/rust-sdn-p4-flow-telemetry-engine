// Copyright (c) 2026 Felipe Madison. All rights reserved.
// Licensed under the MIT License.

//! Autonomous Traffic Engineering & Dijkstra Shortest Path First (SPF) Optimizer.
//! Implements load-aware dynamic rerouting, link failure failover, and composite metric path calculation.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

/// Directed network link between two SDN routers/switches.
#[derive(Debug, Clone)]
pub struct NetworkLink {
    pub from: String,
    pub to: String,
    pub capacity_mbps: f64,
    pub current_load_mbps: f64,
    pub base_latency_ms: f64,
    pub operational: bool,
}

impl NetworkLink {
    pub fn new(from: impl Into<String>, to: impl Into<String>, capacity_mbps: f64, base_latency_ms: f64) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            capacity_mbps,
            current_load_mbps: 0.0,
            base_latency_ms,
            operational: true,
        }
    }

    /// Calculates composite dynamic cost.
    /// Incorporates base propagation latency plus congestion penalty as utilization approaches 100%.
    pub fn dynamic_cost(&self) -> f64 {
        if !self.operational {
            return f64::INFINITY;
        }
        let utilization = self.current_load_mbps / self.capacity_mbps.max(1.0);
        if utilization >= 0.8 {
            // Steep quadratic penalty when link exceeds 80% capacity
            let congestion_factor = (utilization - 0.8) * 50.0;
            self.base_latency_ms + (congestion_factor * congestion_factor * 10.0)
        } else {
            self.base_latency_ms * (1.0 + (utilization * 0.2))
        }
    }

    pub fn utilization_ratio(&self) -> f64 {
        self.current_load_mbps / self.capacity_mbps.max(1.0)
    }
}

/// Helper struct for min-heap priority queue in Dijkstra.
#[derive(Copy, Clone, PartialEq)]
struct State {
    cost: f64,
    node_idx: usize,
}

impl Eq for State {}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        other.cost.partial_cmp(&self.cost) // Reverse for min-heap
    }
}

impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Equal)
    }
}

/// Topology graph modeling national/regional academic backbone networks (e.g. RNP Ipê).
#[derive(Default, Clone)]
pub struct TopologyGraph {
    nodes: Vec<String>,
    node_indices: HashMap<String, usize>,
    links: Vec<NetworkLink>,
}

impl TopologyGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_node(&mut self, node: impl Into<String>) {
        let name = node.into();
        if !self.node_indices.contains_key(&name) {
            let idx = self.nodes.len();
            self.nodes.push(name.clone());
            self.node_indices.insert(name, idx);
        }
    }

    pub fn add_bidirectional_link(&mut self, u: &str, v: &str, capacity_mbps: f64, base_latency_ms: f64) {
        self.add_node(u);
        self.add_node(v);
        self.links.push(NetworkLink::new(u, v, capacity_mbps, base_latency_ms));
        self.links.push(NetworkLink::new(v, u, capacity_mbps, base_latency_ms));
    }

    pub fn set_link_operational(&mut self, from: &str, to: &str, operational: bool) -> bool {
        let mut modified = false;
        for link in &mut self.links {
            if link.from == from && link.to == to {
                link.operational = operational;
                modified = true;
            }
        }
        modified
    }

    pub fn set_link_load(&mut self, from: &str, to: &str, load_mbps: f64) -> bool {
        let mut modified = false;
        for link in &mut self.links {
            if link.from == from && link.to == to {
                link.current_load_mbps = load_mbps;
                modified = true;
            }
        }
        modified
    }

    /// Computes load-aware Shortest Path First (SPF) using Dijkstra algorithm.
    pub fn compute_spf(&self, start: &str, target: &str) -> Option<(Vec<String>, f64)> {
        let start_idx = *self.node_indices.get(start)?;
        let target_idx = *self.node_indices.get(target)?;

        let n = self.nodes.len();
        let mut dist: Vec<f64> = vec![f64::INFINITY; n];
        let mut prev: Vec<Option<usize>> = vec![None; n];
        let mut heap = BinaryHeap::new();

        dist[start_idx] = 0.0;
        heap.push(State { cost: 0.0, node_idx: start_idx });

        while let Some(State { cost, node_idx }) = heap.pop() {
            if node_idx == target_idx {
                // Target reached, reconstruct path
                let mut path = Vec::new();
                let mut curr = Some(target_idx);
                while let Some(idx) = curr {
                    path.push(self.nodes[idx].clone());
                    curr = prev[idx];
                }
                path.reverse();
                return Some((path, cost));
            }

            if cost > dist[node_idx] {
                continue;
            }

            let curr_node_name = &self.nodes[node_idx];
            for link in &self.links {
                if &link.from == curr_node_name && link.operational {
                    if let Some(&neighbor_idx) = self.node_indices.get(&link.to) {
                        let link_cost = link.dynamic_cost();
                        if link_cost.is_finite() {
                            let next_cost = cost + link_cost;
                            if next_cost < dist[neighbor_idx] {
                                dist[neighbor_idx] = next_cost;
                                prev[neighbor_idx] = Some(node_idx);
                                heap.push(State { cost: next_cost, node_idx: neighbor_idx });
                            }
                        }
                    }
                }
            }
        }

        None
    }

    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn link_count(&self) -> usize {
        self.links.len()
    }

    pub fn get_links(&self) -> &[NetworkLink] {
        &self.links
    }
}

/// Notification emitted when an autonomous traffic steering event occurs.
#[derive(Debug, Clone, PartialEq)]
pub struct RerouteEvent {
    pub flow_id: String,
    pub original_path: Vec<String>,
    pub new_path: Vec<String>,
    pub previous_cost: f64,
    pub new_cost: f64,
    pub reason: String,
}

/// Autonomous Traffic Steering Controller.
pub struct TrafficEngineeringController {
    pub topology: TopologyGraph,
    active_paths: HashMap<String, Vec<String>>,
}

impl TrafficEngineeringController {
    pub fn new(topology: TopologyGraph) -> Self {
        Self {
            topology,
            active_paths: HashMap::new(),
        }
    }

    /// Provisions or recalculates the optimal path for a flow.
    pub fn route_flow(&mut self, flow_id: &str, src_node: &str, dst_node: &str) -> Option<Vec<String>> {
        let (path, _) = self.topology.compute_spf(src_node, dst_node)?;
        self.active_paths.insert(flow_id.to_string(), path.clone());
        Some(path)
    }

    /// Evaluates all active flows against current topology conditions (link health and congestion).
    /// If an active path has degraded or broken, reroutes to an alternate optimal path.
    pub fn evaluate_and_reroute(&mut self) -> Vec<RerouteEvent> {
        let mut events = Vec::new();
        let flows: Vec<(String, Vec<String>)> = self.active_paths.iter().map(|(k, v)| (k.clone(), v.clone())).collect();

        for (flow_id, old_path) in flows {
            if old_path.len() < 2 {
                continue;
            }
            let src = &old_path[0];
            let dst = &old_path[old_path.len() - 1];

            // Check if current path has failed links or severe congestion
            let mut path_failed = false;
            let mut severe_congestion = false;

            for window in old_path.windows(2) {
                let u = &window[0];
                let v = &window[1];
                for link in self.topology.get_links() {
                    if &link.from == u && &link.to == v {
                        if !link.operational {
                            path_failed = true;
                        } else if link.utilization_ratio() >= 0.85 {
                            severe_congestion = true;
                        }
                    }
                }
            }

            if path_failed || severe_congestion {
                if let Some((new_path, new_cost)) = self.topology.compute_spf(src, dst) {
                    if new_path != old_path {
                        let reason = if path_failed {
                            "Link failure detected in active path".to_string()
                        } else {
                            "Link utilization exceeded 85% threshold (congestion avoidance)".to_string()
                        };

                        events.push(RerouteEvent {
                            flow_id: flow_id.clone(),
                            original_path: old_path,
                            new_path: new_path.clone(),
                            previous_cost: 0.0,
                            new_cost,
                            reason,
                        });

                        self.active_paths.insert(flow_id, new_path);
                    }
                }
            }
        }

        events
    }
}
