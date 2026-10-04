# Architecture

`epi-core` holds typed entities, compartments, and durations. `epi-graph` is the multiplex contact graph and uses petgraph for connectivity. `epi-models` compiles controls and pathogens into hazards and computes R₀. `epi-simulator` runs Gillespie and discrete-time trajectories. `epi-scenario` loads YAML and drives both the CLI and the workers.

The controller writes experiments to PostgreSQL and publishes batch messages on a NATS JetStream work queue. Workers ack a batch only after results are stored. A redelivered batch whose row is already `completed` is acknowledged and skipped. The worker that observes `runs_completed >= runs_requested` builds the aggregate report.

Event mode `summary` keeps curves and scalars. `sampled` keeps full events for every 20th run. `full` keeps events for the slice. Playback can also re-simulate one seed, because the run is deterministic.

Prometheus gauges for Rₜ and attack rate describe the latest viewed experiment, not every entity. Per-entity series would explode cardinality.
