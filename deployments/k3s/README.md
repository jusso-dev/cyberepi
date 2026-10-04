# k3s

CyberEpi's reference install is a Helm release on k3s. The chart uses the local-path provisioner. It does not need Kafka, Elasticsearch, Ceph, Longhorn, or a service mesh.

```bash
helm install cyberepi ./charts/cyberepi \
  --namespace cyberepi \
  --create-namespace
```

On a single node, set `worker.replicas=1`. On a three-node homelab, `worker.replicas=3` is a reasonable start. HPA is off unless metrics-server is installed (`--set hpa.enabled=true`).

Build local images and import them into k3s when you are not using a registry:

```bash
docker build -t ghcr.io/jusso-dev/cyberepi:0.1.0 .
docker build -f web/Dockerfile -t ghcr.io/jusso-dev/cyberepi-web:0.1.0 .
docker save ghcr.io/jusso-dev/cyberepi:0.1.0 | sudo k3s ctr images import -
docker save ghcr.io/jusso-dev/cyberepi-web:0.1.0 | sudo k3s ctr images import -
```

Then install with `--set image.pullPolicy=IfNotPresent`.

`deployments/k3s/topology-reader.yaml` is the read-only ClusterRole for optional topology import. The chart installs the same role. Discovery requests secret metadata only. Do not grant this role permission to read secret values in application code.

The simulated population is not the set of pods running CyberEpi.
