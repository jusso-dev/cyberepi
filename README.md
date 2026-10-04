# CyberEpi

CyberEpi is a digital outbreak laboratory. It models how a compromise can move through an organisation the way an epidemic moves through a population: exposure, infectiousness, detection, quarantine, recovery, and the loss of protection over time.

It is a **simulation platform**. It does not ship malware, exploit code, scanners, or lateral-movement tools. Transmission is a probability on a contact graph. Nothing in this repository attacks a real network.

Researchers, defenders, students, and homelab operators can ask:

- How quickly can compromise spread through an organisation?
- What is the cyber equivalent of R₀ and Rₜ?
- Which identities or services behave like superspreaders?
- How do MFA, EDR, patching, segmentation, and faster isolation change the outcome?
- What does the distribution look like across thousands of stochastic runs?
- Which combination of controls pushes Rₜ below 1?

## Why cyber epidemiology

A single infected laptop is a case. A shared automation identity that can reach hundreds of systems is closer to a superspreader. Coverage that is only partial is the same problem as an incomplete vaccination campaign: the average looks fine while a connected pocket can still sustain spread.

R₀ here is the average number of secondary compromises one compromised entity would cause in an otherwise susceptible population. Rₜ is that number given who is still susceptible. When Rₜ is above 1 the simulated outbreak is expanding. When it is below 1, it is contracting.

Built-in numbers are **illustrative simulation assumptions**. They are not measurements of any product. Override them.

## Architecture

```text
Next.js laboratory
        │  REST + WebSocket
        ▼
Rust controller ── PostgreSQL
        │
        ▼
NATS JetStream
        │
        ▼
Rust workers ── same simulation engine as the CLI
        │
        ▼
Prometheus → Grafana
```

The k3s cluster that runs CyberEpi is not the population inside a scenario. A pod is not a simulated host unless you explicitly import cluster **metadata** and then simulate on that graph.

Homelab shape: 1–3 k3s nodes, about 4–8 cores and 8–16 GB RAM each, local-path storage. No Kafka, Elasticsearch, Ceph, Longhorn, service mesh, or paid API.

## Quickstart

Local laboratory:

```bash
docker compose up --build
```

Open <http://localhost>. The development login is `admin` / `cyberepi-dev` unless you override `.env`. Change that password before the port is reachable by anyone else.

First simulation on your own machine, without Kubernetes:

```bash
cargo run --release -p cyberepi -- simulate scenarios/demos/basic-outbreak.yaml
```

`basic-outbreak.yaml` is a 10,000-entity medium enterprise, IdentityStealer, 1,000 Gillespie runs, 7 days. A smaller smoke scenario is `scenarios/demos/smoke.yaml`.

One release run of that scenario produced:

```text
Population:             10,000
Runs:                   1,000
R0:                     1.68
Invasion threshold:     551.93
Median peak prevalence: 1,609
Median attack rate:     27.0%
Median outbreak size:   2,700
Extinction probability: 24.8%
Rt dropped below 1:     32h 0m
```

The invasion threshold is far above the mean because the identity provider is a hub: most index cases cause modest spread, and a minority reach the hub and grow. That gap is the superspreader effect, not a second definition of the same average. Re-running the file will match these numbers on this version; changing a rate on purpose will not, which is what the scenario hash is for.

## Install on k3s

```bash
helm install cyberepi ./charts/cyberepi \
  --namespace cyberepi \
  --create-namespace
```

```bash
kubectl get secret cyberepi-auth -n cyberepi -o jsonpath='{.data.password}' | base64 -d; echo
```

Point `cyberepi.local` at the cluster or set `ingress.host`. Worker count:

```bash
helm upgrade cyberepi ./charts/cyberepi -n cyberepi --set worker.replicas=3
```

HPA is off by default so a homelab without metrics-server still runs. Turn it on with `--set hpa.enabled=true` when CPU metrics exist.

Images are published as `ghcr.io/jusso-dev/cyberepi` and `ghcr.io/jusso-dev/cyberepi-web` for amd64 and arm64. For a private registry or a local `k3s ctr images import`, see `deployments/k3s/README.md`.

