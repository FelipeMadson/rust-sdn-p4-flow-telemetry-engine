// Copyright (c) 2026 Felipe Madison. All rights reserved.
// Licensed under the MIT License.

//! Embedded HTTP REST & Prometheus Daemon.
//! Operates with 100% zero external dependencies via std::net::TcpListener.

use crate::match_action::{Action, FlowRule, MatchActionPipeline, MatchField};
use crate::telemetry::TelemetryCollector;
use crate::traffic_engineering::{TopologyGraph, TrafficEngineeringController};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;

/// Server state shared across client worker threads.
pub struct DaemonState {
    pub pipeline: Mutex<MatchActionPipeline>,
    pub telemetry: TelemetryCollector,
    pub te_controller: Mutex<TrafficEngineeringController>,
}

impl DaemonState {
    pub fn new_demo_rnp_backbone() -> Self {
        let mut pipeline = MatchActionPipeline::new();
        // Rule 1: High-priority drop for DNS amplification attacks
        pipeline.add_rule(FlowRule::new(
            "r-drop-dns-amplification",
            100,
            vec![MatchField::ExactProtocol(17), MatchField::ExactDstPort(53)],
            Action::Drop,
        ));
        // Rule 2: Academic Backbone LPM Routing (RNP subnet 200.130.0.0/16)
        pipeline.add_rule(FlowRule::new(
            "r-rnp-backbone-forward",
            50,
            vec![MatchField::LpmDstIp { ip: [200, 130, 0, 0], prefix_len: 16 }],
            Action::Forward { egress_port: 10 },
        ));
        // Rule 3: Internet peering port 80/443
        pipeline.add_rule(FlowRule::new(
            "r-peering-http-https",
            10,
            vec![MatchField::ExactProtocol(6)],
            Action::Forward { egress_port: 1 },
        ));

        let telemetry = TelemetryCollector::new();

        let mut topology = TopologyGraph::new();
        // National Academic Backbone Topology (Brasília, São Paulo, Rio de Janeiro, Fortaleza, Porto Alegre)
        topology.add_bidirectional_link("PoP-DF", "PoP-SP", 100_000.0, 12.0);
        topology.add_bidirectional_link("PoP-SP", "PoP-RJ", 100_000.0, 6.0);
        topology.add_bidirectional_link("PoP-DF", "PoP-RJ", 40_000.0, 14.5);
        topology.add_bidirectional_link("PoP-DF", "PoP-CE", 100_000.0, 22.0);
        topology.add_bidirectional_link("PoP-SP", "PoP-RS", 40_000.0, 18.0);
        topology.add_bidirectional_link("PoP-RJ", "PoP-CE", 40_000.0, 26.0);

        let te_controller = Mutex::new(TrafficEngineeringController::new(topology));

        Self {
            pipeline: Mutex::new(pipeline),
            telemetry,
            te_controller,
        }
    }
}

pub fn handle_client(mut stream: TcpStream, state: Arc<DaemonState>) {
    let mut buffer = [0u8; 4096];
    let bytes_read = match stream.read(&mut buffer) {
        Ok(n) if n > 0 => n,
        _ => return,
    };

    let req_str = String::from_utf8_lossy(&buffer[..bytes_read]);
    let mut lines = req_str.lines();
    let first_line = lines.next().unwrap_or("");
    let parts: Vec<&str> = first_line.split_whitespace().collect();

    if parts.len() < 2 {
        let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\n\r\n");
        return;
    }

    let method = parts[0];
    let path = parts[1];

    if method == "GET" && path == "/health" {
        let body = r#"{"status":"UP","engine":"rust-sdn-p4-flow-telemetry-engine","version":"1.0.0"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes());
    } else if method == "GET" && path == "/metrics" {
        let body = state.telemetry.export_prometheus_metrics();
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain; version=0.0.4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes());
    } else if method == "GET" && path == "/api/v1/topology" {
        let te = state.te_controller.lock().unwrap();
        let links = te.topology.get_links();
        let mut links_json = String::from("[");
        for (i, l) in links.iter().enumerate() {
            if i > 0 {
                links_json.push(',');
            }
            links_json.push_str(&format!(
                r#"{{"from":"{}","to":"{}","capacity_mbps":{},"load_mbps":{},"latency_ms":{},"operational":{}}}"#,
                l.from, l.to, l.capacity_mbps, l.current_load_mbps, l.base_latency_ms, l.operational
            ));
        }
        links_json.push(']');

        let body = format!(
            r#"{{"node_count":{},"link_count":{},"links":{}}}"#,
            te.topology.node_count(),
            te.topology.link_count(),
            links_json
        );
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes());
    } else {
        let body = r#"{"error":"Not Found"}"#;
        let response = format!(
            "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(response.as_bytes());
    }
}

pub fn run_server(addr: &str) -> std::io::Result<()> {
    let listener = TcpListener::bind(addr)?;
    println!("[SDN Controller Daemon] Listening on http://{}", addr);

    let state = Arc::new(DaemonState::new_demo_rnp_backbone());

    for stream in listener.incoming() {
        if let Ok(stream) = stream {
            let state_clone = Arc::clone(&state);
            thread::spawn(move || {
                handle_client(stream, state_clone);
            });
        }
    }

    Ok(())
}
