// Copyright (c) 2026 Felipe Madison. All rights reserved.
// Licensed under the MIT License.

//! High-Performance P4 & OpenFlow Match-Action Pipeline Engine.
//! Supports Exact, Longest Prefix Match (LPM), and Ternary TCAM matching.

use std::sync::atomic::{AtomicU64, Ordering};

/// Represents an IPv4 packet header inspected by the programmable data plane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PacketHeader {
    pub ingress_port: u32,
    pub src_ip: [u8; 4],
    pub dst_ip: [u8; 4],
    pub protocol: u8, // 6 = TCP, 17 = UDP, 1 = ICMP
    pub src_port: u16,
    pub dst_port: u16,
    pub payload_len: usize,
}

impl PacketHeader {
    pub fn new(
        ingress_port: u32,
        src_ip: [u8; 4],
        dst_ip: [u8; 4],
        protocol: u8,
        src_port: u16,
        dst_port: u16,
        payload_len: usize,
    ) -> Self {
        Self {
            ingress_port,
            src_ip,
            dst_ip,
            protocol,
            src_port,
            dst_port,
            payload_len,
        }
    }

    pub fn total_bytes(&self) -> u64 {
        20 + self.payload_len as u64 // IPv4 minimum 20 bytes + payload
    }
}

/// Match criterion for table lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchField {
    ExactIngressPort(u32),
    ExactProtocol(u8),
    ExactDstPort(u16),
    LpmDstIp { ip: [u8; 4], prefix_len: u8 },
    TernaryDstPort { value: u16, mask: u16 },
}

impl MatchField {
    pub fn matches(&self, pkt: &PacketHeader) -> bool {
        match self {
            MatchField::ExactIngressPort(p) => pkt.ingress_port == *p,
            MatchField::ExactProtocol(proto) => pkt.protocol == *proto,
            MatchField::ExactDstPort(port) => pkt.dst_port == *port,
            MatchField::LpmDstIp { ip, prefix_len } => {
                let shift = 32 - prefix_len.min(&32);
                let mask = if *prefix_len == 0 {
                    0u32
                } else if *prefix_len >= 32 {
                    0xFFFFFFFFu32
                } else {
                    (!0u32) << shift
                };
                let pkt_u32 = u32::from_be_bytes(pkt.dst_ip);
                let target_u32 = u32::from_be_bytes(*ip);
                (pkt_u32 & mask) == (target_u32 & mask)
            }
            MatchField::TernaryDstPort { value, mask } => (pkt.dst_port & mask) == (value & mask),
        }
    }
}

/// Action executed when a table entry matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Forward { egress_port: u32 },
    Drop,
    MirrorAndForward { egress_port: u32, mirror_port: u32 },
    SetDstPort { new_dst_port: u16, egress_port: u32 },
}

/// Rule stored in a Match-Action table.
pub struct FlowRule {
    pub id: String,
    pub priority: u32,
    pub matches: Vec<MatchField>,
    pub action: Action,
    pub packet_counter: AtomicU64,
    pub byte_counter: AtomicU64,
}

impl FlowRule {
    pub fn new(id: impl Into<String>, priority: u32, matches: Vec<MatchField>, action: Action) -> Self {
        Self {
            id: id.into(),
            priority,
            matches,
            action,
            packet_counter: AtomicU64::new(0),
            byte_counter: AtomicU64::new(0),
        }
    }

    pub fn matches_packet(&self, pkt: &PacketHeader) -> bool {
        self.matches.iter().all(|m| m.matches(pkt))
    }

    pub fn record_hit(&self, pkt: &PacketHeader) {
        self.packet_counter.fetch_add(1, Ordering::Relaxed);
        self.byte_counter.fetch_add(pkt.total_bytes(), Ordering::Relaxed);
    }
}

/// Result of evaluating a packet through the P4 match-action pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineResult {
    pub egress_ports: Vec<u32>,
    pub dropped: bool,
    pub matched_rule: Option<String>,
    pub bytes_processed: u64,
}

/// Programmable Match-Action Pipeline Engine.
pub struct MatchActionPipeline {
    rules: Vec<FlowRule>,
}

impl Default for MatchActionPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl MatchActionPipeline {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Installs a new flow rule into the pipeline.
    /// Rules are maintained in descending priority order.
    pub fn add_rule(&mut self, rule: FlowRule) {
        self.rules.push(rule);
        self.rules.sort_by(|a, b| b.priority.cmp(&a.priority));
    }

    /// Evaluates an ingress packet against the match-action tables.
    pub fn process(&self, pkt: &PacketHeader) -> PipelineResult {
        for rule in &self.rules {
            if rule.matches_packet(pkt) {
                rule.record_hit(pkt);
                let bytes = pkt.total_bytes();
                return match &rule.action {
                    Action::Forward { egress_port } => PipelineResult {
                        egress_ports: vec![*egress_port],
                        dropped: false,
                        matched_rule: Some(rule.id.clone()),
                        bytes_processed: bytes,
                    },
                    Action::Drop => PipelineResult {
                        egress_ports: Vec::new(),
                        dropped: true,
                        matched_rule: Some(rule.id.clone()),
                        bytes_processed: bytes,
                    },
                    Action::MirrorAndForward { egress_port, mirror_port } => PipelineResult {
                        egress_ports: vec![*egress_port, *mirror_port],
                        dropped: false,
                        matched_rule: Some(rule.id.clone()),
                        bytes_processed: bytes,
                    },
                    Action::SetDstPort { egress_port, .. } => PipelineResult {
                        egress_ports: vec![*egress_port],
                        dropped: false,
                        matched_rule: Some(rule.id.clone()),
                        bytes_processed: bytes,
                    },
                };
            }
        }

        // Default pipeline behavior: Drop unmatched packets
        PipelineResult {
            egress_ports: Vec::new(),
            dropped: true,
            matched_rule: None,
            bytes_processed: pkt.total_bytes(),
        }
    }

    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    pub fn get_rule_stats(&self, rule_id: &str) -> Option<(u64, u64)> {
        self.rules
            .iter()
            .find(|r| r.id == rule_id)
            .map(|r| (r.packet_counter.load(Ordering::Relaxed), r.byte_counter.load(Ordering::Relaxed)))
    }
}
