// Copyright (c) 2026 Felipe Madison. All rights reserved.
// Licensed under the MIT License.

use rust_sdn_p4_flow_telemetry_engine::daemon::run_server;

fn main() {
    let port = std::env::var("PORT").unwrap_or_else(|_| "8087".to_string());
    let addr = format!("127.0.0.1:{}", port);
    println!("Starting rust-sdn-p4-flow-telemetry-engine on {}", addr);
    if let Err(e) = run_server(&addr) {
        eprintln!("Daemon fatal error: {}", e);
        std::process::exit(1);
    }
}
