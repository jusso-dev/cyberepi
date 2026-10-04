# CyberEpi

CyberEpi is a digital outbreak laboratory. It lets you ask epidemiological questions about a **synthetic** organisation: how a compromise spreads, which identities behave like superspreaders, and which defensive controls shrink the outbreak.

It is a simulation. Transmission is a probability on a contact graph. CyberEpi does not scan networks, run exploits, steal credentials, move laterally, or change real systems. See [SAFETY.md](SAFETY.md).

## What you can learn with it

The laboratory is built for researchers, defenders, students, and homelab operators who want to reason about compromise the way an epidemiologist reasons about infection.

A laptop is a case. A shared automation identity that can reach hundreds of systems is closer to a superspreader. MFA on 80% of accounts is closer to an incomplete vaccination campaign: the average looks reassuring while a connected pocket can still sustain spread.

Questions the model is meant to answer:

- How quickly can compromise move through an organisation?
- What is the cyber equivalent of R₀ and Rₜ, and when does Rₜ fall below 1?
- Which identities, services, or workloads contribute disproportionate spread?
- How do MFA, EDR, patching, segmentation, isolation time, and similar controls change outbreak size?
- What happens when coverage is incomplete?
- What does the distribution of outcomes look like across hundreds or thousands of stochastic runs?
- How does the same population respond to different synthetic pathogen profiles?

The controls change rates. A profile such as `IdentityStealer` or `RapidWorm` is a bundle of those rates and channel weights. It is not an exploit and it does not describe a procedure.

Built-in numbers are **illustrative simulation assumptions**. They are not measurements of any vendor, control, or organisation. Override them in the scenario when you have evidence of your own.

## Two different "clusters"

Keep these separate:

| Thing | What it is |
| --- | --- |
| Infrastructure | The machine, Docker Compose stack, or k3s cluster that **runs** CyberEpi |
| Population | The fictional organisation **inside** a scenario |

A Kubernetes pod running the worker is not a simulated host. You can optionally import cluster **metadata** and turn that into a synthetic contact graph, then simulate on the graph. That import is read-only and does not perform an attack. Secret values are not part of the model.

## How a scenario becomes a result

1. A template builds a fictional organisation: users, endpoints, servers, SaaS apps, pipelines, Kubernetes-like identities, and the relationships among them.
2. A pathogen profile weights those relationships. An identity-focused profile spreads more readily along authentication edges than along ordinary network edges.
3. Controls scale the rates. MFA coverage of 0.8, for example, reduces identity-channel transmission for a deterministic 80% of entities. The same seed always covers the same entities.
4. One index case starts infectious. Everyone else starts susceptible, unless a control such as patching places some entities in the protected compartment.
5. The engine runs a stochastic trajectory, either Gillespie (continuous-time) or discrete-time. The same scenario and seed replay the same run.
6. Repeating the run hundreds or thousands of times produces a distribution: median outbreak size, the upper tail, extinction, and when Rₜ drops below 1.

### Compartments

The full model is called SEIDQRP. Each entity is in exactly one compartment:

| State | Reading |
| --- | --- |
| Susceptible | Reachable, not yet exposed |
| Exposed | Exposed, not yet able to propagate |
| Infectious | Compromised and able to propagate |
| Detected | Noticed, still able to propagate until isolation |
| Quarantined | Removed from the contact process |
| Recovered | Restored, with immunity that can wane |
| Protected | Patched or otherwise immune for a time |

Shorter presets exist for classic epidemic models: `sis`, `sir`, `seir`, `seirs`. The default is `seidqrp`.

### The two reproduction numbers

**R₀** is the average number of secondary compromises one compromised entity would cause if everyone else were still susceptible.

**Invasion threshold** is the leading eigenvalue of the next-generation operator on the contact graph. It can be much larger than R₀ when a few hubs can amplify a spark. That gap is the superspreader structure. It does not mean every random case becomes a large outbreak. The Monte Carlo extinction rate shows how often a spark fails to take off.