## Scenario format

```yaml
apiVersion: cyberepi.io/v1
kind: Scenario
metadata:
  name: mfa-effectiveness-demo
population:
  template: medium-enterprise
  size: 5000
pathogen:
  type: IdentityStealer
initial_conditions:
  infected_entities: 1
controls:
  mfa:
    coverage: 0.80
  edr:
    coverage: 0.90
simulation:
  algorithm: gillespie
  duration: 7d
  runs: 1000
  seed: 12345
```

Validate with `cyberepi scenario validate scenario.yaml`. The JSON Schema is `schemas/scenario.schema.json`.

Templates: micro business, small business, medium enterprise, large enterprise, government agency, cloud-native startup, hybrid enterprise, software company, university, managed service provider.

Pathogen profiles: RapidWorm, IdentityStealer, EmailBorne, CloudControlPlane, SupplyChain, SlowPersistent, RansomwareLike, InsiderLike.

Presets: SIS, SIR, SEIR, SEIRS, and the full SEIDQRP chain (susceptible, exposed, compromised, detected, quarantined, recovered, protected).

## CLI

```bash
cyberepi scenario validate scenarios/demos/basic-outbreak.yaml
cyberepi simulate scenarios/demos/small-business-mfa.yaml
cyberepi experiment run scenarios/demos/superspreader.yaml --out experiment.json
cyberepi experiment results experiment.json
cyberepi generate organisation --template medium-enterprise --users 5000
cyberepi cluster status
```

`simulate` does not need Postgres, NATS, or Kubernetes. The same engine runs inside workers when you start an experiment from the API.

## Research methodology

Each experiment records CyberEpi version, scenario hash, seed, algorithm, pathogen, controls, generator version, and time. The same scenario and seed replay the same run on the same build.

Two reproduction numbers are reported:

- **R₀** is the mean individual reproduction number. That matches the plain-language definition above.
- **Invasion threshold** is the leading eigenvalue of the next-generation operator. A large gap between them means the contact graph is heterogeneous: most entities would cause few secondary cases, while a few hubs can amplify a spark.

Extinction means the cumulative number of compromised entities stayed under `max(10, 0.5% of the population)` unless you set another fraction.

Monte Carlo output includes median and upper-tail outbreak size, peak prevalence, attack rate, containment time, and the time Rₜ last fell below 1. Full event traces are optional (`event_mode: full` or `sampled`) so a 10,000-run experiment does not have to store every transition.

Details: `docs/modelling/` and `docs/methodology/`.

## Safety

CyberEpi does not execute vulnerabilities, steal credentials, persist on hosts, move laterally, or destroy data. Defensive controls only change rates. Kubernetes import is read-only, and secret objects are requested as partial metadata so values are not part of the client schema. See [SAFETY.md](SAFETY.md).

## Observability

Workers and the controller expose Prometheus metrics such as `cyberepi_simulations_total`, `cyberepi_queue_depth`, `cyberepi_rt`, and `cyberepi_attack_rate`. They are not labelled per entity. Grafana dashboards in `grafana/dashboards/` cover cluster overview, worker performance, throughput, experiments, and epidemiology. Compose profile `observe` starts Prometheus and Grafana:

```bash
docker compose --profile observe up
```

## Contributing

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd web && npm ci && npm run lint && npm run typecheck && npm test
helm lint charts/cyberepi
```

See [CONTRIBUTING.md](CONTRIBUTING.md).

## Roadmap

- OIDC for Authentik, Keycloak, Entra ID, Google Workspace, and Cloudflare Access, still optional
- PDF reports
- Pluggable defender policies beyond the deterministic ones shipped now (`NoResponse`, `ImmediateIsolation`, `RiskBasedIsolation`, `CriticalAssetProtection`)
- Deeper playback in the laboratory UI

The trait those future policies implement is `DefenderPolicy` in `epi-models`. An eventual learning agent would observe the same state and return the same kinds of decisions: isolate, monitor, revoke, segment, patch, investigate, or do nothing. It would still be acting on the simulation, not on your cluster.
