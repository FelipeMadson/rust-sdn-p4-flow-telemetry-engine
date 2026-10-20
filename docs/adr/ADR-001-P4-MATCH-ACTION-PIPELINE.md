# ADR-001: P4 Match-Action Pipeline Architecture

## Context
High-speed programmable networks require packet classification engines capable of handling Exact, Longest Prefix Match (LPM), and Ternary TCAM wildcard lookups with sub-microsecond latency.

## Decision
We implement a zero-dependency Match-Action pipeline in pure safe Rust 2021. Flow rules are evaluated in descending priority order. Counter updates (`packet_counter` and `byte_counter`) leverage atomic hardware instructions (`AtomicU64`) with relaxed ordering to prevent lock contention on high-throughput multi-core worker threads.

## Consequences
- **Positive**: Sub-microsecond processing without external runtime libraries or unsafe pointer manipulations.
- **Positive**: Thread-safe telemetry collection with zero contention.
- **Negative**: Linear rule evaluation over priority vector; for millions of rules, radix tree hardware acceleration should be paired with FPGA/TCAM offloads.
