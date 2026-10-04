# State models

SEIDQRP compartments:

| State | Cyber reading |
| --- | --- |
| Susceptible | Reachable, not yet exposed |
| Exposed | Exposed, not yet propagating |
| Infectious | Compromised and able to propagate |
| Detected | Noticed, still able to propagate until isolation |
| Quarantined | Removed from the contact process |
| Recovered | Restored, with waning immunity if configured |
| Protected | Patched or otherwise immune for a time |

Presets collapse that chain:

- SIS: susceptible ↔ infectious
- SIR: susceptible → infectious → recovered
- SEIR: susceptible → exposed → infectious → recovered
- SEIRS: SEIR plus return to susceptible
- SEIDQRP: the full chain, including protection

For SEIDQRP the mean time spent infectious is the detection delay. The exit splits between detection and undetected recovery in proportion to the pathogen's detection and recovery weights, adjusted by the entity's sensor probability and any EDR multiplier. Mean time detected is the isolation delay. Mean time quarantined is the recovery time. These are competing exponential clocks so the Gillespie algorithm and the analytical R₀ use the same rates.

Discrete mode is a synchronous approximation of those hazards: `p = 1 - exp(-λ Δt)` on a snapshot of the state, default step 15 minutes.

Seeding places the index cases directly into the infectious compartment. They are already compromises, not merely exposed.
