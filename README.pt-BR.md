# rust-sdn-p4-flow-telemetry-engine

[![CI](https://github.com/FelipeMadson/rust-sdn-p4-flow-telemetry-engine/actions/workflows/ci.yml/badge.svg)](https://github.com/FelipeMadson/rust-sdn-p4-flow-telemetry-engine/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust: 2021](https://img.shields.io/badge/Rust-2021-orange.svg)](https://www.rust-lang.org/)
[![Zero Dependencies](https://img.shields.io/badge/Dependencies-Zero-brightgreen.svg)](Cargo.toml)

> Motor de Pipeline Match-Action P4 & OpenFlow de Alta Performance, Engenharia de Tráfego Autônoma com Dijkstra e Coletor de Telemetria Line-Rate implementado em Rust 100% puro e seguro sem dependências externas.

Desenvolvido por **[Felipe Madison](https://github.com/FelipeMadson)** como implementação de referência para backbones acadêmicos nacionais (como o Ipê da RNP), redes em escala de nuvem e planos de dados programáveis.

---

## 🚀 Destaques Arquiteturais

1. **Pipeline Match-Action P4 & OpenFlow Programável**:
   - Suporte a **Exact Match** (protocolo L4, portas), **Longest Prefix Match (LPM)** com máscara de bits em sub-redes IPv4 e **Curingas Ternários TCAM**.
   - Ordenação estrita por prioridade decrescente com contadores atômicos de pacotes e bytes (`AtomicU64`).
   - Ações integradas: `Forward`, `Drop`, `MirrorAndForward` e `SetDstPort`.

2. **Engenharia de Tráfego Autônoma com Dijkstra & Reroteamento Dinâmico**:
   - Modela topologias ópticas/SDN nacionais (ex: PoPs da RNP: DF, SP, RJ, CE, RS).
   - Calcula **Shortest Path First (SPF)** com custo composto considerando latência base e penalidade quadrática de saturação de enlaces.
   - Detecta falhas de enlace e desvia fluxos automaticamente quando a utilização supera 85%.

3. **Contabilidade de Telemetria Line-Rate**:
   - Rastreamento em tempo real da 5-tupla (`src_ip`, `dst_ip`, `src_port`, `dst_port`, `protocol`).
   - Contabilização de vazão, pacotes e latência média móvel exponencial (EWMA).
   - Exportação nativa para Prometheus no endpoint `/metrics`.

4. **Rust Seguro com Zero Crates Externas**:
   - Zero dependências de terceiros, garantindo imunidade a ataques na cadeia de suprimentos e compilação em menos de 1 segundo.
   - Daemon HTTP REST embutido utilizando `std::net::TcpListener`.

---

## 🛠️ Verificação & Bateria de Testes

Execute a suíte de testes com Cargo:

```bash
cargo test --verbose
```

### Testes Aprovados

```text
running 2 tests
test test_p4_match_action_lpm_and_exact ... ok
test test_telemetry_flow_aggregation_and_prometheus_export ... ok

running 2 tests
test test_topology_dijkstra_spf_shortest_latency ... ok
test test_autonomous_rerouting_on_link_failure_and_congestion ... ok
```

---

## 📜 Licença

Licença MIT. Arquitetado e mantido por [Felipe Madison](https://github.com/FelipeMadson).