**Rₜ** is the same kind of expectation later in the outbreak, using only neighbours that are still susceptible. Above 1, the simulated outbreak is expanding. Below 1, it is contracting.

**Attack rate** is the share of entities that were exposed or compromised during the run.

**Extinction**, unless you change the threshold, means the outbreak stayed under `max(10, 0.5% of the population)`.

More of the mathematics is in [docs/modelling/transmission.md](docs/modelling/transmission.md), [docs/modelling/state-models.md](docs/modelling/state-models.md), and [docs/methodology/reproduction-number.md](docs/methodology/reproduction-number.md).

## Choose how to run it

| Goal | What to use |
| --- | --- |
| One scenario on your laptop, no cluster | CLI: `cyberepi simulate` |
| The laboratory UI, workers, and a database | Docker Compose |
| A homelab k3s cluster | Helm chart in `charts/cyberepi` |

You do not need Kafka, Elasticsearch, Ceph, Longhorn, a service mesh, or a paid API. The reference homelab is one to three k3s nodes, roughly 4–8 CPU cores and 8–16 GB RAM each, with the local-path provisioner. amd64 and arm64 images are the release targets.

## Run the first simulation with the CLI

This path needs [Rust](https://rustup.rs) stable. It does not need Docker, Kubernetes, Postgres, or NATS.

```bash
git clone https://github.com/jusso-dev/cyberepi.git
cd cyberepi
cargo run --release -p cyberepi -- simulate scenarios/demos/smoke.yaml
```

`smoke.yaml` is a small organisation and a handful of runs, so you can see a report quickly. The reference scenario is larger:

```bash
cargo run --release -p cyberepi -- simulate scenarios/demos/basic-outbreak.yaml
```

That file is a synthetic medium enterprise of 10,000 entities, the `IdentityStealer` profile, one index case, the Gillespie algorithm, 7 days, and 1,000 runs. A release run on a recent Apple Silicon machine took under half a minute. Debug builds are much slower; use `--release` for anything beyond a smoke test.

Override the file without editing it:

```bash
cargo run --release -p cyberepi -- simulate scenarios/demos/basic-outbreak.yaml --runs 50
cargo run --release -p cyberepi -- simulate scenarios/demos/basic-outbreak.yaml --format json
cargo run --release -p cyberepi -- simulate scenarios/demos/basic-outbreak.yaml --format csv
```

`--format` accepts `text` (default), `json`, `csv`, or `yaml`. Progress counts are printed on stderr. The report is on stdout.

Install the binary on your `PATH` if you want the shorter command:

```bash
cargo install --path cli
cyberepi simulate scenarios/demos/basic-outbreak.yaml
```

### How to read the report

A release run of `scenarios/demos/basic-outbreak.yaml` on version 0.1.0 produced:

```text
CyberEpi Digital Outbreak Simulation
Population:             10,000
Initial compromises:    1
Simulation duration:    7 days
Runs:                   1,000
R0:                     1.68
Invasion threshold:     551.93
Median peak prevalence: 1,609
Median attack rate:     27.0%
Median outbreak size:   2,700
95th percentile size:   6,891
99th percentile size:   7,140
Median containment:     7 days
Extinction probability: 24.8%
Top superspreaders:
1. idp-primary
2. email-gateway
3. segment-router-05
Rt dropped below 1:
32h 0m
```

Reading it:

- **R₀ 1.68** means a typical susceptible entity, if compromised while everyone else is still susceptible, would be expected to cause about 1.7 further compromises. Above 1, spread can sustain itself.
- **Invasion threshold 552** means the identity provider and similar hubs can amplify far more than the average entity. Most index cases do not become that hub. Some do.
- **Median outbreak 2,700** and **attack rate 27%** are the middle of 1,000 stochastic runs, not a single lucky trajectory.
- **95th and 99th percentiles** are the bad tail. Plan against those if you care about the severe draws, not only the median.
- **Extinction 24.8%** is the share of runs that never really established.
- **Rₜ dropped below 1 at 32h** is the median time the outbreak switched from expanding to contracting.
- **Containment 7 days** here means many runs were still not fully quiet at the end of the simulated week, so the time is censored at the horizon. Lengthen `duration` if you need the tail of recovery.
- **Superspreaders** for this identity-focused profile are the identity provider and the email gateway. A supply-chain profile ranks CI and repository identities instead. The score is structural plus observed spread. The text report includes a short reason for each name.

The same file and the same CyberEpi version reproduce this report. Change a coverage value or the seed and the scenario hash changes with it. Provenance at the bottom of the report records version, hash, seed, algorithm, pathogen, and generator version.

## Run the laboratory with Docker Compose

This path needs Docker with Compose. The first build compiles the Rust services and the Next.js UI, so give it several minutes.

```bash
git clone https://github.com/jusso-dev/cyberepi.git
cd cyberepi
cp .env.example .env
docker compose up --build
```

Open <http://localhost>.

Development login, unless you changed `.env`:

- user: `admin`
- password: `cyberepi-dev`

Change `CYBEREPI_BOOTSTRAP_PASSWORD` before this port is reachable by anyone else. The value in `.env.example` is a local placeholder, not a production secret.

What comes up:

| Service | Role |
| --- | --- |
| Traefik | Routes `/` to the UI and `/api` to the controller |
| PostgreSQL | Scenarios, experiments, users, run summaries |
| NATS JetStream | Hands simulation batches to workers |
| Controller | HTTP API, login, experiment records |
| Worker | Runs batches with the same engine as the CLI |
| Web | The laboratory interface |

Then, in the browser:

1. Sign in.
2. Choose **Run demo experiment**, or open **Scenarios** and build one with the sliders.
3. Watch `completed / requested` until the experiment finishes.
4. Read R₀, the median outbreak, the epidemic curve, the intervention table, and the superspreader list.
5. Open a high-degree identity in the contact sample to see type, privilege, criticality, and susceptibility.

The demo scenario is smaller than `basic-outbreak.yaml` so the UI returns without waiting on a 10,000-by-1,000 run. Use the CLI, or submit that YAML from the advanced editor, when you want the reference experiment.

Scale workers on one machine:

```bash
docker compose up --build --scale worker=3
```

Prometheus and Grafana are optional:

```bash
docker compose --profile observe up --build
```

Grafana is on <http://localhost:3001>. Dashboards live in `grafana/dashboards/` and are provisioned automatically. They cover cluster overview, worker performance, simulation throughput, experiments, and epidemiology metrics. Metrics are not labelled per entity, so a large simulation cannot blow up Prometheus cardinality.

## Install on k3s

This path needs a running k3s (or another Kubernetes) cluster and Helm 3.

From a clone, before images are in a registry:

```bash
docker build -t ghcr.io/jusso-dev/cyberepi:0.1.0 .
docker build -f web/Dockerfile -t ghcr.io/jusso-dev/cyberepi-web:0.1.0 .
docker save ghcr.io/jusso-dev/cyberepi:0.1.0 | sudo k3s ctr images import -
docker save ghcr.io/jusso-dev/cyberepi-web:0.1.0 | sudo k3s ctr images import -
```

Install:

```bash
helm install cyberepi ./charts/cyberepi \
  --namespace cyberepi \
  --create-namespace
```

The chart creates the namespace resources, PostgreSQL on a local-path volume, NATS, the controller, workers, and the UI. Ingress defaults to Traefik and the host `cyberepi.local`. Point that name at the node, or set another host:

```bash
helm install cyberepi ./charts/cyberepi \
  --namespace cyberepi \
  --create-namespace \
  --set ingress.host=cyberepi.home.arpa \
  --set worker.replicas=3
```

The administrator password is generated into a Secret when you do not set one:

```bash
kubectl get secret cyberepi-auth -n cyberepi \
  -o jsonpath='{.data.password}' | base64 -d; echo
```

The username defaults to `admin`.

Single-node k3s is enough. Set `worker.replicas=1` there. On a three-node homelab, `3` is a reasonable start. Horizontal pod autoscaling is **off** unless metrics-server is installed:

```bash
helm upgrade cyberepi ./charts/cyberepi -n cyberepi --set hpa.enabled=true
```

Published release images, once a tag exists, are:

- `ghcr.io/jusso-dev/cyberepi` (controller, worker, and CLI)
- `ghcr.io/jusso-dev/cyberepi-web`

Both are built for linux/amd64 and linux/arm64. More k3s notes are in [deployments/k3s/README.md](deployments/k3s/README.md).

## Scenarios

A scenario is YAML. Validate it before a long run:

```bash
cyberepi scenario validate scenarios/demos/basic-outbreak.yaml
```

The schema is [schemas/scenario.schema.json](schemas/scenario.schema.json).

```yaml
apiVersion: cyberepi.io/v1
kind: Scenario
metadata:
  name: mfa-effectiveness-demo
population:
  template: medium-enterprise   # see templates below
  size: 5000                    # total entities, minimum 32
  seed: 7                       # organisation generator seed
pathogen:
  type: IdentityStealer
initial_conditions:
  infected_entities: 1
  strategy: random              # or targeted (highest expected spread)
model: seidqrp                  # sis | sir | seir | seirs | seidqrp
controls:
  mfa:
    coverage: 0.80              # 0..1, overridable multipliers exist
  edr:
    coverage: 0.90
defender:
  policy: no-response           # or immediate-isolation, risk-based-isolation,
                                # critical-asset-protection
simulation:
  algorithm: gillespie          # or discrete
  transmission: clamped-product # or exponential-saturation
  duration: 7d                  # also 15m, 1h, 1h30m
  runs: 1000
  seed: 12345
  event_mode: summary           # summary | sampled | full
  sample_interval: 1h
```

`strategy: random` draws the index case from identities and accounts, which are the entities that actually have contacts. `targeted` starts at the entity with the highest expected secondary-case count.

`event_mode: summary` stores curves and scalars. `sampled` keeps a full event trace on every 20th run. `full` keeps events for the runs in that slice. Playback can also re-simulate one seed, because the seed determines the trajectory.

Variants compare controls without copying the whole file. Each variant merges onto the base `controls` block:

```yaml
controls: {}
variants:
  - name: No MFA
    controls: {}
  - name: 50% MFA
    controls:
      mfa:
        coverage: 0.50
  - name: 90% MFA
    controls:
      mfa:
        coverage: 0.90
```

The report then includes one block per variant and a comparison table of median size, R₀, peak, and containment.

### Organisation templates

`population.template` is one of:

| Template id | Shape |
| --- | --- |
| `micro-business` | Few people, light administration |
| `small-business` | Small staff, a handful of admins and SaaS apps |
| `medium-enterprise` | The reference population |
| `large-enterprise` | More segments, admins, and servers |
| `government-agency` | User-heavy, fewer cloud workloads |
| `cloud-native-startup` | More Kubernetes identities, pipelines, and workloads |
| `hybrid-enterprise` | Endpoints plus a real cloud footprint |
| `software-company` | Repositories, CI, and automation identities |
| `university` | Many users, lower security maturity |
| `managed-service-provider` | More administrative paths across segments |

`population.size` is the **total** number of entities, not the number of human users. The template decides the mix. `cyberepi generate organisation --users 5000` uses that number the same way: it is the population size.

Maturity in the template sets illustrative detection delay, patch level, and segmentation. It is not a compliance score.

### Pathogen profiles

| Name | Bias |
| --- | --- |
| `IdentityStealer` | Authentication, identity, and SaaS relationships |
| `RapidWorm` | Endpoints and network contact, short latent period |
| `EmailBorne` | Email contact, with some identity follow-on |
| `CloudControlPlane` | Cloud and identity relationships |
| `SupplyChain` | Repositories and CI/CD relationships |
| `SlowPersistent` | Low intensity, long latent period, weak detection |
| `RansomwareLike` | Faster, more visible endpoint and network spread |
| `InsiderLike` | Trusted identity edges, easy to miss |

Names also accept the kebab-case ids (`identity-stealer`, and so on).

### Controls

Each control is optional. Coverage is the fraction of entities that receive it. Strength, where present, is 0 to 1. You can override any multiplier; the catalog defaults are the illustrative assumptions.

`mfa`, `edr`, `segmentation`, `patching`, `rapid_isolation`, `credential_revocation`, `least_privilege`, `application_allowlisting`, `email_security`, `dns_filtering`, `privileged_access_management`, `conditional_access`, `zero_trust_policy`, `backup_isolation`, `user_awareness`, `asset_removal`.

Example of an explicit isolation delay, used by the enterprise demo:

```yaml
rapid_isolation:
  coverage: 1
  isolation_delay: 15m
```

### Defender policies

These are deterministic policies inside the simulation. They do not act on your cluster.

| Policy | Behaviour |
| --- | --- |
| `no-response` | Leave the stochastic clocks alone |
| `immediate-isolation` | Quarantine an entity when it is detected |
| `risk-based-isolation` | Do that for high criticality or high privilege |
| `critical-asset-protection` | Start high-criticality entities protected |

The interface is `DefenderPolicy` in `crates/epi-models`. A later rules engine or learned policy can implement the same decisions: isolate, monitor, revoke, segment, patch, investigate, or do nothing. See [docs/research/defender-policies.md](docs/research/defender-policies.md).

### Demos in the repository

| File | What it is for |
| --- | --- |
| `scenarios/demos/smoke.yaml` | Tiny run for a first look and for tests |
| `scenarios/demos/basic-outbreak.yaml` | 10,000 entities, 1,000 runs, the reference report |
| `scenarios/demos/small-business-mfa.yaml` | No MFA vs 50%, 90%, and 100% |
| `scenarios/demos/enterprise-isolation.yaml` | Isolation at 15 minutes, 1 hour, 4 hours, 24 hours |
| `scenarios/demos/k8s-control-plane.yaml` | Cloud-native population and a least-privilege variant |
| `scenarios/demos/superspreader.yaml` | Supply-chain profile, ranked hubs |
| `scenarios/demos/ui-demo.yaml` | The smaller experiment behind **Run demo experiment** |

```bash
cyberepi simulate scenarios/demos/small-business-mfa.yaml --runs 40
cyberepi simulate scenarios/demos/enterprise-isolation.yaml --runs 40
cyberepi simulate scenarios/demos/superspreader.yaml --runs 40
```

`--runs` replaces the `runs` field for that invocation only.

## CLI reference

```bash
cyberepi scenario validate <file>
cyberepi simulate <file> [--runs N] [--seed N] [--threads N] [--format text|json|csv|yaml]
cyberepi experiment run <file> [--out experiment.json] [--runs N] [--threads N]
cyberepi experiment results <experiment.json>
cyberepi experiment status <experiment.json>
cyberepi generate organisation --template medium-enterprise --users 5000 [--seed N] [--name "..."]
cyberepi cluster status
```

`simulate` and `experiment run` both execute locally. `experiment run --out` writes a JSON bundle you can open again with `experiment results`. `generate organisation` prints entity counts, edge counts, and the largest connected component. It does not create a real directory of accounts.

`cluster status` prints whether `KUBECONFIG` is set and reminds you that infrastructure and the simulated population are different. Live topology import is a read-only library in `crates/epi-kubernetes` (`--features live`). It lists names and relationships. It does not read secret values.

## API

With Compose or Helm, the API is under `/api` on the same host as the UI. OpenAPI is at `/api/v1/openapi.json`. A short HTML index is at `/api/docs`.

Local session cookie: `POST /api/v1/auth/login` with `{"username","password"}`.

Useful routes:

```text
POST /api/v1/scenarios
GET  /api/v1/scenarios
POST /api/v1/scenarios/validate
POST /api/v1/experiments
GET  /api/v1/experiments/{id}
POST /api/v1/experiments/{id}/cancel
GET  /api/v1/experiments/{id}/results
GET  /api/v1/experiments/{id}/export
GET  /api/v1/experiments/{id}/charts/epidemic.png
GET  /api/v1/pathogens
GET  /api/v1/controls
GET  /api/v1/cluster
GET  /api/v1/ws
```

Creating an experiment publishes batches to NATS. Workers acknowledge a batch only after the result is stored. A redelivered batch that is already complete is skipped. Progress is pushed on the WebSocket; the experiment page also refreshes while a run is in progress.

`/api/v1/demo` starts `scenarios/demos/ui-demo.yaml`.

## Repository map

```text
crates/epi-core          entities, compartments, durations
crates/epi-graph         multiplex contact graph
crates/epi-models        transmission laws, R0, defender policies
crates/epi-pathogens     synthetic profiles
crates/epi-controls      illustrative control modifiers
crates/epi-generator     organisation templates
crates/epi-simulator     Gillespie and discrete engines, reports
crates/epi-scenario      YAML loading and execution
crates/epi-kubernetes    metadata-only topology import
crates/epi-orchestrator  Postgres, NATS, sessions
cli/                     cyberepi binary
services/controller      HTTP API
services/worker          batch runner
web/                     Next.js laboratory
charts/cyberepi          Helm chart
scenarios/demos          example scenarios
docs/                    model notes
grafana/dashboards       provisioned dashboards
```

The simulator, the CLI, and the workers share one engine. A result you trust from `cyberepi simulate` is the same mathematics the cluster runs.

## Develop

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
helm lint charts/cyberepi
cd web && npm ci && npm run lint && npm run typecheck && npm test
```

UI development, once a controller is listening on port 8080:

```bash
cd web
npm ci
npm run dev
```

Next.js rewrites `/api` to `CONTROLLER_URL` (default `http://127.0.0.1:8080`). The supported full stack is still `docker compose up`.

Benchmark a chosen population size:

```bash
CYBEREPI_BENCH_SCALE=10000 cargo bench -p epi-simulator
```

`1000` is the default. `100000` and `1000000` are accepted; run those on the homelab, not in CI.

## Observability

Controller metrics are on `:8080/metrics`. Worker metrics are on `:9102/metrics`. Names include:

- `cyberepi_worker_count`
- `cyberepi_active_jobs`
- `cyberepi_queue_depth`
- `cyberepi_simulations_total`
- `cyberepi_simulations_failed_total`
- `cyberepi_simulation_duration_seconds`
- `cyberepi_current_infected`
- `cyberepi_rt`
- `cyberepi_attack_rate`
- `cyberepi_peak_prevalence`

## Safety

Do not treat this repository as an attack framework. There is no exploit payload, scanner, credential theft, persistence, or destructive action. Controls only change simulation parameters. Kubernetes permissions used for optional discovery are `get`, `list`, and `watch`. They do not include create, update, patch, delete, bind, escalate, or impersonate.

Details: [SAFETY.md](SAFETY.md).

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Keep new default rates labelled as illustrative assumptions. Add a test when you change the engine. Seeds must stay reproducible.

## Roadmap

- Optional OIDC (Authentik, Keycloak, Entra ID, Google Workspace, Cloudflare Access) without making an external provider mandatory
- PDF reports
- Richer outbreak playback in the UI
- Further defender policies on the existing `DefenderPolicy` trait

## License

Apache License 2.0. See [LICENSE](LICENSE).
