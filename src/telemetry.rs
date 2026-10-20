// Copyright (c) 2026 Felipe Madison. All rights reserved.
// Licensed under the MIT License.

//! High-Speed IPFIX & In-Band Network Telemetry (INT) Flow Collector.
//! Aggregates line-rate counters and exposes standard Prometheus metrics.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// 5-tuple defining a network communication flow.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FlowKey {
    pub src_ip: [u8; 4],
    pub dst_ip: [u8; 4],
    pub src_port: u16,
    pub dst_port: u16,
    pub protocol: u8,
}

impl FlowKey {
    pub fn new(src_ip: [u8; 4], dst_ip: [u8; 4], src_port: u16, dst_port: u16, protocol: u8) -> Self {
        Self {
            src_ip,
            dst_ip,
            src_port,
            dst_port,
            protocol,
        }
    }

    pub fn to_string_repr(&self) -> String {
        format!(
            "{}.{}.{}.{}:{} -> {}.{}.{}.{}:{} (proto={})",
            self.src_ip[0],
            self.src_ip[1],
            self.src_ip[2],
            self.src_ip[3],
            self.src_port,
            self.dst_ip[0],
            self.dst_ip[1],
            self.dst_ip[2],
            self.dst_ip[3],
            self.dst_port,
            self.protocol
        )
    }
}

/// Dynamic metrics collected per active flow.
#[derive(Debug, Clone)]
pub struct FlowMetrics {
    pub packets: u64,
    pub bytes: u64,
    pub drops: u64,
    pub first_seen_ms: u64,
    pub last_seen_ms: u64,
    pub ewma_latency_us: f64,
}

impl FlowMetrics {
    pub fn new(initial_bytes: u64, initial_latency_us: f64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        Self {
            packets: 1,
            bytes: initial_bytes,
            drops: 0,
            first_seen_ms: now,
            last_seen_ms: now,
            ewma_latency_us: initial_latency_us,
        }
    }

    pub fn update(&mut self, bytes: u64, latency_us: f64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        self.packets += 1;
        self.bytes += bytes;
        self.last_seen_ms = now;
        // Exponential Weighted Moving Average (alpha = 0.2)
        self.ewma_latency_us = (0.2 * latency_us) + (0.8 * self.ewma_latency_us);
    }

    pub fn record_drop(&mut self) {
        self.drops += 1;
    }
}

/// High-throughput Telemetry Collector with thread-safe flow tables.
#[derive(Clone)]
pub struct TelemetryCollector {
    flows: Arc<RwLock<HashMap<FlowKey, FlowMetrics>>>,
    total_packets_processed: Arc<RwLock<u64>>,
    total_bytes_processed: Arc<RwLock<u64>>,
}

impl Default for TelemetryCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl TelemetryCollector {
    pub fn new() -> Self {
        Self {
            flows: Arc::new(RwLock::new(HashMap::new())),
            total_packets_processed: Arc::new(RwLock::new(0)),
            total_bytes_processed: Arc::new(RwLock::new(0)),
        }
    }

    /// Records an observed packet in the flow table.
    pub fn record_packet(&self, key: &FlowKey, bytes: u64, latency_us: f64) {
        let mut flows = self.flows.write().unwrap();
        flows
            .entry(key.clone())
            .and_modify(|m| m.update(bytes, latency_us))
            .or_insert_with(|| FlowMetrics::new(bytes, latency_us));

        let mut total_pkts = self.total_packets_processed.write().unwrap();
        *total_pkts += 1;

        let mut total_bytes = self.total_bytes_processed.write().unwrap();
        *total_bytes += bytes;
    }

    /// Records a packet drop for a specific flow.
    pub fn record_drop(&self, key: &FlowKey) {
        let mut flows = self.flows.write().unwrap();
        if let Some(metrics) = flows.get_mut(key) {
            metrics.record_drop();
        }
    }

    pub fn active_flow_count(&self) -> usize {
        self.flows.read().unwrap().len()
    }

    pub fn get_metrics(&self, key: &FlowKey) -> Option<FlowMetrics> {
        self.flows.read().unwrap().get(key).cloned()
    }

    pub fn get_all_flows(&self) -> HashMap<FlowKey, FlowMetrics> {
        self.flows.read().unwrap().clone()
    }

    /// Generates standard Prometheus exposition format.
    pub fn export_prometheus_metrics(&self) -> String {
        let flows = self.flows.read().unwrap();
        let total_pkts = *self.total_packets_processed.read().unwrap();
        let total_bytes = *self.total_bytes_processed.read().unwrap();

        let mut out = String::new();
        out.push_str("# HELP sdn_telemetry_total_packets Total packets processed by the SDN pipeline\n");
        out.push_str("# TYPE sdn_telemetry_total_packets counter\n");
        out.push_str(&format!("sdn_telemetry_total_packets {}\n", total_pkts));

        out.push_str("# HELP sdn_telemetry_total_bytes Total payload bytes processed\n");
        out.push_str("# TYPE sdn_telemetry_total_bytes counter\n");
        out.push_str(&format!("sdn_telemetry_total_bytes {}\n", total_bytes));

        out.push_str("# HELP sdn_telemetry_active_flows Current active concurrent network flows\n");
        out.push_str("# TYPE sdn_telemetry_active_flows gauge\n");
        out.push_str(&format!("sdn_telemetry_active_flows {}\n", flows.len()));

        for (k, m) in flows.iter() {
            let label = format!(
                "src=\"{}.{}.{}.{}:{}\",dst=\"{}.{}.{}.{}:{}\",proto=\"{}\"",
                k.src_ip[0], k.src_ip[1], k.src_ip[2], k.src_ip[3], k.src_port,
                k.dst_ip[0], k.dst_ip[1], k.dst_ip[2], k.dst_ip[3], k.dst_port,
                k.protocol
            );
            out.push_str(&format!("sdn_flow_packets{{{}}} {}\n", label, m.packets));
            out.push_str(&format!("sdn_flow_bytes{{{}}} {}\n", label, m.bytes));
            out.push_str(&format!("sdn_flow_latency_us{{{}}} {:.2}\n", label, m.ewma_latency_us));
        }

        out
    }
}
