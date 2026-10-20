// Copyright (c) 2026 Felipe Madison. All rights reserved.
// Licensed under the MIT License.

use rust_sdn_p4_flow_telemetry_engine::match_action::{
    Action, FlowRule, MatchActionPipeline, MatchField, PacketHeader,
};
use rust_sdn_p4_flow_telemetry_engine::telemetry::{FlowKey, TelemetryCollector};

#[test]
fn test_p4_match_action_lpm_and_exact() {
    let mut pipeline = MatchActionPipeline::new();

    // High priority: Drop UDP 53 DNS flood
    pipeline.add_rule(FlowRule::new(
        "drop-dns",
        100,
        vec![MatchField::ExactProtocol(17), MatchField::ExactDstPort(53)],
        Action::Drop,
    ));

    // Medium priority: Forward subnet 10.50.0.0/16 to port 4
    pipeline.add_rule(FlowRule::new(
        "lpm-subnet-10-50",
        50,
        vec![MatchField::LpmDstIp { ip: [10, 50, 0, 0], prefix_len: 16 }],
        Action::Forward { egress_port: 4 },
    ));

    // Low priority: Default forward HTTP to port 1
    pipeline.add_rule(FlowRule::new(
        "exact-http",
        10,
        vec![MatchField::ExactDstPort(80)],
        Action::Forward { egress_port: 1 },
    ));

    // Test 1: Packet matching high-priority DNS drop
    let dns_pkt = PacketHeader::new(1, [192, 168, 1, 100], [8, 8, 8, 8], 17, 49152, 53, 64);
    let res1 = pipeline.process(&dns_pkt);
    assert!(res1.dropped, "DNS packet must be dropped by security rule");
    assert_eq!(res1.matched_rule, Some("drop-dns".to_string()));

    // Test 2: Packet matching LPM subnet 10.50.12.34
    let lpm_pkt = PacketHeader::new(1, [192, 168, 1, 100], [10, 50, 12, 34], 6, 49152, 443, 512);
    let res2 = pipeline.process(&lpm_pkt);
    assert!(!res2.dropped);
    assert_eq!(res2.egress_ports, vec![4]);
    assert_eq!(res2.matched_rule, Some("lpm-subnet-10-50".to_string()));

    // Test 3: Packet matching default HTTP
    let http_pkt = PacketHeader::new(1, [192, 168, 1, 100], [172, 16, 0, 1], 6, 49152, 80, 256);
    let res3 = pipeline.process(&http_pkt);
    assert!(!res3.dropped);
    assert_eq!(res3.egress_ports, vec![1]);
    assert_eq!(res3.matched_rule, Some("exact-http".to_string()));

    // Test 4: Unmatched packet drops by default
    let unknown_pkt = PacketHeader::new(1, [192, 168, 1, 100], [172, 16, 0, 1], 6, 49152, 8080, 128);
    let res4 = pipeline.process(&unknown_pkt);
    assert!(res4.dropped);
    assert_eq!(res4.matched_rule, None);
}

#[test]
fn test_telemetry_flow_aggregation_and_prometheus_export() {
    let collector = TelemetryCollector::new();
    let flow = FlowKey::new([10, 0, 0, 1], [10, 0, 0, 2], 44321, 80, 6);

    collector.record_packet(&flow, 1500, 120.0);
    collector.record_packet(&flow, 1500, 100.0);
    collector.record_packet(&flow, 1500, 80.0);

    assert_eq!(collector.active_flow_count(), 1);
    let metrics = collector.get_metrics(&flow).expect("Flow must exist");
    assert_eq!(metrics.packets, 3);
    assert_eq!(metrics.bytes, 4500);

    let prom = collector.export_prometheus_metrics();
    assert!(prom.contains("sdn_telemetry_total_packets 3"));
    assert!(prom.contains("sdn_telemetry_total_bytes 4500"));
    assert!(prom.contains("sdn_telemetry_active_flows 1"));
    assert!(prom.contains("proto=\"6\""));
}
