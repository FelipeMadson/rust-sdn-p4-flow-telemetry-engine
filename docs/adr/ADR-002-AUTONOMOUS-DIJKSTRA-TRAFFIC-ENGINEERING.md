# ADR-002: Autonomous Dijkstra Traffic Engineering & Dynamic Rerouting

## Context
National research backbones and multi-PoP cloud networks (such as RNP Ipê) frequently experience localized fiber cuts and sudden elephant-flow congestions. Static shortest-path routing creates hot spots and packet loss.

## Decision
We implement a dynamic Dijkstra Shortest Path First (SPF) algorithm utilizing a binary min-heap priority queue with composite edge weights. Cost is computed as:
$$\text{Cost} = \text{BaseLatency} \times (1 + 0.2 \cdot U)$$
When link utilization $U \ge 0.85$, a steep quadratic penalty is applied:
$$\text{Penalty} = (U - 0.80) \times 50.0$$
$$\text{Cost} = \text{BaseLatency} + 10.0 \times \text{Penalty}^2$$

The autonomous controller continuously observes topology state and migrates affected flows to alternate disjoint paths.

## Consequences
- **Positive**: Preempts bufferbloat and packet drops by diverting traffic before optical interfaces reach 100% capacity.
- **Positive**: Automatic sub-second failover on physical link severance.
