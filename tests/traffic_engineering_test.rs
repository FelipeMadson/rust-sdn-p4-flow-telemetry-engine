// Copyright (c) 2026 Felipe Madison. All rights reserved.
// Licensed under the MIT License.

use rust_sdn_p4_flow_telemetry_engine::traffic_engineering::{
    TopologyGraph, TrafficEngineeringController,
};

#[test]
fn test_topology_dijkstra_spf_shortest_latency() {
    let mut topo = TopologyGraph::new();
    // Triangle: DF -> SP (12ms), SP -> RJ (6ms), DF -> RJ (25ms)
    topo.add_bidirectional_link("PoP-DF", "PoP-SP", 100_000.0, 12.0);
    topo.add_bidirectional_link("PoP-SP", "PoP-RJ", 100_000.0, 6.0);
    topo.add_bidirectional_link("PoP-DF", "PoP-RJ", 40_000.0, 25.0);

    let (path, cost) = topo.compute_spf("PoP-DF", "PoP-RJ").expect("Path must exist");
    // Path through SP is 12 + 6 = 18ms, which is faster than direct 25ms
    assert_eq!(path, vec!["PoP-DF", "PoP-SP", "PoP-RJ"]);
    assert!(cost < 20.0);
}

#[test]
fn test_autonomous_rerouting_on_link_failure_and_congestion() {
    let mut topo = TopologyGraph::new();
    topo.add_bidirectional_link("Node-A", "Node-B", 10_000.0, 5.0);
    topo.add_bidirectional_link("Node-B", "Node-C", 10_000.0, 5.0);
    topo.add_bidirectional_link("Node-A", "Node-D", 10_000.0, 10.0);
    topo.add_bidirectional_link("Node-D", "Node-C", 10_000.0, 10.0);

    let mut controller = TrafficEngineeringController::new(topo);
    let initial_path = controller
        .route_flow("flow-alpha", "Node-A", "Node-C")
        .expect("Initial path must resolve");

    assert_eq!(initial_path, vec!["Node-A", "Node-B", "Node-C"]);

    // Simulate link failure on active primary path Node-A -> Node-B
    controller
        .topology
        .set_link_operational("Node-A", "Node-B", false);

    let events = controller.evaluate_and_reroute();
    assert_eq!(events.len(), 1, "Reroute event must trigger on link failure");
    assert_eq!(events[0].flow_id, "flow-alpha");
    assert_eq!(events[0].new_path, vec!["Node-A", "Node-D", "Node-C"]);
}
