# Defender policies

```rust
trait DefenderPolicy {
    fn observe(&self, observation: &Observation) -> Decision;
}
```

Decisions: isolate, increase monitoring, revoke an identity-like entity, segment a node off the transmitting graph, prioritise patching, investigate, or do nothing.

Shipped policies are deterministic:

- `NoResponsePolicy` leaves the stochastic clocks alone
- `ImmediateIsolationPolicy` quarantines an entity when it becomes detected
- `RiskBasedIsolationPolicy` does that for high criticality or high privilege
- `CriticalAssetProtectionPolicy` starts high-criticality entities in the protected compartment

A later rules engine, a human playbook encoded as the same trait, or a learned policy can replace the enum without changing the engine. The research question is which policy, combined with which controls, keeps a compromise from becoming an organisational epidemic. The learned policy would still be scored inside the simulator.
